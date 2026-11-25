use std::time::{Duration, Instant};

use futures::StreamExt;
use phoenix_api::{Cache, ClientProperties, Gateway, GatewayConfig, GatewayEvent, models as api};
use phoenix_app::{
    discord_backend::store::{KeyringStore, TokenStore},
    notifications::{self as n, Level, NotifyPrefs, Target, dbus},
};

const TEST_CHANNEL: u64 = 1_574_797_385_463_496_724;
const STAND_IN: api::Snowflake = api::Snowflake(1);

fn private_bus() -> anyhow::Result<String> {
    let bus = std::env::var("PHOENIX_NOTIFY_PRIVATE_BUS").unwrap_or_default();
    if bus.is_empty() || bus.contains("/run/user/") || Some(&bus) == std::env::var("DBUS_SESSION_BUS_ADDRESS").ok().as_ref() {
        anyhow::bail!("refusing to run: PHOENIX_NOTIFY_PRIVATE_BUS must name a private dbus-run-session bus (scripts/notify/real_test.sh)");
    }
    Ok(bus)
}

fn main() -> anyhow::Result<()> {
    let bus = private_bus()?;
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(120);
    println!("probe: private bus {bus}, listening {secs}s on channel {TEST_CHANNEL}");
    let token = tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
        tokio::task::spawn_blocking(|| KeyringStore.load()).await?.map_err(anyhow::Error::msg)?.ok_or_else(|| anyhow::anyhow!("no token in keyring"))
    })?;
    // SAFETY: the keyring runtime is gone and no other thread reads the environment yet.
    unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &bus) };
    let (tx, mut clicks) = futures::channel::mpsc::unbounded::<dbus::Click>();
    if !dbus::spawn(tx) {
        anyhow::bail!("notification worker did not start");
    }
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
    rt.block_on(async move {
        let mut cfg = GatewayConfig::new(token.expose());
        cfg.properties = ClientProperties::linux_desktop();
        let (_gw, mut rx) = Gateway::spawn(cfg);
        let cache = Cache::default();
        let ready = loop {
            let Some(ev) = tokio::time::timeout(Duration::from_secs(40), rx.recv()).await? else { anyhow::bail!("gateway closed before READY") };
            cache.apply(&ev);
            match ev {
                GatewayEvent::Ready(r) => break r,
                GatewayEvent::Fatal { code, reason } => anyhow::bail!("fatal {code}: {reason}"),
                _ => {}
            }
        };
        let me = ready.user.id;
        let prefs = NotifyPrefs::from_ready_with_defaults(
            ready.user_guild_settings.as_ref(),
            ready.guilds.iter().map(|g| (g.id.0, Level::from_default(g.properties.default_message_notifications))).collect(),
        );
        let ch = cache.channel(api::Snowflake(TEST_CHANNEL)).ok_or_else(|| anyhow::anyhow!("test channel not visible"))?;
        let g = ch.guild_id.map(|g| g.0);
        let parent = ch.parent_id.map(|p| p.0);
        println!(
            "probe: READY, #{} level {:?}, muted {}",
            ch.name.clone().unwrap_or_default(),
            prefs.resolved_level(g, TEST_CHANNEL, parent),
            prefs.is_guild_or_category_or_channel_muted(g, TEST_CHANNEL, parent)
        );
        let ctx = phoenix_app::sounds::message_ctx();
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut shown = 0u32;
        while Instant::now() < deadline {
            tokio::select! {
                ev = tokio::time::timeout(Duration::from_secs(1), rx.recv()) => {
                    let Ok(Some(ev)) = ev else { continue };
                    cache.apply(&ev);
                    let GatewayEvent::MessageCreate(m) = &ev else { continue };
                    if m.channel_id.0 != TEST_CHANNEL {
                        continue;
                    }
                    let real = n::show_message(&prefs, &n::facts(&cache, me, m), &ctx);
                    let other = n::show_message(&prefs, &n::facts(&cache, STAND_IN, m), &ctx);
                    println!("probe: MESSAGE_CREATE {} from_me={} -> as us: {real}, as another account: {other}", m.id, m.author.id == me);
                    if (real || other) && let Some(t) = n::build(&cache, m) {
                        println!("probe: toast title={:?} body={:?} icon={:?}", t.title, t.body, t.icon);
                        n::show(t);
                        shown += 1;
                    }
                }
                Some(c) = clicks.next() => {
                    println!("probe: click -> target {:?}, activation token {:?}", c.target, c.token);
                    if c.target == Target::Channel(TEST_CHANNEL) {
                        println!("probe: PASS ({shown} shown, click routed to the channel)");
                        n::clear(TEST_CHANNEL);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        return Ok(());
                    }
                }
            }
        }
        anyhow::bail!("no click received ({shown} notifications shown)")
    })
}

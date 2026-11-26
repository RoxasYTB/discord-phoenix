#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use futures::StreamExt;
#[cfg(target_os = "linux")]
use phoenix_api::{Cache, ClientProperties, Gateway, GatewayConfig, GatewayEvent, RestClient, models as api, rest::CreateMessage};
#[cfg(target_os = "linux")]
use phoenix_app::{
    discord_backend::store::{KeyringStore, TokenStore},
    notifications::{self as n, Level, NotifyPrefs, Target, dbus},
};

#[cfg(target_os = "linux")]
const TEST_CHANNEL: u64 = 1_574_797_385_463_496_724;
#[cfg(target_os = "linux")]
const TEST_GUILD: u64 = 1_369_382_919_948_210_248;
#[cfg(target_os = "linux")]
const STAND_IN: api::Snowflake = api::Snowflake(1);
#[cfg(target_os = "linux")]
const CUES: [&str; 9] = ["message1", "message3", "discodo", "user_join", "user_leave", "disconnect", "deafen", "mute", "call_ringing"];

#[cfg(target_os = "linux")]
async fn send_test(rest: &RestClient, me: api::Snowflake) -> anyhow::Result<()> {
    let ch = rest.get_channel(api::Snowflake(TEST_CHANNEL)).await?;
    let guild = rest.request_json(reqwest::Method::GET, &format!("/guilds/{TEST_GUILD}"), None).await?;
    let (gname, cname) = (guild["name"].as_str().unwrap_or_default(), ch.name.clone().unwrap_or_default());
    anyhow::ensure!(
        ch.guild_id == Some(api::Snowflake(TEST_GUILD)) && gname == "Aperture Sciences" && cname.contains("phoenix"),
        "refusing: channel/guild names do not match the test channel"
    );
    let text = format!("[Test Phoenix] envoyé depuis le client Rust (Phoenix) — notification de bureau, mention et sons <@{}>", me.0);
    let m = rest.send_message(api::Snowflake(TEST_CHANNEL), CreateMessage::text(text)).await?;
    println!("probe: sent test message {} in {gname} / {cname}", m.id);
    Ok(())
}

#[cfg(target_os = "linux")]
fn summary(cache: &Cache, ready: &phoenix_api::gateway::Ready) {
    let rels = cache.relationships();
    let skipped = |u: api::Snowflake| rels.iter().any(|r| r.id == u && (r.is_spam_request || r.user_ignored));
    let friends = rels.iter().filter(|r| r.kind == api::RelationshipType::IncomingRequest && !r.is_spam_request && !r.user_ignored).count();
    let games = api::GameRelationship::parse_all(ready.game_relationships.as_ref())
        .into_iter()
        .filter(|g| g.kind == api::RelationshipType::IncomingRequest && !skipped(g.id))
        .count();
    println!(
        "probe: pending requests {friends} + game {games}, restricted schedule {}, restricted now {}",
        ready.user.restricted_schedule.is_some(),
        n::restricted::active_now()
    );
    for cue in CUES {
        let ok = phoenix_app::sounds::assets::load(cue).and_then(|b| phoenix_app::sounds::assets::decode(&b)).is_some();
        println!("probe: sound {cue} {}", if ok { "decodes" } else { "MISSING" });
    }
}

#[cfg(target_os = "linux")]
fn private_bus() -> anyhow::Result<String> {
    let bus = std::env::var("PHOENIX_NOTIFY_PRIVATE_BUS").unwrap_or_default();
    if bus.is_empty() || bus.contains("/run/user/") || Some(&bus) == std::env::var("DBUS_SESSION_BUS_ADDRESS").ok().as_ref() {
        anyhow::bail!("refusing to run: PHOENIX_NOTIFY_PRIVATE_BUS must name a private dbus-run-session bus (scripts/notify/real_test.sh)");
    }
    Ok(bus)
}

#[cfg(target_os = "linux")]
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
        n::on_ready(&cache);
        summary(&cache, &ready);
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
        if std::env::var_os("PROBE_SEND").is_some() {
            send_test(&RestClient::new(token.expose(), ClientProperties::linux_desktop()), me).await?;
        }
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
                    let mut theirs = (**m).clone();
                    theirs.author.id = STAND_IN;
                    let f = n::facts(&cache, me, &theirs);
                    let mentioned = n::show_message(&prefs, &f, &ctx);
                    let notify = n::should_notify(&prefs, &f);
                    let cues = phoenix_app::sounds::message_sounds(&ctx, &f, theirs.flags, f.blocked, notify, !prefs.allow_no_messages(f.guild, f.channel, f.parent));
                    println!(
                        "probe: MESSAGE_CREATE {} from_me={} -> as us: {real}, as another account: {other}, sent by another account to us (mentions us: {}): popup {mentioned}, sounds {cues:?}",
                        m.id,
                        m.author.id == me,
                        f.mentions_me
                    );
                    if (real || other || mentioned) && let Some(t) = n::build(&cache, m) {
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

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("notify_probe is only available on Linux");
}

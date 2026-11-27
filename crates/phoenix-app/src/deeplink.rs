use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures::{StreamExt, channel::mpsc};
use gpui::{App, Context, Global, WeakEntity};

use crate::{app::PhoenixApp, model::Id};

pub mod ipc;

pub const PROTOCOL: &str = "discord";
pub const MULTI_INSTANCE_FLAG: &str = "--multi-instance";
pub const MULTI_INSTANCE_ENV: &str = "PHOENIX_MULTI_INSTANCE";
pub const REGISTER_FLAG: &str = "--register-protocol";

const RETRY_EVERY: Duration = Duration::from_millis(500);
const RETRY_FOR: u32 = 120;
const GUILD_FALLBACK_AFTER: u32 = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Invite { code: String, event: Option<Id> },
    Channel { guild: Option<Id>, channel: Option<Id>, message: Option<Id> },
    Other(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub token: Option<String>,
    pub url: Option<String>,
}

impl Message {
    pub fn encode(&self) -> String {
        let clean = |s: &Option<String>| s.as_deref().unwrap_or_default().replace(['\t', '\n', '\r'], "");
        format!("{}\t{}\n", clean(&self.token), clean(&self.url))
    }

    pub fn decode(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\n', '\r']);
        let (token, url) = line.split_once('\t')?;
        let some = |s: &str| (!s.is_empty()).then(|| s.to_owned());
        Some(Self { token: some(token), url: some(url) })
    }
}

const WEB_HOSTS: [&str; 6] = ["discord.com", "www.discord.com", "ptb.discord.com", "canary.discord.com", "discordapp.com", "www.discordapp.com"];
const INVITE_HOSTS: [&str; 2] = ["discord.gg", "www.discord.gg"];

fn snowflake(s: &str) -> Option<Id> {
    s.parse::<u64>().ok().filter(|v| *v > 0).map(Id)
}

pub fn parse(url: &str) -> Option<Target> {
    let u = reqwest::Url::parse(url.trim()).ok()?;
    let host = u.host_str().unwrap_or_default().to_ascii_lowercase();
    let event = u.query_pairs().find(|(k, _)| k == "event").and_then(|(_, v)| snowflake(&v));
    match u.scheme() {
        PROTOCOL => Some(route(u.path(), event)),
        "http" | "https" if INVITE_HOSTS.contains(&host.as_str()) => {
            let code = u.path().trim_matches('/').split('/').next().filter(|c| !c.is_empty())?;
            Some(Target::Invite { code: code.to_owned(), event })
        }
        "http" | "https" if WEB_HOSTS.contains(&host.as_str()) => match route(u.path(), event) {
            Target::Other(_) => None,
            t => Some(t),
        },
        _ => None,
    }
}

pub fn route(path: &str, event: Option<Id>) -> Target {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        ["invite" | "invite-proxy", code, ..] => Target::Invite { code: (*code).to_owned(), event },
        ["channels", guild, rest @ ..] => {
            let guild_id = if *guild == "@me" { None } else { snowflake(guild) };
            if guild_id.is_none() && *guild != "@me" {
                return Target::Other(path.to_owned());
            }
            let channel = rest.first().and_then(|c| snowflake(c));
            let message = channel.and(rest.get(1).and_then(|m| snowflake(m)));
            Target::Channel { guild: guild_id, channel, message }
        }
        _ => Target::Other(format!("/{}", parts.join("/"))),
    }
}

pub fn url_from_args(args: &[String]) -> Option<String> {
    if let Some(at) = args.iter().position(|a| a == "--url") {
        let pass = args.iter().position(|a| a == "--")?;
        if pass < at {
            return None;
        }
        return args.get(pass + 1).cloned();
    }
    let prefix = format!("{PROTOCOL}://");
    args.iter().skip(1).find(|a| a.to_ascii_lowercase().starts_with(&prefix)).cloned()
}

pub fn multi_instance(args: &[String]) -> bool {
    args.iter().any(|a| a == MULTI_INSTANCE_FLAG) || std::env::var(MULTI_INSTANCE_ENV).is_ok_and(|v| !v.is_empty() && v != "0")
}

pub fn windows_command(exe: &str) -> String {
    format!("\"{exe}\" --url -- \"%1\"")
}

pub fn register_protocol() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().into_owned();
    if cfg!(windows) {
        let key = format!("HKCU\\Software\\Classes\\{PROTOCOL}");
        let icon = format!("\"{exe}\",0");
        let command = windows_command(&exe);
        let sets: [(String, Option<&str>, &str); 4] = [
            (key.clone(), None, "URL:Discord Protocol"),
            (key.clone(), Some("URL Protocol"), ""),
            (format!("{key}\\DefaultIcon"), None, icon.as_str()),
            (format!("{key}\\shell\\open\\command"), None, command.as_str()),
        ];
        for (k, name, data) in sets {
            let mut cmd = std::process::Command::new("reg");
            cmd.args(["add", &k, "/f", "/d", data]);
            match name {
                Some(n) => cmd.args(["/v", n]),
                None => cmd.arg("/ve"),
            };
            let ok = cmd.status().map_err(|e| e.to_string())?.success();
            if !ok {
                return Err(format!("reg add {k} failed"));
            }
        }
        return Ok(());
    }
    if cfg!(target_os = "macos") {
        return Ok(());
    }
    let ok = std::process::Command::new("xdg-mime")
        .args(["default", "discord-phoenix.desktop", &format!("x-scheme-handler/{PROTOCOL}")])
        .status()
        .map_err(|e| e.to_string())?
        .success();
    if ok { Ok(()) } else { Err("xdg-mime default failed".into()) }
}

pub fn launch_token() -> Option<String> {
    ["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}

pub struct AppTarget(pub WeakEntity<PhoenixApp>);

impl Global for AppTarget {}

pub fn set_target(app: WeakEntity<PhoenixApp>, cx: &mut App) {
    cx.set_global(AppTarget(app));
}

pub fn channel() -> (mpsc::UnboundedSender<Message>, mpsc::UnboundedReceiver<Message>) {
    mpsc::unbounded()
}

pub fn serve(mut rx: mpsc::UnboundedReceiver<Message>, cx: &mut App) {
    cx.spawn(async move |cx| {
        while let Some(msg) = rx.next().await {
            let mut waited = 0;
            loop {
                let ready = cx.update(|cx| cx.try_global::<AppTarget>().and_then(|t| t.0.upgrade()).is_some());
                if ready || waited >= RETRY_FOR {
                    break;
                }
                waited += 1;
                cx.background_executor().timer(RETRY_EVERY).await;
            }
            cx.update(|cx| deliver(msg, cx));
        }
    })
    .detach();
}

fn deliver(msg: Message, cx: &mut App) {
    focus(msg.token, cx);
    let Some(url) = msg.url else { return };
    let Some(app) = cx.try_global::<AppTarget>().and_then(|t| t.0.upgrade()) else { return };
    tracing::info!("open url target={:?}", parse(&url).map(|t| kind(&t)));
    app.update(cx, |a, cx| a.open_deep_link(&url, cx));
}

fn kind(t: &Target) -> &'static str {
    match t {
        Target::Invite { .. } => "invite",
        Target::Channel { message: Some(_), .. } => "message",
        Target::Channel { channel: Some(_), .. } => "channel",
        Target::Channel { guild: Some(_), .. } => "guild",
        Target::Channel { .. } => "home",
        Target::Other(_) => "other",
    }
}

#[cfg(target_os = "linux")]
fn focus(token: Option<String>, cx: &mut App) {
    if let Some(t) = token {
        crate::tray::activation::provide(t);
    }
    crate::tray::activate(cx);
}

#[cfg(not(target_os = "linux"))]
fn focus(_token: Option<String>, cx: &mut App) {
    for w in cx.windows() {
        let _ = w.update(cx, |_, window, _| window.activate_window());
    }
}

static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Done,
    Wait,
}

impl PhoenixApp {
    pub fn open_deep_link(&mut self, url: &str, cx: &mut Context<Self>) -> bool {
        let Some(t) = parse(url) else { return false };
        match t {
            Target::Other(_) => {}
            Target::Invite { code, event } => self.open_invite_link(code, event, cx),
            Target::Channel { guild, channel: Some(c), message } => self.open_channel_link(guild, c, message, cx),
            t @ Target::Channel { .. } => self.route_when_ready(t, cx),
        }
        true
    }

    pub fn open_channel_link(&mut self, guild: Option<Id>, channel: Id, message: Option<Id>, cx: &mut Context<Self>) {
        let target = Target::Channel { guild, channel: Some(channel), message };
        if !self.channels.contains_key(&channel)
            && let Some(t) = self.nav.find_thread(channel).cloned()
        {
            self.channels.insert(channel, t);
        }
        let fetch = !self.channels.contains_key(&channel) && guild.is_some_and(|g| self.guilds.iter().any(|x| x.id == g));
        if !fetch {
            return self.route_when_ready(target, cx);
        }
        self.parity_request(crate::commands::ParityRequest::FetchThread { id: channel }, cx, move |a, r, cx| {
            if let Ok(crate::commands::ParityReply::Thread(ch)) = r {
                a.channels.entry(channel).or_insert(ch);
            }
            a.route_when_ready(target, cx);
        });
    }

    pub fn open_in_app_link(&mut self, url: &str, cx: &mut Context<Self>) -> bool {
        let lower = url.trim().to_ascii_lowercase();
        if !lower.starts_with("http://") && !lower.starts_with("https://") {
            return false;
        }
        match parse(url) {
            Some(Target::Other(_)) | None => false,
            Some(_) => self.open_deep_link(url, cx),
        }
    }

    pub fn route_when_ready(&mut self, t: Target, cx: &mut Context<Self>) {
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        let mut opened = false;
        if self.route_step(&t, 0, &mut opened, cx) == Step::Done {
            return;
        }
        cx.spawn(async move |this, cx| {
            let mut opened = opened;
            for tries in 1..=RETRY_FOR {
                cx.background_executor().timer(RETRY_EVERY).await;
                if GENERATION.load(Ordering::Relaxed) != generation {
                    return;
                }
                let step = this.update(cx, |a, cx| a.route_step(&t, tries, &mut opened, cx));
                if !matches!(step, Ok(Step::Wait)) {
                    return;
                }
            }
        })
        .detach();
    }

    fn route_step(&mut self, t: &Target, tries: u32, opened: &mut bool, cx: &mut Context<Self>) -> Step {
        let Target::Channel { guild, channel, message } = *t else { return Step::Done };
        let guild_known = guild.is_none_or(|g| self.guilds.iter().any(|x| x.id == g));
        if *opened {
            let Some(c) = channel.filter(|c| self.selected_channel == Some(*c)) else { return Step::Done };
            return self.jump_when_loaded(c, message, cx);
        }
        match channel {
            Some(c) if self.channels.contains_key(&c) => {
                self.leave_overlays_for_route();
                self.open_channel_chat(c, cx);
                *opened = true;
                cx.notify();
                self.jump_when_loaded(c, message, cx)
            }
            Some(_) if guild.is_some() && guild_known && tries >= GUILD_FALLBACK_AFTER => {
                self.leave_overlays_for_route();
                self.select_guild(guild, cx);
                Step::Done
            }
            Some(_) => Step::Wait,
            None if guild_known && (guild.is_some() || !self.guilds.is_empty() || tries >= GUILD_FALLBACK_AFTER) => {
                self.leave_overlays_for_route();
                self.select_guild(guild, cx);
                Step::Done
            }
            None => Step::Wait,
        }
    }

    fn jump_when_loaded(&mut self, channel: Id, message: Option<Id>, cx: &mut Context<Self>) -> Step {
        let Some(m) = message else { return Step::Done };
        if !self.messages.get(&channel).is_some_and(|e| e.loaded_once && !e.loading) {
            return Step::Wait;
        }
        self.jump_to_message(channel, m, cx);
        Step::Done
    }

    fn leave_overlays_for_route(&mut self) {
        self.parity.inbox.open = false;
        crate::ui::discovery::close();
    }

    pub fn open_invite_link(&mut self, code: String, event: Option<Id>, cx: &mut Context<Self>) {
        crate::ui::modals::accept_invite::open_loading(&code);
        cx.notify();
        let req = match event {
            Some(e) => crate::ui::home::data::NavRequest::FetchInviteEvent(code.clone(), e),
            None => crate::ui::home::data::NavRequest::FetchInvite(code.clone()),
        };
        self.nav_request(req, cx, move |this, res, cx| {
            let preview = match res {
                Ok(crate::ui::home::data::NavReply::Invite(p)) => Some(*p),
                _ => None,
            };
            match preview {
                Some(p) => match crate::ui::modals::accept_invite::member_route(this, &p) {
                    Some(t) => {
                        crate::ui::modals::accept_invite::close_if(&code);
                        this.route_when_ready(t, cx);
                    }
                    None => crate::ui::modals::accept_invite::show(&code, Ok(p)),
                },
                None => crate::ui::modals::accept_invite::show(&code, Err(())),
            }
            cx.notify();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(g: Option<u64>, c: Option<u64>, m: Option<u64>) -> Option<Target> {
        Some(Target::Channel { guild: g.map(Id), channel: c.map(Id), message: m.map(Id) })
    }

    #[test]
    fn protocol_links_route_like_the_desktop_host() {
        assert_eq!(parse("discord://-/channels/1369382919948210248/1574797385463496724"), ch(Some(1369382919948210248), Some(1574797385463496724), None));
        assert_eq!(parse("discord:///channels/1/2/3"), ch(Some(1), Some(2), Some(3)));
        assert_eq!(parse("discord://app/channels/@me/42"), ch(None, Some(42), None));
        assert_eq!(parse("discord://-/channels/@me"), ch(None, None, None));
        assert_eq!(parse("discord://-/channels/7"), ch(Some(7), None, None));
        assert_eq!(parse("DISCORD://-/invite/abc"), Some(Target::Invite { code: "abc".into(), event: None }));
        assert_eq!(parse("discord://-/invite-proxy/xyz"), Some(Target::Invite { code: "xyz".into(), event: None }));
        assert_eq!(parse("discord://app/settings/nitro"), Some(Target::Other("/settings/nitro".into())));
        assert_eq!(parse("discord://-/channels/abc/1"), Some(Target::Other("/channels/abc/1".into())));
        assert_eq!(parse("discord://"), Some(Target::Other("/".into())));
    }

    #[test]
    fn web_links_cover_invites_channels_and_messages() {
        assert_eq!(parse("https://discord.gg/cS8CHeVv"), Some(Target::Invite { code: "cS8CHeVv".into(), event: None }));
        assert_eq!(parse("https://discord.gg/ev?event=77"), Some(Target::Invite { code: "ev".into(), event: Some(Id(77)) }));
        assert_eq!(parse("https://discord.com/invite/abc-1"), Some(Target::Invite { code: "abc-1".into(), event: None }));
        assert_eq!(parse("https://ptb.discord.com/invite/p1"), Some(Target::Invite { code: "p1".into(), event: None }));
        assert_eq!(parse("https://discordapp.com/invite/old"), Some(Target::Invite { code: "old".into(), event: None }));
        assert_eq!(parse("https://discord.com/channels/1/2/3"), ch(Some(1), Some(2), Some(3)));
        assert_eq!(parse("https://canary.discord.com/channels/@me/9"), ch(None, Some(9), None));
        assert_eq!(parse("https://discord.com/channels/1/2/notanid"), ch(Some(1), Some(2), None));
        assert_eq!(parse("https://discord.com/app"), None);
        assert_eq!(parse("https://discord.gg/"), None);
        assert_eq!(parse("https://notdiscord.gg/abc"), None);
        assert_eq!(parse("https://evil.com/channels/1/2"), None);
        assert_eq!(parse("not a url"), None);
    }

    #[test]
    fn launch_arguments_follow_the_official_extractor() {
        let a = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(url_from_args(&a(&["phoenix", "discord://-/channels/1/2"])).as_deref(), Some("discord://-/channels/1/2"));
        assert_eq!(url_from_args(&a(&["phoenix", "--url", "--", "discord://-/invite/x"])).as_deref(), Some("discord://-/invite/x"));
        assert_eq!(url_from_args(&a(&["phoenix", "--", "x", "--url"])), None);
        assert_eq!(url_from_args(&a(&["discord://first-is-the-binary"])), None);
        assert_eq!(url_from_args(&a(&["phoenix", "--multi-instance"])), None);
        assert!(multi_instance(&a(&["phoenix", "--multi-instance"])));
    }

    #[test]
    fn windows_command_passes_the_url_like_the_official_host() {
        assert_eq!(windows_command("C:\\P\\discord-phoenix.exe"), "\"C:\\P\\discord-phoenix.exe\" --url -- \"%1\"");
        let a = ["C:\\P\\discord-phoenix.exe".to_owned(), "--url".to_owned(), "--".to_owned(), "discord://-/channels/1/2".to_owned()];
        assert_eq!(url_from_args(&a).as_deref(), Some("discord://-/channels/1/2"));
    }

    #[test]
    fn ipc_messages_round_trip() {
        let m = Message { token: Some("tok".into()), url: Some("discord://-/channels/1/2".into()) };
        assert_eq!(Message::decode(&m.encode()), Some(m));
        let focus_only = Message::default();
        assert_eq!(Message::decode(&focus_only.encode()), Some(focus_only));
        let dirty = Message { token: Some("a\tb".into()), url: Some("discord://x\ny".into()) };
        assert_eq!(Message::decode(&dirty.encode()), Some(Message { token: Some("ab".into()), url: Some("discord://xy".into()) }));
        assert_eq!(Message::decode("garbage"), None);
    }
}

use std::{cell::RefCell, collections::VecDeque, process::Command, sync::Mutex};

use gpui::Context;

use crate::{
    app::PhoenixApp,
    markdown::MentionResolver,
    model::{Id, Message},
};

pub const MAX_INCOMING: usize = 200;
const RECENT: usize = 10;
pub const SEND_TTS_MESSAGES: u64 = 1 << 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Utterance {
    pub text: String,
    pub interrupt: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Gate {
    pub enable_tts: bool,
    pub read_all: usize,
    pub in_view: bool,
    pub own: bool,
    pub tts_flag: bool,
    pub self_deaf: bool,
    pub blocked: bool,
    pub muted: bool,
}

pub fn should_speak(g: Gate) -> bool {
    if g.self_deaf {
        return false;
    }
    let flagged = g.enable_tts && g.tts_flag && g.in_view;
    let read = !g.own && (g.read_all == 0 || (g.read_all == 1 && g.in_view));
    (flagged || read) && !g.blocked && !g.muted
}

thread_local! {
    static RECORDED: RefCell<Option<Vec<Utterance>>> = const { RefCell::new(None) };
}

static RECENT_IDS: Mutex<VecDeque<u64>> = Mutex::new(VecDeque::new());
static OUTPUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn enable_output() {
    OUTPUT.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn output() -> bool {
    OUTPUT.load(std::sync::atomic::Ordering::Relaxed)
}

static CURRENT: Mutex<Option<(Id, Id)>> = Mutex::new(None);

pub fn record() {
    RECORDED.with_borrow_mut(|r| *r = Some(Vec::new()));
}

pub fn recorded() -> Vec<Utterance> {
    RECORDED.with_borrow(|r| r.clone().unwrap_or_default())
}

fn seen(id: Id) -> bool {
    let Ok(mut q) = RECENT_IDS.lock() else { return false };
    if q.contains(&id.0) {
        return true;
    }
    q.push_front(id.0);
    q.truncate(RECENT);
    false
}

pub fn speaking(channel: Id, msg: Id) -> bool {
    CURRENT.lock().is_ok_and(|c| *c == Some((channel, msg)))
}

pub fn current_channel() -> Option<Id> {
    CURRENT.lock().ok().and_then(|c| c.map(|(ch, _)| ch))
}

fn set_current(v: Option<(Id, Id)>) {
    if let Ok(mut c) = CURRENT.lock() {
        *c = v;
    }
}

fn domain(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let rest = if rest.get(..4).is_some_and(|w| w.eq_ignore_ascii_case("www.")) { &rest[4..] } else { rest };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    &rest[..end]
}

fn url_start(s: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = s[from..].find("http") {
        let at = from + i;
        let rest = &s[at + 4..];
        if rest.starts_with("://") || rest.starts_with("s://") {
            return Some(at);
        }
        from = at + 4;
    }
    None
}

fn domains(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = url_start(rest) {
        out.push_str(&rest[..i]);
        let url = &rest[i..];
        let end = url.find(char::is_whitespace).unwrap_or(url.len());
        out.push_str(domain(&url[..end]));
        rest = &url[end..];
    }
    out.push_str(rest);
    out
}

pub fn shorten(text: &str, max: Option<usize>) -> String {
    let mut out = domains(text);
    if let Some(max) = max
        && out.chars().count() > max
    {
        let cut: String = out.chars().take(max).collect();
        out = cut.rfind(' ').map(|i| cut[..i].to_owned()).unwrap_or_default();
    }
    out
}

fn replace_tokens(s: &str, open: &str, close: char, mut f: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(open) {
        out.push_str(&rest[..i]);
        let after = &rest[i + open.len()..];
        match after.find(close).and_then(|j| f(&after[..j]).map(|r| (j, r))) {
            Some((j, r)) => {
                out.push_str(&r);
                rest = &after[j + close.len_utf8()..];
            }
            None => {
                out.push_str(open);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn digits(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn spoilers(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("||") {
        let after = &rest[i + 2..];
        match after.find("||") {
            Some(j) if j > 0 => {
                out.push_str(&rest[..i]);
                out.push_str(&crate::t!("tts.spoiler"));
                rest = &after[j + 2..];
            }
            _ => {
                out.push_str(&rest[..i + 2]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn body(content: &str, r: &dyn MentionResolver) -> String {
    let s = spoilers(content);
    let s = replace_tokens(&s, "<@", '>', |t| {
        let id = digits(t.strip_prefix('!').unwrap_or(t))?;
        Some(r.user(id).unwrap_or_else(|| crate::t!("tts.unknown_user").to_string()))
    });
    let s = replace_tokens(&s, "<@&", '>', |t| {
        let id = digits(t)?;
        Some(r.role(id).unwrap_or_else(|| crate::t!("tts.unknown_role").to_string()))
    });
    let s = replace_tokens(&s, "<#", '>', |t| {
        let id = digits(t)?;
        Some(r.channel(id).unwrap_or_else(|| crate::t!("tts.unknown_channel").to_string()))
    });
    let emoji = |t: &str| {
        let (name, id) = t.split_once(':')?;
        digits(id)?;
        (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then(|| format!("{} {name}", crate::t!("tts.emoji")))
    };
    let s = replace_tokens(&s, "<a:", '>', emoji);
    let s = replace_tokens(&s, "<:", '>', emoji);
    let s = replace_tokens(&s, "</", '>', |t| {
        let (name, id) = t.rsplit_once(':')?;
        digits(id)?;
        (!name.is_empty() && !name.contains(char::is_whitespace)).then(|| format!("/{name}"))
    });
    replace_tokens(&s, "<t:", '>', |t| {
        let (ts, style) = t.split_once(':')?;
        let ts: i64 = digits(ts)?.try_into().ok()?;
        let mut chars = style.chars();
        let style = chars.next().filter(|c| c.is_ascii_alphabetic() && chars.next().is_none())?;
        crate::markdown::format_timestamp(ts, style)
    })
}

pub fn utterance(content: &str, author: &str, reply: Option<&str>, r: &dyn MentionResolver) -> String {
    let body = body(content, r);
    match reply {
        Some(reply) => crate::t!("tts.replied", username = author, reply = reply, body = body).to_string(),
        None => crate::t!("tts.said", username = author, body = body).to_string(),
    }
}

fn language() -> &'static str {
    let l = crate::i18n::locale();
    l.split(['-', '_']).next().unwrap_or(l)
}

fn spd(args: &[&str]) -> Option<std::process::Child> {
    Command::new("spd-say").args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().ok()
}

fn sink_file(u: &Utterance) -> bool {
    let Some(path) = std::env::var_os("PHOENIX_TTS_SINK") else { return false };
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}{}", if u.interrupt { "! " } else { "" }, u.text);
    }
    true
}

pub fn stop() {
    set_current(None);
    if RECORDED.with_borrow_mut(|r| r.as_mut().map(|v| v.push(Utterance { text: String::new(), interrupt: true })).is_some()) {
        return;
    }
    if sink_file(&Utterance { text: String::new(), interrupt: true }) || !output() {
        return;
    }
    if let Some(mut c) = spd(&["-C"]) {
        std::thread::spawn(move || {
            let _ = c.wait();
        });
    }
}

fn speak(text: String, interrupt: bool, current: Option<(Id, Id)>) {
    if text.trim().is_empty() {
        return;
    }
    let u = Utterance { text, interrupt };
    if interrupt {
        set_current(None);
    }
    if current.is_some() {
        set_current(current);
    }
    if RECORDED.with_borrow_mut(|r| r.as_mut().map(|v| v.push(u.clone())).is_some()) {
        return;
    }
    if sink_file(&u) || !output() {
        return;
    }
    let lang = language();
    std::thread::spawn(move || {
        if u.interrupt
            && let Some(mut c) = spd(&["-C"])
        {
            let _ = c.wait();
        }
        if let Some(mut c) = spd(&["-w", "-l", lang, "--", u.text.as_str()]) {
            let _ = c.wait();
        }
        if let Ok(mut c) = CURRENT.lock()
            && *c == current
        {
            *c = None;
        }
    });
}

fn author_name(app: &PhoenixApp, m: &Message, guild: Option<Id>) -> String {
    m.author_nick
        .clone()
        .or_else(|| app.member_nick_in(guild, m.author))
        .or_else(|| app.users.get(&m.author).map(|u| u.display_name.clone()))
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn reply_name(app: &PhoenixApp, m: &Message, guild: Option<Id>) -> Option<String> {
    let (uid, nick, _) = m.reply_preview.as_ref()?;
    nick.clone().or_else(|| app.member_nick_in(guild, *uid)).or_else(|| app.users.get(uid).map(|u| u.display_name.clone())).map(|s| s.to_string())
}

fn in_view(app: &PhoenixApp, channel: Id) -> bool {
    app.selected_channel == Some(channel) || app.chat.thread.as_ref().is_some_and(|t| t.thread.id == channel)
}

pub fn on_message(app: &PhoenixApp, m: &Message) {
    if crate::ui::chat::pending::is_local(m) {
        return;
    }
    let ch = m.channel;
    let Some(guild) = app.channels.get(&ch).or_else(|| app.nav.find_thread(ch)).map(|c| c.guild) else { return };
    let gate = Gate {
        enable_tts: app.prefs.synced.flag("tts"),
        read_all: app.prefs.local.pick("read-all", 2),
        in_view: in_view(app, ch),
        own: m.author == app.me.id,
        tts_flag: m.tts,
        self_deaf: app.deafened,
        blocked: app.nav.is_blocked(m.author) || app.nav.is_ignored(m.author),
        muted: false,
    };
    if !should_speak(gate) {
        return;
    }
    if guild.is_some() && app.backend.notify_prefs().is_some_and(|p| p.is_channel_muted(guild.map(|g| g.0), ch.0)) {
        return;
    }
    if seen(m.id) {
        return;
    }
    let r = app.resolver_with_sounds(&[], ch);
    let text = utterance(&m.content, &author_name(app, m, guild), reply_name(app, m, guild).as_deref(), &r);
    speak(shorten(&text, Some(MAX_INCOMING)), false, Some((ch, m.id)));
}

pub fn speak_message(app: &PhoenixApp, channel: Id, msg: Id) {
    let Some(m) = app.messages.get(&channel).and_then(|e| e.index_of(msg).map(|i| e.msgs[i].clone())) else { return };
    let guild = app.channels.get(&channel).or_else(|| app.nav.find_thread(channel)).and_then(|c| c.guild);
    let r = app.resolver_with_sounds(&[], channel);
    let text = utterance(&m.content, &author_name(app, &m, guild), reply_name(app, &m, guild).as_deref(), &r);
    speak(shorten(&text, None), true, Some((channel, msg)));
}

pub fn toggle_message(app: &mut PhoenixApp, channel: Id, msg: Id, cx: &mut Context<PhoenixApp>) {
    if speaking(channel, msg) {
        stop();
    } else {
        speak_message(app, channel, msg);
    }
    cx.notify();
}

pub fn on_channel_select(channel: Option<Id>) {
    if current_channel().is_some_and(|c| Some(c) != channel) {
        stop();
    }
}

pub fn on_message_delete(channel: Id, msg: Id) {
    if speaking(channel, msg) {
        stop();
    }
}

static DEAF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static VIEW: Mutex<Option<Option<Id>>> = Mutex::new(None);

pub fn sync(app: &PhoenixApp) {
    let deaf = app.deafened;
    if DEAF.swap(deaf, std::sync::atomic::Ordering::Relaxed) != deaf && deaf {
        stop();
    }
    let view = app.selected_channel;
    let changed = VIEW.lock().is_ok_and(|mut v| v.replace(view).is_some_and(|old| old != view));
    if changed {
        on_channel_select(view);
    }
}

pub fn can_use_command(app: &PhoenixApp, channel: Id) -> bool {
    let guild = app.channels.get(&channel).or_else(|| app.nav.find_thread(channel)).and_then(|c| c.guild);
    guild.is_some() && app.prefs.synced.flag("tts") && crate::actions::has(app, guild, SEND_TTS_MESSAGES)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct R;
    impl MentionResolver for R {
        fn user(&self, id: u64) -> Option<String> {
            (id == 1).then(|| "Alice".into())
        }
        fn channel(&self, id: u64) -> Option<String> {
            (id == 5).then(|| "général".into())
        }
        fn role(&self, id: u64) -> Option<String> {
            (id == 9).then(|| "Modo".into())
        }
    }

    fn gate() -> Gate {
        Gate { enable_tts: true, read_all: 2, in_view: true, tts_flag: true, ..Default::default() }
    }

    #[test]
    fn decides_like_the_client() {
        assert!(should_speak(gate()));
        assert!(!should_speak(Gate { in_view: false, ..gate() }));
        assert!(!should_speak(Gate { enable_tts: false, ..gate() }));
        assert!(should_speak(Gate { own: true, ..gate() }));
        assert!(!should_speak(Gate { self_deaf: true, ..gate() }));
        assert!(!should_speak(Gate { blocked: true, ..gate() }));
        assert!(!should_speak(Gate { muted: true, ..gate() }));
        let plain = Gate { tts_flag: false, ..gate() };
        assert!(!should_speak(plain));
        assert!(should_speak(Gate { read_all: 1, ..plain }));
        assert!(!should_speak(Gate { read_all: 1, in_view: false, ..plain }));
        assert!(should_speak(Gate { read_all: 0, in_view: false, ..plain }));
        assert!(!should_speak(Gate { read_all: 0, own: true, ..plain }));
    }

    #[test]
    fn formats_like_the_client() {
        let (user, role, emoji, spoiler) = (crate::t!("tts.unknown_user"), crate::t!("tts.unknown_role"), crate::t!("tts.emoji"), crate::t!("tts.spoiler"));
        assert_eq!(body("hi <@1> and <@!2> in <#5> as <@&9> <@&3>", &R), format!("hi Alice and {user} in général as Modo {role}"));
        assert_eq!(body("look ||secret|| <:pog:123> <a:dance:4> </ping:77>", &R), format!("look {spoiler} {emoji} pog {emoji} dance /ping"));
        assert_eq!(body("<@x> <#> <:bad name:1> <t:12>", &R), "<@x> <#> <:bad name:1> <t:12>");
        assert_eq!(utterance("yo", "Bob", None, &R), crate::t!("tts.said", username = "Bob", body = "yo").to_string());
        assert_eq!(utterance("yo", "Bob", Some("Alice"), &R), crate::t!("tts.replied", username = "Bob", reply = "Alice", body = "yo").to_string());
        assert!(crate::t!("tts.said", username = "Bob", body = "yo").contains("Bob"));
    }

    #[test]
    fn shortens_links_and_long_text() {
        assert_eq!(shorten("see https://www.example.com/a?b=1 now", None), "see example.com now");
        assert_eq!(shorten("(https://a.b/c)\nhttp://x.y", None), "(a.b\nx.y");
        assert_eq!(shorten("httpx https", None), "httpx https");
        assert_eq!(shorten("aaa bbb ccc", Some(9)), "aaa bbb");
        assert_eq!(shorten("abcdefghij", Some(5)), "");
        assert_eq!(shorten("short", Some(200)), "short");
    }

    #[test]
    fn records_instead_of_speaking() {
        record();
        speak("Bob said hi".into(), false, Some((Id(5), Id(6))));
        assert!(speaking(Id(5), Id(6)));
        on_channel_select(Some(Id(7)));
        assert!(!speaking(Id(5), Id(6)));
        speak("   ".into(), true, None);
        assert_eq!(recorded(), vec![Utterance { text: "Bob said hi".into(), interrupt: false }, Utterance { text: String::new(), interrupt: true }]);
        assert!(!seen(Id(424242)));
        assert!(seen(Id(424242)));
    }
}

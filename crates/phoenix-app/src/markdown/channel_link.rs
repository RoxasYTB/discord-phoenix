use std::ops::Range;

use super::{Mention, MentionResolver};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkIcon {
    Channel(&'static str),
    Chevron,
    Guild(String),
    GuildText(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelLink {
    pub guild: Option<u64>,
    pub channel: u64,
    pub message: Option<u64>,
    pub icons: Vec<(Range<usize>, LinkIcon)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkChannel {
    pub name: String,
    pub guild: Option<u64>,
    pub icon: &'static str,
    pub dm: bool,
    pub forum_post: Option<(String, &'static str)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkGuild {
    pub name: String,
    pub icon: Option<String>,
    pub acronym: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UrlMatch {
    pub len: usize,
    pub guild: Option<u64>,
    pub channel: Option<u64>,
    pub message: Option<u64>,
}

const HOSTS: [&str; 7] = [
    "discord.com",
    "discordapp.com",
    "canary.discord.com",
    "canary.discordapp.com",
    "ptb.discord.com",
    "ptb.discordapp.com",
    "staging.discord.co",
];

fn segment(s: &str) -> usize {
    let b = s.as_bytes();
    if b.first().is_some_and(u8::is_ascii_digit) {
        return b.iter().take_while(|c| c.is_ascii_digit()).count();
    }
    b.iter().take_while(|c| c.is_ascii_alphabetic() || **c == b'-').count()
}

fn digits(s: &str) -> usize {
    s.bytes().take_while(u8::is_ascii_digit).count()
}

fn id(s: &str) -> Option<u64> {
    s.parse().ok().filter(|v| *v > 0)
}

fn query_len(s: &str) -> usize {
    let Some(q) = s.strip_prefix('?') else { return 0 };
    let n = q.bytes().take_while(|c| c.is_ascii_alphanumeric() || b"_%=&+-".contains(c)).count();
    if n == 0 { 0 } else { n + 1 }
}

fn after_host(rest: &str) -> Option<usize> {
    let tail = rest.strip_prefix("https://")?;
    let host = HOSTS.iter().filter(|h| tail.starts_with(*h)).max_by_key(|h| h.len())?;
    let path = &tail[host.len()..];
    path.starts_with("/channels/").then_some(8 + host.len() + "/channels/".len())
}

pub fn match_url(rest: &str) -> Option<UrlMatch> {
    let start = after_host(rest)?;
    let mut at = start;
    let guild = if rest[at..].starts_with("@me") {
        at += 3;
        None
    } else {
        let n = digits(&rest[at..]);
        if n == 0 {
            return None;
        }
        at += n;
        Some(id(&rest[start..at])?)
    };
    let mut parts: Vec<&str> = Vec::new();
    for _ in 0..2 {
        let Some(seg) = rest[at..].strip_prefix('/') else { break };
        let n = segment(seg);
        if n == 0 {
            break;
        }
        parts.push(&seg[..n]);
        at += 1 + n;
    }
    if let (Some(g), [_, "threads"]) = (guild, parts.as_slice()) {
        return thread_url(rest, at, g);
    }
    if parts.iter().any(|p| !p.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    at += query_len(&rest[at..]);
    Some(UrlMatch { len: at, guild, channel: parts.first().copied().and_then(id), message: parts.get(1).copied().and_then(id) })
}

fn thread_url(rest: &str, mut at: usize, guild: u64) -> Option<UrlMatch> {
    let mut ids = Vec::new();
    for _ in 0..2 {
        let seg = rest[at..].strip_prefix('/')?;
        let n = digits(seg);
        if n == 0 {
            return None;
        }
        ids.push(id(&seg[..n])?);
        at += 1 + n;
    }
    Some(UrlMatch { len: at, guild: Some(guild), channel: Some(ids[0]), message: Some(ids[1]) })
}

struct Label {
    text: String,
    icons: Vec<(Range<usize>, LinkIcon)>,
}

const NAME_MAX: usize = 32;

fn clip(name: &str) -> String {
    if name.chars().count() <= NAME_MAX {
        return name.to_owned();
    }
    let mut s: String = name.chars().take(NAME_MAX).collect();
    s.push('…');
    s
}

impl Label {
    fn icon(&mut self, slot: &str, icon: LinkIcon) {
        let start = self.text.len();
        self.text.push_str(slot);
        self.icons.push((start..self.text.len(), icon));
    }

    fn channel(&mut self, name: &str, icon: &'static str) {
        self.icon("#", LinkIcon::Channel(icon));
        self.text.push_str(&clip(name));
    }

    fn message(&mut self) {
        self.icon("\u{1F4AC}", LinkIcon::Channel("chat"));
    }

    fn guild(&mut self, g: &LinkGuild) {
        let icon = match &g.icon {
            Some(u) => LinkIcon::Guild(u.clone()),
            None => LinkIcon::GuildText(g.acronym.clone()),
        };
        self.icon("\u{2060}", icon);
        self.text.push_str(&clip(&g.name));
    }

    fn chevron(&mut self) {
        self.icon("\u{203A}", LinkIcon::Chevron);
    }
}

pub struct Resolved {
    pub text: String,
    pub italic: bool,
    pub mention: Mention,
}

pub fn resolve(r: &dyn MentionResolver, m: &UrlMatch) -> Option<Resolved> {
    let channel = m.channel?;
    let mut l = Label { text: String::new(), icons: Vec::new() };
    let cur = r.current_guild();
    let mut guild = m.guild;
    let mut italic = false;
    match r.link_channel(channel) {
        None => {
            if let Some(g) = m.guild.filter(|g| Some(*g) != cur).and_then(|g| r.link_guild(g)) {
                l.guild(&g);
                l.chevron();
            }
            l.channel(&crate::t!("common.unknown_lower"), "hash");
            italic = true;
        }
        Some(c) if c.guild.is_none() => {
            if !c.dm {
                return None;
            }
            guild = None;
            l.channel(&c.name, c.icon);
            l.chevron();
            l.message();
        }
        Some(c) => {
            let g = c.guild.and_then(|g| r.link_guild(g))?;
            guild = c.guild;
            let same = c.guild == cur;
            match (same, m.message.is_some()) {
                (true, true) => match &c.forum_post {
                    Some((parent, icon)) => {
                        l.channel(parent, icon);
                        l.chevron();
                        l.channel(&c.name, c.icon);
                    }
                    None => {
                        l.channel(&c.name, c.icon);
                        l.chevron();
                        l.message();
                    }
                },
                (true, false) => l.channel(&c.name, c.icon),
                (false, true) => {
                    l.guild(&g);
                    l.chevron();
                    if c.forum_post.is_some() {
                        l.channel(&c.name, c.icon);
                    } else {
                        l.message();
                    }
                }
                (false, false) => {
                    l.guild(&g);
                    l.chevron();
                    l.channel(&c.name, c.icon);
                }
            }
        }
    }
    let mention = Mention::Link(Box::new(ChannelLink { guild, channel, message: m.message, icons: l.icons }));
    Some(Resolved { text: l.text, italic, mention })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct R;
    impl MentionResolver for R {
        fn user(&self, _: u64) -> Option<String> {
            None
        }
        fn channel(&self, id: u64) -> Option<String> {
            self.link_channel(id).map(|c| c.name)
        }
        fn current_guild(&self) -> Option<u64> {
            Some(1)
        }
        fn link_channel(&self, id: u64) -> Option<LinkChannel> {
            let ch = |name: &str, guild, icon| LinkChannel { name: name.into(), guild, icon, dm: false, forum_post: None };
            match id {
                10 => Some(ch("general", Some(1), "hash")),
                11 => Some(LinkChannel { forum_post: Some(("ideas".into(), "forum")), ..ch("post", Some(1), "threads") }),
                20 => Some(ch("lounge", Some(2), "speaker")),
                30 => Some(LinkChannel { dm: true, ..ch("wumpus", None, "hash") }),
                _ => None,
            }
        }
        fn link_guild(&self, id: u64) -> Option<LinkGuild> {
            match id {
                1 => Some(LinkGuild { name: "Home".into(), icon: None, acronym: "H".into() }),
                2 => Some(LinkGuild { name: "Other".into(), icon: Some("https://cdn/icon.png".into()), acronym: "O".into() }),
                _ => None,
            }
        }
    }

    fn label(url: &str) -> Option<(String, Vec<LinkIcon>, Option<u64>, Option<u64>)> {
        let m = match_url(url)?;
        let r = resolve(&R, &m)?;
        let Mention::Link(l) = r.mention else { return None };
        Some((r.text, l.icons.into_iter().map(|(_, i)| i).collect(), l.guild, l.message))
    }

    #[test]
    fn urls_follow_the_client_regex() {
        let m = |u: &str| match_url(u).map(|m| (m.len, m.guild, m.channel, m.message));
        let u = "https://discord.com/channels/1/10/99";
        assert_eq!(m(u), Some((u.len(), Some(1), Some(10), Some(99))));
        assert_eq!(m("https://ptb.discordapp.com/channels/@me/30"), Some((42, None, Some(30), None)));
        assert_eq!(m("https://discord.com/channels/1/10/99/extra").map(|x| x.0), Some(u.len()));
        assert_eq!(m("https://discord.com/channels/1/10?summaryId=5").map(|x| x.0), Some(45));
        assert_eq!(m("https://discord.com/channels/1/2/threads/3/4"), Some((44, Some(1), Some(3), Some(4))));
        assert_eq!(m("https://discord.com/channels/1/2/threads/3"), None);
        assert_eq!(m("https://discord.com/channels/1/shop"), None);
        assert_eq!(m("http://discord.com/channels/1/10"), None);
        assert_eq!(m("https://example.com/channels/1/10"), None);
        assert_eq!(m("https://discord.com/channels/1").map(|x| x.2), Some(None));
    }

    #[test]
    fn labels_follow_the_client_layout() {
        let chan = LinkIcon::Channel;
        assert_eq!(label("https://discord.com/channels/1/10/99"), Some(("#general\u{203A}\u{1F4AC}".into(), vec![chan("hash"), LinkIcon::Chevron, chan("chat")], Some(1), Some(99))));
        assert_eq!(label("https://discord.com/channels/1/10"), Some(("#general".into(), vec![chan("hash")], Some(1), None)));
        assert_eq!(label("https://discord.com/channels/1/11/5").map(|l| l.0), Some("#ideas\u{203A}#post".into()));
        let other = label("https://discord.com/channels/2/20/7").unwrap();
        assert_eq!(other.0, "\u{2060}Other\u{203A}\u{1F4AC}");
        assert_eq!(other.1[0], LinkIcon::Guild("https://cdn/icon.png".into()));
        assert_eq!(label("https://discord.com/channels/2/20").map(|l| l.0), Some("\u{2060}Other\u{203A}#lounge".into()));
        assert_eq!(label("https://discord.com/channels/@me/30/4").map(|l| (l.0, l.2)), Some(("#wumpus\u{203A}\u{1F4AC}".into(), None)));
        let unknown = resolve(&R, &match_url("https://discord.com/channels/2/77/8").unwrap()).unwrap();
        assert!(unknown.italic);
        assert!(unknown.text.starts_with("\u{2060}Other\u{203A}#"));
        assert!(label("https://discord.com/channels/1").is_none());
    }
}

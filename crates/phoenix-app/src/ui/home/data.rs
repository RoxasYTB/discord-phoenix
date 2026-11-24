//! Navigation data that lives beside the core model: relationships, guild
//! folders, threads, per-channel decorations, invites. Delivered by the
//! backend as [`NavEvent`]s inside `AppEvent::Nav`, so the shared model and
//! `ReadyData` stay untouched.

use std::collections::{HashMap, HashSet};

use gpui::SharedString;

use crate::model::{Channel, Id, ImageRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipKind {
    Friend,
    Blocked,
    Incoming,
    Outgoing,
}

#[derive(Clone, Debug)]
pub struct Relationship {
    pub user: Id,
    pub kind: RelationshipKind,
    pub nickname: Option<SharedString>,
    /// Unix seconds the relationship started (`since`).
    pub since: Option<i64>,
}

/// One entry of `guild_folders`. `id == None` means a lone guild that is not
/// really a folder (Discord stores every top-level guild that way).
#[derive(Clone, Debug, Default)]
pub struct GuildFolder {
    pub id: Option<u64>,
    pub name: Option<SharedString>,
    pub color: Option<u32>,
    pub guilds: Vec<Id>,
}

/// Decorations the core `Channel` struct doesn't carry.
#[derive(Clone, Debug, Default)]
pub struct ChannelExtra {
    pub nsfw: bool,
    /// Private channel: @everyone can't see it (lock badge on the icon).
    pub locked: bool,
    /// Group DM only: custom icon and recipient count.
    pub group_icon: Option<ImageRef>,
    pub recipients: usize,
    /// Group DM name built from recipients when Discord sent none.
    pub group_name: Option<SharedString>,
    /// Group DM only: recipient ids (without us) and the group owner.
    pub members: Vec<Id>,
    pub owner: Option<Id>,
    /// `rate_limit_per_user` seconds (0 = off).
    pub slowmode: u32,
    /// MANAGE_MESSAGES / MANAGE_CHANNELS: slowmode doesn't apply.
    pub bypass_slowmode: bool,
    /// Guild channel where we lack SEND_MESSAGES (SEND_MESSAGES_IN_THREADS in threads).
    pub send_blocked: bool,
}

#[derive(Clone, Debug)]
pub struct InvitePreview {
    pub code: SharedString,
    pub guild: Option<Id>,
    pub guild_name: SharedString,
    pub guild_icon: Option<ImageRef>,
    pub channel_name: Option<SharedString>,
    pub inviter: Option<SharedString>,
    pub online: u32,
    pub members: u32,
}

#[derive(Debug)]
pub enum NavEvent {
    /// Full state after login: replaces everything.
    Snapshot {
        relationships: Vec<Relationship>,
        folders: Vec<GuildFolder>,
        threads: Vec<Channel>,
        extras: Vec<(Id, ChannelExtra)>,
        message_requests: Vec<Id>,
    },
    RelationshipUpsert(Relationship),
    RelationshipRemove(Id),
    /// Friend activity line ("Playing X") for the Active Now column.
    Activity { user: Id, activity: Option<SharedString> },
    ThreadUpsert(Channel),
    ThreadDelete(Id),
    Extras(Vec<(Id, ChannelExtra)>),
    /// Guild banner hashes (`None` = no banner).
    Banners(Vec<(Id, Option<String>)>),
    /// Guild folders / rail order changed from another client.
    Folders(Vec<GuildFolder>),
}

#[derive(Clone, Debug)]
pub enum RelAction {
    Accept,
    /// Ignore an incoming request, cancel an outgoing one, unfriend, unblock.
    Remove,
    Block,
    /// Send a friend request by username.
    AddByName(String),
}

#[derive(Clone, Debug)]
pub enum NavRequest {
    FetchInvite(String),
    AcceptInvite(String),
    /// Create a 7 day invite for a channel; reply is `Text(url)`.
    CreateInvite(Id),
    LeaveGuild(Id),
    Relationship { user: Id, action: RelAction },
    /// Open (or create) the DM with a user; reply is `Channel`.
    OpenDm(Id),
    CloseDm(Id),
    /// Mark a channel read up to its latest message.
    AckChannel(Id),
    /// Mark every channel of a guild read.
    AckGuild(Id),
    /// Mute a guild (`channel: None`) or a channel; `secs` None = until unmuted,
    /// `muted: false` unmutes. DMs use `guild: None`.
    Mute { guild: Option<Id>, channel: Option<Id>, muted: bool, secs: Option<u32> },
    /// Notification level: 0 all, 1 mentions, 2 nothing, 3 inherit (channels).
    NotifyLevel { guild: Option<Id>, channel: Option<Id>, level: u8 },
}

#[derive(Clone, Debug)]
pub enum NavReply {
    Done,
    Invite(InvitePreview),
    Text(SharedString),
    Channel(Channel),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FriendsTab {
    #[default]
    Online,
    All,
    Pending,
    Blocked,
    AddFriend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HomeView {
    #[default]
    Friends,
    MessageRequests,
    /// A DM is open in the chat pane.
    Dm,
    /// Shop page (`/shop`).
    Shop,
}

/// Modal / popout currently shown above the main layout.
#[derive(Clone, Debug, PartialEq)]
pub enum Overlay {
    JoinServer,
    InviteLink { guild: Id, link: Option<SharedString> },
    LeaveGuild(Id),
    Stub(SharedString),
    Topic(Id),
}

#[derive(Default)]
pub struct NavState {
    pub relationships: Vec<Relationship>,
    pub folders: Vec<GuildFolder>,
    pub expanded_folders: HashSet<u64>,
    /// Active threads by parent channel.
    pub threads: HashMap<Id, Vec<Channel>>,
    pub extras: HashMap<Id, ChannelExtra>,
    pub activities: HashMap<Id, SharedString>,
    pub message_requests: Vec<Id>,
    pub home: HomeView,
    pub friends_tab: FriendsTab,
    pub guild_menu: bool,
    pub hide_muted: HashSet<Id>,
    pub overlay: Option<Overlay>,
    pub invite_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub add_friend_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    /// Friends list search field (searchBar_wVFOQW).
    pub friends_search: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub invite_preview: Option<Result<InvitePreview, SharedString>>,
    pub invite_code: Option<String>,
    pub add_friend_result: Option<Result<SharedString, SharedString>>,
    pub busy: bool,
    /// Guild being dragged in the rail (reorder stub).
    pub dragging: Option<Id>,
    pub prefs_loaded: bool,
    pub subs: Vec<gpui::Subscription>,
    pub banners: HashMap<Id, String>,
}

impl NavState {
    pub fn relationship(&self, user: Id) -> Option<&Relationship> {
        self.relationships.iter().find(|r| r.user == user)
    }

    pub fn count(&self, kind: RelationshipKind) -> usize {
        self.relationships.iter().filter(|r| r.kind == kind).count()
    }

    pub fn thread_parent(&self, id: Id) -> Option<Id> {
        self.threads.iter().find(|(_, v)| v.iter().any(|t| t.id == id)).map(|(p, _)| *p)
    }

    pub fn find_thread(&self, id: Id) -> Option<&Channel> {
        self.threads.values().flatten().find(|t| t.id == id)
    }

    pub fn apply(&mut self, ev: NavEvent) {
        match ev {
            NavEvent::Snapshot { relationships, folders, threads, extras, message_requests } => {
                self.relationships = relationships;
                self.folders = folders;
                self.threads.clear();
                for t in threads {
                    self.upsert_thread(t);
                }
                self.extras = extras.into_iter().collect();
                self.message_requests = message_requests;
            }
            NavEvent::RelationshipUpsert(r) => {
                self.relationships.retain(|x| x.user != r.user);
                self.relationships.push(r);
            }
            NavEvent::RelationshipRemove(u) => self.relationships.retain(|x| x.user != u),
            NavEvent::Activity { user, activity } => match activity {
                Some(a) => {
                    self.activities.insert(user, a);
                }
                None => {
                    self.activities.remove(&user);
                }
            },
            NavEvent::ThreadUpsert(t) => self.upsert_thread(t),
            NavEvent::Folders(f) => self.folders = f,
            NavEvent::ThreadDelete(id) => {
                for v in self.threads.values_mut() {
                    v.retain(|t| t.id != id);
                }
            }
            NavEvent::Extras(e) => self.extras.extend(e),
            NavEvent::Banners(b) => {
                for (g, h) in b {
                    match h {
                        Some(h) => {
                            self.banners.insert(g, h);
                        }
                        None => {
                            self.banners.remove(&g);
                        }
                    }
                }
            }
        }
    }

    fn upsert_thread(&mut self, t: Channel) {
        for v in self.threads.values_mut() {
            v.retain(|x| x.id != t.id);
        }
        if let Some(p) = t.parent {
            let v = self.threads.entry(p).or_default();
            v.push(t);
            v.sort_by_key(|x| std::cmp::Reverse(x.id));
        }
    }
}

// ------------------------------------------------------------ persistence

/// Locally persisted navigation prefs (last channel per guild, collapsed
/// categories, open folders). Plain text, one `key value...` per line, so no
/// serde derive is needed in the app crate.
#[derive(Debug, Default, PartialEq)]
pub struct NavPrefs {
    pub last_channel: Vec<(Option<u64>, u64)>,
    pub collapsed: Vec<u64>,
    pub folders: Vec<u64>,
    pub hide_muted: Vec<u64>,
}

impl NavPrefs {
    pub fn path() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
        Some(base.join("discord-phoenix").join("navigation.txt"))
    }

    pub fn parse(s: &str) -> Self {
        let mut p = Self::default();
        for line in s.lines() {
            let mut it = line.split_whitespace();
            let key = it.next().unwrap_or_default();
            let nums: Vec<u64> = it.filter_map(|x| x.parse().ok()).collect();
            match (key, nums.as_slice()) {
                ("last", [g, c]) => p.last_channel.push(((*g != 0).then_some(*g), *c)),
                ("collapsed", ids) => p.collapsed.extend(ids),
                ("folders", ids) => p.folders.extend(ids),
                ("hide_muted", ids) => p.hide_muted.extend(ids),
                _ => {}
            }
        }
        p
    }

    pub fn serialize(&self) -> String {
        let join = |v: &[u64]| v.iter().map(u64::to_string).collect::<Vec<_>>().join(" ");
        let mut out = String::new();
        for (g, c) in &self.last_channel {
            out.push_str(&format!("last {} {c}\n", g.unwrap_or(0)));
        }
        out.push_str(&format!("collapsed {}\n", join(&self.collapsed)));
        out.push_str(&format!("folders {}\n", join(&self.folders)));
        out.push_str(&format!("hide_muted {}\n", join(&self.hide_muted)));
        out
    }

    pub fn load() -> Self {
        Self::path().and_then(|p| std::fs::read_to_string(p).ok()).map(|s| Self::parse(&s)).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(p) = Self::path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, self.serialize());
    }
}

/// Pull a `discord.gg/xyz`, `discord.com/invite/xyz` or bare code out of
/// pasted text.
pub fn invite_code(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    if s.is_empty() {
        return None;
    }
    let s = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
    let code = ["discord.gg/", "discord.com/invite/", "discordapp.com/invite/", "ptb.discord.com/invite/", "canary.discord.com/invite/"]
        .iter()
        .find_map(|p| s.strip_prefix(p))
        .unwrap_or(s);
    let code = code.split(['?', '#']).next().unwrap_or_default();
    (!code.is_empty() && code.len() <= 32 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .then(|| code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_codes() {
        assert_eq!(invite_code("https://discord.gg/rust").as_deref(), Some("rust"));
        assert_eq!(invite_code("discord.com/invite/abc-DEF?event=1").as_deref(), Some("abc-DEF"));
        assert_eq!(invite_code("  xyz123 ").as_deref(), Some("xyz123"));
        assert_eq!(invite_code("hello world"), None);
        assert_eq!(invite_code(""), None);
    }

    #[test]
    fn prefs_roundtrip() {
        let p = NavPrefs {
            last_channel: vec![(None, 5), (Some(7), 9)],
            collapsed: vec![1, 2],
            folders: vec![3],
            hide_muted: vec![],
        };
        assert_eq!(NavPrefs::parse(&p.serialize()), p);
    }

    #[test]
    fn threads_group_by_parent() {
        let mut s = NavState::default();
        let t = Channel {
            id: Id(10),
            guild: Some(Id(1)),
            parent: Some(Id(2)),
            name: "t".into(),
            topic: None,
            kind: crate::model::ChannelKind::Text,
            position: 0,
            unread: false,
            muted: false,
            mentions: 0,
            voice_users: vec![],
        };
        s.apply(NavEvent::ThreadUpsert(t));
        assert_eq!(s.thread_parent(Id(10)), Some(Id(2)));
        s.apply(NavEvent::ThreadDelete(Id(10)));
        assert!(s.threads[&Id(2)].is_empty());
    }
}

use std::{cell::Cell, collections::HashMap};

use gpui::{Entity, Pixels, Point, SharedString};
use gpui_component::input::InputState;
use phoenix_api::gateway::MemberListItem;
use serde_json::Value;

use crate::model::{Id, Member, Presence};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityKind {
    Playing,
    Streaming,
    Listening,
    Watching,
    Custom,
    Competing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    pub kind: ActivityKind,
    pub name: SharedString,
    pub details: Option<SharedString>,
    pub state: Option<SharedString>,
    pub emoji: Option<SharedString>,
    pub url: Option<SharedString>,
    pub display_type: Option<u8>,
    pub image: Option<SharedString>,
    pub start: Option<i64>,
    pub application_id: Option<SharedString>,
}

impl Activity {
    pub fn is_spotify(&self) -> bool {
        self.kind == ActivityKind::Listening && self.name.as_ref() == "Spotify"
    }

    pub fn headline(&self) -> SharedString {
        let n = &self.name;
        match self.kind {
            ActivityKind::Custom => {
                let mut s = String::new();
                if let Some(e) = &self.emoji {
                    s.push_str(e);
                    if self.state.is_some() {
                        s.push(' ');
                    }
                }
                if let Some(t) = &self.state {
                    s.push_str(t);
                }
                s.into()
            }
            ActivityKind::Playing => crate::t!("activity.playing_n", n = n),
            ActivityKind::Streaming => crate::t!("activity.streaming_n", n = self.details.as_ref().unwrap_or(n)),
            ActivityKind::Listening => crate::t!("activity.listening_n", n = n),
            ActivityKind::Watching => crate::t!("activity.watching_n", n = n),
            ActivityKind::Competing => crate::t!("activity.competing_n", n = n),
        }
    }
}

fn s(v: &Value, k: &str) -> Option<SharedString> {
    v.get(k).and_then(Value::as_str).filter(|x| !x.trim().is_empty()).map(|x| SharedString::from(x.to_owned()))
}

fn asset_url(a: &Value) -> Option<SharedString> {
    let key = a.get("assets")?.get("large_image")?.as_str()?;
    if let Some(id) = key.strip_prefix("spotify:") {
        return Some(format!("https://i.scdn.co/image/{id}").into());
    }
    if let Some(p) = key.strip_prefix("mp:") {
        return Some(format!("https://media.discordapp.net/{p}").into());
    }
    let app = a.get("application_id").and_then(|v| v.as_str().map(str::to_owned).or_else(|| v.as_u64().map(|n| n.to_string())))?;
    Some(format!("https://cdn.discordapp.com/app-assets/{app}/{key}.png").into())
}

pub fn parse_activity(a: &Value) -> Option<Activity> {
    if a.get("type").and_then(Value::as_u64) == Some(6) {
        return None;
    }
    let kind = match a.get("type").and_then(Value::as_u64).unwrap_or(0) {
        1 => ActivityKind::Streaming,
        2 => ActivityKind::Listening,
        3 => ActivityKind::Watching,
        4 => ActivityKind::Custom,
        5 => ActivityKind::Competing,
        _ => ActivityKind::Playing,
    };
    let emoji = a.get("emoji").and_then(|e| match e.get("id").and_then(|i| i.as_str().map(str::to_owned).or_else(|| i.as_u64().map(|n| n.to_string()))) {
        Some(id) => {
            let anim = if e.get("animated").and_then(Value::as_bool).unwrap_or(false) { "a" } else { "" };
            let name = e.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()).unwrap_or("emoji");
            Some(format!("<{anim}:{name}:{id}>").into())
        }
        None => s(e, "name"),
    });
    let act = Activity {
        kind,
        name: s(a, "name").unwrap_or_default(),
        details: s(a, "details"),
        state: s(a, "state"),
        emoji,
        url: s(a, "url"),
        display_type: a.get("status_display_type").and_then(Value::as_u64).map(|n| n as u8),
        image: asset_url(a),
        start: a.get("timestamps").and_then(|t| t.get("start")).and_then(Value::as_i64),
        application_id: a.get("application_id").and_then(|v| v.as_str().map(str::to_owned).or_else(|| v.as_u64().map(|n| n.to_string()))).map(Into::into),
    };
    let empty = match kind {
        ActivityKind::Custom => act.state.is_none() && act.emoji.is_none(),
        _ => act.name.is_empty(),
    };
    (!empty).then_some(act)
}

pub fn parse_activities(list: &[Value]) -> Vec<Activity> {
    list.iter().filter_map(parse_activity).collect()
}

pub fn summary(acts: &[Activity]) -> Option<SharedString> {
    acts.iter()
        .find(|a| a.kind == ActivityKind::Custom)
        .or_else(|| acts.first())
        .map(Activity::headline)
        .filter(|t| !t.trim().is_empty())
}

pub fn status_str(p: Presence) -> &'static str {
    match p {
        Presence::Online => "online",
        Presence::Idle => "idle",
        Presence::Dnd => "dnd",
        Presence::Offline => "invisible",
    }
}

pub fn status_label(p: Presence) -> SharedString {
    match p {
        Presence::Online => crate::t!("status.online"),
        Presence::Idle => crate::t!("status.idle"),
        Presence::Dnd => crate::t!("status.dnd"),
        Presence::Offline => crate::t!("status.invisible"),
    }
}

#[derive(Clone, Debug)]
pub enum ListEntry {
    Group { id: SharedString, count: u32 },
    Member(Member, Option<SharedString>),
    Loading,
}

pub fn list_rows<'a>(items: impl Iterator<Item = &'a Option<MemberListItem>>) -> (Vec<ListEntry>, Vec<(Id, Vec<Activity>)>) {
    let (mut rows, mut acts) = (Vec::new(), Vec::new());
    for it in items {
        match it {
            None => rows.push(ListEntry::Loading),
            Some(MemberListItem::Group { id, count }) => {
                rows.push(ListEntry::Group { id: id.clone().into(), count: *count })
            }
            Some(MemberListItem::Member(m)) => {
                let Some(u) = &m.member.user else {
                    rows.push(ListEntry::Loading);
                    continue;
                };
                let p = m.presence.as_ref();
                let a = p.and_then(|p| p.get("activities")).and_then(Value::as_array).map(|a| parse_activities(a)).unwrap_or_default();
                let status = p.and_then(|p| p.get("status")).and_then(Value::as_str).unwrap_or("offline");
                let uid = Id(u.id.0);
                rows.push(ListEntry::Member(Member {
                    user: uid,
                    roles: m.member.roles.iter().map(|r| Id(r.0)).collect(),
                    presence: crate::discord_backend::convert::presence(status),
                    activity: summary(&a),
                    nick: None,
                    avatar: None,
                    decoration: None,
                    nameplate: crate::discord_backend::convert::member_nameplate(&m.member),
                    joined_at: m.member.joined_at.as_deref().and_then(iso_to_unix),
                    rejoined: m.member.flags & 1 != 0,
                    premium_since: m.member.premium_since.as_deref().and_then(iso_to_unix),
                }, m.member.nick.clone().filter(|n| !n.is_empty()).map(Into::into)));
                acts.push((uid, a));
            }
        }
    }
    (rows, acts)
}

pub fn ranges_for(first_visible: usize) -> Vec<[u32; 2]> {
    let chunk = (first_visible / 100) as u32;
    let mut v = vec![[0, 99]];
    for c in [chunk, chunk + 1] {
        if c > 0 && !v.contains(&[c * 100, c * 100 + 99]) {
            v.push([c * 100, c * 100 + 99]);
        }
    }
    v
}

#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub user: Id,
    pub banner: Option<SharedString>,
    pub accent: Option<u32>,
    pub decoration: Option<SharedString>,
    pub pronouns: SharedString,
    pub bio: SharedString,
    pub roles: Vec<Id>,
    pub nick: Option<SharedString>,
    pub created: i64,
    pub joined: Option<i64>,
    pub note: SharedString,
    pub mutual_guilds: usize,
    pub mutual_friends_count: usize,
    pub mutual_friends: Vec<crate::model::User>,
    pub mutual_guild_ids: Vec<Id>,
    pub badges: Vec<(SharedString, SharedString)>,
    pub theme: Option<(u32, u32)>,
    pub effect_sku: Option<u64>,
    pub frame_sku: Option<u64>,
    pub look_skus: Vec<u64>,
}

pub fn snowflake_time(id: u64) -> i64 {
    (((id >> 22) + 1_420_070_400_000) / 1000) as i64
}

pub(crate) fn iso_to_unix(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp())
}

pub fn profile_from_api(p: &phoenix_api::rest::profile::UserProfile, note: String) -> Profile {
    let u = &p.user;
    if std::env::var_os("PHX_DEBUG_PROFILE").is_some() {
        eprintln!(
            "profile {}: badges={} widgets={} extra_keys={:?}",
            u.id,
            Value::Array(p.badges.clone()),
            Value::Array(p.widgets.clone()),
            p.extra.keys().collect::<Vec<_>>()
        );
    }
    let ext = |h: &str| if h.starts_with("a_") { "gif" } else { "png" };
    let guild_banner = p.guild_member_profile.as_ref().and_then(|g| Some((g.guild_id?, g.banner.clone()?))).map(|(g, h)| {
        SharedString::from(format!("https://cdn.discordapp.com/guilds/{}/users/{}/banners/{h}.{}?size=600", g.0, u.id, ext(&h)))
    });
    let banner = guild_banner.or_else(|| {
        p.user_profile
            .as_ref()
            .and_then(|g| g.banner.clone())
            .or_else(|| u.banner.clone())
            .map(|h| SharedString::from(format!("https://cdn.discordapp.com/banners/{}/{h}.{}?size=600", u.id, ext(&h))))
    });
    let decoration = u
        .avatar_decoration_data
        .as_ref()
        .and_then(crate::ui::avatar_deco::url);
    let accent = p
        .user_profile
        .as_ref()
        .and_then(|x| x.theme_colors.as_ref().and_then(|t| t.first().copied()).or(x.accent_color))
        .or(u.accent_color);
    let gm = p.guild_member.as_ref();
    let fx = [p.guild_member_profile.as_ref(), p.user_profile.as_ref()]
        .into_iter()
        .flatten()
        .map(|m| crate::ui::settings::collectibles::applied_skus(&m.collectibles))
        .find(|f| f.0.is_some() || f.1.is_some())
        .unwrap_or_default();
    Profile {
        user: Id(u.id.0),
        banner,
        accent,
        decoration,
        pronouns: p.pronouns().to_owned().into(),
        bio: p.bio().to_owned().into(),
        roles: gm.map(|m| m.roles.iter().map(|r| Id(r.0)).collect()).unwrap_or_default(),
        nick: gm.and_then(|m| m.nick.clone()).map(Into::into),
        created: snowflake_time(u.id.0),
        joined: gm.and_then(|m| m.joined_at.as_deref()).and_then(iso_to_unix),
        note: note.into(),
        mutual_guilds: p.mutual_guilds.len(),
        mutual_friends_count: p.mutual_friends_count.map(|n| n as usize).unwrap_or(p.mutual_friends.len()),
        mutual_friends: p.mutual_friends.iter().map(crate::discord_backend::convert::user).collect(),
        mutual_guild_ids: p.mutual_guilds.iter().map(|g| Id(g.id.0)).collect(),
        badges: p
            .badges
            .iter()
            .filter_map(|b| {
                let icon = b.get("icon")?.as_str()?;
                let desc = b.get("description").and_then(Value::as_str).unwrap_or_default();
                Some((desc.to_owned().into(), format!("https://cdn.discordapp.com/badge-icons/{icon}.png").into()))
            })
            .collect(),
        theme: [p.guild_member_profile.as_ref(), p.user_profile.as_ref()]
            .into_iter()
            .flatten()
            .find_map(|x| x.theme_colors.as_ref())
            .and_then(|t| Some((*t.first()?, *t.get(1)?))),
        effect_sku: fx.0,
        frame_sku: fx.1,
        look_skus: look_skus(p, fx),
    }
}

fn sku_of(v: Option<&Value>) -> Option<u64> {
    v?.get("sku_id")?.as_str()?.parse().ok()
}

pub fn look_skus(p: &phoenix_api::rest::profile::UserProfile, fx: (Option<u64>, Option<u64>)) -> Vec<u64> {
    let gm = p.guild_member.as_ref();
    let deco = sku_of(gm.and_then(|m| m.avatar_decoration_data.as_ref())).or_else(|| sku_of(p.user.avatar_decoration_data.as_ref()));
    let plate = sku_of(gm.and_then(|m| m.collectibles.as_ref()).and_then(|c| c.get("nameplate")))
        .or_else(|| sku_of(p.user.collectibles.as_deref().and_then(|c| c.get("nameplate"))));
    [deco, plate, fx.0, fx.1].into_iter().flatten().collect()
}

pub fn format_date(ts: i64) -> SharedString {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| crate::i18n::medium_date(d.date_naive())).unwrap_or_default().into()
}

pub struct Popout {
    pub user: Id,
    pub guild: Option<Id>,
    pub at: Point<Pixels>,
    pub note: Entity<InputState>,
    pub _sub: gpui::Subscription,
}

pub struct TagPopout {
    pub guild_id: Id,
    pub tag: SharedString,
    pub badge: Option<SharedString>,
    pub at: Point<Pixels>,
    pub profile: Option<crate::commands::guide::GuildProfile>,
    pub loading: bool,
    pub private: bool,
    pub error: Option<SharedString>,
}

#[derive(Default)]
pub struct People {
    pub lists: HashMap<Id, Vec<ListEntry>>,
    pub activities: HashMap<Id, Vec<Activity>>,
    pub profiles: HashMap<Id, Profile>,
    pub popout: Option<Popout>,
    pub tag_popout: Option<TagPopout>,
    pub modal: Option<Id>,
    pub status_picker: bool,
    pub custom_input: Option<Entity<InputState>>,
    pub custom_sub: Option<gpui::Subscription>,
    pub custom_status: Option<SharedString>,
    pub requested: Cell<(Id, usize)>,
    pub thread_lists: HashMap<Id, Vec<ListEntry>>,
    pub thread_requested: std::cell::RefCell<std::collections::HashSet<Id>>,
    pub feed_requested: std::cell::RefCell<std::collections::HashSet<(Id, Id)>>,
}

impl People {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn on_presence(&mut self, user: Id, p: Presence) {
        for rows in self.lists.values_mut() {
            for r in rows.iter_mut() {
                if let ListEntry::Member(m, _) = r
                    && m.user == user
                {
                    m.presence = p;
                }
            }
        }
    }

    pub fn on_activities(&mut self, user: Id, acts: Vec<Activity>) {
        let sum = summary(&acts);
        for rows in self.lists.values_mut() {
            for r in rows.iter_mut() {
                if let ListEntry::Member(m, _) = r
                    && m.user == user
                {
                    m.activity = sum.clone();
                }
            }
        }
        if acts.is_empty() {
            self.activities.remove(&user);
        } else {
            self.activities.insert(user, acts);
        }
    }

    pub fn any_overlay(&self) -> bool {
        self.popout.is_some() || self.modal.is_some() || self.status_picker
    }

    pub fn close_overlays(&mut self) {
        crate::ui::profile_toolbar::reset();
        self.popout = None;
        self.tag_popout = None;
        self.modal = None;
        self.status_picker = false;
        self.custom_sub = None;
        self.custom_input = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shop_this_look_reads_decoration_nameplate_effect_and_frame() {
        let p: phoenix_api::rest::profile::UserProfile = serde_json::from_value(json!({
            "user": {"id": "5", "username": "a", "avatar_decoration_data": {"asset": "x", "sku_id": "11"},
                "collectibles": {"nameplate": {"asset": "n/", "sku_id": "22"}}},
            "user_profile": {"bio": "", "collectibles": [{"sku_id": "33", "type": 1}, {"sku_id": "44", "type": 3}]}
        }))
        .unwrap();
        let prof = profile_from_api(&p, String::new());
        assert_eq!(prof.look_skus, vec![11, 22, 33, 44]);
        let p: phoenix_api::rest::profile::UserProfile = serde_json::from_value(json!({
            "user": {"id": "5", "username": "a", "avatar_decoration_data": {"asset": "x", "sku_id": "11"}},
            "guild_member": {"roles": [], "avatar_decoration_data": {"asset": "y", "sku_id": "12"}}
        }))
        .unwrap();
        assert_eq!(look_skus(&p, (None, None)), vec![12]);
    }

    #[test]
    fn custom_status_custom_emoji_become_tokens() {
        let a = parse_activities(&[json!({"type": 4, "name": "Custom Status", "emoji": {"name": "dance", "id": "42", "animated": true}})]);
        assert_eq!(a[0].emoji.as_deref(), Some("<a:dance:42>"));
        let a = parse_activities(&[json!({"type": 4, "name": "Custom Status", "state": "x", "emoji": {"name": "blob", "id": "7"}})]);
        assert_eq!(a[0].emoji.as_deref(), Some("<:blob:7>"));
    }

    #[test]
    fn activities() {
        let a = parse_activities(&[
            json!({"type": 4, "name": "Custom Status", "state": "coding", "emoji": {"name": "🦀"}}),
            json!({"type": 0, "name": "Factorio", "timestamps": {"start": 5}}),
            json!({"type": 2, "name": "Spotify", "details": "Song", "state": "Artist", "assets": {"large_image": "spotify:ab12"}}),
            json!({"type": 4, "name": "Custom Status"}),
        ]);
        assert_eq!(a.len(), 3);
        assert_eq!(a[0].headline().as_ref(), "🦀 coding");
        assert_eq!(a[1].headline().as_ref(), "Playing Factorio");
        assert_eq!(a[1].start, Some(5));
        assert!(a[2].is_spotify());
        assert_eq!(a[2].image.as_deref(), Some("https://i.scdn.co/image/ab12"));
        assert_eq!(summary(&a[1..]).as_deref(), Some("Playing Factorio"));
        assert_eq!(summary(&a).as_deref(), Some("🦀 coding"));
        assert_eq!(summary(&[]), None);
    }

    #[test]
    fn list_rows_and_ranges() {
        let items: Vec<Option<MemberListItem>> = serde_json::from_value::<Vec<MemberListItem>>(json!([
            {"group": {"id": "online", "count": 1}},
            {"member": {"user": {"id": "9", "username": "u"}, "roles": ["3"],
                        "presence": {"status": "dnd", "activities": [{"type": 0, "name": "Game"}]}}},
        ]))
        .unwrap()
        .into_iter()
        .map(Some)
        .chain([None])
        .collect();
        let (rows, acts) = list_rows(items.iter());
        assert_eq!(rows.len(), 3);
        assert!(matches!(&rows[0], ListEntry::Group { id, count: 1 } if id.as_ref() == "online"));
        match &rows[1] {
            ListEntry::Member(m, nick) => {
                assert!(nick.is_none());
                assert_eq!(m.user, Id(9));
                assert_eq!(m.presence, Presence::Dnd);
                assert_eq!(m.roles, vec![Id(3)]);
                assert_eq!(m.activity.as_deref(), Some("Playing Game"));
            }
            _ => panic!(),
        }
        assert!(matches!(rows[2], ListEntry::Loading));
        assert_eq!(acts[0].0, Id(9));

        assert_eq!(ranges_for(0), vec![[0, 99], [100, 199]]);
        assert_eq!(ranges_for(250), vec![[0, 99], [200, 299], [300, 399]]);
    }

    #[test]
    fn dates() {
        assert_eq!(snowflake_time(0), 1_420_070_400);
        assert_eq!(format_date(1_420_070_400).as_ref(), "Jan 1, 2015");
    }
}

pub mod access;
pub(crate) mod audit;
mod engagement;
mod expression;
pub mod kit;
mod people;
mod profile;
pub mod roles;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{InputState, TextareaState};
use phoenix_api::rest::guild_manage::AdminCall;
use serde_json::Value;

use super::{
    Modal,
    settings_layout::{self, SideItem},
};
use crate::{
    actions::{admin::sf, with_modal},
    app::PhoenixApp,
    model::Id,
};

pub const SECTIONS: [&str; 22] = [
    "menu.gs.profile",
    "menu.gs.tag",
    "menu.gs.engagement",
    "menu.gs.boost_perks",
    "menu.gs.emoji",
    "menu.gs.stickers",
    "menu.gs.soundboard",
    "menu.gs.members",
    "menu.gs.roles",
    "menu.gs.invites",
    "menu.gs.access",
    "menu.gs.integrations",
    "menu.gs.app_directory",
    "menu.gs.safety",
    "menu.gs.audit_log",
    "menu.gs.bans",
    "menu.gs.community",
    "menu.gs.onboarding",
    "menu.gs.analytics",
    "menu.gs.discovery",
    "menu.gs.templates",
    "menu.gs.delete",
];

const GROUPS: [(Option<&str>, &[&str]); 7] = [
    (None, &["menu.gs.profile", "menu.gs.tag", "menu.gs.engagement", "menu.gs.boost_perks"]),
    (Some("gs.group.expression"), &["menu.gs.emoji", "menu.gs.stickers", "menu.gs.soundboard"]),
    (Some("gs.group.people"), &["menu.gs.members", "menu.gs.roles", "menu.gs.invites", "menu.gs.access"]),
    (Some("gs.group.apps"), &["menu.gs.integrations", "menu.gs.app_directory"]),
    (Some("gs.group.moderation"), &["menu.gs.safety", "menu.gs.audit_log", "menu.gs.bans"]),
    (Some(""), &["menu.gs.community", "menu.gs.onboarding", "menu.gs.analytics", "menu.gs.discovery"]),
    (Some(""), &["menu.gs.templates"]),
];

pub const KICK_MEMBERS: u64 = 1 << 1;
pub const BAN_MEMBERS: u64 = 1 << 2;
pub const VIEW_AUDIT_LOG: u64 = 1 << 7;
pub const MANAGE_GUILD_EXPRESSIONS: u64 = 1 << 30;
pub const CREATE_GUILD_EXPRESSIONS: u64 = 1 << 43;

pub fn section_index(key: &str) -> usize {
    SECTIONS.iter().position(|k| *k == key).unwrap_or(0)
}

pub fn section_icon(key: &str) -> Option<&'static str> {
    Some(match key {
        "menu.gs.profile" => "user",
        "menu.gs.tag" => "tag",
        "menu.gs.engagement" => "friends",
        "menu.gs.boost_perks" => "boost",
        "menu.gs.emoji" => "emoji",
        "menu.gs.stickers" => "sticker",
        "menu.gs.soundboard" => "voice/soundboard",
        "menu.gs.members" => "members",
        "menu.gs.roles" => "server/badge",
        "menu.gs.invites" => "invite",
        "menu.gs.access" => "server/envelope",
        "menu.gs.integrations" => "settings/puzzle",
        "menu.gs.app_directory" => "apps",
        "menu.gs.safety" => "shield-lock",
        "menu.gs.audit_log" => "server/clipboard",
        "menu.gs.bans" => "deny",
        "menu.gs.community" => "server/server",
        "menu.gs.onboarding" => "server/list-numbered",
        "menu.gs.analytics" => "picker/trending",
        "menu.gs.discovery" => "server/grid-horizontal",
        "menu.gs.templates" => "server/server-grid",
        "menu.gs.delete" => "chat/trash",
        _ => return None,
    })
}

pub fn section_label(key: &str) -> SharedString {
    match key {
        "menu.gs.roles" => crate::t!("gs.roles_short"),
        k => crate::t!(k),
    }
}

pub fn visible_for(perms: u64, owner: bool, community: bool, key: &str) -> bool {
    use crate::actions::{ADMINISTRATOR, MANAGE_GUILD, MANAGE_ROLES, MODERATE_MEMBERS};
    if perms & ADMINISTRATOR != 0 && key != "menu.gs.delete" {
        return community || !matches!(key, "menu.gs.onboarding" | "menu.gs.analytics" | "menu.gs.discovery");
    }
    let has = |b: u64| perms & b != 0;
    match key {
        "menu.gs.delete" => owner,
        "menu.gs.emoji" | "menu.gs.stickers" | "menu.gs.soundboard" => has(MANAGE_GUILD_EXPRESSIONS | CREATE_GUILD_EXPRESSIONS),
        "menu.gs.members" => has(MANAGE_GUILD | KICK_MEMBERS | BAN_MEMBERS | MODERATE_MEMBERS | MANAGE_ROLES),
        "menu.gs.roles" => has(MANAGE_ROLES),
        "menu.gs.audit_log" => has(VIEW_AUDIT_LOG),
        "menu.gs.bans" => has(BAN_MEMBERS),
        "menu.gs.onboarding" | "menu.gs.analytics" | "menu.gs.discovery" => community && has(MANAGE_GUILD),
        _ => has(MANAGE_GUILD),
    }
}

pub fn visible(app: &PhoenixApp, guild: Id, key: &str) -> bool {
    let g = app.guilds.iter().find(|g| g.id == guild);
    let owner = g.and_then(|g| g.owner) == Some(app.me.id);
    let community = g.is_some_and(|g| g.community);
    visible_for(app.backend.guild_permissions(guild), owner, community, key)
}

pub fn side_items(app: &PhoenixApp, guild: Id) -> Vec<SideItem> {
    let name = app.guilds.iter().find(|g| g.id == guild).map(|g| g.name.clone()).unwrap_or_default();
    let mut out = Vec::new();
    for (header, keys) in GROUPS {
        let shown: Vec<&str> = keys.iter().copied().filter(|k| visible(app, guild, k)).collect();
        if shown.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(SideItem::Separator);
        }
        match header {
            None => out.push(SideItem::Header(name.clone())),
            Some("") => {}
            Some(h) => out.push(SideItem::Header(crate::t!(h))),
        }
        for k in shown {
            out.push(SideItem::Item {
                label: section_label(k),
                index: section_index(k),
                danger: false,
                icon: section_icon(k),
                external: matches!(k, "menu.gs.members" | "menu.gs.app_directory"),
            });
        }
    }
    if visible(app, guild, "menu.gs.delete") {
        out.push(SideItem::Separator);
        out.push(SideItem::Item { label: crate::t!("menu.gs.delete"), index: section_index("menu.gs.delete"), danger: true, icon: section_icon("menu.gs.delete"), external: false });
    }
    out
}

#[derive(Default)]
pub struct Edits {
    pub flags: Option<u64>,
    pub notify: Option<u8>,
    pub afk_channel: Option<Option<u64>>,
    pub afk_timeout: Option<u32>,
    pub system_channel: Option<Option<u64>>,
    pub widget_enabled: Option<bool>,
    pub widget_channel: Option<Option<u64>>,
    pub banner_preset: Option<usize>,
}

pub struct State {
    pub guild: Id,
    pub section: usize,
    pub data: HashMap<&'static str, Value>,
    pub pending: HashSet<&'static str>,
    pub inputs: HashMap<&'static str, Entity<InputState>>,
    pub areas: HashMap<&'static str, Entity<TextareaState>>,
    pub open_select: Option<&'static str>,
    pub expanded: HashSet<String>,
    pub edits: Edits,
    pub role: Option<roles::Editor>,
    pub drag: Option<roles::Drag>,
    pub role_order: Option<Vec<u64>>,
    pub member_sort: people::Sort,
    pub audit_user: Option<u64>,
    pub audit_action: Option<u32>,
    pub menu: Option<String>,
    pub uploads: HashMap<&'static str, std::path::PathBuf>,
    pub ban_page: usize,
    pub access: access::Pending,
    pub subs: Vec<gpui::Subscription>,
}

impl State {
    pub fn new(guild: Id, section: usize) -> Self {
        Self {
            guild,
            section: section.min(SECTIONS.len() - 1),
            data: HashMap::new(),
            pending: HashSet::new(),
            inputs: HashMap::new(),
            areas: HashMap::new(),
            open_select: None,
            expanded: HashSet::new(),
            edits: Edits::default(),
            role: None,
            drag: None,
            role_order: None,
            member_sort: people::Sort::default(),
            audit_user: None,
            audit_action: None,
            menu: None,
            uploads: HashMap::new(),
            ban_page: 0,
            access: access::Pending::default(),
            subs: Vec::new(),
        }
    }

    pub fn key(&self) -> &'static str {
        SECTIONS[self.section]
    }

    pub fn get(&self, k: &str) -> Option<&Value> {
        self.data.get(k)
    }

    pub fn list(&self, k: &str) -> &[Value] {
        self.data.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn guild_json(&self) -> &Value {
        static NULL: Value = Value::Null;
        self.data.get("guild").unwrap_or(&NULL)
    }

    pub fn dirty(&self, cx: &gpui::App) -> bool {
        self.edits.banner_preset.is_some() || self.uploads.contains_key("icon") || self.uploads.contains_key("banner") || !pending_writes(self, cx).is_empty() || access::is_dirty(self)
    }
}

pub fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    with_modal(|m| match m {
        Some(Modal::ServerSettings(s)) => Some(f(s.as_mut())),
        _ => None,
    })
}

pub fn needs(key: &str) -> &'static [&'static str] {
    match key {
        "menu.gs.profile" => &["guild", "profile", "games"],
        "menu.gs.engagement" => &["guild", "widget"],
        "menu.gs.emoji" => &["guild", "emojis"],
        "menu.gs.stickers" => &["guild", "stickers"],
        "menu.gs.soundboard" => &["guild", "sounds"],
        "menu.gs.members" => &["guild", "roles"],
        "menu.gs.roles" => &["guild", "roles", "counts"],
        "menu.gs.invites" => &["invites"],
        "menu.gs.access" => &["guild", "verification", "profile"],
        "menu.gs.bans" => &["bans"],
        "menu.gs.audit_log" => &["audit"],
        _ => &["guild"],
    }
}

pub fn call_for(key: &str, guild: Id, s: &State) -> Option<AdminCall> {
    let g = sf(guild);
    Some(match key {
        "guild" => AdminCall::Guild { guild: g },
        "profile" => AdminCall::GuildProfile { guild: g },
        "games" => {
            let ids: Vec<phoenix_api::models::Snowflake> =
                s.get("profile")?.get("game_application_ids")?.as_array()?.iter().filter_map(|v| v.as_str()?.parse().ok()).map(phoenix_api::models::Snowflake).collect();
            if ids.is_empty() {
                return None;
            }
            AdminCall::Applications { ids }
        }
        "widget" => AdminCall::Widget { guild: g },
        "emojis" => AdminCall::Emojis { guild: g },
        "stickers" => AdminCall::Stickers { guild: g },
        "sounds" => AdminCall::Soundboard { guild: g },
        "roles" => AdminCall::Roles { guild: g },
        "counts" => AdminCall::RoleMemberCounts { guild: g },
        "invites" => AdminCall::GuildInvites { guild: g },
        "verification" => AdminCall::MemberVerification { guild: g },
        "requirements" => AdminCall::DiscoveryRequirements { guild: g },
        "disc_meta" => AdminCall::Server(phoenix_api::rest::guild_server::ServerCall::DiscoveryMetadata { guild: g }),
        "bans" => AdminCall::Bans { guild: g },
        "audit" => AdminCall::AuditLogFiltered {
            guild: g,
            user: s.audit_user.map(phoenix_api::models::Snowflake),
            action: s.audit_action,
            before: None,
            limit: 50,
        },
        _ => return None,
    })
}

pub fn load(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(todo) = with_state(|s| {
        let extra: &[&'static str] = if s.key() == "menu.gs.access" { access::extra_needs(s) } else { &[] };
        let keys: Vec<&'static str> = needs(s.key()).iter().chain(extra).copied().filter(|k| !s.data.contains_key(k) && !s.pending.contains(k)).collect();
        let mut out = Vec::new();
        for k in keys {
            if let Some(c) = call_for(k, s.guild, s) {
                s.pending.insert(k);
                out.push((k, c));
            }
        }
        out
    }) else {
        return;
    };
    for (k, call) in todo {
        a.admin(call, cx, move |a, r, cx| {
            with_state(|s| {
                s.pending.remove(k);
                s.data.insert(k, r.unwrap_or(Value::Null));
            });
            load(a, cx);
            cx.notify();
        });
    }
}

pub fn reload(a: &mut PhoenixApp, key: &'static str, cx: &mut Context<PhoenixApp>) {
    with_state(|s| {
        s.data.remove(key);
    });
    load(a, cx);
}

pub fn opens_app_directory(i: usize) -> bool {
    SECTIONS.get(i) == Some(&"menu.gs.app_directory")
}

pub fn open_app_directory(cx: &mut Context<PhoenixApp>) {
    crate::actions::close_modal();
    crate::ui::discovery::open(crate::ui::discovery::Tab::Apps);
    cx.notify();
}

pub fn preselect_access(sub: &str) {
    with_state(|s| access::preselect(s, sub));
}

pub fn pick(i: usize, a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if opens_app_directory(i) {
        open_app_directory(cx);
        return;
    }
    with_state(|s| {
        s.section = i.min(SECTIONS.len() - 1);
        s.open_select = None;
        s.role = None;
        s.menu = None;
        s.access = access::Pending::default();
    });
    load(a, cx);
}

fn mk(s: &mut State, key: &'static str, ph: SharedString, initial: String, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if s.inputs.contains_key(key) {
        return;
    }
    let i = cx.new(|cx| InputState::new(window, cx).placeholder(ph));
    if !initial.is_empty() {
        i.update(cx, |st, cx| st.set_value(initial, window, cx));
    }
    s.subs.push(cx.observe(&i, |_, _, cx| cx.notify()));
    s.inputs.insert(key, i);
}

pub fn prepare(s: &mut State, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    match s.key() {
        "menu.gs.profile" if s.data.contains_key("guild") => {
            let name = kit::s(s.guild_json(), "name");
            mk(s, "name", SharedString::default(), name, window, cx);
            if !s.areas.contains_key("description") {
                let d = kit::s(s.guild_json(), "description");
                let a = cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 3).placeholder(crate::t!("gs.profile.description_ph")).default_value(d));
                s.areas.insert("description", a);
            }
            let traits = profile::traits(s);
            for (i, key) in profile::TRAIT_KEYS.iter().copied().enumerate() {
                let v = traits.get(i).map(|t| t.1.clone()).unwrap_or_default();
                mk(s, key, crate::t!("gs.profile.trait_ph"), v, window, cx);
            }
        }
        "menu.gs.roles" => mk(s, "role_search", crate::t!("gs.roles.search"), String::new(), window, cx),
        "menu.gs.members" => mk(s, "member_search", crate::t!("gs.members.search"), String::new(), window, cx),
        "menu.gs.bans" => mk(s, "ban_search", crate::t!("gs.bans.search"), String::new(), window, cx),
        "menu.gs.access" => access::prepare(s, window, cx),
        _ => {}
    }
    if let Some(r) = s.role.as_mut()
        && r.name.is_none()
    {
        let name = r.name0.clone();
        let i = cx.new(|cx| InputState::new(window, cx));
        i.update(cx, |st, cx| st.set_value(name, window, cx));
        r.name = Some(i);
        let q = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("gs.role.perm_search")));
        s.subs.push(cx.observe(&q, |_, _, cx| cx.notify()));
        r.perm_search = Some(q);
    }
}

pub fn profile_upload_patch(icon: Option<String>, banner: Option<String>) -> Option<phoenix_api::rest::guild_server::ProfilePatch> {
    (icon.is_some() || banner.is_some()).then(|| phoenix_api::rest::guild_server::ProfilePatch { icon: icon.map(Some), custom_banner: banner.map(Some), ..Default::default() })
}

pub fn pick_upload(key: &'static str, exts: &'static [&'static str], cx: &mut Context<PhoenixApp>) {
    crate::ui::file_pick::pick(exts, cx, move |a, path, cx| match key {
        "emoji" => {
            let guild = with_state(|s| s.guild);
            if let (Some(g), Some(image)) = (guild, crate::ui::file_pick::data_uri(&path)) {
                a.admin_toast(AdminCall::CreateEmoji { guild: sf(g), name: emoji_name_from_file(&path), image }, None, cx);
                with_state(|s| s.data.remove("emojis"));
                load(a, cx);
            }
        }
        _ => {
            with_state(|s| {
                s.uploads.insert(key, path);
            });
        }
    });
}

pub fn emoji_name_from_file(path: &std::path::Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let mut name: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    name.truncate(32);
    while name.len() < 2 {
        name.push('_');
    }
    name
}

fn reset(a: &mut PhoenixApp, w: &mut Window, cx: &mut Context<PhoenixApp>) {
    with_state(|s| {
        s.uploads.clear();
        s.edits = Edits::default();
        s.role_order = None;
        if let Some(r) = s.role.as_mut() {
            r.reset();
        }
        s.areas.clear();
        s.inputs.remove("name");
        if s.key() == "menu.gs.access" {
            access::reset(s);
        }
    });
    let _ = (a, w);
    cx.notify();
}

fn save(a: &mut PhoenixApp, _w: &mut Window, cx: &mut Context<PhoenixApp>) {
    if with_state(|s| s.key() == "menu.gs.access").unwrap_or(false) {
        let writes = with_state(access::submit).flatten();
        for w in writes.into_iter().flatten() {
            a.blocked_write(w, cx);
        }
        cx.notify();
        return;
    }
    let calls = with_state(|s| pending_writes(s, cx)).unwrap_or_default();
    for call in calls {
        a.admin_toast(call, None, cx);
    }
    with_state(|s| {
        s.uploads.clear();
        s.edits = Edits::default();
        s.role_order = None;
        if let Some(r) = s.role.as_mut() {
            r.commit();
        }
        s.data.remove("guild");
        s.data.remove("roles");
        s.data.remove("widget");
    });
    load(a, cx);
}

pub fn pending_writes(s: &State, cx: &gpui::App) -> Vec<AdminCall> {
    let mut out = Vec::new();
    let g = sf(s.guild);
    if let Some(e) = engagement::guild_edit(s, cx) {
        out.push(AdminCall::EditGuild { guild: g, edit: e });
    }
    if s.edits.widget_enabled.is_some() || s.edits.widget_channel.is_some() {
        let w = s.get("widget").cloned().unwrap_or(Value::Null);
        let enabled = s.edits.widget_enabled.unwrap_or_else(|| w.get("enabled").and_then(Value::as_bool).unwrap_or(false));
        let channel = s.edits.widget_channel.unwrap_or_else(|| kit::id_of(&w, "channel_id"));
        out.push(AdminCall::EditWidget { guild: g, enabled, channel: channel.map(phoenix_api::models::Snowflake) });
    }
    if let Some(patch) = profile_upload_patch(s.uploads.get("icon").and_then(|p| crate::ui::file_pick::data_uri(p)), s.uploads.get("banner").and_then(|p| crate::ui::file_pick::data_uri(p))) {
        out.push(AdminCall::Server(phoenix_api::rest::guild_server::ServerCall::SetProfile { guild: g, patch }));
    }
    if let Some(order) = &s.role_order {
        out.push(AdminCall::ReorderRoles { guild: g, order: roles::positions(order) });
    }
    if let Some(r) = &s.role
        && let Some(edit) = r.edit(cx)
    {
        out.push(AdminCall::EditRole { guild: g, role: phoenix_api::models::Snowflake(r.id), edit });
    }
    out
}

fn page(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> (AnyElement, bool) {
    if s.role.is_some() {
        return (roles::editor(app, s, cx), true);
    }
    match s.key() {
        "menu.gs.profile" => (profile::render(app, s, cx), true),
        "menu.gs.engagement" => (engagement::render(app, s, cx), false),
        "menu.gs.emoji" => (expression::emoji(app, s, cx), false),
        "menu.gs.stickers" => (expression::stickers(app, s, cx), false),
        "menu.gs.soundboard" => (expression::soundboard(app, s, cx), false),
        "menu.gs.roles" => (roles::list(app, s, cx), false),
        "menu.gs.members" => (people::members(app, s, cx), false),
        "menu.gs.invites" => (people::invites(app, s, cx), false),
        "menu.gs.access" => (access::render(app, s, cx), false),
        "menu.gs.bans" => (people::bans(app, s, cx), false),
        "menu.gs.audit_log" => (audit::render(app, s, cx), false),
        k => (kit::page_head(section_label(k), None).into_any_element(), false),
    }
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let pick_fn: settings_layout::Pick = Rc::new(pick);
    let side = side_items(app, s.guild);
    let (content, wide) = match crate::ui::server_pages::content(app, s.guild, s.key(), cx) {
        Some(e) => (e, false),
        None => page(app, s, cx),
    };
    let notice = if s.key() == "menu.gs.access" && s.role.is_none() {
        access::bar(s).map(|(text, can_save)| settings_layout::unsaved_bar_with(text, Rc::new(reset), can_save.then(|| Rc::new(save) as settings_layout::Act), cx))
    } else if s.dirty(cx) {
        Some(settings_layout::unsaved_bar(Rc::new(reset), Rc::new(save), cx))
    } else {
        crate::ui::server_pages::notice(app, s.guild, s.key(), cx)
    };
    let shell = if s.role.is_some() {
        settings_layout::overlay_fixed("server-settings", side, s.section, pick_fn, content, notice, cx)
    } else if wide {
        settings_layout::overlay_wide("server-settings", side, s.section, pick_fn, content, notice, cx)
    } else {
        settings_layout::overlay("server-settings", side, s.section, pick_fn, SharedString::default(), content, notice, cx)
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(shell)
        .children(crate::ui::server_pages::modal(app, cx).map(|m| gpui::deferred(m).with_priority(25)))
        .children((s.key() == "menu.gs.access").then(|| access::modal(s, cx)).flatten().map(|m| gpui::deferred(m).with_priority(25)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{ADMINISTRATOR, MANAGE_GUILD, MANAGE_ROLES};

    #[test]
    fn app_directory_entry_leaves_the_settings() {
        assert!(opens_app_directory(section_index("menu.gs.app_directory")));
        assert!(!opens_app_directory(section_index("menu.gs.integrations")));
        assert!(!opens_app_directory(SECTIONS.len()));
    }

    #[test]
    fn uploads_follow_the_client_requests() {
        assert_eq!(emoji_name_from_file(std::path::Path::new("/tmp/My Cat-01.png")), "My_Cat_01");
        assert_eq!(emoji_name_from_file(std::path::Path::new("x.gif")), "x_");
        assert!(profile_upload_patch(None, None).is_none());
        let p = profile_upload_patch(Some("data:image/png;base64,AA".into()), None).unwrap();
        assert_eq!((p.icon, p.custom_banner), (Some(Some("data:image/png;base64,AA".into())), None));
    }

    #[test]
    fn sections() {
        assert_eq!(SECTIONS.len(), 22);
        assert_eq!(section_index("menu.gs.roles"), 8);
        for k in SECTIONS {
            assert!(section_icon(k).is_some(), "{k}");
        }
        let grouped: usize = GROUPS.iter().map(|g| g.1.len()).sum();
        assert_eq!(grouped + 1, SECTIONS.len());
    }

    #[test]
    fn sidebar_follows_permissions() {
        assert!(visible_for(ADMINISTRATOR, false, true, "menu.gs.audit_log"));
        assert!(!visible_for(ADMINISTRATOR, false, true, "menu.gs.delete"));
        assert!(visible_for(0, true, true, "menu.gs.delete"));
        assert!(!visible_for(ADMINISTRATOR, false, false, "menu.gs.onboarding"));
        assert!(visible_for(MANAGE_ROLES, false, true, "menu.gs.roles"));
        assert!(visible_for(MANAGE_ROLES, false, true, "menu.gs.members"));
        assert!(!visible_for(MANAGE_ROLES, false, true, "menu.gs.profile"));
        assert!(visible_for(MANAGE_GUILD, false, true, "menu.gs.profile"));
        assert!(!visible_for(MANAGE_GUILD, false, true, "menu.gs.bans"));
        assert!(visible_for(BAN_MEMBERS, false, true, "menu.gs.bans"));
        assert!(visible_for(CREATE_GUILD_EXPRESSIONS, false, true, "menu.gs.emoji"));
    }

    #[test]
    fn loads_per_section() {
        let s = State::new(Id(1), section_index("menu.gs.audit_log"));
        assert!(matches!(call_for("audit", Id(1), &s), Some(AdminCall::AuditLogFiltered { limit: 50, .. })));
        assert_eq!(needs("menu.gs.roles"), &["guild", "roles", "counts"]);
        for k in SECTIONS {
            for d in needs(k).iter().filter(|d| **d != "games") {
                let c = call_for(d, Id(1), &s).expect(d);
                assert_eq!(phoenix_api::rest::guild_manage::build(&c).method.as_str(), "GET", "{d}");
            }
        }
    }

    #[test]
    fn nothing_to_write_when_clean() {
        let s = State::new(Id(1), 0);
        assert!(s.edits.flags.is_none() && s.role_order.is_none() && s.role.is_none());
    }
}

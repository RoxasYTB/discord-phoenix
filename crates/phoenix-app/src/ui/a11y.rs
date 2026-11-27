use std::cell::{Cell, RefCell};

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, InteractiveElement, Interactivity,
    IntoElement, LayoutId, ParentElement, Pixels, Role, SharedString, Styled, Window, accesskit, div, prelude::*, px,
};

use crate::model::ChannelKind;
use crate::ui::server_bar::MediaState;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static ANNOUNCE: RefCell<Announcements> = RefCell::default();
}

pub fn sync(window: &Window) {
    ACTIVE.with(|a| a.set(window.is_a11y_active()));
}

pub fn on() -> bool {
    ACTIVE.with(Cell::get)
}

pub fn named<E: StatefulInteractiveElement>(el: E, role: Role, label: impl FnOnce() -> SharedString) -> E {
    if on() { el.role(role).aria_label(label()) } else { el }
}

pub fn role<E: StatefulInteractiveElement>(el: E, role: Role) -> E {
    if on() { el.role(role) } else { el }
}

pub fn switch<E: StatefulInteractiveElement>(el: E, label: SharedString, on: bool) -> E {
    if self::on() { el.role(Role::Switch).aria_label(label).aria_toggled(if on { gpui::Toggled::True } else { gpui::Toggled::False }) } else { el }
}

pub fn check<E: StatefulInteractiveElement>(el: E, role: Role, label: SharedString, on: bool) -> E {
    if self::on() { el.role(role).aria_label(label).aria_toggled(if on { gpui::Toggled::True } else { gpui::Toggled::False }) } else { el }
}

pub struct Live<E> {
    inner: E,
    live: accesskit::Live,
}

pub fn live<E: Element>(inner: E, live: accesskit::Live) -> Live<E> {
    Live { inner, live }
}

impl<E: Element> IntoElement for Live<E> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<E: InteractiveElement> InteractiveElement for Live<E> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.inner.interactivity()
    }
}

impl<E: Element> Element for Live<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.inner.id()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.inner.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.inner.prepaint(id, inspector, bounds, layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(id, inspector, bounds, layout, prepaint, window, cx)
    }

    fn a11y_role(&self) -> Option<Role> {
        self.inner.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        self.inner.write_a11y_info(node);
        node.set_live(self.live);
    }

    fn a11y_synthetic_children(&mut self, prepaint: &mut Self::PrepaintState, builder: &mut gpui::A11ySubtreeBuilder) {
        self.inner.a11y_synthetic_children(prepaint, builder);
    }
}

#[derive(Default)]
struct Announcements {
    seq: u64,
    polite: Option<(u64, SharedString)>,
    assertive: Option<(u64, SharedString)>,
}

pub fn announce(text: impl Into<SharedString>, assertive: bool) {
    let text = text.into();
    if text.is_empty() {
        return;
    }
    ANNOUNCE.with(|a| {
        let mut a = a.borrow_mut();
        a.seq += 1;
        let entry = Some((a.seq, text));
        if assertive {
            a.assertive = entry;
        } else {
            a.polite = entry;
        }
    });
}

pub fn announced() -> (Option<SharedString>, Option<SharedString>) {
    ANNOUNCE.with(|a| {
        let a = a.borrow();
        (a.polite.as_ref().map(|p| p.1.clone()), a.assertive.as_ref().map(|p| p.1.clone()))
    })
}

fn region(id: &'static str, entry: Option<(u64, SharedString)>, how: accesskit::Live) -> AnyElement {
    let el = div().id(id).absolute().size(px(0.)).overflow_hidden().role(Role::Log);
    let el = match entry {
        Some((seq, t)) => el.child(div().id(("announcement", seq as usize)).role(Role::Label).aria_label(t)),
        None => el,
    };
    live(el, how).into_any_element()
}

pub fn announcer() -> impl IntoElement {
    let (polite, assertive) = ANNOUNCE.with(|a| {
        let a = a.borrow();
        (a.polite.clone(), a.assertive.clone())
    });
    div()
        .absolute()
        .size(px(0.))
        .child(region("a11y-assertive", assertive, accesskit::Live::Assertive))
        .child(region("a11y-polite", polite, accesskit::Live::Polite))
}

fn plural(one: &'static str, other: &'static str, n: u32) -> &'static str {
    if n == 1 { one } else { other }
}

pub fn guild(name: &str, mentions: u32, unread: bool, media: MediaState) -> SharedString {
    let mut label = if mentions > 0 {
        crate::t!(plural("a11y.guild_mentions_one", "a11y.guild_mentions_other", mentions), n = mentions, name = name).to_string()
    } else if unread {
        crate::t!("a11y.guild_unread", name = name).to_string()
    } else {
        name.to_string()
    };
    let flags = [
        (media.active_event, "a11y.media_event"),
        (media.live_stage, "a11y.media_stage"),
        (media.screenshare, "a11y.media_screenshare"),
        (media.video, "a11y.media_video"),
        (media.audio, "a11y.media_voice"),
        (media.activity, "a11y.media_activity"),
    ];
    let parts: Vec<String> = flags.iter().filter(|(on, _)| *on).map(|(_, k)| crate::t!(*k).to_string()).collect();
    if !parts.is_empty() {
        label.push_str(", ");
        label.push_str(&parts.join(" "));
    }
    label.into()
}

pub fn folder(name: &str, mentions: u32) -> SharedString {
    let label = match mentions {
        0 => crate::t!("a11y.folder", name = name),
        1 => crate::t!("a11y.folder_mentions_one", name = name, n = mentions),
        _ => crate::t!("a11y.folder_mentions_other", name = name, n = mentions),
    };
    label.trim().to_string().into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Announcement,
    Forum,
    Voice,
    Stage,
    Thread,
    Dm,
    GroupDm,
}

impl Kind {
    pub fn of(kind: ChannelKind, dm: bool, group: bool) -> Self {
        match kind {
            _ if dm && group => Self::GroupDm,
            _ if dm => Self::Dm,
            ChannelKind::Voice => Self::Voice,
            ChannelKind::Stage => Self::Stage,
            ChannelKind::Announcement => Self::Announcement,
            ChannelKind::Forum => Self::Forum,
            ChannelKind::Text | ChannelKind::Category => Self::Text,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChannelInfo {
    pub unread: bool,
    pub mentions: u32,
    pub locked: bool,
    pub nsfw: bool,
    pub voice_users: Vec<SharedString>,
    pub members: u32,
    pub user_limit: Option<u32>,
    pub call_secs: Option<i64>,
    pub activities: Vec<SharedString>,
}

fn triple(base: &str, mentions: u32, unread: bool, name: &str) -> String {
    let key = if mentions > 0 {
        format!("a11y.ch_{base}_mentions_{}", if mentions == 1 { "one" } else { "other" })
    } else if unread {
        format!("a11y.ch_{base}_unread")
    } else {
        format!("a11y.ch_{base}")
    };
    crate::i18n::t(&key, &[("name", name), ("n", &mentions.to_string())]).to_string()
}

pub fn channel(kind: Kind, name: &str, info: &ChannelInfo) -> SharedString {
    let (m, u) = (info.mentions, info.unread);
    let head = match kind {
        Kind::Text => triple("text", m, u, name),
        Kind::Announcement => triple("announcement", m, u, name),
        Kind::Forum => triple("forum", m, u, name),
        Kind::Thread => triple("thread", m, u, name),
        Kind::Dm => triple("dm", m, u, name),
        Kind::GroupDm => {
            let n = info.members.max(1);
            let members = crate::t!(plural("a11y.members_one", "a11y.members_other", n), n = n);
            format!("{}, {members}", triple("group", m, u, name))
        }
        Kind::Stage => crate::t!("a11y.ch_stage", name = name).to_string(),
        Kind::Voice => {
            let mut l = vec![crate::t!("a11y.ch_voice", name = name).to_string()];
            if m > 0 {
                l.push(crate::t!(plural("a11y.mentions_one", "a11y.mentions_other", m), n = m).to_string());
            }
            if u {
                l.push(crate::t!("a11y.unread").to_string());
            }
            if !info.voice_users.is_empty() {
                l.push(info.voice_users.iter().take(3).map(|s| &**s).collect::<Vec<&str>>().join(", "));
                let more = info.voice_users.len().saturating_sub(3) as u32;
                if more > 0 {
                    l.push(crate::t!(plural("a11y.voice_overflow_one", "a11y.voice_overflow_other", more), n = more).to_string());
                }
                if let Some(limit) = info.user_limit.filter(|l| *l > 0) {
                    l.push(crate::t!("a11y.voice_limit", n = info.voice_users.len(), limit = limit).to_string());
                }
            }
            if let Some(secs) = info.call_secs {
                l.push(crate::t!("a11y.call_duration", duration = duration(secs)).to_string());
            }
            if !info.activities.is_empty() {
                let names: Vec<&str> = info.activities.iter().map(|s| &**s).collect();
                l.push(crate::t!("a11y.voice_activities", names = names.join(", ")).to_string());
            }
            l.join(", ")
        }
    };
    let mut parts = vec![head];
    if info.nsfw {
        parts.push(crate::t!("a11y.nsfw").to_string());
    }
    if info.locked {
        parts.push(crate::t!("a11y.locked").to_string());
    }
    parts.join(", ").into()
}

pub fn category(name: &str) -> SharedString {
    crate::t!("a11y.category", name = name)
}

pub struct MessageParts<'a> {
    pub author: &'a str,
    pub replied_author: Option<&'a str>,
    pub content: &'a str,
    pub edited: bool,
    pub accessories: &'a [SharedString],
    pub time: &'a str,
}

pub fn message(p: &MessageParts) -> SharedString {
    let head = match p.replied_author {
        Some(r) => crate::t!("a11y.replying", author = p.author, replied = r).to_string(),
        None => p.author.to_string(),
    };
    let mut out = format!("{head} ,");
    let content = p.content.trim();
    if !content.is_empty() || p.edited {
        out.push(' ');
        out.push_str(content);
        if p.edited {
            if !content.is_empty() {
                out.push(' ');
            }
            out.push('(');
            out.push_str(&crate::t!("a11y.edited"));
            out.push(')');
        }
    }
    let acc: Vec<&str> = p.accessories.iter().map(|s| &**s).filter(|s| !s.is_empty()).collect();
    if !acc.is_empty() {
        out.push(' ');
        out.push_str(&acc.join(" "));
    }
    if !p.time.is_empty() {
        out.push_str(" , ");
        out.push_str(p.time);
    }
    out.into()
}

pub fn emoji_name(token: &str) -> String {
    let name = crate::ui::chat::state::reaction_name(token);
    let name = match crate::ui::pickers::emoji::index_of(&name) {
        Some(ix) => crate::ui::pickers::emoji::shortcode(ix).to_string(),
        None => name,
    };
    name.replace('_', " ")
}

pub fn reaction(token: &str, count: u32, me: bool) -> SharedString {
    let name = emoji_name(token);
    let key = match (me, count == 1) {
        (false, true) => "a11y.reaction_one",
        (false, false) => "a11y.reaction_other",
        (true, true) => "a11y.reaction_me_one",
        (true, false) => "a11y.reaction_me_other",
    };
    crate::t!(key, name = name, n = count)
}

pub fn super_reaction(token: &str, count: u32, me: bool) -> SharedString {
    let name = emoji_name(token);
    let key = match (me, count == 1) {
        (false, true) => "a11y.super_reaction_one",
        (false, false) => "a11y.super_reaction_other",
        (true, true) => "a11y.super_reaction_me_one",
        (true, false) => "a11y.super_reaction_me_other",
    };
    crate::t!(key, name = name, n = count)
}

pub fn duration(secs: i64) -> String {
    let s = secs.max(0) as u32;
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(crate::t!(plural("a11y.hours_one", "a11y.hours_other", h), n = h).to_string());
    }
    parts.push(crate::t!(plural("a11y.minutes_one", "a11y.minutes_other", m), n = m).to_string());
    parts.push(crate::t!(plural("a11y.seconds_one", "a11y.seconds_other", sec), n = sec).to_string());
    parts.join(", ")
}

pub fn member(name: &str, status: &str, activity: Option<&str>) -> SharedString {
    match activity.filter(|a| !a.is_empty()) {
        Some(a) => format!("{name}, {status}, {a}").into(),
        None => format!("{name}, {status}").into(),
    }
}

pub fn member_group(name: &str, count: u32) -> SharedString {
    crate::t!(plural("a11y.member_group_one", "a11y.member_group_other", count), name = name, n = count)
}

pub fn attachment(a: &crate::model::Attachment) -> SharedString {
    if a.is_image { crate::t!("a11y.image") } else { a.filename.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_of(element: impl IntoElement) -> accesskit::Node {
        let element = element.into_element();
        let mut node = accesskit::Node::new(element.a11y_role().unwrap_or(Role::Unknown));
        element.write_a11y_info(&mut node);
        node
    }

    #[test]
    fn live_regions_reach_accesskit() {
        let node = node_of(live(div().id("x").role(Role::Log).aria_label("message deleted"), accesskit::Live::Polite));
        assert_eq!(node.role(), Role::Log);
        assert_eq!(node.live(), Some(accesskit::Live::Polite));
        assert_eq!(node.label(), Some("message deleted"));
    }

    #[test]
    fn each_announcement_gets_a_new_node_so_repeats_are_read_again() {
        announce("a", false);
        let first = ANNOUNCE.with(|a| a.borrow().polite.clone().unwrap().0);
        announce("a", false);
        let second = ANNOUNCE.with(|a| a.borrow().polite.clone().unwrap().0);
        assert_ne!(first, second);
        announce("b", true);
        announce("", true);
        assert_eq!(announced(), (Some("a".into()), Some("b".into())));
    }

    #[test]
    fn guild_labels_follow_the_client_order() {
        let none = MediaState::default();
        assert_eq!(&*guild("Aperture", 0, false, none), "Aperture");
        assert_eq!(&*guild("Aperture", 0, true, none), "Unread messages, Aperture");
        assert_eq!(&*guild("Aperture", 1, true, none), "1 mention, Aperture");
        assert_eq!(&*guild("Aperture", 3, false, none), "3 mentions, Aperture");
        let voice = MediaState { audio: true, ..Default::default() };
        assert_eq!(&*guild("Aperture", 0, true, voice), "Unread messages, Aperture, Voice call active");
        let both = MediaState { audio: true, active_event: true, ..Default::default() };
        assert_eq!(&*guild("A", 0, false, both), "A, Scheduled event Voice call active");
        assert_eq!(&*folder("Work", 0), "Work, folder");
        assert_eq!(&*folder("Work", 2), "Work, folder , 2 unread mentions");
    }

    #[test]
    fn channel_labels_match_the_sidebar_links() {
        let plain = ChannelInfo::default();
        assert_eq!(&*channel(Kind::Text, "general", &plain), "general (text channel)");
        let unread = ChannelInfo { unread: true, ..Default::default() };
        assert_eq!(&*channel(Kind::Text, "general", &unread), "unread, general (text channel)");
        let pinged = ChannelInfo { unread: true, mentions: 2, ..Default::default() };
        assert_eq!(&*channel(Kind::Announcement, "news", &pinged), "2 mentions, news (announcement channel)");
        let locked = ChannelInfo { unread: true, locked: true, nsfw: true, ..Default::default() };
        assert_eq!(&*channel(Kind::Text, "x", &locked), "unread, x (text channel), age-restricted, Private Channel (locked)");
        let call = ChannelInfo {
            voice_users: ["a", "b", "c", "d", "e"].iter().map(|s| SharedString::from(*s)).collect(),
            ..Default::default()
        };
        assert_eq!(&*channel(Kind::Voice, "vc", &call), "vc (voice channel), a, b, c, and 2 others");
        let group = ChannelInfo { members: 3, ..Default::default() };
        assert_eq!(&*channel(Kind::GroupDm, "pals", &group), "pals (group message), 3 Members");
        assert_eq!(&*channel(Kind::Dm, "bob", &ChannelInfo { mentions: 1, ..Default::default() }), "1 mention, bob (direct message)");
        assert_eq!(&*category("Info"), "Info (category)");
        assert_eq!(Kind::of(ChannelKind::Text, true, true), Kind::GroupDm);
        assert_eq!(Kind::of(ChannelKind::Voice, false, false), Kind::Voice);
    }

    #[test]
    fn voice_labels_add_the_limit_duration_and_activities() {
        let info = ChannelInfo {
            voice_users: ["a", "b"].iter().map(|s| SharedString::from(*s)).collect(),
            user_limit: Some(5),
            call_secs: Some(34 * 3600 + 6 * 60 + 40),
            activities: vec!["Chess".into(), "Poker".into()],
            ..Default::default()
        };
        assert_eq!(
            &*channel(Kind::Voice, "vc", &info),
            "vc (voice channel), a, b, 2 of 5 users, call duration 34 hours, 6 minutes, 40 seconds, Active activities in this channel: Chess, Poker"
        );
        assert_eq!(duration(61), "1 minute, 1 second");
    }

    #[test]
    fn members_and_super_reactions_are_named() {
        assert_eq!(&*member("Nelly", "Online", Some("Playing Triforce")), "Nelly, Online, Playing Triforce");
        assert_eq!(&*member("Nelly", "Idle", None), "Nelly, Idle");
        assert_eq!(&*member_group("Admins", 12), "Admins, 12 members");
        assert_eq!(&*super_reaction("🔥", 1, false), "fire, 1 super reaction, press to super react");
        assert_eq!(&*super_reaction("🔥", 2, true), "fire, 2 super reactions, press to remove your super reaction");
    }

    #[test]
    fn message_labels_read_author_content_then_time() {
        let none: [SharedString; 0] = [];
        let p = MessageParts { author: "Yohan", replied_author: None, content: "salut", edited: false, accessories: &none, time: "14:03" };
        assert_eq!(&*message(&p), "Yohan , salut , 14:03");
        let acc = [SharedString::from("Image")];
        let p = MessageParts { author: "Yohan", replied_author: Some("Bob"), content: "", edited: false, accessories: &acc, time: "Today at 14:03" };
        assert_eq!(&*message(&p), "Yohan replying to Bob , Image , Today at 14:03");
        let p = MessageParts { author: "A", replied_author: None, content: "fixed", edited: true, accessories: &none, time: "" };
        assert_eq!(&*message(&p), "A , fixed (edited)");
    }

    #[test]
    fn reaction_buttons_name_the_emoji_and_count() {
        assert_eq!(&*reaction("❤️", 10, false), "heart, 10 reactions, press to react");
        assert_eq!(&*reaction("<:uwu_thumb_up:123>", 1, true), "uwu thumb up, 1 reaction, press to remove your reaction");
    }

    #[test]
    fn rows_become_named_nodes() {
        let node = node_of(div().id("g").role(Role::TreeItem).aria_label(guild("A", 0, true, MediaState::default())).aria_selected(true));
        assert_eq!(node.role(), Role::TreeItem);
        assert_eq!(node.label(), Some("Unread messages, A"));
        assert_eq!(node.is_selected(), Some(true));
        let link = node_of(div().id("c").role(Role::Link).aria_label(channel(Kind::Text, "general", &ChannelInfo::default())));
        assert_eq!(link.label(), Some("general (text channel)"));
    }
}

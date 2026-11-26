use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use gpui::{
    AnchoredPositionMode, AnyElement, App, Context, ElementId, Entity, FontWeight,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, anchored, deferred, div,
    PresentTransitionExt, img, point, prelude::*, px, svg,
};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    model::{Guild, Id, ImageRef},
    theme::{Palette, c, generated::idx, m, tok, z},
    ui::{
        blob_mask::masked,
        icon,
        rail_dnd::{self, RailDrop, RailNode},
    },
};

const FOLDER_DEFAULT: u32 = 0x5865F2;

enum RailItem<'a> {
    Guild(&'a Guild),
    Folder { id: u64, name: SharedString, color: u32, guilds: Vec<&'a Guild>, open: bool },
}

fn rail(app: &PhoenixApp) -> Vec<RailItem<'_>> {
    let by_id = |g: Id| app.guilds.iter().find(|x| x.id == g);
    let in_folders: std::collections::HashSet<Id> =
        app.nav.folders.iter().flat_map(|f| f.guilds.iter().copied()).collect();
    let mut out: Vec<RailItem> =
        app.guilds.iter().filter(|g| !in_folders.contains(&g.id)).map(RailItem::Guild).collect();
    for f in &app.nav.folders {
        let guilds: Vec<&Guild> = f.guilds.iter().filter_map(|g| by_id(*g)).collect();
        match f.id {
            Some(id) if !guilds.is_empty() => {
                let name = f.name.clone().unwrap_or_else(|| {
                    guilds.iter().map(|g| g.name.as_ref()).collect::<Vec<_>>().join(", ").into()
                });
                out.push(RailItem::Folder {
                    id,
                    name,
                    color: f.color.filter(|c| *c != 0).unwrap_or(FOLDER_DEFAULT),
                    open: app.nav.expanded_folders.contains(&id),
                    guilds,
                });
            }
            _ => out.extend(guilds.into_iter().map(RailItem::Guild)),
        }
    }
    out
}

pub fn flat_guild_order(app: &PhoenixApp) -> Vec<Id> {
    rail(app)
        .into_iter()
        .flat_map(|item| match item {
            RailItem::Guild(g) => vec![g.id],
            RailItem::Folder { guilds, .. } => guilds.iter().map(|g| g.id).collect(),
        })
        .collect()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    if !cx.has_active_drag() {
        rail_dnd::set_over(None);
    }
    let entity = cx.entity();
    let scroll = RAIL_SCROLL.with(|h| h.clone());
    let mut col = div()
        .id("guild-rail")
        .size_full()
        .overflow_y_scroll()
        .track_scroll(&scroll)
        .pt(z(PAD_TOP))
        .pb(z(pad_bottom()))
        .flex()
        .flex_col()
        .gap(z(gap()))
        .child(home(app, &entity));
    let fav = favorites(app, &entity);
    let fav_n = usize::from(fav.is_some());
    col = col.children(fav);
    let dms = unread_dm_items(app, &entity);
    let base = 2 + fav_n + dms.len();
    col = col.children(dms).child(separator());
    let mut spans: Vec<Span> = Vec::new();
    let is_unread = |g: &Guild| g.unread || g.mentions > 0;
    for (i, item) in rail(app).into_iter().enumerate() {
        match &item {
            RailItem::Guild(g) if is_unread(g) => spans.push(Span { ix: base + i, y: 0., h: ICON, mentions: g.mentions }),
            RailItem::Folder { guilds, open: false, .. } if guilds.iter().any(|g| is_unread(g)) => spans.push(Span {
                ix: base + i,
                y: 0.,
                h: FOLDER,
                mentions: guilds.iter().map(|g| g.mentions).sum(),
            }),
            RailItem::Folder { guilds, open: true, .. } => {
                for (k, g) in guilds.iter().enumerate().filter(|(_, g)| is_unread(g)) {
                    let y = FOLDER + gap() - FOLDER_PAD + k as f32 * (ICON + gap());
                    spans.push(Span { ix: base + i, y, h: ICON, mentions: g.mentions });
                }
            }
            _ => {}
        }
        col = match item {
            RailItem::Guild(g) => col.child(guild(app, &entity, g, None, false)),
            RailItem::Folder { id, name, color, guilds, open } => {
                col.child(folder(app, &entity, id, name, color, &guilds, open))
            }
        };
    }
    let e = entity.clone();
    col = col
        .child(separator())
        .child(
            action(&entity, "add-server", Icon::PlusCircle, crate::t!("rail.add_server"))
                .on_click(move |_, w, cx| e.update(cx, |a, cx| a.open_join_server(w, cx))),
        )
        .child({
            let e = entity.clone();
            action(&entity, "discover", Icon::Compass, crate::t!("rail.discover"))
                .on_click(move |_, _, cx| {
                    e.update(cx, |_, cx| {
                        crate::ui::discovery::open(crate::ui::discovery::Tab::Servers);
                        cx.notify();
                    })
                })
        });
    let (above, below, m_above, m_below) = offscreen_unread(&scroll, &spans);
    div()
        .w(z(m::server_bar_w()))
        .flex_none()
        .h_full()
        .relative()
        .bg(c(Palette::BG_TERTIARY))
        .group(crate::ui::chat::scrollbar::THIN_FADE_GROUP)
        .child(col)
        .children(crate::ui::chat::scrollbar::thin_fade_scrollbar_div(&scroll))
        .children(above.map(|ix| unread_bar(&scroll, ix, true, m_above)))
        .children(below.map(|ix| unread_bar(&scroll, ix, false, m_below)))
}

thread_local! {
    static RAIL_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

#[doc(hidden)]
pub fn scroll_rail_to(y: f32) {
    RAIL_SCROLL.with(|h| h.set_offset(gpui::point(gpui::px(0.), gpui::px(-y.max(0.)))));
}

struct Span {
    ix: usize,
    y: f32,
    h: f32,
    mentions: u32,
}

const RAIL_UNDER_PANELS: f32 = 26.;

fn offscreen_unread(h: &gpui::ScrollHandle, spans: &[Span]) -> (Option<usize>, Option<usize>, u32, u32) {
    let view = h.bounds();
    let (mut above, mut below, mut ma, mut mb) = (None, None, 0, 0);
    if view.size.height <= px(0.) {
        return (above, below, ma, mb);
    }
    for s in spans.iter().filter(|s| s.mentions > 0) {
        let Some(b) = h.bounds_for_item(s.ix) else { continue };
        let top = b.top() + z(s.y);
        if top + z(s.h) <= view.top() {
            above = Some(s.ix);
            ma += s.mentions;
        } else if top >= view.bottom() + z(RAIL_UNDER_PANELS) {
            below.get_or_insert(s.ix);
            mb += s.mentions;
        }
    }
    (above, below, ma, mb)
}

fn unread_bar(h: &gpui::ScrollHandle, ix: usize, top: bool, mentions: u32) -> AnyElement {
    use crate::theme::{generated::idx, tok};
    let h = h.clone();
    div()
        .absolute()
        .left_0()
        .right_0()
        .when(top, |d| d.top_0().p(z(8.)))
        .when(!top, |d| d.bottom_0().pt(z(8.)).px(z(8.)))
        .child(
            div()
                .id(if top { "rail-unread-top" } else { "rail-unread-bottom" })
                .h(z(24.))
                .rounded(z(12.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .map(|d| {
                    if mentions > 0 {
                        d.bg(c(tok(idx::BACKGROUND_FEEDBACK_NOTIFICATION)))
                            .border_1()
                            .border_color(c(tok(idx::BORDER_SUBTLE)))
                            .shadow(crate::ui::inbox::shadow_medium())
                    } else {
                        d.bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST)))
                            .border_1()
                            .border_color(c(tok(idx::BORDER_NORMAL)))
                            .shadow(vec![gpui::BoxShadow {
                                color: c(tok(idx::OPACITY_BLACK_24)),
                                offset: point(px(0.), z(2.)),
                                blur_radius: z(6.),
                                spread_radius: px(0.),
                                inset: false,
                            }])
                    }
                })
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(if mentions > 0 { idx::TEXT_OVERLAY_LIGHT } else { idx::TEXT_STRONG })))
                .on_click(move |_, w, _| {
                    h.scroll_to_item(ix);
                    w.refresh();
                })
                .child(crate::t!("rail.unread_new").to_uppercase()),
        )
        .into_any_element()
}

const ICON: f32 = 40.;
const FOLDER: f32 = 48.;
const ICON_RADIUS: f32 = 12.;
const FOLDER_RADIUS: f32 = 16.;
fn gap() -> f32 {
    crate::theme::space::xs()
}
const PAD_TOP: f32 = 0.;
fn pad_bottom() -> f32 {
    gap() + 16.
}
const FOLDER_PAD: f32 = (FOLDER - ICON) / 2.;
const PREVIEW_PAD: f32 = 4.;
const PREVIEW_GAP: f32 = 2.;
const PILL_MS: u64 = 200;
const EASE_OUT_QUINT: gpui::CubicBezier = gpui::CubicBezier(0.22, 1., 0.36, 1.);

thread_local! {
    static HOVERED: Cell<Option<u64>> = const { Cell::new(None) };
    static PILLS: RefCell<HashMap<u64, (f32, f32, std::time::Instant)>> = RefCell::new(HashMap::new());
}

fn hovered(key: u64) -> bool {
    HOVERED.with(|h| h.get() == Some(key))
}

fn track_hover(el: gpui::Stateful<gpui::Div>, key: u64, entity: &Entity<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let e = entity.clone();
    el.on_hover(move |over, _, cx| {
        let changed = HOVERED.with(|h| {
            let next = if *over { Some(key) } else if h.get() == Some(key) { None } else { h.get() };
            let changed = next != h.get();
            h.set(next);
            changed
        });
        if changed {
            e.update(cx, |_, cx| cx.notify());
        }
    })
}

pub fn right_tooltip(text: SharedString, item_h: f32) -> AnyElement {
    right_tooltip_with(text, None, item_h)
}

fn voice_row(app: &PhoenixApp, users: &[Id]) -> Option<gpui::Div> {
    const MAX: usize = 6;
    if users.is_empty() {
        return None;
    }
    let shown = if users.len() <= MAX { users.len() } else { MAX - 1 };
    let more = users.len() - shown;
    let tok = |i| c(crate::theme::tok(i));
    use crate::theme::generated::idx;
    Some(
        div()
            .mt(z(8.))
            .flex()
            .items_center()
            .child(svg().path(crate::icons::path("speaker")).size(z(20.)).mr(z(8.)).flex_none().text_color(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
            .children(users.iter().take(shown).enumerate().map(|(k, u)| {
                let last = k + 1 == shown && more == 0;
                div().flex_none().size(z(24.)).when(!last, |d| d.mr(z(-6.))).child(crate::ui::avatar(app.users.get(u), 24.))
            }))
            .when(more > 0, |d| {
                d.child(
                    div()
                        .flex_none()
                        .h(z(24.))
                        .min_w(z(24.))
                        .px(z(8.))
                        .rounded(z(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(tok(idx::BACKGROUND_BASE_LOWEST))
                        .text_color(tok(idx::TEXT_DEFAULT))
                        .text_size(z(12.))
                        .line_height(z(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("+{more}")),
                )
            }),
    )
}

fn right_tooltip_with(text: SharedString, extra: Option<gpui::Div>, item_h: f32) -> AnyElement {
    let card = div()
        .max_w(z(196.))
        .px(z(12.))
        .py(z(8.))
        .rounded(z(8.))
        .bg(c(crate::theme::tok(crate::theme::generated::idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(crate::theme::tok(crate::theme::generated::idx::BORDER_SUBTLE)))
        .shadow_lg()
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(Palette::TEXT_NORMAL))
        .child(text)
        .children(extra);
    let mut pointer = div().flex().flex_col().justify_center().items_end().w(z(5.));
    for i in 0..9 {
        let w = 5. - (4. - i as f32).abs();
        pointer = pointer.child(div().w(z(w)).h(z(1.)).bg(c(crate::theme::tok(crate::theme::generated::idx::BACKGROUND_SURFACE_HIGH))));
    }
    let tip = deferred(
        anchored()
            .position_mode(AnchoredPositionMode::Local)
            .position(point(z((m::server_bar_w() + item_h) / 2. + 12. - 5.), z(0.))).child(
            div()
                .h(z(item_h))
                .flex()
                .items_center()
                .child(pointer)
                .child(card.present_transition("rail-tip-zoom", Some(crate::ui::states::tip_transition((0., 0.5)))))
                .present_transition(
                    "rail-tip",
                    Some(gpui::Transition::ease(crate::ui::settings::effects::motion(100), EASE_OUT_QUINT).translate(point(z(4.), z(0.)), point(px(0.), px(0.)))),
                ),
        ),
    )
    .with_priority(2);
    div().absolute().top_0().left_0().child(tip).into_any_element()
}

fn pill(key: u64, selected: bool, hover: bool, unread: bool) -> AnyElement {
    let target = if selected {
        40.
    } else if hover {
        20.
    } else if unread {
        8.
    } else {
        0.
    };
    let (from, to, at) = PILLS.with(|p| {
        let mut p = p.borrow_mut();
        let e = p.entry(key).or_insert((target, target, std::time::Instant::now()));
        if e.1 != target {
            *e = (e.1, target, std::time::Instant::now());
        }
        *e
    });
    let ms = crate::ui::settings::effects::motion(PILL_MS);
    let t = (from != to && at.elapsed() < ms).then(|| gpui::Transition::ease(ms, EASE_OUT_QUINT).start(at).height(z(from), z(to)));
    let bar = div().w(z(4.)).rounded_r(z(4.)).bg(c(Palette::TEXT_HEADER)).h(z(if t.is_some() { to.max(from) } else { to })).present_transition(ElementId::NamedInteger("pill".into(), key), t);
    div().absolute().left_0().top_0().h_full().w(z(4.)).flex().items_center().child(bar).into_any_element()
}

fn count(n: u32) -> Option<u32> {
    (n > 0).then_some(n)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MediaState {
    pub audio: bool,
    pub video: bool,
    pub screenshare: bool,
    pub connected: bool,
    pub live_stage: bool,
    pub active_event: bool,
}

impl MediaState {
    pub fn with_live(mut self, live: &crate::guild_live::Live, muted: bool, here: Option<(u64, bool)>, visible: impl Fn(u64) -> bool) -> Self {
        match here {
            Some((voice, stage)) => {
                self.live_stage = stage;
                self.active_event = live.events.values().any(|c| *c == Some(voice));
            }
            None if muted => {}
            None => {
                self.live_stage = live.stages.values().any(|c| visible(*c));
                self.active_event = live.events.values().any(|c| c.is_none_or(&visible));
            }
        }
        self
    }

    pub fn icon(&self) -> Option<&'static str> {
        if self.active_event {
            Some("calendar")
        } else if self.live_stage {
            Some("stage")
        } else if self.screenshare {
            Some("golive/screen")
        } else if self.video {
            Some("video-call")
        } else if self.audio {
            Some("speaker")
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Presence {
    pub eligible: bool,
    pub channel_muted: bool,
    pub hidden_user: bool,
    pub flags: crate::model::VoiceFlags,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Here {
    pub channel_has_video: bool,
    pub self_stream: bool,
}

pub fn media_state(here: Option<Here>, guild_muted: bool, presences: &[Presence]) -> MediaState {
    if let Some(h) = here {
        return MediaState { audio: true, video: h.channel_has_video, screenshare: h.self_stream, connected: true, ..Default::default() };
    }
    if guild_muted {
        return MediaState::default();
    }
    let seen = |p: &&Presence| !p.hidden_user && p.eligible && !p.channel_muted;
    MediaState {
        audio: presences.iter().any(|p| seen(&p)),
        video: presences.iter().filter(seen).any(|p| p.flags.video),
        screenshare: presences.iter().any(|p| !p.hidden_user && !p.channel_muted && p.flags.stream),
        connected: false,
        ..Default::default()
    }
}

pub fn guild_media(app: &PhoenixApp, g: &Guild) -> MediaState {
    let Some(chans) = app.guild_channels.get(&g.id) else { return MediaState::default() };
    let prefs = app.backend.notify_prefs();
    let flags = crate::ui::voice_panel::flags;
    if let Some(v) = app.voice_channel.filter(|v| chans.contains(v)) {
        let users = app.channels.get(&v).map(|c| c.voice_users.as_slice()).unwrap_or_default();
        let me = app.me.id;
        let here = Here {
            channel_has_video: users.iter().any(|u| flags(*u).video),
            self_stream: crate::ui::go_live::live(app) || flags(me).stream,
        };
        let stage = app.channels.get(&v).is_some_and(|c| c.kind == crate::model::ChannelKind::Stage);
        return media_state(Some(here), false, &[]).with_live(&crate::guild_live::get(g.id.0), false, Some((v.0, stage)), |_| true);
    }
    let guild_muted = prefs.as_ref().is_some_and(|p| p.is_guild_muted(Some(g.id.0)));
    let mut presences = Vec::new();
    for ch in chans.iter().filter_map(|c| app.channels.get(c)) {
        if ch.voice_users.is_empty() {
            continue;
        }
        let eligible = ch.kind != crate::model::ChannelKind::Stage && g.afk_channel != Some(ch.id);
        let channel_muted = prefs.as_ref().is_some_and(|p| p.is_channel_muted(Some(g.id.0), ch.id.0));
        for &u in &ch.voice_users {
            let hidden_user = app.nav.is_blocked(u) || app.nav.is_ignored(u);
            presences.push(Presence { eligible, channel_muted, hidden_user, flags: flags(u) });
        }
    }
    let visible = |c: u64| chans.contains(&crate::model::Id(c));
    media_state(None, guild_muted, &presences).with_live(&crate::guild_live::get(g.id.0), guild_muted, None, visible)
}

pub fn folder_media(app: &PhoenixApp, guilds: &[&Guild]) -> MediaState {
    guilds
        .iter()
        .map(|g| guild_media(app, g))
        .find(|s| s.connected)
        .map(|s| MediaState { audio: !s.video, live_stage: false, active_event: false, ..s })
        .unwrap_or_default()
}

fn voice_badge(state: MediaState) -> Option<AnyElement> {
    use crate::theme::{generated::idx, tok};
    let icon = state.icon()?;
    let bg = tok(if state.connected { idx::STATUS_POSITIVE } else { idx::ICON_OVERLAY_DARK });
    Some(crate::ui::blob_mask::icon_badge(icon, c(bg)).into_any_element())
}

fn row(id: impl Into<ElementId>, key: u64, entity: &Entity<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let row = track_hover(
        div().id(id).w_full().h(z(ICON)).flex_none().flex().justify_center().relative().cursor_pointer(),
        key,
        entity,
    );
    crate::ui::focus_ring::ring_inset(row, crate::ui::states::key(("rail-row", key)), ICON_RADIUS, (m::SERVER_BAR_W - ICON) / 2., 0.)
}

const HOME_KEY: u64 = 1;

pub const RAIL_DM_LIMIT: usize = 20;

pub fn rail_dm_ids(
    dms: &[Id],
    mentions: impl Fn(Id) -> Option<u32>,
    hidden: impl Fn(Id) -> bool,
    favorite: impl Fn(Id) -> bool,
    call_dm: Option<Id>,
) -> Vec<Id> {
    let (mut sorted, rest): (Vec<Id>, Vec<Id>) = dms.iter().copied().partition(|d| favorite(*d));
    sorted.extend(rest);
    let mut out: Vec<Id> = sorted
        .into_iter()
        .filter(|d| !hidden(*d) && mentions(*d).is_some_and(|n| n > 0))
        .take(RAIL_DM_LIMIT)
        .filter(|d| Some(*d) != call_dm)
        .collect();
    if let Some(v) = call_dm.filter(|v| mentions(*v).is_some()) {
        out.insert(0, v);
    }
    out
}

pub fn dm_media_state(here: bool, tracked: bool, self_stream: bool, users: &[crate::model::VoiceFlags]) -> MediaState {
    let in_call = here || !users.is_empty();
    let video_mode = tracked && (self_stream || users.iter().any(|f| f.video || f.stream));
    MediaState {
        audio: in_call && !video_mode,
        video: in_call && video_mode,
        screenshare: self_stream || users.iter().any(|f| f.stream),
        connected: here,
        ..Default::default()
    }
}

pub fn dm_media(app: &PhoenixApp, id: Id) -> MediaState {
    let here = app.voice_channel == Some(id);
    let tracked = here || app.selected_channel == Some(id);
    let users: Vec<crate::model::VoiceFlags> =
        app.parity.call_users.get(&id).into_iter().flatten().map(|u| crate::ui::voice_panel::flags(*u)).collect();
    dm_media_state(here, tracked, here && crate::ui::go_live::live(app), &users)
}

fn unread_dm_items(app: &PhoenixApp, entity: &Entity<PhoenixApp>) -> Vec<AnyElement> {
    let call_dm = app.voice_channel.filter(|v| app.channels.get(v).is_some_and(|c| c.guild.is_none()));
    let requests = |d: Id| app.nav.message_requests.contains(&d) || app.nav.spam_requests.contains(&d);
    let prefs = app.backend.notify_prefs();
    let favorite = |d: Id| prefs.as_ref().is_some_and(|p| p.is_messages_favorite(d.0));
    rail_dm_ids(&app.dms, |d| app.channels.get(&d).map(|c| c.mentions), requests, favorite, call_dm)
        .into_iter()
        .filter_map(|d| app.channels.get(&d))
        .map(|ch| {
            let id = ch.id;
            let key = id.0;
            let hover = hovered(key);
            let u = ch.voice_users.first().and_then(|u| app.users.get(u));
            let name = u.map(|u| u.display_name.clone()).unwrap_or_else(|| ch.name.clone());
            let e = entity.clone();
            row(("rail-dm", id.0), key, entity)
                .on_click(move |_, _, cx| {
                    e.update(cx, |a, cx| {
                        a.select_guild(None, cx);
                        a.select_channel(id, cx);
                    })
                })
                .child(pill(key, app.selected_guild.is_none() && app.selected_channel == Some(id), hover, ch.mentions > 0))
                .child(masked(ICON, false, crate::ui::avatar(u, ICON), voice_badge(dm_media(app, id)), count(ch.mentions)))
                .when(hover, |d| d.child(right_tooltip(name, ICON)))
                .into_any_element()
        })
        .collect()
}
const ADD_KEY: u64 = 2;
const DISCOVER_KEY: u64 = 3;

fn home(app: &PhoenixApp, entity: &Entity<PhoenixApp>) -> AnyElement {
    let selected = app.selected_guild.is_none() && !app.favorites_view && crate::ui::discovery::current().is_none();
    let hover = hovered(HOME_KEY);
    let pending = app.nav.count(crate::ui::home::data::RelationshipKind::Incoming) as u32;
    let e = entity.clone();
    row("home", HOME_KEY, entity)
        .on_click(move |_, _, cx| e.update(cx, |a, cx| a.select_guild(None, cx)))
        .child(pill(HOME_KEY, selected, hover, false))
        .child(masked(
            ICON,
            false,
            div()
                .size(z(ICON))
                .flex()
                .items_center()
                .justify_center()
                .rounded(z(ICON_RADIUS))
                .relative()
                .child(crate::ui::states::fade_to(
                    crate::ui::states::key(("rail-tile", HOME_KEY)),
                    selected || hover,
                    c(Palette::GUILD_ICON_BG),
                    c(Palette::BLURPLE),
                    150,
                    ICON_RADIUS,
                ))
                .child(svg().path(crate::icons::path("discord-logo")).size(z(24.)).flex_none().text_color(if selected || hover { gpui::white() } else { c(Palette::TEXT_NORMAL) })),
            None,
            count(pending),
        ))
        .when(hover, |d| d.child(right_tooltip(crate::t!("dm.title"), ICON)))
        .into_any_element()
}

const FAVORITES_KEY: u64 = 4;

fn favorites(app: &PhoenixApp, entity: &Entity<PhoenixApp>) -> Option<AnyElement> {
    if !crate::ui::favorites::enabled(app) {
        return None;
    }
    let selected = app.favorites_view;
    let hover = hovered(FAVORITES_KEY);
    let (unread, mentions) = crate::ui::favorites::badge(app);
    let e = entity.clone();
    Some(
        row("favorites", FAVORITES_KEY, entity)
            .on_click(move |_, _, cx| e.update(cx, crate::ui::favorites::open))
            .on_mouse_down(gpui::MouseButton::Right, |ev, w, _| crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Favorites, ev.position, w))
            .child(pill(FAVORITES_KEY, selected, hover, unread))
            .child(masked(
                ICON,
                false,
                div()
                    .size(z(ICON))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(z(ICON_RADIUS))
                    .relative()
                    .child(crate::ui::states::fade_to(
                        crate::ui::states::key(("rail-tile", FAVORITES_KEY)),
                        selected || hover,
                        c(Palette::GUILD_ICON_BG),
                        c(Palette::BLURPLE),
                        150,
                        ICON_RADIUS,
                    ))
                    .child(svg().path(crate::icons::path("star")).size(z(20.)).flex_none().text_color(if selected || hover { gpui::white() } else { c(Palette::TEXT_NORMAL) })),
                None,
                count(mentions),
            ))
            .when(hover, |d| d.child(right_tooltip_with(crate::t!("fav.title"), voice_row(app, &crate::ui::favorites::voice_users(app)), ICON)))
            .into_any_element(),
    )
}

fn guild_inner(g: &Guild, size: f32) -> AnyElement {
    match &g.icon {
        ImageRef::Url(u) if crate::media::is_animated(u) => {
            let r = z(ICON_RADIUS * size / ICON);
            let group = SharedString::new_static("phx-guild-icon");
            div()
                .relative()
                .size(z(size))
                .group(group.clone())
                .child(img(crate::media::static_variant(u)).size(z(size)).rounded(r))
                .child(
                    img(u.clone())
                        .id(crate::media::anim_id(u))
                        .absolute()
                        .top_0()
                        .left_0()
                        .size(z(size))
                        .rounded(r)
                        .invisible()
                        .group_hover(group, |s| s.visible()),
                )
                .into_any_element()
        }
        ImageRef::Url(u) => img(u.clone()).size(z(size)).rounded(z(ICON_RADIUS * size / ICON)).into_any_element(),
        _ => div()
            .text_size(z(if size < 30. {
                7.
            } else {
                match g.acronym.chars().count() {
                    0..=2 => 18.,
                    3 | 4 => 16.,
                    5 => 14.,
                    6 => 12.,
                    _ => 10.,
                }
            }))
            .font_weight(FontWeight::MEDIUM)
            .child(g.acronym.clone())
            .into_any_element(),
    }
}

#[derive(Clone)]
pub struct DraggedGuild {
    pub node: RailNode,
    icon: ImageRef,
    acronym: SharedString,
    folder: Option<u32>,
}

impl Render for DraggedGuild {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(color) = self.folder {
            return div().size(px(m::GUILD_ICON)).rounded(z(FOLDER_RADIUS)).bg(c(color).opacity(0.4)).opacity(0.8);
        }
        let inner: AnyElement = match &self.icon {
            ImageRef::Url(u) => img(u.clone()).size_full().into_any_element(),
            _ => div().text_size(z(16.)).child(self.acronym.clone()).into_any_element(),
        };
        div()
            .size(px(m::GUILD_ICON))
            .rounded(z(16.))
            .overflow_hidden()
            .bg(c(Palette::BLURPLE))
            .text_color(gpui::white())
            .flex()
            .items_center()
            .justify_center()
            .opacity(0.8)
            .child(inner)
    }
}

impl PhoenixApp {
    pub fn drop_on_rail(&mut self, src: RailNode, at: Option<RailDrop>, cx: &mut Context<Self>) {
        cx.notify();
        let Some(at) = at else { return };
        let all: Vec<Id> = self.guilds.iter().map(|g| g.id).collect();
        let mut entries = rail_dnd::normalize(&self.nav.folders, &all);
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
        let id = rail_dnd::fresh_folder_id(&entries, seed);
        if !rail_dnd::apply_drop(&mut entries, src, at, id) {
            return;
        }
        let prev = std::mem::replace(&mut self.nav.folders, entries);
        let folders = crate::ui::inbox::folder_tree(self);
        self.action(crate::actions::ActionRequest::SaveFolders(folders), cx, move |a, r, cx| {
            if let Err(e) = r {
                a.nav.folders = prev;
                a.parity_toast(crate::t!("inbox.order_failed", e = e.0).to_string(), cx);
                cx.notify();
            }
        });
    }
}

fn drop_line(top: f32) -> gpui::Div {
    div()
        .absolute()
        .top(z(top))
        .left(z(4.))
        .right(z(4.))
        .h(z(4.))
        .rounded(z(2.))
        .bg(c(tok(idx::STATUS_POSITIVE)))
        .shadow(vec![drop_shadow()])
}

fn drop_shadow() -> gpui::BoxShadow {
    gpui::BoxShadow { color: c(tok(idx::OPACITY_BLACK_28)), offset: point(px(0.), px(0.)), blur_radius: z(3.), spread_radius: px(0.), inset: false }
}

fn drop_marks(node: RailNode, folder: Option<u64>, last: bool, h: f32) -> Option<gpui::Div> {
    match rail_dnd::over()? {
        RailDrop::Before(n) if n == node => Some(drop_line(-6.)),
        RailDrop::Combine(n) if n == node => {
            Some(div().absolute().top(z(12.)).left(z(4.)).size(z(16.)).rounded_full().bg(c(tok(idx::STATUS_POSITIVE))).shadow(vec![drop_shadow()]))
        }
        RailDrop::FolderEnd(f) if last && folder == Some(f) => Some(drop_line(h + 2.)),
        _ => None,
    }
}

fn drop_target(el: gpui::Stateful<gpui::Div>, node: RailNode, folder: Option<u64>, last: bool, entity: &Entity<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let drop_e = entity.clone();
    el.on_drag_move::<DraggedGuild>(move |ev, window, cx| {
        let inside = ev.bounds.contains(&ev.event.position);
        let next = if inside {
            let y = f32::from(ev.event.position.y - ev.bounds.origin.y);
            rail_dnd::zone(ev.drag(cx).node, node, folder, last, y, f32::from(ev.bounds.size.height))
        } else if rail_dnd::over().is_some_and(|d| d.target() == node || (last && matches!(d, RailDrop::FolderEnd(f) if Some(f) == folder))) {
            None
        } else {
            return;
        };
        if rail_dnd::set_over(next) {
            window.refresh();
        }
    })
    .on_drop(move |d: &DraggedGuild, _, cx| {
        let at = rail_dnd::take_over();
        drop_e.update(cx, |a, cx| a.drop_on_rail(d.node, at, cx))
    })
    .children(drop_marks(node, folder, last, ICON))
}

fn guild(app: &PhoenixApp, entity: &Entity<PhoenixApp>, g: &Guild, folder: Option<u64>, last: bool) -> AnyElement {
    let id = g.id;
    let selected = app.selected_guild == Some(id) && !app.favorites_view;
    let hover = hovered(id.0);
    let has_icon = matches!(g.icon, ImageRef::Url(_));
    let active = selected || hover;
    let e = entity.clone();
    let drag = DraggedGuild { node: RailNode::Guild(id), icon: g.icon.clone(), acronym: g.acronym.clone(), folder: None };
    let icon_box = div()
        .size(z(ICON))
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .rounded(z(ICON_RADIUS))
        .when(!has_icon, |d| d.bg(c(if active { Palette::BLURPLE } else { Palette::GUILD_ICON_BG })))
        .text_color(if active && !has_icon { gpui::white() } else { c(Palette::TEXT_NORMAL) })
        .child(guild_inner(g, ICON));
    drop_target(row(("guild", id.0), id.0, entity), RailNode::Guild(id), folder, last, entity)
        .on_click(move |_, _, cx| e.update(cx, |app, cx| app.select_guild(Some(id), cx)))
        .on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| {
            crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Guild(id), ev.position, w)
        })
        .on_drag(drag, |d: &DraggedGuild, _, _, cx: &mut App| cx.new(|_| d.clone()))
        .child(pill(id.0, selected, hover, g.unread))
        .child(masked(ICON, false, icon_box, voice_badge(guild_media(app, g)), count(g.mentions)))
        .when(hover, |d| d.child(right_tooltip(g.name.clone(), ICON)))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn folder(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    id: u64,
    name: SharedString,
    color: u32,
    guilds: &[&Guild],
    open: bool,
) -> AnyElement {
    let key = id | (1 << 63);
    let hover = hovered(key);
    let unread = guilds.iter().any(|g| g.unread);
    let mentions: u32 = guilds.iter().map(|g| g.mentions).sum();
    let has_selected = !app.favorites_view && guilds.iter().any(|g| Some(g.id) == app.selected_guild);
    let e = entity.clone();
    let head_inner: AnyElement = if open {
        div()
            .size(z(FOLDER))
            .p(z(FOLDER_PAD))
            .child(
                div()
                    .size(z(ICON))
                    .rounded(z(ICON_RADIUS))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(hover, |d| d.bg(c(Palette::GUILD_ICON_BG)))
                    .child(
                        div()
                            .w(z(15.))
                            .h(z(15.))
                            .relative()
                            .child(div().absolute().top_0().left_0().w(z(7.5)).h(z(4.)).rounded_tl(z(2.25)).rounded_tr(z(1.5)).bg(c(color)))
                            .child(div().absolute().top(z(2.25)).left_0().w(z(15.)).h(z(12.75)).rounded(z(2.25)).bg(c(color))),
                    ),
            )
            .into_any_element()
    } else {
        let inner = FOLDER - PREVIEW_PAD * 2.;
        let cell = (inner - PREVIEW_GAP) / 2.;
        let mut grid = div().size(z(inner)).flex().flex_wrap().gap(z(PREVIEW_GAP));
        for (i, g) in guilds.iter().take(4).enumerate() {
            let r = |big: bool| if big { z(13.) } else { z(4.) };
            grid = grid.child(
                div()
                    .size(z(cell))
                    .rounded_tl(r(i == 0))
                    .rounded_tr(r(i == 1))
                    .rounded_bl(r(i == 2))
                    .rounded_br(r(i == 3))
                    .overflow_hidden()
                    .bg(c(Palette::GUILD_ICON_BG))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(c(Palette::TEXT_NORMAL))
                    .child(guild_inner(g, cell)),
            );
        }
        div()
            .size(z(FOLDER))
            .rounded(z(FOLDER_RADIUS))
            .p(z(PREVIEW_PAD))
            .bg(c(color).opacity(0.4))
            .child(grid)
            .into_any_element()
    };
    let drag = DraggedGuild { node: RailNode::Folder(id), icon: ImageRef::Default(0), acronym: SharedString::default(), folder: Some(color) };
    let header = drop_target(row(("folder", id), key, entity), RailNode::Folder(id), None, false, entity)
        .on_drag(drag, |d: &DraggedGuild, _, _, cx: &mut App| cx.new(|_| d.clone()))
        .h(z(FOLDER))
        .on_click(move |_, _, cx| e.update(cx, |a, cx| a.toggle_folder(id, cx)))
        .on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| {
            crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Folder(id), ev.position, w)
        })
        .child(pill(key, !open && has_selected, hover, !open && unread))
        .child(masked(
            FOLDER,
            true,
            head_inner,
            if open { None } else { voice_badge(folder_media(app, guilds)) },
            count(mentions).filter(|_| !open),
        ))
        .when(hover, |d| d.child(right_tooltip(name.clone(), FOLDER)));
    if !open {
        return header.into_any_element();
    }
    let mut body =
        div().w_full().mt(z(gap() - FOLDER_PAD)).flex().flex_col().gap(z(gap())).pb(z(FOLDER_PAD));
    let n = guilds.len();
    for (i, g) in guilds.iter().enumerate() {
        body = body.child(guild(app, entity, g, Some(id), i + 1 == n));
    }
    div()
        .w_full()
        .relative()
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(z((m::server_bar_w() - FOLDER) / 2.))
                .w(z(FOLDER))
                .rounded(z(FOLDER_RADIUS))
                .border_1()
                .border_color(c(Palette::BORDER_MUTED))
                .bg(c(color).opacity(0.15)),
        )
        .child(div().flex().flex_col().child(header).child(body))
        .into_any_element()
}

fn separator() -> AnyElement {
    div()
        .w_full()
        .h(z(1.))
        .flex_none()
        .flex()
        .justify_center()
        .child(div().w(z(32.)).h(z(1.)).rounded(z(1.)).bg(c(Palette::GUILD_SEPARATOR)))
        .into_any_element()
}

fn action(entity: &Entity<PhoenixApp>, id: &'static str, i: Icon, label: SharedString) -> gpui::Stateful<gpui::Div> {
    let key = if id == "add-server" { ADD_KEY } else { DISCOVER_KEY };
    let hover = hovered(key);
    let selected = key == DISCOVER_KEY && crate::ui::discovery::current().is_some();
    row(id, key, entity)
        .child(pill(key, selected, hover, false))
        .child(
            div()
                .size(z(ICON))
                .rounded(z(ICON_RADIUS))
                .relative()
                .child(crate::ui::states::fade_to(
                    crate::ui::states::key(("rail-tile", key)),
                    hover || selected,
                    c(Palette::GUILD_ICON_BG),
                    c(Palette::BLURPLE),
                    150,
                    ICON_RADIUS,
                ))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(i, 20. * crate::theme::zoom(), if hover || selected { gpui::white() } else { c(Palette::TEXT_NORMAL) })),
        )
        .when(hover, |d| d.child(right_tooltip(label, ICON)))
}

#[cfg(test)]
mod rail_dm_tests {
    use super::*;

    fn ids(v: &[u64]) -> Vec<Id> {
        v.iter().map(|&x| Id(x)).collect()
    }

    fn run(dms: &[u64], reads: &[(u64, u32)], hidden: &[u64], call: Option<u64>) -> Vec<u64> {
        let m = |d: Id| reads.iter().find(|(c, _)| *c == d.0).map(|(_, n)| *n);
        rail_dm_ids(&ids(dms), m, |d| hidden.contains(&d.0), |_| false, call.map(Id)).into_iter().map(|d| d.0).collect()
    }

    #[test]
    fn favourite_dms_come_first_in_the_private_channel_order() {
        let reads = [(1, 1), (2, 1), (3, 1), (4, 1)];
        let m = |d: Id| reads.iter().find(|(c, _)| *c == d.0).map(|(_, n)| *n);
        let out = rail_dm_ids(&ids(&[1, 2, 3, 4]), m, |_| false, |d| d.0 == 3 || d.0 == 4, None);
        assert_eq!(out, ids(&[3, 4, 1, 2]));
    }

    #[test]
    fn only_dms_with_mentions_in_the_private_channel_order() {
        let reads = [(1, 0), (2, 3), (3, 1), (4, 0), (5, 2)];
        assert_eq!(run(&[5, 1, 3, 2, 4], &reads, &[], None), vec![5, 3, 2]);
    }

    #[test]
    fn muted_dms_with_a_mention_count_still_show() {
        assert_eq!(run(&[7, 8], &[(7, 1), (8, 0)], &[], None), vec![7]);
    }

    #[test]
    fn message_requests_and_spam_never_show() {
        assert_eq!(run(&[1, 2, 3], &[(1, 1), (2, 1), (3, 1)], &[2, 3], None), vec![1]);
    }

    #[test]
    fn the_stack_stops_at_twenty() {
        let dms: Vec<u64> = (1..=30).collect();
        let reads: Vec<(u64, u32)> = dms.iter().map(|&d| (d, 1)).collect();
        let out = run(&dms, &reads, &[], None);
        assert_eq!(out.len(), RAIL_DM_LIMIT);
        assert_eq!(out.last(), Some(&20));
    }

    #[test]
    fn the_dm_call_goes_first_even_without_mentions() {
        let reads = [(1, 2), (2, 0), (3, 1)];
        assert_eq!(run(&[1, 2, 3], &reads, &[], Some(2)), vec![2, 1, 3]);
        assert_eq!(run(&[1, 2, 3], &reads, &[], Some(3)), vec![3, 1]);
        assert_eq!(run(&[1, 3], &reads, &[], Some(9)), vec![1, 3]);
    }

    #[test]
    fn dms_missing_from_the_channel_map_are_skipped() {
        assert_eq!(run(&[1, 2], &[(2, 1)], &[], None), vec![2]);
    }
}

#[cfg(test)]
mod media_tests {
    use super::*;
    use crate::model::VoiceFlags;

    fn p(stream: bool, video: bool) -> Presence {
        Presence { eligible: true, channel_muted: false, hidden_user: false, flags: VoiceFlags { stream, video, ..Default::default() } }
    }

    #[test]
    fn dm_call_badge_follows_the_client_rtc_mode() {
        let voice = VoiceFlags::default();
        let cam = VoiceFlags { video: true, ..Default::default() };
        let live = VoiceFlags { stream: true, ..Default::default() };
        assert_eq!(dm_media_state(false, false, false, &[]).icon(), None);
        assert_eq!(dm_media_state(false, false, false, &[voice]).icon(), Some("speaker"));
        assert_eq!(dm_media_state(false, false, false, &[cam]).icon(), Some("speaker"));
        assert_eq!(dm_media_state(false, true, false, &[cam]).icon(), Some("video-call"));
        assert_eq!(dm_media_state(false, false, false, &[live]).icon(), Some("golive/screen"));
        let here = dm_media_state(true, true, false, &[]);
        assert_eq!(here, MediaState { audio: true, connected: true, ..Default::default() });
        assert_eq!(dm_media_state(true, true, true, &[voice]).icon(), Some("golive/screen"));
        assert!(!dm_media_state(true, true, true, &[voice]).audio);
    }

    #[test]
    fn one_streamer_is_enough_for_the_screen_badge() {
        let s = media_state(None, false, &[p(false, false), p(true, false), p(false, false)]);
        assert_eq!(s, MediaState { audio: true, video: false, screenshare: true, connected: false, ..Default::default() });
        assert_eq!(s.icon(), Some("golive/screen"));
    }

    #[test]
    fn icon_priority_is_screen_then_video_then_voice() {
        assert_eq!(media_state(None, false, &[p(false, true)]).icon(), Some("video-call"));
        assert_eq!(media_state(None, false, &[p(false, false)]).icon(), Some("speaker"));
        assert_eq!(media_state(None, false, &[p(true, true)]).icon(), Some("golive/screen"));
        assert_eq!(media_state(None, false, &[]).icon(), None);
    }

    #[test]
    fn events_and_stages_come_first_like_the_badge_factory() {
        let mut live = crate::guild_live::Live::default();
        live.stages.insert(1, 10);
        live.events.insert(2, None);
        let base = media_state(None, false, &[p(true, true)]);
        assert_eq!(base.with_live(&live, false, None, |_| true).icon(), Some("calendar"));
        live.events.clear();
        assert_eq!(base.with_live(&live, false, None, |_| true).icon(), Some("stage"));
        assert_eq!(base.with_live(&live, false, None, |c| c != 10).icon(), Some("golive/screen"));
        assert_eq!(MediaState::default().with_live(&live, true, None, |_| true).icon(), None);
        live.events.insert(3, Some(50));
        let here = media_state(Some(Here { channel_has_video: false, self_stream: false }), false, &[]);
        let s = here.with_live(&live, true, Some((50, false)), |_| true);
        assert_eq!((s.icon(), s.connected), (Some("calendar"), true));
        assert_eq!(here.with_live(&live, false, Some((60, true)), |_| true).icon(), Some("stage"));
    }

    #[test]
    fn connected_guild_is_green_and_shows_own_stream_only() {
        let s = media_state(Some(Here { channel_has_video: false, self_stream: false }), true, &[p(true, false)]);
        assert_eq!(s, MediaState { audio: true, video: false, screenshare: false, connected: true, ..Default::default() });
        assert_eq!(s.icon(), Some("speaker"));
        let live = media_state(Some(Here { channel_has_video: true, self_stream: true }), false, &[]);
        assert!(live.connected);
        assert_eq!(live.icon(), Some("golive/screen"));
    }

    #[test]
    fn muted_guilds_channels_and_hidden_users_are_skipped() {
        assert_eq!(media_state(None, true, &[p(true, true)]), MediaState::default());
        let muted = Presence { channel_muted: true, ..p(true, true) };
        assert_eq!(media_state(None, false, &[muted]).icon(), None);
        let blocked = Presence { hidden_user: true, ..p(true, false) };
        assert_eq!(media_state(None, false, &[blocked]).icon(), None);
        let afk = Presence { eligible: false, ..p(false, true) };
        assert_eq!(media_state(None, false, &[afk]).icon(), None);
        let afk_stream = Presence { eligible: false, ..p(true, false) };
        assert_eq!(media_state(None, false, &[afk_stream]).icon(), Some("golive/screen"));
    }
}

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    time::{Duration, Instant},
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
        rail_dnd::set_dragging(None);
    }
    let entity = cx.entity();
    let scroll = RAIL_SCROLL.with(|h| h.clone());
    let mut col = crate::ui::a11y::role(div().id("guild-rail"), gpui::Role::Tree)
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
    let dm_n = dms.len();
    let base = 2 + fav_n + dm_n;
    col = col.children(dms).child(separator());
    let mut slots: Vec<Option<Jump>> = vec![None; 1 + dm_n];
    for (i, item) in rail(app).into_iter().enumerate() {
        let ix = base + i;
        match &item {
            RailItem::Guild(g) => slots.push((g.mentions > 0).then_some(Jump { ix, y: 0., h: ICON })),
            RailItem::Folder { guilds, open: false, .. } => {
                slots.push(guilds.iter().any(|g| g.mentions > 0).then_some(Jump { ix, y: 0., h: FOLDER }))
            }
            RailItem::Folder { guilds, open: true, .. } => {
                slots.push(None);
                for (k, g) in guilds.iter().enumerate() {
                    let y = FOLDER + gap() - FOLDER_PAD + k as f32 * (ICON + gap());
                    slots.push((g.mentions > 0).then_some(Jump { ix, y, h: ICON }));
                }
            }
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
    slots.extend([None; TRAILING_SLOTS]);
    let (top_bar, bottom_bar) = unread_bars(&scroll, &slots, cx);
    crate::ui::a11y::named(div().id("servers-nav"), gpui::Role::Navigation, || crate::t!("a11y.servers_sidebar"))
        .w(z(m::server_bar_w()))
        .flex_none()
        .h_full()
        .relative()
        .bg(c(Palette::BG_TERTIARY))
        .group(crate::ui::chat::scrollbar::THIN_FADE_GROUP)
        .child(col)
        .children(crate::ui::chat::scrollbar::thin_fade_scrollbar_div(&scroll))
        .children(top_bar)
        .children(bottom_bar)
}

thread_local! {
    static RAIL_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

#[doc(hidden)]
pub fn scroll_rail_to(y: f32) {
    RAIL_SCROLL.with(|h| h.set_offset(gpui::point(gpui::px(0.), gpui::px(-y.max(0.)))));
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jump {
    pub ix: usize,
    pub y: f32,
    pub h: f32,
}

const TRAILING_SLOTS: usize = 3;
const RAIL_UNDER_PANELS: f32 = 26.;
const RAIL_TOP_INSET: f32 = 12.;
const BADGE_SIZE: f32 = 16.;
const BOTTOM_INSET: f32 = 16.;
const JUMP_PADDING: f32 = 24.;
const BAR_H: f32 = 24.;
const BAR_TRAVEL: f32 = 1.8;
const BAR_TENSION: f32 = 300.;
const BAR_FRICTION: f32 = 10.;
const BAR_IN_DELAY: Duration = Duration::from_millis(100);
const BAR_HIDE_AFTER: Duration = Duration::from_millis(200);
const CALC_DEBOUNCE: Duration = Duration::from_millis(200);
const SCROLL_DEBOUNCE: Duration = Duration::from_millis(400);

pub fn slot_visible(o: usize, bottom: bool, scroll_top: f32, view_h: f32, gap: f32) -> bool {
    let stride = ICON + gap;
    let pos = stride * o as f32 + RAIL_TOP_INSET + 2. + gap;
    if bottom {
        pos + stride + (stride - BADGE_SIZE) <= scroll_top + view_h + RAIL_UNDER_PANELS - BOTTOM_INSET
    } else {
        pos >= scroll_top
    }
}

pub fn hidden_mention<T: Copy>(slots: &[Option<T>], bottom: bool, visible: impl Fn(usize) -> bool) -> Option<T> {
    let order: Box<dyn Iterator<Item = usize>> = if bottom { Box::new((0..slots.len()).rev()) } else { Box::new(0..slots.len()) };
    for o in order {
        if visible(o) {
            return None;
        }
        if let Some(j) = slots[o] {
            return Some(j);
        }
    }
    None
}

pub fn into_view(start: f32, end: f32, scroll_top: f32, size: f32) -> Option<f32> {
    let (start, end) = (start - JUMP_PADDING, end + JUMP_PADDING);
    if start >= scroll_top && end <= scroll_top + size {
        None
    } else if start < scroll_top {
        Some(start)
    } else {
        Some(end - size)
    }
}

#[derive(Clone, Copy, Default)]
struct Bar {
    shown: Option<Jump>,
    toggled: Option<Instant>,
    from: f32,
    animate: bool,
    want: Option<Option<Jump>>,
    want_at: Option<Instant>,
}

impl Bar {
    fn target(&self) -> f32 {
        if self.shown.is_some() { 0. } else { 1. }
    }

    fn delay(&self) -> Duration {
        if self.shown.is_some() { BAR_IN_DELAY } else { Duration::ZERO }
    }

    fn hidden_at(&self, now: Instant) -> f32 {
        let Some(at) = self.toggled.filter(|_| self.animate) else { return self.target() };
        let t = now.saturating_duration_since(at).saturating_sub(self.delay()).as_secs_f32();
        self.from + (self.target() - self.from) * gpui::spring_progress(bar_spring(), t)
    }
}

fn bar_spring() -> gpui::SpringConfig {
    crate::ui::popout_motion::origami(BAR_TENSION, BAR_FRICTION)
}

struct RailBars {
    top: Bar,
    bottom: Bar,
    offset: f32,
    moved: Option<Instant>,
    wake: Option<Instant>,
}

thread_local! {
    static BARS: RefCell<RailBars> =
        RefCell::new(RailBars { top: Bar::default(), bottom: Bar::default(), offset: 0., moved: None, wake: None });
    static RAIL_JUMP: RefCell<Option<(crate::ui::chat::page_scroll::Spring, f32)>> = const { RefCell::new(None) };
}

fn settle(bar: &mut Bar, want: Option<Jump>, moved: Option<Instant>, now: Instant) -> Option<Instant> {
    if want == bar.shown {
        bar.want = None;
        bar.want_at = None;
        return None;
    }
    if bar.want != Some(want) {
        bar.want = Some(want);
        bar.want_at = Some(now);
    }
    let ready = [bar.want_at.map(|t| t + CALC_DEBOUNCE), moved.map(|t| t + SCROLL_DEBOUNCE)].into_iter().flatten().max().unwrap_or(now);
    if now < ready {
        return Some(ready);
    }
    if want.is_some() != bar.shown.is_some() {
        bar.from = bar.hidden_at(now);
        bar.toggled = Some(now);
        bar.animate = crate::media::emoji::window_focused() && !crate::ui::settings::effects::reduced_motion();
    }
    bar.shown = want;
    bar.want = None;
    bar.want_at = None;
    None
}

fn unread_bars(h: &gpui::ScrollHandle, slots: &[Option<Jump>], cx: &mut Context<PhoenixApp>) -> (Option<AnyElement>, Option<AnyElement>) {
    let view = h.bounds();
    let zf = f32::from(z(1.));
    let now = Instant::now();
    let scroll_top = -f32::from(h.offset().y) / zf;
    let view_h = f32::from(view.size.height) / zf;
    let measured = view.size.height > px(0.);
    let g = gap();
    let want = |bottom: bool| {
        measured.then(|| hidden_mention(slots, bottom, |o| slot_visible(o, bottom, scroll_top, view_h, g))).flatten()
    };
    let (want_top, want_bottom) = (want(false), want(true));
    let (top, bottom, wake) = BARS.with_borrow_mut(|b| {
        if (b.offset - scroll_top).abs() > 0.01 {
            b.offset = scroll_top;
            b.moved = Some(now);
        }
        let moved = b.moved;
        let wake = [settle(&mut b.top, want_top, moved, now), settle(&mut b.bottom, want_bottom, moved, now)].into_iter().flatten().min();
        let wake = wake.filter(|w| b.wake.is_none_or(|old| old <= now || *w < old));
        if wake.is_some() {
            b.wake = wake;
        }
        (b.top, b.bottom, wake)
    });
    if let Some(at) = wake {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(at.saturating_duration_since(Instant::now()) + Duration::from_millis(5)).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    (unread_bar(h, &top, true, now), unread_bar(h, &bottom, false, now))
}

fn unread_bar(h: &gpui::ScrollHandle, bar: &Bar, top: bool, now: Instant) -> Option<AnyElement> {
    let gone = bar.toggled.is_none_or(|at| now.saturating_duration_since(at) >= BAR_HIDE_AFTER);
    if bar.shown.is_none() && gone {
        return None;
    }
    let travel = if top { -BAR_H * BAR_TRAVEL } else { BAR_H * BAR_TRAVEL };
    let transition = bar.toggled.filter(|_| bar.animate).map(|at| {
        gpui::Transition::spring(bar_spring())
            .start(at)
            .delay(bar.delay())
            .translate(point(px(0.), z(travel * bar.from)), point(px(0.), z(travel * bar.target())))
            .keep_masks()
    });
    let still = (!bar.animate).then(|| z(travel * bar.target()));
    let mention = bar.shown.is_some();
    let id = if top { "rail-unread-top" } else { "rail-unread-bottom" };
    let pill = div()
        .id(id)
        .relative()
        .when_some(still, |d, y| d.top(y))
        .h(z(BAR_H))
        .rounded(z(12.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .map(|d| {
            if mention {
                d.bg(c(tok(idx::BACKGROUND_FEEDBACK_NOTIFICATION)))
                    .border_color(c(tok(idx::BORDER_SUBTLE)))
                    .shadow(crate::ui::inbox::shadow_medium())
            } else {
                d.bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST)))
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
        .text_color(c(tok(if mention { idx::TEXT_OVERLAY_LIGHT } else { idx::TEXT_STRONG })))
        .child(crate::t!("rail.unread_new").to_uppercase())
        .when_some(bar.shown, |d, j| {
            let h = h.clone();
            d.cursor_pointer()
                .active(|s| s.top(z(1.)).bg(c(tok(idx::RED_430))))
                .on_click(move |_, w, _| jump_to(&h, j, w))
        });
    Some(
        div()
            .absolute()
            .left_0()
            .right_0()
            .overflow_hidden()
            .when(top, |d| d.top_0().p(z(8.)))
            .when(!top, |d| d.bottom_0().pt(z(8.)).px(z(8.)))
            .child(pill.present_transition(if top { "rail-unread-top-slide" } else { "rail-unread-bottom-slide" }, transition))
            .into_any_element(),
    )
}

fn jump_to(h: &gpui::ScrollHandle, j: Jump, window: &mut Window) {
    let Some(b) = h.bounds_for_item(j.ix) else { return };
    let zf = f32::from(z(1.));
    let view = h.bounds();
    let scroll_top = -f32::from(h.offset().y) / zf;
    let size = f32::from(view.size.height) / zf + RAIL_UNDER_PANELS;
    let start = f32::from(b.top() - view.top()) / zf + scroll_top + j.y;
    let Some(to) = into_view(start, start + j.h, scroll_top, size) else { return };
    let to = to.clamp(0., f32::from(h.max_offset().y) / zf);
    let idle = RAIL_JUMP.with_borrow_mut(|r| {
        let idle = r.is_none();
        *r = Some((crate::ui::chat::page_scroll::Spring::new(f64::from(scroll_top), f64::from(to)), scroll_top));
        idle
    });
    if idle {
        window.on_next_frame(jump_frame);
    }
}

fn jump_frame(window: &mut Window, _: &mut App) {
    let handle = RAIL_SCROLL.with(|h| h.clone());
    let zf = f32::from(z(1.));
    let now_top = -f32::from(handle.offset().y) / zf;
    let next = RAIL_JUMP.with_borrow_mut(|r| {
        let (spring, shown) = r.as_mut()?;
        if (now_top - *shown).abs() > 1. {
            *r = None;
            return None;
        }
        let (pos, done) = spring.advance(Instant::now());
        *shown = pos as f32;
        if done {
            *r = None;
        }
        Some((pos as f32, done))
    });
    let Some((pos, done)) = next else { return };
    handle.set_offset(point(px(0.), -z(pos)));
    window.refresh();
    if !done {
        window.on_next_frame(jump_frame);
    }
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
const PREVIEW_OUTER_RADIUS: f32 = 13.;
const PREVIEW_INNER_RADIUS: f32 = 4.;
const PILL_MS: u64 = 200;
const EASE_OUT_QUINT: gpui::CubicBezier = gpui::CubicBezier(0.22, 1., 0.36, 1.);

thread_local! {
    static HOVERED: Cell<Option<u64>> = const { Cell::new(None) };
    static PILLS: RefCell<HashMap<u64, (f32, f32, Instant)>> = RefCell::new(HashMap::new());
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
            e.update(cx, |a, cx| {
                if *over {
                    crate::history_ui::hover_guild(a, Id(key), cx);
                }
                cx.notify()
            });
        }
    })
}

pub fn right_tooltip(text: SharedString, item_h: f32) -> AnyElement {
    right_tooltip_with(text, None, item_h)
}

const TOOLTIP_MAX_USERS: usize = 6;

pub fn tooltip_avatar_slice(len: usize) -> (usize, usize) {
    if len <= TOOLTIP_MAX_USERS {
        (len, 0)
    } else {
        let shown = TOOLTIP_MAX_USERS - 1;
        (shown, (len - shown).min(99))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuildTooltipState {
    pub voice: Vec<Id>,
    pub stream: Vec<Id>,
    pub activity: Vec<Id>,
    pub muted: bool,
}

impl GuildTooltipState {
    pub fn is_empty(&self) -> bool {
        !self.muted && self.voice.is_empty() && self.stream.is_empty() && self.activity.is_empty()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TooltipParticipant {
    pub user: Id,
    pub eligible: bool,
    pub channel_muted: bool,
    pub hidden_user: bool,
    pub streaming: bool,
}

pub fn guild_tooltip_state(guild_muted: bool, participants: &[TooltipParticipant], activity_users: &[Id]) -> GuildTooltipState {
    let mut out = GuildTooltipState { muted: guild_muted, ..Default::default() };
    for &u in activity_users {
        if !out.activity.contains(&u) {
            out.activity.push(u);
        }
    }
    for p in participants {
        if p.hidden_user || p.channel_muted || out.activity.contains(&p.user) {
            continue;
        }
        if p.streaming {
            if !out.stream.contains(&p.user) {
                out.stream.push(p.user);
            }
        } else if p.eligible && !out.voice.contains(&p.user) {
            out.voice.push(p.user);
        }
    }
    out.voice.retain(|u| !out.stream.contains(u));
    out
}

fn collect_guild_tooltip(app: &PhoenixApp, g: &Guild) -> GuildTooltipState {
    let prefs = app.backend.notify_prefs();
    let guild_muted = prefs.as_ref().is_some_and(|p| p.is_guild_muted(Some(g.id.0)));
    let Some(chans) = app.guild_channels.get(&g.id) else {
        return GuildTooltipState { muted: guild_muted, ..Default::default() };
    };
    let connected = app.voice_channel.is_some_and(|v| chans.contains(&v));
    let flags = crate::ui::voice_panel::flags;
    let me = app.me.id;
    let self_stream = connected && (crate::ui::go_live::live(app) || flags(me).stream);
    let mut participants = Vec::new();
    for ch in chans.iter().filter_map(|c| app.channels.get(c)) {
        if ch.voice_users.is_empty() {
            continue;
        }
        let eligible = ch.kind != crate::model::ChannelKind::Stage && g.afk_channel != Some(ch.id);
        let channel_muted = prefs.as_ref().is_some_and(|p| p.is_channel_muted(Some(g.id.0), ch.id.0));
        for &u in &ch.voice_users {
            let hidden_user = app.nav.is_blocked(u) || app.nav.is_ignored(u);
            let streaming = flags(u).stream || (u == me && self_stream);
            participants.push(TooltipParticipant { user: u, eligible, channel_muted, hidden_user, streaming });
        }
    }
    let live = crate::guild_live::get(g.id.0);
    let mut activity_users = Vec::new();
    for a in live.activities.values() {
        for &u in &a.users {
            if !hidden(app, u) {
                activity_users.push(Id(u));
            }
        }
    }
    guild_tooltip_state(guild_muted, &participants, &activity_users)
}

fn activity_row(app: &PhoenixApp, icon_name: &'static str, users: &[Id]) -> Option<gpui::Div> {
    if users.is_empty() {
        return None;
    }
    let (shown, more) = tooltip_avatar_slice(users.len());
    let tok = |i| c(crate::theme::tok(i));
    use crate::theme::generated::idx;
    Some(
        div()
            .mt(z(8.))
            .flex()
            .items_center()
            .child(svg().path(crate::icons::path(icon_name)).size(z(20.)).mr(z(8.)).flex_none().text_color(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
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

fn voice_row(app: &PhoenixApp, users: &[Id]) -> Option<gpui::Div> {
    activity_row(app, "speaker", users)
}

fn guild_tooltip_extra(app: &PhoenixApp, g: &Guild) -> Option<gpui::Div> {
    let state = collect_guild_tooltip(app, g);
    if state.is_empty() {
        return None;
    }
    let after_rows = !state.voice.is_empty() || !state.stream.is_empty();
    Some(
        div()
            .flex()
            .flex_col()
            .children(activity_row(app, "speaker", &state.voice))
            .children(activity_row(app, "voice/screen", &state.stream))
            .children(activity_row(app, "apps", &state.activity))
            .when(state.muted, |d| {
                d.child(
                    div()
                        .when(after_rows, |d| d.mt(z(8.)))
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_MUTED)))
                        .child(crate::t!("rail.muted")),
                )
            }),
    )
}

fn guild_tooltip_name(app: &PhoenixApp, g: &Guild) -> AnyElement {
    let mut features = app.backend.guild_features(g.id);
    if g.discoverable {
        features.push("DISCOVERABLE".into());
    }
    let kind = crate::ui::guild_badge::kind(&features);
    let badge = match kind {
        crate::ui::guild_badge::Kind::Staff => None,
        _ => crate::ui::guild_badge::element(kind, crate::ui::guild_badge::premium(g.boosts, g.boost_tier), 18.),
    };
    div()
        .flex()
        .items_start()
        .children(badge.map(|b| div().flex_none().mt(z(2.)).mr(z(8.)).child(b)))
        .child(div().min_w_0().line_clamp(2).text_ellipsis().child(g.name.clone()))
        .into_any_element()
}

fn right_tooltip_with(text: SharedString, extra: Option<gpui::Div>, item_h: f32) -> AnyElement {
    right_tooltip_el(text.into_any_element(), extra, item_h)
}

fn right_tooltip_el(head: AnyElement, extra: Option<gpui::Div>, item_h: f32) -> AnyElement {
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
        .child(head)
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
        let e = p.entry(key).or_insert((target, target, Instant::now()));
        if e.1 != target {
            *e = (e.1, target, Instant::now());
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
    pub activity: bool,
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

    pub fn with_activities(mut self, live: &crate::guild_live::Live, muted: bool, hidden_user: impl Fn(u64) -> bool, channel_ok: impl Fn(u64) -> bool) -> Self {
        let mut shown = live.activities.values().filter(|a| !a.users.iter().any(|u| hidden_user(*u)));
        self.activity = if self.connected {
            shown.next().is_some()
        } else {
            !muted && shown.any(|a| a.channel.is_some_and(&channel_ok))
        };
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
        } else if self.activity {
            Some("apps")
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
        screenshare: false,
        connected: false,
        ..Default::default()
    }
}

pub fn guild_media(app: &PhoenixApp, g: &Guild) -> MediaState {
    let Some(chans) = app.guild_channels.get(&g.id) else { return MediaState::default() };
    let flags = crate::ui::voice_panel::flags;
    if let Some(v) = app.voice_channel.filter(|v| chans.contains(v)) {
        let users = app.channels.get(&v).map(|c| c.voice_users.as_slice()).unwrap_or_default();
        let me = app.me.id;
        let here = Here {
            channel_has_video: users.iter().any(|u| flags(*u).video),
            self_stream: crate::ui::go_live::live(app) || flags(me).stream,
        };
        let stage = app.channels.get(&v).is_some_and(|c| c.kind == crate::model::ChannelKind::Stage);
        let live = crate::guild_live::get(g.id.0);
        return media_state(Some(here), false, &[])
            .with_live(&live, false, Some((v.0, stage)), |_| true)
            .with_activities(&live, false, |u| hidden(app, u), |_| true);
    }
    let live = crate::guild_live::get(g.id.0);
    let quiet = live.stages.is_empty() && live.events.is_empty() && live.activities.is_empty();
    if quiet && chans.iter().filter_map(|c| app.channels.get(c)).all(|ch| ch.voice_users.is_empty()) {
        return MediaState::default();
    }
    let prefs = app.backend.notify_prefs();
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
    media_state(None, guild_muted, &presences)
        .with_live(&live, guild_muted, None, visible)
        .with_activities(&live, guild_muted, |u| hidden(app, u), |c| activity_channel(app, crate::model::Id(c)))
}

fn hidden(app: &PhoenixApp, u: u64) -> bool {
    app.nav.is_blocked(crate::model::Id(u)) || app.nav.is_ignored(crate::model::Id(u))
}

const USE_EMBEDDED_ACTIVITIES: u64 = 1 << 39;

fn activity_channel(app: &PhoenixApp, id: crate::model::Id) -> bool {
    use crate::model::ChannelKind;
    let Some(ch) = app.channels.get(&id) else { return false };
    let top_level = ch.parent.and_then(|p| app.channels.get(&p)).is_none_or(|p| p.kind == ChannelKind::Category);
    let vocal = matches!(ch.kind, ChannelKind::Voice | ChannelKind::Stage);
    let launchable = top_level
        && matches!(ch.kind, ChannelKind::Text | ChannelKind::Voice)
        && app.backend.channel_permissions(id).is_none_or(|p| p & USE_EMBEDDED_ACTIVITIES != 0);
    vocal || launchable || !top_level
}

pub fn folder_media(app: &PhoenixApp, guilds: &[&Guild]) -> MediaState {
    guilds
        .iter()
        .map(|g| guild_media(app, g))
        .find(|s| s.connected)
        .map(|s| MediaState { audio: !s.video, live_stage: false, active_event: false, activity: false, ..s })
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
    let video_mode = tracked && (self_stream || users.iter().any(|f| f.video));
    MediaState {
        audio: in_call && !video_mode,
        video: in_call && video_mode,
        screenshare: self_stream,
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
            crate::ui::a11y::named(row(("rail-dm", id.0), key, entity), gpui::Role::TreeItem, || {
                crate::ui::a11y::channel(crate::ui::a11y::Kind::Dm, &name, &crate::ui::a11y::ChannelInfo { mentions: ch.mentions, unread: ch.unread, ..Default::default() })
            })
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

pub const DISCODO_KEY: &str = "discodo-easter-egg";
const DISCODO_CLICKS: u32 = 16;
const DISCODO_RESET: Duration = Duration::from_millis(500);
const DISCODO_LABEL: Duration = Duration::from_secs(1);

thread_local! {
    static DISCODO: Cell<(u32, Option<Instant>)> = const { Cell::new((0, None)) };
    static DISCODO_SHOWN: Cell<Option<Instant>> = const { Cell::new(None) };
}

pub fn discodo_click(state: (u32, Option<Instant>), now: Instant) -> ((u32, Option<Instant>), bool) {
    let clicks = if state.1.is_some_and(|t| now.duration_since(t) < DISCODO_RESET) { state.0 } else { 0 };
    if clicks + 1 == DISCODO_CLICKS { ((0, None), true) } else { ((clicks + 1, Some(now)), false) }
}

fn on_home_click(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let now = Instant::now();
    let (next, fire) = discodo_click(DISCODO.get(), now);
    DISCODO.set(next);
    if fire {
        let on = !a.prefs.local.flag(DISCODO_KEY, false);
        a.prefs.local.ui.insert(DISCODO_KEY.into(), u64::from(on));
        a.prefs.local.save();
        crate::sounds::play(if on { "discodo" } else { "user_leave" }, 1.);
        DISCODO_SHOWN.set(Some(now));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DISCODO_LABEL).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    a.select_guild(None, cx);
}

fn home_label(app: &PhoenixApp) -> SharedString {
    match DISCODO_SHOWN.get() {
        Some(t) if t.elapsed() < DISCODO_LABEL => {
            if app.prefs.local.flag(DISCODO_KEY, false) { crate::t!("home.discodo_on") } else { crate::t!("home.discodo_off") }
        }
        _ => crate::t!("dm.title"),
    }
}

fn home(app: &PhoenixApp, entity: &Entity<PhoenixApp>) -> AnyElement {
    let selected = app.selected_guild.is_none() && !app.favorites_view && crate::ui::discovery::current().is_none();
    let hover = hovered(HOME_KEY);
    let pending = app.nav.pending_requests() + app.nav.message_requests.len() as u32;
    let e = entity.clone();
    crate::ui::a11y::named(row("home", HOME_KEY, entity), gpui::Role::TreeItem, || crate::t!("a11y.direct_messages"))
        .when(crate::ui::a11y::on(), |d| d.aria_selected(selected))
        .on_click(move |_, _, cx| e.update(cx, on_home_click))
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
        .when(hover, |d| d.child(right_tooltip(home_label(app), ICON)))
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

fn preview_radii(i: usize) -> [f32; 4] {
    let mut r = [PREVIEW_INNER_RADIUS; 4];
    r[[0, 1, 3, 2][i.min(3)]] = PREVIEW_OUTER_RADIUS;
    r
}

fn corners<E: Styled>(el: E, r: [f32; 4]) -> E {
    el.rounded_tl(z(r[0])).rounded_tr(z(r[1])).rounded_br(z(r[2])).rounded_bl(z(r[3]))
}

fn guild_inner(g: &Guild, size: f32, r: [f32; 4]) -> AnyElement {
    match &g.icon {
        ImageRef::Url(u) if crate::media::is_animated(u) => {
            let group = SharedString::new_static("phx-guild-icon");
            div()
                .relative()
                .size(z(size))
                .group(group.clone())
                .child(corners(img(crate::media::static_variant(u)).size(z(size)), r))
                .child(corners(
                    img(u.clone())
                        .id(crate::media::anim_id(u))
                        .absolute()
                        .top_0()
                        .left_0()
                        .size(z(size))
                        .invisible()
                        .group_hover(group, |s| s.visible()),
                    r,
                ))
                .into_any_element()
        }
        ImageRef::Url(u) => corners(img(u.clone()).size(z(size)), r).into_any_element(),
        _ if size < 30. => div()
            .w_full()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_center()
            .text_size(z(10.))
            .line_height(z(size))
            .font_weight(FontWeight::NORMAL)
            .child(g.acronym.clone())
            .into_any_element(),
        _ => div()
            .text_size(z(match g.acronym.chars().count() {
                0..=2 => 18.,
                3 | 4 => 16.,
                5 => 14.,
                6 => 12.,
                _ => 10.,
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

fn drag_placeholder(folder: bool) -> gpui::Div {
    div()
        .size(z(ICON))
        .when(folder, |d| d.m(z(FOLDER_PAD)))
        .flex_none()
        .rounded(z(ICON_RADIUS))
        .border_1()
        .border_dashed()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
}

fn start_drag(d: &DraggedGuild, cx: &mut App) -> Entity<DraggedGuild> {
    rail_dnd::set_dragging(Some(d.node));
    cx.new(|_| d.clone())
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
        rail_dnd::set_dragging(None);
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
        .child(guild_inner(g, ICON, [ICON_RADIUS; 4]));
    drop_target(row(("guild", id.0), id.0, entity), RailNode::Guild(id), folder, last, entity)
        .map(|d| crate::ui::a11y::named(d, gpui::Role::TreeItem, || crate::ui::a11y::guild(&g.name, g.mentions, g.unread, guild_media(app, g))))
        .when(crate::ui::a11y::on(), |d| d.aria_selected(selected).aria_level(if folder.is_some() { 2 } else { 1 }))
        .on_click(move |_, _, cx| e.update(cx, |app, cx| app.select_guild(Some(id), cx)))
        .on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| {
            crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Guild(id), ev.position, w)
        })
        .on_drag(drag, |d: &DraggedGuild, _, _, cx: &mut App| start_drag(d, cx))
        .map(|d| {
            if rail_dnd::dragging() == Some(RailNode::Guild(id)) {
                return d.child(drag_placeholder(false));
            }
            d.child(pill(id.0, selected, hover, g.unread))
                .child(masked(ICON, false, icon_box, voice_badge(guild_media(app, g)), count(g.mentions)))
                .when(hover, |d| d.child(right_tooltip_el(guild_tooltip_name(app, g), guild_tooltip_extra(app, g), ICON)))
        })
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
            let r = preview_radii(i);
            grid = grid.child(
                corners(div().size(z(cell)), r)
                    .overflow_hidden()
                    .when(!matches!(g.icon, ImageRef::Url(_)), |d| d.bg(c(Palette::BG_PRIMARY)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(c(Palette::TEXT_NORMAL))
                    .child(guild_inner(g, cell, r)),
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
        .on_drag(drag, |d: &DraggedGuild, _, _, cx: &mut App| start_drag(d, cx))
        .map(|d| crate::ui::a11y::named(d, gpui::Role::TreeItem, || crate::ui::a11y::folder(&name, mentions)))
        .when(crate::ui::a11y::on(), |d| d.aria_expanded(open).aria_level(1))
        .h(z(FOLDER))
        .on_click(move |_, _, cx| e.update(cx, |a, cx| a.toggle_folder(id, cx)))
        .on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| {
            crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Folder(id), ev.position, w)
        })
        .map(|d| {
            if rail_dnd::dragging() == Some(RailNode::Folder(id)) {
                return d.child(drag_placeholder(true));
            }
            d.child(pill(key, !open && has_selected, hover, !open && unread))
                .child(masked(
                    FOLDER,
                    true,
                    head_inner,
                    if open { None } else { voice_badge(folder_media(app, guilds)) },
                    count(mentions).filter(|_| !open),
                ))
                .when(hover, |d| d.child(right_tooltip(name.clone(), FOLDER)))
        });
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
    crate::ui::a11y::named(row(id, key, entity), gpui::Role::Button, || label.clone())
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

    #[test]
    fn folder_preview_rounds_only_the_outer_corner() {
        assert_eq!(preview_radii(0), [13., 4., 4., 4.]);
        assert_eq!(preview_radii(1), [4., 13., 4., 4.]);
        assert_eq!(preview_radii(2), [4., 4., 4., 13.]);
        assert_eq!(preview_radii(3), [4., 4., 13., 4.]);
    }

    #[test]
    fn sixteen_quick_home_clicks_toggle_discodo() {
        let t0 = Instant::now();
        let mut st = (0, None);
        for i in 0..15 {
            let (next, fire) = discodo_click(st, t0 + Duration::from_millis(100 * i));
            assert!(!fire);
            st = next;
        }
        let (next, fire) = discodo_click(st, t0 + Duration::from_millis(1500));
        assert!(fire && next == (0, None), "the 16th click fires and resets");
        let (slow, _) = discodo_click((15, Some(t0)), t0 + DISCODO_RESET);
        assert_eq!(slow.0, 1, "a 500 ms pause starts over");
    }

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
        assert_eq!(dm_media_state(false, false, false, &[live]).icon(), Some("speaker"));
        let here = dm_media_state(true, true, false, &[]);
        assert_eq!(here, MediaState { audio: true, connected: true, ..Default::default() });
        assert_eq!(dm_media_state(true, true, true, &[voice]).icon(), Some("golive/screen"));
        assert!(!dm_media_state(true, true, true, &[voice]).audio);
    }

    #[test]
    fn remote_streamer_does_not_show_the_screen_badge() {
        let s = media_state(None, false, &[p(false, false), p(true, false), p(false, false)]);
        assert_eq!(s, MediaState { audio: true, video: false, screenshare: false, connected: false, ..Default::default() });
        assert_eq!(s.icon(), Some("speaker"));
    }

    #[test]
    fn icon_priority_is_screen_then_video_then_voice() {
        assert_eq!(media_state(None, false, &[p(false, true)]).icon(), Some("video-call"));
        assert_eq!(media_state(None, false, &[p(false, false)]).icon(), Some("speaker"));
        assert_eq!(media_state(None, false, &[p(true, true)]).icon(), Some("video-call"));
        assert_eq!(media_state(Some(Here { channel_has_video: true, self_stream: true }), false, &[]).icon(), Some("golive/screen"));
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
        assert_eq!(base.with_live(&live, false, None, |c| c != 10).icon(), Some("video-call"));
        assert_eq!(MediaState::default().with_live(&live, true, None, |_| true).icon(), None);
        live.events.insert(3, Some(50));
        let here = media_state(Some(Here { channel_has_video: false, self_stream: false }), false, &[]);
        let s = here.with_live(&live, true, Some((50, false)), |_| true);
        assert_eq!((s.icon(), s.connected), (Some("calendar"), true));
        assert_eq!(here.with_live(&live, false, Some((60, true)), |_| true).icon(), Some("stage"));
    }

    #[test]
    fn activities_come_last_and_skip_blocked_players() {
        let mut live = crate::guild_live::Live::default();
        live.activities.insert(("gc".into(), 9), crate::guild_live::Activity { channel: Some(50), users: vec![1, 2] });
        let idle = MediaState::default();
        assert_eq!(idle.with_activities(&live, false, |_| false, |_| true).icon(), Some("apps"));
        assert_eq!(idle.with_activities(&live, false, |u| u == 2, |_| true).icon(), None);
        assert_eq!(idle.with_activities(&live, true, |_| false, |_| true).icon(), None);
        assert_eq!(idle.with_activities(&live, false, |_| false, |c| c != 50).icon(), None);
        let voice = media_state(None, false, &[p(false, false)]).with_activities(&live, false, |_| false, |_| true);
        assert_eq!((voice.icon(), voice.activity), (Some("speaker"), true));
        let here = media_state(Some(Here { channel_has_video: false, self_stream: false }), false, &[]);
        assert!(here.with_activities(&live, true, |_| false, |_| false).activity);
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
        assert_eq!(media_state(None, false, &[afk_stream]).icon(), None);
    }

    fn tp(user: u64, streaming: bool) -> TooltipParticipant {
        TooltipParticipant { user: Id(user), eligible: true, channel_muted: false, hidden_user: false, streaming }
    }

    #[test]
    fn tooltip_avatar_slice_caps_at_six_and_ninety_nine() {
        assert_eq!(tooltip_avatar_slice(0), (0, 0));
        assert_eq!(tooltip_avatar_slice(3), (3, 0));
        assert_eq!(tooltip_avatar_slice(6), (6, 0));
        assert_eq!(tooltip_avatar_slice(7), (5, 2));
        assert_eq!(tooltip_avatar_slice(200), (5, 99));
    }

    #[test]
    fn tooltip_partitions_voice_stream_and_activity_without_duplicates() {
        let s = guild_tooltip_state(
            false,
            &[tp(1, false), tp(2, true), tp(3, false), tp(1, false), tp(4, false), tp(5, true)],
            &[Id(4), Id(5)],
        );
        assert_eq!(s.voice, vec![Id(1), Id(3)]);
        assert_eq!(s.stream, vec![Id(2)]);
        assert_eq!(s.activity, vec![Id(4), Id(5)]);
        assert!(!s.muted);
    }

    #[test]
    fn tooltip_muted_guild_keeps_its_voice_and_stream_rows() {
        let empty = guild_tooltip_state(true, &[], &[]);
        assert_eq!(empty, GuildTooltipState { muted: true, ..Default::default() });
        assert!(!empty.is_empty());

        let busy = guild_tooltip_state(true, &[tp(1, false), tp(2, true)], &[Id(3)]);
        assert!(busy.muted);
        assert_eq!(busy.voice, vec![Id(1)]);
        assert_eq!(busy.stream, vec![Id(2)]);
        assert_eq!(busy.activity, vec![Id(3)]);
    }

    #[test]
    fn tooltip_skips_muted_channels_afk_and_hidden_users() {
        let s = guild_tooltip_state(
            false,
            &[
                TooltipParticipant { channel_muted: true, ..tp(1, false) },
                TooltipParticipant { eligible: false, ..tp(2, false) },
                TooltipParticipant { hidden_user: true, ..tp(3, false) },
                tp(4, false),
                TooltipParticipant { eligible: false, ..tp(5, true) },
            ],
            &[],
        );
        assert_eq!(s.voice, vec![Id(4)]);
        assert_eq!(s.stream, vec![Id(5)]);
    }
}

#[cfg(test)]
mod unread_bar_tests {
    use super::*;

    fn j(ix: usize) -> Option<Jump> {
        Some(Jump { ix, y: 0., h: ICON })
    }

    #[test]
    fn bottom_bar_hides_once_the_mentioned_slot_is_in_the_client_band() {
        let view_h = 512. - RAIL_UNDER_PANELS;
        assert!(!slot_visible(8, true, 0., view_h, 10.));
        assert!(!slot_visible(8, true, 11.33, view_h, 10.));
        assert!(slot_visible(8, true, 12., view_h, 10.));
        assert!(slot_visible(8, true, 40., view_h, 10.));
    }

    #[test]
    fn top_bar_counts_a_slot_as_hidden_as_soon_as_its_top_leaves() {
        assert!(slot_visible(2, false, 124., 400., 10.));
        assert!(!slot_visible(2, false, 124.5, 400., 10.));
        assert!(slot_visible(0, false, 0., 400., 8.));
    }

    #[test]
    fn the_walk_stops_at_the_first_visible_slot_from_the_edge() {
        let slots = [None, j(3), None, None, j(6), None, None, None];
        assert_eq!(hidden_mention(&slots, false, |o| o >= 2), j(3));
        assert_eq!(hidden_mention(&slots, false, |o| o >= 1), None);
        assert_eq!(hidden_mention(&slots, true, |o| o <= 3), j(6));
        assert_eq!(hidden_mention(&slots, true, |o| o <= 4), None);
        assert_eq!(hidden_mention(&slots, true, |_| false), j(6));
        assert_eq!(hidden_mention(&slots, false, |_| false), j(3));
    }

    #[test]
    fn jump_scrolls_the_node_into_view_with_the_client_padding() {
        assert_eq!(into_view(100., 140., 0., 400.), None);
        assert_eq!(into_view(500., 540., 0., 400.), Some(164.));
        assert_eq!(into_view(10., 50., 100., 400.), Some(-14.));
        assert_eq!(into_view(20., 60., 0., 400.), Some(-4.));
    }

    #[test]
    fn a_new_target_waits_for_the_scroll_and_calc_debounces() {
        let t0 = Instant::now();
        let mut bar = Bar::default();
        let wake = settle(&mut bar, j(4), Some(t0), t0);
        assert_eq!(wake, Some(t0 + SCROLL_DEBOUNCE));
        assert_eq!(bar.shown, None);
        assert_eq!(settle(&mut bar, j(4), Some(t0), t0 + Duration::from_millis(399)), Some(t0 + SCROLL_DEBOUNCE));
        assert_eq!(settle(&mut bar, j(4), Some(t0), t0 + SCROLL_DEBOUNCE), None);
        assert_eq!(bar.shown, j(4));
        assert_eq!(bar.toggled, Some(t0 + SCROLL_DEBOUNCE));
        let t1 = t0 + Duration::from_secs(2);
        assert_eq!(settle(&mut bar, None, Some(t0), t1), Some(t1 + CALC_DEBOUNCE));
        assert_eq!(settle(&mut bar, j(4), Some(t0), t1 + Duration::from_millis(50)), None);
        assert_eq!(bar.shown, j(4));
        assert_eq!(bar.want, None);
    }

    #[test]
    fn the_bar_spring_is_the_origami_tension_300_friction_10() {
        let s = bar_spring();
        assert!((s.stiffness - 1171.4).abs() < 0.1);
        assert!((s.damping - 31.).abs() < 1e-3);
        let mut bar = Bar { animate: true, toggled: Some(Instant::now()), ..Bar::default() };
        assert!(bar.hidden_at(Instant::now() + Duration::from_secs(3)) > 0.999);
        bar.shown = j(1);
        bar.from = 1.;
        let at = bar.toggled.unwrap_or_else(Instant::now);
        assert!((bar.hidden_at(at + Duration::from_millis(90)) - 1.).abs() < 1e-6);
    }
}

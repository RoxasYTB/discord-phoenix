use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use gpui::{
    Animation, AnimationExt, AnchoredPositionMode, AnyElement, App, Context, ElementId, Entity, FontWeight,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, anchored, deferred, div,
    ease_out_quint, img, point, prelude::*, px, svg,
};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    model::{Guild, Id, ImageRef},
    theme::{Palette, c, m, z},
    ui::{badge, icon},
};

const FOLDER_DEFAULT: u32 = 0x5865F2;

enum RailItem<'a> {
    Guild(&'a Guild, bool),
    Folder { id: u64, name: SharedString, color: u32, guilds: Vec<&'a Guild>, open: bool },
}

fn rail(app: &PhoenixApp) -> Vec<RailItem<'_>> {
    let by_id = |g: Id| app.guilds.iter().find(|x| x.id == g);
    let in_folders: std::collections::HashSet<Id> =
        app.nav.folders.iter().flat_map(|f| f.guilds.iter().copied()).collect();
    let mut out: Vec<RailItem> =
        app.guilds.iter().filter(|g| !in_folders.contains(&g.id)).map(|g| RailItem::Guild(g, false)).collect();
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
            _ => out.extend(guilds.into_iter().map(|g| RailItem::Guild(g, false))),
        }
    }
    out
}

pub fn flat_guild_order(app: &PhoenixApp) -> Vec<Id> {
    rail(app)
        .into_iter()
        .flat_map(|item| match item {
            RailItem::Guild(g, _) => vec![g.id],
            RailItem::Folder { guilds, .. } => guilds.iter().map(|g| g.id).collect(),
        })
        .collect()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
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
    let dms = unread_dm_items(app, &entity);
    let base = 2 + dms.len();
    col = col.children(dms).child(separator());
    let mut spans: Vec<Span> = Vec::new();
    let is_unread = |g: &Guild| g.unread || g.mentions > 0;
    for (i, item) in rail(app).into_iter().enumerate() {
        match &item {
            RailItem::Guild(g, _) if is_unread(g) => spans.push(Span { ix: base + i, y: 0., h: ICON, mentions: g.mentions }),
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
            RailItem::Guild(g, nested) => col.child(guild(app, &entity, g, nested)),
            RailItem::Folder { id, name, color, guilds, open } => {
                col.child(folder(app, &entity, id, name, color, &guilds, open))
            }
        };
    }
    let e = entity.clone();
    col = col
        .child(separator())
        .child(
            action(&entity, "add-server", Icon::Plus, crate::t!("rail.add_server"))
                .on_click(move |_, w, cx| e.update(cx, |a, cx| a.open_join_server(w, cx))),
        )
        .child({
            let e = entity.clone();
            action(&entity, "discover", Icon::Compass, crate::t!("rail.discover"))
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.stub(&crate::t!("rail.discover"), cx)))
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

thread_local! {
    static HOVERED: Cell<Option<u64>> = const { Cell::new(None) };
    static PILLS: RefCell<HashMap<u64, (f32, f32, u32)>> = RefCell::new(HashMap::new());
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
        .with_animation("rail-tip-zoom", crate::ui::states::tip_zoom(), |d, t| crate::ui::states::zoom_card_sized(d, t, 16., 20.));
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
                .child(card)
                .with_animation(
                    "rail-tip",
                    Animation::new(crate::ui::settings::effects::motion(100)).with_easing(ease_out_quint()),
                    |d, t| d.ml(z(4. * (1. - t))),
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
    let (from, to, epoch) = PILLS.with(|p| {
        let mut p = p.borrow_mut();
        let e = p.entry(key).or_insert((target, target, 0));
        if e.1 != target {
            *e = (e.1, target, e.2.wrapping_add(1));
        }
        *e
    });
    let bar = div().w(z(4.)).rounded_r(z(4.)).bg(c(Palette::TEXT_HEADER));
    let bar: AnyElement = if from == to {
        bar.h(z(to)).into_any_element()
    } else {
        bar.with_animation(
            ElementId::Name(format!("pill-{key}-{epoch}").into()),
            Animation::new(crate::ui::settings::effects::motion(PILL_MS)).with_easing(ease_out_quint()),
            move |d, t| d.h(z(from + (to - from) * t)),
        )
        .into_any_element()
    };
    div().absolute().left_0().top_0().h_full().w(z(4.)).flex().items_center().child(bar).into_any_element()
}

fn mention_badge(n: u32) -> impl IntoElement {
    div()
        .absolute()
        .right(z(-4.))
        .bottom(z(-4.))
        .p(z(4.))
        .rounded(z(12.))
        .bg(c(Palette::BG_TERTIARY))
        .child(badge(n).bg(c(Palette::NOTIFICATION)))
}

fn voice_badge(app: &PhoenixApp, g: &Guild) -> Option<AnyElement> {
    use crate::theme::{generated::idx, tok};
    let chans = app.guild_channels.get(&g.id)?;
    let connected = app.voice_channel.is_some_and(|v| chans.contains(&v));
    let busy = connected || chans.iter().filter_map(|c| app.channels.get(c)).any(|c| !c.voice_users.is_empty());
    if !busy {
        return None;
    }
    if !connected && app.backend.notify_prefs().is_some_and(|p| p.is_guild_muted(Some(g.id.0))) {
        return None;
    }
    let bg = tok(if connected { idx::STATUS_POSITIVE } else { idx::ICON_OVERLAY_DARK });
    Some(
        div()
            .absolute()
            .top(z(-2.))
            .right(z(-2.))
            .p(z(2.))
            .rounded_full()
            .bg(c(Palette::BG_TERTIARY))
            .child(
                div().size(z(16.)).rounded_full().bg(c(bg)).flex().items_center().justify_center().child(
                    gpui::svg().path(crate::icons::path("speaker")).size(z(12.)).text_color(gpui::white()),
                ),
            )
            .into_any_element(),
    )
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

fn unread_dm_items(app: &PhoenixApp, entity: &Entity<PhoenixApp>) -> Vec<AnyElement> {
    app.dms
        .iter()
        .filter_map(|d| app.channels.get(d))
        .filter(|ch| ch.mentions > 0 && !ch.muted && !app.nav.message_requests.contains(&ch.id))
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
                .child(pill(key, false, hover, true))
                .child(
                    div()
                        .relative()
                        .size(z(ICON))
                        .child(crate::ui::avatar(u, ICON))
                        .child(mention_badge(ch.mentions)),
                )
                .when(hover, |d| d.child(right_tooltip(name, ICON)))
                .into_any_element()
        })
        .collect()
}
const ADD_KEY: u64 = 2;
const DISCOVER_KEY: u64 = 3;

fn home(app: &PhoenixApp, entity: &Entity<PhoenixApp>) -> AnyElement {
    let selected = app.selected_guild.is_none();
    let hover = hovered(HOME_KEY);
    let pending = app.nav.count(crate::ui::home::data::RelationshipKind::Incoming) as u32;
    let e = entity.clone();
    row("home", HOME_KEY, entity)
        .on_click(move |_, _, cx| e.update(cx, |a, cx| a.select_guild(None, cx)))
        .child(pill(HOME_KEY, selected, hover, false))
        .child(
            div()
                .relative()
                .size(z(ICON))
                .child(
                    div()
                        .size_full()
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
                )
                .when(pending > 0, |d| d.child(mention_badge(pending))),
        )
        .when(hover, |d| d.child(right_tooltip(crate::t!("dm.title"), ICON)))
        .into_any_element()
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
                    0..=4 => 16.,
                    5 => 14.,
                    _ => 12.,
                }
            }))
            .font_weight(FontWeight::MEDIUM)
            .child(g.acronym.clone())
            .into_any_element(),
    }
}

#[derive(Clone)]
pub struct DraggedGuild {
    pub id: Id,
    icon: ImageRef,
    acronym: SharedString,
}

impl Render for DraggedGuild {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
    pub fn move_guild(&mut self, dragged: Id, target: Id, cx: &mut Context<Self>) {
        if dragged == target {
            return;
        }
        if let Some(from) = self.guilds.iter().position(|g| g.id == dragged) {
            let g = self.guilds.remove(from);
            let to = self.guilds.iter().position(|g| g.id == target).unwrap_or(self.guilds.len());
            self.guilds.insert(to, g);
        }
        for f in &mut self.nav.folders {
            f.guilds.retain(|g| *g != dragged);
        }
        if let Some(f) = self.nav.folders.iter_mut().find(|f| f.guilds.contains(&target)) {
            let at = f.guilds.iter().position(|g| *g == target).unwrap_or(0);
            f.guilds.insert(at, dragged);
        }
        self.nav.folders.retain(|f| !f.guilds.is_empty());
        self.sync_guild_folders(cx);
        cx.notify();
    }
}

fn guild(app: &PhoenixApp, entity: &Entity<PhoenixApp>, g: &Guild, _nested: bool) -> AnyElement {
    let id = g.id;
    let selected = app.selected_guild == Some(id);
    let hover = hovered(id.0);
    let has_icon = matches!(g.icon, ImageRef::Url(_));
    let active = selected || hover;
    let e = entity.clone();
    let drop_e = entity.clone();
    let drag = DraggedGuild { id, icon: g.icon.clone(), acronym: g.acronym.clone() };
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
    row(("guild", id.0), id.0, entity)
        .on_click(move |_, _, cx| e.update(cx, |app, cx| app.select_guild(Some(id), cx)))
        .on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| {
            crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Guild(id), ev.position, w)
        })
        .on_drag(drag, |d: &DraggedGuild, _, _, cx: &mut App| cx.new(|_| d.clone()))
        .drag_over::<DraggedGuild>(|s, _, _, _| s.border_t_2().border_color(c(Palette::GREEN)))
        .on_drop(move |d: &DraggedGuild, _, cx| drop_e.update(cx, |a, cx| a.move_guild(d.id, id, cx)))
        .child(pill(id.0, selected, hover, g.unread))
        .child(
            div()
                .relative()
                .size(z(ICON))
                .child(icon_box)
                .children(voice_badge(app, g))
                .when(g.mentions > 0, |d| d.child(mention_badge(g.mentions))),
        )
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
    let has_selected = guilds.iter().any(|g| Some(g.id) == app.selected_guild);
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
    let header = row(("folder", id), key, entity)
        .h(z(FOLDER))
        .on_click(move |_, _, cx| e.update(cx, |a, cx| a.toggle_folder(id, cx)))
        .on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| {
            crate::ui::nav_menus::open(crate::ui::nav_menus::Target::Folder(id), ev.position, w)
        })
        .child(pill(key, !open && has_selected, hover, !open && unread))
        .child(
            div()
                .relative()
                .size(z(FOLDER))
                .child(head_inner)
                .when(!open && mentions > 0, |d| d.child(mention_badge(mentions))),
        )
        .when(hover, |d| d.child(right_tooltip(name.clone(), FOLDER)));
    if !open {
        return header.into_any_element();
    }
    let mut body =
        div().w_full().mt(z(gap() - FOLDER_PAD)).flex().flex_col().gap(z(gap())).pb(z(FOLDER_PAD));
    for g in guilds {
        body = body.child(guild(app, entity, g, true));
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
    row(id, key, entity)
        .child(pill(key, false, hover, false))
        .child(
            div()
                .size(z(ICON))
                .rounded(z(ICON_RADIUS))
                .relative()
                .child(crate::ui::states::fade_to(
                    crate::ui::states::key(("rail-tile", key)),
                    hover,
                    c(Palette::GUILD_ICON_BG),
                    c(Palette::BLURPLE),
                    150,
                    ICON_RADIUS,
                ))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(i, 20. * crate::theme::zoom(), c(Palette::TEXT_NORMAL))),
        )
        .when(hover, |d| d.child(right_tooltip(label, ICON)))
}

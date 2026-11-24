//! 80px guild rail (40px rounded-square tiles, animated pill, name tooltips): Home, guilds and guild folders (from `guild_folders`),
//! Add a Server (join by invite) and Discover. Guilds can be dragged to
//! reorder them locally (not synced to Discord yet).

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

/// Folder colour when none is set (Discord's blurple-ish default).
const FOLDER_DEFAULT: u32 = 0x5865F2;

enum RailItem<'a> {
    Guild(&'a Guild, bool),
    Folder { id: u64, name: SharedString, color: u32, guilds: Vec<&'a Guild>, open: bool },
}

/// Guilds grouped by the user's folders. Guilds not in any folder (joined
/// since the settings were saved) come first, like the official client.
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
    // `stack_TWWX0S` "Direct messages" (vanilla-dm-profile-panel #69): one 40px
    // `listItem_` per DM with unread mentions, same 10px rail gap.
    let dms = unread_dm_items(app, &entity);
    let base = 2 + dms.len();
    col = col.children(dms).child(separator());
    // Unread guilds for the "nouveau" bars: (rail child index, y offset in that
    // child, mentions). Guilds of an open folder are checked one by one, like
    // the official client which tests each guild row, not the folder.
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
                // Header (48) + `folderGuildsList_` margin-top (GAP - FOLDER_PAD), then 40px rows 10px apart.
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
        .child(col)
        .children(above.map(|ix| unread_bar(&scroll, ix, true, m_above)))
        .children(below.map(|ix| unread_bar(&scroll, ix, false, m_below)))
}

thread_local! {
    static RAIL_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

/// Scrolls the server rail to `y` px from the top (offscreen compares copy the
/// client's `scroller_5dqHTG` scrollTop).
#[doc(hidden)]
pub fn scroll_rail_to(y: f32) {
    RAIL_SCROLL.with(|h| h.set_offset(gpui::point(gpui::px(0.), gpui::px(-y.max(0.)))));
}

/// One unread guild row inside a rail child (see `render`).
struct Span {
    ix: usize,
    y: f32,
    h: f32,
    mentions: u32,
}

/// The official rail scroller (`wrapper_5dqHTG`) ends 16px under the panels card top
/// while ours stops 10px above it, so a row in that 26px band still counts as visible
/// (live 2026-11-24: bottom bar `hidden_oJAkDB` with the next unread guild peeking).
const RAIL_UNDER_PANELS: f32 = 26.;

/// Nearest unread row scrolled off above / below the rail viewport (last
/// frame's layout) and the mentions hidden on each side.
fn offscreen_unread(h: &gpui::ScrollHandle, spans: &[Span]) -> (Option<usize>, Option<usize>, u32, u32) {
    let view = h.bounds();
    let (mut above, mut below, mut ma, mut mb) = (None, None, 0, 0);
    if view.size.height <= px(0.) {
        return (above, below, ma, mb);
    }
    // Client guild bar: `isUnread: () => false`, `isMentioned: guild
    // highImportanceMentionCount > 0` — only offscreen mentions raise the bar,
    // a merely unread guild below the fold does not.
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

/// "nouveau" pill. vanilla-guild-text #56/#57 (top) and #607/#608 (bottom):
/// unreadMentionsIndicatorTop_/Bottom_ absolute, padding 8 (bottom: 8 8 26);
/// bar_oJAkDB unread_oJAkDB: h 24, r 12, --background-surface-highest, 1px --border-normal,
/// shadow 0 2 6 --opacity-black-24, 12px/16px semibold uppercase --text-strong; :active top 1px.
/// With mentions offscreen it becomes the red `mention_oJAkDB` variant: background
/// --background-feedback-notification, 1px --border-subtle, --shadow-medium,
/// --text-overlay-light; text intl `A2j16v` "{count} mentions" (uppercased by bar_).
fn unread_bar(h: &gpui::ScrollHandle, ix: usize, top: bool, mentions: u32) -> AnyElement {
    use crate::theme::{generated::idx, tok};
    let h = h.clone();
    div()
        .absolute()
        .left_0()
        .right_0()
        .when(top, |d| d.top_0().p(z(8.)))
        // Bottom: 8 8 26 inside `wrapper_5dqHTG`, whose margin-bottom
        // (`--custom-app-panels-height + --space-xs - 16px`) makes it end 16px
        // under the panels card top. Our rail stops 10px above the card instead,
        // so the bar lands at the same place with no bottom padding.
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
                // vanilla-dm #590/#591: the red mentions bar still reads "nouveau".
                .child(crate::t!("rail.unread_new").to_uppercase()),
        )
        .into_any_element()
}

// Metrics of the official guild rail (refresh layout, `.density-cozy`, dump -channels-N-N+ #50..#583).
/// `--guildbar-avatar-size`.
const ICON: f32 = 40.;
/// `--guildbar-folder-size`.
const FOLDER: f32 = 48.;
/// `--radius-md`: guild icons are rounded squares.
const ICON_RADIUS: f32 = 12.;
/// `--radius-lg`: open folder background.
const FOLDER_RADIUS: f32 = 16.;
/// `--space-xs` (cozy 10, default 8, compact 6): `stack_TWWX0S scroller_5dqHTG` gap.
/// Dump -channels-N-N+ #55 gap 10px.
fn gap() -> f32 {
    crate::theme::space::xs()
}
/// `scroller_5dqHTG` padding-top is 0 on `.platform-linux` (dump #55 starts at y=32).
const PAD_TOP: f32 = 0.;
/// `scroller_5dqHTG` padding-bottom: `calc(var(--space-xs) + 16px)` = 26px (dump #55).
fn pad_bottom() -> f32 {
    gap() + 16.
}
/// `--custom-folder-padding`: `(--guildbar-folder-size - --guildbar-avatar-size) / 2`.
const FOLDER_PAD: f32 = (FOLDER - ICON) / 2.;
/// `--custom-folder-preview-padding` (`--space-4`).
const PREVIEW_PAD: f32 = 4.;
/// `--custom-folder-preview-gap`.
const PREVIEW_GAP: f32 = 2.;
/// `item__o8_Xq` transition: height/opacity/transform .2s ease-out.
const PILL_MS: u64 = 200;

thread_local! {
    /// Rail item currently under the mouse (drives the pill and the tooltip).
    static HOVERED: Cell<Option<u64>> = const { Cell::new(None) };
    /// Last pill target height per rail item, so a change animates from it.
    static PILLS: RefCell<HashMap<u64, (f32, f32, u32)>> = RefCell::new(HashMap::new());
}

fn hovered(key: u64) -> bool {
    HOVERED.with(|h| h.get() == Some(key))
}

/// Make `el` update the shared hover state (and re-render) on enter/leave.
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

/// Server-name tooltip to the right of the rail, like the official client
/// (surface-high card, 1px subtle border, 8px radius, 8/12 padding, pointer).
pub fn right_tooltip(text: SharedString, item_h: f32) -> AnyElement {
    let card = div()
        .max_w(z(196.))
        .px(z(12.))
        .py(z(8.))
        .rounded(z(8.))
        .bg(c(crate::theme::tok(crate::theme::generated::idx::BACKGROUND_SURFACE_HIGH))) // tooltip_ZDdyhE
        .border_1()
        .border_color(c(crate::theme::tok(crate::theme::generated::idx::BORDER_SUBTLE))) // inset 1px --border-subtle
        .shadow_lg()
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(Palette::TEXT_NORMAL))
        .child(text)
        .with_animation("rail-tip-zoom", crate::ui::states::tip_zoom(), |d, t| crate::ui::states::zoom_card_sized(d, t, 16., 20.));
    // Left-pointing 5px triangle, drawn as stacked 1px slices.
    let mut pointer = div().flex().flex_col().justify_center().items_end().w(z(5.));
    for i in 0..9 {
        let w = 5. - (4. - i as f32).abs();
        pointer = pointer.child(div().w(z(w)).h(z(1.)).bg(c(crate::theme::tok(crate::theme::generated::idx::BACKGROUND_SURFACE_HIGH))));
    }
    let tip = deferred(
        // vanilla-tooltip-guild #2089: the card starts 12px right of the 40px tile (x 72 for
        // a tile at 20..60), with the 5px caret in that gap.
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
                    // ref-cache tooltip-rail-appear.json: shown ~60ms after hover, opacity 1,
                    // card slides 4px right-to-left (x 76 → 72) over ~100ms.
                    |d, t| d.ml(z(4. * (1. - t))),
                ),
        ),
    )
    .with_priority(2);
    div().absolute().top_0().left_0().child(tip).into_any_element()
}

/// Pill on the left edge: 40px selected, 20px hovered, 8px unread.
/// `--text-strong`, 8px wide with half of it off-screen, 0 4 4 0 radius.
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

/// Red count badge cut out of the rail background (bottom-right of the icon).
fn mention_badge(n: u32) -> impl IntoElement {
    div()
        .absolute()
        .right(z(-4.)) // lowerBadge_hqZPsU bottom/right 0 + 4px cut-out ring
        .bottom(z(-4.))
        .p(z(4.))
        .rounded(z(12.))
        .bg(c(Palette::BG_TERTIARY))
        .child(badge(n).bg(c(Palette::NOTIFICATION)))
}

/// `upperBadge_hqZPsU` voice badge (GuildMediaStateStore `audio`): someone is
/// in one of the guild's voice channels. Hidden for muted guilds unless we are
/// connected there; `--status-positive` when connected, else `--icon-overlay-dark`
/// (`iconBadge_rwEpIL`). 16px round, 2px icon padding (`icon_lMvXrQ`), 2px
/// cut-out ring (`badgeMaskStroke` 2).
fn voice_badge(app: &PhoenixApp, g: &Guild) -> Option<AnyElement> {
    use crate::theme::{generated::idx, tok};
    let chans = app.guild_channels.get(&g.id)?;
    let connected = app.voice_channel.is_some_and(|v| chans.contains(&v));
    let busy = connected || chans.iter().filter_map(|c| app.channels.get(c)).any(|c| !c.voice_users.is_empty());
    if !busy {
        return None;
    }
    if !connected && app.backend.notify_prefs().and_then(|p| p.guilds.get(&g.id.0).map(|x| x.muted)).unwrap_or(false) {
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

/// One 72px-wide rail row holding a centred 40px tile.
fn row(id: impl Into<ElementId>, key: u64, entity: &Entity<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let row = track_hover(
        div().id(id).w_full().h(z(ICON)).flex_none().flex().justify_center().relative().cursor_pointer(),
        key,
        entity,
    );
    // Tab focus ring around the centred 40px tile (16px in from the 72px row), tile radius 12.
    crate::ui::focus_ring::ring_inset(row, crate::ui::states::key(("rail-row", key)), ICON_RADIUS, (m::SERVER_BAR_W - ICON) / 2., 0.)
}

const HOME_KEY: u64 = 1;

/// Unread DM rail items (vanilla-dm-profile-panel #71..#78): unread pill
/// (`item__o8_Xq visible`), 40px round avatar, `lowerBadge_hqZPsU` mention count.
fn unread_dm_items(app: &PhoenixApp, entity: &Entity<PhoenixApp>) -> Vec<AnyElement> {
    app.dms
        .iter()
        .filter_map(|d| app.channels.get(d))
        // Muted DMs and message requests never surface in the rail (they stay
        // in Home: Message Requests / the muted DM row).
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
    // Unread DMs get their own rail items (`unread_dm_items`): the Home badge only
    // counts pending friend requests.
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
                        // childWrapper_JJ-hgR transition: background-color .15s ease-out
                        .child(crate::ui::states::fade_to(
                            crate::ui::states::key(("rail-tile", HOME_KEY)),
                            selected || hover,
                            c(Palette::GUILD_ICON_BG),
                            c(Palette::BLURPLE),
                            150,
                            ICON_RADIUS,
                        ))
                        // Live (dpr 1.25): the 24x24 glyph sits 8px inside the 40px tile.
                        .child(svg().path(crate::icons::path("discord-logo")).size(z(24.)).flex_none().text_color(if selected || hover { gpui::white() } else { c(Palette::TEXT_NORMAL) })),
                )
                .when(pending > 0, |d| d.child(mention_badge(pending))),
        )
        .when(hover, |d| d.child(right_tooltip(crate::t!("dm.title"), ICON)))
        .into_any_element()
}

fn guild_inner(g: &Guild, size: f32) -> AnyElement {
    match &g.icon {
        // The tile's `--radius-md` must clip the image itself (GPUI doesn't clip children).
        ImageRef::Url(u) => img(u.clone()).size(z(size)).rounded(z(ICON_RADIUS * size / ICON)).into_any_element(),
        _ => div()
            // live client acronym_JJ-hgR (inline size): "T-B", "Tdg", "LRèr" 16px,
            // a 6-character acronym 12px (5 characters assumed 14px).
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

/// Dragged guild + its preview.
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
    /// Local reorder stub: move `dragged` just before `target` (and into the
    /// target's folder). Not yet persisted to Discord's settings.
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
        // Icon guilds have no tile behind transparent pixels (vanilla rail).
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
        // Open folder (`folderIconWrapper_RTMSuZ` padding `--custom-folder-padding`):
        // folder glyph in `--custom-folder-color` on a `--guildbar-avatar-size`
        // tile, `--radius-md`, `--background-mod-subtle` on hover.
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
                    // folderIcon_RTMSuZ: 18px glyph (24-unit folder drawn at 0.75):
                    // 15x15 ink box, tab 7.5 wide, body from 2.25 down, radius 2.25.
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
        // Closed folder (`folderPreviewWrapper_RTMSuZ`): `--guildbar-folder-size`
        // tile, `--radius-lg`, folder colour mixed 60 % transparent, 4px padding;
        // 2x2 preview (`folderPreviewGuildIcon_RTMSuZ`) with 2px gap and
        // 13px outer / 4px inner corner radii.
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
    // `folderHeader_RTMSuZ` / `folderButton_RTMSuZ`: 48x48.
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
    // `folderGuildsList_RTMSuZ>:first-child` margin-top: `--space-xs - --custom-folder-padding`.
    let mut body =
        div().w_full().mt(z(gap() - FOLDER_PAD)).flex().flex_col().gap(z(gap())).pb(z(FOLDER_PAD));
    for g in guilds {
        body = body.child(guild(app, entity, g, true));
    }
    div()
        .w_full()
        .relative()
        .child(
            // `folderGroupBackground_RTMSuZ`: `--guildbar-folder-size` wide,
            // `--radius-lg`, 1px `--border-muted`, folder colour mixed 85 %
            // transparent, inset `--custom-guild-list-padding - --custom-folder-padding`.
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

/// 32x1 `--app-frame-border` line between Home and the guilds.
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

/// Add a Server / Discover tiles (`circleIconButton_g8YUxD`): `--background-mod-subtle`
/// tile, `--text-default` glyph, `--background-brand` background on hover.
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
                // circleIconButton_g8YUxD transition: background-color .15s ease-out
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
                .child(icon(i, 20. * crate::theme::zoom(), c(Palette::TEXT_NORMAL))), // circleIcon_g8YUxD 20px, --text-default
        )
        .when(hover, |d| d.child(right_tooltip(label, ICON)))
}

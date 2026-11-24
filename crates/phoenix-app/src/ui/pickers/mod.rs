pub mod emoji;
#[rustfmt::skip]
pub mod emoji_data;
mod gif;
pub mod grid;
mod prefs;
pub mod soundboard;
pub(crate) mod sticker;
pub mod types;

use std::time::Duration;

use gpui::{
    AnyElement, App, Context, Entity, FontWeight, Keystroke, ListState, ParentElement, Pixels,
    Point, SharedString, Styled, Subscription, WeakEntity, Window, div, prelude::*, px,
};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    model::Id,
    theme::c,
    ui::{self, icon},
};

pub use grid::{Grid, PickItem};
pub use prefs::Prefs;
use types::{GifHome, GifItem, StickerPackData};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmojiTarget {
    Composer,
    Reaction { channel: Id, message: Id },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Gif,
    Sticker,
    Emoji,
}

pub struct OpenPicker {
    pub tab: Tab,
    pub target: EmojiTarget,
    pub input: Entity<InputState>,
    pub grid: Grid,
    pub list: ListState,
    pub selected: usize,
    pub hovered: Option<usize>,
    pub tone_open: bool,
    pub anchor: Option<Point<Pixels>>,
    _sub: Subscription,
}

#[derive(Default)]
pub struct GifState {
    pub home: Option<GifHome>,
    pub home_loading: bool,
    pub shown: Option<String>,
    pub results: Vec<GifItem>,
    pub loading: bool,
    pub error: Option<SharedString>,
    seq: u64,
}

#[derive(Default)]
pub struct StickerState {
    pub packs: Option<Vec<StickerPackData>>,
    pub loading: bool,
    pub inspected: Option<(types::StickerData, SharedString)>,
}

pub struct Pickers {
    pub app: WeakEntity<PhoenixApp>,
    pub open: Option<OpenPicker>,
    pub prefs: Prefs,
    pub gifs: GifState,
    pub stickers: StickerState,
    pub soundboard: soundboard::SoundboardState,
    pub frecency: Option<types::Frecency>,
    pub frecency_loading: bool,
}

impl gpui::Global for Pickers {}

pub fn install(composer: &Entity<gpui_component::input::TextareaState>, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Vec<Subscription> {
    let weak = cx.entity().downgrade();
    cx.set_global(Pickers {
        app: weak.clone(),
        open: None,
        prefs: Prefs::load(),
        gifs: GifState::default(),
        stickers: StickerState::default(),
        soundboard: soundboard::SoundboardState::default(),
        frecency: None,
        frecency_loading: false,
    });
    let keys = cx.intercept_keystrokes(move |ev, window, cx| {
        let Some(app) = weak.upgrade() else { return };
        app.update(cx, |app, cx| {
            if route_key(app, &ev.keystroke, window, cx) {
                cx.stop_propagation();
            }
        });
    });
    let ac = cx.subscribe_in(composer, window, |app, _, ev: &InputEvent, window, cx| {
        if let InputEvent::Change = ev {
            ui::autocomplete::refresh(app, window, cx);
        }
    });
    vec![keys, ac]
}

fn route_key(app: &mut PhoenixApp, ks: &Keystroke, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    let m = &ks.modifiers;
    let plain = !m.control && !m.alt && !m.platform;
    if m.control && !m.shift && ks.key == "f" && app.screen == crate::app::Screen::Main {
        ui::search::open(app, window, cx);
        return true;
    }
    if m.control && !m.shift && ks.key == "e" && app.screen == crate::app::Screen::Main {
        toggle(app, Tab::Emoji, EmojiTarget::Composer, window, cx);
        return true;
    }
    if m.control && !m.shift && ks.key == "g" && app.screen == crate::app::Screen::Main {
        toggle(app, Tab::Gif, EmojiTarget::Composer, window, cx);
        return true;
    }
    if is_open(cx) {
        return plain && picker_key(app, &ks.key, m.shift, window, cx);
    }
    if app.switcher.is_some() {
        return plain && ui::switcher::on_key(app, &ks.key, window, cx);
    }
    if ui::search::is_focused(window, cx) {
        return plain && ui::search::on_key(app, &ks.key, window, cx);
    }
    plain && ui::autocomplete::on_key(app, &ks.key, m.shift, window, cx)
}

pub fn is_open(cx: &App) -> bool {
    cx.try_global::<Pickers>().is_some_and(|p| p.open.is_some())
}

pub fn open_tab(cx: &App) -> Option<(Tab, EmojiTarget)> {
    cx.try_global::<Pickers>()?.open.as_ref().map(|o| (o.tab, o.target))
}

pub fn open_emoji_picker(app: &mut PhoenixApp, target: EmojiTarget, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    open(app, Tab::Emoji, target, window, cx);
}

pub fn toggle(app: &mut PhoenixApp, tab: Tab, target: EmojiTarget, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    match open_tab(cx) {
        Some((t, tg)) if t == tab && tg == target => close(app, window, cx),
        Some((_, tg)) if tg == target => switch_tab(app, tab, window, cx),
        _ => open(app, tab, target, window, cx),
    }
}

pub fn open(app: &mut PhoenixApp, tab: Tab, target: EmojiTarget, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if app.screen != crate::app::Screen::Main {
        return;
    }
    ui::autocomplete::dismiss(cx);
    let tab = if matches!(target, EmojiTarget::Reaction { .. }) { Tab::Emoji } else { tab };
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder(tab)));
    let sub = cx.subscribe_in(&input, window, |app, _, ev: &InputEvent, window, cx| match ev {
        InputEvent::Change => on_query(app, cx),
        InputEvent::PressEnter { shift, .. } => pick_selected(app, *shift, window, cx),
        _ => {}
    });
    input.update(cx, |s, cx| s.focus(window, cx));
    let anchor = matches!(target, EmojiTarget::Reaction { .. }).then(|| window.mouse_position());
    let grid = grid::build(app, "", cx);
    let list = ListState::new(grid.rows.len(), gpui::ListAlignment::Top, px(400.));
    cx.global_mut::<Pickers>().open = Some(OpenPicker {
        tab,
        target,
        input,
        grid,
        list,
        selected: 0,
        hovered: None,
        tone_open: false,
        anchor,
        _sub: sub,
    });
    ensure_tab_data(app, tab, cx);
    cx.notify();
}

fn placeholder(tab: Tab) -> SharedString {
    match tab {
        Tab::Gif => crate::t!("picker.search_klipy"),
        Tab::Sticker => crate::t!("picker.search_stickers"),
        Tab::Emoji => crate::t!("picker.search_emoji"),
    }
}

fn switch_tab(app: &mut PhoenixApp, tab: Tab, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(op) = cx.global_mut::<Pickers>().open.as_mut() else { return };
    op.tab = tab;
    op.selected = 0;
    op.hovered = None;
    op.tone_open = false;
    let input = op.input.clone();
    input.update(cx, |s, cx| {
        s.set_value("", window, cx);
        s.set_placeholder(placeholder(tab), window, cx);
        s.focus(window, cx);
    });
    ensure_tab_data(app, tab, cx);
    cx.notify();
}

pub fn close(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    app.chat.emoji_target = None;
    if cx.global_mut::<Pickers>().open.take().is_some() {
        app.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }
}

fn query(cx: &App) -> String {
    cx.global::<Pickers>().open.as_ref().map(|o| o.input.read(cx).value().to_string()).unwrap_or_default()
}

fn on_query(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some((tab, _)) = open_tab(cx) else { return };
    let q = query(cx);
    match tab {
        Tab::Emoji => {
            let grid = grid::build(app, &q, cx);
            if let Some(op) = cx.global_mut::<Pickers>().open.as_mut() {
                op.list.reset(grid.rows.len());
                op.grid = grid;
                op.selected = 0;
                op.hovered = None;
            }
        }
        Tab::Gif => gif::search_debounced(app, q, cx),
        Tab::Sticker => {}
    }
    cx.notify();
}

pub fn load_frecency(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let p = cx.global_mut::<Pickers>();
    if p.frecency.is_some() || p.frecency_loading {
        return;
    }
    p.frecency_loading = true;
    let fut = app.backend.expression_frecency();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |app, cx| {
            let p = cx.global_mut::<Pickers>();
            p.frecency_loading = false;
            p.frecency = Some(res.unwrap_or_default());
            if matches!(open_tab(cx), Some((Tab::Emoji, _))) {
                on_query(app, cx);
            }
            cx.notify();
        });
    })
    .detach();
}

fn ensure_tab_data(app: &mut PhoenixApp, tab: Tab, cx: &mut Context<PhoenixApp>) {
    load_frecency(app, cx);
    match tab {
        Tab::Gif => gif::load_home(app, cx),
        Tab::Sticker => sticker::load_packs(app, cx),
        Tab::Emoji => {}
    }
}

fn picker_key(app: &mut PhoenixApp, key: &str, shift: bool, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    if key == "escape" {
        close(app, window, cx);
        return true;
    }
    let Some(op) = cx.global_mut::<Pickers>().open.as_mut() else { return false };
    if op.tab != Tab::Emoji {
        return false;
    }
    let n = op.grid.items.len();
    if n == 0 {
        return false;
    }
    let next = match key {
        "left" => op.selected.saturating_sub(1),
        "right" => (op.selected + 1).min(n - 1),
        "up" => op.grid.vertical(op.selected, false),
        "down" => op.grid.vertical(op.selected, true),
        "enter" => {
            pick_selected(app, shift, window, cx);
            return true;
        }
        _ => return false,
    };
    op.selected = next;
    op.hovered = None;
    if let Some(row) = op.grid.row_of(next) {
        op.list.scroll_to_reveal_item(row);
    }
    cx.notify();
    true
}

fn pick_selected(app: &mut PhoenixApp, keep_open: bool, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(op) = cx.global::<Pickers>().open.as_ref() else { return };
    if op.tab != Tab::Emoji {
        return;
    }
    if let Some(item) = op.grid.items.get(op.selected).cloned() {
        pick(app, item, keep_open, window, cx);
    }
}

pub fn pick(app: &mut PhoenixApp, item: PickItem, keep_open: bool, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some((_, target)) = open_tab(cx) else { return };
    let tone = cx.global::<Pickers>().prefs.tone;
    cx.global_mut::<Pickers>().prefs.record(&item);
    match target {
        EmojiTarget::Composer => {
            let text = match &item {
                PickItem::Unicode(ix) => emoji::toned(*ix, tone),
                PickItem::Custom(e) => e.token(),
            };
            app.composer.update(cx, |s, cx| s.insert(text, window, cx));
            if !keep_open {
                close(app, window, cx);
            }
        }
        EmojiTarget::Reaction { channel, message } => {
            let (emoji, id) = match &item {
                PickItem::Unicode(ix) => (emoji::toned(*ix, tone), None),
                PickItem::Custom(e) => (e.token(), Some(e.id)),
            };
            app.chat.emoji_target = Some(crate::ui::chat::state::EmojiTarget::Reaction { channel, msg: message });
            app.emoji_picked(emoji.into(), id, window, cx);
            if !keep_open {
                close(app, window, cx);
            } else {
                app.chat.emoji_target = Some(crate::ui::chat::state::EmojiTarget::Reaction { channel, msg: message });
            }
        }
    }
    cx.notify();
}

pub(crate) fn detach<T: Send + 'static>(fut: crate::backend::BoxFuture<T>, cx: &mut App) {
    cx.background_spawn(async move {
        let _ = fut.await;
    })
    .detach();
}

pub(crate) fn with_app(cx: &mut App, window: &mut Window, f: impl FnOnce(&mut PhoenixApp, &mut Window, &mut Context<PhoenixApp>)) {
    let Some(app) = cx.try_global::<Pickers>().and_then(|p| p.app.upgrade()) else { return };
    app.update(cx, |a, cx| f(a, window, cx));
}

pub(crate) fn debounce(cx: &mut Context<PhoenixApp>, ms: u64, f: impl FnOnce(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static) {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(Duration::from_millis(ms)).await;
        let _ = this.update(cx, f);
    })
    .detach();
}

pub fn render_overlays(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Vec<AnyElement> {
    let mut out = Vec::new();
    if let Some(t) = app.chat.emoji_target
        && !is_open(cx)
    {
        let target = match t {
            crate::ui::chat::state::EmojiTarget::Composer => EmojiTarget::Composer,
            crate::ui::chat::state::EmojiTarget::Reaction { channel, msg } => EmojiTarget::Reaction { channel, message: msg },
        };
        cx.defer_in(window, move |app, window, cx| open(app, Tab::Emoji, target, window, cx));
    }
    if let Some(el) = ui::search::render(app, window, cx) {
        out.push(el);
    }
    if let Some(el) = render_picker(app, window, cx) {
        out.push(el);
    }
    if let Some(el) = soundboard::render(app, window, cx) {
        out.push(el);
    }
    out
}

pub const PICKER_W: f32 = 498.;
pub const PICKER_H: f32 = 486.;
pub const PICKER_H_NO_TABS: f32 = 440.;

pub(crate) fn tk(i: usize) -> gpui::Hsla {
    c(crate::theme::tok(i))
}
pub(crate) use crate::theme::generated::idx;

fn render_picker(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let op = cx.global::<Pickers>().open.as_ref()?;
    let (tab, target, anchor, input) = (op.tab, op.target, op.anchor, op.input.clone());
    let has_tabs = matches!(target, EmojiTarget::Composer);
    let ph = if has_tabs { PICKER_H } else { PICKER_H_NO_TABS };
    let vp = window.viewport_size();
    let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
    let (left, top) = match anchor {
        Some(p) => {
            let x = (f32::from(p.x) - PICKER_W - 8.).clamp(8., (vw - PICKER_W - 8.).max(8.));
            let y = (f32::from(p.y) - 40.).clamp(8., (vh - ph - 8.).max(8.));
            (x, y)
        }
        None => {
            let side = if !app.show_members {
                0.
            } else if crate::ui::dm_profile_panel::peer(app).is_some() {
                320.
            } else if app.selected_guild.is_some()
                || app.selected_channel.is_some_and(|c| crate::ui::dm_profile_panel::is_group_dm(app, c))
            {
                crate::theme::m::member_list_w()
            } else {
                0.
            };
            let right = 16. + side;
            ((vw - right - PICKER_W).max(8.), (vh - 75. - ph).max(8.))
        }
    };
    let nav = has_tabs.then(|| {
        div()
            .flex_none()
            .h(px(30.))
            .px(px(16.))
            .flex()
            .gap(px(8.))
            .child(tab_button(crate::t!("picker.gifs"), Tab::Gif, tab, cx))
            .child(tab_button(crate::t!("picker.stickers"), Tab::Sticker, tab, cx))
            .child(tab_button(crate::t!("picker.emoji"), Tab::Emoji, tab, cx))
    });
    let mut header = div()
        .flex_none()
        .px(px(16.))
        .pb(px(12.))
        .border_b_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .flex()
        .items_center()
        .gap(px(12.))
        .child(search_box(&input));
    if tab == Tab::Emoji {
        header = header.child(grid::tone_button(cx));
        if has_tabs {
            header = header.child(add_emoji_button());
        }
    }
    let body = match tab {
        Tab::Emoji => grid::render_body(app, cx),
        Tab::Gif => gif::render_body(app, cx),
        Tab::Sticker => sticker::render_body(app, cx),
    };
    Some(
        div()
            .id("picker-layer")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|a, _, w, cx| close(a, w, cx)))
            .child(
                div()
                    .id("picker")
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(PICKER_W))
                    .h(px(ph))
                    .rounded(px(8.))
                    .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
                    .border_1()
                    .border_color(tk(if nav.is_none() && crate::theme::high_contrast() {
                        idx::BORDER_NORMAL
                    } else {
                        idx::BORDER_SUBTLE
                    }))
                    .shadow(vec![gpui::BoxShadow {
                        color: gpui::hsla(0., 0., 0., 0.24),
                        offset: gpui::point(px(0.), px(12.)),
                        blur_radius: px(24.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .overflow_hidden()
                    .pt(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(nav)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .child(header)
                            .child(div().flex_1().min_h_0().flex().child(body)),
                    ),
            )
            .into_any_element(),
    )
}

pub(crate) fn search_box(input: &Entity<InputState>) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .h(px(40.))
        .pl(px(4.))
        .pr(px(12.))
        .gap(px(4.))
        .rounded(px(8.))
        .bg(tk(idx::INPUT_BACKGROUND_DEFAULT))
        .border_1()
        .border_color(tk(idx::BORDER_FOCUS))
        .flex()
        .items_center()
        .text_size(px(16.))
        .text_color(tk(idx::TEXT_DEFAULT))
        .child(div().size(px(30.)).flex_none().flex().items_center().justify_center().child(icon(Icon::Search, 16., tk(idx::TEXT_DEFAULT))))
        .child(div().flex_1().min_w_0().child(Input::new(input).appearance(false).bordered(false)))
}

fn add_emoji_button() -> impl IntoElement {
    div()
        .id("picker-add-emoji")
        .flex_none()
        .h(px(40.))
        .px(px(15.))
        .rounded(px(8.))
        .bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
        .border_1()
        .border_color(tk(idx::CONTROL_SECONDARY_BORDER_DEFAULT))
        .hover(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
        .cursor_pointer()
        .flex()
        .items_center()
        .text_size(px(16.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tk(idx::TEXT_STRONG))
        .child(crate::t!("picker.add_emoji"))
}

fn tab_button(label: SharedString, tab: Tab, current: Tab, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let active = tab == current;
    div()
        .id(label.clone())
        .h(px(30.))
        .px(px(12.))
        .rounded(px(8.))
        .flex()
        .items_center()
        .cursor_pointer()
        .text_size(px(14.))
        .line_height(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tk(if active { idx::TEXT_STRONG } else { idx::TEXT_DEFAULT }))
        .when(active, |d| d.bg(tk(idx::BACKGROUND_MOD_NORMAL)).hover(|s| s.bg(tk(idx::BACKGROUND_MOD_STRONG))))
        .when(crate::theme::high_contrast(), |d| {
            d.border_1().border_color(tk(if active { idx::BORDER_NORMAL } else { idx::BORDER_SUBTLE }))
        })
        .when(!active, |d| {
            d.hover(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)).text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
        })
        .on_click(cx.listener(move |a, _, w, cx| switch_tab(a, tab, w, cx)))
        .child(label)
}

pub(crate) fn section_header(lead: Option<AnyElement>, title: SharedString) -> gpui::Div {
    let fg = tk(idx::TEXT_DEFAULT);
    div()
        .h(px(32.))
        .flex_none()
        .pl(px(8.))
        .pr(px(4.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .flex()
        .items_center()
        .text_color(fg)
        .children(lead)
        .child(
            div()
                .mx(px(4.))
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(14.))
                .line_height(px(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(icon(Icon::Chevron, 16., fg))
}

pub(crate) fn empty_state(text: SharedString) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(14.))
        .text_color(tk(idx::TEXT_MUTED))
        .child(text)
        .into_any_element()
}

pub fn svg_asset(path: &str) -> Option<&'static str> {
    macro_rules! sv {
        ($b:literal) => {
            concat!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#, $b, "</svg>")
        };
    }
    Some(match path {
        "icons/picker/clock.svg" => sv!(r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>"#),
        "icons/picker/trophy.svg" => sv!(r#"<path d="M7 4h10v5a5 5 0 0 1-10 0zM7 6H4a3 3 0 0 0 3 4M17 6h3a3 3 0 0 1-3 4M12 14v4M8 21h8"/>"#),
        "icons/picker/trending.svg" => sv!(r#"<path d="m3 17 6-6 4 4 8-8M15 7h6v6"/>"#),
        "icons/picker/fire.svg" => sv!(r#"<path d="M12 22a7 7 0 0 0 7-7c0-4-3-6-4-10-2 2-3 4-3 6-1-1-2-2-2-4-3 3-5 5-5 8a7 7 0 0 0 7 7z"/>"#),
        "icons/picker/star.svg" => sv!(r#"<path d="M12 3l2.8 5.8 6.2.9-4.5 4.3 1.1 6.2L12 17.3 6.4 20.2l1.1-6.2L3 9.7l6.2-.9z"/>"#),
        "icons/picker/nitro.svg" => sv!(r#"<circle cx="14" cy="12" r="7"/><circle cx="14" cy="12" r="3"/><path d="M3 10h4M4 14h3"/>"#),
        _ => return None,
    })
}

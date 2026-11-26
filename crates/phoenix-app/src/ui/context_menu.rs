use crate::theme::z;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, FontWeight, IntoElement, MouseButton, ParentElement, Pixels, Point,
    SharedString, Styled, Window, anchored, deferred, div, prelude::*, svg,
};

use crate::theme::{c, generated::idx, tok};

pub type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
pub type Dismiss = Rc<dyn Fn(&mut Window, &mut App)>;
type ActiveMenu = (Rc<Vec<Nav>>, Point<Pixels>, Dismiss);

pub enum MenuItem {
    Action { label: SharedString, icon: Option<SharedString>, subtext: Option<SharedString>, danger: bool, on_click: Handler },
    Separator,
    Custom(AnyElement),
    Submenu { label: SharedString, items: Vec<MenuItem>, icon: Option<SharedString>, subtext: Option<SharedString> },
    Check { label: SharedString, checked: bool, radio: bool, subtext: Option<SharedString>, icon: Option<SharedString>, on_click: Handler },
    Disabled { label: SharedString, icon: Option<SharedString> },
    Leading { label: SharedString, leading: AnyElement, subtext: Option<SharedString>, on_click: Option<Handler> },
}

thread_local! {
    static OPEN_SUB: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static HL_ROOT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static HL_SUB: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static ACTIVE: std::cell::RefCell<Option<ActiveMenu>> = const { std::cell::RefCell::new(None) };
}

pub fn begin_frame() {
    ACTIVE.with(|a| a.borrow_mut().take());
}

pub fn is_open() -> bool {
    ACTIVE.with(|a| a.borrow().is_some())
}

pub fn on_key(ks: &gpui::Keystroke, w: &mut Window, cx: &mut App) -> bool {
    if ks.modifiers.modified() {
        return false;
    }
    let Some((nav, pos, dismiss)) = ACTIVE.with(|a| a.borrow().clone()) else { return false };
    handle_key(&ks.key, &nav, pos, &dismiss, w, cx)
}

fn set_hl(depth: u8, v: Option<usize>, w: &mut Window) {
    let cell = if depth == 0 { &HL_ROOT } else { &HL_SUB };
    if cell.with(|c| c.replace(v)) != v {
        w.refresh();
    }
}

#[doc(hidden)]
pub fn debug_open_submenu(ix: usize) {
    OPEN_SUB.with(|c| c.set(Some(ix)));
    HL_ROOT.with(|c| c.set(Some(ix)));
}

fn reset_state() {
    OPEN_SUB.with(|c| c.set(None));
    HL_ROOT.with(|c| c.set(None));
    HL_SUB.with(|c| c.set(None));
}

enum Nav {
    Skip,
    Act(Handler),
    Sub(Vec<Option<Handler>>),
}

fn nav_of(items: &[MenuItem]) -> Vec<Nav> {
    items
        .iter()
        .map(|i| match i {
            MenuItem::Action { on_click, .. } | MenuItem::Check { on_click, .. } => Nav::Act(on_click.clone()),
            MenuItem::Leading { on_click: Some(h), .. } => Nav::Act(h.clone()),
            MenuItem::Submenu { items, .. } => Nav::Sub(
                items
                    .iter()
                    .map(|i| match i {
                        MenuItem::Action { on_click, .. } | MenuItem::Check { on_click, .. } => {
                            Some(on_click.clone())
                        }
                        MenuItem::Leading { on_click, .. } => on_click.clone(),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => Nav::Skip,
        })
        .collect()
}

fn step(ok: &[bool], cur: Option<usize>, dir: isize) -> Option<usize> {
    let n = ok.len() as isize;
    if n == 0 || !ok.iter().any(|b| *b) {
        return None;
    }
    let mut i = match cur {
        Some(c) => c as isize,
        None if dir > 0 => -1,
        None => n,
    };
    for _ in 0..n {
        i = (i + dir).rem_euclid(n);
        if ok[i as usize] {
            return Some(i as usize);
        }
    }
    None
}

fn keyboard_click(pos: Point<Pixels>) -> ClickEvent {
    ClickEvent::Keyboard(gpui::KeyboardClickEvent {
        button: gpui::KeyboardButton::Enter,
        bounds: gpui::Bounds::new(pos, gpui::size(z(0.), z(0.))),
    })
}

fn handle_key(key: &str, nav: &[Nav], pos: Point<Pixels>, dismiss: &Dismiss, w: &mut Window, cx: &mut App) -> bool {
    let root = HL_ROOT.with(|c| c.get());
    let sub = HL_SUB.with(|c| c.get());
    let open = OPEN_SUB.with(|c| c.get());
    let sub_list = open.and_then(|o| match &nav[o] {
        Nav::Sub(v) => Some(v),
        _ => None,
    });
    let in_sub = sub.is_some() && sub_list.is_some();
    let root_ok: Vec<bool> = nav.iter().map(|n| !matches!(n, Nav::Skip)).collect();
    match key {
        "down" | "up" | "tab" => {
            let dir = if key == "up" { -1 } else { 1 };
            if in_sub {
                let ok: Vec<bool> = sub_list.unwrap().iter().map(Option::is_some).collect();
                set_hl(1, step(&ok, sub, dir), w);
            } else {
                let next = step(&root_ok, root, dir);
                set_hl(0, next, w);
                set_open_sub(None, w);
            }
        }
        "home" | "end" => {
            let dir = if key == "home" { 1 } else { -1 };
            if in_sub {
                let ok: Vec<bool> = sub_list.unwrap().iter().map(Option::is_some).collect();
                set_hl(1, step(&ok, None, dir), w);
            } else {
                set_hl(0, step(&root_ok, None, dir), w);
                set_open_sub(None, w);
            }
        }
        "right" if !in_sub => {
            if let Some(r) = root
                && let Nav::Sub(v) = &nav[r]
            {
                set_open_sub(Some(r), w);
                let ok: Vec<bool> = v.iter().map(Option::is_some).collect();
                set_hl(1, step(&ok, None, 1), w);
            }
        }
        "left" if in_sub => {
            set_hl(1, None, w);
            set_open_sub(None, w);
        }
        "escape" => {
            if in_sub {
                set_hl(1, None, w);
                set_open_sub(None, w);
            } else {
                dismiss(w, cx);
            }
        }
        "enter" | "space" => {
            if in_sub {
                if let Some(Some(h)) = sub.and_then(|s| sub_list.unwrap().get(s)) {
                    h(&keyboard_click(pos), w, cx);
                    dismiss(w, cx);
                }
            } else if let Some(r) = root {
                match &nav[r] {
                    Nav::Act(h) => {
                        h(&keyboard_click(pos), w, cx);
                        dismiss(w, cx);
                    }
                    Nav::Sub(_) => return handle_key("right", nav, pos, dismiss, w, cx),
                    Nav::Skip => {}
                }
            }
        }
        _ => return false,
    }
    true
}

fn set_open_sub(v: Option<usize>, w: &mut Window) {
    if OPEN_SUB.with(|c| c.replace(v)) != v {
        w.refresh();
    }
}

impl MenuItem {
    pub fn action(
        label: impl Into<SharedString>,
        icon: Option<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self::Action { label: label.into(), icon, subtext: None, danger: false, on_click: Rc::new(on_click) }
    }

    pub fn disabled(label: impl Into<SharedString>, icon: Option<SharedString>) -> Self {
        Self::Disabled { label: label.into(), icon }
    }

    pub fn danger(
        label: impl Into<SharedString>,
        icon: Option<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self::Action { label: label.into(), icon, subtext: None, danger: true, on_click: Rc::new(on_click) }
    }

    pub fn check(
        label: impl Into<SharedString>,
        checked: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self::Check { label: label.into(), checked, radio: false, subtext: None, icon: None, on_click: Rc::new(on_click) }
    }

    pub fn radio(
        label: impl Into<SharedString>,
        checked: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self::Check { label: label.into(), checked, radio: true, subtext: None, icon: None, on_click: Rc::new(on_click) }
    }

    pub fn with_subtext(mut self, text: impl Into<SharedString>) -> Self {
        if let Self::Action { subtext, .. } | Self::Submenu { subtext, .. } | Self::Check { subtext, .. } = &mut self {
            *subtext = Some(text.into());
        }
        self
    }

    pub fn with_icon(mut self, name: &str) -> Self {
        if let Self::Action { icon, .. } | Self::Submenu { icon, .. } | Self::Check { icon, .. } = &mut self {
            *icon = Some(crate::icons::path(name));
        }
        self
    }

    pub fn with_shortcut(mut self, keys: &str) -> Self {
        if let Self::Action { icon, .. } = &mut self {
            *icon = Some(format!("{KBD}{keys}").into());
        }
        self
    }

    pub fn leading(
        label: impl Into<SharedString>,
        leading: AnyElement,
        subtext: Option<SharedString>,
        on_click: Option<Handler>,
    ) -> Self {
        Self::Leading { label: label.into(), leading, subtext, on_click }
    }

    pub fn submenu(label: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        Self::Submenu { label: label.into(), items, icon: None, subtext: None }
    }

    pub fn submenu_with_icon(label: impl Into<SharedString>, icon: &str, items: Vec<MenuItem>) -> Self {
        Self::Submenu { label: label.into(), items, icon: Some(crate::icons::path(icon)), subtext: None }
    }
}

pub const MENU_W: f32 = 220.;
const MENU_MIN_W: f32 = 188.;
const MENU_MAX_W: f32 = 320.;
const VIEWPORT_PAD: f32 = 12.;
pub const EMOJI_ICON: &str = "emoji:";
pub const KBD: &str = "kbd:";
const SUB_OVERLAP: f32 = 4.;
const CHEVRON: &str = "chevron-right";
const CHECK: &str = "check";

thread_local! {
    static VIEW_H: std::cell::Cell<f32> = const { std::cell::Cell::new(700.) };
}

fn row_h(item: &MenuItem) -> f32 {
    match item {
        MenuItem::Separator => 17.,
        MenuItem::Disabled { icon, .. } => if icon.is_some() { 36. } else { 34. },
        MenuItem::Leading { subtext, .. } => if subtext.is_some() { 52. } else { 36. },
        MenuItem::Custom(_) => 60.,
        MenuItem::Action { icon, subtext, .. } => {
            let text: f32 = 18. + if subtext.is_some() { 18. } else { 0. };
            16. + text.max(if icon.as_ref().is_some_and(|n| !n.starts_with(KBD)) { 20. } else { 0. })
        }
        MenuItem::Submenu { subtext: Some(_), .. } | MenuItem::Check { subtext: Some(_), .. } => 52.,
        MenuItem::Submenu { .. } | MenuItem::Check { .. } => 36.,
    }
}

pub fn panel(id: &'static str, items: Vec<MenuItem>, dismiss: Dismiss) -> gpui::Stateful<gpui::Div> {
    panel_at(id, 0, items, dismiss)
}

fn row(id: (&'static str, usize), lit: bool, danger: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .min_h(z(32.))
        .p(z(8.))
        .rounded(z(if lit { 4. } else { 2. }))
        .flex()
        .items_center()
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(if danger { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_STRONG })))
        .when(lit, |d| {
            d.bg(c(tok(if danger { idx::BACKGROUND_FEEDBACK_MOD_CRITICAL } else { idx::INTERACTIVE_BACKGROUND_HOVER })))
        })
        .when(!lit, |d| d.hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))))
        .active(move |s| s.bg(c(tok(if danger { idx::BACKGROUND_FEEDBACK_MOD_CRITICAL } else { idx::BACKGROUND_MOD_SUBTLE }))))
}

fn label_box(label: SharedString) -> gpui::Div {
    div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(label)
}

fn panel_at(id: &'static str, depth: u8, items: Vec<MenuItem>, dismiss: Dismiss) -> gpui::Stateful<gpui::Div> {
    let hl = if depth == 0 { HL_ROOT.with(|c| c.get()) } else { HL_SUB.with(|c| c.get()) };
    let open_sub = if depth == 0 { OPEN_SUB.with(|c| c.get()) } else { None };
    let max_h = VIEW_H.with(|v| v.get()) - VIEWPORT_PAD * 2.;
    let mut list = div()
        .id(SharedString::from(format!("{id}-scroller")))
        .flex()
        .flex_col()
        .py(z(8.))
        .px(z(8.))
        .max_h(z(max_h - 2.))
        .overflow_y_scroll()
        .track_scroll(&crate::ui::chat::scrollbar::handle(id));
    let mut sub_panel = None;
    let mut y = 0.;
    for (ix, item) in items.into_iter().enumerate() {
        let lit = hl == Some(ix);
        let top = y;
        y += row_h(&item);
        list = list.child(match item {
            MenuItem::Separator => div().flex_none().my(z(8.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))).into_any_element(),
            MenuItem::Custom(el) => el,
            MenuItem::Submenu { label, items, icon, subtext } => {
                let open = open_sub == Some(ix);
                if open {
                    sub_panel = Some(
                        div()
                            .absolute()
                            .top(z(top))
                            .left_full()
                            .ml(z(-SUB_OVERLAP))
                            .child(gpui::anchored().snap_to_window_with_margin(z(VIEWPORT_PAD)).child(panel_at("context-submenu", 1, items, dismiss.clone()))),
                    );
                }
                row(("menu-sub", ix), open || lit, false)
                    .on_hover(move |h, w, _| {
                        if *h {
                            set_open_sub(Some(ix), w);
                            set_hl(0, Some(ix), w);
                            set_hl(1, None, w);
                        }
                    })
                    .when_some(icon, |d, path| {
                        let fg = if open || lit { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT };
                        d.child(svg().path(path).mr(z(8.)).size(z(20.)).flex_none().text_color(c(tok(fg))))
                    })
                    .child(div().flex_1().min_w_0().flex().flex_col().child(label_box(label)).when_some(subtext, |d, s| {
                        d.child(
                            div()
                                .mt(z(2.))
                                .text_size(z(12.))
                                .line_height(z(16.))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(c(tok(if open || lit { idx::TEXT_SUBTLE } else { idx::TEXT_MUTED })))
                                .child(s),
                        )
                    }))
                    .child(
                        svg()
                            .path(crate::icons::path(CHEVRON))
                            .ml(z(8.))
                            .size(z(20.))
                            .flex_none()
                            .text_color(if open || lit { gpui::white() } else { c(tok(idx::TEXT_MUTED)) }),
                    )
                    .into_any_element()
            }
            MenuItem::Check { label, checked, radio, subtext, icon, on_click } => {
                let dismiss = dismiss.clone();
                let mark = if radio {
                    crate::ui::controls::radio_indicator(checked)
                } else {
                    let (bg, border) = if checked {
                        (idx::CHECKBOX_BACKGROUND_SELECTED_DEFAULT, idx::CHECKBOX_BORDER_SELECTED_DEFAULT)
                    } else {
                        (idx::CHECKBOX_BACKGROUND_DEFAULT, idx::CHECKBOX_BORDER_DEFAULT)
                    };
                    div()
                        .size(z(20.))
                        .flex_none()
                        .rounded(z(4.))
                        .border_1()
                        .border_color(c(tok(border)))
                        .bg(c(tok(bg)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(checked, |d| {
                            d.child(svg().path(crate::icons::path(CHECK)).size(z(18.)).text_color(gpui::white()))
                        })
                };
                row(("menu-check", ix), lit, false)
                    .on_hover(move |h, w, _| {
                        if *h {
                            if depth == 0 {
                                set_open_sub(None, w);
                            }
                            set_hl(depth, Some(ix), w);
                        }
                    })
                    .when_some(icon, |d, path| {
                        let fg = if lit { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT };
                        d.child(svg().path(path).mr(z(8.)).size(z(20.)).flex_none().text_color(c(tok(fg))))
                    })
                    .child(div().flex_1().min_w_0().flex().flex_col().child(label_box(label)).when_some(subtext, |d, s| {
                        d.child(
                            div()
                                .mt(z(2.))
                                .text_size(z(12.))
                                .line_height(z(16.))
                                .text_color(c(tok(if lit { idx::TEXT_SUBTLE } else { idx::TEXT_MUTED })))
                                .child(s),
                        )
                    }))
                    .child(div().ml(z(8.)).child(mark))
                    .on_click(move |ev, w, cx| {
                        on_click(ev, w, cx);
                        if radio {
                            dismiss(w, cx);
                        } else {
                            w.refresh();
                        }
                    })
                    .into_any_element()
            }
            MenuItem::Disabled { label, icon } => div()
                .id(("menu-item", ix))
                .flex_none()
                .min_h(z(32.))
                .p(z(8.))
                .rounded(z(2.))
                .flex()
                .items_center()
                .opacity(0.5)
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .when_some(icon, |d, name| {
                    let path: SharedString = if name.ends_with(".svg") { name } else { crate::icons::path(&name) };
                    d.child(svg().path(path).mr(z(8.)).size(z(20.)).flex_none().text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                })
                .child(label_box(label))
                .into_any_element(),
            MenuItem::Leading { label, leading, subtext, on_click } => {
                let dismiss = dismiss.clone();
                let enabled = on_click.is_some();
                let body = div().flex_1().min_w_0().flex().flex_col().child(label_box(label)).when_some(subtext, |d, s| {
                    d.child(
                        div()
                            .mt(z(2.))
                            .text_size(z(12.))
                            .line_height(z(16.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(c(tok(if lit && enabled { idx::TEXT_SUBTLE } else { idx::TEXT_MUTED })))
                            .child(s),
                    )
                });
                let el = if enabled { row(("menu-lead", ix), lit, false) } else { row(("menu-lead", ix), false, false).cursor_default().opacity(0.5) };
                el.when(enabled, |d| {
                    d.on_hover(move |h, w, _| {
                        if *h {
                            if depth == 0 {
                                set_open_sub(None, w);
                            }
                            set_hl(depth, Some(ix), w);
                        } else if hl == Some(ix) {
                            set_hl(depth, None, w);
                        }
                    })
                })
                .when(!enabled, |d| d.hover(|s| s.bg(gpui::transparent_black())).active(|s| s.bg(gpui::transparent_black())))
                .child(div().mr(z(8.)).min_w(z(20.)).h(z(20.)).flex_none().flex().items_center().justify_center().child(leading))
                .child(body)
                .when_some(on_click, |d, f| {
                    d.on_click(move |ev, w, cx| {
                        f(ev, w, cx);
                        dismiss(w, cx);
                    })
                })
                .into_any_element()
            }
            MenuItem::Action { label, icon, subtext, danger, on_click } => {
                let dismiss = dismiss.clone();
                let icon_fg = if danger {
                    c(tok(idx::TEXT_FEEDBACK_CRITICAL))
                } else if lit {
                    c(tok(idx::INTERACTIVE_TEXT_ACTIVE))
                } else {
                    c(tok(idx::INTERACTIVE_TEXT_DEFAULT))
                };
                let shortcut = icon.as_ref().and_then(|n| n.strip_prefix(KBD)).map(|k| SharedString::from(k.to_owned()));
                let icon = icon.filter(|n| !n.starts_with(KBD));
                row(("menu-item", ix), lit, danger)
                    .on_hover(move |h, w, _| {
                        if *h {
                            if depth == 0 {
                                set_open_sub(None, w);
                            }
                            set_hl(depth, Some(ix), w);
                        } else if hl == Some(ix) {
                            set_hl(depth, None, w);
                        }
                    })
                    .when_some(icon, |d, name| {
                        if let Some(token) = name.strip_prefix(EMOJI_ICON) {
                            return d.child(div().mr(z(8.)).size(z(20.)).flex_none().child(crate::media::emoji::emoji_element(token, 20.)));
                        }
                        let path: SharedString =
                            if name.ends_with(".svg") { name } else { crate::icons::path(&name) };
                        d.child(svg().path(path).mr(z(8.)).size(z(20.)).flex_none().text_color(icon_fg))
                    })
                    .child(
                        div().flex_1().min_w_0().flex().flex_col().child(label_box(label)).when_some(subtext, |d, s| {
                            d.child(
                                div()
                                    .mt(z(2.))
                                    .text_size(z(12.))
                                    .line_height(z(16.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(c(tok(if lit { idx::TEXT_SUBTLE } else { idx::TEXT_MUTED })))
                                    .child(s),
                            )
                        }),
                    )
                    .when_some(shortcut, |d, k| d.child(div().flex_none().ml(z(8.)).max_h(z(18.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).child(k)))
                    .on_click(move |ev, w, cx| {
                        on_click(ev, w, cx);
                        dismiss(w, cx);
                    })
                    .into_any_element()
            }
        });
    }
    div()
        .id(id)
        .relative()
        .min_w(z(MENU_MIN_W))
        .max_w(z(MENU_MAX_W))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .flex()
        .flex_col()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .child(
            crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Thin)
                .child(list)
                .children(crate::ui::chat::scrollbar::bar_for(id, crate::ui::chat::scrollbar::Bar::Thin)),
        )
        .children(sub_panel)
}

pub fn render(pos: Point<Pixels>, items: Vec<MenuItem>, dismiss: Dismiss) -> AnyElement {
    render_anchored(pos, gpui::Anchor::TopLeft, items, dismiss)
}

pub fn render_anchored(pos: Point<Pixels>, corner: gpui::Anchor, items: Vec<MenuItem>, dismiss: Dismiss) -> AnyElement {
    render_anchored_at(pos, corner, items, dismiss, 10)
}

pub fn render_anchored_at(pos: Point<Pixels>, corner: gpui::Anchor, items: Vec<MenuItem>, dismiss: Dismiss, priority: usize) -> AnyElement {
    let inner = dismiss;
    let dismiss: Dismiss = Rc::new(move |w, cx| {
        reset_state();
        ACTIVE.with(|a| a.borrow_mut().take());
        inner(w, cx)
    });
    let nav = Rc::new(nav_of(&items));
    let kd = dismiss.clone();
    let keys = gpui::canvas(
        |_, _, _| (),
        move |_, _, w, _| {
            VIEW_H.with(|v| v.set(f32::from(w.viewport_size().height) / f32::from(z(1.))));
            ACTIVE.with(|a| *a.borrow_mut() = Some((nav, pos, kd)));
        },
    )
    .absolute()
    .size_0();
    let d1 = dismiss.clone();
    let d2 = dismiss.clone();
    deferred(
        div()
            .id("context-menu-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .on_mouse_down(MouseButton::Left, move |_, w, cx| d1(w, cx))
            .on_mouse_down(MouseButton::Right, move |_, w, cx| d2(w, cx))
            .child(keys)
            .child(
                anchored()
                    .anchor(corner)
                    .position(pos)
                    .snap_to_window_with_margin(z(VIEWPORT_PAD))
                    .child(panel("context-menu", items, dismiss)),
            ),
    )
    .with_priority(priority)
    .into_any_element()
}

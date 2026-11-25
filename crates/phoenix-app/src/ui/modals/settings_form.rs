use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, ElementId, FontWeight, IntoElement, MouseButton, ParentElement, SharedString, Styled, Window, deferred, div, prelude::*, relative,
};

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{radio_indicator, switch},
};

pub type OnPick = Rc<dyn Fn(usize, &mut Window, &mut App)>;
pub type OnValue = Rc<dyn Fn(u32, &mut Window, &mut App)>;
pub type OnWrap = Rc<dyn Fn(&'static str, &mut Window, &mut App)>;
pub type Cb = Rc<dyn Fn(&mut App)>;

pub fn text(t: impl Into<SharedString>, size: f32, line: f32, color: usize) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).text_color(c(tok(color))).child(t.into())
}

pub fn label(t: impl Into<SharedString>) -> gpui::Div {
    text(t, 16., 20., idx::TEXT_STRONG).font_weight(FontWeight::MEDIUM)
}

pub fn helper(t: impl Into<SharedString>) -> gpui::Div {
    text(t, 12., 16., idx::TEXT_SUBTLE).pr(z(12.))
}

pub fn description(t: impl Into<SharedString>) -> gpui::Div {
    text(t, 14., 18., idx::TEXT_SUBTLE)
}

pub fn field(title: SharedString, desc: Option<SharedString>, control: impl IntoElement, help: Option<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .child(div().mb(z(8.)).flex().flex_col().gap(z(4.)).child(label(title)).children(desc.map(description)))
        .child(div().flex().flex_col().gap(z(4.)).child(control).children(help.map(helper)))
}

pub fn inline_field(title: SharedString, desc: Option<AnyElement>, control: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .items_start()
        .gap(z(24.))
        .child(div().flex_1().min_w_0().flex().flex_col().gap(z(4.)).child(label(title)).children(desc))
        .child(div().flex_none().flex().flex_col().items_end().child(control))
}

pub fn switch_field(
    id: &'static str,
    title: SharedString,
    desc: Option<AnyElement>,
    on: bool,
    toggle: Rc<dyn Fn(&mut App)>,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    div()
        .id(id)
        .cursor_pointer()
        .group(crate::ui::controls::SWITCH_ROW)
        .on_click(cx.listener(move |_, _, _, cx| {
            toggle(cx);
            cx.notify();
        }))
        .child(inline_field(title, desc, switch(SharedString::from(format!("{id}-switch")), on)))
        .into_any_element()
}

pub fn select_box(id: impl Into<ElementId>, value: impl Into<SharedString>, open: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .h(z(40.))
        .w_full()
        .flex()
        .items_center()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(if open { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_DEFAULT })))
        .hover(move |s| if open { s } else { s.border_color(c(tok(idx::INPUT_BORDER_HOVER))) })
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .cursor_pointer()
        .child(div().flex_1().min_w_0().pl(z(12.)).truncate().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(value.into()))
        .child(
            div().flex_none().w(z(36.)).h_full().pr(z(4.)).flex().items_center().justify_center().child(
                gpui::svg()
                    .path(crate::icons::path("settings/chevron-down"))
                    .size(z(20.))
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .when(open, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI)))),
            ),
        )
}

pub fn dropdown(
    id: &'static str,
    options: Vec<SharedString>,
    selected: usize,
    open: bool,
    toggle: Rc<dyn Fn(&mut App)>,
    pick: OnPick,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let cur = options.get(selected).cloned().unwrap_or_default();
    let mut wrap = div().relative().w_full().child(select_box(SharedString::from(format!("{id}-box")), cur, open).on_click(cx.listener(move |_, _, _, cx| {
        toggle(cx);
        cx.notify();
    })));
    if open {
        let mut list = div()
            .id(SharedString::from(format!("{id}-list")))
            .occlude()
            .max_h(z(320.))
            .overflow_y_scroll()
            .p(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .shadow(vec![gpui::BoxShadow {
                color: gpui::hsla(0., 0., 0., 0.16),
                offset: gpui::point(z(0.), z(2.)),
                blur_radius: z(8.),
                spread_radius: z(0.),
                inset: false,
            }])
            .flex()
            .flex_col();
        for (k, l) in options.into_iter().enumerate() {
            let pick = pick.clone();
            let on = k == selected;
            list = list.child(
                div()
                    .id((id, k))
                    .flex_none()
                    .h(z(40.))
                    .px(z(8.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                    .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                    .cursor_pointer()
                    .child(div().truncate().child(l))
                    .when(on, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(z(20.)).text_color(c(tok(idx::TEXT_BRAND)))))
                    .on_click(move |_, w, cx| pick(k, w, cx)),
            );
        }
        wrap = wrap.child(deferred(div().absolute().top(z(44.)).left_0().w_full().child(list)).with_priority(30));
    }
    wrap.into_any_element()
}

pub fn radio_group(id: &'static str, options: Vec<(SharedString, Option<SharedString>)>, selected: usize, pick: OnPick) -> AnyElement {
    let mut g = div().pt(z(8.)).pb(z(4.)).flex().flex_col().gap(z(16.));
    for (k, (title, desc)) in options.into_iter().enumerate() {
        let pick = pick.clone();
        g = g.child(
            div()
                .id((id, k))
                .relative()
                .pl(z(32.))
                .rounded(z(8.))
                .cursor_pointer()
                .on_click(move |_, w, cx| pick(k, w, cx))
                .child(div().absolute().top_0().left_0().child(radio_indicator(k == selected)))
                .child(text(title, 16., 20., idx::TEXT_DEFAULT))
                .children(desc.map(|d| description(d).mt(z(4.)))),
        );
    }
    g.into_any_element()
}

pub fn value_slider(id: &'static str, min: u32, max: u32, value: u32, marks: Vec<(u32, SharedString)>, on_set: OnValue) -> AnyElement {
    let span = (max - min).max(1) as f32;
    let frac = |v: u32| (v.clamp(min, max) - min) as f32 / span;
    let n_marks = marks.len();
    let mut track = div().absolute().top_0().bottom_0().left(z(5.)).right(z(5.));
    for (i, (v, l)) in marks.into_iter().enumerate() {
        let lab = text(l, 12., 16., idx::TEXT_SUBTLE).font_weight(FontWeight::MEDIUM).whitespace_nowrap().pl(z(1.)).mb(z(4.));
        let lab = if i == 0 {
            div().w(z(0.)).flex().justify_start().child(lab.ml(z(-5.)))
        } else if i + 1 == n_marks {
            div().w(z(0.)).flex().justify_end().child(lab.mr(z(-5.)))
        } else {
            div().w(z(0.)).flex().justify_center().child(lab)
        };
        track = track.child(
            div()
                .absolute()
                .top_0()
                .left(relative(frac(v)))
                .h(z(48.))
                .w(z(0.))
                .flex()
                .flex_col()
                .items_center()
                .justify_between()
                .child(lab)
                .child(div().w(z(2.)).h(z(24.)).rounded(z(12.)).bg(c(tok(idx::SLIDER_TRACK_BACKGROUND)))),
        );
    }
    let steps = ((max - min + 1) as usize).min(250);
    let mut hits = div().absolute().top_0().bottom_0().left(z(5.)).right(z(5.)).flex();
    for k in 0..steps {
        let v = if steps as u32 == max - min + 1 { min + k as u32 } else { min + ((k as f32 / (steps - 1) as f32) * span).round() as u32 };
        let (a, b) = (on_set.clone(), on_set.clone());
        hits = hits.child(
            div()
                .id(ElementId::NamedInteger(SharedString::from(format!("{id}-hit")), k as u64))
                .h_full()
                .flex_1()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, w, cx| a(v, w, cx))
                .on_mouse_move(move |ev, w, cx| {
                    if ev.dragging() {
                        b(v, w, cx)
                    }
                }),
        );
    }
    let f = frac(value);
    div()
        .relative()
        .w_full()
        .h(z(48.))
        .child(track)
        .child(
            div()
                .absolute()
                .top(z(34.))
                .left_0()
                .w_full()
                .h(z(4.))
                .rounded(z(4.))
                .overflow_hidden()
                .bg(c(tok(idx::SLIDER_TRACK_BACKGROUND)))
                .child(div().h_full().w(relative(f)).border_1().border_color(gpui::hsla(0., 0., 1., 0.1)).bg(c(tok(idx::SLIDER_INDICATOR_BACKGROUND)))),
        )
        .child(hits)
        .child(
            div().absolute().top(z(28.)).left(z(5.)).right(z(5.)).h(z(16.)).child(
                div()
                    .absolute()
                    .top_0()
                    .left(relative(f))
                    .ml(z(-8.))
                    .size(z(16.))
                    .rounded_full()
                    .bg(gpui::white())
                    .border_1()
                    .border_color(c(tok(idx::BORDER_NORMAL)))
                    .shadow_sm(),
            ),
        )
        .into_any_element()
}

pub fn info_notice(t: impl Into<SharedString>) -> AnyElement {
    div()
        .p(z(12.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_FEEDBACK_INFO)))
        .bg(c(tok(idx::BACKGROUND_FEEDBACK_INFO)))
        .flex()
        .items_start()
        .gap(z(8.))
        .child(
            div().h(z(18.)).flex().items_center().child(gpui::svg().path(crate::icons::path("info")).size(z(16.)).text_color(c(tok(idx::ICON_FEEDBACK_INFO)))),
        )
        .child(div().flex_1().min_w_0().child(text(t, 14., 18., idx::TEXT_STRONG)))
        .into_any_element()
}

pub fn format_toolbar(id: &'static str, extra: bool, on_wrap: OnWrap, trailing: Option<AnyElement>) -> AnyElement {
    let mut tools: Vec<(&'static str, &'static str)> =
        vec![("settings/bold", "**"), ("settings/italic", "*"), ("settings/underline", "__"), ("settings/strike", "~~")];
    if extra {
        tools.push(("settings/quote", "> "));
        tools.push(("angle-brackets", "`"));
    }
    let mut row = div().flex();
    for (n, (icon, mark)) in tools.into_iter().enumerate() {
        let w = on_wrap.clone();
        row = row.child(
            div()
                .id((id, n))
                .size(z(32.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .when(n == 0, |d| d.rounded_tl(z(4.)))
                .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .on_click(move |_, win, cx| w(mark, win, cx))
                .child(gpui::svg().path(crate::icons::path(icon)).size(z(20.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
        );
    }
    let w = on_wrap.clone();
    row = row.child(div().my(z(6.)).mx(z(6.)).w(z(1.)).h(z(20.)).bg(c(tok(idx::BORDER_SUBTLE)))).child(
        div()
            .id((id, 99usize))
            .size(z(32.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
            .on_click(move |_, win, cx| w("||", win, cx))
            .child(gpui::svg().path(crate::icons::path("settings/eye")).size(z(20.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
    );
    div().h(z(32.)).mb(z(-4.)).rounded_t(z(4.)).bg(c(tok(idx::BACKGROUND_MOD_NORMAL))).flex().justify_between().child(row).children(trailing).into_any_element()
}

pub fn wrap_selection(text: &str, range: std::ops::Range<usize>, mark: &str) -> (String, std::ops::Range<usize>) {
    let (a, b) = (range.start.min(text.len()), range.end.min(text.len()));
    if mark == "> " {
        let line = text[..a].rfind('\n').map_or(0, |i| i + 1);
        let out = format!("{}{mark}{}", &text[..line], &text[line..]);
        return (out, a + mark.len()..b + mark.len());
    }
    let out = format!("{}{mark}{}{mark}{}", &text[..a], &text[a..b], &text[b..]);
    (out, a + mark.len()..b + mark.len())
}

#[cfg(test)]
mod tests {
    #[test]
    fn wraps_markdown() {
        assert_eq!(super::wrap_selection("hello world", 6..11, "**"), ("hello **world**".into(), 8..13));
        assert_eq!(super::wrap_selection("ab", 1..1, "||"), ("a||||b".into(), 3..3));
        assert_eq!(super::wrap_selection("x\ny", 2..3, "> "), ("x\n> y".into(), 4..5));
    }
}

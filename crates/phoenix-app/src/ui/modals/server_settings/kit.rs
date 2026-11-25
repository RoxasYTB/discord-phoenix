use std::rc::Rc;

use gpui::{AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Stateful, Styled, div, prelude::*};
use serde_json::Value;

use super::{State, with_state};
use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
};

pub type Cx<'a> = Context<'a, PhoenixApp>;
pub type OnPick = Rc<dyn Fn(&mut PhoenixApp, usize, &mut Context<'_, PhoenixApp>)>;

pub fn text(t: impl Into<SharedString>, size: f32, line: f32, weight: FontWeight, color: usize) -> Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(c(tok(color))).child(t.into())
}

pub fn h2(t: impl Into<SharedString>) -> Div {
    text(t, 20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG)
}

pub fn page_head(title: SharedString, desc: Option<SharedString>) -> Div {
    div().flex().flex_col().child(h2(title)).children(desc.map(|d| text(d, 14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT).mt(z(8.))))
}

pub fn legend(title: SharedString, desc: Option<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(text(title, 20., 24., FontWeight::MEDIUM, idx::TEXT_STRONG))
        .children(desc.map(|d| text(d, 14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT)))
}

pub fn fieldset(title: SharedString, desc: Option<SharedString>, rows: Vec<AnyElement>) -> Div {
    div().flex().flex_col().gap(z(16.)).child(legend(title, desc)).child(div().flex().flex_col().gap(z(24.)).children(rows))
}

pub fn labels(label: SharedString, desc: Option<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .min_w_0()
        .child(text(label, 16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG))
        .children(desc.map(|d| text(d, 14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE)))
}

pub fn field(label: SharedString, desc: Option<SharedString>, control: impl IntoElement) -> Div {
    div().flex().flex_col().child(labels(label, desc).mb(z(8.))).child(control)
}

pub fn helper(t: SharedString) -> Div {
    text(t, 12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE).mt(z(4.))
}

pub fn divider() -> Div {
    div().flex_none().h(z(1.)).w_full().bg(c(tok(idx::BORDER_STRONG)))
}

pub fn switch_field(
    id: &'static str,
    label: SharedString,
    desc: Option<SharedString>,
    on: bool,
    flip: impl Fn(&mut State) + 'static,
    cx: &mut Cx,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .gap(z(24.))
        .group(crate::ui::controls::SWITCH_ROW)
        .child(labels(label, desc).flex_1().pt(z(2.)))
        .child(crate::ui::controls::switch((id, 1usize), on))
        .cursor_pointer()
        .on_click(cx.listener(move |_, _, _, cx| {
            with_state(&flip);
            cx.notify();
        }))
        .into_any_element()
}

pub fn select_box(id: impl Into<gpui::ElementId>, value: AnyElement, open: bool) -> Stateful<Div> {
    div()
        .id(id.into())
        .relative()
        .h(z(40.))
        .pl(z(12.))
        .pr(z(8.))
        .flex()
        .items_center()
        .gap(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .hover(|s| s.border_color(c(tok(idx::INPUT_BORDER_HOVER))))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .cursor_pointer()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(value),
        )
        .child(
            gpui::svg()
                .path(crate::icons::path(if open { "chevron-up" } else { "chevron" }))
                .flex_none()
                .mr(z(8.))
                .size(z(18.))
                .text_color(c(tok(idx::ICON_STRONG))),
        )
        .when(open, |d| {
            d.child(
                div().absolute().top(z(-1.)).bottom(z(-1.)).left(z(-1.)).right(z(-1.)).rounded(z(8.)).border_2().border_color(c(tok(idx::INPUT_BORDER_ACTIVE))),
            )
        })
}

pub fn dropdown(s: &State, id: &'static str, value: AnyElement, opts: Vec<SharedString>, cur: Option<usize>, on_pick: OnPick, cx: &mut Cx) -> AnyElement {
    let open = s.open_select == Some(id);
    let field = select_box(SharedString::from(format!("ss-sel-{id}")), value, open).on_click(cx.listener(move |_, _, _, cx| {
        with_state(|s| s.open_select = if s.open_select == Some(id) { None } else { Some(id) });
        cx.notify();
    }));
    let mut wrap = div().relative().w_full().child(field);
    if open {
        let list = div()
            .id(SharedString::from(format!("ss-sel-{id}-list")))
            .absolute()
            .top(z(44.))
            .left_0()
            .w_full()
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
            .occlude()
            .children(opts.into_iter().enumerate().map(|(i, o)| {
                let f = on_pick.clone();
                div()
                    .id(SharedString::from(format!("ss-sel-{id}-{i}")))
                    .h(z(40.))
                    .pl(z(8.))
                    .pr(z(4.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                    .on_click(cx.listener(move |a, _, _, cx| {
                        f(a, i, cx);
                        with_state(|s| s.open_select = None);
                        cx.notify();
                    }))
                    .child(div().flex_1().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(o))
                    .when(cur == Some(i), |d| {
                        d.child(
                            gpui::svg().path(crate::icons::path("circle-check")).flex_none().size(z(20.)).text_color(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW))),
                        )
                    })
            }));
        wrap = wrap.child(gpui::deferred(list).with_priority(30));
    }
    wrap.into_any_element()
}

pub fn btn_sm(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>, kind: crate::ui::controls::BtnKind) -> Stateful<Div> {
    crate::ui::controls::button(id, label, kind).h(z(32.)).min_w(z(0.)).px(z(12.)).text_size(z(14.)).line_height(z(18.))
}

pub fn critical_secondary(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(z(32.))
        .px(z(12.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BORDER_DEFAULT)))
        .bg(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BACKGROUND_DEFAULT)))
        .hover(|s| {
            s.bg(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BACKGROUND_HOVER)))
                .border_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BORDER_HOVER)))
                .text_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_TEXT_HOVER)))
        })
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT)))
        .whitespace_nowrap()
        .child(label.into())
}

pub fn icon_btn(id: impl Into<gpui::ElementId>, icon: &'static str, size: f32, glyph: f32, critical: bool) -> Stateful<Div> {
    let fg = if critical { idx::CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT } else { idx::CONTROL_SECONDARY_TEXT_DEFAULT };
    div()
        .id(id.into())
        .flex_none()
        .size(z(size))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(if critical { idx::CONTROL_CRITICAL_SECONDARY_BORDER_DEFAULT } else { idx::CONTROL_SECONDARY_BORDER_DEFAULT })))
        .bg(c(tok(if critical { idx::CONTROL_CRITICAL_SECONDARY_BACKGROUND_DEFAULT } else { idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT })))
        .hover(move |s| s.bg(c(tok(if critical { idx::CONTROL_CRITICAL_SECONDARY_BACKGROUND_HOVER } else { idx::CONTROL_SECONDARY_BACKGROUND_HOVER }))))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(glyph)).text_color(c(tok(fg))))
}

pub fn search_box(input: Option<&gpui::Entity<gpui_component::input::InputState>>) -> Div {
    div()
        .h(z(40.))
        .pl(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .flex()
        .items_center()
        .child(
            div()
                .size(z(30.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("search")).size(z(16.)).text_color(c(tok(idx::INPUT_ICON_DEFAULT)))),
        )
        .child(div().flex_1().min_w_0().text_size(z(16.)).children(input.map(|i| gpui_component::input::Input::new(i).appearance(false).bordered(false))))
}

pub fn input_value(i: Option<&gpui::Entity<gpui_component::input::InputState>>, cx: &gpui::App) -> String {
    i.map(|i| i.read(cx).value().to_string()).unwrap_or_default()
}

pub fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned()
}

pub fn n(v: &Value, k: &str) -> u64 {
    match v.get(k) {
        Some(Value::String(x)) => x.parse().unwrap_or(0),
        Some(x) => x.as_u64().unwrap_or(0),
        None => 0,
    }
}

pub fn id_of(v: &Value, k: &str) -> Option<u64> {
    match v.get(k)? {
        Value::String(x) => x.parse().ok(),
        x => x.as_u64(),
    }
}

pub fn user_name(u: &Value) -> String {
    let g = s(u, "global_name");
    if g.is_empty() { s(u, "username") } else { g }
}

pub fn user_avatar(u: &Value, size: u32) -> SharedString {
    let id = s(u, "id");
    match u.get("avatar").and_then(Value::as_str) {
        Some(h) => format!("https://cdn.discordapp.com/avatars/{id}/{h}.webp?size={size}").into(),
        None => {
            let ix = id.parse::<u64>().map(|i| (i >> 22) % 6).unwrap_or(0);
            format!("https://cdn.discordapp.com/embed/avatars/{ix}.png").into()
        }
    }
}

pub fn avatar(url: SharedString, size: f32) -> AnyElement {
    gpui::img(url).size(z(size)).flex_none().rounded_full().into_any_element()
}

pub fn fmt_int(n: u64) -> String {
    let sep = if crate::i18n::locale().starts_with("fr") { '\u{202f}' } else { ',' };
    let d = n.to_string();
    let mut out = String::new();
    for (i, ch) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(ch);
    }
    out
}

pub fn snowflake_unix(id: u64) -> i64 {
    ((id >> 22) + 1_420_070_400_000) as i64 / 1000
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn json_helpers() {
        let v = json!({ "id": "41771983423143937", "n": 3, "global_name": null, "username": "ferris", "avatar": null });
        assert_eq!(super::n(&v, "id"), 41_771_983_423_143_937);
        assert_eq!(super::n(&v, "n"), 3);
        assert_eq!(super::user_name(&v), "ferris");
        assert!(super::user_avatar(&v, 32).starts_with("https://cdn.discordapp.com/embed/avatars/"));
        assert_eq!(super::snowflake_unix(41_771_983_423_143_937), 1_430_029_616);
    }
}

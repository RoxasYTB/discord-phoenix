use std::rc::Rc;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, deferred, div, prelude::*};

use crate::{
    actions::close_modal,
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
};

pub type Pick = Rc<dyn Fn(usize, &mut PhoenixApp, &mut Context<PhoenixApp>)>;

pub enum SideItem {
    Header(SharedString),
    ChannelHeader { icon: &'static str, name: SharedString, category: Option<SharedString> },
    Item { label: SharedString, index: usize, danger: bool, icon: Option<&'static str>, external: bool },
    Separator,
}

#[allow(clippy::too_many_arguments)]
fn side_item(n: usize, label: SharedString, index: usize, danger: bool, icon: Option<&'static str>, external: bool, on: bool, pick: Pick, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let fg = if danger {
        idx::TEXT_FEEDBACK_CRITICAL
    } else if on {
        idx::TEXT_STRONG
    } else {
        idx::TEXT_SUBTLE
    };
    let hover_fg = if danger { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_STRONG };
    let (hover_bg, active_bg) = if danger {
        (idx::BACKGROUND_FEEDBACK_MOD_CRITICAL, idx::BACKGROUND_FEEDBACK_MOD_CRITICAL)
    } else if on {
        (idx::BACKGROUND_MOD_STRONG, idx::BACKGROUND_MOD_STRONG)
    } else {
        (idx::BACKGROUND_MOD_SUBTLE, idx::BACKGROUND_MOD_STRONG)
    };
    div()
        .id(("settings-side", n))
        .group(SharedString::from(format!("settings-side-{n}")))
        .flex_none()
        .h(z(40.))
        .mb(z(4.))
        .p(z(10.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(10.))
        .overflow_hidden()
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(fg)))
        .when(!on, |d| d.cursor_pointer())
        .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
        .hover(move |s| s.bg(c(tok(hover_bg))).text_color(c(tok(hover_fg))))
        .active(move |s| {
            let s = s.bg(c(tok(active_bg)));
            if danger { s.text_color(c(tok(idx::WHITE))) } else { s }
        })
        .on_click(cx.listener(move |a, _, _, cx| {
            pick(index, a, cx);
            cx.notify();
        }))
        .children(icon.map(|i| {
            gpui::svg()
                .path(crate::icons::path(i))
                .flex_none()
                .size(z(20.))
                .text_color(c(tok(fg)))
                .group_hover(SharedString::from(format!("settings-side-{n}")), move |s| s.text_color(c(tok(hover_fg))))
        }))
        .child(div().flex_1().min_w_0().truncate().child(label))
        .when(external, |d| d.child(gpui::svg().path(crate::icons::path("viewer/open")).flex_none().size(z(16.)).text_color(c(tok(fg)))))
        .into_any_element()
}

fn side_header(icon: Option<&'static str>, name: SharedString, category: Option<SharedString>) -> AnyElement {
    div()
        .flex_none()
        .pt(z(10.))
        .px(z(10.))
        .pb(z(8.))
        .overflow_hidden()
        .child(
            div()
                .w_full()
                .overflow_hidden()
                .flex()
                .items_baseline()
                .min_w_0()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_MUTED)))
                .children(icon.map(|i| gpui::svg().path(crate::icons::path(i)).flex_none().self_center().mr(z(4.)).size(z(12.)).text_color(c(tok(idx::TEXT_MUTED)))))
                .child(div().flex_shrink(1.).min_w_0().truncate().child(name))
                .children(category.map(|cat| {
                    div()
                        .ml(z(4.))
                        .flex_shrink(1.)
                        .min_w_0()
                        .truncate()
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(cat)
                })),
        )
        .into_any_element()
}

pub fn close_tools(cx: &mut Context<PhoenixApp>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .group("settings-close")
        .child(
            div()
                .id("settings-close")
                .flex_none()
                .size(z(36.))
                .rounded_full()
                .border_2()
                .border_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))).border_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))).text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
                .active(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE))).border_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))).text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))))
                .on_click(cx.listener(|_, _, _, cx| {
                    close_modal();
                    cx.notify();
                }))
                .child(gpui::svg().path(crate::icons::path("close-small")).size(z(18.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).group_hover("settings-close", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))),
        )
        .child(
            div()
                .mt(z(6.))
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover("settings-close", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
                .child("ESC"),
        )
}

pub fn page_title(t: SharedString) -> gpui::Div {
    div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(t)
}

#[allow(clippy::too_many_arguments)]
pub fn overlay(id: &'static str, side_items: Vec<SideItem>, selected: usize, pick: Pick, title: SharedString, content: AnyElement, notice: Option<AnyElement>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    overlay_ex(id, side_items, selected, pick, title, content, notice, false, None, false, cx)
}

#[allow(clippy::too_many_arguments)]
pub fn overlay_fixed(id: &'static str, side_items: Vec<SideItem>, selected: usize, pick: Pick, content: AnyElement, notice: Option<AnyElement>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    overlay_ex(id, side_items, selected, pick, SharedString::default(), content, notice, true, None, true, cx)
}

#[allow(clippy::too_many_arguments)]
pub fn overlay_with(id: &'static str, side_items: Vec<SideItem>, selected: usize, pick: Pick, title: SharedString, content: AnyElement, notice: Option<AnyElement>, extra: Option<AnyElement>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    overlay_ex(id, side_items, selected, pick, title, content, notice, false, extra, false, cx)
}

#[allow(clippy::too_many_arguments)]
pub fn overlay_wide(id: &'static str, side_items: Vec<SideItem>, selected: usize, pick: Pick, content: AnyElement, notice: Option<AnyElement>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    overlay_ex(id, side_items, selected, pick, SharedString::default(), content, notice, true, None, false, cx)
}

#[allow(clippy::too_many_arguments)]
fn overlay_ex(
    id: &'static str,
    side_items: Vec<SideItem>,
    selected: usize,
    pick: Pick,
    title: SharedString,
    content: AnyElement,
    notice: Option<AnyElement>,
    wide: bool,
    extra: Option<AnyElement>,
    fixed: bool,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let mut side = div().flex_1().min_w_0().overflow_hidden().flex().flex_col();
    for (n, it) in side_items.into_iter().enumerate() {
        side = side.child(match it {
            SideItem::Header(h) => side_header(None, h, None),
            SideItem::ChannelHeader { icon, name, category } => side_header(Some(icon), name, category),
            SideItem::Separator => div().flex_none().mx(z(10.)).my(z(8.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))).into_any_element(),
            SideItem::Item { label, index, danger, icon, external } => side_item(n, label, index, danger, icon, external, index == selected, pick.clone(), cx),
        });
    }
    let narrow = is_narrow(crate::ui::title_bar::viewport_w_css());
    let column = |body: Option<AnyElement>| {
        div()
            .when(!narrow, |d| d.flex_grow(1.).flex_shrink(1.).min_w(z(460.)).when(!wide, |d| d.max_w(z(740.))))
            .when(narrow, |d| d.flex_none().w_full())
            .when_some(body, |d, b| if wide { d.child(b) } else { d.pt(z(60.)).px(z(40.)).pb(z(80.)).child(b) })
    };
    let body = if fixed {
        content
    } else {
        div().flex().flex_col().gap(z(24.)).when(!title.is_empty(), |d| d.child(page_title(title))).child(content).into_any_element()
    };
    let scroller = if fixed {
        div().id(SharedString::from(format!("{id}-scroll"))).size_full().flex().child(div().h_full().flex_1().min_w_0().child(body))
    } else {
        div()
            .id(SharedString::from(format!("{id}-scroll")))
            .size_full()
            .flex()
            .items_start()
            .overflow_y_scroll()
            .track_scroll(&crate::ui::chat::scrollbar::handle_n(id, 1))
            .when(narrow, |d| d.pr(z(16.)))
            .child(column(Some(body)))
            .when(!narrow, |d| d.child(div().flex_none().w(z(36.)).mr(z(21.))))
    };
    let tools = if narrow {
        div().absolute().top_0().right(z(16.)).w(z(36.)).pt(z(60.)).child(close_tools(cx))
    } else {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_start()
            .child(column(None))
            .child(div().flex_none().w(z(36.)).mr(z(21.)).pt(z(60.)).child(close_tools(cx)))
    };
    let content_region = div()
        .id(SharedString::from(format!("{id}-main")))
        .relative()
        .flex_grow(1.)
        .flex_shrink(1.)
        .flex_basis(if narrow { gpui::Length::from(gpui::relative(0.7)) } else { gpui::Length::from(z(800.)) })
        .min_w_0()
        .h_full()
        .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
        .child(scroller)
        .child(tools)
        .when(!fixed, |d| d.children(crate::ui::chat::scrollbar::bar_for_n(id, 1, crate::ui::chat::scrollbar::Bar::Auto)))
        .children(notice.map(|n| div().absolute().bottom_0().left_0().right(z(71.)).max_w(z(740.)).px(z(20.)).pb(z(20.)).child(n)));
    let sidebar_region = crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::ThinFade)
        .flex_grow(1.)
        .when(!narrow, |d| d.flex_shrink_0().flex_basis(z(264.)))
        .when(narrow, |d| d.flex_shrink(1.).flex_basis(gpui::relative(0.3)).min_w_0())
        .h_full()
        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
        .child(
            div()
                .id(SharedString::from(format!("{id}-side")))
                .size_full()
                .flex()
                .justify_end()
                .items_start()
                .pr(z(8.))
                .overflow_y_scroll()
                .track_scroll(&crate::ui::chat::scrollbar::handle_n(id, 2))
                .child(div().flex_none().when(narrow, |d| d.w_full()).when(!narrow, |d| d.w(z(264.))).py(z(60.)).pl(z(20.)).pr(z(6.)).flex().child(side)),
        )
        .children(crate::ui::chat::scrollbar::bar_for_n(id, 2, crate::ui::chat::scrollbar::Bar::ThinFade));
    deferred(crate::ui::popout_motion::layer(
        id,
        div()
            .id(id)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .flex()
            .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
            .child(sidebar_region)
            .child(content_region)
            .children(extra),
    ))
    .with_priority(20)
    .into_any_element()
}

pub fn is_narrow(viewport_css: f32) -> bool {
    viewport_css > 0. && viewport_css < 780.
}

pub type Act = Rc<dyn Fn(&mut PhoenixApp, &mut Window, &mut Context<PhoenixApp>)>;

pub fn unsaved_bar(reset: Act, save: Act, cx: &mut Context<PhoenixApp>) -> AnyElement {
    unsaved_bar_with(BarText::default(), reset, Some(save), cx)
}

#[derive(Default)]
pub struct BarText {
    pub message: Option<SharedString>,
    pub error: Option<SharedString>,
    pub save: Option<SharedString>,
    pub save_disabled: bool,
}

pub fn unsaved_bar_with(text: BarText, reset: Act, save: Option<Act>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let critical = text.error.is_some();
    let message = text.error.or(text.message).unwrap_or_else(|| crate::t!("admin.unsaved"));
    let save_label = text.save.unwrap_or_else(|| crate::t!("admin.save_changes"));
    let save_disabled = text.save_disabled;
    div()
        .pt(z(10.))
        .pb(z(10.))
        .pl(z(16.))
        .pr(z(10.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST)))
        .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.2), offset: gpui::point(z(0.), z(2.)), blur_radius: z(10.), spread_radius: z(0.), inset: false }])
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex_shrink(1.)
                .min_w_0()
                .truncate()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(if critical { idx::TEXT_FEEDBACK_CRITICAL } else { idx::WHITE })))
                .child(message),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .justify_end()
                .gap(z(10.))
                .child(
                    div()
                        .id("unsaved-reset")
                        .h(z(32.))
                        .flex()
                        .items_center()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::BRAND_360)))
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .on_click(cx.listener(move |a, _, w, cx| {
                            reset(a, w, cx);
                            cx.notify();
                        }))
                        .child(crate::t!("admin.reset")),
                )
                .children(save.map(|save| {
                    div()
                        .id("unsaved-save")
                        .h(z(32.))
                        .px(z(12.))
                        .rounded(z(8.))
                        .border_1()
                        .border_color(c(tok(idx::CONTROL_CONNECTED_BORDER_DEFAULT)))
                        .bg(c(tok(idx::CONTROL_CONNECTED_BACKGROUND_DEFAULT)))
                        .hover(|s| s.bg(c(tok(idx::CONTROL_CONNECTED_BACKGROUND_HOVER))))
                        .active(|s| s.bg(c(tok(idx::CONTROL_CONNECTED_BACKGROUND_ACTIVE))))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::WHITE)))
                        .when(save_disabled, |d| d.opacity(0.5).cursor_not_allowed())
                        .when(!save_disabled, |d| {
                            d.on_click(cx.listener(move |a, _, w, cx| {
                                save(a, w, cx);
                                cx.notify();
                            }))
                        })
                        .child(save_label)
                })),
        )
        .into_any_element()
}

pub fn eyebrow(t: SharedString) -> gpui::Div {
    div().pb(z(8.)).text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::BOLD).text_color(c(tok(idx::TEXT_SUBTLE))).child(t.to_uppercase())
}

pub fn divider() -> gpui::Div {
    div().my(z(20.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))
}

pub fn link_btn(id: impl Into<gpui::ElementId>, t: SharedString, danger: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .px(z(8.))
        .py(z(4.))
        .rounded(z(4.))
        .text_size(z(14.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .text_color(c(tok(if danger { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_LINK })))
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .child(t)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_split_layout_starts_below_780_css_px() {
        assert!(super::is_narrow(779.));
        assert!(super::is_narrow(640.));
        assert!(!super::is_narrow(780.));
        assert!(!super::is_narrow(1280.));
        assert!(!super::is_narrow(0.));
    }
}

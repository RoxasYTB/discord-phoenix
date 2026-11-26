use crate::theme::z;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::{app::PhoenixApp, model::Id};

pub fn render(app: &PhoenixApp, guild: Id, top: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let items = crate::ui::nav_menus::guild_dropdown(app, guild, cx);
    render_items(items, top, cx)
}

pub fn render_favorites(top: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let items = crate::ui::nav_menus::favorites_items(&cx.entity());
    render_items(items, top, cx)
}

fn render_items(items: Vec<crate::ui::context_menu::MenuItem>, top: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let dismiss: crate::ui::context_menu::Dismiss = std::rc::Rc::new(move |_, cx| {
        e.update(cx, |a, cx| {
            a.nav.guild_menu = false;
            cx.notify();
        })
    });
    div()
        .id("guild-menu-backdrop")
        .absolute()
        .inset_0()
        .on_click(cx.listener(|a, _, _, cx| {
            a.nav.guild_menu = false;
            cx.notify();
        }))
        .child(
            div().absolute().top(px(top)).left_0().right_0().flex().justify_center().child(
                div()
                    .w(z(220.))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(crate::ui::context_menu::panel("guild-menu", items, dismiss)),
            ),
        )
        .into_any_element()
}

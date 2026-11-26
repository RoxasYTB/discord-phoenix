use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::InputState;

use super::{Modal, card, input_box};
use crate::{
    actions::{close_modal, with_modal},
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui::controls::{self, BtnKind, button},
};

pub struct VoicePick {
    pub keybind: u64,
    pub search: Option<Entity<InputState>>,
}

pub struct VoiceSwitch {
    pub channel: Id,
    pub dont_ask: bool,
}

fn is_voice(k: ChannelKind) -> bool {
    matches!(k, ChannelKind::Voice | ChannelKind::Stage)
}

pub fn results(app: &PhoenixApp, q: &str) -> Vec<(Id, SharedString, SharedString)> {
    let q = q.trim().to_lowercase();
    let mut out = Vec::new();
    for g in &app.guilds {
        if q.is_empty() && app.selected_guild != Some(g.id) {
            continue;
        }
        for id in app.guild_channels.get(&g.id).into_iter().flatten() {
            if let Some(ch) = app.channels.get(id).filter(|c| is_voice(c.kind)).filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q)) {
                out.push((*id, ch.name.clone(), g.name.clone()));
            }
        }
    }
    out
}

pub fn channel_row(app: &PhoenixApp, id: Id, name: SharedString, guild: SharedString) -> gpui::Div {
    let kind = app.channels.get(&id).map_or(ChannelKind::Voice, |c| c.kind);
    div()
        .h(z(34.))
        .px(z(8.))
        .flex()
        .items_center()
        .gap(z(8.))
        .child(crate::ui::icon(crate::ui::sidebar::channel_icon(kind), 20., c(tok(idx::CHANNEL_ICON))))
        .child(div().flex_shrink_0().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(name))
        .child(div().flex_shrink(1.).min_w_0().truncate().text_size(z(12.)).text_color(c(tok(idx::TEXT_MUTED))).child(guild))
}

pub fn render_pick(app: &PhoenixApp, p: &VoicePick, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let q = p.search.as_ref().map(|s| s.read(cx).value().to_string()).unwrap_or_default();
    let keybind = p.keybind;
    let rows = results(app, &q);
    let mut list = div().id("vpick-list").max_h(z(320.)).overflow_y_scroll().flex().flex_col();
    if rows.is_empty() {
        list = list.child(div().p(z(16.)).text_center().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("kb.no_channel")));
    }
    for (id, name, guild) in rows {
        list = list.child(
            div()
                .id(("vpick-row", id.0))
                .rounded(z(4.))
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .on_click(cx.listener(move |a, _, _, cx| {
                    close_modal();
                    crate::keybinds::edit(a, keybind, |b| b.channel = Some(id.0), cx);
                }))
                .child(channel_row(app, id, name, guild)),
        );
    }
    let mut body = div().flex().flex_col().gap(z(12.));
    if let Some(s) = &p.search {
        body = body.child(input_box(s));
    }
    card(440., crate::t!("kb.pick_channel"), Some(crate::t!("kb.channel")), body.child(list).into_any_element(), None, cx)
}

fn set_dont_ask(app: &mut PhoenixApp, on: bool, cx: &mut Context<PhoenixApp>) {
    with_modal(|m| {
        if let Some(Modal::VoiceSwitch(s)) = m {
            s.dont_ask = on;
        }
    });
    crate::ui::settings::update_local(app, |p| {
        p.ui.insert("vo-switch-warn".into(), u64::from(!on));
    }, cx);
}

pub fn render_switch(app: &PhoenixApp, s: &VoiceSwitch, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = app.channels.get(&s.channel).map(|c| c.name.to_string()).unwrap_or_default();
    let channel = s.channel;
    let on = s.dont_ask;
    let footer = div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap(z(8.))
        .child(controls::checkbox_option("vswitch-dont-ask", crate::t!("vc.switch_dont_ask"), on).on_click(cx.listener(move |a, _, _, cx| set_dont_ask(a, !on, cx))))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(button("vswitch-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| {
                    close_modal();
                    cx.notify();
                })))
                .child(button("vswitch-ok", crate::t!("vc.switch_confirm"), BtnKind::Primary).on_click(cx.listener(move |a, _, _, cx| {
                    close_modal();
                    a.enter_voice_channel(channel, cx);
                }))),
        );
    card(480., crate::t!("vc.switch_title"), Some(crate::t!("vc.switch_body", channel = name)), div().into_any_element(), Some(footer.into_any_element()), cx)
}

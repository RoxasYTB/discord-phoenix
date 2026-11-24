use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::{Input, InputState};

use super::{Modal, card, close_action, label};
use crate::{
    actions::with_modal,
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button, radio_indicator, switch},
};

pub struct State {
    pub guild: Id,
    pub parent: Option<Id>,
    pub kind: ChannelKind,
    pub private: bool,
    pub name: Option<Entity<InputState>>,
}

impl State {
    pub fn new(guild: Id, parent: Option<Id>, kind: ChannelKind) -> Self {
        Self { guild, parent, kind, private: false, name: None }
    }
}

pub const KINDS: [(ChannelKind, &str, &str, &str); 5] = [
    (ChannelKind::Text, "admin.kind_text", "admin.kind_text_desc", "hash"),
    (ChannelKind::Voice, "admin.kind_voice", "admin.kind_voice_desc", "speaker"),
    (ChannelKind::Forum, "admin.kind_forum", "admin.kind_forum_desc", "forum"),
    (ChannelKind::Announcement, "admin.kind_announcement", "admin.kind_announcement_desc", "megaphone"),
    (ChannelKind::Stage, "admin.kind_stage", "admin.kind_stage_desc", "stage"),
];

pub fn icon_for(k: ChannelKind) -> &'static str {
    KINDS.iter().find(|x| x.0 == k).map_or("hash", |x| x.3)
}

fn with_state(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::CreateChannel(s)) = m {
            f(s)
        }
    });
}

pub fn prepare(s: &mut State, mk: &mut dyn FnMut(SharedString, &str, bool) -> Entity<InputState>) {
    if s.name.is_none() {
        let ph = if s.kind == ChannelKind::Category { crate::t!("admin.category_name_ph") } else { crate::t!("admin.channel_name_ph") };
        s.name = Some(mk(ph, "", true));
    }
}

fn icon(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).size(z(size)).text_color(color)
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let cat = s.kind == ChannelKind::Category;
    let mut body = div().flex().flex_col().gap(z(20.));
    if !cat {
        let mut list = div().flex().flex_col().gap(z(8.)).child(label(crate::t!("admin.channel_type")));
        for (n, (k, title, desc, ic)) in KINDS.iter().enumerate() {
            let (k, on) = (*k, *k == s.kind);
            list = list.child(
                div()
                    .id(("cc-kind", n))
                    .flex()
                    .items_center()
                    .gap(z(12.))
                    .px(z(12.))
                    .py(z(10.))
                    .rounded(z(8.))
                    .cursor_pointer()
                    .bg(c(tok(if on { idx::BACKGROUND_MOD_STRONG } else { idx::BACKGROUND_MOD_SUBTLE })))
                    .hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        with_state(|s| s.kind = k);
                        cx.notify();
                    }))
                    .child(icon(ic, 24., c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!(title)))
                            .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!(desc))),
                    )
                    .child(radio_indicator(on)),
            );
        }
        body = body.child(list);
    }
    if let Some(i) = &s.name {
        body = body.child(
            div().flex().flex_col().child(label(crate::t!(if cat { "admin.category_name" } else { "admin.channel_name" }))).child(
                div()
                    .h(z(40.))
                    .px(z(12.))
                    .gap(z(8.))
                    .rounded(z(8.))
                    .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                    .border_1()
                    .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
                    .flex()
                    .items_center()
                    .text_size(z(16.))
                    .when(!cat, |d| d.child(icon(icon_for(s.kind), 16., c(tok(idx::ICON_SUBTLE)))))
                    .child(Input::new(i).appearance(false).bordered(false)),
            ),
        );
    }
    let private = s.private;
    body = body.child(
        div()
            .id("cc-private")
            .flex()
            .items_center()
            .gap(z(8.))
            .cursor_pointer()
            .on_click(cx.listener(|_, _, _, cx| {
                with_state(|s| s.private = !s.private);
                cx.notify();
            }))
            .child(icon("lock", 18., c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
            .child(
                div()
                    .flex_1()
                    .text_size(z(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .child(crate::t!(if cat { "admin.private_category" } else { "admin.private_channel" })),
            )
            .group(crate::ui::controls::SWITCH_ROW)
            .child(switch("private-switch", private)),
    );
    body = body.child(div().mt(z(-12.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!(if cat {
        "admin.private_category_desc"
    } else {
        "admin.private_channel_desc"
    })));
    let footer = div()
        .flex()
        .gap(z(8.))
        .child(button("cc-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click({
            let close = close_action(cx);
            move |_, w, cx| close(w, cx)
        }))
        .child(
            button("cc-create", crate::t!(if cat { "admin.create_category" } else { "admin.create_channel" }), BtnKind::Primary).on_click(cx.listener(|a, _, _, cx| {
                let taken = with_modal(|m| match m.take() {
                    Some(Modal::CreateChannel(s)) => Some(s),
                    other => {
                        *m = other;
                        None
                    }
                });
                if let Some(s) = taken {
                    let name = s.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
                    a.create_channel(s.guild, s.parent, s.kind, name, s.private, cx);
                }
                cx.notify();
            })),
        );
    let sub = s.parent.and_then(|p| app.channels.get(&p)).filter(|_| !cat).map(|p| crate::t!("admin.in_category", name = p.name.to_string()));
    card(460., crate::t!(if cat { "admin.create_category" } else { "admin.create_channel" }), sub, body.into_any_element(), Some(footer.into_any_element()), cx)
}

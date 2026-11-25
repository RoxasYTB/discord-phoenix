use std::rc::Rc;

use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::{Input, InputState};
use serde_json::Value;

use super::{Modal, card, close_action};
use crate::{
    actions::{admin::sf, with_modal},
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
    pub step: u8,
    pub picked: Vec<(Id, u8)>,
    pub search: Option<Entity<InputState>>,
    pub roles: Option<Vec<Value>>,
}

impl State {
    pub fn new(guild: Id, parent: Option<Id>, kind: ChannelKind) -> Self {
        Self { guild, parent, kind, private: false, name: None, step: 0, picked: Vec::new(), search: None, roles: None }
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
    if s.step == 1 && s.search.is_none() {
        s.search = Some(mk(crate::t!("cs.add_members_ph"), "", true));
    }
}

fn icon(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).flex_none().size(z(size)).text_color(color)
}

fn text(t: impl Into<SharedString>, size: f32, line: f32, color: usize) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).text_color(c(tok(color))).child(t.into())
}

fn field_label(t: SharedString) -> gpui::Div {
    text(t, 16., 20., idx::TEXT_STRONG).font_weight(FontWeight::MEDIUM)
}

fn private_icon(kind: ChannelKind, private: bool) -> &'static str {
    if kind == ChannelKind::Category {
        return "folder";
    }
    crate::ui::modals::channel_settings::channel_icon(kind, private)
}

fn submit(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let taken = with_modal(|m| match m.take() {
        Some(Modal::CreateChannel(s)) => Some(s),
        other => {
            *m = other;
            None
        }
    });
    if let Some(s) = taken {
        let name = s.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
        a.create_channel(s.guild, s.parent, s.kind, name, s.private, s.picked, cx);
    }
    cx.notify();
}

fn members_step(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let toggle: crate::ui::modals::channel_perms::Toggle = {
        let e = e.clone();
        Rc::new(move |id, kind, cx| {
            with_state(|s| {
                if let Some(i) = s.picked.iter().position(|p| p.0 == id) {
                    s.picked.remove(i);
                } else {
                    s.picked.push((id, kind));
                }
            });
            e.update(cx, |_, cx| cx.notify());
        })
    };
    let name = s.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
    let header = div()
        .flex()
        .items_center()
        .gap(z(4.))
        .mt(z(-24.))
        .mb(z(24.))
        .child(icon(private_icon(s.kind, true), 20., c(tok(idx::TEXT_SUBTLE))))
        .child(text(crate::actions::admin::channel_name(s.kind, &name), 16., 20., idx::TEXT_SUBTLE))
        .into_any_element();
    let body = crate::ui::modals::channel_perms::picker_body(app, Some(s.guild), s.roles.as_deref(), &[], &s.picked, s.search.as_ref(), header, toggle, cx);
    let back = cx.listener(|_, _, _, cx| {
        with_state(|s| {
            s.step = 0;
            s.search = None;
        });
        cx.notify();
    });
    let done = cx.listener(|a, _, _, cx| submit(a, cx));
    let footer = div().w_full().flex().gap(z(8.)).child(button("cc-back", crate::t!("cc.back"), BtnKind::Secondary).flex_1().on_click(back)).child(
        button("cc-skip", if s.picked.is_empty() { crate::t!("cc.skip") } else { crate::t!("admin.create_channel") }, BtnKind::Primary).flex_1().on_click(done),
    );
    card(480., crate::t!("cs.add_members"), None, body, Some(footer.into_any_element()), cx)
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if s.step == 1 {
        return members_step(app, s, cx);
    }
    let cat = s.kind == ChannelKind::Category;
    let mut body = div().flex().flex_col().gap(z(20.)).pb(z(12.));
    if !cat {
        let mut list = div().pt(z(8.)).pb(z(4.)).flex().flex_col().gap(z(16.));
        for (n, (k, title, desc, ic)) in KINDS.iter().enumerate() {
            let (k, on) = (*k, *k == s.kind);
            list = list.child(
                div()
                    .id(("cc-kind", n))
                    .relative()
                    .pl(z(56.))
                    .rounded(z(8.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        with_state(|s| s.kind = k);
                        cx.notify();
                    }))
                    .child(div().absolute().top_0().left_0().child(radio_indicator(on)))
                    .child(div().absolute().top_0().left(z(32.)).child(icon(ic, 20., c(tok(idx::TEXT_DEFAULT)))))
                    .child(text(crate::t!(title), 16., 20., idx::TEXT_DEFAULT))
                    .child(text(crate::t!(desc), 14., 18., idx::TEXT_SUBTLE).mt(z(4.)).ml(z(-24.))),
            );
        }
        body = body.child(div().flex().flex_col().child(field_label(crate::t!("admin.channel_type")).mb(z(8.))).child(list));
    }
    if let Some(i) = &s.name {
        body = body.child(
            div().flex().flex_col().child(field_label(crate::t!(if cat { "admin.category_name" } else { "admin.channel_name" })).mb(z(8.))).child(
                div()
                    .h(z(40.))
                    .px(z(4.))
                    .gap(z(4.))
                    .rounded(z(8.))
                    .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                    .border_1()
                    .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
                    .flex()
                    .items_center()
                    .text_size(z(16.))
                    .when(!cat, |d| {
                        d.child(div().size(z(30.)).flex().items_center().justify_center().child(icon(
                            private_icon(s.kind, s.private),
                            20.,
                            c(tok(idx::TEXT_DEFAULT)),
                        )))
                    })
                    .child(div().flex_1().min_w_0().when(cat, |d| d.pl(z(8.))).child(Input::new(i).appearance(false).bordered(false))),
            ),
        );
    }
    let private = s.private;
    body = body.child(
        div()
            .id("cc-private")
            .flex()
            .items_start()
            .gap(z(24.))
            .cursor_pointer()
            .group(crate::ui::controls::SWITCH_ROW)
            .on_click(cx.listener(|_, _, _, cx| {
                with_state(|s| s.private = !s.private);
                cx.notify();
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(z(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(4.))
                            .child(icon("lock", 16., c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                            .child(field_label(crate::t!(if cat { "admin.private_category" } else { "admin.private_channel" }))),
                    )
                    .child(text(crate::t!(if cat { "admin.private_category_desc" } else { "admin.private_channel_desc" }), 14., 18., idx::TEXT_SUBTLE)),
            )
            .child(switch("private-switch", private)),
    );
    if !cat {
        body = body.child(
            div()
                .flex()
                .items_start()
                .gap(z(24.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(z(4.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(z(4.))
                                .child(icon("settings/globe", 16., c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                                .child(field_label(crate::t!("cc.discoverable"))),
                        )
                        .child(text(crate::t!("cc.discoverable_desc"), 14., 18., idx::TEXT_SUBTLE)),
                )
                .child(text(crate::t!("cc.none"), 16., 20., idx::TEXT_MUTED)),
        );
    }
    let named = s.name.as_ref().is_some_and(|i| !i.read(cx).value().trim().is_empty());
    let next = private && !cat;
    let guild = s.guild;
    let go = cx.listener(move |a, _, _, cx| {
        if next {
            let mut need = false;
            with_state(|s| {
                s.step = 1;
                need = s.roles.is_none();
            });
            if need {
                a.admin(phoenix_api::rest::guild_manage::AdminCall::Roles { guild: sf(guild) }, cx, |_, r, cx| {
                    let list = r.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
                    with_state(|s| s.roles = Some(list));
                    cx.notify();
                });
            }
            cx.notify();
        } else {
            submit(a, cx);
        }
    });
    let label = if next { crate::t!("cc.next") } else { crate::t!(if cat { "admin.create_category" } else { "admin.create_channel" }) };
    let footer = div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("cc-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click({
            let close = close_action(cx);
            move |_, w, cx| close(w, cx)
        }))
        .child(button("cc-create", label, BtnKind::Primary).flex_1().when(!named, |b| b.opacity(0.5)).when(named, |b| b.on_click(go)));
    let sub = s.parent.and_then(|p| app.channels.get(&p)).filter(|_| !cat).map(|p| crate::t!("admin.in_category", name = p.name.to_string()));
    card(480., crate::t!(if cat { "admin.create_category" } else { "admin.create_channel" }), sub, body.into_any_element(), Some(footer.into_any_element()), cx)
}

#[cfg(test)]
mod tests {
    #[test]
    fn kinds_follow_the_client_order() {
        let names: Vec<&str> = super::KINDS.iter().map(|k| k.3).collect();
        assert_eq!(names, ["hash", "speaker", "forum", "megaphone", "stage"]);
    }
}

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_server::ServerCall;
use serde_json::Value;

use super::{fetch, icon, label, page, sf, str_of, t, text, title, write};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button},
};

fn column(head: SharedString, rows: &[&str], yes: bool) -> gpui::Div {
    let mut d = div().flex_1().flex().flex_col().child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, head));
    for r in rows {
        d = d.child(
            div()
                .mt(z(16.))
                .flex()
                .gap(z(8.))
                .child(icon(if yes { "check" } else { "close-small" }, 20., if yes { idx::TEXT_FEEDBACK_POSITIVE } else { idx::TEXT_FEEDBACK_CRITICAL }))
                .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!(r))),
        );
    }
    d
}

fn input(v: SharedString, h: f32, placeholder: bool) -> gpui::Div {
    div()
        .mt(z(8.))
        .h(z(h))
        .px(z(12.))
        .py(z(9.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::INPUT_BORDER_DEFAULT))
        .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
        .child(text(16., 20., FontWeight::NORMAL, if placeholder { idx::TEXT_MUTED } else { idx::TEXT_DEFAULT }, v))
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let list = fetch(app, guild, ServerCall::Templates { guild: sf(guild) }, cx);
    let p = page()
        .child(title(crate::t!("menu.gs.templates")))
        .child(div().mt(z(0.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.tpl_lead1"))))
        .child(div().mt(z(14.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.tpl_lead2"))))
        .child(
            div()
                .mt(z(20.))
                .p(z(16.))
                .rounded(z(8.))
                .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                .flex()
                .child(column(crate::t!("gs2.tpl_copies"), &["gs2.tpl_c1", "gs2.tpl_c2", "gs2.tpl_c3"], true))
                .child(column(crate::t!("gs2.tpl_skips"), &["gs2.tpl_s1", "gs2.tpl_s2", "gs2.tpl_s3"], false)),
        );
    let Some(list) = list else { return p.child(super::loading()).into_any_element() };
    let existing: Option<Value> = list.as_array().and_then(|a| a.first().cloned());
    let mut p = p.child(div().mt(z(40.)).flex().gap(z(4.)).child(label(crate::t!("gs2.tpl_title"))).child(text(
        16.,
        20.,
        FontWeight::NORMAL,
        idx::TEXT_FEEDBACK_CRITICAL,
        "*",
    )));
    match existing {
        Some(tpl) => {
            let code = str_of(&tpl, "code").to_owned();
            let (c1, c2) = (code.clone(), code.clone());
            p = p
                .child(input(str_of(&tpl, "name").to_owned().into(), 40., false))
                .child(div().mt(z(20.)).child(label(crate::t!("gs2.tpl_desc"))))
                .child(input(str_of(&tpl, "description").to_owned().into(), 88., false))
                .child(div().mt(z(20.)).child(label(crate::t!("gs2.tpl_link"))))
                .child(input(format!("https://discord.new/{code}").into(), 40., false))
                .child(
                    div()
                        .mt(z(20.))
                        .flex()
                        .gap(z(8.))
                        .child(button("gs2-tpl-sync", crate::t!("gs2.tpl_sync"), BtnKind::Primary).on_click(cx.listener(move |a, _, _, cx| {
                            write(a, ServerCall::SyncTemplate { guild: sf(guild), code: c1.clone() }, None, cx);
                        })))
                        .child(button("gs2-tpl-delete", crate::t!("gs2.tpl_delete"), BtnKind::Critical).on_click(cx.listener(move |a, _, _, cx| {
                            write(a, ServerCall::DeleteTemplate { guild: sf(guild), code: c2.clone() }, None, cx);
                        }))),
                );
        }
        None => {
            let ok = super::value(guild, "tpl-name", cx).is_some_and(|n| !n.trim().is_empty());
            p = p
                .child(super::field(guild, "tpl-name", "", cx).mt(z(8.)))
                .child(div().mt(z(20.)).child(label(crate::t!("gs2.tpl_desc"))))
                .child(super::field(guild, "tpl-desc", "", cx).mt(z(8.)))
                .child(div().mt(z(20.)).flex().child(
                    button("gs2-tpl-create", crate::t!("gs2.tpl_generate"), BtnKind::Primary).when(!ok, |b| b.opacity(0.5)).on_click(cx.listener(
                        move |a, _, _, cx| {
                            let name = super::value(guild, "tpl-name", cx).unwrap_or_default();
                            let description = super::value(guild, "tpl-desc", cx).unwrap_or_default();
                            if !name.trim().is_empty() {
                                write(a, ServerCall::CreateTemplate { guild: sf(guild), name, description }, None, cx);
                            }
                        },
                    )),
                ));
        }
    }
    p.into_any_element()
}

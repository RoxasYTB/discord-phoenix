use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::{
    models::Snowflake,
    rest::guild_server::{CommandPermission, ServerCall},
};
use serde_json::Value;

use super::{Sub, divider, fetch, icon, lead, link, page, set_sub, sf, str_of, sub, t, text, title, write};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button_sm},
};

pub const SECTION: &str = "menu.gs.integrations";

#[derive(Clone, Debug, PartialEq)]
pub struct App {
    pub id: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub added: String,
    pub by: String,
    pub verified: bool,
    pub bot: bool,
    pub commands: bool,
    pub webhooks: usize,
    pub description: String,
    pub integration: String,
}

pub fn apps(integrations: &Value, webhooks: &Value) -> Vec<App> {
    integrations
        .as_array()
        .into_iter()
        .flatten()
        .filter(|i| str_of(i, "type") == "discord")
        .filter_map(|i| {
            let a = i.get("application")?;
            let id = str_of(a, "id").to_owned();
            let icon_url = a.get("icon").and_then(Value::as_str).map(|h| format!("https://cdn.discordapp.com/app-icons/{id}/{h}.png?size=96")).or_else(|| {
                let b = a.get("bot")?;
                b.get("avatar").and_then(Value::as_str).map(|h| format!("https://cdn.discordapp.com/avatars/{}/{h}.png?size=96", str_of(b, "id")))
            });
            let scopes: Vec<&str> = i.get("scopes").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
            let webhooks = webhooks.as_array().into_iter().flatten().filter(|w| str_of(w, "application_id") == id).count();
            let user = i.get("user");
            Some(App {
                name: str_of(a, "name").to_owned(),
                icon_url,
                added: crate::ui::modals::webhooks::snowflake_date(str_of(i, "id").parse().unwrap_or(0)),
                by: user.map(|u| str_of(u, "username").to_owned()).unwrap_or_default(),
                verified: a.get("is_verified").and_then(Value::as_bool).unwrap_or(false),
                bot: a.get("bot").is_some() || scopes.contains(&"bot"),
                commands: scopes.contains(&"applications.commands"),
                webhooks,
                description: str_of(a, "description").to_owned(),
                integration: str_of(i, "id").to_owned(),
                id,
            })
        })
        .collect()
}

fn app_icon(url: Option<&str>, size: f32) -> AnyElement {
    match url {
        Some(u) => crate::media::avatar_image(SharedString::from(u.to_owned()), size),
        None => div().size(z(size)).rounded_full().bg(t(idx::BACKGROUND_SURFACE_HIGH)).into_any_element(),
    }
}

fn icon_tile(name: &str) -> gpui::Div {
    div().size(z(48.)).flex_none().rounded_full().bg(t(idx::CARD_BACKGROUND_DEFAULT)).flex().items_center().justify_center().child(icon(name, 24., idx::ICON_SUBTLE))
}

fn card_row(id: impl Into<gpui::ElementId>, lead_el: AnyElement, head: SharedString, sub: gpui::Div, trailing: AnyElement) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .mb(z(16.))
        .px(z(16.))
        .py(z(20.))
        .rounded(z(8.))
        .bg(t(idx::CARD_BACKGROUND_DEFAULT))
        .flex()
        .items_center()
        .gap(z(8.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(z(16.))
                .child(lead_el)
                .child(div().flex_1().min_w_0().flex().flex_col().child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, head).truncate()).child(sub)),
        )
        .child(trailing)
}

fn tag(name: &str, label: SharedString) -> gpui::Div {
    div()
        .px(z(8.))
        .py(z(4.))
        .rounded(z(4.))
        .bg(t(idx::BACKGROUND_MOD_SUBTLE))
        .border_1()
        .border_color(t(idx::BORDER_MUTED))
        .flex()
        .items_center()
        .gap(z(4.))
        .child(icon(name, 16., idx::TEXT_DEFAULT))
        .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, label))
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    match sub(guild, SECTION) {
        Sub::Webhooks => webhooks(app, guild, cx),
        Sub::App(id) => app_page(app, guild, &id, cx),
        _ => overview(app, guild, cx),
    }
}

fn overview(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let ints = fetch(app, guild, ServerCall::Integrations { guild: sf(guild) }, cx);
    let hooks = fetch(app, guild, ServerCall::Webhooks { guild: sf(guild) }, cx);
    let count = |kind: u64| hooks.as_ref().and_then(Value::as_array).into_iter().flatten().filter(|w| w.get("type").and_then(Value::as_u64) == Some(kind)).count();
    let (incoming, followed) = (count(1), count(2));
    let mut p = page()
        .child(title(crate::t!("menu.gs.integrations")))
        .child(div().mt(z(20.)).child(lead(crate::t!("gs2.int_lead"))))
        .child(link(crate::t!("gs2.int_learn")))
        .child(divider(32.))
        .child(
            card_row(
                "gs2-int-webhooks",
                icon_tile("link").into_any_element(),
                crate::t!("gs2.webhooks"),
                text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.n_webhooks", n = incoming)),
                icon("chevron-right", 16., idx::TEXT_DEFAULT).into_any_element(),
            )
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, _, cx| {
                set_sub(guild, SECTION, Sub::Webhooks);
                cx.notify();
            })),
        )
        .child(card_row(
            "gs2-int-followed",
            icon_tile("megaphone").into_any_element(),
            crate::t!("gs2.followed"),
            text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.n_channels", n = followed)),
            button_sm("gs2-int-followed-more", crate::t!("gs2.learn_more"), BtnKind::Primary).into_any_element(),
        ));
    for (n, (name, desc, cta, col)) in
        [("Twitch", "gs2.twitch_desc", "gs2.twitch_connect", 0x9146FFu32), ("YouTube", "gs2.youtube_desc", "gs2.youtube_connect", 0xE3302Fu32)].into_iter().enumerate()
    {
        p = p.child(card_row(
            ("gs2-int-platform", n),
            div().size(z(48.)).rounded_full().bg(crate::theme::c(col)).flex().items_center().justify_center().child(icon(if n == 0 { "tv" } else { "play" }, 28., idx::WHITE)).into_any_element(),
            name.into(),
            text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!(desc)),
            button_sm(("gs2-int-connect", n), crate::t!(cta), BtnKind::Primary).into_any_element(),
        ));
    }
    p = p.child(divider(32.)).child(div().mb(z(16.)).child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.bots_apps"))));
    p = p.child(
        div()
            .mb(z(16.))
            .h(z(40.))
            .pl(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(t(idx::INPUT_BORDER_DEFAULT))
            .bg(t(idx::INPUT_BACKGROUND_DEFAULT))
            .flex()
            .items_center()
            .gap(z(4.))
            .child(div().size(z(30.)).flex().items_center().justify_center().child(icon("search", 16., idx::ICON_DEFAULT)))
            .child(text(16., 20., FontWeight::NORMAL, idx::TEXT_MUTED, crate::t!("gs2.search_apps"))),
    );
    let (Some(ints), Some(hooks)) = (ints, hooks) else { return p.child(super::loading()).into_any_element() };
    for (n, a) in apps(&ints, &hooks).into_iter().enumerate() {
        let mut tags = div().flex().flex_wrap().gap(z(8.)).mt(z(8.));
        if a.bot {
            tags = tags.child(tag(if a.verified { "circle-check" } else { "apps" }, crate::t!(if a.verified { "gs2.bot_verified" } else { "gs2.bot" })));
        }
        if a.webhooks > 0 {
            tags = tags.child(tag("link", crate::t!("gs2.n_webhooks", n = a.webhooks)));
        }
        if a.commands {
            tags = tags.child(tag("angle-brackets", crate::t!("gs2.commands")));
        }
        let sub = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mt(z(4.))
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .child(icon("calendar", 16., idx::ICON_SUBTLE))
                    .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.added_by", date = a.added, user = a.by))),
            )
            .child(tags);
        let id = a.id.clone();
        p = p.child(
            card_row(
                ("gs2-int-app", n),
                app_icon(a.icon_url.as_deref(), 48.),
                a.name.clone().into(),
                sub,
                div().flex().items_center().gap(z(8.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.manage"))).child(icon("chevron-right", 16., idx::TEXT_DEFAULT)).into_any_element(),
            )
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, _, cx| {
                set_sub(guild, SECTION, Sub::App(id.clone()));
                cx.notify();
            })),
        );
    }
    p.into_any_element()
}

fn breadcrumb(guild: Id, leaf: SharedString, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(z(4.))
        .child(
            div()
                .id("gs2-int-crumb")
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    set_sub(guild, SECTION, Sub::Root);
                    cx.notify();
                }))
                .child(text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT, crate::t!("menu.gs.integrations")).hover(|s| s.text_color(t(idx::TEXT_STRONG)))),
        )
        .child(icon("chevron-right", 18., idx::TEXT_MUTED))
        .child(title(leaf))
}

fn webhooks(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let hooks = fetch(app, guild, ServerCall::Webhooks { guild: sf(guild) }, cx);
    let mut p = page()
        .child(breadcrumb(guild, crate::t!("gs2.webhooks"), cx))
        .child(div().mt(z(20.)).child(lead(crate::t!("gs2.webhooks_lead"))))
        .child(div().mt(z(28.)).mb(z(24.)).flex().child(button_sm("gs2-new-webhook", crate::t!("gs2.new_webhook"), BtnKind::Primary)));
    let Some(hooks) = hooks else { return p.child(super::loading()).into_any_element() };
    for (ch, list) in group_by_channel(&hooks) {
        p = p.child(
            div()
                .mb(z(16.))
                .flex()
                .items_center()
                .gap(z(4.))
                .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("gs2.posts_to")))
                .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, super::channel_label(app, ch))),
        );
        for w in list {
            p = p.child(webhook_row(guild, &w, cx));
        }
    }
    p.into_any_element()
}

pub fn group_by_channel(hooks: &Value) -> Vec<(Option<Id>, Vec<Value>)> {
    let mut out: Vec<(Option<Id>, Vec<Value>)> = Vec::new();
    for w in hooks.as_array().into_iter().flatten().filter(|w| w.get("type").and_then(Value::as_u64) == Some(1)) {
        let ch = super::id_of(w, "channel_id");
        match out.iter_mut().find(|(c, _)| *c == ch) {
            Some((_, l)) => l.push(w.clone()),
            None => out.push((ch, vec![w.clone()])),
        }
    }
    out
}

fn webhook_row(guild: Id, w: &Value, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let wid = crate::ui::modals::webhooks::webhook_id(w).unwrap_or(0);
    let key = format!("webhook:{wid}");
    let open = super::with(|s| s.open.get(&guild) == Some(&key));
    let e = cx.entity();
    crate::ui::modals::webhooks::webhook_row(
        w,
        open,
        move |_, _, cx| {
            super::with(|s| {
                if s.open.get(&guild) == Some(&key) {
                    s.open.remove(&guild);
                } else {
                    s.open.insert(guild, key.clone());
                }
            });
            e.update(cx, |_, cx| cx.notify());
        },
        None,
    )
}

pub fn commands(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = v
        .get("application_commands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|c| c.get("type").and_then(Value::as_u64).unwrap_or(1) == 1)
        .map(|c| format!("/{}", str_of(c, "name")))
        .collect();
    out.sort();
    out
}

pub fn everyone_allowed(perms: Option<&Value>, guild: Id) -> (bool, bool) {
    let list = perms.and_then(|p| p.get("permissions")).and_then(Value::as_array);
    let get = |id: u64| list.into_iter().flatten().find(|x| str_of(x, "id").parse::<u64>().ok() == Some(id)).and_then(|x| x.get("permission")).and_then(Value::as_bool).unwrap_or(true);
    (get(guild.0), get(guild.0.wrapping_sub(1)))
}

fn allow_deny(id: &'static str, allowed: bool, on_set: impl Fn(&mut PhoenixApp, bool, &mut Context<PhoenixApp>) + 'static + Clone, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let seg = |n: usize, on: bool, deny: bool, set: bool| {
        let f = on_set.clone();
        div()
            .id((id, n))
            .w(z(31.))
            .h(z(22.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(z(4.))
            .cursor_pointer()
            .when(on, |d| d.bg(t(if deny { idx::BACKGROUND_FEEDBACK_CRITICAL } else { idx::BACKGROUND_FEEDBACK_POSITIVE })))
            .on_click(cx.listener(move |a, _, _, cx| f(a, set, cx)))
            .child(icon(if deny { "close-small" } else { "check" }, 16., if on { idx::WHITE } else if deny { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_FEEDBACK_POSITIVE }))
    };
    div().flex().rounded(z(4.)).border_1().border_color(t(idx::BORDER_SUBTLE)).child(seg(0, !allowed, true, false)).child(seg(1, allowed, false, true))
}

fn perm_block(head: SharedString, add: SharedString, add_id: &'static str, row_label: SharedString, toggle: gpui::Div) -> gpui::Div {
    div()
        .mt(z(16.))
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .flex()
        .flex_col()
        .child(
            div()
                .px(z(16.))
                .py(z(8.))
                .flex()
                .items_center()
                .justify_between()
                .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                .child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT, head))
                .child(button_sm(add_id, add, BtnKind::Secondary)),
        )
        .child(
            div()
                .px(z(16.))
                .py(z(12.))
                .flex()
                .items_center()
                .gap(z(8.))
                .child(icon("members", 24., idx::ICON_SUBTLE))
                .child(div().flex_1().child(text(16., 20., FontWeight::NORMAL, idx::TEXT_STRONG, row_label)))
                .child(toggle),
        )
}

fn app_page(app: &PhoenixApp, guild: Id, id: &str, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let ints = fetch(app, guild, ServerCall::Integrations { guild: sf(guild) }, cx);
    let hooks = fetch(app, guild, ServerCall::Webhooks { guild: sf(guild) }, cx);
    let app_id = Snowflake(id.parse().unwrap_or(0));
    let cmds = fetch(app, guild, ServerCall::AppCommands { guild: sf(guild), application: app_id }, cx);
    let perm_call = ServerCall::CommandPermissions { guild: sf(guild), application: app_id };
    let perms = fetch(app, guild, perm_call.clone(), cx);
    let perms_done = perms.is_some() || super::failed(guild, &perm_call).is_some();
    let (Some(ints), Some(hooks)) = (ints, hooks) else { return super::loading() };
    let Some(a) = apps(&ints, &hooks).into_iter().find(|a| a.id == id) else { return super::loading() };
    let mut p = page()
        .child(breadcrumb(guild, a.name.clone().into(), cx))
        .child(
            div()
                .mt(z(20.))
                .flex()
                .gap(z(16.))
                .child(app_icon(a.icon_url.as_deref(), 48.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(z(12.))
                        .child(text(24., 32., FontWeight::SEMIBOLD, idx::TEXT_STRONG, a.name.clone()))
                        .child(
                            div()
                                .flex()
                                .gap(z(16.))
                                .child(div().flex().gap(z(4.)).child(icon("calendar", 16., idx::ICON_SUBTLE)).child(text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.added_on", date = a.added))))
                                .child(div().flex().gap(z(4.)).child(icon("user", 16., idx::ICON_SUBTLE)).child(text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.added_by_only", user = a.by)))),
                        ),
                ),
        )
        .child(div().mt(z(28.)).child(crate::ui::profile_bio::bio(&a.description, false, crate::theme::c(crate::theme::tok(idx::TEXT_DEFAULT)))));
    if a.commands {
        let (everyone, all_channels) = everyone_allowed(perms.as_ref(), guild);
        let gid = guild;
        let set = move |which: u64| {
            move |app: &mut PhoenixApp, v: bool, cx: &mut Context<PhoenixApp>| {
                let target = if which == 1 { gid.0 } else { gid.0.wrapping_sub(1) };
                let perm = CommandPermission { id: Snowflake(target), kind: if which == 1 { 1 } else { 3 }, permission: v };
                write(app, ServerCall::SetCommandPermissions { guild: sf(gid), application: app_id, command: app_id, permissions: vec![perm] }, None, cx);
            }
        };
        p = p
            .child(div().mt(z(24.)).flex().items_center().gap(z(8.)).child(icon("angle-brackets", 24., idx::ICON_DEFAULT)).child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.cmd_perms"))))
            .child(div().mt(z(16.)).child(text(14., 18., FontWeight::NORMAL, idx::TEXT_MUTED, crate::t!("gs2.cmd_perms_desc"))));
        if perms_done {
            p = p
                .child(perm_block(crate::t!("gs2.roles_members"), crate::t!("gs2.add_roles_members"), "gs2-add-roles", "@everyone".into(), allow_deny("gs2-perm-everyone", everyone, set(1), cx)))
                .child(perm_block(crate::t!("gs2.channels"), crate::t!("gs2.add_channels"), "gs2-add-channels", crate::t!("gs2.all_channels"), allow_deny("gs2-perm-channels", all_channels, set(3), cx)));
        }
        let mut list = div().mt(z(16.)).rounded(z(8.)).border_1().border_color(t(idx::BORDER_SUBTLE)).flex().flex_col().child(
            div()
                .px(z(16.))
                .py(z(8.))
                .flex()
                .items_center()
                .justify_between()
                .bg(t(idx::BACKGROUND_MOD_SUBTLE))
                .child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT, crate::t!("gs2.commands")))
                .child(
                    div()
                        .w(z(174.))
                        .h(z(30.))
                        .px(z(8.))
                        .rounded(z(8.))
                        .border_1()
                        .border_color(t(idx::INPUT_BORDER_DEFAULT))
                        .flex()
                        .items_center()
                        .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_MUTED, crate::t!("gs2.search_commands"))),
                ),
        );
        for (n, c) in cmds.as_ref().map(commands).unwrap_or_default().into_iter().enumerate() {
            list = list.child(
                div()
                    .id(("gs2-cmd", n))
                    .h(z(44.))
                    .px(z(16.))
                    .border_t_1()
                    .border_color(t(idx::BORDER_SUBTLE))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE)))
                    .child(icon("angle-brackets", 24., idx::ICON_SUBTLE))
                    .child(div().flex_1().child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, c)))
                    .child(icon("chevron-right", 16., idx::ICON_SUBTLE)),
            );
        }
        p = p.child(list);
    }
    if a.bot {
        p = p.child(divider(32.)).child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("gs2.bot"))).child(
            div()
                .mt(z(16.))
                .px(z(16.))
                .py(z(12.))
                .rounded(z(8.))
                .bg(t(idx::CARD_BACKGROUND_DEFAULT))
                .flex()
                .items_center()
                .gap(z(8.))
                .child(app_icon(a.icon_url.as_deref(), 32.))
                .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_STRONG, a.name.clone()))
                .child(div().px(z(4.)).rounded(z(4.)).bg(t(idx::BACKGROUND_BRAND)).child(text(12., 16., FontWeight::SEMIBOLD, idx::WHITE, "APP"))),
        );
    }
    let integration = a.integration.clone();
    p.child(divider(32.))
        .child(div().flex().child(button_sm("gs2-kick-app", crate::t!("gs2.remove_app", name = a.name), BtnKind::Critical).on_click(cx.listener(move |app, _, _, cx| {
            write(app, ServerCall::DeleteIntegration { guild: sf(guild), integration: Snowflake(integration.parse().unwrap_or(0)) }, None, cx);
        }))))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn apps_from_integrations() {
        let ints = json!([
            { "id": "1450000000000000000", "type": "discord", "scopes": ["applications.commands", "bot"], "user": { "username": "roxasytb" },
              "application": { "id": "7", "name": "French.gg", "icon": "abc", "is_verified": true, "description": "x", "bot": { "id": "7" } } },
            { "id": "1", "type": "twitch", "name": "t" }
        ]);
        let hooks = json!([{ "id": "9", "type": 1, "application_id": "7", "channel_id": "3" }, { "id": "10", "type": 2, "channel_id": "3" }]);
        let a = apps(&ints, &hooks);
        assert_eq!(a.len(), 1);
        assert!(a[0].verified && a[0].bot && a[0].commands);
        assert_eq!(a[0].webhooks, 1);
        assert_eq!(a[0].icon_url.as_deref(), Some("https://cdn.discordapp.com/app-icons/7/abc.png?size=96"));
        let g = group_by_channel(&hooks);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].1.len(), 1);
    }

    #[test]
    fn commands_and_default_permissions() {
        let v = json!({ "application_commands": [{ "name": "ping", "type": 1 }, { "name": "config", "type": 1 }, { "name": "Report", "type": 3 }] });
        assert_eq!(commands(&v), vec!["/config", "/ping"]);
        assert_eq!(everyone_allowed(None, Id(10)), (true, true));
        let p = json!({ "permissions": [{ "id": "10", "type": 1, "permission": false }] });
        assert_eq!(everyone_allowed(Some(&p), Id(10)), (false, true));
    }
}

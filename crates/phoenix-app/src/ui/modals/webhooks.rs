use gpui::{AnyElement, ClipboardItem, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::InputState;
use phoenix_api::rest::guild_manage::AdminCall;
use serde_json::{Value, json};

use super::{
    channel_settings::{Section, State, pick_section, short_duration, with_state},
    input_box,
    settings_form::{label, text},
};
use crate::{
    actions::admin::sf,
    app::PhoenixApp,
    model::{Id, ImageRef},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button_sm},
};

pub const DEFAULT_NAMES: [&str; 2] = ["Captain Hook", "Spidey Bot"];

fn icon(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).flex_none().size(z(size)).text_color(color)
}

fn link_text(before: SharedString, link: SharedString, after: SharedString) -> gpui::Div {
    div()
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(gpui::StyledText::new(format!("{before}{link}{after}")).with_highlights([(before.len()..before.len() + link.len(), gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() })]))
}

fn split3(full: SharedString) -> (SharedString, SharedString, SharedString) {
    let s = full.to_string();
    match (s.find('['), s.find(']')) {
        (Some(a), Some(b)) if b > a => (s[..a].to_owned().into(), s[a + 1..b].to_owned().into(), s[b + 1..].to_owned().into()),
        _ => (full, SharedString::default(), SharedString::default()),
    }
}

fn divider(m: f32) -> gpui::Div {
    div().my(z(m)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))
}

pub fn snowflake_date(id: u64) -> String {
    let ms = (id >> 22) as i64 + 1_420_070_400_000;
    chrono::DateTime::from_timestamp_millis(ms).map(|d| crate::i18n::medium_date(d.with_timezone(&chrono::Local).date_naive())).unwrap_or_default()
}

pub fn webhook_id(w: &Value) -> Option<u64> {
    w.get("id")?.as_str()?.parse().ok()
}

pub fn webhook_avatar(w: &Value, size: f32) -> AnyElement {
    let id = webhook_id(w).unwrap_or(0);
    let img = match w.get("avatar").and_then(Value::as_str) {
        Some(h) => ImageRef::Url(format!("https://cdn.discordapp.com/avatars/{id}/{h}.png?size=128").into()),
        None => ImageRef::Default(((id >> 22) % 6) as u8),
    };
    crate::ui::avatar_ref(img, size)
}

pub fn webhook_row(w: &Value, open: bool, on_toggle: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static, body: Option<AnyElement>) -> AnyElement {
    let id = webhook_id(w).unwrap_or(0);
    let name = w.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
    let who = w.pointer("/user/global_name").and_then(Value::as_str).or_else(|| w.pointer("/user/username").and_then(Value::as_str)).unwrap_or_default().to_owned();
    let created = crate::t!("cs.webhook_created", date = snowflake_date(id), user = who);
    div()
        .mb(z(16.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .flex()
        .flex_col()
        .child(
            div()
                .id(SharedString::from(format!("webhook-{id}")))
                .py(z(20.))
                .px(z(16.))
                .cursor_pointer()
                .on_click(on_toggle)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(16.))
                        .child(webhook_avatar(w, 48.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(text(name, 16., 20., idx::TEXT_STRONG).font_weight(FontWeight::MEDIUM).truncate())
                                .child(div().mt(z(4.)).flex().items_center().gap(z(4.)).child(icon("settings/clock", 16., c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))).child(text(created, 12., 16., idx::TEXT_DEFAULT))),
                        )
                        .child(icon("settings/chevron-down", 24., c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).when(!open, |i| i.with_transformation(gpui::Transformation::rotate(gpui::radians(-std::f32::consts::FRAC_PI_2))))),
                ),
        )
        .children(body.map(|b| div().px(z(16.)).pb(z(24.)).child(div().mb(z(24.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))).child(b)))
        .into_any_element()
}

pub fn webhook_list(list: &[Value], open: Option<u64>, mut row: impl FnMut(&Value, bool) -> AnyElement) -> AnyElement {
    let mut d = div().flex().flex_col();
    for w in list {
        d = d.child(row(w, webhook_id(w) == open));
    }
    d.into_any_element()
}

fn card(id: &'static str, icon_name: &'static str, title: SharedString, sub: SharedString, action: AnyElement, cx: &mut Context<PhoenixApp>, sec: Option<Section>) -> AnyElement {
    div()
        .id(id)
        .mb(z(16.))
        .py(z(20.))
        .px(z(16.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .when(sec.is_some(), |d| d.cursor_pointer())
        .on_click(cx.listener(move |a, _, _, cx| {
            if let Some(sec) = sec {
                pick_section(a, sec, cx);
                cx.notify();
            }
        }))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .gap(z(16.))
                        .child(div().size(z(48.)).rounded_full().bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))).flex().items_center().justify_center().child(icon(icon_name, 24., c(tok(idx::TEXT_DEFAULT)))))
                        .child(div().flex_1().min_w_0().flex().flex_col().justify_center().child(text(title, 16., 20., idx::TEXT_STRONG).font_weight(FontWeight::MEDIUM)).child(text(sub, 12., 16., idx::TEXT_DEFAULT).mt(z(4.)))),
                )
                .child(action),
        )
        .into_any_element()
}

pub fn create_webhook(a: &mut PhoenixApp, channel: Id, cx: &mut Context<PhoenixApp>) {
    let n = with_count();
    let name = DEFAULT_NAMES[n % DEFAULT_NAMES.len()].to_owned();
    a.admin(AdminCall::CreateWebhook { channel: sf(channel), name }, cx, move |a, r, cx| {
        match r {
            Ok(v) if !v.is_null() => with_state(|s| {
                s.open_webhook = v.get("id").and_then(Value::as_str).map(str::to_owned);
                s.webhooks.get_or_insert_with(Vec::new).insert(0, v);
                s.section = Section::Webhooks;
            }),
            Ok(_) => {}
            Err(e) => a.parity_toast(e, cx),
        }
        cx.notify();
    });
}

fn with_count() -> usize {
    let mut n = 0;
    with_state(|s| n = s.webhooks.as_ref().map_or(0, Vec::len));
    n
}

pub fn integrations(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let hooks = s.webhooks.as_deref().unwrap_or_default();
    let nh = hooks.iter().filter(|w| w.get("type").and_then(Value::as_u64) == Some(1)).count();
    let nf = hooks.iter().filter(|w| w.get("type").and_then(Value::as_u64) == Some(2)).count();
    let (a, l, b) = split3(crate::t!("cs.integrations_desc"));
    let channel = s.channel;
    let create = button_sm("cs-create-hook", crate::t!("cs.create_webhook"), BtnKind::Primary)
        .on_click(cx.listener(move |a, _, _, cx| {
            cx.stop_propagation();
            create_webhook(a, channel, cx);
        }))
        .into_any_element();
    let view = icon("chevron-right", 24., c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).into_any_element();
    let learn = button_sm("cs-followed-more", crate::t!("cs.learn_more"), BtnKind::Primary).into_any_element();
    div()
        .flex()
        .flex_col()
        .mt(z(-4.))
        .child(link_text(a, l, b))
        .child(divider(32.))
        .child(card("cs-hooks-card", "settings/webhook", crate::t!("cs.webhooks"), crate::t!("cs.n_webhooks", n = nh), if nh > 0 { view } else { create }, cx, (nh > 0).then_some(Section::Webhooks)))
        .child(card("cs-followed-card", "settings/followed", crate::t!("cs.followed"), crate::t!("cs.n_channels", n = nf), learn, cx, (nf > 0).then_some(Section::Followed)))
        .into_any_element()
}

fn breadcrumb(last: SharedString, cx: &mut Context<PhoenixApp>) -> AnyElement {
    div()
        .mb(z(20.))
        .flex()
        .items_center()
        .child(
            div()
                .id("cs-crumb")
                .cursor_pointer()
                .on_click(cx.listener(|a, _, _, cx| {
                    pick_section(a, Section::Integrations, cx);
                    cx.notify();
                }))
                .child(text(crate::t!("admin.cs_integrations"), 20., 24., idx::TEXT_DEFAULT).font_weight(FontWeight::SEMIBOLD)),
        )
        .child(icon("chevron-right", 18., c(tok(idx::TEXT_MUTED))))
        .child(text(last, 20., 24., idx::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD))
        .into_any_element()
}

fn expanded(app: &PhoenixApp, s: &State, w: &Value, name: Option<&Entity<InputState>>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let id = webhook_id(w).unwrap_or(0);
    let url = w.get("url").and_then(Value::as_str).map(str::to_owned).or_else(|| w.get("token").and_then(Value::as_str).map(|t| format!("https://discord.com/api/webhooks/{id}/{t}")));
    let mut options = webhook_channels(app, s.guild);
    if options.is_empty() {
        options.push((s.channel, SharedString::from(format!("#{}", app.channels.get(&s.channel).map(|c| c.name.to_string()).unwrap_or_default()))));
    }
    let current = s.webhook_channel.or_else(|| hook_channel(w)).unwrap_or(s.channel);
    let selected = options.iter().position(|(id, _)| *id == current).unwrap_or(0);
    let labels: Vec<SharedString> = options.iter().map(|(_, l)| l.clone()).collect();
    let ids: Vec<Id> = options.iter().map(|(id, _)| *id).collect();
    let e = cx.entity();
    let toggle: std::rc::Rc<dyn Fn(&mut gpui::App)> = std::rc::Rc::new(|_| with_state(|s| s.open_select = if s.open_select == Some(HOOK_CHANNEL_SELECT) { None } else { Some(HOOK_CHANNEL_SELECT) }));
    let pick: super::settings_form::OnPick = std::rc::Rc::new(move |k, _, cx| {
        with_state(|s| {
            s.webhook_channel = ids.get(k).copied();
            s.open_select = None;
        });
        e.update(cx, |_, cx| cx.notify());
    });
    let open = s.open_select == Some(HOOK_CHANNEL_SELECT);
    div()
        .flex()
        .child(
            div()
                .flex_none()
                .pr(z(12.))
                .mr(z(10.))
                .flex()
                .flex_col()
                .items_center()
                .child(avatar_uploader(w, s.webhook_avatar.as_deref(), cx))
                .child({
                    let t = crate::t!("cs.min_size");
                    let r = t.find("128").map(|i| i..t.len());
                    div().mt(z(10.)).text_size(z(10.)).line_height(z(10.)).text_color(c(tok(idx::TEXT_MUTED))).child(gpui::StyledText::new(t.clone()).with_highlights(r.map(|r| (r, gpui::HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() }))))
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .ml(z(10.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .gap(z(20.))
                        .child(div().flex_1().flex().flex_col().child(label(crate::t!("cs.webhook_name")).mb(z(8.))).children(name.map(input_box)))
                        .child(div().flex_1().flex().flex_col().child(label(crate::t!("cs.webhook_channel")).mb(z(8.))).child(super::settings_form::dropdown("cs-hook-chan", labels, selected, open, toggle, pick, cx))),
                )
                .child(divider(24.))
                .child(
                    div()
                        .flex()
                        .gap(z(16.))
                        .child(button_sm("cs-hook-copy", crate::t!("cs.copy_webhook_url"), BtnKind::Secondary).on_click(move |_, _, cx| {
                            if let Some(u) = &url {
                                cx.write_to_clipboard(ClipboardItem::new_string(u.clone()));
                            }
                        }))
                        .child(
                            button_sm("cs-hook-delete", crate::t!("cs.delete_webhook"), BtnKind::Secondary)
                                .text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL)))
                                .on_click(cx.listener(move |a, _, _, cx| {
                                    with_state(|s| {
                                        if let Some(l) = &mut s.webhooks {
                                            l.retain(|x| webhook_id(x) != Some(id));
                                        }
                                        s.open_webhook = None;
                                        s.webhook_name = None;
                                    });
                                    a.admin_toast(AdminCall::DeleteWebhook { webhook: phoenix_api::models::Snowflake(id) }, None, cx);
                                    cx.notify();
                                })),
                        ),
                ),
        )
        .into_any_element()
}

pub fn webhooks_page(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (a, l, b) = split3(crate::t!("cs.webhooks_desc"));
    let hooks: Vec<Value> = s.webhooks.iter().flatten().filter(|w| w.get("type").and_then(Value::as_u64) == Some(1)).cloned().collect();
    let open = s.open_webhook.as_deref().and_then(|x| x.parse().ok());
    let channel = s.channel;
    let chan = app.channels.get(&s.channel).map(|c| c.name.to_string()).unwrap_or_default();
    let e = cx.entity();
    let list = webhook_list(&hooks, open, |w, is_open| {
        let id = webhook_id(w).map(|x| x.to_string());
        let e = e.clone();
        let toggle = move |_: &gpui::ClickEvent, _: &mut gpui::Window, cx: &mut gpui::App| {
            with_state(|s| {
                s.open_webhook = if s.open_webhook == id { None } else { id.clone() };
                s.webhook_name = None;
                s.webhook_channel = None;
            });
            e.update(cx, |_, cx| cx.notify());
        };
        let body = is_open.then(|| expanded(app, s, w, s.webhook_name.as_ref(), cx));
        webhook_row(w, is_open, toggle, body)
    });
    div()
        .flex()
        .flex_col()
        .child(breadcrumb(crate::t!("cs.webhooks"), cx))
        .child(link_text(a, l, b))
        .child(divider(32.))
        .child(div().mb(z(24.)).flex().child(button_sm("cs-new-hook", crate::t!("cs.new_webhook"), BtnKind::Primary).on_click(cx.listener(move |a, _, _, cx| create_webhook(a, channel, cx)))))
        .when(!hooks.is_empty(), |d| d.child(text(crate::t!("cs.posts_to", channel = format!("#{chan}")), 16., 20., idx::TEXT_DEFAULT).font_weight(FontWeight::MEDIUM).mt(z(24.)).mb(z(16.))))
        .child(list)
        .into_any_element()
}

pub fn followed_page(_app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let hooks: Vec<Value> = s.webhooks.iter().flatten().filter(|w| w.get("type").and_then(Value::as_u64) == Some(2)).cloned().collect();
    let list = webhook_list(&hooks, None, |w, _| webhook_row(w, false, |_, _, _| {}, None));
    div().flex().flex_col().child(breadcrumb(crate::t!("cs.followed"), cx)).child(text(crate::t!("cs.followed_desc"), 14., 18., idx::TEXT_DEFAULT)).child(divider(32.)).child(list).into_any_element()
}

pub const HOOK_CHANNEL_SELECT: u8 = 40;

pub fn webhook_channels(app: &PhoenixApp, guild: Option<Id>) -> Vec<(Id, SharedString)> {
    use crate::model::ChannelKind;
    let Some(g) = guild else { return Vec::new() };
    if !crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 29) {
        return Vec::new();
    }
    let list: Vec<&crate::model::Channel> = app.guild_channels.get(&g).into_iter().flatten().filter_map(|id| app.channels.get(id)).collect();
    let selectable = list.iter().filter(|c| matches!(c.kind, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum)).map(|c| (c.id, SharedString::from(format!("#{}", c.name))));
    let vocal = list.iter().filter(|c| matches!(c.kind, ChannelKind::Voice | ChannelKind::Stage)).map(|c| (c.id, c.name.clone()));
    selectable.chain(vocal).collect()
}

fn hook_channel(w: &Value) -> Option<Id> {
    w.get("channel_id").and_then(Value::as_str).and_then(|x| x.parse().ok()).map(Id)
}

pub fn webhook_dirty(s: &State, cx: &gpui::App) -> Option<(u64, String)> {
    let id: u64 = s.open_webhook.as_deref()?.parse().ok()?;
    let w = s.webhooks.iter().flatten().find(|w| webhook_id(w) == Some(id))?;
    let orig = w.get("name")?.as_str()?.to_owned();
    let moved = s.webhook_channel.is_some_and(|c| Some(c) != hook_channel(w));
    let now = s.webhook_name.as_ref().map(|i| i.read(cx).value().trim().to_owned()).filter(|n| !n.is_empty()).unwrap_or_else(|| orig.clone());
    (now != orig || s.webhook_avatar.is_some() || moved).then_some((id, now))
}

pub fn webhook_body(name: &str, avatar: Option<String>, channel: Option<Id>) -> Value {
    let mut b = json!({ "name": name });
    if let Some(a) = avatar {
        b["avatar"] = json!(a);
    }
    if let Some(c) = channel {
        b["channel_id"] = json!(c.0.to_string());
    }
    b
}

pub fn pick_webhook_avatar(cx: &mut Context<PhoenixApp>) {
    crate::ui::file_pick::pick(&crate::ui::file_pick::IMAGES, cx, |_, path, _| with_state(|s| s.webhook_avatar = Some(path)));
}

pub fn save_webhook(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let mut out = None;
    with_state(|s| {
        if let Some((id, name)) = webhook_dirty(s, cx) {
            let avatar = s.webhook_avatar.as_deref().and_then(crate::ui::file_pick::data_uri);
            let here = s.channel;
            let mut moved = None;
            if let Some(w) = s.webhooks.iter_mut().flatten().find(|w| webhook_id(w) == Some(id)) {
                w["name"] = json!(name);
                moved = s.webhook_channel.filter(|c| Some(*c) != hook_channel(w));
                if let Some(c) = moved {
                    w["channel_id"] = json!(c.0.to_string());
                }
            }
            if moved.is_some_and(|c| c != here) {
                s.open_webhook = None;
                if let Some(l) = &mut s.webhooks {
                    l.retain(|x| webhook_id(x) != Some(id));
                }
            }
            out = Some((id, webhook_body(&name, avatar, moved)));
        }
    });
    if let Some((id, body)) = out {
        a.admin_toast(AdminCall::EditWebhook { webhook: phoenix_api::models::Snowflake(id), body }, None, cx);
    }
}

fn avatar_uploader(w: &Value, local: Option<&std::path::Path>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let id = webhook_id(w).unwrap_or(0);
    let src: gpui::ImageSource = match (local, w.get("avatar").and_then(Value::as_str)) {
        (Some(p), _) => p.to_path_buf().into(),
        (None, Some(h)) => SharedString::from(format!("https://cdn.discordapp.com/avatars/{id}/{h}.png?size=128")).into(),
        (None, None) => SharedString::from("https://discord.com/assets/2ccd8ae8b2379360.png").into(),
    };
    div()
        .id("cs-hook-avatar")
        .relative()
        .w(z(111.))
        .h(z(102.))
        .cursor_pointer()
        .group("cs-hook-avatar")
        .on_click(cx.listener(|_, _, _, cx| pick_webhook_avatar(cx)))
        .child(
            div()
                .absolute()
                .left(z(5.))
                .top_0()
                .size(z(100.))
                .rounded(z(12.))
                .overflow_hidden()
                .child(gpui::img(src).size(z(100.)).object_fit(gpui::ObjectFit::Cover))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .invisible()
                        .group_hover("cs-hook-avatar", |d| d.visible())
                        .bg(c(tok(idx::OPACITY_BLACK_76)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_center()
                        .child(text(crate::t!("cs.change_avatar"), 12., 12., idx::WHITE)),
                ),
        )
        .child(
            div()
                .absolute()
                .left(z(83.))
                .top_0()
                .size(z(28.))
                .rounded_full()
                .bg(gpui::white())
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::img(SharedString::from("https://discord.com/assets/6ca913b246a05ffc.svg")).size(z(28.))),
        )
        .into_any_element()
}

pub fn invites(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let guild = s.guild;
    let (a, l, b) = split3(crate::t!("cs.invites_desc"));
    let mut d = div()
        .flex()
        .flex_col()
        .mt(z(-4.))
        .child(link_text(a, l, b).mb(z(16.)))
        .child(div().flex().child(button_sm("cs-pause-invites", crate::t!("cs.pause_invites"), BtnKind::Critical).on_click(cx.listener(move |a, _, _, cx| {
            if let Some(g) = guild {
                let sec = a.security_state(g);
                with_state(|s| s.security = Some(sec));
            }
            cx.notify();
        }))))
        .child(divider(24.));
    let Some(list) = s.invites.as_deref() else {
        return d.child(text(crate::t!("common.loading"), 14., 18., idx::TEXT_SUBTLE)).into_any_element();
    };
    if list.is_empty() {
        return d.into_any_element();
    }
    let head = |t: &str, w: Option<f32>| {
        let h = text(crate::t!(t).to_uppercase(), 12., 16., idx::TEXT_SUBTLE).font_weight(FontWeight::SEMIBOLD);
        match w {
            Some(w) => h.w(z(w)),
            None => h.flex_1(),
        }
    };
    d = d.child(div().flex().pb(z(8.)).child(head("admin.inviter", None)).child(head("admin.invite_code", Some(120.))).child(head("admin.uses", Some(60.))).child(head("admin.expires", Some(90.))).child(div().w(z(70.))));
    for (n, inv) in list.iter().enumerate() {
        let code = inv.get("code").and_then(Value::as_str).unwrap_or_default().to_owned();
        let who = inv.pointer("/inviter/global_name").and_then(Value::as_str).or_else(|| inv.pointer("/inviter/username").and_then(Value::as_str)).unwrap_or("-").to_owned();
        let uses = inv.get("uses").and_then(Value::as_u64).unwrap_or(0);
        let max = inv.get("max_uses").and_then(Value::as_u64).unwrap_or(0);
        let age = inv.get("max_age").and_then(Value::as_u64).unwrap_or(0);
        let expires = if age == 0 { "∞".to_owned() } else { short_duration(age.min(u32::MAX as u64) as u32) };
        let c2 = code.clone();
        d = d.child(
            div()
                .flex()
                .items_center()
                .py(z(10.))
                .border_t_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(div().flex_1().min_w_0().truncate().child(text(who, 14., 18., idx::TEXT_STRONG)))
                .child(div().w(z(120.)).child(text(code, 14., 18., idx::TEXT_DEFAULT)))
                .child(div().w(z(60.)).child(text(if max == 0 { uses.to_string() } else { format!("{uses}/{max}") }, 14., 18., idx::TEXT_DEFAULT)))
                .child(div().w(z(90.)).child(text(expires, 14., 18., idx::TEXT_DEFAULT)))
                .child(div().w(z(70.)).child(super::settings_layout::link_btn(("cs-inv", n), crate::t!("admin.revoke"), true).on_click(cx.listener(move |a, _, _, cx| {
                    with_state(|s| {
                        if let Some(l) = &mut s.invites {
                            l.retain(|i| i.get("code").and_then(Value::as_str) != Some(c2.as_str()))
                        }
                    });
                    a.admin_toast(AdminCall::DeleteInvite { code: c2.clone() }, None, cx);
                    cx.notify();
                })))),
        );
    }
    d.into_any_element()
}

#[cfg(test)]
mod tests {
    #[test]
    fn webhook_dates_and_links() {
        assert!(!super::snowflake_date(1_574_974_996_328_226_922).is_empty());
        let (a, l, b) = super::split3("x [y] z".into());
        assert_eq!((a.as_ref(), l.as_ref(), b.as_ref()), ("x ", "y", " z"));
        let w = serde_json::json!({ "id": "12", "name": "n" });
        assert_eq!(super::webhook_id(&w), Some(12));
    }

    #[test]
    fn webhook_patch_carries_the_avatar_as_a_data_uri() {
        assert_eq!(super::webhook_body("Hook", None, None), serde_json::json!({ "name": "Hook" }));
        let b = super::webhook_body("Hook", Some(crate::ui::file_pick::encode(b"abc", "image/png")), None);
        assert_eq!(b["avatar"], "data:image/png;base64,YWJj");
        let m = super::webhook_body("Hook", None, Some(crate::model::Id(1_403_447_573_389_381_652)));
        assert_eq!(m, serde_json::json!({ "name": "Hook", "channel_id": "1403447573389381652" }));
    }
}

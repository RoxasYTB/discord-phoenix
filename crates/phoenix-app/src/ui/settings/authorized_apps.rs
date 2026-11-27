use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use parking_lot::RwLock;
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{self, BtnKind},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthorizedApp {
    pub token_id: u64,
    pub app_id: u64,
    pub name: String,
    pub icon: Option<String>,
    pub description: String,
    pub terms: bool,
    pub privacy: bool,
    pub scopes: Vec<String>,
    pub flags: u64,
}

const SOCIAL_LAYER_INTEGRATION_LIMITED: u64 = 1 << 10;
const SOCIAL_LAYER_INTEGRATION: u64 = 1 << 27;
const CONSOLE_APPS: [u64; 3] = [622_174_530_214_821_906, 984_193_235_868_065_795, 1_008_890_872_156_405_890];

pub fn game_connection(a: &AuthorizedApp) -> bool {
    a.flags & (SOCIAL_LAYER_INTEGRATION_LIMITED | SOCIAL_LAYER_INTEGRATION) != 0 && a.scopes.iter().any(|s| s == "sdk.social_layer" || s == "sdk.social_layer_presence")
}

pub fn console_connection(app_id: u64) -> bool {
    CONSOLE_APPS.contains(&app_id)
}

#[derive(Clone, Debug, PartialEq)]
struct Pending {
    token: u64,
    name: String,
    game: bool,
    console: bool,
}

static APPS: RwLock<Option<Vec<AuthorizedApp>>> = RwLock::new(None);
static CONFIRM: RwLock<Option<Pending>> = RwLock::new(None);

pub fn deauth_request(token_id: u64) -> (&'static str, String) {
    ("DELETE", format!("/oauth2/tokens/{token_id}"))
}

fn deauthorize(app: &PhoenixApp, token_id: u64, cx: &mut Context<PhoenixApp>) {
    let fut = app.backend.deauthorize_app(token_id);
    cx.spawn(async move |this, cx| {
        if fut.await.is_ok() {
            if let Some(l) = APPS.write().as_mut() {
                l.retain(|a| a.token_id != token_id);
            }
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

pub fn confirm_modal(cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let Pending { token, name, game, console } = CONFIRM.read().clone()?;
    let (e1, e2) = (cx.entity(), cx.entity());
    let (title, sub) = if game {
        (crate::t!("sp.auth.disconnect_title", name = name), crate::t!("sp.auth.disconnect_body", name = name))
    } else {
        (crate::t!("sp.auth.deauth_title"), crate::t!("sp.auth.deauth_body", name = name))
    };
    let mut body = vec![sub.into_any_element()];
    if game {
        body.push(warning_notice(crate::t!("sp.auth.disconnect_warning")));
    }
    if console {
        body.push(crate::ui::modals::settings_form::info_notice(crate::t!("sp.auth.console_notice", name = name)));
    }
    Some(crate::ui::modal::confirm(crate::ui::modal::Confirm {
        title,
        body,
        confirm_label: crate::t!("sp.auth.deauth"),
        danger: true,
        on_confirm: std::rc::Rc::new(move |_, cx| {
            *CONFIRM.write() = None;
            e1.update(cx, |a, cx| deauthorize(a, token, cx));
        }),
        on_cancel: std::rc::Rc::new(move |_, cx| {
            *CONFIRM.write() = None;
            e2.update(cx, |_, cx| cx.notify());
        }),
    }))
}

fn warning_notice(t: SharedString) -> AnyElement {
    div()
        .p(z(12.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_FEEDBACK_WARNING)))
        .bg(c(tok(idx::BACKGROUND_FEEDBACK_WARNING)))
        .flex()
        .items_start()
        .gap(z(8.))
        .child(div().h(z(18.)).flex().items_center().child(gpui::svg().path(crate::icons::path("warning")).size(z(16.)).text_color(c(tok(idx::ICON_FEEDBACK_WARNING)))))
        .child(div().flex_1().min_w_0().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_STRONG))).child(t))
        .into_any_element()
}

pub fn close_confirm() -> bool {
    CONFIRM.write().take().is_some()
}

pub fn get() -> Option<Vec<AuthorizedApp>> {
    APPS.read().clone()
}

pub fn set(mut v: Vec<AuthorizedApp>) {
    v.sort_by_key(|a| std::cmp::Reverse(a.token_id));
    *APPS.write() = Some(v);
}

pub fn parse(v: &Value) -> Vec<AuthorizedApp> {
    let id = |v: Option<&Value>| v.and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0u64);
    let s = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or_default().to_owned();
    v.as_array()
        .into_iter()
        .flatten()
        .map(|t| {
            let a = t.get("application");
            let g = |k: &str| a.and_then(|a| a.get(k));
            AuthorizedApp {
                token_id: id(t.get("id")),
                app_id: id(g("id")),
                name: s(g("name")),
                icon: g("icon").and_then(Value::as_str).map(str::to_owned),
                description: s(g("description")),
                terms: g("terms_of_service_url").and_then(Value::as_str).is_some_and(|u| !u.is_empty()),
                privacy: g("privacy_policy_url").and_then(Value::as_str).is_some_and(|u| !u.is_empty()),
                scopes: t.get("scopes").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect(),
                flags: g("flags").and_then(Value::as_u64).unwrap_or(0),
            }
        })
        .collect()
}

pub fn fetch(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if APPS.read().is_some() {
        return;
    }
    let fut = app.backend.authorized_apps();
    cx.spawn(async move |this, cx| {
        if let Ok(Some(v)) = fut.await {
            set(v);
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

fn scope_key(s: &str) -> Option<&'static str> {
    Some(match s {
        "identify" => "sp.auth.s.identify",
        "email" => "sp.auth.s.email",
        "guilds" => "sp.auth.s.guilds",
        "guilds.join" => "sp.auth.s.guilds_join",
        "guilds.members.read" => "sp.auth.s.members_read",
        "rpc" => "sp.auth.s.rpc",
        "rpc.voice.read" => "sp.auth.s.voice_read",
        "rpc.voice.write" => "sp.auth.s.voice_write",
        "rpc.notifications.read" => "sp.auth.s.notifications",
        "messages.read" => "sp.auth.s.messages_read",
        "applications.commands" => "sp.auth.s.commands",
        "applications.entitlements" => "sp.auth.s.entitlements",
        "activities.write" => "sp.auth.s.activities_write",
        "relationships.read" => "sp.auth.s.relationships_read",
        "relationships.write" => "sp.auth.s.relationships_write",
        "presences.read" => "sp.auth.s.presences",
        "openid" => "sp.auth.s.openid",
        "dm_channels.read" => "sp.auth.s.dm_read",
        "dm_channels.messages.write" | "dm_channels.messages.read" => "sp.auth.s.dm_messages",
        "activities.invites.write" => "sp.auth.s.invites",
        _ => return None,
    })
}

fn date(id: u64) -> String {
    let ms = (id >> 22) as i64 + 1_420_070_400_000;
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.with_timezone(&chrono::Local).format(&crate::t!("sp.auth.date_fmt")).to_string())
        .unwrap_or_default()
}

fn small(t: impl Into<SharedString>, col: usize) -> gpui::Div {
    div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(col))).child(t.into())
}

pub fn render(_: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(apps) = get() else {
        return small(crate::t!("sp.cn.none"), idx::TEXT_SUBTLE).into_any_element();
    };
    let check = || gpui::svg().path(crate::icons::path("check")).size(z(14.)).flex_none().text_color(c(tok(idx::TEXT_DEFAULT)));
    let cards = apps.into_iter().map(|a| {
        let icon = a
            .icon
            .as_ref()
            .map(|h| format!("https://cdn.discordapp.com/app-icons/{}/{h}.png?size=80", a.app_id))
            .map(|u| gpui::img(u).size(z(40.)).rounded(z(8.)).into_any_element())
            .unwrap_or_else(|| div().size(z(40.)).rounded(z(8.)).bg(c(tok(idx::BRAND_500))).into_any_element());
        let section = |title: SharedString| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(title),
                )
                .child(gpui::svg().path("icons/chevron.svg").size(z(24.)).text_color(c(tok(idx::TEXT_DEFAULT))))
        };
        let perms: Vec<SharedString> = a.scopes.iter().filter_map(|s| scope_key(s)).map(|k| crate::t!(k)).collect();
        let n = a.scopes.len();
        let count = if n == 1 { crate::t!("sp.auth.perm_one") } else { crate::t!("sp.auth.perm_n", n = n) };
        let has_about = !a.description.is_empty() || a.terms || a.privacy;
        div()
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .pb(z(16.))
            .flex()
            .flex_col()
            .child(
                div()
                    .p(z(16.))
                    .flex()
                    .items_center()
                    .gap(z(12.))
                    .child(icon)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(z(4.))
                            .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).truncate().child(a.name.clone()))
                            .child(small(crate::t!("sp.auth.date", date = date(a.token_id)), idx::TEXT_DEFAULT)),
                    )
                    .child(div().size(z(32.)).flex().items_center().justify_center().child(
                        gpui::svg().path(crate::icons::path("more-horizontal")).size(z(16.)).text_color(c(tok(idx::TEXT_DEFAULT))),
                    )),
            )
            .child(
                div()
                    .px(z(16.))
                    .pt(z(4.))
                    .flex()
                    .flex_col()
                    .gap(z(16.))
                    .when(has_about, |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(z(4.))
                                .child(section(crate::t!("sp.auth.about")))
                                .when(!a.description.is_empty(), |d| d.child(small(a.description.clone(), idx::TEXT_DEFAULT)))
                                .when(a.terms || a.privacy, |d| {
                                    d.child(
                                        div()
                                            .pt(z(8.))
                                            .flex()
                                            .items_center()
                                            .gap(z(12.))
                                            .when(a.terms, |d| d.child(small(crate::t!("sp.auth.terms"), idx::TEXT_LINK)))
                                            .when(a.terms && a.privacy, |d| d.child(div().w(z(1.)).h(z(18.)).bg(c(tok(idx::BORDER_SUBTLE)))))
                                            .when(a.privacy, |d| d.child(small(crate::t!("settings.footer.privacy"), idx::TEXT_LINK))),
                                    )
                                }),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(z(8.))
                            .child(div().flex().flex_col().child(section(crate::t!("sp.auth.perms"))).child(small(count, idx::TEXT_SUBTLE)))
                            .children(perms.into_iter().map(|p| div().flex().items_center().gap(z(9.)).child(check()).child(small(p, idx::TEXT_DEFAULT))))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(z(5.))
                                    .child(gpui::svg().path(crate::icons::path("info")).size(z(18.)).flex_none().text_color(c(tok(idx::TEXT_MUTED))))
                                    .child(small(crate::t!("sp.auth.ip"), idx::TEXT_DEFAULT)),
                            ),
                    )
                    .child(
                        div().flex().justify_end().child(
                            controls::button_sm(gpui::ElementId::Name(format!("deauth-{}", a.token_id).into()), crate::t!("sp.auth.deauth"), BtnKind::Secondary)
                                .text_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT)))
                                .on_click({
                                    let pending = Pending { token: a.token_id, name: a.name.clone(), game: game_connection(&a), console: console_connection(a.app_id) };
                                    cx.listener(move |_, _, _, cx| {
                                        *CONFIRM.write() = Some(pending.clone());
                                        cx.notify();
                                    })
                                }),
                        ),
                    ),
            )
            .into_any_element()
    });
    div().flex().flex_col().gap(z(24.)).children(cards).into_any_element()
}

#[cfg(test)]
mod tests {
    #[test]
    fn deauthorize_deletes_the_token() {
        assert_eq!(super::deauth_request(42), ("DELETE", "/oauth2/tokens/42".to_owned()));
    }

    #[test]
    fn parses_tokens() {
        let v = serde_json::json!([{"id": "1300000000000000000", "scopes": ["identify", "guilds"], "application": {"id": "5", "name": "Bot", "icon": null, "description": "d", "privacy_policy_url": "https://x"}}]);
        let a = super::parse(&v);
        assert_eq!(a[0].name, "Bot");
        assert_eq!(a[0].scopes.len(), 2);
        assert!(a[0].privacy && !a[0].terms);
        assert_eq!(super::scope_key("guilds"), Some("sp.auth.s.guilds"));
        assert!(!super::game_connection(&a[0]));
    }

    #[test]
    fn game_connections_get_the_disconnect_variant() {
        let v = serde_json::json!([{"id": "2", "scopes": ["sdk.social_layer"], "application": {"id": "622174530214821906", "name": "Xbox", "flags": 1u64 << 27}}]);
        let a = super::parse(&v);
        assert!(super::game_connection(&a[0]));
        assert!(super::console_connection(a[0].app_id));
        let v = serde_json::json!([{"id": "3", "scopes": ["identify"], "application": {"id": "9", "name": "G", "flags": 1u64 << 10}}]);
        assert!(!super::game_connection(&super::parse(&v)[0]));
        assert!(!super::console_connection(9));
    }
}

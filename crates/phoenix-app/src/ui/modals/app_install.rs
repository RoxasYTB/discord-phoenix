use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, deferred, div, img, prelude::*};
use serde_json::{Value, json};

use super::{Modal, card_with};
use crate::{
    actions::{close_modal, guard::GuardedWrite, open_modal, with_modal},
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

pub const GUILD_INSTALL: u8 = 0;
pub const USER_INSTALL: u8 = 1;
pub const MANAGE_GUILD: u64 = 1 << 5;
pub const WIDTH: f32 = 440.;

pub const ACCOUNT_SCOPES: [&str; 57] = [
    "bot",
    "openid",
    "identify",
    "identify.premium",
    "email",
    "connections",
    "messages.read",
    "guilds",
    "guilds.join",
    "guilds.members.read",
    "guilds.channels.read",
    "gdm.join",
    "rpc",
    "rpc.notifications.read",
    "rpc.voice.read",
    "rpc.voice.write",
    "rpc.video.read",
    "rpc.video.write",
    "rpc.screenshare.read",
    "rpc.screenshare.write",
    "rpc.activities.write",
    "application_identities.write",
    "managed_platform.application_identities.write",
    "applications.builds.upload",
    "applications.builds.read",
    "applications.commands",
    "applications.commands.update",
    "applications.commands.permissions.update",
    "applications.store.update",
    "applications.entitlements",
    "activities.invites.write",
    "activities.read",
    "activities.write",
    "orbs_arcade.play",
    "relationships.read",
    "relationships.write",
    "voice",
    "dm_channels.read",
    "dm_channels.messages.read",
    "dm_channels.messages.write",
    "personal.relationships.read",
    "personal.dm_channels.read",
    "personal.dm_channels.messages.read",
    "personal.guilds.channels.read",
    "personal.guilds.channels.messages.read",
    "personal.activities.write",
    "personal.mentions.read",
    "personal.guilds.messages.search",
    "personal.dm_channels.messages.search",
    "role_connections.write",
    "presences.read",
    "presences.write",
    "gateway.connect",
    "payment_sources.country_code",
    "sdk.social_layer_presence",
    "sdk.social_layer",
    "lobbies.write",
];

const READ_MESSAGES: [&str; 7] = [
    "dm_channels.messages.read",
    "messages.read",
    "personal.dm_channels.messages.read",
    "personal.dm_channels.messages.search",
    "personal.guilds.channels.messages.read",
    "personal.guilds.messages.search",
    "personal.mentions.read",
];

#[derive(Clone, Debug, PartialEq)]
pub struct InstallType {
    pub kind: u8,
    pub scopes: Vec<String>,
    pub permissions: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    Url(String),
    Types(Vec<InstallType>),
    None,
}

fn perms(v: &Value) -> u64 {
    match v {
        Value::String(s) => s.parse().unwrap_or(0),
        Value::Number(n) => n.as_u64().unwrap_or(0),
        _ => 0,
    }
}

fn scopes(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

pub fn plan(app: &Value) -> Plan {
    if let Some(u) = app["custom_install_url"].as_str().filter(|u| !u.is_empty()) {
        return Plan::Url(u.to_owned());
    }
    let mut types: Vec<InstallType> = app["integration_types_config"]
        .as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| {
                    let p = v.get("oauth2_install_params").filter(|p| !p.is_null())?;
                    Some(InstallType { kind: k.parse().ok()?, scopes: scopes(&p["scopes"]), permissions: perms(&p["permissions"]) })
                })
                .collect()
        })
        .unwrap_or_default();
    if !types.is_empty() {
        types.sort_by_key(|t| if t.kind == USER_INSTALL { 0 } else { 1 });
        return Plan::Types(types);
    }
    let p = &app["install_params"];
    if p.is_object() {
        return Plan::Types(vec![InstallType { kind: GUILD_INSTALL, scopes: scopes(&p["scopes"]), permissions: perms(&p["permissions"]) }]);
    }
    Plan::None
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthGuild {
    pub id: u64,
    pub name: SharedString,
    pub permissions: u64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Auth {
    pub name: SharedString,
    pub icon: Option<SharedString>,
    pub bot: Option<bool>,
    pub guilds: Vec<AuthGuild>,
    pub guild_count: Option<u64>,
    pub privacy: bool,
    pub terms: bool,
}

fn id_of(v: &Value) -> u64 {
    v.as_str().and_then(|s| s.parse().ok()).unwrap_or(0)
}

pub fn parse_auth(v: &Value) -> Option<Auth> {
    let a = v.get("application")?;
    let id = id_of(&a["id"]);
    let icon = a["icon"].as_str().map(|h| SharedString::from(format!("https://cdn.discordapp.com/app-icons/{id}/{h}.png?size=80")));
    let bot = v.get("bot").filter(|b| b.is_object()).map(|b| b["public_flags"].as_u64().unwrap_or(0) & (1 << 16) != 0);
    let mut guilds: Vec<AuthGuild> = v["guilds"]
        .as_array()
        .map(|g| g.iter().map(|g| AuthGuild { id: id_of(&g["id"]), name: g["name"].as_str().unwrap_or_default().to_owned().into(), permissions: perms(&g["permissions"]) }).collect())
        .unwrap_or_default();
    guilds.sort_by_key(|g| g.name.to_lowercase());
    Some(Auth {
        name: a["name"].as_str().unwrap_or_default().to_owned().into(),
        icon,
        bot,
        guilds,
        guild_count: a["approximate_guild_count"].as_u64().or_else(|| v["bot"]["approximate_guild_count"].as_u64()),
        privacy: a["privacy_policy_url"].is_string(),
        terms: a["terms_of_service_url"].is_string(),
    })
}

pub fn account_scopes(requested: &[String]) -> Vec<&'static str> {
    ACCOUNT_SCOPES.iter().copied().filter(|s| requested.iter().any(|r| r == s)).collect()
}

pub fn scope_text(scope: &str, all: &[&str]) -> SharedString {
    match scope {
        "identify" => crate::t!("oauth.scope_identify"),
        "openid" => crate::t!("oauth.scope_openid"),
        "email" => crate::t!("oauth.scope_email"),
        "bot" => crate::t!("oauth.scope_bot"),
        "connections" => crate::t!("oauth.scope_connections"),
        "messages.read" => crate::t!("oauth.scope_messages_read"),
        "guilds" => crate::t!("oauth.scope_guilds"),
        "guilds.join" => crate::t!("oauth.scope_guilds_join"),
        "guilds.members.read" if !all.contains(&"voice") => crate::t!("oauth.scope_members_read"),
        "gdm.join" => crate::t!("oauth.scope_gdm_join"),
        "applications.commands" => crate::t!("oauth.scope_commands"),
        "voice" => crate::t!("oauth.scope_voice"),
        "role_connections.write" => crate::t!("oauth.scope_role_connections"),
        other => other.to_owned().into(),
    }
}

pub fn shield_text(requested: &[String]) -> SharedString {
    let has = |s: &str| requested.iter().any(|r| r == s);
    if has("dm_channels.messages.write") || has("sdk.social_layer") {
        crate::t!("oauth.shield_write")
    } else if READ_MESSAGES.iter().any(|s| has(s)) {
        crate::t!("oauth.shield_read")
    } else {
        crate::t!("oauth.shield_none")
    }
}

pub fn needs_guild(kind: u8, requested: &[String]) -> bool {
    kind == GUILD_INSTALL && requested.iter().any(|s| s == "bot" || s == "applications.commands")
}

pub fn wants_permissions(requested: &[String], permissions: u64) -> bool {
    requested.iter().any(|s| s == "bot") && permissions != 0
}

pub fn created_at(id: u64) -> i64 {
    (((id >> 22) + 1_420_070_400_000) / 1000) as i64
}

pub fn requested_bits(permissions: u64) -> Vec<u32> {
    crate::ui::app_profile::PERMISSION_ORDER.iter().copied().filter(|b| permissions & (1u64 << b) != 0).collect()
}

pub fn authorize_write(app_id: u64, s: &[String], kind: u8, guild: Option<u64>, permissions: u64, authorize: bool, location: (Option<u64>, Option<u64>, i64)) -> GuardedWrite {
    let scope: String = crate::ui::discovery::encode(&s.join(" "));
    let mut body = json!({
        "permissions": permissions.to_string(),
        "authorize": authorize,
        "integration_type": kind,
        "location_context": {
            "guild_id": location.0.map_or("10000".to_owned(), |g| g.to_string()),
            "channel_id": location.1.map_or("10000".to_owned(), |c| c.to_string()),
            "channel_type": location.2,
        },
    });
    if let Some(g) = guild.filter(|_| kind == GUILD_INSTALL) {
        body["guild_id"] = json!(g.to_string());
    }
    GuardedWrite {
        method: reqwest::Method::POST,
        path: format!("/oauth2/authorize?client_id={app_id}&scope={scope}"),
        body: Some(body),
        form: None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Loading,
    SelectType,
    Scopes,
    BotPermissions,
    Failed(SharedString),
}

pub struct State {
    pub app_id: u64,
    pub step: Step,
    pub types: Vec<InstallType>,
    pub kind: Option<u8>,
    pub auth: Option<Auth>,
    pub guild: Option<u64>,
    pub denied: u64,
    pub picker: bool,
    pub fake: usize,
    pub busy: bool,
}

impl State {
    fn current(&self) -> Option<&InstallType> {
        self.types.iter().find(|t| Some(t.kind) == self.kind)
    }
}

fn update(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::AppInstall(s)) = m {
            f(s);
        }
    });
}

fn read<R>(f: impl FnOnce(&State) -> R) -> Option<R> {
    with_modal(|m| match m {
        Some(Modal::AppInstall(s)) => Some(f(s)),
        _ => None,
    })
}

pub fn start(app: &mut PhoenixApp, app_id: u64, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let fake = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0) % 8) as usize;
    open_modal(
        Modal::AppInstall(Box::new(State { app_id, step: Step::Loading, types: Vec::new(), kind: None, auth: None, guild: None, denied: 0, picker: false, fake, busy: false })),
        Some(window),
    );
    let fut = app.backend.directory_get(format!("/applications/{app_id}/public"));
    cx.spawn_in(window, async move |this, cx| {
        let res = fut.await;
        let _ = this.update_in(cx, |a, window, cx| {
            match res.as_ref().map(plan) {
                Ok(Plan::Url(u)) => {
                    close_modal();
                    crate::ui::modals::leave_link::click(a, &u, &u, window, cx);
                }
                Ok(Plan::Types(types)) => {
                    let single = (types.len() == 1).then(|| types[0].kind);
                    update(|s| {
                        s.types = types;
                        s.kind = single;
                        s.step = if single.is_some() { Step::Scopes } else { Step::SelectType };
                    });
                    if single.is_some() {
                        fetch_auth(a, cx);
                    }
                }
                Ok(Plan::None) => update(|s| s.step = Step::Failed(crate::t!("oauth.error_title"))),
                Err(e) => {
                    let msg: SharedString = e.0.clone().into();
                    update(|s| s.step = Step::Failed(msg));
                }
            }
            cx.notify();
        });
    })
    .detach();
}

fn fetch_auth(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some((id, t)) = read(|s| (s.app_id, s.current().cloned())) else { return };
    let Some(t) = t else { return };
    update(|s| s.auth = None);
    let scope: String = crate::ui::discovery::encode(&t.scopes.join(" "));
    let fut = app.backend.directory_get(format!("/oauth2/authorize?client_id={id}&scope={scope}&integration_type={}", t.kind));
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |_, cx| {
            match res {
                Ok(v) => match parse_auth(&v) {
                    Some(auth) => update(|s| s.auth = Some(auth)),
                    None => update(|s| s.step = Step::Failed(crate::t!("oauth.error_title"))),
                },
                Err(e) => {
                    let msg: SharedString = e.0.clone().into();
                    update(|s| s.step = Step::Failed(msg));
                }
            }
            cx.notify();
        });
    })
    .detach();
}

pub fn channel_type(kind: &crate::model::ChannelKind, guild: bool) -> i64 {
    use crate::model::ChannelKind as K;
    match kind {
        K::Text if !guild => 1,
        K::Text => 0,
        K::Voice => 2,
        K::Category => 4,
        K::Announcement => 5,
        K::Stage => 13,
        K::Forum => 15,
    }
}

fn location(app: &PhoenixApp) -> (Option<u64>, Option<u64>, i64) {
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c));
    (ch.and_then(|c| c.guild).map(|g| g.0), ch.map(|c| c.id.0), ch.map_or(-1, |c| channel_type(&c.kind, c.guild.is_some())))
}

fn send(app: &mut PhoenixApp, authorize: bool, cx: &mut Context<PhoenixApp>) {
    let at = location(app);
    let w = read(|s| {
        let t = s.current()?;
        Some(authorize_write(s.app_id, &t.scopes, t.kind, s.guild, t.permissions & !s.denied, authorize, at))
    })
    .flatten();
    let Some(w) = w else {
        close_modal();
        cx.notify();
        return;
    };
    if !authorize {
        cx.background_spawn(app.backend.guarded_write(w)).detach();
        close_modal();
        cx.notify();
        return;
    }
    update(|s| s.busy = true);
    let fut = app.backend.guarded_write(w);
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |a, cx| {
            update(|s| s.busy = false);
            match res {
                Ok(()) => close_modal(),
                Err(_) => a.parity_toast(crate::t!("acct.blocked"), cx),
            }
            cx.notify();
        });
    })
    .detach();
}

fn text(t: impl Into<SharedString>, size: f32, line: f32, weight: FontWeight, color: usize) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(c(tok(color))).child(t.into())
}

fn header(app: &PhoenixApp, auth: &Auth, account: bool) -> AnyElement {
    let ring = |el: AnyElement| div().size(z(64.)).flex_none().rounded_full().border_1().border_color(c(tok(idx::BORDER_SUBTLE))).overflow_hidden().child(el);
    let app_icon = match &auth.icon {
        Some(u) => img(u.clone()).size_full().rounded_full().into_any_element(),
        None => div().size_full().bg(c(tok(idx::BACKGROUND_BASE_LOWER))).into_any_element(),
    };
    let dots = div().mx(z(24.)).flex().children((0..3).map(|_| div().mx(z(2.)).size(z(4.)).rounded_full().bg(c(tok(idx::CHANNEL_ICON))).opacity(0.1)));
    let name = div()
        .flex()
        .items_center()
        .text_size(z(20.))
        .line_height(z(24.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(tok(idx::TEXT_STRONG)))
        .child(auth.name.clone())
        .children(auth.bot.map(|v| crate::ui::members::bot_tag_px(v).mt(z(4.)).ml(z(8.))));
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(z(8.))
        .child(div().mb(z(16.)).flex().items_center().justify_between().child(ring(app_icon)).child(dots).child(ring(crate::ui::avatar(Some(&app.me), 64.))))
        .child(name)
        .child(text(if account { crate::t!("oauth.header_account") } else { crate::t!("oauth.header_connect") }, 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT))
        .into_any_element()
}

fn scope_row(label: SharedString, fake: bool) -> AnyElement {
    div()
        .flex()
        .mt(z(16.))
        .child(
            gpui::svg()
                .path(crate::icons::path(if fake { "settings/circle-x" } else { "circle-check" }))
                .size(z(20.))
                .mr(z(12.))
                .flex_none()
                .text_color(c(tok(idx::TEXT_MUTED)))
                .when(fake, |d| d.opacity(0.6)),
        )
        .child(div().flex_1().flex().flex_col().justify_center().child(text(label, 16., 20., FontWeight::NORMAL, if fake { idx::TEXT_MUTED } else { idx::TEXT_DEFAULT })))
        .into_any_element()
}

fn detail(icon: &str, t: SharedString) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(z(8.))
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).flex_none().text_color(c(tok(idx::TEXT_MUTED))))
        .child(text(t, 12., 16., FontWeight::NORMAL, idx::TEXT_MUTED))
        .into_any_element()
}

fn fake_text(i: usize) -> SharedString {
    match i {
        0 => crate::t!("oauth.fake_0"),
        1 => crate::t!("oauth.fake_1"),
        2 => crate::t!("oauth.fake_2"),
        3 => crate::t!("oauth.fake_3"),
        4 => crate::t!("oauth.fake_4"),
        5 => crate::t!("oauth.fake_5"),
        6 => crate::t!("oauth.fake_6"),
        _ => crate::t!("oauth.fake_7"),
    }
}

fn guild_select(s: &State, auth: &Auth, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let manageable: Vec<AuthGuild> = auth.guilds.iter().filter(|g| g.permissions & MANAGE_GUILD != 0 || g.permissions & 8 != 0).cloned().collect();
    let current = s.guild.and_then(|g| manageable.iter().find(|x| x.id == g)).map(|g| g.name.clone());
    let field = crate::ui::controls::select_field("oauth-guild", current.unwrap_or_else(|| crate::t!("oauth.guild_placeholder")))
        .on_click(cx.listener(|_, _, _, cx| {
            update(|s| s.picker = !s.picker);
            cx.notify();
        }));
    let mut wrap = div().relative().w_full().child(field);
    if s.picker {
        let list = div()
            .id("oauth-guild-list")
            .occlude()
            .max_h(z(5. * 40. + 8.))
            .overflow_y_scroll()
            .p(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .flex()
            .flex_col()
            .children(manageable.iter().map(|g| {
                let id = g.id;
                let on = s.guild == Some(id);
                div()
                    .id(("oauth-guild-opt", id))
                    .flex_none()
                    .h(z(40.))
                    .px(z(8.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                    .hover(|st| st.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                    .cursor_pointer()
                    .child(div().truncate().child(g.name.clone()))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        update(|s| {
                            s.guild = Some(id);
                            s.picker = false;
                            s.denied = 0;
                        });
                        cx.notify();
                    }))
            }));
        wrap = wrap.child(deferred(div().absolute().top(z(44.)).left_0().w_full().child(list)).with_priority(30));
    }
    div()
        .mt(z(16.))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(text(crate::t!("oauth.guild_label"), 14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT))
        .child(wrap)
        .child(text(crate::t!("oauth.guild_helper"), 12., 16., FontWeight::NORMAL, idx::TEXT_MUTED))
        .into_any_element()
}

fn scopes_body(s: &State, auth: &Auth, t: &InstallType, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let account = account_scopes(&t.scopes);
    let mut rows: Vec<AnyElement> = account.iter().map(|sc| scope_row(scope_text(sc, &account), false)).collect();
    if t.kind == USER_INSTALL && t.scopes.iter().any(|s| s == "applications.commands") {
        rows.push(scope_row(crate::t!("oauth.scope_dm"), false));
    }
    rows.push(scope_row(fake_text(s.fake), true));
    let scopes = (!account.is_empty()).then(|| {
        div()
            .pb(z(24.))
            .border_b_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .child(text(crate::t!("oauth.scopes_title", app = auth.name.clone()), 12., 16., FontWeight::MEDIUM, idx::TEXT_DEFAULT))
            .children(rows)
    });
    div().flex().flex_col().children(scopes).when(needs_guild(t.kind, &t.scopes), |d| d.child(guild_select(s, auth, cx))).into_any_element()
}

fn details(s: &State, auth: &Auth, t: &InstallType) -> AnyElement {
    let policy = match (auth.privacy, auth.terms) {
        (true, true) => crate::t!("oauth.policy_both", app = auth.name.clone()),
        (true, false) => crate::t!("oauth.policy_privacy", app = auth.name.clone()),
        (false, true) => crate::t!("oauth.policy_terms", app = auth.name.clone()),
        _ => crate::t!("oauth.policy_none", app = auth.name.clone()),
    };
    div()
        .pt(z(24.))
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(detail("lock", policy))
        .child(detail("settings/clock", crate::t!("oauth.active_since", date = crate::presence::format_date(created_at(s.app_id)))))
        .children(
            auth.guild_count
                .filter(|_| t.scopes.iter().any(|x| x == "bot"))
                .map(|n| detail("mod/robot", crate::t!("oauth.guild_count", count = n.to_string()))),
        )
        .child(detail("shield", shield_text(&t.scopes)))
        .into_any_element()
}

fn permissions_body(s: &State, auth: &Auth, t: &InstallType, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(g) = s.guild.and_then(|g| auth.guilds.iter().find(|x| x.id == g)) else { return div().into_any_element() };
    let admin = g.permissions & 8 != 0;
    let bits = requested_bits(t.permissions);
    let (have, lack): (Vec<u32>, Vec<u32>) = bits.into_iter().partition(|b| admin || g.permissions & (1u64 << b) != 0);
    let grant = (!have.is_empty()).then(|| {
        div().child(text(crate::t!("oauth.perms_grant", app = auth.name.clone(), guild = g.name.clone()), 14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT)).child(
            div().flex().flex_col().gap(z(12.)).children(have.iter().map(|b| {
                let bit = *b;
                let on = s.denied & (1u64 << bit) == 0;
                div().mt(z(16.)).child(crate::ui::controls::checkbox_option(("oauth-perm", bit as u64), crate::ui::app_profile::permission_label(bit), on).on_click(cx.listener(
                    move |_, _, _, cx| {
                        update(|s| s.denied ^= 1u64 << bit);
                        cx.notify();
                    },
                )))
            })),
        )
    });
    let missing = (!lack.is_empty()).then(|| {
        div().child(text(crate::t!("oauth.perms_have", app = auth.name.clone()), 14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT)).child(
            div().flex().flex_col().gap(z(12.)).children(lack.iter().map(|b| {
                div()
                    .mt(z(16.))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::MEDIUM)
                    .child(
                        div()
                            .size(z(18.))
                            .mr(z(8.))
                            .flex_none()
                            .rounded(z(1.8))
                            .bg(c(tok(idx::BACKGROUND_FEEDBACK_MOD_CRITICAL)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(gpui::svg().path(crate::icons::path("close-small")).size(z(18.)).text_color(c(tok(idx::WHITE)))),
                    )
                    .child(text(crate::ui::app_profile::permission_label(*b), 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT))
            })),
        )
    });
    div().flex().flex_col().gap(z(24.)).children(grant).children(missing).into_any_element()
}

fn type_list(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .children(s.types.iter().map(|t| {
            let kind = t.kind;
            let (icon, title, sub) = if kind == USER_INSTALL {
                ("invite", crate::t!("oauth.type_user"), crate::t!("oauth.type_user_sub"))
            } else {
                ("server/server", crate::t!("oauth.type_guild"), crate::t!("oauth.type_guild_sub"))
            };
            div()
                .id(("oauth-type", kind as u64))
                .flex()
                .items_center()
                .gap(z(12.))
                .p(z(12.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .cursor_pointer()
                .hover(|st| st.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .child(gpui::svg().path(crate::icons::path(icon)).size(z(24.)).flex_none().text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(text(title, 16., 20., FontWeight::MEDIUM, idx::INTERACTIVE_TEXT_ACTIVE))
                        .child(text(sub, 12., 16., FontWeight::MEDIUM, idx::TEXT_MUTED)),
                )
                .child(gpui::svg().path(crate::icons::path("chevron-right")).size(z(24.)).flex_none().text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                .on_click(cx.listener(move |a, _, _, cx| {
                    update(|s| {
                        s.kind = Some(kind);
                        s.step = Step::Scopes;
                        s.guild = None;
                        s.denied = 0;
                    });
                    fetch_auth(a, cx);
                    cx.notify();
                }))
        }))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let e = cx.entity();
    let close: crate::ui::modal::Action = {
        let e = e.clone();
        std::rc::Rc::new(move |_, cx| {
            e.update(cx, |a, cx| send(a, false, cx));
        })
    };
    let loading = || div().h(z(270.)).flex().items_center().justify_center().child(text(crate::t!("oauth.loading"), 14., 18., FontWeight::NORMAL, idx::TEXT_MUTED)).into_any_element();
    let boxed = |body: AnyElement| {
        div()
            .p(z(32.))
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .border_1()
            .border_color(c(tok(idx::BORDER_MUTED)))
            .child(body)
            .into_any_element()
    };
    let multi = s.types.len() > 1;
    let back_or_cancel = |cx: &mut Context<PhoenixApp>, to: Option<Step>| {
        let label = if to.is_some() { crate::t!("oauth.back") } else { crate::t!("oauth.cancel") };
        button("oauth-cancel", label, BtnKind::Secondary).flex_1().on_click(cx.listener(move |a, _, _, cx| {
            match to.clone() {
                Some(step) => update(|s| {
                    s.step = step;
                    s.picker = false;
                }),
                None => send(a, false, cx),
            }
            cx.notify();
        }))
    };
    let (body, footer): (AnyElement, Option<AnyElement>) = match (&s.step, s.auth.as_ref(), s.current()) {
        (Step::Loading, _, _) => (loading(), None),
        (Step::Failed(msg), _, _) => (
            text(msg.clone(), 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT).into_any_element(),
            Some(button("oauth-close", crate::t!("oauth.close"), BtnKind::Secondary).flex_1().on_click(cx.listener(|_, _, _, cx| {
                close_modal();
                cx.notify();
            })).into_any_element()),
        ),
        (Step::SelectType, _, _) => (type_list(s, cx), None),
        (Step::Scopes, Some(auth), Some(t)) => {
            let account = !account_scopes(&t.scopes).is_empty();
            let pick = needs_guild(t.kind, &t.scopes) && s.guild.is_none();
            let next = wants_permissions(&t.scopes, t.permissions) && !pick;
            let label = if pick {
                crate::t!("oauth.pick_guild")
            } else if next {
                crate::t!("oauth.continue")
            } else {
                crate::t!("oauth.authorize")
            };
            let primary = button("oauth-next", label, BtnKind::Primary)
                .flex_1()
                .when(pick || s.busy, |d| d.opacity(0.5).cursor_not_allowed())
                .on_click(cx.listener(move |a, _, _, cx| {
                    if pick {
                        return;
                    }
                    if next {
                        update(|s| {
                            s.step = Step::BotPermissions;
                            s.picker = false;
                        });
                    } else {
                        send(a, true, cx);
                    }
                    cx.notify();
                }));
            let content = div().flex().flex_col().child(scopes_body(s, auth, t, cx)).child(details(s, auth, t));
            (
                div().flex().flex_col().gap(z(24.)).child(header(app, auth, account)).child(boxed(content.into_any_element())).into_any_element(),
                Some(div().flex_1().flex().gap(z(8.)).child(back_or_cancel(cx, multi.then_some(Step::SelectType))).child(primary).into_any_element()),
            )
        }
        (Step::BotPermissions, Some(auth), Some(t)) => {
            let primary = button("oauth-authorize", crate::t!("oauth.authorize"), BtnKind::Primary)
                .flex_1()
                .when(s.busy, |d| d.opacity(0.5).cursor_not_allowed())
                .on_click(cx.listener(|a, _, _, cx| {
                    send(a, true, cx);
                    cx.notify();
                }));
            (
                div()
                    .flex()
                    .flex_col()
                    .gap(z(24.))
                    .child(header(app, auth, !account_scopes(&t.scopes).is_empty()))
                    .child(boxed(permissions_body(s, auth, t, cx)))
                    .into_any_element(),
                Some(div().flex_1().flex().gap(z(8.)).child(back_or_cancel(cx, Some(Step::Scopes))).child(primary).into_any_element()),
            )
        }
        _ => (loading(), None),
    };
    let title: SharedString = match s.step {
        Step::Failed(_) => crate::t!("oauth.error_title"),
        _ => SharedString::default(),
    };
    card_with(WIDTH, title, None, body, footer, close)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_follows_the_client_order() {
        assert_eq!(plan(&json!({"custom_install_url": "https://x.y/z", "install_params": {"scopes": ["bot"], "permissions": "8"}})), Plan::Url("https://x.y/z".into()));
        let both = plan(&json!({"integration_types_config": {
            "0": {"oauth2_install_params": {"scopes": ["bot", "applications.commands"], "permissions": "2048"}},
            "1": {"oauth2_install_params": {"scopes": ["applications.commands"], "permissions": "0"}}
        }}));
        let Plan::Types(t) = both else { panic!() };
        assert_eq!(t.iter().map(|t| t.kind).collect::<Vec<_>>(), vec![USER_INSTALL, GUILD_INSTALL]);
        assert_eq!(t[1].permissions, 2048);
        let only_guild = plan(&json!({"integration_types_config": {"0": {"oauth2_install_params": {"scopes": ["bot"], "permissions": "8"}}, "1": {}}}));
        assert_eq!(only_guild, Plan::Types(vec![InstallType { kind: GUILD_INSTALL, scopes: vec!["bot".into()], permissions: 8 }]));
        assert_eq!(
            plan(&json!({"install_params": {"scopes": ["bot"], "permissions": "16"}})),
            Plan::Types(vec![InstallType { kind: GUILD_INSTALL, scopes: vec!["bot".into()], permissions: 16 }])
        );
        assert_eq!(plan(&json!({})), Plan::None);
    }

    #[test]
    fn account_scopes_keep_the_client_order_and_skip_webhooks() {
        let req: Vec<String> = ["applications.commands", "webhook.incoming", "bot", "identify"].iter().map(|s| s.to_string()).collect();
        assert_eq!(account_scopes(&req), vec!["bot", "identify", "applications.commands"]);
        assert!(needs_guild(GUILD_INSTALL, &req));
        assert!(!needs_guild(USER_INSTALL, &req));
        assert!(wants_permissions(&req, 8));
        assert!(!wants_permissions(&req, 0));
    }

    #[test]
    fn authorize_is_a_guarded_post_with_the_client_body() {
        let s: Vec<String> = vec!["bot".into(), "applications.commands".into()];
        let w = authorize_write(42, &s, GUILD_INSTALL, Some(7), 2048, true, (Some(1), Some(2), 0));
        assert_eq!(w.method, reqwest::Method::POST);
        assert_eq!(w.path, "/oauth2/authorize?client_id=42&scope=bot%20applications.commands");
        let b = w.body.clone().unwrap();
        assert_eq!(b["guild_id"], "7");
        assert_eq!(b["permissions"], "2048");
        assert_eq!(b["authorize"], true);
        assert_eq!(b["integration_type"], 0);
        assert_eq!(b["location_context"]["channel_id"], "2");
        assert!(w.refusal().is_write_blocked());
        let user = authorize_write(42, &s, USER_INSTALL, Some(7), 0, false, (None, None, -1));
        assert!(user.body.unwrap().get("guild_id").is_none());
    }

    #[test]
    fn parses_the_authorize_payload() {
        let v = json!({
            "application": {"id": "42", "name": "Bot", "icon": "abc", "privacy_policy_url": "https://p", "approximate_guild_count": 12},
            "bot": {"public_flags": 65536},
            "guilds": [{"id": "2", "name": "b", "permissions": "32"}, {"id": "1", "name": "A", "permissions": "0"}]
        });
        let a = parse_auth(&v).unwrap();
        assert_eq!(a.icon.as_deref(), Some("https://cdn.discordapp.com/app-icons/42/abc.png?size=80"));
        assert_eq!(a.bot, Some(true));
        assert_eq!(a.guilds.iter().map(|g| g.id).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(a.guild_count, Some(12));
        assert!(a.privacy && !a.terms);
        assert_eq!(created_at(175928847299117063), 1462015105);
        assert_eq!(requested_bits((1 << 3) | (1 << 11)), vec![3, 11]);
    }
}

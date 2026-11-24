//! Account features of the real backend: MFA methods, QR (remote auth)
//! login, synced settings and desktop notifications.

use phoenix_api::{
    remote_auth::{self, RemoteAuthEvent},
    rest::settings::{self as api_settings, PreloadedUserSettings, SettingChange, Theme},
};

use super::*;
use crate::theme::Density;
use crate::{
    auth::{MfaMethod, MfaOptions, RemoteAuthUpdate},
    ui::settings::{AppTheme, SettingsPatch, SyncedSettings},
};

/// Password login answered with an MFA ticket.
pub(super) fn mfa_required(inner: &Inner, ticket: String, totp: bool, sms: bool, backup: bool) -> Result<ReadyData, BackendError> {
    if !totp && !sms && !backup {
        return Err(err(
            "This account only allows a security key for 2FA, which Phoenix can't use. Log in with the QR code or a token instead.",
        ));
    }
    *inner.mfa_ticket.lock() = Some(Secret::new(ticket));
    *inner.mfa_opts.lock() = MfaOptions { totp, sms, backup };
    Err(err(if totp {
        "Enter the 6-digit code from your authenticator app."
    } else {
        "Choose how to receive your login code below."
    }))
}

impl DiscordBackend {
    pub(super) fn mfa_options_impl(&self) -> MfaOptions {
        *self.inner.mfa_opts.lock()
    }

    pub(super) fn login_mfa_impl(&self, method: MfaMethod, code: String) -> BoxFuture<ReadyData> {
        self.spawn(move |inner| async move {
            let ticket = inner.mfa_ticket.lock().clone().ok_or_else(|| err("Log in with your password first."))?;
            let code: String = code.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
            if code.is_empty() {
                return Err(err("Enter your code."));
            }
            let rest = RestClient::unauthenticated(inner.props.clone());
            let token = match method {
                MfaMethod::Totp => rest.login_mfa_totp(ticket.expose(), &code).await,
                MfaMethod::Backup => rest.login_mfa_backup(ticket.expose(), &code).await,
                MfaMethod::Sms => rest.login_mfa_sms(ticket.expose(), &code).await,
            }
            .map_err(api_err)?;
            inner.mfa_ticket.lock().take();
            connect(inner, Secret::new(token), true).await
        })
    }

    pub(super) fn send_mfa_sms_impl(&self) -> BoxFuture<String> {
        self.spawn(move |inner| async move {
            let ticket = inner.mfa_ticket.lock().clone().ok_or_else(|| err("Log in with your password first."))?;
            let rest = RestClient::unauthenticated(inner.props.clone());
            rest.login_mfa_sms_send(ticket.expose()).await.map_err(api_err)
        })
    }

    pub(super) fn cancel_remote_auth_impl(&self) {
        if let Some(h) = self.inner.remote_auth.lock().take() {
            h.abort();
        }
    }

    pub(super) fn remote_auth_impl(&self) -> BoxFuture<ReadyData> {
        self.cancel_remote_auth_impl();
        self.spawn(move |inner| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let rest = RestClient::unauthenticated(inner.props.clone());
            let task = tokio::spawn(remote_auth::run(rest, tx));
            *inner.remote_auth.lock() = Some(task.abort_handle());
            while let Some(ev) = rx.recv().await {
                match ev {
                    RemoteAuthEvent::Qr(url) => inner.emit(AppEvent::RemoteAuth(RemoteAuthUpdate::Qr(url))),
                    RemoteAuthEvent::Scanned { username, .. } => {
                        inner.emit(AppEvent::RemoteAuth(RemoteAuthUpdate::Scanned { username }))
                    }
                    RemoteAuthEvent::Token(t) => {
                        inner.remote_auth.lock().take();
                        return connect(inner, Secret::new(t), true).await;
                    }
                    RemoteAuthEvent::Cancelled => return Err(err("")),
                }
            }
            match task.await {
                Ok(Err(e)) => Err(err(format!("Couldn't reach Discord's QR login ({}).", short(&e)))),
                _ => Err(err("")),
            }
        })
    }

    pub(super) fn load_settings_impl(&self) -> BoxFuture<SyncedSettings> {
        self.with_session(move |_, s| async move {
            let mut out = match s.rest.get_settings_proto_raw().await {
                Ok(raw) => synced_raw(&raw)?,
                Err(_) => {
                    let v = s.rest.get_settings_json().await.map_err(api_err)?;
                    synced(&api_settings::from_json(&v))
                }
            };
            s.rest.set_locale(&out.locale);
            // Consents and e-mail preferences live on their own routes (best effort).
            let consent = s.rest.get_consent().await.ok();
            let email = s.rest.get_email_settings().await.ok();
            out.rest = crate::ui::settings::synced_map::read_rest(consent.as_ref(), email.as_ref());
            Ok(out)
        })
    }

    pub(super) fn save_settings_impl(&self, patch: SettingsPatch) -> BoxFuture<SyncedSettings> {
        self.with_session(move |_, s| async move {
            if let SettingsPatch::Rest(id, v) = &patch {
                use crate::ui::settings::synced_map::{Rest, rest};
                match rest(id).map(|r| r.0) {
                    Some(Rest::Consent(k)) => s.rest.set_consent(k, *v).await.map_err(api_err)?,
                    Some(Rest::Email(k)) => s.rest.patch_email_setting(k, *v).await.map(|_| ()).map_err(api_err)?,
                    None => return Err(err(format!("unknown setting {id}"))),
                }
                // The caller replaces its whole copy: return the full synced state.
                let raw = s.rest.get_settings_proto_raw().await.map_err(api_err)?;
                let mut out = synced_raw(&raw)?;
                out.rest.insert(*id, *v);
                return Ok(out);
            }
            let change = match &patch {
                SettingsPatch::Theme(t) => SettingChange::Theme(theme_to_api(*t)),
                SettingsPatch::Compact(b) => SettingChange::Compact(*b),
                SettingsPatch::DeveloperMode(b) => SettingChange::DeveloperMode(*b),
                SettingsPatch::Locale(l) => SettingChange::Locale(l.to_string()),
                SettingsPatch::AllowServerDms(b) => SettingChange::DefaultGuildsRestricted(!*b),
                SettingsPatch::Rest(..) => unreachable!("handled above"),
                SettingsPatch::Proto(id, v) => {
                    crate::ui::settings::synced_map::change(id, *v).ok_or_else(|| err(format!("unknown setting {id}")))?
                }
                SettingsPatch::Density(d) => SettingChange::UiDensity(density_to_proto(*d)),
            };
            let raw = s.rest.patch_setting_raw(&change).await.map_err(api_err)?;
            let r = synced_raw(&raw);
            if let Ok(set) = &r {
                s.rest.set_locale(&set.locale);
            }
            r
        })
    }
}

fn short(e: &phoenix_api::Error) -> String {
    let s = e.to_string();
    s.chars().take(120).collect()
}

fn theme_to_api(t: AppTheme) -> Theme {
    match t {
        AppTheme::Dark => Theme::Dark,
        AppTheme::Light => Theme::Light,
        AppTheme::Ash => Theme::Ash,
        AppTheme::Onyx => Theme::Onyx,
    }
}

/// [`synced`] plus every [`crate::ui::settings::synced_map`] entry.
fn synced_raw(raw: &[u8]) -> Result<SyncedSettings, crate::backend::BackendError> {
    let p = PreloadedUserSettings::decode_raw(raw).map_err(api_err)?;
    let mut s = synced(&p);
    s.proto = crate::ui::settings::synced_map::read_all(raw);
    Ok(s)
}

pub fn synced(p: &PreloadedUserSettings) -> SyncedSettings {
    let d = SyncedSettings::default();
    crate::ui::settings::account::set_activity_restricted(p.privacy.as_ref().map(|x| x.activity_restricted_guild_ids.clone()).unwrap_or_default());
    SyncedSettings {
        theme: match p.theme() {
            Theme::Dark => AppTheme::Dark,
            Theme::Light => AppTheme::Light,
            Theme::Ash => AppTheme::Ash,
            Theme::Onyx => AppTheme::Onyx,
        },
        compact: p.compact(),
        developer_mode: p.developer_mode(),
        locale: p.locale().filter(|l| !l.is_empty()).map(|l| l.to_owned().into()).unwrap_or(d.locale),
        allow_server_dms: !p.default_guilds_restricted(),
        proto: Default::default(),
        rest: Default::default(),
        density: density_from_proto(p.ui_density()).unwrap_or(d.density),
    }
}

/// `UIDensity` proto enum (client: UNSET 0, COMPACT 1, COZY 2, RESPONSIVE 3, DEFAULT 4).
fn density_from_proto(v: i32) -> Option<Density> {
    match v {
        1 => Some(Density::Compact),
        2 => Some(Density::Cozy),
        3 | 4 => Some(Density::Default),
        _ => None,
    }
}

fn density_to_proto(d: Density) -> i32 {
    match d {
        Density::Compact => 1,
        Density::Cozy => 2,
        Density::Default => 4,
    }
}

/// Gateway hook (pump): desktop notifications + notification settings updates.
pub(super) fn on_gateway(s: &Session, ev: &GatewayEvent) {
    match ev {
        GatewayEvent::MessageCreate(m) => {
            let me = s.adapter.lock().me;
            let viewing = *s.selected_channel.lock();
            crate::notifications::on_message(&s.notify.lock(), &s.cache, me, viewing, m);
        }
        GatewayEvent::Unknown { name, data } if name == "USER_GUILD_SETTINGS_UPDATE" => s.notify.lock().update(data),
        _ => {}
    }
}

impl DiscordBackend {
    /// `GET /auth/sessions`: number of logged-in devices (Settings > Account).
    pub(super) fn session_count_impl(&self) -> BoxFuture<Option<usize>> {
        self.with_session(move |_, s| async move {
            let v = s.rest.request_json(reqwest::Method::GET, "/auth/sessions", None).await.map_err(api_err)?;
            Ok(v.get("user_sessions").and_then(serde_json::Value::as_array).map(Vec::len))
        })
    }
}

impl DiscordBackend {
    /// Settings > Billing: three read-only GETs.
    pub(super) fn billing_impl(&self) -> BoxFuture<Option<crate::ui::settings::billing::Billing>> {
        use crate::ui::settings::billing as b;
        self.with_session(move |_, s| async move {
            let get = |p: &'static str| {
                let rest = s.rest.clone();
                async move { rest.request_json(reqwest::Method::GET, p, None).await.ok() }
            };
            let raw_sources = get("/users/@me/billing/payment-sources").await.unwrap_or(serde_json::Value::Null);
            let sources = b::mark_default(b::parse_sources(&raw_sources));
            let payments = get("/users/@me/billing/payments?limit=10").await.map(|v| b::parse_payments(&v)).unwrap_or_default();
            let code = get("/users/@me/billing/country-code")
                .await
                .and_then(|v| v.get("country_code").and_then(serde_json::Value::as_str).map(str::to_owned));
            let country = b::source_country(&raw_sources, code);
            Ok(Some(b::Billing { country, sources, payments }))
        })
    }

    /// Settings > Subscriptions, the client's read-only routes (see `ui::settings::subscriptions`).
    pub(super) fn subscriptions_impl(&self) -> BoxFuture<Option<crate::ui::settings::subscriptions::Subs>> {
        use crate::ui::settings::subscriptions as sub;
        self.with_session(move |_, s| async move {
            let get = |p: String| {
                let rest = s.rest.clone();
                async move { rest.request_json(reqwest::Method::GET, &p, None).await.ok() }
            };
            let me = s.cache.current_user().map(|u| u.id.0).unwrap_or(0);
            let mut squad = None;
            if let Some(m) = get("/users/@me/premium-group/membership".into()).await {
                if let Some(id) = m.get("subscription_id").and_then(serde_json::Value::as_str) {
                    if let Some(v) = get(format!("/users/@me/billing/subscriptions/{id}/members")).await {
                        squad = Some(sub::parse_members(&v, me));
                    }
                }
            }
            let credits = get(format!("/users/@me/applications/{}/entitlements?exclude_consumed=true", sub::NITRO_APP))
                .await
                .map(|v| sub::parse_credits(&v))
                .unwrap_or_default();
            Ok(Some(sub::Subs { squad, credits }))
        })
    }
}

impl DiscordBackend {
    /// Settings > Connections: `GET /oauth2/tokens` (read-only).
    pub(super) fn authorized_apps_impl(&self) -> BoxFuture<Option<Vec<crate::ui::settings::authorized_apps::AuthorizedApp>>> {
        self.with_session(move |_, s| async move {
            let v = s.rest.request_json(reqwest::Method::GET, "/oauth2/tokens", None).await.map_err(api_err)?;
            Ok(Some(crate::ui::settings::authorized_apps::parse(&v)))
        })
    }
}

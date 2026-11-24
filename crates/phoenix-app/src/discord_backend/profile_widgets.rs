use reqwest::Method;
use serde_json::Value;

use super::{DiscordBackend, api_err};
use crate::{
    backend::BoxFuture,
    model::Id,
    ui::settings::profile_widgets::{self as w, AppWidget},
};

impl DiscordBackend {
    pub(super) fn profile_widgets_impl(&self, user: Id) -> BoxFuture<Vec<AppWidget>> {
        self.with_session(move |_, s| async move {
            let uid = user.0;
            let prof = s.rest.request_json(Method::GET, &format!("/users/{uid}/profile"), None).await.map_err(api_err)?;
            let widgets = prof.get("widgets").and_then(Value::as_array).cloned().unwrap_or_default();
            let apps = w::app_ids(&widgets);
            if apps.is_empty() {
                return Ok(Vec::new());
            }
            let ident = s
                .rest
                .request_json(Method::GET, &format!("/users/{uid}/application-identities?with_profiles=true"), None)
                .await
                .unwrap_or(Value::Null);
            let mut out = Vec::new();
            for app in apps {
                let Ok(cfg) = s.rest.request_json(Method::GET, &format!("/applications/{app}/widget-configs"), None).await else {
                    continue;
                };
                if std::env::var_os("PHX_DEBUG_WIDGETS").is_some() {
                    eprintln!("widget config shape {}", crate::discord_backend::profile_games::shape_of(&cfg));
                }
                if let Some(card) = w::build(app, &cfg, &w::identity_data(&ident, app)) {
                    out.push(card);
                }
            }
            Ok(out)
        })
    }

    pub(super) fn owned_collectibles_impl(&self) -> BoxFuture<Vec<crate::ui::settings::collectibles::Collectible>> {
        self.with_session(move |_, s| async move {
            let v = s
                .rest
                .request_json(Method::GET, "/users/@me/collectibles-purchases?variants_return_style=2", None)
                .await
                .map_err(api_err)?;
            Ok(crate::ui::settings::collectibles::parse_purchases(&v))
        })
    }

    pub(super) fn recent_activity_impl(&self, user: Id) -> BoxFuture<Vec<crate::ui::settings::profile_activity::RecentGame>> {
        use crate::ui::settings::profile_activity as pa;
        self.with_session(move |_, s| async move {
            let v = s
                .rest
                .request_json(Method::GET, &format!("/content-inventory/users/{}/outbox", user.0), None)
                .await
                .map_err(api_err)?;
            let mut games = pa::parse_outbox(&v);
            let mut ids: Vec<String> = games.iter().filter(|g| g.app_id != 0).map(|g| g.app_id.to_string()).collect();
            ids.dedup();
            if !ids.is_empty() {
                let q = ids.iter().map(|i| format!("application_ids={i}")).collect::<Vec<_>>().join("&");
                if let Ok(apps) = s.rest.request_json(Method::GET, &format!("/applications/public?{q}"), None).await {
                    pa::apply_icons(&mut games, &apps);
                }
            }
            Ok(games)
        })
    }
}

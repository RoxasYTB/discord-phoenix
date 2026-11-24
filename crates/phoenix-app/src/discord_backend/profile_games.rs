use reqwest::Method;
use serde_json::Value;

use super::{DiscordBackend, api_err};
use crate::{
    backend::BoxFuture,
    model::Id,
    ui::profile_games::{self as g, GameWidget},
};

impl DiscordBackend {
    pub(super) fn game_widgets_impl(&self, user: Id) -> BoxFuture<Vec<GameWidget>> {
        self.with_session(move |_, s| async move {
            let prof = s.rest.request_json(Method::GET, &format!("/users/{}/profile", user.0), None).await.map_err(api_err)?;
            let widgets = prof.get("widgets").and_then(Value::as_array).cloned().unwrap_or_default();
            let mut out = Vec::new();
            for (kind, ids) in g::widget_ids(&widgets) {
                let mut games = Vec::new();
                for id in ids {
                    let Ok(v) = s.rest.request_json(Method::GET, &format!("/games/{id}"), None).await else { continue };
                    if std::env::var_os("PHX_DEBUG_GAMES").is_some() {
                        let shape: Vec<String> = v
                            .as_object()
                            .map(|o| o.iter().map(|(k, x)| format!("{k}:{}", shape_of(x))).collect())
                            .unwrap_or_default();
                        eprintln!("game shape {shape:?}");
                    }
                    games.push(g::parse_game(id, &v));
                }
                out.push(GameWidget { kind, games });
            }
            Ok(out)
        })
    }
}

pub(crate) fn shape_of(v: &Value) -> String {
    match v {
        Value::Object(o) => format!("{{{}}}", o.iter().map(|(k, x)| format!("{k}:{}", shape_of(x))).collect::<Vec<_>>().join(",")),
        Value::Array(a) => format!("[{}]", a.first().map(shape_of).unwrap_or_default()),
        Value::String(s) if s.starts_with("http") || s.starts_with("//") => "url".into(),
        Value::String(_) => "str".into(),
        x => format!("{x}"),
    }
}

impl DiscordBackend {
    pub(super) fn wishlist_impl(&self, user: Id) -> BoxFuture<Vec<crate::ui::profile_wishlist::WishItem>> {
        self.with_session(move |_, s| async move {
            let prof = s.rest.request_json(Method::GET, &format!("/users/{}/profile", user.0), None).await.map_err(api_err)?;
            let Some(id) = crate::ui::profile_wishlist::first_id(&prof) else { return Ok(Vec::new()) };
            let v = s.rest.request_json(Method::GET, &format!("/wishlists/{id}?source=user_profile"), None).await.map_err(api_err)?;
            if std::env::var_os("PHX_DEBUG_WISHLIST").is_some() {
                eprintln!("wishlist shape {}", shape_of(&v));
            }
            Ok(crate::ui::profile_wishlist::items(&v))
        })
    }
}

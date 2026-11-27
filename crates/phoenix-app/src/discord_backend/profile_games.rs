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
            let lists = g::widget_ids(&widgets);
            let mut all: Vec<u64> = Vec::new();
            for id in lists.iter().flat_map(|(_, _, ids)| ids) {
                if !all.contains(id) {
                    all.push(*id);
                }
            }
            let found = fetch_games(&s.rest, &all).await;
            let out = lists
                .into_iter()
                .map(|(order, kind, ids)| GameWidget { kind, games: ids.iter().filter_map(|id| found.get(id).cloned()).collect(), order })
                .collect();
            Ok(out)
        })
    }
}

async fn fetch_games(rest: &phoenix_api::RestClient, ids: &[u64]) -> std::collections::HashMap<u64, g::Game> {
    let mut found = std::collections::HashMap::new();
    for chunk in ids.chunks(20) {
        let query = chunk.iter().map(|id| format!("game_ids={id}")).collect::<Vec<_>>().join("&");
        let Ok(v) = rest.request_json(Method::GET, &format!("/games?{query}"), None).await else { continue };
        for game in v.as_array().into_iter().flatten() {
            if let Some(id) = game.get("id").and_then(Value::as_str).and_then(|x| x.parse::<u64>().ok()) {
                found.insert(id, g::parse_game(id, game));
            }
        }
    }
    found
}

impl DiscordBackend {
    pub(super) fn games_impl(&self, ids: Vec<u64>) -> BoxFuture<Vec<g::Game>> {
        self.with_session(move |_, s| async move {
            let mut found = fetch_games(&s.rest, &ids).await;
            Ok(ids.iter().filter_map(|id| found.remove(id)).collect())
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
            let mut items = crate::ui::profile_wishlist::items(&v);
            for it in items.iter_mut().filter(|it| it.wishlist.is_empty()) {
                it.wishlist = id.clone().into();
            }
            Ok(items)
        })
    }
}

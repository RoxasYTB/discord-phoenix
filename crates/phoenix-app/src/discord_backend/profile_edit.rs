use reqwest::Method;
use serde_json::{Map, Value, json};

use super::{DiscordBackend, api_err, convert::sf};
use crate::{backend::BoxFuture, model::Id, ui::settings::profile_editor::ProfilePatch};

pub(crate) fn bodies(guild: bool, p: &ProfilePatch) -> (Map<String, Value>, Map<String, Value>) {
    let mut acct = Map::new();
    let mut prof = Map::new();
    if let Some(n) = &p.display_name {
        let v = if n.trim().is_empty() { Value::Null } else { json!(n) };
        acct.insert(if guild { "nick" } else { "global_name" }.into(), v);
    }
    if let Some(a) = &p.avatar {
        acct.insert("avatar".into(), json!(a));
    }
    let tgt = if guild { &mut acct } else { &mut prof };
    if let Some(v) = &p.pronouns {
        tgt.insert("pronouns".into(), json!(v));
    }
    if let Some(v) = &p.bio {
        tgt.insert("bio".into(), json!(v));
    }
    if let Some(b) = &p.banner {
        tgt.insert("banner".into(), json!(b));
    }
    if let Some((a, b)) = p.theme_colors {
        tgt.insert("theme_colors".into(), json!([a, b]));
    }
    (acct, prof)
}

impl DiscordBackend {
    pub(super) fn profile_update_impl(&self, guild: Option<Id>, patch: ProfilePatch) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let (acct, prof) = bodies(guild.is_some(), &patch);
            let acct_path = match guild {
                Some(g) => format!("/guilds/{}/members/@me", sf(g)),
                None => "/users/@me".into(),
            };
            if !acct.is_empty() {
                s.rest.request_json(Method::PATCH, &acct_path, Some(Value::Object(acct))).await.map_err(api_err)?;
            }
            if !prof.is_empty() {
                s.rest.request_json(Method::PATCH, "/users/@me/profile", Some(Value::Object(prof))).await.map_err(api_err)?;
            }
            Ok(())
        })
    }
}

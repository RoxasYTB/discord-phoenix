use phoenix_api::{
    gateway::{GatewayCommand, GuildSubscription, UpdatePresence},
    rest::profile::CustomStatus,
};
use serde_json::json;

use super::{DiscordBackend, api_err, convert};
use crate::{
    backend::{AppEvent, BoxFuture},
    model::{Id, Presence},
    presence::{Profile, profile_from_api, status_str},
};
use convert::{id, sf};

impl DiscordBackend {
    pub(super) fn people_ranges(&self, guild: Id, channel: Id, ranges: Vec<[u32; 2]>) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let mut sub = GuildSubscription::focused(None);
            sub.channels.insert(sf(channel), ranges);
            let _ = s.gateway.subscribe_guild(sf(guild), sub);
            Ok(())
        })
    }

    pub(super) fn people_profile(&self, user: Id, guild: Option<Id>) -> BoxFuture<Profile> {
        self.with_session(move |inner, s| async move {
            let (p, note) = tokio::join!(s.rest.get_user_profile(sf(user), guild.map(sf)), s.rest.get_user_note(sf(user)));
            let p = p.map_err(api_err)?;
            inner.emit(AppEvent::Users(vec![convert::user(&p.user)]));
            Ok(profile_from_api(&p, note.unwrap_or_default()))
        })
    }

    pub(super) fn people_note(&self, user: Id, note: String) -> BoxFuture<()> {
        self.with_session(move |_, s| async move { s.rest.set_user_note(sf(user), &note).await.map_err(api_err) })
    }

    pub(super) fn people_status(&self, status: Presence, custom: Option<String>) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let custom = custom.filter(|t| !t.trim().is_empty());
            let activities = custom
                .as_ref()
                .map(|t| vec![json!({"type": 4, "name": "Custom Status", "state": t})])
                .unwrap_or_default();
            let _ = s.gateway.send(GatewayCommand::UpdatePresence(UpdatePresence {
                since: 0,
                activities,
                status: status_str(status).into(),
                afk: false,
            }));
            let cs = custom.map(|t| CustomStatus { text: Some(t), ..Default::default() });
            s.rest.update_status_settings(Some(status_str(status)), Some(cs.as_ref())).await.map_err(api_err)
        })
    }

    pub(super) fn people_open_dm(&self, user: Id) -> BoxFuture<Id> {
        self.with_session(move |inner, s| async move {
            let c = s.rest.create_dm(sf(user)).await.map_err(api_err)?;
            let dm = {
                let a = s.adapter.lock();
                convert::dm_channel(&c, &a.prefs.ctx())
            };
            if let Some(dm) = dm {
                inner.emit(AppEvent::DmUpsert(dm));
            }
            Ok(id(c.id))
        })
    }
}

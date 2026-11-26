use phoenix_api::{
    gateway::{GatewayCommand, GuildSubscription, UpdatePresence},
    rest::{profile::CustomStatus, settings::CustomStatusProto},
};
use serde_json::{Value, json};

use super::{DiscordBackend, api_err, convert};
use crate::{
    backend::{AppEvent, BoxFuture},
    model::{Id, Presence},
    presence::{Profile, profile_from_api, status_str},
    ui::modals::custom_status::StatusDraft,
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

    pub(super) fn people_profile(&self, user: Id, guild: Option<Id>, with_note: bool) -> BoxFuture<Profile> {
        self.with_session(move |inner, s| async move {
            let note = async {
                if with_note { s.rest.get_user_note(sf(user)).await } else { Ok(String::new()) }
            };
            let (p, note) = tokio::join!(s.rest.get_user_profile(sf(user), guild.map(sf)), note);
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

    pub(super) fn people_custom_status(&self, status: Presence, draft: Option<StatusDraft>) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let _ = s.gateway.send(GatewayCommand::UpdatePresence(UpdatePresence {
                since: 0,
                activities: draft.as_ref().map(custom_activity).into_iter().collect(),
                status: status_str(status).into(),
                afk: false,
            }));
            let proto = draft.as_ref().map(custom_status_proto);
            s.rest.save_custom_status(proto.as_ref()).await.map(|_| ()).map_err(api_err)
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

pub(crate) fn custom_status_proto(d: &StatusDraft) -> CustomStatusProto {
    CustomStatusProto {
        text: d.text.clone(),
        emoji_id: d.emoji.as_ref().and_then(|e| e.id).map(|i| i.0).unwrap_or(0),
        emoji_name: d.emoji.as_ref().map(|e| e.name.to_string()).unwrap_or_default(),
        expires_at_ms: d.expires_at_ms,
        created_at_ms: d.created_at_ms,
    }
}

pub(crate) fn custom_activity(d: &StatusDraft) -> Value {
    let mut a = json!({"name": "Custom Status", "type": 4});
    if !d.text.is_empty() {
        a["state"] = json!(d.text);
    }
    if d.expires_at_ms > 0 {
        a["timestamps"] = json!({"end": d.expires_at_ms});
    }
    if let Some(e) = &d.emoji {
        a["emoji"] = match e.id {
            Some(id) => json!({"id": id.0.to_string(), "name": e.name, "animated": e.animated}),
            None => json!({"id": null, "name": e.name, "animated": false}),
        };
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::modals::custom_status::{ClearAfter, StatusEmoji, draft};

    #[test]
    fn custom_status_body_matches_the_client() {
        let now = 1_764_085_600_000;
        let e = StatusEmoji { id: Some(Id(42)), name: "dance".into(), animated: true };
        let d = draft("Lunch", Some(e), ClearAfter::Minutes30, now).unwrap();
        let p = custom_status_proto(&d);
        assert_eq!(
            p,
            CustomStatusProto { text: "Lunch".into(), emoji_id: 42, emoji_name: "dance".into(), expires_at_ms: now + 1_800_000, created_at_ms: now }
        );
        assert_eq!(
            custom_activity(&d),
            json!({"name": "Custom Status", "type": 4, "state": "Lunch", "timestamps": {"end": now + 1_800_000}, "emoji": {"id": "42", "name": "dance", "animated": true}})
        );
        let u = draft("", Some(StatusEmoji { id: None, name: "\u{1f354}".into(), animated: false }), ClearAfter::Never, now).unwrap();
        assert_eq!(custom_activity(&u), json!({"name": "Custom Status", "type": 4, "emoji": {"id": null, "name": "\u{1f354}", "animated": false}}));
        let p = custom_status_proto(&u);
        assert_eq!((p.emoji_id, p.expires_at_ms, p.text.as_str()), (0, 0, ""));
    }
}

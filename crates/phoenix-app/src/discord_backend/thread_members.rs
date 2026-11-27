use std::{collections::HashMap, sync::Mutex, time::Duration};

use phoenix_api::gateway::GatewayEvent;
use serde_json::Value;
use tokio::sync::oneshot;

use super::{DiscordBackend, convert};
use crate::{
    backend::{AppEvent, BackendError, BoxFuture},
    model::{Id, Member},
};
use convert::{id, sf};

type Reply = (Vec<Member>, Vec<crate::model::User>);
static PENDING: Mutex<Option<HashMap<u64, oneshot::Sender<Reply>>>> = Mutex::new(None);

fn parse(data: &Value) -> Option<(u64, Reply)> {
    let thread: u64 = data.get("thread_id")?.as_str()?.parse().ok()?;
    let guild = data.get("guild_id").and_then(Value::as_str).and_then(|g| g.parse::<u64>().ok());
    let (mut members, mut users) = (Vec::new(), Vec::new());
    for r in data.get("members")?.as_array()? {
        let Some(u) = r.get("user_id").and_then(Value::as_str).and_then(|u| u.parse::<u64>().ok()) else { continue };
        let m: Option<phoenix_api::models::Member> = r.get("member").and_then(|m| serde_json::from_value(m.clone()).ok());
        if let Some(user) = m.as_ref().and_then(|m| m.user.as_ref()) {
            users.push(convert::user(user));
        }
        let status = r.pointer("/presence/status").and_then(Value::as_str).unwrap_or("offline");
        let uid = phoenix_api::models::Snowflake(u);
        members.push(Member {
            user: Id(u),
            roles: m.as_ref().map(|m| m.roles.iter().copied().map(id).collect()).unwrap_or_default(),
            presence: convert::presence(status),
            activity: None,
            nick: m.as_ref().and_then(|m| m.nick.clone()).filter(|n| !n.is_empty()).map(Into::into),
            avatar: guild.zip(m.as_ref()).and_then(|(g, m)| convert::member_avatar_url(phoenix_api::models::Snowflake(g), uid, m)),
            decoration: m.as_ref().and_then(convert::member_decoration_url),
            nameplate: m.as_ref().and_then(convert::member_nameplate),
            joined_at: m.as_ref().and_then(convert::joined_unix),
            rejoined: m.as_ref().is_some_and(|m| m.flags & 1 != 0),
            premium_since: m.as_ref().and_then(convert::premium_unix),
        });
    }
    Some((thread, (members, users)))
}

pub fn on_gateway(ev: &GatewayEvent) {
    let GatewayEvent::Unknown { name, data } = ev else { return };
    if name != "THREAD_MEMBER_LIST_UPDATE" {
        return;
    }
    let Some((t, reply)) = parse(data) else { return };
    if let Some(tx) = PENDING.lock().ok().and_then(|mut p| p.get_or_insert_default().remove(&t)) {
        let _ = tx.send(reply);
    }
}

impl DiscordBackend {
    pub(super) fn thread_members_impl(&self, thread: Id) -> BoxFuture<Vec<Member>> {
        self.with_session(move |inner, s| async move {
            let guild = s.cache.channel(sf(thread)).and_then(|c| c.guild_id).ok_or_else(|| BackendError("Unknown thread.".into()))?;
            let (tx, rx) = oneshot::channel();
            if let Ok(mut p) = PENDING.lock() {
                p.get_or_insert_default().insert(thread.0, tx);
            }
            s.subs.thread_list(&s.gateway, guild, sf(thread));
            let (members, users) = tokio::time::timeout(Duration::from_secs(10), rx)
                .await
                .map_err(|_| BackendError("Thread member list timed out.".into()))?
                .map_err(|_| BackendError("Thread member list dropped.".into()))?;
            if !users.is_empty() {
                inner.emit(AppEvent::Users(users));
            }
            Ok(members)
        })
    }
}

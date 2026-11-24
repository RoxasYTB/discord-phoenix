//! Navigation data for the real backend: relationships, guild folders,
//! threads, channel decorations and the REST calls behind the Friends page,
//! invites and the guild menu. Emits `AppEvent::Nav`.

use base64::Engine;
use phoenix_api::{
    Cache, RestClient,
    gateway::{GatewayEvent, Ready},
    models::{self as api, ChannelType, RelationshipType},
};
use reqwest::Method;
use serde_json::{Value, json};

use super::convert::{self, id, sf};
use crate::{
    backend::{AppEvent, BackendError},
    model::{self, Id, ImageRef},
    ui::home::data::{
        ChannelExtra, GuildFolder, InvitePreview, NavEvent, NavReply, NavRequest, RelAction, Relationship,
        RelationshipKind,
    },
};

fn rel_kind(k: RelationshipType) -> Option<RelationshipKind> {
    Some(match k {
        RelationshipType::Friend => RelationshipKind::Friend,
        RelationshipType::Blocked => RelationshipKind::Blocked,
        RelationshipType::IncomingRequest => RelationshipKind::Incoming,
        RelationshipType::OutgoingRequest => RelationshipKind::Outgoing,
        _ => return None,
    })
}

fn relationship(r: &api::Relationship) -> Option<Relationship> {
    let kind = match rel_kind(r.kind) {
        Some(k) => k,
        None if r.user_ignored => RelationshipKind::None,
        None => return None,
    };
    Some(Relationship {
        user: id(r.user.as_ref().map(|u| u.id).or(r.user_id).unwrap_or(r.id)),
        kind,
        nickname: r.nickname.clone().filter(|n| !n.is_empty()).map(Into::into),
        since: r.since.as_deref().and_then(crate::presence::iso_to_unix),
        ignored: r.user_ignored,
    })
}

// ------------------------------------------------------------ folders

fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*i)?;
        *i += 1;
        v |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// Minimal protobuf walk: (field, wire type, length-delimited payload, scalar).
pub(crate) fn fields(b: &[u8]) -> Vec<(u64, u8, &[u8], u64)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let Some(key) = varint(b, &mut i) else { break };
        let (num, wt) = (key >> 3, (key & 7) as u8);
        match wt {
            0 => {
                let Some(v) = varint(b, &mut i) else { break };
                out.push((num, wt, &b[0..0], v));
            }
            1 => {
                let Some(s) = b.get(i..i + 8) else { break };
                i += 8;
                out.push((num, wt, s, u64::from_le_bytes(s.try_into().unwrap_or_default())));
            }
            2 => {
                let Some(len) = varint(b, &mut i) else { break };
                let Some(s) = b.get(i..i + len as usize) else { break };
                i += len as usize;
                out.push((num, wt, s, 0));
            }
            5 => {
                let Some(s) = b.get(i..i + 4) else { break };
                i += 4;
                out.push((num, wt, s, u64::from(u32::from_le_bytes(s.try_into().unwrap_or_default()))));
            }
            _ => break,
        }
    }
    out
}

/// Value of a wrapper message (`Int64Value` / `UInt64Value`: field 1).
fn wrapped_u64(b: &[u8]) -> Option<u64> {
    fields(b).into_iter().find(|f| f.0 == 1).map(|f| f.3)
}

fn wrapped_str(b: &[u8]) -> Option<String> {
    fields(b).into_iter().find(|f| f.0 == 1 && f.1 == 2).map(|f| String::from_utf8_lossy(f.2).into_owned())
}

/// `PreloadedUserSettings.guild_folders` (field 14) -> folders (field 1).
pub fn folders_from_proto(b64: &str) -> Vec<GuildFolder> {
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64.trim()) else { return Vec::new() };
    let mut out = Vec::new();
    for (num, wt, payload, _) in fields(&bytes) {
        if num != 14 || wt != 2 {
            continue;
        }
        for (n, w, p, _) in fields(payload) {
            if n != 1 || w != 2 {
                continue;
            }
            let mut f = GuildFolder::default();
            for (fnum, fw, fp, fs) in fields(p) {
                match fnum {
                    1 if fw == 2 => {
                        f.guilds.extend(fp.chunks_exact(8).map(|c| Id(u64::from_le_bytes(c.try_into().unwrap()))))
                    }
                    1 if fw == 1 => f.guilds.push(Id(fs)),
                    2 => f.id = wrapped_u64(fp).filter(|v| *v != 0),
                    3 => f.name = wrapped_str(fp).filter(|s| !s.is_empty()).map(Into::into),
                    4 => f.color = wrapped_u64(fp).map(|c| c as u32),
                    _ => {}
                }
            }
            if !f.guilds.is_empty() {
                out.push(f);
            }
        }
    }
    out
}

fn snowflake(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

pub fn folders_from_json(v: Option<&Value>) -> Vec<GuildFolder> {
    let Some(Value::Array(fs)) = v.and_then(|v| v.get("guild_folders")) else { return Vec::new() };
    fs.iter()
        .map(|f| GuildFolder {
            id: f.get("id").and_then(snowflake).filter(|v| *v != 0),
            name: f.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| s.to_owned().into()),
            color: f.get("color").and_then(Value::as_u64).map(|c| c as u32),
            guilds: f.get("guild_ids").and_then(Value::as_array).into_iter().flatten().filter_map(snowflake).map(Id).collect(),
        })
        .filter(|f| !f.guilds.is_empty())
        .collect()
}

// ------------------------------------------------------- channel bits

/// Thread member `muted` (+ `mute_config.end_time` not yet passed).
fn thread_member_muted(c: &api::Channel) -> bool {
    let Some(m) = c.member.as_ref() else { return false };
    if !m.get("muted").and_then(Value::as_bool).unwrap_or(false) {
        return false;
    }
    let end = m.get("mute_config").and_then(|c| c.get("end_time")).and_then(Value::as_str);
    match end.and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok()) {
        Some(t) => t.timestamp_millis() > chrono::Utc::now().timestamp_millis(),
        None => true,
    }
}

fn thread(c: &api::Channel) -> Option<model::Channel> {
    if !c.kind.is_thread() || c.thread_metadata.as_ref().is_some_and(|m| m.archived) {
        return None;
    }
    Some(model::Channel {
        id: id(c.id),
        guild: c.guild_id.map(id),
        parent: c.parent_id.map(id),
        name: c.display_name().into(),
        topic: None,
        kind: model::ChannelKind::Text,
        position: 0,
        unread: false,
        muted: thread_member_muted(c),
        mentions: 0,
        voice_users: Vec::new(),
    })
}

fn extra(cache: &Cache, c: &api::Channel) -> ChannelExtra {
    // Vanilla client rule (channel-icon picker → "is private channel" helper):
    // a guild channel is locked when @everyone, after its own overwrite, lacks
    // CONNECT (voice/stage), or when the @everyone overwrite denies VIEW_CHANNEL.
    let everyone_denied = |ch: &api::Channel| {
        ch.guild_id.is_some_and(|g| {
            let ow = ch.permission_overwrites.iter().find(|o| o.id == g);
            if ch.kind.is_voice() {
                let mut p = cache
                    .guild(g)
                    .and_then(|gd| gd.roles.iter().find(|r| r.id == g).map(|r| r.permissions))
                    .unwrap_or_default();
                if let Some(o) = ow {
                    p = (p - o.deny) | o.allow;
                }
                if !p.contains(api::Permissions::CONNECT) {
                    return true;
                }
            }
            ow.is_some_and(|o| o.deny.contains(api::Permissions::VIEW_CHANNEL))
        })
    };
    let group = c.kind == ChannelType::GroupDm;
    // Read-only permission check for the composer (the cache's overwrite computation).
    let perms = cache.permissions_in(c.id);
    let locked_thread = c.kind.is_thread() && c.thread_metadata.as_ref().is_some_and(|m| m.locked);
    let send_bit = if c.kind.is_thread() { api::Permissions::SEND_MESSAGES_IN_THREADS } else { api::Permissions::SEND_MESSAGES };
    ChannelExtra {
        slowmode: c.rate_limit_per_user.unwrap_or(0),
        bypass_slowmode: perms.intersects(api::Permissions::MANAGE_MESSAGES | api::Permissions::MANAGE_CHANNELS),
        // Empty = roles not cached yet: don't lock the composer on missing data.
        // A locked thread only takes messages from MANAGE_THREADS holders.
        send_blocked: c.guild_id.is_some()
            && !perms.is_empty()
            && (!perms.contains(send_bit) || (locked_thread && !perms.contains(api::Permissions::MANAGE_THREADS))),
        thread_archived: c.thread_metadata.as_ref().is_some_and(|m| m.archived),
        thread_locked: locked_thread,
        can_manage_threads: perms.contains(api::Permissions::MANAGE_THREADS),
        nsfw: c.nsfw,
        locked: everyone_denied(c),
        group_icon: c
            .icon
            .as_ref()
            .filter(|_| group)
            .map(|h| ImageRef::Url(format!("{}/channel-icons/{}/{h}.png?size=64", api::CDN_BASE, c.id).into())),
        recipients: if c.kind.is_private() { c.recipient_user_ids().len() } else { 0 },
        group_name: group.then(|| group_name(cache, c)).flatten(),
        members: if group { c.recipient_user_ids().into_iter().map(id).collect() } else { Vec::new() },
        owner: c.owner_id.filter(|_| group).map(id),
    }
}

fn guild_extras(cache: &Cache, g: api::Snowflake) -> Vec<(Id, ChannelExtra)> {
    cache.guild_channels(g).iter().map(|c| (id(c.id), extra(cache, c))).collect()
}

/// Group DM name when READY only gave recipient ids (else "Unnamed").
fn group_name(cache: &Cache, c: &api::Channel) -> Option<gpui::SharedString> {
    if c.name.as_deref().is_some_and(|n| !n.is_empty()) || !c.recipients.is_empty() {
        return None;
    }
    let names: Vec<String> =
        c.recipient_ids.iter().filter_map(|u| cache.user(*u)).map(|u| u.display_name().to_owned()).collect();
    (!names.is_empty()).then(|| names.join(", ").into())
}

pub fn snapshot(cache: &Cache, ready: &Ready) -> Vec<AppEvent> {
    let relationships = ready.relationships.iter().filter_map(relationship).collect();
    let mut folders = ready.user_settings_proto.as_deref().map(folders_from_proto).unwrap_or_default();
    if folders.is_empty() {
        folders = folders_from_json(ready.user_settings.as_ref());
    }
    let mut threads = Vec::new();
    let mut extras = Vec::new();
    // Thread mutes also live in `user_guild_settings` channel_overrides.
    let mutes = convert::parse_mutes(ready.user_guild_settings.as_ref());
    for g in cache.guilds() {
        for c in cache.guild_channels(g.id) {
            if let Some(mut t) = thread(&c) {
                t.muted |= mutes.channels.contains(&c.id.0);
                threads.push(t);
            }
        }
        extras.extend(guild_extras(cache, g.id));
    }
    let banners = cache.guilds().iter().map(|g| (id(g.id), g.properties.banner.clone())).collect();
    extras.extend(cache.private_channels().iter().map(|c| (id(c.id), extra(cache, c))));
    let message_requests = cache.private_channels().iter().filter(|c| c.is_message_request).map(|c| id(c.id)).collect();
    vec![
        AppEvent::Nav(NavEvent::Snapshot { relationships, folders, threads, extras, message_requests }),
        AppEvent::Nav(NavEvent::Banners(banners)),
    ]
}

/// Live gateway events that matter to navigation.
pub fn on_gateway(cache: &Cache, ev: &GatewayEvent) -> Vec<AppEvent> {
    let mut out = Vec::new();
    match ev {
        GatewayEvent::RelationshipAdd(r) => {
            if let Some(u) = &r.user {
                out.push(AppEvent::Users(vec![convert::user(u)]));
            }
            if let Some(r) = relationship(r) {
                out.push(AppEvent::Nav(NavEvent::RelationshipUpsert(r)));
            }
        }
        GatewayEvent::RelationshipRemove(r) => {
            out.push(AppEvent::Nav(NavEvent::RelationshipRemove(id(r.user_id.unwrap_or(r.id)))));
        }
        GatewayEvent::ThreadCreate(c) | GatewayEvent::ThreadUpdate(c) => {
            out.push(AppEvent::Nav(NavEvent::Extras(vec![(id(c.id), extra(cache, c))])));
            out.push(AppEvent::Nav(match thread(c) {
                Some(t) => NavEvent::ThreadUpsert(t),
                None => NavEvent::ThreadDelete(id(c.id)),
            }));
        }
        GatewayEvent::ThreadDelete(c) => out.push(AppEvent::Nav(NavEvent::ThreadDelete(id(c.id)))),
        // Sent when a channel is opened: active threads plus my thread members. The
        // sidebar lists only the threads I joined (those with a member entry).
        // Settings saved by another client (type 1 = PreloadedUserSettings). Partial
        // protos carry only changed fields: no folders field means no folder change.
        GatewayEvent::Unknown { name, data } if name == "USER_SETTINGS_PROTO_UPDATE" => {
            let s = data.get("settings");
            if s.and_then(|s| s.get("type")).and_then(Value::as_u64) == Some(1)
                && let Some(p) = s.and_then(|s| s.get("proto")).and_then(Value::as_str)
            {
                out.push(AppEvent::SettingsChanged);
                let f = folders_from_proto(p);
                if !f.is_empty() {
                    out.push(AppEvent::Nav(NavEvent::Folders(f)));
                }
            }
        }
        GatewayEvent::Unknown { name, data } if name == "THREAD_LIST_SYNC" => {
            let joined: Vec<u64> =
                data.get("members").and_then(Value::as_array).into_iter().flatten().filter_map(|m| m.get("id").and_then(snowflake)).collect();
            for t in data.get("threads").and_then(Value::as_array).into_iter().flatten() {
                if let Ok(c) = serde_json::from_value::<api::Channel>(t.clone())
                    && joined.contains(&c.id.0)
                    && let Some(t) = thread(&c)
                {
                    out.push(AppEvent::Nav(NavEvent::ThreadUpsert(t)));
                }
            }
        }
        GatewayEvent::ChannelCreate(c) | GatewayEvent::ChannelUpdate(c) => {
            out.push(AppEvent::Nav(NavEvent::Extras(vec![(id(c.id), extra(cache, c))])));
        }
        GatewayEvent::GuildUpdate(g) => {
            out.push(AppEvent::Nav(NavEvent::Banners(vec![(id(g.id), g.properties.banner.clone())])));
        }
        GatewayEvent::GuildCreate(g) => {
            out.push(AppEvent::Nav(NavEvent::Banners(vec![(id(g.id), g.properties.banner.clone())])));
            out.push(AppEvent::Nav(NavEvent::Extras(guild_extras(cache, g.id))));
            for c in cache.guild_channels(g.id) {
                if let Some(t) = thread(&c) {
                    out.push(AppEvent::Nav(NavEvent::ThreadUpsert(t)));
                }
            }
        }
        GatewayEvent::PresenceUpdate(p) if p.guild_id.is_none() => {
            out.push(AppEvent::Nav(NavEvent::Activity { user: id(p.user.id), activity: convert::activity(&p.activities) }));
        }
        GatewayEvent::ReadySupplemental(r) => {
            let friends = r.merged_presences.as_ref().and_then(|m| m.get("friends")).and_then(Value::as_array);
            for f in friends.into_iter().flatten() {
                let Some(u) = f.get("user_id").and_then(snowflake) else { continue };
                let acts = f.get("activities").and_then(Value::as_array).cloned().unwrap_or_default();
                if let Some(a) = convert::activity(&acts) {
                    out.push(AppEvent::Nav(NavEvent::Activity { user: Id(u), activity: Some(a) }));
                }
                if let Some(s) = f.get("status").and_then(Value::as_str) {
                    out.push(AppEvent::PresenceUpdate { user: Id(u), presence: convert::presence(s) });
                }
            }
        }
        _ => {}
    }
    out
}

// --------------------------------------------------------------- REST

fn api_err(e: phoenix_api::Error) -> BackendError {
    let s = e.to_string();
    BackendError(if s.contains("captcha") || s.contains("Captcha") {
        "Discord asked for a captcha. Join from the official client once.".into()
    } else {
        s
    })
}

fn invite_preview(v: &Value) -> InvitePreview {
    let s = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_owned);
    let g = v.get("guild");
    let gid = g.and_then(|g| g.get("id")).and_then(snowflake);
    let icon = match (gid, s(g.and_then(|g| g.get("icon")))) {
        (Some(gid), Some(h)) => Some(ImageRef::Url(format!("{}/icons/{gid}/{h}.png?size=128", api::CDN_BASE).into())),
        _ => None,
    };
    let inviter = v.get("inviter").map(|u| s(u.get("global_name")).or_else(|| s(u.get("username"))).unwrap_or_default());
    InvitePreview {
        code: s(v.get("code")).unwrap_or_default().into(),
        guild: gid.map(Id),
        guild_name: s(g.and_then(|g| g.get("name")))
            .or_else(|| s(v.get("channel").and_then(|c| c.get("name"))))
            .unwrap_or_else(|| "Group DM".into())
            .into(),
        guild_icon: icon,
        channel_name: s(v.get("channel").and_then(|c| c.get("name"))).map(Into::into),
        inviter: inviter.filter(|i| !i.is_empty()).map(Into::into),
        online: v.get("approximate_presence_count").and_then(Value::as_u64).unwrap_or(0) as u32,
        members: v.get("approximate_member_count").and_then(Value::as_u64).unwrap_or(0) as u32,
    }
}

fn enc(code: &str) -> String {
    code.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

pub async fn request(rest: RestClient, req: NavRequest) -> Result<NavReply, BackendError> {
    let call = |m: Method, p: String, b: Option<Value>| {
        let rest = rest.clone();
        async move { rest.request_json(m, &p, b).await.map_err(api_err) }
    };
    Ok(match req {
        NavRequest::FetchInvite(code) => {
            let v = call(Method::GET, format!("/invites/{}?with_counts=true&with_expiration=true", enc(&code)), None).await?;
            NavReply::Invite(invite_preview(&v))
        }
        NavRequest::AcceptInvite(code) => {
            call(Method::POST, format!("/invites/{}", enc(&code)), Some(json!({}))).await?;
            NavReply::Done
        }
        NavRequest::CreateInvite(ch) => {
            let v = call(
                Method::POST,
                format!("/channels/{}/invites", sf(ch)),
                Some(json!({ "max_age": 604800, "max_uses": 0, "temporary": false })),
            )
            .await?;
            let code = v.get("code").and_then(Value::as_str).unwrap_or_default();
            NavReply::Text(format!("https://discord.gg/{code}").into())
        }
        NavRequest::LeaveGuild(g) => {
            call(Method::DELETE, format!("/users/@me/guilds/{}", sf(g)), Some(json!({ "lurking": false }))).await?;
            NavReply::Done
        }
        NavRequest::Relationship { user, action } => {
            let path = format!("/users/@me/relationships/{}", sf(user));
            match action {
                RelAction::Accept => call(Method::PUT, path, Some(json!({}))).await?,
                RelAction::Block => call(Method::PUT, path, Some(json!({ "type": 2 }))).await?,
                RelAction::Remove => call(Method::DELETE, path, None).await?,
                RelAction::AddByName(name) => {
                    let name = name.trim().trim_start_matches('@').to_owned();
                    call(
                        Method::POST,
                        "/users/@me/relationships".into(),
                        Some(json!({ "username": name, "discriminator": Value::Null })),
                    )
                    .await?
                }
            };
            NavReply::Done
        }
        NavRequest::OpenDm(user) => {
            let c = rest.create_dm(sf(user)).await.map_err(api_err)?;
            NavReply::Channel(model::Channel {
                id: id(c.id),
                guild: None,
                parent: None,
                name: c.display_name().into(),
                topic: None,
                kind: model::ChannelKind::Text,
                position: 0,
                unread: false,
                muted: false,
                mentions: 0,
                voice_users: c.recipient_user_ids().first().copied().map(id).into_iter().collect(),
            })
        }
        NavRequest::CloseDm(ch) => {
            call(Method::DELETE, format!("/channels/{}?silent=false", sf(ch)), None).await?;
            NavReply::Done
        }
        NavRequest::AckChannel(ch) => {
            let v = call(Method::GET, format!("/channels/{}", sf(ch)), None).await?;
            if let Some(last) = v.get("last_message_id").and_then(snowflake) {
                rest.ack_message(sf(ch), sf(Id(last))).await.map_err(api_err)?;
            }
            NavReply::Done
        }
        NavRequest::AckGuild(g) => {
            call(Method::POST, format!("/guilds/{}/ack", sf(g)), Some(json!({}))).await?;
            NavReply::Done
        }
        NavRequest::Mute { guild, channel, muted, secs } => {
            let end = secs.map(|s| (chrono::Utc::now() + chrono::Duration::seconds(s as i64)).to_rfc3339());
            let cfg = json!({ "muted": muted, "mute_config": muted.then(|| json!({
                "selected_time_window": secs.map(|s| s as i64).unwrap_or(-1),
                "end_time": end,
            })) });
            call(Method::PATCH, settings_path(guild), Some(settings_body(channel, cfg))).await?;
            NavReply::Done
        }
        NavRequest::NotifyLevel { guild, channel, level } => {
            let body = settings_body(channel, json!({ "message_notifications": level }));
            call(Method::PATCH, settings_path(guild), Some(body)).await?;
            NavReply::Done
        }
    })
}

fn settings_path(guild: Option<Id>) -> String {
    match guild {
        Some(g) => format!("/users/@me/guilds/{}/settings", sf(g)),
        None => "/users/@me/guilds/@me/settings".into(),
    }
}

fn settings_body(channel: Option<Id>, fields: Value) -> Value {
    match channel {
        Some(ch) => json!({ "channel_overrides": { sf(ch).to_string(): fields } }),
        None => fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_list_sync_lists_only_joined_threads() {
        let t = |i: u64| json!({"id": i.to_string(), "type": 11, "guild_id": "1", "parent_id": "2", "name": format!("t{i}"),
            "thread_metadata": {"archived": false, "locked": false, "auto_archive_duration": 1440, "archive_timestamp": "2026-11-24T00:00:00+00:00"}});
        let data = json!({"guild_id": "1", "threads": [t(10), t(11)], "members": [{"id": "11", "user_id": "5", "flags": 1, "join_timestamp": "2026-11-24T00:00:00+00:00"}]});
        let ev = GatewayEvent::Unknown { name: "THREAD_LIST_SYNC".into(), data };
        let out = on_gateway(&Cache::default(), &ev);
        let ids: Vec<Id> = out.iter().filter_map(|e| match e { AppEvent::Nav(NavEvent::ThreadUpsert(c)) => Some(c.id), _ => None }).collect();
        assert_eq!(ids, vec![Id(11)]);
    }

    #[test]
    fn folder_proto() {
        // guild_folders { folders { guild_ids: [1,2] id { value: 9 } name { value: "ab" } color { value: 255 } } }
        let mut folder = vec![0x0a, 16];
        folder.extend(1u64.to_le_bytes());
        folder.extend(2u64.to_le_bytes());
        folder.extend([0x12, 2, 0x08, 9]);
        folder.extend([0x1a, 4, 0x0a, 2, b'a', b'b']);
        folder.extend([0x22, 3, 0x08, 0xff, 0x01]);
        let mut gf = vec![0x0a, folder.len() as u8];
        gf.extend(folder);
        let mut top = vec![0x72, gf.len() as u8];
        top.extend(gf);
        let b64 = base64::engine::general_purpose::STANDARD.encode(top);
        let f = folders_from_proto(&b64);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].guilds, vec![Id(1), Id(2)]);
        assert_eq!(f[0].id, Some(9));
        assert_eq!(f[0].name.as_deref(), Some("ab"));
        assert_eq!(f[0].color, Some(255));
        // Live update from another client: settings reloaded, folders replaced; other setting types ignored.
        let upd = |t: u64| GatewayEvent::Unknown {
            name: "USER_SETTINGS_PROTO_UPDATE".into(),
            data: json!({"settings": {"type": t, "proto": b64}, "partial": true}),
        };
        let out = on_gateway(&Cache::default(), &upd(1));
        assert!(matches!(out.as_slice(), [AppEvent::SettingsChanged, AppEvent::Nav(NavEvent::Folders(f))] if f.len() == 1));
        assert!(on_gateway(&Cache::default(), &upd(2)).is_empty());
    }

    #[test]
    fn invite_json() {
        let v = json!({"code":"x","guild":{"id":"5","name":"G","icon":"h"},"approximate_member_count":10,"approximate_presence_count":3,"inviter":{"username":"u"}});
        let p = invite_preview(&v);
        assert_eq!(p.guild, Some(Id(5)));
        assert_eq!(p.members, 10);
        assert_eq!(p.inviter.as_deref(), Some("u"));
    }
}

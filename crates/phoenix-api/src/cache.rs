use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use serde_json::Value;

use crate::gateway::GatewayEvent;
use crate::models::*;

pub const DEFAULT_MESSAGE_CAP: usize = 200;

#[derive(Default)]
struct State {
    me: Option<Arc<User>>,
    users: HashMap<UserId, Arc<User>>,
    guilds: HashMap<GuildId, Arc<Guild>>,
    guild_order: Vec<GuildId>,
    channels: HashMap<ChannelId, Arc<Channel>>,
    guild_channels: HashMap<GuildId, Vec<ChannelId>>,
    private_channels: Vec<ChannelId>,
    members: HashMap<(GuildId, UserId), Arc<Member>>,
    messages: HashMap<ChannelId, VecDeque<Arc<Message>>>,
    voice_states: HashMap<(Option<GuildId>, UserId), Arc<VoiceState>>,
    relationships: HashMap<UserId, Arc<Relationship>>,
    read_state: Option<Value>,
}

struct Inner {
    state: RwLock<State>,
    version: AtomicU64,
    message_cap: usize,
}

#[derive(Clone)]
pub struct Cache {
    inner: Arc<Inner>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new(DEFAULT_MESSAGE_CAP)
    }
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache").field("version", &self.version()).finish_non_exhaustive()
    }
}

impl Cache {
    pub fn new(message_cap: usize) -> Self {
        Self {
            inner: Arc::new(Inner { state: RwLock::new(State::default()), version: AtomicU64::new(0), message_cap: message_cap.max(1) }),
        }
    }

    pub fn version(&self) -> u64 {
        self.inner.version.load(Ordering::Acquire)
    }

    fn bump(&self) {
        self.inner.version.fetch_add(1, Ordering::AcqRel);
    }

    pub fn current_user(&self) -> Option<Arc<User>> {
        self.inner.state.read().me.clone()
    }

    pub fn user(&self, id: UserId) -> Option<Arc<User>> {
        self.inner.state.read().users.get(&id).cloned()
    }

    pub fn guild(&self, id: GuildId) -> Option<Arc<Guild>> {
        self.inner.state.read().guilds.get(&id).cloned()
    }

    pub fn guilds(&self) -> Vec<Arc<Guild>> {
        let s = self.inner.state.read();
        s.guild_order.iter().filter_map(|id| s.guilds.get(id).cloned()).collect()
    }

    pub fn channel(&self, id: ChannelId) -> Option<Arc<Channel>> {
        self.inner.state.read().channels.get(&id).cloned()
    }

    pub fn guild_channels(&self, guild: GuildId) -> Vec<Arc<Channel>> {
        let s = self.inner.state.read();
        let mut v: Vec<_> = s
            .guild_channels
            .get(&guild)
            .into_iter()
            .flatten()
            .filter_map(|id| s.channels.get(id).cloned())
            .collect();
        v.sort_by_key(|c| (c.position.unwrap_or(0), c.id));
        v
    }

    pub fn private_channels(&self) -> Vec<Arc<Channel>> {
        let s = self.inner.state.read();
        let mut v: Vec<_> = s.private_channels.iter().filter_map(|id| s.channels.get(id).cloned()).collect();
        v.sort_by_key(|c| std::cmp::Reverse(c.last_message_id.unwrap_or(c.id)));
        v
    }

    pub fn messages(&self, channel: ChannelId) -> Vec<Arc<Message>> {
        self.inner.state.read().messages.get(&channel).map(|q| q.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn message(&self, channel: ChannelId, id: MessageId) -> Option<Arc<Message>> {
        let s = self.inner.state.read();
        s.messages.get(&channel)?.iter().find(|m| m.id == id).cloned()
    }

    pub fn member(&self, guild: GuildId, user: UserId) -> Option<Arc<Member>> {
        self.inner.state.read().members.get(&(guild, user)).cloned()
    }

    pub fn voice_states(&self, guild: Option<GuildId>) -> Vec<Arc<VoiceState>> {
        let s = self.inner.state.read();
        s.voice_states.iter().filter(|((g, _), _)| *g == guild).map(|(_, v)| v.clone()).collect()
    }

    pub fn relationships(&self) -> Vec<Arc<Relationship>> {
        self.inner.state.read().relationships.values().cloned().collect()
    }

    pub fn relationship(&self, user: UserId) -> Option<Arc<Relationship>> {
        self.inner.state.read().relationships.get(&user).cloned()
    }

    pub fn read_state(&self) -> Option<Value> {
        self.inner.state.read().read_state.clone()
    }

    pub fn guild_permissions(&self, id: GuildId) -> Permissions {
        let state = self.inner.state.read();
        let (Some(me), Some(guild)) = (&state.me, state.guilds.get(&id)) else {
            return Permissions::empty();
        };
        let member_roles = state.members.get(&(id, me.id)).map_or(&[][..], |member| member.roles.as_slice());
        compute_base_permissions(id, guild.owner_id(), me.id, member_roles, &guild.roles)
    }

    pub fn permissions_in(&self, channel: ChannelId) -> Permissions {
        let s = self.inner.state.read();
        let Some(ch) = s.channels.get(&channel) else { return Permissions::empty() };
        let Some(gid) = ch.guild_id else { return Permissions::all() };
        let (Some(me), Some(guild)) = (&s.me, s.guilds.get(&gid)) else { return Permissions::empty() };
        let member = s.members.get(&(gid, me.id));
        let roles = member.map(|m| m.roles.clone()).unwrap_or_default();
        let ow_src = if ch.kind.is_thread() { ch.parent_id.and_then(|p| s.channels.get(&p)).unwrap_or(ch) } else { ch };
        let perms = compute_channel_permissions(gid, guild.owner_id(), me.id, &roles, &guild.roles, &ow_src.permission_overwrites);
        if guild.owner_id() == Some(me.id) {
            return perms;
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let muted = member.is_some_and(|m| timed_out(m.communication_disabled_until.as_deref(), now));
        restrict_member(perms, muted, member.is_some_and(|m| m.pending == Some(true)))
    }

    pub fn member_permissions_in(&self, channel: ChannelId, user: UserId) -> Option<Permissions> {
        let s = self.inner.state.read();
        let ch = s.channels.get(&channel)?;
        let gid = ch.guild_id?;
        let guild = s.guilds.get(&gid)?;
        let roles = s.members.get(&(gid, user))?.roles.clone();
        let ow_src = if ch.kind.is_thread() { ch.parent_id.and_then(|p| s.channels.get(&p)).unwrap_or(ch) } else { ch };
        Some(compute_channel_permissions(gid, guild.owner_id(), user, &roles, &guild.roles, &ow_src.permission_overwrites))
    }

    pub fn roles_permissions_in(&self, channel: ChannelId, roles: &[RoleId]) -> Option<Permissions> {
        let s = self.inner.state.read();
        let ch = s.channels.get(&channel)?;
        let gid = ch.guild_id?;
        let guild = s.guilds.get(&gid)?;
        let ow_src = if ch.kind.is_thread() { ch.parent_id.and_then(|p| s.channels.get(&p)).unwrap_or(ch) } else { ch };
        Some(compute_channel_permissions(gid, None, Snowflake(0), roles, &guild.roles, &ow_src.permission_overwrites))
    }

    pub fn insert_messages(&self, channel: ChannelId, msgs: impl IntoIterator<Item = Message>) {
        let cap = self.inner.message_cap;
        let mut s = self.inner.state.write();
        let q = s.messages.entry(channel).or_default();
        for m in msgs {
            upsert_message(q, Arc::new(m));
        }
        while q.len() > cap {
            q.pop_front();
        }
        drop(s);
        self.bump();
    }

    pub fn apply(&self, ev: &GatewayEvent) -> bool {
        let changed = self.apply_inner(ev);
        if changed {
            self.bump();
        }
        changed
    }

    fn apply_inner(&self, ev: &GatewayEvent) -> bool {
        let cap = self.inner.message_cap;
        let mut s = self.inner.state.write();
        match ev {
            GatewayEvent::Ready(r) => {
                *s = State::default();
                let me = Arc::new(r.user.clone());
                s.users.insert(me.id, me.clone());
                s.me = Some(me.clone());
                for u in &r.users {
                    s.users.insert(u.id, Arc::new(u.clone()));
                }
                for (i, g) in r.guilds.iter().enumerate() {
                    insert_guild(&mut s, g.clone());
                    if let Some(ms) = r.merged_members.get(i) {
                        for m in ms {
                            let uid = m.user_id().unwrap_or(me.id);
                            s.members.insert((g.id, uid), Arc::new(m.clone()));
                        }
                    }
                }
                for c in &r.private_channels {
                    let mut c = c.clone();
                    if c.recipients.is_empty() {
                        c.recipients = c.recipient_ids.iter().filter_map(|id| s.users.get(id).map(|u| (**u).clone())).collect();
                    }
                    for u in &c.recipients {
                        s.users.entry(u.id).or_insert_with(|| Arc::new(u.clone()));
                    }
                    s.private_channels.push(c.id);
                    s.channels.insert(c.id, Arc::new(c));
                }
                for rel in &r.relationships {
                    if let Some(u) = &rel.user {
                        s.users.insert(u.id, Arc::new(u.clone()));
                    }
                    s.relationships.insert(rel.id, Arc::new(rel.clone()));
                }
                s.read_state = r.read_state.clone();
            }
            GatewayEvent::ReadySupplemental(r) => {
                for g in &r.guilds {
                    for v in &g.voice_states {
                        let mut v = v.clone();
                        v.guild_id.get_or_insert(g.id);
                        if v.channel_id.is_some() {
                            s.voice_states.insert((Some(g.id), v.user_id), Arc::new(v));
                        }
                    }
                }
            }
            GatewayEvent::UserUpdate(u) => {
                let u = Arc::new((**u).clone());
                if s.me.as_ref().is_some_and(|m| m.id == u.id) {
                    s.me = Some(u.clone());
                }
                s.users.insert(u.id, u);
            }
            GatewayEvent::PresenceUpdate(p) if p.user.username.is_some() => {
                let mut u = s.users.get(&p.user.id).map(|u| (**u).clone()).unwrap_or_else(|| User { id: p.user.id, ..Default::default() });
                if p.user.merge_into(&mut u) {
                    s.users.insert(u.id, Arc::new(u));
                }
            }
            GatewayEvent::GuildCreate(g) => insert_guild(&mut s, (**g).clone()),
            GatewayEvent::GuildUpdate(g) => {
                let Some(old) = s.guilds.get(&g.id).cloned() else {
                    insert_guild(&mut s, (**g).clone());
                    return true;
                };
                let mut new = (*old).clone();
                new.properties = g.properties.clone();
                if !g.roles.is_empty() {
                    new.roles = g.roles.clone();
                }
                if !g.emojis.is_empty() {
                    new.emojis = g.emojis.clone();
                }
                s.guilds.insert(g.id, Arc::new(new));
            }
            GatewayEvent::GuildDelete(d) => {
                if d.unavailable {
                    if let Some(g) = s.guilds.get(&d.id).cloned() {
                        let mut g = (*g).clone();
                        g.unavailable = true;
                        s.guilds.insert(d.id, Arc::new(g));
                    }
                } else {
                    remove_guild(&mut s, d.id);
                }
            }
            GatewayEvent::GuildMemberAdd(m) | GatewayEvent::GuildMemberUpdate(m) => {
                let (Some(gid), Some(uid)) = (m.guild_id, m.user_id()) else { return false };
                if let Some(u) = &m.user {
                    s.users.insert(u.id, Arc::new(u.clone()));
                }
                s.members.insert((gid, uid), Arc::new((**m).clone()));
            }
            GatewayEvent::GuildMemberRemove(r) => {
                s.members.remove(&(r.guild_id, r.user.id));
            }
            GatewayEvent::GuildMembersChunk(c) => {
                for m in &c.members {
                    if let Some(uid) = m.user_id() {
                        if let Some(u) = &m.user {
                            s.users.insert(u.id, Arc::new(u.clone()));
                        }
                        s.members.insert((c.guild_id, uid), Arc::new(m.clone()));
                    }
                }
            }
            GatewayEvent::GuildMemberListUpdate(l) => {
                use crate::gateway::{MemberListItem, MemberListOp};
                let mut changed = false;
                for op in &l.ops {
                    let items: Vec<&MemberListItem> = match op {
                        MemberListOp::Sync { items, .. } => items.iter().collect(),
                        MemberListOp::Insert { item, .. } | MemberListOp::Update { item, .. } => vec![item],
                        _ => vec![],
                    };
                    for it in items {
                        if let MemberListItem::Member(m) = it
                            && let Some(uid) = m.member.user_id()
                        {
                            if let Some(u) = &m.member.user {
                                s.users.insert(u.id, Arc::new(u.clone()));
                            }
                            s.members.insert((l.guild_id, uid), Arc::new(m.member.clone()));
                            changed = true;
                        }
                    }
                }
                return changed;
            }
            GatewayEvent::GuildRoleCreate(r) | GatewayEvent::GuildRoleUpdate(r) => {
                return modify_guild(&mut s, r.guild_id, |g| {
                    match g.roles.iter_mut().find(|x| x.id == r.role.id) {
                        Some(x) => *x = r.role.clone(),
                        None => g.roles.push(r.role.clone()),
                    }
                });
            }
            GatewayEvent::GuildRoleDelete(r) => {
                return modify_guild(&mut s, r.guild_id, |g| g.roles.retain(|x| x.id != r.role_id));
            }
            GatewayEvent::GuildEmojisUpdate(e) => {
                return modify_guild(&mut s, e.guild_id, |g| g.emojis = e.emojis.clone());
            }
            GatewayEvent::Unknown { name, data } if name == "GUILD_STICKERS_UPDATE" => {
                let Some(id) = data.get("guild_id").and_then(Value::as_str).and_then(|x| x.parse().ok()).map(Snowflake) else { return false };
                let Some(list) = data.get("stickers").and_then(Value::as_array) else { return false };
                return modify_guild(&mut s, id, |g| g.stickers = list.clone());
            }
            GatewayEvent::ChannelCreate(c) | GatewayEvent::ChannelUpdate(c) | GatewayEvent::ThreadCreate(c) => insert_channel(&mut s, (**c).clone()),
            GatewayEvent::ThreadUpdate(c) => {
                let mut c = (**c).clone();
                if c.member.is_none() {
                    c.member = s.channels.get(&c.id).and_then(|old| old.member.clone());
                }
                insert_channel(&mut s, c);
            }
            GatewayEvent::Unknown { name, data } if name == "CHANNEL_RECIPIENT_ADD" || name == "CHANNEL_RECIPIENT_REMOVE" => {
                let Some(id) = data.get("channel_id").and_then(Value::as_str).and_then(|x| x.parse().ok()).map(Snowflake) else { return false };
                let Some(u) = data.get("user").and_then(|u| serde_json::from_value::<User>(u.clone()).ok()) else { return false };
                let add = name == "CHANNEL_RECIPIENT_ADD";
                if add {
                    s.users.entry(u.id).or_insert_with(|| Arc::new(u.clone()));
                }
                let Some(ch) = s.channels.get_mut(&id) else { return false };
                let ch = Arc::make_mut(ch);
                ch.recipients.retain(|r| r.id != u.id);
                ch.recipient_ids.retain(|r| *r != u.id);
                if add {
                    ch.recipient_ids.push(u.id);
                    ch.recipients.push(u);
                }
            }
            GatewayEvent::Unknown { name, data } if name == "THREAD_MEMBER_UPDATE" => {
                let Some(id) = data.get("id").and_then(Value::as_str).and_then(|x| x.parse().ok()).map(Snowflake) else { return false };
                let Some(ch) = s.channels.get_mut(&id) else { return false };
                Arc::make_mut(ch).member = Some(data.clone());
            }
            GatewayEvent::Unknown { name, data } if name == "THREAD_MEMBERS_UPDATE" => {
                let Some(me) = s.me.as_ref().map(|u| u.id.0.to_string()) else { return false };
                let Some(id) = data.get("id").and_then(Value::as_str).and_then(|x| x.parse().ok()).map(Snowflake) else { return false };
                let added = data
                    .get("added_members")
                    .and_then(Value::as_array)
                    .and_then(|v| v.iter().find(|m| m.get("user_id").and_then(Value::as_str) == Some(me.as_str())))
                    .cloned();
                let removed = data
                    .get("removed_member_ids")
                    .and_then(Value::as_array)
                    .is_some_and(|v| v.iter().any(|x| x.as_str() == Some(me.as_str())));
                let count = data.get("member_count").and_then(Value::as_u64);
                let preview: Option<Vec<Snowflake>> = data
                    .get("member_ids_preview")
                    .and_then(Value::as_array)
                    .map(|v| v.iter().filter_map(Value::as_str).filter_map(|x| x.parse().ok()).map(Snowflake).collect());
                let Some(ch) = s.channels.get_mut(&id) else { return false };
                let ch = Arc::make_mut(ch);
                if let Some(m) = added {
                    ch.member = Some(m);
                } else if removed {
                    ch.member = None;
                }
                if let Some(n) = count {
                    ch.member_count = Some(n as u32);
                }
                if let Some(p) = preview {
                    ch.member_ids_preview = p;
                }
            }
            GatewayEvent::ChannelDelete(c) | GatewayEvent::ThreadDelete(c) => {
                s.channels.remove(&c.id);
                s.messages.remove(&c.id);
                if let Some(g) = c.guild_id {
                    if let Some(v) = s.guild_channels.get_mut(&g) {
                        v.retain(|x| *x != c.id);
                    }
                } else {
                    s.private_channels.retain(|x| *x != c.id);
                }
            }
            GatewayEvent::MessageCreate(m) => {
                let m: Arc<Message> = Arc::new((**m).clone());
                if !m.author.username.is_empty() {
                    s.users.insert(m.author.id, Arc::new(m.author.clone()));
                }
                if let (Some(g), Some(member), None) = (m.guild_id, &m.member, m.webhook_id) {
                    let mut member = member.clone();
                    if member.user.is_none() {
                        member.user = Some(m.author.clone());
                    }
                    s.members.insert((g, m.author.id), Arc::new(member));
                }
                if let Some(c) = s.channels.get(&m.channel_id).cloned() {
                    let mut c = (*c).clone();
                    c.last_message_id = Some(m.id);
                    s.channels.insert(c.id, Arc::new(c));
                }
                let q = s.messages.entry(m.channel_id).or_default();
                if let Some(n) = &m.nonce
                    && let Some(i) = q.iter().position(|x| x.nonce.as_ref() == Some(n) && x.author.id == m.author.id)
                {
                    q.remove(i);
                }
                upsert_message(q, m);
                while q.len() > cap {
                    q.pop_front();
                }
            }
            GatewayEvent::MessageUpdate(v) => {
                let (Some(cid), Some(mid)) = (snowflake_field(v, "channel_id"), snowflake_field(v, "id")) else { return false };
                let Some(q) = s.messages.get_mut(&cid) else { return false };
                let Some(slot) = q.iter_mut().find(|m| m.id == mid) else { return false };
                let Ok(mut base) = serde_json::to_value(&**slot) else { return false };
                if let (Value::Object(b), Value::Object(u)) = (&mut base, &**v) {
                    for (k, val) in u {
                        b.insert(k.clone(), val.clone());
                    }
                }
                match serde_json::from_value::<Message>(base) {
                    Ok(m) => *slot = Arc::new(m),
                    Err(_) => return false,
                }
            }
            GatewayEvent::MessageDelete(d) => {
                let Some(q) = s.messages.get_mut(&d.channel_id) else { return false };
                let before = q.len();
                q.retain(|m| m.id != d.id);
                return q.len() != before;
            }
            GatewayEvent::MessageDeleteBulk(d) => {
                let Some(q) = s.messages.get_mut(&d.channel_id) else { return false };
                q.retain(|m| !d.ids.contains(&m.id));
            }
            GatewayEvent::MessageReactionAdd(r) | GatewayEvent::MessageReactionRemove(r) => {
                let add = matches!(ev, GatewayEvent::MessageReactionAdd(_));
                let me = s.me.as_ref().is_some_and(|u| u.id == r.user_id);
                return modify_message(&mut s, r.channel_id, r.message_id, |m| {
                    m.apply_reaction(&r.emoji, me, add, r.burst);
                    if add
                        && r.burst
                        && let Some(x) = m.reactions.iter_mut().find(|x| x.emoji.same_as(&r.emoji) && x.burst_colors.is_empty())
                    {
                        x.burst_colors = r.burst_colors.clone();
                    }
                });
            }
            GatewayEvent::MessageReactionAddMany(r) => {
                let me = s.me.as_ref().map(|u| u.id);
                return modify_message(&mut s, r.channel_id, r.message_id, |m| {
                    for b in &r.reactions {
                        for u in &b.users {
                            m.apply_reaction(&b.emoji, Some(*u) == me, true, false);
                        }
                    }
                });
            }
            GatewayEvent::MessageReactionRemoveAll(r) => {
                return modify_message(&mut s, r.channel_id, r.message_id, |m| m.reactions.clear());
            }
            GatewayEvent::MessageReactionRemoveEmoji(r) => {
                let Some(e) = &r.emoji else { return false };
                return modify_message(&mut s, r.channel_id, r.message_id, |m| m.reactions.retain(|x| !x.emoji.same_as(e)));
            }
            GatewayEvent::ChannelUnreads { channels, .. } => {
                let mut changed = false;
                for p in channels {
                    let Some(c) = s.channels.get(&p.id) else { continue };
                    if c.last_message_id == p.last_message_id && c.last_pin_timestamp == p.last_pin_timestamp {
                        continue;
                    }
                    let mut c = (**c).clone();
                    c.last_message_id = p.last_message_id;
                    c.last_pin_timestamp = p.last_pin_timestamp.clone();
                    s.channels.insert(c.id, Arc::new(c));
                    changed = true;
                }
                return changed;
            }
            GatewayEvent::VoiceStateUpdate(v) => {
                let key = (v.guild_id, v.user_id);
                if v.channel_id.is_some() {
                    s.voice_states.insert(key, Arc::new((**v).clone()));
                } else {
                    s.voice_states.remove(&key);
                }
            }
            GatewayEvent::RelationshipAdd(r) => {
                if let Some(u) = &r.user {
                    s.users.insert(u.id, Arc::new(u.clone()));
                }
                s.relationships.insert(r.id, Arc::new((**r).clone()));
            }
            GatewayEvent::RelationshipRemove(r) => {
                s.relationships.remove(&r.id);
            }
            GatewayEvent::Unknown { name, data } if name == "RELATIONSHIP_UPDATE" => {
                let Some(id) = data.get("id").and_then(Value::as_str).and_then(|x| x.parse().ok()).map(Snowflake) else { return false };
                let Some(r) = s.relationships.get_mut(&id) else { return false };
                let r = Arc::make_mut(r);
                if let Some(k) = data.get("type").and_then(Value::as_u64).and_then(|k| u8::try_from(k).ok()) {
                    r.kind = RelationshipType::from(k);
                }
                if let Some(n) = data.get("nickname") {
                    r.nickname = n.as_str().map(str::to_owned);
                }
                if let Some(b) = data.get("user_ignored").and_then(Value::as_bool) {
                    r.user_ignored = b;
                }
                if let Some(b) = data.get("is_spam_request").and_then(Value::as_bool) {
                    r.is_spam_request = b;
                }
                if let Some(t) = data.get("since").and_then(Value::as_str) {
                    r.since = Some(t.to_owned());
                }
            }
            _ => return false,
        }
        true
    }
}

fn snowflake_field(v: &Value, k: &str) -> Option<Snowflake> {
    serde_json::from_value(v.get(k)?.clone()).ok()
}

fn upsert_message(q: &mut VecDeque<Arc<Message>>, m: Arc<Message>) {
    if q.back().is_none_or(|b| b.id < m.id) {
        q.push_back(m);
        return;
    }
    match q.binary_search_by_key(&m.id, |x| x.id) {
        Ok(i) => q[i] = m,
        Err(i) => q.insert(i, m),
    }
}

fn insert_guild(s: &mut State, mut g: Guild) {
    let id = g.id;
    let channels = std::mem::take(&mut g.channels);
    let threads = std::mem::take(&mut g.threads);
    let members = std::mem::take(&mut g.members);
    for v in std::mem::take(&mut g.voice_states) {
        if v.channel_id.is_some() {
            let mut v = v;
            v.guild_id.get_or_insert(id);
            s.voice_states.insert((Some(id), v.user_id), Arc::new(v));
        }
    }
    if let Some(old) = s.guild_channels.remove(&id) {
        for c in old {
            s.channels.remove(&c);
        }
    }
    let mut ids = Vec::with_capacity(channels.len() + threads.len());
    for mut c in channels.into_iter().chain(threads) {
        c.guild_id = Some(id);
        ids.push(c.id);
        s.channels.insert(c.id, Arc::new(c));
    }
    s.guild_channels.insert(id, ids);
    for m in members {
        if let Some(uid) = m.user_id() {
            if let Some(u) = &m.user {
                s.users.insert(u.id, Arc::new(u.clone()));
            }
            s.members.insert((id, uid), Arc::new(m));
        }
    }
    if !s.guild_order.contains(&id) {
        s.guild_order.push(id);
    }
    s.guilds.insert(id, Arc::new(g));
}

fn remove_guild(s: &mut State, id: GuildId) {
    s.guilds.remove(&id);
    s.guild_order.retain(|g| *g != id);
    if let Some(chs) = s.guild_channels.remove(&id) {
        for c in chs {
            s.channels.remove(&c);
            s.messages.remove(&c);
        }
    }
    s.members.retain(|(g, _), _| *g != id);
    s.voice_states.retain(|(g, _), _| *g != Some(id));
}

fn insert_channel(s: &mut State, c: Channel) {
    match c.guild_id {
        Some(g) => {
            let v = s.guild_channels.entry(g).or_default();
            if !v.contains(&c.id) {
                v.push(c.id);
            }
        }
        None => {
            if !s.private_channels.contains(&c.id) {
                s.private_channels.push(c.id);
            }
            for u in &c.recipients {
                s.users.entry(u.id).or_insert_with(|| Arc::new(u.clone()));
            }
        }
    }
    s.channels.insert(c.id, Arc::new(c));
}

fn modify_guild(s: &mut State, id: GuildId, f: impl FnOnce(&mut Guild)) -> bool {
    match s.guilds.get_mut(&id) {
        Some(g) => {
            f(Arc::make_mut(g));
            true
        }
        None => false,
    }
}

fn modify_message(s: &mut State, ch: ChannelId, id: MessageId, f: impl FnOnce(&mut Message)) -> bool {
    let Some(q) = s.messages.get_mut(&ch) else { return false };
    match q.iter_mut().find(|m| m.id == id) {
        Some(m) => {
            f(Arc::make_mut(m));
            true
        }
        None => false,
    }
}

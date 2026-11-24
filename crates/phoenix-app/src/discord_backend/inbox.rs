use phoenix_api::{Cache, models as api};
use serde_json::Value;

use super::convert::{Prefs, id, sf};
use crate::{
    model::Id,
    notifications::{Level, NotifyPrefs},
    ui::inbox_unreads::{self as iu, Candidate, Entry, Kind, Notify},
};

const FAVORITED: u64 = 2048;
const CH_UNREADS_ALL: u64 = 1024;
const CH_UNREADS_MENTIONS: u64 = 512;
const G_UNREADS_ALL: u64 = 2048;
const G_UNREADS_MENTIONS: u64 = 4096;
const TH_ALL: u64 = 2;
const TH_NONE: u64 = 8;

pub struct Ctx<'a> {
    pub cache: &'a Cache,
    pub prefs: &'a Prefs,
    pub notify: &'a NotifyPrefs,
    pub new_notifications: bool,
}

pub fn new_notifications(v: Option<&Value>) -> bool {
    v.and_then(|v| v.get("flags")).and_then(Value::as_u64).unwrap_or(0) & 16 != 0
}

impl Ctx<'_> {
    fn channel_pref(&self, g: u64, c: u64) -> Option<crate::notifications::ChannelPrefs> {
        self.notify.guilds.get(&g).and_then(|p| p.channels.get(&c)).copied()
    }

    fn notify_of(&self, g: u64, c: u64) -> Notify {
        let lvl = match self.channel_pref(g, c).map(|p| p.level) {
            Some(Level::Inherit) | None => self.notify.guild(Some(g)).level,
            Some(l) => l,
        };
        match lvl {
            Level::All => Notify::All,
            Level::Nothing => Notify::Nothing,
            _ => Notify::Default,
        }
    }

    fn unreads_all(&self, g: u64, c: &api::Channel) -> bool {
        if c.kind.is_thread() || !self.new_notifications {
            return true;
        }
        let flag = |ch: u64| {
            let f = self.channel_pref(g, ch).map_or(0, |p| p.flags);
            if f & CH_UNREADS_ALL != 0 {
                Some(true)
            } else if f & CH_UNREADS_MENTIONS != 0 {
                Some(false)
            } else {
                None
            }
        };
        if let Some(v) = flag(c.id.0).or_else(|| c.parent_id.and_then(|p| flag(p.0))) {
            return v;
        }
        let gf = self.notify.guild(Some(g)).flags;
        if gf & G_UNREADS_ALL != 0 {
            return true;
        }
        if gf & G_UNREADS_MENTIONS != 0 {
            return false;
        }
        self.notify.resolved_level(Some(g), c.id.0, c.parent_id.map(|p| p.0)) == Level::All
    }

    fn joined(&self, g: u64) -> Option<u64> {
        let at = self.cache.guild(api::Snowflake(g))?.joined_at.clone()?;
        let ms = chrono::DateTime::parse_from_rfc3339(&at).ok()?.timestamp_millis();
        Some(iu::snowflake_from_ms(ms.max(0) as u64))
    }

    fn candidate(&self, c: &api::Channel) -> Candidate {
        let rc = self.prefs.ctx();
        let (has_unread, mentions) = rc.unread(c);
        let g = c.guild_id.map(|g| g.0);
        let private = c.kind.is_private();
        let thread = c.kind.is_thread();
        let member = c.member.as_ref();
        let muted = match g {
            Some(_) if thread => member.is_some_and(crate::notifications::mute_now),
            _ => self.notify.is_guild_or_category_or_channel_muted(g, c.id.0, c.parent_id.map(|p| p.0)),
        };
        let notify = match (g, thread) {
            (Some(_), true) => {
                let f = member.and_then(|m| m.get("flags")).and_then(Value::as_u64).unwrap_or(0);
                if f & TH_ALL != 0 {
                    Notify::All
                } else if f & TH_NONE != 0 {
                    Notify::Nothing
                } else {
                    Notify::Default
                }
            }
            (Some(g), false) => self.notify_of(g, c.id.0),
            (None, _) => Notify::Default,
        };
        Candidate {
            channel: id(c.id),
            guild: c.guild_id.map(id),
            private,
            thread,
            kind: if matches!(c.kind, api::ChannelType::GuildForum | api::ChannelType::GuildMedia) { Kind::Forum } else { Kind::Messages },
            muted,
            has_unread,
            mentions,
            ack: self.prefs.reads.get(&c.id.0).map(|r| r.last_read).filter(|a| *a != 0),
            joined: g.and_then(|g| self.joined(g)),
            last_message: c.last_message_id.map(|m| m.0),
            can_read_history: private || self.cache.permissions_in(c.id).contains(api::Permissions::READ_MESSAGE_HISTORY),
            favorite: g.and_then(|g| self.channel_pref(g, c.id.0)).is_some_and(|p| p.flags & FAVORITED != 0),
            unreads_all: g.is_none_or(|g| self.unreads_all(g, c)),
            notify,
        }
    }

    pub fn entries(&self, guild_order: &[Id], now_ms: u64) -> Vec<Entry> {
        let mut cands = Vec::new();
        for c in self.cache.private_channels() {
            cands.push(self.candidate(&c));
        }
        let rc = self.prefs.ctx();
        for g in guild_order {
            let all = self.cache.guild_channels(sf(*g));
            let mut threads: Vec<_> = all
                .iter()
                .filter(|t| t.kind.is_thread() && t.member.is_some())
                .filter(|t| !t.thread_metadata.as_ref().is_some_and(|m| m.archived))
                .collect();
            threads.sort_by_key(|t| t.id.0);
            for ch in super::convert::guild_channels(self.cache, sf(*g), &rc) {
                if ch.kind == crate::model::ChannelKind::Category {
                    continue;
                }
                let Some(c) = self.cache.channel(sf(ch.id)) else { continue };
                cands.push(self.candidate(&c));
                for t in threads.iter().filter(|t| t.parent_id == Some(c.id)) {
                    cands.push(self.candidate(t));
                }
            }
        }
        iu::build(cands, now_ms)
    }
}

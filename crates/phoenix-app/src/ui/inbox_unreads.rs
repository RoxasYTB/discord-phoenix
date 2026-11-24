//! Inbox > Unreads: which channels the official client lists, in which order,
//! and which of their messages it shows. Rebuilt from the client's inbox
//! module (2026 bundle, `INBOX_OPEN` initial state + its channel loader):
//!
//! - DMs (most recent first), then every guild in rail order (folders
//!   flattened): its channels in channel-list order, each followed by its
//!   joined active unread threads.
//! - A channel is listed when it is not muted (guild, category or channel; a
//!   thread: its own mute), has an unread message after the ack (or after the
//!   guild join date when it was never read), can be read (READ_MESSAGE_HISTORY)
//!   and either has mentions or counts as unread for its "unreads" setting.
//!   DMs need a mention count (every DM message is one).
//! - Then a stable sort on the client's priority: favourite, DM, high / low
//!   importance mention, last message older than 10 days (last) or 2 days,
//!   then the notification level (all messages, default, nothing).
//! - Messages: `around` the ack, 30, keeping those after the ack up to the last
//!   message; 25 shown, then a "View all" button.
//!
//! The client acks a channel by itself when that load finds nothing to show
//! (`ACK_INBOX_CHANNEL_NO_MESSAGES`). Phoenix never acks from the inbox: such
//! a channel is only hidden.

use crate::model::{Id, Message};

/// Discord epoch (2015-01-01) in unix ms.
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
const DAY_MS: u64 = 86_400_000;
/// `fetchMessages({limit: 30, jump: {messageId: oldestRead}})`.
pub const FETCH_LIMIT: u32 = 30;
/// `t.messages.slice(0, 25)`.
pub const SHOWN: usize = 25;

/// `message_notifications`, resolved (channel override, else guild).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Notify {
    All,
    #[default]
    Default,
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Messages,
    /// Forum / media channel: new posts instead of messages.
    Forum,
    /// Guild scheduled events read state (`guild-events`, sortOrder 5).
    GuildEvents,
}

/// One channel as the backend sees it (read state, settings, permissions).
#[derive(Clone, Debug)]
pub struct Candidate {
    pub channel: Id,
    pub guild: Option<Id>,
    pub private: bool,
    pub thread: bool,
    pub kind: Kind,
    /// Guild, category or channel muted (threads: the thread member's mute).
    pub muted: bool,
    pub has_unread: bool,
    pub mentions: u32,
    /// Read state ack; `None` when the channel was never read.
    pub ack: Option<u64>,
    /// Guild join date as a snowflake (ack fallback).
    pub joined: Option<u64>,
    pub last_message: Option<u64>,
    pub can_read_history: bool,
    /// Channel override flag FAVORITED.
    pub favorite: bool,
    /// The channel's unreads setting resolves to "all messages".
    pub unreads_all: bool,
    pub notify: Notify,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub channel: Id,
    pub guild: Option<Id>,
    pub kind: Kind,
    pub oldest_read: Id,
    pub newest: Id,
    pub mentions: u32,
    pub sort: u8,
}

pub fn snowflake_ms(id: u64) -> u64 {
    (id >> 22) + DISCORD_EPOCH_MS
}

pub fn snowflake_from_ms(ms: u64) -> u64 {
    ms.saturating_sub(DISCORD_EPOCH_MS) << 22
}

fn sort_order(c: &Candidate, last: u64, now_ms: u64) -> u8 {
    if c.kind == Kind::GuildEvents {
        return 5;
    }
    if c.favorite {
        return 0;
    }
    if c.private {
        return 1;
    }
    if c.mentions > 0 {
        // Low importance mentions (3) are not tracked: always high.
        return 2;
    }
    let age = now_ms.saturating_sub(snowflake_ms(last));
    if age > 10 * DAY_MS {
        return 8;
    }
    if age > 2 * DAY_MS {
        return 6;
    }
    match c.notify {
        Notify::All => 4,
        Notify::Nothing => 7,
        Notify::Default => 5,
    }
}

/// `nZ` filter + `sortBy(sortOrder)`; `cands` are in client walk order.
pub fn build(cands: impl IntoIterator<Item = Candidate>, now_ms: u64) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for c in cands {
        if c.muted {
            continue;
        }
        if c.private {
            if c.mentions == 0 {
                continue;
            }
        } else {
            if !(c.has_unread && c.unreads_all) && c.mentions == 0 {
                continue;
            }
            if !c.can_read_history {
                continue;
            }
        }
        let Some(ack) = c.ack.or(c.joined) else { continue };
        let Some(last) = c.last_message else { continue };
        if ack >= last {
            continue;
        }
        out.push(Entry {
            channel: c.channel,
            guild: c.guild,
            kind: c.kind,
            oldest_read: Id(ack),
            newest: Id(last),
            mentions: c.mentions,
            sort: sort_order(&c, last, now_ms),
        });
    }
    // lodash sortBy is stable.
    out.sort_by_key(|e| e.sort);
    out
}

/// Messages of one channel card: after the ack, up to the newest unread,
/// oldest first. Returns the shown ones and whether "View all" is needed.
pub fn shown_messages(e: &Entry, mut fetched: Vec<Message>) -> (Vec<Message>, bool) {
    fetched.retain(|m| m.id.0 > e.oldest_read.0 && m.id.0 <= e.newest.0);
    fetched.sort_by_key(|m| m.id.0);
    fetched.dedup_by_key(|m| m.id.0);
    fetched.truncate(SHOWN);
    let more = fetched.last().is_some_and(|m| m.id.0 < e.newest.0);
    (fetched, more)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_795_000_000_000;

    fn cand(id: u64) -> Candidate {
        let last = snowflake_from_ms(NOW - 3_600_000);
        Candidate {
            channel: Id(id),
            guild: Some(Id(1)),
            private: false,
            thread: false,
            kind: Kind::Messages,
            muted: false,
            has_unread: true,
            mentions: 0,
            ack: Some(last - (1 << 22)),
            joined: None,
            last_message: Some(last),
            can_read_history: true,
            favorite: false,
            unreads_all: true,
            notify: Notify::Default,
        }
    }

    fn ids(v: &[Entry]) -> Vec<u64> {
        v.iter().map(|e| e.channel.0).collect()
    }

    #[test]
    fn muted_and_read_channels_are_skipped() {
        let muted = Candidate { muted: true, ..cand(2) };
        let read = Candidate { ack: cand(3).last_message, ..cand(3) };
        let no_perm = Candidate { can_read_history: false, ..cand(4) };
        assert_eq!(ids(&build([cand(1), muted, read, no_perm], NOW)), vec![1]);
    }

    #[test]
    fn dms_need_a_mention_and_come_first() {
        let dm = |id, mentions| Candidate { private: true, guild: None, mentions, ..cand(id) };
        let out = build([cand(10), dm(20, 0), dm(21, 2)], NOW);
        assert_eq!(ids(&out), vec![21, 10]);
        assert_eq!(out[0].sort, 1);
    }

    #[test]
    fn priority_order_is_stable() {
        let all = Candidate { notify: Notify::All, ..cand(2) };
        let nothing = Candidate { notify: Notify::Nothing, ..cand(3) };
        let mention = Candidate { mentions: 1, ..cand(4) };
        let fav = Candidate { favorite: true, ..cand(5) };
        let old = |id, days: u64| {
            let last = snowflake_from_ms(NOW - days * DAY_MS);
            Candidate { last_message: Some(last), ack: Some(last - (1 << 22)), ..cand(id) }
        };
        let out = build([cand(1), all, nothing, mention, fav, old(6, 3), old(7, 11), cand(8)], NOW);
        assert_eq!(ids(&out), vec![5, 4, 2, 1, 8, 6, 3, 7]);
    }

    #[test]
    fn only_mentions_setting_needs_a_mention() {
        let quiet = Candidate { unreads_all: false, ..cand(2) };
        let pinged = Candidate { unreads_all: false, mentions: 3, ..cand(3) };
        assert_eq!(ids(&build([quiet, pinged], NOW)), vec![3]);
    }

    #[test]
    fn never_read_falls_back_to_the_join_date() {
        let last = cand(2).last_message.unwrap();
        let joined_before = Candidate { ack: None, joined: Some(last - 10), ..cand(2) };
        let joined_after = Candidate { ack: None, joined: Some(last + 10), ..cand(3) };
        let no_join = Candidate { ack: None, joined: None, ..cand(4) };
        let out = build([joined_before, joined_after, no_join], NOW);
        assert_eq!(ids(&out), vec![2]);
        assert_eq!(out[0].oldest_read, Id(last - 10));
    }

    #[test]
    fn shown_messages_keep_the_unread_window() {
        let e = Entry { channel: Id(1), guild: None, kind: Kind::Messages, oldest_read: Id(100), newest: Id(140), mentions: 0, sort: 5 };
        let msg = |id| Message { id: Id(id), ..Default::default() };
        let (shown, more) = shown_messages(&e, vec![msg(90), msg(100), msg(120), msg(110), msg(140), msg(150)]);
        assert_eq!(shown.iter().map(|m| m.id.0).collect::<Vec<_>>(), vec![110, 120, 140]);
        assert!(!more);
        let many: Vec<Message> = (101..=140).map(msg).collect();
        let (shown, more) = shown_messages(&e, many);
        assert_eq!(shown.len(), SHOWN);
        assert!(more);
    }
}

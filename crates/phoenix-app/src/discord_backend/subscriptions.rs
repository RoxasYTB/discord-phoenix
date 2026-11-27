use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use phoenix_api::gateway::{GatewayCommand, GatewayHandle, GuildSubscription, Ranges};
use phoenix_api::models::{ChannelId, GuildId};

pub(super) const FIRST_RANGE: [u32; 2] = [0, 99];
const MAX_CHANNELS: usize = 5;
const MAX_THREAD_LISTS: usize = 5;
const FLUSH_DELAY: Duration = Duration::from_millis(10);

#[derive(Default, Clone)]
struct Guild {
    features: bool,
    channels: Vec<(ChannelId, Ranges)>,
    thread_lists: Vec<ChannelId>,
}

impl Guild {
    fn full(&self) -> GuildSubscription {
        GuildSubscription {
            typing: Some(self.features),
            threads: Some(self.features),
            activities: Some(self.features),
            channels: self.channels.iter().cloned().collect(),
            thread_member_lists: self.thread_lists.clone(),
            ..Default::default()
        }
    }
}

#[derive(Default)]
struct State {
    guilds: HashMap<GuildId, Guild>,
    pending: BTreeMap<GuildId, GuildSubscription>,
    scheduled: bool,
}

fn merge(into: &mut GuildSubscription, d: GuildSubscription) {
    into.typing = d.typing.or(into.typing);
    into.threads = d.threads.or(into.threads);
    into.activities = d.activities.or(into.activities);
    into.member_updates = d.member_updates.or(into.member_updates);
    if !d.members.is_empty() {
        into.members = d.members;
    }
    if !d.channels.is_empty() {
        into.channels = d.channels;
    }
    if !d.thread_member_lists.is_empty() {
        into.thread_member_lists = d.thread_member_lists;
    }
}

impl State {
    fn enqueue(&mut self, g: GuildId, d: GuildSubscription) {
        merge(self.pending.entry(g).or_default(), d);
    }

    fn guild(&mut self, g: GuildId) -> bool {
        let e = self.guilds.entry(g).or_default();
        if e.features {
            return false;
        }
        e.features = true;
        self.enqueue(g, GuildSubscription { typing: Some(true), threads: Some(true), activities: Some(true), ..Default::default() });
        true
    }

    fn channel(&mut self, g: GuildId, ch: ChannelId, ranges: Ranges, keep_existing: bool) -> bool {
        let e = self.guilds.entry(g).or_default();
        if let Some(i) = e.channels.iter().position(|c| c.0 == ch) {
            let c = e.channels.remove(i);
            let unchanged = (keep_existing && !c.1.is_empty()) || c.1 == ranges;
            e.channels.insert(0, if unchanged { c } else { (ch, ranges) });
            if unchanged {
                return false;
            }
        } else {
            e.channels.insert(0, (ch, ranges));
            e.channels.truncate(MAX_CHANNELS);
        }
        let channels = e.channels.iter().cloned().collect();
        self.enqueue(g, GuildSubscription { channels, ..Default::default() });
        true
    }

    fn thread_list(&mut self, g: GuildId, thread: ChannelId) -> bool {
        let e = self.guilds.entry(g).or_default();
        e.thread_lists.retain(|t| *t != thread);
        e.thread_lists.insert(0, thread);
        e.thread_lists.truncate(MAX_THREAD_LISTS);
        let lists = e.thread_lists.clone();
        self.enqueue(g, GuildSubscription { thread_member_lists: lists, ..Default::default() });
        true
    }

    fn new_session(&mut self, keep: Option<GuildId>) {
        self.pending.clear();
        self.guilds.retain(|g, _| Some(*g) == keep);
        if let Some(full) = keep.and_then(|g| self.guilds.get(&g).map(|e| (g, e.full()))) {
            self.pending.insert(full.0, full.1);
        }
    }
}

#[derive(Default)]
pub(super) struct Subscriptions {
    state: Arc<Mutex<State>>,
}

impl Subscriptions {
    pub fn guild(&self, gw: &GatewayHandle, g: GuildId) {
        let queued = self.state.lock().guild(g);
        self.schedule(gw, queued);
    }

    pub fn channel(&self, gw: &GatewayHandle, g: GuildId, ch: ChannelId, ranges: Ranges, keep_existing: bool) {
        let queued = {
            let mut s = self.state.lock();
            let a = s.guild(g);
            s.channel(g, ch, ranges, keep_existing) || a
        };
        self.schedule(gw, queued);
    }

    pub fn thread_list(&self, gw: &GatewayHandle, g: GuildId, thread: ChannelId) {
        let queued = {
            let mut s = self.state.lock();
            let a = s.guild(g);
            s.thread_list(g, thread) || a
        };
        self.schedule(gw, queued);
    }

    pub fn new_session(&self, gw: &GatewayHandle, keep: Option<GuildId>) {
        let queued = {
            let mut s = self.state.lock();
            s.new_session(keep);
            !s.pending.is_empty()
        };
        self.schedule(gw, queued);
    }

    fn schedule(&self, gw: &GatewayHandle, queued: bool) {
        if !queued {
            return;
        }
        {
            let mut s = self.state.lock();
            if s.scheduled {
                return;
            }
            s.scheduled = true;
        }
        let (gw, state) = (gw.clone(), self.state.clone());
        match tokio::runtime::Handle::try_current() {
            Ok(rt) => {
                rt.spawn(async move {
                    tokio::time::sleep(FLUSH_DELAY).await;
                    flush(&gw, &state);
                });
            }
            Err(_) => flush(&gw, &state),
        }
    }
}

fn flush(gw: &GatewayHandle, state: &Mutex<State>) {
    let pending = {
        let mut s = state.lock();
        s.scheduled = false;
        std::mem::take(&mut s.pending)
    };
    if !pending.is_empty() {
        let _ = gw.send(GatewayCommand::GuildSubscriptionsBulk(pending));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phoenix_api::models::Snowflake;
    use serde_json::json;

    fn take(s: &mut State) -> serde_json::Value {
        serde_json::to_value(std::mem::take(&mut s.pending)).unwrap()
    }

    #[test]
    fn features_are_sent_once_per_guild() {
        let mut s = State::default();
        assert!(s.guild(Snowflake(1)));
        assert!(!s.guild(Snowflake(1)));
        assert_eq!(take(&mut s), json!({ "1": { "typing": true, "threads": true, "activities": true } }));
    }

    #[test]
    fn channels_keep_the_five_most_recent_and_send_deltas() {
        let mut s = State::default();
        let g = Snowflake(1);
        s.guild(g);
        s.channel(g, Snowflake(10), vec![FIRST_RANGE], true);
        assert_eq!(take(&mut s), json!({ "1": { "typing": true, "threads": true, "activities": true, "channels": { "10": [[0, 99]] } } }));
        for c in 11..=15 {
            s.channel(g, Snowflake(c), vec![FIRST_RANGE], true);
        }
        let v = take(&mut s);
        let keys: Vec<&String> = v["1"]["channels"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["11", "12", "13", "14", "15"]);
        assert!(v["1"].get("typing").is_none());
        assert!(!s.channel(g, Snowflake(13), vec![FIRST_RANGE], true));
        s.channel(g, Snowflake(13), vec![[0, 99], [100, 199]], false);
        assert!(!s.channel(g, Snowflake(13), vec![FIRST_RANGE], true));
        assert_eq!(take(&mut s)["1"]["channels"]["13"], json!([[0, 99], [100, 199]]));
    }

    #[test]
    fn a_new_session_resends_only_the_selected_guild() {
        let mut s = State::default();
        s.channel(Snowflake(1), Snowflake(10), vec![FIRST_RANGE], true);
        s.channel(Snowflake(2), Snowflake(20), vec![FIRST_RANGE], true);
        s.guild(Snowflake(1));
        take(&mut s);
        s.new_session(Some(Snowflake(2)));
        assert_eq!(take(&mut s), json!({ "2": { "typing": false, "threads": false, "activities": false, "channels": { "20": [[0, 99]] } } }));
        assert!(s.guild(Snowflake(1)));
    }
}

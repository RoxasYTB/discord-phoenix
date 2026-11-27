use std::cell::RefCell;
use std::collections::HashSet;

use crate::app::PhoenixApp;
use crate::model::Id;

thread_local! {
    static NET: RefCell<HashSet<Id>> = RefCell::default();
}

pub fn is_net(user: Id) -> bool {
    NET.with(|n| n.borrow().contains(&user))
}

pub fn forget(user: Id) {
    NET.with(|n| n.borrow_mut().remove(&user));
}

fn call_guild(app: &PhoenixApp) -> Option<(Id, Id)> {
    let ch = app.voice_channel?;
    Some((ch, app.channels.get(&ch)?.guild?))
}

pub fn on_clients(app: &mut PhoenixApp, users: &[Id], connected: bool) -> bool {
    let Some((ch, guild)) = call_guild(app) else { return false };
    let me = app.me.id;
    let mut changed = false;
    let mut fetch = Vec::new();
    for &u in users.iter().filter(|u| **u != me) {
        if connected {
            if app.channels.get(&ch).is_some_and(|c| c.voice_users.contains(&u)) {
                continue;
            }
            for c in app.channels.values_mut().filter(|c| c.guild == Some(guild)) {
                c.voice_users.retain(|x| *x != u);
            }
            if let Some(c) = app.channels.get_mut(&ch) {
                c.voice_users.push(u);
            }
            app.pending_voice.remove(&u);
            NET.with(|n| n.borrow_mut().insert(u));
            if !app.users.contains_key(&u) {
                fetch.push(u);
            }
            changed = true;
        } else if NET.with(|n| n.borrow_mut().remove(&u)) {
            if let Some(c) = app.channels.get_mut(&ch) {
                c.voice_users.retain(|x| *x != u);
            }
            changed = true;
        }
    }
    if !fetch.is_empty() {
        app.backend.request_feed_members(guild, fetch);
    }
    changed
}

pub fn reapply(app: &mut PhoenixApp) {
    let Some(ch) = app.voice_channel else { return };
    let net: Vec<Id> = NET.with(|n| n.borrow().iter().copied().collect());
    if let Some(c) = app.channels.get_mut(&ch) {
        for u in net {
            if !c.voice_users.contains(&u) {
                c.voice_users.push(u);
            }
        }
    }
}

pub fn reset(app: &mut PhoenixApp) {
    let net: Vec<Id> = NET.with(|n| n.borrow_mut().drain().collect());
    if let Some(c) = app.voice_channel.and_then(|ch| app.channels.get_mut(&ch)) {
        c.voice_users.retain(|u| !net.contains(u));
    }
}

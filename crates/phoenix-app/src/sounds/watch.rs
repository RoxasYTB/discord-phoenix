//! Voice and call cues, the client's `useSound` hooks (`No(stores, snapshot, diff)` in the
//! main bundle): after every app event a snapshot of the voice state is compared with
//! the previous one and at most one cue plays per hook, at volume `.4`.
//!
//! - `Nc`: self deafen changed -> `deafen` / `undeafen`, else self mute -> `mute` / `unmute`
//!   (from our toggles only, [`on_self_toggle`]);
//! - `Nu`: own camera while in voice -> `camera_on` / `camera_off`;
//! - `N_`: RTC connected -> `user_join`, connected -> gone -> `disconnect` (not stages);
//! - `Nh`: moved by someone else -> `user_moved`;
//! - `Np`: in the same voice channel: someone starts streaming -> `stream_started`, own
//!   stream ends -> `stream_ended`, more / fewer users (previous count <= 25) ->
//!   `user_join` / `user_leave` (not stages);
//! - `NE`: push-to-talk key while in push-to-talk mode and not muted -> `ptt_start` / `ptt_stop`;
//! - incoming call (`Gc`): `call_ringing` looped, 1 in 1000 `call_ringing_beat` or
//!   `call_ringing_snow_halation` (classic soundpack); outgoing (`413528`): `call_calling`
//!   looped while our call rings someone and nobody joined.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

use phoenix_voice::audio::effects::Handle;

use super::{VOLUME, is_disabled, play, play_loop};
use crate::app::PhoenixApp;
use crate::model::{ChannelKind, Id, Presence};
use crate::voice::VoiceStatus;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snap {
    pub deaf: bool,
    pub mute: bool,
    pub channel: Option<Id>,
    pub stage: bool,
    pub connected: bool,
    pub video: bool,
    /// Other users in our voice channel (guild channels).
    pub others: Vec<Id>,
    /// Users of our voice channel who stream.
    pub streamers: Vec<Id>,
    pub me: Id,
    pub incoming: bool,
    pub outgoing: bool,
}

/// One-shot cues for a change from `a` to `b` (`moved`: the channel change was not ours).
/// Self mute / deafen are this client's own state (the client's MediaEngineStore), so
/// they only change through our toggles: `Nc` runs from [`on_self_toggle`], never on
/// the start-up restore of the saved state.
pub fn cues(a: &Snap, b: &Snap, moved: bool) -> Vec<&'static str> {
    voice_cues(a, b, moved)
}

/// `Nc`: deafen wins over mute.
pub fn toggle_cue(a: &Snap, b: &Snap) -> Option<&'static str> {
    if a.deaf != b.deaf {
        Some(if b.deaf { "deafen" } else { "undeafen" })
    } else if a.mute != b.mute {
        Some(if b.mute { "mute" } else { "unmute" })
    } else {
        None
    }
}

fn voice_cues(a: &Snap, b: &Snap, moved: bool) -> Vec<&'static str> {
    let mut out = Vec::new();
    // Nu
    if a.video != b.video && a.channel.is_some() && b.channel.is_some() {
        out.push(if b.video { "camera_on" } else { "camera_off" });
    }
    // N_
    if !a.connected && b.connected && !b.stage {
        out.push("user_join");
    } else if a.connected && !b.connected && !a.stage {
        out.push("disconnect");
    }
    // Nh
    if moved && a.channel.is_some() && b.channel.is_some() && a.channel != b.channel {
        out.push("user_moved");
    }
    // Np: same channel only.
    if a.channel == b.channel && b.channel.is_some() && !b.stage && a.connected && b.connected {
        let small = a.others.len() <= 25;
        if b.streamers.iter().any(|u| !a.streamers.contains(u)) {
            out.push("stream_started");
        } else if a.streamers.iter().any(|u| !b.streamers.contains(u) && *u == b.me) {
            out.push("stream_ended");
        } else if small && b.others.len() > a.others.len() {
            out.push("user_join");
        } else if small && b.others.len() < a.others.len() {
            out.push("user_leave");
        }
    }
    out
}

thread_local! {
    static PREV: RefCell<Option<Snap>> = const { RefCell::new(None) };
    static RING_IN: RefCell<Option<Handle>> = const { RefCell::new(None) };
    static RING_OUT: RefCell<Option<Handle>> = const { RefCell::new(None) };
}

/// Calls that ring someone other than us, by channel (CALL_CREATE / UPDATE / DELETE).
static CALLS: Mutex<Option<HashMap<Id, bool>>> = Mutex::new(None);
/// The channel we asked to join (a change to it is not "moved").
static EXPECTED: Mutex<Option<Option<Id>>> = Mutex::new(None);

pub fn on_call(channel: Id, ringing: &[Id], me: Id) {
    let mut c = CALLS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    c.get_or_insert_with(HashMap::new).insert(channel, ringing.iter().any(|u| *u != me));
}

pub fn on_join_requested(channel: Option<Id>) {
    *EXPECTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(channel);
}

pub fn snapshot(app: &PhoenixApp) -> Snap {
    let channel = app.voice_channel;
    let ch = channel.and_then(|c| app.channels.get(&c));
    let guild_voice = ch.is_some_and(|c| c.guild.is_some());
    let others: Vec<Id> = if guild_voice { ch.map(|c| c.voice_users.iter().copied().filter(|u| *u != app.me.id).collect()).unwrap_or_default() } else { Vec::new() };
    let mut all = others.clone();
    all.push(app.me.id);
    let streamers = if channel.is_some() { all.into_iter().filter(|u| crate::ui::voice_panel::flags(*u).stream).collect() } else { Vec::new() };
    let outgoing = channel.is_some_and(|c| {
        ch.is_some_and(|x| x.guild.is_none())
            && CALLS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().and_then(|m| m.get(&c).copied()).unwrap_or(false)
    });
    Snap {
        deaf: app.deafened,
        mute: app.mic_muted,
        channel,
        stage: ch.is_some_and(|c| c.kind == ChannelKind::Stage),
        connected: channel.is_some() && app.voice.status == VoiceStatus::Connected,
        video: channel.is_some() && crate::ui::voice_panel::flags(app.me.id).video,
        others,
        streamers,
        me: app.me.id,
        incoming: app.parity.incoming_call.is_some(),
        outgoing,
    }
}

fn set_loop(cell: &'static std::thread::LocalKey<RefCell<Option<Handle>>>, on: bool, start: impl FnOnce() -> Option<Handle>) {
    cell.with(|h| {
        let mut h = h.borrow_mut();
        if on && h.as_ref().is_none_or(Handle::is_stopped) {
            *h = start();
        } else if !on && let Some(x) = h.take() {
            x.stop();
        }
    });
}

/// Classic soundpack ringtone: `500 !== random(1, 1000) ? "call_ringing" : sample([beat, snow_halation])`.
fn ringtone() -> &'static str {
    let r = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    match r % 1000 {
        499 => {
            if r.is_multiple_of(2) { "call_ringing_beat" } else { "call_ringing_snow_halation" }
        }
        _ => "call_ringing",
    }
}

/// After every app event / own toggle.
pub fn watch(app: &PhoenixApp) {
    let quiet = app.prefs.synced.flag("quiet-mode");
    {
        let mut c = super::ctx();
        c.selected = app.selected_channel.map(|i| i.0);
        c.dnd = app.presence == Presence::Dnd;
        c.quiet = quiet;
    }
    let b = snapshot(app);
    let pending = EXPECTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    if pending.is_some_and(|w| w != b.channel) {
        // Not there yet: keep waiting for the requested channel.
        *EXPECTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = pending;
    }
    let prev = PREV.with(|p| p.borrow_mut().replace(b.clone()));
    if let Some(a) = prev
        && a != b
    {
        for s in cues(&a, &b, pending.is_none()) {
            if !is_disabled(s) {
                play(s, VOLUME);
            }
        }
    }
    set_loop(&RING_IN, b.incoming && !is_disabled("call_ringing"), || play_loop(ringtone(), VOLUME));
    set_loop(&RING_OUT, b.outgoing && !is_disabled("call_calling"), || play_loop("call_calling", VOLUME));
}

/// The user panel / shortcut toggled mute or deafen.
pub fn on_self_toggle(app: &PhoenixApp) {
    let b = snapshot(app);
    let prev = PREV.with(|p| p.borrow().clone());
    if let Some(a) = prev
        && let Some(s) = toggle_cue(&a, &b)
        && !is_disabled(s)
    {
        play(s, VOLUME);
    }
    watch(app);
}

/// Push-to-talk key (`NE`): only in push-to-talk mode, in voice and not muted.
pub fn on_ptt(app: &PhoenixApp, pressed: bool) {
    if app.voice_channel.is_none() || app.mic_muted || app.deafened {
        return;
    }
    let s = if pressed { "ptt_start" } else { "ptt_stop" };
    if !is_disabled(s) {
        play(s, VOLUME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s() -> Snap {
        Snap { channel: Some(Id(1)), connected: true, me: Id(9), ..Default::default() }
    }

    #[test]
    fn voice_cues() {
        let a = s();
        assert!(cues(&a, &Snap { deaf: true, ..a.clone() }, false).is_empty(), "restored state is silent");
        assert_eq!(toggle_cue(&a, &Snap { deaf: true, mute: true, ..a.clone() }), Some("deafen"));
        assert_eq!(toggle_cue(&a, &Snap { mute: true, ..a.clone() }), Some("mute"));
        let idle = Snap { channel: None, connected: false, ..a.clone() };
        assert_eq!(cues(&Snap { connected: false, ..a.clone() }, &a, false), ["user_join"]);
        assert_eq!(cues(&a, &idle, false), ["disconnect"]);
        assert!(cues(&Snap { stage: true, ..a.clone() }, &Snap { stage: true, ..idle.clone() }, false).is_empty(), "stages are silent");
        assert_eq!(cues(&a, &Snap { channel: Some(Id(2)), ..a.clone() }, true), ["user_moved"]);
        assert!(cues(&a, &Snap { channel: Some(Id(2)), ..a.clone() }, false).is_empty(), "our own switch");
        let joined = Snap { others: vec![Id(5)], ..a.clone() };
        assert_eq!(cues(&a, &joined, false), ["user_join"]);
        assert_eq!(cues(&joined, &a, false), ["user_leave"]);
        let big = Snap { others: (0..26).map(Id).collect(), ..a.clone() };
        assert!(cues(&big, &Snap { others: (0..27).map(Id).collect(), ..a.clone() }, false).is_empty(), "over 25 users");
        let live = Snap { streamers: vec![Id(5)], ..joined.clone() };
        assert_eq!(cues(&joined, &live, false), ["stream_started"]);
        assert!(cues(&live, &joined, false).is_empty(), "someone else's stream ended");
        let mine = Snap { streamers: vec![Id(9)], ..a.clone() };
        assert_eq!(cues(&mine, &a, false), ["stream_ended"]);
        assert_eq!(cues(&a, &Snap { video: true, ..a.clone() }, false), ["camera_on"]);
    }
}

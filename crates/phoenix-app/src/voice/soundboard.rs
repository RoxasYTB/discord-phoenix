use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use gpui::{Context, SharedString};
use parking_lot::Mutex;
use phoenix_voice::audio::effects::{self, Handle, Pcm};

use crate::app::PhoenixApp;
use crate::model::{Id, SoundboardSound};

#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    pub channel: Id,
    pub user: Id,
    pub sound: Option<Id>,
    pub sound_volume: f32,
    pub emoji_id: Option<Id>,
    pub emoji_name: Option<SharedString>,
    pub emoji_animated: bool,
    pub animation: Option<(u8, u32)>,
}

pub fn sound_url(id: Id) -> String {
    format!("https://cdn.discordapp.com/soundboard-sounds/{}", id.0)
}

pub fn perceptual(v: f32) -> f32 {
    if v == 0. {
        return 0.;
    }
    let n = v / 100.;
    (if n < 1. { n.powf(1. / 2.8) } else { 20. * n.log10() / 6. + 1. }) * 100.
}

pub fn playback_volume(sound_volume: f32, setting: f32, output_volume: u32) -> f32 {
    (sound_volume * (perceptual(setting) / 100.) * (output_volume as f32 / 100.).min(1.)).clamp(0., 1.)
}

pub fn setting_volume(app: &PhoenixApp) -> f32 {
    app.prefs.synced.proto.get("sb-volume").map_or(100., |v| f32::from_bits(*v as u32))
}

#[derive(Default)]
struct Playing {
    users: HashMap<Id, u32>,
    sounds: HashMap<Id, u32>,
    handles: Vec<Handle>,
    generation: u64,
}

fn playing() -> &'static Mutex<Playing> {
    static P: OnceLock<Mutex<Playing>> = OnceLock::new();
    P.get_or_init(Default::default)
}

fn decoded() -> &'static Mutex<HashMap<Id, Arc<Pcm>>> {
    static C: OnceLock<Mutex<HashMap<Id, Arc<Pcm>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

pub fn is_user_playing(user: Id) -> bool {
    playing().lock().users.get(&user).is_some_and(|n| *n > 0)
}

pub fn is_sound_playing(sound: Id) -> bool {
    playing().lock().sounds.get(&sound).is_some_and(|n| *n > 0)
}

pub fn stop_all() {
    crate::ui::voice_effects::clear();
    let mut p = playing().lock();
    for h in p.handles.drain(..) {
        h.stop();
    }
    p.users.clear();
    p.sounds.clear();
    p.generation += 1;
}

fn mark(user: Id, sound: Id, delta: i32) {
    let mut g = playing().lock();
    let p = &mut *g;
    for (map, key) in [(&mut p.users, user), (&mut p.sounds, sound)] {
        let n = map.get(&key).copied().unwrap_or(0) as i32 + delta;
        if n <= 0 {
            map.remove(&key);
        } else {
            map.insert(key, n as u32);
        }
    }
}

pub fn decode(bytes: &[u8]) -> Option<Pcm> {
    if phoenix_voice::audio::ogg_opus::is_ogg(bytes) {
        return phoenix_voice::audio::ogg_opus::decode(bytes);
    }
    crate::sounds::assets::decode(bytes)
}

fn download(id: Id) -> Option<Vec<u8>> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    rt.block_on(async {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().ok()?;
        let r = client.get(sound_url(id)).send().await.ok()?;
        if !r.status().is_success() {
            return None;
        }
        Some(r.bytes().await.ok()?.to_vec())
    })
}

pub fn load(id: Id) -> Option<Arc<Pcm>> {
    if let Some(p) = decoded().lock().get(&id) {
        return Some(p.clone());
    }
    let p = Arc::new(decode(&download(id)?)?);
    decoded().lock().insert(id, p.clone());
    Some(p)
}

fn log(what: &str, sound: Id, user: Id, volume: f32) {
    if std::env::var_os("PHOENIX_SOUND_LOG").is_some() {
        eprintln!("phoenix-soundboard: {what} sound={} user={} volume={volume:.3}", sound.0, user.0);
    }
}

pub fn play(app: &PhoenixApp, sound: Id, sound_volume: f32, user: Id, cx: &mut Context<PhoenixApp>) {
    if app.voice_channel.is_none() {
        return;
    }
    play_locally(app, sound, sound_volume, user, cx);
}

pub fn preview(app: &PhoenixApp, sound: &SoundboardSound, cx: &mut Context<PhoenixApp>) {
    play_locally(app, sound.id, sound.volume, app.me.id, cx);
}

fn play_locally(app: &PhoenixApp, sound: Id, sound_volume: f32, user: Id, cx: &mut Context<PhoenixApp>) {
    if app.deafened {
        return;
    }
    let volume = playback_volume(sound_volume, setting_volume(app), crate::voice::prefs::prefs().lock().output_volume);
    let generation = playing().lock().generation;
    log("load", sound, user, volume);
    cx.spawn(async move |this, cx| {
        let Some(pcm) = cx.background_executor().spawn(async move { load(sound) }).await else {
            log("failed", sound, user, volume);
            return;
        };
        let dur = pcm.duration();
        {
            let mut p = playing().lock();
            if p.generation != generation {
                return;
            }
            let h = effects::play(pcm, volume, false);
            p.handles.retain(|h| !h.is_stopped());
            p.handles.push(h);
        }
        mark(user, sound, 1);
        log("start", sound, user, volume);
        let _ = this.update(cx, |_, cx| cx.notify());
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        std::thread::spawn(move || {
            std::thread::sleep(dur);
            let _ = tx.send(());
        });
        let _ = rx.await;
        if playing().lock().generation == generation {
            mark(user, sound, -1);
        }
        log("end", sound, user, volume);
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

pub fn can_play(app: &PhoenixApp, sound: &SoundboardSound) -> bool {
    let Some(ch) = app.voice_channel else { return false };
    let f = crate::ui::voice_panel::flags(app.me.id);
    if app.deafened || f.server_mute || f.server_deaf || f.suppress {
        return false;
    }
    let voice_guild = app.channels.get(&ch).and_then(|c| c.guild);
    let home = sound.guild.filter(|g| g.0 != 0);
    sound.available && (home.is_none() || home == voice_guild || app.backend.has_premium())
}

pub fn play_own(app: &mut PhoenixApp, sound: SoundboardSound, cx: &mut Context<PhoenixApp>) {
    let Some(ch) = app.voice_channel else { return };
    if !can_play(app, &sound) {
        return;
    }
    let fut = app.backend.send_soundboard_sound(ch, sound.clone());
    std::thread::spawn(move || {
        if let Err(e) = futures::executor::block_on(fut) {
            tracing::warn!("soundboard send failed: {}", e.0);
        }
    });
    let me = app.me.id;
    play(app, sound.id, sound.volume, me, cx);
}

pub fn on_effect(app: &mut PhoenixApp, e: Effect, cx: &mut Context<PhoenixApp>) {
    if Some(e.channel) != app.voice_channel {
        return;
    }
    if std::env::var_os("PHOENIX_SOUND_LOG").is_some() {
        eprintln!("phoenix-soundboard: effect {e:?}");
    }
    crate::ui::voice_effects::start(app, &e, cx);
    if let Some(sound) = e.sound
        && e.user != app.me.id
    {
        play(app, sound, e.sound_volume, e.user, cx);
    }
    cx.notify();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_matches_client_formula() {
        assert!((playback_volume(1., 100., 100) - 1.).abs() < 1e-6);
        assert!((perceptual(50.) - 0.5f32.powf(1. / 2.8) * 100.).abs() < 1e-4);
        assert!((playback_volume(0.5, 100., 200) - 0.5).abs() < 1e-6);
        assert_eq!(playback_volume(1., 0., 100), 0.);
        assert!((playback_volume(1., 100., 50) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn playing_counts() {
        stop_all();
        mark(Id(1), Id(2), 1);
        mark(Id(1), Id(3), 1);
        assert!(is_user_playing(Id(1)));
        assert!(is_sound_playing(Id(2)));
        mark(Id(1), Id(2), -1);
        assert!(is_user_playing(Id(1)));
        assert!(!is_sound_playing(Id(2)));
        stop_all();
        assert!(!is_user_playing(Id(1)));
    }
}

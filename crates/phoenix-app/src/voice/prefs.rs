use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use parking_lot::Mutex;
use serde_json::{Map, Value, json};

use super::VoiceControl;
use crate::model::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    VoiceActivity,
    PushToTalk,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UserAudio {
    pub volume: u32,
    pub muted: bool,
    pub soundboard_muted: bool,
    pub video_disabled: bool,
}

impl Default for UserAudio {
    fn default() -> Self {
        Self { volume: 100, muted: false, soundboard_muted: false, video_disabled: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamAudio {
    pub volume: f32,
    pub muted: bool,
}

pub const STREAM_DEFAULT_VOLUME: f32 = 18.;

impl Default for StreamAudio {
    fn default() -> Self {
        Self { volume: STREAM_DEFAULT_VOLUME, muted: false }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoicePrefs {
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub input_volume: u32,
    pub output_volume: u32,
    pub input_mode: InputMode,
    pub auto_sensitivity: bool,
    pub sensitivity_db: f32,
    pub ptt_key: Option<String>,
    pub ptt_delay_ms: u32,
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub noise_cancellation: bool,
    pub vad_use_krisp: bool,
    pub agc: bool,
    pub attenuation: u32,
    pub attenuate_self: bool,
    pub attenuate_others: bool,
    pub users: HashMap<u64, UserAudio>,
    pub streams: HashMap<u64, StreamAudio>,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub audio_profile: u32,
}

pub const PROFILE_ISOLATION: u32 = 0;
pub const PROFILE_STUDIO: u32 = 1;
pub const PROFILE_CUSTOM: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Processing {
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub noise_cancellation: bool,
    pub agc: bool,
    pub auto_sensitivity: bool,
    pub sensitivity_db: f32,
    pub vad_use_krisp: bool,
    pub push_to_talk: bool,
}

impl Default for VoicePrefs {
    fn default() -> Self {
        Self {
            input_device: None,
            output_device: None,
            input_volume: 100,
            output_volume: 100,
            input_mode: InputMode::VoiceActivity,
            auto_sensitivity: true,
            sensitivity_db: -60.,
            ptt_key: None,
            ptt_delay_ms: 20,
            echo_cancellation: true,
            noise_suppression: false,
            noise_cancellation: true,
            vad_use_krisp: true,
            agc: true,
            attenuation: 0,
            attenuate_self: false,
            attenuate_others: true,
            users: HashMap::new(),
            streams: HashMap::new(),
            self_mute: false,
            self_deaf: false,
            audio_profile: PROFILE_CUSTOM,
        }
    }
}

fn path() -> Option<PathBuf> {
    crate::paths::config_file("voice.json")
}

impl VoicePrefs {
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let n = |k: &str| v.get(k).and_then(Value::as_f64);
        let b = |k: &str, def: bool| v.get(k).and_then(Value::as_bool).unwrap_or(def);
        let users = v
            .get("users")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, u)| {
                        let id = k.parse().ok()?;
                        let volume = u.get("volume").and_then(Value::as_u64).unwrap_or(100).min(200) as u32;
                        let muted = u.get("muted").and_then(Value::as_bool).unwrap_or(false);
                        let soundboard_muted = u.get("soundboard_muted").and_then(Value::as_bool).unwrap_or(false);
                        let video_disabled = u.get("video_disabled").and_then(Value::as_bool).unwrap_or(false);
                        Some((id, UserAudio { volume, muted, soundboard_muted, video_disabled }))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let streams = v
            .get("streams")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, u)| {
                        let id = k.parse().ok()?;
                        let volume = u.get("volume").and_then(Value::as_f64).map_or(STREAM_DEFAULT_VOLUME, |x| x.clamp(0., 200.) as f32);
                        let muted = u.get("muted").and_then(Value::as_bool).unwrap_or(false);
                        Some((id, StreamAudio { volume, muted }))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            input_device: s("input_device"),
            output_device: s("output_device"),
            input_volume: n("input_volume").map(|x| x.clamp(0., 100.) as u32).unwrap_or(d.input_volume),
            output_volume: n("output_volume").map(|x| x.clamp(0., 200.) as u32).unwrap_or(d.output_volume),
            input_mode: if s("input_mode").as_deref() == Some("ptt") { InputMode::PushToTalk } else { InputMode::VoiceActivity },
            auto_sensitivity: b("auto_sensitivity", d.auto_sensitivity),
            sensitivity_db: n("sensitivity_db").map(|x| (x as f32).clamp(-100., 0.)).unwrap_or(d.sensitivity_db),
            ptt_key: s("ptt_key"),
            ptt_delay_ms: n("ptt_delay_ms").map(|x| x.clamp(0., 2000.) as u32).unwrap_or(d.ptt_delay_ms),
            echo_cancellation: b("echo_cancellation", d.echo_cancellation),
            noise_suppression: if v.get("noise_cancellation").is_some() { b("noise_suppression", d.noise_suppression) } else { false },
            noise_cancellation: b("noise_cancellation", b("noise_suppression", d.noise_cancellation)),
            vad_use_krisp: b("vad_use_krisp", d.vad_use_krisp),
            agc: b("agc", d.agc),
            attenuation: n("attenuation").map(|x| x.clamp(0., 100.) as u32).unwrap_or(d.attenuation),
            attenuate_self: b("attenuate_self", d.attenuate_self),
            attenuate_others: b("attenuate_others", d.attenuate_others),
            users,
            streams,
            self_mute: b("self_mute", false),
            self_deaf: b("self_deaf", false),
            audio_profile: n("audio_profile").map(|x| x.clamp(0., 2.) as u32).unwrap_or(d.audio_profile),
        }
    }

    pub fn to_json(&self) -> Value {
        let users: Map<String, Value> = self
            .users
            .iter()
            .filter(|(_, u)| **u != UserAudio::default())
            .map(|(id, u)| {
                (
                    id.to_string(),
                    json!({
                        "volume": u.volume,
                        "muted": u.muted,
                        "soundboard_muted": u.soundboard_muted,
                        "video_disabled": u.video_disabled
                    }),
                )
            })
            .collect();
        let streams: Map<String, Value> = self
            .streams
            .iter()
            .filter(|(_, s)| **s != StreamAudio::default())
            .map(|(id, s)| (id.to_string(), json!({ "volume": s.volume, "muted": s.muted })))
            .collect();
        json!({
            "input_device": self.input_device,
            "output_device": self.output_device,
            "input_volume": self.input_volume,
            "output_volume": self.output_volume,
            "input_mode": if self.input_mode == InputMode::PushToTalk { "ptt" } else { "vad" },
            "auto_sensitivity": self.auto_sensitivity,
            "sensitivity_db": self.sensitivity_db,
            "ptt_key": self.ptt_key,
            "ptt_delay_ms": self.ptt_delay_ms,
            "echo_cancellation": self.echo_cancellation,
            "noise_suppression": self.noise_suppression,
            "noise_cancellation": self.noise_cancellation,
            "vad_use_krisp": self.vad_use_krisp,
            "agc": self.agc,
            "attenuation": self.attenuation,
            "attenuate_self": self.attenuate_self,
            "attenuate_others": self.attenuate_others,
            "users": users,
            "streams": streams,
            "self_mute": self.self_mute,
            "self_deaf": self.self_deaf,
            "audio_profile": self.audio_profile,
        })
    }

    fn load() -> Self {
        path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|v| Self::from_json(&v))
            .unwrap_or_default()
    }

    fn save(&self) {
        let Some(p) = path() else { return };
        let write = || -> std::io::Result<()> {
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let data = serde_json::to_vec_pretty(&self.to_json())?;
            let tmp = p.with_extension("json.tmp");
            std::fs::write(&tmp, data)?;
            std::fs::rename(tmp, p)
        };
        if let Err(error) = write() {
            tracing::warn!(%error, "could not save voice preferences");
        }
    }

    pub fn user(&self, id: Id) -> UserAudio {
        self.users.get(&id.0).copied().unwrap_or_default()
    }

    pub fn stream(&self, owner: Id) -> StreamAudio {
        self.streams.get(&owner.0).copied().unwrap_or_default()
    }

    pub fn user_gain(&self, id: Id) -> f32 {
        let u = self.user(id);
        if u.muted { 0. } else { amplitude(u.volume as f32) }
    }

    pub fn processing(&self) -> Processing {
        let mut p = Processing {
            echo_cancellation: self.echo_cancellation,
            noise_suppression: self.noise_suppression && !self.noise_cancellation,
            noise_cancellation: self.noise_cancellation,
            agc: self.agc,
            auto_sensitivity: self.auto_sensitivity,
            sensitivity_db: self.sensitivity_db,
            vad_use_krisp: self.vad_use_krisp,
            push_to_talk: self.input_mode == InputMode::PushToTalk,
        };
        match self.audio_profile {
            PROFILE_ISOLATION => {
                p.echo_cancellation = true;
                p.noise_suppression = false;
                p.noise_cancellation = true;
                p.agc = true;
                p.auto_sensitivity = true;
                p.vad_use_krisp = true;
            }
            PROFILE_STUDIO => {
                p.echo_cancellation = false;
                p.noise_suppression = false;
                p.noise_cancellation = false;
                p.agc = false;
                p.auto_sensitivity = false;
                p.sensitivity_db = -84.;
                p.vad_use_krisp = false;
                p.push_to_talk = false;
            }
            _ => {}
        }
        p
    }

    pub fn controls(&self) -> Vec<VoiceControl> {
        let e = self.processing();
        let mut v = vec![
            VoiceControl::SetInputDevice(self.input_device.clone()),
            VoiceControl::SetOutputDevice(self.output_device.clone()),
            VoiceControl::SetInputVolume(amplitude(self.input_volume as f32)),
            VoiceControl::SetOutputVolume(amplitude(self.output_volume as f32)),
            VoiceControl::SetSensitivity(if e.auto_sensitivity { None } else { Some(e.sensitivity_db) }),
            VoiceControl::SetNoiseSuppression(e.noise_suppression),
            VoiceControl::SetNoiseCancellation(e.noise_cancellation),
            VoiceControl::SetVadKrisp(e.vad_use_krisp),
            VoiceControl::SetEchoCancellation(e.echo_cancellation),
            VoiceControl::SetAgc(e.agc),
            VoiceControl::SetPttDelay(self.ptt_delay_ms),
            VoiceControl::SetPushToTalk(e.push_to_talk),
            VoiceControl::SetPttKey(if e.push_to_talk { self.ptt_key.clone() } else { None }),
        ];
        v.extend(self.users.keys().map(|u| VoiceControl::SetUserVolume(Id(*u), self.user_gain(Id(*u)))));
        for (owner, s) in &self.streams {
            v.push(VoiceControl::SetStreamVolume(Id(*owner), s.volume / 100.));
            v.push(VoiceControl::SetStreamMute(Id(*owner), s.muted));
        }
        v
    }
}

pub fn amplitude(perceptual_pct: f32) -> f32 {
    if perceptual_pct <= 0. {
        return 0.;
    }
    let n = perceptual_pct / 100.;
    if n < 1. { n.powf(2.8) } else { 10f32.powf((n - 1.) * 6. / 20.) }
}

pub fn prefs() -> &'static Mutex<VoicePrefs> {
    static P: OnceLock<Mutex<VoicePrefs>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(VoicePrefs::load()))
}

pub fn update(f: impl FnOnce(&mut VoicePrefs), save: bool) {
    let (before, after) = {
        let mut p = prefs().lock();
        let before = p.clone();
        f(&mut p);
        if save {
            p.save();
        }
        (before, p.clone())
    };
    if before == after {
        return;
    }
    let old = before.controls();
    for c in after.controls() {
        if !old.contains(&c) {
            super::call::manager().apply_control(None, c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_clamp() {
        let mut p = VoicePrefs { input_volume: 40, input_mode: InputMode::PushToTalk, ptt_key: Some("ctrl-a".into()), ..Default::default() };
        p.users.insert(9, UserAudio { volume: 150, muted: true, ..Default::default() });
        assert_eq!(VoicePrefs::from_json(&p.to_json()), p);
        let c = VoicePrefs::from_json(&json!({"output_volume": 900, "sensitivity_db": -500}));
        assert_eq!((c.output_volume, c.sensitivity_db), (200, -100.));
        assert_eq!(p.user_gain(Id(9)), 0.);
        assert_eq!(p.user_gain(Id(1)), 1.);
        p.users.insert(9, UserAudio { volume: 50, muted: false, ..Default::default() });
        assert!((p.user_gain(Id(9)) - 0.5f32.powf(2.8)).abs() < 1e-6);
        p.users.insert(9, UserAudio { volume: 200, muted: false, ..Default::default() });
        assert!((p.user_gain(Id(9)) - 10f32.powf(0.3)).abs() < 1e-5);
        assert_eq!(amplitude(0.), 0.);
        assert!((amplitude(100.) - 1.).abs() < 1e-6);
        assert!((crate::voice::soundboard::perceptual(amplitude(73.) * 100.) - 73.).abs() < 1e-3);
    }

    #[test]
    fn stream_audio_is_kept_apart_from_the_member_voice() {
        let mut p = VoicePrefs::default();
        assert_eq!(p.stream(Id(5)), StreamAudio { volume: 18., muted: false });
        p.streams.insert(5, StreamAudio { volume: 40., muted: true });
        p.users.insert(5, UserAudio { volume: 120, ..Default::default() });
        let back = VoicePrefs::from_json(&p.to_json());
        assert_eq!(back, p);
        assert!(p.to_json()["streams"]["5"]["muted"].as_bool().unwrap());
        let c = p.controls();
        assert!(c.contains(&VoiceControl::SetStreamVolume(Id(5), 0.4)));
        assert!(c.contains(&VoiceControl::SetStreamMute(Id(5), true)));
        assert!(c.contains(&VoiceControl::SetUserVolume(Id(5), amplitude(120.))));
        p.streams.insert(5, StreamAudio::default());
        assert!(p.to_json()["streams"].as_object().unwrap().is_empty());
        let c = VoicePrefs::from_json(&json!({"streams": {"9": {"volume": 900}}}));
        assert_eq!(c.stream(Id(9)).volume, 200.);
    }

    #[test]
    fn input_profiles_override_the_custom_settings_like_the_client() {
        let custom = VoicePrefs {
            echo_cancellation: false,
            noise_suppression: true,
            noise_cancellation: false,
            agc: false,
            auto_sensitivity: false,
            sensitivity_db: -40.,
            input_mode: InputMode::PushToTalk,
            ..Default::default()
        };
        let e = custom.processing();
        assert!(!e.echo_cancellation && e.noise_suppression && !e.noise_cancellation && !e.agc && e.push_to_talk);
        assert_eq!(e.sensitivity_db, -40.);
        let iso = VoicePrefs { audio_profile: PROFILE_ISOLATION, ..custom.clone() }.processing();
        assert!(iso.echo_cancellation && !iso.noise_suppression && iso.noise_cancellation && iso.agc && iso.auto_sensitivity && iso.vad_use_krisp);
        let studio = VoicePrefs { audio_profile: PROFILE_STUDIO, ..custom.clone() }.processing();
        assert!(!studio.echo_cancellation && !studio.noise_suppression && !studio.noise_cancellation && !studio.agc && !studio.push_to_talk);
        assert_eq!((studio.auto_sensitivity, studio.sensitivity_db), (false, -84.));
        assert!(!studio.vad_use_krisp);
        let c = VoicePrefs { audio_profile: PROFILE_STUDIO, ..custom }.controls();
        assert!(c.contains(&VoiceControl::SetSensitivity(Some(-84.))));
        assert!(c.contains(&VoiceControl::SetPttKey(None)));
    }

    #[test]
    fn old_noise_suppression_flag_becomes_krisp() {
        let old = VoicePrefs::from_json(&json!({"noise_suppression": true}));
        assert!(old.noise_cancellation && !old.noise_suppression);
        let off = VoicePrefs::from_json(&json!({"noise_suppression": false}));
        assert!(!off.noise_cancellation && !off.noise_suppression);
        let d = VoicePrefs::default();
        assert!(d.echo_cancellation && !d.noise_suppression && d.noise_cancellation && d.agc && d.vad_use_krisp);
        assert_eq!(d.audio_profile, PROFILE_CUSTOM);
    }
}

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
}

impl Default for UserAudio {
    fn default() -> Self {
        Self { volume: 100, muted: false }
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
    pub agc: bool,
    pub attenuation: u32,
    pub attenuate_self: bool,
    pub attenuate_others: bool,
    pub users: HashMap<u64, UserAudio>,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub audio_profile: u32,
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
            noise_suppression: true,
            agc: true,
            attenuation: 0,
            attenuate_self: false,
            attenuate_others: true,
            users: HashMap::new(),
            self_mute: false,
            self_deaf: false,
            audio_profile: 0,
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join(crate::APP_ID).join("voice.json"))
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
                        Some((id, UserAudio { volume, muted }))
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
            noise_suppression: b("noise_suppression", d.noise_suppression),
            agc: b("agc", d.agc),
            attenuation: n("attenuation").map(|x| x.clamp(0., 100.) as u32).unwrap_or(d.attenuation),
            attenuate_self: b("attenuate_self", d.attenuate_self),
            attenuate_others: b("attenuate_others", d.attenuate_others),
            users,
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
            .map(|(id, u)| (id.to_string(), json!({ "volume": u.volume, "muted": u.muted })))
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
            "agc": self.agc,
            "attenuation": self.attenuation,
            "attenuate_self": self.attenuate_self,
            "attenuate_others": self.attenuate_others,
            "users": users,
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

    pub fn save(&self) {
        let Some(p) = path() else { return };
        let data = serde_json::to_vec_pretty(&self.to_json()).unwrap_or_default();
        std::thread::spawn(move || {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, data).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        });
    }

    pub fn user(&self, id: Id) -> UserAudio {
        self.users.get(&id.0).copied().unwrap_or_default()
    }

    pub fn user_gain(&self, id: Id) -> f32 {
        let u = self.user(id);
        if u.muted { 0. } else { u.volume as f32 / 100. }
    }

    pub fn controls(&self) -> Vec<VoiceControl> {
        let mut v = vec![
            VoiceControl::SetInputDevice(self.input_device.clone()),
            VoiceControl::SetOutputDevice(self.output_device.clone()),
            VoiceControl::SetInputVolume(self.input_volume as f32 / 100.),
            VoiceControl::SetOutputVolume(self.output_volume as f32 / 100.),
            VoiceControl::SetSensitivity(if self.auto_sensitivity { None } else { Some(self.sensitivity_db) }),
            VoiceControl::SetNoiseSuppression(self.noise_suppression),
            VoiceControl::SetEchoCancellation(self.echo_cancellation),
            VoiceControl::SetAgc(self.agc),
            VoiceControl::SetPttDelay(self.ptt_delay_ms),
            VoiceControl::SetPushToTalk(self.input_mode == InputMode::PushToTalk),
            VoiceControl::SetPttKey(if self.input_mode == InputMode::PushToTalk { self.ptt_key.clone() } else { None }),
        ];
        v.extend(self.users.keys().map(|u| VoiceControl::SetUserVolume(Id(*u), self.user_gain(Id(*u)))));
        v
    }
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
    if save {
        after.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_clamp() {
        let mut p = VoicePrefs { input_volume: 40, input_mode: InputMode::PushToTalk, ptt_key: Some("ctrl-a".into()), ..Default::default() };
        p.users.insert(9, UserAudio { volume: 150, muted: true });
        assert_eq!(VoicePrefs::from_json(&p.to_json()), p);
        let c = VoicePrefs::from_json(&json!({"output_volume": 900, "sensitivity_db": -500}));
        assert_eq!((c.output_volume, c.sensitivity_db), (200, -100.));
        assert_eq!(p.user_gain(Id(9)), 0.);
        assert_eq!(p.user_gain(Id(1)), 1.);
    }
}

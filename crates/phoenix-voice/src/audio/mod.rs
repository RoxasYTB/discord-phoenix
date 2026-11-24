pub mod dsp;
#[cfg(feature = "opus")]
pub mod mixer;
#[cfg(feature = "audio")]
pub mod device;
#[cfg(feature = "audio")]
pub mod pulse;
#[cfg(feature = "audio")]
pub mod effects;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use std::sync::{Mutex, MutexGuard};

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn load_f32(a: &AtomicU32) -> f32 {
    f32::from_bits(a.load(Ordering::Relaxed))
}

fn store_f32(a: &AtomicU32, v: f32) {
    a.store(v.to_bits(), Ordering::Relaxed);
}

pub struct AudioControl {
    muted: AtomicBool,
    deafened: AtomicBool,
    input_gain: AtomicU32,
    output_gain: AtomicU32,
    gate_automatic: AtomicBool,
    gate_threshold_db: AtomicU32,
    input_level_db: AtomicU32,
    user_volume: Mutex<HashMap<u64, f32>>,
    noise_suppression: AtomicBool,
    agc: AtomicBool,
    echo_cancellation: AtomicBool,
    push_to_talk: AtomicBool,
    ptt_pressed: AtomicBool,
    ptt_release_ms: AtomicU32,
    transmitting: AtomicBool,
}

impl Default for AudioControl {
    fn default() -> Self {
        Self {
            muted: AtomicBool::new(false),
            deafened: AtomicBool::new(false),
            input_gain: AtomicU32::new(1.0f32.to_bits()),
            output_gain: AtomicU32::new(1.0f32.to_bits()),
            gate_automatic: AtomicBool::new(true),
            gate_threshold_db: AtomicU32::new((-50.0f32).to_bits()),
            input_level_db: AtomicU32::new((-100.0f32).to_bits()),
            user_volume: Mutex::new(HashMap::new()),
            noise_suppression: AtomicBool::new(true),
            agc: AtomicBool::new(true),
            echo_cancellation: AtomicBool::new(true),
            push_to_talk: AtomicBool::new(false),
            ptt_pressed: AtomicBool::new(false),
            ptt_release_ms: AtomicU32::new(20),
            transmitting: AtomicBool::new(false),
        }
    }
}

impl AudioControl {
    pub fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }
    pub fn set_muted(&self, v: bool) {
        self.muted.store(v, Ordering::Relaxed);
    }
    pub fn deafened(&self) -> bool {
        self.deafened.load(Ordering::Relaxed)
    }
    pub fn set_deafened(&self, v: bool) {
        self.deafened.store(v, Ordering::Relaxed);
    }
    pub fn input_gain(&self) -> f32 {
        load_f32(&self.input_gain)
    }
    pub fn set_input_gain(&self, v: f32) {
        store_f32(&self.input_gain, v.clamp(0.0, 2.0));
    }
    pub fn output_gain(&self) -> f32 {
        load_f32(&self.output_gain)
    }
    pub fn set_output_gain(&self, v: f32) {
        store_f32(&self.output_gain, v.clamp(0.0, 2.0));
    }
    pub fn gate_automatic(&self) -> bool {
        self.gate_automatic.load(Ordering::Relaxed)
    }
    pub fn gate_threshold_db(&self) -> f32 {
        load_f32(&self.gate_threshold_db)
    }
    pub fn set_gate(&self, threshold_db: Option<f32>) {
        self.gate_automatic.store(threshold_db.is_none(), Ordering::Relaxed);
        if let Some(t) = threshold_db {
            store_f32(&self.gate_threshold_db, t.clamp(-100.0, 0.0));
        }
    }
    pub fn input_level_db(&self) -> f32 {
        load_f32(&self.input_level_db)
    }
    pub fn set_input_level_db(&self, v: f32) {
        store_f32(&self.input_level_db, v);
    }
    pub fn user_volume(&self, user: u64) -> f32 {
        lock(&self.user_volume).get(&user).copied().unwrap_or(1.0)
    }
    pub fn set_user_volume(&self, user: u64, v: f32) {
        lock(&self.user_volume).insert(user, v.clamp(0.0, 2.0));
    }
    pub fn noise_suppression(&self) -> bool {
        self.noise_suppression.load(Ordering::Relaxed)
    }
    pub fn set_noise_suppression(&self, v: bool) {
        self.noise_suppression.store(v, Ordering::Relaxed);
    }
    pub fn agc(&self) -> bool {
        self.agc.load(Ordering::Relaxed)
    }
    pub fn set_agc(&self, v: bool) {
        self.agc.store(v, Ordering::Relaxed);
    }
    pub fn echo_cancellation(&self) -> bool {
        self.echo_cancellation.load(Ordering::Relaxed)
    }
    pub fn set_echo_cancellation(&self, v: bool) {
        self.echo_cancellation.store(v, Ordering::Relaxed);
    }
    pub fn push_to_talk(&self) -> bool {
        self.push_to_talk.load(Ordering::Relaxed)
    }
    pub fn set_push_to_talk(&self, v: bool) {
        self.push_to_talk.store(v, Ordering::Relaxed);
    }
    pub fn ptt_pressed(&self) -> bool {
        self.ptt_pressed.load(Ordering::Relaxed)
    }
    pub fn set_ptt_pressed(&self, v: bool) {
        self.ptt_pressed.store(v, Ordering::Relaxed);
    }
    pub fn ptt_release_ms(&self) -> u32 {
        self.ptt_release_ms.load(Ordering::Relaxed)
    }
    pub fn set_ptt_release_ms(&self, v: u32) {
        self.ptt_release_ms.store(v.min(2000), Ordering::Relaxed);
    }
    pub fn transmitting(&self) -> bool {
        self.transmitting.load(Ordering::Relaxed)
    }
    pub fn set_transmitting(&self, v: bool) {
        self.transmitting.store(v, Ordering::Relaxed);
    }
}

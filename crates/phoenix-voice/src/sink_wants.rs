use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::{Map, Value, json};

use crate::video_rx::Layer;

pub const BUDGET: (u32, u32) = (1280, 720);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rung {
    pub want: u32,
    pub width: u32,
    pub height: u32,
}

impl Rung {
    pub fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

#[derive(Debug, Clone)]
pub struct Ladder {
    budget: u64,
    rungs: Vec<Rung>,
}

impl Default for Ladder {
    fn default() -> Self {
        Self::new(BUDGET.0, BUDGET.1)
    }
}

impl Ladder {
    pub fn new(width: u32, height: u32) -> Self {
        let budget = width as u64 * height as u64;
        let ok = |v: u32| matches!(v % 16, 0 | 4 | 8 | 10);
        let sizes: Vec<(u32, u32)> = (1..4096u32).filter(|h| (16 * h) % 9 == 0).map(|h| (16 * h / 9, h)).filter(|&(w, h)| ok(w) && ok(h)).collect();
        let mut rungs = Vec::new();
        let mut last_w = 0;
        let mut want = 100i32;
        for count in 1..=25u64 {
            let fit = sizes.iter().take_while(|&&(w, h)| w as u64 * h as u64 * count <= budget).last().copied().unwrap_or((0, 0));
            if fit.0 != last_w {
                rungs.push(Rung { want: want.max(0) as u32, width: fit.0, height: fit.1 });
                want -= 10;
                last_w = fit.0;
            }
        }
        rungs.retain(|r| r.want != 0);
        rungs.sort_by_key(|r| r.want);
        Self { budget, rungs }
    }

    pub fn rungs(&self) -> &[Rung] {
        &self.rungs
    }

    pub fn max_sink_value(&self, participants: usize, pixels: u64) -> u32 {
        let Some(first) = self.rungs.first() else { return 100 };
        let mut n = first.want;
        for r in &self.rungs {
            if pixels > 0 && pixels < r.pixels() {
                n = r.want;
                break;
            }
            if r.pixels() * participants as u64 > self.budget {
                break;
            }
            n = r.want;
        }
        n
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CallView {
    pub pixels: HashMap<u64, u64>,
    pub selected: Option<u64>,
    pub others: usize,
    pub local_mutes: HashSet<u64>,
    pub video_disabled: HashSet<u64>,
    pub incoming_video: bool,
    pub pip: bool,
}

pub type Wants = BTreeMap<String, u32>;

pub fn any(n: u32) -> Wants {
    BTreeMap::from([("any".to_string(), n)])
}

pub struct SinkWants {
    ladder: Ladder,
    me: u64,
    video: BTreeMap<u64, Vec<Layer>>,
    audio: BTreeMap<u64, u32>,
    view: CallView,
    latest: Wants,
}

impl SinkWants {
    pub fn new(me: u64) -> Self {
        Self { ladder: Ladder::default(), me, video: BTreeMap::new(), audio: BTreeMap::new(), view: CallView { incoming_video: true, ..Default::default() }, latest: any(100) }
    }

    pub fn set_video(&mut self, user: u64, layers: &[Layer]) {
        if user == self.me {
            return;
        }
        if layers.is_empty() {
            self.video.remove(&user);
        } else {
            self.video.insert(user, layers.to_vec());
        }
    }

    pub fn set_audio(&mut self, user: u64, ssrc: u32) {
        if user == self.me {
            return;
        }
        if ssrc > 0 {
            self.audio.insert(user, ssrc);
        } else {
            self.audio.remove(&user);
        }
    }

    pub fn remove_user(&mut self, user: u64) {
        self.video.remove(&user);
        self.audio.remove(&user);
    }

    pub fn set_view(&mut self, view: CallView) {
        self.view = view;
    }

    fn receives(&self, user: u64) -> bool {
        self.view.incoming_video && !self.view.video_disabled.contains(&user)
    }

    pub fn compute(&self) -> Wants {
        let count = self.video.keys().filter(|u| !self.view.video_disabled.contains(u)).count();
        let n = self.ladder.max_sink_value(count, 0);
        let mut w = any(n);
        let one_to_one = self.view.others == 1;
        for (&user, layers) in &self.video {
            let pixels = self.view.pixels.get(&user).copied().unwrap_or(0);
            let c = self.ladder.max_sink_value(count, pixels);
            if !self.receives(user) {
                for l in layers {
                    w.insert(l.ssrc.to_string(), 0);
                }
                continue;
            }
            let selected = self.view.selected == Some(user) && n != 100 && !self.view.pip;
            if layers.len() > 1 {
                for l in layers {
                    let v = match (l.quality == 100, selected) {
                        (true, true) => Some(100),
                        (true, false) | (false, true) => Some(0),
                        (false, false) => one_to_one.then_some(c),
                    };
                    if let Some(v) = v {
                        w.insert(l.ssrc.to_string(), v);
                    }
                }
            } else if let Some(l) = layers.first() {
                if selected {
                    w.insert(l.ssrc.to_string(), 100);
                } else if one_to_one {
                    w.insert(l.ssrc.to_string(), c);
                }
            }
        }
        for (user, &ssrc) in &self.audio {
            if self.view.local_mutes.contains(user) {
                w.insert(ssrc.to_string(), 0);
            }
        }
        w
    }

    pub fn update(&mut self) -> Option<Wants> {
        let w = self.compute();
        if w == self.latest {
            return None;
        }
        self.latest = w.clone();
        Some(w)
    }
}

pub fn wants_json(w: &Wants) -> Value {
    Value::Object(w.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<Map<_, _>>())
}

#[derive(Debug, Default)]
pub struct GoLiveWants {
    ssrc: Option<u32>,
    size: (u32, u32),
    zoom: f32,
    incoming_video: bool,
    latest: Option<Value>,
}

impl GoLiveWants {
    pub fn new() -> Self {
        Self { zoom: 1.0, incoming_video: true, ..Default::default() }
    }

    pub fn set_stream(&mut self, layers: &[Layer]) {
        self.ssrc = layers.first().map(|l| l.ssrc);
    }

    pub fn set_size(&mut self, width: u32, height: u32, zoom: f32) {
        self.size = (width, height);
        self.zoom = zoom;
    }

    pub fn set_incoming_video(&mut self, on: bool) {
        self.incoming_video = on;
    }

    pub fn compute(&self) -> Option<Value> {
        let key = self.ssrc?.to_string();
        let mut m = Map::new();
        if !self.incoming_video {
            m.insert(key, json!(0));
            return Some(Value::Object(m));
        }
        let px = (self.size.0 as f64 * self.size.1 as f64 * self.zoom as f64 * self.zoom as f64).floor() as u64;
        let mut counts = Map::new();
        counts.insert(key.clone(), json!(px));
        m.insert(key, json!(100));
        m.insert("pixelCounts".into(), Value::Object(counts));
        Some(Value::Object(m))
    }

    pub fn update(&mut self) -> Option<Value> {
        let w = self.compute()?;
        if self.latest.as_ref() == Some(&w) {
            return None;
        }
        self.latest = Some(w.clone());
        Some(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(ssrc: u32, quality: u32) -> Layer {
        Layer { ssrc, rtx_ssrc: ssrc + 1, quality }
    }

    #[test]
    fn ladder_matches_the_client_table() {
        let l = Ladder::default();
        let r: Vec<(u32, u32, u32)> = l.rungs().iter().map(|r| (r.want, r.width, r.height)).collect();
        assert_eq!(r.last(), Some(&(100, 1280, 720)));
        assert_eq!(r.iter().rev().nth(1), Some(&(90, 896, 504)));
        assert!(r.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1 < w[1].1));
        assert!(r.iter().all(|&(want, w, h)| want > 0 && want % 10 == 0 && w * 9 == h * 16));
        assert_eq!(l.max_sink_value(0, 0), 100);
        assert_eq!(l.max_sink_value(1, 0), 100);
        assert_eq!(l.max_sink_value(2, 0), 90);
        assert!(l.max_sink_value(9, 0) < l.max_sink_value(4, 0));
        assert_eq!(l.max_sink_value(1, 320 * 180), l.rungs().iter().find(|r| r.pixels() > 320 * 180).unwrap().want);
        assert_eq!(l.max_sink_value(1, 4000 * 4000), 100);
    }

    #[test]
    fn one_camera_needs_no_op15_and_more_lower_the_any_level() {
        let mut s = SinkWants::new(1);
        s.set_video(2, &[layer(20, 100)]);
        s.set_view(CallView { others: 3, incoming_video: true, ..Default::default() });
        assert_eq!(s.update(), None);
        s.set_video(3, &[layer(30, 100)]);
        let w = s.update().unwrap();
        assert_eq!(w, any(90));
        assert_eq!(wants_json(&w), json!({"any": 90}));
        assert_eq!(s.update(), None);
    }

    #[test]
    fn focused_user_gets_full_quality_and_simulcast_picks_a_layer() {
        let mut s = SinkWants::new(1);
        s.set_video(2, &[layer(20, 50), layer(22, 100)]);
        s.set_video(3, &[layer(30, 100)]);
        s.set_view(CallView { others: 3, selected: Some(2), incoming_video: true, ..Default::default() });
        let w = s.compute();
        assert_eq!(w.get("any"), Some(&90));
        assert_eq!(w.get("22"), Some(&100));
        assert_eq!(w.get("20"), Some(&0));
        assert_eq!(w.get("30"), None);
        s.set_view(CallView { others: 3, incoming_video: true, ..Default::default() });
        let w = s.compute();
        assert_eq!(w.get("22"), Some(&0));
        assert_eq!(w.get("20"), None);
        s.set_view(CallView { others: 3, selected: Some(2), pip: true, incoming_video: true, ..Default::default() });
        assert_eq!(s.compute().get("22"), Some(&0));
    }

    #[test]
    fn one_to_one_follows_the_tile_size() {
        let l = Ladder::default();
        let mut s = SinkWants::new(1);
        s.set_video(2, &[layer(20, 100)]);
        s.set_view(CallView { others: 1, pixels: HashMap::from([(2, 400 * 225)]), incoming_video: true, ..Default::default() });
        assert_eq!(s.compute().get("20"), Some(&l.max_sink_value(1, 400 * 225)));
        s.set_view(CallView { others: 1, pixels: HashMap::from([(2, 1920 * 1080)]), incoming_video: true, ..Default::default() });
        assert_eq!(s.compute().get("20"), Some(&100));
    }

    #[test]
    fn local_mutes_and_disabled_video_send_zero() {
        let mut s = SinkWants::new(1);
        s.set_audio(2, 200);
        s.set_audio(1, 100);
        s.set_video(3, &[layer(30, 100)]);
        s.set_video(1, &[layer(10, 100)]);
        s.set_view(CallView { others: 2, local_mutes: HashSet::from([2]), video_disabled: HashSet::from([3]), incoming_video: true, ..Default::default() });
        let w = s.compute();
        assert_eq!(w.get("200"), Some(&0));
        assert_eq!(w.get("30"), Some(&0));
        assert_eq!(w.get("10"), None);
        assert_eq!(w.get("100"), None);
        s.set_view(CallView { others: 2, incoming_video: false, ..Default::default() });
        assert_eq!(s.compute().get("30"), Some(&0));
        s.remove_user(3);
        assert_eq!(s.compute(), any(100));
    }

    #[test]
    fn go_live_requests_the_stream_with_its_pixel_count() {
        let mut g = GoLiveWants::new();
        assert_eq!(g.update(), None);
        g.set_stream(&[layer(77, 100)]);
        assert_eq!(g.update(), Some(json!({"77": 100, "pixelCounts": {"77": 0}})));
        g.set_size(1280, 720, 1.0);
        assert_eq!(g.update(), Some(json!({"77": 100, "pixelCounts": {"77": 921600}})));
        assert_eq!(g.update(), None);
        g.set_size(1280, 720, 2.0);
        assert_eq!(g.compute(), Some(json!({"77": 100, "pixelCounts": {"77": 3686400}})));
        g.set_incoming_video(false);
        assert_eq!(g.compute(), Some(json!({"77": 0})));
    }
}

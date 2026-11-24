use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Codec {
    #[serde(rename = "opus")]
    Opus,
    #[serde(rename = "AV1")]
    Av1,
    #[serde(rename = "H264")]
    H264,
    #[serde(rename = "VP8")]
    Vp8,
    #[serde(rename = "VP9")]
    Vp9,
    #[serde(rename = "H265")]
    H265,
}

impl Codec {
    pub fn name(self) -> &'static str {
        match self {
            Codec::Opus => "opus",
            Codec::Av1 => "AV1",
            Codec::H264 => "H264",
            Codec::Vp8 => "VP8",
            Codec::Vp9 => "VP9",
            Codec::H265 => "H265",
        }
    }

    pub fn from_name(s: &str) -> Option<Codec> {
        Some(match s.to_ascii_uppercase().as_str() {
            "OPUS" => Codec::Opus,
            "AV1" | "AV1X" => Codec::Av1,
            "H264" => Codec::H264,
            "VP8" => Codec::Vp8,
            "VP9" => Codec::Vp9,
            "H265" | "HEVC" => Codec::H265,
            _ => return None,
        })
    }

    pub fn sendable(self) -> bool {
        matches!(self, Codec::Av1 | Codec::H264 | Codec::Vp8)
    }

    pub fn is_video(self) -> bool {
        !matches!(self, Codec::Opus)
    }

    pub fn payload_type(self) -> u8 {
        match self {
            Codec::Opus => 120,
            Codec::Av1 => 105,
            Codec::H264 => 101,
            Codec::Vp8 => 103,
            Codec::Vp9 => 107,
            Codec::H265 => 109,
        }
    }

    pub fn rtx_payload_type(self) -> Option<u8> {
        self.is_video().then(|| self.payload_type() + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Profile {
    ConstrainedBaseline,
    Main,
}

impl H264Profile {
    pub fn profile_level_id(self) -> &'static str {
        match self {
            H264Profile::ConstrainedBaseline => "42e01f",
            H264Profile::Main => "4d001f",
        }
    }

    pub fn apple_safe(profile_idc: u8, level_idc: u8) -> bool {
        matches!(profile_idc, 66 | 77) && level_idc <= 51
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocalCapabilities {
    pub hw_av1_encoder: bool,
    pub h264_encoder: bool,
    pub vp8_encoder: bool,
    pub decoders: Vec<Codec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodecOffer {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub priority: u32,
    pub payload_type: u8,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rtx_payload_type: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub encode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub decode: Option<bool>,
}

impl LocalCapabilities {
    pub fn send_preference(&self) -> Vec<Codec> {
        let mut v = Vec::new();
        if self.hw_av1_encoder {
            v.push(Codec::Av1);
        }
        if self.h264_encoder {
            v.push(Codec::H264);
        }
        if self.vp8_encoder {
            v.push(Codec::Vp8);
        }
        v
    }

    pub fn offer(&self) -> Vec<CodecOffer> {
        let mut out = vec![CodecOffer {
            name: "opus".into(),
            kind: "audio".into(),
            priority: 1000,
            payload_type: Codec::Opus.payload_type(),
            rtx_payload_type: None,
            encode: None,
            decode: None,
        }];
        let send = self.send_preference();
        let mut all: Vec<Codec> = send.clone();
        for d in &self.decoders {
            if d.is_video() && !all.contains(d) {
                all.push(*d);
            }
        }
        for (i, c) in all.iter().enumerate() {
            let encode = send.contains(c) && c.sendable();
            out.push(CodecOffer {
                name: c.name().into(),
                kind: "video".into(),
                priority: 1000 * (i as u32 + 1),
                payload_type: c.payload_type(),
                rtx_payload_type: c.rtx_payload_type(),
                encode: Some(encode),
                decode: Some(self.decoders.contains(c)),
            });
        }
        out
    }
}

pub fn select_send_codec(local: &LocalCapabilities, remote_accepts: &[Codec]) -> Option<Codec> {
    local
        .send_preference()
        .into_iter()
        .find(|c| c.sendable() && remote_accepts.contains(c))
}

pub fn fallback_after(local: &LocalCapabilities, remote_accepts: &[Codec], current: Codec) -> Option<Codec> {
    let pref = local.send_preference();
    let pos = pref.iter().position(|c| *c == current)?;
    pref.into_iter()
        .skip(pos + 1)
        .find(|c| c.sendable() && remote_accepts.contains(c))
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoEncoderConfig {
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate_bps: u32,
    pub h264_profile: Option<H264Profile>,
    pub keyframe_interval: u32,
}

impl VideoEncoderConfig {
    pub fn new(codec: Codec, width: u32, height: u32, fps: u32, bitrate_bps: u32, screen: bool) -> Self {
        let h264_profile = (codec == Codec::H264).then_some(if screen {
            H264Profile::Main
        } else {
            H264Profile::ConstrainedBaseline
        });
        VideoEncoderConfig {
            codec,
            width: width & !1,
            height: height & !1,
            fps,
            bitrate_bps,
            h264_profile,
            keyframe_interval: fps.saturating_mul(4),
        }
    }
}

pub fn split_bitrate(total_bps: u32, camera_active: bool, screen_active: bool) -> (u32, u32) {
    match (camera_active, screen_active) {
        (true, true) => {
            let screen = (total_bps as u64 * 2 / 3) as u32;
            (total_bps - screen, screen)
        }
        (true, false) => (total_bps, 0),
        (false, true) => (0, total_bps),
        (false, false) => (0, 0),
    }
}

pub fn sps_profile_level(sps: &[u8]) -> Option<(u8, u8)> {
    if sps.len() < 4 || sps[0] & 0x1f != 7 {
        return None;
    }
    Some((sps[1], sps[3]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(av1: bool) -> LocalCapabilities {
        LocalCapabilities {
            hw_av1_encoder: av1,
            h264_encoder: true,
            vp8_encoder: true,
            decoders: vec![Codec::Av1, Codec::H264, Codec::Vp8, Codec::Vp9, Codec::H265],
        }
    }

    #[test]
    fn h265_never_selected() {
        let all = [Codec::H265, Codec::Vp9, Codec::H264, Codec::Vp8, Codec::Av1];
        for av1 in [false, true] {
            let c = caps(av1);
            assert_ne!(select_send_codec(&c, &all), Some(Codec::H265));
            assert_eq!(select_send_codec(&c, &[Codec::H265]), None);
            assert!(!c.send_preference().contains(&Codec::H265));
        }
    }

    #[test]
    fn h265_offer_is_decode_only() {
        let o = caps(true).offer();
        let h265 = o.iter().find(|o| o.name == "H265").expect("decode entry");
        assert_eq!(h265.encode, Some(false));
        assert_eq!(h265.decode, Some(true));
        assert_eq!(o.iter().find(|o| o.name == "VP9").and_then(|o| o.encode), Some(false));
    }

    #[test]
    fn preference_order() {
        let all = [Codec::Vp8, Codec::H264, Codec::Av1];
        assert_eq!(select_send_codec(&caps(true), &all), Some(Codec::Av1));
        assert_eq!(select_send_codec(&caps(false), &all), Some(Codec::H264));
        assert_eq!(select_send_codec(&caps(false), &[Codec::Vp8]), Some(Codec::Vp8));
        assert_eq!(fallback_after(&caps(true), &all, Codec::Av1), Some(Codec::H264));
        assert_eq!(fallback_after(&caps(true), &all, Codec::H264), Some(Codec::Vp8));
        assert_eq!(fallback_after(&caps(true), &all, Codec::Vp8), None);
    }

    #[test]
    fn mac_safe_profiles() {
        let cam = VideoEncoderConfig::new(Codec::H264, 1281, 721, 30, 1_000_000, false);
        assert_eq!(cam.h264_profile.map(|p| p.profile_level_id()), Some("42e01f"));
        assert_eq!((cam.width, cam.height), (1280, 720));
        let scr = VideoEncoderConfig::new(Codec::H264, 1920, 1080, 30, 2_000_000, true);
        assert_eq!(scr.h264_profile.map(|p| p.profile_level_id()), Some("4d001f"));
        assert!(H264Profile::apple_safe(0x42, 0x1f));
        assert!(H264Profile::apple_safe(0x4d, 0x1f));
        assert!(!H264Profile::apple_safe(100, 0x1f));
        assert!(!H264Profile::apple_safe(110, 0x1f));
        assert_eq!(sps_profile_level(&[0x67, 0x42, 0xe0, 0x1f]), Some((0x42, 0x1f)));
    }

    #[test]
    fn bitrate_split() {
        assert_eq!(split_bitrate(3_000_000, true, true), (1_000_000, 2_000_000));
        assert_eq!(split_bitrate(3_000_000, true, false), (3_000_000, 0));
    }
}

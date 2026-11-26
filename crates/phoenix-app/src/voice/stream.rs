#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    #[default]
    Video,
    Documents,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Screen,
    Window,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality {
    pub preset: Preset,
    pub resolution: u32,
    pub fps: u32,
    pub sound: bool,
}

impl Default for Quality {
    fn default() -> Self {
        Self { preset: Preset::Video, resolution: 720, fps: 30, sound: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Need {
    None,
    High,
    Mid(u8),
}

const TABLE: [(u32, u32, Need); 18] = [
    (0, 60, Need::High),
    (0, 30, Need::High),
    (0, 15, Need::High),
    (0, 5, Need::None),
    (1440, 60, Need::Mid(2)),
    (1440, 30, Need::Mid(2)),
    (1440, 15, Need::Mid(2)),
    (1080, 60, Need::Mid(2)),
    (1080, 30, Need::Mid(2)),
    (1080, 15, Need::Mid(2)),
    (720, 60, Need::Mid(1)),
    (720, 30, Need::None),
    (720, 15, Need::None),
    (720, 5, Need::None),
    (480, 60, Need::Mid(1)),
    (480, 30, Need::None),
    (480, 15, Need::None),
    (480, 5, Need::None),
];

pub const RESOLUTIONS: [u32; 5] = [480, 720, 1080, 1440, 0];
pub const FRAME_RATES: [u32; 3] = [15, 30, 60];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Perks {
    pub nitro: bool,
    pub guild_tier: u8,
}

pub fn allowed(perks: Perks, resolution: u32, fps: u32, documents: bool) -> bool {
    TABLE.iter().any(|&(r, f, need)| {
        r == resolution
            && f == fps
            && match need {
                Need::None => !(r == 0 && f == 5) || documents,
                Need::High => perks.nitro,
                Need::Mid(tier) => perks.nitro || perks.guild_tier >= tier,
            }
    })
}

pub fn preset_choices(preset: Preset) -> &'static [(u32, u32)] {
    match preset {
        Preset::Video => &[(1440, 60), (1080, 60), (720, 60), (720, 30)],
        Preset::Documents => &[(0, 15), (0, 5)],
        Preset::Custom => &[],
    }
}

pub fn resolve(q: Quality, perks: Perks) -> (u32, u32) {
    let documents = q.preset == Preset::Documents;
    if let Some(&(r, f)) = preset_choices(q.preset).iter().find(|&&(r, f)| allowed(perks, r, f, documents)) {
        return (r, f);
    }
    let fps = if documents { 30 } else { q.fps };
    if allowed(perks, q.resolution, fps, documents) {
        return (q.resolution, fps);
    }
    TABLE
        .iter()
        .find(|&&(r, f, _)| f == fps && allowed(perks, r, f, documents) && r != 0)
        .map_or((720, 30), |&(r, f, _)| (r, f))
}

pub fn frame_size(resolution: u32, source: Option<(u32, u32)>) -> (u32, u32) {
    let (sw, sh) = source.filter(|&(w, h)| w > 0 && h > 0).unwrap_or((1920, 1080));
    let h = if resolution == 0 { sh } else { resolution.min(sh) };
    let w = ((u64::from(h) * u64::from(sw) / u64::from(sh.max(1))) as u32).max(2);
    (w & !1, h & !1)
}

pub fn bitrate(resolution: u32, fps: u32) -> u32 {
    let px = match resolution {
        0 | 1440 => 2560 * 1440,
        1080 => 1920 * 1080,
        720 => 1280 * 720,
        _ => 854 * 480,
    } as f64;
    let bpp = if fps >= 60 { 0.07 } else if fps >= 30 { 0.09 } else { 0.14 };
    ((px * f64::from(fps.max(5)) * bpp) as u32).clamp(1_000_000, 8_000_000)
}

pub fn key(guild: Option<u64>, channel: u64, owner: u64) -> String {
    match guild {
        Some(g) => format!("guild:{g}:{channel}:{owner}"),
        None => format!("call:{channel}:{owner}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub guild: Option<u64>,
    pub channel: u64,
    pub owner: u64,
}

pub fn parse_key(k: &str) -> Option<Key> {
    let p: Vec<&str> = k.split(':').collect();
    match p.as_slice() {
        ["guild", g, c, o] => Some(Key { guild: Some(g.parse().ok()?), channel: c.parse().ok()?, owner: o.parse().ok()? }),
        ["call", c, o] => Some(Key { guild: None, channel: c.parse().ok()?, owner: o.parse().ok()? }),
        _ => None,
    }
}

pub const PREVIEW_MAX: (u32, u32) = (512, 288);
pub const PREVIEW_FIRST_MS: u64 = 500;
pub const PREVIEW_EVERY_MS: u64 = 300_000;

pub const NOTIFY_GUILD_MAX: u32 = 50;

static HIDE_PREVIEW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn hide_preview() -> bool {
    HIDE_PREVIEW.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_hide_preview(on: bool) {
    HIDE_PREVIEW.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn notify_option_shown(guild: bool, members: Option<u32>) -> bool {
    !guild || members.is_none_or(|n| n > 30 && n <= NOTIFY_GUILD_MAX)
}

pub const PREVIEW_RETRY_MS: u64 = 60_000;

pub fn is_black(rgba: &[u8]) -> bool {
    rgba.as_chunks::<4>().0.iter().all(|p| p[0] == 0 && p[1] == 0 && p[2] == 0)
}

pub fn preview_jpeg(width: u32, height: u32, rgba: &[u8]) -> Option<String> {
    use base64::Engine;
    let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())?;
    let scale = (PREVIEW_MAX.0 as f32 / width as f32).min(PREVIEW_MAX.1 as f32 / height as f32);
    let (w, h) = (((width as f32 * scale).round() as u32).max(1), ((height as f32 * scale).round() as u32).max(1));
    let small = image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle);
    let rgb = image::DynamicImage::ImageRgba8(small).to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92).encode_image(&rgb).ok()?;
    Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn notify_friends_only_for_small_servers() {
        assert!(super::notify_option_shown(false, None));
        assert!(super::notify_option_shown(true, None));
        assert!(!super::notify_option_shown(true, Some(30)));
        assert!(super::notify_option_shown(true, Some(31)));
        assert!(super::notify_option_shown(true, Some(50)));
        assert!(!super::notify_option_shown(true, Some(51)));
    }

    use super::*;

    #[test]
    fn presets_follow_the_client_table() {
        let free = Perks::default();
        let nitro = Perks { nitro: true, guild_tier: 0 };
        assert_eq!(resolve(Quality::default(), free), (720, 30));
        assert_eq!(resolve(Quality::default(), nitro), (1440, 60));
        assert_eq!(resolve(Quality { preset: Preset::Video, ..Default::default() }, Perks { nitro: false, guild_tier: 1 }), (720, 60));
        assert_eq!(resolve(Quality { preset: Preset::Documents, ..Default::default() }, nitro), (0, 15));
        assert_eq!(resolve(Quality { preset: Preset::Documents, ..Default::default() }, free), (0, 5));
        assert_eq!(resolve(Quality { preset: Preset::Custom, resolution: 1080, fps: 60, sound: true }, nitro), (1080, 60));
        assert_eq!(resolve(Quality { preset: Preset::Custom, resolution: 1080, fps: 60, sound: true }, free), (720, 30));
        assert!(!allowed(free, 1080, 30, false));
        assert!(allowed(Perks { nitro: false, guild_tier: 2 }, 1080, 30, false));
    }

    #[test]
    fn stream_keys_round_trip() {
        assert_eq!(key(Some(1), 2, 3), "guild:1:2:3");
        assert_eq!(parse_key("guild:1:2:3"), Some(Key { guild: Some(1), channel: 2, owner: 3 }));
        assert_eq!(parse_key("call:2:3"), Some(Key { guild: None, channel: 2, owner: 3 }));
        assert_eq!(parse_key("nope"), None);
    }

    #[test]
    fn frame_sizes_keep_the_source_aspect() {
        assert_eq!(frame_size(720, Some((1920, 1080))), (1280, 720));
        assert_eq!(frame_size(0, Some((2560, 1440))), (2560, 1440));
        assert_eq!(frame_size(1080, None), (1920, 1080));
    }

    #[test]
    fn preview_is_a_small_jpeg() {
        let rgba = vec![200u8; 64 * 36 * 4];
        let url = preview_jpeg(64, 36, &rgba).unwrap();
        assert!(url.starts_with("data:image/jpeg;base64,"));
        assert!(is_black(&[0, 0, 0, 255, 0, 0, 0, 255]));
        assert!(!is_black(&rgba));
    }
}

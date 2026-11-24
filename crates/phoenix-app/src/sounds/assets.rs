use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use phoenix_voice::audio::effects::Pcm;

pub const KNOWN: &[(&str, &str)] = &[
    ("activity_end", "878f7e7f2b3bf972.mp3"),
    ("activity_launch", "a71f183dc4de32a3.mp3"),
    ("activity_user_join", "31d2551c00b7c91e.mp3"),
    ("activity_user_left", "9f34f40112215f5e.mp3"),
    ("asmr_message1", "7abbc45d8e7b179a.mp3"),
    ("bit_message1", "795109c70fe6ea7b.mp3"),
    ("bop_message1", "909464bf55b13935.mp3"),
    ("call_calling", "5146737af413d88e.mp3"),
    ("call_ringing", "c2a7111bb44b8da0.mp3"),
    ("call_ringing_beat", "574220f72e718dad.mp3"),
    ("call_ringing_snow_halation", "82954088613583bc.mp3"),
    ("call_ringing_snowsgiving", "4261355196740dab.mp3"),
    ("camera_off", "b1dd204729fe78b4.mp3"),
    ("camera_on", "855607d0932ea396.mp3"),
    ("clip_error", "a95d4944eee85a14.mp3"),
    ("clip_save", "fece0a7646748c01.mp3"),
    ("ddr-down", "c51fe62d8439bbb5.mp3"),
    ("ddr-left", "585effa1b1c0adb6.mp3"),
    ("ddr-right", "812a423084f53b4e.mp3"),
    ("ddr-up", "7d9fdcf720530f49.mp3"),
    ("deafen", "529ff198eac567af.mp3"),
    ("discodo", "cb85be48267eb1b4.mp3"),
    ("disconnect", "dd0911025fe00b2e.mp3"),
    ("doggo_bark", "e692934b5721e2f4.mp3"),
    ("doggo_cat", "2a8fc918410f139d.mp3"),
    ("doggo_cat_hurt", "0d0e5a71d7cdfcb1.mp3"),
    ("doggo_death", "c55e2628bba98a03.mp3"),
    ("doggo_dog_death", "76d25c608009a255.mp3"),
    ("doggo_dog_hurt", "93efbbc082ecbea8.mp3"),
    ("doggo_human_hit", "ee0371f4f3747ef4.mp3"),
    ("doggo_music", "b76542545f68b644.mp3"),
    ("doggo_rat", "e3c2784c5e48d890.mp3"),
    ("doggo_rat_hit", "9b1f59a879cc2b9c.mp3"),
    ("doggo_skeleton", "84078aa3f23823e6.mp3"),
    ("doggo_skeleton_die", "d0bc5c2deff53a23.mp3"),
    ("doggo_skeleton_hit", "5a6a6325e363c734.mp3"),
    ("doggo_slash", "168441ffecdf124f.mp3"),
    ("doggo_zombie", "9ba3d6476257f26c.mp3"),
    ("doggo_zombie_hit", "5ede61a621f3041f.mp3"),
    ("ducky_message1", "7d8709c9f90d5d1e.mp3"),
    ("halloween_call_calling", "ef24e229a3413c8a.mp3"),
    ("halloween_call_ringing", "7a287e2f88531f25.mp3"),
    ("halloween_deafen", "514663b657bc4c7d.mp3"),
    ("halloween_defean", "514663b657bc4c7d.mp3"),
    ("halloween_disconnect", "9b8ef0a59d731dd6.mp3"),
    ("halloween_message1", "c269de500ca3cb7b.mp3"),
    ("halloween_mute", "d5e69d2c2aac0a27.mp3"),
    ("halloween_undeafen", "4dfa756a71484219.mp3"),
    ("halloween_undefean", "ad4d1c97cf7b8726.mp3"),
    ("halloween_unmute", "6f0d538ed066a1a6.mp3"),
    ("halloween_user_join", "00b48ea9fc787786.mp3"),
    ("halloween_user_leave", "c9b6b9d1ef992f1b.mp3"),
    ("hang_status_select", "c27233e99b338a12.mp3"),
    ("human_man", "e1a50e2d7925ce98.mp3"),
    ("lofi_message1", "5aa5ffdc82cb3c74.mp3"),
    ("mention1", "3baf62fc7aebb0a3.mp3"),
    ("mention2", "e0a0235d2fa32496.mp3"),
    ("mention3", "260dfacf63b6cb13.mp3"),
    ("message1", "3ed22d14f3c30bc4.mp3"),
    ("message2", "91f9f6fa8cc9ea5c.mp3"),
    ("message3", "4f53c2f31ea0cdd8.mp3"),
    ("mute", "2d3b4ba32c34c862.mp3"),
    ("overlayunlock", "1d05544763057d17.mp3"),
    ("poggermode_achievement_unlock", "46354b113608483f.mp3"),
    ("poggermode_applause", "16d261c6a0cb8080.mp3"),
    ("poggermode_enabled", "4b5c35a2c125196f.mp3"),
    ("poggermode_message_send", "97f3552e8c519595.mp3"),
    ("ptt_start", "f5c8ec31889c5317.mp3"),
    ("ptt_stop", "34b5969be3a063ba.mp3"),
    ("reconnect", "7dfc49b36dea3a6c.mp3"),
    ("robot_man", "b5b16d9754a8782e.mp3"),
    ("stage_waiting", "7458c5c3bc415b01.mp3"),
    ("stream_ended", "cbff07b5a83e0321.mp3"),
    ("stream_started", "abe52a3c92953edb.mp3"),
    ("stream_user_joined", "3d2af2764581bb2e.mp3"),
    ("stream_user_left", "24ae9dc0184bd570.mp3"),
    ("success", "90241c95e51f9261.mp3"),
    ("undeafen", "b150f03c89944403.mp3"),
    ("unmute", "e74c4a06134a20e4.mp3"),
    ("user_join", "b135ff6c8e091b43.mp3"),
    ("user_leave", "7b9a183742515fc2.mp3"),
    ("user_moved", "4003230e4086b680.mp3"),
    ("vibing_wumpus", "f4b4eb5a3dd2dbff.mp3"),
    ("voice_message_recording_stopped", "17e98c85a3d66341.mp3"),
    ("winter_call_calling", "a07935fe464c324c.mp3"),
    ("winter_call_ringing", "a4e89378c6268428.mp3"),
    ("winter_deafen", "70fe5e248588bb13.mp3"),
    ("winter_disconnect", "600991ad4560f288.mp3"),
    ("winter_mute", "81034fa14558e50f.mp3"),
    ("winter_undeafen", "5b518eba0a73ec4a.mp3"),
    ("winter_unmute", "b2c49178b1a4e3d9.mp3"),
    ("winter_user_join", "b766ee70384a56ce.mp3"),
    ("winter_user_leave", "cf2c36fde50eb05d.mp3"),
];

const MAGIC: [u8; 8] = [0x30, 0x5c, 0x72, 0xa7, 0x1b, 0x6d, 0xfb, 0xfc];
const EOF_MAGIC: [u8; 8] = [0xd8, 0x41, 0x0d, 0x97, 0x45, 0x6f, 0xfa, 0xf4];
const SPARSE_MAGIC: u64 = 0xeb97_bf01_6553_676b;

pub fn out_dir() -> Option<PathBuf> {
    crate::icons::out_dir().map(|d| d.with_file_name("sounds"))
}

pub fn parse_bundle(js: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(at) = js.find("\"./message1.mp3\":") else { return out };
    let start = js[..at].rfind('{').map_or(at, |i| i + 1);
    let end = js[at..].find('}').map_or(js.len(), |i| at + i);
    for pair in js[start..end].split(',') {
        let Some((k, v)) = pair.split_once(':') else { continue };
        let Some(name) = k.trim_matches('"').strip_prefix("./").and_then(|k| k.strip_suffix(".mp3")) else { continue };
        let id = v.trim_matches('"');
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        for pat in [format!("{id}(e,t,n){{\"use strict\";e.exports=n.p+\""), format!("{id}(e){{\"use strict\";e.exports=\"/assets/")] {
            let mut from = 0;
            while let Some(i) = js[from..].find(&pat) {
                let i = from + i;
                from = i + pat.len();
                if js[..i].bytes().last().is_some_and(|b| b.is_ascii_digit()) {
                    continue;
                }
                if let Some(f) = js[from..].split('"').next().filter(|f| f.ends_with(".mp3")) {
                    out.insert(name.to_owned(), f.to_owned());
                }
                break;
            }
        }
    }
    out
}

pub fn table() -> &'static HashMap<String, String> {
    static T: OnceLock<HashMap<String, String>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t: HashMap<String, String> = KNOWN.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        'caches: for cache in crate::icons::discord_caches() {
            for (_, entry) in crate::icons::cache::js_entries(&cache).into_iter().filter(|(n, _)| n.starts_with("web.")).take(4) {
                if let Some(js) = crate::icons::cache::read_body(&entry) {
                    let parsed = parse_bundle(&js);
                    if !parsed.is_empty() {
                        t.extend(parsed);
                        break 'caches;
                    }
                }
            }
        }
        t
    })
}

pub fn entry_body(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 24 || bytes[..8] != MAGIC {
        return None;
    }
    let kl = u32::from_le_bytes(bytes[12..16].try_into().ok()?) as usize;
    let mut off = 24 + kl;
    let head = |o: usize| -> Option<(u64, i64, i64)> {
        let b = bytes.get(o..o + 32)?;
        Some((
            u64::from_le_bytes(b[0..8].try_into().ok()?),
            i64::from_le_bytes(b[8..16].try_into().ok()?),
            i64::from_le_bytes(b[16..24].try_into().ok()?),
        ))
    };
    if head(off).is_some_and(|(m, _, _)| m == SPARSE_MAGIC) {
        let mut ranges = Vec::new();
        while let Some((m, o, l)) = head(off) {
            if m != SPARSE_MAGIC || o < 0 || l < 0 {
                break;
            }
            let data = bytes.get(off + 32..off + 32 + l as usize)?;
            ranges.push((o as usize, data));
            off += 32 + l as usize;
        }
        ranges.sort_by_key(|r| r.0);
        let mut out = Vec::new();
        for (o, d) in ranges {
            if o != out.len() {
                return None;
            }
            out.extend_from_slice(d);
        }
        return (!out.is_empty()).then_some(out);
    }
    let end = memchr::memmem::find(&bytes[off..], &EOF_MAGIC).map_or(bytes.len(), |p| p + off);
    let body = &bytes[off..end];
    looks_like_mp3(body).then(|| body.to_vec())
}

fn looks_like_mp3(b: &[u8]) -> bool {
    b.starts_with(b"ID3") || (b.len() > 1 && b[0] == 0xff && b[1] & 0xe0 == 0xe0)
}

fn cached_assets() -> HashMap<String, PathBuf> {
    cached_with(".mp3")
}

fn cached_with(ext: &str) -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();
    for cache in crate::icons::discord_caches() {
        for e in std::fs::read_dir(&cache).into_iter().flatten().flatten() {
            let p = e.path();
            let mut head = [0u8; 512];
            let Ok(mut f) = std::fs::File::open(&p) else { continue };
            let n = f.read(&mut head).unwrap_or(0);
            let h = &head[..n];
            if h.len() < 24 || h[..8] != MAGIC {
                continue;
            }
            let kl = u32::from_le_bytes([h[12], h[13], h[14], h[15]]) as usize;
            let Some(key) = h.get(24..24 + kl).and_then(|k| std::str::from_utf8(k).ok()) else { continue };
            let Some(name) = key.rsplit_once("/assets/").map(|(_, n)| n.split('?').next().unwrap_or(n)) else { continue };
            if name.ends_with(ext) {
                let is_sparse = p.to_string_lossy().ends_with("_s");
                if is_sparse || !out.contains_key(name) {
                    out.insert(name.to_owned(), p);
                }
            }
        }
    }
    out
}

fn download(file: &str) -> Option<Vec<u8>> {
    let b = download_any(file)?;
    looks_like_mp3(&b).then_some(b)
}

fn download_any(file: &str) -> Option<Vec<u8>> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    rt.block_on(async {
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().ok()?;
        let r = client.get(format!("https://discord.com/assets/{file}")).send().await.ok()?;
        if !r.status().is_success() {
            return None;
        }
        Some(r.bytes().await.ok()?.to_vec())
    })
}

fn fetch_into(dir: &Path, name: &str, cached: &HashMap<String, PathBuf>) -> Option<Vec<u8>> {
    let file = table().get(name)?;
    let bytes = cached
        .get(file)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| entry_body(&b))
        .or_else(|| download(file))?;
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join(format!("{name}.mp3")), &bytes);
    Some(bytes)
}

pub fn load(name: &str) -> Option<Vec<u8>> {
    let dir = out_dir()?;
    if let Ok(b) = std::fs::read(dir.join(format!("{name}.mp3"))) {
        return Some(b);
    }
    fetch_into(&dir, name, &cached_assets())
}

pub fn asset_file(file: &str) -> Option<PathBuf> {
    let dir = out_dir()?.with_file_name("assets");
    let path = dir.join(file);
    if path.is_file() {
        return Some(path);
    }
    static PENDING: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    {
        let mut p = PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if p.iter().any(|f| f == file) {
            return None;
        }
        p.push(file.to_owned());
    }
    let file = file.to_owned();
    let _ = std::thread::Builder::new().name("asset-fetch".into()).spawn(move || {
        let ext = file.rsplit_once('.').map_or(String::new(), |(_, e)| format!(".{e}"));
        let bytes = cached_with(&ext)
            .get(&file)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| entry_body_any(&b))
            .or_else(|| download_any(&file));
        if let Some(b) = bytes {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join(&file), b);
        }
    });
    None
}

fn entry_body_any(bytes: &[u8]) -> Option<Vec<u8>> {
    if let Some(b) = entry_body(bytes) {
        return Some(b);
    }
    if bytes.len() < 24 || bytes[..8] != MAGIC {
        return None;
    }
    let kl = u32::from_le_bytes(bytes[12..16].try_into().ok()?) as usize;
    let off = 24 + kl;
    let end = memchr::memmem::find(bytes.get(off..)?, &EOF_MAGIC).map_or(bytes.len(), |p| p + off);
    let raw = &bytes[off..end];
    let mut out = Vec::new();
    if brotli_decompressor::Decompressor::new(raw, 4096).read_to_end(&mut out).is_ok() && !out.is_empty() {
        return Some(out);
    }
    (!raw.is_empty()).then(|| raw.to_vec())
}

const PRELOAD: &[&str] = &["message1", "message3", "call_ringing"];

pub fn init() {
    let Some(dir) = out_dir() else { return };
    let _ = std::thread::Builder::new().name("sound-extract".into()).spawn(move || {
        let missing: Vec<&str> = PRELOAD.iter().chain(super::VOICE_LIST).copied().filter(|n| !dir.join(format!("{n}.mp3")).is_file()).collect();
        if missing.is_empty() {
            return;
        }
        let cached = cached_assets();
        for n in missing {
            let _ = fetch_into(&dir, n, &cached);
        }
    });
}

pub fn decode(bytes: &[u8]) -> Option<Pcm> {
    use symphonia::core::{
        audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions, io::MediaSourceStream,
        meta::MetadataOptions, probe::Hint,
    };
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()).ok()?;
    let mut format = probed.format;
    let track = format.default_track()?;
    let tid = track.id;
    let mut dec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).ok()?;
    let (mut samples, mut rate, mut channels) = (Vec::new(), 0u32, 0u16);
    while let Ok(pkt) = format.next_packet() {
        if pkt.track_id() != tid {
            continue;
        }
        match dec.decode(&pkt) {
            Ok(buf) => {
                let spec = *buf.spec();
                rate = spec.rate;
                channels = spec.channels.count() as u16;
                let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                sb.copy_interleaved_ref(buf);
                samples.extend_from_slice(sb.samples());
            }
            Err(Error::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }
    (rate > 0 && channels > 0 && !samples.is_empty()).then_some(Pcm { rate, channels, samples })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_table_and_sparse_entries() {
        let js = r#"x={"./deafen.mp3":"5","./message1.mp3":"31742"};function r(e){}},5(e,t,n){"use strict";e.exports=n.p+"aa.mp3"},131742(e){},31742(e,t,n){"use strict";e.exports=n.p+"bb.mp3"}"#;
        let t = parse_bundle(js);
        assert_eq!(t.get("deafen").map(String::as_str), Some("aa.mp3"));
        assert_eq!(t.get("message1").map(String::as_str), Some("bb.mp3"));
        assert!(KNOWN.iter().any(|(n, _)| *n == "message1") && KNOWN.len() > 90);

        let key = b"1/0/https://discord.com/assets/aa.mp3";
        let mut f = MAGIC.to_vec();
        f.extend_from_slice(&9u32.to_le_bytes());
        f.extend_from_slice(&(key.len() as u32).to_le_bytes());
        f.extend_from_slice(&[0; 8]);
        f.extend_from_slice(key);
        for (o, d) in [(3i64, &b"456"[..]), (0, &b"ID3"[..])] {
            f.extend_from_slice(&SPARSE_MAGIC.to_le_bytes());
            f.extend_from_slice(&o.to_le_bytes());
            f.extend_from_slice(&(d.len() as i64).to_le_bytes());
            f.extend_from_slice(&[0; 8]);
            f.extend_from_slice(d);
        }
        assert_eq!(entry_body(&f).as_deref(), Some(&b"ID3456"[..]));
    }
}

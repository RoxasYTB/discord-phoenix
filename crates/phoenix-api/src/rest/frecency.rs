use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::Method;
use serde::Deserialize;

use super::{Body, RestClient};
use crate::error::{Error, Result};

#[derive(Clone, PartialEq, prost::Message)]
pub struct FrecencyItem {
    #[prost(uint32, tag = "1")]
    pub total_uses: u32,
    #[prost(uint64, repeated, tag = "2")]
    pub recent_uses: Vec<u64>,
    #[prost(int32, tag = "3")]
    pub frecency: i32,
    #[prost(int32, tag = "4")]
    pub score: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FavoriteGif {
    #[prost(int32, tag = "1")]
    pub format: i32,
    #[prost(string, tag = "2")]
    pub src: String,
    #[prost(uint32, tag = "3")]
    pub width: u32,
    #[prost(uint32, tag = "4")]
    pub height: u32,
    #[prost(uint32, tag = "5")]
    pub order: u32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FavoriteGifs {
    #[prost(map = "string, message", tag = "1")]
    pub gifs: HashMap<String, FavoriteGif>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StickerFrecency {
    #[prost(map = "fixed64, message", tag = "1")]
    pub stickers: HashMap<u64, FrecencyItem>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct EmojiFrecency {
    #[prost(map = "string, message", tag = "1")]
    pub emojis: HashMap<String, FrecencyItem>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ApplicationFrecency {
    #[prost(map = "string, message", tag = "1")]
    pub applications: HashMap<String, FrecencyItem>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GuildAndChannelFrecency {
    #[prost(map = "fixed64, message", tag = "1")]
    pub guild_and_channels: HashMap<u64, FrecencyItem>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FavoriteSoundboardSounds {
    #[prost(fixed64, repeated, tag = "1")]
    pub sound_ids: Vec<u64>,
    #[prost(fixed64, repeated, tag = "2")]
    pub ordered_sound_ids: Vec<u64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PlayedSoundFrecency {
    #[prost(map = "string, message", tag = "1")]
    pub played_sounds: HashMap<String, FrecencyItem>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FrecencyUserSettings {
    #[prost(message, optional, tag = "2")]
    pub favorite_gifs: Option<FavoriteGifs>,
    #[prost(message, optional, tag = "4")]
    pub sticker_frecency: Option<StickerFrecency>,
    #[prost(message, optional, tag = "6")]
    pub emoji_frecency: Option<EmojiFrecency>,
    #[prost(message, optional, tag = "8")]
    pub favorite_soundboard_sounds: Option<FavoriteSoundboardSounds>,
    #[prost(message, optional, tag = "9")]
    pub application_frecency: Option<ApplicationFrecency>,
    #[prost(message, optional, tag = "11")]
    pub played_sound_frecency: Option<PlayedSoundFrecency>,
    #[prost(message, optional, tag = "5")]
    pub favorite_emojis: Option<FavoriteEmojis>,
    #[prost(message, optional, tag = "12")]
    pub guild_and_channel_frecency: Option<GuildAndChannelFrecency>,
    #[prost(message, optional, tag = "13")]
    pub emoji_reaction_frecency: Option<EmojiFrecency>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FavoriteEmojis {
    #[prost(string, repeated, tag = "1")]
    pub emojis: Vec<String>,
}

fn rank(it: &FrecencyItem) -> (i32, u32, u64) {
    (it.score, it.total_uses, it.recent_uses.iter().copied().max().unwrap_or(0))
}

impl FrecencyUserSettings {
    pub fn favorite_sounds(&self) -> Vec<u64> {
        self.favorite_soundboard_sounds.as_ref().map(|f| f.sound_ids.clone()).unwrap_or_default()
    }

    pub fn played_sounds_ranked(&self) -> Vec<u64> {
        let mut v: Vec<(u64, &FrecencyItem)> = self
            .played_sound_frecency
            .iter()
            .flat_map(|p| p.played_sounds.iter())
            .filter_map(|(k, it)| k.parse::<u64>().ok().map(|id| (id, it)))
            .collect();
        v.sort_by(|a, b| rank(b.1).cmp(&rank(a.1)).then_with(|| a.0.cmp(&b.0)));
        v.into_iter().map(|(id, _)| id).collect()
    }

    pub fn decode_base64(s: &str) -> Result<Self> {
        let bytes = STANDARD.decode(s.trim()).map_err(|e| Error::Protocol(format!("frecency base64: {e}")))?;
        <Self as prost::Message>::decode(bytes.as_slice()).map_err(|e| Error::Protocol(format!("frecency proto: {e}")))
    }

    pub fn emojis_ranked(&self) -> Vec<String> {
        let mut v: Vec<(&String, &FrecencyItem)> = self.emoji_frecency.iter().flat_map(|e| e.emojis.iter()).collect();
        v.sort_by(|a, b| rank(b.1).cmp(&rank(a.1)).then_with(|| a.0.cmp(b.0)));
        v.into_iter().map(|(k, _)| k.clone()).collect()
    }

    pub fn stickers_ranked(&self) -> Vec<u64> {
        let mut v: Vec<(&u64, &FrecencyItem)> = self.sticker_frecency.iter().flat_map(|e| e.stickers.iter()).collect();
        v.sort_by(|a, b| rank(b.1).cmp(&rank(a.1)).then_with(|| a.0.cmp(b.0)));
        v.into_iter().map(|(k, _)| *k).collect()
    }

    pub fn favorite_gifs(&self) -> Vec<(String, FavoriteGif)> {
        let mut v: Vec<(String, FavoriteGif)> =
            self.favorite_gifs.iter().flat_map(|f| f.gifs.iter()).map(|(k, g)| (k.clone(), g.clone())).collect();
        v.sort_by_key(|g| std::cmp::Reverse(g.1.order));
        v
    }

    pub fn applications_ranked(&self) -> Vec<u64> {
        let mut v: Vec<(u64, &FrecencyItem)> = self
            .application_frecency
            .iter()
            .flat_map(|a| a.applications.iter())
            .filter_map(|(k, it)| k.parse::<u64>().ok().map(|id| (id, it)))
            .collect();
        v.sort_by(|a, b| rank(b.1).cmp(&rank(a.1)).then_with(|| a.0.cmp(&b.0)));
        v.into_iter().map(|(id, _)| id).collect()
    }
}

pub const DAY_MS: i64 = 86_400_000;
const MAX_SAMPLES: usize = 10;
const DEFAULT_EMOJIS: [&str; 10] = ["thumbsup", "eyes", "laughing", "watermelon", "fork_and_knife", "yum", "weary", "tired_face", "poop", "100"];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrecencyOrder {
    pub emojis: Vec<String>,
    pub stickers: Vec<u64>,
    pub reactions: Vec<String>,
}

fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*i)?;
        *i += 1;
        v |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

fn wire_fields(b: &[u8]) -> Vec<(u64, u8, &[u8])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let Some(tag) = varint(b, &mut i) else { break };
        let wt = (tag & 7) as u8;
        let start = i;
        let len = match wt {
            0 => {
                if varint(b, &mut i).is_none() {
                    break;
                }
                0
            }
            1 => 8,
            5 => 4,
            2 => match varint(b, &mut i) {
                Some(n) => n as usize,
                None => break,
            },
            _ => break,
        };
        let from = if wt == 0 { start } else { i };
        let to = if wt == 0 { i } else { i.saturating_add(len) };
        let Some(slice) = b.get(from..to) else { break };
        out.push((tag >> 3, wt, slice));
        i = to;
    }
    out
}

fn map_keys(b: &[u8], top: u64) -> Vec<&[u8]> {
    wire_fields(b)
        .into_iter()
        .filter(|(f, wt, _)| *f == top && *wt == 2)
        .flat_map(|(_, _, m)| wire_fields(m))
        .filter(|(f, wt, _)| *f == 1 && *wt == 2)
        .filter_map(|(_, _, entry)| wire_fields(entry).into_iter().find(|(f, _, _)| *f == 1).map(|(_, _, k)| k))
        .collect()
}

pub fn wire_order(bytes: &[u8]) -> FrecencyOrder {
    FrecencyOrder {
        emojis: map_keys(bytes, 6).into_iter().filter_map(|k| std::str::from_utf8(k).ok().map(str::to_owned)).collect(),
        stickers: map_keys(bytes, 4).into_iter().filter_map(|k| k.try_into().ok().map(u64::from_le_bytes)).collect(),
        reactions: map_keys(bytes, 13).into_iter().filter_map(|k| std::str::from_utf8(k).ok().map(str::to_owned)).collect(),
    }
}

fn is_array_index(k: &str) -> bool {
    !k.is_empty() && (k == "0" || !k.starts_with('0')) && k.bytes().all(|c| c.is_ascii_digit()) && k.parse::<u64>().is_ok_and(|n| n < u64::from(u32::MAX))
}

pub fn js_key_order(keys: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<String> = keys.iter().filter(|k| seen.insert(k.as_str())).cloned().collect();
    let (mut ints, rest): (Vec<String>, Vec<String>) = unique.into_iter().partition(|k| is_array_index(k));
    ints.sort_by_key(|k| k.parse::<u64>().unwrap_or(0));
    ints.extend(rest);
    ints
}

fn weight(days: i64) -> f64 {
    match days {
        ..=3 => 100.,
        4..=15 => 70.,
        16..=30 => 50.,
        31..=45 => 30.,
        46..=80 => 10.,
        _ => 1.,
    }
}

pub fn client_frecency(it: &FrecencyItem, now_ms: i64) -> Option<i64> {
    let uses: Vec<i64> = it.recent_uses.iter().map(|&t| t as i64).filter(|t| *t > 0).collect();
    if uses.is_empty() {
        return None;
    }
    let score: f64 = uses.iter().take(MAX_SAMPLES).map(|&t| weight((now_ms - t) / DAY_MS)).sum();
    Some((f64::from(it.total_uses) * (score / uses.len() as f64)).ceil() as i64)
}

fn place_weight(days: i64) -> f64 {
    match days {
        0 => 100.,
        1 => 70.,
        2..=3 => 50.,
        4..=6 => 30.,
        7.. => 10.,
        _ => 1.,
    }
}

pub fn place_frecency(it: &FrecencyItem, now_ms: i64) -> Option<i64> {
    let uses: Vec<i64> = it.recent_uses.iter().map(|&t| t as i64).filter(|t| *t > 0).collect();
    if uses.is_empty() {
        return None;
    }
    let score: f64 = uses.iter().take(MAX_SAMPLES).map(|&t| place_weight((now_ms - t) / DAY_MS)).sum();
    Some((f64::from(it.total_uses) * (score / uses.len() as f64)).ceil() as i64)
}

pub fn reaction_frecency(it: &FrecencyItem, max_total_uses: u32, now_ms: i64) -> Option<i64> {
    let uses: Vec<i64> = it.recent_uses.iter().map(|&t| t as i64).filter(|t| *t > 0).collect();
    if uses.is_empty() || max_total_uses == 0 {
        return None;
    }
    let score: f64 = uses.iter().take(MAX_SAMPLES).map(|&t| weight((now_ms - t) / DAY_MS)).sum();
    Some((1000. * (f64::from(it.total_uses) / f64::from(max_total_uses) * 0.2 + score / 1000. * 0.8)).trunc() as i64)
}

pub fn rank_like_client<'a>(order: &[String], item: impl Fn(&str) -> Option<&'a FrecencyItem>, now_ms: i64) -> Vec<String> {
    let mut scored: Vec<(String, i64)> =
        js_key_order(order).into_iter().filter_map(|k| item(&k).and_then(|it| client_frecency(it, now_ms)).map(|f| (k, f))).collect();
    scored.sort_by_key(|(_, f)| std::cmp::Reverse(*f));
    scored.into_iter().map(|(k, _)| k).collect()
}

impl FrecencyUserSettings {
    pub fn places_at(&self, now_ms: i64) -> HashMap<u64, i64> {
        self.guild_and_channel_frecency
            .iter()
            .flat_map(|g| g.guild_and_channels.iter())
            .filter_map(|(id, it)| place_frecency(it, now_ms).map(|f| (*id, f)))
            .collect()
    }

    pub fn decode_base64_ordered(s: &str) -> Result<(Self, FrecencyOrder)> {
        let bytes = STANDARD.decode(s.trim()).map_err(|e| Error::Protocol(format!("frecency base64: {e}")))?;
        let settings = <Self as prost::Message>::decode(bytes.as_slice()).map_err(|e| Error::Protocol(format!("frecency proto: {e}")))?;
        Ok((settings, wire_order(&bytes)))
    }

    pub fn emojis_ranked_at(&self, order: &FrecencyOrder, now_ms: i64) -> Vec<String> {
        let map = self.emoji_frecency.as_ref().map(|e| &e.emojis);
        if map.is_none_or(|m| m.is_empty()) {
            let seeded: Vec<String> = DEFAULT_EMOJIS.iter().map(|s| (*s).to_owned()).collect();
            return js_key_order(&seeded);
        }
        let mut keys = order.emojis.clone();
        if let Some(m) = map {
            let mut extra: Vec<&String> = m.keys().filter(|k| !keys.contains(k)).collect();
            extra.sort();
            keys.extend(extra.into_iter().cloned());
        }
        rank_like_client(&keys, |k| map.and_then(|m| m.get(k)), now_ms)
    }

    pub fn favorite_emoji_keys(&self) -> Vec<String> {
        self.favorite_emojis.as_ref().map(|f| f.emojis.clone()).unwrap_or_default()
    }

    pub fn reactions_ranked_at(&self, order: &FrecencyOrder, now_ms: i64) -> Vec<String> {
        let Some(map) = self.emoji_reaction_frecency.as_ref().map(|e| &e.emojis) else { return Vec::new() };
        let max = map.values().map(|it| it.total_uses).max().unwrap_or(0);
        let mut keys = order.reactions.clone();
        let mut extra: Vec<&String> = map.keys().filter(|k| !keys.contains(k)).collect();
        extra.sort();
        keys.extend(extra.into_iter().cloned());
        let mut scored: Vec<(String, i64)> = js_key_order(&keys)
            .into_iter()
            .filter_map(|k| map.get(&k).and_then(|it| reaction_frecency(it, max, now_ms)).map(|f| (k, f)))
            .collect();
        scored.sort_by_key(|(_, f)| std::cmp::Reverse(*f));
        scored.into_iter().map(|(k, _)| k).collect()
    }

    pub fn stickers_ranked_at(&self, order: &FrecencyOrder, now_ms: i64) -> Vec<u64> {
        let Some(map) = self.sticker_frecency.as_ref().map(|s| &s.stickers) else { return Vec::new() };
        let mut ids = order.stickers.clone();
        let mut extra: Vec<u64> = map.keys().copied().filter(|k| !ids.contains(k)).collect();
        extra.sort_unstable();
        ids.extend(extra);
        let keys: Vec<String> = ids.iter().map(u64::to_string).collect();
        rank_like_client(&keys, |k| k.parse::<u64>().ok().and_then(|id| map.get(&id)), now_ms).into_iter().filter_map(|k| k.parse().ok()).collect()
    }
}

#[derive(Deserialize)]
struct ProtoBody {
    settings: String,
}

impl RestClient {
    pub async fn get_frecency_settings(&self) -> Result<FrecencyUserSettings> {
        let b: ProtoBody = self.call(Method::GET, "/users/@me/settings-proto/2", Body::None).await?;
        FrecencyUserSettings::decode_base64(&b.settings)
    }

    pub async fn get_frecency_settings_ordered(&self) -> Result<(FrecencyUserSettings, FrecencyOrder)> {
        let b: ProtoBody = self.call(Method::GET, "/users/@me/settings-proto/2", Body::None).await?;
        FrecencyUserSettings::decode_base64_ordered(&b.settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_frecency_uses_the_channel_weights() {
        let now = 100 * DAY_MS;
        let it = |total: u32, days: &[i64]| FrecencyItem { total_uses: total, recent_uses: days.iter().map(|d| (now - d * DAY_MS) as u64).collect(), frecency: 0, score: 0 };
        assert_eq!(place_frecency(&it(4, &[0, 1]), now), Some(340));
        assert_eq!(place_frecency(&it(1, &[3]), now), Some(50));
        assert_eq!(place_frecency(&it(3, &[6, 30]), now), Some(60));
        assert_eq!(place_frecency(&it(1, &[]), now), None);
        let s = FrecencyUserSettings {
            guild_and_channel_frecency: Some(GuildAndChannelFrecency { guild_and_channels: [(7u64, it(2, &[0]))].into_iter().collect() }),
            ..Default::default()
        };
        let bytes = prost::Message::encode_to_vec(&s);
        let back = <FrecencyUserSettings as prost::Message>::decode(bytes.as_slice()).unwrap();
        assert_eq!(back.places_at(now).get(&7), Some(&200));
    }

    #[test]
    fn ranks_and_roundtrips() {
        let item = |score, uses| FrecencyItem { total_uses: uses, recent_uses: vec![], frecency: 0, score };
        let s = FrecencyUserSettings {
            favorite_gifs: Some(FavoriteGifs {
                gifs: HashMap::from([
                    ("a".into(), FavoriteGif { order: 1, ..Default::default() }),
                    ("b".into(), FavoriteGif { order: 5, ..Default::default() }),
                ]),
            }),
            sticker_frecency: Some(StickerFrecency { stickers: HashMap::from([(1, item(3, 1)), (2, item(9, 1))]) }),
            emoji_frecency: Some(EmojiFrecency {
                emojis: HashMap::from([("joy".into(), item(10, 4)), ("123".into(), item(20, 1))]),
            }),
            application_frecency: Some(ApplicationFrecency {
                applications: HashMap::from([("100".into(), item(5, 1)), ("200".into(), item(15, 2))]),
            }),
            favorite_soundboard_sounds: Some(FavoriteSoundboardSounds { sound_ids: vec![7, 8], ordered_sound_ids: vec![] }),
            played_sound_frecency: Some(PlayedSoundFrecency {
                played_sounds: HashMap::from([("3".into(), item(202, 4)), ("1568753632852054267".into(), item(730, 23))]),
            }),
            ..Default::default()
        };
        let b64 = STANDARD.encode(prost::Message::encode_to_vec(&s));
        let d = FrecencyUserSettings::decode_base64(&b64).unwrap();
        assert_eq!(d.emojis_ranked(), vec!["123".to_string(), "joy".to_string()]);
        assert_eq!(d.stickers_ranked(), vec![2, 1]);
        assert_eq!(d.favorite_gifs()[0].0, "b");
        assert_eq!(d.applications_ranked(), vec![200, 100]);
        assert_eq!(d.favorite_sounds(), vec![7, 8]);
        assert_eq!(d.played_sounds_ranked(), vec![1568753632852054267, 3]);
    }

    fn uses(total: u32, recent: &[i64]) -> FrecencyItem {
        FrecencyItem { total_uses: total, recent_uses: recent.iter().map(|&t| t as u64).collect(), frecency: 0, score: 0 }
    }

    #[test]
    fn client_weights_by_age() {
        let now = 1_000 * DAY_MS;
        assert_eq!(client_frecency(&uses(4, &[now - DAY_MS, now - 2 * DAY_MS]), now), Some(400));
        assert_eq!(client_frecency(&uses(3, &[now - 10 * DAY_MS, now - 20 * DAY_MS, now - 100 * DAY_MS]), now), Some(121));
        assert_eq!(client_frecency(&uses(5, &[]), now), None);
        assert_eq!(client_frecency(&uses(1, &[now - 3 * DAY_MS - 1]), now), Some(100));
        assert_eq!(client_frecency(&uses(1, &[now - 4 * DAY_MS]), now), Some(70));
        let twelve: Vec<i64> = (0..12).map(|i| now - (i + 1) * DAY_MS).collect();
        assert_eq!(client_frecency(&uses(12, &twelve), now), Some(790));
    }

    #[test]
    fn js_object_key_order() {
        let keys: Vec<String> = ["heart", "100", "1234567890123", "8", "heart", "joy"].iter().map(|s| s.to_string()).collect();
        assert_eq!(js_key_order(&keys), vec!["8", "100", "heart", "1234567890123", "joy"]);
    }

    #[test]
    fn ranks_like_the_client_and_keeps_wire_order_on_ties() {
        let now = 500 * DAY_MS;
        let mut emojis = HashMap::new();
        emojis.insert("white_check_mark".to_string(), uses(2, &[now - DAY_MS]));
        emojis.insert("heart".to_string(), uses(9, &[now - 90 * DAY_MS]));
        emojis.insert("wave".to_string(), uses(2, &[now - DAY_MS]));
        let s = FrecencyUserSettings { emoji_frecency: Some(EmojiFrecency { emojis }), ..Default::default() };
        let b64 = STANDARD.encode(prost::Message::encode_to_vec(&s));
        let (d, order) = FrecencyUserSettings::decode_base64_ordered(&b64).unwrap();
        assert_eq!(order.emojis.len(), 3);
        let ranked = d.emojis_ranked_at(&order, now);
        assert_eq!(ranked.last().map(String::as_str), Some("heart"));
        let first_two: Vec<&String> = order.emojis.iter().filter(|k| *k != "heart").collect();
        assert_eq!(ranked[..2].iter().collect::<Vec<_>>(), first_two);
    }

    #[test]
    fn wire_order_reads_sticker_keys() {
        let mut stickers = HashMap::new();
        stickers.insert(749054660769218631u64, uses(1, &[DAY_MS]));
        let s = FrecencyUserSettings { sticker_frecency: Some(StickerFrecency { stickers }), ..Default::default() };
        let bytes = prost::Message::encode_to_vec(&s);
        assert_eq!(wire_order(&bytes).stickers, vec![749054660769218631]);
        assert!(wire_order(&[0xff]).emojis.is_empty());
    }

    #[test]
    fn reactions_use_the_client_reaction_formula() {
        let now = 500 * DAY_MS;
        let mut emojis = HashMap::new();
        emojis.insert("white_check_mark".to_string(), uses(224, &[now - 100 * DAY_MS; 10]));
        emojis.insert("heart".to_string(), uses(74, &[now - DAY_MS; 10]));
        emojis.insert("1554467402027241472".to_string(), uses(8, &[now - 2 * DAY_MS; 8]));
        let s = FrecencyUserSettings {
            emoji_reaction_frecency: Some(EmojiFrecency { emojis }),
            favorite_emojis: Some(FavoriteEmojis { emojis: vec!["wave".into()] }),
            ..Default::default()
        };
        let b64 = STANDARD.encode(prost::Message::encode_to_vec(&s));
        let (d, order) = FrecencyUserSettings::decode_base64_ordered(&b64).unwrap();
        assert_eq!(order.reactions.len(), 3);
        assert_eq!(d.favorite_emoji_keys(), vec!["wave".to_string()]);
        let max = 224;
        assert_eq!(reaction_frecency(&uses(74, &[now - DAY_MS; 10]), max, now), Some(866));
        assert_eq!(reaction_frecency(&uses(224, &[now - 100 * DAY_MS; 10]), max, now), Some(208));
        assert_eq!(d.reactions_ranked_at(&order, now), vec!["heart", "1554467402027241472", "white_check_mark"]);
    }

    #[test]
    fn empty_emoji_frecency_seeds_the_client_defaults() {
        let d = FrecencyUserSettings::default();
        let r = d.emojis_ranked_at(&FrecencyOrder::default(), 0);
        assert_eq!(r[0], "100");
        assert_eq!(r[1], "thumbsup");
        assert_eq!(r.len(), 10);
    }
}

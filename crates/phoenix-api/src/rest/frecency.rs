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
pub struct FrecencyUserSettings {
    #[prost(message, optional, tag = "2")]
    pub favorite_gifs: Option<FavoriteGifs>,
    #[prost(message, optional, tag = "4")]
    pub sticker_frecency: Option<StickerFrecency>,
    #[prost(message, optional, tag = "6")]
    pub emoji_frecency: Option<EmojiFrecency>,
    #[prost(message, optional, tag = "9")]
    pub application_frecency: Option<ApplicationFrecency>,
}

fn rank(it: &FrecencyItem) -> (i32, u32, u64) {
    (it.score, it.total_uses, it.recent_uses.iter().copied().max().unwrap_or(0))
}

impl FrecencyUserSettings {
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

#[derive(Deserialize)]
struct ProtoBody {
    settings: String,
}

impl RestClient {
    pub async fn get_frecency_settings(&self) -> Result<FrecencyUserSettings> {
        let b: ProtoBody = self.call(Method::GET, "/users/@me/settings-proto/2", Body::None).await?;
        FrecencyUserSettings::decode_base64(&b.settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        let b64 = STANDARD.encode(prost::Message::encode_to_vec(&s));
        let d = FrecencyUserSettings::decode_base64(&b64).unwrap();
        assert_eq!(d.emojis_ranked(), vec!["123".to_string(), "joy".to_string()]);
        assert_eq!(d.stickers_ranked(), vec![2, 1]);
        assert_eq!(d.favorite_gifs()[0].0, "b");
        assert_eq!(d.applications_ranked(), vec![200, 100]);
    }
}

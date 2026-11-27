use gpui::SharedString;

use crate::{media::StickerFormat, model::Id};

#[derive(Clone, Debug, PartialEq)]
pub struct GuildEmoji {
    pub id: Id,
    pub name: SharedString,
    pub animated: bool,
    pub guild: Id,
    pub available: bool,
}

impl GuildEmoji {
    pub fn token(&self) -> String {
        format!("<{}:{}:{}>", if self.animated { "a" } else { "" }, self.name, self.id.0)
    }

    pub fn url(&self, size: u32) -> SharedString {
        crate::media::emoji::CustomEmoji { name: self.name.clone(), id: self.id.0, animated: self.animated }.url(size)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StickerData {
    pub id: Id,
    pub name: SharedString,
    pub description: SharedString,
    pub tags: SharedString,
    pub format: StickerFormat,
    pub guild: Option<Id>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StickerPackData {
    pub id: Id,
    pub name: SharedString,
    pub stickers: Vec<StickerData>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GifItem {
    pub url: SharedString,
    pub preview: SharedString,
    pub source: Option<SharedString>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct GifCategoryTile {
    pub name: SharedString,
    pub preview: SharedString,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct GifHome {
    pub trending_preview: Option<SharedString>,
    pub categories: Vec<GifCategoryTile>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Frecency {
    pub emojis: Vec<SharedString>,
    pub reactions: Vec<SharedString>,
    pub favorite_emojis: Vec<SharedString>,
    pub stickers: Vec<Id>,
    pub favorite_gifs: Vec<GifItem>,
    pub applications: Vec<u64>,
    pub favorite_sounds: Vec<u64>,
    pub played_sounds: Vec<u64>,
    pub commands: Vec<String>,
}

//! Data exchanged with the [`crate::backend::Backend`] for the expression
//! pickers (independent from `phoenix-api` types, like `crate::model`).

use gpui::SharedString;

use crate::{media::StickerFormat, model::Id};

/// A guild custom emoji.
#[derive(Clone, Debug, PartialEq)]
pub struct GuildEmoji {
    pub id: Id,
    pub name: SharedString,
    pub animated: bool,
    pub guild: Id,
    /// False when the guild lost the boost level that allowed it.
    pub available: bool,
}

impl GuildEmoji {
    /// Message token: `<:name:id>` / `<a:name:id>`.
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
    /// Autocomplete keywords.
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

/// One GIF result. `url` is what gets sent (Discord embeds it).
#[derive(Clone, Debug, PartialEq)]
pub struct GifItem {
    pub url: SharedString,
    /// Animated preview shown in the picker.
    pub preview: SharedString,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct GifCategoryTile {
    pub name: SharedString,
    pub preview: SharedString,
}

/// GIF picker home: "Trending GIFs" tile + category tiles.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GifHome {
    pub trending_preview: Option<SharedString>,
    pub categories: Vec<GifCategoryTile>,
}

/// Account-synced expression usage (`settings-proto/2`, read-only).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Frecency {
    /// Emoji keys, most used first: custom emoji id, or unicode name / glyph.
    pub emojis: Vec<SharedString>,
    /// Sticker ids, most used first.
    pub stickers: Vec<Id>,
    /// Favorite GIFs, newest first.
    pub favorite_gifs: Vec<GifItem>,
    /// Recently used application ids, most used first.
    pub applications: Vec<u64>,
}

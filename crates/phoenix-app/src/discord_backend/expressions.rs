use std::{sync::Arc, time::Duration};

use phoenix_api::rest::search::{
    Gif, GifFormat, HasFilter, SearchOutcome, SearchQuery, SearchSort, Sticker,
};

use super::{Inner, Session, api_err, convert, sf};
use crate::{
    backend::{AppEvent, BackendError},
    media::StickerFormat,
    model::Id,
    ui::{
        pickers::types::{GifCategoryTile, GifHome, GifItem, GuildEmoji, StickerData, StickerPackData},
        search::{SearchPage, SearchRequest, SortOrder},
    },
};

pub(super) fn guild_emojis(s: &Session, guild: Id) -> Vec<GuildEmoji> {
    let Some(g) = s.cache.guild(sf(guild)) else { return Vec::new() };
    g.emojis
        .iter()
        .filter_map(|e| {
            Some(GuildEmoji {
                id: convert::id(e.id?),
                name: e.name.clone()?.into(),
                animated: e.animated,
                guild,
                available: e.available,
            })
        })
        .collect()
}

fn sticker_format(n: u8) -> StickerFormat {
    match n {
        2 => StickerFormat::Apng,
        3 => StickerFormat::Lottie,
        4 => StickerFormat::Gif,
        _ => StickerFormat::Png,
    }
}

fn sticker(st: &Sticker) -> StickerData {
    StickerData {
        id: convert::id(st.id),
        name: st.name.clone().into(),
        description: st.description.clone().unwrap_or_default().into(),
        tags: st.tags.clone().into(),
        format: sticker_format(st.format_type),
        guild: st.guild_id.map(convert::id),
    }
}

pub(super) fn guild_stickers(s: &Session, guild: Id) -> Vec<StickerData> {
    let Some(g) = s.cache.guild(sf(guild)) else { return Vec::new() };
    g.stickers
        .iter()
        .filter_map(|v| serde_json::from_value::<Sticker>(v.clone()).ok())
        .filter(|st| st.available)
        .map(|st| sticker(&st))
        .collect()
}

pub(super) async fn sticker_packs(s: Arc<Session>) -> Result<Vec<StickerPackData>, BackendError> {
    let locale = s.rest.properties().system_locale.clone();
    let packs = s.rest.get_sticker_packs(&locale).await.map_err(api_err)?;
    Ok(packs
        .iter()
        .map(|p| StickerPackData {
            id: convert::id(p.id),
            name: p.name.clone().into(),
            stickers: p.stickers.iter().map(sticker).collect(),
        })
        .collect())
}

fn gif(g: &Gif) -> GifItem {
    let video = g.src.is_empty() || g.src.split('?').next().is_some_and(|u| u.ends_with(".mp4") || u.ends_with(".webm"));
    let preview = if video && !g.gif_src.is_empty() { g.gif_src.clone() } else { g.src.clone() };
    GifItem { url: g.url.clone().into(), preview: preview.into(), width: g.width, height: g.height }
}

const PREVIEW: GifFormat = GifFormat::TinyGif;

pub(super) async fn gif_trending(s: Arc<Session>) -> Result<GifHome, BackendError> {
    let locale = s.rest.properties().system_locale.clone();
    let t = s.rest.gif_trending(&locale, PREVIEW).await.map_err(api_err)?;
    Ok(GifHome {
        trending_preview: t.gifs.first().map(|g| gif(g).preview),
        categories: t
            .categories
            .iter()
            .map(|c| GifCategoryTile { name: c.name.clone().into(), preview: c.src.clone().into() })
            .collect(),
    })
}

fn tenor_still(src: &str) -> String {
    if let Some(rest) = src.strip_prefix("https://media.tenor.com/")
        && let Some((seg, file)) = rest.split_once('/')
        && seg.len() > 5
        && file.ends_with(".mp4")
    {
        let stem = file.trim_end_matches(".mp4");
        return format!("https://media.tenor.com/{}AAAAM/{stem}.gif", &seg[..seg.len() - 5]);
    }
    src.to_owned()
}

pub(super) async fn frecency(s: Arc<Session>) -> Result<crate::ui::pickers::types::Frecency, BackendError> {
    let (f, order) = s.rest.get_frecency_settings_ordered().await.map_err(api_err)?;
    let now = chrono::Utc::now().timestamp_millis();
    Ok(crate::ui::pickers::types::Frecency {
        emojis: f.emojis_ranked_at(&order, now).into_iter().map(Into::into).collect(),
        reactions: f.reactions_ranked_at(&order, now).into_iter().map(Into::into).collect(),
        favorite_emojis: f.favorite_emoji_keys().into_iter().map(Into::into).collect(),
        stickers: f.stickers_ranked_at(&order, now).into_iter().map(Id).collect(),
        favorite_gifs: f
            .favorite_gifs()
            .into_iter()
            .map(|(url, g)| GifItem { url: url.into(), preview: tenor_still(&g.src).into(), width: g.width, height: g.height })
            .collect(),
        applications: f.applications_ranked(),
        favorite_sounds: f.favorite_sounds(),
        played_sounds: f.played_sounds_ranked(),
        commands: f.commands_ranked_at(&order, now),
    })
}

pub(super) async fn gif_trending_gifs(s: Arc<Session>) -> Result<Vec<GifItem>, BackendError> {
    let locale = s.rest.properties().system_locale.clone();
    Ok(s.rest.gif_trending_gifs(&locale, PREVIEW).await.map_err(api_err)?.iter().map(gif).collect())
}

pub(super) async fn gif_search(s: Arc<Session>, q: String) -> Result<Vec<GifItem>, BackendError> {
    let locale = s.rest.properties().system_locale.clone();
    Ok(s.rest.gif_search(&q, &locale, PREVIEW).await.map_err(api_err)?.iter().map(gif).collect())
}

pub(super) async fn send_sticker(s: Arc<Session>, channel: Id, sticker: Id) -> Result<(), BackendError> {
    let msg = phoenix_api::rest::CreateMessage { sticker_ids: vec![sf(sticker)], ..Default::default() };
    s.rest.send_message(sf(channel), msg).await.map_err(api_err)?;
    Ok(())
}

pub(super) fn query_of(req: &SearchRequest) -> SearchQuery {
    SearchQuery {
        content: req.content.clone(),
        author_ids: req.authors.iter().copied().map(sf).collect(),
        mentions: req.mentions.iter().copied().map(sf).collect(),
        channel_ids: req.channels.iter().copied().map(sf).collect(),
        has: req.has.iter().filter_map(|h| HasFilter::parse(h)).collect(),
        min_id: req.min_id.map(sf),
        max_id: req.max_id.map(sf),
        pinned: req.pinned,
        author_types: req.author_types.clone(),
        attachment_extensions: req.attachment_extensions.clone(),
        sort: match req.sort {
            SortOrder::Newest => SearchSort::Newest,
            SortOrder::Oldest => SearchSort::Oldest,
            SortOrder::Relevance => SearchSort::Relevance,
        },
        offset: req.offset,
        include_nsfw: true,
    }
}

pub(super) async fn search(inner: Arc<Inner>, s: Arc<Session>, req: SearchRequest) -> Result<SearchPage, BackendError> {
    let q = query_of(&req);
    for _ in 0..4 {
        let out = match (req.guild, req.channel) {
            _ if req.all_dms => s.rest.search_dms_tabs(&q, crate::ui::search::PAGE).await,
            (Some(g), _) => s.rest.search_guild_messages(sf(g), &q).await,
            (None, Some(c)) => s.rest.search_channel_messages(sf(c), &q).await,
            (None, None) => return Err(BackendError(crate::t!("search.nothing").to_string())),
        }
        .map_err(api_err)?;
        match out {
            SearchOutcome::Results(r) => {
                let hits: Vec<_> = r.hits().cloned().collect();
                let mut users = Vec::new();
                for m in &hits {
                    convert::message_users(m, &mut users);
                }
                inner.emit(AppEvent::Users(users));
                super::ensure_message_authors(&inner, &s, &hits);
                return Ok(SearchPage { total: r.total_results, messages: hits.iter().map(convert::message).collect() });
            }
            SearchOutcome::Indexing { retry_after } => tokio::time::sleep(Duration::from_secs_f64(retry_after)).await,
        }
    }
    Err(BackendError(crate::t!("search.indexing").to_string()))
}

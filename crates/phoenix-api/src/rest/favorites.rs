use std::collections::BTreeMap;

use super::RestClient;
use super::settings::{BoolValue, field_bytes, fields};
use crate::error::Result;

pub const SETTINGS_TAG: u32 = 15;
pub const REFERENCE_ORIGINAL: i32 = 1;
pub const CATEGORY: i32 = 2;
pub const MAX_ENTRIES: usize = 200;
pub const FREE_LIMIT: usize = 3;
pub const NITRO_LIMIT: usize = 200;
pub const THREAD_TYPES: [u32; 3] = [10, 11, 12];
const DISCORD_EPOCH: u64 = 1_420_070_400_000;

#[derive(Clone, PartialEq, prost::Message)]
pub struct UInt32Value {
    #[prost(uint32, tag = "1")]
    pub value: u32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FavoriteChannel {
    #[prost(string, tag = "1")]
    pub nickname: String,
    #[prost(int32, tag = "2")]
    pub kind: i32,
    #[prost(uint32, tag = "3")]
    pub position: u32,
    #[prost(fixed64, tag = "4")]
    pub parent_id: u64,
    #[prost(message, optional, tag = "5")]
    pub channel_type: Option<UInt32Value>,
    #[prost(bool, tag = "6")]
    pub collapsed: bool,
}

impl FavoriteChannel {
    pub fn is_category(&self) -> bool {
        self.kind == CATEGORY
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Favorites {
    #[prost(btree_map = "fixed64, message", tag = "1")]
    pub favorite_channels: BTreeMap<u64, FavoriteChannel>,
    #[prost(bool, tag = "2")]
    pub muted: bool,
    #[prost(message, optional, tag = "3")]
    pub guild_visible: Option<BoolValue>,
    #[prost(bool, tag = "4")]
    pub auto_add_joined_threads: bool,
    #[prost(message, optional, tag = "5")]
    pub prune_inactive_threads: Option<BoolValue>,
    #[prost(uint32, tag = "6")]
    pub prune_inactive_threads_days: u32,
    #[prost(bool, tag = "7")]
    pub prune_all_favorited_threads: bool,
    #[prost(bool, tag = "8")]
    pub auto_add_joined_threads_per_guild: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Known {
    Missing,
    Hidden,
    Visible { channel_type: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Category(u64),
    Channel(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub id: u64,
    pub position: Option<u32>,
    pub parent: Option<Option<u64>>,
}

pub fn snowflake_at(unix_ms: u64) -> u64 {
    unix_ms.saturating_sub(DISCORD_EPOCH) << 22
}

pub fn is_vocal(channel_type: u32) -> bool {
    matches!(channel_type, 2 | 13)
}

impl Favorites {
    pub fn from_settings(raw: &[u8]) -> Self {
        let mut merged = Vec::new();
        for (tag, wt, _, d) in fields(raw) {
            if tag == SETTINGS_TAG && wt == 2 {
                merged.extend_from_slice(d);
            }
        }
        <Self as prost::Message>::decode(merged.as_slice()).unwrap_or_default()
    }

    pub fn has_settings_field(raw: &[u8]) -> bool {
        fields(raw).iter().any(|f| f.0 == SETTINGS_TAG && f.1 == 2)
    }

    pub fn settings_patch(&self) -> Vec<u8> {
        let mut out = Vec::new();
        field_bytes(SETTINGS_TAG, &prost::Message::encode_to_vec(self), &mut out);
        out
    }

    pub fn set_guild_visible(&mut self, visible: bool) {
        self.guild_visible = Some(BoolValue { value: visible });
    }

    pub fn guild_enabled(&self) -> bool {
        self.guild_visible.as_ref().map_or(!self.favorite_channels.is_empty(), |v| v.value)
    }

    pub fn is_favorite(&self, id: u64) -> bool {
        self.favorite_channels.contains_key(&id)
    }

    pub fn get(&self, id: u64) -> Option<&FavoriteChannel> {
        self.favorite_channels.get(&id)
    }

    pub fn nickname(&self, id: u64) -> Option<&str> {
        self.get(id).map(|f| f.nickname.as_str()).filter(|n| !n.is_empty())
    }

    pub fn next_position(&self) -> u32 {
        self.favorite_channels.values().map(|f| f.position).max().unwrap_or(0) + 1
    }

    pub fn count_against_limit(&self) -> usize {
        self.favorite_channels.values().filter(|f| !f.is_category()).count()
    }

    pub fn limit(nitro: bool) -> usize {
        if nitro { NITRO_LIMIT } else { FREE_LIMIT }
    }

    pub fn categories(&self) -> Vec<(u64, &str)> {
        let mut v: Vec<(u32, u64, &str)> =
            self.favorite_channels.iter().filter(|(_, f)| f.is_category()).map(|(id, f)| (f.position, *id, f.nickname.as_str())).collect();
        v.sort();
        v.into_iter().map(|(_, id, n)| (id, n)).collect()
    }

    pub fn category_named(&self, name: &str) -> Option<u64> {
        let n = name.trim().to_lowercase();
        self.favorite_channels.iter().find(|(_, f)| f.is_category() && f.nickname.trim().to_lowercase() == n).map(|(id, _)| *id)
    }

    pub fn prune(&mut self, known: impl Fn(u64) -> Known) {
        self.favorite_channels.retain(|id, f| {
            if f.is_category() {
                return true;
            }
            match known(*id) {
                Known::Missing => f.channel_type.as_ref().is_some_and(|t| THREAD_TYPES.contains(&t.value)),
                Known::Hidden => false,
                Known::Visible { channel_type } => {
                    if f.channel_type.is_none() {
                        f.channel_type = Some(UInt32Value { value: channel_type });
                    }
                    true
                }
            }
        });
    }

    pub fn limit_reached(&self, kind: i32, nitro: bool) -> Option<usize> {
        if self.favorite_channels.len() >= MAX_ENTRIES {
            return Some(MAX_ENTRIES);
        }
        let limit = Self::limit(nitro);
        (kind != CATEGORY && self.count_against_limit() >= limit).then_some(limit)
    }

    fn fix_parent(&mut self, id: u64) {
        let Some(parent) = self.favorite_channels.get(&id).map(|f| f.parent_id) else { return };
        if parent != 0
            && !self.favorite_channels.get(&parent).is_some_and(FavoriteChannel::is_category)
            && let Some(f) = self.favorite_channels.get_mut(&id)
        {
            f.parent_id = 0;
        }
    }

    pub fn create_category(&mut self, name: &str, id: u64) -> u64 {
        let position = self.next_position();
        self.favorite_channels.insert(id, FavoriteChannel { nickname: name.to_owned(), kind: CATEGORY, position, parent_id: 0, channel_type: None, collapsed: false });
        id
    }

    pub fn add(&mut self, channels: &[(u64, Option<u32>)], parent: Option<u64>, nitro: bool) -> std::result::Result<bool, usize> {
        let was_enabled = self.guild_enabled();
        let mut added = false;
        for &(id, channel_type) in channels {
            if self.is_favorite(id) {
                continue;
            }
            if let Some(limit) = self.limit_reached(REFERENCE_ORIGINAL, nitro) {
                if !added {
                    return Err(limit);
                }
                break;
            }
            let position = self.next_position();
            self.favorite_channels.insert(
                id,
                FavoriteChannel {
                    nickname: String::new(),
                    kind: REFERENCE_ORIGINAL,
                    position,
                    parent_id: parent.unwrap_or(0),
                    channel_type: channel_type.map(|value| UInt32Value { value }),
                    collapsed: false,
                },
            );
            self.fix_parent(id);
            added = true;
        }
        if added && !was_enabled {
            self.guild_visible = Some(BoolValue { value: true });
        }
        Ok(added)
    }

    pub fn add_to_category(&mut self, channels: &[(u64, Option<u32>)], name: &str, new_id: u64, nitro: bool) -> std::result::Result<bool, usize> {
        if channels.iter().all(|(id, _)| self.is_favorite(*id)) {
            return Ok(false);
        }
        if let Some(limit) = self.limit_reached(REFERENCE_ORIGINAL, nitro) {
            return Err(limit);
        }
        let parent = match self.category_named(name) {
            Some(id) => id,
            None => self.create_category(name, new_id),
        };
        if let Some(limit) = self.limit_reached(REFERENCE_ORIGINAL, nitro) {
            return Err(limit);
        }
        self.add(channels, Some(parent), nitro)
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let Some(old) = self.favorite_channels.remove(&id) else { return false };
        if old.is_category() {
            for f in self.favorite_channels.values_mut() {
                if f.parent_id == id {
                    f.parent_id = 0;
                }
            }
        }
        true
    }

    pub fn rename(&mut self, id: u64, nickname: Option<&str>) -> bool {
        let Some(f) = self.favorite_channels.get_mut(&id) else { return false };
        f.nickname = nickname.unwrap_or_default().to_owned();
        true
    }

    pub fn set_collapsed(&mut self, id: Option<u64>, collapsed: bool) -> bool {
        let mut changed = false;
        for (k, f) in self.favorite_channels.iter_mut() {
            if id.is_some_and(|i| i != *k) || !f.is_category() || f.collapsed == collapsed {
                continue;
            }
            f.collapsed = collapsed;
            changed = true;
        }
        changed
    }

    pub fn apply_moves(&mut self, moves: &[Move]) {
        for m in moves {
            let Some(f) = self.favorite_channels.get_mut(&m.id) else { continue };
            if let Some(p) = m.position {
                f.position = p;
            }
            if let Some(parent) = m.parent {
                f.parent_id = parent.unwrap_or(0);
                self.fix_parent(m.id);
            }
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn rows(&self, channel_type: impl Fn(u64) -> Option<u32>) -> Vec<Row> {
        let mut uncategorized: Vec<(u8, u32, u64)> = Vec::new();
        let mut by_cat: BTreeMap<u64, Vec<(u8, u32, u64)>> = BTreeMap::new();
        for (&id, f) in &self.favorite_channels {
            if f.is_category() {
                continue;
            }
            let Some(t) = channel_type(id) else { continue };
            let key = (u8::from(is_vocal(t)), f.position, id);
            if f.parent_id != 0 && self.favorite_channels.get(&f.parent_id).is_some_and(FavoriteChannel::is_category) {
                by_cat.entry(f.parent_id).or_default().push(key);
            } else {
                uncategorized.push(key);
            }
        }
        uncategorized.sort();
        let mut out: Vec<Row> = uncategorized.into_iter().map(|k| Row::Channel(k.2)).collect();
        for (cat, _) in self.categories() {
            out.push(Row::Category(cat));
            let mut kids = by_cat.remove(&cat).unwrap_or_default();
            kids.sort();
            out.extend(kids.into_iter().map(|k| Row::Channel(k.2)));
        }
        out
    }
}

impl RestClient {
    pub async fn patch_favorites(&self, favorites: &Favorites) -> Result<Vec<u8>> {
        let body = prost::Message::encode_to_vec(favorites);
        self.edit_settings_proto(move |raw| super::settings::replace_field(&raw, SETTINGS_TAG, &body)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fav(kind: i32, position: u32, parent_id: u64) -> FavoriteChannel {
        FavoriteChannel { nickname: String::new(), kind, position, parent_id, channel_type: Some(UInt32Value { value: 0 }), collapsed: false }
    }

    #[test]
    fn wire_format_matches_the_client_descriptor() {
        let mut f = Favorites::default();
        f.favorite_channels.insert(
            0x0102_0304_0506_0708,
            FavoriteChannel { nickname: "a".into(), kind: REFERENCE_ORIGINAL, position: 3, parent_id: 9, channel_type: Some(UInt32Value { value: 2 }), collapsed: true },
        );
        f.guild_visible = Some(BoolValue { value: true });
        let bytes = prost::Message::encode_to_vec(&f);
        let entry: Vec<u8> = [
            vec![0x09, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01],
            vec![0x12, 0x16],
            vec![0x0a, 0x01, b'a'],
            vec![0x10, 0x01],
            vec![0x18, 0x03],
            vec![0x21, 9, 0, 0, 0, 0, 0, 0, 0],
            vec![0x2a, 0x02, 0x08, 0x02],
            vec![0x30, 0x01],
        ]
        .concat();
        let mut expect = vec![0x0a, entry.len() as u8];
        expect.extend(entry);
        expect.extend([0x1a, 0x02, 0x08, 0x01]);
        assert_eq!(bytes, expect);
        assert_eq!(<Favorites as prost::Message>::decode(bytes.as_slice()).unwrap(), f);
    }

    #[test]
    fn patch_is_field_15_only_and_round_trips_through_settings() {
        let mut f = Favorites::default();
        f.add(&[(42, Some(0))], None, false).unwrap();
        let patch = f.settings_patch();
        assert_eq!(patch[0], (15 << 3) | 2);
        let mut blob = vec![0x6a, 0x02, 0x08, 0x04];
        blob.extend(&patch);
        assert!(Favorites::has_settings_field(&blob));
        assert_eq!(Favorites::from_settings(&blob), f);
        assert!(!Favorites::has_settings_field(&[0x6a, 0x02, 0x08, 0x04]));
    }

    #[test]
    fn empty_favorites_still_send_the_category() {
        assert_eq!(Favorites::default().settings_patch(), vec![0x7a, 0x00]);
    }

    #[test]
    fn split_occurrences_merge_like_protobuf() {
        let mut a = Favorites::default();
        a.favorite_channels.insert(1, fav(REFERENCE_ORIGINAL, 1, 0));
        let mut b = Favorites::default();
        b.favorite_channels.insert(2, fav(REFERENCE_ORIGINAL, 2, 0));
        b.muted = true;
        let mut blob = a.settings_patch();
        blob.extend(b.settings_patch());
        let m = Favorites::from_settings(&blob);
        assert_eq!(m.favorite_channels.len(), 2);
        assert!(m.muted);
    }

    #[test]
    fn add_follows_the_client() {
        let mut f = Favorites::default();
        assert!(!f.guild_enabled());
        assert_eq!(f.add(&[(10, Some(0)), (11, Some(2))], None, false), Ok(true));
        assert!(f.guild_enabled());
        assert_eq!(f.guild_visible, Some(BoolValue { value: true }));
        assert_eq!(f.get(10).unwrap().position, 1);
        assert_eq!(f.get(11).unwrap().position, 2);
        assert_eq!(f.get(11).unwrap().channel_type, Some(UInt32Value { value: 2 }));
        assert_eq!(f.add(&[(10, Some(0))], None, false), Ok(false));
        assert_eq!(f.add(&[(12, Some(0)), (13, Some(0))], None, false), Ok(true));
        assert!(!f.is_favorite(13));
        assert_eq!(f.add(&[(13, Some(0))], None, false), Err(FREE_LIMIT));
        assert_eq!(f.add(&[(13, Some(0))], None, true), Ok(true));
    }

    #[test]
    fn add_shows_a_hidden_favorites_server_again() {
        let mut f = Favorites { guild_visible: Some(BoolValue { value: false }), ..Default::default() };
        f.favorite_channels.insert(1, fav(REFERENCE_ORIGINAL, 1, 0));
        assert!(!f.guild_enabled());
        f.add(&[(2, None)], None, true).unwrap();
        assert_eq!(f.guild_visible, Some(BoolValue { value: true }));
        let mut g = Favorites::default();
        g.favorite_channels.insert(1, fav(REFERENCE_ORIGINAL, 1, 0));
        g.add(&[(2, None)], None, true).unwrap();
        assert_eq!(g.guild_visible, None);
    }

    #[test]
    fn categories_and_parents() {
        let mut f = Favorites::default();
        let cat = f.create_category("Jeux", 500);
        assert_eq!(f.add(&[(1, Some(0))], Some(cat), true), Ok(true));
        assert_eq!(f.get(1).unwrap().parent_id, cat);
        assert_eq!(f.add(&[(2, Some(0))], Some(999), true), Ok(true));
        assert_eq!(f.get(2).unwrap().parent_id, 0);
        assert_eq!(f.category_named("  jeux "), Some(cat));
        assert_eq!(f.add_to_category(&[(3, Some(0))], "JEUX", 777, true), Ok(true));
        assert_eq!(f.get(3).unwrap().parent_id, cat);
        assert!(f.get(777).is_none());
        assert_eq!(f.add_to_category(&[(4, Some(0))], "Autres", 778, true), Ok(true));
        assert!(f.get(778).unwrap().is_category());
        assert_eq!(f.get(4).unwrap().parent_id, 778);
        assert_eq!(f.count_against_limit(), 4);
        assert!(f.remove(cat));
        assert_eq!(f.get(1).unwrap().parent_id, 0);
        assert_eq!(f.get(3).unwrap().parent_id, 0);
        assert!(!f.remove(cat));
    }

    #[test]
    fn categories_do_not_count_against_the_limit() {
        let mut f = Favorites::default();
        f.add(&[(1, None), (2, None), (3, None)], None, false).unwrap();
        assert_eq!(f.limit_reached(CATEGORY, false), None);
        assert_eq!(f.limit_reached(REFERENCE_ORIGINAL, false), Some(FREE_LIMIT));
        for i in 0..197 {
            f.create_category("c", 1000 + i);
        }
        assert_eq!(f.limit_reached(CATEGORY, true), Some(MAX_ENTRIES));
    }

    #[test]
    fn prune_drops_unknown_and_hidden_but_keeps_threads() {
        let mut f = Favorites::default();
        f.favorite_channels.insert(1, FavoriteChannel { channel_type: None, ..fav(REFERENCE_ORIGINAL, 1, 0) });
        f.favorite_channels.insert(2, fav(REFERENCE_ORIGINAL, 2, 0));
        f.favorite_channels.insert(3, FavoriteChannel { channel_type: Some(UInt32Value { value: 11 }), ..fav(REFERENCE_ORIGINAL, 3, 0) });
        f.favorite_channels.insert(4, fav(REFERENCE_ORIGINAL, 4, 0));
        f.create_category("x", 9);
        f.prune(|id| match id {
            1 => Known::Visible { channel_type: 5 },
            4 => Known::Hidden,
            _ => Known::Missing,
        });
        assert_eq!(f.get(1).unwrap().channel_type, Some(UInt32Value { value: 5 }));
        assert!(f.get(2).is_none());
        assert!(f.get(3).is_some());
        assert!(f.get(4).is_none());
        assert!(f.get(9).is_some());
    }

    #[test]
    fn rows_put_uncategorized_first_then_categories_text_before_voice() {
        let mut f = Favorites::default();
        f.favorite_channels.insert(100, FavoriteChannel { nickname: "B".into(), ..fav(CATEGORY, 5, 0) });
        f.favorite_channels.insert(101, FavoriteChannel { nickname: "A".into(), ..fav(CATEGORY, 2, 0) });
        f.favorite_channels.insert(1, fav(REFERENCE_ORIGINAL, 9, 0));
        f.favorite_channels.insert(2, fav(REFERENCE_ORIGINAL, 1, 0));
        f.favorite_channels.insert(3, fav(REFERENCE_ORIGINAL, 1, 100));
        f.favorite_channels.insert(4, fav(REFERENCE_ORIGINAL, 3, 101));
        f.favorite_channels.insert(5, fav(REFERENCE_ORIGINAL, 2, 101));
        f.favorite_channels.insert(6, fav(REFERENCE_ORIGINAL, 7, 555));
        f.favorite_channels.insert(7, fav(REFERENCE_ORIGINAL, 4, 0));
        let ty = |id: u64| match id {
            5 => Some(2),
            7 => None,
            _ => Some(0),
        };
        assert_eq!(
            f.rows(ty),
            vec![Row::Channel(2), Row::Channel(6), Row::Channel(1), Row::Category(101), Row::Channel(4), Row::Channel(5), Row::Category(100), Row::Channel(3)]
        );
    }

    #[test]
    fn moves_rename_collapse_and_clear() {
        let mut f = Favorites::default();
        let cat = f.create_category("c", 50);
        f.add(&[(1, None), (2, None)], None, true).unwrap();
        f.apply_moves(&[Move { id: 1, position: Some(20), parent: Some(Some(cat)) }, Move { id: 2, position: None, parent: Some(Some(404)) }]);
        assert_eq!(f.get(1).unwrap().position, 20);
        assert_eq!(f.get(1).unwrap().parent_id, cat);
        assert_eq!(f.get(2).unwrap().parent_id, 0);
        assert!(f.rename(1, Some("général")));
        assert_eq!(f.nickname(1), Some("général"));
        assert!(f.rename(1, None));
        assert_eq!(f.nickname(1), None);
        assert!(f.set_collapsed(None, true));
        assert!(!f.set_collapsed(Some(cat), true));
        assert!(f.get(cat).unwrap().collapsed);
        f.clear();
        assert_eq!(f, Favorites::default());
    }

    #[test]
    fn category_ids_are_snowflakes_of_now() {
        assert_eq!(snowflake_at(DISCORD_EPOCH + 1), 1 << 22);
        assert_eq!(snowflake_at(0), 0);
    }
}

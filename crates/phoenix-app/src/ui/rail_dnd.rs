use std::cell::Cell;
use std::collections::HashSet;

use crate::{model::Id, ui::home::data::GuildFolder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailNode {
    Guild(Id),
    Folder(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailDrop {
    Before(RailNode),
    Combine(RailNode),
    FolderEnd(u64),
}

impl RailDrop {
    pub fn target(self) -> RailNode {
        match self {
            Self::Before(n) | Self::Combine(n) => n,
            Self::FolderEnd(f) => RailNode::Folder(f),
        }
    }
}

thread_local! {
    static OVER: Cell<Option<RailDrop>> = const { Cell::new(None) };
}

pub fn over() -> Option<RailDrop> {
    OVER.get()
}

pub fn set_over(at: Option<RailDrop>) -> bool {
    let changed = OVER.get() != at;
    OVER.set(at);
    changed
}

pub fn take_over() -> Option<RailDrop> {
    OVER.take()
}

pub fn zone(src: RailNode, target: RailNode, folder: Option<u64>, last_in_folder: bool, y: f32, h: f32) -> Option<RailDrop> {
    if src == target {
        return None;
    }
    let guild_src = matches!(src, RailNode::Guild(_));
    match folder {
        Some(f) => {
            if !guild_src {
                return None;
            }
            if last_in_folder && y >= h / 2. {
                return Some(RailDrop::FolderEnd(f));
            }
            Some(RailDrop::Before(target))
        }
        None if y < h / 2. - 6. => Some(RailDrop::Before(target)),
        None => guild_src.then_some(RailDrop::Combine(target)),
    }
}

fn lone(g: Id) -> GuildFolder {
    GuildFolder { id: None, name: None, color: None, guilds: vec![g] }
}

pub fn normalize(folders: &[GuildFolder], all: &[Id]) -> Vec<GuildFolder> {
    let listed: HashSet<Id> = folders.iter().flat_map(|f| f.guilds.iter().copied()).collect();
    let mut out: Vec<GuildFolder> = all.iter().filter(|g| !listed.contains(g)).map(|g| lone(*g)).collect();
    for f in folders {
        match f.id {
            Some(_) if !f.guilds.is_empty() => out.push(f.clone()),
            Some(_) => {}
            None => out.extend(f.guilds.iter().map(|g| lone(*g))),
        }
    }
    out
}

fn entry_of(entries: &[GuildFolder], node: RailNode) -> Option<usize> {
    match node {
        RailNode::Guild(g) => entries.iter().position(|e| e.guilds.contains(&g)),
        RailNode::Folder(f) => entries.iter().position(|e| e.id == Some(f)),
    }
}

fn in_folder(entries: &[GuildFolder], g: Id) -> bool {
    entry_of(entries, RailNode::Guild(g)).is_some_and(|i| entries[i].id.is_some())
}

pub fn apply_drop(entries: &mut Vec<GuildFolder>, src: RailNode, at: RailDrop, new_id: u64) -> bool {
    if at.target() == src {
        return false;
    }
    let mut next = entries.clone();
    let moved = match src {
        RailNode::Folder(_) => {
            let ok = match at {
                RailDrop::Before(RailNode::Folder(_)) => true,
                RailDrop::Before(RailNode::Guild(t)) => !in_folder(&next, t),
                _ => false,
            };
            if !ok {
                return false;
            }
            let Some(i) = entry_of(&next, src) else { return false };
            next.remove(i)
        }
        RailNode::Guild(g) => {
            if let RailDrop::Combine(RailNode::Guild(t)) = at
                && in_folder(&next, t)
            {
                return false;
            }
            let Some(i) = entry_of(&next, src) else { return false };
            next[i].guilds.retain(|x| *x != g);
            if next[i].guilds.is_empty() {
                next.remove(i);
            }
            lone(g)
        }
    };
    let Some(i) = entry_of(&next, at.target()) else { return false };
    match at {
        RailDrop::Before(RailNode::Guild(t)) if next[i].id.is_some() => {
            let pos = next[i].guilds.iter().position(|x| *x == t).unwrap_or(0);
            next[i].guilds.insert(pos, moved.guilds[0]);
        }
        RailDrop::Before(_) => next.insert(i, moved),
        RailDrop::Combine(RailNode::Guild(_)) => {
            next[i].id = Some(new_id);
            next[i].guilds.push(moved.guilds[0]);
        }
        RailDrop::Combine(RailNode::Folder(_)) | RailDrop::FolderEnd(_) => next[i].guilds.push(moved.guilds[0]),
    }
    *entries = next;
    true
}

pub fn fresh_folder_id(entries: &[GuildFolder], seed: u64) -> u64 {
    let mut x = seed | 1;
    loop {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let id = x & 0xFFFF_FFFF;
        if id != 0 && !entries.iter().any(|e| e.id == Some(id)) {
            return id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(id: u64, guilds: &[u64]) -> GuildFolder {
        GuildFolder { id: Some(id), name: Some("F".into()), color: Some(3), guilds: guilds.iter().map(|g| Id(*g)).collect() }
    }

    fn order(entries: &[GuildFolder]) -> Vec<(Option<u64>, Vec<u64>)> {
        entries.iter().map(|e| (e.id, e.guilds.iter().map(|g| g.0).collect())).collect()
    }

    fn sample() -> Vec<GuildFolder> {
        normalize(&[lone(Id(1)), folder(9, &[2, 3]), lone(Id(4))], &[Id(1), Id(2), Id(3), Id(4), Id(5)])
    }

    #[test]
    fn normalize_lists_unfiled_guilds_first_and_splits_lone_entries() {
        let n = normalize(&[GuildFolder { id: None, name: None, color: None, guilds: vec![Id(1), Id(2)] }, folder(7, &[])], &[Id(1), Id(2), Id(3)]);
        assert_eq!(order(&n), vec![(None, vec![3]), (None, vec![1]), (None, vec![2])]);
    }

    #[test]
    fn zones_follow_the_client_targets() {
        let (g, h) = (RailNode::Guild(Id(1)), RailNode::Guild(Id(2)));
        assert_eq!(zone(g, h, None, false, 10., 48.), Some(RailDrop::Before(h)));
        assert_eq!(zone(g, h, None, false, 30., 48.), Some(RailDrop::Combine(h)));
        assert_eq!(zone(g, g, None, false, 30., 48.), None);
        let f = RailNode::Folder(9);
        assert_eq!(zone(f, h, None, false, 30., 48.), None);
        assert_eq!(zone(f, h, None, false, 5., 48.), Some(RailDrop::Before(h)));
        assert_eq!(zone(g, h, Some(9), false, 40., 48.), Some(RailDrop::Before(h)));
        assert_eq!(zone(g, h, Some(9), true, 40., 48.), Some(RailDrop::FolderEnd(9)));
        assert_eq!(zone(f, h, Some(9), true, 40., 48.), None);
    }

    #[test]
    fn moves_before_a_lone_guild_and_into_a_folder() {
        let mut e = sample();
        assert!(apply_drop(&mut e, RailNode::Guild(Id(4)), RailDrop::Before(RailNode::Guild(Id(5))), 1));
        assert_eq!(order(&e), vec![(None, vec![4]), (None, vec![5]), (None, vec![1]), (Some(9), vec![2, 3])]);
        assert!(apply_drop(&mut e, RailNode::Guild(Id(5)), RailDrop::Before(RailNode::Guild(Id(3))), 1));
        assert_eq!(order(&e), vec![(None, vec![4]), (None, vec![1]), (Some(9), vec![2, 5, 3])]);
        assert!(apply_drop(&mut e, RailNode::Guild(Id(2)), RailDrop::Before(RailNode::Guild(Id(4))), 1));
        assert_eq!(order(&e), vec![(None, vec![2]), (None, vec![4]), (None, vec![1]), (Some(9), vec![5, 3])]);
    }

    #[test]
    fn combining_two_guilds_creates_a_folder() {
        let mut e = sample();
        assert!(apply_drop(&mut e, RailNode::Guild(Id(5)), RailDrop::Combine(RailNode::Guild(Id(1))), 77));
        assert_eq!(order(&e), vec![(Some(77), vec![1, 5]), (Some(9), vec![2, 3]), (None, vec![4])]);
        assert_eq!((e[0].name.clone(), e[0].color), (None, None));
        assert!(apply_drop(&mut e, RailNode::Guild(Id(4)), RailDrop::Combine(RailNode::Folder(9)), 1));
        assert!(apply_drop(&mut e, RailNode::Guild(Id(1)), RailDrop::FolderEnd(9), 1));
        assert_eq!(order(&e), vec![(Some(77), vec![5]), (Some(9), vec![2, 3, 4, 1])]);
    }

    #[test]
    fn folders_move_as_a_block_and_never_nest() {
        let mut e = sample();
        assert!(apply_drop(&mut e, RailNode::Folder(9), RailDrop::Before(RailNode::Guild(Id(5))), 1));
        assert_eq!(order(&e), vec![(Some(9), vec![2, 3]), (None, vec![5]), (None, vec![1]), (None, vec![4])]);
        let before = e.clone();
        assert!(!apply_drop(&mut e, RailNode::Folder(9), RailDrop::Combine(RailNode::Guild(Id(5))), 1));
        assert!(!apply_drop(&mut e, RailNode::Folder(9), RailDrop::Before(RailNode::Guild(Id(3))), 1));
        assert!(!apply_drop(&mut e, RailNode::Guild(Id(5)), RailDrop::Combine(RailNode::Guild(Id(2))), 1));
        assert!(!apply_drop(&mut e, RailNode::Guild(Id(5)), RailDrop::Before(RailNode::Guild(Id(5))), 1));
        assert_eq!(e, before);
    }

    #[test]
    fn emptied_folders_disappear() {
        let mut e = normalize(&[folder(9, &[2]), lone(Id(1))], &[Id(1), Id(2)]);
        assert!(apply_drop(&mut e, RailNode::Guild(Id(2)), RailDrop::Before(RailNode::Guild(Id(1))), 1));
        assert_eq!(order(&e), vec![(None, vec![2]), (None, vec![1])]);
    }

    #[test]
    fn fresh_ids_are_32_bit_and_unused() {
        let e = sample();
        let id = fresh_folder_id(&e, 12345);
        assert!(id > 0 && id <= u64::from(u32::MAX) && id != 9);
    }
}

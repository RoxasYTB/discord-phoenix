use std::{cell::RefCell, collections::HashMap};

use crate::model::{Id, Member};

const SCAN: usize = 48;

#[derive(Default)]
struct Index {
    key: (usize, u64, u64),
    pos: HashMap<u64, usize>,
}

thread_local! {
    static IDX: RefCell<HashMap<u64, Index>> = RefCell::new(HashMap::new());
}

fn key(ms: &[Member]) -> (usize, u64, u64) {
    (ms.len(), ms.first().map_or(0, |m| m.user.0), ms.last().map_or(0, |m| m.user.0))
}

fn rebuild(ix: &mut Index, ms: &[Member]) {
    ix.key = key(ms);
    ix.pos.clear();
    ix.pos.extend(ms.iter().enumerate().map(|(i, m)| (m.user.0, i)));
}

pub fn invalidate(guild: Id) {
    IDX.with_borrow_mut(|all| {
        all.remove(&guild.0);
    });
}

pub fn find(ms: &[Member], guild: Id, user: Id) -> Option<&Member> {
    if ms.len() < SCAN {
        return ms.iter().find(|m| m.user == user);
    }
    IDX.with_borrow_mut(|all| {
        let ix = all.entry(guild.0).or_default();
        if ix.key != key(ms) {
            rebuild(ix, ms);
        }
        match ix.pos.get(&user.0).map(|&i| ms.get(i)) {
            Some(Some(m)) if m.user == user => Some(m),
            Some(_) => {
                rebuild(ix, ms);
                ix.pos.get(&user.0).and_then(|&i| ms.get(i)).filter(|m| m.user == user)
            }
            None => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(u: u64) -> Member {
        Member {
            user: Id(u),
            roles: Vec::new(),
            presence: Default::default(),
            activity: None,
            nick: None,
            avatar: None,
            decoration: None,
            nameplate: None,
            joined_at: None,
            rejoined: false,
            premium_since: None,
        }
    }

    fn list(ids: impl Iterator<Item = u64>) -> Vec<Member> {
        ids.map(member).collect()
    }

    #[test]
    fn finds_and_follows_changes() {
        let mut ms = list(1..=100);
        assert_eq!(find(&ms, Id(9), Id(50)).map(|m| m.user), Some(Id(50)));
        assert!(find(&ms, Id(9), Id(500)).is_none());
        ms.swap(10, 60);
        assert_eq!(find(&ms, Id(9), Id(11)).map(|m| m.user), Some(Id(11)));
        ms.push(member(500));
        assert_eq!(find(&ms, Id(9), Id(500)).map(|m| m.user), Some(Id(500)));
    }

    #[test]
    fn replaced_list_with_same_ends_finds_new_member() {
        let ms = list(1..=100);
        assert!(find(&ms, Id(8), Id(700)).is_none());
        let mut next = ms.clone();
        next[50] = member(700);
        invalidate(Id(8));
        assert_eq!(find(&next, Id(8), Id(700)).map(|m| m.user), Some(Id(700)));
    }
}

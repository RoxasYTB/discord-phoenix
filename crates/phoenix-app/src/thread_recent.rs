use std::collections::HashMap;

use gpui::SharedString;
use parking_lot::RwLock;

use crate::model::{Id, ImageRef};

#[derive(Clone, Debug, PartialEq)]
pub struct Recent {
    pub author: Id,
    pub name: SharedString,
    pub avatar: ImageRef,
    pub content: SharedString,
    pub ts: i64,
}

static STORE: RwLock<Option<HashMap<Id, Recent>>> = RwLock::new(None);

pub fn set(thread: Id, r: Recent) {
    let mut s = STORE.write();
    let map = s.get_or_insert_with(HashMap::new);
    if map.get(&thread).is_none_or(|o| o.ts <= r.ts) {
        map.insert(thread, r);
    }
}

pub fn get(thread: Id) -> Option<Recent> {
    STORE.read().as_ref()?.get(&thread).cloned()
}

pub fn ago(secs: i64) -> SharedString {
    let s = secs.max(0);
    match s {
        0..3600 => crate::t!("thread.ago_min", n = (s / 60).max(1)),
        3600..86_400 => crate::t!("thread.ago_h", n = s / 3600),
        86_400..2_592_000 => crate::t!("thread.ago_d", n = s / 86_400),
        2_592_000..31_104_000 => crate::t!("thread.ago_30d"),
        _ => crate::t!("thread.ago_1y"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_wins() {
        let r = |ts| Recent { author: Id(1), name: "a".into(), avatar: ImageRef::Default(0), content: format!("{ts}").into(), ts };
        set(Id(99), r(10));
        set(Id(99), r(5));
        assert_eq!(get(Id(99)).unwrap().ts, 10);
        set(Id(99), r(20));
        assert_eq!(get(Id(99)).unwrap().ts, 20);
    }
}

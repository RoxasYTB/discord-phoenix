use std::cell::Cell;

use serde_json::{Map, Value, json};

use crate::model::{ChannelKind, Id};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Text,
    Vocal,
    Category,
}

pub fn class(kind: ChannelKind) -> Class {
    match kind {
        ChannelKind::Voice | ChannelKind::Stage => Class::Vocal,
        ChannelKind::Category => Class::Category,
        _ => Class::Text,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub id: Id,
    pub kind: ChannelKind,
    pub parent: Option<Id>,
    pub position: i32,
}

pub fn ordered(slots: &[Slot], of: Class) -> Vec<Slot> {
    let mut cats: Vec<&Slot> = slots.iter().filter(|s| class(s.kind) == Class::Category).collect();
    cats.sort_by_key(|s| (s.position, s.id.0));
    let cat_ix = |p: Option<Id>| p.and_then(|p| cats.iter().position(|c| c.id == p)).map_or(-1, |i| i as i64);
    let mut out: Vec<Slot> = slots.iter().filter(|s| class(s.kind) == of).copied().collect();
    if of == Class::Category {
        out.sort_by_key(|s| (s.position, s.id.0));
    } else {
        out.sort_by_key(|s| (cat_ix(s.parent), s.position, s.id.0));
    }
    out
}

pub fn can_drop(slots: &[Slot], dragged: Id, target: Id) -> bool {
    let find = |id: Id| slots.iter().find(|s| s.id == id);
    match (find(dragged), find(target)) {
        (Some(d), Some(t)) => d.id != t.id && class(d.kind) == class(t.kind),
        _ => false,
    }
}

pub fn lands_after(slots: &[Slot], dragged: Id, target: Id) -> bool {
    let Some(d) = slots.iter().find(|s| s.id == dragged) else { return false };
    let list = ordered(slots, class(d.kind));
    let ix = |id: Id| list.iter().position(|s| s.id == id);
    matches!((ix(dragged), ix(target)), (Some(a), Some(b)) if a < b)
}

pub fn reorder_body(slots: &[Slot], dragged: Id, target: Id) -> Option<Value> {
    if !can_drop(slots, dragged, target) {
        return None;
    }
    let d = *slots.iter().find(|s| s.id == dragged)?;
    let t = *slots.iter().find(|s| s.id == target)?;
    let mut list = ordered(slots, class(d.kind));
    let a = list.iter().position(|s| s.id == dragged)?;
    let b = list.iter().position(|s| s.id == target)?;
    let item = list.remove(a);
    list.insert(b, item);
    let mut out: Vec<Map<String, Value>> = Vec::new();
    for (i, s) in list.iter().enumerate() {
        if s.position != i as i32 {
            let mut m = Map::new();
            m.insert("id".into(), json!(s.id.0.to_string()));
            m.insert("position".into(), json!(i));
            out.push(m);
        }
    }
    if class(d.kind) != Class::Category && d.parent != t.parent {
        let parent = t.parent.map_or(Value::Null, |p| json!(p.0.to_string()));
        let id = json!(d.id.0.to_string());
        match out.iter_mut().find(|m| m.get("id") == Some(&id)) {
            Some(m) => {
                m.insert("parent_id".into(), parent);
            }
            None => {
                let mut m = Map::new();
                m.insert("id".into(), id);
                m.insert("parent_id".into(), parent);
                out.push(m);
            }
        }
    }
    (!out.is_empty()).then(|| Value::Array(out.into_iter().map(Value::Object).collect()))
}

thread_local! {
    static OVER: Cell<Option<(Id, Id)>> = const { Cell::new(None) };
}

pub fn over() -> Option<(Id, Id)> {
    OVER.get()
}

pub fn set_over(v: Option<(Id, Id)>) -> bool {
    let changed = OVER.get() != v;
    OVER.set(v);
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: u64, kind: ChannelKind, parent: Option<u64>, position: i32) -> Slot {
        Slot { id: Id(id), kind, parent: parent.map(Id), position }
    }

    fn guild() -> Vec<Slot> {
        use ChannelKind::*;
        vec![
            s(1, Category, None, 0),
            s(2, Category, None, 1),
            s(10, Text, None, 0),
            s(11, Text, Some(1), 1),
            s(12, Announcement, Some(1), 2),
            s(13, Voice, Some(1), 0),
            s(14, Text, Some(2), 3),
            s(15, Stage, Some(2), 1),
        ]
    }

    #[test]
    fn orders_by_category_then_position() {
        let ids = |c| ordered(&guild(), c).iter().map(|s| s.id.0).collect::<Vec<_>>();
        assert_eq!(ids(Class::Text), vec![10, 11, 12, 14]);
        assert_eq!(ids(Class::Vocal), vec![13, 15]);
        assert_eq!(ids(Class::Category), vec![1, 2]);
    }

    #[test]
    fn moving_up_within_a_category_sends_position_deltas() {
        let b = reorder_body(&guild(), Id(12), Id(11)).unwrap();
        assert_eq!(b, json!([{"id": "12", "position": 1}, {"id": "11", "position": 2}]));
        assert!(!lands_after(&guild(), Id(12), Id(11)));
    }

    #[test]
    fn moving_into_another_category_sets_the_parent() {
        let b = reorder_body(&guild(), Id(10), Id(14)).unwrap();
        assert_eq!(
            b,
            json!([{"id": "11", "position": 0}, {"id": "12", "position": 1}, {"id": "14", "position": 2}, {"id": "10", "position": 3, "parent_id": "2"}])
        );
        assert!(lands_after(&guild(), Id(10), Id(14)));
        let up = reorder_body(&guild(), Id(14), Id(10)).unwrap();
        assert_eq!(up, json!([{"id": "14", "position": 0, "parent_id": null}, {"id": "10", "position": 1}, {"id": "11", "position": 2}, {"id": "12", "position": 3}]));
    }

    #[test]
    fn only_same_class_channels_swap() {
        assert_eq!(reorder_body(&guild(), Id(13), Id(11)), None);
        assert_eq!(reorder_body(&guild(), Id(1), Id(11)), None);
        assert_eq!(reorder_body(&guild(), Id(11), Id(11)), None);
        assert_eq!(reorder_body(&guild(), Id(2), Id(1)).unwrap(), json!([{"id": "2", "position": 0}, {"id": "1", "position": 1}]));
        assert_eq!(reorder_body(&guild(), Id(15), Id(13)).unwrap(), json!([{"id": "15", "position": 0, "parent_id": "1"}, {"id": "13", "position": 1}]));
    }
}

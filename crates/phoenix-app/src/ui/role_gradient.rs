use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::RwLock;

use crate::model::Id;

fn table() -> &'static RwLock<HashMap<Id, [u32; 3]>> {
    static T: OnceLock<RwLock<HashMap<Id, [u32; 3]>>> = OnceLock::new();
    T.get_or_init(Default::default)
}

pub fn set(id: Id, primary: u32, secondary: Option<u32>, tertiary: Option<u32>) {
    match secondary {
        Some(s) => {
            table().write().insert(id, [primary, s, tertiary.unwrap_or(primary)]);
        }
        None => {
            table().write().remove(&id);
        }
    }
}

pub fn get(id: Id) -> Option<[u32; 3]> {
    table().read().get(&id).copied()
}

pub fn for_member(app: &crate::app::PhoenixApp, guild: Id, m: &crate::model::Member) -> Option<[u32; 3]> {
    let roles = app.roles.get(&guild)?;
    let top = roles.iter().filter(|r| m.roles.contains(&r.id) && r.color.is_some()).max_by_key(|r| r.position)?;
    get(top.id)
}

pub fn for_author(app: &crate::app::PhoenixApp, user: Id) -> Option<[u32; 3]> {
    if app.prefs.local.pick("roles", 0) != 0 {
        return None;
    }
    let g = app.selected_guild?;
    let m = crate::member_index::find(app.members.get(&g)?, g, user)?;
    for_member(app, g, m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_colour_goes_back() {
        set(Id(91), 0x369876, Some(0x71ff9e), None);
        assert_eq!(get(Id(91)), Some([0x369876, 0x71ff9e, 0x369876]));
        set(Id(91), 0x369876, None, None);
        assert_eq!(get(Id(91)), None);
    }
}

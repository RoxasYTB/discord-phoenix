use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::RwLock;

use crate::model::{Id, Role};

pub type RoleGradient = [u32; 3];

fn table() -> &'static RwLock<HashMap<Id, RoleGradient>> {
    static T: OnceLock<RwLock<HashMap<Id, RoleGradient>>> = OnceLock::new();
    T.get_or_init(Default::default)
}

pub fn set(id: Id, primary: u32, secondary: Option<u32>, tertiary: Option<u32>) {
    let secondary = secondary.filter(|&s| s != 0);
    let tertiary = tertiary.filter(|&t| t != 0);
    match secondary {
        Some(s) if primary != 0 => {
            table().write().insert(id, [primary, s, tertiary.unwrap_or(primary)]);
        }
        _ => {
            table().write().remove(&id);
        }
    }
}

pub fn get(id: Id) -> Option<RoleGradient> {
    table().read().get(&id).copied()
}

pub fn highest_colored_role<'a>(roles: &'a [Role], member_roles: &[Id]) -> Option<&'a Role> {
    roles
        .iter()
        .filter(|r| member_roles.contains(&r.id) && r.color.is_some_and(|c| c != 0))
        .max_by_key(|r| (r.position, std::cmp::Reverse(r.id.0)))
}

pub fn style_for_roles(roles: &[Role], member_roles: &[Id]) -> (Option<u32>, Option<RoleGradient>) {
    let Some(top) = highest_colored_role(roles, member_roles) else {
        return (None, None);
    };
    (top.color.map(crate::theme::user_color), get(top.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(id: u64, position: i32, color: Option<u32>) -> Role {
        Role {
            id: Id(id),
            name: format!("r{id}").into(),
            color,
            position,
            hoist: false,
            icon: None,
            managed: false,
            connection: false,
        }
    }

    #[test]
    fn two_colour_goes_back() {
        set(Id(91), 0x369876, Some(0x71ff9e), None);
        assert_eq!(get(Id(91)), Some([0x369876, 0x71ff9e, 0x369876]));
        set(Id(91), 0x369876, None, None);
        assert_eq!(get(Id(91)), None);
    }

    #[test]
    fn zero_primary_or_secondary_clears_gradient() {
        set(Id(92), 0x112233, Some(0x445566), None);
        assert!(get(Id(92)).is_some());
        set(Id(92), 0, Some(0x445566), None);
        assert_eq!(get(Id(92)), None);
        set(Id(92), 0x112233, Some(0), None);
        assert_eq!(get(Id(92)), None);
    }

    #[test]
    fn highest_colored_role_wins_over_lower_gradient_and_skips_zero() {
        let top_uncolored = role(501, 20, None);
        let top_zero = role(502, 15, Some(0));
        let mod_blue = role(503, 10, Some(0x3498DB));
        let low_grad = role(504, 5, Some(0x111111));
        set(Id(503), 0x3498DB, None, None);
        set(Id(504), 0x111111, Some(0x222222), None);
        let roles = [low_grad, mod_blue, top_zero, top_uncolored];
        let member_roles = [Id(501), Id(502), Id(503), Id(504)];
        let (color, gradient) = style_for_roles(&roles, &member_roles);
        assert_eq!(color, Some(crate::theme::user_color(0x3498DB)));
        assert_eq!(gradient, None);
        set(Id(504), 0, None, None);
    }

    #[test]
    fn equal_position_picks_smallest_id() {
        let a = role(602, 10, Some(0xAA0000));
        let b = role(601, 10, Some(0x00BB00));
        let c = role(603, 10, Some(0x0000CC));
        let roles = [a, b, c];
        let member_roles = [Id(601), Id(602), Id(603)];
        let (color, _) = style_for_roles(&roles, &member_roles);
        assert_eq!(color, Some(crate::theme::user_color(0x00BB00)));
    }

    #[test]
    fn high_gradient_role_beats_lower_plain_role() {
        set(Id(701), 0x112233, Some(0x445566), Some(0x778899));
        let roles = [role(700, 3, Some(0xFF0000)), role(701, 8, Some(0x112233))];
        let (color, gradient) = style_for_roles(&roles, &[Id(700), Id(701)]);
        assert_eq!(color, Some(crate::theme::user_color(0x112233)));
        assert_eq!(gradient, Some([0x112233, 0x445566, 0x778899]));
        set(Id(701), 0, None, None);
    }

    #[test]
    fn zero_or_missing_colour_never_paints_black() {
        let roles = [role(801, 9, Some(0)), role(802, 7, None)];
        assert_eq!(style_for_roles(&roles, &[Id(801), Id(802)]), (None, None));
    }

    #[test]
    fn roles_missing_from_cache_are_ignored() {
        let roles = [role(901, 2, Some(0x3498DB))];
        let (color, _) = style_for_roles(&roles, &[Id(999), Id(901), Id(998)]);
        assert_eq!(color, Some(crate::theme::user_color(0x3498DB)));
        assert_eq!(style_for_roles(&roles, &[Id(999)]), (None, None));
        assert_eq!(style_for_roles(&[], &[Id(901)]), (None, None));
    }

    #[test]
    fn equal_position_ignores_cache_order() {
        let roles = [role(1003, 4, Some(0x0000CC)), role(1001, 4, Some(0x00BB00)), role(1002, 4, Some(0xAA0000))];
        let mut rev = roles.clone();
        rev.reverse();
        let ids = [Id(1003), Id(1002), Id(1001)];
        assert_eq!(style_for_roles(&roles, &ids), style_for_roles(&rev, &ids));
        assert_eq!(style_for_roles(&roles, &ids).0, Some(crate::theme::user_color(0x00BB00)));
    }
}

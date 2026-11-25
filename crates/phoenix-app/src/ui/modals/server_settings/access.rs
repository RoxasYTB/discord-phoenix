use gpui::{AnyElement, FontWeight, HighlightStyle, IntoElement, ParentElement, SharedString, StyledText, Styled, div, prelude::*};
use serde_json::Value;

use super::{
    State,
    kit::{Cx, labels, text},
};
use crate::{
    app::PhoenixApp,
    theme::{
        c,
        generated::{global, idx},
        tok, z,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinType {
    Invite,
    Apply,
    Discoverable,
}

pub const AGE_RESTRICTED: u64 = 3;

fn has(guild: &Value, feature: &str) -> bool {
    guild["features"].as_array().is_some_and(|a| a.iter().any(|v| v.as_str() == Some(feature)))
}

pub fn join_type(guild: &Value) -> JoinType {
    if has(guild, "DISCOVERABLE") {
        JoinType::Discoverable
    } else if has(guild, "MEMBER_VERIFICATION_GATE_ENABLED") && has(guild, "MEMBER_VERIFICATION_MANUAL_APPROVAL") {
        JoinType::Apply
    } else {
        JoinType::Invite
    }
}

pub fn rules_required(guild: &Value) -> bool {
    has(guild, "MEMBER_VERIFICATION_GATE_ENABLED")
}

fn content_level(guild: &Value, key: &str) -> Option<u64> {
    guild.get(key).and_then(Value::as_u64)
}

pub fn age_restricted(guild: &Value) -> bool {
    content_level(guild, "owner_configured_content_level") == Some(AGE_RESTRICTED)
}

pub fn age_locked(guild: &Value) -> bool {
    join_type(guild) == JoinType::Discoverable
        || (content_level(guild, "nsfw_level") == Some(AGE_RESTRICTED) && content_level(guild, "owner_configured_content_level") != Some(AGE_RESTRICTED))
}

pub fn options(guild: &Value, admin: bool) -> [(JoinType, bool); 3] {
    let active = join_type(guild);
    let community = has(guild, "COMMUNITY");
    let locked = active == JoinType::Discoverable && !admin;
    [(JoinType::Invite, !locked), (JoinType::Apply, !locked), (JoinType::Discoverable, community && (active == JoinType::Discoverable || admin))]
}

fn with_link(body: SharedString, link: SharedString) -> StyledText {
    let start = body.len();
    let full: SharedString = format!("{body}{link}").into();
    let end = full.len();
    StyledText::new(full).with_highlights([(start..end, HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() })])
}

fn card(kind: JoinType, active: bool, enabled: bool) -> AnyElement {
    let (icon, title, body) = match kind {
        JoinType::Invite => ("lock", crate::t!("gs.access.invite"), crate::t!("gs.access.invite_desc")),
        JoinType::Apply => ("server/envelope", crate::t!("gs.access.apply"), crate::t!("gs.access.apply_desc")),
        JoinType::Discoverable => ("settings/globe", crate::t!("gs.access.discoverable"), crate::t!("gs.access.discoverable_desc")),
    };
    let fg = if active { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT };
    div()
        .id(("gs-access-type", kind as usize))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(z(8.))
        .py(z(12.))
        .px(z(16.))
        .rounded(z(8.))
        .border_1()
        .border_color(if active { c(tok(idx::BORDER_NORMAL)) } else { gpui::transparent_black() })
        .text_center()
        .when(active, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .when(!active && enabled, |d| d.hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))))
        .when(!enabled, |d| d.opacity(0.5).cursor_not_allowed())
        .child(gpui::svg().path(crate::icons::path(icon)).flex_none().size(z(24.)).text_color(c(tok(fg))))
        .child(text(title, 16., 20., FontWeight::SEMIBOLD, fg))
        .child(text(body, 12., 16., FontWeight::MEDIUM, fg))
        .into_any_element()
}

fn switch_row(id: &'static str, label: SharedString, desc: impl IntoElement, on: bool, enabled: bool) -> AnyElement {
    div()
        .id(id)
        .flex()
        .gap(z(24.))
        .when(!enabled, |d| d.opacity(0.5))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pt(z(2.))
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(labels(label, None))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(desc)),
        )
        .child(crate::ui::controls::switch((id, 1usize), on))
        .into_any_element()
}

fn divider() -> AnyElement {
    div().flex_none().h(z(1.)).w_full().bg(c(tok(idx::BORDER_SUBTLE))).into_any_element()
}

fn rules_block(s: &State) -> AnyElement {
    let guild = s.guild_json();
    let on = rules_required(guild);
    let Some(form) = s.get("verification") else {
        return text(crate::t!("common.loading"), 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED).into_any_element();
    };
    let rules = crate::ui::join_flow::parse_member_verification(form).rules;
    div()
        .flex()
        .flex_col()
        .child(switch_row("gs-access-rules", crate::t!("gs.access.rules_title"), crate::t!("gs.access.rules_desc"), on, true))
        .child(
            div()
                .mt(z(24.))
                .p(z(16.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                .when(!on, |d| d.opacity(0.5))
                .child(text(crate::t!("gs.access.rules"), 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED))
                .child(div().my(z(12.)).flex().flex_col().gap(z(12.)).children(rules.into_iter().map(|r| {
                    div()
                        .rounded(z(global::RADIUS_SM))
                        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                        .pl(z(12.))
                        .pr(z(16.))
                        .py(z(11.))
                        .text_size(z(16.))
                        .line_height(z(22.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .whitespace_normal()
                        .child(SharedString::from(r))
                }))),
        )
        .into_any_element()
}

pub fn render(app: &PhoenixApp, s: &State, _cx: &mut Cx) -> AnyElement {
    let guild = s.guild_json();
    let head = text(crate::t!("menu.gs.access"), 20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG);
    if guild.is_null() {
        return div().flex().flex_col().gap(z(32.)).child(head).into_any_element();
    }
    let admin = app.backend.guild_permissions(s.guild) & crate::actions::ADMINISTRATOR != 0;
    let active = join_type(guild);
    let types = div()
        .flex()
        .flex_col()
        .child(text(crate::t!("gs.access.how"), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG))
        .child(
            div()
                .mt(z(8.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_SUBTLE)))
                .child(with_link(crate::t!("gs.access.how_desc"), crate::t!("gs.access.learn_more"))),
        )
        .child(
            div()
                .mt(z(global::SPACE_16))
                .p(z(global::SPACE_8))
                .flex()
                .gap(z(global::SPACE_8))
                .rounded(z(global::RADIUS_MD))
                .border_1()
                .border_color(c(tok(idx::BORDER_MUTED)))
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                .children(options(guild, admin).into_iter().map(|(k, enabled)| card(k, k == active, enabled))),
        );
    let age = switch_row(
        "gs-access-age",
        crate::t!("gs.access.age"),
        with_link(crate::t!("gs.access.age_desc"), crate::t!("gs.access.age_more")),
        age_restricted(guild),
        !age_locked(guild),
    );
    let below = match active {
        JoinType::Invite => Some(rules_block(s)),
        JoinType::Apply | JoinType::Discoverable => None,
    };
    div()
        .flex()
        .flex_col()
        .gap(z(32.))
        .child(head)
        .child(types)
        .child(divider())
        .child(age)
        .child(divider())
        .children(below)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn join_type_follows_the_guild_features() {
        assert_eq!(join_type(&json!({ "features": [] })), JoinType::Invite);
        assert_eq!(join_type(&json!({ "features": ["MEMBER_VERIFICATION_GATE_ENABLED"] })), JoinType::Invite);
        assert_eq!(join_type(&json!({ "features": ["MEMBER_VERIFICATION_GATE_ENABLED", "MEMBER_VERIFICATION_MANUAL_APPROVAL"] })), JoinType::Apply);
        assert_eq!(join_type(&json!({ "features": ["DISCOVERABLE", "MEMBER_VERIFICATION_MANUAL_APPROVAL"] })), JoinType::Discoverable);
        assert!(rules_required(&json!({ "features": ["MEMBER_VERIFICATION_GATE_ENABLED"] })));
    }

    #[test]
    fn discovery_needs_community_and_admins() {
        let plain = json!({ "features": [] });
        assert_eq!(options(&plain, true).map(|o| o.1), [true, true, false]);
        let community = json!({ "features": ["COMMUNITY"] });
        assert_eq!(options(&community, false).map(|o| o.1), [true, true, false]);
        assert_eq!(options(&community, true).map(|o| o.1), [true, true, true]);
        let listed = json!({ "features": ["COMMUNITY", "DISCOVERABLE"] });
        assert_eq!(options(&listed, false).map(|o| o.1), [false, false, true]);
    }

    #[test]
    fn age_switch_state() {
        assert!(age_restricted(&json!({ "owner_configured_content_level": 3 })));
        assert!(!age_restricted(&json!({ "owner_configured_content_level": 0, "nsfw_level": 3 })));
        assert!(age_locked(&json!({ "features": [], "nsfw_level": 3, "owner_configured_content_level": 0 })));
        assert!(age_locked(&json!({ "features": ["DISCOVERABLE"] })));
        assert!(!age_locked(&json!({ "features": [], "nsfw_level": 0 })));
    }
}

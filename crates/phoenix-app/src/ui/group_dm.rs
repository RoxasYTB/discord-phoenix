use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    model::{Id, ImageRef, Member},
    theme::{Palette, c, m, z},
    ui::avatar,
};

pub fn members(app: &PhoenixApp, ch: Id) -> &[Id] {
    app.nav.extras.get(&ch).map(|e| e.members.as_slice()).unwrap_or_default()
}

pub fn group_avatar(app: &PhoenixApp, ch: Id, size: f32, ring: Hsla) -> AnyElement {
    let ex = app.nav.extras.get(&ch);
    if let Some(ImageRef::Url(u)) = ex.and_then(|e| e.group_icon.clone()) {
        return gpui::img(u).size(gpui::px(size)).rounded_full().flex_none().into_any_element();
    }
    let ids = ex.map(|e| e.members.as_slice()).unwrap_or_default();
    let users: Vec<_> = ids.iter().filter_map(|u| app.users.get(u)).take(2).collect();
    if users.len() < 2 {
        return avatar(users.first().copied().or(Some(&app.me)), size);
    }
    let s = size * 2. / 3.;
    let off = size / 3.;
    let gap = 3. * crate::theme::zoom();
    div()
        .relative()
        .flex_none()
        .size(gpui::px(size))
        .child(div().absolute().left_0().top_0().child(avatar(Some(users[0]), s)))
        .child(
            div()
                .absolute()
                .left(gpui::px(off - gap))
                .top(gpui::px(off - gap))
                .size(gpui::px(s + 2. * gap))
                .rounded_full()
                .bg(ring),
        )
        .child(div().absolute().left(gpui::px(off)).top(gpui::px(off)).child(avatar(Some(users[1]), s)))
        .into_any_element()
}

pub fn list_status(app: &PhoenixApp, ch: Id) -> Option<crate::model::Presence> {
    members(app, ch).iter().any(|u| app.presence_of(*u) == crate::model::Presence::Online).then_some(crate::model::Presence::Online)
}

pub fn group_avatar_with_status(app: &PhoenixApp, ch: Id, size: f32, ring: Hsla) -> AnyElement {
    let pile = group_avatar(app, ch, size, ring);
    let Some(p) = list_status(app, ch) else { return pile };
    let (cx, r, s) = crate::ui::status_svg::geometry(size);
    div()
        .relative()
        .flex_none()
        .size(gpui::px(size))
        .child(pile)
        .child(div().absolute().left(gpui::px(cx - r)).top(gpui::px(cx - r)).size(gpui::px(2. * r)).rounded_full().bg(ring))
        .child(div().absolute().left(gpui::px(cx - s / 2.)).top(gpui::px(cx - s / 2.)).child(crate::ui::status_dot(p, s, ring)))
        .into_any_element()
}

pub fn members_panel(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let ch = app.selected_channel?;
    if !app.show_members || app.channels.get(&ch).is_none_or(|c| c.guild.is_some()) || !crate::ui::dm_profile_panel::is_group_dm(app, ch) {
        return None;
    }
    let ex = app.nav.extras.get(&ch)?;
    let mut ids: Vec<Id> = ex.members.clone();
    if !ids.contains(&app.me.id) {
        ids.push(app.me.id);
    }
    let key = |u: &Id| app.users.get(u).map(|u| u.name.to_lowercase()).unwrap_or_default();
    ids.sort_by_key(key);
    let entity = cx.entity();
    let header: SharedString = crate::t!("members.group_header", n = ids.len());
    let rows = ids.iter().map(|u| {
        let mem = Member {
            user: *u,
            roles: Vec::new(),
            presence: app.presence_of(*u),
            activity: app.nav.activities.get(u).cloned(),
            nick: None,
            avatar: None,
            decoration: None,
            nameplate: None,
            joined_at: None,
            rejoined: false,
            premium_since: None,
        };
        crate::ui::members::member_row_in(app, &entity, None, &mem, None, ex.owner == Some(*u))
    });
    Some(
        div()
            .flex_none()
            .h_full()
            .bg(c(Palette::BG_SECONDARY))
            .border_l_1()
            .border_color(c(Palette::DIVIDER))
            .child(
                div()
                    .id("group-members")
                    .w(z(m::member_list_w()))
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .pb(z(10.))
                    .child(crate::ui::members::header(header))
                    .children(rows),
            )
            .into_any_element(),
    )
}

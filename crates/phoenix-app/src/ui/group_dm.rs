//! Group DM pieces: the group avatar (custom icon or the client's generated
//! diagonal face pile) and the right-hand member list.
//!
//! Dump `vanilla-group-dm` (2026-11-23): header #1065 `container_pfDcF-
//! gdmIcon_9yrrgG` 24px (margin-inline-end 4px), intro #1104
//! `editableGdmIcon_5yEg41` 80px, DM list #800 32px. Without a custom icon
//! the client draws two recipient avatars through the masks
//! `svg-mask-diagonal-facepile[-back]-<size>`: each avatar is 2/3 of the
//! box, the back one at (0,0), the front one at (1/3,1/3), and the back one
//! is cut by a circle 3px wider than the front one (r .4583 at 24px, .3708
//! at 80px: (r - 1/3) × size = 3px).
//!
//! Member list #1232 `membersWrap_86es0b`: the same `members_86es0b` scroller
//! as a guild, one `h2` "Membres—N" (#1234, padding 24 4 4 20, 14/18 medium)
//! then `member_Iy0c7A` rows (#1235) with the owner crown
//! (`ownerIcon_Iy0c7A`, aria "Propriétaire du groupe"). N counts us.

use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    model::{Id, ImageRef, Member},
    theme::{Palette, c, m, z},
    ui::avatar,
};

/// Recipients (without us) of a group DM, empty for anything else.
pub fn members(app: &PhoenixApp, ch: Id) -> &[Id] {
    app.nav.extras.get(&ch).map(|e| e.members.as_slice()).unwrap_or_default()
}

/// Group icon at `size` (already zoomed px), `ring` = the colour behind it
/// (paints the 3px mask gap of the face pile).
pub fn group_avatar(app: &PhoenixApp, ch: Id, size: f32, ring: Hsla) -> AnyElement {
    let ex = app.nav.extras.get(&ch);
    if let Some(ImageRef::Url(u)) = ex.and_then(|e| e.group_icon.clone()) {
        return gpui::img(u).size(gpui::px(size)).rounded_full().flex_none().into_any_element();
    }
    let ids = ex.map(|e| e.members.as_slice()).unwrap_or_default();
    let users: Vec<_> = ids.iter().filter_map(|u| app.users.get(u)).take(2).collect();
    if users.len() < 2 {
        // One recipient left (or none known): the client shows a single avatar.
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

/// Right panel of a group DM: header + one row per member (us included),
/// sorted by username like the client.
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
            .id("group-members")
            .w(z(m::member_list_w())) // --custom-member-list-width
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .bg(c(Palette::BG_SECONDARY)) // container_86es0b --custom-channel-members-bg
            .border_l_1()
            .border_color(c(Palette::DIVIDER)) // container_86es0b border-inline-start --border-subtle
            .pb(z(10.)) // members_86es0b padding-bottom 10px
            .child(crate::ui::members::header(header))
            .children(rows)
            .into_any_element(),
    )
}

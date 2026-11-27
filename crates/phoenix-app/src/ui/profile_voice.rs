use gpui::{AnyElement, Entity, FontWeight, Hsla, IntoElement, ParentElement, Styled, div, prelude::*, svg};

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
};

pub const THUMB: f32 = 100.;
pub const RING: f32 = 3.;
pub const POPOUT_THUMB: f32 = 60.;
pub const POPOUT_RING: f32 = 2.;
pub const MAX_SHOWN: usize = 4;
const CONNECT: u64 = 1 << 20;
const SET_VOICE_CHANNEL_STATUS: u64 = 1 << 48;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub top: f32,
    pub left: f32,
    pub size: f32,
}

const fn s(top: f32, left: f32, size: f32) -> Slot {
    Slot { top, left, size }
}

pub const SLOTS: [&[Slot]; 4] = [
    &[s(23., 23., 52.)],
    &[s(13., 13., 32.), s(41., 41., 44.)],
    &[s(9., 14., 31.), s(27., 53., 34.), s(52., 11., 36.)],
    &[s(10., 10., 36.), s(10., 57., 31.), s(57., 10., 31.), s(52., 52., 36.)],
];

pub const POPOUT_SLOTS: [&[Slot]; 4] = [
    &[s(15., 15., 28.)],
    &[s(7., 7., 20.), s(25., 25., 26.)],
    &[s(5., 10., 18.), s(15., 31., 22.), s(29., 5., 24.)],
    &[s(5., 5., 22.), s(7., 33., 18.), s(33., 7., 18.), s(31., 31., 22.)],
];

pub fn slots(count: usize) -> &'static [Slot] {
    SLOTS[count.clamp(1, SLOTS.len()) - 1]
}

pub fn popout_slots(count: usize) -> &'static [Slot] {
    POPOUT_SLOTS[count.clamp(1, POPOUT_SLOTS.len()) - 1]
}

pub fn overflow(count: usize) -> Option<(usize, usize)> {
    (count > MAX_SHOWN).then(|| {
        let more = count - MAX_SHOWN + 1;
        (if more >= 10 { 3 } else { 1 }, more)
    })
}

pub fn overflow_label(more: usize) -> String {
    if more > 99 { ">99".into() } else { format!("+{more}") }
}

#[derive(Clone, Copy)]
pub struct Colors {
    pub header: Hsla,
    pub strong: Hsla,
    pub subtle: Hsla,
    pub card: Hsla,
    pub themed: bool,
    pub popout: bool,
}

impl Colors {
    pub fn popout(themed: bool) -> Self {
        Self {
            header: c(tok(idx::TEXT_STRONG)),
            strong: c(tok(idx::TEXT_DEFAULT)),
            subtle: c(tok(idx::TEXT_SUBTLE)),
            card: c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)),
            themed,
            popout: true,
        }
    }

    pub fn modal(themed: bool) -> Self {
        Self {
            header: c(tok(idx::TEXT_DEFAULT)),
            strong: c(tok(idx::TEXT_STRONG)),
            subtle: c(tok(idx::TEXT_SUBTLE)),
            card: c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)),
            themed,
            popout: false,
        }
    }
}

fn thumbnail(app: &PhoenixApp, users: &[Id], themed: bool, popout: bool) -> AnyElement {
    let (thumb, ring_w, set) = if popout { (POPOUT_THUMB, POPOUT_RING, popout_slots(users.len())) } else { (THUMB, RING, slots(users.len())) };
    let light = crate::ui::profile_theme::is_light();
    let (bg, ring, more_bg) = match (themed, light) {
        (true, false) => (tok(idx::BACKGROUND_MOD_STRONG), tok(idx::OPACITY_WHITE_24), tok(idx::OPACITY_WHITE_24)),
        (true, true) => (tok(idx::OPACITY_WHITE_52), tok(idx::OPACITY_WHITE_72), tok(idx::OPACITY_WHITE_72)),
        _ => (tok(idx::CARD_BACKGROUND_DEFAULT), tok(idx::BACKGROUND_SURFACE_HIGHEST), tok(idx::BACKGROUND_SURFACE_HIGHEST)),
    };
    let over = overflow(users.len());
    let circles = set.iter().enumerate().filter_map(|(n, slot)| {
        let u = *users.get(n)?;
        let inner = match over {
            Some((at, more)) if at == n => div()
                .size_full()
                .rounded_full()
                .bg(c(more_bg))
                .flex()
                .items_center()
                .justify_center()
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(overflow_label(more))
                .into_any_element(),
            _ => crate::ui::avatar(app.users.get(&u), slot.size),
        };
        Some(
            div()
                .absolute()
                .top(z(slot.top - ring_w))
                .left(z(slot.left - ring_w))
                .size(z(slot.size + 2. * ring_w))
                .p(z(ring_w))
                .rounded_full()
                .bg(c(ring))
                .shadow(crate::ui::inbox::shadow_high())
                .child(div().size(z(slot.size)).rounded_full().overflow_hidden().child(inner)),
        )
    });
    div()
        .relative()
        .size(z(thumb))
        .flex_none()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(bg))
        .when(!themed, |d| d.child(div().absolute().inset_0().rounded(z(8.)).bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))))
        .children(circles)
        .into_any_element()
}

pub fn card(app: &PhoenixApp, user: Id, col: Colors, entity: Entity<PhoenixApp>) -> Option<AnyElement> {
    let ch = app.channels.values().find(|c| c.voice_users.contains(&user))?;
    let (cid, cname, gid) = (ch.id, ch.name.clone(), ch.guild);
    let users = ch.voice_users.clone();
    let gname = gid.and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| g.name.clone());
    let perms = app.backend.channel_permissions(cid);
    let can_connect = perms.is_none_or(|p| p & CONNECT != 0);
    let joined = app.voice_channel == Some(cid);
    let own = user == app.me.id;
    let status = crate::voice::start_time::status(cid.0).filter(|s| !s.trim().is_empty());
    let can_set_status = joined && perms.is_none_or(|p| p & SET_VOICE_CHANNEL_STATUS != 0);
    let light = crate::ui::profile_theme::is_light();

    let header = div()
        .flex()
        .items_center()
        .gap(z(4.))
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(col.header)
        .child(crate::t!("members.in_voice"))
        .when(own, |d| {
            d.child(
                div()
                    .id(("profile-voice-info", user.0))
                    .tooltip(crate::ui::tooltip::tip(crate::t!("profile.voice_only_visible")))
                    .child(svg().path(crate::icons::path("info")).size(z(12.)).text_color(col.header)),
            )
        });

    let e_join = entity.clone();
    let name = div()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(col.strong)
        .flex()
        .items_center()
        .min_w_0()
        .child(svg().path(crate::icons::path("speaker")).size(z(12.)).mr(z(2.)).flex_none().text_color(col.strong))
        .child(
            div()
                .id(("profile-voice-name", user.0))
                .min_w_0()
                .text_ellipsis()
                .whitespace_nowrap()
                .when(can_connect, |d| {
                    d.cursor_pointer().hover(|s| s.underline()).on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        e_join.update(cx, |a, cx| open_voice(a, cid, cx));
                    })
                })
                .child(cname),
        );
    let e_guild = entity.clone();
    let guild_row = gname.map(|n| {
        div()
            .id(("profile-voice-guild", user.0))
            .text_size(z(12.))
            .line_height(z(16.))
            .text_color(col.strong)
            .cursor_pointer()
            .hover(|s| s.underline())
            .child(crate::t!("profile.voice_in_guild", guild = n))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                e_guild.update(cx, |a, cx| {
                    a.people.close_overlays();
                    a.select_guild(gid, cx);
                })
            })
    });
    let e_status = entity.clone();
    let status_row = if can_set_status {
        Some(
            div()
                .id(("profile-voice-status", user.0))
                .group("profile-voice-status")
                .flex()
                .items_center()
                .gap(z(4.))
                .cursor_pointer()
                .hover(|s| s.underline())
                .text_size(z(12.))
                .line_height(z(16.))
                .text_color(col.subtle)
                .child(div().min_w_0().text_ellipsis().whitespace_nowrap().child(match &status {
                    Some(st) => crate::ui::emoji_text::voice_status(app, &entity, cid, st, 12.),
                    None => crate::t!("voice.set_status").into_any_element(),
                }))
                .child(
                    svg()
                        .path(crate::icons::path("pen"))
                        .size(z(12.))
                        .flex_none()
                        .text_color(col.subtle)
                        .when(status.is_some(), |d| d.opacity(0.).group_hover("profile-voice-status", |s| s.opacity(1.))),
                )
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    e_status.update(cx, |a, cx| a.open_voice_status(cid, cx));
                })
                .into_any_element(),
        )
    } else {
        status.map(|t| {
            div()
                .min_w_0()
                .text_size(z(12.))
                .line_height(z(16.))
                .text_color(col.subtle)
                .text_ellipsis()
                .whitespace_nowrap()
                .child(crate::ui::emoji_text::voice_status(app, &entity, cid, &t, 12.))
                .into_any_element()
        })
    };
    let entity_card = entity.clone();
    let e_open = entity;
    let popout = col.popout;
    let label = match (popout, joined) {
        (false, true) => crate::t!("profile.voice_open"),
        (false, false) => crate::t!("profile.voice_join"),
        (true, true) => crate::t!("profile.open_voice"),
        (true, false) => crate::t!("profile.join_voice"),
    };
    let action = can_connect.then(|| {
        crate::ui::profile::mana_button(("profile-voice-open", user.0), label, false, false)
            .when(popout, |b| b.flex_1())
            .line_height(z(18.))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                e_open.update(cx, |a, cx| open_voice(a, cid, cx));
            })
    });

    let details = div()
        .self_center()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(div().flex().flex_col().child(name).children(guild_row))
        .children(status_row);
    let actions = action.map(|b| div().mt(z(4.)).flex().items_center().gap(z(8.)).child(b));
    let (details, actions) = if popout { (details, actions) } else { (details.children(actions), None) };

    let card = div()
        .id(("profile-voice-card-large", user.0))
        .flex()
        .flex_col()
        .gap(z(8.))
        .p(z(if popout { 8. } else { 12. }))
        .rounded(z(if popout { 8. } else { 12. }))
        .bg(col.card)
        .when(light, |d| d.border_1().border_color(c(tok(idx::BORDER_STRONG))))
        .child(header)
        .child(div().flex().gap(z(if popout { 8. } else { 12. })).child(thumbnail(app, &users, col.themed, popout)).child(details))
        .children(actions);
    if !popout {
        return Some(card.into_any_element());
    }
    Some(
        card.hover(|s| s.bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND_HOVER))))
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                entity_card.update(cx, |a, cx| a.open_profile_modal_at(user, 0, cx));
            })
            .into_any_element(),
    )
}

pub fn open_voice(app: &mut PhoenixApp, channel: Id, cx: &mut gpui::Context<PhoenixApp>) {
    app.people.close_overlays();
    if app.voice_channel == Some(channel) {
        app.open_channel_chat(channel, cx);
        crate::ui::call_view::show(channel);
        cx.notify();
    } else {
        app.select_channel(channel, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_slots_follow_the_modal_v2_layout() {
        assert_eq!(slots(1), &[s(23., 23., 52.)]);
        assert_eq!(slots(2)[1], s(41., 41., 44.));
        assert_eq!(slots(9).len(), 4);
        assert_eq!(slots(0).len(), 1);
        for set in SLOTS {
            for sl in set {
                assert!(sl.top + sl.size <= THUMB - 2. && sl.left + sl.size <= THUMB - 2.);
            }
        }
    }

    #[test]
    fn popout_slots_fit_the_small_thumbnail() {
        assert_eq!(popout_slots(1), &[s(15., 15., 28.)]);
        assert_eq!(popout_slots(4)[3], s(31., 31., 22.));
        for set in POPOUT_SLOTS {
            for sl in set {
                assert!(sl.top + sl.size <= POPOUT_THUMB - 2. && sl.left + sl.size <= POPOUT_THUMB - 2.);
            }
        }
    }

    #[test]
    fn overflow_counts_match_the_client() {
        assert_eq!(overflow(4), None);
        assert_eq!(overflow(5), Some((1, 2)));
        assert_eq!(overflow(13), Some((3, 10)));
        assert_eq!(overflow_label(150), ">99");
        assert_eq!(overflow_label(7), "+7");
    }
}

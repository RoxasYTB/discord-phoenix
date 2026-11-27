use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    model::Id,
    presence::{ProfileRequestState, format_date},
    theme::{Palette, c, generated::idx, tok, z},
    ui::profile::{avatar_block, banner, banner_buttons, ensure_avatar_colour, info, mutuals_row, name_block_clickable, section_title, theme_backdrop},
};

const PROFILE_RETRY_DELAY: Duration = Duration::from_secs(30);

pub fn is_group_dm(app: &PhoenixApp, channel: Id) -> bool {
    app.nav.extras.get(&channel).is_some_and(|e| e.recipients > 1 || e.group_icon.is_some())
}

pub fn friend_nickname(app: &PhoenixApp, ch: &crate::model::Channel) -> Option<gpui::SharedString> {
    if ch.guild.is_some() || is_group_dm(app, ch.id) {
        return None;
    }
    let peer = ch.voice_users.first()?;
    app.nav.relationship(*peer).and_then(|r| r.nickname.clone()).filter(|n| !n.is_empty())
}

pub fn channel_name(app: &PhoenixApp, ch: &crate::model::Channel) -> gpui::SharedString {
    friend_nickname(app, ch).unwrap_or_else(|| ch.name.clone())
}

pub fn peer(app: &PhoenixApp) -> Option<Id> {
    let ch = app.selected_channel.and_then(|c| app.channels.get(&c))?;
    if ch.guild.is_some() || ch.voice_users.len() != 1 || is_group_dm(app, ch.id) {
        return None;
    }
    ch.voice_users.first().copied()
}

pub(crate) fn ensure_profile(app: &PhoenixApp, uid: Id, cx: &mut Context<PhoenixApp>) {
    if app.people.profiles.contains_key(&uid) {
        return;
    }
    {
        let mut requests = app.people.profile_requests.borrow_mut();
        match requests.get(&uid) {
            Some(ProfileRequestState::Loading) => return,
            Some(ProfileRequestState::Failed(at)) if at.elapsed() < PROFILE_RETRY_DELAY => return,
            _ => {}
        }
        requests.insert(uid, ProfileRequestState::Loading);
    }
    let owner = app.me.id;
    let fut = app.backend.user_profile(uid, None);
    cx.spawn(async move |this, cx| {
        let result = fut.await;
        let failed = result.is_err();
        let _ = this.update(cx, |app, cx| {
            if app.me.id != owner || !matches!(app.people.profile_requests.borrow().get(&uid), Some(ProfileRequestState::Loading)) {
                return;
            }
            match result {
                Ok(profile) => {
                    app.people.profile_requests.borrow_mut().remove(&uid);
                    app.people.profiles.entry(uid).or_insert(profile);
                }
                Err(_) => {
                    app.people.profile_requests.borrow_mut().insert(uid, ProfileRequestState::Failed(Instant::now()));
                }
            }
            cx.notify();
        });
        if failed {
            cx.background_executor().timer(PROFILE_RETRY_DELAY).await;
            let _ = this.update(cx, |app, cx| {
                if app.me.id == owner {
                    cx.notify();
                }
            });
        }
    }).detach();
}

pub(crate) fn retry_profile(app: &mut PhoenixApp, uid: Id, cx: &mut Context<PhoenixApp>) {
    if matches!(app.people.profile_requests.borrow().get(&uid), Some(ProfileRequestState::Loading)) {
        return;
    }
    app.people.profile_requests.borrow_mut().remove(&uid);
    ensure_profile(app, uid, cx);
    cx.notify();
}

fn bio_lines(bio: &str) -> gpui::Div {
    crate::ui::profile_bio::bio(bio, true, c(tok(idx::TEXT_STRONG)))
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !app.show_members || !crate::ui::title_bar::dm_profile_fits(crate::ui::title_bar::viewport_w_css()) {
        return None;
    }
    let uid = peer(app)?;
    ensure_profile(app, uid, cx);
    ensure_avatar_colour(app, uid, cx);
    let theme = app.people.profiles.get(&uid).and_then(|p| p.theme);
    Some(crate::ui::profile_theme::scoped(theme, || panel(app, uid, cx)))
}

fn panel(app: &PhoenixApp, uid: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let i = info(app, uid, None);
    let theme = i.prof.and_then(|p| p.theme);
    let card_bg = tok(idx::CHAT_BACKGROUND_DEFAULT);
    let ring_bg = theme.map(|t| crate::ui::profile_theme::composite(t, t.0)).unwrap_or(card_bg);
    let effect = i
        .prof
        .and_then(|p| p.effect_sku)
        .and_then(|s| crate::ui::profile_fx::product(app, s, cx))
        .and_then(|c| crate::ui::profile_fx::effect(&c, 300., cx));
    let e = cx.entity();
    let bio = i.prof.map(|p| p.bio.clone()).filter(|b| !b.is_empty());
    let created = i.prof.map(|p| p.created);
    let header = div()
        .relative()
        .h(z(144.))
        .mb(z(8.))
        .child(banner(&i, 105., 6.))
        .child(banner_buttons(app, uid, None, 8., cx))
        .child(div().absolute().left(z(8.)).top(z(49.)).child(avatar_block(&i, 80., 6., ring_bg)))
        .children(
            crate::ui::profile_toolbar::status(app, uid, None, crate::ui::profile_toolbar::Layout::Sidebar, theme.is_some(), i.custom.clone(), cx)
                .map(|s| div().absolute().left(z(109.)).top(z(98.)).child(s)),
        );
    let hide_notes = app.prefs.local.streamer_hides_info();
    let mut body = div()
        .mx(z(16.))
        .flex()
        .flex_col()
        .gap(z(20.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(name_block_clickable(
                    &i, false,
                    (!hide_notes).then(|| crate::ui::profile_note::note_button(uid, cx)),
                    false, Some(e.clone()),
                ))
                .children(
                    if crate::ui::profile_note::shown(crate::ui::profile_note::Surface::Sidebar, uid == app.me.id, crate::ui::profile_note::editing(uid).is_some(), hide_notes) {
                        crate::ui::profile_note::note_section(i.prof.map(|p| p.note.clone()), uid, cx)
                    } else {
                        None
                    },
                )
                .children(i.prof.and_then(|p| mutuals_row(p, uid, &cx.entity()))),
        );
    if let Some(b) = bio {
        let e2 = e.clone();
        body = body.child(
            div()
                .flex()
                .flex_col()
                .child(bio_lines(&b))
                .child(
                    div()
                        .id("dm-panel-full-bio")
                        .mt(z(8.))
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.open_profile_modal(uid, cx)))
                        .child(crate::t!("profile.view_full_bio")),
                ),
        );
    }
    if let Some(t) = created {
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(section_title(crate::t!("profile.member_since")).mb(z(0.)))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(format_date(t))),
        );
    }
    let e3 = e.clone();
    let footer = div().flex_none().p(z(16.)).child(
        div()
            .id("dm-panel-view-full")
            .w_full()
            .h(z(39.))
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .border_1()
            .border_color(c(Palette::DIVIDER))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(c(Palette::BG_MOD_SELECTED)))
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(Palette::TEXT_HEADER))
            .on_click(move |_, _, cx| e3.update(cx, |a, cx| a.open_profile_modal(uid, cx)))
            .child(crate::t!("profile.view_full")),
    );
    div()
            .w(z(320.))
            .flex_none()
            .h_full()
            .p(z(8.))
            .bg(c(Palette::BG_PRIMARY))
            .child(
                div()
                    .relative()
                    .size_full()
                    .p(z(2.))
                    .rounded(z(8.))
                    .overflow_hidden()
                    .when(theme.is_none(), |d| d.bg(c(card_bg)))
                    .when_some(theme, |d, t| d.children(theme_backdrop(t, 72.)))
                    .child(
                        div()
                            .relative()
                            .size_full()
                            .rounded(z(6.))
                            .overflow_hidden()
                            .when_some(theme, |d, t| d.bg(crate::ui::profile_theme::overlay(t)))
                            .flex()
                            .flex_col()
                            .child(div().id("dm-panel-scroll").flex_1().min_h_0().overflow_y_scroll().child(header).child(body))
                            .child(footer)
                            .children(effect)
                            .children(crate::ui::profile_toolbar::scrim(uid, crate::ui::profile_toolbar::Layout::Sidebar))
                            .children(crate::ui::profile_toolbar::toast(uid, 105.)),
                    ),
            )
            .into_any_element()
}

use gpui::{Context, Div, FontWeight, IntoElement, ParentElement, Pixels, SharedString, Styled, div, prelude::*, px};

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
};

pub const W: f32 = 600.;
pub const BANNER: f32 = 210.;
pub const HEADER_TOP: f32 = 212.;
pub const AVATAR: f32 = 120.;
pub const RING: f32 = 6.;

pub const TAB_BIO: u8 = 0;
pub const TAB_SERVERS: u8 = 1;
pub const TAB_DATA: u8 = 2;

thread_local! {
    static TAB: std::cell::Cell<u8> = const { std::cell::Cell::new(TAB_BIO) };
}

pub fn set_tab(t: u8) {
    TAB.with(|c| c.set(t));
}

pub fn height(vh: f32) -> f32 {
    (vh - 48.).max(220.)
}

pub fn tabs(mutual: usize) -> [(u8, SharedString); 3] {
    let servers = if mutual == 0 { crate::t!("profile.tab_no_mutual_servers") } else { crate::t!("profile.mutual_servers", n = mutual) };
    [(TAB_BIO, crate::t!("profile.bot_bio")), (TAB_SERVERS, servers), (TAB_DATA, crate::t!("profile.bot_data_access"))]
}

fn heading(t: SharedString) -> Div {
    div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(t)
}

fn section(t: SharedString, body: impl IntoElement) -> Div {
    div().flex().flex_col().gap(z(8.)).child(heading(t)).child(body)
}

fn secondary(id: impl Into<gpui::ElementId>) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .h(z(32.))
        .min_w(z(32.))
        .flex()
        .items_center()
        .justify_center()
        .gap(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
        .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
        .active(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE))))
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
}

pub fn card(app: &PhoenixApp, uid: Id, guild: Option<Id>, vp: gpui::Size<Pixels>, cx: &mut Context<PhoenixApp>) -> Div {
    let i = crate::ui::profile::info(app, uid, guild);
    let e = cx.entity();
    let ring = tok(idx::MODAL_BACKGROUND);
    let sel = TAB.with(|c| c.get());
    let mutual = i.prof.map(|p| p.mutual_guilds).unwrap_or(0);

    let more = {
        let e = e.clone();
        div()
            .id(("bot-profile-more", uid.0))
            .absolute()
            .top(z(12.))
            .right(z(16.))
            .size(z(32.))
            .rounded_full()
            .border_1()
            .border_color(gpui::hsla(0., 0., 1., 0.08))
            .bg(gpui::hsla(0., 0., 0., 0.56))
            .hover(|s| s.bg(gpui::hsla(0., 0., 0., 0.72)))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .tooltip(crate::ui::tooltip::tip(crate::t!("profile.more")))
            .child(gpui::svg().path(crate::icons::path("more-horizontal")).size(z(16.)).text_color(gpui::white()))
            .on_click(move |ev, w, cx| {
                crate::ui::extra_menus::open(crate::ui::extra_menus::Target::ProfileOverflow { user: uid, guild }, ev.position(), w);
                e.update(cx, |_, cx| cx.notify());
            })
    };

    let message = {
        let e = e.clone();
        secondary(("bot-profile-message", uid.0))
            .tooltip(crate::ui::tooltip::tip(crate::t!("pe.send_message")))
            .child(gpui::svg().path(crate::icons::path("chat")).size(z(16.)).text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
            .on_click(move |_, _, cx| e.update(cx, |a, cx| a.message_user(uid, cx)))
    };
    let add_app = secondary(("bot-profile-add-app", uid.0))
        .px(z(11.))
        .child(gpui::svg().path(crate::icons::path("plus")).size(z(16.)).text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
        .child(crate::t!("profile.add_app"))
        .on_click(cx.listener(move |a, _, window, cx| crate::ui::modals::app_install::start(a, uid.0, window, cx)));

    let header = div()
        .relative()
        .min_h(z(52.))
        .mr(z(16.))
        .flex()
        .justify_end()
        .child(
            div()
                .id(("bot-profile-avatar", uid.0))
                .absolute()
                .left(z(24. - RING))
                .top(z(-68. - RING))
                .child(crate::ui::profile::avatar_block(&i, AVATAR, RING, ring)),
        )
        .child(div().pt(z(16.)).flex().items_center().justify_end().gap(z(8.)).child(message).child(add_app));

    let username = i.user.as_deref().map(|u| match i.prof.and_then(|p| p.discriminator.clone()) {
        Some(d) => SharedString::from(format!("{}#{d}", u.name)),
        None => u.name.clone(),
    });
    let info = div()
        .mx(z(16.))
        .mt(z(16.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .text_size(z(24.))
                .line_height(z(32.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(crate::ui::emoji_text::label(i.display.clone(), 24.))
                .child(crate::ui::members::bot_tag_px(i.user.as_deref().is_some_and(|u| u.verified_bot))),
        )
        .children(username.map(|n| div().mt(z(8.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_STRONG))).child(n)));

    let tab = |n: u8, t: SharedString| {
        let on = sel == n;
        div()
            .id(("bot-profile-tab", n as u64))
            .group(SharedString::from(format!("bot-tab-{n}")))
            .relative()
            .h(z(25.))
            .pb(z(16.))
            .mb(z(-1.))
            .flex_none()
            .whitespace_nowrap()
            .text_size(z(14.))
            .line_height(z(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(if on { tok(idx::TEXT_STRONG) } else { tok(idx::TEXT_SUBTLE) }))
            .when(!on, |d| d.cursor_pointer().hover(|s| s.text_color(c(tok(idx::TEXT_STRONG)))))
            .on_click(move |_, window, _| {
                set_tab(n);
                window.refresh();
            })
            .child(t)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(z(2.))
                    .rounded_t(z(2.))
                    .when(on, |d| d.bg(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))))
                    .when(!on, |d| d.group_hover(SharedString::from(format!("bot-tab-{n}")), |s| s.bg(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))))),
            )
    };
    let bar = div()
        .mt(z(16.))
        .px(z(16.))
        .flex()
        .gap(z(32.))
        .border_b_1()
        .border_color(c(tok(idx::USER_PROFILE_BORDER)))
        .children(tabs(mutual).into_iter().map(|(n, t)| tab(n, t)));

    let mut panel = div().flex().flex_col().gap(z(20.)).p(z(16.));
    match sel {
        TAB_SERVERS => {
            let ids = i.prof.map(|p| p.mutual_guild_ids.clone()).unwrap_or_default();
            panel = panel.p(z(12.)).gap(z(0.));
            if ids.is_empty() {
                panel = panel.child(div().p(z(4.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("profile.bot_no_mutual_servers")));
            }
            for gid in ids {
                let Some(g) = app.guilds.iter().find(|g| g.id == gid) else { continue };
                let e3 = e.clone();
                panel = panel.child(
                    div()
                        .id(("bot-mutual-guild", gid.0))
                        .h(z(48.))
                        .p(z(4.))
                        .flex()
                        .items_center()
                        .gap(z(10.))
                        .rounded(z(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                        .child(crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 40.))
                        .child(div().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(g.name.clone()))
                        .on_click(move |_, _, cx| {
                            e3.update(cx, |a, cx| {
                                a.people.close_overlays();
                                a.select_guild(Some(gid), cx);
                            })
                        }),
                );
            }
        }
        TAB_DATA => {
            panel = panel.child(div().text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("profile.bot_data_access_text")));
        }
        _ => {
            if let Some(p) = i.prof.filter(|p| !p.bio.is_empty()) {
                panel = panel.child(crate::ui::profile::bio_block(p.bio.clone(), false));
            }
            panel = panel.children(crate::ui::profile::roles_section(app, guild, uid, cx));
            if let Some(p) = i.prof {
                panel = panel.child(section(crate::t!("profile.created_on"), crate::ui::profile::member_since_row(app, p, guild)));
            }
            let note = i.prof.map(|p| p.note.clone()).unwrap_or_default();
            panel = panel.child(section(
                crate::t!("profile.note_title"),
                crate::ui::profile_note::modal_note(note, uid, crate::ui::profile_note::NoteLook::Bot, cx),
            ));
        }
    }

    let h = height(f32::from(vp.height));
    div()
        .relative()
        .w(px(W))
        .h(px(h))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(z(12.))
        .bg(c(tok(idx::MODAL_BACKGROUND)))
        .shadow_lg()
        .child(crate::ui::profile::banner(&i, BANNER, 12.).border_b_0())
        .child(div().h(z(HEADER_TOP - BANNER)).flex_none())
        .child(header)
        .child(more)
        .child(info)
        .child(
            div()
                .mt(z(12.))
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(bar)
                .child(div().id(("bot-profile-panel", uid.0)).flex_1().min_h_0().overflow_y_scroll().child(panel)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bot_modal_uses_the_client_layout() {
        assert_eq!(height(788.), 740.);
        assert_eq!(height(100.), 220.);
        let t = tabs(1);
        assert_eq!(t[0].0, TAB_BIO);
        assert_eq!(t[1].1, crate::t!("profile.mutual_servers", n = 1));
        assert_eq!(tabs(0)[1].1, crate::t!("profile.tab_no_mutual_servers"));
    }
}

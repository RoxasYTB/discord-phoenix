use gpui::{AnyElement, Context, FontWeight, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    model::{Id, Presence},
    theme::{c, generated::idx, tok, z},
    ui::{
        controls::{BtnKind, button},
        home::data::{NavRequest, RelAction, Relationship, RelationshipKind},
        modals::{Modal, card},
    },
};

pub fn other_requests(rels: &[Relationship]) -> Vec<Relationship> {
    let pick = |f: &dyn Fn(&Relationship) -> bool| {
        let mut v: Vec<Relationship> = rels.iter().filter(|r| r.kind == RelationshipKind::Incoming && f(r)).cloned().collect();
        v.sort_by(|a, b| b.since.cmp(&a.since));
        v
    };
    let mut out = pick(&|r| r.ignored);
    out.extend(pick(&|r| r.spam && !r.ignored));
    out
}

pub fn clear_body(n: usize) -> SharedString {
    if n == 1 { crate::t!("friends.clear_body_one", n = n) } else { crate::t!("friends.clear_body_other", n = n) }
}

fn footer(cancel: crate::ui::modal::Action, confirm: gpui::Stateful<gpui::Div>) -> AnyElement {
    div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("fr-modal-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click(move |_, w, cx| cancel(w, cx)))
        .child(confirm.flex_1())
        .into_any_element()
}

pub fn render_clear(count: usize, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let close = crate::ui::modals::close_action(cx);
    let confirm = button("fr-clear-confirm", crate::t!("friends.clear_confirm"), BtnKind::Critical).on_click(cx.listener(|a, _, _, cx| {
        crate::actions::close_modal();
        a.relationship_action(Id(0), RelAction::ClearPending, cx);
    }));
    card(440., crate::t!("friends.clear_title"), Some(clear_body(count)), div().into_any_element(), Some(footer(close, confirm)), cx)
}

pub fn render_stranger(user: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let close = crate::ui::modals::close_action(cx);
    let confirm = button("fr-stranger-accept", crate::t!("friends.accept"), BtnKind::Primary).on_click(cx.listener(move |a, _, _, cx| {
        crate::actions::close_modal();
        a.relationship_action(user, RelAction::AcceptStranger, cx);
    }));
    card(440., crate::t!("friends.stranger_title"), Some(crate::t!("friends.stranger_body")), div().into_any_element(), Some(footer(close, confirm)), cx)
}

pub fn render_spam(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let rows = other_requests(&app.nav.relationships);
    let mut list = div().id("fr-spam-list").max_h(z(420.)).overflow_y_scroll().flex().flex_col();
    for r in &rows {
        list = list.child(spam_row(app, r, cx));
    }
    let footer = (rows.len() > 1).then(|| {
        button("fr-spam-clear", crate::t!("friends.clear_all"), BtnKind::Secondary)
            .w_full()
            .on_click(cx.listener(|a, _, _, cx| {
                crate::actions::close_modal();
                a.relationship_action(Id(0), RelAction::ClearSpamAndIgnored, cx);
            }))
            .into_any_element()
    });
    card(480., crate::t!("friends.other_requests"), Some(crate::t!("friends.spam_subtitle")), list.into_any_element(), footer, cx)
}

fn spam_row(app: &PhoenixApp, r: &Relationship, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let user = r.user;
    let u = app.users.get(&user);
    let p = app.presence_of(user);
    let name = r.nickname.clone().or_else(|| u.map(|u| u.display_name.clone())).unwrap_or_else(|| SharedString::from(user.0.to_string()));
    let sub = if r.ignored { crate::t!("friends.ignored") } else { crate::t!("friends.potential_spam") };
    let g = SharedString::from(format!("fr-spam-{}", user.0));
    let avatar = if p == Presence::Offline {
        div().child(crate::ui::avatar(u, 32.)).into_any_element()
    } else {
        crate::ui::avatar_with_status(u, 32., p, c(tok(idx::BACKGROUND_SURFACE_HIGH))).into_any_element()
    };
    div()
        .id(("fr-spam", user.0))
        .group(g.clone())
        .h(z(62.))
        .px(z(10.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(12.))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .on_click(cx.listener(move |a, _, _, cx| a.open_profile_modal(user, cx)))
        .child(avatar)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .text_ellipsis()
                        .child(name),
                )
                .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_SUBTLE))).child(sub)),
        )
        .child(
            div()
                .flex()
                .gap(z(10.))
                .ml(z(8.))
                .child(crate::ui::home::friends::action_button(("fr-spam-ok", user.0), "check", true, crate::t!("friends.accept")).on_click(
                    cx.listener(move |a, _, _, cx| a.accept_friend_request(user, cx)),
                ))
                .child(crate::ui::home::friends::action_button(("fr-spam-no", user.0), "close", false, crate::t!("friends.ignore")).on_click(
                    cx.listener(move |a, _, _, cx| a.relationship_action(user, RelAction::Remove, cx)),
                )),
        )
        .into_any_element()
}

impl PhoenixApp {
    pub fn open_spam_requests(&mut self, cx: &mut Context<Self>) {
        crate::actions::open_modal(Modal::SpamRequests, None);
        cx.notify();
    }

    pub fn confirm_clear_pending(&mut self, count: usize, cx: &mut Context<Self>) {
        crate::actions::open_modal(Modal::ClearPending(count), None);
        cx.notify();
    }

    pub fn accept_friend_request(&mut self, user: Id, cx: &mut Context<Self>) {
        self.nav_request(NavRequest::Relationship { user, action: RelAction::Accept }, cx, move |_, res, cx| {
            if let Err(e) = res
                && crate::ui::home::add_friend::Failure::decode(&e.0).is_some_and(|f| f.code == Some(crate::ui::home::add_friend::NO_CONFIRMATION))
            {
                crate::actions::open_modal(Modal::AcceptStranger(user), None);
                cx.notify();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(user: u64, spam: bool, ignored: bool, since: i64) -> Relationship {
        Relationship { user: Id(user), kind: RelationshipKind::Incoming, nickname: None, since: Some(since), ignored, spam, note: None }
    }

    #[test]
    fn ignored_first_then_spam_newest_first() {
        let mut out = rel(9, false, false, 1);
        out.kind = RelationshipKind::Outgoing;
        let rels = vec![rel(1, true, false, 10), rel(2, false, true, 5), rel(3, true, false, 20), rel(4, false, false, 30), out];
        let v: Vec<u64> = other_requests(&rels).iter().map(|r| r.user.0).collect();
        assert_eq!(v, vec![2, 3, 1]);
    }
}

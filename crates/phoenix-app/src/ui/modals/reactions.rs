use std::collections::HashMap;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*};

use super::{Modal, card};
use crate::{
    actions::{ActionReply, ActionRequest, MANAGE_MESSAGES, has, with_modal},
    app::PhoenixApp,
    model::{Id, Reaction},
    theme::{c, generated::idx, tok, z},
};

pub struct Reactions {
    pub channel: Id,
    pub msg: Id,
    pub selected: usize,
    pub users: HashMap<usize, Vec<Id>>,
    pub loading: bool,
}

fn reactions(app: &PhoenixApp, ch: Id, msg: Id) -> Vec<Reaction> {
    app.messages.get(&ch).and_then(|x| x.msgs.iter().find(|m| m.id == msg)).map(|m| m.reactions.clone()).unwrap_or_default()
}

impl PhoenixApp {
    pub fn open_reactions(&mut self, channel: Id, msg: Id, selected: usize, cx: &mut Context<Self>) {
        crate::actions::open_modal(Modal::Reactions(Reactions { channel, msg, selected, users: HashMap::new(), loading: false }), None);
        self.load_reactors(cx);
    }

    fn load_reactors(&mut self, cx: &mut Context<Self>) {
        let want = with_modal(|m| match m {
            Some(Modal::Reactions(r)) if !r.users.contains_key(&r.selected) && !r.loading => {
                r.loading = true;
                Some((r.channel, r.msg, r.selected))
            }
            _ => None,
        });
        let Some((ch, msg, i)) = want else { return };
        let Some(emoji) = reactions(self, ch, msg).get(i).cloned() else { return };
        self.action(ActionRequest::Reactors { channel: ch, msg, emoji }, cx, move |a, r, cx| {
            let ids = match r {
                Ok(ActionReply::Users(us)) => {
                    let ids = us.iter().map(|u| u.id).collect();
                    for u in us {
                        a.users.entry(u.id).or_insert(u);
                    }
                    ids
                }
                Err(e) => {
                    a.parity_toast(e.0, cx);
                    Vec::new()
                }
                _ => Vec::new(),
            };
            with_modal(|m| {
                if let Some(Modal::Reactions(r)) = m {
                    r.loading = false;
                    r.users.insert(i, ids);
                }
            });
        });
    }
}

pub fn tab_label(r: &Reaction) -> String {
    if r.emoji_id.is_some() {
        return format!(":{}:", crate::ui::chat::state::reaction_name(&r.emoji));
    }
    match crate::ui::pickers::emoji::index_of(&r.emoji) {
        Some(ix) => format!(":{}:", crate::ui::pickers::emoji::shortcode(ix).trim_matches(':')),
        None => r.emoji.to_string(),
    }
}

pub fn body_height(viewport_h: f32) -> f32 {
    let modal = if viewport_h > 0. { (viewport_h * 0.9).min(720.) } else { 720. };
    (modal - 8. - 48. - 8. - 9. - 16.).max(120.)
}

pub fn render(app: &PhoenixApp, r: &Reactions, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::ui::chat::scrollbar::{self, Bar};
    let list = reactions(app, r.channel, r.msg);
    let guild = app.channels.get(&r.channel).and_then(|c| c.guild);
    let manage = has(app, guild, crate::actions::ADMINISTRATOR) || has(app, guild, MANAGE_MESSAGES);
    let me = Some(app.me.id);
    let mut tabs = div().flex().flex_col();
    for (i, x) in list.iter().enumerate() {
        let on = i == r.selected;
        tabs = tabs.child(
            div()
                .id(("rx-tab", i))
                .mb(z(8.))
                .px(z(4.))
                .rounded(z(8.))
                .overflow_hidden()
                .flex()
                .items_center()
                .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))).hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG)))))
                .when(!on, |d| d.cursor_pointer().hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER)))))
                .tooltip(crate::ui::tooltip::tip(tab_label(x)))
                .on_click(cx.listener(move |a, _, _, cx| {
                    with_modal(|m| {
                        if let Some(Modal::Reactions(r)) = m {
                            r.selected = i;
                        }
                    });
                    a.load_reactors(cx);
                    cx.notify();
                }))
                .child(div().flex_none().pt(z(4.)).pb(z(4.)).pl(z(4.)).pr(z(8.)).child(crate::media::emoji::emoji_element(&x.emoji, 24.)))
                .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(x.count.to_string())),
        );
    }
    let mut users = div().flex().flex_col();
    match r.users.get(&r.selected) {
        None => users = users.child(div().h(z(52.)).w_full().flex().items_center().justify_center()),
        Some(ids) => {
            for u in ids {
                let user = app.users.get(u);
                let uid = *u;
                let nick = app.member_nick_in(guild, uid).map(|n| n.to_string()).or_else(|| user.map(|u| u.display_name.to_string())).unwrap_or_default();
                let removable = manage || me == Some(uid);
                let emoji = list.get(r.selected).cloned();
                let (ch, msg) = (r.channel, r.msg);
                let grp = gpui::SharedString::from(format!("rx-user-{}", uid.0));
                users = users.child(
                    div()
                        .id(("rx-user", uid.0))
                        .group(grp.clone())
                        .h(z(52.))
                        .ml(z(16.))
                        .mr(z(2.))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .id(("rx-user-open", uid.0))
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .on_click(cx.listener(move |a, _, _, cx| {
                                    crate::actions::close_modal();
                                    a.open_profile_modal(uid, cx);
                                }))
                                .child(div().flex_none().child(crate::ui::avatar(user, 32.)))
                                .child(
                                    div()
                                        .mx(z(10.))
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).truncate().child(nick))
                                        .child(div().text_size(z(12.)).line_height(z(14.)).text_color(c(tok(idx::TEXT_MUTED))).truncate().child(user.map(|u| u.name.clone()).unwrap_or_default())),
                                ),
                        )
                        .when(removable, |d| {
                            d.child(
                                div()
                                    .id(("rx-user-remove", uid.0))
                                    .flex_none()
                                    .size(z(32.))
                                    .rounded(z(8.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .opacity(0.)
                                    .group_hover(grp, |s| s.opacity(1.))
                                    .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                                    .child(gpui::svg().path(crate::icons::path("close-small")).size(z(20.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                                    .on_click(cx.listener(move |a, _, _, cx| {
                                        if let Some(e) = emoji.as_ref() {
                                            let w = crate::actions::guard::GuardedWrite::remove_user_reaction(ch.0, msg.0, &crate::actions::exec::emoji(e), uid.0);
                                            a.blocked_write(w, cx);
                                        }
                                    })),
                            )
                        }),
                );
            }
        }
    }
    let h = body_height(crate::ui::title_bar::viewport_h_css());
    let tabs_el = scrollbar::host(Bar::ThinFade)
        .flex_none()
        .w(z(98.))
        .mt(z(16.))
        .border_r_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .child(div().id("rx-tabs").size_full().overflow_y_scroll().track_scroll(&scrollbar::handle("rx-tabs")).pr(z(24.)).child(tabs))
        .children(scrollbar::bar_for("rx-tabs", Bar::ThinFade));
    let users_el = scrollbar::host(Bar::AutoFade)
        .flex_1()
        .min_w_0()
        .child(div().id("rx-users").size_full().overflow_y_scroll().track_scroll(&scrollbar::handle("rx-users")).pt(z(12.)).pr(z(40.)).child(users))
        .children(scrollbar::bar_for("rx-users", Bar::AutoFade));
    let body = div()
        .mx(z(-24.))
        .mt(z(-8.))
        .mb(z(-16.))
        .flex()
        .flex_col()
        .child(div().pt(z(8.)).border_b_1().border_color(c(tok(idx::BORDER_SUBTLE))))
        .child(div().h(z(h)).pl(z(24.)).flex().child(tabs_el).child(users_el));
    card(600., crate::t!("msg.reactions_title"), None, body.into_any_element(), None, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_tooltips_use_the_emoji_name() {
        let custom = Reaction { emoji: "<:blob:42>".into(), count: 1, me: false, emoji_id: Some(Id(42)) };
        assert_eq!(tab_label(&custom), ":blob:");
        let unicode = Reaction { emoji: "👍".into(), count: 2, me: false, emoji_id: None };
        assert!(tab_label(&unicode).starts_with(':'));
    }

    #[test]
    fn body_fills_the_modal_height() {
        assert_eq!(body_height(1000.), 720. - 89.);
        assert_eq!(body_height(600.), 540. - 89.);
    }
}

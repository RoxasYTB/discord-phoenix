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

pub fn render(app: &PhoenixApp, r: &Reactions, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let list = reactions(app, r.channel, r.msg);
    let guild = app.channels.get(&r.channel).and_then(|c| c.guild);
    let manage = has(app, guild, crate::actions::ADMINISTRATOR) || has(app, guild, MANAGE_MESSAGES);
    let mut tabs = div().id("rx-tabs").w(z(216.)).flex_none().overflow_y_scroll().py(z(8.)).px(z(8.)).bg(c(tok(idx::BACKGROUND_BASE_LOWER))).flex().flex_col().gap(z(2.));
    for (i, x) in list.iter().enumerate() {
        let on = i == r.selected;
        tabs = tabs.child(
            div()
                .id(("rx-tab", i))
                .h(z(32.))
                .px(z(8.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .when(on, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .on_click(cx.listener(move |a, _, _, cx| {
                    with_modal(|m| {
                        if let Some(Modal::Reactions(r)) = m {
                            r.selected = i;
                        }
                    });
                    a.load_reactors(cx);
                    cx.notify();
                }))
                .child(crate::media::emoji::emoji_element(&x.emoji, 20.))
                .child(div().text_size(z(14.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(x.count.to_string())),
        );
    }
    let mut users = div().id("rx-users").flex_1().min_w_0().overflow_y_scroll().p(z(8.)).flex().flex_col();
    match r.users.get(&r.selected) {
        None => users = users.child(div().p(z(8.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("common.loading"))),
        Some(ids) => {
            for u in ids {
                let user = app.users.get(u);
                users = users.child(
                    div()
                        .h(z(40.))
                        .px(z(8.))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .child(crate::ui::avatar(user, 24.))
                        .child(div().text_size(z(16.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(user.map(|u| u.display_name.clone()).unwrap_or_default()))
                        .child(div().text_size(z(14.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(user.map(|u| u.name.clone()).unwrap_or_default())),
                );
            }
        }
    }
    let body = div().h(z(360.)).mx(z(-24.)).flex().child(tabs).child(users);
    let footer = manage.then(|| {
        let (ch, msg) = (r.channel, r.msg);
        let emoji = list.get(r.selected).cloned();
        crate::ui::controls::button("rx-remove", crate::t!("msg.remove_emoji_reactions"), crate::ui::controls::BtnKind::Critical)
            .on_click(cx.listener(move |a, _, _, cx| {
                if let Some(e) = emoji.clone() {
                    a.remove_reactions(ch, msg, Some(e), cx);
                    crate::actions::close_modal();
                }
            }))
            .into_any_element()
    });
    card(600., crate::t!("msg.reactions_title"), None, body.into_any_element(), footer, cx)
}

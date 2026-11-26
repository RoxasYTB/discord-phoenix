use std::collections::HashMap;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*};

use super::Modal;
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
    pub users: HashMap<Entry, Vec<Id>>,
    pub loading: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entry {
    pub index: usize,
    pub burst: bool,
}

fn reactions(app: &PhoenixApp, ch: Id, msg: Id) -> Vec<Reaction> {
    app.messages.get(&ch).and_then(|x| x.msgs.iter().find(|m| m.id == msg)).map(|m| m.reactions.clone()).unwrap_or_default()
}

pub fn entries(list: &[Reaction]) -> Vec<Entry> {
    let mut out = Vec::new();
    for (index, r) in list.iter().enumerate() {
        if r.burst_count > 0 {
            out.push(Entry { index, burst: true });
        }
        if r.count > 0 {
            out.push(Entry { index, burst: false });
        }
    }
    out.sort_by_key(|e| std::cmp::Reverse(entry_count(list, *e)));
    out
}

fn entry_count(list: &[Reaction], e: Entry) -> u32 {
    list.get(e.index).map_or(0, |r| if e.burst { r.burst_count } else { r.count })
}

impl PhoenixApp {
    pub fn open_reactions(&mut self, channel: Id, msg: Id, selected: usize, cx: &mut Context<Self>) {
        crate::actions::open_modal(Modal::Reactions(Reactions { channel, msg, selected, users: HashMap::new(), loading: false }), None);
        self.load_reactors(cx);
        play_header(self, cx);
    }

    fn load_reactors(&mut self, cx: &mut Context<Self>) {
        let want = with_modal(|m| match m {
            Some(Modal::Reactions(r)) => Some((r.channel, r.msg, r.selected, r.users.keys().copied().collect::<Vec<_>>())),
            _ => None,
        });
        let Some((ch, msg, sel, have)) = want else { return };
        let list = reactions(self, ch, msg);
        let Some(i) = entries(&list).get(sel).copied() else { return };
        if have.contains(&i) {
            return;
        }
        with_modal(|m| {
            if let Some(Modal::Reactions(r)) = m {
                r.loading = true;
            }
        });
        let Some(emoji) = list.get(i.index).cloned() else { return };
        self.action(ActionRequest::Reactors { channel: ch, msg, emoji, burst: i.burst }, cx, move |a, r, cx| {
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

pub const MAX_H: f32 = 800.;

pub fn modal_height(viewport_h: f32) -> f32 {
    let room = viewport_h - 2. * (crate::theme::m::TITLE_BAR_H + 24.);
    if viewport_h > 0. { room.min(MAX_H) } else { MAX_H }
}

pub fn body_height(viewport_h: f32) -> f32 {
    (modal_height(viewport_h) - 8. - 48. - 8. - 9. - 16.).max(120.)
}

fn burst_header(r: &Reactions, x: &Reaction) -> AnyElement {
    let key = crate::ui::burst::Key::new(r.channel, r.msg, &x.emoji, x.emoji_id);
    let playing = crate::ui::burst::playing(&key);
    let hide = playing.is_some() || crate::ui::burst::pending(&key) || HEADER_WAIT.with(|w| w.borrow().contains(&key));
    div()
        .relative()
        .py(z(32.))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(z(32.))
                .mb(z(8.))
                .children(playing.map(|(img, _)| crate::ui::burst::overlay(img, 32.)))
                .child(div().when(hide, |d| d.opacity(0.)).child(crate::media::emoji::emoji_element(&x.emoji, 32.))),
        )
        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(tab_label(x)))
        .into_any_element()
}

thread_local! {
    static HEADER_WAIT: std::cell::RefCell<std::collections::HashSet<crate::ui::burst::Key>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

pub fn play_header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let want = with_modal(|m| match m {
        Some(Modal::Reactions(r)) => Some((r.channel, r.msg, r.selected)),
        _ => None,
    });
    let Some((ch, msg, sel)) = want else { return };
    let list = reactions(app, ch, msg);
    let Some(e) = entries(&list).get(sel).copied().filter(|e| e.burst) else { return };
    let Some(x) = list.get(e.index).cloned() else { return };
    let key = crate::ui::burst::Key::new(ch, msg, &x.emoji, x.emoji_id);
    HEADER_WAIT.with(|w| w.borrow_mut().insert(key.clone()));
    cx.spawn(async move |this, cx| {
        let (token, id) = (x.emoji.to_string(), x.emoji_id);
        let color = cx.background_executor().spawn(async move { crate::ui::burst::emoji_color(&token, id) }).await;
        HEADER_WAIT.with(|w| w.borrow_mut().remove(&key));
        let _ = this.update(cx, |_, cx| {
            if let Some(accent) = color {
                crate::ui::burst::play(
                    crate::ui::burst::Play { channel: ch, msg, token: &x.emoji, emoji_id: x.emoji_id, accent, kind: crate::ui::burst::Kind::External },
                    cx,
                );
            }
            cx.notify();
        });
    })
    .detach();
}

pub fn render(app: &PhoenixApp, r: &Reactions, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::ui::chat::scrollbar::{self, Bar};
    let list = reactions(app, r.channel, r.msg);
    let guild = app.channels.get(&r.channel).and_then(|c| c.guild);
    let manage = has(app, guild, crate::actions::ADMINISTRATOR) || has(app, guild, MANAGE_MESSAGES);
    let me = Some(app.me.id);
    let all = entries(&list);
    let current = all.get(r.selected).copied();
    let dark = !crate::theme::is_light();
    let mut tabs = div().flex().flex_col();
    for (i, en) in all.iter().copied().enumerate() {
        let Some(x) = list.get(en.index) else { continue };
        let on = i == r.selected;
        let pal = if en.burst { crate::ui::burst::palette(&x.burst_colors, dark) } else { None };
        let n = entry_count(&list, en);
        tabs = tabs.child(
            div()
                .id(("rx-tab", i))
                .mb(z(8.))
                .px(z(4.))
                .rounded(z(8.))
                .overflow_hidden()
                .flex()
                .items_center()
                .map(|d| match pal {
                    Some(p) if on => d.bg(p.background_rgba(p.opacity)).border_1().border_color(p.background_rgba(1.1 * p.opacity)),
                    Some(p) => d.cursor_pointer().bg(p.background_rgba(0.025)).border_1().border_color(p.background_rgba(0.05)),
                    None if on => d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))).hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG)))),
                    None => d.cursor_pointer().hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER)))),
                })
                .tooltip(crate::ui::tooltip::tip(tab_label(x)))
                .on_click(cx.listener(move |a, _, _, cx| {
                    with_modal(|m| {
                        if let Some(Modal::Reactions(r)) = m {
                            r.selected = i;
                        }
                    });
                    a.load_reactors(cx);
                    play_header(a, cx);
                    cx.notify();
                }))
                .child(div().flex_none().pt(z(4.)).pb(z(4.)).pl(z(4.)).pr(z(8.)).child(crate::media::emoji::emoji_element(&x.emoji, 24.)))
                .child(
                    div()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .map(|d| match pal {
                            Some(p) => d.text_color(p.accent_rgba(1.)),
                            None => d.text_color(c(tok(idx::TEXT_DEFAULT))),
                        })
                        .child(n.to_string()),
                ),
        );
    }
    let mut users = div().flex().flex_col();
    match current.and_then(|e| r.users.get(&e)) {
        None => users = users.child(div().h(z(52.)).w_full().flex().items_center().justify_center()),
        Some(ids) => {
            for u in ids {
                let user = app.users.get(u);
                let uid = *u;
                let nick = app.member_nick_in(guild, uid).map(|n| n.to_string()).or_else(|| user.map(|u| u.display_name.to_string())).unwrap_or_default();
                let removable = manage || me == Some(uid);
                let emoji = current.and_then(|e| list.get(e.index)).cloned();
                let burst = current.is_some_and(|e| e.burst);
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
                                            let em = crate::actions::exec::emoji(e);
                                            let w = if burst {
                                                crate::actions::guard::GuardedWrite::remove_user_burst_reaction(ch.0, msg.0, &em, uid.0)
                                            } else {
                                                crate::actions::guard::GuardedWrite::remove_user_reaction(ch.0, msg.0, &em, uid.0)
                                            };
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
    let header = current.filter(|e| e.burst).and_then(|e| list.get(e.index)).map(|x| burst_header(r, x));
    let users_el = scrollbar::host(Bar::AutoFade)
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .children(header)
        .child(div().id("rx-users").flex_1().min_h_0().w_full().overflow_y_scroll().track_scroll(&scrollbar::handle("rx-users")).pt(z(12.)).pr(z(40.)).child(users))
        .children(scrollbar::bar_for("rx-users", Bar::AutoFade));
    let body = div()
        .mx(z(-24.))
        .mt(z(-8.))
        .mb(z(-16.))
        .flex()
        .flex_col()
        .child(div().pt(z(8.)).border_b_1().border_color(c(tok(idx::BORDER_SUBTLE))))
        .child(div().h(z(h)).pl(z(24.)).flex().child(tabs_el).child(users_el));
    super::card_max(480., MAX_H, crate::t!("msg.reactions_title"), body.into_any_element(), cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_tooltips_use_the_emoji_name() {
        let custom = Reaction { emoji: "<:blob:42>".into(), count: 1, me: false, emoji_id: Some(Id(42)), ..Default::default() };
        assert_eq!(tab_label(&custom), ":blob:");
        let unicode = Reaction { emoji: "👍".into(), count: 2, me: false, emoji_id: None, ..Default::default() };
        assert!(tab_label(&unicode).starts_with(':'));
    }

    #[test]
    fn body_fills_the_modal_height() {
        assert_eq!(body_height(1200.), 800. - 89.);
        assert_eq!(body_height(800.), 800. - 112. - 89.);
        assert_eq!(body_height(0.), 800. - 89.);
    }

    #[test]
    fn super_reactions_get_their_own_tab() {
        let r = |e: &str, count, burst_count| Reaction { emoji: e.into(), count, burst_count, ..Default::default() };
        let list = vec![r("👍", 3, 0), r("🔥", 1, 5), r("😀", 0, 2)];
        let e = entries(&list);
        assert_eq!(
            e,
            vec![
                Entry { index: 1, burst: true },
                Entry { index: 0, burst: false },
                Entry { index: 2, burst: true },
                Entry { index: 1, burst: false },
            ]
        );
        assert_eq!(entry_count(&list, e[0]), 5);
        assert_eq!(entry_count(&list, e[3]), 1);
    }
}

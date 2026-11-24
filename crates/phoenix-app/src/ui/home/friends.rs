use crate::ui::tooltip::Tipped as _;
use crate::theme::z;
use gpui::{
    AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px,
};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    model::{Id, Presence},
    theme::{Palette, c, generated::idx, m, tok},
    ui::{
        avatar_with_status, badge, icon,
        home::data::{FriendsTab, HomeView, NavRequest, RelAction, Relationship, RelationshipKind},
    },
};

pub fn render(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    if app.nav.home == HomeView::MessageRequests {
        return message_requests(app, cx).into_any_element();
    }
    if app.nav.friends_tab == FriendsTab::AddFriend && app.nav.add_friend_input.is_none() {
        ensure_add_input(app, window, cx);
    }
    if app.nav.friends_search.is_none() {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("search.placeholder")));
        let sub = cx.subscribe_in(&input, window, |_: &mut PhoenixApp, _, ev: &InputEvent, _w, cx| {
            if let InputEvent::Change = ev {
                cx.notify();
            }
        });
        app.nav.subs.push(sub);
        app.nav.friends_search = Some(input);
    }
    if let Some(i) = app.nav.friends_search.clone() {
        crate::ui::locale_inputs::keep_placeholder(&i, "search.placeholder", window, cx);
    }
    if let Some(i) = app.nav.add_friend_input.clone() {
        crate::ui::locale_inputs::keep_placeholder(&i, "friends.add_hint", window, cx);
    }
    let body: AnyElement = match app.nav.friends_tab {
        FriendsTab::AddFriend => add_friend(app, cx).into_any_element(),
        tab => list(app, tab, cx).into_any_element(),
    };
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(c(Palette::BG_PRIMARY))
        .child(header(app, cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_row()
                .child(div().flex_1().min_w_0().h_full().child(body))
                .child(active_now(app, cx)),
        )
        .into_any_element()
}

fn ensure_add_input(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("friends.add_hint")));
    let sub = cx.subscribe_in(&input, window, |this: &mut PhoenixApp, _, ev: &InputEvent, _w, cx| {
        if let InputEvent::PressEnter { .. } = ev {
            send_request(this, cx);
        }
    });
    app.nav.subs.push(sub);
    app.nav.add_friend_input = Some(input);
}

fn send_request(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(input) = app.nav.add_friend_input.clone() else { return };
    let name = input.read(cx).value().trim().to_string();
    if name.is_empty() {
        return;
    }
    app.nav_request(
        NavRequest::Relationship { user: Id(0), action: RelAction::AddByName(name.clone()) },
        cx,
        move |this, res, _| {
            this.nav.add_friend_result = Some(match res {
                Ok(_) => Ok(crate::t!("friends.request_sent", name = name)),
                Err(e) => Err(if e.0.is_empty() {
                    crate::t!("friends.request_failed")
                } else {
                    e.0.into()
                }),
            });
        },
    );
}

fn header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let pending = app.nav.count(RelationshipKind::Incoming) as u32;
    let pill = || {
        div()
            .mx(z(8.))
            .px(z(12.))
            .py(z(4.))
            .h(z(32.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .gap(z(6.))
            .cursor_pointer()
            .text_size(z(16.))
            .line_height(z(20.))
    };
    let tab = |id: &'static str, label: SharedString, t: FriendsTab, cx: &mut Context<PhoenixApp>| {
        let sel = app.nav.friends_tab == t;
        pill()
            .id(id)
            .text_color(c(tok(if sel { idx::TEXT_STRONG } else { idx::TEXT_SUBTLE })))
            .when(sel, |d| d.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
            .when(!sel, |d| {
                d.hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).text_color(c(tok(idx::TEXT_STRONG))))
                    .active(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG))).text_color(c(tok(idx::TEXT_STRONG))))
            })
            .on_click(cx.listener(move |a, _, _, cx| {
                a.nav.friends_tab = t;
                cx.notify();
            }))
            .child(label)
            .when(t == FriendsTab::Pending && pending > 0, |d| d.child(badge(pending)))
    };
    let add_sel = app.nav.friends_tab == FriendsTab::AddFriend;
    div()
        .h(px(m::HEADER_H))
        .flex_none()
        .pl(z(20.))
        .pr(z(8.))
        .py(z(8.))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(c(Palette::HEADER_SHADOW))
        .child(div().size(z(32.)).flex().items_center().justify_center().child(icon(Icon::Friends, 20., c(Palette::CHANNEL_ICON))))
        .child(
            div()
                .mr(z(8.))
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(Palette::TEXT_HEADER))
                .child(crate::t!("friends.title")),
        )
        .child(div().mx(z(4.)).size(z(4.)).rounded_full().bg(c(Palette::BG_MOD_SELECTED)))
        .child(tab("tab-online", crate::t!("status.online"), FriendsTab::Online, cx))
        .child(tab("tab-all", crate::t!("friends.all"), FriendsTab::All, cx))
        .child(tab("tab-pending", crate::t!("friends.pending"), FriendsTab::Pending, cx))
        .child(
            pill()
                .id("tab-add")
                .when(add_sel, |d| d.bg(c(tok(idx::OPACITY_BLURPLE_16))).text_color(c(tok(idx::TEXT_BRAND))))
                .when(!add_sel, |d| {
                    d.bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT)))
                        .text_color(gpui::white())
                        .hover(|s| s.bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_HOVER))))
                        .active(|s| s.bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_ACTIVE))))
                })
                .on_click(cx.listener(|a, _, _, cx| {
                    a.nav.friends_tab = FriendsTab::AddFriend;
                    a.nav.add_friend_result = None;
                    cx.notify();
                }))
                .child(crate::t!("friends.add_tab")),
        )
        .child(div().flex_1())
        .child(
            div()
                .pl(z(10.))
                .pr(z(8.))
                .child(
                    div()
                        .id("fr-new-group-dm")
                        .group("fr-new-group-dm")
                        .size(z(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .tip(crate::t!("friends.new_group_dm"))
                        .child(
                            gpui::svg()
                                .path(crate::icons::path("new-group-dm"))
                                .size(z(20.))
                                .text_color(c(tok(idx::ICON_MUTED)))
                                .group_hover("fr-new-group-dm", |s| s.text_color(c(tok(idx::ICON_SUBTLE)))),
                        ),
                ),
        )
}

fn visible(app: &PhoenixApp, tab: FriendsTab) -> Vec<Relationship> {
    let mut v: Vec<Relationship> = app
        .nav
        .relationships
        .iter()
        .filter(|r| match tab {
            FriendsTab::Online => r.kind == RelationshipKind::Friend && app.presence_of(r.user) != Presence::Offline,
            FriendsTab::All => r.kind == RelationshipKind::Friend,
            FriendsTab::Pending => matches!(r.kind, RelationshipKind::Incoming | RelationshipKind::Outgoing),
            FriendsTab::Blocked => r.kind == RelationshipKind::Blocked,
            FriendsTab::AddFriend => false,
        })
        .cloned()
        .collect();
    let name = |r: &Relationship| {
        app.users.get(&r.user).map(|u| u.display_name.to_lowercase()).unwrap_or_default()
    };
    v.sort_by_key(|r| (r.kind == RelationshipKind::Outgoing, name(r)));
    v
}

fn list(app: &PhoenixApp, tab: FriendsTab, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let q = app.nav.friends_search.as_ref().map(|i| i.read(cx).value().to_lowercase()).unwrap_or_default();
    let rows: Vec<Relationship> = visible(app, tab)
        .into_iter()
        .filter(|r| {
            q.is_empty()
                || app.users.get(&r.user).is_some_and(|u| u.display_name.to_lowercase().contains(&q) || u.name.to_lowercase().contains(&q))
        })
        .collect();
    let search = div()
        .flex_none()
        .mt(z(12.))
        .mr(z(16.))
        .mb(z(8.))
        .ml(z(24.))
        .h(z(40.))
        .pl(z(4.))
        .gap(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(Palette::BG_MOD_SELECTED))
        .bg(c(gpui_input_bg()))
        .flex()
        .items_center()
        .child(div().size(z(30.)).flex().items_center().justify_center().child(icon(Icon::Search, 16., c(Palette::TEXT_MUTED))))
        .when_some(app.nav.friends_search.clone(), |d, i| {
            d.child(div().flex_1().child(Input::new(&i).appearance(false).bordered(false).pl(z(0.)).text_size(z(16.)).line_height(z(20.))))
        });
    let mut col = div().id("friends-list").flex_1().min_h_0().overflow_y_scroll().pb(z(8.)).flex().flex_col();
    if rows.is_empty() {
        let msg = if !q.is_empty() {
            crate::t!("friends.empty_search")
        } else {
            match tab {
                FriendsTab::Online => crate::t!("friends.empty_online"),
                FriendsTab::Pending => crate::t!("friends.empty_pending"),
                FriendsTab::Blocked => crate::t!("friends.empty_blocked"),
                _ => crate::t!("friends.empty_all"),
            }
        };
        let add = (q.is_empty() && tab == FriendsTab::All).then(|| {
            crate::ui::controls::button("friends-empty-add", crate::t!("friends.add_tab"), crate::ui::controls::BtnKind::Primary)
                .mt(z(20.))
                .on_click(cx.listener(|a, _, _, cx| {
                    a.nav.friends_tab = FriendsTab::AddFriend;
                    a.nav.add_friend_result = None;
                    cx.notify();
                }))
        });
        col = col.child(
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .w_full()
                    .max_w(z(440.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .mt(z(8.))
                            .text_center()
                            .text_size(z(16.))
                            .line_height(z(20.))
                            .text_color(c(tok(idx::TEXT_MUTED)))
                            .child(msg),
                    )
                    .children(add),
            ),
        );
    } else {
        let mut sections: Vec<(SharedString, Vec<Relationship>)> = Vec::new();
        match tab {
            FriendsTab::Pending => {
                let (inc, out): (Vec<_>, Vec<_>) = rows.into_iter().partition(|r| r.kind == RelationshipKind::Incoming);
                if !inc.is_empty() {
                    sections.push((crate::t!("friends.sec_received", n = inc.len()), inc));
                }
                if !out.is_empty() {
                    sections.push((crate::t!("friends.sec_sent", n = out.len()), out));
                }
            }
            FriendsTab::Online => sections.push((crate::t!("friends.sec_online", n = rows.len()), rows)),
            FriendsTab::All => sections.push((crate::t!("friends.sec_all", n = rows.len()), rows)),
            _ => sections.push((crate::t!("friends.sec_blocked", n = rows.len()), rows)),
        }
        for (title, rows) in sections {
            col = col.child(
                div()
                    .h(z(50.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .ml(z(24.))
                    .mr(z(20.))
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(Palette::TEXT_NORMAL))
                    .child(title),
            );
            for r in rows {
                col = col.child(div().flex_none().h(z(1.)).ml(z(30.)).mr(z(20.)).bg(c(Palette::DIVIDER)));
                col = col.child(friend_row(app, &r, cx));
            }
        }
    }
    div().size_full().flex().flex_col().child(search).child(col).into_any_element()
}

fn gpui_input_bg() -> u32 {
    crate::theme::tok(crate::theme::generated::idx::INPUT_BACKGROUND_DEFAULT)
}

fn round_button(id: (&'static str, u64), name: &str, hover: Hsla, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    let g = SharedString::from(format!("{}-{}", id.0, id.1));
    div()
        .id(id)
        .group(g.clone())
        .size(z(36.))
        .flex_none()
        .rounded_full()
        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .tip(tip)
        .child(
            gpui::svg()
                .path(crate::icons::path(name))
                .size(z(20.))
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover(g, move |s| s.text_color(hover)),
        )
}

fn friend_row(app: &PhoenixApp, r: &Relationship, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let user = r.user;
    let u = app.users.get(&user);
    let p = app.presence_of(user);
    let name = r
        .nickname
        .clone()
        .or_else(|| u.map(|u| u.display_name.clone()))
        .unwrap_or_else(|| SharedString::from(user.0.to_string()));
    let username = u.map(|u| u.name.clone()).unwrap_or_default();
    let sub: SharedString = match r.kind {
        RelationshipKind::None => SharedString::default(),
        RelationshipKind::Incoming | RelationshipKind::Outgoing => username.clone(),
        RelationshipKind::Blocked => crate::t!("friends.blocked"),
        RelationshipKind::Friend => app.nav.activities.get(&user).cloned().unwrap_or_else(|| {
            match p {
                Presence::Online => crate::t!("status.online"),
                Presence::Idle => crate::t!("status.idle"),
                Presence::Dnd => crate::t!("status.dnd"),
                Presence::Offline => crate::t!("status.offline"),
            }
        }),
    };
    let pending = matches!(r.kind, RelationshipKind::Incoming | RelationshipKind::Outgoing);
    let mut actions = div().flex().gap(z(10.)).when(pending, |d| d.ml(z(8.)));
    let act = |action: RelAction| {
        cx.listener(move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _, cx| {
            match &action {
                RelAction::Remove => a.nav.relationships.retain(|x| x.user != user),
                RelAction::Accept => {
                    if let Some(x) = a.nav.relationships.iter_mut().find(|x| x.user == user) {
                        x.kind = RelationshipKind::Friend;
                    }
                }
                _ => {}
            }
            a.nav_request(NavRequest::Relationship { user, action: action.clone() }, cx, |_, _, _| {});
            cx.notify();
        })
    };
    let (hover, positive, critical) =
        (c(tok(idx::INTERACTIVE_TEXT_HOVER)), c(tok(idx::ICON_FEEDBACK_POSITIVE)), c(tok(idx::ICON_FEEDBACK_CRITICAL)));
    match r.kind {
        RelationshipKind::None => {}
        RelationshipKind::Friend => {
            actions = actions
                .child(
                    round_button(("fr-msg", user.0), "chat", hover, crate::t!("profile.message"))
                        .on_click(cx.listener(move |a, _, _, cx| a.open_dm_with(user, cx))),
                )
                .child({
                    round_button(("fr-more", user.0), "more-vertical", hover, crate::t!("header.more"))
                        .on_click(move |ev, window, cx| {
                            cx.stop_propagation();
                            crate::ui::user_menu::open(user, None, ev.position(), window);
                        })
                });
        }
        RelationshipKind::Incoming => {
            actions = actions
                .child(round_button(("fr-ok", user.0), "check", positive, crate::t!("friends.accept")).on_click(act(RelAction::Accept)))
                .child(round_button(("fr-no", user.0), "close", critical, crate::t!("menu.ignore")).on_click(act(RelAction::Remove)));
        }
        RelationshipKind::Outgoing => {
            actions = actions.child(round_button(("fr-cancel", user.0), "close", critical, crate::t!("common.cancel")).on_click(act(RelAction::Remove)));
        }
        RelationshipKind::Blocked => {
            actions = actions.child(
                round_button(("fr-unblock", user.0), "members", critical, crate::t!("friends.unblock"))
                    .on_click(act(RelAction::Remove)),
            );
        }
    }
    let tag = u.and_then(|u| u.tag.clone());
    div()
        .id(("friend", user.0))
        .group("friend-row")
        .h(z(61.))
        .flex_none()
        .ml(z(14.))
        .mr(z(10.))
        .px(z(10.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .on_click(cx.listener(move |a, _, _, cx| a.open_dm_with(user, cx)))
        .child(div().mr(z(12.)).child(crate::ui::avatar_with_status_deco(
            u,
            32.,
            p,
            c(Palette::BG_PRIMARY),
            |d| d.group_hover("friend-row", |s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))),
            crate::ui::avatar_deco::source(u, None),
        )))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(z(20.))
                        .flex()
                        .items_center()
                        .child(
                            crate::fonts::styled_name(div(), u, false)
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(Palette::TEXT_HEADER))
                                .text_ellipsis()
                                .child(name),
                        )
                        .when_some(tag, |d, t| d.child(crate::ui::members::server_tag(&t).ml(z(4.))))
                        .when(!pending, |d| d.child(
                            div()
                                .ml(z(5.))
                                .text_size(z(14.))
                                .line_height(z(16.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c(Palette::TEXT_NORMAL))
                                .opacity(0.)
                                .group_hover("friend-row", |s| s.opacity(1.))
                                .child(username),
                        )),
                )
                .child(
                    div()
                        .text_size(z(14.))
                        .line_height(z(if pending { 18. } else { 17.5 }))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(if pending { tok(idx::TEXT_SUBTLE) } else { Palette::TEXT_MUTED }))
                        .text_ellipsis()
                        .child(sub),
                ),
        )
        .child(actions)
}

fn add_friend(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let input = app.nav.add_friend_input.clone();
    let result = app.nav.add_friend_result.clone();
    let border = match &result {
        Some(Ok(_)) => Palette::GREEN,
        Some(Err(_)) => Palette::RED,
        None => Palette::BG_TERTIARY,
    };
    div()
        .px(z(30.))
        .py(z(20.))
        .flex()
        .flex_col()
        .border_b_1()
        .border_color(c(Palette::DIVIDER))
        .child(
            div()
                .text_size(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .child(crate::t!("friends.h_add")),
        )
        .child(
            div()
                .mt(z(8.))
                .text_size(z(14.))
                .text_color(c(Palette::TEXT_MUTED))
                .child(crate::t!("friends.add_hint")),
        )
        .child(
            div()
                .mt(z(16.))
                .h(z(52.))
                .px(z(12.))
                .rounded(z(8.))
                .bg(c(Palette::BG_TERTIARY))
                .border_1()
                .border_color(c(border))
                .flex()
                .items_center()
                .gap(z(12.))
                .child(
                    div()
                        .flex_1()
                        .text_size(z(16.))
                        .when_some(input, |d, i| d.child(Input::new(&i).appearance(false).bordered(false))),
                )
                .child(
                    div()
                        .id("send-fr")
                        .h(z(32.))
                        .px(z(16.))
                        .rounded(z(4.))
                        .bg(c(Palette::BLURPLE))
                        .text_color(gpui::white())
                        .text_size(z(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .on_click(cx.listener(|a, _, _, cx| send_request(a, cx)))
                        .child(crate::t!("friends.send_request")),
                ),
        )
        .when_some(result, |d, r| {
            let (col, text) = match r {
                Ok(t) => (Palette::GREEN, t),
                Err(t) => (Palette::RED, t),
            };
            d.child(div().mt(z(8.)).text_size(z(14.)).text_color(c(col)).child(text))
        })
}

fn active_now(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let mut active: Vec<(Id, SharedString)> = app
        .nav
        .relationships
        .iter()
        .filter(|r| r.kind == RelationshipKind::Friend)
        .filter_map(|r| app.nav.activities.get(&r.user).map(|a| (r.user, a.clone())))
        .filter(|(u, _)| app.presence_of(*u) != Presence::Offline)
        .collect();
    active.sort_by_key(|(u, _)| u.0);
    let mut col = div()
        .id("active-now")
        .w(z(360.))
        .flex_none()
        .h_full()
        .border_l_1()
        .border_color(c(Palette::DIVIDER))
        .pt(z(14.))
        .pl(z(17.))
        .pr(z(16.))
        .pb(z(16.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .child(
            div()
                .mb(z(9.))
                .text_size(z(20.))
                .line_height(z(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .child(crate::t!("friends.active_now_title")),
        );
    if active.is_empty() {
        return col.child(
            div()
                .mt(z(12.))
                .p(z(16.))
                .rounded(z(8.))
                .bg(c(crate::theme::tok(crate::theme::generated::idx::BACKGROUND_BASE_LOW)))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .mb(z(4.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(crate::theme::tok(crate::theme::generated::idx::TEXT_STRONG)))
                        .child(crate::t!("friends.quiet")),
                )
                .child(
                    div()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_TEXT_DEFAULT)))
                        .text_center()
                        .child(crate::t!("friends.quiet_sub")),
                ),
        );
    }
    let now = crate::mock::now();
    let thumb = |img: Option<SharedString>, size: f32| {
        div()
            .size(z(size))
            .flex_none()
            .rounded(z(4.))
            .overflow_hidden()
            .when_some(img, |d, url| d.child(gpui::img(url).size_full()))
    };
    for (user, activity) in active {
        let u = app.users.get(&user);
        let game = app.people.activities.get(&user).and_then(|v| v.iter().find(|a| a.kind == crate::presence::ActivityKind::Playing)).cloned();
        let sub: SharedString = match &game {
            Some(g) => match g.start {
                Some(st) => format!("{} - {}", g.name, crate::ui::members::elapsed(st, now)).into(),
                None => g.name.clone(),
            },
            None => activity,
        };
        let mut card = div()
            .id(("active", user.0))
            .mt(z(12.))
            .rounded(z(16.))
            .border_1()
            .border_color(c(Palette::DIVIDER))
            .shadow(crate::ui::inbox::low_shadow())
            .overflow_hidden()
            .bg(c(Palette::BG_PRIMARY))
            .cursor_pointer()
            .on_click(cx.listener(move |a, _, _, cx| a.open_dm_with(user, cx)))
            .child(
                div()
                    .p(z(16.))
                    .flex()
                    .items_center()
                    .gap(z(12.))
                    .group("active-card-head")
                    .bg(c(Palette::BG_MSG_HOVER))
                    .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
                    .child(crate::ui::avatar_with_status_deco(
                        u,
                        32.,
                        app.presence_of(user),
                        c(Palette::BG_PRIMARY),
                        |d| d.bg(c(Palette::BG_MSG_HOVER)).group_hover("active-card-head", |s| s.bg(c(Palette::BG_MOD_HOVER))),
                        crate::ui::avatar_deco::source(u, None),
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(z(16.))
                                    .line_height(z(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(Palette::TEXT_NORMAL))
                                    .text_ellipsis()
                                    .child(u.map(|u| u.display_name.clone()).unwrap_or_default()),
                            )
                            .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(Palette::TEXT_MUTED)).text_ellipsis().child(sub)),
                    )
                    .when_some(game.as_ref().and_then(|g| g.image.clone()), |d, i| d.child(thumb(Some(i), 24.))),
            );
        let stream = app.people.activities.get(&user).and_then(|v| v.iter().find(|a| a.kind == crate::presence::ActivityKind::Streaming)).cloned();
        if let Some(s) = stream {
            card = card.child(
                div()
                    .p(z(16.))
                    .flex()
                    .flex_col()
                    .gap(z(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(12.))
                            .child(crate::ui::avatar(u, 32.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(z(14.))
                                            .line_height(z(18.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(c(Palette::TEXT_NORMAL))
                                            .text_ellipsis()
                                            .child(u.map(|u| u.display_name.clone()).unwrap_or_default()),
                                    )
                                    .child(
                                        div()
                                            .text_size(z(12.))
                                            .line_height(z(16.))
                                            .text_color(c(Palette::TEXT_MUTED))
                                            .text_ellipsis()
                                            .child(s.details.clone().unwrap_or(s.name.clone())),
                                    ),
                            )
                            .child(
                                div()
                                    .h(z(16.))
                                    .px(z(8.))
                                    .rounded_full()
                                    .bg(c(crate::theme::tok(crate::theme::generated::idx::STATUS_DANGER)))
                                    .flex()
                                    .items_center()
                                    .text_size(z(12.))
                                    .line_height(z(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(gpui::white())
                                    .child("LIVE"),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(z(165.))
                            .rounded(z(8.))
                            .overflow_hidden()
                            .bg(c(crate::theme::tok(crate::theme::generated::idx::BACKGROUND_BASE_LOWEST)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(s.image.clone(), |d, url| d.child(gpui::img(url).size_full().object_fit(gpui::ObjectFit::Cover))),
                    ),
            );
        } else if let Some(g) = game {
            card = card.child(
                div()
                    .p(z(16.))
                    .flex()
                    .items_center()
                    .gap(z(12.))
                    .child(thumb(g.image.clone(), 32.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(z(14.))
                                    .line_height(z(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(Palette::TEXT_NORMAL))
                                    .text_ellipsis()
                                    .child(g.name.clone()),
                            )
                            .when_some(g.state.clone().or(g.details.clone()), |d, t| {
                                d.child(div().text_size(z(12.)).line_height(z(16.)).text_color(c(Palette::TEXT_MUTED)).text_ellipsis().child(t))
                            }),
                    )
                    .child(crate::ui::avatar(u, 24.)),
            );
        }
        col = col.child(card);
    }
    col
}

fn message_requests(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let reqs: Vec<Id> = app.nav.message_requests.iter().copied().filter(|c| app.channels.contains_key(c)).collect();
    let mut body = div().id("msg-requests").flex_1().overflow_y_scroll().p(z(20.)).flex().flex_col();
    if reqs.is_empty() {
        body = body.child(
            div()
                .pt(z(160.))
                .flex()
                .justify_center()
                .text_size(z(16.))
                .text_color(c(Palette::TEXT_MUTED))
                .child(crate::t!("friends.no_message_requests")),
        );
    }
    for id in reqs {
        let ch = &app.channels[&id];
        let u = ch.voice_users.first().and_then(|u| app.users.get(u));
        body = body.child(
            div()
                .id(("req", id.0))
                .h(z(62.))
                .flex()
                .items_center()
                .gap(z(12.))
                .border_t_1()
                .border_color(c(Palette::DIVIDER))
                .cursor_pointer()
                .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
                .on_click(cx.listener(move |a, _, _, cx| a.select_channel(id, cx)))
                .child(avatar_with_status(u, 32., Presence::Offline, c(Palette::BG_PRIMARY)))
                .child(div().text_size(z(16.)).text_color(c(Palette::TEXT_HEADER)).child(ch.name.clone())),
        );
    }
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(c(Palette::BG_PRIMARY))
        .child(
            div()
                .h(px(m::HEADER_H))
                .flex_none()
                .px(z(16.))
                .flex()
                .items_center()
                .gap(z(8.))
                .border_b_1()
                .border_color(c(Palette::HEADER_SHADOW))
                .child(icon(Icon::Inbox, 24., c(Palette::CHANNEL_ICON)))
                .child(
                    div()
                        .text_size(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(Palette::TEXT_HEADER))
                        .child(crate::t!("friends.message_requests")),
                ),
        )
        .child(body)
}

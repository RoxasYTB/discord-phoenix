use crate::ui::tooltip::Tipped as _;
use crate::theme::z;
use gpui::{
    AnyElement, Context, Focusable, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px,
};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    model::{Id, Presence},
    theme::{Palette, c, generated::idx, m, tok},
    ui::{
        badge, icon,
        home::data::{FriendsTab, HomeView, NavRequest, RelAction, Relationship, RelationshipKind},
    },
};

pub fn render(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    if app.nav.home == HomeView::MessageRequests {
        return crate::ui::home::requests::render(app, cx);
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
        crate::ui::locale_inputs::keep_placeholder(&i, "friends.add_placeholder", window, cx);
        if app.nav.add_friend_reset {
            i.update(cx, |s, cx| {
                s.set_value("", window, cx);
                s.focus(window, cx);
            });
        }
    }
    if let Some(n) = app.nav.add_friend_note.clone()
        && std::mem::take(&mut app.nav.add_friend_reset)
    {
        n.update(cx, |s, cx| s.set_value("", window, cx));
    }
    let body: AnyElement = match app.nav.friends_tab {
        FriendsTab::AddFriend => add_friend(app, window, cx).into_any_element(),
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
                .when(crate::ui::title_bar::active_now_shown(crate::ui::title_bar::viewport_w_css()), |d| d.child(active_now(app, cx))),
        )
        .into_any_element()
}

fn ensure_add_input(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    use crate::ui::home::add_friend::{NOTE_MAX, TAG_MAX};
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("friends.add_placeholder")));
    let last = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let sub = cx.subscribe_in(&input, window, move |this: &mut PhoenixApp, st, ev: &InputEvent, window, cx| match ev {
        InputEvent::PressEnter { .. } => {
            if this.nav.add_friend.can_send {
                send_request(this, cx);
            }
        }
        InputEvent::Change => {
            let now = st.read(cx).value().to_string();
            let now = match crate::ui::modals::clamp_insert(&last.borrow(), &now, TAG_MAX) {
                Some((text, cursor)) => {
                    st.update(cx, |s, cx| {
                        s.set_value(text.clone(), window, cx);
                        s.set_selected_range(cursor..cursor, cx);
                    });
                    text
                }
                None => now,
            };
            this.nav.add_friend.on_change(&now);
            this.nav.add_friend_success_tag = None;
            *last.borrow_mut() = now;
            cx.notify();
        }
        _ => {}
    });
    app.nav.subs.push(sub);
    app.nav.add_friend_input = Some(input);
    let note = cx.new(|cx| {
        gpui_component::input::TextareaState::new(window, cx).auto_grow(2, 2).placeholder(crate::t!("friends.add_note_placeholder"))
    });
    let last = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let sub = cx.subscribe_in(&note, window, move |_: &mut PhoenixApp, st, ev: &InputEvent, window, cx| {
        if !matches!(ev, InputEvent::Change) {
            return;
        }
        let raw = st.read(cx).value().to_string();
        let flat = raw.replace('\n', "");
        let next = crate::ui::modals::clamp_insert(&last.borrow(), &flat, NOTE_MAX).map(|(t, _)| t).unwrap_or(flat);
        if next != raw {
            let cursor = next.len();
            st.update(cx, |s, cx| {
                s.set_value(next.clone(), window, cx);
                s.set_selected_range(cursor..cursor, cx);
            });
        }
        *last.borrow_mut() = next;
        cx.notify();
    });
    app.nav.subs.push(sub);
    app.nav.add_friend_note = Some(note);
}

fn send_request(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    use crate::ui::home::add_friend as af;
    let Some(input) = app.nav.add_friend_input.clone() else { return };
    let tag = af::normalize(&input.read(cx).value());
    let raw_note = app.nav.add_friend_note.as_ref().map(|n| n.read(cx).value().to_string()).unwrap_or_default();
    if !af::valid(&tag) {
        app.nav.add_friend.on_error(crate::t!("friends.request_failed"));
        cx.notify();
        return;
    }
    if af::note_too_long(&raw_note) {
        app.nav.add_friend.on_error(crate::t!("friends.err_note"));
        cx.notify();
        return;
    }
    let note = af::clean_note(&raw_note);
    app.nav_request(NavRequest::Relationship { user: Id(0), action: RelAction::AddByTag { tag: tag.clone(), note } }, cx, move |this, res, _cx| {
        match res {
            Ok(_) => {
                this.nav.add_friend.on_success(&tag);
                this.nav.add_friend_success_tag = Some(tag.clone().into());
                this.nav.add_friend_reset = true;
            }
            Err(e) => {
                let f = af::Failure::decode(&e.0).unwrap_or(af::Failure { status: 0, code: None, note_rejected: false });
                if let Some(d) = af::dialog(&f, &tag) {
                    open_failure_dialog(d);
                }
                this.nav.add_friend.on_error(af::inline_error(&f, &tag));
            }
        }
    });
}

fn open_failure_dialog(d: crate::ui::home::add_friend::Dialog) {
    use crate::ui::home::add_friend::Dialog;
    let (title, body, ok) = match d {
        Dialog::TooFast => (crate::t!("friends.too_fast_title"), crate::t!("friends.too_fast_body"), crate::t!("friends.too_fast_ok")),
        Dialog::VerifyEmail => (crate::t!("friends.verify_title"), crate::t!("friends.verify_body"), crate::t!("friends.verify_ok")),
        Dialog::Failed(body) => (crate::t!("friends.fail_title"), body, crate::t!("friends.fail_ok")),
    };
    crate::actions::open_modal(
        crate::ui::modals::Modal::Form(crate::ui::modals::Form {
            title,
            subtitle: Some(body),
            fields: Vec::new(),
            choices: Vec::new(),
            choice: None,
            check: None,
            submit: ok,
            danger: false,
            on_submit: std::rc::Rc::new(|_, _, _, _| {}),
            preview: None,
            width: 440.,
        }),
        None,
    );
}

pub fn pending_badge(rels: &[Relationship]) -> usize {
    rels.iter().filter(|r| r.kind == RelationshipKind::Incoming && !r.spam && !r.ignored).count()
}

pub fn initial_tab(rels: &[Relationship]) -> FriendsTab {
    let n = |k| rels.iter().filter(|r| r.kind == k).count();
    if n(RelationshipKind::Friend) > 0 {
        FriendsTab::Online
    } else if n(RelationshipKind::Incoming) > 0 {
        FriendsTab::Pending
    } else {
        FriendsTab::AddFriend
    }
}

fn header(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let pending = pending_badge(&app.nav.relationships) as u32;
    let friends = app.nav.count(RelationshipKind::Friend);
    let online = app.nav.relationships.iter().any(|r| r.kind == RelationshipKind::Friend && app.presence_of(r.user) != Presence::Offline);
    let has_pending = app.nav.count(RelationshipKind::Incoming) + app.nav.count(RelationshipKind::Outgoing) > 0;
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
        .child(div().size(z(32.)).flex().items_center().justify_center().child(icon(Icon::Friends, 20. * crate::theme::zoom(), c(Palette::CHANNEL_ICON))))
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
        .when(online, |d| d.child(tab("tab-online", crate::t!("status.online"), FriendsTab::Online, cx)))
        .when(friends > 0, |d| d.child(tab("tab-all", crate::t!("friends.all"), FriendsTab::All, cx)))
        .when(has_pending, |d| d.child(tab("tab-pending", crate::t!("friends.pending"), FriendsTab::Pending, cx)))
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
                    cx.notify();
                }))
                .child(crate::t!("friends.add_tab")),
        )
        .child(div().flex_1())
        .when(crate::ui::title_bar::friends_invite_shown(crate::ui::title_bar::viewport_w_css()), |d| {
            d.child(
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
                            .on_click(cx.listener(|a, ev: &gpui::ClickEvent, _, cx| a.open_group_dm_picker_at(Some((ev.position() + gpui::point(gpui::px(0.), z(20.)), gpui::Anchor::TopRight)), cx)))
                            .child(
                                gpui::svg()
                                    .path(crate::icons::path("new-group-dm"))
                                    .size(z(20.))
                                    .text_color(c(tok(idx::ICON_MUTED)))
                                    .group_hover("fr-new-group-dm", |s| s.text_color(c(tok(idx::ICON_SUBTLE)))),
                            ),
                    ),
            )
        })
}

pub fn in_tab(r: &Relationship, tab: FriendsTab, online: bool) -> bool {
    match tab {
        FriendsTab::Online => r.kind == RelationshipKind::Friend && online,
        FriendsTab::All => r.kind == RelationshipKind::Friend,
        FriendsTab::Pending => (r.kind == RelationshipKind::Incoming && !r.spam && !r.ignored) || r.kind == RelationshipKind::Outgoing,
        FriendsTab::Blocked => r.kind == RelationshipKind::Blocked,
        FriendsTab::AddFriend => false,
    }
}

pub fn order(rows: &mut [Relationship], key: impl Fn(&Relationship) -> String, tab: FriendsTab) {
    let rank = |k: RelationshipKind| match k {
        RelationshipKind::None => 0,
        RelationshipKind::Friend => 1,
        RelationshipKind::Blocked => 2,
        RelationshipKind::Incoming => 3,
        RelationshipKind::Outgoing => 4,
    };
    rows.sort_by_cached_key(|r| (rank(r.kind), key(r)));
    if tab == FriendsTab::Pending {
        rows.sort_by(|a, b| {
            rank(a.kind).cmp(&rank(b.kind)).then_with(|| if a.kind == RelationshipKind::Incoming { b.since.cmp(&a.since) } else { std::cmp::Ordering::Equal })
        });
    }
}

pub fn matches_query(r: &Relationship, username: &str, global: Option<&str>, q: &str) -> bool {
    q.is_empty()
        || [Some(username), r.nickname.as_deref(), global].into_iter().flatten().any(|v| v.to_lowercase().contains(q))
}

fn visible(app: &PhoenixApp, tab: FriendsTab) -> Vec<Relationship> {
    let mut v: Vec<Relationship> = app
        .nav
        .relationships
        .iter()
        .filter(|r| app.users.contains_key(&r.user) && in_tab(r, tab, app.presence_of(r.user) != Presence::Offline))
        .cloned()
        .collect();
    order(
        &mut v,
        |r| {
            let u = app.users.get(&r.user);
            r.nickname
                .as_ref()
                .map(|n| n.to_lowercase())
                .unwrap_or_else(|| u.map(|u| u.display_name.to_lowercase()).unwrap_or_default())
        },
        tab,
    );
    v
}

fn list(app: &PhoenixApp, tab: FriendsTab, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let q = app.nav.friends_search.as_ref().map(|i| i.read(cx).value().to_lowercase()).unwrap_or_default();
    let rows: Vec<Relationship> = visible(app, tab)
        .into_iter()
        .filter(|r| app.users.get(&r.user).is_some_and(|u| matches_query(r, &u.name, Some(u.display_name.as_ref()), &q)))
        .collect();
    let others = tab == FriendsTab::Pending
        && app.nav.relationships.iter().any(|r| r.kind == RelationshipKind::Incoming && (r.spam || r.ignored));
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
        .child(div().size(z(30.)).flex().items_center().justify_center().child(icon(Icon::Search, 16. * crate::theme::zoom(), c(Palette::TEXT_MUTED))))
        .when_some(app.nav.friends_search.clone(), |d, i| {
            d.child(div().flex_1().child(Input::new(&i).appearance(false).bordered(false).pl(z(0.)).text_size(z(16.)).line_height(z(20.))))
        });
    if rows.is_empty() && q.is_empty() {
        return empty_state(tab, others, cx).into_any_element();
    }
    let scroll = FRIENDS_SCROLL.with(|h| h.clone());
    let mut col =
        div().id("friends-list").size_full().overflow_y_scroll().track_scroll(&scroll).pb(z(8.)).pr(z(16.)).flex().flex_col();
    if rows.is_empty() {
        col = col.child(
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .w_full()
                    .max_w(z(440.))
                    .mt(z(8.))
                    .text_center()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_MUTED)))
                    .child(crate::t!("friends.empty_search")),
            ),
        );
    } else {
        let incoming = rows.iter().filter(|r| r.kind == RelationshipKind::Incoming).count();
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
        for (n, (title, rows)) in sections.into_iter().enumerate() {
            let clear = tab == FriendsTab::Pending && n == 0 && incoming >= 5;
            col = col.child(
                div()
                    .h(z(50.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .ml(z(24.))
                            .mr(z(20.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(Palette::TEXT_NORMAL))
                            .child(title),
                    )
                    .when(clear, |d| {
                        d.child(
                            text_button("fr-clear-all", crate::t!("friends.clear_all"), 14., 18., idx::TEXT_BRAND)
                                .mr(z(24.))
                                .on_click(cx.listener(move |a, _, _, cx| a.confirm_clear_pending(incoming, cx))),
                        )
                    }),
            );
            for r in rows {
                col = col.child(div().flex_none().h(z(1.)).ml(z(30.)).mr(z(20.)).bg(c(Palette::DIVIDER)));
                col = col.child(friend_row(app, &r, cx));
            }
        }
        if others {
            col = col.child(
                text_button("fr-other-requests", crate::t!("friends.view_other_requests"), 12., 16., idx::TEXT_STRONG)
                    .mx(z(24.))
                    .py(z(4.))
                    .pr(z(8.))
                    .on_click(cx.listener(|a, _, _, cx| a.open_spam_requests(cx))),
            );
        }
    }
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(search)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(col)
                .children(crate::ui::chat::scrollbar::auto_scrollbar_div(&scroll)),
        )
        .into_any_element()
}

fn text_button(id: &'static str, label: SharedString, size: f32, line: f32, color: usize) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .self_start()
        .cursor_pointer()
        .text_size(z(size))
        .line_height(z(line))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(color)))
        .hover(|s| s.underline())
        .child(label)
}

fn empty_state(tab: FriendsTab, others: bool, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let msg = match tab {
        FriendsTab::Online => crate::t!("friends.empty_online"),
        FriendsTab::Pending => crate::t!("friends.empty_pending"),
        FriendsTab::Blocked => crate::t!("friends.empty_blocked"),
        _ => crate::t!("friends.empty_all"),
    };
    let button: Option<AnyElement> = match tab {
        FriendsTab::All => Some(
            crate::ui::controls::button("friends-empty-add", crate::t!("friends.add_tab"), crate::ui::controls::BtnKind::Primary)
                .mt(z(20.))
                .on_click(cx.listener(|a, _, _, cx| {
                    a.nav.friends_tab = FriendsTab::AddFriend;
                    cx.notify();
                }))
                .into_any_element(),
        ),
        FriendsTab::Pending if others => Some(
            crate::ui::controls::button("friends-empty-spam", crate::t!("friends.view_spam_requests"), crate::ui::controls::BtnKind::Secondary)
                .mt(z(20.))
                .h(z(32.))
                .min_w(z(60.))
                .px(z(12.))
                .text_size(z(14.))
                .line_height(z(18.))
                .on_click(cx.listener(|a, _, _, cx| a.open_spam_requests(cx)))
                .into_any_element(),
        ),
        _ => None,
    };
    div().size_full().flex().items_center().justify_center().child(
        div()
            .w_full()
            .max_w(z(440.))
            .flex()
            .flex_col()
            .items_center()
            .child(div().mt(z(8.)).text_center().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_MUTED))).child(msg))
            .children(button),
    )
}

thread_local! {
    static DISCOVERY_ARROW_W: std::cell::Cell<f32> = const { std::cell::Cell::new(20.) };
}

thread_local! {
    static FRIENDS_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
    static ACTIVE_NOW_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

fn gpui_input_bg() -> u32 {
    crate::theme::tok(crate::theme::generated::idx::INPUT_BACKGROUND_DEFAULT)
}

pub fn action_button(id: (&'static str, u64), name: &str, accept: bool, tip: SharedString) -> gpui::Stateful<gpui::Div> {
    round_button(id, name, c(tok(if accept { idx::ICON_FEEDBACK_POSITIVE } else { idx::ICON_FEEDBACK_CRITICAL })), tip)
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
        RelationshipKind::Friend if r.ignored => crate::t!("friends.ignored"),
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
    let sub = friend_activity(app, user, r.kind).unwrap_or_else(|| sub.into_any_element());
    let mut actions = div().flex().gap(z(10.)).when(pending, |d| d.ml(z(8.)));
    let act = |action: RelAction| {
        cx.listener(move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _, cx| {
            cx.stop_propagation();
            a.relationship_action(user, action.clone(), cx);
        })
    };
    let hover = c(tok(idx::INTERACTIVE_TEXT_HOVER));
    match r.kind {
        RelationshipKind::None => {}
        RelationshipKind::Friend => {
            actions = actions
                .child(round_button(("fr-msg", user.0), "chat", hover, crate::t!("profile.message")).on_click(cx.listener(move |a, _, _, cx| {
                    cx.stop_propagation();
                    a.open_dm_with(user, cx)
                })))
                .child({
                    round_button(("fr-more", user.0), "more-vertical", hover, crate::t!("header.more")).on_click(move |ev, window, cx| {
                        cx.stop_propagation();
                        crate::ui::extra_menus::open(crate::ui::extra_menus::Target::FriendRow(user), ev.position(), window);
                    })
                });
        }
        RelationshipKind::Incoming => {
            actions = actions
                .child(action_button(("fr-ok", user.0), "check", true, crate::t!("friends.accept")).on_click(cx.listener(move |a, _, _, cx| {
                    cx.stop_propagation();
                    a.accept_friend_request(user, cx);
                })))
                .child(action_button(("fr-no", user.0), "close", false, crate::t!("friends.ignore")).on_click(act(RelAction::Remove)));
        }
        RelationshipKind::Outgoing => {
            actions = actions.child(action_button(("fr-cancel", user.0), "close", false, crate::t!("common.cancel")).on_click(act(RelAction::Remove)));
        }
        RelationshipKind::Blocked => {
            actions = actions.child(
                round_button(("fr-unblock", user.0), "members", c(tok(idx::ICON_FEEDBACK_CRITICAL)), crate::t!("friends.unblock"))
                    .on_click(act(RelAction::Remove)),
            );
        }
    }
    let revealed = app.nav.revealed_notes.contains(&user);
    let note_line: Option<AnyElement> = r.note.clone().filter(|_| pending).map(|note| {
        if r.kind == RelationshipKind::Incoming && !revealed {
            div()
                .id(("fr-note", user.0))
                .flex()
                .items_center()
                .gap(z(4.))
                .cursor_pointer()
                .text_size(z(14.))
                .line_height(z(17.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_BRAND)))
                .hover(|s| s.underline())
                .on_click(cx.listener(move |a, _, _, cx| {
                    cx.stop_propagation();
                    a.nav.revealed_notes.insert(user);
                    cx.notify();
                }))
                .child(gpui::svg().path(crate::icons::path("chat/eye")).size(z(16.)).text_color(c(tok(idx::TEXT_BRAND))))
                .child(crate::t!("friends.view_request"))
                .into_any_element()
        } else {
            div()
                .flex()
                .items_start()
                .gap(z(4.))
                .italic()
                .text_size(z(14.))
                .line_height(z(17.5))
                .text_color(c(tok(idx::TEXT_SUBTLE)))
                .child(div().pt(z(3.)).child(gpui::svg().path(crate::icons::path("settings/quote")).size(z(12.)).text_color(c(tok(idx::ICON_SUBTLE)))))
                .child(div().min_w_0().text_ellipsis().child(note))
                .into_any_element()
        }
    });
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
        .on_click(cx.listener(move |a, _, _, cx| if pending { a.open_profile_modal(user, cx) } else { a.open_dm_with(user, cx) }))
        .child(div().mr(z(12.)).child(if pending && p == Presence::Offline {
            div()
                .relative()
                .size(z(32.))
                .child(crate::ui::avatar(u, 32.))
                .when_some(crate::ui::avatar_deco::source(u, None), |d, src| d.child(crate::ui::avatar_deco::layer(src, 32.)))
        } else {
            crate::ui::avatar_with_status_deco(
                u,
                32.,
                p,
                c(Palette::BG_PRIMARY),
                |d| d.group_hover("friend-row", |s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))),
                crate::ui::avatar_deco::source(u, None),
            )
        }))
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
                .map(|d| match note_line {
                    Some(n) => d.child(n),
                    None => d.child(
                        div()
                            .text_size(z(14.))
                            .line_height(z(if pending { 18. } else { 17.5 }))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(if pending { tok(idx::TEXT_SUBTLE) } else { Palette::TEXT_MUTED }))
                            .text_ellipsis()
                            .child(sub),
                    ),
                }),
        )
        .child(actions)
}

fn friend_activity(app: &PhoenixApp, user: Id, kind: RelationshipKind) -> Option<gpui::AnyElement> {
    use crate::presence::ActivityKind as K;
    if kind != RelationshipKind::Friend || app.presence_of(user) == Presence::Offline {
        return None;
    }
    let acts = app.people.activities.get(&user)?;
    let main = acts.iter().find(|a| a.kind != K::Custom);
    let custom = acts
        .iter()
        .find(|a| a.kind == K::Custom)
        .map(|a| (a.emoji.clone(), a.state.clone().filter(|t| !t.trim().is_empty())))
        .filter(|(e, t)| e.is_some() || t.is_some());
    if main.is_none() && custom.is_none() {
        return None;
    }
    let label = |t: SharedString| div().min_w_0().text_ellipsis().whitespace_nowrap().overflow_hidden().child(crate::ui::emoji_text::status_label(t, 16. * crate::theme::zoom() / 1.375));
    let mut row = div().min_w_0().flex().items_center().gap(z(4.)).overflow_hidden();
    if let Some(a) = main {
        if let Some(icon) = crate::ui::members::activity_icon(a) {
            row = row.child(gpui::svg().path(crate::icons::path(icon)).size(z(14.)).flex_none().text_color(c(tok(idx::STATUS_ONLINE))));
        }
        if let Some(t) = crate::ui::members::activity_text(a) {
            row = row.child(label(t));
        }
    }
    if let Some((emoji, state)) = custom {
        if main.is_some() {
            row = row.child(div().flex_none().child("•"));
        }
        if let Some(e) = emoji {
            row = row.child(crate::ui::members::status_emoji(&e, 16.));
        }
        if let Some(t) = state {
            row = row.child(label(t));
        }
    }
    Some(row.into_any_element())
}

pub const WUMPUS_WAVING: &str = "https://discord.com/assets/25c27c1328c986f6.svg";

fn add_friend(app: &PhoenixApp, window: &Window, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    use crate::ui::home::add_friend::NOTE_MAX;
    let input = app.nav.add_friend_input.clone();
    let note = app.nav.add_friend_note.clone();
    let form = app.nav.add_friend.clone();
    let note_len: usize = note.as_ref().map(|n| n.read(cx).value().chars().map(char::len_utf16).sum()).unwrap_or(0);
    let focused = input.as_ref().is_some_and(|i| i.read(cx).focus_handle(cx).contains_focused(window, cx))
        || note.as_ref().is_some_and(|n| n.read(cx).focus_handle(cx).contains_focused(window, cx));
    let (bg, border) = if form.error.is_some() {
        (tok(idx::INPUT_BACKGROUND_ERROR_DEFAULT), tok(idx::INPUT_BORDER_ERROR_DEFAULT))
    } else if form.success.is_some() {
        (tok(idx::INPUT_BACKGROUND_DEFAULT), tok(idx::GREEN_360))
    } else if focused {
        (tok(idx::INPUT_BACKGROUND_DEFAULT), tok(idx::TEXT_LINK))
    } else {
        (tok(idx::INPUT_BACKGROUND_DEFAULT), tok(idx::INPUT_BORDER_DEFAULT))
    };
    let can_send = form.can_send;
    let send = crate::ui::controls::button("send-fr", crate::t!("friends.send_request"), crate::ui::controls::BtnKind::Primary)
        .h(z(32.))
        .min_w(z(60.))
        .px(z(12.))
        .text_size(z(14.))
        .line_height(z(18.))
        .when(!can_send, |b| b.opacity(0.5).cursor_default())
        .on_click(cx.listener(move |a, _, _, cx| {
            if a.nav.add_friend.can_send {
                send_request(a, cx);
            }
        }));
    let status_line = |text: AnyElement| div().mt(z(8.)).text_size(z(14.)).line_height(z(18.)).child(text);
    let message: AnyElement = match (&form.error, &form.success) {
        (Some(e), _) => status_line(div().text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child(e.clone()).into_any_element()).into_any_element(),
        (None, Some(ok)) => {
            let tag = app.nav.add_friend_success_tag.clone().unwrap_or_default();
            let text = gpui::StyledText::new(ok.clone()).with_highlights(
                ok.find(tag.as_ref())
                    .filter(|_| !tag.is_empty())
                    .map(|i| (i..i + tag.len(), gpui::HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })),
            );
            status_line(div().text_color(c(tok(idx::TEXT_FEEDBACK_POSITIVE))).child(text).into_any_element()).into_any_element()
        }
        (None, None) => status_line(div().text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("friends.add_note_hint")).into_any_element())
            .into_any_element(),
    };
    let heading = |t: SharedString| {
        div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(t)
    };
    let lead = |t: SharedString| {
        div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(t)
    };
    let form_box = div()
        .mt(z(16.))
        .px(z(12.))
        .rounded(z(16.))
        .border_1()
        .border_color(c(border))
        .bg(c(bg))
        .flex()
        .flex_col()
        .child(
            div()
                .py(z(2.))
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .mr(z(16.))
                        .h(z(52.))
                        .py(z(4.))
                        .flex()
                        .items_center()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .when_some(input, |d, i| d.child(Input::new(&i).appearance(false).bordered(false).pl(z(0.)))),
                )
                .child(send),
        )
        .child(
            div()
                .border_t_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .py(z(12.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(z(41.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_SUBTLE)))
                        .when_some(note, |d, n| d.child(gpui_component::input::Textarea::new(&n).appearance(false).bordered(false))),
                )
                .child(
                    div().pt(z(4.)).flex().justify_end().child(
                        div()
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(c(tok(if note_len >= NOTE_MAX { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_SUBTLE })))
                            .child(SharedString::from(NOTE_MAX.saturating_sub(note_len).to_string())),
                    ),
                ),
        );
    let explore = div()
        .id("add-friend-discovery")
        .group("add-friend-discovery")
        .w_full()
        .min_w_0()
        .px(z(6.))
        .py(z(1.))
        .flex()
        .items_center()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
        .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
        .active(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE))))
        .cursor_pointer()
        .on_click(cx.listener(|_, _, _, cx| {
            crate::ui::discovery::open(crate::ui::discovery::Tab::Servers);
            cx.notify();
        }))
        .child(
            div()
                .m(z(8.))
                .size(z(36.))
                .flex_none()
                .rounded(z(8.))
                .bg(c(tok(idx::GREEN_360)))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("compass")).size(z(24.)).text_color(gpui::white())),
        )
        .child(
            div()
                .flex_initial()
                .min_w_0()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(crate::t!("friends.explore_discovery")),
        )
        .child({
            let w = DISCOVERY_ARROW_W.with(std::cell::Cell::get);
            div()
                .relative()
                .flex_initial()
                .flex_basis(z(20.))
                .min_w_0()
                .h(z(20.))
                .ml_auto()
                .mr(z(16.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    gpui::svg()
                        .path(crate::icons::path("friends/discovery-arrow"))
                        .flex_none()
                        .size(gpui::px(w))
                        .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                        .group_hover("add-friend-discovery", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))),
                )
                .child(
                    gpui::canvas(
                        |b, window, _| {
                            let side = f32::from(b.size.width).min(f32::from(b.size.height));
                            if (DISCOVERY_ARROW_W.with(std::cell::Cell::get) - side).abs() > 0.1 {
                                DISCOVERY_ARROW_W.with(|c| c.set(side));
                                window.refresh();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
        });
    div()
        .id("add-friend-page")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .px(z(30.))
                .py(z(20.))
                .border_b_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .flex()
                .flex_col()
                .child(heading(crate::t!("friends.h_add")).mb(z(8.)))
                .child(
                    div()
                        .relative()
                        .pr(z(128.))
                        .flex()
                        .items_start()
                        .child(lead(crate::t!("friends.add_hint")))
                        .child(div().absolute().right(z(16.)).bottom(z(-16.)).w(z(84.)).h(z(72.)).child(gpui::img(WUMPUS_WAVING).size_full())),
                )
                .child(form_box)
                .child(message),
        )
        .child(
            div()
                .flex_none()
                .px(z(30.))
                .py(z(20.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(heading(crate::t!("friends.other_places")))
                .child(lead(crate::t!("friends.other_places_body"))),
        )
        .child(div().flex_none().px(z(20.)).flex().gap(z(32.)).child(div().flex_1().min_w_0().child(explore)).child(div().flex_1()))
}

fn active_now_width() -> f32 {
    let vw = crate::ui::title_bar::viewport_w_css();
    if vw <= 0. {
        return 360.;
    }
    crate::ui::title_bar::active_now_w(vw - crate::theme::m::SERVER_BAR_W - crate::ui::sidebar_resize::width())
}

fn active_now(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let scroll = ACTIVE_NOW_SCROLL.with(|h| h.clone());
    div()
        .relative()
        .flex_none()
        .h_full()
        .child(active_now_column(app, cx).track_scroll(&scroll))
        .children(crate::ui::chat::scrollbar::thin_scrollbar_div(&scroll))
}

#[derive(Clone, Debug, PartialEq)]
pub struct NowFriend {
    pub user: Id,
    pub name: SharedString,
    pub voice: Option<Id>,
    pub activity: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NowCard {
    pub voice: Option<Id>,
    pub priority: Vec<Id>,
    pub partied: Vec<Id>,
    pub activity_users: Vec<Id>,
    pub streams: Vec<Id>,
}

pub fn now_cards(friends: &[NowFriend], voice_members: impl Fn(Id) -> Vec<Id>, streaming: impl Fn(Id) -> bool) -> Vec<NowCard> {
    let mut groups: Vec<(Option<Id>, Vec<&NowFriend>)> = Vec::new();
    for f in friends {
        match f.voice.and_then(|v| groups.iter().position(|(g, _)| *g == Some(v))) {
            Some(i) => groups[i].1.push(f),
            None => groups.push((f.voice, vec![f])),
        }
    }
    let mut keyed: Vec<(NowCard, String)> = groups
        .into_iter()
        .map(|(voice, members)| {
            let priority: Vec<Id> = members.iter().map(|f| f.user).collect();
            let mut partied = priority.clone();
            if let Some(v) = voice {
                partied.extend(voice_members(v).into_iter().filter(|u| !priority.contains(u)));
            }
            let card = NowCard {
                voice,
                activity_users: members.iter().filter(|f| f.activity).map(|f| f.user).collect(),
                streams: priority.iter().copied().filter(|u| streaming(*u)).collect(),
                priority,
                partied,
            };
            let names = members.iter().map(|f| f.name.to_lowercase()).collect::<Vec<_>>().join(" ");
            (card, names)
        })
        .filter(|(c, _)| c.voice.is_some() || !c.activity_users.is_empty() || !c.streams.is_empty())
        .collect();
    keyed.sort_by(|(a, an), (b, bn)| {
        let k = |c: &NowCard| (c.partied.len() <= 1, c.streams.is_empty(), c.voice.is_none(), c.activity_users.is_empty());
        k(a).cmp(&k(b)).then_with(|| an.cmp(bn))
    });
    keyed.into_iter().map(|(c, _)| c).collect()
}

pub fn card_title(names: &[SharedString]) -> String {
    let a = names.first().map(|s| s.to_string()).unwrap_or_default();
    let b = names.get(1).map(|s| s.to_string()).unwrap_or_default();
    match names.len() {
        0 | 1 => a,
        2 => crate::t!("friends.now_two", user1 = a, user2 = b).to_string(),
        3 => crate::t!("friends.now_more_one", user1 = a, user2 = b).to_string(),
        n => crate::t!("friends.now_more", user1 = a, user2 = b, extras = n - 2).to_string(),
    }
}

pub fn facepile(total: usize) -> (usize, Option<usize>) {
    let shown = total.min(2);
    match total - shown {
        0 => (shown, None),
        1 => (shown + 1, None),
        rest => (shown, Some(rest.min(99))),
    }
}

fn now_friends(app: &PhoenixApp) -> Vec<NowFriend> {
    let voice_of = |u: Id| {
        app.channels
            .values()
            .find(|c| {
                matches!(c.kind, crate::model::ChannelKind::Voice | crate::model::ChannelKind::Stage)
                    && c.voice_users.contains(&u)
                    && c.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).is_some_and(|g| g.afk_channel != Some(c.id))
            })
            .map(|c| c.id)
    };
    let mut out: Vec<NowFriend> = app
        .nav
        .relationships
        .iter()
        .filter(|r| r.kind == RelationshipKind::Friend && !app.nav.is_blocked(r.user))
        .filter(|r| app.presence_of(r.user) != Presence::Offline)
        .filter_map(|r| {
            let u = app.users.get(&r.user)?;
            if u.bot {
                return None;
            }
            let activity = app.people.activities.get(&r.user).is_some_and(|v| v.iter().any(|a| a.kind != crate::presence::ActivityKind::Custom))
                && app.nav.activities.contains_key(&r.user);
            Some(NowFriend { user: r.user, name: u.name.clone(), voice: voice_of(r.user), activity })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn active_now_column(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let friends = now_friends(app);
    let cards = now_cards(
        &friends,
        |v| app.channels.get(&v).map(|c| c.voice_users.clone()).unwrap_or_default(),
        |u| crate::ui::voice_panel::flags(u).stream,
    );
    let mut col = div()
        .id("active-now")
        .w(z(active_now_width()))
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
    if cards.is_empty() {
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
    for card_info in cards {
        let user = card_info.priority[0];
        let u = app.users.get(&user);
        let lead = if card_info.partied.len() == 1 || card_info.activity_users.len() == 1 {
            card_info.activity_users.first().copied().or(Some(user))
        } else {
            None
        };
        let game = lead.and_then(|id| {
            app.people.activities.get(&id).and_then(|v| v.iter().find(|a| a.kind == crate::presence::ActivityKind::Playing)).cloned()
        });
        let sub: SharedString = match (lead, &game) {
            (Some(_), Some(g)) => match g.start {
                Some(st) => format!("{} - {}", g.name, crate::ui::members::elapsed(st, now)).into(),
                None => g.name.clone(),
            },
            (Some(l), None) if !card_info.activity_users.is_empty() => app.nav.activities.get(&l).cloned().unwrap_or_default(),
            _ if card_info.activity_users.is_empty() => crate::t!("friends.now_in_voice"),
            _ => crate::t!("friends.now_multiple"),
        };
        let names: Vec<SharedString> =
            card_info.partied.iter().map(|id| app.users.get(id).map(|u| u.display_name.clone()).unwrap_or_default()).collect();
        let title: SharedString = card_title(&names).into();
        let lead_user = lead.and_then(|l| app.users.get(&l)).or(u);
        let mut card = div()
            .id(("active", user.0))
            .flex_none()
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
                                    .child(title),
                            )
                            .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(Palette::TEXT_MUTED)).text_ellipsis().child(sub)),
                    )
                    .when_some(game.as_ref().and_then(|g| g.image.clone()), |d, i| d.child(thumb(Some(i), 24.))),
            );
        if let Some(v) = card_info.voice {
            card = card.child(voice_section(app, v));
        }
        for owner in &card_info.streams {
            card = card.child(go_live_section(app, *owner, cx));
        }
        let stream = lead
            .and_then(|l| app.people.activities.get(&l))
            .and_then(|v| v.iter().find(|a| a.kind == crate::presence::ActivityKind::Streaming))
            .cloned();
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
                                    .child(crate::t!("voice.live")),
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
                    .child(crate::ui::avatar(lead_user, 24.)),
            );
        }
        col = col.child(card);
    }
    col
}

fn section_text(t: SharedString, strong: bool) -> gpui::Div {
    let d = div().text_ellipsis();
    if strong {
        d.text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(Palette::TEXT_NORMAL)).child(t)
    } else {
        d.text_size(z(12.)).line_height(z(16.)).text_color(c(Palette::TEXT_MUTED)).child(t)
    }
}

fn voice_section(app: &PhoenixApp, voice: Id) -> AnyElement {
    let ch = app.channels.get(&voice);
    let guild = ch.and_then(|c| c.guild).and_then(|g| app.guilds.iter().find(|x| x.id == g));
    let members: Vec<Id> = ch.map(|c| c.voice_users.clone()).unwrap_or_default();
    let icon_box = match guild.map(|g| &g.icon) {
        Some(crate::model::ImageRef::Url(url)) => gpui::img(url.clone()).size(z(32.)).rounded_full().into_any_element(),
        _ => {
            let acr = guild.map(|g| g.acronym.clone()).unwrap_or_default();
            let size = [14., 14., 12., 12., 10., 8., 6.].get(acr.chars().count()).copied().unwrap_or(6.);
            div()
                .size(z(32.))
                .rounded_full()
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .text_size(z(size))
                .text_color(gpui::white())
                .child(acr)
                .into_any_element()
        }
    };
    let assets = div().size(z(32.)).flex_none().relative().child(icon_box).child(
        div()
            .absolute()
            .bottom(z(-2.))
            .right(z(-2.))
            .size(z(16.))
            .rounded_full()
            .bg(c(Palette::BG_TERTIARY))
            .flex()
            .items_center()
            .justify_center()
            .child(gpui::svg().path(crate::icons::path("speaker")).size(z(10.)).text_color(c(tok(idx::TEXT_DEFAULT)))),
    );
    let (shown, plus) = facepile(members.len());
    let mut pile = div().flex().items_center().flex_none();
    for (i, m) in members.iter().take(shown).enumerate() {
        pile = pile.child(div().when(i > 0, |d| d.ml(z(-2.))).child(crate::ui::avatar(app.users.get(m), 24.)));
    }
    if let Some(n) = plus {
        pile = pile.child(
            div()
                .ml(z(-2.))
                .h(z(24.))
                .min_w(z(24.))
                .px(z(4.))
                .rounded(z(12.))
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                .flex()
                .items_center()
                .justify_center()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(format!("+{n}")),
        );
    }
    div()
        .p(z(16.))
        .flex()
        .items_center()
        .gap(z(12.))
        .child(assets)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(section_text(guild.map(|g| g.name.clone()).unwrap_or_default(), true))
                .child(section_text(ch.map(|c| c.name.clone()).unwrap_or_default(), false)),
        )
        .child(pile)
        .into_any_element()
}

fn go_live_section(app: &PhoenixApp, owner: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let u = app.users.get(&owner);
    let key = crate::ui::call_view::stream_key_in(app, owner);
    if let Some(k) = key.clone() {
        crate::ui::voice_row::fetch_preview(app, k, cx);
    }
    let url = key.as_deref().and_then(crate::ui::voice_row::preview_url);
    div()
        .p(z(16.))
        .flex()
        .flex_col()
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
                        .child(section_text(u.map(|u| u.display_name.clone()).unwrap_or_default(), true))
                        .child(section_text(crate::t!("friends.now_sharing_screen"), false)),
                )
                .child(
                    div()
                        .h(z(16.))
                        .px(z(8.))
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_FEEDBACK_NOTIFICATION)))
                        .flex()
                        .items_center()
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(gpui::white())
                        .child(crate::t!("voice.live")),
                ),
        )
        .child(
            div()
                .mt(z(12.))
                .w_full()
                .h(z(165.))
                .rounded(z(8.))
                .overflow_hidden()
                .bg(c(tok(idx::BACKGROUND_MOD_STRONG)))
                .when_some(url, |d, url| d.child(gpui::img(url).size_full().rounded(z(8.)).object_fit(gpui::ObjectFit::Cover))),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(user: u64, kind: RelationshipKind, since: i64) -> Relationship {
        Relationship { user: Id(user), kind, nickname: None, since: Some(since), ignored: false, spam: false, note: None }
    }

    fn nf(user: u64, name: &str, voice: Option<u64>, activity: bool) -> NowFriend {
        NowFriend { user: Id(user), name: name.into(), voice: voice.map(Id), activity }
    }

    #[test]
    fn friends_in_one_voice_channel_share_a_card_with_the_other_members() {
        let friends = [nf(1, "alchi", Some(50), false), nf(2, "fox", Some(50), false), nf(3, "solo", None, true)];
        let cards = now_cards(&friends, |v| if v == Id(50) { vec![Id(9), Id(2), Id(1)] } else { vec![] }, |u| u == Id(1));
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].voice, Some(Id(50)));
        assert_eq!(cards[0].priority, vec![Id(1), Id(2)]);
        assert_eq!(cards[0].partied, vec![Id(1), Id(2), Id(9)]);
        assert_eq!(cards[0].streams, vec![Id(1)]);
        assert_eq!(cards[1].priority, vec![Id(3)]);
    }

    #[test]
    fn idle_friends_without_voice_or_activity_get_no_card() {
        let cards = now_cards(&[nf(1, "a", None, false)], |_| vec![], |_| false);
        assert!(cards.is_empty());
    }

    #[test]
    fn cards_follow_the_client_order() {
        let friends = [
            nf(1, "zed", None, true),
            nf(2, "amy", None, true),
            nf(3, "bob", Some(70), false),
            nf(4, "cat", Some(80), false),
            nf(5, "dan", Some(80), false),
        ];
        let cards = now_cards(&friends, |_| vec![], |_| false);
        let first: Vec<u64> = cards.iter().map(|c| c.priority[0].0).collect();
        assert_eq!(first, vec![4, 3, 2, 1]);
    }

    #[test]
    fn facepile_shows_two_then_a_third_or_a_count() {
        assert_eq!(facepile(1), (1, None));
        assert_eq!(facepile(2), (2, None));
        assert_eq!(facepile(3), (3, None));
        assert_eq!(facepile(4), (2, Some(2)));
        assert_eq!(facepile(300), (2, Some(99)));
    }

    #[test]
    fn single_member_cards_use_the_plain_name() {
        assert_eq!(card_title(&["Alchi".into()]), "Alchi");
        let three = card_title(&["Fox".into(), "Alchi".into(), "X".into()]);
        assert!(three.starts_with("Fox, Alchi, ") && three.contains('1'));
    }

    #[test]
    fn pending_hides_spam_and_ignored_incoming() {
        let mut spam = rel(1, RelationshipKind::Incoming, 0);
        spam.spam = true;
        let mut ignored = rel(2, RelationshipKind::Incoming, 0);
        ignored.ignored = true;
        let plain = rel(3, RelationshipKind::Incoming, 0);
        let out = rel(4, RelationshipKind::Outgoing, 0);
        let shown: Vec<u64> =
            [&spam, &ignored, &plain, &out].iter().filter(|r| in_tab(r, FriendsTab::Pending, false)).map(|r| r.user.0).collect();
        assert_eq!(shown, vec![3, 4]);
        assert_eq!(pending_badge(&[spam, ignored, plain, out]), 1);
    }

    #[test]
    fn pending_incoming_newest_first_outgoing_by_name() {
        let mut v = vec![
            rel(1, RelationshipKind::Outgoing, 50),
            rel(2, RelationshipKind::Incoming, 10),
            rel(3, RelationshipKind::Incoming, 30),
            rel(4, RelationshipKind::Outgoing, 1),
        ];
        let names = |r: &Relationship| match r.user.0 {
            1 => "zed".to_owned(),
            4 => "amy".to_owned(),
            _ => "x".to_owned(),
        };
        order(&mut v, names, FriendsTab::Pending);
        assert_eq!(v.iter().map(|r| r.user.0).collect::<Vec<_>>(), vec![3, 2, 4, 1]);
    }

    #[test]
    fn first_section_follows_the_client() {
        assert_eq!(initial_tab(&[rel(1, RelationshipKind::Friend, 0)]), FriendsTab::Online);
        assert_eq!(initial_tab(&[rel(1, RelationshipKind::Incoming, 0)]), FriendsTab::Pending);
        assert_eq!(initial_tab(&[rel(1, RelationshipKind::Outgoing, 0)]), FriendsTab::AddFriend);
    }

    #[test]
    fn search_reads_username_nickname_and_global_name() {
        let mut r = rel(1, RelationshipKind::Friend, 0);
        r.nickname = Some("Bestie".into());
        assert!(matches_query(&r, "wumpus", Some("Wump"), "best"));
        assert!(matches_query(&r, "wumpus", None, "mpu"));
        assert!(!matches_query(&r, "wumpus", Some("Wump"), "zzz"));
    }
}

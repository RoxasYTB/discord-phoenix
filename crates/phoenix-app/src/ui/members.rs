//! 240px member list, virtualized.
//!
//! With a real session the rows come from the server-ordered lazy list
//! (GUILD_MEMBER_LIST_UPDATE, `People::lists`): hoisted role groups, then
//! Online / Offline, with placeholders for unsynced ranges. Scrolling asks
//! for the ranges in view (op 37). Without it (mock), `PhoenixApp::member_rows`
//! groups locally.

use crate::theme::z;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    list, prelude::*, px,
};

use crate::{
    app::{MemberRow, PhoenixApp},
    backend::AppEvent,
    model::{Id, Member, Presence},
    presence::{ListEntry, ranges_for},
    theme::{Palette, c, generated::idx, m, role_color, space, tok},
};

/// `AppEvent::MemberListRows` / `AppEvent::Activities`.
pub fn on_event(app: &mut PhoenixApp, ev: AppEvent, cx: &mut Context<PhoenixApp>) {
    match ev {
        AppEvent::MemberListRows { guild, rows } => {
            app.people.lists.insert(guild, rows);
            if app.selected_guild == Some(guild) {
                sync_server_list(app);
            }
        }
        AppEvent::Activities { user, mut activities } => {
            // Guild presences for ourselves often come without the custom status
            // (it lives in the settings proto): keep ours like the client does.
            use crate::presence::ActivityKind as K;
            if user == app.me.id
                && app.people.custom_status.is_some()
                && !activities.iter().any(|a| a.kind == K::Custom)
                && let Some(c) = app.people.activities.get(&user).and_then(|a| a.iter().find(|a| a.kind == K::Custom)).cloned()
            {
                activities.insert(0, c);
            }
            app.people.on_activities(user, activities)
        }
        _ => return,
    }
    cx.notify();
}

/// If the open guild has a server list, size the virtual list for it
/// (keeping the scroll position) and return true.
pub fn sync_server_list(app: &mut PhoenixApp) -> bool {
    // +1: item 0 is the Activity section, which scrolls with
    // the list like the client's (they sit inside `members_86es0b`).
    let Some(len) = app.selected_guild.and_then(|g| rows_for(app, g)).map(|l| l.len() + 1) else {
        return false;
    };
    app.member_rows.clear();
    let old = app.member_list.item_count();
    if old == len {
        app.member_list.remeasure();
    } else if old < len {
        app.member_list.splice(old..old, len - old);
    } else {
        app.member_list.splice(len..old, 0);
    }
    true
}

/// Ask for the ranges around the scroll position when it enters a new chunk.
fn request_ranges(app: &PhoenixApp, g: Id, cx: &mut Context<PhoenixApp>) {
    let (Some(ch), true) = (app.selected_channel, app.people.lists.contains_key(&g)) else { return };
    let top = app.member_list.logical_scroll_top().item_ix.saturating_sub(1);
    let key = (g, top / 100);
    if app.people.requested.get() == key {
        return;
    }
    app.people.requested.set(key);
    if key.1 == 0 {
        return; // [0, 99] is subscribed when the channel opens.
    }
    let fut = app.backend.subscribe_member_ranges(g, ch, ranges_for(top));
    cx.background_spawn(fut).detach();
}

/// The open channel when it is a thread.
fn open_thread(app: &PhoenixApp) -> Option<Id> {
    app.selected_channel.filter(|c| app.nav.thread_parent(*c).is_some())
}

static NO_ROWS: Vec<ListEntry> = Vec::new();

/// Rows the member list shows: the thread's own members in a thread (empty
/// until `thread-members` answers), else the guild's lazy list.
fn rows_for(app: &PhoenixApp, g: Id) -> Option<&Vec<ListEntry>> {
    match open_thread(app) {
        Some(t) => Some(app.people.thread_lists.get(&t).unwrap_or(&NO_ROWS)),
        None => app.people.lists.get(&g),
    }
}

/// Group thread members like the client's thread sidebar: highest hoisted
/// role (online only), then Online, then Offline; names sorted in each.
fn thread_rows(app: &PhoenixApp, g: Id, mut members: Vec<Member>) -> Vec<ListEntry> {
    let roles = app.roles.get(&g).cloned().unwrap_or_default();
    let mut hoisted: Vec<_> = roles.iter().filter(|r| r.hoist).collect();
    hoisted.sort_by_key(|r| std::cmp::Reverse(r.position));
    for m in &mut members {
        if m.presence == Presence::Offline {
            m.presence = app.presence_of(m.user);
        }
    }
    let name = |m: &Member| {
        m.nick.clone().or_else(|| app.users.get(&m.user).map(|u| u.display_name.clone())).unwrap_or_default().to_lowercase()
    };
    members.sort_by_key(name);
    let group_of = |m: &Member| -> String {
        if m.presence == Presence::Offline {
            return "offline".into();
        }
        hoisted.iter().find(|r| m.roles.contains(&r.id)).map(|r| r.id.0.to_string()).unwrap_or_else(|| "online".into())
    };
    let mut order: Vec<String> = hoisted.iter().map(|r| r.id.0.to_string()).collect();
    order.push("online".into());
    order.push("offline".into());
    let mut out = Vec::new();
    for gid in order {
        let ms: Vec<&Member> = members.iter().filter(|m| group_of(m) == gid).collect();
        if ms.is_empty() {
            continue;
        }
        out.push(ListEntry::Group { id: gid.into(), count: ms.len() as u32 });
        out.extend(ms.into_iter().map(|m| ListEntry::Member(m.clone(), m.nick.clone())));
    }
    out
}

/// Fetch the open thread's members once, and keep the virtual list sized
/// for whichever list (thread / guild) is showing.
fn ensure_thread_list(app: &PhoenixApp, g: Id, cx: &mut Context<PhoenixApp>) {
    let want = rows_for(app, g).map(|l| l.len() + 1);
    if want.is_some_and(|n| n != app.member_list.item_count()) {
        let e = cx.entity();
        cx.defer(move |cx| {
            e.update(cx, |a, cx| {
                sync_server_list(a);
                cx.notify();
            })
        });
    }
    let Some(t) = open_thread(app) else { return };
    if !app.people.thread_requested.borrow_mut().insert(t) {
        return;
    }
    let fut = app.backend.thread_members(t);
    cx.spawn(async move |this, cx| {
        let members = match fut.await {
            Ok(m) => m,
            Err(e) => {
                if std::env::var_os("PHX_DEBUG").is_some() {
                    eprintln!("thread members: {}", e.0);
                }
                return;
            }
        };
        let _ = this.update(cx, |a, cx| {
            let rows = thread_rows(a, g, members);
            a.people.thread_lists.insert(t, rows);
            sync_server_list(a);
            cx.notify();
        });
    })
    .detach();
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> impl IntoElement {
    let entity = cx.entity();
    if let Some(g) = app.selected_guild {
        request_ranges(app, g, cx);
        ensure_thread_list(app, g, cx);
    }
    div()
        .w(z(m::member_list_w())) // --custom-member-list-width
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .bg(c(Palette::BG_SECONDARY)) // container_86es0b --custom-channel-members-bg = --background-base-lower
        .border_l_1()
        .border_color(c(Palette::DIVIDER)) // container_86es0b border-inline-start --border-subtle
        .child(
            list(app.member_list.clone(), move |ix, _w, cx| row(&entity, ix, cx))
                .flex_1()
                .w_full()
                // live client: content_tqapBM is 260px of the 268px list, the thin
                // scroller keeps an 8px gutter on the right.
                .pr(z(8.))
                .pb(z(10.)), // members_86es0b padding-bottom 10px
        )
}

/// "2d ago" style elapsed time since `start_ms`.
pub fn elapsed(start_ms: i64, now_s: i64) -> String {
    let s = (now_s - start_ms / 1000).max(0);
    match s {
        0..60 => crate::t!("time.now").to_string(),
        60..3600 => crate::t!("time.minutes_ago_short", n = s / 60).to_string(),
        3600..86_400 => crate::t!("time.hours_ago_short", n = s / 3600).to_string(),
        _ => crate::t!("time.days_ago_short", n = s / 86_400).to_string(),
    }
}

/// Member list Activity rows (client member list groups hook): GLOBAL_FEED
/// entries of the rendered content types whose author is a member of `g`
/// (unknown authors are requested with op 8, like `requestMember`) and can
/// VIEW_CHANNEL the open channel, in feed order.
/// The header counts the cards shown. Live
/// client 2026-11-24, guild with 6 feed entries of which 5 by non-members:
/// "Activité — 1" with one card; authors that are not members never resolve
/// through op 8, so counting them as pending kept "6" forever.
fn feed_cards(app: &PhoenixApp, g: Id) -> Vec<crate::commands::InventoryEntry> {
    let is_member = |u: Id| {
        app.members.get(&g).is_some_and(|ms| ms.iter().any(|m| m.user == u))
            || rows_for(app, g).is_some_and(|rows| rows.iter().any(|r| matches!(r, ListEntry::Member(m, _) if m.user == u)))
    };
    let entries = crate::ui::activity_traits::entries();
    let mut unknown: Vec<Id> = Vec::new();
    let mut out = Vec::new();
    for e in entries {
        // Rendered content types (client member list row switch): PLAYED_GAME (1)
        // and LAUNCHED_ACTIVITY (8, `extra.activity_name`), both name + 48px art.
        if !matches!(e.content_type, 1 | 8) || e.game.is_none() {
            continue;
        }
        if !is_member(e.author) {
            if !unknown.contains(&e.author) && app.people.feed_requested.borrow_mut().insert((g, e.author)) {
                unknown.push(e.author);
            }
            continue;
        }
        if app.selected_channel.is_some_and(|ch| !app.backend.member_can_view(ch, e.author)) {
            continue;
        }
        out.push(e);
    }
    if !unknown.is_empty() {
        app.backend.request_feed_members(g, unknown);
    }
    out
}

/// Activity section expanded past its first 3 cards (toggleExpandIcon).
static ACTIVITY_EXPANDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The "Activity" section at the top of the member list: one card per
/// member playing a game. Dump `-channels-N-N+` nodes 1502 (header) and
/// 1512 (card `container_g7esM5 usesCardRows_g7esM5`).
fn activity_section(app: &PhoenixApp, entity: &Entity<PhoenixApp>, g: Id) -> AnyElement {
    let acts = feed_cards(app, g);
    let count = acts.len();
    let expanded = ACTIVITY_EXPANDED.load(std::sync::atomic::Ordering::Relaxed);
    // Member list content feed gate (client bundle, member list groups hook):
    // shown when the guild allows it (`activity_feed_enabled`) AND the open
    // channel is not a forum (`isForumChannel()` = GUILD_FORUM only; media
    // channels keep it).
    let forum = app
        .selected_channel
        .and_then(|id| app.channels.get(&id))
        .is_some_and(|ch| ch.kind == crate::model::ChannelKind::Forum && !app.backend.is_media_channel(ch.id));
    if acts.is_empty() || forum || !app.backend.activity_feed_enabled(g) {
        return div().into_any_element();
    }
    let now = crate::mock::now();
    // headerContainer_HxSvbL: flex, margin -3px 0; label, 24px settingsButton
    // (padding 4, 12px gear), spacer, 24x18 toggleExpandIcon.
    let muted = c(Palette::TEXT_MUTED);
    let mut col = div().flex_none().flex().flex_col().child(
        div()
            .pt(z(space::lg())) // membersGroup_86es0b padding-block --space-lg --space-xxs, inline --space-md --space-xxs
            .pb(z(space::xxs()))
            .pl(z(space::md()))
            .pr(z(space::xxs()))
            .child(
                div()
                    .h(z(24.))
                    .my(z(-3.))
                    .flex()
                    .items_center()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child(crate::t!("members.activity", n = count))
                    .child(
                        div()
                            .size(z(24.))
                            .p(z(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(gpui::svg().path(crate::icons::path("gear")).size(z(12.)).text_color(muted)),
                    )
                    .child(div().flex_1())
                    // toggleExpandIcon only when expandedCount > 3 (or hidden).
                    .when(acts.len() > 3, |d| {
                        // collapsed (3 cards): ChevronSmallRight; expanded: ChevronSmallDown.
                        let ent = entity.clone();
                        d.child(
                            div()
                                .id("activity-expand")
                                .cursor_pointer()
                                .on_click(move |_, _, cx| {
                                    ACTIVITY_EXPANDED.fetch_xor(true, std::sync::atomic::Ordering::Relaxed);
                                    ent.update(cx, |a, cx| {
                                        a.member_list.remeasure();
                                        cx.notify();
                                    });
                                })
                                // toggleExpandIcon_ 24x18; the 24-unit glyph keeps its aspect
                                // (SVG "meet": 18x18 centred), GPUI svg would stretch it.
                                .w(z(24.))
                                .h(z(18.))
                                .flex()
                                .justify_center()
                                .child(
                                    gpui::svg()
                                        .path(crate::icons::path(if expanded { "chevron" } else { "chevron-right" }))
                                        .size(z(18.))
                                        .text_color(c(Palette::INTERACTIVE)),
                                ),
                        )
                    }),
            ),
    );
    for e in acts.into_iter().take(if expanded { usize::MAX } else { 3 }) {
        let uid = e.author;
        let user_cow = app.member_user(Some(g), uid);
        let user = user_cow.as_deref();
        let mem = app.members.get(&g).and_then(|ms| crate::member_index::find(ms, g, uid));
        let name = mem
            .and_then(|m| m.nick.clone())
            .or_else(|| user.map(|u| u.display_name.clone()))
            .unwrap_or_default();
        let ent = entity.clone();
        let game: SharedString = e.game.clone().unwrap_or_default().into();
        // thumbnailButton_ img: app-icons/{app}/{icon}.png?size=240
        let art = match e.icon.clone() {
            Some(url) => gpui::img(url).size_full().into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(z(16.))
                .font_weight(FontWeight::BOLD)
                .text_color(muted)
                .child(SharedString::from(game.chars().next().map(String::from).unwrap_or_default()))
                .into_any_element(),
        };
        let badge_in = |ic: &'static str, t: SharedString, col: gpui::Hsla| {
            // badgeContainer_CTWncy: gap 4, 12px icon, body-xs/normal 12/16
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .child(gpui::svg().path(crate::icons::path(ic)).size(z(12.)).flex_none().text_color(col))
                .child(div().text_size(z(12.)).line_height(z(16.)).text_color(col).text_ellipsis().child(t))
        };
        let badge = |ic: &'static str, t: SharedString| badge_in(ic, t, c(Palette::TEXT_MUTED));
        // Time badge (client content badge `$`): while the session is live, a running
        // timer in --text-feedback-positive; otherwise "x ago" in the muted colors.
        // Live client 2026-11-24: an entry with trait 3 `is_live` showed "12:08" =
        // now - `started_at` exactly, while the author's presence had no matching start.
        // A matching presence activity start is the fallback.
        let entry_live = e.live && e.expires_ms.is_none_or(|x| x > now * 1000);
        let live_start = e.started_ms.filter(|_| entry_live).or_else(|| {
            app.people.activities.get(&uid).and_then(|acts| {
                acts.iter()
                    .find(|a| e.game.as_deref().is_some_and(|g| a.name.eq_ignore_ascii_case(g)))
                    .and_then(|a| a.start)
            })
        });
        let time_icon = if e.content_type == 8 { "apps" } else { "game-controller" };
        let time_badge = match live_start {
            Some(start) => badge_in(time_icon, live_timer(start, now), c(tok(idx::TEXT_FEEDBACK_POSITIVE))),
            None => badge(time_icon, crate::ui::settings::profile_activity::ago(crate::presence::snowflake_time(e.id.0), now)),
        };
        col = col.child(
            div()
                .id(("activity-card", e.id.0))
                .mt(z(8.)) // container_g7esM5 margin 8px 0 0 8px (no right margin)
                .ml(z(8.))
                // live container_g7esM5 box 252x69.33 (vanilla-members #1512: 52px cardRow
                // + 2x8 padding + 2x0.667 border); 70 drifted every following row by 2/3px per card.
                .w(z(m::member_list_w() - 16.)) // cardPlacement_ max-width: list width - 2 × 8px; fixed so long names cannot widen it
                .h(z(52. + 16. + 2. * 0.667))
                .overflow_hidden()
                .p(z(8.)) // padding 8px
                .rounded(z(12.)) // border-radius 12px
                .border_1()
                .border_color(c(Palette::DIVIDER)) // --border-subtle (#97979f @16%)
                .bg(c(Palette::BG_TERTIARY)) // --background-base-lowest (#0a0a0c midnight)
                .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
                .cursor_pointer()
                .flex()
                .gap(z(8.)) // cardRow_g7esM5 gap 8px
                .on_click(move |ev, window, cx| {
                    let at = ev.position();
                    ent.update(cx, |a, cx| a.open_profile(uid, Some(g), at, window, cx));
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .mb(z(-2.)) // infoSection_g7esM5 margin-bottom -2px
                        .child(
                            // userSection_g7esM5: 18px, margin-bottom 2px; 16px facePile, margin-right 6
                            div()
                                .h(z(18.))
                                .mb(z(2.))
                                .min_w_0()
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .child(div().mr(z(6.)).child(crate::ui::avatar(user, 16.)))
                                .child(
                                    // userName_g7esM5: 14px / 1.2857, 500, ellipsis, max-width 85%.
                                    // Client bundle (content inventory card user section): the
                                    // name gets the member's role colorString (else the class
                                    // color --channels-default) and only the display name
                                    // style's FONT (displayNameStylesFont), never its colors.
                                    // Gradient role colours (colorStrings) paint the name too.
                                    crate::fonts::styled_name(
                                        div()
                                            .min_w_0()
                                            .max_w(gpui::relative(0.85))
                                            .overflow_hidden()
                                            .text_size(z(14.))
                                            .line_height(z(18.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(
                                                mem.and_then(|m| app.member_color(g, m))
                                                    .map(c)
                                                    .unwrap_or_else(|| c(crate::theme::tok(crate::theme::generated::idx::CHANNELS_DEFAULT))),
                                            )
                                            .text_ellipsis(),
                                        user,
                                        false,
                                    )
                                    .child(crate::fonts::name_text_with(
                                        name,
                                        user,
                                        14.,
                                        mem.and_then(|m| crate::ui::role_gradient::for_member(app, g, m)),
                                    )),
                                ),
                        )
                        .child(
                            // heading-xs/medium lineClamp1: 14/18, 500, --text-default #d4d5d8
                            div()
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c(Palette::TEXT_NORMAL))
                                .text_ellipsis()
                                .child(game.clone()),
                        )
                        .child(
                            // badgesContainer_CTWncy: gap 0 8px
                            div()
                                .h(z(16.))
                                .flex()
                                .gap(z(8.))
                                .overflow_hidden()
                                // badgeContainer_: game-controller + "Il y a 2j" (entry id time), then
                                // the traits: FIRST_TIME "New player", RESURRECTED, TRENDING, streak >= 3.
                                // Time badge icon (client): AppsIcon for a launched activity, else GameControllerIcon.
                                .child(time_badge)
                                .when(e.first_time, |d| d.child(badge("medal", crate::t!("members.new_player"))))
                                .when(e.resurrected, |d| d.child(badge("refresh", crate::t!("members.resurrection"))))
                                .when(e.trending, |d| d.child(badge("fire", crate::t!("members.trending"))))
                                .when_some(e.streak_days.filter(|n| *n >= 3), |d, n| d.child(badge("streak", crate::t!("pa.streak", n = n)))),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .size(z(48.)) // thumbnailButton_NJVMoP 48x48, radius 8; img radius 4
                        .rounded(z(4.))
                        .overflow_hidden()
                        // placeholder tile only when the app has no icon
                        .when(e.icon.is_none(), |d| d.bg(c(Palette::BG_MOD_SELECTED)))
                        .child(art),
                ),
        );
    }
    col.into_any_element()
}

/// Running activity timer like the client's content timer: "m:ss", or "h:mm:ss"
/// from one hour. `start_ms` unix ms, `now_s` unix seconds.
fn live_timer(start_ms: i64, now_s: i64) -> SharedString {
    let s = (now_s - start_ms / 1000).max(0);
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") }.into()
}

/// Server tag chiplet: chipletContainerInner_k8jyN7 (--background-mod-strong,
/// 4px radius, 0 4px padding, 16px line height).
/// Badge: vanilla-members #1696 `badge_k8jyN7` 14×14, margin-right 2.
pub fn server_tag(tag: &(SharedString, Option<SharedString>)) -> gpui::Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .px(z(4.))
        .rounded(z(4.))
        .bg(c(tok(idx::BACKGROUND_MOD_STRONG))) // --background-mod-strong
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(Palette::TEXT_NORMAL))
        .when_some(tag.1.clone(), |d, url| d.child(gpui::img(url).size(z(14.)).mr(z(2.)).flex_none()))
        .child(tag.0.clone())
}

pub fn header(t: SharedString) -> AnyElement {
    // membersGroup_86es0b: padding-block --space-lg --space-xxs (20/4),
    // padding-inline --space-md --space-xxs (16/4), height auto. Not
    // uppercased in the 2026 client; 14px/500 (Text sm/medium).
    // Dump node 1598: padding 24px 4px 4px 20px, 46px tall.
    div()
        .pt(z(space::lg()))
        .pb(z(space::xxs()))
        .pl(z(space::md()))
        .pr(z(space::xxs()))
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(Palette::TEXT_MUTED))
        .text_ellipsis()
        .child(t)
        .into_any_element()
}

/// membersGroup_86es0b > membersGroupHeader_86es0b (flex): optional
/// roleIcon_86es0b 16px (margin-right 4, vertical-align top), the
/// membersGroupName_ (ellipsis), then " — N" which never truncates.
fn group_header(app: &PhoenixApp, g: Id, id: &str, count: u32) -> AnyElement {
    let name = group_name(app, g, id);
    let icon = id
        .parse::<u64>()
        .ok()
        .and_then(|r| app.roles.get(&g)?.iter().find(|x| x.id == Id(r))?.icon.clone());
    div()
        .pt(z(space::lg()))
        .pb(z(space::xxs()))
        .pl(z(space::md()))
        .pr(z(space::xxs()))
        .child(
            div()
                .h(z(18.))
                .flex()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(Palette::TEXT_MUTED))
                .when_some(icon, |d, url| d.child(gpui::img(url).flex_none().size(z(16.)).mr(z(4.))))
                .child(div().min_w_0().text_ellipsis().child(crate::ui::emoji_text::label(name.trim().to_string().into(), 12.))) // HTML collapses edge spaces
                .child(div().flex_none().child(SharedString::from(format!(" — {count}")))),
        )
        .into_any_element()
}

fn group_name(app: &PhoenixApp, g: Id, id: &str) -> String {
    match id {
        "online" => crate::t!("status.online").to_string(),
        "offline" => crate::t!("status.offline").to_string(),
        _ => id
            .parse::<u64>()
            .ok()
            .and_then(|r| app.roles.get(&g)?.iter().find(|x| x.id == Id(r)).map(|x| x.name.to_string()))
            .unwrap_or_else(|| crate::t!("members.members").to_string()),
    }
}

fn placeholder(ix: usize) -> AnyElement {
    // Deterministic widths so the skeleton doesn't flicker.
    let w = 60. + ((ix * 37) % 80) as f32;
    div()
        .ml(z(16.))
        .h(px(m::MEMBER_ROW_H))
        .flex()
        .items_center()
        .gap(z(12.))
        .child(div().size(z(32.)).rounded_full().bg(c(Palette::BG_MOD_HOVER)))
        .child(div().w(px(w)).h(z(12.)).rounded(z(6.)).bg(c(Palette::BG_MOD_HOVER)))
        .into_any_element()
}

fn row(entity: &Entity<PhoenixApp>, ix: usize, cx: &mut App) -> AnyElement {
    let app = entity.read(cx);
    let Some(g) = app.selected_guild else { return div().into_any_element() };
    // Item 0: the Activity section, scrolled with the members. (The online /
    // total / voice counts row was Vencord's MemberCount plugin, not vanilla.)
    let Some(ix) = ix.checked_sub(1) else {
        let entity = entity.clone();
        let el = div().flex().flex_col().child(activity_section(app, &entity, g)).into_any_element();
        if crate::ui::activity_traits::stale() {
            cx.defer(move |cx| entity.update(cx, crate::ui::activity_traits::ensure));
        }
        return el;
    };
    if let Some(rows) = rows_for(app, g) {
        return match rows.get(ix) {
            Some(ListEntry::Group { id, count }) => {
                // A group row synced without `count`: fall back to the rows below it.
                let n = if *count > 0 {
                    *count
                } else {
                    rows[ix + 1..].iter().take_while(|r| !matches!(r, ListEntry::Group { .. })).count() as u32
                };
                group_header(app, g, id, n)
            }
            Some(ListEntry::Member(mem, nick)) => member_row(app, entity, g, mem, nick.as_ref()),
            Some(ListEntry::Loading) => placeholder(ix),
            None => div().into_any_element(),
        };
    }
    match app.member_rows.get(ix) {
        Some(MemberRow::Header(t)) => header(t.clone()),
        Some(MemberRow::Member(i)) => match app.members.get(&g).and_then(|ms| ms.get(*i)) {
            Some(mem) => member_row(app, entity, g, mem, None),
            None => div().into_any_element(),
        },
        None => div().into_any_element(),
    }
}

fn member_row(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    g: Id,
    mem: &Member,
    nick: Option<&SharedString>,
) -> AnyElement {
    // The crown only shows outside hoisted role sections: live, the owner listed under a
    // hoisted role has no ownerIcon_, while dumps show it in the Online / Offline groups.
    let hoisted = mem.presence != Presence::Offline
        && app.roles.get(&g).is_some_and(|rs| rs.iter().any(|r| r.hoist && mem.roles.contains(&r.id)));
    let owner = !hoisted && app.guilds.iter().find(|x| x.id == g).and_then(|x| x.owner) == Some(mem.user);
    member_row_in(app, entity, Some(g), mem, nick, owner)
}

/// A member row; `g: None` is a group DM's member list (no role colour,
/// voice looked up across every guild).
pub fn member_row_in(
    app: &PhoenixApp,
    entity: &Entity<PhoenixApp>,
    g: Option<Id>,
    mem: &Member,
    nick: Option<&SharedString>,
    owner: bool,
) -> AnyElement {
    let user_cow = app.member_user(g, mem.user);
    let user = user_cow.as_deref();
    let offline = mem.presence == Presence::Offline;
    let name = nick.cloned().or_else(|| user.map(|u| u.display_name.clone())).unwrap_or_default();
    let uid = mem.user;
    let e = entity.clone();
    let tag = user.and_then(|u| u.tag.clone());
    let acts = app.people.activities.get(&uid).map(Vec::as_slice).unwrap_or(&[]);
    let sub = SubText::of(acts);
    // A Streaming activity wins over the presence ("En direct", #9147ff
    // svg-mask-status-streaming) and over the voice line (dump: the streaming
    // bot shows only its stream text, no speaker).
    let streaming = !offline && acts.iter().any(|a| a.kind == crate::presence::ActivityKind::Streaming);
    let in_voice = !streaming && match g {
        Some(g) => app
            .guild_channels
            .get(&g)
            .is_some_and(|ids| ids.iter().filter_map(|i| app.channels.get(i)).any(|ch| ch.voice_users.contains(&uid))),
        None => app.channels.values().any(|ch| ch.guild.is_some() && ch.voice_users.contains(&uid)),
    };
    // Second line only when the client's activity status renders something; the
    // list item's fallback text is used only before the activities arrive.
    let two_lines = !offline && (in_voice || sub.is_some() || (acts.is_empty() && mem.activity.is_some()));
    // Dump `-channels-N-N+` node 1643: member_Iy0c7A margin-left 8px,
    // padding 1px 0, radius 8 (44px); childContainer_J1Adu_ padding 0 8px 0 10px;
    // memberInner_Iy0c7A padding 4px 0 (42px); avatar 32 + margin-right 12.
    div().py(z(1.)).child(div()
        .id(("member", mem.user.0))
        // Decorations play while the row is hovered (ui::avatar_deco).
        .on_hover({
            let e = e.clone();
            move |h, _, cx| {
                if crate::ui::avatar_deco::set_hover(crate::ui::avatar_deco::hover_key(1, uid.0), *h) {
                    e.update(cx, |_, cx| cx.notify());
                }
            }
        })
        .group("member-row")
        .ml(z(8.))
        // live client: member_Iy0c7A is 44px with a sub line (name 18 + sub 16),
        // 42px without (avatar 32), both including its 1px block padding.
        .h(px(if two_lines { m::MEMBER_ROW_H } else { m::MEMBER_ROW_H - 2. }))
        .pl(z(space::xs())) // childContainer_J1Adu_ padding-inline-start --space-xs
        // childContainer_J1Adu_ padding-right 8px, 32px with `nameplated_J1Adu_`
        // (vanilla-members row at y 718): names ellipsize before the nameplate art.
        .pr(z(if user.is_some_and(|u| u.nameplate.is_some()) { 32. } else { 8. }))
        .py(z(4.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(12.))
        .cursor_pointer()
        .when(offline, |d| d.opacity(0.3))
        // clickable_J1Adu_:hover .childContainer_J1Adu_ / .avatar opacity 1
        .hover(|s| s.bg(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_BACKGROUND_HOVER))).opacity(1.))
        // clickable_J1Adu_:active .childContainer_J1Adu_
        .active(|s| s.bg(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_BACKGROUND_ACTIVE))))
        // `nameplated_J1Adu_`: nameplate layer behind the row content.
        .relative()
        // Tab focus: the client's ring sits 4px inside the 252x44 row (live, 244x36).
        .map(|d| crate::ui::focus_ring::ring_inset(d, crate::ui::states::key(("member", uid.0)), 8., 4., 3.))
        .when_some(
            user.and_then(|u| {
                let sel = app.people.popout.as_ref().is_some_and(|p| p.user == uid);
                crate::ui::nameplate::layer(u, m::MEMBER_ROW_H - 2., crate::ui::nameplate::Place::Member, false, sel)
            }),
            |d, l| d.child(l),
        )
        .when(app.people.popout.as_ref().is_some_and(|p| p.user == uid), |d| {
            // .selected_J1Adu_ .childContainer_J1Adu_ --interactive-background-selected
            d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))).opacity(1.)
        })
        .on_click(move |ev, window, cx| {
            let at = ev.position();
            e.update(cx, |a, cx| a.open_profile(uid, g, at, window, cx));
        })
        .on_mouse_down(gpui::MouseButton::Right, move |ev, window, cx| {
            cx.stop_propagation();
            crate::ui::user_menu::open(uid, g, ev.position, window);
        })
        .child(
            div()
                .relative()
                .flex_none()
                .child({
                    // the status cut shows the row's hover / selected background like the mask
                    let sel = app.people.popout.as_ref().is_some_and(|p| p.user == uid);
                    let cut = move |d: gpui::Div| {
                        d.group_hover("member-row", |s| s.bg(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_BACKGROUND_HOVER))))
                            .when(sel, |d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                    };
                    // The decoration sits under the status cut-out and glyph (ui::avatar_with_status_deco).
                    let deco = crate::ui::avatar_deco::source(user, Some(crate::ui::avatar_deco::hover_key(1, uid.0)));
                    if mem.presence == crate::model::Presence::Online && crate::ui::mobile_status::is_mobile(uid.0) {
                        mobile_avatar(user, cut, deco)
                    } else {
                        crate::ui::avatar_with_status_deco(user, 32., mem.presence, c(Palette::BG_SECONDARY), cut, deco)
                    }
                })
                .when(streaming, |d| d.child(streaming_dot())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(
                            crate::fonts::styled_name(div(), user, false)
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::MEDIUM)
                                // container_J1Adu_ color --channels-default; :hover
                                // --interactive-text-hover, selected --interactive-text-active
                                // (role colours keep their own colour).
                                .map(|d| match g.and_then(|g| app.member_color(g, mem)) {
                                    Some(col) => d.text_color(role_color(Some(col))),
                                    None if app.people.popout.as_ref().is_some_and(|p| p.user == uid) => {
                                        d.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))
                                    }
                                    None => d
                                        .text_color(c(tok(idx::CHANNELS_DEFAULT)))
                                        .group_hover("member-row", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))),
                                })
                                // name_ ellipsis: only the name shrinks, the decorators keep their width
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                // name_Wo4EpK: style font letter-spacing, gradient role colours
                                .child(crate::fonts::name_text_with(
                                    name,
                                    user,
                                    16.,
                                    g.and_then(|g| crate::ui::role_gradient::for_member(app, g, mem)),
                                )),
                        )
                        .when_some(tag, |d, t| d.child(server_tag(&t).my(z(1.)))) // chipletContainerInner_k8jyN7 margin 1px 0 1px 4px
                        .when(user.is_some_and(|u| u.bot), |d| d.child(bot_tag_px(user.is_some_and(|u| u.verified_bot)).ml(z(4.))))
                        .when(owner, |d| {
                            // ownerIcon_Iy0c7A: 14px (icon_Iy0c7A), --text-feedback-warning
                            d.child(gpui::svg().path(crate::icons::path("crown")).size(z(14.)).flex_none().text_color(c(tok(idx::TEXT_FEEDBACK_WARNING))))
                        })
                        .when(mem.premium_since.is_some(), |d| {
                            // premiumIcon_Iy0c7A + icon_Iy0c7A: 14px, margin-inline-start 3px, top 1px;
                            // path fill --guild-boosting-pink (dump vanilla-guild-chat-busy node 1757).
                            d.child(
                                gpui::svg()
                                    .path(crate::icons::path("chat/boost"))
                                    .size(z(14.))
                                    .flex_none()
                                    .ml(z(3.)) // after the row gap(4), as the client (wrapper span + 3px)
                                    .relative()
                                    .top(z(1.))
                                    .text_color(c(tok(idx::GUILD_BOOSTING_PINK))),
                            )
                        }),
                )
                .when(two_lines, |d| d.child(sub_text(in_voice, user.is_some_and(|u| u.bot), sub, mem.activity.clone().filter(|_| acts.is_empty())))),
        ))
        .into_any_element()
}

/// Streaming status over the 32px avatar: rect 10x10 at 22,22 fill
/// --platform-twitch (#9147ff), masked by svg-mask-status-streaming (a play
/// triangle cut out), inside the 16px ring of the list background.
fn streaming_dot() -> gpui::Div {
    let ring = c(Palette::BG_SECONDARY);
    div()
        .absolute()
        .right(px(-4.))
        .bottom(px(-4.))
        .size(px(16.))
        .rounded_full()
        .bg(ring)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(10.))
                .rounded_full()
                .bg(c(tok(idx::PLATFORM_TWITCH)))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path("play")).size(px(6.)).text_color(ring)),
        )
}

/// APP tag, px variant (vanilla botTagRegular_ px_, measured live in the member
/// list): 16px tall, padding 0 4px, radius 4, --background-brand; botTagVerified_
/// check 16px margin-left -2; botText_ 12/16 600 uppercase. The member list adds
/// botTag_Iy0c7A's margin-inline-start 4; the profile's botTag_C2XpHA adds none.
pub fn bot_tag_px(verified: bool) -> gpui::Div {
    div()
        .flex_none()
        .h(z(16.))
        .px(z(4.))
        .rounded(z(4.))
        .bg(c(tok(idx::BACKGROUND_BRAND)))
        .flex()
        .items_center()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(gpui::white())
        .when(verified, |d| d.child(verified_check().ml(z(-2.))))
        .child(crate::t!("common.app").to_uppercase())
}

/// botTagVerified_: 16px CheckmarkSmallBoldIcon in --white; hovering it shows
/// the "Verified App" tooltip (aria-describedby on the live svg).
pub fn verified_check() -> gpui::Div {
    use crate::ui::tooltip::Tipped;
    div()
        .relative()
        .flex_none()
        .size(z(16.))
        .child(gpui::svg().path(crate::icons::path("check-small-bold")).size_full().text_color(gpui::white()))
        .tip(crate::t!("common.verified_app"))
}


/// 32px avatar "online on mobile" (live client masks):
/// `svg-mask-avatar-status-mobile-32` cuts a rect (.59375, .4375) .5x.65625,
/// radius .13125, and the phone glyph is a 10x15 rect at 22,17 in --status-online.
fn mobile_avatar(user: Option<&crate::model::User>, cut: impl FnOnce(gpui::Div) -> gpui::Div, deco: Option<SharedString>) -> gpui::Div {
    div()
        .relative()
        .flex_none()
        .size(px(32.))
        .child(crate::ui::avatar(user, 32.))
        // svg-mask-avatar-decoration-status-mobile-32: same hole as the avatar's (x 19, y 14, 16x21 r 4.2).
        .when_some(deco, |d, src| d.child(crate::ui::avatar_deco::layer(src, 32.)))
        .child(
            div()
                .absolute()
                .left(px(32. * 0.59375))
                .top(px(32. * 0.4375))
                .w(px(32. * 0.5))
                .h(px(32. * 0.65625))
                .rounded(px(32. * 0.13125))
                .bg(c(Palette::BG_SECONDARY))
                .child(cut(div().size_full().rounded(px(32. * 0.13125)))),
        )
        .child(
            gpui::svg()
                .path(crate::ui::status_svg::MOBILE)
                .absolute()
                .left(px(22.))
                .top(px(17.))
                .w(px(10.))
                .h(px(15.))
                .text_color(crate::theme::presence_color(crate::model::Presence::Online)),
        )
}

/// What the member row's second line shows (client activity status component,
/// module 9fb052bb `ActivityStatus`).
struct SubText {
    /// Custom status (emoji, text): customStatusContent_HigqCD. Kept when it has
    /// a trimmed non-empty state or an emoji.
    custom: Option<(Option<SharedString>, Option<SharedString>)>,
    /// First non-custom activity: (text, icon).
    main: Option<(SharedString, Option<&'static str>)>,
    /// Non-custom activities, unique by name.
    count: usize,
}

/// Activity text (client activity text helper): the name; a stream's details
/// first; `status_display_type` 0/1/2 picks name/state/details; Spotify's
/// artists ("a; b" -> "a, b").
fn activity_text(a: &crate::presence::Activity) -> Option<SharedString> {
    use crate::presence::ActivityKind as K;
    let nz = |v: &Option<SharedString>| v.clone().filter(|t| !t.is_empty());
    let name = Some(a.name.clone()).filter(|t| !t.is_empty());
    let (details, state) = (nz(&a.details), nz(&a.state));
    let mut m = if a.kind == K::Streaming { details.clone().or(name.clone()) } else { name.clone() };
    match a.display_type {
        Some(0) if name.is_some() => m = name.clone(),
        Some(1) if state.is_some() => m = state.clone(),
        Some(2) if details.is_some() => m = details.clone(),
        _ => {}
    }
    if a.is_spotify()
        && let Some(st) = state
    {
        return Some(st.replace("; ", ", ").into());
    }
    m
}

/// Activity icon (client activity icon helper): gamepad for playing/competing,
/// music for listening, TV for watching/streaming.
fn activity_icon(a: &crate::presence::Activity) -> Option<&'static str> {
    use crate::presence::ActivityKind as K;
    match a.kind {
        K::Playing | K::Competing => Some("game-controller"),
        K::Listening => Some("music"),
        K::Watching | K::Streaming => Some("tv"),
        K::Custom => None,
    }
}

impl SubText {
    fn of(acts: &[crate::presence::Activity]) -> Option<Self> {
        use crate::presence::ActivityKind as K;
        let custom = acts
            .iter()
            .find(|a| a.kind == K::Custom)
            .map(|a| (a.emoji.clone(), a.state.as_ref().map(|t| SharedString::from(t.trim().to_string())).filter(|t| !t.is_empty())))
            .filter(|(e, t)| e.is_some() || t.is_some());
        let mut names: Vec<&str> = Vec::new();
        for a in acts.iter().filter(|a| a.kind != K::Custom) {
            if !names.contains(&a.name.as_ref()) {
                names.push(a.name.as_ref());
            }
        }
        let main = acts.iter().find(|a| a.kind != K::Custom).map(|a| (activity_text(a).unwrap_or_default(), activity_icon(a)));
        let count = names.len();
        (custom.is_some() || main.is_some()).then_some(Self { custom, main, count })
    }
}

/// subText_J1Adu_ (margin-top -2px) > container_HigqCD textXs_HigqCD (flex, gap 4),
/// in the client's order:
/// 1. the activity (12px icon_HigqCD in --status-online, then its text), else the
///    voice line ("In a voice channel" with the speaker). With a custom status
///    TEXT only the icon stays (`iconOnly_HigqCD`). Bots get no icon.
/// 2. "+N" (activityCounter_HigqCD) when there are several (activities + voice),
///    not for bots; --status-positive when the custom status has text.
/// 3. the "•" dot (dot_HigqCD body-xs/normal) when there is a custom status too;
/// 4. the custom status (emoji 14px + text).
fn sub_text(in_voice: bool, bot: bool, sub: Option<SubText>, fallback: Option<SharedString>) -> gpui::Div {
    let green = c(tok(idx::STATUS_ONLINE));
    let (custom, main, count) = sub.map(|s| (s.custom, s.main, s.count)).unwrap_or((None, None, 0));
    let total = count + usize::from(in_voice);
    let custom_text = custom.as_ref().is_some_and(|c| c.1.is_some());
    let text_el = |t: SharedString| div().min_w_0().font_weight(FontWeight::MEDIUM).text_ellipsis().child(crate::ui::emoji_text::label(t, 12.));
    let mut row = div()
        .mt(z(-2.))
        .h(z(16.))
        .flex()
        .items_center()
        .gap(z(4.))
        .text_size(z(12.))
        .line_height(z(16.))
        .text_color(c(Palette::TEXT_MUTED))
        .overflow_hidden();
    let piece = match main {
        Some((t, icon)) => Some((Some(t), icon)),
        None if in_voice => Some((Some(crate::t!("members.in_voice")), Some("speaker"))),
        None => None,
    };
    if let Some((t, icon)) = piece {
        if let Some(i) = icon.filter(|_| !bot) {
            row = row.child(gpui::svg().path(crate::icons::path(i)).size(z(12.)).flex_none().text_color(green));
        }
        if let Some(t) = t.filter(|t| !custom_text && !t.is_empty()) {
            row = row.child(text_el(t));
        }
        if total > 1 && !bot {
            row = row.child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .when(custom_text, |d| d.text_color(c(tok(idx::STATUS_POSITIVE))))
                    .child(SharedString::from(format!("+{}", total - 1))),
            );
        }
    } else if custom.is_none()
        && let Some(t) = fallback.filter(|_| !in_voice)
    {
        row = row.child(text_el(t));
    }
    if let Some((emoji, state)) = custom {
        if total > 0 {
            row = row.child(div().flex_none().font_weight(FontWeight::NORMAL).text_color(c(Palette::TEXT_NORMAL)).child("•"));
        }
        if let Some(e) = emoji {
            // emoji_HigqCD 14px; custom (animated) emoji as images
            row = row.child(div().flex_none().text_size(z(14.)).child(crate::ui::emoji_text::label_with_custom(e, 14. * crate::theme::zoom() / 1.375)));
        }
        if let Some(t) = state {
            row = row.child(text_el(t));
        }
    }
    row
}

//! Forum and media channels: post list with tag filter and sort, post cards
//! (title, tags, author + first message preview, reply count, activity time,
//! image thumbnail), crate::t!("forum.new_post") composer with tags.
//!
//! Observed layout: 16px gutter, a 40px search/create bar with a blurple
//! crate::t!("forum.new_post") button, a row of 24px tag pills, a "Sort & View" menu, then
//! 8px-radius cards on the secondary background with 16px padding and a
//! 72px rounded thumbnail on the right.
//!
//! The chat column draws [`render`] over the message list when the open
//! channel is a forum.

use crate::theme::z;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Window, div, img, prelude::*, px};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    commands::{
        ForumPost, ForumTag, ParityReply, ParityRequest,
        state::{ForumCompose, ForumSort},
    },
    model::{AttachmentKind, ChannelKind, Id, ThreadRef},
    theme::{Palette, c, generated::idx, tok},
    ui::icon,
};

thread_local! {
    /// The "Search or create a post" input, per open forum.
    static SEARCH: std::cell::RefCell<Option<(Id, gpui::Entity<InputState>)>> = const { std::cell::RefCell::new(None) };
}

thread_local! {
    static LIST_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

fn search_input(forum: Id) -> Option<gpui::Entity<InputState>> {
    SEARCH.with(|s| s.borrow().as_ref().filter(|(f, _)| *f == forum).map(|(_, e)| e.clone()))
}

/// Case-insensitive match of the search text on the title, preview and tags.
fn post_matches(p: &ForumPost, tags: &[ForumTag], q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    let has = |s: &str| s.to_lowercase().contains(q);
    has(&p.title)
        || p.first_message.as_ref().is_some_and(|m| has(&m.content))
        || p.tags.iter().filter_map(|t| tags.iter().find(|x| x.id == *t)).any(|t| has(&t.name))
}

fn open_forum(app: &PhoenixApp) -> Option<Id> {
    let id = app.selected_channel?;
    (app.channels.get(&id)?.kind == ChannelKind::Forum).then_some(id)
}

/// Opens a thread or forum post by id (link, URL): from the cache, else
/// `GET /channels/{id}`, then selects it like the client's full post view.
pub fn open_by_id(app: &mut PhoenixApp, thread: Id, cx: &mut Context<PhoenixApp>) {
    if app.channels.contains_key(&thread) {
        app.select_channel(thread, cx);
        return;
    }
    if app.nav.find_thread(thread).is_some() {
        app.select_thread(thread, cx);
        return;
    }
    app.parity_request(ParityRequest::FetchThread { id: thread }, cx, move |a, r, cx| {
        // `open_channel_chat` switches to the post's guild itself.
        if let Ok(ParityReply::Thread(ch)) = r {
            a.channels.entry(thread).or_insert(ch);
            a.select_channel(thread, cx);
        }
    });
}

/// The author opened this forum post: the "OP" tag next to their name
/// (owner from the loaded post lists of the post's forum).
pub fn is_op(app: &PhoenixApp, channel: Id, author: Id) -> bool {
    let Some(forum) = app
        .channels
        .get(&channel)
        .and_then(|ch| ch.parent)
        .or_else(|| app.nav.find_thread(channel).and_then(|t| t.parent))
        .or_else(|| open_forum(app))
    else {
        return false;
    };
    app.parity
        .forums
        .get(&forum)
        .and_then(|v| v.posts.iter().chain(v.archived.iter()).find(|p| p.thread == channel))
        .is_some_and(|p| p.owner == Some(author))
}

/// "Follow" next to an announcement channel's name (vanilla-announcement
/// followButton_z_gUBg: padding 4 8, margin-inline-end 8 around a
/// button_F7qvhY sm secondary: h 32, padding 3 11, radius 8, body-sm/medium).
pub fn follow_button() -> gpui::Stateful<gpui::Div> {
    div().id("announcement-follow").flex_none().py(z(4.)).px(z(8.)).mr(z(8.)).child(
        div()
            .h(z(32.))
            .px(z(12.))
            .rounded(z(8.))
            .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
            .border_1()
            .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
            .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
            .child(crate::t!("forum.follow")),
    )
}

/// botTagOP_feVUtO (vanilla-thread-channel "OP"): the bot tag box
/// (.rem_.botTag_: h .9375rem, padding 0 .275rem, radius 4px; botTagCozy_
/// margin-inline-start .25rem; .rem_.botTag_.botTagOP_ margin-top .25em)
/// with --brand-260 background and --brand-560 text.
pub fn op_tag() -> gpui::Div {
    div()
        .flex_none()
        .ml(z(4.))
        .mt(z(2.5))
        .px(z(4.4))
        .h(z(15.))
        .rounded(z(4.))
        .bg(c(tok(idx::BRAND_260)))
        .flex()
        .items_center()
        // botText_: .8rem / 15px, 600
        .text_size(z(12.8))
        .line_height(z(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(tok(idx::BRAND_560)))
        .child(crate::t!("forum.op"))
}

/// Unix seconds of a snowflake.
fn snowflake_time(id: Id) -> i64 {
    (((id.0 >> 22) + 1_420_070_400_000) / 1000) as i64
}

pub fn ago(now: i64, ts: i64) -> String {
    crate::i18n::ago_short(now - ts).to_string()
}

/// Automatic retries after a failed post load before only the button is left.
const FORUM_AUTO_RETRIES: u32 = 3;

impl PhoenixApp {
    /// Active posts (`threads/search?archived=false`), every page, then the
    /// first page of archived ones for the "Older posts" section, as the
    /// client does.
    fn load_forum(&mut self, forum: Id, fresh: bool, cx: &mut Context<Self>) {
        let v = self.parity.forums.entry(forum).or_default();
        if v.loading {
            return;
        }
        v.loading = true;
        v.error = None;
        if fresh {
            v.posts.clear();
            v.archived.clear();
            v.archived_more = false;
            v.archived_loaded = false;
        }
        let req = ParityRequest::ForumPosts {
            forum,
            tags: v.tags.clone(),
            offset: v.posts.len() as u32,
            by_creation: v.sort == ForumSort::Created,
            archived: false,
        };
        self.parity_request(req, cx, move |a, res, cx| {
            let v = a.parity.forums.entry(forum).or_default();
            v.loading = false;
            v.loaded = true;
            match res {
                Ok(ParityReply::ForumPosts { posts, has_more }) => {
                    v.failures = 0;
                    let got = !posts.is_empty();
                    v.posts.extend(posts);
                    v.has_more = has_more;
                    if has_more && got {
                        a.load_forum(forum, false, cx);
                    } else {
                        a.load_archived(forum, cx);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    v.failures += 1;
                    v.error = Some(crate::t!("forum.load_failed", e = e.0.clone()).to_string().into());
                    let n = v.failures;
                    a.parity_toast(crate::t!("forum.load_failed", e = e.0).to_string(), cx);
                    // A few automatic retries with exponential backoff
                    // (2s, 4s, 8s); after that only the retry button.
                    if n <= FORUM_AUTO_RETRIES {
                        let delay = std::time::Duration::from_secs(1 << n);
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(delay).await;
                            let _ = this.update(cx, |a, cx| {
                                let still = a.parity.forums.get(&forum).is_some_and(|v| v.error.is_some() && !v.loading);
                                if still && a.selected_channel == Some(forum) {
                                    a.load_forum(forum, true, cx);
                                }
                            });
                        })
                        .detach();
                    }
                }
            }
        });
    }

    /// Next page of archived posts (`threads/search?archived=true`).
    fn load_archived(&mut self, forum: Id, cx: &mut Context<Self>) {
        let v = self.parity.forums.entry(forum).or_default();
        if v.archived_loading || (v.archived_loaded && !v.archived_more) {
            return;
        }
        v.archived_loading = true;
        let req = ParityRequest::ForumPosts {
            forum,
            tags: v.tags.clone(),
            offset: v.archived.len() as u32,
            by_creation: v.sort == ForumSort::Created,
            archived: true,
        };
        self.parity_request(req, cx, move |a, res, cx| {
            let v = a.parity.forums.entry(forum).or_default();
            v.archived_loading = false;
            v.archived_loaded = true;
            if let Ok(ParityReply::ForumPosts { posts, has_more }) = res {
                v.archived_more = has_more && !posts.is_empty();
                v.archived.extend(posts);
            }
            cx.notify();
        });
    }

    fn toggle_forum_tag(&mut self, forum: Id, tag: Id, cx: &mut Context<Self>) {
        let v = self.parity.forums.entry(forum).or_default();
        if let Some(i) = v.tags.iter().position(|t| *t == tag) {
            v.tags.remove(i);
        } else {
            v.tags.push(tag);
        }
        self.load_forum(forum, true, cx);
        cx.notify();
    }

    fn set_forum_sort(&mut self, forum: Id, sort: ForumSort, cx: &mut Context<Self>) {
        let v = self.parity.forums.entry(forum).or_default();
        v.sort = sort;
        v.sort_menu = false;
        self.load_forum(forum, true, cx);
        cx.notify();
    }

    fn submit_post(&mut self, forum: Id, cx: &mut Context<Self>) {
        let Some(comp) = self.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut()) else { return };
        let title = comp.title.read(cx).value().trim().to_owned();
        let content = comp.body.read(cx).value().trim().to_owned();
        if title.is_empty() || content.is_empty() {
            comp.error = Some(crate::t!("forum.needs_title"));
            cx.notify();
            return;
        }
        comp.sending = true;
        let tags = comp.tags.clone();
        self.parity_request(ParityRequest::CreatePost { forum, title: title.clone(), content, tags }, cx, move |a, res, cx| {
            match res {
                Ok(ParityReply::Channel(t)) => {
                    if let Some(v) = a.parity.forums.get_mut(&forum) {
                        v.compose = None;
                    }
                    a.load_forum(forum, true, cx);
                    a.parity.forum_open_thread = Some(ThreadRef { id: t, name: title.into(), message_count: 0 });
                }
                Ok(_) => {}
                Err(e) => {
                    if let Some(comp) = a.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut()) {
                        comp.sending = false;
                        comp.error = Some(e.0.into());
                    }
                }
            }
        });
        cx.notify();
    }
}

/// Window-bound work: first load, composer inputs, opening a new post.
pub fn prepare(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if let Some(t) = app.parity.forum_open_thread.take() {
        app.open_thread(t, window, cx);
    }
    let Some(forum) = open_forum(app) else { return };
    if search_input(forum).is_none() {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("forum.search")));
        cx.subscribe(&input, move |a: &mut PhoenixApp, _, ev: &gpui_component::input::InputEvent, cx| match ev {
            gpui_component::input::InputEvent::Change => cx.notify(),
            // Enter creates a post titled with the search text.
            gpui_component::input::InputEvent::PressEnter { .. } => {
                a.parity.forum_compose_request = Some(forum);
                cx.notify();
            }
            _ => {}
        })
        .detach();
        SEARCH.with(|s| *s.borrow_mut() = Some((forum, input)));
    }
    if let Some(i) = search_input(forum) {
        crate::ui::locale_inputs::keep_placeholder(&i, "forum.search", window, cx);
    }
    let v = app.parity.forums.entry(forum).or_default();
    if !v.loaded && !v.loading {
        app.load_forum(forum, true, cx);
    }
    // Paging: the next archived page once the list is scrolled near its end.
    let (off, max) = LIST_SCROLL.with(|h| (h.offset().y, h.max_offset().y));
    let v = app.parity.forums.entry(forum).or_default();
    if v.archived_more && !v.archived_loading && max > px(0.) && -off > max - px(600.) {
        app.load_archived(forum, cx);
    }
    if app.parity.forum_compose_request == Some(forum) {
        app.parity.forum_compose_request = None;
        let title = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("forum.post_title")));
        let body = cx.new(|cx| TextareaState::new(window, cx).auto_grow(4, 12).placeholder(crate::t!("forum.type_message")));
        let query = search_input(forum).map(|i| i.read(cx).value().to_string()).unwrap_or_default();
        title.update(cx, |s, cx| {
            if !query.trim().is_empty() {
                s.set_value(query.trim().to_string(), window, cx);
            }
            s.focus(window, cx)
        });
        app.parity.forums.entry(forum).or_default().compose =
            Some(ForumCompose { title, body, tags: Vec::new(), sending: false, error: None });
    }
}

// Values below come from the vanilla dump `vanilla-forum` (midnight theme,
// visual refresh) and the matching rules in current-client/css.

/// Header tag filter chip: `pill_snX-u4` (h 32, padding 0 12, radius 20,
/// --control-secondary-background-default, 1px --border-subtle, 14px
/// semibold --control-secondary-text-default).
fn tag_pill(t: &ForumTag, on: bool) -> gpui::Div {
    div()
        .h(z(32.))
        .px(z(12.))
        .rounded(z(20.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .flex()
        .flex_none()
        .items_center()
        .gap(z(4.))
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::SEMIBOLD)
        // .selected_snX-u4: --brand-15a (Light --brand-160) on --brand-500 with
        // --reaction-text-reacted-default.
        .when(on, |d| d.border_color(c(tok(idx::BRAND_500))))
        .bg(c(tok(match (on, crate::theme::is_light()) {
            (true, true) => idx::BRAND_160,
            (true, false) => idx::BRAND_15A,
            _ => idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT,
        })))
        .text_color(c(tok(if on { idx::REACTION_TEXT_REACTED_DEFAULT } else { idx::CONTROL_SECONDARY_TEXT_DEFAULT })))
        .when_some(t.emoji.clone(), |d, e| d.child(crate::media::emoji::emoji_element(&e, 16.)))
        .child(t.name.clone())
}

/// Hover / pressed of a clickable tag pill (`pill_snX-u4 clickable_`, live client
/// 2026-11-24): --background-mod-subtle → hover --background-mod-normal → pressed
/// --background-mod-strong (same values as the control-secondary tokens), text unchanged.
/// A selected pill keeps its blurple fill.
fn pill_states(d: gpui::Stateful<gpui::Div>, on: bool) -> gpui::Stateful<gpui::Div> {
    if on {
        return d;
    }
    d.hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
        .active(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE))))
}

/// Tag shown on a card (`tags_nLtnFF` row, 24px high).
fn card_tag(t: &ForumTag) -> gpui::Div {
    div()
        .h(z(24.))
        .px(z(8.))
        .rounded(z(12.))
        .flex()
        .flex_none()
        .items_center()
        .gap(z(4.))
        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
        .text_size(z(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
        .when_some(t.emoji.clone(), |d, e| d.child(crate::media::emoji::emoji_element(&e, 14.)))
        .child(t.name.clone())
}

/// One-line preview: newlines folded, block markdown prefixes (`# `, `-# `,
/// `> `, `- `) dropped as the inline renderer does.
fn preview_text(s: &str) -> String {
    s.lines()
        .map(|l| {
            let l = l.trim_start();
            ["-# ", "### ", "## ", "# ", ">>> ", "> ", "- ", "* "].iter().find_map(|p| l.strip_prefix(p)).unwrap_or(l)
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Highlighted title/preview, as the client's `isNew || hasUnreads`: never
/// for an archived post; a post with its own read state is unread when its
/// last message is past the ack; one never opened is "new" when its id is
/// past the forum channel's own ack (`isNewForumThread`).
pub(crate) fn is_unread(app: &PhoenixApp, p: &ForumPost) -> bool {
    if p.archived {
        return false;
    }
    match app.backend.last_read(p.thread) {
        Some(r) => p.last_message.is_some_and(|l| l.0 > r.0),
        None => open_forum(app).and_then(|f| app.backend.last_read(f)).is_some_and(|r| p.thread.0 > r.0),
    }
}

/// Footer time: the date for a pinned post, else relative, capped at
/// "More than 30d ago" like the client.
fn footer_time(now: i64, p: &ForumPost) -> String {
    if p.pinned {
        let ts = snowflake_time(p.thread);
        return chrono::DateTime::from_timestamp(ts, 0)
            .map(|d| crate::i18n::long_date(d.with_timezone(&chrono::Local).date_naive()))
            .unwrap_or_default();
    }
    let ts = snowflake_time(p.last_message.unwrap_or(p.thread));
    if now - ts > 30 * 86_400 { crate::t!("forum.over_30d").to_string() } else { ago(now, ts) }
}

fn post_card(app: &PhoenixApp, p: &ForumPost, tags: &[ForumTag], now: i64, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let first = p.first_message.as_ref();
    // The client names the first message's author (the owner can differ).
    let author_id = first.map(|m| m.author).or(p.owner);
    let author = author_id.and_then(|u| app.users.get(&u));
    let guild = app.channels.get(&p.thread).and_then(|ch| ch.guild).or(app.selected_guild);
    let author_color = guild
        .zip(author_id)
        .and_then(|(g, u)| app.members.get(&g).and_then(|ms| ms.iter().find(|m| m.user == u)).and_then(|m| app.member_color(g, m)));
    // Inline markdown (`**bold**`, `code`) and raw mentions resolved like the reply preview,
    // instead of showing the markers.
    let preview: SharedString =
        first.map(|m| crate::ui::chat::reply_preview_text(app, &preview_text(&m.content).into())).unwrap_or_default();
    let thumb = first.and_then(|m| m.attachments.iter().find(|a| a.kind == AttachmentKind::Image)).map(|a| a.url.clone());
    let reaction = first.and_then(|m| m.reactions.iter().max_by_key(|r| r.count)).cloned();
    let r = ThreadRef { id: p.thread, name: p.title.clone().into(), message_count: p.message_count };
    let unread = is_unread(app, p);
    // Read posts dim the title and preview to --text-muted (lineClamp2Plus h3).
    let title_color = if unread { c(Palette::TEXT_HEADER) } else { c(Palette::TEXT_MUTED) };
    let preview_color = if unread { c(Palette::TEXT_NORMAL) } else { c(Palette::TEXT_MUTED) };
    let post_tags: Vec<&ForumTag> = p.tags.iter().filter_map(|t| tags.iter().find(|x| x.id == *t)).collect();
    // card_BkGrJJ li: padding-bottom 16.
    div().pb(z(16.)).child(
        div()
            .id(("forum-post", p.thread.0))
            // container_SOY5H6: padding var(--space-12) calc(var(--space-8) + 2px)
            .py(z(12.))
            .px(z(10.))
            // mainCard_BkGrJJ: --background-base-low, 1px --border-subtle, --radius-md
            .rounded(z(12.))
            .bg(c(Palette::BG_BASE_LOW))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .cursor_pointer()
            // container_SOY5H6 (live client 2026-11-24, transitions 0.2s ease-out not animated):
            // :hover --background-surface-high, --border-strong, --shadow-low;
            // :active --interactive-background-active, --border-strong, --shadow-border + --shadow-high.
            .hover(|s| {
                s.bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))).border_color(c(tok(idx::BORDER_STRONG))).shadow(crate::ui::inbox::low_shadow())
            })
            .active(|s| {
                s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                    .border_color(c(tok(idx::BORDER_STRONG)))
                    .shadow(crate::ui::inbox::popout_shadow())
            })
            .flex()
            .items_start()
            .on_click(cx.listener(move |a, _, w, cx| a.open_thread(r.clone(), w, cx)))
            .when_some(open_forum(app), |d, forum| {
                let post = p.clone();
                d.on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| crate::ui::thread_menu::open_post(post.clone(), forum, ev.position, w))
            })
            .child(
                // body_SOY5H6: padding-bottom 8
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .when(p.pinned || !post_tags.is_empty(), |d| {
                        // tags_nLtnFF: h 24, gap 8, margin-bottom 8
                        d.child(
                            div()
                                .h(z(24.))
                                .mb(z(8.))
                                .flex()
                                .items_center()
                                .gap(z(8.))
                                .overflow_hidden()
                                .when(p.pinned, |d| {
                                    // pinIcon_nLtnFF: 22px circle, --brand-500, 16px icon
                                    d.child(
                                        div()
                                            .size(z(22.))
                                            .flex_none()
                                            .rounded_full()
                                            .bg(c(Palette::BLURPLE))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(icon(Icon::Pin, 14., gpui::white())),
                                    )
                                })
                                .children(post_tags.iter().map(|t| card_tag(t))),
                        )
                    })
                    .child(
                        // header_SOY5H6 padding-bottom 6; h3 heading-lg/semibold 20/24, 2 lines
                        div()
                            .pb(z(6.))
                            .text_size(z(20.))
                            .line_height(z(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(title_color)
                            .line_clamp(2)
                            .child(SharedString::from(p.title.clone())),
                    )
                    .child(
                        // message_SOY5H6: body-sm 14/18; author semibold, content medium + ml 4
                        div()
                            .flex()
                            .h(z(18.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .min_w_0()
                            .overflow_hidden()
                            .child({
                                let author_name: SharedString = author.map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("forum.deleted_user"));
                                crate::fonts::styled_name(
                                    div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(author_color.map(c).unwrap_or(c(Palette::TEXT_HEADER))),
                                    author,
                                    false,
                                )
                                .child(match author_id.and_then(|u| app.author_role_gradient_in(guild, u)) {
                                    Some(stops) => crate::fonts::name_text_with(author_name, author, 14., Some(stops)),
                                    None => crate::fonts::name_text(author_name, author, 14.),
                                })
                            })
                            .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(Palette::TEXT_NORMAL)).child(":"))
                            .child(
                                div()
                                    .ml(z(4.))
                                    .min_w_0()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_color(preview_color)
                                    .child(preview),
                            ),
                    )
                    .child(
                        // footer_SOY5H6: margin-top = body padding 8, h 20, gap 8
                        div()
                            .mt(z(8.))
                            .h(z(20.))
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(c(Palette::TEXT_NORMAL))
                            .when_some(reaction, |d, re| {
                                d.child(
                                    div()
                                        .h(z(20.))
                                        .px(z(6.))
                                        .rounded(z(8.))
                                        .flex()
                                        .items_center()
                                        .gap(z(4.))
                                        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(crate::media::emoji::emoji_element(&re.emoji, 14.))
                                        .child(re.count.to_string()),
                                )
                            })
                            .child(
                                // messageCountBox: icon 14 + margin-right 4, count 14/14 medium
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(z(4.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(gpui::svg().path(crate::icons::path("chat")).size(z(14.)).flex_none().text_color(c(Palette::TEXT_NORMAL)))
                                    .child(p.message_count.to_string()),
                            )
                            .child(div().size(z(4.)).rounded_full().bg(c(Palette::TEXT_MUTED)))
                            .child(footer_time(now, p)),
                    ),
            )
            // bodyMedia_SOY5H6: 72x72, radius 8, margin-left 12
            .when_some(thumb, |d, u| d.child(img(u).ml(z(12.)).size(z(72.)).flex_none().rounded(z(8.)).object_fit(gpui::ObjectFit::Cover))),
    )
    .into_any_element()
}

/// Media channel tile: a 16:9 cover (first image, else a placeholder icon),
/// then tags, the title and the reply count / activity line.
fn media_tile(app: &PhoenixApp, p: &ForumPost, tags: &[ForumTag], now: i64, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let first = p.first_message.as_ref();
    let thumb = first.and_then(|m| m.attachments.iter().find(|a| a.kind == AttachmentKind::Image)).map(|a| a.url.clone());
    let author = p.owner.or(first.map(|m| m.author)).and_then(|u| app.users.get(&u));
    let activity = snowflake_time(p.last_message.unwrap_or(p.thread));
    let r = ThreadRef { id: p.thread, name: p.title.clone().into(), message_count: p.message_count };
    let cover = div().w_full().h(z(146.)).rounded_t(z(8.)).overflow_hidden().bg(c(Palette::BG_TERTIARY));
    let cover = match thumb {
        Some(u) => cover.child(img(u).size_full().object_fit(gpui::ObjectFit::Cover)),
        None => cover.flex().items_center().justify_center().child(icon(Icon::Forum, 40., c(Palette::INTERACTIVE_MUTED))),
    };
    div()
        .id(("media-post", p.thread.0))
        .w(z(260.))
        .flex_none()
        .rounded(z(8.))
        .bg(c(Palette::BG_SECONDARY))
        .border_1()
        .border_color(c(Palette::PANEL_BORDER))
        .cursor_pointer()
        .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
        .on_click(cx.listener(move |a, _, w, cx| a.open_thread(r.clone(), w, cx)))
        .child(cover)
        .child(
            div()
                .p(z(12.))
                .flex()
                .flex_col()
                .gap(z(6.))
                .when(!p.tags.is_empty(), |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(z(4.))
                            .children(p.tags.iter().filter_map(|t| tags.iter().find(|x| x.id == *t)).map(|t| tag_pill(t, false))),
                    )
                })
                .child(
                    div()
                        .text_size(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(Palette::TEXT_HEADER))
                        .text_ellipsis()
                        .child(SharedString::from(p.title.clone())),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .text_size(z(12.))
                        .text_color(c(Palette::TEXT_MUTED))
                        .child(div().min_w_0().text_ellipsis().child(author.map(|u| u.display_name.to_string()).unwrap_or_default()))
                        .child(div().flex().items_center().gap(z(4.)).child(icon(Icon::Threads, 14., c(Palette::INTERACTIVE))).child(p.message_count.to_string()))
                        .child(ago(now, activity)),
                ),
        )
        .into_any_element()
}

/// Post list covering the chat column when a forum is open.
const INLINE_COMPOSER: bool = true;

/// The "Create a post" form that replaces the search bar (vanilla-forum-new-post
/// container_owUmX8): same mainCard header box, title + message on the left,
/// a 78px media tile on the right, the tag row, a divider and the controls.
fn compose_card(forum: Id, comp: &ForumCompose, tags: &[ForumTag], cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let close = cx.listener(move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _, cx| {
        if let Some(v) = a.parity.forums.get_mut(&forum) {
            v.compose = None;
        }
        cx.notify();
    });
    let ready = !comp.title.read(cx).value().trim().is_empty() && !comp.body.read(cx).value().trim().is_empty();
    // tagListInner_owUmX8 gap 4; pill_snX-u4 small: h 24, padding 0 8, radius 20
    let mut tag_list = div().flex().flex_wrap().gap(z(4.));
    for t in tags.iter().filter(|t| !t.moderated) {
        let on = comp.tags.contains(&t.id);
        let id = t.id;
        tag_list = tag_list.child(
            pill_states(tag_pill(t, on).h(z(24.)).px(z(8.)).text_size(z(12.)).line_height(z(16.)).id(("compose-tag", id.0)), on)
                .cursor_pointer()
                .on_click(cx.listener(move |a, _, _, cx| {
                    if let Some(cmp) = a.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut()) {
                        if let Some(i) = cmp.tags.iter().position(|x| *x == id) {
                            cmp.tags.remove(i);
                        } else if cmp.tags.len() < 5 {
                            cmp.tags.push(id);
                        }
                    }
                    cx.notify();
                })),
        );
    }
    let secondary = |id: &'static str, path: &'static str| {
        // button_F7qvhY sm secondary 32x32, radius 8, 16px icon
        div()
            .id(id)
            .size(z(32.))
            .flex_none()
            .rounded(z(8.))
            .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
            .border_1()
            .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
            .flex()
            .items_center()
            .justify_center()
            .child(gpui::svg().path(crate::icons::path(path)).size(z(16.)).flex_none().text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
    };
    div()
        .mt(z(24.)) // headerContainer_BkGrJJ margin-top 24
        .rounded(z(12.)) // mainCard_ --radius-md
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .py(z(8.)) // container_owUmX8 padding 8 0
        .flex()
        .flex_col()
        .child(
            // formContainer_owUmX8: padding 0 8 0 12
            div()
                .pl(z(12.))
                .pr(z(8.))
                .flex()
                .items_start()
                .child(
                    // prefixElement_ 26x32, margin-right 8; clear (X) icon 26
                    div()
                        .id("compose-close")
                        .w(z(26.))
                        .h(z(32.))
                        .mr(z(8.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .on_click(close)
                        .child(gpui::svg().path(crate::icons::path("close")).size(z(26.)).flex_none().text_color(c(Palette::INTERACTIVE))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            // title_owUmX8 heading-lg/semibold 20/24, h 32, padding-top 4
                            div()
                                .h(z(32.))
                                .pt(z(4.))
                                .text_size(z(20.))
                                .line_height(z(24.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(tok(idx::TEXT_STRONG)))
                                .child(Input::new(&comp.title).appearance(false).bordered(false).text_size(z(20.)).line_height(z(24.))),
                        )
                        .child(
                            // channelTextArea_owUmX8: min 80, textArea 16/22 --text-default
                            div()
                                .min_h(z(80.))
                                .text_size(z(16.))
                                .line_height(z(22.))
                                .child(Textarea::new(&comp.body).appearance(false).bordered(false)),
                        ),
                )
                .child(
                    // uploadInput_YeZ5Kj: 78x78, radius 12, 1px --border-subtle
                    div()
                        .size(z(78.))
                        .flex_none()
                        .rounded(z(12.))
                        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE)))
                        .border_1()
                        .border_color(c(tok(idx::BORDER_SUBTLE)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(gpui::svg().path(crate::icons::path("image-plus")).size(z(32.)).flex_none().text_color(c(Palette::INTERACTIVE))),
                ),
        )
        .when_some(comp.error.clone(), |d, er| {
            d.child(div().px(z(46.)).pt(z(4.)).text_size(z(14.)).text_color(c(Palette::TEXT_CRITICAL)).child(er))
        })
        .child(
            // horizontalPadding_ 0 8; tagsContainer_owUmX8 margin 16 0 12, gap 8; tagsIcon 16 --interactive-normal
            div().px(z(8.)).when(!tags.is_empty(), |d| {
                d.child(
                    div()
                        .mt(z(16.))
                        .mb(z(12.))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .child(gpui::svg().path(crate::icons::path("forum/tag")).size(z(16.)).flex_none().text_color(c(Palette::INTERACTIVE)))
                        .child(tag_list),
                )
            }),
        )
        // tagsDivider_owUmX8: 1px --border-subtle
        .child(div().h(px(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(
            // controlsContainer_owUmX8: margin-top 12, h 40, gap 8; controls_ gap 8
            div()
                .px(z(8.))
                .mt(z(12.))
                .h(z(40.))
                .flex()
                .items_center()
                .gap(z(8.))
                .child(
                    // emojiButton_SjG8Ug 32x32, sprite 18
                    div().size(z(32.)).flex_none().flex().items_center().justify_center().child(
                        gpui::svg().path(crate::icons::path("emoji")).size(z(18.)).flex_none().text_color(c(Palette::INTERACTIVE)),
                    ),
                )
                .child(div().flex_1())
                .child(secondary("compose-guide", "file"))
                .child(
                    // button_F7qvhY sm primary: h 32, padding 3 11, radius 8; disabled opacity .5
                    div()
                        .id("compose-post")
                        .h(z(32.))
                        .px(z(12.))
                        .flex_none()
                        .rounded(z(8.))
                        .bg(c(Palette::BLURPLE))
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(gpui::white())
                        .when(!ready || comp.sending, |d| d.opacity(0.5))
                        .when(ready && !comp.sending, |d| d.cursor_pointer().on_click(cx.listener(move |a, _, _, cx| a.submit_post(forum, cx))))
                        .child(gpui::svg().path(crate::icons::path("chat")).size(z(16.)).flex_none().text_color(gpui::white()))
                        .child(if comp.sending { crate::t!("forum.posting") } else { crate::t!("forum.post") }),
                ),
        )
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let forum = open_forum(app)?;
    let tags = app.backend.forum_tags(forum);
    let empty = Default::default();
    let v = app.parity.forums.get(&forum).unwrap_or(&empty);
    let now = chrono::Utc::now().timestamp();

    // headerContainer_BkGrJJ (margin-top 24) > mainCard header_BkGrJJ: h 48,
    // --input-background-default, 1px --border-subtle with
    // --input-border-default at the bottom, --radius-md.
    let search_bar = div()
        .mt(z(24.))
        .h(z(48.))
        .rounded(z(12.))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .border_b_1()
        .flex()
        .items_center()
        .child(div().w(z(46.)).flex_none().flex().justify_center().child(icon(Icon::Search, 24., c(Palette::INTERACTIVE))))
        .child(
            // title_owUmX8: heading-lg/semibold 20/24
            div()
                .flex_1()
                .min_w_0()
                .text_size(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .when_some(search_input(forum), |d, i| d.child(Input::new(&i).appearance(false).bordered(false).text_size(z(20.)).line_height(z(24.)))),
        )
        .child(
            // button_F7qvhY sm secondary: 32x32, radius 8
            div()
                .id("forum-guide")
                .size(z(32.))
                .flex_none()
                .rounded(z(8.))
                .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
                .border_1()
                .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(Icon::File, 16., c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))),
        )
        .child(
            // button_F7qvhY sm primary: h 32, padding 3 11, radius 8, body-sm/medium
            div()
                .id("forum-new-post")
                .ml(z(9.))
                .mr(z(8.))
                .h(z(32.))
                .px(z(12.))
                .flex_none()
                .rounded(z(8.))
                .bg(c(Palette::BLURPLE))
                .cursor_pointer()
                .hover(|s| s.opacity(0.9))
                .flex()
                .items_center()
                .gap(z(4.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(gpui::white())
                .on_click(cx.listener(move |a, _, _, cx| {
                    a.parity.forum_compose_request = Some(forum);
                    cx.notify();
                }))
                .child(gpui::svg().path(crate::icons::path("chat")).size(z(16.)).flex_none().text_color(gpui::white()))
                .child(crate::t!("forum.new_post")),
        );

    // tagsContainer_BkGrJJ: padding 16 0 4
    let sort_open = app.parity.forums.get(&forum).is_some_and(|v| v.sort_menu);
    let mut filters = div().pt(z(16.)).pb(z(4.)).flex().items_center().overflow_hidden().child(
        // sortDropdown_BkGrJJ: h 32, padding 8 8 8 12, --radius-xl, gap 4
        div()
            .id("forum-sort")
            .h(z(32.))
            .pl(z(12.))
            .pr(z(8.))
            .flex_none()
            .rounded(z(24.))
            .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(Palette::INTERACTIVE))
            .on_click(cx.listener(move |a, _, _, cx| {
                let v = a.parity.forums.entry(forum).or_default();
                v.sort_menu = !v.sort_menu;
                cx.notify();
            }))
            // ArrowsUpDownIcon size "xs" (16px)
            .child(gpui::svg().path(crate::icons::path("sort")).size(z(16.)).flex_none().text_color(c(Palette::INTERACTIVE)))
            // sortDropdownText_BkGrJJ: margin 0 4 0 8
            .child(div().ml(z(8.)).mr(z(4.)).child(crate::t!("forum.sort_view")))
            // ChevronSmallUpIcon while the popout is shown, else Down; width 20
            .child(
                gpui::svg()
                    .path(crate::icons::path(if sort_open { "chevron-up" } else { "chevron" }))
                    .w(z(20.))
                    .h(z(20.))
                    .flex_none()
                    .text_color(c(Palette::INTERACTIVE)),
            ),
    );
    if !tags.is_empty() {
        // divider_BkGrJJ: 2x24, margin 0 16, --border-subtle, --radius-xs
        filters = filters.child(div().w(z(2.)).h(z(24.)).mx(z(16.)).flex_none().rounded(z(4.)).bg(c(tok(idx::BORDER_SUBTLE))));
        // tagListInner_BkGrJJ: flex-wrap, gap 8 (tags that don't fit wrap out of sight,
        // never shown cut in half)
        let mut list = div().flex().flex_wrap().gap(z(8.));
        for t in &tags {
            let on = v.tags.contains(&t.id);
            let id = t.id;
            list = list.child(
                pill_states(tag_pill(t, on).id(("forum-tag", id.0)), on)
                    .cursor_pointer()
                    .on_click(cx.listener(move |a, _, _, cx| a.toggle_forum_tag(forum, id, cx))),
            );
        }
        // tagList_BkGrJJ: flex-shrink 1, height 32, overflow hidden
        filters = filters.child(div().min_w_0().h(z(32.)).overflow_hidden().child(list)).child(
            // tagsButton_BkGrJJ: h 32, padding 0 4, radius 20, margin-left 8; inner padding-left 8, gap 4
            div()
                .ml(z(8.))
                .h(z(32.))
                .px(z(4.))
                .flex_none()
                .rounded(z(20.))
                .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .flex()
                .items_center()
                .gap(z(4.))
                .pl(z(12.))
                .text_size(z(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(Palette::INTERACTIVE))
                .child(crate::t!("forum.all_tags"))
                .child(icon(Icon::Chevron, 16., c(Palette::INTERACTIVE))),
        );
    }

    // list_BkGrJJ: cards at 16px from the left, scrollbar gutter on the right.
    let mut col = div()
        .id("forum-list")
        .track_scroll(&LIST_SCROLL.with(|h| h.clone()))
        .size_full()
        .overflow_y_scroll()
        .pl(z(16.))
        .pr(z(32.))
        .flex()
        .flex_col()
        // headerRow_BkGrJJ: padding-bottom 16
        .child(div().pb(z(16.)).child(match v.compose.as_ref() {
            Some(comp) => compose_card(forum, comp, &tags, cx).into_any_element(),
            None => search_bar.into_any_element(),
        }).child(filters));
    if let (Some(err), true) = (v.error.clone(), v.posts.is_empty() && !v.loading) {
        // Failed load: message and a "Try again" button instead of the
        // empty state, like the client's error view.
        col = col.child(
            div()
                .p(z(48.))
                .flex()
                .flex_col()
                .items_center()
                .gap(z(8.))
                .child(icon(Icon::Forum, 64., c(Palette::INTERACTIVE_MUTED)))
                .child(div().text_size(z(14.)).text_color(c(Palette::TEXT_MUTED)).child(err))
                .child(
                    div()
                        .id("forum-retry")
                        .mt(z(8.))
                        .h(z(32.))
                        .px(z(16.))
                        .rounded(z(8.))
                        .flex()
                        .items_center()
                        .bg(c(Palette::BLURPLE)) // --background-brand (button_ filled brand)
                        .text_color(gpui::white())
                        .text_size(z(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .on_click(cx.listener(move |a, _, _, cx| {
                            if let Some(v) = a.parity.forums.get_mut(&forum) {
                                v.failures = 0;
                            }
                            a.load_forum(forum, true, cx);
                        }))
                        .child(crate::t!("common.retry")),
                ),
        );
    } else if v.loaded && v.posts.is_empty() && v.archived.is_empty() && !v.loading && !v.archived_loading {
        col = col.child(
            div()
                .p(z(48.))
                .flex()
                .flex_col()
                .items_center()
                .gap(z(8.))
                .child(icon(Icon::Forum, 64., c(Palette::INTERACTIVE_MUTED)))
                .child(div().text_size(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(Palette::TEXT_HEADER)).child(crate::t!("forum.empty")))
                .child(div().text_size(z(14.)).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("forum.empty_sub"))),
        );
    }
    let query = search_input(forum).map(|i| i.read(cx).value().trim().to_lowercase()).unwrap_or_default();
    let mut active: Vec<&ForumPost> = v.posts.iter().filter(|p| post_matches(p, &tags, &query)).collect();
    // Pinned posts stay on top.
    active.sort_by_key(|p| !p.pinned);
    let older: Vec<&ForumPost> = v.archived.iter().filter(|p| post_matches(p, &tags, &query)).collect();
    if !query.is_empty() && active.is_empty() && older.is_empty() && !(v.posts.is_empty() && v.archived.is_empty()) {
        col = col.child(
            div()
                .p(z(32.))
                .flex()
                .justify_center()
                .text_size(z(14.))
                .text_color(c(Palette::TEXT_MUTED))
                .child(crate::t!("forum.no_match")),
        );
    }
    // archivedDividerRow_BkGrJJ: padding 16 0 8; h2 body-sm/medium --text-muted.
    let older_header = || {
        div()
            .pt(z(16.))
            .pb(z(8.))
            .text_size(z(14.))
            .line_height(z(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(Palette::TEXT_MUTED))
            .child(crate::t!("forum.older"))
    };
    if app.backend.is_media_channel(forum) {
        // Media channels: a wrapping grid of thumbnail tiles.
        col = col.child(div().flex().flex_wrap().gap(z(16.)).children(active.iter().map(|p| media_tile(app, p, &tags, now, cx))));
        if !older.is_empty() {
            col = col
                .child(older_header())
                .child(div().flex().flex_wrap().gap(z(16.)).children(older.iter().map(|p| media_tile(app, p, &tags, now, cx))));
        }
    } else {
        for p in active {
            col = col.child(post_card(app, p, &tags, now, cx));
        }
        if !older.is_empty() {
            col = col.child(older_header());
            for p in older {
                col = col.child(post_card(app, p, &tags, now, cx));
            }
        }
    }
    if v.loading || v.archived_loading {
        col = col.child(div().p(z(16.)).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("common.loading")));
    } else if v.archived_more {
        col = col.child(
            div()
                .id("forum-more")
                .p(z(12.))
                .flex()
                .justify_center()
                .cursor_pointer()
                .text_color(c(Palette::TEXT_LINK))
                .on_click(cx.listener(move |a, _, _, cx| a.load_archived(forum, cx)))
                .child(crate::t!("forum.load_more")),
        );
    }

    let sort_menu = v.sort_menu.then(|| {
        let item = |id: &'static str, label: SharedString, s: ForumSort, on: bool| {
            div()
                .id(id)
                .px(z(8.))
                .py(z(6.))
                .rounded(z(4.))
                .cursor_pointer()
                .hover(|st| st.bg(c(Palette::BLURPLE)))
                .flex()
                .justify_between()
                .text_size(z(14.))
                .text_color(c(Palette::TEXT_NORMAL))
                .child(label)
                .when(on, |d| d.child("✓"))
                .on_click(cx.listener(move |a, _, _, cx| a.set_forum_sort(forum, s, cx)))
        };
        div()
            .absolute()
            .top(z(128.))
            .left(z(16.))
            .w(z(220.))
            .p(z(8.))
            .rounded(z(8.))
            .bg(c(Palette::BG_FLOATING))
            .shadow_lg()
            .occlude()
            .child(div().px(z(8.)).py(z(4.)).text_size(z(12.)).font_weight(FontWeight::BOLD).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("forum.sort_by")))
            .child(item("sort-recent", crate::t!("forum.recent"), ForumSort::Recent, v.sort == ForumSort::Recent))
            .child(item("sort-created", crate::t!("forum.created"), ForumSort::Created, v.sort == ForumSort::Created))
    });

    Some(
        div()
            .id("forum-view")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .bg(c(Palette::BG_PRIMARY))
            .child(col)
            .children(sort_menu)
            .into_any_element(),
    )
}

/// crate::t!("forum.new_post") dialog (title, message, tags).
/// Old modal composer; the client composes inline ([`compose_card`]), kept
/// disabled until removed.
pub fn compose_modal(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if INLINE_COMPOSER {
        return None;
    }
    let forum = open_forum(app)?;
    let comp = app.parity.forums.get(&forum)?.compose.as_ref()?;
    let tags = app.backend.forum_tags(forum);
    let e = cx.entity();
    let close: crate::ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        e.update(cx, |a, cx| {
            if let Some(v) = a.parity.forums.get_mut(&forum) {
                v.compose = None;
            }
            cx.notify();
        })
    });
    let mut tag_row = div().flex().flex_wrap().gap(z(6.));
    for t in tags.iter().filter(|t| !t.moderated) {
        let on = comp.tags.contains(&t.id);
        let id = t.id;
        tag_row = tag_row.child(
            pill_states(tag_pill(t, on).id(("compose-tag", id.0)), on)
                .cursor_pointer()
                .on_click(cx.listener(move |a, _, _, cx| {
                    if let Some(cmp) = a.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut()) {
                        if let Some(i) = cmp.tags.iter().position(|x| *x == id) {
                            cmp.tags.remove(i);
                        } else if cmp.tags.len() < 5 {
                            cmp.tags.push(id);
                        }
                    }
                    cx.notify();
                })),
        );
    }
    let field = |el: AnyElement, h: f32| {
        div().min_h(px(h)).px(z(10.)).py(z(8.)).rounded(z(8.)).bg(c(Palette::INPUT_BG)).text_size(z(16.)).child(el)
    };
    let close2 = close.clone();
    let card = div()
        .w(z(560.))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(Palette::BG_PRIMARY))
        .shadow_lg()
        .flex()
        .flex_col()
        .child(
            div()
                .p(z(16.))
                .flex()
                .flex_col()
                .gap(z(12.))
                .child(div().text_size(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(Palette::TEXT_HEADER)).child(crate::t!("forum.new_post")))
                .child(field(Input::new(&comp.title).appearance(false).bordered(false).into_any_element(), 40.))
                .child(field(Textarea::new(&comp.body).appearance(false).bordered(false).into_any_element(), 100.))
                .when(!tags.is_empty(), |d| {
                    d.child(div().text_size(z(12.)).font_weight(FontWeight::BOLD).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("forum.tags"))).child(tag_row)
                })
                .when_some(comp.error.clone(), |d, er| d.child(div().text_size(z(14.)).text_color(c(Palette::TEXT_CRITICAL)).child(er))),
        )
        .child(
            div()
                .p(z(16.))
                .bg(c(Palette::BG_SECONDARY))
                .flex()
                .justify_end()
                .items_center()
                .gap(z(16.))
                .child(
                    div()
                        .id("compose-cancel")
                        .cursor_pointer()
                        .text_size(z(14.))
                        .text_color(c(Palette::TEXT_NORMAL))
                        .on_click(move |_, w, cx| close2(w, cx))
                        .child(crate::t!("common.cancel")),
                )
                .child(
                    div()
                        .id("compose-post")
                        .h(z(32.))
                        .px(z(16.))
                        .rounded(z(8.))
                        .bg(c(Palette::BLURPLE))
                        .flex()
                        .items_center()
                        .text_size(z(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(gpui::white())
                        .when(!comp.sending, |d| d.cursor_pointer().on_click(cx.listener(move |a, _, _, cx| a.submit_post(forum, cx))))
                        .when(comp.sending, |d| d.opacity(0.5))
                        .child(if comp.sending { crate::t!("forum.posting") } else { crate::t!("forum.post") }),
                ),
        );
    Some(crate::ui::modal::frame("forum-compose", close, card))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_times() {
        assert_eq!(ago(1000, 1000), "now");
        assert_eq!(ago(4000, 400), "1h ago");
        assert_eq!(ago(200_000, 0), "2d ago");
    }

    #[test]
    fn preview_strips_block_markdown() {
        assert_eq!(preview_text("-# oui je vais\n> en cours"), "oui je vais en cours");
    }

    #[test]
    fn snowflake_seconds() {
        // 175928847299117063 = 2016-04-30T11:18:25.796Z
        assert_eq!(snowflake_time(Id(175928847299117063)), 1462015105);
    }
}

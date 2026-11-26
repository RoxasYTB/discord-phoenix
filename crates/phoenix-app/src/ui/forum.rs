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
    ui::{context_menu::MenuItem, icon},
};

thread_local! {
    static SEARCH: std::cell::RefCell<Option<(Id, gpui::Entity<InputState>)>> = const { std::cell::RefCell::new(None) };
}

thread_local! {
    static LIST_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

thread_local! {
    static GUIDE_HEIGHT: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
    static GRID_WIDTH: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
}

thread_local! {
    static SORT_ANCHOR: std::cell::Cell<gpui::Point<gpui::Pixels>> = const { std::cell::Cell::new(gpui::point(gpui::px(330.), gpui::px(170.))) };
    static TAGS_ANCHOR: std::cell::Cell<gpui::Point<gpui::Pixels>> = const { std::cell::Cell::new(gpui::point(gpui::px(900.), gpui::px(170.))) };
}

fn menu_group_label(t: SharedString) -> AnyElement {
    div().px(z(8.)).pt(z(6.)).pb(z(2.)).text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_MUTED))).child(t).into_any_element()
}

fn search_input(forum: Id) -> Option<gpui::Entity<InputState>> {
    SEARCH.with(|s| s.borrow().as_ref().filter(|(f, _)| *f == forum).map(|(_, e)| e.clone()))
}

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

pub fn apply_defaults(v: &mut crate::commands::state::ForumView, d: &crate::commands::ForumSettings) {
    v.sort = if d.sort_created { ForumSort::Created } else { ForumSort::Recent };
    v.gallery = d.gallery;
    v.match_all = d.match_all;
}

pub fn toggle_tag(applied: &[Id], tag: Id) -> Vec<Id> {
    if applied.contains(&tag) {
        return applied.iter().copied().filter(|t| *t != tag).collect();
    }
    let mut v = applied.to_vec();
    if v.len() < 5 {
        v.push(tag);
    }
    v
}

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
        if let Ok(ParityReply::Thread(ch)) = r {
            a.channels.entry(thread).or_insert(ch);
            a.select_channel(thread, cx);
        }
    });
}

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

pub fn follow_button(channel: Id) -> gpui::Stateful<gpui::Div> {
    div().id("announcement-follow").flex_none().py(z(4.)).px(z(8.)).mr(z(8.)).on_click(move |_, w, _| crate::ui::modals::follow::open(channel, w)).child(
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
        .text_size(z(12.8))
        .line_height(z(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(tok(idx::BRAND_560)))
        .child(crate::t!("forum.op"))
}

fn snowflake_time(id: Id) -> i64 {
    (((id.0 >> 22) + 1_420_070_400_000) / 1000) as i64
}

pub fn ago(now: i64, ts: i64) -> String {
    crate::i18n::ago_short(now - ts).to_string()
}

const FORUM_AUTO_RETRIES: u32 = 3;
pub(crate) const MAX_POST_FILES: usize = 10;

pub(crate) fn add_files(files: &mut Vec<std::path::PathBuf>, picked: Vec<std::path::PathBuf>) {
    for p in picked {
        if files.len() >= MAX_POST_FILES {
            break;
        }
        if !files.contains(&p) {
            files.push(p);
        }
    }
}

fn compose_body(app: &PhoenixApp, forum: Id) -> Option<gpui::Entity<TextareaState>> {
    app.parity.forums.get(&forum)?.compose.as_ref().map(|c| c.body.clone())
}

pub fn focus_compose_body(app: &mut PhoenixApp, forum: Id, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if let Some(b) = compose_body(app, forum) {
        b.update(cx, |s, cx| s.focus(window, cx));
    }
}

pub fn insert_compose_body(app: &mut PhoenixApp, forum: Id, text: String, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if let Some(b) = compose_body(app, forum) {
        b.update(cx, |s, cx| s.insert(text, window, cx));
    }
    cx.notify();
}

impl PhoenixApp {
    fn load_forum(&mut self, forum: Id, fresh: bool, cx: &mut Context<Self>) {
        let defaults = self.backend.forum_settings(forum);
        let v = self.parity.forums.entry(forum).or_default();
        if !v.defaults_applied {
            v.defaults_applied = true;
            if let Some(d) = defaults {
                apply_defaults(v, &d);
            }
        }
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
        let generation = v.generation;
        let req = ParityRequest::ForumPosts {
            forum,
            tags: v.tags.clone(),
            offset: v.posts.len() as u32,
            by_creation: v.sort == ForumSort::Created,
            archived: false,
            match_all: v.match_all,
        };
        self.parity_request(req, cx, move |a, res, cx| {
            let waiting_for_session = a.logging_in;
            let v = a.parity.forums.entry(forum).or_default();
            if v.generation != generation {
                return;
            }
            if res.is_err() && waiting_for_session {
                return;
            }
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

    fn load_archived(&mut self, forum: Id, cx: &mut Context<Self>) {
        let v = self.parity.forums.entry(forum).or_default();
        if v.archived_loading || (v.archived_loaded && !v.archived_more) {
            return;
        }
        v.archived_loading = true;
        let generation = v.generation;
        let req = ParityRequest::ForumPosts {
            forum,
            tags: v.tags.clone(),
            offset: v.archived.len() as u32,
            by_creation: v.sort == ForumSort::Created,
            archived: true,
            match_all: v.match_all,
        };
        self.parity_request(req, cx, move |a, res, cx| {
            let v = a.parity.forums.entry(forum).or_default();
            if v.generation != generation {
                return;
            }
            v.archived_loading = false;
            v.archived_loaded = true;
            if let Ok(ParityReply::ForumPosts { posts, has_more }) = res {
                v.archived_more = has_more && !posts.is_empty();
                v.archived.extend(posts);
            }
            cx.notify();
        });
    }

    pub(crate) fn forums_after_resync(&mut self) {
        for v in self.parity.forums.values_mut() {
            v.generation += 1;
            v.loading = false;
            v.loaded = false;
            v.archived_loading = false;
            v.archived_loaded = false;
            v.archived_more = false;
            v.error = None;
            v.failures = 0;
        }
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

    fn set_forum_layout(&mut self, forum: Id, gallery: bool, cx: &mut Context<Self>) {
        let v = self.parity.forums.entry(forum).or_default();
        v.gallery = gallery;
        v.sort_menu = false;
        cx.notify();
    }

    fn set_forum_match(&mut self, forum: Id, all: bool, cx: &mut Context<Self>) {
        let v = self.parity.forums.entry(forum).or_default();
        v.sort_menu = false;
        if v.match_all == all {
            cx.notify();
            return;
        }
        v.match_all = all;
        let reload = !v.tags.is_empty();
        if reload {
            self.load_forum(forum, true, cx);
        }
        cx.notify();
    }

    fn reset_forum_view(&mut self, forum: Id, cx: &mut Context<Self>) {
        let d = self.backend.forum_settings(forum).unwrap_or_default();
        let v = self.parity.forums.entry(forum).or_default();
        let (sort, all) = (v.sort, v.match_all);
        apply_defaults(v, &d);
        v.sort_menu = false;
        if v.sort != sort || (v.match_all != all && !v.tags.is_empty()) {
            self.load_forum(forum, true, cx);
        }
        cx.notify();
    }

    fn clear_forum_tags(&mut self, forum: Id, cx: &mut Context<Self>) {
        let v = self.parity.forums.entry(forum).or_default();
        v.tags_popout = false;
        if v.tags.is_empty() {
            cx.notify();
            return;
        }
        v.tags.clear();
        self.load_forum(forum, true, cx);
        cx.notify();
    }

    fn pick_compose_files(&mut self, forum: Id, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                let _ = this.update(cx, |a, cx| {
                    if let Some(cmp) = a.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut()) {
                        add_files(&mut cmp.files, paths);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn submit_post(&mut self, forum: Id, cx: &mut Context<Self>) {
        let Some(comp) = self.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut()) else { return };
        let title = comp.title.read(cx).value().trim().to_owned();
        let content = comp.body.read(cx).value().trim().to_owned();
        let files = comp.files.clone();
        if title.is_empty() || (content.is_empty() && files.is_empty()) {
            comp.error = Some(crate::t!("forum.needs_title"));
            cx.notify();
            return;
        }
        let tags = comp.tags.clone();
        if !self.backend.forum_settings(forum).unwrap_or_default().can_post(&tags) {
            if let Some(comp) = self.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut()) {
                comp.error = Some(crate::t!("forum.needs_tag"));
            }
            cx.notify();
            return;
        }
        let Some(comp) = self.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut()) else { return };
        comp.sending = true;
        self.parity_request(ParityRequest::CreatePost { forum, title: title.clone(), content, tags, files }, cx, move |a, res, cx| {
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

pub fn prepare(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if let Some(t) = app.parity.forum_open_thread.take() {
        app.open_thread(t, window, cx);
    }
    let Some(forum) = open_forum(app) else { return };
    if search_input(forum).is_none() {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("forum.search")));
        cx.subscribe(&input, move |a: &mut PhoenixApp, _, ev: &gpui_component::input::InputEvent, cx| match ev {
            gpui_component::input::InputEvent::Change => cx.notify(),
            gpui_component::input::InputEvent::PressEnter { shift: true, .. } => {
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
        let v = app.parity.forums.entry(forum).or_default();
        v.compose = Some(ForumCompose { title, body, tags: Vec::new(), files: Vec::new(), media_hover: false, sending: false, error: None });
        if !v.guidelines_seen {
            v.guidelines_seen = true;
            v.guidelines_open = true;
        }
    }
}

fn guide_button(forum: Id, id: &'static str, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    use crate::ui::tooltip::Tipped as _;
    div()
        .id(id)
        .size(z(32.))
        .flex_none()
        .rounded(z(8.))
        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
        .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))).border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_HOVER))))
        .active(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE))).border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_ACTIVE))))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_click(cx.listener(move |a, _, _, cx| {
            let v = a.parity.forums.entry(forum).or_default();
            v.guidelines_open = !v.guidelines_open;
            v.guidelines_seen = true;
            cx.notify();
        }))
        .child(gpui::svg().path(crate::icons::path("forum/book-check")).size(z(16.)).flex_none().text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
        .tip(crate::t!("forum.guide"))
}

fn guidelines(app: &PhoenixApp, forum: Id, text: &str, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let manage = app.selected_guild.is_some_and(|g| crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 4));
    let small_icon = |id: &'static str, path: &'static str| {
        div().id(id).group(id).flex().cursor_pointer().child(
            gpui::svg()
                .path(crate::icons::path(path))
                .size(z(16.))
                .flex_none()
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover(id, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
                .id("i")
                .group_active(id, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))),
        )
    };
    let tall = GUIDE_HEIGHT.with(|h| h.get()) >= f32::from(z(220.)) - 0.5;
    let probe = gpui::canvas(
        |b, w, _| {
            let h = f32::from(b.size.height);
            if GUIDE_HEIGHT.with(|g| g.replace(h)) != h {
                w.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .text_size(z(20.))
                .line_height(z(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(gpui::svg().path(crate::icons::path("forum/book-check")).size(z(20.)).flex_none().text_color(c(tok(idx::TEXT_STRONG))))
                .child(crate::t!("forum.guide"))
                .when(manage, |d| {
                    d.child(small_icon("forum-guide-edit", "pen").on_click(cx.listener(move |a, _, _, cx| a.open_channel_settings(forum, cx))))
                }),
        )
        .child(small_icon("forum-guide-close", "close-small").on_click(cx.listener(move |a, _, _, cx| {
            if let Some(v) = a.parity.forums.get_mut(&forum) {
                v.guidelines_open = false;
            }
            cx.notify();
        })));
    let body = div()
        .relative()
        .flex_none()
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(crate::ui::components::markdown_body_sm(app, text))
        .child(probe);
    let more = tall.then(|| {
        div()
            .absolute()
            .bottom_0()
            .right_0()
            .w_full()
            .child(div().h(z(28.)).bg(gpui::linear_gradient(
                0.,
                gpui::linear_color_stop(c(tok(idx::BACKGROUND_BASE_LOWER)), 0.),
                gpui::linear_color_stop(c(tok(idx::BACKGROUND_BASE_LOWER)).opacity(0.), 1.),
            )))
            .child(
                div().w_full().flex().justify_end().bg(c(tok(idx::BACKGROUND_BASE_LOWER))).child(
                    div()
                        .id("forum-guide-more")
                        .cursor_pointer()
                        .pt(z(4.))
                        .pb(z(16.))
                        .pr(z(16.))
                        .flex()
                        .items_center()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_BRAND)))
                        .on_click(cx.listener(move |a, _, _, cx| {
                            if let Some(v) = a.parity.forums.get_mut(&forum) {
                                v.guidelines_modal = true;
                            }
                            cx.notify();
                        }))
                        .child(crate::t!("forum.guidelines_all"))
                        .child(gpui::svg().path(crate::icons::path("maximize")).size(z(16.)).ml(z(8.)).flex_none().text_color(c(tok(idx::TEXT_BRAND)))),
                ),
            )
    });
    div()
        .mt(z(16.))
        .p(z(12.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .child(header)
        .child(
            div()
                .relative()
                .mt(z(12.))
                .p(z(16.))
                .rounded(z(8.))
                .overflow_hidden()
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .child(div().max_h(z(220.)).overflow_hidden().child(body))
                .children(more),
        )
}

fn tag_pill(t: &ForumTag, on: bool) -> gpui::Div {
    tag_pill_sized(t, on, false)
}

fn tag_pill_sized(t: &ForumTag, on: bool, small: bool) -> gpui::Div {
    div()
        .h(z(if small { 24. } else { 32. }))
        .px(z(if small { 8. } else { 12. }))
        .rounded(z(20.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .flex()
        .flex_none()
        .items_center()
        .overflow_hidden()
        .text_size(z(if small { 12. } else { 14. }))
        .line_height(z(if small { 16. } else { 18. }))
        .font_weight(FontWeight::SEMIBOLD)
        .when(on, |d| d.border_color(c(tok(idx::BRAND_500))))
        .bg(c(tok(match (on, crate::theme::is_light()) {
            (true, true) => idx::BRAND_160,
            (true, false) => idx::BRAND_15A,
            _ => idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT,
        })))
        .text_color(c(tok(if on { idx::REACTION_TEXT_REACTED_DEFAULT } else { idx::CONTROL_SECONDARY_TEXT_DEFAULT })))
        .when_some(t.emoji.clone(), |d, e| d.child(div().mr(z(6.)).flex_none().child(crate::media::emoji::emoji_element(&e, if small { 12. } else { 14. }))))
        .child(div().whitespace_nowrap().child(t.name.clone()))
}

fn pill_states(d: gpui::Stateful<gpui::Div>, on: bool) -> gpui::Stateful<gpui::Div> {
    if on {
        return d.hover(|s| s.bg(c(tok(idx::OPACITY_BLURPLE_32)))).active(|s| s.bg(c(tok(idx::OPACITY_BLURPLE_12))));
    }
    d.hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
        .active(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE))))
}

fn secondary_pill(d: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    d.h(z(32.))
        .flex_none()
        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
        .active(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE))))
        .cursor_pointer()
        .flex()
        .items_center()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
}

fn count_badge(n: usize) -> gpui::Div {
    div()
        .min_w(z(16.))
        .p(z(2.))
        .rounded_full()
        .bg(c(tok(idx::BRAND_500)))
        .flex()
        .items_center()
        .justify_center()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(gpui::white())
        .child(n.to_string())
}

fn primary_sm(id: &'static str, label: SharedString, enabled: bool) -> gpui::Stateful<gpui::Div> {
    use idx::*;
    div()
        .id(id)
        .h(z(32.))
        .px(z(12.))
        .flex_none()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(CONTROL_PRIMARY_BORDER_DEFAULT)))
        .bg(c(tok(CONTROL_PRIMARY_BACKGROUND_DEFAULT)))
        .flex()
        .items_center()
        .gap(z(4.))
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(CONTROL_PRIMARY_TEXT_DEFAULT)))
        .when(enabled, |d| {
            d.cursor_pointer()
                .hover(|s| s.bg(c(tok(CONTROL_PRIMARY_BACKGROUND_HOVER))).border_color(c(tok(CONTROL_PRIMARY_BORDER_HOVER))).text_color(c(tok(CONTROL_PRIMARY_TEXT_HOVER))))
                .active(|s| s.bg(c(tok(CONTROL_PRIMARY_BACKGROUND_ACTIVE))).border_color(c(tok(CONTROL_PRIMARY_BORDER_ACTIVE))).text_color(c(tok(CONTROL_PRIMARY_TEXT_ACTIVE))))
        })
        .when(!enabled, |d| d.opacity(0.5))
        .child(gpui::svg().path(crate::icons::path("chat")).size(z(16.)).flex_none().text_color(c(tok(CONTROL_PRIMARY_TEXT_DEFAULT))))
        .child(label)
}

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

pub(crate) fn is_unread(app: &PhoenixApp, p: &ForumPost) -> bool {
    if p.archived {
        return false;
    }
    match app.backend.last_read(p.thread) {
        Some(r) => p.last_message.is_some_and(|l| l.0 > r.0),
        None => open_forum(app).and_then(|f| app.backend.last_read(f)).is_some_and(|r| p.thread.0 > r.0),
    }
}

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

fn default_emoji_token(id: Option<Id>, name: &str) -> String {
    match id {
        Some(id) => format!("<:{}:{}>", if name.is_empty() { "_" } else { name }, id.0),
        None => name.to_string(),
    }
}

fn card_reaction(r: &crate::model::Reaction, count: bool, thread: Id, msg: Option<Id>, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let rc = r.clone();
    div()
        .id(if count { "card-reaction" } else { "card-default-reaction" })
        .group("card-reaction")
        .flex_none()
        .mr(z(4.))
        .rounded(z(8.))
        .border_1()
        .cursor_pointer()
        .when(r.me, |d| d.bg(c(tok(idx::REACTION_BACKGROUND_REACTED_DEFAULT))).border_color(c(tok(idx::REACTION_BORDER_REACTED_DEFAULT))))
        .when(!r.me, |d| {
            d.bg(c(tok(idx::REACTION_BACKGROUND_DEFAULT)))
                .border_color(gpui::transparent_black())
                .hover(|s| s.bg(c(tok(idx::REACTION_BACKGROUND_HOVER))).border_color(c(tok(idx::REACTION_BORDER_HOVER))))
        })
        .active(|s| s.bg(c(tok(idx::REACTION_BACKGROUND_ACTIVE))).border_color(c(tok(idx::REACTION_BORDER_ACTIVE))))
        .on_click(cx.listener(move |a, _: &gpui::ClickEvent, _, cx| {
            cx.stop_propagation();
            if let Some(m) = msg {
                a.toggle_reaction(thread, m, &rc, cx);
            }
        }))
        .child(
            div()
                .py(z(1.))
                .px(z(6.))
                .flex()
                .items_center()
                .child(div().my(z(2.)).child(crate::media::emoji::emoji_element(&r.emoji, 20.)))
                .when(count, |d| {
                    d.child(
                        div()
                            .ml(z(6.))
                            .text_size(z(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(tok(if r.me { idx::REACTION_TEXT_REACTED_DEFAULT } else { idx::TEXT_SUBTLE })))
                            .group_hover("card-reaction", |s| s.text_color(c(tok(idx::REACTION_TEXT_HOVER))))
                            .child(r.count.to_string()),
                    )
                }),
        )
}

fn post_card(app: &PhoenixApp, p: &ForumPost, tags: &[ForumTag], now: i64, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let first = p.first_message.as_ref();
    let author_id = first.map(|m| m.author).or(p.owner);
    let author = author_id.and_then(|u| app.users.get(&u));
    let guild = app.channels.get(&p.thread).and_then(|ch| ch.guild).or(app.selected_guild);
    let (author_color, author_gradient) = author_id.map_or((None, None), |u| app.author_role_style_in(guild, u));
    let preview: SharedString =
        first.map(|m| crate::ui::chat::reply_preview_text(app, &preview_text(&m.content).into())).unwrap_or_default();
    let thumb = first.and_then(|m| m.attachments.iter().find(|a| a.kind == AttachmentKind::Image)).map(|a| a.url.clone());
    let reaction = first.and_then(|m| m.reactions.iter().max_by_key(|r| r.count)).cloned();
    let first_id = first.map(|m| m.id);
    let default_reaction = first
        .filter(|m| m.reactions.is_empty())
        .and_then(|_| app.channels.get(&p.thread).and_then(|t| t.parent).or_else(|| open_forum(app)))
        .and_then(|f| app.backend.forum_settings(f))
        .and_then(|s| s.reaction)
        .map(|(id, name)| crate::model::Reaction { emoji: default_emoji_token(id, &name).into(), count: 0, me: false, emoji_id: id, ..Default::default() });
    let r = ThreadRef { id: p.thread, name: p.title.clone().into(), message_count: p.message_count };
    let unread = is_unread(app, p);
    let title_color = if unread { c(Palette::TEXT_HEADER) } else { c(Palette::TEXT_MUTED) };
    let preview_color = if unread { c(Palette::TEXT_NORMAL) } else { c(Palette::TEXT_MUTED) };
    let post_tags: Vec<&ForumTag> = p.tags.iter().filter_map(|t| tags.iter().find(|x| x.id == *t)).collect();
    div().pb(z(16.)).child(
        div()
            .id(("forum-post", p.thread.0))
            .py(z(12.))
            .px(z(10.))
            .rounded(z(12.))
            .bg(c(Palette::BG_BASE_LOW))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .cursor_pointer()
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
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .when(p.pinned || !post_tags.is_empty(), |d| {
                        d.child(
                            div()
                                .h(z(24.))
                                .mb(z(8.))
                                .flex()
                                .items_center()
                                .gap(z(8.))
                                .overflow_hidden()
                                .when(p.pinned, |d| {
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
                        div()
                            .pb(z(6.))
                            .text_size(z(20.))
                            .line_height(z(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(title_color)
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(SharedString::from(p.title.clone())),
                    )
                    .child(
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
                                .child(crate::fonts::role_name_text(author_name, author, 14., guild.is_some(), author_gradient, false))
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
                        div()
                            .mt(z(8.))
                            .h(z(20.))
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(c(Palette::TEXT_NORMAL))
                            .when_some(default_reaction, |d, re| d.child(card_reaction(&re, false, p.thread, first_id, cx)))
                            .when_some(reaction, |d, re| d.child(card_reaction(&re, true, p.thread, first_id, cx)))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(z(4.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(gpui::svg().path(crate::icons::path("chat")).size(z(14.)).flex_none().text_color(c(Palette::TEXT_NORMAL)))
                                    .child(p.message_count.to_string()),
                            )
                            .child(div().text_color(c(tok(idx::BORDER_SUBTLE))).child("\u{2022}"))
                            .child(footer_time(now, p)),
                    ),
            )
            .when_some(thumb, |d, u| d.child(img(u).ml(z(12.)).size(z(72.)).flex_none().rounded(z(8.)).object_fit(gpui::ObjectFit::Cover))),
    )
    .into_any_element()
}

pub(crate) fn grid_columns(width: f32) -> (usize, f32) {
    let n = if width <= 320. {
        1
    } else {
        let t = width / 450.;
        ((t + (width / 320. - t) / 2.).floor() as usize).max(1)
    };
    (n, ((width - 48. - 16. * (n as f32 - 1.)) / n as f32).max(0.))
}

pub(crate) const THREE_BY_TWO: f32 = 2. / 3.;
pub(crate) const SIXTEEN_BY_NINE: f32 = 0.5625;

pub(crate) fn grid_media_size(card: f32, ratio: f32) -> (f32, f32) {
    let w = card - 26.;
    (w, (w * ratio).ceil())
}

pub(crate) fn mosaic(n: usize, w: f32, h: f32) -> Vec<(f32, f32, f32, f32)> {
    let hw = ((w - 4.) / 2.).ceil();
    let hh = ((h - 4.) / 2.).ceil();
    match n {
        0 => Vec::new(),
        1 => vec![(0., 0., w, h)],
        2 => vec![(0., 0., hw, h), (hw + 4., 0., hw, h)],
        3 => vec![(0., 0., hw, h), (hw + 4., 0., hw, hh), (hw + 4., hh + 4., hw, hh)],
        _ => vec![(0., 0., hw, hh), (0., hh + 4., hw, hh), (hw + 4., 0., hw, hh), (hw + 4., hh + 4., hw, hh)],
    }
}

fn media_mosaic(urls: &[SharedString], total: usize, w: f32, h: f32, keep_ratio: bool) -> gpui::Div {
    let frames = mosaic(urls.len().min(4), w, h);
    div()
        .relative()
        .ml(z(12.))
        .w(z(w))
        .h(z(h))
        .rounded(z(8.))
        .border_1()
        .border_color(gpui::transparent_black())
        .overflow_hidden()
        .children(urls.iter().zip(frames).map(|(u, (l, t, fw, fh))| {
            div()
                .absolute()
                .left(z(l))
                .top(z(t))
                .w(z(fw))
                .h(z(fh))
                .rounded(z(2.))
                .overflow_hidden()
                .when(keep_ratio, |d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .child(img(u.clone()).size_full().object_fit(if keep_ratio { gpui::ObjectFit::Contain } else { gpui::ObjectFit::Cover }))
        }))
        .when(total > 4, |d| {
            d.child(
                div()
                    .absolute()
                    .bottom(z(6.))
                    .right(z(6.))
                    .h(z(24.))
                    .px(z(4.))
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .rounded(z(4.))
                    .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                    .shadow(crate::ui::inbox::low_shadow())
                    .child(gpui::svg().path(crate::icons::path("image")).size(z(16.)).flex_none().text_color(c(tok(idx::TEXT_DEFAULT))))
                    .child(
                        div()
                            .text_size(z(12.))
                            .line_height(z(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(tok(idx::TEXT_DEFAULT)))
                            .child(format!("+{}", total - 4)),
                    ),
            )
        })
}

fn grid_card(app: &PhoenixApp, p: &ForumPost, tags: &[ForumTag], now: i64, width: f32, media: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let first = p.first_message.as_ref();
    let author_id = first.map(|m| m.author).or(p.owner);
    let author = author_id.and_then(|u| app.users.get(&u));
    let guild = app.channels.get(&p.thread).and_then(|ch| ch.guild).or(app.selected_guild);
    let (author_color, author_gradient) = author_id.map_or((None, None), |u| app.author_role_style_in(guild, u));
    let images: Vec<SharedString> = first.map(|m| m.attachments.iter().filter(|a| a.kind == AttachmentKind::Image).map(|a| a.url.clone()).collect()).unwrap_or_default();
    let media_total = first.map_or(0, |m| m.attachments.iter().filter(|a| matches!(a.kind, AttachmentKind::Image | AttachmentKind::Video)).count());
    let reaction = first.and_then(|m| m.reactions.iter().max_by_key(|r| r.count)).cloned();
    let first_id = first.map(|m| m.id);
    let default_reaction = first
        .filter(|m| m.reactions.is_empty())
        .and_then(|_| app.channels.get(&p.thread).and_then(|t| t.parent).or_else(|| open_forum(app)))
        .and_then(|f| app.backend.forum_settings(f))
        .and_then(|s| s.reaction)
        .map(|(id, name)| crate::model::Reaction { emoji: default_emoji_token(id, &name).into(), count: 0, me: false, emoji_id: id, ..Default::default() });
    let r = ThreadRef { id: p.thread, name: p.title.clone().into(), message_count: p.message_count };
    let unread = is_unread(app, p);
    let post_tags: Vec<&ForumTag> = p.tags.iter().filter_map(|t| tags.iter().find(|x| x.id == *t)).collect();
    let has_tags = p.pinned || !post_tags.is_empty();
    let (mw, mh) = grid_media_size(width, if media { SIXTEEN_BY_NINE } else { THREE_BY_TWO });
    let tag_row = {
        let shadow = crate::ui::inbox::low_shadow();
        div()
            .flex()
            .items_center()
            .gap(z(4.))
            .when(p.pinned, |d| {
                d.child(
                    div()
                        .size(z(24.))
                        .flex_none()
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                        .shadow(shadow.clone())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(Icon::Pin, 14., c(tok(idx::TEXT_DEFAULT)))),
                )
            })
            .children(post_tags.iter().take(3).map(|t| card_tag(t).bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))).shadow(shadow.clone())))
            .when(post_tags.len() > 3, |d| {
                d.child(
                    card_tag(&ForumTag { id: Id(0), name: format!("+{}", post_tags.len() - 3).into(), emoji: None, moderated: false })
                        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                        .shadow(shadow.clone()),
                )
            })
    };
    let body = if !images.is_empty() {
        div()
            .relative()
            .child(media_mosaic(&images[..images.len().min(4)], media_total, mw, mh, media))
            .when(has_tags, |d| d.child(div().absolute().bottom(z(-2.)).left(z(18.)).child(tag_row)))
    } else {
        let preview: SharedString =
            first.map(|m| crate::ui::chat::reply_preview_text(app, &preview_text(&m.content).into())).unwrap_or_default();
        div().relative().child(
            div()
                .relative()
                .mx(z(12.))
                .w(z(mw))
                .h(z(mh))
                .pt(z(12.))
                .px(z(12.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .overflow_hidden()
                .child(
                    div()
                        .overflow_hidden()
                        .text_size(z(16.))
                        .line_height(z(22.))
                        .text_color(c(tok(if unread { idx::TEXT_STRONG } else { idx::TEXT_MUTED })))
                        .child(preview),
                )
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(gpui::relative(if has_tags { 0.5 } else { 0.1 }))
                        .px(z(12.))
                        .pb(z(8.))
                        .flex()
                        .items_end()
                        .bg(gpui::linear_gradient(
                            0.,
                            gpui::linear_color_stop(c(tok(idx::BACKGROUND_BASE_LOW)), 0.2),
                            gpui::linear_color_stop(c(tok(idx::BACKGROUND_BASE_LOW)).opacity(0.), 1.),
                        ))
                        .when(has_tags, |d| d.child(tag_row)),
                ),
        )
    };
    let author_name: SharedString = author.map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("forum.deleted_user"));
    div()
        .id(("forum-grid-post", p.thread.0))
        .w(z(width))
        .h(z(mh + 120.))
        .flex_none()
        .flex()
        .flex_col()
        .justify_center()
        .pb(z(16.))
        .overflow_hidden()
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
        .border_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .cursor_pointer()
        .hover(|s| {
            s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER)))
                .border_color(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER)))
                .shadow(crate::ui::inbox::popout_shadow())
        })
        .active(|s| {
            s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                .border_color(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE)))
                .shadow(crate::ui::inbox::popout_shadow())
        })
        .on_click(cx.listener(move |a, _, w, cx| a.open_thread(r.clone(), w, cx)))
        .when_some(open_forum(app), |d, forum| {
            let post = p.clone();
            d.on_mouse_down(gpui::MouseButton::Right, move |ev, w, _| crate::ui::thread_menu::open_post(post.clone(), forum, ev.position, w))
        })
        .child(
            div()
                .mt(z(12.))
                .mx(z(12.))
                .mb(z(10.))
                .flex()
                .flex_col()
                .child(
                    div().pt(z(12.)).flex().justify_between().child(
                        div()
                            .flex()
                            .gap(z(8.))
                            .min_w_0()
                            .child(
                                crate::fonts::styled_name(
                                    div()
                                        .flex_none()
                                        .text_size(z(14.))
                                        .line_height(z(18.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(author_color.map(c).unwrap_or(c(tok(idx::TEXT_STRONG)))),
                                    author,
                                    false,
                                )
                                .child(crate::fonts::role_name_text(author_name, author, 14., guild.is_some(), author_gradient, false)),
                            )
                            .child(
                                div()
                                    .pt(z(1.))
                                    .text_size(z(12.))
                                    .line_height(z(18.))
                                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                                    .child(ago(now, snowflake_time(p.thread))),
                            ),
                    ),
                )
                .child(
                    div()
                        .mt(z(4.))
                        .w(z(width - 24.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(if unread { idx::TEXT_STRONG } else { idx::TEXT_MUTED })))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(SharedString::from(p.title.clone())),
                ),
        )
        .child(body)
        .child(
            div().w_full().p(z(12.)).flex().justify_between().child(
                div()
                    .h(z(24.))
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .child(gpui::svg().path(crate::icons::path("chat")).size(z(16.)).flex_none().text_color(c(tok(idx::TEXT_DEFAULT))))
                    .child(p.message_count.to_string()),
            )
            .child(
                div()
                    .h(z(24.))
                    .flex()
                    .items_center()
                    .when_some(default_reaction, |d, re| d.child(card_reaction(&re, false, p.thread, first_id, cx)))
                    .when_some(reaction, |d, re| d.child(card_reaction(&re, true, p.thread, first_id, cx))),
            ),
        )
        .into_any_element()
}

fn compose_ready(app: &PhoenixApp, forum: Id, comp: &ForumCompose, cx: &gpui::App) -> (bool, bool) {
    let title = !comp.title.read(cx).value().trim().is_empty();
    let content = !comp.body.read(cx).value().trim().is_empty() || !comp.files.is_empty();
    let needs_tag = !app.backend.forum_settings(forum).unwrap_or_default().can_post(&comp.tags);
    (title && content && !needs_tag, needs_tag && title && content)
}

fn upload_tile(forum: Id, comp: &ForumCompose, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let add = |id: &'static str, cx: &mut Context<PhoenixApp>| {
        div()
            .id(id)
            .group(id)
            .size(z(78.))
            .flex_none()
            .rounded(z(12.))
            .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |a, _, _, cx| a.pick_compose_files(forum, cx)))
            .child(
                gpui::svg()
                    .path(crate::icons::path("image-plus"))
                    .size(z(28.))
                    .flex_none()
                    .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                    .group_hover(id, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
                    .id("i")
                    .group_active(id, |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))),
            )
            .tip(crate::t!("forum.add_media"))
    };
    use crate::ui::tooltip::Tipped as _;
    let frame = div().id("compose-media").relative().size(z(78.)).flex_none();
    if comp.files.is_empty() {
        return frame.child(add("compose-media-add", cx));
    }
    let thumb = |p: &std::path::Path| {
        let image = crate::ui::file_pick::accepts(p, &crate::ui::file_pick::IMAGES);
        div()
            .size(z(78.))
            .flex_none()
            .rounded(z(12.))
            .overflow_hidden()
            .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
            .flex()
            .items_center()
            .justify_center()
            .when(image, |d| d.child(img(p.to_path_buf()).size_full().object_fit(gpui::ObjectFit::Cover)))
            .when(!image, |d| d.child(gpui::svg().path(crate::icons::path("file")).size(z(28.)).flex_none().text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))))
    };
    let collapsed = div()
        .relative()
        .size_full()
        .cursor_pointer()
        .child(thumb(&comp.files[0]))
        .child(
            div()
                .absolute()
                .top(z(-4.))
                .right(z(-4.))
                .h(z(16.))
                .min_w(z(8.))
                .px(z(4.))
                .rounded(z(16.))
                .border_2()
                .border_color(c(tok(idx::BACKGROUND_BASE_LOW)))
                .bg(c(tok(idx::BRAND_500)))
                .flex()
                .items_center()
                .justify_center()
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_OVERLAY_LIGHT)))
                .child(comp.files.len().to_string()),
        );
    let popout = comp.media_hover.then(|| {
        let mut row = div().flex().gap(z(8.));
        for (i, f) in comp.files.iter().enumerate() {
            row = row.child(
                div().relative().child(thumb(f)).child(
                    div()
                        .id(("compose-media-remove", i))
                        .absolute()
                        .top(z(4.))
                        .right(z(4.))
                        .size(z(24.))
                        .rounded(z(6.))
                        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                        .hover(|s| s.bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER))))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(cx.listener(move |a, _, _, cx| {
                            cx.stop_propagation();
                            if let Some(cmp) = a.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut())
                                && i < cmp.files.len()
                            {
                                cmp.files.remove(i);
                                cmp.media_hover = !cmp.files.is_empty();
                            }
                            cx.notify();
                        }))
                        .child(gpui::svg().path(crate::icons::path("chat/trash")).size(z(16.)).flex_none().text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL)))),
                ),
            );
        }
        if comp.files.len() < 10 {
            row = row.child(add("compose-media-more", cx));
        }
        div().absolute().top(z(-24.)).right(z(-24.)).p(z(16.)).child(
            div()
                .p(z(8.))
                .rounded(z(16.))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .shadow(crate::ui::inbox::shadow_medium())
                .child(row),
        )
    });
    frame
        .on_hover(cx.listener(move |a, h: &bool, _, cx| {
            if let Some(cmp) = a.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut())
                && cmp.media_hover != *h
            {
                cmp.media_hover = *h && !cmp.files.is_empty();
                cx.notify();
            }
        }))
        .child(collapsed)
        .children(popout)
}

fn compose_card(app: &PhoenixApp, forum: Id, comp: &ForumCompose, tags: &[ForumTag], has_guide: bool, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let close = cx.listener(move |a: &mut PhoenixApp, _: &gpui::ClickEvent, _, cx| {
        if let Some(v) = a.parity.forums.get_mut(&forum) {
            v.compose = None;
        }
        cx.notify();
    });
    let (ready, tag_missing) = compose_ready(app, forum, comp, cx);
    let full = comp.tags.len() >= 5;
    let moderator = app.selected_guild.is_some_and(|g| crate::ui::inbox::has_perm(app.backend.guild_permissions(g), 1 << 34));
    let usable: Vec<&ForumTag> = tags.iter().filter(|t| moderator || !t.moderated).collect();
    let mut tag_list = div().flex().flex_wrap().gap(z(4.));
    for t in usable.iter().copied() {
        let on = comp.tags.contains(&t.id);
        let id = t.id;
        let disabled = full && !on;
        let pill = tag_pill_sized(t, on, true).id(("compose-tag", id.0));
        tag_list = tag_list.child(if disabled {
            pill.opacity(0.5).cursor_not_allowed()
        } else {
            pill_states(pill, on).cursor_pointer().on_click(cx.listener(move |a, _, _, cx| {
                if let Some(cmp) = a.parity.forums.get_mut(&forum).and_then(|v| v.compose.as_mut()) {
                    cmp.tags = toggle_tag(&cmp.tags, id);
                }
                cx.notify();
            }))
        });
    }
    let emoji_open = matches!(crate::ui::pickers::open_tab(cx), Some((_, crate::ui::pickers::EmojiTarget::ForumPost(f))) if f == forum);
    div()
        .mt(z(24.))
        .rounded(z(12.))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .py(z(8.))
        .flex()
        .flex_col()
        .child(
            div()
                .pl(z(12.))
                .pr(z(8.))
                .flex()
                .items_start()
                .child(
                    div()
                        .id("compose-close")
                        .group("compose-close")
                        .w(z(26.))
                        .h(z(32.))
                        .mr(z(8.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .on_click(close)
                        .child(
                            gpui::svg()
                                .path(crate::icons::path("close"))
                                .size(z(26.))
                                .flex_none()
                                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                                .group_hover("compose-close", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .h(z(32.))
                                .pt(z(4.))
                                .pr(z(12.))
                                .text_size(z(20.))
                                .line_height(z(24.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(tok(idx::TEXT_STRONG)))
                                .child(Input::new(&comp.title).appearance(false).bordered(false).text_size(z(20.)).line_height(z(24.))),
                        )
                        .child(
                            div()
                                .min_h(z(80.))
                                .text_size(z(16.))
                                .line_height(z(22.))
                                .child(Textarea::new(&comp.body).appearance(false).bordered(false)),
                        ),
                )
                .child(upload_tile(forum, comp, cx)),
        )
        .when_some(comp.error.clone(), |d, er| {
            d.child(div().px(z(46.)).pt(z(4.)).text_size(z(14.)).text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child(er))
        })
        .child(
            div().px(z(8.)).when(!usable.is_empty(), |d| {
                d.child(
                    div()
                        .mt(z(16.))
                        .mb(z(12.))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .child(gpui::svg().path(crate::icons::path("forum/tag")).size(z(16.)).flex_none().text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                        .child(tag_list),
                )
            }),
        )
        .child(div().h(px(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(
            div()
                .px(z(8.))
                .mt(z(12.))
                .h(z(40.))
                .flex()
                .items_center()
                .gap(z(8.))
                .child(
                    crate::ui::states::composer_button_full("forum-compose-emoji", Icon::Emoji, 18., true, emoji_open)
                        .when(emoji_open, |d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                        .on_click(cx.listener(move |a, _, w, cx| crate::ui::pickers::toggle(a, crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::ForumPost(forum), w, cx))),
                )
                .child(div().flex_1())
                .when(has_guide, |d| d.child(guide_button(forum, "compose-guide", cx)))
                .child(
                    primary_sm("compose-post", if comp.sending { crate::t!("forum.posting") } else { crate::t!("forum.post") }, ready && !comp.sending)
                        .when(ready && !comp.sending, |d| d.on_click(cx.listener(move |a, _, _, cx| a.submit_post(forum, cx)))),
                ),
        )
        .when(tag_missing, |d| {
            d.child(
                div()
                    .px(z(8.))
                    .pt(z(8.))
                    .flex()
                    .justify_end()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL)))
                    .child(crate::t!("forum.needs_tag")),
            )
        })
}

pub fn loading_rows(view_h: f32) -> usize {
    ((view_h - 200.) / 118.).round().max(0.) as usize
}

pub const LOADING_OPACITY: [f32; 3] = [0.33, 1., 0.66];

fn loading_card(row: usize) -> AnyElement {
    div()
        .flex_none()
        .h(z(102.))
        .mb(z(8.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
        .opacity(LOADING_OPACITY[row % 3])
        .into_any_element()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let forum = open_forum(app)?;
    let tags = app.backend.forum_tags(forum);
    let empty = Default::default();
    let v = app.parity.forums.get(&forum).unwrap_or(&empty);
    let now = chrono::Utc::now().timestamp();
    let guide_text = app.backend.forum_settings(forum).and_then(|s| s.guidelines).filter(|t| !t.is_empty());
    let has_guide = guide_text.is_some();

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
            div()
                .flex_1()
                .min_w_0()
                .text_size(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .when_some(search_input(forum), |d, i| d.child(Input::new(&i).appearance(false).bordered(false).text_size(z(20.)).line_height(z(24.)))),
        )
        .when(has_guide, |d| d.child(guide_button(forum, "forum-guide", cx)))
        .child(div().ml(z(9.)).mr(z(8.)).child(primary_sm("forum-new-post", crate::t!("forum.new_post"), true).on_click(cx.listener(move |a, _, _, cx| {
            a.parity.forum_compose_request = Some(forum);
            cx.notify();
        }))));

    let sort_open = app.parity.forums.get(&forum).is_some_and(|v| v.sort_menu);
    let tags_open = v.tags_popout;
    let mut filters = div().pt(z(16.)).pb(z(4.)).flex().items_center().overflow_hidden().child(
        secondary_pill(div().id("forum-sort"))
            .pl(z(12.))
            .pr(z(8.))
            .rounded(z(24.))
            .on_click(cx.listener(move |a, ev: &gpui::ClickEvent, _, cx| {
                let p = ev.position();
                SORT_ANCHOR.with(|a| a.set(gpui::point(p.x - gpui::px(40.), p.y + gpui::px(20.))));
                let v = a.parity.forums.entry(forum).or_default();
                v.sort_menu = !v.sort_menu;
                v.tags_popout = false;
                cx.notify();
            }))
            .child(gpui::svg().path(crate::icons::path("sort")).size(z(16.)).flex_none().text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
            .child(div().ml(z(8.)).mr(z(4.)).child(crate::t!("forum.sort_view")))
            .child(
                gpui::svg()
                    .path(crate::icons::path(if sort_open { "chevron-up" } else { "chevron" }))
                    .w(z(20.))
                    .h(z(20.))
                    .flex_none()
                    .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))),
            ),
    );
    if !tags.is_empty() {
        filters = filters.child(div().w(z(2.)).h(z(24.)).mx(z(16.)).flex_none().rounded(z(4.)).bg(c(tok(idx::BORDER_SUBTLE))));
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
        let selected = v.tags.len();
        let probe = gpui::canvas(
            |b, _, _| TAGS_ANCHOR.with(|a| a.set(gpui::point(b.right(), b.bottom()))),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        filters = filters.child(div().min_w_0().h(z(32.)).overflow_hidden().child(list)).child(
            secondary_pill(div().id("forum-tags-all"))
                .relative()
                .ml(z(8.))
                .px(z(4.))
                .rounded(z(20.))
                .on_click(cx.listener(move |a, _, _, cx| {
                    let v = a.parity.forums.entry(forum).or_default();
                    v.tags_popout = !v.tags_popout;
                    v.sort_menu = false;
                    cx.notify();
                }))
                .child(probe)
                .child(
                    div()
                        .min_w(z(52.))
                        .when(selected == 0, |d| d.pl(z(8.)))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(z(4.))
                        .map(|d| if selected > 0 { d.child(div().ml(z(4.)).mr(z(2.)).child(count_badge(selected))) } else { d.child(crate::t!("forum.all_tags")) })
                        .child(
                            gpui::svg()
                                .path(crate::icons::path(if tags_open { "chevron-up" } else { "chevron" }))
                                .size(z(20.))
                                .flex_none()
                                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))),
                        ),
                ),
        );
    }

    let media = app.backend.is_media_channel(forum);
    let grid_view = v.gallery || media;
    let mut col = div()
        .id("forum-list")
        .track_scroll(&LIST_SCROLL.with(|h| h.clone()))
        .size_full()
        .overflow_y_scroll()
        .when(grid_view, |d| d.px(z(24.)))
        .when(!grid_view, |d| d.pl(z(16.)).pr(z(32.)))
        .flex()
        .flex_col()
        .when(grid_view, |d| {
            d.child(
                gpui::canvas(
                    |b, w, _| {
                        let width = f32::from(b.size.width) / f32::from(z(1.));
                        if (GRID_WIDTH.with(|g| g.replace(width)) - width).abs() > 0.5 {
                            w.refresh();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
        })
        .child(
            div()
                .pb(z(16.))
                .child(match v.compose.as_ref() {
                    Some(comp) => compose_card(app, forum, comp, &tags, has_guide, cx).into_any_element(),
                    None => search_bar.into_any_element(),
                })
                .when_some(guide_text.filter(|_| v.guidelines_open), |d, t| d.child(guidelines(app, forum, &t, cx)))
                .child(filters),
        );
    if let (Some(err), true) = (v.error.clone(), v.posts.is_empty() && !v.loading) {
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
                        .bg(c(Palette::BLURPLE))
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
    if grid_view {
        let measured = GRID_WIDTH.with(|g| g.get());
        let (_, cw) = grid_columns(if measured > 0. { measured } else { 939. });
        let cw = (cw - 0.05).max(0.);
        let mut grid = |posts: &[&ForumPost]| div().flex().flex_wrap().gap(z(16.)).children(posts.iter().map(|p| grid_card(app, p, &tags, now, cw, media, cx)));
        col = col.child(grid(&active));
        if !older.is_empty() {
            col = col.child(older_header()).child(grid(&older));
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
        for row in 0..loading_rows(crate::ui::title_bar::viewport_h_css()) {
            col = col.child(loading_card(row));
        }
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
        let e = cx.entity();
        let radio = |key: &'static str, on: bool, f: fn(&mut PhoenixApp, Id, &mut Context<PhoenixApp>)| {
            let e = e.clone();
            MenuItem::radio(crate::t!(key), on, move |_, _, cx: &mut gpui::App| e.update(cx, |a, cx| f(a, forum, cx)))
        };
        let mut items = vec![
            MenuItem::Custom(menu_group_label(crate::t!("forum.sort_by"))),
            radio("forum.recent", v.sort == ForumSort::Recent, |a, f, cx| a.set_forum_sort(f, ForumSort::Recent, cx)),
            radio("forum.created", v.sort == ForumSort::Created, |a, f, cx| a.set_forum_sort(f, ForumSort::Created, cx)),
        ];
        if !app.backend.is_media_channel(forum) {
            items.push(MenuItem::Separator);
            items.push(MenuItem::Custom(menu_group_label(crate::t!("forum.view_as"))));
            items.push(radio("forum.view_list", !v.gallery, |a, f, cx| a.set_forum_layout(f, false, cx)));
            items.push(radio("forum.view_gallery", v.gallery, |a, f, cx| a.set_forum_layout(f, true, cx)));
        }
        if !tags.is_empty() {
            items.push(MenuItem::Separator);
            items.push(MenuItem::Custom(menu_group_label(crate::t!("forum.tag_matching"))));
            items.push(radio("forum.match_some", !v.match_all, |a, f, cx| a.set_forum_match(f, false, cx)));
            items.push(radio("forum.match_all", v.match_all, |a, f, cx| a.set_forum_match(f, true, cx)));
        }
        items.push(MenuItem::Separator);
        let reset = e.clone();
        items.push(MenuItem::action(crate::t!("forum.reset_view"), None, move |_, _, cx: &mut gpui::App| reset.update(cx, |a, cx| a.reset_forum_view(forum, cx))));
        let dismiss: crate::ui::context_menu::Dismiss = std::rc::Rc::new(move |w, cx| {
            e.update(cx, |a, _| {
                if let Some(v) = a.parity.forums.get_mut(&forum) {
                    v.sort_menu = false;
                }
            });
            w.refresh();
        });
        let at = SORT_ANCHOR.with(|p| p.get());
        crate::ui::context_menu::render(at, items, dismiss)
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
            .children(crate::ui::chat::scrollbar::auto_scrollbar_div(&LIST_SCROLL.with(|h| h.clone())))
            .children(sort_menu)
            .children(tags_open.then(|| tags_popout(forum, &tags, &v.tags, cx)))
            .into_any_element(),
    )
}

fn tags_popout(forum: Id, tags: &[ForumTag], selected: &[Id], cx: &mut Context<PhoenixApp>) -> AnyElement {
    let at = TAGS_ANCHOR.with(|a| a.get());
    let mut list = div().flex().flex_wrap().items_center().gap(z(8.)).pb(z(16.));
    for t in tags {
        let on = selected.contains(&t.id);
        let id = t.id;
        list = list.child(
            pill_states(tag_pill(t, on).id(("forum-tags-popout", id.0)), on)
                .cursor_pointer()
                .on_click(cx.listener(move |a, _, _, cx| a.toggle_forum_tag(forum, id, cx))),
        );
    }
    let card = div()
        .id("forum-tags-popout")
        .occlude()
        .w(z(336.))
        .p(z(16.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(crate::ui::inbox::shadow_medium())
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .mb(z(8.))
                .py(z(4.))
                .px(z(8.))
                .flex()
                .items_center()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .child(crate::t!("forum.select_tags"))
                .child(div().ml(z(8.)).child(count_badge(selected.len()))),
        )
        .child(list)
        .child(div().mx(z(4.)).mb(z(8.)).h(px(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
        .child(
            div()
                .id("forum-tags-clear")
                .group("forum-tags-clear")
                .cursor_pointer()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_BRAND)))
                .on_click(cx.listener(move |a, _, _, cx| a.clear_forum_tags(forum, cx)))
                .child(div().group_hover("forum-tags-clear", |s| s.underline()).child(crate::t!("forum.clear_all"))),
        );
    gpui::deferred(
        div()
            .id("forum-tags-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |a, _, _, cx| {
                    if let Some(v) = a.parity.forums.get_mut(&forum) {
                        v.tags_popout = false;
                    }
                    cx.notify();
                }),
            )
            .child(gpui::anchored().anchor(gpui::Anchor::TopRight).position(gpui::point(at.x, at.y + z(8.))).snap_to_window_with_margin(z(8.)).child(card)),
    )
    .with_priority(10)
    .into_any_element()
}

pub fn guidelines_modal(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let forum = open_forum(app)?;
    if !app.parity.forums.get(&forum)?.guidelines_modal {
        return None;
    }
    let text = app.backend.forum_settings(forum).and_then(|s| s.guidelines).filter(|t| !t.is_empty())?;
    let e = cx.entity();
    let close: crate::ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        e.update(cx, |a, cx| {
            if let Some(v) = a.parity.forums.get_mut(&forum) {
                v.guidelines_modal = false;
            }
            cx.notify();
        })
    });
    let ok = close.clone();
    let body = div()
        .id("forum-guide-modal-body")
        .min_h_0()
        .overflow_y_scroll()
        .pb(z(16.))
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::NORMAL)
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(crate::ui::components::markdown_body_sm(app, &text));
    let footer = crate::ui::controls::button("forum-guide-ok", crate::t!("forum.got_it"), crate::ui::controls::BtnKind::Primary)
        .flex_1()
        .on_click(move |_, w, cx| ok(w, cx));
    Some(crate::ui::modals::card_with(480., crate::t!("forum.guide"), None, body.into_any_element(), Some(footer.into_any_element()), close))
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
        assert_eq!(snowflake_time(Id(175928847299117063)), 1462015105);
    }

    #[test]
    fn edit_tags_toggles_up_to_five() {
        assert_eq!(toggle_tag(&[Id(1), Id(2)], Id(1)), vec![Id(2)]);
        assert_eq!(toggle_tag(&[Id(1)], Id(3)), vec![Id(1), Id(3)]);
        let full: Vec<Id> = (1..=5).map(Id).collect();
        assert_eq!(toggle_tag(&full, Id(9)), full);
    }

    #[test]
    fn forum_defaults_follow_the_channel() {
        let raw = r#"{"id":"1","type":15,"flags":16,"default_sort_order":1,"default_forum_layout":2,"default_tag_setting":"match_all","default_reaction_emoji":{"emoji_id":null,"emoji_name":"✅"},"topic":"Règles"}"#;
        let ch: phoenix_api::models::Channel = serde_json::from_str(raw).unwrap();
        let d = crate::commands::ForumSettings::from_channel(&ch);
        assert!(d.sort_created && d.gallery && d.require_tag && d.match_all);
        assert_eq!(d.reaction, Some((None, "✅".to_owned())));
        assert_eq!(d.guidelines.as_deref(), Some("Règles"));
        assert!(!d.can_post(&[]));
        assert!(d.can_post(&[Id(3)]));
        let mut v = crate::commands::state::ForumView::default();
        apply_defaults(&mut v, &d);
        assert_eq!(v.sort, ForumSort::Created);
        assert!(v.gallery && v.match_all);
        let plain: phoenix_api::models::Channel = serde_json::from_str(r#"{"id":"2","type":15,"default_reaction_emoji":{"emoji_id":null,"emoji_name":null}}"#).unwrap();
        let d = crate::commands::ForumSettings::from_channel(&plain);
        assert_eq!(d, crate::commands::ForumSettings::default());
    }

    #[test]
    fn gallery_grid_matches_the_client_masonry() {
        assert_eq!(grid_columns(300.), (1, 252.));
        assert_eq!(grid_columns(939.), (2, 437.5));
        let (n, w) = grid_columns(1200.);
        assert_eq!(n, 3);
        assert!((w - 373.333).abs() < 0.01);
        assert_eq!(grid_media_size(437.5, THREE_BY_TWO), (411.5, 275.));
        assert_eq!(grid_media_size(437.5, SIXTEEN_BY_NINE), (411.5, 232.));
    }

    #[test]
    fn media_mosaic_follows_the_client_layout() {
        assert_eq!(mosaic(1, 400., 300.), vec![(0., 0., 400., 300.)]);
        assert_eq!(mosaic(2, 401., 300.), vec![(0., 0., 199., 300.), (203., 0., 199., 300.)]);
        assert_eq!(mosaic(3, 400., 301.)[2], (202., 153., 198., 149.));
        let four = mosaic(6, 400., 300.);
        assert_eq!(four.len(), 4);
        assert_eq!(four[1], (0., 152., 198., 148.));
    }
}

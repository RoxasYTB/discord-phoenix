use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::Arc,
};

use gpui::{
    Animation, AnimationExt, AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, img, prelude::*,
};
use gpui_component::{
    Sizable as _,
    input::{InputEvent, InputState},
};

use crate::{
    app::PhoenixApp,
    assets::Icon,
    model::{ChannelKind, Id},
    theme::{c, fz, generated::idx, tok, z},
};

pub const TITLE_MAX: usize = 140;
pub const THREAD_NAME_MAX: usize = 100;
pub const HEADER_H: f32 = 122.;
pub const HEADER_PAD: f32 = 16.;
pub const HERO: f32 = 78.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Exp {
    Unknown,
    Fetching,
    Off,
    On,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendOptions {
    pub create_thread: bool,
    pub thread_name: String,
    pub publish: bool,
}

thread_local! {
    static EXP: Cell<Exp> = const { Cell::new(Exp::Unknown) };
    static FORCED: Cell<Option<bool>> = const { Cell::new(None) };
    static TITLE: RefCell<Option<(Entity<InputState>, gpui::Subscription)>> = const { RefCell::new(None) };
    static TITLES: RefCell<HashMap<Id, String>> = RefCell::new(HashMap::new());
    static SHOWN_FOR: Cell<Option<Id>> = const { Cell::new(None) };
    static ACTIVE: RefCell<std::collections::HashSet<Id>> = RefCell::new(std::collections::HashSet::new());
    static HERO_FILE: RefCell<HashMap<Id, usize>> = RefCell::new(HashMap::new());
    static HERO_HOVER: Cell<bool> = const { Cell::new(false) };
    static NEXT: RefCell<Option<SendOptions>> = const { RefCell::new(None) };
    static ORIGINAL: RefCell<Option<String>> = const { RefCell::new(None) };
    static PENDING: RefCell<HashMap<u64, SendOptions>> = RefCell::new(HashMap::new());
    static SEND_BOX: Cell<Option<gpui::Bounds<gpui::Pixels>>> = const { Cell::new(None) };
}

pub fn force(on: bool) {
    FORCED.with(|f| f.set(Some(on)));
}

pub fn set_assignments(list: &[(u32, i64)]) {
    let on = phoenix_api::rest::experiments::bucket(list, phoenix_api::rest::experiments::ANNOUNCEMENT_COMPOSER) == 1;
    EXP.with(|e| e.set(if on { Exp::On } else { Exp::Off }));
}

pub fn enabled() -> bool {
    FORCED.with(Cell::get).unwrap_or_else(|| EXP.with(Cell::get) == Exp::On)
}

fn ensure(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if FORCED.with(Cell::get).is_some() || EXP.with(Cell::get) != Exp::Unknown {
        return;
    }
    EXP.with(|e| e.set(Exp::Fetching));
    let fut = app.backend.user_experiments();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |_, cx| {
            match res {
                Ok(list) => set_assignments(&list),
                Err(_) => EXP.with(|e| e.set(Exp::Off)),
            }
            cx.notify();
        });
    })
    .detach();
}

pub fn channel(app: &PhoenixApp) -> Option<Id> {
    let id = app.selected_channel?;
    let ch = app.channels.get(&id)?;
    (ch.kind == ChannelKind::Announcement && app.nav.thread_parent(id).is_none()).then_some(id)
}

pub fn active(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<Id> {
    let id = channel(app)?;
    ensure(app, cx);
    enabled().then_some(id)
}

pub fn is_active(app: &PhoenixApp) -> bool {
    channel(app).is_some() && enabled()
}

pub fn title(channel: Id) -> String {
    TITLES.with(|t| t.borrow().get(&channel).cloned().unwrap_or_default())
}

pub fn clamp_title(s: &str) -> String {
    s.replace(['\r', '\n'], " ").chars().take(TITLE_MAX).collect()
}

pub fn compose(title: &str, body: &str) -> String {
    if title.is_empty() { body.to_owned() } else { format!("# {title}\n{body}") }
}

pub fn thread_name(title: &str, body: &str, fallback: &str) -> String {
    let base = if !title.is_empty() {
        title
    } else if !body.is_empty() {
        body
    } else {
        fallback
    };
    base.chars().take(THREAD_NAME_MAX).collect()
}

pub fn send_disabled(body: &str, title: &str) -> bool {
    body.is_empty() && title.is_empty()
}

pub fn is_hero(channel: Option<Id>, f: &super::state::PendingFile) -> bool {
    channel.is_some_and(|ch| HERO_FILE.with(|h| h.borrow().get(&ch).copied()) == Some(Arc::as_ptr(&f.bytes) as usize))
}

fn hero(app: &PhoenixApp, channel: Id) -> Option<&super::state::PendingFile> {
    let key = HERO_FILE.with(|h| h.borrow().get(&channel).copied())?;
    app.chat.uploads.iter().find(|f| Arc::as_ptr(&f.bytes) as usize == key)
}

fn remove_hero(app: &mut PhoenixApp, channel: Id) {
    if let Some(key) = HERO_FILE.with(|h| h.borrow_mut().remove(&channel)) {
        app.chat.uploads.retain(|f| Arc::as_ptr(&f.bytes) as usize != key);
    }
}

fn prompt_hero(channel: Id, cx: &mut Context<PhoenixApp>) {
    let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some(crate::t!("announce.hero_upload")) });
    cx.spawn(async move |this, cx| {
        let Ok(Ok(Some(paths))) = rx.await else { return };
        let Some(path) = paths.into_iter().next() else { return };
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if !["jpg", "jpeg", "png", "webp", "gif"].contains(&ext.as_str()) {
            return;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "image".into());
        let Ok(bytes) = std::fs::read(&path) else { return };
        let _ = this.update(cx, |a, cx| {
            remove_hero(a, channel);
            if a.push_upload(name, bytes)
                && let Some(f) = a.chat.uploads.last()
            {
                let key = Arc::as_ptr(&f.bytes) as usize;
                HERO_FILE.with(|h| h.borrow_mut().insert(channel, key));
            }
            cx.notify();
        });
    })
    .detach();
}

pub fn enter_inserts_newline(app: &PhoenixApp) -> bool {
    is_active(app)
}

pub fn prepare_submit(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    let Some(ch) = channel(app).filter(|_| enabled()) else { return true };
    let body = app.composer.read(cx).value().to_string();
    let title = title(ch);
    if send_disabled(&body, &title) {
        return false;
    }
    let (thread, publish) = crate::ui::extra_menus::announce_flags(ch);
    let can_thread = crate::commands::builtin::Context::of(app, ch).thread;
    let opts = SendOptions { create_thread: can_thread && thread, thread_name: thread_name(&title, &body, &crate::t!("announce.thread_fallback")), publish };
    if hero(app, ch).is_some()
        && let Some(key) = HERO_FILE.with(|h| h.borrow().get(&ch).copied())
        && let Some(i) = app.chat.uploads.iter().position(|f| Arc::as_ptr(&f.bytes) as usize == key)
    {
        let f = app.chat.uploads.remove(i);
        app.chat.uploads.insert(0, f);
    }
    let text = compose(&title, &body);
    if text != body {
        app.composer.update(cx, |s, cx| s.set_value(text, window, cx));
        ORIGINAL.with(|o| *o.borrow_mut() = Some(body));
    }
    NEXT.with(|n| *n.borrow_mut() = Some(opts));
    true
}

pub fn finish_submit(app: &PhoenixApp, sent: bool, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(ch) = channel(app).filter(|_| enabled()) else { return };
    NEXT.with(|n| n.borrow_mut().take());
    let original = ORIGINAL.with(|o| o.borrow_mut().take());
    if !sent {
        if let Some(body) = original {
            app.composer.update(cx, |s, cx| s.set_value(body, window, cx));
        }
        return;
    }
    TITLES.with(|t| t.borrow_mut().remove(&ch));
    HERO_FILE.with(|h| h.borrow_mut().remove(&ch));
    TITLE.with(|t| {
        if let Some((e, _)) = t.borrow().as_ref() {
            e.update(cx, |s, cx| s.set_value("", window, cx));
        }
    });
}

pub fn take_next(nonce: u64) {
    if let Some(o) = NEXT.with(|n| n.borrow_mut().take()) {
        PENDING.with(|p| p.borrow_mut().insert(nonce, o));
    }
}

pub fn after_sent(app: &mut PhoenixApp, channel: Id, nonce: u64, message: Id, cx: &mut Context<PhoenixApp>) {
    let Some(o) = PENDING.with(|p| p.borrow_mut().remove(&nonce)) else { return };
    if o.create_thread {
        app.blocked_write(crate::actions::guard::GuardedWrite::thread_from_message(channel.0, message.0, &o.thread_name), cx);
    }
    if o.publish {
        app.blocked_write(crate::actions::guard::GuardedWrite::crosspost(channel.0, message.0), cx);
    }
}

fn title_input(channel: Id, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Entity<InputState> {
    if let Some(e) = TITLE.with(|t| t.borrow().as_ref().map(|(e, _)| e.clone())) {
        if SHOWN_FOR.with(Cell::get) != Some(channel) {
            SHOWN_FOR.with(|s| s.set(Some(channel)));
            let v = title(channel);
            e.update(cx, |s, cx| s.set_value(v, window, cx));
        }
        return e;
    }
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("announce.title_placeholder")));
    let sub = cx.subscribe_in(&e, window, |a, st, ev: &InputEvent, w, cx| match ev {
        InputEvent::Change => {
            let raw = st.read(cx).value().to_string();
            let v = clamp_title(&raw);
            if v != raw {
                st.update(cx, |s, cx| s.set_value(v.clone(), w, cx));
            }
            if let Some(ch) = SHOWN_FOR.with(Cell::get) {
                TITLES.with(|t| t.borrow_mut().insert(ch, v));
            }
            cx.notify();
        }
        InputEvent::PressEnter { .. } => {
            a.composer.update(cx, |s, cx| s.focus(w, cx));
        }
        _ => {}
    });
    SHOWN_FOR.with(|s| s.set(Some(channel)));
    let v = title(channel);
    if !v.is_empty() {
        e.update(cx, |s, cx| s.set_value(v, window, cx));
    }
    TITLE.with(|t| *t.borrow_mut() = Some((e.clone(), sub)));
    e
}

fn hero_slot(app: &PhoenixApp, channel: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    if let Some(f) = hero(app, channel) {
        let hover = HERO_HOVER.with(Cell::get);
        let mut slot = div()
            .id("announce-hero")
            .relative()
            .flex_none()
            .size(z(HERO))
            .rounded(z(8.))
            .overflow_hidden()
            .on_hover(cx.listener(|_, on: &bool, _, cx| {
                HERO_HOVER.with(|h| h.set(*on));
                cx.notify();
            }));
        if let Some(i) = f.image.clone() {
            slot = slot.child(img(i).size_full().object_fit(gpui::ObjectFit::Cover));
        }
        return slot
            .when(hover, |d| {
                d.child(
                    div().absolute().top(z(8.)).right(z(8.)).child(
                        div()
                            .id("announce-hero-remove")
                            .size(z(24.))
                            .rounded(z(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip::tip(crate::t!("announce.hero_remove")))
                            .child(gpui::svg().path(crate::icons::path("chat/trash")).size(z(20.)).text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))))
                            .on_click(cx.listener(move |a, _, _, cx| {
                                remove_hero(a, channel);
                                HERO_HOVER.with(|h| h.set(false));
                                cx.notify();
                            })),
                    ),
                )
            })
            .into_any_element();
    }
    div()
        .id("announce-hero-add")
        .group("announce-hero-add")
        .flex_none()
        .size(z(HERO))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .tooltip(crate::ui::tooltip::tip(crate::t!("announce.hero_upload")))
        .child(
            gpui::svg()
                .path(crate::icons::path("image-plus"))
                .size(z(24.))
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover("announce-hero-add", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))),
        )
        .on_click(cx.listener(move |_, _, _, cx| {
            ACTIVE.with(|s| s.borrow_mut().insert(channel));
            prompt_hero(channel, cx);
        }))
        .into_any_element()
}

fn send_button(app: &PhoenixApp, channel: Id, disabled: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (thread, _) = crate::ui::extra_menus::announce_flags(channel);
    let can_thread = crate::commands::builtin::Context::of(app, channel).thread;
    let sub = can_thread && thread;
    let label = crate::t!("announce.send");
    let button = div()
        .id("announce-send")
        .group("announce-send")
        .h(z(32.))
        .px(z(8.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(4.))
        .bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT)))
        .when(disabled, |d| d.opacity(0.5).cursor(gpui::CursorStyle::OperationNotAllowed))
        .when(!disabled, |d| {
            d.cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_HOVER))))
                .active(|s| s.bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_ACTIVE))))
                .on_click(cx.listener(|a, _, w, cx| a.submit_composer(w, cx)))
        })
        .child(
            div()
                .p(z(4.))
                .flex()
                .items_center()
                .gap(z(4.))
                .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_OVERLAY_LIGHT))).child(label))
                .child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(gpui::svg().path(Icon::Send.path()).size(z(16.)).flex_none().text_color(gpui::white()))
                        .when(sub, |d| {
                            d.child(
                                div()
                                    .absolute()
                                    .bottom(z(-4.))
                                    .right(z(-4.))
                                    .size(z(12.))
                                    .p(z(1.))
                                    .rounded(z(8.))
                                    .bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT)))
                                    .group_hover("announce-send-box", |s| s.bg(c(tok(idx::CONTROL_PRIMARY_BACKGROUND_HOVER))))
                                    .child(gpui::svg().path(crate::icons::path("chat/thread")).size(z(10.)).text_color(gpui::white())),
                            )
                        }),
                ),
        );
    div()
        .id("announce-send-box")
        .group("announce-send-box")
        .relative()
        .child(button)
        .child(gpui::canvas(|b, _, _| SEND_BOX.with(|s| s.set(Some(b))), |_, _, _, _| ()).absolute().top_0().left_0().size_full())
        .on_hover(move |on, w, _| {
            use crate::ui::extra_menus::{Target, close, open, opened};
            let mine = matches!(opened(), Some(Target::AnnouncementSend { .. }));
            if *on && opened().is_none() {
                let b = SEND_BOX.with(Cell::get);
                let pos = b.map_or(w.mouse_position(), |b| gpui::point(b.origin.x + b.size.width, b.origin.y - z(8.)));
                open(Target::AnnouncementSend { channel, can_thread }, pos, w);
            } else if !*on && mine {
                let b = SEND_BOX.with(Cell::get);
                let over_menu = b.is_some_and(|b| w.mouse_position().y < b.origin.y);
                if !over_menu {
                    close();
                    w.refresh();
                }
            }
        })
        .into_any_element()
}

pub fn render(app: &PhoenixApp, channel: Id, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let body = app.composer.read(cx).value().to_string();
    let title_text = title(channel);
    let has_hero = hero(app, channel).is_some();
    let started = ACTIVE.with(|s| s.borrow().contains(&channel));
    let open = started || !body.is_empty() || has_hero || !title_text.is_empty();
    let disabled = send_disabled(&body, &title_text);
    let blocked = app.nav.extras.get(&channel).is_some_and(|x| x.send_blocked);
    let placeholder = if blocked { crate::t!("chat.no_permission") } else { super::composer::placeholder(app) };
    let title_box = title_input(channel, window, cx);
    let header = div()
        .flex_none()
        .overflow_hidden()
        .flex()
        .items_center()
        .gap(z(16.))
        .child(hero_slot(app, channel, cx))
        .child(
            div()
                .mt(z(16.))
                .h(z(28.))
                .flex_1()
                .min_w_0()
                .text_size(z(20.))
                .line_height(z(28.))
                .font_weight(FontWeight::BOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(
                    gpui_component::input::Input::new(&title_box)
                        .appearance(false)
                        .bordered(false)
                        .text_size(z(20.))
                        .line_height(z(28.)),
                ),
        );
    let header = if open {
        header
            .with_animation(SharedString::from(format!("announce-header-{}", channel.0)), Animation::new(crate::ui::settings::effects::motion(320)).with_easing(gpui::ease_out_quint()), |d, t| {
                d.h(z(HEADER_H * t)).pt(z(HEADER_PAD * t))
            })
            .into_any_element()
    } else {
        header.h(z(0.)).into_any_element()
    };
    let textarea = div()
        .flex_1()
        .min_w_0()
        .pr(z(8.))
        .relative()
        .py(z(super::composer::text_pad_y()))
        .text_size(fz(16.))
        .line_height(fz(22.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(
            div().mx(z(-4.)).child(
                gpui_component::input::Textarea::new(&app.composer)
                    .token(super::rich_input::render_token)
                    .on_paste({
                        let this = cx.entity().downgrade();
                        move |item, _, cx| this.update(cx, |a, cx| a.paste_clipboard(item, cx)).unwrap_or(false)
                    })
                    .appearance(false)
                    .bordered(false)
                    .xsmall()
                    .text_size(fz(16.))
                    .line_height(fz(22.))
                    .disabled(blocked),
            ),
        )
        .when(body.is_empty(), |d| {
            d.child(
                div()
                    .absolute()
                    .top(z(super::composer::text_pad_y() - 1.5))
                    .h(fz(22.))
                    .left_0()
                    .right_0()
                    .flex()
                    .items_center()
                    .text_color(c(tok(idx::CHANNEL_TEXT_AREA_PLACEHOLDER)))
                    .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(placeholder)),
            )
        });
    let emoji_open = matches!(
        crate::ui::pickers::open_tab(cx),
        Some((crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::Composer))
    );
    let attach = div()
        .id("announce-attach")
        .p(z(6.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .group("announce-attach")
        .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
        .child(
            gpui::svg()
                .path(crate::icons::path("attach/file-up"))
                .size(z(20.))
                .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                .group_hover("announce-attach", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))),
        )
        .on_click(cx.listener(move |a, _, _, cx| {
            ACTIVE.with(|s| s.borrow_mut().insert(channel));
            a.prompt_attach(cx);
        }));
    let footer = div()
        .h(z(56.))
        .px(z(16.))
        .flex()
        .items_center()
        .justify_between()
        .when(blocked, |d| d.invisible())
        .child(
            div()
                .ml(z(-8.))
                .flex()
                .items_center()
                .gap(z(8.))
                .child(attach)
                .child(crate::ui::states::composer_button("gif", Icon::Gif).on_click(cx.listener(|a, _, w, cx| {
                    crate::ui::pickers::toggle(a, crate::ui::pickers::Tab::Gif, crate::ui::pickers::EmojiTarget::Composer, w, cx)
                })))
                .child(
                    crate::ui::states::composer_button_ext("emoji", Icon::Emoji, 18., true)
                        .when(emoji_open, |d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                        .on_click(cx.listener(|a, _, _, cx| a.open_emoji_picker(super::state::EmojiTarget::Composer, cx))),
                ),
        )
        .child(div().ml(z(-8.)).flex().items_center().gap(z(8.)).child(send_button(app, channel, disabled, cx)));
    div()
        .flex_none()
        .relative()
        .flex()
        .flex_col()
        .gap(z(8.))
        .children(super::slash::render(app, cx))
        .child(
            div()
                .relative()
                .px(z(crate::theme::space::xs()))
                .mb(z(crate::theme::space::xs() - 0.8))
                .child(div().absolute().top(z(-24.)).left_0().right_0().child(super::composer::typing(app, window)))
                .child(
                    div()
                        .id("announce-composer")
                        .flex()
                        .flex_col()
                        .rounded(z(8.))
                        .border_1()
                        .border_color(c(tok(idx::BORDER_MUTED)))
                        .bg(c(tok(idx::CHAT_BACKGROUND_DEFAULT)))
                        .relative()
                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |_, _, _, cx| {
                            if ACTIVE.with(|s| s.borrow_mut().insert(channel)) {
                                cx.notify();
                            }
                        }))
                        .children(crate::ui::autocomplete::render(app, window, cx))
                        .child(
                            div()
                                .px(z(16.))
                                .flex()
                                .flex_col()
                                .child(header)
                                .child(div().flex().child(textarea))
                                .children(super::composer::upload_tray_for(app, cx).map(|t| div().ml(z(-16.)).child(t))),
                        )
                        .child(div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
                        .child(footer),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_is_the_title_heading_then_the_body() {
        assert_eq!(compose("", "hello"), "hello");
        assert_eq!(compose("News", "hello"), "# News\nhello");
        assert_eq!(compose("News", ""), "# News\n");
    }

    #[test]
    fn thread_name_falls_back_like_the_client() {
        assert_eq!(thread_name("Patch", "body", "Thread"), "Patch");
        assert_eq!(thread_name("", "body", "Thread"), "body");
        assert_eq!(thread_name("", "", "Thread"), "Thread");
        assert_eq!(thread_name(&"x".repeat(150), "", "Thread").chars().count(), THREAD_NAME_MAX);
    }

    #[test]
    fn title_is_one_line_and_capped() {
        assert_eq!(clamp_title("a\nb"), "a b");
        assert_eq!(clamp_title(&"y".repeat(200)).chars().count(), TITLE_MAX);
    }

    #[test]
    fn send_needs_a_body_or_a_title() {
        assert!(send_disabled("", ""));
        assert!(!send_disabled("hi", ""));
        assert!(!send_disabled("", "News"));
    }

    #[test]
    fn experiment_bucket_one_turns_the_composer_on() {
        set_assignments(&[(phoenix_api::rest::experiments::murmur3(phoenix_api::rest::experiments::ANNOUNCEMENT_COMPOSER), 1)]);
        assert!(enabled());
        set_assignments(&[]);
        assert!(!enabled());
    }
}

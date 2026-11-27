use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{AnimationExt, AnyElement, Bounds, Context, Div, Entity, IntoElement, ParentElement, Pixels, SharedString, Styled, Window, div, prelude::*};
use gpui_component::{Sizable as _, input::{InputEvent, Textarea, TextareaState}};

use crate::{
    app::PhoenixApp,
    model::Id,
    presence::ActivityKind,
    theme::{c, generated::idx, tok, z},
    ui::home::data::RelationshipKind,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    Popout,
    Modal,
    Sidebar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    React,
    Reply,
}

struct Active {
    user: Id,
    layout: Layout,
    kind: Interaction,
    since: Instant,
    input: Option<Entity<TextareaState>>,
    _sub: Option<gpui::Subscription>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
    static SIZES: RefCell<HashMap<(u64, Layout), (f32, f32)>> = RefCell::new(HashMap::new());
    static HEIGHTS: RefCell<HashMap<(u64, Layout), (f32, f32)>> = RefCell::new(HashMap::new());
    static PINNED: RefCell<HashMap<(u64, Layout), Instant>> = RefCell::new(HashMap::new());
    static FOLD: RefCell<HashMap<(u64, Layout), Fold>> = RefCell::new(HashMap::new());
    static TOAST: RefCell<Option<Toast>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toast {
    pub user: Id,
    pub sent: Option<Interaction>,
    pub since: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Fold {
    Open(Instant),
    Closing { from: f32, at: Instant, ms: u64 },
}

fn collapse_from(open_ms: f32, delay: u64, dur: u64, clamped: f32, target: f32) -> Option<(f32, u64)> {
    let p = ((open_ms - delay as f32) / dur.max(1) as f32).clamp(0., 1.);
    (p > 0.).then(|| (clamped + (target - clamped) * p, (dur as f32 * p).round() as u64))
}

pub const FADE_MS: u64 = 150;
pub const ICON_MS: u64 = 400;
pub const SCRIM_MS: u64 = 150;
pub const EXPAND_DELAY_MS: u64 = 300;
pub const EXPAND_MS: u64 = 150;
pub const BAR_DELAY_OTHERS_MS: u64 = 450;
pub const BAR_DELAY_OWN_MS: u64 = 300;
pub const TOAST_MS: u64 = 2700;
pub const TOAST_DELAY_MS: u64 = 200;
pub const OPEN_CLAMP: usize = 8;
pub const LINE_H: f32 = 18.;

pub fn clamp_heights(emoji: bool) -> (f32, f32) {
    let e = if emoji { 1.25 } else { 0. };
    (2. * LINE_H + e, 8. * LINE_H + e)
}

pub fn expand_target(full: f32, cap: f32, unclamped: bool) -> f32 {
    if unclamped { full } else { full.min(cap) }
}

pub fn expandable(clamped: f32, full: f32, cap: f32, unclamped: bool) -> bool {
    clamped > 0. && expand_target(full, cap, unclamped) > clamped + 0.5
}

pub fn delay_ms(ms: u64) -> u64 {
    if crate::ui::settings::effects::motion(ms).as_millis() <= 1 { 0 } else { ms }
}

pub fn bar_delay(me: bool, expandable: bool) -> u64 {
    match (expandable, me) {
        (false, _) => 0,
        (true, true) => delay_ms(BAR_DELAY_OWN_MS),
        (true, false) => delay_ms(BAR_DELAY_OTHERS_MS),
    }
}

pub fn toast_state() -> Option<Toast> {
    TOAST.with(|t| *t.borrow())
}

pub fn set_toast(user: Id, sent: Option<Interaction>) -> Instant {
    let since = Instant::now();
    TOAST.with(|t| *t.borrow_mut() = Some(Toast { user, sent, since }));
    since
}

pub fn clear_toast_if(since: Instant) {
    TOAST.with(|t| {
        let mut t = t.borrow_mut();
        if t.is_some_and(|x| x.since == since) {
            *t = None;
        }
    });
}

pub fn toast_text(sent: Option<Interaction>) -> SharedString {
    match sent {
        Some(Interaction::Reply) => crate::t!("ptb.toast_reply"),
        Some(Interaction::React) => crate::t!("ptb.toast_react"),
        None => crate::t!("ptb.toast_sending"),
    }
}

pub fn toast(user: Id, banner_h: f32) -> Option<AnyElement> {
    let t = toast_state().filter(|t| t.user == user)?;
    let success = t.sent.is_some();
    let card = div()
        .relative()
        .p(z(12.))
        .rounded(z(8.))
        .bg(c(tok(if success { idx::BACKGROUND_BASE_LOW } else { idx::BACKGROUND_SURFACE_HIGHEST })))
        .shadow(crate::ui::inbox::shadow_high())
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .rounded(z(8.))
                .border_1()
                .when(success, |d| d.bg(c(tok(idx::TOAST_SUCCESS_BACKGROUND))).border_color(c(tok(idx::TOAST_SUCCESS_BORDER))))
                .when(!success, |d| d.border_color(c(tok(idx::BORDER_NORMAL)))),
        )
        .child(
            div()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .gap(z(8.))
                .when(success, |d| {
                    d.child(gpui::svg().path(crate::icons::path("circle-check")).size(z(18.)).flex_none().text_color(c(tok(idx::ICON_FEEDBACK_POSITIVE))))
                })
                .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(toast_text(t.sent))),
        );
    let delay = delay_ms(TOAST_DELAY_MS);
    let enter = crate::ui::settings::effects::motion(FADE_MS).as_millis() as u64;
    let total = delay + enter;
    let row = div().absolute().left_0().w_full().top(z((banner_h - 56.) / 2.)).flex().justify_center().child(card);
    let el = if total > 1 && t.since.elapsed() < Duration::from_millis(total) {
        row.with_animation(
            gpui::ElementId::Name(format!("ptb-toast-{}-{:?}", user.0, t.since).into()),
            gpui::Animation::new(Duration::from_millis(total)),
            move |d, x| {
                let p = (((x * total as f32) - delay as f32) / enter.max(1) as f32).clamp(0., 1.);
                d.opacity(p).mt(z(16. * (1. - p)))
            },
        )
        .into_any_element()
    } else {
        row.into_any_element()
    };
    Some(gpui::deferred(el).with_priority(45).into_any_element())
}

fn heights(uid: Id, layout: Layout) -> (f32, f32) {
    HEIGHTS.with(|h| h.borrow().get(&(uid.0, layout)).copied()).unwrap_or_default()
}

fn measure_height(uid: Id, layout: Layout, full: bool) -> impl IntoElement {
    gpui::canvas(
        move |b: Bounds<Pixels>, _, _| {
            let hgt = f32::from(b.size.height) / crate::theme::zoom();
            HEIGHTS.with(|m| {
                let mut m = m.borrow_mut();
                let e = m.entry((uid.0, layout)).or_default();
                if full { e.1 = hgt } else { e.0 = hgt }
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub fn toggle_pinned(uid: Id, layout: Layout) -> bool {
    PINNED.with(|p| {
        let mut p = p.borrow_mut();
        if p.remove(&(uid.0, layout)).is_some() {
            false
        } else {
            p.insert((uid.0, layout), Instant::now());
            true
        }
    })
}

fn pinned(uid: Id, layout: Layout) -> Option<Instant> {
    PINNED.with(|p| p.borrow().get(&(uid.0, layout)).copied())
}

fn status_has_emoji(app: &PhoenixApp, uid: Id) -> bool {
    app.people.activities.get(&uid).and_then(|a| a.iter().find(|a| a.kind == ActivityKind::Custom)).is_some_and(|a| a.emoji.is_some())
}

pub fn ease(t: f32) -> f32 {
    crate::ui::states::cubic_bezier(0.25, 0.1, 0.25, 1.)(t)
}

pub fn icon_ease(t: f32) -> f32 {
    crate::ui::states::cubic_bezier(0.16, 1., 0.3, 1.)(t)
}

pub fn interaction(user: Id, layout: Layout) -> Option<Interaction> {
    ACTIVE.with(|a| a.borrow().as_ref().filter(|a| a.user == user && a.layout == layout).map(|a| a.kind))
}

fn active_since(user: Id, layout: Layout) -> Option<Instant> {
    ACTIVE.with(|a| a.borrow().as_ref().filter(|a| a.user == user && a.layout == layout).map(|a| a.since))
}

fn reply_input(user: Id, layout: Layout) -> Option<Entity<TextareaState>> {
    ACTIVE.with(|a| a.borrow().as_ref().filter(|a| a.user == user && a.layout == layout).and_then(|a| a.input.clone()))
}

pub fn reset() {
    ACTIVE.with(|a| a.borrow_mut().take());
}

pub fn react_target() -> Option<Id> {
    ACTIVE.with(|a| a.borrow().as_ref().filter(|a| a.kind == Interaction::React).map(|a| a.user))
}

pub fn dm_allowed(is_self: bool, lurking: bool, friend: bool, mut guilds: impl Iterator<Item = Id>, restricted: &[u64]) -> bool {
    !is_self && !lurking && (friend || guilds.any(|g| !restricted.contains(&g.0)))
}

pub fn can_dm(app: &PhoenixApp, uid: Id, guild: Option<Id>) -> bool {
    let friend = app.nav.relationship(uid).is_some_and(|r| r.kind == RelationshipKind::Friend);
    let lurking = guild.is_some_and(crate::ui::join_flow::is_lurking);
    let mutual = app.people.profiles.get(&uid).map(|p| p.mutual_guild_ids.clone()).unwrap_or_default();
    let cached = app.members.iter().filter(|(_, ms)| ms.iter().any(|m| m.user == uid)).map(|(g, _)| *g);
    let restricted = crate::ui::settings::account::dm_restricted();
    dm_allowed(uid == app.me.id, lurking, friend, mutual.into_iter().chain(cached), &restricted)
}

pub fn others_toolbar_shown(app: &PhoenixApp, uid: Id, guild: Option<Id>) -> bool {
    let bot = app.users.get(&uid).is_some_and(|u| u.bot);
    !bot && uid != app.me.id && can_dm(app, uid, guild)
}

pub fn status_details(app: &PhoenixApp, uid: Id) -> Option<String> {
    let acts = app.people.activities.get(&uid)?;
    let a = acts.iter().find(|a| a.kind == ActivityKind::Custom)?;
    let emoji = a.emoji.as_deref().map(|e| match e.strip_prefix('<') {
        Some(rest) => {
            let name = rest.trim_start_matches("a:").trim_start_matches(':').split(':').next().unwrap_or_default();
            format!("`:{name}:`")
        }
        None => e.to_string(),
    });
    let text = a.state.as_deref().filter(|s| !s.is_empty()).map(str::to_string);
    match (emoji, text) {
        (None, None) => None,
        (Some(e), None) => Some(e),
        (None, Some(t)) => Some(t),
        (Some(e), Some(t)) => Some(format!("{e} {t}")),
    }
}

pub fn react_content(username: &str, details: Option<&str>, emoji: &str) -> String {
    let head = crate::t!("ptb.reacted_status", username = username);
    match details {
        Some(d) => format!("> -# *{head}*\n> {d}\n{emoji}"),
        None => format!("> -# *{head}*\n{emoji}"),
    }
}

pub fn reply_content(username: &str, details: Option<&str>, input: &str) -> String {
    let head = crate::t!("ptb.replied_status", username = username);
    match details {
        Some(d) => format!("> -# *{head}*\n> {d}\n{input}"),
        None => format!("> -# *{head}*\n{input}"),
    }
}

impl PhoenixApp {
    pub fn send_profile_interaction(&mut self, user: Id, kind: Interaction, content: String, cx: &mut Context<Self>) {
        set_toast(user, None);
        let open = self.backend.open_dm(user);
        let backend = self.backend.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(ch) = open.await {
                let nonce = crate::ui::chat::slash::next_nonce();
                let _ = backend.send_message_ex(ch, content, None, Vec::new(), nonce, None).await;
            }
            let since = set_toast(user, Some(kind));
            let _ = this.update(cx, |_, cx| cx.notify());
            cx.background_executor().timer(Duration::from_millis(TOAST_MS)).await;
            clear_toast_if(since);
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    pub fn profile_react(&mut self, emoji: String, cx: &mut Context<Self>) {
        let Some(user) = react_target() else { return };
        let username = self.users.get(&user).map(|u| u.display_name.to_string()).unwrap_or_default();
        let details = status_details(self, user);
        reset();
        self.send_profile_interaction(user, Interaction::React, react_content(&username, details.as_deref(), &emoji), cx);
        cx.notify();
    }

    pub fn start_profile_interaction(&mut self, user: Id, layout: Layout, kind: Interaction, window: &mut Window, cx: &mut Context<Self>) {
        match kind {
            Interaction::React => {
                ACTIVE.with(|a| {
                    *a.borrow_mut() = Some(Active { user, layout, kind, since: Instant::now(), input: None, _sub: None })
                });
                crate::ui::pickers::open_emoji_picker(self, crate::ui::pickers::EmojiTarget::ProfileReact, window, cx);
            }
            Interaction::Reply => {
                let name = self.users.get(&user).map(|u| u.display_name.to_string()).unwrap_or_default();
                let max_rows = ((f32::from(window.viewport_size().height) * 0.5 / 22.).floor() as usize).max(REPLY_ROWS);
                let input = cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .auto_grow(REPLY_ROWS, max_rows)
                        .submit_on_enter(true)
                        .placeholder(crate::t!("ptb.reply_placeholder", username = name))
                });
                let sub = cx.subscribe_in(&input, window, move |this, input, ev: &InputEvent, _, cx| match ev {
                    InputEvent::PressEnter { shift: false, .. } => {
                        let text = crate::ui::chat::rich_input::peek_plain(input.read(cx).value().trim());
                        if text.trim().is_empty() {
                            return;
                        }
                        let username = this.users.get(&user).map(|u| u.display_name.to_string()).unwrap_or_default();
                        let details = status_details(this, user);
                        reset();
                        this.send_profile_interaction(user, Interaction::Reply, reply_content(&username, details.as_deref(), &text), cx);
                        cx.notify();
                    }
                    InputEvent::Change => crate::ui::chat::rich_input::sync_area(this, input, cx),
                    _ => {}
                });
                input.update(cx, |s, cx| s.focus(window, cx));
                ACTIVE.with(|a| {
                    *a.borrow_mut() = Some(Active { user, layout, kind, since: Instant::now(), input: Some(input), _sub: Some(sub) })
                });
            }
        }
        cx.notify();
    }

    pub fn clear_custom_status_from_profile(&mut self, cx: &mut Context<Self>) {
        self.people.custom_status = None;
        self.prefs.synced.proto.remove(crate::ui::modals::custom_status::EXPIRES_KEY);
        cx.background_spawn(self.backend.save_custom_status(self.presence, None)).detach();
        cx.notify();
    }

    pub fn open_main_profile_settings(&mut self, cx: &mut Context<Self>) {
        self.people.close_overlays();
        crate::ui::settings::open(self, cx);
        if let Some(s) = self.settings.as_mut() {
            s.page = crate::ui::settings::Page::Profiles;
        }
        cx.notify();
    }
}

fn shadow_low() -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: gpui::hsla(0., 0., 0., 0.14),
        offset: gpui::point(z(0.), z(1.)),
        blur_radius: z(4.),
        spread_radius: z(0.),
        inset: false,
    }]
}

fn scaled_icon(key: u64, path: SharedString, scale: bool, themed: bool) -> AnyElement {
    let rest = if themed { idx::ICON_STRONG } else { idx::INTERACTIVE_TEXT_DEFAULT };
    let pressed = gpui::svg()
        .path(path)
        .flex_none()
        .text_color(c(tok(rest)))
        .group_hover("ptb-btn", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
        .id("ptb-glyph")
        .group_active("ptb-btn", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE))).size(z(16.)));
    if !scale {
        return pressed.size(z(16.)).into_any_element();
    }
    let ms = crate::ui::settings::effects::motion(ICON_MS).as_millis() as u64;
    match crate::ui::states::state(key) {
        Some((on, at)) if ms > 0 && at.elapsed() < Duration::from_millis(ms) => pressed
            .with_animation(
                gpui::ElementId::Name(format!("ptb-icon-{key}-{on}-{at:?}").into()),
                gpui::Animation::new(Duration::from_millis(ms)).with_easing(icon_ease),
                move |g, t| g.size(z(if on { 16. + 1.6 * t } else { 17.6 - 1.6 * t })),
            )
            .into_any_element(),
        Some((true, _)) => pressed.size(z(17.6)).into_any_element(),
        _ => pressed.size(z(16.)).into_any_element(),
    }
}

pub fn toolbar_button(id: impl Into<gpui::ElementId>, key: u64, icon: &str, first: bool, last: bool, scale: bool, themed: bool) -> gpui::Stateful<Div> {
    let l = if first { 100. } else { 6. };
    let r = if last { 100. } else { 6. };
    let el = div()
        .id(id)
        .group("ptb-btn")
        .h(z(24.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .py(z(4.))
        .pl(z(if first { 6. } else { 4. }))
        .pr(z(if last { 6. } else { 4. }))
        .rounded_tl(z(l))
        .rounded_bl(z(l))
        .rounded_tr(z(r))
        .rounded_br(z(r))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .active(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
        .child(div().size(z(16.)).flex().flex_none().items_center().justify_center().child(scaled_icon(key, crate::icons::path(icon), scale, themed)));
    crate::ui::states::track(el, key)
}

fn visible(container: u64, bar: u64) -> Option<Instant> {
    let a = crate::ui::states::state(container).filter(|s| s.0).map(|s| s.1);
    let b = crate::ui::states::state(bar).filter(|s| s.0).map(|s| s.1);
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, y) => x.or(y),
    }
}

fn popover(bar_key: u64, since: Option<Instant>, delay: u64, top_rest: f32, right: f32, buttons: Vec<AnyElement>) -> AnyElement {
    let bar = div()
        .id(("ptb-bar", bar_key))
        .absolute()
        .right(z(right))
        .flex()
        .gap(z(2.))
        .p(z(2.))
        .rounded(z(100.))
        .bg(c(tok(idx::USER_PROFILE_TOOLBAR_BACKGROUND)))
        .border_1()
        .border_color(c(tok(idx::USER_PROFILE_TOOLBAR_BORDER)))
        .shadow(shadow_low())
        .children(buttons);
    let bar = crate::ui::states::track(bar, bar_key);
    let ms = crate::ui::settings::effects::motion(FADE_MS).as_millis() as u64;
    let total = ms + delay;
    match since {
        None => bar.top(z(top_rest)).opacity(0.).invisible().into_any_element(),
        Some(at) if ms > 0 && at.elapsed() < Duration::from_millis(total) => bar
            .top(z(top_rest + 2.))
            .opacity(0.)
            .with_animation(
                gpui::ElementId::Name(format!("ptb-fade-{bar_key}-{at:?}").into()),
                gpui::Animation::new(Duration::from_millis(total)),
                move |d, t| {
                    let p = ease((((t * total as f32) - delay as f32) / ms as f32).clamp(0., 1.));
                    d.top(z(top_rest + 2. - 2. * p)).opacity(p)
                },
            )
            .into_any_element(),
        Some(_) => bar.top(z(top_rest)).into_any_element(),
    }
}

fn bubble_bg(hover: bool) -> gpui::Hsla {
    let base = c(tok(idx::BACKGROUND_SURFACE_HIGHEST));
    if !hover {
        return base;
    }
    let over = if crate::theme::is_light() { gpui::hsla(0., 0., 0., 1.) } else { gpui::hsla(0., 0., 1., 1.) };
    crate::ui::states::mix(base, over, 0.04)
}

fn bubble_border(themed: bool) -> gpui::Hsla {
    if themed && !crate::theme::is_light() { c(tok(idx::BORDER_STRONG)) } else { c(tok(idx::BORDER_MUTED)) }
}

fn thought_dots(bg: gpui::Hsla, border: gpui::Hsla) -> [Div; 2] {
    let dot = |size: f32, left: f32, top: f32| {
        div()
            .absolute()
            .left(z(left))
            .top(z(top))
            .size(z(size))
            .rounded_full()
            .bg(bg)
            .border_1()
            .border_color(border)
            .shadow(shadow_low())
    };
    [dot(20., 10., -8.), dot(10., -3., -15.)]
}

fn bubble(content: AnyElement, hover: bool, themed: bool, clickable: bool) -> Div {
    let bg = bubble_bg(hover);
    let border = bubble_border(themed);
    div()
        .relative()
        .children(thought_dots(bg, border))
        .child(
            div()
                .relative()
                .max_w(z(181.))
                .min_w(z(42.))
                .px(z(12.))
                .py(z(8.))
                .rounded(z(16.))
                .bg(bg)
                .border_1()
                .border_color(border)
                .shadow(shadow_low())
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .when(clickable, |d| d.cursor_pointer())
                .child(content),
        )
        .child(div().absolute().top_0().left(z(11.)).w(z(18.)).h(z(1.)).bg(bg))
}

fn measure(uid: Id, layout: Layout) -> impl IntoElement {
    gpui::canvas(
        move |b: Bounds<Pixels>, _, _| {
            let k = crate::theme::zoom();
            let size = (f32::from(b.size.width) / k, f32::from(b.size.height) / k);
            SIZES.with(|m| m.borrow_mut().insert((uid.0, layout), size));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn measured(uid: Id, layout: Layout) -> Option<(f32, f32)> {
    SIZES.with(|m| m.borrow().get(&(uid.0, layout)).copied())
}

pub fn others_right(w: Option<f32>) -> f32 {
    w.map_or(8., |w| 8f32.min(w / 2. - 29.))
}

pub fn own_right(w: Option<f32>) -> f32 {
    w.map_or(8., |w| 8f32.min(w / 2. - 60.))
}

pub fn reply_offset(layout: Layout, themed: bool) -> f32 {
    match layout {
        Layout::Popout if themed => -120.,
        Layout::Popout => -122.,
        Layout::Sidebar => -120.,
        Layout::Modal => 0.,
    }
}

pub const REPLY_ROWS: usize = 4;

fn reply_box(input: &Entity<TextareaState>, layout: Layout, themed: bool, top: f32) -> AnyElement {
    let w = if layout == Layout::Sidebar { 276. } else { 300. };
    let anchored = gpui::deferred(
        gpui::anchored().position_mode(gpui::AnchoredPositionMode::Local).position(gpui::point(z(reply_offset(layout, themed)), z(top + 6.))).child(
            div()
                .id("ptb-reply")
                .occlude()
                .w(z(w))
                .p(z(16.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .shadow(vec![gpui::BoxShadow {
                    color: gpui::hsla(0., 0., 0., 0.24),
                    offset: gpui::point(z(0.), z(12.)),
                    blur_radius: z(24.),
                    spread_radius: z(0.),
                    inset: false,
                }])
                .child(
                    div()
                        .min_h(z(88.))
                        .rounded(z(8.))
                        .px(z(16.))
                        .py(z(11.))
                        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                        .text_size(z(14.))
                        .line_height(z(22.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(
                            div().mx(z(-4.)).child(
                                Textarea::new(input)
                                    .token(crate::ui::chat::rich_input::render_token)
                                    .appearance(false)
                                    .bordered(false)
                                    .xsmall()
                                    .text_size(z(14.))
                                    .line_height(z(22.)),
                            ),
                        ),
                ),
        ),
    )
    .with_priority(40);
    div().absolute().top_0().left_0().child(anchored).into_any_element()
}

pub fn scrim(user: Id, layout: Layout) -> Option<AnyElement> {
    let since = active_since(user, layout)?;
    let ms = crate::ui::settings::effects::motion(SCRIM_MS).as_millis() as u64;
    let el = div()
        .id(("ptb-scrim", user.0))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .occlude()
        .bg(c(tok(idx::BACKGROUND_SCRIM)))
        .on_mouse_down(gpui::MouseButton::Left, |_, w, _| {
            reset();
            w.refresh();
        });
    Some(if ms > 0 && since.elapsed() < Duration::from_millis(ms) {
        el.with_animation(
            gpui::ElementId::Name(format!("ptb-scrim-{}-{since:?}", user.0).into()),
            gpui::Animation::new(Duration::from_millis(ms)),
            |d, t| d.opacity(t),
        )
        .into_any_element()
    } else {
        el.into_any_element()
    })
}

pub fn status(
    app: &PhoenixApp,
    uid: Id,
    guild: Option<Id>,
    layout: Layout,
    themed: bool,
    text: Option<SharedString>,
    cx: &mut Context<PhoenixApp>,
) -> Option<AnyElement> {
    let me = uid == app.me.id;
    let lk = layout as u64;
    let ck = crate::ui::states::key(("ptb-status", uid.0, lk));
    let bk = crate::ui::states::key(("ptb-status-bar", uid.0, lk));
    let hover = crate::ui::states::hovered(ck);
    let size = measured(uid, layout);
    let w = size.map(|s| s.0);
    let e = cx.entity();
    let Some(text) = text else {
        if !me {
            return None;
        }
        let prompt = div()
            .flex()
            .items_center()
            .child(gpui::svg().path(crate::icons::path("plus-circle")).size(z(16.)).mr(z(4.)).flex_none().text_color(c(tok(if hover { idx::TEXT_DEFAULT } else { idx::TEXT_MUTED }))))
            .child(
                div()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .text_color(c(tok(if hover { idx::TEXT_DEFAULT } else { idx::TEXT_MUTED })))
                    .child(crate::t!("ptb.add_status")),
            );
        let el = div()
            .id(("ptb-add-status", uid.0 + lk))
            .relative()
            .on_click(move |_, _, cx| e.update(cx, |a, cx| a.open_custom_status(cx)))
            .tooltip(crate::ui::tooltip::tip(crate::t!("ptb.add_custom_status")))
            .child(bubble(prompt.into_any_element(), false, themed, true));
        return Some(crate::ui::states::track(el, ck).into_any_element());
    };
    let unclamped = layout == Layout::Modal;
    let (_, cap) = clamp_heights(status_has_emoji(app, uid));
    let (ch, fh) = heights(uid, layout);
    let can_expand = expandable(ch, fh, cap, unclamped);
    let interacting = interaction(uid, layout);
    let hover_since = if interacting.is_some() { None } else { crate::ui::states::state(ck).filter(|s| s.0).map(|s| s.1) };
    let open_since = pinned(uid, layout).or(hover_since).filter(|_| can_expand);
    let mk = |t: &SharedString| crate::ui::emoji_text::status_wrapped(t.clone(), true);
    let mk_probe = |t: &SharedString| crate::ui::emoji_text::status_wrapped(t.clone(), false);
    let open_clamp = if unclamped { 1000 } else { OPEN_CLAMP };
    let probe_clamped = div().absolute().top_0().left_0().w_full().invisible().line_clamp(2).child(mk_probe(&text)).child(measure_height(uid, layout, false));
    let probe_full = div().absolute().top_0().left_0().w_full().invisible().line_clamp(open_clamp).child(mk_probe(&text)).child(measure_height(uid, layout, true));
    let target = expand_target(fh, cap, unclamped);
    let fold_key = (uid.0, layout);
    let delay = delay_ms(EXPAND_DELAY_MS);
    let dur = crate::ui::settings::effects::motion(EXPAND_MS).as_millis() as u64;
    let fold = FOLD.with(|f| {
        let mut f = f.borrow_mut();
        match (open_since, f.get(&fold_key).copied()) {
            (Some(at), _) => {
                f.insert(fold_key, Fold::Open(at));
                None
            }
            (None, Some(Fold::Open(start))) => match collapse_from(start.elapsed().as_millis() as f32, delay, dur, ch, target) {
                Some((from, ms)) if ms > 0 => {
                    let at = Instant::now();
                    f.insert(fold_key, Fold::Closing { from, at, ms });
                    Some((from, at, ms))
                }
                _ => {
                    f.remove(&fold_key);
                    None
                }
            },
            (None, Some(Fold::Closing { from, at, ms })) if at.elapsed() < Duration::from_millis(ms) => Some((from, at, ms)),
            (None, Some(_)) => {
                f.remove(&fold_key);
                None
            }
            (None, None) => None,
        }
    });
    let visible_text = match open_since {
        None => match fold {
            Some((from, at, ms)) => div()
                .overflow_hidden()
                .child(mk(&text))
                .line_clamp(open_clamp)
                .max_h(z(from))
                .with_animation(
                    gpui::ElementId::Name(format!("ptb-fold-{}-{lk}-{at:?}", uid.0).into()),
                    gpui::Animation::new(Duration::from_millis(ms)),
                    move |d, t| if t >= 1. { d.line_clamp(2) } else { d.max_h(z(from + (ch - from) * t)) },
                )
                .into_any_element(),
            None => div().line_clamp(2).child(mk(&text)).into_any_element(),
        },
        Some(at) => {
            let total = delay + dur;
            let start = ch;
            let el = div().overflow_hidden().child(mk(&text));
            if total > 1 && at.elapsed() < Duration::from_millis(total) {
                el.line_clamp(2)
                    .with_animation(
                        gpui::ElementId::Name(format!("ptb-expand-{}-{lk}-{at:?}", uid.0).into()),
                        gpui::Animation::new(Duration::from_millis(total)),
                        move |d, t| {
                            let ms = t * total as f32;
                            if ms < delay as f32 {
                                return d.line_clamp(2);
                            }
                            let p = ((ms - delay as f32) / dur.max(1) as f32).clamp(0., 1.);
                            d.line_clamp(open_clamp).max_h(z(start + (target - start) * p))
                        },
                    )
                    .into_any_element()
            } else {
                el.line_clamp(open_clamp).max_h(z(target)).into_any_element()
            }
        }
    };
    let content = div().relative().child(probe_clamped).child(probe_full).child(visible_text).into_any_element();
    let shown = if interacting.is_some() { None } else { visible(ck, bk) };
    let bar = if me {
        let e1 = e.clone();
        let e2 = e.clone();
        let edit = toolbar_button(("ptb-edit", uid.0 + lk), crate::ui::states::key(("ptb-edit", uid.0, lk)), "pen", true, false, false, themed)
            .tooltip(crate::ui::tooltip::tip(crate::t!("ptb.edit")))
            .on_click(move |_, _, cx| e1.update(cx, |a, cx| a.open_custom_status(cx)));
        let clear = toolbar_button(("ptb-clear", uid.0 + lk), crate::ui::states::key(("ptb-clear", uid.0, lk)), "chat/trash", false, true, false, themed)
            .tooltip(crate::ui::tooltip::tip(crate::t!("ptb.clear")))
            .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.clear_custom_status_from_profile(cx)));
        Some(popover(bk, shown, bar_delay(true, can_expand), -20., own_right(w), vec![edit.into_any_element(), clear.into_any_element()]))
    } else if others_toolbar_shown(app, uid, guild) {
        let e1 = e.clone();
        let e2 = e.clone();
        let react = toolbar_button(("ptb-react", uid.0 + lk), crate::ui::states::key(("ptb-react", uid.0, lk)), "chat/add-reaction", true, false, true, themed)
            .tooltip(crate::ui::tooltip::tip(crate::t!("ptb.send_reaction")))
            .on_click(move |_, w, cx| e1.update(cx, |a, cx| a.start_profile_interaction(uid, layout, Interaction::React, w, cx)));
        let reply = toolbar_button(("ptb-reply", uid.0 + lk), crate::ui::states::key(("ptb-reply-btn", uid.0, lk)), "reply", false, true, true, themed)
            .tooltip(crate::ui::tooltip::tip(crate::t!("ptb.reply")))
            .on_click(move |_, w, cx| e2.update(cx, |a, cx| a.start_profile_interaction(uid, layout, Interaction::Reply, w, cx)));
        Some(popover(bk, shown, bar_delay(false, can_expand), -16., others_right(w), vec![react.into_any_element(), reply.into_any_element()]))
    } else {
        None
    };
    let reply = (interacting == Some(Interaction::Reply)).then(|| reply_input(uid, layout)).flatten();
    let h = size.map_or(36., |s| s.1);
    let el = div()
        .id(("ptb-status", uid.0 + lk))
        .relative()
        .child(measure(uid, layout))
        .child(bubble(content, me && hover, themed, false))
        .children(can_expand.then(|| expand_button(uid, layout, open_since.is_some(), me && hover, cx)))
        .children(bar)
        .children(reply.map(|i| reply_box(&i, layout, themed, h)));
    let el = crate::ui::states::track(el, ck);
    Some(if interacting.is_some() { gpui::deferred(el).with_priority(30).into_any_element() } else { el.into_any_element() })
}

fn expand_button(uid: Id, layout: Layout, open: bool, hover: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let key = crate::ui::states::key(("ptb-expand", uid.0, layout as u64));
    let shown = crate::ui::focus_ring::shown(key);
    let e = cx.entity();
    let e2 = e.clone();
    let label = if open { crate::t!("ptb.collapse_status") } else { crate::t!("ptb.expand_status") };
    let btn = div()
        .id(("ptb-expand", uid.0 + layout as u64))
        .absolute()
        .bottom(z(-12.))
        .right(z(8.))
        .size(z(24.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(bubble_bg(hover))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .when(!shown, |d| d.opacity(0.))
        .when(shown, |d| d.cursor_pointer().tooltip(crate::ui::tooltip::tip(label)))
        .child(
            gpui::svg()
                .path(crate::icons::path("chevron"))
                .size(z(16.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .when(open, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI)))),
        )
        .on_click(move |_, _, cx| {
            toggle_pinned(uid, layout);
            e.update(cx, |_, cx| cx.notify());
        })
        .on_key_down(move |ev: &gpui::KeyDownEvent, _, cx| {
            if matches!(ev.keystroke.key.as_str(), "enter" | "space") {
                toggle_pinned(uid, layout);
                e2.update(cx, |_, cx| cx.notify());
                cx.stop_propagation();
            }
        });
    crate::ui::focus_ring::ring(btn, key, 12.).into_any_element()
}

pub fn relationship_button(app: &PhoenixApp, uid: Id, cx: &mut Context<PhoenixApp>) -> Option<gpui::Stateful<Div>> {
    let bot = app.users.get(&uid).is_some_and(|u| u.bot);
    if bot || uid == app.me.id {
        return None;
    }
    let rel = app.nav.relationship(uid).map(|r| r.kind).unwrap_or(RelationshipKind::None);
    let b = |id: &'static str, icon: &str| crate::ui::profile::banner_button((id, uid.0), icon);
    Some(match rel {
        RelationshipKind::Blocked => return None,
        RelationshipKind::Friend => b("profile-friend", "user-check")
            .tooltip(crate::ui::tooltip::tip(crate::t!("ptb.friend")))
            .on_click(move |ev, w, _| crate::ui::extra_menus::open(crate::ui::extra_menus::Target::FriendActions(uid), ev.position(), w)),
        RelationshipKind::Incoming => b("profile-pending-in", "user-clock")
            .tooltip(crate::ui::tooltip::tip(crate::t!("ptb.incoming")))
            .on_click(move |ev, w, _| crate::ui::extra_menus::open(crate::ui::extra_menus::Target::FriendActions(uid), ev.position(), w)),
        RelationshipKind::Outgoing => b("profile-pending-out", "user-clock")
            .tooltip(crate::ui::tooltip::tip(crate::t!("ptb.outgoing")))
            .opacity(0.5)
            .cursor_default(),
        RelationshipKind::None => b("profile-add-friend", "invite").tooltip(crate::ui::tooltip::tip(crate::t!("ptb.add_friend"))).on_click(cx.listener(
            move |a, _, _, cx| a.friend_menu_action(uid, cx),
        )),
    })
}

pub fn friend_action_items(app: &PhoenixApp, e: &Entity<PhoenixApp>, uid: Id) -> Vec<crate::ui::context_menu::MenuItem> {
    use crate::ui::context_menu::MenuItem;
    let rel = app.nav.relationship(uid).map(|r| r.kind);
    let e = e.clone();
    match rel {
        Some(RelationshipKind::Friend) => vec![MenuItem::action(crate::t!("menu.remove_friend"), None, move |_, _, cx: &mut gpui::App| {
            e.update(cx, |a, cx| a.confirm_remove_friend(uid, cx))
        })],
        Some(RelationshipKind::Incoming) => vec![MenuItem::action(crate::t!("ptb.accept_request"), None, move |_, _, cx: &mut gpui::App| {
            e.update(cx, |a, cx| a.friend_menu_action(uid, cx))
        })],
        _ => Vec::new(),
    }
}

pub fn edit_profile_items(e: &Entity<PhoenixApp>, guild: Id) -> Vec<crate::ui::context_menu::MenuItem> {
    use crate::ui::context_menu::MenuItem;
    let (e1, e2) = (e.clone(), e.clone());
    vec![
        MenuItem::action(crate::t!("ptb.edit_server_profile"), None, move |_, _, cx: &mut gpui::App| {
            e1.update(cx, |a, cx| {
                a.people.close_overlays();
                a.open_server_profile(guild, cx)
            })
        })
        .with_subtext(crate::t!("ptb.edit_server_profile_sub")),
        MenuItem::action(crate::t!("ptb.edit_main_profile"), None, move |_, _, cx: &mut gpui::App| {
            e2.update(cx, |a, cx| a.open_main_profile_settings(cx))
        })
        .with_subtext(crate::t!("ptb.edit_main_profile_sub")),
    ]
}

pub fn edit_profile_button(uid: Id, guild: Option<Id>, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<Div> {
    let fg = c(tok(idx::CONTROL_PRIMARY_TEXT_DEFAULT));
    crate::ui::profile::mana_button(("profile-edit", uid.0), "", true, false)
        .w_full()
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .child(gpui::svg().path(crate::icons::path("pen")).size(z(16.)).flex_none().text_color(fg))
                .child(crate::t!("ptb.edit_profile")),
        )
        .on_click(cx.listener(move |a, ev: &gpui::ClickEvent, w, cx| match guild {
            Some(g) => crate::ui::extra_menus::open(crate::ui::extra_menus::Target::EditProfile(g), ev.position(), w),
            None => a.open_main_profile_settings(cx),
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_runs_back_from_where_the_expansion_was() {
        assert_eq!(collapse_from(200., 300, 150, 36., 90.), None);
        assert_eq!(collapse_from(1000., 300, 150, 36., 90.), Some((90., 150)));
        let (from, ms) = collapse_from(375., 300, 150, 36., 90.).unwrap();
        assert!((from - 63.).abs() < 1e-3 && ms == 75, "{from} {ms}");
    }

    #[test]
    fn toolbar_sits_eight_pixels_from_the_end_or_centres_on_small_bubbles() {
        assert_eq!(others_right(None), 8.);
        assert_eq!(others_right(Some(162.)), 8.);
        assert_eq!(others_right(Some(60.)), 1.);
        assert_eq!(own_right(Some(200.)), 8.);
        assert_eq!(own_right(Some(100.)), -10.);
    }

    #[test]
    fn reply_and_react_messages_quote_the_status() {
        let r = reply_content("Nyx", Some("thinking"), "hi");
        assert!(r.starts_with("> -# *"));
        assert!(r.ends_with("*\n> thinking\nhi"));
        let r = react_content("Nyx", None, ":wave:");
        assert!(r.ends_with("*\n:wave:"));
    }

    #[test]
    fn clamped_status_expands_like_the_client() {
        assert_eq!(clamp_heights(false), (36., 144.));
        assert_eq!(clamp_heights(true), (37.25, 145.25));
        assert!(!expandable(36., 36., 144., false));
        assert!(expandable(36., 90., 144., false));
        assert_eq!(expand_target(400., 144., false), 144.);
        assert_eq!(expand_target(400., 144., true), 400.);
        assert!(!expandable(0., 90., 144., false));
        assert_eq!(bar_delay(false, false), 0);
        assert_eq!(bar_delay(false, true), delay_ms(BAR_DELAY_OTHERS_MS));
        assert_eq!(bar_delay(true, true), delay_ms(BAR_DELAY_OWN_MS));
    }

    #[test]
    fn toast_goes_from_sending_to_sent_and_clears_after_its_timer() {
        let u = Id(77);
        set_toast(u, None);
        assert_eq!(toast_state().map(|t| t.sent), Some(None));
        let since = set_toast(u, Some(Interaction::Reply));
        assert_eq!(toast_text(Some(Interaction::Reply)), crate::t!("ptb.toast_reply"));
        clear_toast_if(Instant::now() + Duration::from_secs(1));
        assert!(toast_state().is_some());
        clear_toast_if(since);
        assert!(toast_state().is_none());
    }

    #[test]
    fn expand_button_toggles_the_pinned_state() {
        assert!(toggle_pinned(Id(5), Layout::Popout));
        assert!(pinned(Id(5), Layout::Popout).is_some());
        assert!(!toggle_pinned(Id(5), Layout::Popout));
        assert!(pinned(Id(5), Layout::Popout).is_none());
    }

    #[test]
    fn dms_follow_the_client_rule() {
        let g = |ids: &[u64]| ids.iter().map(|i| Id(*i)).collect::<Vec<_>>().into_iter();
        assert!(dm_allowed(false, false, true, g(&[]), &[]));
        assert!(!dm_allowed(true, false, true, g(&[1]), &[]));
        assert!(!dm_allowed(false, true, true, g(&[1]), &[]));
        assert!(!dm_allowed(false, false, false, g(&[]), &[]));
        assert!(dm_allowed(false, false, false, g(&[1]), &[]));
        assert!(!dm_allowed(false, false, false, g(&[1]), &[1]));
        assert!(dm_allowed(false, false, false, g(&[1, 2]), &[1]));
        assert!(dm_allowed(false, false, true, g(&[1]), &[1]));
    }

    #[test]
    fn easing_matches_css_ease() {
        assert!((ease(0.5) - 0.8024).abs() < 0.01);
        assert!((icon_ease(0.25) - 0.8256).abs() < 0.01);
    }
}

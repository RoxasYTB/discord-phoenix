use chrono::{Duration, Local};
use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::input::{Input, InputState};

use crate::{
    actions::{open_modal, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::{
        controls::{BtnKind, button},
        modals::{Modal, card},
        profile::{avatar_block, banner, info, name_block},
    },
};

pub const MAX_LEN: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearAfter {
    Today,
    Hours4,
    Hours1,
    Minutes30,
    Never,
}

pub const EXPIRES_KEY: &str = "custom-status-expires";

pub const STEPS: [(ClearAfter, i64); 3] = [(ClearAfter::Minutes30, 1_800_000), (ClearAfter::Hours1, 3_600_000), (ClearAfter::Hours4, 14_400_000)];

pub fn default_clear_after(expires_ms: Option<u64>, now: chrono::DateTime<Local>) -> ClearAfter {
    use chrono::{Datelike, TimeZone};
    let Some(e) = expires_ms else { return ClearAfter::Today };
    if e == 0 {
        return ClearAfter::Never;
    }
    let Some(at) = Local.timestamp_millis_opt(e as i64).single() else { return ClearAfter::Today };
    if (at.year(), at.month(), at.day()) != (now.year(), now.month(), now.day()) {
        return ClearAfter::Today;
    }
    let left = e as i64 - now.timestamp_millis();
    STEPS.iter().find(|(_, ms)| left <= *ms).map_or(ClearAfter::Today, |(c, _)| *c)
}

pub const CHOICES: [ClearAfter; 5] = [ClearAfter::Today, ClearAfter::Hours4, ClearAfter::Hours1, ClearAfter::Minutes30, ClearAfter::Never];

impl ClearAfter {
    pub fn duration(self) -> Option<Duration> {
        match self {
            Self::Today => {
                let now = Local::now();
                let next = (now.date_naive() + chrono::Days::new(1)).and_time(now.time()).and_local_timezone(Local).earliest();
                Some(next.map(|t| t - now).unwrap_or_else(|| Duration::hours(24)))
            }
            Self::Hours4 => Some(Duration::hours(4)),
            Self::Hours1 => Some(Duration::hours(1)),
            Self::Minutes30 => Some(Duration::minutes(30)),
            Self::Never => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusEmoji {
    pub id: Option<Id>,
    pub name: SharedString,
    pub animated: bool,
}

impl StatusEmoji {
    pub fn token(&self) -> String {
        match self.id {
            Some(id) => format!("<{}:{}:{}>", if self.animated { "a" } else { "" }, self.name, id.0),
            None => self.name.to_string(),
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        if let Some(e) = crate::media::emoji::parse_custom(token) {
            return Some(Self { id: Some(Id(e.id)), name: e.name, animated: e.animated });
        }
        let t = token.trim();
        (!t.is_empty()).then(|| Self { id: None, name: t.to_owned().into(), animated: false })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusDraft {
    pub text: String,
    pub emoji: Option<StatusEmoji>,
    pub expires_at_ms: u64,
    pub created_at_ms: u64,
}

pub fn draft(text: &str, emoji: Option<StatusEmoji>, clear_after: ClearAfter, now_ms: u64) -> Option<StatusDraft> {
    let text: String = text.trim().chars().take(MAX_LEN).collect();
    if text.is_empty() && emoji.is_none() {
        return None;
    }
    let expires_at_ms = clear_after.duration().map(|d| now_ms.saturating_add(d.num_milliseconds().max(0) as u64)).unwrap_or(0);
    Some(StatusDraft { text, emoji, expires_at_ms, created_at_ms: now_ms })
}

pub struct CustomStatus {
    pub input: Option<Entity<InputState>>,
    pub initial: String,
    pub clear_after: ClearAfter,
    pub select_open: bool,
    pub emoji: Option<StatusEmoji>,
}

pub fn pick_emoji(e: StatusEmoji) {
    set(|s| s.emoji = Some(e));
}

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn when_label(d: Duration, tomorrow_key: &str, today_key: &str) -> SharedString {
    let now = Local::now();
    let at = now + d;
    let time = crate::i18n::clock(at);
    if at.date_naive() == now.date_naive() { crate::t!(today_key, time = time) } else { crate::t!(tomorrow_key, time = time) }
}

pub fn selected_label(c: ClearAfter) -> SharedString {
    match c.duration() {
        Some(d) => when_label(d, "cstatus.clear_tomorrow_at", "cstatus.clear_at"),
        None => crate::t!("cstatus.dont_clear"),
    }
}

pub fn option_label(c: ClearAfter) -> SharedString {
    let span = match c {
        ClearAfter::Today => crate::t!("cstatus.hours", n = 24),
        ClearAfter::Hours4 => crate::t!("cstatus.hours", n = 4),
        ClearAfter::Hours1 => crate::t!("cstatus.hour", n = 1),
        ClearAfter::Minutes30 => crate::t!("cstatus.minutes", n = 30),
        ClearAfter::Never => return crate::t!("cstatus.dont_clear"),
    };
    let when = c.duration().map(|d| when_label(d, "cstatus.tomorrow_at", "cstatus.at")).unwrap_or_default();
    format!("{span} ({when})").into()
}

fn set(f: impl FnOnce(&mut CustomStatus)) {
    with_modal(|m| {
        if let Some(Modal::CustomStatus(s)) = m {
            f(s);
        }
    });
}

pub fn prepare(s: &mut CustomStatus, mk: &mut dyn FnMut(SharedString, &str, bool) -> Entity<InputState>) {
    if s.input.is_none() {
        let initial = s.initial.clone();
        s.input = Some(mk(crate::t!("cstatus.placeholder"), &initial, true));
    }
}

fn preview(app: &PhoenixApp, text: &str, emoji: Option<&StatusEmoji>) -> AnyElement {
    let i = info(app, app.me.id, None);
    let bubble = div()
        .absolute()
        .left(z(105.))
        .top(z(98.))
        .right(z(14.))
        .px(z(12.))
        .py(z(4.))
        .rounded(z(16.))
        .border_1()
        .border_color(tk(idx::BORDER_MUTED))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHEST))
        .shadow_sm()
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(tk(if text.is_empty() && emoji.is_none() { idx::TEXT_MUTED } else { idx::TEXT_STRONG }))
        .child(match (emoji, text.is_empty()) {
            (None, true) => crate::t!("cstatus.placeholder").into_any_element(),
            (None, false) => SharedString::from(text.to_owned()).into_any_element(),
            (Some(e), _) => {
                let line = if text.is_empty() { e.token() } else { format!("{} {text}", e.token()) };
                crate::ui::emoji_text::status_label(line.into(), 14.)
            }
        });
    div()
        .w(z(300.))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .child(
            div()
                .relative()
                .h(z(151.))
                .child(banner(&i, 105., 6.))
                .child(div().absolute().left(z(16.)).top(z(61.)).child(avatar_block(&i, 80., 6., tok(idx::BACKGROUND_SURFACE_HIGH))))
                .child(bubble),
        )
        .child(div().pt(z(8.)).px(z(16.)).pb(z(16.)).child(name_block(&i, false, None)))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, s: &CustomStatus, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let text = s.input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
    let has = !text.is_empty() || s.emoji.is_some();
    let field = div()
        .mt(z(4.))
        .relative()
        .h(z(46.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::INPUT_BORDER_DEFAULT))
        .bg(tk(idx::INPUT_BACKGROUND_DEFAULT))
        .flex()
        .items_center()
        .pl(z(48.))
        .pr(z(36.))
        .text_size(z(16.))
        .text_color(tk(idx::TEXT_DEFAULT))
        .children(s.input.as_ref().map(|i| div().flex_1().min_w_0().child(Input::new(i).appearance(false).bordered(false))))
        .child(
            div()
                .id("cstatus-emoji")
                .absolute()
                .left_0()
                .top_0()
                .h(z(46.))
                .w(z(42.))
                .pl(z(4.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(cx.listener(|a, _, w, cx| crate::ui::pickers::open_emoji_picker(a, crate::ui::pickers::EmojiTarget::Status, w, cx)))
                .child(crate::ui::pickers::status_anchor_probe())
                .child(match &s.emoji {
                    Some(e) => crate::media::emoji::emoji_element(&e.token(), 24. * crate::theme::zoom()),
                    None => gpui::svg().path(crate::icons::path("emoji")).size(z(24.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)).into_any_element(),
                }),
        )
        .when(has, |d| {
            d.child(
                div()
                    .id("cstatus-clear")
                    .absolute()
                    .right_0()
                    .top_0()
                    .h(z(46.))
                    .w(z(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .opacity(0.6)
                    .hover(|st| st.opacity(1.))
                    .on_click(cx.listener(|_, _, w, cx| {
                        set(|s| {
                            s.emoji = None;
                            if let Some(i) = s.input.clone() {
                                i.update(cx, |st, cx| st.set_value("", w, cx));
                            }
                        });
                        cx.notify();
                    }))
                    .child(gpui::svg().path(crate::icons::path("settings/circle-x")).size(z(16.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))),
            )
        });
    let body = div()
        .flex()
        .flex_col()
        .child(div().mt(z(8.)).mb(z(36.)).w_full().flex().items_center().justify_center().child(preview(app, &text, s.emoji.as_ref())))
        .child(
            div()
                .mb(z(20.))
                .relative()
                .child(
                    div()
                        .w_full()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(tk(idx::TEXT_STRONG))
                        .child(crate::t!("cstatus.label")),
                )
                .child(field),
        );
    let select = div()
        .id("cstatus-clear-after")
        .relative()
        .ml(z(-12.))
        .h(z(40.))
        .px(z(12.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(4.))
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tk(idx::TEXT_DEFAULT))
        .hover(|st| st.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
        .on_click(cx.listener(|_, _, _, cx| {
            set(|s| s.select_open = !s.select_open);
            cx.notify();
        }))
        .child(selected_label(s.clear_after))
        .child(gpui::svg().path(crate::icons::path("chevron")).size(z(16.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)))
        .when(s.select_open, |d| d.child(clear_after_menu(s.clear_after, cx)));
    let save = button("cstatus-save", crate::t!("cstatus.save"), BtnKind::Primary).px(z(16.)).on_click(cx.listener(|a, _, _, cx| a.save_custom_status(cx)));
    let footer = div().w_full().flex().items_center().justify_between().child(select).child(save);
    card(480., crate::t!("cstatus.title"), None, body.into_any_element(), Some(footer.into_any_element()), cx)
}

fn clear_after_menu(current: ClearAfter, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut menu = div()
        .id("cstatus-clear-menu")
        .occlude()
        .absolute()
        .bottom(z(44.))
        .left_0()
        .p(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
        .shadow_lg()
        .flex()
        .flex_col()
        .child(
            div()
                .min_w(z(200.))
                .px(z(8.))
                .py(z(8.))
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_MUTED))
                .child(crate::t!("cstatus.clear_after")),
        );
    for (n, ch) in CHOICES.into_iter().enumerate() {
        let sel = ch == current;
        menu = menu.child(
            div()
                .id(("cstatus-opt", n))
                .min_w(z(200.))
                .px(z(8.))
                .py(z(8.))
                .rounded(z(4.))
                .flex()
                .justify_between()
                .gap(z(8.))
                .cursor_pointer()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_STRONG))
                .hover(|st| st.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                .on_click(cx.listener(move |_, _, _, cx| {
                    set(|s| {
                        s.clear_after = ch;
                        s.select_open = false;
                    });
                    cx.notify();
                }))
                .child(option_label(ch))
                .when(sel, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(z(18.)).text_color(tk(idx::BRAND_500)))),
        );
    }
    gpui::deferred(menu).with_priority(30).into_any_element()
}

impl PhoenixApp {
    pub fn open_custom_status(&mut self, cx: &mut Context<Self>) {
        let initial = self.people.custom_status.as_ref().map(|s| s.to_string()).unwrap_or_default();
        let emoji = self
            .people
            .activities
            .get(&self.me.id)
            .and_then(|a| a.iter().find(|x| x.kind == crate::presence::ActivityKind::Custom))
            .and_then(|a| a.emoji.as_deref())
            .and_then(StatusEmoji::from_token);
        self.people.close_overlays();
        let clear_after = default_clear_after(self.prefs.synced.proto.get(EXPIRES_KEY).copied(), Local::now());
        open_modal(Modal::CustomStatus(CustomStatus { input: None, initial, clear_after, select_open: false, emoji }), None);
        cx.notify();
    }

    pub(crate) fn save_custom_status(&mut self, cx: &mut Context<Self>) {
        let mut out = None;
        set(|s| {
            let text = s.input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
            out = Some(draft(&text, s.emoji.clone(), s.clear_after, chrono::Utc::now().timestamp_millis().max(0) as u64));
        });
        let Some(d) = out else { return };
        crate::actions::close_modal();
        self.people.custom_status = d.as_ref().map(|d| d.text.clone()).filter(|t| !t.is_empty()).map(Into::into);
        match &d {
            Some(d) => {
                self.prefs.synced.proto.insert(EXPIRES_KEY, d.expires_at_ms);
            }
            None => {
                self.prefs.synced.proto.remove(EXPIRES_KEY);
            }
        }
        cx.background_spawn(self.backend.save_custom_status(self.presence, d)).detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_carries_the_clear_after_deadline() {
        let now = 1_764_085_600_000;
        let d = draft("  Lunch  ", None, ClearAfter::Hours4, now).unwrap();
        assert_eq!(d.text, "Lunch");
        assert_eq!(d.expires_at_ms, now + 4 * 3_600_000);
        assert_eq!(d.created_at_ms, now);
        assert_eq!(draft("x", None, ClearAfter::Minutes30, now).unwrap().expires_at_ms, now + 1_800_000);
        assert_eq!(draft("x", None, ClearAfter::Hours1, now).unwrap().expires_at_ms, now + 3_600_000);
        assert_eq!(draft("x", None, ClearAfter::Never, now).unwrap().expires_at_ms, 0);
        let today = draft("x", None, ClearAfter::Today, now).unwrap().expires_at_ms - now;
        assert!((23 * 3_600_000..=25 * 3_600_000).contains(&today));
        assert_eq!(draft("   ", None, ClearAfter::Today, now), None);
        let e = StatusEmoji { id: None, name: "\u{1f354}".into(), animated: false };
        let only = draft("", Some(e.clone()), ClearAfter::Never, now).unwrap();
        assert_eq!((only.text.as_str(), only.emoji), ("", Some(e)));
        assert_eq!(draft(&"a".repeat(200), None, ClearAfter::Never, now).unwrap().text.len(), MAX_LEN);
    }

    #[test]
    fn emoji_tokens_round_trip() {
        let c = StatusEmoji::from_token("<a:dance:42>").unwrap();
        assert_eq!(c, StatusEmoji { id: Some(Id(42)), name: "dance".into(), animated: true });
        assert_eq!(c.token(), "<a:dance:42>");
        assert_eq!(StatusEmoji::from_token("\u{1f980}").unwrap().token(), "\u{1f980}");
        assert_eq!(StatusEmoji::from_token(" "), None);
    }

    #[test]
    fn default_duration_reads_the_current_expiry_like_the_client() {
        use chrono::TimeZone;
        let now = Local.with_ymd_and_hms(2026, 11, 25, 10, 0, 0).unwrap();
        let ms = |m: i64| Some((now.timestamp_millis() + m * 60_000) as u64);
        assert_eq!(default_clear_after(None, now), ClearAfter::Today);
        assert_eq!(default_clear_after(Some(0), now), ClearAfter::Never);
        assert_eq!(default_clear_after(ms(20), now), ClearAfter::Minutes30);
        assert_eq!(default_clear_after(ms(45), now), ClearAfter::Hours1);
        assert_eq!(default_clear_after(ms(200), now), ClearAfter::Hours4);
        assert_eq!(default_clear_after(ms(300), now), ClearAfter::Today);
        assert_eq!(default_clear_after(ms(24 * 60), now), ClearAfter::Today);
    }
}

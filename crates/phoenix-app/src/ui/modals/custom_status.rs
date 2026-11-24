//! "Set your status" modal (client "Edit Custom Status Modal"), opened from the
//! pencil on the account popout's status bubble.
//!
//! Live client, 2026-11-24 (opened and closed with Escape, nothing saved):
//! `container_dvN-MP size-md` 480 wide; header "Définir ton statut"; a 304x239
//! profile preview centred with margin 8 0 36 whose thought bubble follows the
//! text; "Statut" heading-sm/semibold label, then the 430px text area (mt 4,
//! padding 12 32 12 48, 1px --input-border-active when focused, radius 8,
//! 16px text, 2 lines) with a 46px emoji button at its start and a 40x46 clear
//! button (16px circle-x) at its end; the action bar holds the "Clear after"
//! select (margin-start -12, padding 8 8 8 12, body-md/medium, 18px chevron)
//! and the primary "Save" button.

use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*, svg};
use gpui_component::input::{Textarea, TextareaState};

use super::{Modal, card};
use crate::{
    actions::{close_modal, with_modal},
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

/// "Clear after" choices: label key and lifetime (`None` = end of today, `Some(0)` = never).
pub const CLEAR_AFTER: [(&str, Option<i64>); 5] = [
    ("status.clear_today", None),
    ("status.clear_4h", Some(4 * 3600)),
    ("status.clear_1h", Some(3600)),
    ("status.clear_30m", Some(1800)),
    ("status.clear_never", Some(0)),
];

pub struct State {
    pub initial: String,
    pub input: Option<Entity<TextareaState>>,
    /// Index in [`CLEAR_AFTER`].
    pub clear_after: usize,
    pub select_open: bool,
}

impl State {
    /// An existing status opens on "Don't clear" (live client); a new one on "Today".
    pub fn new(current: Option<&str>) -> Self {
        let initial = current.unwrap_or_default().to_owned();
        let clear_after = if initial.is_empty() { 0 } else { CLEAR_AFTER.len() - 1 };
        Self { initial, input: None, clear_after, select_open: false }
    }
}

/// ISO 8601 expiry for choice `ix` at `now` (local time for "today").
pub fn expires_at(ix: usize, now: chrono::DateTime<chrono::Local>) -> Option<String> {
    let end = match CLEAR_AFTER.get(ix)?.1 {
        Some(0) => return None,
        Some(secs) => now + chrono::Duration::seconds(secs),
        None => (now.date_naive() + chrono::Days::new(1)).and_hms_opt(0, 0, 0)?.and_local_timezone(chrono::Local).earliest()?,
    };
    Some(end.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

fn with_state(f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::CustomStatus(s)) = m {
            f(s)
        }
    });
}

pub fn prepare(s: &mut State, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if s.input.is_some() {
        return;
    }
    let initial = s.initial.clone();
    let i = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 2).placeholder(crate::t!("status.custom_ask")).default_value(initial));
    i.update(cx, |s, cx| s.focus(window, cx));
    cx.observe(&i, |_, _, cx| cx.notify()).detach();
    s.input = Some(i);
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let text = s.input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
    let me = app.me.id;
    let i = crate::ui::profile::info(app, me, None);
    let surface = tok(idx::BACKGROUND_SURFACE_HIGH);
    // profilePreview_: the account card, 304x239 (outer_P1YEjQ padding 2, radius 8).
    let preview = div()
        .mt(z(8.))
        .mb(z(36.))
        .flex()
        .justify_center()
        .child(
            div()
                .w(z(304.))
                .p(z(2.))
                .rounded(z(8.))
                .bg(c(surface))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(
                    div()
                        .relative()
                        .h(z(151.))
                        .child(crate::ui::profile::banner(&i, 105., 6.))
                        .child(div().absolute().left(z(8.)).top(z(53.)).child(crate::ui::profile::avatar_block(&i, 80., 6., surface)))
                        // referenceContainer_rmCN9_: margin -10 12 0 109 from the banner bottom.
                        .when(!text.trim().is_empty(), |d| {
                            d.child(div().absolute().left(z(103.)).top(z(96.)).child(crate::ui::profile::status_bubble(text.trim().to_owned().into())))
                        }),
                )
                .child(div().pt(z(8.)).px(z(16.)).pb(z(16.)).child(crate::ui::profile::name_block(&i, false, None))),
        );
    // Emoji button (46px, 24px greyscale smile) and clear button (40x46) over the text area.
    let emoji = div()
        .id("cs-emoji")
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .w(z(46.))
        .pl(z(4.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(svg().path(crate::icons::path("emoji")).size(z(24.)).text_color(c(tok(idx::ICON_MUTED))));
    let clear = div()
        .id("cs-clear")
        .absolute()
        .right_0()
        .top_0()
        .w(z(40.))
        .h(z(46.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(svg().path(crate::icons::path("circle-x")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
        .on_click(cx.listener(|_, _, window, cx| {
            let input = with_modal(|m| match m {
                Some(Modal::CustomStatus(s)) => s.input.clone(),
                _ => None,
            });
            if let Some(i) = input {
                i.update(cx, |s, cx| s.set_value("", window, cx));
            }
            cx.notify();
        }));
    let field = div()
        .flex()
        .flex_col()
        .mb(z(20.))
        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(crate::t!("status.label")))
        .child(
            div()
                .mt(z(4.))
                .relative()
                .min_h(z(46.))
                .pt(z(12.))
                .pb(z(12.))
                .pl(z(48.))
                .pr(z(32.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::INPUT_BORDER_ACTIVE)))
                .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                .text_size(z(16.))
                .line_height(z(22.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .children(s.input.as_ref().map(|i| Textarea::new(i).appearance(false).bordered(false)))
                .child(emoji)
                .when(!text.is_empty(), |d| d.child(clear)),
        );
    let body = div().flex().flex_col().child(preview).child(field);

    // Clear-after select: a borderless control with the value and a chevron; its menu lists the choices.
    let sel = s.clear_after;
    let mut select = div()
        .id("cs-clear-after")
        .relative()
        .ml(z(-12.))
        .pt(z(8.))
        .pb(z(8.))
        .pl(z(12.))
        .pr(z(8.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(8.))
        .cursor_pointer()
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(crate::t!(CLEAR_AFTER[sel.min(CLEAR_AFTER.len() - 1)].0))
        .child(svg().path(crate::icons::path("chevron")).size(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))))
        .on_click(cx.listener(|_, _, _, cx| {
            with_state(|s| s.select_open = !s.select_open);
            cx.notify();
        }));
    if s.select_open {
        let mut menu = div()
            .absolute()
            .left_0()
            .bottom(z(44.))
            .w(z(200.))
            .p(z(8.))
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .shadow_lg()
            .flex()
            .flex_col()
            .occlude();
        for (n, (k, _)) in CLEAR_AFTER.iter().enumerate() {
            menu = menu.child(
                div()
                    .id(("cs-clear-opt", n))
                    .px(z(8.))
                    .py(z(8.))
                    .rounded(z(4.))
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(if n == sel { idx::TEXT_STRONG } else { idx::TEXT_DEFAULT })))
                    .when(n == sel, |d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                    .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                    .cursor_pointer()
                    .child(crate::t!(k))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        with_state(|s| {
                            s.clear_after = n;
                            s.select_open = false;
                        });
                        cx.notify();
                    })),
            );
        }
        select = select.child(menu);
    }
    let save = button("cs-save", crate::t!("common.save"), BtnKind::Primary).on_click(cx.listener(|a, _, _, cx| {
        let r = with_modal(|m| match m {
            Some(Modal::CustomStatus(s)) => Some((s.input.as_ref().map(|i| i.read(cx).value().trim().to_owned()).unwrap_or_default(), s.clear_after)),
            _ => None,
        });
        close_modal();
        if let Some((t, ix)) = r {
            a.set_custom_status_until((!t.is_empty()).then_some(t), expires_at(ix, chrono::Local::now()), cx);
        }
        cx.notify();
    }));
    let footer = div().w_full().flex().items_center().justify_between().child(select).child(save);
    card(480., crate::t!("status.set_title"), None, body.into_any_element(), Some(footer.into_any_element()), cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn clear_after_expiry() {
        let now = chrono::Local.with_ymd_and_hms(2026, 11, 24, 15, 0, 0).unwrap();
        assert_eq!(expires_at(CLEAR_AFTER.len() - 1, now), None);
        let in_1h = expires_at(2, now).unwrap();
        assert_eq!(chrono::DateTime::parse_from_rfc3339(&in_1h).unwrap(), now + chrono::Duration::hours(1));
        let today = chrono::DateTime::parse_from_rfc3339(&expires_at(0, now).unwrap()).unwrap().with_timezone(&chrono::Local);
        assert_eq!(today.date_naive(), now.date_naive().succ_opt().unwrap());
        assert_eq!(today.format("%H:%M").to_string(), "00:00");
    }

    #[test]
    fn new_status_defaults_to_today() {
        assert_eq!(State::new(None).clear_after, 0);
        assert_eq!(State::new(Some("busy")).clear_after, CLEAR_AFTER.len() - 1);
    }
}

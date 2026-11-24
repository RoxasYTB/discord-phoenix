use gpui::{AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use parking_lot::RwLock;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{self, BtnKind},
};

type Cx<'a> = Context<'a, PhoenixApp>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountInfo {
    pub email: Option<String>,
    pub phone: Option<String>,
    pub mfa: bool,
    pub adult: Option<bool>,
}

static INFO: RwLock<Option<AccountInfo>> = RwLock::new(None);
static SESSIONS: RwLock<Option<usize>> = RwLock::new(None);

pub fn set_from_user(u: &phoenix_api::models::User) {
    *INFO.write() = Some(AccountInfo {
        email: u.email.clone().filter(|e| !e.is_empty()),
        phone: u.phone.clone().filter(|p| !p.is_empty()),
        mfa: u.mfa_enabled.unwrap_or(false),
        adult: u.nsfw_allowed,
    });
}

pub fn info() -> AccountInfo {
    INFO.read().clone().unwrap_or_default()
}

pub fn fetch_sessions(app: &PhoenixApp, cx: &mut Cx) {
    let fut = app.backend.session_count();
    cx.spawn(async move |this, cx| {
        if let Ok(Some(n)) = fut.await {
            *SESSIONS.write() = Some(n);
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

pub fn mask_email(e: &str) -> String {
    match e.split_once('@') {
        Some((local, domain)) => format!("{}@{domain}", "*".repeat(local.chars().count())),
        None => "*".repeat(e.chars().count()),
    }
}

pub fn mask_phone(p: &str) -> String {
    let n = p.chars().count();
    p.chars().enumerate().map(|(i, ch)| if i + 4 < n { '*' } else { ch }).collect()
}

pub(super) fn value(t: impl Into<SharedString>) -> Div {
    div()
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .whitespace_nowrap()
        .child(t.into())
}

pub(super) fn value_chevron(v: Option<SharedString>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(z(16.))
        .children(v.map(value))
        .child(gpui::svg().path(crate::icons::path("chevron-right")).size(z(24.)).text_color(c(tok(idx::TEXT_DEFAULT))))
        .into_any_element()
}

pub(super) fn reveal_button(id: &'static str, shown: bool, on: impl Fn(&mut PhoenixApp) + 'static, cx: &mut Cx) -> AnyElement {
    div()
        .id(id)
        .text_size(z(16.))
        .line_height(z(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_LINK)))
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| {
            on(a);
            cx.notify();
        }))
        .child(if shown { crate::t!("common.hide") } else { crate::t!("common.reveal") })
        .into_any_element()
}

pub(super) fn linked_desc(t: SharedString) -> AnyElement {
    let link = gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() };
    let (plain, hl) = bracket_runs(&t, link);
    div()
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(c(tok(idx::TEXT_SUBTLE)))
        .child(gpui::StyledText::new(plain).with_highlights(hl))
        .into_any_element()
}

fn leading_item(icon: &'static str, bg: gpui::Hsla, title: SharedString, desc: SharedString, trailing: AnyElement) -> AnyElement {
    div()
        .flex()
        .gap(z(12.))
        .mx(z(-12.))
        .p(z(12.))
        .min_h(z(48.))
        .rounded(z(16.))
        .child(
            div().w(z(48.)).flex_none().flex().items_center().child(
                div()
                    .size(z(48.))
                    .rounded(z(8.))
                    .bg(bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(gpui::svg().path(crate::icons::path(icon)).size(z(24.)).text_color(c(tok(idx::TEXT_DEFAULT)))),
            ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .justify_center()
                .gap(z(4.))
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(title))
                .child(linked_desc(desc)),
        )
        .child(div().flex_none().flex().items_center().child(trailing))
        .into_any_element()
}

pub(super) fn standing_rows(info: &AccountInfo) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if let Some(adult) = info.adult {
        rows.push(leading_item(
            "user-circle",
            c(tok(idx::BACKGROUND_MOD_MUTED)),
            crate::t!("settings.account.age_group"),
            if adult { crate::t!("settings.account.age_adult_d") } else { crate::t!("settings.account.age_teen_d") },
            value(if adult { crate::t!("settings.account.age_adult") } else { crate::t!("settings.account.age_teen") }).into_any_element(),
        ));
    }
    let mut ok = c(tok(idx::STATUS_POSITIVE));
    ok.a = 0.078;
    rows.push(leading_item(
        "check",
        ok,
        crate::t!("settings.account.status"),
        crate::t!("settings.account.standing_d"),
        value_chevron(Some(crate::t!("settings.account.all_good"))),
    ));
    rows
}

pub(super) fn removal_rows() -> Vec<AnyElement> {
    let disable = controls::button("acct-disable", crate::t!("settings.account.disable_btn"), BtnKind::Secondary)
        .text_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT)))
        .bg(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BACKGROUND_DEFAULT)))
        .border_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BORDER_DEFAULT)));
    let delete = controls::button("acct-delete", crate::t!("settings.account.delete_btn"), BtnKind::Critical);
    vec![
        controls::control_item(crate::t!("settings.account.disable"), Some(crate::t!("settings.account.disable_d")), Some(disable.into_any_element()))
            .into_any_element(),
        controls::control_item(crate::t!("settings.account.close"), Some(crate::t!("settings.account.close_d")), Some(delete.into_any_element()))
            .into_any_element(),
    ]
}

static ACTIVITY_RESTRICTED: RwLock<Vec<u64>> = RwLock::new(Vec::new());

pub fn set_activity_restricted(ids: Vec<u64>) {
    *ACTIVITY_RESTRICTED.write() = ids;
}

pub fn activity_restricted() -> Vec<u64> {
    ACTIVITY_RESTRICTED.read().clone()
}

pub(super) fn bold_desc(t: SharedString) -> AnyElement {
    let strong = gpui::HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() };
    let (plain, hl) = bracket_runs(&t, strong);
    div()
        .text_size(z(14.))
        .line_height(z(18.))
        .text_color(c(tok(idx::TEXT_SUBTLE)))
        .child(gpui::StyledText::new(plain).with_highlights(hl))
        .into_any_element()
}

fn bracket_runs(t: &str, style: gpui::HighlightStyle) -> (String, Vec<(std::ops::Range<usize>, gpui::HighlightStyle)>) {
    let mut plain = String::new();
    let mut hl = Vec::new();
    let mut start = None;
    for ch in t.chars() {
        match ch {
            '[' => start = Some(plain.len()),
            ']' => {
                if let Some(s) = start.take() {
                    hl.push((s..plain.len(), style));
                }
            }
            _ => plain.push(ch),
        }
    }
    (plain, hl)
}

static CONNECTIONS: RwLock<Option<usize>> = RwLock::new(None);

pub fn set_connections(n: usize) {
    *CONNECTIONS.write() = Some(n);
}

pub fn connections() -> Option<usize> {
    *CONNECTIONS.read()
}

pub fn sessions() -> Option<usize> {
    *SESSIONS.read()
}

#[cfg(test)]
mod tests {
    #[test]
    fn masks_like_the_client() {
        assert_eq!(super::mask_email("abc@gmail.com"), "***@gmail.com");
        assert_eq!(super::mask_phone("+33612344599"), "********4599");
        assert_eq!(super::mask_phone("12"), "12");
    }
}

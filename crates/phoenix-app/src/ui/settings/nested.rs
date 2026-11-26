use gpui::{AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, relative};

use super::sessions::{self, Session};
use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{self, BtnKind},
};

type Cx<'a> = Context<'a, PhoenixApp>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nested {
    Mfa,
    Devices,
    Standing,
    FamilyCenter,
}

impl Nested {
    pub fn title(self) -> SharedString {
        match self {
            Self::Mfa => crate::t!("settings.account.mfa"),
            Self::Devices => crate::t!("settings.devices.title"),
            Self::Standing => crate::t!("settings.account.status"),
            Self::FamilyCenter => crate::t!("settings.account.family"),
        }
    }

    pub fn sub_index(self) -> usize {
        match self {
            Self::Mfa | Self::Devices => 1,
            Self::Standing => 2,
            Self::FamilyCenter => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Standing {
    #[default]
    AllGood,
    Limited,
    VeryLimited,
    AtRisk,
    Suspended,
}

impl Standing {
    pub const ALL: [Self; 5] = [Self::AllGood, Self::Limited, Self::VeryLimited, Self::AtRisk, Self::Suspended];

    pub fn from_state(n: u64) -> Self {
        match n {
            200 => Self::Limited,
            300 => Self::VeryLimited,
            400 => Self::AtRisk,
            500 => Self::Suspended,
            _ => Self::AllGood,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::AllGood => "all_good",
            Self::Limited => "limited",
            Self::VeryLimited => "very_limited",
            Self::AtRisk => "at_risk",
            Self::Suspended => "suspended",
        }
    }

    pub fn label(self) -> SharedString {
        crate::t!(&format!("settings.standing.{}", self.key()))
    }

    pub fn title(self) -> SharedString {
        crate::t!(&format!("settings.standing.{}_t", self.key()))
    }

    pub fn description(self) -> SharedString {
        match self {
            Self::AllGood => crate::t!("settings.account.standing_d"),
            s => crate::t!(&format!("settings.standing.{}_d", s.key())),
        }
    }

    fn color(self) -> gpui::Hsla {
        c(tok(match self {
            Self::AllGood => idx::STATUS_POSITIVE,
            Self::Limited => idx::STATUS_WARNING,
            Self::VeryLimited => idx::TEXT_FEEDBACK_WARNING,
            Self::AtRisk => idx::ICON_FEEDBACK_CRITICAL,
            Self::Suspended => idx::ICON_MUTED,
        }))
    }

    fn icon(self) -> &'static str {
        match self {
            Self::AllGood => "circle-check",
            Self::Suspended => "settings/circle-x",
            _ => "settings/circle-error",
        }
    }
}

fn text(size: f32, line: f32, weight: FontWeight, color: usize, t: impl Into<SharedString>) -> Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(c(tok(color))).child(t.into())
}

fn cat_header(title: SharedString, subtitle: Option<AnyElement>, trailing: Option<AnyElement>) -> Div {
    div()
        .pb(z(24.))
        .flex()
        .items_center()
        .gap(z(24.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(text(24., 32., FontWeight::MEDIUM, idx::TEXT_STRONG, title))
                .children(subtitle),
        )
        .children(trailing.map(|t| div().flex_none().flex().gap(z(16.)).child(t)))
}

fn subtitle(t: SharedString) -> AnyElement {
    text(14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE, t).into_any_element()
}

fn divider() -> Div {
    div().my(z(50.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))
}

fn links(t: SharedString, color: usize, size: f32, line: f32) -> AnyElement {
    let link = gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() };
    let mut plain = String::new();
    let mut hl = Vec::new();
    let mut start = None;
    for ch in t.chars() {
        match ch {
            '[' => start = Some(plain.len()),
            ']' => {
                if let Some(s) = start.take() {
                    hl.push((s..plain.len(), link));
                }
            }
            _ => plain.push(ch),
        }
    }
    div()
        .text_size(z(size))
        .line_height(z(line))
        .text_color(c(tok(color)))
        .child(gpui::StyledText::new(plain).with_highlights(hl))
        .into_any_element()
}

fn critical_secondary(id: &'static str, label: SharedString) -> gpui::Stateful<Div> {
    controls::button(id, label, BtnKind::Secondary)
        .text_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT)))
        .bg(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BACKGROUND_DEFAULT)))
        .border_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BORDER_DEFAULT)))
}

pub fn open(app: &mut PhoenixApp, n: Nested, cx: &mut Cx) {
    if let Some(s) = app.settings.as_mut() {
        s.nested = Some(n);
        s.scroll.set_offset(gpui::Point::default());
    }
    cx.notify();
}

pub fn close(app: &mut PhoenixApp, cx: &mut Cx) {
    if let Some(s) = app.settings.as_mut() {
        s.nested = None;
        s.scroll.set_offset(gpui::Point::default());
    }
    reset();
    cx.notify();
}

pub fn render(app: &PhoenixApp, n: Nested, cx: &mut Cx) -> Div {
    match n {
        Nested::Devices => devices(cx),
        Nested::Standing => standing(app),
        Nested::Mfa => mfa(app, cx),
        Nested::FamilyCenter => family_center(app, cx),
    }
}

fn session_row(icon: &'static str, muted: bool, label: SharedString, sub: Option<SharedString>, desc: AnyElement, trailing: Option<AnyElement>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(z(16.))
        .child(
            div()
                .flex_none()
                .w(z(48.))
                .h(z(48.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path(icon)).size(z(24.)).flex_none().text_color(c(tok(if muted { idx::ICON_MUTED } else { idx::TEXT_DEFAULT })))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(
                    div()
                        .flex()
                        .gap(z(10.))
                        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, label))
                        .children(sub.map(|s| {
                            div()
                                .flex()
                                .gap(z(10.))
                                .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_SUBTLE, "\u{2022}"))
                                .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_SUBTLE, s))
                        })),
                )
                .child(desc),
        )
        .children(trailing)
}

fn session(s: &Session, current: bool, now: i64, cx: &mut Cx) -> Div {
    let (os, kind) = sessions::os_label(s.os.as_deref());
    let desc = text(14., 18., FontWeight::NORMAL, idx::TEXT_MUTED, sessions::description(s, current, now)).into_any_element();
    let hash = s.id_hash.clone();
    let logout = (!current).then(|| {
        div()
            .id(gpui::ElementId::Name(format!("session-logout-{hash}").into()))
            .group("session-logout")
            .ml_auto()
            .flex_none()
            .h(z(26.))
            .cursor_pointer()
            .on_click(cx.listener(move |a, _, _, cx| sessions::logout(a, vec![hash.clone()], cx)))
            .child(
                gpui::svg()
                    .path(crate::icons::path("close-small"))
                    .size(z(24.))
                    .text_color(c(tok(idx::INTERACTIVE_ICON_DEFAULT)))
                    .group_hover("session-logout", |st| st.text_color(c(tok(idx::INTERACTIVE_ICON_HOVER)))),
            )
            .into_any_element()
    });
    session_row(kind.icon(), false, os, s.platform.clone().filter(|p| !p.is_empty()).map(Into::into), desc, logout)
}

fn group(title: SharedString, rows: Vec<AnyElement>) -> Div {
    div()
        .py(z(18.))
        .flex()
        .flex_col()
        .gap(z(30.))
        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_MUTED, title))
        .child(div().flex().flex_col().gap(z(30.)).children(rows))
}

fn devices(cx: &mut Cx) -> Div {
    let now = chrono::Utc::now().timestamp();
    let list = sessions::list().unwrap_or_default();
    let (cur, others) = sessions::split(&list, sessions::current_hash().as_deref());
    let mfa = super::account::info().mfa;
    let mut body = div().flex().flex_col().gap(z(10.));
    if let Some(s) = cur.as_ref() {
        body = body.child(group(crate::t!("settings.devices.current"), vec![session(s, true, now, cx).into_any_element()]));
    }
    if !others.is_empty() || !mfa {
        let mut rows: Vec<AnyElement> = others.iter().map(|s| session(s, false, now, cx).into_any_element()).collect();
        if !mfa {
            let hint = links(crate::t!("settings.devices.old_hint_d"), idx::TEXT_MUTED, 14., 18.);
            rows.push(
                session_row("settings/laptop-phone", true, crate::t!("settings.devices.old_hint"), None, hint, None)
                    .id("session-old-hint")
                    .cursor_pointer()
                    .on_click(cx.listener(|a, _, _, cx| close(a, cx)))
                    .into_any_element(),
            );
        }
        body = body.child(group(crate::t!("settings.devices.others"), rows));
    }
    if !others.is_empty() {
        let hashes: Vec<String> = others.iter().map(|s| s.id_hash.clone()).collect();
        body = body.child(
            div()
                .py(z(10.))
                .flex()
                .flex_col()
                .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("settings.devices.logout_all")))
                .child(div().mt(z(4.)).child(subtitle(crate::t!("settings.devices.logout_all_d"))))
                .child(
                    div().mt(z(8.)).flex().child(
                        controls::button_sm("session-logout-all", crate::t!("settings.devices.logout_all_btn"), BtnKind::Critical)
                            .on_click(cx.listener(move |a, _, _, cx| sessions::logout(a, hashes.clone(), cx))),
                    ),
                ),
        );
    }
    div()
        .flex()
        .flex_col()
        .child(cat_header(crate::t!("settings.devices.title"), Some(subtitle(crate::t!("settings.devices.subtitle"))), None))
        .child(div().h(z(1.)))
        .child(body)
}

fn standing(app: &PhoenixApp) -> Div {
    let state = super::account::standing();
    let hook = gpui::HighlightStyle { color: Some(state.color()), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() };
    let raw = state.title();
    let (plain, hl) = bracketed(&raw, hook);
    let title = div()
        .text_size(z(20.))
        .line_height(z(24.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_STRONG)))
        .child(gpui::StyledText::new(plain).with_highlights(hl));
    let n = Standing::ALL.len();
    let lefts = [0., 0.175, 0.425, 0.675, 0.85];
    let option = |i: usize, s: Standing| {
        let active = s == state;
        let marker = if active {
            div()
                .size(z(28.))
                .rounded_full()
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .child(gpui::svg().path(crate::icons::path(s.icon())).size(z(28.)).text_color(s.color()))
        } else {
            div()
                .size(z(28.))
                .rounded_full()
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .flex()
                .items_center()
                .justify_center()
                .when(i == 0, |d| d.ml(z(-6.)))
                .when(i + 1 == n, |d| d.mr(z(-6.)))
                .child(div().size(z(16.)).rounded_full().bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
        };
        let label = text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, s.label()).max_w_full().overflow_hidden();
        let col = div().w(relative(0.15)).flex().flex_col().gap(z(8.));
        let col = match i {
            0 => col.items_start(),
            x if x + 1 == n => col.items_end().text_right(),
            _ => col.items_center().text_center(),
        };
        col.child(marker).child(label)
    };
    let sizer = div().invisible().w_full().flex().children(Standing::ALL.iter().enumerate().map(|(i, s)| option(i, *s)));
    let health = div()
        .relative()
        .w_full()
        .child(sizer)
        .child(div().absolute().top(z(12.)).left_0().w_full().h(z(4.)).bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
        .children(Standing::ALL.iter().enumerate().map(|(i, s)| option(i, *s).absolute().top_0().left(relative(lefts[i]))));
    div().flex().flex_col().pt(z(11.)).pb(z(10.)).child(
        div()
            .pt(z(8.))
            .mb(z(32.))
            .flex()
            .gap(z(16.))
            .child(
                div().ml(z(16.)).flex().justify_center().child(
                    div()
                        .size(z(92.))
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_MOD_STRONG)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::ui::avatar(Some(&app.me), 80.)),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(z(24.))
                    .child(div().flex().flex_col().gap(z(8.)).child(title).child(links(state.description(), idx::TEXT_DEFAULT, 14., 18.)))
                    .child(health),
            ),
    )
}

fn bracketed(t: &str, style: gpui::HighlightStyle) -> (String, Vec<(std::ops::Range<usize>, gpui::HighlightStyle)>) {
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

fn mfa(app: &PhoenixApp, cx: &mut Cx) -> Div {
    let info = super::account::info();
    let streamer = app.prefs.local.streamer_hides_info();
    let add_key = controls::button_icon("mfa-add-key", "plus", crate::t!("settings.mfa.add_key"), BtnKind::Secondary).on_click(cx.listener(|a, _, _, cx| {
        let fut = a.backend.guarded_write(crate::actions::guard::GuardedWrite::start_webauthn_register());
        cx.spawn(async move |this, cx| {
            if fut.await.is_err() {
                let _ = this.update(cx, |a, cx| a.parity_toast(crate::t!("settings.mfa.key_error"), cx));
            }
        })
        .detach();
    }));
    let keys = security_keys(app, cx);
    let mut out = div()
        .flex()
        .flex_col()
        .child(cat_header(crate::t!("settings.mfa.keys"), Some(subtitle(crate::t!("settings.mfa.keys_d"))), Some(add_key.into_any_element())))
        .child(div().h(z(1.)))
        .child(keys)
        .child(divider())
        .child(cat_header(crate::t!("settings.mfa.app"), Some(subtitle(crate::t!("settings.mfa.app_d"))), None))
        .child(div().h(z(1.)));
    if info.mfa {
        out = out.child(
            div().child(controls::control_item(
                crate::t!("settings.account.mfa_on"),
                None,
                Some(
                    critical_secondary("mfa-app-remove", crate::t!("settings.mfa.remove"))
                        .on_click(cx.listener(|a, _, w, cx| a.open_account_edit(crate::ui::modals::account_edit::Flow::RemoveTotp, w, cx)))
                        .into_any_element(),
                ),
            )),
        );
    }
    if info.mfa && info.sms {
        let shown = SHOW_PHONE.with(std::cell::Cell::get) && !streamer;
        let phone = info.phone.clone().unwrap_or_default();
        let digits: String = if shown { phone } else { phone.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect() };
        let line = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(z(4.))
            .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE, crate::t!(if shown { "settings.mfa.sms_full" } else { "settings.mfa.sms_ending" }, phone = digits)))
            .child(
                div()
                    .id("mfa-sms-reveal")
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_LINK)))
                    .cursor_pointer()
                    .on_click(cx.listener(|_, _, _, cx| {
                        SHOW_PHONE.with(|c| c.set(!c.get()));
                        cx.notify();
                    }))
                    .child(if shown { crate::t!("common.hide") } else { crate::t!("common.reveal") }),
            );
        out = out
            .child(divider())
            .child(cat_header(crate::t!("settings.mfa.sms"), None, None))
            .child(div().h(z(1.)))
            .child(
                div()
                    .flex()
                    .gap(z(12.))
                    .mx(z(-12.))
                    .p(z(12.))
                    .rounded(z(16.))
                    .child(div().flex_1().min_w_0().flex().flex_col().gap(z(4.)).child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("settings.account.mfa_on"))).child(line))
                    .child(div().flex_none().flex().items_center().child(critical_secondary("mfa-sms-remove", crate::t!("settings.mfa.remove")).on_click(cx.listener(|a, _, w, cx| {
                        a.open_account_edit(crate::ui::modals::account_edit::Flow::Confirm(crate::ui::modals::account_edit::Confirm::DisableSms), w, cx)
                    })))),
            );
    }
    if info.mfa {
        out = out
            .child(divider())
            .child(cat_header(crate::t!("settings.mfa.backup"), Some(subtitle(crate::t!("settings.mfa.backup_d"))), None))
            .child(div().h(z(1.)))
            .child(div().py(z(10.)).flex().child(controls::button_sm("mfa-backup", crate::t!("settings.mfa.backup_btn"), BtnKind::Secondary).on_click(cx.listener(|a, _, w, cx| {
                a.open_account_edit(crate::ui::modals::account_edit::Flow::Confirm(crate::ui::modals::account_edit::Confirm::BackupCodes), w, cx)
            }))));
    }
    out
}

const FAMILY_ARTICLE: &str = "https://support.discord.com/hc/articles/14155043715735";
const FAMILY_HERO_ART: &str = "https://discord.com/assets/bc9100f5e54f39a8.svg";
const FAMILY_LINK_ART: &str = "https://cdn.discordapp.com/assets/content/bb4093f4ae937ad09bc359b162988c3cbb7d49d924073fee7d66a763e738fc67.svg";

fn family_box() -> Div {
    div().rounded(z(12.)).border_1().border_color(c(tok(idx::BORDER_SUBTLE))).bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
}

fn family_tabs(tab: usize, cx: &mut Cx) -> Div {
    let tabs = ["settings.family.tab_activity", "settings.family.tab_family", "settings.family.tab_settings"];
    div().mb(z(24.)).border_b_1().border_color(c(tok(idx::BORDER_SUBTLE))).flex().children(tabs.into_iter().enumerate().map(|(ix, key)| {
        let active = ix == tab;
        let bar = |color: usize| div().absolute().left_0().right_0().bottom(z(-1.)).h(z(2.)).rounded_t(z(2.)).bg(c(tok(color)));
        div()
            .id(("family-tab", ix))
            .group("family-tab")
            .relative()
            .when(ix > 0, |d| d.ml(z(30.)))
            .pb(z(16.))
            .text_size(z(14.))
            .line_height(z(14.))
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .text_color(c(tok(if active { idx::TEXT_BRAND } else { idx::TEXT_SUBTLE })))
            .when(!active, |d| d.cursor_pointer().hover(|s| s.text_color(c(tok(idx::TEXT_STRONG)))))
            .child(crate::t!(key))
            .when(active, |d| d.child(bar(idx::TEXT_BRAND)))
            .when(!active, |d| d.child(bar(idx::BRAND_500).invisible().group_hover("family-tab", |s| s.visible())))
            .on_click(cx.listener(move |_, _, _, cx| {
                FAMILY_TAB.with(|t| t.set(ix));
                cx.notify();
            }))
    }))
}

fn family_hero(cx: &mut Cx) -> Div {
    let body = crate::t!("settings.family.hero_body");
    let link = crate::t!("settings.family.learn_more");
    let full = format!("{body} {link}");
    let range = body.len() + 1..full.len();
    let styled = gpui::StyledText::new(full).with_highlights([(range.clone(), gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() })]);
    let text_block = div()
        .max_w(z(364.))
        .flex()
        .flex_col()
        .gap(z(6.))
        .child(text(24., 32., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("settings.family.hero_title")))
        .child(
            div()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_MUTED)))
                .child(gpui::InteractiveText::new("family-hero-body", styled).on_click(vec![range], |_, _, cx| cx.open_url(FAMILY_ARTICLE))),
        );
    let start = controls::button("family-get-started", crate::t!("settings.family.get_started"), BtnKind::Primary).on_click(cx.listener(|_, _, _, cx| {
        FAMILY_TAB.with(|t| t.set(1));
        cx.notify();
    }));
    family_box()
        .p(z(32.))
        .flex()
        .items_start()
        .justify_between()
        .child(div().min_w_0().flex().flex_col().child(text_block).child(div().mt(z(16.)).flex().child(start)))
        .child(
            div()
                .self_center()
                .flex_none()
                .pl(z(16.))
                .child(gpui::img(SharedString::from(FAMILY_HERO_ART)).w(z(188.)).h(z(136.)).object_fit(gpui::ObjectFit::Contain)),
        )
}

fn family_feature(icon: &'static str, title: &str, desc: &str) -> Div {
    family_box()
        .flex_1()
        .min_w_0()
        .p(z(20.))
        .flex()
        .flex_col()
        .items_start()
        .child(
            div()
                .size(z(36.))
                .mb(z(10.))
                .rounded_full()
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path(icon)).size(z(20.)).text_color(c(tok(idx::TEXT_STRONG)))),
        )
        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!(title)).mb(z(6.)))
        .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!(desc)))
}

fn family_section(title: &str, desc: Option<&str>, rows: &[(&'static str, &str, &str)]) -> Div {
    let last = rows.len().saturating_sub(1);
    let group = div().rounded(z(12.)).overflow_hidden().bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER))).flex().flex_col().children(rows.iter().enumerate().map(|(ix, (icon, h, d))| {
        let color = if *icon == "close-small" { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_STRONG };
        div()
            .relative()
            .p(z(12.))
            .flex()
            .items_center()
            .child(div().flex_none().ml(z(12.)).mr(z(16.)).child(gpui::svg().path(crate::icons::path(icon)).size(z(20.)).text_color(c(tok(color)))))
            .child(
                div()
                    .min_w_0()
                    .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!(h)).mb(z(4.)))
                    .child(text(12., 16., FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!(d)).max_w(z(500.))),
            )
            .when(ix < last, |r| r.child(div().absolute().bottom_0().right_0().left(z(60.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))))
    }));
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .px(z(4.))
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!(title)))
                .children(desc.map(|d| text(12., 16., FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!(d)))),
        )
        .child(group)
}

fn family_activity(cx: &mut Cx) -> Div {
    let features = div()
        .mt(z(12.))
        .flex()
        .gap(z(10.))
        .child(family_feature("chat-check", "settings.family.f1_t", "settings.family.f1_d"))
        .child(family_feature("chat/eye", "settings.family.f2_t", "settings.family.f2_d"))
        .child(family_feature("qr-code", "settings.family.f3_t", "settings.family.f3_d"));
    let sections = div()
        .flex()
        .flex_col()
        .gap(z(24.))
        .child(family_section(
            "settings.family.g1_t",
            Some("settings.family.g1_d"),
            &[
                ("invite", "settings.family.g1_i1_t", "settings.family.g1_i1_d"),
                ("server/server", "settings.family.g1_i2_t", "settings.family.g1_i2_d"),
                ("forum", "settings.family.g1_i3_t", "settings.family.g1_i3_d"),
                ("phone", "settings.family.g1_i4_t", "settings.family.g1_i4_d"),
                ("credit-card", "settings.family.g1_i5_t", "settings.family.g1_i5_d"),
                ("gift", "settings.family.g1_i6_t", "settings.family.g1_i6_d"),
                ("flag", "settings.family.g1_i7_t", "settings.family.g1_i7_d"),
            ],
        ))
        .child(family_section(
            "settings.family.g2_t",
            Some("settings.family.g2_d"),
            &[
                ("picker/clock", "settings.family.g2_i1_t", "settings.family.g2_i1_d"),
                ("piggy-bank", "settings.family.g2_i2_t", "settings.family.g2_i2_d"),
                ("gear", "settings.family.g2_i3_t", "settings.family.g2_i3_d"),
            ],
        ))
        .child(family_section("settings.family.g3_t", None, &[("close-small", "settings.family.g3_i1_t", "settings.family.g3_i1_d")]));
    div().max_w(z(800.)).flex().flex_col().gap(z(32.)).child(family_hero(cx)).child(features).child(sections)
}

fn family_step(n: usize, title: &str, desc: &str) -> Div {
    div()
        .flex()
        .items_center()
        .when(n < 3, |d| d.mb(z(16.)))
        .child(
            div()
                .flex_none()
                .size(z(32.))
                .mr(z(8.))
                .rounded_full()
                .border_1()
                .border_color(c(tok(idx::BORDER_MUTED)))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .flex()
                .items_center()
                .justify_center()
                .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_BRAND, n.to_string())),
        )
        .child(
            div()
                .min_w_0()
                .child(text(14., 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!(title)))
                .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!(desc))),
        )
}

fn family_members() -> Div {
    let banner = family_box()
        .p(z(32.))
        .overflow_hidden()
        .flex()
        .flex_col()
        .items_start()
        .child(
            div()
                .w_full()
                .flex()
                .items_start()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(text(24., 32., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!("settings.family.link_title")).mb(z(8.)))
                        .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!("settings.family.link_body")).mb(z(20.))),
                )
                .child(gpui::img(SharedString::from(FAMILY_LINK_ART)).flex_none().w(z(224.)).h(z(224. * 192. / 288.)).object_fit(gpui::ObjectFit::Contain)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_start()
                .child(family_step(1, "settings.family.step1_t", "settings.family.step1_d"))
                .child(family_step(2, "settings.family.step2_t", "settings.family.step2_d"))
                .child(family_step(3, "settings.family.step3_t", "settings.family.step3_d")),
        );
    let connected = div()
        .child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_DEFAULT, crate::t!("settings.family.connected", count = 0, max = 8)).mb(z(8.)))
        .child(div().p(z(32.)).flex().flex_col().items_center().justify_center().child(text(14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED, crate::t!("settings.family.none"))));
    div().max_w(z(800.)).flex().flex_col().gap(z(32.)).child(banner).child(connected)
}

fn family_settings(app: &PhoenixApp) -> Div {
    let synced = &app.prefs.synced;
    let row = |id: &'static str, title: &str, desc: &str, on: bool, last: bool| {
        div()
            .id(id)
            .py(z(12.))
            .flex()
            .flex_col()
            .gap(z(8.))
            .cursor(gpui::CursorStyle::OperationNotAllowed)
            .when(!last, |d| d.border_b_1().border_color(c(tok(idx::BORDER_SUBTLE))))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(12.))
                    .child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!(title)).flex_1().min_w_0())
                    .child(controls::switch(SharedString::from(format!("{id}-knob")), on)),
            )
            .child(text(14., 18., FontWeight::NORMAL, idx::TEXT_SUBTLE, crate::t!(desc)))
    };
    div()
        .pt(z(20.))
        .flex()
        .flex_col()
        .child(text(16., 20., FontWeight::SEMIBOLD, idx::TEXT_STRONG, crate::t!("settings.family.general")).mb(z(8.)))
        .child(row("family-tab-switch", "settings.family.enable_tab", "settings.family.enable_tab_d", synced.flag("fc-tab"), false))
        .child(row("family-digest-switch", "settings.family.digest", "settings.family.digest_d", synced.flag("e-family"), true))
}

fn family_center(app: &PhoenixApp, cx: &mut Cx) -> Div {
    let tab = FAMILY_TAB.with(std::cell::Cell::get);
    let body = match tab {
        1 => family_members(),
        2 => family_settings(app),
        _ => family_activity(cx),
    };
    div().pt(z(12.)).flex().flex_col().child(family_tabs(tab, cx)).child(body)
}

#[derive(Clone, Debug, PartialEq)]
pub struct SecurityKey {
    pub id: String,
    pub name: SharedString,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

pub fn parse_keys(v: &serde_json::Value) -> Vec<SecurityKey> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|k| SecurityKey {
            id: k.get("id").and_then(serde_json::Value::as_str).unwrap_or_default().to_owned(),
            name: k.get("name").and_then(serde_json::Value::as_str).unwrap_or_default().to_owned().into(),
            last_used: k.get("last_used").and_then(serde_json::Value::as_str).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&chrono::Utc)),
        })
        .collect()
}

fn security_keys(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let state = KEYS.with_borrow(Clone::clone);
    let Some(list) = state else {
        if !KEYS_LOADING.with(std::cell::Cell::get) {
            KEYS_LOADING.with(|l| l.set(true));
            let fut = app.backend.webauthn_credentials();
            cx.spawn(async move |this, cx| {
                let keys = fut.await.map(|v| parse_keys(&v)).unwrap_or_default();
                let _ = this.update(cx, |_, cx| {
                    KEYS.with_borrow_mut(|k| *k = Some(keys));
                    KEYS_LOADING.with(|l| l.set(false));
                    cx.notify();
                });
            })
            .detach();
        }
        return div().h(z(48.)).into_any_element();
    };
    let last = list.len() == 1;
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .children(list.into_iter().map(|k| {
            let target = crate::ui::extra_menus::Target::SecurityKey { id: k.id.clone(), name: k.name.clone(), last };
            let open = crate::ui::extra_menus::opened().as_ref() == Some(&target);
            let used = k.last_used.map(|d| crate::t!("settings.mfa.key_last_used", last_used = crate::i18n::calendar(d.with_timezone(&chrono::Local), chrono::Local::now())));
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(div().flex().flex_col().child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, k.name.clone())).children(used.map(|u| text(14., 18., FontWeight::NORMAL, idx::TEXT_MUTED, u))))
                .child(
                    div()
                        .id(SharedString::from(format!("mfa-key-{}", k.id)))
                        .size(z(32.))
                        .rounded(z(8.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
                        .when(open, |d| d.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_ACTIVE))))
                        .child(gpui::svg().path(crate::icons::path("more-horizontal")).size(z(20.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                        .on_click(move |ev, w, _| {
                            let p = ev.position();
                            crate::ui::extra_menus::open(target.clone(), gpui::point(p.x, p.y + gpui::px(16.)), w)
                        }),
                )
        }))
        .into_any_element()
}

thread_local! {
    static KEYS: std::cell::RefCell<Option<Vec<SecurityKey>>> = const { std::cell::RefCell::new(None) };
    static KEYS_LOADING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SHOW_PHONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAMILY_TAB: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub fn set_family_tab(ix: usize) {
    FAMILY_TAB.with(|t| t.set(ix.min(2)));
}

pub fn reset() {
    SHOW_PHONE.with(|c| c.set(false));
    FAMILY_TAB.with(|t| t.set(0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_keys_parse_the_credentials_list() {
        let v = serde_json::json!([{ "id": "1", "name": "YubiKey", "type": 1, "last_used": "2026-11-20T10:00:00+00:00" }, { "id": "2", "name": "Pixel", "last_used": null }]);
        let k = parse_keys(&v);
        assert_eq!(k.len(), 2);
        assert_eq!(k[0].name.as_ref(), "YubiKey");
        assert!(k[0].last_used.is_some() && k[1].last_used.is_none());
        let w = crate::actions::guard::GuardedWrite::start_webauthn_register();
        assert_eq!((w.method.as_str(), w.path.as_str()), ("POST", "/users/@me/mfa/webauthn/credentials"));
        assert!(!crate::actions::guard::read_only_write(&w));
    }

    #[test]
    fn standing_states_match_the_client_enum() {
        assert_eq!(Standing::from_state(100), Standing::AllGood);
        assert_eq!(Standing::from_state(200), Standing::Limited);
        assert_eq!(Standing::from_state(300), Standing::VeryLimited);
        assert_eq!(Standing::from_state(400), Standing::AtRisk);
        assert_eq!(Standing::from_state(500), Standing::Suspended);
        assert_eq!(Standing::from_state(7), Standing::AllGood);
    }

    #[test]
    fn nested_panels_light_their_sub_nav_row() {
        assert_eq!(Nested::Devices.sub_index(), 1);
        assert_eq!(Nested::Mfa.sub_index(), 1);
        assert_eq!(Nested::Standing.sub_index(), 2);
        assert_eq!(Nested::FamilyCenter.sub_index(), 3);
    }

    #[test]
    fn hook_is_highlighted() {
        let (p, hl) = bracketed("Ton compte est [tout bon]", gpui::HighlightStyle::default());
        assert_eq!(p, "Ton compte est tout bon");
        assert_eq!(hl[0].0, 15..23);
    }
}

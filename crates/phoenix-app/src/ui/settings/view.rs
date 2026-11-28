use crate::theme::z;
use gpui::{
    AnyElement, Context, Div, ElementId, FontWeight, IntoElement, ParentElement, SharedString, Stateful, Styled,
    Window, div, prelude::*,
};

use super::{AppTheme, NAV, Page, SettingsState};
use crate::{
    app::PhoenixApp,
    theme::{Palette, c, generated::idx, tok},
    ui::{
        avatar,
        controls::{self, BtnKind},
    },
};

type Cx<'a> = Context<'a, PhoenixApp>;

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

fn st(app: &mut PhoenixApp) -> Option<&mut SettingsState> {
    app.settings.as_mut()
}

fn h1(t: impl Into<SharedString>) -> Div {
    controls::section_title(t)
}



fn note(t: impl Into<SharedString>) -> Div {
    div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(t.into())
}

fn divider() -> Div {
    div().my(z(50.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)))
}

fn category(title: impl Into<SharedString>, rows: Vec<AnyElement>) -> Div {
    div().flex().flex_col().child(h1(title)).child(div().flex().flex_col().gap(z(10.)).children(rows))
}

fn secondary(id: impl Into<String>, label: impl Into<SharedString>) -> Stateful<Div> {
    controls::button(eid(id), label, BtnKind::Secondary)
}



pub(super) fn slider(
    id: &str,
    values: &[f32],
    unit: &'static str,
    current: f32,
    default: f32,
    set: impl Fn(&mut PhoenixApp, f32, &mut Cx) + 'static,
    cx: &mut Cx,
) -> Div {
    let labels = values.iter().map(|v| SharedString::from(format!("{v}{unit}"))).collect();
    let vals: Vec<f32> = values.to_vec();
    let ent = cx.entity();
    controls::marker_slider(
        SharedString::from(format!("slider-{id}")),
        labels,
        controls::nearest(values, current),
        Some(controls::nearest(values, default)),
        std::rc::Rc::new(move |i, _, cx| {
            let v = vals[i];
            ent.update(cx, |a, cx| set(a, v, cx));
        }),
    )
}

fn field_row(label: impl Into<SharedString>, value: SharedString, action: Option<AnyElement>) -> AnyElement {
    let trailing = div()
        .flex()
        .items_center()
        .gap(z(20.))
        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(value))
        .children(action);
    controls::control_item(label, None, Some(trailing.into_any_element())).into_any_element()
}

fn masked_value(shown: bool, v: String, masked: String, id: &'static str, flip: fn(&mut PhoenixApp), cx: &mut Cx) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(z(4.))
        .child(super::account::value(if shown { v } else { masked }))
        .child(super::account::reveal_button(id, shown, flip, cx))
        .into_any_element()
}

fn my_account(app: &PhoenixApp, cx: &mut Cx) -> Div {
    use super::account as acct;
    let me = &app.me;
    let info = acct::info();
    let (reveal_em, reveal_ph) = if app.prefs.local.streamer_hides_info() {
        (false, false)
    } else {
        app.settings.as_ref().map_or((false, false), |s| (s.reveal_email, s.reveal_phone))
    };
    let trailing = |val: AnyElement, btn: Stateful<Div>| div().flex().items_center().gap(z(20.)).child(val).child(btn).into_any_element();
    use crate::ui::modals::account_edit::{EmailStep, Flow};
    let email_flow = if info.email.is_some() && info.verified { Flow::Email(EmailStep::Start) } else { Flow::Email(EmailStep::Form) };
    let phone_flow = if info.phone.is_some() { Flow::Phone } else { Flow::PhoneEntry };
    let edit = |id: &'static str, label: SharedString, flow: Flow| {
        secondary(id, label).on_click(cx.listener(move |a, _, w, cx| a.open_account_edit(flow, w, cx)))
    };
    let edit_em = edit("edit-em", if info.email.is_some() { crate::t!("common.edit") } else { crate::t!("common.add") }, email_flow);
    let edit_ph = edit("edit-ph", if info.phone.is_some() { crate::t!("common.edit") } else { crate::t!("common.add") }, phone_flow);
    let edit_un = edit("edit-un", crate::t!("common.edit"), Flow::Username);
    let change_pw = edit("change-pw", crate::t!("common.edit"), Flow::Password);
    let email_row = match info.email.clone() {
        Some(e) => {
            let m = acct::mask_email(&e);
            let v = masked_value(reveal_em, e, m, "reveal-email", |a| {
                if let Some(s) = st(a) {
                    s.reveal_email = !s.reveal_email;
                }
            }, cx);
            controls::control_item(crate::t!("settings.account.email"), None, Some(trailing(v, edit_em)))
        }
        None => controls::control_item(crate::t!("settings.account.email"), None, Some(trailing(acct::value(crate::t!("settings.account.no_email")).into_any_element(), edit_em))),
    }
    .into_any_element();
    let phone_row = match info.phone.clone() {
        Some(p) => {
            let m = acct::mask_phone(&p);
            let v = masked_value(reveal_ph, p, m, "reveal-phone", |a| {
                if let Some(s) = st(a) {
                    s.reveal_phone = !s.reveal_phone;
                }
            }, cx);
            controls::control_item(crate::t!("settings.account.phone"), None, Some(trailing(v, edit_ph))).into_any_element()
        }
        None => field_row(crate::t!("settings.account.phone"), crate::t!("settings.account.no_phone"), Some(edit_ph.into_any_element())),
    };
    let link_row = |id: &'static str, title: SharedString, v: Option<SharedString>, to: super::nested::Nested| {
        controls::control_item(title, None, Some(acct::value_chevron(v)))
            .id(id)
            .cursor_pointer()
            .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
            .on_click(cx.listener(move |a, _, _, cx| super::nested::open(a, to, cx)))
            .into_any_element()
    };
    let devices = acct::sessions().map(|n| crate::t!("settings.account.devices_n", n = n));
    let q = query(app, cx);
    let t = |k: &str| crate::t!(k);
    let info_rows = vec![
        (t("settings.account.username"), field_row(crate::t!("settings.account.username"), me.name.clone(), Some(edit_un.into_any_element()))),
        (t("settings.account.email"), email_row),
        (t("settings.account.phone"), phone_row),
    ];
    let security_rows = vec![
        (
            t("settings.account.password"),
            controls::control_item(crate::t!("settings.account.password"), None, Some(change_pw.into_any_element())).into_any_element(),
        ),
        (
            t("settings.account.mfa"),
            link_row(
                "acct-mfa",
                crate::t!("settings.account.mfa"),
                Some(if info.mfa { crate::t!("settings.account.mfa_on") } else { crate::t!("settings.account.mfa_off") }),
                super::nested::Nested::Mfa,
            ),
        ),
        (t("settings.account.devices"), link_row("acct-devices", crate::t!("settings.account.devices"), devices, super::nested::Nested::Devices)),
    ];
    let mut standing_labels = Vec::new();
    if info.adult.is_some() {
        standing_labels.push(t("settings.account.age_group"));
    }
    standing_labels.push(t("settings.account.status"));
    let standing_rows = standing_labels.into_iter().zip(acct::standing_rows(&info, cx)).collect();
    let family_rows = vec![(
        t("settings.account.family_setup"),
        controls::control_item(crate::t!("settings.account.family_setup"), Some(crate::t!("settings.account.family_d")), Some(acct::value_chevron(None)))
            .id("acct-family")
            .cursor_pointer()
            .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
            .on_click(cx.listener(|a, _, _, cx| super::nested::open(a, super::nested::Nested::FamilyCenter, cx)))
            .into_any_element(),
    )];
    let removal: Vec<(SharedString, AnyElement)> = [t("settings.account.disable"), t("settings.account.close")].into_iter().zip(acct::removal_rows(cx)).collect();
    let sections: Vec<Div> = [
        filtered(&q, Some(crate::t!("settings.account.info")), info_rows),
        filtered(&q, Some(crate::t!("settings.account.security")), security_rows),
        filtered(&q, Some(crate::t!("settings.account.status")), standing_rows),
        filtered(&q, Some(crate::t!("settings.account.family")), family_rows),
        filtered(&q, None, removal),
    ]
    .into_iter()
    .flatten()
    .collect();
    let last = sections.len().saturating_sub(1);
    div().children(sections.into_iter().enumerate().map(|(i, d)| if i < last { d.child(divider()) } else { d }))
}

pub(super) fn query(app: &PhoenixApp, cx: &Cx) -> String {
    app.settings.as_ref().and_then(|s| s.search.as_ref()).map(|i| i.read(cx).value().trim().to_lowercase()).unwrap_or_default()
}

fn filtered(q: &str, title: Option<SharedString>, rows: Vec<(SharedString, AnyElement)>) -> Option<Div> {
    let whole = q.is_empty() || title.as_ref().is_some_and(|t| t.to_lowercase().contains(q));
    let kept: Vec<AnyElement> = rows.into_iter().filter(|(t, _)| whole || t.to_lowercase().contains(q)).map(|(_, r)| r).collect();
    if kept.is_empty() {
        return None;
    }
    Some(match title {
        Some(t) => category(t, kept),
        None => div().flex().flex_col().gap(z(10.)).children(kept),
    })
}

pub(super) fn profiles(app: &PhoenixApp) -> Div {
    let me = &app.me;
    let label = |k: &str| {
        div().pb(z(8.)).text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!(k))
    };
    let tile = |w: f32, h: f32| {
        div()
            .w(z(w))
            .h(z(h))
            .flex_none()
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .flex()
            .items_center()
            .justify_center()
    };
    let plus = || gpui::svg().path(crate::icons::path("plus-circle")).size(z(24.)).text_color(c(tok(idx::ICON_DEFAULT)));
    let banner = Palette::BLURPLE;
    let section = |k: &str, body: Div| div().pb(z(24.)).flex().flex_col().child(label(k)).child(body);
    let styles = div()
        .w(z(208.))
        .flex_none()
        .flex()
        .flex_col()
        .child(section("sp.pf.nameplate", tile(208., 48.).justify_between().px(z(8.)).child(avatar(Some(me), 32.)).child(plus())))
        .child(section(
            "sp.pf.avatar",
            div().flex().gap(z(8.)).child(tile(100., 100.).child(avatar(Some(me), 72.))).child(tile(100., 100.).child(plus())),
        ))
        .child(section(
            "sp.pf.name_style",
            tile(208., 42.).child(div().text_size(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(me.display_name.clone())),
        ))
        .child(section("sp.pf.theme", div().flex().gap(z(8.)).child(tile(100., 100.).bg(c(banner))).child(tile(100., 100.).child(plus()))))
        .child(section("sp.pf.effect", div().flex().gap(z(8.)).child(tile(100., 100.).child(plus())).child(tile(100., 100.).child(plus()))));
    let card = div()
        .flex_1()
        .min_w_0()
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .overflow_hidden()
        .child(div().h(z(120.)).bg(c(banner)))
        .child(
            div()
                .relative()
                .px(z(16.))
                .pb(z(16.))
                .child(div().mt(z(-48.)).size(z(96.)).rounded_full().border_4().border_color(c(tok(idx::BACKGROUND_SURFACE_HIGH))).child(avatar(Some(me), 88.)))
                .child(div().pt(z(12.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::BOLD).text_color(c(tok(idx::TEXT_STRONG))).child(me.display_name.clone()))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(me.name.clone())),
        );
    div()
        .flex()
        .flex_col()
        .child(div().pb(z(24.)).text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!("sp.pf.main")))
        .child(div().flex().gap(z(32.)).items_start().child(styles).child(card))
}

fn devices() -> Div {
    category(
        crate::t!("settings.page.devices"),
        vec![
            note(crate::t!("settings.devices.note")).into_any_element(),
            controls::control_item("Linux · Discord Phoenix", Some(crate::t!("settings.devices.this")), None).into_any_element(),
        ],
    )
}


pub(super) fn theme_card(t: AppTheme, selected: bool) -> Stateful<Div> {
    div()
        .id(eid(format!("theme-{}", t.label())))
        .relative()
        .size(z(48.))
        .flex_none()
        .cursor_pointer()
        .child(
            div()
                .size_full()
                .rounded(z(8.))
                .bg(c(t.swatch()))
                .border_1()
                .border_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))),
        )
        .when(selected, |d| {
            d.child(
                div()
                    .absolute()
                    .top(z(-2.))
                    .left(z(-2.))
                    .size(z(52.))
                    .rounded(z(8.))
                    .border(z(4.))
                    .border_color(c(tok(idx::BRAND_500))),
            )
            .child(
                div()
                    .absolute()
                    .top(z(-6.))
                    .right(z(-6.))
                    .size(z(20.))
                    .rounded_full()
                    .bg(c(tok(idx::BRAND_500)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(gpui::svg().path(crate::icons::path("check")).size(z(14.)).text_color(gpui::white())),
            )
        })
        .hover(|s| s.opacity(0.9))
}




fn page(app: &PhoenixApp, cx: &mut Cx) -> Div {
    if let Some(n) = app.settings.as_ref().and_then(|s| s.nested) {
        return super::nested::render(app, n, cx);
    }
    match app.settings.as_ref().map(|s| s.page).unwrap_or_default() {
        Page::MyAccount => my_account(app, cx),
        Page::Devices => devices(),
        Page::VoiceVideo => super::voice::render(app, cx),
        p => super::pages::render(app, super::pages::blocks(p), cx),
    }
}

fn page_icon(p: Page) -> &'static str {
    match p {
        Page::MyAccount | Page::Profiles => "user",
        Page::DataPrivacy => "shield-lock",
        Page::PrivacySafety => "chat-shield",
        Page::Devices => "image",
        Page::Connections => "link",
        Page::Notifications => "bell",
        Page::Keybinds | Page::System => "keyboard",
        Page::Language => "language",
        Page::Appearance => "palette",
        Page::Accessibility => "accessibility",
        Page::VoiceVideo => "mic",
        Page::Advanced => "angle-brackets",
        Page::ActivityPrivacy => "user-platform",
        Page::RegisteredGames => "game-controller",
        Page::Nitro => "nitro",
        Page::ServerBoost => "chat/boost",
        Page::Subscriptions => "subscription",
        Page::GiftInventory => "gift",
        Page::Billing => "credit-card",
    }
}

fn nav_item(id: impl Into<String>, icon: &str, label: impl Into<SharedString>, selected: bool, destructive: bool) -> Stateful<Div> {
    let fg = if selected { idx::TEXT_STRONG } else { idx::TEXT_MUTED };
    let label_fg = if destructive { idx::TEXT_FEEDBACK_CRITICAL } else { fg };
    let id: String = id.into();
    let group: SharedString = format!("nav-{id}").into();
    let fade_key = crate::ui::states::key(("settings-nav", id.as_str()));
    let label: SharedString = label.into();
    let spoken = label.clone();
    crate::ui::a11y::named(div().id(eid(id)), gpui::Role::Link, move || spoken)
        .when(crate::ui::a11y::on(), |d| d.aria_selected(selected))
        .group(group.clone())
        .relative()
        .when(selected, |d| {
            d.child(crate::ui::states::fade_to(
                fade_key,
                true,
                c(tok(idx::BACKGROUND_MOD_MUTED)),
                c(tok(idx::BACKGROUND_MOD_NORMAL)),
                300,
                8.,
            ))
        })
        .when(!selected, |d| {
            crate::ui::states::forget(fade_key);
            d
        })
        .p(z(10.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(10.))
        .cursor_pointer()
        .text_color(c(tok(fg)))
        .when(!selected && !destructive, |d| {
            d.hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_MUTED))).text_color(c(tok(idx::TEXT_STRONG))))
        })
        .when(destructive, |d| d.hover(|s| s.bg(c(tok(idx::BACKGROUND_FEEDBACK_MOD_CRITICAL)))))
        .child(
            gpui::svg()
                .path(crate::icons::path(icon))
                .size(z(20.))
                .flex_none()
                .text_color(c(tok(fg)))
                .when(!destructive, |d| d.group_hover(group, |s| s.text_color(c(tok(idx::TEXT_STRONG))))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .when(destructive, |d| d.text_color(c(tok(label_fg))))
                .child(label),
        )
}

fn subnav(rows: Vec<SharedString>, active: usize, cx: &mut Cx) -> Div {
    let total = rows.len();
    let n = total as f32;
    let active = active.min(total.saturating_sub(1));
    div()
        .pt(z(6.))
        .pb(z(10.))
        .ml(z(19.))
        .flex()
        .gap(z(19.))
        .child(
            div()
                .flex_none()
                .w(z(2.))
                .h(z(20. * n + 20. * (n - 1.)))
                .rounded(z(1.))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .child(div().mt(z(40. * active as f32)).w_full().h(z(20.)).rounded_full().bg(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))),
        )
        .child(div().flex_1().min_w_0().flex().flex_col().gap(z(20.)).children(rows.into_iter().enumerate().map(|(i, t)| {
            div()
                .id(("settings-subnav", i))
                .cursor_pointer()
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(if i == active { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::TEXT_MUTED })))
                .when(i != active, |d| d.hover(|s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))))
                .on_click(cx.listener(move |a, _, _, cx| {
                    if let Some(s) = st(a) {
                        s.active_section = i;
                        s.nested = None;
                        let y = if i == 0 || total <= 1 {
                            gpui::px(0.)
                        } else {
                            let frac = i as f32 / (total - 1) as f32;
                            -(s.scroll.max_offset().y * frac)
                        };
                        s.scroll.set_offset(gpui::point(gpui::px(0.), y));
                    }
                    cx.notify();
                }))
                .child(t)
        })))
}

fn scrolled_section(s: &super::SettingsState, total: usize) -> usize {
    let max = s.scroll.max_offset().y;
    if total <= 1 || max <= gpui::px(1.) {
        return s.active_section;
    }
    let frac = ((-s.scroll.offset().y) / max).clamp(0., 1.);
    (frac * (total - 1) as f32).round() as usize
}

static DEBUG_COPIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn copy_debug_info(app: &mut PhoenixApp, window: &mut Window, cx: &mut Cx) {
    let gpu = window.gpu_specs().map(crate::diagnostics::Gpu::from);
    let text = crate::diagnostics::Report::collect(gpu, &app.prefs.local).render();
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
    DEBUG_COPIED.store(true, std::sync::atomic::Ordering::Relaxed);
    cx.notify();
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
        DEBUG_COPIED.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

fn open_log_dir(cx: &mut gpui::App) {
    let Some(dir) = crate::logging::log_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    cx.open_with_system(&dir);
}

fn footer(cx: &mut Cx) -> Div {
    static INFO: std::sync::OnceLock<phoenix_api::build_info::BuildInfo> = std::sync::OnceLock::new();
    let b = INFO.get_or_init(phoenix_api::build_info::BuildInfo::detect_fast);
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    let line = format!("{} {} • {} {arch}", b.release_channel, b.build_number, b.client_version);
    let note = |t: SharedString, col: usize| div().text_size(z(10.)).line_height(z(12.)).text_color(c(tok(col))).child(t);
    let bullet = || note("•".into(), idx::TEXT_MUTED);
    let link = |id: &'static str, d: Div| d.id(id).cursor_pointer().hover(|s| s.underline());
    div()
        .px(z(10.))
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(note(line.into(), idx::TEXT_MUTED))
        .child(note(crate::diagnostics::version_line().into(), idx::TEXT_MUTED))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(z(4.))
                .child(
                    link(
                        "settings-copy-debug",
                        note(
                            if DEBUG_COPIED.load(std::sync::atomic::Ordering::Relaxed) { crate::t!("settings.footer.debug_copied") } else { crate::t!("settings.footer.copy_debug") },
                            idx::TEXT_LINK,
                        ),
                    )
                    .on_click(cx.listener(|a, _, window, cx| copy_debug_info(a, window, cx))),
                )
                .child(bullet())
                .child(link("settings-open-logs", note(crate::t!("settings.footer.open_logs"), idx::TEXT_LINK)).on_click(|_, _, cx| open_log_dir(cx))),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(z(4.))
                .child(link("settings-privacy", note(crate::t!("settings.footer.privacy"), idx::TEXT_LINK)).on_click(|_, _, cx| cx.open_url(crate::ui::extra_menus::PRIVACY_URL)))
                .child(bullet())
                .child(link("settings-terms", note(crate::t!("settings.footer.terms"), idx::TEXT_LINK)).on_click(|_, _, cx| cx.open_url(crate::ui::extra_menus::TERMS_URL)))
                .child(bullet())
                .child(
                    note(crate::t!("settings.footer.more"), idx::TEXT_MUTED)
                        .id("settings-more")
                        .cursor_pointer()
                        .when(crate::ui::extra_menus::opened() == Some(crate::ui::extra_menus::Target::SettingsMore), |d| d.text_color(c(tok(idx::TEXT_DEFAULT))))
                        .hover(|s| s.underline().text_color(c(tok(idx::TEXT_DEFAULT))))
                        .on_click(|ev, w, _| {
                            let b = ev.position();
                            crate::ui::extra_menus::open(crate::ui::extra_menus::Target::SettingsMore, gpui::point(b.x, b.y - gpui::px(8.)), w)
                        }),
                ),
        )
}

fn subnav_rows(p: Page) -> Vec<SharedString> {
    match p {
        Page::MyAccount => vec![
            crate::t!("settings.account.info"),
            crate::t!("settings.account.security"),
            crate::t!("settings.account.status"),
            crate::t!("settings.account.family"),
        ],
        p => match subnav_keys(p) {
            [] => super::pages::titles(p),
            keys => keys.iter().map(|k| crate::t!(k)).collect(),
        },
    }
}

fn subnav_keys(p: Page) -> &'static [&'static str] {
    match p {
        Page::DataPrivacy => &["settings.sub.data_use", "settings.sub.profile_privacy", "settings.sub.e2ee"],
        Page::PrivacySafety => &["settings.sub.content_filters", "settings.sub.spam", "settings.sub.dms", "settings.sub.friend_requests", "settings.sub.connected_games", "settings.sub.ignore_block"],
        Page::Notifications => &["settings.sub.overview", "settings.sub.sounds", "settings.sub.badges", "settings.sub.email", "settings.sub.advanced_n"],
        Page::Billing => &["settings.sub.payment", "settings.sub.transactions"],
        Page::VoiceVideo => &["settings.sub.voice", "settings.sub.camera", "settings.sub.streaming", "settings.sub.sounds", "settings.sub.soundboard", "settings.sub.advanced"],
        Page::Appearance => &["settings.sub.theme", "settings.sub.app_icon", "settings.sub.messages", "settings.sub.chat_box", "settings.sub.search", "settings.sub.streamer"],
        Page::Accessibility => &["settings.sub.readability", "settings.sub.density", "settings.sub.contrast", "settings.sub.motion", "settings.sub.audio_sr"],
        Page::System => &["settings.sub.general", "settings.sub.custom_keys", "settings.sub.default_keys", "settings.sub.advanced_n"],
        Page::RegisteredGames => &["settings.sub.current_game", "settings.sub.added_games"],
        Page::ActivityPrivacy => &["settings.sub.what_share", "settings.sub.where_share", "settings.sub.joining"],
        Page::Connections => &["settings.sub.connections", "settings.sub.authorized"],
        _ => &[],
    }
}

fn list_is_empty(q: &str) -> bool {
    !NAV.iter().any(|(_, pages)| pages.iter().any(|p| super::pages::matches(*p, q)))
}

thread_local! {
    static NAV_PAGE: std::cell::Cell<Option<Page>> = const { std::cell::Cell::new(None) };
    static NAV_REVEAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn reset_nav() {
    NAV_PAGE.with(|p| p.set(None));
    crate::ui::chat::scrollbar::handle("settings-nav").set_offset(gpui::Point::default());
}

fn reveal_probe() -> impl IntoElement {
    gpui::canvas(
        |b, w, _| {
            if !NAV_REVEAL.with(std::cell::Cell::get) {
                return;
            }
            let h = crate::ui::chat::scrollbar::handle("settings-nav");
            let vp = h.bounds();
            if vp.size.height <= gpui::px(0.) {
                w.refresh();
                return;
            }
            NAV_REVEAL.with(|r| r.set(false));
            let pad = z(12.);
            let mut off = h.offset();
            if b.top() < vp.top() + pad {
                off.y += vp.top() + pad - b.top();
            } else if b.bottom() > vp.bottom() - pad {
                off.y -= b.bottom() - (vp.bottom() - pad);
            } else {
                return;
            }
            off.y = off.y.min(gpui::px(0.)).max(-h.max_offset().y);
            h.set_offset(off);
            w.refresh();
        },
        |_, _, _, _| (),
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub(super) fn select_page(p: Page, cx: &mut Cx) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
    cx.listener(move |a, _, _, cx| {
        if let Some(s) = st(a) {
            s.page = p;
            s.error = None;
            s.open_select = None;
            s.nested = None;
            s.active_section = 0;
            s.scroll.set_offset(gpui::Point::default());
        }
        cx.notify();
    })
}

fn nav(app: &PhoenixApp, cx: &mut Cx) -> Div {
    let cur = app.settings.as_ref().map(|s| s.page).unwrap_or_default();
    if NAV_PAGE.with(|p| p.replace(Some(cur))) != Some(cur) {
        NAV_REVEAL.with(|r| r.set(true));
    }
    let profile = div()
        .id("nav-profiles")
        .p(z(10.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(12.))
        .cursor_pointer()
        .when(cur == Page::Profiles, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .when(cur != Page::Profiles, |d| d.hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_MUTED)))))
        .on_click(select_page(Page::Profiles, cx))
        .child(avatar(Some(&app.me), 48.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(z(2.))
                .child(
                    div()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .child(app.me.display_name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(c(tok(idx::TEXT_MUTED)))
                        .child(crate::t!("settings.edit_profiles"))
                        .child(gpui::svg().path(crate::icons::path("pen")).size(z(12.)).flex_none().text_color(c(tok(idx::TEXT_MUTED)))),
                ),
        );
    let query = query(app, cx);
    let field: AnyElement = match app.settings.as_ref().and_then(|s| s.search.clone()) {
        Some(input) => div()
            .flex_1()
            .min_w_0()
            .text_size(z(16.))
            .text_color(c(tok(idx::TEXT_DEFAULT)))
            .child(gpui_component::input::Input::new(&input).aria_label(crate::t!("settings.search")).appearance(false).bordered(false).px_0().text_size(z(16.)))
            .into_any_element(),
        None => div()
            .flex_1()
            .text_size(z(16.))
            .text_color(c(tok(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT)))
            .child(crate::t!("settings.search"))
            .into_any_element(),
    };
    let search = div()
        .h(z(40.))
        .pl(z(4.))
        .pr(z(4.))
        .flex()
        .items_center()
        .gap(z(4.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(if query.is_empty() { idx::INPUT_BORDER_DEFAULT } else { idx::INPUT_BORDER_ACTIVE })))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .child(
            div().size(z(30.)).flex().items_center().justify_center().child(
                gpui::svg().path(crate::icons::path("search")).size(z(16.)).text_color(c(tok(idx::INPUT_ICON_DEFAULT))),
            ),
        )
        .child(field)
        .when(!query.is_empty(), |d| {
            d.child(
                div()
                    .id("settings-search-clear")
                    .size(z(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|a, _, window, cx| {
                        if let Some(i) = a.settings.as_ref().and_then(|s| s.search.clone()) {
                            i.update(cx, |i, cx| i.set_value("", window, cx));
                        }
                        cx.notify();
                    }))
                    .child(gpui::svg().path(crate::icons::path("settings/circle-x")).size(z(16.)).text_color(c(tok(idx::TEXT_SUBTLE)))),
            )
        });
    let fixed = div()
        .pl(z(4.))
        .pr(z(16.))
        .flex_none()
        .child(div().pb(z(10.)).child(profile))
        .child(search);

    let searching = !query.is_empty();
    let mut list = div().flex().flex_col().gap(z(10.));
    let sections: Vec<(&str, Vec<Page>)> = NAV
        .iter()
        .map(|(t, pages)| (*t, pages.iter().copied().filter(|p| super::pages::matches(*p, &query)).collect::<Vec<_>>()))
        .filter(|(_, v)| !v.is_empty() || !searching)
        .collect();
    let last = sections.len().saturating_sub(1);
    for (ix, (title, pages)) in sections.into_iter().enumerate() {
        let mut sec = div().pb(z(10.)).flex().flex_col().gap(z(4.));
        if ix != last {
            sec = sec.border_b_1().border_color(c(tok(idx::BORDER_SUBTLE)));
        }
        if ix > 0 && !title.is_empty() {
            sec = sec.child(
                div()
                    .mt(z(8.))
                    .mx(z(8.))
                    .mb(z(4.))
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_MUTED)))
                    .child(crate::t!(title)),
            );
        }
        for p in pages {
            let item = nav_item(format!("nav-{}", p.label()), page_icon(p), p.label(), p == cur, false).on_click(select_page(p, cx));
            sec = sec.child(if p == cur { item.child(reveal_probe()) } else { item });
            if p == cur && !searching {
                let rows = subnav_rows(p);
                if rows.len() > 1 {
                    let active = app.settings.as_ref().map_or(0, |s| s.nested.map_or_else(|| scrolled_section(s, rows.len()), super::nested::Nested::sub_index));
                    sec = sec.child(subnav(rows, active, cx));
                }
            }
        }
        if ix == last && !searching {
            sec = sec.child(nav_item("nav-logout", "door-exit", crate::t!("settings.logout"), false, true).on_click(cx.listener(|a, _, _, cx| {
                if let Some(s) = st(a) {
                    s.confirm_logout = true;
                }
                cx.notify();
            })));
            sec = sec.child(footer(cx));
        }
        list = list.child(sec);
    }
    if searching && last == 0 && list_is_empty(&query) {
        list = list.child(div().p(z(10.)).text_size(z(16.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("settings.no_results")));
    }
    div()
        .w(z(253.))
        .flex_none()
        .h_full()
        .pt(z(16.))
        .pl(z(12.))
        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
        .border_r_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .flex()
        .flex_col()
        .child(fixed)
        .child(
            crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::ThinFade)
                .flex_1()
                .min_h_0()
                .mr(z(4.))
                .child(
                    crate::ui::a11y::named(div().id("settings-nav"), gpui::Role::Navigation, || crate::t!("a11y.settings_pages"))
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&crate::ui::chat::scrollbar::handle("settings-nav"))
                        .pt(z(12.))
                        .pb(z(16.))
                        .pl(z(4.))
                        .pr(z(12.))
                        .child(list),
                )
                .children(crate::ui::chat::scrollbar::bar_for("settings-nav", crate::ui::chat::scrollbar::Bar::ThinFade)),
        )
}

fn content_header(label: SharedString, nested: Option<super::nested::Nested>, cx: &mut Cx) -> Div {
    let crumb = |t: SharedString, current: bool| {
        div()
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(if current { FontWeight::MEDIUM } else { FontWeight::NORMAL })
            .text_color(c(tok(if current { idx::TEXT_DEFAULT } else { idx::TEXT_SUBTLE })))
            .whitespace_nowrap()
            .child(t)
    };
    let left = match nested {
        None => crumb(label, true),
        Some(n) => div()
            .flex()
            .items_center()
            .gap(z(8.))
            .child(
                crate::ui::a11y::named(div().id("settings-back"), gpui::Role::Button, || crate::t!("a11y.back"))
                    .size(z(32.))
                    .rounded(z(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                    .on_click(cx.listener(|a, _, _, cx| super::nested::close(a, cx)))
                    .child(gpui::svg().path(crate::icons::path("settings/arrow-left")).size(z(16.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .child(
                        div()
                            .id("settings-crumb-parent")
                            .flex()
                            .items_center()
                            .gap(z(10.))
                            .cursor_pointer()
                            .on_click(cx.listener(|a, _, _, cx| super::nested::close(a, cx)))
                            .child(crumb(label, false))
                            .child(crumb("/".into(), false)),
                    )
                    .child(crumb(n.title(), true)),
            ),
    };
    div()
        .h(z(48.))
        .flex_none()
        .pl(z(if nested.is_some() { 8. } else { 16. }))
        .pr(z(8.))
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .child(left)
        .child(
            crate::ui::a11y::named(div().id("settings-close"), gpui::Role::Button, || crate::t!("a11y.close"))
                .size(z(32.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                .on_click(cx.listener(|a, _, _, cx| {
                    a.settings = None;
                    cx.notify();
                }))
                .child(gpui::svg().path(crate::icons::path("close")).size(z(16.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        )
}

fn hwaccel_modal(on: bool, cx: &mut Cx) -> AnyElement {
    let (e1, e2) = (cx.entity(), cx.entity());
    let lines: Vec<SharedString> = if on {
        vec![crate::t!("sp.sy.hwaccel_on_body")]
    } else {
        vec![crate::t!("sp.sy.hwaccel_off_body"), crate::t!("sp.sy.hwaccel_sure")]
    };
    crate::ui::modal::confirm(crate::ui::modal::Confirm {
        title: crate::t!("sp.sy.hwaccel_title"),
        body: vec![div().flex().flex_col().children(lines).into_any_element()],
        confirm_label: crate::t!("sp.sy.hwaccel_restart"),
        danger: false,
        on_confirm: std::rc::Rc::new(move |_, cx| {
            e1.update(cx, |a, cx| {
                if let Some(s) = a.settings.as_mut() {
                    s.confirm_hwaccel = None;
                }
                a.prefs.local.ui.insert(crate::hwaccel::PREF.to_owned(), on as u64);
                a.prefs.local.save_blocking();
                cx.notify();
            });
            crate::hwaccel::relaunch(cx);
        }),
        on_cancel: std::rc::Rc::new(move |_, cx| {
            e2.update(cx, |a, cx| {
                if let Some(s) = a.settings.as_mut() {
                    s.confirm_hwaccel = None;
                }
                cx.notify();
            })
        }),
    })
}

fn logout_modal(cx: &mut Cx) -> AnyElement {
    let (e1, e2) = (cx.entity(), cx.entity());
    crate::ui::modal::confirm(crate::ui::modal::Confirm {
        title: crate::t!("settings.logout"),
        body: vec![crate::t!("settings.logout_confirm").into_any_element()],
        confirm_label: crate::t!("settings.logout"),
        danger: true,
        on_confirm: std::rc::Rc::new(move |_, cx| {
            e1.update(cx, |a, cx| {
                a.settings = None;
                a.logout(cx);
            })
        }),
        on_cancel: std::rc::Rc::new(move |_, cx| {
            e2.update(cx, |a, cx| {
                if let Some(s) = st(a) {
                    s.confirm_logout = false;
                }
                cx.notify();
            })
        }),
    })
}

fn changelog_inline(inline: &crate::markdown::Inline) -> AnyElement {
    use crate::markdown::Style;
    let mut hl = Vec::new();
    for span in &inline.spans {
        if span.range.end > inline.text.len() || span.range.start >= span.range.end {
            continue;
        }
        let mut h = gpui::HighlightStyle::default();
        if span.style.has(Style::BOLD) {
            h.font_weight = Some(FontWeight::BOLD);
        }
        if span.style.has(Style::ITALIC) {
            h.font_style = Some(gpui::FontStyle::Italic);
        }
        if span.style.has(Style::CODE) {
            h.background_color = Some(c(tok(idx::BACKGROUND_BASE_LOWER)));
        }
        if span.style.has(Style::LINK) || span.link.is_some() {
            h.color = Some(c(tok(idx::TEXT_LINK)));
        }
        hl.push((span.range.clone(), h));
    }
    hl.sort_by_key(|(r, _)| r.start);
    let mut clean: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> = Vec::new();
    for (r, h) in hl {
        if clean.last().is_some_and(|(p, _)| r.start < p.end) {
            continue;
        }
        clean.push((r, h));
    }
    div().w_full().min_w_0().child(gpui::StyledText::new(inline.text.clone()).with_highlights(clean)).into_any_element()
}

fn changelog_body(entries: &[crate::updater::ChangelogEntry]) -> Vec<AnyElement> {
    use crate::markdown::{Block, NoResolve, parse};
    let body_text = |d: Div| d.text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT)));
    let mut out = Vec::new();
    for (ix, e) in entries.iter().enumerate() {
        let date = chrono::DateTime::parse_from_rfc3339(&e.published_at)
            .map(|d| crate::i18n::intl::format(crate::i18n::locale(), "LL", &crate::i18n::intl::Parts::of(&d.with_timezone(&chrono::Local)), None))
            .unwrap_or_default();
        out.push(
            div()
                .mt(z(if ix == 0 { 0. } else { 32. }))
                .flex()
                .items_center()
                .gap(z(4.))
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(if e.prerelease { idx::TEXT_FEEDBACK_WARNING } else { idx::TEXT_BRAND })))
                .child(SharedString::from(e.name.to_uppercase()))
                .child(div().flex_1().h(z(1.)).opacity(0.6).bg(c(tok(if e.prerelease { idx::TEXT_FEEDBACK_WARNING } else { idx::TEXT_BRAND }))))
                .into_any_element(),
        );
        if !date.is_empty() {
            out.push(div().mt(z(4.)).text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_MUTED))).child(SharedString::from(date)).into_any_element());
        }
        for block in parse(&e.body, &NoResolve) {
            let el = match block {
                Block::Heading(_, inline) => body_text(div().mt(z(16.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG)))).child(changelog_inline(&inline)),
                Block::ListItem { depth, inline, ordered } => body_text(div().mt(z(4.)).mb(z(12.)).ml(z(20. + f32::from(depth) * 20.)).flex().gap(z(8.)))
                    .child(div().flex_none().text_color(c(tok(idx::TEXT_MUTED))).child(SharedString::from(ordered.map_or("•".to_owned(), |n| format!("{n}.")))))
                    .child(div().flex_1().min_w_0().child(changelog_inline(&inline))),
                Block::Code { code, .. } => body_text(div().my(z(8.)).p(z(8.)).rounded(z(4.)).bg(c(tok(idx::BACKGROUND_BASE_LOWER))).text_size(z(14.))).child(SharedString::from(code)),
                Block::Paragraph(inline) | Block::Quote(inline) | Block::Subtext(inline) => body_text(div().my(z(8.))).child(changelog_inline(&inline)),
            };
            out.push(el.into_any_element());
        }
    }
    out
}

fn changelog_modal(cx: &mut Cx) -> AnyElement {
    let e = cx.entity();
    let close: crate::ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        e.update(cx, |a, cx| {
            if let Some(s) = st(a) {
                s.show_changelog = false;
            }
            cx.notify();
        })
    });
    let state = crate::updater::changelog();
    let content: Vec<AnyElement> = match &state {
        crate::updater::Changelog::Loaded(list) if !list.is_empty() => changelog_body(list),
        crate::updater::Changelog::Loading => vec![div().py(z(40.)).flex().justify_center().text_size(z(14.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("common.loading")).into_any_element()],
        _ => vec![
            div()
                .py(z(8.))
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(crate::t!("settings.changelog.unavailable"))
                .into_any_element(),
        ],
    };
    let close_btn = close.clone();
    let card = div()
        .relative()
        .w(z(480.))
        .max_h(z(720.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .pt(z(24.))
                .px(z(24.))
                .pb(z(16.))
                .pr(z(60.))
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(crate::t!("settings.more.changelog")))
                .child(div().text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::diagnostics::version_line())),
        )
        .child(
            div()
                .id("changelog-close")
                .absolute()
                .top(z(8.))
                .right(z(8.))
                .size(z(40.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                .on_click(move |_, w, cx| close_btn(w, cx))
                .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        )
        .child(div().id("changelog-body").flex_1().min_h_0().overflow_y_scroll().px(z(24.)).pb(z(20.)).flex().flex_col().children(content))
        .child(
            div()
                .flex_none()
                .px(z(24.))
                .py(z(16.))
                .border_t_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .flex()
                .child(
                    div()
                        .id("changelog-releases")
                        .cursor_pointer()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(c(tok(idx::TEXT_LINK)))
                        .hover(|s| s.underline())
                        .on_click(|_, _, cx| cx.open_url(crate::updater::RELEASES_PAGE_URL))
                        .child(crate::t!("settings.changelog.all_releases")),
                ),
        );
    crate::ui::modal::frame("settings-changelog", close, card)
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Cx) -> Option<AnyElement> {
    let s = app.settings.as_ref()?;
    if s.page == Page::Profiles {
        return Some(super::profile_editor::render(app, window, cx));
    }
    if s.search.is_none() {
        cx.defer_in(window, |a, window, cx| {
            if a.settings.as_ref().is_some_and(|s| s.search.is_none()) {
                let input = cx.new(|cx| gpui_component::input::InputState::new(window, cx).placeholder(crate::t!("settings.search")));
                let sub = cx.observe(&input, |_, _, cx| cx.notify());
                if let Some(s) = a.settings.as_mut() {
                    s.search = Some(input);
                    s.search_sub = Some(sub);
                }
                cx.notify();
            }
        });
    }
    let error = s.error.clone();
    let confirm = s.confirm_logout;
    let changelog = s.show_changelog;
    let hwaccel = s.confirm_hwaccel;
    let page_label = s.page.label();
    let body = div()
        .id("settings-content")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&s.scroll)
        .child(
            div()
                .mx_auto()
                .py(z(64.))
                .w_full()
                .max_w(z(728.))
                .px(z(16.))
                .flex()
                .flex_col()
                .when_some(error, |d, e| {
                    d.child(
                        div()
                            .mb(z(16.))
                            .p(z(12.))
                            .rounded(z(8.))
                            .bg(c(tok(idx::BACKGROUND_FEEDBACK_CRITICAL)))
                            .border_1()
                            .border_color(c(tok(idx::BORDER_FEEDBACK_CRITICAL)))
                            .text_color(c(tok(idx::TEXT_DEFAULT)))
                            .text_size(z(14.))
                            .child(e),
                    )
                })
                .child(page(app, cx)),
        );
    let card = crate::ui::a11y::named(div().id("user-settings-card"), gpui::Role::Dialog, || crate::t!("a11y.user_settings"))
        .size_full()
        .max_w(z(1400.))
        .rounded(z(12.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
        .overflow_hidden()
        .occlude()
        .flex()
        .child(nav(app, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .child(content_header(page_label, s.nested, cx))
                .child(crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Auto).flex_1().min_h_0().child(body).children(crate::ui::chat::scrollbar::auto_scrollbar_div(&s.scroll))),
        );
    Some(
        div()
            .id("user-settings")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|a, _, _, cx| {
                a.settings = None;
                cx.notify();
            }))
            .child(crate::ui::popout_motion::modal_backdrop("user-settings", div().absolute().inset_0().bg(c(tok(idx::BACKGROUND_SCRIM)))))
            .child(crate::ui::popout_motion::modal_card(
                "user-settings",
                div()
                    .absolute()
                    .inset(z(40.))
                    .flex()
                    .justify_center()
                    .child(card.on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())),
            ))
            .when(confirm, |d| d.child(logout_modal(cx)))
            .when(changelog, |d| d.child(changelog_modal(cx)))
            .children(super::authorized_apps::confirm_modal(cx))
            .when_some(hwaccel, |d, on| d.child(hwaccel_modal(on, cx)))
            .into_any_element(),
    )
}

use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, img, prelude::*};
use gpui_component::input::{Input, InputState};

use super::{Modal, card, close_action, field_label};
use crate::{
    actions::{close_modal, guard::GuardedWrite, open_modal, with_modal},
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

type Cx<'a> = Context<'a, PhoenixApp>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmailStep {
    Start,
    Code,
    Reasons,
    Warning,
    Form,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    RemovePhone,
    DisableSms,
    BackupCodes,
    DisableAccount,
    DeleteAccount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Password,
    Username,
    Email(EmailStep),
    Phone,
    PhoneEntry,
    PhoneVerify,
    PhonePassword,
    Confirm(Confirm),
    RemoveTotp,
}

pub struct State {
    pub flow: Flow,
    pub inputs: Vec<Entity<InputState>>,
    built: Option<Flow>,
    pub errors: Vec<Option<SharedString>>,
    pub reason: Option<u8>,
    pub country: usize,
    pub country_open: bool,
    pub blocked: bool,
    pub pending: String,
    pub username: String,
}

struct Spec {
    label: &'static str,
    password: bool,
    required: bool,
}

const fn spec(label: &'static str, password: bool, required: bool) -> Spec {
    Spec { label, password, required }
}

fn specs(flow: Flow) -> &'static [Spec] {
    const PASSWORD: &[Spec] =
        &[spec("acct.current_password", true, true), spec("acct.new_password", true, true), spec("acct.confirm_new_password", true, true)];
    const USERNAME: &[Spec] = &[spec("acct.username", false, false), spec("acct.current_password", true, false)];
    const CODE: &[Spec] = &[spec("acct.verification_code", false, false)];
    const EMAIL: &[Spec] = &[spec("acct.email", false, false), spec("acct.current_password", true, false)];
    const PHONE: &[Spec] = &[spec("acct.phone_number", false, false)];
    const CONFIRM: &[Spec] = &[spec("acct.password", true, true)];
    match flow {
        Flow::Password => PASSWORD,
        Flow::Username => USERNAME,
        Flow::Email(EmailStep::Code) | Flow::PhoneVerify => CODE,
        Flow::Email(EmailStep::Form) => EMAIL,
        Flow::PhoneEntry => PHONE,
        Flow::PhonePassword | Flow::Confirm(_) => CONFIRM,
        _ => &[],
    }
}

pub const SCAM_ARTICLE: &str = "https://discord.com/safety/understanding-and-avoiding-common-scams";
pub const EMAIL_ART: &str = "https://discord.com/assets/902087e396aa9987.svg";
pub const SCAM_ART: &str = "https://cdn.discordapp.com/assets/content/971d3a140aa11a3830346a83266ea28b2f4e586e9b4f02eed4315538f6a9ebbe.svg";
const WARN_REASONS: [u8; 2] = [0, 1];

pub fn open(flow: Flow, username: String, window: Option<&mut Window>) {
    let country = super::countries::default_index();
    open_modal(
        Modal::Account(State {
            flow,
            inputs: Vec::new(),
            built: None,
            errors: Vec::new(),
            reason: None,
            country,
            country_open: false,
            blocked: false,
            pending: String::new(),
            username,
        }),
        window,
    );
}

pub fn prepare(s: &mut State, window: &mut Window, cx: &mut Cx) {
    if s.built == Some(s.flow) {
        return;
    }
    s.built = Some(s.flow);
    s.inputs = specs(s.flow)
        .iter()
        .enumerate()
        .map(|(n, sp)| {
            let i = cx.new(|cx| InputState::new(window, cx).masked(sp.password));
            if s.flow == Flow::Username && n == 0 && !s.username.is_empty() {
                let v = s.username.clone();
                i.update(cx, |st, cx| st.set_value(v, window, cx));
            }
            if n == 0 {
                i.update(cx, |st, cx| st.focus(window, cx));
            }
            i
        })
        .collect();
    s.errors = vec![None; s.inputs.len()];
    s.blocked = false;
}

fn go(flow: Flow, cx: &mut Cx) {
    with_modal(|m| {
        if let Some(Modal::Account(s)) = m {
            s.flow = flow;
        }
    });
    cx.notify();
}

pub fn username_error(name: &str, current: &str) -> Option<&'static str> {
    if name.is_empty() || name == current {
        return None;
    }
    if !name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.') {
        Some("acct.username_chars")
    } else if name.contains("..") {
        Some("acct.username_dots")
    } else if !(2..=32).contains(&name.chars().count()) {
        Some("acct.username_length")
    } else {
        None
    }
}

pub fn password_errors(new: &str, confirm: &str) -> [Option<&'static str>; 2] {
    [new.is_empty().then_some("acct.new_password_empty"), (new != confirm).then_some("acct.password_mismatch")]
}

pub fn email_next(reason: u8) -> EmailStep {
    if WARN_REASONS.contains(&reason) { EmailStep::Warning } else { EmailStep::Form }
}

pub fn write_for(flow: Flow, values: &[String], phone: &str) -> Option<GuardedWrite> {
    let v = |n: usize| values.get(n).map(String::as_str).unwrap_or_default();
    Some(match flow {
        Flow::Password => GuardedWrite::change_password(v(0), v(1)),
        Flow::Username => GuardedWrite::change_username(v(0), None, v(1)),
        Flow::Email(EmailStep::Start) => GuardedWrite::send_email_code(),
        Flow::Email(EmailStep::Code) => GuardedWrite::verify_email_code(v(0)),
        Flow::Email(EmailStep::Form) => GuardedWrite::change_email(v(0), None, v(1)),
        Flow::PhoneEntry => GuardedWrite::begin_add_phone(phone),
        Flow::PhoneVerify => GuardedWrite::verify_phone(phone, v(0)),
        Flow::PhonePassword => GuardedWrite::add_phone("", v(0)),
        Flow::Confirm(Confirm::RemovePhone) => GuardedWrite::remove_phone(v(0)),
        Flow::Confirm(Confirm::DisableSms) => GuardedWrite::disable_sms(v(0)),
        Flow::Confirm(Confirm::BackupCodes) => GuardedWrite::backup_codes_challenge(v(0)),
        Flow::Confirm(Confirm::DisableAccount) => GuardedWrite::disable_account(v(0)),
        Flow::Confirm(Confirm::DeleteAccount) => GuardedWrite::delete_account(v(0)),
        Flow::RemoveTotp => GuardedWrite::disable_totp(),
        _ => return None,
    })
}

fn values(s: &State, cx: &gpui::App) -> Vec<String> {
    s.inputs.iter().map(|i| i.read(cx).value().to_string()).collect()
}

fn full_phone(s: &State, values: &[String]) -> String {
    let code = super::countries::COUNTRIES.get(s.country).map(|c| c.code).unwrap_or("+1");
    format!("{code}{}", values.first().map(String::as_str).unwrap_or_default())
}

fn submit(app: &mut PhoenixApp, cx: &mut Cx) {
    let picked = with_modal(|m| match m {
        Some(Modal::Account(s)) => {
            let vals = values(s, cx);
            match s.flow {
                Flow::PhoneEntry => s.pending = full_phone(s, &vals),
                Flow::Email(EmailStep::Form) => s.pending = vals.first().cloned().unwrap_or_default(),
                _ => {}
            }
            let phone = if s.flow == Flow::PhoneEntry { full_phone(s, &vals) } else { s.pending.clone() };
            Some((s.flow, vals, phone))
        }
        _ => None,
    });
    let Some((flow, vals, phone)) = picked else { return };
    if flow == Flow::Password {
        let errs = password_errors(&vals[1], &vals[2]);
        if errs.iter().any(Option::is_some) || vals[0].is_empty() {
            with_modal(|m| {
                if let Some(Modal::Account(s)) = m {
                    s.errors[1] = errs[0].map(|k| crate::t!(k));
                    s.errors[2] = errs[1].map(|k| crate::t!(k));
                }
            });
            cx.notify();
            return;
        }
    }
    let Some(w) = write_for(flow, &vals, &phone) else { return };
    let fut = app.backend.guarded_write(w);
    cx.spawn(async move |this, cx| {
        let blocked = fut.await.is_err();
        let _ = this.update(cx, |_, cx| {
            with_modal(|m| {
                if let Some(Modal::Account(s)) = m {
                    s.blocked = blocked;
                }
            });
            cx.notify();
        });
    })
    .detach();
}

fn error_row(t: SharedString) -> gpui::Div {
    div()
        .mt(z(8.))
        .flex()
        .items_center()
        .gap(z(4.))
        .child(gpui::svg().path(crate::icons::path("settings/circle-error")).size(z(16.)).flex_none().text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))))
        .child(div().text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child(t))
}

fn helper(t: SharedString) -> gpui::Div {
    div().mt(z(8.)).text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(t)
}

fn input(i: &Entity<InputState>, password: bool, leading: Option<&'static str>) -> gpui::Div {
    let field = Input::new(i).appearance(false).bordered(false).px(z(0.)).text_size(z(16.)).line_height(z(20.));
    let field = if password { field.mask_toggle() } else { field };
    div()
        .h(z(40.))
        .px(z(12.))
        .gap(z(8.))
        .rounded(z(8.))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .flex()
        .items_center()
        .text_size(z(16.))
        .children(leading.map(|l| div().flex_none().text_color(c(tok(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT))).child(l)))
        .child(div().flex_1().min_w_0().child(field))
}

fn fields(s: &State, app: &PhoenixApp, cx: &gpui::App) -> gpui::Div {
    let gap = if s.flow == Flow::Username { 16. } else { 20. };
    let mut col = div().flex().flex_col().gap(z(gap));
    for (n, (sp, i)) in specs(s.flow).iter().zip(&s.inputs).enumerate() {
        let leading = (s.flow == Flow::PhoneEntry).then(|| super::countries::COUNTRIES.get(s.country).map(|c| c.code)).flatten();
        let mut f = div().flex().flex_col().child(field_label(crate::t!(sp.label), sp.required)).child(input(i, sp.password, leading));
        if let Some(e) = s.errors.get(n).cloned().flatten() {
            f = f.child(error_row(e));
        } else if s.flow == Flow::Username && n == 0 {
            let name = i.read(cx).value().to_string();
            f = match username_error(&name, &app.me.name) {
                Some(k) => f.child(error_row(crate::t!(k))),
                None => f.child(helper(crate::t!("acct.username_chars"))),
            };
        }
        col = col.child(f);
    }
    col
}

fn text(t: SharedString) -> gpui::Div {
    div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(t)
}

fn hero(art: &'static str, title: SharedString, subtitle: Option<SharedString>, body: gpui::Div) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(z(8.))
        .child(img(art).h(z(120.)).max_w(z(200.)).object_fit(gpui::ObjectFit::Contain))
        .child(div().text_size(z(24.)).line_height(z(30.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).text_center().child(title))
        .children(subtitle.map(|t| div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_SUBTLE))).text_center().child(t)))
        .child(div().w_full().pt(z(16.)).child(body))
        .into_any_element()
}

type Run = fn(&mut PhoenixApp, &mut Cx);

struct Act {
    id: &'static str,
    label: SharedString,
    kind: BtnKind,
    critical_secondary: bool,
    disabled: bool,
    run: Run,
}

fn act(id: &'static str, label: SharedString, kind: BtnKind, run: Run) -> Act {
    Act { id, label, kind, critical_secondary: false, disabled: false, run }
}

fn cancel(_: &mut PhoenixApp, cx: &mut Cx) {
    close_modal();
    cx.notify();
}

fn footer(acts: Vec<Act>, full: bool, cx: &mut Cx) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .gap(z(8.))
        .justify_end()
        .children(acts.into_iter().map(|a| {
            let run = a.run;
            let mut b = button(a.id, a.label, a.kind);
            if a.critical_secondary {
                b = b
                    .text_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_TEXT_DEFAULT)))
                    .bg(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BACKGROUND_DEFAULT)))
                    .border_color(c(tok(idx::CONTROL_CRITICAL_SECONDARY_BORDER_DEFAULT)));
            }
            if full {
                b = b.flex_1();
            }
            if a.disabled { b.opacity(0.5).cursor_default() } else { b.on_click(cx.listener(move |a, _, _, cx| run(a, cx))) }
        }))
        .into_any_element()
}

fn email_reasons(s: &State, cx: &mut Cx) -> gpui::Div {
    const REASONS: [&str; 4] = ["acct.reason_employee", "acct.reason_someone", "acct.reason_new_email", "acct.reason_other"];
    div().flex().flex_col().gap(z(8.)).children(REASONS.iter().enumerate().map(|(n, k)| {
        let r = n as u8;
        crate::ui::controls::radio_option(("acct-reason", n), crate::t!(k), s.reason == Some(r)).on_click(cx.listener(move |_, _, _, cx| {
            with_modal(|m| {
                if let Some(Modal::Account(s)) = m {
                    s.reason = Some(r);
                }
            });
            cx.notify();
        }))
    }))
}

fn country_picker(s: &State, cx: &mut Cx) -> gpui::Div {
    let list = super::countries::COUNTRIES;
    let cur = list.get(s.country);
    let label = cur.map(|c| c.name()).unwrap_or_default();
    let trigger = div()
        .id("acct-country")
        .h(z(40.))
        .px(z(12.))
        .rounded(z(8.))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .flex()
        .items_center()
        .gap(z(8.))
        .cursor_pointer()
        .child(div().flex_1().min_w_0().truncate().text_size(z(16.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(label))
        .child(gpui::svg().path(crate::icons::path("settings/chevron-down")).size(z(16.)).text_color(c(tok(idx::INPUT_ICON_DEFAULT))))
        .on_click(cx.listener(|_, _, _, cx| {
            with_modal(|m| {
                if let Some(Modal::Account(s)) = m {
                    s.country_open = !s.country_open;
                }
            });
            cx.notify();
        }));
    let mut col = div().flex().flex_col().child(field_label(crate::t!("acct.country_code"), false)).child(trigger);
    if s.country_open {
        col = col.child(
            div()
                .id("acct-country-list")
                .mt(z(4.))
                .max_h(z(240.))
                .overflow_y_scroll()
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .p(z(8.))
                .children(list.iter().enumerate().map(|(n, ct)| {
                    div()
                        .id(("acct-country-row", n))
                        .h(z(36.))
                        .px(z(8.))
                        .rounded(z(4.))
                        .flex()
                        .items_center()
                        .gap(z(8.))
                        .cursor_pointer()
                        .hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                        .when(n == s.country, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                        .child(div().flex_1().min_w_0().truncate().text_size(z(14.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(ct.name()))
                        .child(div().flex_none().text_size(z(14.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(ct.code))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            with_modal(|m| {
                                if let Some(Modal::Account(s)) = m {
                                    s.country = n;
                                    s.country_open = false;
                                }
                            });
                            cx.notify();
                        }))
                })),
        );
    }
    col
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let info = crate::ui::settings::account::info();
    let t = |k: &str| crate::t!(k);
    let mut body = fields(s, app, cx);
    let empty = s.inputs.first().is_none_or(|i| i.read(cx).value().is_empty());
    let next = |id: &'static str, label: SharedString, run: Run| act(id, label, BtnKind::Primary, run);
    let (width, title, subtitle, acts, full): (f32, SharedString, Option<SharedString>, Vec<Act>, bool) = match s.flow {
        Flow::Password => (
            480.,
            t("acct.password_title"),
            Some(t("acct.password_sub")),
            vec![act("acct-cancel", t("common.cancel"), BtnKind::Secondary, cancel), next("acct-done", t("common.done"), submit)],
            false,
        ),
        Flow::Username => (
            480.,
            t("acct.username_title"),
            Some(t("acct.username_sub")),
            vec![act("acct-cancel", t("common.cancel"), BtnKind::Secondary, cancel), next("acct-done", t("common.done"), submit)],
            false,
        ),
        Flow::Phone => {
            let phone = info.phone.clone().unwrap_or_default();
            body = div().flex().items_center().gap(z(16.)).child(
                div()
                    .size(z(48.))
                    .flex_none()
                    .rounded_full()
                    .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(gpui::svg().path(crate::icons::path("settings/mobile-phone")).size(z(24.)).text_color(c(tok(idx::ICON_STRONG)))),
            )
            .child(div().flex().flex_col().gap(z(4.)).child(text(t("acct.phone_current"))).child(text(phone.into())));
            let mut acts = Vec::new();
            if info.email.is_some() {
                let mut rm = act("acct-phone-remove", t("acct.remove"), BtnKind::Secondary, |_, cx| go(Flow::Confirm(Confirm::RemovePhone), cx));
                rm.critical_secondary = true;
                acts.push(rm);
            }
            acts.push(next("acct-phone-edit", t("common.edit"), |_, cx| go(Flow::PhoneEntry, cx)));
            (480., t("acct.phone_title"), None, acts, false)
        }
        Flow::PhoneEntry => {
            body = div()
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(div().flex().flex_col().gap(z(16.)).child(country_picker(s, cx)).child(body))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(t("acct.phone_note")));
            let mut send = next("acct-phone-send", t("acct.send"), submit);
            send.disabled = empty;
            (480., t("acct.phone_enter_title"), Some(t("acct.phone_enter_sub")), vec![send], false)
        }
        Flow::PhoneVerify => (
            400.,
            t("acct.phone_verify_title"),
            Some(crate::t!("acct.phone_verify_sub", phone = s.pending.clone())),
            vec![act("acct-phone-resend", t("acct.resend_code"), BtnKind::Secondary, resend_phone), next("acct-done", t("common.next"), submit)],
            false,
        ),
        Flow::PhonePassword | Flow::Confirm(_) => {
            let plain = match s.flow {
                Flow::Confirm(Confirm::DisableAccount) => Some(t("acct.disable_account_body")),
                Flow::Confirm(Confirm::DeleteAccount) => Some(t("acct.delete_account_body")),
                _ => None,
            };
            let (title, warn, label, critical) = match s.flow {
                Flow::Confirm(Confirm::DisableAccount) => (t("acct.disable_account_title"), None, t("settings.account.disable_btn"), true),
                Flow::Confirm(Confirm::DeleteAccount) => (t("acct.delete_account_title"), None, t("settings.account.delete_btn"), true),
                Flow::Confirm(Confirm::RemovePhone) => (
                    t("acct.remove_phone_title"),
                    info.sms.then(|| t("acct.remove_phone_sms")),
                    t("acct.remove"),
                    true,
                ),
                Flow::Confirm(Confirm::DisableSms) => (t("acct.disable_sms_title"), Some(t("acct.disable_sms_body")), t("acct.confirm"), false),
                Flow::Confirm(Confirm::BackupCodes) => (t("acct.enter_password"), None, t("common.next"), false),
                _ => (t("acct.phone_password_title"), None, t("acct.confirm"), false),
            };
            if let Some(w) = warn {
                body = div()
                    .flex()
                    .flex_col()
                    .gap(z(16.))
                    .child(
                        div()
                            .p(z(12.))
                            .rounded(z(8.))
                            .border_1()
                            .border_color(c(tok(idx::BORDER_FEEDBACK_WARNING)))
                            .bg(c(tok(idx::BACKGROUND_FEEDBACK_WARNING)))
                            .child(text(w)),
                    )
                    .child(body);
            }
            if let Some(p) = plain {
                body = div().flex().flex_col().gap(z(16.)).child(text(p)).child(body);
            }
            let mut ok = act("acct-confirm", label, if critical { BtnKind::Critical } else { BtnKind::Primary }, submit);
            ok.disabled = empty;
            (480., title, None, vec![act("acct-cancel", t("common.cancel"), BtnKind::Secondary, cancel), ok], false)
        }
        Flow::RemoveTotp => (
            480.,
            t("acct.remove_totp_title"),
            Some(t("acct.remove_totp_sub")),
            vec![act("acct-cancel", t("common.cancel"), BtnKind::Secondary, cancel), act("acct-confirm", t("acct.remove"), BtnKind::Critical, submit)],
            false,
        ),
        Flow::Email(step) => return email(s, step, body, cx),
    };
    if s.blocked {
        body = body.child(error_row(t("acct.blocked")));
    }
    card(width, title, subtitle, body.into_any_element(), Some(footer(acts, full, cx)), cx)
}

fn email(s: &State, step: EmailStep, fields: gpui::Div, cx: &mut Cx) -> AnyElement {
    let t = |k: &str| crate::t!(k);
    let info = crate::ui::settings::account::info();
    let next = |id: &'static str, label: SharedString, run: Run| act(id, label, BtnKind::Primary, run);
    let cancel_act = || act("acct-cancel", t("common.cancel"), BtnKind::Secondary, cancel);
    let (art, title, subtitle, mut body, acts): (&'static str, SharedString, Option<SharedString>, gpui::Div, Vec<Act>) = match step {
        EmailStep::Start => (
            EMAIL_ART,
            t("acct.email_verify_title"),
            None,
            div()
                .flex()
                .flex_col()
                .gap(z(16.))
                .child(text(crate::t!("acct.email_verify_body", email = info.email.clone().unwrap_or_default())))
                .child(text(t("acct.email_verify_lost"))),
            vec![cancel_act(), next("acct-send-code", t("acct.send_code"), submit)],
        ),
        EmailStep::Code => (
            EMAIL_ART,
            t("acct.email_code_title"),
            Some(crate::t!("acct.email_code_sub", email = info.email.clone().unwrap_or_default())),
            fields.child(
                div()
                    .id("acct-resend")
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .text_color(c(tok(idx::TEXT_LINK)))
                    .cursor_pointer()
                    .child(t("acct.email_resend")),
            ),
            vec![next("acct-next", t("common.next"), submit)],
        ),
        EmailStep::Reasons => {
            let mut go_on = next("acct-next", t("acct.continue"), |_, cx| {
                let r = with_modal(|m| if let Some(Modal::Account(s)) = m { s.reason } else { None });
                if let Some(r) = r {
                    go(Flow::Email(email_next(r)), cx);
                }
            });
            go_on.disabled = s.reason.is_none();
            (EMAIL_ART, t("acct.email_reason_title"), None, email_reasons(s, cx), vec![cancel_act(), go_on])
        }
        EmailStep::Warning => (
            SCAM_ART,
            t("acct.scam_title"),
            None,
            div()
                .flex()
                .flex_col()
                .gap(z(16.))
                .child(text(t("acct.scam_body")))
                .child(text(t("acct.scam_never"))),
            vec![
                act("acct-risk", t("acct.scam_continue"), BtnKind::Secondary, |_, cx| go(Flow::Email(EmailStep::Form), cx)),
                act("acct-cancel", t("common.cancel"), BtnKind::Primary, cancel),
            ],
        ),
        EmailStep::Form => {
            let back_to = if s.reason.is_some_and(|r| WARN_REASONS.contains(&r)) { EmailStep::Warning } else { EmailStep::Reasons };
            let back: Run = if back_to == EmailStep::Warning {
                |_, cx| go(Flow::Email(EmailStep::Warning), cx)
            } else {
                |_, cx| go(Flow::Email(EmailStep::Reasons), cx)
            };
            (
                EMAIL_ART,
                t("acct.email_form_title"),
                Some(t("acct.email_form_sub")),
                fields,
                vec![act("acct-back", t("acct.back"), BtnKind::Secondary, back), next("acct-done", t("common.done"), submit)],
            )
        }
        EmailStep::Done => (
            EMAIL_ART,
            t("acct.email_done_title"),
            None,
            div()
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(text(crate::t!("acct.email_done_body", email = s.pending.clone())))
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(t("acct.email_done_spam"))),
            vec![next("acct-ok", t("acct.ok"), cancel)],
        ),
    };
    if s.blocked {
        body = body.child(error_row(t("acct.blocked")));
    }
    bare_card(480., hero(art, title, subtitle, body), footer(acts, true, cx), cx)
}

fn bare_card(width: f32, body: AnyElement, footer: AnyElement, cx: &mut Cx) -> AnyElement {
    let close = close_action(cx);
    let close2 = close.clone();
    let el = div()
        .relative()
        .w(z(width))
        .max_h(z(720.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .pt(z(24.))
        .pb(z(24.))
        .px(z(24.))
        .flex()
        .flex_col()
        .gap(z(24.))
        .child(body)
        .child(footer)
        .child(
            div()
                .id("amodal-close")
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
                .on_click(move |_, w, cx| close(w, cx))
                .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        );
    crate::ui::modal::frame("amodal-backdrop", close2, el)
}

fn resend_phone(app: &mut PhoenixApp, cx: &mut Cx) {
    let phone = with_modal(|m| match m {
        Some(Modal::Account(s)) => Some(s.pending.clone()),
        _ => None,
    });
    if let Some(p) = phone {
        let fut = app.backend.guarded_write(GuardedWrite::begin_add_phone(&p));
        cx.spawn(async move |this, cx| {
            let blocked = fut.await.is_err();
            let _ = this.update(cx, |_, cx| {
                with_modal(|m| {
                    if let Some(Modal::Account(s)) = m {
                        s.blocked = blocked;
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }
}

impl PhoenixApp {
    pub fn open_account_edit(&mut self, flow: Flow, window: &mut Window, cx: &mut Cx) {
        open(flow, self.me.name.to_string(), Some(window));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_rules_follow_attempt_username() {
        assert_eq!(username_error("", "me"), None);
        assert_eq!(username_error("me", "me"), None);
        assert_eq!(username_error("ab cd", "me"), Some("acct.username_chars"));
        assert_eq!(username_error("Éric", "me"), Some("acct.username_chars"));
        assert_eq!(username_error("a..b", "me"), Some("acct.username_dots"));
        assert_eq!(username_error("a", "me"), Some("acct.username_length"));
        assert_eq!(username_error(&"a".repeat(33), "me"), Some("acct.username_length"));
        assert_eq!(username_error("neo_1.x", "me"), None);
    }

    #[test]
    fn password_checks_run_before_any_request() {
        assert_eq!(password_errors("", ""), [Some("acct.new_password_empty"), None]);
        assert_eq!(password_errors("a", "b"), [None, Some("acct.password_mismatch")]);
        assert_eq!(password_errors("a", "a"), [None, None]);
    }

    #[test]
    fn email_reason_routes_scams_to_the_warning() {
        assert_eq!(email_next(0), EmailStep::Warning);
        assert_eq!(email_next(1), EmailStep::Warning);
        assert_eq!(email_next(2), EmailStep::Form);
        assert_eq!(email_next(3), EmailStep::Form);
    }

    #[test]
    fn every_final_button_maps_to_a_guarded_write() {
        let vals = vec!["a".to_owned(), "b".to_owned(), "b".to_owned()];
        let flows = [
            Flow::Password,
            Flow::Username,
            Flow::Email(EmailStep::Start),
            Flow::Email(EmailStep::Code),
            Flow::Email(EmailStep::Form),
            Flow::PhoneEntry,
            Flow::PhoneVerify,
            Flow::PhonePassword,
            Flow::Confirm(Confirm::RemovePhone),
            Flow::Confirm(Confirm::DisableSms),
            Flow::Confirm(Confirm::BackupCodes),
            Flow::Confirm(Confirm::DisableAccount),
            Flow::Confirm(Confirm::DeleteAccount),
            Flow::RemoveTotp,
        ];
        for f in flows {
            let w = write_for(f, &vals, "+33600000000").unwrap_or_else(|| panic!("{f:?} has no write"));
            assert_ne!(w.method, reqwest::Method::GET);
            assert!(w.refusal().is_write_blocked());
        }
        assert!(write_for(Flow::Phone, &vals, "").is_none());
        assert!(write_for(Flow::Email(EmailStep::Reasons), &vals, "").is_none());
    }

    #[test]
    fn the_demo_backend_refuses_guarded_writes() {
        let b = crate::mock::MockBackend::new();
        let res = futures::executor::block_on(crate::backend::Backend::guarded_write(&b, GuardedWrite::change_password("a", "b")));
        assert!(res.is_err());
    }
}

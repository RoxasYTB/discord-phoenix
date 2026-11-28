use gpui::{
    AnyElement, Context, Focusable, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*,
};
use gpui_component::input::Input;

use crate::{
    app::PhoenixApp,
    auth::{self, MfaFlow, MfaSlide},
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

type Cx<'a> = Context<'a, PhoenixApp>;

pub const WIDTH: f32 = 480.;

fn text(t: impl Into<SharedString>, size: f32, line: f32, weight: FontWeight, color: usize) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(c(tok(color))).child(t.into())
}

fn header(subtitle: Option<SharedString>) -> impl IntoElement {
    div()
        .relative()
        .flex_none()
        .pt(z(24.))
        .px(z(24.))
        .flex()
        .flex_col()
        .items_start()
        .gap(z(8.))
        .child(text(crate::t!("login_mfa.title"), 24., 30., FontWeight::SEMIBOLD, idx::TEXT_STRONG))
        .children(subtitle.map(|s| text(s, 20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG)))
}

fn close(cx: &mut Cx) -> impl IntoElement {
    crate::ui::focus_ring::ring(
        crate::ui::a11y::named(div().id("mfa-close"), gpui::Role::Button, || crate::t!("common.close"))
            .absolute()
            .top(z(12.))
            .right(z(12.))
            .size(z(40.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
            .on_click(cx.listener(|a, _, _, cx| auth::go_back(a, cx)))
            .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        crate::ui::states::key("mfa-close"),
        8.,
    )
}

fn error(flow: &MfaFlow) -> Option<gpui::Div> {
    flow.error.clone().map(|e| text(e, 14., 18., FontWeight::NORMAL, idx::TEXT_FEEDBACK_CRITICAL).mt(z(8.)))
}

fn code_field(app: &PhoenixApp, slide: MfaSlide, window: &Window, cx: &Cx) -> gpui::Div {
    let focused = app.login_mfa.focus_handle(cx).is_focused(window);
    let disabled = app.login_ui.mfa.submitting;
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(crate::ui::modals::field_label(crate::t!(slide.label_key()), false))
        .child(
            div()
                .h(z(40.))
                .px(z(12.))
                .rounded(z(8.))
                .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
                .border_1()
                .border_color(c(tok(if focused { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_DEFAULT })))
                .hover(|s| s.border_color(c(tok(if focused { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_HOVER }))))
                .flex()
                .items_center()
                .text_size(z(16.))
                .when(disabled, |d| d.opacity(0.5))
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&app.login_mfa)
                            .appearance(false)
                            .bordered(false)
                            .disabled(disabled)
                            .px(z(0.))
                            .text_size(z(16.))
                            .line_height(z(20.)),
                    ),
                ),
        )
}

fn action(id: &'static str, label: SharedString, kind: BtnKind, enabled: bool, loading: bool, run: fn(&mut PhoenixApp, &mut Cx), cx: &mut Cx) -> AnyElement {
    let b = if loading {
        button(id, SharedString::default(), kind).child(crate::ui::spinner::low_motion())
    } else {
        button(id, label, kind)
    }
    .w_full();
    let b = if enabled && !loading { b.on_click(cx.listener(move |a, _, _, cx| run(a, cx))) } else { b.opacity(0.5).cursor_default() };
    crate::ui::focus_ring::ring(b.relative(), crate::ui::states::key(id), 8.).into_any_element()
}

fn choose(app: &mut PhoenixApp, cx: &mut Cx) {
    auth::set_slide(app, MfaSlide::Select, cx);
}

fn backup_ack(app: &mut PhoenixApp, cx: &mut Cx) {
    auth::set_slide(app, MfaSlide::BackupAck, cx);
}

fn footer(flow: &MfaFlow, trailing: Option<AnyElement>, cx: &mut Cx) -> Option<AnyElement> {
    if trailing.is_none() && !flow.can_choose() {
        return None;
    }
    let other = flow
        .can_choose()
        .then(|| action("mfa-other", crate::t!("login_mfa.other"), BtnKind::Secondary, true, false, choose, cx));
    Some(div().px(z(24.)).pb(z(24.)).flex().flex_col().gap(z(8.)).children(other).children(trailing).into_any_element())
}

fn confirm(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let flow = &app.login_ui.mfa;
    let code = app.login_mfa.read(cx).value().to_string();
    action("mfa-confirm", crate::t!("login_mfa.confirm"), BtnKind::Primary, flow.slide.can_submit(&code), flow.submitting, auth::submit_mfa, cx)
}

fn content(children: Vec<AnyElement>) -> AnyElement {
    div().pt(z(24.)).px(z(24.)).pb(z(24.)).flex().flex_col().children(children).into_any_element()
}

fn list(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let rows = app.login_ui.mfa.methods.iter().map(|&m| {
        crate::ui::focus_ring::ring(
            crate::ui::a11y::named(div().id(SharedString::from(format!("mfa-pick-{m:?}"))), gpui::Role::Button, move || crate::t!(m.choice_key()))
                .w_full()
                .mb(z(8.))
                .pl(z(8.))
                .flex()
                .items_center()
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .active(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE))))
                .cursor_pointer()
                .on_click(cx.listener(move |a, _, _, cx| auth::set_slide(a, m.slide(), cx)))
                .child(text(crate::t!(m.choice_key()), 16., 20., FontWeight::SEMIBOLD, idx::TEXT_DEFAULT).ml(z(8.)).min_w_0().truncate())
                .child(
                    div()
                        .h(z(50.))
                        .ml_auto()
                        .mr(z(16.))
                        .flex()
                        .items_center()
                        .child(gpui::svg().path(crate::icons::path("chevron-right")).size(z(20.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
                ),
            crate::ui::states::key(("mfa-pick", m.choice_key())),
            8.,
        )
        .into_any_element()
    });
    content(rows.collect())
}

fn countdown(wait: u8) -> AnyElement {
    let body = |t: SharedString| text(t, 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT).into_any_element();
    if wait == 0 {
        return body(crate::t!("login_mfa.backup_ready"));
    }
    let n = wait.to_string();
    let s = crate::t!("login_mfa.backup_wait", countdown = n).to_string();
    let hl = s.find(&n).map(|i| (i..i + n.len(), gpui::HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() }));
    div()
        .text_size(z(16.))
        .line_height(z(20.))
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(gpui::StyledText::new(s).with_highlights(hl))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Cx) -> AnyElement {
    let flow = &app.login_ui.mfa;
    let slide = flow.slide;
    let (subtitle, body, foot): (Option<SharedString>, Option<AnyElement>, Option<AnyElement>) = match slide {
        MfaSlide::Select => (Some(crate::t!("login_mfa.select_subtitle")), Some(list(app, cx)), None),
        MfaSlide::Webauthn => {
            let body = error(flow).map(|e| content(vec![e.into_any_element()]));
            let key = action("mfa-webauthn", crate::t!("login_mfa.webauthn"), BtnKind::Primary, true, false, auth::webauthn, cx);
            (None, body, footer(&app.login_ui.mfa, Some(key), cx))
        }
        MfaSlide::Totp | MfaSlide::BackupAck => {
            let mut parts = vec![code_field(app, slide, window, cx).into_any_element()];
            parts.extend(error(flow).map(IntoElement::into_any_element));
            let ok = confirm(app, cx);
            (None, Some(content(parts)), footer(&app.login_ui.mfa, Some(ok), cx))
        }
        MfaSlide::Sms => {
            let subtitle = match &flow.sms_phone {
                Some(p) => crate::t!("login_mfa.sms_sent", phone = p),
                None => crate::t!("login_mfa.sms_sending"),
            };
            let sending = flow.sms_sending;
            let resend = if sending {
                button("mfa-resend", SharedString::default(), BtnKind::Secondary).child(crate::ui::spinner::low_motion())
            } else {
                button("mfa-resend", crate::t!("login_mfa.resend"), BtnKind::Secondary)
                    .on_click(cx.listener(|a, _, _, cx| auth::send_sms(a, false, cx)))
            };
            let row = div()
                .flex()
                .items_end()
                .gap(z(8.))
                .child(code_field(app, slide, window, cx))
                .child(crate::ui::focus_ring::ring(resend.relative(), crate::ui::states::key("mfa-resend"), 8.));
            let mut parts = vec![row.into_any_element()];
            parts.extend(error(flow).map(IntoElement::into_any_element));
            let ok = confirm(app, cx);
            (Some(subtitle), Some(content(parts)), footer(&app.login_ui.mfa, Some(ok), cx))
        }
        MfaSlide::Backup => {
            let wait = flow.backup_wait;
            let body = div()
                .flex()
                .flex_col()
                .child(text(crate::t!("login_mfa.backup_warning_1"), 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT))
                .child(text(crate::t!("login_mfa.backup_warning_2"), 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT))
                .child(countdown(wait))
                .into_any_element();
            let go = action("mfa-backup-go", crate::t!("login_mfa.backup_go"), BtnKind::Primary, wait == 0, wait > 0, backup_ack, cx);
            (Some(crate::t!("login_mfa.backup_subtitle")), Some(content(vec![body])), footer(&app.login_ui.mfa, Some(go), cx))
        }
    };
    crate::ui::a11y::role(div().id("mfa-box"), gpui::Role::Dialog)
        .relative()
        .w(z(WIDTH))
        .rounded(z(8.))
        .bg(c(tok(idx::MODAL_BACKGROUND)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.2),
            offset: gpui::point(z(0.), z(2.)),
            blur_radius: z(10.),
            spread_radius: z(0.),
            inset: false,
        }])
        .flex()
        .flex_col()
        .child(header(subtitle))
        .children(body)
        .children(foot)
        .child(close(cx))
        .into_any_element()
}

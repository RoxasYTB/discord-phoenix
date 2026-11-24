//! Modals opened from the context menu actions (`crate::actions`): forward,
//! reactions, report flow, text forms (thread name, nicknames, reminders,
//! events), mod view, DM call card and the server settings overlay.

pub mod channel_settings;
pub mod create_channel;
pub mod custom_status;
pub mod forward;
pub mod guild_notify;
pub mod member;
pub mod privacy;
pub mod reactions;
pub mod report;
pub mod server_settings;
pub mod settings_layout;

use std::rc::Rc;

use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{Input, InputState};

use crate::{
    actions::{close_modal, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

pub type Submit = Rc<dyn Fn(&mut PhoenixApp, Vec<String>, Option<usize>, &mut Context<PhoenixApp>)>;

pub struct Field {
    pub label: SharedString,
    pub placeholder: SharedString,
    pub initial: String,
    pub input: Option<Entity<InputState>>,
}

impl Field {
    pub fn new(label: SharedString, placeholder: SharedString, initial: impl Into<String>) -> Self {
        Self { label, placeholder, initial: initial.into(), input: None }
    }
}

/// Generic form: text fields, an optional radio list and a submit button.
pub struct Form {
    pub title: SharedString,
    pub subtitle: Option<SharedString>,
    pub fields: Vec<Field>,
    pub choices: Vec<SharedString>,
    pub choice: Option<usize>,
    /// Optional checkbox under the fields: its state is passed to `on_submit`
    /// as one more value after the fields' ("1" / "0").
    pub check: Option<(SharedString, bool)>,
    pub submit: SharedString,
    pub danger: bool,
    pub on_submit: Submit,
}

pub enum Modal {
    Form(Form),
    Forward(forward::Forward),
    Reactions(reactions::Reactions),
    Report(report::Report),
    ModView { guild: Id, user: Id },
    /// DM call card: shown only, no call is started from it.
    Call { channel: Id, user: Id },
    ServerSettings { guild: Id, section: usize },
    CreateChannel(create_channel::State),
    ChannelSettings(channel_settings::State),
    Privacy(privacy::Privacy),
    Security(privacy::Security),
    GuildNotify(guild_notify::State),
    CustomStatus(custom_status::State),
}

/// Create the inputs that need a `Window`.
pub fn prepare(window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let mut mk = |placeholder: SharedString, initial: &str, focus: bool| {
        let i = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        if !initial.is_empty() {
            let v = initial.to_owned();
            i.update(cx, |s, cx| s.set_value(v, window, cx));
        }
        if focus {
            i.update(cx, |s, cx| s.focus(window, cx));
        }
        i
    };
    with_modal(|m| match m {
        Some(Modal::Form(f)) => {
            for (n, fl) in f.fields.iter_mut().enumerate() {
                if fl.input.is_none() {
                    fl.input = Some(mk(fl.placeholder.clone(), &fl.initial, n == 0));
                }
            }
        }
        Some(Modal::Forward(f)) => {
            if f.search.is_none() {
                f.search = Some(mk(crate::t!("fwd.search"), "", true));
            }
            if f.comment.is_none() {
                f.comment = Some(mk(crate::t!("fwd.comment"), "", false));
            }
        }
        Some(Modal::CreateChannel(s)) => create_channel::prepare(s, &mut mk),
        Some(Modal::ChannelSettings(s)) => channel_settings::prepare(s, &mut mk),
        _ => {}
    });
    with_modal(|m| {
        if let Some(Modal::CustomStatus(s)) = m {
            custom_status::prepare(s, window, cx);
        }
    });
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    with_modal(|m| {
        Some(match m.as_ref()? {
            Modal::Form(f) => form(f, cx),
            Modal::Forward(f) => forward::render(app, f, cx),
            Modal::Reactions(r) => reactions::render(app, r, cx),
            Modal::Report(r) => report::render(r, cx),
            Modal::ModView { guild, user } => member::mod_view(app, *guild, *user, cx),
            Modal::Call { channel, user } => member::call_card(app, *channel, *user, cx),
            Modal::ServerSettings { guild, section } => server_settings::render(app, *guild, *section, cx),
            Modal::CreateChannel(s) => create_channel::render(app, s, cx),
            Modal::ChannelSettings(s) => channel_settings::render(app, s, cx),
            Modal::Privacy(p) => privacy::render_privacy(app, p, cx),
            Modal::Security(s) => privacy::render_security(s, cx),
            Modal::GuildNotify(s) => guild_notify::render(app, s, cx),
            Modal::CustomStatus(s) => custom_status::render(app, s, cx),
        })
    })
}

pub fn close_action(cx: &mut Context<PhoenixApp>) -> crate::ui::modal::Action {
    let e = cx.entity();
    Rc::new(move |_, cx| {
        close_modal();
        e.update(cx, |_, cx| cx.notify());
    })
}

/// Card in the confirm dialog style (`container_dvN-MP`): title, optional
/// subtitle, body, footer; 40px close button top right.
pub fn card(width: f32, title: SharedString, subtitle: Option<SharedString>, body: AnyElement, footer: Option<AnyElement>, cx: &mut Context<PhoenixApp>) -> AnyElement {
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
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(
            div()
                .pt(z(16.))
                .px(z(24.))
                .pb(z(8.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(div().pr(z(36.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(title))
                .children(subtitle.map(|s| div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(s))),
        )
        .child(div().px(z(24.)).pt(z(8.)).flex().flex_col().min_h_0().text_size(z(16.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(body))
        .children(footer.map(|f| div().pt(z(16.)).px(z(24.)).pb(z(8.)).flex().gap(z(8.)).justify_end().child(f)))
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

/// 40px text input box (`input_` default/hover border tokens).
pub fn input_box(i: &Entity<InputState>) -> gpui::Div {
    div()
        .h(z(40.))
        .px(z(12.))
        .rounded(z(8.))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .flex()
        .items_center()
        .text_size(z(16.))
        .child(Input::new(i).appearance(false).bordered(false))
}

pub fn label(t: SharedString) -> gpui::Div {
    div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).mb(z(8.)).child(t)
}

pub fn values(f: &Form, cx: &gpui::App) -> Vec<String> {
    let mut v: Vec<String> = f.fields.iter().map(|fl| fl.input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default()).collect();
    if let Some((_, on)) = &f.check {
        v.push(if *on { "1" } else { "0" }.to_string());
    }
    v
}

fn form(f: &Form, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut body = div().flex().flex_col().gap(z(16.));
    for (n, fl) in f.fields.iter().enumerate() {
        let mut col = div().id(("aform-field", n)).flex().flex_col().child(label(fl.label.clone()));
        if let Some(i) = &fl.input {
            col = col.child(input_box(i));
        }
        body = body.child(col);
    }
    for (n, ch) in f.choices.iter().enumerate() {
        let on = f.choice == Some(n);
        body = body.child(crate::ui::controls::radio_option(("aform-choice", n), ch.clone(), on).on_click(cx.listener(move |_, _, _, cx| {
            with_modal(|m| {
                if let Some(Modal::Form(f)) = m {
                    f.choice = Some(n);
                }
            });
            cx.notify();
        })));
    }
    if let Some((text, on)) = f.check.clone() {
        body = body.child(crate::ui::controls::checkbox_option("aform-check", text, on).on_click(cx.listener(move |_, _, _, cx| {
            with_modal(|m| {
                if let Some(Modal::Form(f)) = m
                    && let Some((_, v)) = &mut f.check
                {
                    *v = !*v;
                }
            });
            cx.notify();
        })));
    }
    let footer = div()
        .flex()
        .gap(z(8.))
        .child(button("aform-cancel", crate::t!("common.cancel"), BtnKind::Secondary).on_click({
            let close = close_action(cx);
            move |_, w, cx| close(w, cx)
        }))
        .child(
            button("aform-submit", f.submit.clone(), if f.danger { BtnKind::Critical } else { BtnKind::Primary }).on_click(cx.listener(|a, _, _, cx| {
                let taken = with_modal(|m| match m.take() {
                    Some(Modal::Form(f)) => Some(f),
                    other => {
                        *m = other;
                        None
                    }
                });
                if let Some(f) = taken {
                    let vals = values(&f, cx);
                    (f.on_submit)(a, vals, f.choice, cx);
                }
                cx.notify();
            })),
        );
    card(440., f.title.clone(), f.subtitle.clone(), body.into_any_element(), Some(footer.into_any_element()), cx)
}

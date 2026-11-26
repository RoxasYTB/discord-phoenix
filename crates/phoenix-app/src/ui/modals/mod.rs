pub mod account_edit;
pub mod accounts;
pub mod burst_upsell;
pub mod active_threads;
pub mod channel_overview;
pub mod channel_perms;
pub mod channel_perms_data;
pub mod channel_settings;
pub mod create_channel;
pub mod create_event;
pub mod countries;
pub mod custom_status;
pub mod follow;
pub mod forward;
pub mod friend_requests;
pub mod gdm;
pub mod gdm_cleanup;
pub mod gift_nitro;
pub mod go_live;
pub mod ignore;
pub mod guild_notify;
pub mod guild_tag;
pub mod leave_link;
pub mod member;
pub mod nickname;
pub mod poll_create;
pub mod privacy;
pub mod reactions;
pub mod report;
pub mod report_raid;
pub mod server_settings;
pub mod settings_form;
pub mod settings_layout;
pub mod stream_feedback;
pub mod update;
pub mod voice_status;
pub mod webhooks;

use std::rc::Rc;

use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};

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
    pub rows: usize,
    pub area: Option<Entity<TextareaState>>,
    pub max_len: Option<usize>,
    limit: Option<gpui::Subscription>,
}

impl Field {
    pub fn new(label: SharedString, placeholder: SharedString, initial: impl Into<String>) -> Self {
        Self { label, placeholder, initial: initial.into(), input: None, rows: 1, area: None, max_len: None, limit: None }
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    pub fn max_len(mut self, utf16_units: usize) -> Self {
        self.max_len = Some(utf16_units);
        self
    }

    fn value(&self, cx: &gpui::App) -> String {
        match (&self.area, &self.input) {
            (Some(a), _) => a.read(cx).value().to_string(),
            (None, Some(i)) => i.read(cx).value().to_string(),
            _ => String::new(),
        }
    }
}

pub struct Form {
    pub title: SharedString,
    pub subtitle: Option<SharedString>,
    pub fields: Vec<Field>,
    pub choices: Vec<SharedString>,
    pub choice: Option<usize>,
    pub check: Option<(SharedString, bool)>,
    pub submit: SharedString,
    pub danger: bool,
    pub on_submit: Submit,
    pub preview: Option<(std::sync::Arc<gpui::Image>, u32, u32)>,
    pub width: f32,
}

pub enum Modal {
    Form(Form),
    Forward(forward::Forward),
    Reactions(reactions::Reactions),
    Report(report::Report),
    FollowSuccess { line: usize, art: usize },
    Call { channel: Id, user: Id },
    ServerSettings(Box<server_settings::State>),
    CreateChannel(create_channel::State),
    ChannelSettings(Box<channel_settings::State>),
    Privacy(privacy::Privacy),
    Security(privacy::Security),
    GuildNotify(guild_notify::State),
    CreatePoll(poll_create::State),
    GoLive(go_live::State),
    LeaveLink(leave_link::State),
    ReportRaid(report_raid::State),
    GuildTagAdopt(Id),
    Follow(follow::State),
    EditGroup(gdm::EditGroup),
    GroupInvites(gdm::Invites),
    FolderSettings(gdm::Folder),
    Schedule(crate::ui::chat::scheduled::Picker),
    Nickname(nickname::Nickname),
    CreateEvent(Box<create_event::State>),
    FavThreads(crate::ui::favorites::ThreadSettings),
    SwitchAccounts,
    Ignore(crate::model::Id),
    GdmCleanup(gdm_cleanup::Cleanup),
    CustomStatus(custom_status::CustomStatus),
    ActiveThreads(crate::model::Id),
    PlaintextFull { url: SharedString, filename: SharedString, size: u64, viewport: Option<gpui::Size<gpui::Pixels>> },
    SpamRequests,
    ClearPending(usize),
    AcceptStranger(crate::model::Id),
    GiftNitro(gift_nitro::State),
    Update,
    Account(account_edit::State),
    StreamFeedback(stream_feedback::State),
    ShopProduct(Box<phoenix_api::rest::collectibles::LookSku>),
    ShopPurchase(Box<crate::ui::shop_buy::State>),
    QuestVideo(Box<crate::ui::quests_page::Quest>),
    QuestDisclosure(Box<crate::ui::quests_page::Quest>),
    BurstUpsell,
    VoiceStatus(voice_status::State),
}

pub fn prepare(window: &mut Window, cx: &mut Context<PhoenixApp>) {
    crate::ui::server_pages::prepare(window, cx);
    with_modal(|m| {
        if let Some(Modal::ChannelSettings(s)) = m {
            channel_settings::prepare_area(s, window, cx);
        } else if let Some(Modal::CreatePoll(s)) = m {
            poll_create::prepare_question(s, window, cx);
        } else if let Some(Modal::Nickname(s)) = m {
            nickname::prepare(s, window, cx);
        } else if let Some(Modal::CreateEvent(s)) = m {
            create_event::prepare(s, window, cx);
        } else if let Some(Modal::VoiceStatus(s)) = m {
            voice_status::prepare(s, window, cx);
        }
    });
    with_modal(|m| {
        if let Some(Modal::Form(f)) = m {
            for fl in f.fields.iter_mut().filter(|fl| fl.rows > 1 && fl.area.is_none()) {
                let (ph, rows) = (fl.placeholder.clone(), fl.rows);
                let val = match fl.max_len {
                    Some(max) => truncate_utf16(&fl.initial, max).to_owned(),
                    None => fl.initial.clone(),
                };
                let area = cx.new(|cx| TextareaState::new(window, cx).auto_grow(rows, rows).placeholder(ph).default_value(val.clone()));
                if let Some(max) = fl.max_len {
                    let last = Rc::new(std::cell::RefCell::new(val));
                    fl.limit = Some(cx.subscribe_in(&area, window, move |_, st, ev: &gpui_component::input::InputEvent, window, cx| {
                        if !matches!(ev, gpui_component::input::InputEvent::Change) {
                            return;
                        }
                        let now = st.read(cx).value().to_string();
                        let clamped = clamp_insert(&last.borrow(), &now, max);
                        match clamped {
                            Some((text, cursor)) => {
                                st.update(cx, |s, cx| {
                                    s.set_value(text.clone(), window, cx);
                                    s.set_selected_range(cursor..cursor, cx);
                                });
                                *last.borrow_mut() = text;
                            }
                            None => *last.borrow_mut() = now,
                        }
                    }));
                }
                fl.area = Some(area);
            }
        }
    });
    with_modal(|m| {
        if let Some(Modal::ServerSettings(s)) = m {
            server_settings::prepare(s.as_mut(), window, cx);
        }
    });
    with_modal(|m| {
        if let Some(Modal::Account(s)) = m {
            account_edit::prepare(s, window, cx);
        }
    });
    let vp = window.viewport_size();
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
                if fl.rows == 1 && fl.input.is_none() {
                    fl.input = Some(mk(fl.placeholder.clone(), &fl.initial, n == 0));
                }
            }
        }
        Some(Modal::Forward(f)) => {
            if f.search.is_none() {
                let ph = if f.favorites { crate::t!("fav.add_search") } else { crate::t!("fwd.search") };
                f.search = Some(mk(ph, "", true));
            }
            if f.comment.is_none() && !f.favorites {
                f.comment = Some(mk(crate::t!("fwd.comment"), "", false));
            }
        }
        Some(Modal::CreateChannel(s)) => create_channel::prepare(s, &mut mk),
        Some(Modal::ChannelSettings(s)) => channel_settings::prepare(s, &mut mk),
        Some(Modal::CreatePoll(s)) => poll_create::prepare(s, &mut mk),
        Some(Modal::Report(r)) => r.viewport = Some(vp),
        Some(m @ (Modal::EditGroup(_) | Modal::FolderSettings(_))) => gdm::prepare(m, &mut mk),
        Some(Modal::GoLive(g)) => g.viewport = Some(vp),
        Some(Modal::PlaintextFull { viewport, .. }) => *viewport = Some(vp),
        Some(Modal::CustomStatus(s)) => custom_status::prepare(s, &mut mk),
        _ => {}
    });
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    poll_create::restore_if_dismissed();
    if with_modal(|m| matches!(m, Some(Modal::SpamRequests))) && friend_requests::other_requests(&app.nav.relationships).is_empty() {
        close_modal();
    }
    with_modal(|m| {
        Some(match m.as_ref()? {
            Modal::Form(f) => form(f, cx),
            Modal::Forward(f) => forward::render(app, f, cx),
            Modal::Reactions(r) => reactions::render(app, r, cx),
            Modal::Report(r) => report::render(app, r, cx),
            Modal::FollowSuccess { line, art } => follow::success(*line, *art, cx),
            Modal::Call { channel, user } => member::call_card(app, *channel, *user, cx),
            Modal::ServerSettings(s) => server_settings::render(app, s.as_ref(), cx),
            Modal::CreateChannel(s) => create_channel::render(app, s, cx),
            Modal::ChannelSettings(s) => channel_settings::render(app, s, cx),
            Modal::Privacy(p) => privacy::render_privacy(app, p, cx),
            Modal::Security(s) => privacy::render_security(s, cx),
            Modal::GuildNotify(s) => guild_notify::render(app, s, cx),
            Modal::CreatePoll(s) => poll_create::render(app, s, cx),
            Modal::GoLive(s) => go_live::render(app, s, cx),
            Modal::LeaveLink(s) => leave_link::render(s, cx),
            Modal::ReportRaid(s) => report_raid::render(s, cx),
            Modal::GuildTagAdopt(g) => guild_tag::render(app, *g, cx),
            Modal::StreamFeedback(s) => stream_feedback::render(s, cx),
            Modal::Follow(s) => follow::render(app, s, cx),
            Modal::EditGroup(g) => gdm::render_edit_group(g, cx),
            Modal::GroupInvites(s) => gdm::render_invites(s, cx),
            Modal::FolderSettings(f) => gdm::render_folder(f, cx),
            Modal::Schedule(p) => crate::ui::chat::scheduled::render(p, cx),
            Modal::Nickname(s) => nickname::render(s, cx),
            Modal::CreateEvent(s) => create_event::render(app, s, cx),
            Modal::FavThreads(s) => crate::ui::favorites::render_thread_settings(s, cx),
            Modal::SwitchAccounts => accounts::render(app, cx),
            Modal::Ignore(u) => ignore::render(app, *u, cx),
            Modal::GdmCleanup(s) => gdm_cleanup::render(app, s, cx),
            Modal::CustomStatus(s) => custom_status::render(app, s, cx),
            Modal::ActiveThreads(g) => active_threads::render(app, *g, cx),
            Modal::PlaintextFull { url, filename, size, viewport } => crate::ui::chat::plaintext::render_full(url, filename, *size, *viewport, cx),
            Modal::SpamRequests => friend_requests::render_spam(app, cx),
            Modal::ClearPending(n) => friend_requests::render_clear(*n, cx),
            Modal::AcceptStranger(u) => friend_requests::render_stranger(*u, cx),
            Modal::GiftNitro(s) => gift_nitro::render(app, s, cx),
            Modal::Update => update::render(app, cx),
            Modal::Account(s) => account_edit::render(app, s, cx),
            Modal::ShopProduct(l) => crate::ui::shop_pdp::render(app, l, cx),
            Modal::ShopPurchase(s) => crate::ui::shop_buy::render(app, s, cx),
            Modal::QuestVideo(q) => crate::ui::quest_modals::render_video(q, cx),
            Modal::QuestDisclosure(q) => crate::ui::quest_modals::render_disclosure(q, cx),
            Modal::BurstUpsell => burst_upsell::render(cx),
            Modal::VoiceStatus(s) => voice_status::render(s, cx),
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

pub fn strong_ranges(s: &str) -> (String, Vec<std::ops::Range<usize>>) {
    if s.matches("**").count() % 2 == 1 {
        return (s.to_owned(), Vec::new());
    }
    let mut out = String::new();
    let mut ranges = Vec::new();
    for (i, part) in s.split("**").enumerate() {
        let start = out.len();
        out.push_str(part);
        if i % 2 == 1 {
            ranges.push(start..out.len());
        }
    }
    (out, ranges)
}

fn strong_marked(s: &str) -> gpui::StyledText {
    let (text, ranges) = strong_ranges(s);
    gpui::StyledText::new(text).with_highlights(
        ranges.into_iter().map(|r| (r, gpui::HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })),
    )
}

pub fn card(width: f32, title: SharedString, subtitle: Option<SharedString>, body: AnyElement, footer: Option<AnyElement>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    card_with(width, title, subtitle, body, footer, close_action(cx))
}

pub fn card_with(width: f32, title: SharedString, subtitle: Option<SharedString>, body: AnyElement, footer: Option<AnyElement>, close: crate::ui::modal::Action) -> AnyElement {
    card_rich(width, title, subtitle.map(|s| strong_marked(&s).into_any_element()), body, footer, close)
}

pub fn card_max(width: f32, max_h: f32, title: SharedString, body: AnyElement, cx: &mut Context<PhoenixApp>) -> AnyElement {
    card_sized(width, max_h, title, None, body, None, close_action(cx))
}

pub fn card_rich(width: f32, title: SharedString, subtitle: Option<AnyElement>, body: AnyElement, footer: Option<AnyElement>, close: crate::ui::modal::Action) -> AnyElement {
    card_sized(width, 720., title, subtitle, body, footer, close)
}

fn card_sized(
    width: f32,
    max_h: f32,
    title: SharedString,
    subtitle: Option<AnyElement>,
    body: AnyElement,
    footer: Option<AnyElement>,
    close: crate::ui::modal::Action,
) -> AnyElement {
    let close2 = close.clone();
    let el = div()
        .relative()
        .w(z(width))
        .max_h(z(max_h))
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

fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

fn truncate_utf16(s: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, ch) in s.char_indices() {
        units += ch.len_utf16();
        if units > max {
            return &s[..i];
        }
    }
    s
}

pub(crate) fn clamp_insert(old: &str, new: &str, max: usize) -> Option<(String, usize)> {
    if utf16_len(new) <= max {
        return None;
    }
    let prefix = old.char_indices().zip(new.chars()).take_while(|((_, a), b)| a == b).last().map_or(0, |((i, a), _)| i + a.len_utf8());
    let suffix = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum::<usize>();
    let inserted = &new[prefix..new.len() - suffix];
    let kept = utf16_len(&new[..prefix]) + utf16_len(&new[new.len() - suffix..]);
    let fit = truncate_utf16(inserted, max.saturating_sub(kept));
    let mut text = String::with_capacity(prefix + fit.len() + suffix);
    text.push_str(&new[..prefix]);
    text.push_str(fit);
    text.push_str(&new[new.len() - suffix..]);
    if utf16_len(&text) > max {
        let t = truncate_utf16(&text, max).to_owned();
        let cursor = t.len();
        return Some((t, cursor));
    }
    Some((text, prefix + fit.len()))
}

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
        .child(Input::new(i).appearance(false).bordered(false).px(z(0.)).text_size(z(16.)).line_height(z(20.)))
}

fn area_box(a: &Entity<TextareaState>, rows: usize) -> gpui::Div {
    div()
        .h(z(rows as f32 * 20.8 + 24.))
        .px(z(10.))
        .py(z(12.))
        .rounded(z(8.))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .text_size(z(16.))
        .child(Textarea::new(a).appearance(false).bordered(false))
}

pub fn field_label(t: SharedString, required: bool) -> gpui::Div {
    div()
        .flex()
        .gap(z(4.))
        .mb(z(8.))
        .text_size(z(16.))
        .line_height(z(20.))
        .child(div().font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(t))
        .when(required, |d| d.child(div().text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))).child("*")))
}

pub fn label(t: SharedString) -> gpui::Div {
    field_label(t, false)
}

pub fn values(f: &Form, cx: &gpui::App) -> Vec<String> {
    let mut v: Vec<String> = f.fields.iter().map(|fl| fl.value(cx)).collect();
    if let Some((_, on)) = &f.check {
        v.push(if *on { "1" } else { "0" }.to_string());
    }
    v
}

fn form(f: &Form, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let mut body = div().flex().flex_col().gap(z(16.));
    if let Some((im, iw, ih)) = &f.preview {
        let (w, h) = crate::media::fit(*iw, *ih, f.width - 80., 150.);
        body = body.child(div().mt(z(8.)).ml(z(16.)).child(gpui::img(im.clone()).w(z(w)).h(z(h)).object_fit(gpui::ObjectFit::Contain)));
    }
    for (n, fl) in f.fields.iter().enumerate() {
        let mut col = div().id(("aform-field", n)).flex().flex_col().child(label(fl.label.clone()));
        if let Some(a) = &fl.area {
            col = col.child(area_box(a, fl.rows));
        } else if let Some(i) = &fl.input {
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
    card(f.width, f.title.clone(), f.subtitle.clone(), body.into_any_element(), Some(footer.into_any_element()), cx)
}

#[cfg(test)]
mod tests {
    #[test]
    fn strong_markers_in_subtitles() {
        let (t, r) = super::strong_ranges("Ce lien essaie d'ouvrir **une application sur ton appareil**");
        assert_eq!(t, "Ce lien essaie d'ouvrir une application sur ton appareil");
        assert_eq!(r, vec![24..t.len()]);
        assert_eq!(super::strong_ranges("a ** b"), ("a ** b".to_owned(), vec![]));
        assert_eq!(super::strong_ranges("plain"), ("plain".to_owned(), vec![]));
    }

    use super::clamp_insert;

    #[test]
    fn max_length_truncates_the_inserted_text_like_a_textarea() {
        assert_eq!(clamp_insert("abc", "abcd", 4), None);
        assert_eq!(clamp_insert("abc", "abcdef", 4), Some(("abcd".into(), 4)));
        assert_eq!(clamp_insert("abef", "abXYZef", 5), Some(("abXef".into(), 3)));
        assert_eq!(clamp_insert("ab", "ab😀", 3), Some(("ab".into(), 2)));
        assert_eq!(clamp_insert("", "é😀x", 3), Some(("é😀".into(), "é😀".len())));
        assert_eq!(clamp_insert("aaaa", "aaaaa", 4), Some(("aaaa".into(), 4)));
    }
}

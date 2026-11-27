use crate::ui::tooltip::Tipped as _;
use std::cell::Cell;
use std::rc::Rc;

use gpui::{AnyElement, App, Context, FontWeight, Hsla, IntoElement, ParentElement, Pixels, Point, SharedString, Styled, Window, div, prelude::*};
use phoenix_api::rest::stage::{PRIVACY_GUILD_ONLY, StageCall};

use crate::actions::{ADMINISTRATOR, ActionRequest, MANAGE_CHANNELS, MOVE_MEMBERS, MUTE_MEMBERS};
use crate::app::PhoenixApp;
use crate::discord_backend::convert::sf;
use crate::model::{Channel, ChannelKind, Id, VoiceFlags};
use crate::theme::{generated::idx, z};
use crate::ui::call_view::{FULLSCREEN, Look, POPOUT, center_button, center_button_ex, glyph, named, section, t, tray_icon};
use crate::ui::context_menu::{self, MenuItem};

pub const REQUEST_TO_SPEAK: u64 = 1 << 32;
pub const STAGE_MODERATOR: u64 = MANAGE_CHANNELS | MUTE_MEMBERS | MOVE_MEMBERS;
pub const SIDEBAR_W: f32 = 320.;
pub const TOPIC_MAX: usize = 120;

thread_local! {
    static SIDEBAR: Cell<Option<Id>> = const { Cell::new(None) };
    static START_SKIPPED: Cell<Option<Id>> = const { Cell::new(None) };
    static LEAVE_MENU: Cell<Option<Point<Pixels>>> = const { Cell::new(None) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rts {
    None,
    Requested,
    Awaiting,
    OnStage,
}

pub fn rts(f: VoiceFlags) -> Rts {
    match (f.suppress, f.hand.is_some()) {
        (true, true) => Rts::Requested,
        (false, true) => Rts::Awaiting,
        (false, false) => Rts::OnStage,
        (true, false) => Rts::None,
    }
}

fn flags(u: Id) -> VoiceFlags {
    crate::ui::voice_panel::flags(u)
}

pub fn stage_channel(app: &PhoenixApp, ch: Id) -> Option<&Channel> {
    app.channels.get(&ch).filter(|c| c.kind == ChannelKind::Stage)
}

fn perms(app: &PhoenixApp, ch: &Channel) -> u64 {
    app.backend.channel_permissions(ch.id).unwrap_or_else(|| ch.guild.map_or(0, |g| app.backend.guild_permissions(g)))
}

pub fn has_all(perms: u64, bits: u64) -> bool {
    perms & ADMINISTRATOR != 0 || perms & bits == bits
}

pub fn can_mute(app: &PhoenixApp, ch: &Channel) -> bool {
    has_all(perms(app, ch), MUTE_MEMBERS)
}

pub fn is_moderator(app: &PhoenixApp, ch: &Channel) -> bool {
    has_all(perms(app, ch), STAGE_MODERATOR)
}

fn can_request(app: &PhoenixApp, ch: &Channel) -> bool {
    has_all(perms(app, ch), REQUEST_TO_SPEAK)
}

pub fn speakers(ch: &Channel) -> Vec<Id> {
    ch.voice_users.iter().copied().filter(|u| rts(flags(*u)) == Rts::OnStage).collect()
}

pub fn audience(app: &PhoenixApp, ch: &Channel) -> Vec<Id> {
    let mut v: Vec<(String, u64)> = ch
        .voice_users
        .iter()
        .copied()
        .filter(|u| rts(flags(*u)) != Rts::OnStage)
        .map(|u| (crate::ui::call_view::name_key(&crate::ui::sidebar::voice_user_name(app, u), u), u.0))
        .collect();
    v.sort();
    v.into_iter().map(|(_, u)| Id(u)).collect()
}

pub fn request_order(users: &[(Id, VoiceFlags)]) -> Vec<Id> {
    let mut v: Vec<(i64, u64)> = users
        .iter()
        .filter(|(_, f)| matches!(rts(*f), Rts::Requested | Rts::Awaiting))
        .map(|(u, f)| (f.hand.unwrap_or_default(), u.0))
        .collect();
    v.sort();
    v.into_iter().map(|(_, u)| Id(u)).collect()
}

pub fn requests(ch: &Channel) -> Vec<Id> {
    let users: Vec<(Id, VoiceFlags)> = ch.voice_users.iter().map(|u| (*u, flags(*u))).collect();
    request_order(&users)
}

pub fn pending(ch: &Channel) -> usize {
    ch.voice_users.iter().filter(|u| rts(flags(**u)) == Rts::Requested).count()
}

pub fn live(ch: &Channel) -> bool {
    ch.guild.is_some_and(|g| crate::guild_live::get(g.0).stages.values().any(|c| *c == ch.id.0))
}

pub fn topic(ch: &Channel) -> Option<String> {
    crate::guild_live::stage_topic(ch.guild?.0, ch.id.0).filter(|t| !t.is_empty())
}

pub fn tile_users(ch: &Channel) -> Vec<Id> {
    if ch.kind == ChannelKind::Stage { speakers(ch) } else { ch.voice_users.clone() }
}

pub fn show_start(app: &PhoenixApp, ch: &Channel) -> bool {
    app.voice_channel == Some(ch.id) && is_moderator(app, ch) && !live(ch) && START_SKIPPED.with(|s| s.get()) != Some(ch.id)
}

pub fn sidebar_open(ch: Id) -> bool {
    SIDEBAR.with(|s| s.get()) == Some(ch)
}

fn toggle_sidebar(ch: Id) {
    SIDEBAR.with(|s| s.set(if s.get() == Some(ch) { None } else { Some(ch) }));
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn guild_of(app: &PhoenixApp, ch: Id) -> Option<(Id, Id)> {
    stage_channel(app, ch).and_then(|c| c.guild.map(|g| (g, c.id)))
}

impl PhoenixApp {
    pub fn stage_call(&mut self, call: StageCall, cx: &mut Context<Self>) {
        self.action_toast(ActionRequest::Stage(call), None, cx);
    }

    pub fn stage_raise_hand(&mut self, ch: Id, raise: bool, cx: &mut Context<Self>) {
        let Some((g, c)) = guild_of(self, ch) else { return };
        self.stage_call(StageCall::RequestToSpeak { guild: sf(g), channel: sf(c), at: raise.then(now_iso) }, cx);
    }

    pub fn stage_set_self_suppress(&mut self, ch: Id, suppress: bool, cx: &mut Context<Self>) {
        let Some((g, c)) = guild_of(self, ch) else { return };
        let call = if suppress {
            StageCall::MoveSelfToAudience { guild: sf(g), channel: sf(c) }
        } else {
            StageCall::SetSelfSuppress { guild: sf(g), channel: sf(c), suppress: false, silent: false }
        };
        self.stage_call(call, cx);
    }

    pub fn stage_set_user_suppress(&mut self, ch: Id, user: Id, suppress: bool, cx: &mut Context<Self>) {
        let Some((g, c)) = guild_of(self, ch) else { return };
        self.stage_call(StageCall::SetUserSuppress { guild: sf(g), channel: sf(c), user: sf(user), suppress }, cx);
    }

    pub fn stage_invite_to_speak(&mut self, ch: Id, user: Id, cx: &mut Context<Self>) {
        let Some((g, c)) = guild_of(self, ch) else { return };
        self.stage_call(StageCall::InviteToSpeak { guild: sf(g), channel: sf(c), user: sf(user), at: now_iso() }, cx);
    }

    pub fn stage_move_to_audience(&mut self, ch: Id, user: Id, cx: &mut Context<Self>) {
        let Some((g, c)) = guild_of(self, ch) else { return };
        self.stage_call(StageCall::MoveUserToAudience { guild: sf(g), channel: sf(c), user: sf(user) }, cx);
    }

    pub fn stage_end(&mut self, ch: Id, cx: &mut Context<Self>) {
        self.stage_call(StageCall::End { channel: sf(ch) }, cx);
    }

    pub fn open_stage_topic(&mut self, ch: Id, cx: &mut Context<Self>) {
        let Some(chan) = stage_channel(self, ch) else { return };
        let editing = live(chan);
        let current = topic(chan).unwrap_or_default();
        let guild = chan.guild;
        let (title, submit) = if editing {
            (crate::t!("stage.edit"), crate::t!("common.save"))
        } else {
            (crate::t!("stage.start"), crate::t!("stage.start"))
        };
        let field = crate::ui::modals::Field::new(crate::t!("stage.topic_label"), crate::t!("stage.topic_placeholder"), current).max_len(TOPIC_MAX);
        crate::actions::open_modal(
            crate::ui::modals::Modal::Form(crate::ui::modals::Form {
                title,
                subtitle: (!editing).then(|| crate::t!("stage.start_body")),
                fields: vec![field],
                choices: Vec::new(),
                choice: None,
                check: None,
                submit,
                danger: false,
                preview: None,
                width: 440.,
                on_submit: Rc::new(move |a, v, _, cx| {
                    let topic = v.into_iter().next().unwrap_or_default().trim().to_owned();
                    if topic.is_empty() {
                        a.parity_toast(crate::t!("stage.topic_required"), cx);
                        return;
                    }
                    if editing {
                        a.stage_call(StageCall::Edit { channel: sf(ch), topic, privacy_level: PRIVACY_GUILD_ONLY }, cx);
                        return;
                    }
                    let start = StageCall::Start { channel: sf(ch), topic, privacy_level: PRIVACY_GUILD_ONLY, send_start_notification: false };
                    a.action(ActionRequest::Stage(start), cx, move |a, r, cx| match r {
                        Ok(_) => {
                            if let Some(g) = guild {
                                a.stage_call(StageCall::SetSelfSuppress { guild: sf(g), channel: sf(ch), suppress: false, silent: true }, cx);
                            }
                        }
                        Err(e) => a.parity_toast(e.0, cx),
                    });
                }),
            }),
            None,
        );
        cx.notify();
    }
}

fn text(size: f32, line: f32, weight: FontWeight, color: Hsla, s: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(color).child(s.into())
}

fn speaking_label(n: usize) -> SharedString {
    if n == 0 { crate::t!("stage.speaking_zero", n = n) } else { crate::t!("stage.speaking_other", n = n) }
}

pub fn header_summary(app: &PhoenixApp, ch: Id) -> Option<AnyElement> {
    let chan = stage_channel(app, ch)?;
    let muted = t(idx::TEXT_MUTED);
    let speaking = speakers(chan).len();
    let listening = chan.voice_users.len() - speaking;
    Some(
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(z(4.))
            .child(named("mic", 16., muted))
            .child(text(14., 18., FontWeight::NORMAL, muted, speaking_label(speaking)))
            .child(named("members", 16., muted))
            .child(text(14., 18., FontWeight::NORMAL, muted, crate::t!("stage.in_audience", n = listening)))
            .into_any_element(),
    )
}

pub fn header_topic(app: &PhoenixApp, ch: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let chan = stage_channel(app, ch)?;
    let topic = topic(chan)?;
    let edit = is_moderator(app, chan);
    Some(
        div()
            .id("stage-topic")
            .flex_none()
            .max_w(z(320.))
            .min_w_0()
            .text_size(z(14.))
            .line_height(z(18.))
            .text_color(t(idx::TEXT_DEFAULT))
            .text_ellipsis()
            .child(SharedString::from(topic))
            .when(edit, |d| {
                d.cursor_pointer().tip(crate::t!("stage.edit")).on_click(cx.listener(move |a, _, _, cx| a.open_stage_topic(ch, cx)))
            })
            .into_any_element(),
    )
}

pub fn audience_toggle(app: &PhoenixApp, ch: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let chan = stage_channel(app, ch)?;
    let open = sidebar_open(ch);
    let count = if can_mute(app, chan) { pending(chan) } else { 0 };
    let label = if open {
        crate::t!("stage.hide_audience")
    } else if count == 1 {
        crate::t!("stage.show_audience_pending_one", n = count)
    } else if count > 1 {
        crate::t!("stage.show_audience_pending_other", n = count)
    } else {
        crate::t!("stage.show_audience")
    };
    let color = t(if open { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT });
    let badge = (count > 0).then(|| {
        div()
            .absolute()
            .bottom(z(-4.))
            .left(z(12.))
            .h(z(16.))
            .min_w(z(16.))
            .px(z(4.))
            .rounded_full()
            .bg(t(idx::BACKGROUND_FEEDBACK_NOTIFICATION))
            .flex()
            .items_center()
            .justify_center()
            .child(text(12., 16., FontWeight::SEMIBOLD, t(idx::WHITE), if count > 99 { "99+".to_owned() } else { count.to_string() }))
    });
    Some(
        div()
            .id("stage-audience-toggle")
            .relative()
            .size(z(24.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .tip(label)
            .child(named("members", 24., color))
            .children(badge)
            .on_click(cx.listener(move |_, _, _, cx| {
                toggle_sidebar(ch);
                cx.notify();
            }))
            .into_any_element(),
    )
}

fn icon_button(id: (&'static str, u64), icon: &str, tip: SharedString, enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(z(32.))
        .flex_none()
        .rounded(z(8.))
        .flex()
        .items_center()
        .justify_center()
        .bg(t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
        .when(enabled, |d| d.cursor_pointer().hover(|s| s.bg(t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
        .when(!enabled, |d| d.opacity(0.5))
        .tip(tip)
        .child(named(icon, 16., t(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
}

fn member_row(app: &PhoenixApp, ch: Id, user: Id, actions: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = crate::ui::sidebar::voice_user_name(app, user);
    let awaiting = rts(flags(user)) == Rts::Awaiting;
    let row = div()
        .h(z(40.))
        .flex()
        .items_center()
        .gap(z(8.))
        .child(
            div()
                .id(("stage-member", user.0))
                .flex_1()
                .min_w_0()
                .h_full()
                .ml(z(-8.))
                .px(z(8.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE)))
                .child(div().flex_none().child(crate::ui::avatar(app.users.get(&user), 32. * crate::theme::zoom())))
                .child(text(14., 18., FontWeight::MEDIUM, t(idx::TEXT_STRONG), name).flex_1().min_w_0().text_ellipsis()),
        );
    if !actions {
        return row.into_any_element();
    }
    let invite_tip = if awaiting { crate::t!("stage.invite_sent") } else { crate::t!("stage.invite_to_speak") };
    row.child(
        div()
            .flex()
            .gap(z(8.))
            .child(icon_button(("stage-invite", user.0), "sys/mic-arrow-right", invite_tip, !awaiting).when(!awaiting, |d| {
                d.on_click(cx.listener(move |a, _, _, cx| a.stage_set_user_suppress(ch, user, false, cx)))
            }))
            .child(
                icon_button(("stage-dismiss", user.0), "close-small", crate::t!("stage.dismiss"), true)
                    .on_click(cx.listener(move |a, _, _, cx| a.stage_set_user_suppress(ch, user, true, cx))),
            ),
    )
    .into_any_element()
}

fn section_header(label: SharedString) -> gpui::Div {
    div().h(z(40.)).pt(z(16.)).child(text(12., 16., FontWeight::SEMIBOLD, t(idx::TEXT_MUTED), label))
}

pub fn sidebar(app: &PhoenixApp, ch: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let chan = stage_channel(app, ch)?;
    if !sidebar_open(ch) {
        return None;
    }
    let mods = can_mute(app, chan);
    let reqs = requests(chan);
    let crowd = audience(app, chan);
    let mut list = div().id("stage-audience-list").flex_1().min_h_0().px(z(16.)).overflow_y_scroll().flex().flex_col();
    if !reqs.is_empty() {
        list = list.child(section_header(crate::t!("stage.requests_count", n = reqs.len())));
        for &u in &reqs {
            list = list.child(member_row(app, ch, u, mods, cx));
        }
    }
    list = list.child(section_header(crate::t!("stage.audience_count", n = crowd.len())));
    if crowd.is_empty() {
        list = list.child(
            div()
                .h(z(88.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_center()
                .child(text(20., 24., FontWeight::SEMIBOLD, t(idx::TEXT_STRONG), crate::t!("stage.no_listeners")))
                .child(text(14., 18., FontWeight::NORMAL, t(idx::TEXT_DEFAULT), crate::t!("stage.no_listeners_body")).mt(z(8.))),
        );
    }
    for &u in &crowd {
        list = list.child(member_row(app, ch, u, false, cx));
    }
    let header = div()
        .h(z(48.))
        .flex_none()
        .px(z(16.))
        .flex()
        .items_center()
        .gap(z(8.))
        .border_b_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .child(named("members", 24., t(idx::ICON_MUTED)))
        .child(text(16., 20., FontWeight::SEMIBOLD, t(idx::TEXT_STRONG), crate::t!("stage.audience")).flex_1())
        .child(
            div()
                .id("stage-audience-close")
                .size(z(24.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .tip(crate::t!("common.close"))
                .child(named("close-small", 24., t(idx::INTERACTIVE_TEXT_DEFAULT)))
                .on_click(cx.listener(move |_, _, _, cx| {
                    toggle_sidebar(ch);
                    cx.notify();
                })),
        );
    Some(
        div()
            .w(z(SIDEBAR_W))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(t(idx::BACKGROUND_BASE_LOW))
            .rounded_l(z(8.))
            .overflow_hidden()
            .child(header)
            .child(list)
            .into_any_element(),
    )
}

pub fn empty_state() -> AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(text(20., 24., FontWeight::MEDIUM, t(idx::TEXT_STRONG), crate::t!("stage.nobody_on_stage")))
        .into_any_element()
}

fn waiting_line(names: &[SharedString]) -> SharedString {
    match names {
        [] => SharedString::default(),
        [a] => crate::t!("stage.waiting_one", first = a),
        [a, b] => crate::t!("stage.waiting_two", first = a, second = b),
        [a, b, rest @ ..] if rest.len() == 1 => crate::t!("stage.waiting_more_one", first = a, second = b, n = rest.len()),
        [a, b, rest @ ..] => crate::t!("stage.waiting_more_other", first = a, second = b, n = rest.len()),
    }
}

fn prompt_card(id: &'static str, icon: &str, title: SharedString, body: Option<SharedString>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w(z(400.))
        .mt(z(8.))
        .p(z(16.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(16.))
        .bg(t(idx::BACKGROUND_MOD_SUBTLE))
        .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
        .cursor_pointer()
        .child(
            div()
                .size(z(40.))
                .flex_none()
                .rounded_full()
                .bg(t(idx::BACKGROUND_BASE_LOW))
                .flex()
                .items_center()
                .justify_center()
                .child(named(icon, 20., t(idx::INTERACTIVE_TEXT_ACTIVE))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(text(16., 20., FontWeight::SEMIBOLD, t(idx::TEXT_STRONG), title))
                .children(body.map(|b| text(14., 18., FontWeight::NORMAL, t(idx::TEXT_DEFAULT), b))),
        )
}

pub fn start_screen(app: &PhoenixApp, ch: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let chan = stage_channel(app, ch)?;
    let names: Vec<SharedString> = chan.voice_users.iter().filter(|u| **u != app.me.id).map(|u| crate::ui::sidebar::voice_user_name(app, *u)).collect();
    let waiting = waiting_line(&names);
    Some(
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .mt(z(24.))
                    .mb(z(24.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .child(text(32., 40., FontWeight::MEDIUM, t(idx::TEXT_STRONG), crate::t!("stage.waiting_title")))
                    .child(text(20., 24., FontWeight::MEDIUM, t(idx::TEXT_DEFAULT), crate::t!("stage.waiting_sub")).mt(z(8.))),
            )
            .child(prompt_card("stage-start", "stage", crate::t!("stage.start"), None).on_click(cx.listener(move |a, _, _, cx| a.open_stage_topic(ch, cx))))
            .child(
                prompt_card("stage-continue", "mic", crate::t!("stage.continue_without"), Some(crate::t!("stage.continue_without_body"))).on_click(cx.listener(
                    move |_, _, _, cx| {
                        START_SKIPPED.with(|s| s.set(Some(ch)));
                        cx.notify();
                    },
                )),
            )
            .when(!names.is_empty(), |d| {
                d.child(
                    div()
                        .mt(z(24.))
                        .max_w(z(400.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(z(8.))
                        .children(chan.voice_users.iter().filter(|u| **u != app.me.id).take(3).map(|u| crate::ui::avatar(app.users.get(u), 16. * crate::theme::zoom())))
                        .child(text(14., 18., FontWeight::NORMAL, t(idx::TEXT_DEFAULT), waiting)),
                )
            })
            .into_any_element(),
    )
}

fn mic_button(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let muted = app.mic_muted || app.deafened;
    let (icon, look, color) = if muted {
        (crate::assets::Icon::MicOff, Look::Red, t(idx::RED_400))
    } else {
        (crate::assets::Icon::Mic, Look::Dark, t(idx::WHITE))
    };
    center_button_ex(
        "stage-mic",
        crate::ui::icon(icon, 20. * crate::theme::zoom(), color),
        look,
        true,
        Some(Rc::new(|_: &mut PhoenixApp, pos: Point<Pixels>, cx: &mut Context<PhoenixApp>| {
            crate::ui::voice_panel::toggle_call_audio_menu(pos);
            cx.notify();
        })),
        crate::ui::voice_panel::audio_menu_open(crate::ui::voice_panel::Audio::Mic),
        if muted { crate::t!("voice.unmute") } else { crate::t!("voice.mute") },
        |a, cx| {
            a.mic_muted = !a.mic_muted;
            crate::voice::sync_self_state(a);
            cx.notify();
        },
        cx,
    )
}

fn hand_button(app: &PhoenixApp, chan: &Channel, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let raised = flags(app.me.id).hand.is_some();
    let allowed = can_request(app, chan);
    let enabled = raised || allowed;
    let label = if raised {
        crate::t!("stage.cancel_request")
    } else if allowed {
        crate::t!("stage.request_to_speak")
    } else {
        crate::t!("stage.raise_disabled")
    };
    let color = t(if raised { idx::GREEN_300 } else { idx::WHITE });
    let ch = chan.id;
    let btn = center_button(
        "stage-hand",
        named("sys/hand-request-speak", 20., color),
        if raised { Look::Green } else { Look::Dark },
        false,
        label,
        move |a, cx| {
            if enabled {
                a.stage_raise_hand(ch, !raised, cx);
            }
        },
        cx,
    );
    if enabled { btn } else { div().opacity(0.5).child(btn).into_any_element() }
}

fn more_button(cx: &mut Context<PhoenixApp>) -> AnyElement {
    let open = crate::ui::call_menus::opened() == Some(crate::ui::call_menus::TrayMenu::More);
    div()
        .relative()
        .child(center_button_ex(
            "stage-more",
            named("more-horizontal", 20., t(idx::WHITE)),
            Look::Dark,
            false,
            None,
            open,
            crate::t!("call.more"),
            |_, cx| {
                crate::ui::call_menus::toggle(crate::ui::call_menus::TrayMenu::More);
                cx.notify();
            },
            cx,
        ))
        .child(crate::ui::call_menus::probe(crate::ui::call_menus::TrayMenu::More))
        .into_any_element()
}

fn leave_button(app: &PhoenixApp, chan: &Channel, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let split = live(chan) && is_moderator(app, chan);
    let bg = t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_DEFAULT);
    let hover = t(idx::CONTROL_CRITICAL_PRIMARY_BACKGROUND_HOVER);
    let fg = t(idx::CONTROL_CRITICAL_PRIMARY_TEXT_DEFAULT);
    let main = div()
        .id("stage-leave")
        .w(z(65.))
        .h(z(49.))
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .hover(|s| s.bg(hover))
        .cursor_pointer()
        .when(split, |d| d.rounded_l(z(12.)))
        .when(!split, |d| d.rounded(z(12.)))
        .tip(crate::t!("stage.disconnect_quietly"))
        .child(crate::ui::icon(crate::assets::Icon::PhoneHangup, 20. * crate::theme::zoom(), fg))
        .on_click(cx.listener(|a, _, _, cx| a.join_voice(None, cx)));
    if !split {
        return main.into_any_element();
    }
    div()
        .flex()
        .items_center()
        .child(main)
        .child(
            div()
                .id("stage-leave-caret")
                .w(z(24.))
                .h(z(49.))
                .ml(z(1.))
                .rounded_r(z(12.))
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .hover(|s| s.bg(hover))
                .cursor_pointer()
                .child(crate::ui::icon(crate::assets::Icon::Chevron, 16. * crate::theme::zoom(), fg))
                .on_click(cx.listener(|_, ev: &gpui::ClickEvent, _, cx| {
                    LEAVE_MENU.with(|m| m.set(if m.get().is_some() { None } else { Some(ev.position()) }));
                    cx.notify();
                })),
        )
        .into_any_element()
}

pub fn leave_menu(ch: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let pos = LEAVE_MENU.with(|m| m.get())?;
    let e = cx.entity();
    let end = MenuItem::danger(crate::t!("stage.end_for_everyone"), Some(crate::icons::path("settings/circle-x")), move |_, w: &mut Window, cx: &mut App| {
        LEAVE_MENU.with(|m| m.set(None));
        e.update(cx, |a, cx| a.stage_end(ch, cx));
        w.refresh();
    });
    let dismiss: context_menu::Dismiss = Rc::new(|w: &mut Window, _: &mut App| {
        LEAVE_MENU.with(|m| m.set(None));
        w.refresh();
    });
    Some(context_menu::render_anchored(pos, gpui::Anchor::BottomLeft, vec![end], dismiss))
}

pub fn tray(app: &PhoenixApp, chan: &Channel, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let ch = chan.id;
    let me = flags(app.me.id);
    let connected = app.voice_channel == Some(ch);
    let state = rts(me);
    let mute = can_mute(app, chan);
    let row = div().flex().items_center().gap(z(12.));
    if show_start(app, chan) {
        return row.child(leave_button(app, chan, cx)).into_any_element();
    }
    let mut buttons = section();
    if connected {
        buttons = buttons.child(hand_button(app, chan, cx));
    }
    let row = if state == Rts::OnStage || state == Rts::Awaiting || mute {
        if connected && ((mute && me.suppress) || state == Rts::Awaiting) {
            buttons = buttons.child(center_button(
                "stage-speak",
                named("sys/mic-arrow-right", 20., t(idx::WHITE)),
                Look::Dark,
                false,
                crate::t!("stage.speak_on_stage"),
                move |a, cx| a.stage_set_self_suppress(ch, false, cx),
                cx,
            ));
        }
        if connected && !me.suppress {
            buttons = buttons.child(center_button(
                "stage-audience",
                named("stage/group-arrow-down", 20., t(idx::WHITE)),
                Look::Dark,
                false,
                crate::t!("stage.leave_stage"),
                move |a, cx| a.stage_set_self_suppress(ch, true, cx),
                cx,
            ));
        }
        row.when(connected && !me.suppress, |r| r.child(section().child(mic_button(app, cx))))
    } else {
        row
    };
    row.child(buttons.child(more_button(cx))).child(leave_button(app, chan, cx)).into_any_element()
}

pub fn bottom(app: &PhoenixApp, ch: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let chan = stage_channel(app, ch)?;
    let tray_color = t(idx::INTERACTIVE_TEXT_DEFAULT);
    Some(
        div()
            .h(z(49.))
            .mt(z(19.))
            .mb(z(20.))
            .px(z(20.))
            .flex_none()
            .flex()
            .items_center()
            .gap(z(24.))
            .child(div().flex_1())
            .child(div().flex_none().flex().justify_center().child(tray(app, chan, cx)))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_end()
                    .gap(z(24.))
                    .child(tray_icon("stage-popout", glyph(POPOUT, 24., tray_color), crate::t!("call.popout")).on_click(cx.listener(|_, _, _, cx| {
                        let app_entity = cx.entity();
                        cx.defer(move |cx| crate::ui::call_popout::open(app_entity, cx));
                        cx.notify();
                    })))
                    .child(tray_icon("stage-fullscreen", glyph(FULLSCREEN, 24., tray_color), crate::t!("call.fullscreen")).on_click(cx.listener(|_, _, w, _| w.toggle_fullscreen()))),
            )
            .into_any_element(),
    )
}

pub fn user_items(app: &PhoenixApp, ch: Id, user: Id, e: &gpui::Entity<PhoenixApp>) -> Vec<MenuItem> {
    let Some(chan) = stage_channel(app, ch) else { return Vec::new() };
    if user == app.me.id || !can_mute(app, chan) {
        return Vec::new();
    }
    let e = e.clone();
    if rts(flags(user)) == Rts::OnStage {
        vec![MenuItem::action(crate::t!("stage.move_to_audience"), None, move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| a.stage_move_to_audience(ch, user, cx))
        })]
    } else {
        vec![MenuItem::action(crate::t!("stage.invite_to_speak_menu"), None, move |_, _, cx: &mut App| {
            e.update(cx, |a, cx| a.stage_invite_to_speak(ch, user, cx))
        })]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(suppress: bool, hand: Option<i64>) -> VoiceFlags {
        VoiceFlags { suppress, hand, ..Default::default() }
    }

    #[test]
    fn request_to_speak_state_follows_the_client() {
        assert_eq!(rts(f(true, Some(1))), Rts::Requested);
        assert_eq!(rts(f(false, Some(1))), Rts::Awaiting);
        assert_eq!(rts(f(false, None)), Rts::OnStage);
        assert_eq!(rts(f(true, None)), Rts::None);
    }

    #[test]
    fn requests_sort_by_raise_time_then_id() {
        let users = [(Id(9), f(true, Some(20))), (Id(3), f(true, Some(10))), (Id(4), f(false, None)), (Id(2), f(false, Some(20))), (Id(5), f(true, None))];
        assert_eq!(request_order(&users), vec![Id(3), Id(2), Id(9)]);
    }

    #[test]
    fn stage_moderator_needs_all_three_permissions() {
        assert!(has_all(STAGE_MODERATOR, STAGE_MODERATOR));
        assert!(!has_all(MANAGE_CHANNELS | MUTE_MEMBERS, STAGE_MODERATOR));
        assert!(has_all(ADMINISTRATOR, STAGE_MODERATOR));
        assert!(!has_all(0, REQUEST_TO_SPEAK));
    }

    #[test]
    fn waiting_summary_lists_two_names_then_counts_the_rest() {
        assert!(waiting_line(&[]).is_empty());
        let names: Vec<SharedString> = ["A", "B", "C", "D"].into_iter().map(SharedString::from).collect();
        assert!(waiting_line(&names[..1]).contains('A'));
        let two = waiting_line(&names[..2]);
        assert!(two.contains('A') && two.contains('B'));
        assert!(waiting_line(&names).contains('2'));
    }
}

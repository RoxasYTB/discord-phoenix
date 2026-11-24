//! UI state of the parity screens (inbox, forum, interaction modals, poll
//! choices, moderation dialogs, group DM picker) and the small hooks the
//! shared app code calls.

use std::collections::{HashMap, HashSet};

use gpui::{Context, Entity, SharedString};
use gpui_component::input::InputState;
use phoenix_api::gateway::GatewayEvent;
use serde_json::Value;

use super::{ForumPost, ParityReply, ParityRequest, store};
use crate::{
    app::PhoenixApp as App,
    model::{Id, Message},
};

/// Gateway events for the parity screens.
#[derive(Clone, Debug)]
pub enum ParityEvent {
    /// `INTERACTION_MODAL_CREATE`: a bot opened a modal for us.
    Modal(ModalSpec),
    /// `INTERACTION_FAILURE`: the bot didn't answer in time.
    InteractionFailed,
    /// `MESSAGE_POLL_VOTE_ADD/REMOVE`.
    PollVote { channel: Id, message: Id, user: Id, answer: u32, add: bool },
    /// `CALL_CREATE` / `CALL_UPDATE` (`ringing` user ids) and `CALL_DELETE` (empty).
    Call { channel: Id, ringing: Vec<Id> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextInputSpec {
    pub custom_id: String,
    pub label: String,
    pub description: Option<String>,
    /// 1 = short, 2 = paragraph.
    pub style: u8,
    pub placeholder: Option<String>,
    pub value: Option<String>,
    pub required: bool,
    pub min: Option<u32>,
    pub max: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModalSpec {
    pub id: Id,
    pub custom_id: String,
    pub title: String,
    pub app: Id,
    pub app_name: Option<String>,
    pub channel: Option<Id>,
    pub guild: Option<Id>,
    pub inputs: Vec<TextInputSpec>,
}

fn sf(v: Option<&Value>) -> Option<Id> {
    match v? {
        Value::String(s) => s.parse().ok().map(Id),
        Value::Number(n) => n.as_u64().map(Id),
        _ => None,
    }
}

fn text_input(v: &Value, label: Option<&Value>, description: Option<&Value>) -> Option<TextInputSpec> {
    if v.get("type").and_then(Value::as_u64) != Some(4) {
        return None;
    }
    let st = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    Some(TextInputSpec {
        custom_id: st("custom_id").unwrap_or_default(),
        label: label.or(v.get("label")).and_then(Value::as_str).unwrap_or_default().to_owned(),
        description: description.and_then(Value::as_str).map(str::to_owned),
        style: v.get("style").and_then(Value::as_u64).unwrap_or(1) as u8,
        placeholder: st("placeholder"),
        value: st("value"),
        required: v.get("required").and_then(Value::as_bool).unwrap_or(true),
        min: v.get("min_length").and_then(Value::as_u64).map(|n| n as u32),
        max: v.get("max_length").and_then(Value::as_u64).map(|n| n as u32),
    })
}

impl ModalSpec {
    pub fn from_value(v: &Value) -> Option<Self> {
        let mut inputs = Vec::new();
        for row in v.get("components").and_then(Value::as_array)? {
            match row.get("type").and_then(Value::as_u64) {
                // Legacy: action row wrapping one text input.
                Some(1) => inputs.extend(
                    row.get("components").and_then(Value::as_array).into_iter().flatten().filter_map(|c| text_input(c, None, None)),
                ),
                // Label wrapping a text input (newer modals).
                Some(18) => inputs.extend(
                    row.get("component").and_then(|c| text_input(c, row.get("label"), row.get("description"))),
                ),
                _ => {}
            }
        }
        let app = v.get("application");
        Some(Self {
            id: sf(v.get("id"))?,
            custom_id: v.get("custom_id").and_then(Value::as_str)?.to_owned(),
            title: v.get("title").and_then(Value::as_str).unwrap_or_default().to_owned(),
            app: sf(app.and_then(|a| a.get("id"))).or_else(|| sf(v.get("application_id")))?,
            app_name: app.and_then(|a| a.get("name")).and_then(Value::as_str).map(str::to_owned),
            channel: sf(v.get("channel_id")),
            guild: sf(v.get("guild_id")),
            inputs,
        })
    }
}

/// Map a raw gateway event to a [`ParityEvent`].
pub fn from_gateway(ev: &GatewayEvent) -> Option<ParityEvent> {
    let GatewayEvent::Unknown { name, data } = ev else { return None };
    match name.as_str() {
        "INTERACTION_MODAL_CREATE" => ModalSpec::from_value(data).map(ParityEvent::Modal),
        "INTERACTION_FAILURE" => Some(ParityEvent::InteractionFailed),
        "CALL_CREATE" | "CALL_UPDATE" | "CALL_DELETE" => Some(ParityEvent::Call {
            channel: sf(data.get("channel_id"))?,
            ringing: if name == "CALL_DELETE" {
                Vec::new()
            } else {
                data.get("ringing").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| sf(Some(v))).collect()).unwrap_or_default()
            },
        }),
        "MESSAGE_POLL_VOTE_ADD" | "MESSAGE_POLL_VOTE_REMOVE" => Some(ParityEvent::PollVote {
            channel: sf(data.get("channel_id"))?,
            message: sf(data.get("message_id"))?,
            user: sf(data.get("user_id"))?,
            answer: data.get("answer_id").and_then(Value::as_u64)? as u32,
            add: name.ends_with("ADD"),
        }),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InboxTab {
    #[default]
    Mentions,
    Unreads,
    Saved,
    Reminders,
    Scheduled,
}

/// One channel card of Inbox > Unreads.
pub struct UnreadCard {
    pub entry: crate::ui::inbox_unreads::Entry,
    /// Unread messages shown (oldest first, at most 25).
    pub msgs: Vec<Message>,
    /// More unread messages than shown: "View all" button.
    pub more: bool,
    pub loaded: bool,
    /// The messages request failed (client `hasError`): the card stays, never acked.
    pub error: Option<SharedString>,
}

pub struct InboxState {
    pub open: bool,
    pub tab: InboxTab,
    pub mentions: Vec<Message>,
    pub saved: Vec<Message>,
    /// Inbox > Scheduled (`GET /users/@me/scheduled-messages`), sorted by send time.
    pub scheduled: Vec<Message>,
    pub loading: bool,
    pub reached_end: bool,
    pub filter_menu: bool,
    /// Filters of the Mentions tab.
    pub this_server: bool,
    pub everyone: bool,
    pub roles: bool,
    /// Inbox > Unreads, in the client's order; cards load one at a time.
    pub unreads: Vec<UnreadCard>,
    pub unreads_loading: bool,
    /// Bumped on every rebuild: drops loads of a previous opening.
    pub unreads_gen: u64,
}

impl Default for InboxState {
    fn default() -> Self {
        // Client RecentMentionsStore defaults: all servers, @everyone and roles on.
        Self {
            open: false,
            tab: InboxTab::default(),
            mentions: Vec::new(),
            saved: Vec::new(),
            scheduled: Vec::new(),
            loading: false,
            reached_end: false,
            filter_menu: false,
            this_server: false,
            everyone: true,
            roles: true,
            unreads: Vec::new(),
            unreads_loading: false,
            unreads_gen: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ForumSort {
    #[default]
    Recent,
    Created,
}

pub struct ForumCompose {
    pub title: Entity<InputState>,
    pub body: Entity<gpui_component::input::TextareaState>,
    pub tags: Vec<Id>,
    pub sending: bool,
    pub error: Option<SharedString>,
}

#[derive(Default)]
pub struct ForumView {
    pub posts: Vec<ForumPost>,
    /// "Older posts": archived search pages, loaded after the active list.
    pub archived: Vec<ForumPost>,
    pub archived_more: bool,
    pub archived_loading: bool,
    pub archived_loaded: bool,
    pub has_more: bool,
    pub loading: bool,
    pub loaded: bool,
    /// Last failed load of the active posts; shows the error state with a
    /// retry button.
    pub error: Option<SharedString>,
    /// Consecutive failed loads, drives the automatic retry backoff.
    pub failures: u32,
    pub tags: Vec<Id>,
    pub sort: ForumSort,
    pub sort_menu: bool,
    pub compose: Option<ForumCompose>,
}

pub struct ModalView {
    pub spec: ModalSpec,
    pub inputs: Vec<crate::ui::components::Field>,
    pub sending: bool,
    pub error: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModKind {
    Timeout(u64),
    Kick,
    /// Seconds of message history to delete.
    Ban(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct ModDialog {
    pub guild: Id,
    pub user: Id,
    pub kind: ModKind,
}

pub struct GroupDmPicker {
    pub search: Option<Entity<InputState>>,
    pub picked: Vec<Id>,
    pub sending: bool,
}

#[derive(Default)]
pub struct ParityUi {
    pub inbox: InboxState,
    /// DM / group call ringing us (incoming call popup).
    pub incoming_call: Option<Id>,
    pub threads_open: bool,
    /// Archived threads per channel for the threads browser ("older threads").
    pub archived_threads: HashMap<Id, Vec<ForumPost>>,
    pub forums: HashMap<Id, ForumView>,
    /// "New Post" clicked: build the composer on the next frame.
    pub forum_compose_request: Option<Id>,
    /// Post just created: open it once a `Window` is at hand.
    pub forum_open_thread: Option<crate::model::ThreadRef>,
    /// Bot modal waiting for its inputs to be built (needs a `Window`).
    pub pending_modal: Option<ModalSpec>,
    pub modal: Option<ModalView>,
    /// Open select menu: (message, custom_id).
    pub open_select: Option<(Id, String)>,
    /// Values picked in select menus, by (message, custom_id).
    pub select_values: HashMap<(Id, String), Vec<String>>,
    /// Answers ticked in multi-choice polls before "Vote".
    pub poll_choice: HashMap<Id, HashSet<u32>>,
    /// Polls switched to "Show results" before voting (`showResults` of the client's poll store).
    pub poll_show_results: HashSet<Id>,
    /// Interaction buttons waiting for the bot (spinner).
    pub pending: HashSet<(Id, String)>,
    /// Where the last interaction went (modals without `channel_id`).
    pub last_target: Option<(Option<Id>, Id)>,
    /// Read marker captured when a channel is opened: (channel, last read).
    pub read_mark: Option<(Id, Id)>,
    pub moderation: Option<ModDialog>,
    pub group_dm: Option<GroupDmPicker>,
    pub toast: Option<SharedString>,
}

impl App {
    /// Send a [`ParityRequest`]; `done` runs on the UI thread.
    pub fn parity_request(
        &mut self,
        req: ParityRequest,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<ParityReply, crate::backend::BackendError>, &mut Context<Self>) + 'static,
    ) {
        let fut = self.backend.parity(req);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                done(this, res, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn parity_toast(&mut self, msg: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.parity.toast = Some(msg.into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                this.parity.toast = None;
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn refresh_message(&mut self, channel: Id, msg: Id) {
        self.refresh_row(channel, msg);
    }
}

/// `AppEvent::Parity` handler.
pub fn on_event(app: &mut App, ev: ParityEvent, cx: &mut Context<App>) {
    match ev {
        ParityEvent::Modal(spec) => {
            app.parity.pending.clear();
            app.parity.pending_modal = Some(spec);
        }
        ParityEvent::InteractionFailed => {
            app.parity.pending.clear();
            app.parity_toast(crate::t!("interaction.no_response"), cx);
        }
        ParityEvent::PollVote { channel, message, user, answer, add } => {
            let mine = user == app.me.id;
            // Our own votes were applied optimistically already.
            if !mine && store::update_poll(message, |p| p.apply_vote(answer, add, false)) {
                app.refresh_message(channel, message);
            }
        }
        ParityEvent::Call { channel, ringing } => crate::ui::incoming_call::on_call(app, channel, &ringing),
    }
    cx.notify();
}

/// Called by `select_channel` before the channel is marked read: remember
/// where the "NEW" divider goes.
pub fn remember_read_mark(app: &mut App, channel: Id) {
    let unread = app.channels.get(&channel).is_some_and(|c| c.unread);
    app.parity.read_mark = if unread { app.backend.last_read(channel).map(|m| (channel, m)) } else { None };
}

/// Called after the first page of a channel loaded: put the "NEW" divider
/// above the first message after the read marker that isn't ours.
pub fn apply_read_mark(app: &mut App, channel: Id) {
    let Some((ch, last)) = app.parity.read_mark else { return };
    if ch != channel {
        return;
    }
    app.parity.read_mark = None;
    let first = app.messages.get(&channel).and_then(|e| first_unread(&e.msgs, last, app.me.id));
    if let Some(m) = first
        && app.chat.unread_marker.is_none()
    {
        app.chat.unread_marker = Some((channel, m));
        app.refresh_message(channel, m);
    }
}

/// First unread message after `last` in `msgs` that someone else sent.
pub fn first_unread(msgs: &[Message], last: Id, me: Id) -> Option<Id> {
    msgs.iter().find(|m| m.id > last && m.author != me).map(|m| m.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn modal_both_layouts() {
        let v = json!({"id":"7","custom_id":"form","title":"Report","application":{"id":"9","name":"Bot"},
            "channel_id":"3",
            "components":[
                {"type":1,"components":[{"type":4,"custom_id":"a","label":"Name","style":1}]},
                {"type":18,"label":"Why","description":"Tell us","component":{"type":4,"custom_id":"b","style":2,"required":false,"max_length":400}}
            ]});
        let m = ModalSpec::from_value(&v).unwrap();
        assert_eq!((m.id, m.app, m.channel), (Id(7), Id(9), Some(Id(3))));
        assert_eq!(m.inputs.len(), 2);
        assert_eq!(m.inputs[0].label, "Name");
        assert_eq!(m.inputs[1].label, "Why");
        assert_eq!(m.inputs[1].style, 2);
        assert!(!m.inputs[1].required);
        assert_eq!(m.inputs[1].max, Some(400));
    }

    #[test]
    fn gateway_poll_vote() {
        let ev = GatewayEvent::Unknown {
            name: "MESSAGE_POLL_VOTE_REMOVE".into(),
            data: json!({"channel_id":"1","message_id":"2","user_id":"3","answer_id":4}),
        };
        assert!(matches!(
            from_gateway(&ev),
            Some(ParityEvent::PollVote { message: Id(2), answer: 4, add: false, .. })
        ));
    }

    #[test]
    fn unread_marker_skips_own() {
        let mk = |id, author| Message { id: Id(id), author: Id(author), ..Default::default() };
        let msgs = [mk(1, 5), mk(2, 1), mk(3, 5)];
        assert_eq!(first_unread(&msgs, Id(1), Id(1)), Some(Id(3)));
        assert_eq!(first_unread(&msgs, Id(3), Id(1)), None);
    }
}

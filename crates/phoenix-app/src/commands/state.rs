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

#[derive(Clone, Debug)]
pub enum ParityEvent {
    Modal(ModalSpec),
    InteractionCreate { nonce: u64, id: Id },
    InteractionSuccess { nonce: u64, id: Id },
    InteractionFailed { nonce: Option<u64> },
    Autocomplete { nonce: u64, choices: Vec<(String, Value)> },
    PollVote { channel: Id, message: Id, user: Id, answer: u32, add: bool },
    Call { channel: Id, message: Option<Id>, ringing: Vec<Id>, voice: Option<Vec<Id>>, region: Option<CallRegion> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextInputSpec {
    pub custom_id: String,
    pub label: String,
    pub description: Option<String>,
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
                Some(1) => inputs.extend(
                    row.get("components").and_then(Value::as_array).into_iter().flatten().filter_map(|c| text_input(c, None, None)),
                ),
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

pub fn from_gateway(ev: &GatewayEvent) -> Option<ParityEvent> {
    let GatewayEvent::Unknown { name, data } = ev else { return None };
    match name.as_str() {
        "INTERACTION_MODAL_CREATE" => ModalSpec::from_value(data).map(ParityEvent::Modal),
        "INTERACTION_CREATE" => Some(ParityEvent::InteractionCreate { nonce: sf(data.get("nonce"))?.0, id: sf(data.get("id"))? }),
        "INTERACTION_SUCCESS" => Some(ParityEvent::InteractionSuccess { nonce: sf(data.get("nonce"))?.0, id: sf(data.get("id"))? }),
        "INTERACTION_FAILURE" => Some(ParityEvent::InteractionFailed { nonce: sf(data.get("nonce")).map(|n| n.0) }),
        "APPLICATION_COMMAND_AUTOCOMPLETE_RESPONSE" => Some(ParityEvent::Autocomplete {
            nonce: sf(data.get("nonce"))?.0,
            choices: data
                .get("choices")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|c| (crate::commands::slash::choice_name(c).to_owned(), c.get("value").cloned().unwrap_or(Value::Null)))
                        .collect()
                })
                .unwrap_or_default(),
        }),
        "CALL_CREATE" | "CALL_UPDATE" | "CALL_DELETE" => Some(ParityEvent::Call {
            channel: sf(data.get("channel_id"))?,
            message: if name == "CALL_DELETE" { None } else { sf(data.get("message_id")) },
            ringing: if name == "CALL_DELETE" {
                Vec::new()
            } else {
                data.get("ringing").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| sf(Some(v))).collect()).unwrap_or_default()
            },
            voice: if name == "CALL_DELETE" {
                Some(Vec::new())
            } else {
                data.get("voice_states").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| sf(v.get("user_id"))).collect())
            },
            region: (name != "CALL_DELETE").then(|| CallRegion {
                id: data.get("region").and_then(Value::as_str).map(str::to_owned),
                updated: name == "CALL_UPDATE",
            }),
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

pub struct UnreadCard {
    pub entry: crate::ui::inbox_unreads::Entry,
    pub msgs: Vec<Message>,
    pub more: bool,
    pub loaded: bool,
    pub error: Option<SharedString>,
    pub collapsed: bool,
}

pub struct InboxState {
    pub open: bool,
    pub tab: InboxTab,
    pub mentions: Vec<Message>,
    pub saved: Vec<Message>,
    pub scheduled: Vec<Message>,
    pub sched_edit: Option<crate::ui::chat::scheduled::SchedEdit>,
    pub loading: bool,
    pub reached_end: bool,
    pub filter_menu: bool,
    pub this_server: bool,
    pub everyone: bool,
    pub roles: bool,
    pub unreads: Vec<UnreadCard>,
    pub unreads_loading: bool,
    pub unreads_gen: u64,
}

impl Default for InboxState {
    fn default() -> Self {
        Self {
            open: false,
            tab: InboxTab::default(),
            mentions: Vec::new(),
            saved: Vec::new(),
            scheduled: Vec::new(),
            sched_edit: None,
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
    pub files: Vec<std::path::PathBuf>,
    pub media_hover: bool,
    pub sending: bool,
    pub error: Option<SharedString>,
}

#[derive(Default)]
pub struct ForumView {
    pub posts: Vec<ForumPost>,
    pub archived: Vec<ForumPost>,
    pub archived_more: bool,
    pub archived_loading: bool,
    pub archived_loaded: bool,
    pub has_more: bool,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<SharedString>,
    pub failures: u32,
    pub tags: Vec<Id>,
    pub sort: ForumSort,
    pub sort_menu: bool,
    pub tags_popout: bool,
    pub gallery: bool,
    pub match_all: bool,
    pub defaults_applied: bool,
    pub compose: Option<ForumCompose>,
    pub guidelines_open: bool,
    pub guidelines_seen: bool,
    pub guidelines_modal: bool,
    pub generation: u64,
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
    pub channel: Option<Id>,
    pub with: Vec<Id>,
    pub anchor: Option<(gpui::Point<gpui::Pixels>, gpui::Anchor)>,
    pub group: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallRegion {
    pub id: Option<String>,
    pub updated: bool,
}

#[derive(Default)]
pub struct ParityUi {
    pub inbox: InboxState,
    pub incoming_call: Option<Id>,
    pub call_users: HashMap<Id, Vec<Id>>,
    pub call_regions: HashMap<Id, CallRegion>,
    pub call_messages: HashMap<Id, Id>,
    pub threads_open: bool,
    pub archived_threads: HashMap<Id, Vec<ForumPost>>,
    pub forums: HashMap<Id, ForumView>,
    pub forum_compose_request: Option<Id>,
    pub forum_open_thread: Option<crate::model::ThreadRef>,
    pub pending_modal: Option<ModalSpec>,
    pub modal: Option<ModalView>,
    pub open_select: Option<(Id, String)>,
    pub select_values: HashMap<(Id, String), Vec<String>>,
    pub poll_choice: HashMap<Id, HashSet<u32>>,
    pub poll_show_results: HashSet<Id>,
    pub pending: HashSet<(Id, String)>,
    pub last_target: Option<(Option<Id>, Id)>,
    pub read_mark: Option<(Id, Id)>,
    pub moderation: Option<ModDialog>,
    pub group_dm: Option<GroupDmPicker>,
    pub toast: Option<SharedString>,
}

impl App {
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

pub fn on_event(app: &mut App, ev: ParityEvent, cx: &mut Context<App>) {
    match ev {
        ParityEvent::Modal(spec) => {
            app.parity.pending.clear();
            app.parity.pending_modal = Some(spec);
        }
        ParityEvent::InteractionCreate { nonce, id } => {
            if let Some(p) = crate::ui::chat::command_msg::set_interaction(nonce, id) {
                app.refresh_message(p.channel, Id(nonce));
            }
        }
        ParityEvent::InteractionSuccess { nonce, id: _ } => {
            app.parity.pending.clear();
            if let Some(p) = crate::ui::chat::command_msg::take(nonce) {
                app.discard_local(p.channel, Id(nonce), cx);
            }
        }
        ParityEvent::InteractionFailed { nonce } => {
            app.parity.pending.clear();
            let cmd = nonce.and_then(|n| {
                let app_name = crate::ui::chat::command_msg::get(n)?.app_name;
                crate::ui::chat::command_msg::set_phase(n, crate::ui::chat::command_msg::Phase::Failed(Some(crate::t!("slash.late", app = app_name).to_string())))
            });
            if let Some(p) = cmd {
                app.refresh_message(p.channel, Id(nonce.unwrap_or(0)));
            } else if !nonce.is_some_and(|n| crate::ui::chat::slash::autocomplete_failed(n, cx)) {
                app.parity_toast(crate::t!("interaction.no_response"), cx);
            }
        }
        ParityEvent::Autocomplete { nonce, choices } => crate::ui::chat::slash::autocomplete_result(nonce, choices, cx),
        ParityEvent::PollVote { channel, message, user, answer, add } => {
            let mine = user == app.me.id;
            if store::update_poll(message, |p| p.apply_gateway_vote(answer, add, mine)) {
                app.refresh_message(channel, message);
            }
        }
        ParityEvent::Call { channel, message, ringing, voice, region } => {
            match message {
                Some(m) => {
                    app.parity.call_messages.insert(channel, m);
                }
                None if region.is_none() => {
                    app.parity.call_messages.remove(&channel);
                }
                None => {}
            }
            match region {
                None => {
                    app.parity.call_regions.remove(&channel);
                }
                Some(r) => {
                    let prev = app.parity.call_regions.get(&channel);
                    let updated = r.updated && prev.is_some_and(|p| p.updated || p.id != r.id);
                    app.parity.call_regions.insert(channel, CallRegion { id: r.id, updated });
                }
            }
            if let Some(users) = voice {
                set_call_users(app, channel, users);
            }
            crate::ui::incoming_call::on_call(app, channel, &ringing);
        }
    }
    cx.notify();
}

pub fn set_call_users(app: &mut App, channel: Id, users: Vec<Id>) {
    if users.is_empty() {
        app.parity.call_users.remove(&channel);
    } else {
        app.parity.call_users.insert(channel, users);
    }
}

pub fn call_voice_state(calls: &mut HashMap<Id, Vec<Id>>, dm: Option<Id>, user: Id) {
    for users in calls.values_mut() {
        users.retain(|u| *u != user);
    }
    calls.retain(|_, users| !users.is_empty());
    if let Some(c) = dm {
        calls.entry(c).or_default().push(user);
    }
}

pub fn remember_read_mark(app: &mut App, channel: Id) {
    let unread = app.channels.get(&channel).is_some_and(|c| c.unread);
    app.parity.read_mark = if unread { app.backend.last_read(channel).map(|m| (channel, m)) } else { None };
}

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
    if app.chat.new_messages.is_none() {
        app.chat.new_messages = app.messages.get(&channel).and_then(|e| unread_since(&e.msgs, last));
    }
}

pub fn unread_since(msgs: &[Message], last: Id) -> Option<crate::ui::chat::state::NewMessages> {
    let oldest = msgs.iter().find(|m| m.id > last)?;
    let count = msgs.iter().filter(|m| m.id > last).count() as u32;
    let estimated = !msgs.iter().any(|m| m.id <= last);
    Some(crate::ui::chat::state::NewMessages { count, since: oldest.timestamp, estimated })
}

pub fn first_unread(msgs: &[Message], last: Id, me: Id) -> Option<Id> {
    msgs.iter().find(|m| m.id > last && m.author != me).map(|m| m.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn calls_carry_their_voice_states() {
        let ev = |name: &str, data: Value| from_gateway(&GatewayEvent::Unknown { name: name.into(), data });
        let create = ev("CALL_CREATE", json!({"channel_id":"4","message_id":"12","ringing":["9"],"voice_states":[{"user_id":"1","channel_id":"4"}]}));
        assert!(matches!(create, Some(ParityEvent::Call { channel: Id(4), message: Some(Id(12)), ref ringing, voice: Some(ref v), .. }) if ringing == &[Id(9)] && v == &[Id(1)]));
        let update = ev("CALL_UPDATE", json!({"channel_id":"4","ringing":[]}));
        assert!(matches!(update, Some(ParityEvent::Call { voice: None, .. })));
        let delete = ev("CALL_DELETE", json!({"channel_id":"4"}));
        assert!(matches!(delete, Some(ParityEvent::Call { voice: Some(ref v), region: None, .. }) if v.is_empty()));
        let moved = ev("CALL_UPDATE", json!({"channel_id":"4","region":"rotterdam"}));
        assert!(matches!(moved, Some(ParityEvent::Call { region: Some(CallRegion { ref id, updated: true }), .. }) if id.as_deref() == Some("rotterdam")));
    }

    #[test]
    fn dm_voice_states_move_between_calls() {
        let mut calls = HashMap::new();
        call_voice_state(&mut calls, Some(Id(4)), Id(1));
        call_voice_state(&mut calls, Some(Id(4)), Id(2));
        assert_eq!(calls[&Id(4)], [Id(1), Id(2)]);
        call_voice_state(&mut calls, Some(Id(5)), Id(1));
        assert_eq!(calls[&Id(4)], [Id(2)]);
        assert_eq!(calls[&Id(5)], [Id(1)]);
        call_voice_state(&mut calls, None, Id(2));
        assert!(!calls.contains_key(&Id(4)));
    }

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
    fn new_messages_bar_counts_like_the_read_state() {
        let mk = |id, author, ts| Message { id: Id(id), author: Id(author), timestamp: ts, ..Default::default() };
        let msgs = [mk(10, 5, 100), mk(11, 1, 110), mk(12, 5, 120)];
        let n = unread_since(&msgs, Id(10)).unwrap();
        assert_eq!((n.count, n.since, n.estimated), (2, 110, false));
        let n = unread_since(&msgs, Id(3)).unwrap();
        assert_eq!((n.count, n.since, n.estimated), (3, 100, true));
        assert!(unread_since(&msgs, Id(12)).is_none());
    }

    #[test]
    fn unread_marker_skips_own() {
        let mk = |id, author| Message { id: Id(id), author: Id(author), ..Default::default() };
        let msgs = [mk(1, 5), mk(2, 1), mk(3, 5)];
        assert_eq!(first_unread(&msgs, Id(1), Id(1)), Some(Id(3)));
        assert_eq!(first_unread(&msgs, Id(3), Id(1)), None);
    }
}

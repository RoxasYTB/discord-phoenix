use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{ChannelId, GuildId, MessageId, Snowflake, null_default};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct CommandOption {
    #[serde(rename = "type")]
    pub kind: u8,
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub autocomplete: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub choices: Vec<Value>,
    #[serde(default, deserialize_with = "null_default")]
    pub options: Vec<CommandOption>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApplicationCommand {
    pub id: Snowflake,
    pub application_id: Snowflake,
    #[serde(default)]
    pub version: Option<Snowflake>,
    #[serde(rename = "type", default = "one")]
    pub kind: u8,
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub description: String,
    #[serde(default, deserialize_with = "null_default")]
    pub options: Vec<CommandOption>,
}

fn one() -> u8 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandApplication {
    pub id: Snowflake,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CommandIndex {
    #[serde(default, deserialize_with = "null_default")]
    pub applications: Vec<CommandApplication>,
    #[serde(default, deserialize_with = "null_default")]
    pub application_commands: Vec<ApplicationCommand>,
}

impl CommandIndex {
    pub fn matching(&self, prefix: &str) -> Vec<&ApplicationCommand> {
        let p = prefix.to_lowercase();
        let mut v: Vec<_> = self
            .application_commands
            .iter()
            .filter(|c| c.kind == 1 && c.name.starts_with(&p))
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CommandScope {
    Guild(GuildId),
    Channel(ChannelId),
    Me,
}

pub(crate) fn index_path(scope: CommandScope) -> String {
    match scope {
        CommandScope::Guild(g) => format!("/guilds/{g}/application-command-index"),
        CommandScope::Channel(c) => format!("/channels/{c}/application-command-index"),
        CommandScope::Me => "/users/@me/application-command-index".into(),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InteractionTarget {
    pub guild: Option<GuildId>,
    pub channel: ChannelId,
}

fn nonce() -> String {
    Snowflake::now().to_string()
}

fn base(kind: u8, app: Snowflake, t: InteractionTarget, session: &str, data: Value) -> Value {
    let mut v = json!({
        "type": kind,
        "application_id": app,
        "channel_id": t.channel,
        "session_id": session,
        "nonce": nonce(),
        "data": data,
    });
    if let Some(g) = t.guild {
        v["guild_id"] = json!(g);
    }
    v
}

pub fn slash_body(cmd: &ApplicationCommand, t: InteractionTarget, session: &str, options: Value) -> Value {
    base(
        2,
        cmd.application_id,
        t,
        session,
        json!({
            "version": cmd.version.unwrap_or(cmd.id),
            "id": cmd.id,
            "name": cmd.name,
            "type": cmd.kind,
            "options": options,
            "attachments": [],
        }),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn component_body(
    app: Snowflake,
    t: InteractionTarget,
    session: &str,
    message: MessageId,
    message_flags: u64,
    custom_id: &str,
    component_type: u8,
    values: Option<&[String]>,
) -> Value {
    let mut data = json!({ "component_type": component_type, "custom_id": custom_id });
    if let Some(v) = values {
        data["values"] = json!(v);
    }
    let mut b = base(3, app, t, session, data);
    b["message_id"] = json!(message);
    b["message_flags"] = json!(message_flags);
    b
}

pub fn modal_body(app: Snowflake, t: InteractionTarget, session: &str, modal_id: Snowflake, custom_id: &str, components: Value) -> Value {
    base(5, app, t, session, json!({ "id": modal_id, "custom_id": custom_id, "components": components }))
}

impl RestClient {
    pub async fn get_command_index(&self, scope: CommandScope) -> Result<CommandIndex> {
        self.call(Method::GET, &index_path(scope), Body::None).await
    }

    pub async fn create_interaction(&self, body: Value) -> Result<()> {
        self.call_empty(Method::POST, "/interactions", Body::Json(body)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(index_path(CommandScope::Guild(Snowflake(1))), "/guilds/1/application-command-index");
        assert_eq!(index_path(CommandScope::Channel(Snowflake(2))), "/channels/2/application-command-index");
        assert_eq!(index_path(CommandScope::Me), "/users/@me/application-command-index");
    }

    #[test]
    fn parse_index_and_bodies() {
        let idx: CommandIndex = serde_json::from_str(
            r#"{"applications":[{"id":"9","name":"Bot","icon":null}],
               "application_commands":[
                 {"id":"5","application_id":"9","version":"6","type":1,"name":"ping","description":"Pong",
                  "options":[{"type":3,"name":"text","description":"t","required":true}]},
                 {"id":"7","application_id":"9","type":2,"name":"pinger","description":""}]}"#,
        )
        .unwrap();
        let m = idx.matching("PIN");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].options[0].kind, 3);
        let t = InteractionTarget { guild: Some(Snowflake(3)), channel: Snowflake(4) };
        let b = slash_body(m[0], t, "sess", json!([{"type":3,"name":"text","value":"hi"}]));
        assert_eq!(b["type"], 2);
        assert_eq!(b["guild_id"], "3");
        assert_eq!(b["data"]["version"], "6");
        let c = component_body(Snowflake(9), InteractionTarget { guild: None, channel: Snowflake(4) }, "s", Snowflake(8), 0, "btn", 2, None);
        assert_eq!(c["type"], 3);
        assert!(c.get("guild_id").is_none());
        assert_eq!(c["data"]["custom_id"], "btn");
        let s = component_body(Snowflake(9), t, "s", Snowflake(8), 0, "sel", 3, Some(&["a".into()]));
        assert_eq!(s["data"]["values"][0], "a");
        assert_eq!(modal_body(Snowflake(9), t, "s", Snowflake(1), "m", json!([]))["type"], 5);
    }
}

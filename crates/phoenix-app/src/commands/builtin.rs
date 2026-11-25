use serde_json::{Value, json};

use super::SlashCommand;
use crate::{app::PhoenixApp, model::Id};

const KICK_MEMBERS: u64 = 1 << 1;
const BAN_MEMBERS: u64 = 1 << 2;
const CHANGE_NICKNAME: u64 = 1 << 26;

fn opt(name: &str, kind: u64, desc: &str, required: bool) -> Value {
    json!({ "name": name, "type": kind, "description": desc, "required": required })
}

fn text_opt(desc_key: &str, required: bool) -> Value {
    opt("message", 3, &crate::t!(desc_key), required)
}

fn make(id: &str, name: &str, desc: &str, options: Vec<Value>) -> SlashCommand {
    let leaf = json!({ "id": id, "name": name, "type": 1, "description": desc, "options": options });
    SlashCommand {
        name: name.to_owned(),
        description: desc.to_owned(),
        app_name: crate::t!("slash.built_in").to_string(),
        app_icon: None,
        raw: leaf.clone(),
        id: id.to_owned(),
        app_id: None,
        bot_id: None,
        kind: 1,
        path: Vec::new(),
        leaf,
        guild_scoped: false,
    }
}

fn delete_choices() -> Vec<Value> {
    [
        ("slash.ban_none", 0),
        ("slash.ban_1h", 3600),
        ("slash.ban_6h", 21600),
        ("slash.ban_12h", 43200),
        ("slash.ban_24h", 86400),
        ("slash.ban_3d", 259200),
        ("slash.ban_7d", 604800),
    ]
    .iter()
    .map(|(k, v)| json!({ "name": crate::t!(k).to_string(), "value": v }))
    .collect()
}

fn timeout_choices() -> Vec<Value> {
    [
        ("mod.timeout.60s", 60),
        ("mod.timeout.5m", 300),
        ("mod.timeout.10m", 600),
        ("mod.timeout.1h", 3600),
        ("mod.timeout.1d", 86400),
        ("mod.timeout.1w", 604800),
    ]
    .iter()
    .map(|(k, v)| json!({ "name": crate::t!(k).to_string(), "value": v }))
    .collect()
}

pub struct Context {
    pub guild: Option<Id>,
    pub group_dm: bool,
    pub tts: bool,
    pub nick: bool,
    pub thread: bool,
    pub kick: bool,
    pub ban: bool,
    pub timeout: bool,
    pub schedule: bool,
}

impl Context {
    pub fn of(app: &PhoenixApp, channel: Id) -> Self {
        let ch = app.channels.get(&channel).or_else(|| app.nav.find_thread(channel));
        let guild = ch.and_then(|c| c.guild);
        let has = |bit| crate::actions::has(app, guild, bit);
        let text = ch.is_some_and(|c| matches!(c.kind, crate::model::ChannelKind::Text | crate::model::ChannelKind::Announcement));
        Self {
            guild,
            group_dm: guild.is_none() && crate::ui::dm_profile_panel::is_group_dm(app, channel),
            tts: crate::tts::can_use_command(app, channel),
            nick: guild.is_some() && (has(CHANGE_NICKNAME) || has(crate::actions::MANAGE_NICKNAMES)),
            thread: guild.is_some() && text && app.nav.thread_parent(channel).is_none() && has(crate::actions::CREATE_PUBLIC_THREADS),
            kick: has(KICK_MEMBERS),
            ban: has(BAN_MEMBERS),
            timeout: has(crate::actions::MODERATE_MEMBERS),
            schedule: true,
        }
    }
}

pub fn list(cx: &Context) -> Vec<SlashCommand> {
    let mut v = vec![
        make("-16", "gif", &crate::t!("slash.gif_desc"), vec![opt("query", 3, &crate::t!("slash.gif_query"), true)]),
        make("-1", "shrug", &crate::t!("slash.shrug_desc"), vec![text_opt("slash.your_message", false)]),
        make("-2", "tableflip", &crate::t!("slash.tableflip_desc"), vec![text_opt("slash.your_message", false)]),
        make("-3", "unflip", &crate::t!("slash.unflip_desc"), vec![text_opt("slash.your_message", false)]),
        make("-5", "me", &crate::t!("slash.me_desc"), vec![text_opt("slash.your_message", true)]),
        make("-6", "spoiler", &crate::t!("slash.spoiler_desc"), vec![text_opt("slash.your_message", true)]),
        make(
            "-14",
            "msg",
            &crate::t!("slash.msg_desc"),
            vec![opt("user", 6, &crate::t!("slash.msg_user"), true), opt("message", 3, &crate::t!("slash.msg_message"), true)],
        ),
        make("-17", "sticker", &crate::t!("slash.sticker_desc"), vec![opt("query", 3, &crate::t!("slash.sticker_query"), true)]),
    ];
    if cx.tts {
        v.push(make("-4", "tts", &crate::t!("tts.cmd_desc"), vec![text_opt("tts.cmd_message", true)]));
    }
    if cx.nick {
        let mut o = opt("new_nick", 3, &crate::t!("slash.nick_new"), false);
        o["max_length"] = json!(32);
        v.push(make("-7", "nick", &crate::t!("slash.nick_desc"), vec![o]));
    }
    if cx.thread {
        let mut name = opt("name", 3, &crate::t!("slash.thread_name"), true);
        name["max_length"] = json!(100);
        v.push(make("-10", "thread", &crate::t!("slash.thread_desc"), vec![name, opt("message", 3, &crate::t!("slash.thread_message"), true)]));
    }
    if cx.kick {
        v.push(make(
            "-11",
            "kick",
            &crate::t!("slash.kick_desc"),
            vec![opt("user", 6, &crate::t!("slash.kick_user"), true), opt("reason", 3, &crate::t!("slash.kick_reason"), false)],
        ));
    }
    if cx.ban {
        let mut del = opt("delete_messages", 4, &crate::t!("slash.ban_delete"), true);
        del["choices"] = Value::Array(delete_choices());
        v.push(make(
            "-12",
            "ban",
            &crate::t!("slash.ban_desc"),
            vec![opt("user", 6, &crate::t!("slash.ban_user"), true), del, opt("reason", 3, &crate::t!("slash.ban_reason"), false)],
        ));
    }
    if cx.timeout {
        let mut d = opt("duration", 4, &crate::t!("slash.timeout_duration"), true);
        d["choices"] = Value::Array(timeout_choices());
        v.push(make(
            "-13",
            "timeout",
            &crate::t!("slash.timeout_desc"),
            vec![opt("user", 6, &crate::t!("slash.timeout_user"), true), d, opt("reason", 3, &crate::t!("slash.timeout_reason"), false)],
        ));
    }
    if cx.group_dm {
        let mut s = opt("silent", 5, &crate::t!("slash.leave_silent"), false);
        s["description"] = json!(crate::t!("slash.leave_silent_desc").to_string());
        v.push(make("-15", "leave", &crate::t!("slash.leave_desc"), vec![s]));
    }
    if cx.schedule {
        v.push(make("-19", "schedule", &crate::t!("slash.schedule_desc"), vec![opt("message", 3, &crate::t!("slash.schedule_message"), true)]));
    }
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

fn value<'a>(opts: &'a [Value], name: &str) -> Option<&'a Value> {
    opts.iter().find(|o| o.get("name").and_then(Value::as_str) == Some(name)).and_then(|o| o.get("value"))
}

pub fn text(opts: &[Value], name: &str) -> String {
    value(opts, name).map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string())).unwrap_or_default()
}

pub fn int(opts: &[Value], name: &str) -> Option<i64> {
    value(opts, name).and_then(Value::as_i64)
}

pub fn content(id: &str, opts: &[Value]) -> Option<String> {
    let m = text(opts, "message");
    Some(match id {
        "-1" => format!("{m} ¯\\_(ツ)_/¯").trim().to_owned(),
        "-2" => format!("{m} (╯°□°)╯︵ ┻━┻").trim().to_owned(),
        "-3" => format!("{m} ┬─┬ノ( º _ ºノ)").trim().to_owned(),
        "-5" => format!("_{m}_"),
        "-6" => format!("||{m}||").trim().to_owned(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_builtins_match_the_client() {
        let o = |v: &str| vec![json!({"name":"message","value":v})];
        assert_eq!(content("-1", &o("hey")).as_deref(), Some("hey ¯\\_(ツ)_/¯"));
        assert_eq!(content("-1", &[]).as_deref(), Some("¯\\_(ツ)_/¯"));
        assert_eq!(content("-5", &o("waves")).as_deref(), Some("_waves_"));
        assert_eq!(content("-6", &o("secret")).as_deref(), Some("||secret||"));
        assert_eq!(content("-2", &[]).as_deref(), Some("(╯°□°)╯︵ ┻━┻"));
        assert!(content("-7", &[]).is_none());
    }

    #[test]
    fn predicates_gate_the_list() {
        let none = Context { guild: None, group_dm: false, tts: false, nick: false, thread: false, kick: false, ban: false, timeout: false, schedule: true };
        let names: Vec<String> = list(&none).into_iter().map(|c| c.name).collect();
        assert!(names.contains(&"shrug".to_string()) && names.contains(&"gif".to_string()));
        assert!(!names.contains(&"ban".to_string()) && !names.contains(&"tts".to_string()) && !names.contains(&"leave".to_string()));
        let all = Context { guild: Some(Id(1)), group_dm: true, tts: true, nick: true, thread: true, kick: true, ban: true, timeout: true, schedule: true };
        let l = list(&all);
        let ban = l.iter().find(|c| c.name == "ban").unwrap();
        assert_eq!(ban.options()[1]["choices"].as_array().unwrap().len(), 7);
        assert!(l.iter().all(|c| c.builtin()));
    }
}

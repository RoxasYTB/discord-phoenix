use super::*;
use phoenix_api::gateway::events::parse_dispatch;

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../phoenix-api/tests/fixtures/");
const ME: u64 = 100000000000000001;
const CH: u64 = 400000000000000001;
const DM: u64 = 900000000000000001;

fn fixture(file: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{FIX}{file}")).unwrap()).unwrap()
}

fn setup() -> (Cache, Adapter) {
    let cache = Cache::default();
    let e = parse_dispatch("READY", &fixture("ready_user.json").to_string());
    cache.apply(&e);
    let GatewayEvent::Ready(r) = e else { panic!("not ready") };
    let mut prefs = Prefs::from_ready(&r);
    prefs.reads.insert(CH, ReadMark { last_read: 700000000000000050, mentions: 0 });
    prefs.reads.insert(DM, ReadMark { last_read: 900000000000000500, mentions: 0 });
    (cache, Adapter::new(r.user.id, prefs))
}

fn msg(id: u64, channel: u64, guild: bool, author: u64, everyone: bool) -> GatewayEvent {
    let mut m = fixture("message_create.json");
    m["id"] = Value::String(id.to_string());
    m["channel_id"] = Value::String(channel.to_string());
    if !guild {
        m.as_object_mut().unwrap().remove("guild_id");
        m.as_object_mut().unwrap().remove("member");
    }
    m["author"]["id"] = Value::String(author.to_string());
    m["mentions"] = Value::Array(vec![]);
    m["mention_roles"] = Value::Array(vec![]);
    m["mention_everyone"] = Value::Bool(everyone);
    parse_dispatch("MESSAGE_CREATE", &m.to_string())
}

fn feed(cache: &Cache, a: &mut Adapter, e: &GatewayEvent) -> Vec<AppEvent> {
    cache.apply(e);
    a.map(cache, e)
}

fn channel_unread(out: &[AppEvent], ch: u64) -> Option<(bool, u32)> {
    out.iter().rev().find_map(|x| match x {
        AppEvent::ChannelUnread { channel, unread, mentions } if channel.0 == ch => Some((*unread, *mentions)),
        _ => None,
    })
}

#[test]
fn own_message_from_another_client_stays_read() {
    let (cache, mut a) = setup();
    let out = feed(&cache, &mut a, &msg(700000000000000060, CH, true, ME, false));
    assert_eq!(channel_unread(&out, CH), Some((false, 0)));
}

#[test]
fn foreign_message_marks_unread_and_everyone_counts_as_mention() {
    let (cache, mut a) = setup();
    let out = feed(&cache, &mut a, &msg(700000000000000060, CH, true, 200000000000000003, false));
    assert_eq!(channel_unread(&out, CH), Some((true, 0)));
    let out = feed(&cache, &mut a, &msg(700000000000000061, CH, true, 200000000000000003, true));
    assert_eq!(channel_unread(&out, CH), Some((true, 1)));
    let g = out.iter().find_map(|x| match x { AppEvent::GuildUnread { mentions, unread, .. } => Some((*unread, *mentions)), _ => None });
    assert_eq!(g, Some((true, 1)));
}

#[test]
fn every_dm_message_counts_as_a_mention() {
    let (cache, mut a) = setup();
    for (i, n) in [(900000000000000600, 1), (900000000000000601, 2)] {
        let out = feed(&cache, &mut a, &msg(i, DM, false, 200000000000000002, false));
        let dm = out.iter().find_map(|x| match x { AppEvent::DmUpsert(c) => Some(c), _ => None }).expect("dm upsert");
        assert!(dm.unread);
        assert_eq!(dm.mentions, n);
    }
}

#[test]
fn ack_from_the_official_client_clears_unread() {
    let (cache, mut a) = setup();
    feed(&cache, &mut a, &msg(700000000000000061, CH, true, 200000000000000003, true));
    let ack = parse_dispatch("MESSAGE_ACK", &format!(r#"{{"channel_id":"{CH}","message_id":"700000000000000061","version":3}}"#));
    let out = feed(&cache, &mut a, &ack);
    assert_eq!(channel_unread(&out, CH), Some((false, 0)));
    let g = out.iter().find_map(|x| match x { AppEvent::GuildUnread { mentions, unread, .. } => Some((*unread, *mentions)), _ => None });
    assert_eq!(g, Some((false, 0)));
}

#[test]
fn guild_update_renames_the_guild() {
    let (cache, mut a) = setup();
    let mut g = fixture("guild_create.json");
    g["id"] = Value::String("300000000000000001".into());
    g["name"] = Value::String("Renamed".into());
    let out = feed(&cache, &mut a, &parse_dispatch("GUILD_UPDATE", &g.to_string()));
    let (name, chans) = out
        .iter()
        .find_map(|x| match x { AppEvent::GuildCreate { guild, channels, .. } => Some((guild.name.clone(), channels.len())), _ => None })
        .expect("guild upsert");
    assert_eq!(name.as_ref(), "Renamed");
    assert!(chans > 0, "channels kept across GUILD_UPDATE");
}

#[test]
fn guild_settings_update_from_another_client_mutes_live() {
    let (cache, mut a) = setup();
    let upd = |muted: bool| {
        let d = serde_json::json!({"guild_id": "300000000000000001", "muted": muted,
            "channel_overrides": [{"channel_id": CH.to_string(), "muted": muted}]});
        GatewayEvent::Unknown { name: "USER_GUILD_SETTINGS_UPDATE".into(), data: d }
    };
    let out = feed(&cache, &mut a, &upd(true));
    let ch = out.iter().find_map(|x| match x { AppEvent::GuildChannels { channels, .. } => channels.iter().find(|c| c.id.0 == CH).cloned(), _ => None });
    assert!(ch.expect("channel listed").muted);
    feed(&cache, &mut a, &upd(false));
    assert!(!a.prefs.mutes.guilds.contains(&300000000000000001) && !a.prefs.mutes.channels.contains(&CH));
}

#[test]
fn presence_with_profile_updates_the_user() {
    let (cache, mut a) = setup();
    let raw = r#"{"user":{"id":"200000000000000002","username":"alice2","global_name":"Alice Two","avatar":"abc"},"status":"online","activities":[]}"#;
    let out = feed(&cache, &mut a, &parse_dispatch("PRESENCE_UPDATE", raw));
    let u = out.iter().find_map(|x| match x { AppEvent::Users(u) => u.first().cloned(), _ => None }).expect("user refreshed");
    assert_eq!(u.display_name.as_ref(), "Alice Two");
    assert!(matches!(u.avatar, model::ImageRef::Url(_)));
    let out = feed(&cache, &mut a, &parse_dispatch("PRESENCE_UPDATE", r#"{"user":{"id":"200000000000000002"},"status":"idle"}"#));
    assert!(!out.iter().any(|x| matches!(x, AppEvent::Users(_))));
}

#[test]
fn joined_thread_messages_make_the_guild_unread() {
    let (cache, mut a) = setup();
    let th = 400000000000000090u64;
    let raw = format!(
        r#"{{"id":"{th}","type":11,"guild_id":"300000000000000001","parent_id":"{CH}","name":"t","last_message_id":"700000000000000050","member":{{"muted":false,"flags":1}},"thread_metadata":{{"archived":false,"locked":false,"auto_archive_duration":1440,"archive_timestamp":"2026-11-24T00:00:00+00:00"}}}}"#
    );
    feed(&cache, &mut a, &parse_dispatch("THREAD_CREATE", &raw));
    a.prefs.reads.insert(th, ReadMark { last_read: 700000000000000050, mentions: 0 });
    let out = feed(&cache, &mut a, &msg(700000000000000070, th, true, 200000000000000003, true));
    let g = out.iter().find_map(|x| match x { AppEvent::GuildUnread { mentions, unread, .. } => Some((*unread, *mentions)), _ => None });
    assert_eq!(g, Some((true, 1)));
}

#[test]
fn sessions_replace_sets_my_status() {
    let (cache, mut a) = setup();
    let d = serde_json::json!([{"session_id": "abc", "status": "online"}, {"session_id": "all", "status": "dnd", "activities": [{"type": 4, "name": "Custom Status", "state": "busy"}]}]);
    let out = feed(&cache, &mut a, &GatewayEvent::Unknown { name: "SESSIONS_REPLACE".into(), data: d });
    assert!(out.iter().any(|x| matches!(x, AppEvent::PresenceUpdate { user, presence: model::Presence::Dnd } if user.0 == ME)));
    let acts = out.iter().find_map(|x| match x { AppEvent::Activities { user, activities } if user.0 == ME => Some(activities), _ => None }).expect("own activities");
    assert_eq!(acts[0].state.as_deref(), Some("busy"));
}

use std::sync::Arc;

use phoenix_api::Cache;
use phoenix_api::gateway::{GatewayEvent, parse_dispatch};
use phoenix_api::models::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn msg(id: u64, ch: u64, content: &str) -> Message {
    Message {
        id: Snowflake(id),
        channel_id: Snowflake(ch),
        content: content.into(),
        author: User { id: Snowflake(5), username: "u".into(), ..Default::default() },
        ..Default::default()
    }
}

#[test]
fn ready_populates_cache() {
    let cache = Cache::default();
    assert!(cache.apply(&parse_dispatch("READY", &fixture("ready_user.json"))));
    let g = Snowflake(300000000000000001);
    assert_eq!(cache.current_user().unwrap().username, "phoenix");
    assert_eq!(cache.guilds().len(), 2);
    assert_eq!(cache.guild(g).unwrap().name(), "Rustaceans");
    assert!(cache.guild(g).unwrap().channels.is_empty());
    assert_eq!(cache.guild_channels(g).len(), 9);
    let dms = cache.private_channels();
    assert_eq!(dms.len(), 2);
    assert_eq!(dms[0].recipients[0].username, "alice");
    assert_eq!(dms[0].display_name(), "Alice");
    assert_eq!(cache.member(g, cache.current_user().unwrap().id).unwrap().nick.as_deref(), Some("phx"));
    assert_eq!(cache.permissions_in(Snowflake(400000000000000001)), Permissions::all());
    assert_eq!(cache.permissions_in(Snowflake(900000000000000001)), Permissions::all());
}

#[test]
fn message_ring_buffer_and_updates() {
    let cache = Cache::new(3);
    let ch = 77;
    for i in 1..=5 {
        cache.apply(&GatewayEvent::MessageCreate(Box::new(msg(i, ch, "x"))));
    }
    let ids: Vec<u64> = cache.messages(Snowflake(ch)).iter().map(|m| m.id.0).collect();
    assert_eq!(ids, vec![3, 4, 5]);

    cache.insert_messages(Snowflake(ch), vec![msg(2, ch, "old")]);
    let ids: Vec<u64> = cache.messages(Snowflake(ch)).iter().map(|m| m.id.0).collect();
    assert_eq!(ids, vec![3, 4, 5]);

    let snapshot = cache.messages(Snowflake(ch));
    let v0 = cache.version();
    assert!(cache.apply(&parse_dispatch("MESSAGE_UPDATE", r#"{"id":"4","channel_id":"77","content":"edited","edited_timestamp":"2024-01-01T00:00:00Z"}"#)));
    assert!(cache.version() > v0);
    let m = cache.message(Snowflake(ch), Snowflake(4)).unwrap();
    assert_eq!(m.content, "edited");
    assert_eq!(m.author.username, "u");
    assert_eq!(snapshot[1].content, "x");

    assert!(cache.apply(&parse_dispatch("MESSAGE_DELETE", r#"{"id":"3","channel_id":"77"}"#)));
    assert_eq!(cache.messages(Snowflake(ch)).len(), 2);
    assert!(!cache.apply(&parse_dispatch("MESSAGE_DELETE", r#"{"id":"3","channel_id":"77"}"#)));

    cache.apply(&parse_dispatch(
        "MESSAGE_REACTION_ADD",
        r#"{"user_id":"5","channel_id":"77","message_id":"5","emoji":{"id":null,"name":"🦀"},"burst":false,"type":0}"#,
    ));
    assert_eq!(cache.message(Snowflake(ch), Snowflake(5)).unwrap().reactions[0].count, 1);

    assert!(cache.apply(&parse_dispatch(
        "MESSAGE_REACTION_ADD_MANY",
        r#"{"channel_id":"77","message_id":"5","guild_id":"1","reactions":[{"emoji":{"id":null,"name":"🦀"},"users":["6","7"]},{"emoji":{"id":"9","name":"blob"},"users":["8"]}]}"#,
    )));
    let r = cache.message(Snowflake(ch), Snowflake(5)).unwrap().reactions.clone();
    assert_eq!((r[0].count, r[1].count, r.len()), (3, 1, 2));
}

#[test]
fn group_dm_recipients_follow_gateway() {
    let cache = Cache::default();
    cache.apply(&parse_dispatch("CHANNEL_CREATE", r#"{"id":"50","type":3,"recipients":[{"id":"6","username":"a"}]}"#));
    let ids = |c: &Cache| c.channel(Snowflake(50)).unwrap().recipients.iter().map(|u| u.id.0).collect::<Vec<_>>();
    assert!(cache.apply(&parse_dispatch("CHANNEL_RECIPIENT_ADD", r#"{"channel_id":"50","user":{"id":"7","username":"b"}}"#)));
    assert_eq!(ids(&cache), vec![6, 7]);
    assert!(cache.apply(&parse_dispatch("CHANNEL_RECIPIENT_REMOVE", r#"{"channel_id":"50","user":{"id":"6","username":"a"}}"#)));
    assert_eq!(ids(&cache), vec![7]);
    assert_eq!(cache.user(Snowflake(7)).unwrap().username, "b");
}

#[test]
fn relationship_update_merges_into_existing() {
    let cache = Cache::default();
    cache.apply(&parse_dispatch("RELATIONSHIP_ADD", r#"{"id":"9","type":3,"user":{"id":"9","username":"c"},"nickname":null}"#));
    assert!(cache.apply(&parse_dispatch("RELATIONSHIP_UPDATE", r#"{"id":"9","type":1,"nickname":"Cee"}"#)));
    let r = cache.relationship(Snowflake(9)).unwrap();
    assert_eq!((r.kind, r.nickname.as_deref(), r.user.as_ref().map(|u| u.username.as_str())), (RelationshipType::Friend, Some("Cee"), Some("c")));
    assert!(!cache.apply(&parse_dispatch("RELATIONSHIP_UPDATE", r#"{"id":"10","type":1}"#)));
}

#[test]
fn nonce_replaces_local_echo() {
    let cache = Cache::default();
    let mut local = msg(10, 1, "hi");
    local.nonce = Some(Nonce::Str("n1".into()));
    cache.insert_messages(Snowflake(1), vec![local]);
    let mut real = msg(20, 1, "hi");
    real.nonce = Some(Nonce::Str("n1".into()));
    cache.apply(&GatewayEvent::MessageCreate(Box::new(real)));
    let ms = cache.messages(Snowflake(1));
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0].id, Snowflake(20));
}

#[test]
fn guild_lifecycle_and_roles() {
    let cache = Cache::default();
    let g: Guild = serde_json::from_str(&fixture("guild_create.json")).unwrap();
    let gid = g.id;
    cache.apply(&GatewayEvent::GuildCreate(Box::new(g)));
    assert_eq!(cache.guild_channels(gid).len(), 1);
    cache.apply(&parse_dispatch(
        "GUILD_ROLE_CREATE",
        r#"{"guild_id":"300000000000000003","role":{"id":"9","name":"new","permissions":"0"}}"#,
    ));
    assert_eq!(cache.guild(gid).unwrap().roles.len(), 2);
    cache.apply(&parse_dispatch("GUILD_ROLE_DELETE", r#"{"guild_id":"300000000000000003","role_id":"9"}"#));
    assert_eq!(cache.guild(gid).unwrap().roles.len(), 1);
    cache.apply(&parse_dispatch(
        "CHANNEL_CREATE",
        r#"{"id":"5","type":0,"guild_id":"300000000000000003","name":"new","position":1}"#,
    ));
    assert_eq!(cache.guild_channels(gid).len(), 2);
    cache.apply(&parse_dispatch("GUILD_DELETE", r#"{"id":"300000000000000003","unavailable":true}"#));
    assert!(cache.guild(gid).unwrap().unavailable);
    cache.apply(&parse_dispatch("GUILD_DELETE", r#"{"id":"300000000000000003"}"#));
    assert!(cache.guild(gid).is_none());
    assert!(cache.channel(Snowflake(5)).is_none());
}

#[test]
fn voice_state_tracking() {
    let cache = Cache::default();
    cache.apply(&parse_dispatch(
        "VOICE_STATE_UPDATE",
        r#"{"guild_id":"1","channel_id":"2","user_id":"3","session_id":"s"}"#,
    ));
    assert_eq!(cache.voice_states(Some(Snowflake(1))).len(), 1);
    cache.apply(&parse_dispatch("VOICE_STATE_UPDATE", r#"{"guild_id":"1","channel_id":null,"user_id":"3","session_id":"s"}"#));
    assert!(cache.voice_states(Some(Snowflake(1))).is_empty());
}

#[test]
fn cheap_concurrent_reads() {
    let cache = Cache::default();
    cache.apply(&parse_dispatch("READY", &fixture("ready_user.json")));
    let c2 = cache.clone();
    let h = std::thread::spawn(move || c2.guilds().len());
    assert_eq!(h.join().unwrap(), 2);
    let a = cache.guild(Snowflake(300000000000000001)).unwrap();
    let b = cache.guild(Snowflake(300000000000000001)).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn thread_membership_follows_member_events() {
    let cache = Cache::default();
    cache.apply(&parse_dispatch("READY", &fixture("ready_user.json")));
    let me = cache.current_user().unwrap().id.0;
    let raw = r#"{"id":"800000000000000001","type":11,"guild_id":"300000000000000001","parent_id":"400000000000000001","name":"t","member":{"id":"800000000000000001","flags":2,"muted":false},"thread_metadata":{"archived":false,"locked":false}}"#;
    cache.apply(&parse_dispatch("THREAD_CREATE", raw));
    let t = Snowflake(800000000000000001);
    cache.apply(&parse_dispatch("THREAD_UPDATE", r#"{"id":"800000000000000001","type":11,"guild_id":"300000000000000001","parent_id":"400000000000000001","name":"t2","thread_metadata":{"archived":true,"locked":false}}"#));
    let c = cache.channel(t).unwrap();
    assert_eq!(c.name.as_deref(), Some("t2"));
    assert_eq!(c.member.as_ref().unwrap()["flags"], 2);
    let left = format!(r#"{{"id":"800000000000000001","guild_id":"300000000000000001","member_count":0,"removed_member_ids":["{me}"]}}"#);
    cache.apply(&parse_dispatch("THREAD_MEMBERS_UPDATE", &left));
    assert!(cache.channel(t).unwrap().member.is_none());
    assert_eq!(cache.channel(t).unwrap().member_count, Some(0));
    cache.apply(&parse_dispatch("THREAD_MEMBER_UPDATE", r#"{"id":"800000000000000001","guild_id":"300000000000000001","flags":4,"muted":true}"#));
    assert_eq!(cache.channel(t).unwrap().member.as_ref().unwrap()["flags"], 4);
}

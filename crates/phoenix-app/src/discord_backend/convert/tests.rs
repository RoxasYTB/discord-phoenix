use super::*;
use phoenix_api::gateway::events::parse_dispatch;

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../phoenix-api/tests/fixtures/");

fn ev(name: &str, file: &str) -> GatewayEvent {
    let raw = std::fs::read_to_string(format!("{FIX}{file}")).unwrap();
    parse_dispatch(name, &raw)
}

fn ready_cache() -> (Cache, Box<Ready>) {
    let cache = Cache::default();
    let e = ev("READY", "ready_user.json");
    cache.apply(&e);
    let GatewayEvent::Ready(r) = e else { panic!("not ready") };
    (cache, r)
}

#[test]
fn ready_builds_guilds_and_dms() {
    let (cache, r) = ready_cache();
    let prefs = Prefs::from_ready(&r);
    let d = build_ready(&cache, &r, &prefs);
    assert_eq!(d.me.display_name.as_ref(), "Phoenix");
    assert!(d.guilds.iter().any(|g| g.name.as_ref() == "Rustaceans"));
    assert_eq!(d.dm_channels.len(), 2);
    assert_eq!(d.dm_channels[0].id, Id(900000000000000001));
    assert!(d.dm_channels.iter().all(|c| c.guild.is_none() && !c.voice_users.is_empty()));
    let me = &d.users.iter().find(|u| u.id == d.me.id).unwrap().avatar;
    assert!(matches!(me, model::ImageRef::Url(u) if u.contains("size=128")));
}

#[test]
fn message_create_maps() {
    let (cache, r) = ready_cache();
    let mut a = Adapter::new(r.user.id, Prefs::from_ready(&r));
    let e = ev("MESSAGE_CREATE", "message_create.json");
    cache.apply(&e);
    let out = a.map(&cache, &e);
    let m = out.iter().find_map(|x| match x { AppEvent::MessageCreate(m) => Some(m), _ => None }).unwrap();
    assert_eq!(m.id, Id(700000000000000051));
    assert!(m.attachments[0].is_image && !m.attachments[0].url.is_empty());
    assert!(out.iter().any(|x| matches!(x, AppEvent::Users(u) if u.iter().any(|u| u.name.as_ref() == "bob"))));
}

#[test]
fn member_list_update_maps() {
    let (cache, r) = ready_cache();
    let mut a = Adapter::new(r.user.id, Prefs::from_ready(&r));
    let e = ev("GUILD_MEMBER_LIST_UPDATE", "member_list_update.json");
    cache.apply(&e);
    let out = a.map(&cache, &e);
    let ms = out.iter().find_map(|x| match x { AppEvent::GuildMembers { members, .. } => Some(members), _ => None }).unwrap();
    assert_eq!(ms[0].presence, model::Presence::Idle);
    assert!(ms.len() == 1 || (ms.len() == 2 && ms[1].user == Id(r.user.id.0)));
}

#[test]
fn read_state_and_proto_order() {
    let rs = serde_json::json!({"entries": [{"id": "5", "last_message_id": "10", "mention_count": 2}]});
    let m = parse_read_state(Some(&rs));
    assert_eq!(m[&5], ReadMark { last_read: 10, mentions: 2 });
    let mut folder = vec![0x0a, 16];
    folder.extend(7u64.to_le_bytes());
    folder.extend(3u64.to_le_bytes());
    let mut gf = vec![0x0a, folder.len() as u8];
    gf.extend(folder);
    let mut top = vec![0x72, gf.len() as u8];
    top.extend(gf);
    let b64 = base64::engine::general_purpose::STANDARD.encode(top);
    assert_eq!(guild_order_from_proto(&b64), Some(vec![7, 3]));
}

#[test]
fn guild_resource_channels_are_listed_but_not_unread() {
    let (cache, r) = ready_cache();
    let g = r.guilds.iter().find(|g| !g.unavailable).unwrap().id;
    let raw = format!(
        r#"{{"id":"700000000000000077","type":0,"guild_id":"{g}","name":"guide-page","position":0,"flags":128,"last_message_id":"700000000000000099"}}"#
    );
    cache.apply(&parse_dispatch("CHANNEL_CREATE", &raw));
    let mut prefs = Prefs::from_ready(&r);
    let chs = guild_channels(&cache, g, &prefs.ctx());
    let c = chs.iter().find(|c| c.id == Id(700000000000000077)).expect("resource channel listed");
    assert!(!c.unread && c.mentions == 0);
    prefs.mutes.opt_in.insert(g.0);
    let chs = guild_channels(&cache, g, &prefs.ctx());
    assert!(!chs.iter().any(|c| c.id == Id(700000000000000077)));
}

#[test]
fn opt_in_channel_overrides_are_followed() {
    let v = serde_json::json!([{
        "guild_id": "1",
        "flags": 16384,
        "channel_overrides": [
            { "channel_id": "10", "flags": 4096, "muted": false },
            { "channel_id": "11", "flags": 0, "muted": true }
        ]
    }]);
    let m = parse_mutes(Some(&v));
    assert!(m.opt_in.contains(&1));
    assert!(m.opted_in.contains(&10));
    assert!(!m.opted_in.contains(&11));
    assert!(m.channels.contains(&11));
    let mut m = m;
    m.apply_guild_entry(1, &serde_json::json!({ "guild_id": "1", "channel_overrides": [{ "channel_id": "10", "flags": 0 }] }));
    assert!(!m.opted_in.contains(&10));
}

#[test]
fn late_sync_of_the_previous_channel_is_not_shown() {
    let (cache, r) = ready_cache();
    let mut a = Adapter::new(r.user.id, Prefs::from_ready(&r));
    let g = "300000000000000001";
    for (c, ow) in [("700000000000000081", "[]"), ("700000000000000082", r#"[{"id":"300000000000000001","type":0,"allow":"0","deny":"1024"}]"#)] {
        let raw = format!(r#"{{"id":"{c}","type":0,"guild_id":"{g}","name":"c","position":0,"permission_overwrites":{ow}}}"#);
        cache.apply(&parse_dispatch("CHANNEL_CREATE", &raw));
    }
    let sync = |list: &str, user: &str| {
        let raw = format!(
            r#"{{"guild_id":"{g}","id":"{list}","groups":[{{"id":"online","count":1}}],"ops":[{{"op":"SYNC","range":[0,99],"items":[
            {{"group":{{"id":"online"}}}},
            {{"member":{{"user":{{"id":"{user}","username":"u","discriminator":"0"}},"roles":[],"presence":{{"status":"online"}}}}}}]}}]}}"#
        );
        parse_dispatch("GUILD_MEMBER_LIST_UPDATE", &raw)
    };
    let rows_of = |out: &[AppEvent]| {
        out.iter().find_map(|x| match x {
            AppEvent::MemberListRows { rows, .. } => Some(rows.iter().filter_map(|r| match r {
                crate::presence::ListEntry::Member(m, _) => Some(m.user.0),
                _ => None,
            }).collect::<Vec<_>>()),
            _ => None,
        })
    };
    a.show_list(&cache, api::Snowflake(700000000000000082));
    assert!(a.show_list(&cache, api::Snowflake(700000000000000081)).is_empty());
    let open = a.map(&cache, &sync("everyone", "200000000000000011"));
    assert_eq!(rows_of(&open), Some(vec![200000000000000011]));
    let late = a.map(&cache, &sync("1834083668", "200000000000000012"));
    assert_eq!(rows_of(&late), None);
    let back = a.show_list(&cache, api::Snowflake(700000000000000082));
    assert_eq!(rows_of(&back), Some(vec![200000000000000012]));
}

#[test]
fn attachment_spoilers_from_flag_or_prefix() {
    let m: phoenix_api::models::Message = serde_json::from_value(serde_json::json!({
        "id": "5", "channel_id": "7", "content": "", "timestamp": "2026-11-24T15:00:00+00:00",
        "author": {"id": "1", "username": "me"},
        "attachments": [
            {"id": "1", "filename": "a.png", "size": 3, "url": "https://cdn.discordapp.com/a.png", "proxy_url": "https://media.discordapp.net/a.png", "content_type": "image/png", "flags": 8},
            {"id": "2", "filename": "SPOILER_b.png", "size": 3, "url": "https://cdn.discordapp.com/b.png", "proxy_url": "https://media.discordapp.net/b.png", "content_type": "image/png"},
            {"id": "3", "filename": "c.png", "size": 3, "url": "https://cdn.discordapp.com/c.png", "proxy_url": "https://media.discordapp.net/c.png", "content_type": "image/png"}
        ]
    }))
    .unwrap();
    let out = message(&m);
    let s: Vec<bool> = out.attachments.iter().map(|a| a.is_spoiler()).collect();
    assert_eq!(s, [true, true, false]);
}

use phoenix_api::gateway::{GatewayEvent, MemberListItem, MemberListOp, parse_dispatch};
use phoenix_api::models::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn ready_user_account_shape() {
    let GatewayEvent::Ready(r) = parse_dispatch("READY", &fixture("ready_user.json")) else { panic!("not ready") };
    assert_eq!(r.user.username, "phoenix");
    assert_eq!(r.user.display_name(), "Phoenix");
    assert_eq!(r.user.mfa_enabled, Some(true));
    assert_eq!(r.session_id, "6e1a7b5f0c9d4e2f");
    assert_eq!(r.resume_gateway_url.as_deref(), Some("wss://gateway-us-east1-b.discord.gg"));
    assert_eq!(r.users.len(), 2);
    assert_eq!(r.users[1].tag(), "bob#4242");

    let g = &r.guilds[0];
    assert_eq!(g.name(), "Rustaceans");
    assert_eq!(g.owner_id(), Some(Snowflake(200000000000000002)));
    assert!(g.lazy && g.large);
    assert_eq!(g.data_mode.as_deref(), Some("full"));
    assert_eq!(g.roles.len(), 2);
    assert_eq!(g.everyone_role().unwrap().name, "@everyone");
    assert_eq!(g.channels.len(), 8);
    assert!(g.channels.iter().all(|c| c.guild_id == Some(g.id)));
    let kinds: Vec<ChannelType> = g.channels.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        vec![
            ChannelType::GuildCategory,
            ChannelType::GuildText,
            ChannelType::GuildVoice,
            ChannelType::GuildForum,
            ChannelType::GuildAnnouncement,
            ChannelType::GuildStageVoice,
            ChannelType::GuildMedia,
            ChannelType::Unknown(99),
        ]
    );
    let general = &g.channels[1];
    assert_eq!(general.parent_id, Some(Snowflake(400000000000000000)));
    assert_eq!(general.permission_overwrites[0].deny, Permissions::SEND_MESSAGES);
    assert_eq!(general.permission_overwrites[0].kind, OverwriteType::Role);
    let forum = &g.channels[3];
    assert_eq!(forum.available_tags[0].name, "solved");
    assert_eq!(forum.default_reaction_emoji.as_ref().unwrap().emoji_name.as_deref(), Some("👍"));
    let thread = &g.threads[0];
    assert!(thread.kind.is_thread());
    assert!(!thread.thread_metadata.as_ref().unwrap().archived);
    assert_eq!(thread.applied_tags, vec![Snowflake(800000000000000001)]);

    assert!(r.guilds[1].unavailable);

    assert_eq!(r.merged_members[0][0].user_id(), Some(r.user.id));
    assert_eq!(r.merged_members[0][0].nick.as_deref(), Some("phx"));

    let dm = &r.private_channels[0];
    assert_eq!(dm.kind, ChannelType::Dm);
    assert_eq!(dm.recipient_user_ids(), vec![Snowflake(200000000000000002)]);
    assert_eq!(r.private_channels[1].kind, ChannelType::GroupDm);

    assert_eq!(r.relationships[0].kind, RelationshipType::Friend);
    assert_eq!(r.relationships[1].kind, RelationshipType::Unknown(7));
}

#[test]
fn guild_create_classic_shape() {
    let g: Guild = serde_json::from_str(&fixture("guild_create.json")).unwrap();
    assert_eq!(g.name(), "Classic Shape");
    assert_eq!(g.owner_id(), Some(Snowflake(100000000000000001)));
    assert_eq!(g.members[0].user.as_ref().unwrap().username, "phoenix");
    assert!(g.voice_states[0].self_mute);
    assert_eq!(g.channels[0].guild_id, Some(g.id));
    let again: Guild = serde_json::from_value(serde_json::to_value(&g).unwrap()).unwrap();
    assert_eq!(again, g);
}

#[test]
fn message_full() {
    let m: Message = serde_json::from_str(&fixture("message_create.json")).unwrap();
    assert_eq!(m.kind, MessageType::Reply);
    assert!(m.is_reply());
    assert_eq!(m.author.avatar_url(64), "https://cdn.discordapp.com/avatars/200000000000000003/bobhash.png?size=64");
    assert_eq!(m.member.as_ref().unwrap().nick.as_deref(), Some("Bobby"));
    assert!(m.mentions_user(Snowflake(200000000000000002)));
    assert!(m.mentions[0].member.is_some());
    assert_eq!(m.mention_roles, vec![Snowflake(500000000000000001)]);
    assert_eq!(m.nonce, Some(Nonce::Str("1234567890123456789".into())));

    assert_eq!(m.attachments.len(), 2);
    assert!(m.attachments[0].is_image());
    assert_eq!(m.attachments[0].width, Some(640));
    assert_eq!(m.attachments[1].duration_secs, Some(3.5));

    let e = &m.embeds[0];
    assert_eq!(e.kind.as_deref(), Some("rich"));
    assert_eq!(e.color, Some(14177041));
    assert_eq!(e.fields.len(), 2);
    assert!(e.fields[0].inline && !e.fields[1].inline);
    assert_eq!(e.footer.as_ref().unwrap().text, "blog");
    assert_eq!(m.embeds[1].video.as_ref().unwrap().width, Some(200));

    assert_eq!(m.reactions[0].emoji.route_form(), "🦀");
    assert_eq!(m.reactions[1].emoji.route_form(), "ferris:600000000000000001");
    assert_eq!(m.reactions[0].count_details.burst, 1);

    assert_eq!(m.sticker_items[1].format_type, StickerFormat::Lottie);
    assert!(m.sticker_items[1].url().ends_with(".json"));

    let r = m.referenced_message.as_ref().unwrap();
    assert_eq!(r.content, "original");
    assert!(r.edited_timestamp.is_some());
    assert_eq!(m.message_reference.as_ref().unwrap().message_id, Some(r.id));
}

#[test]
fn message_unknown_type_and_int_nonce() {
    let m: Message = serde_json::from_str(
        r#"{"id":"1","channel_id":"2","type":250,"nonce":42,"author":{"id":"3","username":"x"},"content":null}"#,
    )
    .unwrap();
    assert_eq!(m.kind, MessageType::Unknown(250));
    assert_eq!(m.nonce, Some(Nonce::Int(42)));
    assert_eq!(m.content, "");
}

#[test]
fn reaction_local_apply() {
    let mut m: Message = serde_json::from_str(&fixture("message_create.json")).unwrap();
    let crab = ReactionEmoji::unicode("🦀");
    m.apply_reaction(&crab, false, false, false);
    assert_eq!(m.reactions[0].count, 2);
    let new = ReactionEmoji::unicode("🔥");
    m.apply_reaction(&new, true, true, false);
    assert!(m.reactions.last().unwrap().me);
    m.apply_reaction(&new, true, false, false);
    assert!(!m.reactions.iter().any(|r| r.emoji.same_as(&new)));
}

#[test]
fn member_list_update() {
    let GatewayEvent::GuildMemberListUpdate(l) = parse_dispatch("GUILD_MEMBER_LIST_UPDATE", &fixture("member_list_update.json"))
    else {
        panic!()
    };
    assert_eq!(l.online_count, 210);
    assert_eq!(l.ops.len(), 4);
    let MemberListOp::Sync { range, items } = &l.ops[0] else { panic!() };
    assert_eq!(*range, [0, 99]);
    assert!(matches!(&items[0], MemberListItem::Group { count: 3, .. }));
    let MemberListItem::Member(mm) = &items[1] else { panic!() };
    assert_eq!(mm.member.user_id(), Some(Snowflake(200000000000000002)));
    assert_eq!(mm.presence.as_ref().unwrap()["status"], "online");
    assert!(matches!(l.ops[2], MemberListOp::Delete { index: 5 }));
}

#[test]
fn voice_events() {
    let GatewayEvent::VoiceServerUpdate(v) =
        parse_dispatch("VOICE_SERVER_UPDATE", r#"{"token":"t0k","guild_id":"1","endpoint":"c-cdg07.discord.media:443"}"#)
    else {
        panic!()
    };
    assert_eq!(v.endpoint.as_deref(), Some("c-cdg07.discord.media:443"));
    let GatewayEvent::VoiceStateUpdate(s) = parse_dispatch(
        "VOICE_STATE_UPDATE",
        r#"{"guild_id":"1","channel_id":"2","user_id":"3","session_id":"abc","deaf":false,"mute":false,"self_deaf":false,"self_mute":true,"self_video":false,"suppress":false}"#,
    ) else {
        panic!()
    };
    assert_eq!(s.session_id, "abc");
    assert!(s.self_mute);
}

#[test]
fn unknown_and_malformed_dispatch_pass_through() {
    match parse_dispatch("SOMETHING_NEW", r#"{"a":1}"#) {
        GatewayEvent::Unknown { name, data } => {
            assert_eq!(name, "SOMETHING_NEW");
            assert_eq!(data["a"], 1);
        }
        e => panic!("{e:?}"),
    }
    assert!(matches!(parse_dispatch("MESSAGE_DELETE", r#"{"nope":true}"#), GatewayEvent::Unknown { .. }));
}

#[test]
fn snowflake_time() {
    let id = Snowflake(700000000000000051);
    assert_eq!(Snowflake::from_timestamp_ms(id.timestamp_ms()).timestamp_ms(), id.timestamp_ms());
}

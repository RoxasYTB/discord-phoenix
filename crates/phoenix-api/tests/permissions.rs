use phoenix_api::models::*;

const G: Snowflake = Snowflake(1);
const ME: Snowflake = Snowflake(10);
const MUTED: Snowflake = Snowflake(20);
const HELPER: Snowflake = Snowflake(21);
const ADMIN: Snowflake = Snowflake(22);

fn role(id: Snowflake, p: Permissions) -> Role {
    Role { id, permissions: p, ..Default::default() }
}

fn roles() -> Vec<Role> {
    let base = Permissions::VIEW_CHANNEL
        | Permissions::SEND_MESSAGES
        | Permissions::READ_MESSAGE_HISTORY
        | Permissions::ADD_REACTIONS
        | Permissions::EMBED_LINKS
        | Permissions::ATTACH_FILES;
    vec![
        role(G, base),
        role(MUTED, Permissions::empty()),
        role(HELPER, Permissions::MANAGE_MESSAGES),
        role(ADMIN, Permissions::ADMINISTRATOR),
    ]
}

fn ow(id: Snowflake, kind: OverwriteType, allow: Permissions, deny: Permissions) -> PermissionOverwrite {
    PermissionOverwrite { id, kind, allow, deny }
}

#[test]
fn base_permissions() {
    let r = roles();
    let p = compute_base_permissions(G, Some(Snowflake(99)), ME, &[], &r);
    assert!(p.contains(Permissions::SEND_MESSAGES));
    assert!(!p.contains(Permissions::MANAGE_MESSAGES));
    let p = compute_base_permissions(G, Some(Snowflake(99)), ME, &[HELPER], &r);
    assert!(p.contains(Permissions::MANAGE_MESSAGES));
    assert_eq!(compute_base_permissions(G, Some(Snowflake(99)), ME, &[ADMIN], &r), Permissions::all());
    assert_eq!(compute_base_permissions(G, Some(ME), ME, &[], &r), Permissions::all());
}

#[test]
fn overwrite_order() {
    let r = roles();
    let ows = vec![
        ow(G, OverwriteType::Role, Permissions::empty(), Permissions::SEND_MESSAGES),
        ow(HELPER, OverwriteType::Role, Permissions::SEND_MESSAGES, Permissions::empty()),
        ow(MUTED, OverwriteType::Role, Permissions::empty(), Permissions::SEND_MESSAGES),
    ];
    let p = compute_channel_permissions(G, None, ME, &[], &r, &ows);
    assert!(!p.contains(Permissions::SEND_MESSAGES));
    assert!(!p.contains(Permissions::ATTACH_FILES));
    assert!(p.contains(Permissions::VIEW_CHANNEL));

    let p = compute_channel_permissions(G, None, ME, &[HELPER, MUTED], &r, &ows);
    assert!(p.contains(Permissions::SEND_MESSAGES));
    assert!(p.contains(Permissions::ATTACH_FILES));

    let mut ows2 = ows.clone();
    ows2.push(ow(ME, OverwriteType::Member, Permissions::empty(), Permissions::SEND_MESSAGES));
    let p = compute_channel_permissions(G, None, ME, &[HELPER], &r, &ows2);
    assert!(!p.contains(Permissions::SEND_MESSAGES));
}

#[test]
fn hidden_channel_and_admin_bypass() {
    let r = roles();
    let ows = vec![ow(G, OverwriteType::Role, Permissions::empty(), Permissions::VIEW_CHANNEL)];
    assert_eq!(compute_channel_permissions(G, None, ME, &[], &r, &ows), Permissions::empty());
    assert_eq!(compute_channel_permissions(G, None, ME, &[ADMIN], &r, &ows), Permissions::all());
}

#[test]
fn permission_serde() {
    let p: Permissions = serde_json::from_str("\"1071698660929\"").unwrap();
    assert_eq!(p.bits(), 1071698660929);
    assert_eq!(serde_json::to_string(&p).unwrap(), "\"1071698660929\"");
    let p: Permissions = serde_json::from_str("\"9223372036854775808\"").unwrap();
    assert_eq!(p.bits(), 1 << 63);
}

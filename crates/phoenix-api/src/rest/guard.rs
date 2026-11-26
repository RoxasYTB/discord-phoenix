use reqwest::Method;
use serde_json::{Map, Value, json};

use super::RestClient;
use crate::error::{Error, Result};

pub const CHANGE_PHONE_REASON: &str = "user_settings_update";

pub fn read_only(method: &Method) -> bool {
    *method == Method::GET
}

pub fn blocked(method: &Method, path: impl Into<String>) -> Error {
    Error::WriteBlocked { method: method.to_string(), path: path.into() }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuardedWrite {
    pub method: Method,
    pub path: String,
    pub body: Option<Value>,
}

fn write(method: Method, path: impl Into<String>, body: Option<Value>) -> GuardedWrite {
    GuardedWrite { method, path: path.into(), body }
}

fn me_patch(fields: &[(&str, Option<&str>)]) -> GuardedWrite {
    let mut body = Map::new();
    for (k, v) in fields.iter().filter_map(|(k, v)| v.map(|v| (*k, v))) {
        body.insert(k.to_owned(), Value::String(v.to_owned()));
    }
    write(Method::PATCH, "/users/@me", Some(Value::Object(body)))
}

impl GuardedWrite {
    pub fn change_password(password: &str, new_password: &str) -> Self {
        me_patch(&[("password", Some(password)), ("new_password", Some(new_password))])
    }

    pub fn change_username(username: &str, discriminator: Option<&str>, password: &str) -> Self {
        me_patch(&[("username", Some(username)), ("password", Some(password)), ("discriminator", discriminator)])
    }

    pub fn change_email(email: &str, email_token: Option<&str>, password: &str) -> Self {
        me_patch(&[("email", Some(email)), ("email_token", email_token), ("password", Some(password))])
    }

    pub fn send_email_code() -> Self {
        write(Method::PUT, "/users/@me/email", None)
    }

    pub fn verify_email_code(code: &str) -> Self {
        write(Method::POST, "/users/@me/email/verify-code", Some(json!({ "code": code })))
    }

    pub fn username_attempt(username: &str) -> Self {
        write(Method::POST, "/users/@me/pomelo-attempt", Some(json!({ "username": username })))
    }

    pub fn begin_add_phone(phone: &str) -> Self {
        write(Method::POST, "/users/@me/phone", Some(json!({ "phone": phone, "change_phone_reason": CHANGE_PHONE_REASON })))
    }

    pub fn verify_phone(phone: &str, code: &str) -> Self {
        write(Method::POST, "/phone-verifications/verify", Some(json!({ "phone": phone, "code": code })))
    }

    pub fn add_phone(phone_token: &str, password: &str) -> Self {
        write(
            Method::POST,
            "/users/@me/phone",
            Some(json!({ "phone_token": phone_token, "password": password, "change_phone_reason": CHANGE_PHONE_REASON })),
        )
    }

    pub fn remove_phone(password: &str) -> Self {
        write(Method::DELETE, "/users/@me/phone", Some(json!({ "password": password, "change_phone_reason": CHANGE_PHONE_REASON })))
    }

    pub fn disable_sms(password: &str) -> Self {
        write(Method::POST, "/users/@me/mfa/sms/disable", Some(json!({ "password": password })))
    }

    pub fn disable_totp() -> Self {
        write(Method::POST, "/users/@me/mfa/totp/disable", None)
    }

    pub fn backup_codes_challenge(password: &str) -> Self {
        write(Method::POST, "/auth/verify/view-backup-codes-challenge", Some(json!({ "password": password })))
    }

    pub fn wishlist_visibility(wishlist: &str, public: bool) -> Self {
        write(Method::PATCH, format!("/users/@me/wishlists/{wishlist}"), Some(json!({ "visibility": u8::from(public) })))
    }

    pub fn wishlist_add_sku(sku: &str) -> Self {
        write(Method::POST, "/users/@me/wishlist/items", Some(json!({ "sku_id": sku })))
    }

    pub fn crosspost(channel: u64, message: u64) -> Self {
        write(Method::POST, format!("/channels/{channel}/messages/{message}/crosspost"), None)
    }

    pub fn change_call_region(channel: u64, region: &str) -> Self {
        write(Method::PATCH, format!("/channels/{channel}/call"), Some(json!({ "region": region })))
    }

    pub fn thread_from_message(channel: u64, message: u64, name: &str) -> Self {
        write(Method::POST, format!("/channels/{channel}/messages/{message}/threads"), Some(json!({ "name": name })))
    }

    pub fn put_onboarding(guild: u64, body: Value) -> Self {
        write(Method::PUT, format!("/guilds/{guild}/onboarding"), Some(body))
    }

    pub fn put_home_settings(guild: u64, body: Value) -> Self {
        write(Method::PUT, format!("/guilds/{guild}/new-member-welcome"), Some(body))
    }

    pub fn put_channel_overwrite(channel: u64, overwrite: u64, body: Value) -> Self {
        write(Method::PUT, format!("/channels/{channel}/permissions/{overwrite}"), Some(body))
    }

    pub fn automod_alert_action(guild: u64, channel: u64, message: u64, action_type: u8) -> Self {
        write(
            Method::POST,
            format!("/guilds/{guild}/auto-moderation/alert-action"),
            Some(json!({ "message_id": message.to_string(), "channel_id": channel.to_string(), "alert_action_type": action_type })),
        )
    }

    pub fn automod_clear_mention_raid(guild: u64) -> Self {
        write(Method::POST, format!("/guilds/{guild}/auto-moderation/clear-mention-raid"), None)
    }

    pub fn edit_guild(guild: u64, body: Value) -> Self {
        write(Method::PATCH, format!("/guilds/{guild}"), Some(body))
    }

    pub fn member_verification(guild: u64, form_fields: Value, enabled: bool) -> Self {
        write(Method::PATCH, format!("/guilds/{guild}/member-verification"), Some(json!({ "form_fields": form_fields, "enabled": enabled })))
    }

    pub fn guild_profile_visibility(guild: u64, visibility: u8) -> Self {
        write(Method::PATCH, format!("/guilds/{guild}/profile"), Some(json!({ "visibility": visibility })))
    }

    pub fn discovery_metadata(guild: u64, body: Value) -> Self {
        write(Method::PATCH, format!("/guilds/{guild}/discovery-metadata"), Some(body))
    }

    pub fn join_guild(guild: u64) -> Self {
        write(Method::PUT, format!("/guilds/{guild}/members/@me"), None)
    }

    pub fn refusal(&self) -> Error {
        blocked(&self.method, self.path.clone())
    }
}

impl RestClient {
    pub async fn guarded_write(&self, w: &GuardedWrite) -> Result<Value> {
        Err(w.refusal())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_write() -> Vec<GuardedWrite> {
        vec![
            GuardedWrite::change_password("old", "new"),
            GuardedWrite::change_username("name", None, "pw"),
            GuardedWrite::change_email("a@b.c", Some("tok"), "pw"),
            GuardedWrite::send_email_code(),
            GuardedWrite::verify_email_code("123456"),
            GuardedWrite::username_attempt("name"),
            GuardedWrite::begin_add_phone("+33600000000"),
            GuardedWrite::verify_phone("+33600000000", "123456"),
            GuardedWrite::add_phone("tok", "pw"),
            GuardedWrite::remove_phone("pw"),
            GuardedWrite::disable_sms("pw"),
            GuardedWrite::disable_totp(),
            GuardedWrite::backup_codes_challenge("pw"),
            GuardedWrite::wishlist_visibility("1", true),
            GuardedWrite::wishlist_add_sku("2"),
            GuardedWrite::crosspost(3, 4),
            GuardedWrite::thread_from_message(3, 4, "t"),
            GuardedWrite::put_onboarding(5, json!({ "default_channel_ids": ["6"] })),
            GuardedWrite::put_home_settings(5, json!({ "guild_id": "5" })),
            GuardedWrite::put_channel_overwrite(6, 5, json!({ "id": "5", "type": 0 })),
            GuardedWrite::change_call_region(5, "rotterdam"),
            GuardedWrite::automod_alert_action(5, 6, 7, 3),
            GuardedWrite::automod_clear_mention_raid(5),
            GuardedWrite::edit_guild(5, json!({ "owner_configured_content_level": 3 })),
            GuardedWrite::member_verification(5, json!([]), false),
            GuardedWrite::guild_profile_visibility(5, 3),
            GuardedWrite::discovery_metadata(5, json!({ "keywords": [] })),
        ]
    }

    #[test]
    fn only_get_is_read_only() {
        assert!(read_only(&Method::GET));
        for m in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::HEAD, Method::OPTIONS] {
            assert!(!read_only(&m), "{m}");
        }
        assert!(blocked(&Method::DELETE, "/guilds/1/members/2").is_write_blocked());
    }

    #[test]
    fn bodies_match_the_client() {
        let pw = GuardedWrite::change_password("old", "new");
        assert_eq!((pw.method.clone(), pw.path.as_str()), (Method::PATCH, "/users/@me"));
        assert_eq!(pw.body, Some(json!({ "password": "old", "new_password": "new" })));
        let un = GuardedWrite::change_username("neo", None, "pw");
        assert_eq!(un.body, Some(json!({ "username": "neo", "password": "pw" })));
        let em = GuardedWrite::change_email("a@b.c", Some("tok"), "pw");
        assert_eq!(em.body, Some(json!({ "email": "a@b.c", "email_token": "tok", "password": "pw" })));
        assert_eq!(GuardedWrite::send_email_code().method, Method::PUT);
        let rm = GuardedWrite::remove_phone("pw");
        assert_eq!((rm.method, rm.body), (Method::DELETE, Some(json!({ "password": "pw", "change_phone_reason": "user_settings_update" }))));
        assert_eq!(GuardedWrite::wishlist_visibility("9", false).body, Some(json!({ "visibility": 0 })));
        assert_eq!(GuardedWrite::crosspost(3, 4).path, "/channels/3/messages/4/crosspost");
        let ob = GuardedWrite::put_onboarding(5, json!({ "prompts": [] }));
        assert_eq!((ob.method, ob.path.as_str(), ob.body), (Method::PUT, "/guilds/5/onboarding", Some(json!({ "prompts": [] }))));
        let home = GuardedWrite::put_home_settings(5, json!({ "enabled": false }));
        assert_eq!((home.method, home.path.as_str()), (Method::PUT, "/guilds/5/new-member-welcome"));
        let region = GuardedWrite::change_call_region(5, "rotterdam");
        assert_eq!((region.method, region.path.as_str(), region.body), (Method::PATCH, "/channels/5/call", Some(json!({ "region": "rotterdam" }))));
        let am = GuardedWrite::automod_alert_action(5, 6, 7, 3);
        assert_eq!(
            (am.method, am.path.as_str(), am.body),
            (Method::POST, "/guilds/5/auto-moderation/alert-action", Some(json!({ "message_id": "7", "channel_id": "6", "alert_action_type": 3 })))
        );
        assert_eq!(GuardedWrite::automod_clear_mention_raid(5).path, "/guilds/5/auto-moderation/clear-mention-raid");
        let mv = GuardedWrite::member_verification(5, json!([{ "field_type": "TERMS" }]), true);
        assert_eq!(
            (mv.method, mv.path.as_str(), mv.body),
            (Method::PATCH, "/guilds/5/member-verification", Some(json!({ "form_fields": [{ "field_type": "TERMS" }], "enabled": true })))
        );
        assert_eq!(GuardedWrite::guild_profile_visibility(5, 1).body, Some(json!({ "visibility": 1 })));
        assert_eq!(GuardedWrite::edit_guild(5, json!({})).path, "/guilds/5");
    }

    #[tokio::test]
    async fn every_write_is_refused_without_touching_the_network() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let rest = RestClient::with_base(Some("test-token".into()), crate::ClientProperties::linux_desktop(), format!("http://{addr}"));
        for w in every_write() {
            assert!(!read_only(&w.method), "{} {} is not a write", w.method, w.path);
            let err = rest.guarded_write(&w).await.unwrap_err();
            assert!(err.is_write_blocked(), "{} {} was not blocked: {err}", w.method, w.path);
        }
        let accepted = tokio::time::timeout(std::time::Duration::from_millis(200), listener.accept()).await;
        assert!(accepted.is_err(), "a guarded write opened a connection");
    }
}

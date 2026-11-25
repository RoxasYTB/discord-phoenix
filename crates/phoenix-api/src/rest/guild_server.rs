use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::guild_manage::ApiReq;
use crate::models::Snowflake;

#[derive(Debug, Clone, PartialEq)]
pub enum ServerCall {
    Guild { guild: Snowflake },
    AutomodRules { guild: Snowflake },
    CreateAutomodRule { guild: Snowflake, rule: AutomodRule },
    EditAutomodRule { guild: Snowflake, rule: AutomodRule },
    DeleteAutomodRule { guild: Snowflake, rule: Snowflake },
    Integrations { guild: Snowflake },
    DeleteIntegration { guild: Snowflake, integration: Snowflake },
    Webhooks { guild: Snowflake },
    EditWebhook { webhook: Snowflake, name: Option<String>, channel: Option<Snowflake> },
    DeleteWebhook { webhook: Snowflake },
    AppCommands { guild: Snowflake, application: Snowflake },
    CommandPermissions { guild: Snowflake, application: Snowflake },
    SetCommandPermissions { guild: Snowflake, application: Snowflake, command: Snowflake, permissions: Vec<CommandPermission> },
    VanityUrl { guild: Snowflake },
    SetVanityUrl { guild: Snowflake, code: String },
    Templates { guild: Snowflake },
    CreateTemplate { guild: Snowflake, name: String, description: String },
    SyncTemplate { guild: Snowflake, code: String },
    DeleteTemplate { guild: Snowflake, code: String },
    Onboarding { guild: Snowflake },
    SetOnboarding { guild: Snowflake, onboarding: Value },
    WelcomeScreen { guild: Snowflake },
    PremiumSubscriptions { guild: Snowflake },
    Analytics { guild: Snowflake, kind: AnalyticsKind, start: String, end: String },
    DiscoveryMetadata { guild: Snowflake },
    DiscoveryCategories { locale: String },
    DiscoverableContent { guild: Snowflake },
    ServerIndex { guild: Snowflake },
    TopGames { guild: Snowflake },
    PublicApps { ids: Vec<Snowflake> },
    EditDiscoveryMetadata { guild: Snowflake, patch: Value },
    Profile { guild: Snowflake },
    SetProfile { guild: Snowflake, patch: ProfilePatch },
    EditSafety { guild: Snowflake, edit: SafetyEdit },
    SetMfaLevel { guild: Snowflake, level: u8 },
    DeleteGuild { guild: Snowflake },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsKind {
    Engagement,
    Growth,
    Retention,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SafetyEdit {
    pub verification_level: Option<u8>,
    pub explicit_content_filter: Option<u8>,
    pub premium_progress_bar_enabled: Option<bool>,
    pub safety_alerts_channel: Option<Option<Snowflake>>,
    pub rules_channel: Option<Option<Snowflake>>,
    pub public_updates_channel: Option<Option<Snowflake>>,
    pub preferred_locale: Option<String>,
    pub description: Option<String>,
    pub features: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfilePatch {
    pub tag: Option<Option<String>>,
    pub badge: Option<u32>,
    pub badge_color_primary: Option<Option<String>>,
    pub badge_color_secondary: Option<Option<String>>,
    pub icon: Option<Option<String>>,
    pub custom_banner: Option<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandPermission {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: u8,
    pub permission: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AutomodAction {
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TriggerMetadata {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keyword_filter: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regex_patterns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_list: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_total_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_raid_protection_enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AutomodRule {
    #[serde(default, skip_serializing)]
    pub id: Option<Snowflake>,
    pub name: String,
    pub event_type: u8,
    pub trigger_type: u8,
    #[serde(default)]
    pub trigger_metadata: TriggerMetadata,
    #[serde(default)]
    pub actions: Vec<AutomodAction>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub exempt_roles: Vec<Snowflake>,
    #[serde(default)]
    pub exempt_channels: Vec<Snowflake>,
}

pub mod trigger {
    pub const KEYWORD: u8 = 1;
    pub const SPAM: u8 = 3;
    pub const KEYWORD_PRESET: u8 = 4;
    pub const MENTION_SPAM: u8 = 5;
    pub const MEMBER_PROFILE: u8 = 6;
}

pub mod action {
    pub const BLOCK_MESSAGE: u8 = 1;
    pub const SEND_ALERT: u8 = 2;
    pub const TIMEOUT: u8 = 3;
    pub const BLOCK_INTERACTION: u8 = 4;
}

impl AutomodRule {
    pub fn body(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Some(o) = v.as_object_mut() {
            o.remove("id");
        }
        v
    }

    pub fn alert_channel(&self) -> Option<Snowflake> {
        self.actions.iter().find(|a| a.kind == action::SEND_ALERT).and_then(|a| a.metadata.get("channel_id")?.as_str()?.parse().ok().map(Snowflake))
    }

    pub fn timeout_seconds(&self) -> Option<u64> {
        self.actions.iter().find(|a| a.kind == action::TIMEOUT).and_then(|a| a.metadata.get("duration_seconds")?.as_u64())
    }

    pub fn has_action(&self, kind: u8) -> bool {
        self.actions.iter().any(|a| a.kind == kind)
    }
}

pub fn parse_rules(v: &Value) -> Vec<AutomodRule> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let mut rule: AutomodRule = serde_json::from_value(r.clone()).ok()?;
            rule.id = r.get("id").and_then(|x| x.as_str()).and_then(|s| s.parse().ok()).map(Snowflake);
            Some(rule)
        })
        .collect()
}

fn req(method: Method, path: String, body: Option<Value>) -> ApiReq {
    ApiReq { method, path, body }
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b => format!("%{b:02X}"),
        })
        .collect()
}

pub fn build(call: &ServerCall) -> ApiReq {
    use ServerCall as S;
    match call {
        S::Guild { guild } => req(Method::GET, format!("/guilds/{guild}?with_counts=true"), None),
        S::AutomodRules { guild } => req(Method::GET, format!("/guilds/{guild}/auto-moderation/rules"), None),
        S::CreateAutomodRule { guild, rule } => req(Method::POST, format!("/guilds/{guild}/auto-moderation/rules"), Some(rule.body())),
        S::EditAutomodRule { guild, rule } => {
            let id = rule.id.map(|i| i.to_string()).unwrap_or_default();
            req(Method::PATCH, format!("/guilds/{guild}/auto-moderation/rules/{id}"), Some(rule.body()))
        }
        S::DeleteAutomodRule { guild, rule } => req(Method::DELETE, format!("/guilds/{guild}/auto-moderation/rules/{rule}"), None),
        S::Integrations { guild } => {
            req(Method::GET, format!("/guilds/{guild}/integrations?include_applications=true&include_role_connections_metadata=true"), None)
        }
        S::DeleteIntegration { guild, integration } => req(Method::DELETE, format!("/guilds/{guild}/integrations/{integration}"), None),
        S::Webhooks { guild } => req(Method::GET, format!("/guilds/{guild}/webhooks"), None),
        S::EditWebhook { webhook, name, channel } => {
            let mut b = json!({});
            if let Some(n) = name {
                b["name"] = json!(n.trim());
            }
            if let Some(c) = channel {
                b["channel_id"] = json!(c);
            }
            req(Method::PATCH, format!("/webhooks/{webhook}"), Some(b))
        }
        S::DeleteWebhook { webhook } => req(Method::DELETE, format!("/webhooks/{webhook}"), None),
        S::AppCommands { guild, application } => req(Method::GET, format!("/guilds/{guild}/application-commands/{application}"), None),
        S::CommandPermissions { guild, application } => {
            req(Method::GET, format!("/applications/{application}/guilds/{guild}/commands/{application}/permissions"), None)
        }
        S::SetCommandPermissions { guild, application, command, permissions } => req(
            Method::PUT,
            format!("/applications/{application}/guilds/{guild}/commands/{command}/permissions"),
            Some(json!({ "permissions": permissions })),
        ),
        S::VanityUrl { guild } => req(Method::GET, format!("/guilds/{guild}/vanity-url"), None),
        S::SetVanityUrl { guild, code } => req(Method::PATCH, format!("/guilds/{guild}/vanity-url"), Some(json!({ "code": code.trim() }))),
        S::Templates { guild } => req(Method::GET, format!("/guilds/{guild}/templates"), None),
        S::CreateTemplate { guild, name, description } => {
            req(Method::POST, format!("/guilds/{guild}/templates"), Some(json!({ "name": name.trim(), "description": description.trim() })))
        }
        S::SyncTemplate { guild, code } => req(Method::PUT, format!("/guilds/{guild}/templates/{code}"), None),
        S::DeleteTemplate { guild, code } => req(Method::DELETE, format!("/guilds/{guild}/templates/{code}"), None),
        S::Onboarding { guild } => req(Method::GET, format!("/guilds/{guild}/onboarding"), None),
        S::SetOnboarding { guild, onboarding } => req(Method::PUT, format!("/guilds/{guild}/onboarding"), Some(onboarding.clone())),
        S::WelcomeScreen { guild } => req(Method::GET, format!("/guilds/{guild}/welcome-screen"), None),
        S::PremiumSubscriptions { guild } => req(Method::GET, format!("/guilds/{guild}/premium/subscriptions"), None),
        S::Analytics { guild, kind, start, end } => {
            let path = match kind {
                AnalyticsKind::Engagement => "engagement/overview",
                AnalyticsKind::Growth => "growth-activation/overview",
                AnalyticsKind::Retention => "growth-activation/retention",
            };
            req(Method::GET, format!("/guilds/{guild}/analytics/{path}?start={}&end={}&interval=2", enc(start), enc(end)), None)
        }
        S::DiscoveryMetadata { guild } => req(Method::GET, format!("/guilds/{guild}/discovery-metadata"), None),
        S::DiscoveryCategories { locale } => req(Method::GET, format!("/discovery/categories?locale={}&primary_only=false", enc(locale)), None),
        S::DiscoverableContent { guild } => req(Method::GET, format!("/guilds/{guild}/discoverable-content"), None),
        S::ServerIndex { guild } => req(Method::GET, format!("/index/servers/{guild}"), None),
        S::TopGames { guild } => req(Method::GET, format!("/guilds/{guild}/top-games"), None),
        S::PublicApps { ids } => {
            let q: Vec<String> = ids.iter().map(|i| format!("application_ids={i}")).collect();
            req(Method::GET, format!("/applications/public?{}", q.join("&")), None)
        }
        S::EditDiscoveryMetadata { guild, patch } => req(Method::PATCH, format!("/guilds/{guild}/discovery-metadata"), Some(patch.clone())),
        S::Profile { guild } => req(Method::GET, format!("/guilds/{guild}/profile"), None),
        S::SetProfile { guild, patch } => {
            let mut b = json!({});
            if let Some(t) = &patch.tag {
                b["tag"] = json!(t);
            }
            if let Some(x) = patch.badge {
                b["badge"] = json!(x);
            }
            if let Some(x) = &patch.badge_color_primary {
                b["badge_color_primary"] = json!(x);
            }
            if let Some(x) = &patch.badge_color_secondary {
                b["badge_color_secondary"] = json!(x);
            }
            if let Some(x) = &patch.icon {
                b["icon"] = json!(x);
            }
            if let Some(x) = &patch.custom_banner {
                b["custom_banner"] = json!(x);
            }
            req(Method::PATCH, format!("/guilds/{guild}/profile"), Some(b))
        }
        S::EditSafety { guild, edit } => {
            let mut b = json!({});
            if let Some(x) = edit.verification_level {
                b["verification_level"] = json!(x);
            }
            if let Some(x) = edit.explicit_content_filter {
                b["explicit_content_filter"] = json!(x);
            }
            if let Some(x) = edit.premium_progress_bar_enabled {
                b["premium_progress_bar_enabled"] = json!(x);
            }
            if let Some(x) = edit.safety_alerts_channel {
                b["safety_alerts_channel_id"] = json!(x);
            }
            if let Some(x) = edit.rules_channel {
                b["rules_channel_id"] = json!(x);
            }
            if let Some(x) = edit.public_updates_channel {
                b["public_updates_channel_id"] = json!(x);
            }
            if let Some(x) = &edit.preferred_locale {
                b["preferred_locale"] = json!(x);
            }
            if let Some(x) = &edit.description {
                b["description"] = json!(x);
            }
            if let Some(x) = &edit.features {
                b["features"] = json!(x);
            }
            req(Method::PATCH, format!("/guilds/{guild}"), Some(b))
        }
        S::SetMfaLevel { guild, level } => req(Method::POST, format!("/guilds/{guild}/mfa"), Some(json!({ "level": level }))),
        S::DeleteGuild { guild } => req(Method::POST, format!("/guilds/{guild}/delete"), None),
    }
}

impl ServerCall {
    pub fn is_read(&self) -> bool {
        build(self).method == Method::GET
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_games_and_public_apps_are_reads() {
        let r = build(&ServerCall::TopGames { guild: Snowflake(5) });
        assert_eq!((r.method.clone(), r.path.as_str()), (Method::GET, "/guilds/5/top-games"));
        let r = build(&ServerCall::PublicApps { ids: vec![Snowflake(1), Snowflake(2)] });
        assert_eq!(r.path, "/applications/public?application_ids=1&application_ids=2");
        assert!(ServerCall::TopGames { guild: Snowflake(5) }.is_read());
    }

    const G: Snowflake = Snowflake(1);

    #[test]
    fn reads_match_the_client() {
        for (call, path) in [
            (ServerCall::AutomodRules { guild: G }, "/guilds/1/auto-moderation/rules"),
            (ServerCall::Integrations { guild: G }, "/guilds/1/integrations?include_applications=true&include_role_connections_metadata=true"),
            (ServerCall::Webhooks { guild: G }, "/guilds/1/webhooks"),
            (ServerCall::VanityUrl { guild: G }, "/guilds/1/vanity-url"),
            (ServerCall::Templates { guild: G }, "/guilds/1/templates"),
            (ServerCall::Onboarding { guild: G }, "/guilds/1/onboarding"),
            (ServerCall::DiscoveryMetadata { guild: G }, "/guilds/1/discovery-metadata"),
            (ServerCall::DiscoveryCategories { locale: "fr".into() }, "/discovery/categories?locale=fr&primary_only=false"),
            (ServerCall::PremiumSubscriptions { guild: G }, "/guilds/1/premium/subscriptions"),
            (ServerCall::Profile { guild: G }, "/guilds/1/profile"),
            (ServerCall::DiscoverableContent { guild: G }, "/guilds/1/discoverable-content"),
            (ServerCall::ServerIndex { guild: G }, "/index/servers/1"),
            (ServerCall::AppCommands { guild: G, application: Snowflake(7) }, "/guilds/1/application-commands/7"),
            (ServerCall::CommandPermissions { guild: G, application: Snowflake(7) }, "/applications/7/guilds/1/commands/7/permissions"),
        ] {
            let r = build(&call);
            assert_eq!((r.method, r.path.as_str(), r.body), (Method::GET, path, None));
            assert!(call.is_read());
        }
        let r = build(&ServerCall::Analytics { guild: G, kind: AnalyticsKind::Retention, start: "2026-10-10T02:01:56.019Z".into(), end: "2026-11-25T02:01:56.019Z".into() });
        assert_eq!(r.path, "/guilds/1/analytics/growth-activation/retention?start=2026-10-10T02%3A01%3A56.019Z&end=2026-11-25T02%3A01%3A56.019Z&interval=2");
    }

    #[test]
    fn automod_rule_round_trip() {
        let raw = json!([{ "id": "9", "guild_id": "1", "creator_id": "2", "name": "anti bot", "event_type": 1,
            "actions": [{ "type": 1, "metadata": {} }, { "type": 2, "metadata": { "channel_id": "5" } }, { "type": 3, "metadata": { "duration_seconds": 86400 } }],
            "trigger_type": 1, "enabled": true, "exempt_roles": [], "exempt_channels": ["5"],
            "trigger_metadata": { "keyword_filter": ["check bio"], "regex_patterns": ["check\\s*bio"], "allow_list": [] } }]);
        let rules = parse_rules(&raw);
        assert_eq!(rules.len(), 1);
        let r = &rules[0];
        assert_eq!(r.id, Some(Snowflake(9)));
        assert_eq!(r.alert_channel(), Some(Snowflake(5)));
        assert_eq!(r.timeout_seconds(), Some(86400));
        assert!(r.has_action(action::BLOCK_MESSAGE) && !r.has_action(action::BLOCK_INTERACTION));
        let call = build(&ServerCall::EditAutomodRule { guild: G, rule: r.clone() });
        assert_eq!((call.method, call.path.as_str()), (Method::PATCH, "/guilds/1/auto-moderation/rules/9"));
        let body = call.body.unwrap();
        assert!(body.get("id").is_none());
        assert_eq!(body["trigger_metadata"], json!({ "keyword_filter": ["check bio"], "regex_patterns": ["check\\s*bio"] }));
        assert_eq!(body["exempt_channels"], json!(["5"]));
        let call = build(&ServerCall::CreateAutomodRule { guild: G, rule: AutomodRule { name: "x".into(), event_type: 1, trigger_type: trigger::MENTION_SPAM, ..Default::default() } });
        assert_eq!((call.method, call.path.as_str()), (Method::POST, "/guilds/1/auto-moderation/rules"));
        let call = build(&ServerCall::DeleteAutomodRule { guild: G, rule: Snowflake(9) });
        assert_eq!((call.method, call.path.as_str(), call.body), (Method::DELETE, "/guilds/1/auto-moderation/rules/9", None));
    }

    #[test]
    fn writes_match_the_client() {
        let r = build(&ServerCall::SetVanityUrl { guild: G, code: " phoenix ".into() });
        assert_eq!((r.method, r.path.as_str(), r.body.unwrap()), (Method::PATCH, "/guilds/1/vanity-url", json!({ "code": "phoenix" })));
        let r = build(&ServerCall::SetMfaLevel { guild: G, level: 1 });
        assert_eq!((r.method, r.path.as_str(), r.body.unwrap()), (Method::POST, "/guilds/1/mfa", json!({ "level": 1 })));
        let r = build(&ServerCall::DeleteGuild { guild: G });
        assert_eq!((r.method, r.path.as_str(), r.body), (Method::POST, "/guilds/1/delete", None));
        let r = build(&ServerCall::CreateTemplate { guild: G, name: "Base ".into(), description: "".into() });
        assert_eq!((r.method, r.path.as_str(), r.body.unwrap()), (Method::POST, "/guilds/1/templates", json!({ "name": "Base", "description": "" })));
        let r = build(&ServerCall::SyncTemplate { guild: G, code: "abc".into() });
        assert_eq!((r.method, r.path.as_str()), (Method::PUT, "/guilds/1/templates/abc"));
        let r = build(&ServerCall::DeleteWebhook { webhook: Snowflake(3) });
        assert_eq!((r.method, r.path.as_str()), (Method::DELETE, "/webhooks/3"));
        let r = build(&ServerCall::EditWebhook { webhook: Snowflake(3), name: Some(" Hook ".into()), channel: None });
        assert_eq!(r.body.unwrap(), json!({ "name": "Hook" }));
        let r = build(&ServerCall::SetProfile { guild: G, patch: ProfilePatch { tag: Some(Some("GLAD".into())), badge: Some(8), ..Default::default() } });
        assert_eq!((r.method, r.path.as_str(), r.body.unwrap()), (Method::PATCH, "/guilds/1/profile", json!({ "tag": "GLAD", "badge": 8 })));
        let r = build(&ServerCall::EditSafety { guild: G, edit: SafetyEdit { verification_level: Some(2), safety_alerts_channel: Some(None), ..Default::default() } });
        assert_eq!((r.method, r.path.as_str(), r.body.unwrap()), (Method::PATCH, "/guilds/1", json!({ "verification_level": 2, "safety_alerts_channel_id": null })));
        let perms = vec![CommandPermission { id: Snowflake(1), kind: 1, permission: false }];
        let r = build(&ServerCall::SetCommandPermissions { guild: G, application: Snowflake(7), command: Snowflake(8), permissions: perms });
        assert_eq!((r.method, r.path.as_str()), (Method::PUT, "/applications/7/guilds/1/commands/8/permissions"));
        assert_eq!(r.body.unwrap(), json!({ "permissions": [{ "id": "1", "type": 1, "permission": false }] }));
        assert!(!ServerCall::DeleteGuild { guild: G }.is_read());
        let r = build(&ServerCall::SetProfile { guild: G, patch: ProfilePatch { icon: Some(Some("data:image/png;base64,AA".into())), custom_banner: Some(None), ..Default::default() } });
        assert_eq!(r.body, Some(json!({ "icon": "data:image/png;base64,AA", "custom_banner": null })));
        let r = build(&ServerCall::SetProfile { guild: G, patch: ProfilePatch { badge_color_primary: Some(None), badge_color_secondary: Some(None), ..Default::default() } });
        assert_eq!(r.body, Some(json!({ "badge_color_primary": null, "badge_color_secondary": null })));
        let r = build(&ServerCall::SetProfile { guild: G, patch: ProfilePatch { badge_color_primary: Some(Some("#ff1c90".into())), ..Default::default() } });
        assert_eq!(r.body, Some(json!({ "badge_color_primary": "#ff1c90" })));
    }
}

use serde_json::Value;

use crate::model::Id;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuideEmoji {
    pub id: Option<Id>,
    pub name: Option<String>,
    pub animated: bool,
}

impl GuideEmoji {
    pub fn token(&self) -> Option<String> {
        match (self.id, self.name.as_deref()) {
            (Some(id), n) => Some(format!("<{}:{}:{}>", if self.animated { "a" } else { "" }, n.unwrap_or("e"), id.0)),
            (None, Some(n)) if n.is_ascii() => crate::ui::pickers::emoji::by_shortcode(n).map(|i| crate::ui::pickers::emoji::glyph(i).to_owned()),
            (None, Some(n)) if !n.is_empty() => Some(n.to_owned()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuideEntry {
    pub channel: Id,
    pub title: String,
    pub description: String,
    pub emoji: GuideEmoji,
    pub icon: Option<String>,
    pub action_type: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuideTrait {
    pub label: String,
    pub emoji: GuideEmoji,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuildProfile {
    pub member_count: u64,
    pub online_count: u64,
    pub description: String,
    pub tag: Option<String>,
    pub badge_hash: Option<String>,
    pub traits: Vec<GuideTrait>,
    pub custom_banner_hash: Option<String>,
    pub premium_tier: u8,
    pub premium_subscription_count: u32,
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuildGuide {
    pub enabled: bool,
    pub welcome_authors: Vec<Id>,
    pub welcome_message: String,
    pub actions: Vec<GuideEntry>,
    pub resources: Vec<GuideEntry>,
    pub profile: Option<GuildProfile>,
}

fn sid(v: &Value, k: &str) -> Option<Id> {
    v.get(k).and_then(|x| x.as_str()).and_then(|x| x.parse::<u64>().ok()).map(Id)
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_owned()
}

fn emoji(v: Option<&Value>) -> GuideEmoji {
    let Some(e) = v.filter(|e| e.is_object()) else { return GuideEmoji::default() };
    GuideEmoji {
        id: sid(e, "id"),
        name: e.get("name").and_then(|x| x.as_str()).map(str::to_owned),
        animated: e.get("animated").and_then(|x| x.as_bool()).unwrap_or(false),
    }
}

fn entries(v: &Value, k: &str) -> Vec<GuideEntry> {
    v.get(k)
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    Some(GuideEntry {
                        channel: sid(r, "channel_id")?,
                        title: s(r, "title"),
                        description: s(r, "description"),
                        emoji: emoji(r.get("emoji")),
                        icon: r.get("icon").and_then(|x| x.as_str()).map(str::to_owned),
                        action_type: r.get("action_type").and_then(|x| x.as_u64()).unwrap_or(0) as u8,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_home_settings(v: &Value) -> GuildGuide {
    let welcome = v.get("welcome_message");
    GuildGuide {
        enabled: v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
        welcome_authors: welcome
            .and_then(|w| w.get("author_ids"))
            .and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()?.parse().ok().map(Id)).collect())
            .unwrap_or_default(),
        welcome_message: welcome.map(|w| s(w, "message")).unwrap_or_default(),
        actions: entries(v, "new_member_actions"),
        resources: entries(v, "resource_channels"),
        profile: None,
    }
}

pub fn parse_profile(v: &Value) -> GuildProfile {
    let n = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    let opt = |k: &str| v.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(str::to_owned);
    let mut traits: Vec<(u64, GuideTrait)> = v
        .get("traits")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|t| {
                    let e = GuideEmoji {
                        id: sid(t, "emoji_id"),
                        name: t.get("emoji_name").and_then(|x| x.as_str()).map(str::to_owned),
                        animated: t.get("emoji_animated").and_then(|x| x.as_bool()).unwrap_or(false),
                    };
                    (t.get("position").and_then(|x| x.as_u64()).unwrap_or(0), GuideTrait { label: s(t, "label"), emoji: e })
                })
                .collect()
        })
        .unwrap_or_default();
    traits.sort_by_key(|(p, _)| *p);
    GuildProfile {
        member_count: n("member_count"),
        online_count: n("online_count"),
        description: s(v, "description"),
        tag: opt("tag"),
        badge_hash: opt("badge_hash"),
        traits: traits.into_iter().map(|(_, t)| t).collect(),
        custom_banner_hash: opt("custom_banner_hash"),
        premium_tier: n("premium_tier") as u8,
        premium_subscription_count: n("premium_subscription_count") as u32,
        features: v
            .get("features")
            .and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_home_settings() {
        let v = serde_json::json!({
            "guild_id": "1", "enabled": true,
            "welcome_message": { "author_ids": ["5", "6"], "message": "Hi **all**" },
            "new_member_actions": [{ "channel_id": "7", "action_type": 1, "title": "Say hi", "description": "", "emoji": { "id": null, "name": "👋" }, "icon": null }],
            "resource_channels": [{ "channel_id": "8", "title": "Rules", "description": "Read them", "emoji": null, "icon": "abc" }]
        });
        let g = parse_home_settings(&v);
        assert_eq!(g.welcome_authors, vec![Id(5), Id(6)]);
        assert_eq!(g.actions[0].action_type, 1);
        assert_eq!(g.actions[0].emoji.token().as_deref(), Some("👋"));
        assert_eq!(g.resources[0].icon.as_deref(), Some("abc"));
        assert_eq!(g.resources[0].emoji.token(), None);
    }

    #[test]
    fn parses_profile_traits_in_order() {
        let v = serde_json::json!({
            "member_count": 1393, "online_count": 310, "description": "d", "tag": "</>", "badge_hash": "h",
            "traits": [
                { "emoji_id": null, "emoji_name": "technologist", "emoji_animated": false, "label": "Tech", "position": 1 },
                { "emoji_id": "9", "emoji_name": "pc", "emoji_animated": true, "label": "Dev", "position": 0 }
            ],
            "custom_banner_hash": "b", "premium_tier": 3, "premium_subscription_count": 31, "features": ["COMMUNITY"]
        });
        let p = parse_profile(&v);
        assert_eq!(p.online_count, 310);
        assert_eq!(p.traits[0].label, "Dev");
        assert_eq!(p.traits[0].emoji.token().as_deref(), Some("<a:pc:9>"));
        assert_eq!(p.premium_tier, 3);
    }
}

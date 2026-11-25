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
    pub id: Id,
    pub name: String,
    pub icon_hash: Option<String>,
    pub visibility: u8,
    pub member_count: u64,
    pub online_count: u64,
    pub description: String,
    pub tag: Option<String>,
    pub badge_hash: Option<String>,
    pub traits: Vec<GuideTrait>,
    pub custom_banner_hash: Option<String>,
    pub brand_color_primary: Option<String>,
    pub premium_tier: u8,
    pub premium_subscription_count: u32,
    pub features: Vec<String>,
    pub games: Vec<ProfileGame>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileGame {
    pub id: u64,
    pub name: String,
    pub icon: Option<String>,
    pub high: bool,
    pub score: u64,
}

pub fn apply_games(games: &mut Vec<ProfileGame>, fetched: &Value) {
    let found: std::collections::HashMap<u64, &Value> = fetched
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| Some((g.get("id")?.as_str()?.parse().ok()?, g)))
        .collect();
    games.retain_mut(|g| {
        let Some(v) = found.get(&g.id) else { return false };
        g.name = s(v, "name");
        let icon = v.get("media").and_then(|m| m.get("icon"));
        g.icon = match (icon.and_then(|i| i.get("type")).and_then(Value::as_str), icon.and_then(|i| i.get("value")).and_then(Value::as_str)) {
            (Some("hash"), Some(h)) if !h.is_empty() => Some(format!("https://cdn.discordapp.com/app-icons/{}/{h}.png?size=48", g.id)),
            (Some("url"), Some(u)) if !u.is_empty() => Some(u.to_owned()),
            _ => v.get("icon_hash").and_then(Value::as_str).filter(|h| !h.is_empty()).map(|h| format!("https://cdn.discordapp.com/app-icons/{}/{h}.png?size=48", g.id)),
        };
        true
    });
    games.sort_by_key(|g| std::cmp::Reverse(g.score));
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
        id: sid(v, "id").unwrap_or_default(),
        name: s(v, "name"),
        icon_hash: opt("icon_hash"),
        visibility: n("visibility") as u8,
        member_count: n("member_count"),
        online_count: n("online_count"),
        description: s(v, "description"),
        tag: opt("tag"),
        badge_hash: opt("badge_hash"),
        traits: traits.into_iter().map(|(_, t)| t).collect(),
        custom_banner_hash: opt("custom_banner_hash"),
        brand_color_primary: opt("brand_color_primary"),
        premium_tier: n("premium_tier") as u8,
        premium_subscription_count: n("premium_subscription_count") as u32,
        features: v
            .get("features")
            .and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
        games: v
            .get("game_application_ids")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|x| x.as_str()?.parse::<u64>().ok())
            .map(|id| {
                let act = v.get("game_activity").and_then(|a| a.get(id.to_string()));
                ProfileGame {
                    id,
                    high: act.and_then(|a| a.get("activity_level")).and_then(Value::as_u64) == Some(2),
                    score: act.and_then(|a| a.get("activity_score")).and_then(Value::as_u64).unwrap_or(0),
                    ..Default::default()
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_games_follow_activity_and_the_games_store() {
        let mut p = parse_profile(&serde_json::json!({
            "game_application_ids": ["1", "2", "3"],
            "game_activity": { "2": { "activity_level": 2, "activity_score": 50 } }
        }));
        assert_eq!(p.games.iter().map(|g| g.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        apply_games(&mut p.games, &serde_json::json!([
            { "id": "1", "name": "A", "media": { "icon": { "type": "hash", "value": "ab" } } },
            { "id": "2", "name": "B", "icon_hash": "cd" }
        ]));
        assert_eq!(p.games.iter().map(|g| (g.id, g.high)).collect::<Vec<_>>(), vec![(2, true), (1, false)]);
        assert_eq!(p.games[1].icon.as_deref(), Some("https://cdn.discordapp.com/app-icons/1/ab.png?size=48"));
        assert_eq!(p.games[0].icon.as_deref(), Some("https://cdn.discordapp.com/app-icons/2/cd.png?size=48"));
    }

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

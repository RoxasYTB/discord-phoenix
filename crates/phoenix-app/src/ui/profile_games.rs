use gpui::SharedString;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Favorite,
    Current,
    WantToPlay,
    Played,
}

impl Kind {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "favorite_games" => Self::Favorite,
            "current_games" => Self::Current,
            "want_to_play_games" => Self::WantToPlay,
            "played_games" => Self::Played,
            _ => return None,
        })
    }
    pub fn cap(self) -> usize {
        match self {
            Self::Favorite => 1,
            Self::Current => 5,
            _ => 20,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Game {
    pub id: u64,
    pub name: SharedString,
    pub cover: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameWidget {
    pub kind: Kind,
    pub games: Vec<Game>,
}

pub fn widget_ids(widgets: &[Value]) -> Vec<(Kind, Vec<u64>)> {
    widgets
        .iter()
        .filter_map(|w| {
            let d = w.get("data")?;
            let kind = Kind::parse(d.get("type")?.as_str()?)?;
            let ids = d
                .get("games")?
                .as_array()?
                .iter()
                .filter_map(|g| g.get("game_id")?.as_str()?.parse().ok())
                .take(kind.cap())
                .collect();
            Some((kind, ids))
        })
        .collect()
}

pub fn parse_game(id: u64, v: &Value) -> Game {
    let cover = v
        .get("media")
        .and_then(|m| m.get("cover_url").or_else(|| m.get("cover").and_then(|c| c.get("value"))))
        .and_then(Value::as_str)
        .filter(|u| !u.is_empty())
        .map(|u| if u.starts_with("//") { format!("https:{u}") } else { u.to_owned() })
        .map(Into::into);
    let name = v.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
    Game { id, name: name.into(), cover }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn reads_game_widgets_in_order_and_caps() {
        let w = json!([
            {"data": {"type": "application", "application_id": "1"}},
            {"data": {"type": "favorite_games", "games": [{"game_id": "5"}, {"game_id": "6"}]}},
            {"data": {"type": "played_games", "games": [{"game_id": "7"}]}}
        ]);
        let v = super::widget_ids(w.as_array().unwrap());
        assert_eq!(v, vec![(super::Kind::Favorite, vec![5]), (super::Kind::Played, vec![7])]);
        let g = super::parse_game(5, &json!({"name": "P", "media": {"cover_url": "https://x/c.png"}}));
        assert_eq!((g.name.as_ref(), g.cover.as_deref()), ("P", Some("https://x/c.png")));
    }
}

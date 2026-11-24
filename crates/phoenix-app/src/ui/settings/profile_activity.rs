use gpui::{Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, img};
use serde_json::Value;

use crate::theme::z;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecentGame {
    pub app_id: u64,
    pub name: SharedString,
    pub icon: Option<SharedString>,
    pub ended_at: i64,
    pub created: i64,
    pub first_time: bool,
    pub streak_days: Option<u64>,
}

pub fn parse_outbox(v: &Value) -> Vec<RecentGame> {
    v.get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let x = e.get("extra")?;
            (x.get("type")?.as_str()? == "played_game_extra").then_some(())?;
            let traits = e.get("traits").and_then(Value::as_array).cloned().unwrap_or_default();
            let t = |n: u64| traits.iter().find(|t| t.get("type").and_then(Value::as_u64) == Some(n));
            Some(RecentGame {
                app_id: x.get("application_id").and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0),
                name: x.get("game_name")?.as_str()?.to_owned().into(),
                icon: None,
                created: e.get("id").and_then(Value::as_str).and_then(|s| s.parse().ok()).map(crate::presence::snowflake_time).unwrap_or(0),
                ended_at: e
                    .get("ended_at")
                    .and_then(Value::as_str)
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.timestamp())
                    .unwrap_or(0),
                first_time: t(1).and_then(|t| t.get("first_time")).and_then(Value::as_bool).unwrap_or(false),
                streak_days: t(8).and_then(|t| t.get("streak_count_days")).and_then(Value::as_u64).filter(|d| *d >= 3),
            })
        })
        .collect()
}

pub fn apply_icons(games: &mut [RecentGame], apps: &Value) {
    for a in apps.as_array().into_iter().flatten() {
        let (Some(id), Some(icon)) = (a.get("id").and_then(Value::as_str).and_then(|s| s.parse::<u64>().ok()), a.get("icon").and_then(Value::as_str)) else {
            continue;
        };
        for g in games.iter_mut().filter(|g| g.app_id == id) {
            g.icon = Some(format!("https://cdn.discordapp.com/app-icons/{id}/{icon}.png?size=128").into());
        }
    }
}

pub fn ago(created: i64, now: i64) -> SharedString {
    let d = now - created;
    let a = d.abs();
    let r = |unit: i64| (d as f64 / unit as f64).round() as i64;
    let (n, key) = if a < 60 {
        (d, "time.seconds_ago_short")
    } else if a < 3600 {
        (r(60), "time.minutes_ago_short")
    } else if a < 12 * 3600 {
        (r(3600), "time.hours_ago_short")
    } else if a < 9 * 86_400 {
        (r(86_400), "time.days_ago_short")
    } else if a < 28 * 86_400 {
        (r(7 * 86_400), "time.weeks_ago_short")
    } else {
        (r(30 * 86_400), "time.months_ago_short")
    };
    crate::t!(key, n = n)
}

pub struct Colors {
    pub strong: Hsla,
    pub muted: Hsla,
    pub card: Hsla,
    pub link: Hsla,
}

fn badge(icon: &str, label: SharedString, col: &Colors) -> Div {
    div()
        .flex()
        .items_center()
        .gap(z(4.))
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(12.)).text_color(col.strong))
        .child(div().text_size(z(12.)).line_height(z(16.)).text_color(col.muted).child(label))
}

fn card(g: &RecentGame, now: i64, col: &Colors) -> Div {
    let mut badges = vec![badge("game-controller", ago(g.created, now), col)];
    if g.first_time {
        badges.push(badge("medal", crate::t!("pa.new_player"), col));
    }
    if let Some(d) = g.streak_days.filter(|_| now - g.created < 48 * 3600) {
        badges.push(badge("streak", crate::t!("pa.streak", n = d), col));
    }
    div()
        .relative()
        .p(z(12.))
        .rounded(z(12.))
        .bg(col.card)
        .flex()
        .gap(z(12.))
        .child(match &g.icon {
            Some(u) => img(u.clone()).size(z(60.)).rounded(z(8.)).flex_none().into_any_element(),
            None => div().size(z(60.)).rounded(z(8.)).flex_none().bg(col.card).into_any_element(),
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .mr(z(24.))
                .my_auto()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(col.strong).child(g.name.clone()))
                .child(div().flex().flex_wrap().gap_x(z(8.)).children(badges)),
        )
        .child(
            div()
                .absolute()
                .top(z(12.))
                .right(z(16.))
                .child(gpui::svg().path(crate::icons::path("more-horizontal")).size(z(16.)).text_color(col.strong)),
        )
}

pub fn render(games: &[RecentGame], col: &Colors) -> Div {
    let now = chrono::Utc::now().timestamp();
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(col.muted).child(crate::t!("pa.recent")))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x(z(4.))
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .text_color(col.strong)
                        .child(crate::t!("pa.recent_hint"))
                        .child(div().text_color(col.link).child(crate::t!("pa.learn_more"))),
                ),
        )
        .child(div().flex().flex_col().gap(z(8.)).children(games.iter().map(|g| card(g, now, col))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_played_games_and_traits() {
        let v = json!({"entries": [
            {"traits": [{"type": 1, "first_time": true}, {"type": 8, "streak_count_days": 4}],
             "extra": {"type": "played_game_extra", "game_name": "G", "application_id": "5"},
             "ended_at": "2026-11-22T19:10:32.461000+00:00"},
            {"extra": {"type": "listened_media_extra"}}
        ]});
        let mut g = parse_outbox(&v);
        assert_eq!(g.len(), 1);
        assert!(g[0].first_time);
        assert_eq!(g[0].streak_days, Some(4));
        apply_icons(&mut g, &json!([{"id": "5", "icon": "abc"}]));
        assert_eq!(g[0].icon.as_deref(), Some("https://cdn.discordapp.com/app-icons/5/abc.png?size=128"));
        assert_eq!(ago(0, 2 * 86_400 + 5), crate::t!("time.days_ago_short", n = 2));
        assert_eq!(ago(0, 19 * 3600), crate::t!("time.days_ago_short", n = 1));
        assert_eq!(ago(0, 11 * 3600 + 1900), crate::t!("time.hours_ago_short", n = 12));
        assert_eq!(ago(0, 10 * 86_400), crate::t!("time.weeks_ago_short", n = 1));
    }
}

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*};
use phoenix_api::rest::guild_server::{AnalyticsKind, ServerCall};
use serde_json::Value;

use super::{fetch, page, sf, t, text, title};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button},
};

pub fn weekly(v: &Value, key: &str) -> Option<(f64, Option<f64>)> {
    let a = v.as_array()?;
    let get = |i: usize| a.get(i).and_then(|x| x.get(key)).and_then(Value::as_f64);
    let cur = get(0)?;
    let prev = get(1);
    Some((cur, prev.filter(|p| *p != 0.).map(|p| (cur - p) / p * 100.)))
}

pub fn fr_number(x: f64, decimals: usize) -> String {
    format!("{x:.decimals$}").replace('.', ",")
}

pub fn window_at(end: chrono::DateTime<chrono::Utc>) -> (String, String) {
    let start = end - chrono::Duration::days(46);
    let f = |d: chrono::DateTime<chrono::Utc>| d.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    (f(start), f(end))
}

fn window(guild: Id) -> (String, String) {
    let key = (guild, "analytics-window".to_owned());
    if let Some((a, b)) = super::with(|s| s.text.get(&key).and_then(|v| v.split_once('|').map(|(a, b)| (a.to_owned(), b.to_owned())))) {
        return (a, b);
    }
    let (a, b) = window_at(chrono::Utc::now());
    super::with(|s| s.text.insert(key, format!("{a}|{b}")));
    (a, b)
}

fn trend(delta: Option<f64>) -> gpui::Div {
    let d = delta.unwrap_or(0.);
    let arrow = if d > 0.05 {
        Some(("chevron-up", idx::TEXT_FEEDBACK_POSITIVE))
    } else if d < -0.05 {
        Some(("chevron", idx::TEXT_FEEDBACK_CRITICAL))
    } else {
        None
    };
    div()
        .mt(z(4.))
        .flex()
        .items_center()
        .gap(z(5.))
        .children(arrow.map(|(n, c)| super::icon(n, 14., c)))
        .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.since_last_week", pct = fr_number(d.abs(), if d.abs().fract().abs() < f64::EPSILON { 0 } else { 1 }))))
}

fn stat(title_key: &str, value: String, delta: Option<f64>, info: bool, left: bool) -> gpui::Div {
    div()
        .flex_1()
        .flex_basis(z(0.))
        .min_w_0()
        .mt(z(16.))
        .when(!left, |d| d.ml(z(16.)))
        .p(z(16.))
        .rounded(z(12.))
        .bg(t(idx::BACKGROUND_SURFACE_HIGHER))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .flex()
        .flex_col()
        .child(div().flex().items_center().gap(z(6.)).child(text(16., 20., FontWeight::MEDIUM, idx::TEXT_STRONG, crate::t!(title_key))).when(info, |d| d.child(super::icon("info", 16., idx::ICON_DEFAULT))))
        .child(div().mt(z(8.)).child(text(20., 24., FontWeight::SEMIBOLD, idx::TEXT_STRONG, value)))
        .child(trend(delta))
}

pub fn render(app: &PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (start, end) = window(guild);
    let call = |kind| ServerCall::Analytics { guild: sf(guild), kind, start: start.clone(), end: end.clone() };
    let eng = fetch(app, guild, call(AnalyticsKind::Engagement), cx);
    let growth = fetch(app, guild, call(AnalyticsKind::Growth), cx);
    let ret = fetch(app, guild, call(AnalyticsKind::Retention), cx);
    let mut p = page()
        .child(title(crate::t!("menu.gs.analytics")))
        .child(div().mt(z(8.)).child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.analytics_lead"))))
        .child(div().mt(z(14.)).child(text(16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.analytics_lead2"))))
        .child(
            div()
                .mt(z(56.))
                .flex()
                .flex_col()
                .items_center()
                .gap(z(20.))
                .child(div().w(z(350.)).text_center().child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, crate::t!("gs2.analytics_external"))))
                .child(button("gs2-analytics-open", crate::t!("gs2.analytics_open"), BtnKind::Primary)),
        );
    let (Some(eng), Some(growth), Some(ret)) = (eng, growth, ret) else { return p.child(super::loading()).into_any_element() };
    let v = weekly(&eng, "visitors");
    let c = weekly(&eng, "communicators");
    let n = weekly(&growth, "new_members");
    let r = weekly(&ret, "pct_retained");
    let num = |x: Option<(f64, Option<f64>)>| x.map(|(v, _)| fr_number(v, 0)).unwrap_or_else(|| "-".into());
    p = p.child(div().mt(z(46.)).flex().child(stat("gs2.weekly_visitors", num(v), v.and_then(|x| x.1), true, true)).child(stat("gs2.weekly_communicators", num(c), c.and_then(|x| x.1), true, false))).child(
        div()
            .flex()
            .child(stat("gs2.weekly_new_members", num(n), n.and_then(|x| x.1), false, true))
            .child(stat("gs2.weekly_retention", r.map(|(v, _)| format!("{}%", fr_number(v, 1))).unwrap_or_else(|| "-".into()), r.and_then(|x| x.1), true, false)),
    );
    p.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn weekly_matches_the_client() {
        let eng = json!([{ "visitors": 240, "communicators": 40 }, { "visitors": 290, "communicators": 34 }]);
        let (v, d) = weekly(&eng, "visitors").unwrap();
        assert_eq!(v, 240.);
        assert_eq!(fr_number(d.unwrap().abs(), 1), "17,2");
        assert_eq!(fr_number(weekly(&eng, "communicators").unwrap().1.unwrap(), 1), "17,6");
        let ret = json!([{ "pct_retained": 24.137931034482758 }, { "pct_retained": 40.0 }]);
        let (r, d) = weekly(&ret, "pct_retained").unwrap();
        assert_eq!(fr_number(r, 1), "24,1");
        assert_eq!(fr_number(d.unwrap().abs(), 1), "39,7");
    }

    #[test]
    fn the_window_is_fixed_once_per_guild() {
        let a = window(Id(77));
        std::thread::sleep(std::time::Duration::from_millis(3));
        assert_eq!(window(Id(77)), a);
        let end = chrono::DateTime::parse_from_rfc3339("2026-11-25T02:01:56.019Z").unwrap().with_timezone(&chrono::Utc);
        assert_eq!(window_at(end), ("2026-10-10T02:01:56.019Z".to_owned(), "2026-11-25T02:01:56.019Z".to_owned()));
    }
}

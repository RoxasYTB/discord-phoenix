use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use serde_json::Value;

use crate::app::PhoenixApp;
use crate::model::Id;
use crate::theme::{generated::idx, z};
use crate::ui::call_view::t;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppInfo {
    pub loading: bool,
    pub name: Option<SharedString>,
    pub icon: Option<SharedString>,
    pub splash: Option<SharedString>,
}

thread_local! {
    static INFO: RefCell<HashMap<u64, AppInfo>> = RefCell::new(HashMap::new());
}

pub fn channel_activities(app: &PhoenixApp, ch: Id) -> Vec<(u64, Vec<u64>)> {
    let Some(guild) = app.channels.get(&ch).and_then(|c| c.guild) else { return Vec::new() };
    let me = app.me.id.0;
    let mut out: Vec<(u64, Vec<u64>)> = crate::guild_live::get(guild.0)
        .activities
        .into_iter()
        .filter(|(_, a)| a.channel == Some(ch.0) && !a.users.contains(&me))
        .map(|((_, app_id), a)| (app_id, a.users))
        .collect();
    out.sort_by_key(|(id, _)| *id);
    out
}

pub fn info(app_id: u64) -> AppInfo {
    INFO.with(|m| m.borrow().get(&app_id).cloned()).unwrap_or_default()
}

pub fn parse_public(app_id: u64, v: &Value) -> (Option<SharedString>, Option<SharedString>) {
    let name = v.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| SharedString::from(s.to_owned()));
    let icon = v
        .get("icon")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|h| SharedString::from(format!("https://cdn.discordapp.com/app-icons/{app_id}/{h}.png?size=32")));
    (name, icon)
}

pub fn parse_splash(app_id: u64, v: &Value) -> Option<SharedString> {
    let id = v.as_array()?.iter().find(|a| a.get("name").and_then(Value::as_str) == Some("embedded_background"))?.get("id")?.as_str()?;
    (!id.is_empty()).then(|| SharedString::from(format!("https://cdn.discordapp.com/app-assets/{app_id}/{id}.png?size=1024")))
}

pub fn ensure(app: &PhoenixApp, app_id: u64, cx: &mut gpui::App) {
    let fresh = INFO.with(|m| {
        let mut m = m.borrow_mut();
        if m.contains_key(&app_id) {
            return false;
        }
        m.insert(app_id, AppInfo { loading: true, ..Default::default() });
        true
    });
    if !fresh {
        return;
    }
    let public = app.backend.directory_get(format!("/applications/{app_id}/public"));
    let assets = app.backend.directory_get(format!("/oauth2/applications/{app_id}/assets"));
    cx.spawn(async move |cx| {
        let (name, icon) = public.await.map(|v| parse_public(app_id, &v)).unwrap_or_default();
        let splash = assets.await.ok().and_then(|v| parse_splash(app_id, &v));
        cx.update(|cx| {
            INFO.with(|m| m.borrow_mut().insert(app_id, AppInfo { loading: false, name, icon, splash }));
            cx.refresh_windows();
        });
    })
    .detach();
}

pub fn size_class(w: f32) -> u8 {
    if w > 400. {
        2
    } else if w > 300. {
        1
    } else {
        0
    }
}

pub fn summary_len(users: usize) -> (usize, Option<usize>) {
    if users <= 4 { (users, None) } else { (3, Some(users - 3)) }
}

fn summary(app: &PhoenixApp, users: &[u64], size: f32) -> gpui::Div {
    let zoom = crate::theme::zoom();
    let (shown, more) = summary_len(users.len());
    let overlap = match size as u32 {
        56 => 14.,
        32 => 8.,
        _ => 6.,
    };
    let last = shown.saturating_sub(1);
    let mut row = div().flex().items_center();
    for (i, u) in users.iter().take(shown).enumerate() {
        let masked = i < last || more.is_some();
        row = row.child(div().flex_none().size(z(size)).when(masked, |d| d.mr(z(-overlap))).child(crate::ui::avatar(app.users.get(&Id(*u)), size * zoom)));
    }
    if let Some(n) = more {
        let (font, radius) = if size >= 56. { (24., 28.) } else { (12., size / 2.) };
        row = row.child(
            div()
                .flex_none()
                .h(z(size))
                .min_w(z(size))
                .px(z(8.))
                .rounded(z(radius))
                .bg(t(idx::BACKGROUND_BASE_LOWEST))
                .flex()
                .items_center()
                .justify_center()
                .text_size(z(font))
                .line_height(z(font))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t(idx::TEXT_DEFAULT))
                .child(format!("+{n}")),
        );
    }
    row
}

pub fn subheader(names: &[SharedString]) -> Option<SharedString> {
    let first = names.first()?;
    Some(match names.len() {
        1 => crate::t!("call.activity_playing_one", username = first.as_ref()),
        2 => crate::t!("call.activity_playing_two", username = first.as_ref(), count = 1),
        n => crate::t!("call.activity_playing_many", username = first.as_ref(), count = n - 1),
    })
}

pub fn tile(app: &PhoenixApp, app_id: u64, w: f32, h: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    ensure(app, app_id, cx);
    let users = app
        .voice_channel
        .and_then(|ch| channel_activities(app, ch).into_iter().find(|(a, _)| *a == app_id).map(|(_, u)| u))
        .unwrap_or_default();
    let names: Vec<SharedString> = users
        .iter()
        .filter(|u| app.users.contains_key(&Id(**u)))
        .map(|u| crate::ui::sidebar::voice_user_name(app, Id(*u)))
        .collect();
    let shown: Vec<u64> = users.iter().copied().filter(|u| app.users.contains_key(&Id(*u))).collect();
    let info = info(app_id);
    let p = size_class(w);
    let avatar = match p {
        2 => 56.,
        1 => 32.,
        _ => 24.,
    };
    let (sub_size, head_size) = match p {
        2 => ((14., 18.), (32., 40.)),
        1 => ((12., 16.), (20., 24.)),
        _ => ((0., 0.), (14., 18.)),
    };
    let label = crate::t!("call.join_activity");
    let button = div()
        .id(SharedString::from(format!("call-activity-join-{app_id}")))
        .flex_none()
        .rounded(z(8.))
        .border_1()
        .border_color(t(idx::CONTROL_OVERLAY_PRIMARY_BORDER_DEFAULT))
        .bg(t(idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_DEFAULT))
        .hover(|s| s.bg(t(idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_HOVER)))
        .text_color(t(idx::CONTROL_OVERLAY_PRIMARY_TEXT_DEFAULT))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .font_weight(FontWeight::MEDIUM)
        .map(|d| {
            if p == 2 {
                d.min_w(z(100.)).min_h(z(38.)).px(z(15.)).py(z(7.)).text_size(z(16.)).line_height(z(20.))
            } else {
                d.min_w(z(60.)).min_h(z(30.)).px(z(11.)).py(z(3.)).text_size(z(14.)).line_height(z(18.))
            }
        })
        .child(label.clone())
        .on_click(cx.listener(move |a, _, _, cx| {
            cx.stop_propagation();
            a.stub(&label, cx);
        }));
    let splash = div()
        .absolute()
        .inset_0()
        .bg(t(idx::OPACITY_BLACK_60))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(summary(app, &shown, avatar))
        .when(p > 0, |d| {
            d.children(subheader(&names).map(|s| {
                div()
                    .mt(z(4.))
                    .text_size(z(sub_size.0))
                    .line_height(z(sub_size.1))
                    .text_color(t(idx::TEXT_DEFAULT))
                    .child(s)
            }))
        })
        .children(info.name.clone().map(|n| {
            div()
                .my(z(4.))
                .text_size(z(head_size.0))
                .line_height(z(head_size.1))
                .font_weight(FontWeight::BOLD)
                .text_color(t(idx::WHITE))
                .child(n)
        }))
        .child(div().mt(z(4.)).flex().child(button));
    div()
        .id(SharedString::from(format!("call-activity-{app_id}")))
        .relative()
        .w(z(w))
        .h(z(h))
        .flex_none()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(t(idx::BACKGROUND_BASE_LOWEST))
        .children(info.splash.clone().map(|u| gpui::img(u).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover)))
        .child(splash)
        .into_any_element()
}

pub fn playing_app(app: &PhoenixApp, user: Id) -> Option<u64> {
    app.people
        .activities
        .get(&user)?
        .iter()
        .find(|a| a.kind == crate::presence::ActivityKind::Playing)?
        .application_id
        .as_ref()?
        .parse()
        .ok()
}

pub fn badge_shown(width: f32) -> (bool, bool) {
    (width > 160., width > 220.)
}

pub fn badge(app: &PhoenixApp, user: Id, width: f32, cx: &mut gpui::App) -> Option<AnyElement> {
    let (shown, with_name) = badge_shown(width);
    if !shown {
        return None;
    }
    let app_id = playing_app(app, user)?;
    ensure(app, app_id, cx);
    let info = info(app_id);
    let icon = info.icon.clone()?;
    let name = info.name.clone();
    Some(
        div()
            .flex()
            .flex_initial()
            .items_center()
            .gap(z(4.))
            .min_w(z(20.))
            .min_h(z(20.))
            .when(with_name, |d| d.px(z(8.)).py(z(6.)))
            .when(!with_name, |d| d.p(z(6.)))
            .rounded(z(8.))
            .bg(t(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT))
            .text_color(t(idx::CONTROL_OVERLAY_SECONDARY_TEXT_DEFAULT))
            .text_size(z(14.))
            .line_height(z(18.))
            .child(
                div()
                    .relative()
                    .flex_none()
                    .size(z(20.))
                    .rounded(z(4.))
                    .overflow_hidden()
                    .child(gpui::img(icon).size_full())
                    .child(div().absolute().inset_0().rounded(z(4.)).border_1().border_color(t(idx::BORDER_SUBTLE))),
            )
            .when(with_name, |d| d.children(name.map(|n| div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(n))))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn splash_uses_the_embedded_background_asset() {
        let v = json!([{ "id": "1", "name": "embedded_cover", "type": 1 }, { "id": "2", "name": "embedded_background", "type": 1 }]);
        assert_eq!(parse_splash(9, &v).as_deref(), Some("https://cdn.discordapp.com/app-assets/9/2.png?size=1024"));
        assert_eq!(parse_splash(9, &json!([{ "id": "1", "name": "embedded_cover" }])), None);
    }

    #[test]
    fn public_app_gives_name_and_icon() {
        let (name, icon) = parse_public(5, &json!({ "id": "5", "name": "Poker", "icon": "ab" }));
        assert_eq!(name.as_deref(), Some("Poker"));
        assert_eq!(icon.as_deref(), Some("https://cdn.discordapp.com/app-icons/5/ab.png?size=32"));
        assert_eq!(parse_public(5, &json!({ "name": "", "icon": null })), (None, None));
    }

    #[test]
    fn tile_sizes_follow_the_client_breakpoints() {
        assert_eq!(size_class(401.), 2);
        assert_eq!(size_class(400.), 1);
        assert_eq!(size_class(301.), 1);
        assert_eq!(size_class(300.), 0);
        assert_eq!(summary_len(4), (4, None));
        assert_eq!(summary_len(6), (3, Some(3)));
        assert_eq!(badge_shown(160.), (false, false));
        assert_eq!(badge_shown(200.), (true, false));
        assert_eq!(badge_shown(221.), (true, true));
    }
}

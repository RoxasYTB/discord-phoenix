use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use parking_lot::RwLock;
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkedAccount {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub verified: bool,
    pub visibility: u8,
    pub show_activity: bool,
    pub friend_sync: bool,
    pub metadata_visibility: u8,
}

static LIST: RwLock<Option<Vec<LinkedAccount>>> = RwLock::new(None);

pub fn get() -> Option<Vec<LinkedAccount>> {
    LIST.read().clone()
}

pub fn set(v: Vec<LinkedAccount>) {
    super::account::set_connections(v.len());
    *LIST.write() = Some(v);
}

pub fn parse(v: &Value) -> Vec<LinkedAccount> {
    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let b = |o: &Value, k: &str| o.get(k).and_then(Value::as_bool).unwrap_or(false);
    let u = |o: &Value, k: &str| o.get(k).and_then(Value::as_u64).unwrap_or(0) as u8;
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|o| {
            let kind = s(o, "type");
            let name = s(o, "name");
            if kind.is_empty() && name.is_empty() {
                return None;
            }
            Some(LinkedAccount {
                id: s(o, "id"),
                kind,
                name,
                verified: b(o, "verified"),
                visibility: u(o, "visibility"),
                show_activity: b(o, "show_activity"),
                friend_sync: b(o, "friend_sync"),
                metadata_visibility: u(o, "metadata_visibility"),
            })
        })
        .collect()
}

pub fn fetch(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if LIST.read().is_some() {
        return;
    }
    let fut = app.backend.user_connections();
    cx.spawn(async move |this, cx| {
        if let Ok(Some(v)) = fut.await {
            set(v);
            let _ = this.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

pub fn platform_kind(label: &str) -> &'static str {
    match label {
        "Riot Games" | "League of Legends" => "riotgames",
        "Battle.net" => "battlenet",
        "Bluesky" => "bluesky",
        "Letterboxd" => "letterboxd",
        "PayPal" => "paypal",
        "Reddit" => "reddit",
        "Steam" => "steam",
        "X" => "twitter",
        "eBay" => "ebay",
        "Crunchyroll" => "crunchyroll",
        "PlayStation Network" => "playstation",
        "Spotify" => "spotify",
        "Xbox" => "xbox",
        "Amazon Music" => "amazon-music",
        "Bungie.net" => "bungie",
        "Domain" => "domain",
        "Epic Games" => "epicgames",
        "Facebook" => "facebook",
        "GitHub" => "github",
        "Roblox" => "roblox",
        "Twitch" => "twitch",
        "YouTube" => "youtube",
        _ => "",
    }
}

pub fn own_flow(kind: &str) -> bool {
    matches!(kind, "crunchyroll" | "xbox" | "playstation" | "playstation-stg" | "domain" | "bluesky" | "mastodon")
}

pub fn authorize_path(kind: &str) -> Option<String> {
    let kind = if kind == "leagueoflegends" { "riotgames" } else { kind };
    (!kind.is_empty() && !own_flow(kind)).then(|| format!("/connections/{kind}/authorize?"))
}

pub fn connect(app: &mut PhoenixApp, kind: &str, cx: &mut Context<PhoenixApp>) {
    let Some(path) = authorize_path(kind) else {
        app.parity_toast(crate::t!("sp.cn.own_flow"), cx);
        return;
    };
    let fut = app.backend.connection_authorize(path);
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |a, cx| match res {
            Ok(Some(url)) => cx.open_url(&url),
            _ => a.parity_toast(crate::t!("sp.cn.connect_error"), cx),
        });
    })
    .detach();
}

pub fn platform_label(kind: &str) -> &'static str {
    match kind {
        "riotgames" => "Riot Games",
        "battlenet" => "Battle.net",
        "bluesky" => "Bluesky",
        "letterboxd" => "Letterboxd",
        "paypal" => "PayPal",
        "reddit" => "Reddit",
        "steam" => "Steam",
        "twitter" | "x" => "X",
        "ebay" => "eBay",
        "crunchyroll" => "Crunchyroll",
        "playstation" => "PlayStation Network",
        "spotify" => "Spotify",
        "xbox" => "Xbox",
        "amazonmusic" | "amazon-music" => "Amazon Music",
        "bungie" => "Bungie.net",
        "domain" => "Domain",
        "epicgames" => "Epic Games",
        "facebook" => "Facebook",
        "github" => "GitHub",
        "roblox" => "Roblox",
        "twitch" => "Twitch",
        "youtube" => "YouTube",
        "instagram" => "Instagram",
        "tiktok" => "TikTok",
        "leagueoflegends" => "League of Legends",
        "skype" => "Skype",
        _ => "Connection",
    }
}

pub fn shows_activity(kind: &str) -> bool {
    matches!(kind, "spotify" | "xbox" | "playstation" | "playstation-stg" | "crunchyroll")
}

pub fn syncs_friends(kind: &str) -> bool {
    matches!(kind, "facebook" | "contacts")
}

pub fn has_metadata(kind: &str) -> bool {
    matches!(kind, "reddit" | "steam" | "letterboxd" | "bluesky" | "twitter" | "mastodon" | "ebay" | "paypal" | "tiktok")
}

fn option_row(id: (&'static str, usize), label: SharedString, on: bool, dim: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(z(12.))
        .cursor(gpui::CursorStyle::OperationNotAllowed)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(label),
        )
        .child(controls::switch((id.0, id.1 + 1000), on))
        .when(dim, |d| d.opacity(0.5))
}

fn card(acc: &LinkedAccount, ix: usize) -> gpui::Stateful<gpui::Div> {
    let label = platform_label(&acc.kind);
    let icon: AnyElement = match super::art::connection_icon(&acc.kind) {
        Some((url, app_icon)) => gpui::img(SharedString::from(url))
            .flex_none()
            .size(z(32.))
            .when(app_icon, |i| i.rounded(z(32.)))
            .object_fit(gpui::ObjectFit::Contain)
            .into_any_element(),
        None => gpui::svg().path(crate::icons::path("chat/link")).flex_none().size(z(32.)).text_color(c(tok(idx::ICON_DEFAULT))).into_any_element(),
    };
    let header = div()
        .flex()
        .items_center()
        .child(icon)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .ml(z(16.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(acc.name.clone()),
                )
                .child(div().ml(z(16.)).text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(label)),
        )
        .child(
            div()
                .mx(z(8.))
                .flex_none()
                .cursor(gpui::CursorStyle::OperationNotAllowed)
                .child(gpui::svg().path(crate::icons::path("close-small")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
        );
    let visible = acc.visibility == 1;
    let mut options = div().flex().flex_col().gap(z(20.)).child(option_row(("conn-vis", ix * 4), crate::t!("sp.cn.show_profile"), visible, false));
    if has_metadata(&acc.kind) {
        options = options.child(option_row(("conn-meta", ix * 4), crate::t!("sp.cn.show_details"), acc.metadata_visibility == 1, !visible));
    }
    if shows_activity(&acc.kind) {
        options = options.child(option_row(("conn-act", ix * 4), crate::t!("sp.cn.show_status", platform = label), acc.show_activity, false));
    }
    if syncs_friends(&acc.kind) {
        options = options.child(option_row(("conn-sync", ix * 4), crate::t!("sp.cn.friend_sync"), acc.friend_sync, false));
    }
    div()
        .id(("conn-card", ix))
        .w_full()
        .p(z(16.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
        .flex()
        .flex_col()
        .gap(z(20.))
        .child(header)
        .child(options)
}

pub fn render(_: &PhoenixApp, _: &mut Context<PhoenixApp>) -> AnyElement {
    let list = get().unwrap_or_default();
    if list.is_empty() && super::account::connections().unwrap_or(0) > 0 {
        return div().into_any_element();
    }
    if !list.is_empty() {
        let key = if list.len() == 1 { "sp.cn.count_one" } else { "sp.cn.count_other" };
        return div()
            .flex()
            .flex_col()
            .gap(z(24.))
            .child(
                div()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .child(crate::t!(key, count = list.len())),
            )
            .children(list.iter().enumerate().map(|(ix, acc)| card(acc, ix)))
            .into_any_element();
    }
    div()
        .min_h(z(200.))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(z(440.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .text_center()
                .text_color(c(tok(idx::TEXT_MUTED)))
                .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).child(crate::t!("sp.cn.empty")))
                .child(div().text_size(z(16.)).line_height(z(20.)).child(crate::t!("sp.cn.empty_d"))),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_map_to_the_client_connect_flow() {
        assert_eq!(authorize_path(platform_kind("Steam")).as_deref(), Some("/connections/steam/authorize?"));
        assert_eq!(authorize_path(platform_kind("X")).as_deref(), Some("/connections/twitter/authorize?"));
        assert_eq!(authorize_path("leagueoflegends").as_deref(), Some("/connections/riotgames/authorize?"));
        assert_eq!(authorize_path(platform_kind("Xbox")), None);
        assert_eq!(authorize_path(platform_kind("Bluesky")), None);
        assert_eq!(authorize_path(platform_kind("Domain")), None);
        assert_eq!(authorize_path(""), None);
        for (name, _, _) in crate::ui::settings::art::connections() {
            assert!(!platform_kind(name).is_empty(), "{name}");
        }
    }
    use serde_json::json;

    #[test]
    fn parses_linked_accounts() {
        let v = json!([
            {
                "id": "12345",
                "type": "github",
                "name": "octocat",
                "verified": true,
                "visibility": 1,
                "show_activity": true,
                "friend_sync": false,
                "metadata_visibility": 1
            },
            {
                "id": "67890",
                "type": "spotify",
                "name": "dj",
                "verified": false,
                "visibility": 0,
                "show_activity": false,
                "friend_sync": true,
                "metadata_visibility": 0
            }
        ]);
        let list = parse(&v);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].kind, "github");
        assert_eq!(list[0].name, "octocat");
        assert!(list[0].verified);
        assert_eq!(list[0].visibility, 1);
        assert!(list[0].show_activity);
        assert!(!list[0].friend_sync);
        assert_eq!(list[1].kind, "spotify");
        assert!(!list[1].verified);
        assert!(list[1].friend_sync);
        assert_eq!(platform_label(&list[0].kind), "GitHub");
        assert_eq!(platform_label(&list[1].kind), "Spotify");
        assert!(shows_activity("spotify") && !shows_activity("github"));
        assert!(syncs_friends("facebook") && !syncs_friends("spotify"));
        assert!(has_metadata("steam") && !has_metadata("spotify"));
    }
}

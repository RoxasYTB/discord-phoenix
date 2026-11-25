use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, StyledText, TextRun, div, prelude::*};

use super::{Modal, card, close_action};
use crate::{
    actions::{close_modal, open_modal, with_modal},
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{BtnKind, button},
};

pub struct State {
    pub url: String,
    pub protocol: bool,
    pub trust: bool,
}

const DISCORD_HOSTS: [&str; 7] = ["discord.gg", "discord.gift", "discord.new", "media.discordapp.net", "discord.com", "open.spotify.com", "www.spotify.com"];

#[derive(Debug, PartialEq, Eq)]
pub struct Parts {
    pub protocol: String,
    pub authority: &'static str,
    pub host: String,
    pub rest: String,
}

pub fn parts(url: &str) -> Option<Parts> {
    let u = reqwest::Url::parse(url).ok()?;
    let protocol = format!("{}:", u.scheme());
    let host = u.host_str().unwrap_or_default().to_owned();
    let after = &url[url.find(':').map_or(0, |i| i + 1)..];
    let (authority, after) = match after.strip_prefix("//") {
        Some(a) => ("//", a),
        None => ("", after),
    };
    let rest = match after.to_lowercase().find(&host) {
        Some(i) if !host.is_empty() => after.get(i + host.len()..).unwrap_or_default().to_owned(),
        _ => after.to_owned(),
    };
    Some(Parts { protocol, authority, host, rest })
}

fn discord_host(host: &str) -> bool {
    let h = host.to_lowercase();
    DISCORD_HOSTS.contains(&h.as_str()) || ["discordapp.com", "discord.com", "discordmerch.com"].iter().any(|d| h == *d || h.ends_with(&format!(".{d}")))
}

pub fn trusted(app: &PhoenixApp, text: &str, url: &str) -> bool {
    let Some(p) = parts(url) else { return false };
    let web = p.protocol == "http:" || p.protocol == "https:";
    let local = &app.prefs.local;
    if !web {
        return local.trusted_protocols.contains(&p.protocol);
    }
    let shown_as_is = text == url && !dm_with_stranger(app);
    shown_as_is || discord_host(&p.host) || local.trusted_domains.contains(&p.host.to_lowercase())
}

fn dm_with_stranger(app: &PhoenixApp) -> bool {
    let Some(ch) = app.selected_channel.and_then(|c| app.channels.get(&c)) else { return false };
    if ch.guild.is_some() || crate::ui::dm_profile_panel::is_group_dm(app, ch.id) {
        return false;
    }
    let peer = ch.voice_users.first().copied();
    !peer.is_some_and(|u| app.nav.relationships.iter().any(|r| r.user == u && r.kind == crate::ui::home::data::RelationshipKind::Friend))
}

pub fn click(app: &mut PhoenixApp, text: &str, url: &str, window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) {
    if trusted(app, text, url) {
        cx.open_url(url);
        return;
    }
    let protocol = parts(url).is_some_and(|p| p.protocol != "http:" && p.protocol != "https:");
    open_modal(Modal::LeaveLink(State { url: url.to_owned(), protocol, trust: false }), Some(window));
    cx.notify();
}

fn span(text: String, strong: bool) -> (String, TextRun) {
    let mut font = crate::richtext::ui_font();
    font.weight = if strong { FontWeight::SEMIBOLD } else { FontWeight::NORMAL };
    let color = c(tok(if strong { idx::TEXT_DEFAULT } else { idx::TEXT_MUTED }));
    let run = TextRun { len: text.len(), font, color, background_color: None, underline: None, strikethrough: None };
    (text, run)
}

fn styled(pieces: Vec<(String, TextRun)>) -> StyledText {
    let text: String = pieces.iter().map(|p| p.0.as_str()).collect();
    StyledText::new(SharedString::from(text)).with_runs(pieces.into_iter().map(|p| p.1).collect())
}

pub fn render(s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let p = parts(&s.url).unwrap_or(Parts { protocol: String::new(), authority: "", host: String::new(), rest: s.url.clone() });
    let callout = div()
        .id("leave-link-url")
        .min_w(z(408.))
        .max_w(z(498.))
        .max_h(z(144.))
        .overflow_y_scroll()
        .mb(z(12.))
        .p(z(16.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .text_size(z(16.))
        .line_height(z(20.))
        .child(styled(vec![span(format!("{}{}", p.protocol, p.authority), s.protocol), span(p.host.clone(), !s.protocol), span(p.rest.clone(), false)]));
    let (lead, target) = if s.protocol {
        (crate::t!("link.trust_protocol"), p.protocol.trim_end_matches(':').to_owned())
    } else {
        (crate::t!("link.trust_domain"), p.host.clone())
    };
    let label = styled(vec![span(lead.to_string(), false), span(target, true), span(crate::t!("link.trust_suffix").to_string(), false)]);
    let check = crate::ui::controls::checkbox_option_el("leave-link-trust", label.into_any_element(), s.trust).on_click(cx.listener(|_, _, _, cx| {
        with_modal(|m| {
            if let Some(Modal::LeaveLink(s)) = m {
                s.trust = !s.trust;
            }
        });
        cx.notify();
    }));
    let body = div().flex().flex_col().child(callout).child(check);
    let footer = div()
        .flex_1()
        .flex()
        .gap(z(8.))
        .child(button("leave-link-back", crate::t!("link.back"), BtnKind::Secondary).flex_1().on_click({
            let close = close_action(cx);
            move |_, w, cx| close(w, cx)
        }))
        .child(button("leave-link-go", if s.protocol { crate::t!("link.open") } else { crate::t!("link.visit") }, BtnKind::Primary).flex_1().on_click(
            cx.listener(|a, _, _, cx| {
                let taken = with_modal(|m| match m.take() {
                    Some(Modal::LeaveLink(s)) => Some(s),
                    other => {
                        *m = other;
                        None
                    }
                });
                if let Some(s) = taken {
                    confirm(a, &s);
                    cx.open_url(&s.url);
                }
                close_modal();
                cx.notify();
            }),
        ));
    let subtitle = if s.protocol { crate::t!("link.leaving_app") } else { crate::t!("link.leaving_site") };
    card(480., crate::t!("link.leaving"), Some(subtitle), body.into_any_element(), Some(footer.into_any_element()), cx)
}

fn confirm(app: &mut PhoenixApp, s: &State) {
    if !s.trust {
        return;
    }
    let Some(p) = parts(&s.url) else { return };
    let local = &mut app.prefs.local;
    if s.protocol {
        local.trusted_protocols.insert(p.protocol);
    } else {
        local.trusted_domains.insert(p.host.to_lowercase());
    }
    local.save();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_like_the_client() {
        let p = parts("https://Example.com:8080/a/b?q=1#x").unwrap();
        assert_eq!((p.protocol.as_str(), p.authority, p.host.as_str(), p.rest.as_str()), ("https:", "//", "example.com", ":8080/a/b?q=1#x"));
        let p = parts("steam://run/440").unwrap();
        assert_eq!((p.protocol.as_str(), p.authority, p.host.as_str(), p.rest.as_str()), ("steam:", "//", "run", "/440"));
    }

    #[test]
    fn discord_hosts_are_trusted() {
        assert!(discord_host("discord.com"));
        assert!(discord_host("ptb.discord.com"));
        assert!(discord_host("cdn.discordapp.com"));
        assert!(discord_host("discord.gg"));
        assert!(discord_host("open.spotify.com"));
        assert!(!discord_host("discord.com.evil.net"));
        assert!(!discord_host("example.com"));
    }
}

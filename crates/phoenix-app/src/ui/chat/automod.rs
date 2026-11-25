use gpui::{AnyElement, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, svg};

use super::system::{Seg, parse_template};
use crate::{
    app::PhoenixApp,
    model::{Id, Message},
    richtext::RenderedMessage,
    theme::{Palette, c, generated::idx, tok, z},
};

pub const AVATAR_PNG: &str = "https://discord.com/assets/32336875baac6ac1.png";
pub const AVATAR_GIF: &str = "https://discord.com/assets/d9bd8932f55dc4ba.gif";

pub const CONTENT_BLOCK: usize = 0xA0;

pub const ACTION_SET_COMPLETED: u8 = 1;
pub const ACTION_DELETE_USER_MESSAGE: u8 = 3;
pub const ACTION_SUBMIT_FEEDBACK: u8 = 4;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Data {
    pub notification: bool,
    pub fields: Vec<(String, String)>,
    pub content: String,
}

impl Data {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.fields.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }
}

pub fn data(msg: &Message) -> Option<Data> {
    let e = msg.embeds.first()?;
    let notification = match e.kind.as_deref() {
        Some("auto_moderation_message") => false,
        Some("auto_moderation_notification") => true,
        _ => return None,
    };
    Some(Data {
        notification,
        fields: e.fields.iter().map(|f| (f.name.to_string(), f.value.to_string())).collect(),
        content: e.description.as_deref().unwrap_or_default().to_string(),
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum Header {
    Plain(&'static str),
    Channel(&'static str),
    AppModal(&'static str),
    AppMessage(&'static str),
}

pub fn header(d: &Data, interaction_user_known: bool) -> Header {
    if d.get("quarantine_user").is_some() {
        let key = match d.get("quarantine_user_action") {
            Some("block_profile_update") => match d.get("block_profile_update_type") {
                Some("nickname_update") => Some("sys.am_nick_update"),
                Some("nickname_reset") => Some("sys.am_nick_reset"),
                _ => None,
            },
            Some("quarantine_user") => match d.get("quarantine_event") {
                Some("message_send") => Some("sys.am_q_message"),
                Some("guild_join") => Some("sys.am_q_join"),
                Some("username_update") => Some("sys.am_q_username"),
                Some("clan_tag_update") => Some("sys.am_q_tag"),
                _ => None,
            },
            Some("block_guest_join") => Some("sys.am_guest_join"),
            _ => None,
        };
        if let Some(k) = key {
            return Header::Plain(k);
        }
    }
    if let Some(o) = d.get("voice_channel_status_outcome") {
        return Header::Channel(if o == "blocked" { "sys.am_vc_blocked" } else { "sys.am_vc_flagged" });
    }
    if let Some(o) = d.get("guild_room_note_outcome") {
        return Header::Channel(if o == "blocked" { "sys.am_note_blocked" } else { "sys.am_note_flagged" });
    }
    let blocked = d.get("decision_outcome") == Some("blocked");
    if d.get("application_name").is_some() {
        if d.get("interaction_callback_type") == Some("MODAL") && interaction_user_known {
            return Header::AppModal(if blocked { "sys.am_appmsg_modal_blocked" } else { "sys.am_appmsg_modal_flagged" });
        }
        return Header::AppMessage(if blocked { "sys.am_appmsg_blocked" } else { "sys.am_appmsg_flagged" });
    }
    Header::Channel(if blocked { "sys.am_blocked" } else { "sys.am_flagged" })
}

pub fn interaction_user(d: &Data) -> Option<Id> {
    d.get("interaction_user_id").and_then(|v| v.trim().parse::<u64>().ok()).map(Id)
}

pub fn timeout_text(code: &str, raw: Option<&str>) -> Option<String> {
    let secs: f64 = raw?.trim().parse().ok()?;
    let t = |k: &str, n: &str| crate::i18n::t_in(code, k, &[("n", n)]).to_string();
    match secs as i64 {
        _ if secs.fract() != 0. => None,
        60 => Some(t("sys.am_dur_60s", "60")),
        300 => Some(t("sys.am_dur_mins", "5")),
        600 => Some(t("sys.am_dur_mins", "10")),
        3600 => Some(t("sys.am_dur_1h", "1")),
        86_400 => Some(t("sys.am_dur_1d", "1")),
        604_800 => Some(t("sys.am_dur_1w", "1")),
        _ => None,
    }
}

pub fn quarantine_field(code: &str, v: &str) -> String {
    let key = match v {
        "nickname" => "sys.am_field_nickname",
        "display_name" => "sys.am_field_display",
        "clan_tag" => "sys.am_field_tag",
        _ => "sys.am_field_username",
    };
    capitalize(&crate::i18n::t_in(code, key, &[]))
}

fn capitalize(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut ch = w.chars();
            match ch.next() {
                Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Debug, PartialEq)]
pub enum Foot {
    Text(String),
    Dot,
}

pub fn footer(code: &str, d: &Data) -> Vec<Foot> {
    let t = |k: &str, var: &str, v: &str| crate::i18n::t_in(code, k, &[(var, v)]).to_string();
    let mut out = Vec::new();
    if let Some(k) = d.get("keyword") {
        out.push(Foot::Text(t("sys.am_keyword", "k", k)));
        out.push(Foot::Dot);
    }
    if let Some(r) = d.get("rule_name") {
        out.push(Foot::Text(t("sys.am_rule", "r", r)));
    }
    if let Some(dur) = timeout_text(code, d.get("timeout_duration")) {
        out.push(Foot::Dot);
        out.push(Foot::Text(t("sys.am_timeout", "d", &dur)));
    }
    if let Some(q) = d.get("quarantine_user") {
        out.push(Foot::Dot);
        out.push(Foot::Text(t("sys.am_reason", "r", &quarantine_field(code, q))));
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertAction {
    pub action_type: u8,
    pub actor: Option<String>,
    pub ts: Option<String>,
}

pub fn alert_actions(d: &Data) -> Option<Vec<AlertAction>> {
    let v: serde_json::Value = serde_json::from_str(d.get("alert_actions_execution")?).ok()?;
    let obj = v.get("actions")?.as_object()?;
    let mut out: Vec<AlertAction> = obj
        .iter()
        .filter_map(|(k, a)| {
            Some(AlertAction {
                action_type: k.parse().ok()?,
                actor: a.get("actor").and_then(|x| x.as_str()).map(str::to_string),
                ts: a.get("ts").and_then(|x| x.as_str()).map(str::to_string),
            })
        })
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.action_type.to_string()));
    Some(out)
}

pub fn deleted_already(d: &Data) -> bool {
    alert_actions(d).is_some_and(|v| v.iter().any(|a| a.action_type == ACTION_DELETE_USER_MESSAGE))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub jump: bool,
    pub delete: bool,
}

pub fn buttons(d: &Data, manage_messages: bool) -> Buttons {
    let flagged = d.get("flagged_message_id").is_some();
    Buttons { jump: d.get("channel_id").is_some(), delete: manage_messages && flagged && !deleted_already(d) }
}

pub fn highlight(content: &str, word: Option<&str>) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    match word.filter(|w| !w.is_empty()) {
        Some(w) => split_highlight(content, w, 0, &mut out),
        None => out.push((content.to_string(), false)),
    }
    out.retain(|(t, _)| !t.is_empty());
    out
}

fn split_highlight(s: &str, w: &str, depth: u8, out: &mut Vec<(String, bool)>) {
    let edge_ok = |i: usize| i == 0 || s[..i].chars().next_back().is_some_and(char::is_whitespace);
    let tail_ok = |i: usize| i + w.len() == s.len() || s[i + w.len()..].chars().next().is_some_and(char::is_whitespace);
    let mut from = 0;
    let found = loop {
        let Some(p) = (depth <= 10).then(|| s.get(from..).and_then(|r| r.find(w))).flatten() else { break None };
        let at = from + p;
        if edge_ok(at) && tail_ok(at) {
            break Some(at);
        }
        from = at + s[at..].chars().next().map_or(1, char::len_utf8);
    };
    match found {
        Some(at) => {
            split_highlight(&s[..at], w, depth + 1, out);
            out.push((w.to_string(), true));
            split_highlight(&s[at + w.len()..], w, depth + 1, out);
        }
        None => out.push((s.to_string(), false)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    Raid,
    MentionRaid,
    AlertsEnabled,
    InteractionBlocked,
    Unsupported,
}

pub fn notice(d: &Data) -> Notice {
    match d.get("notification_type") {
        None | Some("raid") => Notice::Raid,
        Some("mention_raid") => Notice::MentionRaid,
        Some("activity_alerts_enabled") => Notice::AlertsEnabled,
        Some("interaction_blocked") => Notice::InteractionBlocked,
        Some(_) => Notice::Unsupported,
    }
}

pub fn resolved_key(reason: Option<&str>) -> &'static str {
    match reason {
        None => "sys.am_resolve",
        Some("LEGITIMATE_ACTIVITY") => "sys.am_resolved_none",
        Some("DM_SPAM") => "sys.am_resolved_dm",
        Some("JOIN_RAID") => "sys.am_resolved_join",
        Some(_) => "sys.am_resolved_other",
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RaidCard {
    pub dm: bool,
    pub subheader: Vec<String>,
    pub began: Option<chrono::DateTime<chrono::FixedOffset>>,
    pub resolve: &'static str,
}

pub fn raid_card(code: &str, d: &Data) -> RaidCard {
    let dm = d.get("raid_type") == Some("DM_RAID");
    let mut sub = Vec::new();
    if let Some(n) = d.get("join_attempts").and_then(|v| v.trim().parse::<i64>().ok()) {
        sub.push(crate::i18n::t_in(code, "sys.am_joins", &[("n", &n.to_string())]).to_string());
    }
    if let Some(n) = d.get("dms_sent").and_then(|v| v.trim().parse::<i64>().ok()) {
        sub.push(crate::i18n::t_in(code, "sys.am_dms", &[("n", &n.to_string())]).to_string());
    }
    let began = d.get("raid_datetime").and_then(|v| chrono::DateTime::parse_from_rfc3339(v.trim()).ok());
    RaidCard { dm, subheader: sub, began, resolve: resolved_key(d.get("resolved_reason")) }
}

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_space = false;
    for ch in text.chars() {
        let breaking = ch.is_whitespace() && ch != '\u{a0}';
        if breaking {
            in_space = true;
        } else if in_space {
            out.push(std::mem::take(&mut cur));
            in_space = false;
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn dot() -> gpui::Div {
    div().flex_none().size(z(4.)).rounded_full().bg(tk(idx::BACKGROUND_MOD_SUBTLE))
}

fn small(text: impl Into<SharedString>, color: usize) -> gpui::Div {
    div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(tk(color)).child(text.into())
}

fn text_button(id: (&'static str, u64), label: SharedString, medium: bool) -> gpui::Stateful<gpui::Div> {
    let group: SharedString = format!("{}-{}", id.0, id.1).into();
    div()
        .id(id)
        .group(group.clone())
        .flex_none()
        .cursor_pointer()
        .child(
            div()
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(if medium { FontWeight::MEDIUM } else { FontWeight::NORMAL })
                .text_color(tk(idx::TEXT_BRAND))
                .group_hover(group, |s| s.underline())
                .child(label),
        )
}

fn footer_action(child: impl IntoElement) -> gpui::Div {
    div().flex().items_center().p(z(8.)).child(child)
}

fn shield_action(id: (&'static str, u64), on: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static, medium: bool) -> gpui::Div {
    footer_action(
        div()
            .flex()
            .items_center()
            .child(svg().path(crate::icons::path("shield")).flex_none().size(z(16.)).mr(z(4.)).text_color(tk(idx::BRAND_500)))
            .child(text_button(id, crate::t!("sys.am_actions"), medium).on_click(on)),
    )
}

pub fn avatar(group: SharedString) -> AnyElement {
    div()
        .relative()
        .size(z(40.))
        .rounded_full()
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .child(div().size(z(40.)).group_hover(group.clone(), |s| s.opacity(0.)).child(gpui::img(SharedString::new_static(AVATAR_PNG)).size(z(40.))))
        .child(div().absolute().inset_0().opacity(0.).group_hover(group, |s| s.opacity(1.)).child(gpui::img(SharedString::new_static(AVATAR_GIF)).size(z(40.))))
        .into_any_element()
}

fn name_header(header_text: Vec<AnyElement>, time: SharedString) -> gpui::Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .min_h(z(22.))
        .child(div().mr(z(4.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_BRAND)).child(crate::t!("sys.automod")))
        .child(super::bot_tag_label(true, crate::t!("sys.official")).ml_0().mr(z(4.)).mt_0())
        .children(header_text)
        .child(div().ml(z(4.)).text_size(z(12.)).line_height(z(22.)).text_color(c(Palette::TEXT_MUTED)).child(time))
}

fn frame(msg: &Message, r: &RenderedMessage, group: SharedString, body: Vec<AnyElement>) -> AnyElement {
    div()
        .id(("automod", msg.id.0))
        .group(group.clone())
        .relative()
        .pl(z(80.))
        .pr(z(48.))
        .py(z(2.))
        .mt(z(if r.group_start { 19. } else { 2. }))
        .mb(z(2.))
        .min_h(z(44.))
        .flex()
        .flex_col()
        .text_size(z(16.))
        .line_height(z(24.))
        .text_color(tk(idx::TEXT_DEFAULT))
        .hover(|s| s.bg(c(Palette::BG_MSG_HOVER)))
        .child(div().absolute().left(z(8.)).top(z(2.)).w(z(72.)).h(z(40.)).flex().items_center().justify_center().child(avatar(group)))
        .children(body)
        .into_any_element()
}

fn channel_chip(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, ch: Option<Id>) -> AnyElement {
    let known = ch.and_then(|id| app.channels.get(&id).map(|c| (id, c.name.clone())));
    let name: SharedString = known.as_ref().map_or_else(|| crate::t!("sys.am_unknown_channel"), |(_, n)| n.clone());
    let e = entity.clone();
    let target = known.map(|(id, _)| id);
    div()
        .id(("automod-ch", msg.id.0))
        .flex()
        .items_center()
        .font_weight(FontWeight::MEDIUM)
        .text_color(tk(idx::TEXT_LINK))
        .cursor_pointer()
        .hover(|s| s.underline())
        .child(svg().path(crate::icons::path("hash")).flex_none().size(z(16.)).mr(z(2.)).text_color(tk(idx::TEXT_LINK)))
        .child(name)
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            if let Some(id) = target {
                e.update(cx, |a, cx| a.select_channel(id, cx));
            }
        })
        .into_any_element()
}

fn username(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, uid: Id, key: &'static str) -> AnyElement {
    let user = app.users.get(&uid);
    let (name_color, gradient) = app.author_role_style(uid);
    let name_color = name_color.map(c).unwrap_or_else(|| c(Palette::TEXT_HEADER));
    let name: SharedString = msg
        .author_nick
        .clone()
        .filter(|_| uid == msg.author)
        .or_else(|| app.member_nick(uid))
        .or_else(|| user.map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("sys.someone"));
    crate::fonts::styled_name(div().font_weight(FontWeight::MEDIUM).text_color(name_color).cursor_pointer().hover(|s| s.underline()), user, false)
        .id((key, msg.id.0))
        .on_click(super::message_ext::open_user(entity, uid, msg.channel))
        .child(crate::fonts::role_name_text(name, user, 16., app.selected_guild.is_some(), gradient))
        .into_any_element()
}

fn header_text(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, d: &Data) -> Vec<AnyElement> {
    let ch = d.get("channel_id").and_then(|v| v.parse::<u64>().ok()).map(Id);
    let iu = interaction_user(d).filter(|u| app.users.contains_key(u));
    let h = header(d, iu.is_some());
    let (key, with_channel) = match h {
        Header::Plain(k) => (k, false),
        Header::Channel(k) | Header::AppMessage(k) => (k, true),
        Header::AppModal(k) => (k, false),
    };
    let strong = |w: String| div().text_color(tk(idx::TEXT_STRONG)).child(w).into_any_element();
    let bold = |w: String| div().text_color(tk(idx::TEXT_STRONG)).font_weight(FontWeight::SEMIBOLD).child(w).into_any_element();
    let app_name = d.get("application_name").unwrap_or_default().to_string();
    let mut out = Vec::new();
    for seg in parse_template(&crate::i18n::t(key, &[]), &[("ch", String::new()), ("app", app_name)]) {
        match seg {
            Seg::Link(..) if with_channel => out.push(channel_chip(app, entity, msg, ch)),
            Seg::Name(n) if n == "owner" => out.push(username(app, entity, msg, msg.author, "automod-owner")),
            Seg::Name(n) if n == "iu" => {
                if let Some(u) = iu {
                    out.push(username(app, entity, msg, u, "automod-iu"));
                }
            }
            Seg::Strong(t) => out.extend(words(&t).into_iter().map(bold)),
            Seg::Text(t) | Seg::Name(t) | Seg::Link(_, t) => out.extend(words(&t).into_iter().map(strong)),
            Seg::Author => {}
        }
    }
    out
}

fn flagged_message(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, d: &Data, footer_row: Vec<Foot>, reason: Option<String>) -> AnyElement {
    let user = app.users.get(&msg.author);
    let author = username(app, entity, msg, msg.author, "automod-author");
    let revealed = |i: usize| app.revealed.contains(&(msg.id, i));
    let segments = highlight(&d.content, d.get("keyword_matched_content"));
    let rich = crate::richtext::highlighted_inline(&segments, &app.resolver_with_sounds(&[], msg.channel), &revealed, tk(idx::TEXT_DEFAULT), c(tok(idx::YELLOW_300)).opacity(0.3));
    let content = (!rich.text.is_empty()).then(|| super::rich(entity, msg.id, CONTENT_BLOCK, &rich, None));
    let row: Vec<AnyElement> = footer_row
        .into_iter()
        .map(|f| match f {
            Foot::Text(t) => small(t, idx::TEXT_MUTED).into_any_element(),
            Foot::Dot => dot().mx(z(8.)).into_any_element(),
        })
        .collect();
    let avatar_px = f32::from(z(40.));
    div()
        .mt(z(4.))
        .p(z(12.))
        .rounded_t(z(8.))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .child(
            div()
                .relative()
                .ml(z(-8.))
                .pl(z(72.))
                .pr(z(48.))
                .py(z(2.))
                .min_h(z(44.))
                .child(div().absolute().left(z(16.)).top(z(2.)).size(z(40.)).child(crate::ui::avatar(user, avatar_px)))
                .child(div().flex().items_center().h(z(22.)).line_height(z(22.)).child(author))
                .child(div().text_color(tk(idx::TEXT_DEFAULT)).children(content))
                .when(!row.is_empty(), |el| el.child(div().flex().items_center().flex_wrap().mt(z(2.)).children(row)))
                .children(reason.map(|r| {
                    div().mt(z(4.)).px(z(8.)).pt(z(4.)).pb(z(6.)).rounded(z(4.)).line_height(z(20.)).bg(tk(idx::BACKGROUND_MOD_NORMAL)).child(small(r, idx::TEXT_DEFAULT))
                })),
        )
        .into_any_element()
}

fn action_icon(a: &AlertAction) -> Option<AnyElement> {
    let (icon, color) = match a.action_type {
        ACTION_DELETE_USER_MESSAGE => ("chat/trash", idx::TEXT_MUTED),
        ACTION_SET_COMPLETED => ("check", idx::TEXT_FEEDBACK_POSITIVE),
        ACTION_SUBMIT_FEEDBACK => ("flag", idx::TEXT_MUTED),
        _ => return None,
    };
    Some(div().pl(z(6.)).child(svg().path(crate::icons::path(icon)).flex_none().size(z(16.)).text_color(tk(color))).into_any_element())
}

fn message_card(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &RenderedMessage, d: &Data) -> AnyElement {
    let code = crate::i18n::locale();
    let guild = app.channel_guild(msg.channel);
    let manage = crate::actions::has(app, guild, crate::actions::MANAGE_MESSAGES) || crate::actions::has(app, guild, crate::actions::ADMINISTRATOR);
    let b = buttons(d, manage);
    let embed_ch = d.get("channel_id").and_then(|v| v.parse::<u64>().ok());
    let mid = msg.id.0;
    let channel = msg.channel;
    let mut actions: Vec<AnyElement> = vec![shield_action(("am-actions", mid), super::message_ext::open_user(entity, msg.author, msg.channel), false).into_any_element()];
    if b.jump {
        let e = entity.clone();
        actions.push(dot().into_any_element());
        actions.push(
            footer_action(text_button(("am-jump", mid), crate::t!("sys.am_jump"), false).on_click(move |_, _, cx| {
                if let Some(ch) = embed_ch {
                    e.update(cx, |a, cx| a.jump_to_message(Id(ch), Id(mid), cx));
                }
            }))
            .into_any_element(),
        );
    }
    let gid = guild.map_or(0, |g| g.0);
    let e = entity.clone();
    actions.push(dot().into_any_element());
    actions.push(
        footer_action(text_button(("am-report", mid), crate::t!("sys.am_report"), false).on_click(move |_, _, cx| {
            e.update(cx, |a, cx| a.blocked_write(crate::actions::guard::GuardedWrite::automod_alert_action(gid, channel.0, mid, ACTION_SUBMIT_FEEDBACK), cx));
        }))
        .into_any_element(),
    );
    if b.delete {
        let e = entity.clone();
        actions.push(dot().into_any_element());
        actions.push(
            footer_action(text_button(("am-delete", mid), crate::t!("sys.am_delete"), false).on_click(move |_, _, cx| {
                e.update(cx, |a, cx| a.blocked_write(crate::actions::guard::GuardedWrite::automod_alert_action(gid, channel.0, mid, ACTION_DELETE_USER_MESSAGE), cx));
            }))
            .into_any_element(),
        );
    }
    let icons: Vec<AnyElement> = alert_actions(d).unwrap_or_default().iter().filter_map(action_icon).collect();
    let foot = footer(code, d);
    let reason = d.get("decision_reason").map(str::to_string);
    let group: SharedString = format!("automod-{mid}").into();
    frame(
        msg,
        r,
        group,
        vec![
            name_header(header_text(app, entity, msg, d), r.time_full.clone()).into_any_element(),
            flagged_message(app, entity, msg, d, foot, reason),
            div()
                .rounded_b(z(8.))
                .bg(tk(idx::BACKGROUND_BASE_LOWEST))
                .line_height(z(20.))
                .pt(z(2.))
                .px(z(18.))
                .pb(z(4.))
                .child(div().flex().items_center().ml(z(-8.)).children(actions).when(!icons.is_empty(), |el| el.child(div().ml_auto().flex().items_center().children(icons))))
                .into_any_element(),
        ],
    )
}

fn card(header: AnyElement, subheader: Option<AnyElement>, content: Option<AnyElement>, footer: Option<AnyElement>) -> AnyElement {
    div()
        .mt(z(4.))
        .mb(z(12.))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(z(8.))
                .p(z(12.))
                .child(div().flex().flex_col().items_start().gap(z(2.)).child(header).children(subheader.map(|s| div().flex().items_center().gap(z(8.)).child(s))))
                .children(content),
        )
        .children(footer.map(|f| div().flex().items_center().p(z(4.)).bg(tk(idx::BACKGROUND_MOD_NORMAL)).child(f)))
        .into_any_element()
}

fn card_header(icon: &'static str, color: usize, text: SharedString) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(z(4.))
        .child(svg().path(crate::icons::path(icon)).flex_none().size(z(16.)).text_color(tk(color)))
        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(color)).child(text))
        .into_any_element()
}

fn muted_body(text: SharedString) -> AnyElement {
    div().text_size(z(16.)).line_height(z(20.)).text_color(tk(idx::TEXT_MUTED)).child(text).into_any_element()
}

fn ago(ts: chrono::DateTime<chrono::FixedOffset>) -> String {
    crate::ui::settings::sessions::from_now(chrono::Utc::now().signed_duration_since(ts).num_seconds())
}

fn notification_card(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &RenderedMessage, d: &Data) -> AnyElement {
    let code = crate::i18n::locale();
    let mid = msg.id.0;
    let guild = app.channel_guild(msg.channel);
    let kind = notice(d);
    let body = match kind {
        Notice::Raid => {
            let rc = raid_card(code, d);
            let mut sub: Vec<AnyElement> = rc.subheader.iter().map(|s| small(s.clone(), idx::TEXT_DEFAULT).into_any_element()).collect();
            if let Some(t) = rc.began {
                sub.push(dot().into_any_element());
                sub.push(small(ago(t), idx::TEXT_DEFAULT).into_any_element());
            }
            let content = rc.began.map(|t| {
                let dt = super::system::format_alert(code, t.with_timezone(&chrono::Local).naive_local());
                muted_body(crate::i18n::t("sys.am_began", &[("dt", &dt)]))
            });
            let show = crate::actions::has(app, guild, crate::actions::MANAGE_GUILD) || crate::actions::has(app, guild, crate::actions::ADMINISTRATOR);
            let footer = show.then(|| {
                let e = entity.clone();
                div()
                    .flex()
                    .items_center()
                    .child(shield_action(("am-raid-actions", mid), super::message_ext::open_user(entity, msg.author, msg.channel), true))
                    .child(dot())
                    .child(footer_action(text_button(("am-resolve", mid), crate::t!(rc.resolve), true).on_click(move |_, _, cx| {
                        e.update(cx, |a, cx| a.parity_toast(crate::t!("acct.blocked"), cx));
                    })))
                    .into_any_element()
            });
            let (icon, title) = if rc.dm { ("settings/circle-error", "sys.am_dm_raid") } else { ("mod/chat-warning", "sys.am_join_raid") };
            card(card_header(icon, idx::TEXT_FEEDBACK_CRITICAL, crate::t!(title)), Some(div().flex().items_center().gap(z(4.)).children(sub).into_any_element()), content, footer)
        }
        Notice::MentionRaid => {
            let sub = d.get("raid_datetime").and_then(|v| chrono::DateTime::parse_from_rfc3339(v.trim()).ok()).map(|t| small(ago(t), idx::TEXT_DEFAULT).into_any_element());
            let gid = guild.map_or(0, |g| g.0);
            let (e1, e2) = (entity.clone(), entity.clone());
            let footer = div()
                .flex()
                .items_center()
                .child(footer_action(text_button(("am-unrestrict", mid), crate::t!("sys.am_remove_restrictions"), true).on_click(move |_, _, cx| {
                    e1.update(cx, |a, cx| a.blocked_write(crate::actions::guard::GuardedWrite::automod_clear_mention_raid(gid), cx));
                })))
                .child(dot())
                .child(footer_action(text_button(("am-configure", mid), crate::t!("sys.am_configure"), true).on_click(move |_, _, cx| {
                    if let Some(g) = guild {
                        let section = crate::ui::modals::server_settings::section_index("menu.gs.safety");
                        e2.update(cx, |a, cx| a.open_server_settings(g, section, cx));
                    }
                })))
                .into_any_element();
            card(card_header("settings/circle-error", idx::TEXT_FEEDBACK_CRITICAL, crate::t!("sys.am_mention_raid")), sub, Some(muted_body(crate::t!("sys.am_mention_body"))), Some(footer))
        }
        Notice::AlertsEnabled => {
            let by = d.get("action_by_user_id").and_then(|v| v.parse::<u64>().ok()).map(Id).and_then(|u| app.users.get(&u).map(|x| (u, x)));
            let mut sub: Vec<AnyElement> = Vec::new();
            if let Some((uid, user)) = by {
                let color = app.author_role_style(uid).0.map(c).unwrap_or_else(|| tk(idx::TEXT_DEFAULT));
                sub.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(small(crate::t!("sys.am_enabled_by"), idx::TEXT_DEFAULT))
                        .child(
                            div()
                                .id(("am-enabled-by", mid))
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .on_click(super::message_ext::open_user(entity, uid, msg.channel))
                                .child(div().size(z(16.)).child(crate::ui::avatar(Some(user), f32::from(z(16.)))))
                                .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(color).child(format!(" @{}", user.name))),
                        )
                        .into_any_element(),
                );
            }
            sub.push(dot().into_any_element());
            sub.push(small(crate::ui::settings::sessions::from_now(chrono::Utc::now().timestamp() - msg.timestamp), idx::TEXT_DEFAULT).into_any_element());
            card(
                card_header("settings-info", idx::TEXT_FEEDBACK_POSITIVE, crate::t!("sys.am_alerts_on")),
                Some(div().flex().items_center().gap(z(4.)).children(sub).into_any_element()),
                Some(muted_body(crate::t!("sys.am_alerts_body"))),
                None,
            )
        }
        Notice::InteractionBlocked => card(card_header("settings/circle-error", idx::TEXT_DEFAULT, crate::t!("sys.am_app_blocked")), None, Some(muted_body(crate::t!("sys.am_app_body"))), None),
        Notice::Unsupported => card(card_header("settings/circle-error", idx::TEXT_DEFAULT, crate::t!("sys.am_unsupported")), None, Some(muted_body(crate::t!("sys.am_unsupported_body"))), None),
    };
    let alert = matches!(kind, Notice::Raid);
    let extra: Vec<AnyElement> = if alert { words(&crate::t!("sys.am_safety_alert")).into_iter().map(|w| div().text_color(tk(idx::TEXT_STRONG)).child(w).into_any_element()).collect() } else { Vec::new() };
    let group: SharedString = format!("automod-{mid}").into();
    frame(msg, r, group, vec![name_header(extra, r.time_full.clone()).into_any_element(), body])
}

pub fn render(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &RenderedMessage) -> AnyElement {
    let d = data(msg).unwrap_or_default();
    if d.notification { notification_card(app, entity, msg, r, &d) } else { message_card(app, entity, msg, r, &d) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(fields: &[(&str, &str)], content: &str) -> Data {
        Data { notification: false, fields: fields.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), content: content.into() }
    }

    #[test]
    fn blocked_message() {
        let x = d(&[("rule_name", "Gros mots"), ("channel_id", "5"), ("keyword", "nul*"), ("keyword_matched_content", "nul"), ("decision_outcome", "blocked")], "c'est nul ici");
        assert_eq!(header(&x, false), Header::Channel("sys.am_blocked"));
        assert_eq!(crate::i18n::t_in("fr", "sys.am_blocked", &[]), "a bloqué un message dans [{ch}]");
        assert_eq!(
            footer("fr", &x),
            vec![Foot::Text("Mot-clé\u{a0}: nul*".into()), Foot::Dot, Foot::Text("Règle\u{a0}: Gros mots".into())]
        );
        assert_eq!(buttons(&x, true), Buttons { jump: true, delete: false });
        assert_eq!(highlight(&x.content, x.get("keyword_matched_content")), vec![("c'est ".into(), false), ("nul".into(), true), (" ici".into(), false)]);
    }

    #[test]
    fn flagged_message_with_delete_button() {
        let x = d(&[("rule_name", "Liens"), ("channel_id", "5"), ("flagged_message_id", "9"), ("decision_outcome", "flagged")], "https://x");
        assert_eq!(header(&x, false), Header::Channel("sys.am_flagged"));
        assert_eq!(header(&d(&[], ""), false), Header::Channel("sys.am_flagged"));
        assert_eq!(buttons(&x, true), Buttons { jump: true, delete: true });
        assert!(!buttons(&x, false).delete);
        let done = d(&[("flagged_message_id", "9"), ("alert_actions_execution", r#"{"actions":{"3":{"actor":"1","ts":"2026-11-25T10:00:00Z"},"1":{"actor":"1","ts":"2026-11-25T10:01:00Z"}}}"#)], "");
        assert!(deleted_already(&done));
        assert!(!buttons(&done, true).delete);
        assert_eq!(alert_actions(&done).unwrap().iter().map(|a| a.action_type).collect::<Vec<_>>(), vec![3, 1]);
        assert_eq!(footer("en", &x), vec![Foot::Text("Rule: Liens".into())]);
    }

    #[test]
    fn member_timed_out() {
        let x = d(&[("rule_name", "Spam"), ("keyword", "free nitro"), ("timeout_duration", "600"), ("decision_outcome", "blocked")], "free nitro");
        let f = footer("fr", &x);
        assert_eq!(f.last(), Some(&Foot::Text("Exclusion temporaire\u{a0}: 10\u{a0}min".into())));
        assert_eq!(f[f.len() - 2], Foot::Dot);
        assert_eq!(timeout_text("en", Some("60")).as_deref(), Some("60 secs"));
        assert_eq!(timeout_text("fr", Some("3600")).as_deref(), Some("1\u{a0}heure"));
        assert_eq!(timeout_text("en", Some("604800")).as_deref(), Some("1 week"));
        assert_eq!(timeout_text("en", Some("1234")), None);
        assert_eq!(timeout_text("en", None), None);
    }

    #[test]
    fn name_or_profile_flagged() {
        let x = d(&[("quarantine_user", "nickname"), ("quarantine_user_action", "quarantine_user"), ("quarantine_event", "username_update"), ("rule_name", "Pseudos")], "");
        assert_eq!(header(&x, false), Header::Plain("sys.am_q_username"));
        assert_eq!(footer("fr", &x).last(), Some(&Foot::Text("Raison\u{a0}: Pseudo".into())));
        let nick = d(&[("quarantine_user", "username"), ("quarantine_user_action", "block_profile_update"), ("block_profile_update_type", "nickname_update")], "");
        assert_eq!(header(&nick, false), Header::Plain("sys.am_nick_update"));
        assert_eq!(footer("en", &nick), vec![Foot::Dot, Foot::Text("Reason: Username".into())]);
        assert_eq!(quarantine_field("en", "display_name"), "Display Name");
        let unknown = d(&[("quarantine_user", "bio"), ("quarantine_user_action", "quarantine_user"), ("quarantine_event", "other")], "");
        assert_eq!(header(&unknown, false), Header::Channel("sys.am_flagged"));
        assert_eq!(header(&d(&[("quarantine_user", "bio"), ("quarantine_user_action", "block_guest_join")], ""), false), Header::Plain("sys.am_guest_join"));
        assert_eq!(header(&d(&[("voice_channel_status_outcome", "blocked")], ""), false), Header::Channel("sys.am_vc_blocked"));
    }

    #[test]
    fn application_variants() {
        let modal = d(&[("application_name", "Bot"), ("interaction_callback_type", "MODAL"), ("interaction_user_id", "77"), ("decision_outcome", "flagged")], "");
        assert_eq!(interaction_user(&modal), Some(Id(77)));
        assert_eq!(header(&modal, true), Header::AppModal("sys.am_appmsg_modal_flagged"));
        assert_eq!(header(&modal, false), Header::AppMessage("sys.am_appmsg_flagged"));
        let blocked = d(&[("application_name", "Bot"), ("interaction_callback_type", "MODAL"), ("interaction_user_id", "77"), ("decision_outcome", "blocked")], "");
        assert_eq!(header(&blocked, true), Header::AppModal("sys.am_appmsg_modal_blocked"));
        let msg = d(&[("application_name", "Bot"), ("interaction_callback_type", "CHANNEL_MESSAGE_WITH_SOURCE"), ("decision_outcome", "blocked")], "");
        assert_eq!(header(&msg, true), Header::AppMessage("sys.am_appmsg_blocked"));
        let vc = d(&[("application_name", "Bot"), ("voice_channel_status_outcome", "flagged")], "");
        assert_eq!(header(&vc, true), Header::Channel("sys.am_vc_flagged"));
        let segs = parse_template(&crate::i18n::t_in("en", "sys.am_appmsg_modal_blocked", &[]), &[("ch", String::new()), ("app", "Bot".into())]);
        assert_eq!(
            segs,
            vec![
                Seg::Text("has blocked ".into()),
                Seg::Strong("Bot".into()),
                Seg::Text("'s modal from being shown to ".into()),
                Seg::Name("iu".into()),
                Seg::Text(". The interaction with ".into()),
                Seg::Strong("Bot".into()),
                Seg::Text(" was started by ".into()),
                Seg::Name("owner".into()),
                Seg::Text(".".into()),
            ]
        );
        let fr = parse_template(&crate::i18n::t_in("fr", "sys.am_appmsg_flagged", &[]), &[("ch", String::new()), ("app", "Bot".into())]);
        assert_eq!(fr, vec![Seg::Text("a signalé le message de ".into()), Seg::Strong("Bot".into()), Seg::Text(" dans ".into()), Seg::Link(0, String::new()), Seg::Text(", envoyé par ".into()), Seg::Name("owner".into())]);
    }

    #[test]
    fn raid_alert() {
        let mut x = d(&[("notification_type", "raid"), ("join_attempts", "42"), ("raid_datetime", "2026-11-25T10:00:00+00:00"), ("raid_type", "JOIN_RAID")], "");
        x.notification = true;
        assert_eq!(notice(&x), Notice::Raid);
        let rc = raid_card("fr", &x);
        assert!(!rc.dm);
        assert_eq!(rc.subheader, vec!["~42\u{a0}adhésions traitées".to_string()]);
        assert!(rc.began.is_some());
        assert_eq!(rc.resolve, "sys.am_resolve");
        let dm = d(&[("raid_type", "DM_RAID"), ("dms_sent", "7"), ("resolved_reason", "DM_SPAM")], "");
        let rc = raid_card("en", &dm);
        assert!(rc.dm && rc.subheader == vec!["~7 DMs Detected".to_string()]);
        assert_eq!(rc.resolve, "sys.am_resolved_dm");
        assert_eq!(notice(&d(&[], "")), Notice::Raid);
        assert_eq!(notice(&d(&[("notification_type", "mention_raid")], "")), Notice::MentionRaid);
        assert_eq!(notice(&d(&[("notification_type", "activity_alerts_enabled")], "")), Notice::AlertsEnabled);
        assert_eq!(notice(&d(&[("notification_type", "interaction_blocked")], "")), Notice::InteractionBlocked);
        assert_eq!(notice(&d(&[("notification_type", "new_thing")], "")), Notice::Unsupported);
    }

    #[test]
    fn highlight_needs_word_edges() {
        assert_eq!(highlight("nullement nul", Some("nul")), vec![("nullement ".into(), false), ("nul".into(), true)]);
        assert_eq!(highlight("annuler", Some("nul")), vec![("annuler".into(), false)]);
        assert_eq!(highlight("x", None), vec![("x".into(), false)]);
        assert_eq!(highlight("nul, nul et nul", Some("nul")), vec![("nul, ".into(), false), ("nul".into(), true), (" et ".into(), false), ("nul".into(), true)]);
        assert_eq!(highlight("nul nul", Some("nul")), vec![("nul".into(), true), (" ".into(), false), ("nul".into(), true)]);
        let many = vec!["x"; 14].join(" ");
        assert_eq!(highlight(&many, Some("x")).iter().filter(|(_, h)| *h).count(), 11);
    }

    #[test]
    fn only_automod_embeds_are_cards() {
        let mut m = Message::default();
        assert!(data(&m).is_none());
        m.embeds.push(crate::model::Embed { kind: Some("rich".into()), ..Default::default() });
        assert!(data(&m).is_none());
        m.embeds[0].kind = Some("auto_moderation_notification".into());
        m.embeds[0].description = Some("hello".into());
        let x = data(&m).unwrap();
        assert!(x.notification && x.content == "hello");
    }
}

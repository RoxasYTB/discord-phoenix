use gpui::{AnyElement, Entity, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use super::icons::ChatIcon;
use crate::{
    app::PhoenixApp,
    model::{Message, MessageKind},
    richtext::RenderedMessage,
    theme::{Palette, c, z},
};

fn join_text(id: u64) -> (SharedString, SharedString) {
    let n = (id >> 22) as usize % 6;
    (crate::i18n::t(&format!("sys.join{n}.before"), &[]), crate::i18n::t(&format!("sys.join{n}.after"), &[]))
}

const ASSET_JOIN: &str = "https://discord.com/assets/8b9de960da063cb9.svg";
const ASSET_LEAVE: &str = "https://discord.com/assets/53e42368b1409f1a.svg";
const ASSET_CALL: &str = "https://discord.com/assets/61ccab047dad737a.svg";
const ASSET_CALL_MISSED: &str = "https://discord.com/assets/6f076a64de25de4b.svg";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SysIcon {
    Asset(&'static str),
    Glyph(ChatIcon, u32),
    Named(&'static str, f32, Option<usize>),
    Live,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Humanized {
    Seconds,
    Minute,
    Minutes(i64),
    Hour,
    Hours(i64),
    Day,
    Days(i64),
    Month,
    Months(i64),
    Year,
    Years(i64),
}

pub fn humanize(ms: i64) -> Humanized {
    let ms = ms.abs() as f64;
    let days_f = ms / 86_400_000.;
    let months_f = days_f * 4800. / 146_097.;
    let (s, m, h, d, mo, y) = (
        (ms / 1000.).round() as i64,
        (ms / 60_000.).round() as i64,
        (ms / 3_600_000.).round() as i64,
        days_f.round() as i64,
        months_f.round() as i64,
        (months_f / 12.).round() as i64,
    );
    if s <= 44 {
        Humanized::Seconds
    } else if m <= 1 {
        Humanized::Minute
    } else if m < 45 {
        Humanized::Minutes(m)
    } else if h <= 1 {
        Humanized::Hour
    } else if h < 22 {
        Humanized::Hours(h)
    } else if d <= 1 {
        Humanized::Day
    } else if d < 26 {
        Humanized::Days(d)
    } else if mo <= 1 {
        Humanized::Month
    } else if mo < 11 {
        Humanized::Months(mo)
    } else if y <= 1 {
        Humanized::Year
    } else {
        Humanized::Years(y)
    }
}

pub fn humanize_text(ms: i64) -> SharedString {
    humanize_in(crate::i18n::locale(), ms)
}

pub fn humanize_in(code: &str, ms: i64) -> SharedString {
    let (key, n) = match humanize(ms) {
        Humanized::Seconds => ("sys.dur.s", None),
        Humanized::Minute => ("sys.dur.m", None),
        Humanized::Minutes(n) => ("sys.dur.mm", Some(n)),
        Humanized::Hour => ("sys.dur.h", None),
        Humanized::Hours(n) => ("sys.dur.hh", Some(n)),
        Humanized::Day => ("sys.dur.d", None),
        Humanized::Days(n) => ("sys.dur.dd", Some(n)),
        Humanized::Month => ("sys.dur.mo", None),
        Humanized::Months(n) => ("sys.dur.mos", Some(n)),
        Humanized::Year => ("sys.dur.y", None),
        Humanized::Years(n) => ("sys.dur.yy", Some(n)),
    };
    match n {
        Some(n) => crate::i18n::t_in(code, key, &[("n", &n.to_string())]),
        None => crate::i18n::t_in(code, key, &[]),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallState {
    Missed,
    Started,
    Joinable,
}

pub fn call_state(call: Option<&crate::model::CallInfo>, active: bool, me: crate::model::Id, in_this_call: bool) -> CallState {
    if !active && call.is_some_and(|c| !c.participants.contains(&me)) {
        CallState::Missed
    } else if active && !in_this_call {
        CallState::Joinable
    } else {
        CallState::Started
    }
}

const ASSET_DISCOVERY_OUT: &str = "https://discord.com/assets/49dd33256719a93e.svg";
const ASSET_DISCOVERY_BACK: &str = "https://discord.com/assets/5c56b22c17556059.svg";

const FALLBACK_TYPES: [u8; 8] = [24, 35, 48, 49, 50, 51, 64, 65];

const AUTOMOD_TYPES: [u8; 4] = [36, 37, 38, 39];

const AUTOMOD_AVATAR: &str = "https://discord.com/assets/32336875baac6ac1.png";

pub const INTEGRATIONS_ARTICLE: &str = "15104189280151";

#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Text(String),
    Author,
    Strong(String),
    Name(String),
    Link(usize, String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SysLink {
    EditGroup,
    Guild,
    GuildSettings,
    Followed,
    JoinStream,
    HelpCenter,
    Inert,
}

#[derive(Clone, Debug, Default)]
pub struct SysCtx<'a> {
    pub type_id: u8,
    pub content: &'a str,
    pub group_dm: bool,
    pub forum_post: bool,
    pub guild_name: Option<&'a str>,
    pub data: Option<&'a crate::model::SysData>,
    pub stream: Option<StreamRef>,
    pub mine: bool,
    pub other: Option<String>,
    pub time: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StreamRef {
    pub channel: String,
    pub activity: Option<String>,
    pub ended_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Generic {
    pub icon: SysIcon,
    pub segs: Vec<Seg>,
    pub links: &'static [SysLink],
    pub strong_text: bool,
}

pub fn parse_template(tpl: &str, vars: &[(&str, String)]) -> Vec<Seg> {
    let fill = |s: &str| {
        let mut o = String::new();
        let mut rest = s;
        while let Some(i) = rest.find('{') {
            o.push_str(&rest[..i]);
            let tail = &rest[i..];
            match tail.find('}').and_then(|j| vars.iter().find(|(k, _)| **k == tail[1..j]).map(|(_, v)| (j, v))) {
                Some((j, v)) => {
                    o.push_str(v);
                    rest = &tail[j + 1..];
                }
                None => {
                    o.push('{');
                    rest = &tail[1..];
                }
            }
        }
        o.push_str(rest);
        o
    };
    let mut out = Vec::new();
    let mut rest = tpl;
    let mut links = 0;
    while !rest.is_empty() {
        let next = ["{u}", "**", "<", "["].iter().filter_map(|m| rest.find(m).map(|i| (i, *m))).min_by_key(|(i, _)| *i);
        let Some((i, m)) = next else {
            out.push(Seg::Text(fill(rest)));
            break;
        };
        if i > 0 {
            out.push(Seg::Text(fill(&rest[..i])));
        }
        rest = &rest[i + m.len()..];
        let close = match m {
            "{u}" => {
                out.push(Seg::Author);
                continue;
            }
            "**" => "**",
            "<" => ">",
            _ => "]",
        };
        let end = rest.find(close).unwrap_or(rest.len());
        let inner = fill(&rest[..end]);
        out.push(match m {
            "**" => Seg::Strong(inner),
            "<" => Seg::Name(inner),
            _ => {
                links += 1;
                Seg::Link(links - 1, inner)
            }
        });
        rest = rest.get(end + close.len()..).unwrap_or("");
    }
    out.retain(|s| !matches!(s, Seg::Text(t) if t.is_empty()));
    out
}

pub fn boost_count(content: &str) -> u32 {
    match content.trim().parse::<f64>() {
        Ok(n) if n.is_finite() && n >= 1. => n as u32,
        _ => 1,
    }
}

pub fn effective_type(msg: &Message) -> u8 {
    match msg.kind {
        MessageKind::Boost(t) => 8 + t.min(3),
        MessageKind::ChannelNameChange => 4,
        MessageKind::StageStart => 27,
        MessageKind::StageEnd => 28,
        MessageKind::StageSpeaker => 29,
        MessageKind::StageTopic => 31,
        _ => msg.type_id,
    }
}

pub fn generic_in(code: &str, ctx: &SysCtx) -> Option<Generic> {
    use crate::theme::generated::idx;
    use SysLink::*;
    let tr = |k: &str| crate::i18n::t_in(code, k, &[]).to_string();
    let pen = SysIcon::Named("chat/edit", 12., None);
    let stage = |tint: Option<usize>| SysIcon::Named("stage", 24., tint);
    let warn = SysIcon::Named("settings/circle-error", 16., Some(idx::TEXT_FEEDBACK_WARNING));
    let guild = ctx.guild_name.unwrap_or("this server").to_string();
    let n = boost_count(ctx.content);
    let mut vars: Vec<(&str, String)> = vec![("c", ctx.content.to_string()), ("g", guild), ("n", n.to_string())];
    let (icon, key, links, strong_text): (SysIcon, &str, &'static [SysLink], bool) = match ctx.type_id {
        4 if ctx.group_dm && ctx.content.is_empty() => (pen, "sys.rename_group_clear", &[EditGroup], false),
        4 if ctx.group_dm => (pen, "sys.rename_group", &[EditGroup], false),
        4 if ctx.forum_post => (pen, "sys.rename_post", &[], false),
        4 => (pen, "sys.rename", &[], false),
        5 => (pen, "sys.group_icon", &[EditGroup], false),
        8..=11 => {
            let gem = SysIcon::Named("chat/boost", 16., Some(idx::GUILD_BOOSTING_PINK));
            let tier = ctx.type_id - 8;
            if tier == 0 || ctx.guild_name.is_none() {
                (gem, if n > 1 { "sys.boost_n" } else { "sys.boost" }, &[], false)
            } else {
                vars.push(("t", tr(&format!("sys.boost_tier{tier}"))));
                (gem, if n > 1 { "sys.boost_tier_n" } else { "sys.boost_tier" }, &[], false)
            }
        }
        12 => (SysIcon::Asset(ASSET_JOIN), "sys.follow_add", &[Followed], false),
        14 => (SysIcon::Asset(ASSET_DISCOVERY_OUT), "sys.discovery_out", &[Guild, GuildSettings], false),
        15 => (SysIcon::Asset(ASSET_DISCOVERY_BACK), "sys.discovery_back", &[], false),
        16 => (warn, "sys.discovery_warn1", &[Guild], false),
        17 => (warn, "sys.discovery_warn3", &[Guild], false),
        27 => (stage(Some(idx::GREEN_360)), "sys.stage_start", &[], true),
        28 => (stage(None), "sys.stage_end", &[], true),
        29 => (SysIcon::Named("sys/mic-arrow-right", 24., None), "sys.stage_speaker", &[], true),
        30 => (SysIcon::Named("sys/hand-request-speak", 24., None), "sys.stage_hand", &[], true),
        31 => (stage(None), "sys.stage_topic", &[], true),
        13 => {
            let st = ctx.stream.as_ref()?;
            vars.push(("ch", st.channel.clone()));
            match st.ended_ms {
                Some(ms) => {
                    vars.push(("d", humanize_in(code, ms).to_string()));
                    (SysIcon::Named("voice/screen-x", 20., None), "sys.stream_ended", &[], false)
                }
                None => {
                    vars.push(("act", st.activity.clone().unwrap_or_else(|| "unknown".into())));
                    (SysIcon::Live, "sys.stream_live", &[JoinStream], false)
                }
            }
        }
        25 => {
            let d = ctx.data?;
            vars.push(("t", d.tier_name.as_deref().unwrap_or_default().to_string()));
            vars.push(("m", d.months.to_string()));
            let key = match (d.months > 0, d.renewal) {
                (true, true) if d.months == 1 => "sys.role_sub_renew_months1",
                (true, true) => "sys.role_sub_renew_months",
                (true, false) if d.months == 1 => "sys.role_sub_months1",
                (true, false) => "sys.role_sub_months",
                (false, true) => "sys.role_sub_renew",
                (false, false) => "sys.role_sub",
            };
            (SysIcon::Asset(ASSET_JOIN), key, &[Inert], false)
        }
        32 => match ctx.data.and_then(|d| d.app_name.as_deref()) {
            Some(a) => {
                vars.push(("a", a.to_string()));
                (SysIcon::Named("sys/ticket", 24., None), "sys.app_premium", &[], false)
            }
            None => (SysIcon::Named("sys/ticket", 24., None), "sys.app_premium_deleted", &[], false),
        },
        33 | 34 => {
            let added = ctx.type_id == 33;
            let icon = SysIcon::Asset(if added { ASSET_JOIN } else { ASSET_LEAVE });
            match ctx.data.and_then(|d| d.app_name.as_deref()) {
                Some(a) => {
                    vars.push(("a", a.to_string()));
                    (icon, if added { "sys.app_added" } else { "sys.app_removed" }, &[Inert, HelpCenter], false)
                }
                None => (icon, if added { "sys.app_added_deleted" } else { "sys.app_removed_deleted" }, &[HelpCenter], false),
            }
        }
        36 => {
            vars.push(("time", ctx.time.clone()));
            (SysIcon::Asset(AUTOMOD_AVATAR), "sys.alert_on", &[], false)
        }
        37 => (SysIcon::Asset(AUTOMOD_AVATAR), "sys.alert_off", &[], false),
        38 => (SysIcon::Asset(AUTOMOD_AVATAR), "sys.alert_raid", &[], false),
        39 => (SysIcon::Asset(AUTOMOD_AVATAR), "sys.alert_resolved", &[], false),
        44 => {
            vars.push(("p", ctx.data.and_then(|d| d.product.as_deref())?.to_string()));
            (SysIcon::Asset(ASSET_JOIN), "sys.purchase", &[Inert], false)
        }
        52 => (SysIcon::Asset(ASSET_JOIN), "sys.join_request_approved", &[], false),
        53 => (SysIcon::Asset(ASSET_LEAVE), "sys.join_request_rejected", &[], false),
        54 => (SysIcon::Asset(ASSET_LEAVE), "sys.join_request_withdrawn", &[], false),
        58..=62 => {
            let (icon, key) = match ctx.type_id {
                58 => ("chat/trash", "sys.report_deleted"),
                59 => ("mod/clock-warning", "sys.report_timeout"),
                60 => ("mod/user-minus", "sys.report_kick"),
                61 => ("mod/hammer", "sys.report_ban"),
                _ => ("check-small", "sys.report_closed"),
            };
            vars.push(("o", ctx.other.clone().unwrap_or_default()));
            (SysIcon::Named(icon, 20., None), key, &[], false)
        }
        67 => {
            vars.push(("o", ctx.other.clone()?));
            vars.push(("note", ctx.content.to_string()));
            let key = match (ctx.mine, ctx.content.is_empty()) {
                (true, false) => "sys.friend_accepted_by_me_note",
                (false, false) => "sys.friend_accepted_note",
                (true, true) => "sys.friend_accepted_by_me",
                (false, true) => "sys.friend_accepted",
            };
            (SysIcon::Named("user-check", 18., Some(idx::STATUS_POSITIVE)), key, &[], false)
        }
        _ => return None,
    };
    Some(Generic { icon, segs: parse_template(&tr(key), &vars), links, strong_text })
}

fn split_words(text: &str) -> Vec<String> {
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

fn author_name(app: &PhoenixApp, msg: &Message) -> SharedString {
    msg.author_nick
        .clone()
        .or_else(|| app.member_nick(msg.author))
        .or_else(|| app.users.get(&msg.author).map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("sys.someone"))
}

fn author_el(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message) -> AnyElement {
    let (name_color, name_gradient) = app.author_role_style(msg.author);
    let name_color = name_color.map(c).unwrap_or_else(|| c(Palette::TEXT_HEADER));
    let author = app.users.get(&msg.author);
    crate::fonts::styled_name(
        div().font_weight(FontWeight::MEDIUM).text_color(name_color).cursor_pointer().hover(|s| s.underline()),
        author,
        false,
    )
    .id(("sys-author", msg.id.0))
    .on_click(super::message_ext::open_user(entity, msg.author, msg.channel))
    .child(crate::fonts::role_name_text(author_name(app, msg), author, 16., app.selected_guild.is_some(), name_gradient))
    .into_any_element()
}

fn icon_el(glyph: SysIcon) -> AnyElement {
    match glyph {
        SysIcon::Named(name, size, tint) => div()
            .absolute()
            .left(z(44. - size / 2.))
            .top(z(15. - size / 2.))
            .child(
                gpui::svg()
                    .path(crate::icons::path(name))
                    .size(z(size))
                    .text_color(tint.map_or_else(|| c(Palette::TEXT_MUTED), |t| c(crate::theme::tok(t)))),
            )
            .into_any_element(),
        SysIcon::Asset(url) => div().absolute().left(z(36.)).top(z(7.)).child(gpui::img(SharedString::new_static(url)).size(z(16.))).into_any_element(),
        SysIcon::Glyph(icon, color) => div().absolute().left(z(36.)).top(z(7.)).child(super::icons::icon(icon, 16., c(color))).into_any_element(),
        SysIcon::Live => div()
            .absolute()
            .left(z(8.))
            .top(z(2.))
            .w(z(72.))
            .h(z(22.))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .h(z(16.))
                    .px(z(4.))
                    .rounded(z(8.))
                    .bg(c(crate::theme::tok(crate::theme::generated::idx::STATUS_DANGER)))
                    .flex()
                    .items_center()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(gpui::white())
                    .child(crate::t!("voice.live")),
            )
            .into_any_element(),
    }
}

fn generic_line(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &RenderedMessage, g: Generic) -> AnyElement {
    use crate::theme::{generated::idx, tok};
    let text_color = if g.strong_text { c(tok(idx::TEXT_STRONG)) } else { c(Palette::TEXT_MUTED) };
    let guild = app.channel_guild(msg.channel);
    let channel = msg.channel;
    let stream_key = msg.sys.as_ref().and_then(|d| d.ref_channel).map(|ch| crate::voice::stream::key(msg.sys.as_ref().and_then(|d| d.ref_guild).map(|g| g.0), ch.0, msg.author.0));
    let mut children: Vec<AnyElement> = Vec::new();
    for seg in g.segs {
        match seg {
            Seg::Text(t) => children.extend(split_words(&t).into_iter().map(|w| div().child(w).into_any_element())),
            Seg::Strong(t) => children.extend(split_words(&t).into_iter().map(|w| div().font_weight(FontWeight::SEMIBOLD).child(w).into_any_element())),
            Seg::Name(t) => children.extend(
                split_words(&t).into_iter().map(|w| div().font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(w).into_any_element()),
            ),
            Seg::Author => children.push(author_el(app, entity, msg)),
            Seg::Link(i, label) => {
                let kind = g.links.get(i).copied();
                let color = if kind == Some(SysLink::EditGroup) { idx::TEXT_LINK } else { idx::INTERACTIVE_TEXT_ACTIVE };
                let e = entity.clone();
                let sk = stream_key.clone();
                let el = div()
                    .id(("sys-link", msg.id.0.wrapping_mul(8).wrapping_add(i as u64)))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(color)))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(label)
                    .on_click(move |_, _, cx| match kind {
                        Some(SysLink::EditGroup) => e.update(cx, |a, cx| a.open_edit_group(channel, cx)),
                        Some(SysLink::Guild) => {
                            if let Some(gid) = guild {
                                e.update(cx, |a, cx| a.select_guild(Some(gid), cx));
                            }
                        }
                        Some(SysLink::GuildSettings) => {
                            if let Some(gid) = guild {
                                let section = crate::ui::modals::server_settings::section_index("menu.gs.access");
                                e.update(cx, |a, cx| a.open_server_settings(gid, section, cx));
                            }
                        }
                        Some(SysLink::JoinStream) => {
                            if let Some(key) = sk.clone() {
                                e.update(cx, |a, cx| crate::ui::voice_row::watch_from_sidebar(a, key, cx));
                            }
                        }
                        Some(SysLink::HelpCenter) => {
                            cx.open_url(&format!("https://support.discord.com/hc/{}/articles/{INTEGRATIONS_ARTICLE}", crate::i18n::locale().to_lowercase()));
                        }
                        Some(SysLink::Followed | SysLink::Inert) | None => {}
                    });
                children.push(el.into_any_element());
            }
        }
    }
    div()
        .id(("sysmsg", msg.id.0))
        .relative()
        .pl(z(80.))
        .pr(z(48.))
        .py(z(2.))
        .mt(z(if r.group_start { 17. } else { 0. }))
        .min_h(z(22.))
        .flex()
        .flex_wrap()
        .items_center()
        .text_size(z(16.))
        .line_height(z(22.))
        .text_color(text_color)
        .hover(|s| s.bg(c(Palette::BG_MSG_HOVER)))
        .child(icon_el(g.icon))
        .children(children)
        .child(div().ml(z(6.)).text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)).child(r.time_full.clone()))
        .into_any_element()
}

fn automod_line(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &RenderedMessage, g: Generic) -> AnyElement {
    use crate::theme::{generated::idx, tok};
    let mut children: Vec<AnyElement> = Vec::new();
    for seg in g.segs {
        match seg {
            Seg::Author => children.push(author_el(app, entity, msg)),
            Seg::Text(t) | Seg::Strong(t) | Seg::Name(t) | Seg::Link(_, t) => children.extend(split_words(&t).into_iter().map(|w| div().child(w).into_any_element())),
        }
    }
    let header = div()
        .flex()
        .items_center()
        .h(z(22.))
        .child(div().mr(z(4.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_BRAND))).child(crate::t!("sys.automod")))
        .child(super::bot_tag_label(true, crate::t!("sys.official")).ml_0().mr(z(2.)).mt(z(2.)))
        .child(div().ml(z(4.)).text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)).child(r.time_full.clone()));
    div()
        .id(("sysmsg", msg.id.0))
        .relative()
        .pl(z(80.))
        .pr(z(48.))
        .py(z(2.))
        .mt(z(if r.group_start { 17. } else { 0. }))
        .min_h(z(44.))
        .flex()
        .flex_col()
        .text_size(z(16.))
        .line_height(z(22.))
        .hover(|s| s.bg(c(Palette::BG_MSG_HOVER)))
        .child(div().absolute().left(z(24.)).top(z(2.)).child(gpui::img(SharedString::new_static(AUTOMOD_AVATAR)).size(z(40.)).rounded_full()))
        .child(header)
        .child(div().flex().flex_wrap().items_center().line_height(z(24.)).text_color(c(tok(idx::TEXT_DEFAULT))).children(children))
        .into_any_element()
}

fn with_divider(r: &RenderedMessage, line: AnyElement) -> AnyElement {
    match r.day_divider.clone() {
        Some(label) => div().child(super::day_divider(label)).child(line).into_any_element(),
        None => line,
    }
}

fn user_name(app: &PhoenixApp, uid: crate::model::Id) -> String {
    app.member_nick(uid).or_else(|| app.users.get(&uid).map(|u| u.display_name.clone())).unwrap_or_else(|| crate::t!("sys.someone")).to_string()
}

pub fn format_alert(code: &str, t: chrono::NaiveDateTime) -> String {
    if code.starts_with("en") { t.format("%m/%d/%Y, %-I:%M %p").to_string() } else { t.format("%d/%m/%Y %H:%M").to_string() }
}

pub fn alert_time(content: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(content.trim())
        .ok()
        .map(|d| format_alert(crate::i18n::locale(), d.with_timezone(&chrono::Local).naive_local()))
        .unwrap_or_default()
}

pub fn ctx_of<'a>(app: &'a PhoenixApp, msg: &'a Message) -> SysCtx<'a> {
    let guild = app.channel_guild(msg.channel);
    let data = msg.sys.as_deref();
    let type_id = effective_type(msg);
    let stream = (type_id == 13)
        .then(|| data.and_then(|d| d.ref_guild.and(d.ref_channel)))
        .flatten()
        .and_then(|ch| app.channels.get(&ch))
        .map(|ch| StreamRef {
            channel: ch.name.to_string(),
            activity: app
                .people
                .activities
                .get(&msg.author)
                .and_then(|v| v.iter().find(|a| matches!(a.kind, crate::presence::ActivityKind::Playing)))
                .map(|a| a.name.to_string()),
            ended_ms: msg.call.as_ref().and_then(|c| c.duration_ms),
        });
    let other = match type_id {
        58..=62 => data.and_then(|d| d.mention).map(|u| user_name(app, u)),
        67 => app
            .channels
            .get(&msg.channel)
            .filter(|c| c.guild.is_none() && c.voice_users.len() == 1)
            .and_then(|c| c.voice_users.first().copied())
            .map(|u| user_name(app, u)),
        _ => None,
    };
    SysCtx {
        type_id,
        content: msg.content.as_ref(),
        group_dm: guild.is_none() && crate::ui::dm_profile_panel::is_group_dm(app, msg.channel),
        forum_post: guild.is_some() && app.thread_state(msg.channel).forum_post,
        guild_name: guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| g.name.as_ref()),
        data,
        stream,
        mine: msg.author == app.me.id,
        other,
        time: if type_id == 36 { alert_time(&msg.content) } else { String::new() },
    }
}

pub fn is_system(kind: MessageKind) -> bool {
    !matches!(kind, MessageKind::Default | MessageKind::Reply)
}

pub fn render(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &RenderedMessage) -> Option<AnyElement> {
    if !is_system(msg.kind) {
        return None;
    }
    if let MessageKind::PollResult(poll) = msg.kind {
        let line = poll_result(app, entity, msg, r, poll);
        return Some(with_divider(r, line));
    }
    if let Some(g) = generic_in(crate::i18n::locale(), &ctx_of(app, msg)) {
        let line = if AUTOMOD_TYPES.contains(&msg.type_id) { automod_line(app, entity, msg, r, g) } else { generic_line(app, entity, msg, r, g) };
        return Some(with_divider(r, line));
    }
    if msg.kind == MessageKind::OtherSystem && (msg.content.is_empty() || !FALLBACK_TYPES.contains(&msg.type_id)) {
        return Some(match r.day_divider.clone() {
            Some(label) => super::day_divider(label).into_any_element(),
            None => div().into_any_element(),
        });
    }
    let target = |t: Option<crate::model::Id>| t.filter(|t| *t != msg.author);
    let other: Option<(crate::model::Id, SharedString, SharedString)> = match msg.kind {
        MessageKind::RecipientAdd(t) | MessageKind::RecipientRemove(t) => target(t).map(|t| {
            let n = app.users.get(&t).map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("sys.someone"));
            let tail = if matches!(msg.kind, MessageKind::RecipientAdd(_)) { "sys.recipient_add_tail" } else { "sys.recipient_kick_tail" };
            (t, n, crate::i18n::t(tail, &[]))
        }),
        _ => None,
    };
    let call = (msg.kind == MessageKind::Call).then(|| {
        let active = app.parity.call_messages.get(&msg.channel) == Some(&msg.id);
        call_state(msg.call.as_ref(), active, app.me.id, app.voice_channel == Some(msg.channel))
    });
    let (icon, color, before, after): (ChatIcon, u32, SharedString, SharedString) = match msg.kind {
        MessageKind::Join => {
            let (b, a) = join_text(msg.id.0);
            (ChatIcon::Join, Palette::GREEN, b, a)
        }
        MessageKind::Pin => (ChatIcon::Join, Palette::INTERACTIVE, "".into(), crate::t!("sys.pin_lead")),
        MessageKind::ThreadCreated => (
            ChatIcon::Thread,
            Palette::INTERACTIVE,
            "".into(),
            crate::t!("sys.thread_prefix"),
        ),
        MessageKind::RecipientAdd(t) => match target(t) {
            Some(_) => (ChatIcon::Join, Palette::GREEN, "".into(), crate::t!("sys.recipient_add_named")),
            None => (ChatIcon::Join, Palette::GREEN, "".into(), crate::t!("sys.recipient_add")),
        },
        MessageKind::RecipientRemove(t) => match target(t) {
            Some(_) => (ChatIcon::Join, Palette::RED, "".into(), crate::t!("sys.recipient_kick_named")),
            None => (ChatIcon::Join, Palette::RED, "".into(), crate::t!("sys.recipient_remove")),
        },
        MessageKind::Call => {
            let dur = msg.call.as_ref().and_then(|c| c.duration_ms).map(humanize_text);
            match (call, dur) {
                (Some(CallState::Missed), Some(d)) => (ChatIcon::Join, Palette::GREEN, crate::t!("sys.call_missed_lead"), crate::t!("sys.call_missed_lasted", d = d)),
                (Some(CallState::Missed), None) => (ChatIcon::Join, Palette::GREEN, crate::t!("sys.call_missed_lead"), ".".into()),
                (_, Some(d)) => (ChatIcon::Join, Palette::GREEN, "".into(), crate::t!("sys.call_lasted", d = d)),
                (_, None) => (ChatIcon::Join, Palette::GREEN, "".into(), crate::t!("sys.call")),
            }
        }
        _ => (
            ChatIcon::More,
            Palette::INTERACTIVE,
            "".into(),
            if msg.content.is_empty() { "".into() } else { format!(": {}", msg.content).into() },
        ),
    };
    let glyph = match msg.kind {
        MessageKind::Pin => SysIcon::Named("pin", 18., None),
        MessageKind::Join | MessageKind::RecipientAdd(_) => SysIcon::Asset(ASSET_JOIN),
        MessageKind::RecipientRemove(_) => SysIcon::Asset(ASSET_LEAVE),
        MessageKind::Call if call == Some(CallState::Missed) => SysIcon::Asset(ASSET_CALL_MISSED),
        MessageKind::Call => SysIcon::Asset(ASSET_CALL),
        _ => SysIcon::Glyph(icon, color),
    };
    let joinable = call == Some(CallState::Joinable);
    let thread = msg.thread.clone().filter(|_| msg.kind == MessageKind::ThreadCreated);
    let line = div()
        .id(("sysmsg", msg.id.0))
        .relative()
        .pl(z(80.))
        .pr(z(48.))
        .py(z(2.))
        .mt(z(if r.group_start { 17. } else { 0. }))
        .min_h(z(22.))
        .flex()
        .flex_wrap()
        .items_center()
        .text_size(z(16.))
        .line_height(z(22.))
        .text_color(c(Palette::TEXT_MUTED))
        .hover(|s| s.bg(c(Palette::BG_MSG_HOVER)))
        .child(icon_el(glyph))
        .children(flow(before))
        .child(author_el(app, entity, msg))
        .children(flow(after))
        .when_some(thread.clone(), |d, t| {
            let (e1, e2) = (entity.clone(), entity.clone());
            let link = |id: (&'static str, u64), text: SharedString| {
                div()
                    .id(id)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_TEXT_ACTIVE)))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(text)
            };
            let t2 = t.clone();
            d.child(link(("sys-thread-name", msg.id.0), t.name.clone()).on_click(move |_, w, cx| {
                let t = t2.clone();
                e1.update(cx, |a, cx| a.open_thread_link(t, w, cx));
            }))
            .children(flow(crate::t!("sys.thread_see_all")))
            .child(link(("sys-thread-all", msg.id.0), crate::t!("sys.thread_all_link")).on_click(move |_, _, cx| {
                e2.update(cx, |a, cx| a.toggle_threads_popout(cx));
            }))
            .child(".")
        })
        .when(thread.is_none() && msg.kind == MessageKind::ThreadCreated && !msg.content.is_empty(), |d| {
            let e2 = entity.clone();
            let active = c(crate::theme::tok(crate::theme::generated::idx::INTERACTIVE_TEXT_ACTIVE));
            d.child(div().font_weight(FontWeight::MEDIUM).text_color(active).child(msg.content.clone()))
                .children(flow(crate::t!("sys.thread_see_all")))
                .child(
                    div()
                        .id(("sys-thread-all", msg.id.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(active)
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .child(crate::t!("sys.thread_all_link"))
                        .on_click(move |_, _, cx| {
                            e2.update(cx, |a, cx| a.toggle_threads_popout(cx));
                        }),
                )
                .child(".")
        })
        .when(msg.kind == MessageKind::Pin, |d| {
            let (e1, e2) = (entity.clone(), entity.clone());
            let (ch, pinned) = (msg.channel, msg.reply_to);
            let link = |id: (&'static str, u64), text: SharedString| {
                div()
                    .id(id)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(crate::theme::tok(crate::theme::generated::idx::TEXT_STRONG)))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(text)
            };
            d.child(link(("sys-pin-msg", msg.id.0), crate::t!("sys.pin_link")).on_click(move |_, _, cx| {
                if let Some(id) = pinned {
                    e1.update(cx, |a, cx| a.jump_to_message(ch, id, cx));
                }
            }))
            .children(flow(crate::t!("sys.pin_tail")))
            .child(link(("sys-pin-all", msg.id.0), crate::t!("sys.pin_all")).on_click(move |_, _, cx| {
                e2.update(cx, |a, cx| a.toggle_pins(cx));
            }))
            .child(".")
        })
        .when_some(other, |d, (uid, n, tail)| {
            d.child(
                div()
                    .id(("sys-target", msg.id.0))
                    .on_click(super::message_ext::open_user(entity, uid, msg.channel))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(Palette::TEXT_HEADER))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(n),
            )
            .children(flow(tail))
        })
        .when(joinable, |d| {
            let e = entity.clone();
            let ch = msg.channel;
            d.child(div().ml(z(3.)).child("\u{2014}")).child(
                div()
                    .id(("sys-join-call", msg.id.0))
                    .ml(z(3.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(crate::theme::tok(crate::theme::generated::idx::TEXT_LINK)))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(crate::t!("sys.call_join"))
                    .on_click(move |_, _, cx| {
                        e.update(cx, |a, cx| {
                            a.join_voice(Some(ch), cx);
                            crate::ui::call_view::show(ch);
                            cx.notify();
                        });
                    }),
            )
        })
        .child(
            div()
                .ml(z(6.))
                .text_size(z(12.))
                .text_color(c(Palette::TEXT_MUTED))
                .child(r.time_full.clone()),
        );
    let cta = if msg.kind == MessageKind::Join { welcome_cta(app, entity, msg) } else { None };
    let line: AnyElement = match (cta, thread) {
        (Some(b), _) => div().child(line).child(b).into_any_element(),
        (None, Some(t)) => div()
            .relative()
            .child(line)
            .child(div().pl(z(80.)).pr(z(30.)).pb(z(2.)).flex().child(super::message_ext::thread_chip(app, entity, &t, true)))
            .child(super::message_ext::thread_spine(28. + if r.group_start { 17. } else { 0. }))
            .into_any_element(),
        (None, None) => line.into_any_element(),
    };
    Some(match r.day_divider.clone() {
        Some(label) => div().child(super::day_divider(label)).child(line).into_any_element(),
        None => line,
    })
}

const WAVE_STICKERS: [(u64, bool); 6] = [
    (749054660769218631, false),
    (751606379340365864, false),
    (754108890559283200, false),
    (781291131828699156, true),
    (816087792291282944, false),
    (819128604311027752, false),
];

fn wave_sticker(me: u64, msg: u64) -> (u64, bool) {
    WAVE_STICKERS[((me >> 22) + (msg >> 22)) as usize % WAVE_STICKERS.len()]
}

fn welcome_cta(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message) -> Option<AnyElement> {
    let bot = app.users.get(&msg.author).is_some_and(|u| u.bot);
    let blocked = app.nav.extras.get(&msg.channel).is_some_and(|x| x.send_blocked);
    if bot || blocked || msg.author == app.me.id {
        return None;
    }
    let (sticker, apng) = wave_sticker(app.me.id.0, msg.id.0);
    let label = crate::t!("sys.wave");
    let icon = if apng {
        gpui::img(SharedString::from(format!("https://media.discordapp.net/stickers/{sticker}.png?size=40")))
    } else {
        crate::media::lottie::image(crate::media::lottie::url(sticker), 40, true)
    };
    let button = crate::ui::controls::button(("wave-cta", msg.id.0), SharedString::default(), crate::ui::controls::BtnKind::Secondary)
        .h(z(32.))
        .px(z(12.))
        .text_size(z(14.))
        .gap(z(4.))
        .child(icon.flex_none().size(z(20.)))
        .child(label);
    let (e, channel) = (entity.clone(), msg.channel);
    Some(
        div()
            .flex()
            .pl(z(80.))
            .pt(z(4.))
            .mb(z(2.))
            .child(button.on_click(move |_, _, cx| {
                e.update(cx, |a, cx| crate::ui::pickers::detach(a.backend.send_sticker(channel, crate::model::Id(sticker)), cx));
            }))
            .into_any_element(),
    )
}

fn poll_field<'a>(msg: &'a Message, name: &str) -> Option<&'a str> {
    msg.embeds.first()?.fields.iter().find(|f| f.name.as_ref() == name).map(|f| f.value.as_ref())
}

pub fn truncate_question(q: &str) -> String {
    if q.chars().count() > 50 { q.chars().take(49).collect::<String>() + "\u{2026}" } else { q.to_string() }
}

fn poll_result(app: &PhoenixApp, entity: &Entity<PhoenixApp>, msg: &Message, r: &RenderedMessage, poll: Option<crate::model::Id>) -> AnyElement {
    use crate::theme::{generated::idx, tok};
    let tk = |i: usize| c(tok(i));
    let name: SharedString = msg
        .author_nick
        .clone()
        .or_else(|| app.member_nick(msg.author))
        .or_else(|| app.users.get(&msg.author).map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("sys.someone"));
    let (name_color, name_gradient) = app.author_role_style(msg.author);
    let name_color = name_color.map(c).unwrap_or_else(|| tk(idx::INTERACTIVE_TEXT_ACTIVE));
    let question = truncate_question(poll_field(msg, "poll_question_text").unwrap_or(""));
    let votes: u32 = poll_field(msg, "victor_answer_votes").and_then(|v| v.parse().ok()).unwrap_or(0);
    let total: u32 = poll_field(msg, "total_votes").and_then(|v| v.parse().ok()).unwrap_or(0);
    let victor = poll_field(msg, "victor_answer_text").map(str::to_string);
    let emoji = poll_field(msg, "victor_answer_emoji_name").map(str::to_string);
    let emoji_id = poll_field(msg, "victor_answer_emoji_id").map(str::to_string);
    let (ch, e1, e2) = (msg.channel, entity.clone(), entity.clone());
    let link = |id: (&'static str, u64), text: SharedString, color: gpui::Hsla| {
        div().id(id).font_weight(FontWeight::MEDIUM).text_color(color).cursor_pointer().hover(|s| s.underline()).child(text)
    };
    let line = div()
        .id(("sysmsg", msg.id.0))
        .relative()
        .pl(z(80.))
        .pr(z(48.))
        .py(z(2.))
        .min_h(z(22.))
        .flex()
        .flex_wrap()
        .items_center()
        .text_size(z(16.))
        .line_height(z(22.))
        .text_color(c(Palette::TEXT_MUTED))
        .child(
            div().absolute().left(z(36.)).top(z(7.)).child(gpui::svg().path(crate::icons::path("attach/poll")).size(z(16.)).text_color(tk(idx::INTERACTIVE_ICON_DEFAULT))),
        )
        .children({
            let q = link(("sys-poll-q", msg.id.0), question.into(), tk(idx::INTERACTIVE_TEXT_ACTIVE))
                .on_click(move |_, _, cx| {
                    if let Some(p) = poll {
                        e1.update(cx, |a, cx| a.jump_to_message(ch, p, cx));
                    }
                })
                .into_any_element();
            let author_user = app.users.get(&msg.author);
            let author = crate::fonts::styled_name(
                div().id(("sys-poll-author", msg.id.0)).font_weight(FontWeight::MEDIUM).text_color(name_color).cursor_pointer().hover(|s| s.underline()),
                author_user,
                false,
            )
            .on_click(super::message_ext::open_user(entity, msg.author, msg.channel))
            .child(crate::fonts::role_name_text(name, author_user, 16., app.selected_guild.is_some(), name_gradient))
            .into_any_element();
            let t = |k: &str| crate::i18n::t(k, &[]).into_any_element();
            if crate::i18n::locale() == "en" {
                vec![author, t("sys.poll_closed_b"), q, t("sys.poll_closed_c")]
            } else {
                vec![t("sys.poll_closed_a"), q, t("sys.poll_closed_b"), author, t("sys.poll_closed_c")]
            }
        })
        .child(div().ml(z(6.)).text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)).child(r.time_full.clone()));
    let pct = if total == 0 { 0 } else { (votes as f64 * 100. / total as f64).round() as u32 };
    let emoji_el = emoji.map(|n| {
        let src: SharedString = match emoji_id {
            Some(id) => format!("https://cdn.discordapp.com/emojis/{id}.webp?size=48").into(),
            None => crate::media::emoji::twemoji_url(&n).unwrap_or_default(),
        };
        div()
            .size(z(40.))
            .flex_none()
            .rounded(z(8.))
            .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
            .flex()
            .items_center()
            .justify_center()
            .child(gpui::img(src).size(z(22.)))
    });
    let has_image = emoji_el.is_some();
    let description = if victor.is_some() && votes > 0 {
        div()
            .flex()
            .flex_col()
            .gap(z(2.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(tk(idx::TEXT_DEFAULT))
                            .child(victor.unwrap_or_default()),
                    )
                    .child(crate::ui::components::voted_check(16. * crate::theme::zoom(), true, true)),
            )
            .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("sys.poll_winner", pct = pct)))
    } else {
        div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_DEFAULT)).child(crate::t!("sys.poll_no_winner"))
    };
    let card = div()
        .ml(z(80.))
        .mt(z(8.))
        .mb(z(2.))
        .w_full()
        .max_w(z(376. + (if has_image { 24. } else { 32. }) + 2.))
        .py(z(8.))
        .pl(z(if has_image { 8. } else { 16. }))
        .pr(z(16.))
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .flex()
        .items_center()
        .gap(z(8.))
        .children(emoji_el)
        .child(div().flex_1().min_w_0().overflow_hidden().child(description))
        .child(crate::ui::controls::button_sm(("sys-poll-view", msg.id.0), crate::t!("sys.poll_view"), crate::ui::controls::BtnKind::Secondary).on_click(move |_, _, cx| {
            if let Some(p) = poll {
                e2.update(cx, |a, cx| a.jump_to_message(ch, p, cx));
            }
        }));
    let mentioned = msg.author == app.me.id;
    let (bg, hover) = if mentioned {
        (Some(idx::MESSAGE_MENTIONED_BACKGROUND_DEFAULT), idx::MESSAGE_MENTIONED_BACKGROUND_HOVER)
    } else {
        (None, idx::MESSAGE_BACKGROUND_HOVER)
    };
    div()
        .id(("sysmsg-poll", msg.id.0))
        .relative()
        .mt(z(if r.group_start { 17. } else { 0. }))
        .py(z(2.))
        .flex()
        .flex_col()
        .when_some(bg, |d, b| d.bg(tk(b)))
        .hover(move |s| s.bg(tk(hover)))
        .when(mentioned, |d| d.child(div().absolute().left_0().top_0().bottom_0().w(z(2.)).bg(tk(idx::ICON_FEEDBACK_WARNING))))
        .child(line)
        .child(div().flex().pr(z(48.)).child(card))
        .into_any_element()
}

fn flow(text: SharedString) -> Vec<SharedString> {
    super::message_ext::words(&text, 0..text.len()).into_iter().map(|r| SharedString::from(text[r].to_owned())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(g: &Generic) -> String {
        g.segs
            .iter()
            .map(|s| match s {
                Seg::Text(t) => t.clone(),
                Seg::Author => "@U".into(),
                Seg::Strong(t) => format!("**{t}**"),
                Seg::Name(t) => format!("<{t}>"),
                Seg::Link(i, t) => format!("[{i}:{t}]"),
            })
            .collect::<String>()
            .replace('\u{a0}', " ")
    }

    fn sys_gen(code: &str, type_id: u8, content: &str) -> Generic {
        generic_in(code, &SysCtx { type_id, content, guild_name: Some("Aperture"), ..Default::default() }).unwrap()
    }

    #[test]
    fn template_parser_keeps_user_text_literal() {
        let v = vec![("c", "**{g}** [x] <y>".to_string()), ("g", "G".to_string())];
        assert_eq!(
            parse_template("{u} a: **{c}** [{g}] <{c}>", &v),
            vec![
                Seg::Author,
                Seg::Text(" a: ".into()),
                Seg::Strong("**{g}** [x] <y>".into()),
                Seg::Text(" ".into()),
                Seg::Link(0, "G".into()),
                Seg::Text(" ".into()),
                Seg::Name("**{g}** [x] <y>".into()),
            ]
        );
        assert_eq!(split_words("a b\u{a0}: c"), vec!["a ", "b\u{a0}: ", "c"]);
    }

    #[test]
    fn channel_and_group_renames_follow_the_client() {
        let g = sys_gen("fr", 4, "général");
        assert_eq!(g.icon, SysIcon::Named("chat/edit", 12., None));
        assert_eq!(plain(&g), "@U a changé le nom du salon : **général**");
        assert_eq!(plain(&sys_gen("en", 4, "general")), "@U changed the channel name: **general**");
        let post = generic_in("fr", &SysCtx { type_id: 4, content: "Idée", forum_post: true, ..Default::default() }).unwrap();
        assert_eq!(plain(&post), "@U a changé le titre du post : **Idée**");
        let group = generic_in("fr", &SysCtx { type_id: 4, content: "Les amis", group_dm: true, ..Default::default() }).unwrap();
        assert_eq!(plain(&group), "@U a changé le nom du groupe : <Les amis>. [0:Modifier le groupe]");
        assert_eq!(group.links, &[SysLink::EditGroup]);
        let cleared = generic_in("en", &SysCtx { type_id: 4, content: "", group_dm: true, ..Default::default() }).unwrap();
        assert_eq!(plain(&cleared), "@U removed the custom group name. [0:Edit Group]");
        let icon = sys_gen("fr", 5, "");
        assert_eq!(plain(&icon), "@U a changé l'icône du groupe. [0:Modifier le groupe]");
        assert_eq!(icon.icon, SysIcon::Named("chat/edit", 12., None));
    }

    #[test]
    fn boosts_follow_the_client() {
        use crate::theme::generated::idx;
        let b = sys_gen("fr", 8, "");
        assert_eq!(b.icon, SysIcon::Named("chat/boost", 16., Some(idx::GUILD_BOOSTING_PINK)));
        assert_eq!(plain(&b), "@U vient de booster le serveur !");
        assert_eq!(plain(&sys_gen("fr", 8, "3")), "@U vient de booster le serveur **3** fois !");
        assert_eq!(plain(&sys_gen("fr", 9, "")), "@U vient de booster le serveur ! Aperture a atteint **Niveau 1 !**");
        assert_eq!(plain(&sys_gen("fr", 10, "2")), "@U vient de booster le serveur **2** fois ! Aperture a atteint le niveau **Niveau 2 !**");
        assert_eq!(plain(&sys_gen("en", 11, "")), "@U just boosted the server! Aperture has achieved **Level 3!**");
        let no_guild = generic_in("en", &SysCtx { type_id: 11, content: "", ..Default::default() }).unwrap();
        assert_eq!(plain(&no_guild), "@U just boosted the server!");
        assert_eq!(boost_count("0"), 1);
        assert_eq!(boost_count("x"), 1);
        assert_eq!(boost_count(" 4 "), 4);
    }

    #[test]
    fn follow_and_discovery_follow_the_client() {
        use crate::theme::generated::idx;
        let f = sys_gen("fr", 12, "Phoenix #annonces");
        assert_eq!(f.icon, SysIcon::Asset(ASSET_JOIN));
        assert_eq!(plain(&f), "@U a ajouté [0:Phoenix #annonces] dans ce salon. Les mises à jour les plus importantes s'afficheront ici.");
        let out = sys_gen("en", 14, "");
        assert_eq!(out.icon, SysIcon::Asset(ASSET_DISCOVERY_OUT));
        assert_eq!(out.links, &[SysLink::Guild, SysLink::GuildSettings]);
        assert!(plain(&out).starts_with("[0:Aperture] has been removed from Server Discovery"));
        assert!(plain(&out).contains("Check [1:Server Settings] for more details."));
        let back = sys_gen("fr", 15, "");
        assert_eq!(back.icon, SysIcon::Asset(ASSET_DISCOVERY_BACK));
        assert!(!back.segs.contains(&Seg::Author));
        let w = generic_in("en", &SysCtx { type_id: 16, ..Default::default() }).unwrap();
        assert_eq!(w.icon, SysIcon::Named("settings/circle-error", 16., Some(idx::TEXT_FEEDBACK_WARNING)));
        assert!(plain(&w).starts_with("[0:this server] has failed Discovery activity requirements for 1 week."));
        assert!(plain(&sys_gen("fr", 17, "")).contains("pendant 3 semaines d'affilée"));
    }

    #[test]
    fn stage_messages_follow_the_client() {
        use crate::theme::generated::idx;
        let s = sys_gen("fr", 27, "Réunion");
        assert_eq!(s.icon, SysIcon::Named("stage", 24., Some(idx::GREEN_360)));
        assert!(s.strong_text);
        assert_eq!(plain(&s), "@U a commencé **Réunion**");
        assert_eq!(plain(&sys_gen("fr", 28, "Réunion")), "@U a mis fin à **Réunion**");
        assert_eq!(plain(&sys_gen("en", 29, "")), "@U is now a speaker.");
        assert_eq!(sys_gen("en", 30, "").icon, SysIcon::Named("sys/hand-request-speak", 24., None));
        assert_eq!(plain(&sys_gen("fr", 30, "")), "@U a demandé à prendre la parole.");
        assert_eq!(plain(&sys_gen("fr", 31, "Sujet")), "@U a changé le sujet de la conférence : **Sujet**");
    }

    #[test]
    fn role_subscription_app_and_purchase_messages_follow_the_client() {
        let d = crate::model::SysData { app_name: Some("Bot".into()), tier_name: Some("Or".into()), months: 3, product: Some("Pack".into()), ..Default::default() };
        let one = crate::model::SysData { months: 1, ..d.clone() };
        let renew = crate::model::SysData { months: 0, renewal: true, ..d.clone() };
        let bare = crate::model::SysData::default();
        let g = |code: &str, type_id: u8, data: &crate::model::SysData| generic_in(code, &SysCtx { type_id, guild_name: Some("Aperture"), data: Some(data), ..Default::default() });
        let sub = g("fr", 25, &d).unwrap();
        assert_eq!(sub.icon, SysIcon::Asset(ASSET_JOIN));
        assert_eq!(plain(&sub), "@U a un abonnement **Or** à [0:Aperture] depuis 3 mois !");
        assert_eq!(plain(&g("en", 25, &one).unwrap()), "@U joined **Or** and has been a subscriber of [0:Aperture] for 1 month!");
        assert_eq!(plain(&g("en", 25, &renew).unwrap()), "@U renewed **Or** in their [0:Aperture] membership!");
        assert!(generic_in("fr", &SysCtx { type_id: 25, ..Default::default() }).is_none());
        let premium = g("fr", 32, &d).unwrap();
        assert_eq!(premium.icon, SysIcon::Named("sys/ticket", 24., None));
        assert_eq!(plain(&premium), "@U a fait passer Bot à la version premium sur ce serveur ! 🎉");
        assert_eq!(plain(&g("en", 32, &bare).unwrap()), "@U upgraded a deleted application to premium for this server! 🎉");
        let added = g("en", 33, &d).unwrap();
        assert_eq!(plain(&added), "@U added the [0:Bot] app. See our [1:help center] for more info.");
        assert_eq!(added.links, &[SysLink::Inert, SysLink::HelpCenter]);
        let removed = g("fr", 34, &bare).unwrap();
        assert_eq!(removed.icon, SysIcon::Asset(ASSET_LEAVE));
        assert_eq!(plain(&removed), "@U a enlevé une application supprimée. Visite notre [0:centre d'aide] pour plus d'informations.");
        assert_eq!(removed.links, &[SysLink::HelpCenter]);
        assert_eq!(plain(&g("fr", 44, &d).unwrap()), "@U a acheté [0:Pack] !");
        assert!(g("fr", 44, &bare).is_none());
    }

    #[test]
    fn join_requests_and_reports_follow_the_client() {
        let ok = sys_gen("fr", 52, "");
        assert_eq!(ok.icon, SysIcon::Asset(ASSET_JOIN));
        assert_eq!(plain(&ok), "Cette candidature a été approuvée ! Bienvenue !");
        assert_eq!(sys_gen("en", 53, "").icon, SysIcon::Asset(ASSET_LEAVE));
        assert_eq!(plain(&sys_gen("en", 53, "")), "This application was rejected.");
        assert_eq!(plain(&sys_gen("fr", 54, "")), "Cette candidature a été retirée.");
        let r = |t: u8| generic_in("en", &SysCtx { type_id: t, other: Some("Bob".into()), ..Default::default() }).unwrap();
        assert_eq!(plain(&r(61)), "@U banned <Bob>");
        assert_eq!(r(61).icon, SysIcon::Named("mod/hammer", 20., None));
        assert_eq!(plain(&r(60)), "@U kicked <Bob>");
        assert_eq!(r(60).icon, SysIcon::Named("mod/user-minus", 20., None));
        assert_eq!(plain(&r(59)), "@U timed out <Bob>");
        assert_eq!(r(59).icon, SysIcon::Named("mod/clock-warning", 20., None));
        assert_eq!(plain(&r(58)), "@U deleted the message");
        assert_eq!(r(58).icon, SysIcon::Named("chat/trash", 20., None));
        assert_eq!(plain(&r(62)), "@U resolved this flag");
        assert_eq!(r(62).icon, SysIcon::Named("check-small", 20., None));
    }

    #[test]
    fn streams_follow_the_client() {
        assert!(generic_in("en", &SysCtx { type_id: 13, ..Default::default() }).is_none());
        let live = StreamRef { channel: "Salon".into(), activity: None, ended_ms: None };
        let g = generic_in("fr", &SysCtx { type_id: 13, stream: Some(live.clone()), ..Default::default() }).unwrap();
        assert_eq!(g.icon, SysIcon::Live);
        assert_eq!(plain(&g), "@U is live! Now streaming unknown. [0:Join Stream]");
        assert_eq!(g.links, &[SysLink::JoinStream]);
        let game = StreamRef { activity: Some("Portal 2".into()), ..live.clone() };
        assert_eq!(plain(&generic_in("en", &SysCtx { type_id: 13, stream: Some(game), ..Default::default() }).unwrap()), "@U is live! Now streaming Portal 2. [0:Join Stream]");
        let ended = StreamRef { ended_ms: Some(23 * 60_000), ..live };
        let e = generic_in("en", &SysCtx { type_id: 13, stream: Some(ended), ..Default::default() }).unwrap();
        assert_eq!(e.icon, SysIcon::Named("voice/screen-x", 20., None));
        assert_eq!(plain(&e), "@U was live for 23 minutes in Salon");
    }

    #[test]
    fn safety_alerts_and_friend_requests_follow_the_client() {
        let on = generic_in("en", &SysCtx { type_id: 36, time: "11/25/2026, 9:30 PM".into(), ..Default::default() }).unwrap();
        assert_eq!(plain(&on), "@U enabled security actions until 11/25/2026, 9:30 PM.");
        assert_eq!(plain(&sys_gen("fr", 37, "")), "@U a désactivé les actions de cybersécurité.");
        assert_eq!(plain(&sys_gen("en", 38, "")), "@U reported a raid in Aperture.");
        assert_eq!(plain(&sys_gen("fr", 39, "")), "@U a résolu une alerte d'activité.");
        for t in AUTOMOD_TYPES {
            assert!(!FALLBACK_TYPES.contains(&t));
        }
        let t = chrono::NaiveDate::from_ymd_opt(2026, 11, 25).unwrap().and_hms_opt(21, 30, 0).unwrap();
        assert_eq!(format_alert("fr", t), "25/11/2026 21:30");
        assert_eq!(format_alert("en-US", t), "11/25/2026, 9:30 PM");
        assert_eq!(alert_time("pas une date"), "");
        let f = |code: &str, mine: bool, content: &str| generic_in(code, &SysCtx { type_id: 67, mine, content, other: Some("Bob".into()), ..Default::default() }).unwrap();
        assert_eq!(plain(&f("fr", true, "salut")), "Tu as accepté la demande d'ami envoyée par <Bob>. Il/Elle a dit : <salut>");
        assert_eq!(plain(&f("en", false, "")), "<Bob> accepted your friend request.");
        assert_eq!(plain(&f("en", true, "")), "You accepted <Bob>'s friend request.");
        assert_eq!(plain(&f("fr", false, "yo")), "<Bob> a accepté ta demande d'ami. Tu as dit : <yo>");
        assert_eq!(f("fr", false, "").icon, SysIcon::Named("user-check", 18., Some(crate::theme::generated::idx::STATUS_POSITIVE)));
        assert!(generic_in("fr", &SysCtx { type_id: 67, ..Default::default() }).is_none());
    }

    #[test]
    fn hidden_and_user_types() {
        for t in [0u8, 19, 20, 22, 40, 42, 55, 56, 57, 63] {
            assert!(generic_in("fr", &SysCtx { type_id: t, ..Default::default() }).is_none(), "{t}");
            assert!(!FALLBACK_TYPES.contains(&t));
        }
        let m = Message { kind: MessageKind::Boost(2), ..Default::default() };
        assert_eq!(effective_type(&m), 10);
        let m = Message { kind: MessageKind::OtherSystem, type_id: 30, ..Default::default() };
        assert_eq!(effective_type(&m), 30);
    }

    #[test]
    fn call_durations_follow_moment_humanize() {
        let s = 1000;
        let m = 60 * s;
        let h = 60 * m;
        let d = 24 * h;
        assert_eq!(humanize(0), Humanized::Seconds);
        assert_eq!(humanize(44 * s + 499), Humanized::Seconds);
        assert_eq!(humanize(44 * s + 500), Humanized::Minute);
        assert_eq!(humanize(89 * s), Humanized::Minute);
        assert_eq!(humanize(90 * s), Humanized::Minutes(2));
        assert_eq!(humanize(44 * m + 29 * s), Humanized::Minutes(44));
        assert_eq!(humanize(44 * m + 30 * s), Humanized::Hour);
        assert_eq!(humanize(89 * m), Humanized::Hour);
        assert_eq!(humanize(90 * m), Humanized::Hours(2));
        assert_eq!(humanize(21 * h + 29 * m), Humanized::Hours(21));
        assert_eq!(humanize(21 * h + 30 * m), Humanized::Day);
        assert_eq!(humanize(36 * h), Humanized::Days(2));
        assert_eq!(humanize(25 * d + 11 * h), Humanized::Days(25));
        assert_eq!(humanize(26 * d), Humanized::Month);
        assert_eq!(humanize(46 * d), Humanized::Months(2));
        assert_eq!(humanize(320 * d), Humanized::Year);
        assert_eq!(humanize(600 * d), Humanized::Years(2));
        assert_eq!(humanize(-5 * s), Humanized::Seconds);
    }

    #[test]
    fn call_states_follow_the_client() {
        use crate::model::{CallInfo, Id};
        let me = Id(1);
        let ended = CallInfo { participants: vec![Id(2)], duration_ms: Some(5000) };
        assert_eq!(call_state(Some(&ended), false, me, false), CallState::Missed);
        let joined = CallInfo { participants: vec![Id(2), me], duration_ms: Some(5000) };
        assert_eq!(call_state(Some(&joined), false, me, false), CallState::Started);
        assert_eq!(call_state(None, false, me, false), CallState::Started);
        let ongoing = CallInfo { participants: vec![Id(2)], duration_ms: None };
        assert_eq!(call_state(Some(&ongoing), true, me, false), CallState::Joinable);
        assert_eq!(call_state(Some(&ongoing), true, me, true), CallState::Started);
    }

    #[test]
    fn poll_result_title_is_cut_like_the_client() {
        let q = "[Test Phoenix] envoyé depuis le client Rust (Phoenix) — sondage de test : quelle réponse ?";
        assert_eq!(super::truncate_question(q), "[Test Phoenix] envoyé depuis le client Rust (Phoe\u{2026}");
        assert_eq!(super::truncate_question("court"), "court");
    }

    #[test]
    fn join_text_is_stable() {
        assert_eq!(join_text(1 << 22), join_text(1 << 22));
        assert!(is_system(MessageKind::Join));
        assert!(!is_system(MessageKind::Reply));
        assert_eq!(wave_sticker(0, 0).0, 749054660769218631);
        assert_eq!(wave_sticker(3 << 22, 0), (781291131828699156, true));
    }
}

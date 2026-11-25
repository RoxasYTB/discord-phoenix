use std::rc::Rc;

use gpui::{AnyElement, ClipboardItem, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::rest::guild_manage::GuildEdit;
use serde_json::Value;

use super::{
    State,
    kit::{self, Cx, divider, field, fieldset, labels, page_head, switch_field, text},
    with_state,
};
use crate::{
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
};

pub const SUPPRESS_JOIN: u64 = 1 << 0;
pub const SUPPRESS_PREMIUM: u64 = 1 << 1;
pub const SUPPRESS_GUILD_REMINDER: u64 = 1 << 2;
pub const SUPPRESS_JOIN_REPLIES: u64 = 1 << 3;

pub const AFK_TIMEOUTS: [u32; 5] = [60, 300, 900, 1800, 3600];

pub fn flags(s: &State) -> u64 {
    s.edits.flags.unwrap_or_else(|| kit::n(s.guild_json(), "system_channel_flags"))
}

pub fn guild_edit(s: &State, cx: &gpui::App) -> Option<GuildEdit> {
    let g = s.guild_json();
    let e = &s.edits;
    let name = s.inputs.get("name").map(|i| i.read(cx).value().to_string()).filter(|n| !n.trim().is_empty() && n.trim() != kit::s(g, "name"));
    let description = s.areas.get("description").map(|a| a.read(cx).value().to_string()).filter(|d| *d != kit::s(g, "description"));
    let edit = GuildEdit {
        name,
        description,
        system_channel: e.system_channel.map(|c| c.map(phoenix_api::models::Snowflake)),
        afk_channel: e.afk_channel.map(|c| c.map(phoenix_api::models::Snowflake)),
        afk_timeout: e.afk_timeout,
        default_notifications: e.notify,
        system_channel_flags: e.flags,
        ..Default::default()
    };
    (edit != GuildEdit::default()).then_some(edit)
}

fn afk_label(secs: u32) -> SharedString {
    match secs {
        3600 => crate::t!("gs.eng.afk_1h"),
        s => crate::t!("gs.eng.afk_min", n = s / 60),
    }
}

fn channel_value(app: &PhoenixApp, ch: Option<u64>, none: SharedString) -> AnyElement {
    let Some(ch) = ch.and_then(|c| app.channels.get(&Id(c))) else {
        return text(none, 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT).mx(z(4.)).into_any_element();
    };
    let cat = ch.parent.and_then(|p| app.channels.get(&p)).map(|p| p.name.clone());
    div()
        .flex()
        .items_center()
        .min_w_0()
        .child(
            gpui::svg()
                .path(crate::icons::path(if ch.kind == ChannelKind::Voice { "speaker" } else { "hash" }))
                .flex_none()
                .size(z(16.))
                .text_color(c(tok(idx::ICON_MUTED))),
        )
        .child(text(ch.name.clone(), 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT).mx(z(4.)).flex_shrink(1.).min_w_0().truncate())
        .children(cat.map(|n| text(n, 14., 18., FontWeight::MEDIUM, idx::TEXT_MUTED).ml(z(4.)).flex_shrink(1.).min_w_0().truncate()))
        .into_any_element()
}

fn channels(app: &PhoenixApp, guild: Id, kind: ChannelKind) -> Vec<Id> {
    let mut v: Vec<&crate::model::Channel> =
        app.guild_channels.get(&guild).into_iter().flatten().filter_map(|id| app.channels.get(id)).filter(|ch| ch.kind == kind).collect();
    v.sort_by_key(|ch| (ch.parent.and_then(|p| app.channels.get(&p)).map(|p| p.position).unwrap_or(-1), ch.position));
    v.into_iter().map(|ch| ch.id).collect()
}

#[allow(clippy::too_many_arguments)]
fn channel_dropdown(
    app: &PhoenixApp,
    s: &State,
    id: &'static str,
    kind: ChannelKind,
    cur: Option<u64>,
    none: SharedString,
    set: fn(&mut State, Option<u64>),
    cx: &mut Cx,
) -> AnyElement {
    let list = channels(app, s.guild, kind);
    let mut opts: Vec<SharedString> = vec![none.clone()];
    opts.extend(list.iter().map(|c| app.channels.get(c).map(|ch| ch.name.clone()).unwrap_or_default()));
    let at = cur.and_then(|c| list.iter().position(|x| x.0 == c)).map_or(0, |p| p + 1);
    let ids: Vec<Option<u64>> = std::iter::once(None).chain(list.iter().map(|c| Some(c.0))).collect();
    kit::dropdown(
        s,
        id,
        channel_value(app, cur, none),
        opts,
        Some(at),
        Rc::new(move |_: &mut PhoenixApp, i: usize, _: &mut Cx| {
            let v = ids.get(i).copied().flatten();
            with_state(|s| set(s, v));
        }),
        cx,
    )
}

fn copy_input(id: &'static str, value: String, cx: &mut Cx) -> AnyElement {
    let v = value.clone();
    div()
        .h(z(40.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .flex()
        .items_center()
        .child(div().flex_1().min_w_0().pl(z(10.)).mr(z(10.)).truncate().text_size(z(16.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(value))
        .child(
            div()
                .id(id)
                .flex_none()
                .h(z(30.))
                .px(z(20.))
                .m(z(4.))
                .ml(z(0.))
                .rounded(z(4.))
                .border_1()
                .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
                .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
                .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
                .flex()
                .items_center()
                .cursor_pointer()
                .child(text(crate::t!("gs.eng.copy"), 14., 16., FontWeight::MEDIUM, idx::CONTROL_SECONDARY_TEXT_DEFAULT))
                .on_click(cx.listener(move |a, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(v.clone()));
                    a.parity_toast(crate::t!("gs.eng.copied"), cx);
                })),
        )
        .into_any_element()
}

fn radio(id: (&'static str, usize), label: SharedString, on: bool, cx: &mut Cx, pick: impl Fn(&mut State) + 'static) -> AnyElement {
    div()
        .id(id)
        .relative()
        .h(z(20.))
        .pl(z(32.))
        .flex()
        .items_center()
        .cursor_pointer()
        .child(div().absolute().left_0().top_0().child(crate::ui::controls::radio_indicator(on)))
        .child(text(label, 16., 20., FontWeight::NORMAL, idx::TEXT_DEFAULT))
        .on_click(cx.listener(move |_, _, _, cx| {
            with_state(&pick);
            cx.notify();
        }))
        .into_any_element()
}

fn half(a: impl IntoElement, b: impl IntoElement) -> gpui::Div {
    div().flex().gap(z(16.)).child(div().flex_1().min_w_0().child(a)).child(div().flex_1().min_w_0().child(b))
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Cx) -> AnyElement {
    let g = s.guild_json();
    let f = flags(s);
    let flip = |bit: u64| {
        move |s: &mut State| {
            let cur = flags(s);
            s.edits.flags = Some(cur ^ bit).filter(|v| *v != kit::n(s.guild_json(), "system_channel_flags"));
        }
    };
    let sys = s.edits.system_channel.unwrap_or_else(|| kit::id_of(g, "system_channel_id"));
    let sys_row = div().flex().gap(z(24.)).child(labels(crate::t!("gs.eng.sys_channel"), Some(crate::t!("gs.eng.sys_channel_desc"))).flex_1()).child(
        div().flex_1().min_w_0().child(channel_dropdown(
            app,
            s,
            "sys",
            ChannelKind::Text,
            sys,
            crate::t!("gs.eng.no_sys"),
            |s, v| s.edits.system_channel = Some(v),
            cx,
        )),
    );
    let system = fieldset(
        crate::t!("gs.eng.system"),
        Some(crate::t!("gs.eng.system_desc")),
        vec![
            switch_field("ss-sys-join", crate::t!("gs.eng.join"), None, f & SUPPRESS_JOIN == 0, flip(SUPPRESS_JOIN), cx),
            switch_field("ss-sys-replies", crate::t!("gs.eng.join_replies"), None, f & SUPPRESS_JOIN_REPLIES == 0, flip(SUPPRESS_JOIN_REPLIES), cx),
            switch_field("ss-sys-boost", crate::t!("gs.eng.boost"), None, f & SUPPRESS_PREMIUM == 0, flip(SUPPRESS_PREMIUM), cx),
            switch_field("ss-sys-tips", crate::t!("gs.eng.tips"), None, f & SUPPRESS_GUILD_REMINDER == 0, flip(SUPPRESS_GUILD_REMINDER), cx),
            sys_row.into_any_element(),
        ],
    );
    let feed_on = !g.get("features").and_then(Value::as_array).is_some_and(|f| f.iter().any(|x| x == "ACTIVITY_FEED_DISABLED_BY_USER"));
    let feed = fieldset(
        crate::t!("gs.eng.feed"),
        Some(crate::t!("gs.eng.feed_desc")),
        vec![
            div()
                .flex()
                .gap(z(24.))
                .child(labels(crate::t!("gs.eng.feed_show"), None).flex_1().pt(z(2.)))
                .child(crate::ui::controls::switch("ss-feed", feed_on))
                .into_any_element(),
        ],
    );
    let notify = s.edits.notify.unwrap_or(kit::n(g, "default_message_notifications") as u8);
    let set_notify = |v: u8| move |s: &mut State| s.edits.notify = Some(v).filter(|v| u64::from(*v) != kit::n(s.guild_json(), "default_message_notifications"));
    let notifications = field(
        crate::t!("gs.eng.notify"),
        Some(crate::t!("gs.eng.notify_desc")),
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .pt(z(8.))
                    .pb(z(4.))
                    .flex()
                    .flex_col()
                    .gap(z(16.))
                    .child(radio(("ss-notify", 0), crate::t!("gs.eng.notify_all"), notify == 0, cx, set_notify(0)))
                    .child(radio(("ss-notify", 1), crate::t!("gs.eng.notify_mentions"), notify == 1, cx, set_notify(1))),
            )
            .child(text(crate::t!("gs.eng.notify_hint"), 12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE)),
    );
    let afk = s.edits.afk_channel.unwrap_or_else(|| kit::id_of(g, "afk_channel_id"));
    let timeout = s.edits.afk_timeout.unwrap_or(kit::n(g, "afk_timeout") as u32);
    let t_at = AFK_TIMEOUTS.iter().position(|t| *t == timeout);
    let inactive = div()
        .flex()
        .flex_col()
        .child(
            half(
                field(
                    crate::t!("gs.eng.afk_channel"),
                    None,
                    channel_dropdown(app, s, "afk", ChannelKind::Voice, afk, crate::t!("gs.eng.no_afk"), |s, v| s.edits.afk_channel = Some(v), cx),
                ),
                field(
                    crate::t!("gs.eng.afk_timeout"),
                    None,
                    kit::dropdown(
                        s,
                        "afk_timeout",
                        text(afk_label(timeout), 16., 20., FontWeight::NORMAL, idx::TEXT_STRONG).into_any_element(),
                        AFK_TIMEOUTS.iter().map(|t| afk_label(*t)).collect(),
                        t_at,
                        Rc::new(|_: &mut PhoenixApp, i: usize, _: &mut Cx| {
                            with_state(|s| s.edits.afk_timeout = Some(AFK_TIMEOUTS[i]).filter(|t| u64::from(*t) != kit::n(s.guild_json(), "afk_timeout")));
                        }),
                        cx,
                    ),
                ),
            )
            .mb(z(8.)),
        )
        .child(text(crate::t!("gs.eng.afk_desc"), 14., 18., FontWeight::MEDIUM, idx::TEXT_SUBTLE));
    let w = s.get("widget").cloned().unwrap_or(Value::Null);
    let w_on = s.edits.widget_enabled.unwrap_or_else(|| w.get("enabled").and_then(Value::as_bool).unwrap_or(false));
    let w_ch = s.edits.widget_channel.unwrap_or_else(|| kit::id_of(&w, "channel_id"));
    let gid = s.guild.0.to_string();
    let widget = fieldset(
        crate::t!("gs.eng.widget"),
        Some(crate::t!("gs.eng.widget_desc")),
        vec![
            switch_field("ss-widget", crate::t!("gs.eng.widget_on"), None, w_on, |s| {
                let base = s.get("widget").and_then(|w| w.get("enabled")).and_then(Value::as_bool).unwrap_or(false);
                let cur = s.edits.widget_enabled.unwrap_or(base);
                s.edits.widget_enabled = Some(!cur).filter(|v| *v != base);
            }, cx),
            div()
                .flex()
                .flex_col()
                .child(field(
                    crate::t!("gs.eng.widget_channel"),
                    Some(crate::t!("gs.eng.widget_channel_desc")),
                    channel_dropdown(app, s, "widget", ChannelKind::Text, w_ch, crate::t!("gs.eng.widget_none"), |s, v| s.edits.widget_channel = Some(v), cx),
                ))
                .child(
                    div()
                        .mt(z(24.))
                        .flex()
                        .gap(z(20.))
                        .child(div().flex_1().min_w_0().child(field(crate::t!("gs.eng.server_id"), None, copy_input("ss-copy-id", gid.clone(), cx))))
                        .child(div().flex_1().min_w_0().child(field(crate::t!("gs.eng.json_api"), None, copy_input("ss-copy-json", format!("https://discord.com/api/guilds/{gid}/widget.json"), cx)))),
                )
                .child(div().my(z(24.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))))
                .child(
                    div().flex().gap(z(20.)).child(
                        div().flex_1().min_w_0().child(field(
                            crate::t!("gs.eng.premade"),
                            Some(crate::t!("gs.eng.premade_desc")),
                            copy_input(
                                "ss-copy-html",
                                format!(r#"<iframe src="https://discord.com/widget?id={gid}&theme=dark" width="350" height="500" allowtransparency="true" frameborder="0" sandbox="allow-popups allow-popups-to-escape-sandbox allow-same-origin allow-scripts"></iframe>"#),
                                cx,
                            ),
                        )),
                    )
                    .child(div().flex_1()),
                )
                .into_any_element(),
        ],
    );
    div()
        .flex()
        .flex_col()
        .gap(z(40.))
        .child(page_head(crate::t!("menu.gs.engagement"), Some(crate::t!("gs.eng.desc"))))
        .child(system)
        .child(divider())
        .child(feed)
        .child(divider())
        .child(notifications)
        .child(divider())
        .child(inactive)
        .child(divider())
        .child(widget)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn state() -> State {
        let mut s = State::new(Id(1), super::super::section_index("menu.gs.engagement"));
        s.data.insert(
            "guild",
            json!({ "name": "G", "system_channel_flags": 13, "default_message_notifications": 1, "afk_timeout": 3600, "system_channel_id": "5" }),
        );
        s
    }

    #[test]
    fn system_flags_toggle_back_to_clean() {
        let mut s = state();
        assert_eq!(flags(&s) & SUPPRESS_PREMIUM, 0);
        s.edits.flags = Some(13 ^ SUPPRESS_PREMIUM);
        let cx_free = GuildEdit { system_channel_flags: Some(15), ..Default::default() };
        assert_eq!(cx_free.system_channel_flags, Some(15));
        s.edits.flags = Some(13 ^ SUPPRESS_PREMIUM ^ SUPPRESS_PREMIUM).filter(|v| *v != 13);
        assert_eq!(s.edits.flags, None);
    }

    #[test]
    fn afk_labels() {
        assert_eq!(AFK_TIMEOUTS.len(), 5);
        assert_eq!(AFK_TIMEOUTS[4], 3600);
    }
}

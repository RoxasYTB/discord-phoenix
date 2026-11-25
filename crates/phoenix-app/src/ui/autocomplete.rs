use gpui::{
    AnyElement, App, Context, Focusable, FontWeight, ParentElement, SharedString, Styled, Window,
    div, prelude::*,
};

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

use crate::{
    app::PhoenixApp,
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui::{
        icon,
        pickers::{
            Pickers,
            emoji::{self, name_score},
            grid::{PickItem, item_element},
            types::{GuildEmoji, StickerData},
        },
        sidebar::channel_icon,
    },
};

const MAX: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Emoji,
    Mention,
    Channel,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Emoji(PickItem),
    User(Id),
    Role { id: Id, name: SharedString, color: Option<u32> },
    Everyone(&'static str),
    Channel(Id),
    Sticker { data: StickerData, source: Option<SharedString>, similar: Option<SharedString> },
}

#[derive(Default)]
pub struct Autocomplete {
    pub active: Option<Active>,
}

pub struct Active {
    pub kind: Kind,
    pub query: String,
    pub start: usize,
    pub end: usize,
    pub items: Vec<Item>,
    pub selected: usize,
}

impl gpui::Global for Autocomplete {}

pub fn token_at(text: &str, cursor: usize) -> Option<(Kind, String, usize)> {
    let cursor = cursor.min(text.len());
    if !text.is_char_boundary(cursor) {
        return None;
    }
    let before = &text[..cursor];
    let start = before.rfind(char::is_whitespace).map(|i| i + before[i..].chars().next().map_or(1, char::len_utf8)).unwrap_or(0);
    let tok = &before[start..];
    let mut chars = tok.chars();
    let kind = match chars.next()? {
        ':' => Kind::Emoji,
        '@' => Kind::Mention,
        '#' => Kind::Channel,
        _ => return None,
    };
    let q: String = chars.collect();
    match kind {
        Kind::Emoji if q.chars().count() < 2 || q.contains(':') => None,
        Kind::Mention | Kind::Channel if q.contains(['<', '>']) => None,
        _ => Some((kind, q, start)),
    }
}

pub fn dismiss(cx: &mut App) {
    if let Some(a) = cx.try_global::<Autocomplete>()
        && a.active.is_some()
    {
        cx.global_mut::<Autocomplete>().active = None;
    }
}

fn custom_emojis(app: &PhoenixApp) -> Vec<GuildEmoji> {
    let mut guilds: Vec<Id> = app.guilds.iter().map(|g| g.id).collect();
    if let Some(sel) = app.selected_guild {
        guilds.sort_by_key(|g| *g != sel);
    }
    guilds.into_iter().flat_map(|g| app.backend.guild_emojis(g)).collect()
}

pub fn suggestions(app: &PhoenixApp, kind: Kind, q: &str) -> Vec<Item> {
    let q = q.to_lowercase();
    match kind {
        Kind::Emoji => {
            let mut custom: Vec<(u8, GuildEmoji)> = custom_emojis(app)
                .into_iter()
                .filter_map(|e| name_score(&e.name.to_lowercase(), &q).map(|s| (s, e)))
                .collect();
            custom.sort_by_key(|(s, _)| *s);
            let mut out: Vec<Item> = custom.into_iter().take(MAX).map(|(_, e)| Item::Emoji(PickItem::Custom(e))).collect();
            out.extend(emoji::search(&q, MAX).into_iter().map(|i| Item::Emoji(PickItem::Unicode(i))));
            out.truncate(MAX);
            out
        }
        Kind::Mention => {
            let mut scored: Vec<(u8, SharedString, Item)> = Vec::new();
            let mut seen = std::collections::HashSet::new();
            let mut consider = |id: Id, scored: &mut Vec<(u8, SharedString, Item)>| {
                if !seen.insert(id) {
                    return;
                }
                let Some(u) = app.users.get(&id) else { return };
                let s = [u.display_name.to_lowercase(), u.name.to_lowercase()]
                    .iter()
                    .filter_map(|n| if q.is_empty() { Some(4) } else { name_score(n, &q) })
                    .min();
                if let Some(s) = s {
                    scored.push((s, u.display_name.clone(), Item::User(id)));
                }
            };
            if let Some(g) = app.selected_guild {
                for m in app.members.get(&g).into_iter().flatten() {
                    consider(m.user, &mut scored);
                }
                if let Some(msgs) = app.selected_channel.and_then(|c| app.messages.get(&c)) {
                    for m in msgs.msgs.iter().rev().take(200) {
                        consider(m.author, &mut scored);
                    }
                }
                for r in app.roles.get(&g).into_iter().flatten() {
                    if r.name == "@everyone" {
                        continue;
                    }
                    if let Some(s) = if q.is_empty() { Some(5) } else { name_score(&r.name.to_lowercase(), &q) } {
                        scored.push((s, r.name.clone(), Item::Role { id: r.id, name: r.name.clone(), color: r.color }));
                    }
                }
                for e in ["everyone", "here"] {
                    if e.starts_with(&q) {
                        scored.push((if q.is_empty() { 6 } else { 1 }, e.into(), Item::Everyone(e)));
                    }
                }
            } else if let Some(msgs) = app.selected_channel.and_then(|c| app.messages.get(&c)) {
                for m in msgs.msgs.iter().rev().take(200) {
                    consider(m.author, &mut scored);
                }
                consider(app.me.id, &mut scored);
            }
            scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            scored.into_iter().take(MAX).map(|(_, _, i)| i).collect()
        }
        Kind::Channel => {
            let Some(g) = app.selected_guild else { return Vec::new() };
            let mut scored: Vec<(u8, i32, Item)> = app
                .guild_channels
                .get(&g)
                .into_iter()
                .flatten()
                .filter_map(|id| app.channels.get(id))
                .filter(|ch| ch.kind != ChannelKind::Category)
                .filter_map(|ch| {
                    let s = if q.is_empty() { Some(4) } else { name_score(&ch.name.to_lowercase(), &q) }?;
                    Some((s, ch.position, Item::Channel(ch.id)))
                })
                .collect();
            scored.sort_by_key(|(s, p, _)| (*s, *p));
            scored.into_iter().take(MAX).map(|(_, _, i)| i).collect()
        }
    }
}

fn sticker_matches(app: &PhoenixApp, q: &str, cx: &App) -> Vec<Item> {
    let q = q.to_lowercase();
    let premium = app.backend.has_premium();
    let mut pool: Vec<(StickerData, Option<SharedString>)> = Vec::new();
    for g in &app.guilds {
        if premium || Some(g.id) == app.selected_guild {
            pool.extend(app.backend.guild_stickers(g.id).into_iter().map(|s| (s, Some(g.name.clone()))));
        }
    }
    if let Some(packs) = cx.try_global::<Pickers>().and_then(|p| p.stickers.packs.as_ref()) {
        for pack in packs {
            pool.extend(pack.stickers.iter().cloned().map(|s| (s, Some(pack.name.clone()))));
        }
    }
    let mut by_name = Vec::new();
    let mut by_tag = Vec::new();
    for (data, source) in pool {
        if data.name.to_lowercase().contains(&q) {
            by_name.push(Item::Sticker { data, source, similar: None });
        } else if let Some(tag) = data.tags.to_lowercase().split([',', ' ']).find(|t| !t.is_empty() && t.starts_with(&q)).map(|t| SharedString::from(t.to_owned())) {
            by_tag.push(Item::Sticker { data, source, similar: Some(tag) });
        }
    }
    by_name.extend(by_tag);
    by_name
}

fn with_stickers(emojis: Vec<Item>, stickers: Vec<Item>) -> Vec<Item> {
    let n_st = stickers.len().min(4.max(8usize.saturating_sub(emojis.len())));
    let n_em = emojis.len().min(6.max(10usize.saturating_sub(n_st)));
    emojis.into_iter().take(n_em).chain(stickers.into_iter().take(n_st)).collect()
}

pub fn refresh(app: &mut PhoenixApp, _window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if !cx.has_global::<Autocomplete>() {
        cx.set_global(Autocomplete::default());
    }
    let (text, cursor) = {
        let s = app.composer.read(cx);
        (s.value().to_string(), s.cursor())
    };
    let next = token_at(&text, cursor).and_then(|(kind, query, start)| {
        let mut items = suggestions(app, kind, &query);
        if kind == Kind::Emoji && app.prefs.synced.flag("ac-stickers") {
            items = with_stickers(items, sticker_matches(app, &query, cx));
        }
        (!items.is_empty()).then_some(Active { kind, query, start, end: cursor, items, selected: 0 })
    });
    if next.as_ref().is_some_and(|a| a.kind == Kind::Emoji) && cx.has_global::<Pickers>() {
        crate::ui::pickers::sticker::load_packs(app, cx);
    }
    let had = cx.global::<Autocomplete>().active.is_some();
    if had || next.is_some() {
        cx.global_mut::<Autocomplete>().active = next;
        cx.notify();
    }
}

pub fn insertion(app: &PhoenixApp, item: &Item, tone: u8) -> String {
    match item {
        Item::Emoji(PickItem::Unicode(ix)) => emoji::toned(*ix, tone),
        Item::Emoji(PickItem::Custom(e)) => e.token(),
        Item::User(id) => format!("<@{}>", id.0),
        Item::Role { id, .. } => format!("<@&{}>", id.0),
        Item::Everyone(e) => format!("@{e}"),
        Item::Channel(id) => {
            let _ = app;
            format!("<#{}>", id.0)
        }
        Item::Sticker { .. } => String::new(),
    }
}

pub fn accept(app: &mut PhoenixApp, index: usize, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(a) = cx.try_global::<Autocomplete>().and_then(|a| a.active.as_ref()) else { return };
    let Some(item) = a.items.get(index).cloned() else { return };
    let range = a.start..a.end;
    let tone = cx.try_global::<Pickers>().map(|p| p.prefs.tone).unwrap_or(0);
    if let (Item::Emoji(pi), true) = (&item, cx.has_global::<Pickers>()) {
        cx.global_mut::<Pickers>().prefs.record(pi);
    }
    if let (Item::Sticker { data, .. }, Some(ch)) = (&item, app.selected_channel) {
        crate::ui::pickers::detach(app.backend.send_sticker(ch, data.id), cx);
    }
    let text = match &item {
        Item::Sticker { .. } => String::new(),
        _ => format!("{} ", crate::ui::chat::rich_input::insertion(app, insertion(app, &item, tone))),
    };
    cx.global_mut::<Autocomplete>().active = None;
    app.composer.update(cx, |s, cx| {
        s.set_selected_range(range, cx);
        s.replace(text, window, cx);
    });
    cx.notify();
}

pub fn on_key(app: &mut PhoenixApp, key: &str, _shift: bool, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    let Some(a) = cx.try_global::<Autocomplete>().and_then(|a| a.active.as_ref()) else { return false };
    if !app.composer.focus_handle(cx).is_focused(window) {
        return false;
    }
    let (n, sel) = (a.items.len(), a.selected);
    match key {
        "up" | "down" => {
            let next = if key == "up" { (sel + n - 1) % n } else { (sel + 1) % n };
            if let Some(a) = cx.global_mut::<Autocomplete>().active.as_mut() {
                a.selected = next;
            }
            cx.notify();
            true
        }
        "tab" | "enter" => {
            accept(app, sel, window, cx);
            true
        }
        "escape" => {
            dismiss(cx);
            cx.notify();
            true
        }
        _ => false,
    }
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let a = cx.try_global::<Autocomplete>()?.active.as_ref()?;
    let tone = cx.try_global::<Pickers>().map(|p| p.prefs.tone).unwrap_or(0);
    let title: SharedString = match a.kind {
        Kind::Emoji => crate::t!("ac.emoji", q = a.query),
        Kind::Mention => if a.query.is_empty() { crate::t!("members.members") } else { crate::t!("ac.members", q = a.query) },
        Kind::Channel => if a.query.is_empty() { crate::t!("ac.text_channels") } else { crate::t!("ac.channels", q = a.query) },
    };
    let mut rows = div().flex().flex_col().pb(z(8.));
    let mut stickers_started = false;
    for (i, item) in a.items.iter().enumerate() {
        let selected = i == a.selected;
        let mut similar: Option<SharedString> = None;
        if matches!(item, Item::Sticker { .. }) && !stickers_started {
            stickers_started = true;
            if i > 0 {
                rows = rows.child(div().px(z(16.)).py(z(8.)).child(div().h(z(1.)).bg(tk(idx::BORDER_SUBTLE))));
            }
            rows = rows.child(
                div()
                    .px(z(8.))
                    .py(z(12.))
                    .text_size(z(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                    .child(crate::t!("ac.stickers", q = a.query)),
            );
        }
        let small = matches!(item, Item::Emoji(_) | Item::Sticker { .. });
        let (lead, main, hint): (AnyElement, SharedString, Option<SharedString>) = match item {
            Item::Sticker { data, source, similar: sim } => {
                similar = sim.clone();
                (crate::ui::pickers::sticker::sticker_graphic(data, 40.), data.name.clone(), source.clone())
            }
            Item::Emoji(pi) => (item_element(pi, tone, 20.), format!(":{}:", pi.name()).into(), None),
            Item::User(id) => {
                let u = app.users.get(id);
                (
                    crate::ui::avatar_with_status_overlay(u, 24., app.presence_of(*id), tk(idx::BACKGROUND_SURFACE_HIGH), move |d| {
                        d.when(selected, |d| d.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                    })
                    .into_any_element(),
                    u.map(|u| u.display_name.clone()).unwrap_or_else(|| crate::t!("common.unknown")),
                    u.map(|u| u.name.clone()),
                )
            }
            Item::Role { name, color, .. } => (
                div().w(z(16.)).text_color(color.map(c).unwrap_or_else(|| tk(idx::INTERACTIVE_TEXT_DEFAULT))).child("@").into_any_element(),
                format!("@{name}").into(),
                Some(crate::t!("ac.role")),
            ),
            Item::Everyone(e) => (
                div().w(z(16.)).child("@").into_any_element(),
                format!("@{e}").into(),
                Some(if *e == "here" { crate::t!("ac.here") } else { crate::t!("ac.everyone") }),
            ),
            Item::Channel(id) => {
                let ch = app.channels.get(id);
                let parent = ch.and_then(|c| c.parent).and_then(|p| app.channels.get(&p)).map(|p| p.name.clone());
                (
                    icon(channel_icon(ch.map_or(ChannelKind::Text, |c| c.kind)), 16., tk(idx::INTERACTIVE_TEXT_DEFAULT)).into_any_element(),
                    ch.map(|c| c.name.clone()).unwrap_or_default(),
                    parent,
                )
            }
        };
        rows = rows.child(
            div().px(z(8.)).child(
                div()
                    .id(("ac", i))
                    .when(small, |d| d.px(z(8.)).py(z(4.)))
                    .when(!small, |d| d.p(z(8.)))
                    .rounded(z(3.))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .when(selected, |d| d.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                    .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |a, _, w, cx| accept(a, i, w, cx)))
                    .text_size(z(14.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                    .child(div().flex_none().mr(z(8.)).child(lead))
                    .child(
                        div()
                            .flex_shrink(1.)
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(main)
                            .when_some(similar, |d, t| {
                                d.child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::NORMAL).child(crate::t!("ac.similar", q = t)))
                            }),
                    )
                    .when_some(hint, |d, h| {
                        d.child(div().flex_1().ml(z(16.)).min_w_0().overflow_hidden().whitespace_nowrap().flex().justify_end().child(h))
                    }),
            ),
        );
    }
    Some(
        div()
            .absolute()
            .left(z(8.))
            .right(z(16.))
            .bottom(gpui::relative(1.))
            .pb(z(8.))
            .child(
                div()
                    .rounded(z(5.))
                    .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
                    .shadow(crate::ui::inbox::popout_shadow())
                    .occlude()
                    .overflow_hidden()
                    .child(
                        div()
                            .px(z(8.))
                            .py(z(12.))
                            .text_size(z(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                            .child(title),
                    )
                    .child(rows),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticker_and_emoji_counts() {
        let st = |n: u64| Item::Sticker {
            data: StickerData {
                id: Id(n),
                name: "s".into(),
                description: "".into(),
                tags: "".into(),
                format: crate::media::StickerFormat::Png,
                guild: None,
            },
            source: None,
            similar: None,
        };
        let em = |n: u64| Item::Channel(Id(n));
        let count = |v: &[Item]| v.iter().filter(|i| matches!(i, Item::Sticker { .. })).count();
        let v = with_stickers((0..10).map(em).collect(), (0..6).map(st).collect());
        assert_eq!((v.len() - count(&v), count(&v)), (6, 4));
        let v = with_stickers((0..2).map(em).collect(), (0..9).map(st).collect());
        assert_eq!((v.len() - count(&v), count(&v)), (2, 6));
    }

    #[test]
    fn tokens() {
        assert_eq!(token_at("hi :th", 6), Some((Kind::Emoji, "th".into(), 3)));
        assert_eq!(token_at("hi :t", 5), None);
        assert_eq!(token_at("hi :thumbsup: x", 13), None);
        assert_eq!(token_at("@", 1), Some((Kind::Mention, "".into(), 0)));
        assert_eq!(token_at("go #gen", 7), Some((Kind::Channel, "gen".into(), 3)));
        assert_eq!(token_at("go #gen now", 11), None);
        assert_eq!(token_at("é @bo", 6), Some((Kind::Mention, "bo".into(), 3)));
        assert_eq!(token_at("mail a@b", 8), None);
    }
}

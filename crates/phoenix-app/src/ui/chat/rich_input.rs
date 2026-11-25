use std::{cell::RefCell, ops::Range};

use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ParentElement, Styled, Window, div};
use gpui_component::input::{InlineToken, InlineTokenContext, InputContent};

use crate::{
    app::PhoenixApp,
    markdown::MentionResolver,
    theme::{Palette, c},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    User(u64),
    Role(u64),
    Channel(u64),
    Emoji { name: String, animated: bool },
}

pub fn legacy(app: &PhoenixApp) -> bool {
    app.prefs.synced.flag("legacy-input")
}

pub fn enabled(app: &PhoenixApp) -> bool {
    app.prefs.synced.flag("md-preview") && !legacy(app)
}

fn id_of(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn kind(inner: &str) -> Option<Kind> {
    if let Some(r) = inner.strip_prefix("@&") {
        return id_of(r).map(Kind::Role);
    }
    if let Some(u) = inner.strip_prefix('@') {
        return id_of(u.strip_prefix('!').unwrap_or(u)).map(Kind::User);
    }
    if let Some(ch) = inner.strip_prefix('#') {
        return id_of(ch).map(Kind::Channel);
    }
    let (animated, rest) = match inner.strip_prefix("a:") {
        Some(r) => (true, r),
        None => (false, inner.strip_prefix(':')?),
    };
    let (name, id) = rest.split_once(':')?;
    id_of(id)?;
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then(|| Kind::Emoji { name: name.to_owned(), animated })
}

pub fn spans(text: &str) -> Vec<(Range<usize>, Kind)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = text[from..].find('<') {
        let start = from + i;
        let Some(len) = text[start + 1..].find(['>', '<']) else { break };
        let end = start + 1 + len;
        if text.as_bytes()[end] == b'>'
            && let Some(k) = kind(&text[start + 1..end])
        {
            out.push((start..end + 1, k));
            from = end + 1;
        } else {
            from = end;
        }
    }
    out
}

pub fn label(k: &Kind, r: &dyn MentionResolver) -> String {
    match k {
        Kind::User(id) => format!("@{}", r.user(*id).unwrap_or_else(|| crate::t!("tts.unknown_user").to_string())),
        Kind::Role(id) => format!("@{}", r.role(*id).unwrap_or_else(|| crate::t!("tts.unknown_role").to_string())),
        Kind::Channel(id) => format!("#{}", r.channel(*id).unwrap_or_else(|| crate::t!("tts.unknown_channel").to_string())),
        Kind::Emoji { name, .. } => format!(":{name}:"),
    }
}

pub fn content(text: &str, r: &dyn MentionResolver) -> InputContent {
    let mut content = InputContent::new(text.to_owned());
    for (range, k) in spans(text) {
        let raw = text[range.clone()].to_owned();
        let token = InlineToken::new(raw.clone(), raw).with_label(label(&k, r));
        if let Ok(next) = content.clone().with_token(range, token) {
            content = next;
        }
    }
    content
}

fn wanted(app: &PhoenixApp, cx: &App) -> InputContent {
    let text = app.composer.read(cx).value().to_string();
    if !enabled(app) {
        return InputContent::new(text);
    }
    match app.selected_channel {
        Some(ch) => content(&text, &app.resolver_with_sounds(&[], ch)),
        None => content(&text, &app.resolver()),
    }
}

pub fn sync(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let want = wanted(app, cx);
    app.composer.update(cx, |s, cx| {
        let _ = s.set_tokens(want, cx);
    });
}

pub fn needs_sync(app: &PhoenixApp, cx: &App) -> bool {
    let want = wanted(app, cx);
    let have = app.composer.read(cx).tokens();
    have.len() != want.tokens().len() || have.iter().zip(want.tokens()).any(|(a, b)| a.range() != b.range() || a.token().label() != b.token().label())
}

pub fn render_token(ctx: &InlineTokenContext, _: &mut Window, _: &mut App) -> AnyElement {
    let text = ctx.token().text().to_string();
    let size = f32::from(ctx.line_height());
    if text.starts_with("<:") || text.starts_with("<a:") {
        return div().h(ctx.line_height()).flex().items_center().child(crate::media::emoji::emoji_element(&text, size)).into_any_element();
    }
    div()
        .h(ctx.line_height())
        .flex()
        .items_center()
        .px(gpui::px(2.))
        .rounded(gpui::px(3.))
        .bg(c(Palette::MENTION_BG))
        .text_color(c(Palette::MENTION_FG))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(ctx.token().label().clone())
        .into_any_element()
}

thread_local! {
    static PLAIN: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

pub fn plain_text(k: &Kind, app: &PhoenixApp) -> Option<String> {
    Some(match k {
        Kind::User(id) => format!("@{}", app.users.get(&crate::model::Id(*id))?.name),
        Kind::Role(id) => format!("@{}", app.roles.values().flatten().find(|r| r.id.0 == *id)?.name),
        Kind::Channel(id) => format!("#{}", app.channels.get(&crate::model::Id(*id))?.name),
        Kind::Emoji { name, .. } => format!(":{name}:"),
    })
}

pub fn insertion(app: &PhoenixApp, raw: String) -> String {
    if enabled(app) {
        return raw;
    }
    let Some(k) = raw.strip_prefix('<').and_then(|r| r.strip_suffix('>')).and_then(kind) else { return raw };
    let Some(shown) = plain_text(&k, app) else { return raw };
    PLAIN.with_borrow_mut(|p| {
        p.retain(|(s, _)| *s != shown);
        p.push((shown.clone(), raw));
    });
    shown
}

fn boundary(c: Option<char>) -> bool {
    c.is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
}

pub fn resolve(text: &str, map: &[(String, String)]) -> String {
    let mut pairs: Vec<&(String, String)> = map.iter().collect();
    pairs.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'outer: while i < text.len() {
        for (shown, raw) in &pairs {
            if text[i..].starts_with(shown.as_str()) && boundary(text[i + shown.len()..].chars().next()) {
                out.push_str(raw);
                i += shown.len();
                continue 'outer;
            }
        }
        let ch = text[i..].chars().next().unwrap_or_default();
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    out
}

pub fn take_plain(text: &str) -> String {
    let map = PLAIN.with_borrow_mut(std::mem::take);
    if map.is_empty() { text.to_owned() } else { resolve(text, &map) }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct R;
    impl MentionResolver for R {
        fn user(&self, id: u64) -> Option<String> {
            (id == 1).then(|| "Alice".into())
        }
        fn channel(&self, id: u64) -> Option<String> {
            (id == 5).then(|| "général".into())
        }
        fn role(&self, id: u64) -> Option<String> {
            (id == 9).then(|| "Modo".into())
        }
    }

    #[test]
    fn finds_the_discord_tokens() {
        let t = "hi <@1> <@!2> <@&9> <#5> <:pog:12> <a:dance:3> <nope> <@x> <t:1:R>";
        let kinds: Vec<Kind> = spans(t).into_iter().map(|(_, k)| k).collect();
        assert_eq!(
            kinds,
            vec![
                Kind::User(1),
                Kind::User(2),
                Kind::Role(9),
                Kind::Channel(5),
                Kind::Emoji { name: "pog".into(), animated: false },
                Kind::Emoji { name: "dance".into(), animated: true },
            ]
        );
        assert_eq!(spans("<<@1>"), vec![(1..5, Kind::User(1))]);
    }

    #[test]
    fn builds_labelled_tokens() {
        let c = content("yo <@1> in <#5> <:pog:12>", &R);
        let labels: Vec<String> = c.tokens().iter().map(|t| t.token().label().to_string()).collect();
        assert_eq!(labels, vec!["@Alice", "#général", ":pog:"]);
        assert_eq!(c.tokens()[0].range(), 3..7);
        assert_eq!(c.text().as_ref(), "yo <@1> in <#5> <:pog:12>");
    }

    #[test]
    fn plain_names_go_back_to_ids_on_send() {
        let map = vec![("@bob".to_owned(), "<@7>".to_owned()), ("@bobby".to_owned(), "<@8>".to_owned()), (":pog:".to_owned(), "<:pog:12>".to_owned())];
        assert_eq!(resolve("hey @bobby and @bob :pog: @bobx", &map), "hey <@8> and <@7> <:pog:12> @bobx");
        assert_eq!(resolve("café @bob.", &map), "café <@7>.");
    }
}

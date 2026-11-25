use std::collections::HashMap;

use gpui::{AnyElement, App, Context, FontWeight, ParentElement, SharedString, Styled, Window, div, prelude::*};
use serde_json::Value;

use crate::{
    app::PhoenixApp,
    commands::{ParityReply, ParityRequest, SlashCommand, slash},
    model::Id,
    theme::{Palette, c, z},
};

#[derive(Default)]
pub struct SlashUi {
    cache: HashMap<Id, Vec<SlashCommand>>,
    loading: Option<Id>,
    selected: usize,
    dismissed: Option<String>,
}

impl gpui::Global for SlashUi {}

fn state(cx: &mut App) -> &mut SlashUi {
    if cx.try_global::<SlashUi>().is_none() {
        cx.set_global(SlashUi::default());
    }
    cx.global_mut::<SlashUi>()
}

pub fn type_hint(kind: u64) -> &'static str {
    match kind {
        4 => "integer",
        5 => "true/false",
        6 => "user",
        7 => "channel",
        8 => "role",
        9 => "mentionable",
        10 => "number",
        11 => "attachment",
        _ => "text",
    }
}

fn text(app: &PhoenixApp, cx: &App) -> String {
    app.composer.read(cx).value().to_string()
}

fn name_query(t: &str) -> Option<&str> {
    let q = t.strip_prefix('/')?;
    (!q.contains(char::is_whitespace)).then_some(q)
}

pub fn ensure_loaded(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(ch) = app.selected_channel else { return };
    let st = state(cx);
    if st.cache.contains_key(&ch) || st.loading == Some(ch) {
        return;
    }
    st.loading = Some(ch);
    let guild = app.channels.get(&ch).and_then(|c| c.guild);
    let fut = app.backend.parity(ParityRequest::Commands { guild, channel: ch });
    cx.spawn(async move |this, cx| {
        let list = match fut.await {
            Ok(ParityReply::Commands(v)) => v,
            _ => Vec::new(),
        };
        let _ = this.update(cx, |_, cx| {
            let st = state(cx);
            st.loading = None;
            st.cache.insert(ch, list);
            cx.notify();
        });
    })
    .detach();
}

pub fn cached_commands(app: &PhoenixApp, cx: &App) -> Vec<SlashCommand> {
    let Some(ch) = app.selected_channel else { return vec![] };
    cx.try_global::<SlashUi>().and_then(|s| s.cache.get(&ch)).cloned().unwrap_or_default()
}

pub const TTS_ID: &str = "-4";

pub fn builtins(app: &PhoenixApp) -> Vec<SlashCommand> {
    let Some(ch) = app.selected_channel else { return vec![] };
    if !crate::tts::can_use_command(app, ch) {
        return vec![];
    }
    vec![SlashCommand {
        name: "tts".into(),
        description: crate::t!("tts.cmd_desc").to_string(),
        app_name: crate::t!("slash.built_in").to_string(),
        app_icon: None,
        raw: serde_json::json!({
            "id": TTS_ID,
            "name": "tts",
            "type": 1,
            "options": [{"name": "message", "type": 3, "required": true, "description": crate::t!("tts.cmd_message").to_string()}],
        }),
    }]
}

fn is_builtin(cmd: &SlashCommand) -> bool {
    cmd.raw.get("id").and_then(Value::as_str) == Some(TTS_ID)
}

fn matches(app: &PhoenixApp, q: &str, cx: &App) -> Vec<SlashCommand> {
    let Some(ch) = app.selected_channel else { return vec![] };
    let cached = cx.try_global::<SlashUi>().and_then(|s| s.cache.get(&ch));
    if cached.is_none() {
        return vec![];
    }
    let all: Vec<SlashCommand> = builtins(app).into_iter().chain(cached.into_iter().flatten().cloned()).collect();
    let q = q.to_lowercase();
    let mut v: Vec<SlashCommand> = all.iter().filter(|c| c.name.to_lowercase().starts_with(&q)).cloned().collect();
    if v.is_empty() && !q.is_empty() {
        v = all.iter().filter(|c| c.name.to_lowercase().contains(&q)).cloned().collect();
    }
    v.sort_by(|a, b| a.app_name.cmp(&b.app_name).then(a.name.cmp(&b.name)));
    v.truncate(25);
    v
}

fn find(app: &PhoenixApp, name: &str, cx: &App) -> Option<SlashCommand> {
    let ch = app.selected_channel?;
    if let Some(b) = builtins(app).into_iter().find(|c| c.name == name) {
        return Some(b);
    }
    cx.try_global::<SlashUi>()?.cache.get(&ch)?.iter().find(|c| c.name == name).cloned()
}

fn list_open(app: &PhoenixApp, cx: &App) -> Option<(String, Vec<SlashCommand>)> {
    let t = text(app, cx);
    let q = name_query(&t)?;
    if cx.try_global::<SlashUi>().is_some_and(|s| s.dismissed.as_deref() == Some(t.as_str())) {
        return None;
    }
    let m = matches(app, q, cx);
    (!m.is_empty()).then(|| (t.clone(), m))
}

fn fill(app: &mut PhoenixApp, cmd: &SlashCommand, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let v = format!("/{} ", cmd.name);
    app.composer.update(cx, |s, cx| s.set_value(v, window, cx));
    state(cx).selected = 0;
    cx.notify();
}

pub fn on_key(app: &mut PhoenixApp, key: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    if super::rich_input::legacy(app) {
        return false;
    }
    let Some((t, items)) = list_open(app, cx) else { return false };
    let n = items.len();
    let sel = state(cx).selected.min(n - 1);
    match key {
        "up" => state(cx).selected = (sel + n - 1) % n,
        "down" => state(cx).selected = (sel + 1) % n,
        "tab" | "enter" => fill(app, &items[sel], window, cx),
        "escape" => state(cx).dismissed = Some(t),
        _ => return false,
    }
    cx.notify();
    true
}

pub fn submit(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    let t = text(app, cx);
    let Some((name, args)) = slash::split_invocation(t.trim()) else { return false };
    let Some(cmd) = find(app, name, cx) else { return false };
    let Some(ch) = app.selected_channel else { return false };
    if super::rich_input::legacy(app) && !is_builtin(&cmd) {
        return false;
    }
    if is_builtin(&cmd) {
        let message = args.trim().to_owned();
        if message.is_empty() {
            return true;
        }
        app.composer.update(cx, |s, cx| s.set_value("", window, cx));
        app.chat.error = None;
        let fut = app.backend.send_tts_message(ch, message);
        cx.spawn(async move |this, cx| match fut.await {
            Ok(m) => {
                let _ = this.update(cx, |a, cx| a.append_message(m, cx));
            }
            Err(e) => {
                let _ = this.update(cx, |a, cx| {
                    a.chat.error = Some(crate::t!("slash.failed", e = format!("{e:?}")));
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
        return true;
    }
    if let Err(e) = slash::build_options(&cmd.raw, args) {
        app.chat.error = Some(e.into());
        cx.notify();
        return true;
    }
    let guild = app.channels.get(&ch).and_then(|c| c.guild);
    let fut = app.backend.parity(ParityRequest::RunCommand { guild, channel: ch, command: cmd.raw.clone(), args: args.to_owned() });
    app.composer.update(cx, |s, cx| s.set_value("", window, cx));
    app.chat.error = None;
    cx.spawn(async move |this, cx| {
        if let Err(e) = fut.await {
            let _ = this.update(cx, |a, cx| {
                a.chat.error = Some(crate::t!("slash.failed", e = format!("{e:?}")));
                cx.notify();
            });
        }
    })
    .detach();
    cx.notify();
    true
}

fn app_icon(cmd: &SlashCommand, size: f32) -> AnyElement {
    if let Some(url) = &cmd.app_icon {
        return gpui::img(SharedString::from(url.clone())).size(z(size)).flex_none().rounded_full().into_any_element();
    }
    let initial: SharedString = cmd.app_name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default().into();
    div()
        .size(z(size))
        .flex_none()
        .rounded_full()
        .bg(c(Palette::BLURPLE))
        .flex()
        .items_center()
        .justify_center()
        .text_size(z(size / 2.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(gpui::white())
        .child(initial)
        .into_any_element()
}

fn option_list(raw: &Value) -> Vec<(String, u64, bool, String)> {
    raw.get("options")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|o| {
                    (
                        o.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
                        o.get("type").and_then(Value::as_u64).unwrap_or(3),
                        o.get("required").and_then(Value::as_bool).unwrap_or(false),
                        o.get("description").and_then(Value::as_str).unwrap_or("").to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn popup() -> gpui::Div {
    div()
        .absolute()
        .left(z(8. + crate::theme::space::xs()))
        .right(z(16. + crate::theme::space::xs()))
        .bottom(z(8.))
        .rounded(z(5.))
        .bg(super::tk(crate::theme::generated::idx::BACKGROUND_SURFACE_HIGH))
        .shadow(crate::ui::inbox::popout_shadow())
        .overflow_hidden()
        .flex()
        .flex_col()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let t = text(app, cx);
    if !t.starts_with('/') || super::rich_input::legacy(app) {
        return None;
    }
    ensure_loaded(app, cx);
    let anchor = |inner: gpui::Div| div().relative().h(z(0.)).child(inner).into_any_element();
    if name_query(&t).is_some() {
        let loading = cx.try_global::<SlashUi>().is_some_and(|s| s.loading == app.selected_channel);
        let Some((_, items)) = list_open(app, cx) else {
            return loading.then(|| {
                anchor(popup().p(z(12.)).text_size(z(14.)).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("slash.loading")))
            });
        };
        let sel = cx.try_global::<SlashUi>().map_or(0, |s| s.selected).min(items.len() - 1);
        use crate::theme::generated::idx;
        let mut rail = div()
            .w(z(48.))
            .h_full()
            .flex_none()
            .bg(super::tk(idx::BACKGROUND_BASE_LOWEST))
            .pt(z(16.))
            .pb(z(8.))
            .flex()
            .flex_col()
            .items_center()
            .gap(z(8.));
        let mut list = div().id("slash-list").flex_1().min_w_0().h_full().overflow_y_scroll().track_scroll(&crate::ui::chat::scrollbar::handle("slash-list")).pt(z(8.)).px(z(8.)).flex().flex_col();
        let mut last_app: Option<&str> = None;
        for (i, cmd) in items.iter().enumerate() {
            if last_app != Some(cmd.app_name.as_str()) {
                if last_app.is_some() {
                    list = list.child(div().h(z(16.)).flex_none());
                }
                last_app = Some(cmd.app_name.as_str());
                rail = rail.child(div().size(z(32.)).flex_none().rounded(z(4.)).cursor_pointer().child(app_icon(cmd, 32.)));
                list = list.child(
                    div()
                        .h(z(32.))
                        .flex_none()
                        .bg(super::tk(idx::BACKGROUND_SURFACE_HIGH))
                        .pl(z(8.))
                        .pr(z(4.))
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(app_icon(cmd, 16.))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(super::tk(idx::TEXT_DEFAULT))
                                .child(SharedString::from(cmd.app_name.clone())),
                        ),
                );
            }
            let opts = option_list(&cmd.raw);
            let usage: String = opts
                .iter()
                .filter(|o| o.2)
                .map(|o| format!(" {}", o.0))
                .collect();
            let c2 = cmd.clone();
            list = list.child(
                div()
                    .id(("slash-cmd", i))
                    .h(z(56.))
                    .flex_none()
                    .p(z(8.))
                    .rounded(z(3.))
                    .cursor_pointer()
                    .when(i == sel, |d| d.bg(super::tk(crate::theme::generated::idx::INTERACTIVE_BACKGROUND_HOVER)))
                    .hover(|s| s.bg(super::tk(crate::theme::generated::idx::INTERACTIVE_BACKGROUND_HOVER)))
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .child(
                        div()
                            .flex_none()
                            .text_size(z(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(Palette::TEXT_HEADER))
                            .child(SharedString::from(format!("/{}", cmd.name)))
                            .child(div().text_color(c(Palette::TEXT_MUTED)).child(SharedString::from(usage))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_size(z(14.))
                            .text_color(c(Palette::TEXT_MUTED))
                            .child(SharedString::from(cmd.description.clone())),
                    )
                    .child(div().flex_none().text_size(z(12.)).text_color(c(Palette::TEXT_MUTED)).child(SharedString::from(cmd.app_name.clone())))
                    .on_click(cx.listener(move |a, _, w, cx| fill(a, &c2, w, cx))),
            );
        }
        let list = crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Thin).flex_1().min_w_0().h_full().child(list).children(crate::ui::chat::scrollbar::bar_for("slash-list", crate::ui::chat::scrollbar::Bar::Thin));
        return Some(anchor(popup().h(z(420.)).flex_row().child(rail).child(list)));
    }
    let (name, args) = slash::split_invocation(t.trim_start())?;
    let cmd = find(app, name, cx)?;
    let opts = option_list(&cmd.raw);
    let given: Vec<String> = args
        .split_whitespace()
        .filter_map(|w| w.split_once(':').map(|(k, _)| k.to_lowercase()))
        .collect();
    let mut chips = div().flex().flex_wrap().gap(z(6.)).items_center();
    chips = chips.child(
        div()
            .mr(z(4.))
            .text_size(z(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(Palette::TEXT_HEADER))
            .child(SharedString::from(format!("/{}", cmd.name))),
    );
    let mut desc: Option<String> = None;
    for (n, kind, req, d) in &opts {
        let filled = given.contains(n);
        if !filled && desc.is_none() {
            desc = Some(d.clone());
        }
        chips = chips.child(
            div()
                .rounded(z(4.))
                .border_1()
                .border_color(super::tk(crate::theme::generated::idx::BACKGROUND_BASE_LOWEST))
                .bg(super::tk(crate::theme::generated::idx::BACKGROUND_MOD_NORMAL))
                .flex()
                .text_size(z(14.))
                .child(div().px(z(8.)).py(z(1.)).rounded_l(z(4.)).bg(super::tk(crate::theme::generated::idx::BACKGROUND_SURFACE_HIGH)).text_color(c(Palette::TEXT_NORMAL)).child(SharedString::from(n.to_string())))
                .child(div().px(z(8.)).py(z(1.)).text_color(c(Palette::TEXT_MUTED)).child(type_hint(*kind)))
                .when(*req && !filled, |d| d.child(div().text_color(c(Palette::RED)).child("*"))),
        );
    }
    let bar = popup()
        .p(z(12.))
        .gap(z(6.))
        .child(chips)
        .child(
            div()
                .text_size(z(12.))
                .text_color(c(Palette::TEXT_MUTED))
                .child(SharedString::from(desc.unwrap_or(cmd.description.clone()))),
        );
    Some(anchor(bar))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_only_while_typing_name() {
        assert_eq!(name_query("/ba"), Some("ba"));
        assert_eq!(name_query("/"), Some(""));
        assert_eq!(name_query("/ban user:1"), None);
        assert_eq!(name_query("hi"), None);
        assert_eq!(type_hint(6), "user");
    }
}

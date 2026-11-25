use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use gpui::{AnyElement, App, Context, FontWeight, ParentElement, SharedString, Styled, Window, div, prelude::*};
use serde_json::{Value, json};

use crate::{
    app::PhoenixApp,
    commands::{ParityReply, ParityRequest, SlashCommand, builtin, slash},
    model::Id,
    theme::{Palette, c, fz, generated::idx, tok, z},
    ui::chat::command_msg,
};

static NONCE_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn next_nonce() -> u64 {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let seq = NONCE_SEQ.fetch_add(1, Ordering::Relaxed) & 0xfff;
    phoenix_api::models::Snowflake::from_timestamp_ms(ms).0 | seq
}

#[derive(Default)]
pub struct SlashUi {
    cache: HashMap<Id, Vec<SlashCommand>>,
    loading: Option<Id>,
    selected: usize,
    dismissed: Option<String>,
    auto_nonce: Option<u64>,
    auto_key: String,
    auto_choices: Vec<(String, Value)>,
}

impl gpui::Global for SlashUi {}

fn state(cx: &mut App) -> &mut SlashUi {
    if cx.try_global::<SlashUi>().is_none() {
        cx.set_global(SlashUi::default());
    }
    cx.global_mut::<SlashUi>()
}

pub fn autocomplete_result(nonce: u64, choices: Vec<(String, Value)>, cx: &mut App) {
    let st = state(cx);
    if st.auto_nonce == Some(nonce) {
        st.auto_choices = choices;
    }
}

pub fn autocomplete_failed(nonce: u64, cx: &mut App) -> bool {
    let st = state(cx);
    if st.auto_nonce == Some(nonce) {
        st.auto_nonce = None;
        return true;
    }
    false
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

pub fn context_commands(app: &PhoenixApp, kind: u8, cx: &App) -> Vec<SlashCommand> {
    let mut cmds: Vec<SlashCommand> = cached_commands(app, cx).into_iter().filter(|c| c.kind == kind).collect();
    cmds.sort_by(|a, b| a.name.cmp(&b.name));
    cmds
}

pub fn run_context_command(app: &mut PhoenixApp, cmd: &SlashCommand, channel: Id, target: Id, cx: &mut Context<PhoenixApp>) {
    let guild = app.channels.get(&channel).and_then(|c| c.guild);
    let nonce = next_nonce();
    let bot = cmd.bot_id.or(cmd.app_id).unwrap_or(Id(0));
    command_msg::track(
        nonce,
        command_msg::Pending { channel, app_name: cmd.app_name.clone(), phase: command_msg::Phase::Sending, interaction: None },
    );
    let ph = command_msg::placeholder(app, channel, nonce, bot, &cmd.name, cmd.kind, (cmd.kind == 2).then_some(target));
    app.append_message(ph, cx);
    let fut = app.backend.parity(ParityRequest::ContextCommand { guild, channel, command: cmd.raw.clone(), target, nonce });
    cx.spawn(async move |this, cx| {
        if let Err(e) = fut.await {
            let _ = this.update(cx, |a, cx| {
                command_msg::set_phase(nonce, command_msg::Phase::Failed(Some(crate::t!("slash.failed", e = format!("{e:?}")).to_string())));
                a.refresh_message(channel, Id(nonce));
                cx.notify();
            });
        }
    })
    .detach();
    cx.notify();
}

pub fn builtins(app: &PhoenixApp) -> Vec<SlashCommand> {
    let Some(ch) = app.selected_channel else { return vec![] };
    builtin::list(&builtin::Context::of(app, ch))
}

fn all_slash(app: &PhoenixApp, cx: &App) -> Vec<SlashCommand> {
    let Some(ch) = app.selected_channel else { return vec![] };
    let cached = cx.try_global::<SlashUi>().and_then(|s| s.cache.get(&ch));
    builtins(app)
        .into_iter()
        .chain(cached.into_iter().flatten().filter(|c| c.kind == 1).cloned())
        .collect()
}

fn matches(app: &PhoenixApp, q: &str, cx: &App) -> Vec<SlashCommand> {
    let all = all_slash(app, cx);
    let q = q.trim();
    if q.is_empty() {
        let mut v = all;
        v.sort_by(|a, b| a.builtin().cmp(&b.builtin()).then(a.app_name.cmp(&b.app_name)).then(a.name.cmp(&b.name)));
        v.truncate(50);
        return v;
    }
    let mut scored: Vec<(u8, SlashCommand)> = all
        .into_iter()
        .filter_map(|c| slash::score(&c.name, &c.description, c.options(), q).map(|s| (s, c)))
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.name.cmp(&b.1.name)));
    scored.truncate(25);
    scored.into_iter().map(|(_, c)| c).collect()
}

fn find(app: &PhoenixApp, text: &str, cx: &App) -> Option<(SlashCommand, String)> {
    let all = all_slash(app, cx);
    let (cmd, rest) = slash::match_command(text, &all, |c| c.name.as_str())?;
    Some((cmd.clone(), rest.to_owned()))
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
    let first_req = cmd.options().iter().find(|o| slash::required(o)).map(slash::name);
    let v = match first_req {
        Some(o) => format!("/{} {}:", cmd.name, o),
        None => format!("/{} ", cmd.name),
    };
    app.composer.update(cx, |s, cx| s.set_value(v, window, cx));
    let st = state(cx);
    st.selected = 0;
    st.auto_choices.clear();
    st.auto_key.clear();
    cx.notify();
}

fn fill_option(app: &mut PhoenixApp, cmd_name: &str, opt_name: &str, val: Option<&str>, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let cur = text(app, cx);
    let next = if let Some(v) = val {
        let prefix = format!("{opt_name}:");
        if let Some(pos) = cur.rfind(&prefix) {
            format!("{}{prefix}{v} ", &cur[..pos])
        } else {
            format!("{} {opt_name}:{v} ", cur.trim_end())
        }
    } else if cur.trim_end().ends_with(&format!("/{cmd_name}")) {
        format!("/{cmd_name} {opt_name}:")
    } else {
        format!("{} {opt_name}:", cur.trim_end())
    };
    app.composer.update(cx, |s, cx| s.set_value(next, window, cx));
    cx.notify();
}

fn maybe_autocomplete(app: &PhoenixApp, cmd: &SlashCommand, args: &str, cx: &mut Context<PhoenixApp>) {
    if cmd.builtin() {
        return;
    }
    let Some((opt_name, raw_val)) = slash::option_at(args, args.len()) else { return };
    let Some(opt) = cmd.options().iter().find(|o| slash::name(o) == opt_name && slash::autocomplete(o)) else { return };
    let Some(ch) = app.selected_channel else { return };
    let key = format!("{}:{}:{opt_name}:{raw_val}", ch.0, cmd.id);
    if state(cx).auto_key == key {
        return;
    }
    let nonce = next_nonce();
    {
        let st = state(cx);
        st.auto_key = key;
        st.auto_nonce = Some(nonce);
    }
    let mut leaf_opts = Vec::new();
    for (k, v) in slash::pairs(args) {
        if k == opt_name {
            leaf_opts.push(json!({ "name": k, "type": slash::kind(opt), "value": raw_val, "focused": true }));
        } else if let Some(o) = cmd.options().iter().find(|o| slash::name(o) == k)
            && let Ok(tv) = slash::typed(o, &v, &[])
        {
            leaf_opts.push(json!({ "name": k, "type": slash::kind(o), "value": tv }));
        }
    }
    if !leaf_opts.iter().any(|o| o.get("name").and_then(Value::as_str) == Some(opt_name.as_str())) {
        leaf_opts.push(json!({ "name": opt_name, "type": slash::kind(opt), "value": raw_val, "focused": true }));
    }
    let options = slash::wrap(&cmd.path, leaf_opts);
    let guild = app.channels.get(&ch).and_then(|c| c.guild);
    let fut = app.backend.parity(ParityRequest::Autocomplete { guild, channel: ch, command: cmd.raw.clone(), options, nonce });
    cx.spawn(async move |_, _| {
        let _ = fut.await;
    })
    .detach();
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
    let Some((cmd, args)) = find(app, t.trim(), cx) else { return false };
    let Some(ch) = app.selected_channel else { return false };
    if super::rich_input::legacy(app) && !cmd.builtin() {
        return false;
    }
    let auto = cx.try_global::<SlashUi>().map(|s| s.auto_choices.clone()).unwrap_or_default();
    let vals = match slash::leaf_values(&cmd.leaf, &args, &auto) {
        Ok(v) => v,
        Err(err) => {
            app.chat.error = Some(if err.option.is_empty() {
                err.message.into()
            } else {
                format!("{}: {}", err.option, err.message).into()
            });
            cx.notify();
            return true;
        }
    };
    if cmd.builtin() {
        if cmd.id == "-4" {
            let message = builtin::text(&vals, "message");
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
        if let Some(content) = builtin::content(&cmd.id, &vals) {
            app.composer.update(cx, |s, cx| s.set_value("", window, cx));
            app.chat.error = None;
            let fut = app.backend.send_message(ch, content, None);
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
        app.composer.update(cx, |s, cx| s.set_value("", window, cx));
        app.chat.error = None;
        cx.notify();
        return true;
    }
    let options = slash::wrap(&cmd.path, vals);
    let guild = app.channels.get(&ch).and_then(|c| c.guild);
    let nonce = next_nonce();
    let bot = cmd.bot_id.or(cmd.app_id).unwrap_or(Id(0));
    command_msg::track(
        nonce,
        command_msg::Pending { channel: ch, app_name: cmd.app_name.clone(), phase: command_msg::Phase::Sending, interaction: None },
    );
    let ph = command_msg::placeholder(app, ch, nonce, bot, &cmd.name, cmd.kind, None);
    app.append_message(ph, cx);
    let fut = app.backend.parity(ParityRequest::RunCommand { guild, channel: ch, command: cmd.raw.clone(), options, nonce });
    app.composer.update(cx, |s, cx| s.set_value("", window, cx));
    app.chat.error = None;
    cx.spawn(async move |this, cx| {
        if let Err(e) = fut.await {
            let _ = this.update(cx, |a, cx| {
                command_msg::set_phase(nonce, command_msg::Phase::Failed(Some(crate::t!("slash.failed", e = format!("{e:?}")).to_string())));
                a.refresh_message(ch, Id(nonce));
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

fn popup() -> gpui::Div {
    div()
        .absolute()
        .left(z(8. + crate::theme::space::xs()))
        .right(z(16. + crate::theme::space::xs()))
        .bottom(z(8.))
        .rounded(z(5.))
        .bg(super::tk(idx::BACKGROUND_SURFACE_HIGH))
        .shadow(crate::ui::inbox::popout_shadow())
        .overflow_hidden()
        .flex()
        .flex_col()
}

fn command_row(i: usize, sel: usize, cmd: &SlashCommand, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let usage: String = cmd.options().iter().filter(|o| slash::required(o)).map(|o| format!(" {}", slash::name(o))).collect();
    let c2 = cmd.clone();
    div()
        .id(("slash-cmd", i))
        .h(z(56.))
        .flex_none()
        .px(z(8.))
        .py(z(8.))
        .rounded(z(3.))
        .cursor_pointer()
        .when(i == sel, |d| d.bg(super::tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
        .hover(|s| s.bg(super::tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
        .flex()
        .items_center()
        .gap(z(12.))
        .child(app_icon(cmd, 32.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .text_size(fz(16.))
                        .line_height(fz(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(SharedString::from(format!("/{}", cmd.name)))
                        .when(!usage.is_empty(), |d| d.child(div().text_size(fz(14.)).text_color(c(tok(idx::TEXT_MUTED))).child(SharedString::from(usage)))),
                )
                .child(
                    div()
                        .mt(z(4.))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_size(fz(12.))
                        .line_height(fz(16.))
                        .text_color(c(tok(idx::TEXT_MUTED)))
                        .child(SharedString::from(cmd.description.clone())),
                ),
        )
        .child(
            div()
                .flex_none()
                .ml(z(16.))
                .text_size(fz(12.))
                .line_height(fz(16.))
                .text_color(c(tok(idx::TEXT_MUTED)))
                .child(SharedString::from(cmd.app_name.clone())),
        )
        .on_click(cx.listener(move |a, _, w, cx| fill(a, &c2, w, cx)))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let t = text(app, cx);
    if !t.starts_with('/') || super::rich_input::legacy(app) {
        return None;
    }
    ensure_loaded(app, cx);
    let anchor = |inner: gpui::Div| div().relative().h(z(0.)).child(inner).into_any_element();
    if let Some(q) = name_query(&t) {
        let loading = cx.try_global::<SlashUi>().is_some_and(|s| s.loading == app.selected_channel);
        let Some((_, items)) = list_open(app, cx) else {
            return loading.then(|| anchor(popup().p(z(12.)).text_size(z(14.)).text_color(c(Palette::TEXT_MUTED)).child(crate::t!("slash.loading"))));
        };
        let sel = cx.try_global::<SlashUi>().map_or(0, |s| s.selected).min(items.len() - 1);
        let mut list = div()
            .id("slash-list")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&crate::ui::chat::scrollbar::handle("slash-list"))
            .p(z(8.))
            .flex()
            .flex_col();
        if !q.is_empty() {
            list = list.child(
                div()
                    .px(z(8.))
                    .py(z(4.))
                    .text_size(fz(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_MUTED)))
                    .child(crate::t!("slash.matching", q = q)),
            );
            for (i, cmd) in items.iter().enumerate() {
                list = list.child(command_row(i, sel, cmd, cx));
            }
            let list = crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Thin)
                .flex_1()
                .min_w_0()
                .h_full()
                .child(list)
                .children(crate::ui::chat::scrollbar::bar_for("slash-list", crate::ui::chat::scrollbar::Bar::Thin));
            return Some(anchor(popup().max_h(z(490.)).h(z((items.len() as f32 * 56. + 52.).min(490.))).child(list)));
        }
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
        let mut last_app: Option<&str> = None;
        for (i, cmd) in items.iter().enumerate() {
            if last_app != Some(cmd.app_name.as_str()) {
                if last_app.is_some() {
                    list = list.child(div().h(z(12.)).flex_none());
                }
                last_app = Some(cmd.app_name.as_str());
                rail = rail.child(div().size(z(32.)).flex_none().rounded(z(4.)).cursor_pointer().child(app_icon(cmd, 32.)));
                list = list.child(
                    div()
                        .h(z(32.))
                        .flex_none()
                        .pl(z(8.))
                        .pr(z(4.))
                        .flex()
                        .items_center()
                        .gap(z(6.))
                        .child(app_icon(cmd, 16.))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(fz(14.))
                                .line_height(fz(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(tok(idx::TEXT_DEFAULT)))
                                .child(SharedString::from(cmd.app_name.clone())),
                        ),
                );
            }
            list = list.child(command_row(i, sel, cmd, cx));
        }
        let list = crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::Thin)
            .flex_1()
            .min_w_0()
            .h_full()
            .child(list)
            .children(crate::ui::chat::scrollbar::bar_for("slash-list", crate::ui::chat::scrollbar::Bar::Thin));
        return Some(anchor(popup().h(z(420.)).flex_row().child(rail).child(list)));
    }
    let (cmd, args) = find(app, t.trim_start(), cx)?;
    maybe_autocomplete(app, &cmd, &args, cx);
    let given: Vec<String> = slash::pairs(&args).into_iter().map(|(k, _)| k.to_lowercase()).collect();
    let active_opt = slash::option_at(&args, args.len());
    let mut chips = div().flex().flex_wrap().gap(z(6.)).items_center();
    chips = chips.child(
        div()
            .mr(z(4.))
            .text_size(fz(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(tok(idx::TEXT_STRONG)))
            .child(SharedString::from(format!("/{}", cmd.name))),
    );
    let mut desc: Option<String> = None;
    for (i, o) in cmd.options().iter().enumerate() {
        let n = slash::name(o);
        let kind = slash::kind(o);
        let req = slash::required(o);
        let d = slash::description(o);
        let filled = given.iter().any(|g| g == n);
        if active_opt.as_ref().is_some_and(|(k, _)| k == n) || (!filled && desc.is_none()) {
            desc = Some(d.to_owned());
        }
        let cn = cmd.name.clone();
        let on = n.to_owned();
        chips = chips.child(
            div()
                .id(("slash-opt", i))
                .cursor_pointer()
                .rounded(z(4.))
                .border_1()
                .border_color(super::tk(idx::BACKGROUND_BASE_LOWEST))
                .bg(super::tk(idx::BACKGROUND_MOD_NORMAL))
                .flex()
                .text_size(fz(14.))
                .child(div().px(z(8.)).py(z(1.)).rounded_l(z(4.)).bg(super::tk(idx::BACKGROUND_SURFACE_HIGH)).text_color(c(tok(idx::TEXT_DEFAULT))).child(SharedString::from(n.to_owned())))
                .child(div().px(z(8.)).py(z(1.)).text_color(c(tok(idx::TEXT_MUTED))).child(type_hint(kind)))
                .when(req && !filled, |d| d.child(div().pr(z(6.)).text_color(c(Palette::RED)).child("*")))
                .on_click(cx.listener(move |a, _, w, cx| fill_option(a, &cn, &on, None, w, cx))),
        );
    }
    let mut choices_el: Option<gpui::Div> = None;
    if let Some((opt_name, raw_val)) = &active_opt
        && let Some(o) = cmd.options().iter().find(|o| slash::name(o) == opt_name)
    {
        let q = raw_val.to_lowercase();
        let mut items: Vec<(String, String)> = Vec::new();
        if let Some(ch_list) = slash::choices(o) {
            for c_val in ch_list {
                let label = slash::choice_name(c_val).to_owned();
                let val = c_val.get("value").map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string())).unwrap_or_default();
                if q.is_empty() || label.to_lowercase().contains(&q) || val.to_lowercase().contains(&q) {
                    items.push((label, val));
                }
            }
        } else if slash::kind(o) == slash::BOOLEAN {
            for b in ["true", "false"] {
                if q.is_empty() || b.starts_with(&q) {
                    items.push((b.to_owned(), b.to_owned()));
                }
            }
        } else if slash::autocomplete(o)
            && let Some(st) = cx.try_global::<SlashUi>()
        {
            for (label, v) in &st.auto_choices {
                let val = v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string());
                items.push((label.clone(), val));
            }
        }
        if !items.is_empty() {
            let mut list = div().mt(z(6.)).pt(z(6.)).border_t_1().border_color(super::tk(idx::BORDER_SUBTLE)).flex().flex_col().gap(z(2.));
            for (i, (label, val)) in items.into_iter().take(10).enumerate() {
                let cn = cmd.name.clone();
                let on = opt_name.clone();
                let v2 = val.clone();
                list = list.child(
                    div()
                        .id(("slash-choice", i))
                        .px(z(8.))
                        .py(z(4.))
                        .rounded(z(3.))
                        .cursor_pointer()
                        .hover(|s| s.bg(super::tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                        .text_size(fz(14.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(SharedString::from(label))
                        .on_click(cx.listener(move |a, _, w, cx| fill_option(a, &cn, &on, Some(&v2), w, cx))),
                );
            }
            choices_el = Some(list);
        }
    }
    let bar = popup()
        .p(z(12.))
        .gap(z(6.))
        .child(chips)
        .child(
            div()
                .text_size(fz(12.))
                .text_color(c(tok(idx::TEXT_MUTED)))
                .child(SharedString::from(desc.unwrap_or(cmd.description.clone()))),
        )
        .children(choices_el);
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

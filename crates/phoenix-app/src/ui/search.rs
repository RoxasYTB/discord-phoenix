use crate::theme::z;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, ParentElement, SharedString, Styled, Subscription,
    Window, div, prelude::*, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use phoenix_api::rest::search::{date_to_snowflake, parse_search};

use crate::{
    search_highlight::Highlighter,
    app::{PhoenixApp, format_time_full},
    assets::Icon,
    model::{ChannelKind, Id, Message},
    theme::{Palette, c, generated::idx, m as layout},
    ui::avatar,
};

pub const PAGE: u32 = 25;
pub const WIDTH: f32 = 420.;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchRequest {
    pub guild: Option<Id>,
    pub channel: Option<Id>,
    pub all_dms: bool,
    pub content: Option<String>,
    pub authors: Vec<Id>,
    pub mentions: Vec<Id>,
    pub channels: Vec<Id>,
    pub has: Vec<String>,
    pub min_id: Option<Id>,
    pub max_id: Option<Id>,
    pub pinned: Option<bool>,
    pub author_types: Vec<String>,
    pub offset: u32,
    pub sort: SortOrder,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    Newest,
    Oldest,
    Relevance,
}

#[derive(Clone, Debug, Default)]
pub struct SearchPage {
    pub total: u32,
    pub messages: Vec<Message>,
}

pub struct SearchState {
    pub input: Entity<InputState>,
    pub request: Option<SearchRequest>,
    pub page: Option<SearchPage>,
    pub loading: bool,
    pub error: Option<SharedString>,
    sort_menu: bool,
    dm_menu: bool,
    seq: u64,
    idle: bool,
    _sub: Subscription,
}

#[derive(Default)]
pub struct SearchHistory(pub Vec<String>);

impl gpui::Global for SearchHistory {}

#[derive(Default)]
pub struct Search(pub Option<SearchState>);

impl gpui::Global for Search {}

const DAY: u64 = 86_400_000 << 22;

pub fn cross_dm(app: &PhoenixApp) -> bool {
    crate::ui::settings::synced_map::get_pick(&app.prefs.synced.proto, "dm-search") == 1
}

pub fn in_dm(app: &PhoenixApp) -> bool {
    app.selected_guild.is_none() && app.selected_channel.is_some()
}

fn dm_peer(app: &PhoenixApp, ch: Id) -> Option<&crate::model::User> {
    let c = app.channels.get(&ch)?;
    if c.guild.is_some() || crate::ui::dm_profile_panel::is_group_dm(app, ch) {
        return None;
    }
    c.voice_users.first().and_then(|u| app.users.get(u))
}

pub fn dm_tag(app: &PhoenixApp, ch: Id) -> SharedString {
    dm_peer(app, ch).map(|u| u.name.clone()).or_else(|| app.channels.get(&ch).map(|c| c.name.clone())).unwrap_or_default()
}

fn dm_label(app: &PhoenixApp, ch: Id) -> SharedString {
    dm_peer(app, ch).map(|u| u.display_name.clone()).or_else(|| app.channels.get(&ch).map(|c| c.name.clone())).unwrap_or_default()
}

fn quote(v: &str) -> String {
    if v.contains(' ') { format!("\"{v}\"") } else { v.to_owned() }
}

pub fn strip_in(q: &str) -> String {
    let mut out = String::new();
    let mut rest = q;
    while let Some(i) = rest.find("in:") {
        let at_word = i == 0 || rest[..i].ends_with(' ');
        if !at_word {
            out.push_str(&rest[..i + 3]);
            rest = &rest[i + 3..];
            continue;
        }
        out.push_str(&rest[..i]);
        let v = rest[i + 3..].trim_start();
        let end = if let Some(stripped) = v.strip_prefix('"') { stripped.find('"').map_or(v.len(), |e| e + 2) } else { v.find(' ').unwrap_or(v.len()) };
        rest = &v[end..];
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn build_request(app: &PhoenixApp, text: &str) -> Result<SearchRequest, String> {
    let p = parse_search(text);
    let all_dms = in_dm(app) && cross_dm(app);
    let mut req = SearchRequest {
        guild: app.selected_guild,
        channel: if app.selected_guild.is_none() && !all_dms { app.selected_channel } else { None },
        all_dms,
        content: (!p.content.trim().is_empty()).then(|| p.content.trim().to_owned()),
        has: p.has.iter().map(|h| h.as_str().to_owned()).collect(),
        pinned: p.pinned,
        author_types: p.author_types.clone(),
        ..Default::default()
    };
    if req.guild.is_none() && req.channel.is_none() && !req.all_dms {
        return Err(crate::t!("search.need_scope").into());
    }
    let find_user = |name: &str| -> Result<Id, String> {
        let n = name.to_lowercase();
        let mut best: Option<(u8, Id)> = None;
        for u in app.users.values() {
            for cand in [u.name.to_lowercase(), u.display_name.to_lowercase()] {
                let s = if cand == n { 0 } else if cand.starts_with(&n) { 1 } else { continue };
                if best.is_none_or(|(b, _)| s < b) {
                    best = Some((s, u.id));
                }
            }
        }
        best.map(|(_, id)| id).ok_or_else(|| crate::t!("search.no_user", name = name).to_string())
    };
    for f in &p.from {
        req.authors.push(find_user(f)?);
    }
    for f in &p.mentions {
        req.mentions.push(find_user(f)?);
    }
    for name in &p.in_channels {
        let n = name.to_lowercase();
        if req.all_dms {
            let ch = app
                .dms
                .iter()
                .copied()
                .find(|d| dm_tag(app, *d).to_lowercase() == n || dm_label(app, *d).to_lowercase() == n)
                .ok_or_else(|| crate::t!("search.no_channel", name = name).to_string())?;
            req.channels.push(ch);
            continue;
        }
        let ch = app
            .guild_channels
            .get(&req.guild.unwrap_or_default())
            .into_iter()
            .flatten()
            .filter_map(|id| app.channels.get(id))
            .find(|c| c.kind != ChannelKind::Category && c.name.to_lowercase() == n)
            .ok_or_else(|| crate::t!("search.no_channel", name = name).to_string())?;
        req.channels.push(ch.id);
    }
    let day = |d: &str| date_to_snowflake(d).map(|s| s.0).ok_or_else(|| crate::t!("search.bad_date", d = d).to_string());
    if let Some(d) = &p.before {
        req.max_id = Some(Id(day(d)?));
    }
    if let Some(d) = &p.after {
        req.min_id = Some(Id(day(d)? + DAY));
    }
    if let Some(d) = &p.during {
        let s = day(d)?;
        req.min_id = Some(Id(s));
        req.max_id = Some(Id(s + DAY));
    }
    let empty = req.content.is_none()
        && req.authors.is_empty()
        && req.mentions.is_empty()
        && req.channels.is_empty()
        && req.has.is_empty()
        && req.min_id.is_none()
        && req.max_id.is_none()
        && req.pinned.is_none()
        && req.author_types.is_empty();
    if empty {
        return Err(String::new());
    }
    Ok(req)
}

pub fn results_shown(cx: &App) -> bool {
    cx.try_global::<Search>().and_then(|s| s.0.as_ref()).is_some_and(|s| s.page.is_some() && s.request.is_some())
}

pub fn is_open(cx: &App) -> bool {
    cx.try_global::<Search>().is_some_and(|s| s.0.is_some())
}

pub fn is_focused(window: &Window, cx: &App) -> bool {
    cx.try_global::<Search>()
        .and_then(|s| s.0.as_ref())
        .is_some_and(|s| gpui::Focusable::focus_handle(s.input.read(cx), cx).is_focused(window))
}

pub fn open(_app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if let Some(s) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()) {
        let input = s.input.clone();
        if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
            s.idle = false;
        }
        input.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
        return;
    }
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("search.placeholder")));
    let sub = cx.subscribe_in(&input, window, |app, _, ev: &InputEvent, _, cx| {
        if let InputEvent::PressEnter { .. } = ev {
            submit(app, cx);
        }
    });
    input.update(cx, |s, cx| s.focus(window, cx));
    cx.set_global(Search(Some(SearchState {
        input,
        request: None,
        page: None,
        loading: false,
        error: None,
        sort_menu: false,
        dm_menu: false,
        seq: 0,
        idle: false,
        _sub: sub,
    })));
    cx.notify();
}

pub fn close(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if cx.has_global::<Search>() {
        cx.global_mut::<Search>().0 = None;
    }
    app.composer.update(cx, |s, cx| s.focus(window, cx));
    cx.notify();
}

fn set_idle(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let empty = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).is_none_or(|s| s.input.read(cx).value().trim().is_empty());
    if empty {
        close(app, window, cx);
        return;
    }
    if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
        s.idle = true;
    }
    app.composer.update(cx, |s, cx| s.focus(window, cx));
    cx.notify();
}

pub fn on_key(app: &mut PhoenixApp, key: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    if key == "escape" && crate::ui::search_filters::is_open(cx) {
        if !crate::ui::search_filters::close_popover(cx) {
            crate::ui::search_filters::close(cx);
        }
        return true;
    }
    if key == "escape" && (close_sort_menu(cx) | close_dm_menu(cx)) {
        return true;
    }
    if key == "escape" {
        close(app, window, cx);
        return true;
    }
    false
}

fn submit(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(s) = cx.global::<Search>().0.as_ref() else { return };
    let text = s.input.read(cx).value().to_string();
    match build_request(app, &text) {
        Ok(req) => {
            let t = text.trim().to_owned();
            let h = &mut cx.default_global::<SearchHistory>().0;
            h.retain(|x| *x != t);
            h.insert(0, t);
            h.truncate(5);
            run(app, req, cx)
        }
        Err(e) => {
            if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
                s.error = (!e.is_empty()).then(|| e.into());
                s.page = None;
                s.request = None;
            }
            cx.notify();
        }
    }
}

pub fn run(app: &mut PhoenixApp, req: SearchRequest, cx: &mut Context<PhoenixApp>) {
    let Some(s) = cx.global_mut::<Search>().0.as_mut() else { return };
    s.seq += 1;
    let seq = s.seq;
    s.loading = true;
    s.error = None;
    s.request = Some(req.clone());
    let fut = app.backend.search_messages(req);
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |_, cx| {
            let Some(s) = cx.global_mut::<Search>().0.as_mut() else { return };
            if s.seq != seq {
                return;
            }
            s.loading = false;
            match res {
                Ok(p) => s.page = Some(p),
                Err(e) => {
                    s.page = None;
                    s.error = Some(e.0.into());
                }
            }
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

fn rerun(app: &mut PhoenixApp, f: impl FnOnce(&mut SearchRequest), cx: &mut Context<PhoenixApp>) {
    let Some(mut req) = cx.global::<Search>().0.as_ref().and_then(|s| s.request.clone()) else { return };
    f(&mut req);
    run(app, req, cx);
}

pub fn set_query(q: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(input) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).map(|s| s.input.clone()) else { return };
    let q = q.to_string();
    input.update(cx, |s, cx| s.set_value(q, window, cx));
}

pub fn append_and_submit(app: &mut PhoenixApp, tokens: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(input) = cx.global::<Search>().0.as_ref().map(|s| s.input.clone()) else { return };
    if !tokens.is_empty() {
        input.update(cx, |s, cx| {
            let v = s.value().trim_end().to_string();
            let sep = if v.is_empty() { "" } else { " " };
            s.set_value(format!("{v}{sep}{tokens}"), window, cx);
        });
    }
    submit(app, cx);
}

const MORE_KEYS: [&str; 5] = ["before:", "after:", "during:", "pinned:", "authorType:"];

fn add_filter(prefix: &'static str, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(s) = cx.global::<Search>().0.as_ref() else { return };
    let input = s.input.clone();
    input.update(cx, |s, cx| {
        let v = s.value().to_string();
        let sep = if v.is_empty() || v.ends_with(' ') { "" } else { " " };
        s.set_value(format!("{v}{sep}{prefix}"), window, cx);
        s.focus(window, cx);
    });
}

const FILTERS: [(&str, &str, &str, &str, &str); 4] = [
    ("from:", "search/user", "search.f_from", "search.k_from", "search.v_user"),
    ("in:", "hash", "search.f_in", "search.k_in", "search.v_channel"),
    ("has:", "search/attachment", "search.f_has", "search.k_has", "search.v_has"),
    ("mentions:", "search/at", "search.f_mentions", "search.k_mentions", "search.v_user"),
];


fn result_card(app: &PhoenixApp, ix: usize, m: &Message, hl: &Highlighter, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let guild = app.channel_guild(m.channel).or(app.selected_guild);
    let author_cow = app.member_user(guild, m.author);
    let author = author_cow.as_deref();
    let channel = m.channel;
    let id = m.id;
    let content: String = m.content.chars().take(400).collect();
    let rich = !content.is_empty();
    let acx: &App = cx;
    let extras: Vec<AnyElement> = m
        .attachments
        .iter()
        .map(|a| crate::ui::chat::attachment(a, acx))
        .chain(m.embeds.iter().map(|e| crate::ui::chat::embed(app, e, acx)))
        .chain(crate::ui::chat::invite_embed::render(app, &cx.entity(), m.author, &m.content, acx))
        .chain(crate::ui::components::render_static(app, &cx.entity(), m, acx))
        .collect();
    let bot = author.is_some_and(|u| u.bot);
    let shown: SharedString = m
        .author_nick
        .clone()
        .or_else(|| app.member_nick_in(guild, m.author))
        .or_else(|| author.map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("common.unknown"));
    let (role_color, role_gradient) = app.author_role_style_in(guild, m.author);
    let reply = m.reply_to.is_some().then(|| reply_line(app, m, &cx.entity()));
    let text: Option<AnyElement> = rich.then(|| crate::ui::components::markdown_highlighted(app, &content, &cx.entity(), m.id, hl));
    div()
        .id(("search-hit", ix))
        .mb(z(8.))
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .relative()
        .group("search-hit")
        .pt(z(6.))
        .pb(z(5.))
        .pl(z(80.))
        .pr(z(20.))
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| a.jump_to_message(channel, id, cx)))
        .children(reply)
        .child(
            div()
                .absolute()
                .left(z(20.))
                .top(z(if m.reply_to.is_some() { 30. } else { 8. }))
                .child(avatar(author, 40.))
                .when_some(crate::ui::avatar_deco::source(author, None), |d, src| d.child(crate::ui::avatar_deco::layer(src, 40. * crate::theme::zoom()))),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .min_h(z(22.))
                .min_w_0()
                .child(
                    crate::fonts::styled_name(
                        div()
                            .max_w_full()
                            .text_size(z(16.))
                            .line_height(z(22.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(role_color.map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG))),
                        author,
                        false,
                    )
                    .id(("search-author", ix))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .on_click(crate::ui::chat::message_ext::open_user(&cx.entity(), m.author, m.channel))
                    .child(crate::fonts::role_name_text(shown, author, 16., guild.is_some(), role_gradient)),
                )
                .when_some(author.and_then(|u| u.tag.clone()), |d, tg| d.child(crate::ui::chat::server_tag_chip(&tg, false)))
                .when_some(app.author_role_icon_in(guild, m.author), |d, u| d.child(crate::ui::chat::role_icon(u)))
                .when(bot, |d| d.child(crate::ui::chat::bot_tag(author.is_some_and(|u| u.verified_bot))))
                .child(
                    div()
                        .flex_none()
                        .ml(z(4.))
                        .text_size(z(12.))
                        .line_height(z(22.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tk(idx::TEXT_MUTED))
                        .child(format_time_full(m.timestamp)),
                ),
        )
        .when_some(text, |d, t| d.child(div().text_size(z(16.)).line_height(z(22.)).text_color(tk(idx::TEXT_DEFAULT)).child(t)))
        .children(extras)
        .child(
            div().absolute().top(z(8.)).right(z(8.)).invisible().group_hover("search-hit", |s| s.visible()).child(
                div()
                    .id(("search-jump", ix))
                    .h(z(24.))
                    .p(z(4.))
                    .rounded(z(3.))
                    .flex()
                    .items_center()
                    .bg(tk(idx::BACKGROUND_BASE_LOWEST))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .hover(|s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
                    .active(|s| s.text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)).mt(z(1.)))
                    .on_click(cx.listener(move |a, _, _, cx| {
                        cx.stop_propagation();
                        a.jump_to_message(channel, id, cx)
                    }))
                    .child(crate::t!("pins.jump")),
            ),
        )
        .into_any_element()
}

fn highlight_plain(text: SharedString, hl: &Highlighter) -> gpui::StyledText {
    let style = gpui::HighlightStyle { background_color: Some(gpui::rgba(crate::search_highlight::HIGHLIGHT_RGBA).into()), ..Default::default() };
    let ranges = hl.ranges(&text);
    gpui::StyledText::new(text).with_highlights(ranges.into_iter().map(|r| (r, style)))
}

fn reply_line(app: &PhoenixApp, m: &Message, entity: &gpui::Entity<PhoenixApp>) -> gpui::Div {
    let spine = div()
        .absolute()
        .left(z(-41.))
        .top(z(9.))
        .w(z(37.))
        .h(z(13.))
        .border_l_2()
        .border_t_2()
        .rounded_tl(z(6.))
        .border_color(tk(idx::SPINE_DEFAULT));
    let row = div().relative().h(z(18.)).mb(z(4.)).flex().items_center().gap(z(4.)).text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_DEFAULT)).child(spine);
    match &m.reply_preview {
        Some((a, nick, text)) => {
            let guild = app.channel_guild(m.channel).or(app.selected_guild);
            let ru_cow = app.member_user(guild, *a);
            let ru = ru_cow.as_deref();
            let name: SharedString = nick.clone().or_else(|| app.member_nick_in(guild, *a)).or_else(|| ru.map(|u| u.display_name.clone())).unwrap_or_else(|| crate::t!("common.unknown"));
            let (reply_color, reply_gradient) = app.author_role_style_in(guild, *a);
            let preview: String = text.chars().take(200).collect::<String>().replace('\n', " ");
            row.child(avatar(ru, 16.))
                .child(
                    crate::fonts::styled_name(
                        div().flex_none().font_weight(FontWeight::MEDIUM).opacity(0.64).text_color(reply_color.map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG))),
                        ru,
                        false,
                    )
                    .id(("search-reply-author", m.id.0))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .on_click(crate::ui::chat::message_ext::open_user(entity, *a, m.channel))
                    .child(crate::fonts::role_name_text(name, ru, 14., guild.is_some(), reply_gradient)),
                )
                .map(|r| {
                    if preview.trim().is_empty() && m.reply_voice {
                        r.child(div().flex_1().min_w_0().truncate().italic().child(crate::t!("chat.reply_click_to_see_voice")))
                    } else {
                        r.child(div().flex_1().min_w_0().truncate().child(preview))
                    }
                })
        }
        None => row
            .child(div().size(z(16.)).flex().items_center().justify_center().child(named("reply", 12., tk(idx::TEXT_DEFAULT))))
            .child(div().italic().truncate().child(crate::t!("search.reply_unloaded"))),
    }
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let s = cx.try_global::<Search>()?.0.as_ref()?;
    let input = s.input.clone();
    let loading = s.loading;
    let error = s.error.clone();
    let page = s.page.clone();
    let req = s.request.clone();
    let sort_menu = s.sort_menu;
    if s.idle && (page.is_none() || req.is_none()) && !gpui::Focusable::focus_handle(input.read(cx), cx).is_focused(window) {
        return None;
    }
    if (page.is_none() || req.is_none())
        && let Some(modal) = crate::ui::search_filters::render(app, window, cx)
    {
        return Some(modal);
    }
    let (Some(page), Some(req)) = (page, req) else {
        return Some(options_popout(app, input, error, loading, cx));
    };
    Some(results_panel(app, input, page, req, error, loading, sort_menu, window, cx))
}

fn close_dm_menu(cx: &mut App) -> bool {
    let Some(s) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()) else { return false };
    if !s.dm_menu {
        return false;
    }
    if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
        s.dm_menu = false;
    }
    cx.refresh_windows();
    true
}

fn close_sort_menu(cx: &mut App) -> bool {
    let open = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).is_some_and(|s| s.sort_menu);
    if open {
        if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
            s.sort_menu = false;
        }
        cx.refresh_windows();
    }
    open
}

fn group_digits(n: u32) -> String {
    let sep = if crate::i18n::locale() == "en" { ',' } else { '\u{202f}' };
    let d = n.to_string();
    let mut out = String::new();
    for (i, ch) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(ch);
    }
    out
}

fn page_items(cur: u32, last: u32) -> Vec<Option<u32>> {
    let mut want: Vec<u32> = vec![1, last, cur.saturating_sub(1).max(1), cur, (cur + 1).min(last)];
    if cur <= 2 {
        want.push(3.min(last));
    }
    if cur + 1 >= last {
        want.push(last.saturating_sub(2).max(1));
    }
    want.sort_unstable();
    want.dedup();
    let mut out = Vec::new();
    let mut prev = 0;
    for p in want {
        if p > prev + 1 {
            out.push(None);
        }
        out.push(Some(p));
        prev = p;
    }
    out
}

fn live_bar(input: &Entity<InputState>, active: bool, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    div()
        .id("search-bar-live")
        .absolute()
        .top(z(layout::TITLE_BAR_H + 8.))
        .right(z(8.))
        .w(z(244.))
        .h(z(32.))
        .rounded(z(8.))
        .bg(c(Palette::BG_PRIMARY))
        .border_1()
        .border_color(tk(if active { idx::INPUT_BORDER_ACTIVE } else { idx::INPUT_BORDER_DEFAULT }))
        .occlude()
        .flex()
        .items_center()
        .child(div().flex_1().min_w_0().px(z(6.)).text_size(z(14.)).font_weight(FontWeight::MEDIUM).child(Input::new(input).appearance(false).bordered(false)))
        .child(
            div()
                .id("search-bar-close")
                .size(z(24.))
                .p(z(4.))
                .flex()
                .items_center()
                .cursor_pointer()
                .on_click(cx.listener(|a, _, w, cx| close(a, w, cx)))
                .child(named("close", 16., tk(idx::ICON_DEFAULT))),
        )
}

fn sm_secondary(id: &'static str, glyph: &str, label: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(z(32.))
        .px(z(12.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_NORMAL)))
        .flex()
        .items_center()
        .gap(z(4.))
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tk(idx::TEXT_STRONG))
        .child(named(glyph, 16., tk(idx::TEXT_STRONG)))
        .child(label)
}

#[allow(clippy::too_many_arguments)]
fn results_panel(
    app: &PhoenixApp,
    input: Entity<InputState>,
    page: SearchPage,
    req: SearchRequest,
    error: Option<SharedString>,
    loading: bool,
    sort_menu: bool,
    window: &mut Window,
    cx: &mut Context<PhoenixApp>,
) -> AnyElement {
    let total = page.total;
    let dm_scope = req.all_dms || (req.guild.is_none() && req.channel.is_some());
    let dm_menu = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).is_some_and(|s| s.dm_menu);
    let title: SharedString = if loading {
        crate::t!("search.searching")
    } else if total == 0 {
        crate::t!("search.no_result_title")
    } else if total == 1 {
        crate::t!("search.result_one")
    } else {
        crate::t!("search.results", n = group_digits(total))
    };
    let header = div()
        .flex_none()
        .h(z(48.))
        .px(z(16.))
        .py(z(8.))
        .flex()
        .items_center()
        .justify_between()
        .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(title))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(sm_secondary("search-results-filters", "inbox/filter", crate::t!("search.filters")).on_click(cx.listener(|a, _, w, cx| {
                    crate::ui::search_filters::open(a, w, cx);
                })))
                .child(sm_secondary("search-results-sort", "sort", crate::t!("search.sort")).on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|_, _, _, cx| {
                        cx.stop_propagation();
                        if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
                            s.sort_menu = !s.sort_menu;
                            s.dm_menu = false;
                        }
                        cx.notify();
                    }),
                ))
                .when(dm_scope, |d| {
                    d.child(icon_secondary("search-results-dm-settings", "gear").on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                            if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
                                s.dm_menu = !s.dm_menu;
                                s.sort_menu = false;
                            }
                            cx.notify();
                        }),
                    ))
                }),
        );
    let results_scroll = crate::ui::chat::scrollbar::handle("search-results");
    let gutter = if f32::from(results_scroll.max_offset().y) > 0. { 8. } else { 0. };
    let mut body = div().flex().flex_col().pt(z(16.)).pl(z(16.)).pr(z(16. + gutter));
    if let Some(e) = error.filter(|_| !loading) {
        body = body.child(empty_state(e, true));
    } else if page.messages.is_empty() && !loading {
        body = body.child(empty_state(crate::t!("search.no_results"), false));
    } else {
        let hl = Highlighter::new(req.content.as_deref().unwrap_or_default(), false);
        let mut i = 0;
        while i < page.messages.len() {
            let ch = page.messages[i].channel;
            let mut group = div().mb(z(24.)).flex().flex_col().child(channel_group_header(app, ch, &hl));
            while i < page.messages.len() && page.messages[i].channel == ch {
                group = group.child(result_card(app, i, &page.messages[i], &hl, cx));
                i += 1;
            }
            body = body.child(group);
        }
    }
    let pages = total.div_ceil(PAGE).max(1);
    let cur = req.offset / PAGE + 1;
    let pager = (pages > 1).then(|| pager(cur, pages, cx));
    let mut panel = div()
        .id("search-popout")
        .absolute()
        .top(z(layout::TITLE_BAR_H + layout::HEADER_H))
        .right_0()
        .bottom_0()
        .w(px(WIDTH))
        .bg(tk(idx::BACKGROUND_BASE_LOWEST))
        .border_l_1()
        .border_color(tk(idx::APP_FRAME_BORDER))
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| {
            close_sort_menu(cx);
            close_dm_menu(cx);
        }))
        .flex()
        .flex_col()
        .child(header)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(div().id("search-scroll").flex_1().min_h_0().overflow_y_scroll().track_scroll(&results_scroll).child(body))
                .children(crate::ui::chat::scrollbar::thin_scrollbar_div(&results_scroll)),
        )
        .children(pager);
    if sort_menu {
        panel = panel.child(sort_popover(req.sort, cx));
    }
    if dm_menu && dm_scope {
        panel = panel.child(dm_popover(req.all_dms, cx));
    }
    let modal = crate::ui::search_filters::render(app, window, cx);
    div().absolute().top_0().left_0().right_0().bottom_0().child(live_bar(&input, false, cx)).child(panel).children(modal).into_any_element()
}

fn channel_group_header(app: &PhoenixApp, ch: Id, hl: &Highlighter) -> gpui::Div {
    let channel = app.channels.get(&ch);
    let private = channel.is_some_and(|c| c.guild.is_none());
    let name: SharedString = match channel {
        Some(_) if private => dm_label(app, ch),
        Some(c) => c.name.clone(),
        None => crate::t!("common.unknown_lower"),
    };
    let glyph = match channel {
        Some(_) if private => crate::icons::path("search/at"),
        Some(c) => crate::ui::sidebar::channel_icon(c.kind).path(),
        None => Icon::Hash.path(),
    };
    let parent = channel.and_then(|c| c.parent).and_then(|p| app.channels.get(&p)).map(|p| p.name.clone());
    div()
        .h(z(22.))
        .mb(z(8.))
        .flex()
        .items_center()
        .child(div().w(z(20.)).flex_none().pr(z(4.)).child(gpui::svg().path(glyph).size(z(16.)).text_color(tk(idx::TEXT_STRONG))))
        .child(div().flex_1().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(header_name(name, hl)))
        .when_some(parent, |d, p| {
            d.child(
                div()
                    .max_w(z(190.))
                    .min_w_0()
                    .flex()
                    .items_center()
                    .child(named("folder", 14., tk(idx::TEXT_DEFAULT)).ml(z(4.)).mr(z(4.)))
                    .child(div().min_w_0().truncate().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_DEFAULT)).child(p)),
            )
        })
}

fn header_name(name: SharedString, hl: &Highlighter) -> AnyElement {
    let emoji: Vec<_> = crate::richtext::emoji_ranges(&name).into_iter().map(|(r, _)| r).filter(|r| !name[r.clone()].starts_with('<')).collect();
    if emoji.is_empty() {
        return highlight_plain(name, hl).into_any_element();
    }
    let size = 22. * crate::theme::zoom();
    let mut row = div().flex().items_center().min_w_0().overflow_hidden().whitespace_nowrap();
    let mut at = 0;
    for r in emoji {
        if r.start > at {
            row = row.child(div().flex_none().child(highlight_plain(name[at..r.start].to_string().into(), hl)));
        }
        row = row.child(div().flex_none().mx(px(1.)).child(crate::media::emoji::emoji_element(&name[r.clone()], size)));
        at = r.end;
    }
    if at < name.len() {
        row = row.child(div().min_w_0().truncate().child(highlight_plain(name[at..].to_string().into(), hl)));
    }
    row.into_any_element()
}

fn empty_state(text: SharedString, error: bool) -> gpui::Div {
    div().min_h(z(600.)).p(z(20.)).flex().items_center().justify_center().child(
        div()
            .w(z(280.))
            .pb(z(40.))
            .flex()
            .flex_col()
            .items_center()
            .gap(z(40.))
            .child(
                div()
                    .size(z(160.))
                    .rounded_full()
                    .border_1()
                    .border_color(tk(idx::BORDER_SUBTLE))
                    .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(named(if error { "warning" } else { "search" }, 72., tk(if error { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_MUTED }))),
            )
            .child(
                div()
                    .text_center()
                    .text_size(z(16.))
                    .line_height(z(24.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(if error { idx::TEXT_FEEDBACK_CRITICAL } else { idx::TEXT_DEFAULT }))
                    .child(text),
            ),
    )
}

fn pager(cur: u32, pages: u32, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let caret = |next: bool| {
        let g = named("chevron-right", 16., tk(idx::TEXT_STRONG));
        if next { g } else { g.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))) }
    };
    let end = |id: &'static str, label: SharedString, next: bool, enabled: bool| {
        div()
            .id(id)
            .h(z(40.))
            .when(next, |d| d.pl(z(12.)).pr(z(8.)))
            .when(!next, |d| d.pl(z(8.)).pr(z(12.)))
            .rounded(z(8.))
            .flex()
            .items_center()
            .gap(z(4.))
            .text_size(z(16.))
            .line_height(z(18.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(tk(idx::TEXT_STRONG))
            .when(!enabled, |d| d.opacity(0.5))
            .when(enabled, |d| d.cursor_pointer().hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE))))
            .when(!next, |d| d.child(caret(false)))
            .child(label)
            .when(next, |d| d.child(caret(true)))
    };
    let mut nav = div().flex().items_center().gap(z(8.));
    let mut prev = end("search-prev", crate::t!("search.prev_page"), false, cur > 1);
    if cur > 1 {
        prev = prev.on_click(cx.listener(move |a, _, _, cx| go_page(a, cur - 1, cx)));
    }
    nav = nav.child(prev);
    for (k, item) in page_items(cur, pages).into_iter().enumerate() {
        nav = nav.child(match item {
            None => div()
                .id(("search-page-gap", k))
                .w(z(28.))
                .flex()
                .justify_center()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_DEFAULT))
                .child("…"),
            Some(p) => {
                let on = p == cur;
                div()
                    .id(("search-page", p as usize))
                    .h(z(28.))
                    .min_w(z(28.))
                    .px(z(6.))
                    .rounded(z(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(z(16.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(on, |d| d.bg(tk(idx::BRAND_500)).text_color(tk(idx::WHITE)))
                    .when(!on, |d| d.text_color(tk(idx::TEXT_STRONG)).cursor_pointer().hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE))).on_click(cx.listener(move |a, _, _, cx| go_page(a, p, cx))))
                    .child(p.to_string())
            }
        });
    }
    let mut next = end("search-next", crate::t!("common.next"), true, cur < pages);
    if cur < pages {
        next = next.on_click(cx.listener(move |a, _, _, cx| go_page(a, cur + 1, cx)));
    }
    nav = nav.child(next);
    div().flex_none().h(z(56.)).px(z(16.)).flex().items_center().justify_center().child(nav)
}

fn go_page(app: &mut PhoenixApp, p: u32, cx: &mut Context<PhoenixApp>) {
    crate::ui::chat::scrollbar::handle("search-results").set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
    rerun(app, |r| r.offset = p.saturating_sub(1) * PAGE, cx);
}

fn icon_secondary(id: &'static str, glyph: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(z(32.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_NORMAL)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(named(glyph, 16., tk(idx::TEXT_STRONG)))
}

pub fn set_cross_dm(app: &mut PhoenixApp, on: bool, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    close_dm_menu(cx);
    if cross_dm(app) == on {
        return;
    }
    crate::ui::settings::save(app, crate::ui::settings::SettingsPatch::Proto("dm-search", u64::from(on)), cx);
    let Some(input) = cx.global::<Search>().0.as_ref().map(|s| s.input.clone()) else { return };
    if !on {
        input.update(cx, |s, cx| {
            let v = strip_in(&s.value());
            s.set_value(v, window, cx);
        });
    }
    submit(app, cx);
}

fn dm_popover(all: bool, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let row = |key: &'static str, on: bool, value: bool, cx: &mut Context<PhoenixApp>| {
        div()
            .id(key)
            .h(z(36.))
            .p(z(8.))
            .rounded(z(2.))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
            .on_click(cx.listener(move |a, _, w, cx| set_cross_dm(a, value, w, cx)))
            .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(crate::t!(key)))
            .child(crate::ui::controls::radio_indicator(on))
    };
    div()
        .id("search-dm-menu")
        .absolute()
        .top(z(47.))
        .right(z(16.))
        .w(z(237.))
        .p(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
        .shadow(crate::ui::inbox::popout_shadow())
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .h(z(36.))
                .p(z(8.))
                .flex()
                .items_center()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_MUTED))
                .child(crate::t!("search.dm_default")),
        )
        .child(row("search.dm_this", !all, false, cx))
        .child(row("search.dm_all", all, true, cx))
}

fn sort_popover(cur: SortOrder, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let items = [(SortOrder::Newest, "search.newest"), (SortOrder::Oldest, "search.sort_date"), (SortOrder::Relevance, "search.relevance")];
    div()
        .id("search-sort-menu")
        .absolute()
        .top(z(47.))
        .right(z(16.))
        .w(z(144.))
        .py(z(8.))
        .pl(z(8.))
        .pr(z(8.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
        .shadow(crate::ui::inbox::popout_shadow())
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .children(items.into_iter().map(|(o, key)| {
            div()
                .id(key)
                .h(z(36.))
                .p(z(8.))
                .rounded(z(2.))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
                .on_click(cx.listener(move |a, _, _, cx| {
                    close_sort_menu(cx);
                    rerun(
                        a,
                        |r| {
                            r.sort = o;
                            r.offset = 0
                        },
                        cx,
                    )
                }))
                .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(crate::t!(key)))
                .child(crate::ui::controls::radio_indicator(o == cur))
        }))
}

fn tk(i: usize) -> gpui::Hsla {
    c(crate::theme::tok(i))
}

fn named(name: &str, size: f32, color: gpui::Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path(name)).size(z(size)).flex_none().text_color(color)
}

#[derive(Debug, PartialEq)]
enum HistorySeg {
    Text(String),
    Filter { key: &'static str, value: String },
}

fn history_segments(q: &str) -> Vec<HistorySeg> {
    let keys: Vec<&'static str> = FILTERS.iter().map(|f| f.0).chain(MORE_KEYS).collect();
    let mut out: Vec<HistorySeg> = Vec::new();
    for word in q.split_whitespace() {
        let lower = word.to_lowercase();
        if let Some(k) = keys.iter().find(|k| lower.starts_with(**k) && word.len() > k.len()) {
            out.push(HistorySeg::Filter { key: k, value: word[k.len()..].to_owned() });
        } else if let Some(HistorySeg::Text(t)) = out.last_mut() {
            t.push(' ');
            t.push_str(word);
        } else {
            out.push(HistorySeg::Text(word.to_owned()));
        }
    }
    out
}

fn history_pills(app: &PhoenixApp, q: &str) -> gpui::Div {
    let mut row = div().flex().items_center().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_STRONG)).whitespace_nowrap().overflow_hidden();
    let pill = || div().py(z(3.)).bg(tk(idx::BACKGROUND_MOD_STRONG));
    for seg in history_segments(q) {
        row = match seg {
            HistorySeg::Text(t) => row.child(div().mr(z(8.)).child(t)),
            HistorySeg::Filter { key, value } => {
                let label: SharedString = FILTERS.iter().find(|f| f.0 == key).map(|f| crate::i18n::t(f.3, &[])).unwrap_or(key.into());
                let user = (key == "from:" || key == "mentions:")
                    .then(|| {
                        let n = value.to_lowercase();
                        app.users.values().find(|u| u.name.to_lowercase() == n || u.display_name.to_lowercase() == n)
                    })
                    .flatten();
                let mut answer = pill().pl(z(2.)).pr(z(6.)).mr(z(8.)).rounded_r(z(4.)).flex().items_center().gap(z(4.));
                if let Some(u) = user {
                    answer = answer.child(crate::ui::avatar(Some(u), 16.));
                }
                row.child(pill().pl(z(6.)).pr(z(2.)).rounded_l(z(4.)).child(label)).child(answer.child(value))
            }
        };
    }
    row
}

fn option_row(id: (&'static str, usize)) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(z(8.))
        .py(z(4.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
}

fn group_header(label: SharedString) -> gpui::Div {
    div()
        .px(z(8.))
        .py(z(4.))
        .flex()
        .items_center()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tk(idx::TEXT_SUBTLE))
        .child(div().flex_1().child(label))
}

fn options_popout(app: &PhoenixApp, input: Entity<InputState>, error: Option<SharedString>, loading: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let history = cx.try_global::<SearchHistory>().map(|h| h.0.clone()).unwrap_or_default();
    let mut filters = div().flex().flex_col().gap(z(4.)).child(group_header(crate::t!("search.filters")));
    let dm = in_dm(app);
    let all_dms = dm && cross_dm(app);
    for (i, (k, glyph, label, key, value)) in FILTERS.into_iter().enumerate() {
        if k == "in:" && dm && !all_dms {
            continue;
        }
        filters = filters.child(
            option_row(("search-filter", i))
                .on_click(cx.listener(move |_, _, w, cx| add_filter(k, w, cx)))
                .child(named(glyph, 20., tk(idx::TEXT_MUTED)).mr(z(8.)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(crate::t!(label)))
                        .child(
                            div()
                                .flex()
                                .gap(z(4.))
                                .text_size(z(14.))
                                .line_height(z(18.))
                                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(tk(idx::TEXT_SUBTLE)).child(crate::t!(key)))
                                .child(div().font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_MUTED)).child(crate::t!(value))),
                        ),
                ),
        );
    }
    filters = filters.child(
        option_row(("search-filter", FILTERS.len()))
            .on_click(cx.listener(|a, _, w, cx| {
                crate::ui::search_filters::open(a, w, cx);
            }))
            .child(named("inbox/filter", 20., tk(idx::TEXT_MUTED)).mr(z(8.)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(div().text_color(tk(idx::TEXT_STRONG)).child(crate::t!("search.f_more")))
                    .child(div().text_color(tk(idx::TEXT_MUTED)).child(crate::t!("search.v_more"))),
            ),
    );
    let mut body = div().flex().flex_col();
    let query = input.read(cx).value().trim().to_string();
    let query_empty = query.is_empty();
    if !query.is_empty() {
        body = body
            .child(
                option_row(("search-for", 0))
                    .on_click(cx.listener(|a, _, _, cx| submit(a, cx)))
                    .child(named("search", 20., tk(idx::TEXT_MUTED)).mr(z(8.)))
                    .child(
                        div()
                            .flex()
                            .gap(z(4.))
                            .text_size(z(14.))
                            .line_height(z(18.))
                            .text_color(tk(idx::TEXT_STRONG))
                            .child(div().font_weight(FontWeight::MEDIUM).child(crate::t!("search.search_for")))
                            .child(div().mr(z(8.)).font_weight(FontWeight::SEMIBOLD).child(SharedString::from(query))),
                    ),
            )
            .child(div().my(z(8.)).mx(z(-8.)).h(z(1.)).bg(tk(idx::BORDER_SUBTLE)));
    }
    if all_dms && query_empty
        && let Some(cur) = app.selected_channel
    {
        let tag = dm_tag(app, cur);
        let token = format!("in:{}", quote(&tag));
        body = body.child(
            option_row(("search-find-in", 0))
                .on_click(cx.listener(move |a, _, w, cx| {
                    let Some(inp) = cx.global::<Search>().0.as_ref().map(|s| s.input.clone()) else { return };
                    let t = token.clone();
                    inp.update(cx, |s, cx| s.set_value(t, w, cx));
                    submit(a, cx);
                }))
                .child(named("search", 20., tk(idx::TEXT_MUTED)).mr(z(8.)))
                .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(crate::t!("search.find_in", name = tag))),
        );
    }
    body = body.child(filters);
    if !history.is_empty() {
        let mut group = div().flex().flex_col().gap(z(4.)).child(
            group_header(crate::t!("search.history")).child(
                div()
                    .id("search-clear-history")
                    .cursor_pointer()
                    .child(named("chat/trash", 18., tk(idx::TEXT_SUBTLE)))
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.default_global::<SearchHistory>().0.clear();
                        cx.notify();
                    })),
            ),
        );
        for (i, q) in history.into_iter().enumerate() {
            let text = q.clone();
            group = group.child(
                option_row(("search-history", i))
                    .on_click(cx.listener(move |a, _, w, cx| {
                        let Some(inp) = cx.global::<Search>().0.as_ref().map(|s| s.input.clone()) else { return };
                        inp.update(cx, |s, cx| s.set_value(text.clone(), w, cx));
                        submit(a, cx);
                    }))
                    .child(named("search", 20., tk(idx::TEXT_MUTED)).mr(z(8.)))
                    .child(history_pills(app, &q)),
            );
        }
        body = body.child(div().my(z(8.)).mx(z(-8.)).h(z(1.)).bg(tk(idx::BORDER_SUBTLE))).child(group);
    }
    if let Some(e) = error {
        body = body.child(div().px(z(8.)).pt(z(8.)).text_size(z(14.)).text_color(tk(idx::TEXT_FEEDBACK_CRITICAL)).child(e));
    }
    if loading {
        body = body.child(div().px(z(8.)).pt(z(8.)).text_size(z(14.)).text_color(tk(idx::TEXT_MUTED)).child(crate::t!("search.searching")));
    }
    let bar = live_bar(&input, true, cx);
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .child(bar)
        .child(
            div()
                .id("search-popout")
                .debug_selector(|| "search-options-popout".into())
                .on_mouse_down_out(cx.listener(|a, ev: &gpui::MouseDownEvent, w, cx| {
                    let bar = gpui::Bounds::new(
                        gpui::point(w.viewport_size().width - z(8. + 244.), z(layout::TITLE_BAR_H + 8.)),
                        gpui::size(z(244.), z(32.)),
                    );
                    if !bar.contains(&ev.position) && !crate::ui::search_filters::is_open(cx) {
                        set_idle(a, w, cx);
                    }
                }))
                .absolute()
                .top(z(layout::TITLE_BAR_H + layout::HEADER_H - 3.))
                .right(z(12.))
                .w(z(356.))
                .px(z(8.))
                .py(z(12.))
                .rounded(z(8.))
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
                .shadow(crate::ui::inbox::popout_shadow())
                .occlude()
                .child(body),
        )
        .into_any_element()
}

#[cfg(test)]
mod history_tests {
    use super::*;


    #[test]
    fn pager_pages() {
        assert_eq!(page_items(1, 164), vec![Some(1), Some(2), Some(3), None, Some(164)]);
        assert_eq!(page_items(2, 164), vec![Some(1), Some(2), Some(3), None, Some(164)]);
        assert_eq!(page_items(10, 164), vec![Some(1), None, Some(9), Some(10), Some(11), None, Some(164)]);
        assert_eq!(page_items(164, 164), vec![Some(1), None, Some(162), Some(163), Some(164)]);
        assert_eq!(page_items(1, 2), vec![Some(1), Some(2)]);
    }

    #[test]
    fn splits_filters_and_text() {
        assert_eq!(
            history_segments("hello world from:kasiopy has:image"),
            vec![
                HistorySeg::Text("hello world".into()),
                HistorySeg::Filter { key: "from:", value: "kasiopy".into() },
                HistorySeg::Filter { key: "has:", value: "image".into() },
            ]
        );
        assert_eq!(history_segments("from:"), vec![HistorySeg::Text("from:".into())]);
    }

    #[test]
    fn strip_in_drops_channel_filters_only() {
        assert_eq!(strip_in("salut in:staar from:bob"), "salut from:bob");
        assert_eq!(strip_in("in:\"HG Team\" merci"), "merci");
        assert_eq!(strip_in("in: staar"), "");
        assert_eq!(strip_in("main:x"), "main:x");
        assert_eq!(quote("a b"), "\"a b\"");
    }
}

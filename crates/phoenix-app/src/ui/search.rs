//! Message search popout (Ctrl+F): `from:`, `mentions:`, `in:`, `has:`,
//! `before:`, `after:`, `during:` and `pinned:` filters, paged results.

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

/// What to search, with names already resolved to ids.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchRequest {
    /// Guild scope; when `None`, `channel` (a DM) is searched.
    pub guild: Option<Id>,
    pub channel: Option<Id>,
    pub content: Option<String>,
    pub authors: Vec<Id>,
    pub mentions: Vec<Id>,
    pub channels: Vec<Id>,
    /// `has:` values ("link", "image"...).
    pub has: Vec<String>,
    pub min_id: Option<Id>,
    pub max_id: Option<Id>,
    pub pinned: Option<bool>,
    /// `authorType:` values (user / bot / webhook).
    pub author_types: Vec<String>,
    pub offset: u32,
    pub sort: SortOrder,
}

/// "Trier" menu of the results header (Plus récents / Date / Pertinence in the client).
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
    /// The "Trier" menu is open.
    sort_menu: bool,
    seq: u64,
    _sub: Subscription,
}

/// Queries run this session, newest first (the popout's "History" group).
#[derive(Default)]
pub struct SearchHistory(pub Vec<String>);

impl gpui::Global for SearchHistory {}

#[derive(Default)]
pub struct Search(pub Option<SearchState>);

impl gpui::Global for Search {}

const DAY: u64 = 86_400_000 << 22;

/// Turn the typed text into a request, resolving `from:`/`in:` names.
pub fn build_request(app: &PhoenixApp, text: &str) -> Result<SearchRequest, String> {
    let p = parse_search(text);
    let mut req = SearchRequest {
        guild: app.selected_guild,
        channel: if app.selected_guild.is_none() { app.selected_channel } else { None },
        content: (!p.content.trim().is_empty()).then(|| p.content.trim().to_owned()),
        has: p.has.iter().map(|h| h.as_str().to_owned()).collect(),
        pinned: p.pinned,
        author_types: p.author_types.clone(),
        ..Default::default()
    };
    if req.guild.is_none() && req.channel.is_none() {
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

/// The results sidebar is shown (not just the options popout): the chat makes
/// room for it instead of the member list / profile panel.
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
        seq: 0,
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

pub fn on_key(app: &mut PhoenixApp, key: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) -> bool {
    if key == "escape" && crate::ui::search_filters::is_open(cx) {
        if !crate::ui::search_filters::close_popover(cx) {
            crate::ui::search_filters::close(cx);
        }
        return true;
    }
    if key == "escape" && close_sort_menu(cx) {
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

/// Put `q` in the search box (real_shots `PHX_OPEN=search:<q>`).
pub fn set_query(q: &str, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    let Some(input) = cx.try_global::<Search>().and_then(|s| s.0.as_ref()).map(|s| s.input.clone()) else { return };
    let q = q.to_string();
    input.update(cx, |s, cx| s.set_value(q, window, cx));
}

/// Append `tokens` (from the filters modal) to the query and run it.
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

/// Keys set by the "Filtres" modal (dates, `pinned:`, `authorType:`).
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

/// "Filters" group of the live popout (dump search-popout `groupWrapper_lm2_KW`):
/// (inserted keyword, icon, label, shown key, shown value).
const FILTERS: [(&str, &str, &str, &str, &str); 4] = [
    ("from:", "search/user", "search.f_from", "search.k_from", "search.v_user"),
    ("in:", "hash", "search.f_in", "search.k_in", "search.v_channel"),
    ("has:", "search/attachment", "search.f_has", "search.k_has", "search.v_has"),
    ("mentions:", "search/at", "search.f_mentions", "search.k_mentions", "search.v_user"),
];


/// One hit (`searchResult_`): 1px --border-subtle, radius 8, --background-base-lowest,
/// margin-bottom 8, the message laid out cozy (padding 8 20 8 80: 40px avatar at 20,
/// name 16/22 medium in the author colour + tag / role icon / APP, 12px medium
/// --text-muted time 4px after, content 16/22 --text-default). Hover shows "Y aller"
/// (`buttonsContainer_` top 8 right 8: 24px, padding 4, radius 3, body-xs/medium
/// --text-default on --background-base-lowest), which jumps to the message.
fn result_card(app: &PhoenixApp, ix: usize, m: &Message, hl: &Highlighter, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let guild = app.channel_guild(m.channel).or(app.selected_guild);
    let author_cow = app.member_user(guild, m.author);
    let author = author_cow.as_deref();
    let channel = m.channel;
    let id = m.id;
    let content: String = m.content.chars().take(400).collect();
    let rich = !content.is_empty();
    // Attachments and embeds like the chat (bot results are often embed-only).
    let acx: &App = cx;
    let extras: Vec<AnyElement> = m
        .attachments
        .iter()
        .map(|a| crate::ui::chat::attachment(a, acx))
        .chain(m.embeds.iter().map(|e| crate::ui::chat::embed(app, e, acx)))
        // Bot components (Components V2 messages have nothing else), drawn without
        // interactions and without highlights, like the client's search results.
        .chain(crate::ui::components::render_static(app, &cx.entity(), m, acx))
        .collect();
    let bot = author.is_some_and(|u| u.bot);
    // The guild nickname at send time, like the chat ("Loris (#1 Roxas lover)").
    let shown: SharedString = m
        .author_nick
        .clone()
        .or_else(|| app.member_nick_in(guild, m.author))
        .or_else(|| author.map(|u| u.display_name.clone()))
        .unwrap_or_else(|| crate::t!("common.unknown"));
    let role_gradient = app.author_role_gradient_in(guild, m.author);
    let reply = m.reply_to.is_some().then(|| reply_line(app, m));
    // Mentions, emoji and markdown like in the chat, with the searched words highlighted.
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
        .pt(z(8.))
        .pb(z(8.))
        .pl(z(80.))
        .pr(z(20.))
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| a.jump_to_message(channel, id, cx)))
        .children(reply)
        .child(div().absolute().left(z(20.)).top(z(if m.reply_to.is_some() { 32. } else { 10. })).child(avatar(author, 40.)))
        .child(
            div()
                .flex()
                .items_center()
                .h(z(22.))
                .min_w_0()
                .child(
                    crate::fonts::styled_name(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(z(16.))
                            .line_height(z(22.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(app.author_color_in(guild, m.author).map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG))),
                        author,
                        false,
                    )
                    .child(crate::fonts::name_text_with(shown, author, 16., role_gradient)),
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

/// `text` with the searched words behind `span.highlight`: the client highlights the
/// channel name of each result group too (live dump: "règlement" lit "r" + "glement"
/// in `channelNameText_`).
fn highlight_plain(text: SharedString, hl: &Highlighter) -> gpui::StyledText {
    let style = gpui::HighlightStyle { background_color: Some(gpui::rgba(crate::search_highlight::HIGHLIGHT_RGBA).into()), ..Default::default() };
    let ranges = hl.ranges(&text);
    gpui::StyledText::new(text).with_highlights(ranges.into_iter().map(|r| (r, style)))
}

/// Reply line over a hit (`repliedMessage_ messageSpine_`, 18px, margin-bottom 4): the
/// spine from the avatar, then 16px avatar + name (64%) + preview, or the italic
/// "could not be loaded" placeholder with the reply glyph when the reply has no preview.
fn reply_line(app: &PhoenixApp, m: &Message) -> gpui::Div {
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
            let preview: String = text.chars().take(200).collect::<String>().replace('\n', " ");
            row.child(avatar(ru, 16.))
                .child(
                    crate::fonts::styled_name(
                        div().flex_none().font_weight(FontWeight::MEDIUM).opacity(0.64).text_color(app.author_color_in(guild, *a).map(c).unwrap_or_else(|| tk(idx::TEXT_STRONG))),
                        ru,
                        false,
                    )
                    .child(match app.author_role_gradient_in(guild, *a) {
                        Some(stops) => crate::fonts::name_text_with(name, ru, 14., Some(stops)),
                        None => crate::fonts::name_text(name, ru, 14.),
                    }),
                )
                .child(div().flex_1().min_w_0().truncate().child(preview))
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
    // The client hides the search popout while the "Plus de filtres" modal is up (the
    // header bar goes back to idle) and shows it again when the modal closes.
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

/// Close the "Trier" menu; false when it was not open.
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

/// "4 083" / "4,083": digit groups with the locale's separator.
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

/// Page numbers of the pager: the first, the last and the ones around the current
/// page (three at either end), `None` for an ellipsis (1 2 3 … 164 / 1 2 3 … 164).
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

/// The header search bar while a search is shown: the query with its clear (x) button.
fn live_bar(input: &Entity<InputState>, active: bool, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    // The header `searchBar_` (244×32, radius 8, --input-background-default,
    // 1px --input-border-default; editor padding 0 6, 14/22 medium) as a live input.
    div()
        .id("search-bar-live")
        .absolute()
        .top(z(layout::TITLE_BAR_H + 8.))
        .right(z(8.))
        .w(z(244.))
        .h(z(32.))
        .rounded(z(8.))
        // The input token is translucent: lay it over the header background.
        .bg(c(Palette::BG_PRIMARY))
        .border_1()
        // Typing (popout open): --input-border-active; results shown: the idle border.
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

/// `button_ sm secondary hasText`: 32px, 1px --border-subtle-ish, radius 8,
/// --background-mod-subtle; padding 3 11, 16px glyph + 14/18 medium, gap 4.
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

/// Results panel (live CDP 2026-11-24, `searchResultsWrap_`): a 48px `searchHeader_`
/// (padding 8 16: the total, body-md/medium --text-strong, then "Filtres" and "Trier"
/// sm secondary buttons, gap 8), the scroller (padding 16 16 0) with the hits grouped by
/// channel, and the 56px `paginationDock_`.
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
                        // Before the panel's own mouse down (which closes the menu).
                        cx.stop_propagation();
                        if let Some(s) = cx.global_mut::<Search>().0.as_mut() {
                            s.sort_menu = !s.sort_menu;
                        }
                        cx.notify();
                    }),
                )),
        );
    // Body: hits grouped by channel (`searchResultGroup_`, margin-bottom 24), the empty
    // state or the error.
    let mut body = div().flex().flex_col().pt(z(16.)).px(z(16.));
    if let Some(e) = error.filter(|_| !loading) {
        body = body.child(empty_state(e, true));
    } else if page.messages.is_empty() && !loading {
        body = body.child(empty_state(crate::t!("search.no_results"), false));
    } else {
        // The client's search highlighter (`wG(query)`), and `th(query)` for the channel names.
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
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .border_l_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| {
            close_sort_menu(cx);
        }))
        .flex()
        .flex_col()
        .child(header)
        .child(div().id("search-scroll").flex_1().min_h_0().overflow_y_scroll().child(body))
        .children(pager);
    if sort_menu {
        panel = panel.child(sort_popover(req.sort, cx));
    }
    let modal = crate::ui::search_filters::render(app, window, cx);
    div().absolute().top_0().left_0().right_0().bottom_0().child(live_bar(&input, false, cx)).child(panel).children(modal).into_any_element()
}

/// `channelNameContainer_` (22px, margin-bottom 8): 16px channel glyph (padding-end 4),
/// name body-md/medium --text-strong; on the right `parentChannelName_` (folder glyph
/// 16×12 + category body-xs/medium --text-default, one line).
fn channel_group_header(app: &PhoenixApp, ch: Id, hl: &Highlighter) -> gpui::Div {
    let channel = app.channels.get(&ch);
    let name: SharedString = channel.map(|c| c.name.clone()).unwrap_or_else(|| crate::t!("common.unknown_lower"));
    let glyph = channel.map(|c| crate::ui::sidebar::channel_icon(c.kind)).unwrap_or(Icon::Hash);
    let parent = channel.and_then(|c| c.parent).and_then(|p| app.channels.get(&p)).map(|p| p.name.clone());
    div()
        .h(z(22.))
        .mb(z(8.))
        .flex()
        .items_center()
        .child(div().w(z(20.)).flex_none().pr(z(4.)).child(gpui::svg().path(glyph.path()).size(z(16.)).text_color(tk(idx::TEXT_STRONG))))
        .child(div().flex_1().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(tk(idx::TEXT_STRONG)).child(highlight_plain(name, hl)))
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

/// `emptyResultsWrap_` (padding 20) > `emptyResultsContent_` (280 wide, centred,
/// padding-bottom 40): a 160px illustration (ours: a magnifier in a ring) and the text
/// 16/24 medium --text-default, centred.
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

/// `paginationDock_` (56px, padding 0 16) > `pageControl_` (padding 4 0): 40px
/// "Précédent" / "Suivant" blank buttons (16px caret, 16/18 semibold, radius 8, padding
/// 0 12 0 8 / 0 8 0 12), 28px round page buttons (radius 14, 16 semibold; the current one
/// --brand-500 with white text) 8px apart and "…" (14/18 semibold).
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

/// Show page `p` (1-based) of the current search.
fn go_page(app: &mut PhoenixApp, p: u32, cx: &mut Context<PhoenixApp>) {
    rerun(app, |r| r.offset = p.saturating_sub(1) * PAGE, cx);
}

/// "Trier" menu (`menu_ flexible_`): 144 wide under the button, right-aligned, 1px
/// --border-subtle, radius 8, --background-surface-higher; scroller padding 8 0 8 8;
/// 36px `menuitemradio` rows (padding 8, radius 2): label body-sm/medium --text-strong and
/// a 20px radio indicator on the right.
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

/// `itemContainer_lm2_KW` > `item_`: radius 8, padding 4 8; 20px `itemIcon_`
/// (--text-muted, margin-end 8); hover --interactive-background-hover.
/// A history query split into free text and `key:value` filters.
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

/// History row content (search-popout #75 `autocompletePillContainer_lm2_KW`):
/// free text = `nonText_` body-sm/semibold --text-strong, margin-end 8;
/// filter = `filterPill_` (padding 3 2 3 6, radius --radius-xs 0 0 --radius-xs)
/// plus `answerPill_` (padding 3 6 3 2, radius 0 --radius-xs --radius-xs 0,
/// gap 4, margin-end 8), both --background-mod-strong; user answers carry a
/// 16px avatar.
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

/// `groupHeader_lm2_KW`: padding 4 8, body-xs/semibold 12/16 --text-subtle.
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

/// The search options popout (dump search-popout): the header search bar turns
/// into the input, and a 356px dropdown opens under it (right edge 12px from
/// the window, top = header bottom − 4): `container_cbVzxY` padding 12 8,
/// radius 8, 1px --border-subtle, --background-surface-high, --shadow-high.
/// Groups (`groupWrapper_lm2_KW`, gap 4): "Filters" (44px rows: label
/// body-sm/medium --text-strong, then key body-sm/semibold --text-subtle + value
/// body-sm/medium --text-muted, gap 4), a `divider_` (margin 8 −8, 1px
/// --border-subtle), "History" (28px rows: search glyph + query
/// body-sm/semibold --text-strong; 18px clear-history trash).
fn options_popout(app: &PhoenixApp, input: Entity<InputState>, error: Option<SharedString>, loading: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let history = cx.try_global::<SearchHistory>().map(|h| h.0.clone()).unwrap_or_default();
    let mut filters = div().flex().flex_col().gap(z(4.)).child(group_header(crate::t!("search.filters")));
    for (i, (k, glyph, label, key, value)) in FILTERS.into_iter().enumerate() {
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
            // The client opens the "Filtres" modal (live dump search-more-filters).
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
    // Typed query: a 28px "Search for: <query>" row (dump search-popout-vanilla
    // #3369–#3375 `itemContainer_` > `labelWithElements_` body-sm/medium gap 4,
    // query `nonText_` body-sm/semibold --text-strong), then a `divider_` (#3376).
    let query = input.read(cx).value().trim().to_string();
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
                // Live client: the options popout closes when the search bar loses
                // focus; its layer lets the press through (no `trapClicks_`).
                .on_mouse_down_out(cx.listener(|a, ev: &gpui::MouseDownEvent, w, cx| {
                    let bar = gpui::Bounds::new(
                        gpui::point(w.viewport_size().width - z(8. + 244.), z(layout::TITLE_BAR_H + 8.)),
                        gpui::size(z(244.), z(32.)),
                    );
                    // A filter popover (date picker, combobox) draws outside the popout.
                    if !bar.contains(&ev.position) && !crate::ui::search_filters::is_open(cx) {
                        close(a, w, cx);
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
        // Live client: page 1 and 2 of 164 show "1 2 3 … 164".
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
}

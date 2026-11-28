use std::cell::Cell;
use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, App, Bounds, Context, Entity, FontWeight, ListAlignment, ListOffset, ListState, ParentElement, Pixels, SharedString,
    Styled, Subscription, Window, div, img, list, prelude::*, px,
};
use gpui_component::input::{InputEvent, InputState};

use super::{Pickers, idx, search_box, tk, with_app};
use crate::{
    app::PhoenixApp,
    model::{Id, ImageRef, SoundboardSound},
    ui::tooltip::Tipped as _,
};

pub const W: f32 = 531.;
pub const H: f32 = 520.;
pub const RAIL_W: f32 = 48.;
pub const HEADER_H: f32 = 64.;
pub const BUTTON_W: f32 = 148.;
pub const BUTTON_H: f32 = 40.;
pub const PER_ROW: usize = 3;
pub const GAP_ABOVE_BUTTON: f32 = 16.;

thread_local! {
    static ANCHOR: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    static PANEL_ANCHOR: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    static FROM_PANEL: Cell<bool> = const { Cell::new(false) };
    static BADGE_EXP: Cell<Option<bool>> = const { Cell::new(None) };
    static BADGE_FETCHING: Cell<bool> = const { Cell::new(false) };
    static BADGE_REQUESTED: std::cell::RefCell<HashSet<Id>> = std::cell::RefCell::new(HashSet::new());
}

pub fn is_new_sound(s: &SoundboardSound, seen: u64, me: Id) -> bool {
    s.available && s.user != Some(me) && s.id.0 > seen
}

pub fn set_badge_experiment(list: &[(u32, i64)]) {
    let on = phoenix_api::rest::experiments::bucket(list, phoenix_api::rest::experiments::SOUNDBOARD_NEW_SOUNDS_BADGE) == 1;
    BADGE_EXP.with(|e| e.set(Some(on)));
}

fn badge_experiment(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    if let Some(on) = BADGE_EXP.with(Cell::get) {
        return on;
    }
    if !BADGE_FETCHING.with(|f| f.replace(true)) {
        let fut = app.backend.user_experiments();
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |_, cx| {
                match res {
                    Ok(list) => set_badge_experiment(&list),
                    Err(_) => BADGE_EXP.with(|e| e.set(Some(false))),
                }
                cx.notify();
            });
        })
        .detach();
    }
    false
}

pub fn has_new_sounds(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> bool {
    let Some(g) = voice_guild(app) else { return false };
    if !badge_experiment(app, cx) {
        return false;
    }
    let Some(seen) = app.nav.sounds_seen.get(&g).copied() else { return false };
    match cx.global::<Pickers>().soundboard.guild_sounds.get(&g) {
        Some(sounds) => sounds.iter().any(|s| is_new_sound(s, seen, app.me.id)),
        None => {
            if BADGE_REQUESTED.with(|r| r.borrow_mut().insert(g)) {
                app.backend.request_soundboard_sounds(vec![g]);
            }
            false
        }
    }
}

pub fn anchor_probe() -> impl IntoElement {
    gpui::canvas(|b, _, _| ANCHOR.set(Some(b)), |_, _, _, _| {}).absolute().top_0().left_0().size_full()
}

pub fn panel_anchor_probe() -> impl IntoElement {
    gpui::canvas(|b, _, _| PANEL_ANCHOR.set(Some(b)), |_, _, _, _| {}).absolute().top_0().left_0().size_full()
}

fn current_anchor() -> Option<Bounds<Pixels>> {
    if FROM_PANEL.get() { PANEL_ANCHOR.get().or(ANCHOR.get()) } else { ANCHOR.get().or(PANEL_ANCHOR.get()) }
}

pub fn toggle_from_panel(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    FROM_PANEL.set(true);
    toggle(app, window, cx);
}

pub fn toggle_from_call(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    FROM_PANEL.set(false);
    toggle(app, window, cx);
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Top(SharedString),
    Favorites,
    Frequent,
    Guild(Id),
    Defaults,
    Search,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub kind: Kind,
    pub key: String,
    pub sounds: Vec<SoundboardSound>,
    pub locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Header(usize),
    Sounds { section: usize, start: usize },
}

pub struct SoundboardState {
    pub open: bool,
    pub defaults: Option<Vec<SoundboardSound>>,
    pub loading: bool,
    pub guild_sounds: HashMap<Id, Vec<SoundboardSound>>,
    pub requested_guilds: bool,
    pub top: HashMap<Id, Vec<Id>>,
    pub top_requested: HashSet<Id>,
    pub collapsed: HashSet<String>,
    pub sections: Vec<Section>,
    pub rows: Vec<Row>,
    pub list: ListState,
    pub input: Option<Entity<InputState>>,
    pub query: String,
    sub: Option<Subscription>,
}

impl Default for SoundboardState {
    fn default() -> Self {
        Self {
            open: false,
            defaults: None,
            loading: false,
            guild_sounds: HashMap::new(),
            requested_guilds: false,
            top: HashMap::new(),
            top_requested: HashSet::new(),
            collapsed: HashSet::new(),
            sections: Vec::new(),
            rows: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(400.)),
            input: None,
            query: String::new(),
            sub: None,
        }
    }
}

pub fn is_open(cx: &App) -> bool {
    cx.try_global::<Pickers>().is_some_and(|p| p.soundboard.open)
}

pub fn disabled(app: &PhoenixApp) -> bool {
    disabled_reason(app).is_some()
}

pub fn disabled_reason(app: &PhoenixApp) -> Option<SharedString> {
    let f = crate::ui::voice_panel::flags(app.me.id);
    if app.deafened || f.server_deaf {
        Some(crate::t!("soundboard.disabled_deaf"))
    } else if f.server_mute {
        Some(crate::t!("soundboard.disabled_server_mute"))
    } else if f.suppress {
        Some(crate::t!("soundboard.disabled_suppressed"))
    } else {
        None
    }
}

pub fn toggle(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) {
    if is_open(cx) {
        close(cx);
        cx.notify();
        return;
    }
    if app.voice_channel.is_none() || disabled(app) {
        return;
    }
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("soundboard.search")));
    let sub = cx.subscribe_in(&input, window, |app, _, ev: &InputEvent, _, cx| {
        if matches!(ev, InputEvent::Change) {
            rebuild(app, cx);
            cx.notify();
        }
    });
    input.update(cx, |s, cx| s.focus(window, cx));
    {
        let st = &mut cx.global_mut::<Pickers>().soundboard;
        st.open = true;
        st.input = Some(input);
        st.sub = Some(sub);
        st.query.clear();
    }
    ensure_loaded(app, cx);
    super::load_frecency(app, cx);
    rebuild(app, cx);
    cx.notify();
}

pub fn close(cx: &mut App) {
    let st = &mut cx.global_mut::<Pickers>().soundboard;
    st.open = false;
    st.input = None;
    st.sub = None;
}

pub fn on_sounds(app: &PhoenixApp, guild: Id, sounds: Vec<SoundboardSound>, cx: &mut Context<PhoenixApp>) {
    cx.global_mut::<Pickers>().soundboard.guild_sounds.insert(guild, sounds);
    refresh(app, cx);
    cx.notify();
}

pub fn refresh(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if is_open(cx) {
        rebuild(app, cx);
    }
}

fn voice_guild(app: &PhoenixApp) -> Option<Id> {
    app.voice_channel.and_then(|c| app.channels.get(&c)).and_then(|c| c.guild)
}

pub(crate) fn ensure_loaded(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let vg = voice_guild(app);
    let (need_defaults, need_guilds, need_top) = {
        let st = &mut cx.global_mut::<Pickers>().soundboard;
        let nd = st.defaults.is_none() && !st.loading;
        if nd {
            st.loading = true;
        }
        let ng = !st.requested_guilds;
        st.requested_guilds = true;
        let nt = vg.filter(|g| st.top_requested.insert(*g));
        (nd, ng, nt)
    };
    if need_defaults {
        let fut = app.backend.soundboard_default_sounds();
        cx.spawn(async move |this, cx| {
            let res = fut.await.unwrap_or_default();
            let _ = this.update(cx, |app, cx| {
                let st = &mut cx.global_mut::<Pickers>().soundboard;
                st.defaults = Some(res);
                st.loading = false;
                refresh(app, cx);
                cx.notify();
            });
        })
        .detach();
    }
    if need_guilds {
        let guild_ids: Vec<Id> = app.guilds.iter().map(|g| g.id).collect();
        if !guild_ids.is_empty() {
            app.backend.request_soundboard_sounds(guild_ids);
        }
    }
    if let Some(g) = need_top {
        let fut = app.backend.top_soundboard_sounds(g);
        cx.spawn(async move |this, cx| {
            let res = fut.await.unwrap_or_default();
            let _ = this.update(cx, |app, cx| {
                cx.global_mut::<Pickers>().soundboard.top.insert(g, res);
                refresh(app, cx);
                cx.notify();
            });
        })
        .detach();
    }
}

pub struct Inputs<'a> {
    pub guild_sounds: &'a HashMap<Id, Vec<SoundboardSound>>,
    pub defaults: &'a [SoundboardSound],
    pub top: &'a [Id],
    pub favorites: &'a [u64],
    pub played: &'a [u64],
    pub nitro: bool,
    pub query: &'a str,
}

pub fn build_sections(app: &PhoenixApp, i: &Inputs) -> Vec<Section> {
    let Inputs { guild_sounds, defaults, top, favorites, played, nitro, query } = *i;
    let vg = voice_guild(app);
    let mut known: HashMap<Id, SoundboardSound> = HashMap::new();
    for g in &app.guilds {
        for s in guild_sounds.get(&g.id).into_iter().flatten() {
            known.insert(s.id, s.clone());
        }
    }
    for s in defaults {
        known.entry(s.id).or_insert_with(|| s.clone());
    }
    let q = query.trim().to_lowercase();
    if !q.is_empty() {
        let mut hits: Vec<SoundboardSound> = Vec::new();
        let mut seen = HashSet::new();
        let order = app.guilds.iter().flat_map(|g| guild_sounds.get(&g.id).into_iter().flatten()).chain(defaults.iter());
        for s in order {
            if s.name.as_ref().is_some_and(|n| n.to_lowercase().contains(&q)) && seen.insert(s.id) {
                hits.push(s.clone());
            }
        }
        return vec![Section { kind: Kind::Search, key: "search".into(), sounds: hits, locked: false }];
    }
    let mut out = Vec::new();
    if let Some(g) = vg {
        let mine = guild_sounds.get(&g).cloned().unwrap_or_default();
        let picks: Vec<SoundboardSound> = top.iter().take(3).filter_map(|id| mine.iter().find(|s| s.id == *id).cloned()).collect();
        if !picks.is_empty() {
            let name = app.guilds.iter().find(|x| x.id == g).map(|x| x.name.clone()).unwrap_or_default();
            out.push(Section { kind: Kind::Top(name), key: "top".into(), sounds: picks, locked: false });
        }
    }
    let favs: Vec<SoundboardSound> = favorites.iter().filter_map(|id| known.get(&Id(*id)).cloned()).collect();
    if !favs.is_empty() {
        out.push(Section { kind: Kind::Favorites, key: "favorites".into(), sounds: favs, locked: false });
    }
    let fav_set: HashSet<u64> = favorites.iter().copied().collect();
    let freq: Vec<SoundboardSound> =
        played.iter().filter(|id| !fav_set.contains(id)).take(3).filter_map(|id| known.get(&Id(*id)).cloned()).collect();
    if !freq.is_empty() {
        out.push(Section { kind: Kind::Frequent, key: "frequent".into(), sounds: freq, locked: false });
    }
    if let Some(g) = vg
        && let Some(list) = guild_sounds.get(&g).filter(|l| !l.is_empty())
    {
        out.push(Section { kind: Kind::Guild(g), key: g.0.to_string(), sounds: list.clone(), locked: false });
    }
    let default_section = (!defaults.is_empty()).then(|| Section { kind: Kind::Defaults, key: "0".into(), sounds: defaults.to_vec(), locked: false });
    if !nitro && let Some(d) = default_section.clone() {
        out.push(d);
    }
    for g in &app.guilds {
        if Some(g.id) == vg {
            continue;
        }
        if let Some(list) = guild_sounds.get(&g.id).filter(|l| !l.is_empty()) {
            out.push(Section { kind: Kind::Guild(g.id), key: g.id.0.to_string(), sounds: list.clone(), locked: !nitro });
        }
    }
    if nitro && let Some(d) = default_section {
        out.push(d);
    }
    if !nitro {
        for s in &mut out {
            if matches!(s.kind, Kind::Favorites | Kind::Frequent) {
                s.locked = s.sounds.iter().any(|x| x.guild.is_some_and(|g| g.0 != 0 && Some(g) != vg));
            }
        }
    }
    out
}

pub fn build_rows(sections: &[Section], collapsed: &HashSet<String>) -> Vec<Row> {
    let mut rows = Vec::new();
    for (i, s) in sections.iter().enumerate() {
        if !matches!(s.kind, Kind::Search) {
            rows.push(Row::Header(i));
        }
        if collapsed.contains(&s.key) {
            continue;
        }
        let mut start = 0;
        while start < s.sounds.len() {
            rows.push(Row::Sounds { section: i, start });
            start += PER_ROW;
        }
    }
    rows
}

pub fn rebuild(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let nitro = app.backend.premium_tier2();
    let (favorites, played) = cx
        .global::<Pickers>()
        .frecency
        .as_ref()
        .map(|f| (f.favorite_sounds.clone(), f.played_sounds.clone()))
        .unwrap_or_default();
    let query = cx.global::<Pickers>().soundboard.input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
    let st = &cx.global::<Pickers>().soundboard;
    let top = voice_guild(app).and_then(|g| st.top.get(&g).cloned()).unwrap_or_default();
    let defaults = st.defaults.clone().unwrap_or_default();
    let sections = build_sections(
        app,
        &Inputs { guild_sounds: &st.guild_sounds, defaults: &defaults, top: &top, favorites: &favorites, played: &played, nitro, query: &query },
    );
    let rows = build_rows(&sections, &st.collapsed);
    let st = &mut cx.global_mut::<Pickers>().soundboard;
    let changed_query = st.query != query;
    st.query = query;
    st.sections = sections;
    if st.rows != rows || changed_query {
        st.rows = rows;
        st.list.reset(st.rows.len());
    }
}

fn active_section(st: &SoundboardState) -> usize {
    let top = st.list.logical_scroll_top().item_ix;
    match st.rows.get(top) {
        Some(Row::Header(s)) | Some(Row::Sounds { section: s, .. }) => *s,
        None => 0,
    }
}

fn guild_of(app: &PhoenixApp, g: Id) -> Option<&crate::model::Guild> {
    app.guilds.iter().find(|x| x.id == g)
}

fn guild_icon(app: &PhoenixApp, g: Id, size: f32) -> AnyElement {
    match guild_of(app, g) {
        Some(guild) => match &guild.icon {
            ImageRef::Url(u) => img(u.clone()).size(px(size)).rounded(px(size / 4.)).into_any_element(),
            _ => div()
                .size(px(size))
                .rounded(px(size / 4.))
                .bg(tk(idx::BACKGROUND_BASE_LOW))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(size * 0.375))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_DEFAULT))
                .child(guild.acronym.clone())
                .into_any_element(),
        },
        None => div().size(px(size)).into_any_element(),
    }
}

fn named(name: &str, size: f32, color: gpui::Hsla) -> AnyElement {
    gpui::svg().path(crate::icons::path(name)).size(px(size)).flex_none().text_color(color).into_any_element()
}

fn section_title(app: &PhoenixApp, kind: &Kind) -> SharedString {
    match kind {
        Kind::Top(name) => crate::t!("soundboard.top", guild = name),
        Kind::Favorites => crate::t!("soundboard.favorites"),
        Kind::Frequent => crate::t!("soundboard.frequent"),
        Kind::Guild(g) => guild_of(app, *g).map(|g| g.name.clone()).unwrap_or_default(),
        Kind::Defaults => crate::t!("soundboard.defaults"),
        Kind::Search => crate::t!("soundboard.results"),
    }
}

fn section_icon(app: &PhoenixApp, kind: &Kind, size: f32, color: gpui::Hsla) -> AnyElement {
    match kind {
        Kind::Top(_) => named("picker/trophy", size, color),
        Kind::Favorites => named("picker/star", size, color),
        Kind::Frequent => named("picker/clock", size, color),
        Kind::Guild(g) => guild_icon(app, *g, size),
        Kind::Defaults | Kind::Search => named("discord", size, color),
    }
}

fn header_row(app: &PhoenixApp, si: usize, s: &Section, collapsed: bool) -> AnyElement {
    let fg = tk(idx::INTERACTIVE_TEXT_DEFAULT);
    let key = s.key.clone();
    div()
        .id(("sb-header", si))
        .h(px(32.))
        .flex_none()
        .pl(px(8.))
        .pr(px(4.))
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .flex()
        .items_center()
        .child(
            div()
                .id(("sb-header-click", si))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_color(fg)
                .hover(|d| d.text_color(tk(idx::INTERACTIVE_TEXT_ACTIVE)))
                .child(section_icon(app, &s.kind, 16., fg))
                .child(
                    div()
                        .mx(px(4.))
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(14.))
                        .line_height(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(section_title(app, &s.kind)),
                )
                .child(named(if collapsed { "chevron-right" } else { "chevron" }, 16., fg))
                .on_click(move |_, w, cx| {
                    let key = key.clone();
                    with_app(cx, w, move |app, _, cx| {
                        let st = &mut cx.global_mut::<Pickers>().soundboard;
                        if !st.collapsed.remove(&key) {
                            st.collapsed.insert(key);
                        }
                        rebuild(app, cx);
                        cx.notify();
                    });
                }),
        )
        .into_any_element()
}

fn emoji_of(s: &SoundboardSound) -> Option<AnyElement> {
    if let Some(id) = s.emoji_id {
        return Some(img(SharedString::from(format!("https://cdn.discordapp.com/emojis/{}.webp?size=40", id.0))).size(px(20.)).flex_none().into_any_element());
    }
    s.emoji_name.as_ref().filter(|n| !n.is_empty()).map(|n| crate::media::emoji::emoji_element(n, 20.))
}

const OVERLAY_FADE_MS: u64 = 200;

pub fn sound_button(app: &PhoenixApp, s: &SoundboardSound, locked: bool, key: (usize, usize)) -> AnyElement {
    let playing = crate::voice::soundboard::is_sound_playing(s.id);
    let label = s.name.clone().unwrap_or_default();
    let emoji = emoji_of(s);
    let has_emoji = emoji.is_some();
    let sound = s.clone();
    let preview = s.clone();
    let can = !locked && crate::voice::soundboard::can_play(app, s);
    let info = div()
        .flex_1()
        .min_w_0()
        .h_full()
        .p(px(8.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .overflow_hidden()
        .children(emoji)
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .line_clamp(2)
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(idx::TEXT_STRONG))
                .when(!has_emoji, |d| d.text_center())
                .child(label.clone()),
        );
    let overlay_bg = || div().absolute().inset_0().bg(tk(idx::BACKGROUND_SURFACE_HIGHEST)).opacity(0.8);
    let secondary = |id: &'static str, icon: &str, tip: SharedString| {
        div()
            .id(SharedString::from(format!("{id}-{}-{}", key.0, key.1)))
            .flex()
            .cursor_pointer()
            .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
            .hover(|d| d.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
            .tip(tip)
            .child(named(icon, 16., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
    };
    let preview_btn = secondary("sb-preview", "speaker", crate::t!("soundboard.preview", name = label))
        .on_click(move |_, w, cx| {
            cx.stop_propagation();
            let p = preview.clone();
            with_app(cx, w, move |app, _, cx| crate::voice::soundboard::preview(app, &p, cx));
        });
    let fav = cx_favorite(app, s.id);
    let fav_btn = div()
        .id(SharedString::from(format!("sb-fav-{}-{}", key.0, key.1)))
        .flex()
        .cursor_pointer()
        .on_click(|_, _, cx| cx.stop_propagation())
        .tip(if fav {
            crate::t!("soundboard.unfavorite", name = label)
        } else {
            crate::t!("soundboard.favorite", name = label)
        })
        .child(if fav {
            named("star", 16., tk(idx::TEXT_FEEDBACK_WARNING))
        } else {
            named("star-outline", 16., tk(idx::INTERACTIVE_TEXT_DEFAULT))
        });
    let actions = div()
        .absolute()
        .inset_0()
        .px(px(8.))
        .flex()
        .items_center()
        .justify_between()
        .child(preview_btn)
        .when(!locked, |d| d.child(div().flex_1().flex().justify_center().child(named("play", 20., tk(idx::INTERACTIVE_TEXT_ACTIVE)))))
        .child(fav_btn);
    let hk = crate::ui::states::key(("sb-sound", s.id.0, key));
    let hover_layer = div()
        .absolute()
        .inset_0()
        .when(!locked, |d| d.child(overlay_bg()))
        .child(actions);
    let hover_layer = match crate::ui::states::state(hk) {
        Some((true, at)) if at.elapsed() < std::time::Duration::from_millis(OVERLAY_FADE_MS) => {
            use gpui::AnimationExt as _;
            hover_layer
                .with_animation(
                    gpui::ElementId::Name(format!("sb-overlay-{hk}-{at:?}").into()),
                    gpui::Animation::new(std::time::Duration::from_millis(OVERLAY_FADE_MS)).with_easing(crate::ui::states::cubic_bezier(0.25, 0.1, 0.25, 1.)),
                    |d, t| d.opacity(t),
                )
                .into_any_element()
        }
        Some((true, _)) => hover_layer.into_any_element(),
        _ => hover_layer.invisible().into_any_element(),
    };
    let button = crate::ui::states::track(div().id(SharedString::from(format!("sb-sound-{}-{}", key.0, key.1))), hk)
        .relative()
        .w(px(BUTTON_W))
        .h(px(BUTTON_H))
        .flex_none()
        .rounded(px(8.))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .border_1()
        .border_color(if playing {
            tk(idx::BRAND_500)
        } else if crate::theme::high_contrast() {
            tk(idx::BORDER_MUTED)
        } else {
            gpui::transparent_black()
        })
        .when(!playing, |d| d.hover(|d| d.border_color(tk(idx::BORDER_SUBTLE))))
        .when(can, |d| d.cursor_pointer())
        .flex()
        .child(info)
        .when(locked, |d| {
            d.child(overlay_bg())
                .child(div().absolute().inset_0().flex().items_center().justify_center().child(named("lock", 20., tk(idx::INTERACTIVE_TEXT_ACTIVE))))
        })
        .child(hover_layer)
        .on_click(move |_, w, cx| {
            let s = sound.clone();
            with_app(cx, w, move |app, _, cx| {
                crate::voice::soundboard::play_own(app, s, cx);
                cx.notify();
            });
        });
    div().w(px(BUTTON_W)).h(px(BUTTON_H)).flex_none().rounded(px(8.)).bg(tk(idx::BACKGROUND_BASE_LOWER)).child(button).into_any_element()
}

fn cx_favorite(_app: &PhoenixApp, id: Id) -> bool {
    FAVORITES.with(|f| f.borrow().contains(&id.0))
}

thread_local! {
    static FAVORITES: std::cell::RefCell<HashSet<u64>> = std::cell::RefCell::new(HashSet::new());
}

fn render_row(ix: usize, cx: &mut App) -> AnyElement {
    let Some(app) = cx.try_global::<Pickers>().and_then(|p| p.app.upgrade()) else { return div().into_any_element() };
    let p = cx.global::<Pickers>();
    let st = &p.soundboard;
    let Some(row) = st.rows.get(ix).copied() else { return div().into_any_element() };
    let app = app.read(cx);
    match row {
        Row::Header(si) => {
            let s = &st.sections[si];
            header_row(app, si, s, st.collapsed.contains(&s.key))
        }
        Row::Sounds { section, start } => {
            let s = &st.sections[section];
            let end = (start + PER_ROW).min(s.sounds.len());
            div()
                .flex()
                .gap(px(8.))
                .pl(px(8.))
                .pb(px(8.))
                .children(s.sounds[start..end].iter().enumerate().map(|(i, x)| sound_button(app, x, s.locked || !x.available, (section, start + i))))
                .into_any_element()
        }
    }
}

fn rail(app: &PhoenixApp, st: &SoundboardState) -> AnyElement {
    let active = active_section(st);
    let mut col = div().id("sb-rail").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().items_center().pt(px(8.));
    for (si, s) in st.sections.iter().enumerate() {
        if matches!(s.kind, Kind::Search) {
            continue;
        }
        let is_active = si == active;
        let guild = matches!(s.kind, Kind::Guild(_));
        let first = st.rows.iter().position(|r| *r == Row::Header(si)).unwrap_or(0);
        let ls = st.list.clone();
        let color = tk(if is_active { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT });
        let content = match &s.kind {
            Kind::Guild(g) => guild_icon(app, *g, 32.),
            k => section_icon(app, k, 24., color),
        };
        col = col.child(
            super::grid::rail_item(("sb-rail", si), guild, is_active, 8.)
                .tip(section_title(app, &s.kind))
                .child(content)
                .on_click(move |_, w, cx| {
                    ls.scroll_to(ListOffset { item_ix: first, offset_in_item: px(0.) });
                    with_app(cx, w, |_, _, cx| cx.notify());
                }),
        );
    }
    div()
        .w(px(RAIL_W))
        .flex_none()
        .h_full()
        .rounded_bl(px(8.))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .when(crate::theme::high_contrast(), |d| d.border_r_1().border_color(tk(idx::BORDER_SUBTLE)))
        .flex()
        .flex_col()
        .child(col)
        .into_any_element()
}

pub fn geometry(vw: f32, vh: f32, anchor: Option<Bounds<Pixels>>) -> (f32, f32, f32) {
    match anchor {
        Some(b) => {
            let cx = f32::from(b.origin.x) + f32::from(b.size.width) / 2.;
            let bottom = f32::from(b.origin.y) - GAP_ABOVE_BUTTON;
            let h = H.min(bottom - 8.).max(200.);
            let left = (cx - W / 2.).clamp(8., (vw - W - 8.).max(8.));
            (left, bottom - h, h)
        }
        None => {
            let h = H.min(vh - 84.).max(200.);
            (80., (vh - 68. - h).max(8.), h)
        }
    }
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !is_open(cx) {
        return None;
    }
    if app.voice_channel.is_none() {
        close(cx);
        return None;
    }
    if let Some(f) = cx.global::<Pickers>().frecency.as_ref() {
        let set: HashSet<u64> = f.favorite_sounds.iter().copied().collect();
        FAVORITES.with(|x| *x.borrow_mut() = set);
    }
    let vp = window.viewport_size();
    let (left, top, h) = geometry(f32::from(vp.width), f32::from(vp.height), current_anchor());
    let st = &cx.global::<Pickers>().soundboard;
    let input = st.input.clone()?;
    let rail_el = rail(app, st);
    let empty = st.rows.is_empty();
    let loading = st.loading;
    let list_el: AnyElement = if empty {
        super::empty_state(if loading { crate::t!("soundboard.loading") } else { crate::t!("soundboard.empty") })
    } else {
        div()
            .relative()
            .size_full()
            .child(list(st.list.clone(), |ix, _w, cx| render_row(ix, cx)).size_full())
            .children(crate::ui::chat::scrollbar::thin_list_scrollbar(&st.list))
            .into_any_element()
    };
    let header = div()
        .h(px(HEADER_H))
        .flex_none()
        .p(px(12.))
        .flex()
        .items_center()
        .shadow(vec![
            gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.2), offset: gpui::point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: false },
            gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.04), offset: gpui::point(px(0.), px(1.5)), blur_radius: px(0.), spread_radius: px(0.), inset: false },
            gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.04), offset: gpui::point(px(0.), px(2.)), blur_radius: px(0.), spread_radius: px(0.), inset: false },
        ])
        .child(search_box(&input))
        .child(
            div()
                .id("sb-volume")
                .ml(px(12.))
                .flex()
                .cursor_pointer()
                .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                .tip(crate::t!("soundboard.volume"))
                .child(named("speaker", 24., tk(idx::INTERACTIVE_TEXT_DEFAULT))),
        );
    let panel = div()
        .id("soundboard-picker")
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(W))
        .h(px(h))
        .rounded(px(8.))
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .shadow(crate::ui::inbox::popout_shadow())
        .overflow_hidden()
        .flex()
        .flex_col()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(header)
        .child(div().flex_1().min_h_0().flex().child(rail_el).child(div().flex_1().min_w_0().h_full().child(list_el)));
    Some(
        div()
            .id("soundboard-layer")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    close(cx);
                    cx.notify();
                }),
            )
            .child(panel)
            .into_any_element(),
    )
}

pub fn call_button(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::theme::z;
    let off = disabled(app);
    let open = is_open(cx);
    let hover = tk(idx::BACKGROUND_MOD_MUTED);
    let hk = crate::ui::states::key("call-soundboard");
    crate::ui::states::track(div().id("call-soundboard"), hk)
        .relative()
        .size(z(40.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(z(8.))
        .when(open && !off, |d| d.bg(hover))
        .when(!off, |d| d.hover(move |s| s.bg(hover)).cursor_pointer())
        .when(off, |d| d.opacity(0.5).cursor_not_allowed())
        .tip(disabled_reason(app).unwrap_or_else(|| crate::t!("voice.soundboard")))
        .child(crate::ui::hover_icon::icon(hk, crate::icons::lottie_icons::Lottie::Soundboard, false, 20. * crate::theme::zoom(), (tk(idx::WHITE), tk(idx::WHITE)), || {
            gpui::svg().path("icons/voice/soundboard.svg").size(z(20.)).flex_none().text_color(tk(idx::WHITE)).into_any_element()
        }))
        .child(anchor_probe())
        .on_click(cx.listener(move |app, _, window, cx| {
            if !off {
                crate::ui::hover_icon::clicked(hk);
                toggle_from_call(app, window, cx);
            }
        }))
        .into_any_element()
}

pub fn panel_button(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    use crate::theme::z;
    let off = disabled(app);
    let hk = crate::ui::states::key("voice-soundboard");
    let fg = tk(idx::CONTROL_SECONDARY_TEXT_DEFAULT);
    let dot = !off && has_new_sounds(app, cx);
    let cut = tk(if crate::ui::states::hovered(hk) { idx::CONTROL_SECONDARY_BACKGROUND_HOVER } else { idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT });
    crate::ui::states::track(div().id("voice-soundboard"), hk)
        .relative()
        .flex_1()
        .min_w_0()
        .h(z(32.))
        .rounded(z(8.))
        .border_1()
        .border_color(tk(idx::BORDER_MUTED))
        .bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
        .when(!off, |d| d.hover(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))).cursor_pointer())
        .when(off, |d| d.opacity(0.3).cursor_not_allowed())
        .flex()
        .items_center()
        .justify_center()
        .tip(disabled_reason(app).unwrap_or_else(|| crate::t!("voice.soundboard")))
        .child(
            div()
                .relative()
                .flex_none()
                .size(z(20.))
                .child(crate::ui::hover_icon::icon(hk, crate::icons::lottie_icons::Lottie::Soundboard, false, 20. * crate::theme::zoom(), (fg, fg), || {
                    gpui::svg().path("icons/voice/soundboard.svg").size(z(20.)).flex_none().text_color(fg).into_any_element()
                }))
                .when(dot, |d| {
                    d.child(div().absolute().left(z(12.)).top(z(12.)).size(z(10.)).rounded_full().bg(cut))
                        .child(div().absolute().right_0().bottom_0().size(z(6.)).rounded_full().bg(tk(idx::BLURPLE_50)))
                }),
        )
        .child(panel_anchor_probe())
        .on_click(cx.listener(move |app, _, window, cx| {
            if !off {
                crate::ui::hover_icon::clicked(hk);
                toggle_from_panel(app, window, cx);
            }
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: u64, g: u64) -> SoundboardSound {
        SoundboardSound { id: Id(id), guild: Some(Id(g)), name: Some(format!("s{id}").into()), volume: 1., available: true, ..Default::default() }
    }

    #[test]
    fn new_sounds_follow_the_seen_threshold() {
        let me = Id(7);
        let mut a = s(50, 1);
        assert!(is_new_sound(&a, 40, me));
        assert!(!is_new_sound(&a, 50, me));
        a.user = Some(me);
        assert!(!is_new_sound(&a, 40, me));
        let b = SoundboardSound { available: false, ..s(60, 1) };
        assert!(!is_new_sound(&b, 40, me));
    }

    #[test]
    fn rows_group_by_three_and_collapse() {
        let sections = vec![
            Section { kind: Kind::Favorites, key: "favorites".into(), sounds: vec![s(1, 1)], locked: false },
            Section { kind: Kind::Defaults, key: "0".into(), sounds: (0..7).map(|i| s(10 + i, 0)).collect(), locked: false },
        ];
        let rows = build_rows(&sections, &HashSet::new());
        assert_eq!(rows.len(), 1 + 1 + 1 + 3);
        assert_eq!(rows[3], Row::Sounds { section: 1, start: 0 });
        assert_eq!(rows[5], Row::Sounds { section: 1, start: 6 });
        let collapsed: HashSet<String> = ["0".to_string()].into_iter().collect();
        assert_eq!(build_rows(&sections, &collapsed).len(), 3);
    }

    #[test]
    fn popout_sits_above_the_button() {
        let b = Bounds { origin: gpui::point(px(1017.3), px(775.2)), size: gpui::size(px(40.), px(40.)) };
        let (left, top, h) = geometry(1536., 840., Some(b));
        assert!((left - 771.8).abs() < 0.5, "{left}");
        assert!((top - 239.2).abs() < 0.5, "{top}");
        assert_eq!(h, H);
    }
}

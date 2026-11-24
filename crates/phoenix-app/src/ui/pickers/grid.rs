use crate::ui::tooltip::Tipped as _;
use gpui::{
    AnyElement, App, Context, FontWeight, ListOffset, ParentElement, SharedString, Styled, div,
    img, list, prelude::*, px,
};

use super::{Pickers, emoji, pick, types::GuildEmoji, with_app};
use crate::{
    app::PhoenixApp,
    media::emoji::emoji_element,
    model::{Guild, ImageRef},
};

pub const COLS: usize = 9;
const CELL: f32 = 48.;
const EMOJI_PX: f32 = 40.;
const FREQUENT: usize = 42;

#[derive(Clone, Debug, PartialEq)]
pub enum PickItem {
    Unicode(usize),
    Custom(GuildEmoji),
}

impl PickItem {
    pub fn name(&self) -> SharedString {
        match self {
            Self::Unicode(ix) => emoji::shortcode(*ix).into(),
            Self::Custom(e) => e.name.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SectionIcon {
    Frequent,
    Guild(ImageRef, SharedString),
    Category(usize),
}

#[derive(Clone, Debug)]
pub struct Section {
    pub title: SharedString,
    pub icon: SectionIcon,
    pub first_row: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Header(usize),
    Cells { start: usize, end: usize },
}

#[derive(Default)]
pub struct Grid {
    pub items: Vec<PickItem>,
    pub rows: Vec<Row>,
    pub sections: Vec<Section>,
    pub guild_names: Vec<(crate::model::Id, SharedString)>,
    pub premium: bool,
    pub current_guild: Option<crate::model::Id>,
}

impl Grid {
    pub fn locked(&self, item: &PickItem) -> bool {
        match item {
            PickItem::Unicode(_) => false,
            PickItem::Custom(e) => !e.available || (!self.premium && Some(e.guild) != self.current_guild),
        }
    }

    pub fn section_locked(&self, si: usize) -> bool {
        let Some(SectionIcon::Guild(..)) = self.sections.get(si).map(|s| &s.icon) else { return false };
        let first = self.sections[si].first_row;
        self.rows[first..]
            .iter()
            .take_while(|r| !matches!(r, Row::Header(h) if *h != si))
            .find_map(|r| if let Row::Cells { start, .. } = r { self.items.get(*start) } else { None })
            .is_some_and(|it| self.locked(it))
    }

    fn push_section(&mut self, title: SharedString, icon: SectionIcon, items: Vec<PickItem>, header: bool) {
        if items.is_empty() {
            return;
        }
        let si = self.sections.len();
        self.sections.push(Section { title, icon, first_row: self.rows.len() });
        if header {
            self.rows.push(Row::Header(si));
        }
        let base = self.items.len();
        let n = items.len();
        self.items.extend(items);
        for chunk in (0..n).step_by(COLS) {
            self.rows.push(Row::Cells { start: base + chunk, end: base + (chunk + COLS).min(n) });
        }
    }

    pub fn row_of(&self, item: usize) -> Option<usize> {
        self.rows.iter().position(|r| matches!(r, Row::Cells { start, end } if (*start..*end).contains(&item)))
    }

    pub fn vertical(&self, item: usize, down: bool) -> usize {
        let Some(r) = self.row_of(item) else { return item };
        let Row::Cells { start, .. } = self.rows[r] else { return item };
        let col = item - start;
        let mut ix = r;
        loop {
            ix = if down {
                if ix + 1 >= self.rows.len() {
                    return item;
                }
                ix + 1
            } else {
                if ix == 0 {
                    return item;
                }
                ix - 1
            };
            if let Row::Cells { start, end } = self.rows[ix] {
                return (start + col).min(end - 1);
            }
        }
    }

    pub fn section_at_row(&self, row: usize) -> usize {
        self.sections.iter().rposition(|s| s.first_row <= row).unwrap_or(0)
    }
}

pub fn build(app: &PhoenixApp, query: &str, cx: &App) -> Grid {
    let mut guilds: Vec<&Guild> = app.guilds.iter().collect();
    if let Some(sel) = app.selected_guild {
        guilds.sort_by_key(|g| g.id != sel);
    }
    let customs: Vec<(&Guild, Vec<GuildEmoji>)> = guilds
        .into_iter()
        .map(|g| (g, app.backend.guild_emojis(g.id)))
        .filter(|(_, e)| !e.is_empty())
        .collect();
    let mut grid = Grid {
        guild_names: customs.iter().map(|(g, _)| (g.id, g.name.clone())).collect(),
        premium: app.backend.has_premium(),
        current_guild: app.selected_guild,
        ..Default::default()
    };
    let q = query.trim().trim_matches(':').to_lowercase();
    if q.is_empty() {
        let now = chrono::Utc::now().timestamp();
        let mut freq: Vec<PickItem> = Vec::new();
        if let Some(f) = cx.try_global::<Pickers>().and_then(|p| p.frecency.as_ref()) {
            for key in &f.emojis {
                let item = match key.parse::<u64>() {
                    Ok(id) => customs.iter().flat_map(|(_, es)| es).find(|e| e.id.0 == id).cloned().map(PickItem::Custom),
                    Err(_) => emoji::by_shortcode(key).or_else(|| emoji::index_of(key)).map(PickItem::Unicode),
                };
                if let Some(it) = item
                    && !freq.contains(&it)
                {
                    freq.push(it);
                }
                if freq.len() >= FREQUENT {
                    break;
                }
            }
        }
        for it in cx.try_global::<Pickers>().map(|p| p.prefs.frequent(FREQUENT, now)).unwrap_or_default() {
            if freq.len() >= FREQUENT {
                break;
            }
            if !freq.contains(&it) {
                freq.push(it);
            }
        }
        grid.push_section(crate::t!("picker.frequent"), SectionIcon::Frequent, freq, true);
        for (g, es) in customs {
            let items = es.into_iter().map(PickItem::Custom).collect();
            grid.push_section(g.name.clone(), SectionIcon::Guild(g.icon.clone(), g.acronym.clone()), items, true);
        }
        for cat in 0..emoji::CATEGORIES.len() {
            let items = (0..emoji::EMOJI.len()).filter(|&i| emoji::EMOJI[i].3 as usize == cat).map(PickItem::Unicode).collect();
            grid.push_section(crate::i18n::t(&format!("emoji.cat.{cat}"), &[]), SectionIcon::Category(cat), items, true);
        }
    } else {
        let mut custom: Vec<(u8, GuildEmoji)> = customs
            .into_iter()
            .flat_map(|(_, es)| es)
            .filter_map(|e| emoji::name_score(&e.name.to_lowercase(), &q).map(|s| (s, e)))
            .collect();
        custom.sort_by_key(|(s, _)| *s);
        let mut items: Vec<PickItem> = custom.into_iter().map(|(_, e)| PickItem::Custom(e)).collect();
        items.extend(emoji::search(&q, 300).into_iter().map(PickItem::Unicode));
        grid.push_section(crate::t!("picker.results"), SectionIcon::Frequent, items, false);
    }
    grid
}

use super::{idx, section_header, tk};

pub fn item_element(item: &PickItem, tone: u8, size: f32) -> AnyElement {
    match item {
        PickItem::Unicode(ix) => emoji_element(&emoji::toned(*ix, tone), size),
        PickItem::Custom(e) => crate::media::emoji::emoji_element(&e.token(), size),
    }
}

pub fn item_element_hover(item: &PickItem, tone: u8, size: f32) -> AnyElement {
    match item {
        PickItem::Unicode(ix) => emoji_element(&emoji::toned(*ix, tone), size),
        PickItem::Custom(e) => crate::media::emoji::emoji_element_hover(&e.token(), size),
    }
}

fn section_icon(icon: &SectionIcon) -> Option<AnyElement> {
    let fg = tk(idx::TEXT_DEFAULT);
    match icon {
        SectionIcon::Frequent => Some(gpui::svg().path(crate::icons::path("picker/clock")).size(px(16.)).flex_none().text_color(fg).into_any_element()),
        SectionIcon::Guild(ImageRef::Url(u), _) => Some(img(u.clone()).size(px(16.)).rounded(px(4.)).flex_none().into_any_element()),
        SectionIcon::Guild(..) => None,
        SectionIcon::Category(cat) => Some(emoji_element(emoji::CATEGORY_ICON[*cat], 16.)),
    }
}

fn render_row(ix: usize, cx: &mut App) -> AnyElement {
    let p = cx.global::<Pickers>();
    let Some(op) = p.open.as_ref() else { return div().into_any_element() };
    let tone = p.prefs.tone;
    let last_of_section = !matches!(op.grid.rows.get(ix + 1), Some(Row::Cells { .. }));
    match op.grid.rows.get(ix).copied() {
        Some(Row::Header(si)) => {
            let s = &op.grid.sections[si];
            section_header(section_icon(&s.icon), s.title.clone()).into_any_element()
        }
        Some(Row::Cells { start, end }) => {
            let selected = op.selected;
            let mut row = div().flex().pl(px(8.)).when(last_of_section, |d| d.pb(px(12.)));
            for i in start..end {
                let item = op.grid.items[i].clone();
                let is_sel = i == selected;
                let locked = op.grid.locked(&item);
                row = row.child(
                    div()
                        .id(("emoji-cell", i))
                        .relative()
                        .size(px(CELL))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .when(is_sel, |d| {
                            if crate::theme::is_light() {
                                d.bg(tk(idx::BACKGROUND_BASE_LOW)).border_1().border_color(tk(idx::BORDER_SUBTLE))
                            } else {
                                d.bg(tk(idx::INTERACTIVE_BACKGROUND_SELECTED))
                            }
                        })
                        .on_hover(move |h, w, cx| {
                            if *h {
                                with_app(cx, w, |_, _, cx| {
                                    if let Some(op) = cx.global_mut::<Pickers>().open.as_mut()
                                        && (op.hovered != Some(i) || op.selected != i)
                                    {
                                        op.hovered = Some(i);
                                        op.selected = i;
                                        cx.notify();
                                    }
                                });
                            }
                        })
                        .when(!locked, |d| {
                            d.on_click({
                                let item = item.clone();
                                move |ev, w, cx| {
                                    let keep = ev.modifiers().shift;
                                    let item = item.clone();
                                    with_app(cx, w, move |a, w, cx| pick(a, item, keep, w, cx));
                                }
                            })
                        })
                        .child(div().when(locked, |d| d.opacity(0.3)).child(item_element_hover(&item, tone, EMOJI_PX)))
                        .when(locked, |d| d.child(lock_badge())),
                );
            }
            row.into_any_element()
        }
        None => div().into_any_element(),
    }
}

fn lock_badge() -> impl IntoElement {
    div().absolute().inset_0().flex().items_center().justify_center().child(
        div()
            .size(px(20.))
            .rounded(px(2.))
            .bg(tk(idx::OPACITY_BLACK_60))
            .flex()
            .items_center()
            .justify_center()
            .child(crate::ui::icon(crate::assets::Icon::Lock, 14., tk(idx::ICON_STRONG))),
    )
}

pub(crate) fn rail() -> gpui::Stateful<gpui::Div> {
    div()
        .id("picker-rail")
        .w(px(48.))
        .flex_none()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .items_center()
        .pt(px(8.))
}

pub(crate) fn rail_shell(list: impl IntoElement, shortcut: impl IntoElement) -> gpui::Div {
    div()
        .w(px(48.))
        .flex_none()
        .h_full()
        .rounded_bl(px(8.))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .when(crate::theme::high_contrast(), |d| d.border_r_1().border_color(tk(idx::BORDER_SUBTLE)))
        .flex()
        .flex_col()
        .child(list)
        .child(
            div()
                .h(px(48.))
                .flex_none()
                .border_t_1()
                .border_color(tk(idx::BORDER_NORMAL))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(shortcut),
        )
}

pub(crate) fn rail_pill(active: bool) -> impl IntoElement {
    div()
        .absolute()
        .left(px(-8.))
        .top_0()
        .bottom_0()
        .w(px(8.))
        .flex()
        .items_center()
        .when(active, |d| d.child(div().w(px(4.)).h(px(24.)).rounded_r(px(12.)).bg(tk(idx::INTERACTIVE_TEXT_ACTIVE))))
}

pub(crate) fn rail_item(id: (&'static str, usize), guild: bool, active: bool, mb: f32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .relative()
        .size(px(32.))
        .flex_none()
        .mb(px(mb))
        .rounded(px(if guild { 8. } else { 4. }))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(!guild && active, |d| d.bg(tk(idx::BACKGROUND_BASE_LOW)))
        .when(!guild && !active, |d| d.hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER))))
        .child(rail_pill(active))
}

pub(crate) fn rail_guild_icon(icon: &ImageRef, acronym: &SharedString) -> AnyElement {
    match icon {
        ImageRef::Url(u) => img(u.clone()).size(px(32.)).rounded(px(8.)).into_any_element(),
        _ => div()
            .size(px(32.))
            .rounded(px(8.))
            .bg(tk(idx::BACKGROUND_BASE_LOW))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tk(idx::TEXT_DEFAULT))
            .child(acronym.clone())
            .into_any_element(),
    }
}

fn rail_lock() -> impl IntoElement {
    div()
        .absolute()
        .right(px(-1.))
        .bottom(px(-1.))
        .size(px(16.))
        .rounded_full()
        .bg(tk(idx::BACKGROUND_BASE_LOW))
        .border_1()
        .border_color(tk(idx::BACKGROUND_BASE_LOWEST))
        .flex()
        .items_center()
        .justify_center()
        .child(crate::ui::icon(crate::assets::Icon::Lock, 10., tk(idx::INTERACTIVE_TEXT_DEFAULT)))
}

pub(crate) fn inspector(graphic: Option<AnyElement>, title: Option<SharedString>, from: Option<SharedString>, trailing: Option<AnyElement>) -> gpui::Div {
    let mut text = div().ml(px(8.)).flex_1().min_w_0().flex().flex_col();
    if let Some(t) = title {
        text = text.child(
            div()
                .text_size(px(16.))
                .line_height(px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tk(idx::TEXT_DEFAULT))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(t),
        );
    }
    if let Some(g) = from {
        let full = crate::t!("picker.from", g = g);
        let (pre, _) = full.split_once(g.as_ref()).unwrap_or((full.as_ref(), ""));
        text = text.child(
            div()
                .flex()
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(SharedString::from(pre.to_owned()))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(g)),
        );
    }
    div()
        .h(px(48.))
        .flex_none()
        .px(px(16.))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .border_t_1()
        .border_color(tk(idx::BORDER_NORMAL))
        .flex()
        .items_center()
        .children(graphic.map(|g| div().size(px(28.)).flex_none().flex().items_center().justify_center().child(g)))
        .child(text)
        .children(trailing)
}

pub fn render_body(_app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let p = cx.global::<Pickers>();
    let Some(op) = p.open.as_ref() else { return div().into_any_element() };
    let tone = p.prefs.tone;
    let list_state = op.list.clone();
    let active = op.grid.section_at_row(op.list.logical_scroll_top().item_ix);
    let preview = op.hovered.or(Some(op.selected)).and_then(|i| op.grid.items.get(i).cloned());
    let preview_from: Option<SharedString> = match &preview {
        Some(PickItem::Custom(e)) => op.grid.guild_names.iter().find(|(g, _)| *g == e.guild).map(|(_, n)| n.clone()),
        _ => None,
    };
    let empty = op.grid.items.is_empty();
    let searching = op.grid.rows.first().is_some_and(|r| matches!(r, Row::Cells { .. }));

    let mut list_el = rail();
    let mut first_unicode = None;
    if !searching && !op.grid.premium {
        list_el = list_el.child(
            div().flex_none().flex().justify_center().pb(px(8.)).mb(px(8.)).border_b_1().border_color(tk(idx::BORDER_SUBTLE)).child(
                rail_item(("emoji-nitro", 0), false, false, 0.)
                    .child(
                        gpui::svg()
                            .path(crate::icons::path("picker/nitro"))
                            .size(px(24.))
                            .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)),
                    )
                    .tip(crate::t!("picker.see_nitro_emoji")),
            ),
        );
    }
    if !searching {
        let n = op.grid.sections.len();
        for (si, s) in op.grid.sections.iter().enumerate() {
            let first = s.first_row;
            let is_active = si == active;
            let next_is_guild = matches!(op.grid.sections.get(si + 1).map(|s| &s.icon), Some(SectionIcon::Guild(..)));
            let (guild, content): (bool, AnyElement) = match &s.icon {
                SectionIcon::Frequent => (
                    false,
                    gpui::svg()
                        .path(crate::icons::path("picker/clock"))
                        .size(px(24.))
                        .text_color(tk(if is_active { idx::INTERACTIVE_TEXT_ACTIVE } else { idx::INTERACTIVE_TEXT_DEFAULT }))
                        .into_any_element(),
                ),
                SectionIcon::Category(cat) => {
                    first_unicode.get_or_insert(first);
                    (false, div().when(!is_active, |d| d.opacity(0.6)).child(emoji_element(emoji::CATEGORY_ICON[*cat], 24.)).into_any_element())
                }
                SectionIcon::Guild(icon, acr) => (true, rail_guild_icon(icon, acr)),
            };
            if matches!(s.icon, SectionIcon::Category(0)) && si > 0 {
                list_el = list_el.child(div().flex_none().w(px(32.)).h(px(1.)).mt(px(4.)).mb(px(12.)).bg(tk(idx::BORDER_SUBTLE)));
            }
            let mb = if guild || next_is_guild || si + 1 == n { 8. } else { 2. };
            let ls = list_state.clone();
            let locked = guild && op.grid.section_locked(si);
            list_el = list_el.child(
                rail_item(("emoji-section", si), guild, is_active, mb)
                    .on_click(move |_, w, cx| {
                        ls.scroll_to(ListOffset { item_ix: first, offset_in_item: px(0.) });
                        with_app(cx, w, |_, _, cx| cx.notify());
                    })
                    .child(content)
                    .when(locked, |d| d.child(rail_lock())),
            );
        }
    }
    let ls = list_state.clone();
    let shortcut = div()
        .id("emoji-unicode-shortcut")
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .on_click(move |_, w, cx| {
            if let Some(f) = first_unicode {
                ls.scroll_to(ListOffset { item_ix: f, offset_in_item: px(0.) });
                with_app(cx, w, |_, _, cx| cx.notify());
            }
        })
        .child(crate::ui::icon(crate::assets::Icon::Emoji, 24., tk(idx::INTERACTIVE_TEXT_DEFAULT)));
    let sidebar = rail_shell(list_el, shortcut);

    let grid_el = if empty {
        super::empty_state(crate::t!("picker.no_emoji"))
    } else {
        list(list_state, |ix, _w, cx| render_row(ix, cx)).size_full().into_any_element()
    };

    let footer = match preview {
        Some(item) => inspector(
            Some(item_element(&item, tone, 28.)),
            Some(SharedString::from(format!(":{}:", item.name()))),
            preview_from,
            None,
        ),
        None => inspector(None, None, None, None),
    };

    div()
        .size_full()
        .flex()
        .flex_row()
        .when(!searching, |d| d.child(sidebar))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().flex_1().min_h_0().when(searching, |d| d.pt(px(8.))).child(grid_el))
                .child(footer),
        )
        .into_any_element()
}

pub fn tone_button(cx: &mut Context<PhoenixApp>) -> AnyElement {
    let p = cx.global::<Pickers>();
    let tone = p.prefs.tone;
    let open = p.open.as_ref().is_some_and(|o| o.tone_open);
    let mut el = div().relative().size(px(38.)).flex_none().child(
        div()
            .id("tone-button")
            .size(px(38.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .cursor_pointer()
            .on_click(cx.listener(|_, _, _, cx| {
                if let Some(op) = cx.global_mut::<Pickers>().open.as_mut() {
                    op.tone_open = !op.tone_open;
                }
                cx.notify();
            }))
            .child(emoji_element(&emoji::with_tone(emoji::TONE_PREVIEW, tone), 24.)),
    );
    if open {
        let mut menu = div()
            .absolute()
            .top(px(-1.))
            .right(px(-1.))
            .w(px(40.))
            .rounded(px(4.))
            .bg(tk(idx::BACKGROUND_SURFACE_HIGHER))
            .border_1()
            .border_color(tk(idx::BORDER_SUBTLE))
            .flex()
            .flex_col();
        let order = std::iter::once(tone).chain((0..=5u8).filter(move |t| *t != tone));
        for t in order {
            menu = menu.child(
                div()
                    .id(("tone", t as usize))
                    .size(px(38.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(tk(idx::INTERACTIVE_BACKGROUND_HOVER)))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        let p = cx.global_mut::<Pickers>();
                        p.prefs.set_tone(t);
                        if let Some(op) = p.open.as_mut() {
                            op.tone_open = false;
                        }
                        cx.notify();
                    }))
                    .child(emoji_element(&emoji::with_tone(emoji::TONE_PREVIEW, t), 24.)),
            );
        }
        el = el.child(gpui::deferred(menu).with_priority(1));
    }
    el.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_of(sizes: &[usize]) -> Grid {
        let mut g = Grid::default();
        let mut n = 0;
        for &s in sizes {
            let items = (n..n + s).map(PickItem::Unicode).collect();
            n += s;
            g.push_section("x".into(), SectionIcon::Frequent, items, true);
        }
        g
    }

    #[test]
    fn rows_and_navigation() {
        let g = grid_of(&[11, 4]);
        assert_eq!(g.rows.len(), 5);
        assert_eq!(g.row_of(10), Some(2));
        assert_eq!(g.vertical(1, true), 10);
        assert_eq!(g.vertical(8, true), 10);
        assert_eq!(g.vertical(10, true), 12);
        assert_eq!(g.vertical(12, false), 10);
        assert_eq!(g.vertical(0, false), 0);
        assert_eq!(g.section_at_row(3), 1);
    }
}

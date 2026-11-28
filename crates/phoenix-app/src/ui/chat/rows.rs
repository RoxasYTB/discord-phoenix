use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    time::{Duration, Instant},
};

use gpui::{AnyElement, App, AvailableSpace, Bounds, Entity, IntoElement, ListOffset, ListState, Pixels, Styled, canvas, px, size};

use crate::{app::PhoenixApp, model::Id};

type RenderRow = fn(&Entity<PhoenixApp>, usize, &mut App) -> AnyElement;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Keep {
    channel: Id,
    item: usize,
    y: f32,
}

const ESTIMATE: f32 = 56.;
const MEASURE_BUDGET: Duration = Duration::from_millis(3);
const HEIGHTS_CAP: usize = 20_000;

type HeightKey = (u64, u32, u32);

thread_local! {
    static HEIGHTS: RefCell<HashMap<HeightKey, f32>> = RefCell::new(HashMap::new());
    static HINTED: Cell<(u64, u32, usize)> = const { Cell::new((0, 0, usize::MAX)) };
    static PENDING: RefCell<Vec<std::ops::Range<usize>>> = const { RefCell::new(Vec::new()) };
    static KEEP: Cell<Option<Keep>> = const { Cell::new(None) };
    static TOP_ROW: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    static TOP_ROW_SHOWN: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    static SETTLED: Cell<bool> = const { Cell::new(true) };
}

#[cfg(test)]
pub fn is_settled() -> bool {
    SETTLED.get()
}

pub fn insert(list: &ListState, at: usize, n: usize) {
    list.splice(at..at, n);
    PENDING.with_borrow_mut(|p| p.push(at..at + n));
    list.remeasure_items(0..0);
}

pub fn refresh(list: &ListState, ix: usize) {
    list.remeasure_items(ix..ix + 1);
}

pub fn refresh_containing(app: &PhoenixApp, needle: &str) {
    let Some(entry) = app.selected_channel.and_then(|c| app.messages.get(&c)) else { return };
    for (ix, m) in entry.msgs.iter().enumerate() {
        if m.content.contains(needle) {
            app.message_list.remeasure_items(ix + 1..ix + 2);
        }
    }
}

pub fn prepend(list: &ListState, channel: Id, n: usize, len: usize) {
    let top = list.logical_scroll_top();
    let keep = (n > 0 && n < len && top.item_ix == 0).then(|| {
        let view = list.viewport_bounds();
        list.bounds_for_item(1)
            .map(|b| f32::from(b.top() - view.top()))
            .or_else(|| list.bounds_for_item(0).map(|b| f32::from(b.size.height - top.offset_in_item)))
            .unwrap_or(0.)
    });
    if n > 0 {
        list.splice(1..1, n);
        PENDING.with_borrow_mut(|p| p.push(1..1 + n));
    }
    list.remeasure_items(0..1);
    if n > 0 && n < len {
        list.remeasure_items(n + 1..n + 2);
    }
    KEEP.set(keep.map(|y| Keep { channel, item: n + 1, y }));
}

pub fn measure_everything() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("PHX_MEASURE_ALL").is_some_and(|v| !v.is_empty() && v != "0"))
}

fn row_key(app: &PhoenixApp, channel: Id, ix: usize) -> Option<u64> {
    if ix == 0 {
        return Some(!channel.0);
    }
    app.messages.get(&channel)?.msgs.get(ix - 1).map(|m| m.id.0)
}

fn set_hint(list: &ListState, ix: usize, height: f32) {
    list.splice(ix..ix + 1, 1);
    let _ = list.clone().with_uniform_item_height(px(height));
}

fn cached(key: Option<u64>, width: u32, generation: u32) -> Option<f32> {
    HEIGHTS.with_borrow(|h| h.get(&(key?, width, generation)).copied())
}

fn remember(key: Option<u64>, width: u32, generation: u32, height: f32) {
    let Some(key) = key else { return };
    HEIGHTS.with_borrow_mut(|h| {
        if h.len() >= HEIGHTS_CAP {
            h.clear();
        }
        h.insert((key, width, generation), height);
    });
}

pub fn measure_ahead(entity: Entity<PhoenixApp>, render: RenderRow) -> AnyElement {
    canvas(
        move |_, window, cx| {
            if measure_everything() {
                return;
            }
            let (list, channel) = {
                let app = entity.read(cx);
                (app.message_list.clone(), app.selected_channel)
            };
            let Some(channel) = channel else { return };
            let view = list.viewport_bounds();
            if view.size.width <= px(0.) {
                return;
            }
            let width = f32::from(view.size.width).round() as u32;
            let generation = crate::theme::style_generation();
            let count = list.item_count();
            let top = list.logical_scroll_top();
            let keys: Vec<Option<u64>> = {
                let app = entity.read(cx);
                (0..count).map(|ix| row_key(app, channel, ix)).collect()
            };
            for (ix, key) in keys.iter().enumerate().skip(top.item_ix) {
                if let Some(b) = list.bounds_for_item(ix) {
                    remember(*key, width, generation, f32::from(b.size.height));
                }
            }
            let pending = PENDING.take();
            let (seen_channel, seen_width, seen_count) = HINTED.get();
            let full = seen_channel != channel.0 || seen_width != width || (seen_count != count && pending.is_empty());
            if full || !pending.is_empty() || seen_count != count {
                HINTED.set((channel.0, width, count));
                let ranges: Vec<std::ops::Range<usize>> = if full { std::iter::once(0..count).collect() } else { pending };
                for ix in ranges.into_iter().flatten().filter(|ix| *ix < count) {
                    if ix != top.item_ix
                        && list.bounds_for_item(ix).is_none()
                        && let Some(h) = cached(keys[ix], width, generation)
                    {
                        set_hint(&list, ix, h);
                    }
                }
                let _ = list.clone().with_uniform_item_height(px(ESTIMATE));
            }
            let mut order: Vec<usize> = (0..count).filter(|ix| keys[*ix].is_some() && cached(keys[*ix], width, generation).is_none()).collect();
            if order.is_empty() {
                SETTLED.set(true);
                return;
            }
            SETTLED.set(false);
            order.sort_by_key(|ix| ix.abs_diff(top.item_ix));
            let space = size(AvailableSpace::Definite(view.size.width), AvailableSpace::MinContent);
            let started = Instant::now();
            for ix in order.iter().copied() {
                if started.elapsed() > MEASURE_BUDGET {
                    break;
                }
                let h = f32::from(render(&entity, ix, cx).layout_as_root(space, window, cx).height);
                remember(keys[ix], width, generation, h);
                if ix != top.item_ix && list.bounds_for_item(ix).is_none() {
                    set_hint(&list, ix, h);
                }
            }
            window.request_animation_frame();
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_0()
    .into_any_element()
}

pub fn top_row_probe() -> AnyElement {
    canvas(|b, _, _| TOP_ROW.set(Some(b)), |_, _, _, _| {}).absolute().top_0().left_0().size_full().into_any_element()
}

pub fn skeleton_in_view(list: &ListState) -> bool {
    let view = list.viewport_bounds();
    view.size.height > px(0.) && TOP_ROW_SHOWN.get().is_some_and(|b| b.bottom() > view.top() && b.top() < view.bottom())
}

pub fn skeleton_shown(app: &PhoenixApp) -> bool {
    app.selected_channel
        .and_then(|c| app.messages.get(&c))
        .is_some_and(|e| !e.reached_start)
        && skeleton_in_view(&app.message_list)
}

fn anchor_above(heights: impl Iterator<Item = f32>, item: usize, y: f32) -> ListOffset {
    if y <= 0. {
        return ListOffset { item_ix: item, offset_in_item: px(-y) };
    }
    let mut left = y;
    let mut ix = item;
    for h in heights {
        if ix == 0 {
            break;
        }
        ix -= 1;
        if left <= h {
            return ListOffset { item_ix: ix, offset_in_item: px(h - left) };
        }
        left -= h;
    }
    ListOffset { item_ix: 0, offset_in_item: px(0.) }
}

pub fn keep_anchor(entity: Entity<PhoenixApp>, render: RenderRow) -> AnyElement {
    canvas(
        move |_, window, cx| {
            TOP_ROW.set(None);
            let Some(k) = KEEP.take() else { return };
            let (list, selected) = {
                let app = entity.read(cx);
                (app.message_list.clone(), app.selected_channel)
            };
            if selected != Some(k.channel) || k.item >= list.item_count() {
                return;
            }
            let width = list.viewport_bounds().size.width;
            if width <= px(0.) {
                return;
            }
            let space = size(AvailableSpace::Definite(width), AvailableSpace::MinContent);
            let heights = (0..k.item).rev().map(|ix| f32::from(render(&entity, ix, cx).layout_as_root(space, window, cx).height));
            let at = anchor_above(heights, k.item, k.y);
            list.scroll_to(at);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_0()
    .into_any_element()
}

pub fn load_when_visible(entity: Entity<PhoenixApp>) -> AnyElement {
    canvas(
        move |_, _, cx| {
            TOP_ROW_SHOWN.set(TOP_ROW.get());
            let app = entity.read(cx);
            let wanted = app
                .selected_channel
                .and_then(|c| app.messages.get(&c))
                .is_some_and(|e| e.loaded_once && !e.loading && !e.reached_start)
                && skeleton_in_view(&app.message_list);
            if !wanted {
                return;
            }
            let entity = entity.clone();
            cx.spawn(async move |cx| {
                entity.update(cx, |a, cx| a.load_older(cx));
            })
            .detach();
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_0()
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_lands_in_the_new_row_that_covers_the_old_top() {
        let a = anchor_above([100., 40., 60.].into_iter(), 4, 120.);
        assert_eq!((a.item_ix, f32::from(a.offset_in_item)), (2, 20.));
        let b = anchor_above([100.].into_iter(), 4, 50.);
        assert_eq!((b.item_ix, f32::from(b.offset_in_item)), (3, 50.));
        let c = anchor_above([10., 10.].into_iter(), 2, 500.);
        assert_eq!((c.item_ix, f32::from(c.offset_in_item)), (0, 0.));
        let d = anchor_above(std::iter::empty(), 3, -12.);
        assert_eq!((d.item_ix, f32::from(d.offset_in_item)), (3, 12.));
    }
}

use std::cell::Cell;

use gpui::{AnyElement, App, AvailableSpace, Bounds, Entity, IntoElement, ListOffset, ListState, Pixels, Styled, canvas, px, size};

use crate::{app::PhoenixApp, model::Id};

type RenderRow = fn(&Entity<PhoenixApp>, usize, &mut App) -> AnyElement;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Keep {
    channel: Id,
    item: usize,
    y: f32,
}

thread_local! {
    static KEEP: Cell<Option<Keep>> = const { Cell::new(None) };
    static TOP_ROW: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    static TOP_ROW_SHOWN: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
}

pub fn insert(list: &ListState, at: usize, n: usize) {
    list.splice(at..at, n);
    list.remeasure_items(0..0);
}

pub fn refresh(list: &ListState, ix: usize) {
    list.remeasure_items(ix..ix + 1);
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
    }
    list.remeasure_items(0..1);
    if n > 0 && n < len {
        list.remeasure_items(n + 1..n + 2);
    }
    KEEP.set(keep.map(|y| Keep { channel, item: n + 1, y }));
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

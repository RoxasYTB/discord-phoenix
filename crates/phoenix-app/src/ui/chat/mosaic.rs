//! Media mosaic: several image/video attachments of one message laid out in the
//! client's grids (`oneByTwoGrid_`, `twoByTwoGrid_`, `threeByThreeGrid_` …).
//!
//! Layouts per item count (vanilla pins-list dump + css `*_idVBIj`):
//! 2 → 1×2, 3 → solo + 2×1 column, 4 → 2×2, 5 → 1×2 + 3, 6 → 3×3 (6),
//! 7 → 1 + 3×3 (6), 8 → 1×2 + 3×3 (6), 9 → 3×3, 10 → 1 + 3×3 (9).

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, div, img, prelude::*};

use crate::{
    media,
    model::{Attachment, AttachmentKind},
    theme::{ca, z},
};

/// visualMediaItemContainer_ max-width 550px.
const MAX_W: f32 = 550.;
/// oneByTwoGrid_ / twoByTwoGrid_ / threeByThreeGrid_ grid-gap 4px.
const GAP: f32 = 4.;
/// visualMediaItemContainer_ border-radius 8px (outer corners).
const OUTER_R: f32 = 8.;
/// mosaicItemMediaMosaic_ border-radius 2px (inner corners).
const INNER_R: f32 = 2.;

/// Image or video attachment that takes part in the mosaic.
pub fn is_visual(a: &Attachment) -> bool {
    matches!(a.kind, AttachmentKind::Image | AttachmentKind::Video) && super::attach::loadable(&a.url)
}

#[derive(Clone, Copy, Default)]
struct Corners {
    tl: bool,
    tr: bool,
    bl: bool,
    br: bool,
}

fn radius(outer: bool) -> gpui::Pixels {
    z(if outer { OUTER_R } else { INNER_R })
}

fn cell(a: &Attachment, k: Corners, cx: &App) -> gpui::Div {
    let key = a.url.clone();
    let spoiler = a.is_spoiler() && !media::is_revealed(&key, cx);
    // Proxy asked for the largest cell size at 2x (the cell is at most 550×350).
    let src = if a.kind == AttachmentKind::Video { media::video_poster(&a.url, 1100, 700) } else { media::resized(&a.url, 1100, 700) };
    let viewer = media::ViewerImage {
        url: media::resized(&a.url, a.width.min(2048), a.height.min(2048)),
        original: if a.original.is_empty() { a.url.clone() } else { a.original.clone() },
        filename: a.filename.clone(),
        width: a.width,
        height: a.height,
        author: media::current_author(),
    };
    let video = a.kind == AttachmentKind::Video;
    let original = viewer.original.clone();
    let mut im = img(src.clone());
    if let Some(cache) = media::lru::chat() {
        im = im.image_cache(&cache);
    }
    // The id keeps the frame index of GIF / animated WebP uploads (media::anim_id).
    let im = im.id("frames");
    div()
        .relative()
        .overflow_hidden()
        .rounded_tl(radius(k.tl))
        .rounded_tr(radius(k.tr))
        .rounded_bl(radius(k.bl))
        .rounded_br(radius(k.br))
        // `imageContainer_` --background-mod-normal (base-lower = chat background, invisible).
        .bg(crate::ui::chat::tk(crate::theme::generated::idx::BACKGROUND_MOD_NORMAL))
        .child(
            div()
                .id(gpui::ElementId::Name(format!("mosaic-{src}").into()))
                .size_full()
                .cursor_pointer()
                .child(
                    // lazyImg_ object-fit cover
                    im.size_full()
                        .rounded_tl(radius(k.tl))
                        .rounded_tr(radius(k.tr))
                        .rounded_bl(radius(k.bl))
                        .rounded_br(radius(k.br))
                        .object_fit(gpui::ObjectFit::Cover),
                )
                .on_click(move |_, _, cx| {
                    if spoiler {
                        media::reveal(key.clone(), cx);
                    } else if video {
                        cx.open_url(&original);
                    } else {
                        media::open_viewer(viewer.clone(), cx);
                    }
                }),
        )
        .when(spoiler, |d| d.child(div().absolute().inset_0().bg(ca(0x000000E6))))
        .when(video, |d| {
            d.child(
                div().absolute().inset_0().flex().items_center().justify_center().child(
                    div()
                        .size(z(48.))
                        .rounded_full()
                        .bg(ca(0x000000B3))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(gpui::white())
                        .text_size(z(20.))
                        .child("\u{25B6}"),
                ),
            )
        })
}

/// Width/height of the tallest item (min aspect), so a row is as tall as Discord's.
fn row_aspect(items: &[&Attachment]) -> f32 {
    if items.iter().any(|a| a.width == 0 || a.height == 0) {
        return 1.;
    }
    items.iter().map(|a| a.width as f32 / a.height as f32).fold(f32::INFINITY, f32::min).clamp(0.25, 4.)
}

/// Row shapes (top to bottom) for `n` items.
fn rows(n: usize) -> Vec<Row> {
    use Row::*;
    match n {
        0 | 1 => vec![One],
        2 => vec![Two],
        3 => vec![Three],
        4 => vec![Four],
        5 => vec![Two, Grid(3)],
        6 => vec![Grid(6)],
        7 => vec![One, Grid(6)],
        8 => vec![Two, Grid(6)],
        9 => vec![Grid(9)],
        _ => vec![One, Grid(9)],
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Row {
    /// oneByOneGridMosaic_: full width, max-height 280px.
    One,
    /// oneByTwoGrid_: two oneByTwoGridItem_ (flex 1), max-height 280px.
    Two,
    /// oneByTwoLayoutThreeGrid_: oneByTwoSoloItem_ (flex 2) + twoByOneGrid_ column, max-height 350px.
    Three,
    /// twoByTwoGrid_: 2 columns × 2 rows, max-height 350px.
    Four,
    /// threeByThreeGrid_: 3 columns, square cells (aspect-ratio 1/1).
    Grid(usize),
}

impl Row {
    fn len(self) -> usize {
        match self {
            Row::One => 1,
            Row::Two => 2,
            Row::Three => 3,
            Row::Four => 4,
            Row::Grid(n) => n,
        }
    }
}

/// Mosaic for 2..=10 visual attachments (extra items beyond 10 are dropped, like the client's cap).
pub fn render(items: &[&Attachment], cx: &App) -> AnyElement {
    let items = &items[..items.len().min(10)];
    let shape = rows(items.len());
    let last_row = shape.len() - 1;
    let mut col = div().mt(z(4.)).w_full().max_w(z(MAX_W)).flex().flex_col().gap(z(GAP));
    let mut at = 0;
    for (ri, r) in shape.iter().copied().enumerate() {
        let its = &items[at..at + r.len()];
        at += r.len();
        let (top, bottom) = (ri == 0, ri == last_row);
        let el: AnyElement = match r {
            Row::One => {
                let k = Corners { tl: top, tr: top, bl: bottom, br: bottom };
                cell(its[0], k, cx).w_full().aspect_ratio(row_aspect(its)).max_h(z(280.)).into_any_element()
            }
            Row::Two => {
                let ar = row_aspect(its) * 2.;
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .gap(z(GAP))
                    .aspect_ratio(ar)
                    .max_h(z(280.))
                    .child(cell(its[0], Corners { tl: top, bl: bottom, ..Default::default() }, cx).flex_1().min_w_0().h_full())
                    .child(cell(its[1], Corners { tr: top, br: bottom, ..Default::default() }, cx).flex_1().min_w_0().h_full())
                    .into_any_element()
            }
            Row::Three => div()
                .w_full()
                .flex()
                .flex_row()
                .gap(z(GAP))
                .aspect_ratio(row_aspect(&its[..1]) * 1.5)
                .max_h(z(350.))
                .child({
                    let mut solo = cell(its[0], Corners { tl: true, bl: true, ..Default::default() }, cx).flex_basis(gpui::relative(0.)).min_w_0().h_full();
                    solo.style().flex_grow = Some(2.); // oneByTwoSoloItem_ flex 2
                    solo
                })
                .child(
                    // oneByTwoDuoItem_ > twoByOneGrid_: column, gap 4px, twoByOneGridItem_ flex 1.
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .gap(z(GAP))
                        .child(cell(its[1], Corners { tr: true, ..Default::default() }, cx).w_full().flex_1().min_h_0())
                        .child(cell(its[2], Corners { br: true, ..Default::default() }, cx).w_full().flex_1().min_h_0()),
                )
                .into_any_element(),
            Row::Four => div()
                .w_full()
                .grid()
                .grid_cols(2)
                .gap(z(GAP))
                .children(its.iter().enumerate().map(|(i, a)| {
                    let k = Corners { tl: i == 0, tr: i == 1, bl: i == 2, br: i == 3 };
                    // (350 - gap) / 2 per row
                    cell(a, k, cx).w_full().aspect_ratio(row_aspect(its)).max_h(z(173.))
                }))
                .into_any_element(),
            Row::Grid(n) => {
                let rows_n = n.div_ceil(3);
                div()
                    .w_full()
                    .grid()
                    .grid_cols(3)
                    .gap(z(GAP))
                    .children(its.iter().enumerate().map(|(i, a)| {
                        let (r, cc) = (i / 3, i % 3);
                        let k = Corners {
                            tl: top && r == 0 && cc == 0,
                            tr: top && r == 0 && cc == 2,
                            bl: bottom && r + 1 == rows_n && cc == 0,
                            br: bottom && r + 1 == rows_n && cc == 2,
                        };
                        cell(a, k, cx).w_full().aspect_ratio(1.)
                    }))
                    .into_any_element()
            }
        };
        col = col.child(el);
    }
    col.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_shapes_cover_every_item() {
        for n in 2..=10 {
            assert_eq!(rows(n).iter().map(|r| r.len()).sum::<usize>(), n, "{n} items");
        }
    }
}

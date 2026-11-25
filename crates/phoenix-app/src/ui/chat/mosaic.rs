use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, div, img, prelude::*};

use crate::{
    media,
    model::{Attachment, AttachmentKind},
    theme::{ca, z},
};

const MAX_W: f32 = 550.;
const GAP: f32 = 4.;
const OUTER_R: f32 = 8.;
const INNER_R: f32 = 2.;

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
    let alt = super::alt_text::description(a, spoiler);
    let group = super::attach::video_group(&a.url);
    let alt_key = a.url.clone();
    let original = viewer.original.clone();
    let mut im = img(src.clone());
    if let Some(cache) = media::lru::chat() {
        im = im.image_cache(&cache);
    }
    let im = im.id("frames");
    div()
        .relative()
        .when(video, |d| d.group(group.clone()))
        .overflow_hidden()
        .rounded_tl(radius(k.tl))
        .rounded_tr(radius(k.tr))
        .rounded_bl(radius(k.bl))
        .rounded_br(radius(k.br))
        .bg(crate::ui::chat::tk(crate::theme::generated::idx::BACKGROUND_MOD_NORMAL))
        .child(
            div()
                .id(gpui::ElementId::Name(format!("mosaic-{src}").into()))
                .size_full()
                .cursor_pointer()
                .child(
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
        .when(spoiler, |d| {
            let group: gpui::SharedString = format!("mosaic-spoiler-{}", a.url).into();
            d.group(group.clone())
                .bg(crate::ui::chat::tk(crate::theme::generated::idx::SPOILER_BACKGROUND_COLOR_HIDDEN))
                .child(img(crate::ui::chat::spoiler_blur(&a.url, video, (550., 350.))).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
                .child(div().absolute().inset_0().flex().items_center().justify_center().child(crate::ui::chat::spoiler_pill(group)))
        })
        .children(alt.map(|t| {
            if video { super::alt_text::video_badge(alt_key, t, group.clone()) } else { super::alt_text::badge(alt_key, t) }
        }))
        .when(video && !spoiler, |d| {
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

fn ratio_box(ar: f32, max_h: f32) -> gpui::Div {
    div().relative().w_full().max_h(z(max_h)).overflow_hidden().child(div().w_full().pt(gpui::relative(1. / ar.max(0.01))))
}

fn content_h((w, h): (u32, u32), max_w: f32, max_h: f32, cell_w: f32) -> f32 {
    if w == 0 || h == 0 {
        return cell_w.min(max_h);
    }
    let (wf, hf) = (w as f32, h as f32);
    let r = if w == h { 1. } else { (max_w / wf).max(max_h / hf).min(1.) };
    let g = (wf * r).round().min(max_w).max(1.);
    let t = (hf * r).round().min(max_h);
    cell_w * t / g
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Geom {
    content: f32,
    cap: f32,
}

impl Geom {
    fn height(self) -> f32 {
        self.content.min(self.cap)
    }
}

fn half(max_w: f32) -> f32 {
    ((max_w - GAP) / 2.).round()
}

fn two_thirds(max_w: f32) -> f32 {
    (2. * (max_w - GAP) / 3.).round()
}

fn third(max_w: f32) -> f32 {
    (max_w - 2. * GAP) / 3.
}

fn geom(r: Row, d: &[(u32, u32)], max_w: f32) -> Geom {
    match r {
        Row::One => Geom { content: content_h(d[0], max_w, 350., max_w), cap: 280. },
        Row::Two => {
            let s = half(max_w);
            Geom { content: d.iter().map(|x| content_h(*x, s, s, s)).fold(0., f32::max), cap: 280. }
        }
        Row::Three => {
            let s = two_thirds(max_w);
            let duo = s / 2.;
            let solo = content_h(d[0], s, 350., s);
            let col = content_h(d[1], duo, 173., duo) + GAP + content_h(d[2], duo, 173., duo);
            Geom { content: solo.max(col), cap: 350. }
        }
        Row::Four => {
            let s = half(max_w);
            Geom { content: d.iter().map(|x| content_h(*x, s, 173., s)).fold(0., f32::max), cap: (350. - GAP) / 2. }
        }
        Row::Grid(_) => Geom { content: third(max_w), cap: f32::INFINITY },
    }
}

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
    One,
    Two,
    Three,
    Four,
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

pub fn render(items: &[&Attachment], cx: &App) -> AnyElement {
    grid(items, MAX_W, cx).mt(z(4.)).into_any_element()
}

pub fn grid(items: &[&Attachment], max_w: f32, cx: &App) -> gpui::Div {
    let items = &items[..items.len().min(10)];
    let shape = rows(items.len());
    let last_row = shape.len() - 1;
    let mut col = div().w_full().max_w(z(max_w)).flex().flex_col().gap(z(GAP));
    let mut at = 0;
    for (ri, r) in shape.iter().copied().enumerate() {
        let its = &items[at..at + r.len()];
        at += r.len();
        let (top, bottom) = (ri == 0, ri == last_row);
        let d: Vec<(u32, u32)> = its.iter().map(|a| (a.width, a.height)).collect();
        let g = geom(r, &d, max_w);
        let el: AnyElement = match r {
            Row::One => {
                let k = Corners { tl: top, tr: top, bl: bottom, br: bottom };
                ratio_box(max_w / g.content, g.cap).child(cell(its[0], k, cx).absolute().inset_0()).into_any_element()
            }
            Row::Two => {
                ratio_box(max_w / g.content, g.cap)
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .flex_row()
                            .gap(z(GAP))
                            .child(cell(its[0], Corners { tl: top, bl: bottom, ..Default::default() }, cx).flex_1().min_w_0().h_full())
                            .child(cell(its[1], Corners { tr: top, br: bottom, ..Default::default() }, cx).flex_1().min_w_0().h_full()),
                    )
                    .into_any_element()
            }
            Row::Three => ratio_box(max_w / g.content, g.cap)
                .child(div().absolute().inset_0().flex().flex_row().gap(z(GAP)).child({
                    let mut solo = cell(its[0], Corners { tl: true, bl: true, ..Default::default() }, cx).flex_basis(gpui::relative(0.)).min_w_0().h_full();
                    solo.style().flex_grow = Some(2.);
                    solo
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .gap(z(GAP))
                        .child(cell(its[1], Corners { tr: true, ..Default::default() }, cx).w_full().flex_1().min_h_0())
                        .child(cell(its[2], Corners { br: true, ..Default::default() }, cx).w_full().flex_1().min_h_0()),
                ))
                .into_any_element(),
            Row::Four => div()
                .w_full()
                .grid()
                .grid_cols(2)
                .gap(z(GAP))
                .children(its.iter().enumerate().map(|(i, a)| {
                    let k = Corners { tl: i == 0, tr: i == 1, bl: i == 2, br: i == 3 };
                    cell(a, k, cx).w_full().aspect_ratio(half(max_w) / g.content).max_h(z(g.cap))
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
    col
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

    fn total(n: usize, d: (u32, u32), max_w: f32) -> f32 {
        let shape = rows(n);
        let mut h = 0.;
        let mut at = 0;
        for (i, r) in shape.iter().copied().enumerate() {
            let its = vec![d; r.len()];
            at += r.len();
            let g = geom(r, &its, max_w);
            let row = match r {
                Row::Four => 2. * g.height() + GAP,
                Row::Grid(k) => k.div_ceil(3) as f32 * g.height() + (k.div_ceil(3) - 1) as f32 * GAP,
                _ => g.height(),
            };
            h += row + if i > 0 { GAP } else { 0. };
        }
        assert_eq!(at, n);
        h
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.05
    }

    #[test]
    fn three_images_split_364_and_182_at_350_high() {
        assert_eq!(two_thirds(MAX_W), 364.);
        assert_eq!(two_thirds(MAX_W) / 2., 182.);
        assert_eq!(MAX_W - two_thirds(MAX_W) - GAP, 182.);
        for d in [(1920, 1080), (1080, 1920), (800, 800), (4000, 3000)] {
            assert_eq!(geom(Row::Three, &[d, d, d], MAX_W).height(), 350., "{d:?}");
        }
    }

    #[test]
    fn mosaic_heights_follow_the_client_layout() {
        let hd = (1920, 1080);
        let expected = [(2, 273.), (3, 350.), (4, 350.), (5, 273. + 4. + 542. / 3.), (6, 2. * 542. / 3. + 4.), (7, 280. + 4. + 2. * 542. / 3. + 4.), (8, 273. + 4. + 2. * 542. / 3. + 4.), (9, 550.), (10, 280. + 4. + 550.)];
        for (n, h) in expected {
            assert!(close(total(n, hd, MAX_W), h), "{n} images: {} vs {h}", total(n, hd, MAX_W));
        }
    }

    #[test]
    fn small_and_wide_images_stay_under_the_caps() {
        assert!(close(geom(Row::Two, &[(300, 100), (300, 100)], MAX_W).height(), 100.));
        assert!(close(geom(Row::Two, &[(300, 100), (100, 300)], MAX_W).height(), 280.));
        assert_eq!(geom(Row::One, &[(4000, 500)], MAX_W).height(), 280.);
        assert!(close(geom(Row::Four, &[(200, 100); 4], MAX_W).height(), 136.5));
        assert_eq!(geom(Row::Four, &[(64, 64); 4], MAX_W).height(), 173.);
        assert!(close(third(600.), 592. / 3.));
        assert_eq!(half(600.), 298.);
    }
}

use std::cell::RefCell;
use std::sync::Arc;

use gpui::{AnyElement, Context, FontWeight, InteractiveText, IntoElement, ParentElement, RenderImage, Styled, StyledText, div, img, prelude::*};

use crate::actions::{close_modal, with_modal};
use crate::app::PhoenixApp;
use crate::theme::{c, generated::idx, tok, z};
use crate::ui::controls::{BtnKind, button, button_icon};

use super::Modal;

pub const ART: &str = "https://discord.com/assets/ac8ab79ce1412299.gif";
const WIDTH: f32 = 480.;
const PAD: f32 = 32.;
const ART_W: f32 = 288.;
const ART_PX: u32 = 432;
const COLS: usize = 24;
const ROWS: usize = 12;

enum Art {
    Loading,
    Ready(Arc<RenderImage>),
    Failed,
}

thread_local! {
    static ART_IMG: RefCell<Option<Art>> = const { RefCell::new(None) };
}

pub fn open(cx: &mut Context<PhoenixApp>) {
    crate::actions::open_modal(Modal::BurstUpsell, None);
    cx.notify();
}

pub fn is_open() -> bool {
    with_modal(|m| matches!(m, Some(Modal::BurstUpsell)))
}

pub fn release_art() {
    if !is_open() {
        ART_IMG.with(|a| {
            if matches!(*a.borrow(), Some(Art::Ready(_)) | Some(Art::Failed)) {
                *a.borrow_mut() = None;
            }
        });
    }
}

fn decode(bytes: &[u8]) -> Option<RenderImage> {
    use image::AnimationDecoder;
    let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    let mut frames = Vec::new();
    for f in dec.into_frames() {
        let f = f.ok()?;
        let delay = f.delay();
        let buf = f.into_buffer();
        let h = ((buf.height() as u64 * ART_PX as u64) / buf.width().max(1) as u64).max(1) as u32;
        let mut buf = image::imageops::resize(&buf, ART_PX, h, image::imageops::FilterType::Triangle);
        for p in buf.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        frames.push(image::Frame::from_parts(buf, 0, 0, delay));
    }
    (!frames.is_empty()).then(|| RenderImage::new(frames))
}

fn art(cx: &mut Context<PhoenixApp>) -> Option<Arc<RenderImage>> {
    let state = ART_IMG.with(|a| match &*a.borrow() {
        Some(Art::Ready(i)) => Some(Some(i.clone())),
        Some(_) => Some(None),
        None => None,
    });
    if let Some(s) = state {
        return s;
    }
    ART_IMG.with(|a| *a.borrow_mut() = Some(Art::Loading));
    let http = cx.http_client();
    cx.spawn(async move |this, cx| {
        use futures::AsyncReadExt;
        let mut bytes = Vec::new();
        let ok = match http.get(ART, Default::default(), true).await {
            Ok(mut r) => r.body_mut().read_to_end(&mut bytes).await.is_ok(),
            Err(_) => false,
        };
        let img = if ok { cx.background_spawn(async move { decode(&bytes) }).await } else { None };
        ART_IMG.with(|a| *a.borrow_mut() = Some(img.map_or(Art::Failed, |i| Art::Ready(Arc::new(i)))));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
    None
}

pub fn mask_alpha(x: f32, y: f32) -> f32 {
    let d = ((x - 0.5).powi(2) + (y - 1.).powi(2)).sqrt();
    ((d - 0.6) / 0.4).clamp(0., 1.)
}

fn mix(a: gpui::Hsla, b: gpui::Hsla, t: f32) -> gpui::Hsla {
    let (a, b) = (gpui::Rgba::from(a), gpui::Rgba::from(b));
    let l = |x: f32, y: f32| x + (y - x) * t;
    gpui::Rgba { r: l(a.r, b.r), g: l(a.g, b.g), b: l(a.b, b.b), a: l(a.a, b.a) }.into()
}

fn wash() -> AnyElement {
    let start = c(tok(idx::EXPRESSIVE_GRADIENT_NITRO_PINK_START));
    let end = c(tok(idx::EXPRESSIVE_GRADIENT_NITRO_PINK_END));
    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .children((0..ROWS).map(|r| {
            let y = (r as f32 + 0.5) / ROWS as f32;
            div().flex_1().w_full().flex().children((0..COLS).map(move |k| {
                let x = (k as f32 + 0.5) / COLS as f32;
                let col = mix(start, end, x);
                div().flex_1().h_full().bg(col.opacity(mask_alpha(x, y)))
            }))
        }))
        .into_any_element()
}

fn subtitle(e: gpui::Entity<PhoenixApp>) -> AnyElement {
    let before = crate::t!("burst.upsell_sub");
    let link = crate::t!("burst.upsell_link");
    let after = crate::t!("burst.upsell_end");
    let start = before.len();
    let full = format!("{before}{link}{after}");
    let range = start..start + link.len();
    let style = gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() };
    let styled = StyledText::new(full).with_highlights(vec![(range.clone(), style)]);
    InteractiveText::new("burst-upsell-sub", styled)
        .on_click(vec![range], move |_, _, cx| {
            e.update(cx, |a, cx| {
                close_modal();
                open_nitro(a, cx);
            })
        })
        .into_any_element()
}

pub fn open_nitro(a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    crate::ui::settings::open(a, cx);
    if let Some(s) = a.settings.as_mut() {
        s.page = crate::ui::settings::Page::Nitro;
    }
    cx.notify();
}

pub fn render(cx: &mut Context<PhoenixApp>) -> AnyElement {
    let close = super::close_action(cx);
    let (e_get, e_sub) = (cx.entity(), cx.entity());
    let close_x = close.clone();
    let close_no = close.clone();
    let art_h = ART_W * 720. / 1280.;
    let graphic = div().w_full().flex().justify_center().pt(z(20.)).pb(z(16.)).child(
        div().w_full().max_w(z(ART_W)).h(z(art_h)).flex_none().map(|d| match art(cx) {
            Some(i) => d.child(img(i).id("burst-upsell-art").size_full()),
            None => d,
        }),
    );
    let header = div()
        .relative()
        .px(z(PAD))
        .pt(z(16.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .items_center()
        .gap(z(8.))
        .child(graphic)
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(z(24.))
                .line_height(z(32.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(crate::t!("burst.upsell_title")),
        )
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_SUBTLE)))
                .child(subtitle(e_sub)),
        );
    let actions = div()
        .px(z(PAD))
        .pt(z(16.))
        .pb(z(32.))
        .flex()
        .gap(z(8.))
        .child(
            div().flex_1().flex().child(
                button("burst-upsell-no", crate::t!("burst.no_thanks"), BtnKind::Secondary)
                    .flex_1()
                    .on_click(move |_, w, cx| close_no(w, cx)),
            ),
        )
        .child(
            div().flex_1().flex().child(
                button_icon("burst-upsell-get", "nitro", crate::t!("burst.get_nitro"), BtnKind::Expressive)
                    .flex_1()
                    .on_click(move |_, _, cx| {
                        e_get.update(cx, |a, cx| {
                            close_modal();
                            open_nitro(a, cx);
                        })
                    }),
            ),
        );
    let card = div()
        .id("burst-upsell")
        .relative()
        .w(z(WIDTH))
        .max_w_full()
        .rounded(z(12.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .relative()
                .pt(z(8.))
                .child(wash())
                .child(header),
        )
        .child(actions)
        .child(
            div()
                .id("burst-upsell-close")
                .absolute()
                .top(z(8.))
                .right(z(8.))
                .size(z(40.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                .on_click(move |_, w, cx| close_x(w, cx))
                .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT)))),
        );
    crate::ui::modal::frame("burst-upsell-backdrop", close, card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wash_fades_in_towards_the_top_corners() {
        assert_eq!(mask_alpha(0.5, 1.), 0.);
        assert_eq!(mask_alpha(0.5, 0.45), 0.);
        assert!(mask_alpha(0., 0.) > 0.9);
        assert!(mask_alpha(0.5, 0.) > 0.9);
    }
}

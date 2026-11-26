use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, FontWeight, GlobalElementId, Hsla, InspectorElementId, IntoElement,
    LayoutId, ParentElement, Pixels, SharedString, Styled, Window, div, point, prelude::*, size,
};

use crate::theme::{Palette, c, z};

pub const STROKE: f32 = 2.;
pub const ICON_BADGE: f32 = 16.;
pub const ICON_BADGE_GLYPH: f32 = 12.;
pub const COUNT_HEIGHT: f32 = 16.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hole {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
}

impl Hole {
    pub fn circle(cx: f32, cy: f32, r: f32) -> Self {
        Hole { x: cx - r, y: cy - r, w: 2. * r, h: 2. * r, r }
    }

    pub fn distance(&self, px: f32, py: f32) -> f32 {
        let (hw, hh) = (self.w / 2., self.h / 2.);
        let (rx, ry) = (self.r.min(hw).max(0.), self.r.min(hh).max(0.));
        let qx = (px - self.x - hw).abs() - hw + rx;
        let qy = (py - self.y - hh).abs() - hh + ry;
        if qx > 0. && qy > 0. && rx > 0. && ry > 0. {
            return (((qx / rx).powi(2) + (qy / ry).powi(2)).sqrt() - 1.) * rx.min(ry);
        }
        if qx > 0. && qy > 0. {
            return ((qx - rx).max(0.).powi(2) + (qy - ry).max(0.).powi(2)).sqrt();
        }
        (qx - rx).max(qy - ry)
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        self.distance(px, py) < 0.
    }
}

pub fn count_width(n: u32) -> f32 {
    match n {
        0..=9 => 16.,
        10..=99 => 22.,
        _ => 30.,
    }
}

pub fn count_label(n: u32) -> SharedString {
    if n < 1000 { n.to_string().into() } else { format!("{}k+", (n / 1000).min(9)).into() }
}

pub fn mask_size(view: f32) -> f32 {
    view / 2.
}

pub fn upper_hole(view: f32, folder: bool) -> Hole {
    let u = if folder { 20. } else { mask_size(view) };
    Hole { x: view - u + STROKE, y: -STROKE, w: u, h: u, r: u / 2. }
}

pub fn lower_hole(view: f32, w: f32, h: f32) -> Hole {
    let (hw, hh) = (w + 2. * STROKE, h + 2. * STROKE);
    Hole { x: view - hw + STROKE, y: view - hh + STROKE, w: hw, h: hh, r: mask_size(view) / 2. }
}

pub fn status_hole(size: f32) -> Hole {
    let (cx, r, _) = crate::ui::status_svg::geometry(size);
    Hole::circle(cx, cx, r)
}

pub struct Cut {
    child: AnyElement,
    holes: Vec<Hole>,
}

pub fn cut(child: impl IntoElement, holes: impl IntoIterator<Item = Hole>) -> Cut {
    Cut { child: child.into_any_element(), holes: holes.into_iter().collect() }
}

impl IntoElement for Cut {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Cut {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.holes.is_empty() {
            self.child.paint(window, cx);
            return;
        }
        let holes: Vec<(Bounds<Pixels>, Pixels)> = self
            .holes
            .iter()
            .map(|h| {
                (Bounds::new(point(bounds.origin.x + z(h.x), bounds.origin.y + z(h.y)), size(z(h.w), z(h.h))), z(h.r))
            })
            .collect();
        window.with_cutouts(&holes, |window| self.child.paint(window, cx));
    }
}

pub fn icon_badge(icon: &'static str, bg: Hsla) -> Div {
    div()
        .size(z(ICON_BADGE))
        .rounded_full()
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(ICON_BADGE_GLYPH)).text_color(gpui::white()))
}

pub fn count_badge(n: u32) -> Div {
    div()
        .h(z(COUNT_HEIGHT))
        .w(z(count_width(n)))
        .flex_none()
        .rounded_full()
        .bg(c(Palette::NOTIFICATION))
        .text_color(gpui::white())
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .flex()
        .items_center()
        .justify_center()
        .child(count_label(n))
}

pub fn masked(view: f32, folder: bool, icon: impl IntoElement, upper: Option<AnyElement>, lower: Option<u32>) -> Div {
    let mut holes: Vec<Hole> = Vec::new();
    if upper.is_some() {
        holes.push(upper_hole(view, folder));
    }
    if let Some(n) = lower {
        holes.push(lower_hole(view, count_width(n), COUNT_HEIGHT));
    }
    div()
        .relative()
        .flex_none()
        .size(z(view))
        .child(cut(icon, holes))
        .when_some(lower, |d, n| d.child(div().absolute().right_0().bottom_0().child(count_badge(n))))
        .when_some(upper, |d, b| d.child(div().absolute().right_0().top_0().child(b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guild_upper_hole_is_the_client_rect() {
        assert_eq!(upper_hole(40., false), Hole { x: 22., y: -2., w: 20., h: 20., r: 10. });
    }

    #[test]
    fn folder_upper_hole_keeps_twenty_pixels() {
        assert_eq!(upper_hole(48., true), Hole { x: 30., y: -2., w: 20., h: 20., r: 10. });
    }

    #[test]
    fn lower_hole_grows_left_with_the_count_width() {
        assert_eq!(lower_hole(40., count_width(1), 16.), Hole { x: 22., y: 22., w: 20., h: 20., r: 10. });
        assert_eq!(lower_hole(40., count_width(12), 16.), Hole { x: 16., y: 22., w: 26., h: 20., r: 10. });
        assert_eq!(lower_hole(40., count_width(150), 16.), Hole { x: 8., y: 22., w: 34., h: 20., r: 10. });
        assert_eq!(lower_hole(48., count_width(3), 16.), Hole { x: 30., y: 30., w: 20., h: 20., r: 12. });
    }

    #[test]
    fn folder_count_hole_has_the_svg_twelve_by_ten_corners() {
        let h = lower_hole(48., count_width(12), 16.);
        assert_eq!(h, Hole { x: 24., y: 30., w: 26., h: 20., r: 12. });
        let (cx, cy) = (h.x + 12. - 12. * std::f32::consts::FRAC_1_SQRT_2, h.y + 10. - 10. * std::f32::consts::FRAC_1_SQRT_2);
        assert!(h.distance(cx, cy).abs() < 1e-4);
        assert!(h.distance(h.x, h.y + 10.).abs() < 1e-4);
        assert!(h.distance(h.x + 13., h.y).abs() < 1e-4);
        let circle = Hole { r: 10., ..h };
        assert!(circle.distance(h.x + 1.2, h.y + 1.2) < h.distance(h.x + 1.2, h.y + 1.2));
    }

    #[test]
    fn count_badge_widths_and_labels_follow_the_client() {
        assert_eq!([1, 9, 10, 99, 100, 999, 5000].map(count_width), [16., 16., 22., 22., 30., 30., 30.]);
        assert_eq!(count_label(7).as_ref(), "7");
        assert_eq!(count_label(120).as_ref(), "120");
        assert_eq!(count_label(1500).as_ref(), "1k+");
        assert_eq!(count_label(42_000).as_ref(), "9k+");
    }

    #[test]
    fn the_ring_between_icon_and_badge_is_two_pixels() {
        let upper = upper_hole(40., false);
        let badge_center = (40. - ICON_BADGE / 2., ICON_BADGE / 2.);
        assert!((upper.distance(badge_center.0, badge_center.1) + ICON_BADGE / 2. + STROKE).abs() < 1e-4);
        assert!(upper.contains(40. - ICON_BADGE - 1., 8.));
        assert!(!upper.contains(40. - ICON_BADGE - STROKE - 0.1, 8.));
        let lower = lower_hole(40., 22., 16.);
        assert!(lower.contains(40. - 22. - 1.9, 32.));
        assert!(!lower.contains(40. - 22. - 2.1, 32.));
        assert!(!lower.contains(10., 10.));
    }

    #[test]
    fn status_hole_wraps_the_dot_with_its_ring() {
        let h = status_hole(32.);
        let (cx, r, s) = crate::ui::status_svg::geometry(32.);
        assert_eq!(h, Hole { x: cx - r, y: cx - r, w: 2. * r, h: 2. * r, r });
        assert!(r > s / 2.);
        assert!(h.contains(cx, cx));
        assert!(!h.contains(0., 0.));
    }
}

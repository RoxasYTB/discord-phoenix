use crate::theme::z;
use gpui::{AnyElement, CubicBezier, ElementId, IntoElement, PresentTransitionExt, Transition, point, px};
use std::time::{Duration, Instant};

pub const EASE_OUT: CubicBezier = CubicBezier(0.0, 0.0, 0.58, 1.0);
pub const RENDER_DELAY: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    None,
    Translate,
    Scale,
    Fade,
}

pub fn kind_for(kind: Kind, reduced: bool) -> Kind {
    if reduced && kind != Kind::None { Kind::Fade } else { kind }
}

pub fn translate_from(side: Side) -> (f32, f32) {
    match side {
        Side::Top | Side::Center => (0., -10.),
        Side::Bottom => (0., 10.),
        Side::Left => (-10., 0.),
        Side::Right => (10., 0.),
    }
}

pub fn scale_origin(side: Side) -> (f32, f32) {
    match side {
        Side::Top => (0.5, 1.),
        Side::Bottom | Side::Right => (0.5, 0.),
        Side::Left => (1., 0.),
        Side::Center => (0.5, 0.5),
    }
}

pub fn transition(kind: Kind, side: Side, reduced: bool) -> Option<Transition> {
    match kind_for(kind, reduced) {
        Kind::None => None,
        Kind::Fade => Some(Transition::ease(Duration::from_millis(80), EASE_OUT).delay(RENDER_DELAY).opacity(0., 1.)),
        Kind::Translate => {
            let (x, y) = translate_from(side);
            Some(Transition::ease(Duration::from_millis(200), EASE_OUT).delay(RENDER_DELAY).translate(point(z(x), z(y)), point(px(0.), px(0.))))
        }
        Kind::Scale => {
            let (ox, oy) = scale_origin(side);
            Some(Transition::ease(Duration::from_millis(120), EASE_OUT).delay(RENDER_DELAY).origin(ox, oy).scale(0.01, 1.).opacity(0., 1.))
        }
    }
}

pub fn popout(id: impl Into<ElementId>, kind: Kind, side: Side, el: impl IntoElement + 'static) -> AnyElement {
    let t = transition(kind, side, crate::ui::settings::effects::reduced_motion());
    el.present_transition(id, t).into_any_element()
}

pub fn side_of(anchor: f32, popout_start: f32) -> Side {
    if popout_start < anchor { Side::Left } else { Side::Right }
}

pub const MODAL_ENTER: gpui::SpringConfig = gpui::SpringConfig::new(1000., 48., 1.);
pub const MODAL_EXIT: gpui::SpringConfig = gpui::SpringConfig::new(1200., 80., 1.);
pub const MODAL_DELAY: Duration = Duration::from_millis(64);

pub fn modal_enter(reduced: bool) -> Transition {
    let t = Transition::spring(MODAL_ENTER).delay(MODAL_DELAY).opacity(0., 1.);
    if reduced { t } else { t.scale(0.9, 1.) }
}

pub fn modal_exit(reduced: bool) -> Transition {
    let t = Transition::spring(MODAL_EXIT).opacity(1., 0.);
    if reduced { t } else { t.scale(1., 0.9) }
}

pub fn backdrop_enter() -> Transition {
    Transition::spring(MODAL_ENTER).opacity(0., 1.)
}

pub fn backdrop_exit() -> Transition {
    Transition::spring(MODAL_ENTER).opacity(1., 0.)
}

pub fn modal_card(id: &'static str, el: impl IntoElement + 'static) -> AnyElement {
    let reduced = crate::ui::settings::effects::reduced_motion();
    el.present_transition(ElementId::NamedInteger("modal-card-pt".into(), crate::ui::states::key((id, "card"))), Some(modal_enter(reduced)))
        .on_exit(crate::ui::states::key((id, "card")), modal_exit(reduced))
        .into_any_element()
}

pub fn modal_backdrop(id: &'static str, el: impl IntoElement + 'static) -> AnyElement {
    el.present_transition(ElementId::NamedInteger("modal-backdrop-pt".into(), crate::ui::states::key((id, "backdrop"))), Some(backdrop_enter()))
        .on_exit(crate::ui::states::key((id, "backdrop")), backdrop_exit())
        .into_any_element()
}

pub const LAYER_SPRING: gpui::SpringConfig = gpui::SpringConfig::new(447.4, 31., 1.);

pub fn origami(tension: f32, friction: f32) -> gpui::SpringConfig {
    gpui::SpringConfig::new((tension - 30.) * 3.62 + 194., (friction - 8.) * 3. + 25., 1.)
}

pub fn layer_enter(reduced: bool) -> Transition {
    let t = Transition::spring(LAYER_SPRING).opacity(0., 1.);
    if reduced { t } else { t.scale(1.1, 1.) }
}

pub fn layer_exit(reduced: bool) -> Transition {
    let t = Transition::spring(LAYER_SPRING).opacity(1., 0.);
    if reduced { t } else { t.scale(1., 1.1) }
}

thread_local! {
    static LAYER: std::cell::Cell<Option<(bool, Instant)>> = const { std::cell::Cell::new(None) };
}

pub fn full_layer_open() -> bool {
    crate::actions::with_modal(|m| matches!(m, Some(crate::actions::Modal::ServerSettings(_) | crate::actions::Modal::ChannelSettings(_))))
}

pub fn layer_change(open: bool) -> (bool, Instant) {
    LAYER.with(|l| {
        let next = match l.get() {
            Some((was, at)) if was == open => (was, at),
            Some(_) => (open, Instant::now()),
            None => (open, Instant::now() - Duration::from_secs(10)),
        };
        l.set(Some(next));
        next
    })
}

pub fn base(el: impl IntoElement + 'static) -> AnyElement {
    let (open, at) = layer_change(full_layer_open());
    el.present_transition("base-layer", base_layer(open, at, crate::ui::settings::effects::reduced_motion())).into_any_element()
}

pub fn layer(id: &'static str, el: impl IntoElement + 'static) -> AnyElement {
    let reduced = crate::ui::settings::effects::reduced_motion();
    let key = crate::ui::states::key((id, "layer"));
    el.present_transition(ElementId::NamedInteger("layer-pt".into(), key), Some(layer_enter(reduced))).on_exit(key, layer_exit(reduced)).into_any_element()
}

pub fn base_layer(open: bool, at: Instant, reduced: bool) -> Option<Transition> {
    let t = Transition::spring(LAYER_SPRING).start(at);
    let (s, o) = if open { ((1., 0.93), (1., 0.)) } else { ((0.93, 1.), (0., 1.)) };
    let t = t.opacity(o.0, o.1);
    let t = if reduced { t } else { t.scale(s.0, s.1) };
    let settled = at.elapsed().as_secs_f32() > gpui::spring_settle_time(LAYER_SPRING);
    (open || !settled).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popouts_slide_10px_toward_their_anchor_like_the_client_animator() {
        assert_eq!(translate_from(Side::Top), (0., -10.));
        assert_eq!(translate_from(Side::Bottom), (0., 10.));
        assert_eq!(translate_from(Side::Left), (-10., 0.));
        assert_eq!(translate_from(Side::Right), (10., 0.));
        assert_eq!(translate_from(Side::Center), (0., -10.));
        let t = transition(Kind::Translate, Side::Bottom, false).unwrap();
        assert_eq!(t.delay, RENDER_DELAY);
        assert_eq!(t.timing, gpui::TransitionTiming::Ease { duration: Duration::from_millis(200), easing: EASE_OUT });
        assert_eq!(t.opacity, (1., 1.));
    }

    #[test]
    fn reduced_motion_fades_in_80ms_and_none_stays_still() {
        let t = transition(Kind::Translate, Side::Top, true).unwrap();
        assert_eq!(t.timing, gpui::TransitionTiming::Ease { duration: Duration::from_millis(80), easing: EASE_OUT });
        assert_eq!(t.opacity, (0., 1.));
        assert_eq!(t.translate.0, point(px(0.), px(0.)));
        assert!(transition(Kind::None, Side::Top, false).is_none());
        assert!(transition(Kind::None, Side::Top, true).is_none());
    }

    #[test]
    fn scale_grows_from_the_side_facing_the_anchor() {
        assert_eq!(scale_origin(Side::Top), (0.5, 1.));
        assert_eq!(scale_origin(Side::Bottom), (0.5, 0.));
        assert_eq!(scale_origin(Side::Left), (1., 0.));
        assert_eq!(scale_origin(Side::Right), (0.5, 0.));
        let t = transition(Kind::Scale, Side::Top, false).unwrap();
        assert_eq!((t.scale, t.opacity), ((0.01, 1.), (0., 1.)));
    }

    #[test]
    fn modals_spring_in_after_64ms_and_spring_out_like_the_client_modal_root() {
        let enter = modal_enter(false);
        assert_eq!((enter.scale, enter.opacity, enter.delay), ((0.9, 1.), (0., 1.), MODAL_DELAY));
        assert_eq!(enter.timing, gpui::TransitionTiming::Spring(MODAL_ENTER));
        let exit = modal_exit(false);
        assert_eq!((exit.scale, exit.opacity), ((1., 0.9), (1., 0.)));
        assert_eq!(modal_exit(true).scale, (1., 1.));
    }

    #[test]
    fn the_settings_layer_uses_the_origami_spring_of_the_client_layers() {
        let o = origami(100., 10.);
        assert!((o.stiffness - LAYER_SPRING.stiffness).abs() < 1e-3 && o.damping == LAYER_SPRING.damping);
        assert_eq!(layer_enter(false).scale, (1.1, 1.));
        assert_eq!(layer_exit(false).scale, (1., 1.1));
        let at = Instant::now();
        assert_eq!(base_layer(true, at, false).unwrap().scale, (1., 0.93));
        assert!(base_layer(false, at - Duration::from_secs(2), false).is_none());
        assert!(base_layer(true, at - Duration::from_secs(2), false).is_some());
    }
}

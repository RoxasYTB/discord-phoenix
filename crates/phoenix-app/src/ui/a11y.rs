use std::cell::RefCell;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, InteractiveElement, Interactivity,
    IntoElement, LayoutId, ParentElement, Pixels, Role, SharedString, Styled, Window, accesskit, div, prelude::*, px,
};

pub struct Live<E> {
    inner: E,
    live: accesskit::Live,
}

pub fn live<E: Element>(inner: E, live: accesskit::Live) -> Live<E> {
    Live { inner, live }
}

impl<E: Element> IntoElement for Live<E> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<E: InteractiveElement> InteractiveElement for Live<E> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.inner.interactivity()
    }
}

impl<E: Element> Element for Live<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.inner.id()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.inner.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.inner.prepaint(id, inspector, bounds, layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(id, inspector, bounds, layout, prepaint, window, cx)
    }

    fn a11y_role(&self) -> Option<Role> {
        self.inner.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        self.inner.write_a11y_info(node);
        node.set_live(self.live);
    }

    fn a11y_synthetic_children(&mut self, prepaint: &mut Self::PrepaintState, builder: &mut gpui::A11ySubtreeBuilder) {
        self.inner.a11y_synthetic_children(prepaint, builder);
    }
}

#[derive(Default)]
struct Announcements {
    polite: Option<SharedString>,
    assertive: Option<SharedString>,
}

thread_local! {
    static ANNOUNCE: RefCell<Announcements> = RefCell::default();
}

pub fn announce(text: SharedString, assertive: bool) {
    ANNOUNCE.with(|a| {
        let mut a = a.borrow_mut();
        if assertive {
            a.assertive = Some(text);
        } else {
            a.polite = Some(text);
        }
    });
}

pub fn announced() -> (Option<SharedString>, Option<SharedString>) {
    ANNOUNCE.with(|a| {
        let a = a.borrow();
        (a.polite.clone(), a.assertive.clone())
    })
}

fn region(id: &'static str, text: Option<SharedString>, how: accesskit::Live) -> AnyElement {
    let el = div().id(id).absolute().size(px(0.)).overflow_hidden().role(Role::Log);
    let el = match text {
        Some(t) => el.aria_label(t.clone()).child(div().id((id, 1usize)).role(Role::Label).aria_label(t)),
        None => el,
    };
    live(el, how).into_any_element()
}

pub fn announcer() -> impl IntoElement {
    let (polite, assertive) = announced();
    div()
        .absolute()
        .size(px(0.))
        .child(region("a11y-assertive", assertive, accesskit::Live::Assertive))
        .child(region("a11y-polite", polite, accesskit::Live::Polite))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_of(element: impl IntoElement) -> accesskit::Node {
        let element = element.into_element();
        let mut node = accesskit::Node::new(element.a11y_role().unwrap_or(Role::Unknown));
        element.write_a11y_info(&mut node);
        node
    }

    #[test]
    fn live_regions_reach_accesskit() {
        let node = node_of(live(div().id("x").role(Role::Log).aria_label("message deleted"), accesskit::Live::Polite));
        assert_eq!(node.role(), Role::Log);
        assert_eq!(node.live(), Some(accesskit::Live::Polite));
        assert_eq!(node.label(), Some("message deleted"));
    }

    #[test]
    fn announcements_keep_the_latest_text_per_politeness() {
        announce("a".into(), false);
        announce("b".into(), true);
        announce("c".into(), false);
        assert_eq!(announced(), (Some("c".into()), Some("b".into())));
    }
}

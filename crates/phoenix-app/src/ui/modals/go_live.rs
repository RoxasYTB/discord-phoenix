use std::cell::Cell;
use std::rc::Rc;

use gpui::{AnyElement, Bounds, Context, FontWeight, IntoElement, ParentElement, Pixels, SharedString, Size, Styled, Window, div, point, prelude::*, px, svg};

use super::Modal;
use crate::actions::{close_modal, open_modal, with_modal};
use crate::app::PhoenixApp;
use crate::theme::{c, generated::idx, tok, z};
use crate::ui::context_menu::MenuItem;
use crate::ui::controls::{BtnKind, button};
use crate::ui::tooltip::Tipped as _;
use crate::voice::stream::{self, Preset, Quality, SourceKind};

#[derive(Debug, Clone, Default)]
pub struct State {
    pub devices_tab: bool,
    pub kind: Option<SourceKind>,
    pub menu: bool,
    pub viewport: Option<Size<Pixels>>,
}

thread_local! {
    static COG: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
}

pub const WIDTH: f32 = 960.;
pub const HEIGHT: f32 = 570.;

pub fn open(window: Option<&mut Window>) {
    open_modal(Modal::GoLive(State::default()), window);
}

fn update(cx: &mut Context<PhoenixApp>, f: impl FnOnce(&mut State)) {
    with_modal(|m| {
        if let Some(Modal::GoLive(s)) = m {
            f(s);
        }
    });
    cx.notify();
}

fn t(i: usize) -> gpui::Hsla {
    c(tok(i))
}

pub fn height(viewport_h: f32) -> f32 {
    if viewport_h <= 820. { HEIGHT.min(viewport_h - 90.).min(550.) } else { HEIGHT }
}

fn tab(id: &'static str, icon: &'static str, label: SharedString, selected: bool, devices: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let fg = if selected { t(idx::INTERACTIVE_TEXT_ACTIVE) } else { t(idx::TEXT_SUBTLE) };
    div()
        .id(id)
        .flex_1()
        .flex_basis(px(0.))
        .px(z(16.))
        .py(z(8.))
        .rounded(z(8.))
        .flex()
        .justify_center()
        .cursor_pointer()
        .when(selected, |d| d.bg(t(idx::BACKGROUND_SURFACE_HIGH)))
        .when(!selected, |d| d.hover(|s| s.bg(t(idx::BACKGROUND_MOD_SUBTLE))))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .text_size(z(14.))
                .line_height(z(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg)
                .child(svg().path(crate::icons::path(icon)).flex_none().size(z(16.)).text_color(t(idx::INTERACTIVE_ICON_DEFAULT)))
                .child(label),
        )
        .on_click(cx.listener(move |_, _, _, cx| update(cx, |s| s.devices_tab = devices)))
        .into_any_element()
}

fn source_card(id: &'static str, icon: &'static str, label: SharedString, kind: SourceKind, selected: bool, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (bg, fg) = if selected {
        (t(idx::INTERACTIVE_BACKGROUND_ACTIVE), t(idx::INTERACTIVE_TEXT_ACTIVE))
    } else {
        (t(idx::BACKGROUND_BASE_LOWER), t(idx::INTERACTIVE_TEXT_DEFAULT))
    };
    div()
        .id(id)
        .group(id)
        .w(z(227.6))
        .h(z(165.6))
        .p(z(32.))
        .rounded(z(4.))
        .border_1()
        .border_color(if selected { t(idx::BRAND_500) } else { t(idx::BORDER_SUBTLE) })
        .bg(bg)
        .text_color(fg)
        .when(!selected, |d| d.hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER)).text_color(t(idx::INTERACTIVE_TEXT_HOVER))))
        .active(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_ACTIVE)).text_color(t(idx::INTERACTIVE_TEXT_ACTIVE)))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size_full()
                .p(z(16.))
                .flex()
                .flex_col()
                .items_center()
                .justify_around()
                .child(svg().path(crate::icons::path(icon)).flex_none().size(z(32.)).text_color(t(idx::INTERACTIVE_ICON_DEFAULT)))
                .child(div().max_w(z(130.)).text_center().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).child(label)),
        )
        .on_click(cx.listener(move |_, _, _, cx| update(cx, |s| s.kind = Some(kind))))
        .into_any_element()
}

pub fn preset_name(q: Quality) -> SharedString {
    match q.preset {
        Preset::Video => crate::t!("golive.gaming"),
        Preset::Documents => crate::t!("golive.docs"),
        Preset::Custom => crate::t!("golive.custom"),
    }
}

pub fn resolution_label(r: u32) -> SharedString {
    if r == 0 { crate::t!("golive.source") } else { format!("{r}p").into() }
}

fn summary(app: &PhoenixApp) -> AnyElement {
    let q = crate::ui::go_live::quality();
    let (r, f) = stream::resolve(q, crate::ui::go_live::perks(app));
    let dot = || div().text_color(t(idx::BACKGROUND_MOD_SUBTLE)).child("•");
    let mut detail = div().flex().items_center().gap(z(8.)).text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(t(idx::TEXT_MUTED));
    detail = detail.child(dot()).child(resolution_label(r)).child(dot()).child(format!("{f}fps"));
    if q.sound {
        detail = detail.child(dot()).child(crate::t!("golive.system_audio"));
    }
    div()
        .flex()
        .items_center()
        .gap(z(16.))
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_col()
                .justify_center()
                .gap(z(2.))
                .min_w_0()
                .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_STRONG)).text_ellipsis().child(preset_name(q)))
                .child(detail),
        )
        .into_any_element()
}

fn group_label(text: SharedString) -> AnyElement {
    div().p(z(8.)).text_size(z(14.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(t(idx::TEXT_MUTED)).child(text).into_any_element()
}

pub fn settings_items(app: &PhoenixApp, entity: gpui::Entity<PhoenixApp>) -> Vec<MenuItem> {
    let q = crate::ui::go_live::quality();
    let perks = crate::ui::go_live::perks(app);
    let set = move |entity: &gpui::Entity<PhoenixApp>, cx: &mut gpui::App, q: Quality| {
        crate::ui::go_live::set_quality(q);
        entity.update(cx, |_, cx| cx.notify());
    };
    let set = Rc::new(set);
    let preset_sub = |p: Preset| {
        let &(r, f) = stream::preset_choices(p).iter().find(|&&(r, f)| stream::allowed(perks, r, f, p == Preset::Documents)).unwrap_or(&(720, 30));
        let key = if p == Preset::Video { "golive.gaming_d" } else { "golive.docs_d" };
        crate::t!(key, res = resolution_label(r), fps = f)
    };
    let radio = |label: SharedString, p: Preset| {
        let (e, set) = (entity.clone(), set.clone());
        MenuItem::radio(label, q.preset == p, move |_, _, cx| set(&e, cx, Quality { preset: p, ..q }))
    };
    let mut res_items = Vec::new();
    for r in stream::RESOLUTIONS {
        let fps = if q.preset == Preset::Documents { 30 } else { q.fps };
        let label = resolution_label(r);
        if stream::allowed(perks, r, fps, false) {
            let (e, set) = (entity.clone(), set.clone());
            let checked = q.preset == Preset::Custom && q.resolution == r;
            res_items.push(MenuItem::radio(label, checked, move |_, _, cx| set(&e, cx, Quality { preset: Preset::Custom, resolution: r, ..q })));
        } else {
            res_items.push(MenuItem::disabled(label, None));
        }
    }
    let mut fps_items = Vec::new();
    for f in stream::FRAME_RATES {
        let label: SharedString = crate::t!("golive.fps_n", value = f);
        if stream::allowed(perks, q.resolution, f, false) {
            let (e, set) = (entity.clone(), set.clone());
            let checked = q.preset == Preset::Custom && q.fps == f;
            fps_items.push(MenuItem::radio(label, checked, move |_, _, cx| set(&e, cx, Quality { preset: Preset::Custom, fps: f, ..q })));
        } else {
            fps_items.push(MenuItem::disabled(label, None));
        }
    }
    let (e, set2) = (entity.clone(), set.clone());
    vec![
        MenuItem::Custom(group_label(crate::t!("golive.mode"))),
        radio(crate::t!("golive.gaming"), Preset::Video).with_subtext(preset_sub(Preset::Video)),
        radio(crate::t!("golive.docs"), Preset::Documents).with_subtext(preset_sub(Preset::Documents)),
        radio(crate::t!("golive.custom"), Preset::Custom),
        MenuItem::Separator,
        MenuItem::submenu(crate::t!("golive.resolution"), res_items),
        MenuItem::submenu(crate::t!("golive.fps"), fps_items),
        MenuItem::Separator,
        MenuItem::check(crate::t!("golive.mute"), !q.sound, move |_, _, cx| set2(&e, cx, Quality { sound: !q.sound, ..q })),
    ]
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let zoom = crate::theme::zoom();
    let (vw, vh) = s.viewport.map_or((1536., 840.), |vp| (f32::from(vp.width) / zoom, f32::from(vp.height) / zoom));
    let w = WIDTH.min(vw - 48.);
    let h = height(vh);
    let close = super::close_action(cx);
    let kind = s.kind;
    let body = if s.devices_tab {
        div().size_full().flex().items_center().justify_center().text_size(z(14.)).text_color(t(idx::TEXT_MUTED)).child(crate::t!("golive.no_devices")).into_any_element()
    } else {
        div()
            .size_full()
            .pt(z(16.))
            .px(z(40.))
            .pb(z(40.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(div().mb(z(8.)).text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(t(idx::TEXT_DEFAULT)).child(crate::t!("golive.title")))
            .child(
                div()
                    .mt(z(32.))
                    .flex()
                    .gap(z(16.))
                    .child(source_card("golive-screen", "golive/screen", crate::t!("golive.screen"), SourceKind::Screen, kind == Some(SourceKind::Screen), cx))
                    .child(source_card("golive-window", "golive/window", crate::t!("golive.window"), SourceKind::Window, kind == Some(SourceKind::Window), cx)),
            )
            .into_any_element()
    };
    let entity = cx.entity();
    let go = cx.listener(move |a, _, _, cx| {
        close_modal();
        crate::ui::go_live::start(a, kind.unwrap_or(SourceKind::Any));
        cx.notify();
    });
    let card = div()
        .w(z(w))
        .rounded(z(12.))
        .bg(t(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(t(idx::BORDER_SUBTLE))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        }])
        .pt(z(8.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(
            div()
                .h(z(h))
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(
                    div().px(z(24.)).py(z(16.)).child(
                        div()
                            .flex()
                            .p(z(4.))
                            .gap(z(4.))
                            .rounded(z(12.))
                            .bg(t(idx::BACKGROUND_BASE_LOWEST))
                            .child(tab("golive-tab-apps", "golive/window", crate::t!("golive.applications"), !s.devices_tab, false, cx))
                            .child(tab("golive-tab-devices", "video-call", crate::t!("golive.devices"), s.devices_tab, true, cx)),
                    ),
                )
                .child(div().flex_1().min_h_0().px(z(24.)).pt(z(8.)).child(body))
                .child(
                    div().bg(t(idx::BACKGROUND_SURFACE_HIGH)).child(
                        div()
                            .flex()
                            .justify_between()
                            .gap(z(32.))
                            .pt(z(16.))
                            .px(z(24.))
                            .child(summary(app))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(z(8.))
                                    .child(button("golive-go", crate::t!("golive.choose"), BtnKind::Primary).on_click(go))
                                    .child(
                                        div()
                                            .id("golive-options")
                                            .relative()
                                            .size(z(40.))
                                            .flex_none()
                                            .rounded(z(8.))
                                            .border_1()
                                            .border_color(t(idx::CONTROL_SECONDARY_BORDER_DEFAULT))
                                            .bg(t(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
                                            .hover(|s| s.bg(t(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .tip(crate::t!("golive.options"))
                                            .child(gpui::canvas(|b, _, _| COG.with(|c| c.set(Some(b))), |_, _, _, _| {}).absolute().size_full())
                                            .child(svg().path(crate::icons::path("gear")).size(z(20.)).text_color(t(idx::CONTROL_SECONDARY_TEXT_DEFAULT)))
                                            .on_click(cx.listener(|_, _, _, cx| update(cx, |s| s.menu = !s.menu))),
                                    ),
                            ),
                    ),
                ),
        );
    let menu = (s.menu)
        .then(|| COG.with(Cell::get))
        .flatten()
        .map(|b| {
            let e = entity.clone();
            let dismiss: crate::ui::context_menu::Dismiss = Rc::new(move |_, cx| {
                with_modal(|m| {
                    if let Some(Modal::GoLive(s)) = m {
                        s.menu = false;
                    }
                });
                e.update(cx, |_, cx| cx.notify());
            });
            crate::ui::context_menu::render_anchored_at(point(b.right(), b.top() - z(8.)), gpui::Anchor::BottomRight, settings_items(app, entity.clone()), dismiss, 30)
        });
    crate::ui::modal::frame("golive-backdrop", close, card.children(menu))
}

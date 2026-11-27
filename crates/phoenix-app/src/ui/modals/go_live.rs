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
    #[cfg(windows)]
    pub screens_tab: bool,
    pub kind: Option<SourceKind>,
    #[cfg(windows)]
    sources: sources::Picker,
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

fn tab(id: &'static str, icon: &'static str, label: SharedString, selected: bool, select: impl Fn(&mut State) + 'static, cx: &mut Context<PhoenixApp>) -> AnyElement {
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
        .on_click(cx.listener(move |_, _, _, cx| update(cx, &select)))
        .into_any_element()
}

#[cfg(not(windows))]
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

#[cfg(not(windows))]
fn tabs(d: gpui::Div, s: &State, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    d.child(tab("golive-tab-apps", "golive/window", crate::t!("golive.applications"), !s.devices_tab, |s| s.devices_tab = false, cx))
        .child(tab("golive-tab-devices", "video-call", crate::t!("golive.devices"), s.devices_tab, |s| s.devices_tab = true, cx))
}

#[cfg(windows)]
fn tabs(d: gpui::Div, s: &State, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let apps = !s.devices_tab && !s.screens_tab;
    d.child(tab("golive-tab-apps", "golive/window", crate::t!("golive.applications"), apps, |s| sources::switch(s, false, false), cx))
        .child(tab("golive-tab-screens", "golive/screen", crate::t!("golive.screens"), s.screens_tab, |s| sources::switch(s, true, false), cx))
        .child(tab("golive-tab-devices", "video-call", crate::t!("golive.devices"), s.devices_tab, |s| sources::switch(s, false, true), cx))
}

#[cfg(windows)]
mod sources {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;

    use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
    use phoenix_voice::media::desktop::{Capture, Target};

    use super::{State, t, update};
    use crate::app::PhoenixApp;
    use crate::theme::{generated::idx, z};
    use crate::voice::StreamSource;

    #[derive(Debug, Clone)]
    struct Choice {
        label: String,
        screen: Option<usize>,
        source: StreamSource,
        preview: Option<Arc<gpui::RenderImage>>,
    }

    #[derive(Debug, Clone, Default)]
    pub struct Picker {
        choices: Vec<Choice>,
        selected: Option<usize>,
        requested: Rc<Cell<bool>>,
        loaded: bool,
        error: Option<String>,
    }

    pub fn switch(s: &mut State, screens: bool, devices: bool) {
        if (s.screens_tab, s.devices_tab) != (screens, devices) {
            s.screens_tab = screens;
            s.devices_tab = devices;
            s.sources = Picker::default();
        }
    }

    fn preview(capture: &Capture) -> Option<Arc<gpui::RenderImage>> {
        let mut pixels = capture.thumbnail(256, 144)?;
        for p in pixels.pixels_mut() {
            p.0.swap(0, 2);
        }
        Some(Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)])))
    }

    fn discover(screens: bool, devices: bool) -> Result<Vec<Choice>, String> {
        if devices {
            return Ok(phoenix_voice::media::camera::windows_cameras()
                .into_iter()
                .map(|name| Choice { label: name.clone(), screen: None, source: StreamSource::Camera(name), preview: None })
                .collect());
        }
        let entries = Capture::list(!screens).map_err(|e| e.to_string())?;
        Ok(entries
            .into_iter()
            .enumerate()
            .map(|(i, (target, label, capture))| Choice {
                label,
                screen: matches!(target, Target::Monitor(_)).then_some(i + 1),
                preview: preview(&capture),
                source: StreamSource::Selected { target, size: capture.size() },
            })
            .collect())
    }

    pub fn body(s: &State, cx: &mut Context<PhoenixApp>) -> (AnyElement, Option<StreamSource>) {
        let p = &s.sources;
        if !p.requested.replace(true) {
            let (screens, devices) = (s.screens_tab, s.devices_tab);
            let current = p.requested.clone();
            let job = cx.background_executor().spawn(async move { discover(screens, devices) });
            cx.spawn(async move |this, cx| {
                let result = job.await;
                let _ = this.update(cx, |_, cx| {
                    update(cx, |state| {
                        if !Rc::ptr_eq(&state.sources.requested, &current) {
                            return;
                        }
                        state.sources.loaded = true;
                        match result {
                            Ok(rows) => state.sources.choices = rows,
                            Err(err) => state.sources.error = Some(err),
                        }
                    })
                });
            })
            .detach();
        }
        let empty: SharedString = if let Some(err) = &p.error {
            err.clone().into()
        } else if !p.loaded {
            crate::t!("golive.loading_sources")
        } else if s.devices_tab {
            crate::t!("golive.no_devices")
        } else {
            crate::t!("golive.no_sources")
        };
        let list = div()
            .id("share-sources")
            .size_full()
            .overflow_y_scroll()
            .p(z(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(z(10.))
                    .when(p.choices.is_empty(), |d| d.child(div().text_size(z(14.)).text_color(t(idx::TEXT_MUTED)).child(empty)))
                    .children(p.choices.iter().enumerate().map(|(i, choice)| {
                        let label: SharedString = match choice.screen {
                            Some(n) => crate::t!("golive.screen_n", index = n),
                            None => choice.label.clone().into(),
                        };
                        let selected = p.selected == Some(i);
                        div()
                            .id(("share-source", i))
                            .w(z(250.))
                            .p(z(8.))
                            .rounded(z(6.))
                            .border_1()
                            .border_color(t(if selected { idx::BRAND_500 } else { idx::BORDER_SUBTLE }))
                            .when(!selected, |d| d.hover(|s| s.bg(t(idx::INTERACTIVE_BACKGROUND_HOVER))))
                            .cursor_pointer()
                            .flex()
                            .flex_col()
                            .gap(z(6.))
                            .when_some(choice.preview.clone(), |d, img| d.child(gpui::img(img).w_full().h(z(125.))))
                            .child(
                                div()
                                    .text_size(z(14.))
                                    .line_height(z(18.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t(idx::INTERACTIVE_TEXT_DEFAULT))
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .child(label),
                            )
                            .on_click(cx.listener(move |_, _, _, cx| update(cx, |s| s.sources.selected = Some(i))))
                    })),
            )
            .into_any_element();
        let picked = p.selected.and_then(|i| p.choices.get(i)).map(|c| c.source.clone());
        (list, picked)
    }
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

pub fn group_label(text: SharedString) -> AnyElement {
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
    let upsell = |entity: &gpui::Entity<PhoenixApp>, cx: &mut gpui::App| {
        entity.update(cx, |a, cx| {
            close_modal();
            crate::ui::settings::open(a, cx);
            if let Some(s) = a.settings.as_mut() {
                s.page = crate::ui::settings::Page::Nitro;
            }
            cx.notify();
        });
    };
    let mut res_items = Vec::new();
    for r in OPTION_RESOLUTIONS {
        let (e, set) = (entity.clone(), set.clone());
        let ok = stream::allowed(perks, r, q.fps, false);
        res_items.push(MenuItem::radio(resolution_label(r), q.resolution == r, move |_, _, cx| {
            if ok { set(&e, cx, Quality { preset: Preset::Custom, resolution: r, ..q }) } else { upsell(&e, cx) }
        }));
    }
    let mut fps_items = Vec::new();
    for f in stream::FRAME_RATES {
        let (e, set) = (entity.clone(), set.clone());
        let ok = stream::allowed(perks, q.resolution, f, false);
        fps_items.push(MenuItem::radio(fps_label(f), q.fps == f, move |_, _, cx| {
            if ok { set(&e, cx, Quality { preset: Preset::Custom, fps: f, ..q }) } else { upsell(&e, cx) }
        }));
    }
    let (e, set2) = (entity.clone(), set.clone());
    let hide = stream::hide_preview();
    let e3 = entity.clone();
    let mut advanced = vec![MenuItem::check(crate::t!("golive.hide_preview"), hide, move |_, _, cx| {
        stream::set_hide_preview(!hide);
        e3.update(cx, |_, cx| cx.notify());
    })];
    if crate::ui::go_live::notify_shown(app) {
        let on = crate::ui::go_live::notify_friends();
        let e4 = entity.clone();
        advanced.push(MenuItem::check(crate::t!("golive.notify_friends"), on, move |_, _, cx| {
            crate::ui::go_live::set_notify_friends(!on);
            e4.update(cx, |_, cx| cx.notify());
        }));
    }
    let mut items = vec![
        MenuItem::Custom(group_label(crate::t!("golive.mode"))),
        radio(crate::t!("golive.gaming"), Preset::Video).with_subtext(preset_sub(Preset::Video)),
        radio(crate::t!("golive.docs"), Preset::Documents).with_subtext(preset_sub(Preset::Documents)),
        radio(crate::t!("golive.custom"), Preset::Custom),
    ];
    if q.preset == Preset::Custom {
        items.push(MenuItem::Separator);
        items.push(MenuItem::submenu(crate::t!("golive.resolution"), res_items));
        items.push(MenuItem::submenu(crate::t!("golive.fps"), fps_items));
    }
    items.push(MenuItem::Separator);
    items.push(MenuItem::check(crate::t!("golive.mute"), !q.sound, move |_, _, cx| set2(&e, cx, Quality { sound: !q.sound, ..q })));
    items.push(MenuItem::submenu(crate::t!("golive.advanced"), advanced));
    items
}

pub const OPTION_RESOLUTIONS: [u32; 4] = [720, 1080, 1440, 0];

pub fn fps_label(f: u32) -> SharedString {
    format!("{f}fps").into()
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let zoom = crate::theme::zoom();
    let (vw, vh) = s.viewport.map_or((1536., 840.), |vp| (f32::from(vp.width) / zoom, f32::from(vp.height) / zoom));
    let w = WIDTH.min(vw - 48.);
    let h = height(vh);
    let close = super::close_action(cx);
    #[cfg(windows)]
    let (body, picked) = sources::body(s, cx);
    #[cfg(not(windows))]
    let kind = s.kind;
    #[cfg(not(windows))]
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
    #[cfg(not(windows))]
    let (picked, ready) = (crate::voice::StreamSource::Portal(kind.unwrap_or(SourceKind::Any)), true);
    #[cfg(windows)]
    let ready = picked.is_some();
    let go = cx.listener(move |a, _, _, cx| {
        #[cfg(windows)]
        let Some(source) = picked.clone() else { return };
        #[cfg(not(windows))]
        let source = picked.clone();
        close_modal();
        crate::ui::go_live::start(a, source);
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
                            .map(|d| tabs(d, s, cx)),
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
                                    .child(button("golive-go", crate::t!("golive.choose"), BtnKind::Primary).when(ready, |b| b.on_click(go)).when(!ready, |b| b.opacity(0.5)))
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

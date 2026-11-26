use std::path::PathBuf;

use gpui::{
    AnyElement, Context, Div, Entity, FontWeight, IntoElement, ParentElement, SharedString, Stateful, Styled, Window,
    div, img, prelude::*, px,
};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};

use super::Page;
use crate::{
    app::PhoenixApp,
    model::Id,
    presence::Profile,
    theme::{c, generated::idx, tok, z},
    ui::{
        avatar,
        controls::{self, BtnKind},
    },
};

type Cx<'a> = Context<'a, PhoenixApp>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfilePatch {
    pub display_name: Option<String>,
    pub pronouns: Option<String>,
    pub bio: Option<String>,
    pub avatar: Option<Option<String>>,
    pub banner: Option<Option<String>>,
    pub theme_colors: Option<(u32, u32)>,
}

impl ProfilePatch {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draft {
    pub name: String,
    pub pronouns: String,
    pub bio: String,
    pub theme: Option<(u32, u32)>,
}

pub fn diff(orig: &Draft, cur: &Draft, avatar: Option<&str>, banner: Option<&str>) -> ProfilePatch {
    let ch = |a: &String, b: &String| (a != b).then(|| b.clone());
    ProfilePatch {
        display_name: ch(&orig.name, &cur.name),
        pronouns: ch(&orig.pronouns, &cur.pronouns),
        bio: ch(&orig.bio, &cur.bio),
        avatar: avatar.map(|a| Some(a.to_string())),
        banner: banner.map(|b| Some(b.to_string())),
        theme_colors: if orig.theme != cur.theme { cur.theme } else { None },
    }
}

const SWATCHES: [u32; 10] = [0x00A7FF, 0x00FBFF, 0x5865F2, 0xEB459E, 0xED4245, 0xFEE75C, 0x57F287, 0x1ABC9C, 0x9B59B6, 0x23272A];

pub struct EditorState {
    pub scope: Option<Id>,
    pub modal: bool,
    pub collapsed: bool,
    pub switcher_open: bool,
    pub notice_hidden: bool,
    pub tab: usize,
    pub profile: Option<Profile>,
    pub original: Draft,
    pub theme: Option<(u32, u32)>,
    pub palette: Option<u8>,
    pub name: Option<Entity<InputState>>,
    pub pronouns: Option<Entity<InputState>>,
    pub bio: Option<Entity<TextareaState>>,
    pub subs: Vec<gpui::Subscription>,
    pub avatar: Option<PathBuf>,
    pub banner: Option<PathBuf>,
    pub saving: bool,
    pub error: Option<SharedString>,
    pub rebuild: bool,
    pub generation: u32,
    pub widgets: Vec<super::profile_widgets::AppWidget>,
    pub picker: Option<super::collectible_picker::PickerState>,
    pub collectibles: Option<Vec<super::collectibles::Collectible>>,
    pub shop: Option<super::collectibles::ShopCatalog>,
    pub equipped: super::collectible_picker::Equipped,
    pub recent: Vec<super::profile_activity::RecentGame>,
}

impl EditorState {
    fn new(scope: Option<Id>) -> Self {
        Self {
            scope,
            modal: false,
            collapsed: false,
            switcher_open: false,
            notice_hidden: false,
            tab: 0,
            profile: None,
            original: Draft::default(),
            theme: None,
            palette: None,
            name: None,
            pronouns: None,
            bio: None,
            subs: Vec::new(),
            avatar: None,
            banner: None,
            saving: false,
            error: None,
            rebuild: true,
            generation: 0,
            widgets: Vec::new(),
            picker: None,
            collectibles: None,
            shop: None,
            equipped: Default::default(),
            recent: Vec::new(),
        }
    }

    pub fn current(&self, cx: &gpui::App) -> Draft {
        Draft {
            name: self.name.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| self.original.name.clone()),
            pronouns: self
                .pronouns
                .as_ref()
                .map(|i| i.read(cx).value().to_string())
                .unwrap_or_else(|| self.original.pronouns.clone()),
            bio: self.bio.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| self.original.bio.clone()),
            theme: self.theme,
        }
    }

    pub fn dirty(&self, cx: &gpui::App) -> bool {
        self.avatar.is_some() || self.banner.is_some() || self.current(cx) != self.original
    }
}

pub fn on_escape(s: &mut super::SettingsState) -> bool {
    if let Some(e) = s.profile.as_mut()
        && (e.switcher_open || e.palette.is_some() || e.picker.is_some())
    {
        e.picker = None;
        e.switcher_open = false;
        e.palette = None;
        return false;
    }
    if s.profile.as_ref().is_some_and(|e| e.modal) {
        return true;
    }
    s.profile = None;
    s.page = Page::MyAccount;
    false
}

fn ed(app: &mut PhoenixApp) -> Option<&mut EditorState> {
    app.settings.as_mut()?.profile.as_mut()
}

fn original_for(app: &PhoenixApp, scope: Option<Id>, p: Option<&Profile>) -> Draft {
    let name = match scope {
        Some(_) => p.and_then(|p| p.nick.clone()).unwrap_or_default().to_string(),
        None => app.me.display_name.to_string(),
    };
    Draft {
        name,
        pronouns: p.map(|p| p.pronouns.to_string()).unwrap_or_default(),
        bio: p.map(|p| p.bio.to_string()).unwrap_or_default(),
        theme: p.and_then(|p| p.theme),
    }
}

fn fetch(app: &mut PhoenixApp, cx: &mut Cx) {
    let Some(e) = app.settings.as_ref().and_then(|s| s.profile.as_ref()) else { return };
    let (scope, generation) = (e.scope, e.generation);
    let fut = app.backend.user_profile(app.me.id, scope);
    let wfut = app.backend.profile_widgets(app.me.id);
    let afut = app.backend.recent_activity(app.me.id);
    cx.spawn(async move |this, cx| {
        let Ok(r) = afut.await else { return };
        let _ = this.update(cx, |this, cx| {
            if let Some(e) = ed(this)
                && e.generation == generation
            {
                e.recent = r;
                cx.notify();
            }
        });
    })
    .detach();
    cx.spawn(async move |this, cx| {
        let Ok(w) = wfut.await else { return };
        let _ = this.update(cx, |this, cx| {
            if let Some(e) = ed(this)
                && e.generation == generation
            {
                e.widgets = w;
                cx.notify();
            }
        });
    })
    .detach();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |this, cx| {
            let Ok(p) = res else { return };
            let orig = original_for(this, scope, Some(&p));
            if let Some(e) = ed(this)
                && e.generation == generation
            {
                e.original = orig;
                e.theme = e.original.theme;
                e.profile = Some(p);
                e.rebuild = true;
                cx.notify();
            }
        });
    })
    .detach();
}

fn switch_scope(app: &mut PhoenixApp, scope: Option<Id>, cx: &mut Cx) {
    let orig = original_for(app, scope, None);
    if let Some(e) = ed(app) {
        let (generation, modal, tab) = (e.generation + 1, e.modal, e.tab);
        *e = EditorState::new(scope);
        e.generation = generation;
        e.modal = modal;
        e.tab = tab;
        e.original = orig;
    }
    fetch(app, cx);
    cx.notify();
}

pub fn open_scoped(app: &mut PhoenixApp, guild: Id, cx: &mut Cx) {
    let orig = original_for(app, Some(guild), None);
    let Some(s) = app.settings.as_mut() else { return };
    let generation = s.profile.as_ref().map_or(0, |e| e.generation + 1);
    let mut e = EditorState::new(Some(guild));
    e.generation = generation;
    e.original = orig;
    s.profile = Some(e);
    fetch(app, cx);
    cx.notify();
}

pub fn open_modal(app: &mut PhoenixApp, guild: Option<Id>, tab: usize, cx: &mut Cx) {
    let orig = original_for(app, guild, None);
    let mut e = EditorState::new(guild);
    e.modal = true;
    e.tab = tab;
    e.original = orig;
    app.settings = Some(super::SettingsState { page: Page::Profiles, profile: Some(e), ..Default::default() });
    fetch(app, cx);
    cx.notify();
}

fn reset(app: &mut PhoenixApp, cx: &mut Cx) {
    if let Some(e) = ed(app) {
        e.avatar = None;
        e.banner = None;
        e.theme = e.original.theme;
        e.error = None;
        e.rebuild = true;
    }
    cx.notify();
}

fn data_uri(path: &std::path::Path) -> Option<String> {
    crate::ui::file_pick::data_uri(path)
}

fn save(app: &mut PhoenixApp, cx: &mut Cx) {
    let Some(e) = app.settings.as_ref().and_then(|s| s.profile.as_ref()) else { return };
    if e.saving {
        return;
    }
    let cur = e.current(cx);
    let scope = e.scope;
    let avatar = e.avatar.as_deref().and_then(data_uri);
    let banner = e.banner.as_deref().and_then(data_uri);
    let patch = diff(&e.original, &cur, avatar.as_deref(), banner.as_deref());
    if patch.is_empty() {
        return;
    }
    let fut = app.backend.update_profile(scope, patch);
    if let Some(e) = ed(app) {
        e.saving = true;
        e.error = None;
    }
    cx.notify();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |this, cx| {
            if res.is_ok() && scope.is_none() {
                this.me.display_name = cur.name.clone().into();
            }
            if let Some(e) = ed(this) {
                e.saving = false;
                match res {
                    Ok(()) => {
                        e.original = cur;
                        e.avatar = None;
                        e.banner = None;
                    }
                    Err(err) => e.error = Some(err.0.into()),
                }
            }
            cx.notify();
        });
    })
    .detach();
}

fn pick_image(banner: bool, cx: &mut Cx) {
    let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
    cx.spawn(async move |this, cx| {
        if let Ok(Ok(Some(mut paths))) = rx.await
            && let Some(p) = paths.pop()
        {
            let _ = this.update(cx, |this, cx| {
                if let Some(e) = ed(this) {
                    if banner {
                        e.banner = Some(p);
                    } else {
                        e.avatar = Some(p);
                    }
                }
                cx.notify();
            });
        }
    })
    .detach();
}

fn ensure_inputs(app: &PhoenixApp, window: &mut Window, cx: &mut Cx) {
    let Some(e) = app.settings.as_ref().and_then(|s| s.profile.as_ref()) else { return };
    if !e.rebuild {
        return;
    }
    cx.defer_in(window, |a, window, cx| {
        let Some(e) = ed(a) else { return };
        if !e.rebuild {
            return;
        }
        let o = e.original.clone();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(o.name.clone()));
        let hint = if e.scope.is_some() { crate::t!("pe.server_pronouns") } else { crate::t!("pe.add_pronouns") };
        let pronouns = cx.new(|cx| InputState::new(window, cx).placeholder(hint).default_value(o.pronouns.clone()));
        let bio = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 10).default_value(o.bio.clone()));
        let subs = vec![cx.observe(&name, |_, _, cx| cx.notify()), cx.observe(&pronouns, |_, _, cx| cx.notify()), cx.observe(&bio, |_, _, cx| cx.notify())];
        let Some(e) = ed(a) else { return };
        e.name = Some(name);
        e.pronouns = Some(pronouns);
        e.bio = Some(bio);
        e.subs = subs;
        e.rebuild = false;
        cx.notify();
    });
}

pub(super) fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |v: u32, s: u32| ((v >> s) & 0xFF) as f32;
    [16, 8, 0].iter().fold(0, |acc, &s| acc | ((ch(a, s) * (1. - t) + ch(b, s) * t).round() as u32) << s)
}

pub(super) fn white(a: f32) -> gpui::Hsla {
    gpui::hsla(0., 0., 1., a)
}

fn modal_bg((a, b): (u32, u32)) -> u32 {
    mix(mix(a, b, 0.5), 0, 0.8)
}

pub(super) fn hex(v: u32) -> gpui::Hsla {
    c(v & 0xFFFFFF)
}

fn group_label(key: &str, nitro: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(z(4.))
        .text_size(z(14.))
        .line_height(z(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_STRONG)))
        .child(crate::t!(key))
        .when(nitro, |d| d.child(gpui::svg().path(crate::icons::path("nitro")).size(z(16.)).text_color(c(tok(idx::TEXT_STRONG)))))
}

fn tile(id: &str, w: f32, h: f32, locked: bool) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("pe-tile-{id}")))
        .relative()
        .w(z(w))
        .h(z(h))
        .flex_none()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        .border_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .when(locked, |d| d.opacity(0.5))
}

fn plus_icon(size: f32) -> gpui::Svg {
    gpui::svg().path(crate::icons::path("plus-circle")).size(z(size)).text_color(c(tok(idx::ICON_STRONG)))
}

fn overlay_button(id: &str, icon: &str) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("pe-ov-{id}")))
        .size(z(31.))
        .flex_none()
        .rounded(z(8.))
        .bg(c(tok(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).text_color(c(tok(idx::TEXT_OVERLAY_LIGHT))))
}

fn scope_name(app: &PhoenixApp, scope: Option<Id>) -> SharedString {
    match scope.and_then(|g| app.guilds.iter().find(|x| x.id == g)) {
        Some(g) => g.name.clone(),
        None => crate::t!("sp.pf.main"),
    }
}

fn styles_panel(app: &PhoenixApp, e: &EditorState, modal: Option<crate::ui::profile::ModalBox>, cx: &mut Cx) -> Div {
    let nitro = app.backend.has_premium();
    let locked = !nitro;
    let me = &app.me;
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .px(z(8.))
        .py(z(4.))
        .gap(z(4.))
        .border_b_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .when(modal.is_some(), |d| d.flex_row_reverse())
        .when(modal.is_some(), |d| {
            d.child(
                div()
                    .id("pe-hide-styles")
                    .size(z(32.))
                    .flex_none()
                    .rounded(z(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                    .child(gpui::svg().path(crate::icons::path("double-chevron-right")).size(z(16.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT))))
                    .tooltip(crate::ui::tooltip::tip(crate::t!("pe.hide_styles")))
                    .on_click(cx.listener(|a, _, _, cx| {
                        if let Some(e) = ed(a) {
                            e.collapsed = true;
                            e.switcher_open = false;
                        }
                        cx.notify();
                    })),
            )
        })
        .child(
            div().py(z(4.)).child(
                div()
                    .id("pe-scope")
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .pt(z(8.))
                    .pr(z(4.))
                    .pb(z(4.))
                    .pl(z(8.))
                    .rounded(z(8.))
                    .cursor_pointer()
                    .when(e.switcher_open, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                    .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .children(e.scope.and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| {
                        div().flex_none().rounded(z(4.)).overflow_hidden().child(crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 20.))
                    }))
                    .child(div().max_w(z(150.)).truncate().child(scope_name(app, e.scope)))
                    .child(
                        gpui::svg()
                            .path(crate::icons::path("chevron"))
                            .size(z(16.))
                            .text_color(c(tok(idx::ICON_STRONG)))
                            .when(e.switcher_open, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI)))),
                    )
                    .on_click(cx.listener(|a, _, _, cx| {
                        if let Some(e) = ed(a) {
                            e.switcher_open = !e.switcher_open;
                        }
                        cx.notify();
                    })),
            ),
        );

    let group = |label: Div, body: AnyElement| div().flex().flex_col().gap(z(8.)).child(label).child(body);
    let avatar_preview: AnyElement = match &e.avatar {
        Some(p) => img(p.clone()).size(px(72.)).rounded_full().into_any_element(),
        None => avatar(Some(me), 72.),
    };
    let banner_bg = e.theme.map(|t| t.0).or(e.profile.as_ref().and_then(|p| p.accent)).unwrap_or(0x5865F2);
    let banner_tile: AnyElement = match (&e.banner, e.profile.as_ref().and_then(|p| p.banner.clone())) {
        (Some(p), _) => img(p.clone()).size_full().into_any_element(),
        (None, Some(url)) => img(url).size_full().object_fit(gpui::ObjectFit::Cover).into_any_element(),
        (None, None) => plus_icon(24.).into_any_element(),
    };
    let swatch = |i: u8, col: u32| {
        div()
            .id(SharedString::from(format!("pe-sw-{i}")))
            .size(z(24.))
            .rounded(z(8.))
            .border_2()
            .border_color(gpui::white())
            .bg(hex(col))
            .cursor_pointer()
            .on_click(cx.listener(move |a, _, _, cx| {
                if !a.backend.has_premium() {
                    return;
                }
                if let Some(e) = ed(a) {
                    e.palette = if e.palette == Some(i) { None } else { Some(i) };
                }
                cx.notify();
            }))
    };
    let theme = e.theme.unwrap_or((banner_bg, banner_bg));
    let theme_tile = div()
        .relative()
        .w(z(100.))
        .h(z(100.))
        .flex_none()
        .rounded(z(8.))
        .overflow_hidden()
        .bg(gpui::linear_gradient(180., gpui::linear_color_stop(hex(theme.0), 0.), gpui::linear_color_stop(hex(theme.1), 1.)))
        .flex()
        .flex_col()
        .items_center()
        .justify_between()
        .py(z(4.))
        .child(swatch(0, theme.0))
        .child(swatch(1, theme.1))
        .when(locked, |d| d.opacity(0.5));
    let palette = e.palette.map(|which| {
        div().flex().flex_wrap().gap(z(6.)).children(SWATCHES.iter().map(|&col| {
            div()
                .id(SharedString::from(format!("pe-pal-{col:06x}")))
                .size(z(24.))
                .rounded(z(8.))
                .bg(hex(col))
                .cursor_pointer()
                .when((if which == 0 { theme.0 } else { theme.1 }) == col, |d| d.border_2().border_color(gpui::white()))
                .on_click(cx.listener(move |a, _, _, cx| {
                    if let Some(e) = ed(a) {
                        let (mut p, mut s) = e.theme.unwrap_or((col, col));
                        if which == 0 {
                            p = col
                        } else {
                            s = col
                        }
                        e.theme = Some((p, s));
                    }
                    cx.notify();
                }))
        }))
    });
    let plate = nameplate_url(app, e).map(|u| {
        div().absolute().top_0().left_0().size_full().rounded(z(8.)).overflow_hidden().child(img(u).absolute().top_0().right_0().h_full().w(z(48. * 213. / 40.)).opacity(0.6))
    });
    let nameplate = div().relative().flex().items_center().justify_between().w_full().px(z(8.)).children(plate).child(
        div().flex().items_center().gap(z(8.)).child(avatar(Some(me), 32.)).child(div().w(z(110.)).h(z(10.)).rounded_full().bg(c(tok(idx::BACKGROUND_MOD_STRONG)))),
    );
    let content = div()
        .flex()
        .flex_col()
        .gap(z(24.))
        .p(z(16.))
        .child(group(group_label("sp.pf.nameplate", false), tile("nameplate", 208., 42., false).child(nameplate.child(plus_icon(20.))).on_click(cx.listener(|a, _, _, cx| super::collectible_picker::open(a, super::collectibles::Kind::Nameplate, cx))).into_any_element()))
        .child(group(
            group_label("sp.pf.avatar", false),
            div()
                .flex()
                .gap(z(8.))
                .child(tile("avatar", 100., 100., false).child(avatar_preview).on_click(cx.listener(|_, _, _, cx| pick_image(false, cx))))
                .child(tile("decoration", 100., 100., false).map(|t| match decoration_url(e) {
                    Some(d) => t.child(div().relative().size(z(72.)).child(avatar(Some(me), 72.)).child(img(d).absolute().size(z(86.4)).left(z(-7.2)).top(z(-7.2)))),
                    None => t.child(plus_icon(24.)),
                }).on_click(cx.listener(|a, _, _, cx| super::collectible_picker::open(a, super::collectibles::Kind::Decoration, cx))))
                .into_any_element(),
        ))
        .child(group(
            group_label("sp.pf.name_style", true),
            tile("name-style", 208., 42., locked)
                .child(crate::fonts::styled_name(div().text_size(z(24.)).line_height(z(32.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))), Some(me), true).child(crate::fonts::name_text(if e.original.name.is_empty() { me.display_name.clone() } else { SharedString::from(e.original.name.clone()) }, Some(me), 24.)))
                .into_any_element(),
        ))
        .child(group(
            group_label("sp.pf.theme", true),
            div()
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(
                    div().flex().gap(z(8.)).child(theme_tile).child(tile("banner", 100., 100., locked).child(banner_tile).on_click(cx.listener(|a, _, _, cx| {
                        if a.backend.has_premium() {
                            pick_image(true, cx)
                        }
                    }))),
                )
                .children(palette)
                .into_any_element(),
        ))
        .child(group(
            group_label("sp.pf.effect", false),
            div()
                .flex()
                .gap(z(8.))
                .child(tile("effect", 100., 100., false).child(plus_icon(24.)).on_click(cx.listener(|a, _, _, cx| super::collectible_picker::open(a, super::collectibles::Kind::Effect, cx))))
                .child(tile("frame", 100., 100., false).child(plus_icon(24.)).on_click(cx.listener(|a, _, _, cx| super::collectible_picker::open(a, super::collectibles::Kind::Frame, cx))))
                .into_any_element(),
        ));
    let panel = match modal {
        Some(mb) => div()
            .w(px(mb.panel))
            .pt(z(mb.system_bar))
            .bg(c(tok(idx::MODAL_BACKGROUND)))
            .rounded_l(z((mb.radius - 1.).max(0.)))
            .when(mb.border_block, |d| d.border_t_1().border_b_1())
            .border_l_1()
            .border_color(c(tok(idx::BORDER_NORMAL)))
            .child(div().absolute().top_0().bottom_0().right_0().w(px(1.)).bg(c(tok(idx::BORDER_SUBTLE)))),
        None => div().w(z(242.)).pt(z(32.)).bg(c(tok(idx::BACKGROUND_BASE_LOWEST))).border_l_1().border_r_1().border_color(c(tok(idx::BORDER_SUBTLE))),
    };
    panel
        .relative()
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(header)
        .child(div().id("pe-styles").flex_1().min_h_0().overflow_y_scroll().children((e.scope.is_some() && !e.notice_hidden).then(|| {
            let notice = div()
                .flex()
                .p(z(8.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::ICON_FEEDBACK_INFO)))
                .bg(c(tok(idx::BACKGROUND_FEEDBACK_MOD_INFO)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .whitespace_normal()
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(crate::t!("sp.pf.server_notice")),
                )
                .child(
                    div()
                        .id("pe-notice-x")
                        .flex_none()
                        .size(z(24.))
                        .mt(z(-2.))
                        .mr(z(-2.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(c(tok(idx::ICON_SUBTLE)))
                        .hover(|s| s.text_color(c(tok(idx::ICON_STRONG))))
                        .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::ICON_SUBTLE))))
                        .on_click(cx.listener(|a, _, _, cx| {
                            if let Some(e) = ed(a) {
                                e.notice_hidden = true;
                            }
                            cx.notify();
                        })),
                );
            div().px(z(16.)).pt(z(16.)).child(notice)
        })).child(content))
}

fn switcher(app: &PhoenixApp, e: &EditorState, cx: &mut Cx) -> Div {
    let row = |id: SharedString, icon: AnyElement, name: SharedString, sub: Option<SharedString>, sel: bool| {
        div()
            .id(id)
            .flex()
            .items_center()
            .flex_none()
            .min_h(z(40.))
            .p(z(4.))
            .pl(z(8.))
            .rounded(z(4.))
            .cursor_pointer()
            .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
            .child(div().flex_none().size(z(32.)).ml(z(-8.)).flex().items_center().justify_center().child(icon))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(name))
                    .children(sub.map(|s| div().truncate().text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_MUTED))).child(s))),
            )
            .when(sel, |d| d.child(div().flex_none().px(z(4.)).child(gpui::svg().path(crate::icons::path("check")).size(z(20.)).text_color(c(tok(idx::ICON_STRONG))))))
    };
    let main = row(
        "pe-scope-main".into(),
        gpui::svg().path(crate::icons::path("discord-logo")).size(z(20.)).text_color(c(tok(idx::ICON_STRONG))).into_any_element(),
        crate::t!("sp.pf.main"),
        None,
        e.scope.is_none(),
    )
    .on_click(cx.listener(|a, _, _, cx| switch_scope(a, None, cx)));
    let guilds = app.guilds.iter().map(|g| {
        let gid = g.id;
        row(
            format!("pe-scope-{}", g.id.0).into(),
            crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 20.).into_any_element(),
            g.name.clone(),
            Some(app.me.display_name.clone()),
            e.scope == Some(gid),
        )
        .on_click(cx.listener(move |a, _, _, cx| switch_scope(a, Some(gid), cx)))
    });
    div()
        .absolute()
        .top(z(80.))
        .left(z(9.))
        .w(z(264.))
        .max_h(z(221.))
        .p(z(4.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow { color: c(tok(idx::OPACITY_BLACK_16)), offset: gpui::point(z(0.), z(2.)), blur_radius: z(8.), spread_radius: z(0.), inset: false }])
        .child(div().id("pe-scope-list").max_h(z(212.)).overflow_y_scroll().flex().flex_col().child(main).children(guilds))
}

fn section(key: &str, color: gpui::Hsla, body: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).text_color(color).child(crate::t!(key)))
        .child(body)
}

fn bio_section(server: bool, color: gpui::Hsla, body: impl IntoElement) -> Div {
    let label = if server { crate::t!("pe.server_bio") } else { crate::t!("pe.bio") };
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .child(label)
                .when(server, |d| d.child(gpui::svg().path(crate::icons::path("nitro")).size(z(12.)).flex_none().text_color(color))),
        )
        .child(body)
}

fn inline_field(child: AnyElement) -> Div {
    div().rounded(z(8.)).px(z(7.)).ml(z(-7.)).hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))).child(child)
}

fn card(app: &PhoenixApp, e: &EditorState, bio_focused: bool, cx: &mut Cx) -> Div {
    let me = &app.me;
    let banner_col = e.theme.map(|t| t.0).or(e.profile.as_ref().and_then(|p| p.accent)).unwrap_or(0x5865F2);
    let banner: AnyElement = match (&e.banner, e.profile.as_ref().and_then(|p| p.banner.clone())) {
        (Some(p), _) => img(p.clone()).w_full().h(z(140.)).into_any_element(),
        (None, Some(url)) => img(url).w_full().h(z(140.)).into_any_element(),
        _ => div().w_full().h(z(140.)).bg(hex(banner_col)).into_any_element(),
    };
    let ring = match e.theme {
        Some(t) => hex(mix(t.0, 0, 0.6)),
        None => c(tok(idx::BACKGROUND_SURFACE_HIGH)),
    };
    let av: AnyElement = match &e.avatar {
        Some(p) => img(p.clone()).size(px(112.)).rounded_full().into_any_element(),
        None => crate::ui::avatar_with_status(Some(me), 112., app.presence_of(me.id), ring).into_any_element(),
    };
    let themed = e.theme.is_some();
    let card_layers: Vec<Div> = match e.theme {
        Some(t) => vec![
            div().absolute().top_0().left_0().right_0().h(z(140.)).bg(hex(t.0)),
            div()
                .absolute()
                .top(z(140.))
                .left_0()
                .right_0()
                .bottom_0()
                .bg(gpui::linear_gradient(180., gpui::linear_color_stop(hex(t.0), 0.), gpui::linear_color_stop(hex(t.1), 1.))),
            div().absolute().inset_0().bg(gpui::hsla(0., 0., 0., 0.6)),
        ],
        None => vec![div().absolute().inset_0().bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))],
    };
    let strong = if themed { white(0.96) } else { c(tok(idx::TEXT_STRONG)) };
    let heading = if themed { white(0.88) } else { c(tok(idx::TEXT_SUBTLE)) };
    let name: AnyElement = match &e.name {
        Some(i) => Input::new(i).appearance(false).bordered(false).text_size(z(24.)).font_weight(FontWeight::SEMIBOLD).into_any_element(),
        None => SharedString::from(e.original.name.clone()).into_any_element(),
    };
    let pronouns: AnyElement = match &e.pronouns {
        Some(i) => div().w(z(if e.scope.is_some() { 201. } else { 134. })).child(Input::new(i).appearance(false).bordered(false).px(z(5.)).text_size(z(14.)).line_height(z(18.))).into_any_element(),
        None => SharedString::from(e.original.pronouns.clone()).into_any_element(),
    };
    let bio_text: SharedString = e.bio.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_else(|| e.original.bio.clone()).into();
    let bio: AnyElement = match &e.bio {
        Some(i) if bio_focused || bio_text.is_empty() => Textarea::new(i).appearance(false).bordered(false).into_any_element(),
        Some(i) => {
            let fh = gpui::Focusable::focus_handle(i.read(cx), cx);
            div()
                .id("pe-bio-preview")
                .cursor_text()
                .on_click(move |_, window, cx| window.focus(&fh, cx))
                .child(bio_markup(bio_text))
                .into_any_element()
        }
        None => bio_markup(bio_text).into_any_element(),
    };
    let created = e.profile.as_ref().map(|p| p.created).unwrap_or_else(|| crate::presence::snowflake_time(me.id.0));
    let sec_btn = |id: &str, icon: &str| {
        div()
            .id(SharedString::from(format!("pe-btn-{id}")))
            .size(z(31.))
            .rounded(z(8.))
            .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
            .flex()
            .items_center()
            .justify_center()
            .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
    };
    let body = div()
        .flex()
        .flex_col()
        .gap(z(24.))
        .pt(z(4.))
        .pr(z(24.))
        .pb(z(32.))
        .pl(z(32.))
        .child(
            div()
                .flex()
                .flex_col()
                .child(inline_field(
                    crate::fonts::styled_name(div().text_size(z(24.)).line_height(z(32.)).font_weight(FontWeight::SEMIBOLD).text_color(strong), Some(me), true)
                        .child(name)
                        .into_any_element(),
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(6.))
                        .h(z(32.))
                        .mt(z(4.))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(strong)
                        .child(me.name.clone())
                        .child(div().size(z(4.)).mx(z(1.)).rounded_full().bg(strong))
                        .child(div().flex_none().rounded(z(8.)).hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))).italic().child(pronouns)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(z(6.))
                        .mt(z(11.))
                        .when_some(me.tag.clone(), |d, t| d.child(tag_select(t, strong)))
                        .when_some(badges_row(e.profile.as_ref()), |d, b| d.child(b)),
                ),
        )
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(
                    controls::button_icon("pe-msg", "chat", crate::t!("pe.send_message"), BtnKind::Primary)
                        .h(z(31.))
                        .min_w(z(0.))
                        .px(z(11.))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .opacity(0.5)
                        .when_some(e.theme, |b, t| b.bg(hex(gradient_button_color(t.0)))),
                )
                .child(sec_btn("shop", "shop"))
                .child(sec_btn("more", "chat/more")),
        )
        .child(bio_section(e.scope.is_some(), heading, inline_field(div().mt(z(-8.)).pt(z(5.)).pb(z(4.)).text_size(z(14.)).line_height(z(18.)).text_color(strong).child(bio).into_any_element())))
        .map(|d| match e.profile.as_ref() {
            Some(p) => d.child(crate::ui::profile::member_since_section(app, p, e.scope)),
            None => d.child(section(
                "pe.member_since",
                heading,
                div().text_size(z(14.)).line_height(z(18.)).text_color(strong).child(crate::presence::format_date(created)),
            )),
        })
        .children(crate::ui::profile::roles_section(app, e.scope, me.id, cx))
        .child(section(
            "pe.connections",
            heading,
            div()
                .flex()
                .items_center()
                .gap(z(2.))
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if themed { white(0.76) } else { c(tok(idx::TEXT_MUTED)) })
                .child(gpui::svg().path(crate::icons::path("plus")).size(z(18.)).text_color(if themed { white(0.76) } else { c(tok(idx::TEXT_MUTED)) }))
                .child(crate::t!("pe.add_connection")),
        ))
        .when(crate::ui::profile_note::shown(crate::ui::profile_note::Surface::FullProfile, true, false, app.prefs.local.streamer_hides_info()), |d| {
            d.child(section(
            "pe.note",
            heading,
            div()
                .mt(z(-4.))
                .ml(z(-7.))
                .px(z(7.))
                .rounded(z(8.))
                .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .text_size(z(14.))
                .line_height(z(18.))
                .map(|d| match e.profile.as_ref().map(|p| p.note.clone()).filter(|n| !n.is_empty()) {
                    Some(n) => d.text_color(strong).child(n),
                    None => d.text_color(if themed { white(0.6) } else { c(tok(idx::TEXT_MUTED)) }).child(crate::t!("profile.note_placeholder")),
                }),
            ))
        });
    let effect = applied(app, e, super::collectibles::Kind::Effect, cx).and_then(|c| crate::ui::profile_fx::effect(&c, 400., cx));
    let frame = applied(app, e, super::collectibles::Kind::Frame, cx).and_then(|c| c.frame);
    let inner = div()
        .w(z(400.))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .rounded_tl(z(16.))
        .rounded_tr(z(16.))
        .relative()
        .overflow_hidden()
        .children(card_layers)
        .shadow_md()
        .child(
            div()
                .relative()
                .h(z(220.))
                .flex_none()
                .child(div().id("pe-banner").group("pe-banner").relative().child(banner).child(
                    div().absolute().top(z(8.)).right(z(8.)).opacity(0.).group_hover("pe-banner", |s| s.opacity(1.)).child(overlay_button("banner", "pen").on_click(cx.listener(|a, _, _, cx| {
                        if a.backend.has_premium() {
                            pick_image(true, cx)
                        }
                    }))),
                ))
                .child(
                    div()
                        .absolute()
                        .left(z(32.))
                        .top(z(80.))
                        .size(z(120.))
                        .rounded_full()
                        .bg(ring)
                        .group("pe-avatar")
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(av)
                        .children(decoration_url(e).map(|d| img(d).absolute().size(z(134.4)).left(z(-7.2)).top(z(-7.2))))
                        .child(
                            div().absolute().top(z(44.)).left(z(44.)).opacity(0.).group_hover("pe-avatar", |s| s.opacity(1.)).child(overlay_button("avatar", "pen").on_click(cx.listener(|_, _, _, cx| pick_image(false, cx)))),
                        ),
                ),
        )
        .children(
            crate::ui::profile_toolbar::status(app, me.id, e.scope, crate::ui::profile_toolbar::Layout::Modal, themed, crate::ui::profile::info(app, me.id, e.scope).custom, cx)
                .map(|s| div().absolute().left(z(172.)).top(z(132.)).child(s)),
        )
        .child(div().id("pe-card-body").flex_1().min_h_0().overflow_y_scroll().child(body))
        .children(effect);
    div()
        .relative()
        .w(z(400.))
        .h_full()
        .flex_none()
        .flex()
        .children(frame.as_ref().and_then(|f| crate::ui::profile_fx::frame(f, 400., false, cx)))
        .child(inner)
        .children(frame.as_ref().and_then(|f| crate::ui::profile_fx::frame(f, 400., true, cx)))
}

fn applied(app: &PhoenixApp, e: &EditorState, kind: super::collectibles::Kind, cx: &mut Cx) -> Option<super::collectibles::Collectible> {
    use super::collectibles::Kind;
    let sku = match e.equipped.get(kind) {
        Some(s) => s?,
        None => match kind {
            Kind::Effect => e.profile.as_ref()?.effect_sku?,
            Kind::Frame => e.profile.as_ref()?.frame_sku?,
            _ => return None,
        },
    };
    applied_item(e, kind, sku).cloned().or_else(|| crate::ui::profile_fx::product(app, sku, cx))
}

fn nameplate_url(app: &PhoenixApp, e: &EditorState) -> Option<SharedString> {
    use super::collectibles::Kind;
    match e.equipped.get(Kind::Nameplate) {
        Some(Some(sku)) => applied_item(e, Kind::Nameplate, sku)?.preview.clone(),
        Some(None) => None,
        None => app.me.nameplate.as_ref().map(|n| n.0.clone()),
    }
}

fn applied_item(e: &EditorState, kind: super::collectibles::Kind, sku: u64) -> Option<&super::collectibles::Collectible> {
    let shop = e.shop.as_ref().into_iter().flat_map(|s| s.nitro.iter().chain(&s.shop));
    e.collectibles.iter().flatten().chain(shop).find(|c| c.sku == sku && c.kind == kind)
}

fn decoration_url(e: &EditorState) -> Option<SharedString> {
    use super::collectibles::Kind;
    match e.equipped.get(Kind::Decoration) {
        Some(Some(sku)) => applied_item(e, Kind::Decoration, sku)?.preview.clone(),
        Some(None) => None,
        None => e.profile.as_ref()?.decoration.clone(),
    }
}

pub(super) fn bio_markup(t: SharedString) -> gpui::StyledText {
    let link = gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() };
    let mut hl = Vec::new();
    let mut i = 0;
    while let Some(off) = t[i..].find("http") {
        let st = i + off;
        let rest = &t[st..];
        if rest.starts_with("https://") || rest.starts_with("http://") {
            let len = rest.find(char::is_whitespace).unwrap_or(rest.len());
            hl.push((st..st + len, link));
            i = st + len;
        } else {
            i = st + 4;
        }
    }
    gpui::StyledText::new(t).with_highlights(hl)
}

fn tag_select((t, badge): (SharedString, Option<SharedString>), color: gpui::Hsla) -> Div {
    div()
        .flex_none()
        .h(z(24.))
        .pl(z(8.))
        .pr(z(6.))
        .flex()
        .items_center()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(2.))
                .text_size(z(12.))
                .line_height(z(16.))
                .not_italic()
                .text_color(color)
                .when_some(badge, |d, b| d.child(img(b).size(z(14.)).flex_none()))
                .child(t),
        )
        .child(gpui::svg().path(crate::icons::path("chevron")).size(z(16.)).ml(z(2.)).text_color(color))
}

pub(super) fn badges_row(p: Option<&Profile>) -> Option<Div> {
    let b = p.map(|p| p.badges.as_slice()).unwrap_or_default();
    (!b.is_empty()).then(|| {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(z(2.))
            .my(z(1.))
            .px(z(3.))
            .py(z(1.))
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .border_1()
            .border_color(c(tok(idx::BORDER_MUTED)))
            .children(b.iter().map(|(_, icon)| img(icon.clone()).size(z(20.)).flex_none()))
    })
}

fn lin(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn unlin(v: f32) -> f32 {
    if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1. / 2.4) - 0.055 }
}

fn luminance(rgb: u32) -> f32 {
    let ch = |s: u32| lin(((rgb >> s) & 0xff) as f32 / 255.);
    0.2126 * ch(16) + 0.7152 * ch(8) + 0.0722 * ch(0)
}

pub(crate) fn lab_shift(rgb: u32, d: f32) -> u32 {
    let ch = |s: u32| lin(((rgb >> s) & 0xff) as f32 / 255.);
    let (r, g, b) = (ch(16), ch(8), ch(0));
    let x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047;
    let y = 0.2126729 * r + 0.7151522 * g + 0.072175 * b;
    let zz = (0.0193339 * r + 0.119192 * g + 0.9503041 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856452 { t.cbrt() } else { t / 0.12841855 + 0.137_931_03 };
    let (fx, fy, fz) = (f(x), f(y), f(zz));
    let (l, a, bb) = (116. * fy - 16. + d, 500. * (fx - fy), 200. * (fy - fz));
    let fy = (l + 16.) / 116.;
    let inv = |t: f32| if t > 0.206_896_55 { t * t * t } else { 0.12841855 * (t - 0.137_931_03) };
    let (x, y, zz) = (0.95047 * inv(fy + a / 500.), inv(fy), 1.08883 * inv(fy - bb / 200.));
    let r = 3.2404542 * x - 1.5371385 * y - 0.4985314 * zz;
    let g = -0.969266 * x + 1.8760108 * y + 0.041556 * zz;
    let b = 0.0556434 * x - 0.2040259 * y + 1.0572252 * zz;
    let q = |v: f32| (unlin(v).clamp(0., 1.) * 255.).round() as u32;
    (q(r) << 16) | (q(g) << 8) | q(b)
}

pub(super) fn gradient_button_color(primary: u32) -> u32 {
    let contrast = |c: u32| 1.05 / (luminance(c) + 0.05);
    let mut c = primary;
    for _ in 0..100 {
        let k = contrast(c);
        if k < 7. {
            c = lab_shift(c, -18.);
        } else if k > 10. {
            c = lab_shift(c, 18.);
        } else {
            break;
        }
    }
    c
}

fn tabs(app: &PhoenixApp, e: &EditorState, cx: &mut Cx) -> Div {
    let themed = e.theme.is_some();
    let strong = if themed { white(0.96) } else { c(tok(idx::TEXT_STRONG)) };
    let dim = if themed { white(0.88) } else { c(tok(idx::TEXT_SUBTLE)) };
    let labels = ["pe.tab.board", "pe.tab.activity", "pe.tab.wishlist"];
    let bar = div().flex().gap(z(30.)).border_b_1().border_color(c(tok(idx::BORDER_SUBTLE))).children(labels.iter().enumerate().map(|(i, k)| {
        let sel = e.tab == i;
        div()
            .id(SharedString::from(format!("pe-tab-{i}")))
            .pb(z(16.))
            .border_b_2()
            .border_color(if sel { c(tok(idx::INTERACTIVE_TEXT_ACTIVE)) } else { gpui::transparent_black() })
            .text_size(z(14.))
            .line_height(z(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if sel { strong } else { dim })
            .cursor_pointer()
            .child(crate::t!(*k))
            .on_click(cx.listener(move |a, _, _, cx| {
                if let Some(e) = ed(a) {
                    e.tab = i;
                }
                cx.notify();
            }))
    }));
    let wcol = super::profile_widgets::WidgetColors {
        strong,
        muted: dim,
        card: if themed { gpui::hsla(240. / 360., 0.04, 0.61, 0.16) } else { c(tok(idx::BACKGROUND_MOD_SUBTLE)) },
        more: false,
    };
    let board: Vec<Div> = if e.tab == 0 { e.widgets.iter().map(|w| super::profile_widgets::render(w, &wcol)).collect() } else { Vec::new() };
    let panel = match e.tab {
        0 => div()
            .flex()
            .items_center()
            .justify_between()
            .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(dim).child(crate::t!("pe.widgets")))
            .child(controls::button_icon("pe-add-widget", "plus", crate::t!("pe.add_widget"), BtnKind::Secondary).h(z(31.)).min_w(z(0.)).px(z(11.)).text_size(z(14.)).line_height(z(18.)).on_click(cx.listener(|_, _, _, cx| {
                super::widget_add::open();
                cx.notify();
            }))),
        1 if !e.recent.is_empty() => super::profile_activity::render(
            &e.recent,
            true,
            &super::profile_activity::Colors { strong, muted: dim, card: wcol.card, link: c(tok(idx::TEXT_LINK)) },
        ),
        1 => div().text_size(z(14.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("pe.no_activity")),
        _ => {
            let board = crate::ui::profile_board::get(app, app.me.id, cx);
            crate::ui::profile::wishlist_panel(app, app.me.id, &board.wishlist, cx)
        }
    };
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .pt(z(20.))
        .child(div().pl(z(32.)).pr(z(16.)).child(bar))
        .child(div().pt(z(17.)).pl(z(32.)).pr(z(16.)).flex().flex_col().gap(z(12.)).child(panel).children(board))
}

fn unsaved_bar(e: &EditorState, cx: &mut Cx) -> Div {
    div()
        .absolute()
        .top(z(56.))
        .left(z(48.))
        .w(z(400.))
        .py(z(10.))
        .pl(z(16.))
        .pr(z(10.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .flex()
        .items_center()
        .justify_between()
        .gap(z(10.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .truncate()
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(e.error.clone().unwrap_or_else(|| crate::t!("pe.unsaved"))),
        )
        .child(
            div()
                .id("pe-reset")
                .text_size(z(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::TEXT_LINK)))
                .cursor_pointer()
                .child(crate::t!("pe.reset"))
                .on_click(cx.listener(|a, _, _, cx| reset(a, cx))),
        )
        .child(
            controls::button_sm("pe-save", if e.saving { crate::t!("pe.saving") } else { crate::t!("pe.save") }, BtnKind::Primary)
                .on_click(cx.listener(|a, _, _, cx| save(a, cx))),
        )
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Cx) -> AnyElement {
    let Some(s) = app.settings.as_ref() else { return div().into_any_element() };
    let Some(e) = s.profile.as_ref() else {
        cx.defer_in(window, |a, _, cx| {
            let orig = original_for(a, None, None);
            if let Some(s) = a.settings.as_mut()
                && s.profile.is_none()
            {
                let mut e = EditorState::new(None);
                e.original = orig;
                s.profile = Some(e);
            }
            fetch(a, cx);
            cx.notify();
        });
        return div().absolute().inset_0().bg(c(tok(idx::BACKGROUND_SCRIM))).into_any_element();
    };
    ensure_inputs(app, window, cx);
    let dirty = e.dirty(cx);
    let bio_focused = e.bio.as_ref().is_some_and(|i| gpui::Focusable::focus_handle(i.read(cx), cx).is_focused(window));
    let close = div()
        .id("pe-close")
        .absolute()
        .top(z(40.))
        .right(z(8.))
        .size(z(31.))
        .rounded(z(8.))
        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
        .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(gpui::svg().path(crate::icons::path("close")).size(z(16.)).text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
        .on_click(cx.listener(|a, _, _, cx| {
            if let Some(s) = a.settings.as_mut() {
                s.profile = None;
                s.page = Page::MyAccount;
            }
            cx.notify();
        }));
    let backdrop: Vec<AnyElement> = match e.theme {
        Some(t) => {
            let m = hex(modal_bg(t));
            let banner = e.profile.as_ref().and_then(|p| p.banner.clone()).map(|u| SharedString::from(u.replace("size=600", "size=1536")));
            let mut v: Vec<AnyElement> = vec![div().absolute().inset_0().bg(m).into_any_element()];
            if let Some(u) = banner {
                v.push(div().absolute().top_0().left_0().right_0().h(z(376.)).opacity(0.3).child(img(u).w_full().h(z(376.)).grayscale(false)).into_any_element());
                v.push(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(z(376.))
                        .bg(gpui::linear_gradient(180., gpui::linear_color_stop(m.opacity(0.), 0.), gpui::linear_color_stop(m, 280. / 376.)))
                        .into_any_element(),
                );
            }
            v.push(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(z(300.))
                    .bg(gpui::linear_gradient(180., gpui::linear_color_stop(gpui::hsla(0., 0., 0., 0.48), 0.), gpui::linear_color_stop(gpui::hsla(0., 0., 0., 0.12), 1.)))
                    .into_any_element(),
            );
            v.push(div().absolute().top(z(300.)).left_0().right_0().bottom_0().bg(gpui::hsla(0., 0., 0., 0.12)).into_any_element());
            v
        }
        None => vec![div().absolute().inset_0().bg(c(tok(idx::BACKGROUND_BASE_LOW))).child(div().size_full().bg(c(tok(idx::BACKGROUND_SCRIM)))).into_any_element()],
    };
    if e.modal {
        let vp = window.viewport_size();
        let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
        let editing = !e.collapsed && vw > 928.;
        let mb = crate::ui::profile::modal_box(vw, vh, editing);
        let pad = if vw >= 1025. { 48. } else { 32. };
        let r = (mb.radius - 1.).max(0.);
        let close = div()
            .id("pe-close")
            .absolute()
            .top(z(8. + mb.system_bar))
            .right(z(8.))
            .size(z(32.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
            .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
            .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(gpui::svg().path(crate::icons::path("close")).size(z(16.)).text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
            .on_click(cx.listener(|a, _, _, cx| {
                a.settings = None;
                cx.notify();
            }));
        let right = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .rounded_r(z(r))
            .when(!editing, |d| d.rounded_l(z(r)).when(mb.border_inline, |d| d.border_l_1()))
            .when(mb.border_block, |d| d.border_t_1().border_b_1())
            .when(mb.border_inline, |d| d.border_r_1())
            .border_color(c(tok(idx::BORDER_NORMAL)))
            .children(backdrop)
            .child(
                div()
                    .relative()
                    .size_full()
                    .pt(z(pad + mb.system_bar))
                    .pr(z(32.))
                    .pl(z(pad))
                    .flex()
                    .child(card(app, e, bio_focused, cx))
                    .child(tabs(app, e, cx)),
            )
            .when(dirty || e.error.is_some(), |d| d.child(unsaved_bar(e, cx)))
            .child(close);
        let show_styles = (!editing && vw > 928.).then(|| {
            div()
                .id("pe-show-styles")
                .absolute()
                .top(z(40.))
                .left(z(-30.))
                .w(z(40.))
                .h(z(40.))
                .pr(z(10.))
                .rounded_l(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_NORMAL)))
                .bg(c(tok(idx::MODAL_BACKGROUND)))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(gpui::svg().path(crate::icons::path("paintbrush-thin")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                .tooltip(crate::ui::tooltip::tip(crate::t!("pe.show_styles")))
                .on_click(cx.listener(|a, _, _, cx| {
                    if let Some(e) = ed(a) {
                        e.collapsed = false;
                    }
                    cx.notify();
                }))
        });
        return div()
            .id("profile-editor")
            .absolute()
            .inset_0()
            .bg(c(tok(idx::BACKGROUND_SCRIM)))
            .occlude()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|a, _, _, cx| {
                    a.settings = None;
                    cx.notify();
                }),
            )
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("profile-editor-modal")
                    .relative()
                    .w(px(mb.w))
                    .h(px(mb.h))
                    .flex()
                    .occlude()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .shadow_lg()
                    .children(show_styles)
                    .when(editing, |d| d.child(styles_panel(app, e, Some(mb), cx)))
                    .child(right)
                    .when(e.switcher_open, |d| d.child(switcher(app, e, cx))),
            )
            .children(super::collectible_picker::render(app, e, cx))
            .into_any_element();
    }
    let right = div()
        .relative()
        .flex_1()
        .min_w_0()
        .h_full()
        .overflow_hidden()
        .children(backdrop)
        .child(
            div()
                .relative()
                .size_full()
                .pt(z(80.))
                .pr(z(32.))
                .pl(z(48.))
                .flex()
                .child(card(app, e, bio_focused, cx))
                .child(tabs(app, e, cx)),
        )
        .when(dirty || e.error.is_some(), |d| d.child(unsaved_bar(e, cx)));
    div()
        .id("profile-editor")
        .absolute()
        .inset_0()
        .bg(c(tok(idx::BACKGROUND_SCRIM)))
        .occlude()
        .flex()
        .justify_center()
        .child(
            div()
                .relative()
                .size_full()
                .max_w(z(1184.))
                .mx(z(48.))
                .flex()
                .child(styles_panel(app, e, None, cx))
                .child(right)
                .child(close)
                .when(e.switcher_open, |d| d.child(switcher(app, e, cx))),
        )
        .children(super::collectible_picker::render(app, e, cx))
        .children(super::widget_add::modal(e.widgets.iter().map(|w| w.app_id).collect(), cx))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_color_matches_client() {
        let got = gradient_button_color(0x00A7FF);
        for sh in [16, 8, 0] {
            assert!((((got >> sh) & 0xff) as i32 - ((0x004D9A >> sh) & 0xff)).abs() <= 1, "{got:06x}");
        }
    }

    #[test]
    fn diff_only_sends_changed_fields() {
        let o = Draft { name: "Rox".into(), pronouns: "".into(), bio: "hi".into(), theme: None };
        let mut c = o.clone();
        assert!(diff(&o, &c, None, None).is_empty());
        c.pronouns = "il/lui".into();
        c.theme = Some((1, 2));
        let p = diff(&o, &c, Some("data:x"), None);
        assert_eq!(p.pronouns.as_deref(), Some("il/lui"));
        assert_eq!(p.display_name, None);
        assert_eq!(p.bio, None);
        assert_eq!(p.theme_colors, Some((1, 2)));
        assert_eq!(p.avatar, Some(Some("data:x".into())));
    }

    #[test]
    fn guild_patch_goes_to_member_endpoint_body() {
        let p = ProfilePatch { display_name: Some("Nick".into()), bio: Some("b".into()), ..Default::default() };
        let (acct, prof) = crate::discord_backend::profile_edit_bodies(true, &p);
        assert_eq!(acct.get("nick").and_then(|v| v.as_str()), Some("Nick"));
        assert_eq!(acct.get("bio").and_then(|v| v.as_str()), Some("b"));
        assert!(prof.is_empty());
        let (acct, prof) = crate::discord_backend::profile_edit_bodies(false, &p);
        assert_eq!(acct.get("global_name").and_then(|v| v.as_str()), Some("Nick"));
        assert_eq!(prof.get("bio").and_then(|v| v.as_str()), Some("b"));
    }
}

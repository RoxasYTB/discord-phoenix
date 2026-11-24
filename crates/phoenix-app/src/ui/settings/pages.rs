//! Declarative content of the settings pages, reproduced from the live 2026
//! client (CDP outlines of each page, 2026-11-23): every page is a list of
//! [`B`] blocks rendered with the client's control layout.
//!
//! Layout values (panel_nNq2Vb children):
//! * category: heading-xl/medium 24/32 title, optional body-sm lead 4px
//!   under it, then container_Gl6U0S rows (padding --space-xs/--space-sm,
//!   margin negated) holding baseControlItem_FiiQ6I (padding/gap 12,
//!   min-height 48, radius --radius-lg; `.clickable` hover
//!   --interactive-background-hover).
//! * categories are split by divider_ZaYWLb (1px, margin 50px 0).
//! * legends: body-md/semibold 16/20 (30px box); keybind groups body-lg/medium 20/24.
//! * key caps (key_dMUH0f): --background-mod-muted, 1px --border-strong,
//!   radius 4, height 23, padding 3 6 4, 12/600 text, 3px apart.
//!
//! Settings with no backing preference are kept for the session only
//! (`SettingsState::toggles` / `picks`) so the page behaves like the client
//! without pretending to sync anything.

use gpui::{AnyElement, Context, Div, ElementId, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use super::{AppTheme, LocalPrefs, Page, SettingsPatch, SyncedSettings, save, update_local};
use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{self, BtnKind},
};

type Cx<'a> = Context<'a, PhoenixApp>;
type Render = fn(&PhoenixApp, &mut Context<'_, PhoenixApp>) -> AnyElement;

/// Where a switch reads/writes its value.
#[derive(Clone, Copy)]
pub enum Bind {
    /// Session-only, with the client's default.
    Ui(bool),
    Local(fn(&LocalPrefs) -> bool, fn(&mut LocalPrefs, bool)),
    Synced(fn(&SyncedSettings) -> bool, fn(bool) -> SettingsPatch),
    /// Synced through settings-proto ([`super::synced_map`] entry = the id).
    Proto,
    /// Client-only, saved in `LocalPrefs::ui` under the id; client default.
    Pref(bool),
}

/// Where a radio group / select reads/writes its index.
#[derive(Clone, Copy)]
pub enum Pick {
    Ui(usize),
    Synced(fn(&SyncedSettings) -> usize, fn(usize) -> SettingsPatch),
    Proto,
    Pref(usize),
}

/// One block of a settings page. Strings are i18n keys ("" = none).
pub enum B {
    Title(&'static str),
    Lead(&'static str),
    Legend(&'static str),
    Switch(&'static str, &'static str, &'static str, Bind),
    /// Switch whose description is the "Sound preview" link.
    Sound(&'static str, &'static str),
    Radios(&'static str, &'static str, &'static str, &'static [(&'static str, &'static str)], Pick),
    Select(&'static str, &'static str, &'static str, &'static [&'static str], Pick),
    Action(&'static str, &'static str, &'static str, bool),
    Related(&'static str, &'static str, &'static str, Page),
    /// Collapsed "Show advanced ..." row revealing more blocks.
    Expand(&'static str, &'static str, &'static str, &'static [B]),
    KeyGroup(&'static str, &'static str, &'static [(&'static str, &'static [&'static str])]),

    Divider,
    /// Custom control; the key is used for search.
    Custom(&'static str, Render),
}

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

fn tr(k: &str) -> SharedString {
    crate::t!(k)
}

// ---------------------------------------------------------------- rendering

fn title_text(t: SharedString) -> Div {
    div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(t)
}

fn desc_text(t: SharedString) -> Div {
    // body-sm/normal 14/18 --text-subtle
    div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(t)
}

/// baseControlItem row with the trailing control aligned to the title line.
fn item(title: SharedString, desc: Option<AnyElement>, trailing: Option<AnyElement>, top: bool) -> Div {
    item_ex(title, desc, trailing, top, false)
}

/// [`item`] with the disabled look (`dim`: title at opacity .5).
fn item_ex(title: SharedString, desc: Option<AnyElement>, trailing: Option<AnyElement>, top: bool, dim: bool) -> Div {
    div()
        .flex()
        .gap(z(12.))
        .mx(z(-12.))
        .px(z(12.))
        // vanilla-settings-13 #2688 baseSettingWrapper_4Z: switch rows pad
        // 10px 0 (no 48px floor); other rows keep baseControlItem's 12 / 48.
        .when(top, |d| d.py(z(10.)))
        .when(!top, |d| d.py(z(12.)).min_h(z(48.)))
        .rounded(z(16.)) // --radius-lg
        // vanilla-settings-02 #2700 description_VhFsFg: switch rows wrap
        // their description at 572px, other rows use the full 696.
        .child(div().flex_1().min_w_0().flex().flex_col().justify_center().gap(z(4.)).child(title_text(title).when(dim, |d| d.opacity(0.5))).children(desc.map(|d| div().when(top, |d| d.max_w(z(572.))).child(d))))
        .children(trailing.map(|t| {
            let d = div().flex_none().flex();
            if top { d.items_start().child(t) } else { d.items_center().child(t) }
        }))
}

fn get_bool(app: &PhoenixApp, id: &str, b: Bind) -> bool {
    match b {
        Bind::Ui(d) => app.settings.as_ref().and_then(|s| s.toggles.get(id).copied()).unwrap_or(d),
        Bind::Local(g, _) => g(&app.prefs.local),
        Bind::Synced(g, _) => g(&app.prefs.synced),
        Bind::Proto => app.prefs.synced.flag(id),
        Bind::Pref(d) => app.prefs.local.flag(id, d),
    }
}

fn set_bool(app: &mut PhoenixApp, id: &'static str, b: Bind, v: bool, cx: &mut Cx) {
    match b {
        Bind::Ui(_) => {
            if let Some(s) = app.settings.as_mut() {
                s.toggles.insert(id, v);
            }
            cx.notify();
        }
        Bind::Local(_, s) => update_local(app, |p| s(p, v), cx),
        Bind::Synced(_, p) => save(app, p(v), cx),
        Bind::Proto if super::synced_map::rest(id).is_some() => save(app, SettingsPatch::Rest(id, v), cx),
        Bind::Proto => {
            let w = super::synced_map::bool_wire(&app.prefs.synced.proto, id, v);
            save(app, SettingsPatch::Proto(id, w), cx)
        }
        Bind::Pref(_) => update_local(app, |p| {
            p.ui.insert(id.to_owned(), v as u64);
        }, cx),
    }
}

fn get_pick(app: &PhoenixApp, id: &str, p: Pick) -> usize {
    match p {
        Pick::Ui(d) => app.settings.as_ref().and_then(|s| s.picks.get(id).copied()).unwrap_or(d),
        Pick::Synced(g, _) => g(&app.prefs.synced),
        Pick::Proto => app.prefs.synced.pick(id),
        Pick::Pref(d) => app.prefs.local.pick(id, d),
    }
}

fn set_pick(app: &mut PhoenixApp, id: &'static str, p: Pick, v: usize, cx: &mut Cx) {
    if let Some(s) = app.settings.as_mut() {
        s.open_select = None;
    }
    match p {
        Pick::Ui(_) => {
            if let Some(s) = app.settings.as_mut() {
                s.picks.insert(id, v);
            }
            cx.notify();
        }
        Pick::Synced(_, f) => save(app, f(v), cx),
        Pick::Proto => save(app, SettingsPatch::Proto(id, super::synced_map::pick_wire(id, v)), cx),
        Pick::Pref(_) => update_local(app, |p| {
            p.ui.insert(id.to_owned(), v as u64);
        }, cx),
    }
}

fn switch_row(app: &PhoenixApp, id: &'static str, title: SharedString, desc: Option<AnyElement>, b: Bind, cx: &mut Cx) -> AnyElement {
    let synced = &app.prefs.synced;
    let disabled = matches!(b, Bind::Proto) && super::synced_map::disabled(&synced.proto, synced.compact, id);
    // APPEARANCE_DISPLAY_COMPACT_AVATARS reads `!compact || display_compact_avatars`.
    let on = get_bool(app, id, b) || (id == "avatars" && !synced.compact);
    if disabled {
        // container_VhFsFg[data-disabled=true]: label + description at .5,
        // the indicator at .5 too, cursor not-allowed, no toggle.
        let row = item_ex(
            title,
            desc.map(|d| div().opacity(0.5).child(d).into_any_element()),
            Some(div().opacity(0.5).child(controls::switch(eid(format!("sw-knob-{id}")), on)).into_any_element()),
            true,
            true,
        );
        return row.id(eid(format!("sw-{id}"))).cursor(gpui::CursorStyle::OperationNotAllowed).into_any_element();
    }
    item(title, desc, Some(controls::switch(eid(format!("sw-knob-{id}")), on).into_any_element()), true)
        .id(eid(format!("sw-{id}")))
        .group(controls::SWITCH_ROW)
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| set_bool(a, id, b, !on, cx)))
        .into_any_element()
}

/// Dropdown field + its listBox (listBox_b5PEJH: padding 4, 40px options,
/// radius 4) floating under it while open.
pub(super) fn dropdown(
    app: &PhoenixApp,
    id: &'static str,
    opts: Vec<SharedString>,
    cur: usize,
    width: f32,
    on_pick: impl Fn(&mut PhoenixApp, usize, &mut Cx) + Clone + 'static,
    cx: &mut Cx,
) -> AnyElement {
    let field = controls::select_field(eid(format!("sel-{id}")), opts.get(cur).cloned().unwrap_or_default());
    dropdown_with(app, id, field, opts, cur, width, on_pick, cx)
}

/// [`dropdown`] around a custom closed `field` (e.g. the ghost select of
/// the content filters).
#[allow(clippy::too_many_arguments)]
fn dropdown_with(
    app: &PhoenixApp,
    id: &'static str,
    field: gpui::Stateful<Div>,
    opts: Vec<SharedString>,
    cur: usize,
    width: f32,
    on_pick: impl Fn(&mut PhoenixApp, usize, &mut Cx) + Clone + 'static,
    cx: &mut Cx,
) -> AnyElement {
    let open = app.settings.as_ref().is_some_and(|s| s.open_select == Some(id));
    let field = field
        .w(z(width))
        // selectField_ :focus-within: 2px outline --input-border-active, offset -2
        .when(open, |d| {
            d.relative().child(
                div().absolute().top(z(-1.)).bottom(z(-1.)).left(z(-1.)).right(z(-1.)).rounded(z(8.)).border_2().border_color(c(tok(idx::INPUT_BORDER_ACTIVE))),
            )
        })
        .on_click(cx.listener(move |a, _, _, cx| {
            if let Some(s) = a.settings.as_mut() {
                s.open_select = if s.open_select == Some(id) { None } else { Some(id) };
            }
            cx.notify();
        }));
    let mut wrap = div().relative().child(field);
    if open {
        let list = div()
            .id(eid(format!("sel-{id}-list")))
            .absolute()
            .top(z(44.))
            .left_0()
            .w(z(width))
            .max_h(z(320.))
            .overflow_y_scroll()
            .p(z(4.))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHER)))
            .shadow_lg()
            .occlude()
            .children(opts.into_iter().enumerate().map(|(i, o)| {
                let f = on_pick.clone();
                div()
                    .id(eid(format!("sel-{id}-{i}")))
                    .h(z(40.))
                    .px(z(8.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .when(i == cur, |d| d.bg(c(tok(idx::INTERACTIVE_BACKGROUND_SELECTED))))
                    .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                    .on_click(cx.listener(move |a, _, _, cx| f(a, i, cx)))
                    .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(o))
                    .when(i == cur, |d| {
                        d.child(gpui::svg().path(crate::icons::path("check")).size(z(16.)).text_color(c(tok(idx::CONTROL_BRAND_FOREGROUND_NEW))))
                    })
            }));
        wrap = wrap.child(gpui::deferred(list).with_priority(2));
    }
    wrap.into_any_element()
}

fn radio_group(app: &PhoenixApp, id: &'static str, opts: &'static [(&'static str, &'static str)], p: Pick, cx: &mut Cx) -> Div {
    let cur = get_pick(app, id, p);
    // group_oJuRbG: options 36px apart (indicator 20, text at 32px)
    div().flex().flex_col().children(opts.iter().enumerate().map(|(i, (label, desc))| {
        div()
            .id(eid(format!("rg-{id}-{i}")))
            .py(z(6.))
            .flex()
            .gap(z(12.))
            .cursor_pointer()
            .on_click(cx.listener(move |a, _, _, cx| set_pick(a, id, p, i, cx)))
            .child(div().pt(z(1.)).child(controls::radio_indicator(i == cur)))
            .child(
                div()
                    .flex_1()
                    // let long descriptions wrap (vanilla-settings-02 #2841: 664px, 2 lines)
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(z(4.))
                    .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(tr(label)))
                    .when(!desc.is_empty(), |d| d.child(desc_text(tr(desc)))),
            )
    }))
}

fn key_cap(k: &'static str) -> Div {
    div()
        .h(z(23.))
        .min_w(z(14.))
        .pt(z(3.))
        .pb(z(4.))
        .px(z(6.))
        .rounded(z(4.))
        .border_1()
        .border_color(c(tok(idx::BORDER_STRONG)))
        .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
        .flex()
        .items_center()
        .justify_center()
        .text_size(z(12.))
        .line_height(z(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(tok(idx::TEXT_STRONG)))
        .child(k)
}

fn key_group(legend: &'static str, lead: &'static str, rows: &'static [(&'static str, &'static [&'static str])]) -> Div {
    let n = rows.len();
    div()
        .flex()
        .flex_col()
        .when(!legend.is_empty(), |d| {
            d.child(
                div()
                    .pt(z(16.))
                    .text_size(z(20.))
                    .line_height(z(24.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_STRONG)))
                    .child(tr(legend)),
            )
        })
        .when(!lead.is_empty(), |d| d.child(div().pt(z(4.)).child(desc_text(tr(lead)))))
        .child(div().mt(z(16.)).rounded(z(8.)).border_1().border_color(c(tok(idx::BORDER_SUBTLE))).children(rows.iter().enumerate().map(
            |(i, (action, keys))| {
                div()
                    .px(z(16.))
                    .min_h(z(56.))
                    .flex()
                    .items_center()
                    .gap(z(6.))
                    .when(i + 1 < n, |d| d.border_b_1().border_color(c(tok(idx::BORDER_SUBTLE))))
                    .child(div().flex_1().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(tr(action)))
                    .child(div().flex().gap(z(3.)).children(keys.iter().map(|k| key_cap(k))))
            },
        )))
}

/// Related-row description; "Confidentialité des activités" appends the
/// servers the activity is shared with, in bold (vanilla-settings-02 #2883
/// strong: "Aperture Sciences et 41 autres serveurs").
fn related_desc(app: &PhoenixApp, title: &str, desc: &'static str) -> AnyElement {
    if title != "settings.page.activity" {
        return desc_text(tr(desc)).into_any_element();
    }
    let restricted = super::account::activity_restricted();
    let shared: Vec<&crate::model::Guild> = app.guilds.iter().filter(|g| !restricted.contains(&g.id.0)).collect();
    let tail = match shared.as_slice() {
        [] => crate::t!("sp.activity_shared_none"),
        [g] => crate::t!("sp.activity_shared_one", guild = g.name),
        [g, _] => crate::t!("sp.activity_shared_two", guild = g.name),
        [g, rest @ ..] => crate::t!("sp.activity_shared_many", guild = g.name, n = rest.len()),
    };
    super::account::bold_desc(format!("{} {tail}", tr("sp.activity_related_d")).into())
}

fn related(app: &PhoenixApp, icon: &'static str, title: &'static str, desc: &'static str, page: Page, cx: &mut Cx) -> AnyElement {
    let lead = div()
        .w(z(48.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        // vanilla-settings-13 #2733 icon_W6yko_: 48px tile, --radius-sm,
        // --background-mod-muted, 24px glyph.
        .child(
            div()
                .size(z(48.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
                .flex()
                .items_center()
                .justify_center()
                .child(gpui::svg().path(crate::icons::path(icon)).size(z(24.)).text_color(c(tok(idx::ICON_DEFAULT)))),
        );
    div()
        .id(eid(format!("rel-{title}")))
        .flex()
        .gap(z(12.))
        .mx(z(-12.))
        .p(z(12.))
        .min_h(z(48.))
        .rounded(z(16.))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
        .on_click(cx.listener(move |a, _, _, cx| {
            if let Some(s) = a.settings.as_mut() {
                s.page = page;
                s.open_select = None;
            }
            cx.notify();
        }))
        .child(lead)
        .child(div().flex_1().min_w_0().flex().flex_col().gap(z(4.)).child(title_text(tr(title))).child(related_desc(app, title, desc)))
        .child(
            div().flex_none().flex().items_center().child(
                gpui::svg().path(crate::icons::path("chevron-right")).size(z(24.)).text_color(c(tok(idx::ICON_DEFAULT))),
            ),
        )
        .into_any_element()
}

fn render_block(app: &PhoenixApp, b: &'static B, out: &mut Vec<AnyElement>, cx: &mut Cx) {
    match b {
        B::Title(t) => out.push(controls::section_title(tr(t)).into_any_element()),
        // Lead sits 4px under the 32px heading (vanilla-settings-14b #3019→#3021): pb 24 + column gap 10 - 30.
        B::Lead(t) => out.push(div().mt(z(-30.)).pb(z(22.)).child(desc_text(tr(t))).into_any_element()),
        B::Legend(t) => out.push(
            div()
                .pt(z(10.))
                .pb(z(4.))
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(tr(t))
                .into_any_element(),
        ),
        B::Switch(id, t, d, bind) => {
            let desc = (!d.is_empty()).then(|| desc_text(tr(d)).into_any_element());
            out.push(switch_row(app, id, tr(t), desc, *bind, cx));
        }
        B::Sound(id, t) => {
            // the preview link (anchor_R6ReNl, 14/18 --text-link)
            let link = div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_LINK))).child(tr("sp.sound_preview"));
            out.push(switch_row(app, id, tr(t), Some(link.into_any_element()), Bind::Pref(*id != "snd-msg-cur"), cx)); // vanilla-settings-04: current-channel sound off
        }
        B::Radios(id, t, d, opts, p) => {
            let mut col = div().flex().flex_col().py(z(12.));
            if !t.is_empty() {
                col = col.child(title_text(tr(t)).pb(z(if d.is_empty() { 8. } else { 4. })));
            }
            if !d.is_empty() {
                col = col.child(desc_text(tr(d)).pb(z(8.)));
            }
            out.push(col.child(radio_group(app, id, opts, *p, cx)).into_any_element());
        }
        B::Select(id, t, d, opts, p) => {
            let cur = get_pick(app, id, *p);
            let (id, p) = (*id, *p);
            let opts: Vec<SharedString> = opts.iter().map(|o| tr(o)).collect();
            let dd = dropdown(app, id, opts, cur, 280., move |a, i, cx| set_pick(a, id, p, i, cx), cx);
            let desc = (!d.is_empty()).then(|| desc_text(tr(d)).into_any_element());
            out.push(item(tr(t), desc, Some(dd), false).into_any_element());
        }
        B::Action(t, d, label, danger) => {
            let kind = BtnKind::Secondary;
            let btn = controls::button(eid(format!("act-{t}")), tr(label), kind)
                .when(*danger, |b| b.text_color(c(tok(idx::TEXT_FEEDBACK_CRITICAL))))
                .into_any_element();
            let desc = (!d.is_empty()).then(|| desc_text(tr(d)).into_any_element());
            out.push(item(tr(t), desc, Some(btn), false).into_any_element());
        }
        B::Related(icon, t, d, page) => {
            {
                out.push(
                    div()
                        .pt(z(12.))
                        .pb(z(2.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_MUTED)))
                        .child(tr("sp.related"))
                        .into_any_element(),
                );
            }
            out.push(related(app, icon, t, d, *page, cx));
        }
        B::Expand(id, t, d, inner) => {
            let open = app.settings.as_ref().is_some_and(|s| s.expanded.contains(id));
            let id = *id;
            out.push(
                item(
                    tr(t),
                    Some(desc_text(tr(d)).into_any_element()),
                    // vanilla-settings-07 #2828 triggerButton_MdUQoo: 48px
                    // --background-mod-subtle tile, --radius-sm, 24px
                    // --icon-strong chevron (down closed, up open).
                    Some(
                        div()
                            .size(z(48.))
                            .rounded(z(8.))
                            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                gpui::svg()
                                    .path(crate::icons::path("chevron"))
                                    .size(z(24.))
                                    .text_color(c(tok(idx::ICON_STRONG)))
                                    .when(open, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI)))),
                            )
                            .into_any_element(),
                    ),
                    false,
                )
                .id(eid(format!("exp-{id}")))
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .on_click(cx.listener(move |a, _, _, cx| {
                    if let Some(s) = a.settings.as_mut()
                        && !s.expanded.remove(id)
                    {
                        s.expanded.insert(id);
                    }
                    cx.notify();
                }))
                .into_any_element(),
            );
            if open {
                for b in inner.iter() {
                    render_block(app, b, out, cx);
                }
            }
        }
        B::KeyGroup(l, d, rows) => out.push(key_group(l, d, rows).into_any_element()),
        B::Divider => out.push(div().my(z(50.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))).into_any_element()),
        B::Custom(_, f) => out.push(f(app, cx)),
    }
}

/// Renders a page's blocks (rows 10px apart like the stacked containers).
pub fn render(app: &PhoenixApp, blocks: &'static [B], cx: &mut Cx) -> Div {
    let mut out = Vec::new();
    for b in blocks {
        render_block(app, b, &mut out, cx);
    }
    div().flex().flex_col().gap(z(10.)).children(out)
}

// ------------------------------------------------------------------ search

fn push_texts(blocks: &'static [B], out: &mut Vec<&'static str>) {
    for b in blocks {
        match b {
            B::Title(t) | B::Lead(t) | B::Legend(t) | B::Sound(_, t) | B::Custom(t, _) => out.push(t),
            B::Switch(_, t, d, _) | B::Action(t, d, _, _) | B::Related(_, t, d, _) => out.extend([*t, *d]),
            B::Radios(_, t, d, o, _) => {
                out.extend([*t, *d]);
                out.extend(o.iter().flat_map(|(a, b)| [*a, *b]));
            }
            B::Select(_, t, d, o, _) => {
                out.extend([*t, *d]);
                out.extend(o.iter().copied());
            }
            B::Expand(_, t, d, inner) => {
                out.extend([*t, *d]);
                push_texts(inner, out);
            }
            B::KeyGroup(l, d, rows) => {
                out.extend([*l, *d]);
                out.extend(rows.iter().map(|r| r.0));
            }
            B::Divider => {}
        }
    }
}

/// Category titles of a page (sidebar sub-navigation under the active item).
pub fn titles(p: Page) -> Vec<SharedString> {
    blocks(p).iter().filter_map(|b| if let B::Title(t) = b { Some(tr(t)) } else { None }).collect()
}

/// True when the page label or any of its texts contains `q` (lowercase).
pub fn matches(p: Page, q: &str) -> bool {
    if q.is_empty() || p.label().to_lowercase().contains(q) {
        return true;
    }
    let mut keys = Vec::new();
    push_texts(blocks(p), &mut keys);
    keys.extend(extra_search(p).iter().copied());
    keys.iter().filter(|k| !k.is_empty()).any(|k| tr(k).to_lowercase().contains(q))
}

fn extra_search(p: Page) -> &'static [&'static str] {
    match p {
        Page::MyAccount => &[
            "settings.account.info",
            "settings.account.display_name",
            "settings.account.username",
            "settings.account.email",
            "settings.account.phone",
            "settings.account.security",
            "settings.account.password",
            "settings.account.status",
        ],
        Page::Profiles => &["sp.pf.nameplate", "sp.pf.avatar", "sp.pf.name_style", "sp.pf.theme", "sp.pf.effect"],
        Page::VoiceVideo => &[
            "sp.vo.t1",
            "sp.vo.t2",
            "voice.input_device",
            "voice.output_device",
            "voice.input_volume",
            "voice.output_volume",
            "voice.mic_test",
            "voice.input_mode",
            "voice.sensitivity",
            "voice.echo",
            "voice.noise",
            "voice.agc",
            "voice.attenuation",
        ],
        Page::Devices => &["settings.devices.current"],
        _ => &[],
    }
}

// ------------------------------------------------------------------- pages

const NONE: &[B] = &[];

pub fn blocks(p: Page) -> &'static [B] {
    match p {
        Page::DataPrivacy => DATA,
        Page::PrivacySafety => CONTENT,
        Page::Notifications => NOTIF,
        Page::Appearance => APPEARANCE,
        Page::Accessibility => A11Y,
        Page::System | Page::Keybinds => SYSTEM,
        Page::Language => LANGUAGE,
        Page::ActivityPrivacy => ACTIVITY,
        Page::Connections => CONNECTIONS,
        Page::Advanced => DEVELOPER,
        Page::Profiles => PROFILES,
        Page::RegisteredGames => GAMES,
        Page::Nitro => NITRO,
        Page::ServerBoost => BOOST,
        Page::Subscriptions => SUBSCRIPTIONS,
        Page::GiftInventory => GIFTS,
        Page::Billing => BILLING,
        _ => NONE,
    }
}

/// "Filtres de contenu sensible" (vanilla-settings-03 #2700-#2749): header
/// (16/500 title, 14/18 description + link), then tabbedSettingsContainer_
/// (gap 48): a 226px vertical tabBar_ (40px tabs, padding 10 8, 2px
/// --text-brand left border when selected, else 1px --border-subtle and
/// --text-subtle) beside the panel of selectItemRow_ rows (49px, 1px bottom
/// border) with a borderless 140px select (16px state icon + label, 18px
/// chevron) and the tab's 14/18 --text-muted note.
fn content_filters(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    const TABS: [(&str, &str); 2] = [("sp.cs.tab_sexual", "sp.cs.tab_sexual_d"), ("sp.cs.tab_violent", "sp.cs.tab_violent_d")];
    const ROWS: [[(&str, &str, usize); 3]; 2] = [
        [("cf-friends", "sp.cs.f_friends", 0), ("cf-others", "sp.cs.f_others", 2), ("cf-servers", "sp.cs.f_servers", 0)],
        [("cfv-friends", "sp.cs.f_friends", 0), ("cfv-others", "sp.cs.f_others", 1), ("cfv-servers", "sp.cs.f_servers", 0)],
    ];
    const ICONS: [&str; 3] = ["chat/eye", "blur", "deny"];
    let tab = get_pick(app, "cf-tab", Pick::Ui(0)).min(1);
    let header = div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(title_text(tr("sp.cs.sensitive")))
        .child(super::account::linked_desc(format!("{} [{}]", tr("sp.cs.sensitive_d"), tr("sp.learn_more")).into()));
    let tabs = div().w(z(226.)).flex_none().pt(z(12.)).flex().flex_col().gap(z(16.)).children(TABS.iter().enumerate().map(|(i, (t, _))| {
        let on = i == tab;
        div()
            .id(eid(format!("cf-tab-{i}")))
            .h(z(40.))
            .px(z(16.))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(z(16.))
            .line_height(z(20.))
            .when(on, |d| d.border_l_2().border_color(c(tok(idx::TEXT_BRAND))).text_color(c(tok(idx::TEXT_BRAND))))
            .when(!on, |d| {
                d.border_l_1()
                    .border_color(c(tok(idx::BORDER_SUBTLE)))
                    .text_color(c(tok(idx::TEXT_SUBTLE)))
                    .hover(|s| s.text_color(c(tok(idx::TEXT_DEFAULT))))
            })
            .on_click(cx.listener(move |a, _, _, cx| set_pick(a, "cf-tab", Pick::Ui(0), i, cx)))
            .child(tr(t))
    }));
    let opts: Vec<SharedString> = SHOW_BLOCK.iter().map(|o| tr(o)).collect();
    let rows = ROWS[tab].iter().map(|&(id, label, def)| {
        // settings-proto explicit/gore_content_settings (synced_map entries).
        let pk = if super::synced_map::entry(id).is_some() { Pick::Proto } else { Pick::Ui(def) };
        let cur = get_pick(app, id, pk).min(2);
        let field = div()
            .id(eid(format!("sel-{id}")))
            .h(z(40.))
            .pl(z(12.))
            .pr(z(8.))
            .flex()
            .items_center()
            .gap(z(8.))
            .rounded(z(8.))
            .cursor_pointer()
            .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(z(8.))
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .child(gpui::svg().path(crate::icons::path(ICONS[cur])).size(z(16.)).text_color(c(tok(idx::TEXT_DEFAULT))))
                    .child(opts[cur].clone()),
            )
            .child(gpui::svg().path("icons/chevron.svg").size(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))));
        let dd = dropdown_with(app, id, field, opts.clone(), cur, 140., move |a, i, cx| set_pick(a, id, pk, i, cx), cx);
        div()
            .h(z(49.))
            .py(z(4.))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .child(div().flex_1().min_w_0().child(title_text(tr(label))))
            .child(dd)
    });
    let panel = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(div().flex().flex_col().children(rows))
        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(tr(TABS[tab].1)));
    div()
        .py(z(10.))
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(header)
        .child(div().flex().gap(z(48.)).child(tabs).child(panel))
        .into_any_element()
}

// "Jeux enregistrés" (vanilla-settings-12 dump): Current Game / Added Games
// with the IGDB note, then Related Settings → Activity Privacy.
static GAMES: &[B] = &[
    B::Title("sp.rg.current"),
    B::Custom("sp.rg.none", current_game),
    B::Divider,
    B::Title("sp.rg.added"),
    B::Lead("sp.rg.igdb"),
    B::Divider,
    B::Related("user-profile", "settings.page.activity", "sp.rg.related_d", Page::ActivityPrivacy),
];

/// vanilla-settings-12 #2686 `notDetected_kQPUsd`: --background-mod-subtle
/// box, padding 16, radius 12 (computed), game name 16/22 semibold
/// --interactive-text-active (#2688), then "Not seeing your game?" 14 --text-muted
/// + "Add it!" body-sm/medium --text-brand text button (#2689-#2692, gap 4).
fn current_game(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(z(4.)) // gameNameLastPlayed_ gap
        .p(z(16.)) // .notDetected_ padding
        .rounded(z(12.)) // #2686 border-radius
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))) // .notDetected_ background-color
        .child(
            div()
                .text_size(z(16.))
                .line_height(z(22.))
                .font_weight(FontWeight::SEMIBOLD) // .gameName_ font-weight
                .text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))
                .child(tr("sp.rg.none")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.)) // nowPlayingAdd_ gap
                .text_size(z(14.))
                .line_height(z(18.))
                .child(div().text_color(c(tok(idx::TEXT_MUTED))).child(tr("sp.rg.cant_find")))
                .child(
                    div()
                        .id("rg-add")
                        .cursor_pointer()
                        .font_weight(FontWeight::MEDIUM) // body-sm/medium
                        .text_color(c(tok(idx::TEXT_BRAND))) // .primary_nJgowo color
                        .child(tr("sp.rg.add")),
                ),
        )
        .into_any_element()
}

// Billing pages: Phoenix doesn't handle payments, each page points to the web.
static NITRO: &[B] = &[B::Custom("sp.ni.title", nitro_page)];

/// Nitro page as the 2026 client lays it out (vanilla-settings-05b, page
/// fully loaded): `tierCard_20JRSQ` 696x378, radius 16, 95.07deg gradient
/// rgb(133,71,198) → rgb(184,69,193) 50% → rgb(171,93,138); `tierInfo_` at
/// 40/40, 327 wide: nitro-md 32px bold title, five 24px-icon perks 32px apart
/// (body-md 16/20 white, 8px gap), then (padding-top 16, gap 8) 39px
/// `overlay-primary` white and `overlay-secondary` rgba(0,0,0,0.56) buttons;
/// `tierImage_` 313x328 art on the right. Then the 40px-padded `giftCard_`
/// (radius 16, 41px under): 147x146 gift art, heading-xl/semibold 24/32
/// title, body-sm text and the "Offrir Nitro" button. Art loads at runtime.
fn nitro_page(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    let perk = |icon: &'static str, t: &'static str| {
        div()
            .h(z(24.))
            .flex()
            .items_center()
            .gap(z(8.))
            .child(gpui::svg().path(crate::icons::path(icon)).size(z(24.)).text_color(gpui::white()))
            .child(div().text_size(z(16.)).line_height(z(20.)).text_color(gpui::white()).child(tr(t)))
    };
    let overlay_btn = |id: &'static str, t: &'static str, primary: bool| {
        div()
            .id(id)
            .h(z(39.))
            .rounded(z(8.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(if primary { gpui::white().into() } else { gpui::rgba(0x0000008f) })
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if primary { gpui::black() } else { gpui::white() })
            .child(tr(t))
    };
    // gpui gradients take two stops: one per half around the 50% stop.
    let half = |l: bool, a: u32, b: u32| {
        let d = div().absolute().top_0().h_full().w_1_2().bg(gpui::linear_gradient(95.07, gpui::linear_color_stop(gpui::rgb(a), 0.), gpui::linear_color_stop(gpui::rgb(b), 1.)));
        if l { d.left_0().rounded_l(z(16.)) } else { d.right_0().rounded_r(z(16.)) }
    };
    let tier = div()
        .relative()
        .mt(z(11.))
        .h(z(378.))
        .rounded(z(16.))
        .child(half(true, 0x8547c6, 0xb845c1))
        .child(half(false, 0xb845c1, 0xab5d8a))
        .child(gpui::img(SharedString::from(super::art::nitro_tier())).absolute().top(z(25.)).left(z(384.)).w(z(313.)).h(z(328.)))
        .child(
            div()
                .absolute()
                .top(z(40.))
                .left(z(40.))
                .w(z(327.))
                .flex()
                .flex_col()
                .child(div().h(z(36.)).text_size(z(32.)).line_height(z(24.)).font_weight(FontWeight::EXTRA_BOLD).italic().text_color(gpui::white()).child(tr("sp.ni.title")))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(z(8.))
                        .child(perk("chat/upload", "sp.ni.p1"))
                        .child(perk("chat/add-reaction", "sp.ni.p2"))
                        .child(perk("emoji", "sp.ni.p3"))
                        .child(perk("voice/screen", "sp.ni.p4"))
                        .child(perk("user-profile", "sp.ni.p5")),
                )
                .child(
                    div()
                        .pt(z(24.)) // tierInfo_ gap 8 + stack_ padding-top 16
                        .flex()
                        .flex_col()
                        .gap(z(8.))
                        .child(overlay_btn("nitro-more", "sp.ni.more", true))
                        .child(overlay_btn("nitro-manage", "sp.ni.manage", false)),
                ),
        );
    let gift = div()
        .mt(z(41.))
        .p(z(40.))
        .rounded(z(16.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .flex()
        .items_center()
        .gap(z(80.))
        .child(div().pl(z(41.)).child(gpui::img(SharedString::from(super::art::nitro_gift())).w(z(147.)).h(z(146.))))
        .child(
            div()
                .w(z(307.))
                .flex()
                .flex_col()
                .child(div().text_size(z(24.)).line_height(z(32.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(tr("sp.ni.gift")))
                .child(div().pt(z(16.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(tr("sp.ni.gift_d")))
                .child(div().pt(z(16.)).child(controls::button_icon(eid("nitro-gift"), "gift", tr("sp.ni.gift_btn"), BtnKind::Secondary))),
        );
    div().flex().flex_col().child(tier).child(gift).into_any_element()
}
static BOOST: &[B] = &[B::Custom("sp.bo.title", super::boost::page)];
static SUBSCRIPTIONS: &[B] = &[B::Custom("sp.su.title", super::gift::subscriptions)];
static GIFTS: &[B] = &[
    B::Custom("sp.gi.buy", super::gift::promo),
    B::Divider,
    B::Title("sp.gi.codes"),
    B::Custom("sp.gi.code_label", super::gift::redeem),
    B::Divider,
    B::Title("sp.gi.badge"),
    B::Custom("sp.gi.footer", super::gift::badges),
    B::Divider,
    B::Title("sp.gi.bought"),
    B::Custom("sp.gi.none", super::gift::purchased),
];
/// Facturation (vanilla-settings-06c, #3454-#3517): the data-free frame of the
/// page. Payment sources, balance and transactions need the billing API, which
/// Phoenix doesn't call, so those rows stay empty.
fn billing_sources(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    let line = || div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))); // divider_ZaYWLb
    let data = super::billing::get().unwrap_or_default();
    let has_sources = !data.sources.is_empty();
    // #3469 paymentSourceRow_: 32x22 card glyph, brand 16/20 + "Par défaut"
    // pill (body-xs/semibold, #4f505a, r3, padding 0 4), sub-line 14/18
    // --text-subtle, sm secondary "Modifier".
    let source_rows = data.sources.iter().enumerate().map(|(i, src)| {
        div()
            .mt(z(if i == 0 { 26. } else { 16. }))
            .h(z(42.))
            .flex()
            .items_center()
            .gap(z(10.))
            .child(
                div().w(z(32.)).h(z(22.)).flex_none().rounded(z(3.)).bg(c(tok(idx::BACKGROUND_MOD_STRONG))).flex().items_center().justify_center()
                    .text_size(z(8.)).font_weight(FontWeight::BOLD).text_color(c(tok(idx::TEXT_STRONG))).child(src.brand.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(z(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(z(8.))
                            .child(div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(src.brand.clone()))
                            .when(src.default, |d| {
                                d.child(
                                    div().px(z(4.)).rounded(z(3.)).bg(gpui::rgb(0x4f505a)).text_size(z(12.)).line_height(z(14.)).font_weight(FontWeight::SEMIBOLD)
                                        .text_color(gpui::white()).child(tr("sp.bl.default")),
                                )
                            }),
                    )
                    .child(desc_text(src.sub.clone().into())),
            )
            .child(controls::button_sm(eid(format!("billing-edit-{i}")), tr("common.edit"), BtnKind::Secondary))
            .into_any_element()
    });
    let country = data.country.as_deref().map(|cc| {
        let key = format!("country.{cc}");
        let name = tr(&key);
        let name = if *name == *key { cc.to_owned().into() } else { name };
        // #3462 body-sm --text-subtle, 4px under the 32px heading
        div().mt(z(-20.)).child(desc_text(crate::t!("sp.bl.country", name = name)))
    });
    div()
        .mt(z(-10.)) // title pb 24 + column gap 10 → line 24px under the heading
        .flex()
        .flex_col()
        .children(country)
        .children(source_rows)
        .when(has_sources, |d| d.child(line().mt(z(16.))))
        .child(
            // #3482 paymentSourceRow_ (gift card balance): 32px icon, 14/18 500 label at +46, sm secondary "Utiliser"
            div()
                .mt(z(if has_sources { 32. } else { 50. })) // #3482 (no source rows): 26 + the 24px the empty spacer #3465 leaves
                .h(z(32.))
                .flex()
                .items_center()
                .child(
                    // #3484 32px card glyph: brand tile with the white Discord logo
                    div().size(z(32.)).flex_none().flex().items_center().child(
                        div().w(z(32.)).h(z(22.)).rounded(z(3.)).bg(c(tok(idx::BRAND_500))).flex().items_center().justify_center()
                            .child(gpui::svg().path(crate::icons::path("discord-logo")).size(z(16.)).text_color(gpui::white())),
                    ),
                )
                .child(div().ml(z(14.)).flex_1().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(tr("sp.bl.gift")))
                .child(controls::button_sm(eid("billing-redeem"), tr("sp.bl.redeem"), BtnKind::Secondary)),
        )
        .child(line().mt(z(16.))) // #3493
        .child(
            // #3494 footer_: primary md button, right aligned, 20px under the divider
            div().mt(z(20.)).mb(z(10.)).flex().justify_end().child(controls::button(eid("billing-add"), tr("sp.bl.add"), BtnKind::Primary)),
        )
        .into_any_element()
}

/// "Historique des transactions" table header (#3511 paymentRow_, 53px,
/// bottom border; #3513-#3515 12/12 600 Date 122px at +21, Description at +151,
/// Montant right).
fn billing_history(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    let pays = super::billing::get().map(|b| b.payments).unwrap_or_default();
    let col = |t: &str| div().text_size(z(12.)).line_height(z(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(tr(t));
    div()
        .mt(z(-10.)) // #3505 line 24px under the heading
        .flex()
        .flex_col()
        .child(
            // #3510 paymentPane_: 1px --border-subtle, r8, --background-base-lower
            div()
                .mt(z(35.)) // #3505 empty 1px spacer 24px under the heading, pane 10 below
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                .child(div()
                .h(z(53.))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(div().ml(z(21.)).w(z(130.)).flex_none().child(col("sp.bl.date")))
                .child(div().flex_1().child(col("sp.bl.desc")))
                .child(div().mr(z(64.)).child(col("sp.bl.amount"))))
                // #3518 payment_ rows: 57px, 1px bottom border, padding-inline 16,
                // date 14/18 (131px), 20px shop glyph + 16px description,
                // orb/fiat amount, 24px expand chevron.
                .children(pays.iter().enumerate().map(|(i, p)| {
                    div()
                        .h(z(57.))
                        .px(z(16.))
                        .flex()
                        .items_center()
                        .gap(z(16.))
                        .when(i + 1 < pays.len(), |d| d.border_b_1().border_color(c(tok(idx::BORDER_SUBTLE))))
                        .child(div().w(z(131.)).flex_none().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(p.date.clone()))
                        .child(
                            div().flex_1().min_w_0().flex().items_center().gap(z(10.))
                                .child(gpui::svg().path(crate::icons::path("shop")).size(z(20.)).flex_none().text_color(c(tok(idx::TEXT_STRONG))))
                                .child(div().text_size(z(16.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_STRONG))).truncate().child(p.description.clone())),
                        )
                        .child(
                            div().flex().items_center().gap(z(4.)).text_size(z(16.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_STRONG)))
                                .when(p.orbs, |d| d.child(gpui::svg().path(crate::icons::path("orbs")).size(z(16.)).text_color(c(tok(idx::TEXT_STRONG)))))
                                .child(p.amount.clone()),
                        )
                        .child(gpui::svg().path("icons/chevron.svg").size(z(24.)).text_color(c(tok(idx::TEXT_STRONG))))
                })),
        )
        .when(pays.is_empty(), |d| d.child(div().pt(z(16.)).child(desc_text(tr("sp.billing.web")))))
        .into_any_element()
}

static BILLING: &[B] = &[
    B::Title("sp.bl.t1"),
    B::Custom("sp.bl.t1", billing_sources),
    B::Divider,
    B::Title("sp.bl.t2"),
    B::Custom("sp.bl.t2", billing_history),
];

static DATA: &[B] = &[
    B::Title("sp.data.t1"),
    B::Custom("sp.data.core", |_, _| {
        item(tr("sp.data.core"), Some(desc_text(tr("sp.data.core_d")).into_any_element()), None, false).into_any_element()
    }),
    B::Switch("data-improve", "sp.data.improve", "sp.data.improve_d", Bind::Proto),
    B::Switch("data-personalize", "sp.data.personalize", "sp.data.personalize_d", Bind::Proto),
    B::Switch("data-sponsored", "sp.data.sponsored", "sp.data.sponsored_d", Bind::Proto),
    B::Switch("data-third", "sp.data.third", "sp.data.third_d", Bind::Proto),
    B::Switch("data-discovery", "sp.data.discovery", "sp.data.discovery_d", Bind::Proto),
    B::Switch("data-clips", "sp.data.clips", "sp.data.clips_d", Bind::Proto),
    B::Action("sp.data.request", "sp.data.request_d", "sp.data.request_b", false),
    B::Related("apps", "sp.games", "sp.games_d", Page::RegisteredGames),
    B::Divider,
    B::Title("sp.data.t2"),
    B::Lead("sp.data.t2_d"),
    B::Radios(
        "share-profile",
        "sp.data.share_profile",
        "",
        &[("sp.data.sp_all", "sp.data.sp_all_d"), ("sp.data.sp_small", "sp.data.sp_small_d"), ("sp.data.sp_friends", "sp.data.sp_friends_d")],
        Pick::Proto,
    ),
    B::Switch("share-update", "sp.data.share_update", "sp.data.share_update_d", Bind::Proto),
    B::Related("user-profile", "settings.page.activity", "sp.activity_related_d", Page::ActivityPrivacy),
    B::Divider,
    B::Title("sp.data.t3"),
    B::Switch("e2ee-codes", "sp.data.codes", "sp.data.codes_d", Bind::Pref(false)),
];

const SHOW_BLOCK: &[&str] = &["sp.cs.show", "sp.cs.blur", "sp.cs.block"];

static CONTENT: &[B] = &[
    B::Title("sp.cs.t1"),
    B::Custom("sp.cs.sensitive", content_filters),
    B::Switch("age-cmds", "sp.cs.age_cmds", "sp.cs.age_cmds_d", Bind::Proto),
    B::Switch("age-ios", "sp.cs.age_ios", "sp.cs.age_ios_d", Bind::Proto),
    B::Related("image", "settings.page.appearance", "sp.cs.appearance_d", Page::Appearance),
    B::Divider,
    B::Title("sp.cs.t2"),
    B::Radios("spam", "sp.cs.spam", "sp.cs.spam_d", &[("sp.cs.spam_all", ""), ("sp.cs.spam_nf", ""), ("sp.cs.spam_none", "")], Pick::Proto),
    B::Divider,
    B::Title("sp.cs.t3"),
    B::Switch(
        "server-dms",
        "sp.cs.dms",
        "sp.cs.dms_d",
        Bind::Synced(|s| s.allow_server_dms, SettingsPatch::AllowServerDms),
    ),
    B::Switch("msg-requests", "sp.cs.requests", "sp.cs.requests_d", Bind::Proto),
    B::Divider,
    B::Title("sp.cs.t4"),
    B::Lead("sp.cs.t4_d"),
    B::Legend("sp.cs.fr_from"),
    B::Switch("fr-everyone", "sp.cs.fr_everyone", "", Bind::Proto),
    B::Switch("fr-fof", "sp.cs.fr_fof", "", Bind::Proto),
    B::Switch("fr-members", "sp.cs.fr_members", "sp.cs.fr_members_d", Bind::Proto),
    B::Switch("fr-msg", "sp.cs.fr_msg", "sp.cs.fr_msg_d", Bind::Proto),
    B::Divider,
    B::Title("sp.cs.t5"),
    B::Lead("sp.cs.t5_d"),
    B::Switch("game-dms", "sp.cs.game_dms", "sp.cs.game_dms_d", Bind::Proto),
    B::Radios("ingame", "sp.cs.ingame", "sp.cs.ingame_d", &[("sp.cs.ig_all", ""), ("sp.cs.ig_players", ""), ("sp.cs.ig_none", "")], Pick::Proto),
    B::Related("apps", "sp.cs.authorized", "sp.cs.authorized_d", Page::Connections),
    B::Divider,
    B::Title("sp.cs.t6"),
    B::Lead("sp.cs.t6_d"),
    B::Custom("sp.cs.blocked", |_, _| {
        item(tr("sp.cs.blocked"), Some(desc_text(tr("sp.cs.blocked_d")).into_any_element()), None, false).into_any_element()
    }),
];

static NOTIF: &[B] = &[
    B::Title("sp.no.t1"),
    B::Switch(
        "desktop-notif",
        "sp.no.desktop",
        "sp.no.desktop_d",
        Bind::Local(|p| p.desktop_notifications, |p, v| p.desktop_notifications = v),
    ),
    B::Legend("sp.no.when"),
    B::Switch("n-stream", "sp.no.stream", "", Bind::Proto),
    B::Switch("n-anniv", "sp.no.anniv", "", Bind::Proto),
    B::Switch("n-online", "sp.no.online", "", Bind::Proto),
    B::Switch("n-event", "sp.no.event", "", Bind::Proto),
    B::Switch("n-profile", "sp.no.profile", "", Bind::Proto),
    B::Select("n-react", "sp.no.react", "", &["sp.no.r_all", "sp.no.r_dm", "sp.no.r_never"], Pick::Proto),
    B::Divider,
    B::Title("sp.no.t2"),
    B::Sound("snd-msg", "sp.no.msg"),
    B::Sound("snd-msg-cur", "sp.no.msg_cur"),
    B::Sound("snd-ring", "sp.no.ring"),
    B::Switch(
        "notif-sounds",
        "sp.no.mute_all",
        "sp.no.mute_all_d",
        Bind::Local(|p| !p.notification_sounds, |p, v| p.notification_sounds = !v),
    ),
    B::Related("speaker", "settings.page.voice_video", "sp.no.voice_d", Page::VoiceVideo),
    B::Divider,
    B::Title("sp.no.t3"),
    B::Switch("badge", "sp.no.badge", "sp.no.badge_d", Bind::Pref(true)),
    B::Divider,
    B::Title("sp.no.t4"),
    B::Switch("e-comm", "sp.no.e_comm", "sp.no.e_comm_d", Bind::Proto),
    B::Switch("e-social", "sp.no.e_social", "sp.no.e_social_d", Bind::Proto),
    B::Switch("e-news", "sp.no.e_news", "sp.no.e_news_d", Bind::Proto),
    B::Switch("e-tips", "sp.no.e_tips", "sp.no.e_tips_d", Bind::Proto),
    B::Switch("e-recs", "sp.no.e_recs", "sp.no.e_recs_d", Bind::Proto),
    B::Action("sp.no.unsub", "", "sp.no.unsub_b", true),
    B::Divider,
    B::Title("sp.no.t5"),
    B::Expand(
        "notif-adv",
        "sp.no.adv",
        "sp.no.adv_d",
        &[
            B::Select(
                "push-delay",
                "sp.no.push",
                "sp.no.push_d",
                &["sp.no.min1", "sp.no.min2", "sp.no.min3", "sp.no.min4", "sp.no.min5", "sp.no.min6", "sp.no.min7", "sp.no.min8", "sp.no.min9", "sp.no.min10"],
                Pick::Proto,
            ),
            B::Switch("tts", "sp.no.tts", "sp.no.tts_d", Bind::Proto),
            B::Radios("read-all", "sp.no.read_all", "sp.no.read_all_d", &[("sp.no.ra_all", ""), ("sp.no.ra_cur", ""), ("sp.no.r_never", "")], Pick::Pref(2)),
        ],
    ),
];

fn theme_row(app: &PhoenixApp, cx: &mut Context<'_, PhoenixApp>) -> AnyElement {
    let theme = app.prefs.synced.theme;
    div()
        .flex()
        .flex_col()
        .pt(z(16.))
        // vanilla-settings-08 #2721 heading-sm/semibold 16/20, swatches 24px under it.
        .child(title_text(tr("sp.ap.defaults")).font_weight(FontWeight::SEMIBOLD).pb(z(24.)))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .children(AppTheme::ALL.map(|t| {
                    super::view::theme_card(t, t == theme).on_click(cx.listener(move |a, _, _, cx| save(a, SettingsPatch::Theme(t), cx)))
                }))
                // 5th `defaultThemeSelection_` "Synchroniser avec l'ordinateur"
                // (vanilla-settings-08b): the current theme's background with
                // the 24px RefreshIcon in `iconWrapper_IhyaN_`.
                .child(
                    div()
                        .id("theme-sync-os")
                        .size(z(48.))
                        .flex_none()
                        .rounded(z(8.))
                        .bg(c(theme.swatch()))
                        .border_1()
                        .border_color(c(tok(idx::BORDER_SUBTLE)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(gpui::svg().path(crate::icons::path("refresh")).size(z(24.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
                ),
        )
        .into_any_element()
}

/// "Donne à ton Discord le style que tu veux" (`container_yH_g_W
/// displayContainer_`, vanilla-settings-08b): 104px black card, radius 8,
/// padding 16 25; 117x69 banner art, 16px gap, heading-lg/semibold 20/24 +
/// body-sm 14/18 text (378px), primary md "Vas-y, essaie" button.
fn theme_banner(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div()
        .mt(z(29.))
        .flex()
        .items_center()
        .gap(z(16.))
        .py(z(16.))
        .px(z(25.))
        .rounded(z(8.))
        .bg(gpui::black()) // displayContainer_ background oklab(0 0 0)
        .child(gpui::img(SharedString::from(super::art::theme_banner())).w(z(117.)).h(z(69.)).flex_none())
        .child(
            div()
                .w(z(378.)) // textContent_yH_g_W width
                .flex_none()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(tr("sp.ap.banner")))
                .child(desc_text(tr("sp.ap.banner_d")).text_color(c(tok(idx::TEXT_DEFAULT)))),
        )
        .child(controls::button(eid("theme-banner-try"), tr("sp.ap.banner_try"), BtnKind::Primary))
        .into_any_element()
}

/// "Thème de couleur à durée limitée" (vanilla-settings-08b): heading-sm
/// semibold + round `body-xs/semibold` uppercase countdown pill
/// (rgba(151,151,159,0.2), padding 0 8, 12px clock), body-sm 14/18
/// --text-muted note with "En savoir plus" link, then the 48px Vice City
/// swatch (135deg gradient under the `darkOverlay_` 0.2 black layer).
fn limited_theme(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div()
        .mt(z(39.))
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .child(title_text(tr("sp.ap.limited")).font_weight(FontWeight::SEMIBOLD))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .px(z(8.))
                        .h(z(16.))
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_MOD_STRONG)))
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(gpui::svg().path(crate::icons::path("picker/clock")).size(z(12.)).text_color(c(tok(idx::TEXT_DEFAULT))))
                        .child(tr("sp.ap.limited_left")),
                ),
        )
        .child(
            div()
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(c(tok(idx::TEXT_MUTED)))
                // The link runs inline after the note (`anchor_` inside the same text), not on its own line.
                .child({
                    let (note, more) = (tr("sp.ap.limited_d"), tr("sp.ap.limited_more"));
                    let start = note.len() + 1;
                    let text = format!("{note} {more}");
                    let end = text.len();
                    gpui::StyledText::new(text).with_highlights([(start..end, gpui::HighlightStyle { color: Some(c(tok(idx::TEXT_LINK))), ..Default::default() })])
                }),
        )
        .child(
            div().pt(z(20.)).child(
                div()
                    .id("theme-vice-city")
                    .size(z(48.))
                    .rounded(z(8.))
                    .cursor_pointer()
                    .bg(gpui::linear_gradient(135., gpui::linear_color_stop(gpui::rgb(0x4645ff), 0.), gpui::linear_color_stop(gpui::rgb(0xf89a03), 1.)))
                    .child(
                        div()
                            .size_full()
                            .rounded(z(8.))
                            .bg(gpui::linear_gradient(135., gpui::linear_color_stop(gpui::rgba(0xaf01f380), 0.33), gpui::linear_color_stop(gpui::rgba(0xff669880), 0.67)))
                            .child(div().size_full().rounded(z(8.)).bg(gpui::rgba(0x00000033))),
                    ),
            ),
        )
        .into_any_element()
}

static APPEARANCE: &[B] = &[
    B::Custom("sp.ap.t1", theme_title),
    B::Switch("theme-sync", "sp.ap.sync", "sp.ap.sync_d", Bind::Pref(false)),
    B::Custom("sp.ap.defaults", theme_row),
    B::Custom("sp.ap.banner", theme_banner),
    B::Custom("sp.ap.limited", limited_theme),
    B::Custom("sp.ap.colors", nitro_themes),
    B::Switch("theme-devices", "sp.ap.sync_devices", "", Bind::Pref(true)),
    B::Switch("theme-profiles", "sp.ap.apply_profiles", "", Bind::Pref(true)),
    B::Related("help", "settings.page.accessibility", "sp.ap.a11y_d", Page::Accessibility),
    // vanilla-settings-08b: "Icône de l'appli" follows the theme category.
    B::Divider,
    B::Custom("sp.ap.icon", nitro_app_icon),
    B::Divider,
    B::Title("sp.ap.t2"),
    B::Legend("sp.ap.media"),
    B::Switch("media-links", "sp.ap.links", "", Bind::Proto),
    B::Switch("media-uploads", "sp.ap.uploads", "", Bind::Proto),
    B::Switch("embeds", "sp.ap.embeds", "", Bind::Proto),
    B::Switch("reactions", "sp.ap.reactions", "", Bind::Proto),
    B::Select("spoilers", "sp.ap.spoilers", "", &["sp.ap.sp_click", "sp.ap.sp_always", "sp.ap.sp_mod"], Pick::Proto),
    B::Switch("split-threads", "sp.ap.split", "", Bind::Proto),
    B::Switch("avatars", "sp.ap.avatars", "sp.ap.avatars_d", Bind::Proto),
    B::Switch("favs", "sp.ap.favs", "sp.ap.favs_d", Bind::Proto),
    B::Related("help", "settings.page.accessibility", "sp.ap.density_d", Page::Accessibility),
    B::Divider,
    B::Title("sp.ap.t3"),
    B::Switch("md-preview", "sp.ap.preview", "", Bind::Proto),
    B::Switch("emoticons", "sp.ap.emoticons", "sp.ap.emoticons_d", Bind::Proto),
    B::Switch("ac-stickers", "sp.ap.stickers", "", Bind::Proto),
    B::Switch("ac-games", "sp.ap.games_ac", "", Bind::Proto),
    B::Switch("send-btn", "sp.ap.send_btn", "", Bind::Pref(false)),
    B::Divider,
    B::Title("sp.ap.t4"),
    B::Radios("dm-search", "sp.ap.dm_search", "", &[("sp.ap.dm_sel", ""), ("sp.ap.dm_all", "")], Pick::Proto),
    B::Divider,
    B::Title("sp.ap.t5"),
    B::Switch("streamer", "sp.ap.streamer", "sp.ap.streamer_d", Bind::Pref(false)),
    B::Switch("streamer-auto", "sp.ap.streamer_auto", "", Bind::Pref(true)),
    B::Legend("sp.ap.streamer_when"),
    B::Switch("st-info", "sp.ap.hide_info", "", Bind::Pref(true)),
    B::Switch("st-invites", "sp.ap.hide_invites", "", Bind::Pref(true)),
    B::Switch("st-sounds", "sp.ap.no_sounds", "", Bind::Pref(true)),
    B::Switch("st-notifs", "sp.ap.no_notifs", "", Bind::Pref(true)),
];

fn slider_row(title: &'static str, desc: &'static str, s: Div) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .py(z(12.))
        .gap(z(4.))
        .child(title_text(tr(title)))
        .child(desc_text(tr(desc)))
        .child(div().pt(z(8.)).child(s))
        .into_any_element()
}

fn density_idx(p: &SyncedSettings) -> usize {
    crate::theme::Density::ALL.iter().position(|d| *d == p.density).unwrap_or(0)
}

static A11Y: &[B] = &[
    B::Custom("sp.ac.preview", a11y_preview),
    B::Title("sp.ac.t1"),
    B::Custom("sp.ac.font", |app, cx| {
        let fs = app.prefs.local.font_size;
        slider_row(
            "sp.ac.font",
            "sp.ac.font_d",
            super::view::slider("font", &[12., 14., 15., 16., 18., 20., 24.], "px", fs, 16., |a, v, cx| update_local(a, |p| p.font_size = v, cx), cx),
        )
    }),
    B::Switch("underline", "sp.ac.underline", "sp.ac.underline_d", Bind::Pref(false)),
    B::Switch("name-styles", "sp.ac.name_styles", "sp.ac.name_styles_d", Bind::Pref(true)),
    B::Divider,
    B::Title("sp.ac.t2"),
    // Compacte / Par défaut / Spacieuse = Density::ALL order
    B::Radios(
        "density",
        "settings.appearance.density",
        "sp.ac.density_d",
        &[("settings.density.compact", ""), ("settings.density.default", ""), ("settings.density.cozy", "")],
        Pick::Synced(density_idx, |i| SettingsPatch::Density(crate::theme::Density::ALL[i.min(2)])),
    ),
    B::Radios(
        "display",
        "sp.ac.display",
        "sp.ac.display_d",
        &[("sp.ac.d_default", ""), ("sp.ac.d_compact", "")],
        Pick::Synced(|s| s.compact as usize, |i| SettingsPatch::Compact(i == 1)),
    ),
    B::Custom("sp.ac.zoom", |app, cx| {
        let zoom = app.prefs.local.zoom as f32;
        slider_row(
            "sp.ac.zoom",
            "sp.ac.zoom_d",
            super::view::slider(
                "zoom",
                &[50., 67., 75., 80., 90., 100., 110., 125., 150., 175., 200.],
                "",
                zoom,
                100.,
                |a, v, cx| update_local(a, |p| p.zoom = v as _, cx),
                cx,
            ),
        )
    }),
    B::Divider,
    B::Title("sp.ac.t3"),
    B::Custom("sp.ac.sat", |app, cx| {
        let sat = app.prefs.local.saturation as f32;
        slider_row(
            "sp.ac.sat",
            "sp.ac.sat_d",
            super::view::slider(
                "sat",
                &[0., 10., 20., 30., 40., 50., 60., 70., 80., 90., 100.],
                "%",
                sat,
                100.,
                |a, v, cx| update_local(a, |p| p.saturation = v as _, cx),
                cx,
            ),
        )
    }),
    B::Switch("sat-custom", "sp.ac.sat_custom", "sp.ac.sat_custom_d", Bind::Pref(false)),
    B::Switch("contrast", "sp.ac.contrast", "sp.ac.contrast_d", Bind::Pref(false)),
    B::Select("roles", "sp.ac.roles", "sp.ac.roles_d", &["sp.ac.r_names", "sp.ac.r_next", "sp.ac.r_none"], Pick::Pref(0)),
    B::Switch("indicators", "sp.ac.indicators", "sp.ac.indicators_d", Bind::Pref(false)),
    B::Related("emoji", "settings.page.appearance", "sp.ac.appearance_d", Page::Appearance),
    B::Divider,
    B::Title("sp.ac.t4"),
    B::Switch("reduced-motion", "sp.ac.motion", "sp.ac.motion_d", Bind::Local(|p| p.reduced_motion, |p, v| p.reduced_motion = v)),
    B::Switch("motion-sync", "sp.ac.motion_sync", "", Bind::Pref(true)),
    B::Switch("gifs", "sp.ac.gifs", "", Bind::Proto),
    B::Switch("anim-emoji", "sp.ac.anim_emoji", "", Bind::Proto),
    B::Radios(
        "stickers",
        "sp.ac.stickers",
        "",
        &[("sp.ac.st_always", ""), ("sp.ac.st_inter", "sp.ac.st_inter_d"), ("sp.ac.st_never", "")],
        Pick::Proto,
    ),
    B::Divider,
    B::Title("sp.ac.t5"),
    B::Switch("alt-text", "sp.ac.alt", "sp.ac.alt_d", Bind::Proto),
    B::Switch("legacy-input", "sp.ac.legacy", "sp.ac.legacy_d", Bind::Proto),
];

static SYSTEM: &[B] = &[
    B::Title("sp.sy.t1"),
    B::Switch("startup", "sp.sy.startup", "", Bind::Pref(false)),
    B::Switch("tray", "sp.sy.tray", "sp.sy.tray_d", Bind::Local(|p| p.minimize_to_tray, |p, v| p.minimize_to_tray = v)),
    B::Switch("hwaccel", "sp.sy.hwaccel", "sp.sy.hwaccel_d", Bind::Pref(true)),
    B::Custom("sp.sy.hwaccel", hwaccel_banner),
    B::Divider,
    B::Custom("sp.sy.t2", custom_keybinds),
    B::Divider,
    B::Title("sp.sy.t3"),
    B::KeyGroup("", "", &[("sp.kb.show", &["ctrl", "/"])]),
    B::KeyGroup(
        "sp.kb.g_msg",
        "sp.kb.g_msg_d",
        &[
            ("sp.kb.edit", &["e"]),
            ("sp.kb.delete", &["backspace"]),
            ("sp.kb.pin", &["p"]),
            ("sp.kb.react", &["+"]),
            ("sp.kb.reply", &["r"]),
            ("sp.kb.forward", &["f"]),
            ("sp.kb.copy", &["ctrl", "c"]),
            ("sp.kb.unread", &["alt", "enter"]),
            ("sp.kb.focus", &["esc"]),
        ],
    ),
    B::KeyGroup(
        "sp.kb.g_nav",
        "",
        &[
            ("sp.kb.servers", &["ctrl", "alt", "↑ ↓"]),
            ("sp.kb.channels", &["alt", "↑ ↓"]),
            ("sp.kb.history", &["alt", "← →"]),
            ("sp.kb.unreads", &["alt", "shift", "↑ ↓"]),
            ("sp.kb.mentions", &["ctrl", "shift", "alt", "↑ ↓"]),
            ("sp.kb.call", &["ctrl", "shift", "alt", "v"]),
            ("sp.kb.last", &["ctrl", "alt", "→"]),
            ("sp.kb.switcher", &["ctrl", "k"]),
            ("sp.kb.create", &["ctrl", "shift", "n"]),
        ],
    ),
    B::KeyGroup(
        "sp.kb.g_chat",
        "",
        &[
            ("sp.kb.server_read", &["shift", "esc"]),
            ("sp.kb.channel_read", &["esc"]),
            ("sp.kb.group", &["ctrl", "shift", "t"]),
            ("sp.kb.pins", &["ctrl", "p"]),
            ("sp.kb.inbox", &["ctrl", "i"]),
            ("sp.kb.members", &["ctrl", "u"]),
            ("sp.kb.emoji", &["ctrl", "e"]),
            ("sp.kb.gif", &["ctrl", "g"]),
            ("sp.kb.sticker", &["ctrl", "s"]),
            ("sp.kb.scroll", &["page up", "page down"]),
            ("sp.kb.oldest", &["shift", "page up"]),
            ("sp.kb.upload", &["ctrl", "shift", "u"]),
        ],
    ),
    B::KeyGroup(
        "sp.kb.g_voice",
        "",
        &[
            ("sp.kb.mute", &["ctrl", "shift", "m"]),
            ("sp.kb.deafen", &["ctrl", "shift", "d"]),
            ("sp.kb.answer", &["ctrl", "enter"]),
            ("sp.kb.decline", &["esc"]),
        ],
    ),
    B::KeyGroup(
        "sp.kb.g_misc",
        "",
        &[("sp.kb.help", &["ctrl", "shift", "h"]), ("sp.kb.search", &["ctrl", "f"]), ("sp.kb.settings", &["ctrl", ","])],
    ),
    B::Divider,
    B::Title("sp.sy.t4"),
    B::Action("sp.sy.perf", "sp.sy.perf_d", "sp.sy.capture", false),
];

fn language_row(app: &PhoenixApp, cx: &mut Context<'_, PhoenixApp>) -> AnyElement {
    let cur = app.prefs.synced.locale.clone();
    let ix = super::view::LOCALES.iter().position(|(c, _)| *c == cur.as_ref()).unwrap_or(0);
    let opts: Vec<SharedString> = super::view::LOCALES.iter().map(|(_, n)| SharedString::from(*n)).collect();
    // vanilla-settings-11: the field shows the language on the left and its
    // native name 16/20 --text-default right-aligned before the chevron.
    let name = opts.get(ix).cloned().unwrap_or_default();
    let field = div()
        .id(eid("sel-language"))
        .h(z(40.))
        .pl(z(12.))
        .flex()
        .items_center()
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INPUT_BORDER_DEFAULT)))
        .hover(|s| s.border_color(c(tok(idx::INPUT_BORDER_HOVER))))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .cursor_pointer()
        .child(div().flex_1().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(name.clone()))
        .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).whitespace_nowrap().child(name))
        .child(
            div().w(z(36.)).h_full().flex().items_center().justify_center().pr(z(4.))
                .child(gpui::svg().path("icons/chevron.svg").size(z(20.)).text_color(c(tok(idx::TEXT_STRONG)))),
        );
    let dd = dropdown_with(
        app,
        "language",
        field,
        opts,
        ix,
        696.,
        |a, i, cx| {
            if let Some(s) = a.settings.as_mut() {
                s.open_select = None;
            }
            save(a, SettingsPatch::Locale(super::view::LOCALES[i].0.into()), cx)
        },
        cx,
    );
    div()
        .flex()
        .flex_col()
        .py(z(12.))
        .gap(z(4.))
        .child(title_text(tr("sp.la.select")))
        .child(desc_text(tr("sp.la.select_d")))
        .child(div().pt(z(8.)).child(dd))
        .into_any_element()
}

static LANGUAGE: &[B] = &[
    B::Custom("sp.la.select", language_row),
    B::Radios("time-format", "sp.la.time", "", &[("sp.la.auto", ""), ("sp.la.h12", ""), ("sp.la.h24", "")], Pick::Proto),
];

static ACTIVITY: &[B] = &[
    B::Title("sp.at.t1"),
    B::Switch("act-share", "sp.at.share", "sp.at.share_d", Bind::Proto),
    B::Switch("act-online", "sp.at.online", "sp.at.online_d", Bind::Proto),
    B::Switch("act-stream", "sp.at.stream", "sp.at.stream_d", Bind::Proto),
    B::Related("apps", "sp.games", "sp.games_d", Page::RegisteredGames),
    B::Divider,
    B::Title("sp.at.t2"),
    B::Radios("act-where", "sp.at.auto", "sp.at.auto_d", &[("sp.at.a_all", ""), ("sp.at.a_small", ""), ("sp.at.a_none", "")], Pick::Proto),
    B::Related("lock", "settings.page.privacy", "sp.data.t2_d", Page::DataPrivacy),
    B::Divider,
    B::Title("sp.at.t3"),
    B::Lead("sp.at.t3_d"),
    B::Switch("act-friends", "sp.at.friends", "sp.at.friends_d", Bind::Proto),
    B::Switch("act-voice", "sp.at.voice", "sp.at.voice_d", Bind::Proto),
];

fn connection_tiles(_: &PhoenixApp, _: &mut Context<'_, PhoenixApp>) -> AnyElement {
    // vanilla-settings-14b `accountButtonInner_yX-kzH`: 44px tile, radius 8,
    // rgba(151,151,159,0.12) (--background-mod-subtle), 8px apart (52px
    // pitch), rows 53px apart; the logo is the client's art loaded at runtime
    // (`applicationIcon_jn4e52` app icons are 28px).
    let tile = |(name, url, app): (&'static str, String, bool)| {
        let logo = if app { 28. } else { 32. };
        div()
            .id(eid(format!("conn-{name}")))
            .size(z(44.))
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
            .child(gpui::img(SharedString::from(url)).size(z(logo)).rounded(z(if app { 6. } else { 0. })).object_fit(gpui::ObjectFit::Contain))
    };
    div()
        .flex()
        .flex_col()
        .pt(z(12.))
        .gap(z(16.))
        .child(title_text(tr("sp.cn.add")))
        .child(div().flex().flex_wrap().gap_x(z(8.)).gap_y(z(9.)).children(super::art::connections().into_iter().map(tile)))
        .into_any_element()
}

static CONNECTIONS: &[B] = &[
    B::Title("sp.cn.t1"),
    B::Lead("sp.cn.t1_d"),
    B::Custom("sp.cn.add", connection_tiles),
    B::Custom("sp.cn.empty", no_connections),
    B::Divider,
    B::Title("sp.cn.t2"),
    B::Lead("sp.cn.t2_d"),
    B::Custom("sp.cn.t2", super::authorized_apps::render),
];


/// connectionList_ empty state (vanilla-settings-14 #2782-#2786): a 200px
/// area with the centred 440px "Aucune connexion" heading-lg/semibold and
/// body-md text, both --text-muted. Only when READY lists no connection.
fn no_connections(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    if super::account::connections().unwrap_or(0) > 0 {
        return div().into_any_element();
    }
    div()
        .h(z(200.))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(z(440.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .text_center()
                .text_color(c(tok(idx::TEXT_MUTED)))
                .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).child(tr("sp.cn.empty")))
                .child(div().text_size(z(16.)).line_height(z(20.)).child(tr("sp.cn.empty_d"))),
        )
        .into_any_element()
}

// Vanilla Developer page (vanilla-settings-15): Developer Mode and
// Application Test Mode switches only.
static DEVELOPER: &[B] = &[
    B::Switch(
        "dev-mode",
        "settings.advanced.dev_mode",
        "sp.dv.mode_d",
        Bind::Synced(|s| s.developer_mode, SettingsPatch::DeveloperMode),
    ),
    B::Switch("app-test", "sp.dv.test", "sp.dv.test_d", Bind::Ui(false)),
];

static PROFILES: &[B] = &[B::Custom("sp.pf.main", |app, _| super::view::profiles(app).into_any_element())];

// ------------------------------------------------- extra blocks (vanilla dumps)

/// Inline notice `container_0v8Ae1 info_` (vanilla-settings-10 #2764):
/// --background-feedback-mod-info, 1px --inlinenotice-border-info, radius
/// --radius-sm, padding --space-12, 16px --icon-feedback-info glyph, gap 8,
/// body-sm 14/18 --text-strong copy; optional trailing action (gap 12).
fn info_notice(text: SharedString, action: Option<gpui::Stateful<Div>>) -> Div {
    div()
        .flex()
        .items_start()
        .gap(z(16.)) // --space-16
        .p(z(12.)) // --space-12
        .rounded(z(8.)) // --radius-sm
        .border_1()
        .border_color(c(tok(idx::INLINENOTICE_BORDER_INFO)))
        .bg(c(tok(idx::BACKGROUND_FEEDBACK_MOD_INFO)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .gap(z(8.)) // iconAndTextContainer_ --space-8
                .child(gpui::svg().path(crate::icons::path("info")).flex_none().size(z(16.)).mt(z(1.)).text_color(c(tok(idx::ICON_FEEDBACK_INFO))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap(z(12.)) // contentsContainer_ gap
                        .child(div().flex_1().min_w_0().max_w(z(410.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_STRONG))).child(text))
                        .children(action),
                ),
        )
}

/// Hardware-acceleration warning `root_vb4RFD warning_` (vanilla-settings-10
/// #2728): padding 8, --radius-xs, --background-feedback-mod-warning, 1px
/// --icon-feedback-warning border, 20px warning icon + 10px margin, body-sm/medium.
fn hwaccel_banner(app: &PhoenixApp, _: &mut Cx) -> AnyElement {
    if get_bool(app, "hwaccel", Bind::Pref(true)) {
        return div().into_any_element();
    }
    div()
        .flex()
        .items_center()
        .p(z(8.))
        .rounded(z(crate::theme::generated::global::RADIUS_XS))
        .border_1()
        .border_color(c(tok(idx::ICON_FEEDBACK_WARNING)))
        .bg(c(tok(idx::BACKGROUND_FEEDBACK_MOD_WARNING)))
        .child(gpui::svg().path(crate::icons::path("warning")).flex_none().size(z(20.)).mr(z(10.)).text_color(c(tok(idx::ICON_FEEDBACK_WARNING))))
        .child(div().flex_1().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(tr("sp.sy.hwaccel_off")))
        .into_any_element()
}

/// vanilla-settings-10: h2 heading-xl 24/32 + 427px description, primary
/// "Add a keybind" button at the right (x=909), two info notices, then the
/// user's keybind rows or the empty note.
fn custom_keybinds(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let n = get_pick(app, "kb-count", Pick::Ui(0));
    let add = controls::button_icon(eid("kb-add"), "plus", tr("sp.sy.add_kb"), BtnKind::Secondary)
        .on_click(cx.listener(move |a, _, _, cx| set_pick(a, "kb-count", Pick::Ui(0), n + 1, cx)));
    let mut col = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(
            div()
                .flex()
                .items_start()
                .gap(z(16.))
                // #2738 description box is 427px wide; the button sits at the right (x=909)
                .child(div().flex_1().min_w_0().flex().flex_col().gap(z(4.)).child(controls::section_title(tr("sp.sy.t2"))).child(desc_text(tr("sp.sy.t2_d")).max_w(z(427.))))
                .child(add),
        )
        .child(info_notice(tr("sp.sy.kb_disabled"), None))
        .child(info_notice(
            tr("sp.sy.kb_saved"),
            Some(controls::button_sm(eid("kb-os"), tr("sp.sy.kb_os"), BtnKind::Secondary)),
        ));
    if n == 0 {
        col = col.child(desc_text(tr("sp.sy.no_kb")));
    }
    for i in 0..n {
        // keybind row: action select + recorded shortcut + remove
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(z(16.))
                .p(z(16.))
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
                .child(
                    div().flex_1().flex().flex_col().gap(z(8.)).child(title_text(tr("sp.sy.kb_action"))).child(desc_text(tr("sp.sy.kb_unassigned"))),
                )
                .child(
                    div().flex_1().flex().flex_col().gap(z(8.)).child(title_text(tr("sp.sy.kb_keybind"))).child(desc_text(tr("sp.sy.kb_none"))),
                )
                .child(
                    div()
                        .id(eid(format!("kb-rm-{i}")))
                        .cursor_pointer()
                        .child(gpui::svg().path(crate::icons::path("close")).size(z(20.)).text_color(c(tok(idx::ICON_DEFAULT))))
                        .on_click(cx.listener(move |a, _, _, cx| set_pick(a, "kb-count", Pick::Ui(0), n - 1, cx))),
                ),
        );
    }
    col.into_any_element()
}

/// vanilla-settings-09 "Aperçu" (#2687-#2759): h2 body-md/medium, then
/// `preview_TpS9Es` (--background-base-low, 1px --border-normal, --radius-sm,
/// padding --space-24, columns 1fr + min-content, gap --space-8). Left: two
/// cozy messages by the current user (40px avatar at 20px, text at 80px, 16/22),
/// the first with two reactions (`reaction_MmKct8` radius 8, padding 2 6, 20px
/// emoji, 16/600 count), the second with a link. Right (`previewRight_`):
/// three 32px status avatars 6px apart and a primary sm "Sample button".
fn a11y_preview(app: &PhoenixApp, _: &mut Cx) -> AnyElement {
    let me = &app.me;
    let name: SharedString = if me.display_name.is_empty() { me.name.clone() } else { me.display_name.clone() };
    let msg = |body: AnyElement, extra: Option<Div>| {
        div()
            .relative()
            .pl(z(80.)) // cozyMessage_ padding-left
            .pr(z(30.))
            .py(z(2.))
            .child(div().absolute().left(z(20.)).top(z(4.)).child(crate::ui::avatar(Some(me), 40.)))
            .child(
                div()
                    .flex()
                    .gap(z(8.))
                    .items_baseline()
                    .child(div().text_size(z(16.)).line_height(z(22.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(name.clone()))
                    .child(div().text_size(z(12.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_MUTED))).child("11:11")),
            )
            .child(div().text_size(z(16.)).line_height(z(22.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(body))
            .children(extra)
    };
    let reaction = |emoji: &'static str, n: &'static str, me_: bool| {
        div()
            .flex()
            .items_center()
            .gap(z(6.))
            .px(z(6.))
            .py(z(2.))
            .rounded(z(8.))
            .border_1()
            .when(me_, |d| {
                d.bg(c(tok(idx::REACTION_BACKGROUND_REACTED_DEFAULT))).border_color(c(tok(idx::REACTION_BORDER_REACTED_DEFAULT)))
            })
            .when(!me_, |d| d.bg(c(tok(idx::REACTION_BACKGROUND_DEFAULT))).border_color(gpui::transparent_black()))
            .child(div().text_size(z(16.)).line_height(z(20.)).child(emoji))
            .child(div().text_size(z(16.)).line_height(z(16.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(n))
    };
    let link_body = div()
        .flex()
        .gap(z(4.))
        .child(tr("sp.ac.sample2"))
        .child(div().text_color(c(tok(idx::TEXT_LINK))).child("https://discord.com/accessibility"))
        .into_any_element();
    // `messages_TpS9Es` starts 19px left of the padded box (#2691 x=423 vs 442).
    let left = div()
        .flex_1()
        .min_w_0()
        .ml(z(-19.))
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(msg(
            div().child(tr("sp.ac.sample1")).into_any_element(),
            Some(div().flex().gap(z(4.)).py(z(4.)).child(reaction("👍", "3", true)).child(reaction("🎉", "1", false))),
        ))
        .child(msg(link_body, None));
    let ring = c(tok(idx::BACKGROUND_BASE_LOW));
    let right = div()
        .flex_none()
        .flex()
        .flex_col()
        .items_end()
        .justify_between()
        .gap(z(20.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(z(6.))
                .children([crate::model::Presence::Idle, crate::model::Presence::Dnd, crate::model::Presence::Online].map(|p| crate::ui::avatar_with_status(Some(me), 32., p, ring))),
        )
        .child(controls::button_sm(eid("a11y-sample"), tr("sp.ac.sample_btn"), BtnKind::Primary));
    div()
        .flex()
        .flex_col()
        .gap(z(8.))
        // #2689 h2 body-md/medium in --text-muted
        .child(title_text(tr("sp.ac.preview")).text_color(c(tok(idx::TEXT_MUTED))))
        .child(
            div()
                .flex()
                .gap(z(8.)) // --space-8
                .p(z(24.)) // --space-24
                .max_h(z({
                    // max-height 25vh (#2690: 175 in a 700px viewport); 700 before the first frame.
                    let vh = crate::ui::title_bar::viewport_h_css();
                    0.25 * if vh > 0. { vh } else { 700. }
                }))
                .overflow_hidden()
                .rounded(z(8.)) // --radius-sm
                .border_1()
                .border_color(c(tok(idx::BORDER_NORMAL)))
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .child(left)
                .child(right),
        )
        .into_any_element()
}

/// Title row with the secondary "Theme preview" button (vanilla-settings-08,
/// span at x=980 w=119).
fn theme_title(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div()
        .flex()
        .items_start()
        .justify_between()
        .child(controls::section_title(tr("sp.ap.t1")))
        .child(controls::button(eid("theme-preview"), tr("sp.ap.theme_preview"), BtnKind::Secondary))
        .into_any_element()
}

/// Colour-theme swatches, computed `background-image` of each
/// `themeSelection_IhyaN_` in vanilla-settings-08 (#2793-#2856): angle, first
/// stop, most distinct stop (gpui gradients take two stops). Light themes
/// already include the `.lightOverlay_` --opacity-white-48 layer.
const THEME_SWATCHES: [(f32, u32, f32, u32, f32); 22] = [
    (180., 0xa7d9cd, 0.06, 0xcde3b0, 0.93), //  #2793
    (180., 0xf9d796, 0.31, 0xf6c0a8, 0.67), //  #2796
    (149., 0x99bbce, 0.06, 0xbcbcdb, 0.26), //  #2799
    (38., 0xf7d3d7, 0.04, 0xd1eccf, 0.69), //  #2802
    (154., 0xcd9cb7, 0.09, 0xd1c89a, 0.76), //  #2805
    (180., 0xf9d3da, 0.09, 0xd6dffd, 0.94), //  #2808
    (180., 0xd0defb, 0.07, 0xe6eecf, 0.96), //  #2811
    (39., 0xf3ece7, 0.13, 0xefeacf, 0.52), //  #2814
    (180., 0x48288c, 0.28, 0xdb7f4b, 0.71), //  #2817
    (180., 0x0eb5bf, 0.04, 0xa308a7, 0.40), //  #2820
    (180., 0x142215, 0.11, 0xa98e4b, 0.84), //  #2823
    (180., 0x950909, 0.16, 0x000000, 0.72), //  #2826
    (180., 0x5348ca, 0.11, 0x140730, 0.62), //  #2829
    (180., 0x895240, 0.15, 0x8f4343, 0.75), //  #2832
    (180., 0x665069, 0.13, 0x91a3d1, 0.86), //  #2835
    (180., 0x647962, 0.02, 0x6a8482, 0.96), //  #2838
    (180., 0x3a7ca1, 0.06, 0x58579a, 0.26), //  #2841
    (180., 0x01a89e, 0.00, 0xb43898, 1.00), //  #2844
    (180., 0xaf1a6c, 0.19, 0xe7a525, 0.81), //  #2847
    (180., 0x062053, 0.10, 0x13929a, 0.48), //  #2850
    (180., 0x857664, 0.14, 0x5b4421, 0.60), //  #2853
    (180., 0x2c3fe7, 0.11, 0x261d83, 0.65), //  #2856
];

/// vanilla-settings-08 "Thèmes de couleur" (#2777-#2856): h2 body-md/semibold
/// 16/20 + body-sm 14/18 subtitle, then a `stack_` (padding-top 24, gap 8) of
/// 48px `themeSelection_` tiles (radius 8); the first is the custom theme tile
/// (`borderOverlay_` inset 1px --border-strong, 24px palette icon) with the
/// "nouveau" badge (`newBadge_` top -8, body-xs/semibold 12/16, padding 0 8,
/// round, brand background #2790).
fn nitro_themes(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    let tile = |i: usize, (ang, a, pa, b, pb): (f32, u32, f32, u32, f32)| {
        div()
            .id(eid(format!("nitro-theme-{i}")))
            .flex_none()
            .size(z(48.))
            .rounded(z(8.)) // .themeSelection_ border-radius
            .cursor_pointer()
            .bg(gpui::linear_gradient(
                ang,
                gpui::linear_color_stop(gpui::rgb(a), pa),
                gpui::linear_color_stop(gpui::rgb(b), pb),
            ))
    };
    let custom = div()
        .id("nitro-theme-custom")
        .relative()
        .flex_none()
        .size(z(48.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_STRONG))) // .borderOverlay_ inset 1px --border-strong
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path("palette")).size(z(24.)).text_color(c(tok(idx::ICON_STRONG))))
        .child(
            div()
                .absolute()
                .top(z(-8.)) // .newBadge_ top
                .left(z(-12.))
                .w(z(72.))
                .flex()
                .justify_center()
                .child(
                    div()
                        .px(z(8.))
                        .rounded_full()
                        .bg(c(tok(idx::BACKGROUND_BRAND)))
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(gpui::white())
                        .child(tr("sp.ap.new")),
                ),
        );
    div()
        .flex()
        .flex_col()
        .child(title_text(tr("sp.ap.color_themes")).font_weight(FontWeight::SEMIBOLD))
        .child(desc_text(tr("sp.ap.color_themes_d")))
        .child(
            div()
                .pt(z(24.))
                .flex()
                .flex_wrap()
                .gap(z(8.))
                .child(custom)
                .children(THEME_SWATCHES.iter().enumerate().map(|(i, s)| tile(i, *s))),
        )
        .into_any_element()
}

/// vanilla-settings-08 "Icône de l'appli" (#2903-#2919): heading-xl title +
/// 373px body-sm description, secondary "Icon preview" button at the right,
/// then 48px `appIconSelection_` tiles (radius 8, gap 8, box-shadow 0 0 2px
/// --interactive-text-default). The icon art is a Discord asset we don't
/// ship, so the tiles stay empty.
fn nitro_app_icon(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(z(4.))
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .child(div().flex().flex_col().gap(z(4.)).child(controls::section_title(tr("sp.ap.app_icon"))).child(desc_text(tr("sp.ap.app_icon_d"))))
                .child(controls::button(eid("icon-preview"), tr("sp.ap.icon_preview"), BtnKind::Secondary)),
        )
        .child(div().pt(z(20.)).flex().flex_wrap().gap(z(8.)).children(super::art::app_icons().into_iter().enumerate().map(|(i, u)| app_icon_tile(i, u))))
        // "Icône d'application à durée limitée" (body-md/medium) + the Vice City icon
        .child(div().pt(z(30.)).child(title_text(tr("sp.ap.limited_icon"))))
        .child(div().pt(z(72.)).child(app_icon_tile(99, super::art::limited_app_icon())))
        .into_any_element()
}

/// 48px `appIconSelection_jWcVMn` tile (radius 8, box-shadow 0 0 2px
/// --interactive-text-default) with the client's icon art loaded at runtime.
fn app_icon_tile(i: usize, url: String) -> gpui::Stateful<Div> {
    div()
        .id(eid(format!("nitro-icon-{i}")))
        .size(z(48.))
        .rounded(z(8.))
        .overflow_hidden()
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
        .shadow(vec![gpui::BoxShadow {
            color: c(tok(idx::INTERACTIVE_TEXT_DEFAULT)),
            offset: gpui::point(gpui::px(0.), gpui::px(0.)),
            blur_radius: gpui::px(2.),
            spread_radius: gpui::px(0.),
            inset: false,
        }])
        .child(gpui::img(SharedString::from(url)).size_full().rounded(z(8.)))
}

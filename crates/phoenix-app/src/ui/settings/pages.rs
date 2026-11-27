use gpui::{AnyElement, Context, Div, ElementId, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use super::{AppTheme, LocalPrefs, Page, SettingsPatch, SyncedSettings, save, update_local};
use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{self, BtnKind},
};

type Cx<'a> = Context<'a, PhoenixApp>;
type Render = fn(&PhoenixApp, &mut Context<'_, PhoenixApp>) -> AnyElement;

#[derive(Clone, Copy)]
pub enum Bind {
    Ui(bool),
    Local(fn(&LocalPrefs) -> bool, fn(&mut LocalPrefs, bool)),
    Synced(fn(&SyncedSettings) -> bool, fn(bool) -> SettingsPatch),
    Proto,
    Pref(bool),
    Os(fn() -> bool, fn(bool)),
}

#[derive(Clone, Copy)]
pub enum Pick {
    Ui(usize),
    Synced(fn(&SyncedSettings) -> usize, fn(usize) -> SettingsPatch),
    Proto,
    Pref(usize),
}

pub enum B {
    Title(&'static str),
    Lead(&'static str),
    Legend(&'static str),
    Switch(&'static str, &'static str, &'static str, Bind),
    Sound(&'static str, &'static str),
    Radios(&'static str, &'static str, &'static str, &'static [(&'static str, &'static str)], Pick),
    Select(&'static str, &'static str, &'static str, &'static [&'static str], Pick),
    Action(&'static str, &'static str, &'static str, bool),
    Related(&'static str, &'static str, &'static str, Page),
    Expand(&'static str, &'static str, &'static str, &'static [B]),
    Collapse(&'static str, usize, &'static [B]),
    KeyGroup(&'static str, &'static str, &'static [(&'static str, &'static [&'static str])]),

    Divider,
    Custom(&'static str, Render),
}

fn sound_for(id: &str) -> String {
    match id {
        "snd-msg" => "message1".into(),
        "snd-msg-cur" => "message3".into(),
        "snd-ring" => "call_ringing".into(),
        i => i.strip_prefix("snd-").unwrap_or(i).into(),
    }
}

pub static VOICE_SOUNDS: &[B] = &[B::Collapse(
    "vo-sounds",
    4,
    &[
        B::Sound("snd-deafen", "sp.snd.deafen"),
        B::Sound("snd-undeafen", "sp.snd.undeafen"),
        B::Sound("snd-mute", "sp.snd.mute"),
        B::Sound("snd-unmute", "sp.snd.unmute"),
        B::Sound("snd-camera_on", "sp.snd.camera_on"),
        B::Sound("snd-camera_off", "sp.snd.camera_off"),
        B::Sound("snd-disconnect", "sp.snd.disconnect"),
        B::Sound("snd-ptt_start", "sp.snd.ptt_start"),
        B::Sound("snd-ptt_stop", "sp.snd.ptt_stop"),
        B::Sound("snd-user_join", "sp.snd.user_join"),
        B::Sound("snd-user_leave", "sp.snd.user_leave"),
        B::Sound("snd-user_moved", "sp.snd.user_moved"),
        B::Sound("snd-call_calling", "sp.snd.call_calling"),
        B::Sound("snd-stream_started", "sp.snd.stream_started"),
        B::Sound("snd-stream_ended", "sp.snd.stream_ended"),
        B::Sound("snd-stream_user_joined", "sp.snd.stream_user_joined"),
        B::Sound("snd-stream_user_left", "sp.snd.stream_user_left"),
        B::Sound("snd-activity_launch", "sp.snd.activity_launch"),
        B::Sound("snd-activity_end", "sp.snd.activity_end"),
        B::Sound("snd-activity_user_join", "sp.snd.activity_user_join"),
        B::Sound("snd-activity_user_left", "sp.snd.activity_user_left"),
        B::Sound("snd-reconnect", "sp.snd.reconnect"),
    ],
)];

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

fn tr(k: &str) -> SharedString {
    crate::t!(k)
}

fn title_text(t: SharedString) -> Div {
    div().text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(t)
}

fn desc_text(t: SharedString) -> Div {
    div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(t)
}

fn item(title: SharedString, desc: Option<AnyElement>, trailing: Option<AnyElement>, top: bool) -> Div {
    item_ex(title, desc, trailing, top, false)
}

fn item_ex(title: SharedString, desc: Option<AnyElement>, trailing: Option<AnyElement>, top: bool, dim: bool) -> Div {
    div()
        .flex()
        .gap(z(12.))
        .mx(z(-12.))
        .px(z(12.))
        .when(top, |d| d.py(z(10.)).min_h(z(48.)))
        .when(!top, |d| d.py(z(12.)).min_h(z(48.)))
        .rounded(z(16.))
        .child(div().flex_1().min_w_0().flex().flex_col().justify_center().gap(z(4.)).child(title_text(title).when(dim, |d| d.opacity(0.5))).children(desc.map(|d| div().when(top, |d| d.max_w(z(572.))).child(d))))
        .children(trailing.map(|t| {
            let d = div().flex_none().flex();
            if top { d.items_start().child(t) } else { d.items_center().child(t) }
        }))
}

fn select_width(opts: &[SharedString], cx: &gpui::App) -> f32 {
    let ts = cx.text_system();
    let font = ts.resolve_font(&crate::richtext::ui_font());
    let longest = opts
        .iter()
        .map(|o| o.chars().map(|ch| ts.advance(font, gpui::px(16.), ch).map(|a| f32::from(a.width)).unwrap_or(0.)).sum::<f32>())
        .fold(0., f32::max);
    (longest + 16. + 24. + 8.).max(200.) + 48.
}

fn get_bool(app: &PhoenixApp, id: &str, b: Bind) -> bool {
    match b {
        Bind::Ui(d) => app.settings.as_ref().and_then(|s| s.toggles.get(id).copied()).unwrap_or(d),
        Bind::Local(g, _) => g(&app.prefs.local),
        Bind::Synced(g, _) => g(&app.prefs.synced),
        Bind::Proto => app.prefs.synced.flag(id),
        Bind::Pref(d) => app.prefs.local.flag(id, d),
        Bind::Os(g, _) => g(),
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
        Bind::Pref(_) if id == crate::hwaccel::PREF => {
            if let Some(s) = app.settings.as_mut() {
                s.confirm_hwaccel = Some(v);
            }
            cx.notify();
        }
        Bind::Pref(_) => update_local(app, |p| {
            p.ui.insert(id.to_owned(), v as u64);
        }, cx),
        Bind::Os(_, s) => {
            s(v);
            cx.notify();
        }
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
    let disabled = (matches!(b, Bind::Proto) && super::synced_map::disabled(&synced.proto, synced.compact, id))
        || (id == "theme-devices" && get_bool(app, "theme-sync", Bind::Pref(false)));
    let on = get_bool(app, id, b) || (id == "avatars" && !synced.compact);
    let spoken = title.clone();
    let a11y = move |d: gpui::Stateful<gpui::Div>| crate::ui::a11y::switch(d, spoken, on);
    if disabled {
        let row = item_ex(
            title,
            desc.map(|d| div().opacity(0.5).child(d).into_any_element()),
            Some(div().opacity(0.5).child(controls::switch(eid(format!("sw-knob-{id}")), on)).into_any_element()),
            true,
            true,
        );
        return a11y(row.id(eid(format!("sw-{id}")))).cursor(gpui::CursorStyle::OperationNotAllowed).into_any_element();
    }
    a11y(item(title, desc, Some(controls::switch(eid(format!("sw-knob-{id}")), on).into_any_element()), true).id(eid(format!("sw-{id}"))))
        .group(controls::SWITCH_ROW)
        .cursor_pointer()
        .on_click(cx.listener(move |a, _, _, cx| set_bool(a, id, b, !on, cx)))
        .into_any_element()
}

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
            .shadow(vec![gpui::BoxShadow { color: gpui::hsla(0., 0., 0., 0.16), offset: gpui::point(z(0.), z(2.)), blur_radius: z(8.), spread_radius: z(0.), inset: false }])
            .occlude()
            .children(opts.into_iter().enumerate().map(|(i, o)| {
                let f = on_pick.clone();
                div()
                    .id(eid(format!("sel-{id}-{i}")))
                    .h(z(40.))
                    .pl(z(8.))
                    .pr(z(4.))
                    .gap(z(4.))
                    .rounded(z(4.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
                    .on_click(cx.listener(move |a, _, _, cx| f(a, i, cx)))
                    .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(o))
                    .when(i == cur, |d| {
                        d.child(gpui::svg().path(crate::icons::path("check")).size(z(20.)).flex_none().text_color(c(tok(idx::TEXT_STRONG))))
                    })
            }));
        wrap = wrap.child(gpui::deferred(list).with_priority(2));
    }
    wrap.into_any_element()
}

fn radio_group(app: &PhoenixApp, id: &'static str, opts: &'static [(&'static str, &'static str)], p: Pick, cx: &mut Cx) -> Div {
    let cur = get_pick(app, id, p);
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
        B::Title(t) => out.push(controls::section_title(tr(t)).pb(z(15.)).into_any_element()),
        B::Lead(t) => out.push(div().mt(z(-21.)).pb(z(15.)).child(desc_text(tr(t))).into_any_element()),
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
            let desc = (!d.is_empty()).then(|| {
                let d = tr(d);
                if d.contains('[') { super::account::linked_desc(d) } else { desc_text(d).into_any_element() }
            });
            out.push(switch_row(app, id, tr(t), desc, *bind, cx));
        }
        B::Sound(id, t) => {
            let name = sound_for(id);
            let link = div()
                .id(eid(format!("snd-preview-{id}")))
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(c(tok(idx::TEXT_LINK)))
                .cursor_pointer()
                .hover(|s| s.underline())
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    crate::sounds::preview(&name);
                })
                .child(tr("sp.sound_preview"));
            let bind = Bind::Pref(*id != "snd-msg-cur");
            if app.prefs.local.notification_sounds {
                out.push(switch_row(app, id, tr(t), Some(link.into_any_element()), bind, cx));
            } else {
                let row = item_ex(
                    tr(t),
                    Some(div().opacity(0.5).child(link).into_any_element()),
                    Some(div().opacity(0.5).child(controls::switch(eid(format!("sw-knob-{id}")), false)).into_any_element()),
                    true,
                    true,
                );
                out.push(row.id(eid(format!("sw-{id}"))).cursor(gpui::CursorStyle::OperationNotAllowed).into_any_element());
            }
        }
        B::Collapse(id, n, inner) => {
            let open = app.settings.as_ref().is_some_and(|s| s.expanded.contains(id));
            for b in inner.iter().take(*n) {
                render_block(app, b, out, cx);
            }
            let rest = inner.len().saturating_sub(*n);
            if open {
                for b in inner.iter().skip(*n) {
                    render_block(app, b, out, cx);
                }
            }
            let id = *id;
            let label = if open { crate::t!("sp.snd.hide", n = rest) } else { crate::t!("sp.snd.show", n = rest) };
            out.push(
                item(
                    label,
                    None,
                    Some(
                        gpui::svg()
                            .path(crate::icons::path("chevron"))
                            .size(z(24.))
                            .text_color(c(tok(idx::ICON_STRONG)))
                            .when(open, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI))))
                            .into_any_element(),
                    ),
                    true,
                )
                .id(eid(format!("col-{id}")))
                .cursor_pointer()
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
            let width = select_width(&opts, cx);
            let dd = dropdown(app, id, opts, cur, width, move |a, i, cx| set_pick(a, id, p, i, cx), cx);
            let desc = (!d.is_empty()).then(|| desc_text(tr(d)).into_any_element());
            out.push(item(tr(t), desc, Some(dd), false).gap(z(24.)).into_any_element());
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
        B::Divider => out.push(div().mt(z(50.)).mb(z(40.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))).into_any_element()),
        B::Custom(_, f) => out.push(f(app, cx)),
    }
}

pub fn render(app: &PhoenixApp, blocks: &'static [B], cx: &mut Cx) -> Div {
    let q = super::view::query(app, cx);
    let mut out = Vec::new();
    for b in filter(blocks, &q) {
        render_block(app, b, &mut out, cx);
    }
    div().flex().flex_col().gap(z(10.)).children(out)
}

static DIVIDER: B = B::Divider;

fn block_matches(b: &'static B, q: &str) -> bool {
    let mut keys = Vec::new();
    push_texts(std::slice::from_ref(b), &mut keys);
    keys.iter().filter(|k| !k.is_empty()).any(|k| tr(k).to_lowercase().contains(q))
}

pub(super) fn filter(blocks: &'static [B], q: &str) -> Vec<&'static B> {
    if q.is_empty() {
        return blocks.iter().collect();
    }
    let mut out: Vec<&'static B> = Vec::new();
    for section in blocks.split(|b| matches!(b, B::Divider)) {
        let whole = section.iter().any(|b| matches!(b, B::Title(_)) && block_matches(b, q));
        let kept: Vec<&'static B> = section
            .iter()
            .filter(|b| whole || matches!(b, B::Title(_)) || (!matches!(b, B::Lead(_)) && block_matches(b, q)))
            .collect();
        if kept.iter().all(|b| matches!(b, B::Title(_))) {
            continue;
        }
        if !out.is_empty() {
            out.push(&DIVIDER);
        }
        out.extend(kept);
    }
    out
}

fn push_texts(blocks: &'static [B], out: &mut Vec<&'static str>) {
    for b in blocks {
        match b {
            B::Title(t) | B::Legend(t) | B::Sound(_, t) | B::Custom(t, _) => out.push(t),
            B::Switch(_, t, _, _) | B::Action(t, _, _, _) | B::Related(_, t, _, _) | B::Radios(_, t, _, _, _) | B::Select(_, t, _, _, _) => out.push(t),
            B::Expand(_, t, _, inner) => {
                out.push(t);
                push_texts(inner, out);
            }
            B::Collapse(_, _, inner) => push_texts(inner, out),
            B::KeyGroup(l, _, rows) => {
                out.push(l);
                out.extend(rows.iter().map(|r| r.0));
            }
            B::Lead(_) | B::Divider => {}
        }
    }
}

pub fn titles(p: Page) -> Vec<SharedString> {
    blocks(p).iter().filter_map(|b| if let B::Title(t) = b { Some(tr(t)) } else { None }).collect()
}

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
            "settings.account.mfa",
            "settings.account.devices",
            "settings.account.age_group",
            "settings.account.family",
            "settings.account.family_setup",
            "settings.account.disable",
            "settings.account.close",
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
    let tabs = div().w(z(226.)).flex_none().pt(z(12.)).flex().flex_col().gap(z(20.)).children(TABS.iter().enumerate().map(|(i, (t, _))| {
        let on = i == tab;
        div()
            .id(eid(format!("cf-tab-{i}")))
            .h(z(40.))
            .px(z(10.))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(z(16.))
            .line_height(z(20.))
            .when(on, |d| d.ml(z(-1.)).border_l_2().border_color(c(tok(idx::TEXT_BRAND))).text_color(c(tok(idx::TEXT_BRAND))))
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

static GAMES: &[B] = &[
    B::Title("sp.rg.current"),
    B::Custom("sp.rg.none", current_game),
    B::Divider,
    B::Title("sp.rg.added"),
    B::Lead("sp.rg.igdb"),
    B::Divider,
    B::Related("user-profile", "settings.page.activity", "sp.rg.related_d", Page::ActivityPrivacy),
];

fn current_game(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div()
        .mt(z(10.))
        .flex()
        .flex_col()
        .gap(z(4.))
        .p(z(16.))
        .rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
        .child(
            div()
                .text_size(z(16.))
                .line_height(z(22.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))
                .child(tr("sp.rg.none")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .text_size(z(14.))
                .line_height(z(18.))
                .child(div().text_color(c(tok(idx::TEXT_MUTED))).child(tr("sp.rg.cant_find")))
                .child(
                    div()
                        .id("rg-add")
                        .cursor_pointer()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::TEXT_BRAND)))
                        .child(tr("sp.rg.add")),
                ),
        )
        .into_any_element()
}

static NITRO: &[B] = &[B::Custom("sp.ni.title", nitro_page)];

fn nitro_page(app: &PhoenixApp, _: &mut Cx) -> AnyElement {
    let title: AnyElement = if app.backend.premium_with_group() {
        crate::fonts::nitro(div().h(z(36.)).text_size(z(32.)).line_height(z(24.)), true).text_color(gpui::white()).child(SharedString::from(tr("sp.su.squad").to_uppercase())).into_any_element()
    } else {
        gpui::svg().path(crate::icons::path("settings/nitro-wordmark")).w(z(160.)).h(z(40.)).mb(z(4.)).text_color(gpui::white()).into_any_element()
    };
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
                .child(title)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(z(8.))
                        .child(perk("chat/upload", "sp.ni.p1"))
                        .child(perk("chat/add-reaction", "sp.ni.p3"))
                        .child(perk("super-reaction", "sp.ni.p2"))
                        .child(perk("voice/screen", "sp.ni.p4"))
                        .child(perk("user-square", "sp.ni.p5")),
                )
                .child(
                    div()
                        .pt(z(24.))
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
fn billing_sources(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    let line = || div().h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE)));
    let data = super::billing::get().unwrap_or_default();
    let has_sources = !data.sources.is_empty();
    let source_rows = data.sources.iter().enumerate().map(|(i, src)| {
        div()
            .mt(z(if i == 0 { 52. } else { 16. }))
            .h(z(42.))
            .flex()
            .items_center()
            .gap(z(14.))
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
                                    div().h(z(14.)).px(z(4.)).rounded(z(3.)).flex().items_center().bg(c(tok(idx::PRIMARY_500))).text_size(z(12.)).line_height(z(14.)).font_weight(FontWeight::SEMIBOLD)
                                        .text_color(gpui::white()).child(tr("sp.bl.default").to_uppercase()),
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
        div().mt(z(-20.)).child(desc_text(crate::t!("sp.bl.country", name = name)))
    });
    div()
        .mt(z(-1.))
        .flex()
        .flex_col()
        .children(country)
        .children(source_rows)
        .when(has_sources, |d| d.child(line().mt(z(16.))))
        .child(
            div()
                .mt(z(if has_sources { 16. } else { 50. }))
                .h(z(32.))
                .flex()
                .items_center()
                .child(
                    div().size(z(32.)).flex_none().flex().items_center().child(
                        div().w(z(32.)).h(z(22.)).rounded(z(2.)).bg(c(tok(idx::BRAND_500))).flex().items_center().justify_center()
                            .child(gpui::svg().path(crate::icons::path("discord-logo")).size(z(16.)).text_color(gpui::white())),
                    ),
                )
                .child(div().ml(z(14.)).flex_1().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_DEFAULT))).child(tr("sp.bl.gift")))
                .child(controls::button_sm(eid("billing-redeem"), tr("sp.bl.redeem"), BtnKind::Secondary)),
        )
        .child(line().mt(z(16.)))
        .child(
            div().mt(z(20.)).flex().justify_end().child(controls::button(eid("billing-add"), tr("sp.bl.add"), BtnKind::Primary)),
        )
        .into_any_element()
}

fn billing_history(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    let pays = super::billing::get().map(|b| b.payments).unwrap_or_default();
    let col = |t: &str| div().text_size(z(12.)).line_height(z(12.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_DEFAULT))).child(tr(t));
    div()
        .mt(z(-1.))
        .flex()
        .flex_col()
        .child(
            div()
                .mt(z(3.))
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
        item(tr("sp.data.core"), Some(super::account::linked_desc(tr("sp.data.core_d"))), None, false).into_any_element()
    }),
    B::Switch("data-improve", "sp.data.improve", "sp.data.improve_d", Bind::Proto),
    B::Switch("data-personalize", "sp.data.personalize", "sp.data.personalize_d", Bind::Proto),
    B::Switch("data-sponsored", "sp.data.sponsored", "sp.data.sponsored_d", Bind::Proto),
    B::Switch("data-third", "sp.data.third", "sp.data.third_d", Bind::Proto),
    B::Switch("data-discovery", "sp.data.discovery", "sp.data.discovery_d", Bind::Proto),
    B::Switch("data-clips", "sp.data.clips", "sp.data.clips_d", Bind::Proto),
    B::Action("sp.data.request", "sp.data.request_d", "sp.data.request_b", false),
    B::Related("game-controller", "sp.games", "sp.games_d", Page::RegisteredGames),
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
    let theme = crate::theme::active();
    let gradient = super::client_source(&app.prefs).is_some();
    div()
        .flex()
        .flex_col()
        .pt(z(16.))
        .child(title_text(tr("sp.ap.defaults")).font_weight(FontWeight::SEMIBOLD).pb(z(24.)))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .children(AppTheme::ALL.map(|t| {
                    super::view::theme_card(t, t == theme && !gradient).on_click(cx.listener(move |a, _, _, cx| pick_theme(a, t, cx)))
                }))
                .child(
                    div()
                        .id("theme-sync-os")
                        .size(z(48.))
                        .flex_none()
                        .rounded(z(8.))
                        .bg(c(theme.swatch()))
                        .border_1()
                        .border_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_click(cx.listener(|a, _, _, cx| {
                            update_local(a, |p| {
                                p.ui.insert("theme-sync".into(), 1);
                            }, cx)
                        }))
                        .child(gpui::svg().path(crate::icons::path("refresh")).size(z(24.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
                ),
        )
        .into_any_element()
}

fn pick_theme(app: &mut PhoenixApp, t: AppTheme, cx: &mut Cx) {
    if super::client_source(&app.prefs).is_some() {
        update_local(app, |p| {
            p.ui.insert(super::LOCAL_CLIENT_THEME.into(), 0);
        }, cx);
    }
    if app.prefs.local.flag("theme-sync", false) {
        update_local(app, |p| {
            p.ui.insert("theme-sync".into(), 0);
        }, cx);
    }
    if app.prefs.local.flag("theme-devices", true) {
        save(app, SettingsPatch::Theme(t), cx);
    } else {
        let ix = AppTheme::ALL.iter().position(|x| *x == t).unwrap_or(0) as u64;
        update_local(app, |p| {
            p.ui.insert(super::LOCAL_THEME.into(), ix);
        }, cx);
    }
}

fn theme_banner(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div()
        .mt(z(29.))
        .flex()
        .items_center()
        .gap(z(16.))
        .py(z(16.))
        .px(z(25.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_BASE_LOWEST)))
        .child(gpui::img(SharedString::from(super::art::theme_banner())).w(z(117.)).h(z(69.)).flex_none())
        .child(
            div()
                .w(z(378.))
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

const LIMITED_THEME_END_ALL: i64 = 1_797_184_800_000;
const LIMITED_THEME_END_NITRO: i64 = 1_803_146_400_000;

#[derive(Debug, PartialEq, Eq)]
enum TimeLeft {
    Days(i64),
    Hours(i64),
    UnderHour,
}

fn limited_time_left(now_ms: i64, end_ms: i64) -> Option<TimeLeft> {
    let secs = (end_ms - now_ms) / 1000;
    if secs <= 0 {
        return None;
    }
    let (mut d, mut h, mut m, sec) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60, secs % 60);
    if sec > 0 {
        m += 1;
    }
    if m == 60 {
        h += 1;
        m = 0;
    }
    if h == 24 {
        d += 1;
        h = 0;
    }
    Some(if d > 0 {
        TimeLeft::Days(d + i64::from(h > 0))
    } else if h > 0 {
        let h = h + i64::from(m > 45);
        if h > 11 { TimeLeft::Days(1) } else { TimeLeft::Hours(h) }
    } else if m > 45 {
        TimeLeft::Hours(1)
    } else {
        TimeLeft::UnderHour
    })
}

fn time_left_label(t: TimeLeft) -> SharedString {
    match t {
        TimeLeft::Days(n) => crate::t!("sp.ap.limited_left", n = n),
        TimeLeft::Hours(n) => crate::t!("sp.ap.limited_left_h", n = n),
        TimeLeft::UnderHour => crate::t!("sp.ap.limited_left_m"),
    }
}

fn limited_theme(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let selected = super::client_preset(&app.prefs).is_some_and(|p| p.id == crate::theme::client_theme::VICE_CITY.id);
    let end = if app.backend.premium_tier2() { LIMITED_THEME_END_NITRO } else { LIMITED_THEME_END_ALL };
    let left = limited_time_left(chrono::Utc::now().timestamp_millis(), end).map(time_left_label);
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
                .children(left.map(|left| {
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
                        .child(left)
                })),
        )
        .child(
            div()
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(c(tok(idx::TEXT_MUTED)))
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
                    .relative()
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
                    )
                    .when(selected, |d| d.child(selected_ring()))
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|a, _, _, cx| {
                        update_local(a, |p| {
                            p.ui.insert(super::LOCAL_CLIENT_THEME.into(), u64::from(crate::theme::client_theme::VICE_CITY.id) + 1);
                        }, cx)
                    })),
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
    B::Switch("theme-profiles", "sp.ap.apply_profiles", "", Bind::Pref(false)),
    B::Related("help", "settings.page.accessibility", "sp.ap.a11y_d", Page::Accessibility),
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
    B::Switch("reduced-motion", "sp.ac.motion", "sp.ac.motion_d", Bind::Local(|p| super::effects::effective_reduced_motion(p, super::effects::os_reduced_motion()), |p, v| {
        p.reduced_motion = v;
        p.ui.insert("motion-sync".into(), 0);
    })),
    B::Switch("motion-sync", "sp.ac.motion_sync", "", Bind::Local(|p| p.flag("motion-sync", true), |p, v| {
        if !v {
            p.reduced_motion = super::effects::os_reduced_motion();
        }
        p.ui.insert("motion-sync".into(), u64::from(v));
    })),
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
    B::Switch("startup", "sp.sy.startup", "", Bind::Os(crate::autostart::is_installed, crate::autostart::set)),
    B::Switch("tray", "sp.sy.tray", "sp.sy.tray_d", Bind::Local(|p| p.minimize_to_tray, |p, v| p.minimize_to_tray = v)),
    B::Switch("hwaccel", "sp.sy.hwaccel", "sp.sy.hwaccel_d", Bind::Pref(true)),
    B::Custom("sp.sy.hwaccel", hwaccel_banner),
    B::Divider,
    B::Title("sp.sy.updates"),
    B::Switch("beta-channel", "sp.sy.beta_channel", "sp.sy.beta_channel_d", Bind::Local(|p| p.beta_channel, |p, v| p.beta_channel = v)),
    B::Custom("sp.sy.updates", updates_section),
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
    let cur = crate::i18n::languages::normalize(&app.prefs.synced.locale);
    let langs = crate::i18n::languages::LANGUAGES;
    let ix = crate::i18n::languages::index_of(cur);
    let opts: Vec<SharedString> = langs.iter().map(|l| SharedString::from(l.native)).collect();
    let active = &langs[ix];
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
        .child(div().flex_1().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_STRONG))).child(active.native))
        .child(div().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(idx::TEXT_DEFAULT))).whitespace_nowrap().child(active.english))
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
            save(a, SettingsPatch::Locale(crate::i18n::languages::LANGUAGES[i].code.into()), cx)
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
    B::Related("game-controller", "sp.games", "sp.games_d", Page::RegisteredGames),
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

fn connection_tiles(_: &PhoenixApp, cx: &mut Context<'_, PhoenixApp>) -> AnyElement {
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
            .on_click(cx.listener(move |a, _, _, cx| super::connections::connect(a, super::connections::platform_kind(name), cx)))
            .child(gpui::img(SharedString::from(url)).size(z(logo)).when(app, |i| i.rounded_full()).object_fit(gpui::ObjectFit::Contain))
    };
    div()
        .flex()
        .flex_col()
        .pt(z(10.))
        .gap(z(16.))
        .child(title_text(tr("sp.cn.add")))
        .child(div().flex().flex_wrap().gap_x(z(8.)).gap_y(z(9.)).children(super::art::connections().into_iter().map(tile)))
        .into_any_element()
}

static CONNECTIONS: &[B] = &[
    B::Title("sp.cn.t1"),
    B::Lead("sp.cn.t1_d"),
    B::Custom("sp.cn.add", connection_tiles),
    B::Custom("sp.cn.empty", super::connections::render),
    B::Divider,
    B::Title("sp.cn.t2"),
    B::Lead("sp.cn.t2_d"),
    B::Custom("sp.cn.t2", super::authorized_apps::render),
];

static DEVELOPER: &[B] = &[
    B::Switch(
        "dev-mode",
        "settings.advanced.dev_mode",
        "sp.dv.mode_d",
        Bind::Synced(|s| s.developer_mode, SettingsPatch::DeveloperMode),
    ),
    B::Switch("app-test", "sp.dv.test", "sp.dv.test_d", Bind::Ui(false)),
    B::Switch(
        super::PROTECTED_WRITES,
        "sp.dv.protected",
        "sp.dv.protected_d",
        Bind::Local(LocalPrefs::protected_writes, |p, v| {
            p.ui.insert(super::PROTECTED_WRITES.to_owned(), u64::from(v));
            crate::actions::guard::set_writes_enabled(v);
        }),
    ),
];

static PROFILES: &[B] = &[B::Custom("sp.pf.main", |app, _| super::view::profiles(app).into_any_element())];

fn info_notice(text: SharedString, action: Option<gpui::Stateful<Div>>) -> Div {
    div()
        .flex()
        .items_start()
        .gap(z(16.))
        .p(z(12.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::INLINENOTICE_BORDER_INFO)))
        .bg(c(tok(idx::BACKGROUND_FEEDBACK_MOD_INFO)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .gap(z(8.))
                .child(gpui::svg().path(crate::icons::path("info")).flex_none().size(z(16.)).mt(z(1.)).text_color(c(tok(idx::ICON_FEEDBACK_INFO))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap(z(12.))
                        .child(div().flex_1().min_w_0().max_w(z(410.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_STRONG))).child(text))
                        .children(action),
                ),
        )
}

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

fn updates_section(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    use crate::updater::{self, UpdateStatus};
    let status = updater::status();
    let beta = app.prefs.local.beta_channel;
    let status_text: SharedString = match &status {
        UpdateStatus::Idle => format!("Discord Phoenix v{}", crate::diagnostics::VERSION).into(),
        UpdateStatus::Checking => crate::t!("update.checking"),
        UpdateStatus::UpToDate => format!("{} (v{})", crate::t!("update.up_to_date"), crate::diagnostics::VERSION).into(),
        UpdateStatus::Available(r)
        | UpdateStatus::Downloading { release: r, .. }
        | UpdateStatus::Verifying(r)
        | UpdateStatus::ReadyToRestart { release: r, .. }
        | UpdateStatus::Installed { release: r, .. } => format!("{} {}", crate::t!("update.available_short"), r.tag).into(),
        UpdateStatus::Error(e) => e.clone().into(),
    };
    let status_color = match &status {
        UpdateStatus::Available(_) | UpdateStatus::ReadyToRestart { .. } | UpdateStatus::Installed { .. } => idx::TEXT_FEEDBACK_POSITIVE,
        UpdateStatus::Error(_) => idx::TEXT_FEEDBACK_CRITICAL,
        _ => idx::TEXT_SUBTLE,
    };
    let check_btn = controls::button(eid("check-updates"), crate::t!("update.check_now"), BtnKind::Secondary)
        .on_click(cx.listener(move |_, _, _, cx| updater::spawn_check(beta, cx)));
    let has_release = status.release().is_some();
    let mut row = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(z(12.))
        .child(div().flex_1().min_w_0().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(status_color))).child(status_text))
        .child(check_btn);
    if has_release {
        row = row.child(
            controls::button(eid("view-update"), crate::t!("update.view_update"), BtnKind::Active).on_click(cx.listener(|_, _, w, cx| {
                crate::actions::open_modal(crate::ui::modals::Modal::Update, Some(w));
                cx.notify();
            })),
        );
    }
    row.into_any_element()
}

fn static_id(s: String) -> &'static str {
    thread_local! {
        static IDS: std::cell::RefCell<std::collections::HashMap<String, &'static str>> = Default::default();
    }
    IDS.with(|m| *m.borrow_mut().entry(s.clone()).or_insert_with(|| Box::leak(s.into_boxed_str())))
}

fn kb_label(t: SharedString) -> Div {
    div().pb(z(8.)).text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_STRONG))).child(t)
}

fn kb_field(id: &'static str, text: SharedString, strong: bool) -> gpui::Stateful<Div> {
    div()
        .id(eid(id))
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
        .child(div().flex_1().min_w_0().truncate().text_size(z(16.)).line_height(z(20.)).text_color(c(tok(if strong { idx::TEXT_STRONG } else { idx::TEXT_MUTED }))).child(text))
        .child(div().w(z(36.)).h_full().flex().items_center().justify_center().pr(z(4.)).child(gpui::svg().path("icons/chevron.svg").size(z(20.)).text_color(c(tok(idx::TEXT_STRONG)))))
}

fn kb_recorder(k: &crate::keybinds::Keybind, cx: &mut Cx) -> Div {
    let recording = crate::keybinds::recording() == Some(k.id);
    let id = k.id;
    let has = !k.shortcut.is_empty();
    let label: SharedString = if recording {
        crate::t!("voice.press_key")
    } else if has {
        crate::voice::ptt::label(&k.shortcut).into()
    } else {
        crate::t!("kb.none")
    };
    div()
        .h(z(40.))
        .pl(z(12.))
        .pr(z(4.))
        .rounded(z(8.))
        .bg(c(tok(idx::INPUT_BACKGROUND_DEFAULT)))
        .border_1()
        .border_color(c(tok(if recording { idx::BORDER_FEEDBACK_CRITICAL } else { idx::INPUT_BORDER_DEFAULT })))
        .flex()
        .items_center()
        .gap(z(8.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(z(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(if recording { idx::TEXT_FEEDBACK_CRITICAL } else if has { idx::TEXT_STRONG } else { idx::TEXT_MUTED })))
                .child(label),
        )
        .child(
            controls::button_sm(
                eid(format!("kb-rec-{id}")),
                if recording { crate::t!("voice.stop_recording") } else if has { crate::t!("voice.edit_keybind") } else { crate::t!("voice.record_keybind") },
                if recording { BtnKind::CriticalSecondary } else { BtnKind::Secondary },
            )
            .max_w(z(120.))
            .on_click(cx.listener(move |_, _, _, cx| {
                crate::voice::ptt::set_recording(false);
                crate::keybinds::set_recording(if recording { None } else { Some(id) });
                cx.notify();
            })),
        )
}

fn kb_entry(app: &PhoenixApp, k: &crate::keybinds::Keybind, cx: &mut Cx) -> Div {
    use crate::keybinds::{ACTIONS, Action};
    use crate::ui::tooltip::Tipped;
    let id = k.id;
    let cur = ACTIONS.iter().position(|a| *a == k.action).unwrap_or(0);
    let action = dropdown_with(
        app,
        static_id(format!("kb-act-{id}")),
        kb_field(static_id(format!("kb-act-field-{id}")), k.action.label(), true),
        ACTIONS.iter().map(|a| a.label()).collect(),
        cur,
        280.,
        move |a, i, cx| {
            if let Some(s) = a.settings.as_mut() {
                s.open_select = None;
            }
            crate::keybinds::edit(a, id, |b| b.action = ACTIONS[i], cx)
        },
        cx,
    );
    let enabled = k.enabled;
    let controls_col = div()
        .flex()
        .items_center()
        .justify_end()
        .gap(z(8.))
        .ml(z(24.))
        .pt(z(28.))
        .child(
            div()
                .id(eid(format!("kb-rm-{id}")))
                .size(z(32.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(c(tok(idx::CONTROL_ICON_ONLY_BACKGROUND_HOVER))))
                .child(gpui::svg().path(crate::icons::path("chat/trash")).size(z(20.)).text_color(c(tok(idx::CONTROL_ICON_ONLY_ICON_DEFAULT))))
                .on_click(cx.listener(move |a, _, _, cx| {
                    if crate::keybinds::recording() == Some(id) {
                        crate::keybinds::set_recording(None);
                    }
                    update_local(a, |p| p.keybinds.retain(|k| k.id != id), cx)
                })),
        )
        .child(
            div()
                .relative()
                .tip(if enabled { crate::t!("kb.disable") } else { crate::t!("kb.enable") })
                .child(controls::switch(eid(format!("kb-on-{id}")), enabled).on_click(cx.listener(move |a, _, _, cx| crate::keybinds::edit(a, id, |b| b.enabled = !enabled, cx)))),
        );
    let channel = (k.action == Action::SwitchToVoiceChannel).then(|| {
        let sel = k.channel.map(crate::model::Id).and_then(|cid| app.channels.get(&cid).map(|ch| (cid, ch)));
        let shown: AnyElement = match sel {
            Some((cid, ch)) => {
                let guild = ch.guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| g.name.clone()).unwrap_or_default();
                crate::ui::modals::voice_pick::channel_row(app, cid, ch.name.clone(), guild).into_any_element()
            }
            None => div().px(z(16.)).text_size(z(16.)).line_height(z(20.)).font_weight(FontWeight::MEDIUM).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("kb.no_voice_selected")).into_any_element(),
        };
        div().mt(z(12.)).flex().flex_col().child(kb_label(crate::t!("kb.channel"))).child(
            div()
                .flex()
                .items_stretch()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .mr(z(8.))
                        .min_h(z(40.))
                        .flex()
                        .flex_col()
                        .justify_center()
                        .rounded(z(4.))
                        .border_1()
                        .border_color(c(tok(idx::BORDER_SUBTLE)))
                        .child(shown),
                )
                .child(controls::button(eid(format!("kb-pick-{id}")), crate::t!("kb.pick_channel"), BtnKind::Primary).on_click(cx.listener(move |_, _, _, cx| {
                    crate::actions::open_modal(crate::ui::modals::Modal::VoicePick(crate::ui::modals::voice_pick::VoicePick { keybind: id, search: None }), None);
                    cx.notify();
                }))),
        )
    });
    let portal = (!k.shortcut.is_empty() && k.enabled).then(|| crate::keybinds::portal_trigger(id)).flatten().map(|t| {
        if !shortcut_eligible(&k.shortcut) {
            row_notice(crate::t!("kb.portal_ineligible"), true)
        } else {
            match t {
                Some(d) if !d.is_empty() => row_notice(crate::t!("kb.portal_bound", trigger = d), false),
                Some(_) => div(),
                None => row_notice(crate::t!("kb.portal_pending"), true),
            }
        }
    });
    div()
        .flex()
        .flex_col()
        .children(portal)
        .child(
            div()
                .flex()
                .gap(z(16.))
                .child(div().w(z(280.)).flex_none().flex().flex_col().child(kb_label(crate::t!("kb.action"))).child(action))
                .child(div().w(z(280.)).flex_none().flex().flex_col().child(kb_label(crate::t!("sp.sy.kb_keybind"))).child(kb_recorder(k, cx)))
                .child(controls_col),
        )
        .children(k.action.description().map(|d| div().w(z(280.)).mt(z(2.)).text_size(z(12.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_SUBTLE))).child(d)))
        .children(channel)
}

fn shortcut_eligible(keystroke: &str) -> bool {
    let mut keys = keystroke.split('-').filter(|p| !p.is_empty() && !matches!(*p, "ctrl" | "alt" | "shift" | "cmd" | "super" | "platform"));
    keys.next().is_some() && keys.next().is_none()
}

fn row_notice(text: SharedString, warning: bool) -> Div {
    let (border, bg, icon, glyph) = if warning {
        (idx::INLINENOTICE_BORDER_WARNING, idx::BACKGROUND_FEEDBACK_MOD_WARNING, idx::ICON_FEEDBACK_WARNING, "warning")
    } else {
        (idx::INLINENOTICE_BORDER_INFO, idx::BACKGROUND_FEEDBACK_MOD_INFO, idx::ICON_FEEDBACK_INFO, "info")
    };
    div()
        .mb(z(12.))
        .flex()
        .gap(z(8.))
        .p(z(12.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(border)))
        .bg(c(tok(bg)))
        .child(gpui::svg().path(crate::icons::path(glyph)).flex_none().size(z(16.)).mt(z(1.)).text_color(c(tok(icon))))
        .child(div().flex_1().min_w_0().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_STRONG))).child({
            let (t, ranges) = crate::ui::modals::strong_ranges(&text);
            gpui::StyledText::new(t).with_highlights(ranges.into_iter().map(|r| (r, gpui::HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })))
        }))
}

fn custom_keybinds(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let add = controls::button_icon(eid("kb-add"), "plus", tr("sp.sy.add_kb"), BtnKind::Secondary)
        .on_click(cx.listener(|a, _, _, cx| update_local(a, |p| {
            crate::keybinds::add(&mut p.keybinds);
        }, cx)));
    let mut col = div()
        .flex()
        .flex_col()
        .gap(z(16.))
        .child(
            div()
                .flex()
                .items_start()
                .gap(z(16.))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(z(4.)).child(controls::section_title(tr("sp.sy.t2"))).child(desc_text(tr("sp.sy.t2_d")).max_w(z(427.))))
                .child(add),
        )
        .child(info_notice(tr("sp.sy.kb_disabled"), None))
        .when(crate::keybinds::portal_active(), |d| {
            d.child(info_notice(
                tr("sp.sy.kb_saved"),
                Some(controls::button_sm(eid("kb-os"), tr("sp.sy.kb_os"), BtnKind::Secondary).on_click(|_, _, _| {
                    crate::voice::call::configure_portal();
                })),
            ))
        });
    let mut list: Vec<&crate::keybinds::Keybind> = app.prefs.local.keybinds.iter().collect();
    list.sort_by_key(|k| k.id);
    if list.is_empty() {
        col = col.child(desc_text(tr("sp.sy.no_kb")));
    }
    let n = list.len();
    for (i, k) in list.into_iter().enumerate() {
        col = col.child(kb_entry(app, k, cx));
        if i + 1 < n {
            col = col.child(div().my(z(8.)).h(z(1.)).bg(c(tok(idx::BORDER_SUBTLE))));
        }
    }
    col.into_any_element()
}

fn a11y_preview(app: &PhoenixApp, _: &mut Cx) -> AnyElement {
    let me = &app.me;
    let name: SharedString = if me.display_name.is_empty() { me.name.clone() } else { me.display_name.clone() };
    let msg = |body: AnyElement, extra: Option<Div>| {
        div()
            .relative()
            .pl(z(80.))
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
        .child(title_text(tr("sp.ac.preview")).text_color(c(tok(idx::TEXT_MUTED))))
        .child(
            div()
                .flex()
                .gap(z(8.))
                .p(z(24.))
                .max_h(z({
                    let vh = crate::ui::title_bar::viewport_h_css();
                    0.25 * if vh > 0. { vh } else { 700. }
                }))
                .overflow_hidden()
                .rounded(z(8.))
                .border_1()
                .border_color(c(tok(idx::BORDER_NORMAL)))
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .child(left)
                .child(right),
        )
        .into_any_element()
}

fn theme_title(_: &PhoenixApp, _: &mut Cx) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .pb(z(15.))
        .child(controls::section_title(tr("sp.ap.t1")).pb_0())
        .child(controls::button(eid("theme-preview"), tr("sp.ap.theme_preview"), BtnKind::Secondary))
        .into_any_element()
}

const THEME_SWATCHES: [(f32, u32, f32, u32, f32); 22] = [
    (180., 0xa7d9cd, 0.06, 0xcde3b0, 0.93),
    (180., 0xf9d796, 0.31, 0xf6c0a8, 0.67),
    (149., 0x99bbce, 0.06, 0xbcbcdb, 0.26),
    (38., 0xf7d3d7, 0.04, 0xd1eccf, 0.69),
    (154., 0xcd9cb7, 0.09, 0xd1c89a, 0.76),
    (180., 0xf9d3da, 0.09, 0xd6dffd, 0.94),
    (180., 0xd0defb, 0.07, 0xe6eecf, 0.96),
    (39., 0xf3ece7, 0.13, 0xefeacf, 0.52),
    (180., 0x48288c, 0.28, 0xdb7f4b, 0.71),
    (180., 0x0eb5bf, 0.04, 0xa308a7, 0.40),
    (180., 0x142215, 0.11, 0xa98e4b, 0.84),
    (180., 0x950909, 0.16, 0x000000, 0.72),
    (180., 0x5348ca, 0.11, 0x140730, 0.62),
    (180., 0x895240, 0.15, 0x8f4343, 0.75),
    (180., 0x665069, 0.13, 0x91a3d1, 0.86),
    (180., 0x647962, 0.02, 0x6a8482, 0.96),
    (180., 0x3a7ca1, 0.06, 0x58579a, 0.26),
    (180., 0x01a89e, 0.00, 0xb43898, 1.00),
    (180., 0xaf1a6c, 0.19, 0xe7a525, 0.81),
    (180., 0x062053, 0.10, 0x13929a, 0.48),
    (180., 0x857664, 0.14, 0x5b4421, 0.60),
    (180., 0x2c3fe7, 0.11, 0x261d83, 0.65),
];

fn selected_ring() -> gpui::Div {
    div().absolute().top(z(-2.)).left(z(-2.)).size(z(52.)).child(div().size_full().rounded(z(8.)).border(z(4.)).border_color(c(tok(idx::BRAND_500)))).child(
        div()
            .absolute()
            .top(z(-4.))
            .right(z(-4.))
            .size(z(20.))
            .rounded_full()
            .bg(c(tok(idx::BRAND_500)))
            .flex()
            .items_center()
            .justify_center()
            .child(gpui::svg().path(crate::icons::path("check")).size(z(14.)).text_color(gpui::white())),
    )
}

fn nitro_themes(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let current = super::client_preset(&app.prefs).map(|p| p.id);
    let tile = |i: usize, (ang, a, pa, b, pb): (f32, u32, f32, u32, f32)| {
        let preset = crate::theme::client_theme::PRESETS.get(i).map(|p| p.id);
        let selected = preset.is_some() && preset == current;
        let face = div().size_full().rounded(z(8.)).bg(gpui::linear_gradient(
            ang,
            gpui::linear_color_stop(gpui::rgb(a), pa),
            gpui::linear_color_stop(gpui::rgb(b), pb),
        ));
        div()
            .id(eid(format!("nitro-theme-{i}")))
            .relative()
            .flex_none()
            .size(z(48.))
            .cursor_pointer()
            .child(face)
            .when(selected, |d| d.child(selected_ring()))
            .hover(|s| s.opacity(0.9))
            .when_some(preset, |d, id| {
                d.on_click(cx.listener(move |a, _, _, cx| {
                    update_local(a, |p| {
                        p.ui.insert(super::LOCAL_CLIENT_THEME.into(), u64::from(id) + 1);
                    }, cx)
                }))
            })
    };
    let custom = div()
        .id("nitro-theme-custom")
        .relative()
        .flex_none()
        .size(z(48.))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(idx::BORDER_STRONG)))
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(crate::icons::path("palette")).size(z(24.)).text_color(c(tok(idx::ICON_STRONG))))
        .child(
            div()
                .absolute()
                .top(z(-8.))
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
        .pt(z(30.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(4.))
                .child(title_text(tr("sp.ap.color_themes")).font_weight(FontWeight::SEMIBOLD))
                .child(gpui::svg().path(crate::icons::path("nitro")).size(z(16.)).text_color(c(tok(idx::TEXT_STRONG)))),
        )
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
        .child(div().pt(z(30.)).child(title_text(tr("sp.ap.limited_icon"))))
        .child(div().pt(z(72.)).child(app_icon_tile(99, super::art::limited_app_icon())))
        .into_any_element()
}

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

#[cfg(test)]
mod search_tests {
    use super::*;

    static SAMPLE: &[B] = &[
        B::Title("settings.account.security"),
        B::Lead("settings.account.family_d"),
        B::Action("settings.account.password", "", "", false),
        B::Action("settings.account.devices", "", "", false),
        B::Divider,
        B::Title("settings.account.family"),
        B::Action("settings.account.family_setup", "", "", false),
    ];

    fn titles(v: &[&B]) -> Vec<String> {
        v.iter()
            .map(|b| match b {
                B::Title(t) | B::Lead(t) => (*t).to_owned(),
                B::Action(t, ..) => (*t).to_owned(),
                B::Divider => "-".to_owned(),
                _ => "?".to_owned(),
            })
            .collect()
    }

    #[test]
    fn empty_query_keeps_everything() {
        assert_eq!(filter(SAMPLE, "").len(), SAMPLE.len());
    }

    #[test]
    fn query_keeps_matching_rows_under_their_title() {
        let q = tr("settings.account.devices").to_lowercase();
        assert_eq!(titles(&filter(SAMPLE, &q)), ["settings.account.security", "settings.account.devices"]);
    }

    #[test]
    fn matching_title_keeps_its_whole_section() {
        let q = tr("settings.account.family").to_lowercase();
        let got = titles(&filter(SAMPLE, &q));
        assert!(got.ends_with(&["settings.account.family".to_owned(), "settings.account.family_setup".to_owned()]));
    }
}

#[cfg(test)]
mod limited_tests {
    use super::*;

    #[test]
    fn time_left_rounds_like_the_client_pill() {
        let h = 3_600_000;
        let now = 1_795_617_000_000;
        assert_eq!(limited_time_left(now, LIMITED_THEME_END_NITRO), Some(TimeLeft::Days(88)));
        assert_eq!(limited_time_left(now, LIMITED_THEME_END_ALL), Some(TimeLeft::Days(19)));
        assert_eq!(limited_time_left(0, 2 * 24 * h), Some(TimeLeft::Days(2)));
        assert_eq!(limited_time_left(0, 2 * 24 * h + 1000), Some(TimeLeft::Days(2)));
        assert_eq!(limited_time_left(0, 2 * 24 * h + h), Some(TimeLeft::Days(3)));
        assert_eq!(limited_time_left(0, 5 * h + 50 * 60_000), Some(TimeLeft::Hours(6)));
        assert_eq!(limited_time_left(0, 11 * h + 50 * 60_000), Some(TimeLeft::Days(1)));
        assert_eq!(limited_time_left(0, 50 * 60_000), Some(TimeLeft::Hours(1)));
        assert_eq!(limited_time_left(0, 10 * 60_000), Some(TimeLeft::UnderHour));
        assert_eq!(limited_time_left(10, 10), None);
    }
}

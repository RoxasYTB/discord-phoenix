use crate::theme::{generated::idx, tok, z};
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, Pixels, Point, SharedString, Styled,
    Window, div, img, prelude::*, px,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    model::{Id, ImageRef},
    presence::{Activity, ActivityKind, Popout, Profile, format_date, status_label},
    theme::{DEFAULT_AVATAR, Palette, c, role_color},
    ui::{members::bot_tag_px, status_picker},
};

const POPOUT_W: f32 = 300.;
const POPOUT_H: f32 = 540.;
const MODAL_W: f32 = 962.;
const MODAL_H: f32 = 800.;

impl PhoenixApp {
    pub fn open_profile(
        &mut self,
        user: Id,
        guild: Option<Id>,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::ui::profile_toolbar::reset();
        if self.people.popout.as_ref().is_some_and(|p| p.user == user) {
            self.people.popout = None;
            cx.notify();
            return;
        }
        let note = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("profile.note_placeholder")));
        let sub = cx.subscribe_in(&note, window, move |this, input, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                let text = input.read(cx).value().to_string();
                let changed = this.people.profiles.get(&user).is_some_and(|p| p.note.as_ref() != text);
                if changed {
                    if let Some(p) = this.people.profiles.get_mut(&user) {
                        p.note = text.clone().into();
                    }
                    cx.background_spawn(this.backend.set_note(user, text)).detach();
                }
            }
        });
        self.people.status_picker = false;
        self.people.popout = Some(Popout { user, guild, at, note: note.clone(), _sub: sub });
        let fut = self.backend.user_profile(user, guild);
        cx.spawn_in(window, async move |this, cx| {
            let Ok(p) = fut.await else { return };
            let _ = this.update_in(cx, |a, window, cx| {
                note.update(cx, |n, cx| n.set_value(p.note.to_string(), window, cx));
                a.people.profiles.insert(user, p);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn open_profile_modal(&mut self, user: Id, cx: &mut Context<Self>) {
        self.open_profile_modal_at(user, TAB_BOARD, cx);
    }

    pub fn open_profile_modal_at(&mut self, user: Id, tab: u8, cx: &mut Context<Self>) {
        crate::ui::profile_toolbar::reset();
        self.people.popout = None;
        set_modal_tab(tab);
        self.people.modal = Some(user);
        cx.notify();
    }

    pub fn message_user(&mut self, user: Id, cx: &mut Context<Self>) {
        self.people.close_overlays();
        let fut = self.backend.open_dm(user);
        cx.spawn(async move |this, cx| {
            let Ok(ch) = fut.await else { return };
            let _ = this.update(cx, |a, cx| a.select_channel(ch, cx));
        })
        .detach();
        cx.notify();
    }
}

pub fn overlays(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = crate::ui::user_menu::render(app, cx).into_iter().collect();
    let widget_menu = crate::ui::profile_board::menu();
    let stream_menu = crate::ui::voice_row::stream_menu(cx.entity());
    out.extend(crate::ui::nav_menus::render(app, cx));
    out.extend(crate::ui::thread_menu::render(app, cx));
    out.extend(crate::ui::extra_menus::render(app, cx));
    if !app.people.any_overlay() {
        return out;
    }
    let dim = app.people.modal.is_some();
    out.push(
        div()
            .id("people-backdrop")
            .absolute()
            .inset_0()
            .when(dim, |d| d.bg(c(tok(idx::BACKGROUND_SCRIM))))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|a, _, _, cx| {
                    a.people.close_overlays();
                    cx.notify();
                }),
            )
            .into_any_element(),
    );
    let vp = window.viewport_size();
    if let Some(p) = &app.people.popout {
        let x = (p.at.x - px(POPOUT_W + 16.)).max(z(8.));
        let y = (p.at.y - z(50.)).min(vp.height - px(POPOUT_H + 8.)).max(z(8.));
        out.push(
            div()
                .id("profile-popout")
                .absolute()
                .left(x)
                .top(y)
                .w(px(POPOUT_W + if app.people.profiles.get(&p.user).is_some_and(|x| x.theme.is_some()) { 4. } else { 0. }))
                .max_h(vp.height - y - px(10.))
                .flex()
                .flex_col()
                .occlude()
                .child(popout_card(app, p.user, p.guild, Some(&p.note), cx))
                .into_any_element(),
        );
    }
    if let Some(u) = app.people.modal {
        let w = px(MODAL_W).min(vp.width - px(96.));
        let h = px(MODAL_H).min(vp.height - px(96.));
        out.push(
            div()
                .id("profile-modal")
                .absolute()
                .left(((vp.width - w) / 2.).max(px(0.)))
                .top(((vp.height - h) / 2.).max(px(0.)))
                .w(w)
                .h(h)
                .occlude()
                .child(modal_card(app, u, app.selected_guild, vp, cx))
                .into_any_element(),
        );
    }
    if app.people.status_picker {
        out.push(status_picker::render(app, window, cx));
    }
    out.extend(widget_menu);
    out.extend(stream_menu);
    out
}

pub(crate) fn section_title(t: SharedString) -> Div {
    div()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(tok(idx::TEXT_STRONG)))
        .mb(z(4.))
        .child(t)
}

fn activity_header(a: &Activity) -> Option<SharedString> {
    Some(match a.kind {
        ActivityKind::Custom => return None,
        ActivityKind::Playing => crate::t!("activity.playing"),
        ActivityKind::Streaming if ["twitch", "youtube"].contains(&a.name.to_lowercase().as_str()) => crate::t!("activity.streaming_platform", name = a.name),
        ActivityKind::Streaming => crate::t!("activity.live"),
        ActivityKind::Listening => crate::t!("activity.listening", name = a.name),
        ActivityKind::Watching => crate::t!("activity.watching", name = a.name),
        ActivityKind::Competing => crate::t!("activity.competing"),
    })
}

pub(crate) fn activity_block(a: &Activity, big: bool, bot: bool) -> Option<AnyElement> {
    let title = activity_header(a)?;
    let spotify = a.is_spotify();
    let line1 = if spotify { a.details.clone() } else if a.kind == ActivityKind::Streaming { a.details.clone().or_else(|| Some(a.name.clone())) } else { Some(a.name.clone()) };
    let mut lines: Vec<SharedString> = Vec::new();
    if spotify {
        if let Some(s) = &a.state {
            lines.push(crate::t!("activity.by", s = s));
        }
    } else {
        lines.extend(a.details.clone().filter(|_| a.kind != ActivityKind::Streaming));
        lines.extend(a.state.clone());
    }
    if let Some(start) = a.start.filter(|_| !bot) {
        let mins = (chrono::Utc::now().timestamp_millis() - start).max(0) / 60_000;
        lines.push(if mins >= 60 { format!("{}:{:02}:00", mins / 60, mins % 60) } else { format!("{mins}:00") }.into());
    }
    let s = if big { 60. } else { 48. };
    let art: Option<AnyElement> = (!bot).then(|| match &a.image {
        Some(u) => img(u.clone()).size(z(s)).rounded(z(8.)).flex_none().into_any_element(),
        None => div().size(z(s)).rounded(z(8.)).flex_none().bg(c(tok(idx::BACKGROUND_MOD_NORMAL))).into_any_element(),
    });
    Some(
        div()
            .flex()
            .flex_col()
            .gap(z(8.))
            .p(z(if big { 12. } else { 8. }))
            .rounded(z(if big { 12. } else { 8. }))
            .bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND_HOVER)))
            .border_1()
            .border_color(c(tok(idx::BORDER_SUBTLE)))
            .child(
                div()
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .text_ellipsis()
                    .child(title),
            )
            .child(
                div()
                    .flex()
                    .gap(z(if big { 12. } else { 8. }))
                    .items_center()
                    .children(art)
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(z(4.))
                            .when_some(line1, |d, l| {
                                d.child(
                                    div()
                                        .text_size(z(14.))
                                        .line_height(z(18.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c(tok(idx::TEXT_STRONG)))
                                        .text_ellipsis()
                                        .child(l),
                                )
                            })
                            .children(lines.into_iter().map(|l| {
                                div()
                                    .text_size(z(12.))
                                    .line_height(z(16.))
                                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                                    .text_ellipsis()
                                    .child(l)
                            })),
                    ),
            )
            .into_any_element(),
    )
}

fn section_heading(t: SharedString) -> Div {
    div()
        .text_size(z(12.))
        .line_height(z(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(idx::TEXT_DEFAULT)))
        .child(t)
}

pub(crate) struct Info<'a> {
    pub(crate) user: Option<std::borrow::Cow<'a, crate::model::User>>,
    pub(crate) prof: Option<&'a Profile>,
    pub(crate) presence: crate::model::Presence,
    pub(crate) acts: &'a [Activity],
    pub(crate) custom: Option<SharedString>,
    pub(crate) display: SharedString,
    pub(crate) username: SharedString,
    pub(crate) accent: u32,
    pub(crate) roles: Vec<crate::model::Role>,
}

pub(crate) fn info<'a>(app: &'a PhoenixApp, uid: Id, guild: Option<Id>) -> Info<'a> {
    let user_cow = app.member_user(guild, uid);
    let user = user_cow.as_deref();
    let prof: Option<&Profile> = app.people.profiles.get(&uid);
    let presence = app.presence_of(uid);
    let acts = app.people.activities.get(&uid).map(Vec::as_slice).unwrap_or_default();
    let custom = if uid == app.me.id { app.people.custom_status.clone() } else { None }
        .or_else(|| acts.iter().find(|a| a.kind == ActivityKind::Custom).map(Activity::headline));
    let member = guild.and_then(|g| app.members.get(&g)).and_then(|ms| ms.iter().find(|m| m.user == uid));
    let nick = prof.and_then(|p| p.nick.clone()).or_else(|| {
        guild.and_then(|g| app.people.lists.get(&g)).and_then(|rows| {
            rows.iter().find_map(|r| match r {
                crate::presence::ListEntry::Member(m, n) if m.user == uid => n.clone(),
                _ => None,
            })
        })
    });
    let display = nick.or_else(|| user.map(|u| u.display_name.clone())).unwrap_or_else(|| crate::t!("common.unknown_user"));
    let username = user.map(|u| u.name.clone()).unwrap_or_default();
    let fallback_accent = match user.map(|u| &u.avatar) {
        Some(ImageRef::Default(i)) => DEFAULT_AVATAR[*i as usize % DEFAULT_AVATAR.len()],
        _ => Palette::BLURPLE,
    };
    let avatar_colour = match user.map(|u| &u.avatar) {
        Some(ImageRef::Url(u)) if prof.is_none_or(|p| p.banner.is_none()) => crate::ui::avatar_color::get(u),
        _ => None,
    };
    let accent = prof.and_then(|p| p.accent).or(avatar_colour).unwrap_or(fallback_accent);
    let mut role_ids: Vec<Id> =
        prof.map(|p| p.roles.clone()).filter(|r| !r.is_empty()).or_else(|| member.map(|m| m.roles.clone())).unwrap_or_default();
    let guild_roles = guild.and_then(|g| app.roles.get(&g));
    if let Some(gr) = guild_roles {
        role_ids.sort_by_key(|r| -gr.iter().find(|x| x.id == *r).map(|x| x.position).unwrap_or(0));
    }
    let roles = role_ids.iter().filter_map(|r| guild_roles?.iter().find(|x| x.id == *r).cloned()).collect();
    Info { user: user_cow, prof, presence, acts, custom, display, username, accent, roles }
}

pub(crate) fn banner(i: &Info, h: f32, radius: f32) -> Div {
    div()
        .h(z(h))
        .w_full()
        .flex_none()
        .when(i.prof.is_none_or(|p| p.theme.is_none()), |d| d.bg(c(i.accent)))
        .rounded_t(z(radius))
        .overflow_hidden()
        .border_b_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .when_some(i.prof.and_then(|p| p.banner.clone()), |d, b| d.child(img(b.clone()).w_full().h(z(h)).rounded_t(z(radius))).on_mouse_down(gpui::MouseButton::Right, image_menu(b)))
}

pub(crate) fn image_menu(url: SharedString) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static {
    move |ev, w, cx| {
        cx.stop_propagation();
        crate::ui::extra_menus::open(crate::ui::extra_menus::Target::Image(url.clone()), ev.position, w);
    }
}

pub(crate) fn avatar_block(i: &Info, size: f32, ring: f32, ring_color: u32) -> Div {
    let src = i.user.as_deref().and_then(|u| match &u.avatar {
        ImageRef::Url(url) => Some(url.clone()),
        ImageRef::Default(_) => None,
    });
    div()
        .when_some(src, |d, url| d.on_mouse_down(gpui::MouseButton::Right, image_menu(url)))
        .p(z(ring))
        .rounded_full()
        .bg(c(ring_color))
        .child(crate::ui::avatar_with_status_deco(
            i.user.as_deref(),
            size,
            i.presence,
            c(ring_color),
            |d| d,
            i.prof.and_then(|p| p.decoration.clone()).map(|deco| crate::ui::avatar_deco::src(&deco, true)),
        ))
}

fn badges(i: &Info) -> Option<Div> {
    let b = i.prof.map(|p| p.badges.as_slice()).unwrap_or_default();
    (!b.is_empty()).then(|| {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(z(2.))
            .my(z(1.))
            .px(z(3.))
            .py(z(1.))
            .rounded(z(8.))
            .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
            .border_1()
            .border_color(c(tok(idx::BORDER_MUTED)))
            .children({
                let (shown, hidden) = crate::ui::profile_bio::split_badges(b, i.user.as_deref().is_some_and(|u| u.bot));
                shown
                    .iter()
                    .map(|(_, icon)| img(icon.clone()).size(z(20.)).flex_none().into_any_element())
                    .chain((hidden > 0).then(|| crate::ui::profile_bio::overflow_slot(hidden).into_any_element()))
                    .collect::<Vec<_>>()
            })
    })
}

pub(crate) fn name_block(i: &Info, big: bool, trailing: Option<AnyElement>) -> Div {
    name_block_with(i, big, trailing, false)
}

pub(crate) fn name_block_with(i: &Info, big: bool, trailing: Option<AnyElement>, underline_on_hover: bool) -> Div {
    name_block_clickable(i, big, trailing, underline_on_hover, None)
}

pub(crate) fn name_block_clickable(
    i: &Info,
    big: bool,
    trailing: Option<AnyElement>,
    underline_on_hover: bool,
    entity: Option<gpui::Entity<PhoenixApp>>,
) -> Div {
    let tag = i.user.as_deref().and_then(|u| u.tag.clone());
    let tag_guild = i.user.as_deref().and_then(|u| u.tag_guild);
    let pronouns = i.prof.map(|p| p.pronouns.clone()).filter(|p| !p.is_empty());
    let bot = i.user.as_deref().is_some_and(|u| u.bot);
    let verified = i.user.as_deref().is_some_and(|u| u.verified_bot);
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .group(crate::ui::profile_note::NAME_ROW_GROUP)
                .flex()
                .items_center()
                .gap(z(4.))
                .child(
                    crate::fonts::styled_name(div(), i.user.as_deref(), true)
                        .id(("profile-name-text", i.user.as_deref().map_or(0, |u| u.id.0)))
                        .text_size(z(if big { 24. } else { 20. }))
                        .line_height(z(if big { 32. } else { 24. }))
                        .font_weight(FontWeight::SEMIBOLD)
                        .when(crate::fonts::name_style(i.user.as_deref()).is_none_or(|s| s.2.is_empty()), |d| d.text_color(c(tok(idx::TEXT_STRONG))))
                        .when(underline_on_hover, |d| d.hover(|s| s.underline()))
                        .text_ellipsis()
                        .child(crate::fonts::name_text(i.display.clone(), i.user.as_deref(), if big { 24. } else { 20. })),
                )
                .when(bot, |d| d.child(bot_tag_px(verified)))
                .children(trailing),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .when(bot, |d| d.mt(z(8.)))
                .gap_x(z(6.))
                .gap_y(z(if big { 4. } else { 2. }))
                .text_size(z(14.))
                .line_height(z(18.))
                .child(div().h(z(24.)).flex().items_center().text_color(c(tok(idx::TEXT_STRONG))).child(i.username.clone()))
                .when_some(pronouns, |d, p| {
                    d.child(div().size(z(4.)).mx(z(1.)).rounded_full().bg(c(tok(idx::TEXT_DEFAULT))))
                        .child(div().mr(z(2.)).text_ellipsis().text_color(c(tok(idx::TEXT_DEFAULT))).child(p))
                })
                .when_some(tag, |d, (t, badge)| d.child(guild_tag_pill(t, badge, tag_guild, entity)))
                .when_some(badges(i), |d, b| d.child(b)),
        )
}

fn guild_tag_pill(
    t: SharedString,
    badge: Option<SharedString>,
    guild_id: Option<Id>,
    entity: Option<gpui::Entity<PhoenixApp>>,
) -> impl IntoElement {
    let tip = crate::t!("tag_popout.tooltip", tag = t.as_ref());
    let tag_s = t.clone();
    let badge_s = badge.clone();
    div()
        .id(SharedString::from(format!("guild-tag-{t}")))
        .h(z(24.))
        .px(z(8.))
        .flex()
        .items_center()
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_STRONG))))
        .cursor_pointer()
        .tooltip(crate::ui::tooltip::tip(tip))
        .when_some(guild_id.zip(entity), |d, (gid, ent)| {
            d.on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(move |ev, w, cx| {
                cx.stop_propagation();
                ent.update(cx, |a, cx| a.open_tag_popout(gid, tag_s.clone(), badge_s.clone(), ev.position(), w, cx));
            })
        })
        .text_size(z(12.))
        .line_height(z(16.))
        .text_color(c(tok(idx::TEXT_STRONG)))
        .when_some(badge, |d, b| d.child(img(b).size(z(14.)).mr(z(2.)).flex_none()))
        .child(t)
}

const ROLE_ROW_W: f32 = 268.;

fn role_text_w(cx: &gpui::App, s: &str) -> f32 {
    let ts = cx.text_system();
    let id = ts.resolve_font(&gpui::font(crate::theme::FONT_UI()));
    s.chars().map(|ch| ts.advance(id, px(12.), ch).map(|a| f32::from(a.width)).unwrap_or(0.)).sum()
}

fn role_pill_w(text: f32, r: &crate::model::Role, removable: bool) -> f32 {
    2. + 2. + 12. + 8. + (if r.connection { 16. } else { 0. }) + text + (if r.icon.is_some() { 16. } else { 0. }) + (if removable { 23. } else { 8. })
}

fn roles_fit(widths: &[f32], second_row: f32) -> (usize, usize) {
    let mut shown = 0;
    let mut row2 = 0;
    for row in 0..2 {
        if row == 1 {
            row2 = shown;
        }
        let cap = if row == 1 { second_row } else { ROLE_ROW_W };
        let mut used = 0.;
        while shown < widths.len() {
            let w = widths[shown].min(cap);
            if used + w > cap {
                break;
            }
            used += w + 4.;
            shown += 1;
        }
    }
    (shown, row2)
}

fn roles_block(app: &PhoenixApp, guild: Option<Id>, uid: Id, roles: Vec<crate::model::Role>, cx: &mut Context<PhoenixApp>) -> Div {
    let entity = cx.entity();
    let removable: Vec<bool> = roles.iter().map(|r| guild.is_some_and(|g| role_removable(app, g, uid, r))).collect();
    let widths: Vec<f32> = roles
        .iter()
        .zip(&removable)
        .map(|(r, rm)| role_pill_w(role_text_w(cx, &r.name), r, *rm))
        .collect();
    let expanded = ROLES_EXPANDED.with(|e| e.borrow().contains(&uid));
    let total = roles.len();
    let can_add = guild.is_some_and(|g| {
        use crate::actions::{ADMINISTRATOR, MANAGE_ROLES};
        app.backend.guild_permissions(g) & (ADMINISTRATOR | MANAGE_ROLES) != 0
    });
    let add_w = if can_add { 28. } else { 0. };
    let expand_w = 2. + 6. + 8. + role_text_w(cx, &format!("+{total}")) + 4.;
    let second_row = (ROLE_ROW_W - expand_w - add_w).max(60.);
    let (shown, row2) = if expanded { (total, total) } else { roles_fit(&widths, second_row) };
    let toggle = (shown < total || expanded).then(|| {
        let label = if expanded { crate::t!("profile.roles_collapse") } else { crate::t!("profile.roles_view_all") };
        div()
            .id(("profile-roles-toggle", uid.0))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .min_h(z(24.))
            .min_w(z(24.))
            .when(expanded, |d| d.px(z(4.)))
            .when(!expanded, |d| d.pl(z(6.)).pr(z(8.)))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::USER_PROFILE_BORDER)))
            .text_size(z(12.))
            .line_height(z(16.))
            .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
            .cursor_pointer()
            .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))).text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
            .tooltip(crate::ui::tooltip::tip(label))
            .on_click(move |_, w, cx| {
                cx.stop_propagation();
                ROLES_EXPANDED.with(|e| {
                    let mut e = e.borrow_mut();
                    if !e.remove(&uid) {
                        e.insert(uid);
                    }
                });
                w.refresh();
            })
            .map(|d| {
                if expanded {
                    d.child(gpui::svg().path(crate::icons::path("chevron-small-left")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))
                } else {
                    d.child(SharedString::from(format!("+{}", total - shown)))
                }
            })
    });
    let add_btn = can_add.then(|| {
        let e = entity.clone();
        div()
            .id(("profile-roles-add", uid.0))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(z(24.))
            .rounded(z(8.))
            .cursor_pointer()
            .group("profile-roles-add")
            .tooltip(crate::ui::tooltip::tip(crate::t!("profile.add_role")))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                e.update(cx, |a, cx| a.stub(&crate::t!("profile.add_role"), cx));
            })
            .child(
                gpui::svg()
                    .path(crate::icons::path("plus-small"))
                    .size(z(18.))
                    .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
                    .group_hover("profile-roles-add", |s| s.text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))),
            )
    });
    div().mt(z(2.)).flex().flex_wrap().gap(z(4.)).children(roles.into_iter().take(shown).enumerate().map(|(i, r)| {
        let cap = if !expanded && shown < total && i >= row2 { second_row } else { ROLE_ROW_W };
        let remove = guild.filter(|g| role_removable(app, *g, uid, &r)).map(|g| {
            let e = entity.clone();
            let role = r.clone();
            let group = SharedString::from(format!("profile-role-remove-{}", r.id.0));
            div()
                .id(("profile-role-remove", r.id.0))
                .group(group.clone())
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(z(24.))
                .my(z(-1.))
                .mr(z(-1.))
                .rounded(z(4.))
                .cursor_pointer()
                .tooltip(crate::ui::tooltip::tip(crate::t!("profile.remove_role")))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    e.update(cx, |a, cx| a.remove_profile_role(g, uid, &role, cx));
                })
                .child(
                    gpui::svg()
                        .path(crate::icons::path("close-small"))
                        .size(z(12.))
                        .text_color(c(tok(idx::ICON_SUBTLE)))
                        .group_hover(group, |s| s.text_color(c(tok(idx::ICON_STRONG)))),
                )
        });
        div()
            .flex()
            .items_center()
            .min_h(z(24.))
            .max_w(z(cap))
            .pl(z(2.))
            .when(remove.is_none(), |d| d.pr(z(8.)))
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::USER_PROFILE_BORDER)))
            .text_size(z(12.))
            .line_height(z(16.))
            .text_color(c(tok(idx::TEXT_DEFAULT)))
            .child(div().size(z(12.)).mx(z(4.)).flex_none().rounded_full().bg(role_color(r.color.or(Some(0x99AAB5)))))
            .when(r.connection, |d| {
                d.child(gpui::svg().path(crate::icons::path("link")).size(z(12.)).mr(z(4.)).flex_none().text_color(c(tok(idx::ICON_DEFAULT))))
            })
            .child(div().min_w_0().text_ellipsis().whitespace_nowrap().overflow_hidden().child(r.name.clone()))
            .when_some(r.icon.clone(), |d, u| {
                d.child(div().flex_none().ml(z(4.)).child(img(u).size(z(12.)).object_fit(gpui::ObjectFit::Contain)))
            })
            .children(remove)
    })).children(toggle).children(add_btn)
}

pub(crate) fn member_role_ids(app: &PhoenixApp, guild: Id, user: Id) -> Vec<Id> {
    app.members
        .get(&guild)
        .and_then(|ms| ms.iter().find(|m| m.user == user))
        .map(|m| m.roles.clone())
        .or_else(|| app.people.profiles.get(&user).map(|p| p.roles.clone()))
        .unwrap_or_default()
}

pub(crate) fn sorts_before(guild: Id, a: &crate::model::Role, b: &crate::model::Role) -> bool {
    let everyone = |r: &crate::model::Role| r.id == guild;
    match (everyone(a), everyone(b)) {
        (true, _) => false,
        (false, true) => true,
        _ if a.position != b.position => a.position > b.position,
        _ => a.id < b.id,
    }
}

pub(crate) fn role_removable(app: &PhoenixApp, guild: Id, target: Id, role: &crate::model::Role) -> bool {
    use crate::actions::{ADMINISTRATOR, MANAGE_ROLES};
    let me = app.me.id;
    if role.connection {
        return target == me;
    }
    if role.managed || app.backend.guild_permissions(guild) & (ADMINISTRATOR | MANAGE_ROLES) == 0 {
        return false;
    }
    if app.guilds.iter().any(|g| g.id == guild && g.owner == Some(me)) {
        return true;
    }
    let mine = member_role_ids(app, guild, me);
    let guild_roles = app.roles.get(&guild).map(Vec::as_slice).unwrap_or_default();
    let top = guild_roles.iter().filter(|r| mine.contains(&r.id)).reduce(|b, r| if sorts_before(guild, r, b) { r } else { b });
    top.is_some_and(|t| sorts_before(guild, t, role))
}

pub(crate) fn bio_block(bio: SharedString, clamp: bool) -> Div {
    let long = clamp && crate::ui::profile_bio::bio_is_long(&bio);
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(z(14.))
                .line_height(z(18.))
                .child(crate::ui::profile_bio::bio(&bio, clamp, c(tok(idx::TEXT_DEFAULT)))),
        )
        .when(long, |d| {
            d.child(
                div()
                    .mt(z(18.))
                    .text_size(z(12.))
                    .line_height(z(16.))
                    .text_color(c(tok(idx::TEXT_DEFAULT)))
                    .child(crate::t!("profile.view_full_bio")),
            )
        })
}

pub(crate) fn mutuals_row(p: &Profile, uid: Id, e: &gpui::Entity<PhoenixApp>) -> Option<Div> {
    let friends = p.mutual_friends_count;
    let servers = p.mutual_guilds;
    if friends == 0 && servers == 0 {
        return None;
    }
    let both = friends > 0 && servers > 0;
    let (fs, lh) = if both { (12., 16.) } else { (14., 18.) };
    let text = move |t: SharedString| {
        div().text_size(z(fs)).line_height(z(lh)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).child(t)
    };
    let section = |tab: u8| {
        let e = e.clone();
        div()
            .id(("profile-mutuals", uid.0.wrapping_mul(8).wrapping_add(tab as u64)))
            .flex()
            .items_center()
            .gap(z(4.))
            .py(z(4.))
            .cursor_pointer()
            .hover(|s| s.underline())
            .on_click(move |_, _, cx| e.update(cx, |a, cx| a.open_profile_modal_at(uid, tab, cx)))
    };
    let stack = div().flex().children(p.mutual_friends.iter().take(3).enumerate().map(|(n, u)| {
        div().when(n < 2.min(p.mutual_friends.len().saturating_sub(1)), |d| d.mr(z(-3.))).child(crate::ui::avatar(Some(u), 16.))
    }));
    Some(
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(z(6.))
            .gap_y(z(2.))
            .when(friends > 0, |d| {
                d.child(section(TAB_FRIENDS).when(!p.mutual_friends.is_empty(), |s| s.child(stack)).child(text(if friends == 1 { crate::t!("profile.mutual_friends_one", n = friends) } else { crate::t!("profile.mutual_friends", n = friends) })))
            })
            .when(both, |d| d.child(div().size(z(4.)).rounded_full().bg(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))))
            .when(servers > 0, |d| d.child(section(TAB_SERVERS).child(text(if servers == 1 { crate::t!("profile.mutual_servers_one", n = servers) } else { crate::t!("profile.mutual_servers", n = servers) })))),
    )
}

pub fn mana_button(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>, primary: bool, md: bool) -> gpui::Stateful<Div> {
    let (bg, hover, fg, border) = if primary {
        (
            idx::CONTROL_PRIMARY_BACKGROUND_DEFAULT,
            idx::CONTROL_PRIMARY_BACKGROUND_HOVER,
            idx::CONTROL_PRIMARY_TEXT_DEFAULT,
            idx::CONTROL_PRIMARY_BORDER_DEFAULT,
        )
    } else {
        (
            idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT,
            idx::CONTROL_SECONDARY_BACKGROUND_HOVER,
            idx::CONTROL_SECONDARY_TEXT_DEFAULT,
            idx::CONTROL_SECONDARY_BORDER_DEFAULT,
        )
    };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(z(if md { 40. } else { 32. }))
        .min_w(z(if md { 100. } else { 60. }))
        .px(z(if md { 16. } else { 12. }))
        .rounded(z(8.))
        .border_1()
        .border_color(c(tok(border)))
        .bg(c(tok(bg)))
        .hover(move |s| s.bg(c(tok(hover))))
        .cursor_pointer()
        .text_size(z(14.))
        .line_height(z(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(tok(fg)))
        .whitespace_nowrap()
        .child(label.into())
}

fn wishlist_controls(owner: bool, cx: &mut Context<PhoenixApp>) -> Div {
    use crate::ui::extra_menus::{Target, open, opened};
    let row = div().flex().items_center().gap(z(8.));
    if !owner {
        return row.child(
            mana_button("profile-wishlist-shop", crate::t!("profile.browse_shop"), false, false)
                .gap(z(4.))
                .child(gpui::svg().path(crate::icons::path("shop")).size(z(16.)).flex_none().text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
                .on_click(cx.listener(|a, _, _, cx| a.open_home_view(crate::ui::home::data::HomeView::Shop, cx))),
        );
    }
    let add_open = opened() == Some(Target::WishlistAdd);
    let chevron = if add_open { "close-small" } else { "chevron" };
    let add = mana_button("profile-wishlist-add", crate::t!("wishlist.add_item"), false, false)
        .gap(z(4.))
        .child(gpui::svg().path(crate::icons::path(chevron)).size(z(16.)).flex_none().text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
        .on_click(|ev, w, _| open(Target::WishlistAdd, ev.position(), w));
    let more = mana_button("profile-wishlist-more", SharedString::default(), false, false)
        .min_w(z(32.))
        .w(z(32.))
        .px_0()
        .child(gpui::svg().path(crate::icons::path("more-horizontal")).size(z(16.)).text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
        .on_click(|ev, w, _| open(Target::WishlistMore(None), ev.position(), w));
    row.child(add).child(more)
}

pub(crate) fn banner_button(id: impl Into<gpui::ElementId>, icon: &str) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .size(z(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .bg(c(tok(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_DEFAULT)))
        .hover(|s| s.bg(c(tok(idx::CONTROL_OVERLAY_SECONDARY_BACKGROUND_ACTIVE))))
        .border_1()
        .border_color(c(tok(idx::OPACITY_WHITE_8)))
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).text_color(c(tok(idx::WHITE))))
}

pub(crate) fn theme_backdrop(t: (u32, u32), footer: f32) -> [Div; 3] {
    theme_backdrop_at(t, 105., footer, 8., 8.)
}

pub(crate) fn theme_backdrop_at((primary, secondary): (u32, u32), banner: f32, footer: f32, top_r: f32, bottom_r: f32) -> [Div; 3] {
    [
        div().absolute().top_0().left_0().right_0().h(z(banner)).rounded_t(z(top_r)).bg(c(primary)),
        div()
            .absolute()
            .top(z(banner))
            .left_0()
            .right_0()
            .bottom(z(footer))
            .when(banner <= 0., |d| d.rounded_t(z(top_r)))
            .when(footer <= 0., |d| d.rounded_b(z(bottom_r)))
            .bg(gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(c(primary), 0.),
                gpui::linear_color_stop(c(secondary), 1.),
            )),
        div().absolute().left_0().right_0().bottom_0().h(z(footer)).rounded_b(z(bottom_r)).bg(c(secondary)),
    ]
}

pub(crate) fn ensure_avatar_colour(app: &PhoenixApp, uid: Id, cx: &mut gpui::App) {
    let prof = app.people.profiles.get(&uid);
    if prof.is_some_and(|p| p.banner.is_some() || p.accent.is_some()) {
        return;
    }
    if let Some(ImageRef::Url(u)) = app.users.get(&uid).map(|u| &u.avatar) {
        crate::ui::avatar_color::ensure(u, cx);
    }
}

pub(crate) fn banner_buttons(app: &PhoenixApp, uid: Id, guild: Option<Id>, end: f32, cx: &mut Context<PhoenixApp>) -> Div {
    let friend_btn = crate::ui::profile_toolbar::relationship_button(app, uid, cx);
    let more = (uid != app.me.id).then(|| {
        banner_button(("profile-more", uid.0), "chat/more").tooltip(crate::ui::tooltip::tip(crate::t!("profile.more"))).on_click(cx.listener(
            move |_, ev: &gpui::ClickEvent, w, cx| {
                crate::ui::extra_menus::open(crate::ui::extra_menus::Target::ProfileOverflow { user: uid, guild }, ev.position(), w);
                cx.notify();
            },
        ))
    });
    let mod_view = (uid != app.me.id && mod_view_allowed(app, guild)).then(|| {
        banner_button(("profile-mod-view", uid.0), "moderation")
            .tooltip(crate::ui::tooltip::tip(crate::t!("profile.open_mod_view")))
            .on_click(cx.listener(move |a, _, _, cx| {
                if let Some(g) = guild {
                    a.open_mod_view(g, uid, cx);
                }
            }))
    });
    div().absolute().right(z(end)).top(z(8.)).flex().gap(z(8.)).children(mod_view).children(friend_btn).children(more)
}

pub(crate) fn mod_view_allowed(app: &PhoenixApp, guild: Option<Id>) -> bool {
    use crate::actions::{ADMINISTRATOR, MANAGE_GUILD, MANAGE_NICKNAMES, MANAGE_ROLES, MODERATE_MEMBERS};
    const KICK_MEMBERS: u64 = 1 << 1;
    const BAN_MEMBERS: u64 = 1 << 2;
    let Some(g) = guild else { return false };
    let perms = app.backend.guild_permissions(g);
    perms & (ADMINISTRATOR | MANAGE_GUILD | BAN_MEMBERS | KICK_MEMBERS | MODERATE_MEMBERS | MANAGE_ROLES | MANAGE_NICKNAMES) != 0
}

fn popout_card(
    app: &PhoenixApp,
    uid: Id,
    guild: Option<Id>,
    note: Option<&gpui::Entity<InputState>>,
    cx: &mut Context<PhoenixApp>,
) -> Div {
    let theme = app.people.profiles.get(&uid).and_then(|p| p.theme);
    crate::ui::profile_theme::scoped(theme, || popout_card_inner(app, uid, guild, note, cx))
}

fn popout_card_inner(
    app: &PhoenixApp,
    uid: Id,
    guild: Option<Id>,
    _note: Option<&gpui::Entity<InputState>>,
    cx: &mut Context<PhoenixApp>,
) -> Div {
    ensure_avatar_colour(app, uid, cx);
    let i = info(app, uid, guild);
    let bot = i.user.as_deref().is_some_and(|u| u.bot);
    let theme = i.prof.and_then(|p| p.theme);
    let bg = theme.map(|t| crate::ui::profile_theme::composite(t, t.0)).unwrap_or(tok(idx::BACKGROUND_SURFACE_HIGH));
    let e = cx.entity();
    let e_full = e.clone();
    let header = div()
        .relative()
        .flex_none()
        .min_h(z(105. + 35.))
        .child(banner(&i, 105., if theme.is_some() { 6. } else { 8. }))
        .child(
            div()
                .id(("profile-avatar", uid.0))
                .absolute()
                .left(z(10.))
                .top(z(55.))
                .cursor_pointer()
                .on_click(move |_, _, cx| e_full.update(cx, |a, cx| a.open_profile_modal(uid, cx)))
                .child(avatar_block(&i, 80., 6., bg)),
        )
        .children((uid != app.me.id).then(|| banner_buttons(app, uid, guild, 12., cx)))
        .children(
            crate::ui::profile_toolbar::status(app, uid, guild, crate::ui::profile_toolbar::Layout::Popout, theme.is_some(), i.custom.clone(), cx)
                .map(|s| div().absolute().left(z(105.)).top(z(98.)).child(s)),
        );

    let e_name = e.clone();
    let mut body = div()
        .id(("profile-body", uid.0))
        .flex()
        .flex_col()
        .gap(z(12.))
        .pt(z(4.))
        .pl(z(16.))
        .pr(z(16.))
        .pb(z(8.))
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&crate::ui::chat::scrollbar::handle("profile-popout-body"))
        .child(
            div()
                .id(("profile-name", uid.0))
                .cursor_pointer()
                .on_click(move |_, _, cx| e_name.update(cx, |a, cx| a.open_profile_modal(uid, cx)))
                .child(name_block_clickable(
                    &i,
                    false,
                    (uid != app.me.id && !app.prefs.local.streamer_hides_info()).then(|| crate::ui::profile_note::note_button(uid, cx)),
                    true,
                    Some(cx.entity()),
                )),
        );
    if uid == app.me.id {
        body = body.child(div().text_size(z(12.)).text_color(c(tok(idx::TEXT_MUTED))).child(status_label(i.presence)));
    }
    if uid != app.me.id && !app.prefs.local.streamer_hides_info() && crate::ui::profile_note::editing(uid).is_some() {
        body = body.children(crate::ui::profile_note::note_section(i.prof.map(|p| p.note.clone()), uid, cx));
    }
    match i.prof {
        None => {
            body = body.child(div().text_size(z(12.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("profile.loading")));
        }
        Some(p) => {
            if uid != app.me.id {
                body = body.children(mutuals_row(p, uid, &cx.entity()));
            }
            if !p.bio.is_empty() {
                body = body.child(bio_block(p.bio.clone(), true));
            }
        }
    }
    let board = crate::ui::profile_board::get(app, uid, cx);
    if board.widgets.is_empty()
        && let Some(crumb) = crate::ui::profile_games::breadcrumb(&board.games, theme.is_some())
    {
        let e_crumb = e.clone();
        body = body.child(crumb.on_click(move |_, _, cx| e_crumb.update(cx, |a, cx| a.open_profile_modal(uid, cx))));
    }
    if let Some(w) = board.widgets.first() {
        let col = crate::ui::settings::profile_widgets::WidgetColors {
            strong: c(tok(idx::TEXT_STRONG)),
            muted: c(tok(idx::TEXT_MUTED)),
            card: c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)),
            more: false,
        };
        body = body.child(crate::ui::settings::profile_widgets::render_compact(w, &col));
    }
    let offline = matches!(i.presence, crate::model::Presence::Offline);
    let voice_here = guild.is_none_or(|g| app.channels.values().any(|c| c.guild == Some(g) && c.voice_users.contains(&uid)));
    for a in i.acts.iter().filter(|_| !offline) {
        if let Some(b) = activity_block(a, false, bot) {
            body = body.child(div().pb(z(8.)).child(b));
        }
    }
    if !offline && voice_here {
        let live = crate::ui::voice_row::stream_activity_card(app, uid, cx.entity(), cx);
        body = match live {
            Some(card) => body.child(card),
            None => body.children(crate::ui::voice_row::voice_activity_card(app, uid, cx.entity())),
        };
    }
    if guild.is_some() && !i.roles.is_empty() {
        body = body.child(roles_block(app, guild, uid, i.roles.clone(), cx));
    }

    let footer = if uid == app.me.id {
        Some(div().flex().flex_col().px(z(16.)).pb(z(12.)).child(crate::ui::profile_toolbar::edit_profile_button(uid, guild, cx)))
    } else {
        None
    };
    let footer = footer.or_else(|| crate::ui::profile_toolbar::can_dm(app, uid, guild).then(|| {
        div().flex().flex_col().px(z(16.)).pb(z(12.)).child(
            div()
                .id(("profile-message", uid.0))
                .h(z(44.))
                .pl(z(15.))
                .pr(z(12.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .cursor_text()
                .bg(c(crate::ui::profile_theme::scoped(None, || tok(idx::INPUT_BACKGROUND_DEFAULT))))
                .border_1()
                .border_color(c(crate::ui::profile_theme::scoped(None, || tok(idx::INPUT_BORDER_DEFAULT))))
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.message_user(uid, cx)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(z(14.))
                        .line_height(z(22.))
                        .text_ellipsis()
                        .text_color(c(tok(idx::INPUT_PLACEHOLDER_TEXT_DEFAULT)))
                        .child(crate::t!("profile.message_user", name = i.display)),
                )
                .child(
                    div()
                        .size(z(32.))
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .child(gpui::svg().path(crate::icons::path("emoji")).size(z(18.)).text_color(c(tok(idx::ICON_SUBTLE)))),
                ),
        )
    }));

    let (fx_sku, frame_sku) = i.prof.map(|p| (p.effect_sku, p.frame_sku)).unwrap_or_default();
    let effect = fx_sku.and_then(|s| crate::ui::profile_fx::product(app, s, cx)).and_then(|c| crate::ui::profile_fx::effect(&c, 300., cx));
    let frame = frame_sku.and_then(|s| crate::ui::profile_fx::product(app, s, cx)).and_then(|c| c.frame);
    let body = crate::ui::chat::scrollbar::host(crate::ui::chat::scrollbar::Bar::ThinFade).min_h_0().child(body).children(crate::ui::chat::scrollbar::bar_for("profile-popout-body", crate::ui::chat::scrollbar::Bar::ThinFade));
    let inner = div().flex().flex_col().gap(z(8.)).pb(z(4.)).min_h_0().child(header).child(body).children(footer);
    let card = div()
        .relative()
        .w_full()
        .max_h_full()
        .flex()
        .flex_col()
        .rounded(z(8.))
        .shadow(popout_shadow())
        .overflow_hidden();
    let card = match theme {
        None => card.bg(c(bg)).child(inner),
        Some(t) => card.p(z(2.)).children(theme_backdrop(t, 0.)).child(
            inner.relative().rounded(z(6.)).overflow_hidden().bg(crate::ui::profile_theme::overlay(t)),
        ),
    }
    .children(effect.map(|e| {
        let pad = if theme.is_some() { 2. } else { 0. };
        div().absolute().top(z(pad)).left(z(pad)).right(z(pad)).bottom(z(pad)).child(e)
    }))
    .children(crate::ui::profile_toolbar::scrim(uid, crate::ui::profile_toolbar::Layout::Popout))
    .children(crate::ui::profile_toolbar::toast(uid, 105.));
    let frame_w = if theme.is_some() { 304. } else { 300. };
    match frame {
        None => card,
        Some(f) => div()
            .relative()
            .w_full()
            .max_h_full()
            .flex()
            .flex_col()
            .children(crate::ui::profile_fx::frame(&f, frame_w, false, cx))
            .child(card)
            .children(crate::ui::profile_fx::frame(&f, frame_w, true, cx)),
    }
}

pub(crate) fn popout_shadow() -> Vec<gpui::BoxShadow> {
    vec![
        gpui::BoxShadow {
            color: gpui::hsla(0., 0., 1., 0.08),
            offset: gpui::point(z(0.), z(0.)),
            blur_radius: z(0.),
            spread_radius: z(1.),
            inset: false,
        },
        gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.24),
            offset: gpui::point(z(0.), z(12.)),
            blur_radius: z(24.),
            spread_radius: z(0.),
            inset: false,
        },
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModalActions {
    EditProfile,
    Hidden,
    Others,
}

pub(crate) fn modal_actions(me: bool, rel: crate::ui::home::data::RelationshipKind) -> ModalActions {
    if me {
        ModalActions::EditProfile
    } else if rel == crate::ui::home::data::RelationshipKind::Blocked {
        ModalActions::Hidden
    } else {
        ModalActions::Others
    }
}

fn mutual_tab_label<N: PartialEq + Default + std::fmt::Display>(n: N, none: &str, some: &str) -> SharedString {
    if n == N::default() { crate::t!(none) } else { crate::t!(some, n = n) }
}

pub const TAB_FRIENDS: u8 = 1;
pub const TAB_SERVERS: u8 = 2;
pub const TAB_BOARD: u8 = 3;
pub const TAB_WISHLIST: u8 = 4;

thread_local! {
    static ROLES_EXPANDED: std::cell::RefCell<std::collections::HashSet<Id>> = std::cell::RefCell::new(std::collections::HashSet::new());
    static PANEL_SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
    static MODAL_TAB: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

pub fn set_modal_tab(t: u8) {
    MODAL_TAB.with(|c| c.set(t));
    PANEL_SCROLL.with(|h| h.set_offset(gpui::Point::default()));
}

fn mutual_row(
    id: impl Into<gpui::ElementId>,
    icon: AnyElement,
    name: SharedString,
    tag: Option<AnyElement>,
    sub: Option<SharedString>,
) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .p(z(4.))
        .pr(z(32.))
        .ml(z(12.))
        .my(z(1.))
        .rounded(z(4.))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .child(div().mr(z(10.)).flex_none().child(icon))
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .min_w_0()
                        .child(
                            div()
                                .min_w_0()
                                .text_size(z(16.))
                                .line_height(z(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c(tok(idx::TEXT_STRONG)))
                                .text_ellipsis()
                                .child(crate::ui::emoji_text::label(name, 16.)),
                        )
                        .children(tag),
                )
                .when_some(sub, |d, s| {
                    d.child(
                        div()
                            .text_size(z(12.))
                            .line_height(z(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(tok(idx::TEXT_DEFAULT)))
                            .text_ellipsis()
                            .child(s),
                    )
                }),
        )
}

fn modal_card(app: &PhoenixApp, uid: Id, guild: Option<Id>, vp: gpui::Size<Pixels>, cx: &mut Context<PhoenixApp>) -> Div {
    let theme = app.people.profiles.get(&uid).and_then(|p| p.theme);
    crate::ui::profile_theme::scoped(theme, || modal_card_inner(app, uid, guild, vp, cx))
}

fn modal_card_inner(app: &PhoenixApp, uid: Id, guild: Option<Id>, vp: gpui::Size<Pixels>, cx: &mut Context<PhoenixApp>) -> Div {
    let vw = vp.width;
    let i = info(app, uid, guild);
    let e = cx.entity();
    let theme = i.prof.and_then(|p| p.theme);
    let card_bg = theme.map(|t| crate::ui::profile_theme::composite(t, t.0)).unwrap_or(tok(idx::BACKGROUND_SURFACE_HIGH));
    let pw = (f32::from(vw) * 0.455 - 48. * 0.91).clamp(300., 400.);
    let pad = if f32::from(vw) >= 1025. { 48. } else { 32. };
    let system_bar: f32 = if f32::from(vp.height) >= 785. { 0. } else { 32. };
    const GUTTER: f32 = 32.;
    const CARD_PAD: f32 = 32.;
    const BANNER: f32 = 140.;
    const AV: f32 = 120.;
    const RING: f32 = 7.;

    let e2 = e.clone();
    let icon_btn = |id: (&'static str, u64), icon: &str| {
        div()
            .id(id)
            .size(z(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(z(8.))
            .border_1()
            .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
            .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
            .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
            .cursor_pointer()
            .child(gpui::svg().path(crate::icons::path(icon)).size(z(16.)).text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))))
    };
    let rel = app.nav.relationship(uid).map(|r| r.kind).unwrap_or(crate::ui::home::data::RelationshipKind::None);
    let bot = app.users.get(&uid).is_some_and(|u| u.bot);
    let me = uid == app.me.id;
    let shop_look = (!me && rel != crate::ui::home::data::RelationshipKind::Blocked && crate::ui::shop_look::button_experiment(app, cx)).then(|| {
        crate::ui::shop_look::prefetch(app, uid, cx);
        crate::ui::shop_look::state(&crate::ui::shop_look::skus(app, uid))
    });
    let shop_look = shop_look.filter(|l| crate::ui::shop_look::button_shown(true, l)).map(|_| {
        let es = e.clone();
        icon_btn(("profile-modal-shop-look", uid.0), "shop-sparkle").tooltip(crate::ui::tooltip::tip(crate::t!("shop_look.title"))).on_click(move |ev, w, cx| {
            crate::ui::extra_menus::open(crate::ui::extra_menus::Target::ShopLook(uid), ev.position(), w);
            es.update(cx, |_, cx| cx.notify());
        })
    });
    let actions = modal_actions(me, rel);
    let edit = (actions == ModalActions::EditProfile).then(|| crate::ui::profile_toolbar::edit_profile_button(uid, guild, cx));
    let buttons = (actions != ModalActions::Hidden).then(|| {
        use crate::ui::home::data::RelationshipKind as R;
        let send = |primary: bool| {
            let e2 = e2.clone();
            let fg = if primary { idx::CONTROL_PRIMARY_TEXT_DEFAULT } else { idx::CONTROL_SECONDARY_TEXT_DEFAULT };
            mana_button(("profile-modal-message", uid.0), "", primary, false)
                .on_click(move |_, _, cx| e2.update(cx, |a, cx| a.message_user(uid, cx)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(gpui::svg().path(crate::icons::path("chat")).size(z(16.)).text_color(c(tok(fg))))
                        .child(crate::t!("pe.send_message")),
                )
        };
        let e_more = e.clone();
        let more = icon_btn(("profile-modal-more", uid.0), "chat/more").tooltip(crate::ui::tooltip::tip(crate::t!("profile.more"))).on_click(
            move |ev, w, cx| {
                crate::ui::extra_menus::open(crate::ui::extra_menus::Target::ProfileOverflow { user: uid, guild }, ev.position(), w);
                e_more.update(cx, |_, cx| cx.notify());
            },
        );
        let row = div().flex().items_center().gap(z(8.));
        if me {
            return row.children(edit);
        }
        if bot || rel == R::Incoming {
            return row.child(send(true)).children(shop_look).child(more);
        }
        if matches!(rel, R::Friend | R::Outgoing) {
            let (id, icon, tip) = if rel == R::Friend { ("profile-modal-friend", "user-check", "ptb.friend") } else { ("profile-modal-pending", "user-clock", "ptb.outgoing") };
            let friend = icon_btn((id, uid.0), icon).tooltip(crate::ui::tooltip::tip(crate::t!(tip)));
            let friend = if rel == R::Friend {
                friend.on_click(move |ev, w, _| crate::ui::extra_menus::open(crate::ui::extra_menus::Target::FriendActions(uid), ev.position(), w))
            } else {
                friend.opacity(0.5).cursor_default()
            };
            return row.child(send(true)).child(friend).children(shop_look).child(more);
        }
        let e_add = e.clone();
        let add = mana_button(("profile-modal-add-friend", uid.0), "", true, false)
            .on_click(move |_, _, cx| {
                e_add.update(cx, |a, cx| {
                    a.nav_request(crate::ui::home::data::NavRequest::Relationship { user: uid, action: crate::ui::home::data::RelAction::Accept }, cx, |_, _, _| {})
                })
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(4.))
                    .child(gpui::svg().path(crate::icons::path("invite")).size(z(16.)).text_color(c(tok(idx::CONTROL_PRIMARY_TEXT_DEFAULT))))
                    .child(crate::t!("ptb.add_friend")),
            );
        let e3 = e2.clone();
        let msg_icon = icon_btn(("profile-modal-message-icon", uid.0), "chat")
            .tooltip(crate::ui::tooltip::tip(crate::t!("ptb.message")))
            .on_click(move |_, _, cx| e3.update(cx, |a, cx| a.message_user(uid, cx)));
        row.child(add).child(msg_icon).children(shop_look).child(more)
    });

    let mut pbody = div()
        .flex()
        .flex_col()
        .gap(z(24.))
        .mt(z(-4.))
        .pt(z(4.))
        .pl(z(CARD_PAD))
        .pr(z(CARD_PAD - 8.))
        .pb(z(CARD_PAD))
        .child(name_block(&i, true, None))
        .children(buttons);
    match i.prof {
        None => {
            pbody = pbody.child(div().text_size(z(12.)).text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("profile.loading")));
        }
        Some(p) => {
            if !p.bio.is_empty() {
                pbody = pbody.child(bio_block(p.bio.clone(), false));
            }
            let since = div()
                .flex()
                .items_center()
                .gap(z(8.))
                .text_size(z(14.))
                .line_height(z(18.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(z(4.))
                        .child(gpui::svg().path(crate::icons::path("discord-logo")).size(z(16.)).flex_none().text_color(c(tok(idx::TEXT_SUBTLE))))
                        .child(format_date(p.created)),
                )
                .when_some(p.joined, |d, j| {
                    let gicon = guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).map(|g| {
                        crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 16.)
                    });
                    d.child(div().size(z(4.)).rounded_full().bg(c(tok(idx::TEXT_DEFAULT))))
                        .child(div().flex().items_center().gap(z(4.)).children(gicon).child(format_date(j)))
                });
            pbody = pbody.child(div().flex().flex_col().gap(z(8.)).child(section_heading(crate::t!("profile.member_since"))).child(since));
            if let Some(t) = app.nav.relationship(uid).filter(|r| r.kind == crate::ui::home::data::RelationshipKind::Friend).and_then(|r| r.since) {
                pbody = pbody.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(z(8.))
                        .child(section_heading(crate::t!("profile.friends_since")))
                        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(format_date(t))),
                );
            }
        }
    }
    if guild.is_some() && !i.roles.is_empty() {
        pbody = pbody.child(
            div()
                .flex()
                .flex_col()
                .gap(z(4.))
                .child(section_heading(if i.roles.len() == 1 { crate::t!("profile.role") } else { crate::t!("profile.roles") }))
                .child(roles_block(app, guild, uid, i.roles.clone(), cx)),
        );
    }

    let profile = div()
        .id("profile-modal-card")
        .relative()
        .w(z(pw))
        .flex_none()
        .flex()
        .flex_col()
        .overflow_y_scroll()
        .when(theme.is_none(), |d| d.bg(c(card_bg)))
        .when_some(theme, |d, t| {
            d.children(theme_backdrop_at(t, BANNER, 0., 16., 0.))
                .child(div().absolute().top_0().left_0().right_0().bottom_0().rounded_t(z(16.)).bg(crate::ui::profile_theme::overlay(t)))
        })
        .rounded_t(z(16.))
        .border_1()
        .border_color(c(tok(idx::BORDER_MUTED)))
        .shadow_md()
        .child(banner(&i, BANNER, 16.))
        .child(div().absolute().left(z(CARD_PAD - RING)).top(z(BANNER - AV / 2. - RING)).child(avatar_block(&i, AV, RING, card_bg)))
        .children(
            crate::ui::profile_toolbar::status(app, uid, guild, crate::ui::profile_toolbar::Layout::Modal, theme.is_some(), i.custom.clone(), cx)
                .map(|s| div().absolute().left(z(CARD_PAD + AV + 20.)).top(z(BANNER - 8.)).child(s)),
        )
        .children(crate::ui::profile_toolbar::toast(uid, BANNER))
        .child(div().h(z(220. - BANNER)).flex_none())
        .child(pbody);

    let mutual_f = i.prof.map(|p| p.mutual_friends_count).unwrap_or(0);
    let mutual_s = i.prof.map(|p| p.mutual_guilds).unwrap_or(0);
    let board = crate::ui::profile_board::get(app, uid, cx);
    let has_board = !board.is_empty();
    let sel = MODAL_TAB.with(|c| c.get());
    let has_wishlist = me || !board.wishlist.is_empty();
    let sel = match sel {
        TAB_BOARD if !has_board => 0,
        TAB_WISHLIST if !has_wishlist => 0,
        TAB_FRIENDS | TAB_SERVERS if me => 0,
        s => s,
    };
    let first = if has_board { TAB_BOARD } else { 0 };
    let tab = |n: u8, t: SharedString| {
        let on = sel == n;
        div()
            .id(("profile-modal-tab", n as u64))
            .relative()
            .map(|d| if n == TAB_SERVERS { d.flex_shrink(1.).min_w_0().overflow_hidden().text_ellipsis() } else { d.flex_none() })
            .whitespace_nowrap()
            .when(n != first, |d| d.ml(z(16.)))
            .pb(z(16.))
            .mb(z(-1.))
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(if on { tok(idx::TEXT_STRONG) } else { tok(idx::TEXT_SUBTLE) }))
            .hover(|s| s.text_color(c(tok(idx::TEXT_STRONG))))
            .on_click(move |_, window, _| {
                set_modal_tab(n);
                window.refresh();
            })
            .child(t)
            .when(on, |d| {
                d.child(div().absolute().left_0().right_0().bottom_0().h(z(2.)).rounded_t(z(2.)).bg(c(tok(idx::TEXT_STRONG))))
            })
    };
    let mut panel = div().flex().flex_col().pt(z(20.)).pr(z(8.)).pb(z(16.));
    let empty = |t: SharedString| div().ml(z(12.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(t);
    match sel {
        TAB_BOARD => {
            panel = panel.gap(z(12.));
            let wcol = crate::ui::settings::profile_widgets::WidgetColors {
                strong: c(tok(idx::TEXT_STRONG)),
                muted: c(tok(idx::TEXT_MUTED)),
                card: c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)),
                more: true,
            };
            let gcol = crate::ui::profile_board::Colors {
                strong: c(tok(idx::TEXT_STRONG)),
                card: c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)),
                cover: c(tok(idx::BACKGROUND_MOD_SUBTLE)),
            };
            panel = panel.children(crate::ui::profile_board::ordered(&board, |w| crate::ui::settings::profile_widgets::render(w, &wcol), |w| crate::ui::profile_board::card(w, &gcol)));
        }
        TAB_WISHLIST => {
            let items = &board.wishlist;
            if items.is_empty() {
                panel = panel.child(empty(crate::t!("profile.empty_wishlist")));
            } else {
                panel = panel
                    .child(
                        div()
                            .h(z(32.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_size(z(12.)).line_height(z(16.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_SUBTLE))).child(crate::t!("profile.wishlist_items", n = items.len())))
                            .child(wishlist_controls(uid == app.me.id, cx)),
                    )
                    .child(div().py(z(12.)).flex().flex_wrap().gap(z(16.)).children(items.iter().map(|it| {
                        div()
                            .size(z(133.))
                            .rounded(z(12.))
                            .overflow_hidden()
                            .bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)))
                            .children(it.image.clone().map(|u| img(u).size_full().object_fit(gpui::ObjectFit::Contain)))
                    })));
            }
        }
        TAB_FRIENDS => {
            panel = panel.gap(z(12.));
            let friends = i.prof.map(|p| p.mutual_friends.as_slice()).unwrap_or_default();
            if friends.is_empty() {
                panel = panel.child(empty(crate::t!("profile.no_mutual_friends")));
            }
            for u in friends {
                let fid = u.id;
                let e3 = e.clone();
                let icon = crate::ui::avatar_with_status(Some(u), 40., app.presence_of(u.id), c(tok(idx::BACKGROUND_BASE_LOWER))).into_any_element();
                panel = panel.child(
                    mutual_row(
                        ("mutual-friend", u.id.0),
                        icon,
                        u.display_name.clone(),
                        u.tag.as_ref().map(|t| crate::ui::members::server_tag(t).ml(z(4.)).into_any_element()),
                        info(app, fid, None).custom,
                    )
                    .on_click(move |ev, window, cx| {
                        let at = ev.position();
                        e3.update(cx, |a, cx| {
                            a.people.modal = None;
                            a.open_profile(fid, None, at, window, cx);
                        })
                    }),
                );
            }
        }
        TAB_SERVERS => {
            panel = panel.gap(z(12.));
            let ids = i.prof.map(|p| p.mutual_guild_ids.as_slice()).unwrap_or_default();
            if ids.is_empty() {
                panel = panel.child(empty(crate::t!("profile.no_mutual_servers")));
            }
            for gid in ids {
                let Some(g) = app.guilds.iter().find(|g| g.id == *gid) else { continue };
                let gid = g.id;
                let e3 = e.clone();
                let icon = crate::ui::invite::guild_icon(Some(&g.icon), &g.name, 40.);
                panel = panel.child(mutual_row(("mutual-guild", gid.0), icon, g.name.clone(), None, None).on_click(move |_, _, cx| {
                    e3.update(cx, |a, cx| {
                        a.people.close_overlays();
                        a.select_guild(Some(gid), cx);
                    })
                }));
            }
        }
        _ => {
            panel = panel.gap(z(24.));
            let cards: Vec<AnyElement> = i.acts.iter().filter_map(|a| activity_block(a, true, bot)).collect();
            if cards.is_empty() {
                panel = panel.child(empty(crate::t!("profile.idle_activity", name = i.display)));
            } else {
                panel = panel.child(div().flex().flex_col().gap(z(8.)).children(cards));
            }
        }
    }
    let right = div()
        .id("profile-modal-tabs")
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .pt(z(20.))
        .child(
            div()
                .flex()
                .ml(z(GUTTER))
                .mr(z(16.))
                .mt(z(4.))
                .overflow_hidden()
                .border_b_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .children(has_board.then(|| tab(TAB_BOARD, crate::t!("pe.tab.board"))))
                .child(tab(0, crate::t!("profile.activity")))
                .children(has_wishlist.then(|| tab(TAB_WISHLIST, crate::t!("pe.tab.wishlist"))))
                .children((!me).then(|| tab(TAB_FRIENDS, mutual_tab_label(mutual_f, "profile.tab_no_mutual_friends", "profile.mutual_friends"))))
                .children((!me).then(|| tab(TAB_SERVERS, mutual_tab_label(mutual_s, "profile.tab_no_mutual_servers", "profile.mutual_servers")))),
        )
        .child({
            let h = PANEL_SCROLL.with(|h| h.clone());
            let vh = f32::from(h.bounds().size.height);
            let max = f32::from(h.max_offset().y);
            let thumb = (vh > 0. && max > 0.).then(|| {
                let off = (-f32::from(h.offset().y)).clamp(0., max);
                let len = (vh * vh / (vh + max)).max(f32::from(z(40.))).min(vh);
                let top = (vh - len) * off / max;
                div()
                    .absolute()
                    .top(gpui::px(top) + z(2.))
                    .h(gpui::px(len) - z(4.))
                    .right(z(2.))
                    .w(z(4.))
                    .rounded(z(4.))
                    .group_hover("profile-modal-panel", |s| s.bg(c(tok(idx::SCROLLBAR_THIN_THUMB))))
            });
            div()
                .group("profile-modal-panel")
                .relative()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("profile-modal-panel")
                        .flex_1()
                        .min_h_0()
                        .pl(z(if matches!(sel, 1 | 2) { GUTTER - 12. } else { GUTTER }))
                        .pr(z(8.))
                        .overflow_y_scroll()
                        .track_scroll(&h)
                        .child(panel),
                )
                .children(thumb)
        });

    let mw = (f32::from(vp.width) - 96.).min(MODAL_W);
    let backdrop = i.prof.and_then(|p| p.banner.clone()).and_then(|b| crate::ui::profile_bg::get(&b, mw, cx));
    let e3 = e.clone();
    let close = div()
        .id("profile-modal-close")
        .absolute()
        .top(z(8. + system_bar))
        .right(z(8.))
        .size(z(32.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .border_1()
        .border_color(c(tok(idx::CONTROL_SECONDARY_BORDER_DEFAULT)))
        .bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT)))
        .hover(|s| s.bg(c(tok(idx::CONTROL_SECONDARY_BACKGROUND_HOVER))))
        .on_click(move |_, _, cx| {
            e3.update(cx, |a, cx| {
                a.people.close_overlays();
                cx.notify();
            })
        })
        .child(gpui::svg().path(crate::icons::path("close")).size(z(16.)).text_color(c(tok(idx::CONTROL_SECONDARY_TEXT_DEFAULT))));

    div()
        .relative()
        .size_full()
        .flex()
        .overflow_hidden()
        .rounded(z(16.))
        .border_1()
        .border_color(c(tok(idx::BORDER_NORMAL)))
        .when(theme.is_none(), |d| d.bg(c(tok(idx::BACKGROUND_BASE_LOWER))))
        .when_some(theme, |d, t| {
            use crate::ui::profile_theme::{modal_background, modal_inner};
            d.bg(modal_background(t))
                .child(div().absolute().top_0().left_0().right_0().h(z(300.)).rounded_t(z(15.)).bg(gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(modal_inner(t, 0.8), 0.),
                    gpui::linear_color_stop(modal_inner(t, 0.2), 1.),
                )))
                .child(div().absolute().top(z(300.)).left_0().right_0().bottom_0().rounded_b(z(15.)).bg(modal_inner(t, 0.2)))
        })
        .children(backdrop.map(|(a, h)| img(gpui::ImageSource::Render(a)).absolute().top_0().left_0().w(z(mw)).h(z(h)).rounded_t(z(15.))))
        .shadow_lg()
        .pt(z(pad + system_bar))
        .pl(z(pad))
        .pr(z(32.))
        .child({
            let frame = i
                .prof
                .and_then(|p| p.frame_sku)
                .and_then(|s| crate::ui::profile_fx::product(app, s, cx))
                .and_then(|c| c.frame);
            match frame {
                None => profile.into_any_element(),
                Some(f) => div()
                    .relative()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .children(crate::ui::profile_fx::frame(&f, pw, false, cx))
                    .child(profile)
                    .children(crate::ui::profile_fx::frame(&f, pw, true, cx))
                    .into_any_element(),
            }
        })
        .child(right)
        .child(close)
}

#[cfg(test)]
mod role_order_tests {
    use super::sorts_before;
    use crate::model::{Id, Role};

    fn role(id: u64, position: i32) -> Role {
        Role { id: Id(id), name: "r".into(), color: None, position, hoist: false, icon: None, managed: false, connection: false }
    }

    #[test]
    fn higher_role_matches_the_client_comparator() {
        let g = Id(1);
        assert!(sorts_before(g, &role(10, 5), &role(11, 3)));
        assert!(!sorts_before(g, &role(11, 3), &role(10, 5)));
        assert!(sorts_before(g, &role(10, 4), &role(11, 4)));
        assert!(!sorts_before(g, &role(11, 4), &role(10, 4)));
        assert!(sorts_before(g, &role(10, 0), &role(1, 0)));
        assert!(!sorts_before(g, &role(1, 0), &role(10, 0)));
        assert!(!sorts_before(g, &role(10, 4), &role(10, 4)));
    }
}

#[cfg(test)]
mod role_fit_tests {
    use super::{role_pill_w, roles_fit};
    use crate::model::{Id, Role};

    #[test]
    fn role_pill_reserves_dot_link_name_icon_and_remove_like_the_client() {
        let mut r = Role { id: Id(2), name: "r".into(), color: None, position: 1, hoist: false, icon: None, managed: false, connection: false };
        assert_eq!(role_pill_w(40., &r, false), 2. + 2. + 12. + 8. + 40. + 8.);
        r.icon = Some("https://cdn.discordapp.com/role-icons/2/a.webp".into());
        assert_eq!(role_pill_w(40., &r, false), 2. + 2. + 12. + 8. + 40. + 16. + 8.);
        r.connection = true;
        assert_eq!(role_pill_w(40., &r, true), 2. + 2. + 12. + 8. + 16. + 40. + 16. + 23.);
    }

    #[test]
    fn roles_fill_two_rows_like_the_client() {
        assert_eq!(roles_fit(&[100., 100., 100.], 230.), (3, 2));
        assert_eq!(roles_fit(&[100., 100., 100., 100., 100.], 230.), (4, 2));
        assert_eq!(roles_fit(&[300., 300., 300.], 230.), (2, 1));
        assert_eq!(roles_fit(&[], 230.), (0, 0));
    }
}

#[cfg(test)]
mod activity_header_tests {
    use super::activity_header;
    use crate::presence::{Activity, ActivityKind};

    fn streaming(name: &str) -> Activity {
        Activity {
            kind: ActivityKind::Streaming,
            name: name.to_owned().into(),
            details: None,
            state: None,
            emoji: None,
            url: None,
            display_type: None,
            image: None,
            start: None,
            application_id: None,
        }
    }

    #[test]
    fn own_profile_modal_shows_only_edit_profile() {
        use super::{ModalActions, modal_actions};
        use crate::ui::home::data::RelationshipKind as R;
        assert_eq!(modal_actions(true, R::None), ModalActions::EditProfile);
        assert_eq!(modal_actions(true, R::Friend), ModalActions::EditProfile);
        assert_eq!(modal_actions(false, R::Friend), ModalActions::Others);
        assert_eq!(modal_actions(false, R::None), ModalActions::Others);
        assert_eq!(modal_actions(false, R::Blocked), ModalActions::Hidden);
    }

    #[test]
    fn empty_mutual_tabs_use_the_client_none_label() {
        assert_eq!(super::mutual_tab_label(0u32, "profile.tab_no_mutual_friends", "profile.mutual_friends"), crate::t!("profile.tab_no_mutual_friends"));
        assert_eq!(super::mutual_tab_label(3usize, "profile.tab_no_mutual_servers", "profile.mutual_servers"), crate::t!("profile.mutual_servers", n = 3));
    }

    #[test]
    fn streaming_header_names_only_known_platforms() {
        assert_eq!(activity_header(&streaming("Twitch")), Some(crate::t!("activity.streaming_platform", name = "Twitch")));
        assert_eq!(activity_header(&streaming("youtube")), Some(crate::t!("activity.streaming_platform", name = "youtube")));
        assert_eq!(activity_header(&streaming("J'aime bien ce point violet.")), Some(crate::t!("activity.live")));
        let mut custom = streaming("x");
        custom.kind = ActivityKind::Custom;
        assert_eq!(activity_header(&custom), None);
    }
}

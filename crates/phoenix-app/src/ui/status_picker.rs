//! Account popout, opened from the avatar in the bottom user panel. Rebuilt
//! from the vanilla client dumps `vanilla-account-popout` (#3643
//! `outer_P1YEjQ` .. #3794) and `vanilla-account-status-sub` (#2911
//! `menu_M3qk4j statusPickerModalMenu_nitlyF`), 2026-11-23:
//! - the regular profile popout card (banner, avatar, editable custom status
//!   bubble, name + copy-username button, username / server tag / badges, bio,
//!   activities), Nitro theme gradient when the profile has one;
//! - `menus_nitlyF` (gap 12): [Edit Profile | Status > | Badges] and
//!   [Switch Accounts > | Copy User ID] overlay groups. The Orbs balance row
//!   is not reproduced (no Orbs data is fetched).
//! - the status submenu (300px): Online ‖ Idle, Do Not Disturb (subtext),
//!   Invisible (subtext), each with a 10px status dot in a 20px accessory.
//!
//! Status changes go through op 3 and `PATCH /users/@me/settings`.

use crate::theme::z;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, anchored, deferred, div,
    point, prelude::*, px, svg,
};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    model::Presence,
    presence::status_label,
    theme::{c, generated::idx, tok},
    ui::{
        profile::{avatar_block, banner, ensure_avatar_colour, info, name_block, theme_backdrop},
        status_dot,
    },
};

impl PhoenixApp {
    pub fn toggle_status_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = !self.people.status_picker;
        self.people.close_overlays();
        if open {
            let cur = self.people.custom_status.clone().unwrap_or_default();
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("status.custom_placeholder")).default_value(cur));
            let sub = cx.subscribe_in(&input, window, |this, input, ev: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    let t = input.read(cx).value().trim().to_owned();
                    this.set_custom_status((!t.is_empty()).then_some(t), cx);
                    this.people.close_overlays();
                }
            });
            self.people.custom_input = Some(input);
            self.people.custom_sub = Some(sub);
            self.people.status_picker = true;
            // The card shows our own profile (banner, theme, bio, badges).
            let me = self.me.id;
            if !self.people.profiles.contains_key(&me) {
                let fut = self.backend.user_profile(me, None);
                cx.spawn(async move |this, cx| {
                    let Ok(p) = fut.await else { return };
                    let _ = this.update(cx, |a, cx| {
                        a.people.profiles.insert(me, p);
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn set_own_status(&mut self, p: Presence, cx: &mut Context<Self>) {
        self.presence = p;
        let custom = self.people.custom_status.as_ref().map(|s| s.to_string());
        cx.background_spawn(self.backend.set_status(p, custom)).detach();
        self.people.close_overlays();
        cx.notify();
    }

    pub fn set_custom_status(&mut self, text: Option<String>, cx: &mut Context<Self>) {
        self.people.custom_status = text.clone().map(Into::into);
        cx.background_spawn(self.backend.set_status(self.presence, text)).detach();
        cx.notify();
    }

    /// Custom status from the "Set your status" modal, cleared by the server at `expires_at`.
    pub fn set_custom_status_until(&mut self, text: Option<String>, expires_at: Option<String>, cx: &mut Context<Self>) {
        self.people.custom_status = text.clone().map(Into::into);
        cx.background_spawn(self.backend.set_status_until(self.presence, text, expires_at)).detach();
        cx.notify();
    }

    /// Pencil on the status bubble: the account popout closes and the modal opens.
    pub fn open_custom_status_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cur = self.people.custom_status.as_ref().map(|s| s.to_string());
        self.people.close_overlays();
        crate::actions::open_modal(crate::ui::modals::Modal::CustomStatus(crate::ui::modals::custom_status::State::new(cur.as_deref())), Some(window));
        cx.notify();
    }
}

/// Status submenu order (Online, separator, then the rest with subtexts).
const ITEMS: [(Presence, Option<&str>); 4] = [
    (Presence::Online, None),
    (Presence::Idle, None),
    (Presence::Dnd, Some("status.dnd_desc")),
    (Presence::Offline, Some("status.invisible_desc")),
];

thread_local! {
    /// Status submenu shown next to the popout.
    static STATUS_SUB: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn set_sub(v: bool, w: &mut Window) {
    if STATUS_SUB.with(|c| c.replace(v)) != v {
        w.refresh();
    }
}

/// Open the status submenu (offscreen screenshots).
pub fn debug_open_status_submenu(v: bool) {
    STATUS_SUB.with(|c| c.set(v));
}

/// `menuItemInner_nitlyF`: radius 8, --interactive-text-default, hover
/// --user-profile-overlay-background-hover / --interactive-text-hover.
/// `menuItemContent_nitlyF`: padding 8 (end 0 before a submenu caret), gap 8;
/// `menuItemIcon_nitlyF` 16px; label body-sm/medium 14/16; trailing
/// `submenuIconWrapper_nitlyF` padding-inline-start 8 + 16px chevron.
fn pop_row(id: &'static str, icon: AnyElement, label: AnyElement, trailing: Option<AnyElement>, caret: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .rounded(z(8.))
        .p(z(8.))
        .flex()
        .items_center()
        .gap(z(8.))
        .cursor_pointer()
        .text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))
        .hover(|s| s.bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND_HOVER))).text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER))))
        .child(div().size(z(16.)).flex_none().flex().items_center().justify_center().child(icon))
        .child(
            // menuItemLabel_nitlyF: flex 1, gap 4, space-between
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .gap(z(4.))
                .child(div().text_size(z(14.)).line_height(z(16.)).font_weight(FontWeight::MEDIUM).child(label))
                .children(trailing),
        )
        .when(caret, |d| d.child(div().pl(z(8.)).flex_none().child(svg().path(crate::icons::path("chevron-right")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))))))
}

fn icon(name: &str) -> AnyElement {
    svg().path(crate::icons::path(name)).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).into_any_element()
}

fn text(t: SharedString) -> AnyElement {
    div().child(t).into_any_element()
}

/// `overlay_P1YEjQ menuOverlay_nitlyF`: padding 8, radius 8,
/// --user-profile-overlay-background; `menuItem_nitlyF:after` separators
/// (1px --user-profile-border, margin 8px 4px) between items.
fn pop_group(rows: Vec<AnyElement>) -> gpui::Div {
    let n = rows.len();
    let mut g = div().rounded(z(8.)).p(z(8.)).bg(c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND))).flex().flex_col();
    for (k, r) in rows.into_iter().enumerate() {
        g = g.child(r);
        if k + 1 < n {
            g = g.child(div().h(z(1.)).mx(z(4.)).my(z(8.)).bg(c(tok(idx::USER_PROFILE_BORDER))));
        }
    }
    g
}

/// Status submenu (`menu_M3qk4j`: 300px, --background-surface-higher, 1px
/// --border-subtle, radius 8; scroller padding 8; `item_M3qk4j` padding 8,
/// 14/18 500; `statusAccessory_` 20px box margin-end 8 with a 10px dot;
/// subtext body-xs/normal 12/16 --text-muted margin-top 2; separator margin 8 0).
fn status_submenu(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    let bg = c(tok(idx::BACKGROUND_SURFACE_HIGHER));
    let mut menu = div()
        .id("status-sub")
        .occlude()
        .w(z(300.))
        .p(z(8.))
        .rounded(z(8.))
        .bg(bg)
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow_lg()
        .flex()
        .flex_col()
        .on_hover(|h, w, _| {
            if !*h {
                set_sub(false, w)
            }
        });
    for (k, (p, desc)) in ITEMS.into_iter().enumerate() {
        if k == 1 {
            menu = menu.child(div().h(z(1.)).my(z(8.)).bg(c(tok(idx::BORDER_SUBTLE))));
        }
        let selected = app.presence == p;
        menu = menu.child(
            div()
                .id(("status-item", k))
                .p(z(8.))
                .min_h(z(36.))
                .rounded(z(4.))
                .flex()
                .items_start()
                .cursor_pointer()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(tok(idx::INTERACTIVE_TEXT_HOVER)))
                .when(selected, |d| d.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
                .on_click(cx.listener(move |a, _, _, cx| a.set_own_status(p, cx)))
                .child(div().size(z(20.)).mr(z(8.)).flex_none().flex().items_center().justify_center().child(status_dot(p, 10., bg)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pt(z(1.))
                        .child(status_label(p))
                        .children(desc.map(|d| {
                            div()
                                .mt(z(2.))
                                .text_size(z(12.))
                                .line_height(z(16.))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(c(tok(idx::TEXT_MUTED)))
                                .child(crate::t!(d))
                        })),
                ),
        );
    }
    menu
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let me = app.me.id;
    ensure_avatar_colour(app, me, cx);
    let i = info(app, me, None);
    let theme = i.prof.and_then(|p| p.theme);
    let surface = tok(idx::BACKGROUND_SURFACE_HIGH);
    let ring = theme.map(|(p, _)| {
        let ch = |sh: u32| (((p >> sh) & 0xFF) as f32 * 0.4).round() as u32;
        (ch(16) << 16) | (ch(8) << 8) | ch(0)
    });
    // header_NOlkhE: 151px; banner 105; avatar_WRLIGC top 59 / start 14 (80px + 6px ring).
    let header = div()
        .relative()
        .flex_none()
        .h(z(151.))
        .child(banner(&i, 105., 6.))
        .child(div().absolute().left(z(8.)).top(z(53.)).child(avatar_block(&i, 80., 6., ring.unwrap_or(surface))));
    // container_rmCN9_ editable: start 105, top 98, end 14; outer_rmCN9_ radius 16,
    // 1px --border-muted, --background-surface-highest; inner padding 8 12, 14/18.
    let bubble = app.people.custom_input.as_ref().map(|input| {
        let has = app.people.custom_status.is_some();
        div()
            .absolute()
            .left(z(105.))
            .top(z(98.))
            .right(z(14.))
            .px(z(12.))
            .py(z(4.))
            .rounded(z(16.))
            .border_1()
            .border_color(c(tok(idx::BORDER_MUTED)))
            .bg(c(tok(idx::BACKGROUND_SURFACE_HIGHEST)))
            .shadow_sm()
            .flex()
            .items_center()
            .gap(z(4.))
            .text_size(z(14.))
            .line_height(z(18.))
            .text_color(c(tok(idx::TEXT_STRONG)))
            .child(div().flex_1().min_w_0().child(Input::new(input).appearance(false).bordered(false)))
            .when(has, |d| {
                // bar_UIX3Vg "Clear custom status" (TrashIcon 16px)
                d.child(
                    div()
                        .id("custom-clear")
                        .size(z(24.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                        .on_click(cx.listener(|a, _, _, cx| {
                            a.set_custom_status(None, cx);
                            a.people.close_overlays();
                        }))
                        .child(svg().path(crate::icons::path("chat/trash")).size(z(16.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT)))),
                )
            })
    });

    // copyButton_hpQ_E_: 24px, padding 4, round, CopyIcon 16 --interactive-text-default.
    let username = app.me.name.to_string();
    let copy_name = div()
        .id("acct-copy-username")
        .size(z(24.))
        .p(z(4.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .on_click(cx.listener(move |_, _, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(username.clone()))))
        .child(icon("chat/copy"))
        .into_any_element();

    // Status row: 12px status dot (mainStatusIcon_), label, BellSlashIcon 12px
    // while Do Not Disturb mutes notifications, submenu chevron.
    let dnd = app.presence == Presence::Dnd;
    let sub_open = STATUS_SUB.with(|c| c.get());
    let status_row = pop_row(
        "acct-status",
        status_dot(app.presence, 12., c(tok(idx::BACKGROUND_SURFACE_HIGH))).into_any_element(),
        text(status_label(app.presence)),
        dnd.then(|| svg().path(crate::icons::path("bell-slash")).size(z(12.)).text_color(c(tok(idx::INTERACTIVE_TEXT_DEFAULT))).into_any_element()),
        true,
    )
    .relative()
    .on_hover(|h, w, _| {
        if *h {
            set_sub(true, w)
        }
    })
    .on_click(|_, w, _| set_sub(!STATUS_SUB.with(|c| c.get()), w))
    // statusPickerModalMenu_: 12px past the row's end, 14px above its top.
    .when(sub_open, |d| {
        d.child(deferred(
            anchored().position_mode(gpui::AnchoredPositionMode::Local).position(point(px(252. + 12.), px(-14.))).snap_to_window().child(status_submenu(app, cx)),
        ))
    });
    let hover_off = |id: &'static str, ic: &str, label: SharedString| {
        pop_row(id, icon(ic), text(label), None, false).on_hover(|h, w, _| {
            if *h {
                set_sub(false, w)
            }
        })
    };
    let edit = hover_off("acct-edit", "pen", crate::t!("status.edit_profile")).on_click(cx.listener(|a, _, _, cx| {
        a.people.close_overlays();
        crate::ui::settings::open(a, cx);
        if let Some(s) = a.settings.as_mut() {
            s.page = crate::ui::settings::Page::Profiles;
        }
    }));
    let badges = hover_off("acct-badges", "ribbon", crate::t!("status.badges"))
        .on_click(cx.listener(|a, _, _, cx| a.stub(&crate::t!("status.badges"), cx)));
    let switch = pop_row("acct-switch", icon("user-circle"), text(crate::t!("status.switch_accounts")), None, true)
        .on_hover(|h, w, _| {
            if *h {
                set_sub(false, w)
            }
        })
        .on_click(cx.listener(|a, _, _, cx| a.stub(&crate::t!("status.switch_accounts"), cx)));
    let my_id = me.0.to_string();
    let copy = hover_off("acct-copy-id", "id", crate::t!("common.copy_user_id")).on_click(cx.listener(move |a, _, _, cx| {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(my_id.clone()));
        a.people.close_overlays();
        cx.notify();
    }));
    let menus = div()
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(pop_group(vec![edit.into_any_element(), status_row.into_any_element(), badges.into_any_element()]))
        .child(pop_group(vec![switch.into_any_element(), copy.into_any_element()]));

    // body_nitlyF: padding 4 8 16 16 (scrollbar gutter), gap 12; userInfo_ margin-bottom -4.
    let mut body = div()
        .id("acct-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .pt(z(4.))
        .pr(z(8.))
        .pb(z(16.))
        .pl(z(16.))
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(div().mb(z(-4.)).child(name_block(&i, false, Some(copy_name))));
    if let Some(bio) = i.prof.map(|p| p.bio.clone()).filter(|b| !b.is_empty()) {
        // descriptionClamp_BcddaK: 3 lines of body-sm 14/18
        body = body.child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_STRONG))).line_clamp(3).child(bio));
    }
    // Live account popout: the first Board app widget as a compact card under the bio.
    if let Some(w) = crate::ui::profile_board::get(app, me, cx).widgets.first() {
        let col = crate::ui::settings::profile_widgets::WidgetColors {
            strong: c(tok(idx::TEXT_STRONG)),
            muted: c(tok(idx::TEXT_MUTED)),
            card: c(tok(idx::USER_PROFILE_OVERLAY_BACKGROUND)),
            fade: match theme {
                Some(t) => {
                    let mix = |sh: u32| ((((t.0 >> sh) & 0xFF) as f32 * 0.3 + ((t.1 >> sh) & 0xFF) as f32 * 0.7) as u32) << sh;
                    c(crate::ui::profile_theme::composite(t, mix(16) | mix(8) | mix(0)))
                }
                None => c(surface),
            },
            more: false,
        };
        body = body.child(crate::ui::settings::profile_widgets::render_compact(w, &col));
    }
    let body = body.child(menus);

    // outer_P1YEjQ: 300 (+2px theme padding), radius 8; inner_P1YEjQ radius 6,
    // gap 8, padding-bottom 4. Height: 100vh - 116 at most (dump: 584 of 700).
    let max_h = window.viewport_size().height - px(116.);
    let card = div()
        .id("status-picker")
        .relative()
        .w(z(304.))
        .max_h(max_h)
        .p(z(2.))
        .rounded(z(8.))
        .overflow_hidden()
        .when(theme.is_none(), |d| d.bg(c(surface)).border_1().border_color(c(tok(idx::BORDER_SUBTLE))))
        .when_some(theme, |d, t| d.children(theme_backdrop(t, 0.)))
        .shadow_lg()
        .occlude()
        .child(
            div()
                .relative()
                .max_h(max_h - px(4.))
                .rounded(z(6.))
                .overflow_hidden()
                .when(theme.is_some(), |d| d.bg(gpui::hsla(0., 0., 0., 0.6)))
                .pb(z(4.))
                .flex()
                .flex_col()
                .gap(z(8.))
                .child(header.children(bubble))
                .child(body),
        );

    // panels_ at (10, 633) h 57; the popout sits at x 10 with its bottom 9px
    // above the panels (dump: 40..624 in a 700px window).
    div().absolute().left(px(10.)).bottom(px(76.)).child(card).into_any_element()
}

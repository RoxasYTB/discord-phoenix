use std::collections::HashSet;

use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use phoenix_api::{models::Snowflake, rest::guild_manage::AdminCall};

use crate::{
    actions::{ActionRequest, open_modal, with_modal},
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::{
        context_menu::MenuItem,
        controls::{BtnKind, button},
        modals::{Modal, card},
    },
};

pub const INACTIVE_DAYS: i64 = 180;
pub const MAX_LISTED: usize = 5000;
pub const MAX_SELECTED: usize = 1000;
const ROW_H: f32 = 48.;
const LIST_H: f32 = 480.;

pub struct Cleanup {
    pub items: Vec<(Id, i64)>,
    pub selected: HashSet<Id>,
    pub newest: bool,
    pub sort_menu: Option<gpui::Point<gpui::Pixels>>,
    pub leaving: bool,
}

pub fn snowflake_ms(id: u64) -> i64 {
    ((id >> 22) + 1_420_070_400_000) as i64
}

pub fn inactive(groups: &[(Id, i64)], now_ms: i64) -> Vec<(Id, i64)> {
    let cutoff = now_ms - INACTIVE_DAYS * 86_400_000;
    let mut v: Vec<(Id, i64)> = groups.iter().copied().filter(|(_, t)| *t < cutoff).collect();
    v.sort_by_key(|(_, t)| *t);
    v.truncate(MAX_LISTED);
    v
}

pub fn sorted(items: &[(Id, i64)], newest: bool) -> Vec<(Id, i64)> {
    let mut v = items.to_vec();
    if newest {
        v.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
    } else {
        v.sort_by_key(|(_, t)| *t);
    }
    v
}

pub fn leave_call(selected: &HashSet<Id>, order: &[(Id, i64)]) -> AdminCall {
    AdminCall::BulkLeaveGroupDms { channels: order.iter().filter(|(c, _)| selected.contains(c)).map(|(c, _)| Snowflake(c.0)).collect() }
}

fn with_state(f: impl FnOnce(&mut Cleanup)) {
    with_modal(|m| {
        if let Some(Modal::GdmCleanup(s)) = m {
            f(s);
        }
    });
}

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn check(selected: bool, disabled: bool) -> gpui::Div {
    let (bg, border) = if selected {
        (idx::CHECKBOX_BACKGROUND_SELECTED_DEFAULT, idx::CHECKBOX_BORDER_SELECTED_DEFAULT)
    } else {
        (idx::CHECKBOX_BACKGROUND_DEFAULT, idx::CHECKBOX_BORDER_DEFAULT)
    };
    div()
        .size(z(20.))
        .flex_none()
        .rounded(z(4.))
        .border_1()
        .border_color(tk(border))
        .bg(tk(bg))
        .when(disabled, |d| d.opacity(0.5))
        .flex()
        .items_center()
        .justify_center()
        .when(selected, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(z(16.)).text_color(tk(idx::CHECKBOX_ICON_ACTIVE))))
}

fn row(app: &PhoenixApp, ch: Id, selected: bool, disabled: bool, n: usize, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let name = app.channels.get(&ch).map(|c| c.name.clone()).unwrap_or_default();
    let members = app.nav.extras.get(&ch).map_or(1, |e| e.recipients + 1);
    div()
        .id(("gdm-clean", n))
        .h(z(ROW_H))
        .px(z(8.))
        .rounded(z(8.))
        .flex()
        .items_center()
        .gap(z(12.))
        .cursor_pointer()
        .when(selected, |d| d.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
        .hover(|s| s.bg(tk(idx::BACKGROUND_MOD_NORMAL)))
        .when(!disabled, |d| {
            d.on_click(cx.listener(move |_, _, _, cx| {
                with_state(|s| {
                    if !s.selected.remove(&ch) && s.selected.len() < MAX_SELECTED {
                        s.selected.insert(ch);
                    }
                });
                cx.notify();
            }))
        })
        .child(crate::ui::group_dm::group_avatar(app, ch, 32., tk(idx::BACKGROUND_SURFACE_HIGH)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tk(idx::TEXT_STRONG))
                        .child(name),
                )
                .child(div().text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_MUTED)).child(if members == 1 {
                    crate::t!("gdm.cleanup.member", n = members)
                } else {
                    crate::t!("gdm.cleanup.members", n = members)
                })),
        )
        .child(check(selected, disabled))
        .into_any_element()
}

pub fn render(app: &PhoenixApp, s: &Cleanup, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let empty = s.items.is_empty();
    let count = s.selected.len();
    let order = sorted(&s.items, s.newest);
    let selectable = s.items.len().min(MAX_SELECTED);
    let all = selectable > 0 && count >= selectable;
    let body = if empty {
        div()
            .h(z(LIST_H))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(z(4.))
            .child(
                div()
                    .text_size(z(16.))
                    .line_height(z(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(idx::TEXT_DEFAULT))
                    .child(crate::t!("gdm.cleanup.empty_title")),
            )
            .child(
                div()
                    .text_size(z(14.))
                    .line_height(z(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tk(idx::TEXT_MUTED))
                    .child(crate::t!("gdm.cleanup.empty_body")),
            )
            .into_any_element()
    } else {
        let toggle_label = if all {
            crate::t!("gdm.cleanup.deselect_all")
        } else if s.items.len() <= MAX_SELECTED {
            crate::t!("gdm.cleanup.select_all")
        } else {
            crate::t!("gdm.cleanup.select_n", n = MAX_SELECTED)
        };
        let sort = div()
            .id("gdm-clean-sort")
            .flex()
            .items_center()
            .gap(z(4.))
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
            .hover(|st| st.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
            .on_click(cx.listener(|_, ev: &gpui::ClickEvent, _, cx| {
                let p = ev.position();
                with_state(|s| s.sort_menu = Some(gpui::point(p.x, p.y + gpui::px(12.))));
                cx.notify();
            }))
            .child(crate::t!("gdm.cleanup.sort"))
            .child(gpui::svg().path(crate::icons::path("sort")).size(z(12.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)));
        let order_ids: Vec<Id> = order.iter().take(MAX_SELECTED).map(|(c, _)| *c).collect();
        let toggle = div()
            .id("gdm-clean-all")
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tk(idx::TEXT_LINK))
            .hover(|st| st.underline())
            .on_click(cx.listener(move |_, _, _, cx| {
                let ids = order_ids.clone();
                with_state(|s| {
                    if s.selected.len() >= s.items.len().min(MAX_SELECTED) {
                        s.selected.clear();
                    } else {
                        s.selected = ids.into_iter().collect();
                    }
                });
                cx.notify();
            }))
            .child(toggle_label);
        let full = count >= MAX_SELECTED;
        let mut list = div().id("gdm-clean-list").h(z(LIST_H)).overflow_y_scroll().flex().flex_col().pb(z(16.));
        for (n, (ch, _)) in order.iter().enumerate() {
            let sel = s.selected.contains(ch);
            list = list.child(row(app, *ch, sel, full && !sel, n, cx));
        }
        div().flex().flex_col().gap(z(8.)).child(div().flex().items_center().justify_between().child(sort).child(toggle)).child(list).into_any_element()
    };
    let close = crate::ui::modals::close_action(cx);
    let footer = if empty {
        div().w_full().flex().child(button("gdm-clean-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click(move |_, w, cx| close(w, cx)))
    } else {
        let label = if count == 0 {
            crate::t!("gdm.cleanup.leave")
        } else if count == 1 {
            crate::t!("gdm.cleanup.leave_one", n = count)
        } else {
            crate::t!("gdm.cleanup.leave_n", n = count)
        };
        let order2 = order.clone();
        div().w_full().flex().child(
            button("gdm-clean-leave", label, BtnKind::Critical)
                .flex_1()
                .when(count == 0 || s.leaving, |d| d.opacity(0.5).cursor_default())
                .on_click(cx.listener(move |a, _, _, cx| a.leave_inactive_groups(&order2, cx))),
        )
    };
    let mut el = div().relative().child(body);
    if let Some(pos) = s.sort_menu {
        let newest = s.newest;
        let pick = |label: SharedString, value: bool| {
            MenuItem::radio(label, newest == value, move |_, w, _: &mut App| {
                with_state(|s| {
                    s.newest = value;
                    s.sort_menu = None;
                });
                w.refresh();
            })
        };
        let items = vec![pick(crate::t!("gdm.cleanup.newest"), true), pick(crate::t!("gdm.cleanup.oldest"), false)];
        let dismiss: crate::ui::context_menu::Dismiss = std::rc::Rc::new(|w, _| {
            with_state(|s| s.sort_menu = None);
            w.refresh();
        });
        el = el.child(crate::ui::context_menu::render_anchored_at(pos, gpui::Anchor::TopLeft, items, dismiss, 40));
    }
    card(480., crate::t!("gdm.cleanup.title"), Some(crate::t!("gdm.cleanup.subtitle")), el.into_any_element(), Some(footer.into_any_element()), cx)
}

impl PhoenixApp {
    pub fn open_gdm_cleanup(&mut self, cx: &mut Context<Self>) {
        let groups: Vec<(Id, i64)> = self
            .dms
            .iter()
            .filter(|ch| self.nav.extras.get(ch).is_some_and(|e| e.owner.is_some() || e.recipients > 1))
            .map(|ch| (*ch, snowflake_ms(self.backend.channel_last_message(*ch).unwrap_or(*ch).0)))
            .collect();
        let items = inactive(&groups, chrono::Utc::now().timestamp_millis());
        let selected = items.iter().take(MAX_SELECTED).map(|(c, _)| *c).collect();
        open_modal(Modal::GdmCleanup(Cleanup { items, selected, newest: false, sort_menu: None, leaving: false }), None);
        cx.notify();
    }

    fn leave_inactive_groups(&mut self, order: &[(Id, i64)], cx: &mut Context<Self>) {
        let mut call = None;
        let mut n = 0;
        with_state(|s| {
            if s.leaving || s.selected.is_empty() {
                return;
            }
            s.leaving = true;
            n = s.selected.len();
            call = Some(leave_call(&s.selected, order));
        });
        let Some(call) = call else { return };
        self.action(ActionRequest::Admin(call), cx, move |a, r, cx| {
            match r {
                Ok(_) => {
                    crate::actions::close_modal();
                    let text = if n == 1 { crate::t!("gdm.cleanup.left_one", n = n) } else { crate::t!("gdm.cleanup.left_n", n = n) };
                    a.parity_toast(text, cx);
                }
                Err(e) => {
                    with_state(|s| s.leaving = false);
                    a.parity_toast(e.0, cx);
                }
            }
            cx.notify();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_groups_idle_for_six_months_oldest_first() {
        let now = 1_795_600_000_000;
        let day = 86_400_000;
        let g = vec![(Id(1), now - 10 * day), (Id(2), now - 200 * day), (Id(3), now - 400 * day), (Id(4), now - 181 * day)];
        let v = inactive(&g, now);
        assert_eq!(v.iter().map(|x| x.0).collect::<Vec<_>>(), vec![Id(3), Id(2), Id(4)]);
        assert_eq!(sorted(&v, true).first().map(|x| x.0), Some(Id(4)));
        assert_eq!(snowflake_ms(1_574_797_385_463_496_724), 1_795_531_355_015);
    }

    #[test]
    fn leaving_sends_the_selected_ids_in_list_order() {
        let order = vec![(Id(3), 0), (Id(2), 1), (Id(4), 2)];
        let sel: HashSet<Id> = [Id(4), Id(3)].into_iter().collect();
        let r = phoenix_api::rest::guild_manage::build(&leave_call(&sel, &order));
        assert_eq!(r.path, "/users/@me/channels/bulk-leave");
        assert_eq!(r.body.unwrap(), serde_json::json!({"channel_ids": ["3", "4"]}));
    }
}

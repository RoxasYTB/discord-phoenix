use gpui::{AnyElement, Context, FontWeight, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    actions::with_modal,
    app::PhoenixApp,
    icons::Icon,
    model::Id,
    theme::{c, generated::idx, tok, z},
    ui::{
        avatar,
        controls::{BtnKind, button},
        modals::{Modal, card},
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GiftPlan {
    NitroYearly,
    NitroMonthly,
    BasicYearly,
    BasicMonthly,
}

impl GiftPlan {
    pub const ALL: [Self; 4] = [Self::NitroYearly, Self::NitroMonthly, Self::BasicYearly, Self::BasicMonthly];

    pub fn title(self) -> SharedString {
        match self {
            Self::NitroYearly => crate::t!("gift.plan.nitro_yearly"),
            Self::NitroMonthly => crate::t!("gift.plan.nitro_monthly"),
            Self::BasicYearly => crate::t!("gift.plan.basic_yearly"),
            Self::BasicMonthly => crate::t!("gift.plan.basic_monthly"),
        }
    }

    pub fn price(self) -> SharedString {
        match self {
            Self::NitroYearly => crate::t!("gift.price.nitro_yearly"),
            Self::NitroMonthly => crate::t!("gift.price.nitro_monthly"),
            Self::BasicYearly => crate::t!("gift.price.basic_yearly"),
            Self::BasicMonthly => crate::t!("gift.price.basic_monthly"),
        }
    }

    pub fn has_discount(self) -> bool {
        matches!(self, Self::NitroYearly | Self::BasicYearly)
    }

    pub fn is_nitro(self) -> bool {
        matches!(self, Self::NitroYearly | Self::NitroMonthly)
    }
}

pub struct State {
    pub recipient: Option<Id>,
    pub plan: GiftPlan,
    pub notice: bool,
}

impl State {
    pub fn new(recipient: Option<Id>) -> Self {
        Self { recipient, plan: GiftPlan::NitroYearly, notice: false }
    }

    pub fn select(&mut self, plan: GiftPlan) {
        self.plan = plan;
        self.notice = false;
    }
}

pub fn render(app: &PhoenixApp, s: &State, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let close = crate::ui::modals::close_action(cx);
    let mut body = div().flex().flex_col().pt(z(4.));

    if let Some(uid) = s.recipient {
        let u = app.users.get(&uid);
        let name = u.map(|u| u.display_name.clone()).unwrap_or_else(|| SharedString::from(uid.0.to_string()));
        let handle = u.map(|u| u.username.clone()).unwrap_or_default();
        body = body.child(
            div()
                .mb(z(12.))
                .p(z(12.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .flex()
                .items_center()
                .gap(z(8.))
                .child(avatar(u, 24.))
                .child(div().text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(name))
                .when(!handle.is_empty(), |d| {
                    d.child(div().text_size(z(13.)).line_height(z(16.)).text_color(c(tok(idx::TEXT_MUTED))).child(handle))
                }),
        );
    }

    body = body.child(
        div()
            .mb(z(8.))
            .text_size(z(16.))
            .line_height(z(20.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))
            .child(crate::t!("gift.choose_plan")),
    );

    for (i, plan) in GiftPlan::ALL.into_iter().enumerate() {
        let selected = s.plan == plan;
        let label = plan.title();
        let price = plan.price();
        let row = div()
            .id(("gift-plan", i))
            .mb(z(12.))
            .p(z(12.))
            .rounded(z(12.))
            .cursor_pointer()
            .when(selected, |d| d.bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))).border_2().border_color(c(tok(idx::BRAND_500))))
            .when(!selected, |d| d.border_1().border_color(c(tok(idx::BORDER_NORMAL))))
            .hover(|st| st.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
            .on_click(cx.listener(move |_, _, _, cx| {
                with_modal(|m| {
                    if let Modal::GiftNitro(st) = m {
                        st.select(plan);
                    }
                });
                cx.notify();
            }))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(z(10.))
                    .child(
                        div()
                            .size(z(20.))
                            .rounded_full()
                            .border_2()
                            .border_color(if selected { c(tok(idx::BRAND_500)) } else { c(tok(idx::INTERACTIVE_TEXT_DEFAULT)) })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(selected, |d| d.child(div().size(z(10.)).rounded_full().bg(c(tok(idx::BRAND_500))))),
                    )
                    .child(crate::ui::icon(
                        if plan.is_nitro() { Icon::Nitro } else { Icon::Gift },
                        18.,
                        if selected { c(tok(idx::INTERACTIVE_TEXT_ACTIVE)) } else { c(tok(idx::INTERACTIVE_TEXT_DEFAULT)) },
                    ))
                    .child(
                        div()
                            .text_size(z(15.))
                            .line_height(z(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if selected { c(tok(idx::INTERACTIVE_TEXT_ACTIVE)) } else { c(tok(idx::INTERACTIVE_TEXT_DEFAULT)) })
                            .child(label),
                    )
                    .when(plan.has_discount(), |d| {
                        d.child(
                            div()
                                .px(z(8.))
                                .py(z(2.))
                                .rounded(z(16.))
                                .bg(c(tok(idx::GREEN_360)))
                                .text_size(z(11.))
                                .line_height(z(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(gpui::white())
                                .child(crate::t!("gift.discount")),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(z(15.))
                    .line_height(z(20.))
                    .font_weight(if selected { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                    .text_color(if selected { c(tok(idx::INTERACTIVE_TEXT_ACTIVE)) } else { c(tok(idx::INTERACTIVE_TEXT_DEFAULT)) })
                    .child(price),
            );
        body = body.child(row);
    }

    if s.notice {
        body = body.child(
            div()
                .mt(z(4.))
                .p(z(12.))
                .rounded(z(8.))
                .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .text_size(z(13.))
                .line_height(z(18.))
                .text_color(c(tok(idx::TEXT_SUBTLE)))
                .child(crate::t!("gift.no_purchase_notice")),
        );
    }

    let footer = div()
        .w_full()
        .flex()
        .gap(z(8.))
        .child(button("gift-cancel", crate::t!("common.cancel"), BtnKind::Secondary).flex_1().on_click(move |_, w, cx| close(w, cx)))
        .child(
            button("gift-next", crate::t!("gift.next"), BtnKind::Primary).flex_1().on_click(cx.listener(|_, _, _, cx| {
                with_modal(|m| {
                    if let Modal::GiftNitro(st) = m {
                        st.notice = true;
                    }
                });
                cx.notify();
            })),
        )
        .into_any_element();

    card(480., crate::t!("nitro.gift"), Some(crate::t!("gift.subtitle")), body.into_any_element(), Some(footer), cx)
}

impl PhoenixApp {
    pub fn open_gift_nitro(&mut self, recipient: Option<Id>, cx: &mut Context<Self>) {
        crate::actions::open_modal(Modal::GiftNitro(State::new(recipient)), None);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_nitro_yearly_without_notice() {
        let st = State::new(Some(Id(42)));
        assert_eq!(st.recipient, Some(Id(42)));
        assert_eq!(st.plan, GiftPlan::NitroYearly);
        assert!(!st.notice);
    }

    #[test]
    fn selecting_plan_resets_notice() {
        let mut st = State::new(None);
        st.notice = true;
        st.select(GiftPlan::BasicMonthly);
        assert_eq!(st.plan, GiftPlan::BasicMonthly);
        assert!(!st.notice);
    }

    #[test]
    fn annual_plans_carry_discount_badge() {
        assert!(GiftPlan::NitroYearly.has_discount());
        assert!(GiftPlan::BasicYearly.has_discount());
        assert!(!GiftPlan::NitroMonthly.has_discount());
        assert!(!GiftPlan::BasicMonthly.has_discount());
    }
}

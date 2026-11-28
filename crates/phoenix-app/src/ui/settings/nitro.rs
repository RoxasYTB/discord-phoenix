use gpui::{AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{self, BtnKind},
};

use super::Page;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Plan {
    Free,
    Basic,
    Classic,
    Nitro,
    Unknown,
}

impl Plan {
    fn from_tier(tier: Option<u8>) -> Self {
        match tier {
            Some(0) => Self::Free,
            Some(1) => Self::Classic,
            Some(2) => Self::Nitro,
            Some(3) => Self::Basic,
            _ => Self::Unknown,
        }
    }

    fn for_account(app: &PhoenixApp) -> Self {
        Self::from_tier(app.backend.nitro_tier())
    }

    fn name(self) -> SharedString {
        match self {
            Self::Free => crate::t!("sp.ni.plan_free"),
            Self::Basic => "Nitro Basic".into(),
            Self::Classic => "Nitro Classic".into(),
            Self::Nitro => "Nitro".into(),
            Self::Unknown => crate::t!("sp.ni.plan_unknown"),
        }
    }
}

fn text(value: SharedString, size: f32, weight: FontWeight, color: usize) -> Div {
    div().text_size(z(size)).line_height(z(size + 5.)).font_weight(weight).text_color(c(tok(color))).child(value)
}

fn status_card(app: &PhoenixApp) -> Div {
    let plan = Plan::for_account(app);
    let active = matches!(plan, Plan::Basic | Plan::Classic | Plan::Nitro);
    let (label, description) = match plan {
        Plan::Free => (crate::t!("sp.ni.status_free"), crate::t!("sp.ni.status_free_d")),
        Plan::Unknown => (crate::t!("sp.ni.status_unknown"), crate::t!("sp.ni.status_unknown_d")),
        _ => (crate::t!("sp.ni.status_active"), crate::t!("sp.ni.status_active_d")),
    };
    let accent = if active { idx::TEXT_BRAND } else { idx::ICON_MUTED };

    div()
        .w_full()
        .p(z(20.))
        .rounded(z(12.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(
            div().flex().items_center().gap(z(12.)).flex_wrap()
                .child(
                    div().size(z(42.)).flex_none().rounded(z(10.))
                        .flex().items_center().justify_center()
                        .bg(c(tok(idx::BACKGROUND_MOD_NORMAL)))
                        .child(gpui::svg().path(crate::icons::path("nitro")).size(z(22.)).text_color(c(tok(accent)))),
                )
                .child(
                    div().flex().flex_col().flex_1().min_w_0()
                        .child(text(crate::t!("sp.ni.current_plan"), 13., FontWeight::MEDIUM, idx::TEXT_MUTED))
                        .child(text(plan.name(), 22., FontWeight::SEMIBOLD, idx::TEXT_STRONG)),
                )
                .child(
                    text(label, 12., FontWeight::SEMIBOLD, if active { idx::TEXT_FEEDBACK_POSITIVE } else { idx::TEXT_DEFAULT })
                        .flex_none().px(z(9.)).py(z(4.)).rounded_full()
                        .bg(c(tok(if active { idx::BACKGROUND_FEEDBACK_MOD_POSITIVE } else { idx::BACKGROUND_MOD_NORMAL }))),
                ),
        )
        .child(text(description, 14., FontWeight::NORMAL, idx::TEXT_DEFAULT))
}

fn comparison(active: Plan) -> Div {
    let plans = [Plan::Free, Plan::Basic, Plan::Nitro];
    let feature_rows = [
        (crate::t!("sp.ni.compare_uploads"), [crate::t!("sp.ni.limit_free"), crate::t!("sp.ni.limit_basic"), crate::t!("sp.ni.limit_nitro")]),
        (crate::t!("sp.ni.p3"), availability([false, true, true])),
        (crate::t!("sp.ni.compare_stickers"), availability([false, true, true])),
        (crate::t!("sp.ni.compare_themes"), availability([false, false, true])),
        (crate::t!("sp.ni.compare_stream"), availability([false, false, true])),
        (crate::t!("sp.ni.compare_profiles"), availability([false, false, true])),
        (crate::t!("sp.ni.compare_boosts"), ["0".into(), "0".into(), "2".into()]),
    ];

    let heading = div().flex().items_center().gap(z(8.)).px(z(12.)).py(z(10.))
        .child(text(crate::t!("sp.ni.compare_perks"), 13., FontWeight::MEDIUM, idx::TEXT_MUTED).flex_1().min_w_0())
        .children(plans.into_iter().map(|plan| {
            let selected = active == plan;
            text(plan.name(), 13., FontWeight::SEMIBOLD, if selected { idx::TEXT_BRAND } else { idx::TEXT_STRONG })
                .flex_1().min_w_0().py(z(3.)).text_center().rounded(z(5.))
                .when(selected, |d| d.bg(c(tok(idx::BACKGROUND_MOD_NORMAL))))
        }));

    div().flex().flex_col().gap(z(8.))
        .child(text(crate::t!("sp.ni.compare"), 18., FontWeight::SEMIBOLD, idx::TEXT_STRONG))
        .child(
            div().rounded(z(12.)).border_1().border_color(c(tok(idx::BORDER_SUBTLE)))
                .bg(c(tok(idx::BACKGROUND_BASE_LOW)))
                .child(heading)
                .children(feature_rows.into_iter().map(|(feature, values)| {
                    div().flex().items_center().gap(z(8.)).px(z(12.)).py(z(9.))
                        .border_t_1().border_color(c(tok(idx::BORDER_SUBTLE)))
                        .child(text(feature, 13., FontWeight::MEDIUM, idx::TEXT_DEFAULT).flex_1().min_w_0())
                        .children(values.into_iter().zip(plans).map(|(value, plan)| {
                            text(value, 13., FontWeight::NORMAL, if active == plan { idx::TEXT_STRONG } else { idx::TEXT_DEFAULT })
                                .flex_1().min_w_0().text_center()
                        }))
                })),
        )
}

fn availability(included: [bool; 3]) -> [SharedString; 3] {
    included.map(|has_perk| {
        if has_perk { crate::t!("sp.ni.included") } else { crate::t!("sp.ni.not_included") }
    })
}

fn wordmark(plan: Plan, size: f32) -> Div {
    div().flex().flex_col().items_center()
        .child(crate::fonts::nitro(div().text_size(z(size)).line_height(z(size)), true).child("NITRO"))
        .when(plan == Plan::Basic, |d| {
            d.child(crate::fonts::nitro(div().text_size(z(12.)).line_height(z(12.)), true).child("BASIC"))
        })
}

fn monthly_price(plan: Plan) -> SharedString {
    use crate::ui::modals::gift_nitro::GiftPlan;
    match plan {
        Plan::Basic => GiftPlan::BasicMonthly.price(),
        _ => GiftPlan::NitroMonthly.price(),
    }
}

enum Benefit {
    Included,
    Excluded,
    Text(SharedString),
}

fn benefit_cell(id: (&'static str, usize), value: Benefit) -> Div {
    let cell = div().w(gpui::relative(0.3)).px(z(8.)).flex().items_center().justify_center().text_center();
    match value {
        Benefit::Text(value) => cell.child(text(value, 14., FontWeight::BOLD, idx::TEXT_STRONG)),
        Benefit::Included | Benefit::Excluded => {
            let included = matches!(value, Benefit::Included);
            let label = if included { crate::t!("sp.ni.included") } else { crate::t!("sp.ni.excluded") };
            cell.child(crate::ui::a11y::named(div().id(id), gpui::Role::Label, move || label.clone())
                .when(included, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(z(24.)).text_color(c(tok(idx::TEXT_STRONG)))))
                .when(!included, |d| d.child(text("—".into(), 18., FontWeight::MEDIUM, idx::TEXT_MUTED))))
        }
    }
}

fn member_comparison(active: Plan) -> Div {
    use Benefit::{Excluded, Included, Text};
    let feature_rows = [
        ("price", Text(monthly_price(Plan::Basic)), Text(monthly_price(Plan::Nitro))),
        ("emoji", Included, Included),
        ("stickers", Included, Included),
        ("app_icons", Included, Included),
        ("reactions", Included, Included),
        ("uploads", Text(crate::t!("sp.ni.limit_basic")), Text(crate::t!("sp.ni.table.limit_full"))),
        ("stream", Excluded, Text(crate::t!("sp.ni.table.stream_full"))),
        ("boosts", Excluded, Included),
        ("profile", Excluded, Included),
        ("name_styles", Excluded, Included),
        ("server_profiles", Excluded, Included),
        ("shop", Excluded, Included),
        ("themes", Excluded, Included),
        ("badge", Included, Included),
        ("video_backgrounds", Included, Included),
        ("sounds", Excluded, Included),
        ("entrance_sounds", Excluded, Included),
        ("servers", Excluded, Included),
        ("messages", Excluded, Included),
        ("friend_passes", Excluded, Included),
        ("quest_rewards", Excluded, Included),
    ];
    div()
        .flex()
        .flex_col()
        .gap(z(44.))
        .child(text(crate::t!("sp.ni.your_benefits"), 30., FontWeight::BOLD, idx::TEXT_STRONG).text_center())
        .child(
            div().relative()
                .child(
                    div().absolute().top_0().bottom_0().w(gpui::relative(0.3))
                        .right(gpui::relative(if active == Plan::Basic { 0.3 } else { 0. }))
                        .rounded(z(16.)).border_2().border_color(gpui::rgb(0xb553b7))
                        .bg(gpui::rgba(0xcf78d019)),
                )
                .child(
                    div().relative().flex().items_center().min_h(z(82.)).text_color(c(tok(idx::TEXT_STRONG)))
                        .child(text(crate::t!("sp.ni.table.heading"), 20., FontWeight::BOLD, idx::TEXT_STRONG).w(gpui::relative(0.4)).pr(z(12.)))
                        .children([Plan::Basic, Plan::Nitro].map(|plan| {
                            div().w(gpui::relative(0.3)).flex().justify_center().child(wordmark(plan, 32.))
                        })),
                )
                .children(feature_rows.into_iter().enumerate().map(|(i, (key, basic, nitro))| {
                    div().relative().flex().items_center().min_h(z(68.)).border_t_1().border_color(c(tok(idx::BORDER_SUBTLE)))
                        .child(text(crate::t!(&format!("sp.ni.table.{key}")), 14., FontWeight::MEDIUM, idx::TEXT_DEFAULT)
                            .w(gpui::relative(0.4)).py(z(12.)).pr(z(12.)))
                        .child(benefit_cell(("nitro-basic-benefit", i), basic))
                        .child(benefit_cell(("nitro-full-benefit", i), nitro))
                })),
        )
}

fn plan_actions(cx: &mut Context<PhoenixApp>) -> Div {
    div().flex().flex_wrap().gap(z(8.))
        .child(
            controls::button("nitro-manage", crate::t!("sp.ni.open_subscriptions"), BtnKind::Primary)
                .on_click(super::view::select_page(Page::Subscriptions, cx)),
        )
        .child(
            controls::button_icon("nitro-gift", "gift", crate::t!("sp.ni.gift_btn"), BtnKind::Secondary)
                .on_click(cx.listener(|app, _, _, cx| app.open_gift_nitro(None, cx))),
        )
}

fn banner_action(id: &'static str, label: SharedString, primary: bool) -> gpui::Stateful<Div> {
    let spoken = label.clone();
    crate::ui::a11y::named(div().id(id), gpui::Role::Button, move || spoken.clone())
        .w_full().min_h(z(40.)).px(z(12.)).py(z(6.))
        .flex().items_center().justify_center()
        .rounded(z(8.)).cursor_pointer()
        .bg(if primary { gpui::white().into() } else { gpui::rgba(0x251029b8) })
        .hover(move |d| d.bg(if primary { gpui::rgb(0xece8f2).into() } else { gpui::rgba(0x251029eb) }))
        .text_color(if primary { gpui::black() } else { gpui::white() })
        .text_size(z(14.)).line_height(z(18.)).font_weight(FontWeight::MEDIUM)
        .text_center().child(label)
}

fn member_banner(app: &PhoenixApp, plan: Plan, cx: &mut Context<PhoenixApp>) -> Div {
    let full = plan == Plan::Nitro;
    let title: AnyElement = if full && app.backend.premium_with_group() {
        crate::fonts::nitro(div().h(z(36.)).text_size(z(32.)).line_height(z(24.)), true)
            .child(SharedString::from(crate::t!("sp.su.squad").to_uppercase()))
            .into_any_element()
    } else {
        div().flex().child(wordmark(plan, 52.)).into_any_element()
    };
    let perks = if full {
        vec![
            ("chat/upload", crate::t!("sp.ni.p1")),
            ("chat/add-reaction", crate::t!("sp.ni.p3")),
            ("super-reaction", crate::t!("sp.ni.p2")),
            ("voice/screen", crate::t!("sp.ni.p4")),
            ("boost", crate::t!("sp.ni.two_boosts")),
            ("user-square", crate::t!("sp.ni.p5")),
        ]
    } else {
        vec![
            ("chat/upload", crate::t!("sp.ni.limit_basic")),
            ("chat/add-reaction", crate::t!("sp.ni.p3")),
            ("sticker", crate::t!("sp.ni.compare_stickers")),
        ]
    };

    div()
        .w_full()
        .relative()
        .overflow_hidden()
        .rounded(z(16.))
        .when(full, |d| {
            d.bg(gpui::rgb(0x8645bd)).child(
                gpui::img("nitro-member-gradient.svg").absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Fill),
            )
        })
        .when(!full, |d| {
            d.bg(gpui::linear_gradient(
                90.,
                gpui::linear_color_stop(gpui::rgb(0x27538d), 0.),
                gpui::linear_color_stop(gpui::rgb(0x4678a3), 1.),
            ))
        })
        .p(z(40.))
        .flex()
        .items_center()
        .gap(z(14.))
        .child(
            div().relative().flex().flex_col().flex_1().min_w_0().max_w(z(328.))
                .text_color(gpui::white())
                .child(title)
                .child(div().text_size(z(14.)).line_height(z(20.)).font_weight(FontWeight::BOLD).child(monthly_price(plan)))
                .child(
                    div().mt(z(16.)).flex().flex_col().gap(z(8.))
                        .children(perks.into_iter().map(|(icon, label)| {
                            div().flex().items_center().gap(z(8.))
                                .child(gpui::svg().path(crate::icons::path(icon)).size(z(24.)).flex_none().text_color(gpui::white()))
                                .child(div().min_w_0().text_size(z(14.)).line_height(z(20.)).child(label))
                        })),
                )
                .child(
                    div().mt(z(24.)).flex().flex_col().gap(z(8.))
                        .child(
                            banner_action("nitro-more", crate::t!("sp.ni.more"), true)
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.settings = None;
                                    app.open_home_view(crate::ui::home::data::HomeView::Nitro, cx);
                                })),
                        )
                        .child(
                            banner_action("nitro-manage", crate::t!("sp.ni.manage"), false)
                                .on_click(super::view::select_page(Page::Subscriptions, cx)),
                        ),
                ),
        )
        .when(full, |d| {
            d.child(
                div().relative().flex_1().min_w_0().mr(z(-40.))
                    .child(gpui::img(super::art::nitro_tier()).w_full().max_w(z(296.)).h(z(280.)).object_fit(gpui::ObjectFit::Contain)),
            )
        })
}

fn gift_banner(cx: &mut Context<PhoenixApp>) -> Div {
    const OFFER_START: i64 = 1_795_453_200;
    const OFFER_END: i64 = 1_796_112_000;
    const OFFER_LINK: &str = "https://support.discord.com/hc/articles/44757116266007";
    let seasonal = (OFFER_START..OFFER_END).contains(&chrono::Utc::now().timestamp());
    let details_url = if seasonal { OFFER_LINK } else { "https://support.discord.com/hc/articles/115000435108" };
    div().w_full().p(z(32.)).rounded(z(16.)).border_1().border_color(c(tok(idx::BORDER_SUBTLE)))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH))).flex().gap(z(40.))
        .child(
            div().w(gpui::relative(0.37)).max_w(z(232.)).min_w_0()
                .child(gpui::img(super::art::nitro_gift()).w_full().h(z(130.)).object_fit(gpui::ObjectFit::Contain)),
        )
        .child(
            div().flex_1().min_w_0().flex().flex_col().gap(z(8.))
                .child(text(if seasonal { crate::t!("sp.ni.offer_title") } else { crate::t!("sp.ni.gift") }, 18., FontWeight::BOLD, idx::TEXT_STRONG))
                .child(text(if seasonal { crate::t!("sp.ni.offer_description") } else { crate::t!("sp.ni.gift_d") }, 14., FontWeight::NORMAL, idx::TEXT_MUTED))
                .child(
                    div().flex().flex_wrap().gap(z(16.)).mt(z(6.))
                        .child(
                            controls::button_icon("nitro-gift", "gift", crate::t!("sp.ni.gift_btn"), BtnKind::Primary)
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    if seasonal {
                                        cx.open_url("https://discord.com/settings/premium");
                                    } else {
                                        app.open_gift_nitro(None, cx);
                                    }
                                })),
                        )
                        .child(
                            controls::button("nitro-gift-more", crate::t!("sp.learn_more"), BtnKind::Secondary)
                                .on_click(move |_, _, cx| cx.open_url(details_url)),
                        ),
                ),
        )
}

pub(super) fn page(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let plan = Plan::for_account(app);
    let page = crate::fonts::ui_root(div().w_full().flex().flex_col().gap(z(40.)));
    if matches!(plan, Plan::Nitro | Plan::Basic) {
        page.pt(z(10.)).child(gift_banner(cx)).child(member_banner(app, plan, cx)).child(member_comparison(plan))
    } else {
        page.child(div().flex().flex_col().gap(z(12.)).child(status_card(app)).child(plan_actions(cx)))
            .child(comparison(plan))
            .child(
                controls::button_sm("nitro-help", crate::t!("sp.ni.official_perks"), BtnKind::Secondary)
                    .on_click(|_, _, cx| cx.open_url("https://support.discord.com/hc/articles/115000435108")),
            )
    }
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::Plan;

    #[test]
    fn distinguishes_known_plans_from_unavailable_status() {
        assert_eq!(Plan::from_tier(None), Plan::Unknown);
        assert_eq!(Plan::from_tier(Some(0)), Plan::Free);
        assert_eq!(Plan::from_tier(Some(1)), Plan::Classic);
        assert_eq!(Plan::from_tier(Some(2)), Plan::Nitro);
        assert_eq!(Plan::from_tier(Some(3)), Plan::Basic);
        assert_eq!(Plan::from_tier(Some(255)), Plan::Unknown);
    }
}

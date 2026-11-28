use gpui::{AnyElement, Context, FontWeight, ParentElement, SharedString, Styled, Window, div, img, prelude::*};

use crate::ui::tooltip::Tipped as _;

use crate::{
    app::PhoenixApp,
    assets::Icon,
    theme::{c, generated::idx, tok, z},
    ui::{avatar, icon},
};

pub const ORB_MULTIPLIER_ART: &str = "https://cdn.discordapp.com/assets/content/6a45cf480a4894d29a155fbc23df4dca701a69e7f09227ef964a61bdb6e5833a.png";
pub const UPLOADS_ART: &str = "https://cdn.discordapp.com/assets/content/804d87fc1c1330e6413159eaacc359de89bdfbec7b4602532b1bd074868783c8.svg";
pub const NAME_STYLES_ART: &str = "https://cdn.discordapp.com/assets/content/cd580e29aa6ad4aa731dba64c23331d6bc556ad2e236ec1b5781206f6e71cb50.svg";
pub const THEMES_ART: &str = "https://cdn.discordapp.com/assets/content/c0c412ad58e2520901e2cb967308eb432d0b349b3b5f54b14f141e12b80f7d42.svg";
pub const LASTING_REWARDS_ART: &str = "https://cdn.discordapp.com/assets/content/ef081d5f86c6c1d63f7f026517c0226c59bba2618b3d65a7ce91dc10019b35ad.svg";
pub const APP_ICONS_ART: &str = "https://cdn.discordapp.com/assets/content/7bb191dd09571f8910a367c7cf35b500ea7b64dde29309c7c74831cc16b1dc1d.png";
pub const CARD_W: f32 = 330.;
pub const CARD_H: f32 = 380.;
pub const MAX_CARDS: usize = 6;
const SCROLL_GUTTER: f32 = 16.;
const HERO_GRADIENT_H: f32 = 984.;

thread_local! {
    static PAGE_W: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
    static MY_PREMIUM_SINCE: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
    static PREMIUM_SINCE_ASKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

const TENURE: [(u32, &str, &str); 8] = [
    (1, "nitro.tier_bronze", "https://cdn.discordapp.com/assets/content/d8df65e0c94fd9d1f5028b4b252710fede2f4059d419d32b6f70f253f0c27999.png"),
    (3, "nitro.tier_silver", "https://cdn.discordapp.com/assets/content/d0e033cda164abbfc02c39e70afcccb410954ceb63544178608eff0323092ccd.png"),
    (6, "nitro.tier_gold", "https://cdn.discordapp.com/assets/content/a67846bf44a97eeaf00209a31f30146f2c3685abb207ec89f1aa97404efb7883.png"),
    (12, "nitro.tier_platinum", "https://cdn.discordapp.com/assets/content/10b3c17fdd7d4f454e9c2335a94e48f05a1ed9c23e5f48101e5b9895f8055bb7.png"),
    (24, "nitro.tier_diamond", "https://cdn.discordapp.com/assets/content/1468c2db0e265bed25bdcae7f6fe586a66493cd7c8cf0ca9228dc8b899e22d1b.png"),
    (36, "nitro.tier_emerald", "https://cdn.discordapp.com/assets/content/5d4f18789f059f49d6bb2519692c96e36479ddac46af90030e6bb0473199f0ed.png"),
    (60, "nitro.tier_ruby", "https://cdn.discordapp.com/assets/content/ecfc2da5e1ee0563dc487d35e1aad74f083dd2f18e5798028ffbc96f255d6840.png"),
    (72, "nitro.tier_opal", "https://cdn.discordapp.com/assets/content/7fbfa17e5011425252d9ff6c1f286f41560c8acc58cb543b3c895031ffda5289.png"),
];

fn months_since(since: chrono::DateTime<chrono::Utc>, now: chrono::DateTime<chrono::Utc>) -> u32 {
    let mut m = 0;
    while since.checked_add_months(chrono::Months::new(m + 1)).is_some_and(|d| d <= now) {
        m += 1;
    }
    m
}

pub(crate) fn tenure(since: i64, now: i64) -> Option<(usize, Option<(usize, i64)>)> {
    let (since, now) = (chrono::DateTime::from_timestamp(since, 0)?, chrono::DateTime::from_timestamp(now, 0)?);
    let months = months_since(since, now);
    let tier = TENURE.iter().rposition(|t| t.0 <= months)?;
    let next = TENURE.get(tier + 1).and_then(|n| {
        let at = since.checked_add_months(chrono::Months::new(n.0))?;
        Some((tier + 1, ((at - now).num_seconds() + 86_399) / 86_400))
    });
    Some((tier, next))
}

fn tenure_perk(since: i64, now: i64) -> Option<(Perk, SharedString)> {
    let (tier, next) = tenure(since, now)?;
    let (_, name, art) = TENURE[tier];
    let description = next.map_or_else(SharedString::default, |(n, days)| {
        let next_name = crate::t!(TENURE[n].1);
        if days <= 30 {
            crate::t!("nitro.tenure_next_days", n = days, next = next_name)
        } else {
            crate::t!("nitro.tenure_next_months", n = (days as f64 / 30.4375).ceil() as i64, next = next_name)
        }
    });
    let date = chrono::DateTime::from_timestamp(since, 0)?.with_timezone(&chrono::Local);
    let date = if crate::i18n::locale().starts_with("fr") { date.format("%d/%m/%y") } else { date.format("%-m/%-d/%y") }.to_string();
    Some((Perk { id: "tenure-badge", title: crate::t!(name), description, art, art_max_w: Some(276.), cta: None }, crate::t!("nitro.tenure_since", date = date)))
}

fn ensure_premium_since(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if !app.backend.has_premium() || PREMIUM_SINCE_ASKED.with(|a| a.replace(true)) {
        return;
    }
    let fut = app.backend.user_profile(app.me.id, None);
    cx.spawn(async move |this, cx| {
        let Ok(p) = fut.await else { return };
        MY_PREMIUM_SINCE.with(|s| s.set(p.premium_since));
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

pub(crate) fn grid_columns(page_w: f32) -> (usize, Option<f32>) {
    if page_w <= 0. {
        return (3, None);
    }
    let cols = if page_w >= 960. { 3 } else if page_w >= 720. { 2 } else { 1 };
    (cols, Some((page_w - 64.).min(1200.)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cta {
    Quests,
    Profiles,
    Themes,
    AppIcons,
    VoiceVideo,
    Shop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Perk {
    pub id: &'static str,
    pub title: SharedString,
    pub description: SharedString,
    pub art: &'static str,
    pub art_max_w: Option<f32>,
    pub cta: Option<Cta>,
}

pub fn perks(multiplier: Option<f64>) -> Vec<Perk> {
    let mut out = Vec::new();
    if let Some(m) = multiplier {
        let m = format!("{m:.1}");
        let m = if crate::i18n::locale().starts_with("fr") { m.replace('.', ",") } else { m };
        out.push(Perk {
            id: "quest-orb-multiplier",
            title: crate::t!("nitro.c_orbs"),
            description: crate::t!("nitro.c_orbs_d", n = m),
            art: ORB_MULTIPLIER_ART,
            art_max_w: Some(280.),
            cta: Some(Cta::Quests),
        });
    }
    out.push(Perk { id: "file-upload", title: crate::t!("nitro.p_upload"), description: crate::t!("nitro.p_upload_d"), art: UPLOADS_ART, art_max_w: None, cta: None });
    out.push(Perk { id: "display-name-styles", title: crate::t!("nitro.p_name_styles"), description: crate::t!("nitro.p_name_styles_d"), art: NAME_STYLES_ART, art_max_w: None, cta: Some(Cta::Profiles) });
    out.push(Perk { id: "client-themes", title: crate::t!("nitro.p_themes"), description: crate::t!("nitro.c_themes_d"), art: THEMES_ART, art_max_w: None, cta: Some(Cta::Themes) });
    out.push(Perk { id: "permadecos", title: crate::t!("nitro.c_lasting"), description: crate::t!("nitro.c_lasting_d"), art: LASTING_REWARDS_ART, art_max_w: None, cta: Some(Cta::Profiles) });
    out.push(Perk { id: "custom-app-icons", title: crate::t!("nitro.c_app_icons"), description: crate::t!("nitro.c_app_icons_d"), art: APP_ICONS_ART, art_max_w: None, cta: Some(Cta::AppIcons) });
    out.truncate(MAX_CARDS);
    out
}

fn run(a: &mut PhoenixApp, cta: Cta, cx: &mut Context<PhoenixApp>) {
    let page = match cta {
        Cta::Quests => return a.open_home_view(crate::ui::home::data::HomeView::Quests, cx),
        Cta::Shop => return a.open_home_view(crate::ui::home::data::HomeView::Shop, cx),
        Cta::VoiceVideo => crate::ui::settings::Page::VoiceVideo,
        Cta::Profiles => crate::ui::settings::Page::Profiles,
        Cta::Themes | Cta::AppIcons => crate::ui::settings::Page::Appearance,
    };
    crate::ui::settings::open(a, cx);
    if let Some(s) = a.settings.as_mut() {
        s.page = page;
    }
    cx.notify();
}

fn section_title(t: SharedString) -> gpui::Div {
    crate::fonts::nitro(div(), true)
        .mb(z(24.))
        .text_size(z(24.))
        .line_height(z(26.))
        .text_color(c(tok(idx::TEXT_STRONG)))
        .child(SharedString::from(t.to_uppercase()))
}

fn card(ix: usize, p: &Perk, pill: Option<SharedString>, caption: Option<SharedString>, w: f32, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let group: SharedString = format!("nitro-card-{ix}").into();
    let cta = p.cta;
    div()
        .id(("nitro-card", ix))
        .group(group.clone())
        .w(z(w))
        .h(z(CARD_H))
        .flex_none()
        .rounded(z(16.))
        .bg(c(tok(idx::BACKGROUND_SURFACE_HIGH)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .overflow_hidden()
        .relative()
        .p(z(20.))
        .flex()
        .flex_col()
        .children(pill.map(|t| {
            div()
                .absolute()
                .top(z(24.))
                .left(z(24.))
                .px(z(8.))
                .py(z(2.))
                .rounded_full()
                .bg(gpui::white())
                .text_size(z(12.))
                .line_height(z(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(gpui::rgb(0x2f3035))
                .child(SharedString::from(t.to_uppercase()))
        }))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(img(p.art).max_h(z(180.)).max_w(z(p.art_max_w.unwrap_or(290.))).rounded(z(16.)).object_fit(gpui::ObjectFit::Contain)),
        )
        .child(
            div()
                .flex_none()
                .mx(z(-20.))
                .mb(z(-20.))
                .p(z(20.))
                .flex()
                .flex_col()
                .children(caption.map(|t| div().mb(z(4.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_MUTED))).child(t)))
                .child(div().text_size(z(20.)).line_height(z(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(tok(idx::TEXT_STRONG))).child(p.title.clone()))
                .child(
                    div()
                        .mt(z(4.))
                        .max_h(z(18.))
                        .overflow_hidden()
                        .group_hover(group.clone(), |s| s.max_h(z(90.)))
                        .text_size(z(14.))
                        .line_height(z(18.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(p.description.clone()),
                )
                .when(cta.is_some(), |d| d.child(div().h(z(0.)).group_hover(group.clone(), |s| s.h(z(36.)).mt(z(16.))))),
        )
        .children(cta.map(|cta| {
            div()
                .absolute()
                .left(z(20.))
                .right(z(20.))
                .bottom(z(-60.))
                .opacity(0.)
                .group_hover(group, |s| s.bottom(z(20.)).opacity(1.))
                .child(
                    div()
                        .id(("nitro-card-cta", ix))
                        .w_full()
                        .h(z(40.))
                        .rounded(z(8.))
                        .border_1()
                        .border_color(c(tok(idx::CONTROL_OVERLAY_PRIMARY_BORDER_DEFAULT)))
                        .bg(c(tok(idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_DEFAULT)))
                        .hover(|s| s.bg(c(tok(idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_HOVER))).border_color(c(tok(idx::CONTROL_OVERLAY_PRIMARY_BORDER_HOVER))))
                        .active(|s| s.bg(c(tok(idx::CONTROL_OVERLAY_PRIMARY_BACKGROUND_ACTIVE))).border_color(c(tok(idx::CONTROL_OVERLAY_PRIMARY_BORDER_ACTIVE))))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(tok(idx::CONTROL_OVERLAY_PRIMARY_TEXT_DEFAULT)))
                        .on_click(cx.listener(move |a, _, _, cx| run(a, cta, cx)))
                        .child(crate::t!("nitro.take_me_there")),
                )
        }))
        .into_any_element()
}

fn compare_plans(cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let perks = [
        ("nitro.compare_emoji", true, None),
        ("nitro.compare_uploads", true, None),
        ("nitro.compare_reactions", true, None),
        ("nitro.compare_badges", true, None),
        ("nitro.compare_video_backgrounds", true, None),
        ("nitro.c_app_icons", true, Some(Cta::AppIcons)),
        ("nitro.p_profile", false, Some(Cta::Profiles)),
        ("nitro.p_name_styles", false, Some(Cta::Profiles)),
        ("nitro.p_themes", false, Some(Cta::Themes)),
        ("nitro.p_stream", false, Some(Cta::VoiceVideo)),
        ("nitro.compare_soundboard", false, Some(Cta::VoiceVideo)),
        ("nitro.compare_messages", false, None),
        ("nitro.compare_servers", false, None),
        ("nitro.compare_boosts", false, None),
        ("nitro.compare_shop", false, Some(Cta::Shop)),
        ("nitro.compare_quests", false, Some(Cta::Quests)),
    ];
    let col = || div().w(z(108.)).flex_none().text_center();
    let included = |available| {
        if available { crate::t!("nitro.compare_included") } else { SharedString::from("—") }
    };
    let headers = div().flex().items_center().gap(z(8.)).px(z(12.)).py(z(10.)).font_weight(FontWeight::SEMIBOLD)
        .child(div().flex_1().min_w_0().child(crate::t!("nitro.compare_perk")))
        .child(col().child("Nitro Basic"))
        .child(col().child("Nitro"));
    div().flex().flex_col()
        .child(headers)
        .children(perks.into_iter().map(|(key, basic, action)| {
            div().id(key).flex().items_center().gap(z(8.)).px(z(12.)).py(z(8.))
                .border_t_1().border_color(c(tok(idx::BORDER_SUBTLE)))
                .child(div().flex_1().min_w_0().child(crate::t!(key)))
                .child(col().text_sm().child(included(basic)))
                .child(col().text_sm().child(included(true)))
                .when_some(action, |d, action| {
                    d.cursor_pointer()
                        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
                        .on_click(cx.listener(move |app, _, _, cx| run(app, action, cx)))
                })
        }))
        .child(div().id("nitro-official-benefits").mt(z(10.)).px(z(12.)).py(z(6.)).text_color(c(tok(idx::TEXT_LINK)))
            .cursor_pointer().child(crate::t!("nitro.compare_details"))
            .on_click(|_, _, cx| cx.open_url("https://support.discord.com/hc/en-us/articles/115000435108")))
}

pub fn render(app: &mut PhoenixApp, _window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let me = app.me.clone();
    let overlay_btn = |id: &'static str| {
        div()
            .id(id)
            .h(z(32.))
            .px(z(11.))
            .flex()
            .items_center()
            .gap(z(4.))
            .rounded(z(8.))
            .bg(gpui::hsla(0., 0., 0., 0.56))
            .border_1()
            .border_color(gpui::hsla(240. / 360., 0.03, 0.61, 0.16))
            .cursor_pointer()
            .text_size(z(14.))
            .line_height(z(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(gpui::white())
    };
    let nav = div()
        .h(z(49.))
        .flex_none()
        .px(z(16.))
        .flex()
        .items_center()
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(6.))
                .text_color(gpui::white())
                .child(icon(Icon::Nitro, 24., gpui::white()))
                .child(crate::fonts::nitro(div().text_size(z(18.)), true).child("NITRO")),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .gap(z(8.))
                .child(
                    overlay_btn("nitro-heart")
                        .w(z(32.))
                        .px(z(0.))
                        .justify_center()
                        .on_click(cx.listener(|a, _, _, cx| {
                            a.blocked_write(crate::actions::guard::GuardedWrite::wishlist_add_sku(crate::ui::extra_menus::NITRO_SKU), cx)
                        }))
                        .child(gpui::svg().path(crate::icons::path("heart")).size(z(16.)).text_color(gpui::white()))
                        .tip_rich(crate::t!("nitro.wishlist_tip"), crate::t!("nitro.wishlist_tip_body"), crate::ui::tooltip::Side::Bottom),
                )
                .child(overlay_btn("nitro-gift").child(icon(Icon::Gift, 16., gpui::white())).child(crate::t!("nitro.gift")).on_click(cx.listener(|a, _, _, cx| a.open_gift_nitro(None, cx)))),
        );
    let hello = div()
        .flex()
        .flex_col()
        .gap(z(12.))
        .child(crate::fonts::nitro(div().text_size(z(32.)).line_height(z(34.)), true).text_color(gpui::white()).child(SharedString::from(greeting(chrono::Timelike::hour(&chrono::Local::now())).to_uppercase())))
        .child(
            div()
                .flex()
                .items_center()
                .gap(z(16.))
                .child(avatar(Some(&me), 64.))
                .child(crate::fonts::styled_name(div().text_size(z(40.)).line_height(z(58.)).font_weight(FontWeight::BOLD).text_color(gpui::white()), Some(&me), false).child(me.display_name.clone())),
        );
    crate::ui::quests_page::ensure_loaded(app, cx);
    let (cols, content_w) = grid_columns(PAGE_W.with(std::cell::Cell::get));
    let card_w = content_w.map_or(CARD_W, |w| (w - 24. * (cols - 1) as f32) / cols as f32);
    let mut cards: Vec<AnyElement> = Vec::new();
    ensure_premium_since(app, cx);
    if let Some((perk, caption)) = MY_PREMIUM_SINCE.with(std::cell::Cell::get).and_then(|since| tenure_perk(since, chrono::Utc::now().timestamp())) {
        cards.push(card(usize::MAX, &perk, Some(crate::t!("nitro.tenure_pill")), Some(caption), card_w, cx));
    }
    for (i, p) in perks(crate::ui::quests_page::premium_multiplier()).iter().enumerate() {
        cards.push(card(i, p, None, None, card_w, cx));
    }
    let grid = div().flex().flex_wrap().gap_x(z(24.)).gap_y(z(24.)).children(cards);
    div()
        .id("nitro-home")
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .bg(gpui::black())
        .child(
            div()
                .id("nitro-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&crate::ui::chat::scrollbar::handle("nitro-scroll"))
                .flex()
                .flex_col()
                .relative()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h(z(HERO_GRADIENT_H))
                        .bg(gpui::linear_gradient(
                            160.,
                            gpui::linear_color_stop(c(tok(idx::BACKGROUND_BRAND)).opacity(0.55), 0.),
                            gpui::linear_color_stop(gpui::black(), 1.),
                        )),
                )
                .child(
                    gpui::canvas(|b, _, _| PAGE_W.with(|w| w.set(f32::from(b.size.width) / crate::theme::zoom() - SCROLL_GUTTER)), |_, _, _, _| {})
                        .w_full()
                        .h(gpui::px(0.)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(nav)
                        .child(div().relative().px(z(32.)).pt(z(80.)).pb(z(64.)).child(hello).child(hero_art(cx))),
                )
                .child(
                    div()
                        .px(z(32.))
                        .pb(z(32.))
                        .flex()
                        .flex_col()
                        .child(section_title(crate::t!("nitro.whats_new")))
                        .child(grid),
                )
                .child(div().px(z(32.)).pb(z(32.)).child(section_title(crate::t!("nitro.compare_title"))).child(compare_plans(cx))),
        )
        .children(crate::ui::chat::scrollbar::bar_for("nitro-scroll", crate::ui::chat::scrollbar::Bar::Auto))
        .into_any_element()
}

const ART_W: f32 = 340.31;
const WUMPUS: &str = "https://cdn.discordapp.com/assets/content/a3e8e17987398023e2afd61ec5078a9bce18b2832f2f1775a1ba3c033ce13270.webm";
const TRINKETS: [(&str, f32, f32, f32, f32); 4] = [
    ("https://cdn.discordapp.com/assets/content/0b1a59149e615fc048010a3c7f109f8695c8b2004712e99417fcb0dec43fcb44.png", 90., -10., 43.05, 46.72),
    ("https://cdn.discordapp.com/assets/content/b39a5f2755e6da320fce10c8a4a10bdebef9926b671256b1681976198c5656d4.png", 291.59, 25., 23.96, 24.68),
    ("https://cdn.discordapp.com/assets/content/7c23a220a6f31150648930e2ebb435aa7cc89ad57895275bed6f1900869f4de0.png", 58.65, 113.57, 41.44, 38.21),
    ("https://cdn.discordapp.com/assets/content/ae5638c61a572593c6b03b92e80d3846e0cfe7a9e893f3faf05aecd670a4017d.png", 311.81, 158.83, 29.31, 41.22),
];

fn hero_art(cx: &mut Context<PhoenixApp>) -> AnyElement {
    let plays = !crate::ui::settings::effects::reduced_motion();
    let mut art = div().absolute().top(z(80.)).right(z(32.)).w(z(ART_W)).h(z(0.)).child(
        div()
            .absolute()
            .left(z(89.7))
            .top(z(-0.17))
            .size(z(174.56))
            .child(crate::ui::chat::video_player::gifv("nitro-home-wumpus".into(), WUMPUS.into(), SharedString::default(), plays, cx)),
    );
    for (url, x, y, w, h) in TRINKETS {
        art = art.child(img(url).absolute().left(z(x)).top(z(y)).w(z(w)).h(z(h)).object_fit(gpui::ObjectFit::Contain));
    }
    art.into_any_element()
}

pub(crate) fn greeting(hour: u32) -> SharedString {
    match hour {
        5..12 => crate::t!("nitro.hello_morning"),
        12..17 => crate::t!("nitro.hello_afternoon"),
        _ => crate::t!("nitro.hello_evening"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whats_new_follows_the_client_card_order() {
        let ids: Vec<&str> = perks(Some(1.2)).iter().map(|p| p.id).collect();
        assert_eq!(ids, ["quest-orb-multiplier", "file-upload", "display-name-styles", "client-themes", "permadecos", "custom-app-icons"]);
        assert_eq!(perks(None).len(), 5);
        assert!(perks(Some(1.2))[0].description.contains("1"));
        assert_eq!(perks(None)[0].cta, None);
    }

    #[test]
    fn tenure_badge_follows_the_client_tiers() {
        let since = chrono::DateTime::parse_from_rfc3339("2026-08-16T22:21:22Z").unwrap().timestamp();
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-28T12:00:00Z").unwrap().timestamp();
        let (tier, next) = tenure(since, now).unwrap();
        assert_eq!(tier, 1);
        let (n, days) = next.unwrap();
        assert_eq!(n, 2);
        assert_eq!((days as f64 / 30.4375).ceil() as i64, 3);
        assert_eq!(tenure(since, since + 86_400), None);
    }

    #[test]
    fn whats_new_grid_follows_the_container_queries() {
        assert_eq!(grid_columns(881.67).0, 2);
        assert_eq!(grid_columns(960.).0, 3);
        assert_eq!(grid_columns(700.).0, 1);
        let (cols, w) = grid_columns(881.67);
        let card = (w.unwrap() - 24. * (cols - 1) as f32) / cols as f32;
        assert!((card - 396.835).abs() < 0.01);
        assert_eq!(grid_columns(1600.).1, Some(1200.));
    }
}

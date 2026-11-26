use std::cell::Cell;

use gpui::{AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div, img, prelude::*, rgb, rgba};

use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{c, generated::idx, tok, z},
};

pub const HEADER_IMAGE: &str = "https://cdn.discordapp.com/assets/content/f91111a24ca4c59e87a462e8a3523938628e03e3723c31e5681991a07b0acf48.png";
pub const CONTENT_MAX_W: f32 = 1224.;
pub const TIER_BOOSTS: [u32; 4] = [0, 2, 7, 14];
pub const STACK_BELOW: f32 = 1100.;
pub const VIDEO_THUMB: &str = "https://cdn.discordapp.com/assets/content/7b8d6c9c9f8bc8092089a2f55081695d40f7f303f053fcc6f944bf4b38403573.png";
pub const TIER_ICONS: [&str; 3] = [
    "https://cdn.discordapp.com/assets/content/33e98755a49cf85d07b8189a0001926c17d91599d53662a39999329f5253254f.svg",
    "https://cdn.discordapp.com/assets/content/3bc18204c2bf43975ded85a602c7816d032bc07cf9b1d5589e52404f9bc1d687.svg",
    "https://cdn.discordapp.com/assets/content/cb869ee40a8ed5d6210e69a02cebbf03025be47006b310ec2564f74f6ad1dff8.svg",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell4 {
    Count(u32),
    Kbps(u32),
    Mb(u32),
    Res(u32, u32),
    Static,
    Animated,
    Bool(bool),
}

pub struct PerkRow {
    pub label: &'static str,
    pub values: [Cell4; 4],
}

pub const PERK_ROWS: [PerkRow; 12] = [
    PerkRow { label: "boost.t.emoji", values: [Cell4::Count(50), Cell4::Count(100), Cell4::Count(150), Cell4::Count(250)] },
    PerkRow { label: "boost.t.stickers", values: [Cell4::Count(5), Cell4::Count(15), Cell4::Count(30), Cell4::Count(60)] },
    PerkRow { label: "boost.t.soundboard", values: [Cell4::Count(8), Cell4::Count(24), Cell4::Count(36), Cell4::Count(48)] },
    PerkRow { label: "boost.t.stream", values: [Cell4::Res(720, 30), Cell4::Res(720, 60), Cell4::Res(1440, 60), Cell4::Res(1440, 60)] },
    PerkRow { label: "boost.t.audio", values: [Cell4::Kbps(96), Cell4::Kbps(128), Cell4::Kbps(256), Cell4::Kbps(384)] },
    PerkRow { label: "boost.t.upload", values: [Cell4::Mb(10), Cell4::Mb(10), Cell4::Mb(50), Cell4::Mb(100)] },
    PerkRow { label: "boost.t.stage", values: [Cell4::Count(50), Cell4::Count(50), Cell4::Count(150), Cell4::Count(300)] },
    PerkRow { label: "boost.t.animated_icon", values: [Cell4::Bool(false), Cell4::Bool(true), Cell4::Bool(true), Cell4::Bool(true)] },
    PerkRow { label: "boost.t.invite_bg", values: [Cell4::Bool(false), Cell4::Bool(true), Cell4::Bool(true), Cell4::Bool(true)] },
    PerkRow { label: "boost.t.banner", values: [Cell4::Bool(false), Cell4::Bool(false), Cell4::Static, Cell4::Animated] },
    PerkRow { label: "boost.t.role_icons", values: [Cell4::Bool(false), Cell4::Bool(false), Cell4::Bool(true), Cell4::Bool(true)] },
    PerkRow { label: "boost.t.vanity", values: [Cell4::Bool(false), Cell4::Bool(false), Cell4::Bool(false), Cell4::Bool(true)] },
];

pub const FAQ: [(&str, &str); 9] = [
    ("boost.faq.q1", "boost.faq.a1"),
    ("boost.faq.q2", "boost.faq.a2"),
    ("boost.faq.q3", "boost.faq.a3"),
    ("boost.faq.q4", "boost.faq.a4"),
    ("boost.faq.q5", "boost.faq.a5"),
    ("boost.faq.q6", "boost.faq.a6"),
    ("boost.faq.q7", "boost.faq.a7"),
    ("boost.faq.q8", "boost.faq.a8"),
    ("boost.faq.q9", "boost.faq.a9"),
];

pub const PROFILE_BADGE_ART: &str = "https://discord.com/assets/185ece790f20d870.svg";

pub fn recommended_tier(current: u8) -> u8 {
    if current <= 1 { 2 } else { 3 }
}

pub fn cell_text(v: Cell4) -> Option<SharedString> {
    Some(match v {
        Cell4::Bool(_) => return None,
        Cell4::Count(n) => n.to_string().into(),
        Cell4::Kbps(n) => crate::t!("boost.t.kbps", bitrate = n),
        Cell4::Mb(n) => crate::t!("boost.t.mb", size = n),
        Cell4::Res(r, f) => crate::t!("boost.t.res", resolution = format!("{r}p"), framerate = f),
        Cell4::Static => crate::t!("boost.t.static"),
        Cell4::Animated => crate::t!("boost.t.animated"),
    })
}

thread_local! {
    static FAQ_OPEN: Cell<Option<usize>> = const { Cell::new(None) };
    static OPEN: Cell<Option<Id>> = const { Cell::new(None) };
    static SCROLL: gpui::ScrollHandle = gpui::ScrollHandle::new();
}

pub fn open(guild: Id) {
    OPEN.with(|o| o.set(Some(guild)));
    SCROLL.with(|s| s.set_offset(gpui::point(gpui::px(0.), gpui::px(0.))));
}

pub fn close() {
    OPEN.with(|o| o.set(None));
}

pub fn is_open() -> bool {
    OPEN.with(|o| o.get()).is_some()
}

fn tk(i: usize) -> Hsla {
    c(tok(i))
}

fn text(size: f32, line: f32, weight: FontWeight, color: Hsla, s: impl Into<SharedString>) -> gpui::Div {
    div().text_size(z(size)).line_height(z(line)).font_weight(weight).text_color(color).child(s.into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perk {
    pub icon: &'static str,
    pub key: &'static str,
}

pub const TIERS: [(u8, &[Perk]); 3] = [
    (
        1,
        &[
            Perk { icon: "chat/add-reaction", key: "boost.perk.emoji" },
            Perk { icon: "voice/soundboard", key: "boost.perk.soundboard" },
            Perk { icon: "gif", key: "boost.perk.animated_icon" },
            Perk { icon: "headphones", key: "boost.perk.audio" },
        ],
    ),
    (
        2,
        &[
            Perk { icon: "voice/screen", key: "boost.perk.hd_stream" },
            Perk { icon: "chat/upload", key: "boost.perk.upload_50" },
            Perk { icon: "settings/shield-user", key: "boost.perk.role_icons" },
            Perk { icon: "images", key: "boost.perk.banner" },
        ],
    ),
    (
        3,
        &[
            Perk { icon: "star", key: "boost.perk.vanity" },
            Perk { icon: "chat/upload", key: "boost.perk.upload_100" },
            Perk { icon: "headphones", key: "boost.perk.audio_max" },
            Perk { icon: "gif", key: "boost.perk.animated_banner" },
            Perk { icon: "stage", key: "boost.perk.stage" },
        ],
    ),
];

pub fn boosts_label(n: u32) -> SharedString {
    match n {
        0 => crate::t!("boost.none"),
        1 => crate::t!("boost.one", n = n),
        _ => crate::t!("boost.many", n = n),
    }
}

pub fn perk_text(key: &str) -> SharedString {
    match key {
        "boost.perk.emoji" => crate::t!(key, n = 100),
        "boost.perk.soundboard" => crate::t!(key, n = 24),
        "boost.perk.stage" => crate::t!(key, n = 300),
        _ => crate::t!(key),
    }
}

fn gem(size: f32, color: Hsla) -> gpui::Svg {
    gpui::svg().path(crate::icons::path("boost")).size(z(size)).text_color(color)
}

fn tier_card(tier: u8, perks: &[Perk], active: bool) -> AnyElement {
    let pink = tk(idx::GUILD_BOOSTING_PINK_REFRESH);
    let muted: Hsla = rgb(0x94959c).into();
    let bar = {
        let mut b = div().absolute().top(z(40.)).h(z(8.)).left_0().right_0();
        if tier == 1 {
            b = b.left(z(if active { 64. } else { 40. })).rounded_l(z(100.));
        }
        if tier == 3 {
            b = b.right(z(24.)).rounded_r(z(100.));
        }
        if active {
            b.bg(gpui::linear_gradient(90., gpui::linear_color_stop(rgb(0xff4ebb), 0.), gpui::linear_color_stop(rgba(0xff4ebb66), 1.)))
        } else {
            b.bg(rgb(0x474464))
        }
    };
    let border = match (active, tier) {
        (true, _) | (false, 1) => rgb(0x202024),
        (false, 2) => rgb(0x211e40),
        _ => rgb(0x201d3d),
    };
    let gem_box = div()
        .absolute()
        .left(z(24.))
        .top(z(24.))
        .size(z(40.))
        .rounded_full()
        .bg(border)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(z(30.))
                .rounded_full()
                .bg(if active { pink } else { rgb(0x474464).into() })
                .flex()
                .items_center()
                .justify_center()
                .child(img(TIER_ICONS[(tier as usize).saturating_sub(1).min(2)]).size(z(24.))),
        );
    let title = text(24., 30., FontWeight::SEMIBOLD, if active { tk(idx::TEXT_STRONG) } else { muted }, crate::t!(match tier {
        1 => "boost.level1",
        2 => "boost.level2",
        _ => "boost.level3",
    }))
    .flex_none();
    let badge = div()
        .h(z(30.))
        .px(z(8.))
        .rounded(z(8.))
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .flex()
        .items_center()
        .gap(z(4.))
        .text_color(rgb(0xaaaab1))
        .child(gem(12., rgb(0xaaaab1).into()))
        .child(text(16., 20., FontWeight::MEDIUM, rgb(0xaaaab1).into(), boosts_label(TIER_BOOSTS[tier as usize])));
    let mut list = div().flex().flex_col().gap(z(12.));
    for p in perks.iter().take(4) {
        let col = if active { tk(idx::TEXT_DEFAULT) } else { muted };
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(z(8.))
                .child(gpui::svg().path(crate::icons::path(p.icon)).flex_none().size(z(20.)).text_color(col))
                .child(text(16., 20., FontWeight::MEDIUM, col, perk_text(p.key))),
        );
    }
    list = list.child(text(16., 20., FontWeight::MEDIUM, muted, crate::t!("boost.and_more")));
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(z(16.))
        .border_1()
        .border_color(if active { gpui::hsla(240. / 360., 0.04, 0.61, 0.04) } else { tk(idx::BORDER_SUBTLE) })
        .bg(tk(idx::BACKGROUND_MOD_SUBTLE))
        .shadow_sm()
        .child(div().relative().h(z(64.)).child(bar).child(gem_box))
        .child(div().p(z(24.)).flex().flex_col().gap(z(12.)).child(div().flex().items_center().justify_between().child(title).child(badge)).child(list))
        .into_any_element()
}

fn section_heading(t: SharedString) -> gpui::Div {
    text(32., 40., FontWeight::SEMIBOLD, tk(idx::TEXT_STRONG), t).text_center()
}

fn value_cell(v: Cell4, boosted: bool) -> AnyElement {
    match v {
        Cell4::Bool(true) => gpui::svg().path(crate::icons::path("check")).size(z(24.)).text_color(tk(idx::TEXT_STRONG)).into_any_element(),
        Cell4::Bool(false) => gpui::svg().path(crate::icons::path("close-small")).size(z(24.)).text_color(tk(idx::INTERACTIVE_MUTED)).into_any_element(),
        _ => text(16., 20., if boosted { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM }, if boosted { tk(idx::TEXT_STRONG) } else { tk(idx::TEXT_MUTED) }, cell_text(v).unwrap_or_default()).into_any_element(),
    }
}

fn perks_table(current: u8) -> AnyElement {
    let cell = || div().flex_1().min_w_0().px(z(16.));
    let tier_cell = || cell().flex().flex_col().items_center().justify_center().text_center();
    let mut header = div().w_full().flex().items_start().py(z(16.)).border_b_1().border_color(tk(idx::BORDER_NORMAL)).child(cell().child(text(20., 24., FontWeight::SEMIBOLD, tk(idx::TEXT_DEFAULT), crate::t!("boost.t.perks"))));
    for (n, key) in ["boost.t.unboosted", "boost.level1", "boost.level2", "boost.level3"].iter().enumerate() {
        header = header.child(tier_cell().child(text(20., 24., FontWeight::SEMIBOLD, tk(idx::TEXT_DEFAULT), crate::t!(key))).child(text(14., 18., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), boosts_label(TIER_BOOSTS[n]))));
    }
    let mut table = div().w_full().flex().flex_col().child(header);
    for (i, row) in PERK_ROWS.iter().enumerate() {
        let mut r = div().w_full().flex().items_center().py(z(16.)).when(i + 1 < PERK_ROWS.len(), |d| d.border_b_1().border_color(tk(idx::BORDER_NORMAL))).child(cell().child(text(16., 20., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), crate::t!(row.label))));
        for (n, v) in row.values.iter().enumerate() {
            r = r.child(tier_cell().child(value_cell(*v, n > 0)));
        }
        table = table.child(r);
    }
    let rec = recommended_tier(current);
    let pink = tk(idx::GUILD_BOOSTING_PINK_REFRESH);
    let highlight = div()
        .absolute()
        .top(z(-20.))
        .bottom(z(-20.))
        .w(gpui::relative(0.2))
        .when(rec == 3, |d| d.right_0())
        .when(rec == 2, |d| d.right(gpui::relative(0.2)))
        .rounded(z(12.))
        .border_2()
        .border_color(pink)
        .child(
            div().absolute().top(z(-12.)).left_0().right_0().flex().justify_center().child(
                div()
                    .px(z(8.))
                    .py(z(4.))
                    .rounded(z(4.))
                    .bg(pink)
                    .child(text(12., 16., FontWeight::SEMIBOLD, gpui::white(), if current >= 3 { crate::t!("boost.t.current") } else { crate::t!("boost.t.recommended") }.to_uppercase()).whitespace_nowrap()),
            ),
        );
    div()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(z(48.))
        .child(section_heading(crate::t!("boost.t.heading")))
        .child(div().relative().w_full().child(highlight).child(table))
        .into_any_element()
}

fn community() -> AnyElement {
    let card = |icon: AnyElement, t: SharedString| {
        div()
            .flex_1()
            .min_w(z(200.))
            .p(z(24.))
            .rounded(z(16.))
            .bg(tk(idx::BACKGROUND_BASE_LOW))
            .border_1()
            .border_color(tk(idx::BORDER_SUBTLE))
            .shadow_sm()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(z(12.))
            .child(icon)
            .child(text(16., 20., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), t).text_center())
    };
    let svg = |p: &'static str| gpui::svg().path(crate::icons::path(p)).size(z(48.)).text_color(tk(idx::ICON_MUTED)).into_any_element();
    div()
        .w_full()
        .my(z(80.))
        .flex()
        .flex_col()
        .items_center()
        .gap(z(32.))
        .child(section_heading(crate::t!("boost.c.heading")))
        .child(
            div()
                .w_full()
                .flex()
                .gap(z(24.))
                .child(card(svg("boost"), crate::t!("boost.c.icon")))
                .child(card(img(PROFILE_BADGE_ART).size(z(48.)).into_any_element(), crate::t!("boost.c.badge")))
                .child(card(svg("settings/shield-user"), crate::t!("boost.c.role")))
                .child(card(svg("heart"), crate::t!("boost.c.support"))),
        )
        .into_any_element()
}

fn faq() -> AnyElement {
    let open = FAQ_OPEN.with(|f| f.get());
    let mut list = div().w_full().flex().flex_col().gap(z(4.));
    for (i, (q, a)) in FAQ.iter().enumerate() {
        let on = open == Some(i);
        let group: SharedString = format!("boost-faq-{i}").into();
        list = list.child(
            div()
                .id(("boost-faq", i))
                .group(group.clone())
                .w_full()
                .p(z(12.))
                .rounded(z(8.))
                .border_1()
                .shadow_sm()
                .cursor_pointer()
                .flex()
                .flex_col()
                .gap(z(4.))
                .when(on, |d| d.bg(tk(idx::BACKGROUND_SURFACE_HIGHEST)).border_color(tk(idx::BORDER_STRONG)))
                .when(!on, |d| d.bg(tk(idx::BACKGROUND_BASE_LOW)).border_color(tk(idx::BORDER_SUBTLE)))
                .on_click(move |_, w, _| {
                    FAQ_OPEN.with(|f| f.set(if f.get() == Some(i) { None } else { Some(i) }));
                    w.refresh();
                })
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(z(12.))
                        .child(
                            text(16., 20., FontWeight::SEMIBOLD, if on { tk(idx::TEXT_DEFAULT) } else { tk(idx::TEXT_MUTED) }, crate::t!(q))
                                .flex_1()
                                .group_hover(group, |s| s.text_color(tk(idx::TEXT_DEFAULT))),
                        )
                        .child(
                            gpui::svg()
                                .path(crate::icons::path("chevron"))
                                .size(z(18.))
                                .flex_none()
                                .text_color(tk(idx::INTERACTIVE_ICON_DEFAULT))
                                .when(on, |s| s.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI)))),
                        ),
                )
                .when(on, |d| d.child(text(14., 18., FontWeight::MEDIUM, tk(idx::TEXT_MUTED), crate::t!(a)).pt(z(8.)))),
        );
    }
    div().w_full().flex().flex_col().items_start().child(section_heading(crate::t!("boost.faq.heading")).w_full()).child(div().h(z(32.))).child(list).into_any_element()
}

fn unavailable_notice() -> AnyElement {
    use crate::ui::tooltip::Tipped as _;
    div()
        .flex()
        .justify_center()
        .mb(z(32.))
        .child(
            div()
                .id("boost-unavailable")
                .px(z(16.))
                .py(z(8.))
                .rounded_full()
                .bg(gpui::hsla(0., 0., 1., 0.1))
                .border_2()
                .border_color(gpui::white())
                .child(text(16., 20., FontWeight::MEDIUM, gpui::white(), crate::t!("boost.unavailable").to_uppercase()))
                .tip(crate::t!("boost.unavailable_tip", name = crate::t!("sp.su.squad"))),
        )
        .into_any_element()
}

pub fn render(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let gid = OPEN.with(|o| o.get())?;
    let g = app.guilds.iter().find(|x| x.id == gid)?;
    let white = gpui::white();
    let vw = f32::from(window.viewport_size().width) / crate::theme::zoom();
    let pink = tk(idx::GUILD_BOOSTING_PINK_REFRESH);
    let icon = div()
        .size(z(70.))
        .rounded(z(19.))
        .overflow_hidden()
        .border_1()
        .border_color(gpui::hsla(0., 0., 1., 0.16))
        .shadow_sm()
        .child(crate::ui::invite::guild_icon(Some(&g.icon), &g.name, f32::from(z(70.))));
    let count = div()
        .flex()
        .items_center()
        .gap(z(2.))
        .child(gem(18., if g.boosts > 0 { pink } else { gpui::hsla(0., 0., 1., 0.6) }).flex_none())
        .child(text(16., 20., FontWeight::SEMIBOLD, white, boosts_label(g.boosts)));
    let server = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(z(12.))
        .child(icon)
        .child(div().flex().flex_col().items_center().gap(z(4.)).child(text(20., 24., FontWeight::SEMIBOLD, white, g.name.clone())).child(count));
    let group_member = app.backend.premium_group_member();
    let disable = |b: gpui::Stateful<gpui::Div>| b.opacity(0.5).cursor_not_allowed().hover(|s| s).active(|s| s);
    let boost_btn = crate::ui::controls::button_icon("boost-layer-cta", "boost", crate::t!("boost.cta"), crate::ui::controls::BtnKind::Expressive);
    let boost_btn = if group_member { disable(boost_btn) } else { boost_btn.on_click(cx.listener(|a, _, _, cx| a.stub(&crate::t!("boost.cta"), cx))) };
    let tier2 = app.backend.premium_tier2();
    let nitro_label = if tier2 { crate::t!("boost.gift_nitro") } else { crate::t!("boost.get_nitro") };
    let nl = nitro_label.clone();
    let nitro_btn = if tier2 {
        crate::ui::controls::button_icon("boost-layer-nitro", "gift", nitro_label, crate::ui::controls::BtnKind::Secondary)
    } else {
        crate::ui::controls::button("boost-layer-nitro", nitro_label, crate::ui::controls::BtnKind::Secondary)
    };
    let nitro_btn = if group_member && !tier2 { disable(nitro_btn) } else { nitro_btn.on_click(cx.listener(move |a, _, _, cx| a.stub(&nl, cx))) };
    let hero = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(z(40.))
        .child(
            div()
                .max_w(z(770.))
                .text_center()
                .text_size(z(42.))
                .line_height(z(46.2))
                .font_weight(FontWeight::BOLD)
                .font_family(crate::fonts::display_family(crate::fonts::NITRO))
                .text_color(tk(idx::TEXT_STRONG))
                .child(crate::t!("boost.heading").to_uppercase()),
        )
        .children(group_member.then(unavailable_notice))
        .child(div().flex().justify_center().gap(z(12.)).child(boost_btn).child(nitro_btn));
    let top = div().flex().flex_col().items_center().gap(z(24.)).mb(z(40.)).child(server).child(hero);
    let mut cards = div().w_full().flex().gap(z(24.));
    if vw <= STACK_BELOW {
        cards = cards.flex_col();
    }
    for (tier, perks) in TIERS {
        cards = cards.child(tier_card(tier, perks, g.boost_tier >= tier));
    }
    let background = div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .min_h(z(1280.))
        .overflow_hidden()
        .child(
            div().absolute().inset_0().bg(tk(idx::BACKGROUND_BASE_LOWEST)).child(div().absolute().inset_0().bg(gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(rgba(0x9c59d199), 0.),
                gpui::linear_color_stop(rgba(0x9c59d100), 0.8),
            ))),
        )
        .child(img(HEADER_IMAGE).w_full().object_fit(gpui::ObjectFit::Cover));
    let video = div()
        .w_full()
        .flex()
        .justify_center()
        .mb(z(40.))
        .child(
            div()
                .relative()
                .w(z(772.))
                .h(z(435.))
                .rounded(z(16.))
                .overflow_hidden()
                .border_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .child(img(VIDEO_THUMB).size_full().object_fit(gpui::ObjectFit::Cover))
                .child(div().absolute().inset_0().flex().items_center().justify_center().child(div().size(z(76.)).rounded_full().bg(gpui::white()).flex().items_center().justify_center().child(gpui::svg().path(crate::icons::path("play")).size(z(36.)).text_color(gpui::black())))),
        );
    let content = div()
        .relative()
        .w_full()
        .max_w(z(CONTENT_MAX_W))
        .mx_auto()
        .pt(z(64.))
        .px(z(32.))
        .pb(z(32.))
        .flex()
        .flex_col()
        .child(top)
        .child(video)
        .child(cards)
        .child(div().h(z(96.)))
        .child(perks_table(g.boost_tier))
        .child(community())
        .child(faq());
    let scroller = SCROLL.with(|s| {
        div()
            .id("boost-layer-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(s)
            .bg(tk(idx::BACKGROUND_BASE_LOWEST))
            .child(div().relative().w_full().child(background).child(content).child(div().h(z(120.))))
    });
    let close_btn = div()
        .id("boost-layer-close")
        .absolute()
        .top(z(24.))
        .right(z(24.))
        .size(z(40.))
        .rounded(z(12.))
        .bg(rgba(0x10101280))
        .hover(|s| s.bg(rgba(0x101012b3)))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .tooltip(crate::ui::tooltip::tip(crate::t!("common.close")))
        .on_click(cx.listener(|_, _, _, cx| {
            close();
            cx.notify();
        }))
        .child(gpui::svg().path(crate::icons::path("close-small")).size(z(24.)).text_color(tk(idx::ICON_DEFAULT)));
    let keys = gpui::canvas(
        |_, _, _| (),
        |_, _, w, _| {
            w.on_key_event(|ev: &gpui::KeyDownEvent, phase, w, cx| {
                if phase == gpui::DispatchPhase::Capture && ev.keystroke.key == "escape" && is_open() {
                    close();
                    w.refresh();
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .size_0();
    Some(
        gpui::deferred(div().id("boost-layer").absolute().inset_0().occlude().child(scroller).child(close_btn).child(keys))
            .with_priority(5)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_follow_the_client_definitions() {
        assert_eq!(TIER_BOOSTS, [0, 2, 7, 14]);
        assert_eq!(TIERS.map(|(t, p)| (t, p.len())), [(1, 4), (2, 4), (3, 5)]);
        assert_eq!(TIERS[0].1[1].key, "boost.perk.soundboard");
        assert_eq!(TIERS[2].1[4].icon, "stage");
    }

    #[test]
    fn layer_opens_and_closes() {
        open(Id(5));
        assert!(is_open());
        close();
        assert!(!is_open());
    }

    #[test]
    fn perks_table_matches_the_client_limits() {
        assert_eq!(PERK_ROWS[0].values, [Cell4::Count(50), Cell4::Count(100), Cell4::Count(150), Cell4::Count(250)]);
        assert_eq!(PERK_ROWS[9].values[3], Cell4::Animated);
        assert_eq!(PERK_ROWS[11].values, [Cell4::Bool(false), Cell4::Bool(false), Cell4::Bool(false), Cell4::Bool(true)]);
        assert_eq!((recommended_tier(0), recommended_tier(1), recommended_tier(2), recommended_tier(3)), (2, 2, 3, 3));
        assert!(cell_text(Cell4::Res(720, 30)).is_some_and(|t| t.contains("720p") && t.contains("30")));
        assert!(cell_text(Cell4::Kbps(384)).is_some_and(|t| t.contains("384")));
        assert!(cell_text(Cell4::Bool(true)).is_none());
        assert_eq!(FAQ.len(), 9);
    }
}

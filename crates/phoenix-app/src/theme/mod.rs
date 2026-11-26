use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use gpui::{App, Hsla, Pixels, Rgba, Window, px, rgb, rgba};
use gpui_component::theme::{Theme, ThemeMode};

use crate::ui::settings::AppTheme;

pub mod client_mix;
pub mod client_theme;
pub mod generated;

use generated::idx;

pub fn c(hex: u32) -> Hsla {
    if is_token(hex) {
        rgba(resolve(hex)).into()
    } else {
        rgb(hex).into()
    }
}

pub fn ca(hex_rgba: u32) -> Hsla {
    rgba(resolve(hex_rgba)).into()
}

pub fn c_for(hex: u32, theme: AppTheme) -> Hsla {
    if is_token(hex) {
        rgba(resolve_for(hex, theme)).into()
    } else {
        rgb(hex).into()
    }
}

#[allow(non_snake_case)]
pub fn FONT_UI() -> &'static str {
    crate::fonts::ui_family()
}
#[allow(non_snake_case)]
pub fn FONT_MONO() -> &'static str {
    crate::fonts::mono_family()
}

pub const fn tok(i: usize) -> u32 {
    TOKEN | i as u32
}

pub struct Palette;
#[allow(dead_code)]
impl Palette {
    pub const BG_TERTIARY: u32 = tok(idx::BACKGROUND_BASE_LOWEST);
    pub const BG_SECONDARY: u32 = tok(idx::BACKGROUND_BASE_LOWER);
    pub const BG_SECONDARY_ALT: u32 = tok(idx::BACKGROUND_BASE_LOW);
    pub const BG_PRIMARY: u32 = tok(idx::BACKGROUND_BASE_LOWER);
    pub const BG_FLOATING: u32 = tok(idx::BACKGROUND_SURFACE_HIGH);
    pub const BG_MOD_HOVER: u32 = tok(idx::BACKGROUND_MOD_SUBTLE);
    pub const BG_MOD_SELECTED: u32 = tok(idx::BACKGROUND_MOD_STRONG);
    pub const BG_MSG_HOVER: u32 = tok(idx::BACKGROUND_MOD_MUTED);
    pub const INPUT_BG: u32 = tok(idx::CHAT_BACKGROUND_DEFAULT);
    pub const EMBED_BG: u32 = tok(idx::BACKGROUND_SURFACE_HIGH);
    pub const CODE_BG: u32 = tok(idx::BACKGROUND_BASE_LOWEST);
    pub const CODE_BORDER: u32 = tok(idx::BORDER_SUBTLE);
    pub const DIVIDER: u32 = tok(idx::BORDER_SUBTLE);
    pub const HEADER_SHADOW: u32 = tok(idx::BORDER_SUBTLE);
    pub const TEXT_NORMAL: u32 = tok(idx::TEXT_DEFAULT);
    pub const TEXT_MUTED: u32 = tok(idx::TEXT_MUTED);
    pub const TEXT_HEADER: u32 = tok(idx::TEXT_STRONG);
    pub const TEXT_LINK: u32 = tok(idx::TEXT_LINK);
    pub const INTERACTIVE: u32 = tok(idx::INTERACTIVE_TEXT_DEFAULT);
    pub const INTERACTIVE_HOVER: u32 = tok(idx::INTERACTIVE_TEXT_HOVER);
    pub const INTERACTIVE_MUTED: u32 = tok(idx::INTERACTIVE_MUTED);
    pub const CHANNEL_ICON: u32 = tok(idx::CHANNEL_ICON);
    pub const BLURPLE: u32 = tok(idx::BACKGROUND_BRAND);
    pub const GREEN: u32 = tok(idx::STATUS_POSITIVE);
    pub const YELLOW: u32 = tok(idx::STATUS_WARNING);
    pub const RED: u32 = tok(idx::STATUS_DANGER);
    pub const MENTION_FG: u32 = tok(idx::MENTION_FOREGROUND);
    pub const SPOILER_BG: u32 = tok(idx::SPOILER_HIDDEN_BACKGROUND);
    pub const APP_FRAME: u32 = tok(idx::APP_FRAME_BACKGROUND);
    pub const SURFACE_HIGHER: u32 = tok(idx::BACKGROUND_SURFACE_HIGHER);
    pub const SURFACE_HIGHEST: u32 = tok(idx::BACKGROUND_SURFACE_HIGHEST);
    pub const NOTIFICATION: u32 = tok(idx::BACKGROUND_FEEDBACK_NOTIFICATION);
    pub const TEXT_BRAND: u32 = tok(idx::TEXT_BRAND);
    pub const GUILD_SEPARATOR: u32 = tok(idx::APP_FRAME_BORDER);
    pub const PANEL_BORDER: u32 = tok(idx::BORDER_SUBTLE);
    pub const GUILD_ICON_BG: u32 = tok(idx::BACKGROUND_MOD_SUBTLE);
    pub const TEXT_CRITICAL: u32 = tok(idx::TEXT_FEEDBACK_CRITICAL);
    pub const BORDER_MUTED: u32 = tok(idx::BORDER_MUTED);
    pub const APP_FRAME_BORDER: u32 = tok(idx::APP_FRAME_BORDER);
    pub const BG_BASE_LOW: u32 = tok(idx::BACKGROUND_BASE_LOW);
    pub const GREY: u32 = tok(idx::ICON_STATUS_OFFLINE);
    pub const MENTION_BG: u32 = tok(idx::MENTION_BACKGROUND);
    pub const MENTIONED_MSG_BG: u32 = tok(idx::MESSAGE_MENTIONED_BACKGROUND_DEFAULT);
    pub const MENTIONED_MSG_BAR: u32 = tok(idx::ICON_FEEDBACK_WARNING);
    pub const SPOILER_REVEALED: u32 = tok(idx::SPOILER_REVEALED_BACKGROUND);
}

pub const TOKEN: u32 = 0x0100_0000;
pub const TOKEN_COUNT: usize = generated::COLOR_COUNT;

fn is_token(hex: u32) -> bool {
    hex & 0xFF00_0000 == TOKEN && ((hex & 0xFFFF) as usize) < TOKEN_COUNT
}

static THEME: AtomicU8 = AtomicU8::new(0);
static HIGH_CONTRAST: AtomicBool = AtomicBool::new(false);

pub fn high_contrast() -> bool {
    HIGH_CONTRAST.load(Ordering::Relaxed)
}
static ZOOM: AtomicU32 = AtomicU32::new(100);
static FONT_PX: AtomicU32 = AtomicU32::new(16);
static DENSITY: AtomicU8 = AtomicU8::new(Density::Cozy as u8);
static SATURATION: AtomicU32 = AtomicU32::new(100);
static USER_COLORS: AtomicBool = AtomicBool::new(false);

pub fn desaturate_rgb(v: u32, percent: u32) -> u32 {
    if percent >= 100 {
        return v;
    }
    let mut h: Hsla = rgb(v).into();
    h.s *= percent as f32 / 100.;
    let r: Rgba = h.into();
    let ch = |x: f32| (x.clamp(0., 1.) * 255.).round() as u32;
    (ch(r.r) << 16) | (ch(r.g) << 8) | ch(r.b)
}

pub fn user_color(v: u32) -> u32 {
    if !USER_COLORS.load(Ordering::Relaxed) {
        return v;
    }
    desaturate_rgb(v, SATURATION.load(Ordering::Relaxed))
}

pub fn user_color_saturation() -> f32 {
    if USER_COLORS.load(Ordering::Relaxed) { SATURATION.load(Ordering::Relaxed).min(100) as f32 / 100. } else { 1. }
}

pub fn base_low_rgb() -> u32 {
    let tables = if high_contrast() { &generated::THEMES_HC } else { &generated::THEMES };
    tables[THEME.load(Ordering::Relaxed) as usize][idx::BACKGROUND_BASE_LOW] >> 8
}

pub fn saturate(v: u32) -> u32 {
    let f = SATURATION.load(Ordering::Relaxed);
    if f >= 100 {
        return v;
    }
    let mut h: Hsla = rgba(v).into();
    h.s *= f as f32 / 100.;
    let r: Rgba = h.into();
    let ch = |x: f32| (x.clamp(0., 1.) * 255.).round() as u32;
    (ch(r.r) << 24) | (ch(r.g) << 16) | (ch(r.b) << 8) | ch(r.a)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    Compact = 0,
    Default = 1,
    #[default]
    Cozy = 2,
}

impl Density {
    pub const ALL: [Density; 3] = [Density::Compact, Density::Default, Density::Cozy];
    pub fn key(self) -> &'static str {
        match self {
            Density::Compact => "compact",
            Density::Default => "default",
            Density::Cozy => "cozy",
        }
    }
    pub fn from_key(k: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.key() == k)
    }
    pub const fn scale(self) -> [f32; 7] {
        match self {
            Density::Compact => [4., 6., 10., 12., 16., 20., 26.],
            Density::Default => [4., 8., 12., 16., 20., 24., 32.],
            Density::Cozy => [4., 10., 16., 20., 24., 30., 40.],
        }
    }
}

pub fn density() -> Density {
    match DENSITY.load(Ordering::Relaxed) {
        0 => Density::Compact,
        1 => Density::Default,
        _ => Density::Cozy,
    }
}

pub mod space {
    use super::density;
    pub fn xxs() -> f32 { density().scale()[0] }
    pub fn xs() -> f32 { density().scale()[1] }
    pub fn sm() -> f32 { density().scale()[2] }
    pub fn md() -> f32 { density().scale()[3] }
    pub fn lg() -> f32 { density().scale()[4] }
    pub fn xl() -> f32 { density().scale()[5] }
    pub fn xxl() -> f32 { density().scale()[6] }
    pub fn channels_name_lh() -> f32 { [20., 24., 28.][density() as usize] }
    pub fn spine_left() -> f32 { [20., 24., 28.][density() as usize] }
    pub fn category_before() -> f32 { if density() == super::Density::Compact { 8. } else { 16. } }
}
static STYLE_GEN: AtomicU32 = AtomicU32::new(0);

pub fn bump_style_generation() {
    STYLE_GEN.fetch_add(1, Ordering::Relaxed);
}

pub fn style_generation() -> u32 {
    STYLE_GEN.load(Ordering::Relaxed)
}

fn theme_index(t: AppTheme) -> u8 {
    match t {
        AppTheme::Dark => 0,
        AppTheme::Light => 1,
        AppTheme::Ash => 2,
        AppTheme::Onyx => 3,
    }
}

pub fn resolve(hex: u32) -> u32 {
    if is_token(hex) {
        if let Some(v) = crate::ui::profile_theme::lookup((hex & 0xFFFF) as usize) {
            return v;
        }
        let i = (hex & 0xFFFF) as usize;
        if let Some(v) = client_theme::lookup(i) {
            return saturate(v);
        }
        let tables = if high_contrast() { &generated::THEMES_HC } else { &generated::THEMES };
        saturate(tables[THEME.load(Ordering::Relaxed) as usize][i])
    } else {
        hex
    }
}

pub fn resolve_for(hex: u32, theme: AppTheme) -> u32 {
    if is_token(hex) {
        generated::THEMES[theme_index(theme) as usize][(hex & 0xFFFF) as usize]
    } else {
        hex
    }
}

pub fn active() -> AppTheme {
    match THEME.load(Ordering::Relaxed) {
        1 => AppTheme::Light,
        2 => AppTheme::Ash,
        3 => AppTheme::Onyx,
        _ => AppTheme::Dark,
    }
}

pub fn frame_borders() -> bool {
    active() == AppTheme::Onyx || high_contrast()
}

pub fn is_light() -> bool {
    active() == AppTheme::Light
}

pub fn sync(prefs: &crate::ui::settings::Prefs, window: &mut Window, cx: &mut App) {
    let os_dark = matches!(window.appearance(), gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark);
    let t = theme_index(crate::ui::settings::effective_theme(prefs, os_dark));
    let prev = THEME.swap(t, Ordering::Relaxed);
    let pz = ZOOM.swap(prefs.local.zoom.clamp(50, 200), Ordering::Relaxed);
    let pf = FONT_PX.swap(prefs.local.font_size.round().clamp(12., 24.) as u32, Ordering::Relaxed);
    let pd = DENSITY.swap(prefs.synced.density as u8, Ordering::Relaxed);
    let ps = SATURATION.swap(prefs.local.saturation.min(100), Ordering::Relaxed);
    let uc = prefs.local.flag("sat-custom", false);
    if USER_COLORS.swap(uc, Ordering::Relaxed) != uc {
        STYLE_GEN.fetch_add(1, Ordering::Relaxed);
    }
    let hc = prefs.local.flag("contrast", false);
    if HIGH_CONTRAST.swap(hc, Ordering::Relaxed) != hc {
        STYLE_GEN.fetch_add(1, Ordering::Relaxed);
    }
    if client_theme::set_active(crate::ui::settings::client_preset(prefs), hc) {
        STYLE_GEN.fetch_add(1, Ordering::Relaxed);
    }
    if ps != prefs.local.saturation.min(100) {
        STYLE_GEN.fetch_add(1, Ordering::Relaxed);
    }
    static LOCALE: std::sync::Mutex<&str> = std::sync::Mutex::new("");
    let locale = crate::i18n::locale();
    let locale_changed = LOCALE.lock().map(|mut l| std::mem::replace(&mut *l, locale) != locale).unwrap_or(false);
    if locale_changed || prev != t || pz != zoom_raw() || pf != FONT_PX.load(Ordering::Relaxed) || pd != prefs.synced.density as u8 {
        STYLE_GEN.fetch_add(1, Ordering::Relaxed);
    }
    window.set_page_zoom(page_zoom());
    if window.rem_size() != px(16.) {
        window.set_rem_size(px(16.));
    }
    if prev != t {
        let mode = if t == 1 { ThemeMode::Light } else { ThemeMode::Dark };
        Theme::change(mode, Some(window), cx);
    }
}

fn zoom_raw() -> u32 {
    ZOOM.load(Ordering::Relaxed)
}

pub fn page_zoom() -> f32 {
    ZOOM.load(Ordering::Relaxed) as f32 / 100.
}

pub fn zoom() -> f32 {
    1.0
}

pub fn font_scale() -> f32 {
    FONT_PX.load(Ordering::Relaxed) as f32 / 16.
}

pub fn font_scaled_down() -> bool {
    FONT_PX.load(Ordering::Relaxed) < 16
}

pub fn z(v: f32) -> Pixels {
    px(v * zoom())
}

pub fn fz(v: f32) -> Pixels {
    px(v * zoom() * font_scale())
}

pub const DEFAULT_AVATAR: [u32; 5] = [0x5865F2, 0x757E8A, 0x3BA55C, 0xFAA61A, 0xED4245];

pub fn presence_color(p: crate::model::Presence) -> Hsla {
    use crate::model::Presence::*;
    c(match p {
        Online => Palette::GREEN,
        Idle => Palette::YELLOW,
        Dnd => Palette::RED,
        Offline => Palette::GREY,
    })
}

pub fn role_color(v: Option<u32>) -> Hsla {
    v.map(c).unwrap_or_else(|| c(Palette::TEXT_NORMAL))
}

#[allow(dead_code)]
pub fn to_rgba(h: Hsla) -> Rgba {
    h.into()
}

pub mod m {
    use super::generated::global as g;
    pub const SERVER_BAR_W: f32 = 80.;
    pub fn server_bar_w() -> f32 {
        GUILD_ICON_BOX + 2. * super::space::md()
    }
    const GUILD_ICON_BOX: f32 = 40.;
    pub fn sidebar_extra_w() -> f32 {
        SERVER_BAR_W - server_bar_w()
    }
    pub const GUILD_ICON: f32 = g::GUILDBAR_AVATAR_SIZE;
    pub const SIDEBAR_W: f32 = g::CUSTOM_GUILD_SIDEBAR_WIDTH;
    pub fn member_list_w() -> f32 {
        match super::density() {
            super::Density::Compact => 256.,
            super::Density::Default => g::CUSTOM_MEMBER_LIST_WIDTH,
            super::Density::Cozy => 268.,
        }
    }
    pub const HEADER_H: f32 = g::CUSTOM_CHANNEL_HEADER_HEIGHT;
    pub const TITLE_BAR_H: f32 = g::CUSTOM_APP_TOP_BAR_HEIGHT;
    pub const TITLE_BAR_ITEM_RADIUS: f32 = g::CUSTOM_APP_TOP_BAR_ITEM_RADIUS;
    pub const HEADER_ICON: f32 = g::CHAT_INPUT_ICON_SIZE;
    pub const FRAME_RADIUS: f32 = g::RADIUS_MD;
    pub const USER_PANEL_H: f32 = 56.;
    pub const AVATAR: f32 = 40.;
    pub const MSG_GUTTER: f32 = 72.;
    pub const MEMBER_ROW_H: f32 = 42.;
}

#[cfg(test)]
mod tests {
    use super::generated::{self, global, idx, shadow};
    use super::*;

    #[test]
    fn generated_tokens_parse_known_values() {
        assert_eq!(generated::DARK[idx::BACKGROUND_BASE_LOW], 0x202024FF);
        assert_eq!(generated::ASH[idx::BACKGROUND_BASE_LOW], 0x36373EFF);
        assert_eq!(generated::DARK[idx::TEXT_DEFAULT], 0xEFEFF1FF);
        assert_eq!(generated::DARK[idx::MENTION_BACKGROUND], 0x5865F23D);
        assert_eq!(generated::NAMES[idx::BACKGROUND_BRAND], "--background-brand");
        assert_eq!(global::SPACE_8, 8.0);
        assert_eq!(global::RADIUS_MD, 12.0);
        assert_eq!(global::GUILDBAR_AVATAR_SIZE, 40.0);
        assert_eq!(shadow::SHADOW_HIGH[0][0].blur, 24.0);
        assert_eq!(resolve_for(Palette::BG_TERTIARY, AppTheme::Dark), 0x121214FF);
        assert_eq!(resolve_for(Palette::BLURPLE, AppTheme::Light), 0x5865F2FF);
    }
}

#[cfg(test)]
mod user_color_tests {
    #[test]
    fn user_colors_lose_saturation_like_the_client() {
        assert_eq!(super::desaturate_rgb(0xFF0000, 100), 0xFF0000);
        assert_eq!(super::desaturate_rgb(0xFF0000, 0), 0x808080);
        let half = super::desaturate_rgb(0xFF0000, 50);
        assert_eq!(half, 0xBF4040);
    }
}

#[cfg(test)]
mod density_tests {
    use super::Density;

    #[test]
    fn density_scales_match_css() {
        let c = Density::Cozy.scale();
        assert_eq!((c[1], c[3], c[5]), (10., 20., 30.));
        assert_eq!(Density::Default.scale()[3], 16.);
        assert_eq!(Density::Compact.scale()[6], 26.);
        assert_eq!(Density::default(), Density::Cozy);
        for d in Density::ALL {
            assert_eq!(Density::from_key(d.key()), Some(d));
        }
    }
}

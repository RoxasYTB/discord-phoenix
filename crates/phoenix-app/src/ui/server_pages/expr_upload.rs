use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    sync::Arc,
};

use gpui::{AnyElement, Bounds, Context, DispatchPhase, FontWeight, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, SharedString, Styled, canvas, div, prelude::*, relative};
use phoenix_api::rest::{
    form::{SoundUpload, StickerUpload, md5_hex},
    guard::GuardedWrite,
};
use phoenix_voice::audio::effects::Pcm;

use super::{icon, t, text};
use crate::{
    app::PhoenixApp,
    model::Id,
    theme::{generated::idx, z},
    ui::controls::{BtnKind, button, button_sm},
};

pub const STICKER_MAX_BYTES: usize = 524_288;
pub const STICKER_SIZE: u32 = 320;
pub const STICKER_NAME_MAX: usize = 30;
pub const STICKER_DESC_MAX: usize = 100;
pub const SOUND_NAME_MAX: usize = 32;
pub const SOUND_MAX_MS: f64 = 5_000.;
pub const TRIMMER_H: f32 = 72.;
pub const HANDLE_W: f32 = 16.;
pub const WAVE_BLOCK: f32 = 2.;
pub const LOUDNESS_THRESHOLD: f32 = 0.3;
pub const MODAL_W: f32 = 480.;
pub const WAVE_W: f32 = MODAL_W - 48. - 18. - 40. - 2. * HANDLE_W;

pub const STICKER_TYPES: [&str; 6] = ["application/json", "image/png", "image/apng", "image/gif", "image/jpeg", "image/jpg"];
pub const STICKER_EXTS: [&str; 6] = ["json", "png", "apng", "gif", "jpeg", "jpg"];
pub const SOUND_TYPES: [&str; 9] = ["audio/mpeg", "audio/mp3", "audio/wav", "audio/x-wav", "audio/ogg", "application/ogg", "audio/opus", "audio/x-opus+ogg", "video/mp4"];
pub const SOUND_EXTS: [&str; 6] = ["mp3", "wav", "ogg", "opus", "x-wav", "mp4"];

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Unicode(String),
    Custom { id: u64, name: String, animated: bool },
}

impl Tag {
    pub fn sticker_tags(&self) -> String {
        match self {
            Tag::Unicode(s) => s.clone(),
            Tag::Custom { id, .. } => id.to_string(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Tag::Unicode(s) => crate::ui::pickers::emoji::index_of(s).map(|i| crate::ui::pickers::emoji::shortcode(i).to_owned()).unwrap_or_else(|| s.clone()),
            Tag::Custom { name, .. } => name.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sticker,
    Sound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    Start,
    End,
    Playhead,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trim {
    pub start_ms: f64,
    pub end_ms: f64,
    pub playhead_ms: f64,
}

#[derive(Clone)]
pub struct Picked {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub original_md5: String,
    pub preview: Option<Arc<gpui::Image>>,
    pub lottie: Option<Arc<Vec<u8>>>,
    pub pcm: Option<Arc<Pcm>>,
    pub duration_ms: f64,
    pub peaks: Vec<f32>,
}

#[derive(Clone)]
pub struct Upload {
    pub guild: Id,
    pub kind: Kind,
    pub partnered: bool,
    pub file: Option<Picked>,
    pub tag: Option<Tag>,
    pub error: Option<(SharedString, bool)>,
    pub volume: f32,
    pub max_volume: f32,
    pub trim: Trim,
    pub drag: Option<Grip>,
    pub playing: Option<phoenix_voice::audio::effects::Handle>,
    pub clock: Option<(std::time::Instant, f64)>,
}

thread_local! {
    static UPLOAD: RefCell<Option<Upload>> = const { RefCell::new(None) };
    static BOUNDS: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Upload) -> R) -> Option<R> {
    UPLOAD.with(|u| u.borrow_mut().as_mut().map(f))
}

fn snapshot<R>(f: impl FnOnce(&Upload) -> R) -> Option<R> {
    UPLOAD.with(|u| u.borrow().as_ref().map(f))
}

pub fn is_open() -> bool {
    UPLOAD.with(|u| u.borrow().is_some())
}

pub fn open_sticker(guild: Id, partnered: bool) {
    open(guild, Kind::Sticker, partnered);
}

pub fn open_sound(guild: Id) {
    open(guild, Kind::Sound, false);
}

fn open(guild: Id, kind: Kind, partnered: bool) {
    super::forget_prefix(guild, "eu-");
    UPLOAD.with(|u| {
        *u.borrow_mut() = Some(Upload {
            guild,
            kind,
            partnered,
            file: None,
            tag: None,
            error: None,
            volume: 1.,
            max_volume: 1.,
            trim: Trim { start_ms: 0., end_ms: 2. * 10_000., playhead_ms: 0. },
            drag: None,
            playing: None,
            clock: None,
        })
    });
}

pub fn close() {
    let guild = UPLOAD.with(|u| {
        let mut u = u.borrow_mut();
        let g = u.as_ref().map(|x| x.guild);
        if let Some(h) = u.as_ref().and_then(|x| x.playing.as_ref()) {
            h.stop();
        }
        *u = None;
        g
    });
    if let Some(g) = guild {
        super::forget_prefix(g, "eu-");
    }
}

pub fn pick_emoji(tag: Tag) {
    with(|u| u.tag = Some(tag));
}

pub fn sticker_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("apng") => "image/apng",
        Some("gif") => "image/gif",
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => "",
    }
}

pub fn sound_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("x-wav") => "audio/x-wav",
        Some("ogg") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("mp4") => "video/mp4",
        _ => "",
    }
}

pub fn is_apng(bytes: &[u8]) -> bool {
    let mut i = 8;
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let kind = &bytes[i + 4..i + 8];
        if kind == b"acTL" {
            return true;
        }
        if kind == b"IDAT" {
            return false;
        }
        i += 12 + len;
    }
    false
}

pub fn fit_png(bytes: &[u8], size: u32) -> Option<Vec<u8>> {
    let src = image::load_from_memory(bytes).ok()?;
    let scaled = src.resize(size, size, image::imageops::FilterType::Triangle).to_rgba8();
    let mut canvas = image::RgbaImage::new(size, size);
    let (x, y) = ((size - scaled.width()) / 2, (size - scaled.height()) / 2);
    image::imageops::overlay(&mut canvas, &scaled, i64::from(x), i64::from(y));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(canvas).write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

#[derive(Debug, PartialEq)]
pub enum StickerCheck {
    Ok { bytes: Vec<u8>, mime: &'static str, resized: bool },
    Unsupported,
    TooBig,
    LottieNotAllowed,
}

pub fn check_sticker(mime: &str, bytes: &[u8], partnered: bool) -> StickerCheck {
    if !STICKER_TYPES.contains(&mime) {
        return StickerCheck::Unsupported;
    }
    let (out, out_mime, resized) = match mime {
        "image/jpeg" | "image/jpg" => match fit_png(bytes, STICKER_SIZE) {
            Some(p) => (p, "image/png", true),
            None => return StickerCheck::Unsupported,
        },
        "image/png" if bytes.len() > STICKER_MAX_BYTES && !is_apng(bytes) => match fit_png(bytes, STICKER_SIZE) {
            Some(p) => (p, "image/png", true),
            None => return StickerCheck::Unsupported,
        },
        m => (bytes.to_vec(), STICKER_TYPES.iter().copied().find(|t| *t == m).unwrap_or("image/png"), false),
    };
    if out.len() > STICKER_MAX_BYTES {
        return StickerCheck::TooBig;
    }
    if out_mime == "application/json" && !partnered {
        return StickerCheck::LottieNotAllowed;
    }
    StickerCheck::Ok { bytes: out, mime: out_mime, resized }
}

pub fn sticker_ready(busy: bool, blocking: bool, name: &str, has_tag: bool, has_file: bool, desc: &str) -> bool {
    let d = desc.chars().count();
    let desc_ok = d == 0 || (2..=STICKER_DESC_MAX).contains(&d);
    !busy && !blocking && name.chars().count() >= 2 && has_tag && has_file && desc_ok
}

pub fn sound_ready(name: &str, has_file: bool, trim: Option<Trim>) -> bool {
    let len_ok = trim.is_none_or(|t| {
        let s = (t.end_ms - t.start_ms) / 1000.;
        s > 0. && s <= SOUND_MAX_MS / 1000.
    });
    name.chars().count() >= 2 && has_file && len_ok
}

pub fn js_number(x: f32) -> String {
    if x != 0. && x.abs() < 1e-6 { format!("{x:e}") } else { format!("{x}") }
}

pub fn waveform(channel: &[f32], width: f32) -> (Vec<f32>, f32) {
    let blocks = (width / WAVE_BLOCK).floor().max(0.) as usize;
    if blocks == 0 || channel.is_empty() {
        return (Vec::new(), 1.);
    }
    let per = channel.len() / blocks;
    let mut h = Vec::with_capacity(blocks);
    for b in 0..blocks {
        let start = per * b;
        let sum: f32 = channel[start..start + per].iter().map(|x| x.abs()).sum();
        h.push(if per == 0 { f32::NAN } else { sum / per as f32 });
    }
    let max = h.iter().copied().fold(f32::MIN, f32::max);
    let norm: Vec<f32> = h.iter().map(|v| v * max.powi(-1)).collect();
    let mut sorted: Vec<(String, f32)> = h.iter().map(|v| (js_number(*v), *v)).collect();
    sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
    let p95 = sorted[((0.95 * h.len() as f32).floor() as usize).min(h.len() - 1)].1;
    (norm, (LOUDNESS_THRESHOLD / p95).min(1.))
}

pub fn first_channel(pcm: &Pcm) -> Vec<f32> {
    let ch = usize::from(pcm.channels.max(1));
    pcm.samples.iter().step_by(ch).copied().collect()
}

pub fn duration_ms(pcm: &Pcm) -> f64 {
    let frames = pcm.samples.len() / usize::from(pcm.channels.max(1));
    frames as f64 * 1000. / f64::from(pcm.rate.max(1))
}

pub fn trimmed(pcm: &Pcm, start_ms: f64, end_ms: f64) -> Pcm {
    let total = duration_ms(pcm);
    let ch = usize::from(pcm.channels.max(1));
    let frames = pcm.samples.len() / ch;
    let end = end_ms.min(total);
    if start_ms <= 0. && end >= total {
        return Pcm { rate: pcm.rate, channels: pcm.channels, samples: pcm.samples.clone() };
    }
    let a = ((start_ms / total) * frames as f64).floor() as usize;
    let b = (((end / total) * frames as f64).floor() as usize).min(frames.saturating_sub(1));
    let mut samples = Vec::with_capacity((b.saturating_sub(a) + 1) * ch);
    for f in a..=b {
        samples.extend_from_slice(&pcm.samples[f * ch..f * ch + ch]);
    }
    samples.truncate(b.saturating_sub(a) * ch);
    Pcm { rate: pcm.rate, channels: pcm.channels, samples }
}

pub fn left_pct(ms: f64, total_ms: f64) -> f32 {
    (100. * ms / total_ms.max(1.)) as f32
}

pub fn right_pct(ms: f64, total_ms: f64) -> f32 {
    (-(100. * ms) / total_ms.max(1.) + 100.) as f32
}

pub fn volume_label(v: f32, max: f32) -> String {
    format!("{:.0}%", 100. * v / max.max(f32::EPSILON))
}

pub fn sticker_write(guild: Id, name: &str, tag: &Tag, description: &str, file: &Picked) -> GuardedWrite {
    GuardedWrite::create_sticker(
        guild.0,
        &StickerUpload {
            name: name.to_owned(),
            tags: tag.sticker_tags(),
            description: description.to_owned(),
            filename: file.name.clone(),
            content_type: file.mime.clone(),
            file: file.bytes.clone(),
            original_md5: Some(file.original_md5.clone()),
        },
    )
}

pub fn sound_write(guild: Id, name: &str, volume: f32, tag: Option<&Tag>, ogg: &[u8]) -> GuardedWrite {
    let (emoji_id, emoji_name) = match tag {
        Some(Tag::Custom { id, .. }) => (Some(id.to_string()), None),
        Some(Tag::Unicode(s)) => (None, Some(s.clone())),
        None => (None, None),
    };
    GuardedWrite::create_sound(
        guild.0,
        &SoundUpload { name: name.to_owned(), sound: crate::ui::file_pick::encode(ogg, "audio/ogg"), volume: f64::from((volume * 100.).round() / 100.), emoji_id, emoji_name },
    )
}

fn kib(bytes: usize) -> SharedString {
    crate::t!("size.kb", size = bytes.div_ceil(1024))
}

fn load_sticker(path: &Path) {
    let mime = sticker_mime(path);
    let Ok(bytes) = std::fs::read(path) else { return };
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("sticker").to_owned();
    let had_file = snapshot(|u| u.file.is_some()).unwrap_or(false);
    let partnered = snapshot(|u| u.partnered).unwrap_or(false);
    let res = check_sticker(mime, &bytes, partnered);
    with(|u| match res {
        StickerCheck::Unsupported => u.error = Some((crate::t!("eu.unsupported"), true)),
        StickerCheck::TooBig => u.error = Some((crate::t!("eu.too_big", size = kib(STICKER_MAX_BYTES)), !had_file)),
        StickerCheck::LottieNotAllowed => u.error = Some((crate::t!("eu.lottie"), true)),
        StickerCheck::Ok { bytes: out, mime: out_mime, .. } => {
            let preview = match out_mime {
                "image/gif" => Some(gpui::ImageFormat::Gif),
                "application/json" => None,
                _ => Some(gpui::ImageFormat::Png),
            }
            .map(|f| Arc::new(gpui::Image::from_bytes(f, out.clone())));
            u.error = None;
            let lottie = (out_mime == "application/json").then(|| Arc::new(out.clone()));
            u.file = Some(Picked { name: name.clone(), mime: out_mime.to_owned(), original_md5: md5_hex(&bytes), bytes: out, preview, lottie, pcm: None, duration_ms: 0., peaks: Vec::new() });
        }
    });
}

fn load_sound(path: &Path) {
    let mime = sound_mime(path);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("sound").to_owned();
    if !SOUND_TYPES.contains(&mime) {
        with(|u| u.error = Some((crate::t!("eu.bad_audio", kind = mime), false)));
        return;
    }
    let Ok(bytes) = std::fs::read(path) else { return };
    let Some(pcm) = crate::media::audio_file::decode(&bytes, &name) else {
        with(|u| u.error = Some((crate::t!("eu.unreadable"), false)));
        return;
    };
    let total = duration_ms(&pcm);
    let (peaks, mult) = waveform(&first_channel(&pcm), WAVE_W);
    with(|u| {
        if let Some(h) = u.playing.take() {
            h.stop();
        }
        u.clock = None;
        u.error = None;
        u.max_volume = mult;
        u.volume = u.volume.min(mult);
        u.trim = Trim { start_ms: 0., end_ms: total, playhead_ms: 0. };
        u.file = Some(Picked { name, mime: mime.to_owned(), original_md5: md5_hex(&bytes), bytes, preview: None, lottie: None, pcm: Some(Arc::new(pcm)), duration_ms: total, peaks });
    });
}

pub fn load_path(path: &Path) {
    match snapshot(|u| u.kind) {
        Some(Kind::Sticker) => load_sticker(path),
        Some(Kind::Sound) => load_sound(path),
        None => {}
    }
}

fn browse(cx: &mut Context<PhoenixApp>) {
    let exts: &'static [&'static str] = match snapshot(|u| u.kind) {
        Some(Kind::Sound) => &SOUND_EXTS,
        _ => &STICKER_EXTS,
    };
    crate::ui::file_pick::pick(exts, cx, |_, path, cx| {
        load_path(&path);
        cx.notify();
    });
}

fn label(s: SharedString, required: bool) -> gpui::Div {
    crate::ui::modals::field_label(s, required)
}

fn input_shell() -> gpui::Div {
    div().rounded(z(8.)).bg(t(idx::INPUT_BACKGROUND_DEFAULT)).border_1().border_color(t(idx::INPUT_BORDER_DEFAULT))
}

fn file_input(id: &'static str, filename: Option<&str>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (shown, col) = match filename {
        Some(f) if !f.is_empty() => (SharedString::from(f.to_owned()), idx::TEXT_DEFAULT),
        _ => (crate::t!("eu.choose_file"), idx::TEXT_MUTED),
    };
    input_shell()
        .id(id)
        .min_h(z(40.))
        .pl(z(8.))
        .pr(z(4.))
        .flex()
        .items_center()
        .gap(z(8.))
        .cursor_pointer()
        .child(icon("chat/upload", 18., idx::INTERACTIVE_TEXT_DEFAULT).flex_none())
        .child(div().flex_1().min_w_0().child(text(16., 20., FontWeight::NORMAL, col, shown).truncate()))
        .child(button_sm(SharedString::from(format!("{id}-browse")), crate::t!("eu.browse"), BtnKind::Secondary).flex_none())
        .on_click(cx.listener(|_, _, _, cx| browse(cx)))
        .into_any_element()
}

fn tag_glyph(tag: &Tag) -> AnyElement {
    match tag {
        Tag::Custom { id, animated, .. } => gpui::img(SharedString::from(format!("https://cdn.discordapp.com/emojis/{id}.{}?size=48", if *animated { "gif" } else { "webp" })))
            .size(z(20.))
            .flex_none()
            .into_any_element(),
        Tag::Unicode(s) => div().size(z(20.)).flex_none().flex().items_center().justify_center().text_size(z(18.)).line_height(z(20.)).child(s.clone()).into_any_element(),
    }
}

fn emoji_input(required: bool, tag: Option<&Tag>, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (shown, col) = match tag {
        Some(t) => (SharedString::from(format!(":{}:", t.label())), idx::TEXT_DEFAULT),
        None => (crate::t!("eu.click_select"), idx::TEXT_MUTED),
    };
    let has = tag.is_some();
    div()
        .w_full()
        .child(label(crate::t!("eu.related_emoji"), required))
        .child(
            input_shell()
                .id("eu-emoji")
                .group("eu-emoji")
                .min_h(z(40.))
                .px(z(12.))
                .flex()
                .items_center()
                .gap(z(8.))
                .cursor_pointer()
                .child(match tag {
                    Some(t) => tag_glyph(t),
                    None => icon("emoji", 20., idx::INTERACTIVE_TEXT_DEFAULT).flex_none().into_any_element(),
                })
                .child(div().flex_1().min_w_0().child(text(16., 20., FontWeight::NORMAL, col, shown).truncate()))
                .when(has, |d| {
                    d.child(
                        div()
                            .id("eu-emoji-clear")
                            .flex_none()
                            .invisible()
                            .group_hover("eu-emoji", |s| s.visible())
                            .child(icon("close-small", 16., idx::INTERACTIVE_TEXT_DEFAULT))
                            .on_click(cx.listener(|_, _, _, cx| {
                                with(|u| u.tag = None);
                                cx.stop_propagation();
                                cx.notify();
                            })),
                    )
                })
                .on_click(cx.listener(|a, _, w, cx| {
                    crate::ui::pickers::open(a, crate::ui::pickers::Tab::Emoji, crate::ui::pickers::EmojiTarget::ExprUpload, w, cx);
                })),
        )
        .into_any_element()
}

fn text_input(guild: Id, key: &str, placeholder: SharedString, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let empty = super::value(guild, key, cx).is_none_or(|v| v.is_empty());
    super::field(guild, key, "", cx).relative().when(empty, |d| {
        d.child(div().absolute().top_0().left(z(12.)).right(z(12.)).h_full().flex().items_center().child(text(16., 20., FontWeight::NORMAL, idx::TEXT_MUTED, placeholder).truncate()))
    })
}

fn clipped(guild: Id, key: &str, max: usize, cx: &gpui::App) -> String {
    super::value(guild, key, cx).or_else(|| super::area_value(guild, key, cx)).unwrap_or_default().chars().take(max).collect()
}

fn primary(id: &'static str, label: SharedString, ok: bool, run: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    button(id, label, BtnKind::Primary).when(!ok, |d| d.opacity(0.5).cursor(gpui::CursorStyle::OperationNotAllowed)).when(ok, |d| {
        d.on_click(cx.listener(move |a, _, _, cx| {
            run(a, cx);
            cx.notify();
        }))
    })
}

fn footer(kind: Kind, ok: bool, run: impl Fn(&mut PhoenixApp, &mut Context<PhoenixApp>) + 'static, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let (cancel, upload) = match kind {
        Kind::Sticker => (crate::t!("eu.never_mind"), crate::t!("eu.upload")),
        Kind::Sound => (crate::t!("eu.sound_never_mind"), crate::t!("eu.sound_upload")),
    };
    div()
        .flex()
        .gap(z(8.))
        .child(button("eu-cancel", cancel, BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| {
            close();
            cx.notify();
        })))
        .child(primary("eu-submit", upload, ok, run, cx))
        .into_any_element()
}

fn close_action(cx: &mut Context<PhoenixApp>) -> crate::ui::modal::Action {
    let e = cx.entity();
    Rc::new(move |_, cx| {
        close();
        e.update(cx, |_, cx| cx.notify());
    })
}

fn sticker_preview(file: Option<&Picked>) -> AnyElement {
    let half = |dark: bool| {
        let lottie = file.and_then(|f| f.lottie.clone().map(|b| (SharedString::from(format!("upload-{}", f.original_md5)), b)));
        let inner: AnyElement = match (file.and_then(|f| f.preview.clone()), lottie) {
            (Some(img), _) => gpui::img(img).size(z(160.)).object_fit(gpui::ObjectFit::Contain).into_any_element(),
            (None, Some((key, bytes))) => {
                let still = !crate::media::sticker_plays(&key);
                crate::media::sticker_hover(key.clone(), crate::media::lottie::image_bytes(key, bytes, 320, still).size(z(160.)).object_fit(gpui::ObjectFit::Contain)).into_any_element()
            }
            (None, None) => icon("image-plus", 32., idx::PRIMARY_400).into_any_element(),
        };
        div().flex_1().h_full().relative().flex().items_center().justify_center().bg(t(if dark { idx::PRIMARY_600 } else { idx::WHITE })).child(inner)
    };
    div().h(z(214.)).w_full().flex().overflow_hidden().rounded(z(8.)).border_1().border_color(t(idx::BORDER_SUBTLE)).child(half(true)).child(half(false)).into_any_element()
}

fn sticker_modal(u: &Upload, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let g = u.guild;
    let name = clipped(g, "eu-name", STICKER_NAME_MAX, cx);
    let desc = clipped(g, "eu-desc", STICKER_DESC_MAX, cx);
    let blocking = u.error.as_ref().is_some_and(|e| e.1);
    let ok = sticker_ready(false, blocking, &name, u.tag.is_some(), u.file.is_some(), &desc);
    let desc_empty = desc.is_empty();
    let body = div()
        .id("eu-body")
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(z(16.))
        .pb(z(16.))
        .child(div().child(label(crate::t!("eu.preview"), false)).child(sticker_preview(u.file.as_ref())))
        .child(div().child(label(crate::t!("eu.file"), true)).child(file_input("eu-file", u.file.as_ref().map(|f| f.name.as_str()), cx)))
        .child(emoji_input(true, u.tag.as_ref(), cx))
        .child(div().child(label(crate::t!("eu.sticker_name"), true)).child(text_input(g, "eu-name", crate::t!("eu.sticker_name_ph"), cx)))
        .child(
            div()
                .child(label(crate::t!("eu.description"), false))
                .child(super::area_field(g, "eu-desc", "", 3, 64., cx).when(desc_empty, |d| {
                    d.child(div().absolute().top(z(10.)).left(z(10.)).right(z(10.)).child(text(16., 20., FontWeight::NORMAL, idx::TEXT_MUTED, crate::t!("eu.description_ph"))))
                }))
                .child(text(12., 16., FontWeight::NORMAL, idx::TEXT_SUBTLE, crate::t!("eu.description_help")).mt(z(8.))),
        )
        .children(u.error.as_ref().map(|(m, _)| text(14., 18., FontWeight::NORMAL, idx::TEXT_FEEDBACK_CRITICAL, m.clone())));
    let submit = move |a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>| {
        let name = clipped(g, "eu-name", STICKER_NAME_MAX, cx);
        let desc = clipped(g, "eu-desc", STICKER_DESC_MAX, cx);
        let Some((Some(tag), Some(file))) = snapshot(|u| (u.tag.clone(), u.file.clone())) else { return };
        a.blocked_write(sticker_write(g, &name, &tag, &desc, &file), cx);
    };
    let subtitle = if u.partnered { crate::t!("eu.sticker_sub_lottie", size = kib(STICKER_MAX_BYTES)) } else { crate::t!("eu.sticker_sub", size = kib(STICKER_MAX_BYTES)) };
    crate::ui::modals::card_with(MODAL_W, crate::t!("eu.sticker_title"), Some(subtitle), body.into_any_element(), Some(footer(Kind::Sticker, ok, submit, cx)), close_action(cx))
}

fn frac_at(x: Pixels) -> Option<f64> {
    let b = BOUNDS.with(Cell::get)?;
    let w = f32::from(b.size.width) - 2. * HANDLE_W;
    Some(f64::from(((f32::from(x) - f32::from(b.origin.x) - HANDLE_W) / w.max(1.)).clamp(0., 1.)))
}

pub fn grip_for(trim: Trim, total: f64, frac: f64, width: f32) -> Grip {
    let px = |ms: f64| (ms / total.max(1.)) as f32 * width;
    let x = frac as f32 * width;
    if (x - px(trim.start_ms)).abs() <= HANDLE_W {
        Grip::Start
    } else if (x - px(trim.end_ms)).abs() <= HANDLE_W {
        Grip::End
    } else {
        Grip::Playhead
    }
}

pub fn drag_to(trim: Trim, grip: Grip, ms: f64) -> Trim {
    let mut t = trim;
    match grip {
        Grip::Start => {
            t.start_ms = ms.min(t.end_ms);
            t.playhead_ms = t.start_ms;
        }
        Grip::End => {
            t.end_ms = ms.max(t.start_ms);
            t.playhead_ms = t.start_ms;
        }
        Grip::Playhead => t.playhead_ms = ms.clamp(t.start_ms, t.end_ms),
    }
    t
}

fn apply_drag(x: Pixels) {
    let Some(f) = frac_at(x) else { return };
    with(|u| {
        let (Some(grip), Some(total)) = (u.drag, u.file.as_ref().map(|f| f.duration_ms)) else { return };
        u.trim = drag_to(u.trim, grip, (f * total).round());
    });
}

fn halt(u: &mut Upload) {
    if let Some(h) = u.playing.take() {
        h.stop();
    }
    u.clock = None;
}

fn stop_playback() {
    with(halt);
}

fn start(u: &mut Upload, volume: f32) {
    let Some(pcm) = u.file.as_ref().and_then(|f| f.pcm.clone()) else { return };
    let from = if u.trim.playhead_ms >= u.trim.end_ms { u.trim.start_ms } else { u.trim.playhead_ms };
    let clip = Arc::new(trimmed(&pcm, from, u.trim.end_ms));
    u.trim.playhead_ms = from;
    u.playing = crate::sounds::play_pcm(clip, volume);
    u.clock = Some((std::time::Instant::now(), from));
}

fn toggle_play() {
    with(|u| {
        if u.clock.is_some() {
            halt(u);
            return;
        }
        let v = u.volume.min(1.);
        start(u, v);
    });
}

pub fn play_silent_from(ms: f64) {
    with(|u| {
        halt(u);
        u.trim.playhead_ms = ms.clamp(u.trim.start_ms, u.trim.end_ms);
        start(u, 0.);
    });
}

pub fn advance(trim: Trim, from: f64, elapsed_ms: f64) -> (Trim, bool) {
    let pos = from + elapsed_ms;
    if pos < trim.end_ms {
        (Trim { playhead_ms: pos.max(trim.playhead_ms), ..trim }, true)
    } else {
        (Trim { playhead_ms: trim.start_ms, ..trim }, false)
    }
}

fn tick() -> bool {
    with(|u| {
        let Some((t0, from)) = u.clock else { return false };
        let (trim, on) = advance(u.trim, from, t0.elapsed().as_secs_f64() * 1000.);
        u.trim = trim;
        if !on {
            halt(u);
        }
        on
    })
    .unwrap_or(false)
}

fn waveform_canvas(peaks: Vec<f32>) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            if peaks.is_empty() {
                return;
            }
            let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
            let e = w / peaks.len() as f32;
            let a = e * 0.5;
            let col = t(idx::ICON_STRONG);
            for (i, p) in peaks.iter().enumerate() {
                let s = p * h;
                if !s.is_finite() {
                    continue;
                }
                let x = i as f32 * e + a;
                let y = h / 2. - s / 2.;
                window.paint_quad(gpui::fill(Bounds::new(gpui::point(b.origin.x + gpui::px(x), b.origin.y + gpui::px(y)), gpui::size(gpui::px((e - a).max(0.)), gpui::px(s))), col));
            }
        },
    )
    .absolute()
    .top_0()
    .bottom_0()
    .left(z(HANDLE_W))
    .right(z(HANDLE_W))
    .into_any_element()
}

fn handle_bar(start: bool) -> gpui::Div {
    let pill = div()
        .w(z(6.))
        .h(z(20.))
        .rounded(z(50.))
        .bg(t(idx::INTERACTIVE_TEXT_ACTIVE))
        .flex()
        .items_center()
        .justify_center()
        .child(icon(if start { "chevron-right" } else { "chevron-small-left" }, 16., idx::BACKGROUND_MOD_NORMAL).mx(z(-2.)).flex_none());
    let d = div().absolute().top_0().h(z(TRIMMER_H)).w(z(2.)).rounded(z(10.)).bg(t(idx::INTERACTIVE_TEXT_ACTIVE)).flex().items_center().justify_center().child(pill);
    if start { d.left(z(-2.)) } else { d.right(z(-2.)) }
}

fn trimmer(u: &Upload, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(file) = u.file.as_ref() else { return div().into_any_element() };
    let total = file.duration_ms;
    let tr = u.trim;
    let len_ms = tr.end_ms - tr.start_ms;
    let warn = len_ms > SOUND_MAX_MS;
    let playing = u.clock.is_some();
    let weak = cx.entity().downgrade();
    let probe = canvas(
        |b, _, _| BOUNDS.with(|c| c.set(Some(b))),
        move |_, _, window, _| {
            if playing {
                window.request_animation_frame();
            }
            if snapshot(|u| u.drag.is_none()).unwrap_or(true) {
                return;
            }
            let w1 = weak.clone();
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                apply_drag(ev.position.x);
                let _ = w1.update(cx, |_, cx| cx.notify());
            });
            let w2 = weak.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                with(|u| u.drag = None);
                let _ = w2.update(cx, |_, cx| cx.notify());
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let shade = |left: bool| {
        let d = div().absolute().top_0().bottom_0().bg(t(idx::BACKGROUND_BASE_LOWER)).opacity(0.7);
        if left { d.left(z(-9999.)).right(relative(1.)) } else { d.left(relative(1.)).right(z(-9999.)) }
    };
    let frame = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(relative(left_pct(tr.start_ms, total) / 100.))
        .right(relative(right_pct(tr.end_ms, total) / 100.))
        .child(div().absolute().top_0().bottom_0().left_0().w_full().child(shade(true)).child(shade(false)))
        .child(handle_bar(true))
        .child(handle_bar(false));
    let playhead = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(relative(left_pct(tr.playhead_ms, total) / 100.))
        .w(z(6.))
        .ml(z(-6.))
        .child(div().absolute().top(z(4.)).bottom(z(4.)).left(z(3.)).w(z(3.)).rounded(z(50.)).bg(t(idx::BRAND_500)));
    let timeline = div()
        .id("eu-timeline")
        .relative()
        .flex_1()
        .h(z(TRIMMER_H))
        .overflow_hidden()
        .cursor(gpui::CursorStyle::ResizeLeftRight)
        .child(probe)
        .child(waveform_canvas(file.peaks.clone()))
        .child(div().absolute().top_0().bottom_0().left(z(HANDLE_W)).right(z(HANDLE_W)).child(frame).child(playhead))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_, ev: &MouseDownEvent, _, cx| {
                stop_playback();
                if let (Some(f), Some(b)) = (frac_at(ev.position.x), BOUNDS.with(Cell::get)) {
                    let w = f32::from(b.size.width) - 2. * HANDLE_W;
                    with(|u| u.drag = Some(grip_for(u.trim, total, f, w)));
                    apply_drag(ev.position.x);
                }
                cx.stop_propagation();
                cx.notify();
            }),
        );
    let play = div()
        .id("eu-play")
        .size(z(32.))
        .rounded_full()
        .bg(t(idx::BACKGROUND_BASE_LOWEST))
        .hover(|s| s.bg(t(idx::BACKGROUND_MOD_NORMAL)))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .child(icon(if playing { "stop" } else { "play" }, 16., idx::INTERACTIVE_TEXT_DEFAULT))
        .on_click(cx.listener(|_, _, _, cx| {
            toggle_play();
            cx.notify();
        }));
    let play_col = div()
        .px(z(4.))
        .h(z(TRIMMER_H))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(play)
        .child(div().h(z(0.)).mt(z(4.)).child(text(12., 16., FontWeight::NORMAL, if warn { idx::TEXT_FEEDBACK_WARNING } else { idx::TEXT_FEEDBACK_POSITIVE }, format!("{:.2}s", len_ms / 1000.))));
    div()
        .h(z(94.))
        .p(z(8.))
        .rounded(z(4.))
        .bg(t(idx::BACKGROUND_BASE_LOWER))
        .border_1()
        .border_color(t(idx::BACKGROUND_BASE_LOWEST))
        .flex()
        .items_center()
        .justify_center()
        .child(div().flex_1().w_full().h(z(TRIMMER_H)).flex().overflow_hidden().child(play_col).child(timeline))
        .into_any_element()
}

fn volume_slider(u: &Upload, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let max = u.max_volume.max(f32::EPSILON);
    let frac = u.volume / max;
    let on: crate::ui::settings::voice::OnSlide = Rc::new(move |_, f, end, _| {
        with(|u| {
            let v = f * u.max_volume;
            u.volume = if end { (100. * v).round() / 100. } else { v };
        });
    });
    crate::ui::settings::voice::slider("eu-volume", frac, volume_label(u.volume, max).into(), None, on, cx).into_any_element()
}

fn sound_modal(u: &Upload, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let g = u.guild;
    let name = clipped(g, "eu-name", SOUND_NAME_MAX, cx);
    let ok = sound_ready(&name, u.file.is_some(), u.file.as_ref().map(|_| u.trim));
    let body = div()
        .id("eu-body")
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(z(16.))
        .pb(z(16.))
        .children(u.error.as_ref().map(|(m, _)| {
            div()
                .p(z(12.))
                .rounded(z(8.))
                .border_1()
                .border_color(t(idx::BORDER_FEEDBACK_CRITICAL))
                .bg(t(idx::BACKGROUND_FEEDBACK_CRITICAL))
                .flex()
                .gap(z(8.))
                .child(icon("circle-error", 20., idx::TEXT_FEEDBACK_CRITICAL).flex_none())
                .child(div().flex_1().min_w_0().child(text(14., 18., FontWeight::NORMAL, idx::TEXT_DEFAULT, m.clone())))
        }))
        .when(u.file.is_some(), |d| d.child(div().child(label(crate::t!("eu.preview"), false)).child(trimmer(u, cx))))
        .child(div().child(label(crate::t!("eu.file"), true)).child(file_input("eu-file", u.file.as_ref().map(|f| f.name.as_str()), cx)))
        .child(
            div()
                .flex()
                .items_end()
                .gap(z(8.))
                .child(div().flex_1().min_w_0().child(label(crate::t!("eu.sound_name"), true)).child(text_input(g, "eu-name", crate::t!("eu.sound_name"), cx)))
                .child(div().flex_1().min_w_0().child(emoji_input(false, u.tag.as_ref(), cx))),
        )
        .child(div().child(label(crate::t!("eu.volume"), false)).child(volume_slider(u, cx)));
    let submit = move |a: &mut PhoenixApp, cx: &mut Context<PhoenixApp>| {
        let name = clipped(g, "eu-name", SOUND_NAME_MAX, cx);
        let Some((Some(file), tag, volume, trim)) = snapshot(|u| (u.file.clone(), u.tag.clone(), u.volume, u.trim)) else { return };
        let Some(pcm) = file.pcm else { return };
        let Ok(ogg) = phoenix_voice::audio::voice_message::encode_sound_ogg(&trimmed(&pcm, trim.start_ms, trim.end_ms)) else {
            with(|u| u.error = Some((crate::t!("eu.processing_failed"), false)));
            return;
        };
        a.blocked_write(sound_write(g, &name, volume, tag.as_ref(), &ogg), cx);
    };
    crate::ui::modals::card_with(MODAL_W, crate::t!("eu.sound_title"), None, body.into_any_element(), Some(footer(Kind::Sound, ok, submit, cx)), close_action(cx))
}

pub fn render(cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    tick();
    let u = snapshot(Upload::clone)?;
    Some(match u.kind {
        Kind::Sticker => sticker_modal(&u, cx),
        Kind::Sound => sound_modal(&u, cx),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba([200, 10, 10, 255]))).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn playhead_advances_then_resets_at_the_end_like_the_client_interval() {
        let tr = Trim { start_ms: 200., end_ms: 1200., playhead_ms: 500. };
        let (a, on) = advance(tr, 500., 300.);
        assert!(on);
        assert_eq!(a.playhead_ms, 800.);
        let (b, on) = advance(tr, 500., 700.);
        assert!(!on);
        assert_eq!(b.playhead_ms, 200.);
        assert_eq!((b.start_ms, b.end_ms), (200., 1200.));
    }

    #[test]
    fn sticker_files_follow_the_client_checks() {
        assert_eq!(sticker_mime(Path::new("a.JPG")), "image/jpeg");
        assert_eq!(sticker_mime(Path::new("a.webp")), "");
        assert_eq!(check_sticker("image/webp", b"x", false), StickerCheck::Unsupported);
        let small = png(320, 320);
        match check_sticker("image/png", &small, false) {
            StickerCheck::Ok { bytes, mime, resized } => {
                assert_eq!((bytes, mime, resized), (small.clone(), "image/png", false));
            }
            other => panic!("{other:?}"),
        }
        let mut jpg = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(640, 320, image::Rgb([1, 2, 3]))).write_to(&mut jpg, image::ImageFormat::Jpeg).unwrap();
        match check_sticker("image/jpeg", jpg.get_ref(), false) {
            StickerCheck::Ok { bytes, mime, resized } => {
                assert!(resized);
                assert_eq!(mime, "image/png");
                let im = image::load_from_memory(&bytes).unwrap();
                assert_eq!((im.width(), im.height()), (320, 320));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(check_sticker("application/json", b"{}", false), StickerCheck::LottieNotAllowed);
        assert!(matches!(check_sticker("application/json", b"{}", true), StickerCheck::Ok { mime: "application/json", .. }));
        let big: Vec<u8> = std::iter::repeat_n(0u8, STICKER_MAX_BYTES + 1).collect();
        assert_eq!(check_sticker("image/gif", &big, false), StickerCheck::TooBig);
    }

    #[test]
    fn apng_is_detected_before_the_first_frame() {
        let mut p = png(4, 4);
        assert!(!is_apng(&p));
        let actl = [0, 0, 0, 8, b'a', b'c', b'T', b'L', 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        let ihdr_end = 8 + 12 + 13;
        p.splice(ihdr_end..ihdr_end, actl);
        assert!(is_apng(&p));
    }

    #[test]
    fn submit_rules_match_the_client() {
        assert!(sticker_ready(false, false, "ab", true, true, ""));
        assert!(!sticker_ready(false, false, "a", true, true, ""));
        assert!(!sticker_ready(false, false, "ab", false, true, ""));
        assert!(!sticker_ready(false, false, "ab", true, true, "x"));
        assert!(!sticker_ready(false, true, "ab", true, true, ""));
        let t = Trim { start_ms: 0., end_ms: 5000., playhead_ms: 0. };
        assert!(sound_ready("ab", true, Some(t)));
        assert!(!sound_ready("ab", true, Some(Trim { end_ms: 5001., ..t })));
        assert!(!sound_ready("ab", true, Some(Trim { end_ms: 0., ..t })));
        assert!(!sound_ready("ab", false, None));
    }

    #[test]
    fn writes_go_to_the_guard() {
        let file = Picked { name: "cat.png".into(), mime: "image/png".into(), bytes: vec![1, 2], original_md5: md5_hex(b"o"), preview: None, lottie: None, pcm: None, duration_ms: 0., peaks: Vec::new() };
        let w = sticker_write(Id(5), "cat hug", &Tag::Custom { id: 42, name: "cat".into(), animated: false }, "", &file);
        assert_eq!((w.method.as_str(), w.path.as_str()), ("POST", "/guilds/5/stickers"));
        let form = w.form.unwrap();
        let names: Vec<&str> = form.parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["name", "tags", "description", "file"]);
        assert_eq!(form.parts[1].data, b"42");
        assert_eq!(form.headers[0].0, "X-Discord-Original-MD5");
        let s = sound_write(Id(5), "ding", 0.854, Some(&Tag::Unicode("🔔".into())), b"OggS");
        assert_eq!(s.path, "/guilds/5/soundboard-sounds");
        let b = s.body.clone().unwrap();
        assert_eq!(b["volume"], serde_json::json!(0.85));
        assert_eq!(b["emoji_name"], "🔔");
        assert_eq!(b["sound"], "data:audio/ogg;base64,T2dnUw==");
        assert!(!crate::actions::guard::read_only_write(&s));
    }

    #[test]
    fn waveform_follows_the_worker() {
        let mut ch = vec![0.1f32; 400];
        ch[200..210].iter_mut().for_each(|x| *x = -0.9);
        let (w, mult) = waveform(&ch, 40.);
        assert_eq!(w.len(), 20);
        assert!((w[10] - 1.).abs() < 1e-6);
        assert!((w[0] - 0.2).abs() < 1e-4);
        assert!((mult - 0.6).abs() < 1e-4);
        let loud = vec![0.6f32; 100];
        assert!((waveform(&loud, 10.).1 - 0.5).abs() < 1e-6);
        assert_eq!(js_number(1e-7), "1e-7");
        assert_eq!(js_number(0.25), "0.25");
    }

    #[test]
    fn trim_and_handles() {
        let pcm = Pcm { rate: 1000, channels: 2, samples: (0..2000).map(|i| i as f32).collect() };
        assert_eq!(duration_ms(&pcm), 1000.);
        let t = trimmed(&pcm, 250., 750.);
        assert_eq!(t.samples.len(), 1000);
        assert_eq!(t.samples[0], 500.);
        assert_eq!(trimmed(&pcm, 0., 1000.).samples.len(), 2000);
        let tr = Trim { start_ms: 0., end_ms: 1000., playhead_ms: 0. };
        assert_eq!(grip_for(tr, 1000., 0.01, 300.), Grip::Start);
        assert_eq!(grip_for(tr, 1000., 0.99, 300.), Grip::End);
        assert_eq!(grip_for(tr, 1000., 0.5, 300.), Grip::Playhead);
        let moved = drag_to(tr, Grip::End, 400.);
        assert_eq!((moved.end_ms, moved.playhead_ms), (400., 0.));
        assert_eq!(drag_to(moved, Grip::Start, 600.).start_ms, 400.);
        assert_eq!(drag_to(moved, Grip::Playhead, 900.).playhead_ms, 400.);
        assert_eq!((left_pct(250., 1000.), right_pct(750., 1000.)), (25., 25.));
        assert_eq!(volume_label(0.3, 0.6), "50%");
    }

    #[test]
    fn modal_state_opens_and_closes() {
        open_sound(Id(9));
        assert!(is_open());
        pick_emoji(Tag::Unicode("🔔".into()));
        assert_eq!(snapshot(|u| u.tag.clone()).flatten(), Some(Tag::Unicode("🔔".into())));
        close();
        assert!(!is_open());
    }
}

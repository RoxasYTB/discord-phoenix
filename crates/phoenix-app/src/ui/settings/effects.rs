use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};

use crate::app::PhoenixApp;

static SPOILERS_ALWAYS: AtomicBool = AtomicBool::new(false);
static HOUR_CYCLE: AtomicU8 = AtomicU8::new(0);
static GIF_AUTOPLAY: AtomicBool = AtomicBool::new(true);
static UNDERLINE_LINKS: AtomicBool = AtomicBool::new(false);
static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);
static INLINE_ATTACHMENTS: AtomicBool = AtomicBool::new(true);
static ALT_TEXT: AtomicBool = AtomicBool::new(false);
static STICKER_MODE: AtomicU8 = AtomicU8::new(0);

pub fn sync(app: &PhoenixApp) {
    let p = &app.prefs;
    let always = match p.synced.pick("spoilers") {
        1 => true,
        2 => {
            use crate::actions::{ADMINISTRATOR, MANAGE_MESSAGES, has};
            has(app, app.selected_guild, MANAGE_MESSAGES) || has(app, app.selected_guild, ADMINISTRATOR)
        }
        _ => false,
    };
    let a = SPOILERS_ALWAYS.swap(always, Relaxed);
    let h = HOUR_CYCLE.swap(p.synced.pick("time-format") as u8, Relaxed);
    let u = UNDERLINE_LINKS.swap(p.local.flag("underline", false), Relaxed);
    if a != spoilers_always() || h != HOUR_CYCLE.load(Relaxed) || u != underline_links() {
        crate::theme::bump_style_generation();
    }
    GIF_AUTOPLAY.store(p.synced.flag("gifs"), Relaxed);
    REDUCED_MOTION.store(p.local.reduced_motion, Relaxed);
    STICKER_MODE.store(p.synced.pick("stickers") as u8, Relaxed);
    INLINE_ATTACHMENTS.store(p.synced.flag("media-uploads"), Relaxed);
    ALT_TEXT.store(p.synced.flag("alt-text"), Relaxed);
}

pub fn spoilers_always() -> bool {
    SPOILERS_ALWAYS.load(Relaxed)
}

pub fn hour24() -> Option<bool> {
    match HOUR_CYCLE.load(Relaxed) {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    }
}

pub fn gif_autoplay() -> bool {
    GIF_AUTOPLAY.load(Relaxed)
}

pub fn motion(ms: u64) -> std::time::Duration {
    std::time::Duration::from_millis(if REDUCED_MOTION.load(Relaxed) { 1 } else { ms })
}

pub fn sticker_mode() -> u8 {
    STICKER_MODE.load(Relaxed)
}

pub fn sticker_autoplay() -> bool {
    sticker_mode() == 0
}

pub fn sticker_hover_plays() -> bool {
    sticker_mode() == 1
}

pub fn inline_attachments() -> bool {
    INLINE_ATTACHMENTS.load(Relaxed)
}

pub fn alt_text() -> bool {
    ALT_TEXT.load(Relaxed)
}

pub fn underline_links() -> bool {
    UNDERLINE_LINKS.load(Relaxed)
}

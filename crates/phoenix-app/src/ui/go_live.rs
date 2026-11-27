use std::cell::Cell;

use gpui::Context;

use crate::app::PhoenixApp;
use crate::voice::stream::{Perks, Quality};
use crate::voice::{StreamSource, VoiceControl};

thread_local! {
    static QUALITY: Cell<Quality> = Cell::new(Quality::default());
    static NOTIFY_FRIENDS: Cell<bool> = const { Cell::new(false) };
}

pub fn notify_friends() -> bool {
    NOTIFY_FRIENDS.with(Cell::get)
}

pub fn set_notify_friends(on: bool) {
    NOTIFY_FRIENDS.with(|c| c.set(on));
}

pub fn notify_shown(app: &PhoenixApp) -> bool {
    let guild = app.voice_channel.and_then(|c| app.channels.get(&c)).and_then(|c| c.guild);
    let members = guild.and_then(|g| app.guilds.iter().find(|x| x.id == g)).and_then(|g| g.member_count);
    crate::voice::stream::notify_option_shown(guild.is_some(), members)
}

pub fn quality() -> Quality {
    QUALITY.with(Cell::get)
}

pub fn set_quality(q: Quality) {
    QUALITY.with(|c| c.set(q));
}

pub fn update_quality(app: &PhoenixApp, q: Quality) {
    set_quality(q);
    if live(app) {
        let (resolution, fps) = crate::voice::stream::resolve(q, perks(app));
        drop(app.backend.voice_control(VoiceControl::UpdateStreamQuality { resolution, fps, sound: q.sound }));
    }
}

pub fn perks(app: &PhoenixApp) -> Perks {
    let guild_tier = app
        .voice_channel
        .and_then(|c| app.channels.get(&c))
        .and_then(|c| c.guild)
        .or(app.selected_guild)
        .and_then(|g| app.guilds.iter().find(|x| x.id == g))
        .map_or(0, |g| g.boost_tier);
    Perks { nitro: app.backend.premium_tier2(), guild_tier }
}

pub fn live(app: &PhoenixApp) -> bool {
    app.voice.own_stream.is_some() || crate::voice::call::own_stream_pending()
}

pub fn start(app: &PhoenixApp, source: StreamSource) {
    let q = quality();
    let (resolution, fps) = crate::voice::stream::resolve(q, perks(app));
    drop(app.backend.voice_control(VoiceControl::StartStream { source, resolution, fps, sound: q.sound }));
}

pub fn stop(app: &PhoenixApp) {
    drop(app.backend.voice_control(VoiceControl::StopStream));
}

pub fn toggle(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if live(app) {
        stop(app);
    } else {
        crate::ui::modals::go_live::open(None);
    }
    cx.notify();
}

pub fn label(app: &PhoenixApp) -> gpui::SharedString {
    if live(app) { crate::t!("voice.stop_share") } else { crate::t!("voice.share_screen") }
}

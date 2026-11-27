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

pub(crate) enum HangupAction {
    Disconnect,
    StopWatching(Vec<String>),
}

impl HangupAction {
    pub(crate) fn current(app: &PhoenixApp) -> Self {
        use crate::ui::call_view::{self, Focus};

        if let Focus::Tile(owner, true) = call_view::focus()
            && owner != app.me.id
            && let Some(key) = call_view::stream_key(app, owner)
            && app.voice.watching.contains(&key)
        {
            return Self::StopWatching(vec![key]);
        }
        let others: Vec<String> = app
            .voice_channel
            .map(|channel| {
                app.voice
                    .watching
                    .iter()
                    .filter(|key| crate::voice::stream::parse_key(key).is_some_and(|stream| stream.channel == channel.0 && stream.owner != app.me.id.0))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if others.is_empty() { Self::Disconnect } else { Self::StopWatching(others) }
    }

    pub(crate) fn disconnects(&self) -> bool {
        matches!(self, Self::Disconnect)
    }

    pub(crate) fn icon(&self) -> gpui::SharedString {
        match self {
            Self::Disconnect => crate::assets::Icon::PhoneHangup.path(),
            Self::StopWatching(_) => crate::icons::path("voice/screen-x"),
        }
    }

    pub(crate) fn label(&self) -> gpui::SharedString {
        match self {
            Self::Disconnect => crate::t!("voice.disconnect"),
            Self::StopWatching(_) => crate::t!("voice.stop_watching"),
        }
    }

    pub(crate) fn run(&self, app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
        match self {
            Self::Disconnect => app.join_voice(None, cx),
            Self::StopWatching(keys) => {
                for key in keys {
                    drop(app.backend.voice_control(VoiceControl::StopWatching(key.clone())));
                }
            }
        }
        cx.notify();
    }
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

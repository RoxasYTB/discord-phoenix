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
    StopSharing,
    StopWatching(String),
    TurnOffCamera,
    HideCamera(crate::model::Id),
}

impl HangupAction {
    pub(crate) fn current(app: &PhoenixApp) -> Self {
        use crate::ui::call_view::{self, Focus};

        if let Focus::Tile(owner, true) = call_view::focus()
            && owner != app.me.id
            && let Some(key) = call_view::stream_key(app, owner)
            && app.voice.watching.contains(&key)
        {
            return Self::StopWatching(key);
        }
        let camera_action = |user| {
            if user == app.me.id { Self::TurnOffCamera } else { Self::HideCamera(user) }
        };
        let users = app.voice_channel.and_then(|ch| app.channels.get(&ch)).map_or(&[][..], |ch| ch.voice_users.as_slice());
        if let Focus::Tile(user, false) = call_view::focus()
            && users.contains(&user)
            && call_view::camera_visible(app, user)
        {
            return camera_action(user);
        }
        if live(app) {
            return Self::StopSharing;
        }
        if let Some(channel) = app.voice_channel
            && let Some(key) = app.voice.watching.iter().find(|key| {
                crate::voice::stream::parse_key(key).is_some_and(|stream| stream.channel == channel.0)
            })
        {
            return Self::StopWatching(key.clone());
        }
        if call_view::camera_visible(app, app.me.id) {
            return Self::TurnOffCamera;
        }
        let mut cameras = users.iter().copied().filter(|&user| call_view::camera_visible(app, user));
        if let Some(user) = cameras.next()
            && cameras.next().is_none()
        {
            return camera_action(user);
        }
        Self::Disconnect
    }

    pub(crate) fn disconnects(&self) -> bool {
        matches!(self, Self::Disconnect)
    }

    pub(crate) fn icon(&self) -> gpui::SharedString {
        match self {
            Self::Disconnect => crate::assets::Icon::PhoneHangup.path(),
            Self::StopSharing | Self::StopWatching(_) => crate::icons::path("voice/screen-x"),
            Self::TurnOffCamera | Self::HideCamera(_) => "icons/call/camera-off.svg".into(),
        }
    }

    pub(crate) fn label(&self) -> gpui::SharedString {
        match self {
            Self::Disconnect => crate::t!("voice.disconnect"),
            Self::StopSharing => crate::t!("voice.stop_share"),
            Self::StopWatching(_) | Self::HideCamera(_) => crate::t!("voice.stop_watching"),
            Self::TurnOffCamera => crate::t!("voice.camera_off"),
        }
    }

    pub(crate) fn run(&self, app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
        match self {
            Self::Disconnect => app.join_voice(None, cx),
            Self::StopSharing => stop(app),
            Self::StopWatching(key) => drop(app.backend.voice_control(VoiceControl::StopWatching(key.clone()))),
            Self::TurnOffCamera => drop(app.backend.voice_control(VoiceControl::SetCamera(false))),
            Self::HideCamera(user) => {
                app.voice.hidden_cameras.insert(*user);
                crate::voice::call::set_hidden_cameras(&app.voice.hidden_cameras);
                crate::ui::call_view::set_focus(crate::ui::call_view::Focus::Grid);
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

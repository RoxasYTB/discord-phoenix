use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use gpui::{AnyElement, App, Bounds, Context, IntoElement, Pixels, Styled, point};

use crate::app::PhoenixApp;
use crate::model::{ChannelKind, Id};
use crate::theme::z;
use crate::ui::context_menu::{Dismiss, MenuItem};
use crate::voice::stream::{self, Preset, Quality};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrayMenu {
    More,
    Streams,
}

thread_local! {
    static OPEN: Cell<Option<TrayMenu>> = const { Cell::new(None) };
    static MORE_AT: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    static SCREEN_AT: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    static NO_VIDEO_HIDDEN: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
    static TOASTS_OFF: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
    static SELF_VIDEO_HIDDEN: Cell<bool> = const { Cell::new(false) };
}

pub const SPACING: f32 = 16.;

pub fn open(m: Option<TrayMenu>) {
    OPEN.with(|c| c.set(m));
}

pub fn opened() -> Option<TrayMenu> {
    OPEN.with(Cell::get)
}

pub fn toggle(m: TrayMenu) {
    open(if opened() == Some(m) { None } else { Some(m) });
}

pub fn probe(menu: TrayMenu) -> impl IntoElement {
    gpui::canvas(
        move |b, _, _| match menu {
            TrayMenu::More => MORE_AT.with(|c| c.set(Some(b))),
            TrayMenu::Streams => SCREEN_AT.with(|c| c.set(Some(b))),
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

pub fn no_video_hidden(channel: Id) -> bool {
    NO_VIDEO_HIDDEN.with(|s| s.borrow().contains(&channel.0))
}

pub fn set_no_video_hidden(channel: Id, hidden: bool) {
    NO_VIDEO_HIDDEN.with(|s| {
        let mut s = s.borrow_mut();
        if hidden {
            s.insert(channel.0);
        } else {
            s.remove(&channel.0);
        }
    });
}

pub fn chat_toasts(channel: Id) -> bool {
    !TOASTS_OFF.with(|s| s.borrow().contains(&channel.0))
}

fn set_chat_toasts(channel: Id, on: bool) {
    TOASTS_OFF.with(|s| {
        let mut s = s.borrow_mut();
        if on {
            s.remove(&channel.0);
        } else {
            s.insert(channel.0);
        }
    });
}

pub fn self_video_hidden() -> bool {
    SELF_VIDEO_HIDDEN.with(Cell::get)
}

pub fn set_self_video_hidden(hidden: bool) {
    SELF_VIDEO_HIDDEN.with(|c| c.set(hidden));
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoreEntry {
    InviteFriends,
    GridView,
    ShowMyCamera,
    ShowNoVideo,
    ChatToasts,
    VoiceSettings,
    Region,
    ReportStream,
    Separator,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct MoreCtx {
    pub stage: bool,
    pub guild_voice: bool,
    pub private: bool,
    pub can_invite: bool,
    pub stream_selected: bool,
    pub remote: bool,
}

pub fn more_layout(c: MoreCtx) -> Vec<MoreEntry> {
    use MoreEntry as M;
    let mut groups: Vec<Vec<MoreEntry>> = Vec::new();
    groups.push(if !c.stage && !c.private && c.can_invite { vec![M::InviteFriends] } else { vec![] });
    let mut view = Vec::new();
    if !c.remote && !c.stage {
        view.push(M::GridView);
    }
    view.push(M::ShowMyCamera);
    if !c.stage {
        view.push(M::ShowNoVideo);
    }
    if c.stage || c.guild_voice {
        view.push(M::ChatToasts);
    }
    groups.push(view);
    let mut settings = vec![M::VoiceSettings];
    if c.private {
        settings.push(M::Region);
    }
    groups.push(settings);
    groups.push(if c.stream_selected { vec![M::ReportStream] } else { vec![] });
    let mut out = Vec::new();
    for g in groups.into_iter().filter(|g| !g.is_empty()) {
        if !out.is_empty() {
            out.push(M::Separator);
        }
        out.extend(g);
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StreamEntry {
    StopStreaming,
    Quality,
    ShareAudio,
    ReportProblem,
    Separator,
}

pub fn streams_layout(live: bool, sound_supported: bool) -> Vec<StreamEntry> {
    use StreamEntry as S;
    if !live {
        return Vec::new();
    }
    let mut out = vec![S::StopStreaming, S::Quality];
    if sound_supported {
        out.push(S::ShareAudio);
    }
    out.push(S::Separator);
    out.push(S::ReportProblem);
    out
}

pub const QUALITY_RESOLUTIONS: [u32; 5] = [480, 720, 1080, 1440, 0];

fn grid_checked(app: &PhoenixApp, channel: Id) -> bool {
    match super::call_view::focus() {
        super::call_view::Focus::Grid => true,
        super::call_view::Focus::Tile(..) => false,
        super::call_view::Focus::Auto => !app
            .channels
            .get(&channel)
            .is_some_and(|c| c.voice_users.iter().any(|u| crate::ui::voice_panel::flags(*u).stream)),
    }
}

fn stream_selected(app: &PhoenixApp, channel: Id) -> bool {
    match super::call_view::focus() {
        super::call_view::Focus::Tile(_, s) => s,
        super::call_view::Focus::Grid => false,
        super::call_view::Focus::Auto => !grid_checked(app, channel),
    }
}

fn context(app: &PhoenixApp, channel: Id) -> MoreCtx {
    let ch = app.channels.get(&channel);
    let guild = ch.and_then(|c| c.guild);
    let perms = guild.map_or(0, |g| app.backend.guild_permissions(g));
    MoreCtx {
        stage: ch.is_some_and(|c| c.kind == ChannelKind::Stage),
        guild_voice: ch.is_some_and(|c| c.kind == ChannelKind::Voice),
        private: guild.is_none(),
        can_invite: guild.is_some() && (crate::ui::inbox::has_perm(perms, 1) || crate::ui::inbox::has_perm(perms, 1 << 3)),
        stream_selected: stream_selected(app, channel),
        remote: false,
    }
}

fn refresh(e: &gpui::Entity<PhoenixApp>, cx: &mut App) {
    e.update(cx, |_, cx| cx.notify());
}

fn more_items(app: &PhoenixApp, channel: Id, cx: &mut Context<PhoenixApp>) -> Vec<MenuItem> {
    use MoreEntry as M;
    let e = cx.entity();
    let guild = app.channels.get(&channel).and_then(|c| c.guild);
    more_layout(context(app, channel))
        .into_iter()
        .map(|entry| {
            let e = e.clone();
            match entry {
                M::InviteFriends => MenuItem::action(crate::t!("call.invite_voice"), Some(crate::icons::path("invite")), move |_, _, cx: &mut App| {
                    if let Some(g) = guild {
                        e.update(cx, |a, cx| a.open_invite_people(g, cx));
                    }
                }),
                M::GridView => {
                    let on = grid_checked(app, channel);
                    MenuItem::check(crate::t!("call.grid_view"), on, move |_, _, cx: &mut App| {
                        super::call_view::set_focus(if on { super::call_view::Focus::Auto } else { super::call_view::Focus::Grid });
                        refresh(&e, cx);
                    })
                }
                M::ShowMyCamera => {
                    let on = !self_video_hidden();
                    MenuItem::check(crate::t!("call.show_my_camera"), on, move |_, _, cx: &mut App| {
                        set_self_video_hidden(on);
                        refresh(&e, cx);
                    })
                }
                M::ShowNoVideo => {
                    let on = !no_video_hidden(channel);
                    MenuItem::check(crate::t!("call.show_no_video"), on, move |_, _, cx: &mut App| {
                        set_no_video_hidden(channel, on);
                        refresh(&e, cx);
                    })
                }
                M::ChatToasts => {
                    let on = chat_toasts(channel);
                    MenuItem::check(crate::t!("call.chat_toasts"), on, move |_, _, cx: &mut App| {
                        set_chat_toasts(channel, !on);
                        refresh(&e, cx);
                    })
                }
                M::VoiceSettings => MenuItem::action(crate::t!("voice.voice_video_settings"), Some(crate::icons::path("gear")), move |_, _, cx: &mut App| {
                    e.update(cx, |a, cx| {
                        crate::ui::settings::open(a, cx);
                        if let Some(s) = a.settings.as_mut() {
                            s.page = crate::ui::settings::Page::VoiceVideo;
                        }
                    })
                }),
                M::Region => MenuItem::submenu(crate::t!("call.region"), vec![MenuItem::radio(crate::t!("call.region_auto"), true, |_, _, _: &mut App| {})])
                    .with_subtext(crate::t!("call.region_auto")),
                M::ReportStream => report_item(e),
                M::Separator => MenuItem::Separator,
            }
        })
        .collect()
}

fn report_item(e: gpui::Entity<PhoenixApp>) -> MenuItem {
    MenuItem::danger(crate::t!("call.report_problem"), Some(crate::icons::path("settings/circle-error")), move |_, _, cx: &mut App| {
        e.update(cx, |a, cx| a.parity_toast(crate::t!("call.report_unavailable"), cx))
    })
}

fn set_quality(e: &gpui::Entity<PhoenixApp>, cx: &mut App, q: Quality) {
    crate::ui::go_live::set_quality(q);
    refresh(e, cx);
}

fn nitro_upsell(e: &gpui::Entity<PhoenixApp>, cx: &mut App) {
    e.update(cx, |a, cx| {
        crate::ui::settings::open(a, cx);
        if let Some(s) = a.settings.as_mut() {
            s.page = crate::ui::settings::Page::Nitro;
        }
        cx.notify();
    });
}

fn streams_items(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Vec<MenuItem> {
    use StreamEntry as S;
    let e = cx.entity();
    let q = crate::ui::go_live::quality();
    let perks = crate::ui::go_live::perks(app);
    let (res, fps) = stream::resolve(q, perks);
    streams_layout(stream_live(app), true)
        .into_iter()
        .map(|entry| {
            let e = e.clone();
            match entry {
                S::StopStreaming => MenuItem::danger(crate::t!("call.stop_streaming"), Some(crate::icons::path("voice/screen-x")), move |_, _, cx: &mut App| {
                    e.update(cx, |a, cx| {
                        crate::ui::go_live::stop(a);
                        cx.notify();
                    })
                }),
                S::Quality => {
                    let mut items = vec![MenuItem::Custom(crate::ui::modals::go_live::group_label(crate::t!("golive.fps")))];
                    for f in stream::FRAME_RATES {
                        let (e, ok) = (e.clone(), stream::allowed(perks, res, f, false));
                        items.push(MenuItem::radio(crate::t!("golive.fps_n", value = f), f == fps, move |_, _, cx: &mut App| {
                            if ok { set_quality(&e, cx, Quality { preset: Preset::Custom, resolution: res, fps: f, ..q }) } else { nitro_upsell(&e, cx) }
                        }));
                    }
                    items.push(MenuItem::Custom(crate::ui::modals::go_live::group_label(crate::t!("call.resolution"))));
                    let fps_for_res = if q.preset == Preset::Documents { 30 } else { fps };
                    for r in QUALITY_RESOLUTIONS {
                        let (e, ok) = (e.clone(), stream::allowed(perks, r, fps_for_res, false));
                        items.push(MenuItem::radio(crate::ui::modals::go_live::resolution_label(r), r == res, move |_, _, cx: &mut App| {
                            if ok { set_quality(&e, cx, Quality { preset: Preset::Custom, resolution: r, fps: fps_for_res, ..q }) } else { nitro_upsell(&e, cx) }
                        }));
                    }
                    MenuItem::submenu(crate::t!("call.stream_quality"), items)
                }
                S::ShareAudio => MenuItem::check(crate::t!("call.share_stream_audio"), q.sound, move |_, _, cx: &mut App| set_quality(&e, cx, Quality { sound: !q.sound, ..q })),
                S::ReportProblem => report_item(e),
                S::Separator => MenuItem::Separator,
            }
        })
        .collect()
}

pub fn stream_live(app: &PhoenixApp) -> bool {
    crate::ui::go_live::live(app) || crate::ui::voice_panel::flags(app.me.id).stream
}

pub fn render(app: &PhoenixApp, channel: Id, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    let menu = opened()?;
    let e = cx.entity();
    let dismiss: Dismiss = Rc::new(move |_, cx| {
        open(None);
        e.update(cx, |_, cx| cx.notify());
    });
    let (items, at, corner) = match menu {
        TrayMenu::More => {
            let b = MORE_AT.with(Cell::get)?;
            (more_items(app, channel, cx), point(b.right(), b.top() - z(SPACING)), gpui::Anchor::BottomRight)
        }
        TrayMenu::Streams => {
            let b = SCREEN_AT.with(Cell::get)?;
            let items = streams_items(app, cx);
            if items.is_empty() {
                return None;
            }
            (items, point(b.center().x, b.top() - z(SPACING)), gpui::Anchor::BottomCenter)
        }
    };
    Some(crate::ui::context_menu::render_anchored(at, corner, items, dismiss))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_menu_follows_the_client_groups() {
        use MoreEntry as M;
        let voice = MoreCtx { guild_voice: true, can_invite: true, ..Default::default() };
        assert_eq!(
            more_layout(voice),
            vec![M::InviteFriends, M::Separator, M::GridView, M::ShowMyCamera, M::ShowNoVideo, M::ChatToasts, M::Separator, M::VoiceSettings]
        );
        let streaming = MoreCtx { stream_selected: true, ..voice };
        assert_eq!(more_layout(streaming).last(), Some(&M::ReportStream));
        let no_invite = MoreCtx { can_invite: false, ..voice };
        assert_eq!(more_layout(no_invite)[0], M::GridView);
        let stage = MoreCtx { stage: true, can_invite: true, ..Default::default() };
        assert_eq!(more_layout(stage), vec![M::ShowMyCamera, M::ChatToasts, M::Separator, M::VoiceSettings]);
        let dm = MoreCtx { private: true, ..Default::default() };
        assert_eq!(more_layout(dm), vec![M::GridView, M::ShowMyCamera, M::ShowNoVideo, M::Separator, M::VoiceSettings, M::Region]);
    }

    #[test]
    fn manage_streams_lists_the_own_stream_options() {
        use StreamEntry as S;
        assert!(streams_layout(false, true).is_empty());
        assert_eq!(streams_layout(true, true), vec![S::StopStreaming, S::Quality, S::ShareAudio, S::Separator, S::ReportProblem]);
        assert_eq!(streams_layout(true, false), vec![S::StopStreaming, S::Quality, S::Separator, S::ReportProblem]);
    }

    #[test]
    fn call_view_toggles_are_local() {
        let ch = Id(9);
        assert!(!no_video_hidden(ch));
        set_no_video_hidden(ch, true);
        assert!(no_video_hidden(ch));
        set_no_video_hidden(ch, false);
        assert!(chat_toasts(ch));
        set_chat_toasts(ch, false);
        assert!(!chat_toasts(ch));
        open(Some(TrayMenu::More));
        toggle(TrayMenu::More);
        assert_eq!(opened(), None);
    }
}

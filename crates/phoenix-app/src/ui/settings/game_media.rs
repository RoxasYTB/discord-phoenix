use gpui::{AnyElement, Context, Div, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    clips,
    keybinds::{self, Action},
    theme::{c, generated::idx, tok, z},
    ui::{
        controls::{self, BtnKind},
        game_overlay as overlay,
    },
};

use super::{
    LocalPrefs, Page,
    pages::{B, Bind},
};

type Cx<'a> = Context<'a, PhoenixApp>;

pub(super) const CLIPS: &[B] = &[
    B::Title("game.clips.capture"),
    B::Custom("game.clips.enable", capture),
    B::Divider,
    B::Title("game.clips.settings"),
    B::Custom("game.clips.duration", clip_settings),
    B::Divider,
    B::Title("game.clips.storage"),
    B::Custom("game.clips.location", storage),
];

pub(super) const OVERLAY: &[B] = &[
    B::Title("game.overlay.overview"),
    B::Custom("game.overlay.enable", overlay_settings),
    B::Divider,
    B::Title("game.overlay.widgets"),
    B::Switch(
        overlay::RIGHT,
        "game.overlay.right",
        "game.overlay.right_d",
        Bind::Local(|p| p.flag(overlay::RIGHT, false), |p, v| set(p, overlay::RIGHT, v)),
    ),
    B::Divider,
    B::Title("game.overlay.voice"),
    B::Switch(
        overlay::VOICE,
        "game.overlay.show_voice",
        "game.overlay.show_voice_d",
        Bind::Local(|p| p.flag(overlay::VOICE, true), |p, v| set(p, overlay::VOICE, v)),
    ),
    B::Switch(
        overlay::SPEAKERS,
        "game.overlay.speakers",
        "game.overlay.speakers_d",
        Bind::Local(|p| p.flag(overlay::SPEAKERS, false), |p, v| set(p, overlay::SPEAKERS, v)),
    ),
    B::Divider,
    B::Title("game.overlay.notifications"),
    B::Switch(
        overlay::NOTIFICATIONS,
        "game.overlay.clip_notifications",
        "game.overlay.clip_notifications_d",
        Bind::Local(|p| p.flag(overlay::NOTIFICATIONS, true), |p, v| set(p, overlay::NOTIFICATIONS, v)),
    ),
];

fn set(prefs: &mut LocalPrefs, id: &str, enabled: bool) {
    prefs.ui.insert(id.to_owned(), u64::from(enabled));
}

fn text(value: impl Into<SharedString>) -> Div {
    div()
        .text_size(z(14.))
        .line_height(z(19.))
        .text_color(c(tok(idx::TEXT_SUBTLE)))
        .child(value.into())
}

fn capture(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    if !cfg!(windows) {
        return text(crate::t!("game.windows_only")).into_any_element();
    }
    let enabled = app.prefs.local.flag(clips::ENABLED, false);
    let mut column = div()
        .flex()
        .flex_col()
        .items_start()
        .gap(z(12.))
        .child(text(crate::t!("game.clips.capture_d")))
        .child(
            controls::button_sm(
                "game-clips-enable",
                crate::t!(if enabled { "game.clips.disable" } else { "game.clips.enable" }),
                BtnKind::Secondary,
            )
            .on_click(cx.listener(move |a, _, _, cx| {
                toggle_feature(a, clips::ENABLED, !enabled, Action::SaveClip, "alt-c", cx);
            })),
        );
    if !enabled {
        return column.into_any_element();
    }
    if let Some(title) = &app.clips.title {
        column = column.child(text(format!("{} {title}", crate::t!("game.clips.active")))).child(
            div()
                .flex()
                .flex_wrap()
                .gap(z(8.))
                .when(!app.clips.saving, |d| {
                    d.child(
                        controls::button_sm("game-clips-save", crate::t!("game.clips.save"), BtnKind::Primary)
                            .on_click(cx.listener(|a, _, _, cx| clips::save(a, cx))),
                    )
                })
                .child(
                    controls::button_sm("game-clips-stop", crate::t!("game.clips.stop"), BtnKind::Secondary).on_click(cx.listener(|a, _, _, cx| {
                        clips::stop(a, cx);
                    })),
                ),
        );
    } else if let Some(settings) = &app.settings {
        if settings.clips_scanning {
            column = column.child(text(crate::t!("common.loading")));
        } else {
            column = column.child(
                controls::button_sm("game-clips-load-sources", crate::t!("game.clips.source"), BtnKind::Primary)
                    .on_click(cx.listener(|a, _, _, cx| scan_sources(a, cx))),
            );
            if let Some(sources) = &settings.clips_sources {
                match sources {
                    Ok(sources) if sources.is_empty() => column = column.child(text(crate::t!("game.clips.no_windows"))),
                    Ok(sources) => {
                        let mut options = vec![crate::t!("game.clips.source_placeholder")];
                        options.extend(sources.iter().map(|source| SharedString::from(source.title.clone())));
                        let sources = sources.clone();
                        column = column.child(super::pages::dropdown(
                            app,
                            "clip-source",
                            options,
                            0,
                            420.,
                            move |a, index, cx| {
                                if let Some(s) = a.settings.as_mut() {
                                    s.open_select = None;
                                }
                                if let Some(source) = index.checked_sub(1).and_then(|index| sources.get(index)) {
                                    clips::start(a, source.clone(), cx);
                                }
                                cx.notify();
                            },
                            cx,
                        ));
                    }
                    Err(error) => column = column.child(text(error.clone())),
                }
            }
        }
    }
    if let Some(message) = &app.clips.message {
        column = column.child(text(message.clone()));
    }
    column.into_any_element()
}

fn scan_sources(app: &mut PhoenixApp, cx: &mut Cx) {
    let Some(settings) = app.settings.as_mut() else { return };
    settings.clips_scanning = true;
    settings.open_select = None;
    let task = cx.background_executor().spawn(async { clips::sources() });
    cx.spawn(async move |app, cx| {
        let result = task.await;
        let _ = app.update(cx, |a, cx| {
            if let Some(s) = a.settings.as_mut().filter(|s| s.clips_scanning) {
                s.clips_sources = Some(result);
                s.clips_scanning = false;
                cx.notify();
            }
        });
    })
    .detach();
    cx.notify();
}

fn toggle_feature(app: &mut PhoenixApp, feature: &str, enabled: bool, action: Action, shortcut: &str, cx: &mut Cx) {
    super::update_local(
        app,
        |prefs| {
            set(prefs, feature, enabled);
            if enabled && !prefs.keybinds.iter().any(|bind| bind.action == action) {
                let used = prefs.keybinds.iter().any(|bind| bind.enabled && bind.shortcut == shortcut);
                keybinds::add(&mut prefs.keybinds);
                let bind = prefs.keybinds.last_mut().expect("binding was appended");
                bind.action = action;
                if !used {
                    bind.shortcut = shortcut.to_owned();
                }
            }
        },
        cx,
    );
}

fn shortcut(app: &PhoenixApp, action: Action, id: &'static str, cx: &mut Cx) -> Div {
    let label = app
        .prefs
        .local
        .keybinds
        .iter()
        .find(|bind| bind.action == action && bind.enabled && !bind.shortcut.is_empty())
        .map(|bind| crate::voice::ptt::label(&bind.shortcut).into())
        .unwrap_or_else(|| crate::t!("game.shortcut_unset"));
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(z(8.))
        .child(text(label))
        .child(controls::button_sm(id, crate::t!("game.shortcut_edit"), BtnKind::Secondary).on_click(super::view::select_page(Page::System, cx)))
}

fn clip_settings(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let duration = app.prefs.local.pick(clips::LENGTH, 0).min(clips::SECONDS.len() - 1);
    let durations = ["game.clips.30", "game.clips.60", "game.clips.120"]
        .into_iter()
        .map(|key| crate::t!(key))
        .collect();
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(z(8.))
        .child(text(crate::t!("game.clips.duration")))
        .child(super::pages::dropdown(
            app,
            "clip-duration",
            durations,
            duration,
            230.,
            |a, index, cx| {
                if let Some(s) = a.settings.as_mut() {
                    s.open_select = None;
                }
                super::update_local(
                    a,
                    |p| {
                        p.ui.insert(clips::LENGTH.into(), index as u64);
                    },
                    cx,
                );
            },
            cx,
        ))
        .child(shortcut(app, Action::SaveClip, "clip-custom-binding", cx))
        .into_any_element()
}

fn storage(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    let folder = clips::folder(app);
    let path_label = folder.to_string_lossy().into_owned();
    let open_folder =
        controls::button_sm("game-clips-folder", crate::t!("game.clips.open_folder"), BtnKind::Secondary).on_click(cx.listener(move |a, _, _, cx| {
            if std::fs::create_dir_all(&folder).is_ok() {
                cx.open_with_system(&folder);
            } else {
                a.parity_toast(crate::t!("game.clips.failed"), cx);
            }
        }));
    let select = controls::button_sm("game-clips-location", crate::t!("game.clips.location"), BtnKind::Secondary).on_click(cx.listener(|_, _, _, cx| {
        let choice = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(crate::t!("game.clips.location")),
        });
        cx.spawn(async move |app, cx| {
            if let Ok(Ok(Some(paths))) = choice.await
                && let Some(path) = paths.into_iter().next().filter(|path| path.is_absolute())
            {
                let _ = app.update(cx, |a, cx| super::update_local(a, |p| p.clips_directory = Some(path), cx));
            }
        })
        .detach();
    }));
    let mut actions = div().flex().flex_wrap().gap(z(8.)).child(select).child(open_folder);
    if let Some(path) = app.clips.last_saved.clone() {
        actions = actions.child(
            controls::button_sm("game-clips-last", crate::t!("game.clips.open_last"), BtnKind::Secondary).on_click(move |_, _, cx| cx.open_with_system(&path)),
        );
    }
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(z(8.))
        .child(text(path_label))
        .child(actions)
        .into_any_element()
}

fn overlay_settings(app: &PhoenixApp, cx: &mut Cx) -> AnyElement {
    if !cfg!(windows) {
        return text(crate::t!("game.windows_only")).into_any_element();
    }
    let enabled = app.prefs.local.flag(overlay::ENABLED, false);
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(z(12.))
        .child(text(crate::t!("game.overlay.overview_d")))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(z(8.))
                .child(
                    controls::button_sm(
                        "game-overlay-enable",
                        crate::t!(if enabled { "game.overlay.disable" } else { "game.overlay.enable" }),
                        BtnKind::Secondary,
                    )
                    .on_click(cx.listener(move |a, _, _, cx| {
                        toggle_feature(a, overlay::ENABLED, !enabled, Action::ToggleOverlay, "ctrl-shift-o", cx);
                    })),
                )
                .when(enabled, |d| {
                    d.child(
                        controls::button_sm(
                            "game-overlay-open",
                            crate::t!(if overlay::is_open() { "game.overlay.hide" } else { "game.overlay.open" }),
                            BtnKind::Primary,
                        )
                        .on_click(cx.listener(|a, _, _, cx| overlay::toggle(a, cx))),
                    )
                }),
        )
        .child(shortcut(app, Action::ToggleOverlay, "overlay-custom-binding", cx))
        .into_any_element()
}

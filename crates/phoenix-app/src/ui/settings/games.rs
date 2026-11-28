use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, prelude::*};

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, tok, z},
    ui::controls::{self, BtnKind},
};

const PICKER: &str = "registered-game-process";

fn running_apps() -> std::io::Result<Vec<String>> {
    #[cfg(windows)]
    let output = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command",
                "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-Process | Where-Object { $_.MainWindowHandle -ne 0 -and $_.ProcessName -notlike 'Phoenix*' -and $_.ProcessName -ne 'discord-phoenix' } | Select-Object -ExpandProperty ProcessName"])
            .creation_flags(0x0800_0000)
            .output()?
    };
    #[cfg(not(windows))]
    let output = {
        let user = std::env::var_os("USER").ok_or_else(|| std::io::Error::other("User unavailable"))?;
        std::process::Command::new("ps").args(["-U"]).arg(user).args(["-o", "comm="]).output()?
    };
    if !output.status.success() {
        return Err(std::io::Error::other("Process list unavailable"));
    }
    let mut apps: Vec<_> = String::from_utf8_lossy(&output.stdout).lines().filter_map(|line| {
        let name = line.trim().rsplit('/').next().unwrap_or_default().trim_start_matches('\u{feff}');
        (!name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)).then(|| name.to_owned())
    }).collect();
    apps.sort_unstable_by_key(|name| name.to_lowercase());
    apps.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Ok(apps)
}

pub(super) fn refresh(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(s) = app.settings.as_mut() else { return };
    if s.scanning_games {
        return;
    }
    s.scanning_games = true;
    s.picks.remove(PICKER);
    let task = cx.background_executor().spawn(async { running_apps().map_err(|_| ()) });
    cx.spawn(async move |app, cx| {
        let apps = task.await;
        let _ = app.update(cx, |a, cx| {
            if let Some(s) = a.settings.as_mut().filter(|s| s.scanning_games) {
                s.running_game_apps = Some(apps);
                s.scanning_games = false;
                cx.notify();
            }
        });
    }).detach();
}

pub(super) fn current(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let Some(s) = app.settings.as_ref() else { return div().into_any_element() };
    let running = s.running_game_apps.as_ref().and_then(|r| r.as_ref().ok());
    let added = &app.prefs.local.registered_games;
    let playing = running.and_then(|apps| apps.iter().find(|name| added.contains(*name)));
    let current = playing.map(|name| name.clone().into()).unwrap_or_else(|| crate::t!("sp.rg.none"));

    let mut card = div().mt(z(10.)).flex().flex_col().gap(z(8.)).p(z(16.)).rounded(z(12.))
        .bg(c(tok(idx::BACKGROUND_MOD_SUBTLE)))
        .child(div().text_size(z(16.)).font_weight(gpui::FontWeight::SEMIBOLD).child(current))
        .child(div().flex().items_center().gap(z(4.)).text_size(z(14.))
            .child(div().text_color(c(tok(idx::TEXT_MUTED))).child(crate::t!("sp.rg.cant_find")))
            .child(div().id("rg-add").cursor_pointer().text_color(c(tok(idx::TEXT_LINK)))
                .child(crate::t!("sp.rg.add"))
                .on_click(cx.listener(|a, _, _, cx| {
                    if let Some(s) = a.settings.as_mut() {
                        s.game_picker_open = true;
                    }
                    refresh(a, cx);
                    cx.notify();
                }))));

    if s.game_picker_open {
        let available: Vec<_> = running.into_iter().flatten().filter(|name| !added.contains(*name)).cloned().collect();
        let selected = s.picks.get(PICKER).copied();
        let close = controls::button_sm("rg-cancel", crate::t!("common.cancel"), BtnKind::Secondary)
            .on_click(cx.listener(|a, _, _, cx| {
                if let Some(s) = a.settings.as_mut() {
                    s.game_picker_open = false;
                    s.open_select = None;
                }
                cx.notify();
            }));
        let mut panel = div().flex().flex_col().gap(z(8.)).items_start();
        if !s.scanning_games && !available.is_empty() {
            let mut options = vec![crate::t!("sp.rg.choose_process")];
            options.extend(available.iter().cloned().map(gpui::SharedString::from));
            let width = (crate::ui::title_bar::viewport_w_css() - 420.).clamp(140., 360.);
            panel = panel.child(super::pages::dropdown(app, PICKER, options, selected.unwrap_or(0), width, |a, i, cx| {
                if let Some(s) = a.settings.as_mut() {
                    s.picks.insert(PICKER, i);
                    s.open_select = None;
                }
                cx.notify();
            }, cx));
        } else {
            panel = panel.child(div().text_size(z(14.)).child(if s.scanning_games {
                crate::t!("common.loading")
            } else if s.running_game_apps.as_ref().is_some_and(Result::is_err) {
                crate::t!("sp.rg.process_error")
            } else {
                crate::t!("sp.rg.no_processes")
            }));
        }
        let name = selected.and_then(|i| i.checked_sub(1)).and_then(|i| available.get(i)).cloned();
        let mut actions = div().flex().items_center().gap(z(8.)).child(close);
        if let Some(name) = name.filter(|_| !s.scanning_games) {
            actions = actions.child(controls::button_sm("rg-save", crate::t!("sp.rg.save_process"), BtnKind::Primary)
                .on_click(cx.listener(move |a, _, _, cx| {
                    a.prefs.local.registered_games.insert(name.clone());
                    a.prefs.local.save();
                    if let Some(s) = a.settings.as_mut() {
                        s.game_picker_open = false;
                        s.open_select = None;
                    }
                    cx.notify();
                })));
        }
        card = card.child(panel.child(actions));
    }
    card.into_any_element()
}

pub(super) fn added(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> AnyElement {
    div().flex().flex_col().children(app.prefs.local.registered_games.iter().enumerate().map(|(i, name)| {
        let process = name.clone();
        div().flex().items_center().justify_between().gap(z(12.)).py(z(8.))
            .border_t_1().border_color(c(tok(idx::BORDER_SUBTLE)))
            .child(div().min_w_0().text_ellipsis().overflow_hidden().child(name.clone()))
            .child(controls::button_sm(("rg-remove", i), crate::t!("sp.rg.remove_process"), BtnKind::Secondary)
                .on_click(cx.listener(move |a, _, _, cx| {
                    a.prefs.local.registered_games.remove(&process);
                    a.prefs.local.save();
                    if let Some(s) = a.settings.as_mut() {
                        s.picks.remove(PICKER);
                    }
                    cx.notify();
                })))
    })).into_any_element()
}

use std::{path::Path, time::Duration};

use gpui::{AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, WeakEntity, div, svg};

use crate::{
    app::PhoenixApp,
    theme::{c, generated::idx, m, tok, z},
    ui::settings::LocalPrefs,
};

pub const TOOLS: &[(&str, &[&str])] = &[
    ("OBS", &["obs/obs.exe", "obs32.exe", "obs64.exe", "OBS.app", "obs"]),
    ("XSplit", &["XSplit.Gamecaster.exe", "XSplit.Core.exe", "Gamecaster.exe"]),
    ("Bebo", &["bebo.exe"]),
    ("Streamlabs Desktop", &["Streamlabs OBS.exe", "Streamlabs Desktop.exe", "Streamlabs Desktop.app"]),
    ("Twitch Studio", &["TwitchStudio.exe", "Twitch Studio.app"]),
];

const POLL: Duration = Duration::from_secs(5);

fn executables() -> impl Iterator<Item = String> {
    TOOLS.iter().flat_map(|(_, exes)| exes.iter()).map(|e| e.to_lowercase())
}

pub fn is_streamer_tool(exe_path: &str) -> bool {
    let lower = exe_path.to_lowercase();
    let base = lower.rsplit('/').next().unwrap_or_default().rsplit('\\').next().unwrap_or_default();
    if executables().any(|e| e == base) {
        return true;
    }
    executables().any(|e| {
        lower.ends_with(&e) && {
            let at = lower.len() - e.len();
            at == 0 || matches!(lower.as_bytes()[at - 1], b'/' | b'\\')
        }
    })
}

fn candidates(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(exe) = std::fs::read_link(dir.join("exe")) {
        out.push(exe.to_string_lossy().trim_end_matches(" (deleted)").to_owned());
    }
    if let Ok(cmd) = std::fs::read(dir.join("cmdline"))
        && let Some(arg0) = cmd.split(|b| *b == 0).next().filter(|a| !a.is_empty())
    {
        out.push(String::from_utf8_lossy(arg0).into_owned());
    }
    out
}

pub fn scan(proc_root: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(proc_root) else { return 0 };
    entries
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())))
        .filter(|e| candidates(&e.path()).iter().any(|c| is_streamer_tool(c)))
        .count()
}

pub fn auto_toggle(prefs: &mut LocalPrefs, count: usize) -> bool {
    if !prefs.flag("streamer-auto", true) {
        return false;
    }
    prefs.ui.insert("streamer".into(), u64::from(count > 0));
    true
}

pub fn enabled(prefs: &LocalPrefs) -> bool {
    prefs.flag("streamer", false)
}

pub fn hides_invites(prefs: &LocalPrefs) -> bool {
    enabled(prefs) && prefs.flag("st-invites", true)
}

pub fn on_tools_changed(app: &mut PhoenixApp, count: usize, cx: &mut Context<PhoenixApp>) {
    let mut next = app.prefs.local.clone();
    if auto_toggle(&mut next, count) && next != app.prefs.local {
        crate::ui::settings::update_local(app, |p| *p = next, cx);
    }
}

pub fn disable(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    crate::ui::settings::update_local(app, |p| {
        p.ui.insert("streamer".into(), 0);
    }, cx);
}

pub const NOTICE_H: f32 = 37.;

pub fn notice(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> Option<AnyElement> {
    if !enabled(&app.prefs.local) {
        return None;
    }
    let white = c(tok(idx::WHITE));
    let close = div()
        .id("streamer-notice-close")
        .absolute()
        .top_0()
        .right_0()
        .size(z(36.))
        .flex()
        .items_center()
        .cursor_pointer()
        .on_click(cx.listener(|a, _, _, cx| disable(a, cx)))
        .child(svg().path(crate::icons::path("close-small")).size(z(18.)).text_color(white));
    let button = div()
        .id("streamer-notice-disable")
        .ml(z(10.))
        .h(z(24.))
        .px(z(10.))
        .flex()
        .items_center()
        .rounded(z(3.))
        .border_1()
        .border_color(white)
        .line_height(z(22.))
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::BACKGROUND_MOD_SUBTLE))))
        .on_click(cx.listener(|a, _, _, cx| disable(a, cx)))
        .child(crate::t!("streamer.disable"));
    Some(
        div()
            .id("streamer-notice")
            .absolute()
            .top_0()
            .right_0()
            .left(z(m::server_bar_w()))
            .h(z(NOTICE_H))
            .pl(z(4.))
            .pr(z(28.))
            .bg(c(tok(idx::PLATFORM_TWITCH)))
            .border_t_1()
            .border_l_1()
            .border_color(c(tok(idx::APP_FRAME_BORDER)))
            .rounded_tl(z(8.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(z(14.))
            .line_height(z(36.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(white)
            .child(close)
            .child(crate::t!("streamer.notice"))
            .child(button)
            .into_any_element(),
    )
}

pub fn watch(app: WeakEntity<PhoenixApp>, cx: &mut App) {
    if !cfg!(target_os = "linux") || std::env::var_os("PHOENIX_NO_STREAMER_SCAN").is_some() {
        return;
    }
    cx.spawn(async move |cx| {
        let mut last = 0;
        loop {
            let count = cx.background_executor().spawn(async { scan(Path::new("/proc")) }).await;
            if count != last {
                last = count;
                if app.update(cx, |a, cx| on_tools_changed(a, count, cx)).is_err() {
                    break;
                }
            }
            cx.background_executor().timer(POLL).await;
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_like_the_running_game_store() {
        assert!(is_streamer_tool("/usr/bin/obs"));
        assert!(is_streamer_tool("/app/bin/obs"));
        assert!(is_streamer_tool("obs"));
        assert!(is_streamer_tool("C:\\Program Files\\XSplit\\XSplit.Core.exe"));
        assert!(is_streamer_tool("/home/u/.wine/drive_c/Program Files/obs-studio/bin/64bit/obs64.exe"));
        assert!(is_streamer_tool("/games/obs/obs.exe"));
        assert!(!is_streamer_tool("Z:\\games\\obs\\obs.exe"));
        assert!(!is_streamer_tool("/usr/bin/obs-browser-page"));
        assert!(!is_streamer_tool("/usr/bin/jobs"));
        assert!(!is_streamer_tool("/usr/bin/spotify"));
        assert!(!is_streamer_tool(""));
    }

    #[test]
    fn scan_counts_matching_processes() {
        let root = std::env::temp_dir().join(format!("phoenix-proc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let proc_dir = |pid: &str, cmd: &[u8]| {
            let d = root.join(pid);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("cmdline"), cmd).unwrap();
            d
        };
        proc_dir("10", b"/usr/bin/obs\0--startreplaybuffer\0");
        proc_dir("11", b"C:\\XSplit\\XSplit.Core.exe\0");
        proc_dir("12", b"/usr/bin/bash\0");
        proc_dir("self", b"/usr/bin/obs\0");
        let exe = proc_dir("13", b"");
        std::os::unix::fs::symlink("/opt/obs/bin/obs", exe.join("exe")).unwrap();
        assert_eq!(scan(&root), 3);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(scan(&root), 0);
    }

    #[test]
    fn auto_toggle_follows_the_tool_count() {
        let mut p = LocalPrefs::default();
        assert!(auto_toggle(&mut p, 1));
        assert!(enabled(&p));
        assert!(hides_invites(&p));
        assert!(auto_toggle(&mut p, 0));
        assert!(!enabled(&p));
        p.ui.insert("streamer-auto".into(), 0);
        assert!(!auto_toggle(&mut p, 2));
        assert!(!enabled(&p));
        p.ui.insert("streamer".into(), 1);
        p.ui.insert("st-invites".into(), 0);
        assert!(!hides_invites(&p));
    }
}

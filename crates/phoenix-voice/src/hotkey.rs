use std::sync::Arc;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use tokio::sync::oneshot;

use crate::audio::AudioControl;

pub const SHORTCUT_ID: &str = "push-to-talk";
pub const APP_ID: &str = "io.github.roxasytb.DiscordPhoenix";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalStatus {
    Bound(String),
    Unavailable(String),
}

pub struct GlobalPtt {
    stop: Option<oneshot::Sender<()>>,
}

impl Drop for GlobalPtt {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
    }
}

pub fn xdg_trigger(keystroke: &str) -> String {
    keystroke
        .split('-')
        .filter(|p| !p.is_empty())
        .map(|p| match p {
            "ctrl" => "CTRL".to_string(),
            "alt" => "ALT".to_string(),
            "shift" => "SHIFT".to_string(),
            "cmd" | "super" | "platform" => "LOGO".to_string(),
            "space" => "space".to_string(),
            k => k.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

pub fn start(
    control: Arc<AudioControl>,
    keystroke: &str,
    status: impl Fn(PortalStatus) + Send + 'static,
) -> GlobalPtt {
    let (tx, rx) = oneshot::channel();
    let trigger = xdg_trigger(keystroke);
    let spawned = std::thread::Builder::new().name("phoenix-ptt-portal".into()).spawn(move || {
        futures_executor::block_on(async move {
            if let Err(e) = run(control.clone(), &trigger, &status, rx).await {
                status(PortalStatus::Unavailable(e.to_string()));
            }
            control.set_ptt_pressed(false);
        })
    });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "push-to-talk portal thread");
    }
    GlobalPtt { stop: Some(tx) }
}

async fn run(
    control: Arc<AudioControl>,
    trigger: &str,
    status: &(impl Fn(PortalStatus) + Send),
    mut stop: oneshot::Receiver<()>,
) -> ashpd::Result<()> {
    let conn = ashpd::zbus::Connection::session().await?;
    if let Ok(id) = APP_ID.parse() {
        if let Err(e) = ashpd::register_host_app_with_connection(conn.clone(), id).await {
            tracing::debug!(error = %e, "portal registry");
        }
    }
    let gs = GlobalShortcuts::with_connection(conn).await?;
    let session = gs.create_session(Default::default()).await?;
    let shortcut = NewShortcut::new(SHORTCUT_ID, "Push to Talk").preferred_trigger(Some(trigger));
    let bound = gs.bind_shortcuts(&session, &[shortcut], None, Default::default()).await?.response()?;
    let desc = bound
        .shortcuts()
        .iter()
        .find(|s| s.id() == SHORTCUT_ID)
        .map(|s| s.trigger_description().to_string())
        .unwrap_or_default();
    status(PortalStatus::Bound(desc));
    let mut down = gs.receive_activated().await?;
    let mut up = gs.receive_deactivated().await?;
    loop {
        tokio::select! {
            _ = &mut stop => break,
            a = down.next() => match a {
                Some(a) if a.shortcut_id() == SHORTCUT_ID => control.set_ptt_pressed(true),
                Some(_) => {}
                None => break,
            },
            d = up.next() => match d {
                Some(d) if d.shortcut_id() == SHORTCUT_ID => control.set_ptt_pressed(false),
                Some(_) => {}
                None => break,
            },
        }
    }
    let _ = session.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn trigger_syntax() {
        assert_eq!(super::xdg_trigger("ctrl-shift-a"), "CTRL+SHIFT+a");
        assert_eq!(super::xdg_trigger("f13"), "f13");
        assert_eq!(super::xdg_trigger("super-space"), "LOGO+space");
    }
}

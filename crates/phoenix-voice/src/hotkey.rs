use std::sync::Arc;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use tokio::sync::oneshot;

use crate::audio::AudioControl;

pub const SHORTCUT_ID: &str = "push-to-talk";
pub const APP_ID: &str = "io.github.roxasytb.DiscordPhoenix";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalStatus {
    Bound(Vec<(String, String)>),
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub id: String,
    pub description: String,
    pub keystroke: String,
}

pub struct GlobalPtt {
    stop: Option<oneshot::Sender<()>>,
    configure: tokio::sync::mpsc::UnboundedSender<()>,
}

impl GlobalPtt {
    pub fn configure(&self) {
        let _ = self.configure.send(());
    }
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

pub fn eligible(keystroke: &str) -> bool {
    let mut keys = keystroke.split('-').filter(|p| !p.is_empty() && !matches!(*p, "ctrl" | "alt" | "shift" | "cmd" | "super" | "platform"));
    keys.next().is_some() && keys.next().is_none()
}

pub fn start(
    control: Arc<AudioControl>,
    bindings: Vec<Binding>,
    on_event: impl Fn(&str, bool) + Send + 'static,
    status: impl Fn(PortalStatus) + Send + 'static,
) -> GlobalPtt {
    let (tx, rx) = oneshot::channel();
    let (ctx, crx) = tokio::sync::mpsc::unbounded_channel();
    let spawned = std::thread::Builder::new().name("phoenix-shortcuts-portal".into()).spawn(move || {
        futures_executor::block_on(async move {
            let handler = |id: &str, down: bool| {
                if id == SHORTCUT_ID {
                    control.set_ptt_pressed(down);
                } else {
                    on_event(id, down);
                }
            };
            if let Err(e) = run(&bindings, &handler, &status, rx, crx).await {
                status(PortalStatus::Unavailable(e.to_string()));
            }
            control.set_ptt_pressed(false);
        })
    });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "global shortcuts portal thread");
    }
    GlobalPtt { stop: Some(tx), configure: ctx }
}

async fn run(
    bindings: &[Binding],
    on_event: &impl Fn(&str, bool),
    status: &(impl Fn(PortalStatus) + Send),
    mut stop: oneshot::Receiver<()>,
    mut configure: tokio::sync::mpsc::UnboundedReceiver<()>,
) -> ashpd::Result<()> {
    let conn = ashpd::zbus::Connection::session().await?;
    if let Ok(id) = APP_ID.parse()
        && let Err(e) = ashpd::register_host_app_with_connection(conn.clone(), id).await
    {
        tracing::debug!(error = %e, "portal registry");
    }
    let gs = GlobalShortcuts::with_connection(conn).await?;
    let session = gs.create_session(Default::default()).await?;
    let wanted: Vec<NewShortcut> = bindings
        .iter()
        .filter(|b| eligible(&b.keystroke))
        .map(|b| NewShortcut::new(b.id.as_str(), b.description.as_str()).preferred_trigger(Some(xdg_trigger(&b.keystroke).as_str())))
        .collect();
    let bound = gs.bind_shortcuts(&session, &wanted, None, Default::default()).await?.response()?;
    status(PortalStatus::Bound(bound.shortcuts().iter().map(|s| (s.id().to_string(), s.trigger_description().to_string())).collect()));
    let mut down = gs.receive_activated().await?;
    let mut up = gs.receive_deactivated().await?;
    loop {
        tokio::select! {
            _ = &mut stop => break,
            Some(()) = configure.recv() => {
                if let Err(e) = gs.configure_shortcuts(&session, None, Default::default()).await {
                    tracing::warn!(error = %e, "configure global shortcuts");
                }
            }
            a = down.next() => match a {
                Some(a) => on_event(a.shortcut_id(), true),
                None => break,
            },
            d = up.next() => match d {
                Some(d) => on_event(d.shortcut_id(), false),
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
        assert!(super::eligible("ctrl-f9"));
        assert!(super::eligible("grave"));
        assert!(!super::eligible("ctrl"));
        assert!(!super::eligible(""));
    }
}

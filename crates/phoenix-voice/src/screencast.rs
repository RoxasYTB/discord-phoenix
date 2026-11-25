use std::os::fd::OwnedFd;

use ashpd::desktop::PersistMode;
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
use ashpd::enumflags2::BitFlags;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Screen,
    Window,
    Any,
}

#[derive(Debug)]
pub struct Picked {
    pub fd: OwnedFd,
    pub node: u32,
    pub size: Option<(u32, u32)>,
    pub session: PortalSession,
}

#[derive(Debug)]
pub struct PortalSession {
    stop: Option<oneshot::Sender<()>>,
}

impl Drop for PortalSession {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
    }
}

pub fn pick(kind: Kind, done: impl FnOnce(Result<Picked, String>) + Send + 'static) {
    let spawned = std::thread::Builder::new().name("phoenix-screencast-portal".into()).spawn(move || {
        futures_executor::block_on(async move {
            let (stop_tx, stop_rx) = oneshot::channel();
            let session = PortalSession { stop: Some(stop_tx) };
            match open(kind).await {
                Ok((proxy, s, fd, node, size)) => {
                    done(Ok(Picked { fd, node, size, session }));
                    let _ = stop_rx.await;
                    let _ = s.close().await;
                    drop(proxy);
                }
                Err(e) => done(Err(e.to_string())),
            }
        })
    });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "screencast portal thread");
    }
}

type Opened = (Screencast, ashpd::desktop::Session<Screencast>, OwnedFd, u32, Option<(u32, u32)>);

async fn open(kind: Kind) -> ashpd::Result<Opened> {
    let conn = ashpd::zbus::Connection::session().await?;
    if let Ok(id) = crate::hotkey::APP_ID.parse()
        && let Err(e) = ashpd::register_host_app_with_connection(conn.clone(), id).await
    {
        tracing::debug!(error = %e, "portal registry");
    }
    let proxy = Screencast::with_connection(conn).await?;
    let session = proxy.create_session(Default::default()).await?;
    let types: BitFlags<SourceType> = match kind {
        Kind::Screen => SourceType::Monitor.into(),
        Kind::Window => SourceType::Window.into(),
        Kind::Any => SourceType::Monitor | SourceType::Window,
    };
    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default().set_cursor_mode(CursorMode::Embedded).set_sources(types).set_multiple(false).set_persist_mode(PersistMode::DoNot),
        )
        .await?;
    let streams = proxy.start(&session, None, Default::default()).await?.response()?;
    let stream = streams.streams().first().ok_or(ashpd::Error::NoResponse)?;
    let node = stream.pipe_wire_node_id();
    let size = stream.size().map(|(w, h)| (w.max(0) as u32, h.max(0) as u32));
    let fd = proxy.open_pipe_wire_remote(&session, Default::default()).await?;
    Ok((proxy, session, fd, node, size))
}

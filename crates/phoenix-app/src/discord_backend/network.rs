use std::sync::Arc;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(super) fn online(state: u32) -> Option<bool> {
    match state {
        50.. => Some(true),
        10 | 20 | 30 => Some(false),
        _ => None,
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) fn watch(_: &Arc<super::Inner>) {}

#[cfg(target_os = "linux")]
pub(super) fn watch(inner: &Arc<super::Inner>) {
    static WATCH: std::sync::Once = std::sync::Once::new();
    WATCH.call_once(|| {
        let inner = inner.clone();
        let _ = std::thread::Builder::new().name("net-watch".into()).spawn(move || {
            let on = move |up: bool| {
                if let Some(s) = inner.session.lock().clone() {
                    let _ = s.gateway.network_changed(up);
                }
            };
            if let Err(e) = zbus::block_on(run(&on)) {
                tracing::info!("no network state notifier: {e}");
            }
        });
    });
}

#[cfg(target_os = "linux")]
async fn run(on: &dyn Fn(bool)) -> zbus::Result<()> {
    use futures::StreamExt;
    let conn = zbus::Connection::system().await?;
    let proxy = zbus::Proxy::new(&conn, "org.freedesktop.NetworkManager", "/org/freedesktop/NetworkManager", "org.freedesktop.NetworkManager").await?;
    let mut changes = proxy.receive_signal("StateChanged").await?;
    let mut last = None;
    while let Some(m) = changes.next().await {
        let Some(up) = m.body().deserialize::<u32>().ok().and_then(online) else { continue };
        if last.replace(up) != Some(up) {
            on(up);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::online;

    #[test]
    fn network_manager_states_map_to_online_and_offline() {
        assert_eq!(online(70), Some(true));
        assert_eq!(online(50), Some(true));
        assert_eq!(online(40), None);
        assert_eq!(online(20), Some(false));
        assert_eq!(online(10), Some(false));
        assert_eq!(online(0), None);
    }
}

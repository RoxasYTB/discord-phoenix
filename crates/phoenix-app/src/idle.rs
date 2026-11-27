pub type OnIdle = Box<dyn FnMut(usize, bool) + Send>;
type Shared = std::sync::Arc<parking_lot::Mutex<OnIdle>>;

pub const LOCK: usize = usize::MAX;

pub fn spawn(timeouts_ms: Vec<u32>, on: OnIdle) {
    if timeouts_ms.is_empty() {
        return;
    }
    let on: Shared = std::sync::Arc::new(parking_lot::Mutex::new(on));
    let locks = on.clone();
    let _ = std::thread::Builder::new().name("idle-watch".into()).spawn(move || {
        if run(timeouts_ms, on).is_none() {
            tracing::info!("no idle notifier on this session, staying online");
        }
    });
    let _ = std::thread::Builder::new().name("lock-watch".into()).spawn(move || {
        if watch_lock(locks).is_none() {
            tracing::info!("no logind session, screen lock is not tracked");
        }
    });
}

#[cfg(target_os = "linux")]
fn run(timeouts_ms: Vec<u32>, on: Shared) -> Option<()> {
    wl::run(timeouts_ms, on)
}

#[cfg(not(target_os = "linux"))]
fn run(_: Vec<u32>, _: Shared) -> Option<()> {
    None
}

#[cfg(target_os = "linux")]
fn watch_lock(on: Shared) -> Option<()> {
    const LOGIND: &str = "org.freedesktop.login1";
    let (_conn, session) = zbus::block_on(async {
        let conn = zbus::Connection::system().await.ok()?;
        let manager = zbus::Proxy::new(&conn, LOGIND, "/org/freedesktop/login1", "org.freedesktop.login1.Manager").await.ok()?;
        let path: zbus::zvariant::OwnedObjectPath = manager.call("GetSession", &("auto",)).await.ok()?;
        let session = zbus::Proxy::new(&conn, LOGIND, path, "org.freedesktop.login1.Session").await.ok()?;
        Some((conn, session))
    })?;
    let mut locked = false;
    loop {
        let now = zbus::block_on(session.get_property::<bool>("LockedHint")).ok()?;
        if let Some(change) = lock_change(&mut locked, now) {
            (*on.lock())(LOCK, change);
        }
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}

#[cfg(not(target_os = "linux"))]
fn watch_lock(_: Shared) -> Option<()> {
    None
}

pub fn lock_change(last: &mut bool, now: bool) -> Option<bool> {
    (*last != now).then(|| {
        *last = now;
        now
    })
}

#[cfg(target_os = "linux")]
mod wl {
    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::{wl_registry, wl_seat::{self, WlSeat}};
    use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
    use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notification_v1::{self, ExtIdleNotificationV1};
    use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::{self, ExtIdleNotifierV1};

    use super::Shared;

    struct State {
        on: Shared,
    }

    impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
        fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
    }

    impl Dispatch<WlSeat, ()> for State {
        fn event(_: &mut Self, _: &WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
    }

    impl Dispatch<ExtIdleNotifierV1, ()> for State {
        fn event(_: &mut Self, _: &ExtIdleNotifierV1, _: ext_idle_notifier_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
    }

    impl Dispatch<ExtIdleNotificationV1, usize> for State {
        fn event(
            state: &mut Self,
            _: &ExtIdleNotificationV1,
            ev: ext_idle_notification_v1::Event,
            which: &usize,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match ev {
                ext_idle_notification_v1::Event::Idled => (*state.on.lock())(*which, true),
                ext_idle_notification_v1::Event::Resumed => (*state.on.lock())(*which, false),
                _ => {}
            }
        }
    }

    pub(super) fn run(timeouts_ms: Vec<u32>, on: Shared) -> Option<()> {
        let conn = Connection::connect_to_env().ok()?;
        let (globals, mut queue) = registry_queue_init::<State>(&conn).ok()?;
        let qh = queue.handle();
        let seat: WlSeat = globals.bind(&qh, 1..=1, ()).ok()?;
        let notifier: ExtIdleNotifierV1 = globals.bind(&qh, 1..=2, ()).ok()?;
        let _watches: Vec<ExtIdleNotificationV1> = timeouts_ms
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if notifier.version() >= 2 {
                    notifier.get_input_idle_notification(*t, &seat, &qh, i)
                } else {
                    notifier.get_idle_notification(*t, &seat, &qh, i)
                }
            })
            .collect();
        let mut state = State { on };
        loop {
            queue.blocking_dispatch(&mut state).ok()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::lock_change;

    #[test]
    fn lock_changes_fire_once_per_transition() {
        let mut last = false;
        assert_eq!(lock_change(&mut last, false), None);
        assert_eq!(lock_change(&mut last, true), Some(true));
        assert_eq!(lock_change(&mut last, true), None);
        assert_eq!(lock_change(&mut last, false), Some(false));
    }
}

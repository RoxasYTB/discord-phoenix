pub type OnIdle = Box<dyn FnMut(usize, bool) + Send>;

pub fn spawn(timeouts_ms: Vec<u32>, on: OnIdle) {
    if timeouts_ms.is_empty() {
        return;
    }
    let _ = std::thread::Builder::new().name("idle-watch".into()).spawn(move || {
        if run(timeouts_ms, on).is_none() {
            tracing::info!("no idle notifier on this session, staying online");
        }
    });
}

#[cfg(target_os = "linux")]
fn run(timeouts_ms: Vec<u32>, on: OnIdle) -> Option<()> {
    wl::run(timeouts_ms, on)
}

#[cfg(not(target_os = "linux"))]
fn run(_: Vec<u32>, _: OnIdle) -> Option<()> {
    None
}

#[cfg(target_os = "linux")]
mod wl {
    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::{wl_registry, wl_seat::{self, WlSeat}};
    use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
    use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notification_v1::{self, ExtIdleNotificationV1};
    use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::{self, ExtIdleNotifierV1};

    use super::OnIdle;

    struct State {
        on: OnIdle,
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
                ext_idle_notification_v1::Event::Idled => (state.on)(*which, true),
                ext_idle_notification_v1::Event::Resumed => (state.on)(*which, false),
                _ => {}
            }
        }
    }

    pub(super) fn run(timeouts_ms: Vec<u32>, on: OnIdle) -> Option<()> {
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

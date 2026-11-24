use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

use futures::{StreamExt, channel::mpsc};
use gpui::App;

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

enum Cmd {
    Open,
    Quit,
}

pub fn spawn(cx: &mut App) {
    if !cfg!(target_os = "linux") || std::env::var_os("PHOENIX_NO_TRAY").is_some() {
        return;
    }
    let (tx, mut rx) = mpsc::unbounded::<Cmd>();
    let labels = (crate::t!("tray.open").to_string(), crate::t!("tray.quit").to_string());
    let icon = pixmap();
    let spawned = std::thread::Builder::new().name("phoenix-tray".into()).spawn(move || {
        if let Err(e) = zbus::block_on(serve(tx, labels, icon)) {
            eprintln!("tray icon unavailable: {e}");
        }
        RUNNING.store(false, Ordering::Relaxed);
    });
    if spawned.is_err() {
        return;
    }
    cx.spawn(async move |cx| {
        while let Some(cmd) = rx.next().await {
            let quit = matches!(cmd, Cmd::Quit);
            cx.update(|cx| {
                if quit {
                    cx.quit();
                    return;
                }
                for w in cx.windows() {
                    let _ = w.update(cx, |_, window, _| window.activate_window());
                }
            });
            if quit {
                break;
            }
        }
    })
    .detach();
}

fn pixmap() -> Vec<(i32, i32, Vec<u8>)> {
    let Ok(img) = image::load_from_memory_with_format(crate::assets::APP_ICON_PNG, image::ImageFormat::Png) else {
        return Vec::new();
    };
    [32u32, 64]
        .iter()
        .map(|&s| {
            let rgba = image::imageops::resize(&img.to_rgba8(), s, s, image::imageops::FilterType::Lanczos3);
            let argb = rgba.pixels().flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
            (s as i32, s as i32, argb)
        })
        .collect()
}

async fn serve(tx: mpsc::UnboundedSender<Cmd>, labels: (String, String), icon: Vec<(i32, i32, Vec<u8>)>) -> zbus::Result<()> {
    let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    let conn = zbus::connection::Builder::session()?
        .name(name.as_str())?
        .serve_at("/StatusNotifierItem", Item { tx: tx.clone(), icon })?
        .serve_at("/MenuBar", Menu { tx, labels })?
        .build()
        .await?;
    conn.call_method(
        Some("org.kde.StatusNotifierWatcher"),
        "/StatusNotifierWatcher",
        Some("org.kde.StatusNotifierWatcher"),
        "RegisterStatusNotifierItem",
        &(name.as_str(),),
    )
    .await?;
    RUNNING.store(true, Ordering::Relaxed);
    std::future::pending::<()>().await;
    Ok(())
}

struct Item {
    tx: mpsc::UnboundedSender<Cmd>,
    icon: Vec<(i32, i32, Vec<u8>)>,
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    fn activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.unbounded_send(Cmd::Open);
    }
    fn secondary_activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.unbounded_send(Cmd::Open);
    }
    fn context_menu(&self, _x: i32, _y: i32) {}
    fn scroll(&self, _delta: i32, _orientation: String) {}
    #[zbus(property)]
    fn category(&self) -> String {
        "Communications".into()
    }
    #[zbus(property)]
    fn id(&self) -> String {
        crate::APP_ID.into()
    }
    #[zbus(property)]
    fn title(&self) -> String {
        "Discord Phoenix".into()
    }
    #[zbus(property)]
    fn status(&self) -> String {
        "Active".into()
    }
    #[zbus(property)]
    fn icon_name(&self) -> String {
        "".into()
    }
    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        self.icon.clone()
    }
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn menu(&self) -> zbus::zvariant::OwnedObjectPath {
        zbus::zvariant::OwnedObjectPath::try_from("/MenuBar").expect("static object path")
    }
}

struct Menu {
    tx: mpsc::UnboundedSender<Cmd>,
    labels: (String, String),
}

const ID_OPEN: i32 = 1;
const ID_SEP: i32 = 2;
const ID_QUIT: i32 = 3;

type Props = HashMap<String, zbus::zvariant::OwnedValue>;

fn props(pairs: &[(&str, zbus::zvariant::Value<'_>)]) -> Props {
    pairs
        .iter()
        .filter_map(|(k, v)| Some(((*k).to_string(), zbus::zvariant::OwnedValue::try_from(v.try_clone().ok()?).ok()?)))
        .collect()
}

impl Menu {
    fn item_props(&self, id: i32) -> Props {
        use zbus::zvariant::Value;
        match id {
            ID_OPEN => props(&[("label", Value::from(self.labels.0.as_str()))]),
            ID_SEP => props(&[("type", Value::from("separator"))]),
            ID_QUIT => props(&[("label", Value::from(self.labels.1.as_str()))]),
            _ => props(&[("children-display", Value::from("submenu"))]),
        }
    }
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn get_layout(
        &self,
        _parent_id: i32,
        _recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> (u32, (i32, Props, Vec<zbus::zvariant::OwnedValue>)) {
        let children = [ID_OPEN, ID_SEP, ID_QUIT]
            .into_iter()
            .filter_map(|id| {
                let node = (id, self.item_props(id), Vec::<zbus::zvariant::OwnedValue>::new());
                zbus::zvariant::OwnedValue::try_from(zbus::zvariant::Value::from(zbus::zvariant::Structure::from(node))).ok()
            })
            .collect();
        (1, (0, self.item_props(0), children))
    }

    fn get_group_properties(&self, ids: Vec<i32>, _property_names: Vec<String>) -> Vec<(i32, Props)> {
        ids.into_iter().map(|id| (id, self.item_props(id))).collect()
    }

    fn event(&self, id: i32, event_id: String, _data: zbus::zvariant::OwnedValue, _timestamp: u32) {
        if event_id != "clicked" {
            return;
        }
        let cmd = match id {
            ID_OPEN => Cmd::Open,
            ID_QUIT => Cmd::Quit,
            _ => return,
        };
        let _ = self.tx.unbounded_send(cmd);
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }
    #[zbus(property)]
    fn status(&self) -> String {
        "normal".into()
    }
    #[zbus(property)]
    fn text_direction(&self) -> String {
        "ltr".into()
    }
}

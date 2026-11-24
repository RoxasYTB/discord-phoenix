use std::{cell::RefCell, collections::BTreeSet};

use crate::{
    app::{PhoenixApp, SidebarRow},
    model::Id,
};

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Pins {
    pub pinned: BTreeSet<u64>,
    pub favorites: BTreeSet<u64>,
    pub show_all: BTreeSet<u64>,
}

impl Pins {
    pub fn parse(s: &str) -> Self {
        let mut p = Self::default();
        for line in s.lines() {
            let mut it = line.split_whitespace();
            let set = match it.next() {
                Some("pinned") => &mut p.pinned,
                Some("favorites") => &mut p.favorites,
                Some("show_all") => &mut p.show_all,
                _ => continue,
            };
            set.extend(it.filter_map(|x| x.parse::<u64>().ok()));
        }
        p
    }

    pub fn serialize(&self) -> String {
        let line = |k: &str, s: &BTreeSet<u64>| {
            let v: Vec<String> = s.iter().map(u64::to_string).collect();
            format!("{k} {}\n", v.join(" "))
        };
        format!("{}{}{}", line("pinned", &self.pinned), line("favorites", &self.favorites), line("show_all", &self.show_all))
    }

    fn path() -> Option<std::path::PathBuf> {
        crate::ui::home::data::NavPrefs::path().map(|p| p.with_file_name("menu-pins.txt"))
    }
}

thread_local! {
    static PINS: RefCell<Option<Pins>> = const { RefCell::new(None) };
}

pub fn with<R>(f: impl FnOnce(&mut Pins) -> R) -> R {
    PINS.with(|p| {
        let mut p = p.borrow_mut();
        let pins = p.get_or_insert_with(|| {
            if cfg!(test) {
                return Pins::default();
            }
            Pins::path().and_then(|p| std::fs::read_to_string(p).ok()).map(|s| Pins::parse(&s)).unwrap_or_default()
        });
        f(pins)
    })
}

fn save() {
    if cfg!(test) {
        return;
    }
    let s = with(|p| p.serialize());
    if let Some(path) = Pins::path() {
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(path, s);
    }
}

pub fn is_pinned(id: Id) -> bool {
    with(|p| p.pinned.contains(&id.0))
}

pub fn is_favorite(id: Id) -> bool {
    with(|p| p.favorites.contains(&id.0))
}

pub fn shows_all(guild: Id) -> bool {
    with(|p| p.show_all.contains(&guild.0))
}

fn toggle(set: impl FnOnce(&mut Pins) -> &mut BTreeSet<u64>, id: Id) {
    with(|p| {
        let s = set(p);
        if !s.remove(&id.0) {
            s.insert(id.0);
        }
    });
    save();
}

pub fn toggle_pin(id: Id) {
    toggle(|p| &mut p.pinned, id)
}

pub fn toggle_favorite(id: Id) {
    toggle(|p| &mut p.favorites, id)
}

pub fn toggle_show_all(guild: Id) {
    toggle(|p| &mut p.show_all, guild)
}

pub fn apply(rows: &mut Vec<SidebarRow>) {
    let pinned = |r: &SidebarRow| matches!(r, SidebarRow::Channel(id) if is_pinned(*id));
    if !rows.iter().any(pinned) {
        return;
    }
    let (mut top, mut rest) = (Vec::new(), Vec::new());
    let mut lifting = false;
    for r in rows.drain(..) {
        match r {
            SidebarRow::Thread(_) | SidebarRow::VoiceUser { .. } | SidebarRow::StageAudience(_) => {}
            _ => lifting = pinned(&r),
        }
        if lifting { top.push(r) } else { rest.push(r) }
    }
    top.extend(rest);
    *rows = top;
}

impl PhoenixApp {
    pub fn toggle_channel_pin(&mut self, id: Id, cx: &mut gpui::Context<Self>) {
        toggle_pin(id);
        self.rebuild_sidebar();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut p = Pins::default();
        p.pinned.insert(5);
        p.favorites.extend([1, 2]);
        assert_eq!(Pins::parse(&p.serialize()), p);
    }

    #[test]
    fn pinned_rows_first() {
        toggle_pin(Id(3));
        let mut rows = vec![SidebarRow::Channel(Id(1)), SidebarRow::Category(Id(9)), SidebarRow::Channel(Id(3))];
        apply(&mut rows);
        assert!(matches!(rows[0], SidebarRow::Channel(Id(3))));
        assert_eq!(rows.len(), 3);
        toggle_pin(Id(3));
    }
}

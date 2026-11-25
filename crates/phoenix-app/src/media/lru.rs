use std::{cell::RefCell, collections::HashMap, sync::Arc};

use gpui::{App, AppContext, Entity, Global, ImageCache, ImageCacheError, ImageCacheItem, RenderImage, Resource, WeakEntity, Window, hash};

pub const BUDGET_BYTES: u64 = 128 * 1024 * 1024;
const KEEP_RECENT: u64 = 256;

pub struct LruImageCache {
    items: HashMap<u64, (ImageCacheItem, u64)>,
    tick: u64,
    budget: u64,
}

fn bytes_of(img: &RenderImage) -> u64 {
    (0..img.frame_count())
        .map(|i| {
            let s = img.size(i);
            s.width.0.max(0) as u64 * s.height.0.max(0) as u64 * 4
        })
        .sum()
}

impl LruImageCache {
    pub fn new(budget: u64, cx: &mut App) -> Entity<Self> {
        let e = cx.new(|_| Self { items: HashMap::new(), tick: 0, budget });
        cx.observe_release(&e, |c, cx| {
            for (_, (item, _)) in c.items.drain() {
                if let Some(Ok(img)) = item.get() {
                    cx.drop_image(img, None);
                }
            }
        })
        .detach();
        e
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn evict(&mut self, protected: u64, window: &mut Window, cx: &mut App) {
        let mut entries: Vec<(u64, u64, u64)> = self
            .items
            .iter()
            .filter_map(|(k, (item, t))| {
                let bytes = item.get()?.ok().map(|img| bytes_of(&img))?;
                Some((*t, *k, bytes))
            })
            .collect();
        let victims = select_evictions(&mut entries, self.budget, self.tick, KEEP_RECENT, protected);
        for k in victims {
            if let Some((item, _)) = self.items.remove(&k)
                && let Some(Ok(img)) = item.get()
            {
                cx.drop_image(img, Some(window));
            }
        }
    }
}

fn select_evictions(entries: &mut [(u64, u64, u64)], budget: u64, tick: u64, keep_recent: u64, protected: u64) -> Vec<u64> {
    let mut used: u64 = entries.iter().map(|(_, _, b)| *b).sum();
    if used <= budget {
        return Vec::new();
    }
    entries.sort_unstable_by_key(|(t, k, _)| (*t, *k));
    let fresh = tick.saturating_sub(keep_recent);
    let target = budget * 9 / 10;
    let mut victims = Vec::new();
    for &(t, k, bytes) in entries.iter() {
        if used <= target || t >= fresh {
            break;
        }
        if k == protected {
            continue;
        }
        used = used.saturating_sub(bytes);
        victims.push(k);
    }
    if used > budget {
        for &(t, k, bytes) in entries.iter() {
            if used <= budget {
                break;
            }
            if t < fresh || k == protected {
                continue;
            }
            used = used.saturating_sub(bytes);
            victims.push(k);
        }
    }
    victims
}

impl ImageCache for LruImageCache {
    fn load(&mut self, resource: &Resource, window: &mut Window, cx: &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        self.tick += 1;
        let tick = self.tick;
        let key = hash(resource);
        let inserted = !self.items.contains_key(&key);
        let entry = self.items.entry(key).or_insert_with(|| (ImageCacheItem::new(resource, cx), tick));
        entry.1 = tick;
        let out = entry.0.use_image(window);
        if (inserted && self.items.len() >= 4) || self.tick.is_multiple_of(16) {
            self.evict(key, window, cx);
        }
        out
    }
}

struct ChatCache(#[allow(dead_code)] Entity<LruImageCache>);
impl Global for ChatCache {}

thread_local! {
    static CHAT: RefCell<Option<WeakEntity<LruImageCache>>> = const { RefCell::new(None) };
}

pub fn init(cx: &mut App) {
    let c = LruImageCache::new(BUDGET_BYTES, cx);
    CHAT.with_borrow_mut(|s| *s = Some(c.downgrade()));
    cx.set_global(ChatCache(c));
}

pub fn chat() -> Option<Entity<LruImageCache>> {
    CHAT.with_borrow(|s| s.as_ref()?.upgrade())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_oldest_first_when_over_budget() {
        let mut entries = vec![(1, 10, 50), (2, 20, 50), (3, 30, 50)];
        let victims = select_evictions(&mut entries, 100, 500, 100, 30);
        assert_eq!(victims, vec![10, 20]);
    }

    #[test]
    fn enforces_strict_budget_on_recent_burst() {
        let mut entries = vec![(100, 1, 60), (101, 2, 60), (102, 3, 60)];
        let victims = select_evictions(&mut entries, 100, 102, 256, 3);
        assert_eq!(victims, vec![1, 2]);
    }

    #[test]
    fn keeps_everything_under_budget() {
        let mut entries = vec![(1, 10, 40), (2, 20, 40)];
        assert!(select_evictions(&mut entries, 100, 500, 100, 20).is_empty());
    }
}

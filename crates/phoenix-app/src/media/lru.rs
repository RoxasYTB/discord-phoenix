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

    fn used(&self) -> u64 {
        self.items.values().filter_map(|(i, _)| i.get()?.ok()).map(|i| bytes_of(&i)).sum()
    }

    fn evict(&mut self, window: &mut Window, cx: &mut App) {
        let mut used = self.used();
        if used <= self.budget {
            return;
        }
        let mut order: Vec<(u64, u64)> = self.items.iter().map(|(k, (_, t))| (*t, *k)).collect();
        order.sort_unstable();
        let fresh = self.tick.saturating_sub(KEEP_RECENT);
        for (t, k) in order {
            if used <= self.budget * 9 / 10 || t >= fresh {
                break;
            }
            if let Some((item, _)) = self.items.remove(&k)
                && let Some(Ok(img)) = item.get()
            {
                used = used.saturating_sub(bytes_of(&img));
                cx.drop_image(img, Some(window));
            }
        }
    }
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
        if inserted && self.items.len().is_multiple_of(32) {
            self.evict(window, cx);
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

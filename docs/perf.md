# Performance: start-up and message loading

Goal: instant start and instant channel switches. The UI is drawn from the
disk cache before the network answers, and message pages are served from
memory or disk.

## How to measure

`PHX_TIMINGS=1` makes the app and the examples print `PHX_TIMING t=<ms since
process start> <event> [<duration>ms]` on stderr:

| event | what it times |
|---|---|
| `main_window_opened`, `first_frame` | app start → window built / first frame (app binary only) |
| `connect_start` | token loaded from the keyring, connect begins |
| `rest_get_me` | token check (GET /users/@me), from `connect_start` |
| `gateway_ready` | `connect_start` → READY parsed |
| `ready_built` | `connect_start` → READY converted to UI data |
| `snapshot_loaded` / `snapshot_applied` | READY snapshot read from disk / drawn |
| `ready_applied` | live READY applied to the UI |
| `rest_messages_latest` / `rest_messages_older` | GET /channels/{id}/messages |
| `messages_cache_fresh` / `messages_cache_stale` | page served from the message cache |
| `channel_messages_ready[load\|memory]` | channel opened → messages in app state |
| `channel_switch_frame` | channel opened → first frame rendered with them |
| `media_disk` / `media_net` | avatar / emoji / attachment GET, disk cache hit or network |

Real account, read-only, offscreen (no window, no acks):

```
PHX_TIMINGS=1 PHX_SCALE=1 PHX_W=1280 PHX_H=800 PHX_THEME=dark PHX_AUDIO=00 PHX_WAIT=12 \
  int-bin/real_shots out.png 2>&1 | grep PHX_TIMING
```

Without `PHX_CHANNEL` and with no CDP, it opens the default channel of the
first guild. Some of the time before `connect_start` is the example's own
CDP probes, not the app.

## Baseline (fbb7a46, 2026-11-24, before any cache)

Two cold runs, warm media disk cache.

| step | run 1 | run 2 |
|---|---|---|
| process start → `connect_start` (keyring + example setup) | 2803 ms | 2355 ms |
| `rest_get_me` | 227 ms | 209 ms |
| connect → READY | 1614 ms | 1348 ms |
| READY → UI data | 27 ms | 31 ms |
| process start → READY in UI | ~4440 ms | ~3735 ms |
| GET latest messages | 212 ms | 252 ms |
| channel opened → messages in state | 756 ms | 553 ms |
| channel opened → frame | 756 ms | 553 ms |
| avatars/emoji (104 loads) | all disk hits, < 0.2 ms each | same |

Reading: the user waits ~1.6 s on the gateway and ~0.25 s on the token check
before anything shows, then another ~0.5–0.75 s for the first channel. Media
is not the bottleneck (disk cache is hot; decoding already runs in GPUI's
background image loader).

## Changes and their effect

See the table at the bottom; each row is one commit, measured the same way.

| commit | change | process start → guilds drawn | channel opened → frame |
|---|---|---|---|
| fbb7a46 | baseline | ~3.7–4.4 s | 553–756 ms |
| 510d303 + c0dbb86 | READY snapshot on disk, message page cache (memory LRU 64 + disk), hover prefetch, warm 6 recent channels after READY; token check during the handshake | ~2.1–2.3 s (snapshot drawn ~270 ms after the app starts connecting) | 91–97 ms from the disk page, 8–9 ms from memory |

### After the cache (a10c2a5, three runs)

| step | run 1 | run 2 | run 3 |
|---|---|---|---|
| `connect_start` → `snapshot_loaded` (read + parse READY off-thread) | 181 ms | 202 ms | 186 ms |
| `connect_start` → `snapshot_applied` (guilds, channels, DMs on screen) | 305 ms | 303 ms | 293 ms |
| cached channel → messages in state | 91 ms | 97 ms | 95 ms |
| connect → live READY (network; unchanged) | 3103 ms | 1198 ms | 1234 ms |
| live READY applied, selection kept, page from memory → frame | 9 ms | 8.5 ms | 9 ms |
| `rest_get_me` (now overlaps the handshake) | 207 ms | 198 ms | 199 ms |

The ~1.8 s before `connect_start` in real_shots is the example's CDP probes
(the user's Discord had no CDP port during these runs), not the app.

### Round 2 (649899d, two runs)

Pre-READY history over REST, scroll kept on refresh, unread-mention warm-up
(`warm_channels 7` = 8 max mentions + 6 recent, deduped), snapshot parsed on a
thread at backend creation.

| step | run 1 | run 2 |
|---|---|---|
| snapshot read (disk) | 3.4 ms | 6.8 ms |
| snapshot JSON parse | 148 ms | 198 ms |
| snapshot → UI data | 54 ms | 65 ms |
| `connect_start` → `snapshot_applied` | **112 ms** (was ~300) | **116 ms** |
| cached channel → frame (cold UI) | 150 ms | 167 ms |
| after live READY, page from memory → frame | 13.7 ms | 11.3 ms |

The preload finishes before the window in real_shots; in the app the parse
(~150–200 ms) overlaps window creation instead of following it.

### Round 3: RESUME across launches, off-thread layout (90c930a…79bcfec)

Every dispatch after READY is appended to `session/journal.log` by one
writer thread; at launch READY + supplemental + journal are replayed through
the same cache and adapter code, so the state is the one at exit. If the
journal was written less than 150 s ago and its owner process is gone,
the gateway sends RESUME (falls back to IDENTIFY + resync on INVALID_SESSION).
A second Phoenix running at the same time writes nothing and never resumes.

| step | IDENTIFY (READY) | RESUME |
|---|---|---|
| `connect_start` → live session in the UI (`ready_applied`) | 1450–1820 ms | **263–376 ms** |
| of which gateway | 1327–1426 ms (READY) | 0 (session built from disk after `rest_get_me`, ~210–245 ms) |
| snapshot load (read + parse + replay + build) | 238–381 ms, on the preload thread | same |

Message layout off the UI thread, first page of a cached channel → frame
(load average 7–9 during all runs, other agents building):

| variant | runs |
|---|---|
| UI thread (round 2) | 115, 135, 139, 140, 150, 167, 206, 229 ms |
| GPUI background executor | 211, 213, 271 ms; 600–870 ms when it ran during the RESUME burst (queued behind image decodes) |
| dedicated thread per page | 596, 611, 622, 670, 677 ms |

Under contention every thread hop made the first page slower, so the open
channel is laid out inline again; only pages for channels no longer on
screen go off-thread. The 91 → 160 ms "regression" of round 2 matches the
load: same code, 115–229 ms over 8 runs at load 7–9, the cache lookup itself
stays under 1 ms.

## What's next

- The snapshot cost is now the JSON parse (150–200 ms). A compact binary
  snapshot needs serde on the UI model types (`model.rs`, `SharedString`,
  `ImageRef`) and on the nav events: not done.
- First page layout (~100–200 ms cold) is inline; re-measure off-thread
  layout on an idle machine, and look at what is cold the first time
  (it is 8–15 ms warm).
- RESUME: relationships / user settings changes of the previous run come
  from READY in `nav::snapshot`, so they are stale until the next READY.
- Prefetch the channels with unread mentions after READY (now only the 6
  most recently viewed channels and hovered rows).

## Phase 2: Rendering, page layout, and strict LRU memory budget

### Audit findings

1. **Page layout (`history_ui::render_page`)**: cloned three `HashMap`s (`users`, `channels`, `roles`) for every single message in a 50-message page even when `soundboard_sounds` was empty.
2. **Markdown / richtext (`richtext.rs`)**:
   - `emoji_ranges` scanned every character of plain ASCII messages with no `<`.
   - `run_for` called `ui_font()` (acquiring the fallback `RwLock` and cloning `FontFallbacks`) for every span instead of once per inline block.
3. **5,000-message chat scrolling (`ui/chat.rs`, `ui/chat/message_ext.rs`)**:
   - `message()` built the `author_name` element tree (and queried role dot/gradient) on every message even when `!compact && !r.group_start`.
   - `message()` cloned the full `User` struct into `media::set_current_author` on every text-only message with no attachments or embeds.
   - Reply previews recomputed `richtext::preview_with_mentions(&entry.rendered[p].parsed)` on every frame and fell back to an unbounded reverse scan of all `entry.msgs` when no nick was cached.
   - `rich()` called `message_ext::code_ranges(t)` (allocating a `Vec`) on every render just to check if inline code existed; `rich_with_emoji_scaled()` cloned the entire `RichText` to check `is_jumbo`.
   - `embed_markdown()` cloned every `RBlock` on every render.
4. **Image / GIF LRU cache (`media/lru.rs`)**:
   - Eviction only ran when `inserted && items.len().is_multiple_of(32)`, so bursts of 4–31 large decoded GIFs/APNGs/WebPs could bypass eviction entirely.
   - `KEEP_RECENT = 256` prevented evicting recently loaded large animations even when total decoded pixels exceeded the 128 MiB budget.

### Optimisations applied

- **`history_ui::MessagePageResolver`**: borrows `&PageResolver` and `&[SoundboardSound]` per message (zero `HashMap` clones per message).
- **`richtext.rs`**: ASCII fast path in `emoji_ranges`, hoisted `ui_font()` per inline block, zero-clone `is_jumbo_text`, and cached `preview_mentions` on `RenderedMessage`.
- **`ui/chat.rs` & `ui/chat/message_ext.rs`**: lazy `author_name` construction only for `compact || group_start`, skip `media::set_current_author` when no attachments/embeds, reuse cached `preview` + `preview_mentions` in replies with a bounded fallback scan (`take(64)`), zero-alloc `has_code`, and borrow-based `embed_markdown`.
- **`media/lru.rs`**: check eviction on inserts once `len >= 4` or every 16 loads, and add a strict second eviction pass when decoded bytes still exceed `BUDGET_BYTES` (128 MiB) while protecting the currently loaded resource.

### Reproducible 5,000-message benchmark (`int-bin/bench 5000`)

Baseline measured before phase 2 (`int-bin/bench 5000`, headless 1280×800):

| metric | baseline |
|---|---|
| `markdown_ns_per_msg` | 31,819 ns |
| `markdown_allocs_per_msg` | 18.1 |
| `markdown_bytes_per_msg` | 2,102 B |
| `channel_load_ms` (5,000 msgs) | 20,011 ms |
| `scroll_frame_us_p50` | 32,312 µs |
| `scroll_frame_us_p95` | 53,908 µs |
| `scroll_allocs_per_frame` | 10,771 |
| `idle_nocapture_us` | 13,414 µs |
| `idle_nocapture_allocs` | 8,367 |
| `rss_main_view_kib` | 348,988 KiB |
| `rss_after_scroll_kib` | 388,568 KiB |

Unit tests also verify 5,000 synthetic messages through `richtext::build` (`richtext::emoji_tests::builds_five_thousand_synthetic_messages`) and strict LRU budget enforcement (`media::lru::tests`).

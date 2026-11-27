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
| `channel_switch_frame` | channel opened → first frame rendered with them (only frames where the open channel has its page) |
| `page_layout_bg` | first, older or refreshed page laid out on a `phoenix-layout` thread |
| `snapshot_applied` / `ready_applied` | now also give the time spent applying it on the UI thread |
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
- Prefetch the channels with unread mentions after READY. Hovered rows,
  server icons and the 8 most recent cached channels are now preloaded into
  memory (see `docs/OPTIMIZATION.md`, 26/11). `PHX_SWITCH_BENCH=<n>` in
  `real_shots` times server, channel and DM switches (`PHX_SWITCH_HOVER=<ms>`
  hovers first, `PHX_NO_PRELOAD=1` turns the preload off).

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

## Phoenix vs official Discord 1.0.171 (2026-11-25, main ac72344d)

Same machine (8 threads, NVIDIA 580 on Wayland), load average 5–16 during
the runs because other builds were going on. Every Phoenix run went through
`phoenix-nice` (SCHED_IDLE), which changes nothing in the CPU seconds counted
per process. Scripts and raw outputs are kept outside the repo; the method
below is enough to redo them.

### Method

- **Memory, threads, CPU, wakeups**: every process of the tree is summed
  from `/proc/<pid>/smaps_rollup` (Rss, Pss, Private = USS), `/proc/<pid>/stat`
  (utime + stime over 60 s, thread count) and
  `/proc/<pid>/task/*/status` (voluntary + involuntary context switches over
  60 s = wakeups/s). For Discord, the tree is the main process and all its
  children (zygotes, GPU, broker, network, renderer, audio) plus
  `chrome_crashpad_handler`.
- **Discord**: the instance the user had open with CDP (`--remote-debugging-port`),
  logged in, window open, not driven during the runs. Three 60 s samples.
- **Phoenix, real account**: `int-bin/real_shots` (dev profile) offscreen,
  read-only, `PHX_WAIT=125`, sampled from t = 55 s to 115 s, with `HOME`,
  `XDG_CONFIG_HOME` and `XDG_CACHE_HOME` pointed at an empty scratch directory
  (with `.config/discord` linked to the real install for fonts). So there is
  no CDP probe and the user's own session journal is never touched. A first
  run fills that scratch cache (IDENTIFY); the run that was measured is the
  second one, which resumes (RESUME) from it.
- **Phoenix, window**: the published release binary (v0.1.0-beta.1, LTO,
  stripped) with `PHOENIX_MOCK=1 PHX_TIMINGS=1 PHOENIX_BENCH_EXIT_MS=…`
  inside `dbus-run-session -- kwin_wayland --virtual --width 1280 --height 800`.
  The window lives in a private compositor: nothing appears on the user's
  screen and the account is not touched.
- **Scroll**: `bench 5000` (MockBackend, headless 1280×800), built at
  ac72344d. Each binary was run twice, with runs alternating
  new/old/new/old.

### Side by side

| | Discord 1.0.171 (Electron) | Phoenix, real account (dev build, headless) | Phoenix, window (release, mock data) |
|---|---|---|---|
| processes | 9 + crashpad | 1 | 1 |
| threads | 135 + 3 | 18 | 23 |
| RSS (sum) | 1,427–1,504 MiB | 423 MiB | 287 MiB |
| PSS (sum) | 880–963 MiB | **342 MiB** | **202 MiB** |
| USS (private) | 699–783 MiB | 328 MiB | 187 MiB |
| CPU at rest, 60 s | 8.2 / 8.4 / 11.5 % of one core (13 % and 51 % in two earlier busy samples) | 4.1 % (5.1 % on the cold run, media still loading) | **0.45 %** |
| wakeups/s at rest | 243–303 (1,819 in the busy sample) | 186 | 127, of which 100 come from one NVIDIA driver thread (`libnvidia-eglcore`) |
| on disk | 560 MB install (`Discord` 208 MB, `modules` 293 MB) + 708 MB profile (`Cache` 175 MB) | — | **55.9 MB** binary (23.9 MB tar.gz) |

For memory, PSS is the number to compare: the shared Chromium pages are
counted once instead of once per process. With the real account Phoenix
uses **2.6–2.8× less PSS** than Discord, in a dev build that still holds a
128 MiB decoded-image budget. With the mock data and the release binary the
ratio is 4.4–4.8×.

The CPU numbers do not compare like for like. The real-account Phoenix run
is a dev build (phoenix crates at `opt-level` 0), headless, with the example
polling every 50 ms. The release run receives no gateway traffic. So the
real figure in a release build is somewhere between 0.45 % and 4 %, against
8–11 % for Discord.

### Start-up

| | runs |
|---|---|
| release, mock, process start → `first_frame` | 2,056 / 2,188 / 2,271 ms, then 1,442 / 1,603 ms |
| same, `PHX_NO_FONT_FALLBACK=1` | **428 / 495 / 500 ms** |
| dev build (`int-bin/discord-phoenix`), mock | 6,767 / 7,146 ms |
| real account, RESUME: `connect_start` → `snapshot_applied` | 118 ms (`resume_session_built` 240 ms) |
| real account, cold cache (IDENTIFY): `connect_start` → READY / first channel frame | 1,274 ms / +367 ms |

**1.0–1.8 s of the release start-up goes into building the font fallback
chain**, before the window opens. `strace` shows 52 `fc-match` processes
spawned one after the other (811 ms on their own, measured with a shell
loop) and 2,672 font files opened. Discord's own time to first frame was not
measured: that needs a restart of the user's client.

### `bench 5000`, 24/11 binary vs ac72344d

The old binary is `int-bin/bench` from 2026-11-24 09:39 (before phase 2).
The new one was built at ac72344d. In between there is a whole day of
parity work, so this measures main as it is, not phase 2 on its own. Both
are dev builds and the load average was 6–13.

| metric | 24/11 (2 runs) | ac72344d (2 runs) | change |
|---|---|---|---|
| `markdown_ns_per_msg` | 27,553 / 34,140 | 28,118 / 26,528 | noise |
| `markdown_allocs_per_msg` | 18.1 | **16.8** | −7 % |
| `channel_load_ms` (5,000 msgs) | 19,481 / 19,525 | 21,349 / 19,635 | noise |
| `scroll_frame_us_p50` | 27,697 / 29,427 | 33,108 / 30,180 | +3–20 % (load) |
| `scroll_frame_us_p95` | 37,746 / 41,508 | 48,891 / 40,916 | noise |
| `scroll_allocs_per_frame` | 10,770 / 10,775 | **12,551 / 12,556** | **+17 %** |
| `scroll_bytes_per_frame` | 20.7 MB | 21.8 MB | +5 % |
| `idle_allocs_per_frame` | 8,804 / 8,889 | 10,095 | +14 % |
| `rss_main_view_kib` | 405,716 / 374,136 | 369,016 / 350,688 | −7 % |
| `rss_after_scroll_kib` | 424,884 / 397,036 | 386,620 / 357,920 | **−9 %** |

Phase 2 shows up in the markdown allocations and in memory: the strict LRU
pass and dropping the per-message `HashMap` clones save 30–40 MB after the
scroll. The allocations per scroll frame went up, though. The parity work
added since 24/11 (role gradients, nameplates, decorations, reactions,
components) builds more elements per message than phase 2 saved. Times are
dominated by the load on the machine and by the dev profile. Allocation
counts do not depend on load, so they are the number to follow.

### Next targets (opti + refacto phase), by expected gain

1. **Font fallback off the start-up path.** It costs 1.0–1.8 s out of
   1.4–2.3 s, measured with `PHX_NO_FONT_FALLBACK`. Ask fontconfig once
   (`fc-match -s` for each locale gives the whole sorted list) or link
   libfontconfig instead of spawning 52 processes. Cache the chain on disk,
   keyed by locale and the mtime of `/var/cache/fontconfig`. Register the
   faces lazily, on the first glyph that misses. Expected: **first frame
   about 0.5 s instead of 1.5–2.3 s**.
2. **Allocations per scroll frame.** 12.5k allocations and 21.8 MB
   allocated per frame for about 15 visible messages, so roughly 800
   allocations per message per frame. Keep the element inputs of a message
   (author line, shaped text runs, reaction row, decoration lookups) on its
   `RenderedMessage` and rebuild them only when the message or the theme
   changes. Use `SharedString`/`Arc` instead of `String` clones in `ui/chat`.
   Expected: −50 to −70 % allocations per frame and −30 to −40 % frame time.
   This is what makes long channels smooth.
3. **Idle frames.** `idle_allocs_per_frame` = 10k, so a frame with nothing
   moving still rebuilds the whole tree. Find the `cx.notify()` calls that
   fire with no state change: the 1 s app tick in `app.rs` only needs to run
   while typing indicators, the slowmode cooldown or a call timer are active.
   Expected: CPU at rest towards 0 % outside the driver, and −1 wakeup/s.
4. **Timers and wakeups.** The tray polls the whole app state every 500 ms
   (`tray.rs`, 2 Hz on the two `blocking-*` threads plus the `Timer`
   thread). Push it from the places that change unread, mention or voice
   state. Turn the 1 s tick into a one-shot timer. Expected: Phoenix's own
   wakeups at rest from about 27/s to under 5/s. The 100/s of
   `libnvidia-eglcore` come from the driver and Discord's GPU process pays
   them too. Check whether they stop when the window is hidden or occluded
   (no frame requested).
5. **One runtime, one HTTP client.** Today there is a 2-worker tokio runtime
   in `discord_backend.rs`, plus 8 separate current-thread runtimes, each
   with its own thread and often its own reqwest/rustls client (updater ×2,
   sound assets, soundboard, i18n, font fallback, voice effects, D-Bus
   notifications). On top of that come about 40 ad-hoc `std::thread::spawn`
   calls for one-shot jobs. Move them all to the backend runtime (or GPUI's
   background executor) with one shared client: TLS sessions and HTTP/2
   connections get reused, and there are fewer threads. Expected: −5 to −15
   MB RSS, −8 short-lived threads, and fewer TLS handshakes to
   `discord.com`/CDN on start-up.
6. **Binary snapshot for RESUME.** `snapshot_parsed` 165 ms +
   `snapshot_replayed` 288 ms on the preload thread. A compact binary format
   (see "What's next" above) would cut both. Expected: −200 to −300 ms
   before the cached session is on screen.
7. **Decoded media budget.** The real-account run sits 140 MiB of PSS
   above the mock window (342 vs 202 MiB). Part of that gap is the dev build
   (a 195 MB binary mapped instead of 56 MB), and the rest is account state
   plus decoded avatars, emoji and GIF frames. To split the two, run a
   release `real_shots`, then:
   - decode at the size actually displayed;
   - keep only the first frame of an animation that is not hovered;
   - lower `BUDGET_BYTES` from 128 MiB to 64 MiB.
   Expected: −50 to −100 MB on a large account.
8. **Heavy dependencies.** The release binary is 55.9 MB. To check with
   `cargo bloat --release --crates` on the integrator:
   - `symphonia` (mp3 only, for the client sounds);
   - `image` with gif + webp, already needed;
   - `regex` (8 call sites in phoenix-app), where `memchr` or hand parsers
     may do;
   - `tracing-subscriber` with `env-filter`;
   - `keyring` with `crypto-rust`;
   - two async stacks (tokio, plus smol/async-io from GPUI and zbus).
   Expected: −5 to −10 MB binary and a shorter link time.
9. **Measure in an optimised profile.** `int-bin` is the dev profile:
   phoenix crates at `opt-level` 0, so frame times are 5–10× what users
   get. A `[profile.bench]` (or `profile.dev.package.phoenix-app.opt-level =
   1` only for `bench`/`frames`) would give numbers that mean something
   without a full LTO build. Cost: one more build of phoenix-app in the
   shared target dir, so check `df -h /home` first.

## Smooth scrolling and Lottie playback (6a203f9c…1bff4114, 2026-11-26)

`int-bin/frames 2000` (mock, headless 1280×800, dev profile, under `phoenix-nice`), same binary
(`0ad2f8a3`) run twice: "before" with `PHX_INSTANT_WHEEL=1 PHX_LOTTIE_PER_FRAME=1` (old paths),
"after" with the defaults. The `live` rows drive platform frame requests every 16 ms (draw only
when something is dirty, present-only otherwise) and report the CPU of the whole process.

| scenario | before | after |
|---|---|---|
| `lottie_hover_mic` (mic hovered 50 frames, left 10, eight times): CPU of one core | 34.8 % | **6.6 %** |
| same, mean time per frame request | 7.26 ms | **1.25 ms** |
| same, frame requests over 16 ms | 153 / 480 | **16 / 480** |
| `lottie_idle` CPU | 0.5 % | 2.4 % (sheets of the visible icons built ahead, once) |
| `scroll_chat_wheel` (precise 120 px deltas, full draw per frame) p50 / p95 | 69.9 / 126.4 ms | 72.3 / 105.1 ms |
| `scroll_members` p50 / p95 | 33.3 / 51.2 ms | 36.4 / 56.1 ms |
| `scroll_chat_notches` (one wheel notch every 8 frames) CPU | 23.8 % | 60.9 % |
| `scroll_members_notches` CPU | 12.3 % | 41.2 % |

- Lottie: a playing icon no longer redraws the window. What is left during a hover is the one
  full redraw of the hover change itself, and the present-only frames.
- Scrolling: the cost of one scroll frame does not change (the `scroll_chat_wheel` and
  `scroll_members` rows are within the run-to-run noise of this machine). A notch now moves the
  content over 100–200 ms instead of one frame, so it draws ~12 frames instead of 1, hence the
  higher CPU while the wheel turns. That is the price of the animation, like in Chromium, and it
  makes allocations per scroll frame (target 2 of "Next targets") more important.
- `PHX_TRACE_SCROLL=1` prints the chat offset at each frame during notches: it moves by small
  steps, eased and monotonic (for example −28800.00, −28792.77, −28773.69, −28755.34,
  −28743.05 px, one notch of 60 px every 8 frames). The headless window is inactive, so GPUI's
  inactive-window frame limiter only runs every second request there; on screen the compositor
  frame callback steps it on every frame.

## Fallback fonts from the disk cache (fe037d9b…0c97fcfb, 2026-11-26)

The 24 renamed text faces and the two emoji copies are kept in
`~/.cache/discord-phoenix/fonts/fallback-built` (about 48 MB, never in git).
`current.json` holds the key: a format number, the app version, the
fontconfig stamp, and the size and mtime of every file the build read
(source faces, bold candidates, the emoji font, the web Noto folder). Any
change rebuilds once. The chain and the gg sans extraction now run on their
own threads, and the UI no longer waits for the extraction when gg sans is
already installed from the decoded cache.

`real_shots` with `PHX_BOOT_FRAMES`, real account, offscreen, int-bin dev
build (load average 6–11, so only the order of magnitude counts):

| mark | before (52f3c2cd) | after (358d46b7, warm) |
|---|---|---|
| fallback chain prepared | ~2.6 s built (`font_fallback_ready` 6.14 s) | **49–66 ms** from the cache (`font_fallback_ready` 1.07 s) |
| `init_done` | 0.65 s | 0.36 s |
| `connect_start` | 1.81 s | 0.75 s |
| `snapshot_applied` | 6.22 s | 1.15 s |
| `ready_applied` (RESUME) | 6.81 s | 2.47 s |
| `boot_screen_fade` (UI shown) | 6.23 s | **2.80 s**, 0.33 s after READY |
| first captured frame | 12.6 s, already `showing=false` | 0.1 s, connecting screen |

The UI is shown once the open channel has its page after READY; the cached
session itself is on screen from 1.15 s under the connecting screen. The
headless example runs every GPUI task on one thread, so the app itself
should be faster than these numbers.

## Allocations per scroll frame (target 2), 2026-11-27

`int-bin/frames 2000` (mock, headless 1280×800, dev profile, under `phoenix-nice`), before at
`fdffabf2`, two runs:

| scenario | allocs / frame | bytes / frame | p50 / p95 |
|---|---|---|---|
| `idle` | 7,241 / 7,241 | 3.30 MB | 10.3 / 15.0 ms |
| `scroll_chat_wheel` | 26,064 / 26,055 | 11.2 MB | 40.5 / 53.7 ms |
| `scroll_chat_fast` | 22,608 / 22,325 | 9.2 MB | 32.6 / 46.0 ms |
| `hover_messages` | 13,086 / 12,392 | 5.5 MB | 14.7 / 31.5 ms |
| `scroll_members` | 12,864 / 12,864 | 5.8 MB | 16.9 / 21.0 ms |

Where they come from: `PHX_ALLOC_SITES=97 frames 2000` (one allocation in 97 with its
backtrace, 120 `scroll_chat_wheel` frames at `d23e86fb`, 33,779 samples):
- GPUI layout is the largest part: taffy flexbox 17.7 %, block 9.1 %, `new_with_children`
  5.8 %, absolute (`oof`) 3.9 %; then `Interactivity::new` 7.7 %, mouse listeners about 9 %,
  `TextStyle::default` 2.1 %, element ids 2.3 %, tab stops 2.6 %. All of it scales with the
  number of elements, so the lever is building fewer of them.
- Building the view: 11.6 % of the samples are under `PhoenixApp::render`, 11.1 % under the chat
  rows, and 7.2 % under `message_ext::toolbar` alone: every visible message built its hover
  toolbar (7 to 9 buttons, each with an icon, a tooltip and listeners) and kept it invisible.
- Smaller ones: the guild rail cloned the whole `NotifyPrefs` once per guild per frame
  (`guild_media`, 3.5 % on the mock, more on a real account with many overrides),
  `icons::path` formatted the same `icons/<name>.svg` string on every call (1.1 %), every
  Unicode emoji rebuilt its Twemoji URL and code points (`twemoji_url` 1.3 %).

Changes: the toolbar is only built for the hovered message, the one with an open menu and the
keyboard-selected one (it keeps its `invisible` + group-hover style, so a stale hover only costs
the build); `guild_media` returns early, before reading the notification settings, when no
channel of the guild has voice users and the guild has no live stage or event; `icons::path`
and the Twemoji URL are memoised per thread (the emoji table is capped at 8,192 entries).

## Long-run stability and resource leaks (2026-11-27)

### Baseline (`int-bin/real_shots`, real account, read-only, offscreen, `PHX_SWITCH_BENCH=10`)

Switching across 10 guilds, 10 channels and 10 DMs (dev build, under `phoenix-nice`):

| metric | at rest (perf.md) | after 30 switches (before) |
|---|---|---|
| RSS | 423 MiB | 875 MiB (`895,808 KiB`) |
| PSS | 342 MiB | 855 MiB (`875,883 KiB`) |
| Private_Dirty | — | 626 MiB (`641,364 KiB`) |
| Threads | 18 | 21 (bounded) |
| Open FDs | — | 128 (bounded) |

### Leaks and idle redraws fixed

1. **GPU sprite atlas leaks on eviction**: `ui/profile_fx.rs`, `ui/squircle.rs`, `ui/nameplate.rs` and `media/lottie.rs` evicted `Arc<RenderImage>` entries from their global caches without calling `cx.drop_image(img, None)`, leaving evicted textures (including multi-frame APNG profile effects and Lottie stickers) in the GPU atlas for the lifetime of the process.
2. **Unbounded channel message cache (`app.rs`)**: `PhoenixApp::messages` kept every visited channel forever and appended gateway `MESSAGE_CREATE` events without bound. Now capped at 32 channels (LRU eviction on channel switch) and 200 messages per background channel.
3. **Failed image entries in `LruImageCache` (`media/lru.rs`)**: `Err(_)` entries were skipped by `evict()` and accumulated indefinitely; stale failed entries are now pruned once `items.len() > 512`.
4. **Unbounded global caches**: capped `commands/store.rs` (`interactions` and `slot` at 2,048 entries, evicting oldest snowflake), `voice/soundboard.rs` (`decoded` PCM cache at 64 entries), and `ui/profile_board.rs` (`Store` at 128 entries).
5. **Idle 60 fps redraws**: `ui/profile_fx.rs` now calls `window.request_animation_frame()` only while an effect is actively animating (stopped/reduced-motion effects no longer redraw continuously); `ui/voice_panel.rs` (`meter_row`) only requests animation frames while a live mic level is present.

After (`baeb208d`, same command, two runs; frame times are noisy, the machine was loaded):

| scenario | allocs / frame before → after | bytes / frame | change |
|---|---|---|---|
| `idle` | 7,241 → 7,191 | 3.30 → 3.30 MB | −1 % |
| `scroll_chat_wheel` | 26,060 → 23,040 | 11.2 → 10.5 MB | −12 % |
| `scroll_chat_fast` (`scroll_by` 600 px, no pointer) | 22,467 → 7,811 | 9.2 → 3.6 MB | **−65 %** |
| `hover_messages` | 12,739 → 9,909 | 5.5 → 4.5 MB | −22 % |
| `scroll_members` | 12,864 → 12,714 | 5.8 → 5.8 MB | −1 % |

`scroll_chat_fast` p50 went from 32.6–37.7 ms to 16.8–17.3 ms. `scroll_chat_wheel` keeps most of its
cost because the pointer sits on the chat: each frame a new message comes under it, its hover
state changes and the toolbar is built for it; the rest is GPUI layout of the visible rows.
Next step for this target: fewer elements per row (the reply and author lines, the per-row
tooltips and listeners), measured with `PHX_ALLOC_SITES`.

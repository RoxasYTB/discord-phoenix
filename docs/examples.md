# Examples

Offscreen renderers, probes and benchmarks in `crates/phoenix-app/examples/`.
Run them from `int-bin/` (see `docs/BUILD.md`).

## Demo people (MockBackend)

The demo users in `crates/phoenix-app/src/demo_media.rs` carry real avatars
(user id + hash) and real production server tags (identity guild id, tag,
badge hash) of current users, picked neutral (no staff, no photos of people),
as allowed by the project owner on 2026-11-25. Only ids and hashes are in the
repo: the images are never committed. `demo_media::install` sets the caching
CDN client in the mock examples and preloads every avatar and badge into the
local media cache (`~/.cache/.../media`, outside git), so a later run works
offline. `PHX_DEMO_NO_PREFETCH=1` skips the preload. The badge URL is built by
`PrimaryGuild::badge_url`, the same as on the real account, and every demo tag
has its badge. Demo messages and servers stay fictional.

## `anim_emoji_shot`

Offscreen frames of animated custom emoji as Phoenix draws them (message size,
jumbo, reaction, picker tile at rest and hovered-off):
`anim_emoji_shot <out-prefix> <dir> <id>...` writes `<out-prefix>-<n>.png`,
one frame every 150 ms. `<dir>` holds generated test art `<id>.gif` (playing)
and `<id>.png` (still), served through the `local/` asset path: no real emoji.

Then it measures playback: the colour at the centre of the first 22px emoji
is sampled every 20 ms for 3 s (`fps` = colour changes per second), and the
process CPU time is read from `/proc/self/stat` over 5 s of 60 Hz redraws,
once with the emoji playing and once with `Play animated emoji` off.

## `bench`

Headless micro/macro benchmark on mock data (no on-screen window).
`cargo run --release -p phoenix-app --example bench [-- N]`

1. markdown parse + rich-text layout of a message corpus (ns and allocs per message)
2. mock login and a channel of N (default 5000) messages loaded by scrolling to the top,
   then frame time and allocations per frame while scrolling, and RSS.

Prints `key=value` lines that scripts/bench/run.sh collects.

## `call_view`

Offscreen render of the voice call view with mock voice state (never joins voice).
`cargo run -p phoenix-app --example call_view -- out.png [chat] [grid] [nostrip] [caret=mic|camera]`
`PHX_MOUSE=x,y PHX_HOVER_ANIM=0,100,… [PHX_MOUSE_LEAVE=x,y]` writes one frame per instant of the hover animation (`out-t0100.png`, `out-leave.png`), see docs/parity/hover-animations.md.

## `component_shots`

Offscreen MOCK render of components that the plain mock screenshot does not show:
member list Activity cards, a nameplated member row, and the profile popout opened
from a voice channel member (with its "In a voice channel" card).
`cargo run -p phoenix-app --example component_shots -- out_dir`

## `content_shots`

Offscreen render of the mock #showcase channel (every kind of message content).
`content_shots out.png [dark|light|ash|onyx]`; PHOENIX_SHOT_H sets the window height.

## `dialog_shots`

Offscreen renders of dialogs and settings pages at the live client's
1280x700 viewport. `cargo run -p phoenix-app --example dialog_shots -- out.png <scene>`
scene: settings:<page> | leave | delete | status

## `frames`

Frame times per interaction, offscreen on mock data (no window on screen).
`cargo run --release -p phoenix-app --example frames [-- N]`

For each scenario: p50 / p95 / max frame time, frames over 16 ms, and
allocations per frame. A "frame" is refresh + layout + paint
(`run_until_parked`), without the GPU readback of screenshots.
Prints one `scenario p50_us p95_us max_us over16 allocs bytes` line each,
then RSS. See docs/perf.md.

## `gif_mem`

Memory bench for chat gifs: decodes every `<prefix>*.gif` in a directory
the way GPUI does (every frame kept as a full RGBA canvas), holds them all
like 20 gifs on screen would, and prints decoded bytes + process PSS.
usage: gif_mem <dir> <prefix>   (see scripts/bench/gifs.sh)

## `nav_shots`

Offscreen render of the navigation (guild rail, channel list, voice and
user panels) while connected to a voice channel.
`cargo run -p phoenix-app --example nav_shots -- out.png`

## `profile_editor`

Offscreen render of the full-screen profile editor to a PNG.
`cargo run -p phoenix-app --example profile_editor -- out.png [dirty]`
(`dirty` types into the pronouns field to show the unsaved-changes bar).

`PHX_REAL=1` renders the user's REAL account instead of the mock (token from
the keyring, strictly read-only: every write the backend could do is refused,
so nothing is ever saved). `PHX_TAB=0|1|2` picks Board / Activity / Wishlist,
`PHX_SWITCHER=1` opens the profile switcher, `PHX_NO_PREMIUM=1` (mock) shows
the Nitro locks. `PHX_WAIT` seconds to wait for the profile and images (default 12).

## `real_probe`

Headless, READ-ONLY probe against the real account whose token is in the
keyring. It never sends messages, never acks, never types, never changes
presence (the gateway identifies with status "unknown") and never joins
voice. It logs parse failures, unknown events, suspicious empty fields and
slow requests so they can be turned into anonymized regression tests.

```sh
cargo run -p phoenix-app --example real_probe -- [seconds=150] [live.json]
```
`live.json` (optional) is a list of `{id, pill, badge}` read from the
official client's guild bar, to compare guild order and unread state.
Output contains ids but never the token; keep it out of the repo.

## `real_shots`

Offscreen render of Phoenix on the user's REAL account, strictly read-only.

`cargo run -p phoenix-app --example real_shots -- out.png`

The token comes from the keyring (never printed, never rewritten). The
backend is wrapped so nothing can be sent, acked, typed, reacted, joined or
changed: only READY, channel selection, message history, member list
subscription and media downloads go out.

Location: env `PHX_GUILD` + `PHX_CHANNEL` (`PHX_GUILD` alone = its first text channel), or `PHX_DM=<channel id>`, or by
default the location shown by the user's Discord (`cdp.mjs eval location.pathname`).
Size / theme / density: env `PHX_W`, `PHX_H`, `PHX_THEME` (dark|light|ash|onyx), `PHX_HC`,
`PHX_DENSITY` (compact|default|cozy), `PHX_FONT` (px), otherwise read from the live Discord
through CDP. `PHX_COMPACT=1` = compact message display.
`PHX_AUDIO` = two digits mute/deaf (e.g. `11`), otherwise read from the live user panel.
`PHX_PROFILE=<user id>|peer` opens that profile (`PHX_PROFILE_KIND` popout|modal, `PHX_PROFILE_BOX=x,y`).
`PHX_VIEW=friends` opens the Friends page (`PHX_TAB` = online|all|pending|blocked|add);
`PHX_VIEW=inbox` opens the inbox popout over the location (`PHX_TAB` =
mentions|unreads|saved|reminders|scheduled).
`PHX_OPEN` = pins|threads|search[:q]|filters[:date|calendar|list<n>]|settings[:page]|server_settings[:n]|emoji|gif|sticker|msgmenu|chanmenu[:n]|catmenu[:n]|guildmenu|guilddrop|usermenu[:n]|switcher|thread:<id>|thread_create:<msg>|attach|poll_create|leave|invite|raid|add_to_dm|post[:n]|events|community[:browse]|guide.
`PHX_PROFILE=member:<n>` = nth member of the list; `PHX_DEBUG_GIF=1` prints the GIF home previews.
`PHX_FX_STILL=1` = still profile effects (reduce motion).
`PHX_GRADIENT_AT=0,375,…` writes `<out>-gNNNN.png`, the frame with the gradient clock that many ms
after the first scrolling name gradient (hover a message with `PHX_MOUSE`), see GPUI patch 17.
`PHX_PRESENT_CPU=<secs>` prints the process CPU while a hovered name scrolls through
re-presents, with a full redraw every frame, and idle.
`PHX_SEARCH_SORT=old|rel` and `PHX_SEARCH_PAGE=<n>` with `PHX_OPEN=search:<q>`.
`PHX_WAIT` = seconds to wait for messages and images (default 15).
`PHX_RAIL_MENTIONS=<guild or DM id>:<n>,…` shows those mention counts on the rail icons (local display only, for the badge cut-outs).
`PHX_DM_CALL=<DM id>[:voice|video|stream[:here]]` puts the DM's recipient in a call with those flags (and the current user with `here`, which also sets the connected call), local display only, for the rail's DM voice badge.
`PHX_TABS=n` presses Tab n times before the capture (`-n` = Shift+Tab), for the focus rings.
`PHX_SEND=<what>` (send-lifecycle test, ONLY in the hard-coded test channel
`SEND_TEST_CHANNEL` in `real_shots.rs`, refused anywhere else): types
"[Test Phoenix] envoyé depuis le client Rust (Phoenix) — <what>" and presses Enter.
The POST is held `PHX_SEND_HOLD` ms (default 4000) so the pending copy can be shot to
`<out>-pending.png`; the settled state goes to `<out>`. `PHX_SEND_FAIL=400` answers
with a local 400 instead of sending (nothing leaves the machine).

## `screenshot`

Offscreen render of the main window to a PNG (no on-screen window).
`cargo run -p phoenix-app --example screenshot -- out.png [login]`
`PHX_DM=<channel id>` opens a demo DM (901 NEON, 902 TOON, 903 POP, 904 GUMMY display name styles).
`PHX_MOUSE=x,y` with `PHX_GRADIENT_AT=0,500,…` writes `<out>-gNNNN.png`, the frame with the
animation clock that many ms after the first running name animation (GPUI patches 17 and 18), and
`PHX_PRESENT_CPU=<secs>` prints the CPU while it animates (present only, full redraw) and idle,
like `real_shots`.

## `shortcuts_shot`

Offscreen renders (mock data) of the keyboard-shortcut UI at the live dump's
1536x840 viewport: `cargo run -p phoenix-app --example shortcuts_shot -- out.png <scene>`
scene: modal (Ctrl+/ combos) | focus (keyboard-focused message) | thread ("Nouveau fil" sidebar)
| mention / emoji / channel (composer autocomplete) | long (character counter) | multiline | slowmode | blocked | upload (attachment tray)

## `states_shots`

Offscreen renders of hover / pressed states (docs/parity/states.md).
`cargo run -p phoenix-app --example states_shots -- out_dir x,y[,down] ...`
writes one PNG per point with the mouse moved there (left-pressed with `,down`,
right-clicked with `,right`; a later point can then hover the open menu).
Env: `PHX_SETTINGS=1` opens settings; `PHX_OPEN=pins|inbox|threads|groupdm|privacy|server_profile|search[:query]` a popout.

## `theme_shots`

Offscreen render of the main window in a given theme and zoom.
`cargo run -p phoenix-app --example theme_shots -- out.png [dark|light|ash|onyx] [zoom%]` (PHOENIX_HC=1: high contrast)
Env: `PHX_COMPACT` (compact mode), `PHX_DM` (open a DM).

## `threads_shots`

Offscreen render of the threads browser with archived ("older") threads.
`cargo run -p phoenix-app --example threads_shots -- out.png`

## `upload_probe`

Real upload check, hard-locked to the two channels the user allowed test
sends in: #général and the #phoenix sandbox of the "Aperture Sciences" test
server. Uses the same REST flow as the app (upload slots, chunked PUT to
Cloud Storage, JSON message). Prints the steps and the progress, never the
token or the signed URLs.

```sh
PROBE_CHANNEL=phoenix|general PROBE_CHUNK_KIB=4096 \
  cargo run -p phoenix-app --example upload_probe -- "<what is tested>" <file>[:spoiler][:alt=text] ...
```

## `views`

Offscreen render of one non-chat view in a given theme.
`cargo run -p phoenix-app --example views -- out.png [dark|light|ash|onyx] <scene>`
Scenes: friends-all, friends-pending, inbox, pins, threads, search, profile, profile-modal, appearance, account, status, switcher, emoji, gif, sticker, friends, forum, post, events, browse, guide, dm-menu, dm-submenu.

## `voice_settings`

Offscreen render of the Voice & Video settings page to a PNG.
`cargo run -p phoenix-app --example voice_settings -- out.png`


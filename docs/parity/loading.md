# Start-up and loading states

Sources: the installed desktop app (`resources/app.asar`: `splash/index.html`, `splash/index.css`,
`splashScreenPreload.js`, `splash/videos/connecting.webp`), the web bundle (`web.*.js`, the
connecting screen component that uses CSS module `_oDjP9R`, the app spinner module with
`7ba7fcf2c4710bb7.webm`), chunk 868544 (connecting CSS) and chunk 123141 (message placeholder CSS,
`_oITdBA`), read on 26/11 from the local install and cache. Assets are read at runtime from the
local install and never committed.

Legend: ✅ implemented and traced to the source, ⚠ partial, ❌ missing.

## Desktop splash window (`splash.rs`)

| item | client | ours | status |
|---|---|---|---|
| window | 300×300, frameless, `--account-grey` #282b30 | `splash::window_options`, 300×300 client decorations, #282b30 | ✅ |
| logo | `connecting.webp`, 200×200, 400×400 source, 64 frames of 16–17 ms (1 066 ms loop) | same file read out of `app.asar` (`splash::asar_read`), animated by `gpui::img` | ✅ |
| status | `.splash-status` white, 16px, weight 500, `top:-30px`; "Checking for updates…" then "Starting…" (English only in the client) | `Status::CheckingForUpdates` → `Status::Starting` | ✅ |
| progress | placeholder 8px, 10px top margin (bar only while downloading updates) | placeholder only (no update download at start) | ✅ |
| quotes | `DISCORD_SPLASH_SCREEN_QUOTE` handler is empty in this build: no quote is shown | none | ✅ |
| order | splash, then the main window shows the connecting screen | main.rs opens the splash, waits for the font fallback chain (at most 3 s; 49–66 ms when it comes from `fonts/fallback-built`, see perf.md), says "Starting…", opens the main window and closes the splash | ✅ |

## Connecting screen in the main window (`ui/boot.rs`)

| item | client | ours | status |
|---|---|---|---|
| container | absolute, z 3000, `--background-base-lowest`, centred | overlay above everything except the window chrome | ✅ |
| spinner | app spinner video 200×200 (same animation as `connecting.webp`, checked frame by frame) | `connecting.webp` | ✅ |
| title | "Did you know" (`v0R1Lh`), 16px, medium, `--text-subtle`, 8px below, line height of body-xs (1.333) | `boot.did_you_know` | ✅ |
| tip | one of 58 tips at random, plus the polls tip for `en-*` locales; 16/20, `--text-default`, max 300px; keyboard hooks drawn as key combos (`keybind_oDjP9R`, -2px end margin, -1px lift) | `boot.tip_01`…`boot.tip_59`, client AST walked for every locale with a table, key caps from `shortcuts::combo` | ✅ |
| text block | `top:-20px` | -20px | ✅ |
| exit | container opacity .2s ease-out, text and spinner .15s (`--connecting-content-fade-duration`), spinner scale 1 → .5, unmounted after 200ms | `Boot::frame` | ✅ |
| problems | after 10s, slides up from below (.2s ease-out): "Connection problems? Let us know!" 14px, "Tweet Us" + "Server Status" links 16px medium `--text-link`, hover opacity .8 | same; X icon from `XNeutralIcon`, status icon = the inline `pF` component (14×14 viewBox, `icon_oDjP9R` 16 wide, `--text-link`), extracted by its path (`boot/status`) | ✅ wt/inv-home |
| incident | web bundle `lw.checkIncidents` when the problems block appears (10 s): `GET https://status.discord.com/api/v2/scheduled-maintenances/active.json` and `/api/v2/incidents/unresolved.json` (both must answer), incident = first unresolved incident or first active maintenance (`MaintenanceStore`); the screen then drops « Le savais-tu », shows `t.name` in `title_oDjP9R contentBase_oDjP9R` (`--text-strong` 16/20 italic, medium, mb 10, max 600) and `incident_updates[0].body` in `body_oDjP9R contentBase_oDjP9R` (`--text-default` 14/16 medium, max 600); classes from chunk 868544 | `boot::check_incidents` (once, background thread), `parse_incident` (test), `boot::render` | ✅ (no incident open on 26/11, so the real render is the tip) |
| connected sound | `discodo` on connect, only with the `discodoEasterEgg` key (16 quick Home clicks), see sounds.md | played when the screen starts to fade, same key | ✅ |
| when it hides | the client hides it on gateway connect | Phoenix hides it when the UI is stable: fonts installed (fallback chain + gg sans extraction), the cached session applied, and the open text channel has its cached or fetched page; capped at 5s on the main screen | ✅ |

## Message placeholders (`ui/chat/skeleton.rs`)

| item | client | ours | status |
|---|---|---|---|
| where | chat top placeholder while `hasMoreBefore` (and while the first page loads) | chat row 0 when the start is not reached, thread panel while loading | ✅ |
| count | `generateMessageSpecs`: page size `clamp(ceil(1.05·ceil((winH−145, ≥400)/(65 compact, 91 cozy)·fontScale)), 10, 30)`, groups = page (compact) or round(.87·page), each group 1–4 messages, attachments max(1, round(groups/30·8)) of 140–400 × 100–320 | `skeleton::generate`, same formula, seeded per channel | ✅ |
| height | Σ spacing·r + (44 cozy / 26 compact)·r + (n−1)·(22 / 26)·r + Σ (attachment + 6r) | `Specs::total_height` | ✅ |
| cozy | 72px start padding, avatar 40px at 16/4 opacity .08, name blob 80–120 px opacity .1–.2 per group, line blob 120–320 px opacity .06, blobs 16px tall, 8px radius, 3px top margin in 22px rows, `--text-default` | same | ✅ |
| compact | 80px start padding, timestamp blob 50px (opacity .02–.08, −64px/4px margins, hidden after the first line), name blob with 8px end margin, then the line | same | ✅ |
| attachment | opacity .03, 6px radius | same | ✅ |
| animation | none: the placeholders are static in the client CSS | static | ✅ |
| other views | profile modal mutual friends / servers: `Spinner` (289873, wandering cubes) in `empty_6X8yxV` (column, centred, padding 20 0) while `mutualFriends == null` / `mutualGuilds == null && isFetching` (chunk `ffb161a803a726fc.js` `e0` / `e1`); messages popout (pins, inbox): `loadingPlaceholder_6bgemo` (margin 120 0) with the spinner when nothing is loaded yet, `loadingMore_6bgemo` (margin 32 0) below loaded messages (`5de20f764ef64522.js`); forum list: section 3 rows `round((innerHeight − 200) / 118)` of `loadingCard_BkGrJJ` (102 px, `--background-base-low`, radius md, mb 8, opacity .33 / 1 / .66 by `row % 3`, `8277e633e33111b5.js`); spinner CSS `193433`: 32 px box, two 10 px `--brand-400` cubes, 1.8 s ease-in-out keyframes (22 px moves, −90° steps, scale .5 at 25 % / 75 %), second cube −.9 s; reduced motion → `lowMotion` (three 6 px `--interactive-text-default` dots, 1.4 s, delays .2 s) | `ui::spinner` (cubes drawn as rotated SVG squares), `forum::loading_card`, `inbox::loading_spinner`, profile modal tabs | ✅ tests `cubes_follow_the_keyframes`, `dots_pulse_after_their_delay` |

## Live check (26/11, #phoenix with a slow network emulated over CDP)

Client DOM: 27 `wrapper_oITdBA cozy_oITdBA` rows inside a 1 478px holder, `animation: none` on every blob,
avatar 40px at +16/+4, blobs at +72, 16px tall, 8px radius, name blob 4.5px below its 22px row top,
opacities .08 / .10–.19 / .06, 16px top margin on each group. The Phoenix render of the same channel at
1280×700, dpr 1.5 lands the avatar and blobs on the same columns (682 vs 681 device px). The blobs
are random on both sides, so only the geometry is compared. When the channel is empty, the
placeholder now keeps the 20px gap above the composer like the last message does.

## Chat scroll range and older pages (`ui/chat/rows.rs`)

| item | client | ours | status |
|---|---|---|---|
| scroll range | every loaded message is in the DOM (CDP 27/11 on the user's open channel: 38 `li[id^=chat-messages-]` rows, `scrollHeight` 5 232 for a 567 px scroller), so the thumb is exact and only changes when messages are added | the message list measures every row (`ListState::measure_all`); each insertion re-arms the measure (`rows::insert`), a changed row keeps its height until it is measured again (`rows::refresh`). Before: only the rows within 800 px of the viewport were measured, the range grew 856 → 1 704 → 2 270 → 3 042 px while scrolling up through the same 50 messages of #général | ✅ test `the_thumb_keeps_its_size_while_scrolling_through_loaded_messages` |
| older page | loaded while the top placeholder is on screen; the scroller keeps its distance from the bottom, so the messages on screen stay put and the new ones fill the space of the placeholder | `rows::load_when_visible` asks for the older page after any frame where row 0 is on screen (not only on a scroll event); `rows::prepend` keeps the first old message at the same height on screen (the new rows above it are laid out once in `rows::keep_anchor`, before the list). Before: the anchor stayed in row 0, so the placeholders stayed on screen above 1 200 loaded messages until the next scroll | ✅ tests `an_older_page_replaces_the_placeholders_and_keeps_the_view`, `placeholders_on_screen_load_the_older_page_without_a_scroll` |

## Start-up frames (`real_shots`, `PHX_BOOT_FRAMES`)

`PHX_BOOT_FRAMES=<prefix>` saves one frame every `PHX_BOOT_EVERY` ms (100 by default) from the moment the
window opens, running pending tasks only for that interval between captures, until three frames after the
connecting screen is gone. `PHX_ONLY_GUILD`, `PHX_GUILD` and `PHX_CHANNEL` are applied on every frame (rail
limited to that server, target channel opened as soon as the cached session has it). Measured 26/11 at
358d46b7: connecting screen from 0.1 s to 2.8 s, then the UI. The frame buffer is larger than the window
when `PHX_SCALE` is not 2 (black band on the right and bottom): crop to `PHX_W·PHX_SCALE × PHX_H·PHX_SCALE`.
The spinner video is not drawn in these headless frames yet.

## Channel preload (`history_ui::preload`, `hover_channel`, `hover_guild`)

| item | client | ours | status |
|---|---|---|---|
| trigger | `onMouseDown` on a channel row (`handleMouseDown` in the text channel component, chunk `6b05ca0762af596a.js`), a DM row (`e8`, web bundle) and a favourite (`onMouseDown`), all through `ChannelActions.preload(guildId, channelId)` (module 377480) | mouse down on a channel or DM row; hover also reads the page from the message cache (or fetches it in the background when there is none), and hovering a server icon does the same for the channel it would open (`PhoenixApp::guild_target`) | ✅ |
| what it does | dispatches `CHANNEL_PRELOAD` with `context: MAIN`; `MessageManager` (123363) fetches the messages (`j` → `P({guildId, channelId})`), `ForumManager` loads forum posts; no `CHANNEL_SELECT`, so no ack | first page loaded and laid out on a `phoenix-layout` thread into `PhoenixApp::messages`; nothing is selected or acked; at most 24 preloaded pages are kept, the oldest unopened one is dropped | ✅ |
| after READY | — | the 8 most recently cached channels are preloaded, one every 150 ms, 1.5 s after READY | ✅ |

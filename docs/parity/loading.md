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
| order | splash, then the main window shows the connecting screen | main.rs opens the splash, waits for the font fallback chain (at most 3 s), says "Starting…", opens the main window and closes the splash | ✅ |

## Connecting screen in the main window (`ui/boot.rs`)

| item | client | ours | status |
|---|---|---|---|
| container | absolute, z 3000, `--background-base-lowest`, centred | overlay above everything except the window chrome | ✅ |
| spinner | app spinner video 200×200 (same animation as `connecting.webp`, checked frame by frame) | `connecting.webp` | ✅ |
| title | "Did you know" (`v0R1Lh`), 16px, medium, `--text-subtle`, 8px below, line height of body-xs (1.333) | `boot.did_you_know` | ✅ |
| tip | one of 58 tips at random, plus the polls tip for `en-*` locales; 16/20, `--text-default`, max 300px; keyboard hooks drawn as key combos (`keybind_oDjP9R`, -2px end margin, -1px lift) | `boot.tip_01`…`boot.tip_59`, client AST walked for every locale with a table, key caps from `shortcuts::combo` | ✅ |
| text block | `top:-20px` | -20px | ✅ |
| exit | container opacity .2s ease-out, text and spinner .15s (`--connecting-content-fade-duration`), spinner scale 1 → .5, unmounted after 200ms | `Boot::frame` | ✅ |
| problems | after 10s, slides up from below (.2s ease-out): "Connection problems? Let us know!" 14px, "Tweet Us" + "Server Status" links 16px medium `--text-link`, hover opacity .8 | same; X icon from `XNeutralIcon` | ⚠ the status link icon is a local SVG of the client, not drawn |
| incident | statuspage incident name and update body replace the tip | not fetched | ❌ |
| connected sound | `discodo` when the "connected" sound option is on | not played | ❌ |
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
| other views | the profile mutual friends/servers lists use 40/60 and 135 px blobs at .08; the member list has its own placeholders; inbox, search and forum lists use spinners | member list already done; profile lists not checked | ⚠ |

## Live check (26/11, #phoenix with a slow network emulated over CDP)

Client DOM: 27 `wrapper_oITdBA cozy_oITdBA` rows inside a 1 478px holder, `animation: none` on every blob,
avatar 40px at +16/+4, blobs at +72, 16px tall, 8px radius, name blob 4.5px below its 22px row top,
opacities .08 / .10–.19 / .06, 16px top margin on each group. The Phoenix render of the same channel at
1280×700, dpr 1.5 lands the avatar and blobs on the same columns (682 vs 681 device px). The blobs
are random on both sides, so only the geometry is compared. When the channel is empty, the
placeholder now keeps the 20px gap above the composer like the last message does.

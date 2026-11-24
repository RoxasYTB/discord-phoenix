# Responsive layout (narrow windows, zoom 50–200 %)

Reference: the live 2026 client over CDP, 2026-11-24, theme Onyx, cozy, font 16, zoom 125 %
(`devicePixelRatio` 1.25), guild text channel without a topic, member list shown, channel list
dragged to 345 px. The width was changed only through `Emulation.setDeviceMetricsOverride`
(the user's window was never moved or resized), then `Emulation.clearDeviceMetricsOverride`
restored 1536×788. All values below are CSS px (`innerWidth`).

## How zoom works

Discord's zoom is Chromium page zoom: a window of W device-independent px is `W / zoom` CSS px
wide and every rule below applies to that CSS width. Phoenix does the same (`set_page_zoom`, see
`docs/gpui-patches.md`): layout runs in CSS px and the thresholds below are written in CSS px,
so they hold at every zoom level. A 1700 px window at 150 % is a 1133 CSS px layout; at 200 %
it is 850 CSS px.

Offscreen check without a zoom setting: `PHX_W=<css width> PHX_SCALE=<dpr × zoom>` on
`real_shots` (for example `PHX_W=1130 PHX_SCALE=1.875` is 150 % at dpr 1.25), or
`theme_shots out.png dark 150|200` on the demo data.

## Thresholds

| Area | Live behaviour | Phoenix | State |
|---|---|---|---|
| Member list | Never hidden by width. Stays 268 px down to `innerWidth` 480 even when the chat column reaches 0 px; only the toolbar button hides it | `ui/chat.rs` shows it whenever `show_members` | ✅ (no auto-hide, same as live) |
| Channel list + rail | Fixed (416 px here, the dragged width); never shrinks with the window | unchanged | ✅ |
| Header search | `clamp(64, header content width − 320, 244)`; content width = header border box − 28 (padding 20 + 8). Header 983 → 244, 583 → 235, 543 → 195, 483 → 135, 433 → 85, 383 → 64. Below that it is squeezed further (38 px at header 223) | `header_search_w` from a width probe on the header | ✅ |
| Header toolbar buttons | Threads, notifications, pins, member list: all four stay at every width (32 px, gap 10, padding-left 10, search margin 4). No overflow menu | unchanged | ✅ |
| Channel title | `titleWrapper_` is flex-shrink 0: the title keeps its full width and `children_` (flex 1, overflow hidden) clips it once the search is at 64 px (children_ 138 → 109 → 9). No ellipsis on the title | title `flex_shrink(0)` except thread titles (flex 0 1 auto) | ✅ (was shrinking to "Di") |
| Composer buttons | `channelTextArea_` ≥ 450 px: gift, GIF, sticker, emoji, apps. Below (449.8 measured): gift, emoji, apps. Nothing else hides, even at 2 px | `full_buttons` from a width probe on the text area | ✅ |
| Server rail badges | Icon 40 px; `upperBadge_` 16 px at (24, 0), `lowerBadge_` 16 px at (24, 24), cut-out rings; the two never touch | same boxes at 100 % (pixel check on `theme_shots` at dpr 2) | ✅ (layout); rasterisation at 150/200 % not compared against a live capture at that zoom |

Offscreen check after the merge (7b62021, `theme_shots` demo data, 1280×800 at dpr 2): at 150 %
(853 CSS px) the title stays whole, the topic takes the remaining room, the search is about 175 px (read off the image) and
the composer shows gift, emoji, apps; at 200 % (640 CSS px) the search is 64 px and the title is
clipped by `children_` exactly like the client at that width.

Not measured: a channel with a topic (the user's channel had none; Phoenix keeps the topic as
the flexible, ellipsized part after the title), DM and forum headers, compact density.

## Member list (read-only checks, 2026-11-24)

Same guild (44 members, under the lazy list's first 100-row range), live list read with a 2080 px
tall emulated viewport so the whole list is in the DOM, Phoenix with `real_shots PHX_H=2080`.

| Check | Live | Phoenix | State |
|---|---|---|---|
| Groups and counts | hoisted roles in server order, then "Hors ligne — 25" | same four groups, same counts and order (rows come from GUILD_MEMBER_LIST_UPDATE as sent) | ✅ |
| Offline group | present below the role groups for this guild size | rendered from the server's `offline` group | ✅ |
| Owner crown | no `ownerIcon_` when the owner sits in a hoisted role group; dumps (`vanilla-threads-lrer`, `vanilla-search-popout-2`) show it in "Hors ligne" | crown only outside hoisted role sections (wt/members26) | ✅ (2 data points) |
| Range loading | — | [0, 99] on open, then the chunk in view and the next one (op 37) with placeholder rows | ❌ not compared: needs a guild with more than 100 members open in the live client, which would mean navigating (and acking) there |
| "Afficher plus" | not found in this guild's list | — | ❌ not observed live |
| Activity section | absent in the live list right now | Phoenix shows "Activité — 7" for the same guild | ❌ gate differs (`activity_feed_enabled`), not handled here |

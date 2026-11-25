# Keyboard navigation and accessibility parity

Sources: the live 2026 client over CDP (keys sent with `Input.dispatchKeyEvent`, nothing validated, every
pass closed with Esc and checked), and the local web bundle `web.55063002a61599e3.js` (module ids below).
Private captures stay in the scratch area; nothing from the real account is committed.

## Quick Switcher (Ctrl+K)
Code: `crates/phoenix-app/src/ui/switcher.rs`. Store: bundle module 174768 (`QuickSwitcherStore`), search
engine 290676, match scores 997625 (`queryChannels`, `queryGuilds`, `eN` for users), frecency 205761.

| Behaviour | Client | Phoenix |
|---|---|---|
| Prefixes | `@` users (in the current server: friends and members), `@@` all users, `#` text, `!` voice, `*` servers, `$` game profiles. Regex `^@\|#\|!\|\*\|\$`: only `@` must lead, the first of the others is removed wherever it is | ✅ `parse`, unit test |
| Empty query | sections "Previous Channels" (history of the last 8 selected channels, current one left out), "Drafts", "Mentions", "Unread Channels" (current server, muted ones and their categories skipped). History is cut to 3 rows when another section exists, 7 otherwise | ✅ except Drafts (❌ Phoenix keeps no draft list). History is saved in `navigation.txt` (`history` line), like the persisted store |
| Prefix with empty query | header + `#`/`!`: the current server's channels by position, `*`: every server, `@`: people who talked in the current channel (newest first), `$`: empty | ✅ |
| Headers | "Search friends and users in: {server}", "Searching All Users", "Searching Text Channels", "Searching Voice Channels", "Searching Servers", "Searching Game Profiles" | ✅ strings from the client bundles |
| Scores | channels: `^q` 10 if equal else 7, contains 5, every word 3, subsequence 1; extra words matched on the server and category names at half weight (capped at 6); voice −1 in text searches; +3 when the channel has a frecency, capped at 7 or 10. Servers and group DMs use the same scale, users 10 for a prefix of username/nickname/global name, 1 for an accent-folded prefix or subsequence. Score ×1000 × booster (1 + frecency / max frecency; friends +0.2, DM recipients +0.1) | ✅ `eg`, `best`, `channels`, `users`, unit and GPUI tests. Accent folding covers Latin letters (the client also maps confusables, module 209034) |
| Limits | 5 per category with no prefix, 100 with a prefix; a single non-empty category is fetched again with 100 | ✅ |
| Frecency | settings-proto 2 field 12 `guild_and_channel_frecency` (map fixed64 → FrecencyItem), weights 100/70/50/30/10 for 0/1/2–3/4–6/7+ days, `ceil(total × score / samples)` | ✅ `phoenix-api` `place_frecency`, test. Loaded with the expression frecency when the switcher opens |
| Arrows | Up/Down wrap and skip headers (`Vv`), first non-header selected after each search; Tab does nothing; mouse hover selects | ✅ GPUI test |
| Rows | 34 px, radius 3, text `interactive-text-default`, unread rows `text-strong`, selected row background only (no text color change, no hover background); server rows show the note "Server"; icons 20 px, server icon radius 3 | ✅ (CSS `481597.28b4829815acffe9.css`, `.result_hm_rAU` …) |
| No result | "Can't find what you're looking for?" + "Learn more about Quick Switcher" link | ✅ text, ⚠ link not clickable |
| Not done | applications, game profiles, links/invites and in-app pages (Shop, Nitro, Quests, Settings) results | ❌ no data source in Phoenix |

## F6 sections
Client module 312138: F6 walks the elements marked `data-jump-section` in DOM order, Shift+F6 backwards, both
wrap; from outside any section the first one gets focus. Measured order on a server channel: back button
(title bar), selected server, selected channel, account button, a message, the member list.

| | Status |
|---|---|
| Order, wrap, Shift+F6, start from outside | ✅ `shortcuts.rs` `jump_section`, GPUI test |
| Rings | ✅ 4 px `#00b0f4` on the back button (radius 6) and the account button (new), existing rings elsewhere. ⚠ the client draws no ring on the focused server icon; Phoenix does |
| Messages | ✅ focuses the newest message (message focus mode); the client keeps the last focused message |
| Member list | ✅ first member row; the client focuses the first control of the list header (activity settings button) when it exists |

## Focused message keys
See `shortcuts.md` (E, Backspace, P, +, R, F, S, Ctrl+C, Alt+Enter, Esc, Shift+F10). The client modal lists
Backspace for delete; no Delete key binding was found in the bundle.

## Screen readers (AccessKit)
The GPUI fork ships AccessKit 0.24 and the Linux platform (`gpui-pre-linux` 0.3.12) runs the
`accesskit_unix` adapter (AT-SPI) on X11 and Wayland. A node is exported when its element has an id and a
role. `div` exposes role, label, description, selected, expanded, toggled, values and positions; there is no
live-region setter, so `ui/a11y.rs` wraps an element and sets `Live` on its node (same pattern as
gpui-base `A11yDisabled`).

| Element | Client (DOM) | Phoenix |
|---|---|---|
| Server rail | `nav` "Servers sidebar", `tree` / `treeitem` | ✅ server rows `TreeItem` + name + selected; ⚠ no tree container node yet |
| Channel list | links with a label and `aria-selected` | ✅ `Link` + name + selected (client label adds "(text channel)", lock state: not reproduced) |
| Messages | `ol role=list` "Messages in {channel}", each message `role=article` labelled by author, content and accessories | ✅ `List` + "Messages in …", `Article` labelled "author, content" |
| Members | `role=list` "Members", rows `listitem` | ✅ |
| Back / account buttons | `role=button` "Go back", "Manage profile and status" | ✅ |
| Announcer | two `role=log` regions (`aria-live` assertive and polite, `aria-relevant=additions`) used by `announce()` for actions, e.g. "message deleted" after a delete | ✅ `ui/a11y.rs` `announcer`, wired to message deletion |
| New messages | the client does not announce incoming messages (the list is not a live region; TTS is the separate "Read all messages out loud" setting) | ✅ same: no live region on the list |
| Composer | `role=textbox` "Message #channel", `aria-multiline`, `aria-autocomplete=list` | ⚠ gpui-component input role only, no label |
| Not measured with a real screen reader | — | ⚠ Orca was not run on this machine |

## Accessibility settings
Already wired and documented in `settings-wiring.md`: Reduced Motion and its system sync, Saturation and
"Apply saturation to custom colors", High Contrast Mode, chat font size (12–24 px, `theme` `FONT_PX`), zoom
(Ctrl +/−/0, the client's 50–200 % steps), Always underline links, role color display, switch indicators.

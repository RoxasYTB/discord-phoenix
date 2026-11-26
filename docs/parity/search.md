# Message search parity

Sources: the client's search module (`95de45d742273fa4.js`, chunk 183752, module 586944: popout,
keyboard, history), the token table (`web.b63d846b329d6470.js` module 224467: `ea()` filters,
`j()` dates, `et()` users, `en()` channels), the FR/EN string tables (`5ef1f6e6b679dd77.js`,
`c30b23bab1166ada.js`), and live CDP readings of the autocomplete popout in the user's current
route, 2026-11-26 (typing `de:`, `contient:`, `avant:`, `épinglé:`, `auteur:` and clearing it; no
non-GET request seen). Private dumps: `/mnt/windata/project-phoenix/compare/func-keys-ac-*.json`.
Code: `phoenix-api/src/rest/search.rs` (parser), `ui/search_query.rs` (keys, dates, answers),
`ui/search.rs` (popout, keyboard).

## Filters
| Item | Client | Phoenix | Status |
|---|---|---|---|
| filter keys | localized (`1TUdFo` from/de, `i96lO+` mentions, `CqCvir` has/contient, `qZ+7BA` before/avant, `KSDx7M` after/après, `tIxkOo`+`h2NzSd` on·during/le·pendant, `WNpFHa` in/dans, `0B74eY` pinned/épinglé, `us8IQi` authorType/auteur), case-insensitive | same words from the locale files, English keys still accepted | ✅ |
| has: values | localized words (`Y()`): link, embed, poll, forward→`snapshot`, file, video, image, sound, sticker | same, plus `has:forward` → `has=snapshot` | ✅ |
| authorType: values | user/bot/webhook localized, `-` prefix negates | same | ✅ |
| pinned: | only `true` / `false` | same (was: anything else meant false) | ✅ |
| dates (`j()`) | today, yesterday, week, month, year (localized), month names, weekday names (current locale week), `YYYY` from 2015 to now = the whole year, otherwise parsed as `YYYY-MM-DD` (so `2026-05` is May 1); before = start of the range, after = end of the range, local time | same rules, local midnight snowflakes | ✅ |
| other filters | linkFrom, fileType, fileName | not parsed | ❌ not built |

## Popout
| Item | Client | Phoenix | Status |
|---|---|---|---|
| filter mode (cursor in `key:value`) | only the answer group: header `ed()` (From User `catERA`, Mentions User `l3K4B/`, In Channel `vHyCgl`, Message Contains `IC7gHM`, none for pinned/author type: 8px empty header), rows 28 (26 without icon) with 4px gap, `item_lm2_KW` padding 4 8, radius 8, first row selected (`rgba(151,151,159,.12)`); users: 20px avatar, name body-sm/semibold text-default + username body-sm/medium text-subtle; channels: icon + body-sm/semibold text-strong; has: icon (`H()` icons) + body-sm/medium text-strong; at most 10 users | same layout, up to 10 users and channels | ✅ |
| keyboard | Up/Down wrap, Enter picks the selected row or searches, Esc clears a non-empty query then blurs | same | ✅ |
| picking an answer | replaces the typed value and runs the search (`performSearch: true`) | same | ✅ |
| date filters | a calendar picker (`CalendarPicker`, min 2015-05-15, max today) | the normal popout; the date is typed | ❌ calendar only in the More filters modal |
| free-text mode (`FILTER_ALL`) | "Search for" row, then matching users/channels/has across groups with the key prefixed | "Search for" row and the filter list | ❌ cross-group suggestions not built |
| history | 5 entries, prefix merge, localized key pills | 5 entries, pills now use the localized key | ✅ |

## Requests
| Item | Status |
|---|---|
| server and channel/DM search use `GET …/messages/search` | ✅ |
| "search all DMs" uses the client's `POST /users/@me/messages/search/tabs` (read-only query, but not a GET) | ⚠ kept as merged in 3b1e998d; agents must not run it on the real account |

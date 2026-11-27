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
| linkFrom / fileType / fileName | `RpRAZD` linkFrom/lienDe, `TMNjFm` fileType/typeFichier, `5xtLRC` fileName/nomFichier; answer regex `R` = one non-space word, no autocompletion; `Zf()` sends `getMatch(1)` as `link_hostname`, `attachment_extension`, `attachment_filename` (deduplicated sets) | parsed with the localized and English keys, sent as the same query keys (GET) and in the tabs body; the popout stays closed while typing their value, as the client renders no row for a filter without autocompletions | ✅ |

## Popout
| Item | Client | Phoenix | Status |
|---|---|---|---|
| filter mode (cursor in `key:value`) | only the answer group: header `ed()` (From User `catERA`, Mentions User `l3K4B/`, In Channel `vHyCgl`, Message Contains `IC7gHM`, none for pinned/author type: 8px empty header), rows 28 (26 without icon) with 4px gap, `item_lm2_KW` padding 4 8, radius 8, first row selected (`rgba(151,151,159,.12)`); users: 20px avatar, name body-sm/semibold text-default + username body-sm/medium text-subtle; channels: icon + body-sm/semibold text-strong; has: icon (`H()` icons) + body-sm/medium text-strong; at most 10 users | same layout, up to 10 users and channels | ✅ |
| keyboard | Up/Down wrap, Enter picks the selected row or searches, Esc clears a non-empty query then blurs | same | ✅ |
| picking an answer | replaces the typed value and runs the search (`performSearch: true`) | same | ✅ |
| date filters | `eB`: when the mode filter is a date (`S.av`), the listbox is `eU` = `CalendarPicker` (module 939538: `value` defaults to today, `fixedHeight` so always 6 weeks, `minDate` 2015-05-15, `maxDate` today) inside `container_cbVzxY datePickerContainer_cbVzxY` (padding 0) with `calendar_cbVzxY` (314 wide content, flex centered, so 356 with the 20px padding and border); a pick writes `YYYY-MM-DD ` (`ump`) over the answer and searches | the same calendar (`search_filters::calendar_box`, shared with the More filters modal) inside the popout: today selected and bold, 6 weeks, no previous arrow on May 2015, future days and days before 2015-05-15 disabled, month arrows keep their month while the filter token stays the same; a pick completes the token and runs the search | ✅ from JS/CSS (⚠ not compared live: the official client was not running with CDP during this pass) |
| free-text mode (`FILTER_ALL`) | `Gk()`: no filter at the cursor. Rows: "Search for {value}" (`rCnaoo`); then, when `SearchAutocompleteStore.D()` finds answers for the trailing word in FROM, MENTIONS and IN (set `y`, at most 3 each, IN only where eligible), a single "Add search filters" row (`M1tf+7`, no sublabel) and one group per filter (`ed()` titles), else the Filters group (`UdhTtk`, From/In/Has/Mentions + "More filters" `diOL4i` / "dates, author type, and more" `1axf1T`). No history in this mode (EMPTY only). Group rows: UserIcon / AtIcon / TextIcon 20 (`itemIcon_lm2_KW`), user label = 16px avatar + name body-sm/semibold text-default, sublabel `filterSublabelTextContainer` (gap 4) = key body-sm/semibold text-subtle + username body-sm/medium text-muted; channel label = key body-sm/medium text-strong + 14px channel icon (text-default, mr 4) + name body-sm/semibold text-strong. A pick replaces the word with `key value` without searching (`performSearch: false`) | same rows, groups and limits (`search_query::cross_answers`), picking writes `key:value ` over the trailing word | ✅ from JS/CSS (⚠ not compared live, same reason) |
| keyboard over every row | `eq`: one index over all rows (Search for, filters, history, groups); FILTER mode starts on row 0, the other modes on none (-1); Up/Down wrap; Enter on none searches | same (`Layout::rows`, `first_selected`) | ✅ |
| group dividers | `eE` draws `divider_lm2_KW` (1px border-subtle, margin 8 -8) before every group that does not start at index 0 | same (so a divider now follows "Find in" in the all-DMs empty popout) | ✅ |
| history | 5 entries, prefix merge, localized key pills, shown only in EMPTY mode | 5 entries, pills now use the localized key, empty query only | ✅ |

## Requests
| Item | Status |
|---|---|
| server and channel/DM search use `GET …/messages/search` | ✅ |
| "search all DMs" uses the client's `POST /users/@me/messages/search/tabs` (read-only query, but not a GET) | ⚠ kept as merged in 3b1e998d; agents must not run it on the real account |
| 202 "index not ready" (chunk `9329220d4bdc3583.js` module 177068 `SearchFetcher`): retry after `retry-after` seconds (5 s when missing or 0), at most 5 retries, the results show the context text from `Y7()`: `AXPbZr` (server), `Q0JJjv` (DM), `Br0xJA` (all DMs) | the backend returns the 202 at once (`report_indexing`), the panel shows the same text and retries up to 5 times (body `retry_after`, else 5 s); the member page search keeps its own blocking retries | ✅ (⚠ the results chunk that lays this text out is not in the local cache: drawn with the empty-state layout) |

## Search tabs (live check 27/11)

Source: live chunks fetched from the CDN for build 681625 (`065be327ed5934fd.js` module 753806 `fetchMessages`,
module 256796 `fetchTabMessages`, `9329220d4bdc3583.js` module 177068 `SearchFetcher`, web bundle module 768570).
No search was run in the client and no `…/search/tabs` request was sent for this check.

| item | client | Phoenix | status |
|---|---|---|---|
| server, channel and DM search | `MS` fetcher: `GET …/messages/search`, no tabs in the results panel | same | ✅ |
| search all DMs | `DX` fetcher: `POST /users/@me/messages/search/tabs` with the single `messages` tab, limit 25 | same | ✅ |
| tab filters (module 768570 `B7`/`su`) | `messages {}`, `media {has:[image,video]}`, `links {has:[link]}`, `files {has:[file]}`, `pins {pinned:true}`, `us = {sort_by:"timestamp", sort_order:"desc"}`; on desktop only used by the Mod View member panel (counts of messages, links and media through `POST /guilds/{id}/messages/search/tabs`) | the results panel has no tabs, like the client; the Mod View counts are not built (a GET `…/messages/search?author_id=…&has=link` gives the same totals) | ⚠ |


## Results in threads

| Element | Client | Phoenix | State |
|---|---|---|---|
| group header of a hit in a thread | `SEARCH_MESSAGES_SUCCESS` carries `threads: body.threads ?? []` (classic reply) or `tab.threads` (tabs reply), chunk `fb74ed8e7dc14b59.js`; the channel store then knows archived threads too | `SearchResponse.threads` (lenient parse), `SearchPage.threads`, `search::adopt_threads` adds them to `app.channels`; a hit whose channel is still unknown gets one `GET /channels/{id}` (at most 5 per page); the header falls back to `nav.find_thread` | ✅ unit test `replies_keep_the_threads_of_their_hits`; was `# inconnu` before (QA 27/11) |

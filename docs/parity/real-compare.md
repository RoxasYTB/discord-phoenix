# Real-data compare sweep (Discord client vs `real_shots`)

Scores only: the captures show the user's real account and stay in
`/mnt/windata/project-phoenix/compare/sweep/` (never in git).

## Method (2026-11-24, wt/sweep)

- Client: the live 2026 client over CDP, Onyx, cozy, font 16, page zoom 125 %. Each view is
  one short CDP pass (2–9 s) under `flock /tmp/discord-cdp-driver.lock`:
  `Emulation.setDeviceMetricsOverride` to W×H **CSS px** at dpr 1.5 (width/height × zoom,
  device scale 1.5 / zoom, because the emulated size is in DIP), open the view, read the region
  boxes and the state `real_shots` needs (scroll anchors, sidebar width, audio, folders),
  screenshot, undo our own actions, `Emulation.clearDeviceMetricsOverride`.
  `Network.enable` logs every non-GET request of the pass.
- Phoenix: int-bin `real_shots` at the same W×H, `PHX_SCALE=1.5`, rendered after the pass
  from the saved state (`PHX_SCROLL`, `PHX_SIDEBAR_W`, …) so the client is not held.
- Score: `scripts/compare/compare.py --discord … --phoenix … --regions boxes.json`,
  `mismatch_pct` = share of pixels whose difference exceeds 24/255, per region.
- Skipped: a pass is skipped when the route changed since the previous pass (the user is
  using Discord: 5 min pause), when the user is typing, or when a modal of someone else is
  open. Views that did not open as expected are discarded.

Caveats: the two sides are a few seconds apart, so new messages, typing indicators, live
timers and presence changes count as mismatch. The messages region also counts scroll
offsets (when the client sits above the history Phoenix has loaded). Rows are for the route
the client showed at that time (guild A = 18-member text channel, guild B = 44-member text
channel).

Views not measured:
- Home / DMs: the client was never on `/channels/@me`, and the sweep does not navigate. Compared offline instead (wt/mute): `real_shots PHX_DM=<the dumped DM>` 1280×700 at dpr 1.5 against `dumps/vanilla-dm.png` (2026-11-23): title bar 2.7 %, rail 10.6 %, channel list 19.9 %, header 3.3 %, messages 42.6 %, composer 1.0 %, overall 28.6 %. The rail, list and messages differ by content (one day apart: new messages, DM order, NOUVEAU pill); layout and header line up. The dump's DM profile panel shows the full-art (hovered / animated theme) state, which a still dump cannot score: ➖ live measure only when the user opens a DM; the agents never navigate there (it would ack the DM).
- Profile popout: dropped after one pass, because an `ack` POST left the client while the
  popout was open. It was the client's own read ack for a message that arrived in the open
  channel, but the brief says to close such a view and not reopen it.
- Search at 1536×788: discarded (the search did not open, another agent's modal was on top).

## Before (int-bin 2f62f74)

Mismatch % per region.

| Size | View | Title bar | Rail | Channels | Header | Messages | Composer | Members | User panel | Inbox | Search | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1280×720 | channel (B) | 1.8 | 4.6 | 3.1 | 4.2 | 26.0 | 4.8 | 19.0 | 8.0 | — | — | **14.6** |
| 1280×720 | inbox (B) | 1.9 | 4.6 | 3.1 | 4.6 | 15.3 | 4.8 | 16.5 | 8.0 | 9.4 | — | **10.3** |
| 1280×720 | search (B) | 1.8 | 4.6 | 3.1 | 4.1 | 28.6 | 4.5 | — | 8.0 | — | 15.3 | **17.4** |
| 1536×788 | channel (A) | 2.5 | 30.7 | 2.9 | 3.6 | 8.0 | 2.5 | 7.5 | 8.2 | — | — | **7.2** |
| 1536×788 | inbox (B) | 1.5 | 4.6 | 3.0 | 3.3 | 12.3 | 5.9 | — | 8.2 | 9.4 | 15.0 | **9.0** |
| 1920×1080 | channel (B) | 1.2 | 31.6 | 3.2 | 2.4 | 4.3 | 2.3 | 17.9 | 8.1 | — | — | **6.7** |
| 1920×1080 | inbox (B) | 1.2 | 31.6 | 3.2 | 2.7 | 7.8 | 2.3 | 16.6 | 8.1 | 11.6 | — | **8.5** |
| 1920×1080 | search (B) | 1.2 | 31.6 | 3.2 | 2.1 | 9.5 | 2.8 | — | 8.1 | — | 23.9 | **9.1** |

## After (int-bin 478aa7f, same method, 15:16–15:20)

All rows on guild B (the client stayed there). Search at 1536×788 measured this time.

| Size | View | Title bar | Rail | Channels | Header | Messages | Composer | Members | User panel | Inbox | Search | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1280×720 | channel | 1.8 | 4.4 | 3.0 | 4.3 | 30.6 | 4.7 | 16.3 | 7.9 | — | — | **15.8** |
| 1280×720 | inbox | 1.9 | 4.4 | 3.0 | 4.6 | 10.4 | 4.8 | 14.2 | 7.9 | 11.1 | — | **8.0** |
| 1280×720 | search | 1.8 | 4.4 | 3.0 | 3.8 | 29.4 | 4.5 | — | 7.9 | — | 15.2 | **17.5** |
| 1536×788 | channel | 1.5 | 4.2 | 3.1 | 3.2 | 4.6 | 3.3 | 15.5 | 8.0 | — | — | **5.7** |
| 1536×788 | inbox | 1.5 | 4.2 | 3.1 | 3.6 | 10.8 | 3.4 | 13.6 | 8.0 | 11.7 | — | **8.2** |
| 1536×788 | search | 1.5 | 4.2 | 3.1 | 2.9 | 21.4 | 4.3 | — | 8.0 | — | 15.3 | **14.2** |
| 1920×1080 | channel | 1.2 | 3.8 | 3.2 | 2.4 | 8.4 | 2.3 | 15.0 | 7.9 | — | — | **7.5** |
| 1920×1080 | inbox | 1.2 | 3.8 | 3.2 | 2.7 | 12.6 | 2.3 | 13.5 | 7.9 | 13.3 | — | **9.6** |
| 1920×1080 | search | 1.2 | 3.8 | 3.2 | 2.1 | 12.9 | 2.8 | — | 7.9 | — | 23.9 | **10.3** |

Reading: the rail drops from 30–32 % to 4 % wherever the client rail was scrolled. The user
panel now shows the custom status (the 8 % left is text antialiasing and the ellipsis). The
messages region is decided by timing (new messages between the two captures) and by the
client sitting above the loaded history at 1280×720 (30 %). Member list scores are left to
wt/members2. Search results (15–24 %) are being rebuilt on wt/search.

## Gaps found, by visibility

| Gap | Cause (observed) | State |
|---|---|---|
| Server rail 30 % off whenever the client rail is scrolled | `real_shots` did not copy the rail `scrollTop` (91 px measured) | ✅ compare tool fixed (rail field in `SCROLL_JS`) |
| Member list "Activité — 6/7" vs "Activité — 1" / none | Phoenix counted feed entries by non-members as pending; see `responsive.md` | ✅ |
| Activity card "il y a 11min" vs green "12:08" timer | live entry (trait 3 `is_live`) times from `started_at` | ✅ |
| User panel "Ne pas déranger" vs the custom status; own member row without it | the member list sync sends our own row with `activities: []` (checked on the gateway: READY and SESSIONS_REPLACE carry the custom status), and that reset the own activities | ✅ |
| Header bell: the client shows the slashed bell on guild A's channel, Phoenix the plain bell | Client chat header `nH`: `BellSlashIcon` when `isChannelMuted \|\| resolvedMessageNotifications(channel) !== ALL_MESSAGES` (level: channel override, then category, then the server entry, then the server `default_message_notifications`). Guild A resolves to Only Mentions. `notifications::NotifyPrefs::header_bell_slashed` (wt/mute). Mute chain `isGuildOrCategoryOrChannelMuted` with `mute_config.end_time` (`tG`) is shared by notifications, guild unread, server icon and menus | ✅ wt/mute (read from READY only, no client menu opened) |
| Member list nameplate differs on one member (another image), nameplate brightness too high | Per-guild `member.collectibles.nameplate` was not parsed/overridden in `member_user`; `img_AFi54G` `mask-image: linear-gradient(to right, rgba(0,0,0,0.3) <fade>px, rgb(0,0,0) <fade+50>px)` attenuates nameplate art behind text while revealing right-side art at full `0.6` opacity on short rows (e.g. `Clem`: `151.5px -> 201.5px`, `bFant0m3`: `220px -> 270px`, measured live via CDP 2026-11-24). Dynamic fade mask + `bot_tag_px` double-margin removal (`wt/diff-members`, `957ff645`) dropped `member list` mismatch from 11.8 % (mean 3.9 %) to 10.8 % (mean 3.6 %) at 1536×840 dpr 1.25 | ✅ wt/sweep-audit + wt/diff-members |
| Nameplate "cut" at the right edge of member / DM rows: bright 2–4 dp strip on the last pixels, art washed out towards the right, texts of plated rows judged narrower | Row geometry already matched (live CDP 2026-11-24, 1536×840 @125 %: `childContainer_J1Adu_ nameplated_J1Adu_` 252 px, x 1276→1528, padding `0 32px 0 10px`; scroller `scrollbarGutterStable thin` 268 → content 260, so the 8 px gutter is reserved, not overlaid; `img_AFi54G` 223.925×41.99, `left 28.075px right 0`, `object-fit: fill`, `object-position: 100% 50%`, natural 448×84; one-line rows 213×40, DM 256×48). Real causes: (1) the baked mask clamped its end ratio to 1, so `fade` 220 on a 224 px image ramped .3→1 over the last 4 px instead of reaching .356 (client formula `linear-gradient(to right, rgba(0,0,0,.3) Fpx, #000 F+50px)` runs past the box); (2) the palette gradient was painted over the image, while it is `container_AFi54G`'s background, under the image (SBLOW right edge: client (36,10,38), before (29,12,44)); (3) `fade` was a text-length guess, the client reads `layout_` `contentRect.width + 10` (member list) / `- 5` (DM `layout_TBNqol`, ref `ti`) from a `ResizeObserver` (web.b63d846b329d6470.js module 351952). Phoenix now measures the same box with a probe (`nameplate::width_probe`) | ✅ wt/np-edge |
| Plated rows ellipsize one glyph earlier ("Happy 03…" vs "Happy 03/1…") | Same limit (1496 CSS px, `padding-right 32`), but GPUI's `truncate_line` trims trailing ASCII punctuation and whitespace before the ellipsis (`vendor/gpui-pre` line_wrapper.rs) and uses per-char advances; CSS `text-overflow: ellipsis` keeps them. Fixed by GPUI patch 4 `truncate_line_end_shaped` (docs/gpui-patches.md, text.md "Ellipsis": longest grapheme prefix + "…", punctuation kept; live 1280×720 identical) | ✅ |
| Member list one name ellipsized early (combining marks / emoji in styled name) | member list zone: cluster font fallback for combining marks, GPUI patches 5–7 (text.md "Combining marks": every glyph within 1 device px of the client capture, no early ellipsis) | ✅ |
| Search results panel: old layout (query field, "Plus récents / Plus anciens" tabs, plain author names, no highlight, no reply line, no embeds) vs the client's "N résultats" + Filtres + Trier header and full message cards | search results rebuilt by wt/search (MERGED 58273ab): "N résultats" header, Filtres / Trier, full message cards, pager (pickers.md search rows) | ✅ |
| Voice channel member rows: the client shows "Muet" + "En sourdine" for a self-muted, self-deafened member; one Phoenix render had the row without the icons, a later one had no voice rows at all | Three causes: `apply_ready` read the old voice users after `clear_session` had emptied `channels` (dead code, so the users from READY_SUPPLEMENTAL were lost when the ReadyData came after it); voice states from GUILD_CREATE or already cached at READY never sent their `VoiceFlags` (row without icons); my own row used the local mic buttons even when another client of mine held the call. Fixed, test `voice_users_survive_ready_after_supplemental`. Icons measured live (2026-11-24): `icon_YUoOxm` 16×16, gap 4 (`iconGroup_`), `--channels-default`, `iconServer_` `--icon-feedback-critical`, order Muet then En sourdine, no hover rule; glyphs = extracted MicrophoneSlashIcon / HeadphonesSlashIcon | ✅ wt/mute |
| Channel list "Nouveaux messages non lus" pill missing | compare artefact: with `NO_ACKS` the selected channel stays unread in `real_shots`, which hides the pill | — (not a product bug) |
| Message & reply usernames missing role color gradients (`twoColorGradient_` / `usernameGradient_`) | `chat.rs` used `emoji_text::label` for message authors and `fonts::name_text` for reply usernames instead of `role_gradient::for_author` + `fonts::name_text_with` (verified live via CDP, 2026-11-24) | ✅ wt/diff-messages (`957ff645`) |
| `article` link embeds rendered large preview image as 80×80 side thumbnail | `chat.rs` `embed()` only promoted `video` thumbnails to full-width media; Discord `sanitizeEmbed` also promotes `article` thumbnails (verified live on `immich.app` 1024×576 embed) | ✅ wt/diff-messages (`957ff645`) |
| Two-item media mosaic (`oneByTwoGrid_`) too short (~155px vs 273px), shifting all messages above by ~100px | `mosaic.rs` `Row::Two` used `row_aspect(its) * 2.` instead of 1:1 square cells (`ratio_box(2., 280.)`, measured live 550×273 / 273×273 per cell); `messages` mismatch dropped 31.1 % → 10.2 %, overall 17.1 % → 7.5 % | ✅ wt/diff-messages (`635719ae`) |
| Composer 5.4% → 2.3%, Header 3.5% → 2.4%, selected channel icons | Composer `channelTextArea_` border 0.8px vs GPUI 1px shifted the whole box 1px up (`mb` -0.8px, placeholder `top` -1.5px, buttons `mr` 13px); header search bar placeholder `pl` 8px / `lh` 18px and icon `--icon-muted`; selected channel action icons `--interactive-text-active`; thread row unread pill (`wt/diff-chrome-composer`, 957ff645) | ✅ wt/diff-chrome-composer |

## Sweep 2 (2026-11-25, wt/sweep2): popouts, settings, voice rows

Same method, extended driver `~/discord-phoenix-ref/sweep/{sweep2.mjs,run2.py,boxes2.js}`
(views `channel members phx phxmembers pins threads emoji gif sticker plus inbox search
set-<page>`; the popout box is the last `layer_` of `layerContainer_`, the settings boxes are
`aside.sidebar_cR4ic2` and the rest of the layer). Sizes 1536×840 and 1280×720, dpr 1.5,
client page zoom 125 %. `real_shots` now follows the client member list toggle
(`PHX_MEMBERS`), so views where the user hides the list are scored without it.

Notes on the passes:
- The client sends `POST /science` and `/metrics/v2` (telemetry) on every popout, and the read
  `ack` of the open channel (#général, or #phoenix when the pass visits it) when a message
  arrives during inbox / search. No other non-GET request was seen.
- The members pass first left the client member list open (the undo looked for the "Afficher"
  label, which becomes "Masquer la liste des membres" once open). It was closed by hand right
  after; the driver now clicks the "Masquer" label.
- The 2026 settings sidebar has no "Raccourcis clavier" entry, so there is no keybinds page to
  compare.
- Scores of the "after" table were rendered from the saved client state with int-bin 492a6177,
  after the GPUI baseline fix (5b4da8a5). The intermediate int-bin d9c87f70 drew every text
  about 5 px low (the channel list header was clipped), so its scores are not used.

### Before (1536×840 int-bin 20926677, 1280×720 int-bin 9889a054)

| Size | View | Title bar | Guild rail | Channel list | Header | Messages | Composer | Member list | User panel | Popout | Search results | Settings nav | Settings content | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1536×840 | channel | 1.5 | 2.9 | 3.9 | 2.5 | 12.5 | 10.8 | — | 7.2 | — | — | — | — | **9.3** |
| 1536×840 | members | 1.5 | 2.9 | 3.9 | 2.5 | 7.0 | 4.1 | 9.1 | 7.2 | — | — | — | — | **6.0** |
| 1536×840 | phx | 1.5 | 2.8 | 2.6 | 2.8 | 11.9 | 10.4 | — | 7.2 | — | — | — | — | **8.6** |
| 1536×840 | pins | 1.5 | 2.9 | 3.9 | 2.5 | 18.5 | 10.4 | — | 7.2 | 18.5 | — | — | — | **12.8** |
| 1536×840 | threads | 1.5 | 2.9 | 3.9 | 2.6 | 9.8 | 4.1 | 8.9 | 7.2 | 12.1 | — | — | — | **7.2** |
| 1536×840 | emoji | 1.5 | 2.9 | 3.9 | 2.5 | 23.9 | 3.8 | 8.8 | 7.2 | 42.1 | — | — | — | **13.6** |
| 1536×840 | gif | 1.5 | 2.9 | 3.9 | 2.5 | 14.9 | 3.7 | 8.8 | 7.2 | 20.1 | — | — | — | **9.5** |
| 1536×840 | sticker | 1.5 | 2.9 | 3.9 | 2.5 | 33.4 | 10.3 | — | 7.2 | 48.1 | — | — | — | **21.7** |
| 1536×840 | plus | 1.5 | 2.9 | 3.9 | 2.6 | 13.4 | 11.6 | — | 7.2 | 12.0 | — | — | — | **9.9** |
| 1536×840 | inbox | 1.6 | 2.9 | 3.9 | 3.2 | 18.9 | 10.8 | — | 7.2 | 29.5 | — | — | — | **13.1** |
| 1536×840 | search | 1.5 | 2.9 | 6.5 | 2.8 | 15.9 | 5.6 | — | 7.2 | — | 20.3 | — | — | **11.7** |
| 1536×840 | set-account | 0.6 | 0.3 | 2.2 | 0.1 | 1.7 | 2.8 | — | 2.0 | — | — | 2.8 | 1.6 | **1.7** |
| 1536×840 | set-privacy | 0.6 | 0.3 | 2.4 | 1.0 | 9.6 | 4.7 | — | 2.0 | — | — | 3.1 | 8.1 | **6.6** |
| 1536×840 | set-appearance | 0.6 | 0.3 | 11.5 | 0.1 | 6.7 | 9.5 | — | 28.1 | — | — | 15.9 | 6.2 | **7.1** |
| 1536×840 | set-voice | 0.6 | 0.3 | 12.1 | 0.2 | 4.9 | 3.6 | — | 2.7 | — | — | 16.7 | 4.3 | **5.8** |
| 1536×840 | set-notifications | 0.6 | 0.2 | 2.0 | 0.2 | 4.6 | 2.8 | — | 2.0 | — | — | 2.5 | 3.9 | **3.4** |
| 1280×720 | channel | 1.8 | 3.0 | 2.8 | 3.3 | 19.0 | 10.7 | — | 7.2 | — | — | — | — | **12.0** |
| 1280×720 | members | 1.8 | 3.0 | 2.6 | 3.1 | 13.0 | 4.4 | 9.7 | 7.2 | — | — | — | — | **7.9** |
| 1280×720 | phx | 2.7 | 3.3 | 7.6 | 5.6 | 6.5 | 5.0 | — | 8.8 | — | — | — | — | **6.3** |
| 1280×720 | pins | 1.8 | 3.0 | 2.6 | 3.1 | 12.2 | 4.4 | — | 7.2 | 17.9 | — | — | — | **7.9** |
| 1280×720 | threads | 1.8 | 3.0 | 2.6 | 3.2 | 6.0 | 3.3 | — | 7.2 | 9.8 | — | — | — | **4.5** |
| 1280×720 | emoji | 1.8 | 3.0 | 2.6 | 3.0 | 28.9 | 3.6 | — | 7.2 | 42.0 | — | — | — | **16.7** |
| 1280×720 | gif | 1.8 | 3.0 | 2.6 | 3.0 | 19.0 | 3.4 | — | 7.2 | 21.8 | — | — | — | **11.5** |
| 1280×720 | sticker | 1.8 | 3.0 | 2.6 | 3.0 | 30.6 | 3.4 | — | 7.2 | 45.2 | — | — | — | **17.6** |
| 1280×720 | plus | 1.8 | 3.0 | 3.7 | 3.0 | 8.9 | 4.6 | — | 7.2 | 15.0 | — | — | — | **6.4** |
| 1280×720 | inbox | 1.9 | 3.0 | 3.7 | 4.1 | 22.0 | 3.2 | — | 7.2 | 29.2 | — | — | — | **13.4** |
| 1280×720 | search | 1.8 | 3.0 | 3.7 | 3.2 | 18.9 | 4.8 | — | 7.2 | — | 18.1 | — | — | **11.9** |
| 1280×720 | set-account | 0.7 | 1.1 | 2.0 | 0.1 | 1.5 | 2.4 | — | 2.5 | — | — | 2.8 | 1.3 | **1.6** |
| 1280×720 | set-privacy | 1.9 | 1.5 | 5.2 | 1.4 | 11.2 | 8.0 | — | 4.4 | — | — | 7.1 | 9.2 | **8.1** |
| 1280×720 | set-appearance | 1.9 | 5.7 | 8.1 | 0.1 | 5.9 | 1.5 | — | 4.6 | — | — | 12.7 | 4.6 | **5.7** |
| 1280×720 | set-voice | 1.9 | 5.7 | 7.9 | 0.1 | 4.9 | 2.7 | — | 4.6 | — | — | 12.4 | 4.0 | **5.2** |
| 1280×720 | set-notifications | 1.9 | 5.9 | 10.5 | 0.1 | 5.9 | 6.0 | — | 5.1 | — | — | 16.2 | 5.0 | **6.6** |

### After

| Size | View | Title bar | Guild rail | Channel list | Header | Messages | Composer | Member list | User panel | Popout | Search results | Settings nav | Settings content | Overall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1536×840 | channel | 1.5 | 2.8 | 3.4 | 2.3 | 4.3 | 3.2 | — | 7.5 | — | — | — | — | **3.8** |
| 1536×840 | members | 1.5 | 2.8 | 3.4 | 2.4 | 5.8 | 3.6 | 22.2 | 7.5 | — | — | — | — | **7.3** |
| 1536×840 | phx | 1.5 | 2.8 | 3.5 | 2.6 | 4.3 | 3.1 | — | 7.5 | — | — | — | — | **3.8** |
| 1536×840 | pins | 1.5 | 2.8 | 3.4 | 2.4 | 10.6 | 2.7 | — | 7.5 | 18.6 | — | — | — | **7.5** |
| 1536×840 | threads | 1.5 | 2.8 | 3.4 | 2.5 | 4.9 | 3.1 | — | 7.5 | 9.4 | — | — | — | **4.2** |
| 1536×840 | plus | 1.5 | 2.8 | 3.4 | 2.3 | 4.6 | 4.2 | — | 7.5 | 12.6 | — | — | — | **4.0** |
| 1536×840 | inbox | 1.6 | 2.8 | 3.4 | 3.3 | 11.8 | 3.2 | — | 7.5 | 19.0 | — | — | — | **8.3** |
| 1536×840 | search | 1.5 | 2.8 | 3.4 | 2.5 | 12.4 | 5.1 | — | 7.5 | — | 20.6 | — | — | **8.9** |
| 1536×840 | set-privacy | 0.6 | 0.2 | 2.4 | 1.0 | 9.3 | 2.4 | — | 2.0 | — | — | 3.1 | 7.7 | **6.3** |
| 1536×840 | set-appearance | 0.6 | 0.2 | 10.8 | 0.1 | 4.0 | 8.1 | — | 27.9 | — | — | 14.9 | 3.9 | **5.2** |
| 1536×840 | set-voice | 0.6 | 0.2 | 11.1 | 0.2 | 3.2 | 1.0 | — | 3.5 | — | — | 15.4 | 2.7 | **4.4** |
| 1280×720 | channel | 1.8 | 3.0 | 3.7 | 3.0 | 5.7 | 3.8 | — | 7.3 | — | — | — | — | **4.7** |
| 1280×720 | members | 1.8 | 3.0 | 3.7 | 3.2 | 8.8 | 3.9 | 19.7 | 7.3 | — | — | — | — | **8.4** |
| 1280×720 | phx | 1.8 | 3.0 | 3.8 | 3.5 | 6.1 | 3.8 | — | 7.3 | — | — | — | — | **4.9** |
| 1280×720 | pins | 1.8 | 3.0 | 3.7 | 3.1 | 11.3 | 3.9 | — | 7.3 | 17.8 | — | — | — | **7.6** |
| 1280×720 | threads | 1.8 | 3.0 | 3.7 | 3.2 | 4.9 | 2.8 | — | 7.3 | 9.8 | — | — | — | **4.2** |
| 1280×720 | plus | 1.8 | 3.0 | 3.7 | 3.0 | 6.3 | 4.6 | — | 7.3 | 12.6 | — | — | — | **5.0** |
| 1280×720 | inbox | 1.9 | 3.0 | 3.7 | 4.2 | 14.3 | 2.8 | — | 7.3 | 18.9 | — | — | — | **9.3** |
| 1280×720 | search | 1.8 | 3.0 | 3.7 | 3.3 | 14.9 | 4.2 | — | 7.3 | — | 18.3 | — | — | **9.8** |
| 1280×720 | set-privacy | 0.7 | 1.2 | 2.3 | 1.0 | 11.2 | 8.9 | — | 2.5 | — | — | 3.2 | 9.2 | **7.3** |
| 1280×720 | set-appearance | 0.7 | 5.6 | 7.6 | 0.1 | 5.7 | 1.0 | — | 2.6 | — | — | 12.3 | 4.3 | **5.4** |
| 1280×720 | set-voice | 0.7 | 5.5 | 7.3 | 0.1 | 4.4 | 1.6 | — | 2.6 | — | — | 11.8 | 3.5 | **4.7** |
| 1536×840 | emoji | 1.5 | 2.9 | 6.5 | 2.5 | 27.8 | 3.4 | 20.4 | 7.5 | 35.4 | — | — | — | **17.7** |
| 1536×840 | gif | 1.5 | 2.9 | 6.5 | 2.5 | 17.7 | 3.3 | 20.4 | 7.5 | 10.9 | — | — | — | **13.1** |
| 1536×840 | sticker | 1.5 | 2.9 | 6.5 | 2.3 | 21.2 | 2.9 | — | 7.5 | 33.2 | — | — | — | **14.6** |
| 1280×720 | emoji | 1.8 | 3.0 | 2.6 | 3.0 | 26.8 | 3.2 | — | 7.2 | 34.8 | — | — | — | **15.6** |
| 1280×720 | gif | 1.8 | 3.0 | 2.6 | 3.0 | 14.7 | 3.0 | — | 7.2 | 10.2 | — | — | — | **9.1** |
| 1280×720 | sticker | 1.8 | 3.0 | 2.6 | 3.0 | 26.0 | 3.1 | — | 7.2 | 33.2 | — | — | — | **15.2** |

emoji / gif / sticker rows of the after table reuse the client captures of the before pass (the client was restarted on Home before they could be taken again), so their message and member list regions still compare against the earlier client state; only the popout column is meaningful there. Member list rows drift with presence (the re-render is later than the capture).

### Gaps closed by wt/sweep2

| Gap (visibility) | Cause | State |
|---|---|---|
| Channel list "Nouveaux messages non lus" pill spans the whole list (medium) | 2026 pill `bar_t1P8Sn` is sized to its label and centered in `container_t1P8Sn` (flex, justify center, top 8): padding 4 8, radius 20, 0.8 px border, shadow 0 12 24 black 24 % | ✅ `channel_unread_bar.rs` |
| Voice user rows: plain name font, LIVE badge before the mute / deafen icons (medium) | the client uses the display name style font (`dnsFont_*`) on `usernameFont_YUoOxm` and puts `badgeWrapper_` after the icons in `iconGroup_YUoOxm` | ✅ `voice_panel.rs` |
| Member list: speaker icon for a member who streams in voice (small) | client draws `ScreenIcon` 12 px `#45a366` ("En partage d'écran") | ✅ `members.rs` |
| Pins / inbox / search cards: no avatar decoration (medium) | `cozy_message` and `result_card` drew the bare avatar | ✅ `inbox.rs`, `search.rs` |
| Threads popout 480 px instead of 537.6 at 1536 (medium) | `.browser_l-3BKx { width: 35vw; min-width 480px; max-width 600px }` | ✅ `inbox.rs` (+ `title_bar::viewport_w_css`) |
| Threads popout "Commencé par" name in normal weight, bright bullet (small) | `startedByName_yRVHqK` 600 + role color, `bullet_yRVHqK` rgba(151,151,159,.08), padding 0 8 | ✅ |
| "+" menu: upload and thread icons (small) | client `FileUpIcon`, `ThreadPlusIcon` (React fiber names) | ✅ `composer.rs`, `icons/names.rs` |
| Pin system message: arrow icon, no links (medium) | client `PinIcon` 18 px, "un message" and "messages épinglés" links in `--text-strong` 500 | ✅ `chat/system.rs` |
| Données et confidentialité: links missing in four descriptions (small) | "désactivant" / "supprimant" and "En savoir plus" are links in the live page | ✅ `settings/pages.rs`, `fr.yml` |
| Sticker picker footer "de Fréquemment utilisés" (small) | the footer names the pack or server ("de Wumpus Beyond"), never the section | ✅ `pickers/sticker.rs` |
| Expression picker 18 px too far right (medium) | live `contentWrapper_58b0l2` right edge 1502 at 1536 wide: 34 px from the chat edge, bottom 75.6 | ✅ `pickers/mod.rs` |

### Gaps still open, by visibility (for other agents)

| Gap | Probable cause | Where |
|---|---|---|
| ✅ (wt/invites) — Invite links (`discord.gg/…`) render as plain links: no "Tu as été invité(e)" card, no invalid-invite card, in chat and in pins | no invite embed at all (needs `GET /invites/{code}?with_counts=true` and the card) | `ui/chat.rs` embeds, `ui/inbox.rs` `cozy_message` |
| ✅ (wt/invites) — Emoji picker: no "Émojis populaires sur <serveur>" section (trophy rail icon, first row) and no "POPULAIRES" badge in the footer | client section `TOP_GUILD_EMOJI` (`TrophyIcon`, `topEmojiSectionContainer_6OcK5D`), data from the guild top-emojis store | `ui/pickers/` |
| ✅ (wt/unreads) Inbox "Messages non lus" — the list is built (wt/popouts) and loads on the real account; cards were blank because embeds were not drawn, the header lacked the check-double icon (icon extractor picked `ChatCheckIcon` from an earlier file), and the card bell / collapse buttons were missing | see popouts.md "Inbox > Unreads" | `ui/inbox.rs`, `icons/mod.rs` |
| ✅ (wt/details2) **Medium** — "+" menu 30–40 px lower than the client | client bottom = plus button top − 8; Phoenix bottom lands ~30 px under it (ATTACH_BTN bounds or anchored offset) | `ui/chat/composer.rs` `attach_menu` |
| **Medium** — "+" menu "Programmer un message" disabled, no chevron submenu | scheduled messages not implemented (client: `CalendarPlusIcon` + `ChevronSmallRightIcon`) | `composer.rs` |
| ✅ (wt/unreads) Threads popout "Commencé par" empty for an owner outside the member cache | `parity()` now sends archived thread / forum post owners and first authors to `ensure_authors` (op 8 for the missing ones) | `discord_backend.rs` |
| ✅ (wt/unreads) Search results: the name was ellipsized instead of wrapping the timestamp | the client header flows inline, so the time goes under a long name (ml 4); Phoenix header is now `flex-wrap` with no truncation. Server avatars: results already go through `ensure_message_authors`; the after capture shows them | `ui/search.rs` `result_card` |
| ✅ (wt/invites) — Frequent emoji / sticker order differs (✅ ❤ … vs ❤ …, Wave first in the client) | frecency scoring | `ui/pickers/` |
| ✅ (wt/unreads) Channel list showed the top pill where the client shows the bottom one | the rule (`ChannelListUnreadsStore` `rh`) was already the client's; rows below the rendered range of the virtual list had no bounds and were skipped, so the bottom side looked empty. Unmeasured rows after the scroll top now count as below. Still open: the client's "N in voice" bottom bar (`voice-channels` mode) | `channel_unread_bar.rs` |
| ✅ (wt/details2) **Medium** — Settings: the selected page does not scroll into view in the sidebar, Voix & Vidéo device names "Par défaut" vs the real names, modal ~3 px lower at 1280×720 | sidebar scroll, audio device enumeration | `ui/settings/` |
| ✅ (wt/details2) **Small** — Voice row avatars sharper than the client | client loads `size=32` for a 24 px avatar at dpr 1.5 | `ui::avatar` size choice |
| ✅ (wt/details2) **Small** — GIF picker "Favoris" tile shows another favorite | client uses the most recent favorite | `ui/pickers/` |
| ✅ (wt/details2) **Small** — Threads search field: client grows it (`container_VhFsFg` flex-grow 1), Phoenix 248 px | | `ui/inbox.rs` |
| — Voice message "0:02" vs "0:05" | client playback state (it had been played), not a product gap | — |

### Gaps closed by wt/details2 (measured live through CDP, 2026-11-25)

| Gap | Client reference | Fix |
|---|---|---|
| "+" menu 32 px too low | `menu_M3qk4j` layer `bottom: 62.8px; left: 441px` at 1536×840: menu bottom 777.2 = "Plus d'options de message" button top 785.2 − 8, left = button left. Same rule at 1280×720 | the `ATTACH_BTN` probe canvas was `absolute()` without insets inside a block container, so Taffy put it at its static position under the 32 px button. `composer.rs` adds `top_0().left_0()` |
| Expression pickers (emoji / GIF / stickers) anchor | layer `bottom: 75.6px; right: 10px`, `contentWrapper_58b0l2` 498×486, right edge 1502 at 1536 (unchanged since wt/sweep2) | ✅ already right; the Applications launcher was not opened (it writes `settings-proto/1`) |
| Settings sidebar does not reveal the selected page | `scrollSidebarNodeIntoView(node, {block: "nearest"})` with `padding: 12` on every settings navigation (web.js, settings scroll manager) | `settings/view.rs` `reveal_probe`: nearest scroll with a 12 px margin when the page changes; the sidebar starts at the top when settings open |
| Voix & Vidéo device select shows "Par défaut" | the default entry is the native default name with "Default" replaced by `Q3YKwS` ("Paramètres audio système par défaut"), e.g. "Paramètres audio système par défaut: Logitech G PRO X Gaming Headset Mono"; `deviceLabel_bUNrgk` 16/20 weight 500, `white-space: nowrap; overflow: hidden; text-overflow: clip` | `settings/voice.rs`: default label = `voice.system_default` + ": " + PipeWire/PulseAudio default (`pactl get-default-source/sink`, first in the list), weight 500, clipped. Read-only: Phoenix never changes the system default |
| Settings modal 3 px low at 1280×720 | modal frame 40→680 / 40→1240 in both captures with int-bin 492a6177 | no change needed (the gap came from the d9c87f70 baseline bug) |
| Voice row avatars sharper | voice user avatars are loaded with `?size=32` for a 24 px avatar | `ui::avatar_cdn` + `media::with_cdn_size`, used by `voice_panel.rs` |
| GIF "Favoris" tile shows another favorite | `favoritesTile: src = _.sample(_.values(favorites)).src` (random favorite, picked when the GIF picker mounts) | `pickers/gif.rs`: random favorite per picker opening; `real_shots` can pin the client's pick with `PHX_GIF_FAV` (URL substring) |
| Threads search field 248 px | `container_VhFsFg` flex 1 1 auto (content basis 205.2) and `spacer_c0tu5x` flex 1 share the free space: 276.5 px at 1536 (popout 537.6), 247.7 px at 1280 (popout 480) | `inbox.rs`: `flex_grow` + `flex_basis(205.2)` next to the existing `flex_1` spacer |

Closed by wt/invites (2026-11-25): invite cards (profile card v2, invalid, legacy, group DM,
friend) in chat, pins, inbox and search; the top server emoji row with its trophy and POPULAIRES
badge; the frequent emoji and sticker order (client frecency, checked item for item on the live
proto). Details and what stays open (event, stream and voice v2 invite cards, the newly added
emoji block, reaction frecency) are in `message-content.md` (Invite cards) and `pickers.md`.

### wt/unreads (2026-11-25 07:15, int-bin 48c1fe8d)

Inbox "Messages non lus" at 1280×720: popout region 7.0 % mismatch / 2.8 % mean diff (after table above: 18.9 %; before: 29.2 %). The client was on `/channels/@me` during the pass, so the channel list and message regions of that run are not comparable. The 1536×840 client capture was discarded by the driver (view not in the expected state). Same 9 cards in the same order as the client; remaining differences: a user mention of a non-member in an embed shows "@Utilisateur inconnu" where the client shows the username it already knows (the client resolves it from its user cache), and the collapsed state is not persisted (`collapsedInInbox` in the settings proto).

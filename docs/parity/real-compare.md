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
- Home / DMs: the client was never on `/channels/@me`, and the sweep does not navigate. Compared offline instead (wt/mute): `real_shots PHX_DM=<the dumped DM>` 1280×700 at dpr 1.5 against `dumps/vanilla-dm.png` (2026-11-23): title bar 2.7 %, rail 10.6 %, channel list 19.9 %, header 3.3 %, messages 42.6 %, composer 1.0 %, overall 28.6 %. The rail, list and messages differ by content (one day apart: new messages, DM order, NOUVEAU pill); layout and header line up. The dump's DM profile panel shows the full-art (hovered / animated theme) state, which a still dump cannot score: ⚠ to measure live when the user opens a DM.
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
| Member list one name ellipsized early (combining marks / emoji in styled name) | member list zone | ❌ handed to wt/members2 |
| Search results panel: old layout (query field, "Plus récents / Plus anciens" tabs, plain author names, no highlight, no reply line, no embeds) vs the client's "N résultats" + Filtres + Trier header and full message cards | search results | ❌ handed to wt/search (rebuild in progress at 15:20) |
| Voice channel member rows: the client shows "Muet" + "En sourdine" for a self-muted, self-deafened member; one Phoenix render had the row without the icons, a later one had no voice rows at all | Three causes: `apply_ready` read the old voice users after `clear_session` had emptied `channels` (dead code, so the users from READY_SUPPLEMENTAL were lost when the ReadyData came after it); voice states from GUILD_CREATE or already cached at READY never sent their `VoiceFlags` (row without icons); my own row used the local mic buttons even when another client of mine held the call. Fixed, test `voice_users_survive_ready_after_supplemental`. Icons measured live (2026-11-24): `icon_YUoOxm` 16×16, gap 4 (`iconGroup_`), `--channels-default`, `iconServer_` `--icon-feedback-critical`, order Muet then En sourdine, no hover rule; glyphs = extracted MicrophoneSlashIcon / HeadphonesSlashIcon | ✅ wt/mute |
| Channel list "Nouveaux messages non lus" pill missing | compare artefact: with `NO_ACKS` the selected channel stays unread in `real_shots`, which hides the pill | — (not a product bug) |
| Message & reply usernames missing role color gradients (`twoColorGradient_` / `usernameGradient_`) | `chat.rs` used `emoji_text::label` for message authors and `fonts::name_text` for reply usernames instead of `role_gradient::for_author` + `fonts::name_text_with` (verified live via CDP, 2026-11-24) | ✅ wt/diff-messages (`957ff645`) |
| `article` link embeds rendered large preview image as 80×80 side thumbnail | `chat.rs` `embed()` only promoted `video` thumbnails to full-width media; Discord `sanitizeEmbed` also promotes `article` thumbnails (verified live on `immich.app` 1024×576 embed) | ✅ wt/diff-messages (`957ff645`) |
| Two-item media mosaic (`oneByTwoGrid_`) too short (~155px vs 273px), shifting all messages above by ~100px | `mosaic.rs` `Row::Two` used `row_aspect(its) * 2.` instead of 1:1 square cells (`ratio_box(2., 280.)`, measured live 550×273 / 273×273 per cell); `messages` mismatch dropped 31.1 % → 10.2 %, overall 17.1 % → 7.5 % | ✅ wt/diff-messages (`635719ae`) |
| Composer 5.4% → 2.3%, Header 3.5% → 2.4%, selected channel icons | Composer `channelTextArea_` border 0.8px vs GPUI 1px shifted the whole box 1px up (`mb` -0.8px, placeholder `top` -1.5px, buttons `mr` 13px); header search bar placeholder `pl` 8px / `lh` 18px and icon `--icon-muted`; selected channel action icons `--interactive-text-active`; thread row unread pill (`wt/diff-chrome-composer`, 957ff645) | ✅ wt/diff-chrome-composer |

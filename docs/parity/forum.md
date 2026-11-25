# Forum channels, forum posts and the thread sidebar

Reference: vanilla dumps `vanilla-forum` (forum list), `vanilla-thread-channel`
(forum post opened as a channel), ref-cache `create-thread-sidebar-*` (chat
sidebar frame). Real-data renders: `real_shots` with `PHX_GUILD`/`PHX_CHANNEL`
on a forum, `PHX_OPEN=post[:n]` for the post sidebar.

| element | CSS / dump source | property | value | our code | status |
|---|---|---|---|---|---|
| search/create bar | mainCard_BkGrJJ header_BkGrJJ | h / radius | 48 / 12 | ui/forum.rs render | ✅ (dump box 547x49) |
| "New Post" button | button_F7qvhY sm primary | h / radius / font | 32 / 8 / 14 500 | ui/forum.rs | ✅ |
| guide button | button_F7qvhY sm secondary | 32x32 / radius 8 | | ui/forum.rs | ✅ |
| sort dropdown | sortDropdown_BkGrJJ | h 32, pad 8 8 8 12, radius 24 | | ui/forum.rs | ✅ |
| tag pill | pill_snX-u4 | h 32, pad 0 12, radius 20, 14/600 | | ui/forum.rs tag_pill | ✅ |
| "All" tags button | tagsButton_BkGrJJ | h 32, radius 20, ml 8 | | ui/forum.rs | ✅ |
| post card | container_SOY5H6 mainCard_BkGrJJ | pad 12 10, radius 12, 1px border, gap 16 | | ui/forum.rs post_card | ✅ (101px card, 72px thumb at 1.25 zoom) |
| post title | postTitleText_SOY5H6 heading-lg/semibold | 20/24 600 | | ui/forum.rs | ✅ |
| title colour read/unread | `isNew || hasUnreads` (web bundle, module 767581) | archived → muted; never-opened post past forum ack → strong | | ui/forum.rs is_unread | ✅ (pinned "Médias" now strong like the client) |
| preview author | first message author | name | | ui/forum.rs post_card | ✅ |
| OP tag | botTagOP_feVUtO | --brand-260 bg, --brand-560 text, h 15, pad 0 4.4, radius 4, mt 2.5 | | ui/forum.rs op_tag, ui/chat.rs | ✅ (dump) / render pending |
| thread sidebar frame | chatLayerWrapper_ / container_FrbQJp | w 480, radius 12 0 0, --background-base-lower | | ui/chat/panels.rs thread_panel | ✅ (ref-cache) |
| thread sidebar header | container_ybUK7Z | h 49, pad 8 8 8 20, icon 32/20, body-md/medium --text-strong | | ui/chat/panels.rs | ✅ |
| thread sidebar composer | channelTextArea_ | margin 0 10 10, radius 8, --border-subtle, --chat-background-default | | ui/chat/panels.rs | ✅ |
| thread sidebar header toolbar, forum post | vanilla-forum-post-sidebar (2026-11-24) toolbar_ybUK7Z | no thread icon (title at the 20px padding, h2 body-md/medium), toolbar padding-left 10 gap 10: More + Close, 32px wrappers, 20px glyph --icon-muted | | ui/chat/panels.rs thread_panel | ✅ (dump). Regular thread sidebar (non-forum) not dumped: keeps the 32px thread icon of the create-thread sidebar |
| forum post action bar: copy link + jump to top | vanilla-thread-channel buttons_HeWz1C | sm secondary 32x32, radius 8, gap 4 | | ui/chat/panels.rs forum_post_bar | ✅ (dump) |
| forum post action bar: reactions + Following | vanilla-thread-channel reactButtons_HeWz1C (#1336–#1361) | starter message reaction pills (h28 r8) + add-reaction button (32x28 r8) + Follow/Following sm secondary button (`Abonné(e)` / `Suivre` from `is_thread_joined`) | | ui/chat/panels.rs forum_post_bar, backend.rs is_thread_joined | ✅ (dump) |
| opening a forum post by URL (not in active threads) | GET /channels/{id} (read-only) | | | ui/forum.rs open_by_id, ParityRequest::FetchThread | ✅ real_shots `PHX_CHANNEL=<post id>` opens the post with its forum breadcrumb (2026-11-24, 06081ab) |

## State coverage
- post card (live client 2026-11-24, `container_SOY5H6`, transitions 0.2s ease-out): rest --background-base-low + 1px --border-subtle, no shadow ✅; hover --background-surface-high, --border-strong, --shadow-low ✅; pressed (:active) --interactive-background-active, --border-strong, --shadow-border + --shadow-high ✅ (ui/forum.rs post_card).
- tag pill (`pill_snX-u4 clickable_`, transition background-color 0.2s): rest --background-mod-subtle, hover --background-mod-normal, pressed --background-mod-strong, text --control-secondary-text-default unchanged ✅ (ui/forum.rs pill_states). Selected pill hover not measured (no tag selected in the live forum).
- post card right-click (`thread-context`, dumps/vanilla-menu-thread, Manage Threads, pinned locked post): Mark As Read (disabled when read) | Add to Favorites > | Follow Post, Close Post, Unlock Post, Edit Post, Edit Tags >, Copy Link | Mute Post (disabled), Notification Settings (disabled) | Unpin Post, Delete Post | Copy Thread ID ✅ (ui/thread_menu.rs render_post; writes are stubs). Not dumped: the menu of an archived post, of a followed post (mute enabled) and without moderation rights.
| inline new post form | vanilla-forum-new-post container_owUmX8 | title 20/600 h32, body min 80, media tile 78 r12, tag row mt16 mb12 gap 8, small pills h24, divider, controls h40 mt12 | | ui/forum.rs compose_card | ✅ (dump) / render pending |
| announcement "Follow" button | vanilla-announcement followButton_z_gUBg | padding 4 8, mr 8; sm secondary h32 r8 14/500 | | ui/forum.rs follow_button, ui/chat.rs header | ✅ (dump); ✅ follow modal (`ui/modals/follow.rs`), from module 464035 (chunk 670774 `9827a3a225b5c7c1.js`) and its CSS `MXlLC1` (`64c9091e6d46ec50.css`), both fetched from the public asset CDN since they were not in the local cache: 440 small root; header `header_MXlLC1` on --background-base-lowest with the theme art (`ef95e909088f721c.svg` dark / `9b2c01218d4ab2ff.svg` light, cover), guild icon LARGE 50, channel pill (ml 16, max 320, --background-base-lower r4, inner --interactive-background-selected p8, AnnouncementsIcon 20 --text-muted margin-inline 2 8, body-md/medium ellipsis); content pt 32, centred heading-lg/semibold `mvPFbA`, body-sm --text-muted `kbpkxJ` mt 8 mb 24 (age-restricted: `DrNm/5` in --text-feedback-critical); stack gap 16: "Envoyer vers" select of the guilds that have GUILD_TEXT channels with MANAGE_WEBHOOKS (guild icon MINI 20 as leading), "Sélectionner le salon" select (hash icon), hint body-sm `Z0quyN` (or `6b6QoF` in critical and both selects disabled when no guild qualifies) mb 16; footer Cancel / Follow (disabled until both are picked). Picking a guild selects its channel named "general", else the first. Follow = `POST /channels/{id}/followers {webhook_channel_id}` (module 857182), coded and unit-tested, never executed; error 30007 → `1eZ4aB`, other errors → `LgwhuN`. Not built: the per-session "last destination" preselection and the success modal (module 505859, not fetched) |
| Events modal frame + header | vanilla-events-modal root__KoJTP / flex_hkRWAH | 602x400 r12, header pad 16 24 8, icon 24 mr 8, divider 1x16 mx 16, sm primary | | ui/events.rs | ✅ (dump) |
| Events empty state | container_NkbMg8 | heading-xl 24/32 mb 8, body-sm 14/18, roleTip mt 4 | | ui/events.rs | ✅ (dump) |
| Event card | tokens + `GuildEvent` fields (no live event on the measured guild) | 160px cover image (`guild-events/{id}/{hash}.png`), date/live + interested count, title, description, footer with voice/stage/calendar location + Interested button + copy link | | ui/events.rs card | ✅ (no live event dump) |
| Channels & Roles page (Customize tab) | vanilla-channels-roles tabBar_xTP3zy / prompt_pHGfKY / optionButton_87WSWY | tabs mt 32 mx 32 gap 24 pb 16; prompt pad 16 r8 surface-high; options 2 cols gap 8, r12, pad 12 16, emoji 22 mr 12 | | ui/community.rs | ✅ (dump) |
| Channels & Roles: Browse Channels tab | vanilla-browse-channels (2026-11-24) | tab badge (body-xs/semibold, --badge-secondary-*); search field h40 r8 --input-*; intro card pad 16 r8 mb 24 (heading-lg title, body-md text + semibold tip); category row pad 20 0 10, body-lg/medium + "Follow Category" switch 24px apart; channel rows pad 10 16, 1px --border-subtle sides/top (bottom on the last), r 8 on the ends, icon 20 gap 10, name body-md/medium (--text-muted while the category is followed), body-xs subtitle "Last activity …" · topic, 20px checkbox | | ui/community.rs browse_body | ✅ (dump). Switches and checkboxes come from READY `channel_overrides[].flags & 4096` and are read-only; the search filters channel names locally as you type (client behaviour checked live: a category name alone matches nothing, its header shows above matching channels; no result leaves only the intro card), with the 30px clear button; followed-category rows (`disabled_kU9idL`): name `--text-muted`, checkbox opacity .5 + not-allowed cursor, 18px checkmark; the intro illustration is not drawn (Discord artwork) |
| Channels & Roles: "Mon profil" column | vanilla-channels-roles #1166–#1198 | 288 wide beside the questions (gap 16), pb 32; heading-sm/semibold + body-xs `--text-muted` subtitle; card mt 16 pad 16 r8; avatar 80; name body-lg/medium 20/24 (margin 16 0 4); bio body-sm `--text-muted`; separator 1px `--border-subtle` margin 16 0; "Rôles" 14/500 `--text-muted` mb 8; role pills pad 4 8 gap 4 r16 `--background-mod-normal`, 8px dot (default grey #99aab5), body-xs/medium `--text-strong` | ui/community.rs profile_column | ✅ (dump). ⚠ card background: black on onyx, where base-low/lower/lowest are all black, so the token is a guess (`--background-base-lowest`); the column is not sticky while the questions scroll; the selected answers inside the questions are not shown |

## Server Guide (`/channels/{g}/@home`)

Data: the client loads the page with two read-only GETs. `GET /guilds/{g}/profile` was seen
in the live Network log when the page opened (2026-11-24, plus the usual `POST /science`
telemetry, no other write). `GET /guilds/{g}/new-member-welcome` is the client's
`GUILD_HOME_SETTINGS` route (local web bundle); it was cached from an earlier visit, so the
live log did not show it again. The header art is the READY guild property `home_header`
(`guilds/{g}/home-headers/{hash}.png`). The sidebar row shows only for guilds with
COMMUNITY + GUILD_SERVER_GUIDE + GUILD_ONBOARDING. Render: `PHX_OPEN=guide` in real_shots.

| element | dump source | values | our code | status |
|---|---|---|---|---|
| sidebar row "Server Guide" | vanilla-server-guide #643–#647 | SignPostIcon 20, selected row `--interactive-background-selected`, text and icon `--interactive-text-active` | ui/sidebar.rs guild_top | ✅ (dump) |
| Channels & Roles row selected state | same rule (`modeSelected_TNyK-N`) | | ui/sidebar.rs | ✅ (CSS) |
| page header bar | #1011–#1017 container_ybUK7Z | h 49, 32/20 icon `--icon-muted`, body-md/medium title | ui/guide.rs | ✅ |
| scroller | homeContainer_MG-lb3 | padding 32, gap 32 | ui/guide.rs | ✅ |
| header art | header_lqGeMc / headerArt_lqGeMc | margin-top -16, 16px wider each side, 25% of its width tall, radius 16, cover | ui/guide.rs header | ✅ |
| guild icon | headerIcon_lqGeMc iconSizeXLarge | 100 + 4px `--background-base-low` border, radius 30, margin -50 0 16 | ui/guide.rs | ✅ |
| name row | headerName_lqGeMc | heading-xxl/semibold 32/40, gap 8, discovery badge 24, md secondary "Invite" pushed right | ui/guide.rs | ✅ (Invite opens the invite modal) |
| Resources | container_Qo6EL- / row_Qo6EL- | mt 16 gap 8, heading-lg/semibold; row pad 16 r8 1px `--border-subtle`, `--background-surface-high` → hover `-higher`; title body-md/semibold mb 8; text body-sm `--text-default`, 3 lines max. The API description is often empty (it was on the measured guild): the client then shows the oldest message of the channel (content, else the first embed description), loaded with `GET /channels/{c}/messages?after={c}&limit=5`, and its first image as a 72px thumbnail on the right (custom icons win) | ui/guide.rs resource, load_preview | ✅ text (behaviour from the live client script); ⚠ thumbnail and custom icon URL not seen on a real guild; click opens the channel (the client opens it as a side panel) |
| sidebar card | container_Xweo-z / header_E7xgJz / body_Xweo-z | 300 wide, r16, `--background-surface-highest`, 120px discovery splash, 70px icon frame at -34, name heading-sm/semibold + badge 18 + tag chiplet, 8px online/offline dots + body-sm `--text-muted` counts (gap 0 8), "depuis nov. 2024", description, trait chips pad 4 8 r16 | ui/guide.rs card | ✅ |
| card icon mask | svg-mask-clan-icon | the client mask is `<rect rx=.3 ry=.3 width=1 height=1>` in objectBoundingBox units (mask defs in the web bundle), applied to the 70px frame and to the 64px guild icon inside it (profile card `f`: `mask:CLAN_ICON,width:70,height:70` around `iconSize:64`) | ui/guide.rs `card` | ✅ a plain rounded rect at 30 % of the side is that mask exactly: radius 21 on the frame, 19.2 on the icon |
| trait overflow "+ N en plus" | items_rw6y68 overflowMeasurement_ (#1097–#1116, 266px row, 85px overflow chip) | single-row fit with `+ N en plus` / `+ N more` collapse chip | ui/guide.rs fit_traits, card | ✅ (dump + unit test) |
| welcome message + new member actions ("Pour commencer") | `GET /guilds/{g}/new-member-welcome` | welcome card (authors + bio) and "Pour commencer" action rows (emoji, title, description, click opens channel) | commands/guide.rs, ui/guide.rs | ✅ (no live dump on measured guild) |

> ⚠ **Do not open Channels & Roles (`/channels/{g}/customize-community`) in the real client again.**
> On 2026-11-24 the client sent `PUT /guilds/{g}/onboarding-responses` by itself while the page
> was open (Customize → Browse tab, search typed, then left), seen in the CDP Network log. The
> earlier dumps of this page were probably taken the same way. Phoenix never sends it.


## Thread actions (wt/threads, 2026-11-24)

Sources: live `thread-context` menus in the sandbox channel `〢💬・phoenix` (CDP, 1280×800, zoom 125 %),
the requests the client sent there (Network log), and the thread actions module of the cached web bundle
(`web.b63d846b329d6470.js`, object with `archiveThread`, `lockThread`, `joinThread`, `addMember`,
`setNotificationSettings`…). Locations from `9b3cd218fe829ac6.js` (`Toolbar Overflow`, `Sidebar Overflow`)
and `d4030cd60f4ad310.js` (`Context Menu`).

| action | client request (captured or bundle) | Phoenix | status |
|---|---|---|---|
| close thread | `PATCH /channels/{t}` `{"archived":true}` (captured) | actions/thread.rs `archive_edit` | ✅ unit test + live API 200 |
| open (unarchive) | `{"archived":false}` (captured) | `unarchive_edit` | ✅ |
| lock / unlock | `{"locked":L,"archived":A}`, an archived thread is unarchived first (captured unlock `{"locked":false,"archived":false}`, bundle `lockThread`/`unlockThread`) | `lock_steps` | ✅ unit test + live API on an archived private thread |
| join | `POST /channels/{t}/thread-members/@me?location=…` (captured, `Toolbar Overflow`) | AdminCall::JoinThread | ✅ |
| leave | `DELETE …/thread-members/@me?location=…` (captured) | AdminCall::LeaveThread | ✅ live API 204 |
| add a member | `POST …/thread-members/{user}?location=…` (bundle `addMember`) | AdminCall::AddThreadMember, only offered for yourself | ✅ live API 204 (re-added myself to a private test thread) |
| notification level | `PATCH …/thread-members/@me/settings` `{"flags":2|4|8}`, joins first (bundle) | AdminCall::ThreadMemberSettings, radios read the member `flags` | ⚠ wired but never run live (read only by request); radio state ✅ (menu showed « Tous les messages » for flags 1) |
| unmute | same route `{"muted":false}` (bundle); row subtext « Muet jusqu'au {endTime} » (`j7h4AJ`, module 145408: `toLocaleString(locale,{month:'numeric',day:'numeric',hour:'numeric',minute:'2-digit'})` → fr `25/11 14:05`, en `11/25, 2:05 PM`, checked with CDP eval); labels « Rétablir les notifications du fil / du post » (`Cq/TzF`, `0JQfsP`), mute « Rendre le post muet » for posts (`nP+Ykd`) | thread_menu.rs `muted_until`; a muted member is unmuted locally once `end_time` passed (JoinedThreadsStore timer) | ✅ real_shots 43e7e9a0 (test thread muted 1 h by API, menu « Rétablir les notifications du fil / Muet jusqu'au 25/11 02:00 », then reset) |
| mute for a duration | module 812141 `p(t)` (chunk `27a3efd351a1f2d2.js`): `{muted:true, mute_config:{selected_time_window:t, end_time:t>0 ? now+t (toISOString) : null}}`, windows 900/3600/10800/28800/86400/-1 (`Xx` enum in the web bundle); same `setNotificationSettings` route, joins first | thread_menu.rs `mute_item` → `mute_thread_for`, actions/thread.rs `mute_patch` | ✅ unit test + live API 200 on the sandbox test thread (server answered `mute_config {end_time, selected_time_window:900}`), then reset to `muted:false, mute_config:null` |
| edit thread (rename, slowmode, Hide After Inactivity) | settings save sent the full channel object; Phoenix sends only changed fields `{name, rate_limit_per_user, auto_archive_duration}` | modals/channel_settings.rs thread page | ✅ live API 200 (restored the test thread name and 4320 with this body) |
| Hide After Inactivity options | live select: 1 heure, 24 heures, 3 jours, 1 semaine | AUTO_ARCHIVE | ✅ |
| spoiler channel toggle in thread settings | `IS_SPOILER_CHANNEL: 2097152` (1<<21, web bundle `lx`); `handleThreadSpoilerChange` → `flags ^ bit` (chunk `e751ad05577cfaf5.js`), label « Salon de spoiler » + `ddWXHa` description, placed after Hide After Inactivity | channel_settings.rs `SPOILER_CHANNEL`, `thread_edit` sends `{flags}` only when changed | ✅ unit test + live API 200 (flag set then restored to 0 on the test thread) |
| delete | `DELETE /channels/{t}`; modal « Supprimer le fil » / « Tu es sûr(e) de vouloir supprimer "{name}" ? Cette action est irréversible. » (live) | confirm_delete_thread | ✅ (not run: the test threads are kept) |
| private thread | `POST /channels/{c}/threads` type 12 | thread_create.rs | ✅ live API 201 |
| forum post: follow, close, lock, edit tags, pin, delete | same routes; pin = `flags | 2` (+ `archived:false` when archived), tags = `applied_tags` (bundle `pin`/`updateFlags`) | thread_menu.rs render_post | ✅ unit tests, never run (no test forum) |

### Menus (live dumps)
- Sidebar right click (`Context Menu`): Mark As Read | Favorites › | Close, Lock, Edit, Copy Link | Mute ›, Notification Settings › | Delete | Leave (Join when not joined, assumed) | Copy ID. ✅
- Full view `Plus` (`Toolbar Overflow`): [Join when not joined] | Jump To Top, Edit, Open in Split View | Favorites › | Close/Open, Lock/Unlock, Delete | Leave | Copy Link, Copy ID. ✅ (joined, left and archived variants dumped)
- Split view `Plus` (`Sidebar Overflow`): from the bundle: [Join] Open in Full View | … | Search ⧉, Pins ⧉ | … | Leave | Copy Link, Copy ID. ✅ built in the client order (9b3cd218fe829ac6.js `iL`): [Join] Open in Full View | Mute, Notifications | Edit | Search, Pins (WindowLaunchIcon, both open the thread in full view then dispatch FOCUS_SEARCH / TOGGLE_CHANNEL_PINS) | Close, Lock, Delete | Leave | Copy Link, Copy ID; no Jump To Top and no Favorites in this menu (those are Toolbar Overflow only); French label « Ouvrir en pleine fenêtre » (`IxVmZi`, used by the thread menus chunk `9b3cd218fe829ac6.js`) ✅, split view « Ouvrir en fenêtre fractionnée » (`bX7EaG`) ✅.
- Notification submenu values 2 / 4 / 8 (« Tous les messages », « @mentions seulement », « Aucun message »). Mute submenu 15 min … « Jusqu'à ce que je le change ». ✅

### Views
- Full view toolbar of a text channel thread: Threads, bell, pins, member list, More (live). The member list button is hidden on an archived thread (live). The bell is slashed when the thread is not joined (live after leaving), muted or not on all messages. ✅
- Threads browser rows: « Nom : message • Il y a 6 h » with a 16px avatar; archived rows « Commencé par … • Dernière activité : Il y a 6 h »; clicking opens the full view (live). ✅ real_shots 4a18475a (sandbox: 2 joined + archived private thread). When the gateway sent no `most_recent_messages`, the preview comes from `GET messages?limit=1` (read only). Author = server nick (`member_nick`, client `S.Ay(t).nick`, capture compare/sandbox/browser-discord.png « Yohan »). A rename system message (type 4) is previewed with its raw content, like the client: ThreadMessageStore only skips `THREAD_STARTER_MESSAGE`, and the row renders `p.o(message, content)` (module 448368) with no system-message branch. ✅ Facepile on every row, active and archived: `U` in chunk `9cf96fa6a4624cbd.js` = ThreadMembersStore `memberIdsPreview` + `memberCount`, max 50; Phoenix reads `member_ids_preview` from the cached channel (also refreshed by THREAD_MEMBERS_UPDATE) ✅ code.
- Threads browser search filters joined and archived threads by name (case insensitive). ✅
- Thread member list with nobody in it: « Il n'y a personne sur ce fil. » + « Tu peux inviter des personnes en utilisant @mention dans un message. » (live after leaving). ✅ code
- Full view `Plus` menu compared with real_shots c1ad21f9 on the sandbox thread: same items, order and separators as the live dump. ✅
- Sidebar thread rows take unread and mentions from the joined thread's read state. ✅ code.

## Forum settings (GET `/guilds/{g}/channels`, 2026-11-24)
Aperture Sciences forums: `bugs` and `aide-membres` default to the gallery (`default_forum_layout` 2), `to-do` and `bugs`
require a tag (`flags` 16), `absence` sorts by creation (`default_sort_order` 1) and matches all tags
(`default_tag_setting` `match_all`), several have a default reaction (✅, 👍, a custom emoji).

| element | source | Phoenix | status |
|---|---|---|---|
| default sort / layout / tag matching | channel fields above | ForumSettings + apply_defaults | ✅ unit test |
| Sort and View menu | bundle `sort-and-view` (chunk `10923e5c961551e9.js`): Sort by (`jOPmcI` « Récemment en ligne », `UIltXd` « Date du post ») / View as `mFMDSq` « Voir en tant que » (`NJFr+g` Liste, `wKeggb` Galerie, not for media) / Tag Matching `Paxaug` « Correspondance des tags » (`rQ0ctQ` « Faire correspondre certains », `FCXUu0` « Faire tout correspondre », only with tags) / `reset-all` `3b//lO` « Restaurer les paramètres par défaut » (default sort, layout and tag setting of the channel) | forum.rs sort_menu, `reset_forum_view` | ✅ (labels from the client fr/en string chunks `5ef1f6e6b679dd77.js` / `c30b23bab1166ada.js`) |
| tag filter request | `tag=a,b&tag_setting=match_some|match_all` (bundle `loadArchivedThreads`) | threads_search_path | ✅ unit test |
| gallery view | client grid cards | media tiles reused | ⚠ card layout not compared |
| required tag on new post | flag 16 | local check before POST | ✅ (never posted) |
| guidelines panel | `nX` in chunk `10923e5c961551e9.js` + CSS `container_hbPaoT` (29c2632a1792507e.css): between the header card and the tags row; mt 16, pad 12, 1px `--border-subtle`, `--radius-md`, `--background-base-low`; header heading-lg/semibold, BookCheckIcon 20, gap 8, PencilIcon xs (MANAGE_CHANNELS, opens the channel settings), XSmallIcon xs close (`--interactive-text-default/hover/active`); body `descriptionContainer` mt 12 pad 16 `--radius-sm` `--background-mod-subtle`, body-sm text-default, max-height 220; ≥ 220 tall → 28px gradient from `--background-base-lower` + « Afficher tout » (`Vu7odK`, body-sm/semibold text-brand, MaximizeIcon xs ml 8, pad 4 16 16 0). Open state `guidelinesOpen` (default false), toggled by the BookCheck secondary button (shown only with a topic, tooltip « Guide des posts » `4d4T4l`, in the search card and the compose card), opened when the composer opens the first time | forum.rs `guidelines`, `guide_button` | ✅ real_shots 8f945b38 (`PHX_OPEN=guidelines` on « suggestions »); ⚠ « Afficher tout » opens a modal (module 494664, not in the local cache): Phoenix expands the panel in place; the client's `hasSeen` store is per session here |
| default reaction button | card footer `eA` (chunk `6651e9581db5d17c.js`): when the first message has no reaction, `eN` = Reaction with the channel `defaultReactionEmoji`, `hideCount`, count 0; then the top reaction `eb`; class `updateReactionButton_SOY5H6` (margin-inline-end 4, inner padding 1px 6px) on `reaction_UNkajC` (r8, 1px border, emoji 20 + 2px margin, count 14 semibold `--text-subtle` ml 6); footer bullet `•` `--border-subtle` | forum.rs `card_reaction` (click toggles the reaction on the first message) | ✅ code (no live forum compare: opening a forum acks it) |
| live forum compare | opening a forum in the real client acks it: not done | | ⛔ |

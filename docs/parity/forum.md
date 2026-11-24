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
| forum post action bar: reactions + Following | vanilla-thread-channel reactButtons_HeWz1C | | | — | ❌ follow state not in model |
| opening a forum post by URL (not in active threads) | GET /channels/{id} (read-only) | | | ui/forum.rs open_by_id, ParityRequest::FetchThread | ✅ real_shots `PHX_CHANNEL=<post id>` opens the post with its forum breadcrumb (2026-11-24, 06081ab) |

## State coverage
- post card (live client 2026-11-24, `container_SOY5H6`, transitions 0.2s ease-out): rest --background-base-low + 1px --border-subtle, no shadow ✅; hover --background-surface-high, --border-strong, --shadow-low ✅; pressed (:active) --interactive-background-active, --border-strong, --shadow-border + --shadow-high ✅ (ui/forum.rs post_card).
- tag pill (`pill_snX-u4 clickable_`, transition background-color 0.2s): rest --background-mod-subtle, hover --background-mod-normal, pressed --background-mod-strong, text --control-secondary-text-default unchanged ✅ (ui/forum.rs pill_states). Selected pill hover not measured (no tag selected in the live forum).
- post card right-click (`thread-context`, dumps/vanilla-menu-thread, Manage Threads, pinned locked post): Mark As Read (disabled when read) | Add to Favorites > | Follow Post, Close Post, Unlock Post, Edit Post, Edit Tags >, Copy Link | Mute Post (disabled), Notification Settings (disabled) | Unpin Post, Delete Post | Copy Thread ID ✅ (ui/thread_menu.rs render_post; writes are stubs). Not dumped: the menu of an archived post, of a followed post (mute enabled) and without moderation rights.
| inline new post form | vanilla-forum-new-post container_owUmX8 | title 20/600 h32, body min 80, media tile 78 r12, tag row mt16 mb12 gap 8, small pills h24, divider, controls h40 mt12 | | ui/forum.rs compose_card | ✅ (dump) / render pending |
| announcement "Follow" button | vanilla-announcement followButton_z_gUBg | padding 4 8, mr 8; sm secondary h32 r8 14/500 | | ui/forum.rs follow_button, ui/chat.rs header | ✅ (dump); follow modal ❌ |
| Events modal frame + header | vanilla-events-modal root__KoJTP / flex_hkRWAH | 602x400 r12, header pad 16 24 8, icon 24 mr 8, divider 1x16 mx 16, sm primary | | ui/events.rs | ✅ (dump) |
| Events empty state | container_NkbMg8 | heading-xl 24/32 mb 8, body-sm 14/18, roleTip mt 4 | | ui/events.rs | ✅ (dump) |
| Event card | not dumped (no event on the measured guild) | | | ui/events.rs card | ❌ tokens only |
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
| card icon mask | svg-mask-clan-icon | squircle, measured ≈22 / 19px corners on the capture | ui/guide.rs | ⚠ rounded rect approximation |
| trait overflow "+ N en plus" | items_rw6y68 overflowMeasurement_ | the client wraps chips on one row and collapses the rest | ui/guide.rs (wraps all chips) | ❌ |
| welcome message + new member actions ("Pour commencer") | — | parsed from new-member-welcome, not shown on the measured guild (actions already done / none) | commands/guide.rs | ❌ no dump, not rendered |

> ⚠ **Do not open Channels & Roles (`/channels/{g}/customize-community`) in the real client again.**
> On 2026-11-24 the client sent `PUT /guilds/{g}/onboarding-responses` by itself while the page
> was open (Customize → Browse tab, search typed, then left), seen in the CDP Network log. The
> earlier dumps of this page were probably taken the same way. Phoenix never sends it.

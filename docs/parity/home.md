# Home area parity (DM list, Friends, Active Now, Message Requests, DM intro)

Reference: vanilla dumps from 2026-11-23 in /mnt/windata/project-phoenix/dumps (`vanilla-home-friends`, `vanilla-friends-online|all|pending|addfriend`, `vanilla-dm`, `vanilla-group-dm`, `vanilla-message-requests`, `vanilla-menu-dm`, `vanilla-menu-group-dm`). CSS is in ~/discord-phoenix-ref/current-client/css.
Status legend: ✅ means the values match the dump or CSS and the code cites them. ⚠ means it is implemented from CSS but not yet seen in a dump or a compare. ❌ means it is missing.
No real-data compare ran for this pass: the user's Discord was on a guild channel, so it was not navigated.

## DM list (sidebar)

| component | state | source | our code | status |
|---|---|---|---|---|
| Friends / Nitro / Shop / Quests rows | rest, hover, active, selected | home-friends #603–#633 (38px, radius 8, `--text-muted` → `--background-mod-strong`) | ui/sidebar.rs `dm_top` | ✅ |
| Message Requests row | shown only with pending requests, count badge | message-requests dump | ui/sidebar.rs `dm_top` | ✅ |
| Divider | rest | `sectionDivider_u88IQl` 1px, margin 12/8 | ui/sidebar.rs `dm_top` | ✅ |
| "Direct Messages" header | rest | #652 24px, padding 0 6 4 20, body-sm/medium `--channels-default` | ui/sidebar.rs `dm_top` | ✅ |
| "Direct Messages" header | hover: text `--interactive-text-hover` | `privateChannelsHeaderContainer_:hover` | ui/sidebar.rs `dm_top` | ✅ (this pass) |
| Header "More" button | hidden (opacity 0), shown on header hover | #655 `moreButton_Bce46W` | ui/sidebar.rs `dm_top` | ✅ (this pass, menu is a stub) |
| Header "Create DM" button | rest `--icon-muted`, hover `--icon-subtle`, tooltip | #657 `iconWrapper_ybUK7Z clickable_` | ui/sidebar.rs `dm_top` | ✅ (this pass) |
| DM row | rest, hover, active, selected (48px, link pl 10 / pr 0, avatar 32 + 12) | #659–#694 | ui/sidebar.rs channel row | ✅ |
| DM row nameplate | rest / hover / selected opacity; gradient under the image; mask start = `layout_TBNqol` width − 5 (module 351952, ref `ti`), measured with `nameplate::width_probe` | #661 `container_AFi54G` | ui/nameplate.rs, ui/sidebar.rs | ✅ |
| DM row server tag | chiplet, margin-left 4 | #675 | ui/sidebar.rs | ✅ |
| Group DM row | face pile 32px, "N members" | group-dm #800 | ui/sidebar.rs, ui/group_dm.rs | ✅ |
| Close button | display:none until the row is hovered; 24px, padding 3, margin-right 6, opacity .7 → 1 on hover | #680–#698 `closeButton_zln_tS` | ui/sidebar.rs `close` | ✅ (this pass; it used to take room with opacity 0) |
| Close button, plated | opacity 1, plate behind the glyph, glyph white .9 | `closeButtonPlated_`, `innerCloseButtonPlated_`, JS module 351952 `e3c95a79030889c3.js` (`--custom-nameplate-neutral`: dark `rgba(0,0,0,.22/.33)`, light `rgba(255,255,255,.22/.33)`) | ui/sidebar.rs `close` | ✅ |
| Close tooltip "Close DM" / "Leave Group" | hover | aria-label #681 / #806 | ui/sidebar.rs | ✅ |
| Unread pill (left 4×8) | unread, not selected | `unread_TNyK-N` | ui/sidebar.rs | ✅ |
| Mention badge | mentions > 0 | `numberBadge` | ui/sidebar.rs | ✅ |

## Friends page

| component | state | source | our code | status |
|---|---|---|---|---|
| Header bar (icon, title, dot) | rest | friends-online #1066 / #1074 | ui/home/friends.rs `header` | ✅ |
| Online / All / Pending tabs | idle `--text-subtle`, selected `--background-mod-strong` + `--text-strong` | #1125–#1127 `topPill_ .themed_` | ui/home/friends.rs `header` | ✅ (this pass, it used to use palette colours) |
| Tabs | hover `--background-mod-subtle`, active `--background-mod-strong` | `.topPill_6zZYhF .themed_.item_:hover/:active` | ui/home/friends.rs | ✅ (this pass) |
| Pending tab badge | count > 0 | dump | ui/home/friends.rs | ✅ |
| Add Friend tab | rest / hover / active / selected (`--opacity-blurple-16` + `--text-brand`) | #1128 `addFriend_zrUwGA` | ui/home/friends.rs | ✅ (this pass, hover and active added) |
| Blocked tab | the 2026 client removed the tab (Blocked lives in settings) | vanilla-friends-* (no tab) | not shown | ✅ |
| "New Group DM" toolbar button | rest, hover, tooltip | `inviteToolbar_zrUwGA` padding-inline-end 8, `iconWrapper_ybUK7Z clickable_` 32px, `icon_ybUK7Z` --icon-muted → hover --icon-subtle (vanilla-friends-online #1132–#1134) | ui/home/friends.rs `fr-new-group-dm` | ✅ |
| Search bar | rest, typing | `searchBar_wVFOQW`, #1141–#1143 | ui/home/friends.rs `list` | ✅ |
| Section title "Online — N" | rest | #1150 (em dash) | locales `friends.section` | ✅ (this pass, it used a hyphen) |
| Friend row | rest, hover (subtle bg, discriminator shown), 61px | `peopleListItem_y75PxA` | ui/home/friends.rs `friend_row` | ✅ |
| Row action buttons (Message, More) | rest, hover; accept / deny on pending | `actionButton_5GqXgh` | ui/home/friends.rs `round_button` | ✅ |
| Row action tooltips | hover | client strings Message / More / Accept / Ignore / Cancel / Unblock; glyph tinted on button hover (`actionButton_5GqXgh`, `actionAccept_`/`actionDeny_`) | ui/home/friends.rs `round_button` | ✅ |
| Pending rows (username subtext, Incoming / Outgoing) | rest | friends-pending | ui/home/friends.rs | ✅ |
| Add Friend form | rest, focus, success, error | friends-addfriend | ui/home/friends.rs `add_friend` | ✅ |
| Empty states (Online / All / Pending / Blocked) | text | client strings, Add Friend button on All, no-results note for searches | ui/home/friends.rs | ✅ text and buttons done. No illustration to extract: `FriendsEmptyState` (web bundle, module 396478) declares lightSrc/darkSrc per section but only renders `pp` (centred column) + `SG` (note); the image component `G8` is never used there |

## Active Now column

| component | state | source | our code | status |
|---|---|---|---|---|
| Column, 360px, heading-lg title | rest | friends-online #1635 (`--text-strong`) | ui/home/friends.rs `active_now` | ✅ |
| Voice card (face pile, "In a voice channel") | rest, hover | `itemCard_-7KzVR`, `headerFull_o3hXTo` | ui/home/friends.rs | ✅ |
| Game card (icon, elapsed time) | rest | `gameSection_o3hXTo` | ui/home/friends.rs | ✅ |
| Stream card with "LIVE" badge and preview | rest | friends-online #1673–#1686 (`applicationStreamingSection_o3hXTo`, `badge_4guu7-`, `applicationStreamingPreviewWrapper_o3hXTo` 294×165) | ui/home/friends.rs | ✅ |
| Empty "It's quiet for now…" | no activity | `emptyCard_-7KzVR` (`--background-base-low`, radius 8, padding 16), `emptyHeader_-7KzVR`, `emptyText_-7KzVR` (`--interactive-text-default`) | ui/home/friends.rs | ✅ |

## Message Requests, Nitro, Shop

| component | state | source | our code | status |
|---|---|---|---|---|
| Message Requests list | rest, hover, accept / ignore | vanilla-message-requests | ui/home/friends.rs `message_requests` | ✅ |
| Nitro home (`/store`) | rest, hover | `vanilla-nitro-home` (49px `navBarContent_jHfUxk`, hero gradient + "BONJOUR," + avatar/name, 330×380 `card_ualTdV` grid) | ui/home/nitro.rs | ✅ |
| Gift Nitro modal | open, plan select, no-purchase notice | `804439.f0ff1c535e7be47b.css` (`selectionBox_rvSze9`, `selectedPlan_rvSze9`, `planOptionDiscount_rvSze9`, `card_w20Das`, `stepBody_HYpJ9I`) | ui/modals/gift_nitro.rs, ui/chat/composer.rs, ui/home/nitro.rs | ✅ |
| Shop (`/shop`) | rest, hover | `vanilla-shop` / `vanilla-shop-hover` (`headerBar_ViLURN`, tabs, search, wishlist/orbs pills, 246px `productCardContainer_UGbKo5`, hover lift + buy/gift swap) | ui/shop_page.rs | ✅ |

## DM channel

| component | state | source | our code | status |
|---|---|---|---|---|
| DM header toolbar: voice call, video call, pins, add to DM, profile | rest, hover, tooltip | vanilla-dm #1101–#1109 | ui/chat.rs | ✅ |
| Group DM intro (80px icon, 32/40 bold `--text-strong` title, margin 8 0, description with bold name) | rest | group-dm #1103–#1112 `container_w6h4R2` (margin 20) | ui/chat.rs `intro` | ✅ (this pass) |
| Group DM intro buttons: Invite to Group DM (primary), Edit Group (secondary) | rest, hover, active | group-dm #1113–#1123 | ui/chat.rs `intro` | ✅ (this pass; Edit Group is a stub) |
| 1:1 DM intro (avatar, name, description) | rest | same container | ui/chat.rs `intro` | ✅ |
| 1:1 DM intro: username line, mutual servers row (`container_nM9YUb` gap 8, dot divider), Add/Remove Friend + Block buttons (`stack_TWWX0S` gap 8, sm secondary 32px radius 8) | rest, hover | vanilla-dm-start #1135–#1156: avatar 80, `heading-xxl/semibold` 32/40, `heading-xl/medium` 24/32, `body-sm/normal` 14/18 mutuals link | ui/chat.rs `dm_intro_actions` | ✅ values match the dump; no pixel compare yet (user awake, CDP offline) |

## Coverage

- 44 component × state rows: 44 ✅ (100 %).
- Shipped in this pass: plated close button (`--custom-nameplate-neutral` dark/light), Active Now empty card (`emptyCard_-7KzVR`) and Stream card (`LIVE` badge + 16:9 preview), Nitro hero gradient + card hover, Shop bundle title row + pill hover.
- Still to do:
  - a 1:1 DM start dump, then the mutual servers row and its buttons;
  - Nitro and Shop pages;
  - empty-state illustrations through runtime asset extraction;
  - a real-data compare when the user's Discord shows Home.

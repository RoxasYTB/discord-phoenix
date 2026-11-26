# Navigation parity audit

Primary source: the live-client DOM dumps (`/mnt/windata/project-phoenix/dumps/-channels-N-N+.json` = "N-N", `-channels-me+.json` = "me"; node `#i` = dump index, inspected with `~/discord-phoenix-ref/tree.py`). The user's client runs `.density-cozy` + `.enlarge-target-size` + `.theme-midnight` (Onyx), so the dump values below override the default-density CSS rows further down where they disagree.

## Vanilla client re-dump (2026-11-23, Vencord off)

Dumps: `vanilla-guild-text` (guild text channel, open folder in the rail), `vanilla-dm`, `vanilla-voice-sidebar`, `vanilla-forum`, `vanilla-guild-menu` in `/mnt/windata/project-phoenix/dumps` (private). Viewport 1280×700 CSS px, `density-cozy`, `theme-midnight`. These rows override the older ones above.

| element | dump node | property | dump value | our code | status |
|---|---|---|---|---|---|
| title bar | guild-text #25 `bar_3zSH5d` | height | 32, no fill | title_bar.rs `render` | ✅ |
| back / forward | #31..#35 | box | leading 80; buttons 26×24 at x 13/41, 18px glyph | title_bar.rs `nav_arrow` | ✅ |
| title (guild) | #27..#29 | icon / text | 16px guild icon + `body-sm/medium` guild name | title_bar.rs | ✅ |
| title (DM) | dm #27..#29 | text | always "Messages privés" (18px glyph), never the DM name | title_bar.rs `title` | ✅ (was the DM name) |
| trailing | #36..#45 | buttons | inbox, help: 24 r6, 18px glyph, gap 12 | title_bar.rs `bar_button` | ✅ |
| help glyph | #45 | svg | CircleQuestionIcon, backdrop circle `fill=transparent` | icons/extract.rs drops `secondaryColor` shapes | ✅ (rendered as a filled disc) |
| window buttons | #46..#52 `winButtons_3zSH5d winButtonsWithDivider` | layout | gap 4, divider 1px `--border-subtle` h 20 mx 4, 32×32 buttons, 12px glyph `--interactive-text-default`; close hover `--control-critical-primary-*` | title_bar.rs `win_buttons` (only with client-side decorations) | ✅ |
| Vencord-only | old #24..#38 | toolbox icon, macOS buttons | not in vanilla | — | removed / never ported |
| guild list | #55/#60 | width / gap / padding | 80 / 10 / 0 0 26 | server_bar.rs | ✅ |
| separator | #71 | size / color | 32×1 r1 `#97979f33` | server_bar.rs `separator` | ✅ |
| open folder | #74 `folderGroupBackground` | bg / border / radius | folder colour 15%, 1px `#97979f29`, 16 | server_bar.rs `folder` | ✅ |
| folder header | #78 | size | 48×48 | server_bar.rs `folder` | ✅ |
| Home button logo | live CDP 2026-11-24 `wrapper_JJ-hgR` > `childWrapper_JJ-hgR` | glyph size | 40×40 tile, 24×24 glyph (viewBox 24) at 8,8, no padding; was 30 | server_bar.rs `home` | ✅ user report "logo trop grand"; the title bar has no logo (back/forward buttons only) |
| "nouveau" bar | vanilla-guild-text #56/#57 top, #607/#608 bottom `unreadMentionsBar_5dqHTG unread_oJAkDB` | pill | padding 8 (bottom 8 8 26), h 24 r 12, --background-surface-highest, 1px --border-normal, shadow 0 2 6 --opacity-black-24, 12/16 600 uppercase --text-strong | server_bar.rs `unread_bar` (shown when an unread item is scrolled off; click scrolls to it) | ✅ (dump values; compare view had both pills hidden, rail 16.2→15.5 %; red mentions variant implemented from CSS `mention_oJAkDB` (--background-feedback-notification, 1px --border-subtle, --shadow-medium, --text-overlay-light) — ✅ vanilla-dm #590 `bar_oJAkDB unreadMentionsBar_5dqHTG mention_oJAkDB` 64×24 r12 rgb(210,45,57) = --background-feedback-notification #d22d39, 1px --border-subtle (onyx α.20), shadow 0 4 8 α.16 = --shadow-medium, #591 text "nouveau" 12/600 uppercase white: label is "nouveau" like the grey pill, not a mention count (fixed)) |
| channel list | #610 `sidebarList_pog62O` | border / radius | 1px left `#97979f33`, 12 top-left (width 336 = user resize) | sidebar.rs `render`, sidebar_resize.rs | ✅ drag handle `sidebarResizeHandle_pog62O` 4px, mt 12, ew-resize, hover --background-mod-strong; range aria 264–432 (incl. 80 rail) → 184–352; persisted in ~/.config/discord-phoenix/sidebar_width; real_shots reads the live `sidebarList_` width (compare 20261123-145943: both lists 336) |
| guild header | #613..#635 | height / padding / dropdown / invite | 49 / 10 / 32 r8 gap 2 / 32 r8 20px glyph; Discovery badge 18px before the name | sidebar.rs `header` | ✅ (vanilla-guild-text #616 gap 10, #617 20px box / #619 18px badge; name nowrap; Discovery badge = client `GuildBadge` DISCOVERABLE → `ExperimentalWebBadgeIcon`, two tones --badge-inverted-background / --badge-inverted-text, extracted at runtime (`discovery-badge`, `discovery-badge-fg` via `#secondary`) ✅ compare 20261123-152127 header 8.1 %) |
| boost goal | #653..#662 `container_BFMMcK` | bar | 311×26 pill, margin 0 0 12 8, padding 4 4 16, border-bottom `#97979f33`, 12px semibold text | sidebar.rs `boost_goal` | ✅ shown when `premium_progress_bar_enabled` && boosts > 0 (client `GUILD_PREMIUM_PROGRESS_BAR` rule); progress `calc(n/max% - 4px)`, pink-refresh gradient 40→10 %, count text opacity .7; max computed like client `tg` (module 616740): 14 (`M2T[TIER_3]`, 0 with PREMIUM_TIER_3_OVERRIDE) + standalone power-up prices 22 (module 568065, static) = 36, matches live "18/36". real_shots side by side on 1369382919948210248: same text, pill and layout (compare 20261123-143926, channel list 33.1 % before the width fix) |
| top rows | #663..#678 | rows | Events / Members / Server Boosts, same `wrapper_TNyK-N` rows as channels | sidebar.rs `guild_top` | ✅ (new) |
| top divider | #679 `sectionDivider_ysO7Gy` | line | 296×1 at x+20, then 12px gap | sidebar.rs `guild_top` | ✅ (new) |
| channel row | #664/#665 | wrapper / link | margin-left 10, padding 1 0 / 36, padding 4 10, r 8, gap 10 | sidebar.rs `channel_row` | ✅ |
| channel name | #667 | font | 16/28 500 `#7a7b83` (`--channels-default`) | sidebar.rs | ✅ |
| row actions | #691..#696 `children_TNyK-N` | layout | margin -4 -4 -4 0; 24px items (padding 4, 16px): Invite, Edit Channel (manage rights) | sidebar.rs `invite_hover` | ✅ (gear was missing) |
| category | #716 | row | 24, padding 0 6 0 20 | sidebar.rs `row` | ✅ |
| category add | #722 | position | 24px, right edge of the row (x 377 of 407) | sidebar.rs `row` (`.w_full()`) | ✅ (was next to the chevron) |
| voice channel | voice-sidebar | row / users / timer | same 36px row; connected users 24px avatar rows; elapsed timer `47:50:07` monospace on the right | voice_panel.rs `voice_user_row`, sidebar.rs `channel_row` timer | ✅ rows; ✅ timer: `channelInfo_Wx6SnQ` ml 12 pr 4, 12/16 mono `--text-feedback-positive`, `H:MM:SS` from `voice_start_time` (gateway op 43 REQUEST_CHANNEL_INFO → CHANNEL_INFO / VOICE_CHANNEL_START_TIME_UPDATE, voice/start_time.rs). Evidence: guild 1366311080011501578 "Général" occupied — Discord 22:38 (compare 20261123-145943) vs real_shots 31:35 nine minutes later, same green mono right-aligned. User-limit pill `wrapper_QQRrAh` (h 20, 1px --border-normal, --radius-md, users_ 28px --background-base-low, total_ 20px --background-mod-strong + 6px slant, 12/16 medium --interactive-text-default) ✅ real_shots render matches Travaux Groupe #1–#6 "00 │ 06/07". Connected channel icon --text-feedback-positive (`iconLive_Wx6SnQ`) implemented. Locked icons ✅: client rule (icon picker `gU` → helper module 148719) = guild channel where @everyone (role perms − overwrite.deny + allow) lacks CONNECT for voice/stage, or the @everyone overwrite denies VIEW_CHANNEL; NSFW wins over lock. Implemented in discord_backend/nav.rs `extra`, glyphs text-lock/voice-lock/announcement-lock/forum-lock/stage-lock via icons/names.rs, sidebar.rs `decorated_icon`. Evidence: compare 20261123-152813 (channel list 14.7 % mismatch) — Phoenix shows TextLockIcon on "🔑┆moderation", live aria "moderation (salon textuel), Salon privé (verrouillé)"; Général / Travaux Groupe #1–#3 flagged locked live, now matched by the CONNECT rule |
| guild menu | guild-menu | items | Boost, Server Tag (chip), Invite, Settings, Insights, Create Channel / Category / Event, Active Threads, App Directory, Show All Channels (checkbox), Notification Settings, Privacy Settings, Edit Per-server Profile, Hide Muted Channels (checkbox) | guild_menu.rs (other owner) | see popouts.md |
| panels card | #987 | box | x 10, 396 wide (spans under the guild rail), r 8, 1px `#97979f29` | sidebar.rs `panels` | ✅ |
| user panel | #990/#991 | container / account | 56, padding 14, gap 10 / 36, padding 5 6, margin-left -6, r 4 | sidebar.rs `user_panel` | ✅ |
| user subtext | panel | text | custom status text instead of "Online" | sidebar.rs `user_panel` + convert.rs `self_activities` (settings proto / READY sessions) | ✅ |
| self mute / deaf | panel | state | red slashed mic + headset | voice_panel.rs `audio_button` | ✅ state persisted in voice.json; real_shots reads the live panel switches (`audioButtonWithMenu_` aria-checked, or `PHX_AUDIO`): red slashed mic + headset on both sides (compare 20261123-143403) |
| DM nav | dm #604..#644 | rows | Amis / Ton Nitro / Boutique / Quêtes, 40px rows; promo card under Quêtes | sidebar.rs `dm_top` | ✅ the "promo" is `questHomeHeroBackgroundImage_hjotnx` at opacity 0 at rest (#657), shown on hover only; hover text `--text-strong` done. Hero image itself needs quest data (not wired) |
| DM rows | dm #661.. | li | 50 tall | sidebar.rs `channel_row` | ✅ |

## Dump-verified (density-cozy, with Vencord — superseded)

| element | dump node | property | dump value | our code | status |
|---|---|---|---|---|---|
| title bar | N-N #13 `bar_3zSH5d` | height / background | 32 / none (app bg) | title_bar.rs `render` | ✅ |
| back / forward | N-N #19..#23 `backForwardButtons_nS9Yqn` | box / padding / radius / glyph | 80px leading, 26×24, `0 2 0 6`/`0 6 0 2`, 6px outer, 18px `--interactive-icon-default` | title_bar.rs `nav_arrow` | ✅ (history not wired) |
| title | N-N #15..#17 `title_LnjVAX` | gap / icon / text | 8 / 16px guild icon r4 / 14/500 `--text-strong` | title_bar.rs | ✅ |
| trailing | N-N #24..#38 | gap / button / glyph | 12 / 24 r6 / 18 `--icon-muted` | title_bar.rs `bar_button`, `win_buttons` | ✅ vanilla has no Quests button (Inbox, Help, then `winButtonsWithDivider_` 3×32px, 12px glyph, flush right); trailing pr 12 only without client decorations |
| guild list | N-N #50 `nav.guilds_` | width | 80 (40 + 2×20, `--space-md` cozy) | `m::SERVER_BAR_W` | ✅ |
| guild list | N-N #55 `scroller_5dqHTG` | gap / padding | 10 / `0 0 26` (linux) | server_bar.rs `GAP`, `PAD_*` | ✅ |
| guild icon | N-N #160..#163 | size / mask | 40 / squircle | server_bar.rs `ICON` | ✅ |
| separator | N-N #66 `guildSeparator_3WHVfy` | size / color | 32×1 / `#97979f33` | server_bar.rs `separator` | ✅ |
| folder | N-N #68..#81 | header / bg / radius / list | 48 / folder colour 15% / 16 / first item margin-top 6, gap 10 | server_bar.rs `folder` | ✅ |
| channel list | N-N #585 `sidebarList_pog62O` | border / radius | 1px top+left `--border-subtle`, 12px top-left | sidebar.rs `render` | ✅ (width is user-resized 336 in the dump; default 268, now resizable) |
| guild header | N-N #588 `header_XH4OpX` | height / padding / border | 49 / 10 / 1px `--border-subtle` | sidebar.rs `header` | ✅ |
| guild dropdown | N-N #590 `guildDropdown_XH4OpX` | height / padding / radius / gap | 32 / 0 10 / 8 / 2 | sidebar.rs `header` | ✅ |
| guild name | N-N #605 | font | 16/20 600 `--text-strong` + 16px chevron | sidebar.rs `header` | ✅ |
| invite button | N-N #609 `inviteButton_XH4OpX` | size / radius / glyph | 32 / 8 / 20 | sidebar.rs `header` (icon `invite`) | ✅ |
| channel row | N-N #628 `wrapper_TNyK-N` | margin-left / padding | 10 / 1 0 | sidebar.rs `channel_row` | ✅ |
| channel link | N-N #629/#638 `link_TNyK-N` | height / padding / radius / gap | 36 / 4 10 / 8 / 10 | sidebar.rs `channel_row` | ✅ |
| channel name | N-N #643 | font | 16 / 500 / line 28, `--channels-default` | sidebar.rs `channel_row` | ✅ |
| channel selected | N-N #858 | bg / color | `#97979f3d` / `--text-strong` | `INTERACTIVE_BACKGROUND_SELECTED` | ✅ |
| channel hover actions | N-N #865 `children_TNyK-N` | size | 2×24 at margin -4 | sidebar.rs `invite_hover` | ✅ |
| category | N-N #655..#663 | padding-top / height / padding / font | 16 / 24 / 0 6 0 20 / 14/18 500 | sidebar.rs `row` | ✅ |
| category add | N-N #662 `addButton forceVisible` | size / visibility | 24 (padding 4, 16 glyph) / always | sidebar.rs `row` | ✅ |
| panels card | N-N #976 `panels_pog62O` | inset / radius / border | 10 / 8 / 1px `--border-muted` | sidebar.rs `panels_card` | ✅ |
| user panel | N-N #979 `container_g6VwXp` | height / padding / gap | 56 / 14 / 10 | sidebar.rs `user_panel` | ✅ |
| account button | N-N #980 | margin / padding / gap / radius | -6 / 5 6 / 8 / 4 | sidebar.rs `user_panel` | ✅ |
| name / subtext | N-N #985 / #989 | font | 16/22 500 `--text-strong` / 12/13 500 `--interactive-text-default` | sidebar.rs `user_panel` | ✅ |
| mic / deafen | N-N #999..#1016 | button / chevron | 32 (r `8 0 0 8`) + 1 + 16 (r `0 8 8 0`), red 12% when muted | voice_panel.rs `audio_button` | ✅ |
| settings | N-N #1017 | size / radius | 32 / 8 | voice_panel.rs `panel_button` | ✅ |
| DM search | me #587/#588 `searchBar_u88IQl` | header / button | 49, padding 0 10 / 31px r8 `--control-secondary-background-default`, 1px `--border-muted`, 14/18 500 centred | sidebar.rs `header` | ✅ |
| DM nav rows | me #597..#625 `channel_zln_tS` | li / inner / link | 40 (margin-left 8) / 38 r8 / padding 8 16 8 10, 20px glyph + 12, 16/20 500 | sidebar.rs `dm_top` | ✅ |
| DM nav selected | me #598 | bg / color | `--background-mod-strong` / `--text-default` | sidebar.rs `dm_top` | ✅ |
| DM divider | me #648 `sectionDivider_u88IQl` | margin / color | 12 8 / `--border-subtle` | sidebar.rs `dm_top` | ✅ |
| DM header | me #649 `privateChannelsHeaderContainer_Bce46W` | height / padding / font | 24 / 0 6 4 20 / 14/18 500 `--channels-default` | sidebar.rs `dm_top` | ✅ |
| DM row | me #656..#665 `dm_zln_tS` | li / link / avatar | 50 (margin-left 8) / 48, pl 10 / 32 + margin 12 | sidebar.rs `channel_row` | ✅ |
| voice users, threads, voice panel | — | — | not present in the dumps yet (needs a dump of a guild with people in voice and a connected call) | voice_panel.rs | ❓ |

## CSS reference (default density)

Source: `~/discord-phoenix-ref/current-client/css/`, using the default density (`.density-default`: `--space-xs` 8, `--space-sm` 12, `--space-md` 16) and without the `refresh-fast-follow-avatars` experiment (`--guildbar-avatar-size` 40, `--guildbar-folder-size` 48). Colours are generated tokens (`theme::generated::idx`). Paths are relative to `crates/phoenix-app/src/ui/`.

| element | CSS source (file + selector prefix) | property | CSS value | our code | status |
|---|---|---|---|---|---|
| title bar | 419070 `:root` | height | `--custom-app-top-bar-height` 32px | title_bar.rs `m::TITLE_BAR_H` | ✅ |
| title bar | 300559 `bar_3zSH5d` | padding-inline-end / gap | `--space-12` / `--space-8` | title_bar.rs `.pr(12)` | ✅ |
| title bar title | 300559 `title_3zSH5d` | layout | absolute, centred | title_bar.rs `.absolute().inset_0()` | ✅ |
| title bar trailing | 300559 `trailing_3zSH5d` | gap | `--space-12` | title_bar.rs `.gap(12)` | ✅ |
| title bar button | `clickable_fErCLM` | size / radius | `--space-24` / `--custom-app-top-bar-item-radius` 6px | title_bar.rs `bar_button` | ✅ |
| title bar button | `clickable_fErCLM` | color / hover / selected | `--icon-muted` / bg `--background-mod-normal` / `--interactive-icon-active` | title_bar.rs `bar_button` | ✅ |
| guild list | 419070 `:root` | width | `--guildbar-avatar-size + 2*--custom-guild-list-padding` = 72 | `m::SERVER_BAR_W` | ✅ |
| guild list | 703298 `wrapper_5dqHTG` | background | `--background-base-lowest` | server_bar.rs `BG_TERTIARY` | ✅ |
| guild list | 703298 `scroller_5dqHTG` | padding-top / bottom | 4px (win/osx) / `--space-xs + 16px` = 24 | server_bar.rs `PAD_TOP`, `PAD_BOTTOM` | ✅ |
| guild list | 703298 `scroller_5dqHTG>[hidden]` | item spacing | `--space-xs` 8 | server_bar.rs `GAP` | ✅ |
| guild item | 607986 `listItem_rwEpIL` | width / justify | list width, centred | server_bar.rs `row` | ✅ |
| guild icon | 343178 `wrapper_JJ-hgR`, `icon_JJ-hgR` | size | `--guildbar-avatar-size` 40 | server_bar.rs `ICON` | ✅ |
| guild icon | `--radius-md` (squircle mask) | radius | 12px | server_bar.rs `ICON_RADIUS` | ✅ (the client mask is `svg-mask-squircle`, path `P` in the web bundle's mask defs; measured against it, a rounded rectangle of radius 0.3 × size stays within 0.0041 × size of the outline, 0.17 px at 40, so the rail keeps the radius; the exact shape is available as `ui/squircle.rs` and used where Phoenix had radii further off) |
| guild icon (acronym) | 343178 `childWrapper_JJ-hgR` | bg / color | `--background-mod-subtle` / `--text-default` | server_bar.rs `guild` | ✅ |
| guild icon hover/selected | 343178 `wrapper:hover .childWrapper` | bg / color | `--background-brand` / `--white` | server_bar.rs `guild` | ✅ |
| acronym | 343178 `acronym_JJ-hgR` | font-weight | `--font-weight-medium` | server_bar.rs `guild_inner` | ✅ |
| pill | 441674 `wrapper__o8_Xq` / `item__o8_Xq` | width / visible part | 8px, margin-inline-start -4 → 4px | server_bar.rs `pill` | ✅ |
| pill | 441674 `item__o8_Xq` | radius / color | `0 4px 4px 0` / `--text-strong` | server_bar.rs `pill` | ✅ |
| pill | 441674 `.visible/.hovered/.selected` | height | 8 / 20 / 40 | server_bar.rs `pill` | ✅ |
| pill | 441674 `.full-motion .item` | transition | height .2s ease-out | server_bar.rs `PILL_MS` 200 | ✅ |
| separator | 878504 `guildSeparator_3WHVfy` | size / radius / color | 32x1 / 1px / `--app-frame-border` | server_bar.rs `separator` | ✅ |
| mention badge | 239502 `lowerBadge_hqZPsU` | position | bottom 0, right 0 (cut-out) | server_bar.rs `mention_badge` | ✅ |
| badge | 5257 `base_lMvXrQ` / `baseShapeRound` | height / min-width / radius | 16 / 16 / `--radius-round` | mod.rs `badge` | ✅ |
| add / discover | 607986 `circleIconButton_g8YUxD` | size / bg / color | 40 / `--background-mod-subtle` / `--text-default` | server_bar.rs `action` | ✅ |
| add / discover hover | 607986 `circleIconButton:hover` | bg | `--background-brand` | server_bar.rs `action` | ✅ |
| add / discover icon | 607986 `circleIcon_g8YUxD` | size | 20px | server_bar.rs `action` | ✅ |
| folder header | 878504 `folderHeader_RTMSuZ`, `folderButton` | size | `--guildbar-folder-size` 48 | server_bar.rs `folder` `.h(FOLDER)` | ✅ |
| closed folder | 878504 `folderPreviewWrapper_RTMSuZ` | size / radius / padding / bg | 48 / `--radius-lg` / 4px / folder colour, 60% transparent | server_bar.rs `folder` | ✅ |
| folder preview | 878504 `folderPreview`, `folderPreviewGuildIcon` | gap / cell / radii | 2px / 19px / 13px outer, 4px inner | server_bar.rs `folder` | ✅ |
| open folder icon | 878504 `folderIconWrapper`, `folderIcon` | padding / size / radius / hover | 4 / 40 / `--radius-md` / `--background-mod-subtle` | server_bar.rs `folder` | ✅ |
| open folder bg | 878504 `folderGroupBackground_RTMSuZ` | width / radius / border / bg | 48 / `--radius-lg` / 1px `--border-muted` / colour, 85% transparent | server_bar.rs `folder` | ✅ |
| folder list | 878504 `folderGuildsList>:first-child` | margin-top | `--space-xs - --custom-folder-padding` = 4 | server_bar.rs `folder` | ✅ |
| guild tooltip | `tooltip_3_v8qp`, `guildTooltipWrapper__-eZlF` | radius / border / padding / max-width | 8 / 1px `--border-subtle` / 8 12 / 196 | server_bar.rs `right_tooltip` | ✅ |
| channel list | 795186 `sidebarList_pog62O` | width | `--custom-guild-sidebar-width` 268 (incl. 1px border) | `m::SIDEBAR_W` | ✅ |
| channel list | 795186 `sidebarList_pog62O` | border / radius | 1px `--app-frame-border` start+top / `--radius-md` | sidebar.rs `render` | ✅ |
| channel list | 795186 `sidebar_pog62O` | background | `--background-base-lower` | sidebar.rs `BG_SECONDARY` | ✅ |
| guild header | 65a `header__xoAVS` | height / border | `--custom-channel-header-height` 49 / 1px `--border-subtle` | sidebar.rs `header` | ✅ |
| guild header | 65a `header` + `title` padding | name inset | `--space-xs` + `--space-xs` = 16 | sidebar.rs `header .px(16)` | ✅ |
| category | 65a `containerDefault_yOdhm_` | padding-top | `--custom-category-channel-space-before-category` 16 | sidebar.rs `Category .pt(16)` | ✅ |
| category | 65a `wrapperCommon_yOdhm_` | height / padding-inline | 24 / `--space-md` `--space-md - --space-xs` (16/8) | sidebar.rs `Category` | ✅ |
| category | 65a `wrapperCommon` | color / hover | `--channels-default` / `--interactive-text-hover` | sidebar.rs `Category` | ✅ |
| category | 65a `mainContent` / `icon_yOdhm_` | gap / chevron | 4px / 12px after the name, -90deg collapsed | sidebar.rs `Category` | ✅ |
| category add | 65a `addButton_yOdhm_` | size / visibility | 16px / hover only | sidebar.rs `Category` | ✅ |
| channel row | f4b2 `wrapper_TNyK-N` | margin-start / padding | `--space-xs` / 1px 0 | sidebar.rs `channel_row` | ✅ |
| channel row | f4b2 `link_TNyK-N` | radius / padding | `--radius-sm` / `--space-xxs` `--space-xs` | sidebar.rs `ROW_RADIUS`, `.px(8)` | ✅ |
| channel row | f4b2 `basicChannelRowLink` | min-height / gap | 24 (+8 → 32) / `--space-xs` | sidebar.rs `.h(32)`, `.gap(8)` | ✅ |
| channel name | f4b2 `name_TNyK-N` | line-height / color | `--channels-name-line-height` 24 / `--channels-default` | sidebar.rs `channel_row` | ✅ |
| channel icon | f4b2 `icon_TNyK-N` | size / color | 20 / `--channel-icon` | sidebar.rs `decorated_icon` | ✅ |
| hover | f4b2 `wrapper:hover .link` / `.name` | bg / color | `--interactive-background-hover` / `--interactive-text-hover` | sidebar.rs `channel_row` | ✅ |
| selected | f4b2 `modeSelected .link` / `.name` | bg / color / weight | `--interactive-background-selected` / `--interactive-text-active` / 500 | sidebar.rs `channel_row` | ✅ |
| unread | f4b2 `modeUnreadImportant` | name / icon | `--interactive-text-active` 500 / `--icon-subtle` | sidebar.rs `channel_row` | ✅ |
| muted | f4b2 `modeMuted` | name / icon | `--text-muted` / `--icon-muted`, opacity .5 | sidebar.rs `channel_row` | ✅ |
| unread pill | f4b2 `unread_TNyK-N` / `unreadImportant` | size / inset / radius / color | 4x8 / `-(--space-xs)+1px` / `0 --radius-xs --radius-xs 0` / `--interactive-text-active` | sidebar.rs `channel_row` | ✅ |
| hover action icon | f4b2 `iconItem_Wx6SnQ`, `actionIcon` | margin / size / color | 4px / 16 / `--interactive-text-default` | sidebar.rs `invite_hover` | ✅ |
| thread row | f4b2 `typeThread.wrapper` | margin-start | `28px + --space-xs` = 36 | sidebar.rs `thread_row` | ✅ |
| thread row | f4b2 `typeThread .link` | padding | 4px `--space-xs` | sidebar.rs `thread_row` | ✅ |
| thread spine | 3cf8 `spine_reDhww` | offset / color | `--channels-spine-offset-left` 24 / `--spine-default` | sidebar.rs `thread_row` | ✅ |
| voice user | 880186 `content_YUoOxm` | radius / padding / gap | `--radius-sm` / 4px `--space-xs` / 8 | voice_panel.rs `voice_user_row` | ✅ |
| voice user | 880186 `userSmall` / `avatarSmall` | height / avatar | 32 / 24 | voice_panel.rs `voice_user_row` | ✅ |
| voice user | 880186 `username` / `usernameSpeaking` | color | `--channels-default` / `--interactive-text-active` | voice_panel.rs `voice_user_row` | ✅ |
| voice user hover | 880186 `.clickable:hover .content` | bg | `--interactive-background-hover` | voice_panel.rs `voice_user_row` | ✅ |
| voice icons | 880186 `icons`, `icon_YUoOxm` | gap / size / color | 4 / 16 / `--channels-default` | voice_panel.rs `voice_user_row` | ✅ |
| server mute | 880186 `iconServer_YUoOxm` | color | `--icon-feedback-critical` | voice_panel.rs `voice_user_row` | ✅ |
| LIVE | 880186 `liveIcon_YUoOxm` | bg / color | `--background-feedback-notification` / white | voice_panel.rs `voice_user_row` | ✅ |
| server tag | 234017 `chipletContainerInner_k8jyN7` | bg / radius / padding / line-height | `--background-mod-strong` / 4 / 0 4 / 16 | voice_panel.rs `server_tag` | ✅ |
| server tag | `chipletParent_kQORUk` | gap to name | padding-start 4 | voice_panel.rs `server_tag` | ✅ |
| panels card | 795186 `panels_pog62O` | inset / radius / border / bg | `--space-xs` / `--radius-sm` / 1px `--border-muted` / `--background-base-low` | sidebar.rs `panels_card` | ✅ |
| user panel | 718878 `container_g6VwXp` | height / padding / gap | 56 / 12 (`--custom-guild-list-padding - --custom-panels-spacing + 4`) / `--space-xs` | sidebar.rs `user_panel` | ✅ |
| account button | 718878 `accountPopoutButtonWrapper` | margin / padding / radius / gap / min-width | -4 / 5 4 / `--radius-xs` / 8 / 112 | sidebar.rs `user_panel` | ✅ |
| account button | 718878 `accountPopoutButtonWrapper:hover` | bg | `--background-mod-normal` | sidebar.rs `user_panel` | ✅ |
| name | 718878 `panelTitleContainer` | margin | -1 top, 2 bottom | sidebar.rs `user_panel` | ✅ |
| buttons | 718878 `buttons_g6VwXp` | gap | `--space-8` | sidebar.rs `user_panel` | ✅ |
| panel button | 718878 `button_Y-8oZW` | size / radius / color / hover | 32 / `--radius-sm` / `--icon-subtle` / `--control-secondary-background-hover` | voice_panel.rs `panel_button` | ✅ |
| muted / deafened | 718878 `redGlow_Y-8oZW`, `redIcon__WEKcz` | bg / hover / icon | `--opacity-red-12` / `-24` / `--icon-feedback-critical` | voice_panel.rs `audio_button` | ✅ |
| mic / deafen chevron | 718878 `buttonChevron_gKpIZP` | size / gap / radii / icon | 16x32 / 1px / square inner corners / 16px | voice_panel.rs `audio_button` | ✅ |
| chevron hover | 718878 `audioButtonParent:hover` | bg | `--background-mod-muted` for both, hovered part `--control-secondary-background-hover` | voice_panel.rs `audio_button` | ✅ |
| voice panel | 593586 `container_9U6LUN` | padding / gap / border | 12 / 12 / 1px `--border-muted` bottom | voice_panel.rs `voice_panel` | ✅ |
| status line | 593586 `connection_9U6LUN`, 587669 `rtcConnectionStatus` | height | 32 | voice_panel.rs `voice_panel` | ✅ |
| ping glyph | 587669 `ping.largePing` | size / padding / radius / margin / bg | 18 + 7 = 32 / 8 / 8 / `--background-feedback-mod-*` | voice_panel.rs `large_ping` | ✅ |
| status label | 587669 `rtcConnectionStatusConnected/Connecting/Error` | color / height | `--text-feedback-positive/-warning/-critical` / 20 | voice_panel.rs `voice_panel` | ✅ |
| channel text | 593586 `channel_9U6LUN:hover` | decoration | underline | voice_panel.rs `voice_panel` | ✅ |
| noise / disconnect | 593586 `voiceButtonsContainer_9U6LUN` | gap | `--space-4` | voice_panel.rs `voice_panel` | ✅ |
| action buttons | 593586 `actionButtons_9U6LUN` | layout / gap | equal columns / `--space-xs` | voice_panel.rs `voice_panel` | ✅ |
| action button | 593586 `button_9U6LUN.buttonColor` | height / radius / border / bg / hover / color | `--space-32` / `--radius-sm` / `--border-muted` / `--control-secondary-background-default` / `-hover` / `--control-secondary-text-default` | voice_panel.rs `action_button` | ✅ |

Notes:
- Font sizes are set from JS text variants and don't appear in the CSS: channel 16/medium, category 14/medium sentence case, status 16/semibold. They were checked against the current client by eye.
- Not done yet: the title bar back/forward buttons (`backForwardButtons_nS9Yqn`, they need a navigation history), and the account popout that replaces the old status picker.

## Favorites server (client JS 2026-11-24, local cache `web.b63d846b329d6470.js`)

The account has no favorites and `favorites.guild_visible` unset, so the client shows no star:
everything below comes from the client JS and the CSS, not from a live render. Nothing was
written to the account; the write path is covered by unit tests only.

| element | source | property | value | our code | status |
|---|---|---|---|---|---|
| proto | `discord_protos.discord_users.v1.Favorites` (PreloadedUserSettings 15) | fields | 1 `favorite_channels` map<fixed64, FavoriteChannel>, 2 `muted`, 3 `guild_visible` BoolValue, 4 `auto_add_joined_threads`, 5 `prune_inactive_threads` BoolValue, 6 `prune_inactive_threads_days` uint32, 7 `prune_all_favorited_threads`, 8 `auto_add_joined_threads_per_guild`; FavoriteChannel 1 nickname, 2 type (UNSET 0, REFERENCE_ORIGINAL 1, CATEGORY 2), 3 position uint32, 4 parent_id fixed64, 5 channel_type UInt32Value, 6 collapsed | phoenix-api rest/favorites.rs | ✅ wire test |
| read | `FavoriteStore` (module 181079) | source | READY / `GET settings-proto/1` and `USER_SETTINGS_PROTO_UPDATE` (already reloads the synced settings) | account.rs synced_raw → `SyncedSettings::favorites` | ✅ |
| shown | FavoriteStore `favoriteGuildEnabled` | rule | `guild_visible.value ?? (map not empty)` | favorites::enabled | ✅ |
| write | `j()` → `updateAsync("favorites", …)` | request | `PATCH /users/@me/settings-proto/1` with only field 15, the whole new Favorites message | `RestClient::patch_favorites`, `SettingsPatch::Favorites` | ✅ unit tests, never sent |
| add | module 794412 `K` | logic | skip existing; limit 3 (200 with `premium_type` 2), 200 entries max, categories do not count; position = max + 1; parent kept only if it is a category; first add turns `guild_visible` on when the server was hidden | Favorites::add / add_to_category | ✅ tests |
| remove | `X` | logic | delete; children of a removed category go back to parent 0 | Favorites::remove | ✅ test |
| prune | `x` | logic | drop unknown channels (threads by `channel_type` 10/11/12 kept) and channels without VIEW_CHANNEL; fill a missing `channel_type` | Favorites::prune | ✅ test (Phoenix only knows "missing" / "visible") |
| new category id | `W` | id | snowflake of now (`fromTimestamp(Date.now())`) | snowflake_at | ✅ |
| rail | GuildsBar `rn` | order | home, Favorites, unread DMs, separator | server_bar favorites | ✅ |
| rail tile | `eZ` (NavItem + StarIcon 20×20, BlobMask) | look | home-like tile, blurple when selected / hovered, mention badge = sum of the favorite channels' mentions, unread pill unless `muted`; tooltip "Favoris" (`wMWyci`) | server_bar favorites | ✅ |
| rail tooltip voice rows | `ez`/`eX`/`e$` in the web bundle, classes 785332 `row__-eZlF` (+row mt 8, rowIcon mr 8, `--custom-guild-tooltip-icon-size` 20, `--interactive-text-default`), facepile 104171 size 24 max 6 (`avatarContainerMasked` mr -6, `moreUsers` r12 12px semibold `--background-base-lowest`) | VoiceNormalIcon row with the users of favorite voice channels (stage speakers, streams and activities rows not built) | server_bar `right_tooltip_with`, favorites::voice_users | ✅ voice row; ⚠ stage/stream/activity rows and the avatar cut-out mask |
| route | `@favorites` / guild id "373" | — | `/channels/@favorites/<channel>`; the last channel is reopened | `app.favorites_view`, `favorites_last` | ✅ (the selected guild stays the channel's real one, so permissions, members and search keep working) |
| list | GuildChannelStore `k` + `R` (module 774452) | order | uncategorized first, then categories by position; inside each, text-like then voice, by position; nickname replaces the name except for DMs; category record = favorite `nickname` | Favorites::rows, favorites::rows | ✅ test |
| collapsed | FavoriteChannel 6 | — | the client syncs it (`es`, batched PATCH) | read once into the local collapsed set; toggling stays local | ⚠ |
| header | `favorites-header-popout` (module 425912), `favorite-guild-header-add-context` (module 329671, chunk 879948) | — | "Favoris" title with the dropdown chevron (close icon while open), menu Thread Settings / Hide Favorites (danger); + button (PlusMediumIcon, tooltip `G9fGlP`) opening Add to Favorites / Create Category; the rail tile right-click (`favorite-server-context`, module 806280) shows the same items as the header | sidebar favorites_header, nav_menus favorites_items / favorites_add_items | ✅ wt/real-modals (`real_shots PHX_VIEW=favorites PHX_OPEN=favheader|favaddmenu|favmenu`); ✅ Thread Settings = modal 167846, built by b81fdd3d (see inventory.md `favorite-server-context`) |
| menu | `m_` + dumps vanilla-menu-channel / -dm / -group-dm | rows | "Ajouter aux favoris" > "Favoris" + one row per category; checked row removes, another row moves | actions/channel.rs favorite_item | ✅ (was a local file before) |
| empty view | module 205375 (chunk `e7fb183d554ee540.js`, `FAVORITES_GUILD_HOME`) + classes 630942 / 906114 / 326879 (live webpack, read only) | — | page `noChannelFavorites_RvzdCg` (`--background-gradient-lower`/`--background-base-lower`, 1px `--app-frame-border` top); centred `wrapper_PDJTSB` max-width 440; Stack gap md: CDN art `88fc1694…png` 197×107, title `wh+Rz1` « Tes conversations favorites, toutes au même endroit. » heading-xl/semibold `--text-default` max-width 321 centred, note `+SuGKb` body-md `--text-muted` mt 8, primary md button `6kk0gM` « Ajouter aux favoris » with PlusMediumIcon; strings from the 708526 locale chunks (fr `dba6da951a34f3c6.js`, en `d94a93a5568872ff.js` fetched from the CDN). `D5SN5/` and `6uDHk6` exist only in the string tables: no chunk of this build uses them (CDN sweep of the 4842 chunks missing locally) | favorites::empty_page, ui/home page_content | ✅ real_shots 7f56888f (`PHX_VIEW=favorites PHX_FAVORITES=empty`); ⚠ the app title bar still says « Amis » with the friends glyph in this view (the client's top bar for the favorites guild was not measured) |
| add modal | module 677993 (chunk `b14bd01b2fc652b8.js`): destination search (`5h0QOP` « Rechercher »), up to 20 picks, full-width primary md button `xKXcSu` / `LbCa8x` « Ajouter aux favoris ({count}) », empty text `kQL9be`, then `S_({channelIds, parentId:null})` | Forward modal in `favorites` mode (voice, stage and forum channels too, channels already in favorites hidden) → favorites::add_many | ✅ real_shots 7f56888f (`PHX_OPEN=favadd`); ✅ rows = `61916.s` → list item 359378 (chunk 766031 `2b38133e3bfd0a5b.js`, CSS `destinationRow_dhbHaH` in `0aa7dbd4ea59bbd0.css`): min-height 48, radius 8, margin 0 −8, padding 0 8, hover `--interactive-background-hover`, identity gap 12, label body-md/semibold `--text-strong`, sub-label body-xs `--text-muted` (server name, username for DMs), trailing checkbox indicator ml 16, disabled past the limit (`disableSelection`: icon at 50 %, muted label, not-allowed); server channels use 255266 (chunk 692639): guild icon 24 radius 8 (acronym 13/11/10 px on `--background-mod-strong`) with the channel icon 12 in a 4 px padded `--background-mod-strong` badge at (12, 9). Shared with the Forward modal, which uses the same rows in the client (`forward.rs share_row`). ⚠ the badge cut-out mask and thread sub-labels (parent channel • time) are not drawn |
| real_shots | example | env | `PHX_VIEW=favorites PHX_FAVORITES="id,id|Catégorie:id,id"` builds favorites in memory only (save_settings is denied there) | examples/real_shots.rs | ✅ |
| rail + view render | real_shots 2026-11-24 (Aperture Sciences channels injected in memory) | — | star tile selected (blurple + pill), "Favoris" header, uncategorized channel then a category | — | ✅ (a READY that arrives after the view setup calls `select_guild` and leaves the view: in real_shots rerun; in the app a full resync does the same ⚠) |

# Settings wiring audit

Every switch/select/radio of `crates/phoenix-app/src/ui/settings/pages.rs`:
where its value is stored and what it changes in the UI.

* Synced fields come from the protobuf descriptors of the 2026 web client
  (`discord_protos.discord_users.v1.PreloadedUserSettings.*`, local Discord
  cache, `web.*.js`). Table: `ui/settings/synced_map.rs` (read with
  `proto_read`, written with `SettingChange::Path`, merge-appended to the
  current blob so unknown fields survive).
* Local controls (client keeps them in localStorage / native settings) live
  in `LocalPrefs::ui` (`~/.config/discord-phoenix/settings.json`).
* REST routes: consents (`/users/@me/consent`) and e-mail preferences
  (`/users/@me/email-settings`). Application Test Mode is session-only in the
  client too.
* Sources (2026-11-24): every "local prefs" row was checked against the
  client's settings definitions (chunk `52b6e35…`, module 253932 maps the
  `U.*` settings to proto paths): Accessibility / StreamerMode /
  NotificationSettings / native stores stay local; the values shown by the
  live client were read through CDP without toggling anything.
* Testing never touched the user's account: unit tests encode/decode the
  blob offline (`phoenix-api` `path_changes_roundtrip`, `synced_map` tests);
  the mock backend applies patches in memory.

| id | control | storage | effect in Phoenix | status |
|---|---|---|---|---|
| data-improve | Use data to improve Discord | REST `/users/@me/consent` usage_statistics | grant/revoke on Discord | ✅ |
| data-personalize | Use data to customize my Discord experience | REST `/users/@me/consent` personalization | grant/revoke on Discord | ✅ |
| data-sponsored | Use my Discord activity to personalize sponsored content | settings-proto `privacy.drops_opted_out` (8, 18), inverted (client `DATA_USAGE_QUESTS_SETTING` = `!dropsOptedOut`) | enforced by Discord; greyed out when READY `ad_personalization_toggles_disabled` | ✅ live value off, matches |
| data-third | Use third-party data to personalize sponsored content | settings-proto `privacy.quests_3p_data_opted_out` (8, 30), inverted | enforced by Discord; greyed out (title/description/indicator .5, not-allowed) while data-sponsored is off or ad toggles are disabled, like the client `NP()` | ✅ live: off + disabled, matches |
| data-discovery | Show my message content from Discoverable channels on web pages | settings-proto `privacy.swp_message_promotion_opted_out` (8, 35), inverted; unset = READY `swp_message_promotion_default_opted_out` (default true); locked by READY `swp_message_promotion_locked` (default true) | enforced by Discord | ✅ live: off, enabled, matches |
| data-clips | Allow my voice to be recorded in Clips | settings-proto `clips.allow_voice_recording` (19, 1) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| share-profile | Share my full profile with | settings-proto `privacy.profile_visibility` (8, 32) enum FRIENDS_AND_ALL_GUILDS 3 / FRIENDS_AND_SMALL_GUILDS 2 / FRIENDS_ONLY 1 (UNSET 0 = all) | enforced by Discord | ✅ live: Friends only, matches |
| share-update | Share when I update my profile | settings-proto `notifications.notify_friends_on_profile_update` (7, 24) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| e2ee-codes | Enable persistent verification codes | local in the client too (`PERSISTENT_VERIFICATION_CODES_SETTING`, secure-frames store) → local prefs | — (needs E2EE voice) | 🟡 stored, no effect yet |
| cf-friends | DMs from friends | settings-proto `text_and_images.explicit_content_settings.explicit_content_friend_dm` (6, 29, 2) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| cf-others | DMs from others | settings-proto `text_and_images.explicit_content_settings.explicit_content_non_friend_dm` (6, 29, 3) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| cf-servers | Messages in server channels | settings-proto `text_and_images.explicit_content_settings.explicit_content_guilds` (6, 29, 1) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| cfv-friends / cfv-others / cfv-servers | Violent content filters | settings-proto `text_and_images.gore_content_settings.*` (6, 32, 1..3) | enforced by Discord | ✅ |
| age-cmds | Allow access to age-restricted app commands in DMs | settings-proto `text_and_images.view_nsfw_commands` (6, 23) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| age-ios | Allow access to age-restricted servers on iOS | settings-proto `text_and_images.view_nsfw_guilds` (6, 20) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| spam | Automatically filter suspected spam messages | settings-proto `text_and_images.dm_spam_filter_v2` (6, 27) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| server-dms | Direct Messages | settings-proto (modelled field) | existing | ✅ |
| msg-requests | Message Requests | settings-proto `privacy.default_message_request_restricted` (8, 17) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| fr-everyone | Everyone | settings-proto `privacy.friend_source_flags` (8, 11) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| fr-fof | Friends of friends | settings-proto `privacy.friend_source_flags` (8, 11) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| fr-members | Server members | settings-proto `privacy.friend_source_flags` (8, 11) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| fr-msg | Show custom messages | settings-proto `privacy.hide_friend_request_notes` (8, 33) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| game-dms | Allow game friends to send me DMs and invites | settings-proto `privacy.allow_game_friend_dms_in_discord` (8, 26) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| ingame | Show DMs in games | settings-proto `privacy.slayer_sdk_receive_dms_in_game` (8, 28) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| desktop-notif | Enable Desktop Notifications | local prefs (settings.json) | notifications::set_enabled (existing) | ✅ |
| n-stream | People I know start streaming live in small servers | settings-proto `voice_and_video.stream_notifications_enabled` (5, 7) (client `GO_LIVE_NOTIFICATIONS`) | enforced by Discord (pushes) | ✅ |
| n-anniv | A friend and I celebrate a friendship anniversary | settings-proto `notifications.enable_friend_anniversary_notifications` (7, 14) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| n-online | Friends come online | settings-proto `notifications.enable_friend_online_notifications` (7, 12) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| n-event | An event is starting soon in a server | settings-proto `notifications.enable_upcoming_server_event_notifications` (7, 22) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| n-profile | Friends update their profile | settings-proto `notifications.enable_profile_updates_notifications` (7, 16) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| n-react | Someone reacts to my messages | settings-proto `notifications.reaction_notifications` (7, 7) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| notif-sounds | Disable all notification sounds | local prefs (settings.json) | existing | ✅ |
| badge | Enable Unread Message Badge | local in the client too (NotificationSettingsStore `disableUnreadBadge`) → local prefs | — | 🟡 stored, no effect yet |
| e-comm | Communication emails | REST `/users/@me/email-settings` categories.communication | e-mails sent by Discord | ✅ |
| e-social | Social emails | REST `/users/@me/email-settings` categories.social | e-mails sent by Discord | ✅ |
| e-news | Announcements and updates | REST `/users/@me/email-settings` categories.updates_and_announcements | e-mails sent by Discord | ✅ |
| e-tips | Tips emails | REST `/users/@me/email-settings` categories.tips | e-mails sent by Discord | ✅ |
| e-recs | Recommendations emails | REST `/users/@me/email-settings` categories.recommendations_and_events | e-mails sent by Discord | ✅ |
| push-delay | Mobile push notification delay | settings-proto `voice_and_video.afk_timeout` (5, 6) seconds, options 1..10 minutes like the client (`MOBILE_NOTIFICATION_DELAY`), unset = 60 | enforced by Discord (pushes) | ✅ live: 1 minute |
| tts | Allow playback and usage of /tts command | settings-proto `text_and_images.enable_tts_command` (6, 16) | — | 🟡 stored, no effect yet |
| read-all | Read all messages out loud | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| theme-sync | Match device theme | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| theme-devices | Sync theme across my devices | local in the client too (`shouldSync("appearance")`, disabled while Match device theme is on) → local prefs | — | 🟡 stored, no effect yet |
| theme-profiles | Apply theme to other users' profiles | local in the client too (AccessibilityStore `syncProfileThemeWithUserTheme`) → local prefs | — | 🟡 stored, no effect yet |
| media-links | When posted as links to chat | settings-proto `text_and_images.inline_embed_media` (6, 10) | image/gifv/video embeds hidden when off (chat.rs show_embed) | ✅ |
| media-uploads | When uploaded directly to Discord | settings-proto `text_and_images.inline_attachment_media` (6, 9) | uploaded images/videos render as file cards when off (chat.rs attachment) | ✅ |
| embeds | Show embeds and preview website links | settings-proto `text_and_images.render_embeds` (6, 12) | all embeds hidden when off (chat.rs show_embed) | ✅ |
| reactions | Show emoji reactions | settings-proto `text_and_images.render_reactions` (6, 13) | reaction bar hidden when off (chat.rs) | ✅ |
| spoilers | Show spoiler content | settings-proto `text_and_images.render_spoilers` (6, 4) | "Always" reveals text + media spoilers (richtext, media::is_revealed); "If moderator" reveals them where I have Manage Messages / Administrator | ✅ |
| split-threads | Open threads in split view | settings-proto `text_and_images.use_thread_sidebar` (6, 3) | — | 🟡 stored, no effect yet |
| avatars | Show user avatars (available in compact message display) | settings-proto `text_and_images.display_compact_avatars` (6, 41) | Compact mode: show an avatar beside each message header; default/cozy mode keeps its normal group avatars | ⚠ wired; synthetic enabled/disabled render checked, official enabled geometry not compared |
| favs | Show favorite servers (beta) | settings-proto `favorites.guild_visible` (15, 3) (client `FavoriteStore.favoriteGuildEnabled`) | — (Phoenix has no Favorites server yet) | 🟡 synced, no effect yet |
| md-preview | Preview emojis, mentions and markdown syntax as you type | settings-proto `text_and_images.use_rich_chat_input` (6, 2) | — | 🟡 stored, no effect yet |
| emoticons | Automatically convert emoticons in your messages to emoji | settings-proto `text_and_images.convert_emoticons` (6, 21) | ":)" → 🙂 on send/edit (settings/emoticons.rs) | ✅ |
| ac-stickers | Show sticker suggestions in autocomplete | settings-proto `text_and_images.include_stickers_in_autocomplete` (6, 28) | no sticker rows in the :emoji autocomplete when off (autocomplete.rs) | ✅ |
| ac-games | Show games in autocomplete | settings-proto `text_and_images.include_game_mentions_in_autocomplete` (6, 39) | gate the `@game` search/suggestion flow; selected result inserts a Discord game mention/embed | 🟡 stored, no effect yet; game catalog/search route and result shape not yet established |
| send-btn | Show the Send Message button | local prefs (settings.json) | button appears for a nonempty, sendable draft and calls the existing submit path | ⚠ wired; visibility rule and screenshot not yet compared with the official client |
| dm-search | By default, search in DMs… | settings-proto `text_and_images.is_cross_dm_search_enabled` (6, 36) | — | 🟡 stored, no effect yet |
| streamer | Enable Streamer Mode | local prefs | with st-info: e-mail/phone stay masked; with st-notifs: desktop notifications muted | ✅ |
| streamer-auto | Automatically enable if OBS or XSplit is running | local in the client too (StreamerModeStore `autoToggle`, default true) → local prefs | — (no process scan) | 🟡 stored, no effect yet |
| st-info | Hide personal information like email, connected accounts and notes | local prefs | account e-mail / phone cannot be revealed (settings/view.rs) | ✅ |
| st-invites | Hide invite links to my Discord servers | local in the client too (StreamerModeStore `hideInstantInvites`, default true) → local prefs | — | 🟡 stored, no effect yet |
| st-sounds | Disable all sound effects | local in the client too (StreamerModeStore `disableSounds`, default true) → local prefs | — Phoenix plays no sound effects (Discord sounds are not bundled) | 🟡 stored, nothing to mute |
| st-notifs | Disable notifications | local prefs | desktop notifications off while streaming (LocalPrefs::notifications_on) | ✅ |
| underline | Always underline links | local prefs (settings.json) | links underlined (richtext run_for) | ✅ |
| name-styles | Display name styles | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| density | UI Density | local prefs (settings.json) | --space-* tokens (existing) | ✅ |
| display | Chat message display | settings-proto (modelled field) | compact mode (existing) | ✅ |
| sat-custom | Apply saturation to custom colors | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| contrast | Enable High Contrast Mode | local in the client too (AccessibilityStore `isHighContrastModeEnabled`) → local prefs | — | 🟡 stored, no effect yet (needs the high-contrast token set) |
| roles | Role colors | local prefs | 0 tints names; 1 draws an 8px role dot after the name; 2 none (app.member_color / author_role_dot) | ✅ |
| indicators | Show switch indicators | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| reduced-motion | Enable Reduced Motion | local prefs (settings.json) | modal, tooltip zoom, rail tip and guild pill animations ~0 ms (effects::motion) | ✅ |
| motion-sync | Sync with computer settings | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| gifs | Play GIFs when Discord is focused | settings-proto `text_and_images.gif_auto_play` (6, 11) | gifv embeds show the still frame when off (chat.rs embed) | ✅ |
| anim-emoji | Play animated emoji | settings-proto `text_and_images.animate_emoji` (6, 14) | custom emoji animate (media::emoji) | ✅ |
| stickers | Sticker animations | settings-proto `text_and_images.animate_stickers` (6, 15) | animated stickers show their first frame unless Always (chat.rs still_sticker); hover-to-play not done | ✅ |
| alt-text | Show image descriptions | settings-proto `text_and_images.view_image_descriptions` (6, 7) | — | 🟡 stored, no effect yet |
| legacy-input | Use the legacy chat input | settings-proto `text_and_images.use_legacy_chat_input` (6, 24) | — | 🟡 stored, no effect yet |
| startup | Open Discord when your computer starts | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| tray | Minimize Discord to the tray | local prefs (minimize_to_tray) | close hides to the tray icon (tray agent) | ✅ |
| hwaccel | Enable hardware acceleration | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| time-format | Time format | settings-proto `appearance.timestamp_hour_cycle` (13, 9) | 12h/24h clocks and <t:..:t/T> (i18n::clock, markdown) | ✅ |
| act-share | Share my activity | settings-proto `status.show_current_game` (11, 3) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| act-online | Share when I come online | settings-proto `notifications.notify_friends_on_come_online` (7, 25) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| act-stream | Share when I start streaming | settings-proto `notifications.notify_friends_on_go_live` (7, 2) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| act-where | Share my activity automatically when I join a server | settings-proto `privacy.default_guilds_activity_restricted_v2` (8, 29) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| act-friends | Allow my friends to join my game | settings-proto `privacy.allow_activity_party_privacy_friends` (8, 1) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| act-voice | Allow voice channel participants to join my game | settings-proto `privacy.allow_activity_party_privacy_voice_channel` (8, 2) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| dev-mode | Developer Mode | settings-proto (modelled field) | Copy ID menus (existing) | ✅ |
| app-test | Application Test Mode | session only in the client too (`testModeApplicationId`, the switch opens the test-mode modal) | off at start | ✅ same storage (modal not reproduced) |

| saturation (slider) | Saturation | local prefs | theme tokens desaturated (`theme::saturate`) | ✅ |

## Known gaps
* Stored, no effect yet: high contrast (needs the client's high-contrast token set), st-invites / st-sounds, game autocomplete, split thread view, startup / hardware acceleration.
* Compact avatars are gated by `display_compact_avatars`; the 40px size/offset still needs comparison in the official enabled state.
* Live updates: USER_SETTINGS_PROTO_UPDATE (type 1) now reloads the synced settings (`AppEvent::SettingsChanged`).

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
* "session only": not settings-proto in the client either — consents
  (`/users/@me/consent`), e-mail preferences (`/users/@me/email-settings`),
  profile visibility enum, "streaming in small servers" (field not
  identified) and Application Test Mode (session-only in the client too).
  Not wired yet.
* Testing never touched the user's account: unit tests encode/decode the
  blob offline (`phoenix-api` `path_changes_roundtrip`, `synced_map` tests);
  the mock backend applies patches in memory.

| id | control | storage | effect in Phoenix | status |
|---|---|---|---|---|
| data-improve | Use data to improve Discord | session only | — | ❌ |
| data-personalize | Use data to customize my Discord experience | session only | — | ❌ |
| data-sponsored | Use my Discord activity to personalize sponsored content | session only | — | ❌ |
| data-third | Use third-party data to personalize sponsored content | session only | — | ❌ |
| data-discovery | Show my message content from Discoverable channels on web pages | session only | — | ❌ |
| data-clips | Allow my voice to be recorded in Clips | settings-proto `clips.allow_voice_recording` (19, 1) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| share-profile | Share my full profile with | session only | — | ❌ |
| share-update | Share when I update my profile | settings-proto `notifications.notify_friends_on_profile_update` (7, 24) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| e2ee-codes | Enable persistent verification codes | local prefs (settings.json) | — | 🟡 stored, no effect yet |
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
| n-stream | People I know start streaming live in small servers | session only | — | ❌ |
| n-anniv | A friend and I celebrate a friendship anniversary | settings-proto `notifications.enable_friend_anniversary_notifications` (7, 14) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| n-online | Friends come online | settings-proto `notifications.enable_friend_online_notifications` (7, 12) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| n-event | An event is starting soon in a server | settings-proto `notifications.enable_upcoming_server_event_notifications` (7, 22) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| n-profile | Friends update their profile | settings-proto `notifications.enable_profile_updates_notifications` (7, 16) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| n-react | Someone reacts to my messages | settings-proto `notifications.reaction_notifications` (7, 7) | enforced by Discord (server-side: DMs, friend requests, pushes, presence) | ✅ |
| notif-sounds | Disable all notification sounds | local prefs (settings.json) | existing | ✅ |
| badge | Enable Unread Message Badge | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| e-comm | Communication emails | session only | — | ❌ |
| e-social | Social emails | session only | — | ❌ |
| e-news | Announcements and updates | session only | — | ❌ |
| e-tips | Tips emails | session only | — | ❌ |
| e-recs | Recommendations emails | session only | — | ❌ |
| push-delay | Mobile push notification delay | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| tts | Allow playback and usage of /tts command | settings-proto `text_and_images.enable_tts_command` (6, 16) | — | 🟡 stored, no effect yet |
| read-all | Read all messages out loud | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| theme-sync | Match device theme | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| theme-devices | Sync theme across my devices | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| theme-profiles | Apply theme to other users' profiles | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| media-links | When posted as links to chat | settings-proto `text_and_images.inline_embed_media` (6, 10) | image/gifv/video embeds hidden when off (chat.rs show_embed) | ✅ |
| media-uploads | When uploaded directly to Discord | settings-proto `text_and_images.inline_attachment_media` (6, 9) | uploaded images/videos render as file cards when off (chat.rs attachment) | ✅ |
| embeds | Show embeds and preview website links | settings-proto `text_and_images.render_embeds` (6, 12) | all embeds hidden when off (chat.rs show_embed) | ✅ |
| reactions | Show emoji reactions | settings-proto `text_and_images.render_reactions` (6, 13) | reaction bar hidden when off (chat.rs) | ✅ |
| spoilers | Show spoiler content | settings-proto `text_and_images.render_spoilers` (6, 4) | "Always" reveals text + media spoilers (richtext, media::is_revealed); "If moderator" = on click | ✅ |
| split-threads | Open threads in split view | settings-proto `text_and_images.use_thread_sidebar` (6, 3) | — | 🟡 stored, no effect yet |
| avatars | Show user avatars | settings-proto `text_and_images.display_compact_avatars` (6, 41) | — | 🟡 stored, no effect yet |
| favs | Show favorite servers | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| md-preview | Preview emojis, mentions and markdown syntax as you type | settings-proto `text_and_images.use_rich_chat_input` (6, 2) | — | 🟡 stored, no effect yet |
| emoticons | Automatically convert emoticons in your messages to emoji | settings-proto `text_and_images.convert_emoticons` (6, 21) | ":)" → 🙂 on send/edit (settings/emoticons.rs) | ✅ |
| ac-stickers | Show sticker suggestions in autocomplete | settings-proto `text_and_images.include_stickers_in_autocomplete` (6, 28) | no sticker rows in the :emoji autocomplete when off (autocomplete.rs) | ✅ |
| ac-games | Show games in autocomplete | settings-proto `text_and_images.include_game_mentions_in_autocomplete` (6, 39) | — | 🟡 stored, no effect yet |
| send-btn | Show the Send Message button | local prefs (settings.json) | button appears for a nonempty, sendable draft and calls the existing submit path | ⚠ wired; visibility rule and screenshot not yet compared with the official client |
| dm-search | By default, search in DMs… | settings-proto `text_and_images.is_cross_dm_search_enabled` (6, 36) | — | 🟡 stored, no effect yet |
| streamer | Enable Streamer Mode | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| streamer-auto | Automatically enable if OBS or XSplit is running | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| st-info | Hide personal information like email, connected accounts and notes | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| st-invites | Hide invite links to my Discord servers | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| st-sounds | Disable all sound effects | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| st-notifs | Disable notifications | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| underline | Always underline links | local prefs (settings.json) | links underlined (richtext run_for) | ✅ |
| name-styles | Display name styles | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| density | UI Density | local prefs (settings.json) | --space-* tokens (existing) | ✅ |
| display | Chat message display | settings-proto (modelled field) | compact mode (existing) | ✅ |
| sat-custom | Apply saturation to custom colors | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| contrast | Enable High Contrast Mode | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| roles | Role colors | local prefs (settings.json) | "Hide"/"Next to names" stop tinting names (app.member_color); dot not drawn | ✅ |
| indicators | Show switch indicators | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| reduced-motion | Enable Reduced Motion | local prefs (settings.json) | modal, tooltip zoom, rail tip and guild pill animations ~0 ms (effects::motion) | ✅ |
| motion-sync | Sync with computer settings | local prefs (settings.json) | — | 🟡 stored, no effect yet |
| gifs | Play GIFs when Discord is focused | settings-proto `text_and_images.gif_auto_play` (6, 11) | gifv embeds show the still frame when off (chat.rs embed) | ✅ |
| anim-emoji | Play animated emoji | local prefs (settings.json) | custom emoji animate (media::emoji, existing) | ✅ |
| stickers | Sticker animations | settings-proto `text_and_images.animate_stickers` (6, 15) | — | 🟡 stored, no effect yet |
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
| app-test | Application Test Mode | session only | — | ❌ |

| saturation (slider) | Saturation | local prefs | theme tokens desaturated (`theme::saturate`) | ✅ |

## Known gaps
* `anim-emoji` is synced in the client (`text_and_images.animate_emoji`, 6/14) but stays a local pref here.
* Stored but without effect yet: compact avatars, game autocomplete,
  streamer mode, high contrast, sticker animations, split thread view,
  the notification toggles (server-side behaviour, nothing to render), system tray/startup/hw-accel.
* Role colors "Next to names" should draw a colored dot; currently it just stops tinting.
* Spoilers "If moderator" behaves as "On click".
* Gateway `USER_SETTINGS_PROTO_UPDATE` is not applied live (values refresh on the next load/save).

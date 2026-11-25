# Desktop notifications

Source: the 2026-11-25 client bundle in `~/discord-phoenix-ref/current-client/js` (read locally):
`998e3527691a7525.js` (NotificationStore, webpack module 534765), `web.b63d846b329d6470.js`
modules 411238 (`shouldNotify` `lx`, `K`, notification text `TB`), 119622 (NotificationUtils
`showNotification`), 451909 (`unparseWithMeta`), 47167 (channel names `m1`), 779014
(`SystemMessageUtils.stringify`), 441711 (friend request notifications), 607508 (thread level),
95701 (channel type sets), 253932 (`quietMode`). Strings from `jsref/en2.js` / `jsref/fr.js`.
CDP on the Linux client (read only): `DiscordNative.features.supports("notifications") === false`,
so Linux uses the HTML5 `Notification` path (Electron → libnotify) and keeps **one** notification
per channel (`tu = supports("notifications") ? 20 : 1`).

Code: `crates/phoenix-app/src/notifications.rs` (rules, gateway hooks, click routing),
`notifications/content.rs` (title / body), `notifications/dbus.rs` (Linux backend).

## Rules

| Rule | Client source | Phoenix | State |
|---|---|---|---|
| Desktop switch | `desktopType === NEVER` → no popup (sound only) | `LocalPrefs.desktop_notifications` → `notifications::enabled` | ✅ |
| Do Not Disturb, quiet mode, restricted hours | `tE()`, `K`: status DND, `quietMode` proto | `sounds::MessageCtx.dnd/quiet` (`show_message`, `gate_open`) | ✅ unit test; restricted hours (Family Center) ❌ not in Phoenix |
| Streamer mode | `showNotification`: `StreamerModeStore.disableNotifications` (streamer && "Disable notifications") | `LocalPrefs::notifications_on` (`st-notifs`) | ✅ |
| @silent | `SUPPRESS_NOTIFICATIONS` (1<<12) | `MessageFacts.suppressed` | ✅ unit test |
| Own messages | `K`: same user, except `SELF_MENTIONABLE_SYSTEM` (type 46, poll result) | `from_me`, `self_mentionable` | ✅ unit test |
| Spammer, blocked / ignored | `K`: `SPAMMER` (1<<20), `isBlockedOrIgnored`; `isBlockedOrIgnoredForMessage` (author, then `interaction_metadata.user`) | `spammer`, `blocked` (`Blocked` or `user_ignored`, author or interaction user) | ✅ unit test (`message_requests_and_ignored_users_from_cache`); guild space leaderboards ❌ not in Phoenix |
| Viewed channel | `!includeSelectedChannel && _(id)` with `includeSelectedChannel = !windowFocused`; `_` = selected channel or `getCurrentSidebarChannelId(selected)` (module 411238) | `focused && MessageCtx::viewing(channel)`: `selected` or `sidebar` (the open thread panel, `sounds::watch`); also gates the message sound (`message_sounds`), like `lx` does for `MESSAGE_CREATE` | ✅ unit test |
| Mutes | `allowNoMessages`: guild / category / channel muted (`mute_config.end_time`), level Nothing | `NotifyPrefs::allow_no_messages` (wt/mute) | ✅ unit test |
| Levels | `allowAllMessages` → all, else `isMentioned` | `resolved_level` channel → category → guild → server default | ✅ unit test |
| @everyone / roles | `isMentioned` with `suppressEveryone` / `suppressRoles` of the guild (DMs: the private entry) | `GuildPrefs.suppress_everyone/roles` | ✅ unit test |
| Text in voice | `OU` (voice, stage): "all messages" only while connected to that voice channel | `voice_text`, `in_voice` (cache voice states) | ✅ unit test |
| Threads | `JoinedThreadsStore.isMuted`; level from the thread member flags (ALL 2 / MENTIONS 4 / NONE 8, not joined = nothing, else parent level, muted parent = nothing) | `thread_level` | ✅ unit test |
| Thread starter | uses the parent channel | `facts` | ✅ |
| Opt-in channels (onboarding) | `allowNoMessages`: `isOptInEnabled(guild)` (guild flag `OPT_IN_CHANNELS_ON` 16384, `n3`) `&& !isChannelRecordOrParentOptedIn(channel)` (override flag `OPT_IN_ENABLED` 4096, `vv`, on the channel or its record `parent_id`: category for a channel, parent channel for a thread; DMs never) | `NotifyPrefs::allow_no_messages`, `is_channel_or_parent_opted_in` | ✅ unit test (`opt_in_channels_and_message_requests`, `opt_in_uses_the_thread_parent_channel`); full server preview ❌ not in Phoenix |
| Message requests, game overlay focus, activities | `lx`: `MessageRequestStore.isMessageRequest(channel)` (module 380335: `isMessageRequest && !isSpam`, spam DMs are not filtered here), `getFocusedPID`, embedded activity | `MessageFacts::message_request` (`is_message_request && !is_spam`), blocks the popup and the sound | ✅ unit test; game overlay & embedded activity ❌ not in Phoenix |
| Rate limit | one tracked notification per channel on Linux (older one closed), message sound throttled 1 s | `dbus::Live` replaces the channel's notification (`replaces_id`); sounds: `sounds::throttled` | ✅ unit test |
| Clearing | `WINDOW_FOCUS` (selected channel), `CHANNEL_SELECT`, `MESSAGE_ACK` close the channel's notifications | `sync_focus` (render), `open_channel_chat`, gateway `MESSAGE_ACK` (`on_ack`; ignored for the selected channel while the window is in the background, where the client would not have acked) | ✅ |
| Friend requests | module 441711: `RELATIONSHIP_ADD` incoming, not ignored → title `username`, body "Demande d'ami envoyée.", tag user id, click opens the profile and Friends › Pending; `FRIEND_REQUEST_ACCEPTED` (friend that was outgoing) → "Demande d'ami acceptée.", click opens the DM | `on_relationship_add`, `relationship_toast` | ✅ unit test |
| Incoming calls | no `showNotification` for calls in the client (ringtone + in-app call only) | ringtone in `sounds::watch` | ✅ (nothing to show) |
| "Reply" action | the strings exist (`TBA5Xg`, `MJ+sFa`) but no desktop caller passes `actions` | none | ✅ (not offered, like the client) |
| "Show content" switch | no desktop setting in the 2026 client (`notifications` proto has none; the Notifications page has no such row) | none | ✅ (nothing to hide) |
| Threads created | `THREAD_CREATE` (`998e3527691a7525.js`): `tE()`, parent in `GUILD_THREADS_ONLY` (15, 16), gateway `newly_created`, `q1`: owner known, `K(me, owner, parent)` without the no-messages rule, parent not muted, not viewed, `getNewForumThreadsCreated(parent)` (override `NEW_FORUM_THREADS_ON` 16384 / `OFF` 8192, else level All). Title `2IGVl5` "Serveur (#forum)", body `jPhTvT` "post - Posted by Nom", owner avatar (round), click opens the thread, no sound | `thread_create_toast` | ✅ unit test (`thread_create_rules`) |
| Scheduled events | `GUILD_SCHEDULED_EVENT_UPDATE`: `notification_type` 1 (`EVENT_START`); stage (1) / voice (2): `channel_id`, not already in it (`tT`), `host_id` user (no `creator_id` fallback), `CONNECT + VIEW_CHANNEL`, title `bOu6Wn`, body `GV9L8u` "\"sujet\" with Nom", click joins the channel; external (3): title `bOu6Wn`, body `mYyaRB`, click opens the server. No mute or level check, guild icon 128 (static, square), no sound | `scheduled_event_toast` (`Target::Voice` / `Target::Guild`) | ✅ unit test (`scheduled_event_rules`); the event details modal on click ❌ (Phoenix opens the server) |
| Stage started | `STAGE_INSTANCE_UPDATE`: `send_start_notification`, not in it, `Wv` = `K(me, host, channel)` (no-messages rule included), `CONNECT + VIEW_CHANNEL`, once per instance (`tA`, seeded with every `stage_instances` id at `CONNECTION_OPEN`, cleared by `STAGE_INSTANCE_DELETE`). Title `bZ4Okd`, body `qTelnO`, guild icon, click joins the stage | `stage_instance_toast`, `on_ready` seeds `seen_stages` from `Guild::stage_instances` | ✅ unit test (`stage_instance_rules`) |
| Reminders | `MESSAGE_REMINDER_DUE` is **not** a gateway event: `MessageRemindersNotificationManager` (module 343509) arms one `setTimeout` on the first reminder with `dueAt > now` (if within a week) on `SAVED_MESSAGES_UPDATE` (GET `/users/@me/saved-messages` at `POST_CONNECTION_OPEN`), `SAVED_MESSAGE_CREATE`, `SAVED_MESSAGE_DELETE`. Then `tE()`, channel `saveData.channelId`, `TB` icon + body, title `IjZJB5` "⏰ Rappel", click jumps to the message | `notifications::reminders` (GET at READY, gateway `SAVED_MESSAGE_CREATE` / `DELETE`, one tokio timer), `reminder_toast` (`Target::Message`) | ✅ unit test (`reminders_fire_locally`) |
| Activity started | `ACTIVITY_START` (gateway `{user_id, activity}`): `tE()`, `activity.type` 0 (playing), user known; title `XoTWsI`, body `o4Aipn` "Nom started playing Jeu!", avatar, sound `message2` at 0.4, click opens the DM | `activity_start_toast`, `on_activity_start` | ✅ unit test (`activity_start_rules`) |
| Icon shape | `showNotification`: the canvas circle only when `isUserAvatar` (messages, threads, reminders, friends, activity); guild icons (events, stage) stay square | `Toast::round` → `dbus::avatar_image(bytes, round)` | ✅ |

## Taskbar badge (wt/setfx, checked)

Client (`web.b63d846b329d6470.js`, app root `Cd`): `count = getTotalMentionCount() + pendingIncoming`
(`RelationshipStore.getPendingCount()` without spam / ignored, plus game friend requests);
`0 && hasAnyUnread && !disableUnreadBadge` → `-1` (dot); `setBadge` on Linux is
`app.setBadgeCount(max(count, 0))`, so the dot never shows there and the "badge" switch does not
change the count. Phoenix `tray::TrayState`: `count = mentions + pending` sent as
`com.canonical.Unity.LauncherEntry` `count` / `count-visible` (what Electron's `setBadgeCount` does
on Linux), the switch only drives the unread tray icon. ✅ consistent; pending requests exclude spam and ignored users (game friends ❌ not in Phoenix).

## Content (`TB`)

| Part | Client | Phoenix | State |
|---|---|---|---|
| Icon | author avatar, server avatar if set, 128 px, clipped to a circle (canvas `arc`) | `content::avatar_url`, `dbus::avatar_image` + `circle` → `image-data` hint | ✅ unit test |
| Title | text, announcement, voice, threads: `Name (#salon, Catégorie)` with each part wrapped in U+2068/U+2069; threads `"fil"` then the parent; thread starter: the parent and its category; group DM `Name (Groupe)`; DMs and stage channels: the name alone. Name = server nickname, friend nickname in DMs, then display name | `content::title`, `CacheLookup::member_name` | ✅ unit test |
| Body | forwarded → "Message transféré"; sticker → "Autocollant envoyé : X"; poll → "[sondage] Q"; poll result → "Le sondage Q est clos"; system messages through `stringify` (join: 13 variants picked by `timestamp % 13`, pin, boost, group add / remove / leave); else `unparseWithMeta`: users `@pseudo`, roles `@rôle`, channels `#salon` / `#"fil"`, custom emoji `:nom:`, spoilers `<spoiler>`, timestamps formatted, code kept, mentions and code isolated; DMs drop ` -#` from `> -# *` | `content::body`, `content::unparse`, `content::system` | ✅ unit test; other system types (calls, stage, boosts tiers, …) return no notification ❌; activity invites, premium referral and checkpoint cards ❌ |
| Empty body | first embed `title description` / description / title / first field `name value`; voice message → "Message vocal envoyé"; attachment → "fichier uploadé" (title + extension) | `content::fallback` | ✅ unit test |

## Linux backend (`org.freedesktop.Notifications`)

`Notify("Discord Phoenix", replaces_id, "discord-phoenix", title, body, ["default", ""], hints, -1)`
with hints `desktop-entry`, `category=im.received`, `suppress-sound=true` (the cue plays through
`sounds`, like the client's `silent: true`), `urgency=1`, `image-data` (circular avatar). Body is
escaped when the server reports `body-markup`. One notification per channel: a new message
replaces the previous one (`replaces_id`), the channel's notification is closed with
`CloseNotification` when it is read. A click (`ActionInvoked "default"`) takes the
`ActivationToken` sent just before (xdg-activation, same path as the tray, `tray::activation`),
raises the window and opens the channel (`open_channel_chat`; voice channels open their chat,
like `updateChatOpen`). Windows and macOS use notify-rust (title, body, app icon), with no click
routing ❌.

## Real test

`scripts/notify/real_test.sh` runs `examples/notify_probe.rs` inside `dbus-run-session` with a
fake notification server (`scripts/notify/fake_notifyd.py`) and `dbus-monitor`; nothing reaches the
desktop bus (both refuse to run on `/run/user/…/bus`). The probe connects to the real account,
listens to #phoenix only, applies `show_message` both as us and as another account (a message we
send is never notified to ourselves), shows the toast, and checks that the fake server's
`ActivationToken` + `ActionInvoked` come back as a click on #phoenix. Captures go to
`/mnt/windata/project-phoenix/compare/notifs` (private, never committed). The token is read on the
desktop bus (keyring) before the probe switches `DBUS_SESSION_BUS_ADDRESS` to the private bus.

Run of 2026-11-25 14:00 (message sent from the official client in #phoenix): as us → not shown (own
message, `K`), as another account → `Notify("Discord Phoenix", 0, "discord-phoenix",
"⁨Yohan⁩ (⁨#〢💬・phoenix⁩, ⁨📝 ─ ☲☲ Communautaire ☲☲⁩)", "<message>", ["default", ""],
{desktop-entry, category=im.received, suppress-sound, urgency=1, image-data 128×128 RGBA circle}, -1)`;
the fake server's `ActivationToken` + `ActionInvoked` came back as a click on #phoenix with the
token, then `CloseNotification(1)` when the channel was opened. ✅

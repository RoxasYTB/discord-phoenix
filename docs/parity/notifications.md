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
| Spammer, blocked / ignored | `K`: `SPAMMER` (1<<20), `isBlockedOrIgnored` | `spammer`, `blocked` | ✅ unit test (ignored users: ❌, Phoenix has no ignore list) |
| Viewed channel | `!includeSelectedChannel && selected` with `includeSelectedChannel = !windowFocused` | `focused && selected == channel` | ✅ unit test; the thread side panel (`getCurrentSidebarChannelId`) is not counted ❌ |
| Mutes | `allowNoMessages`: guild / category / channel muted (`mute_config.end_time`), level Nothing | `NotifyPrefs::allow_no_messages` (wt/mute) | ✅ unit test |
| Levels | `allowAllMessages` → all, else `isMentioned` | `resolved_level` channel → category → guild → server default | ✅ unit test |
| @everyone / roles | `isMentioned` with `suppressEveryone` / `suppressRoles` of the guild (DMs: the private entry) | `GuildPrefs.suppress_everyone/roles` | ✅ unit test |
| Text in voice | `OU` (voice, stage): "all messages" only while connected to that voice channel | `voice_text`, `in_voice` (cache voice states) | ✅ unit test |
| Threads | `JoinedThreadsStore.isMuted`; level from the thread member flags (ALL 2 / MENTIONS 4 / NONE 8, not joined = nothing, else parent level, muted parent = nothing) | `thread_level` | ✅ unit test |
| Thread starter | uses the parent channel | `facts` | ✅ |
| Opt-in channels (onboarding) | `isOptInEnabled && !isChannelRecordOrParentOptedIn` | — | ❌ not wired in `NotifyPrefs` |
| Message requests, game overlay focus, activities | `isMessageRequest`, `getFocusedPID`, embedded activity | — | ❌ Phoenix has none of these |
| Rate limit | one tracked notification per channel on Linux (older one closed), message sound throttled 1 s | `dbus::Live` replaces the channel's notification (`replaces_id`); sounds: `sounds::throttled` | ✅ unit test |
| Clearing | `WINDOW_FOCUS` (selected channel), `CHANNEL_SELECT`, `MESSAGE_ACK` close the channel's notifications | `sync_focus` (render), `open_channel_chat`, gateway `MESSAGE_ACK` (`on_ack`; ignored for the selected channel while the window is in the background, where the client would not have acked) | ✅ |
| Friend requests | module 441711: `RELATIONSHIP_ADD` incoming, not ignored → title `username`, body "Demande d'ami envoyée.", tag user id, click opens the profile and Friends › Pending; `FRIEND_REQUEST_ACCEPTED` (friend that was outgoing) → "Demande d'ami acceptée.", click opens the DM | `on_relationship_add`, `relationship_toast` | ✅ unit test |
| Incoming calls | no `showNotification` for calls in the client (ringtone + in-app call only) | ringtone in `sounds::watch` | ✅ (nothing to show) |
| "Reply" action | the strings exist (`TBA5Xg`, `MJ+sFa`) but no desktop caller passes `actions` | none | ✅ (not offered, like the client) |
| "Show content" switch | no desktop setting in the 2026 client (`notifications` proto has none; the Notifications page has no such row) | none | ✅ (nothing to hide) |
| Activity started, stage, events, reminders, threads created | `ACTIVITY_START`, `STAGE_INSTANCE_UPDATE`, `GUILD_SCHEDULED_EVENT_UPDATE`, `MESSAGE_REMINDER_DUE`, `THREAD_CREATE` | — | ❌ not yet |

## Taskbar badge (wt/setfx, checked)

Client (`web.b63d846b329d6470.js`, app root `Cd`): `count = getTotalMentionCount() + pendingIncoming`
(`RelationshipStore.getPendingCount()` without spam / ignored, plus game friend requests);
`0 && hasAnyUnread && !disableUnreadBadge` → `-1` (dot); `setBadge` on Linux is
`app.setBadgeCount(max(count, 0))`, so the dot never shows there and the "badge" switch does not
change the count. Phoenix `tray::TrayState`: `count = mentions + pending` sent as
`com.canonical.Unity.LauncherEntry` `count` / `count-visible` (what Electron's `setBadgeCount` does
on Linux), the switch only drives the unread tray icon. ✅ consistent; pending requests count every
incoming request (Phoenix has no spam / ignored split, and no game friends) ❌ minor.

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

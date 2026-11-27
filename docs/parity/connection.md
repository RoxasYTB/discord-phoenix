# Connection: gateway stability and REST hygiene

Sources: the web bundle (`web.*.js`, read 26/11 from `~/discord-phoenix-ref/jsref/web.js`): `GatewaySocket`
(class `tf`, module with `resumeUrl`, `_handleClose`, `_doResumeOrIdentify`, `expeditedHeartbeat`),
the `Backoff` class (module 158390), the `GatewayConnectionStore` (`isTryingToConnect`, `isConnected`),
the `SessionHeartbeatScheduler` (`K()` QoS payload) and the connecting screen (`CF` / `CB`).
The Phoenix log `~/.cache/discord-phoenix/logs/phoenix.log` (25–26/11) gave the field symptoms:
51 lines `429, retrying` (45 on `GET /users/@me/notes/:id`, 6 on `PATCH /users/@me/settings-proto/1`),
two `Connection reset without closing handshake`, and a resume about every hour (op 7).

Legend: ✅ implemented and tested, ⚠ partial, ❌ missing.

## Gateway (`phoenix-api/src/gateway/mod.rs`)

| item | client | Phoenix | status |
|---|---|---|---|
| heartbeat opcode | op 40 `QOS_HEARTBEAT`, `d: { seq, qos }`, seq starts at 0 | same (`heartbeat_payload`) | ✅ `heartbeat_is_the_qos_opcode`, mock `becoming_active_sends_a_qos_heartbeat_at_once` |
| QoS payload | `{ active, ver: 31, reasons }`, reasons `foregrounded` (window focused on an app route) and `rtc_connected` (in voice); a beat reports the union of activity seen since the previous beat (`currentPayload` / `upcomingState`); going from inactive to active sends a beat at once | `QosState`, fed by `Backend::set_activity` from the render loop (window active, voice connected) | ✅ `qos_reports_activity_seen_since_the_last_beat` |
| first beat | `random() × interval` after HELLO, then every interval | same | ✅ |
| missing ACK | next tick without ACK → close 4000, reconnect through the backoff (not immediately) | same | ✅ mock main test |
| server op 1 | send a beat and restart the interval | same | ✅ |
| late timer | after every message, beat again if the last beat is older than interval + 5 s | same (`HEARTBEAT_GRACE`) | ✅ |
| expedited heartbeat | power resume and network online: beat now, reconnect if no ACK in 5 s (15 s when offline); when not connected, reset the backoff and connect now, except for "offline" (`networkStateChange(15e3, …, false)`) | Linux suspend is detected by the wall clock jumping ahead of the monotonic clock (checked every 5 s, 10 s drift) → expedited beat of 5 s, or an immediate reconnect while waiting; network changes come from NetworkManager `StateChanged` on the system bus (≥ 50 online, ≤ 30 offline, `discord_backend/network.rs`) → `GatewayHandle::network_changed`: online = 5 s and reconnect now while waiting, offline = 15 s and no reconnect | ✅ `wake_watch_sees_a_wall_clock_jump`, mocks `expedited_heartbeat_without_ack_reconnects`, `network_changes_send_an_expedited_heartbeat`, `network_manager_states_map_to_online_and_offline` |
| HELLO timeout | 30 s from the connect attempt, then drop the resume URL | 30 s covering the TCP/TLS/WebSocket handshake and HELLO, resume URL dropped | ✅ mock `hello_timeout_reconnects` |
| resume or identify | resume only when a session exists and the last ACK is at most 3 minutes old (wall clock), otherwise identify | `should_resume` (`RESUME_WINDOW`), wall clock so a suspended PC identifies | ✅ `resume_only_within_three_minutes_of_the_last_ack`, `stale_session_identifies_instead_of_resuming` |
| resume URL | `resume_gateway_url` from READY; dropped on any socket error (`onError`) and HELLO timeout | same: a `Connection reset without closing handshake` resumes on the main gateway | ✅ mock `reset_without_close_drops_the_resume_url_and_resumes_on_the_main_gateway` |
| op 7 | close 4000, connect again at once, resume | same | ✅ |
| op 9 | on the same socket: `d=true` → resume or identify, `d=false` → identify; no reconnect, no delay | same (the old code closed and waited 1–5 s) | ✅ mock main test + `invalid_session_resumable_resumes_on_the_same_socket` |
| close codes | only 4004 is final (log out); every other code reconnects through the backoff | same; 4007 and 4009 also drop the session so the next connect identifies directly (the client sends a RESUME and gets op 9) | ✅ `only_auth_failure_is_fatal`, mocks `close_4007_starts_a_new_session`, `non_auth_close_codes_reconnect`, `fatal_close_stops` |
| backoff | min 1 s, max 60 s, `current += 2·current·random()`, reset on READY, READY_SUPPLEMENTAL and RESUMED | `Backoff` with the same formula | ✅ `backoff_grows_like_the_client_and_resets` |
| session reset | after a close, when the backoff has failed more than 4 times: drop session and seq (`_reset`, dispatches `CONNECTION_CLOSED`) | same, emits `GatewayEvent::SessionReset` | ✅ mock `five_failed_attempts_reset_the_session_and_identify` |
| zlib-stream | new inflater per socket | `ZlibStream::new()` per connection; the mock uses a fresh encoder per socket, so a reused inflater would fail the tests | ✅ |
| op 37 after a new session | subscriptions flushed again on `CONNECTION_OPEN` | `Resync` re-selects the open channel, which sends op 37 again (`subscribed` is cleared on READY) | ✅ |
| commands while down | sent only when the session is established (dropped otherwise) | queued and flushed after READY/RESUMED | ⚠ different on purpose (no lost voice/subscription command) |

## Connection indicator (`ui/boot.rs`)

The client has no banner for the main gateway. `CB` renders the connecting screen (`CF`) while the socket is
not closed and `CF` hides it while `GatewayConnectionStore.isConnected()` (SESSION_ESTABLISHED or RESUMING).
That store only re-reads on its own actions: CONNECTION_OPEN, CONNECTION_RESUMED and CONNECTION_CLOSED, and
not on CONNECTION_INTERRUPTED. So a short drop (op 7, ACK timeout, one failed attempt) shows nothing; the
screen comes back (fade in .2 s, same tip, "Connection problems?" again after 10 s) only after CONNECTION_CLOSED,
that is the session reset after 5 failed attempts, and it fades out on the next READY or RESUMED.

| item | Phoenix | status |
|---|---|---|
| short drop | nothing changes | ✅ |
| session reset | `GatewayEvent::SessionReset` → `AppEvent::Connection(false)` → `Boot::connection` shows the screen again (fade in from 0, spinner .5 → 1, problems timer restarted) | ✅ `a_reset_session_brings_the_screen_back_until_the_gateway_is_up` |
| back online | READY or RESUMED → `AppEvent::Connection(true)` → normal fade out; never stuck: the screen needs the gateway up again, nothing else | ✅ |
| presence after a new session | READY rebuilds everything through `Resync` (presences included) | ✅ |
| connected sound | the connecting screen (`CF`) plays `discodo` when it hides and the easter egg is on; the screen comes back after a session reset, so the sound plays again on the next READY, as at start-up (`ui/boot.rs`, `DISCODO_KEY`) | ✅ |

## REST (`phoenix-api/src/rest`)

| item | before | now | status |
|---|---|---|---|
| unknown bucket | every concurrent request on a route without known headers went out at once (the voice join sent 20+ `GET /users/@me/notes/:id` and got 45 × 429) | one request per unknown bucket goes out, the others wait 25 ms steps until its `X-RateLimit-*` headers arrive (probe released on error, expires after 15 s); routes that answer without rate-limit headers are not serialised | ✅ `unknown_bucket_lets_one_request_through_until_headers_arrive`, `probes_are_released_on_failure_and_expire`, `routes_without_rate_limit_headers_are_not_serialised` |
| stale headers | a late response could set `remaining` back up and let a burst through | within one window `remaining` only goes down | ✅ `stale_responses_do_not_give_back_spent_requests` |
| user notes | `GET /users/@me/notes/:id` on every profile load (popout, call tiles, own profile) | cached per user with one request in flight (`tokio::sync::OnceCell`), updated by our PUT and by `USER_NOTE_UPDATE`; the client's UserNotesStore keeps notes the same way | ✅ |
| settings-proto writes | each write does `GET /users/@me/settings-proto/1` then `PATCH` (the client only PATCHes) | unchanged: a cached copy could send a stale full blob over the user's settings, so it needs the partial-proto writes first | ❌ |
| start-up `GET /users/@me` | Phoenix checks the token with it before the gateway; the client reads the user from READY | unchanged (token check for the cached-session start) | ⚠ |

No live CDP capture of the client's network was possible on 26/11 (CDP port closed), so the client side above
comes from the bundle only.

## Live check against the client (27/11)

Sources: CDP `Network.webSocketFrameSent` on the main page of the running client (stable 1.0.171, build
681625, native 99581), the client's `X-Super-Properties` header (decoded, ids removed), the live bundle
`web.1732a939a32a5e17.js` fetched from the CDN (`_doIdentify`, `handleIdentify`, `presenceUpdate`,
`GuildSubscriptionsStore`, `SelfPresenceStore`), `discord_desktop_core/core.asar` (crash reporter metadata)
and `~/.config/discord/installer.db`. Observation only: no reconnect was forced, no server or channel was
opened, so the identify and op 37 rows come from the live bundle.

The client sends ETF (`encoding=etf`, zstd-stream); Phoenix sends JSON over zlib-stream. The payloads below
are compared after decoding.

| item | client (live) | Phoenix | status |
|---|---|---|---|
| op 40 frames | `{op:40, d:{seq, qos:{active, ver:31, reasons}}}`, `reasons: ["foregrounded"]` when focused, `active:false, reasons:[]` otherwise; no op 1 at all | same | ✅ |
| op 40 interval | 41.25 s (four beats in a row: 41 252, 41 250, 41 251 ms); an extra beat as soon as the window gets focused, the interval timer keeps running | same | ✅ |
| identify keys | `token, capabilities, properties, presence, compress:false, client_state:{guild_versions:{}}, qos_token` | same keys | ✅ |
| `qos_token` | base64 of `QosToken { client_provided: { is_active }, derived }`, `derived` = READY `qos_token` (base64 `DerivedQosData`), kept per user in the persisted `DerivedQosDataStore` (a READY without `qos_token` deletes the entry) | same encoding; the derived data is stored per user in `derived_qos.json` (config dir, mode 600) and loaded for the user of the token (`discord_backend/qos_store.rs`) | ✅ `qos_token_wraps_the_derived_data_from_ready`, `ready_without_a_token_drops_the_stored_one`, `the_user_id_comes_from_the_first_token_segment` |
| identify `properties` | super properties + `client_app_state`, `is_fast_connect:false`, `gateway_connect_reasons` (module 469177 `L7()`: `COLD_START` until the first mount, then the mounted reasons, `AppSkeleton` while the app shell is mounted, sorted and joined with `,`, `NO_REASONS` when empty), `installation_id` when tracked | same reasons: `PhoenixApp::render` mounts the "app skeleton" while the main screen is shown (`props::set_app_skeleton`), so the start-up identify says `COLD_START` and later ones `AppSkeleton`; no `installation_id` | ✅ reasons (`connect_reasons_follow_the_app_skeleton`, `identify_shape`); ⚠ `installation_id` |
| super properties | key order `os, browser, release_channel, client_version, os_version, os_version_is_release, os_arch, app_arch, system_locale, has_client_mods, client_launch_id, browser_user_agent, browser_version, window_manager, distro, runtime_environment, display_server, client_build_number, native_build_number, client_event_source, launch_signature, client_heartbeat_session_id, client_app_state` | same order and values (`os_version` = kernel release, `system_locale` from the OS locale, `window_manager` = `$XDG_CURRENT_DESKTOP,$GDMSESSION`, `distro` = os-release `PRETTY_NAME`, `display_server` = `$XDG_SESSION_TYPE`, `native_build_number` = `metadata_version` from `installer.db`, `client_app_state` follows window focus in REST too); was a fixed `6.12.0` / `en-US` / `Debian GNU/Linux` / no native build | ✅ except `launch_signature` ❌ (made by the client's libdiscore WASM `generateLaunchSignature`, not reproducible) |
| capabilities | `1767421`, a literal in `_doIdentify` (bits 0, 2–10, 12–15, 17, 19, 20; bits 17, 19 and 20 have no public name) | same value (`capabilities::DEFAULT`); see "Capabilities 1767421" below for what changes | ✅ `default_capabilities_are_the_client_ones`, `ready_with_the_client_capabilities_parses`, `passive_update_v2_expands_like_the_client_dispatcher`, `passive_update_v2_moves_unreads_voice_and_members` |
| identify presence | `SelfPresenceStore.getLocalPresence()`: `status:"unknown", since:0, activities:[], afk:false` before the first READY, then the current presence on every later identify | the first identify sends `unknown`, later ones send the last op 3 | ✅ `identify_carries_the_last_presence_sent` |
| op 3 shape | `{status, since, activities, afk}` (no `broadcast`) | same | ✅ |
| op 3 timing | after READY only if the presence differs from the one sent (so once after the first READY), on RESUMED always (`forceUpdate`), on every change of status, custom status, idle or AFK; identical payloads are dropped; at most 5 per 20 s, the next one is delayed to the end of the window | same (`self_presence::Sent::gate`) | ✅ `op3_is_deduplicated_and_limited_to_five_per_twenty_seconds` |
| op 14 | not sent by the 2026 client (no `LAZY_REQUEST` in the bundle) | not sent by the app | ✅ |
| op 37 on server open | `GuildSubscriptionsStore`: `{typing, activities, threads}: true` once per guild, then deltas only; `channels` holds the 5 most recent channels of the guild (LRU) with their ranges, `[[0,99]]` on select without replacing ranges set by the member list; `thread_member_lists` the same way; pending deltas are merged and flushed in one op 37 | same (`discord_backend/subscriptions.rs`, 10 ms flush); a new session resends the selected guild in full | ✅ `features_are_sent_once_per_guild`, `channels_keep_the_five_most_recent_and_send_deltas`, `a_new_session_resends_only_the_selected_guild` |
| op 37 member list gate | on `CHANNEL_SELECT` the guild features are always subscribed, the channel only when the member list is shown (`getSection() === MEMBERS`: members open, no search, no side chat); showing the list later subscribes it (`syncWith([ChannelSectionStore], B)`) | same: the chat view reports the members section (`Backend::set_members_shown`), `select_channel` only subscribes the channel when it is shown, and showing it subscribes the open channel with `[[0,99]]` | ✅ |
| op 37 on idle | `IDLE {idle:true}` → `w(false, true)`: every subscribed guild except the selected one, the voice one and the one of the selected channel is cleared and sent as `{typing:false, threads, activities:false, members:[], member_updates:false, channels:{}, thread_member_lists:[]}` (`threads` stays as it was; a guild without threads is forgotten) | same, on the 10 min idle of the idle watcher (`self_presence::watch_idle` → `idle_unsubscribe`); a later select sends only `typing` and `activities` again | ✅ `going_idle_clears_every_guild_but_the_kept_ones` |
| op 37 thread member lists | LRU of 3 threads per guild (`new LRU({max:3})`); an already subscribed thread only refreshes its rank | was 5 and always resent; now 3 and no resend | ✅ `thread_member_lists_keep_the_three_most_recent` |
| op 37 full state | after a new session, `k()` sends `U.get(guild)`: all seven keys, empty `members` and `thread_member_lists` included | same | ✅ `a_new_session_resends_only_the_selected_guild` |

## Capabilities 1767421 (27/11)

Sources: `_doIdentify` in the live bundle (`capabilities:1767421`), the PASSIVE_UPDATE_V2 dispatcher
(`e$(["PASSIVE_UPDATE_V2"], …)`) and its stores (ReadStateStore, VoiceStateStore, GuildMemberStore), the
public capability list (docs.discord.food, bits 0–16), and two short gateway sessions opened with the
account's token, one with `8639` and one with `1767421`. Only the structure of the payloads (keys and value
types, ids replaced by `<id>`) and counts were kept, in memory and in the private scratchpad; no value, no
message content and no raw frame was written anywhere. The live client's own socket (ETF + zstd-stream,
started long before) cannot be decoded mid-stream, so it was not used.

What changes with the client's set, measured on the same account (44 guilds, 2205 channels):

| change | effect | Phoenix |
|---|---|---|
| guild fields | all guild fields move into `properties`; `data_mode:"full"`, `channel_settings`, `application_command_counts`, `experiments` added; `premium_subscription_count` stays at the top; `properties.max_members` is `null` | `RawGuild` already reads `properties` first and falls back to the top level ✅ |
| `version` | gone from channels, roles, emojis and stickers | unused ✅ |
| guild `soundboard_sounds` | gone from READY guilds | Phoenix asks for them with op 31 ✅ |
| `user_settings` (bit 9) | gone from READY, no `USER_SETTINGS_UPDATE`; `user_settings_proto` and `USER_SETTINGS_PROTO_UPDATE` only | every reader already prefers the proto ✅ |
| bit 1 dropped | 260 users in READY instead of 240, more `PRESENCE_UPDATE` for implicit relationships | parsed like the others ✅ |
| `guild_experiments` (bit 6) | populations become arrays | not parsed ✅ |
| channels (bit 15) | same 2205 channels, same names and keys (minus `version`) on this account | ✅ |
| PASSIVE_UPDATE_V2 (bit 14) | for guilds without an op 37 subscription, `{guild_id, updated_members, updated_channels:[{id, last_message_id, last_pin_timestamp}], updated_voice_states, removed_voice_states:[user ids]}` instead of separate events | `expand()` turns it into the events the client's stores apply: member updates, `ChannelUnreads` (new last message and pin, so the unread dots and pin badges move), voice state updates (removals as `channel_id: null`), with the guild id set on each; the session journal replays it the same way ✅ |
| CALL_CREATE on connect (bit 12) | existing DM calls arrive after READY | handled like a live CALL_CREATE ✅ |

No PASSIVE_UPDATE_V2 was seen during the 25 min probe, so its fields come from the dispatcher only
(the unit tests use synthetic payloads).

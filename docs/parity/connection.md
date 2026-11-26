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
| expedited heartbeat | power resume and network online: beat now, reconnect if no ACK in 5 s (15 s when offline); when not connected, reset the backoff and connect now | Linux suspend is detected by the wall clock jumping ahead of the monotonic clock (checked every 5 s, 10 s drift) → expedited beat of 5 s, or an immediate reconnect while waiting; `GatewayHandle::expedited_heartbeat` for other triggers | ✅ `wake_watch_sees_a_wall_clock_jump`, mock `expedited_heartbeat_without_ack_reconnects`; ⚠ no network online/offline hook yet |
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
| connected sound | `discodo` when the connected sound is on | ❌ (same gap as start-up) |

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

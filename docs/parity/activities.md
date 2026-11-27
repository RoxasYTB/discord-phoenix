# Activities (Embedded App SDK) and voice lobby variants

Sources: chunk `c76f31703f6181cd.js` (module 175203, CallTile), `9c90d6f3aabcf794.js` (804980, ActivityTile), `9cf4c852c9727cd1.js` (484191 ChannelCallEmpty, 526369 participant grid, 874739 call content), `d6caa202792cce11.js` (304047 tile game badge, 345812 width rules), `web.b63d846b329d6470.js` (597798 `selectVoiceChannel`, 147036 `Pd` channel full, 890615 connect check, 550151 activity joinability, 913659 application assets, 283488 `embedded_background`), `bb98d1ee1758f772.js` (361698 channel full alert, fetched by GET from the CDN). CSS `120933.986663c05fca8ee0.css` (`*_oEq_gK`), `655282.bf86a1d84104cab5.css` (`*_7NOr2f`), `587882.16f444432b7c5123.css` (`*_m-EqeQ`, UserSummaryItem). Module-to-chunk lookups were read from `webpackChunkdiscord_app` over CDP (read only, no view opened).

## Voice lobby (not connected)

| Item | Client | Phoenix |
|---|---|---|
| no CONNECT | ChannelCallEmpty gets `hasConnectPermission = can(CONNECT, channel)`; the only change is the `overlay-primary` button: text « Salon verrouillé » (`TVBCKZ`) and `disabled` (`button_F7qvhY:disabled` opacity .5, pointer-events none). No other icon in the lobby: the lock is the header channel icon (`voice-lock`, already drawn from the channel's locked state) | `call_view::render_lobby`: opacity .5, no hover, no click, `call.lobby_locked` ✅ |
| full channel | the lobby is unchanged; clicking « Rejoindre » runs `selectVoiceChannel`, which opens the alert 361698 when `Pd` is true: `userLimit > 0 && voiceStates >= userLimit && !(MOVE_MEMBERS && CONNECT)` (or the guild video limit when someone has video). Alert: title « Ce salon est plein » (`elyVbv`), text « Oups ! Ce salon a atteint sa limite max de membres ! » (`nIeRep`), one primary « OK » (`BddRzS`) | `call_view::channel_full` + `lobby_join`: the same alert as a one-button form (the form now hides « Annuler » when it is a plain OK alert). ➖ the guild video limit (`maxVideoChannelUsers`) is not checked. ⚠ the sidebar click still goes through `enter_voice_channel` (app.rs, owned by the voice-switch work) without this check |
| empty channel | no tiles, subtitle « Personne n'est actuellement dans un salon vocal. » (`zSqdrS`) | already drawn (`call.lobby_empty`) ✅ |
| AFK channel | no lobby variant (ChannelCallEmpty has no AFK branch). In the call, the grid 526369 skips the invite / activity tile when `guild.afkChannelId === channel.id` | `call_view::solo_tile_allowed` ✅ |
| invite tile rules | 526369: only in a guild (`null != P`), not AFK, not Stage, `userLimit <= 0 \|\| userLimit > 1`, one user participant, and `isPrivate() \|\| canInvite` | same; the tile is no longer drawn in DM calls ✅ |

## Activities

### What the client does
- Launch: `POST /activities/{channel_id}/{application_id}` with `{session_id, guild_id}` (`ACTIVITY_CHANNEL_LAUNCH`); join an instance: `POST /activities/applications/{app}/activity-instances/{instance}/join`; leave: `POST /applications/{app}/activities/{location}/instances/{instance}/leave` with `{session_id}`.
- The instance list arrives in `GUILD_CREATE.activity_instances` and `EMBEDDED_ACTIVITY_UPDATE_V2` (`location.channel_id`, `application_id`, `participants[].user_id`). Phoenix already stores them in `guild_live`.
- The app runs in an iframe on `https://{application_id}.discordsays.com` and talks to the client with the Embedded App SDK over `postMessage`: handshake, then RPC commands (`AUTHORIZE`, `AUTHENTICATE`, `GET_CHANNEL`, `GET_ACTIVITY_INSTANCE_CONNECTED_PARTICIPANTS`, `SET_ACTIVITY`, voice state subscriptions, `ACTIVITY_INSTANCE_PARTICIPANTS_UPDATE`, layout mode, orientation, thermal state, etc.).
- `AUTHORIZE` makes the client call `POST /oauth2/authorize` for the application. That adds the app to the account's authorized apps.

### Call grid tile (activity started by someone else), drawn from gateway data ✅
- Participant type `ACTIVITY` (index key `\x01` + sort key) comes before streams and users; an instance the current user is in is filtered out (874739).
- ActivityTile 804980 when not running locally: `embedded_background` asset at 1024 (`GET /oauth2/applications/{id}/assets`, url `app-assets/{app}/{asset}.png?size=1024`), `object-fit: cover`; `splash_oEq_gK` veil `--opacity-black-60`, centred column; UserSummaryItem max 4 (3 + « +N » beyond 4) at 56 / 32 / 24 for widths > 400 / > 300 / else (overlap 14 / 8 / 6); subheader `text-default` 14/18 margin-top 4 (12/16 medium, hidden small): « {username} joue à » (`7Uuia2`) or « {username} et N autres » (`cpe6CK`); header `--white` 32/40 bold margin 4 0 (20/24 medium, 14/18 small) = application name; buttons margin-top 4, `overlay-primary` « Rejoindre l'activité » (`4i2vj+`), `md` when width > 400, else `sm`.
- Phoenix: `ui/call_activity.rs` (`channel_activities`, `tile`), name and icon from `GET /applications/{id}/public`. The join button opens the existing placeholder overlay (`stub`), because Phoenix cannot run the app (see below). ➖ the avatar cut-out mask (`svg-mask-voice-user-summary-item`) is drawn as a plain overlap; the `sm` label size (14/18) is not measured.

### Game badge on user tiles ✅ (unverified against a real tile)
- CallTileOverlay (175203): `em ?? eu`, where `eu` is the `application_id` of the user's first PLAYING activity (672979); hidden when the tile is idle, ≤ 160 px wide (`DO`), or when the game record has no icon. Name hidden at ≤ 220 px (`VE`, padding 6). It sits after the name pill in `overlayIconsContainer` (gap 8, compact 4).
- Look: `experimentOverlayTitle` (`--control-overlay-secondary-background-default`, tile radius, padding 6 8, gap 4, min-height 20), icon 20 px radius 4 with a 1 px `--border-subtle` inner border, name body-sm ellipsis.
- Phoenix: `call_activity::badge`. ⚠ The client uses the game record icon (`getCanonicalGameId` → games API); Phoenix uses the application icon. Not checked against a live tile: no member of the test channel was playing.

### Running an activity in Phoenix: not done, blocked
1. **No web engine in the process.** GPUI draws everything itself and has no web view. wry on Linux needs a GTK window (`webkit2gtk-4.1`): it cannot be embedded in a GPUI window, only opened as a separate top-level window. On this machine only the runtime libraries are installed (`libwebkit2gtk-4.1.so.0`), not the `-dev` packages (no `webkit2gtk-4.1.pc`, no `gtk+-3.0.pc`), so the crate does not build without installing system packages, and `/home` has about 3 GB free for a second large dependency tree.
2. **Authorization writes to the account.** Almost every activity calls `authorize` first, which ends in `POST /oauth2/authorize` for that app. That adds an authorized application to the real account, outside the test channel. It is not covered by the test-channel permission.
3. **Launch/join are writes tied to voice.** Launch and join need a voice session in the channel (`session_id`), so Phoenix must be connected to `〢🔊・phoenix-vocal` (1574900893613039696). The user must not be in voice at that moment, and joining needs another account to start an activity first.

### Plan for a later session
1. Helper process `phoenix-activity` (separate crate, built only when the feature is on): wry + tao window, loads `https://{app}.discordsays.com/?instance_id=…&channel_id=…&guild_id=…&frame_id=…&platform=desktop`, injects a `postMessage` shim, and forwards SDK frames as JSON lines on stdin/stdout.
2. RPC bridge in Phoenix (`voice/activity_rpc.rs`): handshake, `GET_CHANNEL`, participants from `guild_live`, voice states, `AUTHORIZE` (shows the client's consent modal, then `POST /oauth2/authorize`), `AUTHENTICATE`, `SET_ACTIVITY`; refuse every other write.
3. Launch from the call view and the lobby launcher (`POST /activities/{channel}/{app}`), join from the tile button (`…/activity-instances/{id}/join`), leave on hang-up or window close.
4. First test: Phoenix connected to the test voice channel only, user not in voice, an app that does not need `authorize`, and a second account to check that the instance shows up. Never open the official client's activity launcher (it writes `settings-proto/1`).

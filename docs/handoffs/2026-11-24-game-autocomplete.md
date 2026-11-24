# Handoff — autocomplete `@game` (2026-11-24)

## Goal and guardrails

Continue the Phoenix parity goal. Official Discord is available through the screenshot-only CDP
helper `/home/yohan/discord-phoenix-ref/cdp.mjs`; do not type, click, navigate, inspect DOM, or
change settings on the live account. It is read-only and may contain private content. Do not retain
private screenshots. The user authorized commits/backups but not pushes or GitHub Actions. Keep
builds serialized and check disk/CPU before any compile; current `/home` free space at investigation
was 3.3 GB. Preserve the three untracked PNGs under `docs/screenshots/`.

## Current repository checkpoint

- Checkout: `/home/yohan/discord-phoenix`
- Branch: `main`, 12 commits ahead of `origin/main` before this handoff's docs commit.
- HEAD before this handoff: `2faa7d5` (`Add compact avatar visual test harness`).
- Existing untracked images: `finitions-layout.png`, `i18n-fr.png`, `wip-layout.png` (preserve).
- Do not run another Cargo build until disk and process load have been rechecked.

## Public behavior established

Discord's official [August 11, 2026 changelog](https://discord.com/blog/discord-update-august-11-2026-changelog)
says desktop users can type `@game`, search games that have a Discord profile, and embed a game in
the message. It names Palworld and Sunderfolk as examples. The source does **not** document the
suggestion row, search route, exact insertion token/payload, keyboard behavior, or setting-off
behavior.

## Phoenix code findings

- `crates/phoenix-app/src/ui/autocomplete.rs`: autocomplete tokenizer currently recognizes only
  `:emoji`, `@mention`, and `#channel`. `Kind`, `Item`, `suggestions`, `insertion`, and rendering
  have no game variant.
- `crates/phoenix-app/src/ui/settings/synced_map.rs`: `ac-games` maps to settings-proto path
  `text_and_images.include_game_mentions_in_autocomplete` (6,39); the value is stored.
- `crates/phoenix-app/src/discord_backend/profile_games.rs`: Phoenix calls `GET /games/{id}` only
  for IDs already present in profile widget data. This is not a search endpoint.
- `PhoenixApp.presences` and `presence::Activity` contain activity names, but the official changelog
  does not say the autocomplete catalogue is derived from user activity. Do not use this as a
  substitute catalogue.

## Decision

Do not implement speculative `@game` behavior yet. Current docs now mark the toggle's intended
gate and explicitly state what is still unknown. No account settings were touched, no message was
typed or sent, and no build/test ran during this investigation.

## Next safe work

1. While respecting screenshot-only CDP restrictions, visually inspect only already-open screens;
   do not type `@game` into the live composer because typing broadcasts presence.
2. Search official public docs or read-only route metadata for the game's search API and payload.
   If unavailable, make a clearly synthetic test fixture and keep game autocomplete marked blocked
   by missing contract rather than inventing endpoint behavior.
3. Prefer the next parity item with a complete behavior contract and synthetic/mock verification.
4. Before build: `df -h /home /mnt/windata`, inspect Cargo/rustc processes, use one low-priority build
   at a time. Keep `/home` headroom in mind.
5. Before ending a substantial batch: review `git diff --check`, commit docs/code as one scoped
   checkpoint, create a dated bundle/patch backup under `/mnt/windata/project-phoenix/backups/`,
   verify `git bundle verify`, and preserve all untracked user PNGs.

## Additional read-only research (same session)

- Official Discord's public [Developer Newsletter, March 2026](https://discord.com/developer-newsletter/march-2026),
  [game profile announcement](https://discord.com/blog/claim-your-game), and
  [Profile Widgets FAQ](https://support.discord.com/hc/en-us/articles/35344672307607-Profile-Widgets-FAQ)
  establish that Discord has a game database/profile and profile widgets select games from that
  database. They do not document an end-user autocomplete search endpoint or mention payload.
- Official developer documentation search did not surface a public route for `@game` search. This
  supports the decision to leave `ac-games` unwired pending a read-only, evidenced contract; it is
  not proof that Discord has no private client route.
- Soundboard is a separate confirmed functional gap: `crates/phoenix-app/src/ui/call_view.rs`
  renders `call-soundboard` with an empty callback, `crates/phoenix-app/src/shortcuts.rs` routes
  Ctrl+Shift+B to `stub`, and `crates/phoenix-app/src/ui/voice_panel.rs` only renders the button.
  No soundboard API or model is wired in `backend.rs`, `discord_backend.rs`, or `phoenix-api`.
  Further read-only inspection of local reference bundle
  `/home/yohan/discord-phoenix-ref/current-client/js/web.b63d846b329d6470.js` found a concrete
  read path: `GET /soundboard-default-sounds`; guild sounds are requested through Gateway
  `REQUEST_SOUNDBOARD_SOUNDS` with `guild_ids` and returned as `SOUNDBOARD_SOUNDS` updates with
  `guild_id` and `soundboard_sounds[]`. The observed gateway sound fields are `name`, `sound_id`,
  `emoji_name`, `emoji_id`, `user_id`, `volume`, `available`. The route registry also includes
  `/guilds/{id}/soundboard-sounds`, `/guilds/{id}/top-soundboard-sounds`, and sound metadata routes;
  do not assume a method or behavior from their names alone. `REQUEST_SOUNDBOARD_SOUNDS` is a
  read/fetch request; the separate sound-send route is deliberately not called. Implement a mock
  parser/model and visible picker first; keep actual playback behind a later explicit safe test.
  UI geometry still needs an eligible visual reference. The official guide establishes the basic
  flow (open voice controls, choose a server, choose a sound):
  https://support.discord.com/hc/en-us/articles/12612888127767-Discord-Soundboard-Guide-Using-Adding-and-Managing-Sounds.

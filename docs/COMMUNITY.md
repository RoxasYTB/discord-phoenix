# Community server

Progress updates for Phoenix are posted on the project's Discord server, *Discord | Project Phoenix EARLY ACCESS & BETA [UNOFFICIAL]* (guild `1574895284901777448`). The server is in French, so the posts are too.

## Channels

| Channel | Id | Used for |
|---|---|---|
| `📢・annonces` | `1574895965171617982` | One post per milestone: what is new, what comes next, the "not final" warning, up to 4 README screenshots |
| `📸・captures` | `1574895971513532477` | One gallery post per milestone, up to 10 README screenshots with a one-line caption |
| `⚙️・avancés` | `1574898034859581631` | The single tracking message (Done / In progress / Next), edited in place and pinned |
| `📋・changelogs` | `1575141380785836133` | Changelogs: everything merged since the last release, grouped by area, with real-account renders; several messages of at most 10 images when needed. Says plainly that the changes ship with the next beta, without a date |

The other channels (rules, roles, discussion, feedback, bugs, support, team, logs) are never written to.

## Tracking message

- `⚙️・avancés`, message `1574898034859581631/1574898068434714644`: the owner's original "Déjà présent / Manque ou partiel" list, now kept as Fait / En cours / À venir. Edit it, never post a second one.

## Rules

- At most one post per channel per milestone. No `@everyone`, no `@here`, no mentions, no replies to members on the owner's behalf, no moderation and no server settings.
- Images come from the screenshots already reviewed for the README (`docs/screenshots/<date>/`), or, for changelogs, from recent real-account renders copied into a local `reviewed-screenshots/` folder (never committed) after a privacy pass with the README rules: no email or phone, no account page, no DMs, no server other than Aperture Sciences, no third-party profile or stream, third-party names or awkward text blurred.
- Releases are made by the owner only. A changelog never promises a date.
- Every post says that the builds and screenshots are not final versions.

## Posting

Posts go through `crates/phoenix-app/examples/community_post.rs`, which uses the real Phoenix REST client with the token from the keyring. It refuses anything but this guild and the four channels above (checked by id and by name), any text over 2000 characters or containing `@everyone`, `@here` or a mention, any image outside `docs/screenshots/` or a local `reviewed-screenshots/` folder, and edits or pins of messages the account did not write. Mentions are disabled with `allowed_mentions`.

```sh
cargo build -p phoenix-app --example community_post
EX=target/debug/examples/community_post
$EX check annonces annonces.txt docs/screenshots/2026-11-25/chat-general.png
$EX send annonces annonces.txt docs/screenshots/2026-11-25/chat-general.png
$EX edit avances 1574898068434714644 avances.txt
$EX pin avances 1574898068434714644
```

`check` only runs the GET guards and validates the text and images. To update the tracking message after a milestone, rewrite the three lists from the README status and the `docs/parity/` tables, then `edit` it.

## Automatic changelog posts

Dashboard changelog entries (`phoenix-log`) that are marked `--public` are posted to `📋・changelogs` by a user service, `phoenix-changelog-post` (`Restart=always`), which runs a small script next to the `community_post` binary, outside the repository.

- It reads the dashboard changelog every 30 seconds and posts each new public entry as one message: the title in bold, then the text, with at most 10 PNG screenshots.
- It goes through `community_post send changelogs`, so the same guard applies: this guild and channel only, no mentions, `allowed_mentions` empty, at most 2000 characters, images only from a local `reviewed-screenshots/` folder.
- Before posting, it strips internal wording: `wt/` branch names, file paths, `real_shots`, `PHX_*` switches, "agent", "MERGED", em dashes. The entry's author still writes the text for users.
- It skips the automatic "Intégration …" entries, waits a random 5 to 13 seconds between two posts, and keeps the ids it has posted, so an entry is never posted twice. On first start, every existing entry is marked as already seen.
- A post that fails three times is given up and logged.

```sh
systemctl --user status phoenix-changelog-post
systemctl --user restart phoenix-changelog-post
```

Only mark an entry `--public` after checking each screenshot against the README rules (no DMs, no account page, no email or phone, no server other than Aperture Sciences, no third-party profile or stream, third-party names blurred on a copy). A posted entry is never deleted.

## History

| Date | Channel | Message |
|---|---|---|
| 2026-11-25 | `📢・annonces` | `1575061609292890135` (4 screenshots) |
| 2026-11-25 | `📸・captures` | `1575061633758134332` (10 screenshots) |
| 2026-11-25 | `⚙️・avancés` | `1574898068434714644` (edited and pinned) |
| 2026-11-25 | `📋・changelogs` | catch-up since v0.1.0-beta.1: intro `1575142678163628053`, then 18 entries up to `1575143479523414159` |

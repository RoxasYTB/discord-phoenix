# Handoff — Soundboard mentions in chat

## Scope

Implement only the static rendering of Discord's inline Soundboard token in message content. Do not trigger playback, join voice, send messages, or change account/server settings. Work against synthetic fixtures until a safe, visible live example is available.

## Evidence and implementation

- The official local client bundle parser recognizes `<sound:guildId:soundId>` as a Soundboard AST node.
- Its message renderer prefers the message's `soundboard_sounds` metadata; the renderer can fall back to the global Soundboard store. A known sound displays optional emoji and its name. The click path needs channel/voice context and starts playback. An unresolved mention has a separate “Unknown” presentation in one renderer.
- API message model now decodes message-local sound metadata (`sound_id`, `guild_id`, `name`, `emoji_id`, `emoji_name`), and backend conversion carries it into the app model; extra sound fields are ignored by Serde's default behavior.
- Markdown resolves known mentions to their name and optional textual emoji. Unknown tokens remain literal. Both regular message rendering and background history-page rendering are wired. Custom emoji image resolution is not implemented in this slice.
- `mock_showcase.rs` has one synthetic “Synth chime” fixture.
- Styling currently reuses the mention foreground/background and medium weight as a provisional chip. No exact padding, radius, hover, focus, or click styling has been observed. No playback action is connected.

## Next steps

1. Inspect the code diff and compile/test after checking disk, memory, and CPU. Keep one Cargo build at a time, at most four jobs, low priority, shared `CARGO_TARGET_DIR` and existing `LIBRARY_PATH`; `/home` was at 97% (3.3 GB free) at last check, so do not build until capacity is improved or verified safe.
2. Add focused API deserialization and backend-conversion coverage, including missing/null metadata and guild-scoped lookup.
3. Render the synthetic showcase and compare a screenshot to the official client only if the reference can show a synthetic/safe mention without exposing private account content. Use screenshot-only CDP; do not dump DOM/ARIA/text or persist account screenshots.
4. Measure width changes and hover/focus styling on a safe fixture. Keep this row ⚠ until measured. Do not connect playback as part of this static-rendering change.
5. Update README/changelog only with inspected synthetic screenshots. Add the required warning: **<u>CES CAPTURES NE SONT PAS LES VERSIONS DÉFINITIVES.</u>**
6. Make a dated backup before the next substantial edit and commit this slice only after validation. Do not push without a request.

## Current validation status

No compilation or tests have been run for this work yet. `git diff --check` is clean. The earlier broad `rustfmt --check` showed formatting differences in existing files and did not modify them; avoid whole-file rustfmt churn. The untracked screenshots already in `docs/screenshots/` belong to earlier progress and must be preserved.

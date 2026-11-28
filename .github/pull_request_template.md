## Summary

<!-- What does this change and why? Link issues with "Fixes #123". -->

## Screenshots / GIFs (before → after)

<!--
Required for any visible change. Same view, same window size and scale, demo data only
(PHOENIX_MOCK=1 or the `screenshot` example), never your own account.
Anything that moves (hover, animation, menu, scrolling, playback) needs a before and an after GIF too.
See CONTRIBUTING.md, "Screenshots and GIFs". If nothing visible changes, write "No visible change".
-->

| Before | After |
| --- | --- |
|  |  |

GIFs:

## Testing

<!-- How did you test it? OS / GPU if relevant. -->

## Checklist

- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes, and unrelated code is not reformatted
- [ ] `cargo test --workspace` passes
- [ ] Parity values cite their source (CSS rule, dump or live measurement); no Discord binary assets committed, since they load from the local install (see CONTRIBUTING.md)
- [ ] No tokens, credentials or personal data in code, tests, fixtures or logs
- [ ] I ran the app and tested the change myself; UI changes include before/after screenshots (same view and size, demo data) compared with the official client, and before/after GIFs for anything that moves
- [ ] I understand every line of this PR and take responsibility for it (AI-assisted or not), and I did the quality check in CONTRIBUTING.md
- [ ] No version bump, release tag or release workflow change (releases are made by the maintainer)

## Summary

<!-- What does this change and why? Link issues with "Fixes #123". -->

## Testing

<!-- How did you test it? OS / GPU if relevant. -->

## Checklist

- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes, and unrelated code is not reformatted
- [ ] `cargo test --workspace` passes
- [ ] Parity values cite their source (CSS rule, dump or live measurement); no Discord binary assets committed, since they load from the local install (see CONTRIBUTING.md)
- [ ] No tokens, credentials or personal data in code, tests, fixtures or logs
- [ ] I ran the app and tested the change myself; UI changes include before/after screenshots compared with the official client
- [ ] I understand every line of this PR and take responsibility for it (AI-assisted or not), and I did the quality check in CONTRIBUTING.md

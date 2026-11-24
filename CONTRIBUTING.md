# Contributing to Discord Phoenix

Thanks for helping. Discord Phoenix is an **unofficial** client. It is not affiliated with, endorsed by, or supported by Discord Inc. "Discord" is a trademark of Discord Inc. and is used here only to describe what the client connects to.

## Reference material: 1:1 parity

Phoenix aims to be visually and functionally identical to the official 2026 client. To get there, contributors may and should work from the real client:

- its CSS (values, variables, class rules), computed styles and DOM, read from your own running client (for example through the Chrome DevTools Protocol);
- the behaviour of its JavaScript: settings protobuf fields, request flows, timings, animation curves;
- its icons, fonts, sounds, images and translation strings, used as the reference for what Phoenix shows;
- network traffic from your own account, and the [official developer docs](https://discord.com/developers/docs).

How we use Discord's files in practice:

- **Measured values go into the code**: sizes, colours, radii and timings, with a comment citing the CSS rule or dump they come from (see `docs/parity/`).
- **Binary assets are loaded at runtime from the user's own Discord install or cache**: icons, fonts and sounds. They are not committed to this repository; see `crates/phoenix-app/src/icons` and `src/fonts`. Hand-drawn fallbacks cover users without a local Discord.
- **Dumps and captures from a real account stay private**: never commit names, messages, IDs, signed URLs or tokens. Public screenshots use demo data.
- Never contribute anything obtained under an NDA or by breaching an agreement.

We also don't accept self-bot features, spam/automation tools, or ways to bypass rate limits, Nitro restrictions or moderation.

## Development

```sh
cargo build
cargo run -p phoenix-app
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Linux needs: `libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev libxcb1-dev libfontconfig-dev libssl-dev libasound2-dev pkg-config`.

Workspace layout:

- `crates/phoenix-api`: REST + gateway client
- `crates/phoenix-voice`: voice/RTC
- `crates/phoenix-app`: GPUI desktop app

## Pull requests

- Keep PRs focused; one topic each.
- CI (fmt, clippy with `-D warnings`, tests on Linux/Windows/macOS) must pass.
- Tests and fixtures must not contain real tokens, user IDs or message content.
- By contributing, you agree your work is dual-licensed under MIT OR Apache-2.0, like the project.

## Releases

Pushing a tag `vX.Y.Z` (or `vX.Y.Z-beta.N`) triggers `.github/workflows/release.yml`. Tags containing `beta`, `alpha` or `rc` are published as pre-releases.

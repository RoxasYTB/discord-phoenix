# Contributing to Discord Phoenix

Thanks for helping. Discord Phoenix is an **unofficial** client. It is not affiliated with, endorsed by, or supported by Discord Inc. "Discord" is a trademark of Discord Inc. and is used here only to describe what the client connects to.

## Reporting bugs

Use the issue forms and follow [docs/REPORTING_BUGS.md](docs/REPORTING_BUGS.md) ([français](docs/SIGNALER_UN_BUG.md)). It explains how to copy the version and debug info from Settings, where the logs are, how to make a side-by-side comparison with the official client, and what never to post (tokens, `journal.log`, HAR files, cookies). Maintainers triage with the labels and cycle described there.

## Reference material: 1:1 parity

Phoenix aims to be visually and functionally identical to the official 2026 client. To get there, contributors may and should work from the real client:

- its CSS (values, variables, class rules), computed styles and DOM, read from your own running client (for example through the Chrome DevTools Protocol);
- the behaviour of its JavaScript: settings protobuf fields, request flows, timings, animation curves;
- its icons, fonts, sounds, images and translation strings, used as the reference for what Phoenix shows;
- network traffic from your own account, and the [official developer docs](https://discord.com/developers/docs).

How we use Discord's files in practice:

- **Measured values go into the code**: sizes, colours, radii and timings, with the source (CSS rule, dump or measurement) recorded in `docs/parity/`.
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

## AI-assisted contributions and quality control

Contributions written with the help of AI tools are welcome. The same rules apply to them as to everything else:

- **Every contribution is reviewed again by a maintainer** before it is merged, whoever or whatever wrote it. Expect questions and change requests.
- **You are responsible for what you publish.** You must understand every line you submit and be able to explain it. "The AI wrote it" is not an answer to a review comment.
- **Do your own quality check before opening a PR**:
  - it builds, and `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass;
  - you ran the app and tried the change yourself, including the edge cases (empty states, long names, narrow window, other themes);
  - for UI changes, you compared the result with the official client at the same window size and zoom, and attached before/after screenshots, plus before/after GIFs for anything that moves (see [Screenshots and GIFs](#screenshots-and-gifs));
  - measured values have their source recorded in `docs/parity/`;
  - the diff only touches what the change needs: no unrelated reformatting, no dead code, no code comments (only `// SAFETY:` is allowed);
  - no tokens, personal data or private screenshots anywhere in the diff;
  - for AI-generated code, you checked that APIs, endpoints and fields really exist and behave as claimed, and did not just trust the tool's output.

PRs that skip these checks will be sent back.

## Pull requests

- Keep PRs focused; one topic each.
- Visible changes need before/after screenshots, and before/after GIFs when something moves, made with demo data (see [Screenshots and GIFs](#screenshots-and-gifs)).
- CI (fmt, clippy with `-D warnings`, tests on Linux/Windows/macOS) must pass.
- Tests and fixtures must not contain real tokens, user IDs or message content.
- No code comments (`//`, `///`, `//!`, `/* */`, `#` in scripts), except `// SAFETY:` above an `unsafe` block. Explanations go in the commit message or `docs/`; parity sources go in `docs/parity/`; example usage goes in `docs/examples.md`.
- By contributing, you agree your work is dual-licensed under MIT OR Apache-2.0, like the project.

## Screenshots and GIFs

Every PR or issue about something visible shows it. This is required, not a nice extra.

- **Before and after screenshots** of the same view, at the same window size and scale (1280x800 at scale 1.5 unless the bug needs another size). Put them side by side or one under the other, "before" first.
- **Before and after GIFs** for anything that moves: hover and pressed states with a transition, animations, menus and popouts that open or close, scrolling, video and GIF playback, loading. Static shots do not show timing.
- **GIF quality**: full palette computed over every frame, no lossy optimiser, the real frame rate, no resizing. The command below does that. Keep each file under 25 MB by recording a shorter clip, not by scaling it down.
- **Demo data only**. GitHub is public. Render with the mock backend, never with your account: no DMs, no account or profile pages, no email, phone number, real server or real user. If a view cannot be reached on demo data, blur everything private and say so.
- A PR that changes nothing visible says "no visible change" in that section instead.

### Making them

Use the mock backend to try the change by hand:

```sh
PHOENIX_MOCK=1 PHOENIX_MOCK_AUTOLOGIN=1 cargo run -p phoenix-app
```

For the screenshots, render the same view offscreen on `main` and on your branch with the `screenshot` example (demo data, no window on screen):

```sh
PHX_W=1280 PHX_H=800 PHX_SCALE=1.5 cargo run -p phoenix-app --example screenshot -- before.png
PHX_W=1280 PHX_H=800 PHX_SCALE=1.5 cargo run -p phoenix-app --example screenshot -- after.png
```

`PHX_DM=<id>` opens a demo DM, and `PHX_SCRIPT` drives the window with real input events (clicks, hovers, keys, wheel) before the capture; the script language is in [docs/qa.md](docs/qa.md#script-commands). Other views have their own examples in [docs/examples.md](docs/examples.md).

For a GIF, write a script that sets up the state and records it. `record <name> <frames>@<fps>` saves numbered PNGs while the app keeps running, plus a `.ffconcat` file with the measured time of each frame:

```
move 640,400
record hover 45@30
```

```sh
PHX_W=1280 PHX_H=800 PHX_SCALE=1.5 PHX_SCRIPT=hover.qa \
  cargo run -p phoenix-app --example screenshot -- after.png
```

Then turn the frames into a GIF:

```sh
python3 scripts/media/gif.py after-hover.ffconcat after-hover.gif
```

It runs ffmpeg with one palette for the whole clip (`palettegen=stats_mode=full`, `paletteuse` with `sierra2_4a` dithering, no scaling), then writes the measured delay of each frame into the GIF, so it plays at the real rate. ffmpeg's concat demuxer drops those per-frame durations for PNG input, which is why a plain `ffmpeg -f concat` is not enough. Frames closer than 20 ms are merged, since browsers slow down shorter GIF delays. For a screen recording, the same filter works straight on the video:

```sh
ffmpeg -i clip.mp4 -filter_complex "split[a][b];[a]palettegen=stats_mode=full[p];[b][p]paletteuse=dither=sierra2_4a:diff_mode=rectangle" -loop 0 clip.gif
```

`--dither none` gives steadier frames on flat UI, the default `sierra2_4a` hides banding on gradients and images. Do the same on `main` for the "before" GIF, with the same script.

Drop the files into the PR or issue description. Maintainers keep theirs on the `pr-media` branch so they never land on `main`.

## Releases

Releases are made by the project maintainer only. Pull requests must not bump the version, edit the changelog for a release, create or push tags, or touch the release workflow; the maintainer does that when cutting a release.

For the maintainer: pushing a tag `vX.Y.Z` (or `vX.Y.Z-beta.N`) triggers `.github/workflows/release.yml`. Tags containing `beta`, `alpha` or `rc` are published as pre-releases.

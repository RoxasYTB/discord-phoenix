# Reporting bugs

*Version française : [SIGNALER_UN_BUG.md](SIGNALER_UN_BUG.md).*

Discord Phoenix is in beta, so bug reports are the most useful thing you can send. A good report lets a maintainer see the bug on their own machine within a few minutes. This page explains what to collect and how, and what you should **never** post.

Open a report from [New issue](https://github.com/RoxasYTB/project-phoenix/issues/new/choose) and pick the form that fits:

| Form | Use it for |
|---|---|
| 🐞 Bug report | Something doesn't work |
| 🎨 Visual bug or difference with the official client | Something looks different from Discord: size, colour, spacing, font, icon, animation |
| 💥 Crash or freeze | Phoenix closes, freezes, won't start or shows a black window |
| 🔊 Voice or video | Audio, microphone, camera, screen share, soundboard |
| ✨ Feature request | Something the official client has that Phoenix doesn't, or a new idea |

Questions and help go to the [project Discord server](https://discord.gg/ffCe2RuBKt) (in French). Security problems, especially anything that could leak a token, go to [private vulnerability reporting](https://github.com/RoxasYTB/project-phoenix/security/advisories/new), never to a public issue.

## 1. Find your version

Open **Settings** (the gear next to your name). The bottom of the left sidebar shows two lines:

```
stable 471091 • 1.0.9215 x64
Discord Phoenix 0.1.0-beta.1 (abcd1234)
```

The first line is the Discord build Phoenix presents itself as. The second is the Phoenix version and the commit it was built from. Copy the second line into the **Phoenix version** field.

Before reporting, check that you have the latest [release](https://github.com/RoxasYTB/project-phoenix/releases): your bug may already be fixed.

## 2. Copy the debug info

Under the version, click **Copy debug info**. The link changes to **Copied!** and your clipboard now holds something like:

```
Discord Phoenix 0.1.0-beta.1 (abcd1234)
OS: Debian GNU/Linux 13 (trixie), kernel 6.12.48 (x86_64)
Session: Wayland (KDE)
GPU: NVIDIA GeForce GTX 1080 | driver: NVIDIA 580.95.05
Renderer: wgpu | hardware acceleration: on
Language: fr
Zoom: 100% | font: 16px | density: default
Theme: dark
Log level: info
```

Paste it as is into the **Debug info** field. It holds no account data: no username, no ID, no server, no token.

If Phoenix doesn't get as far as Settings, fill the OS, GPU and driver fields by hand.

## 3. Get the logs

Phoenix writes its log to a file. Click **Open logs folder** (next to **Copy debug info**) or go there yourself:

| OS | Folder |
|---|---|
| Linux | `~/.cache/discord-phoenix/logs/` (or `$XDG_CACHE_HOME/discord-phoenix/logs/`) |
| Windows | `%LOCALAPPDATA%\discord-phoenix\logs\` |
| macOS | `~/.cache/discord-phoenix/logs/` |

The current file is `phoenix.log`. When it reaches 5 MB it becomes `phoenix.log.1`, then `phoenix.log.2`; older files are deleted. Paste the last 200 lines or so, from just before the bug. For a crash after a restart, the lines you need may be at the end of `phoenix.log.1`.

Phoenix masks tokens, cookies, authorization headers, emails and message text before writing anything. **Read what you paste anyway** and remove whatever you don't want public: server or channel names, file names, IDs.

### More detail: launch from a terminal

For crashes, a black window, or when you are asked for more detail, launch Phoenix from a terminal. You then get the log, the panic message and a backtrace:

**Linux**

```sh
RUST_LOG=debug RUST_BACKTRACE=1 discord-phoenix 2>&1 | tee ~/phoenix-run.txt
```

With the AppImage: `RUST_LOG=debug RUST_BACKTRACE=1 ./discord-phoenix-*.AppImage 2>&1 | tee ~/phoenix-run.txt`.

**Windows** (PowerShell, in the folder that holds `discord-phoenix.exe`)

```powershell
$env:RUST_LOG="debug"; $env:RUST_BACKTRACE="1"; .\discord-phoenix.exe 2>&1 | Tee-Object $HOME\phoenix-run.txt
```

**macOS**

```sh
RUST_LOG=debug RUST_BACKTRACE=1 "/Applications/Discord Phoenix.app/Contents/MacOS/discord-phoenix" 2>&1 | tee ~/phoenix-run.txt
```

`RUST_LOG` sets the level: `info` (default), `debug`, or `trace` for one module only (`RUST_LOG=info,phoenix_voice=trace`). `trace` everywhere produces a lot of output; use it only if asked.

## 4. Screenshots and video

A screenshot is **required** for anything visual, and a short video is the best way to show an animation, flicker or a bug that takes several steps.

| OS | Screenshot | Video |
|---|---|---|
| Linux, KDE | **Spectacle** (`Print Screen`, or `Meta+Shift+S` for a region) | Spectacle, *Record* mode, or OBS Studio |
| Linux, GNOME | `Print Screen` opens the screenshot tool | The same tool, video button (`Ctrl+Alt+Shift+R` on older versions) |
| Windows | `Win+Shift+S` (Snipping Tool) | Snipping Tool, *Record*, or `Win+Alt+R` (Xbox Game Bar) |
| macOS | `Cmd+Shift+4` (region), `Cmd+Shift+4` then `Space` (window) | `Cmd+Shift+5`, *Record selected portion* |

Tips:

- Capture the whole Phoenix window, not just a crop, unless the detail is tiny. Then add a zoomed crop as well.
- Keep the OS scale and Phoenix zoom you used when the bug happened, and say what they are.
- Videos: 10 to 30 seconds, MP4 or WebM, under 10 MB so GitHub accepts them. Hover slowly over what matters.

### Blurring

Before uploading, **blur or cover** every username, avatar, message, email, phone number, and the name and icon of any private server. Server names in your sidebar count too.

- **Spectacle**: *Annotate*, then the blur or pixelate tool. **Windows Snipping Tool**: *Edit*, then draw an opaque shape (the highlighter is not enough). **macOS Preview**: *Tools › Annotate › Rectangle*, filled in black.
- Use opaque boxes or strong pixelation. A light blur on text can sometimes be read back.
- Videos are harder to blur: record a test server or a channel with nothing private in it.

## 5. Compare with the official client

Phoenix aims to look and behave exactly like the official 2026 client, so the most useful report is often "Discord does X, Phoenix does Y".

1. Open the official Discord desktop client and Phoenix with **the same window size**. On Linux, KDE's *More Actions › Configure Special Window Settings* can set an exact size; on Windows and macOS, resize both by hand and check the size on the screenshots.
2. Set **the same zoom**: `Ctrl+0` resets the official client to 100 %; check Phoenix in *Settings › Appearance*.
3. Use the same theme, the same density (*Appearance › Message spacing*) and the same language.
4. Open the same view in both, in the same state (hover, menu open, selected channel).
5. Take one screenshot of each and put them **side by side: Phoenix on the left, Discord on the right**. You can attach two images if you can't combine them.
6. In the form, say whether the official client does the same thing.

If the official client has the same bug, it's a Discord bug, not a Phoenix one, and Phoenix will copy it.

## 6. What you must never post

None of these belong in an issue, a comment, a screenshot or a Discord message, even "just this once":

- **Your Discord token**, or anything from `localStorage`, the keyring or the Credential Manager. Anyone with it controls your account. Nobody from this project will ever ask for it.
- **`journal.log`** (in `~/.cache/discord-phoenix/session/`). It's a local copy of your **messages**, not a debug log. Only `phoenix.log` and its rotated copies under `logs/` are debug logs.
- **HAR files** or DevTools network exports. They contain your token and cookies.
- **Cookies** and request headers (`Authorization`, `Cookie`, `X-Super-Properties`).
- Your email address, phone number, payment details, and other people's messages or profiles.

If you posted any of these by mistake, delete the comment or edit the issue right away, then **change your Discord password**: that invalidates every existing token.

## 7. Example of a good report

> **[Visual] Message hover toolbar is 2 px lower than in the official client**
>
> **Summary**: The toolbar that appears when hovering a message sits 2 px lower and overlaps the next message.
>
> **Area**: Messages, chat
>
> **Steps to reproduce**
> 1. Open any server text channel.
> 2. Hover the second message of a group (not the first one).
> 3. Look at the toolbar on the right.
>
> **Expected**: The toolbar's top edge is 16 px above the message, as in the official client.
> **Actual**: It is 14 px above and covers the top of the next message.
>
> **How often**: Always · **Regression**: Worked in 0.1.0-beta.1 (1a2b3c4d) · **Official client**: works
>
> **Screenshots**: `phoenix-vs-discord.png` (Phoenix on the left, Discord on the right, both 1280×800 at 100 %, usernames blurred)
>
> **Debug info**
> ```
> Discord Phoenix 0.1.0-beta.2 (9f8e7d6c)
> OS: Fedora Linux 43 (KDE Plasma), kernel 6.17.4 (x86_64)
> Session: Wayland (KDE)
> GPU: AMD Radeon RX 6700 XT (RADV NAVI22) | driver: radv Mesa 25.2.4
> Renderer: wgpu | hardware acceleration: on
> Language: en
> Zoom: 100% | font: 16px | density: default
> Theme: dark
> Log level: info
> ```
>
> **Display**: 150 % screen scale, 100 % zoom, 2560×1440 · **Theme**: Dark

One bug per issue. If you find several, open several issues; it makes them easier to track and close.

## For maintainers: triage

### Labels

| Label | Meaning | Set by |
|---|---|---|
| `bug` | Something is broken | form |
| `visual` | Rendering, layout, colours, fonts, icons, animation | form |
| `parity` | Differs from the official client | form, triage |
| `crash` | Crash, freeze, won't start | form |
| `voice` | Voice, video, screen share, soundboard | form |
| `enhancement` | Feature request | form |
| `needs-repro` | Not reproduced by a maintainer yet, or details missing | form, removed at triage |
| `confirmed` | Reproduced by a maintainer | triage |
| `regression` | Worked in an earlier version | triage |
| `duplicate`, `wontfix`, `invalid` | GitHub defaults | triage |

The forms only apply labels that already exist in the repository, so create them once:

```sh
gh label create bug --color d73a4a --description "Something is broken" --force
gh label create visual --color c5def5 --description "Rendering, layout, colours, fonts, icons" --force
gh label create parity --color 5319e7 --description "Differs from the official client" --force
gh label create crash --color b60205 --description "Crash, freeze or won't start" --force
gh label create voice --color 0e8a16 --description "Voice, video, screen share, soundboard" --force
gh label create enhancement --color a2eeef --description "Feature request" --force
gh label create needs-repro --color fbca04 --description "Not reproduced yet or details missing" --force
gh label create confirmed --color 1d76db --description "Reproduced by a maintainer" --force
gh label create regression --color e99695 --description "Worked in an earlier version" --force
gh label create "priority: high" --color b60205 --description "Crash, data loss, login or voice broken for many" --force
gh label create "priority: medium" --color d93f0b --description "Visible bug with a workaround" --force
gh label create "priority: low" --color fef2c0 --description "Cosmetic or rare" --force
```

### Triage cycle

1. **Read** within a few days. Check that there's no token, cookie, HAR file, `journal.log`, email or unblurred private data. If there is, edit it out immediately (or delete the comment) and tell the reporter to change their password if a token was posted.
2. **Duplicates**: search for the same bug; close with `duplicate` and a link to the original.
3. **Missing details**: ask for whatever the form didn't get (debug info, logs, side-by-side screenshot) and keep `needs-repro`. Close after 14 days without an answer, with a note that it can be reopened.
4. **Reproduce** on the same OS and session type when possible, with the same zoom and window size. Once reproduced, replace `needs-repro` with `confirmed`. Add `regression` when an earlier version worked, and find the commit if it's cheap (`git bisect` between the two versions shown in the debug info).
5. **Parity**: for `visual` and `parity` issues, measure the official client (CSS, computed styles, DOM, as in [PARITY_WORKFLOW.md](PARITY_WORKFLOW.md)) and add or update the row in the matching [docs/parity/](parity/) table, with a link to the issue. The fix commit then marks the row ✅ with its source.
6. **Priority**:
   - `priority: high`: crash, freeze, can't log in, messages not sent or shown, voice broken for many people, any token or privacy leak (handle that as a security issue).
   - `priority: medium`: a visible bug or parity gap in a common view, with a workaround.
   - `priority: low`: cosmetic, rare, or in a rarely used view.
7. **Fix**: reference the issue in the commit or PR (`Fixes #123`). Ask the reporter to confirm on the next release, then close.

Crashes with a backtrace go first; most of them also show up in `phoenix.log` as an `ERROR panic` line with the location.

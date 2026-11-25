# Window chrome (frameless window, app top bar, window buttons)

CSS: ~/discord-phoenix-ref/current-client/css/300559.*.css. Dump: vanilla-guild-text (#13 bar, #46..#52 window buttons).

| element | CSS source | property | CSS value | our code | status |
|---|---|---|---|---|---|
| window | Electron `frame: false` | decorations | frameless | lib.rs window_options `WindowDecorations::Client` | ✅ (needs live check) |
| top bar | `bar_3zSH5d` | height | `--custom-app-top-bar-height` 32px | title_bar.rs render | ✅ |
| top bar | `bar_3zSH5d` | `-webkit-app-region` | drag | window_chrome::drag_region (move on drag, dblclick = maximize) | ✅ |
| bar buttons | `clickable_` / `winButtons_` | `-webkit-app-region` / pointer-events | no-drag / auto | `.occlude()` | ✅ |
| win buttons | `winButtons_3zSH5d` | gap | `--space-4` 4px | win_buttons | ✅ |
| divider | `winButtonsWithDivider_:before` | border / height / margin-inline | 1px `--border-subtle` / 32-12 / 4px | win_buttons | ✅ |
| button | `winButton_3zSH5d` | size | 32×32 | win_buttons btn | ✅ |
| glyph | dump #48/#50/#52 | box | 12×12 | glyph_box | ✅ |
| minimize glyph | dump #48 | shape | 10×1 rect at (1,6) | win-min-g | ✅ |
| maximize glyph | dump #50 | shape | 9×9 rect at 1.5, 1px stroke | win-max-g (10×10 border) | ✅ |
| close glyph | dump #52 | shape | 10×10 cross | assets `icons/window/close.svg` (own drawing) | ✅ |
| color | `winButtons_` | color | `--interactive-text-default` | fg | ✅ |
| min/max hover | `winButtonMinMax_:hover` | bg / color | `--interactive-background-hover` / `--interactive-text-hover` | hover + group_hover | ✅ |
| min/max active | `winButtonMinMax_:active` | bg / color | `--interactive-background-active` / `--interactive-text-active` | active + group_active | ✅ |
| close hover | `winButtonClose_:hover` | bg / color | `--control-critical-primary-background-default` / `-icon-default` | hover + group_hover | ✅ |
| resize edges | Chromium frameless Linux | inner border | 5px, corners 16px, ns/ew/nwse/nesw cursors | window_chrome::resize_handles | ✅ (no CSS; compositor resize) |
| maximized | — | resize strips | none when maximized/tiled/fullscreen | resize_handles | ✅ |

State coverage: default, hover, active (min/max), close hover. No focus/tooltip on window buttons in the client.
| close button | client "Minimize to tray" (default on) | behaviour | hide to tray | title_bar win-close → minimize when tray is up; tray.rs SNI Open/Quit | ⚠ minimised, not hidden: see "Hide to tray on Wayland" below |

## System tray (`tray.rs`)

Reference: `discord_desktop_core` `systemTray` module (core.asar of app 1.0.171, read 2026-11-24) and the
web client's `SystemTrayManager` (strings `CK()`, states `C$()`).

| Item | Client | Phoenix | Status |
|---|---|---|---|
| Protocol | Electron `Tray` on Linux = StatusNotifierItem + dbusmenu | zbus SNI (`org.kde.StatusNotifierItem-<pid>-1`) + `com.canonical.dbusmenu` | ✅ |
| Left click | `click` → `onTrayClicked` (show the window) | Plasma's system tray (`org.kde.plasma.systemtray`, `libKF6StatusNotifierItem`) calls `ProvideXdgActivationToken(token)` right before `Activate`; `tray::activation` keeps the token (single use, 10 s) and sends `xdg_activation_v1.activate(token, wl_surface)` for the window on GPUI's own `wl_display` (guest `wayland-client` backend from the raw window handle), so KWin raises and unminimises the window; without a token (X11, menu item "Open", other trays) it falls back to `Window::activate_window` | ✅ code + unit test; ⚠ not clicked live (a test instance would put a second window and tray icon on the user's screen) |
| Tooltip | `setToolTip(appName)` | `ToolTip` = "Discord Phoenix" | ✅ |
| Menu | `[Open {app}]`, then, connected only, `Rendre muet` / `Mettre en sourdine` (checkboxes) and `Ouvrir les paramètres vocaux et vidéo`, then `Rechercher les mises à jour…` / `Remerciements`, then `Quitter {app}`; a separator before each non-empty group | same order and labels (fr/en from the client's strings), checkmarks follow the panel mute/deafen, Voice settings opens Settings > Voix & vidéo, Remerciements opens `discord.com/acknowledgements` | ✅ / ➖ no "Check for updates" (Phoenix has no updater) |
| Icons | `tray`, `tray-unread`, `tray-connected`, `tray-speaking`, `tray-muted`, `tray-deafened` from `app/images/systemtray/linux`; priority connected → deafened > muted > speaking > connected, else unread, else default | the same PNGs loaded at runtime from the local Discord install (never committed), app icon as fallback; state polled every 500 ms, `NewIcon` / `LayoutUpdated` signals | ✅ / ⚠ unread = any mention or unread DM (client `hasAnyUnread`), server mute/suppress not modelled |
| Close hint balloon | `displayBalloon` (Windows only) | — | ➖ |

Tests run on a separate process only: the user's Phoenix and windows are never closed or minimised.

## Hide to tray on Wayland

The client hides its window (`BrowserWindow.hide()`) when "Minimize to tray" is on. Phoenix minimises
instead, on purpose:

* an `xdg_toplevel` can only disappear by being unmapped (null buffer + commit), and mapping it again
  needs a fresh initial commit / configure cycle;
* GPUI 0.3.12 has no hide / show for a window (`PlatformWindow` has `minimize`, `activate` and
  `remove`): an unmap done behind its back would be undone by the next frame, or break the configure
  handshake;
* closing the window and opening a new one on Open would rebuild the root view's window state
  (focus handles, text inputs, IME, scroll positions, subscriptions) and needs GPUI work first.

So the close button minimises when the tray is up (KWin keeps the task bar entry), and the tray Open
action raises it with the token above. Revisit if GPUI gains `hide`/`show` for Wayland windows.

## Floating PiP, Call Popout & Compact Window Mode

- **Floating Voice/Video PiP** (`ui/pip.rs`): shown when connected to a voice channel while viewing another channel (and not popped out), only for a stream the user is watching: live 2026-11-25 over CDP on another route, `[class*=pictureInPicture_]` has 0 children for a voice-only call, an unwatched stream and the camera on. 320×180 16:9 card, draggable into all 4 corners with snap-to-corner and viewport clamping, showing active stream/camera/speaker, hover controls (mic, deafen, disconnect, return to call, popout).
- **Multi-window Call Popout** (`ui/call_popout.rs`): `call-popout` button in `call_view.rs` and `pip.rs` opens a secondary frameless GPUI window (`cx.open_window`) observing `PhoenixApp` for live state sync (tiles, video/stream, mute/deafen/disconnect), with clean return to the main window on close.
- **Narrow windows**: the custom compact mode (member list hidden under 940 px, channel drawer under 720 px) was removed: the 2026 client has neither. The real thresholds are in `responsive.md` (section "Width thresholds from the client CSS and JS"). Window minimum 940×500 and default 1280×720 from `discord_desktop_core` `core.asar` (`MIN_WIDTH=settings.get("MIN_WIDTH",940)`, `MIN_HEIGHT … 500`, `width:1280,height:720`).
- **Window Shortcuts** (`shortcuts.rs`): `F11` toggles fullscreen (`ToggleFullscreen`), `Ctrl+`/`Ctrl-`/`Ctrl+0` step page zoom (50–200 %).

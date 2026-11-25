# Keyboard shortcuts parity

Source: the live 2026 client's shortcuts modal (Ctrl+/), read on 2026-11-24, plus the
`binds:[...]` keybind table in the local web bundles. Implementation:
`crates/phoenix-app/src/shortcuts.rs` (bindings, handlers, Ctrl+/ modal, tests).

## Navigation
| Shortcut | Action | Status |
|---|---|---|
| Ctrl+Alt+Up / Down (also Ctrl+Shift+[ / ], Ctrl+Shift+Tab / Ctrl+Tab) | Previous / next server (Home first, wraps) | ✅ |
| Alt+Up / Down | Previous / next channel (text channels, threads, the joined voice channel; DMs at Home; wraps) | ✅ |
| Alt+Left / Right | Back / forward in history | ✅ |
| Alt+Shift+Up / Down | Previous / next unread channel (skips muted) | ✅ |
| Ctrl+Shift+Alt+Up / Down | Previous / next channel with mentions | ✅ |
| Ctrl+Shift+Alt+V | Jump to the current call | ✅ |
| Ctrl+Alt+Right | Toggle between the last server and DMs | ✅ |
| Ctrl+K | Quick Switcher | ✅ |
| Ctrl+Shift+N | Create or join a server | ✅ |
| Ctrl+1..9 | Nth rail item: 1 = Home (bound in the client as `mod+${n}`, not listed in the modal) | ✅ |

## Voice and video
| Shortcut | Action | Status |
|---|---|---|
| Ctrl+Shift+M | Toggle mute | ✅ |
| Ctrl+Shift+D | Toggle deafen | ✅ |
| Ctrl+Enter | Answer an incoming call | ✅ `ui::incoming_call` (popup `root_dcNks8` while CALL_CREATE/UPDATE rings us; joins the call) |
| Esc | Decline an incoming call | ✅ POST `call/stop-ringing`, like the client keydown handler |
| Ctrl+' | Start a call in a DM or group DM | ✅ (1:1 DM: opens our call UI) |
| Ctrl+Shift+B | Toggle the soundboard | ✅ outside a call: the live client does nothing (synthetic keydown over CDP, 2026-11-24, no popout or dialog appeared; the same method opens Ctrl+/), and Phoenix now ignores it too. ➖ in a call — **besoin utilisateur**: the popout only exists while connected to voice, and joining a call is forbidden on the real account; Phoenix opens the voice panel's placeholder there. The picker itself is tracked in `pickers.md` |

## Chat
| Shortcut | Action | Status |
|---|---|---|
| Shift+Esc | Mark server read | ✅ |
| Esc | Mark channel read (and jump to present) | ✅ |
| Ctrl+Shift+T | Create a group DM | ✅ |
| Ctrl+Shift+Alt+N | Create a thread | ✅ Focused message: opens the thread form. Otherwise: opens the “Nouveau fil” sidebar (rebuilt from ref-cache create-thread-sidebar-*.json), which sends `POST /channels/{id}/threads` and then the starter message |
| Ctrl+P | Toggle pins popout | ✅ |
| Ctrl+I | Toggle inbox popout | ✅ |
| Ctrl+Shift+E | Mark the inbox's listed channels read (only while the inbox is open) | ✅ |
| Ctrl+U | Toggle member list | ✅ |
| Ctrl+E / Ctrl+G / Ctrl+S | Emoji / GIF / sticker picker | ✅ |
| Page Up / Page Down | Scroll the chat | ✅ |
| Shift+Page Up | Jump to the oldest unread message | ✅ |
| Any key | Focus the composer and type the character at the caret | ✅ |
| Alt+R | Record a voice message | ✅ opens the recorder bar like the client's `RECORD_VOICE_MESSAGE` handler (see `voice-messages.md`). The measured account lacks the desktop recording experiment, so its live client does nothing on Alt+R; Phoenix follows the enabled code path. The microphone is only opened when the user presses it |
| Ctrl+Shift+U | Upload a file | ✅ |
| Ctrl+Shift+L | Copy channel link | ✅ |

## Focused-message shortcuts
Shift+Tab from an empty composer focuses the newest message. Up and Down then move the focus (clamped, no wrap), and the focused row gets the hover/selected background. The focus clears when another element takes focus.
We have not checked how the client itself enters message focus (CDP can't send real Shift+Tab).
| Shortcut | Action | Status |
|---|---|---|
| E | Edit (own message) | ✅ |
| Backspace | Delete (own message, with confirm; Shift skips it) | ✅ |
| P | Pin | ✅ |
| + | Add reaction | ✅ |
| R | Reply | ✅ |
| F | Forward | ✅ |
| S | Speak the message (text to speech) | ✅ via `spd-say` (speech-dispatcher), falling back to `espeak-ng`; shows a toast when neither is installed |
| Ctrl+C | Copy text | ✅ |
| Alt+Enter | Mark unread | ✅ |
| Esc | Focus the composer | ✅ |

## Misc
| Shortcut | Action | Status |
|---|---|---|
| Ctrl+Shift+H, F1 | Get help (opens support site) | ✅ |
| Ctrl+F, Ctrl+Shift+F | Search | ✅ |
| Shift+F10 | Open context menu (focused message) | ✅ |
| Ctrl+/ (also Ctrl+Shift+/) | Keyboard combos modal (Esc closes it) | ✅. Rebuilt from the live modal (ref-cache `shortcuts-modal-default.json`). Card measures 950x672 in both at 1536x840, keybind cards are 180x81 with a 24px gap, and caps are 23px high. Missing: the Discord Revolution arrows decoration (Discord's SVG assets, not copied), real SVG arrow caps (we use ↑↓←→ glyphs), Grid columns follow `repeat(auto-fill, minmax(180px, max-content))`, with free space shared out when a row is too wide |
| H H Right N K | Easter egg | ✅ (wt/sounds, `ui/easter.rs`) "Puissance démoniaque": in the Ctrl+/ modal the last five keydowns H H Right N K (bundle module 453244, not with reduced motion) replace the list with the client's Raging Demon: 1 s 50 % backdrop, layer + app shake (`Shakeable` 2400 ms, intensity 5) at 1.8 s, eight explosions every 240 ms with the client's nine images (`/assets/3e4d5e…png` … `a53fe6…svg`, loaded at runtime from the local cache or discord.com/assets) and the CSS keyframes (`explCircle*`, `explLines*`, duration .15 s), the symbol 750 ms after the last one for 3 s, then the modal closes 500 ms after the backdrop fades. Same modal: the DDR arrows (`ddrArrows_jCEr5v`, sprites + `ddr-*` sounds, 100 ms throttle). The symbol background is the client's ray fan (`703942.f0dddb56b335a1ca.css` `.symbolBackground_CA3Z_c:before/:after`: fourteen hard-stop `linear-gradient(θ, transparent 50%, #fff/#ddd 0)` layers, 90° to −90°, on each half, the `:before` half turned 180°): `easter::rays` paints the same thirteen wedges per half and their point reflection, with the same opacity curve. ➖ side-by-side compare: triggering it in the client plays the explosion audio on the user's machine while they use it |
| Ctrl+Alt+Shift+W | Lo-fi Wumpus | ⚠ (wt/sounds) the music part: `vibing_wumpus` (`/assets/f4b4eb5a3dd2dbff.mp3`, runtime) looped and faded to .5 in 100 ms steps of .2 × (target − start), like `VIBING_WUMPUS_PLAY_MUSIC`; the same keys fade it out and stop it. ❌ the modal: it is a Rive animation (`VibingWumpusRive`, .riv) that GPUI cannot draw, and its chunk is not in the local cache |

The composer lets the app-wide combos through, as the client's textarea does: Page Up/Down, Alt+arrows, Ctrl+Alt+…, Ctrl+Shift+… (except the editing ones), and Ctrl+I/P/U/K/S//. Text-editing keys stay with the input. See `shortcuts/message.rs::passes_through_composer`.

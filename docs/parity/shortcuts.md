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
| Alt+R | Record a voice message | ➖ **besoin utilisateur**. Listed in the live Ctrl+/ modal ("Enregistrer un message vocal", Alt R, checked 2026-11-24), but pressing it in the client opens the recorder, which asks for and uses the microphone and leads to an upload: not triggered, the campaign forbids mic capture and sends. Phoenix opens a placeholder only; nothing is recorded or sent. Measuring the recorder needs the user (or a throwaway account) to open it |
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
| H H Right N K | Easter egg | ➖ Listed in the live Ctrl+/ modal as "Puissance démoniaque" (H H → N K, checked 2026-11-24). The effect is Discord's own animation and sound assets, which the clean-room rule forbids shipping; not triggered on the real client (it plays audio on the user's machine) |
| Ctrl+Alt+Shift+W | Lo-fi Wumpus | ➖ Listed in the live Ctrl+/ modal as "Écoute des beats lo-fi avec Wumpus" (checked 2026-11-24). It plays Discord's own Wumpus music video, an asset we cannot ship; not triggered (audio) |

The composer lets the app-wide combos through, as the client's textarea does: Page Up/Down, Alt+arrows, Ctrl+Alt+…, Ctrl+Shift+… (except the editing ones), and Ctrl+I/P/U/K/S//. Text-editing keys stay with the input. See `shortcuts/message.rs::passes_through_composer`.

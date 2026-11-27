# Input QA

Static renders prove a view looks right, not that it reacts. The input QA runs
the offscreen `real_shots` window on the real account (read-only backend, no
acks) and feeds it the same `PlatformInput` events the Linux platform sends:
wheel notches as `ScrollDelta::Lines` (3 lines a notch, like Wayland
`AxisValue120`), touchpad swipes as a burst of `ScrollDelta::Pixels`, mouse
down / up with a click count, moves with a pressed button for drags, and
keystrokes through `window.dispatch_keystroke` (so key bindings and the text
input handler both see them). Between steps it reads app state and compares.

```sh
cd /mnt/windata/project-phoenix/compare/qa-input
PHX_AGENT=1 PHX_NO_SCROLL=1 PHX_MEMBERS=true PHX_IN_VOICE=false PHX_FOLDERS=0 \
PHX_W=1280 PHX_H=800 PHX_SCALE=1.5 PHX_GUILD=1369382919948210248 \
PHX_CHANNEL=1574797385463496724 PHX_SCRIPT=~/discord-phoenix/scripts/qa/basic_input.qa \
flock /tmp/phoenix-render.lock phoenix-nice ~/discord-phoenix/int-bin/real_shots qa.png
```

Without `PHX_NO_SCROLL`, `PHX_MEMBERS` and `PHX_IN_VOICE`, `real_shots` copies the
sidebar scroll, the member list toggle and the voice panel from the official client
over CDP. The 27/11 runs inherited a hidden member list that way (`members item #2 is
not laid out`, `members_y` stuck at 0) and a scrolled sidebar (channel click on the
wrong row), so the scenario pins them. `PHX_FOLDERS=0` closes every rail folder, so the
guild step opens the second folder (40,173) and clicks its first server (40,229).

#phoenix gets new test messages all the time, so the video steps do not click a
fixed point: `jump <message id>` scrolls to the test video message the way a reply
or search jump does, `@video` is the bounds of the video player painted last
(`region video @video` hashes its pixels, `@video.br+-16,-16` is the fullscreen
button), and `action bottom` goes back to the present afterwards.

The offscreen window runs on GPUI's test dispatcher, whose timers only fire when its
clock moves. `wait`, `until` and every step that drives frames advance that clock by
the real time spent, so timer-driven code (the video ticker, hover delays, springs
scheduled with `timer`) runs as on the desktop. Before 27/11 it did not: the video
started (`video_t=0.00`) but its ticker never ran between screenshots, which only
moved because each shot advances the clock by 4 s.

The attachment player is the legacy one unless `PHOENIX_DVP=1`: Space plays or
pauses, ←/→ seek 5 s, digits jump to a tenth, fullscreen is the bar button and Esc
leaves it. `l`, `j`, `k`, `f` and `m` exist only in the DVP player, so the scenario
uses `key right` and clicks the fullscreen button.

A bottom-aligned list that sits at its end anchors past its last item, and GPUI's
`ListState::bounds_for_item` then returns `None` for every row (`chat item #108 is
not laid out`). Point at the newest message from the list edge (`@chat.b+0,-60`)
instead of `@chat#-2` while the chat is at the bottom.

Every check prints `QA OK [step] ...` or `QA KO [step] ...`, then
`QA summary ok=N ko=M` and the list of shots (`qa-<name>.png`).
`PHX_SCRIPT_STRICT=1` turns a failed check into a failed run.

The mock counterpart, `ui/input_qa_tests.rs`, sends the same kind of events to a
`VisualTestContext` window on demo data and runs with `cargo test` in the
integrator.

## Script commands

Points are window coordinates in logical px (`x,y`) or anchors:
`@chat`, `@sidebar`, `@members`, `@guilds` (list viewports), `@settings` (the
settings scroll area), `@video` (the video player painted last), `@window`. `.tl .tr .bl .br .t .b .l .r` pick a corner
or edge instead of the centre, `#n` picks list item `n` (`#-1` = last), and
`+dx,dy` offsets the point: `@chat#-1.tl+30,20`, `@chat.b+0,-24`.

| command | effect |
| --- | --- |
| `step <label>` | names the checks that follow |
| `wait <ms>` | drives frames (animations, springs, network) |
| `move <pt>`, `hover <pt> [ms]` | mouse move |
| `down <pt> [left\|right\|middle]`, `up <pt> [btn]` | one press or release |
| `click <pt> [btn]`, `dblclick <pt>` | move, then down + up (click count 1, 2) |
| `drag <from> <to> [steps]` | press, moves with the left button held, release |
| `wheel <pt> <notches>` | mouse wheel, positive = up (towards older messages) |
| `lines <pt> <dy>`, `pixels <pt> <dy>` | one raw scroll event |
| `touchpad <pt> <px> [events]` | a touchpad swipe split in pixel events |
| `key <k>...` | keystrokes (`pageup`, `ctrl-k`, `escape`, `shift-tab`) |
| `type <text>` | one keystroke per character |
| `hold shift\|ctrl\|alt\|none` | modifiers for the following mouse events |
| `action pageup\|pagedown\|top\|bottom` | dispatch the action itself (no key) |
| `select <channel id>` | setup only: open a channel without clicking |
| `jump <message id>` | setup only: jump to a message of the open channel |
| `text <text>` | insert text in the composer (one input change, like a paste or an IME commit; `\n` = new line) |
| `clip_image <file>` | put a PNG or JPEG on the offscreen window's clipboard (never the user's), for `key ctrl-v` |
| `region <name> x,y,w,h` | a screen rectangle for `px:<name>` |
| `shot <name>` | capture `<out>-<name>.png` |
| `print <probe>...`, `mark <probe>...` | print, or remember for `changed` / `up` / `down` |
| `expect <probe> <op> [value]` | one check |
| `until <ms> <probe> <op> [value]` | wait until the check holds, then report it |

Operators: `== != < > <= >=`, `~` (contains), `changed` / `same` (against the
mark), `up` / `down` (greater / smaller than the mark by more than the value,
0.5 by default).

Probes: `chat_y` (px from the top of the message list), `chat_end` (px above the
bottom), `chat_max`,
`chat_item`, `chat_items`, `chat_thumb` (height of the chat scrollbar thumb), `chat_skeleton` (1 while the loading placeholders at the top are on screen), `at_bottom`, `sidebar_y`, `members_y`, `guilds_y`,
`settings`, `settings_y`, `channel`, `guild`, `msgmenu`, `navmenu`,
`usermenu`, `menus`, `popout`, `profile`, `modal`, `switcher`, `composer`,
`ac` (autocomplete rows), `video_t` (current time of the clicked video, -1
when none), `fullscreen`, `px:<region>` (hash of the region's pixels, for
"the video frame changes"), `at:<point>` (prints where a point resolves).

More probes: `flash` (message being highlighted after a jump), `replying`, `messages` (loaded
messages), `reached_start`, `oldest`, `last`, `last_mine`, `last_reply`, `last_files`,
`last_embeds`, `last_thread`, `last_app` (application of the last message), `last_text` (sandbox
channel only), `sending`, `uploads`, `chat_error`, `typing` (typing users in the open channel),
`typing_any`, `typing_label`, `unread_guilds`, `mention_total`, `unreads` (`guild:mentions` of every
unread guild), `search` (loaded results, -1 when closed), `picker` (open picker tab), `audio_t`
(position of the audio or voice message being played, -1 when none), `theme`, `zoom`, `font`,
`notify_on`, `notify_sounds`, `settings_error`.

Writes from a script: with `PHX_WRITE=1`, `real_shots` sends, replies, uploads, creates threads
and polls (text or question starting with `[Test Phoenix]`), schedules messages, votes, and runs
application commands and components, ONLY in the sandbox channel `1574797385463496724` and in the
threads it created there. A GIF picked in the picker is sent there with the test prefix in front of
its link. Everything else stays refused. Run setting changes with `XDG_CONFIG_HOME` pointing at a
copy of the config, so the local preferences of the user's Phoenix are never rewritten.

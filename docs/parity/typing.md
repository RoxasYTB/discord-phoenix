# Typing indicator (TypingUsers, inTextChannel)

Source: current-client/css (`base_U9Tvvz`, `inTextChannel_U9Tvvz`, `dot_U9Tvvz`, `cooldownWrapper_bbCgC2`) and the TypingUsers component in the client JS (locale keys lJ9sZX / rB0CUa / StKThj / Q8lUnE).

| element | CSS / JS source | property | value | our code | status |
|---|---|---|---|---|---|
| row position | inTextChannel_.base_ | top / inset-inline | -24px / 0 (above the form, over the chat) | ui/chat/composer.rs form `.absolute().top(-24)` | ✅ |
| form margin | --custom-chat-input-margin-bottom | margin-bottom | --space-xs (no extra 24px) | composer.rs form `.mb(space::xs())` | ✅ |
| row box | base_ | height / font / weight / color | 24px, 12px/16px, 500, --text-subtle | composer.rs `typing()` | ✅ |
| row start | inTextChannel_ / typingDots_ | padding-inline-start | --space-md + 8px | composer.rs `typing()` | ✅ |
| dots | dots_ / dot_ | gap / size / color | 2px, 7px circle, --text-default | composer.rs `dot` | ✅ |
| dot animation | typing-dot-pulse | 1.2s steps(12), delays -1.2/-1.05/-0.9s, .32/.8 ↔ 1/1 at 25% | composer.rs `dot_frame` | ✅ |
| dot animation, reduced motion | `.reduce-motion .dot_U9Tvvz` | 2.4s steps(24), delays -2.4/-2.1/-1.8s (same phases) | composer.rs `dot_timing` (also the slash command « réfléchit » dots) | ✅ sweep5, test `reduced_motion_dots_run_slower_with_finer_steps`. `.stop-animation .dot_U9Tvvz` (no animation, opacity 1) is never set by the 2026 bundle (only `stop-animations` on layers), so it is not reproduced |
| text | inTextChannel_ .text_ | margin-inline-start | --space-md + --space-8 | composer.rs `line` | ✅ |
| names | .text_ > strong | color / weight | --text-default / semibold | composer.rs `bold` | ✅ |
| name choice | JS getName(guild, channel, user) | nick → global name → username | app.rs `typing_name` | ✅ |
| strings | lJ9sZX/rB0CUa/StKThj/Q8lUnE | "… est en train d'écrire...", "A et B sont…", "A, B, et C sont…", "**Plusieurs personnes** sont…" | locales/*.yml chat.typing_* | ✅ |
| overflow fallback | JS qD/0qZ | 1–3 typers, text width + 56px > typingDots_ box → "**Plusieurs personnes** sont en train d'écrire…" | composer.rs `overflow` | ✅ |
| slowmode | cooldownWrapper_ | right of the same row, padding 3 8, pe 16 | composer.rs `cooldown` | ✅ |

# Locked thread banner (ChannelChatNotice, chatHeaderBar_By_-nl)

| element | source | property | value | our code | status |
|---|---|---|---|---|---|
| banner | chatHeaderBar_By_-nl | bg / padding / gap | --background-mod-normal, 8px 16px, 16px | ui/chat/thread_banner.rs | ✅ |
| text | chatHeaderBarText_ + body-sm/normal | flex 1, text-default | V/JF2N (locked) / rEeodK (archived + locked) | thread_banner.rs | ✅ |
| composer | client canSendMessage | locked thread without MANAGE_THREADS | no-permission text area | discord_backend/nav.rs `send_blocked` | ✅ |
| Open / Unlock buttons | 0dvvEi / zA9d1J | MANAGE_THREADS only | `ChannelExtra.can_manage_threads` + `thread_banner.rs` secondary sm button (read-only stub on click to avoid writes outside sandbox) | ✅ |

# Blocked / ignored users

| element | source | value | our code | status |
|---|---|---|---|---|
| model | RelationshipStore: type 2 = blocked, `user_ignored` = ignored (not if blocked) | ui/home/data.rs `is_blocked` / `is_ignored` | ✅ |
| collapsed run | n8/n5 collapsed-message-item, +FcYM/ / VFWjc+ plurals, " — " Montrer (XJuakA) / Masquer (fgq1gs) | ui/chat/blocked.rs | ✅ |
| style | blockedSystemMessage_ 16/22, blockedMessageText_ --text-default, blockedIcon_ 16px --text-muted (hover --icon-feedback-critical), blockedAction_ --text-muted / hover --text-link + underline, expanded_ --message-background-hover | blocked.rs | ✅ |
| icons | DenyIcon (blocked), EyeSlashIcon (ignored) | icons/names.rs | ✅ |
| typing | TypingUsers rejects isBlockedOrIgnored | app.rs `typing_name` | ✅ |
| mentions / notifications | blocked authors don't ping | app.rs append_message, notifications.rs | ✅ |
| blocked DM | composer notice + Unblock (existing) | composer.rs blocked_dm_notice | ✅ |
| ignored with relationship type 0/5 | kept as `RelationshipKind::None` when `user_ignored` is true | `discord_backend/nav.rs` `relationship()`, `ui/home/data.rs` `RelationshipKind::None` | ✅ |

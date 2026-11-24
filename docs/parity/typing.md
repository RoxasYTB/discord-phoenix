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
| Open / Unlock buttons | 0dvvEi / zA9d1J | MANAGE_THREADS only | write actions, not shown | — | ❌ |

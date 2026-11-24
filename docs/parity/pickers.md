# Expression pickers (emoji / GIF / sticker)

Source: dumps `vanilla-picker-emoji`, `vanilla-picker-gif`, `vanilla-picker-sticker` (1280x700 viewport) + CSS bundle 569603.

| element | CSS / dump source | property | value | our code | status |
|---|---|---|---|---|---|
| picker body | emojiPicker_-sZhZn (#2502) | size | 497x427 | pickers/mod.rs PICKER_W | ✅ |
| nav tabs | nav_58b0l2 (#2495) | height / padding | 30 / 0 16 | pickers/mod.rs tab_button | ✅ |
| search | container_VhFsFg (#2504) | size | 272x40 | pickers/mod.rs search_box | ✅ |
| diversity | diversitySelector_eHzdRB (#2511) | size | 38 | grid.rs tone_button | ✅ |
| emoji cell | emojiItemMedium_6OcK5D | size / img | 48 / 40 | grid.rs render_row | ✅ |
| rail | wrapper_6AvvhR (#2697) | width | 48 | grid.rs rail | ✅ |
| rail Nitro row | row_3MXiRB (#2701) | border-bottom / pb / mb | 1px --border-subtle / 8 / 8 | grid.rs render_body | ✅ (non-Nitro only) |
| rail pill | .small__o8_Xq .item.selected | height | 24px, on default categories too | grid.rs rail_pill | ✅ |
| frequently used | categorySection (#2525), 42 items | source | settings-proto/2 emoji frecency | grid.rs build + rest/frecency.rs | ✅ |
| inspector | inspector_1bDrGE (#2687) | height / graphic | 48 / 28 | grid.rs inspector | ✅ |
| GIF tiles | result_H8nOze | size / gaps | 231x110, 11 / 12 | gif.rs | ✅ |
| GIF favorites tile | img gif_H8nOze under categoryFadeBlurple | image | newest favorite GIF | gif.rs (favorite_gifs) | ✅ |
| GIF trending tile | img under categoryFade | image | first trending GIF | gif.rs | ❌ blank on real data, cause unknown |
| sticker cell | sticker_5z8c5P | size / gap | 98 / 13 | sticker.rs | ✅ |
| sticker frequent | stickerCategoryGeneric (#2731) | rail + section | clock, first | sticker.rs | ✅ |
| soundboard | official [Soundboard guide](https://support.discord.com/hc/en-us/articles/12612888127767-Discord-Soundboard-Guide-Using-Adding-and-Managing-Sounds); reference UI still needs an active voice call | access | opened from voice controls; guide describes choosing an available server, then a sound. No Phoenix call, sound playback, upload, or settings mutation was performed for this audit | — | ❌ UI/API not measured; guide establishes the high-level flow only |

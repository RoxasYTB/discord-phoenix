# Patches to GPUI (`gpui-pre` 0.3.12)

`vendor/gpui-pre` is the crates.io `gpui-pre` 0.3.12 source, wired in through
`[patch.crates-io]` in the workspace `Cargo.toml` (and excluded from the workspace
members). Examples, benches, tests, docs and the lockfile were removed from the copy,
with their `[[example]]`/`[[bench]]`/`[[test]]` tables, to keep it small. The library
source is untouched except for the patches listed here. Each one is marked
`PHOENIX PATCH` in the code.

To upgrade GPUI: copy the new release into `vendor/gpui-pre`, re-apply the patches below,
and bump the `=0.3.12` pins in `crates/phoenix-app/Cargo.toml`.

## 1. Subpixel layout (`src/taffy.rs`, `impl ToTaffy<f32> for AbsoluteLength`)

**Problem.** Upstream rounds every authored length (padding, margin, gap, width, height)
to whole device pixels *before* layout. At a fractional scale such as 125 % this makes
lengths drift: a 38px channel row (1 + 36 + 1) is laid out as 1 + 45 + 1 = 47 device px
instead of 47.5. The official client (Chromium) keeps layout fractional and snaps only
the painted edges. So our lists drifted by 0.5 dp per row: about 3 px after 6 rows in the
channel list, with the same drift in the guild rail (measured with `scripts/compare`
against the live client at DPR 1.25).

**Patch.** `AbsoluteLength::to_taffy` returns the exact device-pixel value
(`logical * scale_factor`) instead of `round_half_toward_zero(logical * scale_factor)`.
The rest of upstream's snapping is kept:
- `layout_bounds` still rounds each element's outer edges in absolute device pixels after
  layout. That is one snapping step at "paint", and coincident edges still close.
- Border widths still go through `round_stroke_to_device_pixel`, so a 1px border stays
  1 dp and is never blurred.
- Measured text/leaf sizes are still ceiled to device pixels.

At integer scale factors (1x, 2x) authored integer lengths are already whole device
pixels, so the output there is unchanged.

## 2. Fractional measured sizes (`src/element.rs` `Drawable::layout_as_root`, `src/window.rs`, `src/taffy.rs`)

**Problem.** `list` measures each item as its own layout root and stacks the measured
heights. The measured size was the snapped one, and the far edge of a root at origin 0
rounds half toward zero (47.5 becomes 47), so with patch 1 alone the channel list still
lost 0.5 dp per row.

**Patch.** `layout_as_root` returns the raw taffy size (new
`Window::layout_size_unsnapped` / `TaffyLayoutEngine::layout_size_unsnapped`). Items are
then placed at fractional offsets, and `layout_bounds` snaps their edges in absolute
device pixels at paint, so they close with no drift.

## Effect (live compare, DPR 1.25, same screen)

| region | upstream | patched |
|---|---|---|
| guild rail | 14.2 % | 2.6 % (patch 1) |
| member list | 16.0 % | 12.3 % (patch 1) |
| messages | 6.8 % | 5.4 % (patch 1) |
| overall | 8.1 % | 6.7 % (patch 1) |

## How the patch is wired (shared build cache)

`Cargo.toml` patches `gpui-pre` with a **git rev of this repository**, not a path.
Cargo gives a path dependency a different package id in every checkout or worktree, which
rebuilt GPUI and everything above it (gpui-component, the platform crates…) once per worktree.
A git rev has one id everywhere, so all checkouts share one compiled GPUI.

After changing `vendor/gpui-pre`: commit and push it, then set `rev` in `Cargo.toml`
to that commit and run `cargo update -p gpui-pre`.

## 3. Paint only the children that were prepainted (`src/elements/div.rs`, `src/element.rs`)

**Problem.** The real app panicked intermittently with `must call prepaint before paint`
in `paint<Stateful<Div>>`. `Div::prepaint` and `Div::paint` each recompute the style
and skip the children when `display == None`. Hover and group-hover styles are part of
that style, and the hover state can differ between the two calls (the group hitbox of
the current frame is only known after prepaint). A div that is `hidden()` and becomes
`flex()` on group hover (the voice channel row icons in `ui/sidebar.rs`) skipped its
children in prepaint and then painted them: the `vc-chat` stateful child panicked.

**Patch.** `Div` remembers which children it prepainted (`prepainted_children`) and
paint follows that list instead of reading `display` again. The panic messages of
`Drawable::prepaint`/`paint` now name the element type, its id, its source location and
the phase it was in.

## Page zoom (`src/window.rs`, `src/interactive.rs`, `src/platform.rs`)

**Problem.** Discord's zoom (Ctrl+= / Ctrl+- / Ctrl+0, Appearance > Zoom level) is
Electron `webFrame.setZoomFactor`, i.e. Chromium page zoom: every CSS px, border, icon,
hit area, scrollbar and popout position scales together, and the viewport shrinks in
CSS px. Scaling lengths by hand in the app (`z(v)`) left every unwrapped `px()` at 100 %.

**Patch.** `Window::set_page_zoom(f32)` / `Window::page_zoom()`. The window keeps the raw
platform scale factor and content size; the effective `scale_factor` becomes
`platform * zoom` and `viewport_size` becomes `content / zoom`. `dispatch_event` divides
input positions (and pixel scroll deltas) by the zoom, and the values going back to the
platform (window menu position, client inset, input region, IME bounds, character index
lookups) are multiplied by it. Search `PHOENIX PATCH (page zoom)`.

## 4. Chromium-style ellipsis (`src/elements/text.rs`, `src/text_system/line_wrapper.rs`)

**Problem.** Phoenix cut truncated labels one character before the client at the same
width ("Happy 03…" against "Happy 03/1…" in the member list). Two causes: upstream trims
spaces and ASCII punctuation before the "…", and it decides the cut on a sum of
per-character advances, which ignores kerning and overestimates the shaped width.

**Reference.** Chromium 152 (the client's engine), probed in headless Chrome with a
monospace font so every cut is readable:
- `white-space: nowrap; text-overflow: ellipsis` keeps the longest prefix whose width
  plus the ellipsis fits, spaces and punctuation included ("Happy …", "ab, …", "ab.…").
- `-webkit-line-clamp` ends the last line at its own soft wrap, drops the collapsible
  space, then appends "…" ("bbbb…", "cccc, dddd.…"). It cuts inside a word only when the
  word plus the ellipsis does not fit. Punctuation is kept.

**Patch.** `LineWrapper::truncate_line_end_shaped` cuts one shaped line at the last whole
grapheme whose shaped advance (`LineLayout::x_for_index`) plus the affix fits. `Text`
uses it for single-line end truncation (it already shaped the text to test the fit).
The per-character fallback and `truncate_wrapped_line` no longer trim punctuation, and
the last clamped line stops at its last word break when one fits. Search
`PHOENIX PATCH (ellipsis)`.

The commit is kept on the `gpui-ellipsis` branch of origin so the pinned `rev` stays
reachable.

## 5. Glyph offsets (`vendor/gpui-pre-wgpu/src/cosmic_text_system.rs`, `layout_line_no_separators`)

**Problem.** Combining marks painted one letter to the right of their base: the bot name
"ღ🌸~͓̽ǤŁa̠̠ĐØS~🌸ღ" in Aperture Sciences showed the U+0320 macrons under "Đ" instead of "a".
`cosmic_text::LayoutGlyph::x`/`y` are the pen position only; the shaper's GPOS offsets are in
`x_offset`/`y_offset` (em units) and cosmic-text adds them in `LayoutGlyph::physical`. GPUI
copied `x`/`y` and dropped the offsets, so a zero-advance mark sat at the pen position after
its base.

**Patch.** `ShapedGlyph::position` is `(x + x_offset·size, y − y_offset·size)`, like
`physical`. Advances, and so widths and `x_for_index`, are unchanged. Search
`PHOENIX PATCH (glyph offsets)`.

## 6. Cluster fallback (`vendor/gpui-pre-wgpu/src/cosmic_text_system.rs`, `compute_run_spans`)

**Problem.** The same name was 1.8px wider than in the client and got an ellipsis. GPUI picks
the font of a grapheme from its first character, so "~" + U+033D U+0353 and "a" + U+0320 U+0320
stayed in gg sans (which has "~" and "a" but no marks). cosmic-text then reshaped the whole
cluster in its own fallback, the system "Noto Sans" Regular (9.15 and 8.98px). Chromium gives
the cluster to the first font of the stack that covers every character (CDP
`CSS.getPlatformFontsForNode`: Noto Sans Medium, the web font, 8.15 and 8.18px).

**Reference.** Live client, 2026-11-24, `Range.getBoundingClientRect` on a probe span with
`font-family: var(--font-primary)` 16px/500: total 140.01px; prefix widths per grapheme in
`docs/parity/text.md`.

**Patch.** `pick_grapheme_slot` checks every non-default-ignorable character of a multi-char
grapheme: primary font if it covers all of them, else the first font of the fallback chain
that does, else the old first-character choice (cosmic-text's fallback takes over). Single
characters keep upstream's `pick_covering_slot`. Search `PHOENIX PATCH (cluster fallback)`.
Unit tests: `cluster_goes_to_the_first_font_covering_every_mark`,
`ignorables_do_not_need_a_glyph`, `uncovered_cluster_keeps_the_first_character_choice`.

## Wiring of `gpui-pre-wgpu`

`vendor/gpui-pre-wgpu` is the crates.io 0.3.12 source (bench table and `benches/` removed),
patched like `gpui-pre` through a git rev of this repository in `[patch.crates-io]`. Both
crates point at the same rev, kept on the `gpui-marks` branch of origin (it is
`gpui-ellipsis` + `vendor/gpui-pre-wgpu`). Only the two `[[package]]` entries of
`Cargo.lock` change when the rev moves: edit them by hand, because `cargo update -p` also
re-resolves unrelated crates (itertools, windows-sys).

## 7. Text box width (`src/elements/text.rs`, `ceil_to_layout_unit`)

**Problem.** After patches 5 and 6 the bot name had the client's advances but still got an
ellipsis: the "✓ APP" badge beside it was 46px wide against 44.7px in the client (compare
finish/ellipsis-1280x720, DPR 1.5). Upstream rounds every measured text box up to a whole
logical pixel, so "APP" in gg sans 12px/600 (23.16px, CDP `getBoundingClientRect`) took 24px.

**Patch.** The measured width is rounded up to 1/64 px (Chromium's LayoutUnit) instead. It still
never rounds down, so a text given its own measured width does not re-wrap. Unit test
`text_width_rounds_up_to_a_layout_unit`. Search `PHOENIX PATCH (text width)`.

Rev: `265d2211` on `gpui-marks` (patches 4 to 7).

## Running the vendored tests

`cargo test -p gpui-pre -p gpui-pre-wgpu --lib` (from the workspace, on the pinned rev) does not
compile: the crates.io packages ship without their dev-dependencies (`naga`, `util_macros`) and
without the files their tests `include_bytes!` (`assets/fonts/lilex`, `assets/fonts/ibm-plex-sans`,
`examples/image/*.jpg`). Tried on 2026-11-24 (95 s, compile errors only, no test ran). The
patch tests (`test_multiline_truncation_keeps_last_line_word_wrap`,
`cluster_goes_to_the_first_font_covering_every_mark`, `text_width_rounds_up_to_a_layout_unit`…)
need those pieces restored in `vendor/` before they can run.

// todo("windows"): remove
#![cfg_attr(windows, allow(dead_code))]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AtlasTextureId, AtlasTile, Background, Bounds, ContentMask, Corners, Edges, Hsla, Pixels,
    Point, Radians, ScaledPixels, Size, bounds_tree::BoundsTree, point,
};
use std::{
    fmt::Debug,
    iter::Peekable,
    ops::{Add, Range, Sub},
    slice,
};

#[allow(non_camel_case_types, unused)]
#[expect(missing_docs)]
pub type PathVertex_ScaledPixels = PathVertex<ScaledPixels>;

#[expect(missing_docs)]
pub type DrawOrder = u32;

/// A boolean stored as a `u32` so that GPU-facing structs contain no
/// compiler-inserted padding bytes, which would be undefined behavior to
/// reinterpret as `&[u8]` when writing instance buffers. Guaranteed to be
/// `0` or `1` by construction; shaders read it as a `u32`/`uint`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct PaddedBool32(u32);

impl From<bool> for PaddedBool32 {
    fn from(value: bool) -> Self {
        PaddedBool32(value as u32)
    }
}

#[derive(Default)]
#[expect(missing_docs)]
pub struct Scene {
    pub(crate) paint_operations: Vec<PaintOperation>,
    primitive_bounds: BoundsTree<ScaledPixels>,
    layer_stack: Vec<DrawOrder>,
    pub shadows: Vec<Shadow>,
    pub quads: Vec<Quad>,
    pub paths: Vec<Path<ScaledPixels>>,
    pub underlines: Vec<Underline>,
    pub monochrome_sprites: Vec<MonochromeSprite>,
    pub subpixel_sprites: Vec<SubpixelSprite>,
    pub polychrome_sprites: Vec<PolychromeSprite>,
    pub surfaces: Vec<PaintSurface>,
    pub glyph_gradients: Vec<GlyphGradient>,
    pub glyph_gradient_scrolls: Vec<GlyphGradientScroll>,
    pub glyph_animations: Vec<Option<std::sync::Arc<crate::GlyphAnimation>>>,
    pub sprite_sequences: Vec<ImageSequence>,
    pub gradient_time: f64,
    pub cutouts: Vec<CutoutSet>,
}

/// PHOENIX PATCH (cut-outs): the largest number of cut-out sets a frame can use. Primitives
/// painted after the table is full are drawn without their holes.
pub const MAX_CUTOUT_SETS: usize = 256;

/// PHOENIX PATCH (cut-outs): `PolychromeSprite::pad` holds the image sequence index (patch 20)
/// in its low 16 bits and the cut-out set index in its high 16 bits.
pub fn poly_sequence(pad: u32) -> u32 {
    pad & 0xFFFF
}

/// PHOENIX PATCH (cut-outs): the cut-out set index of a polychrome sprite `pad`.
pub fn poly_cutout(pad: u32) -> u32 {
    pad >> 16
}

/// PHOENIX PATCH (cut-outs): `pad` with its cut-out set index replaced by `cutout`.
pub fn poly_with_cutout(pad: u32, cutout: u32) -> u32 {
    (pad & 0xFFFF) | (cutout.min(0xFFFF) << 16)
}

/// PHOENIX PATCH (cut-outs): `pad` with its image sequence index replaced by `sequence`.
pub fn poly_with_sequence(pad: u32, sequence: u32) -> u32 {
    (pad & !0xFFFF) | (sequence & 0xFFFF)
}

/// PHOENIX PATCH (cut-outs): one hole, a rounded rectangle in device pixels. A hole with an
/// empty size cuts nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct CutoutHole {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub radius: f32,
    pub pad: [f32; 3],
}

impl CutoutHole {
    /// Signed distance from `point` to the edge of the hole: negative inside, positive outside.
    pub fn distance(&self, point: [f32; 2]) -> f32 {
        let half = [self.size[0] / 2., self.size[1] / 2.];
        let radius = self.radius.min(half[0]).min(half[1]).max(0.);
        let center = [self.origin[0] + half[0], self.origin[1] + half[1]];
        let qx = (point[0] - center[0]).abs() - half[0] + radius;
        let qy = (point[1] - center[1]).abs() - half[1] + radius;
        let outside = (qx.max(0.).powi(2) + qy.max(0.).powi(2)).sqrt();
        outside + qx.max(qy).min(0.) - radius
    }

    /// Coverage left by the hole at `point`, with the same half-pixel ramp as the shader.
    pub fn coverage(&self, point: [f32; 2]) -> f32 {
        if self.size[0] <= 0. || self.size[1] <= 0. {
            return 1.;
        }
        (self.distance(point) + 0.5).clamp(0., 1.)
    }
}

/// PHOENIX PATCH (cut-outs): the holes punched into the quads and images painted inside
/// `Window::with_cutouts` (an SVG `<mask>` with black shapes, like the client's BlobMask).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct CutoutSet {
    pub holes: [CutoutHole; 2],
}

impl CutoutSet {
    /// Coverage left by every hole at `point`.
    pub fn coverage(&self, point: [f32; 2]) -> f32 {
        self.holes.iter().map(|h| h.coverage(point)).product()
    }
}

// PHOENIX PATCH (image sequences): a polychrome sprite whose tile is one cell of a sheet of
// frames, picked at present time like the gradient scrolls, so an animated icon is played
// without notifying, laying out or painting its view.
/// The frames of an animated image laid out in a grid on one atlas tile, played at `fps`
/// from `start` (on `gradient_clock`) and held on the last frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ImageSequence {
    /// `gradient_clock` time of frame 0.
    pub start: f64,
    /// Frames per second.
    pub fps: f32,
    /// Added to the elapsed frame count before flooring it.
    pub phase: f32,
    /// Number of frames in the sheet.
    pub count: u32,
    /// Frames per row of the sheet.
    pub columns: u32,
    /// Size of one frame in the sheet.
    pub cell: crate::Size<crate::DevicePixels>,
}

impl ImageSequence {
    /// The frame shown at `now`.
    pub fn index(&self, now: f64) -> u32 {
        let last = self.count.saturating_sub(1);
        let at = ((now - self.start) * self.fps as f64 + self.phase as f64).floor();
        if at.is_nan() || at <= 0.0 {
            0
        } else {
            (at as u64).min(last as u64) as u32
        }
    }

    /// Whether a later frame than the one at `now` is still to come.
    pub fn running(&self, now: f64) -> bool {
        self.index(now) + 1 < self.count
    }

    /// Moves `sprite`'s tile from the first cell to the cell shown at `now`.
    pub fn apply(&self, sprite: &mut PolychromeSprite, now: f64) {
        let index = self.index(now);
        let columns = self.columns.max(1);
        let (column, row) = ((index % columns) as i32, (index / columns) as i32);
        sprite.tile.bounds.origin.x.0 += self.cell.width.0 * column;
        sprite.tile.bounds.origin.y.0 += self.cell.height.0 * row;
    }
}

#[expect(missing_docs)]
impl Scene {
    pub fn clear(&mut self) {
        self.paint_operations.clear();
        self.primitive_bounds.clear();
        self.layer_stack.clear();
        self.paths.clear();
        self.shadows.clear();
        self.quads.clear();
        self.underlines.clear();
        self.monochrome_sprites.clear();
        self.subpixel_sprites.clear();
        self.polychrome_sprites.clear();
        self.surfaces.clear();
        self.glyph_gradients.clear();
        self.glyph_gradient_scrolls.clear();
        self.glyph_animations.clear();
        self.sprite_sequences.clear();
        self.cutouts.clear();
    }

    /// PHOENIX PATCH (cut-outs): stores `set` and returns the index the primitives carry
    /// (`1 + position`), or 0 when the table is full.
    pub fn push_cutouts(&mut self, set: CutoutSet) -> u32 {
        if self.cutouts.last() == Some(&set) {
            return self.cutouts.len() as u32;
        }
        if self.cutouts.len() >= MAX_CUTOUT_SETS {
            return 0;
        }
        self.cutouts.push(set);
        self.cutouts.len() as u32
    }

    /// PHOENIX PATCH (cut-outs): the set an index from `push_cutouts` refers to.
    pub fn cutout_set(&self, index: u32) -> Option<&CutoutSet> {
        index.checked_sub(1).and_then(|i| self.cutouts.get(i as usize))
    }

    pub fn len(&self) -> usize {
        self.paint_operations.len()
    }

    pub fn push_layer(&mut self, bounds: Bounds<ScaledPixels>) {
        let order = self.primitive_bounds.insert(bounds);
        self.layer_stack.push(order);
        self.paint_operations
            .push(PaintOperation::StartLayer(bounds));
    }

    pub fn pop_layer(&mut self) {
        self.layer_stack.pop();
        self.paint_operations.push(PaintOperation::EndLayer);
    }

    pub fn insert_primitive(&mut self, primitive: impl Into<Primitive>) {
        let mut primitive = primitive.into();
        let clipped_bounds = primitive
            .bounds()
            .intersect(&primitive.content_mask().bounds);

        if clipped_bounds.is_empty() {
            return;
        }

        let order = self
            .layer_stack
            .last()
            .copied()
            .unwrap_or_else(|| self.primitive_bounds.insert(clipped_bounds));
        match &mut primitive {
            Primitive::Shadow(shadow) => {
                shadow.order = order;
                self.shadows.push(*shadow);
            }
            Primitive::Quad(quad) => {
                quad.order = order;
                self.quads.push(*quad);
            }
            Primitive::Path(path) => {
                path.order = order;
                path.id = PathId(self.paths.len());
                self.paths.push(path.clone());
            }
            Primitive::Underline(underline) => {
                underline.order = order;
                self.underlines.push(*underline);
            }
            Primitive::MonochromeSprite(sprite) => {
                sprite.order = order;
                self.monochrome_sprites.push(*sprite);
            }
            Primitive::SubpixelSprite(sprite) => {
                sprite.order = order;
                self.subpixel_sprites.push(*sprite);
            }
            Primitive::PolychromeSprite(sprite) => {
                sprite.order = order;
                self.polychrome_sprites.push(*sprite);
            }
            Primitive::Surface(surface) => {
                surface.order = order;
                self.surfaces.push(surface.clone());
            }
        }
        self.paint_operations
            .push(PaintOperation::Primitive(primitive));
    }

    pub fn insert_gradient_sprite(&mut self, sprite: MonochromeSprite, gradient: GlyphGradient) {
        self.insert_scrolling_gradient_sprite(sprite, gradient, GlyphGradientScroll::default());
    }

    pub fn insert_scrolling_gradient_sprite(
        &mut self,
        sprite: MonochromeSprite,
        gradient: GlyphGradient,
        scroll: GlyphGradientScroll,
    ) {
        self.insert_animated_gradient_sprite(sprite, gradient, scroll, None);
    }

    // PHOENIX PATCH (text keyframes): a gradient sprite with a keyframe animation applied at
    // present time by `glyph_sprite_now`, see `crate::GlyphAnimation`.
    pub fn insert_animated_gradient_sprite(
        &mut self,
        mut sprite: MonochromeSprite,
        gradient: GlyphGradient,
        scroll: GlyphGradientScroll,
        animation: Option<std::sync::Arc<crate::GlyphAnimation>>,
    ) {
        self.glyph_gradients.push(gradient);
        self.glyph_gradient_scrolls.push(scroll);
        self.glyph_animations.push(animation);
        sprite.pad = self.glyph_gradients.len() as u32;
        self.insert_primitive(sprite);
    }

    pub fn insert_sequence_sprite(&mut self, mut sprite: PolychromeSprite, sequence: ImageSequence) {
        self.sprite_sequences.push(sequence);
        sprite.pad = poly_with_sequence(sprite.pad, self.sprite_sequences.len() as u32);
        self.insert_primitive(sprite);
    }

    pub fn sprite_sequence(&self, sprite: &PolychromeSprite) -> Option<&ImageSequence> {
        poly_sequence(sprite.pad)
            .checked_sub(1)
            .and_then(|index| self.sprite_sequences.get(index as usize))
    }

    pub fn polychrome_sprite_now(&self, sprite: &PolychromeSprite) -> PolychromeSprite {
        let mut sprite = *sprite;
        if let Some(sequence) = self.sprite_sequence(&sprite) {
            sequence.apply(&mut sprite, self.gradient_time);
        }
        sprite
    }

    pub fn plays_sequences(&self, now: f64) -> bool {
        self.sprite_sequences.iter().any(|s| s.running(now))
    }

    pub fn glyph_animation(&self, sprite: &MonochromeSprite) -> Option<&std::sync::Arc<crate::GlyphAnimation>> {
        sprite
            .pad
            .checked_sub(1)
            .and_then(|index| self.glyph_animations.get(index as usize))
            .and_then(Option::as_ref)
    }

    pub fn glyph_sprite_now(&self, sprite: &MonochromeSprite) -> Option<(MonochromeSprite, GlyphGradient)> {
        let mut gradient = self.glyph_gradient_now(sprite)?;
        let mut sprite = *sprite;
        if let Some(animation) = self.glyph_animation(&sprite) {
            animation.apply(&mut sprite, &mut gradient, self.gradient_time);
        }
        Some((sprite, gradient))
    }

    pub fn animates_glyphs(&self, now: f64) -> bool {
        self.glyph_animations
            .iter()
            .flatten()
            .any(|a| a.running(now))
    }

    pub fn glyph_gradient_scroll(&self, sprite: &MonochromeSprite) -> GlyphGradientScroll {
        sprite
            .pad
            .checked_sub(1)
            .and_then(|index| self.glyph_gradient_scrolls.get(index as usize))
            .copied()
            .unwrap_or_default()
    }

    pub fn glyph_gradient_now(&self, sprite: &MonochromeSprite) -> Option<GlyphGradient> {
        let gradient = *self.glyph_gradient(sprite)?;
        Some(
            self.glyph_gradient_scroll(sprite)
                .apply(gradient, self.gradient_time),
        )
    }

    pub fn scrolls_gradients(&self, now: f64) -> bool {
        self.glyph_gradient_scrolls.iter().any(|s| s.running(now))
            || self.animates_glyphs(now)
            || self.plays_sequences(now)
    }

    pub fn glyph_gradient(&self, sprite: &MonochromeSprite) -> Option<&GlyphGradient> {
        sprite
            .pad
            .checked_sub(1)
            .and_then(|index| self.glyph_gradients.get(index as usize))
    }

    pub fn replay(&mut self, range: Range<usize>, prev_scene: &Scene) {
        for operation in &prev_scene.paint_operations[range] {
            match operation {
                PaintOperation::Primitive(Primitive::MonochromeSprite(sprite)) if sprite.pad != 0 => {
                    match prev_scene.glyph_gradient(sprite) {
                        Some(gradient) => self.insert_animated_gradient_sprite(
                            *sprite,
                            *gradient,
                            prev_scene.glyph_gradient_scroll(sprite),
                            prev_scene.glyph_animation(sprite).cloned(),
                        ),
                        None => self.insert_primitive(MonochromeSprite { pad: 0, ..*sprite }),
                    }
                }
                PaintOperation::Primitive(Primitive::Quad(quad)) if quad.background.pad != 0 => {
                    let mut quad = *quad;
                    quad.background.pad = prev_scene
                        .cutout_set(quad.background.pad)
                        .map_or(0, |set| self.push_cutouts(*set));
                    self.insert_primitive(quad);
                }
                PaintOperation::Primitive(Primitive::PolychromeSprite(sprite)) if sprite.pad != 0 => {
                    let cutout = prev_scene
                        .cutout_set(poly_cutout(sprite.pad))
                        .map_or(0, |set| self.push_cutouts(*set));
                    let plain = PolychromeSprite { pad: poly_with_cutout(0, cutout), ..*sprite };
                    match prev_scene.sprite_sequence(sprite) {
                        Some(sequence) => self.insert_sequence_sprite(plain, *sequence),
                        None => self.insert_primitive(plain),
                    }
                }
                PaintOperation::Primitive(primitive) => self.insert_primitive(primitive.clone()),
                PaintOperation::StartLayer(bounds) => self.push_layer(*bounds),
                PaintOperation::EndLayer => self.pop_layer(),
            }
        }
    }

    pub fn finish(&mut self) {
        self.shadows.sort_by_key(|shadow| shadow.order);
        self.quads.sort_by_key(|quad| quad.order);
        self.paths.sort_by_key(|path| path.order);
        self.underlines.sort_by_key(|underline| underline.order);
        self.monochrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.texture_id.index));
        self.subpixel_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.texture_id.index));
        self.polychrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.texture_id.index));
        self.surfaces.sort_by_key(|surface| surface.order);
    }

    #[cfg_attr(
        all(
            any(target_os = "linux", target_os = "freebsd"),
            not(any(feature = "x11", feature = "wayland"))
        ),
        allow(dead_code)
    )]
    pub fn batches(&self) -> impl Iterator<Item = PrimitiveBatch> + '_ {
        BatchIterator {
            shadows_start: 0,
            shadows_iter: self.shadows.iter().peekable(),
            quads_start: 0,
            quads_iter: self.quads.iter().peekable(),
            paths_start: 0,
            paths_iter: self.paths.iter().peekable(),
            underlines_start: 0,
            underlines_iter: self.underlines.iter().peekable(),
            monochrome_sprites_start: 0,
            monochrome_sprites_iter: self.monochrome_sprites.iter().peekable(),
            subpixel_sprites_start: 0,
            subpixel_sprites_iter: self.subpixel_sprites.iter().peekable(),
            polychrome_sprites_start: 0,
            polychrome_sprites_iter: self.polychrome_sprites.iter().peekable(),
            surfaces_start: 0,
            surfaces_iter: self.surfaces.iter().peekable(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Default)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
pub(crate) enum PrimitiveKind {
    Shadow,
    #[default]
    Quad,
    Path,
    Underline,
    MonochromeSprite,
    SubpixelSprite,
    PolychromeSprite,
    Surface,
}

pub(crate) enum PaintOperation {
    Primitive(Primitive),
    StartLayer(Bounds<ScaledPixels>),
    EndLayer,
}

#[derive(Clone)]
#[expect(missing_docs)]
pub enum Primitive {
    Shadow(Shadow),
    Quad(Quad),
    Path(Path<ScaledPixels>),
    Underline(Underline),
    MonochromeSprite(MonochromeSprite),
    SubpixelSprite(SubpixelSprite),
    PolychromeSprite(PolychromeSprite),
    Surface(PaintSurface),
}

#[expect(missing_docs)]
impl Primitive {
    pub fn bounds(&self) -> &Bounds<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.bounds,
            Primitive::Quad(quad) => &quad.bounds,
            Primitive::Path(path) => &path.bounds,
            Primitive::Underline(underline) => &underline.bounds,
            Primitive::MonochromeSprite(sprite) => &sprite.bounds,
            Primitive::SubpixelSprite(sprite) => &sprite.bounds,
            Primitive::PolychromeSprite(sprite) => &sprite.bounds,
            Primitive::Surface(surface) => &surface.bounds,
        }
    }

    pub fn content_mask(&self) -> &ContentMask<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.content_mask,
            Primitive::Quad(quad) => &quad.content_mask,
            Primitive::Path(path) => &path.content_mask,
            Primitive::Underline(underline) => &underline.content_mask,
            Primitive::MonochromeSprite(sprite) => &sprite.content_mask,
            Primitive::SubpixelSprite(sprite) => &sprite.content_mask,
            Primitive::PolychromeSprite(sprite) => &sprite.content_mask,
            Primitive::Surface(surface) => &surface.content_mask,
        }
    }
}

#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
struct BatchIterator<'a> {
    shadows_start: usize,
    shadows_iter: Peekable<slice::Iter<'a, Shadow>>,
    quads_start: usize,
    quads_iter: Peekable<slice::Iter<'a, Quad>>,
    paths_start: usize,
    paths_iter: Peekable<slice::Iter<'a, Path<ScaledPixels>>>,
    underlines_start: usize,
    underlines_iter: Peekable<slice::Iter<'a, Underline>>,
    monochrome_sprites_start: usize,
    monochrome_sprites_iter: Peekable<slice::Iter<'a, MonochromeSprite>>,
    subpixel_sprites_start: usize,
    subpixel_sprites_iter: Peekable<slice::Iter<'a, SubpixelSprite>>,
    polychrome_sprites_start: usize,
    polychrome_sprites_iter: Peekable<slice::Iter<'a, PolychromeSprite>>,
    surfaces_start: usize,
    surfaces_iter: Peekable<slice::Iter<'a, PaintSurface>>,
}

impl<'a> Iterator for BatchIterator<'a> {
    type Item = PrimitiveBatch;

    fn next(&mut self) -> Option<Self::Item> {
        let mut orders_and_kinds = [
            (
                self.shadows_iter.peek().map(|s| s.order),
                PrimitiveKind::Shadow,
            ),
            (self.quads_iter.peek().map(|q| q.order), PrimitiveKind::Quad),
            (self.paths_iter.peek().map(|q| q.order), PrimitiveKind::Path),
            (
                self.underlines_iter.peek().map(|u| u.order),
                PrimitiveKind::Underline,
            ),
            (
                self.monochrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::MonochromeSprite,
            ),
            (
                self.subpixel_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::SubpixelSprite,
            ),
            (
                self.polychrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::PolychromeSprite,
            ),
            (
                self.surfaces_iter.peek().map(|s| s.order),
                PrimitiveKind::Surface,
            ),
        ];
        orders_and_kinds.sort_by_key(|(order, kind)| (order.unwrap_or(u32::MAX), *kind));

        let first = orders_and_kinds[0];
        let second = orders_and_kinds[1];
        let (batch_kind, max_order_and_kind) = if first.0.is_some() {
            (first.1, (second.0.unwrap_or(u32::MAX), second.1))
        } else {
            return None;
        };

        match batch_kind {
            PrimitiveKind::Shadow => {
                let shadows_start = self.shadows_start;
                let mut shadows_end = shadows_start + 1;
                self.shadows_iter.next();
                while self
                    .shadows_iter
                    .next_if(|shadow| (shadow.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    shadows_end += 1;
                }
                self.shadows_start = shadows_end;
                Some(PrimitiveBatch::Shadows(shadows_start..shadows_end))
            }
            PrimitiveKind::Quad => {
                let quads_start = self.quads_start;
                let mut quads_end = quads_start + 1;
                self.quads_iter.next();
                while self
                    .quads_iter
                    .next_if(|quad| (quad.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    quads_end += 1;
                }
                self.quads_start = quads_end;
                Some(PrimitiveBatch::Quads(quads_start..quads_end))
            }
            PrimitiveKind::Path => {
                let paths_start = self.paths_start;
                let mut paths_end = paths_start + 1;
                self.paths_iter.next();
                while self
                    .paths_iter
                    .next_if(|path| (path.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    paths_end += 1;
                }
                self.paths_start = paths_end;
                Some(PrimitiveBatch::Paths(paths_start..paths_end))
            }
            PrimitiveKind::Underline => {
                let underlines_start = self.underlines_start;
                let mut underlines_end = underlines_start + 1;
                self.underlines_iter.next();
                while self
                    .underlines_iter
                    .next_if(|underline| (underline.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    underlines_end += 1;
                }
                self.underlines_start = underlines_end;
                Some(PrimitiveBatch::Underlines(underlines_start..underlines_end))
            }
            PrimitiveKind::MonochromeSprite => {
                let texture_id = self.monochrome_sprites_iter.peek().unwrap().tile.texture_id;
                let sprites_start = self.monochrome_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.monochrome_sprites_iter.next();
                while self
                    .monochrome_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.monochrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::SubpixelSprite => {
                let texture_id = self.subpixel_sprites_iter.peek().unwrap().tile.texture_id;
                let sprites_start = self.subpixel_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.subpixel_sprites_iter.next();
                while self
                    .subpixel_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.subpixel_sprites_start = sprites_end;
                Some(PrimitiveBatch::SubpixelSprites {
                    texture_id,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::PolychromeSprite => {
                let texture_id = self.polychrome_sprites_iter.peek().unwrap().tile.texture_id;
                let sprites_start = self.polychrome_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.polychrome_sprites_iter.next();
                while self
                    .polychrome_sprites_iter
                    .next_if(|sprite| {
                        (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.polychrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::Surface => {
                let surfaces_start = self.surfaces_start;
                let mut surfaces_end = surfaces_start + 1;
                self.surfaces_iter.next();
                while self
                    .surfaces_iter
                    .next_if(|surface| (surface.order, batch_kind) < max_order_and_kind)
                    .is_some()
                {
                    surfaces_end += 1;
                }
                self.surfaces_start = surfaces_end;
                Some(PrimitiveBatch::Surfaces(surfaces_start..surfaces_end))
            }
        }
    }
}

#[derive(Debug)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
#[allow(missing_docs)]
pub enum PrimitiveBatch {
    Shadows(Range<usize>),
    Quads(Range<usize>),
    Paths(Range<usize>),
    Underlines(Range<usize>),
    MonochromeSprites {
        texture_id: AtlasTextureId,
        range: Range<usize>,
    },
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    SubpixelSprites {
        texture_id: AtlasTextureId,
        range: Range<usize>,
    },
    PolychromeSprites {
        texture_id: AtlasTextureId,
        range: Range<usize>,
    },
    Surfaces(Range<usize>),
}

impl PrimitiveBatch {
    #[expect(missing_docs)]
    pub fn label(&self) -> String {
        match self {
            Self::Shadows(range) => format!("shadows ({})", range.len()),
            Self::Quads(range) => format!("quads ({})", range.len()),
            Self::Paths(range) => format!("paths ({})", range.len()),
            Self::Underlines(range) => format!("underlines ({})", range.len()),
            Self::MonochromeSprites { texture_id, range } => {
                format!(
                    "monochrome sprites ({}) on atlas {}",
                    range.len(),
                    texture_id.index
                )
            }
            Self::SubpixelSprites { texture_id, range } => {
                format!(
                    "subpixel sprites ({}) on atlas {}",
                    range.len(),
                    texture_id.index
                )
            }
            Self::PolychromeSprites { texture_id, range } => {
                format!(
                    "polychrome sprites ({}) on atlas {}",
                    range.len(),
                    texture_id.index
                )
            }
            Self::Surfaces(range) => format!("surfaces ({})", range.len()),
        }
    }
}

#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct Quad {
    pub order: DrawOrder,
    pub border_style: BorderStyle,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub background: Background,
    pub border_color: Hsla,
    pub corner_radii: Corners<ScaledPixels>,
    pub border_widths: Edges<ScaledPixels>,
}

impl From<Quad> for Primitive {
    fn from(quad: Quad) -> Self {
        Primitive::Quad(quad)
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct Underline {
    pub order: DrawOrder,
    pub pad: u32, // align to 8 bytes
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub thickness: ScaledPixels,
    pub wavy: PaddedBool32,
}

impl From<Underline> for Primitive {
    fn from(underline: Underline) -> Self {
        Primitive::Underline(underline)
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct Shadow {
    pub order: DrawOrder,
    pub blur_radius: ScaledPixels,
    pub bounds: Bounds<ScaledPixels>,
    pub corner_radii: Corners<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub element_bounds: Bounds<ScaledPixels>,
    pub element_corner_radii: Corners<ScaledPixels>,
    /// 0 = drop shadow (rendered outside the element), 1 = inset shadow (rendered inside).
    pub inset: u32,
    pub pad: u32, // align to 8 bytes
}

impl From<Shadow> for Primitive {
    fn from(shadow: Shadow) -> Self {
        Primitive::Shadow(shadow)
    }
}

/// The style of a border.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub enum BorderStyle {
    /// A solid border.
    #[default]
    Solid = 0,
    /// A dashed border.
    Dashed = 1,
}

/// A data type representing a 2 dimensional transformation that can be applied to an element.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct TransformationMatrix {
    /// 2x2 matrix containing rotation and scale,
    /// stored row-major
    pub rotation_scale: [[f32; 2]; 2],
    /// translation vector
    pub translation: [f32; 2],
}

impl Eq for TransformationMatrix {}

impl TransformationMatrix {
    /// The unit matrix, has no effect.
    pub fn unit() -> Self {
        Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [0.0, 0.0],
        }
    }

    /// Move the origin by a given point
    pub fn translate(self, point: Point<ScaledPixels>) -> Self {
        self.compose(Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [point.x.0, point.y.0],
        })
    }

    /// Clockwise rotation in radians around the origin
    pub fn rotate(self, angle: Radians) -> Self {
        self.compose(Self {
            rotation_scale: [
                [angle.0.cos(), -angle.0.sin()],
                [angle.0.sin(), angle.0.cos()],
            ],
            translation: [0.0, 0.0],
        })
    }

    /// Scale around the origin
    pub fn scale(self, size: Size<f32>) -> Self {
        self.compose(Self {
            rotation_scale: [[size.width, 0.0], [0.0, size.height]],
            translation: [0.0, 0.0],
        })
    }

    /// Perform matrix multiplication with another transformation
    /// to produce a new transformation that is the result of
    /// applying both transformations: first, `other`, then `self`.
    #[inline]
    pub fn compose(self, other: TransformationMatrix) -> TransformationMatrix {
        if other == Self::unit() {
            return self;
        }
        // Perform matrix multiplication
        TransformationMatrix {
            rotation_scale: [
                [
                    self.rotation_scale[0][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][0],
                    self.rotation_scale[0][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][1],
                ],
                [
                    self.rotation_scale[1][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][0],
                    self.rotation_scale[1][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][1],
                ],
            ],
            translation: [
                self.translation[0]
                    + self.rotation_scale[0][0] * other.translation[0]
                    + self.rotation_scale[0][1] * other.translation[1],
                self.translation[1]
                    + self.rotation_scale[1][0] * other.translation[0]
                    + self.rotation_scale[1][1] * other.translation[1],
            ],
        }
    }

    /// Apply transformation to a point, mainly useful for debugging
    pub fn apply(&self, point: Point<Pixels>) -> Point<Pixels> {
        let input = [point.x.0, point.y.0];
        let mut output = self.translation;
        for (i, output_cell) in output.iter_mut().enumerate() {
            for (k, input_cell) in input.iter().enumerate() {
                *output_cell += self.rotation_scale[i][k] * *input_cell;
            }
        }
        Point::new(output[0].into(), output[1].into())
    }
}

impl Default for TransformationMatrix {
    fn default() -> Self {
        Self::unit()
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct MonochromeSprite {
    pub order: DrawOrder,
    pub pad: u32,
    pub tile: AtlasTile,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub transformation: TransformationMatrix,
}

// PHOENIX PATCH (text gradient): per-pixel gradient fill of monochrome glyphs, see
// docs/gpui-patches.md. A sprite with `pad != 0` reads `Scene::glyph_gradients[pad - 1]`.
/// Maximum number of colour stops of a glyph gradient.
pub const MAX_GLYPH_GRADIENT_STOPS: usize = 12;

#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C)]
#[expect(missing_docs)]
pub struct GlyphGradientStop {
    pub color: [f32; 4],
    pub position: f32,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C)]
#[expect(missing_docs)]
pub struct GlyphGradient {
    pub origin: [f32; 2],
    pub step: [f32; 2],
    pub offset: f32,
    pub period: f32,
    pub stop_count: u32,
    pub opacity: f32,
    pub stops: [GlyphGradientStop; MAX_GLYPH_GRADIENT_STOPS],
}

// PHOENIX PATCH (gradient scroll): `background-position-x` animation of a glyph gradient,
// in device pixels per second of `crate::gradient_clock`, applied by `Scene::glyph_gradient_now`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[expect(missing_docs)]
pub struct GlyphGradientScroll {
    pub speed: f32,
    pub start: f64,
    pub duration: f32,
}

impl GlyphGradientScroll {
    /// Whether the animation still moves at `now`.
    pub fn running(&self, now: f64) -> bool {
        self.speed != 0.0 && now - self.start < self.duration as f64
    }

    /// Horizontal shift of the gradient at `now`, wrapped by a repeat `period`.
    pub fn displacement(&self, now: f64, period: f32) -> f32 {
        if self.speed == 0.0 {
            return 0.0;
        }
        let elapsed = (now - self.start).clamp(0.0, self.duration as f64);
        let shift = self.speed as f64 * elapsed;
        if period > 0.0 {
            shift.rem_euclid(period as f64) as f32
        } else {
            shift as f32
        }
    }

    /// The gradient with its tile moved to where the animation is at `now`.
    pub fn apply(&self, mut gradient: GlyphGradient, now: f64) -> GlyphGradient {
        gradient.origin[0] += self.displacement(now, gradient.period);
        gradient
    }
}

impl GlyphGradient {
    /// Gradient position at a device pixel, as the glyph shader computes it.
    pub fn position_at(&self, x: f32, y: f32) -> f32 {
        let mut local_x = x - self.origin[0];
        let local_y = y - self.origin[1];
        if self.period > 0.0 {
            local_x -= self.period * (local_x / self.period).floor();
        }
        local_x * self.step[0] + local_y * self.step[1] + self.offset
    }

    /// Colour at a gradient position, as the glyph shader computes it.
    pub fn color_at_position(&self, t: f32) -> [f32; 4] {
        let count = (self.stop_count as usize).clamp(1, MAX_GLYPH_GRADIENT_STOPS);
        let stops = &self.stops[..count];
        let mut color = stops[0].color;
        for pair in stops.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if t >= b.position {
                color = b.color;
            } else if t > a.position {
                let f = (t - a.position) / (b.position - a.position).max(f32::EPSILON);
                for ((c, x), y) in color.iter_mut().zip(a.color).zip(b.color) {
                    *c = x + (y - x) * f;
                }
                break;
            } else {
                break;
            }
        }
        color[3] *= self.opacity;
        color
    }
}

impl From<MonochromeSprite> for Primitive {
    fn from(sprite: MonochromeSprite) -> Self {
        Primitive::MonochromeSprite(sprite)
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct SubpixelSprite {
    pub order: DrawOrder,
    pub pad: u32, // align to 8 bytes
    pub tile: AtlasTile,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub transformation: TransformationMatrix,
}

impl From<SubpixelSprite> for Primitive {
    fn from(sprite: SubpixelSprite) -> Self {
        Primitive::SubpixelSprite(sprite)
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PolychromeSprite {
    pub order: DrawOrder,
    pub pad: u32,
    pub tile: AtlasTile,
    pub grayscale: PaddedBool32,
    pub opacity: f32,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub corner_radii: Corners<ScaledPixels>,
}

impl From<PolychromeSprite> for Primitive {
    fn from(sprite: PolychromeSprite) -> Self {
        Primitive::PolychromeSprite(sprite)
    }
}

#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct PaintSurface {
    pub order: DrawOrder,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub image_buffer: objc2_core_foundation::CFRetained<objc2_core_video::CVPixelBuffer>,
}

impl From<PaintSurface> for Primitive {
    fn from(surface: PaintSurface) -> Self {
        Primitive::Surface(surface)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[expect(missing_docs)]
pub struct PathId(pub usize);

/// A line made up of a series of vertices and control points.
#[derive(Clone, Debug)]
#[expect(missing_docs)]
pub struct Path<P: Clone + Debug + Default + PartialEq> {
    pub id: PathId,
    pub order: DrawOrder,
    pub bounds: Bounds<P>,
    pub content_mask: ContentMask<P>,
    pub vertices: Vec<PathVertex<P>>,
    pub color: Background,
    start: Point<P>,
    current: Point<P>,
    contour_count: usize,
}

impl Path<Pixels> {
    /// Create a new path with the given starting point.
    pub fn new(start: Point<Pixels>) -> Self {
        Self {
            id: PathId(0),
            order: DrawOrder::default(),
            vertices: Vec::new(),
            start,
            current: start,
            bounds: Bounds {
                origin: start,
                size: Default::default(),
            },
            content_mask: Default::default(),
            color: Default::default(),
            contour_count: 0,
        }
    }

    /// Scale this path by the given factor.
    pub fn scale(&self, factor: f32) -> Path<ScaledPixels> {
        Path {
            id: self.id,
            order: self.order,
            bounds: self.bounds.scale(factor),
            content_mask: self.content_mask.scale(factor),
            vertices: self
                .vertices
                .iter()
                .map(|vertex| vertex.scale(factor))
                .collect(),
            start: self.start.map(|start| start.scale(factor)),
            current: self.current.scale(factor),
            contour_count: self.contour_count,
            color: self.color,
        }
    }

    /// Move the start, current point to the given point.
    pub fn move_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        self.start = to;
        self.current = to;
    }

    /// Draw a straight line from the current point to the given point.
    pub fn line_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }
        self.current = to;
    }

    /// Draw a curve from the current point to the given point, using the given control point.
    pub fn curve_to(&mut self, to: Point<Pixels>, ctrl: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }

        self.push_triangle(
            (self.current, ctrl, to),
            (point(0., 0.), point(0.5, 0.), point(1., 1.)),
        );
        self.current = to;
    }

    /// Push a triangle to the Path.
    pub fn push_triangle(
        &mut self,
        xy: (Point<Pixels>, Point<Pixels>, Point<Pixels>),
        st: (Point<f32>, Point<f32>, Point<f32>),
    ) {
        self.bounds = self
            .bounds
            .union(&Bounds {
                origin: xy.0,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.1,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.2,
                size: Default::default(),
            });

        self.vertices.push(PathVertex {
            xy_position: xy.0,
            st_position: st.0,
            content_mask: Default::default(),
        });
        self.vertices.push(PathVertex {
            xy_position: xy.1,
            st_position: st.1,
            content_mask: Default::default(),
        });
        self.vertices.push(PathVertex {
            xy_position: xy.2,
            st_position: st.2,
            content_mask: Default::default(),
        });
    }
}

impl<T> Path<T>
where
    T: Clone + Debug + Default + PartialEq + PartialOrd + Add<T, Output = T> + Sub<Output = T>,
{
    #[allow(unused)]
    #[expect(missing_docs)]
    pub fn clipped_bounds(&self) -> Bounds<T> {
        self.bounds.intersect(&self.content_mask.bounds)
    }
}

impl From<Path<ScaledPixels>> for Primitive {
    fn from(path: Path<ScaledPixels>) -> Self {
        Primitive::Path(path)
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PathVertex<P: Clone + Debug + Default + PartialEq> {
    pub xy_position: Point<P>,
    pub st_position: Point<f32>,
    pub content_mask: ContentMask<P>,
}

#[expect(missing_docs)]
impl PathVertex<Pixels> {
    pub fn scale(&self, factor: f32) -> PathVertex<ScaledPixels> {
        PathVertex {
            xy_position: self.xy_position.scale(factor),
            st_position: self.st_position,
            content_mask: self.content_mask.scale(factor),
        }
    }
}

#[cfg(test)]
mod image_sequence_tests {
    use super::*;
    use crate::{DevicePixels, size};

    fn sequence() -> ImageSequence {
        ImageSequence {
            start: 10.0,
            fps: 60.0,
            phase: 0.5,
            count: 40,
            columns: 8,
            cell: size(DevicePixels(36), DevicePixels(36)),
        }
    }

    #[test]
    fn index_rounds_the_elapsed_frames_and_holds_the_last_one() {
        let s = sequence();
        assert_eq!(s.index(9.0), 0);
        assert_eq!(s.index(10.0), 0);
        assert_eq!(s.index(10.0 + 0.6 / 60.0), 1);
        assert_eq!(s.index(10.5), 30);
        assert_eq!(s.index(11.0), 39);
        assert_eq!(s.index(100.0), 39);
        assert!(s.running(10.5));
        assert!(!s.running(11.0));
    }

    #[test]
    fn apply_moves_the_tile_to_the_cell_of_the_frame() {
        let s = sequence();
        let mut scene = Scene::default();
        let mut sprite = PolychromeSprite {
            order: 0,
            pad: 0,
            tile: AtlasTile {
                texture_id: AtlasTextureId {
                    index: 0,
                    kind: crate::AtlasTextureKind::Polychrome,
                },
                tile_id: crate::TileId(1),
                padding: 0,
                bounds: Bounds {
                    origin: point(DevicePixels(100), DevicePixels(200)),
                    size: size(DevicePixels(36), DevicePixels(36)),
                },
            },
            grayscale: false.into(),
            opacity: 1.0,
            bounds: Default::default(),
            content_mask: Default::default(),
            corner_radii: Default::default(),
        };
        sprite.bounds.size = size(ScaledPixels(36.0), ScaledPixels(36.0));
        sprite.content_mask.bounds.size = size(ScaledPixels(100.0), ScaledPixels(100.0));
        scene.insert_sequence_sprite(sprite, s);
        scene.gradient_time = 10.0 + 10.0 / 60.0;
        let now = scene.polychrome_sprite_now(&scene.polychrome_sprites[0]);
        assert_eq!(now.tile.bounds.origin, point(DevicePixels(100 + 36 * 2), DevicePixels(200 + 36)));
        assert_eq!(now.tile.bounds.size, size(DevicePixels(36), DevicePixels(36)));
        assert!(scene.scrolls_gradients(10.2));
        assert!(!scene.scrolls_gradients(12.0));
        let mut replayed = Scene::default();
        replayed.replay(0..scene.paint_operations.len(), &scene);
        assert_eq!(replayed.sprite_sequences, vec![s]);
    }

    #[test]
    fn sequence_and_cutout_share_the_pad_word() {
        let pad = poly_with_cutout(poly_with_sequence(0, 3), 7);
        assert_eq!((poly_sequence(pad), poly_cutout(pad)), (3, 7));
        let pad = poly_with_sequence(pad, 9);
        assert_eq!((poly_sequence(pad), poly_cutout(pad)), (9, 7));
        assert_eq!(poly_cutout(poly_with_cutout(pad, 0)), 0);
    }
}

#[cfg(test)]
mod cutout_tests {
    use super::*;

    fn hole(x: f32, y: f32, w: f32, h: f32, r: f32) -> CutoutHole {
        CutoutHole { origin: [x, y], size: [w, h], radius: r, pad: [0.; 3] }
    }

    #[test]
    fn round_hole_distance_is_the_circle_distance() {
        let h = hole(22., -2., 20., 20., 10.);
        assert!((h.distance([32., 8.]) + 10.).abs() < 1e-4);
        assert!((h.distance([32., 20.]) - 2.).abs() < 1e-4);
        assert!((h.distance([22., 8.])).abs() < 1e-4);
        assert_eq!(h.coverage([32., 8.]), 0.);
        assert_eq!(h.coverage([0., 0.]), 1.);
        assert!((h.coverage([22., 8.]) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn pill_hole_keeps_its_straight_sides() {
        let h = hole(10., 22., 32., 20., 10.);
        assert!((h.distance([26., 22.])).abs() < 1e-4);
        assert!((h.distance([26., 32.]) + 10.).abs() < 1e-4);
        assert!((h.distance([10., 32.])).abs() < 1e-4);
    }

    #[test]
    fn empty_hole_cuts_nothing() {
        let set = CutoutSet { holes: [hole(0., 0., 10., 10., 5.), CutoutHole::default()] };
        assert_eq!(set.coverage([5., 5.]), 0.);
        assert_eq!(set.coverage([20., 20.]), 1.);
    }

    #[test]
    fn push_reuses_the_last_set_and_stops_when_full() {
        let mut scene = Scene::default();
        let a = CutoutSet { holes: [hole(0., 0., 4., 4., 2.), CutoutHole::default()] };
        let b = CutoutSet { holes: [hole(8., 0., 4., 4., 2.), CutoutHole::default()] };
        assert_eq!(scene.push_cutouts(a), 1);
        assert_eq!(scene.push_cutouts(a), 1);
        assert_eq!(scene.push_cutouts(b), 2);
        assert_eq!(scene.cutout_set(2), Some(&b));
        assert_eq!(scene.cutout_set(0), None);
        for i in 0..MAX_CUTOUT_SETS * 2 {
            let mut s = a;
            s.holes[0].origin[0] = i as f32 + 100.;
            scene.push_cutouts(s);
        }
        assert_eq!(scene.cutouts.len(), MAX_CUTOUT_SETS);
        let mut s = a;
        s.holes[0].origin[0] = -1.;
        assert_eq!(scene.push_cutouts(s), 0);
        scene.clear();
        assert!(scene.cutouts.is_empty());
    }
}

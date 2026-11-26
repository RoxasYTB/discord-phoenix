// PHOENIX PATCH (text gradient): monochrome glyphs filled with a linear gradient sampled
// per pixel (CSS `background-clip: text`). Storage-buffer backends only; see
// docs/gpui-patches.md (patch 16) in the Phoenix repository.

struct GlyphGradientStop {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    position: f32,
}

struct GlyphGradient {
    origin_x: f32,
    origin_y: f32,
    step_x: f32,
    step_y: f32,
    offset: f32,
    period: f32,
    stop_count: u32,
    opacity: f32,
    stops: array<GlyphGradientStop, 8>,
}

struct GradientSprite {
    sprite: MonochromeSprite,
    gradient: GlyphGradient,
}

@group(1) @binding(0) var<storage, read> b_gradient_sprites: array<GradientSprite>;

struct GradientSpriteVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) tile_position: vec2<f32>,
    @location(1) scene_position: vec2<f32>,
    @location(2) @interpolate(flat) instance_id: u32,
    @location(3) clip_distances: vec4<f32>,
}

@vertex
fn vs_gradient_sprite(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> GradientSpriteVarying {
    let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));
    let sprite = b_gradient_sprites[instance_id].sprite;

    var out = GradientSpriteVarying();
    out.position = to_device_position_transformed(unit_vertex, sprite.bounds, sprite.transformation);
    let local = unit_vertex * vec2<f32>(sprite.bounds.size) + sprite.bounds.origin;
    out.scene_position = transpose(sprite.transformation.rotation_scale) * local + sprite.transformation.translation;
    out.tile_position = to_tile_position(unit_vertex, sprite.tile);
    out.instance_id = instance_id;
    out.clip_distances = distance_from_clip_rect_transformed(unit_vertex, sprite.bounds, sprite.content_mask, sprite.transformation);
    return out;
}

fn glyph_gradient_stop(instance_id: u32, index: u32) -> GlyphGradientStop {
    return b_gradient_sprites[instance_id].gradient.stops[index];
}

fn glyph_gradient_color(instance_id: u32, position: vec2<f32>) -> vec4<f32> {
    let origin_x = b_gradient_sprites[instance_id].gradient.origin_x;
    let origin_y = b_gradient_sprites[instance_id].gradient.origin_y;
    let step_x = b_gradient_sprites[instance_id].gradient.step_x;
    let step_y = b_gradient_sprites[instance_id].gradient.step_y;
    let offset = b_gradient_sprites[instance_id].gradient.offset;
    let period = b_gradient_sprites[instance_id].gradient.period;
    let opacity = b_gradient_sprites[instance_id].gradient.opacity;
    let count = clamp(b_gradient_sprites[instance_id].gradient.stop_count, 1u, 8u);

    var local = position - vec2<f32>(origin_x, origin_y);
    if (period > 0.0) {
        local.x = local.x - period * floor(local.x / period);
    }
    let t = local.x * step_x + local.y * step_y + offset;
    let first = glyph_gradient_stop(instance_id, 0u);
    var color = vec4<f32>(first.r, first.g, first.b, first.a);
    for (var i = 1u; i < count; i++) {
        let a = glyph_gradient_stop(instance_id, i - 1u);
        let b = glyph_gradient_stop(instance_id, i);
        let ca = vec4<f32>(a.r, a.g, a.b, a.a);
        let cb = vec4<f32>(b.r, b.g, b.b, b.a);
        if (t >= b.position) {
            color = cb;
        } else {
            if (t > a.position) {
                let f = (t - a.position) / max(b.position - a.position, 1e-6);
                color = mix(ca, cb, f);
            }
            break;
        }
    }
    return vec4<f32>(color.rgb, color.a * opacity);
}

@fragment
fn fs_gradient_sprite(input: GradientSpriteVarying) -> @location(0) vec4<f32> {
    let sample = textureSample(t_sprite, s_sprite, input.tile_position).r;
    let color = glyph_gradient_color(input.instance_id, input.scene_position);
    let alpha_corrected = apply_contrast_and_gamma_correction(sample, color.rgb, gamma_params.grayscale_enhanced_contrast, gamma_params.gamma_ratios);

    if (any(input.clip_distances < vec4<f32>(0.0))) {
        return vec4<f32>(0.0);
    }

    return blend_color(color, alpha_corrected);
}

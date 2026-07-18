struct Adjustments {
    exposure: f32,
    brightness: f32,
    contrast: f32,
    saturation: f32,
    vibrance: f32,
    temperature: f32,
    tint: f32,
    black_white_intensity: f32,
    black_white_tone: f32,
    black_white_neutrals: f32,
    grain: f32,
    black_point: f32,
    brilliance: f32,
    highlights: f32,
    shadows: f32,
    level_luminance_black: f32,
    level_luminance_mid: f32,
    level_luminance_white: f32,
    level_rgb_black: f32,
    level_rgb_mid: f32,
    level_rgb_white: f32,
    level_red_black: f32,
    level_red_mid: f32,
    level_red_white: f32,
    level_green_black: f32,
    level_green_mid: f32,
    level_green_white: f32,
    level_blue_black: f32,
    level_blue_mid: f32,
    level_blue_white: f32,
    definition_amount: f32,
    rotation_radians: f32,
    uv_min_x: f32,
    uv_min_y: f32,
    uv_span_x: f32,
    uv_span_y: f32,
    source_uv_min_x: f32,
    source_uv_min_y: f32,
    source_uv_span_x: f32,
    source_uv_span_y: f32,
};

@group(0) @binding(0)
var source_texture: texture_2d<f32>;

@group(0) @binding(1)
var source_sampler: sampler;

@group(0) @binding(2)
var<uniform> adjustments: Adjustments;

@group(0) @binding(3)
var curve_lut: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

fn luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn rotate_uv(uv: vec2<f32>, radians: f32) -> vec2<f32> {
    let centered = uv - vec2<f32>(0.5, 0.5);
    let s = sin(radians);
    let c = cos(radians);
    let rotated = vec2<f32>(
        centered.x * c - centered.y * s,
        centered.x * s + centered.y * c,
    );
    return rotated + vec2<f32>(0.5, 0.5);
}

fn curve_lut_uv(value: f32) -> vec2<f32> {
    let x = (clamp(value, 0.0, 1.0) * 255.0 + 0.5) / 256.0;
    return vec2<f32>(x, 0.5);
}

fn apply_curves(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        textureSampleLevel(curve_lut, source_sampler, curve_lut_uv(color.r), 0.0).r,
        textureSampleLevel(curve_lut, source_sampler, curve_lut_uv(color.g), 0.0).g,
        textureSampleLevel(curve_lut, source_sampler, curve_lut_uv(color.b), 0.0).b,
    );
}

fn apply_levels_value(value: f32, black_value: f32, mid_value: f32, white_value: f32) -> f32 {
    let black = clamp(black_value, 0.0, 0.98);
    let white = clamp(white_value, black + 0.01, 1.0);
    let mid = clamp(mid_value, 0.05, 0.95);
    let normalized = clamp((value - black) / (white - black), 0.0, 1.0);
    return pow(normalized, max(1.0 - mid, 0.05) / max(mid, 0.05));
}

fn apply_levels(color: vec3<f32>) -> vec3<f32> {
    let before_luminance = luma(color);
    let after_luminance = apply_levels_value(
        before_luminance,
        adjustments.level_luminance_black,
        adjustments.level_luminance_mid,
        adjustments.level_luminance_white,
    );

    var leveled = color;
    if (before_luminance > 0.000001) {
        leveled *= after_luminance / before_luminance;
    } else {
        leveled += vec3<f32>(after_luminance);
    }

    leveled = vec3<f32>(
        apply_levels_value(
            leveled.r,
            adjustments.level_rgb_black,
            adjustments.level_rgb_mid,
            adjustments.level_rgb_white,
        ),
        apply_levels_value(
            leveled.g,
            adjustments.level_rgb_black,
            adjustments.level_rgb_mid,
            adjustments.level_rgb_white,
        ),
        apply_levels_value(
            leveled.b,
            adjustments.level_rgb_black,
            adjustments.level_rgb_mid,
            adjustments.level_rgb_white,
        ),
    );

    return vec3<f32>(
        apply_levels_value(
            leveled.r,
            adjustments.level_red_black,
            adjustments.level_red_mid,
            adjustments.level_red_white,
        ),
        apply_levels_value(
            leveled.g,
            adjustments.level_green_black,
            adjustments.level_green_mid,
            adjustments.level_green_white,
        ),
        apply_levels_value(
            leveled.b,
            adjustments.level_blue_black,
            adjustments.level_blue_mid,
            adjustments.level_blue_white,
        ),
    );
}

fn hash_noise(pixel: vec2<u32>) -> f32 {
    var value = pixel.x * 0x045d9f3bu + pixel.y * 0x119de1f3u;
    value = value ^ (value >> 16u);
    value = value * 0x045d9f3bu;
    value = value ^ (value >> 16u);
    return f32(value) / 4294967295.0;
}

fn apply_grain(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let amount = clamp(adjustments.grain, 0.0, 1.0);
    if (amount <= 0.0) {
        return color;
    }

    let dimensions_u = textureDimensions(source_texture, 0);
    let dimensions = vec2<f32>(f32(dimensions_u.x), f32(dimensions_u.y));
    let pixel_f = min(
        floor(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * dimensions),
        dimensions - vec2<f32>(1.0),
    );
    let pixel = vec2<u32>(u32(pixel_f.x), u32(pixel_f.y));
    let noise = hash_noise(pixel) * 2.0 - 1.0;
    let delta = noise * amount * (28.0 / 255.0);
    return color + vec3<f32>(delta);
}

fn apply_adjustments(color_in: vec3<f32>) -> vec3<f32> {
    var color = color_in;
    color *= pow(2.0, adjustments.exposure);
    color += vec3<f32>(adjustments.brightness * 0.25);

    let contrast = 1.0 + adjustments.contrast * 0.6;
    color = (color - vec3<f32>(0.5)) * contrast + vec3<f32>(0.5);

    let luminance = luma(color);
    let shadow_weight = clamp(1.0 - luminance * 2.0, 0.0, 1.0);
    let highlight_weight = clamp((luminance - 0.5) * 2.0, 0.0, 1.0);
    let shadow_lift = adjustments.shadows * 0.35 * shadow_weight;
    let highlight_pull = adjustments.highlights * 0.35 * highlight_weight;
    color += vec3<f32>(shadow_lift - highlight_pull);

    let black_point = adjustments.black_point * 0.2;
    color = (color - vec3<f32>(black_point)) / max(1.0 - black_point, 0.01);

    let brilliant_luma = luma(color);
    let brilliance = adjustments.brilliance * 0.25;
    color += (color - vec3<f32>(brilliant_luma)) * brilliance + vec3<f32>(brilliance * 0.35);

    let temperature = adjustments.temperature * 0.12;
    let tint = adjustments.tint * 0.10;
    color.r += temperature - tint * 0.5;
    color.g += tint;
    color.b -= temperature + tint * 0.5;

    let sat_luma = luma(color);
    let max_channel = max(max(color.r, color.g), color.b);
    let min_channel = min(min(color.r, color.g), color.b);
    let saturation_distance = clamp(max_channel - min_channel, 0.0, 1.0);
    let sat_factor =
        1.0 + adjustments.saturation * 0.8 +
        adjustments.vibrance * (1.0 - saturation_distance) * 0.8;
    color = vec3<f32>(sat_luma) + (color - vec3<f32>(sat_luma)) * sat_factor;

    let bw_amount = clamp(adjustments.black_white_intensity, 0.0, 1.0);
    if (bw_amount > 0.0) {
        let bw = luma(color) + adjustments.black_white_neutrals * 0.12;
        let toned = vec3<f32>(
            bw + adjustments.black_white_tone * 0.12,
            bw,
            bw - adjustments.black_white_tone * 0.12,
        );
        color = mix(color, toned, bw_amount);
    }

    color = apply_curves(color);
    color = apply_levels(color);

    return color;
}

fn sample_adjusted(uv: vec2<f32>) -> vec3<f32> {
    let sample = textureSampleLevel(
        source_texture,
        source_sampler,
        clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)),
        0.0,
    );
    return apply_adjustments(sample.rgb);
}

fn apply_sharpen(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let amount = clamp(adjustments.definition_amount, 0.0, 1.0);
    if (amount <= 0.0) {
        return color;
    }

    let dimensions_u = textureDimensions(source_texture, 0);
    let texel = vec2<f32>(1.0) / vec2<f32>(f32(dimensions_u.x), f32(dimensions_u.y));
    let radius = 0.65 + amount * 0.85;

    let top_left = sample_adjusted(uv + texel * radius * vec2<f32>(-1.0, -1.0));
    let top = sample_adjusted(uv + texel * radius * vec2<f32>(0.0, -1.0));
    let top_right = sample_adjusted(uv + texel * radius * vec2<f32>(1.0, -1.0));
    let left = sample_adjusted(uv + texel * radius * vec2<f32>(-1.0, 0.0));
    let right = sample_adjusted(uv + texel * radius * vec2<f32>(1.0, 0.0));
    let bottom_left = sample_adjusted(uv + texel * radius * vec2<f32>(-1.0, 1.0));
    let bottom = sample_adjusted(uv + texel * radius * vec2<f32>(0.0, 1.0));
    let bottom_right = sample_adjusted(uv + texel * radius * vec2<f32>(1.0, 1.0));

    let blurred =
        (top_left + top_right + bottom_left + bottom_right) * 0.0625 +
        (top + left + right + bottom) * 0.125 +
        color * 0.25;
    let diff = color - blurred;
    let threshold = vec3<f32>(8.0 / 255.0);
    let sharpened = color + diff;

    return select(color, sharpened, abs(diff) > threshold);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let destination_uv = vec2<f32>(
        adjustments.uv_min_x + in.uv.x * adjustments.uv_span_x,
        adjustments.uv_min_y + in.uv.y * adjustments.uv_span_y,
    );
    let local_uv = rotate_uv(destination_uv, adjustments.rotation_radians);
    if (local_uv.x < 0.0 || local_uv.x > 1.0 || local_uv.y < 0.0 || local_uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let uv = vec2<f32>(
        adjustments.source_uv_min_x + local_uv.x * adjustments.source_uv_span_x,
        adjustments.source_uv_min_y + local_uv.y * adjustments.source_uv_span_y,
    );

    let sample = textureSample(source_texture, source_sampler, uv);
    var color = apply_adjustments(sample.rgb);
    color = apply_sharpen(color, uv);
    color = apply_grain(color, uv);

    let alpha = sample.a;
    let premultiplied_color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)) * alpha;
    return vec4<f32>(premultiplied_color, alpha);
}

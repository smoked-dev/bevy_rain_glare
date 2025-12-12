#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct RainGlareSettings {
    intensity: f32,
    threshold: f32,
    streak_length_px: f32,
    rain_density: f32,

    wind: vec2<f32>,
    speed: f32,
    time: f32,

    pattern_scale: f32,
    mask_thickness_px: f32,
    snap_to_pixel: f32,
    tail_quant_steps: f32,

    view_angle_factor: f32,
};

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: RainGlareSettings;

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn hash11(x: f32) -> f32 {
    return fract(sin(x) * 43758.5453123);
}

fn hash12(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn lens_rain_mask(
    uv: vec2<f32>,
    dims: vec2<f32>,
    dir: vec2<f32>,
    t: f32,
    density: f32,
    speed: f32,
    pattern_scale: f32,
    thickness_px: f32,
    tail_quant_steps: f32,
) -> f32 {
    let p = uv * dims;
    let perp = vec2<f32>(-dir.y, dir.x);

    let u = dot(p, perp);
    let v = dot(p, dir);

    // Smaller pattern => smaller spacing/period.
    let s = max(pattern_scale, 0.001);
    let spacing = 7.0 / s;
    let period  = 46.0 / s;

    let line_id = floor(u / spacing);

    // Randomize active lines based on density.
    let r = hash11(line_id * 12.9898 + 78.233);
    let line_active = step(r, clamp(density, 0.0, 1.0));

    // Distance to line center in pixels.
    let dist = abs(fract(u / spacing) - 0.5) * spacing;

    // HARD EDGE width (no smoothing). Clamp thickness so it can't exceed half the cell.
    let thick = min(max(thickness_px, 0.1), spacing * 0.49);
    let width = 1.0 - step(thick, dist); // 1 inside, 0 outside

    // Animate along direction.
    let phase = fract((v / period) + t * speed * 0.25 + r);

    // Tail shape (can be quantized for crunchy retro steps).
    var tail = exp(-phase * 6.0);
    if (tail_quant_steps >= 2.0) {
        let steps = tail_quant_steps;
        tail = floor(tail * steps) / steps;
    }

    return clamp(line_active * width * tail, 0.0, 1.0);
}

fn snap_uv_to_pixel_center(uv: vec2<f32>, dims: vec2<f32>) -> vec2<f32> {
    let px = floor(uv * dims) + vec2<f32>(0.5, 0.5);
    return px / dims;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(screen_texture, screen_sampler, in.uv);

    let dims_u = textureDimensions(screen_texture);
    let dims = vec2<f32>(f32(dims_u.x), f32(dims_u.y));

    let wind_len = length(settings.wind);
    let dir = select(vec2<f32>(0.0, 1.0), settings.wind / wind_len, wind_len > 1e-5);

    let rain = lens_rain_mask(
        in.uv, dims, dir,
        settings.time,
        settings.rain_density,
        settings.speed,
        settings.pattern_scale,
        settings.mask_thickness_px,
        settings.tail_quant_steps,
    );

    let samples: i32 = 16;
    let len_uv = settings.streak_length_px / max(dims.y, 1.0);
    let step_uv = dir * (len_uv / f32(samples));

    // Disable jitter when snapping (keeps the retro edges clean).
    let jitter_mask = 1.0 - step(0.5, settings.snap_to_pixel);
    let jitter = (hash12(in.uv * dims + vec2<f32>(settings.time, settings.time * 1.37)) - 0.5) * 0.9 * jitter_mask;

    let uv0 = in.uv + vec2<f32>(jitter / dims.x, 0.0);

    var accum = vec3<f32>(0.0);
    var wsum = 0.0;

    for (var i: i32 = 0; i < samples; i = i + 1) {
        let fi = f32(i);
        let suv = uv0 - step_uv * (fi + jitter);

        let use_snap = settings.snap_to_pixel >= 0.5;
        let uv_s = select(suv, snap_uv_to_pixel_center(suv, dims), use_snap);

        let c = textureSample(screen_texture, screen_sampler, uv_s).rgb;

        // Bright-pass weight
        let b = clamp((luma(c) - settings.threshold) / max(1.0 - settings.threshold, 1e-5), 0.0, 1.0);

        let w = b * exp(-fi * 0.16);
        accum += c * w;
        wsum += w;
    }

    let streak = accum / max(wsum, 1e-5);
//    let out_rgb = base.rgb + streak * (settings.intensity * rain);
    let angle_fade = settings.view_angle_factor;
    let out_rgb = base.rgb + streak * (settings.intensity * rain * angle_fade);

    return vec4<f32>(out_rgb, base.a);
}

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct RainGlareSettings {
    intensity: f32,
    threshold: f32,
    streak_length_px: f32,
    rain_density: f32,

    wind: vec2<f32>,
    speed: f32,
    time: f32,
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

// Procedural raindrops on the lens: thin streak lines oriented along `dir`.
// Visible where bright highlights exist (multiplied against the streak result).
fn lens_rain_mask(
    uv: vec2<f32>,
    dims: vec2<f32>,
    dir: vec2<f32>,
    t: f32,
    density: f32,
    speed: f32,
) -> f32 {
    let p = uv * dims;
    let perp = vec2<f32>(-dir.y, dir.x);

    let u = dot(p, perp);
    let v = dot(p, dir);

    // Spacing between streak lines in pixels.
    let spacing = 7.0;
    let line_id = floor(u / spacing);

    // Randomize which lines are active based on density.
    let r = hash11(line_id * 12.9898 + 78.233);
    let line_active = step(r, clamp(density, 0.0, 1.0));

    // Distance to the center of the current line cell, in pixels.
    let dist = abs(fract(u / spacing) - 0.5) * spacing;

    // Thin line profile (gaussian-ish).
    let sigma = 0.8;
    let width = exp(-(dist * dist) / (2.0 * sigma * sigma));

    // Droplet phase along the streak direction, moving over time.
    let period = 46.0;
    let phase = fract((v / period) + t * speed * 0.25 + r);

    // Head at phase ~0, exponential tail as phase increases.
    let tail = exp(-phase * 6.0);

    return clamp(line_active * width * tail, 0.0, 1.0);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(screen_texture, screen_sampler, in.uv);

    let dims_u = textureDimensions(screen_texture);
    let dims = vec2<f32>(f32(dims_u.x), f32(dims_u.y));

    let wind_len = length(settings.wind);
    let dir = select(vec2<f32>(0.0, 1.0), settings.wind / wind_len, wind_len > 1e-5);

    // Lens-rain mask (thin moving streak lines).
    let rain = lens_rain_mask(in.uv, dims, dir, settings.time, settings.rain_density, settings.speed);

    // Directional bright-pass smear (sample upstream so streaks trail downwind).
    let samples: i32 = 16;
    let len_uv = settings.streak_length_px / max(dims.y, 1.0);
    let step_uv = dir * (len_uv / f32(samples));

    // Per-pixel jitter to reduce banding / overly perfect streaks.
    let jitter = (hash12(in.uv * dims + vec2<f32>(settings.time, settings.time * 1.37)) - 0.5) * 0.9;
    let uv0 = in.uv + vec2<f32>(jitter / dims.x, 0.0);

    var accum = vec3<f32>(0.0);
    var wsum = 0.0;

    for (var i: i32 = 0; i < samples; i = i + 1) {
        let fi = f32(i);
        let suv = uv0 - step_uv * (fi + jitter);

        let c = textureSample(screen_texture, screen_sampler, suv).rgb;
        let b = clamp((luma(c) - settings.threshold) / max(1.0 - settings.threshold, 1e-5), 0.0, 1.0);

        // Exponential falloff so the streak tapers.
        let w = b * exp(-fi * 0.16);

        accum += c * w;
        wsum += w;
    }

    let streak = accum / max(wsum, 1e-5);

    // Only add glare where the lens rain mask says there's a droplet line.
    let out_rgb = base.rgb + streak * (settings.intensity * rain);

    return vec4<f32>(out_rgb, base.a);
}

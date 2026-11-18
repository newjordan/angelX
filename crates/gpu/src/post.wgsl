struct Post {
    texel: vec4<f32>,
    bloom: vec4<f32>,
    look: vec4<f32>,
    hit: vec4<f32>,
};

@group(0) @binding(0) var<uniform> post: Post;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var glow: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Varyings {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: Varyings;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

fn tap(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(source, linear_sampler, uv, 0.0).rgb;
}

fn bright(c: vec3<f32>) -> vec3<f32> {
    let peak = max(c.r, max(c.g, c.b));
    let low = min(c.r, min(c.g, c.b));
    let chroma = (peak - low) / max(peak, 1e-4);
    let threshold = post.bloom.x;
    let knee = post.bloom.y;
    var soft = clamp(peak - threshold + knee, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee + 1e-5);
    let over = max(soft, peak - threshold) / max(peak, 1e-4);
    return c * over * mix(post.bloom.z, 1.0, chroma);
}

@fragment
fn fs_bright(in: Varyings) -> @location(0) vec4<f32> {
    let t = post.texel.xy;
    let c = (tap(in.uv + vec2<f32>(-t.x, -t.y)) + tap(in.uv + vec2<f32>(t.x, -t.y))
        + tap(in.uv + vec2<f32>(-t.x, t.y)) + tap(in.uv + vec2<f32>(t.x, t.y))) * 0.25;
    let light = textureSampleLevel(glow, linear_sampler, in.uv, 0.0).rgb;
    let lit = smoothstep(0.0, post.hit.w, max(light.r, max(light.g, light.b)));
    return vec4<f32>(bright(c) * lit + light * post.hit.z, 1.0);
}

@fragment
fn fs_down(in: Varyings) -> @location(0) vec4<f32> {
    let t = post.texel.xy;
    let c = tap(in.uv) * 4.0 + tap(in.uv - t) + tap(in.uv + t)
        + tap(in.uv + vec2<f32>(t.x, -t.y)) + tap(in.uv + vec2<f32>(-t.x, t.y));
    return vec4<f32>(c * 0.125, 1.0);
}

@fragment
fn fs_up(in: Varyings) -> @location(0) vec4<f32> {
    let t = post.texel.xy;
    var c = tap(in.uv) * 4.0;
    c += (tap(in.uv + vec2<f32>(t.x, 0.0)) + tap(in.uv - vec2<f32>(t.x, 0.0))
        + tap(in.uv + vec2<f32>(0.0, t.y)) + tap(in.uv - vec2<f32>(0.0, t.y))) * 2.0;
    c += tap(in.uv + t) + tap(in.uv - t) + tap(in.uv + vec2<f32>(t.x, -t.y)) + tap(in.uv + vec2<f32>(-t.x, t.y));
    return vec4<f32>(c * (post.bloom.w / 16.0), 1.0);
}

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(vec3<f32>(p.x, p.y, p.x + p.y) * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn bayer(p: vec2<i32>) -> f32 {
    let x = u32(p.x) & 3u;
    let y = u32(p.y) & 3u;
    var m = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    return (m[y * 4u + x] + 0.5) / 16.0;
}

@fragment
fn fs_composite(in: Varyings) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(source));
    let px = vec2<i32>(in.position.xy);
    let at = clamp(px, vec2<i32>(0), size - 1);
    var base = textureLoad(source, at, 0);
    let impact = post.look.z;
    let from_middle = in.uv - 0.5;
    let split = vec2<i32>(round(from_middle * 2.0 * post.hit.x * impact));
    let red = textureLoad(source, clamp(at + split, vec2<i32>(0), size - 1), 0);
    let blue = textureLoad(source, clamp(at - split, vec2<i32>(0), size - 1), 0);
    base = vec4<f32>(red.r, base.g, blue.b, max(base.a, max(red.a, blue.a)));
    var rgb = base.rgb * (1.0 + impact * post.hit.y);
    let grid = max(post.look.w, 1.0);
    let cell = floor(in.position.xy / grid);
    let cell_uv = (cell + 0.5) * grid / vec2<f32>(size);
    let reach = length((cell_uv - 0.5) * 2.0) * 0.70710678;
    rgb *= 1.0 - post.look.x * smoothstep(0.55, 1.0, reach);
    let glowing = textureSampleLevel(glow, linear_sampler, in.uv, 0.0).rgb;
    let grey = dot(glowing, vec3<f32>(0.2126, 0.7152, 0.0722));
    let bloom = clamp(mix(vec3<f32>(grey), glowing, 1.4) * post.bloom.w, vec3<f32>(0.0), vec3<f32>(1.0));
    rgb = rgb + bloom * (1.0 - min(rgb, vec3<f32>(1.0)));
    let luma = clamp(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0, 1.0);
    let grain = (hash(cell) - 0.5) * post.look.y * 4.0 * (1.0 - luma);
    rgb *= 1.0 + grain;
    rgb += (bayer(vec2<i32>(cell)) - 0.5) / 255.0;
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, base.a);
}

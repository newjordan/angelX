// The stage's post pass (post.rs): a soft bloom of what burns and flies, a
// vignette into black, a film's grain in the middle tones, and a dither at
// the last step to eight bits; the colours split a little on a big hit.
//
// Every pass draws one triangle over its whole target. The bloom is a chain
// of ever smaller pictures: what is bright enough (`fs_bright`, the stage
// halved), halved three times more (`fs_down`), and added back up each
// level over the next (`fs_up`, blended One + One). `fs_composite` lays it
// over the stage, read texel for texel so the pixel art stays crisp.
// Everything is premultiplied, as every texture the core keeps is.

struct Post {
    // xy: one texel of the source, in its uv; zw: the target, in pixels.
    texel: vec4<f32>,
    // x: the threshold, y: its soft knee, z: what white counts for against
    // a colour, w: this pass's weight (an up pass) or the bloom's (composite).
    bloom: vec4<f32>,
    // x: the vignette's depth at the corners, y: the grain, z: the impact
    // (0..1), w: the target's pixels to one of the stage's.
    look: vec4<f32>,
    // x: the colours' split at a full impact, in pixels; y: how much the
    // light rises with it; z: how much of the stage's own light blooms; w:
    // the light at which what is bright blooms fully.
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
    // One triangle over the target: (-1, -1), (3, -1), (-1, 3).
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

// How much of a colour is bright: the part of its brightest channel over
// the threshold, eased in over the knee; a colour counts fully, white less.
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
    // Four taps a texel out: the stage's 4 x 4 texels under this one.
    let t = post.texel.xy;
    let c = (tap(in.uv + vec2<f32>(-t.x, -t.y)) + tap(in.uv + vec2<f32>(t.x, -t.y))
        + tap(in.uv + vec2<f32>(-t.x, t.y)) + tap(in.uv + vec2<f32>(t.x, t.y))) * 0.25;
    // What is bright blooms only where there is light: the stage's own
    // lights (its fires' pools, its shots' glow, its motes), drawn alone at
    // this size into `glow`. A knight's gold in the dark is paint; a shot's
    // heart in its own glow is a flame. The light's motes bloom themselves.
    let light = textureSampleLevel(glow, linear_sampler, in.uv, 0.0).rgb;
    let lit = smoothstep(0.0, post.hit.w, max(light.r, max(light.g, light.b)));
    return vec4<f32>(bright(c) * lit + light * post.hit.z, 1.0);
}

@fragment
fn fs_down(in: Varyings) -> @location(0) vec4<f32> {
    // The dual filter's way down: the middle four times, the corners once.
    let t = post.texel.xy;
    let c = tap(in.uv) * 4.0 + tap(in.uv - t) + tap(in.uv + t)
        + tap(in.uv + vec2<f32>(t.x, -t.y)) + tap(in.uv + vec2<f32>(-t.x, t.y));
    return vec4<f32>(c * 0.125, 1.0);
}

@fragment
fn fs_up(in: Varyings) -> @location(0) vec4<f32> {
    // A tent over the smaller level's 3 x 3 texels, weighted for its share.
    let t = post.texel.xy;
    var c = tap(in.uv) * 4.0;
    c += (tap(in.uv + vec2<f32>(t.x, 0.0)) + tap(in.uv - vec2<f32>(t.x, 0.0))
        + tap(in.uv + vec2<f32>(0.0, t.y)) + tap(in.uv - vec2<f32>(0.0, t.y))) * 2.0;
    c += tap(in.uv + t) + tap(in.uv - t) + tap(in.uv + vec2<f32>(t.x, -t.y)) + tap(in.uv + vec2<f32>(-t.x, t.y));
    return vec4<f32>(c * (post.bloom.w / 16.0), 1.0);
}

// A cheap hash of a pixel, 0..1.
fn hash(p: vec2<f32>) -> f32 {
    var q = fract(vec3<f32>(p.x, p.y, p.x + p.y) * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

// The 4 x 4 ordered dither's threshold at a pixel, 0..1.
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
    // A big hit splits the colours outward from the middle, by whole pixels
    // (the pixel art moves, it does not blur).
    let impact = post.look.z;
    let from_middle = in.uv - 0.5;
    let split = vec2<i32>(round(from_middle * 2.0 * post.hit.x * impact));
    let red = textureLoad(source, clamp(at + split, vec2<i32>(0), size - 1), 0);
    let blue = textureLoad(source, clamp(at - split, vec2<i32>(0), size - 1), 0);
    base = vec4<f32>(red.r, base.g, blue.b, max(base.a, max(red.a, blue.a)));
    // The light rises a little with the hit; black stays black.
    var rgb = base.rgb * (1.0 + impact * post.hit.y);
    // What is the stage's own (the vignette, the grain, the dither) is
    // whole stage pixels, as its art is: one value over each, never a
    // gradient inside one.
    let grid = max(post.look.w, 1.0);
    let cell = floor(in.position.xy / grid);
    let cell_uv = (cell + 0.5) * grid / vec2<f32>(size);
    // Into black at the corners: a wide, soft fall that leaves the middle
    // be. The bloom goes over it: a flame in a corner still shines.
    let reach = length((cell_uv - 0.5) * 2.0) * 0.70710678;
    rgb *= 1.0 - post.look.x * smoothstep(0.55, 1.0, reach);
    // The bloom is screened over, not added: it lights the dark round a
    // flame and leaves its bright heart as it was, never burnt to white.
    // Its colour a little deeper than its source's: a halo is the light's
    // own colour, never a white wash.
    let glowing = textureSampleLevel(glow, linear_sampler, in.uv, 0.0).rgb;
    let grey = dot(glowing, vec3<f32>(0.2126, 0.7152, 0.0722));
    let bloom = clamp(mix(vec3<f32>(grey), glowing, 1.4) * post.bloom.w, vec3<f32>(0.0), vec3<f32>(1.0));
    rgb = rgb + bloom * (1.0 - min(rgb, vec3<f32>(1.0)));
    // The canvas's grain, still, in the middle tones only, in the paint
    // (it scales what is there): none in black, none in white, none in a
    // channel a colour has none of.
    let luma = clamp(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0, 1.0);
    let grain = (hash(cell) - 0.5) * post.look.y * 4.0 * (1.0 - luma);
    rgb *= 1.0 + grain;
    // Dither to the eight bits it is stored in: a soft gradient keeps no
    // bands, and an exact colour (black among them) rounds back to itself.
    rgb += (bayer(vec2<i32>(cell)) - 0.5) / 255.0;
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, base.a);
}

// The world's look (docs/GLASS_CORE.md): the post pass of a scene that has an
// `angel_stage::look::Look`. The scene was drawn at the look's small internal
// size into a colour texture and an accent mask (mesh.wgsl, `fs_look`); this
// puts every pixel onto the ramp and writes the result, upscaled by nearest
// when the output is bigger than the internal size.
//
// A pixel the mesh pass already made final (mask 0.5: a look that lights
// its scene, mesh.wgsl's `lit_look`) is left as it is. A pixel of an accent
// surface is the nearest accent colour, as it is. Any
// other is placed on the ramp by its luminance: between the two entries whose
// own luminances hold it, the upper one where its place between them is past
// the Bayer threshold at its internal pixel, the lower where it is not.
// `Look::quantize` in angel-stage is the same thing on the CPU, and the tests
// hold this to it.

struct Look {
    // Each ramp entry: rgb 0..1 and its luminance (never falling) in w.
    ramp: array<vec4<f32>, 16>,
    // Each accent: rgb 0..1.
    accent: array<vec4<f32>, 4>,
    // x: ramp entries, y: accents, z: the Bayer matrix's bits per coordinate
    // (2 for 4x4, 3 for 8x8, 0 for none).
    counts: vec4<u32>,
    // x, y: the internal size; z, w: the output's.
    sizes: vec4<f32>,
};

@group(0) @binding(0) var<uniform> look: Look;
@group(0) @binding(1) var colour: texture_2d<f32>;
@group(0) @binding(2) var mask: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One triangle that covers the target.
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

// The Bayer threshold at an internal pixel: strictly between 0 and 1. The
// coordinates' bits, lowest first, give the value its pairs (x ^ y, y) from
// the top down (angel_stage::look::bayer_index).
fn threshold(p: vec2<u32>) -> f32 {
    let bits = look.counts.z;
    if bits == 0u {
        return 0.5;
    }
    var v = 0u;
    for (var i = 0u; i < bits; i = i + 1u) {
        let xb = (p.x >> i) & 1u;
        let yb = (p.y >> i) & 1u;
        v = (v << 2u) | (((xb ^ yb) << 1u) | yb);
    }
    return (f32(v) + 0.5) / f32(1u << (2u * bits));
}

fn pick(luma: f32, threshold: f32) -> u32 {
    let n = look.counts.x;
    if luma <= look.ramp[0].w {
        return 0u;
    }
    if luma >= look.ramp[n - 1u].w {
        return n - 1u;
    }
    // The last entry at or under the pixel; the next is over it.
    var k = 0u;
    for (var i = 0u; i < n; i = i + 1u) {
        if look.ramp[i].w <= luma {
            k = i;
        }
    }
    let lo = look.ramp[k].w;
    let hi = look.ramp[k + 1u].w;
    let t = (luma - lo) / (hi - lo);
    return select(k, k + 1u, t > threshold);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let internal = vec2<f32>(look.sizes.x, look.sizes.y);
    let output = vec2<f32>(look.sizes.z, look.sizes.w);
    // Which internal pixel this output pixel is a part of.
    let last = vec2<u32>(u32(look.sizes.x) - 1u, u32(look.sizes.y) - 1u);
    let p = min(vec2<u32>(floor(frag.xy * internal / output)), last);
    let texel = textureLoad(colour, p, 0);
    // The look is hard-edged: a pixel is on or it is not.
    if texel.a < 0.5 {
        return vec4<f32>(0.0);
    }
    let rgb = texel.rgb / texel.a;
    let marked = textureLoad(mask, p, 0).r;
    // Made final where it was drawn (a look that lights its scene).
    if marked > 0.25 && marked < 0.75 {
        return vec4<f32>(rgb, 1.0);
    }
    if marked > 0.75 && look.counts.y > 0u {
        var best = look.accent[0].rgb;
        var best_distance = 1e9;
        for (var i = 0u; i < look.counts.y; i = i + 1u) {
            let v = look.accent[i].rgb - rgb;
            let d = dot(v, v);
            if d < best_distance {
                best_distance = d;
                best = look.accent[i].rgb;
            }
        }
        return vec4<f32>(best, 1.0);
    }
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(look.ramp[pick(luma, threshold(p))].rgb, 1.0);
}

struct Look {
    ramp: array<vec4<f32>, 16>,
    accent: array<vec4<f32>, 4>,
    counts: vec4<u32>,
    sizes: vec4<f32>,
    dof: vec4<f32>,
};

@group(0) @binding(0) var<uniform> look: Look;
@group(0) @binding(1) var colour: texture_2d<f32>;
@group(0) @binding(2) var mask: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

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
    let last = vec2<u32>(u32(look.sizes.x) - 1u, u32(look.sizes.y) - 1u);
    let p = min(vec2<u32>(floor(frag.xy * internal / output)), last);
    let texel = textureLoad(colour, p, 0);
    if texel.a < 0.5 {
        return vec4<f32>(0.0);
    }
    let rgb = texel.rgb / texel.a;
    let marked = textureLoad(mask, p, 0).rg;
    if marked.r > 0.25 && marked.r < 0.75 {
        if marked.g > 0.0 && look.dof.x > 0.0 {
            return soft(p, marked.g, last);
        }
        return vec4<f32>(rgb, 1.0);
    }
    if marked.r > 0.75 && look.counts.y > 0u {
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

fn soft(p: vec2<u32>, blur: f32, last: vec2<u32>) -> vec4<f32> {
    let r = blur * look.dof.x;
    let reach = i32(ceil(r));
    var sum = vec4<f32>(0.0);
    var weight = 0.0;
    for (var dy = -reach; dy <= reach; dy = dy + 1) {
        for (var dx = -reach; dx <= reach; dx = dx + 1) {
            let d = length(vec2<f32>(f32(dx), f32(dy)));
            let q = vec2<u32>(clamp(vec2<i32>(p) + vec2<i32>(dx, dy), vec2<i32>(0), vec2<i32>(last)));
            let tap = textureLoad(colour, q, 0);
            let tap_blur = textureLoad(mask, q, 0).g * look.dof.x;
            let w = clamp(r + 0.5 - d, 0.0, 1.0) * clamp(tap_blur + 1.0 - d, 0.0, 1.0);
            if w > 0.0 && tap.a >= 0.5 {
                sum = sum + vec4<f32>(tap.rgb / tap.a, 1.0) * w;
            }
            weight = weight + w;
        }
    }
    return sum / max(weight, 1e-6);
}

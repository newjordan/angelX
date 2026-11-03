// The mesh pipeline: a 3D scene's triangles (angel-stage's `scene`).
// texture x vertex colour x tint, shaded by one fixed light when the material
// is lit, out premultiplied to match the target's blend (One,
// OneMinusSrcAlpha). One draw is one range of one mesh with one material; its
// uniforms sit at a dynamic offset in one buffer.
//
// A skinned mesh is bent by its joints (`vs_skinned`, the palette in group
// 3). A material's finish: chrome reflects a studio made here, liquid
// ripples and wobbles on the scene's clock, and a glowing surface shines
// past its colour with a halo drawn round it (`vs_halo`, `fs_halo`).

struct Draw {
    model: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    // Straight RGBA, 0..1.
    tint: vec4<f32>,
    // x: how alpha is used. 0 opaque (the texture's alpha is ignored), 1 the
    // texture was handed over straight, 2 it was premultiplied. Either way it
    // is stored premultiplied (uploads convert, targets render so), so 1 and
    // 2 read it alike. y: 1 when the material is lit. z: its look role in
    // the low byte (1 an accent surface, a scene's look keeps its colour
    // undithered; look.wgsl), and in the next its brand ceiling (docs/
    // BRAND.md): the step of an eight-step ramp plus one light never lifts
    // it past, 0 none. w: 1
    // when the picture is cast from `projector` instead of laid on by uv;
    // 2 for a card lit by a look (`fs_card`) whose normal map is group 2's
    // picture.
    flags: vec4<u32>,
    // x: how much of a lit surface shines by itself, 0..1. yzw: a projected
    // surface's patch's rectangle in group 2's picture: x, y, and
    // width * 256 + height, in pixels.
    light: vec4<f32>,
    // World to the projector's clip space (angel_stage::scene::Projector).
    projector: mat4x4<f32>,
    // A projected surface the projector does not see (angel_stage::scene::
    // Patch): xyz where the patch's corner lies in the world, w the world's
    // size of a pixel of it; w 0 when there is none, and the surface is its
    // tint. (`patch` is a word WGSL keeps.)
    unseen: vec4<f32>,
    // The finish (angel_stage::scene::Material): x chrome, y liquid, z the
    // scene's clock in seconds, w glow; each 0..1 but the clock.
    finish: vec4<f32>,
    // x: the mesh's radius in the world, what liquid is sized by; y: 1 when
    // its normals split where its faces meet (a cube's, a flat-shaded
    // model's), so liquid moves its points by where they are alone; z:
    // 1 for a halo's draw; w: 1 when the halo is a rounded box's edges,
    // which fade out where they meet its flat faces.
    shape: vec4<f32>,
    // Where the camera is (w 1), or the way toward it (w 0).
    eye: vec4<f32>,
    // y: the joints in this draw's palette, 0 for a mesh not bent.
    skin: vec4<u32>,
};

// The one light every scene has (angel_stage::scene::KEY_LIGHT and
// AMBIENT_LIGHT): the direction toward it, and the least a face gets.
const KEY_LIGHT: vec3<f32> = vec3<f32>(-0.47, 0.7, 0.54);
const AMBIENT: f32 = 0.42;

// How a projected surface the projector does not see is shaded by the way
// it faces (angel_stage::scene::FACE_SHADES): a top, a front, a side.
const FACE_TOP: f32 = 1.0;
const FACE_FRONT: f32 = 0.86;
const FACE_SIDE: f32 = 0.74;
// How much more edge-on than the camera the projector may see a surface and
// still cast on it (angel_stage::scene::GRAZING).
const GRAZING: f32 = 0.5;

@group(0) @binding(0) var<uniform> draw: Draw;
@group(1) @binding(0) var picture: texture_2d<f32>;
@group(1) @binding(1) var picture_sampler: sampler;
// The patch a projected surface is where its projector does not see it.
@group(2) @binding(0) var patch_picture: texture_2d<f32>;
@group(2) @binding(1) var patch_sampler: sampler;
// A skinned draw's joint matrices (angel_stage::scene::skin_matrices), its
// slice of the palettes at a dynamic offset. Binding 3: group 3's others
// are a look's, in other pipelines.
@group(3) @binding(3) var<uniform> palette: array<mat4x4<f32>, 256>;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    // The world normal, for the lit materials.
    @location(2) normal: vec3<f32>,
    // Where the point is in the world, for the look's fog and a projector.
    @location(3) world: vec3<f32>,
    // Where the projector and the camera are (`eye_of`), the same for the
    // whole draw: made once a vertex, not once a pixel.
    @location(4) @interpolate(flat) projector_eye: vec4<f32>,
    @location(5) @interpolate(flat) camera_eye: vec4<f32>,
};

@vertex
fn vs_main(v: Vertex) -> Varyings {
    return place(draw.model, v.position, v.normal, v.uv, v.color);
}

// A skinned vertex: its joints and how much each pulls it, beside the rest.
struct SkinnedVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) joints: vec4<u32>,
    @location(5) weights: vec4<f32>,
};

// A skinned mesh: placed by its joints' matrices blended by its weights
// (linear blend skinning, the weights scaled to sum to 1; a joint past the
// palette pulls nothing), as angel_stage::scene::skin_matrix does it; a
// vertex nothing pulls stays with its node.
@vertex
fn vs_skinned(v: SkinnedVertex) -> Varyings {
    let count = min(draw.skin.y, 256u);
    var m = mat4x4<f32>(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
    var total = 0.0;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let j = v.joints[k];
        let w = v.weights[k];
        if j < count && w > 0.0 {
            m = m + palette[j] * w;
            total = total + w;
        }
    }
    var placed = draw.model;
    if total > 1e-6 {
        placed = m * (1.0 / total);
    }
    return place(placed, v.position, v.normal, v.uv, v.color);
}

// A vertex in the world by matrix `m`, wobbled when the finish is liquid.
fn place(m: mat4x4<f32>, position: vec3<f32>, normal: vec3<f32>, uv: vec2<f32>, color: vec4<f32>) -> Varyings {
    var out: Varyings;
    var world = (m * vec4<f32>(position, 1.0)).xyz;
    out.uv = uv;
    out.color = color;
    // The normal through the model's cofactor matrix (the inverse transpose
    // times the determinant), so a stretched box keeps normals square to its
    // faces; a mirrored model (a negative determinant) is turned back.
    let a = m[0].xyz;
    let b = m[1].xyz;
    let c = m[2].xyz;
    let cofactor = mat3x3<f32>(cross(b, c), cross(c, a), cross(a, b));
    let side = select(1.0, -1.0, dot(a, cross(b, c)) < 0.0);
    out.normal = cofactor * normal * side;
    if draw.finish.y > 0.0 {
        world = world + wobble(world, out.normal);
    }
    out.position = draw.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.projector_eye = vec4<f32>(0.0);
    out.camera_eye = vec4<f32>(0.0);
    if draw.flags.w == 1u {
        out.projector_eye = eye_of(draw.projector);
        out.camera_eye = eye_of(draw.view_proj);
    }
    return out;
}

// ---- liquid ----

// One travelling wave at `q` along `d`, `f` waves a unit, at `phase`, of
// height `a`: x its height, yzw its slope.
fn wave(q: vec3<f32>, d: vec3<f32>, f: f32, phase: f32, a: f32) -> vec4<f32> {
    let x = dot(q, d) * f + phase;
    return vec4<f32>(a * sin(x), d * (a * f * cos(x)));
}

// How a liquid surface moves at `p` (in the mesh's radii, so a ripple is
// the same size on a cube as on a figure) at time `t`: x its height, about
// -1..1, and yzw its slope. Five waves over a slow warp, so the crests bend
// and drift as a pool of mercury's do.
fn flow(p: vec3<f32>, t: f32) -> vec4<f32> {
    let q = p + 0.35 * vec3<f32>(sin(p.y * 1.3 + t * 0.53), sin(p.z * 1.7 - t * 0.41), sin(p.x * 1.1 + t * 0.67));
    var out = wave(q, vec3<f32>(0.80, 0.55, 0.24), 2.1, t * 1.10, 0.36);
    out = out + wave(q, vec3<f32>(-0.42, 0.85, 0.31), 3.3, t * 1.55, 0.26);
    out = out + wave(q, vec3<f32>(0.18, -0.36, 0.92), 4.7, -t * 1.90, 0.20);
    out = out + wave(q, vec3<f32>(-0.70, -0.20, -0.68), 6.9, t * 2.60, 0.12);
    out = out + wave(q, vec3<f32>(0.33, 0.93, -0.15), 9.8, -t * 3.30, 0.06);
    return out;
}

// Where a liquid vertex moves, by a little of its radius: along its
// normal; or, for a mesh whose normals split where its faces meet, by a
// field of where it is alone (three of the waves, one an axis), so the
// points a seam shares move together and it never opens.
fn wobble(world: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let radius = max(draw.shape.x, 1e-4);
    let t = draw.finish.z;
    let q = world / radius * 1.2;
    let size = draw.finish.y * 0.045 * radius;
    if draw.shape.y > 0.5 {
        let field = vec3<f32>(
            flow(q, t).x,
            flow(q.yzx + vec3<f32>(1.7, 3.1, 0.6), t * 1.07).x,
            flow(q.zxy + vec3<f32>(4.2, 0.9, 2.3), t * 0.93).x,
        );
        return field * size;
    }
    return normal / max(length(normal), 1e-20) * (flow(q, t).x * size);
}

// A liquid surface's normal: turned by the slope of its wobble and of
// finer ripples riding it.
fn rippled(n: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let radius = max(draw.shape.x, 1e-4);
    let t = draw.finish.z;
    let slope = flow(world / radius * 1.2, t).yzw * 0.054 + flow(world / radius * 3.1, t * 1.4).yzw * 0.06;
    let across = slope - n * dot(slope, n);
    return normalize(n - across * draw.finish.y);
}

// ---- chrome ----

// A softbox: a soft-edged rectangle of light, at azimuth `centre.x` and
// height `centre.y` (the sine of its elevation), `half` its half size in
// the same, brightest at its middle.
fn softbox(azimuth: f32, height: f32, centre: vec2<f32>, half: vec2<f32>) -> f32 {
    var d = azimuth - centre.x;
    d = d - 6.2831853 * round(d / 6.2831853);
    let e = abs(vec2<f32>(d, height - centre.y)) / half;
    let edge = max(e.x, e.y);
    return (1.0 - smoothstep(0.72, 1.0, edge)) * (1.0 - 0.22 * min(dot(e, e), 1.0));
}

// The studio a chrome surface reflects, seen along `r`: a cool sky darker
// overhead, a dark floor, a bright horizon, three softboxes and a dim
// bounce off the floor, and the light warmer on one side of the room than
// the other. Linear light, past 1 where
// it is bright.
fn studio(r: vec3<f32>) -> vec3<f32> {
    let y = clamp(r.y, -1.0, 1.0);
    let azimuth = atan2(r.x, r.z);
    let sky = mix(vec3<f32>(0.62, 0.70, 0.82), vec3<f32>(0.10, 0.13, 0.22), sqrt(smoothstep(0.0, 1.0, y)));
    let ground = mix(vec3<f32>(0.26, 0.22, 0.18), vec3<f32>(0.03, 0.03, 0.035), pow(smoothstep(0.0, 0.8, -y), 0.6));
    var c = select(ground, sky, y >= 0.0);
    c = c + vec3<f32>(2.0, 1.9, 1.75) * exp(-abs(y) * 38.0) + vec3<f32>(0.34, 0.36, 0.40) * exp(-abs(y) * 5.0);
    // The key overhead and to the left, a tall cool strip on the right, a
    // warm one behind on the left.
    c = c + vec3<f32>(3.4, 3.3, 3.1) * softbox(azimuth, y, vec2<f32>(-0.55, 0.78), vec2<f32>(0.70, 0.16));
    c = c + vec3<f32>(2.3, 2.6, 3.1) * softbox(azimuth, y, vec2<f32>(1.40, 0.22), vec2<f32>(0.11, 0.38));
    c = c + vec3<f32>(3.1, 2.5, 1.9) * softbox(azimuth, y, vec2<f32>(-1.95, 0.28), vec2<f32>(0.13, 0.30));
    // And a dim bounce off the floor in front, so what faces down still
    // reads.
    c = c + vec3<f32>(0.42, 0.40, 0.38) * softbox(azimuth, y, vec2<f32>(0.0, -0.32), vec2<f32>(1.5, 0.22));
    let side = 0.5 + 0.5 * sin(azimuth + 0.4);
    return c * mix(vec3<f32>(1.08, 0.97, 0.86), vec3<f32>(0.86, 0.97, 1.10), side);
}

// Linear light to display: a filmic curve (Narkowicz's fit of ACES), so a
// softbox burns to white while the floor stays deep.
fn filmic(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

// The way from `world` toward the eye.
fn toward_eye(world: vec3<f32>) -> vec3<f32> {
    if draw.eye.w > 0.5 {
        let to = draw.eye.xyz - world;
        return to / max(length(to), 1e-20);
    }
    return draw.eye.xyz / max(length(draw.eye.xyz), 1e-20);
}

// A chrome surface of unit normal `n` (toward the eye's side): the studio
// reflected in it, tinted face on by `tint` and running to white at the rim
// (Schlick's Fresnel for a metal), a hard highlight of the key light, and a
// cool rim.
fn chrome_of(n: vec3<f32>, world: vec3<f32>, tint: vec3<f32>) -> vec3<f32> {
    let v = toward_eye(world);
    let r = reflect(-v, n);
    let facing = clamp(dot(n, v), 0.0, 1.0);
    let rim = pow(1.0 - facing, 5.0);
    let fresnel = tint + (vec3<f32>(1.0) - tint) * rim;
    var c = studio(r) * fresnel;
    c = c + vec3<f32>(1.0, 0.97, 0.92) * (pow(max(dot(r, normalize(KEY_LIGHT)), 0.0), 160.0) * 4.0);
    c = c + vec3<f32>(0.70, 0.80, 0.95) * (pow(1.0 - facing, 3.0) * 0.30);
    return filmic(c);
}

// A surface drawn with a finish: liquid turns its normal, chrome mixes the
// studio's reflection over its colour `base` (straight, as otherwise
// drawn), and glow lifts it past its colour toward white.
fn finished(base: vec3<f32>, in_normal: vec3<f32>, world: vec3<f32>, front: bool) -> vec3<f32> {
    var n = normalize(in_normal);
    n = select(-n, n, front);
    if draw.finish.y > 0.0 {
        n = rippled(n, world);
    }
    var c = base;
    if draw.finish.x > 0.0 {
        c = mix(c, chrome_of(n, world, draw.tint.rgb), draw.finish.x);
    }
    let g = draw.finish.w;
    if g > 0.0 {
        // Brighter than its colour, keeping its hue, a little white in its
        // hottest.
        c = c * (1.0 + 0.45 * g) + vec3<f32>(0.08 * g * g);
    }
    return min(c, vec3<f32>(1.0));
}

// ---- the halo ----

// A glowing surface's halo: the surface again, pushed out along its normals
// a little of its radius, drawn over everything (testing depth, leaving
// it). For a rounded box's edges, `uv.x` carries how far the normal is from
// its box's faces, the halo fading out where the edges meet them.
@vertex
fn vs_halo(v: Vertex) -> Varyings {
    var out = place(draw.model, v.position, v.normal, v.uv, v.color);
    let n = out.normal / max(length(out.normal), 1e-20);
    let reach = draw.shape.x * (0.025 + 0.055 * draw.finish.w);
    out.world = out.world + n * reach;
    out.position = draw.view_proj * vec4<f32>(out.world, 1.0);
    let a = abs(normalize(v.normal));
    out.uv = vec2<f32>(clamp((1.0 - max(max(a.x, a.y), a.z)) / 0.2929, 0.0, 1.0), 0.0);
    return out;
}

// Light added where the halo's shell faces the eye, gone toward its
// outline: covering nothing (alpha 0), so it adds to what is behind.
@fragment
fn fs_halo(in: Varyings) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let facing = abs(dot(n, toward_eye(in.world)));
    var fade = 1.0;
    if draw.shape.w > 0.5 {
        fade = smoothstep(0.0, 1.0, in.uv.x);
    }
    let light = pow(facing, 2.2) * draw.finish.w * 0.85 * fade * draw.tint.a;
    return vec4<f32>(min(draw.tint.rgb * (1.0 + 0.4 * draw.finish.w), vec3<f32>(1.0)) * light, 0.0);
}

// `p / w` for a clip point, kept finite where w is not in front (that
// point is not cast anything, but its neighbours' derivatives still read it).
fn divided(p: vec4<f32>) -> vec2<f32> {
    return p.xy / select(1.0, p.w, p.w > 1e-12);
}

// Where a camera (world to clip) is, in homogeneous world coordinates: the
// point its x, y and w rows all take to 0 (their generalized cross
// product). w is 0 for one that looks along a direction (orthographic), and
// xyz is then that direction.
fn eye_of(m: mat4x4<f32>) -> vec4<f32> {
    let a = vec4<f32>(m[0].x, m[1].x, m[2].x, m[3].x);
    let b = vec4<f32>(m[0].y, m[1].y, m[2].y, m[3].y);
    let d = vec4<f32>(m[0].w, m[1].w, m[2].w, m[3].w);
    return vec4<f32>(
        dot(a.yzw, cross(b.yzw, d.yzw)),
        -dot(a.xzw, cross(b.xzw, d.xzw)),
        dot(a.xyw, cross(b.xyw, d.xyw)),
        -dot(a.xyz, cross(b.xyz, d.xyz)),
    );
}

// How squarely a camera whose eye is `e` (`eye_of`) sees a surface of unit
// normal `n` at world point `p`: the cosine between the normal and the way
// to the eye, 0 edge-on.
fn squareness(e: vec4<f32>, n: vec3<f32>, p: vec3<f32>) -> f32 {
    let ray = e.xyz - p * e.w;
    return abs(dot(n, ray)) / max(length(ray), 1e-30);
}

// Where the projector casts this world point: xy the texel coordinates
// (v down), z 1 when the projector sees it there and 0 when not. Not seen:
// behind the projector, outside its picture, a face turned away from it,
// which is when the projector sees the surface wound against the way the
// camera does (the sign of the one's screen-space Jacobian over the
// other's, which no flip of the window's y can change), or one the
// projector sees much more edge-on than the camera does (GRAZING), whose
// texels would smear over it. Computed from the world point here, never
// from interpolated clip coordinates (WebGL), and called before any branch
// that is not the same for the whole draw: derivatives need every fragment
// of the quad.
fn cast_of(world: vec3<f32>, normal_in: vec3<f32>, projector_eye: vec4<f32>, camera_eye: vec4<f32>) -> vec3<f32> {
    let p = draw.projector * vec4<f32>(world, 1.0);
    let q = divided(p);
    let c = divided(draw.view_proj * vec4<f32>(world, 1.0));
    let dq_x = dpdx(q);
    let dq_y = dpdy(q);
    let dc_x = dpdx(c);
    let dc_y = dpdy(c);
    let turn_projector = dq_x.x * dq_y.y - dq_x.y * dq_y.x;
    let turn_camera = dc_x.x * dc_y.y - dc_x.y * dc_y.x;
    let uv = vec2<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5);
    let inside = all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    let n = normal_in / max(length(normal_in), 1e-30);
    let square = squareness(projector_eye, n, world) >= GRAZING * squareness(camera_eye, n, world);
    let seen = p.w > 0.0 && inside && turn_projector * turn_camera > 0.0 && square;
    return vec3<f32>(uv, select(0.0, 1.0, seen));
}

// What a projected surface the projector does not see is: its patch tiled
// over it (or white, for its tint alone), laid on from the side it faces
// most and mirrored at each repeat, and how much it is shaded for facing
// so (a surface that shines, emissive 1, is not).
fn unseen_of(world: vec3<f32>, normal_in: vec3<f32>) -> vec4<f32> {
    let n = normal_in / max(length(normal_in), 1e-30);
    let a = abs(n);
    let top = a.y >= max(a.x, a.z);
    let front = !top && a.z >= a.x;
    let r = world - draw.unseen.xyz;
    let across = select(select(vec2<f32>(r.z, -r.y), vec2<f32>(r.x, -r.y), front), r.xz, top);
    let face = select(select(FACE_SIDE, FACE_FRONT, front), select(FACE_SIDE, FACE_TOP, n.y > 0.0), top);
    let wh = u32(draw.light.w);
    let size = max(vec2<f32>(f32(wh >> 8u), f32(wh & 255u)), vec2<f32>(1.0));
    let t = across / select(1.0, draw.unseen.w, draw.unseen.w > 0.0) / size;
    // A triangle wave: 0 to 1 and back, each copy the last one flipped.
    let mirrored = vec2<f32>(1.0) - abs(fract(t * 0.5) * 2.0 - vec2<f32>(1.0));
    // The middle of the patch's pixel there, in the picture.
    let pixel = min(floor(mirrored * size), size - vec2<f32>(1.0));
    let picture_size = vec2<f32>(textureDimensions(patch_picture));
    let texel = textureSampleLevel(patch_picture, patch_sampler, (draw.light.yz + pixel + vec2<f32>(0.5)) / picture_size, 0.0);
    let shade = mix(face, 1.0, draw.light.x);
    return vec4<f32>(select(vec3<f32>(1.0), texel.rgb, draw.unseen.w > 0.0) * shade, 1.0);
}

// The texel and the tint a surface is drawn with: the picture by the mesh's
// uv and the material's tint, or, cast from a projector, the picture where
// the projector sees the surface (untinted), and where not its patch,
// shaded, or its tint, shaded.
struct Surface {
    texel: vec4<f32>,
    tint: vec4<f32>,
};

fn surface_of(uv: vec2<f32>, thrown: vec3<f32>, world: vec3<f32>, normal: vec3<f32>) -> Surface {
    let projected = draw.flags.w == 1u;
    // An explicit level: pictures have no mips, and no derivatives are needed.
    let texel = textureSampleLevel(picture, picture_sampler, select(uv, thrown.xy, projected), 0.0);
    let missed = projected && thrown.z < 0.5;
    // Only a surface the projector misses needs it (no derivatives in it).
    var unseen = vec4<f32>(1.0);
    if missed {
        unseen = unseen_of(world, normal);
    }
    let has_patch = draw.unseen.w > 0.0;
    // Not seen: the patch, shaded, its alpha the tint's; or the tint, shaded.
    let missed_tint = select(vec4<f32>(draw.tint.rgb * unseen.rgb, draw.tint.a), vec4<f32>(1.0, 1.0, 1.0, draw.tint.a), has_patch);
    var out: Surface;
    out.texel = select(texel, select(vec4<f32>(1.0), unseen, has_patch), missed);
    out.tint = select(select(draw.tint, vec4<f32>(1.0), projected), missed_tint, missed);
    return out;
}

// A plain Lambert from the key light over an ambient floor; a surface that
// shines by itself takes its own colour back. A back face seen (a
// double-sided card) is lit from the side it shows. Unlit is 1.
fn shade_of(normal_in: vec3<f32>, front: bool) -> f32 {
    var shade = 1.0;
    if draw.flags.y == 1u {
        let normal = select(-normalize(normal_in), normalize(normal_in), front);
        let lambert = max(dot(normal, normalize(KEY_LIGHT)), 0.0);
        shade = mix(AMBIENT + (1.0 - AMBIENT) * lambert, 1.0, draw.light.x);
    }
    return shade;
}

@fragment
fn fs_main(in: Varyings, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Only a projected surface is cast anything (a branch on the draw, so
    // every fragment of a quad takes it alike and derivatives hold).
    var thrown = vec3<f32>(0.0);
    if draw.flags.w == 1u {
        thrown = cast_of(in.world, in.normal, in.projector_eye, in.camera_eye);
    }
    let surface = surface_of(in.uv, thrown, in.world, in.normal);
    let texel = surface.texel;
    let m = in.color * surface.tint;
    let rgb = m.rgb * shade_of(in.normal, front);
    if draw.finish.x > 0.0 || draw.finish.y > 0.0 || draw.finish.w > 0.0 {
        // A finish: the straight colour as drawn, then finished, alpha as
        // the surface's.
        let straight = select(texel.rgb, texel.rgb / texel.a, texel.a > 0.0);
        var alpha = 1.0;
        if draw.flags.x != 0u {
            alpha = texel.a * m.a;
        }
        var lit = rgb;
        if draw.finish.y > 0.0 && draw.flags.y == 1u {
            // Liquid on a lit surface: lit by its rippled normal.
            let n = rippled(select(-normalize(in.normal), normalize(in.normal), front), in.world);
            lit = m.rgb * mix(AMBIENT + (1.0 - AMBIENT) * max(dot(n, normalize(KEY_LIGHT)), 0.0), 1.0, draw.light.x);
        }
        return vec4<f32>(finished(straight * lit, in.normal, in.world, front) * alpha, alpha);
    }
    if draw.flags.x == 0u {
        // Alpha ignored: the colour back out of its premultiplied storage.
        let colour = select(texel.rgb, texel.rgb / texel.a, texel.a > 0.0);
        return vec4<f32>(colour * rgb, 1.0);
    }
    return texel * vec4<f32>(rgb * m.a, m.a);
}

// A scene's look (angel_stage::look::Look), by the pipelines that draw into
// the internal targets: group 3 holds what is the same for the whole scene.
// The fog is by distance from the eye.
struct SceneLook {
    // xyz: the eye, in the world; w: 1 when the look lights the scene
    // (`Look::lights`), and its pixels are placed on their ramps here.
    eye: vec4<f32>,
    // x: fog starts, y: fog ends, z: 1 when there is fog, w: the clock.
    fog: vec4<f32>,
    // xyz: the fog's colour, 0..1; w: which ramp entry it is.
    fog_color: vec4<f32>,
    // xyz: toward the sun (unit); w: its strength.
    sun: vec4<f32>,
    // x: the ambient; y: a shadow texel's side in uv; z: 1 with a water
    // ramp; w: the height the sky's light rises over (0: none).
    sun_more: vec4<f32>,
    // World to the shadow map's clip space.
    shadow: mat4x4<f32>,
    // x: ramp entries, y: zones, z: Bayer bits per coordinate, w: accents.
    counts: vec4<u32>,
    // x: 1 with a warm ramp, y: 1 with shadows, z: lamps, w: 1 with a sun.
    flags: vec4<u32>,
    ramp: array<vec4<f32>, 16>,
    warm: array<vec4<f32>, 16>,
    accent: array<vec4<f32>, 4>,
    // Two a lamp: xyz where, w reach; then x strength.
    lamps: array<vec4<f32>, 32>,
    // Two a zone: x and z of its middle, its radius, its feather; then its
    // box's half width and depth.
    zones: array<vec4<f32>, 8>,
    // 16 entries a zone.
    zone_ramps: array<vec4<f32>, 64>,
    water: array<vec4<f32>, 16>,
    // Each point kept in sight: xyz, w the opening's radius (0: none).
    reveal: array<vec4<f32>, 4>,
    // x: world units in a metre.
    units: vec4<f32>,
    // How figures' cards are lit (`Look::cards`): the least and the most of
    // the open's light, how far the lamps warm them, a lamp's rim; all 0
    // when they are not. Then how much farther the lamps reach them.
    cards: vec4<f32>,
    cards_more: vec4<f32>,
    // The depth of field (`Look::dof`): the nearest and farthest depths
    // kept sharp, the depth over which the blur grows, 1 when there is one.
    dof: vec4<f32>,
    // A point's depth along the camera, in world units: dot(xyz, p) + w.
    depth_row: vec4<f32>,
};

@group(3) @binding(0) var<uniform> scene_look: SceneLook;
@group(3) @binding(1) var shadow_map: texture_depth_2d;
@group(3) @binding(2) var shadow_sampler: sampler_comparison;

struct LookOut {
    // Straight colour as drawn and fogged, alpha 1; or, when the look
    // lights the scene, the pixel's final colour.
    @location(0) color: vec4<f32>,
    // x: 1 where an accent surface was drawn and is not lost in the fog, 0.5
    // where the pixel is already final (look.wgsl leaves it), else 0. y: how
    // far it is out of focus, 0..1 (`blur_of`).
    @location(1) mask: vec2<f32>,
};

// How far a world point is out of the depth of field's focus, 0 (sharp) to
// 1 (blurred its whole radius): 0 between the nearest and farthest depths
// kept sharp, growing smoothly over the spread past either.
fn blur_of(world: vec3<f32>) -> f32 {
    let dof = scene_look.dof;
    if dof.w < 0.5 {
        return 0.0;
    }
    let depth = dot(scene_look.depth_row.xyz, world) + scene_look.depth_row.w;
    let off = max(max(dof.x - depth, depth - dof.y), 0.0);
    return smoothstep(0.0, max(dof.z, 1e-6), off);
}

// The sun's shadow pass: where a vertex is in the shadow map's clip space.
@vertex
fn vs_shadow(v: Vertex) -> @builtin(position) vec4<f32> {
    return scene_look.shadow * draw.model * vec4<f32>(v.position, 1.0);
}

// The Bayer threshold at an internal pixel, as look.wgsl's `threshold`.
fn bayer(p: vec2<u32>) -> f32 {
    let bits = scene_look.counts.z;
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

fn luma(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// How much of the sun reaches a world point: 1 in the open, 0 where
// something stands between, a nine-tap blend at the edge (each tap the
// sampler's own four-texel compare). Pushed out along the normal by a
// texel so a lit face does not shadow itself.
fn sunlit(world: vec3<f32>, n: vec3<f32>) -> f32 {
    if scene_look.flags.y == 0u {
        return 1.0;
    }
    let texel = scene_look.sun_more.y;
    // A texel of the map is this many world units: 2 / scale of x.
    let world_texel = 2.0 / max(length(scene_look.shadow[0].xyz), 1e-6) * texel;
    let p = scene_look.shadow * vec4<f32>(world + n * world_texel * 1.5, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || p.z > 1.0 || p.z < 0.0 {
        return 1.0;
    }
    var sum = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let o = vec2<f32>(f32(dx), f32(dy)) * texel;
            sum = sum + textureSampleCompareLevel(shadow_map, shadow_sampler, uv + o, p.z - 0.0005);
        }
    }
    return sum / 9.0;
}

// The light at a world point of normal `n` from the look's sun and lamps:
// x the brightness it is multiplied by, y how far the lamps warm it.
fn lighting(world: vec3<f32>, n: vec3<f32>, flat_face: bool) -> vec2<f32> {
    var light = 1.0;
    if scene_look.flags.w == 1u {
        let sun = scene_look.sun;
        // A figure is lit as an upright card facing the light would be.
        let facing = select(max(dot(n, sun.xyz), 0.0), 0.75, flat_face);
        var sky = 1.0;
        if scene_look.sun_more.w > 0.0 {
            sky = mix(0.6, 1.0, smoothstep(0.0, scene_look.sun_more.w, world.y));
        }
        light = scene_look.sun_more.x * sky + sun.w * facing * sunlit(world, n);
    }
    var warm = 0.0;
    for (var i = 0u; i < scene_look.flags.z; i = i + 1u) {
        let at = scene_look.lamps[2u * i];
        let strength = scene_look.lamps[2u * i + 1u].x;
        let to = at.xyz - world;
        let d = length(to);
        let fall = clamp(1.0 - d / max(at.w, 1e-6), 0.0, 1.0);
        // Faces turned from the lamp still catch some of it.
        let wrap = select(0.45 + 0.55 * max(dot(n, to / max(d, 1e-6)), 0.0), 0.8, flat_face);
        let add = strength * fall * fall * wrap;
        light = light + add;
        warm = warm + add;
    }
    return vec2<f32>(light, warm);
}

// How far ground point `xz` is in zone `z`, 0..1.
fn zone_weight(z: u32, xz: vec2<f32>) -> f32 {
    let zone = scene_look.zones[2u * z];
    let half = scene_look.zones[2u * z + 1u].xy;
    let d = length(max(abs(xz - zone.xy) - half, vec2<f32>(0.0)));
    return 1.0 - smoothstep(zone.z, zone.z + max(zone.w, 1e-6), d);
}

// Ramp entry `i` at ground point `xz`: the look's own, each zone mixed over
// it by its weight there, then toward the warm ramp by `warm`.
fn entry_at(i: u32, xz: vec2<f32>, weights: vec4<f32>, warm: f32) -> vec3<f32> {
    var c = scene_look.ramp[i].rgb;
    for (var z = 0u; z < scene_look.counts.y; z = z + 1u) {
        c = mix(c, scene_look.zone_ramps[z * 16u + i].rgb, weights[z]);
    }
    if scene_look.flags.x == 1u {
        c = mix(c, scene_look.warm[i].rgb, warm);
    }
    return c;
}

// The pixel's place on its ramp: the ramp at its ground point, its
// brightness placed between the two entries that hold it by the Bayer
// threshold at internal pixel `p` (as look.wgsl's `pick`).
fn on_ramp(level: f32, xz: vec2<f32>, warm: f32, p: vec2<u32>, water: bool) -> vec3<f32> {
    var weights = vec4<f32>(0.0);
    for (var z = 0u; z < scene_look.counts.y; z = z + 1u) {
        weights[z] = zone_weight(z, xz);
    }
    if water {
        return on_water(level, p);
    }
    let n = scene_look.counts.x;
    let t = bayer(p);
    var lo = entry_at(0u, xz, weights, warm);
    var lo_level = luma(lo);
    if level <= lo_level {
        return lo;
    }
    for (var i = 1u; i < n; i = i + 1u) {
        let hi = entry_at(i, xz, weights, warm);
        // Levels never fall: an entry darker than the last is held at it.
        let hi_level = max(luma(hi), lo_level);
        if level < hi_level {
            let f = (level - lo_level) / max(hi_level - lo_level, 1e-6);
            return select(lo, hi, f > t);
        }
        lo = hi;
        lo_level = hi_level;
    }
    return lo;
}

// A water pixel's place on the water's own ramp (`Look::water`).
fn on_water(level: f32, p: vec2<u32>) -> vec3<f32> {
    let n = scene_look.counts.x;
    let t = bayer(p);
    var lo = scene_look.water[0].rgb;
    var lo_level = luma(lo);
    if level <= lo_level {
        return lo;
    }
    for (var i = 1u; i < n; i = i + 1u) {
        let hi = scene_look.water[i].rgb;
        let hi_level = max(luma(hi), lo_level);
        if level < hi_level {
            let f = (level - lo_level) / max(hi_level - lo_level, 1e-6);
            return select(lo, hi, f > t);
        }
        lo = hi;
        lo_level = hi_level;
    }
    return lo;
}

// Water's brightness at a point: slow bands drifting across it with the
// clock, bent, in metres of the world (`Look::metre`) so a stage in pixels
// and a scene in metres run alike; and how bright the light glints off it
// there (0..1).
fn water_of(world: vec3<f32>) -> vec2<f32> {
    let t = scene_look.fog.w;
    let m = world.xz / max(scene_look.units.x, 1e-6);
    let u = m.x * 0.82 + m.y * 0.36;
    let v = m.y * 0.82 - m.x * 0.36;
    let bend = sin(v * 0.21 + u * 0.07) * 1.7 + sin(u * 0.031 - t * 0.2) * 2.0;
    let a = sin(u * 0.55 - t * 1.4 + bend);
    let b = sin(u * 1.37 - t * 2.3 + v * 0.35 + bend * 0.6);
    let c = sin(u * 0.21 + v * 0.12 - t * 0.6);
    let ripple = a * 0.5 + b * 0.3 + c * 0.2;
    // The glint: where the crests line up, off the sun toward the eye.
    let to_eye = normalize(scene_look.eye.xyz - world);
    let half = normalize(to_eye + scene_look.sun.xyz);
    let spec = pow(max(half.y, 0.0), 6.0);
    let glint = smoothstep(0.62, 0.9, ripple) * spec;
    return vec2<f32>(ripple, glint);
}

// The look is hard-edged: a surface is covered at half alpha or more, or it is
// not there, so what the quantizer reads has no soft edges to put on the ramp.
@fragment
fn fs_look(in: Varyings, @builtin(front_facing) front: bool) -> LookOut {
    // Only a projected surface is cast anything (a branch on the draw, so
    // every fragment of a quad takes it alike and derivatives hold).
    var thrown = vec3<f32>(0.0);
    if draw.flags.w == 1u {
        thrown = cast_of(in.world, in.normal, in.projector_eye, in.camera_eye);
    }
    let surface = surface_of(in.uv, thrown, in.world, in.normal);
    let texel = surface.texel;
    let m = in.color * surface.tint;
    // Textures are stored premultiplied: the straight colour is over the alpha.
    let straight = select(texel.rgb, texel.rgb / texel.a, texel.a > 0.0);
    var alpha = 1.0;
    if draw.flags.x != 0u {
        alpha = texel.a * m.a;
    }
    if alpha < 0.5 {
        discard;
    }
    var fog = 0.0;
    if scene_look.fog.z > 0.5 {
        let span = max(scene_look.fog.y - scene_look.fog.x, 1e-6);
        fog = clamp((distance(in.world, scene_look.eye.xyz) - scene_look.fog.x) / span, 0.0, 1.0);
    }
    var out: LookOut;
    if scene_look.eye.w > 0.5 {
        if opened(in.world, in.normal, in.position.xy) {
            discard;
        }
        out = lit_look(in, front, straight * m.rgb, fog);
        out.mask.y = blur_of(in.world);
        return out;
    }
    let rgb = straight * m.rgb * shade_of(in.normal, front);
    if (draw.flags.z & 0xffu) == 1u && fog < 0.5 {
        // An accent in clear air: its own colour, kept as it is.
        out.color = vec4<f32>(rgb, 1.0);
        out.mask = vec2<f32>(1.0, 0.0);
    } else {
        out.color = vec4<f32>(mix(rgb, scene_look.fog_color.rgb, fog), 1.0);
        out.mask = vec2<f32>(0.0);
    }
    return out;
}

// The depth of a lit scene's surface for what stands among it, what the
// look's reveal opens left out (`DepthUse::Revealed`).
@fragment
fn fs_reveal_depth(in: Varyings) {
    if scene_look.eye.w > 0.5 && opened(in.world, in.normal, in.position.xy) {
        discard;
    }
}

// Whether this point is stippled away so the eye sees the look's reveal
// point through it: it stands higher than the point's feet (the point is
// taken as a chest, 1.2 over them), between the eye and the point, within the
// radius of the line of sight (the opening narrowing near both ends, so the
// ground at the point's feet and what is round the eye stay), dithered out
// toward the edge. Figures and the sky are never opened.
fn opened(world: vec3<f32>, normal: vec3<f32>, frag: vec2<f32>) -> bool {
    let role = draw.flags.z & 0xffu;
    if role == 2u || role == 4u {
        return false;
    }
    var fade = 0.0;
    for (var k = 0u; k < 4u; k = k + 1u) {
        fade = max(fade, opening(scene_look.reveal[k], world, normal));
    }
    return fade > bayer(vec2<u32>(floor(frag)));
}

// How far one reveal point `r` opens this point, 0..1.
fn opening(r: vec4<f32>, world: vec3<f32>, normal: vec3<f32>) -> f32 {
    if r.w <= 0.0 {
        return 0.0;
    }
    // What lies at or under the point's feet (the floor it stands on, the
    // ground before it) is never opened: only what rises in the way.
    let metre = max(scene_look.units.x, 1e-6);
    if world.y < r.y - 0.7 * metre {
        return 0.0;
    }
    // Nor ground rising a little before it: a face turned up stays unless it
    // is well over the point's head.
    if normalize(normal).y > 0.7 && world.y < r.y + 1.3 * metre {
        return 0.0;
    }
    let a = scene_look.eye.xyz;
    let ab = r.xyz - a;
    let t = dot(world - a, ab) / max(dot(ab, ab), 1e-6);
    if t <= 0.02 || t >= 1.0 {
        return 0.0;
    }
    let d = distance(world, a + ab * t);
    // The opening widens away from the eye and narrows to nothing in the
    // last stretch before the point (by distance, in metres, so a stage's
    // far pinhole and a near follow camera open alike): what stands right
    // at the point's feet stays.
    let before = (1.0 - t) * length(ab);
    let radius = r.w * smoothstep(0.02, 0.2, t) * smoothstep(0.3 * metre, 1.5 * metre, before);
    return 1.0 - smoothstep(radius * 0.55, radius, d);
}

// A pixel of a scene its look lights (`Look::lights`), made final here: lit
// by the sun and lamps, then put on its place's ramp, unless it is an
// accent (its nearest accent), a figure (its own colours, lit), water
// (bands and glints) or the sky (the ramp of where the eye is).
fn lit_look(in: Varyings, front: bool, base: vec3<f32>, fog: f32) -> LookOut {
    var out: LookOut;
    out.mask = vec2<f32>(0.5, 0.0);
    let n0 = normalize(in.normal);
    let n = select(-n0, n0, front);
    let role = draw.flags.z & 0xffu;
    // In a fragment the position is the pixel's middle in the target.
    let p = vec2<u32>(floor(in.position.xy));
    let xz = in.world.xz;
    if role == 1u && fog < 0.5 && scene_look.counts.w > 0u {
        var best = scene_look.accent[0].rgb;
        var best_distance = 1e9;
        for (var i = 0u; i < scene_look.counts.w; i = i + 1u) {
            let v = scene_look.accent[i].rgb - base;
            let d = dot(v, v);
            if d < best_distance {
                best_distance = d;
                best = scene_look.accent[i].rgb;
            }
        }
        out.color = vec4<f32>(best, 1.0);
        return out;
    }
    if role == 2u {
        // A figure: its own colours under the light where it stands, never
        // so dark it is lost, fading into the fog's brightness far off.
        let lit = lighting(in.world, n, true);
        let shade = clamp(0.5 + 0.55 * lit.x, 0.5, 1.15);
        let warm_tint = vec3<f32>(1.0, 0.86, 0.66);
        let c = base * shade * mix(vec3<f32>(1.0), warm_tint, clamp(lit.y * 0.6, 0.0, 0.6));
        let fogged = mix(c, scene_look.fog_color.rgb, fog * 0.85);
        out.color = vec4<f32>(fogged, 1.0);
        return out;
    }
    if role == 5u {
        // An ember: its own tone on the warm ramp, breathing slowly.
        let breath = 1.0 + 0.08 * sin(scene_look.fog.w * 1.7 + in.world.x * 0.05 + in.world.z * 0.07);
        out.color = vec4<f32>(on_ramp(luma(base) * breath, xz, 1.0, p, false), 1.0);
        return out;
    }
    if role == 4u {
        // The sky: its own brightness, on the ramp of where the eye stands.
        out.color = vec4<f32>(on_ramp(luma(base), scene_look.eye.xz, 0.0, p, false), 1.0);
        return out;
    }
    var lit = vec2<f32>(1.0, 0.0);
    if draw.flags.y == 1u || role == 3u {
        lit = lighting(in.world, n, false);
        lit.x = mix(lit.x, 1.0, draw.light.x);
    }
    var level = luma(base) * lit.x;
    if role == 3u {
        let w = water_of(in.world);
        // Grazing water shows the sky: brighter toward the far side.
        let to_eye = normalize(scene_look.eye.xyz - in.world);
        let sky = pow(1.0 - clamp(to_eye.y, 0.0, 1.0), 3.0);
        level = level * (0.82 + 0.22 * w.x) + 0.12 * sky;
        if w.y > 0.5 && fog < 0.5 && scene_look.counts.w > 0u {
            out.color = vec4<f32>(scene_look.accent[0].rgb, 1.0);
            return out;
        }
    }
    let warm = clamp(lit.y * 1.4, 0.0, 1.0);
    // The fog pulls the brightness toward its entry of the ramp here.
    if scene_look.fog.z > 0.5 {
        let fog_entry = u32(scene_look.fog_color.w);
        var weights = vec4<f32>(0.0);
        for (var z = 0u; z < scene_look.counts.y; z = z + 1u) {
            weights[z] = zone_weight(z, xz);
        }
        var fog_level = luma(entry_at(fog_entry, xz, weights, 0.0));
        if role == 3u && scene_look.sun_more.z > 0.5 {
            fog_level = luma(scene_look.water[fog_entry].rgb);
        }
        level = mix(level, fog_level, fog);
    }
    let own_water = role == 3u && scene_look.sun_more.z > 0.5;
    // The brand guide's ceiling (docs/BRAND.md): however it is lit, a
    // surface goes no brighter than its class's step of the ramp here.
    let ceiling = draw.flags.z >> 8u;
    if ceiling > 0u && !own_water {
        let last = max(scene_look.counts.x, 1u) - 1u;
        let step = min(u32(round(f32(ceiling - 1u) * f32(last) / 7.0)), last);
        var weights = vec4<f32>(0.0);
        for (var z = 0u; z < scene_look.counts.y; z = z + 1u) {
            weights[z] = zone_weight(z, xz);
        }
        level = min(level, luma(entry_at(step, xz, weights, 0.0)));
    }
    out.color = vec4<f32>(on_ramp(level, xz, warm * (1.0 - fog), p, own_water), 1.0);
    return out;
}

// A figure's card in a world its look lights (`Look::cards`), drawn after
// the world at the stage texture's own size, through the plain pipelines'
// blend and depth (`fs_main`'s), with the look's group 3. A card is lit as
// an upright face turned to the eye, at the point of it this is, by the
// look's sun (and its shadows) and lamps; one with a normal map (`flags.w`
// 2, group 2) is lit pixel by pixel, each pixel that face turned by its
// normal (`card_normal`). The sun's
// light is taken as a part of a reference, halfway between what the card
// would get in the open (the ambient and the sun on that face) and full
// light, held between the least and as drawn; the lamps add to it, up to
// the most, and move its colour toward the warm ramp's hue at its
// brightness, and a lamp behind it lights its outline. Its pixels
// keep their own colours: the light multiplies them, never puts them on a
// ramp. An accent card and a pixel stored a step under opaque (a signal
// ink, angel_stage::look::UNLIT_ALPHA) stay as drawn.
const CARD_REFERENCE: f32 = 0.5;

@fragment
fn fs_card(in: Varyings) -> @location(0) vec4<f32> {
    let texel = textureSampleLevel(picture, picture_sampler, in.uv, 0.0);
    let m = in.color * draw.tint;
    let straight = select(texel.rgb, texel.rgb / texel.a, texel.a > 0.0);
    var alpha = 1.0;
    if draw.flags.x != 0u {
        alpha = texel.a * m.a;
    }
    let ink = texel.a > 0.993 && texel.a < 0.999;
    if (draw.flags.z & 0xffu) == 1u || ink || scene_look.cards.y <= 0.0 {
        let a = select(alpha, m.a, ink);
        return vec4<f32>(straight * m.rgb * a, a);
    }
    if alpha <= 0.0 {
        discard;
    }
    let eye = scene_look.eye.xyz;
    // The upright face turned to the eye, and this pixel's own way.
    var n0 = vec3<f32>(eye.x - in.world.x, 0.0, eye.z - in.world.z);
    n0 = select(vec3<f32>(0.0, 0.0, 1.0), n0 / max(length(n0), 1e-6), length(n0) > 1e-3);
    let n = card_normal(in.uv, n0);
    // The sky and the sun on it, against the same in the open (on the
    // upright face: the reference is the card's, not the pixel's).
    var sun = 1.0;
    var open = 1.0;
    if scene_look.flags.w == 1u {
        var sky = 1.0;
        if scene_look.sun_more.w > 0.0 {
            sky = mix(0.6, 1.0, smoothstep(0.0, scene_look.sun_more.w, in.world.y));
        }
        let facing = max(dot(n, scene_look.sun.xyz), 0.0);
        open = scene_look.sun_more.x * sky + scene_look.sun.w * max(dot(n0, scene_look.sun.xyz), 0.0);
        sun = scene_look.sun_more.x * sky + scene_look.sun.w * facing * sunlit(in.world, n);
    }
    let reference = max(mix(open, 1.0, CARD_REFERENCE), 1e-3);
    // The lamps, reaching a card `cards_more.x` times as far as the floor
    // (an upright figure faces the flame the floor lies under), a lamp
    // behind it still lighting it a little; and how far behind the card
    // they are, for its rim, the more on the side the pixel turns to.
    var lamps = 0.0;
    var behind = 0.0;
    let lateral = n - n0 * dot(n, n0);
    for (var i = 0u; i < scene_look.flags.z; i = i + 1u) {
        let at = scene_look.lamps[2u * i];
        let strength = scene_look.lamps[2u * i + 1u].x;
        let to = at.xyz - in.world;
        let d = length(to);
        let dir = to / max(d, 1e-6);
        let fall = clamp(1.0 - d / max(at.w * scene_look.cards_more.x, 1e-6), 0.0, 1.0);
        // The upright face's wrapped light, and the pixel's turn from that
        // face taken at a plain Lambert's slope (none when it faces as the
        // card does), so a turned pixel reads its lamp's side.
        let toward0 = max(dot(n0, dir), 0.0);
        let turn = max(dot(n, dir), 0.0) - toward0;
        lamps = lamps + strength * fall * fall * max(0.45 + 0.55 * toward0 + turn, 0.1);
        let side = clamp(1.0 + 1.5 * dot(lateral, dir), 0.0, 1.5);
        behind = behind + strength * fall * max(-dot(n0, dir), 0.0) * side;
    }
    let shade = clamp(clamp(sun / reference, scene_look.cards.x, 1.0) + lamps, scene_look.cards.x, scene_look.cards.y);
    var c = straight * m.rgb * shade;
    // The lamps' warmth: toward the warm ramp's hue at the pixel's own
    // brightness, as far as they would warm the world (`lit_look`), by
    // `cards.z` of it.
    var warm = clamp(lamps * 1.4, 0.0, 1.0) * scene_look.cards.z;
    // A lamp behind the card lights its outline: the pixels with a clear
    // neighbour.
    if scene_look.cards.w > 0.0 && behind > 0.0 {
        let step = 1.0 / vec2<f32>(textureDimensions(picture));
        let clear = textureSampleLevel(picture, picture_sampler, in.uv + vec2<f32>(step.x, 0.0), 0.0).a
            * textureSampleLevel(picture, picture_sampler, in.uv - vec2<f32>(step.x, 0.0), 0.0).a
            * textureSampleLevel(picture, picture_sampler, in.uv + vec2<f32>(0.0, step.y), 0.0).a
            * textureSampleLevel(picture, picture_sampler, in.uv - vec2<f32>(0.0, step.y), 0.0).a;
        if clear < 0.5 {
            let rim = clamp(behind * scene_look.cards.w, 0.0, 1.0);
            c = c * (1.0 + rim);
            warm = max(warm, rim);
        }
    }
    if scene_look.flags.x == 1u && warm > 0.0 {
        let hue = warm_at(luma(c));
        c = mix(c, luma(c) * hue / max(luma(hue), 1e-4), warm);
    }
    var fog = 0.0;
    if scene_look.fog.z > 0.5 {
        let span = max(scene_look.fog.y - scene_look.fog.x, 1e-6);
        fog = clamp((distance(in.world, eye) - scene_look.fog.x) / span, 0.0, 1.0);
    }
    c = mix(c, scene_look.fog_color.rgb, fog * 0.85);
    return vec4<f32>(min(c, vec3<f32>(1.0)) * alpha, alpha);
}

// The way a card's pixel faces: the upright face `n0` turned by the card's
// normal map (group 2's picture, when `flags.w` is 2), its red along the
// picture's right and its green up, as far as the look says
// (`cards_more.y`: 0 not at all, 1 as the map has it). Drawn nearest: one
// normal a pixel of the picture.
fn card_normal(uv: vec2<f32>, n0: vec3<f32>) -> vec3<f32> {
    let strength = scene_look.cards_more.y;
    if draw.flags.w != 2u || strength <= 0.0 {
        return n0;
    }
    let t = textureSampleLevel(patch_picture, patch_sampler, uv, 0.0).rgb * 2.0 - vec3<f32>(1.0);
    // The picture's right as it stands (the card's x), square to the face.
    let x_axis = draw.model[0].xyz;
    let flat_right = x_axis - n0 * dot(x_axis, n0);
    let right = select(cross(vec3<f32>(0.0, 1.0, 0.0), n0), normalize(flat_right), length(flat_right) > 1e-4);
    let up = cross(n0, right);
    let turned = vec3<f32>(t.xy * strength, max(t.z, 0.0));
    let local = select(vec3<f32>(0.0, 0.0, 1.0), normalize(turned), length(turned) > 1e-4);
    return normalize(right * local.x + up * local.y + n0 * local.z);
}

// The warm ramp's colour at brightness `level`: between the two entries
// whose brightnesses hold it, in proportion (no dither: a card is never
// put on a ramp, only given its hue).
fn warm_at(level: f32) -> vec3<f32> {
    let n = max(scene_look.counts.x, 1u);
    var lo = scene_look.warm[0].rgb;
    var lo_level = luma(lo);
    if level <= lo_level {
        return lo;
    }
    for (var i = 1u; i < n; i = i + 1u) {
        let hi = scene_look.warm[i].rgb;
        let hi_level = max(luma(hi), lo_level);
        if level < hi_level {
            return mix(lo, hi, (level - lo_level) / max(hi_level - lo_level, 1e-6));
        }
        lo = hi;
        lo_level = hi_level;
    }
    return lo;
}

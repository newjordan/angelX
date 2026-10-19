// The mesh pipeline: a 3D scene's triangles (angel-stage's `scene`).
// texture x vertex colour x tint, shaded by one fixed light when the material
// is lit, out premultiplied to match the target's blend (One,
// OneMinusSrcAlpha). One draw is one range of one mesh with one material; its
// uniforms sit at a dynamic offset in one buffer.

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
    // when the picture is cast from `projector` instead of laid on by uv.
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
    var out: Varyings;
    let world = draw.model * vec4<f32>(v.position, 1.0);
    out.position = draw.view_proj * world;
    out.uv = v.uv;
    out.color = v.color;
    // The normal through the model's cofactor matrix (the inverse transpose
    // times the determinant), so a stretched box keeps normals square to its
    // faces; a mirrored model (a negative determinant) is turned back.
    let a = draw.model[0].xyz;
    let b = draw.model[1].xyz;
    let c = draw.model[2].xyz;
    let cofactor = mat3x3<f32>(cross(b, c), cross(c, a), cross(a, b));
    let side = select(1.0, -1.0, dot(a, cross(b, c)) < 0.0);
    out.normal = cofactor * v.normal * side;
    out.world = world.xyz;
    out.projector_eye = vec4<f32>(0.0);
    out.camera_eye = vec4<f32>(0.0);
    if draw.flags.w == 1u {
        out.projector_eye = eye_of(draw.projector);
        out.camera_eye = eye_of(draw.view_proj);
    }
    return out;
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
};

@group(3) @binding(0) var<uniform> scene_look: SceneLook;
@group(3) @binding(1) var shadow_map: texture_depth_2d;
@group(3) @binding(2) var shadow_sampler: sampler_comparison;

struct LookOut {
    // Straight colour as drawn and fogged, alpha 1; or, when the look
    // lights the scene, the pixel's final colour.
    @location(0) color: vec4<f32>,
    // 1 where an accent surface was drawn and is not lost in the fog, 0.5
    // where the pixel is already final (look.wgsl leaves it), else 0.
    @location(1) mask: f32,
};

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
        return lit_look(in, front, straight * m.rgb, fog);
    }
    let rgb = straight * m.rgb * shade_of(in.normal, front);
    if (draw.flags.z & 0xffu) == 1u && fog < 0.5 {
        // An accent in clear air: its own colour, kept as it is.
        out.color = vec4<f32>(rgb, 1.0);
        out.mask = 1.0;
    } else {
        out.color = vec4<f32>(mix(rgb, scene_look.fog_color.rgb, fog), 1.0);
        out.mask = 0.0;
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
    out.mask = 0.5;
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

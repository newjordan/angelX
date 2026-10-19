// One pipeline for everything in the window: every rectangle is an instance.
// kind 0: a solid rectangle (backgrounds, lines, the cursor, solid layers)
// kind 1: an alpha mask from the atlas, tinted (text, box drawing, braille)
// kind 2: a colour picture from the atlas (emoji)
// kind 3: a picture texture bound in the atlas slot, sampled
// kind 4: the same (a scene drawn into a picture)
// kind 11: curved foreground parchment with fixed material coordinates
// kind 12: one textured mesh triangle in window pixels
// kinds 16..22: the seven ray-marched solids (LayerKind::Primitive)
// Every texture is stored premultiplied; the target blends premultiplied.

struct Globals {
    screen: vec2<f32>,
    // x: the text coverage curve (1 = as rasterized).
    text: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) kind: u32,
    @location(4) pivot: vec2<f32>,
    @location(5) angle: f32,
    // A stage seen at a tilt: x, y the floor's centre and the row that keeps
    // its size, z how much nearer each row down comes, w the zoom.
    @location(6) tilt: vec4<f32>,
    // x: 0 untouched, 1 on the floor, 2 standing on it at (y, z); w the
    // floor's foreshortening down the screen.
    @location(7) stand: vec4<f32>,
};

struct Varyings {
    @builtin(position) position: vec4<f32>,
    // Over the depth (`q`): the fragment divides it back (`fs_main`).
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) kind: u32,
    @location(3) @interpolate(flat) surface: vec4<f32>,
    // One over the depth `w` the vertex stood at: 1 off a tilted floor.
    @location(4) q: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32, instance: Instance) -> Varyings {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[vertex];
    if instance.kind == 12u {
        // Three textured mesh vertices encoded in an existing instance. The
        // second triangle is degenerate: the compositor still draws six vertices.
        var points = array<vec2<f32>, 3>(instance.rect.xy, instance.rect.zw, instance.pivot);
        var texels = array<vec2<f32>, 3>(instance.uv.xy, instance.uv.zw, instance.tilt.xy);
        let i = min(vertex, 2u);
        let p = points[i];
        var mesh: Varyings;
        mesh.position = vec4<f32>(p.x / globals.screen.x * 2.0 - 1.0, 1.0 - p.y / globals.screen.y * 2.0, 0.0, 1.0);
        mesh.uv = texels[i];
        mesh.color = instance.color;
        mesh.kind = instance.kind;
        mesh.surface = vec4<f32>(0.0);
        mesh.q = 1.0;
        return mesh;
    }
    // Rotate about the pivot (transitions); angle 0 leaves it where it is.
    let local = instance.rect.xy + corner * instance.rect.zw - instance.pivot;
    let c = cos(instance.angle);
    let s = sin(instance.angle);
    let pixel = instance.pivot + vec2<f32>(local.x * c - local.y * s, local.x * s + local.y * c);
    // On a tilted floor: rows further up stand further away. The floor is
    // projected (w carries the depth, so pictures stay true in perspective);
    // what stands is placed on its foot and scaled, never tipped.
    var w = 1.0;
    var at = pixel;
    if instance.stand.x > 2.5 {
        // A foreground paper strip, parameterized by true released arc length.
        // Keep this projection paired with glass's scroll::surface / Page::hit.
        let box = instance.tilt;
        let open = clamp(instance.stand.y, 0.0, 1.025);
        let radius = 0.036;
        let edge = radius * 1.35;
        let top = radius * 1.15;
        let travel = max(open * (1.0 - 2.0 * top), 0.002);
        let d = clamp((pixel.y - box.y) / box.w, 0.0, travel);
        var yy = top + d;
        var z = 0.0;
        if d < edge {
            let a = (1.0 - d / edge) * 0.95;
            yy = top + edge - radius * sin(a) / 0.95 * 1.35;
            z = radius * (1.0 - cos(a));
        } else if d > travel - edge {
            let a = clamp((d - (travel - edge)) / edge, 0.0, 1.0) * 1.25;
            yy = top + travel - edge + radius * sin(a) * 1.35 / 1.25;
            z = radius * (1.0 - cos(a));
        }
        yy = yy - pow(1.0 - min(open, 1.0), 2.0) * 0.10 + instance.stand.z * d;
        w = 1.0 - z * 0.65;
        let centre = box.x + box.z * 0.5;
        at = vec2<f32>(centre * w + pixel.x - centre, (box.y + yy * box.w) * w);
    } else if instance.stand.x > 0.5 {
        let centre = instance.tilt.xy;
        let near = instance.tilt.z;
        let zoom = instance.tilt.w;
        let squash = instance.stand.w;
        if instance.stand.x < 1.5 {
            let d = pixel - centre;
            w = max(1.0 - near * d.y, 0.05);
            at = vec2<f32>(centre.x * w + zoom * d.x, centre.y * w + zoom * squash * d.y);
        } else {
            let foot = instance.stand.yz;
            let a = foot - centre;
            let depth = max(1.0 - near * a.y, 0.05);
            let placed = centre + vec2<f32>(zoom * a.x, zoom * squash * a.y) / depth;
            at = placed + (pixel - foot) * (zoom / depth);
        }
    }
    // The depth is divided out here, not left to the rasterizer: every
    // vertex leaves with w = 1, and the texel coordinates leave over w with
    // 1/w beside them for the fragment to divide back, which is the
    // perspective-correct lookup done by hand. A browser's WebGL drew some
    // sessions' tilted floor with the lookup wrong when w varied across a
    // quad (texels at uv / w^2: the whole floor leaning right, 2026-10-17),
    // with the same calls and bytes as the sessions it drew right. w is never
    // below 0.05 (the floor clamps it), so nothing crosses the eye.
    let q = 1.0 / w;
    var out: Varyings;
    out.position = vec4<f32>(
        (at.x / globals.screen.x * 2.0 - w) * q,
        (w - at.y / globals.screen.y * 2.0) * q,
        0.0,
        1.0,
    );
    out.uv = instance.uv.xy + corner * instance.uv.zw;
    out.color = instance.color;
    out.kind = instance.kind;
    out.surface = instance.uv;
    if instance.kind >= 16u && instance.kind <= 22u {
        let aspect = instance.uv.z;
        out.uv = (corner - vec2<f32>(0.5)) * 2.8
            * vec2<f32>(max(aspect, 1.0), max(1.0 / aspect, 1.0));
    }
    out.uv = out.uv * q;
    out.q = q;
    return out;
}

fn object_point(p: vec3<f32>, phase: f32) -> vec3<f32> {
    let c = cos(phase);
    let s = sin(phase);
    let q = vec3<f32>(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
    let tilt = 0.36 + 0.18 * sin(phase);
    return vec3<f32>(q.x, cos(tilt) * q.y - sin(tilt) * q.z,
        sin(tilt) * q.y + cos(tilt) * q.z);
}

// Keep paired with angel_stage::primitive::distance (the quad-only hosts' copy).
fn object_distance(p: vec3<f32>, shape: u32, phase: f32) -> f32 {
    let q = object_point(p, phase);
    let r = length(q.xz);
    switch shape {
        case 0u: {
            let d = abs(q) - vec3<f32>(0.62);
            return length(max(d, vec3<f32>(0.0))) + min(max(d.x, max(d.y, d.z)), 0.0) - 0.04;
        }
        case 1u: { return length(q) - 0.82; }
        case 2u: { return max(max((max(abs(q.x), abs(q.z)) + 0.48 * q.y - 0.39) / 1.11, -q.y - 0.72), q.y - 0.82); }
        case 3u: { return max(r - 0.62, abs(q.y) - 0.73); }
        case 4u: { return max(max((r + 0.45 * q.y - 0.37) / 1.10, -q.y - 0.74), q.y - 0.82); }
        case 5u: { return length(vec2<f32>(r - 0.57, q.y)) - 0.25; }
        default: { return (abs(q.x) + abs(q.y) + abs(q.z) - 1.12) / sqrt(3.0); }
    }
}

fn solid_object(in: Varyings) -> vec4<f32> {
    let shape = in.kind - 16u;
    let phase = in.surface.x;
    let origin = vec3<f32>(in.uv.x, -in.uv.y, 2.5);
    var z = 0.0;
    var nearest = 10.0;
    var hit = false;
    // Bounded sphere tracing. Track closest approach for the outer rim.
    for (var i = 0; i < 80; i = i + 1) {
        let p = origin - vec3<f32>(0.0, 0.0, z);
        let d = object_distance(p, shape, phase);
        nearest = min(nearest, d);
        if d < 0.0025 { hit = true; break; }
        z = z + max(d * 0.85, 0.002);
        if z > 4.2 { break; }
    }
    let tint = in.color.rgb;
    if !hit {
        let halo = exp(-nearest * nearest * 180.0) * 0.42
            + exp(-nearest * nearest * 2800.0) * 0.55;
        // Premultiplied emission adds without an opaque rectangular backing.
        return vec4<f32>(tint * halo * in.color.a, 0.0);
    }
    let p = origin - vec3<f32>(0.0, 0.0, z);
    let e = 0.004;
    let n = normalize(vec3<f32>(
        object_distance(p + vec3<f32>(e, 0.0, 0.0), shape, phase) - object_distance(p - vec3<f32>(e, 0.0, 0.0), shape, phase),
        object_distance(p + vec3<f32>(0.0, e, 0.0), shape, phase) - object_distance(p - vec3<f32>(0.0, e, 0.0), shape, phase),
        object_distance(p + vec3<f32>(0.0, 0.0, e), shape, phase) - object_distance(p - vec3<f32>(0.0, 0.0, e), shape, phase)));
    let light = normalize(vec3<f32>(-0.5, 0.7, 1.1));
    let diffuse = max(dot(n, light), 0.0);
    let specular = pow(max(dot(n, normalize(light + vec3<f32>(0.0, 0.0, 1.0))), 0.0), 48.0);
    let rim = pow(1.0 - abs(n.z), 2.4);
    let q = object_point(p, phase);
    let seam = pow(0.5 + 0.5 * cos(q.y * 18.0 + atan2(q.z, q.x) * 3.0), 20.0);
    let rgb = tint * (0.13 + 0.62 * diffuse + 0.6 * rim + 0.13 * seam)
        + vec3<f32>(specular * 0.75 + rim * 0.25);
    return vec4<f32>(rgb * in.color.a, in.color.a);
}

@fragment
fn fs_main(arriving: Varyings) -> @location(0) vec4<f32> {
    // The texel coordinates back from over the depth (`vs_main`).
    var in = arriving;
    in.uv = arriving.uv / arriving.q;
    if in.kind == 11u {
        // Fibre, warm edge wear, and very fine laid-paper grain. Fixed material
        // coordinates keep it attached to the sheet throughout unrolling.
        let noise = fract(sin(dot(floor(in.uv * vec2<f32>(900.0, 1400.0)),
            vec2<f32>(12.9898, 78.233))) * 43758.5453);
        let fibres = 0.985 + 0.015 * sin(in.uv.y * 3100.0 + sin(in.uv.x * 170.0) * 0.7);
        let edge = smoothstep(0.0, 0.035, min(in.uv.x, 1.0 - in.uv.x));
        let tone = (0.965 + noise * 0.045) * fibres * (0.90 + 0.10 * edge);
        return vec4<f32>(in.color.rgb * tone * in.color.a, in.color.a);
    }
    if in.kind == 12u {
        // Texels are premultiplied in storage; the tint lights them.
        let size = vec2<f32>(textureDimensions(atlas));
        let c = textureSampleLevel(atlas, picture_sampler, in.uv / size, 0.0);
        return vec4<f32>(c.rgb * in.color.rgb * in.color.a, c.a * in.color.a);
    }
    if in.kind >= 16u && in.kind <= 22u {
        return solid_object(in);
    }
    if in.kind == 0u {
        return in.color;
    }
    if in.kind == 3u {
        // Not uniform control flow, so an explicit level, no derivatives.
        // Texels are premultiplied in storage (uploads convert, stage
        // targets render so), so no rgb*alpha here — that would darken
        // every translucent texel twice.
        let size = vec2<f32>(textureDimensions(atlas));
        let c = textureSampleLevel(atlas, picture_sampler, in.uv / size, 0.0);
        return c * in.color.a;
    }
    if in.kind == 4u {
        let size = vec2<f32>(textureDimensions(atlas));
        let c = textureSampleLevel(atlas, picture_sampler, in.uv / size, 0.0);
        return c * in.color.a;
    }
    if in.kind == 5u {
        // An uploaded picture stored straight under
        // ANGEL_GLASS_GPU_PREMULTIPLY: premultiplied here instead.
        let size = vec2<f32>(textureDimensions(atlas));
        let c = textureSampleLevel(atlas, picture_sampler, in.uv / size, 0.0);
        return vec4<f32>(c.rgb * c.a, c.a) * in.color.a;
    }
    let texel = textureLoad(atlas, vec2<i32>(floor(in.uv)), 0);
    if in.kind == 1u {
        return in.color * pow(texel.a, globals.text.x);
    }
    return texel * in.color.a;
}

// The Round Table, seen from above at dusk: the council the live stage has
// seated, drawn on black paper in the realm palette
// (cockpit/assets/realm/palette.json, named by the overworld's inks).
//
// Light means activity. A running seat's candle burns and pools dithered
// firelight on the timber; a returned seat's answer lies on the table as
// parchment; a failed one lies there in amber; a cut seat sits in the dark.
// The centre of the table shows the deed: scales while judges weigh, a wax
// seal while verifiers check, the gathered answers during synthesis. The picture is 160x90 art pixels, each 2x2 in the 320x180 frame.

struct PortalState {
    seats: u32,
    stage_kind: u32,
    reserved0: u32,
    reserved1: u32,
    states: array<vec4<u32>, 4>,
}

@group(0) @binding(0)
var<uniform> state: PortalState;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var output: VertexOut;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return output;
}

const PI: f32 = 3.14159265;
const CENTER: vec2<f32> = vec2<f32>(80.0, 46.0);
const TABLE: vec2<f32> = vec2<f32>(42.0, 22.0);
const SEAT_RING: vec2<f32> = vec2<f32>(58.0, 33.0);
const DAIS: vec2<f32> = vec2<f32>(72.0, 41.0);

const RUNNING: u32 = 0u;
const RETURNED: u32 = 1u;
const FAILED: u32 = 2u;
const CUT: u32 = 3u;
const STAGE_SYNTHESIS: u32 = 1u;
const STAGE_JUDGE: u32 = 2u;
const STAGE_VERIFY: u32 = 3u;

// Inks, by the overworld's names (stage/world_viz/overworld/ink.rs).
const INK_BLACK: u32 = 0x000000u;
const INK_k: u32 = 0x0b0a08u;
const INK_K: u32 = 0x151614u;
const INK_X: u32 = 0x322b2bu;
const INK_g: u32 = 0x3d3a36u;
const INK_h: u32 = 0x94968cu;
const INK_i: u32 = 0xb0b2a8u;
const INK_H: u32 = 0xcdcfc6u;
const INK_n: u32 = 0x1a0e07u;
const INK_b: u32 = 0x331a0cu;
const INK_I: u32 = 0x452712u;
const INK_p: u32 = 0x652c10u;
const INK_R: u32 = 0x924f1du;
const INK_o: u32 = 0xa97639u;
const INK_O: u32 = 0xbe914bu;
const INK_j: u32 = 0x515349u;
// Signal bank: meaning only.
const INK_a: u32 = 0x7a4a12u;
const INK_4: u32 = 0xecb64au;
const INK_6: u32 = 0xfbd070u;
const INK_dollar: u32 = 0xc7b48fu;
const INK_c: u32 = 0xe3d2c3u;

fn rgb(v: u32) -> vec3<f32> {
    return vec3<f32>(
        f32((v >> 16u) & 255u),
        f32((v >> 8u) & 255u),
        f32(v & 255u),
    ) / 255.0;
}

/// Timber bank, dark to light: n b I B p r R o O t.
fn timber(index: i32) -> u32 {
    var ramp = array<u32, 10>(
        0x1a0e07u, 0x331a0cu, 0x452712u, 0x582b13u, 0x652c10u,
        0x784620u, 0x924f1du, 0xa97639u, 0xbe914bu, 0xd8a95eu,
    );
    return ramp[clamp(index, 0, 9)];
}

/// Structure greys, dark to light: black k K X g j G J.
fn stone(index: i32) -> u32 {
    var ramp = array<u32, 8>(
        0x000000u, 0x0b0a08u, 0x151614u, 0x322b2bu,
        0x3d3a36u, 0x515349u, 0x63655cu, 0x7a7c72u,
    );
    return ramp[clamp(index, 0, 7)];
}

/// Surcoats: identity, never state, so none come from the signal bank.
/// Stone Q u S, foliage m e, timber R P.
fn surcoat(seat: u32) -> u32 {
    var coats = array<u32, 7>(
        0x527597u, 0x5a7330u, 0x924f1du, 0x456070u,
        0x35543fu, 0x684629u, 0x2f4a52u,
    );
    return coats[seat % 7u];
}

fn bayer(p: vec2<i32>) -> f32 {
    var m = array<f32, 16>(
        0.0, 8.0, 2.0, 10.0,
        12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0,
        15.0, 7.0, 13.0, 5.0,
    );
    return (m[(p.y & 3) * 4 + (p.x & 3)] + 0.5) / 16.0;
}

fn hash(x: i32, y: i32, salt: u32) -> u32 {
    var h = u32(x) * 374761393u + u32(y) * 668265263u + salt * 2246822519u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return h ^ (h >> 16u);
}

fn ellipse(p: vec2<f32>, center: vec2<f32>, radii: vec2<f32>) -> f32 {
    let q = (p - center) / radii;
    return dot(q, q);
}

fn seat_count() -> u32 {
    return min(state.seats, 16u);
}

fn seat_state(seat: u32) -> u32 {
    return state.states[seat / 4u][seat % 4u];
}

/// Seat 0 is the head of the table, at the top; the rest follow clockwise.
fn seat_angle(seat: u32) -> f32 {
    return -0.5 * PI + f32(seat) * 2.0 * PI / f32(max(seat_count(), 1u));
}

fn seat_position(seat: u32) -> vec2<f32> {
    let a = seat_angle(seat);
    return CENTER + vec2<f32>(cos(a) * SEAT_RING.x, sin(a) * SEAT_RING.y);
}

/// Where a seat's candle stands, or its answer lies: on the table before it.
fn place_setting(seat: u32) -> vec2<f32> {
    let a = seat_angle(seat);
    return floor(CENTER + vec2<f32>(cos(a) * TABLE.x * 0.74, sin(a) * TABLE.y * 0.66));
}

/// Firelight from every burning candle, 0..1. The table is foreshortened, so
/// a round pool on it is an ellipse on screen.
fn firelight(p: vec2<f32>) -> f32 {
    var light = 0.0;
    for (var seat: u32 = 0u; seat < 16u; seat = seat + 1u) {
        if (seat < seat_count() && seat_state(seat) == RUNNING) {
            let d = length((p - place_setting(seat) - vec2<f32>(0.5, -1.0)) * vec2<f32>(1.0, 1.8));
            light += pow(max(0.0, 1.0 - d / 17.0), 1.5);
        }
    }
    return min(light, 1.0);
}

/// One step up its ramp per dithered third of light.
fn lit_steps(light: f32, t: f32) -> i32 {
    return i32(floor(light * 2.6 + t));
}

/// A candle: two pixels of wax and a flame above them.
fn candle(d: vec2<i32>) -> u32 {
    if (d.y >= -2 && d.y <= 0 && d.x >= 0 && d.x <= 1) {
        return select(INK_i, INK_h, d.x == 1);
    }
    if (d.y == -3 && d.x >= 0 && d.x <= 1) {
        return INK_4;
    }
    if (d.y == -4 && d.x == 0) {
        return INK_6;
    }
    return 0u;
}

/// A sheet laid on the table: 5x3, one written line. Amber when it failed.
fn parchment(d: vec2<i32>, failed: bool) -> u32 {
    if (d.x < -2 || d.x > 2 || d.y < -1 || d.y > 1) {
        return 0u;
    }
    if (d.y == 0 && d.x >= -1 && d.x <= 1) {
        return select(INK_dollar, INK_4, failed);
    }
    return select(INK_c, INK_a, failed);
}

/// Scales standing on the boss: a post, a beam, two brass pans on chains.
fn scales(d: vec2<i32>) -> u32 {
    if (d.x == 0 && d.y >= -5 && d.y <= 0) {
        return INK_h;
    }
    if (d.y == -5 && abs(d.x) <= 4) {
        return INK_h;
    }
    if (abs(d.x) == 4 && (d.y == -4 || d.y == -3)) {
        return INK_j;
    }
    if (d.y == -2 && abs(d.x) >= 3 && abs(d.x) <= 5) {
        return INK_O;
    }
    if (d.y == 1 && abs(d.x) <= 1) {
        return INK_I;
    }
    return 0u;
}

/// A wax seal pressed on the boss, its cross impressed, lit from the upper
/// left.
fn seal(p: vec2<f32>) -> u32 {
    let q = p - CENTER - vec2<f32>(0.0, -0.5);
    let r = length(q * vec2<f32>(1.0, 1.4));
    if (r > 3.9) {
        return 0u;
    }
    if (r <= 2.3 && (abs(q.x) < 0.6 || abs(q.y) < 0.5)) {
        return INK_p;
    }
    if (r > 2.9 && q.x < 0.0 && q.y < 0.0) {
        return INK_o;
    }
    return INK_R;
}

/// A knight from above: shoulders in his surcoat, a steel helm with the
/// visor turned to the table, and a dark edge around him.
struct Knight {
    ink: u32,
    inside: bool,
}

/// 0 outside, 1 surcoat, 2 helm, 3 pauldron.
fn knight_shape(q: vec2<f32>, out_dir: vec2<f32>) -> i32 {
    let across = dot(q, vec2<f32>(-out_dir.y, out_dir.x));
    let along = dot(q, out_dir);
    let helm = q - out_dir * 0.4;
    if (dot(helm, helm) <= 2.3 * 2.3) {
        return 2;
    }
    let shoulder = vec2<f32>(abs(across) - 4.6, along + 0.3);
    if (dot(shoulder, shoulder) <= 1.3 * 1.3) {
        return 3;
    }
    if ((across / 5.8) * (across / 5.8) + (along / 3.3) * (along / 3.3) <= 1.0) {
        return 1;
    }
    return 0;
}

fn knight(p: vec2<f32>, seat: u32) -> Knight {
    let center = seat_position(seat);
    let out_dir = normalize(center - CENTER);
    let q = p - center;
    let shape = knight_shape(q, out_dir);
    let seat_is = seat_state(seat);
    let dim = seat_is == CUT;
    if (shape == 0) {
        var offsets = array<vec2<f32>, 4>(
            vec2<f32>(1.0, 0.0), vec2<f32>(-1.0, 0.0),
            vec2<f32>(0.0, 1.0), vec2<f32>(0.0, -1.0),
        );
        var touching = false;
        for (var k: i32 = 0; k < 4; k = k + 1) {
            if (knight_shape(q + offsets[k], out_dir) != 0) {
                touching = true;
            }
        }
        return Knight(INK_k, touching);
    }
    if (shape == 2 || shape == 3) {
        if (dim) {
            return Knight(INK_g, true);
        }
        // Steel catches the light from the upper left.
        let local = select(q - out_dir * 0.4, q, shape == 3);
        if (shape == 2 && local.x < -0.4 && local.y < -0.4) {
            return Knight(select(INK_i, INK_H, seat_is == RUNNING), true);
        }
        return Knight(select(INK_h, INK_g, shape == 3 && local.y > 0.8), true);
    }
    if (dim) {
        return Knight(INK_X, true);
    }
    return Knight(surcoat(seat), true);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let art = floor(position.xy / 2.0);
    let ip = vec2<i32>(art);
    let p = art + vec2<f32>(0.5);
    let t = bayer(ip);
    let light = firelight(p);

    // Black paper, with the hall's flagstone joints as sparse marks that
    // fade out toward the frame's edge.
    var ink = INK_BLACK;
    let edge = length((p - CENTER) / vec2<f32>(80.0, 45.0));
    if (hash(ip.x / 3, ip.y / 3, 7u) % 29u == 0u && t > smoothstep(0.75, 1.05, edge)) {
        ink = INK_K;
    }

    // The dais: its flagstone joints drawn as broken marks on the paper,
    // warmed where a candle reaches, inside one ruled stone edge.
    let on_dais = ellipse(p, CENTER + vec2<f32>(0.0, 1.0), DAIS);
    if (on_dais <= 1.0) {
        let row = ip.y / 5;
        let joint = ip.y % 5 == 0 || (ip.x + row * 7) % 11 == 0;
        ink = INK_BLACK;
        if (joint && hash(ip.x / 2, ip.y, 11u) % 5u < 2u) {
            ink = stone(2 + lit_steps(light, t));
        }
        if (on_dais > 0.955) {
            ink = INK_X;
        }
    }

    // The table's shadow falls on the dais.
    let on_table = ellipse(p, CENTER, TABLE);
    if (on_table > 1.0 && ellipse(p, CENTER + vec2<f32>(0.0, 3.0), TABLE) <= 1.0) {
        ink = INK_k;
    }

    // The table: timber boards at dusk, a dark rim, the boss at its heart.
    if (on_table <= 1.0) {
        let board = ip.y / 4;
        let seam = ip.y % 4 == 0 || (ip.x + board * 9) % 17 == 0;
        var base = select(3, 2, seam);
        if (!seam && hash(ip.x / 2, ip.y, 3u) % 7u == 0u) {
            base = 2;
        }
        ink = timber(base + lit_steps(light, t));
        if (on_table > 0.84) {
            ink = select(INK_b, INK_n, p.y > CENTER.y);
        }
        let boss = ellipse(p, CENTER, vec2<f32>(7.0, 3.8));
        if (boss <= 1.0) {
            ink = select(INK_I, INK_n, boss > 0.6);
        }
    }

    // The deed at the centre of the table.
    if (state.stage_kind == STAGE_JUDGE) {
        let beam = scales(ip - vec2<i32>(CENTER));
        if (beam != 0u) {
            ink = beam;
        }
    }
    if (state.stage_kind == STAGE_VERIFY) {
        let wax = seal(p);
        if (wax != 0u) {
            ink = wax;
        }
    }
    if (state.stage_kind == STAGE_SYNTHESIS) {
        var stack = array<vec2<i32>, 3>(vec2<i32>(-5, -1), vec2<i32>(4, -2), vec2<i32>(0, 0));
        for (var k: i32 = 0; k < 3; k = k + 1) {
            let sheet = parchment(ip - vec2<i32>(CENTER) - stack[k], false);
            if (sheet != 0u) {
                ink = sheet;
            }
        }
    }

    // What lies before each seat: a candle burning, or its answer.
    for (var seat: u32 = 0u; seat < 16u; seat = seat + 1u) {
        if (seat < seat_count()) {
            let d = ip - vec2<i32>(place_setting(seat));
            let seat_is = seat_state(seat);
            var mark = 0u;
            if (seat_is == RUNNING) {
                mark = candle(d);
            } else if (seat_is == RETURNED || seat_is == FAILED) {
                mark = parchment(d, seat_is == FAILED);
            }
            if (mark != 0u) {
                ink = mark;
            }
        }
    }

    // The council.
    for (var seat: u32 = 0u; seat < 16u; seat = seat + 1u) {
        if (seat < seat_count()) {
            let figure = knight(p, seat);
            if (figure.inside) {
                ink = figure.ink;
            }
        }
    }

    return vec4<f32>(rgb(ink), 1.0);
}

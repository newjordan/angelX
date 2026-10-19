//! What moves in Griffinwing's air, as labelled blocks for the art wave: the
//! griffins (a knight's, a rider's), the snuffers and kites, the lantern
//! towers, the dive-bombers, the Wyrm and its lamps, and the things on the
//! rail (gates, rings, arch, ridge, shutters). Built fresh each frame into
//! one reused mesh. Art never sets a hitbox: every size here is for the
//! picture alone; the sim's own radii judge every hit.
use super::canyon::{block, cuboid, diamond, hoop};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::{Mesh, mat};
use crate::rules::griffinwing::trig::Det;
use crate::rules::griffinwing::{
    Beast, Craft, Foe, Kind, Mode, Prop, Q, Session, St, Thing, Wyrm, boss, rail, rail::units,
    things,
};

/// A fixed-point length in units.
fn u(v: i32) -> f32 {
    units(v)
}

/// The rail's frame at `s` units: along, across (to the right) and up.
fn frame(s: f32) -> (V3, V3, V3) {
    let yaw = rail::rail().at(s).yaw;
    let (sin, cos) = yaw.dsin_cos();
    (v3(cos, sin, 0.0), v3(-sin, cos, 0.0), V3::UP)
}

fn at(s: f32, x: f32, z: f32) -> V3 {
    let w = rail::rail().world(s, x, z);
    v3(w[0], w[1], w[2])
}

/// A seat's saddle cloth.
fn cloth(seat: u32) -> u8 {
    match seat {
        1 | 3 | 4 | 6 => mat::CLOTH_GOLD,
        _ => mat::CLOTH_BLUE,
    }
}

/// A griffin from behind, a knight astride: a tawny lion's body, an eagle's
/// head, broad wings beating with `beat`, leaning `roll` radians about its
/// line and climbing `pitch`.
#[allow(clippy::too_many_arguments)]
pub fn griffin(
    m: &mut Mesh,
    c: V3,
    f: V3,
    r: V3,
    beat: f32,
    roll: f32,
    pitch: f32,
    saddle: u8,
    scale: f32,
) {
    let (sp, cp) = pitch.dsin_cos();
    let f2 = f * cp + V3::UP * sp;
    let up1 = V3::UP * cp - f * sp;
    let (sr, cr) = roll.dsin_cos();
    let right = r * cr + up1 * sr;
    let up = up1 * cr - r * sr;
    let k = scale;
    cuboid(
        m,
        c,
        f2 * (0.55 * k),
        right * (0.24 * k),
        up * (0.16 * k),
        mat::HORSE_BAY,
    );
    cuboid(
        m,
        c - f2 * (0.55 * k),
        f2 * (0.2 * k),
        right * (0.16 * k),
        up * (0.12 * k),
        mat::HORSE_BAY,
    );
    cuboid(
        m,
        c + f2 * (0.7 * k) + up * (0.14 * k),
        f2 * (0.2 * k),
        right * (0.17 * k),
        up * (0.17 * k),
        mat::HORSE_GREY,
    );
    cuboid(
        m,
        c + f2 * (0.95 * k) + up * (0.1 * k),
        f2 * (0.1 * k),
        right * (0.05 * k),
        up * (0.05 * k),
        mat::CLOTH_GOLD,
    );
    let tail = c - f2 * (1.15 * k) + up * (0.05 * k);
    m.push_quad(
        tail + right * (0.18 * k),
        tail - right * (0.18 * k),
        tail - right * (0.1 * k) - f2 * (0.4 * k),
        tail + right * (0.1 * k) - f2 * (0.4 * k),
        mat::HORSE_CHESTNUT,
    );
    // The saddle cloth, and the knight on it.
    cuboid(
        m,
        c + up * (0.17 * k),
        f2 * (0.22 * k),
        right * (0.26 * k),
        up * (0.02 * k),
        saddle,
    );
    cuboid(
        m,
        c + up * (0.38 * k),
        f2 * (0.1 * k),
        right * (0.12 * k),
        up * (0.2 * k),
        mat::ARMOR,
    );
    cuboid(
        m,
        c + up * (0.62 * k),
        f2 * (0.09 * k),
        right * (0.09 * k),
        up * (0.1 * k),
        mat::ARMOR,
    );
    cuboid(
        m,
        c + up * (0.76 * k) - f2 * (0.05 * k),
        f2 * (0.04 * k),
        right * (0.03 * k),
        up * (0.1 * k),
        saddle,
    );
    let lift = beat.dsin() * 0.5;
    for side in [-1.0f32, 1.0] {
        let root = c + right * (side * 0.22 * k) + up * (0.12 * k);
        let elbow = root + right * (side * 0.8 * k) + up * ((0.18 + lift * 0.5) * k);
        let tip = elbow + right * (side * 0.75 * k) + up * ((0.12 + lift) * k);
        m.push_quad(
            root + f2 * (0.4 * k),
            elbow + f2 * (0.3 * k),
            elbow - f2 * (0.4 * k),
            root - f2 * (0.35 * k),
            mat::HORSE_CHESTNUT,
        );
        m.push_quad(
            elbow + f2 * (0.3 * k),
            tip + f2 * (0.05 * k),
            tip - f2 * (0.55 * k),
            elbow - f2 * (0.4 * k),
            mat::HORSE_BAY,
        );
    }
}

/// A snuffer: a hooded wisp with two pale eyes and a wick-stub tail, facing
/// the camera (it comes from ahead), ringed in the cold light it was snuffing
/// from, so it reads against the dark.
fn wisp(m: &mut Mesh, c: V3, f: V3, r: V3, beat: f32, glow: bool) {
    let k = 2.0;
    cuboid(
        m,
        c,
        f * (0.4 * k),
        r * (0.42 * k),
        V3::UP * (0.5 * k),
        mat::STONE,
    );
    cuboid(
        m,
        c + V3::UP * (0.62 * k),
        f * (0.28 * k),
        r * (0.3 * k),
        V3::UP * (0.3 * k),
        mat::STONE_DARK,
    );
    hoop(
        m,
        c - f * (0.5 * k) + V3::UP * (0.2 * k),
        r,
        V3::UP,
        0.62 * k,
        0.16 * k,
        12,
        mat::MOONLIGHT,
    );
    for side in [-1.0f32, 1.0] {
        cuboid(
            m,
            c - f * (0.44 * k) + r * (side * 0.17 * k) + V3::UP * (0.3 * k),
            f * 0.03,
            r * (0.1 * k),
            V3::UP * (0.09 * k),
            if glow { mat::FIRE } else { mat::MOONLIGHT },
        );
    }
    let wag = beat.dsin() * 0.12;
    cuboid(
        m,
        c + f * (0.5 * k) + V3::UP * (0.4 * k + wag),
        f * (0.3 * k),
        r * 0.05,
        V3::UP * 0.05,
        mat::FIRE,
    );
}

/// A kite: a lantern-bat, a pointed hood and beating wings.
fn kite(m: &mut Mesh, c: V3, f: V3, r: V3, beat: f32, hot: bool) {
    diamond(m, c, 0.7, 0.4, mat::STONE);
    cuboid(
        m,
        c - f * 0.1,
        f * 0.12,
        r * 0.32,
        V3::UP * 0.42,
        if hot { mat::FIRE } else { mat::WINDOW },
    );
    let lift = beat.dsin() * 0.5;
    for side in [-1.0f32, 1.0] {
        let root = c + r * (side * 0.4);
        let tip = root + r * (side * 2.0) + V3::UP * (0.2 + lift * 1.0);
        m.push_quad(
            root + f * 0.4,
            tip + f * 0.15,
            tip - f * 0.6,
            root - f * 0.4,
            mat::STONE,
        );
        m.push_quad(
            tip + f * 0.15,
            tip + r * (side * 0.12) - f * 0.6,
            tip - f * 0.6,
            tip,
            mat::MOONLIGHT,
        );
    }
}

/// A hooded lantern tower on a ledge.
fn turret(m: &mut Mesh, c: V3, f: V3, r: V3, hot: bool) {
    cuboid(
        m,
        c - V3::UP * 0.7,
        f * 1.3,
        r * 1.3,
        V3::UP * 0.9,
        mat::STONE,
    );
    cuboid(
        m,
        c + V3::UP * 0.5,
        f * 0.9,
        r * 0.9,
        V3::UP * 0.7,
        mat::STONE,
    );
    cuboid(
        m,
        c - f * 0.82 + V3::UP * 0.5,
        f * 0.05,
        r * 0.6,
        V3::UP * 0.6,
        if hot { mat::FIRE } else { mat::MOONLIGHT },
    );
    hoop(
        m,
        c - f * 0.9 + V3::UP * 0.5,
        r,
        V3::UP,
        0.9,
        0.18,
        12,
        if hot { mat::FIRE } else { mat::MOONLIGHT },
    );
    cuboid(
        m,
        c + V3::UP * 1.5,
        f * 0.6,
        r * 0.6,
        V3::UP * 0.4,
        mat::STONE_DARK,
    );
    cuboid(
        m,
        c + V3::UP * 2.0,
        f * 0.3,
        r * 0.3,
        V3::UP * 0.2,
        mat::STONE_DARK,
    );
}

/// A dive-bomber: an arrowhead with wings, pointing where it goes.
fn bomber(m: &mut Mesh, c: V3, f: V3, r: V3, beat: f32) {
    diamond(m, c, 0.9, 0.0, mat::ARMOR);
    cuboid(
        m,
        c - f * 0.5,
        f * 0.06,
        r * 0.4,
        V3::UP * 0.1,
        mat::MOONLIGHT,
    );
    let lift = beat.dsin() * 0.3;
    for side in [-1.0f32, 1.0] {
        let root = c + r * (side * 0.4);
        let tip = root + r * (side * 1.1) + V3::UP * lift;
        m.push_quad(
            root + f * 0.5,
            tip - f * 0.1,
            tip - f * 0.7,
            root - f * 0.2,
            mat::ARMOR,
        );
    }
}

/// A lamp: a flame in a lantern, the cold-light shield round it while it is
/// shut, a bright flame once it is open, dark once it has gone out.
fn lamp(m: &mut Mesh, c: V3, f: V3, r: V3, foe: &Foe, lit: bool, turn: f32) {
    match foe.st {
        St::Broken => {
            diamond(m, c, 0.6, turn, mat::STONE_DARK);
        }
        St::Open => {
            diamond(m, c, 1.3, turn, mat::EMBER);
            diamond(m, c, 0.8, -turn, mat::FLAME);
            for i in 0..4 {
                let a = turn * 2.0 + i as f32 * std::f32::consts::FRAC_PI_2;
                let q = c + r * (a.dcos() * 2.4) + V3::UP * (a.dsin() * 2.4);
                diamond(m, q, 0.3, a, mat::CRYSTAL);
            }
        }
        _ => {
            diamond(
                m,
                c,
                if lit { 1.3 } else { 0.8 },
                turn,
                if lit { mat::FIRE } else { mat::STONE },
            );
            if lit {
                hoop(m, c, r, V3::UP, 2.6, 0.3, 18, mat::MOONLIGHT);
                hoop(m, c, f, V3::UP, 2.6, 0.3, 18, mat::MOONLIGHT);
                hoop(m, c, f, r, 2.6, 0.3, 18, mat::MOONLIGHT);
            } else {
                hoop(m, c, r, V3::UP, 1.6, 0.14, 12, mat::CRYSTAL);
            }
        }
    }
}

/// The Wyrm: a smoke-dark head in scaled armour with its horn cone and
/// lantern, jaws that open to spit, coils down into the roost.
fn wyrm(m: &mut Mesh, s: f32, b: &Beast, tick: u32) {
    let (f, r, _) = frame(s);
    let head = at(s, u(b.x), u(b.z));
    let k = 1.5;
    let open = match b.st {
        Wyrm::Tell | Wyrm::Roar => 0.9,
        Wyrm::Coil => 0.4,
        _ => 0.0,
    };
    let breathe = ((tick as f32) * 0.1).dsin() * 0.2;
    // The head and its jaws, facing the camera (back along the rail).
    cuboid(
        m,
        head,
        f * (2.2 * k),
        r * (3.0 * k),
        V3::UP * (2.0 * k),
        mat::ARMOR,
    );
    cuboid(
        m,
        head - f * (2.3 * k) - V3::UP * ((1.0 + open) * k),
        f * (1.4 * k),
        r * (2.4 * k),
        V3::UP * (0.5 * k),
        mat::ARMOR,
    );
    cuboid(
        m,
        head - f * (2.35 * k) + V3::UP * ((0.4 - open * 0.2) * k),
        f * 0.2,
        r * (1.8 * k),
        V3::UP * (0.3 * k),
        if open > 0.0 {
            mat::MOONLIGHT
        } else {
            mat::STONE_DARK
        },
    );
    for side in [-1.0f32, 1.0] {
        // Eyes under a heavy brow, horn-ears at the sides.
        cuboid(
            m,
            head - f * (2.25 * k) + r * (side * 1.6 * k) + V3::UP * ((1.1 + breathe) * k),
            f * 0.1,
            r * (0.8 * k),
            V3::UP * (0.36 * k),
            mat::MOONLIGHT,
        );
        cuboid(
            m,
            head - f * (2.22 * k) + r * (side * 1.6 * k) + V3::UP * (1.65 * k),
            f * (0.2 * k),
            r * (1.1 * k),
            V3::UP * (0.2 * k),
            mat::STONE_DARK,
        );
        cuboid(
            m,
            head + r * (side * 3.1 * k) + V3::UP * (1.6 * k),
            f * (0.4 * k),
            r * (0.4 * k),
            V3::UP * (1.4 * k),
            mat::STONE,
        );
        // Nostrils.
        cuboid(
            m,
            head - f * (2.3 * k) + r * (side * 0.5 * k) + V3::UP * (0.0 * k),
            f * 0.06,
            r * (0.2 * k),
            V3::UP * (0.14 * k),
            mat::STONE_DARK,
        );
    }
    // Teeth along the jaw.
    for i in -4..=4 {
        let tx = i as f32 * 0.55 * k;
        cuboid(
            m,
            head - f * (2.35 * k) + r * tx + V3::UP * ((-0.35 - open * 0.5) * k),
            f * 0.05,
            r * (0.12 * k),
            V3::UP * (0.28 * k),
            mat::BONE,
        );
    }
    // The horn cone with its lantern.
    cuboid(
        m,
        head + V3::UP * (3.0 * k),
        f * (0.7 * k),
        r * (0.7 * k),
        V3::UP * (1.0 * k),
        mat::BRASS,
    );
    cuboid(
        m,
        head + V3::UP * (4.2 * k),
        f * (0.4 * k),
        r * (0.4 * k),
        V3::UP * (0.4 * k),
        mat::BRASS,
    );
    cuboid(
        m,
        head + V3::UP * (4.9 * k),
        f * (0.25 * k),
        r * (0.25 * k),
        V3::UP * (0.3 * k),
        mat::FIRE,
    );
    // The coils: scaled body going down and curling back.
    for i in 0..9 {
        let t = i as f32;
        let sway = ((tick as f32) * 0.05 + t * 0.7).dsin();
        let c =
            head + f * ((1.8 + t * 1.2) * k) + r * (sway * 2.2) - V3::UP * ((2.5 + t * 1.9) * k);
        cuboid(
            m,
            c,
            f * (1.3 * k),
            r * ((2.6 - t * 0.15) * k),
            V3::UP * (1.1 * k),
            if i % 2 == 0 { mat::ARMOR } else { mat::BRASS },
        );
    }
}

/// The coils' sweep when it lands: two great slabs of body, a gap between.
fn coil_wall(m: &mut Mesh, s: f32, b: &Beast, half: f32) {
    let gap = u(b.gap);
    let g = u(boss::GAP_HALF);
    let slab = |m: &mut Mesh, x0: f32, x1: f32| {
        if x1 - x0 > 0.2 {
            block(
                m,
                rail::rail(),
                s,
                (x0 + x1) * 0.5,
                0.0,
                (1.6, (x1 - x0) * 0.5, 9.0),
                mat::STONE_DARK,
            );
            block(
                m,
                rail::rail(),
                s - 1.65,
                (x0 + x1) * 0.5,
                0.0,
                (0.05, (x1 - x0) * 0.5, 8.4),
                mat::BRASS,
            );
        }
    };
    slab(m, -half, gap - g);
    slab(m, gap + g, half);
}

/// The cold-light roar: a column of light from the Wyrm's mouth.
fn roar_beam(m: &mut Mesh, s0: f32, s1: f32, b: &Beast) {
    let x = u(b.col);
    let w = u(boss::COL_HALF);
    let r = rail::rail();
    let q = |s: f32, x: f32, z: f32| {
        let p = r.world(s, x, z);
        v3(p[0], p[1], p[2])
    };
    for z in [-5.0f32, 0.0, 5.0] {
        m.push_quad(
            q(s0, x - w, z),
            q(s1, x - w, z),
            q(s1, x + w, z),
            q(s0, x + w, z),
            mat::MOONLIGHT,
        );
    }
}

/// A prop on the rail.
fn prop(m: &mut Mesh, p: &Prop, t: u32, wx: i32, tick: u32, half: f32) {
    let s = u(p.s);
    let (f, r, _) = frame(s);
    let c = at(s, u(p.x), u(p.z));
    if super::set::painted(p.thing) {
        // A painted gate or ring is stood over the scene (`set.rs`).
        return;
    }
    match p.thing {
        Thing::Gate => {
            let radius = u(p.w);
            hoop(m, c, r, V3::UP, radius, 0.22, 20, mat::WINDOW);
            hoop(m, c, r, V3::UP, radius + 0.22, 0.12, 20, mat::FIRE);
            for i in 0..4 {
                let a = i as f32 * std::f32::consts::FRAC_PI_2 + tick as f32 * 0.04;
                cuboid(
                    m,
                    c + r * (a.dcos() * (radius + 0.5)) + V3::UP * (a.dsin() * (radius + 0.5)),
                    f * 0.12,
                    r * 0.12,
                    V3::UP * 0.2,
                    mat::FIRE,
                );
            }
        }
        Thing::Ring => {
            let radius = u(p.w);
            hoop(m, c, r, V3::UP, radius, 0.3, 20, mat::FIRE);
            hoop(m, c, r, V3::UP, radius - 0.3, 0.14, 20, mat::CLOTH_GOLD);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                cuboid(
                    m,
                    c + r * (a.dcos() * (radius + 0.45)) + V3::UP * (a.dsin() * (radius + 0.45)),
                    f * 0.1,
                    r * 0.14,
                    V3::UP * 0.24,
                    mat::WINDOW,
                );
            }
        }
        Thing::Arch => {
            let (ox, oz) = (u(p.x), u(p.z));
            let (w, h) = (u(p.w), u(p.h));
            let depth = u(p.len) * 0.5;
            let sc = s + depth;
            let rl = rail::rail();
            // Four slabs round the opening, wall to wall.
            block(
                m,
                rl,
                sc,
                (-half + ox - w) * 0.5,
                1.0,
                (depth, (ox - w + half) * 0.5, 15.0),
                mat::ROCK,
            );
            block(
                m,
                rl,
                sc,
                (half + ox + w) * 0.5,
                1.0,
                (depth, (half - ox - w) * 0.5, 15.0),
                mat::ROCK,
            );
            block(
                m,
                rl,
                sc,
                ox,
                (oz + h + 16.0) * 0.5,
                (depth, w, (16.0 - oz - h) * 0.5),
                mat::ROCK,
            );
            block(
                m,
                rl,
                sc,
                ox,
                (-14.0 + oz - h) * 0.5,
                (depth, w, (oz - h + 14.0) * 0.5),
                mat::ROCK,
            );
            for k in 0..5 {
                let x = ox - w + w * 0.4 * k as f32;
                block(
                    m,
                    rl,
                    sc,
                    x,
                    oz + h + 0.5,
                    (depth * 0.8, 0.4, 0.5),
                    mat::STONE,
                );
            }
        }
        Thing::Ridge => {
            let (w, h) = (u(p.w), u(p.h));
            let depth = u(p.len) * 0.5;
            block(
                m,
                rail::rail(),
                s + depth,
                u(p.x),
                u(p.z),
                (depth, w, h),
                mat::ROCK,
            );
            block(
                m,
                rail::rail(),
                s + depth,
                u(p.x),
                u(p.z) + h + 0.3,
                (depth * 0.8, w * 0.7, 0.4),
                mat::STONE_DARK,
            );
        }
        Thing::Shutter => {
            let gap = u(things::shutter_gap(p, t, wx));
            let rl = rail::rail();
            let sc = s + u(p.len) * 0.1;
            for side in [-1.0f32, 1.0] {
                let inner = gap;
                let outer = half + 1.0;
                block(
                    m,
                    rl,
                    sc,
                    side * (inner + outer) * 0.5,
                    1.0,
                    (0.5, (outer - inner) * 0.5, 15.0),
                    mat::STONE_DARK,
                );
                block(
                    m,
                    rl,
                    sc - 0.55,
                    side * (inner + 0.2),
                    1.0,
                    (0.05, 0.2, 14.0),
                    mat::FIRE,
                );
                block(
                    m,
                    rl,
                    sc - 0.55,
                    side * (inner + 0.2),
                    9.0,
                    (0.1, 0.5, 0.5),
                    mat::FIRE,
                );
            }
        }
    }
}

/// The last beacon: a great lantern on a tower, lit as the run ends.
fn beacon(m: &mut Mesh, s: f32, lit: bool) {
    let rl = rail::rail();
    block(m, rl, s, 0.0, -4.0, (3.0, 3.0, 12.0), mat::STONE);
    block(m, rl, s, 0.0, 9.0, (3.6, 3.6, 0.8), mat::STONE_DARK);
    block(
        m,
        rl,
        s - 0.1,
        0.0,
        11.5,
        (1.6, 1.6, 2.0),
        if lit { mat::FIRE } else { mat::STONE_DARK },
    );
    block(m, rl, s, 0.0, 14.2, (1.0, 1.0, 0.8), mat::BRASS);
}

/// Everything that moves this frame, into `m`, as `seat` sees it.
pub fn movers(m: &mut Mesh, s: &Session, seat: u32, eye: V3) {
    m.tris.clear();
    let anchor = u(s.anchor());
    let tick = s.elapsed;
    let n = crate::rules::griffinwing::Nums::default();
    let half = rail::rail().at(anchor).half;
    // The crafts.
    let draw_craft = |m: &mut Mesh, id: u32, c: &Craft, lost: bool| {
        let su = u(s.craft_s(c));
        let (f, r, _) = frame(su);
        let mut z = u(c.z);
        let mut roll = c.bank as f32 / Q as f32 * 0.42;
        let mut pitch = c.lift as f32 / Q as f32 * 0.22;
        let mut scale = 0.7;
        if c.roll > 0 {
            // A roll turns the whole craft over, fast at the start.
            let k = 1.0 - c.roll as f32 / c.roll_len.max(1) as f32;
            roll += c.dir as f32 * k * std::f32::consts::TAU;
        }
        if c.down > 0 {
            let k = 1.0 - c.down as f32 / crate::rules::griffinwing::RISE as f32;
            z -= k * 6.0;
            roll += k * 9.0;
            pitch -= k;
            scale = (0.7 - k * 0.4).max(0.25);
        } else if lost {
            // A lost rider falls out of the picture under its own weight.
            let k = ((crate::rules::griffinwing::dm(10) - c.z) as f32
                / crate::rules::griffinwing::dm(120) as f32)
                .clamp(0.0, 1.0);
            roll += k * 9.0;
            pitch -= k;
            scale = (0.7 - k * 0.4).max(0.25);
        }
        let centre = at(su, u(c.x), z + 0.0);
        // A rival at the lens would fill it: leave out one that close.
        if id != seat && (centre - eye).length() < 2.4 {
            return;
        }
        griffin(
            m,
            centre,
            f,
            r,
            tick as f32 * 0.35 + id as f32,
            roll,
            pitch,
            cloth(id),
            scale,
        );
    };
    for (&id, seat_) in &s.seats {
        let c = &seat_.craft;
        // The unhurt moment after a blow blinks.
        let blink = c.hurt > 0 && c.down == 0 && (tick / 3) % 2 == 0;
        if !blink {
            draw_craft(m, id, c, false);
        }
    }
    for w in &s.wing {
        if !w.lost || w.craft.z > crate::rules::griffinwing::dm(-100) {
            draw_craft(m, u32::from(w.id), &w.craft, w.lost);
        }
    }
    // The foes.
    for f in &s.foes {
        let su = u(f.s);
        let (fw, r, _) = frame(su);
        let c = at(su, u(f.x), u(f.z));
        let beat = tick as f32 * 0.3 + f.phase as f32;
        match f.kind {
            Kind::Wisp => wisp(m, c, fw, r, beat, f.st == St::Tell),
            Kind::Kite => kite(m, c, fw, r, beat, f.st == St::Latched),
            Kind::Turret => turret(m, c, fw, r, f.st == St::Tell),
            Kind::Bomber => {
                if f.st != St::Hide {
                    // Pointing along its way: the frame turned to its velocity.
                    let d = fw * u(f.vs - crate::rules::griffinwing::PACE)
                        + r * u(f.vx)
                        + V3::UP * u(f.vz);
                    let dir = if f.st == St::Dive {
                        d.normalize()
                    } else {
                        fw * -1.0
                    };
                    let dir = if dir.length() < 0.5 { fw * -1.0 } else { dir };
                    let side = V3::UP.cross(dir).normalize();
                    let side = if side.length() < 0.5 { r } else { side };
                    bomber(m, c, dir, side, beat);
                }
            }
            Kind::Lamp => {
                let lit = s
                    .boss
                    .as_ref()
                    .is_some_and(|b| b.lamps[usize::from(b.phase.min(2))] == f.id);
                lamp(m, c, fw, r, f, lit, tick as f32 * 0.07 + f.phase as f32);
            }
        }
    }
    if let Some(b) = &s.boss {
        wyrm(m, anchor + u(boss::HEAD), b, tick);
        if b.st == Wyrm::Coil {
            coil_wall(m, anchor, b, half * 0.9);
        }
        if b.st == Wyrm::Roar {
            roar_beam(m, anchor, anchor + u(boss::HEAD), b);
        }
    }
    for p in &s.props {
        let taken = p.taken & crate::rules::griffinwing::bit(seat) != 0;
        if taken && matches!(p.thing, Thing::Gate | Thing::Ring) {
            continue;
        }
        if (u(p.s) - anchor).abs() < 110.0 {
            prop(m, p, s.t, n.wx(), tick, half);
        }
    }
    // The last beacon: a painted one is stood over the scene (`set.rs`).
    if !super::set::beacon_painted() {
        if matches!(s.mode, Mode::Victory | Mode::Tally) {
            beacon(m, u(s.home) + 100.0, true);
        } else if s.stage == 2 && s.boss.is_none() {
            beacon(
                m,
                u(crate::rules::griffinwing::rail::ORIGIN[2]) + 560.0,
                false,
            );
        }
    }
}

//! Each place's buildings and landmarks in the monolith language
//! (docs/REALM-MONOLITH.md, "Scale"): one or two big masses on the place's
//! footprint, stepped, slotted, finned and corniced, its door a deep slot
//! on the side its door is, lit windows and braziers where something
//! happens. Nothing stands past the footprint the map gives it (a knight
//! walks round it), and everything starts under the ground so nothing
//! floats.

use angel_stage::set::{Kind, SetLamp};

use super::chunks::Build;
use super::kit::Kit;
use super::realm_ground::Terrain;
use super::realm_mono::{Mono, Piece, spire};

/// What a build stands: its pieces, and its lamps each with how much it
/// matters (the chunk keeps the few that matter most).
#[derive(Default)]
pub struct Stood {
    pub pieces: Vec<Piece>,
    pub lamps: Vec<(f32, SetLamp)>,
}

impl Stood {
    fn take(&mut self, m: Mono) {
        self.pieces.extend(m.pieces);
    }

    fn lamp(&mut self, m: &Mono, at: [f32; 3], reach: f32, strength: f32, weight: f32) {
        self.lamps.push((weight, SetLamp { at: m.point(at), reach, strength }));
    }
}

/// How tall a place's main mass is on a footprint `w` by `d` (pixels):
/// 60 on two tiles, 90 on four, at most 120.
pub fn height_for(w: f32, d: f32) -> f32 {
    (28.0 + w.min(d)).clamp(44.0, 120.0)
}

/// Build `b` (not a castle: `realm_castle`) on its footprint.
pub fn place(b: &Build, terrain: &Terrain) -> Stood {
    let foot = super::blockout::px(b.foot);
    let mut m = Mono::on(foot, b.face, b.seed);
    let y0 = terrain.lowest(foot).min(0.0) - 2.0;
    let base = height_for(m.w, m.d) * (0.92 + 0.16 * m.unit(1));
    let mut s = Stood::default();
    let town = 0.35;
    match b.kit {
        Kit::Cottage | Kit::Bothy => {
            hall(&mut m, Kind::Stone, base * 0.8, y0, town);
            roof_steps(&mut m, Kind::Stone, base * 0.8);
        }
        Kit::Hall | Kit::LandOffice => {
            hall(&mut m, Kind::Stone, base, y0, town);
            if m.w >= 48.0 {
                back_tower(&mut m, Kind::Stone, base * 1.35, y0, town);
            }
            if b.kit == Kit::LandOffice {
                colonnade(&mut m, Kind::Plaster, base * 0.45);
            }
        }
        Kit::Barn | Kit::Stable | Kit::Ranch => {
            hall(&mut m, Kind::Wood, base * 0.62, y0, 0.0);
            roof_steps(&mut m, Kind::Wood, base * 0.62);
        }
        Kit::Forge | Kit::Workshop | Kit::Wheelwright | Kit::Timberyard => {
            let h = base * if b.kit == Kit::Forge { 0.85 } else { 0.72 };
            let door = hall(&mut m, Kind::Stone, h, y0, town);
            stack(&mut m, Kind::Stone, h * 1.45);
            if b.kit == Kit::Forge {
                // The forge's fire in its bay: activity.
                let at = m.brazier(door[0] + m.w * 0.28, m.d / 2.0 - 3.5, 0.0);
                s.lamp(&m, at, 54.0, 0.85, 2.0);
            }
            if b.kit == Kit::Timberyard {
                logs(&mut m, y0);
            }
        }
        Kit::Tower | Kit::Turret | Kit::Rookery | Kit::Ward | Kit::Aviary | Kit::Belfry => {
            let h = (base * 1.5).min(170.0);
            tower(&mut m, Kind::Stone, h, y0, 0.3);
            if b.kit == Kit::Belfry {
                m.block(Kind::Door, [-3.0, 3.0], [h - 22.0, h - 9.0], [m.d / 2.0 - 5.2, m.d / 2.0 - 4.6]);
            }
        }
        Kit::Lighthouse => {
            let h = 180.0;
            tower(&mut m, Kind::Plaster, h, y0, 0.2);
            let r = (m.w.min(m.d) / 2.0 - 8.0).max(4.0);
            m.block(Kind::Window, [-r, r], [h + 1.0, h + 8.0], [-r, r]);
            m.block(Kind::Stone, [-r - 1.5, r + 1.5], [h + 8.0, h + 11.0], [-r - 1.5, r + 1.5]);
            s.lamp(&m, [0.0, h + 4.0, 0.0], 90.0, 1.0, 3.0);
        }
        Kit::Windmill => {
            let h = base * 1.2;
            tower(&mut m, Kind::Plaster, h, y0, 0.0);
            // The sails: a cross of slabs standing proud of the front.
            let z = m.d / 2.0 - 2.5;
            let span = (m.w * 1.1).min(m.w - 1.0);
            m.block(Kind::Wood, [-span / 2.0, span / 2.0], [h * 0.72 - 2.0, h * 0.72 + 2.0], [z, z + 2.0]);
            m.block(Kind::Wood, [-2.0, 2.0], [h * 0.72 - span / 2.0, h * 0.72 + span / 2.0], [z, z + 2.0]);
        }
        Kit::Chapel | Kit::Priory | Kit::School | Kit::College | Kit::Bathhouse | Kit::Observatory => {
            temple(&mut m, Kind::Stone, base, y0, town);
            match b.kit {
                Kit::School => {
                    let (x, z) = (m.w * 0.25, -m.d * 0.15);
                    m.put("pyramid", Kind::Slate, [x, base * 1.25, z], [12.0, 46.0, 12.0], 0.0);
                }
                Kit::Bathhouse | Kit::Observatory => {
                    let r = (m.w.min(m.d) * 0.36).min(26.0);
                    m.put("sphere-16", Kind::Slate, [0.0, base * 0.62 - r * 0.45, -m.d * 0.05], [r * 2.0, r * 1.6, r * 2.0], 0.0);
                }
                Kit::College => {
                    let at = m.brazier(-m.w * 0.12, m.d / 2.0 - 3.0, 0.0);
                    s.lamp(&m, at, 56.0, 0.8, 1.5);
                }
                _ => {}
            }
        }
        Kit::RoundTable => {
            round_hall(&mut m, Kind::Stone, base * 0.85, y0);
            let at = m.brazier(0.0, m.d / 2.0 - 3.0, 0.0);
            s.lamp(&m, at, 50.0, 0.8, 1.5);
        }
        Kit::Gatehouse | Kit::TollBridge => {
            gatehouse(&mut m, Kind::Stone, base * 1.1, y0);
        }
        Kit::MineMouth | Kit::DelveGate | Kit::Crag | Kit::Cliffs | Kit::Eyrie => {
            let h = match b.kit {
                Kit::DelveGate => 110.0,
                Kit::MineMouth => 84.0,
                Kit::Eyrie => 120.0,
                _ => base * 1.1,
            };
            heap(&mut m, h, y0);
            if matches!(b.kit, Kit::MineMouth | Kit::DelveGate) {
                portal(&mut m, h);
                for side in [-1.0f32, 1.0] {
                    let at = m.brazier(side * (m.w / 2.0 - 4.0), m.d / 2.0 - 3.5, 0.0);
                    if side < 0.0 {
                        s.lamp(&m, [0.0, at[1], at[2]], 64.0, 0.95, if b.kit == Kit::DelveGate { 3.0 } else { 2.0 });
                    }
                }
            }
            if b.kit == Kit::Eyrie {
                m.put("cylinder-12", Kind::Wood, [0.0, h - 3.0, -2.0], [m.w * 0.5, 7.0, m.d * 0.5], 0.0);
            }
        }
        Kit::Colosseum | Kit::Hippodrome => {
            let h = if b.kit == Kit::Colosseum { 104.0 } else { 52.0 };
            arena(&mut m, Kind::Stone, h, y0);
            if b.kit == Kit::Colosseum {
                for side in [-1.0f32, 1.0] {
                    let at = m.brazier(side * 15.0, m.d / 2.0 - 3.0, 0.0);
                    if side < 0.0 {
                        s.lamp(&m, [0.0, at[1], at[2]], 70.0, 0.95, 2.5);
                    }
                }
            }
        }
        Kit::Watermill | Kit::Sluice | Kit::Quay | Kit::Causeway | Kit::Aqueduct | Kit::Saltworks | Kit::Strand => {
            waterworks(&mut m, b.kit, base, y0);
        }
        Kit::Ruins | Kit::SunkenAbbey | Kit::Stubs => {
            ruin(&mut m, base * 0.9, y0);
        }
        Kit::Well => {
            well(&mut m, y0);
        }
        Kit::DragonKeep | Kit::Keep | Kit::Castle(_) => {
            // Castles are `realm_castle`'s; one that comes here stands as a
            // great tower.
            tower(&mut m, Kind::Stone, base * 1.6, y0, 0.2);
        }
        _ => {
            yard(&mut m, b.kit, base, y0, &mut s);
        }
    }
    s.take(m);
    s
}

/// The common mass: a stepped plinth, a body to `h`, its cornice, a string
/// course, fins across the front, a row of slits high in it (`lit` of them
/// lit) and a door as a deep slot in the middle of the front, the plinth
/// parted round it. Returns the door's foot (the frame's).
pub fn hall(m: &mut Mono, kind: Kind, h: f32, y0: f32, lit: f32) -> [f32; 3] {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let inset = if m.w.min(m.d) < 28.0 { 2.5 } else { 4.5 };
    let (bx, bz) = ([-hx + inset, hx - inset], [-hz + inset, hz - inset]);
    let (t0, t1) = (4.0, 9.0);
    let dw = (m.w * 0.18).clamp(5.0, 10.0);
    let gap = dw / 2.0 + 3.0;
    let front = bz[1];
    // The plinth: two tiers, parted at the door.
    m.block(kind, [-hx, hx], [y0, t0], [-hz, front]);
    m.block(kind, [-hx, -gap], [y0, t0], [front, hz]);
    m.block(kind, [gap, hx], [y0, t0], [front, hz]);
    m.block(kind, [-hx + 2.0, hx - 2.0], [t0, t1], [-hz + 2.0, front]);
    m.block(kind, [-hx + 2.0, -gap], [t0, t1], [front, hz - 2.0]);
    m.block(kind, [gap, hx - 2.0], [t0, t1], [front, hz - 2.0]);
    m.block(kind, bx, [t1, h], bz);
    m.cornice(kind, bx, bz, h, 1.5);
    if h > 36.0 {
        m.course(kind, bx, bz, t1 + (h - t1) * 0.5);
        m.fins(kind, [bx[0] + 2.0, bx[1] - 2.0], front, [t1 + 2.0, h - 4.0], 7.0, 1.4);
        m.slits([bx[0] + 2.0, bx[1] - 2.0], front + 1.0, t1 + (h - t1) * 0.66, 6.0, 7.0, lit);
    }
    let dh = (h * 0.32).clamp(12.0, 24.0);
    m.door(kind, 0.0, front, 0.0, [dw, dh], 2.5);
    [0.0, 0.0, front]
}

/// A roof as two stepped tiers on a hall's top, darker: a house reads as a house.
fn roof_steps(m: &mut Mono, kind: Kind, h: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let inset = if m.w.min(m.d) < 28.0 { 2.5 } else { 4.5 };
    let roof = if kind == Kind::Wood { Kind::Thatch } else { Kind::Slate };
    m.block(roof, [-hx + inset + 3.0, hx - inset - 3.0], [h + 1.0, h + 7.0], [-hz + inset + 3.0, hz - inset - 3.0]);
    m.block(roof, [-hx + inset + 7.0, hx - inset - 7.0], [h + 7.0, h + 12.0], [-hz + inset + 6.0, hz - inset - 6.0]);
}

/// A taller block rising at the back of a long hall, to one side.
fn back_tower(m: &mut Mono, kind: Kind, h: f32, y0: f32, lit: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let tw = (m.w * 0.3).clamp(14.0, 30.0);
    let side = if m.unit(7) < 0.5 { -1.0 } else { 1.0 };
    let x = side * (hx - 5.0 - tw / 2.0);
    let (z0, z1) = (-hz + 4.5, -hz + 4.5 + (m.d * 0.5).max(14.0));
    m.block(kind, [x - tw / 2.0, x + tw / 2.0], [y0, h], [z0, z1]);
    m.cornice(kind, [x - tw / 2.0, x + tw / 2.0], [z0, z1], h, 1.2);
    m.block(Kind::Door, [x - 1.0, x + 1.0], [h * 0.45, h - 8.0], [z1 - 0.2, z1 + 0.25]);
    if m.unit(9) < lit + 0.3 {
        m.block(Kind::Window, [x - 0.7, x + 0.7], [h * 0.7, h * 0.7 + 5.0], [z1 + 0.25, z1 + 0.35]);
    }
}

/// A chimney stack at a back corner.
fn stack(m: &mut Mono, kind: Kind, h: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let x = hx - 8.0;
    m.block(kind, [x - 3.0, x + 3.0], [0.0, h], [-hz + 3.5, -hz + 9.5]);
    m.block(kind, [x - 4.0, x + 4.0], [h - 3.0, h], [-hz + 2.5, -hz + 10.5]);
}

/// A tall square tower: a stepped plinth all round, its body, a crown band,
/// two deep slots up its front with a pier standing proud between them and
/// a few lit windows in them, a door at its foot.
pub fn tower(m: &mut Mono, kind: Kind, h: f32, y0: f32, lit: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let small = m.w.min(m.d) < 24.0;
    let (i1, ib) = if small { (1.5, 3.0) } else { (2.5, 5.0) };
    m.block(kind, [-hx, hx], [y0, 6.0], [-hz, hz]);
    m.block(kind, [-hx + i1, hx - i1], [6.0, 12.0], [-hz + i1, hz - i1]);
    let (bx, bz) = ([-hx + ib, hx - ib], [-hz + ib, hz - ib]);
    m.block(kind, bx, [12.0, h], bz);
    m.block(kind, [bx[0] - 1.5, bx[1] + 1.5], [h - 6.0, h + 1.0], [bz[0] - 1.5, bz[1] + 1.5]);
    let face = bz[1];
    let half = (bx[1] - bx[0]) / 2.0;
    if half >= 8.0 {
        let sx = half * 0.45;
        for (k, x) in [-sx, sx].into_iter().enumerate() {
            m.block(Kind::Door, [x - 1.0, x + 1.0], [18.0, h - 9.0], [face - 0.2, face + 0.2]);
            let rows = ((h - 40.0) / 16.0).floor().max(0.0) as u32;
            for r in 0..rows {
                if m.unit(40 + r * 2 + k as u32) < lit {
                    let y = 30.0 + r as f32 * 16.0;
                    m.block(Kind::Window, [x - 0.7, x + 0.7], [y, y + 5.0], [face + 0.2, face + 0.3]);
                }
            }
        }
        m.block(kind, [-sx + 2.5, sx - 2.5], [18.0, h - 9.0], [face, face + 1.2]);
    } else {
        m.block(Kind::Door, [-0.8, 0.8], [16.0, h - 9.0], [face - 0.2, face + 0.2]);
    }
    let dw = (half * 0.7).clamp(4.0, 8.0);
    m.block(Kind::Door, [-dw / 2.0, dw / 2.0], [y0, 14.0], [face - 0.3, hz + 0.1]);
}

/// A temple: a long hall, a stepped portico across its front, a tall slab
/// tower at its back.
fn temple(m: &mut Mono, kind: Kind, h: f32, y0: f32, lit: f32) {
    let hall_h = h * 0.62;
    hall(m, kind, hall_h, y0, lit);
    let hz = m.d / 2.0 - 0.5;
    let tw = (m.w * 0.36).clamp(12.0, 34.0);
    let td = (m.d * 0.42).clamp(10.0, 30.0);
    let (z0, z1) = (-hz + 3.0, -hz + 3.0 + td);
    m.block(kind, [-tw / 2.0, tw / 2.0], [y0, h], [z0, z1]);
    m.cornice(kind, [-tw / 2.0, tw / 2.0], [z0, z1], h, 1.5);
    m.block(Kind::Door, [-1.2, 1.2], [hall_h + 8.0, h - 8.0], [z1 - 0.2, z1 + 0.2]);
    if m.unit(11) < 0.7 {
        m.block(Kind::Window, [-0.8, 0.8], [h - 22.0, h - 15.0], [z1 + 0.2, z1 + 0.3]);
    }
    // Fins up the tower's flanks.
    for side in [-1.0f32, 1.0] {
        let x = side * (tw / 2.0 + 0.8);
        m.block(kind, [x - 0.8, x + 0.8], [hall_h, h - 5.0], [z0 + 2.0, z1 - 2.0]);
    }
}

/// A colonnade: posts standing free along the front of the plinth.
fn colonnade(m: &mut Mono, kind: Kind, h: f32) {
    let hz = m.d / 2.0 - 0.5;
    let n = ((m.w - 10.0) / 8.0).floor() as i32;
    for k in 0..n {
        let x = -(n - 1) as f32 * 4.0 + k as f32 * 8.0;
        if x.abs() < 7.0 {
            continue;
        }
        m.block(kind, [x - 1.3, x + 1.3], [4.0, h], [hz - 3.5, hz - 1.0]);
    }
    m.block(kind, [-m.w / 2.0 + 2.0, m.w / 2.0 - 2.0], [h, h + 3.0], [hz - 4.0, hz - 0.5]);
}

/// A round hall: drums stepping in, a cornice ring, slots round it.
fn round_hall(m: &mut Mono, kind: Kind, h: f32, y0: f32) {
    let r = m.w.min(m.d) / 2.0 - 0.5;
    m.put("cylinder-16", kind, [0.0, y0, 0.0], [2.0 * r, 6.0 - y0, 2.0 * r], 0.0);
    m.put("cylinder-16", kind, [0.0, 6.0, 0.0], [2.0 * r - 4.0, 6.0, 2.0 * r - 4.0], 0.0);
    m.put("cylinder-16", kind, [0.0, 12.0, 0.0], [2.0 * r - 8.0, h - 12.0, 2.0 * r - 8.0], 0.0);
    m.put("cylinder-16", kind, [0.0, h - 4.0, 0.0], [2.0 * r - 5.0, 5.0, 2.0 * r - 5.0], 0.0);
    m.put("cone-16", Kind::Slate, [0.0, h + 1.0, 0.0], [2.0 * r - 8.0, h * 0.3, 2.0 * r - 8.0], 0.0);
    m.block(Kind::Door, [-3.0, 3.0], [y0, 16.0], [r - 4.6, r - 3.3]);
}

/// A gatehouse: two towers and a slot of a way through between them under
/// a heavy lintel.
fn gatehouse(m: &mut Mono, kind: Kind, h: f32, y0: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let tw = (m.w * 0.34).max(8.0);
    for side in [-1.0f32, 1.0] {
        let x0 = if side < 0.0 { -hx } else { hx - tw };
        let x = [x0, x0 + tw];
        m.block(kind, x, [y0, 6.0], [-hz, hz]);
        m.block(kind, [x[0] + 1.5, x[1] - 1.5], [6.0, h], [-hz + 1.5, hz - 1.5]);
        m.block(kind, [x[0] + 0.5, x[1] - 0.5], [h - 5.0, h + 1.0], [-hz + 0.5, hz - 0.5]);
    }
    let inner = hx - tw;
    m.block(kind, [-inner - 1.0, inner + 1.0], [h * 0.45, h * 0.62], [-hz + 1.0, hz - 1.0]);
    m.block(Kind::Door, [-inner, inner], [y0, h * 0.45], [-hz + 2.0, -hz + 3.5]);
}

/// A heap of rock as great slabs stacked into a stepped mass to `h`.
fn heap(m: &mut Mono, h: f32, y0: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let tiers = 3;
    for k in 0..tiers {
        let t = k as f32 / tiers as f32;
        let (ix, iz) = (hx * (0.08 + 0.3 * t), hz * (0.06 + 0.32 * t));
        let back = hz * 0.25 * t;
        let (ya, yb) = (if k == 0 { y0 } else { h * t - 2.0 }, h * (t + 1.0 / tiers as f32));
        let turn = (m.unit(20 + k) - 0.5) * 8.0;
        let (w, d) = (2.0 * (hx - ix), 2.0 * (hz - iz) - back);
        m.put("box", Kind::Rock, [0.0, ya, -back / 2.0], [w, yb - ya, d], turn);
    }
}

/// A portal in a heap's front: two jamb slabs, a lintel, the dark slot.
fn portal(m: &mut Mono, h: f32) {
    let hz = m.d / 2.0 - 0.5;
    let w = (m.w * 0.22).clamp(8.0, 14.0);
    let tall = (h * 0.32).clamp(18.0, 30.0);
    let z = hz - 3.0;
    m.block(Kind::Door, [-w / 2.0, w / 2.0], [-3.0, tall], [z - 1.0, z + 0.4]);
    for side in [-1.0f32, 1.0] {
        let x = side * (w / 2.0 + 2.5);
        m.block(Kind::Stone, [x - 2.5, x + 2.5], [-3.0, tall + 4.0], [z - 3.0, hz]);
    }
    m.block(Kind::Stone, [-w / 2.0 - 5.0, w / 2.0 + 5.0], [tall, tall + 7.0], [z - 3.0, hz]);
}

/// An arena: a ring of tall walls round the footprint, pylons at its
/// corners, fins outside, a gate slot in the front.
fn arena(m: &mut Mono, kind: Kind, h: f32, y0: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let t = (m.w.min(m.d) * 0.12).clamp(6.0, 16.0);
    let gate = 10.0;
    // The floor inside.
    m.block(Kind::Sand, [-hx + t, hx - t], [y0, 0.6], [-hz + t, hz - t]);
    // The ring: front parted at the gate, back, sides.
    m.block(kind, [-hx, -gate], [y0, h], [hz - t, hz]);
    m.block(kind, [gate, hx], [y0, h], [hz - t, hz]);
    m.block(kind, [-hx, hx], [y0, h], [-hz, -hz + t]);
    m.block(kind, [-hx, -hx + t], [y0, h], [-hz + t, hz - t]);
    m.block(kind, [hx - t, hx], [y0, h], [-hz + t, hz - t]);
    // The cornice standing out inward and the lintel over the gate.
    m.block(kind, [-gate - 2.0, gate + 2.0], [h * 0.42, h], [hz - t, hz]);
    m.block(Kind::Door, [-gate, gate], [y0, h * 0.42], [hz - t + 1.0, hz - t + 2.0]);
    for (x, z) in [(-hx, -hz), (hx - t * 1.4, -hz), (-hx, hz - t * 1.4), (hx - t * 1.4, hz - t * 1.4)] {
        m.block(kind, [x, x + t * 1.4], [y0, h + 12.0], [z, z + t * 1.4]);
    }
    m.fins(kind, [-hx + t * 1.4, -gate - 2.0], hz, [8.0, h - 4.0], 9.0, 1.4);
    m.fins(kind, [gate + 2.0, hx - t * 1.4], hz, [8.0, h - 4.0], 9.0, 1.4);
    m.slits([-hx + t * 1.4, -gate - 2.0], hz + 0.5, h * 0.7, 7.0, 9.0, 0.25);
    m.slits([gate + 2.0, hx - t * 1.4], hz + 0.5, h * 0.7, 7.0, 9.0, 0.25);
}

/// Works by the water: battered low walls, a mass, and what each is for.
fn waterworks(m: &mut Mono, kit: Kit, base: f32, y0: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    match kit {
        Kit::Aqueduct => {
            let n = ((m.w / 18.0).floor() as i32).max(2);
            let span = 2.0 * hx / n as f32;
            for k in 0..n {
                let x = -hx + span * (k as f32 + 0.5);
                m.put("arch-12", Kind::Stone, [x, y0, 0.0], [span, base * 0.7 - y0, m.d * 0.42], 0.0);
            }
            m.block(Kind::Stone, [-hx, hx], [base * 0.7, base * 0.7 + 7.0], [-m.d * 0.24, m.d * 0.24]);
            m.block(Kind::Water, [-hx + 1.0, hx - 1.0], [base * 0.7 + 7.0, base * 0.7 + 7.4], [-m.d * 0.12, m.d * 0.12]);
        }
        Kit::Saltworks | Kit::Strand => {
            // Pans (or boats) in rows before a shed.
            let rows = 2;
            for r in 0..rows {
                let z = hz - 6.0 - r as f32 * 11.0;
                for k in 0..3 {
                    let x = -hx + 6.0 + k as f32 * (2.0 * hx - 12.0) / 2.0;
                    let kind = if kit == Kit::Saltworks { Kind::Plaster } else { Kind::Wood };
                    m.block(kind, [x - 7.0, x + 7.0], [y0, 3.0], [z - 4.0, z + 4.0]);
                }
            }
            m.block(Kind::Stone, [-hx, -hx + m.w * 0.4], [y0, base * 0.55], [-hz, -hz + m.d * 0.4]);
            m.cornice(Kind::Stone, [-hx, -hx + m.w * 0.4], [-hz, -hz + m.d * 0.4], base * 0.55, 0.0);
        }
        _ => {
            // A battered quay along the front, a mass behind.
            m.put("wedge", Kind::Stone, [0.0, y0, hz - 5.0], [2.0 * hx, 10.0 - y0, 10.0], 180.0);
            let h = base * if kit == Kit::Watermill { 0.85 } else { 0.6 };
            m.block(Kind::Stone, [-hx + 3.0, hx - 3.0], [y0, h], [-hz, hz - 10.0]);
            m.cornice(Kind::Stone, [-hx + 3.0, hx - 3.0], [-hz, hz - 10.0], h, 1.2);
            m.fins(Kind::Stone, [-hx + 4.0, hx - 4.0], hz - 10.0, [10.0, h - 4.0], 8.0, 1.2);
            m.block(Kind::Door, [-4.0, 4.0], [y0, h * 0.4], [hz - 10.2, hz - 9.6]);
            if kit == Kit::TollBridge || kit == Kit::Causeway {
                for side in [-1.0f32, 1.0] {
                    let x = side * (hx - 6.0);
                    m.block(Kind::Stone, [x - 6.0, x + 6.0], [y0, h * 1.4], [-hz + 2.0, -hz + 14.0]);
                }
            }
        }
    }
}

/// A ruin: stubs of wall of many heights, a broken tower, a fallen slab.
fn ruin(m: &mut Mono, h: f32, y0: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let n = 6;
    for k in 0..n {
        let t = k as f32 / (n - 1) as f32;
        let x = -hx + 4.0 + t * (2.0 * hx - 8.0);
        let z = if k % 2 == 0 { -hz + 4.0 } else { hz - 6.0 };
        let tall = h * (0.15 + 0.5 * m.unit(60 + k as u32));
        m.block(Kind::Stone, [x - 3.5, x + 3.5], [y0, tall], [z - 2.5, z + 2.5]);
    }
    // The broken tower at a back corner.
    let tw = (m.w * 0.26).clamp(10.0, 20.0);
    m.block(Kind::Stone, [-hx, -hx + tw], [y0, h * 1.1], [-hz, -hz + tw]);
    m.block(Kind::Stone, [-hx + tw * 0.4, -hx + tw], [h * 1.1, h * 1.25], [-hz, -hz + tw * 0.6]);
    m.put("box", Kind::Stone, [hx * 0.2, y0, hz * 0.1], [m.w * 0.4, 5.0 - y0, 5.0], 14.0);
}

/// A well: a stepped square kerb, a dark mouth, a lintel on two posts.
fn well(m: &mut Mono, y0: f32) {
    let (hx, hz) = (m.w / 2.0 - 1.0, m.d / 2.0 - 1.0);
    m.block(Kind::Stone, [-hx, hx], [y0, 3.0], [-hz, hz]);
    m.block(Kind::Stone, [-hx + 2.0, hx - 2.0], [3.0, 7.0], [-hz + 2.0, hz - 2.0]);
    m.block(Kind::Door, [-hx + 4.0, hx - 4.0], [7.0, 7.3], [-hz + 4.0, hz - 4.0]);
    for side in [-1.0f32, 1.0] {
        let x = side * (hx - 2.5);
        m.block(Kind::Stone, [x - 1.2, x + 1.2], [7.0, 17.0], [-1.2, 1.2]);
    }
    m.block(Kind::Stone, [-hx + 1.0, hx - 1.0], [17.0, 19.5], [-1.6, 1.6]);
}

/// Logs stacked in a yard's front corner.
fn logs(m: &mut Mono, y0: f32) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    for k in 0..3 {
        let y = y0 + 2.0 + k as f32 * 3.5;
        m.block(Kind::Trunk, [-hx + 1.0 + k as f32, -hx + 13.0 - k as f32], [y, y + 3.5], [hz - 8.0, hz - 1.0]);
    }
}

/// The open places (gardens, butts, greens, fairs, stalls, flags...): a low
/// kerb round the footprint, parted at the front, and what each stands in
/// it, low or slender, a monolith or two where it has one.
fn yard(m: &mut Mono, kit: Kit, base: f32, y0: f32, s: &mut Stood) {
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let small = m.w.min(m.d) < 24.0;
    if !small {
        let k = 2.5;
        let kerb = Kind::Stone;
        m.block(kerb, [-hx, hx], [y0, 5.0], [-hz, -hz + k]);
        m.block(kerb, [-hx, -hx + k], [y0, 5.0], [-hz + k, hz]);
        m.block(kerb, [hx - k, hx], [y0, 5.0], [-hz + k, hz]);
        m.block(kerb, [-hx + k, -6.0], [y0, 5.0], [hz - k, hz]);
        m.block(kerb, [6.0, hx - k], [y0, 5.0], [hz - k, hz]);
    }
    let (ix, iz) = (hx - 4.0, hz - 4.0);
    match kit {
        Kit::Orchard | Kit::Garden | Kit::Flowerbeds => {
            let (nx, nz) = (((2.0 * ix) / 11.0).floor().max(1.0) as i32, ((2.0 * iz) / 11.0).floor().max(1.0) as i32);
            for j in 0..nz {
                for i in 0..nx {
                    let x = -ix + (i as f32 + 0.5) * 2.0 * ix / nx as f32;
                    let z = -iz + (j as f32 + 0.5) * 2.0 * iz / nz as f32;
                    let u = m.unit(100 + (j * nx + i) as u32);
                    if kit == Kit::Orchard {
                        let [wx, wz] = m.world(x, z);
                        m.pieces.push(spire(wx, y0, wz, 4.5, 26.0 + 10.0 * u, u * 60.0));
                    } else {
                        m.block(if kit == Kit::Garden { Kind::Hedge } else { Kind::Crop }, [x - 4.0, x + 4.0], [y0, 6.0 + 4.0 * u], [z - 3.0, z + 3.0]);
                    }
                }
            }
        }
        Kit::HedgeMaze => {
            for ring in 0..3 {
                let r = ring as f32 * 7.0;
                let (ax, az) = (ix - r, iz - r);
                if ax < 4.0 || az < 4.0 {
                    break;
                }
                let gap = if ring % 2 == 0 { (4.0, 1.0) } else { (1.0, 4.0) };
                m.block(Kind::Hedge, [-ax, ax], [y0, 14.0], [-az, -az + 2.5]);
                m.block(Kind::Hedge, [-ax, -gap.0], [y0, 14.0], [az - 2.5, az]);
                m.block(Kind::Hedge, [gap.1 + 3.0, ax], [y0, 14.0], [az - 2.5, az]);
                m.block(Kind::Hedge, [-ax, -ax + 2.5], [y0, 14.0], [-az, az]);
                m.block(Kind::Hedge, [ax - 2.5, ax], [y0, 14.0], [-az, az]);
            }
        }
        Kit::Butts | Kit::Board | Kit::Flags | Kit::Kites | Kit::Downs | Kit::Quintain | Kit::Scarecrow => {
            // Standing slabs: targets, a board, banners as monoliths.
            let n = match kit {
                Kit::Board | Kit::Quintain | Kit::Scarecrow => 1,
                Kit::Butts => 3,
                _ => 3,
            };
            let tall = match kit {
                Kit::Flags | Kit::Kites => base * 0.9,
                Kit::Downs => base * 0.55,
                Kit::Board => 30.0,
                Kit::Quintain | Kit::Scarecrow => 22.0,
                _ => 18.0,
            };
            for k in 0..n {
                let x = if n == 1 { 0.0 } else { -ix + 4.0 + k as f32 * (2.0 * ix - 8.0) / (n - 1) as f32 };
                let slender = matches!(kit, Kit::Quintain | Kit::Scarecrow);
                let (sw, sd) = if slender { (2.0, 2.0) } else if kit == Kit::Board { ((2.0 * ix).min(40.0), 3.0) } else { (6.0, 3.5) };
                let turn = (m.unit(80 + k) - 0.5) * 10.0;
                m.put("box", if matches!(kit, Kit::Butts | Kit::Board) { Kind::Wood } else { Kind::Stone }, [x, y0, -iz * 0.2], [sw, tall - y0, sd], turn);
                if slender {
                    m.block(Kind::Wood, [x - 7.0, x + 7.0], [tall - 7.0, tall - 5.5], [-iz * 0.2 - 0.8, -iz * 0.2 + 0.8]);
                }
            }
        }
        Kit::Fair | Kit::Stage | Kit::Laundry | Kit::GoosePen | Kit::Caravan | Kit::Fletchery => {
            // A stage or a green: a stepped platform and a back wall; booths.
            let h = base * 0.5;
            m.block(Kind::Stone, [-ix, ix], [y0, 4.0], [-iz, 0.0]);
            m.block(Kind::Stone, [-ix + 3.0, ix - 3.0], [4.0, 8.0], [-iz, -iz * 0.4]);
            m.block(Kind::Stone, [-ix, ix], [y0, h], [-iz, -iz + 4.0]);
            m.cornice(Kind::Stone, [-ix, ix], [-iz, -iz + 4.0], h, 1.0);
            for k in 0..2 {
                let x = if k == 0 { -ix * 0.6 } else { ix * 0.6 };
                m.block(Kind::Cloth, [x - 6.0, x + 6.0], [y0, 12.0], [iz * 0.3 - 5.0, iz * 0.3 + 5.0]);
                m.put("pyramid", Kind::Cloth, [x, 12.0, iz * 0.3], [13.0, 8.0, 11.0], 0.0);
            }
            if kit == Kit::Fair {
                let at = m.brazier(0.0, iz * 0.5, 0.0);
                s.lamp(m, at, 56.0, 0.8, 1.0);
            }
        }
        Kit::Stall | Kit::Pavilion | Kit::Shrine => {
            let h = if kit == Kit::Pavilion { 16.0 } else { 9.0 };
            m.block(if kit == Kit::Pavilion { Kind::Cloth } else { Kind::Wood }, [-hx + 1.0, hx - 1.0], [y0, h], [-hz + 1.0, hz - 1.0]);
            m.put("pyramid", if kit == Kit::Pavilion { Kind::Cloth } else { Kind::Slate }, [0.0, h, 0.0], [m.w - 1.0, 10.0, m.d - 1.0], 0.0);
        }
        _ => {
            // Anything else: a low mass in the yard.
            if small {
                m.block(Kind::Stone, [-hx, hx], [y0, base * 0.5], [-hz, hz]);
            } else {
                hall(m, Kind::Stone, base * 0.6, y0, 0.2);
            }
        }
    }
}

/// One of the run's finds standing on its tile (`world::finds`), over the
/// footprint `foot` (kingdom pixels), facing south as its flat art does: in
/// the same language, small enough for the open ground it stands on. A
/// campfire burns (a lamp); a shrine's candle once a knight has knelt; a
/// ruin's cache till it is taken.
pub fn find(kind: crate::world::finds::Kind, foot: [f32; 4], seed: u32, found: bool) -> Stood {
    use crate::world::finds::Kind as F;
    let mut m = Mono::on(foot, super::kit::Face::South, seed);
    let mut s = Stood::default();
    let (hx, hz) = (m.w / 2.0, m.d / 2.0);
    let y0 = -1.5;
    match kind {
        F::Shrine => {
            m.block(Kind::Stone, [-hx, hx], [y0, 3.0], [-hz, hz]);
            m.block(Kind::Stone, [-hx + 2.0, hx - 2.0], [3.0, 6.0], [-hz + 1.5, hz - 1.5]);
            m.block(Kind::Stone, [-hx + 3.0, hx - 3.0], [6.0, 26.0], [-hz + 2.0, -hz + 6.0]);
            m.block(Kind::Stone, [-hx + 2.0, hx - 2.0], [26.0, 29.0], [-hz + 1.5, -hz + 6.5]);
            m.block(Kind::Door, [-2.5, 2.5], [8.0, 18.0], [-hz + 6.0, -hz + 6.3]);
            if found {
                m.block(Kind::Fire, [-0.8, 0.8], [8.0, 10.5], [-hz + 6.3, -hz + 7.6]);
                s.lamp(&m, [0.0, 12.0, -hz + 8.0], 36.0, 0.7, 1.0);
            }
        }
        F::Stones => {
            for k in 0..4 {
                let x = -hx + 5.0 + k as f32 * (2.0 * hx - 10.0) / 3.0;
                let z = if k == 1 || k == 2 { -hz + 3.0 } else { 0.0 };
                let tall = 18.0 + 14.0 * m.unit(10 + k);
                m.put("box", Kind::Stone, [x, y0, z], [5.5, tall - y0, 3.2], (m.unit(20 + k) - 0.5) * 16.0);
            }
        }
        F::Cave => {
            heap(&mut m, 26.0, y0);
            m.block(Kind::Door, [-4.0, 4.0], [y0, 12.0], [hz - 3.5, hz - 2.6]);
            m.block(Kind::Stone, [-7.0, 7.0], [12.0, 15.0], [hz - 4.5, hz - 1.0]);
        }
        F::Campfire => {
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::TAU;
                let (x, z) = (a.cos() * (hx - 1.5), a.sin() * (hz - 1.5));
                m.block(Kind::Rock, [x - 1.3, x + 1.3], [y0, 2.2], [z - 1.2, z + 1.2]);
            }
            m.put("box", Kind::Trunk, [0.0, 0.0, 0.0], [8.0, 1.6, 1.6], 30.0);
            m.put("box", Kind::Trunk, [0.0, 0.0, 0.0], [8.0, 1.6, 1.6], -30.0);
            m.block(Kind::Fire, [-1.8, 1.8], [1.0, 6.5], [-1.5, 1.5]);
            s.lamp(&m, [0.0, 7.0, 0.0], 50.0, 0.85, 2.0);
        }
        F::LostPack => {
            m.block(Kind::Cloth, [-hx + 3.0, hx - 6.0], [y0, 5.0], [-hz + 1.0, hz - 1.0]);
            m.block(Kind::Wood, [hx - 6.0, hx - 2.0], [y0, 3.0], [-hz + 1.5, hz - 1.5]);
        }
        F::Hermit => {
            m.block(Kind::Stone, [-hx, hx], [y0, 10.0], [-hz, -hz + 3.0]);
            m.put("wedge", Kind::Thatch, [0.0, 0.0, -hz + 3.0 + (2.0 * hz - 4.0) / 2.0], [2.0 * hx, 13.0, 2.0 * hz - 4.0], 0.0);
        }
        F::Ruin => {
            for k in 0..4 {
                let x = -hx + 3.5 + k as f32 * (2.0 * hx - 7.0) / 3.0;
                let tall = 6.0 + 16.0 * m.unit(30 + k);
                m.block(Kind::Stone, [x - 3.0, x + 3.0], [y0, tall], [-hz, -hz + 4.0]);
            }
            if !found {
                m.block(Kind::Wood, [-2.5, 2.5], [0.0, 3.0], [hz - 4.0, hz - 1.0]);
            }
        }
        F::Well => well(&mut m, y0),
    }
    s.take(m);
    s
}

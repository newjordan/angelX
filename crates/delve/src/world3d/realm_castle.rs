//! Castles in the monolith language (docs/REALM-MONOLITH.md, "Scale"): great
//! exterior walls a knight walks round and never sees whole. A castle is
//! built from its footprint, the tile before its gate and the way it faces,
//! so the map's castles (`world::houses`, the Keep, the Dragon Keep today;
//! the great footprints `realm/map` gives them next) all come through
//! [`castle`]:
//!
//! - **A great footprint** (16 tiles wide or more, 6 deep): curtain walls
//!   round it on stepped plinths, ribbed with fins under a heavy cornice,
//!   square towers at its corners, two gate towers and the gate's deep
//!   slot between them on the side it faces, a keep as a monolith inside.
//! - **A small one** (today's): one great keep on a stepped plinth filling
//!   it, towers at its corners rising past it, the gate a deep slot under a
//!   lintel where the gate is, slits high up, braziers either side of the
//!   gate and a lamp hung in it.
//!
//! Nothing stands past the footprint. Each house's walls and tops change
//! its stone's tone and its crown; the Dragon Keep is dark and broken.

use angel_stage::set::{Kind, SetLamp};

use super::blockout::{Rect, px};
use super::chunks::Land;
use super::kit::Face;
use super::realm_ground::Terrain;
use super::realm_mono::Mono;
use super::realm_place::Stood;
use crate::world::houses;

/// What crowns a castle's towers and walls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Crown {
    /// Merlons along the tops.
    Merlons,
    /// A stepped crown, inset tier on tier.
    Stepped,
    /// Square caps on the towers, of this kind.
    Caps(Kind),
    /// Broken: towers of ragged heights, no crown.
    Broken,
}

/// A castle to build: its footprint (tiles), the middle of its gate (the
/// kingdom's pixels, on the face it opens in), the way its gate faces, the
/// other doors in its face (their middles), its stone, its crown, how many
/// towers, how tall.
#[derive(Clone, Debug)]
pub struct Site {
    pub name: String,
    pub foot: Rect,
    pub gate: [f32; 2],
    pub face: Face,
    pub doors: Vec<[f32; 2]>,
    pub walls: Kind,
    pub crown: Crown,
    pub towers: u8,
    pub seed: u32,
    /// How tall the curtain (or a small castle's body) is, kingdom pixels.
    pub height: f32,
}

/// The kingdom's castles as the map holds them (`world::castles`): the
/// houses' great walls on the March, the King's Keep across the north of
/// Castle Town (the Rookery's and the Chapel's doors in its face), the
/// Dragon Keep on the crater's rim.
pub fn sites(_land: &Land) -> Vec<Site> {
    use crate::world::castles::{CastleKind, Facing};
    crate::world::castles()
        .iter()
        .map(|c| {
            let face = match c.facing {
                Facing::South => Face::South,
                Facing::North => Face::North,
                Facing::East => Face::East,
                Facing::West => Face::West,
            };
            let seed = super::chunks::seed_of(c.foot);
            let (gx, gy, gw, gh) = c.gate;
            let gate = [(gx as f32 + gw as f32 / 2.0) * 16.0, (gy as f32 + gh as f32 / 2.0) * 16.0];
            let doors = c.doors.iter().map(|&(_, (x, y))| [x as f32 * 16.0 + 8.0, y as f32 * 16.0 + 8.0]).collect();
            let (walls, crown, towers, height) = match c.kind {
                CastleKind::House(id) => {
                    let house = houses::get(id);
                    let walls = match house.walls {
                        houses::Walls::PaleStone | houses::Walls::Plaster => Kind::Plaster,
                        houses::Walls::DarkStone => Kind::Stone,
                        houses::Walls::Timber => Kind::Wood,
                    };
                    let crown = match house.tops {
                        houses::Tops::Battlements => Crown::Merlons,
                        houses::Tops::Domes => Crown::Stepped,
                        houses::Tops::Pointed => Crown::Caps(match house.roof {
                            houses::RoofKind::Tile => Kind::Roof,
                            houses::RoofKind::Thatch => Kind::Thatch,
                            houses::RoofKind::Slate => Kind::Slate,
                        }),
                    };
                    (walls, crown, if house.towers >= 3 { 4 } else { 2 }, 160.0 + 30.0 * unit(seed, 1))
                }
                CastleKind::Keep => (Kind::Stone, Crown::Merlons, 4, 190.0),
                CastleKind::DragonKeep => (Kind::Slate, Crown::Broken, 4, 170.0),
            };
            Site { name: c.name.clone(), foot: c.foot, gate, face, doors, walls, crown, towers, seed, height }
        })
        .collect()
}

/// Whether build kit `kit` is a castle's (stood by [`castle`], not by
/// `realm_place`).
pub fn is_castle(kit: super::kit::Kit) -> bool {
    use super::kit::Kit;
    matches!(kit, Kit::Castle(_) | Kit::Keep | Kit::Turret | Kit::DragonKeep)
}

fn unit(seed: u32, salt: u32) -> f32 {
    let mut h = seed ^ salt.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}

/// The castle `site` as it stands.
pub fn castle(site: &Site, terrain: &Terrain) -> Stood {
    let foot = px(site.foot);
    let mut m = Mono::on(foot, site.face, site.seed);
    let y0 = terrain.lowest(foot).min(0.0) - 3.0;
    let centre = [(foot[0] + foot[2]) / 2.0, (foot[1] + foot[3]) / 2.0];
    // A point's place along the front, in the castle's own frame.
    let along = |p: [f32; 2]| match site.face {
        Face::South => p[0] - centre[0],
        Face::North => centre[0] - p[0],
        Face::East => centre[1] - p[1],
        Face::West => p[1] - centre[1],
    };
    let gate = along(site.gate);
    let doors: Vec<f32> = site.doors.iter().map(|&d| along(d)).collect();
    let mut s = Stood::default();
    if m.w >= 160.0 && m.d >= 64.0 {
        great(&mut m, site, gate, &doors, y0, &mut s);
    } else {
        keep(&mut m, site, gate, y0, &mut s);
    }
    s.pieces.extend(m.pieces);
    s
}

/// A small footprint: one great keep filling it.
fn keep(m: &mut Mono, site: &Site, along: f32, y0: f32, s: &mut Stood) {
    let kind = site.walls;
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let h = site.height;
    let broken = site.crown == Crown::Broken;
    let gw = (m.w * 0.16).clamp(8.0, 14.0);
    let gx = along.clamp(-hx + gw / 2.0 + 6.0, hx - gw / 2.0 - 6.0);
    let gap = (gx - gw / 2.0 - 3.0, gx + gw / 2.0 + 3.0);
    // The stepped plinth, parted at the gate on the front.
    let tiers = [(8.0, 0.0), (7.0, 3.0), (7.0, 5.5)];
    let body = 7.5;
    let front = hz - body;
    let mut y = y0;
    for &(rise, inset) in &tiers {
        let top = if y == y0 { 8.0 } else { y + rise };
        m.block(kind, [-hx + inset, hx - inset], [y, top], [-hz + inset, front]);
        m.block(kind, [-hx + inset, gap.0], [y, top], [front, hz - inset]);
        m.block(kind, [gap.1, hx - inset], [y, top], [front, hz - inset]);
        y = top;
    }
    let (bx, bz) = ([-hx + body, hx - body], [-hz + body, front]);
    m.block(kind, bx, [y, h], bz);
    if !broken {
        m.cornice(kind, bx, bz, h, 2.0);
    }
    m.course(kind, bx, bz, h * 0.42);
    m.course(kind, bx, bz, h * 0.64);
    // Towers at the corners, rising past the body.
    let tw = (m.w.min(m.d) * 0.3).clamp(12.0, 30.0);
    let mut corners = vec![(-hx + 1.0, hz - 1.0 - tw), (hx - 1.0 - tw, hz - 1.0 - tw)];
    if site.towers >= 3 {
        corners.push((-hx + 1.0, -hz + 1.0));
        corners.push((hx - 1.0 - tw, -hz + 1.0));
    }
    let mut tower_spans = Vec::new();
    for (k, &(x, z)) in corners.iter().enumerate() {
        let th = if broken { h * (0.7 + 0.5 * unit(site.seed, 30 + k as u32)) } else { h + 34.0 + 10.0 * unit(site.seed, 30 + k as u32) };
        m.block(kind, [x, x + tw], [y0, th], [z, z + tw]);
        if !broken {
            m.block(kind, [x - 1.0, x + tw + 1.0], [th - 7.0, th + 1.0], [z - 1.0, z + tw + 1.0]);
        }
        // A deep slot up its front, a lit window or two in it.
        let fx = x + tw / 2.0;
        let fz = z + tw;
        m.block(Kind::Door, [fx - 1.2, fx + 1.2], [30.0, th - 12.0], [fz - 0.2, fz + 0.25]);
        for r in 0..3 {
            if unit(site.seed, 50 + k as u32 * 3 + r) < if broken { 0.25 } else { 0.4 } {
                let wy = 50.0 + r as f32 * ((th - 70.0) / 3.0).max(12.0);
                m.block(Kind::Window, [fx - 0.8, fx + 0.8], [wy, wy + 6.0], [fz + 0.25, fz + 0.35]);
            }
        }
        match site.crown {
            Crown::Caps(roof) => m.put("pyramid", roof, [x + tw / 2.0, th + 1.0, z + tw / 2.0], [tw + 2.0, tw * 1.1, tw + 2.0], 0.0),
            Crown::Stepped => {
                m.block(kind, [x + 2.5, x + tw - 2.5], [th + 1.0, th + 7.0], [z + 2.5, z + tw - 2.5]);
                m.block(kind, [x + 5.0, x + tw - 5.0], [th + 7.0, th + 12.0], [z + 5.0, z + tw - 5.0]);
            }
            Crown::Merlons => {
                for (mx, mz) in [(x - 1.0, z - 1.0), (x + tw - 3.0, z - 1.0), (x - 1.0, z + tw - 3.0), (x + tw - 3.0, z + tw - 3.0)] {
                    m.block(kind, [mx, mx + 4.0], [th + 1.0, th + 6.0], [mz, mz + 4.0]);
                }
            }
            Crown::Broken => {
                m.block(kind, [x, x + tw * 0.45], [th, th + 10.0], [z, z + tw * 0.6]);
            }
        }
        tower_spans.push((x, x + tw));
    }
    // Fins up the front between the towers, and the slits high in it.
    let (fa, fb) = (-hx + 1.0 + tw + 2.0, hx - 1.0 - tw - 2.0);
    if fb > fa + 8.0 {
        m.fins(kind, [fa, gap.0 - 1.0], bz[1], [y + 2.0, h - 4.0], 8.0, 1.6);
        m.fins(kind, [gap.1 + 1.0, fb], bz[1], [y + 2.0, h - 4.0], 8.0, 1.6);
        m.slits([fa, fb], bz[1] + 1.6, h * 0.72, 7.0, 8.0, if broken { 0.15 } else { 0.3 });
    }
    if site.crown == Crown::Merlons {
        let n = ((bx[1] - bx[0]) / 8.0).floor() as i32;
        for k in 0..n {
            let x = bx[0] + 2.0 + k as f32 * 8.0;
            m.block(kind, [x, x + 4.0], [h + 1.0, h + 6.0], [bz[1] - 2.0, bz[1] + 2.0]);
        }
    } else if site.crown == Crown::Stepped || matches!(site.crown, Crown::Caps(_)) {
        m.block(kind, [bx[0] + 4.0, bx[1] - 4.0], [h + 1.0, h + 7.0], [bz[0] + 4.0, bz[1] - 4.0]);
    }
    // The gate: a deep slot under a heavy lintel, its doors at the back;
    // braziers either side on the plinth; a lamp hung in it.
    let gh = (h * 0.24).clamp(26.0, 44.0);
    m.block(Kind::Door, [gx - gw / 2.0, gx + gw / 2.0], [y0, gh], [bz[1] - 0.6, bz[1] + 0.3]);
    for side in [-1.0f32, 1.0] {
        let jx = gx + side * (gw / 2.0 + 1.75);
        m.block(kind, [jx - 1.75, jx + 1.75], [y0, gh + 6.0], [bz[1], hz]);
    }
    m.block(kind, [gx - gw / 2.0 - 4.0, gx + gw / 2.0 + 4.0], [gh, gh + 9.0], [bz[1], hz]);
    m.block(Kind::Metal, [gx - 0.3, gx + 0.3], [gh * 0.62, gh], [bz[1] + 2.5, bz[1] + 3.1]);
    m.block(Kind::Fire, [gx - 1.4, gx + 1.4], [gh * 0.55, gh * 0.62 + 0.5], [bz[1] + 1.4, bz[1] + 4.2]);
    s.lamps.push((4.0, SetLamp { at: m.point([gx, gh * 0.58, hz + 4.0]), reach: 72.0, strength: if broken { 1.1 } else { 0.95 } }));
    for side in [-1.0f32, 1.0] {
        let bxp = gx + side * (gw / 2.0 + 7.5);
        if bxp.abs() + 3.0 < hx {
            m.brazier(bxp, hz - 3.0, 8.0);
        }
    }
}


/// The spans of `[a, b]` left when `holes` are taken out of it.
fn spans([a, b]: [f32; 2], holes: &[(f32, f32)]) -> Vec<[f32; 2]> {
    let mut out = vec![[a, b]];
    for &(h0, h1) in holes {
        out = out
            .into_iter()
            .flat_map(|[x0, x1]| {
                if h1 <= x0 || h0 >= x1 {
                    vec![[x0, x1]]
                } else {
                    [[x0, h0.max(x0)], [h1.min(x1), x1]].into_iter().filter(|s| s[1] - s[0] > 0.5).collect()
                }
            })
            .collect();
    }
    out
}

/// A great footprint: curtain walls round it on a stepped plinth, square
/// towers at its corners, two gate towers and the gate's deep slot between
/// them, the other doors as slots in the face, a keep as a monolith in the
/// ward (a footprint too shallow for a ward is one great mass inside its
/// walls).
fn great(m: &mut Mono, site: &Site, gate: f32, doors: &[f32], y0: f32, s: &mut Stood) {
    let kind = site.walls;
    let (hx, hz) = (m.w / 2.0 - 0.5, m.d / 2.0 - 0.5);
    let h = site.height;
    let broken = site.crown == Crown::Broken;
    let po = 10.0;
    let t = 18.0;
    let tower = (m.d * 0.3).clamp(32.0, 46.0);
    let th = (h + 52.0).min(245.0);
    let (ox, oz) = (hx - po, hz - po);
    let gw = 22.0;
    let gtw = 30.0;
    let gx = gate.clamp(-ox + tower + gtw + gw / 2.0, ox - tower - gtw - gw / 2.0);
    let solid = 2.0 * (oz - t) < 56.0;
    let doors: Vec<f32> = doors.iter().copied().filter(|d| (d - gx).abs() > gw / 2.0 + gtw + 8.0 && d.abs() < ox - tower - 8.0).collect();
    let mut openings = vec![(gx - gw / 2.0 - gtw, gx + gw / 2.0 + gtw)];
    openings.extend(doors.iter().map(|&d| (d - 7.0, d + 7.0)));
    // The plinth round the outside: two tiers, parted at the gate and the doors.
    for (ya, yb, out) in [(y0, 8.0, po), (8.0, 15.0, po * 0.5)] {
        let (ax, az) = (ox + out, oz + out);
        m.block(kind, [-ax, ax], [ya, yb], [-az, -oz + t]);
        m.block(kind, [-ax, -ox + t], [ya, yb], [-oz + t, oz - t]);
        m.block(kind, [ox - t, ax], [ya, yb], [-oz + t, oz - t]);
        for [a, b] in spans([-ax, ax], &openings) {
            m.block(kind, [a, b], [ya, yb], [oz - t, az]);
        }
    }
    // The curtain: back, sides, the front parted at the gate.
    m.block(kind, [-ox, ox], [y0, h], [-oz, -oz + t]);
    m.block(kind, [-ox, -ox + t], [y0, h], [-oz + t, oz - t]);
    m.block(kind, [ox - t, ox], [y0, h], [-oz + t, oz - t]);
    let front = spans([-ox, ox], &openings[..1]);
    for &[a, b] in &front {
        m.block(kind, [a, b], [y0, h], [oz - t, oz]);
    }
    if solid {
        // Too shallow for a ward: one great mass between the walls, its
        // roof a step under their tops.
        m.block(kind, [-ox + t, ox - t], [y0, h - 8.0], [-oz + t, oz - t]);
    }
    if !broken {
        m.cornice(kind, [-ox, ox], [-oz, -oz + t], h, 2.0);
        for &[a, b] in &front {
            m.cornice(kind, [a, b], [oz - t, oz], h, 2.0);
        }
        m.cornice(kind, [-ox, -ox + t], [-oz + t, oz - t], h, 2.0);
        m.cornice(kind, [ox - t, ox], [-oz + t, oz - t], h, 2.0);
    }
    // The front: courses, fins, slits high in it, the doors as deep slots.
    let (fa, fb) = (-ox + tower, ox - tower);
    for [a, b] in spans([fa, fb], &openings) {
        m.course(kind, [a, b], [oz - t, oz], h * 0.4);
        m.course(kind, [a, b], [oz - t, oz], h * 0.62);
        m.fins(kind, [a, b], oz, [17.0, h - 5.0], 20.0, 2.2);
        m.slits([a, b], oz + 0.6, h * 0.72, 9.0, 20.0, if broken { 0.15 } else { 0.3 });
    }
    for &d in &doors {
        m.door(kind, d, oz, 0.0, [9.0, 28.0], 4.0);
    }
    // The back wall's inner face shows over the front: fins on it too.
    m.fins(kind, [-ox + t + 4.0, ox - t - 4.0], -oz + t, [h * 0.35, h - 5.0], 20.0, 2.0);
    // Corner towers, from the footprint's edge, rising past the curtain.
    let mut corners = vec![(-hx + 1.0, hz - 1.0 - tower), (hx - 1.0 - tower, hz - 1.0 - tower), (-hx + 1.0, -hz + 1.0), (hx - 1.0 - tower, -hz + 1.0)];
    if site.towers < 3 {
        corners.truncate(2);
    }
    for (k, &(x, z)) in corners.iter().enumerate() {
        let top = if broken { h * (0.75 + 0.55 * unit(site.seed, 30 + k as u32)) } else { th - 8.0 * unit(site.seed, 30 + k as u32) };
        m.block(kind, [x, x + tower], [y0, 12.0], [z, z + tower]);
        m.block(kind, [x + 2.0, x + tower - 2.0], [12.0, top], [z + 2.0, z + tower - 2.0]);
        let fz = z + tower - 2.0;
        for sx in [x + tower * 0.32, x + tower * 0.68] {
            m.block(Kind::Door, [sx - 1.3, sx + 1.3], [30.0, top - 14.0], [fz - 0.2, fz + 0.25]);
            for r in 0..3 {
                if unit(site.seed, 60 + k as u32 * 5 + r) < if broken { 0.2 } else { 0.35 } {
                    let wy = 48.0 + r as f32 * ((top - 80.0) / 3.0).max(14.0);
                    m.block(Kind::Window, [sx - 0.8, sx + 0.8], [wy, wy + 7.0], [fz + 0.25, fz + 0.35]);
                }
            }
        }
        m.block(kind, [x + tower * 0.32 + 2.5, x + tower * 0.68 - 2.5], [30.0, top - 14.0], [fz, fz + 1.4]);
        crown(m, site.crown, kind, [x, x + tower], [z, z + tower], top);
    }
    // The gate towers, standing out to the footprint's edge, and the slot.
    let gh = (h * 0.32).clamp(44.0, 64.0);
    for side in [-1.0f32, 1.0] {
        let x = if side < 0.0 { gx - gw / 2.0 - gtw } else { gx + gw / 2.0 };
        let top = if broken { h * (0.8 + 0.3 * unit(site.seed, 90 + side as u32)) } else { h + 26.0 };
        m.block(kind, [x, x + gtw], [y0, top], [oz - t - 10.0, hz - 1.0]);
        if !broken {
            m.block(kind, [x - 1.5, x + gtw + 1.5], [top - 7.0, top + 1.0], [oz - t - 11.5, hz - 0.5]);
        }
        let mx = x + gtw / 2.0;
        m.block(Kind::Door, [mx - 1.2, mx + 1.2], [gh + 12.0, top - 12.0], [hz - 1.2, hz - 0.8]);
        crown(m, site.crown, kind, [x, x + gtw], [oz - t - 10.0, hz - 1.0], top);
    }
    // The way through: dark doors deep in, a heavy lintel over it.
    m.block(Kind::Door, [gx - gw / 2.0, gx + gw / 2.0], [y0, gh], [oz - t - 8.0, oz - t - 6.0]);
    m.block(kind, [gx - gw / 2.0, gx + gw / 2.0], [gh, h + 18.0], [oz - t - 10.0, hz - 3.0]);
    m.block(Kind::Sand, [gx - gw / 2.0, gx + gw / 2.0], [y0, 0.4], [oz - t - 6.0, hz]);
    // A lamp hung in the slot, braziers at its mouth.
    m.block(Kind::Metal, [gx - 0.4, gx + 0.4], [gh * 0.62, gh], [hz - 8.4, hz - 7.6]);
    m.block(Kind::Fire, [gx - 1.8, gx + 1.8], [gh * 0.52, gh * 0.62], [hz - 9.8, hz - 6.2]);
    s.lamps.push((4.0, SetLamp { at: m.point([gx, gh * 0.56, hz - 4.0]), reach: 96.0, strength: if broken { 1.15 } else { 1.0 } }));
    for side in [-1.0f32, 1.0] {
        m.brazier(gx + side * (gw / 2.0 - 3.0), hz - 3.5, 0.0);
    }
    // The keep in the ward.
    if !solid {
        let (wz0, wz1) = (-oz + t, oz - t);
        let kx = (ox - t - 28.0).min(70.0);
        let kd = ((wz1 - wz0) * 0.55).clamp(30.0, 70.0);
        let kz1 = wz0 + 10.0 + kd;
        let kh = (h + 64.0).min(245.0);
        if kx > 24.0 {
            m.stepped(kind, [-kx, kx], [wz0 + 10.0, kz1], y0, &[(14.0 - y0, 6.0), (10.0, 3.0)], [true, true, false, true]);
            m.block(kind, [-kx, kx], [y0, if broken { kh * 0.7 } else { kh }], [wz0 + 10.0, kz1]);
            if !broken {
                m.cornice(kind, [-kx, kx], [wz0 + 10.0, kz1], kh, 2.5);
                m.fins(kind, [-kx + 4.0, kx - 4.0], kz1, [h * 0.5, kh - 5.0], 16.0, 2.0);
                m.slits([-kx + 4.0, kx - 4.0], kz1 + 0.5, kh * 0.82, 10.0, 16.0, 0.4);
            }
        }
    }
}

/// A tower's crown over `x` by `z` at `top`.
fn crown(m: &mut Mono, crown: Crown, kind: Kind, [x0, x1]: [f32; 2], [z0, z1]: [f32; 2], top: f32) {
    let (w, d) = (x1 - x0, z1 - z0);
    match crown {
        Crown::Caps(roof) => m.put("pyramid", roof, [(x0 + x1) / 2.0, top + 1.0, (z0 + z1) / 2.0], [w - 2.0, w.min(d) * 0.9, d - 2.0], 0.0),
        Crown::Stepped => {
            m.block(kind, [x0 + 4.0, x1 - 4.0], [top + 1.0, top + 8.0], [z0 + 4.0, z1 - 4.0]);
            m.block(kind, [x0 + 8.0, x1 - 8.0], [top + 8.0, top + 14.0], [z0 + 8.0, z1 - 8.0]);
        }
        Crown::Merlons => {
            let n = ((w / 8.0).floor() as i32).max(2);
            for k in 0..n {
                let x = x0 + 1.0 + k as f32 * (w - 6.0) / (n - 1) as f32;
                m.block(kind, [x, x + 4.0], [top + 1.0, top + 7.0], [z1 - 4.0, z1]);
                m.block(kind, [x, x + 4.0], [top + 1.0, top + 7.0], [z0, z0 + 4.0]);
            }
        }
        Crown::Broken => {
            m.block(kind, [x0 + 2.0, x0 + w * 0.45], [top, top + 12.0], [z0 + 2.0, z0 + d * 0.6]);
        }
    }
}

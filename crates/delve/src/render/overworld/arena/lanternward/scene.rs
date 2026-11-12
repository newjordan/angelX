//! The room, from above: tiles, things, bodies, light and the footprints a
//! tell shows. Art never sets a hitbox; every block is drawn where the rules
//! say the thing is.
use super::super::super::ink::{Img, bayer, hash};
use super::super::knights;
use super::super::juice;
use super::super::pfx::Pen;
use super::bank::{self, painted, slot, slot_mid};
use super::{feel, hud, light, moments};
use crate::rules::{
    Run,
    juice::Weight,
    lanternward::{
        self as lw, Act, Atk, Bk, Danger, Dk, Fk, Fs, Hold, Pk, Seat, Session, Ws,
        world::{Q, TILE, angle_of, cos_q, plan, sin_q, unit},
    },
};

/// The picture's frame on the room: where its top left is, in pixels, and
/// whose screen it is.
pub(super) struct View<'a> {
    pub run: &'a Run,
    pub s: &'a Session,
    pub id: u32,
    pub ox: i32,
    pub oy: i32,
    pub tick: u32,
}

impl View<'_> {
    pub fn x(&self, q: i32) -> i32 {
        q.div_euclid(Q) - self.ox
    }
    pub fn y(&self, q: i32) -> i32 {
        q.div_euclid(Q) - self.oy
    }
    fn on_screen(&self, x: i32, y: i32, pad: i32) -> bool {
        x > -pad && y > -pad && x < 384 + pad && y < 224 + pad
    }
}

pub(super) fn draw(run: &Run, focus: Option<u32>) -> Img {
    let s = run.lanternward.as_ref().expect("a Lanternward session");
    let id = focus
        .filter(|id| s.seats.contains_key(id))
        .or_else(|| s.seats.keys().next().copied())
        .unwrap_or(1);
    let cam = s.seats.get(&id).map_or_else(
        || {
            let p = plan(usize::from(s.room));
            (p.w() * TILE * Q / 2, p.h() * TILE * Q / 2)
        },
        |k| k.cam,
    );
    let mut shake = super::super::camera::smooth_shake(u64::from(s.elapsed), u32::from(s.shake), 12, 4.0);
    // A blow is felt through the floor: the heaviest flinch on show shakes the
    // room, and a hold keeps shaking on the realm's own hit-stop count.
    let phase = u64::from(s.elapsed) * 3 + u64::from(run.hitstop.left);
    let blows = s
        .foes
        .iter()
        .filter_map(|f| feel::hit_age(f.flinch).map(|a| (feel::foe_weight_for(s, f), a)))
        .chain(s.boss.iter().filter_map(|w| feel::hit_age(w.flinch).map(|a| (feel::boss_weight_for(s, w), a))));
    let kick = juice::shake(blows, phase);
    shake = (shake.0 + kick.0, shake.1 + kick.1);
    let v = View {
        run,
        s,
        id,
        ox: cam.0.div_euclid(Q) - 192 + shake.0,
        oy: cam.1.div_euclid(Q) - 112 + shake.1,
        tick: s.elapsed,
    };
    let mut im = Img::black(384, 224);
    let dark = plan(usize::from(s.room)).dark
        || (s.room == 4 && s.boss.as_ref().is_some_and(|w| w.phase >= 2 && w.hp > 0));
    tiles(&mut im, &v);
    let lit = if dark { light::levels(&v) } else { Vec::new() };
    if dark {
        light::lit_floor(&mut im, &v, &lit);
    } else {
        light::pools(&mut im, &v);
        footprints(&mut im, &v);
    }
    bodies(&mut im, &v);
    if dark {
        light::gloam(&mut im, &v, &lit);
        // A blow's footprint is read in the dark too, and so is a hood's eyes.
        footprints(&mut im, &v);
        eyes(&mut im, &v);
    }
    effects(&mut im, &v);
    if s.flash > 0 {
        flash(&mut im, s.flash);
    }
    hud::draw(&mut im, &v);
    moments::draw(&mut im, &v);
    im
}

// ----- tiles -----------------------------------------------------------------

/// A room's stone: floor, mortar, speckle.
fn palette(room: u8) -> (char, char, char) {
    match room {
        0 => ('x', 's', 'S'),
        1 => ('b', 'n', 'I'),
        2 => ('x', 's', 'u'),
        3 => ('K', 'k', 'X'),
        _ => ('g', 'K', 'X'),
    }
}

fn tiles(im: &mut Img, v: &View) {
    let p = plan(usize::from(v.s.room));
    let (tx0, ty0) = (v.ox.div_euclid(TILE), v.oy.div_euclid(TILE));
    for ty in ty0..=ty0 + 14 {
        for tx in tx0..=tx0 + 24 {
            if tx < 0 || ty < 0 || tx >= p.w() || ty >= p.h() {
                continue;
            }
            let (x, y) = (tx * TILE - v.ox, ty * TILE - v.oy);
            match p.at(tx, ty) {
                b'#' => wall(im, v, tx, ty, x, y),
                b'=' => pit(im, v, tx, ty, x, y),
                b'Q' => doorway(im, v, tx, ty, x, y),
                c => {
                    floor(im, v.s.room, tx, ty, x, y);
                    if c == b'V' {
                        for bx in [2, 6, 10, 14] {
                            im.rect(x + bx, y, 1, 16, 'h');
                            im.rect(x + bx, y, 1, 2, 'H');
                        }
                    }
                }
            }
        }
    }
    if v.s.room == 4 {
        ring(im, v);
    }
    // A secret closet is black until its way is open.
    if let Some((x0, y0, x1, y1)) = lw::world::closet(usize::from(v.s.room))
        && !v.s.secret_open()
    {
        im.rect(x0 * TILE - v.ox, y0 * TILE - v.oy, (x1 - x0 + 1) * TILE, (y1 - y0 + 1) * TILE, 'k');
    }
}

fn floor(im: &mut Img, room: u8, tx: i32, ty: i32, x: i32, y: i32) {
    let (base, mortar, spec) = palette(room);
    im.rect(x, y, TILE, TILE, base);
    im.rect(x, y + 15, TILE, 1, mortar);
    let off = if ty & 1 == 0 { 0 } else { 8 };
    im.rect(x + (off + 15) % 16, y, 1, 16, mortar);
    let h = hash(tx, ty, 11 + u32::from(room));
    for i in 0..3 {
        let (sx, sy) = (((h >> (i * 5)) & 15) as i32, ((h >> (i * 5 + 2)) & 15) as i32);
        if sx != 15 && sy != 15 {
            im.put(x + sx, y + sy, spec);
        }
    }
}

fn wall(im: &mut Img, v: &View, tx: i32, ty: i32, x: i32, y: i32) {
    let p = plan(usize::from(v.s.room));
    let below_floor = !matches!(p.at(tx, ty + 1), b'#');
    im.rect(x, y, TILE, TILE, 'j');
    // Courses of stone.
    for row in [5, 10] {
        im.rect(x, y + row, TILE, 1, 'g');
    }
    for (row, off) in [(0, 4), (6, 10), (11, 2)] {
        im.rect(x + off, y + row, 1, 5, 'g');
    }
    im.rect(x, y, TILE, 1, 'J');
    if below_floor {
        // The face that looks into the room.
        im.rect(x, y + 12, TILE, 4, 'g');
        im.rect(x, y + 15, TILE, 1, 'K');
    }
    let h = hash(tx, ty, 29);
    im.put(x + (h & 15) as i32, y + ((h >> 4) & 7) as i32, 'G');
}

fn pit(im: &mut Img, v: &View, tx: i32, ty: i32, x: i32, y: i32) {
    let p = plan(usize::from(v.s.room));
    im.rect(x, y, TILE, TILE, 'k');
    for d in (0..TILE * 2).step_by(5) {
        for i in 0..=d.min(TILE - 1) {
            let (px, py) = (x + i, y + d - i);
            if py >= y && py < y + TILE && px < x + TILE {
                im.put(px, py, 'K');
            }
        }
    }
    if p.at(tx, ty - 1) != b'=' {
        im.rect(x, y, TILE, 3, 'X');
        im.rect(x, y + 3, TILE, 1, 'K');
    }
    if p.at(tx - 1, ty) != b'=' {
        im.rect(x, y, 2, TILE, 'X');
    }
    if p.at(tx + 1, ty) != b'=' {
        im.rect(x + TILE - 2, y, 2, TILE, 'X');
    }
}

fn doorway(im: &mut Img, v: &View, _tx: i32, ty: i32, x: i32, y: i32) {
    im.rect(x, y, TILE, TILE, 'k');
    for step in 0..4 {
        im.rect(x + 1, y + 2 + step * 4, TILE - 2, 1, if v.s.exit_open() { '4' } else { 'g' });
    }
    let _ = ty;
}

/// The Warden's floor: a great ring and its spokes.
fn ring(im: &mut Img, v: &View) {
    let p = plan(4);
    let (cx, cy) = (p.w() * TILE / 2 - v.ox, 9 * TILE - v.oy);
    for r in [60, 96] {
        for a in 0..160 {
            let (px, py) = polar(cx, cy, a * 360 / 160, r, 90);
            im.put(px, py, 'X');
        }
    }
}

// ----- footprints -------------------------------------------------------------

/// Where a blow will land (ochre while it is a tell, red while it lands).
fn footprints(im: &mut Img, v: &View) {
    for f in &v.s.foes {
        if f.alive() && f.danger != Danger::None {
            let hot = matches!(f.st, Fs::Strike);
            danger(im, v, f.danger, hot, u32::from(f.t));
        }
    }
    if let Some(w) = &v.s.boss
        && w.hp > 0
        && w.danger != Danger::None
    {
        let hot = w.st == Ws::Strike;
        danger(im, v, w.danger, hot, u32::from(w.t));
        if hot && w.atk == Atk::Slam {
            danger(im, v, Danger::Ring { x: w.x, y: w.y, r: (12 + lw::SLAM_SPEED * i32::from(lw::SLAM_TICKS - w.t)) * Q, w: 6 * Q }, true, 0);
        }
    }
}

fn inside(d: Danger, x: i32, y: i32) -> bool {
    match d {
        Danger::Fan { x: cx, y: cy, deg, half, r } => {
            let dd = lw::world::dist(x - cx, y - cy);
            dd <= r
                && (dd < Q
                    || lw::world::dot(unit(x - cx, y - cy), lw::world::dir(deg)) >= lw::world::cos_q(half))
        }
        _ => d.hits(x, y, 0),
    }
}

fn danger(im: &mut Img, v: &View, d: Danger, hot: bool, ticks: u32) {
    let (lo, hi) = bounds(d);
    let (x0, y0, x1, y1) = (v.x(lo.0) - 2, v.y(lo.1) - 2, v.x(hi.0) + 2, v.y(hi.1) + 2);
    let ink = if hot { '7' } else { 'O' };
    let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(383), y1.min(223));
    for py in y0..=y1 {
        for px in x0..=x1 {
            let (qx, qy) = ((px + v.ox) * Q + Q / 2, (py + v.oy) * Q + Q / 2);
            if !inside(d, qx, qy) {
                continue;
            }
            // The edge, and a dither of fill that quickens as the blow nears.
            let edge = !inside(d, qx + 2 * Q, qy) || !inside(d, qx - 2 * Q, qy) || !inside(d, qx, qy + 2 * Q) || !inside(d, qx, qy - 2 * Q);
            let pulse = if hot { 0.55 } else { 0.22 + 0.3 * (1.0 - (ticks.min(60) as f32 / 60.0)) };
            // A dashed edge and a thin stipple, quicker as the blow nears.
            if edge && (px + py + (v.tick / 3) as i32) & 1 == 0 || (!edge && bayer(px, py) < pulse * 0.3) {
                im.put(px, py, ink);
            }
        }
    }
}

fn bounds(d: Danger) -> ((i32, i32), (i32, i32)) {
    match d {
        Danger::None => ((0, 0), (0, 0)),
        Danger::Capsule { ax, ay, bx, by, r } => ((ax.min(bx) - r, ay.min(by) - r), (ax.max(bx) + r, ay.max(by) + r)),
        Danger::Circle { x, y, r } => ((x - r, y - r), (x + r, y + r)),
        Danger::Fan { x, y, r, .. } => ((x - r, y - r), (x + r, y + r)),
        Danger::Ring { x, y, r, w } => ((x - r - w, y - r - w), (x + r + w, y + r + w)),
    }
}

// ----- bodies ------------------------------------------------------------------

enum Thing {
    Prop(usize),
    Knight(u32),
    Foe(usize),
    Boss,
    Drop(usize),
    Fuse(usize),
    Bolt(usize),
    Key,
}

fn bodies(im: &mut Img, v: &View) {
    let s = v.s;
    let mut order: Vec<(i32, i32, Thing)> = Vec::new();
    for (i, p) in s.props.iter().enumerate() {
        let y = if p.kind == Pk::Block { p.y } else { p.y + 6 * Q };
        // Floor things go under everything that stands.
        let rank = i32::from(!matches!(p.kind, Pk::Plate | Pk::Sigil | Pk::Fissure | Pk::Grass));
        order.push((rank, y, Thing::Prop(i)));
    }
    for (i, d) in s.drops.iter().enumerate() {
        order.push((1, d.y, Thing::Drop(i)));
    }
    for (i, f) in s.fuses.iter().enumerate() {
        order.push((1, f.y, Thing::Fuse(i)));
    }
    for (&id, k) in &s.seats {
        order.push((2, k.y, Thing::Knight(id)));
    }
    for (i, f) in s.foes.iter().enumerate() {
        order.push((2, f.y, Thing::Foe(i)));
    }
    if let Some(w) = &s.boss {
        order.push((2, w.y, Thing::Boss));
    }
    for (i, b) in s.bolts.iter().enumerate() {
        order.push((3, b.y, Thing::Bolt(i)));
    }
    if matches!(s.inv.boss, Hold::Lying(..)) {
        order.push((1, 0, Thing::Key));
    }
    order.sort_by_key(|&(rank, y, _)| (rank, y));
    for (_, _, t) in order {
        match t {
            Thing::Prop(i) => prop(im, v, i),
            Thing::Drop(i) => drop_at(im, v, i),
            Thing::Fuse(i) => fuse(im, v, i),
            Thing::Knight(id) => knight(im, v, id),
            Thing::Foe(i) => foe(im, v, i),
            Thing::Boss => boss(im, v),
            Thing::Bolt(i) => bolt(im, v, i),
            Thing::Key => {
                if let Hold::Lying(x, y) = s.inv.boss {
                    key_icon(im, v.x(x), v.y(y) - 4);
                }
            }
        }
    }
}

fn shadow(im: &mut Img, x: i32, y: i32, w: i32) {
    for dx in -w..=w {
        let ink = if dx.abs() == w && bayer(x + dx, y) < 0.5 { None } else { Some('s') };
        if let Some(c) = ink {
            im.put(x + dx, y, c);
            if dx.abs() < w {
                im.put(x + dx, y + 1, c);
            }
        }
    }
}

fn flame(im: &mut Img, x: i32, y: i32, tick: u32, big: bool) {
    let sway = [0, 1, 0, -1][(tick / 3 % 4) as usize];
    let h = if big { 8 } else { 5 };
    for dy in 0..h {
        let w = if big { (h - dy).min(4) / 2 + 1 } else { (h - dy).min(3) / 2 + 1 };
        let c = if dy < h / 3 { '6' } else if dy < 2 * h / 3 { '@' } else { '4' };
        im.rect(x - w + sway * (dy / 3), y - dy, w * 2, 1, c);
    }
    im.put(x, y - 1, 'w');
}

/// The ticks a brazier's dying flame plays (`gutter` in `slots.json`, 3 by 4).
const GUTTER: u32 = 12;

fn prop(im: &mut Img, v: &View, i: usize) {
    let p = &v.s.props[i];
    let (x, y) = (v.x(p.x), v.y(p.y));
    if !v.on_screen(x, y, 40) {
        return;
    }
    // What is shut up in a closet is not seen.
    if let Some((x0, y0, x1, y1)) = lw::world::closet(usize::from(v.s.room))
        && !v.s.secret_open()
        && (x0..=x1).contains(&i32::from(p.tx))
        && (y0..=y1).contains(&i32::from(p.ty))
    {
        return;
    }
    let foot = y + 8;
    let t = v.tick;
    match p.kind {
        Pk::Pot => {
            if p.on == 0 {
                shadow(im, x, foot - 1, 5);
                slot(im, "pot", "whole", 0, x, foot);
                if p.id == 1 {
                    // A bomb pot: a black ball in it, the fuse alight.
                    im.rect(x - 2, foot - 10, 5, 4, 'k');
                    im.put(x + 1, foot - 11, if (t / 4).is_multiple_of(2) { '6' } else { '7' });
                }
            }
        }
        Pk::Folk => {
            shadow(im, x, foot - 1, 5);
            let talking = v.s.seats.values().any(|k| matches!(k.read, Some((r, _)) if r >= 100));
            slot(im, "folk", if talking { "talk" } else { "idle" }, t, x, foot);
        }
        Pk::Lever => {
            shadow(im, x, foot - 1, 4);
            slot(im, "lever", if p.on == 1 { "pulled" } else { "up" }, t, x, foot);
        }
        Pk::Grass => {
            let (state, n) = if p.on == 0 { ("tuft", t / 24) } else { ("cut", 0) };
            let sway = if p.on == 0 && (hash(i32::from(p.tx), i32::from(p.ty), 5) + n) & 1 == 0 { 1 } else { 0 };
            slot(im, "grass", state, 0, x + sway, foot);
        }
        Pk::Sign => {
            shadow(im, x, foot - 1, 5);
            slot(im, "sign", "idle", 0, x, foot);
        }
        Pk::Brazier | Pk::Rest | Pk::Great | Pk::Beacon | Pk::Ember => {
            let lit = p.lit();
            let tall = match p.kind {
                Pk::Beacon => 36,
                Pk::Great => 22,
                Pk::Ember => 10,
                _ => 18,
            };
            shadow(im, x, foot - 1, 6);
            // A brazier's last twelve ticks are its dying flame, played once.
            let dying = p.kind == Pk::Brazier && p.on == 0 && u32::from(p.t) <= GUTTER && painted("brazier", "gutter");
            let (id, state) = match p.kind {
                Pk::Brazier => ("brazier", if !lit { "out" } else if dying { "gutter" } else { "lit" }),
                Pk::Rest => ("rest", "lit"),
                Pk::Ember => ("ember", "lit"),
                Pk::Great => ("great", if lit { "lit" } else { "out" }),
                _ => ("beacon", if p.on == 1 { "lit" } else if p.t > 0 { "kindling" } else { "cold" }),
            };
            slot(im, id, state, if dying { GUTTER - u32::from(p.t) } else { t }, x, foot);
            // A painted frame carries its own fire; the code's flame is the block's.
            if (lit || (p.kind == Pk::Beacon && p.t > 0)) && !painted(id, state) {
                flame(im, x, foot - tall, t, p.kind == Pk::Beacon || p.kind == Pk::Great);
            }
            // A lit Beacon sends its light up and out: a pillar that fades as
            // it climbs, ringed by the realm's own rays.
            if p.kind == Pk::Beacon && p.on == 1 {
                let since = v.s.done.map_or(0, |d| v.tick.saturating_sub(d)) as i32;
                let rise = (since * 4).min(foot);
                for dy in 0..rise {
                    let y0 = foot - tall - dy;
                    let w = 2 + (dy / 24).min(3);
                    for dx in -w..=w {
                        if bayer(x + dx, y0) < 1.0 - (dy as f32 / (rise.max(1) as f32 + 40.0)) {
                            im.put(x + dx, y0, if dx.abs() < 2 { 'w' } else { '6' });
                        }
                    }
                }
                for a in 0..8 {
                    let r = 14 + (since / 2) % 12;
                    let (px, py) = polar(x, foot - tall, a * 45, r, 100);
                    im.put(px, py, '5');
                }
            }
            if p.kind == Pk::Beacon && p.on == 0 && v.s.stage >= 3 {
                let w = (i32::from(p.t) * 24 / i32::from(lw::BEACON_TICKS)).min(24);
                im.rect(x - 12, foot + 4, 24, 3, 'k');
                im.rect(x - 12, foot + 4, w, 3, '6');
            }
        }
        Pk::Chest => {
            if p.on != 2 {
                shadow(im, x, foot - 1, 7);
                slot(im, "chest", if p.on == 1 { "open" } else { "closed" }, t, x, foot);
                // The block's dark mouth; a painted chest has its own.
                if p.on == 1 && !painted("chest", "open") {
                    im.rect(x - 5, foot - 13, 10, 3, 'k');
                }
            }
        }
        Pk::Gate => {
            if p.on == 0 {
                slot_mid(im, "gate", "closed", t, x, y);
                // The block's three bars; a painted gate has its own.
                if !painted("gate", "closed") {
                    for bx in [3, 7, 11] {
                        im.rect(x - 8 + bx, y - 8, 1, 16, 'h');
                    }
                }
            } else if painted("gate", "open") {
                slot_mid(im, "gate", "open", t, x, y);
            } else {
                im.rect(x - 8, y - 8, 2, 16, 'j');
                im.rect(x + 6, y - 8, 2, 16, 'j');
            }
        }
        Pk::Plate => {
            slot_mid(im, "plate", if p.on == 1 { "down" } else { "up" }, 0, x, y);
        }
        Pk::Block => {
            shadow(im, x, y + 8, 7);
            slot_mid(im, "block", "idle", 0, x, y);
        }
        Pk::Crystal => {
            shadow(im, x, foot - 1, 5);
            let glow = |im: &mut Img| {
                if p.on == 1 {
                    for r in [10, 14] {
                        ring_px(im, x, foot - 10, r, '3', t);
                    }
                }
            };
            // The glow's dots are over a block, behind a painted crystal.
            let own = painted("crystal", "lit");
            if own {
                glow(im);
            }
            slot(im, "crystal", if p.on == 1 { "lit" } else { "dormant" }, t, x, foot);
            if !own {
                glow(im);
            }
        }
        Pk::Cracked => {
            slot_mid(im, "cracked", if p.on == 0 { "intact" } else { "blown" }, 0, x, y);
        }
        Pk::Sigil => {
            if p.on == 1 {
                slot_mid(im, "sigil", "found", t / 8, x, y);
            } else if p.t > 0 {
                // the rune writes itself as the flare holds
                let n = (usize::from(p.t) * 3 / usize::from(lw::SIGIL_TICKS)).min(2);
                slot_mid(im, "sigil", "revealing", 0, x, y);
                let _ = n;
                im.stamp(bank::frame_at("sigil", "revealing", n), x - 8, y - 8);
            }
        }
        Pk::Lock | Pk::Boss => {
            let id = if p.kind == Pk::Lock { "lock" } else { "boss-door" };
            let state = match (p.kind == Pk::Lock, p.on == 0) {
                (true, true) => "locked",
                (false, true) => "chained",
                _ => "open",
            };
            slot_mid(im, id, state, t, x, y);
        }
        Pk::Fissure => {
            if p.t > 0 {
                slot_mid(im, "fissure", "open", t / 6, x, y);
            } else {
                slot_mid(im, "fissure", "shut", 0, x, y);
            }
        }
    }
}

/// The point `r` pixels from `(x, y)` at whole degrees `deg`, its up and down
/// squashed to `squash` per cent (a ring on the floor seen from above).
pub(super) fn polar(x: i32, y: i32, deg: i32, r: i32, squash: i32) -> (i32, i32) {
    (x + cos_q(deg) * r / Q, y + sin_q(deg) * r * squash / (100 * Q))
}

pub(super) fn ring_px(im: &mut Img, x: i32, y: i32, r: i32, ink: char, tick: u32) {
    for a in 0..32 {
        if (a + tick as i32 / 2) % 3 == 0 {
            continue;
        }
        let (px, py) = polar(x, y, a * 360 / 32, r, 80);
        im.put(px, py, ink);
    }
}

fn sigil(im: &mut Img, x: i32, y: i32, ink: char, tick: u32) {
    ring_px(im, x, y, 6, ink, tick);
    im.line(x - 3, y, x + 3, y, ink);
    im.line(x, y - 3, x, y + 3, ink);
}

fn key_icon(im: &mut Img, x: i32, y: i32) {
    im.rect(x - 3, y - 3, 5, 5, '6');
    im.rect(x - 2, y - 2, 3, 3, 'k');
    im.rect(x + 2, y - 1, 5, 2, '6');
    im.rect(x + 5, y + 1, 1, 2, '6');
}

fn drop_at(im: &mut Img, v: &View, i: usize) {
    let d = &v.s.drops[i];
    let (x, y) = (v.x(d.x), v.y(d.y));
    if !v.on_screen(x, y, 16) || (d.age > lw::DROP_LIFE * 3 / 4 && (d.age / 4).is_multiple_of(2) && d.kind != Dk::Container) {
        return;
    }
    let bob = [0, -1, -1, 0][(v.tick / 4 % 4) as usize];
    shadow(im, x, y + 3, 3);
    match d.kind {
        Dk::Heart => slot(im, "heart", "idle", v.tick, x, y + 3 + bob),
        Dk::Gem => slot(im, "gem", "idle", v.tick, x, y + 3 + bob),
        Dk::Bomb => slot(im, "bomb", "idle", 0, x, y + 3 + bob),
        Dk::Container => {
            slot(im, "container", "idle", v.tick, x, y + 4 + bob);
            ring_px(im, x, y - 2, 9, '6', v.tick);
        }
    }
}

fn fuse(im: &mut Img, v: &View, i: usize) {
    let f = &v.s.fuses[i];
    let (x, y) = (v.x(f.x), v.y(f.y));
    shadow(im, x, y + 3, 4);
    slot(im, "bomb", if f.t < 20 && (f.t / 3).is_multiple_of(2) { "fuse" } else { "idle" }, 0, x, y + 4);
    if (f.t / 4).is_multiple_of(2) {
        im.put(x + 2, y - 8, '6');
    }
}

fn bolt(im: &mut Img, v: &View, i: usize) {
    let b = &v.s.bolts[i];
    let (x, y) = (v.x(b.x), v.y(b.y));
    if !v.on_screen(x, y, 8) {
        return;
    }
    let spin = v.tick / 3;
    // A tail that streaks back along the flight, in the bolt's own inks.
    let (vx, vy) = (b.vx as f32 / Q as f32, b.vy as f32 / Q as f32);
    let speed = vx.hypot(vy).max(0.1);
    let (ux, uy) = (vx / speed, vy / speed);
    let (tail, ink) = match b.kind {
        Bk::Light => (7, ['5', '4']),
        Bk::Bright => (10, ['w', '6']),
        Bk::Iron => (4, ['h', 'g']),
        Bk::Dim => (4, ['h', 'g']),
        Bk::Pot => (0, ['J', 'G']),
    };
    for d in 2..tail {
        let (px_, py_) = ((x as f32 - ux * d as f32).round() as i32, (y as f32 - uy * d as f32).round() as i32);
        if bayer(px_, py_) < 1.0 - d as f32 / tail as f32 {
            im.put(px_, py_, ink[usize::from(d > tail / 2)]);
        }
    }
    match b.kind {
        Bk::Iron => slot_mid(im, "bolt", "iron", 0, x, y),
        Bk::Light => {
            ring_px(im, x, y, 5, '4', v.tick);
            slot_mid(im, "bolt", "light", spin, x, y);
        }
        Bk::Bright => {
            ring_px(im, x, y, 7, 'w', v.tick);
            ring_px(im, x, y, 10, '6', v.tick + 1);
            slot_mid(im, "bolt", "bright", spin, x, y);
        }
        Bk::Dim => slot_mid(im, "bolt", "dim", 0, x, y),
        Bk::Pot => slot(im, "pot", "whole", 0, x, y + 6),
    }
}

// ----- knights ------------------------------------------------------------------

pub(super) fn knight_img(v: &View, id: u32, k: &Seat) -> Img {
    let Some(hero) = v.run.players.get(&id) else {
        return Img::new(16, 16);
    };
    let mut h = hero.clone();
    h.aim_x = k.face.0 as f32 / Q as f32;
    h.aim_y = k.face.1 as f32 / Q as f32;
    h.shielding = k.shield > 0 && k.act == Act::Free;
    h.owned.shield = true;
    h.sword = true;
    // The blade is drawn here (the arm is empty until it swings).
    h.weapon = crate::rules::Weapon::Fists;
    h.forged = None;
    h.swing = u32::from(matches!(k.act, Act::Slash | Act::Lunge | Act::Spin) && k.t < 9 || k.charged());
    let im = knights::figure(&h, id);
    if h.aim_x < -0.3 { im.flip_h() } else { im }
}

fn knight(im: &mut Img, v: &View, id: u32) {
    let s = v.s;
    let k = &s.seats[&id];
    let (x, y) = (v.x(k.x), v.y(k.y));
    if !v.on_screen(x, y, 40) {
        return;
    }
    let lift = k.z / Q;
    if k.act == Act::Down {
        shadow(im, x, y + 4, 8);
        im.rect(x - 7, y - 1, 14, 6, knights::seat_cloth(id));
        im.frame(x - 7, y - 1, 14, 6, 'k');
        im.ellipse((x - 8) as f32, (y + 2) as f32, 3.0, 3.0, 'c');
        // A knight on the ground waits for a friend's hand.
        let w = (k.rev * 14 / lw::REVIVE_HOLD).min(14) as i32;
        im.rect(x - 7, y - 6, 14, 2, 'k');
        im.rect(x - 7, y - 6, w, 2, 'A');
        tag(im, x, y - 14, &format!("{id}"), knights::seat_cloth(id));
        return;
    }
    // Blink while the ward of a hit is on, and as a new seat sits down.
    if k.ward > 0 && (k.ward / 2).is_multiple_of(2) {
        return;
    }
    if k.joined > 0 && v.tick.saturating_sub(k.joined) < 24 && (v.tick / 3).is_multiple_of(2) {
        return;
    }
    shadow(im, x, y + 4, 6 - lift.min(3) / 2);
    let mut fig = knight_img(v, id, k);
    match k.act {
        Act::Roll => fig = knights::rolled(&fig, lw::ROLL_TICKS.saturating_sub(k.t)),
        Act::Hop if k.n == 1 => fig = knights::rolled(&fig, lw::HOP_TICKS.saturating_sub(k.t)),
        Act::Fall => {
            let shrink = (k.t as i32 * 6 / lw::FALL_TICKS as i32).min(8);
            im.rect(x - 6 + shrink / 2, y - shrink, 12 - shrink, 4, 'k');
            return;
        }
        _ => {}
    }
    // Struck: a white frame, then back to the colours.
    if k.act == Act::Hurt && k.t < 3 {
        fig = juice::impact_frame(&fig, 'H');
    }
    let (fw, fh) = (fig.w, fig.h);
    let foot = y + 4 - lift;
    // A roll kicks up dust behind it.
    if k.act == Act::Roll {
        let (ux, uy) = (-k.mv.0.signum(), -k.mv.1.signum());
        for i in 1..=5 {
            let (dx, dy) = (ux * i * 3, uy * i * 2);
            let ink = if i < 3 { 'J' } else { 'G' };
            im.rect(x + dx - 1, foot + dy - 2 - (i / 3), 3 - (i / 3).min(2), 2, ink);
        }
    }
    im.stamp(&fig, x - fw / 2, foot - fh);
    // The shield's first moments are the parry's: a bright rim reads the window.
    let window = lw::Rules::of(&v.run.tune).parry;
    if k.shield > 0 && k.shield <= window && k.act == Act::Free {
        let side = angle_of(k.face.0, k.face.1);
        for a in -40..=40 {
            if a % 6 == 0 {
                let (px_, py_) = polar(x, foot - 7, side + a, 11, 90);
                im.put(px_, py_, if k.shield < 3 { 'w' } else { 'V' });
            }
        }
    }
    // The sword at rest, held low on the facing side.
    if matches!(k.act, Act::Free | Act::Hop | Act::Item) && k.shield == 0 && k.carry == lw::Carry::Nothing {
        let side = if k.face.0 < -Q / 4 { -1 } else { 1 };
        let hx = x + side * 5;
        im.line(hx, foot - 6, hx + side * 2, foot - 13, 'H');
        im.put(hx, foot - 6, 'o');
        im.put(hx - side, foot - 7, 'o');
    }
    // The wick-spark: a light at the hip once the Lantern is theirs.
    if s.inv.lantern {
        let lx = x + if k.face.0 < 0 { -6 } else { 6 };
        im.put(lx, foot - 6, if k.flaring() { 'w' } else { '5' });
        if k.flaring() {
            ring_px(im, x, foot - 8, 12 + (v.tick as i32 / 2 % 3), '4', v.tick);
        }
    }
    if k.charged() && k.act == Act::Free || (k.charge > 4 && k.act == Act::Free && (v.tick / 2).is_multiple_of(2)) {
        let r = if k.charged() { 10 } else { 5 };
        ring_px(im, x, foot - 8, r, 'w', v.tick);
    }
    if k.carry == lw::Carry::Pot {
        slot(im, "pot", "whole", 0, x, foot - fh + 3);
    }
    // A flame from the ember, carried to a brazier, burns lower as it goes.
    if k.taper > 0 {
        let side = if k.face.0 < -Q / 4 { -1 } else { 1 };
        flame(im, x + side * 7, foot - 9, v.tick, k.taper > 90);
    }
    if let Hold::Carried(c, ..) = s.inv.boss
        && u32::from(c) == id
    {
        key_icon(im, x, foot - fh - 7);
    }
    if k.act == Act::Item
        && let Some(item) = k.item
    {
        item_icon(im, x, foot - fh - 12, item, v.tick);
    }
    // The seat's colour under the feet, so a friend is known at a glance.
    im.rect(x - 1, foot + 2, 3, 1, knights::seat_cloth(id));
    feel::swing_arc(im, k, x, foot - 8);
}

fn tag(im: &mut Img, x: i32, y: i32, s: &str, ink: char) {
    im.rect(x - 3, y - 1, 7, 9, 'k');
    im.text(x - 2, y, s, ink);
}

fn item_icon(im: &mut Img, x: i32, y: i32, item: lw::Item, tick: u32) {
    for a in 0..8 {
        let deg = a * 45 + tick as i32 * 3;
        let (ix, iy) = polar(x, y, deg, 9, 100);
        let (ox, oy) = polar(x, y, deg, 13, 100);
        im.line(ix, iy, ox, oy, '6');
    }
    match item {
        lw::Item::Lantern => {
            im.rect(x - 4, y - 5, 8, 9, 'k');
            im.rect(x - 3, y - 4, 6, 7, '6');
            im.rect(x - 1, y - 7, 2, 2, 'g');
        }
        lw::Item::Map => {
            im.rect(x - 5, y - 5, 10, 9, 'T');
            im.frame(x - 5, y - 5, 10, 9, 'B');
            im.line(x - 3, y - 2, x + 2, y + 1, '7');
        }
        lw::Item::SmallKey | lw::Item::BossKey => key_icon(im, x, y),
        lw::Item::Container => {
            im.ellipse(x as f32, y as f32, 5.0, 5.0, '7');
            im.ellipse(x as f32, y as f32, 3.0, 3.0, 'w');
        }
        _ => {
            im.rect(x - 3, y - 3, 6, 6, '4');
        }
    }
}

// ----- foes -----------------------------------------------------------------------

/// A struck body: stood white for the first ticks of the flinch, then squashed
/// wide and stretched tall as it settles. Its feet stay where they were.
fn struck(pic: &Img, flinch: u8, weight: Weight) -> Img {
    let Some(age) = feel::hit_age(flinch) else { return pic.clone() };
    if age < weight.flash() {
        juice::impact_frame(pic, 'H')
    } else {
        juice::squash_stretch(pic, age, weight.squash()).0
    }
}

/// Stamp `pic` standing on `foot` at `x`, centred.
fn stand(im: &mut Img, pic: &Img, x: i32, foot: i32) {
    im.stamp(pic, x - pic.w / 2, foot - pic.h);
}

fn foe(im: &mut Img, v: &View, i: usize) {
    let f = &v.s.foes[i];
    let (x, y) = (v.x(f.x), v.y(f.y));
    if !v.on_screen(x, y, 40) {
        return;
    }
    let weight = feel::foe_weight_for(v.s, f);
    let tick = u64::from(v.tick);
    let seed = hash(i32::from(f.id), f.hp as i32, 9);
    match f.kind {
        Fk::Snuffer => {
            let lit = v.s.light_at(f.x, f.y) > 0 || f.st == Fs::Dying;
            if v.s.room == 3 && !lit && !matches!(f.st, Fs::Tell | Fs::Strike) {
                return;
            }
            let hover = if f.st == Fs::Dying { 0 } else { [0, -1, -2, -1][(v.tick / 5 % 4) as usize] };
            let hooded = f.hooded && f.lit == 0;
            let foot = y + 10 + hover;
            shadow(im, x, y + 9, 5 + (-hover).min(2) / 2);
            let pic = match f.st {
                Fs::Dying => {
                    // it unravels into smoke over the fourteen ticks it has
                    bank::frame_at("snuffer", "die", usize::from(f.dead) * 7 / usize::from(lw::DEATH_TICKS).max(1)).clone()
                }
                _ if hooded => bank::frame("snuffer", "hooded", v.tick).clone(),
                _ if f.flinch > 0 && f.st != Fs::Tell => {
                    bank::frame_at("snuffer", "hit", usize::from(lw::FLINCH_TICKS.saturating_sub(f.flinch)) / 4).clone()
                }
                Fs::Dormant => bank::frame("snuffer", "idle", v.tick).clone(),
                Fs::Drift => bank::frame("snuffer", "drift", v.tick).clone(),
                Fs::Tell => {
                    let done = usize::from(lw::SNUFFER_TELL.saturating_sub(f.t));
                    bank::frame_at("snuffer", "tell", done * 4 / usize::from(lw::SNUFFER_TELL)).clone()
                }
                Fs::Strike => bank::frame("snuffer", "lunge", v.tick).clone(),
                Fs::Recover => bank::frame("snuffer", "recover", v.tick).clone(),
                Fs::Stagger => bank::frame("snuffer", "stagger", v.tick).clone(),
            };
            // It faces the way it will go: the picture is drawn facing right and flipped.
            let pic = if f.fx < -Q / 3 { pic.flip_h() } else { pic };
            // The gathering shudder of a tell: it trembles more as the blow nears.
            let shudder = if f.st == Fs::Tell && f.t < 12 { (v.tick as i32 % 2) * 2 - 1 } else { 0 };
            stand(im, &struck(&pic, f.flinch, weight), x + shudder, foot);
            if f.st == Fs::Dying {
                juice::death_burst(im, (x as f32, (y - 2) as f32), u32::from(f.dead), seed, tick);
            } else if let Some(age) = feel::hit_age(f.flinch) {
                juice::impact_burst(im, (x as f32, (y - 3) as f32), age, seed, weight, tick);
            }
            if f.st == Fs::Tell && f.t < lw::SNUFFER_TELL / 2 {
                feel::exclaim(im, x, y - 22, v.tick);
            }
            if f.st == Fs::Stagger {
                feel::stars(im, x, y - 11 + hover, v.tick, 3, 7);
            }
            if f.alive() && f.hp < f.full {
                let fresh = f.flinch > lw::FLINCH_TICKS / 2;
                bar(im, x, y - 15, 12, f.hp, f.full, if fresh { 'w' } else { 'O' });
            }
            if hooded && f.alive() {
                feel::lamp_icon(im, x, y - 19 + hover, v.tick);
            }
        }
        Fk::Turret => {
            shadow(im, x, y + 11, 7);
            let (state, idx) = match f.st {
                Fs::Tell => {
                    let total = u32::from(f.tell_total(v)).max(1);
                    ("tell", (total.saturating_sub(u32::from(f.t)) * 7 / total).min(6) as usize)
                }
                Fs::Strike => ("fire", usize::from(f.t < 4)),
                Fs::Recover => ("recover", (v.tick / 15 % 4) as usize),
                Fs::Stagger => ("stun", (v.tick / 6 % 2) as usize),
                _ => ("idle", (v.tick / 12 % 2) as usize),
            };
            let pic = bank::frame_at("turret", state, idx);
            stand(im, pic, x, y + 12);
            // The barrel points the way it will fire.
            let recoil = if f.st == Fs::Strike { 3 - i32::from(f.t.min(3)) } else { 0 };
            feel::barrel(im, x, y - 2, (f.fx, f.fy), matches!(f.st, Fs::Tell | Fs::Strike), recoil);
            if f.st == Fs::Strike && f.t < 3 {
                let (mx, my) = (x + f.fx * 12 / Q, y - 2 + f.fy * 12 / Q);
                juice::impact_burst(im, (mx as f32, my as f32), 3 - u32::from(f.t), seed, Weight::Graze, tick);
            }
            if f.st == Fs::Tell && f.t < 14 {
                feel::exclaim(im, x, y - 20, v.tick);
            }
            if f.st == Fs::Stagger {
                feel::stars(im, x, y - 12, v.tick, 3, 8);
            }
        }
    }
}

trait TellTotal {
    fn tell_total(&self, v: &View) -> u16;
}
impl TellTotal for lw::Foe {
    fn tell_total(&self, v: &View) -> u16 {
        lw::Rules::of(&v.run.tune).tell_for(v.s.seats.len() as u32) as u16
    }
}

fn bar(im: &mut Img, x: i32, y: i32, w: i32, hp: u32, full: u32, ink: char) {
    im.rect(x - w / 2 - 1, y - 1, w + 2, 4, 'k');
    im.rect(x - w / 2, y, w, 2, 'X');
    im.rect(x - w / 2, y, (u64::from(hp) * w as u64 / u64::from(full.max(1))) as i32, 2, ink);
}

/// The Warden's frame: which state its body is in and how far through it.
fn warden_pose(v: &View, w: &lw::Warden) -> (&'static str, usize) {
    let tell = lw::Rules::of(&v.run.tune).tell_for(v.s.seats.len() as u32).max(1);
    let through = |total: u32, left: u16, frames: u32| (total.saturating_sub(u32::from(left)) * frames / total.max(1)).min(frames - 1) as usize;
    if w.flinch > 0 && !matches!(w.st, Ws::Dying | Ws::Asleep | Ws::Stagger) {
        return ("hit", usize::from(w.flinch < lw::FLINCH_TICKS / 2));
    }
    match w.st {
        Ws::Asleep => ("sleep", 0),
        // It wakes with a rear: the first moments of the fight.
        Ws::Rest if woke(w) => ("rear", through(30, w.t.saturating_sub(40), 6)),
        Ws::Rest => ("walk", (v.tick / 8 % 4) as usize),
        Ws::Tell => match w.atk {
            Atk::Beam => ("beam-tell", through(tell, w.t, 7)),
            Atk::Slam => ("slam-tell", through(tell, w.t, 7)),
            Atk::Summon => ("summon", through(tell, w.t, 4)),
        },
        Ws::Strike => match w.atk {
            Atk::Beam => ("beam", (v.tick / 6 % 6) as usize),
            Atk::Slam => ("slam", through(u32::from(lw::SLAM_TICKS), w.t, 5)),
            Atk::Summon => ("summon", (v.tick / 6 % 4) as usize),
        },
        Ws::Recover => ("idle", (v.tick / 12 % 2) as usize),
        Ws::Stagger => ("stagger", (v.tick / 10 % 3) as usize),
        Ws::Dying => ("die", (usize::from(w.dead) / 6).min(7)),
    }
}

/// Whether the Warden has only just woken: untouched, at the start, with the
/// rules' wake-up count still running down.
pub(super) fn woke(w: &lw::Warden) -> bool {
    w.st == Ws::Rest && w.hp == w.full && w.n == 0 && w.phase == 0 && w.t > 40
}

fn boss(im: &mut Img, v: &View) {
    let Some(w) = &v.s.boss else { return };
    let (x, y) = (v.x(w.x), v.y(w.y));
    if !v.on_screen(x, y, 60) {
        return;
    }
    let weight = feel::boss_weight_for(v.s, w);
    let tick = u64::from(v.tick);
    shadow(im, x, y + 24, 18);
    shadow(im, x, y + 25, 14);
    let (state, idx) = warden_pose(v, w);
    let pic = bank::frame_at("warden", state, idx);
    // A big blow is about to land: the whole body shudders through the end of the tell.
    let last = match w.st {
        Ws::Tell => w.t < 18,
        _ => false,
    };
    let jitter = if last { (v.tick as i32 % 2) * 2 - 1 } else { 0 };
    let rise = if w.st == Ws::Dying && w.dead > 5 { 0 } else { 0 };
    let mut body = struck(pic, w.flinch, weight);
    // It faces its target: the picture is drawn front-on; a long way off to one side it turns its shoulders.
    if w.face.0 < -Q * 7 / 10 {
        body = body.flip_h();
    }
    stand(im, &body, x + jitter, y + 28 + rise);
    // Rubble thrown up as a slam lands, the dust of a fall.
    if w.st == Ws::Strike && w.atk == Atk::Slam {
        let age = u32::from(lw::SLAM_TICKS.saturating_sub(w.t));
        juice::impact_burst(im, (x as f32, (y + 22) as f32), age, 7, Weight::Crushing, tick);
    }
    // The lamp-heart in its chest: shut, or open and burning.
    if w.st != Ws::Dying {
        heart(im, v, w);
    }
    // Light gathers in the raised palm before a beam.
    if w.st == Ws::Tell && w.atk == Atk::Beam {
        let k = tell_progress(v, w);
        let palm = (x + 15, y - 14);
        let mut pen = Pen::new(im, 160);
        for n in 0..8 {
            let a = (n as f32) * 0.785 + v.tick as f32 * 0.2;
            let r = 22.0 * (1.0 - k) + 3.0;
            let (px, py) = (palm.0 as f32 + a.cos() * r, palm.1 as f32 + a.sin() * r * 0.8);
            pen.light(px as i32, py as i32, if k > 0.6 { 'w' } else { '6' });
        }
    }
    if w.st == Ws::Stagger {
        feel::stars(im, x, y - 30, v.tick, 5, 12);
    }
    if w.st == Ws::Dying {
        let age = u32::from(w.dead);
        if age < 60 {
            juice::death_burst(im, (x as f32, (y - 4) as f32), age, 3, tick);
        }
        if age > 18 && age < 70 {
            juice::death_burst(im, ((x - 12) as f32, (y + 6) as f32), age - 18, 11, tick);
        }
    } else if let Some(age) = feel::hit_age(w.flinch) {
        juice::impact_burst(im, (x as f32, (y - 6) as f32), age, hash(w.hp as i32, 3, 4), weight, tick);
    }
}

/// How far through its tell the Warden is, 0 to 1.
fn tell_progress(v: &View, w: &lw::Warden) -> f32 {
    let tell = lw::Rules::of(&v.run.tune).tell_for(v.s.seats.len() as u32).max(1);
    1.0 - (f32::from(w.t) / tell as f32).min(1.0)
}

/// The caged lamp over the Warden's belt: dark glass while it is shut, the
/// burning lamp when light has found it, flickering and ringed.
fn heart(im: &mut Img, v: &View, w: &lw::Warden) {
    let (hx, hy) = (v.x(w.heart().0), v.y(w.heart().1));
    let open = v.s.boss_open();
    let frame = if open {
        bank::frame_at("warden", "heart-open", (v.tick / 6 % 2) as usize)
    } else {
        bank::frame("warden", "heart-shut", v.tick)
    };
    // The frame is the body's size with its lamp at (24, 37).
    im.stamp(frame, hx - 24, hy - 37);
    if open {
        let pulse = [0, 1, 2, 1][(v.tick / 3 % 4) as usize];
        super::scene::ring_px(im, hx, hy, 11 + pulse, '4', v.tick);
        super::scene::ring_px(im, hx, hy, 15 + pulse, '5', v.tick + 1);
        // Embers lifting off it.
        for k in 0..4 {
            let up = ((v.tick as i32 * 2 + k * 9) % 22) as i32;
            let dx = [-5, 3, 6, -2][k as usize];
            im.put(hx + dx + (up / 7) * if k % 2 == 0 { 1 } else { -1 }, hy - 8 - up, if up < 10 { '6' } else { '4' });
        }
    } else {
        // Light gathering on the shutter: a lit pip for each seat's light that is on it.
        let lit = w.lit.iter().filter(|&&t| t > 0 && t + lw::EXPOSE_TICKS > v.tick).count();
        for a in 0..lit {
            im.put(hx - 3 + a as i32 * 3, hy - 9, '6');
            im.put(hx - 3 + a as i32 * 3, hy - 10, 'w');
        }
    }
}

// ----- effects --------------------------------------------------------------------

fn effects(im: &mut Img, v: &View) {
    feel::effects(im, v);
    feel::kindle(im, v);
    feel::lock_on(im, v);
}

// ----- light ---------------------------------------------------------------------

/// A hooded snuffer in the dark shows only its eyes.
fn eyes(im: &mut Img, v: &View) {
    for f in &v.s.foes {
        if !f.alive() || f.kind != Fk::Snuffer {
            continue;
        }
        let (x, y) = (v.x(f.x), v.y(f.y));
        if v.s.light_at(f.x, f.y) == 0 && v.on_screen(x, y, 8) {
            let blink = (v.tick / 40).is_multiple_of(7);
            if !blink {
                im.put(x - 3, y - 2, '3');
                im.put(x + 3, y - 2, '3');
            }
        }
    }
}

fn flash(im: &mut Img, n: u32) {
    // A parry's flash: a thin cool veil of dots over the room that thins as it fades.
    let dense = (n.min(8) as f32) / 110.0;
    for y in 0..224i32 {
        for x in 0..384i32 {
            if bayer(x, y) < dense {
                im.put(x, y, if bayer(x + 2, y + 1) < 0.25 { 'V' } else { 'U' });
            }
        }
    }
}

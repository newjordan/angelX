//! The room, from above: tiles, things, bodies, light and the footprints a
//! tell shows. Art never sets a hitbox; every block is drawn where the rules
//! say the thing is.
use super::super::super::ink::{Img, bayer, hash};
use super::super::knights;
use super::bank::{painted, slot, slot_mid};
use super::hud;
use crate::rules::{
    Run,
    lanternward::{
        self as lw, Act, Atk, Bk, Danger, Dk, Fk, Fs, Fxk, Hold, Pk, Seat, Session, Ws,
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
    // A shake is a few pixels either way, fixed by the tick.
    let shake = if s.shake > 0 {
        let h = hash(s.elapsed as i32, 7, 3);
        let m = (s.shake as i32).min(4);
        ((h & 7) as i32 % (2 * m + 1) - m, ((h >> 3) & 7) as i32 % (2 * m + 1) - m)
    } else {
        (0, 0)
    };
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
    if !dark {
        footprints(&mut im, &v);
    }
    bodies(&mut im, &v);
    if dark {
        gloam(&mut im, &v);
        // A blow's footprint is read in the dark too, and so is a hood's eyes.
        footprints(&mut im, &v);
        eyes(&mut im, &v);
    }
    effects(&mut im, &v);
    if s.flash > 0 {
        flash(&mut im, s.flash);
    }
    hud::draw(&mut im, &v);
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
            let down = p.on == 1;
            im.rect(x - 6, y - 6, 12, 12, if down { 'g' } else { 'S' });
            im.frame(x - 6, y - 6, 12, 12, if down { 'k' } else { 'O' });
            if !down {
                im.rect(x - 2, y - 2, 4, 4, 'O');
            }
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
            if p.on == 0 {
                slot_mid(im, "cracked", "intact", 0, x, y);
            } else {
                im.rect(x - 7, y + 2, 14, 5, 'G');
                im.rect(x - 4, y, 6, 3, 'J');
            }
        }
        Pk::Sigil => {
            if p.on == 1 {
                sigil(im, x, y, '3', t);
            } else if p.t > 0 {
                let r = 2 + i32::from(p.t) * 6 / i32::from(lw::SIGIL_TICKS);
                ring_px(im, x, y, r, '2', t);
            }
        }
        Pk::Lock | Pk::Boss => {
            if p.on == 0 {
                let id = if p.kind == Pk::Lock { "lock" } else { "boss-door" };
                slot_mid(im, id, if p.kind == Pk::Lock { "locked" } else { "chained" }, t, x, y);
            } else {
                im.rect(x - 8, y - 8, 2, 16, 'j');
                im.rect(x + 6, y - 8, 2, 16, 'j');
            }
        }
        Pk::Fissure => {
            if p.t > 0 {
                im.ellipse(x as f32, y as f32, 6.0, 4.0, 'k');
                im.ellipse(x as f32, y as f32, 3.0, 2.0, '7');
            } else {
                im.line(x - 5, y, x + 5, y + 1, 'K');
                im.line(x - 2, y - 3, x + 3, y + 3, 'K');
            }
        }
    }
}

/// The point `r` pixels from `(x, y)` at whole degrees `deg`, its up and down
/// squashed to `squash` per cent (a ring on the floor seen from above).
fn polar(x: i32, y: i32, deg: i32, r: i32, squash: i32) -> (i32, i32) {
    (x + cos_q(deg) * r / Q, y + sin_q(deg) * r * squash / (100 * Q))
}

fn ring_px(im: &mut Img, x: i32, y: i32, r: i32, ink: char, tick: u32) {
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
    match b.kind {
        Bk::Iron => {
            im.rect(x - 2, y - 2, 5, 5, 'k');
            im.rect(x - 1, y - 1, 3, 3, 'O');
            im.put(x, y, 'w');
        }
        Bk::Light => {
            ring_px(im, x, y, 4, '4', v.tick);
            im.rect(x - 2, y - 2, 5, 5, '5');
            im.rect(x - 1, y - 1, 3, 3, 'w');
        }
        Bk::Bright => {
            ring_px(im, x, y, 6, 'w', v.tick);
            im.rect(x - 3, y - 3, 7, 7, 'w');
            im.rect(x - 1, y - 1, 3, 3, '6');
        }
        Bk::Dim => {
            im.rect(x - 2, y - 2, 4, 4, 'h');
            im.put(x, y, 'H');
        }
        Bk::Pot => slot(im, "pot", "whole", 0, x, y + 6),
    }
    // A short tail so the flight reads.
    let (tx, ty) = (-(b.vx / Q).clamp(-4, 4), -(b.vy / Q).clamp(-4, 4));
    im.put(x + tx * 2, y + ty * 2, if matches!(b.kind, Bk::Iron | Bk::Dim) { 'h' } else { '4' });
}

// ----- knights ------------------------------------------------------------------

fn knight_img(v: &View, id: u32, k: &Seat) -> Img {
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
    let (fw, fh) = (fig.w, fig.h);
    let foot = y + 4 - lift;
    im.stamp(&fig, x - fw / 2, foot - fh);
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
    swing_arc(im, v, k, x, foot - 8);
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

/// The sword's arc, the spin's ring and the lunge's point.
fn swing_arc(im: &mut Img, v: &View, k: &Seat, x: i32, y: i32) {
    let face = k.face;
    let base = angle_of(face.0, face.1);
    match k.act {
        Act::Slash => {
            let n = usize::from(k.n.min(2));
            let (wind, active) = ([2, 2, 4][n], [3, 3, 4][n]);
            if k.t as i32 > wind - 1 && k.t as i32 <= wind + active + 2 {
                let reach = [18, 18, 22][n];
                let thick = if n == 2 { 6 } else { 4 };
                // The blade sweeps from one side of the facing to the other;
                // the slash after it comes back.
                let sweep = if n == 1 { -1 } else { 1 };
                let span = if n == 2 { 149 } else { 109 };
                // Progress and distance behind it, in hundredths.
                let progress = ((k.t as i32 - wind).max(0) * 100 / active).min(120);
                let steps = 26;
                for st in 0..steps {
                    let f = st * 100 / steps;
                    // The bright head is where the blade is; a tail trails it.
                    let behind = progress - f;
                    if !(0..=55).contains(&behind) {
                        continue;
                    }
                    let a = base + sweep * (f - 50) * span / 100;
                    let t = (thick * (100 - behind * 14 / 10) / 100).max(thick / 4);
                    let (ix, iy) = polar(x, y, a, reach - t, 90);
                    let (ox, oy) = polar(x, y, a, reach, 90);
                    let ink = if behind < 12 { 'w' } else if behind < 30 { 'W' } else { 'V' };
                    im.line(ix, iy, ox, oy, ink);
                }
            }
        }
        Act::Spin => {
            let r = (6 + k.t as i32 * 2).min(22);
            for a in 0..28 {
                let (px, py) = polar(x, y, a * 360 / 28 + k.t as i32 * 52, r, 90);
                im.put(px, py, if a % 2 == 0 { 'w' } else { 'V' });
            }
        }
        Act::Lunge
            if (lw::LUNGE_WIND..lw::LUNGE_WIND + lw::LUNGE_ACTIVE).contains(&k.t) => {
                let len = 24;
                im.line(x, y, x + face.0 * len / Q, y + face.1 * len / Q, 'W');
                im.put(x + face.0 * (len + 2) / Q, y + face.1 * (len + 2) / Q, 'w');
            }
        Act::Plunge => {
            im.line(x, y - 14, x, y + 2, 'W');
        }
        _ => {}
    }
    let _ = v;
}

// ----- foes -----------------------------------------------------------------------

fn foe(im: &mut Img, v: &View, i: usize) {
    let f = &v.s.foes[i];
    let (x, y) = (v.x(f.x), v.y(f.y));
    if !v.on_screen(x, y, 40) {
        return;
    }
    match f.kind {
        Fk::Snuffer => {
            let lit = v.s.light_at(f.x, f.y) > 0 || f.st == Fs::Dying;
            if v.s.room == 3 && !lit && !matches!(f.st, Fs::Tell | Fs::Strike) {
                return;
            }
            let hover = if f.st == Fs::Dying { 0 } else { [0, -1, -2, -1][(v.tick / 5 % 4) as usize] };
            shadow(im, x, y + 9, 5);
            let (state, tk) = match f.st {
                Fs::Dying => ("die", u32::from(f.dead) / 2),
                _ if f.hooded && f.lit == 0 => ("hooded", v.tick),
                Fs::Dormant => ("idle", v.tick),
                Fs::Drift => ("drift", v.tick),
                Fs::Tell => ("tell", u32::from(lw::SNUFFER_TELL.saturating_sub(f.t)) * 24 / u32::from(lw::SNUFFER_TELL)),
                Fs::Strike => ("lunge", v.tick),
                Fs::Recover => ("recover", v.tick),
                Fs::Stagger => ("stagger", v.tick),
            };
            let jitter = if f.flinch > 0 { (f.flinch as i32 % 2) * 2 - 1 } else { 0 };
            slot(im, "snuffer", state, tk, x + jitter, y + 10 + hover);
            // A white flash on the tick it is struck.
            if f.flinch >= lw::FLINCH_TICKS - 1 {
                im.ellipse(x as f32, (y + hover) as f32, 8.0, 9.0, 'w');
            }
            // The eyes look where it will go.
            let (ex, ey) = (f.fx * 2 / Q, f.fy * 2 / Q);
            if f.st != Fs::Dying {
                im.put(x - 3 + ex, y - 2 + hover + ey, 'w');
                im.put(x + 3 + ex, y - 2 + hover + ey, 'w');
            }
            if f.st == Fs::Stagger {
                for a in 0..3 {
                    im.put(x - 5 + a * 5, y - 14 + (v.tick as i32 / 2 + a) % 2, '6');
                }
            }
            if f.alive() && f.hp < f.full {
                bar(im, x, y - 14, 12, f.hp, f.full, 'O');
            }
            if f.hooded && f.lit == 0 && f.alive() {
                im.text(x - 8, y + 12, "HOOD", 'u');
            }
        }
        Fk::Turret => {
            shadow(im, x, y + 11, 7);
            let (state, tk) = match f.st {
                Fs::Tell => ("tell", u32::from(f.tell_total(v)).saturating_sub(u32::from(f.t)) / 2),
                Fs::Strike => ("fire", v.tick),
                Fs::Recover => ("recover", v.tick),
                Fs::Stagger => ("stun", v.tick),
                _ => ("idle", v.tick),
            };
            slot(im, "turret", state, tk, x, y + 12);
            // The barrel points the way it will fire.
            let (bx, by) = (x + f.fx * 11 / Q, y + f.fy * 11 / Q);
            im.line(x, y, bx, by, 'k');
            if matches!(f.st, Fs::Tell | Fs::Strike) {
                im.put(bx, by, 'w');
            }
            if f.st == Fs::Stagger {
                for a in 0..3 {
                    im.put(x - 5 + a * 5, y - 18 + (v.tick as i32 / 2 + a) % 2, '6');
                }
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

fn boss(im: &mut Img, v: &View) {
    let Some(w) = &v.s.boss else { return };
    let (x, y) = (v.x(w.x), v.y(w.y));
    if !v.on_screen(x, y, 60) {
        return;
    }
    shadow(im, x, y + 24, 18);
    let (state, tk) = match w.st {
        Ws::Asleep => ("sleep", v.tick),
        Ws::Rest => ("walk", v.tick),
        Ws::Tell => (
            if w.atk == Atk::Beam { "beam-tell" } else if w.atk == Atk::Slam { "slam-tell" } else { "summon" },
            v.tick,
        ),
        Ws::Strike => (if w.atk == Atk::Beam { "beam" } else if w.atk == Atk::Slam { "slam" } else { "summon" }, v.tick),
        Ws::Recover => ("idle", v.tick),
        Ws::Stagger => ("stagger", v.tick),
        Ws::Dying => ("die", u32::from(w.dead) / 4),
    };
    let jitter = if w.flinch > 0 { (w.flinch as i32 % 2) * 2 - 1 } else { 0 };
    slot(im, "warden", state, tk, x + jitter, y + 28);
    // The lamp-heart in its chest: shut, or open and burning.
    if w.st != Ws::Dying {
        let (hx, hy) = (v.x(w.heart().0), v.y(w.heart().1));
        let open = v.s.boss_open();
        im.ellipse(hx as f32, hy as f32, 7.0, 7.0, 'k');
        if open {
            im.ellipse(hx as f32, hy as f32, 5.0, 5.0, '6');
            im.ellipse(hx as f32, hy as f32, 2.0, 2.0, 'w');
            ring_px(im, hx, hy, 10 + (v.tick as i32 / 3 % 3), '4', v.tick);
        } else {
            im.ellipse(hx as f32, hy as f32, 5.0, 5.0, 'X');
            im.rect(hx - 5, hy - 1, 10, 3, 'g');
            // Light gathering on the shutter.
            let lit = w.lit.iter().filter(|&&t| t > 0 && t + lw::EXPOSE_TICKS > v.tick).count();
            for a in 0..lit {
                im.put(hx - 3 + a as i32 * 3, hy - 4, '6');
            }
        }
    }
    if w.st == Ws::Stagger {
        for a in 0..5 {
            im.put(x - 10 + a * 5, y - 40 + (v.tick as i32 / 2 + a) % 2, '6');
        }
    }
}

// ----- effects --------------------------------------------------------------------

fn effects(im: &mut Img, v: &View) {
    // The wick-spark over every locked target, in the locking knight's own
    // view only (a friend's lock is a small tick, not a banner).
    for (&id, k) in &v.s.seats {
        if let Some(l) = k.lock
            && let Some((tx, ty)) = v.s.lock_point(l)
        {
            let (x, y) = (v.x(tx), v.y(ty));
            let bob = [0, -1, -2, -1][(v.tick / 3 % 4) as usize];
            let top = match l {
                lw::Lock::Boss => -12,
                lw::Lock::Foe(_) => -16,
                _ => -20,
            };
            let ink = if id == v.id { '5' } else { knights::seat_cloth(id) };
            if id == v.id {
                slot_mid(im, "mark", "idle", v.tick, x, y + top + bob);
                // Four brackets close on the target the tick it is taken.
                let r = 9 + (6u32.saturating_sub(k.lock_t.min(6))) as i32;
                for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    im.rect(x + dx * r - (dx > 0) as i32, y + dy * r - (dy > 0) as i32, 2, 2, ink);
                }
            } else {
                im.put(x, y + top + bob, ink);
            }
        }
    }
    for fx in &v.s.fx {
        let (x, y) = (v.x(fx.x), v.y(fx.y));
        if !v.on_screen(x, y, 40) {
            continue;
        }
        let t = i32::from(fx.t);
        match fx.kind {
            Fxk::Puff => {
                let r = 3 + t / 2;
                for a in 0..8 {
                    let rr = r + a % 3;
                    let ink = if t < 8 { 'h' } else { 'J' };
                    let (px, py) = polar(x, y, a * 45, rr, 80);
                    im.rect(px - 1, py - 3 - t / 3, 3, 3, ink);
                }
            }
            Fxk::Light => {
                for a in 0..5 {
                    let dx = (a - 2) * 3;
                    im.put(x + dx, y - 4 - t - (a * 2) % 5, if (t + a) % 2 == 0 { '6' } else { 'w' });
                }
            }
            Fxk::Spark => {
                let r = 2 + t;
                for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1), (0, -2), (2, 0)] {
                    im.put(x + dx * r, y - 4 + dy * r, if t < 3 { 'w' } else { '6' });
                }
            }
            Fxk::Ring => {
                ring_px(im, x, y, 3 + t * 2 + i32::from(fx.a) / 4, 'V', 0);
            }
            Fxk::Parry => {
                let r = 4 + t * 2;
                ring_px(im, x, y, r, 'w', 0);
                ring_px(im, x, y, r / 2, 'w', 1);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    im.line(x + dx * 3, y + dy * 3, x + dx * (6 + t), y + dy * (6 + t), 'w');
                }
            }
            Fxk::Tick => {
                ring_px(im, x, y - 6, 4 + t, '6', 0);
            }
            Fxk::Dust => {
                for a in 0..5 {
                    im.put(x - 6 + a * 3, y + 2 - t / 2 + (a % 2), 'J');
                }
            }
            Fxk::Solved => {
                let big = fx.a > 0;
                let r = 4 + t * if big { 3 } else { 2 };
                for a in 0..8 {
                    let (ix, iy) = polar(x, y, a * 45, r - 3, 100);
                    let (ox, oy) = polar(x, y, a * 45, r, 100);
                    im.line(ix, iy, ox, oy, if a % 2 == 0 { '6' } else { 'w' });
                }
            }
            Fxk::Blast => {
                // A flash, a ring of fire running out and a pall of smoke.
                let r = 4 + t * 3;
                if t < 3 {
                    im.ellipse(x as f32, y as f32, (r + 4) as f32, (r + 4) as f32 * 0.8, 'w');
                }
                for (grow, ink) in [(0, if t < 6 { '6' } else { 'O' }), (2, if t < 8 { '7' } else { 'g' })] {
                    for a in 0..28 {
                        let (px, py) = polar(x, y, a * 360 / 28, (r - grow).max(2), 80);
                        im.rect(px, py, 2, 2, ink);
                    }
                }
                if t > 4 {
                    for a in 0..6 {
                        let (px, py) = polar(x, y, a * 60 + 20, 4 + t, 70);
                        im.rect(px - 1, py - 2 - t / 2, 3, 3, 'h');
                    }
                }
            }
            Fxk::Wave => {
                ring_px(im, x, y, 6 + t * 5, 'O', 0);
            }
        }
    }
}

// ----- light -----------------------------------------------------------------------

/// The Gloam: only light shows the floor. Dithered, so the edge of a lantern's
/// reach is a soft stipple, not a ring.
fn gloam(im: &mut Img, v: &View) {
    let lights = v.s.lights();
    for cy in 0..112 {
        for cx in 0..192 {
            let (px, py) = (cx * 2, cy * 2);
            let (qx, qy) = ((px + v.ox) * Q + Q, (py + v.oy) * Q + Q);
            let level = lw::props::light_in(&lights, qx, qy);
            let dark = match level {
                3 => continue,
                2 => 0.12,
                1 => 0.5,
                _ => 1.0,
            };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                if bayer(px + dx, py + dy) < dark {
                    im.put(px + dx, py + dy, 'k');
                }
            }
        }
    }
}

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
    // A parry's white: a stipple that thins as the flash fades.
    let dense = (n.min(8) as f32) / 48.0;
    for y in 0..224 {
        for x in 0..384 {
            if bayer(x, y) < dense {
                im.put(x, y, 'w');
            }
        }
    }
}

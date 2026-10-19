//! Griffinwing's picture over the 3D scene: the free reticle (near and far)
//! with its charge ring, the lock brackets, the locked danger footprints,
//! bolts with their trails, the flashes, the shield and the spur, the wing's
//! portraits calling out, the beat's title, the Wyrm's lamps and the tally.
//! All of it is laid on the one native picture (the terminal's cells sample
//! the same one), in the realm's own inks.
use super::cam::Cam;
use super::layout::{self, Box2, H, Rank, W, Want, plate_of};
use crate::render::overworld::arena::knights as kn;
use crate::render::overworld::chrome::{self, Tone};
use crate::render::overworld::ink::{Img, text_width};
use crate::render::world3d::math::v3;
use crate::render::world3d::raster;
use crate::rules::griffinwing::trig::Det;
use crate::rules::griffinwing::{
    self as gw, Arm, Burst, Craft, Kind, Medal, Mode, Q, Session, St, Wyrm, boss, rail, rail::units,
};
use crate::rules::{HZ, Run};

/// A blow in the last few ticks: the picture's edge flashes.
const FLASH: u32 = 28;

/// What the overlay needs to place a thing in the picture.
pub struct Ctx<'a> {
    pub s: &'a Session,
    pub run: &'a Run,
    pub seat: u32,
    pub cam: &'a Cam,
    pub depth: &'a [f32],
}

impl Ctx<'_> {
    pub fn craft(&self) -> Option<&Craft> {
        self.s.seats.get(&self.seat).map(|x| &x.craft)
    }
    /// A rail point (fixed point) on the picture: where, and how far.
    pub fn at(&self, s: i32, x: i32, z: i32) -> Option<(f32, f32, f32)> {
        let w = rail::rail().world(units(s), units(x), units(z));
        let p = self.cam.point(v3(w[0], w[1], w[2]));
        raster::project_point(&self.cam.view(), W as usize, H as usize, p)
    }
    /// How many dots tall a thing `h` units high is at `depth`.
    pub fn tall(&self, h: f32, depth: f32) -> f32 {
        raster::dots_tall(&self.cam.view(), W as usize, H as usize, h, depth)
    }
    /// Whether the scene leaves this dot showing in front of `depth`.
    pub fn clear(&self, x: i32, y: i32, depth: f32) -> bool {
        if !(0..W).contains(&x) || !(0..H).contains(&y) {
            return false;
        }
        self.depth
            .get((y * W + x) as usize)
            .is_none_or(|&d| d > depth - 0.25)
    }
}

// ─── little shapes ──────────────────────────────────────────────────────────

fn disc(im: &mut Img, cx: i32, cy: i32, r: i32, ch: char) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r + r / 2 {
                im.put(cx + dx, cy + dy, ch);
            }
        }
    }
}

fn ring(im: &mut Img, cx: i32, cy: i32, r: i32, ch: char) {
    let n = (r * 6).clamp(12, 72);
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        im.put(
            cx + (a.dcos() * r as f32).round() as i32,
            cy + (a.dsin() * r as f32).round() as i32,
            ch,
        );
    }
}

/// Four corner brackets round a box, two dots thick so they survive the
/// terminal's coarser cells.
fn brackets(im: &mut Img, cx: i32, cy: i32, half: i32, arm: i32, ch: char) {
    for (sx, sy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        let (x, y) = (cx + sx * half, cy + sy * half);
        for d in 0..2 {
            im.line(x, y - sy * d, x - sx * arm, y - sy * d, ch);
            im.line(x - sx * d, y, x - sx * d, y - sy * arm, ch);
        }
    }
}

/// Words on a dark plate, so they read on any ground.
fn label(im: &mut Img, x: i32, y: i32, s: &str, ink: char) {
    let p = plate_of(x, y, s);
    im.rect(p.x, p.y, p.w, p.h, 'k');
    im.text(x, y, s, ink);
}

fn centred(im: &mut Img, cx: i32, y: i32, s: &str, ink: char) {
    label(im, cx - text_width(s) / 2, y, s, ink);
}

/// The plate `big` lays under words `k` times the font's size.
fn big_box(cx: i32, y: i32, s: &str, k: i32) -> Box2 {
    let w = text_width(s) * k;
    Box2::new(cx - w / 2 - 3, y - 3, w + 6, 7 * k + 6)
}

/// The plate `centred` lays under `s`.
fn centred_box(cx: i32, y: i32, s: &str) -> Box2 {
    plate_of(cx - text_width(s) / 2, y, s)
}

/// Words `k` times the font's size, on a dark plate, centred on `cx`.
fn big(im: &mut Img, cx: i32, y: i32, s: &str, ink: char, k: i32) {
    let mut words = Img::new(text_width(s), 7);
    words.text(0, 0, s, ink);
    let w = words.w * k;
    let p = big_box(cx, y, s, k);
    im.rect(p.x, p.y, p.w, p.h, 'k');
    im.stamp_times(&words, cx - w / 2, y, k);
}

fn plate(im: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    chrome::panel(im, (x, y, w, h), Tone::SLATE);
}

// ─── who is who ─────────────────────────────────────────────────────────────

/// A rider's name and the ink it is marked in; a knight's own.
pub fn who(run: &Run, id: u8) -> (String, char) {
    match (id, gw::rider_name(id)) {
        (7, Some(name)) => (name.into(), '4'),
        (8, Some(name)) => (name.into(), '3'),
        (9, Some(name)) => (name.into(), 'A'),
        (10, Some(name)) => (name.into(), 'z'),
        _ => {
            let name = run
                .players
                .get(&u32::from(id))
                .map_or_else(|| format!("KNIGHT {id}"), |h| h.name.to_uppercase());
            (
                name.chars().take(10).collect(),
                kn::seat_cloth(u32::from(id)),
            )
        }
    }
}

/// A portrait, 20 by 20: a knight's own face, or a rider's block with its
/// letter (a labelled placeholder until the art wave paints them).
fn portrait(im: &mut Img, run: &Run, id: u8, x: i32, y: i32) {
    let (name, ink) = who(run, id);
    im.rect(x, y, 20, 20, 'k');
    if let Some(hero) = run.players.get(&u32::from(id)).filter(|_| id <= 6) {
        let face = kn::portrait(hero, u32::from(id));
        let (fx, fy) = (x + (20 - face.w) / 2, y + 20 - face.h);
        im.stamp(&face, fx, fy.max(y - 2));
    } else {
        im.rect(x + 2, y + 2, 16, 16, 'x');
        im.rect(x + 6, y + 3, 8, 5, 'V');
        im.rect(x + 4, y + 9, 12, 8, ink);
        im.text(
            x + 7,
            y + 10,
            &name.chars().take(1).collect::<String>(),
            'k',
        );
    }
    im.frame(x, y, 20, 20, ink);
}

// ─── in the world ───────────────────────────────────────────────────────────

/// Speed streaks off the edges of the picture, while the spur is on.
fn streaks(im: &mut Img, tick: u32) {
    let (cx, cy) = (W / 2, H / 2 - 6);
    for k in 0..30u32 {
        let a = (k as f32 * 2.399 + (tick / 2 % 5) as f32 * 0.31) % std::f32::consts::TAU;
        let (dx, dy) = (a.dcos(), a.dsin());
        let r0 = 70.0 + ((k * 37 + tick * 9) % 90) as f32;
        let r1 = r0 + 18.0 + (k % 5) as f32 * 4.0;
        im.line(
            cx + (dx * r0 * 1.5) as i32,
            cy + (dy * r0) as i32,
            cx + (dx * r1 * 1.5) as i32,
            cy + (dy * r1) as i32,
            if k % 3 == 0 { 'W' } else { 'v' },
        );
    }
}

fn bolts(im: &mut Img, c: &Ctx) {
    let s = c.s;
    for b in &s.bolts {
        if b.delay > 0 {
            continue;
        }
        let Some((x, y, d)) = c.at(b.s, b.x, b.z) else {
            continue;
        };
        let (x, y) = (x.round() as i32, y.round() as i32);
        if !c.clear(x, y, d) {
            continue;
        }
        let tail = c.at(b.s - b.vs * 2, b.x - b.vx * 2, b.z - b.vz * 2);
        let (core, halo, trail, r) = match b.shot {
            Arm::Lance => ('6', '4', '5', 1),
            Arm::Star => ('w', '6', '5', 2),
            Arm::Ally => ('3', '2', 'z', 1),
            Arm::Spark => ('w', 'z', 'Q', 1),
            Arm::Gloam => ('w', '3', '2', (c.tall(1.3, d) / 2.0) as i32),
            Arm::Turned => ('9', '6', '5', (c.tall(1.5, d) / 2.0) as i32),
        };
        let r = r.clamp(1, 9);
        if let Some((tx, ty, _)) = tail {
            im.line(x, y, tx.round() as i32, ty.round() as i32, trail);
        }
        if r > 1 {
            disc(im, x, y, r, halo);
            disc(im, x, y, (r - 1).max(1), core);
            if matches!(b.shot, Arm::Gloam) {
                // A cold flame in a dark heart.
                disc(im, x, y, (r / 2).max(1), 'q');
                ring(im, x, y, r + 1, 'z');
            }
        } else {
            im.put(x, y, core);
            im.put(x + 1, y, halo);
            im.put(x - 1, y, halo);
            im.put(x, y + 1, halo);
            im.put(x, y - 1, halo);
        }
    }
}

/// A pale pip on every foe far off, where its block is a few dots: the
/// snuffers' own eyes, so a flight can be seen coming.
fn pips(im: &mut Img, c: &Ctx) {
    for f in &c.s.foes {
        if !f.lockable() || f.kind == Kind::Lamp {
            continue;
        }
        let Some((x, y, d)) = c.at(f.s, f.x, f.z) else {
            continue;
        };
        if d < 14.0 {
            continue;
        }
        let (x, y) = (x.round() as i32, y.round() as i32);
        if !c.clear(x, y, d) {
            continue;
        }
        let ink = match f.kind {
            Kind::Turret => '5',
            Kind::Bomber => '7',
            _ => 'W',
        };
        im.rect(x - 1, y - 1, 2, 2, ink);
        im.put(x - 2, y, 'v');
        im.put(x + 1, y, 'v');
    }
}

// ─── the floating labels ────────────────────────────────────────────────────

/// How old a flare is, in ticks since it began.
fn age_of(f: &gw::Flare) -> i32 {
    match f.what {
        Burst::Starfall => 36 - i32::from(f.ticks),
        Burst::Fall => 40 - i32::from(f.ticks),
        _ => 14 - i32::from(f.ticks),
    }
}

/// A foe that wears a name: in range, lockable, not telling, not locked.
struct Named<'a> {
    name: &'static str,
    ink: char,
    foe: &'a gw::Foe,
    x: i32,
    y: i32,
    half: i32,
    ahead: i32,
}

/// At most five, the nearest first.
fn named<'a>(c: &Ctx<'a>) -> Vec<Named<'a>> {
    let me = c.craft().map_or(0, |cr| c.s.craft_s(cr));
    let mut out = Vec::new();
    for f in &c.s.foes {
        let ahead = f.s - me;
        if f.st == St::Tell || !(gw::dm(80)..=gw::dm(380)).contains(&ahead) || !f.lockable() {
            continue;
        }
        // A locked foe has its bracket and its number; no name over it.
        if c.craft()
            .is_some_and(|me| me.locks.iter().any(|l| l.foe == f.id))
        {
            continue;
        }
        let Some((x, y, d)) = c.at(f.s, f.x, f.z) else {
            continue;
        };
        let name = match (f.kind, f.st) {
            (Kind::Kite, St::Latched) => "LATCHED",
            (Kind::Kite, _) => "KITE",
            (Kind::Wisp, _) => "WISP",
            (Kind::Turret, _) => "TURRET",
            (Kind::Bomber, _) => "BOMBER",
            (Kind::Lamp, St::Open) => "LAMP OPEN",
            (Kind::Lamp, St::Broken) => "",
            (Kind::Lamp, _) => "LAMP",
        };
        if name.is_empty() {
            continue;
        }
        let half = (c.tall(f.radius() as f32 / Q as f32 * 2.0, d) / 2.0).max(4.0) as i32;
        out.push(Named {
            name,
            ink: if f.st == St::Latched { '7' } else { 'h' },
            foe: f,
            x: x.round() as i32,
            y: y.round() as i32,
            half,
            ahead,
        });
    }
    out.sort_by_key(|n| n.ahead);
    out.truncate(5);
    out
}

/// The health bar under a hurt foe that wears a name.
fn bar_of(n: &Named) -> Option<Box2> {
    let f = n.foe;
    if f.hp >= f.full || f.hp == 0 {
        return None;
    }
    let w = (n.half * 2).clamp(10, 40);
    Some(Box2::new(n.x - w / 2, n.y + n.half + 3, w, 2))
}

/// The labels the picture wants: the wing's names over the other craft, the
/// Wyrm's, the foes', the things on the rail and the words of the moment (a
/// bolt turned, the gap, the roar). `layout::plan` finds each of them a place.
pub(super) fn wants(c: &Ctx) -> Vec<Want> {
    let s = c.s;
    let me = c.craft().map_or(0, |cr| s.craft_s(cr));
    let mut out: Vec<Want> = Vec::new();
    let mut want = |rank: Rank, text: &str, ink: char, x: i32, y: i32, near: i32, reach: i32| {
        out.push(Want {
            rank,
            text: text.to_string(),
            ink,
            x,
            y,
            near,
            reach,
        });
    };
    // The other craft: a rider's name, another knight's, over its head.
    let craft = |at: Option<(f32, f32, f32)>| -> Option<(i32, i32, f32)> {
        let (x, y, d) = at?;
        let (x, y) = (x.round() as i32, y.round() as i32);
        let on = (0..W).contains(&x) && (0..H).contains(&y);
        (d >= 4.0 && on).then_some((x, y, d))
    };
    let lift = |d: f32| ((c.tall(1.6, d) / 2.0) as i32 + 8).clamp(12, 36);
    for (&id, seat) in &s.seats {
        if id != c.seat && seat.craft.down == 0 {
            let at = c.at(s.craft_s(&seat.craft), seat.craft.x, seat.craft.z);
            if let Some((x, y, d)) = craft(at) {
                let (name, ink) = who(c.run, id as u8);
                want(
                    Rank::Craft,
                    &name,
                    ink,
                    x,
                    y - lift(d),
                    (d * 10.0) as i32,
                    44,
                );
            }
        }
    }
    for w in s.wing.iter().filter(|w| !w.lost) {
        let at = c.at(s.craft_s(&w.craft), w.craft.x, w.craft.z);
        if let Some((x, y, d)) = craft(at) {
            let (name, ink) = who(c.run, w.id);
            want(
                Rank::Craft,
                &name,
                ink,
                x,
                y - lift(d),
                (d * 10.0) as i32,
                44,
            );
        }
    }
    let over = matches!(s.mode, Mode::Victory | Mode::Tally);
    if let Some(b) = &s.boss {
        if !over {
            if let Some((x, y, _)) = craft(c.at(s.anchor() + boss::HEAD, b.x, b.z + gw::dm(70))) {
                want(Rank::Craft, "GLOAM WYRM", 'h', x, y, -1, 30);
            }
        }
    }
    for n in named(c) {
        want(
            Rank::Foe,
            n.name,
            n.ink,
            n.x,
            n.y - n.half - 10,
            n.ahead,
            30,
        );
    }
    let mut shown = 0;
    for p in &s.props {
        let ahead = p.s - me;
        if !(gw::dm(80)..=gw::dm(420)).contains(&ahead) || shown >= 3 {
            continue;
        }
        let (text, ink, up) = match p.thing {
            gw::Thing::Gate => ("GATE", '5', 4),
            gw::Thing::Ring => ("HEARTH", '5', 4),
            gw::Thing::Arch => ("ARCH", 'h', 0),
            gw::Thing::Ridge => ("RIDGE", 'h', 0),
            gw::Thing::Shutter => ("SHUTTER", '7', 0),
        };
        let pz = p.z
            + if up > 0 {
                gw::dm(28)
            } else {
                gw::dm(p.h / 25 + 10)
            };
        if let Some((x, y, _)) = craft(c.at(p.s, p.x, pz)) {
            want(Rank::Thing, text, ink, x, y, ahead, 30);
        }
        shown += 1;
    }
    if s.mode == Mode::Victory {
        if let Some((x, y, _)) = craft(c.at(s.home + gw::dm(1000), 0, gw::dm(160))) {
            want(Rank::Thing, "BEACON", '5', x, y, 0, 30);
        }
    }
    // The words of the moment: the Wyrm's gap and its roar, a bolt turned
    // round and a chain of pops. The newest few; a pile of them is no use.
    if let (Some(b), Some(me)) = (&s.boss, c.craft()) {
        if !over && matches!(b.st, Wyrm::Tell | Wyrm::Coil | Wyrm::Roar) {
            let here = s.craft_s(me) + gw::dm(10);
            let (text, ink, col) = match b.atk {
                1 => ("GAP", '5', b.gap),
                2 => ("ROAR", '7', b.col),
                _ => ("", ' ', 0),
            };
            if let Some((x, y, _)) = c.at(here, col, 0) {
                if !text.is_empty() {
                    want(Rank::Moment, text, ink, x as i32, y as i32 - 4, -2, 24);
                }
            }
        }
    }
    let mut turns = 0;
    let mut pops = 0;
    let mut fx: Vec<_> = s.fx.iter().collect();
    fx.sort_by_key(|f| age_of(f));
    for f in fx {
        let age = age_of(f);
        let Some((x, y, _)) = c.at(f.s, f.x, f.z) else {
            continue;
        };
        let (x, y) = (x.round() as i32, y.round() as i32);
        match f.what {
            Burst::Turn if age < 9 && turns < 2 => {
                turns += 1;
                want(Rank::Moment, "REFLECT", '6', x + 2, y - 12, age, 24);
            }
            Burst::Pop if f.n >= 2 && age < 10 && pops < 3 => {
                pops += 1;
                let text = format!("x{}", f.n);
                want(
                    Rank::Moment,
                    &text,
                    '5',
                    x + 4 + text_width(&text) / 2,
                    y - 10 - age,
                    age,
                    24,
                );
            }
            _ => {}
        }
    }
    out
}

/// The floating labels, each on its plate, where `layout::plan` put them,
/// and the health bars of the foes that are hurt.
fn words(im: &mut Img, c: &Ctx) {
    for n in named(c) {
        if let Some(b) = bar_of(&n) {
            let f = n.foe;
            im.rect(b.x, b.y, b.w, b.h, 'g');
            im.rect(
                b.x,
                b.y,
                (b.w * f.hp as i32 / f.full.max(1) as i32).max(1),
                2,
                '7',
            );
        }
    }
    for p in layout::plan(c).placed {
        im.rect(p.plate.x, p.plate.y, p.plate.w, p.plate.h, 'k');
        im.text(p.at.0, p.at.1, &p.text, p.ink);
    }
}

/// The locked footprints of what is about to land: a ring where each shot
/// will, a lane for a dive, the Wyrm's gap and its column. They pulse faster
/// as the tell runs out, and never show for less than forty ticks.
fn footprints(im: &mut Img, c: &Ctx) {
    let s = c.s;
    let Some(me) = c.craft() else { return };
    let here = s.craft_s(me);
    let tick = s.elapsed;
    let mark = |im: &mut Img, x: i32, z: i32, left: u32, ink: char| {
        let Some((px, py, d)) = c.at(here + gw::dm(10), x, z) else {
            return;
        };
        let (px, py) = (px.round() as i32, py.round() as i32);
        let r = (c.tall(2.6, d) / 2.0).clamp(6.0, 26.0) as i32;
        let beat = if left < 20 { 3 } else { 6 };
        let pulse = (tick / beat) % 2 == 0;
        ring(im, px, py, r, if pulse { ink } else { '8' });
        ring(im, px, py, r - 2, if pulse { '9' } else { ink });
        im.line(px - r / 2, py, px + r / 2, py, ink);
        im.line(px, py - r / 2, px, py + r / 2, ink);
        if left > 0 && r >= 8 {
            im.text(px - 2, py - r - 9, "!", '9');
        }
    };
    for f in &s.foes {
        if f.st != St::Tell {
            continue;
        }
        match f.kind {
            Kind::Turret => {
                for (x, z) in Session::marks(f, s.pressure()) {
                    mark(im, x, z, f.ticks, '7');
                }
            }
            Kind::Bomber | Kind::Wisp => {
                mark(im, f.fx, f.fz, f.ticks, '7');
                // The bomber's lane, from where it hangs to the mark.
                if f.kind == Kind::Bomber {
                    if let (Some(a), Some(b)) =
                        (c.at(f.s, f.x, f.z), c.at(here + gw::dm(10), f.fx, f.fz))
                    {
                        let (x0, y0, x1, y1) = (a.0 as i32, a.1 as i32, b.0 as i32, b.1 as i32);
                        for k in 0..12 {
                            if k % 2 == 0 {
                                let t0 = k as f32 / 12.0;
                                let t1 = (k + 1) as f32 / 12.0;
                                let lerp = |a: i32, b: i32, t: f32| a + ((b - a) as f32 * t) as i32;
                                im.line(
                                    lerp(x0, x1, t0),
                                    lerp(y0, y1, t0),
                                    lerp(x0, x1, t1),
                                    lerp(y0, y1, t1),
                                    '7',
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(b) = &s.boss {
        // Once it has fallen, its footprints go with it.
        let over = matches!(s.mode, Mode::Victory | Mode::Tally);
        if !over && matches!(b.st, Wyrm::Tell | Wyrm::Coil | Wyrm::Roar) {
            let wz = gw::Nums::default().wz();
            let wx = gw::Nums::default().wx();
            let hatch = |im: &mut Img, x0: i32, x1: i32, ink: char| {
                let (Some(a), Some(b2)) = (
                    c.at(here + gw::dm(10), x0, wz),
                    c.at(here + gw::dm(10), x1, -wz),
                ) else {
                    return;
                };
                let (xa, xb) = (a.0.min(b2.0) as i32, a.0.max(b2.0) as i32);
                let (ya, yb) = (a.1.min(b2.1) as i32, a.1.max(b2.1) as i32);
                for y in (ya..=yb).step_by(1) {
                    for x in xa..=xb {
                        if (x + y + (tick as i32 / 3)) % 6 == 0 && (x - y).rem_euclid(2) == 0 {
                            im.put(x, y, ink);
                        }
                    }
                }
            };
            match b.atk {
                0 if b.st == Wyrm::Tell => {
                    for &(x, z) in &b.marks {
                        mark(im, x, z, b.ticks, '7');
                    }
                }
                1 => {
                    hatch(im, -wx - gw::dm(20), b.gap - boss::GAP_HALF, '7');
                    hatch(im, b.gap + boss::GAP_HALF, wx + gw::dm(20), '7');
                    for edge in [b.gap - boss::GAP_HALF, b.gap + boss::GAP_HALF] {
                        if let (Some(a), Some(b2)) = (
                            c.at(here + gw::dm(10), edge, wz),
                            c.at(here + gw::dm(10), edge, -wz),
                        ) {
                            im.line(a.0 as i32, a.1 as i32, b2.0 as i32, b2.1 as i32, '5');
                        }
                    }
                }
                2 => {
                    hatch(im, b.col - boss::COL_HALF, b.col + boss::COL_HALF, '9');
                }
                _ => {}
            }
        }
    }
}

/// A locked foe's bracket: its centre, its half size, the locks it holds.
fn lock_marks(c: &Ctx, me: &Craft) -> Vec<(i32, i32, i32, u8)> {
    let mut out = Vec::new();
    for l in &me.locks {
        if let Some(f) = c.s.foes.iter().find(|f| f.id == l.foe) {
            let Some((x, y, d)) = c.at(f.s, f.x, f.z) else {
                continue;
            };
            let half =
                (c.tall(f.radius() as f32 / Q as f32 * 2.8, d) / 2.0).clamp(7.0, 26.0) as i32;
            out.push((x.round() as i32, y.round() as i32, half, l.n));
        }
    }
    out
}

/// Where the far and the near reticle fall, and the craft under them.
type Aim = (
    Option<(f32, f32, f32)>,
    Option<(f32, f32, f32)>,
    Option<(f32, f32, f32)>,
);

fn aim(c: &Ctx, me: &Craft) -> Aim {
    let here = c.s.craft_s(me);
    let at = |dist: i32| {
        let (rx, rz) = (
            (i64::from(me.rx) * i64::from(dist) / i64::from(gw::FAR)) as i32,
            (i64::from(me.rz) * i64::from(dist) / i64::from(gw::FAR)) as i32,
        );
        c.at(here + dist, me.x + rx, me.z + rz)
    };
    (
        at(gw::FAR),
        at(gw::NEAR),
        c.at(here + gw::dm(8), me.x, me.z),
    )
}

/// The "x3" count by the far reticle: on its right, or on its left when the
/// right is the frame's edge.
fn count_at(fx: i32, fy: i32, text: &str) -> (i32, i32) {
    let w = text_width(text);
    let x = if fx + 14 + w + 2 > W - 1 {
        fx - 14 - w
    } else {
        fx + 14
    };
    (x.max(2), (fy - 4).clamp(2, H - 12))
}

/// The "READY" under the far reticle, kept inside the frame.
fn ready_at(fx: i32, fy: i32) -> (i32, i32) {
    let w = text_width("READY");
    ((fx - 9).clamp(2, W - 3 - w), (fy + 17).clamp(2, H - 12))
}

/// What the reticle covers: the far and the near ones, the count and the
/// READY by them, each bracket and its numbers. No label may be laid on it.
pub(super) fn reticle_boxes(c: &Ctx) -> Vec<Box2> {
    let Some(me) = c.craft() else {
        return Vec::new();
    };
    if me.down > 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut n = 0;
    for (x, y, half, k) in lock_marks(c, me) {
        n += i32::from(k);
        out.push(Box2::new(
            x - half - 1,
            y - half - 10,
            2 * half + 3 + i32::from(k) * 6 + 8,
            2 * half + 12,
        ));
    }
    if let (Some(f), Some(nr), _) = aim(c, me) {
        let (fx, fy) = (f.0.round() as i32, f.1.round() as i32);
        let (nx, ny) = (nr.0.round() as i32, nr.1.round() as i32);
        out.push(Box2::new(fx - 14, fy - 14, 29, 29));
        out.push(Box2::new(nx - 9, ny - 9, 19, 19));
        if n > 0 {
            let text = format!("x{n}");
            let (x, y) = count_at(fx, fy, &text);
            out.push(plate_of(x, y, &text));
        }
        if me.charge >= gw::Nums::default().charge.max(1) {
            let (x, y) = ready_at(fx, fy);
            out.push(plate_of(x, y, "READY"));
        }
    }
    out
}

/// The reticle: the far one small and exact, the near one wider, a line
/// down to the craft, a ring that fills as the charge gathers, brackets on
/// what is locked.
fn reticles(im: &mut Img, c: &Ctx) {
    let Some(me) = c.craft() else { return };
    if me.down > 0 {
        return;
    }
    let s = c.s;
    let tick = s.elapsed;
    // The locks.
    let mut n = 0;
    for (x, y, half, k) in lock_marks(c, me) {
        let pulse = tick % 6 < 3;
        brackets(im, x, y, half, 4, if pulse { '6' } else { '5' });
        for j in 0..k {
            n += 1;
            im.text(
                x + half - 3 + i32::from(j) * 6,
                (y - half - 8).max(1),
                &n.to_string(),
                '6',
            );
        }
    }
    let (far, near, craft) = aim(c, me);
    let ink = if me.held > 7 { '5' } else { '6' };
    if let (Some(f), Some(nr)) = (far, near) {
        let (fx, fy) = (f.0.round() as i32, f.1.round() as i32);
        let (nx, ny) = (nr.0.round() as i32, nr.1.round() as i32);
        // The line from the craft to the reticles, dotted.
        if let Some(cr) = craft {
            let (cx, cy) = (cr.0 as i32, cr.1 as i32);
            for k in 0..16 {
                if k % 3 == 0 {
                    let t = k as f32 / 16.0;
                    let px = cx + ((nx - cx) as f32 * t) as i32;
                    let py = cy + ((ny - cy) as f32 * t) as i32;
                    im.put(px, py, '4');
                }
            }
        }
        // The near reticle: four corner ticks. The far one: a ring and a cross.
        brackets(im, nx, ny, 8, 3, '4');
        ring(im, fx, fy, 7, 'k');
        ring(im, fx, fy, 6, ink);
        ring(im, fx, fy, 5, ink);
        for d in 0..2 {
            im.line(fx - 10, fy + d, fx - 3, fy + d, ink);
            im.line(fx + 3, fy + d, fx + 10, fy + d, ink);
            im.line(fx + d, fy - 10, fx + d, fy - 3, ink);
            im.line(fx + d, fy + 3, fx + d, fy + 10, ink);
        }
        im.rect(fx, fy, 2, 2, 'w');
        // The charge: an arc about the far reticle, a segment at a time.
        if me.charge > 0 {
            let full = gw::Nums::default().charge.max(1);
            let segs = 24;
            let lit = (segs * me.charge.min(full) / full) as i32;
            let done = me.charge >= full;
            for k in 0..segs as i32 {
                let a =
                    k as f32 / segs as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                let (px, py) = (
                    fx + (a.dcos() * 13.0).round() as i32,
                    fy + (a.dsin() * 13.0).round() as i32,
                );
                let on = k < lit;
                im.put(
                    px,
                    py,
                    if done && tick % 4 < 2 {
                        'w'
                    } else if on {
                        '5'
                    } else {
                        'g'
                    },
                );
                if on {
                    im.put(
                        px + a.dcos().signum() as i32 * i32::from(a.dcos().abs() > 0.3),
                        py,
                        if done { '9' } else { '4' },
                    );
                }
            }
            if done {
                let (x, y) = ready_at(fx, fy);
                label(im, x, y, "READY", '5');
            }
        }
        if n > 0 {
            let text = format!("x{n}");
            let (x, y) = count_at(fx, fy, &text);
            label(im, x, y, &text, '6');
        }
    }
}

/// A rolling craft's spinning shield: bright while it turns bolts round, dim
/// for the last few ticks of the roll.
fn shields(im: &mut Img, c: &Ctx) {
    let s = c.s;
    let reflect = gw::Nums::default().reflect();
    let one = |im: &mut Img, craft: &Craft| {
        if craft.roll == 0 || craft.down > 0 {
            return;
        }
        let Some((x, y, d)) = c.at(s.craft_s(craft), craft.x, craft.z) else {
            return;
        };
        let (x, y) = (x.round() as i32, y.round() as i32);
        let r = (c.tall(5.2, d) / 2.0).clamp(10.0, 44.0);
        let live = craft.reflecting(reflect);
        let spin = (craft.roll_len - craft.roll) as f32 * 0.55 * f32::from(craft.dir);
        for k in 0..14 {
            let a = spin + k as f32 * std::f32::consts::TAU / 14.0;
            let (px, py) = (x + (a.dcos() * r) as i32, y + (a.dsin() * r * 0.8) as i32);
            let ink = if !live {
                'u'
            } else if k % 2 == 0 {
                'w'
            } else {
                '3'
            };
            im.rect(px, py, 2, 2, ink);
        }
        if live {
            ring(im, x, y, r as i32 - 2, 'z');
        }
    };
    for seat in s.seats.values() {
        one(im, &seat.craft);
    }
    for w in &s.wing {
        one(im, &w.craft);
    }
}

/// The picture's flashes, pops and rings.
fn flares(im: &mut Img, c: &Ctx) {
    let s = c.s;
    for f in &s.fx {
        let age = age_of(f);
        match f.what {
            Burst::Starfall => {
                // A white-gold star sweeps the screen from its middle.
                let rays = 20;
                let reach = 40 + age * 12;
                for k in 0..rays {
                    let a = k as f32 / rays as f32 * std::f32::consts::TAU + age as f32 * 0.05;
                    let (dx, dy) = (a.dcos(), a.dsin());
                    if age < 28 {
                        im.line(
                            W / 2 + (dx * 10.0) as i32,
                            H / 2 + (dy * 8.0) as i32,
                            W / 2 + (dx * reach as f32 * 1.4) as i32,
                            H / 2 + (dy * reach as f32) as i32,
                            if k % 2 == 0 { '6' } else { 'w' },
                        );
                    }
                }
                if age < 5 {
                    for y in 0..H {
                        for x in 0..W {
                            if (x + y + age) % 2 == 0 {
                                im.put(x, y, if age < 2 { 'w' } else { '9' });
                            }
                        }
                    }
                }
            }
            _ => {
                let Some((x, y, d)) = c.at(f.s, f.x, f.z) else {
                    continue;
                };
                let (x, y) = (x.round() as i32, y.round() as i32);
                let u = c.tall(1.0, d).max(2.0);
                match f.what {
                    Burst::Pop => {
                        // Cold smoke, and a spark of returned light.
                        let r = (u * (0.5 + age as f32 * 0.16)) as i32;
                        ring(im, x, y, r.max(2), 'h');
                        ring(im, x, y, (r - 2).max(1), 'J');
                        for k in 0..6 {
                            let a = k as f32 * 1.047 + age as f32 * 0.2;
                            im.put(
                                x + (a.dcos() * r as f32 * 0.6) as i32,
                                y + (a.dsin() * r as f32 * 0.6) as i32,
                                'i',
                            );
                        }
                        if age < 9 {
                            disc(im, x, y, ((9 - age) as f32 * 0.4 * u / 4.0) as i32 + 1, '6');
                            im.put(x, y, 'w');
                        }
                    }
                    Burst::Hit => {
                        disc(im, x, y, 2 + (3 - age.min(3)), 'w');
                        ring(im, x, y, 3 + age / 2, '6');
                    }
                    Burst::Turn => {
                        let r = (u * (0.7 + age as f32 * 0.22)) as i32;
                        ring(im, x, y, r, '6');
                        ring(im, x, y, (r - 2).max(1), 'w');
                    }
                    Burst::Lock => {
                        let r = (u * 1.1) as i32 + (8 - i32::from(f.ticks)).max(0) / 2;
                        brackets(im, x, y, r.max(6) + i32::from(f.ticks) / 2, 4, 'w');
                    }
                    Burst::Gate => {
                        let r = (u * (1.0 + age as f32 * 0.4)) as i32;
                        ring(im, x, y, r, '5');
                        ring(im, x, y, r + 2, '6');
                        for k in 0..8 {
                            let a = k as f32 * 0.785;
                            im.put(
                                x + (a.dcos() * (r + 4) as f32) as i32,
                                y + (a.dsin() * (r + 4) as f32) as i32,
                                'w',
                            );
                        }
                    }
                    Burst::Scrape => {
                        for k in 0..10 {
                            let a = k as f32 * 0.628;
                            let r = (age as f32 * 1.6 + 2.0) * (1.0 + (k % 3) as f32 * 0.4);
                            im.put(
                                x + (a.dcos() * r) as i32,
                                y + (a.dsin() * r) as i32 + age / 2,
                                if k % 2 == 0 { 'G' } else { 'h' },
                            );
                        }
                    }
                    Burst::Fall => {
                        for k in 0..12 {
                            let a = k as f32 * 0.52 + age as f32 * 0.1;
                            let r = 3.0 + age as f32 * 0.8;
                            im.put(
                                x + (a.dcos() * r) as i32,
                                y + (a.dsin() * r * 0.6) as i32 + age,
                                if k % 2 == 0 { 'i' } else { 'H' },
                            );
                        }
                    }
                    Burst::Hurt => {
                        ring(im, x, y, (u * (0.8 + age as f32 * 0.15)) as i32, '7');
                    }
                    Burst::Lamp => {
                        let r = (u * (0.6 + age as f32 * 0.3)) as i32;
                        ring(im, x, y, r, '3');
                        ring(im, x, y, r + 2, 'w');
                        disc(im, x, y, (u * 0.5) as i32, '2');
                    }
                    Burst::Starfall => {}
                }
            }
        }
    }
}

// ─── the HUD ────────────────────────────────────────────────────────────────

/// The shield, the bombs and the spur, top left: the plate they sit on.
const GAUGES: Box2 = Box2::new(5, 5, 88, 34);
/// The spur's meter, in the plate.
const SPUR_BAR: Box2 = Box2::new(66, 28, 22, 4);
/// The score and the team's, top right: the plate they sit on.
const SCORE: Box2 = Box2::new(296, 5, 83, 22);
/// The key strip along the bottom, for the first forty seconds.
const KEYS_TEXT: &str = "F SHOOT  HOLD F LOCK  SPACE ROLL  E SPUR  Q BRAKE  X BOMB";
/// A call-out's plate is this tall, and they stack at this pitch.
const CALL_H: i32 = 25;
const CALL_PITCH: i32 = 28;

fn keys_box() -> Box2 {
    let w = text_width(KEYS_TEXT);
    Box2::new((W - w) / 2 - 4, H - 12, w + 8, 11)
}

/// The shield, the bombs and the spur, top left.
fn gauges(im: &mut Img, c: &Ctx) {
    let Some(me) = c.craft() else { return };
    let tick = c.s.elapsed;
    plate(im, GAUGES.x, GAUGES.y, GAUGES.w, GAUGES.h);
    im.text(10, 8, "SHIELD", 'h');
    let (bx, by, bw) = (10, 17, 76);
    im.rect(bx, by, bw, 6, 'g');
    let low = me.shield <= 30;
    let ink = if low {
        if tick % 8 < 4 { '7' } else { '8' }
    } else if me.shield <= 60 {
        '5'
    } else {
        '3'
    };
    let fill = bw * me.shield.max(0) / gw::SHIELD;
    im.rect(bx, by, fill, 6, ink);
    im.rect(bx, by, fill, 1, 'w');
    for k in 1..4 {
        im.rect(bx + bw * k / 4, by, 1, 6, 'k');
    }
    // Starfall, a star each.
    for k in 0..3u8 {
        let (sx, sy) = (11 + i32::from(k) * 9, 26);
        let on = k < me.bombs;
        let ink = if on { '5' } else { 'g' };
        im.rect(sx + 2, sy, 1, 6, ink);
        im.rect(sx, sy + 2, 5, 1, ink);
        if on {
            im.put(sx + 1, sy + 1, '6');
            im.put(sx + 3, sy + 1, '6');
            im.put(sx + 1, sy + 3, '6');
            im.put(sx + 3, sy + 3, '6');
        }
    }
    // The spur's meter: the word ends three dots before it starts.
    im.text(40, 26, "SPUR", 'h');
    im.rect(SPUR_BAR.x, SPUR_BAR.y, SPUR_BAR.w, SPUR_BAR.h, 'g');
    let ink = if me.spent {
        '7'
    } else if me.boost {
        'w'
    } else {
        '2'
    };
    im.rect(
        SPUR_BAR.x,
        SPUR_BAR.y,
        SPUR_BAR.w * me.meter / gw::METER,
        SPUR_BAR.h,
        ink,
    );
}

/// The score, the hit counter, top right.
fn score(im: &mut Img, c: &Ctx) {
    let Some(me) = c.craft() else { return };
    plate(im, SCORE.x, SCORE.y, SCORE.w, SCORE.h);
    let mine = format!("{}", me.pts);
    im.text(301, 8, "SCORE", 'h');
    im.text(374 - text_width(&mine), 8, &mine, '5');
    let team = format!("{}", c.s.bank.pts + c.s.beat.pts);
    im.text(301, 17, "TEAM", 'h');
    im.text(374 - text_width(&team), 17, &team, 'i');
    if let Some((text, k, cx)) = chain_of(c) {
        big(im, cx, 32, &text, '5', k);
    }
}

/// The hit counter under the score: its words, its size, its centre (kept
/// in the frame however long the chain).
fn chain_of(c: &Ctx) -> Option<(String, i32, i32)> {
    let me = c.craft()?;
    if me.chain < 2 {
        return None;
    }
    let text = format!("{} HITS", me.chain);
    // The big counter makes way for a beat's card, the small one stays.
    let k = 1 + i32::from(me.chain_ticks > 66 && card(c).is_none());
    let cx = 332.min(W - 6 - text_width(&text) * k / 2);
    Some((text, k, cx))
}

fn roster_name(c: &Ctx) -> String {
    format!(
        "{}  {}/3",
        rail::NAMES[usize::from(c.s.stage.min(2))],
        c.s.stage + 1
    )
}

/// The beat's name over the row of pips, as one box.
fn roster_box(c: &Ctx) -> Box2 {
    let n = (c.s.seats.len() + c.s.wing.len()) as i32;
    let title = centred_box(W / 2, 6, &roster_name(c));
    title.union(&Box2::new((W - n * 15) / 2, 16, n * 15 - 2, 15))
}

/// The wing: a pip for each craft, its shield under it, the beat's name.
fn roster(im: &mut Img, c: &Ctx) {
    let s = c.s;
    let name = roster_name(c);
    centred(im, W / 2, 6, &name, 'i');
    let mut pips: Vec<(u8, i32, bool, bool)> = Vec::new();
    for (&id, seat) in &s.seats {
        pips.push((
            id as u8,
            seat.craft.shield,
            seat.craft.down > 0,
            s.latched(id as u8),
        ));
    }
    for w in &s.wing {
        pips.push((w.id, w.craft.shield, w.lost, s.latched(w.id)));
    }
    let width = pips.len() as i32 * 15;
    let x0 = (W - width) / 2;
    for (k, (id, shield, out, latched)) in pips.iter().enumerate() {
        let x = x0 + k as i32 * 15;
        let (_, ink) = who(c.run, *id);
        im.rect(x, 17, 13, 11, 'k');
        im.frame(
            x,
            17,
            13,
            11,
            if u32::from(*id) == c.seat { 'w' } else { ink },
        );
        if *out {
            im.line(x + 2, 19, x + 10, 25, '7');
            im.line(x + 10, 19, x + 2, 25, '7');
        } else {
            im.rect(x + 3, 19, 7, 5, ink);
        }
        im.rect(x, 29, 13, 2, 'g');
        if !out {
            let low = if *shield <= 30 { '7' } else { '3' };
            im.rect(x, 29, 13 * shield.max(&0) / gw::SHIELD, 2, low);
        }
        if *latched {
            im.rect(x + 9, 16, 4, 4, '7');
            im.put(x + 10, 17, 'w');
        }
    }
}

/// The Wyrm's lamps and the lit one's flame.
/// The panel the lamps sit on, while the Wyrm is up.
fn wyrm_box(c: &Ctx) -> Option<Box2> {
    let b = c.s.boss.as_ref()?;
    if b.st == Wyrm::Rise || matches!(c.s.mode, Mode::Victory | Mode::Tally) {
        return None;
    }
    Some(Box2::new(W / 2 - 54, 36, 108, 24))
}

fn wyrm_panel(im: &mut Img, c: &Ctx) {
    let Some(b) = &c.s.boss else { return };
    let Some(panel) = wyrm_box(c) else { return };
    let y = panel.y;
    plate(im, panel.x, panel.y, panel.w, panel.h);
    for (k, id) in b.lamps.iter().enumerate() {
        let x = W / 2 - 48 + k as i32 * 22;
        let foe = c.s.foes.iter().find(|f| f.id == *id);
        let lit = usize::from(b.phase) == k;
        let (ink, edge) = match foe.map(|f| f.st) {
            Some(St::Broken) => ('g', 'j'),
            Some(St::Open) => ('5', 'w'),
            _ if lit => ('3', 'w'),
            _ => ('S', 'u'),
        };
        let (cx, cy) = (x + 6, y + 10);
        for d in 0..6 {
            im.line(cx - d, cy - 6 + d, cx + d, cy - 6 + d, ink);
            im.line(cx - d, cy + 6 - d, cx + d, cy + 6 - d, ink);
        }
        im.put(cx, cy, edge);
        if lit && foe.is_some_and(|f| f.st != St::Broken) {
            ring(im, cx, cy, 8, edge);
        }
    }
    // The lit lamp's flame.
    let lit = b.lamps[usize::from(b.phase.min(2))];
    if let Some(f) = c.s.foes.iter().find(|f| f.id == lit && f.st != St::Broken) {
        im.rect(W / 2 + 22, y + 6, 28, 5, 'g');
        im.rect(
            W / 2 + 22,
            y + 6,
            28 * f.hp as i32 / f.full.max(1) as i32,
            5,
            if f.st == St::Open { '5' } else { 'u' },
        );
        im.text(
            W / 2 + 22,
            y + 14,
            if f.st == St::Open { "OPEN" } else { "SHUT" },
            if f.st == St::Open { '5' } else { 'z' },
        );
    }
}

/// The wing's call-outs, newest first: a portrait and a short line on a
/// plate. They stack up from just over the key strip (whether or not it is
/// showing, so they do not jump when it goes), each as wide as its words.
fn call_plates(c: &Ctx) -> Vec<(gw::Call, Box2)> {
    let keys = keys_box();
    c.s.calls
        .iter()
        .rev()
        .take(2)
        .enumerate()
        .map(|(i, call)| {
            let (name, _) = who(c.run, call.who);
            let me = u32::from(call.who) == c.seat;
            let line = call.line.says(me, call.who <= 6);
            let w = (33 + text_width(&name).max(text_width(line)) + 7).max(96);
            let y = keys.y - 2 - CALL_H - i as i32 * CALL_PITCH;
            (*call, Box2::new(6, y, w, CALL_H))
        })
        .collect()
}

/// The wing's call-outs: a portrait and a short line, newest at the bottom.
fn calls(im: &mut Img, c: &Ctx) {
    for (call, b) in call_plates(c) {
        let fade = call.ticks < 12 && call.ticks % 4 < 2;
        if fade {
            continue;
        }
        plate(im, b.x, b.y, b.w, b.h);
        portrait(im, c.run, call.who, b.x + 3, b.y + 3);
        let (name, ink) = who(c.run, call.who);
        // The name on its own black plate: a red cloth is dim on the slate.
        label(im, b.x + 27, b.y + 4, &name, ink);
        let me = u32::from(call.who) == c.seat;
        im.text(b.x + 27, b.y + 14, call.line.says(me, call.who <= 6), 'w');
    }
}

/// A beat's card: where its plate and its lines go. `None` once it is gone.
pub(super) struct Card {
    pub plate: Box2,
    pub name: &'static str,
    pub words: &'static str,
    pub keys: Option<&'static str>,
}

pub(super) fn card(c: &Ctx) -> Option<Card> {
    let s = c.s;
    if s.mode != Mode::Play || s.t >= 90 || (s.stage == 2 && s.t >= rail::BOSS_AT) {
        return None;
    }
    // A knight who is down has the middle of the picture to say so.
    if c.craft().is_some_and(|me| me.down > 0) {
        return None;
    }
    let stage = usize::from(s.stage.min(2));
    let (name, words) = (rail::NAMES[stage], rail::WORDS[stage]);
    let keys = (s.stage == 0 && s.retries == 0).then_some("F SHOOT   SPACE ROLL   X BOMB");
    let inner = (text_width(name) * 2 + 6)
        .max(text_width(words) + 2)
        .max(keys.map_or(0, |k| text_width(k) + 2));
    let w = inner + 28;
    let h = if keys.is_some() { 48 } else { 36 };
    Some(Card {
        plate: Box2::new((W - w) / 2, 44, w, h),
        name,
        words,
        keys,
    })
}

impl Card {
    /// Each line's own plate, top to bottom: all inside the card's.
    #[cfg(test)]
    pub(super) fn lines(&self) -> Vec<Box2> {
        let y = self.plate.y;
        let mut out = vec![
            big_box(W / 2, y + 6, self.name, 2),
            centred_box(W / 2, y + 25, self.words),
        ];
        if let Some(k) = self.keys {
            out.push(centred_box(W / 2, y + 37, k));
        }
        out
    }
}

fn title(im: &mut Img, c: &Ctx) {
    let Some(card) = card(c) else { return };
    let left = 90 - c.s.t;
    if left < 12 && left % 4 < 2 {
        return;
    }
    let y = card.plate.y;
    plate(im, card.plate.x, card.plate.y, card.plate.w, card.plate.h);
    big(im, W / 2, y + 6, card.name, '5', 2);
    centred(im, W / 2, y + 25, card.words, 'w');
    if let Some(k) = card.keys {
        centred(im, W / 2, y + 37, k, 'h');
    }
}

/// A line of words that stands in the middle of the picture: centred on
/// `cx` at `y`, `k` times the font's size, under a group's name.
struct Say {
    group: &'static str,
    cx: i32,
    y: i32,
    text: String,
    ink: char,
    k: i32,
}

fn says(c: &Ctx) -> Vec<Say> {
    let s = c.s;
    let mut out = Vec::new();
    let mut say = |group: &'static str, y: i32, text: &str, ink: char, k: i32| {
        out.push(Say {
            group,
            cx: W / 2,
            y,
            text: text.to_string(),
            ink,
            k,
        });
    };
    match s.mode {
        Mode::Retry => {
            say("retry", 84, "TRY AGAIN", '7', 3);
            say("retry", 112, "THE BEAT STARTS OVER", 'i', 1);
        }
        Mode::Victory if s.mode_t < 150 => say("victory", 60, "THE WYRM FALLS", '5', 2),
        _ => {}
    }
    if let Some(me) = c.craft() {
        if me.down > 0 && s.mode == Mode::Play {
            let secs = me.down.div_ceil(HZ);
            say("down", 80, "DOWN", '7', 3);
            say("down", 106, &format!("RISING IN {secs}"), 'i', 1);
        }
    }
    out
}

impl Say {
    fn plate(&self) -> Box2 {
        if self.k > 1 {
            big_box(self.cx, self.y, &self.text, self.k)
        } else {
            centred_box(self.cx, self.y, &self.text)
        }
    }
}

/// The retry, a fall, the victory banner.
fn states(im: &mut Img, c: &Ctx) {
    let s = c.s;
    if s.mode == Mode::Retry {
        // The picture darkens and says so.
        for y in (0..H).step_by(2) {
            im.rect(0, y, W, 1, 'k');
        }
    }
    for say in says(c) {
        if say.k > 1 {
            big(im, say.cx, say.y, &say.text, say.ink, say.k);
        } else {
            centred(im, say.cx, say.y, &say.text, say.ink);
        }
    }
    if let Some(me) = c.craft() {
        // Worn thin: a red edge that breathes.
        if me.shield <= 30 && me.down == 0 && s.elapsed % 24 < 12 {
            for k in 0..3 {
                im.frame(k, k, W - 2 * k, H - 2 * k, if k == 0 { '7' } else { '8' });
            }
        }
        if me.hurt > FLASH && me.down == 0 {
            for k in 0..2 {
                im.frame(k, k, W - 2 * k, H - 2 * k, '7');
            }
        }
    }
}

fn keys_shown(s: &Session) -> bool {
    s.elapsed <= 40 * HZ && s.mode == Mode::Play
}

/// The key strip, small and quiet, for the first forty seconds.
fn keys(im: &mut Img, c: &Ctx) {
    if !keys_shown(c.s) {
        return;
    }
    let b = keys_box();
    plate(im, b.x, b.y, b.w, b.h);
    im.text(b.x + 4, b.y + 2, KEYS_TEXT, 'h');
}

fn time(ticks: u32) -> String {
    let secs = ticks / HZ;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// The tally's panel: tall enough for a row a knight.
fn tally_box(s: &Session) -> Box2 {
    let (pw, ph) = (300, 106 + s.seats.len() as i32 * 14);
    Box2::new((W - pw) / 2, 22, pw, ph)
}

/// The end tally: hits, wingmen saved, time, the medal, and what each knight
/// did.
fn tally(im: &mut Img, c: &Ctx) {
    let s = c.s;
    if s.mode != Mode::Tally {
        return;
    }
    let panel = tally_box(s);
    let (px, py, pw, ph) = (panel.x, panel.y, panel.w, panel.h);
    plate(im, px, py, pw, ph);
    big(im, W / 2, py + 7, "BEACON LIT", '5', 2);
    let medal = s.medal.unwrap_or(Medal::Bronze);
    let ink = match medal {
        Medal::Gold => '5',
        Medal::Silver => 'H',
        Medal::Bronze => 'R',
    };
    // The medal: a disc on a ribbon, labelled.
    let (mx, my) = (px + 40, py + 54);
    im.rect(mx - 6, my - 28, 4, 14, '7');
    im.rect(mx + 2, my - 28, 4, 14, '1');
    disc(im, mx, my, 15, ink);
    disc(im, mx, my, 11, if medal == Medal::Gold { '6' } else { 'W' });
    ring(im, mx, my, 13, 'k');
    im.text(
        mx - 2,
        my - 3,
        &medal.name().chars().take(1).collect::<String>(),
        'k',
    );
    centred(im, mx, my + 20, medal.name(), ink);
    let lines = [
        ("HITS", format!("{}", s.bank.kills)),
        ("SAVED", format!("{}", s.bank.saved)),
        ("LOST", format!("{}", s.bank.lost)),
        ("TIME", time(s.done.unwrap_or(s.elapsed))),
        ("SCORE", format!("{}", s.bank.pts)),
    ];
    for (k, (name, value)) in lines.iter().enumerate() {
        let y = py + 28 + k as i32 * 12;
        im.text(px + 90, y, name, 'h');
        if *name == "LOST" && s.bank.lost > 0 {
            label(im, px + 160, y, value, '7');
        } else {
            im.text(px + 160, y, value, 'w');
        }
    }
    im.text(px + 210, py + 28, "PURSE", 'h');
    im.text(px + 210, py + 40, &format!("+{}G", s.purse()), '4');
    if s.retries > 0 {
        label(im, px + 210, py + 52, &format!("FALLS {}", s.retries), '7');
    }
    // What each knight did.
    for (k, (&id, seat)) in s.seats.iter().enumerate() {
        let y = py + 96 + k as i32 * 14;
        if id == c.seat {
            chrome::highlight(im, (px + 6, y - 2, pw - 12, 13));
        }
        let (name, ink) = who(c.run, id as u8);
        im.rect(px + 12, y, 6, 6, ink);
        label(im, px + 24, y, &name, ink);
        im.text(px + 110, y, &format!("{} PTS", seat.craft.pts), 'w');
        im.text(px + 170, y, &format!("{} HITS", seat.craft.hits), 'h');
        im.text(px + 230, y, &format!("{} SAVED", seat.craft.saved), 'h');
    }
    if s.mode_t > gw::TALLY_HOLD {
        centred(im, W / 2, py + ph - 12, "E: RETURN", '5');
    }
}

/// The health bars of the foes that wear names: no label is laid on them.
pub(super) fn bar_boxes(c: &Ctx) -> Vec<Box2> {
    named(c).iter().filter_map(bar_of).collect()
}

/// Every plate and block of words the HUD has up this tick, by name. They are
/// what `paint` draws, and none may lie on another: the tests hold them to it,
/// at every seat count, over a whole run. (A fading call-out and a blinking
/// card count as up: their place is theirs.)
pub(super) fn boxes(c: &Ctx) -> Vec<(&'static str, Box2)> {
    let s = c.s;
    let mut out = Vec::new();
    if s.mode == Mode::Tally {
        // The tally has the picture to itself.
        out.push(("tally", tally_box(s)));
        return out;
    }
    if c.craft().is_some() {
        out.push(("gauges", GAUGES));
        out.push(("score", SCORE));
        if let Some((text, k, cx)) = chain_of(c) {
            out.push(("chain", big_box(cx, 32, &text, k)));
        }
    }
    out.push(("roster", roster_box(c)));
    if let Some(b) = wyrm_box(c) {
        out.push(("wyrm", b));
    }
    for (_, b) in call_plates(c) {
        out.push(("call", b));
    }
    if let Some(card) = card(c) {
        out.push(("card", card.plate));
    }
    if keys_shown(s) {
        out.push(("keys", keys_box()));
    }
    let mut groups: Vec<(&'static str, Box2)> = Vec::new();
    for say in says(c) {
        let b = say.plate();
        match groups.iter_mut().find(|(g, _)| *g == say.group) {
            Some((_, g)) => *g = g.union(&b),
            None => groups.push((say.group, b)),
        }
    }
    out.extend(groups);
    out
}

/// Everything over the scene, for the viewer's own craft.
pub fn paint(im: &mut Img, c: &Ctx) {
    let s = c.s;
    if c.craft().is_some_and(|me| me.boost) {
        streaks(im, s.elapsed);
    }
    footprints(im, c);
    pips(im, c);
    shields(im, c);
    flares(im, c);
    // The names over the rings and flashes, under the bolts and the reticle.
    words(im, c);
    bolts(im, c);
    reticles(im, c);
    // The tally has the picture to itself: the rest of the HUD stands down.
    if s.mode != Mode::Tally {
        gauges(im, c);
        score(im, c);
        roster(im, c);
        wyrm_panel(im, c);
        calls(im, c);
    }
    title(im, c);
    keys(im, c);
    states(im, c);
    tally(im, c);
}

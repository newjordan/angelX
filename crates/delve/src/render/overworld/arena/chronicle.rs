//! What an open floor keeps and shows of what happens on it (`rules::chronicle`
//! and the chambers' own ticks): the remains of the dead, a cairn flying a
//! fallen knight's colours, the ward across the guardian's mouths while it
//! lives, the bars that close them and the doors' slam, the chambers'
//! braziers (cold until a knight comes, blazing while the chamber is not
//! won, coals after), a won chamber's hush, a skull's call from the floor,
//! a ring round a knight raised in a borrowed body and a heartbeat under a
//! knight who is low.
//!
//! Every mark is a function of the run: the traces are kept on the floor,
//! the rest on the chambers' ticks and the heroes. Nothing is kept between
//! frames, so a seat, a friend's mirror and a browser guest draw the same
//! pixels. All of it lies on the floor, under the figures (`under`), but a
//! cairn's name (`over`).

use super::super::chrome::{self, Tone};
use super::super::ink::{Img, Rgb, bayer, hash, ink, shade, text_width};
use super::pfx::{Pen, Ramp, Shape, Spray, ease, mote, ramps};
use super::{TILE, UNIT, at, dungeon_art, knights, sprites};
use crate::rules::chronicle::Trace;
use crate::rules::gore::{self, Body};
use crate::rules::layout::Chamber;
use crate::rules::{EnemyKind, HZ, Hero, Run};

/// Ticks remains take to spread to their full pool.
const SETTLE: u64 = 36;
/// How long the doors' slam shows.
const SLAM: u64 = 28;
/// How long a won chamber's hush shows.
const HUSH: u64 = 60;
/// A chamber's braziers catch in turn from a knight's first step in,
/// `KINDLE_STEP` ticks apart, each flaring for `FLARE` ticks as it does.
const KINDLE_STEP: u64 = 7;
const FLARE: u64 = 30;
/// Ticks the bars take to close from a doorway's ends to its middle.
const DROP: u64 = 8;
/// How near a knight stands for a cairn to say whose it is (tiles).
const READ_REACH: f32 = 4.0;
/// How near a knight stands for the ward to wake (tiles).
const WARD_REACH: f32 = 8.0;
/// A knight at or under this share of their health shows a heartbeat.
const LOW: u32 = 30;
/// Ticks between a skull's calls, and between a low knight's beats.
const CALL: u64 = 2 * HZ as u64;
const BEAT: u64 = 40;
/// The most motes one pass paints. A pen counts every pixel its shapes
/// reach, in the window or not: a hush's ring is long.
const BUDGET: u32 = 40_000;

fn rgb(ch: char) -> Rgb {
    ink(ch).unwrap_or([0, 0, 0])
}

/// Whether a thing `margin` pixels about `(x, y)` can land in the window.
fn seen(cv: &Img, (x, y): (i32, i32), margin: i32) -> bool {
    let (sx, sy, sw, sh) = cv.span();
    x >= sx - margin && y >= sy - margin && x < sx + sw + margin && y < sy + sh + margin
}

/// Under the figures: what lies on the floor.
pub(super) fn under(cv: &mut Img, run: &Run) {
    let Some(open) = run.open_here() else {
        return;
    };
    for trace in &open.traces {
        let (x, y) = trace.at();
        let spot = at(x, y);
        if !seen(cv, spot, 30) {
            continue;
        }
        match trace {
            Trace::Remains { kind, born, .. } => remains(cv, *kind, spot, *born, run.tick),
            Trace::Cairn { knight, born, .. } => {
                // The bones are the mark while the knight lies on it.
                if !lying_by(run, *knight, (x, y)) {
                    cairn(cv, *knight, spot, *born, run.tick);
                }
            }
        }
    }
    for ch in &open.chambers {
        if ch.guarded() && !ch.cleared && !ch.sealed {
            ward(cv, run, ch);
        }
    }
    // The pack's art leaves the open floor's fixtures out: the guardian's
    // bars and the chambers' braziers are drawn here (`drawn_here`).
    let here = drawn_here(run);
    if here {
        for ch in open.chambers.iter().filter(|c| c.sealed) {
            gate_bars(cv, run, ch);
        }
    }
    let mut pen = Pen::new(cv, BUDGET);
    for (i, ch) in open.chambers.iter().enumerate() {
        if here {
            braziers_lit(&mut pen, run, ch, i);
        }
        let age = run.tick.saturating_sub(ch.sealed_at);
        if ch.sealed_at > 0 && ch.sealed && age < SLAM + DROP {
            slam(&mut pen, ch, age);
        }
        let age = run.tick.saturating_sub(ch.cleared_at);
        if ch.cleared_at > 0 && age < HUSH {
            hush(&mut pen, ch, age);
        }
    }
    calls(&mut pen, run);
    raised(&mut pen, run);
    heartbeats(&mut pen, run);
}

/// Over the figures: a cairn's name, when a knight stands by it.
pub(super) fn over(cv: &mut Img, run: &Run, legible: bool) {
    if !legible {
        return;
    }
    let Some(open) = run.open_here() else {
        return;
    };
    for trace in &open.traces {
        let Trace::Cairn { knight, name, x, y, .. } = trace else {
            continue;
        };
        if lying_by(run, *knight, (*x, *y)) {
            continue;
        }
        let near = run.players.values().any(|h| {
            h.hp > 0 && !h.absent && (h.x - x).hypot(h.y - y) < READ_REACH * crate::rules::TILE_UNITS
        });
        let (px, py) = at(*x, *y);
        if !near || !seen(cv, (px, py), 60) {
            continue;
        }
        let words = format!("{name} FELL HERE");
        let w = text_width(&words);
        let (lx, ly) = (px - w / 2, py - 30);
        chrome::label(cv, (lx, ly), &words, 'J', Tone::SLATE);
    }
}

/// Whether knight `id` lies down by `(x, y)`: their bones are on the spot.
fn lying_by(run: &Run, id: u32, (x, y): (f32, f32)) -> bool {
    run.players.get(&id).is_some_and(|h| h.hp == 0 && !h.absent && (h.x - x).hypot(h.y - y) < 5.0)
}

// ─── remains ─────────────────────────────────────────────────────────────────

/// What a pool of remains is made of: its rim, its heart, a fleck.
struct Stuff {
    rim: Rgb,
    heart: Rgb,
    fleck: Rgb,
    /// Dust and bone lie thin: a scatter, no pool.
    thin: bool,
}

fn stuff(kind: EnemyKind) -> Stuff {
    let of = |rim: char, heart: char, fleck: char, thin: bool| Stuff { rim: rgb(rim), heart: rgb(heart), fleck: rgb(fleck), thin };
    match gore::body(kind) {
        Some(Body::Flesh) => of('8', 'p', '7', false),
        Some(Body::Bone) => of('g', 'G', 'h', true),
        Some(Body::Ash) => of('X', 'g', 'a', true),
        Some(Body::Wood) => of('I', 'B', 'o', true),
        Some(Body::Ichor) => of('F', 'E', 'l', false),
        Some(Body::Shard) => {
            let c = super::vfx::colours_of(kind);
            Stuff { rim: shade(c[2], 0.55), heart: shade(c[1], 0.7), fleck: c[1], thin: true }
        }
        // The great ones scorch the stone they fall on.
        None => of('X', 'j', 'g', false),
    }
}

/// A monster's remains at `(cx, cy)`: a pool, or a scatter, thinned by a
/// dither so the floor shows through, spreading for `SETTLE` ticks.
fn remains(cv: &mut Img, kind: EnemyKind, (cx, cy): (i32, i32), born: u64, now: u64) {
    let stuff = stuff(kind);
    let great = gore::body(kind).is_none();
    let grow = (now.saturating_sub(born) as f32 / SETTLE as f32).clamp(0.0, 1.0);
    let seed = hash(cx, cy, 0x5e) as i32;
    let base = (kind.radius() * UNIT * if stuff.thin { 0.9 } else { 1.3 }).clamp(5.0, 34.0);
    let rx = (base * (0.8 + (seed & 7) as f32 / 18.0) * (0.35 + 0.65 * ease::out_quad(grow))).max(1.0);
    let ry = (rx * 0.55).max(1.0);
    // A thin scatter covers little; a pool is dense at its heart.
    let spread = if stuff.thin { 0.7 } else { 2.2 };
    for dy in -(ry.ceil() as i32)..=ry.ceil() as i32 {
        for dx in -(rx.ceil() as i32)..=rx.ceil() as i32 {
            let d = (dx as f32 / rx).powi(2) + (dy as f32 / ry).powi(2);
            if d > 1.0 {
                continue;
            }
            let (x, y) = (cx + dx, cy + dy);
            let cover = (1.0 - d) * spread;
            if cover <= bayer(x, y) {
                continue;
            }
            cv.set(x, y, if d < 0.3 && !stuff.thin { stuff.heart } else { stuff.rim });
        }
    }
    // Flecks flung out of it: a bone, a spatter, a cinder.
    let flecks = if great { 8 } else { 5 };
    for k in 0..flecks {
        let a = (hash(seed, k, 0x11) & 0xffff) as f32 / 65536.0 * std::f32::consts::TAU;
        let far = rx * (0.7 + (hash(seed, k, 0x12) % 100) as f32 / 160.0) * grow;
        let (fx, fy) = (cx + (a.cos() * far).round() as i32, cy + (a.sin() * far * 0.55).round() as i32);
        cv.set(fx, fy, stuff.fleck);
        if stuff.thin && !great && k == 0 {
            cv.set(fx + 1, fy, stuff.fleck);
        }
        if great {
            // A scorch cracks the stone out from the middle.
            let len = (base * 0.9 * grow).round() as i32;
            for s in 0..len {
                let t = s as f32 / len.max(1) as f32;
                cv.set(cx + (a.cos() * (rx * 0.3 + t * len as f32)).round() as i32, cy + (a.sin() * (ry * 0.3 + t * len as f32 * 0.55)).round() as i32, stuff.rim);
            }
        }
    }
}

// ─── cairns ──────────────────────────────────────────────────────────────────

/// A cairn of three stones, its base at the bottom middle: cold stone, a
/// dark rim round each stone.
fn stones() -> Img {
    Img::from_rows(&[
        "....sSSs....",
        "...sUvUus...",
        "..sSuuxuSs..",
        "..sUvUuuuus.",
        ".sSuuxuuxSSs",
        "sUvUUuuuuxuS",
        "sSsxxsSSsxxs",
    ])
}

/// The ink a knight's cloth is in: its seat's colours (`knights::seat_colours`)
/// for the tabard's red, its shade and its gold.
fn cloth(id: u32) -> (char, char, char) {
    let seat = knights::seat_colours(id);
    let pick = |c: char| seat.iter().find(|p| p.0 == c).map_or(c, |p| p.1);
    (pick('7'), pick('8'), pick('5'))
}

/// A cairn with the knight's pennon on a stick over it: it goes up over a
/// few ticks, and the cloth stirs.
fn cairn(cv: &mut Img, id: u32, (cx, cy): (i32, i32), born: u64, now: u64) {
    let age = now.saturating_sub(born);
    let rise = ease::out_quad((age as f32 / 14.0).min(1.0));
    let stones = stones();
    // A shadow it sits in.
    cv.ellipse(cx as f32, (cy + 1) as f32, 8.0, 2.5, 'k');
    let top = cy - (stones.h as f32 * rise).round() as i32 + 1;
    cv.stamp(&stones, cx - stones.w / 2, top);
    // The pennon: a stick out of the top stone, a cloth that stirs.
    let (main, dark, gold) = cloth(id);
    let stick = (8.0 * rise).round() as i32;
    for k in 0..stick {
        cv.put(cx, top - k, 'P');
    }
    if rise >= 1.0 {
        let wave = ((now / 9) % 4) as i32;
        let lift = [0, 0, 1, 0][wave as usize];
        for (row, tone) in [main, main, dark].into_iter().enumerate() {
            let len = 5 - row as i32 - (wave == 2) as i32;
            for k in 1..=len.max(2) {
                cv.put(cx + k, top - 8 + row as i32 + lift * (k / 3), tone);
            }
        }
        cv.put(cx + 1, top - 8, gold);
    }
}

// ─── the guardian's mouths ───────────────────────────────────────────────────

/// A ward across each of the guardian's mouths while it lives: a band of
/// ember laid along the threshold, faint until a knight comes near and
/// hatched and edged in red then, with a pulse running down it every few
/// seconds. A knight who steps past it is shut in (`Run::seal_guardian`).
fn ward(cv: &mut Img, run: &Run, ch: &Chamber) {
    let (y0, h) = (i32::from(ch.rect.1), i32::from(ch.rect.3));
    for &(c, r) in &ch.gates {
        let (c, r) = (i32::from(c), i32::from(r));
        let across = r == y0 || r == y0 + h - 1;
        let (px, py) = (c * TILE, r * TILE);
        if !seen(cv, (px, py), TILE) {
            continue;
        }
        let near = run.players.values().any(|hero| {
            hero.hp > 0 && !hero.absent && {
                let (dx, dy) = (hero.x - (c as f32 + 0.5) * crate::rules::TILE_UNITS, hero.y - (r as f32 + 0.5) * crate::rules::TILE_UNITS);
                dx.hypot(dy) < WARD_REACH * crate::rules::TILE_UNITS
            }
        });
        let pulse = (run.tick / 2 + (c + r) as u64 * 3) % 48;
        for i in 0..TILE {
            let along = i + if across { c * TILE } else { r * TILE };
            let lit = (u64::from(along as u32) % 48).abs_diff(pulse) < 3;
            for j in 5..11 {
                let edge = j == 5 || j == 10;
                let tone = if edge && lit {
                    '@'
                } else if edge {
                    // Edges run in dashes, two on and one off.
                    if i % 3 == 2 { continue; }
                    if near { '7' } else { '8' }
                } else if near && (i + 2 * j) % 5 < 2 {
                    '8'
                } else if lit {
                    '8'
                } else {
                    continue;
                };
                let (x, y) = if across { (px + i, py + j) } else { (px + j, py + i) };
                cv.put(x, y, tone);
            }
        }
    }
}

/// A tile's place along its run of gate tiles: how far it stands from the
/// nearer end of the run, and how many tiles the run has.
fn place(ch: &Chamber, (c, r): (i32, i32), across: bool) -> (i32, i32) {
    let has = |k: i32| {
        let (c, r) = if across { (c + k, r) } else { (c, r + k) };
        ch.gates.contains(&(c as u16, r as u16))
    };
    let (mut before, mut after) = (0, 0);
    while has(-before - 1) {
        before += 1;
    }
    while has(after + 1) {
        after += 1;
    }
    (before.min(after), before + after + 1)
}

/// The tick after the shutting that the bars reach a tile: the ends of a
/// doorway first, its middle last.
fn arrives(d: i32, n: i32) -> u64 {
    d as u64 * DROP / (((n + 1) / 2).max(1)) as u64
}

/// Whether a gate tile is on a horizontal wall of its chamber.
fn across(ch: &Chamber, r: i32) -> bool {
    r == i32::from(ch.rect.1) || r == i32::from(ch.rect.1) + i32::from(ch.rect.3) - 1
}

/// A shut mouth's bars, closing in from the ends of the doorway to its
/// middle over `DROP` ticks after it shuts (all of them, for a floor that
/// was shut before the tick was kept).
fn gate_bars(cv: &mut Img, run: &Run, ch: &Chamber) {
    let age = if ch.sealed_at == 0 { u64::MAX / 2 } else { run.tick.saturating_sub(ch.sealed_at) };
    for &(c, r) in &ch.gates {
        let (c, r) = (i32::from(c), i32::from(r));
        let across = across(ch, r);
        let (d, n) = place(ch, (c, r), across);
        if age >= arrives(d, n) && seen(cv, (c * TILE, r * TILE), TILE) {
            super::open::bar(cv, c, r, across);
        }
    }
}

/// The doors' slam: as each tile's bars come down it flashes hot and cools
/// to embers, a cloud of dust rolls out of it either side along the floor,
/// and the braziers flare red.
fn slam(pen: &mut Pen, ch: &Chamber, age: u64) {
    let hot = (1.0 - age as f32 / SLAM as f32).max(0.0) * (age as f32 / 2.0).min(1.0);
    for brazier in braziers(ch) {
        let (fx, fy) = flame(brazier);
        if seen(pen.cv, (fx as i32, fy as i32), 30) {
            pen.glow((fx, fy), 20.0, 0.8, ramps::EMBER, 0.6 * hot);
        }
    }
    for &(c, r) in &ch.gates {
        let (c, r) = (i32::from(c), i32::from(r));
        let across = across(ch, r);
        let (d, n) = place(ch, (c, r), across);
        let Some(since) = age.checked_sub(arrives(d, n)) else {
            continue;
        };
        let (px, py) = (c * TILE, r * TILE);
        if since >= SLAM || !seen(pen.cv, (px, py), 40) {
            continue;
        }
        if since < 14 {
            let tone = if since < 3 { '6' } else if since < 6 { '@' } else if since < 10 { '7' } else { '8' };
            for i in 0..TILE {
                // Cooling, it breaks into embers.
                if since >= 10 && (i + since as i32) % 3 != 0 {
                    continue;
                }
                for j in 6..10 {
                    let (x, y) = if across { (px + i, py + j) } else { (px + j, py + i) };
                    pen.put(x, y, tone);
                }
            }
        }
        // Dust rolls out either side, low and slow, thinning as it goes.
        let t = since as f32 / SLAM as f32;
        let (mx, my) = (px + TILE / 2, py + TILE / 2);
        let out = 3.0 + 13.0 * ease::out_quad(t);
        for side in [-1.0f32, 1.0] {
            let jitter = (hash(c, r, 0x51 + (side > 0.0) as u32) % 5) as f32 - 2.0;
            let centre = if across { (mx as f32 + jitter, my as f32 + side * out) } else { (mx as f32 + side * out, my as f32 + jitter) };
            pen.glow(centre, 4.0 + 6.0 * t, 0.55, DUST, 0.5 * (1.0 - t));
        }
    }
}

/// Grey dust: from a pale core down to the dark.
const DUST: Ramp = Ramp(['h', 'J', 'G', 'j', 'g', 'X']);

// ─── a chamber's braziers ────────────────────────────────────────────────────

/// A chamber's four braziers, in tiles, round from the north-west.
fn braziers(ch: &Chamber) -> [(i32, i32); 4] {
    let (x, y, w, h) = (i32::from(ch.rect.0), i32::from(ch.rect.1), i32::from(ch.rect.2), i32::from(ch.rect.3));
    [(x, y), (x + w - 1, y), (x + w - 1, y + h - 1), (x, y + h - 1)]
}

/// Where a brazier's flame stands, in room pixels.
fn flame((c, r): (i32, i32)) -> (f32, f32) {
    ((c * TILE + TILE / 2) as f32, (r * TILE + 6) as f32)
}

/// Whether the open floor's fixtures are drawn here: the Demonic Dungeon's
/// art (`dungeon_art`) bakes the floor and the walls and none of them
/// (`open::fixtures` is the kit's own). A floor drawn in the kit bakes its
/// braziers and bars, and this draws none.
pub(super) fn drawn_here(run: &Run) -> bool {
    dungeon_art::supported(run)
}

/// What a chamber's brazier `k` is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fire {
    /// Cold iron: no knight has come into the chamber, or not to this one yet.
    Cold,
    /// Catching, `0..1` through its flare.
    Catching(f32),
    Blazing,
    /// The chamber is won, its hush over: coals.
    Embers,
}

/// The fire of brazier `k` in `ch` at `now`: cold until a knight first
/// stands in the chamber, then caught one after another round the walls,
/// blazing while the chamber is not won and embers after.
fn fire_of(ch: &Chamber, k: usize, now: u64) -> Fire {
    if !ch.seen {
        return Fire::Cold;
    }
    // A floor seen before the tick was kept has its fires burning.
    let caught = if ch.seen_at == 0 { 0 } else { ch.seen_at + k as u64 * KINDLE_STEP };
    if now < caught {
        return Fire::Cold;
    }
    // A company that was fought, and is dead: its chamber lies in coals. A
    // quiet chamber (the arrival) was never fought and keeps its fires.
    if ch.cleared && ch.stirred && (ch.cleared_at == 0 || now >= ch.cleared_at + HUSH) {
        return Fire::Embers;
    }
    let since = now - caught;
    if ch.seen_at > 0 && since < FLARE {
        return Fire::Catching(since as f32 / FLARE as f32);
    }
    Fire::Blazing
}

/// A chamber's four braziers, each in its state, with the light it throws.
fn braziers_lit(pen: &mut Pen, run: &Run, ch: &Chamber, index: usize) {
    let tick = run.tick as u32;
    for (k, brazier) in braziers(ch).into_iter().enumerate() {
        let (fx, fy) = flame(brazier);
        if !seen(pen.cv, (fx as i32, fy as i32), 40) {
            continue;
        }
        let fire = fire_of(ch, k, run.tick);
        let variant = hash(brazier.0, brazier.1, 101 + index as u32);
        let sprite = match fire {
            Fire::Cold => {
                let mut cold = sprites::brazier(false, 0, variant);
                cold.put(3, 5, 'a');
                cold.put(4, 5, 'a');
                cold
            }
            Fire::Embers => sprites::brazier(false, tick / 4 + k as u32, variant),
            Fire::Catching(_) | Fire::Blazing => sprites::brazier(true, tick / 4 + k as u32, variant),
        };
        pen.cv.stamp(&sprite, brazier.0 * TILE + (TILE - sprite.w) / 2, brazier.1 * TILE + TILE - sprite.h);
        let flicker = 0.92 + 0.08 * ((tick / 3 + k as u32) % 3) as f32 / 2.0;
        match fire {
            Fire::Cold => {}
            Fire::Embers => pen.glow((fx, fy), 12.0, 0.8, ramps::EMBER, 0.14),
            Fire::Blazing => pen.glow((fx, fy), 22.0, 0.8, ramps::FIRE, if ch.guarded() { 0.46 } else { 0.36 } * flicker),
            Fire::Catching(t) => {
                // Up in a moment, then down to the burn it settles to.
                let env = if t < 0.2 { t / 0.2 } else { 1.0 - (t - 0.2) / 0.8 };
                pen.glow((fx, fy), 22.0 + 6.0 * env, 0.8, ramps::FIRE, (0.36 + 0.3 * env) * flicker);
                let spray = Spray {
                    gravity: -0.02,
                    squash: 0.6,
                    ..Spray::round(hash(brazier.0, brazier.1, 0x4b), 5, (fx, fy - 3.0), (0.2, 0.7), (12.0, 24.0))
                };
                spray.each(t * FLARE as f32, |m| mote(pen, &m, Shape::Ember, ramps::EMBER, 1, run.tick + k as u64));
            }
        }
    }
}

// ─── a won chamber ───────────────────────────────────────────────────────────

/// A chamber's last foe fallen: a ring of gold goes out from its middle to
/// its walls, and its four braziers flare and settle.
fn hush(pen: &mut Pen, ch: &Chamber, age: u64) {
    let (x, y, w, h) = (i32::from(ch.rect.0), i32::from(ch.rect.1), i32::from(ch.rect.2), i32::from(ch.rect.3));
    let t = age as f32 / HUSH as f32;
    let (mx, my) = ((x + w / 2) * TILE + TILE / 2, (y + h / 2) * TILE + TILE / 2);
    if !seen(pen.cv, (mx, my), w * TILE / 2 + 20) {
        return;
    }
    let reach = ((w.min(h * 2) - 2) * TILE) as f32 / 2.0;
    pen.ring((mx as f32, my as f32), 6.0 + reach * ease::out_quad(t.min(1.0)), 3.4, 0.5, ramps::HOLY, 0.3 + 0.7 * t);
    // The braziers breathe out: up fast, then down.
    let flare = (1.0 - ease::out_quad(((t - 0.15) / 0.85).clamp(0.0, 1.0))) * (t * 8.0).min(1.0);
    for brazier in braziers(ch) {
        let (fx, fy) = flame(brazier);
        if seen(pen.cv, (fx as i32, fy as i32), 30) {
            pen.glow((fx, fy), 22.0, 0.8, ramps::FIRE, 0.55 * flare);
        }
    }
}

// ─── a skull's call, a heartbeat ─────────────────────────────────────────────

/// The ring a knight's seat is marked in: its cloth's gold, red and shade.
fn seat_ramp(id: u32) -> Ramp {
    let (main, dark, gold) = cloth(id);
    Ramp([gold, main, main, dark, dark, 'k'])
}

/// A fallen knight's skull calls from the floor while it lies uncarried:
/// a ring in their colours opens and fades, each couple of seconds, and a
/// mote lifts off it. A friend far across a chamber sees where to go.
fn calls(pen: &mut Pen, run: &Run) {
    if run.phase == crate::rules::Phase::Wiped {
        return;
    }
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp == 0 && !h.absent) {
        let Some((sx, sy)) = run.skull_lying(id) else {
            continue;
        };
        // The skull lies a little off the bones.
        let (px, py) = at(sx, sy);
        if !seen(pen.cv, (px, py), 30) || hero.fell == 0 {
            continue;
        }
        let phase = (run.tick + u64::from(id) * 17) % CALL;
        let t = phase as f32 / CALL as f32;
        let (px, py) = (px as f32 + 0.5, py as f32 + 1.5);
        pen.ring((px, py), 3.0 + 12.0 * ease::out_quad(t), 1.3, 0.5, seat_ramp(id), t);
        if t < 0.7 {
            pen.light(px.round() as i32, (py - 3.0 - 9.0 * t).round() as i32, seat_ramp(id).at(t));
        }
    }
}

/// A knight raised in a borrowed body (`rules::skulls`) is warded for a
/// moment: a ring in their colours closes round them as the ward runs out,
/// and a few motes of their light lift off at the start.
fn raised(pen: &mut Pen, run: &Run) {
    let ward = crate::rules::skulls::RAISED_WARD;
    for (&id, hero) in run.players.iter() {
        if hero.possessed.is_none() || hero.hp == 0 || hero.absent || hero.invulnerable == 0 || hero.invulnerable > ward {
            continue;
        }
        let (px, py) = at(hero.x, hero.y);
        if !seen(pen.cv, (px, py), 40) {
            continue;
        }
        let left = hero.invulnerable as f32 / ward as f32;
        let ramp = seat_ramp(id);
        pen.ring((px as f32 + 0.5, py as f32 + 1.5), 6.0 + 8.0 * ease::out_quad(left), 1.4, 0.5, ramp, 0.25 + 0.5 * (1.0 - left));
        if left > 0.6 {
            let rise = (left - 0.6) / 0.4;
            for k in 0..4 {
                let a = (k as f32 + 0.5) * std::f32::consts::FRAC_PI_2 + left * 3.0;
                pen.light((px as f32 + a.cos() * 6.0).round() as i32, (py as f32 - 2.0 - (1.0 - rise) * 14.0 + a.sin() * 2.0).round() as i32, ramp.at(1.0 - rise));
            }
        }
    }
}

/// A knight low on health beats a slow dark red ring under them, twice a
/// beat, so the party sees who needs a friend.
fn heartbeats(pen: &mut Pen, run: &Run) {
    for (&id, hero) in run.players.iter() {
        if !low(hero) {
            continue;
        }
        let (px, py) = at(hero.x, hero.y);
        if !seen(pen.cv, (px, py), 30) {
            continue;
        }
        let phase = (run.tick + u64::from(id) * 7) % BEAT;
        // Lub, then dub.
        let t = if phase < 10 { phase as f32 / 10.0 } else if (12..22).contains(&phase) { (phase - 12) as f32 / 10.0 } else { continue };
        pen.ring((px as f32 + 0.5, py as f32 + 2.5), 5.0 + 6.0 * ease::out_quad(t), 1.1, 0.45, BEATING, t);
    }
}

/// Dark red through its shade to the floor.
const BEATING: Ramp = Ramp(['7', '7', '8', '8', 'p', 'b']);

/// Standing, and at or under `LOW` of their health.
fn low(hero: &Hero) -> bool {
    hero.hp > 0 && !hero.absent && !hero.stone && hero.max_hp > 0 && hero.hp * 100 <= hero.max_hp * LOW
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__chronicle__tests.rs"]
mod tests;

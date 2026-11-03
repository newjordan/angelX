//! The garden's night (`garden`), what moves in it over the still art:
//! the lanterns' glow, the mushroom bed breathing light, the kettle's
//! steam and its drip into the jug, notes out of the cottage, balloons on
//! the eaves, the fairies and their trails; the house's bar and a banner
//! over each wave, the bite when a guest gets in at the hatch, the house
//! rattling when it is near gone; the burst when it holds and the slow sad
//! end when it does not; and the puff and the little picture over a knight
//! who takes the herbs or the mushrooms, with what the gnome says to it in
//! a bubble.
//!
//! Every mark is a pure function of the run: its tick, the garden's
//! state, the room's last line and when it was said. Nothing is kept, and
//! no pass paints more than its pen's budget.

use super::super::super::chrome;
use super::super::super::ink::{GLYPH_ADVANCE, Img, bayer, hash, ink, text_width};
use super::super::at;
use super::super::pfx::{self, Mote, Pen, Ramp, Shape, Spray, ease, ramps, unit};
use super::{Party, g, guests};
use crate::rules::{HZ, Run};
use std::f32::consts::{PI, TAU};

/// The most motes the garden paints under its figures, and over them.
const UNDER: u32 = 1500;
const OVER: u32 = 2600;

/// Each lantern colour's light.
const GLOW: [Ramp; 5] = [ramps::HOLY, ramps::FROST, ramps::NATURE, ramps::EMBER, ramps::BONE];

/// Confetti and fireworks: the party's colours.
const PARTY: [Ramp; 5] = [ramps::GOLD, ramps::GENIE, ramps::ROSE, ramps::NATURE, ramps::FROST];

fn tick_of(run: &Run) -> u32 {
    run.tick as u32
}

// ─── under the figures ───────────────────────────────────────────────────────

/// Under everyone who stands: the lanterns' light, the mushroom bed, the
/// kettle, the cottage's notes, the hatch and the house's wounds.
pub(super) fn under(cv: &mut Img, run: &Run, garden: Option<&g::Garden>) {
    let party = garden.map_or(Party::Quiet, |gd| gd.party);
    let mut pen = Pen::new(cv, UNDER);
    lantern_glow(&mut pen, run, party);
    mushrooms(&mut pen, run.tick);
    kettle(&mut pen, run.tick);
    if matches!(party, Party::Holding | Party::Held) {
        notes(&mut pen, run.tick);
    }
    if let Some(gd) = garden {
        if gd.party == Party::Holding {
            wounds(&mut pen, run, gd);
        }
        hatch(&mut pen, run, gd);
    }
}

/// A glow round every lit lantern, pulsing with the party; when the house
/// falls, a curl of smoke off each as it goes out.
fn lantern_glow(pen: &mut Pen, run: &Run, party: Party) {
    let tick = tick_of(run);
    let mid = (super::STRING - 1) as f32 / 2.0;
    for (k, lx, ly) in super::lanterns(tick, super::droop(run, party)) {
        let at = (lx as f32 + 0.5, (ly + 3) as f32);
        let Some(c) = super::lantern_lit(run, party, k) else {
            if let Some(age) = super::ended(run, party).filter(|_| party == Party::Lost) {
                let from_end = (k as f32).min(2.0 * mid - k as f32);
                let since = age as f32 - from_end / (mid + 0.5) * super::LIGHTS_OUT;
                if (0.0..20.0).contains(&since) {
                    for j in 0..2u32 {
                        let t = since - j as f32 * 5.0;
                        if t > 0.0 {
                            let m = Mote { i: j, at: (at.0 + (t * 0.3).sin(), at.1 - 2.0 - t * 0.6), vel: (0.0, -0.6), k: t / 20.0, seed: k as u32 };
                            pfx::mote(pen, &m, Shape::Smoke, ramps::SMOKE, 1, run.tick);
                        }
                    }
                }
            }
            continue;
        };
        let power = if party == Party::Quiet { 0.4 } else { 0.5 + 0.18 * (tick as f32 * 0.21 + k as f32 * 1.1).sin() };
        pen.glow(at, 7.5, 0.9, GLOW[c], power);
    }
}

/// The mushroom bed breathing: a cold light that swells and settles under
/// its caps, the caps lighting one after the next across the bed, and
/// spores going up off them, twinkling.
fn mushrooms(pen: &mut Pen, tick: u64) {
    let t = tick as f32;
    let (x, y) = super::px(g::SHROOMS);
    let breath = 0.5 + 0.5 * (t * 0.06).sin();
    pen.glow((x as f32, (y - 7) as f32), 26.0, 0.36, ramps::FROST.cooler(1), 0.2 + breath * 0.22);
    for (k, (mx, my, cap)) in super::caps().enumerate() {
        let lit = 0.5 + 0.5 * (t * 0.06 - k as f32 * 0.7).sin();
        let ink = ['q', 'Q', 'z', 'V'][((lit * 3.99) as usize).min(3)];
        pen.cv.ellipse(mx as f32, (my - 1) as f32, cap + 0.5, cap * 0.6, ink);
        pen.cv.put(mx - 1, my - 2, 'W');
        if lit > 0.9 {
            pen.cv.put(mx, my - 2, 'w');
        }
    }
    let tick = tick as u32;
    for k in 0..6u32 {
        let life = 64;
        let n = tick + k * 11;
        let a = (n % life) as f32;
        let s = hash(k as i32, (n / life) as i32, 0x5bd);
        let Some((mx, my, _)) = super::caps().nth((s % 7) as usize) else { continue };
        let p = (mx as f32 + (a * 0.12 + k as f32).sin() * 3.0, my as f32 - 4.0 - a * 0.38);
        pen.twinkle(p, 1.0, a / life as f32, if k % 2 == 0 { ramps::FROST } else { ramps::ARCANE });
    }
}

/// The kettle at its work: steam curling off its lid, a bead at the tap
/// that swells, lets go and drops into the jug, and now and then a glint
/// off the jug.
fn kettle(pen: &mut Pen, tick: u64) {
    let (cx, cy) = super::pot();
    let t = tick as u32;
    for k in 0..8u32 {
        let life = 48;
        let n = t + k * 6;
        let a = n % life;
        let kf = a as f32 / life as f32;
        let s = hash(k as i32, (n / life) as i32, 0x57e);
        let x = cx as f32 + (a as f32 * 0.17 + (s % 7) as f32).sin() * 2.5 + kf * kf * 10.0;
        let y = (cy - 17) as f32 - ease::out(kf) * 24.0;
        let m = Mote { i: k, at: (x, y), vel: (0.3, -0.5), k: kf, seed: s };
        pfx::mote(pen, &m, Shape::Smoke, ramps::SMOKE, 2, tick);
    }
    // The drip: the tap is over the jug's mouth.
    let (dx, tap, mouth) = super::drip();
    let a = t % 40;
    if a < 22 {
        pen.cv.put(dx, tap, 'T');
        if a > 12 {
            pen.cv.put(dx, tap + 1, 't');
        }
    } else if a < 22 + (mouth - tap) as u32 {
        let y = tap + (a - 22) as i32;
        pen.cv.put(dx, y, 'T');
        pen.cv.put(dx, y - 1, 't');
    } else if a < 30 + (mouth - tap) as u32 {
        let s = (a - 22 - (mouth - tap) as u32) as i32;
        pen.light(dx - 1 - s / 3, mouth - 1 - s / 4, 'T');
        pen.light(dx + 1 + s / 3, mouth - 1 - s / 4, 'T');
    }
    let (jx, jy) = super::jug();
    pen.twinkle(((jx - 2) as f32, (jy - 3) as f32), 2.0, (t % 110) as f32 / 18.0, ramps::HOLY);
}

/// Notes out of the cottage while the party's on: out of its windows,
/// its chimney and its door, swaying up over the roof.
fn notes(pen: &mut Pen, tick: u64) {
    let (x, y, w, _) = super::px_rect(g::HOUSE);
    let mouths = [(x + 19, y + 32), (x + w - 19, y + 32), (x + w - 18, y - 4), (x + w / 2, y + 52)];
    const LIFE: u32 = 56;
    let t = tick as u32;
    for k in 0..8u32 {
        let n = t + k * LIFE / 8;
        let a = n % LIFE;
        let s = hash(k as i32, (n / LIFE) as i32, 0x0e5);
        let (mx, my) = mouths[(s % 4) as usize];
        let kf = a as f32 / LIFE as f32;
        let p = (
            mx as f32 + (a as f32 * 0.16 + (s & 7) as f32).sin() * 4.0 + (unit(k as i32, s as i32, 3) - 0.5) * 14.0 * kf,
            my as f32 - 4.0 - ease::out(kf) * 32.0,
        );
        if kf > 0.7 && bayer(p.0 as i32, p.1 as i32) < (kf - 0.7) / 0.3 {
            continue;
        }
        let ramp = [ramps::HOLY, ramps::GENIE, ramps::NATURE, ramps::ROSE][(s >> 4) as usize % 4];
        pfx::mote(pen, &Mote { i: k, at: p, vel: (0.0, -1.0), k: kf * 0.6, seed: s }, Shape::Note, ramp, 1, tick);
    }
}

/// The house near gone: cracks across its plaster (more the lower it
/// is), and at the last its windows guttering, its walls rattling and
/// straw shaken down off its thatch.
fn wounds(pen: &mut Pen, run: &Run, gd: &g::Garden) {
    let (x, y, w, h) = super::px_rect(g::HOUSE);
    let tick = tick_of(run);
    let cracks = [(x + 20, y + 44), (x + w - 22, y + 52), (x + 44, y + 27)];
    let shown = if gd.house * 10 < g::HOUSE_HP * 3 { 3 } else if gd.house * 2 < g::HOUSE_HP { 2 } else if gd.house * 10 < g::HOUSE_HP * 7 { 1 } else { 0 };
    for (k, &(cx, cy)) in cracks.iter().take(shown).enumerate() {
        let s = hash(k as i32, 1, 0xc4a);
        let (mut px, mut py) = (cx, cy);
        for j in 0..6 {
            let (nx, ny) = (px + if (s >> j) & 1 == 0 { 1 } else { -1 }, py + 2);
            pen.cv.line(px, py, nx, ny, if j % 2 == 0 { 'J' } else { 'G' });
            (px, py) = (nx, ny);
        }
    }
    if gd.house * 10 > g::HOUSE_HP * 3 {
        return;
    }
    // The windows gutter.
    if (tick / 3).is_multiple_of(5) {
        for wx in [x + 12, x + w - 26] {
            pen.cv.rect(wx + 2, y + 32, 10, 6, 'a');
        }
    }
    // The walls rattle: motion marks either side of it.
    let jig = (tick / 2 % 2) as i32;
    for k in 0..5 {
        let my = y + 28 + k * 9;
        pen.put(x + 1 - jig, my, 'h');
        pen.put(x + 1 - jig, my + 1, 'h');
        pen.put(x + w - 2 + jig, my + 3, 'h');
        pen.put(x + w - 2 + jig, my + 4, 'h');
    }
    // Straw shaken down off the thatch.
    for k in 0..7u32 {
        let n = tick + k * 5;
        let a = (n % 26) as f32;
        let s = hash(k as i32, (n / 26) as i32, 0x57a);
        let (sx, sy) = (x + 6 + (s % (w as u32 - 12)) as i32, y + 26);
        let p = (sx as f32 + (a * 0.4).sin(), sy as f32 + a * a * 0.05);
        if p.1 < (y + h) as f32 {
            pen.put(p.0 as i32, p.1 as i32, if k % 2 == 0 { 'o' } else { 'P' });
        }
    }
}

/// The cellar hatch: flung up on its hinge as a guest gets in, its back
/// end going down into the dark, and banged shut again; hanging open for
/// good once they have all got in.
fn hatch(pen: &mut Pen, run: &Run, gd: &g::Garden) {
    let (hx, hy) = super::hatch();
    let bite = gd.bitten.map(|(when, kind)| (run.tick.saturating_sub(when) as f32, kind)).filter(|&(a, _)| a < 16.0);
    let lift = match (gd.party, bite) {
        (Party::Lost, _) => 7.0,
        (_, Some((a, _))) => ease::bump(a / 16.0) * 7.0,
        _ => return,
    };
    let cv = &mut *pen.cv;
    cv.rect(hx - 9, hy - 1, 19, 4, 'k');
    cv.rect(hx - 8, hy + 2, 17, 1, 'K');
    if let Some((a, kind)) = bite {
        guests::going_in(cv, kind, (hx, hy), a);
    }
    // The lid, up on its back edge: shorter the higher it stands.
    let top = hy - 2 - lift.round() as i32;
    let deep = (6 - lift.round() as i32).max(2);
    cv.rect(hx - 10, top, 21, deep, 'r');
    cv.frame(hx - 10, top, 21, deep, 'b');
    cv.line(hx, top, hx, top + deep - 1, 'b');
    if gd.party == Party::Lost {
        // What they left on the step.
        for k in 0..6 {
            let s = hash(k, 7, 0xc12);
            cv.put(hx - 12 + (s % 25) as i32, hy + 4 + (s >> 8) as i32 % 3, if k % 2 == 0 { 'o' } else { 'T' });
        }
    }
}

// ─── the gnome's things ──────────────────────────────────────────────────────

/// The gnome's mug where he dropped it, on its side, its foam run out.
pub(super) fn spilt_mug(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x, y - 3, 4, 3, 'o');
    cv.line(x, y - 3, x + 3, y - 3, 'O');
    cv.put(x - 1, y - 2, 'r');
    cv.put(x + 4, y - 3, 'H');
    cv.put(x + 4, y - 2, 'H');
    cv.line(x + 4, y, x + 9, y, 't');
    cv.put(x + 6, y - 1, 'T');
}

/// Balloons tied to the eaves while the party's on, bobbing; when the
/// house falls the air goes out of each in turn, and off it zips about
/// the cottage, smaller and smaller, its breath trailing, and drops to lie
/// limp in the grass by the walls.
pub(super) fn balloons(cv: &mut Img, run: &Run, party: Party) {
    if party == Party::Quiet {
        return;
    }
    let (x, y, w, h) = super::px_rect(g::HOUSE);
    let ties = [(x + 6, y + 27), (x + w - 7, y + 27), (x + 2, y + 44)];
    let colours = [('7', '8', '@'), ('z', 'q', 'W'), ('y', 'l', 'Y'), ('6', 'a', '9'), ('R', 'p', 'o'), ('C', 'e', 'Y')];
    let t = run.tick as f32;
    let gone = super::ended(run, party).filter(|_| party == Party::Lost).map(|a| a as f32);
    for (k, &(tx, ty)) in ties.iter().enumerate() {
        for b in 0..2 {
            let i = k * 2 + b;
            let (body, shade, lit) = colours[i % colours.len()];
            let side = b as f32 * 2.0 - 1.0;
            let home = (
                tx as f32 + side * 4.0 + (t * 0.05 + i as f32 * 1.3).sin() * 1.5,
                ty as f32 - 16.0 - b as f32 * 3.0 + (t * 0.08 + i as f32).sin() * 1.2,
            );
            let rest = (tx + (i as i32 % 3) * 5 - 5, y + h + 3 + (i as i32 % 2) * 3);
            let (c, size) = if party == Party::Lost {
                let a = gone.map_or(f32::MAX, |a| a - i as f32 * 5.0).max(0.0);
                if a >= ZIP {
                    // Lying limp in the grass.
                    cv.rect(rest.0, rest.1, 4, 2, shade);
                    cv.put(rest.0 + 1, rest.1, body);
                    cv.put(rest.0 + 2, rest.1 - 1, shade);
                    cv.line(rest.0 + 4, rest.1 + 1, rest.0 + 8, rest.1 + 2, 'J');
                    continue;
                }
                // Its breath going out behind it.
                for j in 1..5 {
                    let back = zip(home, rest, i, (a - j as f32 * 1.2).max(0.0));
                    if (run.tick as i32 + j) % 2 == 0 {
                        cv.put(back.0 as i32, back.1 as i32, 'h');
                    }
                }
                (zip(home, rest, i, a), 1.0 - (a / ZIP).min(1.0) * 0.6)
            } else {
                cv.line(tx, ty, home.0.round() as i32, (home.1 + 4.0).round() as i32, 'J');
                (home, 1.0)
            };
            let (rx, ry) = (3.2 * size, 4.0 * size);
            for py in (c.1 - ry).floor() as i32..=(c.1 + ry).ceil() as i32 {
                for px in (c.0 - rx).floor() as i32..=(c.0 + rx).ceil() as i32 {
                    let nx = (px as f32 + 0.5 - c.0) / rx;
                    let ny = (py as f32 + 0.5 - c.1) / ry;
                    if nx * nx + ny * ny <= 1.0 {
                        cv.put(px, py, if nx > 0.35 || ny > 0.5 { shade } else { body });
                    }
                }
            }
            cv.put((c.0 - rx * 0.4).round() as i32, (c.1 - ry * 0.45).round() as i32, lit);
            cv.put(c.0.round() as i32, (c.1 + ry).round() as i32, shade);
        }
    }
}

/// The ticks a balloon takes to zip about and drop.
const ZIP: f32 = 34.0;

/// Where balloon `i` is `a` ticks after its air began to go: up and off
/// in loops about where it was tied, then down to where it will lie.
fn zip(home: (f32, f32), rest: (i32, i32), i: usize, a: f32) -> (f32, f32) {
    let seed = i as f32 * 2.1;
    let fly = (a / 22.0).min(1.0);
    let loop_ = (
        home.0 + (a * 0.75 + seed).sin() * (5.0 + a * 0.9),
        home.1 - a * 1.1 + (a * 1.05 + seed).cos() * 4.0,
    );
    if fly < 1.0 {
        return loop_;
    }
    let top = (home.0 + (22.0 * 0.75 + seed).sin() * 24.8, home.1 - 24.2 + (22.0 * 1.05 + seed).cos() * 4.0);
    let k = ease::inward(((a - 22.0) / (ZIP - 22.0)).min(1.0));
    (top.0 + (rest.0 as f32 - top.0) * k, top.1 + (rest.1 as f32 - top.1) * k)
}

// ─── over the figures ────────────────────────────────────────────────────────

/// Over everything that stands: the fairies, the queen's dust, what the
/// guests leave, the bite's splinters and its word, the house's bar, the
/// party's start and its end, the moments a knight takes something, and,
/// while there is room to read them, the banners and the gnome's bubble.
pub(super) fn over(cv: &mut Img, run: &Run, garden: Option<&g::Garden>, legible: bool) {
    let party = garden.map_or(Party::Quiet, |gd| gd.party);
    let mut pen = Pen::new(cv, OVER);
    fairies(&mut pen, run, party);
    guests::wing_dust(&mut pen, run);
    guests::deaths(&mut pen, run);
    if party == Party::Quiet {
        shifty(&mut pen, run);
    }
    party_starts(&mut pen, run);
    if matches!(party, Party::Holding | Party::Held) {
        cheers(&mut pen, run);
    }
    if let Some(gd) = garden {
        bite(&mut pen, run, gd, legible);
        if gd.party == Party::Holding {
            house_bar(pen.cv, run, gd);
        }
    }
    match party {
        Party::Held => victory(&mut pen, run),
        Party::Lost => gloom(&mut pen, run),
        _ => {}
    }
    moments(&mut pen, run, legible);
    if legible {
        banner(pen.cv, run, party);
        bubble(pen.cv, run);
    }
}

/// Where fairy `k` of `n` is at tick `t`: round the cottage, never quite
/// straight, with a hiccup of a dip now and then and, less often, a loop.
fn fairy_at(k: u32, n: u32, t: f32) -> (f32, f32) {
    let (hx, hy) = super::fairy_home();
    let kf = k as f32;
    let a = t * 0.035 + kf * TAU / n as f32;
    let wob = (t * 0.31 + kf).sin() * 4.0 + (t * 0.07 + kf * 2.0).sin() * 3.0;
    let h = (t + kf * 37.0).rem_euclid(97.0);
    let hic = if h < 12.0 { ease::bump(h / 12.0) * 6.0 } else { 0.0 };
    let l = (t + kf * 61.0).rem_euclid(173.0);
    let (lx, ly) = if l < 26.0 {
        let b = ease::both(l / 26.0) * TAU;
        (b.sin() * 7.0, (b.cos() - 1.0) * 6.0)
    } else {
        (0.0, 0.0)
    };
    (hx as f32 + a.cos() * 78.0 + lx, hy as f32 + a.sin() * 26.0 + wob + hic + ly)
}

/// The fairies round the cottage, each with a sparkling trail; more of
/// them once the party's on. When the house falls they sit on the thatch
/// with their wings down.
fn fairies(pen: &mut Pen, run: &Run, party: Party) {
    let tick = run.tick as f32;
    if party == Party::Lost {
        let (x, y, w, _) = super::px_rect(g::HOUSE);
        for k in 0..3 {
            let (fx, fy) = (x + 20 + k * (w - 40) / 2, y + 12 + (k % 2) * 4);
            pen.cv.stamp(&super::fairy(1), fx - 3, fy - 3);
            // A sigh, now and then.
            let a = (run.tick as i32 + k * 31) % 80;
            if a < 12 {
                pen.light(fx + 3 + a / 4, fy - 4 - a / 3, 'h');
            }
        }
        return;
    }
    let n = if party == Party::Quiet { 3 } else { 5 };
    for k in 0..n {
        let at = fairy_at(k, n, tick);
        let mut pts = [(0.0f32, 0.0f32); 9];
        for (j, p) in pts.iter_mut().enumerate() {
            *p = fairy_at(k, n, tick - j as f32 * 1.6);
        }
        pen.ribbon(&pts, if k % 2 == 1 { ramps::GENIE } else { ramps::HOLY }, 1.0);
        for j in [3usize, 6, 8] {
            let tw = ((tick + j as f32 * 2.0 + k as f32 * 5.0) % 14.0) / 14.0;
            pen.twinkle(pts[j], 1.4, tw, ramps::HOLY);
        }
        pen.glow(at, 4.5, 1.0, ramps::HOLY.cooler(1), 0.5);
        let im = super::fairy((run.tick / 3) as usize + k as usize);
        pen.cv.stamp(&im, at.0.round() as i32 - 3, at.1.round() as i32 - 3);
    }
}

/// The blue folk with a knight near: a bead of sweat down the cap, and an
/// innocent little whistle.
fn shifty(pen: &mut Pen, run: &Run) {
    let tick = tick_of(run);
    for (i, &spot) in g::FOLK.iter().enumerate() {
        let (fx, fy) = super::px(spot);
        if !super::folk_near(run, (fx, fy)) {
            continue;
        }
        let face = if (fx < 192) == (i % 2 == 0) { 1 } else { -1 };
        let fx = fx + super::sidle(run, (fx, fy), tick + i as u32 * 9);
        let a = (tick + i as u32 * 13) % 40;
        if a < 24 {
            let (sx, sy) = (fx - face * 5, fy - 11 + a as i32 / 6);
            pen.put(sx, sy, 'z');
            pen.put(sx, sy - 1, if a < 12 { 'w' } else { 'z' });
        }
        let w = (tick + i as u32 * 29) % 64;
        if w < 40 {
            let k = w as f32 / 40.0;
            let p = ((fx + face * 5) as f32 + (w as f32 * 0.3).sin() * 1.5 + face as f32 * k * 4.0, (fy - 8) as f32 - k * 14.0);
            let m = Mote { i: i as u32, at: p, vel: (0.0, -1.0), k: k * 0.7, seed: i as u32 };
            pfx::mote(pen, &m, Shape::Note, ramps::HOLY, 1, run.tick);
        }
    }
}

/// The gnome's mug going up: a glint off it and its foam slopping over,
/// every time he cheers (`garden::GNOME_CHEERS`).
fn cheers(pen: &mut Pen, run: &Run) {
    let beat = (run.tick % 72) as f32;
    if beat >= 14.0 {
        return;
    }
    let (gx, gy) = super::px(g::GNOME);
    let mug = ((gx + 4) as f32, (gy - 17) as f32);
    pen.twinkle((mug.0 + 2.0, mug.1 - 2.0), 2.5, beat / 10.0, ramps::HOLY);
    let foam = Spray { seed: (run.tick / 72) as u32 ^ 0xf0a, n: if pen.calm { 3 } else { 6 }, at: mug, angle: -PI / 2.0 + 0.4, spread: 1.6, speed: (0.8, 1.8), life: (8.0, 13.0), drag: 0.05, gravity: 0.2, squash: 1.0, stagger: 0.0 };
    foam.each(beat, |m| pfx::mote(pen, &m, Shape::Droplet, ramps::BONE, 1, run.tick));
}

/// The party starting: confetti thrown up off the gnome and a ring of
/// gold opening on the grass round him.
fn party_starts(pen: &mut Pen, run: &Run) {
    let Some((age, _, line)) = super::said(run, 60) else { return };
    if !line.starts_with("STAY A WHILE") {
        return;
    }
    let a = age as f32;
    let (gx, gy) = super::px(g::GNOME);
    if a < 18.0 {
        pen.ring((gx as f32, gy as f32 + 1.0), 4.0 + ease::out(a / 18.0) * 30.0, 1.6, 0.42, ramps::GOLD, a / 18.0);
        pfx::rays(pen, ((gx + 6) as f32, (gy - 18) as f32), 10, 14.0, 0.9, 0.3, ramps::GOLD, a / 14.0);
    }
    confetti(pen, run.tick, (run.tick as u32).wrapping_sub(age as u32), ((gx + 4) as f32, (gy - 18) as f32), a, if pen.calm { 18 } else { 40 });
}

/// Confetti thrown from `at`, `age` ticks ago, seeded by `seed`: bits of
/// every party colour, tumbling down.
fn confetti(pen: &mut Pen, tick: u64, seed: u32, at: (f32, f32), age: f32, n: u32) {
    let spray = Spray { seed, n, at, angle: -PI / 2.0, spread: 2.6, speed: (1.8, 4.2), life: (34.0, 56.0), drag: 0.06, gravity: 0.08, squash: 1.0, stagger: 0.15 };
    spray.each(age, |m| {
        if m.k > 0.75 && bayer(m.at.0 as i32, m.at.1 as i32) < (m.k - 0.75) / 0.25 {
            return;
        }
        pfx::mote(pen, &m, Shape::Petal, PARTY[m.i as usize % PARTY.len()], 1, tick);
    });
}

/// A guest got in: splinters and crumbs off the hatch, a red ring of the
/// bite, and its word over the hatch.
fn bite(pen: &mut Pen, run: &Run, gd: &g::Garden, legible: bool) {
    let Some((when, _)) = gd.bitten else { return };
    let age = run.tick.saturating_sub(when);
    if age >= 34 {
        return;
    }
    let a = age as f32;
    let (hx, hy) = super::hatch();
    let at = (hx as f32, (hy - 1) as f32);
    let s = (when as u32) ^ 0xb17e;
    let bits = Spray { seed: s, n: if pen.calm { 5 } else { 11 }, at, angle: -PI / 2.0, spread: 2.6, speed: (1.4, 3.4), life: (10.0, 18.0), drag: 0.08, gravity: 0.22, squash: 0.9, stagger: 0.0 };
    bits.each(a, |m| pfx::mote(pen, &m, if m.i % 3 == 0 { Shape::Dot } else { Shape::Shard }, ramps::EARTH, 1, run.tick));
    if a < 10.0 {
        pen.ring(at, 3.0 + a * 1.8, 1.2, 0.55, ramps::BLOOD, a / 10.0);
    }
    if legible && a < 30.0 {
        let word = ["NOM!", "CHOMP!", "MUNCH!", "NOM NOM!"][(hash(when as i32, 0, 0x404) % 4) as usize];
        let rise = ease::out(a / 30.0) * 10.0;
        let pop = if a < 6.0 { (ease::spring(a / 6.0) * 3.0) as i32 - 3 } else { 0 };
        let fill = if (age / 3) % 2 == 0 { '6' } else { '@' };
        outlined(pen.cv, word, (hx, hy - 20 - rise as i32 - pop), fill, 'k');
    }
}

/// The house's bar over its roof while the door is held: a little house,
/// then a pip for every bite it has left in it, green, then gold, then red
/// and blinking. The pip a bite just took flashes as it goes, and the bar
/// jolts.
fn house_bar(cv: &mut Img, run: &Run, gd: &g::Garden) {
    let (x, y, w, _) = super::px_rect(g::HOUSE);
    let since = gd.bitten.map_or(u64::MAX, |(t, _)| run.tick.saturating_sub(t));
    let jolt = if since < 8 { [2, -2, 1, -1, 1, 0, -1, 0][since as usize] } else { 0 };
    let (bx, by, bw, bh) = (x + 10 + jolt, y - 13, w - 20, 11);
    chrome::panel(cv, (bx, by, bw, bh), chrome::Tone::HEALTH);
    // The little house.
    let (ix, iy) = (bx + 3, by + 2);
    for (dy, row) in ["..7..", ".777.", "77777", ".iTi.", ".ipi."].iter().enumerate() {
        for (dx, c) in row.chars().enumerate() {
            if c != '.' {
                cv.put(ix + dx as i32, iy + dy as i32, c);
            }
        }
    }
    let pips = (g::HOUSE_HP / g::BITE) as i32;
    let left = gd.house.div_ceil(g::BITE) as i32;
    let pw = (bw - 13) / pips;
    let x0 = bx + 10;
    let blink = (run.tick / 5).is_multiple_of(2);
    let (top, body) = if gd.house * 2 > g::HOUSE_HP {
        ('Y', 'y')
    } else if gd.house * 4 > g::HOUSE_HP {
        ('5', '4')
    } else if blink {
        ('@', '7')
    } else {
        ('7', '8')
    };
    for i in 0..pips {
        let (px, py) = (x0 + i * pw, by + 3);
        if i < left {
            cv.rect(px, py, pw - 1, 5, body);
            cv.rect(px, py, pw - 1, 1, top);
        } else if i == left && since < 12 {
            cv.rect(px, py, pw - 1, 5, if since < 4 { 'w' } else { '7' });
        } else {
            cv.rect(px, py, pw - 1, 5, 'K');
            cv.put(px + pw / 2 - 1, py + 2, 'X');
        }
    }
}

/// The house held: rockets up off the lawn bursting in every party
/// colour, confetti over the cottage, and the door lit, before the
/// fairies come out with their wish (`magic`).
fn victory(pen: &mut Pen, run: &Run) {
    let Some(age) = super::ended(run, Party::Held).filter(|&a| a < 90) else { return };
    let a = age as f32;
    let (x, y, w, _) = super::px_rect(g::HOUSE);
    let (hx, hy) = super::hatch();
    if a < 30.0 {
        pen.glow((hx as f32, (hy - 8) as f32), 24.0, 0.7, ramps::HOLY, 1.0 - a / 30.0);
    }
    // Up off the lawn, bursting over the garden.
    let rockets = [((70, 178), 0.0, (104, 96)), ((314, 178), 5.0, (284, 92)), ((192, 186), 10.0, (192, 132)), ((122, 182), 15.0, (140, 128)), ((262, 182), 20.0, (244, 124))];
    for (k, &((fx, fy), delay, (tx, ty))) in rockets.iter().enumerate() {
        let t = a - delay;
        if t < 0.0 {
            continue;
        }
        let ramp = PARTY[k];
        let from = (fx as f32, fy as f32);
        let top = (tx as f32, ty as f32);
        if t < 10.0 {
            // Up it goes, a spark trailing.
            let k = ease::out(t / 10.0);
            let p = (from.0 + (top.0 - from.0) * k, from.1 + (top.1 - from.1) * k);
            pen.streak(p, (top.0 - from.0, top.1 - from.1), 6.0, ramps::EMBER, 0.2);
            continue;
        }
        let b = t - 10.0;
        if b < 6.0 {
            pen.glow(top, 9.0, 1.0, ramp, 1.0 - b / 6.0);
        }
        pfx::rays(pen, top, 12, 15.0, 1.0, k as f32, ramp, b / 16.0);
        pen.ring(top, 3.0 + ease::out(b / 18.0) * 18.0, 1.2, 1.0, ramp, b / 22.0);
        let stars = Spray { seed: 0xf1e + k as u32 * 77, n: if pen.calm { 8 } else { 18 }, at: top, angle: 0.0, spread: TAU, speed: (1.0, 2.6), life: (16.0, 28.0), drag: 0.07, gravity: 0.06, squash: 1.0, stagger: 0.0 };
        stars.each(b, |m| pfx::mote(pen, &m, Shape::Star, ramp, 1, run.tick));
    }
    confetti(pen, run.tick, 0xc0ff, ((x + w / 2) as f32, (y + 14) as f32), a, if pen.calm { 20 } else { 48 });
}

/// The house fell: a small grey cloud comes over the gnome and rains on
/// him, for as long as the night is lost.
fn gloom(pen: &mut Pen, run: &Run) {
    let (gx, gy) = super::gnome_spot(Party::Lost);
    let age = super::ended(run, Party::Lost).map_or(60.0, |a| a as f32);
    let come = ease::out((age / 30.0).min(1.0));
    let t = run.tick as f32;
    let c = (gx as f32 + (t * 0.04).sin() * 1.5, (gy - 27) as f32 - (1.0 - come) * 20.0);
    for (dx, dy, r) in [(-5.0, 1.0, 3.2), (4.0, 1.0, 3.6), (0.0, -1.5, 4.2), (7.0, 2.0, 2.4), (-8.0, 2.0, 2.2)] {
        pen.cv.ellipse(c.0 + dx, c.1 + dy, r + 1.0, (r + 1.0) * 0.75, 'g');
    }
    for (dx, dy, r) in [(-5.0, 1.0, 3.2), (4.0, 1.0, 3.6), (0.0, -1.5, 4.2), (7.0, 2.0, 2.4), (-8.0, 2.0, 2.2)] {
        pen.cv.ellipse(c.0 + dx, c.1 + dy, r, r * 0.75, 'J');
        pen.cv.ellipse(c.0 + dx - 0.8, c.1 + dy - 0.8, r * 0.6, r * 0.4, 'h');
    }
    pen.cv.line((c.0 - 9.0) as i32, (c.1 + 3.0) as i32, (c.0 + 9.0) as i32, (c.1 + 3.0) as i32, 'G');
    if come < 1.0 {
        return;
    }
    for k in 0..6u32 {
        let n = run.tick as u32 + k * 4;
        let a = (n % 18) as f32;
        let s = hash(k as i32, (n / 18) as i32, 0x4a1);
        let p = (c.0 - 7.0 + (s % 15) as f32, c.1 + 5.0 + a * 0.55);
        let m = Mote { i: k, at: p, vel: (0.0, 1.0), k: a / 18.0, seed: s };
        pfx::mote(pen, &m, Shape::Droplet, ramps::WATER, 1, run.tick);
    }
}

// ─── a knight's moment ───────────────────────────────────────────────────────

/// Herbs and spices, tied in a bunch.
const HERB_ICON: [&str; 9] = ["...y.A...", "..yAyMy..", ".yAYAyMyA", "AyMyAYAy.", ".MyAyMyA.", "..rtttr..", "...lml...", "..l.m.l..", ".l..m..l."];
/// A mushroom from the bed.
const SHROOM_ICON: [&str; 9] = ["...qqqq...", ".qqzWzzqq.", "qzzzzzWzzq", "qzWzzzzzWq", "qqzzzWzzqq", ".qqqqqqqq.", "....HiH...", "....HiH...", "...HHHHH.."];

/// A knight who just took the herbs or the mushrooms: a puff and a swirl
/// about their head, and over it, in a little thought of its own, a
/// picture of what they took. The herbs make them sneeze; the mushrooms
/// go round them in a helix as the room starts to turn.
fn moments(pen: &mut Pen, run: &Run, legible: bool) {
    let Some((age, who, line)) = super::said(run, 76) else { return };
    let herbs = line.starts_with("LOCALLY GROWN");
    if !herbs && !line.starts_with("FOR THE ADVENTUROUS") {
        return;
    }
    let Some(hero) = run.players.get(&who) else { return };
    let (x, y) = at(hero.x, hero.y);
    let head = (x as f32, (y - 17) as f32);
    let a = age as f32;
    let s = who.wrapping_mul(0x9e37) ^ (run.tick as u32).wrapping_sub(age as u32);
    let calm = pen.calm;
    let few = |n: u32| if calm { n / 2 } else { n };
    if herbs {
        let puff = Spray { stagger: 0.0, drag: 0.1, ..Spray::round(s, few(10), (head.0, head.1 + 4.0), (0.6, 1.6), (12.0, 22.0)) };
        puff.each(a, |m| pfx::mote(pen, &m, Shape::Smoke, ramps::NATURE, 2, run.tick));
        let spice = Spray { seed: s ^ 0x5b1, n: few(24), at: head, angle: -PI / 2.0, spread: 2.8, speed: (0.8, 2.2), life: (24.0, 44.0), drag: 0.06, gravity: -0.012, squash: 1.0, stagger: 0.4 };
        spice.each(a, |m| {
            let ramp = [ramps::EMBER, ramps::GOLD, ramps::NATURE, ramps::FIRE][m.i as usize % 4];
            pfx::mote(pen, &m, if m.i % 5 == 0 { Shape::Petal } else { Shape::Dot }, ramp, 2, run.tick);
        });
        // And then the spice gets them.
        if (24.0..48.0).contains(&a) {
            let sneeze = Spray { seed: s ^ 0xac, n: few(12), at: (head.0 + 4.0, head.1 + 4.0), angle: 0.25, spread: 1.1, speed: (1.8, 3.4), life: (8.0, 15.0), drag: 0.1, gravity: 0.1, squash: 1.0, stagger: 0.0 };
            sneeze.each(a - 24.0, |m| pfx::mote(pen, &m, Shape::Dot, if m.i % 2 == 0 { ramps::GOLD } else { ramps::EMBER }, 1, run.tick));
            if legible {
                let jig = if a < 32.0 { ((a as i32) % 2) * 2 - 1 } else { 0 };
                let k = ease::back(((a - 24.0) / 6.0).min(1.0));
                outlined(pen.cv, "ACHOO!", (x + jig, y - 50 - (k * 4.0) as i32), 'T', 'k');
            }
        }
    } else {
        if a < 9.0 {
            pen.glow(head, 13.0, 1.0, ramps::ARCANE, 1.0 - a / 9.0);
        }
        let n = few(18);
        for i in 0..n {
            let ang = a * 0.24 + i as f32 * TAU / n as f32;
            let r = (13.0 - a * 0.1).max(5.0);
            let rise = head.1 + 14.0 - a * 0.45 - (i % 4) as f32 * 2.0;
            let p = (head.0 + ang.cos() * r, rise + ang.sin() * r * 0.35);
            let phase = ((a - i as f32 * 0.7) / 46.0).clamp(0.0, 1.0);
            let ramp = [ramps::ARCANE, ramps::ROSE, ramps::NATURE][i as usize % 3];
            if i % 3 == 0 {
                pen.twinkle(p, 1.6, phase, ramp);
            } else if phase > 0.0 && phase < 1.0 {
                pfx::mote(pen, &Mote { i, at: p, vel: (-ang.sin(), ang.cos() * 0.35), k: phase, seed: i }, Shape::Spark, ramp, 1, run.tick);
            }
        }
        if legible && (22.0..66.0).contains(&a) {
            let word = "WHOA...";
            let mut cx = x + 10;
            for (i, ch) in word.chars().enumerate() {
                let bob = ((a * 0.3 + i as f32 * 0.9).sin() * 2.0).round() as i32;
                let mut glyph = [0u8; 4];
                cx = outlined_at(pen.cv, ch.encode_utf8(&mut glyph), (cx, y - 40 + bob), ['3', '2', 'w'][i % 3], 'k');
            }
        }
    }
    let icon: &[&str] = if herbs { &HERB_ICON } else { &SHROOM_ICON };
    let sway = if herbs { 0.0 } else { (a * 0.35).sin() * 1.5 };
    let up = ease::back((a / 10.0).min(1.0)) * 6.0 + a * 0.05;
    let fade = ((a - 50.0) / 24.0).clamp(0.0, 1.0);
    thought(pen.cv, &Img::from_rows(icon), ((head.0 + sway).round() as i32, (head.1 - 3.0 - up).round() as i32), fade);
}

/// `icon` in a little round thought, its tail down to `(x, y)`, faded
/// away through the dither by `fade` (0 whole, 1 gone).
fn thought(cv: &mut Img, icon: &Img, (x, y): (i32, i32), fade: f32) {
    if fade >= 1.0 {
        return;
    }
    let (w, h) = (icon.w + 6, icon.h + 5);
    let mut bub = Img::new(w, h + 4);
    chrome::inset_panel(&mut bub, (0, 0, w, h), chrome::Tone::PAINTED.rimmed([ink('b').unwrap_or_default(); 3]));
    bub.stamp(icon, 3, 3);
    for (dx, dy, c) in [(w / 2 - 1, h, 'b'), (w / 2, h, 'T'), (w / 2 + 1, h, 'b'), (w / 2 - 1, h + 1, 'b'), (w / 2, h + 1, 'b'), (w / 2 - 2, h + 3, 'b')] {
        bub.put(dx, dy, c);
    }
    let (ox, oy) = (x - w / 2, y - bub.h);
    for py in 0..bub.h {
        for px in 0..bub.w {
            if let Some(c) = bub.get(px, py)
                && bayer(ox + px, oy + py) >= fade
            {
                cv.set(ox + px, oy + py, c);
            }
        }
    }
}

// ─── words ───────────────────────────────────────────────────────────────────

/// `text` centred on `x` from row `y`, in `fill` with a rim of `rim` all
/// round, so it reads on anything.
fn outlined(cv: &mut Img, text: &str, (x, y): (i32, i32), fill: char, rim: char) {
    outlined_at(cv, text, (x - text_width(text) / 2, y), fill, rim);
}

/// As `outlined`, from its left edge; returns where the next would go.
fn outlined_at(cv: &mut Img, text: &str, (x, y): (i32, i32), fill: char, rim: char) -> i32 {
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
        cv.text(x + dx, y + dy, text, rim);
    }
    cv.text(x, y, text, fill)
}

/// `text` at twice its size centred on `x`, its top at `y`: each letter
/// in `fill` over a drop of `shade`, raised or sunk by `drop(i)`.
fn big(cv: &mut Img, text: &str, (x, y): (i32, i32), (fill, shade): (char, char), drop: impl Fn(usize) -> i32) {
    let mut cx = x - text_width(text);
    let mut letter = Img::new(6, 8);
    for (i, ch) in text.chars().enumerate() {
        let mut glyph = [0u8; 4];
        let s = ch.encode_utf8(&mut glyph);
        letter.clear_all();
        letter.text(1, 1, s, shade);
        cv.stamp_twice(&letter, cx, y + drop(i));
        letter.clear_all();
        letter.text(0, 0, s, fill);
        cv.stamp_twice(&letter, cx, y + drop(i));
        cx += GLYPH_ADVANCE * 2;
    }
}

/// Who comes through the gates in wave `w`.
fn who_comes(w: usize) -> String {
    use crate::rules::EnemyKind as K;
    g::WAVES[w]
        .iter()
        .map(|&(kind, n)| match kind {
            K::Leech => format!("{n} CATERPILLARS"),
            K::Biter => format!("{n} WORMS"),
            K::Spiderling => format!("{n} BEETLES"),
            K::Silkmother => "THE MOTH QUEEN".to_string(),
            _ => format!("{n} MORE"),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// The row the garden's banners stand on: low over the lawn, under the
/// knights' feet as they mostly stand, clear of the house and its roof.
const BANNER: i32 = 154;

/// The banner low over the lawn: each wave as it comes in, the house
/// held, the party over.
fn banner(cv: &mut Img, run: &Run, party: Party) {
    if let Some((age, _, line)) = super::said(run, 84)
        && let Some(w) = g::WAVE_LINES.iter().position(|l| *l == line)
    {
        let title = format!("WAVE {} OF {}", w + 1, g::WAVES.len());
        sign(cv, age as f32, 84.0, BANNER, &title, &who_comes(w), ('6', 'a'), chrome::Tone::FRAMED, false);
        return;
    }
    match party {
        Party::Held => {
            if let Some(age) = super::ended(run, party).filter(|&a| a < 110) {
                sign(cv, age as f32, 110.0, BANNER, "THE HOUSE HELD!", "HERE COME THE FAIRIES...", ('5', 'a'), chrome::Tone::FRAMED, false);
            }
        }
        Party::Lost => {
            if let Some(age) = super::ended(run, party).filter(|&a| a < 130) {
                sign(cv, age as f32, 130.0, BANNER, "THE PARTY'S OVER", "THEY GOT INTO THE CELLAR", ('h', 'g'), chrome::Tone::SLATE, true);
            }
        }
        _ => {}
    }
}

/// A banner `age` ticks into its `life`, its top at row `y`: in from the
/// left past its mark and back, held, and away to the right. A sad one
/// does not leave; its letters sag one after the next instead.
#[allow(clippy::too_many_arguments)]
fn sign(cv: &mut Img, a: f32, life: f32, y: i32, title: &str, sub: &str, inks: (char, char), tone: chrome::Tone, sad: bool) {
    let slide = if a < 14.0 {
        (1.0 - ease::back(a / 14.0)) * -300.0
    } else if !sad && a > life - 12.0 {
        ease::inward((a - (life - 12.0)) / 12.0) * 300.0
    } else {
        0.0
    };
    let (tw, sw) = (text_width(title) * 2, text_width(sub));
    let (w, h) = (tw.max(sw) + 18, if sad { 36 } else { 31 });
    let x = 192 - w / 2 + slide as i32;
    chrome::panel(cv, (x, y, w, h), tone);
    big(cv, title, (x + w / 2, y + 4), inks, |i| {
        if sad {
            let k = ((a - 16.0 - i as f32 * 3.0) / 18.0).clamp(0.0, 1.0);
            (ease::out(k) * (2.0 + (i % 3) as f32)).round() as i32
        } else if a < 24.0 {
            // The letters land one after the next.
            let k = ((a - i as f32 * 0.8) / 8.0).clamp(0.0, 1.0);
            ((1.0 - ease::spring(k)) * -4.0).round() as i32
        } else {
            0
        }
    });
    cv.text(x + (w - sw) / 2, y + if sad { 26 } else { 21 }, sub, 'h');
}

/// What the gnome says, from the room's line: his own words without the
/// numbers (the board has those), and, for the fairies' line, his cheer.
fn gnome_says(line: &str) -> Option<&str> {
    if line.chars().any(|c| c.is_ascii_lowercase()) {
        return None;
    }
    if line == g::HELD_LINE {
        return Some("THE CELLAR'S SAFE! PARTY ON!");
    }
    if line == g::LOST_LINE {
        return Some("MY CELLAR... MY LOVELY CELLAR...");
    }
    let line = line.strip_prefix("HE SAYS: ").unwrap_or(line);
    Some(match line.find(". +") {
        Some(i) => &line[..=i],
        None => line,
    })
}

/// `text` broken at its spaces into rows of no more than `most` letters,
/// three at most.
fn wrap(text: &str, most: usize) -> Vec<&str> {
    let mut rows = Vec::with_capacity(3);
    let (mut start, mut end) = (0, 0);
    for (i, _) in text.match_indices(' ').chain(std::iter::once((text.len(), ""))) {
        if i - start > most && end > start {
            rows.push(&text[start..end]);
            start = end + 1;
        }
        end = i;
    }
    rows.push(&text[start..]);
    rows.truncate(3);
    rows
}

/// What the gnome just said, in a bubble off to one side of him, its tail
/// to his hat, for three seconds.
fn bubble(cv: &mut Img, run: &Run) {
    let Some((age, who, line)) = super::said(run, u64::from(HZ) * 3) else { return };
    let Some(words) = gnome_says(line) else { return };
    let rows = wrap(words, 20);
    let w = rows.iter().map(|r| text_width(r)).max().unwrap_or(0) + 8;
    let h = rows.len() as i32 * 9 + 4;
    let (gx, gy) = super::gnome_spot(super::here(run).map_or(Party::Quiet, |gd| gd.party));
    let pop = if age < 4 { 2 - age as i32 / 2 } else { 0 };
    // Off to the side away from the knight it was said to, so it covers
    // neither of them.
    let left = run.players.get(&who).is_some_and(|h| at(h.x, h.y).0 > gx);
    let x0 = if left { (gx - 6 - w).max(3) } else { (gx + 6).min(cv.w - w - 3) };
    let y0 = gy - 26 - h + pop;
    let rim = ink('b').unwrap_or_default();
    chrome::inset_panel(cv, (x0, y0, w, h), chrome::Tone::PAINTED.rimmed([rim; 3]));
    for (i, row) in rows.iter().enumerate() {
        cv.text(x0 + 4, y0 + 2 + i as i32 * 9, row, 'b');
    }
    let (tail, way) = if left { (x0 + w - 5, 1) } else { (x0 + 4, -1) };
    for k in 0..5 {
        cv.put(tail + way * (k / 2), y0 + h + k, 'b');
        cv.put(tail + 1 + way * (k / 2), y0 + h + k, 'T');
    }
}

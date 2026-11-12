//! Cinder Stack's screen, as a seat plays it: the kiln room (Crock, the well and its pots) on a layer
//! that shakes with the blows, and over it, still, the corner, the rivals' little boards, the
//! meters, the words and the moments. It reads the board and its marks and keeps nothing.
use super::super::super::ink::{Img, bayer};
use super::super::juice;
use super::super::{NATIVE_H, NATIVE_W};
use super::{BX, BY, CH, CW, art, finish, fx, hud, knight, landing, moments, pot, room};
use crate::rules::Run;
use crate::rules::cinder::{Board, COLS, ROWS, Session};
use crate::rules::cinder_show::Kind;
use std::sync::OnceLock;

/// Both layers of seat `focus`'s screen (or the first seat's): the picture, and an empty one over it.
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let s = run.cinder.as_ref().expect("Cinder showing");
    let id = focus.filter(|id| s.boards.contains_key(id)).or_else(|| s.boards.keys().next().copied()).unwrap();
    let ended = if s.left == 0 { (run.tick as u32).saturating_sub(s.done) } else { 0 };
    let hold = finish::held(s);
    let picture = if s.left == 0 && ended >= hold + finish::WIPE {
        finish::scene(run, s, id, ended - hold - finish::WIPE)
    } else {
        let mut im = play(run, s, id, ended);
        if s.left == 0 && ended >= hold {
            juice::iris(&mut im, (192, 112), (ended - hold) as f32 / finish::WIPE as f32, 'k');
        }
        im
    };
    (picture, Img::new(NATIVE_W, NATIVE_H))
}

/// A sprite in the white of a blow landing: every lit pixel in a pale ink by its own light, so the
/// frame has the figure's form and not a blob, and its black outline stays.
fn flash_frame(src: &Img) -> Img {
    let mut out = src.clone();
    for y in 0..src.h {
        for x in 0..src.w {
            if let Some([r, g, b]) = src.get(x, y).filter(|&c| c != super::super::super::ink::BLACK) {
                let light = (u32::from(r) * 3 + u32::from(g) * 6 + u32::from(b)) / 10;
                out.put(x, y, if light > 150 { 'H' } else if light > 80 { 'i' } else { 'h' });
            }
        }
    }
    out
}

/// The pots in white, for the frame a blow lands on them.
fn pot_white(c: u8) -> &'static Img {
    static BANKS: [OnceLock<[Img; 4]>; 2] = [OnceLock::new(), OnceLock::new()];
    let bank = BANKS[usize::from(super::super::slot_art::enabled())].get_or_init(|| {
        std::array::from_fn(|i| {
            let mut im = Img::new(CW, CH);
            pot(&mut im, 0, 0, i as u8 + 1, false);
            flash_frame(&im)
        })
    });
    &bank[usize::from(c.clamp(1, 4) - 1)]
}

fn pot_flat(c: u8) -> &'static Img {
    static BANKS: [OnceLock<[Img; 4]>; 2] = [OnceLock::new(), OnceLock::new()];
    let bank = BANKS[usize::from(super::super::slot_art::enabled())].get_or_init(|| {
        std::array::from_fn(|i| {
            let mut im = Img::new(CW, CH);
            pot(&mut im, 0, 0, i as u8 + 1, false);
            im
        })
    });
    &bank[usize::from(c.clamp(1, 4) - 1)]
}

/// A pot squashed wide and low (`k` 0 to 1), its feet on the floor of its cell.
fn squashed(src: &Img, k: f32) -> Img {
    let (w, h) = (src.w + (k * 5.0) as i32, src.h - (k * 6.0) as i32);
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = src.get(x * src.w / w, y * src.h / h) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// How far a landed pair is squashed `age` ticks after it came to rest.
fn squash_k(age: u32, hard: bool) -> f32 {
    [0.9, 0.7, 0.45, 0.25, 0.1][age.min(4) as usize] * if hard { 1.0 } else { 0.55 }
}

/// The world's shake: the heaviest blow's, and a one-pixel thump under a dropped pair.
fn shake(b: &Board, bclock: u32, tick: u64) -> (i32, i32) {
    if crate::render::viewer::calm() {
        return (0, 0);
    }
    let (mut dx, mut dy) = juice::shake(b.show.marks.iter().map(|m| (m.kind.weight(), m.age(bclock))), tick);
    // The smallest pop still shakes a little: the kit's graze is none.
    if (dx, dy) == (0, 0) {
        if let Some(age) = b.show.marks.iter().filter(|m| matches!(m.kind, Kind::Pop { .. }) && m.age(bclock) < 3).map(|m| m.age(bclock)).min() {
            (dx, dy) = crate::motion::shake(tick, 3 - age, 3, 1.6);
        }
    }
    if b.show.marks.iter().any(|m| matches!(m.kind, Kind::Land { hard: true, .. }) && m.age(bclock) < 3) {
        dy += 1;
    }
    (dx, dy)
}

fn crock(world: &mut Img, b: &Board, bclock: u32, tick: u64, ended: u32) {
    let a = art();
    let stage = b.crack_stage();
    let bob = i32::from(tick % 36 < 18);
    // A board that has stopped carries on counting its pop down, for the picture.
    let flash = b.flash.saturating_sub(ended);
    let figure = if flash > 0 {
        &a.flinch[stage][usize::from(flash <= 4)]
    } else if b.tell > 0 {
        &a.windup[stage][usize::from(b.tell <= b.windup / 2)]
    } else {
        &a.crock[stage]
    };
    // The frame the fed pots land, and a stage cracking, in white.
    let struck = b.show.marks.iter().any(|m| match m.kind {
        Kind::Pop { .. } => m.age(bclock).checked_sub(8).is_some_and(|a| a < 2),
        Kind::Stage { .. } => m.age(bclock) < 4,
        _ => false,
    });
    let white;
    let figure = if struck {
        white = flash_frame(figure);
        &white
    } else {
        figure
    };
    if b.tell > 0 {
        world.stamp(&a.tell, 282, 140);
    }
    let reaction = if flash > 0 && super::super::slot_art::enabled() {
        (if flash % 2 == 0 { -1 } else { 1 }) * (stage as i32 + 1)
    } else {
        0
    };
    world.stamp(figure, 310 - figure.w / 2 + reaction, 158 - figure.h + bob);
}

/// The well: the slag's column filling with its warning, every pot (the ones that popped white for a
/// frame, then flying to the kiln), the ghost of where the pair will stop, the pair, and the slag
/// coming down.
fn board(world: &mut Img, run: &Run, s: &Session, b: &Board, bclock: u32, ended: u32) {
    let a = art();
    let tick = run.tick;
    let flash = b.flash.saturating_sub(ended);
    if b.tell > 0 && b.retry == 0 {
        let prog = 1.0 - b.tell as f32 / b.windup.max(1) as f32;
        let x0 = BX + i32::from(b.column) * CW;
        for y in BY..BY + ROWS as i32 * CH {
            for x in x0..x0 + CW {
                if bayer(x, y) < prog * 0.4 {
                    world.put(x, y, 'p');
                }
            }
        }
    }
    let mut white: Vec<(usize, usize)> = Vec::new();
    let mut landed: Vec<(usize, usize, u32, bool)> = Vec::new();
    for m in &b.show.marks {
        let age = m.age(bclock);
        match m.kind {
            Kind::Slag { col, row, .. } if age < 2 => white.push((usize::from(col), usize::from(row))),
            Kind::Land { a, b: other, hard, .. } if age < 5 => {
                landed.push((usize::from(a.0), usize::from(a.1), age, hard));
                landed.push((usize::from(other.0), usize::from(other.1), age, hard));
            }
            _ => {}
        }
    }
    for y in 0..ROWS {
        for x in 0..COLS {
            let c = b.cells[y][x];
            if c == 0 {
                continue;
            }
            let (px, py) = hud::cell(x, y);
            if b.pop[y][x] && b.flash > 0 && flash == 0 {
                // The pop has run its course while the board stood still: they are gone.
                continue;
            }
            if b.pop[y][x] && flash > 0 {
                let age = 8 - flash as i32;
                world.stamp(&a.clear[(age / 3) as usize], px, py - 2);
                if age < 2 {
                    // Two ticks of white where they stood: the blow lands.
                    world.stamp(pot_white(c), px, py);
                } else {
                    let t = age - 2;
                    let lift = [2, 6, 4][x % 3] * t * (6 - t) * (py - BY) / (ROWS as i32 * CH);
                    for back in (0..3).rev() {
                        let u = (t - back).max(0);
                        let (tx, ty) = ((px * (6 - u) + 288 * u) / 6, (py * (6 - u) + 98 * u) / 6 - [2, 6, 4][x % 3] * u * (6 - u) * (py - BY) / (ROWS as i32 * CH));
                        if back > 0 {
                            world.put(tx + CW / 2, ty + CH / 2, if back == 1 { 'o' } else { 'R' });
                        }
                    }
                    let (tx, ty) = ((px * (6 - t) + 288 * t) / 6, (py * (6 - t) + 98 * t) / 6 - lift);
                    pot(world, tx, ty, c, false);
                }
            } else if white.contains(&(x, y)) {
                world.stamp(pot_white(c), px, py);
            } else if let Some(&(_, _, age, hard)) = landed.iter().find(|l| l.0 == x && l.1 == y) {
                let sq = squashed(pot_flat(c), squash_k(age, hard));
                world.stamp(&sq, px - (sq.w - CW) / 2, py + CH - sq.h);
            } else {
                pot(world, px, py, c, false);
            }
        }
    }
    if flash == 0 && b.retry == 0 && s.left > 0 {
        let colours = b.colours();
        for (n, (x, y)) in Board::pair_at(b.x, b.ghost(), b.turn).into_iter().enumerate() {
            pot(world, BX + i32::from(x) * CW, BY + i32::from(y) * CH, colours[n], true);
        }
        for (n, (x, y)) in Board::pair_at(b.x, b.y, b.turn).into_iter().enumerate() {
            pot(world, BX + i32::from(x) * CW, BY + i32::from(y) * CH, colours[n], false);
        }
    }
    if b.tell > 0 {
        let (x, y) = (BX + i32::from(b.column) * CW, BY + landing(b) * CH);
        // Continuous boundaries never blink; the exact landing cell is solidly marked.
        world.line(x, BY, x, BY + ROWS as i32 * CH - 1, 'o');
        world.line(x + CW - 1, BY, x + CW - 1, BY + ROWS as i32 * CH - 1, 'o');
        world.frame(x + 1, y + 1, CW - 2, CH - 2, 't');
        if b.tell <= 8 && tick / 2 % 2 == 0 {
            world.frame(x - 1, y - 1, CW + 2, CH + 2, 'H');
        }
        // A warning mark over the head of the column for the last half second.
        if b.tell <= 15 && tick / 3 % 2 == 0 {
            let cx = x + CW / 2;
            world.rect(cx - 3, BY + 2, 7, 15, 'k');
            world.rect(cx - 1, BY + 3, 3, 8, 'T');
            world.rect(cx - 1, BY + 13, 3, 3, 'T');
        }
        let fall_y = BY + b.windup.saturating_sub(b.tell) as i32 * (y - BY) / b.windup.max(1) as i32;
        world.stamp(&a.slag, x + 3, fall_y);
    }
    if b.impact > 0 {
        world.stamp(&a.impact[((12 - b.impact) / 3) as usize], BX + i32::from(b.column) * CW - 8, BY + i32::from(b.impact_row) * CH - 2);
    }
}

/// Seat `id`'s screen while the round is on, or in its last breath (`ended` ticks after it).
fn play(run: &Run, s: &Session, id: u32, ended: u32) -> Img {
    let b = &s.boards[&id];
    let bclock = b.clock + ended;
    let tick = run.tick;
    let mut world = room(0, false).clone();
    if super::super::slot_art::enabled() && super::super::slot_art::bytes("cinder.room", "room").is_some() {
        for (x, y, w, h) in [(0, 0, 88, 112), (250, 0, 134, 66), (251, 175, 126, 32), (0, 212, 384, 12)] {
            world.rect(x, y, w, h, 'k');
        }
    }
    fx::embers(&mut world, tick);
    crock(&mut world, b, bclock, tick, ended);
    fx::steam(&mut world, tick);
    board(&mut world, run, s, b, bclock, ended);
    for m in &b.show.marks {
        let age = m.age(bclock);
        let seed = m.born.wrapping_mul(0x9e37_79b1) ^ id.wrapping_mul(0x85eb_ca77);
        match m.kind {
            Kind::Pop { by, chain, gain } => {
                fx::pop(&mut world, &by, chain, gain, age, seed, tick);
                fx::carry(&mut world, fx::centroid(&by), gain, age);
                if (8..22).contains(&age) {
                    fx::eat(&mut world, gain, age - 8, seed, tick);
                }
            }
            Kind::Land { a, b, hard, from } => fx::land(&mut world, a, b, hard, from, age, seed, tick),
            Kind::Slag { col, row, hit, .. } => fx::slag(&mut world, col, row, hit, age, seed, tick),
            Kind::Stage { n } => fx::stage(&mut world, n, age, seed, tick),
            Kind::Cough => {}
        }
    }
    let (dx, dy) = shake(b, bclock, tick);
    let mut out = Img::black(NATIVE_W, NATIVE_H);
    out.stamp(&world, dx, dy);
    // The stack climbing to the lid, or the kiln coughing, rims the well in danger.
    let high = b.height();
    if b.retry > 0 || (high >= 8 && s.left > 0) {
        let pulse = 0.55 + 0.45 * ((tick as f32) * 0.25).sin();
        let k = if b.retry > 0 { 1.0 } else { ((high - 7) as f32 / 3.0).min(1.0) * pulse };
        juice::vignette(&mut out, (BX, BY, COLS as i32 * CW, ROWS as i32 * CH), k, 10, 'R');
    }
    // A stage of the kiln cracking lights the whole picture for a beat, in warm paper.
    let flare = b.show.marks.iter().filter_map(|m| matches!(m.kind, Kind::Stage { .. }).then(|| m.age(bclock))).filter(|&a| a < 4).min();
    if let (Some(age), false) = (flare, crate::render::viewer::calm()) {
        juice::vignette(&mut out, (0, 0, NATIVE_W, NATIVE_H), 0.8 * (1.0 - age as f32 / 4.0), 70, 'T');
    }
    // Slag that came down in your column, close by, is a near miss.
    let near = b.show.marks.iter().filter_map(|m| matches!(m.kind, Kind::Slag { near: true, hit: false, .. }).then(|| m.age(bclock))).filter(|&a| a < 8).min();
    if let Some(age) = near {
        juice::vignette(&mut out, (BX, BY, COLS as i32 * CW, ROWS as i32 * CH), 0.9 * (1.0 - age as f32 / 8.0), 8, 'O');
    }
    if let Some(k) = knight(run, id, false) {
        out.stamp(&k, hud::GUTTER_X - k.w / 2, 40 - k.h);
    }
    for (n, c) in b.next().into_iter().enumerate() {
        pot(&mut out, 21 + n as i32 * 24, 58, c, false);
    }
    hud::paint_tiles(&mut out, run, s, id, tick as u32);
    hud::paint_meters(&mut out, run, s, id, bclock, tick as u32);
    for w in hud::words(run, s, id, bclock) {
        out.text(w.x, w.y, &w.text, w.ink);
    }
    moments::paint(&mut out, run, s, id, bclock, ended);
    out
}

//! The Deep Road's meetings, drawn in the room they stand in
//! (`rules::deep::road`): a lost patrol round its cold lamp, a caravan's
//! lantern-beast with its wares laid on a cloth, a blind hermit with his
//! lamp, captives in an iron cage, a drowned shrine half under black water,
//! an envoy under their people's pennant.
//!
//! Each shows the state it is in and nothing else: a lamp burns only once
//! it has been lit, a shrine's water goes still after an offering and dark
//! after the hoard is taken, the cage stands open and empty once its
//! captives are freed. A soft pool of light goes only under what burns.
//! What a meeting says is the board's (`panel::road`).

use super::super::super::chrome;
use super::super::super::ink::{BLACK, Img, bayer, hash, text_width};
use super::super::super::kit;
use super::super::super::light::step_down;
use super::super::{TILE, at, stand};
use super::{folk, look, panel};
use crate::rules::{
    Run,
    deep::{
        Culture, Sign,
        missions::SignKind,
        road::{ENVOY_LEAVES, Face, Kind, Meeting, REACH, Stage},
    },
};

/// The meeting on the floor the party stands in, drawn over its room.
pub fn meeting(cv: &mut Img, run: &Run) {
    let Some(d) = run.deep.as_ref().filter(|d| d.village.is_none()) else {
        return;
    };
    let Some(m) = d.meeting.as_ref().filter(|m| m.room == run.at) else {
        return;
    };
    let (ux, uy) = m.point();
    let (x, y) = at(ux, uy);
    let y = y + TILE / 2 - 2;
    let tick = run.tick as u32;
    let near = run.players.values().any(|h| {
        let (hx, hy) = at(h.x, h.y);
        h.hp > 0 && (hx - x).abs() + (hy - y).abs() < 7 * TILE
    });
    let age = m.at.map(|t| run.tick.saturating_sub(t));
    let top = match m.kind {
        Kind::Patrol => patrol(cv, m, (x, y), tick, near),
        Kind::Caravan => caravan(cv, m, (x, y), tick, near),
        Kind::Hermit => hermit(cv, m, (x, y), tick, age),
        Kind::Captives => captives(cv, m, (x, y), tick, near, age),
        Kind::Shrine => shrine(cv, m, (x, y), tick),
        Kind::Envoy => envoy(cv, m, (x, y), tick, near, age, m.gone(run.tick)),
    };
    if let Some(age) = age.filter(|&a| a < 40) {
        flare(cv, (x, top), age as u32);
    }
    let quiet = !run.fight_on();
    let talkable = matches!(m.stage, Stage::Open | Stage::Settled) && !m.gone(run.tick) && quiet && m.talks.is_empty();
    if talkable && near {
        super::folk::bubble(cv, x, top - 6, tick);
    }
    // The prompt shows only where E would speak.
    let reach = REACH * super::super::UNIT;
    let by = run.players.values().any(|h| {
        let (hx, hy) = at(h.x, h.y);
        h.hp > 0 && !h.stone && ((hx - x) as f32).hypot((hy - (y - 6)) as f32) < reach
    });
    if talkable && by {
        let label = "E: SPEAK";
        let w = text_width(label);
        let (lx, ly) = ((x - w / 2).clamp(3, cv.w - w - 3), top.max(2));
        chrome::label(cv, (lx, ly), label, 'O', chrome::Tone::KEY);
    }
}

/// Sparks going up where a meeting was just settled: a thing done.
fn flare(cv: &mut Img, (x, top): (i32, i32), age: u32) {
    for k in 0..14u32 {
        let h = hash(k as i32, 3, 9601);
        let r = age as f32 * (0.5 + (h % 10) as f32 / 12.0);
        let a = (h >> 4) as f32 % 628.0 / 100.0;
        cv.put(x + (a.cos() * r) as i32, top + 8 - age as i32 / 2 + (a.sin() * r * 0.5) as i32, ['9', '6', '5'][k as usize % 3]);
    }
}

/// A soft pool of light round `(cx, cy)`, dithered: dim ink out to `r`,
/// the brighter ink within its heart. `power` is `0..=1`.
fn glow(cv: &mut Img, (cx, cy): (i32, i32), r: i32, (outer, inner): (char, char), power: f32) {
    let ry = (r * 3 / 5).max(1);
    for dy in -ry..=ry {
        for dx in -r..=r {
            let d = ((dx as f32 / r as f32).powi(2) + (dy as f32 / ry as f32).powi(2)).sqrt();
            if d >= 1.0 {
                continue;
            }
            let t = (1.0 - d) * power;
            let b = bayer(cx + dx, cy + dy);
            if t > 0.55 && b < (t - 0.45) * 1.6 {
                cv.put(cx + dx, cy + dy, inner);
            } else if b < t * 0.5 {
                cv.put(cx + dx, cy + dy, outer);
            }
        }
    }
}

/// `im` taken `n` steps down its banks: a figure in the dark.
fn dim(im: &Img, n: usize) -> Img {
    let mut out = im.clone();
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y).filter(|&c| c != BLACK) {
                out.set(x, y, step_down(c, n));
            }
        }
    }
    out
}

/// A shadow under a standing thing.
fn shadow(cv: &mut Img, (x, y): (i32, i32), rx: f32, ry: f32) {
    super::super::super::material::shadow(cv, (x as f32, y as f32), (rx, ry), &|_, _| true);
}

// ─── the lost patrol ─────────────────────────────────────────────────────────

/// Three of a people's watch round their lamp: a captain on her feet with
/// a spear, two huddled in the dark. The lamp is their own, cold till a
/// knight shares a light. Returns the top of the scene.
fn patrol(cv: &mut Img, m: &Meeting, (x, y): (i32, i32), tick: u32, near: bool) -> i32 {
    let p = m.people;
    let lit = m.choice == Some(1);
    let fighting = m.stage == Stage::Fighting;
    let glad = m.stage == Stage::Settled && m.choice != Some(2);
    let up = fighting || glad;
    let lamp_at = (x - 26, y);
    let sign = Sign { kind: SignKind::Lamp, mission: 0, room: 0, col: 0, row: 0, done: lit, at: None, culture: p };
    if lit {
        let lk = look(p);
        glow(cv, (lamp_at.0 + 8, lamp_at.1 - 6), 38, (lk.glow[0], lk.glow[1]), 0.75);
    }
    panel::lamp(cv, &sign, lamp_at, tick);
    shadow(cv, (x, y), 24.0, 4.0);
    // The two on the ground, or on their feet if they have a reason.
    for (dx, who) in [(-8, 1usize), (6, 2)] {
        let im = if up {
            folk::figure(p, who, false, true)
        } else if near && who == 2 {
            folk::lost_figure(p, folk::Lost::Waving, tick)
        } else {
            folk::lost_figure(p, folk::Lost::Huddled, tick)
        };
        let im = if m.choice == Some(2) { dim(&im, 1) } else { im };
        stand(cv, &im, x + dx, y);
    }
    // The captain, a spear in her hand.
    let cap = folk::figure(p, 0, false, false);
    let cx = x + 22;
    let sway = if fighting { ((tick / 4) % 2) as i32 } else { 0 };
    stand(cv, &cap, cx, y);
    cv.line(cx + 4, y - 6, cx + 8, y - 8, 'o');
    cv.line(cx + 8, y, cx + 8 - sway, y - 30, 'P');
    cv.line(cx + 9, y, cx + 9 - sway, y - 30, 'r');
    cv.put(cx + 8 - sway, y - 31, 'h');
    cv.put(cx + 8 - sway, y - 32, 'i');
    cv.put(cx + 8 - sway, y - 33, 'H');
    // Glad, or hurt at heart.
    if glad && !(tick / 6).is_multiple_of(3) {
        super::panel::heart(cv, cx - 3, y - cap.h - 6 - ((tick / 4) % 4) as i32, false);
    }
    y - 54
}

// ─── the caravan ─────────────────────────────────────────────────────────────

/// A trader of a people and a lantern-beast laden with odd goods, their
/// wares laid out on a cloth; a lantern on a pole over the beast's back.
fn caravan(cv: &mut Img, m: &Meeting, (x, y): (i32, i32), tick: u32, near: bool) -> i32 {
    let p = m.people;
    let lk = look(p);
    let cloth = cloth_inks(p);
    // The lantern's pool, then the shadow, the beast, the wares, the trader.
    let flick = 0.8 + 0.1 * (((tick / 3) % 4) as f32);
    glow(cv, (x - 14, y - 16), 40, ('b', 'B'), flick * 0.8);
    shadow(cv, (x - 2, y), 26.0, 4.0);
    let b = beast(tick, cloth);
    cv.stamp(&b, x - 24 - 4, y - b.h + 1);
    wares(cv, m, (x + 18, y - 1), p);
    let trader = folk::figure(p, 3, near, false);
    stand(cv, &trader, x + 38, y);
    // Their satchel.
    cv.rect(x + 31, y - 8, 4, 5, 'o');
    cv.line(x + 31, y - 9, x + 36, y - 4, 'b');
    let _ = lk;
    y - 54
}

/// A blanket's colours: stripes of the trader's people.
fn cloth_inks(p: Culture) -> (char, char) {
    let lk = look(p);
    (lk.rock[3], lk.glow[1])
}

/// The lantern-beast: a broad, shaggy pack animal on stone-hard hooves,
/// horns curling back, a lantern on a pole over its back. Original to the
/// Deep.
fn beast(tick: u32, cloth: (char, char)) -> Img {
    let mut im = Img::new(54, 40);
    let oy = 10;
    let breath = ((tick / 24) % 2) as f32 * 0.4;
    // Far legs, darker; near legs.
    for lx in [10, 33] {
        im.rect(lx, oy + 21, 4, 8, 'b');
        im.rect(lx, oy + 28, 4, 2, 'k');
    }
    for lx in [6, 15, 29, 38] {
        let near = lx == 6 || lx == 38;
        im.rect(lx, oy + 20, 4, 9, if near { 'I' } else { 'B' });
        im.line(lx, oy + 21, lx, oy + 28, if near { 'B' } else { 'r' });
        im.rect(lx - 1, oy + 28, 6, 2, 'k');
        im.line(lx - 1, oy + 28, lx + 4, oy + 28, 'X');
    }
    // The body, lit from above, shaggy at the belly.
    im.ellipse(25.0, (oy + 14) as f32, 20.0, 8.0 + breath, 'B');
    im.ellipse(24.0, (oy + 12) as f32, 18.0, 5.5 + breath, 'r');
    im.ellipse(22.0, (oy + 10) as f32, 12.0, 2.8, 'P');
    im.ellipse(25.0, (oy + 20) as f32, 17.0, 2.4, 'b');
    for k in 0..9 {
        let sx = 8 + k * 4;
        im.line(sx, oy + 20, sx - 1, oy + 24 + (k % 3), 'I');
    }
    // The neck and head, low, horns swept back and up.
    im.ellipse(42.0, (oy + 13) as f32, 6.0, 6.0, 'B');
    im.ellipse(47.0, (oy + 16) as f32, 5.5, 4.5, 'r');
    im.ellipse(46.0, (oy + 14) as f32, 3.0, 2.0, 'P');
    im.ellipse(50.0, (oy + 18) as f32, 3.0, 2.5, 'o');
    im.rect(51, oy + 17, 2, 2, 'b');
    im.rect(44, oy + 20, 6, 1, 'b');
    im.put(46, oy + 13, 'k');
    im.put(47, oy + 13, '4');
    im.rect(41, oy + 8, 3, 3, 'B');
    im.line(43, oy + 8, 40, oy + 1, 'T');
    im.line(40, oy + 1, 43, oy - 1, 't');
    im.line(46, oy + 9, 47, oy + 2, 'T');
    im.line(47, oy + 2, 50, oy + 1, 't');
    // A blanket across the back, striped in the trader's people's colours.
    im.rect(12, oy + 5, 22, 6, cloth.0);
    for k in 0..5 {
        im.rect(14 + k * 4, oy + 5, 2, 6, cloth.1);
    }
    im.line(12, oy + 11, 33, oy + 11, 'b');
    // Panniers, strapped on.
    im.rect(10, oy + 11, 10, 11, 'o');
    im.rect(10, oy + 11, 10, 2, 'O');
    im.rect(10, oy + 21, 10, 1, 'b');
    im.line(10, oy + 11, 10, oy + 21, 'r');
    im.rect(24, oy + 12, 9, 10, 'O');
    im.rect(24, oy + 12, 9, 2, 't');
    im.rect(24, oy + 21, 9, 1, 'r');
    im.line(8, oy + 8, 34, oy + 8, 'b');
    // The pole and its lantern.
    im.line(6, oy + 8, 6, 3, 'P');
    im.line(7, oy + 8, 7, 3, 'r');
    im.rect(3, 0, 7, 6, 'k');
    let flame = if (tick / 5).is_multiple_of(3) { '6' } else { '5' };
    im.rect(4, 1, 5, 4, '4');
    im.rect(5, 2, 3, 2, flame);
    im.put(6, 2, '9');
    im.outline_outside('n');
    im
}

/// The wares laid on a cloth: jars, a coil of cord, shards, a lump of the
/// people's own good; only what is still unsold.
fn wares(cv: &mut Img, m: &Meeting, (x, y): (i32, i32), p: Culture) {
    let lk = look(p);
    cv.ellipse(x as f32, (y - 1) as f32, 16.0, 3.5, 'k');
    cv.rect(x - 14, y - 4, 29, 4, 'p');
    cv.line(x - 14, y - 4, x + 14, y - 4, 'R');
    cv.line(x - 14, y - 1, x + 14, y - 1, 'b');
    for k in 0..6 {
        cv.rect(x - 12 + k * 5, y - 3, 2, 2, 'O');
    }
    // The gift: a bundle of the people's relic.
    if m.sold & 1 == 0 {
        cv.rect(x - 12, y - 10, 7, 6, 'O');
        cv.rect(x - 12, y - 10, 7, 2, 't');
        cv.line(x - 12, y - 7, x - 6, y - 7, 'b');
        cv.put(x - 9, y - 11, lk.glow[3]);
    }
    // Deep-honey, in a squat jar.
    if m.sold & 2 == 0 {
        cv.ellipse((x + 1) as f32, (y - 8) as f32, 4.0, 4.0, 'R');
        cv.ellipse(x as f32, (y - 9) as f32, 2.0, 2.0, 'O');
        cv.rect(x - 2, y - 13, 5, 2, 'P');
        cv.put(x - 1, y - 9, '6');
    }
    // A gem laid out, for the trade.
    if m.sold & 4 == 0 {
        cv.line(x + 8, y - 5, x + 12, y - 9, 'z');
        cv.line(x + 12, y - 9, x + 14, y - 5, 'Q');
        cv.line(x + 8, y - 5, x + 14, y - 5, 'q');
        cv.put(x + 11, y - 8, 'V');
    }
}

// ─── the hermit ──────────────────────────────────────────────────────────────

/// A blind hermit on a mat, in a violet robe, a cloth across his eyes, a
/// staff in his hand and a cold lamp at his knee that he lights for the
/// right answer. Returns the top of the scene.
fn hermit(cv: &mut Img, m: &Meeting, (x, y): (i32, i32), tick: u32, age: Option<u64>) -> i32 {
    let right = m.stage == Stage::Settled && m.choice == Some(m.key + 1);
    let wrong = m.stage == Stage::Settled && !right;
    shadow(cv, (x, y), 14.0, 3.5);
    let lamp_at = (x - 16, y - 1);
    if right {
        glow(cv, (lamp_at.0 + 3, lamp_at.1 - 10), 44, ('B', 'o'), 0.85 + 0.1 * (((tick / 4) % 3) as f32));
    }
    cv.stamp(&kit::lantern(right), lamp_at.0 - 3, lamp_at.1 - 14);
    if wrong && age.is_some_and(|a| a < 80) {
        // The lamp's smoke, as it dies.
        for k in 0..5u32 {
            let t = (tick + k * 9) % 36;
            cv.put(lamp_at.0 + ((t as f32 * 0.4).sin() * 2.0) as i32, lamp_at.1 - 16 - t as i32 / 2, if t < 16 { 'j' } else { 'g' });
        }
    }
    cv.stamp(&hermit_figure(tick, right), x - 12, y - 30);
    y - 44
}

fn hermit_figure(tick: u32, lit: bool) -> Img {
    let mut im = Img::new(26, 31);
    // The mat.
    im.ellipse(12.0, 28.0, 12.0, 2.5, 'b');
    im.ellipse(12.0, 27.0, 10.0, 1.8, 'I');
    // The robe: a bell of cloth, lit from the left.
    for yy in 12..28 {
        let half = 4 + (yy - 12) * 5 / 15;
        for xx in 12 - half..=12 + half {
            let ink = if xx < 12 - half / 2 { '[' } else if xx > 12 + half / 2 { '(' } else { ')' };
            im.put(xx, yy, ink);
        }
        im.put(12 - half, yy, ']');
    }
    // Folds.
    for xx in [8, 11, 15] {
        im.line(xx, 18, xx + (xx - 12) / 3, 27, '(');
    }
    // A cord at the waist.
    im.line(8, 19, 17, 19, 'b');
    im.put(15, 20, 'o');
    // The hood and the face in it.
    im.ellipse(12.0, 9.0, 6.0, 6.5, ')');
    im.ellipse(11.0, 8.0, 4.0, 4.0, '[');
    im.ellipse(12.0, 11.0, 3.6, 4.3, 'd');
    im.ellipse(12.0, 11.0, 2.6, 3.4, 'o');
    // The cloth across his eyes, knotted at the side.
    im.rect(9, 10, 7, 2, 'c');
    im.line(9, 10, 15, 10, '9');
    im.put(16, 11, 'c');
    im.put(16, 12, 'c');
    // A white beard to the chest.
    im.rect(10, 13, 5, 3, 'i');
    im.rect(11, 16, 3, 2, 'h');
    // Hands on the staff.
    im.line(19, 3, 19, 28, 'P');
    im.line(20, 3, 20, 28, 'r');
    im.rect(18, 2, 4, 2, 'O');
    im.put(19, 1, 't');
    im.rect(17, 17, 3, 3, 'o');
    // The staff's small charm, a bell, tinkling.
    if (tick / 30).is_multiple_of(2) {
        im.put(22, 5, '4');
    } else {
        im.put(22, 6, '4');
    }
    if lit {
        im.put(13, 12, 'T');
    }
    im.outline_outside('d');
    im
}

// ─── the captives ────────────────────────────────────────────────────────────

/// Three of a people in an iron cage, tired. Their captors rise from the
/// room when a knight comes near. Freed, the door stands open and they go
/// home up the stair as lights.
fn captives(cv: &mut Img, m: &Meeting, (x, y): (i32, i32), tick: u32, near: bool, age: Option<u64>) -> i32 {
    let p = m.people;
    let freed = m.stage == Stage::Settled;
    shadow(cv, (x, y), 24.0, 4.0);
    let (cx, cy) = (x - 24, y - 34);
    // Straw on the cage floor.
    cv.rect(cx + 3, cy + 29, 42, 3, 'b');
    for k in 0..12 {
        let h = hash(k, 5, 9611);
        cv.line(cx + 4 + (h % 40) as i32, cy + 30, cx + 6 + (h % 40) as i32, cy + 28, 'o');
    }
    if !freed {
        let pose = |slot: usize| {
            if m.stage == Stage::Open || (near && slot == 1) { folk::Lost::Waving } else { folk::Lost::Huddled }
        };
        for (slot, dx) in [(0usize, 8), (1, 24), (2, 40)] {
            let im = folk::lost_figure(p, pose(slot), tick + slot as u32 * 7);
            stand(cv, &im, cx + dx + 3, cy + 31);
        }
    } else if let Some(age) = age {
        // Home: three lights going up and away.
        for slot in 0..3i32 {
            let t = age as f32 / 70.0 - slot as f32 * 0.08;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let (hx, hy) = (cx as f32 + 11.0 + slot as f32 * 16.0 + t * 14.0, cy as f32 + 22.0 - t * t * 150.0);
            for k in 0..9 {
                let f = k as f32 / 9.0;
                cv.put((hx - f * 7.0) as i32, (hy + f * 18.0) as i32, if k < 2 { 'c' } else if k < 5 { '9' } else { '6' });
            }
            cv.rect(hx as i32 - 1, hy as i32 - 1, 3, 3, '9');
        }
    }
    cage(cv, (cx, cy), freed, m.stage == Stage::Fighting, tick);
    cy - 12
}

/// The cage's iron: a flat top, bars, a plank floor, a padlock; open, its
/// door swung back.
fn cage(cv: &mut Img, (x, y): (i32, i32), open: bool, rattled: bool, tick: u32) {
    let shake = if rattled { ((tick / 2) % 2) as i32 } else { 0 };
    let x = x + shake;
    // Plank floor and top.
    cv.rect(x, y + 30, 48, 3, 'g');
    cv.line(x, y + 30, x + 47, y + 30, 'j');
    cv.rect(x, y + 5, 48, 3, 'G');
    cv.line(x, y + 5, x + 47, y + 5, 'h');
    cv.line(x, y + 8, x + 47, y + 8, 'g');
    // Bars.
    for k in 0..10 {
        let bx = x + 2 + k * 5;
        let door = k >= 7 && open;
        if door {
            // The door swung out on its hinge: leaning, seen edge-on.
            cv.line(bx + 12 - (k - 7) * 2, y + 8, bx + 18 - (k - 7) * 2, y + 30, 'j');
            continue;
        }
        cv.line(bx, y + 8, bx, y + 30, 'j');
        cv.line(bx - 1, y + 8, bx - 1, y + 30, 'h');
        cv.line(bx + 1, y + 8, bx + 1, y + 30, 'g');
    }
    // Crossbars.
    cv.line(x, y + 18, x + if open { 33 } else { 47 }, y + 18, 'G');
    // Rivets on the top.
    for k in 0..6 {
        cv.put(x + 4 + k * 8, y + 6, 'J');
    }
    // The padlock, brass; gone when it is open.
    if !open {
        cv.rect(x + 34, y + 20, 5, 5, 'o');
        cv.rect(x + 34, y + 20, 5, 1, 'O');
        cv.line(x + 35, y + 17, x + 35, y + 20, 'h');
        cv.line(x + 38, y + 17, x + 38, y + 20, 'h');
        cv.put(x + 36, y + 22, 'k');
    } else {
        cv.rect(x + 44, y + 31, 3, 2, 'o');
    }
}

// ─── the drowned shrine ──────────────────────────────────────────────────────

/// A shrine of a people, half under black water: two pillars and a lintel
/// with a bell hung in them, a lamp burning under the water, a hoard at its
/// foot. The water answers what was done: still after an offering, empty
/// and dark after the hoard is taken, churning while the drowned rise.
fn shrine(cv: &mut Img, m: &Meeting, (x, y): (i32, i32), tick: u32) -> i32 {
    let p = m.people;
    let lk = look(p);
    let plundered = m.choice == Some(1);
    let offered = m.choice == Some(2);
    let churning = m.stage == Stage::Fighting;
    let (cx, cy) = (x, y - 2);
    let power = if plundered { 0.0 } else if churning { 0.7 + 0.3 * ((tick / 2 % 3) as f32 / 2.0) } else { 0.8 };
    // The pool, black, rimmed in a cold ink, ringed by dithering.
    let (shore, deep, mid) = if plundered { ('x', 'k', 'K') } else { ('S', 's', 'x') };
    cv.ellipse(cx as f32, cy as f32, 37.0, 12.0, shore);
    cv.ellipse(cx as f32, (cy + 1) as f32, 35.0, 10.5, deep);
    cv.ellipse(cx as f32, (cy + 1) as f32, 29.0, 7.5, mid);
    for k in 0..46 {
        let h = hash(k, 2, 9621);
        let a = (h % 628) as f32 / 100.0;
        let rr = 0.3 + (h >> 10) as f32 % 60.0 / 100.0;
        cv.put(cx + (a.cos() * 34.0 * rr) as i32, cy + 1 + (a.sin() * 9.5 * rr) as i32, if plundered { 'K' } else { 'S' });
    }
    // The far shore, lit where the lamp's light reaches it.
    if !plundered {
        for k in 0..28 {
            let a = std::f32::consts::PI * (1.1 + 0.8 * k as f32 / 27.0);
            cv.put(cx + (a.cos() * 37.0) as i32, cy + (a.sin() * 12.0) as i32, 'u');
        }
    }
    // The lamp under the water, and the pool of its light on the surface.
    if !plundered {
        let flick = if offered { 1.0 } else { power };
        glow(cv, (cx, cy), 44, ('0', lk.flame.1), flick * 0.9);
        cv.ellipse(cx as f32, (cy + 1) as f32, 6.5, 2.5, '0');
        cv.ellipse(cx as f32, (cy + 1) as f32, 4.0, 1.6, lk.flame.1);
        cv.rect(cx - 2, cy, 4, 2, lk.flame.0);
        cv.put(cx, cy, '3');
    }
    // The pillars and the lintel, half in the water.
    for px_ in [cx - 25, cx + 19] {
        cv.rect(px_, cy - 32, 7, 36, 'u');
        cv.rect(px_, cy - 32, 3, 36, 'U');
        cv.rect(px_ + 5, cy - 32, 2, 36, 's');
        for k in 0..5 {
            cv.line(px_, cy - 28 + k * 7, px_ + 6, cy - 28 + k * 7, 'S');
        }
    }
    cv.rect(cx - 28, cy - 38, 56, 7, 'u');
    cv.rect(cx - 28, cy - 38, 56, 2, 'v');
    cv.rect(cx - 28, cy - 32, 56, 1, 's');
    cv.rect(cx - 8, cy - 42, 16, 4, 'U');
    cv.rect(cx - 8, cy - 42, 16, 1, 'V');
    // The people's own sign carved on the lintel: its lamp-word's colour.
    cv.rect(cx - 2, cy - 36, 4, 3, lk.flame.1);
    // The water laps over their feet: dark taken back over the stone.
    cv.ellipse(cx as f32, (cy + 4) as f32, 33.0, 5.5, deep);
    cv.ellipse(cx as f32, (cy + 4) as f32, 26.0, 3.5, mid);
    // The bell, on its chain.
    cv.line(cx, cy - 31, cx, cy - 26, 'j');
    let sway = if churning { (((tick / 3) % 5) as i32 - 2).signum() } else { 0 };
    let bx = cx + sway;
    cv.stamp(&bell(), bx - 5, cy - 27);
    // The hoard at the foot, till it is taken.
    if !plundered {
        for k in 0..7 {
            let h = hash(k, 7, 9622);
            let gx = cx - 12 + (h % 24) as i32;
            let gy = cy + 6 + ((h >> 8) % 3) as i32;
            let tone = ['4', '5', '6', 'z'][(h >> 12) as usize % 4];
            cv.rect(gx, gy, 2, 1, tone);
            if (tick / 7 + k as u32).is_multiple_of(9) {
                cv.put(gx, gy - 1, '9');
            }
        }
    }
    // Ripples, wide and slow; fast and many when the drowned rise; none
    // after an offering, the water gone still.
    let rings = if offered { 0 } else if churning { 4 } else { 2 };
    for r in 0..rings {
        let t = ((tick / if churning { 2 } else { 6 } + r * 9) % 30) as f32 / 30.0;
        let (rx, ry) = (6.0 + t * 26.0, 1.5 + t * 6.0);
        for k in 0..32 {
            let a = k as f32 / 32.0 * std::f32::consts::TAU;
            if k % 3 != 0 {
                cv.put(cx + (a.cos() * rx) as i32, cy + 2 + (a.sin() * ry) as i32, if t < 0.5 { 'S' } else { 'x' });
            }
        }
    }
    if plundered {
        // A dull red mark on the water where the hoard was.
        for k in 0..5i32 {
            cv.put(cx - 4 + k * 2, cy + 7, '7');
        }
    }
    cy - 58
}

/// The shrine's bell: flared at the lip, lit from the left, its clapper
/// hanging.
fn bell() -> Img {
    Img::from_rows(&[
        "....BBB....",
        "...BoOOB...",
        "..BoOOtOB..",
        "..BoOOtOB..",
        ".BoOOOtOOB.",
        ".BoOOtOOOB.",
        "BoOOOOtOOOB",
        "BTTTTTTTTTB",
        ".bbbbbbbbb.",
        "....BtB....",
        ".....t.....",
    ])
}

// ─── the envoy ───────────────────────────────────────────────────────────────

/// A people's banner: its field and its charge, plain cloth inks.
fn banner(p: Culture) -> (char, char) {
    match p {
        Culture::Capfolk => ('R', 'T'),
        Culture::Glimmerkin => ('q', 'z'),
        Culture::Emberfolk => ('p', 'O'),
        Culture::Lampwick => ('Q', 'O'),
        Culture::Roostfolk => ('X', 'R'),
        Culture::Rootkin => ('E', 'Y'),
        Culture::Chimefolk => ('u', 'v'),
        Culture::Saltkin => ('h', '%'),
    }
}

/// An envoy of a people under their pennant, a sealed letter in hand. Sent
/// home, they walk back the way they came; with their letter taken, they
/// stand lighter.
fn envoy(cv: &mut Img, m: &Meeting, (x, y): (i32, i32), tick: u32, near: bool, age: Option<u64>, gone: bool) -> i32 {
    let p = m.people;
    let sent = m.choice == Some(3);
    let walked = if sent { age.unwrap_or(0).min(ENVOY_LEAVES) as i32 } else { 0 };
    if gone {
        return y - 62;
    }
    let ex = x - walked / 2;
    shadow(cv, (ex, y), 10.0, 3.0);
    let fig = folk::figure(p, 0, near && !sent, false);
    let hop = if sent { [0, 1, 2, 1][(tick / 3 % 4) as usize] } else { 0 };
    stand(cv, &fig, ex, y - hop);
    // The pennant: a pole at their side, a swallow-tailed flag in the
    // people's colours that waves.
    let (px_, top) = (ex + 11, y - 40);
    cv.line(px_, y, px_, top, 'P');
    cv.line(px_ + 1, y, px_ + 1, top, 'r');
    cv.rect(px_ - 1, top - 2, 4, 2, 'O');
    let (field, charge) = banner(p);
    for col in 0..15i32 {
        let wave = (((col as f32) * 0.5 + tick as f32 * 0.15).sin() * 1.6) as i32;
        let hgt = (11 - col / 2).max(3);
        for row in 0..hgt {
            let ink = if row < 2 { charge } else { field };
            cv.put(px_ + 2 + col, top + 1 + row + wave, ink);
        }
    }
    // The letter, sealed with red wax, till it is taken.
    if m.choice.is_none() || sent {
        cv.rect(ex - 9, y - 10, 6, 4, 'c');
        cv.line(ex - 9, y - 10, ex - 4, y - 10, '9');
        cv.put(ex - 6, y - 8, '7');
    }
    y - 62
}

// ─── the board's portraits ───────────────────────────────────────────────────

/// A speaker's portrait for the talk board, three times size.
pub fn portrait(face: Face) -> Img {
    match face {
        Face::Folk(culture, who) => folk::portrait(culture, who),
        Face::Hermit => scaled(&hermit_face(), 3),
        Face::Shrine => scaled(&shrine_face(), 3),
    }
}

fn scaled(im: &Img, k: i32) -> Img {
    let mut out = Img::new(im.w * k, im.h * k);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = im.get(x / k, y / k) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// The hermit, head and shoulders: hood, blindfold, white beard.
fn hermit_face() -> Img {
    let mut im = Img::new(14, 14);
    im.ellipse(7.0, 5.5, 5.5, 5.5, ')');
    im.ellipse(6.0, 4.5, 3.5, 3.5, '[');
    im.rect(1, 9, 12, 5, ')');
    im.rect(1, 9, 3, 5, '[');
    im.rect(10, 9, 3, 5, '(');
    im.ellipse(7.0, 7.0, 3.4, 4.0, 'd');
    im.ellipse(7.0, 7.0, 2.6, 3.2, 'o');
    im.rect(4, 6, 7, 2, 'c');
    im.line(4, 6, 10, 6, '9');
    im.rect(5, 9, 5, 3, 'i');
    im.rect(6, 12, 3, 2, 'h');
    im.outline_outside('d');
    im
}

/// The shrine's carved face: a worn mask in the stone, eyes shut.
fn shrine_face() -> Img {
    let mut im = Img::new(14, 14);
    im.rect(1, 1, 12, 12, 'u');
    im.rect(1, 1, 12, 2, 'U');
    im.rect(1, 11, 12, 2, 's');
    im.rect(1, 1, 2, 12, 'U');
    im.rect(11, 1, 2, 12, 's');
    im.ellipse(7.0, 7.0, 4.0, 5.0, 'S');
    im.ellipse(7.0, 7.0, 3.0, 4.0, 'u');
    im.line(4, 6, 6, 6, 's');
    im.line(8, 6, 10, 6, 's');
    im.line(7, 7, 7, 9, 'S');
    im.line(5, 10, 9, 10, 's');
    // A drowned lamp's glint in the stone.
    im.put(11, 3, '2');
    im.outline_outside('s');
    im
}

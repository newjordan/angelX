//! The Powder Room's proof reels, frame-time bench and the walk over whole
//! runs: the real `Run` played through by bomber bots (each walks to the
//! nearest crate, sets a keg when it can get clear, and runs from any lane
//! that will burn), recorded on seat one's own screen (and seat two's) at
//! every tick (30 fps) in windows: the opening, a blast, a chain, a sooting,
//! a run of crates, the last seconds, and the finish with the store going
//! up and then the yard. It changes no game code and writes only under the
//! directory it is given.
//!
//!   ANGEL_POWDER_REEL=<dir> [REEL_ONLY=start,chain] cargo test -p angel-delve --lib write_powder_reel -- --ignored --nocapture
//!   cargo test -p angel-delve --lib powder_frame_time -- --ignored --nocapture
//!
//! `/work/artifacts/minigames-night-20261107/powder/make_gifs.py` makes the GIFs from the frames.
use super::*;
use crate::render::overworld::{Img, arena::frame_for};
use crate::rules::folly::TITLE;
use crate::rules::powder::{FUSE, KEGS_EACH, Keg, POWDER_TIME, Powder};
use crate::rules::powder_show::{self, Cry};
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(crate) type Keys = BTreeMap<u32, Input>;
type Tile2 = (i32, i32);

/// The store with `seats` knights at the table, on a floor laid for them.
pub(crate) fn fresh_for(seats: u32) -> Run {
    let mut r = Run::new(41, 1, None);
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.descend_for_test();
    r.descend_for_test();
    assert_eq!(r.dungeon.depth % 3, 0, "a third floor");
    r.enemies.clear();
    for (id, name) in [(1, "Aldric"), (2, "Brienne"), (3, "Cedric"), (4, "Dara"), (5, "Edda"), (6, "Fenn")] {
        if let Some(h) = r.players.get_mut(&id) {
            h.name = name.into();
        }
    }
    r.pick_folly();
    for h in r.players.values_mut() {
        h.carried.add(Spoil::Gold, 50);
    }
    // Fortune's dare card lifts long before a knight finds a folly.
    r.dare = None;
    r.enter_for_test(r.folly_room.expect("a hall on floor three"));
    assert!(r.powder.is_some(), "the powder room");
    r
}

pub(crate) fn fresh() -> Run {
    fresh_for(4)
}

fn tile_of(h: &Hero) -> Tile2 {
    ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32)
}

/// The tiles that will burn within `soon` ticks (or burn now), with the
/// kegs' own.
fn lanes(run: &Run, p: &Powder, soon: u32) -> BTreeSet<Tile2> {
    let mut out: BTreeSet<Tile2> = p.fire.iter().map(|f| f.0).collect();
    for k in p.kegs.iter().filter(|k| k.fuse <= soon) {
        out.extend(lit_by(run, p, k.at));
    }
    out
}

fn lit_by(run: &Run, p: &Powder, at: Tile2) -> Vec<Tile2> {
    let (arms, _) = powder_show::cross(run.room(), &p.crates, at);
    let mut v = vec![at];
    for d in 0..4 {
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][d];
        v.extend((1..=i32::from(arms[d])).map(|i| (at.0 + dx * i, at.1 + dy * i)));
    }
    v
}

/// The nearest tile (by steps over open floor) that satisfies `goal`,
/// avoiding `avoid` (the start excepted), and the first step towards it.
fn route(run: &Run, from: Tile2, goal: impl Fn(Tile2) -> bool, avoid: &BTreeSet<Tile2>, limit: usize) -> Option<(Tile2, usize)> {
    let room = run.room();
    let mut seen = BTreeSet::from([from]);
    let mut queue = std::collections::VecDeque::from([(from, None::<Tile2>, 0usize)]);
    while let Some((t, first, d)) = queue.pop_front() {
        if t != from && goal(t) {
            return first.map(|f| (f, d));
        }
        if d >= limit {
            continue;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (t.0 + dx, t.1 + dy);
            if seen.contains(&n) || room.tile(n.0, n.1) != Tile::Floor || avoid.contains(&n) {
                continue;
            }
            seen.insert(n);
            queue.push_back((n, first.or(Some(n)), d + 1));
        }
    }
    None
}

/// The press that walks `hero` from its tile to the next, along the middle
/// of the lane.
fn step_to(hero: &Hero, from: Tile2, to: Tile2) -> (i8, i8) {
    let mid = |t: i32| (t as f32 + 0.5) * TILE_UNITS;
    let (ex, ey) = (mid(from.0) - hero.x, mid(from.1) - hero.y);
    let slack = 0.25 * TILE_UNITS;
    let sign = |v: f32| if v.abs() < 0.08 { 0 } else { v.signum() as i8 };
    if to.0 != from.0 {
        if ey.abs() > slack { (0, sign(ey)) } else { ((to.0 - from.0) as i8, sign(ey)) }
    } else if ex.abs() > slack {
        (sign(ex), 0)
    } else {
        (sign(ex), (to.1 - from.1) as i8)
    }
}

fn beside_crate(run: &Run, p: &Powder, t: Tile2) -> bool {
    let cols = run.room().cols as i32;
    [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| p.crates.contains(&(((t.1 + dy) * cols + t.0 + dx) as u16)))
}

/// Whether a keg set down at `t` leaves a way out of its own lanes (and
/// everyone's) within a few steps.
fn safe_drop(run: &Run, p: &Powder, t: Tile2, danger: &BTreeSet<Tile2>) -> bool {
    let mut would = danger.clone();
    would.extend(lit_by(run, p, t));
    route(run, t, |n| !would.contains(&n), danger, 3).is_some()
}

/// What a bomber presses: out of any lane that will burn; else, beside a
/// crate with a keg to spare and a way clear, set one; else walk to the
/// nearest place that is both.
pub(crate) fn keys(run: &Run) -> Keys {
    let Some(p) = run.powder.as_ref() else { return Keys::new() };
    if p.titled() || p.done {
        return Keys::new();
    }
    let danger = lanes(run, p, 80);
    let burning: BTreeSet<Tile2> = p.fire.iter().map(|f| f.0).collect();
    let mut out = Keys::new();
    for (&id, hero) in run.players.iter().filter(|(id, h)| h.hp > 0 && p.broken.contains_key(id)) {
        let me = tile_of(hero);
        let mut input = Input::default();
        let mine = p.kegs.iter().filter(|k| k.owner == id).count();
        let free_hand = mine < KEGS_EACH && !p.kegs.iter().any(|k| k.at == me) && !p.held.contains(&id) && (run.tick as u32 + id * 7) % 4 != 0;
        // A bomber now and then is careless: it stays in a lane, sets a keg
        // where there is no way out. So the reel has a sooting to show.
        let reckless = (id + run.tick as u32 / 120) % 3 == 0;
        if danger.contains(&me) && !reckless {
            // Run for the nearest tile out of the lanes.
            if let Some((next, _)) = route(run, me, |t| !danger.contains(&t), &burning, 14) {
                (input.move_x, input.move_y) = step_to(hero, me, next);
            }
        } else if free_hand && beside_crate(run, p, me) && (reckless || safe_drop(run, p, me, &danger)) {
            input.bomb = true;
        } else if let Some((next, _)) = route(
            run,
            me,
            |t| !danger.contains(&t) && beside_crate(run, p, t) && safe_drop(run, p, t, &danger),
            &danger,
            60,
        ) {
            (input.move_x, input.move_y) = step_to(hero, me, next);
        }
        out.insert(id, input);
    }
    out
}

/// The ticks that matter, found by playing the store through once unseen.
#[derive(Debug, Default)]
struct Marks {
    first_blast: Option<u32>,
    hauls: Vec<(u32, u32)>,
    chains: Vec<(u32, u32, u8)>,
    soots: Vec<(u32, u32)>,
    rolls: Vec<(u32, u32, u8)>,
    stores: Vec<(u32, u8)>,
    lead: Option<(u32, u32)>,
    end: Option<u32>,
}

fn survey(seats: u32) -> Marks {
    let mut r = fresh_for(seats);
    let mut marks = Marks::default();
    let mut seen = BTreeSet::new();
    for t in 0..4_500u32 {
        let k = keys(&r);
        r.step(&k);
        let Some(p) = r.powder.as_ref() else { break };
        if marks.first_blast.is_none() && !p.show.blasts.is_empty() {
            marks.first_blast = Some(t);
        }
        for h in p.show.shouts.iter() {
            if !seen.insert((h.born, u32::from(h.who), u32::from(h.cry.rank()))) {
                continue;
            }
            match h.cry {
                Cry::Haul(_) => marks.hauls.push((t, u32::from(h.who))),
                Cry::Chain(n) => marks.chains.push((t, u32::from(h.who), n)),
                Cry::Roll(n) => marks.rolls.push((t, u32::from(h.who), n)),
                Cry::Store(n) => marks.stores.push((t, n)),
                Cry::Lead if marks.lead.is_none() => marks.lead = Some((t, u32::from(h.who))),
                _ => {}
            }
        }
        for s in &p.show.soots {
            if seen.insert((s.born, u32::from(s.who), 99)) {
                marks.soots.push((t, u32::from(s.who)));
            }
        }
        if p.done && marks.end.is_none() {
            marks.end = Some(t);
        }
        if marks.end.is_some_and(|e| t > e + 360) {
            break;
        }
    }
    marks
}

fn save(dir: &PathBuf, n: usize, sheet: &Img) {
    image::save_buffer(dir.join(format!("f{n:04}.png")), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
}

/// Lay a lattice of kegs over a cleared patch near seat one, the root's
/// fuse almost out: a chain that runs the whole patch. For the proof.
fn stage_chain(r: &mut Run) {
    let cols = r.room().cols as i32;
    let hero = r.players[&1].clone();
    let (hx, hy) = tile_of(&hero);
    // A patch of odd-odd kegs three by four, two tiles apart, found a few
    // tiles in front of the knight, kept inside the walls.
    let (x0, y0) = ((hx + 3).clamp(2, cols - 12) | 1, (hy + 1).clamp(2, r.room().rows as i32 - 10) | 1);
    let at = r.at;
    let mut kegs = Vec::new();
    {
        let p = r.powder.as_mut().unwrap();
        p.kegs.clear();
        for j in 0..3 {
            for i in 0..4 {
                kegs.push((x0 + i * 2, y0 + j * 2));
            }
        }
        // Open every lane of the lattice and every keg's tile.
        for y in y0 - 1..=y0 + 5 {
            for x in x0 - 1..=x0 + 7 {
                let pier = powder::pillar(x, y);
                if !pier && (x % 2 == 1 || y % 2 == 1) && (x >= x0 && x <= x0 + 6 && y >= y0 && y <= y0 + 4) {
                    p.crates.remove(&((y * cols + x) as u16));
                }
            }
        }
        for (k, &(x, y)) in kegs.iter().enumerate() {
            p.kegs.push(Keg { at: (x, y), owner: 1 + (k as u32 % 2), fuse: if k == 0 { 3 } else { FUSE + 40 } });
        }
    }
    for y in y0 - 1..=y0 + 5 {
        for x in x0 - 1..=x0 + 7 {
            if !powder::pillar(x, y) && (x >= x0 && x <= x0 + 6 && y >= y0 && y <= y0 + 4) {
                r.dungeon.rooms[at].set(x as usize, y as usize, Tile::Floor);
            }
        }
    }
    // Crates just outside, so there is something to break.
    r.dungeon.world_revision = r.dungeon.world_revision.wrapping_add(1);
    // Stand knight one a little back from it.
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = ((x0 as f32 - 2.5) * TILE_UNITS, (y0 as f32 + 2.5) * TILE_UNITS);
}

/// Play again, writing a frame of `seat`'s screen every tick in `[from, to)`.
fn record_for(root: &PathBuf, name: &str, seats: u32, from: u32, to: u32, seat: u32, also: Option<u32>, stage: Option<u32>) {
    let dir = root.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = fresh_for(seats);
    let mut n = 0;
    for t in 0..to {
        if stage == Some(t) {
            stage_chain(&mut r);
        }
        let k = keys(&r);
        r.step(&k);
        if t >= from {
            let mut sheet = Img::black(384 * if also.is_some() { 2 } else { 1 }, 224);
            sheet.stamp(&frame_for(&r, 384, 224, Some(seat)), 0, 0);
            if let Some(other) = also {
                sheet.stamp(&frame_for(&r, 384, 224, Some(other)), 384, 0);
            }
            save(&dir, n, &sheet);
            n += 1;
        }
    }
    println!("{name}: {n} frames from tick {from}, seat {seat}");
}

fn record(root: &PathBuf, name: &str, from: u32, to: u32, seat: u32, also: Option<u32>) {
    record_for(root, name, 4, from, to, seat, also, None);
}

#[test]
#[ignore = "writes the Powder Room's proof reels"]
fn write_powder_reel() {
    let Some(root) = std::env::var_os("ANGEL_POWDER_REEL") else { return };
    let root = PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let marks = survey(4);
    println!("{marks:?}");
    let only = std::env::var("REEL_ONLY").unwrap_or_default();
    let want = |n: &str| only.is_empty() || only.split(',').any(|x| x == n);
    if want("start") {
        record(&root, "start", 0, TITLE + 170, 1, Some(2));
    }
    if want("blast") && let Some(t) = marks.first_blast {
        record(&root, "blast", t.saturating_sub(70), t + 70, 1, None);
    }
    if want("haul") && let Some(&(t, who)) = marks.hauls.first() {
        record(&root, "haul", t.saturating_sub(60), t + 80, who.max(1), None);
    }
    if want("chain") {
        // Staged: a lattice of kegs near seat one set going.
        let t = TITLE + 60;
        record_for(&root, "chain", 4, t, t + 150, 1, Some(2), Some(t));
    }
    if want("soot") && let Some(&(t, who)) = marks.soots.first() {
        record(&root, "soot", t.saturating_sub(50), t + 70, who, None);
    }
    if want("roll") && let Some(&(t, who, _)) = marks.rolls.first() {
        record(&root, "roll", t.saturating_sub(80), t + 70, who, None);
    }
    if want("solo") {
        record_for(&root, "solo", 1, 0, TITLE + 200, 1, None, None);
    }
    if want("six") {
        record_for(&root, "six", 6, 0, TITLE + 200, 6, None, None);
    }
    if want("last") {
        let t = TITLE + POWDER_TIME - 6 * 30;
        record(&root, "last", t, t + 6 * 30, 1, Some(2));
    }
    if want("finish") && let Some(end) = marks.end {
        record(&root, "finish", end.saturating_sub(40), end + 360, 1, Some(2));
    }
}

/// The Powder Room's frame time: four bombers play to the middle of the
/// minute, then seat one's screen is drawn twice a tick, as the store draws
/// it and as the room was drawn before it (`powder_room::OFF`), so both see
/// the same game; and each part of the store is timed alone.
#[test]
#[ignore = "times the Powder Room's frames"]
fn powder_frame_time() {
    use crate::render::overworld::arena::{powder_profile, powder_set_off};
    use std::time::Instant;
    let mut r = fresh();
    for _ in 0..TITLE + 600 {
        let k = keys(&r);
        r.step(&k);
    }
    let _ = frame_for(&r, 384, 224, Some(1));
    powder_set_off(true);
    let _ = frame_for(&r, 384, 224, Some(1));
    powder_set_off(false);
    let (mut new, mut old) = (Vec::new(), Vec::new());
    let mut parts = [f64::MAX; 3];
    let mut sums = [0.0f64; 3];
    let best = |r: &Run, n: usize| {
        (0..n)
            .map(|_| {
                let t = Instant::now();
                let img = frame_for(r, 384, 224, Some(1));
                std::hint::black_box(&img);
                t.elapsed().as_secs_f64() * 1000.0
            })
            .fold(f64::MAX, f64::min)
    };
    for _ in 0..240 {
        let k = keys(&r);
        r.step(&k);
        new.push(best(&r, 5));
        let mut tick_parts = [f64::MAX; 3];
        for _ in 0..5 {
            for (i, (_, ms)) in powder_profile(&r, Some(1)).iter().enumerate() {
                tick_parts[i] = tick_parts[i].min(*ms);
            }
        }
        for i in 0..3 {
            sums[i] += tick_parts[i] / 240.0;
            parts[i] = sums[i];
        }
        powder_set_off(true);
        old.push(best(&r, 5));
        powder_set_off(false);
    }
    let stats = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let slow = v[v.len() * 9 / 10..].iter().sum::<f64>() / (v.len() - v.len() * 9 / 10) as f64;
        (mean, v[v.len() / 2], slow)
    };
    let (n, o) = (stats(&mut new), stats(&mut old));
    println!("powder frame (new): mean {:.2} ms, median {:.2} ms, slowest tenth {:.2} ms, tick {}", n.0, n.1, n.2, r.tick);
    println!("powder frame (old): mean {:.2} ms, median {:.2} ms, slowest tenth {:.2} ms", o.0, o.1, o.2);
    println!("powder frame: new/old mean {:.2}x, slowest tenth {:.2}x", n.0 / o.0, n.2 / o.2);
    println!("powder parts (ms): floor {:.2}, over {:.2}, screen {:.2}", parts[0], parts[1], parts[2]);
    // The heaviest pictures: a staged chain tearing a patch of the store,
    // and the whole store going up at the end.
    let heavy = |label: &str, r: &mut Run, ticks: u32| {
        let mut ms = Vec::new();
        for _ in 0..ticks {
            let k = keys(r);
            r.step(&k);
            ms.push(best(r, 3));
        }
        let mean = ms.iter().sum::<f64>() / ms.len() as f64;
        let max = ms.iter().copied().fold(0.0, f64::max);
        println!("powder {label}: mean {mean:.2} ms, slowest {max:.2} ms over {ticks} ticks");
    };
    stage_chain(&mut r);
    heavy("chain", &mut r, 70);
    while r.powder.as_ref().is_some_and(|p| !p.done) {
        let k = keys(&r);
        r.step(&k);
    }
    heavy("finale ripple", &mut r, 70);
}

/// The store draws through the whole minute and its finish, for a lone
/// knight and for every table up to six seats, and the store it draws is
/// the same twice (the host's and a mirror's are one).
#[test]
fn the_store_draws_through_the_whole_minute_the_same_twice() {
    use std::hash::{Hash, Hasher};
    let digest = |seats: u32| {
        let mut r = fresh_for(seats);
        let mut states = Vec::new();
        for t in 0..TITLE + POWDER_TIME + 380 {
            let k = keys(&r);
            r.step(&k);
            if t % 31 == 0 || t > TITLE + POWDER_TIME && t % 9 == 0 {
                let img = frame_for(&r, 384, 224, Some(1));
                assert_eq!((img.w, img.h), (384, 224));
                let mut h = std::collections::hash_map::DefaultHasher::new();
                serde_json::to_string(&r.powder).unwrap().hash(&mut h);
                states.push((t, h.finish()));
            }
            if let Some(p) = r.powder.as_ref() {
                assert!(p.valid(), "the store stays valid at tick {t}");
            }
        }
        let p = r.powder.as_ref().expect("the store is still on the books");
        assert!(p.done && p.show.ended.is_some(), "the minute ends with its finish noted");
        assert!(p.show.team > 0, "crates were broken");
        states
    };
    for seats in [1, 4] {
        assert_eq!(digest(seats), digest(seats), "{seats} seats: the same store twice");
    }
}

fn rect_of(c: &crate::render::overworld::arena::PowderClaim) -> (i32, i32, i32, i32) {
    c.rect
}

/// No two words meet on any seat's screen, and every box is whole in the
/// frame, through whole runs at one to six seats: the strip, the store's
/// bar, the title and countdown, every call, the numbers that fly and pop,
/// the last seconds, the finish and the yard.
#[test]
fn the_words_never_meet_and_stay_in_the_frame_at_one_to_six_seats() {
    use crate::render::overworld::arena::powder_words;
    use crate::render::overworld::arena::powder_meets as meets;
    let mut checked = 0usize;
    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    for seats in 1..=6u32 {
        let mut r = fresh_for(seats);
        let mut recent = 0u32;
        for t in 0..TITLE + POWDER_TIME + 400 {
            let k = keys(&r);
            r.step(&k);
            // Staged chain half way, so the biggest calls and a crowd of
            // numbers are on the screen too.
            if t == TITLE + 600 {
                stage_chain(&mut r);
            }
            if let Some(p) = r.powder.as_ref()
                && (!p.show.shouts.is_empty() || !p.show.soots.is_empty() || p.show.blasts.iter().any(|b| b.age(r.tick as u32) < 40))
            {
                recent = 24;
            }
            let busy = recent > 0;
            recent = recent.saturating_sub(1);
            if !(t % 6 == 0 || busy && t % 2 == 0) {
                continue;
            }
            for seat in 1..=seats.min(3).max(1) {
                let focus = if seat == 3 { seats } else { seat };
                let _ = frame_for(&r, 384, 224, Some(focus));
                let boxes = powder_words();
                checked += 1;
                seen.extend(boxes.iter().map(|b| b.what));
                for (i, a) in boxes.iter().enumerate() {
                    let ra = rect_of(a);
                    assert!(
                        ra.0 >= 0 && ra.1 >= 0 && ra.0 + ra.2 <= 384 && ra.1 + ra.3 <= 224,
                        "{seats} seats, tick {t}, seat {focus}: {:?} {ra:?} leaves the frame",
                        a.what
                    );
                    for b in &boxes[i + 1..] {
                        assert!(
                            !meets(ra, rect_of(b)),
                            "{seats} seats, tick {t}, seat {focus}: {:?} {ra:?} meets {:?} {:?}",
                            a.what,
                            b.what,
                            rect_of(b)
                        );
                    }
                }
            }
        }
    }
    assert!(checked > 600, "the walk looked at {checked} screens");
    // The walk met every kind of words the store puts up.
    for what in ["clock", "name", "count", "pot", "title", "beat", "go", "call", "flying", "sooted", "last", "finale", "winner", "gold", "table"] {
        assert!(seen.contains(what), "the walk never saw {what:?}; saw {seen:?}");
    }
}

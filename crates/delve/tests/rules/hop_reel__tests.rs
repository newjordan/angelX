//! Book Hop's proof reels, frame-time bench and the walk over whole runs:
//! the real `Run` played through by hopper bots (each reads the books ahead
//! and steps onto one as it comes, a few of them hasty), recorded on seat
//! one's own screen (and seat two's) at every tick (30 fps) in windows: the
//! opening, a sinking, a crossing, a run of clean ones, the archive lit, the
//! last seconds, and the finish with the reading room. It changes no game
//! code and writes only under the directory it is given.
//!
//!   ANGEL_HOP_REEL=<dir> [REEL_ONLY=start,lit] cargo test -p angel-delve --lib write_hop_reel -- --ignored --nocapture
//!   cargo test -p angel-delve --lib hop_frame_time -- --ignored --nocapture
//!
//! `/work/artifacts/minigames-night-20261107/book_hop/make_gifs.py` makes the GIFs from the frames.
use super::*;
use crate::render::overworld::{Img, arena::frame_for};
use crate::rules::folly::TITLE;
use crate::rules::hop::{self, EDGE_GRACE, HOP_TIME, PACES};
use crate::rules::hop_show::Cry;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(crate) type Keys = BTreeMap<u32, Input>;

/// The hall with `seats` knights at the table, on a sixth floor laid for them.
pub(crate) fn fresh_for(seats: u32) -> Run {
    let mut r = Run::new(41, 1, None);
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    for _ in 0..5 {
        r.descend_for_test();
    }
    assert_eq!(r.dungeon.depth % 6, 0, "a sixth floor");
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
    r.enter_for_test(r.folly_room.expect("a hall on floor six"));
    assert!(r.hop.is_some(), "book hop");
    r
}

pub(crate) fn fresh() -> Run {
    fresh_for(4)
}

/// How hasty each seat is, out of a hundred: the chance in a quarter second
/// of stepping on whether or not a book is there.
const HASTE: [u32; 6] = [8, 5, 14, 0, 0, 12];

/// What a hopper presses: wait on the bank or the book until a book in the
/// next lane will be under them when they get there, then go; slide to the
/// nearest book that will do; walk against a book that is carrying them
/// into the wall.
pub(crate) fn keys(run: &Run) -> Keys {
    let Some(h) = run.hop.as_ref() else { return Keys::new() };
    if h.titled() || h.done {
        return Keys::new();
    }
    let (cols, rows) = (run.room().cols as i32, run.room().rows as i32);
    let width = cols as f32 * TILE_UNITS;
    let t = h.t - TITLE;
    run.players
        .iter()
        .map(|(&id, me)| {
            let row = (me.y / TILE_UNITS) as i32;
            let here = hop::lane(rows, row);
            let dir = here.map_or(0.0, |l| if l % 2 == 0 { 1.0 } else { -1.0 });
            let drift = here.map_or(0.0, |l| if hop::afloat_with(l, t, width, me.x, 0.0) { dir * PACES[l % PACES.len()] } else { 0.0 });
            let ahead = row - 1;
            let lead = 3u32;
            let fits = |dx: f32, margin: f32| -> bool {
                let xf = me.x + dx + drift * lead as f32 / HZ as f32;
                match hop::lane(rows, ahead) {
                    None => true,
                    Some(l) => hop::afloat_with(l, t + lead, width, xf, -margin),
                }
            };
            let hasty = crate::render::overworld::ink::hash(id as i32, (run.tick / 12) as i32, 3) % 100 < HASTE[(id as usize - 1) % 6] / 2;
            let (mut move_x, mut move_y) = (0i8, 0i8);
            if fits(0.0, if id == 4 || id == 5 { 0.9 } else { 0.5 }) || hasty {
                move_y = -1;
            } else if let Some(dx) = [0.8f32, -0.8, 1.6, -1.6, 2.4, -2.4, 3.2, -3.2].into_iter().find(|&d| fits(d, 0.5)) {
                move_x = if dx > 0.0 { 1 } else { -1 };
            }
            // A book carrying them toward the wall: take the next lane, ready or not.
            if here.is_some() && drift != 0.0 {
                let to_wall = if dir > 0.0 { width - 2.5 * TILE_UNITS - me.x } else { me.x - 2.5 * TILE_UNITS };
                if to_wall < 5.0 {
                    move_y = -1;
                }
            }
            (id, Input { move_x, move_y, ..Default::default() })
        })
        .collect()
}

/// The ticks that matter, found by playing the minute through once unseen.
#[derive(Debug, Default)]
struct Marks {
    /// The first sink (tick, seat, rows lost), and the biggest.
    sinks: Vec<(u32, u32, u8, bool)>,
    crosses: Vec<(u32, u32, u16)>,
    /// A run of clean crossings (tick, seat, length).
    streaks: Vec<(u32, u32, u8)>,
    lit: Vec<u32>,
    lead: Option<(u32, u32)>,
    end: Option<u32>,
}

fn survey(seats: u32) -> Marks {
    let mut r = fresh_for(seats);
    let mut marks = Marks::default();
    let mut seen = BTreeSet::new();
    for t in 0..4_000u32 {
        let k = keys(&r);
        r.step(&k);
        let Some(h) = r.hop.as_ref() else { break };
        for s in &h.show.sinks {
            if seen.insert((s.born, u32::from(s.who), 1)) {
                marks.sinks.push((t, u32::from(s.who), s.lost, s.wall));
            }
        }
        for c in &h.show.crosses {
            if seen.insert((c.born, u32::from(c.who), 2)) {
                marks.crosses.push((t, u32::from(c.who), c.n));
            }
        }
        for c in &h.show.shouts {
            if !seen.insert((c.born, u32::from(c.who), u32::from(c.cry.rank()) + 10)) {
                continue;
            }
            match c.cry {
                Cry::Clean(n) if n >= 2 => marks.streaks.push((t, u32::from(c.who), n)),
                Cry::Lit(_) => marks.lit.push(t),
                Cry::Lead if marks.lead.is_none() => marks.lead = Some((t, u32::from(c.who))),
                _ => {}
            }
        }
        if h.done && marks.end.is_none() {
            marks.end = Some(t);
        }
        if marks.end.is_some_and(|e| t > e + 330) {
            break;
        }
    }
    marks
}

fn save(dir: &PathBuf, n: usize, sheet: &Img) {
    image::save_buffer(dir.join(format!("f{n:04}.png")), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
}

/// Play again, writing a frame of `seat`'s screen every tick in `[from, to)`.
fn record_for(root: &PathBuf, name: &str, seats: u32, from: u32, to: u32, seat: u32, also: Option<u32>) {
    record_staged(root, name, seats, from, to, seat, also, None);
}

/// As `record_for`, with one thing staged first: at tick `.0` the run is
/// handed to `.1` (a clean run is rare for these bots, so the one that
/// shows the call is given its two crossings before the real third).
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn record_staged(root: &PathBuf, name: &str, seats: u32, from: u32, to: u32, seat: u32, also: Option<u32>, stage: Option<(u32, &dyn Fn(&mut Run))>) {
    let dir = root.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = fresh_for(seats);
    let mut n = 0;
    for t in 0..to {
        if let Some((at, f)) = stage
            && t == at
        {
            f(&mut r);
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
    record_for(root, name, 4, from, to, seat, also);
}

/// The audit's own scene, for the before and after of one picture: two knights
/// who both walk straight up for twelve seconds, Fortune's dare card left
/// to come as it did.
fn record_audit(root: &PathBuf) {
    let dir = root.join("audit-scene");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = Run::new(41, 1, None);
    r.join_seat(2, "Two");
    for _ in 0..5 {
        r.descend_for_test();
    }
    r.enemies.clear();
    r.pick_folly();
    r.enter_for_test(r.folly_room.expect("a hall on floor six"));
    let up = BTreeMap::from([
        (1, Input { move_y: -1, ..Default::default() }),
        (2, Input { move_y: -1, move_x: 1, ..Default::default() }),
    ]);
    for t in 0..360 {
        r.step(&up);
        let mut sheet = Img::black(768, 224);
        sheet.stamp(&frame_for(&r, 384, 224, Some(1)), 0, 0);
        sheet.stamp(&frame_for(&r, 384, 224, Some(2)), 384, 0);
        save(&dir, t, &sheet);
    }
    println!("audit-scene: 360 frames");
}

#[test]
#[ignore = "writes Book Hop's proof reels"]
fn write_hop_reel() {
    let Some(root) = std::env::var_os("ANGEL_HOP_REEL") else { return };
    let root = PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let only = std::env::var("REEL_ONLY").unwrap_or_default();
    let want = |n: &str| only.is_empty() || only.split(',').any(|x| x == n);
    if want("audit-scene") {
        record_audit(&root);
    }
    let marks = survey(4);
    println!("{marks:?}");
    if want("start") {
        record(&root, "start", 0, TITLE + 150, 1, Some(2));
    }
    if want("sink") && let Some(&(t, who, _, _)) = marks.sinks.iter().filter(|s| s.1 == 1).max_by_key(|s| s.2).or(marks.sinks.first()) {
        record(&root, "sink", t.saturating_sub(40), t + 70, who, None);
    }
    if want("wall") && let Some(&(t, who, _, _)) = marks.sinks.iter().find(|s| s.3) {
        record(&root, "wall", t.saturating_sub(50), t + 60, who, None);
    }
    if want("cross") && let Some(&(t, who, _)) = marks.crosses.iter().find(|c| c.1 == 1).or(marks.crosses.first()) {
        record(&root, "cross", t.saturating_sub(50), t + 80, who, Some(2));
    }
    if want("streak") {
        // The first crossing of seat one, with the two before it given.
        if let Some(&(t, who, _)) = marks.crosses.iter().filter(|c| c.1 == 1).nth(1).or(marks.crosses.first()) {
            let give = |r: &mut Run| {
                if let Some(h) = r.hop.as_mut() {
                    h.show.streak.insert(who, 2);
                }
            };
            record_staged(&root, "streak", 4, t.saturating_sub(70), t + 90, who, None, Some((t.saturating_sub(4), &give)));
        }
    }
    if want("lit") && let Some(&t) = marks.lit.first() {
        record(&root, "lit", t.saturating_sub(60), t + 90, 1, Some(2));
    }
    if want("solo") {
        record_for(&root, "solo", 1, 0, TITLE + 240, 1, None);
    }
    if want("six") {
        record_for(&root, "six", 6, 0, TITLE + 150, 6, None);
    }
    if want("last") {
        let t = TITLE + HOP_TIME - 6 * 30;
        record(&root, "last", t, t + 6 * 30, 1, Some(2));
    }
    if want("finish") && let Some(end) = marks.end {
        record(&root, "finish", end.saturating_sub(40), end + 330, 1, Some(2));
    }
}

/// Book Hop's frame time: four hoppers play to the middle of the minute,
/// then seat one's screen is drawn twice a tick, as the archive draws it
/// and as the room was drawn before it (`book_hop::OFF`), so both see the
/// same game; and each part of the archive is timed alone.
#[test]
#[ignore = "times Book Hop's frames"]
fn hop_frame_time() {
    use crate::render::overworld::arena::{hop_profile, hop_set_off};
    use std::time::Instant;
    let mut r = fresh();
    for _ in 0..TITLE + 600 {
        let k = keys(&r);
        r.step(&k);
    }
    let _ = frame_for(&r, 384, 224, Some(1));
    hop_set_off(true);
    let _ = frame_for(&r, 384, 224, Some(1));
    hop_set_off(false);
    let (mut new, mut old) = (Vec::new(), Vec::new());
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
            for (i, (_, ms)) in hop_profile(&r, Some(1)).iter().enumerate() {
                tick_parts[i] = tick_parts[i].min(*ms);
            }
        }
        for i in 0..3 {
            sums[i] += tick_parts[i] / 240.0;
        }
        hop_set_off(true);
        old.push(best(&r, 5));
        hop_set_off(false);
    }
    let stats = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let slow = v[v.len() * 9 / 10..].iter().sum::<f64>() / (v.len() - v.len() * 9 / 10) as f64;
        (mean, v[v.len() / 2], slow)
    };
    let (n, o) = (stats(&mut new), stats(&mut old));
    println!("hop frame (new): mean {:.2} ms, median {:.2} ms, slowest tenth {:.2} ms, tick {}", n.0, n.1, n.2, r.tick);
    println!("hop frame (old): mean {:.2} ms, median {:.2} ms, slowest tenth {:.2} ms", o.0, o.1, o.2);
    println!("hop frame: new/old mean {:.2}x, slowest tenth {:.2}x", n.0 / o.0, n.2 / o.2);
    println!("hop parts (ms): floor {:.2}, over {:.2}, screen {:.2}", sums[0], sums[1], sums[2]);
}

// ─── the walks ──────────────────────────────────────────────────────────────

/// The hoppers really cross: a four-knight minute scores, sinks, lights the
/// archive at least once, and ends with its finish noted and the show valid
/// the whole way.
#[test]
fn the_hoppers_cross_sink_and_light_the_archive() {
    let mut r = fresh_for(4);
    let (mut sinks, mut lit) = (0, 0);
    for t in 0..TITLE + HOP_TIME + 40 {
        let k = keys(&r);
        r.step(&k);
        let h = r.hop.as_ref().expect("the hop is on the books");
        assert!(h.valid(), "the hop stays valid at tick {t}");
        sinks = sinks.max(h.show.sunk.values().map(|&n| usize::from(n)).sum::<usize>());
        lit = lit.max(h.show.waves);
    }
    let h = r.hop.as_ref().unwrap();
    assert!(h.done && h.show.ended.is_some());
    let total: u16 = h.crossings.values().sum();
    assert!(total >= 8, "four hoppers crossed {total} times in a minute");
    assert!(sinks >= 2, "somebody sank");
    assert!(lit >= 1, "the whole table crossed at least once");
}

fn rect_of(c: &crate::render::overworld::arena::HopClaim) -> (i32, i32, i32, i32) {
    c.rect
}

/// No two words meet on any seat's screen, and every box is whole in the
/// frame, through whole runs at one to six seats: the strip, the title and
/// countdown, every call, the numbers that fly and pop, the last seconds,
/// the finish and the reading room.
#[test]
fn the_words_never_meet_at_one_to_six_seats() {
    use crate::render::overworld::arena::{hop_meets as meets, hop_words};
    let mut checked = 0usize;
    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    for seats in 1..=6u32 {
        let mut r = fresh_for(seats);
        let mut recent = 0u32;
        for t in 0..TITLE + HOP_TIME + 400 {
            let k = keys(&r);
            r.step(&k);
            if let Some(h) = r.hop.as_ref()
                && (!h.show.shouts.is_empty() || !h.show.sinks.is_empty() || !h.show.crosses.is_empty())
            {
                recent = 24;
            }
            let busy = recent > 0;
            recent = recent.saturating_sub(1);
            if !(t % 6 == 0 || busy && t % 2 == 0) {
                continue;
            }
            for seat in [1, 2.min(seats), 3.min(seats), seats] {
                let _ = frame_for(&r, 384, 224, Some(seat));
                let boxes = hop_words();
                checked += 1;
                seen.extend(boxes.iter().map(|b| b.what));
                for (i, a) in boxes.iter().enumerate() {
                    let ra = rect_of(a);
                    assert!(
                        ra.0 >= 0 && ra.1 >= 0 && ra.0 + ra.2 <= 384 && ra.1 + ra.3 <= 224,
                        "{seats} seats, tick {t}, seat {seat}: {:?} {ra:?} leaves the frame",
                        a.what
                    );
                    for b in &boxes[i + 1..] {
                        assert!(!meets(ra, rect_of(b)), "{seats} seats, tick {t}, seat {seat}: {:?} {ra:?} meets {:?} {:?}", a.what, b.what, rect_of(b));
                    }
                }
            }
        }
    }
    assert!(checked > 700, "the walk looked at {checked} screens");
    for what in ["clock", "chip name", "chip count", "pot", "title", "count", "go", "call", "flying", "last", "finale", "winner", "gold", "table", "podium name"] {
        assert!(seen.contains(what), "the walk never saw {what:?}; saw {seen:?}");
    }
}

/// `YOU` names only the seat looking: through whole runs, a seat's screen
/// has `YOU` in its own chip and in no other, never a second one in the
/// strip, and `YOU WIN` only on the winner's screen.
#[test]
fn you_is_only_the_seat_looking() {
    use crate::render::overworld::arena::{hop_chip_xs, hop_said};
    let mut told = 0;
    for seats in [2u32, 4, 6] {
        let mut r = fresh_for(seats);
        // A rival on the default name is `K3`, never `YOU`.
        r.players.get_mut(&3.min(seats)).unwrap().name = "You".into();
        for t in 0..TITLE + HOP_TIME + 330 {
            let k = keys(&r);
            r.step(&k);
            if t % 9 != 0 {
                continue;
            }
            let ended = r.hop.as_ref().and_then(|h| h.show.ended);
            let chips = hop_chip_xs(&r);
            for me in 1..=seats {
                let _ = frame_for(&r, 384, 224, Some(me));
                let said = hop_said();
                let mine = chips.iter().find(|c| c.0 == me).map(|c| c.1);
                let yous: Vec<_> = said.iter().filter(|s| s.0 == "chip name" && s.1 == "YOU").collect();
                for you in &yous {
                    assert_eq!(Some(you.2.0 - 7), mine, "{seats} seats, tick {t}: seat {me} shows YOU at {:?}, not in its own chip", you.2);
                }
                if ended.is_none() {
                    assert_eq!(yous.len(), 1, "{seats} seats, tick {t}, seat {me}: one YOU in the strip");
                    // No other chip says a name that is `YOU`.
                    assert!(said.iter().filter(|s| s.0 == "chip name" && s.1 != "YOU").all(|s| s.1 != "KNIGHT 3"));
                }
                let wins = said.iter().any(|s| s.1 == "YOU WIN" || s.1.ends_with("|YOU WIN"));
                if let Some(e) = ended.filter(|_| seats > 1) {
                    // Told only the winner; the winner is told at some point of the finish.
                    assert!(!wins || me == u32::from(e.winner), "{seats} seats, tick {t}: seat {me} was told YOU WIN, the winner is {}", e.winner);
                    told += usize::from(wins);
                } else {
                    assert!(!wins, "{seats} seats, tick {t}: YOU WIN before the end");
                }
            }
        }
    }
    assert!(told >= 3, "the winner was told in each run ({told})");
}

/// The archive draws through the whole minute and its finish, for a lone
/// knight, a full table and six seats, and the hop it draws is the same
/// twice (the host's and a mirror's are one). Its pixels are the same twice
/// too (`the_archive_pixels_are_the_same_twice`, which wants the process to
/// itself: another test's global switches, the calm screen among them,
/// change a frame).
#[test]
fn the_archive_draws_through_the_whole_minute_the_same_twice() {
    archive_digests(false);
}

#[test]
#[ignore = "pixel determinism: run alone"]
fn the_archive_pixels_are_the_same_twice() {
    archive_digests(true);
}

fn archive_digests(pixels: bool) {
    use std::hash::{Hash, Hasher};
    let digest = |seats: u32| {
        let mut r = fresh_for(seats);
        let mut frames = Vec::new();
        let mut states = Vec::new();
        for t in 0..TITLE + HOP_TIME + 420 {
            let k = keys(&r);
            r.step(&k);
            if t % 29 == 0 || t > TITLE + HOP_TIME && t % 7 == 0 {
                let img = frame_for(&r, 384, 224, Some(1));
                assert_eq!((img.w, img.h), (384, 224));
                let mut h = std::collections::hash_map::DefaultHasher::new();
                img.rgb_bytes().hash(&mut h);
                frames.push((t, h.finish()));
                let mut h = std::collections::hash_map::DefaultHasher::new();
                serde_json::to_string(&r.hop).unwrap().hash(&mut h);
                states.push((t, h.finish()));
            }
            if let Some(hop) = r.hop.as_ref() {
                assert!(hop.valid(), "the hop stays valid at tick {t}");
            }
        }
        let hop = r.hop.as_ref().expect("the hop is still on the books");
        assert!(hop.done && hop.show.ended.is_some(), "{seats} seats: the minute ends with its finish noted (t {}, done {}, at {} in {}, phase {:?}, hold {}, hp {:?}, foes {})", hop.t, hop.done, r.at, hop.room, r.phase, r.hitstop.left, r.players.values().map(|h| h.hp).collect::<Vec<_>>(), r.enemies.len());
        (frames, states)
    };
    for seats in [1, 4, 6] {
        let (a, b) = (digest(seats), digest(seats));
        assert_eq!(a.1, b.1, "{seats} seats: the same hop twice");
        if pixels {
            assert_eq!(a.0, b.0, "{seats} seats: the same picture twice");
        }
    }
}

/// Every seat's strip runs along the hall's foot, clear of the near bank:
/// the strip's top row sits under the bank's last row of flagstones but for
/// the one row of lip.
#[test]
fn the_strip_keeps_to_the_foot_and_the_tray_stays_away() {
    let r = fresh_for(4);
    assert!(crate::render::overworld::arena::hop_tucks_tray(&r), "the skills bar keeps out of the hop's way");
    let bank_foot = (hop::near_bank(r.room().rows as i32) + 1) * 16;
    assert!(crate::render::overworld::arena::hop_strip_top() >= bank_foot - 2, "the strip does not cover the near bank");
}

/// The calls the bots rarely earn, staged on a live run: a run of clean
/// crossings (the flame over the knight and in the strip), the table's
/// archive lit and a change of lead, all at once, on every seat's screen:
/// each is said, whole in the frame and clear of every other word.
#[test]
fn the_rarer_calls_are_drawn_clear_of_every_other_word() {
    use crate::render::overworld::arena::{hop_meets as meets, hop_said, hop_words};
    let mut r = fresh_for(4);
    for _ in 0..TITLE + 120 {
        let k = keys(&r);
        r.step(&k);
    }
    let now = r.tick as u32;
    {
        let h = r.hop.as_mut().unwrap();
        h.show.streak.insert(2, 5);
        h.show.streak.insert(1, 3);
        h.show.shout(now, Cry::Clean(5), 2);
        h.show.shout(now, Cry::Clean(3), 1);
        h.show.shout(now, Cry::Lit(2), 0);
        h.show.shout(now, Cry::Lead, 3);
    }
    let mut said_any = BTreeSet::new();
    for me in 1..=4 {
        let _ = frame_for(&r, 384, 224, Some(me));
        let boxes = hop_words();
        for (i, a) in boxes.iter().enumerate() {
            let ra = a.rect;
            assert!(ra.0 >= 0 && ra.1 >= 0 && ra.0 + ra.2 <= 384 && ra.1 + ra.3 <= 224, "seat {me}: {:?} {ra:?} leaves the frame", a.what);
            for b in &boxes[i + 1..] {
                assert!(!meets(ra, b.rect), "seat {me}: {:?} {ra:?} meets {:?} {:?}", a.what, b.what, b.rect);
            }
        }
        for s in hop_said() {
            said_any.insert(s.1);
        }
        // The strip shows the runs: a streak digit for each of the two.
        let streaks = hop_said().iter().filter(|s| s.0 == "chip streak").count();
        assert_eq!(streaks, 2, "seat {me}: both runs are in the strip");
    }
    assert!(said_any.iter().any(|s| s.starts_with("ARCHIVE LIT")), "the archive is lit: {said_any:?}");
}

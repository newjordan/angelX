//! The lists as the realm draws them: a whole bout driven through the real
//! `Run` by scripted hands at one to six seats, the frames written for a
//! reel (`write_joust_reel`), and every screen of every seat judged for the
//! three rules of words: no two boxes meet, none leaves the 384 by 224
//! canvas, and `YOU` names only the knight looking.

use super::*;
use crate::realm::chivalry::Stable;
use crate::render::overworld::arena::frame_for;
use crate::rules::home::{BUY_HOLD, Home};
use crate::rules::joust::{Aim, Stage};
use crate::rules::{Input, TILE_UNITS, world};
use crate::realm::{Spoil, Spoils};
use crate::rules::RoomKind;
use std::collections::BTreeMap;

type Keys = BTreeMap<u32, Input>;

/// A party of `seats` at the lists with `gold` in the realm's purse: knight
/// 1 on the mount plate, knight 2 nearest to it, any more further off.
pub(super) fn party(seats: u32, gold: u32) -> Run {
    let home = Home { stable: Stable::default(), ..Home::default() };
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, gold);
    let mut run = Run::at_home(7, 1, None, home, treasury);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let lists = world::room_of(&run.dungeon, RoomKind::Lists).unwrap();
    run.arrive(lists, (1.9, 8.5));
    for id in 1..=seats {
        let h = run.players.get_mut(&id).unwrap();
        h.invulnerable = 0;
        (h.x, h.y) = match id {
            1 => (1.9 * TILE_UNITS, 8.5 * TILE_UNITS),
            n => ((3.0 + 5.0 * n as f32) * TILE_UNITS / 2.0, 21.0),
        };
    }
    run.step(&Keys::new());
    run
}

pub(super) fn mount(run: &mut Run) {
    let press = Keys::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&press);
    }
    run.step(&Keys::new());
    assert!(run.joust.is_some(), "mounted at the plate");
}

pub(super) const NAMES: [&str; 6] = ["Gawain", "Rowan", "Elaine", "You", "", "Piers"];

pub(super) fn named(run: &mut Run) {
    for (i, name) in NAMES.iter().enumerate() {
        if let Some(h) = run.players.get_mut(&(i as u32 + 1)) {
            h.name = (*name).into();
        }
    }
}

/// How a rider rides a pass.
#[derive(Clone, Copy)]
pub(super) struct Plan {
    pub aim: Aim,
    pub shield: Aim,
    /// Ticks against the meeting the strike falls (none: never).
    pub strike: Option<i32>,
    /// Ticks into the ready stage before spurring.
    pub spur: u32,
}

/// One rider's keys this tick, from his plan.
pub(super) fn rider_keys(run: &Run, id: u32, plan: Plan, e_was: &mut bool) -> Input {
    let Some(j) = run.joust.as_ref() else { return Input::default() };
    let foe = j.foe.as_ref().filter(|f| f.id == id);
    let (shield, spurred) = match foe {
        Some(f) => (f.shield, f.spurred),
        None => (j.shield, j.spurred),
    };
    match j.stage {
        Stage::Ready => Input { fire: !spurred && j.t >= plan.spur && j.t % 2 == 0, ..Input::default() },
        Stage::Charge => {
            let next = j.t + 1;
            let mut input = Input { move_y: if plan.aim == Aim::High { -1 } else { 1 }, ..Input::default() };
            if shield != plan.shield && !*e_was {
                input.interact = true;
            }
            *e_was = input.interact;
            if let Some(off) = plan.strike
                && next as i32 == j.meeting() as i32 + off
            {
                input.fire = true;
            }
            input
        }
        Stage::Done => Input::default(),
    }
}

/// The gallery's hands: back a rider and raise before the flag, throw a
/// favour as the riders close.
pub(super) fn gallery_keys(run: &Run, id: u32, backs: u8, cheer_at: u32) -> Input {
    let Some(j) = run.joust.as_ref() else { return Input::default() };
    match j.stage {
        Stage::Ready if j.course == 0 => {
            let w = j.gallery.get(&id);
            let side_set = w.is_some_and(|w| w.side.is_some());
            if !side_set && j.t >= 4 + id {
                return Input { move_x: if backs == 0 { -1 } else { 1 }, ..Input::default() };
            }
            if side_set && j.t % 8 == (id % 4) && w.is_some_and(|w| w.stake < 20) {
                return Input { fire: true, ..Input::default() };
            }
            Input::default()
        }
        Stage::Charge if j.t == cheer_at => Input { fire: true, ..Input::default() },
        _ => Input::default(),
    }
}

/// Frames of one scene on disk.
struct Reel {
    dir: std::path::PathBuf,
    n: u32,
    seats: Vec<u32>,
}

impl Reel {
    fn new(root: &std::path::Path, name: &str, seats: &[u32]) -> Reel {
        let dir = root.join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Reel { dir, n: 0, seats: seats.to_vec() }
    }

    fn shot(&mut self, run: &Run) {
        let mut sheet = Img::black(384 * self.seats.len() as i32, 224);
        for (i, &seat) in self.seats.iter().enumerate() {
            if run.players.contains_key(&seat) {
                sheet.stamp(&frame_for(run, 384, 224, Some(seat)), 384 * i as i32, 0);
            }
        }
        image::save_buffer(
            self.dir.join(format!("f{:04}.png", self.n)),
            &sheet.rgb_bytes(),
            sheet.w as u32,
            sheet.h as u32,
            image::ColorType::Rgb8,
        )
        .unwrap();
        self.n += 1;
    }
}

/// The plans a scripted bout rides, pass after pass: the knight's and the
/// friend's. The first pass trades a clean hit for a shield; the second a
/// mighty hit for a miss and the foe goes over.
fn plans(course: u8, variant: u8) -> (Plan, Plan) {
    match (variant, course) {
        // The foe wins: his lance finds the knight's open side and the
        // knight never strikes.
        (1, _) => (
            Plan { aim: Aim::Low, shield: Aim::Low, strike: None, spur: 20 },
            Plan { aim: Aim::High, shield: Aim::Low, strike: Some(0), spur: 24 },
        ),
        // Nobody strikes: three passes ridden for nothing.
        (2, _) => (
            Plan { aim: Aim::Low, shield: Aim::Low, strike: None, spur: 14 },
            Plan { aim: Aim::Low, shield: Aim::Low, strike: None, spur: 16 },
        ),
        (_, c) => plans_a(c),
    }
}

fn plans_a(course: u8) -> (Plan, Plan) {
    match course {
        0 => (
            Plan { aim: Aim::High, shield: Aim::Low, strike: Some(0), spur: 50 },
            Plan { aim: Aim::Low, shield: Aim::Low, strike: Some(1), spur: 70 },
        ),
        1 => (
            Plan { aim: Aim::Low, shield: Aim::High, strike: Some(0), spur: 10 },
            Plan { aim: Aim::Low, shield: Aim::Low, strike: None, spur: 14 },
        ),
        _ => (
            Plan { aim: Aim::High, shield: Aim::Low, strike: Some(0), spur: 10 },
            Plan { aim: Aim::Low, shield: Aim::High, strike: Some(0), spur: 12 },
        ),
    }
}

/// Drive one scripted bout at `seats` seats, handing every tick to `each`.
pub(super) fn ride(seats: u32, variant: u8, mut each: impl FnMut(&Run, u32)) -> Run {
    let mut run = party(seats, 300);
    named(&mut run);
    mount(&mut run);
    let (mut ea, mut eb) = (false, false);
    let mut t = 0u32;
    while run.joust.is_some() && t < 3000 {
        let Some(j) = run.joust.as_ref() else { break };
        let (a, b) = plans(j.course, variant);
        let mut keys = Keys::new();
        keys.insert(1, rider_keys(&run, 1, a, &mut ea));
        if seats >= 2 {
            keys.insert(2, rider_keys(&run, 2, b, &mut eb));
        }
        for id in 3..=seats {
            let cheer = if j.course == 1 { u32::MAX } else { j.meeting() / 2 + id };
            keys.insert(id, gallery_keys(&run, id, (id % 2) as u8, cheer));
        }
        run.step(&keys);
        t += 1;
        each(&run, t);
    }
    run
}

/// `ANGEL_JOUST_REEL=<dir> cargo test -p angel-delve --lib write_joust_reel -- --ignored --nocapture`
/// writes `<dir>/<name>/f0000.png ...` at 30 fps, seats side by side.
#[test]
#[ignore = "writes the reel"]
fn write_joust_reel() {
    let Some(root) = std::env::var_os("ANGEL_JOUST_REEL") else { return };
    let root = std::path::PathBuf::from(root);
    let seats: u32 = std::env::var("JOUST_SEATS").ok().and_then(|s| s.parse().ok()).unwrap_or(4);
    let views: Vec<u32> = std::env::var("JOUST_VIEWS")
        .ok()
        .map(|s| s.split(',').filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_else(|| vec![1]);
    let name = std::env::var("JOUST_NAME").unwrap_or_else(|_| "match".into());
    let variant: u8 = std::env::var("JOUST_VARIANT").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut reel = Reel::new(&root, &name, &views);
    let mut last = (Stage::Ready, 99u8, false);
    ride(seats, variant, |run, _| {
        if let Some(j) = run.joust.as_ref() {
            let now = (j.stage, j.course, j.hits.is_some());
            if now != last {
                println!("frame {}: {:?} course {} hits {:?} t {}", reel.n, j.stage, j.course, j.hits, j.t);
                last = now;
            }
        }
        reel.shot(run)
    });
    println!("{name}: {} frames", reel.n);
}

// ── The words ─────────────────────────────────────────────────────────────

use super::hud;

/// What the seat looking should read for knight `id`, worked out on its own.
fn expected(run: &Run, id: u32, me: u32, take: usize) -> String {
    if id == me {
        return "YOU".into();
    }
    let name = run.players[&id].name.trim().to_uppercase();
    if name.is_empty() || name == "YOU" { format!("KNIGHT {id}") } else { name.chars().take(take).collect() }
}

/// Draw `me`'s screen and judge the words on it.
pub(super) fn judge(run: &Run, me: u32, what: &str) {
    let _ = frame_for(run, 384, 224, Some(me));
    let all = hud::last();
    let j = run.joust.as_ref().expect("a bout");
    for c in &all {
        assert!(hud::inside(c.rect), "{what}: {} {:?} {:?} leaves 384x224", c.what, c.text, c.rect);
    }
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert!(!hud::meets(a.rect, b.rect), "{what}: {} {:?} {:?} meets {} {:?} {:?}", a.what, a.text, a.rect, b.what, b.text, b.rect);
        }
    }
    let rider = |side: u8| j.rider(side);
    let viewer_rides = rider(0) == Some(me) || rider(1) == Some(me);
    for c in &all {
        let words: Vec<&str> = c.text.split(' ').collect();
        // The chips name the riders: the viewer YOU, nobody else.
        if c.what == "chip" {
            let side = if c.rect.0 < 192 { 0 } else { 1 };
            let want = match rider(side) {
                Some(id) => expected(run, id, me, 10),
                None => j.rival().name.to_uppercase().chars().take(10).collect(),
            };
            assert_eq!(c.text, want, "{what}: the chip of side {side}");
        }
        if words.contains(&"YOU") {
            match c.what {
                "chip" | "podium-name" | "gallery" | "status" | "title" | "call" | "verdict" | "wager" | "prompts" => {}
                other => panic!("{what}: {other} {:?} says YOU", c.text),
            }
            assert!(
                run.players.contains_key(&me),
                "{what}: {} {:?} says YOU to a seat that is not at the table",
                c.what, c.text
            );
            // Only a rider is told he is down, or that he wins.
            if c.text.contains("YOU ARE DOWN") || c.text.starts_with("YOU WIN") {
                assert!(viewer_rides, "{what}: {:?} said to a seat that is not riding", c.text);
            }
            if c.text.contains("YOU ARE DOWN") {
                let down = if j.fallen.0 { rider(0) } else { rider(1) };
                assert_eq!(down, Some(me), "{what}: {:?}", c.text);
            }
        }
        // A KNIGHT n is a real seat with the default name, never the viewer.
        let numbered = words.iter().position(|w| *w == "KNIGHT").filter(|at| words.get(at + 1).is_some_and(|w| w.parse::<u32>().is_ok()));
        if let Some(at) = numbered {
            let id: u32 = words.get(at + 1).and_then(|w| w.parse().ok()).unwrap_or(0);
            assert!(id != me && run.players.contains_key(&id), "{what}: {} {:?} calls a knight who is not there", c.what, c.text);
            assert_eq!(expected(run, id, me, 10), format!("KNIGHT {id}"), "{what}: {:?}", c.text);
        }
    }
    // The gallery lines name each knight in the gallery by their own name.
    let tags: Vec<&hud::Claim> = all.iter().filter(|c| c.what == "gallery").collect();
    for c in tags {
        let who = c.text.split(' ').next().unwrap_or("");
        let known: Vec<String> = j.gallery.keys().map(|&id| expected(run, id, me, 8)).collect();
        let first_two: String = c.text.split(' ').take(2).collect::<Vec<_>>().join(" ");
        assert!(
            known.iter().any(|k| c.text.starts_with(k.as_str())) || known.iter().any(|k| first_two == *k),
            "{what}: gallery line {:?} ({who}) is not one of {known:?}",
            c.text
        );
    }
}

/// Whole bouts at one to six seats: on every sampled tick, on a few seats'
/// own screens, the three rules hold.
#[test]
fn the_lists_words_never_meet_and_yous_belong_to_the_viewer() {
    for (seats, variant) in (1..=6u32).flat_map(|n| (0..3u8).map(move |v| (n, v))) {
        let views: Vec<u32> = match seats {
            1 => vec![1],
            2 => vec![1, 2],
            n => vec![1, 2, n],
        };
        let mut seen = 0;
        let run = ride(seats, variant, |run, t| {
            if run.joust.is_none() {
                return;
            }
            let every = if run.joust.as_ref().is_some_and(|j| j.stage == Stage::Done) { 4 } else { 3 };
            if t % every == 0 {
                for &v in &views {
                    judge(run, v, &format!("{seats} seats, variant {variant}, seat {v}, tick {t}"));
                    seen += 1;
                }
            }
        });
        assert!(run.joust.is_none(), "{seats} seats, variant {variant}: the bout ends");
        assert!(seen > 90, "{seats} seats, variant {variant}: {seen} screens judged");
    }
}

/// `cargo test -p angel-delve --lib lists_frame_time -- --ignored --nocapture`:
/// what a seat's screen costs mid-bout, the room as it was against the lists
/// as they are now, both drawn from every state of sixty ticks of the charge
/// (the quickest of five draws each, so a busy box moves them together), the
/// mean of those.
#[test]
#[ignore = "times the screen"]
fn lists_frame_time() {
    let mut run = party(4, 300);
    named(&mut run);
    mount(&mut run);
    let (mut ea, mut eb) = (false, false);
    let mut t = 0;
    while run.joust.as_ref().is_some_and(|j| j.stage != Stage::Charge) && t < 400 {
        let j = run.joust.as_ref().unwrap();
        let (a, b) = plans(j.course, 0);
        let mut keys = Keys::new();
        keys.insert(1, rider_keys(&run, 1, a, &mut ea));
        keys.insert(2, rider_keys(&run, 2, b, &mut eb));
        run.step(&keys);
        t += 1;
    }
    let draw = |run: &Run, old: bool| {
        super::OLD_ROOM.with(|o| o.set(old));
        let mut quickest = f64::MAX;
        for _ in 0..5 {
            let at = std::time::Instant::now();
            let im = frame_for(run, 384, 224, Some(1));
            quickest = quickest.min(at.elapsed().as_secs_f64() * 1000.0);
            assert_eq!((im.w, im.h), (384, 224));
        }
        super::OLD_ROOM.with(|o| o.set(false));
        quickest
    };
    let _ = (draw(&run, true), draw(&run, false));
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for _ in 0..110 {
        before.push(draw(&run, true));
        after.push(draw(&run, false));
        let Some(j) = run.joust.as_ref() else { break };
        let (a, b) = plans(j.course, 0);
        let mut keys = Keys::new();
        keys.insert(1, rider_keys(&run, 1, a, &mut ea));
        keys.insert(2, rider_keys(&run, 2, b, &mut eb));
        run.step(&keys);
    }
    let mean = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v.iter().sum::<f64>() / v.len() as f64
    };
    let (b, a) = (mean(&mut before), mean(&mut after));
    println!("the room as it was: mean {b:.2} ms (cheapest {:.2}, dearest {:.2})", before[0], before[before.len() - 1]);
    println!("the lists now: mean {a:.2} ms (cheapest {:.2}, dearest {:.2})", after[0], after[after.len() - 1]);
    println!("ratio {:.2}", a / b);
}

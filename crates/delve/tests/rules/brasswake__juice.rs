//! Brasswake's juice: frame times, the words test and the proof reels.
use super::*;
use crate::render::overworld::arena::frame_for;
use crate::rules::brasswake::LENGTH;
use std::time::Instant;

type Keys = BTreeMap<u32, Input>;

fn table(seats: u32) -> Run {
    let mut r = Run::brasswake_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    if seats == 1 {
        r.leave(2);
    }
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    assert!(r.brasswake.is_some(), "the game did not open");
    r
}

fn bots(r: &Run) -> Keys {
    r.brasswake.as_ref().map_or_else(Keys::new, |s| s.lanes.iter().map(|(&id, l)| (id, l.bot_input(LENGTH - s.left))).collect())
}

/// How long a seat's screen takes to draw over a bot's round: mean and worst of the middle nine tenths.
#[test]
#[ignore = "measures the picture's cost"]
fn brasswake_frame_times() {
    let mut r = table(4);
    let mut times = Vec::new();
    for t in 0..1500u32 {
        let k = bots(&r);
        r.step(&k);
        if t % 3 == 0 {
            let at = Instant::now();
            let im = frame_for(&r, 384, 224, Some(1));
            times.push(at.elapsed().as_secs_f64() * 1000.0);
            assert_eq!((im.w, im.h), (384, 224));
        }
    }
    times.drain(..6);
    times.sort_by(|a, b| a.total_cmp(b));
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let p95 = times[times.len() * 95 / 100];
    println!("brasswake frame: mean {mean:.3} ms, p95 {p95:.3} ms over {} frames", times.len());
}

// ─── the proof: reels of the real run ────────────────────────────────────────

use crate::render::overworld::Img;
use std::path::{Path, PathBuf};

struct Reel {
    dir: PathBuf,
    n: u32,
    seats: Vec<u32>,
}

impl Reel {
    fn new(root: &Path, name: &str, seats: &[u32]) -> Reel {
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
        image::save_buffer(self.dir.join(format!("f{:04}.png", self.n)), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
        self.n += 1;
    }
}

/// What a lane's bot does, but late: a seat idles for `lag` ticks of the round first so the seats differ.
fn lagged(r: &Run, lag: &dyn Fn(u32) -> u32) -> Keys {
    let Some(s) = r.brasswake.as_ref() else { return Keys::new() };
    let elapsed = LENGTH - s.left;
    s.lanes.iter().map(|(&id, l)| (id, if elapsed < lag(id) { Input::default() } else { l.bot_input(elapsed) })).collect()
}

/// One bot-driven table, recording seats' own screens over windows of the round.
/// `windows`: (name, first tick, last tick, step), ticks counted from the press of E.
fn run_reels(root: &Path, seats: u32, windows: &[(&str, u32, u32, &[u32])], mut tweak: impl FnMut(&mut Run, u32)) {
    let mut r = table(seats);
    let mut reels: Vec<(u32, u32, Reel)> = windows.iter().map(|&(n, a, b, shown)| (a, b, Reel::new(root, n, shown))).collect();
    let end = windows.iter().map(|w| w.2).max().unwrap_or(0);
    for t in 0..=end {
        tweak(&mut r, t);
        for (a, b, reel) in &mut reels {
            if t >= *a && t <= *b {
                reel.shot(&r);
            }
        }
        let k = lagged(&r, &|id| (id - 1) * 18);
        r.step(&k);
    }
    for (_, _, reel) in &reels {
        println!("{}: {} frames", reel.dir.display(), reel.n);
    }
}

/// A day's log of the bot round: the ticks of the moments, to choose windows from.
#[test]
#[ignore = "writes the proof"]
fn brasswake_timeline() {
    let mut r = table(4);
    let mut seen = std::collections::BTreeSet::new();
    for t in 0..LENGTH {
        let k = lagged(&r, &|id| (id - 1) * 18);
        r.step(&k);
        let Some(s) = r.brasswake.as_ref() else { break };
        for (&id, d) in &s.lanes {
            for m in &d.show.marks {
                if m.born + 1 == r.tick as u32 {
                    let tag = format!("{:?}", m.kind).split(|c: char| !c.is_alphanumeric()).next().unwrap().to_string();
                    if id == 1 && seen.insert((tag.clone(), t / 30)) && !matches!(tag.as_str(), "Step") {
                        println!("t={t} seat {id} {:?}", m.kind);
                    }
                }
            }
        }
        for c in &s.show.list {
            if c.born + 1 == r.tick as u32 {
                println!("t={t} CALL {:?} who {}", c.cry, c.who);
            }
        }
        if s.left == 0 {
            println!("t={t} END");
            break;
        }
    }
}

/// The reels of a bot-driven table. `BRASS_WIN` names the windows as `name:first:last:seats`, comma
/// separated, ticks counted from the press of E and seats as digits (`12` shows seats one and two).
#[test]
#[ignore = "writes the proof"]
fn write_brasswake_proof() {
    let Some(root) = std::env::var_os("ANGEL_BRASS_SHOTS") else { return };
    let root = PathBuf::from(root);
    let spec = std::env::var("BRASS_WIN").unwrap_or_else(|_| "start:0:130:1,brawl:150:330:1,cask:330:520:12".into());
    let owned: Vec<(String, u32, u32, Vec<u32>)> = spec
        .split(',')
        .map(|w| {
            let p: Vec<&str> = w.split(':').collect();
            (p[0].to_string(), p[1].parse().unwrap(), p[2].parse().unwrap(), p[3].chars().map(|c| c.to_digit(10).unwrap()).collect())
        })
        .collect();
    let windows: Vec<(&str, u32, u32, &[u32])> = owned.iter().map(|(n, a, b, s)| (n.as_str(), *a, *b, s.as_slice())).collect();
    let seats = std::env::var("BRASS_SEATS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    run_reels(&root, seats, &windows, |_, _| {});
}

/// The end screen: a posed finish, stepped on so its clock runs.
#[test]
#[ignore = "writes the proof"]
fn write_brasswake_podium() {
    let Some(root) = std::env::var_os("ANGEL_BRASS_SHOTS") else { return };
    let seats: usize = std::env::var("BRASS_SEATS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let mut r = Run::brasswake_review_for_test("podium");
    r.brasswake.as_mut().unwrap().broke = true;
    let mut reel = Reel::new(&PathBuf::from(root), "podium", &[1]);
    let _ = seats;
    for t in 0..260 {
        reel.shot(&r);
        r.step(&Keys::new());
        let _ = t;
    }
    println!("podium: {} frames", reel.n);
}

/// A posed moment of the review set, stepped on a little so its marks and clocks run.
#[test]
#[ignore = "writes the proof"]
fn write_brasswake_moments() {
    let Some(root) = std::env::var_os("ANGEL_BRASS_SHOTS") else { return };
    let root = PathBuf::from(root);
    for (moment, ticks) in [("slam-tell", 70u32), ("rush-tell", 70), ("kick", 40), ("defeat", 40), ("throw", 70), ("down", 50)] {
        let mut r = Run::brasswake_review_for_test(moment);
        let mut reel = Reel::new(&root, &format!("moment-{moment}"), &[1]);
        for _ in 0..ticks {
            reel.shot(&r);
            let k = r.brasswake.as_ref().map_or_else(Keys::new, |s| s.lanes.iter().map(|(&id, _)| (id, Input::default())).collect());
            r.step(&k);
        }
        println!("moment {moment}: {} frames", reel.n);
    }
}

// ─── the show: marks, weights, calls ─────────────────────────────────────────

use crate::rules::brasswake::{
    Beat, Foe, Lane, Session,
    show::{Cry, Kind},
};
use crate::rules::juice::Weight;
use crate::rules::tune::Tune;

fn close_lane() -> Lane {
    let mut d = Lane::new(41);
    d.x = 150;
    d.y = 150;
    d.foes[0].x = 180;
    d.foes[0].y = 150;
    d.foes[1].x = 240;
    d.foes[1].y = 174;
    d
}

#[test]
fn a_jab_a_cross_and_a_kick_are_three_weights_and_the_kick_holds_the_aisle_longest() {
    let t = Tune::default();
    let mut d = close_lane();
    let mut now = 0u64;
    let mut holds = Vec::new();
    for n in 0..3u8 {
        d.step_at(Input { fire: true, ..Default::default() }, &t, now, 0, now as u32);
        let mark = d.show.marks.iter().rev().find(|m| matches!(m.kind, Kind::Blow { .. })).copied().unwrap();
        let Kind::Blow { swing, slain, gain, .. } = mark.kind else { unreachable!() };
        assert_eq!(swing, n);
        assert!(!slain);
        assert_eq!(gain, 10);
        assert_eq!(mark.kind.weight(), [Weight::Graze, Weight::Solid, Weight::Crushing][n as usize]);
        holds.push(d.freeze);
        assert!(d.valid());
        for _ in 0..t.brass_combo + 1 {
            now += 1;
            d.foes[0].x = d.x + 30;
            d.step_at(Input::default(), &t, now, 0, now as u32);
        }
        now += 1;
    }
    // (The count is read after the tick that set it ran once.)
    assert_eq!(holds[0], t.brass_stop - 1, "a jab holds as long as the knob says");
    assert_eq!(holds[1], t.brass_stop - 1);
    assert_eq!(holds[2], t.brass_stop, "a kick holds a tick longer, on this aisle only");
}

#[test]
fn a_kill_pays_its_bounty_in_the_mark_and_a_keeper_falls_as_a_finale() {
    let t = Tune::default();
    let mut d = close_lane();
    d.foes[0].hp = 2;
    d.step_at(Input { fire: true, ..Default::default() }, &t, 0, 0, 0);
    let m = d.show.marks.iter().find(|m| matches!(m.kind, Kind::Blow { .. })).unwrap();
    assert!(matches!(m.kind, Kind::Blow { slain: true, boss: false, gain: 110, .. }));
    assert_eq!(m.kind.weight(), Weight::Kill);
    assert_eq!(d.foes[0].beat, Beat::Defeat);
    assert_eq!(d.freeze, t.brass_stop + 1);
    // The fallen count the ticks they have lain.
    for n in 1..=4u64 {
        d.step_at(Input::default(), &t, n * 3, 0, 0);
    }
    assert!(d.foes[0].ticks > 0 && d.valid());
    // A keeper.
    let mut k = Lane::new(41);
    k.wave = 3;
    k.x = 700;
    k.y = 150;
    k.foes = vec![Foe::new(730, 150, true)];
    k.foes[0].hp = 2;
    k.step_at(Input { fire: true, ..Default::default() }, &t, 0, 0, 0);
    let m = k.show.marks.iter().find(|m| matches!(m.kind, Kind::Blow { .. })).unwrap();
    assert!(matches!(m.kind, Kind::Blow { slain: true, boss: true, gain: 610, .. }));
    assert_eq!(m.kind.weight(), Weight::Finale);
    assert_eq!(k.freeze, t.brass_stop + 3);
}

#[test]
fn a_taken_blow_a_slam_and_a_broken_cask_each_leave_a_mark() {
    let t = Tune::default();
    let mut d = close_lane();
    d.foes[0].beat = Beat::Tell;
    d.foes[0].ticks = 1;
    d.foes[0].danger = crate::rules::brasswake::Danger { x: 150, y: 140, w: 48, h: 28 };
    d.step_at(Input::default(), &t, 10, 0, 10);
    assert!(d.show.marks.iter().any(|m| matches!(m.kind, Kind::Quake { hit: true, rush: false, .. })));
    assert!(d.show.marks.iter().any(|m| matches!(m.kind, Kind::Hurt { guarded: false, .. })));
    assert_eq!(d.hp, 5);
    // A cask that lands on the mark.
    let mut e = close_lane();
    e.incoming.push(crate::rules::brasswake::Cask {
        danger: crate::rules::brasswake::Danger { x: 140, y: 138, w: 40, h: 24 },
        tell: 1,
        burst: 0,
        from: 2,
    });
    e.step_at(Input::default(), &t, 20, 0, 20);
    assert!(e.show.marks.iter().any(|m| matches!(m.kind, Kind::Smash { hit: true, from: 2 })));
    // Marks go when they are old, and the book stays inside its bounds.
    for n in 0..60u64 {
        e.step_at(Input::default(), &t, 21 + n, 0, 21);
    }
    assert!(e.show.marks.iter().all(|m| m.age(80) < 48));
    assert!(e.valid());
}

#[test]
fn the_table_hears_who_leads_who_reached_the_keeper_and_a_team_kick() {
    let mut s = Session::new(1..=3, 41);
    s.lanes.get_mut(&2).unwrap().score = 50;
    s.call_the_table(100);
    assert_eq!(s.show.leader, Some(2));
    assert!(s.show.list.iter().any(|c| c.cry == Cry::Lead && c.who == 2));
    // No change of lead, no second call.
    s.call_the_table(101);
    assert_eq!(s.show.list.iter().filter(|c| c.cry == Cry::Lead).count(), 1);
    s.lanes.get_mut(&3).unwrap().score = 90;
    s.call_the_table(102);
    assert!(s.show.list.iter().any(|c| c.cry == Cry::Lead && c.who == 3));
    // The first to reach the keeper is called once.
    s.lanes.get_mut(&1).unwrap().wave = 3;
    s.lanes.get_mut(&1).unwrap().foes = vec![Foe::new(920, 150, true)];
    s.call_the_table(103);
    s.lanes.get_mut(&2).unwrap().wave = 3;
    s.lanes.get_mut(&2).unwrap().foes = vec![Foe::new(920, 150, true)];
    s.call_the_table(104);
    assert_eq!(s.show.list.iter().filter(|c| c.cry == Cry::Keeper).count(), 1);
    assert_eq!(s.show.first_keeper, Some(1));
    // Two aisles kick within ten ticks of each other: a team kick, once, then it rests.
    assert!(!s.show.kicked(1, 200));
    assert!(s.show.kicked(2, 205));
    assert!(!s.show.kicked(3, 207), "a team kick rests");
    assert!(s.show.kicked(1, 205 + 95) || s.show.kicked(2, 205 + 96));
    assert!(s.valid());
}

#[test]
fn the_show_rides_the_wire_and_an_old_snapshot_loads_with_a_quiet_one() {
    let mut r = table(4);
    for _ in 0..90 {
        let k = bots(&r);
        r.step(&k);
    }
    let s = r.brasswake.as_ref().unwrap();
    assert!(s.valid());
    assert!(s.lanes.values().any(|d| !d.show.quiet()), "ninety ticks of bots leave marks");
    let json = serde_json::to_string(s).unwrap();
    let back: Session = serde_json::from_str(&json).unwrap();
    assert_eq!(serde_json::to_string(&back).unwrap(), json);
    // A snapshot from before the show: the fields are simply missing.
    let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
    let o = old.as_object_mut().unwrap();
    o.remove("show");
    o.remove("done");
    o.remove("broke");
    for lane in o["lanes"].as_object_mut().unwrap().values_mut() {
        lane.as_object_mut().unwrap().remove("show");
    }
    let loaded: Session = serde_json::from_value(old).unwrap();
    assert!(loaded.valid() && loaded.show.quiet());
    assert!(loaded.lanes.values().all(|d| d.show.quiet()));
    // A quiet lane writes nothing.
    let fresh = serde_json::to_string(&Lane::new(41)).unwrap();
    assert!(!fresh.contains("\"show\""));
}

#[test]
fn a_whole_bot_round_ends_with_marks_calls_and_a_best_that_can_be_beaten() {
    let mut r = table(4);
    let mut kinds = std::collections::BTreeSet::new();
    let mut heard = std::collections::BTreeSet::new();
    for _ in 0..LENGTH {
        let k = lagged(&r, &|id| (id - 1) * 18);
        r.step(&k);
        let Some(s) = r.brasswake.as_ref() else { break };
        assert!(s.valid(), "tick {}", r.tick);
        for d in s.lanes.values() {
            for m in &d.show.marks {
                kinds.insert(format!("{:?}", m.kind).split(|c: char| !c.is_alphanumeric()).next().unwrap().to_string());
            }
        }
        heard.extend(s.show.list.iter().map(|c| format!("{:?}", c.cry)));
        if s.left == 0 {
            break;
        }
    }
    for want in ["Blow", "Step", "Clear", "Grab", "Throw", "Smash", "Finish", "Streak", "Quake"] {
        assert!(kinds.contains(want), "no {want} mark in a bot round: {kinds:?}");
    }
    assert!(heard.contains("Keeper") && heard.contains("Slain"), "{heard:?}");
    let s = r.brasswake.as_ref().unwrap();
    assert_eq!(s.left, 0);
    assert!(s.broke, "the first round beats an empty ledger");
    assert!(s.done > 0);
}

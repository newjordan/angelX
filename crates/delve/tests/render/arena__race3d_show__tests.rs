//! The races' juice: the start, the wind, the wrecks, the calls, the photo
//! finish and the podium, as the picture draws them from the race's records.
use super::*;
use super::testing::*;
use super::{finish, moments};
use crate::rules::race3d::{show::Kind, Wreck};
use crate::rules::Input;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Keys = BTreeMap<u32, Input>;

fn race_mut(run: &mut Run) -> &mut Race3d {
    run.race3d.as_mut().unwrap()
}

fn race_ref(run: &Run) -> &Race3d {
    run.race3d.as_ref().unwrap()
}

fn place(run: &mut Run, id: u32, s: f32, x: f32, z: f32, vs: f32) {
    let r = race_mut(run).riders.get_mut(&id).unwrap();
    (r.s, r.x, r.z, r.vs) = (s, x, z, vs);
}

fn frame(run: &Run, seat: u32) -> Img {
    super::super::frame_for(run, NATIVE_W, NATIVE_H, Some(seat))
}

#[test]
fn the_start_is_lamps_then_green_then_gone() {
    for course in [Course::Shaft, Course::Cart, Course::Griffin] {
        let mut run = racing(course, 3);
        let mut shots = Vec::new();
        for t in [2, HZ + 4, 2 * HZ + 4, TITLE + 3, TITLE + 40] {
            race_mut(&mut run).t = t;
            shots.push(frame(&run, 1));
        }
        for pair in shots.windows(2) {
            assert_ne!(pair[0], pair[1], "{course:?}: every beat of the start looks like its own");
        }
        // The lamps are over the road in the middle of the screen.
        let lamps = shots[0].get(192 - 30, 68);
        assert!(lamps.is_some());
    }
}

#[test]
fn a_wreck_is_written_down_and_holds_the_room() {
    let mut run = racing(Course::Griffin, 2);
    let t = r3::track(Course::Griffin);
    // Straight at the first spire.
    place(&mut run, 1, t.posts[1] + 22.0 - 9.0, -2.5, 0.0, 10.0);
    place(&mut run, 2, t.posts[1] + 22.0 - 30.0, 3.0, 0.0, 10.0);
    let mut held = false;
    for _ in 0..40 {
        run.step(&Keys::new());
        held |= run.hitstop.left > 0;
        if race_ref(&run).show.since(Kind::Wreck, 1, race_ref(&run).t).is_some() {
            break;
        }
    }
    let race = race_ref(&run);
    let wreck = race.show.of(Kind::Wreck, race.t).find(|f| f.seat == 1).expect("the spire was met");
    assert_eq!(wreck.wreck, Wreck::Tumble);
    assert!(wreck.weight >= crate::rules::juice::Weight::Crushing, "a griffin at pace hits hard: {:?}", wreck.weight);
    assert!(held || run.hitstop.left > 0, "a hard wreck holds the room a moment");
    // The frame of the wreck is not the frame before it.
    let before = frame(&run, 1);
    race_mut(&mut run).show.fx.clear();
    assert_ne!(before, frame(&run, 1), "the wreck's own effects are in the picture");
}

#[test]
fn the_show_never_goes_to_the_module() {
    // The leg's ask is the race as the saves hold it: a show with records in
    // it must be set aside, or the module would be asked a thing it has no
    // field for. The oracle (which reads the same ask) answers the same.
    let mut run = racing(Course::Cart, 2);
    for _ in 0..20 {
        run.step(&Keys::new());
    }
    race_mut(&mut run).show.fx.push(crate::rules::race3d::show::Fx {
        kind: Kind::Hop,
        seat: 1,
        other: 0,
        n: 0,
        at: [1.0, 0.0, 0.0],
        weight: crate::rules::juice::Weight::Graze,
        wreck: Wreck::None,
        born: 1,
    });
    run.step(&Keys::new());
    assert!(race_ref(&run).valid(), "the race still holds with a show on it");
}

#[test]
fn the_finish_holds_the_frame_then_the_podium_takes_the_screen() {
    let mut run = racing(Course::Cart, 3);
    let finish = r3::track(Course::Cart).finish;
    for (k, id) in [1u32, 2, 3].into_iter().enumerate() {
        place(&mut run, id, finish - 3.0 - k as f32 * 1.6, 0.0, 0.0, 8.5);
        race_mut(&mut run).riders.get_mut(&id).unwrap().lane = 1;
    }
    let mut frozen = None;
    let mut podium = None;
    for _ in 0..(60 + r3::PODIUM) {
        run.step(&Keys::new());
        let race = race_ref(&run);
        if race.done {
            let age = race.t - race.ended;
            if age == 20 {
                frozen = Some(frame(&run, 1));
            }
            if age == 100 {
                podium = Some(frame(&run, 1));
            }
        }
    }
    let (frozen, podium) = (frozen.expect("a held frame"), podium.expect("a podium"));
    assert_ne!(frozen, podium);
    let race = race_ref(&run);
    assert!(race.show.fx.iter().any(|f| f.kind == Kind::Finish), "the line was written down");
}

#[test]
fn only_the_viewers_own_place_reads_you_whoever_won() {
    for seats in 2..=4u32 {
        let mut run = racing(Course::Cart, seats);
        let finish = r3::track(Course::Cart).finish;
        // The last seat is first home, so the host (a knight named "You", the
        // game's default) is nowhere near the front.
        for id in 1..=seats {
            place(&mut run, id, finish - 3.0 - (seats - id) as f32 * 1.6, 0.0, 0.0, 8.5);
            race_mut(&mut run).riders.get_mut(&id).unwrap().lane = 1;
        }
        for _ in 0..200 {
            run.step(&Keys::new());
            if race_ref(&run).done {
                break;
            }
        }
        assert!(race_ref(&run).done, "{seats} seats: everyone came home");
        assert_eq!(run.players[&1].name, "You", "the host still carries the default name");
        for viewer in 1..=seats {
            let race = race_ref(&run);
            let stand = finish::standings(&run, race, Some(viewer), 9);
            let yous: Vec<u32> = stand.iter().filter(|(_, n)| n == "YOU").map(|&(id, _)| id).collect();
            assert_eq!(yous, vec![viewer], "{seats} seats, seat {viewer}'s screen: YOU is the viewer's own place and only that: {stand:?}");
            let mut names: Vec<&String> = stand.iter().map(|(_, n)| n).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), stand.len(), "every knight has a name of their own: {stand:?}");
            let winner = stand[0].0;
            let head = finish::headline(&run, winner, Some(viewer));
            assert!(!head.contains("YOU WINS"), "{head}");
            if winner == viewer {
                assert_eq!(head, "YOU WIN");
            } else {
                assert!(head.ends_with(" WINS") && !head.starts_with("YOU"), "{head}");
            }
            // The held frame and the podium draw for every seat.
            let age = race.t - race.ended;
            assert!(age < finish::OPENS);
            let _ = frame(&run, viewer);
            let podium = finish::podium(&run, race, Some(viewer), 90);
            assert!(podium.pixels().any(|c| c != BLACK));
        }
        // The banners name a rival the same way: "You" is never a rival's name.
        assert_eq!(moments::called(&run, 1, Some(2), 9), "KNIGHT 1");
        assert_eq!(moments::called(&run, 2, Some(1), 9), "LYNETTE");
        assert_eq!(moments::called(&run, 1, Some(1), 9), "YOU");
    }
}

#[test]
fn every_moment_draws_on_the_realms_inks() {
    for course in [Course::Shaft, Course::Cart, Course::Griffin, Course::Touge] {
        let mut run = racing(course, 3);
        let t = r3::track(course);
        for (k, id) in [1u32, 2, 3].into_iter().enumerate() {
            place(&mut run, id, t.posts[2] + 3.0 + k as f32 * 2.0, 0.0, 0.0, 9.0);
        }
        for _ in 0..40 {
            run.step(&Keys::from([(1, Input { fire: true, move_y: -1, ..Default::default() })]));
        }
        // The picture of the world: the Touge's gearbox plates paint their own colours.
        let img = layers(&run, Some(1), true).0;
        for c in img.pixels() {
            assert!(super::super::super::ink::palette_index(c).is_some(), "{course:?}: {c:?} is off the palette");
        }
    }
}

/// `cargo test race3d_show_bench -- --ignored --nocapture`: what the juice
/// costs a seat's frame, apart from the 3D course it rides on. The cheapest
/// of sixty frames of each, so a busy box does not move it: the whole
/// frame, the frame without its heads-up (the world, the knights and the
/// effects), and the world effects alone.
#[test]
#[ignore]
fn race3d_show_bench() {
    use std::time::Instant;
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
    for course in [Course::Shaft, Course::Cart, Course::Griffin] {
        let mut run = racing(course, 4);
        let t = r3::track(course);
        for (k, id) in [1u32, 2, 3, 4].into_iter().enumerate() {
            place(&mut run, id, t.posts[2] + k as f32 * 3.0, 0.0, 0.0, 7.0);
        }
        for (label, at, wreck) in [("steady", TITLE + 300, false), ("start", TITLE + 8, false), ("wreck", TITLE + 300, true)] {
            let (mut whole, mut world, mut effects) = (f64::MAX, f64::MAX, f64::MAX);
            for k in 0..60u32 {
                race_mut(&mut run).t = at + k % 3;
                if wreck {
                    let race = race_mut(&mut run);
                    race.show.fx.clear();
                    let now = race.t;
                    for (i, w) in [Wreck::Timber, Wreck::Tumble, Wreck::Bonk].into_iter().enumerate() {
                        race.show.fx.push(crate::rules::race3d::show::Fx {
                            kind: Kind::Wreck,
                            seat: 1 + i as u8,
                            other: 0,
                            n: 100,
                            at: [t.posts[2] + 3.0 + i as f32 * 3.0, 0.0, 0.0],
                            weight: crate::rules::juice::Weight::Kill,
                            wreck: w,
                            born: now - 4,
                        });
                    }
                }
                let t0 = Instant::now();
                let _ = layers(&run, Some(1), true);
                whole = whole.min(ms(t0));
                let t1 = Instant::now();
                let _ = layers(&run, Some(1), false);
                world = world.min(ms(t1));
                let race = race_ref(&run);
                let view = super::camera(race, Some(1), NATIVE_W, NATIVE_H);
                let lens = super::fx::Lens { view: &view, w: NATIVE_W, h: NATIVE_H, race, seat: Some(1), track: t };
                let t2 = Instant::now();
                let _ = super::fx::layers(&lens, super::moments::surge(race));
                effects = effects.min(ms(t2));
            }
            eprintln!(
                "{course:?} {label}: {whole:.2} ms a seat's frame; without the heads-up {world:.2} ms; the world effects alone {effects:.2} ms"
            );
        }
    }
}

// ─── the proof ──────────────────────────────────────────────────────────────

/// The frames of one scene on disk, seats side by side.
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
                sheet.stamp(&frame(run, seat), 384 * i as i32, 0);
            }
        }
        image::save_buffer(self.dir.join(format!("f{:04}.png", self.n)), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
        self.n += 1;
    }
}

/// Step `ticks` ticks, a frame every second one (15 fps).
fn play(root: &Path, name: &str, run: &mut Run, seats: &[u32], ticks: u32, mut keys: impl FnMut(&Run, u32) -> Keys) {
    let mut reel = Reel::new(root, name, seats);
    reel.shot(run);
    for t in 0..ticks {
        let k = keys(run, t);
        run.step(&k);
        if t % 2 == 1 {
            reel.shot(run);
        }
    }
    println!("{name}: {} frames", reel.n);
}

/// A knight who keys the obvious things (the audit's own bot).
fn autopilot(run: &Run, id: u32) -> Input {
    use crate::rules::race3d::{drift_bot, laid, rotten, track, Ground, ItemKind, ROT};
    let r = run.race3d.as_ref().unwrap();
    let Some(me) = r.riders.get(&id) else { return Input::default() };
    let t = track(r.course);
    let mut input = Input::default();
    match r.course {
        Course::Touge => return drift_bot(r, id),
        Course::Shaft => {
            input.move_y = -1;
            let at = t.at(me.s);
            let aim = if at.ground == Ground::Lodestone { 0.6 } else { 0.0 };
            let drift = me.x + me.vx * 0.5;
            if drift > aim + 0.25 {
                input.move_x = -1;
            } else if drift < aim - 0.25 {
                input.move_x = 1;
            }
            let plank = ((me.s + 1.2) / 2.0) as u16;
            let gone = t.at(me.s + 1.2).ground == Ground::Collapse && rotten(plank) && r.planks.get(&plank).is_some_and(|&b| r.t + 3 >= b + ROT);
            input.fire = gone && me.z == 0.0;
            if (at.width < 3.0 || t.at(me.s + 6.0).width < 3.0) && me.vs > 5.0 {
                input.move_y = 1;
            }
        }
        Course::Cart => {
            if (1..=8).any(|d| !laid(t, me.s + d as f32, me.lane)) {
                input.interact = r.t % 2 == 0;
                input.move_x = if me.lane == 0 || (me.lane == 1 && laid(t, me.s + 8.0, 2)) { 1 } else { -1 };
            }
            let v = me.vs.max(1.0);
            let gap = t.gaps.iter().any(|g| g.lane == me.lane && (0.3..0.6 + 0.2 * v).contains(&(g.s.0 - me.s)));
            let timber = t.items.iter().any(|i| {
                i.kind == ItemKind::Timber && (i.x - me.x).abs() < 1.0 && (0.6 + 0.2 * v..0.6 + 0.4 * v).contains(&(i.s - me.s))
            });
            input.fire = (gap || timber) && me.z == 0.0 && me.keys & 1 == 0;
            input.move_y = -1;
        }
        Course::Griffin => {
            let mut best = (f32::MAX, 0.0f32, 0.0f32);
            for gx in [-3.0f32, -1.5, 0.0, 1.5, 3.0] {
                for gz in [-2.0f32, 0.0, 2.0] {
                    let blocked = t.blocks.iter().any(|b| b.s > me.s - 1.0 && b.s < me.s + 18.0 && gx + 1.0 > b.x.0 && gx - 1.0 < b.x.1 && gz + 0.9 > b.z.0 && gz - 0.9 < b.z.1)
                        || t.items.iter().enumerate().any(|(i, g)| {
                            g.kind == ItemKind::Gargoyle && !r.taken.contains(&(i as u16)) && g.s > me.s && g.s < me.s + 12.0 && (g.x - gx).hypot(g.z - gz) < 1.6
                        });
                    // Rings pull: the nearest ring ahead is worth a detour.
                    let ring = t.items.iter().enumerate().any(|(i, g)| {
                        g.kind == ItemKind::Ring && me.rings & (1u64 << i) == 0 && g.s > me.s + 2.0 && g.s < me.s + 16.0 && (g.x - gx).hypot(g.z - gz) < 0.5
                    });
                    let cost = if blocked { 1000.0 } else { 0.0 } + (gx - me.x).abs() + (gz - me.z).abs() - if ring { 3.0 } else { 0.0 };
                    if cost < best.0 {
                        best = (cost, gx, gz);
                    }
                }
            }
            if best.1 > me.x + 0.3 {
                input.move_x = 1;
            } else if best.1 < me.x - 0.3 {
                input.move_x = -1;
            }
            if best.2 > me.z + 0.3 {
                input.move_y = -1;
            } else if best.2 < me.z - 0.3 {
                input.move_y = 1;
            }
            input.fire = r.t % 12 == 0;
        }
    }
    input
}

/// Every seat on the bot, the seats' own habits showing: one brakes to a
/// crawl now and then, one lags, so there is a field to pass.
fn field(run: &Run, t: u32) -> Keys {
    run.players
        .keys()
        .map(|&id| {
            let mut i = autopilot(run, id);
            if run.race3d.as_ref().is_some_and(|r| r.course != Course::Griffin) {
                // The ball and the cart: seat 2 dawdles, seat 3 comes on late.
                let slow = (id == 2 && t % 120 < 14) || (id == 3 && t < 40);
                if slow {
                    i.move_y = 1;
                    i.fire = false;
                }
            }
            (id, i)
        })
        .collect()
}

/// `ANGEL_RACE3D_PROOF=<dir> [PROOF_ONLY=ball-start,...] cargo test -p
/// angel-delve --lib write_race3d_proof -- --ignored --nocapture`: the
/// scenes of the after-GIFs, seats one and two side by side.
#[test]
#[ignore]
fn write_race3d_proof() {
    let Some(root) = std::env::var_os("ANGEL_RACE3D_PROOF") else { return };
    let root = PathBuf::from(root);
    let only: Option<Vec<String>> = std::env::var("PROOF_ONLY").ok().map(|v| v.split(',').map(String::from).collect());
    let want = |name: &str| only.as_ref().is_none_or(|o| o.iter().any(|x| x == name));
    let seats = [1u32, 2];
    // The practice flight: the same gorge, nothing staked.
    if want("eyrie-start") {
        let mut run = racing(Course::Griffin, 4);
        classed(&mut run);
        race_mut(&mut run).t = 0;
        race_mut(&mut run).practice = true;
        play(&root, "eyrie-start", &mut run, &seats, TITLE + 50, |r, t| field(r, t));
    }
    // The Torchlight Touge: the drift's own sparks and boosts.
    if want("touge-ride") {
        let mut run = racing(Course::Touge, 4);
        classed(&mut run);
        race_mut(&mut run).t = TITLE - 40;
        play(&root, "touge-ride", &mut run, &seats, 360, |r, _| r.players.keys().map(|&id| (id, autopilot(r, id))).collect());
    }
    for (course, tag) in [(Course::Shaft, "ball"), (Course::Cart, "cart"), (Course::Griffin, "griffin")] {
        let t = r3::track(course);
        // The start: the lamps, the green, the launch.
        if want(&format!("{tag}-start")) {
            let mut run = racing(course, 4);
            classed(&mut run);
            race_mut(&mut run).t = 0;
            play(&root, &format!("{tag}-start"), &mut run, &seats, TITLE + 60, |r, t| field(r, t));
        }
        // The ride: a field strung out, passes, pickups, wind.
        if want(&format!("{tag}-ride")) {
            let mut run = racing(course, 4);
            classed(&mut run);
            race_mut(&mut run).t = TITLE + 40;
            let from = match course {
                Course::Shaft => t.posts[1] + 4.0,
                _ => t.posts[1] + 6.0,
            };
            for (k, id) in [1u32, 2, 3, 4].into_iter().enumerate() {
                let (x, z) = match course {
                    Course::Griffin => ([0.0, -2.0, 2.2, 0.8][k], [0.0, 0.8, -0.6, 1.6][k]),
                    Course::Cart => ([0.0, -1.7, 1.7, 0.0][k], 0.0),
                    _ => ([0.0, -0.8, 0.8, 0.0][k], 0.0),
                };
                place(&mut run, id, from + [0.0, 2.5, 5.0, 8.0][k], x, z, 8.0);
                if course == Course::Cart {
                    race_mut(&mut run).riders.get_mut(&id).unwrap().lane = [1, 0, 2, 1][k];
                }
            }
            play(&root, &format!("{tag}-ride"), &mut run, &seats, 330, |r, t| field(r, t));
        }
        // A big hit: seat one into the thing it should have missed.
        if want(&format!("{tag}-wreck")) {
            let mut run = racing(course, 4);
            classed(&mut run);
            race_mut(&mut run).t = TITLE + 40;
            match course {
                Course::Shaft => {
                    // Into the ore carts at the crossing.
                    let start = t.posts[5];
                    race_mut(&mut run).t = 260;
                    place(&mut run, 1, start + 2.5, 0.0, 0.0, 9.0);
                    place(&mut run, 2, start - 5.0, 0.8, 0.0, 9.0);
                    place(&mut run, 3, start - 9.0, -0.8, 0.0, 9.0);
                    place(&mut run, 4, start + 7.0, 0.0, 0.0, 9.0);
                }
                Course::Cart => {
                    // Into the fallen timber on the right line.
                    let timber = t.items.iter().find(|i| i.kind == r3::ItemKind::Timber).unwrap();
                    let lane = if timber.x > 1.0 { 2 } else if timber.x < -1.0 { 0 } else { 1 };
                    place(&mut run, 1, timber.s - 7.0, timber.x, 0.0, 10.0);
                    place(&mut run, 2, timber.s - 12.0, r3::LANES[1], 0.0, 9.0);
                    place(&mut run, 3, timber.s - 4.0, r3::LANES[0], 0.0, 9.0);
                    place(&mut run, 4, timber.s - 18.0, r3::LANES[2], 0.0, 9.0);
                    for (id, l) in [(1u32, lane), (2, 1), (3, 0), (4, 2)] {
                        race_mut(&mut run).riders.get_mut(&id).unwrap().lane = l;
                    }
                }
                _ => {
                    // Into the first spire.
                    let at = t.posts[1] + 22.0;
                    place(&mut run, 1, at - 12.0, -2.5, 0.0, 10.0);
                    place(&mut run, 2, at - 16.0, 2.5, 0.5, 10.0);
                    place(&mut run, 3, at - 22.0, 0.0, 1.5, 10.0);
                    place(&mut run, 4, at - 9.0, 3.0, -1.0, 10.0);
                }
            }
            play(&root, &format!("{tag}-wreck"), &mut run, &seats, 150, |r, t| {
                let mut keys = field(r, t);
                // Seat one holds on straight and gets what it asked for.
                keys.insert(1, if course == Course::Griffin { Input::default() } else { Input { move_y: -1, ..Default::default() } });
                keys
            });
        }
        // The finish: a photo finish, the rest home, the freeze and the podium.
        if want(&format!("{tag}-finish")) {
            let mut run = racing(course, 4);
            classed(&mut run);
            let end = t.finish;
            let gaps = [11.0f32, 12.6, 21.0, 30.0];
            for (k, id) in [1u32, 2, 3, 4].into_iter().enumerate() {
                let (x, z) = match course {
                    Course::Griffin => ([0.0, -1.6, 1.6, 0.6][k], 0.4),
                    Course::Cart => (0.0, 0.0),
                    _ => ([0.0, 0.0, 0.0, 0.0][k], 0.0),
                };
                place(&mut run, id, end - gaps[k], x, z, 9.0);
                if course == Course::Cart {
                    race_mut(&mut run).riders.get_mut(&id).unwrap().lane = 1;
                }
            }
            // Nobody dawdles in the last stretch: a photo finish is two knights at one pace.
            play(&root, &format!("{tag}-finish"), &mut run, &seats, 90 + 30 + r3::PODIUM, |r, _| r.players.keys().map(|&id| (id, autopilot(r, id))).collect());
        }
    }
}

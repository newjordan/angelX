//! The Hippodrome race's show: the words on every seat's screen over whole races at one to six
//! seats (nothing meets, everything is in frame, `YOU` is only the viewer), the moments the race
//! calls, a friend's swapped view agreeing with the host's, and the reels the proof is cut from.
//!
//!   ANGEL_CHARIOT_REEL=<dir> [REEL_ONLY=start,ride] \
//!     cargo test -p angel-delve --lib write_chariot_reel -- --ignored --nocapture
use super::moments::{inside, meets};
use crate::render::overworld::Img;
use crate::render::overworld::arena::frame_for;
use crate::render::overworld::ink::said;
use crate::rules::chariot::{GATE_TICKS, Phase, show::Kind};
use crate::rules::{Input, RoomKind, Run, hippodrome, world};
use std::collections::BTreeMap;
use std::path::PathBuf;

type Keys = BTreeMap<u32, Input>;

/// The names a table of up to six carries: a clean name, the default `You`, a short one, an empty
/// one, a long one and one with a number in it.
const NAMES: [&str; 6] = ["Gawain", "You", "Dara", "", "Alexandrina", "Bran 2"];

/// A table of `seats` knights in the Hippodrome, the race boarded and standing at the gate.
pub(super) fn table(seats: u32) -> Run {
    let mut r = Run::at_home(7, 1, Some("Gawain"), Default::default(), Default::default());
    for id in 2..=seats {
        r.join_seat(id, &format!("Seat {id}"));
    }
    // The run seats a guest beside the host: a table of one is the host alone.
    if seats == 1 {
        r.leave(2);
    }
    for id in 1..=seats {
        r.players.get_mut(&id).unwrap().name = NAMES[id as usize - 1].to_string();
    }
    let hippo = world::room_of(&r.dungeon, RoomKind::Hippodrome).expect("the Hippodrome");
    let (c, row, _, _) = hippodrome::COACH;
    r.arrive(hippo, (c as f32 + 0.5, row as f32 + 0.5));
    r.step(&Keys::new());
    r.board_chariot(1);
    assert!(r.chariot.is_some(), "the race did not open");
    r
}

/// A knight at the reins: odd seats ride at whoever is alongside and whip hard, even seats keep
/// clear and let the wind come back, now and then a seat drops iron on whoever rides its tail.
pub(super) fn hands(r: &Run) -> Keys {
    let Some(c) = r.chariot.as_ref() else { return Keys::new() };
    c.seats()
        .into_iter()
        .map(|(seat, idx)| {
            let me = &c.racers[idx];
            let near = c
                .racers
                .iter()
                .enumerate()
                .filter(|(j, o)| *j != idx && o.place.is_none() && (o.dist - me.dist).abs() < 30.0)
                .min_by(|a, b| (a.1.lane - me.lane).abs().total_cmp(&(b.1.lane - me.lane).abs()));
            let ram = seat % 2 == 1;
            let steer = match near {
                Some((_, o)) if ram => if o.lane > me.lane { 1 } else { -1 },
                Some((_, o)) => if o.lane > me.lane { -1 } else { 1 },
                None => if me.lane > 0.3 { -1 } else if me.lane < -0.3 { 1 } else { 0 },
            };
            let whip = if ram { me.wind > 0.15 } else { me.wind > 0.55 && (c.clock / 40 + seat) % 3 != 0 };
            let tail = c.racers.iter().enumerate().any(|(j, o)| j != idx && me.dist - o.dist > 4.0 && me.dist - o.dist < 16.0 && (me.lane - o.lane).abs() < 0.5);
            let iron = tail && c.clock % 7 == seat % 7;
            (seat, Input { move_x: steer, dash: whip, bomb: iron, ..Default::default() })
        })
        .collect()
}

fn frame(r: &Run, seat: u32) -> Img {
    frame_for(r, 384, 224, Some(seat))
}

/// The race played through to the end of its result: calls `each` before every tick.
fn through(r: &mut Run, mut each: impl FnMut(&Run)) {
    for _ in 0..60_000 {
        if r.chariot.is_none() {
            return;
        }
        each(r);
        let k = hands(r);
        r.step(&k);
    }
    panic!("the race never ended");
}

fn reel_dir() -> Option<PathBuf> {
    std::env::var_os("ANGEL_CHARIOT_REEL").map(PathBuf::from)
}

fn want(name: &str) -> bool {
    std::env::var("REEL_ONLY").map_or(true, |o| o.split(',').any(|x| x == name))
}

fn save(dir: &std::path::Path, n: u32, seats: &[u32], r: &Run) {
    let mut sheet = Img::black(384 * seats.len() as i32, 224);
    for (i, &seat) in seats.iter().enumerate() {
        if r.players.contains_key(&seat) {
            sheet.stamp(&frame(r, seat), 384 * i as i32, 0);
        }
    }
    image::save_buffer(dir.join(format!("f{n:04}.png")), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
}

/// The ticks the race's moments land on, found by running it: the first barge, wreck, lead
/// change, photograph and line, and when the race was done.
fn moments_of(seats: u32) -> BTreeMap<&'static str, u32> {
    let mut found = BTreeMap::new();
    let mut r = table(seats);
    let mut n = 0u32;
    through(&mut r, |r| {
        n += 1;
        let c = r.chariot.as_ref().unwrap();
        for f in &c.show.fx {
            let name = match f.kind {
                Kind::Barge if f.weight >= crate::rules::juice::Weight::Crushing => "barge",
                Kind::Wreck => "wreck",
                Kind::Lead => "lead",
                Kind::Photo => "photo",
                Kind::Finish if f.n == 1 => "line",
                Kind::Fastest => "fastest",
                _ => continue,
            };
            found.entry(name).or_insert((n - 1).saturating_sub(c.clock.saturating_sub(f.born)));
        }
        if c.phase == Phase::Done {
            found.entry("done").or_insert((n - 1).saturating_sub(c.t));
        }
    });
    found
}

/// Two teams side by side in a corner at pace, the second with less nerve: a barge.
fn setup_barge(r: &mut Run) {
    let c = r.chariot.as_mut().unwrap();
    let course = c.course();
    let probe = c.racers[0].clone();
    let bend = (0..400)
        .map(|k| k as f32 * course.lap / 400.0)
        .find(|&d| {
            let mut p = probe.clone();
            p.dist = d;
            p.cornering(course)
        })
        .unwrap();
    c.racers[0].blood.nerve = 9;
    c.racers[1].blood.nerve = 2;
    for (i, lane) in [(0, -0.1), (1, 0.1)] {
        c.racers[i].dist = bend;
        c.racers[i].lane = lane;
        c.racers[i].speed = 8.0;
    }
    for k in 2..c.racers.len() {
        c.racers[k].dist = bend + 120.0 + 30.0 * k as f32;
    }
}

/// The first two teams a pace apart and a few strides from the line: a photograph.
fn setup_photo(r: &mut Run) {
    let c = r.chariot.as_mut().unwrap();
    let total = c.course().lap * crate::rules::chariot::LAPS as f32;
    for (i, d, lane) in [(0, 70.0, -0.5), (1, 110.0, 0.4)] {
        c.racers[i].dist = total - d;
        c.racers[i].lane = lane;
        c.racers[i].speed = 9.0;
        c.racers[i].wind = 1.0;
    }
    for k in 2..c.racers.len() {
        c.racers[k].dist = total - 500.0 - 60.0 * k as f32;
    }
}

/// Play a table of `seats` and write the frames of the tick window `[from, to)` (30 fps); a
/// scenario, if given, sets the field up at the tick `from`.
fn reel(root: &std::path::Path, name: &str, seats: u32, show: &[u32], from: u32, to: u32) {
    reel_with(root, name, seats, show, from, to, None);
}

fn reel_with(root: &std::path::Path, name: &str, seats: u32, show: &[u32], from: u32, to: u32, scenario: Option<fn(&mut Run)>) {
    let dir = root.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = table(seats);
    let mut n = 0;
    for t in 0..to {
        if r.chariot.is_none() {
            break;
        }
        if t == from {
            if let Some(setup) = scenario {
                setup(&mut r);
            }
        }
        if t >= from {
            save(&dir, n, show, &r);
            n += 1;
        }
        let k = hands(&r);
        r.step(&k);
    }
    println!("{name}: {n} frames");
}

#[test]
#[ignore = "writes the reels the proof is cut from"]
fn write_chariot_reel() {
    let Some(root) = reel_dir() else { return };
    let m = moments_of(4);
    println!("moments at 4 seats: {m:?}");
    let at = |k: &str, back: u32| m.get(k).copied().unwrap_or(300).saturating_sub(back);
    for (name, seats, show, from, to) in [
        ("start", 4, vec![1, 2], 0, GATE_TICKS + 90),
        ("ride", 4, vec![1, 2], GATE_TICKS + 150, GATE_TICKS + 300),
        ("wreck", 4, vec![1, 2], at("wreck", 20), at("wreck", 0) + 80),
        ("lead", 4, vec![1, 2], at("lead", 20), at("lead", 0) + 90),
        ("finish", 4, vec![1, 2], at("line", 40), at("line", 0) + 150),
        ("podium", 4, vec![1, 2], at("done", 0) + 40, at("done", 0) + 330),
        ("six", 6, vec![1, 6], GATE_TICKS + 200, GATE_TICKS + 360),
        ("solo", 1, vec![1], GATE_TICKS + 120, GATE_TICKS + 270),
    ] {
        if want(name) {
            reel(&root, name, seats, &show, from, to);
        }
    }
    if want("barge") {
        reel_with(&root, "barge", 4, &[1, 2], GATE_TICKS + 30, GATE_TICKS + 140, Some(setup_barge));
    }
    if want("photo") {
        reel_with(&root, "photo", 4, &[1, 2], GATE_TICKS + 30, GATE_TICKS + 230, Some(setup_photo));
    }
}

// ─── the words ───────────────────────────────────────────────────────────────

use super::view::View;

/// How the test spells a team from the viewer's seat.
fn spelled(r: &Run, v: &View, team: u8, me_team: Option<u8>) -> Option<String> {
    if Some(team) == me_team {
        return Some("YOU".into());
    }
    let Some(seat) = v.seat_of(team) else {
        // A yard's own team is called by its house (or the horse's name at the head of the field).
        let t = v.race.racers.iter().find(|t| t.team == team)?;
        return Some(if team == 0 && t.name != "Yard Nag" { t.name.to_uppercase() } else { t.house.to_uppercase() });
    };
    let n = r.players[&seat].name.trim().to_uppercase();
    Some(if n.is_empty() || n == "YOU" { format!("KNIGHT {seat}") } else { n })
}

/// The rules of the words, for every claim on `me`'s screen: nothing meets, everything is in
/// frame, and `YOU` is only the viewer.
fn check(r: &Run, me: u32, what: &str) -> Vec<(String, (i32, i32, i32, i32), Option<u8>)> {
    let v = View::new(r, Some(me), (384, 224)).unwrap();
    let me_team = v.mine.then(|| v.my_team());
    let claims = super::claims_for(r, Some(me)).unwrap();
    for (i, (an, a, _)) in claims.iter().enumerate() {
        assert!(inside(*a), "{what}: seat {me}: {an} leaves the frame: {a:?}");
        for (bn, b, _) in &claims[i + 1..] {
            assert!(!meets(*a, *b), "{what}: seat {me}: {an} {a:?} meets {bn} {b:?}");
        }
    }
    let plan = super::moments::plan_of(&v);
    for w in &plan.words {
        if let (true, Some(team)) = (w.kind == "tag" || w.kind == "ledger", w.who) {
            let want = spelled(r, &v, team, me_team);
            if let Some(want) = want {
                let take = if w.kind == "ledger" { 8 } else { w.text.chars().count() };
                let head: String = want.chars().take(take).collect();
                assert!(w.text.contains(&head), "{what}: seat {me}: team {team} is {want:?}, not {:?}", w.text);
            }
        }
        let says_you = w.text.split_whitespace().any(|t| t == "YOU");
        if says_you {
            assert_eq!(w.who, me_team, "{what}: seat {me}: YOU on another knight: {:?}", w.text);
        }
    }
    for (_, text, _, who) in &plan.podium {
        if text.split_whitespace().any(|t| t == "YOU") {
            assert_eq!(*who, me_team, "{what}: seat {me}: the podium says YOU for another knight: {text:?}");
        }
        if let (Some(team), Some(want)) = (who, me_team) {
            if *team == want {
                assert_eq!(text, "YOU", "{what}: seat {me}: their own place is YOU, not {text:?}");
            }
        }
    }
    if let Some(b) = &plan.banner {
        let says_you = b.title.split_whitespace().any(|t| t == "YOU") || b.sub.split_whitespace().any(|t| t == "YOU");
        if says_you {
            assert_eq!(b.who, me_team, "{what}: seat {me}: a banner says YOU for another knight: {:?} {:?}", b.title, b.sub);
        }
    }
    claims
}

/// Every line the pen wrote on the whole 384 by 224 screen sits inside one of the claimed boxes.
fn check_pen(r: &Run, me: u32, what: &str) {
    let (_, said) = said::read(|| frame(r, me));
    let claims = super::claims_for(r, Some(me)).unwrap();
    for line in said.lines.iter().filter(|l| l.on == (384, 224)) {
        let rect = (line.at.0, line.at.1, crate::render::overworld::ink::text_width(&line.text), 7);
        assert!(inside(rect), "{what}: seat {me}: {:?} leaves the screen", line.text);
        assert!(
            claims.iter().any(|(_, c, _)| rect.0 >= c.0 && rect.1 >= c.1 && rect.0 + rect.2 <= c.0 + c.2 && rect.1 + rect.3 <= c.1 + c.3),
            "{what}: seat {me}: {:?} at {rect:?} is not in a claimed box",
            line.text
        );
    }
}

#[test]
fn the_words_never_meet_stay_in_frame_and_you_is_only_you() {
    for seats in 1..=6u32 {
        let mut r = table(seats);
        let mut kinds = std::collections::BTreeSet::new();
        let mut at = 0u32;
        through(&mut r, |r| {
            at += 1;
            for me in 1..=seats {
                let claims = check(r, me, &format!("{seats} seats, tick {}", r.tick));
                for (name, ..) in &claims {
                    kinds.insert(name.split(' ').next().unwrap_or("").to_string());
                }
                let v = View::new(r, Some(me), (384, 224)).unwrap();
                if let Some(b) = super::moments::plan_of(&v).banner {
                    kinds.insert(format!("banner {}", b.title.split(' ').next().unwrap_or("")));
                }
            }
            if at % 17 == 0 {
                check_pen(r, 1 + at / 17 % seats, &format!("{seats} seats, tick {}", r.tick));
            }
            // A friend's own screen is the swapped view: it must claim what the host's frame of
            // that seat claims.
            if at % 53 == 0 && seats > 1 {
                let me = 2;
                let view = r.view_for_seat(me).unwrap_or_else(|| r.clone());
                assert_eq!(
                    super::claims_for(&view, Some(me)).unwrap(),
                    super::claims_for(r, Some(me)).unwrap(),
                    "{seats} seats, tick {}: the friend's swapped view claims what the host's frame does",
                    r.tick
                );
            }
        });
        assert!(kinds.len() >= 9, "{seats} seats: the run showed too few moments: {kinds:?}");
    }
}

#[test]
fn every_seat_races_its_own_team_and_six_seats_make_six_teams() {
    let r = table(6);
    let c = r.chariot.as_ref().unwrap();
    assert_eq!(c.racers.len(), 6);
    assert_eq!(c.seats().len(), 6, "every seat drives");
    let mut teams: Vec<u8> = c.racers.iter().map(|t| t.team).collect();
    teams.sort_unstable();
    assert_eq!(teams, [0, 1, 2, 3, 4, 5]);
    // Each seat's picture rides its own team: the frames differ, and seat 6's is not the host's.
    let host = frame(&r, 1);
    let sixth = frame(&r, 6);
    assert_ne!(host, sixth, "two seats, two cameras");
}

#[test]
fn the_title_the_count_and_the_go_show_on_every_seat() {
    let mut r = table(4);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..GATE_TICKS + 40 {
        for me in 1..=4 {
            let v = View::new(&r, Some(me), (384, 224)).unwrap();
            let p = super::moments::plan_of(&v);
            if let Some(c) = &p.count {
                seen.insert(c.label.to_string());
            }
            if p.banner.as_ref().is_some_and(|b| b.sub == "THREE LAPS") {
                seen.insert("title".to_string());
            }
        }
        r.step(&Keys::new());
    }
    assert_eq!(seen, ["1", "2", "3", "GO", "title"].into_iter().map(String::from).collect());
}

#[test]
fn a_barge_and_a_photograph_show_on_every_seat() {
    let mut r = table(4);
    for _ in 0..GATE_TICKS + 40 {
        let k = hands(&r);
        r.step(&k);
    }
    setup_barge(&mut r);
    let mut called = std::collections::BTreeSet::new();
    for _ in 0..60 {
        let k = hands(&r);
        r.step(&k);
        for me in 1..=4 {
            check(&r, me, "the barge");
            let v = View::new(&r, Some(me), (384, 224)).unwrap();
            if let Some(b) = super::moments::plan_of(&v).banner {
                called.insert(b.title);
            }
        }
    }
    assert!(called.contains("BARGE"), "the winner is told: {called:?}");
    assert!(called.contains("SHOVED") || called.contains("WRECKED"), "the loser is told: {called:?}");
    let mut r = table(4);
    for _ in 0..GATE_TICKS + 40 {
        let k = hands(&r);
        r.step(&k);
    }
    setup_photo(&mut r);
    let mut called = std::collections::BTreeSet::new();
    for _ in 0..120 {
        let k = hands(&r);
        r.step(&k);
        for me in 1..=4 {
            check(&r, me, "the photograph");
            let v = View::new(&r, Some(me), (384, 224)).unwrap();
            if let Some(b) = super::moments::plan_of(&v).banner {
                called.insert(b.title);
            }
        }
    }
    assert!(called.contains("PHOTO FINISH"), "the photograph is called on every seat: {called:?}");
}

/// The cost of a frame over a four-seat race, as the proof measures it.
#[test]
#[ignore = "measures the picture's cost"]
fn bench_chariot_frame() {
    use std::time::Instant;
    let mut r = table(4);
    for _ in 0..GATE_TICKS + 120 {
        let k = hands(&r);
        r.step(&k);
    }
    let mut times = Vec::new();
    for _ in 0..200 {
        let k = hands(&r);
        r.step(&k);
        let t = Instant::now();
        for seat in 1..=4 {
            std::hint::black_box(frame(&r, seat));
        }
        times.push(t.elapsed().as_secs_f64() * 1000.0 / 4.0);
    }
    times.sort_by(|a, b| a.total_cmp(b));
    println!("chariot frame: median {:.2} ms, p90 {:.2} ms per seat", times[times.len() / 2], times[times.len() * 9 / 10]);
}

#[test]
fn the_chariot_lines_up_on_the_stands() {
    // The bake of every course finishes and a frame draws from each.
    let mut r = table(2);
    let course_count = crate::rules::chariot::TRACKS.len();
    for track in 0..course_count {
        r.chariot.as_mut().unwrap().track = track;
        for _ in 0..GATE_TICKS + 20 {
            r.step(&Keys::new());
        }
        let f = frame(&r, 1);
        assert_eq!((f.w, f.h), (384, 224));
    }
    let _ = Phase::Gate;
}

#[test]
fn every_yard_runs_in_its_own_cloth_and_wears_no_other() {
    use super::art::{self, View as Seen};
    use crate::render::overworld::ink::ink;
    let houses = ["qwen", "grok", "glm", "deepseek", "keep", "tower", "marsh"];
    for view in [Seen::Side, Seen::Head, Seen::Rear] {
        for house in houses {
            let l = art::livery(house);
            let im = art::team(l, 1, 0, view, false);
            let count = |c: char| (0..im.h).flat_map(|y| (0..im.w).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y) == ink(c)).count();
            assert!(count(l.cloth) > 12, "{house} {view:?} wears its cloth: {}", count(l.cloth));
            for other in houses.iter().filter(|o| **o != house) {
                let o = art::livery(other);
                if o.cloth != l.cloth && o.cloth != l.trim && o.cloth != 'h' {
                    assert_eq!(count(o.cloth), 0, "{house} {view:?} wears {other}'s cloth");
                }
            }
        }
    }
}

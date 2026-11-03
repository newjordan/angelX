//! The steps up drawn: each moment paints over its knight while it plays
//! and nothing once it has; every constellation has its stars on its chart;
//! the road ahead lays out a party of one to six inside the board; and the
//! same run draws the same pixels.
use super::*;
use crate::rules::rewards::{Earned, Kind, Step};
use std::collections::BTreeMap;

fn room() -> Run {
    let mut run = crate::rules::feel::testing::quiet();
    crate::rules::feel::testing::bare(&mut run);
    for _ in 0..200 {
        run.step(&BTreeMap::new());
    }
    run.floor_plates.clear();
    run
}

fn with(run: &Run, knight: u32, kind: Kind, age: u64) -> Run {
    let mut run = run.clone();
    run.rewards.steps = vec![Step { at: run.tick.saturating_sub(age), knight, kind }];
    run
}

fn differs(a: &Img, b: &Img) -> usize {
    (0..a.h).flat_map(|y| (0..a.w).map(move |x| (x, y))).filter(|&(x, y)| a.get(x, y) != b.get(x, y)).count()
}

#[test]
fn each_moment_paints_while_it_plays_and_nothing_after() {
    let run = room();
    let plain = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    for (kind, mid) in [
        (Kind::Level { level: 3 }, 30),
        (Kind::Tier { wish: "Seeker Fletching".into(), from: 1, tier: 3 }, 30),
        (Kind::Rung { px: 32 }, 40),
        (Kind::Relic { name: "Awakened Fork".into() }, 20),
        (Kind::Record { floor: 4 }, 40),
        (Kind::Lit { index: 0, by: "Ada".into() }, 90),
        (Kind::Gift { wishes: 1 }, 40),
    ] {
        let knight = if kind.banner() { 0 } else { 1 };
        let life = kind.life();
        let playing = frame_for(&with(&run, knight, kind.clone(), mid), NATIVE_W, NATIVE_H, Some(1));
        let shown = differs(&plain, &playing);
        let mut over = Img::new(800, 600);
        super::over(&mut over, &with(&run, knight, kind.clone(), mid));
        let painted = (0..over.h).flat_map(|y| (0..over.w).map(move |x| (x, y))).filter(|&(x, y)| over.get(x, y).is_some()).count();
        assert!(shown > 40, "{kind:?} shows: {shown} pixels; plate {:?}; over alone {painted}", run.floor_plate(Some(1)).is_some());
        let after = with(&run, knight, kind.clone(), life);
        assert!(after.rewards.playing(after.tick).next().is_none());
        assert_eq!(differs(&plain, &frame_for(&after, NATIVE_W, NATIVE_H, Some(1))), 0, "{kind:?} gone");
        // The same run, the same pixels.
        let again = frame_for(&with(&run, knight, kind.clone(), mid), NATIVE_W, NATIVE_H, Some(1));
        assert_eq!(differs(&playing, &again), 0, "{kind:?} is a pure picture");
    }
}

#[test]
fn every_constellation_has_its_stars_on_its_chart() {
    for index in 0..20 {
        let stars = rewards::stars(index);
        assert_eq!(stars.len() as u32, crate::rules::rewards::size(index), "constellation {index}");
        for (x, y) in stars {
            assert!((0.0..=48.0).contains(&x) && (0.0..=28.0).contains(&y), "{index}: ({x}, {y})");
        }
    }
}

#[test]
fn the_road_ahead_lays_out_one_to_six_knights() {
    for n in 1..=6u32 {
        let mut run = room();
        for id in 2..=n {
            run.join_seat(id, &format!("Knight{id}"));
        }
        for id in 1..=n {
            run.ledger.earned.insert(id, Earned { xp: 1234, from_level: 1, level: 2, wishes: 7, best: Some(("Seeker Fletching".into(), 3)), relics: 1, gold: 40, rung: 64 });
        }
        run.ledger.record = true;
        run.ledger.deepest = 4;
        run.home.sky.stars = 12;
        let early = rewards::road_page(&run, 2);
        let full = rewards::road_page(&run, 400);
        assert!(differs(&early, &full) > 500, "{n}: it builds");
        // Nothing leaves the board's own margins.
        for y in 0..NATIVE_H {
            for x in [0, 1, NATIVE_W - 2, NATIVE_W - 1] {
                assert_eq!(full.get(x, y), Some([0, 0, 0]), "{n} knights: ({x}, {y})");
            }
        }
        assert_eq!(differs(&full, &rewards::road_page(&run, 400)), 0);
    }
}

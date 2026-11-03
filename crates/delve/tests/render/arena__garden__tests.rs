//! The garden's promises, drawn: its guests are bugs in the garden and
//! nowhere else, a frame of the party is the same drawn twice, and the
//! trip turns only the room's view, keeps its black, and is gentle on a
//! calm screen.

use super::super::{NATIVE_H, NATIVE_W, frame_for, sprites};
use super::*;
use crate::render::viewer::{self, Preferences};
use crate::rules::nooks::{self, Nook};
use std::collections::BTreeMap;

/// A garden with the party on and one of each guest on the lawn.
fn party() -> Run {
    let mut run = Run::new(41, 1, None);
    run.enemies.clear();
    run.dungeon.rooms.push(nooks::room(RoomKind::GnomeParty, (12, 12), 3));
    let index = run.dungeon.rooms.len() - 1;
    run.nook = Some(Nook { room: index, ..Default::default() });
    run.enter_for_test(index);
    run.enemies.clear();
    let garden = g::Garden { party: Party::Holding, house: g::HOUSE_HP, ..Default::default() };
    run.nook.as_mut().unwrap().garden = Some(garden);
    for (k, kind) in [EnemyKind::Leech, EnemyKind::Biter, EnemyKind::Spiderling, EnemyKind::Silkmother].into_iter().enumerate() {
        run.spawn_at_for_test(kind, 10.0 + k as f32 * 6.0, 20.0);
    }
    run
}

#[test]
fn the_guests_are_bugs_in_the_garden_and_nowhere_else() {
    let run = party();
    for e in &run.enemies {
        let bug = guest(&run, e, 12).expect("a garden guest");
        assert_ne!(bug, sprites::enemy(e.kind, 12), "{:?} wears the garden's look", e.kind);
    }
    let mut fight = Run::new(11, 1, None);
    let i = fight.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    fight.enter_for_test(i);
    fight.enemies.clear();
    fight.spawn_at_for_test(EnemyKind::Leech, 10.0, 10.0);
    assert!(guest(&fight, &fight.enemies[0], 12).is_none(), "a leech in a fight room is a leech");
}

#[test]
fn a_frame_of_the_party_is_the_same_drawn_twice() {
    let mut run = party();
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    let a = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let b = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    assert_eq!(a, b);
}

/// A picture of coloured stripes with a black square in its middle.
fn card(w: i32, h: i32) -> Img {
    let mut im = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            im.put(x, y, ['G', 'y', 'R', 'z'][((x / 7 + y / 5) % 4) as usize]);
        }
    }
    for y in h / 2 - 6..h / 2 + 6 {
        for x in w / 2 - 6..w / 2 + 6 {
            im.set(x, y, [0, 0, 0]);
        }
    }
    im
}

fn change(a: &Img, b: &Img) -> u64 {
    let mut d = 0;
    for y in 0..a.h {
        for x in 0..a.w {
            if let (Some(p), Some(q)) = (a.get(x, y), b.get(x, y)) {
                d += (0..3).map(|i| u64::from(p[i].abs_diff(q[i]))).sum::<u64>();
            }
        }
    }
    d
}

#[test]
fn the_trip_turns_the_view_keeps_its_black_and_is_gentle_when_calm() {
    let (w, h) = (NATIVE_W * 2, NATIVE_H * 2);
    let plain = card(w, h + 40);
    let mut wild = plain.clone();
    viewer::trip_rows(&mut wild, 300, 20, 20 + h);
    for x in 0..w {
        for y in (0..20).chain(20 + h..h + 40) {
            assert_eq!(wild.get(x, y), plain.get(x, y), "the bands are left to be read");
        }
    }
    assert_eq!(wild.get(w / 2, (h + 40) / 2), Some([0, 0, 0]), "black stays black");
    let mut calm = plain.clone();
    viewer::with(Preferences { calm_screen: true }, || viewer::trip_rows(&mut calm, 300, 20, 20 + h));
    let (strong, soft) = (change(&plain, &wild), change(&plain, &calm));
    assert!(soft * 4 < strong, "a calm screen's trip is much the gentler: {soft} against {strong}");
    let mut again = plain.clone();
    viewer::trip_rows(&mut again, 300, 20, 20 + h);
    assert_eq!(again, wild, "the same tick turns the same pixels");
}

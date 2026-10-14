use super::*;
use std::collections::BTreeMap;

fn input(i: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, i)])
}

/// A calm, cleared fight room, one knight at the middle, no monsters.
fn quiet() -> Run {
    let mut run = Run::new(4, 1, Some("Matt"));
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.window = None;
    run.frogs.wild.clear();
    for hero in run.players.values_mut() {
        hero.hp = 100;
        hero.max_hp = 100;
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    let hero = run.players.get_mut(&2).unwrap();
    (hero.x, hero.y) = (40.0, 24.0);
    run
}

fn frog_at(run: &mut Run, x: f32, y: f32) {
    run.frogs.wild.push(Wild {
        x,
        y,
        from: (x, y),
        to: (x, y),
        hop: 0,
        rest: 50,
        leap: None,
        face: 1,
    });
}

/// Into a Sanctuary with the party as it is.
fn to_the_sanctuary(run: &mut Run) {
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Sanctuary)
        .unwrap_or_else(|| {
            let at = run.at;
            run.dungeon.rooms[at].kind = RoomKind::Sanctuary;
            at
        });
    run.enter_for_test(room);
}

#[test]
fn about_one_room_in_five_has_a_frog() {
    let mut with = 0;
    let mut rooms = 0;
    for seed in 0..40 {
        let mut run = Run::new(seed, 1, None);
        run.descend_for_test();
        for index in 0..run.dungeon.rooms.len() {
            if !matches!(
                run.dungeon.rooms[index].kind,
                RoomKind::Fight | RoomKind::Hall | RoomKind::Treasure | RoomKind::Start | RoomKind::Stairs
            ) {
                continue;
            }
            run.frogs.wild.clear();
            run.maybe_frog(index, true);
            rooms += 1;
            with += run.frogs.wild.len().min(1);
        }
    }
    let share = with as f32 / rooms as f32;
    assert!((0.12..0.30).contains(&share), "{share} of {rooms}");
}

#[test]
fn a_frog_is_not_waiting_in_a_room_already_seen_or_in_the_sanctuary() {
    let mut run = Run::new(4, 1, None);
    run.descend_for_test();
    for index in 0..run.dungeon.rooms.len() {
        run.frogs.wild.clear();
        run.maybe_frog(index, false);
        assert!(run.frogs.wild.is_empty());
    }
    let sanctuary = run.at;
    assert_eq!(run.dungeon.rooms[sanctuary].kind, RoomKind::Sanctuary);
    run.maybe_frog(sanctuary, true);
    assert!(run.frogs.wild.is_empty());
}

#[test]
fn a_frog_hops_about_its_room() {
    let mut run = quiet();
    frog_at(&mut run, 10.0, 8.0);
    let start = (run.frogs.wild[0].x, run.frogs.wild[0].y);
    let mut hopped = false;
    for _ in 0..300 {
        run.step(&BTreeMap::new());
        let f = &run.frogs.wild[0];
        hopped |= f.hop > 0;
    }
    let f = &run.frogs.wild[0];
    assert!(hopped);
    assert!((f.x - start.0).hypot(f.y - start.1) > 0.5);
}

#[test]
fn a_frog_leaps_onto_the_shoulder_of_a_knight_who_comes_near() {
    let mut run = quiet();
    frog_at(&mut run, 30.0, 14.0);
    // Far: nothing.
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    assert!(!run.players[&1].rider);
    // Near: it leaps, and a moment later it is aboard.
    run.players.get_mut(&1).unwrap().x = 28.0;
    run.step(&BTreeMap::new());
    assert!(run.frogs.wild[0].leap.is_some(), "the leap begins");
    assert!(!run.players[&1].rider, "not at once");
    for _ in 0..LEAP_TICKS {
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&1].rider, "aboard");
    assert!(run.frogs.wild.is_empty());
    assert!(run.cues.iter().any(|c| c == "frog_boards"));
    assert!(run.found.as_ref().unwrap().2.contains("shoulder"));
}

#[test]
fn only_one_frog_to_a_knight_and_the_second_waits_for_another() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().rider = true;
    frog_at(&mut run, 25.0, 14.0);
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.frogs.wild.len(), 1, "still hopping, unboarded");
    assert!(run.frogs.wild[0].leap.is_none());
    // The other knight comes by: that one takes it.
    let (x, y) = (run.frogs.wild[0].x, run.frogs.wild[0].y);
    let matt = run.players.get_mut(&2).unwrap();
    (matt.x, matt.y) = (x + 1.0, y);
    for _ in 0..LEAP_TICKS + 2 {
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&2].rider);
    assert!(run.frogs.wild.is_empty());
}

#[test]
fn a_fallen_carrier_drops_the_frog() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().rider = true;
    run.players.get_mut(&1).unwrap().hp = 0;
    run.step(&BTreeMap::new());
    assert!(!run.players[&1].rider);
    assert_eq!(run.frogs.wild.len(), 1, "it hops off");
    // The fallen cannot take it back; a standing knight can.
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    assert!(!run.players[&1].rider);
}

#[test]
fn the_frog_rides_through_the_doors_until_the_sanctuary() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().rider = true;
    let at = run.at;
    run.enter_for_test(at);
    assert!(run.players[&1].rider, "a new room does not shake it off");
    run.cues.clear();
    to_the_sanctuary(&mut run);
    assert!(!run.players[&1].rider, "hopped down at the Sanctuary");
    assert_eq!(run.frogs.shown.len(), 1, "and is someone");
    assert!(run.cues.iter().any(|c| c.starts_with("frog_")));
}

#[test]
fn a_gift_is_a_princess_a_prince_or_a_clown_and_the_clown_is_one_in_three() {
    let mut counts = [0u32; 3];
    for seed in 0..300 {
        let mut run = Run::new(seed, 1, None);
        run.players.get_mut(&1).unwrap().rider = true;
        run.tick = seed;
        to_the_sanctuary_force(&mut run);
        match run.frogs.shown[0].gift {
            Gift::Princess => counts[0] += 1,
            Gift::Prince => counts[1] += 1,
            Gift::Clown => counts[2] += 1,
        }
    }
    assert!(counts.iter().all(|&n| n > 40), "{counts:?}");
    assert!(counts[2] > 70 && counts[2] < 140, "a clown about a third: {counts:?}");
}

fn to_the_sanctuary_force(run: &mut Run) {
    let at = run.at;
    run.dungeon.rooms[at].kind = RoomKind::Sanctuary;
    run.hallow();
}

#[test]
fn a_princess_mends_the_party_and_lends_ten_health_for_the_floor() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().hp = 10;
    run.give(Gift::Princess);
    for hero in run.players.values() {
        assert_eq!((hero.hp, hero.max_hp), (110, 110));
    }
    run.lift_gifts();
    for hero in run.players.values() {
        assert_eq!((hero.hp, hero.max_hp), (100, 100));
        assert!(!hero.boon.any());
    }
}

#[test]
fn a_prince_makes_shots_harder_and_quicker_for_the_floor() {
    let mut run = quiet();
    let (dmg, cool) = {
        let h = &run.players[&1];
        (h.scaled(100), h.cooldown(60))
    };
    run.give(Gift::Prince);
    let h = &run.players[&1];
    assert_eq!(h.scaled(100), dmg + dmg * PRINCE_DAMAGE / 100);
    assert!(h.cooldown(60) < cool);
    run.lift_gifts();
    let h = &run.players[&1];
    assert_eq!((h.scaled(100), h.cooldown(60)), (dmg, cool));
}

#[test]
fn a_clown_takes_a_quarter_of_everyones_health_for_the_floor_and_the_next_gives_it_back() {
    let mut run = quiet();
    run.players.get_mut(&2).unwrap().hp = 60;
    run.give(Gift::Clown);
    let (a, b) = (&run.players[&1], &run.players[&2]);
    assert_eq!((a.hp, a.max_hp), (75, 75), "most and current alike");
    assert_eq!((b.hp, b.max_hp), (45, 75));
    assert_eq!(a.boon.from, Some(Gift::Clown));
    // The floor goes on: still cut.
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].max_hp, 75);
    // The stairs: the most comes back, and the stairs mend a little.
    run.descend_for_test();
    let a = &run.players[&1];
    assert_eq!(a.max_hp, 100, "restored on the next floor");
    assert!(!a.boon.any());
    assert!(a.hp >= 75, "{}", a.hp);
}

#[test]
fn the_herald_names_the_clown_and_what_it_costs() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().rider = true;
    // Force the roll: the clown is whatever the roll gives, so look it up.
    to_the_sanctuary_force(&mut run);
    let gift = run.frogs.shown[0].gift;
    let line = &run.found.as_ref().unwrap().2;
    assert_eq!(line, gift.herald());
    assert_eq!(
        Gift::Clown.herald(),
        "A clown! Your whole party feels 25% worse."
    );
}

#[test]
fn the_figure_lingers_then_leaves_and_the_hud_tells_of_frog_and_gift() {
    let mut run = quiet();
    run.players.get_mut(&1).unwrap().rider = true;
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    assert!(hud.perks.iter().any(|p| p.contains("frog")), "{:?}", hud.perks);
    to_the_sanctuary_force(&mut run);
    run.give(Gift::Prince);
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    assert!(hud.perks.iter().any(|p| p.contains("prince")));
    for _ in 0..(LINGER + PUFF + 2) as u32 {
        run.step(&BTreeMap::new());
    }
    assert!(run.frogs.shown.is_empty(), "they take their leave");
}

#[test]
fn a_frog_changes_the_frame_for_a_browser_guest() {
    let mut run = quiet();
    let plain = crate::render::overworld::arena::frame_for(&run, 384, 224, Some(1));
    frog_at(&mut run, 20.0, 14.0);
    let with = crate::render::overworld::arena::frame_for(&run, 384, 224, Some(1));
    assert_ne!(plain.rgb_bytes(), with.rgb_bytes());
    run.frogs.wild.clear();
    run.players.get_mut(&1).unwrap().rider = true;
    let carried = crate::render::overworld::arena::frame_for(&run, 384, 224, Some(1));
    assert_ne!(plain.rgb_bytes(), carried.rgb_bytes());
    let _ = input(Input::default());
}

fn save(img: &crate::render::overworld::Img, name: &str) {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_frog_shots -- --ignored`
#[test]
#[ignore]
fn write_frog_shots() {
    use crate::render::overworld::{Img, arena};
    // The sprites on black paper, two animation frames.
    let mut sheet = Img::black(9 * 24 + 4, 2 * 24 + 4);
    for (row, tick) in [0u32, 12].into_iter().enumerate() {
        for (col, im) in arena::frog_sprites(tick).iter().enumerate() {
            sheet.stamp(
                im,
                2 + col as i32 * 24 + (24 - im.w) / 2,
                2 + row as i32 * 24 + (24 - im.h),
            );
        }
    }
    save(&sheet, "frog_sheet");
    // In a room: a frog hopping, and one riding a knight's shoulder.
    let mut run = quiet();
    frog_at(&mut run, 16.0, 14.0);
    run.frogs.wild[0].hop = 5;
    run.frogs.wild[0].from = (15.0, 14.0);
    run.frogs.wild[0].to = (17.0, 14.0);
    run.players.get_mut(&1).unwrap().rider = true;
    run.players.get_mut(&2).unwrap().x = 30.0;
    run.players.get_mut(&2).unwrap().y = 14.0;
    save(&arena::frame_for(&run, 384, 224, Some(1)), "frog_room");
    // The Sanctuary: the puff, then each figure with its plate.
    for (name, gift, age) in [
        ("frog_puff", Gift::Princess, 9u64),
        ("frog_princess", Gift::Princess, 60),
        ("frog_prince", Gift::Prince, 60),
        ("frog_clown", Gift::Clown, 60),
    ] {
        let mut run = quiet();
        let at = run.at;
        run.dungeon.rooms[at].kind = RoomKind::Sanctuary;
        run.tick = 200;
        run.frogs.shown.push(Shown {
            gift,
            x: 26.0,
            y: 14.0,
            born: 200 - age,
        });
        save(&arena::frame_for(&run, 384, 224, Some(1)), name);
    }
}

#[test]
fn health_grown_under_the_clowns_curse_never_lifts_above_the_cap() {
    let mut run = quiet();
    let cap = run.players[&1].hp_cap();
    for hero in run.players.values_mut() {
        (hero.max_hp, hero.hp) = (cap, cap);
    }
    run.give(Gift::Clown);
    // Cursed, the knight mends and grows back to the cap.
    for hero in run.players.values_mut() {
        (hero.max_hp, hero.hp) = (hero.hp_cap(), hero.hp_cap());
    }
    run.lift_gifts();
    for hero in run.players.values() {
        assert!(hero.max_hp <= hero.hp_cap(), "{} > {}", hero.max_hp, hero.hp_cap());
        assert!(hero.hp <= hero.max_hp);
    }
}

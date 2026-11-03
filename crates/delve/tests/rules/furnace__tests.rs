use super::*;

fn arena() -> Run {
    let mut run = Run::new(17, 1, Some("Friend"));
    run.dungeon = crawl_floor(2, Pack::Cavern, 17);
    let at = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    run.enter_for_test(at);
    let room = &mut run.dungeon.rooms[at];
    for y in 1..room.rows - 1 {
        for x in 1..room.cols - 1 {
            room.set_for_test(x, y, Tile::Floor);
        }
    }
    run.enemies.clear();
    run.projectiles.clear();
    run.calm_for_test();
    run.works.blazes.clear();
    let i = run.bosses.iter().position(|b| b.id == ID).unwrap();
    run.spawn_boss_at_for_test(i, 24.0, 14.0);
    for h in run.players.values_mut() {
        (h.x, h.y) = (12.0, 20.0);
        h.invulnerable = 10000;
    }
    run.phase = Phase::Fighting;
    run.enemies[0].seen_hp = run.enemies[0].hp;
    run
}

fn boss(run: &Run) -> &Boss {
    &run.bosses[run.enemies[0].boss.unwrap() as usize]
}

fn windup(run: &Run, kind: Windup) -> u8 {
    boss(run).windups.iter().position(|u| u.kind == kind).unwrap() as u8
}

/// A blow of `dealt` this tick, weighed by the furnace: what it took.
fn blow(run: &mut Run, dealt: u32) -> u32 {
    let e = &mut run.enemies[0];
    e.seen_hp = e.hp;
    e.hp -= dealt;
    run.tend_furnace();
    let e = &run.enemies[0];
    e.seen_hp - e.hp
}

fn at_share(run: &mut Run, share: f32) {
    let e = &mut run.enemies[0];
    e.hp = (e.max_hp as f32 * share) as u32;
    e.seen_hp = e.hp;
}

#[test]
fn furnace_breathes_from_the_opening_and_leaps_once_stoked() {
    let run = arena();
    let from = |stage: Stage| -> Vec<Windup> {
        boss(&run).windups.iter().filter(|u| u.from <= stage).map(|u| u.kind).collect()
    };
    assert_eq!(from(Stage::Open), [Windup::Inhale]);
    assert_eq!(from(Stage::Middle), [Windup::Leap, Windup::Inhale]);
}

#[test]
fn furnace_iron_lets_a_third_through_its_open_core_takes_more_and_molten_takes_all() {
    let mut run = arena();
    assert!(armored(&run.enemies[0], boss(&run)));
    assert_eq!(blow(&mut run, 100), 30);

    let inhale = windup(&run, Windup::Inhale);
    let e = &mut run.enemies[0];
    (e.stage, e.timer, e.signature.windup) = (signatures::DAZED, 5, inhale);
    assert!(venting(&run.enemies[0], boss(&run)));
    assert_eq!(blow(&mut run, 100), 150);

    run.enemies[0].stage = 0;
    at_share(&mut run, 0.2);
    assert!(molten(&run.enemies[0], boss(&run)));
    assert_eq!(blow(&mut run, 40), 40);
}

#[test]
fn furnace_never_weighs_a_killing_blow_or_another_guardian() {
    let mut run = arena();
    let e = &mut run.enemies[0];
    e.seen_hp = e.hp;
    e.hp = 0;
    run.tend_furnace();
    assert_eq!(run.enemies[0].hp, 0);

    let other = bosses::builtin().into_iter().find(|b| !is(b)).unwrap();
    assert_eq!(takes(&run.enemies[0], &other), 100);
    for up in &boss(&run).windups {
        assert_eq!(vent_for(&other, up), 0);
    }
}

#[test]
fn furnace_breath_holds_the_door_open_past_its_recovery() {
    let mut run = arena();
    let inhale = windup(&run, Windup::Inhale);
    let up = boss(&run).windups[usize::from(inhale)].clone();
    assert_eq!(vent_for(boss(&run), &up), VENT);
    let open = signatures::spent_for(&up) + VENT;
    let grid = Grid {
        room: &run.dungeon.rooms[run.at],
        barred: true,
    };
    let e = &mut run.enemies[0];
    (e.stage, e.timer, e.signature.windup) = (signatures::DAZED, signatures::spent_for(&up), inhale);
    let b = &run.bosses[e.boss.unwrap() as usize];
    let mut deeds = foes::Deeds::default();
    assert!(signatures::wind(e, b, &[(12.0, 20.0)], &grid, 17, &mut deeds));
    assert_eq!(e.stage, signatures::DAZED, "still open past the usual recovery");
    e.timer = open;
    signatures::wind(e, b, &[(12.0, 20.0)], &grid, 17, &mut deeds);
    assert_eq!(e.stage, 0, "the door shuts at last");
}

#[test]
fn furnace_stoked_landings_leave_one_pit_and_the_opening_none() {
    let mut run = arena();
    let leap = windup(&run, Windup::Leap);
    let e = &mut run.enemies[0];
    (e.stage, e.timer, e.signature.windup) = (signatures::DAZED, 0, leap);
    run.tend_furnace();
    assert!(run.works.blazes.is_empty(), "no pit in the opening");

    at_share(&mut run, 0.6);
    run.tend_furnace();
    // Held in time on its landing, it lights the spot only once.
    run.tend_furnace();
    assert_eq!(run.works.blazes.len(), 1);
    let pit = run.works.blazes[0];
    assert!(pit.hostile());
    assert_eq!((pit.x, pit.y, pit.radius, pit.life), (24.0, 14.0, PIT_RADIUS, PIT_LIFE));
}

#[test]
fn furnace_molten_wake_burns_behind_it_and_keeps_to_its_cap() {
    let mut run = arena();
    at_share(&mut run, 0.2);
    for step in 0..(BLAZES_MOST as u32 + 6) * WAKE_EVERY {
        let e = &mut run.enemies[0];
        e.age = step;
        e.x = 6.0 + step as f32 * 0.05;
        run.tend_furnace();
    }
    let own = run.works.blazes.iter().filter(|b| b.hostile()).count();
    assert_eq!(own, BLAZES_MOST);
    assert!(run.works.blazes.iter().all(|b| b.radius == WAKE_RADIUS));
}

#[test]
fn furnace_a_knights_fire_is_never_put_out_for_its_wake() {
    let mut run = arena();
    for k in 0..24 {
        run.works.blazes.push(skills::Blaze {
            x: k as f32,
            y: 2.0,
            age: 0,
            life: 100,
            owner: 1,
            power: 3,
            radius: 1.0,
        });
    }
    at_share(&mut run, 0.2);
    run.enemies[0].age = 0;
    run.tend_furnace();
    assert_eq!(run.works.blazes.len(), 24);
    assert!(run.works.blazes.iter().all(|b| b.owner == 1));
}

#[test]
fn furnace_each_new_stage_is_called_out_and_its_own_flame_spares_it() {
    let mut run = arena();
    at_share(&mut run, bosses::MIDDLE_FROM + 0.01);
    run.sounds.clear();
    blow(&mut run, 100);
    assert_eq!(run.enemies[0].cry, CRY);
    assert!(run.sounds.contains(&"vent_fire"));
    assert!(fireproof(&run.enemies[0], &run.bosses));
}

/// Cinderjaw's fight, a frame for each of its states, written to
/// `FURNACE_SHOTS` (required).
/// `cargo test furnace_shots -- --ignored`
#[test]
fn furnace_names_its_iron_the_first_few_times_it_turns_a_blow() {
    let mut run = arena();
    assert!(!signatures::hinting(&run.enemies[0]));
    blow(&mut run, 60);
    assert!(signatures::hinting(&run.enemies[0]), "the first clang names it");
    let named = crate::render::overworld::arena::frame(&run, 384, 224);
    let mut unnamed = run.clone();
    unnamed.enemies[0].hint = 0;
    let quiet = crate::render::overworld::arena::frame(&unnamed, 384, 224);
    assert_ne!(quiet.rgb_bytes(), named.rgb_bytes(), "and the word is drawn");
    for _ in 0..signatures::HINT_SHOW {
        run.tend_furnace();
    }
    assert!(!signatures::hinting(&run.enemies[0]), "for a moment only");
    blow(&mut run, 60);
    assert_eq!(run.enemies[0].hints, 1, "not again so soon");

    // Its open core is no lesson in iron.
    let inhale = windup(&run, Windup::Inhale);
    let e = &mut run.enemies[0];
    (e.hint, e.stage, e.timer, e.signature.windup) = (0, signatures::DAZED, 5, inhale);
    blow(&mut run, 60);
    assert_eq!(run.enemies[0].hints, 1);

    run.enemies[0].stage = 0;
    for _ in 0..3 * signatures::HINTS {
        run.enemies[0].hint = 0;
        blow(&mut run, 60);
    }
    assert_eq!(run.enemies[0].hints, signatures::HINTS, "and only the first few times");
}

#[test]
#[ignore]
fn furnace_shots() {
    let dir = std::path::PathBuf::from(
        std::env::var("FURNACE_SHOTS").expect("set FURNACE_SHOTS to the output directory"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = arena();
    blow(&mut run, 60);
    run.enemies[0].hurt = 4;
    save(&run, "1-iron-clang");

    let inhale = windup(&run, Windup::Inhale);
    let e = &mut run.enemies[0];
    (e.hurt, e.stage, e.timer, e.signature.windup) = (0, signatures::WIND, 40, inhale);
    save(&run, "2-breath");
    let e = &mut run.enemies[0];
    (e.stage, e.timer) = (signatures::DAZED, 20);
    save(&run, "3-vent");

    let leap = windup(&run, Windup::Leap);
    at_share(&mut run, 0.6);
    let e = &mut run.enemies[0];
    (e.stage, e.timer, e.signature.windup) = (signatures::DAZED, 0, leap);
    run.tend_furnace();
    run.enemies[0].cry = CRY;
    save(&run, "4-stoked-pit");

    run.works.blazes.clear();
    at_share(&mut run, 0.2);
    for step in 0..4 * WAKE_EVERY {
        let e = &mut run.enemies[0];
        (e.stage, e.age, e.x) = (0, step, 14.0 + step as f32 * 0.12);
        run.tend_furnace();
        for b in &mut run.works.blazes {
            b.age += 1;
        }
    }
    run.enemies[0].cry = CRY;
    save(&run, "5-molten");
}

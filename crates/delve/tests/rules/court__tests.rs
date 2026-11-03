use super::*;

fn arena() -> Run {
    let mut run = Run::new(17, 1, Some("Friend"));
    run.dungeon = crawl_floor(1, Pack::Crypt, 17);
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
    let i = run.bosses.iter().position(|b| b.id == ID).unwrap_or_else(|| {
        run.bosses.push(bosses::checked(ID).unwrap());
        run.bosses.len() - 1
    });
    run.spawn_boss_at_for_test(i, 24.0, 14.0);
    for h in run.players.values_mut() {
        (h.x, h.y) = (30.0, 14.0);
        h.invulnerable = 10000;
    }
    run.phase = Phase::Fighting;
    let e = &mut run.enemies[0];
    e.seen_hp = e.hp;
    e.dir = (1.0, 0.0);
    run
}

fn boss(run: &Run) -> &Boss {
    &run.bosses[run.enemies[0].boss.unwrap() as usize]
}

/// A blow of `dealt` struck from `from` this tick, through the real hook
/// and the ward: what it took.
fn blow(run: &mut Run, dealt: u32, from: (f32, f32)) -> u32 {
    let extra = signatures::rear_bonus(&run.enemies[0], &run.bosses, from, dealt);
    let e = &mut run.enemies[0];
    e.seen_hp = e.hp;
    e.hp -= dealt + extra;
    run.tend_court();
    let e = &run.enemies[0];
    e.seen_hp - e.hp
}

fn at_share(run: &mut Run, share: f32) {
    let e = &mut run.enemies[0];
    e.hp = (e.max_hp as f32 * share) as u32;
    e.seen_hp = e.hp;
}

#[test]
fn court_turns_toward_the_nearest_knight_no_faster_than_its_rate() {
    let mut run = arena();
    let heroes = [(24.0, 30.0), (24.0, 2.0)];
    let mut e = run.enemies[0].clone();
    let b = boss(&run).clone();
    turn(&mut e, &b, &heroes);
    let step = turn_rate(&e, &b).to_radians() * DT;
    let a = e.dir.1.atan2(e.dir.0);
    assert!((a.abs() - step).abs() < 1e-4, "turned {a}, may turn {step}");
    assert!(a < 0.0, "toward the nearer knight, above it");
    for _ in 0..3 * HZ {
        turn(&mut e, &b, &heroes);
    }
    assert!(e.dir.1 < -0.999, "faces the knight at last: {:?}", e.dir);

    // Spent after a charge-up it stands as it is.
    e.stage = signatures::DAZED;
    let held = e.dir;
    turn(&mut e, &b, &[(40.0, 14.0)]);
    assert_eq!(e.dir, held);
    run.enemies[0] = e;
}

#[test]
fn court_a_knight_eight_units_out_circles_faster_than_it_turns_before_its_rage() {
    let mut run = arena();
    for share in [1.0, 0.5] {
        at_share(&mut run, share);
        let rate = turn_rate(&run.enemies[0], boss(&run)).to_radians();
        assert!(rate * 8.0 < HERO_SPEED, "at {share}: {rate} rad/s");
    }
}

#[test]
fn court_ward_halves_the_front_the_back_lands_past_whole_and_the_crack_opens_the_front() {
    let mut run = arena();
    let (front, back) = ((30.0, 14.0), (18.0, 14.0));
    assert_eq!(blow(&mut run, 100, front), WARD_TAKES);
    assert_eq!(blow(&mut run, 100, back), BACK_TAKES);
    assert!(BACK_TAKES > 100);

    at_share(&mut run, 0.2);
    assert!(cracked(&run.enemies[0], boss(&run)));
    assert_eq!(blow(&mut run, 100, front), 100);
    at_share(&mut run, 0.2);
    assert_eq!(blow(&mut run, 100, back), BACK_TAKES);
}

#[test]
fn court_killing_blows_stand_and_the_rippers_back_is_its_own() {
    let mut run = arena();
    let e = &mut run.enemies[0];
    e.seen_hp = e.hp;
    e.hp = 0;
    run.tend_court();
    assert_eq!(run.enemies[0].hp, 0);

    let ripper = bosses::checked(signatures::RIPPER).unwrap();
    assert!(!is(&ripper));
    let mut e = run.enemies[0].clone();
    (e.hp, e.stage) = (100, 0);
    e.boss = Some(run.bosses.len() as u8);
    let mut bosses = run.bosses.clone();
    bosses.push(ripper);
    assert_eq!(signatures::rear_bonus(&e, &bosses, (18.0, 14.0), 100), 0, "not spent, no bonus");
    e.stage = signatures::DAZED;
    assert_eq!(signatures::rear_bonus(&e, &bosses, (18.0, 14.0), 100), 50);
}

#[test]
fn court_its_broken_ward_is_called_out() {
    let mut run = arena();
    let rage = boss(&run).rage;
    at_share(&mut run, rage + 0.02);
    run.sounds.clear();
    blow(&mut run, 60, (30.0, 14.0));
    assert!(cracked(&run.enemies[0], boss(&run)), "{}", run.enemies[0].hp);
    assert_eq!(run.enemies[0].cry, CRY);
    assert!(run.sounds.contains(&"crack_open"));
}

/// The Relic Liege's fight, a frame for each of its states, written to
/// `COURT_SHOTS` (required).
/// `cargo test court_shots -- --ignored`
#[test]
fn court_names_its_ward_the_first_few_times_it_turns_a_blow_from_the_front() {
    let mut run = arena();
    let (front, back) = ((30.0, 14.0), (18.0, 14.0));
    blow(&mut run, 40, front);
    assert!(signatures::hinting(&run.enemies[0]), "a warded blow names it");
    let named = crate::render::overworld::arena::frame(&run, 384, 224);
    let mut unnamed = run.clone();
    unnamed.enemies[0].hint = 0;
    let quiet = crate::render::overworld::arena::frame(&unnamed, 384, 224);
    assert_ne!(quiet.rgb_bytes(), named.rgb_bytes(), "and the word is drawn");
    for _ in 0..signatures::HINT_SHOW {
        run.tend_court();
    }
    assert!(!signatures::hinting(&run.enemies[0]), "for a moment only");

    // A knight already at its back has the lesson.
    run.enemies[0].hint = 0;
    if let Some(h) = run.players.values_mut().next() {
        (h.x, h.y) = back;
    }
    blow(&mut run, 40, back);
    assert_eq!(run.enemies[0].hints, 1);

    // Nor does a broken ward teach anything.
    at_share(&mut run, 0.2);
    for h in run.players.values_mut() {
        (h.x, h.y) = front;
    }
    blow(&mut run, 40, front);
    assert_eq!(run.enemies[0].hints, 1);
}

#[test]
#[ignore]
fn court_shots() {
    let dir = std::path::PathBuf::from(
        std::env::var("COURT_SHOTS").expect("set COURT_SHOTS to the output directory"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = arena();
    let a = 2.4f32;
    run.enemies[0].dir = (a.cos(), a.sin());
    save(&run, "1-gaze");
    run.enemies[0].stage = signatures::DAZED;
    save(&run, "2-spent");
    run.enemies[0].stage = 0;
    at_share(&mut run, 0.2);
    run.enemies[0].cry = CRY;
    save(&run, "3-ward-broken");
    let mut run = arena();
    blow(&mut run, 40, (30.0, 14.0));
    save(&run, "4-warded");
}

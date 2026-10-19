use super::*;
use std::collections::BTreeMap;

fn fight(run: &mut Run) {
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
}

/// A calm, empty room with the knight low in the middle.
fn quiet() -> Run {
    let mut run = Run::new(4, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run.window = None;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 20.0);
    hero.hp = 100_000;
    hero.max_hp = 100_000;
    run
}

/// One volley of `emitter` from `from` at `at`, into a fresh list.
fn volley(
    run: &Run,
    emitter: Emitter,
    from: (f32, f32),
    at: (f32, f32),
    count: u32,
    step: u8,
) -> Vec<Projectile> {
    let grid = Grid {
        room: &run.dungeon.rooms[run.at],
        barred: false,
    };
    let aim = Aim {
        from,
        at,
        pace: (6.0, 0.0),
        grid: &grid,
        count,
        salt: 0x5eed,
        step,
        kind: Shot::Wisp,
        damage: 12,
        guardian: None,
    };
    let mut shots = Vec::new();
    emit(emitter, &aim, &mut shots);
    shots
}

/// Advance motion only; room collision and damage have separate run tests.
fn fly(shots: &mut Vec<Projectile>, at: (f32, f32), ticks: u32) {
    let knights = [(1u32, at.0, at.1)];
    for _ in 0..ticks {
        let mut pops = Vec::new();
        for shot in shots.iter_mut() {
            if shot.traits.path.beam.is_some() {
                shot.traits.path.age += 1;
                shot.ttl = shot.ttl.saturating_sub(1);
                continue;
            }
            if steer(shot, &knights, &mut pops) == Steer::Fly {
                shot.x += shot.vx * DT;
                shot.y += shot.vy * DT;
                shot.ttl = shot.ttl.saturating_sub(1);
            }
        }
        shots.retain(|s| s.ttl > 0);
        shots.extend(pops);
    }
}

fn print(shots: &[Projectile]) -> String {
    serde_json::to_string(shots).unwrap()
}

#[test]
fn every_emitter_is_the_same_from_the_same_seed() {
    let run = quiet();
    for emitter in Emitter::ALL {
        let mut a = volley(&run, emitter, (24.0, 8.0), (24.0, 20.0), 3, 1);
        let mut b = volley(&run, emitter, (24.0, 8.0), (24.0, 20.0), 3, 1);
        assert!(!a.is_empty(), "{emitter:?} fires nothing");
        assert_eq!(print(&a), print(&b), "{emitter:?}");
        fly(&mut a, (24.0, 20.0), 60);
        fly(&mut b, (24.0, 20.0), 60);
        assert_eq!(print(&a), print(&b), "{emitter:?} in flight");
    }
}

#[test]
fn every_emitter_has_a_boss_name_that_reads_back() {
    for emitter in Emitter::ALL {
        assert_eq!(Emitter::named(emitter.name()), Some(emitter));
        assert!(emitter.shots() > 0);
        let raw = format!(
            "name Test\nwhere crypt\nattack {}\nart\nkkkk\n",
            emitter.name()
        );
        let (boss, _) = bosses::check("test", &raw).unwrap();
        assert_eq!(boss.attacks[0].pattern, bosses::Pattern::Named(emitter));
    }
    assert_eq!(Emitter::named("gap-ring"), Some(Emitter::GapRing));
    assert_eq!(Emitter::named("wobble"), None);
}

#[test]
fn a_plain_shot_is_left_as_it_was() {
    let mut shot = Projectile {
        x: 10.0,
        y: 10.0,
        vx: 3.0,
        vy: -2.0,
        hostile: true,
        look: None,
        kind: Shot::Orb,
        damage: 10,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: ShotTraits::default(),
        ttl: 100,
    };
    let mut pops = Vec::new();
    assert_eq!(steer(&mut shot, &[(1, 0.0, 0.0)], &mut pops), Steer::Fly);
    assert_eq!((shot.x, shot.y, shot.vx, shot.vy), (10.0, 10.0, 3.0, -2.0));
    assert_eq!(shot.traits.path, patterns::Path::default());
    assert!(pops.is_empty());
}

#[test]
fn a_gap_ring_leaves_a_gap_a_knight_fits_through() {
    let run = quiet();
    let grid = Grid {
        room: &run.dungeon.rooms[run.at],
        barred: false,
    };
    let fits = 2.0 * (HERO_RADIUS + 0.16);
    for step in 0..=MOST_STEP {
        for count in 0..40 {
            let aim = Aim {
                from: (24.0, 8.0),
                at: (24.0, 20.0),
                pace: (0.0, 0.0),
                grid: &grid,
                count,
                salt: u64::from(count) * 7919,
                step,
                kind: Shot::Wisp,
                damage: 12,
                guardian: None,
            };
            let mut angles = gap_ring(&aim);
            angles.sort_by(f32::total_cmp);
            let widest = angles
                .windows(2)
                .map(|w| w[1] - w[0])
                .chain(std::iter::once(
                    angles[0] + std::f32::consts::TAU - angles[angles.len() - 1],
                ))
                .fold(0.0f32, f32::max);
            assert!(
                widest >= 2.0 * GAP_HALF,
                "step {step} volley {count}: {widest}"
            );
            // Two units out the gap is already wider than a knight.
            let chord = 2.0 * 2.0 * (widest / 2.0).sin();
            assert!(chord > fits, "{chord} vs {fits}");
            // And every other neighbour is closer than the gap.
            let rest = angles
                .windows(2)
                .map(|w| w[1] - w[0])
                .filter(|&d| d < widest)
                .count();
            assert!(rest + 2 >= angles.len(), "one gap only");
        }
    }
}

#[test]
fn a_wall_leaves_one_gap_a_knight_fits_through_and_the_next_wall_moves_it() {
    // On open floor: a wall's slots touching a prop are left out (`wall`).
    let mut run = quiet();
    let at = run.at;
    for row in 1..run.room().rows - 1 {
        for col in 1..run.room().cols - 1 {
            run.dungeon.rooms[at].set_for_test(col, row, Tile::Floor);
        }
    }
    let shots = volley(&run, Emitter::GapWall, (24.0, 8.0), (24.0, 20.0), 5, 0);
    let first: Vec<f32> = {
        let mut xs: Vec<f32> = shots
            .iter()
            .filter(|s| s.traits.path.wait == 0)
            .map(|s| s.x)
            .collect();
        xs.sort_by(f32::total_cmp);
        xs
    };
    let second: Vec<f32> = {
        let mut xs: Vec<f32> = shots
            .iter()
            .filter(|s| s.traits.path.wait > 0)
            .map(|s| s.x)
            .collect();
        xs.sort_by(f32::total_cmp);
        xs
    };
    let gap = |xs: &[f32]| {
        xs.windows(2)
            .map(|w| (w[1] - w[0], w[0]))
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap()
    };
    let (wide, at) = gap(&first);
    assert!(
        wide >= 3.0 - 1e-3 && wide > 2.0 * (HERO_RADIUS + 0.16) + 0.5,
        "{wide}"
    );
    let (wide2, at2) = gap(&second);
    assert!(wide2 >= 3.0 - 1e-3, "{wide2}");
    assert!((at - at2).abs() > 0.5, "the gap moved: {at} {at2}");
}

#[test]
fn a_laser_warns_before_it_burns() {
    let mut run = quiet();
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    hero.shielding = false;
    let full = hero.hp;
    let shots = volley(&run, Emitter::Laser, (24.0, 8.0), (24.0, 20.0), 0, 0);
    assert_eq!(shots.len(), 1);
    let beam = shots[0].traits.path.beam.unwrap();
    assert!(beam.reach > 12.0, "it reaches the knight: {}", beam.reach);
    run.projectiles = shots;
    for tick in 0..u32::from(BEAM_WARN) {
        run.step(&BTreeMap::new());
        let hero = &run.players[&1];
        assert_eq!(hero.hp, full, "struck in its warning at {tick}");
        hero_still(&mut run);
    }
    let mut struck = false;
    for _ in 0..u32::from(BEAM_BURN) + 2 {
        run.step(&BTreeMap::new());
        struck |= run.players[&1].hp < full;
        hero_still(&mut run);
    }
    assert!(struck, "the beam burns");
    assert!(
        run.projectiles.iter().all(|p| p.traits.path.beam.is_none()),
        "and is gone"
    );
    // Struck once, however long it burned.
    let lost = full - run.players[&1].hp;
    assert!(lost > 0 && lost < 100, "{lost}");
}

/// Keep knight 1 where it is and without its hurt grace.
fn hero_still(run: &mut Run) {
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 20.0);
}

#[test]
fn a_sweep_turns_across_its_arc_while_it_burns() {
    let run = quiet();
    let mut shots = volley(&run, Emitter::Sweep, (24.0, 8.0), (24.0, 20.0), 0, 0);
    let first = shots[0].traits.path.beam.unwrap();
    let start = first.angle;
    // A sweep tells longer than a plain beam, and burns across a narrow arc.
    assert_eq!(first.warn, SWEEP_WARN);
    assert_eq!((SWEEP_WARN, SWEEP_BURN, SWEEP_ARC), (30, 18, 1.1));
    assert!(SWEEP_WARN > BEAM_WARN);
    assert_eq!(shots[0].ttl, u32::from(SWEEP_WARN) + u32::from(SWEEP_BURN) + 1);
    let grid = Grid {
        room: &run.dungeon.rooms[run.at],
        barred: false,
    };
    let mut players = BTreeMap::new();
    let mut sounds = Vec::new();
    for _ in 0..u32::from(SWEEP_WARN) {
        assert!(
            !shots[0].traits.path.beam.unwrap().burning(shots[0].traits.path.age),
            "it warns the whole {SWEEP_WARN} ticks"
        );
        shine(&mut shots[0], &mut players, &grid, &mut sounds, &mut feel::Feel::default());
    }
    assert_eq!(
        shots[0].traits.path.beam.unwrap().angle,
        start,
        "still while it warns"
    );
    for _ in 0..u32::from(SWEEP_BURN) {
        shine(&mut shots[0], &mut players, &grid, &mut sounds, &mut feel::Feel::default());
    }
    let end = shots[0].traits.path.beam.unwrap().angle;
    assert!(
        ((end - start).abs() - SWEEP_ARC).abs() < 1e-3,
        "{start} -> {end} spans {SWEEP_ARC}"
    );
    // A plain beam keeps its shorter tell.
    let laser = volley(&run, Emitter::Laser, (24.0, 8.0), (24.0, 20.0), 0, 0);
    assert_eq!(laser[0].traits.path.beam.unwrap().warn, BEAM_WARN);
}

#[test]
fn a_beam_strikes_for_its_share_of_the_shots_damage() {
    assert_eq!(BEAM_SHARE, 60);
    let mut run = quiet();
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    hero.shielding = false;
    let mut shots = volley(&run, Emitter::Laser, (24.0, 8.0), (24.0, 20.0), 0, 0);
    shots[0].damage = 100;
    run.projectiles = shots;
    for _ in 0..u32::from(BEAM_WARN) {
        run.step(&BTreeMap::new());
        hero_still(&mut run);
    }
    let before = run.players[&1].clone();
    assert_eq!(before.invulnerable, 0);
    for _ in 0..u32::from(BEAM_BURN) + 2 {
        run.step(&BTreeMap::new());
        hero_still(&mut run);
    }
    let lost = before.hp - run.players[&1].hp;
    let (mut share, mut whole) = (before.clone(), before.clone());
    share.hurt(100 * BEAM_SHARE / 100);
    whole.hurt(100);
    assert_eq!(lost, before.hp - share.hp, "60% of the shot's damage");
    assert!(lost < before.hp - whole.hp, "less than the shot whole");
}

#[test]
fn rain_and_crossfire_are_marked_before_they_fly() {
    let run = quiet();
    for emitter in [Emitter::Rain, Emitter::Crossfire] {
        let shots = volley(&run, emitter, (24.0, 8.0), (24.0, 14.0), 0, 0);
        assert!(shots.len() >= 4, "{emitter:?}: {}", shots.len());
        for shot in &shots {
            assert!(
                shot.traits.path.mark && shot.traits.path.wait >= 18,
                "{emitter:?}"
            );
            assert!(!run.room().width().is_nan());
        }
    }
    // A waiting shot strikes nobody, however close.
    let mut run = quiet();
    run.step(&BTreeMap::new());
    run.players.get_mut(&1).unwrap().invulnerable = 0;
    let full = run.players[&1].hp;
    let mut shots = volley(&run, Emitter::Crossfire, (24.0, 8.0), (24.0, 20.0), 0, 0);
    for shot in &mut shots {
        (shot.x, shot.y) = (24.0, 20.0);
    }
    run.projectiles = shots;
    for _ in 0..10 {
        run.step(&BTreeMap::new());
        hero_still(&mut run);
    }
    assert_eq!(run.players[&1].hp, full);
}

#[test]
fn delayed_shots_stay_harmless_through_the_whole_wait() {
    let run = quiet();
    for emitter in [Emitter::Burst, Emitter::Rain, Emitter::Crossfire] {
        let mut shot = volley(&run, emitter, (24.0, 8.0), (24.0, 20.0), 0, 0)
            .into_iter()
            .find(|s| s.traits.path.wait > 0)
            .unwrap();
        let wait = shot.traits.path.wait;
        for _ in 0..wait {
            assert_eq!(steer(&mut shot, &[], &mut Vec::new()), Steer::Hold);
            assert!(!shot.in_flight());
        }
        assert_eq!(steer(&mut shot, &[], &mut Vec::new()), Steer::Fly);
        assert!(shot.in_flight());
    }
}

#[test]
fn mines_hang_then_pop_and_splits_burst() {
    let run = quiet();
    for (emitter, ring) in [(Emitter::Mines, 6), (Emitter::Split, 6)] {
        let mut shots = volley(&run, emitter, (24.0, 8.0), (24.0, 20.0), 0, 0);
        let n = shots.len();
        fly(&mut shots, (40.0, 26.0), 32);
        if emitter == Emitter::Mines {
            assert!(
                shots.iter().all(|s| s.vx.hypot(s.vy) < 0.01),
                "mines hang still"
            );
        }
        fly(&mut shots, (40.0, 26.0), 28);
        assert_eq!(shots.len(), n * ring, "{emitter:?} popped");
        assert!(
            shots
                .iter()
                .all(|s| s.traits.path == patterns::Path::default())
        );
    }
}

#[test]
fn a_reaim_ring_stops_then_flies_at_the_knight() {
    let run = quiet();
    let mut shots = volley(&run, Emitter::Reaim, (24.0, 8.0), (24.0, 20.0), 0, 0);
    fly(&mut shots, (40.0, 20.0), 30);
    assert!(shots.iter().all(|s| s.vx.hypot(s.vy) < 0.01), "stopped");
    for s in &shots {
        let out = (s.x - 24.0).hypot(s.y - 8.0);
        assert!((2.5..4.0).contains(&out), "hangs a few steps out: {out}");
    }
    fly(&mut shots, (40.0, 20.0), 20);
    for s in &shots {
        let to = (20.0 - s.y).atan2(40.0 - s.x);
        let now = s.vy.atan2(s.vx);
        assert!((to - now).sin().abs() < 0.1, "re-aimed at the knight");
    }
}

#[test]
fn shots_stay_slower_than_a_knight() {
    let run = quiet();
    for emitter in Emitter::ALL {
        for step in [0, MOST_STEP] {
            let mut shots = volley(&run, emitter, (24.0, 8.0), (24.0, 20.0), 1, step);
            for _ in 0..4 {
                fly(&mut shots, (24.0, 20.0), 15);
                for s in &shots {
                    let speed = s.vx.hypot(s.vy);
                    assert!(speed <= SHOT_TOP && SHOT_TOP < 13.0, "{emitter:?} at {speed}");
                }
            }
        }
    }
}

#[test]
fn more_knights_thicken_a_volley_and_never_thin_it() {
    let sight = |heads, depth| Sight {
        knights: &[],
        paces: &[],
        pack: Pack::Crypt,
        depth,
        heads,
    };
    for depth in 1..=6 {
        let mut last = 0;
        for heads in 1..=4 {
            let step = sight(heads, depth).step(false);
            assert!(step >= last);
            last = step;
        }
        assert!(sight(4, depth).step(false) > sight(1, depth).step(false));
        assert!(sight(1, depth).step(true) >= sight(1, depth).step(false));
    }
    let run = quiet();
    for emitter in Emitter::ALL {
        let one = volley(&run, emitter, (24.0, 8.0), (24.0, 20.0), 1, 0).len();
        let four = volley(&run, emitter, (24.0, 8.0), (24.0, 20.0), 1, 3).len();
        assert!(four >= one, "{emitter:?}");
    }
}

#[test]
fn every_floor_throws_its_own_shapes() {
    use EnemyKind::*;
    let mut seen = std::collections::BTreeSet::new();
    for pack in [
        Pack::Crypt,
        Pack::Cavern,
        Pack::Hellforge,
        Pack::Archive,
        Pack::Fungal,
        Pack::Unknown,
    ] {
        let mut here = std::collections::BTreeSet::new();
        for &(kind, _) in pack.roster_at(3) {
            if let Some(sig) = signature(kind, pack, false) {
                for &(e, _) in sig.chain {
                    here.insert(e.name());
                }
            }
            if let Some(sig) = signature(kind, pack, true) {
                assert!(sig.chain.len() >= signature(kind, pack, false).unwrap().chain.len());
                for &(e, _) in sig.chain {
                    here.insert(e.name());
                }
            }
        }
        assert!(here.len() >= 3, "{pack:?}: {here:?}");
        seen.extend(here);
    }
    // Between them, the floors use the whole vocabulary.
    for emitter in Emitter::ALL {
        assert!(seen.contains(emitter.name()), "{emitter:?} unused");
    }
    // The crypt's skeleton and the mines' differ.
    let crypt = signature(Skeleton, Pack::Crypt, false).unwrap().chain[0].0;
    let mines = signature(Skeleton, Pack::Cavern, false).unwrap().chain[0].0;
    assert_ne!(crypt, mines);
}

#[test]
fn monsters_fire_their_signatures_in_a_room() {
    for (pack, kind, look) in [
        (Pack::Crypt, EnemyKind::Wraith, Shot::Wisp),
        (Pack::Cavern, EnemyKind::Skeleton, Shot::Ore),
        (Pack::Archive, EnemyKind::Wraith, Shot::Page),
        (Pack::Fungal, EnemyKind::Imp, Shot::Spore),
        (Pack::Hellforge, EnemyKind::Imp, Shot::Flame),
    ] {
        let mut run = quiet();
        run.dungeon.pack = pack;
        // Past the starter floors, where volleys keep their own pace.
        run.dungeon.depth = 3;
        run.spawn_at_for_test(kind, 24.0, 8.0);
        let mut fired = false;
        for _ in 0..200 {
            run.step(&BTreeMap::new());
            fired |= run.projectiles.iter().any(|p| p.hostile && p.kind == look);
        }
        assert!(fired, "{pack:?} {kind:?}");
    }
}

#[test]
fn lead_patterns_follow_spread_aggro_and_the_chosen_knights_pace() {
    let mut run = quiet();
    run.join_seat(2, "Moving");
    // An imp outside the Mines and the Keep leads its shots (the Mines'
    // lobs ore, the Keep's quickens a ring).
    run.dungeon.pack = Pack::Crypt;
    let at = run.at;
    for row in 1..run.room().rows - 1 {
        for col in 1..run.room().cols - 1 {
            run.dungeon.rooms[at].set_for_test(col, row, Tile::Floor);
        }
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (12.0, 14.0);
    let hero = run.players.get_mut(&2).unwrap();
    (hero.x, hero.y) = (36.0, 14.0);
    let before = (hero.x, hero.y);
    run.spawn_at_for_test(EnemyKind::Imp, 20.0, 14.0);
    let enemy = run.enemies.last_mut().unwrap();
    enemy.wits.foe = 2;
    // Floor one's pace (`STARTER_PACE`) stretches the wait between volleys.
    let every = signature(enemy.kind, Pack::Crypt, false).unwrap().every * STARTER_PACE[0] / 100;
    enemy.age = every * 2 - enemy.id * 13 % every - 1;
    run.step(&BTreeMap::from([(
        2,
        Input {
            move_y: -1,
            ..Default::default()
        },
    )]));
    let shots: Vec<_> = run
        .projectiles
        .iter()
        .filter(|p| p.kind == Shot::Ember)
        .collect();
    assert_eq!(shots.len(), 5, "party thickness is kept with spread aggro");
    let hero = &run.players[&2];
    let enemy = &run.enemies[0];
    let ahead = (
        hero.x + (hero.x - before.0) / DT * LEAD_AHEAD,
        hero.y + (hero.y - before.1) / DT * LEAD_AHEAD,
    );
    let expected = (ahead.1 - enemy.y).atan2(ahead.0 - enemy.x);
    let center = shots[shots.len() / 2];
    assert!((center.vy.atan2(center.vx) - expected).abs() < 1e-5);
    assert!(
        center.vx > 0.0,
        "the nearer, unassigned knight is to the left"
    );
}

#[test]
fn the_shot_cap_holds_under_a_crowd_of_patterns() {
    let mut run = quiet();
    run.dungeon.pack = Pack::Hellforge;
    run.dungeon.depth = 6;
    for i in 0..24 {
        let kind = if i % 2 == 0 {
            EnemyKind::Demon
        } else {
            EnemyKind::Dragon
        };
        run.spawn_at_for_test(
            kind,
            6.0 + (i % 12) as f32 * 3.0,
            4.0 + (i / 12) as f32 * 3.0,
        );
        let e = run.enemies.last_mut().unwrap();
        e.elite = true;
        e.hp = 100_000;
        e.max_hp = 100_000;
    }
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    let mut most = 0;
    for _ in 0..240 {
        run.step(&BTreeMap::new());
        assert!(run.projectiles.len() <= MAX_PROJECTILES);
        most = most.max(run.projectiles.len());
    }
    assert!(most > 200, "the crowd fired: {most}");
}

#[test]
fn a_boss_file_names_patterns_and_chains_phases() {
    let art = "art\nkkkk\nkkkk\n";
    let ok = format!(
        "name Test\nwhere crypt\nattack gap_ring every=90 damage=12\nattack laser every=120 shot=eye\nphases flower sweep rain twin_spiral every=40 volleys=2\n{art}"
    );
    let (boss, _) = bosses::check("test", &ok).unwrap();
    assert_eq!(
        boss.attacks[0].pattern,
        bosses::Pattern::Named(Emitter::GapRing)
    );
    assert_eq!(boss.attacks[0].bolt.shot(Pack::Crypt), Shot::Wisp);
    assert_eq!(boss.attacks[1].bolt.shot(Pack::Crypt), Shot::Eye);
    let phases = boss.phases.clone().unwrap();
    assert_eq!(
        phases.chain,
        vec![
            Emitter::Flower,
            Emitter::Sweep,
            Emitter::Rain,
            Emitter::TwinSpiral
        ]
    );
    assert_eq!(phases.volleys, 2);
    for bad in [
        "attack wobble every=90",
        "attack ring shot=glitter",
        "phases flower nonsense",
        "phases flower",
        "phases flower sweep rain twin_spiral laser",
        "phases flower sweep\nphases rain laser",
    ] {
        let raw = format!("name Test\nwhere crypt\nattack ring\n{bad}\n{art}");
        assert!(bosses::check("test", &raw).is_err(), "{bad}");
    }
    // The rules name every pattern.
    let rules = bosses::rules();
    for emitter in Emitter::ALL {
        assert!(rules.contains(emitter.name()), "{emitter:?}");
    }
}

#[test]
fn a_boss_walks_its_phases_in_turn() {
    let art = "art\nkkkk\nkkkk\n";
    let raw = format!(
        "name Phaser\nwhere crypt\nhp 900\nmove anchor\nattack seeker every=240\nphases gap_ring laser every=30 volleys=1 shot=wisp from=open\n{art}"
    );
    let (boss, _) = bosses::check("phaser", &raw).unwrap();
    let mut run = quiet();
    run.bosses.push(boss);
    let index = run.bosses.len() - 1;
    run.spawn_boss_at_for_test(index, 24.0, 8.0);
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    let (mut rings, mut beams) = (false, false);
    for _ in 0..200 {
        run.step(&BTreeMap::new());
        let shots = &run.projectiles;
        // A guardian's gap ring is a guardian's size (`guardian_count`).
        rings |= shots
            .iter()
            .filter(|p| p.kind == Shot::Wisp && p.traits.path.beam.is_none())
            .count()
            >= 6;
        beams |= shots.iter().any(|p| p.traits.path.beam.is_some());
    }
    assert!(rings && beams, "rings {rings} beams {beams}");
}

#[test]
fn the_shipped_bosses_chain_phases() {
    for &(id, raw) in bosses::BUILTIN {
        let (boss, _) = bosses::check(id, raw).unwrap();
        assert!(boss.phases.is_some(), "{id}");
    }
    let mut run = quiet();
    for pack in [
        Pack::Crypt,
        Pack::Cavern,
        Pack::Hellforge,
        Pack::Archive,
        Pack::Fungal,
        Pack::Unknown,
    ] {
        for index in run.pack_bosses_for_test(pack) {
            let boss = &run.bosses[index];
            assert!(boss.phases.is_some(), "{}", boss.id);
        }
    }
}

#[test]
fn a_flower_rotates_by_the_same_step_each_volley() {
    let run = quiet();
    let mut last = None;
    for count in 0..12 {
        let shots = volley(&run, Emitter::Flower, (24.0, 8.0), (24.0, 20.0), count, 0);
        let angle = shots[0].vy.atan2(shots[0].vx);
        if let Some(before) = last {
            let delta: f32 = angle - before;
            assert!((delta.sin().atan2(delta.cos()) - FLOWER_TURN).abs() < 1e-5);
        }
        last = Some(angle);
    }
}

#[test]
fn every_emitter_respects_the_remaining_shot_capacity() {
    let run = quiet();
    let grid = Grid {
        room: run.room(),
        barred: false,
    };
    let aim = Aim {
        from: (24.0, 8.0),
        at: (24.0, 20.0),
        pace: (0.0, 0.0),
        grid: &grid,
        count: 0,
        salt: 5,
        step: MOST_STEP,
        kind: Shot::Eye,
        damage: 12,
        guardian: None,
    };
    let filler = volley(&run, Emitter::Burst, aim.from, aim.at, 0, 0)[0].clone();
    for emitter in Emitter::ALL {
        let mut shots = vec![filler.clone(); MAX_PROJECTILES - 1];
        emit(emitter, &aim, &mut shots);
        assert_eq!(shots.len(), MAX_PROJECTILES, "{emitter:?}");
        let full = print(&shots);
        emit(emitter, &aim, &mut shots);
        assert_eq!(print(&shots), full, "full capacity: {emitter:?}");
    }
}

#[test]
fn a_beam_burns_for_exactly_its_duration_and_strikes_each_knight_once() {
    let run = quiet();
    let grid = Grid {
        room: run.room(),
        barred: false,
    };
    for emitter in [Emitter::Laser, Emitter::Sweep] {
        let mut shot = volley(&run, emitter, (24.0, 8.0), (24.0, 20.0), 0, 0).remove(0);
        let beam = shot.traits.path.beam.unwrap();
        let mut players = run.players.clone();
        let start_hp = players[&1].hp;
        let mut sounds = Vec::new();
        for _ in 0..beam.warn {
            shine(&mut shot, &mut players, &grid, &mut sounds, &mut feel::Feel::default());
            assert_eq!(players[&1].hp, start_hp);
        }
        for tick in 0..beam.burn {
            let hero = players.get_mut(&1).unwrap();
            let a = beam.angle + beam.sweep * f32::from(tick);
            (hero.x, hero.y) = (shot.x + a.cos() * 6.0, shot.y + a.sin() * 6.0);
            hero.invulnerable = 0;
            shine(&mut shot, &mut players, &grid, &mut sounds, &mut feel::Feel::default());
            assert!(shot.traits.path.beam.unwrap().burning(shot.traits.path.age));
        }
        assert_eq!(sounds.iter().filter(|&&s| s == "hero_hurt").count(), 1);
        // A fresh knight on the line after the burn takes nothing.
        let mut fresh = players[&1].clone();
        fresh.invulnerable = 0;
        let fresh_hp = fresh.hp;
        players.insert(2, fresh);
        shine(&mut shot, &mut players, &grid, &mut sounds, &mut feel::Feel::default());
        assert_eq!(shot.ttl, 0);
        assert_eq!(players[&2].hp, fresh_hp);
        assert!(!shot.traits.path.beam.unwrap().burning(shot.traits.path.age));
    }
}

#[test]
fn ore_bounces_once_off_room_stone() {
    let mut run = quiet();
    run.step(&BTreeMap::new());
    run.projectiles = volley(&run, Emitter::Bounce, (44.0, 14.0), (48.0, 14.0), 0, 0);
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.projectiles.len(), 3);
    assert!(
        run.projectiles
            .iter()
            .all(|p| p.vx < 0.0 && p.traits.bounce == 0)
    );
    // Send them back into the same wall; the spent bounce does not renew.
    for shot in &mut run.projectiles {
        shot.vx = shot.vx.abs();
    }
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    assert!(run.projectiles.is_empty());
}

#[test]
fn weaving_curving_and_accelerating_shots_move_as_defined() {
    let run = quiet();
    let mut weave = volley(&run, Emitter::Weave, (24.0, 8.0), (24.0, 20.0), 0, 0);
    let mut xs = Vec::new();
    for _ in 0..60 {
        let before = (weave[0].x, weave[0].y);
        fly(&mut weave, (24.0, 20.0), 1);
        xs.push(weave[0].x);
        let speed = (weave[0].x - before.0).hypot(weave[0].y - before.1) / DT;
        assert!(speed <= 10.0, "weave's actual pace: {speed}");
    }
    assert!(xs.iter().any(|&x| x < 23.0) && xs.iter().any(|&x| x > 25.0));
    let mut curve = volley(&run, Emitter::Curve, (24.0, 8.0), (24.0, 20.0), 0, 0);
    let before = curve[0].vy.atan2(curve[0].vx);
    fly(&mut curve, (24.0, 20.0), 30);
    assert!((curve[0].vy.atan2(curve[0].vx) - before - CURVE_TURN).abs() < 1e-4);
    let mut surge = volley(&run, Emitter::Surge, (24.0, 8.0), (24.0, 20.0), 0, 0);
    fly(&mut surge, (24.0, 20.0), 60);
    assert!(
        surge
            .iter()
            .all(|p| (p.vx.hypot(p.vy) - SURGE_TOP).abs() < 1e-4)
    );
}

#[test]
fn seekers_turn_gently_and_expire() {
    let mut run = quiet();
    run.step(&BTreeMap::new());
    run.projectiles = volley(&run, Emitter::Seeker, (24.0, 8.0), (24.0, 20.0), 0, 0);
    let mut shot = run.projectiles[0].clone();
    let before = shot.vy.atan2(shot.vx);
    steer(&mut shot, &[(1, 40.0, 8.0)], &mut Vec::new());
    let delta = shot.vy.atan2(shot.vx) - before;
    assert!(delta < 0.0 && delta.abs() <= SEEKER_TURN * DT + 1e-5);
    for _ in 0..SEEKER_LIFE {
        run.step(&BTreeMap::new());
    }
    assert!(run.projectiles.is_empty());
}

#[test]
fn boss_phases_keep_their_order_through_rage_and_serialization() {
    let raw = "name Phaser\nwhere crypt\nhp 900\nmove anchor\nrage 40%\nphases burst laser split every=60 volleys=2\nart\nkkkk\n";
    let (boss, _) = bosses::check("phaser", raw).unwrap();
    let mut run = quiet();
    run.bosses.push(boss);
    run.spawn_boss_at_for_test(run.bosses.len() - 1, 24.0, 8.0);
    let grid = Grid {
        room: run.room(),
        barred: false,
    };
    let sight = Sight {
        knights: &[(24.0, 20.0)],
        paces: &[(0.0, 0.0)],
        pack: Pack::Crypt,
        depth: 1,
        heads: 1,
    };
    let mut enemy = run.enemies[0].clone();
    let mut seen = Vec::new();
    for tick in 30..600 {
        enemy.age = tick;
        if seen.len() == 3 {
            enemy.hp = enemy.max_hp / 3;
        }
        let mut shots = Vec::new();
        patterns::volley(&mut enemy, &sight, &grid, run.seed, &run.bosses, &mut shots);
        if !shots.is_empty() {
            let e = if shots[0].traits.path.beam.is_some() {
                Emitter::Laser
            } else if shots[0].traits.path.bursts() {
                Emitter::Split
            } else {
                Emitter::Burst
            };
            seen.push(e);
            let restored: Enemy =
                serde_json::from_str(&serde_json::to_string(&enemy).unwrap()).unwrap();
            assert_eq!(restored.pattern_volleys, enemy.pattern_volleys);
            enemy = restored;
        }
        if seen.len() == 8 {
            break;
        }
    }
    assert_eq!(
        seen,
        vec![
            Emitter::Burst,
            Emitter::Burst,
            Emitter::Laser,
            Emitter::Laser,
            Emitter::Split,
            Emitter::Split,
            Emitter::Burst,
            Emitter::Burst
        ]
    );
}

#[test]
fn old_shots_deserialize_with_an_empty_path() {
    let run = quiet();
    let shot = volley(&run, Emitter::Burst, (24.0, 8.0), (24.0, 20.0), 0, 0).remove(0);
    let mut raw = serde_json::to_value(&shot).unwrap();
    raw["traits"].as_object_mut().unwrap().remove("path");
    let restored: Projectile = serde_json::from_value(raw).unwrap();
    assert_eq!(restored.traits.path, Path::default());
}

/// `quiet`, its floor open but for one tomb (a `Tile::Block`) at tile
/// (12, 5): x 24 to 26, y 10 to 12. The knight stands well away.
fn tomb_room() -> Run {
    let mut run = open_room();
    let at = run.at;
    run.dungeon.rooms[at].set_for_test(12, 5, Tile::Block);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (40.0, 24.0);
    run
}

/// `quiet` with nothing on its floor, settled.
fn open_room() -> Run {
    let mut run = quiet();
    let at = run.at;
    for row in 1..run.room().rows - 1 {
        for col in 1..run.room().cols - 1 {
            run.dungeon.rooms[at].set_for_test(col, row, Tile::Floor);
        }
    }
    settle(&mut run);
    run
}

/// A few empty ticks, so the knight's health and the room's phase are what
/// a fight keeps (`quiet`'s great health is cut to the knight's own).
fn settle(run: &mut Run) {
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    run.projectiles.clear();
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
}

/// A plain shot flying east from `(x, y)` at `speed`, a monster's or a
/// knight's arrow.
fn east(x: f32, y: f32, speed: f32, hostile: bool) -> Projectile {
    Projectile {
        x,
        y,
        vx: speed,
        vy: 0.0,
        hostile,
        kind: if hostile { Shot::Wisp } else { Shot::Arrow },
        look: None,
        damage: 10,
        pierce: 0,
        last_hit: None,
        ttl: 4 * HZ,
        empowered: false,
        traits: ShotTraits::default(),
    }
}

#[test]
fn darts_are_the_fastest_and_thinnest_shot() {
    let run = quiet();
    let darts = volley(&run, Emitter::Darts, (24.0, 8.0), (24.0, 20.0), 0, 0);
    assert_eq!(darts.len() as u32, DARTS.0);
    for (i, d) in darts.iter().enumerate() {
        assert_eq!(d.traits.path.form, Form::Dart);
        assert_eq!(family(d), Family::Dart);
        assert!((d.vx.hypot(d.vy) - SHOT_TOP).abs() < 1e-4);
        // One after another down the line.
        assert_eq!(u32::from(d.traits.path.wait), i as u32 * u32::from(DART_GAP));
    }
    assert!(girth(&darts[0]) < STRIKE);
    // Past a knight by a body and a plain shot's strike: a plain shot hits,
    // a dart goes by.
    let mut run = open_room();
    let (hx, hy) = (run.players[&1].x, run.players[&1].y);
    let by = hy - (HERO_RADIUS + STRIKE - 0.02);
    let mut plain = east(hx - 6.0, by, SHOT_TOP, true);
    let mut dart = plain.clone();
    dart.traits.path.form = Form::Dart;
    plain.damage = 7;
    dart.damage = 5;
    for (shot, hit) in [(plain, true), (dart, false)] {
        let hp = run.players[&1].hp;
        run.projectiles = vec![shot];
        for _ in 0..30 {
            run.step(&BTreeMap::new());
        }
        assert_eq!(run.players[&1].hp < hp, hit);
        run.players.get_mut(&1).unwrap().invulnerable = 0;
    }
}

#[test]
fn heavy_orbs_are_slow_and_wide_and_can_be_walked_round() {
    let run = quiet();
    let orbs = volley(&run, Emitter::Orbs, (24.0, 8.0), (24.0, 20.0), 0, 2);
    assert_eq!(orbs.len() as u32, ORBS.0 + 2 * ORBS.1);
    for o in &orbs {
        assert_eq!(family(o), Family::Orb);
        assert!(o.vx.hypot(o.vy) < 3.0, "a stroll, slower than any other shot");
        assert!(girth(o) > STRIKE && body(o) > BODY);
    }
    // It strikes wider than a plain shot, and a knight a step aside is
    // clear of it.
    let mut run = open_room();
    let (hx, hy) = (run.players[&1].x, run.players[&1].y);
    let mut near = east(hx - 4.0, hy - (HERO_RADIUS + ORB_STRIKE - 0.05), ORBS.2, true);
    near.traits.path.form = Form::Orb;
    let mut far = near.clone();
    far.y = hy - (HERO_RADIUS + ORB_STRIKE + 0.3);
    for (shot, hit) in [(far, false), (near, true)] {
        let hp = run.players[&1].hp;
        run.projectiles = vec![shot];
        for _ in 0..3 * HZ {
            run.step(&BTreeMap::new());
        }
        assert_eq!(run.players[&1].hp < hp, hit);
        run.players.get_mut(&1).unwrap().invulnerable = 0;
    }
}

#[test]
fn a_lob_flies_over_everything_and_lands_on_its_mark() {
    let mut run = tomb_room();
    run.join_seat(2, "Under");
    // The thrower west of the tomb, the knight east of it; a friend stands
    // under the arc.
    let from = (22.5, 11.0);
    let target = (34.0, 11.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = target;
    let under = run.players.get_mut(&2).unwrap();
    (under.x, under.y) = (28.5, 11.4);
    settle(&mut run);
    let grid = Grid {
        room: &run.dungeon.rooms[run.at],
        barred: false,
    };
    let aim = Aim {
        from,
        at: target,
        pace: (0.0, 0.0),
        grid: &grid,
        count: 0,
        salt: 1,
        step: 0,
        kind: Shot::Ore,
        damage: 12,
        guardian: None,
    };
    let mut shots = Vec::new();
    emit(Emitter::Lob, &aim, &mut shots);
    assert_eq!(shots.len(), 1);
    let lob = shots[0].clone();
    assert_eq!(family(&lob), Family::Lob);
    // Its mark is the knight's spot from the first tick, and it rises.
    let ((lx, ly), rise) = lob_flight(&lob).unwrap();
    assert!((lx - target.0).abs() < 1e-3 && (ly - target.1).abs() < 1e-3);
    assert_eq!(rise, 0.0);
    assert!((lob.vx.hypot(lob.vy) * DT * f32::from(LOB_FLIGHT) - (target.0 - from.0)).abs() < 1e-3);
    assert!(lob.vx.hypot(lob.vy) <= SHOT_TOP);
    run.projectiles = vec![lob];
    let (hp, under_hp) = (run.players[&1].hp, run.players[&2].hp);
    let mut high = 0.0f32;
    for t in 1..=u32::from(LOB_FLIGHT) {
        if let Some(p) = run.projectiles.first() {
            high = high.max(lob_flight(p).unwrap().1);
        }
        // Over the tomb, the friend and all: nothing until it lands.
        run.step(&BTreeMap::new());
        if t < u32::from(LOB_FLIGHT) {
            assert_eq!(run.players[&1].hp, hp, "tick {t}");
            assert_eq!(run.projectiles.len(), 1, "over the tomb at tick {t}");
        }
    }
    assert!(high > 2.0, "it arcs: {high}");
    assert_eq!(run.players[&2].hp, under_hp);
    assert!(run.players[&1].hp < hp, "it lands on the knight's spot");
    assert!(run.projectiles.is_empty());
    assert!(run.feel.impacts.iter().any(|i| (i.x - target.0).abs() < 0.5));
}

#[test]
fn drift_slows_in_flight_and_surge_quickens() {
    let run = quiet();
    let mut drift = volley(&run, Emitter::Drift, (24.0, 8.0), (24.0, 20.0), 0, 0);
    assert_eq!(family(&drift[0]), Family::Brake);
    assert!((drift[0].vx.hypot(drift[0].vy) - DRIFT.2).abs() < 1e-4);
    let mut speeds = Vec::new();
    for _ in 0..HZ {
        fly(&mut drift, (24.0, 20.0), 1);
        speeds.push(drift[0].vx.hypot(drift[0].vy));
    }
    assert!(speeds.windows(2).all(|w| w[1] <= w[0] + 1e-5), "it only slows");
    assert!((speeds[speeds.len() - 1] - DRIFT_TOP).abs() < 1e-4, "it creeps on at the last");
    let mut surge = volley(&run, Emitter::Surge, (24.0, 8.0), (24.0, 20.0), 0, 0);
    assert_eq!(family(&surge[0]), Family::Quicken);
    let first = surge[0].vx.hypot(surge[0].vy);
    fly(&mut surge, (24.0, 20.0), HZ / 2);
    let half = surge[0].vx.hypot(surge[0].vy);
    assert!(first < 2.0 && half > first + 2.0, "{first} then {half}");
}

#[test]
fn a_tomb_stops_a_monsters_shot_and_a_knights_where_their_bodies_touch_it() {
    for hostile in [true, false] {
        // Grazing the tomb's south face: its middle passes, its body does not.
        let mut run = tomb_room();
        run.projectiles = vec![east(18.0, 12.15, 8.0, hostile)];
        for _ in 0..HZ {
            run.step(&BTreeMap::new());
            if run.projectiles.is_empty() {
                break;
            }
        }
        assert!(run.projectiles.is_empty(), "hostile {hostile}: stopped");
        let spark = run.feel.impacts.iter().find(|i| (i.y - 12.15).abs() < 0.1);
        let stuck = run.feel.stuck.iter().find(|s| (s.y - 12.15).abs() < 0.1);
        let at = spark.map(|i| i.x).or(stuck.map(|s| s.x)).expect("a spark or a stuck arrow");
        assert!((23.0..24.5).contains(&at), "hostile {hostile}: stops at the tomb's face, {at}");
        // Half a unit clear of it, the same shot flies on by.
        let mut run = tomb_room();
        run.projectiles = vec![east(18.0, 12.6, 8.0, hostile)];
        for _ in 0..2 * HZ {
            run.step(&BTreeMap::new());
        }
        assert!(run.projectiles.iter().any(|p| p.x > 27.0), "hostile {hostile}: by");
    }
    // Water and pits stay shot-through.
    let mut run = tomb_room();
    let at = run.at;
    run.dungeon.rooms[at].set_for_test(12, 5, Tile::Hazard);
    run.projectiles = vec![east(18.0, 11.0, 8.0, true)];
    for _ in 0..2 * HZ {
        run.step(&BTreeMap::new());
    }
    assert!(run.projectiles.iter().any(|p| p.x > 27.0));
}

#[test]
fn a_knights_predicted_shot_stops_at_the_same_tomb_as_the_servers() {
    for y in [11.0, 12.1, 12.6] {
        let mut run = tomb_room();
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (16.0, y);
        let mut arrow = east(16.5, y, 24.0, false);
        arrow.traits.owner = 1;
        run.projectiles = vec![arrow.clone()];
        let mut server = None;
        let mut path = Vec::new();
        for t in 1..=40u32 {
            run.step(&BTreeMap::new());
            match run.projectiles.first() {
                Some(p) => path.push((t, p.x)),
                None => {
                    server = Some(t);
                    break;
                }
            }
        }
        let predicted = (1..=40u32).find(|&t| run.flown(1, &arrow, (arrow.x, arrow.y), t as f32).is_none());
        if y < 12.5 {
            let (s, p) = (server.expect("the server's stops"), predicted.expect("the ghost stops"));
            assert!(s == p || s == p + 1, "y {y}: server gone at {s}, ghost at {p}");
            for t in 1..p {
                let (x, _) = run.flown(1, &arrow, (arrow.x, arrow.y), t as f32).unwrap();
                assert!(x < 24.0, "y {y}: the ghost never flies into the tomb ({x} at {t})");
            }
        } else {
            assert!(path.iter().any(|&(_, x)| x > 27.0), "y {y}: the server's flies by");
            let past = run.flown(1, &arrow, (arrow.x, arrow.y), 20.0).expect("so does the ghost");
            assert!(past.0 > 27.0);
        }
    }
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_pattern_shots -- --ignored`:
/// one picture of each pattern mid-volley, and a boss walking its phases.
#[test]
#[ignore]
fn write_pattern_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let gallery = || {
        let mut run = quiet();
        let room = &mut run.dungeon.rooms[run.at];
        // An open exhibition floor keeps each whole shape visible.
        for row in 1..room.rows - 1 {
            for col in 1..room.cols - 1 {
                room.set_for_test(col, row, Tile::Floor);
            }
        }
        run
    };
    let save = |run: &Run, name: &str| {
        let mut view = run.clone();
        for enemy in &mut view.enemies {
            enemy.frozen = 0;
        }
        let img = crate::render::overworld::arena::frame(&view, 768, 448);
        let pic = image::RgbImage::from_raw(img.w as u32, img.h as u32, img.rgb_bytes()).unwrap();
        pic.save(dir.join(format!("{name}.png"))).unwrap();
    };
    // Who throws it, on which floor, in which look, and how many ticks in
    // the picture is taken.
    let cases: [(Emitter, EnemyKind, Pack, Shot, u32); 18] = [
        (
            Emitter::Burst,
            EnemyKind::Skeleton,
            Pack::Crypt,
            Shot::Bone,
            26,
        ),
        (Emitter::Lead, EnemyKind::Imp, Pack::Cavern, Shot::Ore, 26),
        (
            Emitter::GapRing,
            EnemyKind::Wraith,
            Pack::Crypt,
            Shot::Wisp,
            40,
        ),
        (
            Emitter::Flower,
            EnemyKind::Wraith,
            Pack::Crypt,
            Shot::Wisp,
            40,
        ),
        (
            Emitter::TwinSpiral,
            EnemyKind::Demon,
            Pack::Hellforge,
            Shot::Flame,
            30,
        ),
        (
            Emitter::Weave,
            EnemyKind::Wraith,
            Pack::Archive,
            Shot::Page,
            40,
        ),
        (
            Emitter::Curve,
            EnemyKind::Skeleton,
            Pack::Archive,
            Shot::Page,
            30,
        ),
        (
            Emitter::Surge,
            EnemyKind::Imp,
            Pack::Hellforge,
            Shot::Flame,
            30,
        ),
        (
            Emitter::Reaim,
            EnemyKind::Wraith,
            Pack::Hellforge,
            Shot::Flame,
            54,
        ),
        (
            Emitter::Split,
            EnemyKind::Imp,
            Pack::Fungal,
            Shot::Spore,
            44,
        ),
        (
            Emitter::Bounce,
            EnemyKind::Skeleton,
            Pack::Cavern,
            Shot::Ore,
            44,
        ),
        (
            Emitter::Laser,
            EnemyKind::Wraith,
            Pack::Unknown,
            Shot::Eye,
            25,
        ),
        (
            Emitter::Sweep,
            EnemyKind::Hollow,
            Pack::Unknown,
            Shot::Eye,
            34,
        ),
        (
            Emitter::Mines,
            EnemyKind::Silkmother,
            Pack::Fungal,
            Shot::Spore,
            40,
        ),
        (
            Emitter::GapWall,
            EnemyKind::Pavise,
            Pack::Hellforge,
            Shot::Flame,
            40,
        ),
        (
            Emitter::Crossfire,
            EnemyKind::Shaman,
            Pack::Fungal,
            Shot::Spore,
            70,
        ),
        (
            Emitter::Rain,
            EnemyKind::Dragon,
            Pack::Hellforge,
            Shot::Flame,
            60,
        ),
        (
            Emitter::Seeker,
            EnemyKind::Necromancer,
            Pack::Archive,
            Shot::Page,
            40,
        ),
    ];
    for (emitter, kind, pack, look, ticks) in cases {
        let mut run = gallery();
        run.dungeon.pack = pack;
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        let (from, at) = if emitter == Emitter::Bounce {
            ((44.0, 14.0), (48.0, 14.0))
        } else {
            ((24.0, 7.0), (24.0, 20.0))
        };
        run.spawn_at_for_test(kind, from.0, from.1);
        {
            // Only the exhibited volley moves; the caster poses for it.
            let e = run.enemies.last_mut().unwrap();
            e.hp = 100_000;
            e.max_hp = 100_000;
            e.seen_hp = 100_000;
            e.age = TELEGRAPH + 1;
            e.frozen = 100_000;
        }
        let grid_shots = {
            let grid = Grid {
                room: &run.dungeon.rooms[run.at],
                barred: false,
            };
            let aim = Aim {
                from,
                at,
                pace: (8.0, 0.0),
                grid: &grid,
                count: 1,
                salt: 0x5eed,
                step: 1,
                kind: look,
                damage: 12,
                guardian: None,
            };
            let mut shots = Vec::new();
            emit(emitter, &aim, &mut shots);
            shots
        };
        run.projectiles = grid_shots;
        let name = emitter.name();
        for t in 0..ticks {
            run.step(&BTreeMap::new());
            // The knight runs right, so a lead can be seen to lead.
            let hero = run.players.get_mut(&1).unwrap();
            (hero.x, hero.y) = (24.0 + t as f32 * 0.1, 20.0);
            if emitter == Emitter::Reaim && t == 32 {
                save(&run, "reaim_hang");
            }
            if matches!(
                emitter,
                Emitter::Laser | Emitter::Sweep | Emitter::Rain | Emitter::Crossfire
            ) && t == 12
            {
                save(&run, &format!("{name}_warn"));
            }
        }
        save(&run, name);
    }
    // A boss walking four phases: a ring with a gap, a sweep, rain, twin
    // spirals.
    let raw = "name THE PHASE KEEPER\nwhere hellforge\nhp 900\nsize 1.6\nmove anchor\nattack seeker every=240\nphases gap_ring sweep rain twin_spiral every=45 volleys=1\nart\n..7777..\n.777777.\n77677677\n77777777\n.777777.\n..7..7..\n";
    let (boss, _) = bosses::check("phase-keeper", raw).unwrap();
    let mut run = gallery();
    run.dungeon.pack = Pack::Hellforge;
    run.bosses.push(boss);
    let index = run.bosses.len() - 1;
    run.spawn_boss_at_for_test(index, 24.0, 7.0);
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    // Each phase's picture, part way into its volley.
    let every = run.bosses[index].phases.as_ref().unwrap().every;
    let chain = run.bosses[index].phases.as_ref().unwrap().chain.clone();
    let mut fired: Vec<(u32, Emitter)> = Vec::new();
    for _ in 0..6 * every {
        let before = run.enemies[0].pattern_volleys;
        run.step(&BTreeMap::new());
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (24.0, 20.0);
        if run.enemies[0].pattern_volleys != before {
            let phase = before as usize % chain.len();
            fired.push((run.enemies[0].age, chain[phase]));
        }
        let now = run.enemies[0].age;
        if let Some(&(at, emitter)) = fired.iter().find(|(at, _)| now == at + 26)
            && fired.len() <= chain.len()
        {
            let k = fired.iter().position(|f| f.0 == at).unwrap();
            save(&run, &format!("boss_phase_{k}_{}", emitter.name()));
        }
    }
}


/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_mixed_fight_shots -- --ignored`:
/// a crypt fight's shots side by side, a few ticks apart: a skeleton's
/// darts, a necromancer's heavy orb, an imp's lob over a tomb with its
/// shadow and landing ring, drift slowing, and a wisp breaking on a tomb.
#[test]
#[ignore]
fn write_mixed_fight_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = tomb_room();
    run.dungeon.pack = Pack::Crypt;
    let at = run.at;
    for (c, r) in [(6, 9), (17, 3), (18, 9)] {
        run.dungeon.rooms[at].set_for_test(c, r, Tile::Block);
    }
    let knight = (34.0, 15.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = knight;
    hero.hp = 100_000;
    hero.max_hp = 100_000;
    hero.invulnerable = 1_000_000;
    for (kind, x, y) in [
        (EnemyKind::Skeleton, 8.0, 24.0),
        (EnemyKind::Necromancer, 40.0, 5.0),
        (EnemyKind::Imp, 21.0, 9.0),
        (EnemyKind::Demon, 10.0, 6.0),
    ] {
        run.spawn_at_for_test(kind, x, y);
    }
    for e in &mut run.enemies {
        e.frozen = 1_000_000;
    }
    run.phase = Phase::Fighting;
    let throw = |run: &Run, emitter: Emitter, kind: Shot, from: (f32, f32), to: (f32, f32)| {
        let grid = Grid {
            room: &run.dungeon.rooms[run.at],
            barred: false,
        };
        let aim = Aim {
            from,
            at: to,
            pace: (0.0, 0.0),
            grid: &grid,
            count: 1,
            salt: 9,
            step: 0,
            kind,
            damage: 10,
            guardian: None,
        };
        let mut shots = Vec::new();
        emit(emitter, &aim, &mut shots);
        shots
    };
    let mut shots = Vec::new();
    shots.extend(throw(&run, Emitter::Darts, Shot::Bone, (8.0, 24.0), knight));
    shots.extend(throw(&run, Emitter::Darts, Shot::Bone, (8.0, 24.0), (14.0, 17.0)));
    shots.extend(throw(&run, Emitter::Orbs, Shot::Wisp, (40.0, 5.0), knight));
    shots.extend(throw(&run, Emitter::Lob, Shot::Ember, (21.0, 9.0), knight));
    shots.extend(throw(&run, Emitter::Drift, Shot::Ember, (10.0, 6.0), knight));
    // A wisp flying at the tomb's west face.
    shots.push(east(19.0, 11.0, 6.0, true));
    run.projectiles = shots;
    for t in 1..=24u32 {
        for e in &mut run.enemies {
            e.frozen = 1_000_000;
        }
        run.step(&BTreeMap::new());
        if [6, 12, 18, 24].contains(&t) {
            for (w, h) in [(384, 224), (768, 448)] {
                let img = crate::render::overworld::arena::frame(&run, w, h);
                let pic = image::RgbImage::from_raw(img.w as u32, img.h as u32, img.rgb_bytes()).unwrap();
                pic.save(dir.join(format!("mixed-t{t:02}-{w}.png"))).unwrap();
            }
        }
    }
}

#[test]
fn a_greater_demon_on_the_third_floor_fires_thin_slow_volleys_for_a_full_party() {
    // Three knights on the third floor: every other monster's volleys
    // thicken three steps; a demon's one (`DEMON_STEP`), every three
    // seconds, twelve a shot (the operator: "the demons in lvl 3 are bad").
    let mut run = quiet();
    for id in 2..=3 {
        run.join_seat(id, &format!("Knight {id}"));
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.hp, hero.max_hp) = (20.0 + id as f32 * 3.0, 20.0, 100_000, 100_000);
    }
    run.dungeon.depth = 3;
    run.spawn_at_for_test(EnemyKind::Demon, 24.0, 8.0);
    let demon = run.enemies.last_mut().unwrap();
    (demon.age, demon.elite) = (TELEGRAPH, false);
    let sig = signature(EnemyKind::Demon, Pack::Hellforge, false).unwrap();
    assert_eq!((sig.every, sig.damage), (90, 12));
    let (mut volleys, mut most) = (0, 0usize);
    for _ in 0..6 * HZ {
        let before = run.projectiles.iter().filter(|p| p.hostile).count();
        run.step(&BTreeMap::new());
        let fired = run.projectiles.iter().filter(|p| p.hostile).count().saturating_sub(before);
        if fired > 0 {
            volleys += 1;
            most = most.max(fired);
        }
    }
    assert!(volleys >= 1 && most >= 16, "it fires: {volleys} volleys, {most} at most");
    assert!(volleys <= 3, "a volley every three seconds or slower: {volleys} in six");
    assert!(most <= 32, "a twin spiral of 32 at most: {most}");
}

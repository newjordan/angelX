use super::*;

/// Floor one's entrance (a plain room) as a fight, knight one at its
/// middle, one hunter of `kind` at `(x, y)`.
fn with(kind: EnemyKind, x: f32, y: f32) -> Run {
    let mut run = Run::new(33, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.spawn_at_for_test(kind, x, y);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    hero.invulnerable = 0;
    run.phase = Phase::Fighting;
    run
}

/// Step with knight one held at full health.
fn hold(run: &mut Run) {
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = hero.max_hp;
    run.step(&BTreeMap::new());
}

fn staring(run: &Run) -> bool {
    run.enemies
        .iter()
        .any(|e| e.kind == EnemyKind::Flesher && e.stage == 1)
}

#[test]
fn the_flesher_stares_down_a_line_then_hooks_a_knight_and_reels_him_in() {
    let mut run = with(EnemyKind::Flesher, 36.0, 14.0);
    let (mut stared, mut caught) = (false, false);
    for _ in 0..6 * HZ {
        hold(&mut run);
        stared |= staring(&run);
        caught |= run.hooks.iter().any(|h| h.caught == Some(1));
        if caught && run.hooks.is_empty() {
            break;
        }
    }
    assert!(stared, "he stares first");
    assert!(caught, "the hook catches the knight");
    assert!(run.hooks.is_empty(), "and comes home");
    let flesher = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Flesher)
        .unwrap();
    let hero = &run.players[&1];
    assert!(
        (hero.x - flesher.x).hypot(hero.y - flesher.y) < 3.5,
        "reeled in to his cleaver"
    );
    assert!(run.cues.iter().any(|c| c == "hooked"));
    assert!(run.feats.contains(&"fresh_meat"));
}

#[test]
fn a_knight_who_steps_off_the_line_is_missed() {
    let mut run = with(EnemyKind::Flesher, 36.0, 14.0);
    while !staring(&run) {
        hold(&mut run);
    }
    run.players.get_mut(&1).unwrap().y += 6.0;
    for _ in 0..2 * HZ {
        hold(&mut run);
    }
    assert!(!run.cues.iter().any(|c| c == "hooked"));
}

#[test]
fn a_knight_rolling_through_the_hook_is_missed() {
    let mut run = with(EnemyKind::Flesher, 36.0, 14.0);
    while !staring(&run) {
        hold(&mut run);
    }
    for _ in 0..2 * HZ {
        run.players.get_mut(&1).unwrap().dash_ticks = 5;
        hold(&mut run);
    }
    assert!(!run.cues.iter().any(|c| c == "hooked"));
}

#[test]
fn the_rot_wears_at_whoever_stands_close_but_never_fells_them() {
    let mut run = with(EnemyKind::Flesher, 26.0, 14.0);
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 10_000;
    let start = hero.hp;
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&1].hp < start, "it wears");
    run.players.get_mut(&1).unwrap().hp = 3;
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].hp, 1, "it never fells");
}

#[test]
fn the_silkmother_spins_webs_and_hatches_a_brood_of_at_most_six() {
    let mut run = with(EnemyKind::Silkmother, 36.0, 8.0);
    for _ in 0..24 * HZ {
        hold(&mut run);
    }
    assert!(!run.webs.is_empty(), "webs under the knight");
    let brood = run
        .enemies
        .iter()
        .filter(|e| e.kind == EnemyKind::Spiderling)
        .count();
    assert!((2..=6).contains(&brood), "{brood} spiderlings");
    assert!(run.cues.iter().any(|c| c == "brood_hatch"));
}

#[test]
fn a_knight_in_a_web_is_slowed() {
    let mut run = with(EnemyKind::Bat, 46.0, 2.0);
    run.enemies.clear();
    run.phase = Phase::Exploring;
    let walk = |run: &mut Run| {
        let x = run.players[&1].x;
        let mut inputs = BTreeMap::new();
        inputs.insert(
            1,
            Input {
                move_x: 1,
                ..Default::default()
            },
        );
        for _ in 0..6 {
            run.step(&inputs);
        }
        run.players[&1].x - x
    };
    let free = walk(&mut run.clone());
    run.webs.push(hunters::Web {
        x: 24.0,
        y: 14.0,
        left: 100,
    });
    let webbed = walk(&mut run);
    assert!(webbed < free * 0.7, "{webbed} against {free}");
}

#[test]
fn the_hunters_keep_to_the_deep() {
    for depth in 1..=FLOORS {
        for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge] {
            assert!(
                pack.roster_at(depth).iter().all(|(k, _)| !matches!(
                    k,
                    EnemyKind::Flesher | EnemyKind::Silkmother | EnemyKind::Spiderling
                )),
                "{pack:?} floor {depth}"
            );
        }
    }
    let has = |pack: Pack, kind: EnemyKind| pack.roster_at(5).iter().any(|(k, _)| *k == kind);
    assert!(has(Pack::Archive, EnemyKind::Silkmother));
    assert!(has(Pack::Fungal, EnemyKind::Flesher));
    assert!(has(Pack::Unknown, EnemyKind::Flesher));
}

#[test]
fn friends_see_the_hooks_and_the_webs() {
    let mut run = with(EnemyKind::Flesher, 36.0, 14.0);
    while run.hooks.is_empty() {
        hold(&mut run);
    }
    run.webs.push(hunters::Web {
        x: 10.0,
        y: 10.0,
        left: 50,
    });
    let live = run.live();
    let mut friend: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    friend.hooks.clear();
    friend.webs.clear();
    assert!(friend.apply_live(live));
    assert_eq!((friend.hooks.len(), friend.webs.len()), (1, 1));
}

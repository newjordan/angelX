//! The circles below the hell gate: hordes led by champions, embers in the
//! fight that burn a knight as any hit does, and fewer draughts.

use super::*;
use crate::render::overworld::Img;

/// A party of four on floor `depth` of circle `circle` (0: above).
fn circle_floor(circle: u32, depth: u32) -> Run {
    crate::rules::party_scale_tests::circle_run(3, depth, 4, crate::rules::party_scale_tests::Kit::Kit, circle)
}

/// Each unguarded chamber's company: (its size, its elites).
fn companies(run: &Run) -> Vec<(usize, usize)> {
    let o = run.dungeon.open.as_ref().expect("an open floor");
    (0..o.chambers.len())
        .filter(|&i| !o.chambers[i].guarded())
        .map(|i| {
            let company: Vec<&Enemy> = run.enemies.iter().filter(|e| e.home == Some(i as u8)).collect();
            (company.len(), company.iter().filter(|e| e.elite).count())
        })
        .filter(|&(n, _)| n > 0)
        .collect()
}

#[test]
fn every_company_of_a_circle_is_a_horde_led_by_champions() {
    let above = companies(&circle_floor(0, 1));
    assert!(above.iter().all(|&(_, elites)| elites == 0), "no champions on the first floor above: {above:?}");
    let first = companies(&circle_floor(1, 1));
    assert!(first.iter().all(|&(_, elites)| elites >= 1), "a champion at the head of each: {first:?}");
    let fifth = companies(&circle_floor(5, 1));
    assert!(
        fifth.iter().all(|&(n, elites)| elites >= 2.min(n)),
        "two in the Fifth Circle: {fifth:?}"
    );
    let count = |c: &[(usize, usize)]| c.iter().map(|&(n, _)| n).sum::<usize>();
    assert!(count(&first) > count(&above), "{} against {}", count(&first), count(&above));
    assert!(count(&fifth) > count(&first), "{} against {}", count(&fifth), count(&first));
}

/// A fight room with knight 1 at its middle and one skeleton in its corner.
fn ember_room(circle: u32) -> Run {
    let mut run = Run::new(4, 1, None);
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
    run.spectacle.circle = circle;
    run.spawn_at_for_test(EnemyKind::Skeleton, 8.0, 6.0);
    run.enemies[0].frozen = 100_000;
    // A tick for the skeleton to take on its circle's strength.
    run.step_sim_for_test(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    hero.invulnerable = 0;
    run
}

fn ember(x: f32, y: f32, fall: u32) -> hazards::Rock {
    hazards::Rock { x, y, fall, dust: 0, ember: true }
}

#[test]
fn an_ember_burns_a_knight_once_a_ward_and_spares_the_monsters() {
    let mut run = ember_room(2);
    let (sx, sy) = (run.enemies[0].x, run.enemies[0].y);
    let foe_hp = run.enemies[0].hp;
    let full = run.players[&1].hp;
    // Two embers land on the knight at once, and one on the skeleton.
    run.rocks = vec![ember(24.0, 14.0, 1), ember(24.2, 14.1, 1), ember(sx, sy, 1)];
    run.step_sim_for_test(&BTreeMap::new());
    let hero = &run.players[&1];
    let lost = full - hero.hp;
    assert!(lost > 0, "the ember burns");
    assert!(lost * 100 <= hero.max_hp * crate::rules::hero::ONE_HIT_MOST, "one hit's worth at most: {lost}");
    assert!(hero.invulnerable > 0, "the ward after a hit holds");
    assert_eq!(run.enemies[0].hp, foe_hp, "hell's fire spares its own");
    // Another lands inside the ward: nothing.
    let after = hero.hp;
    run.rocks.push(ember(24.0, 14.0, 1));
    run.step_sim_for_test(&BTreeMap::new());
    assert_eq!(run.players[&1].hp, after, "inside the ward, an ember takes nothing");
}

#[test]
fn embers_fall_in_a_fight_from_the_second_circle() {
    for (circle, falls) in [(0, false), (1, false), (2, true), (5, true)] {
        let mut run = ember_room(circle);
        run.players.get_mut(&1).unwrap().invulnerable = 100_000;
        let mut seen = false;
        for _ in 0..6 * HZ {
            run.step_sim_for_test(&BTreeMap::new());
            seen |= run.rocks.iter().any(|r| r.ember);
        }
        assert_eq!(seen, falls, "circle {circle}");
    }
}

#[test]
fn hell_burns_most_of_the_draughts_the_slain_drop() {
    let mut above = Run::new(4, 1, None);
    let mut below = Run::new(4, 1, None);
    below.spectacle.circle = 3;
    let kept = |run: &mut Run| (0..400).filter(|_| run.spared_by_hell("potion")).count();
    assert_eq!(kept(&mut above), 400, "above the gate, every one");
    let k = kept(&mut below);
    assert!((120..=240).contains(&k), "about half in the Third Circle: {k} of 400");
    assert!((0..50).all(|_| below.spared_by_hell("bomb")), "what does not heal is never burnt");
}

fn save(img: &Img, dir: &std::path::Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_hell_shots -- --ignored`: embers
/// falling about a knight in the Second Circle, and one landing.
#[test]
#[ignore]
fn write_hell_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let mut run = ember_room(2);
    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
    run.rocks = vec![
        ember(21.0, 12.0, hazards::ROCK_FALL),
        ember(27.0, 15.0, hazards::ROCK_FALL * 2 / 3),
        ember(23.0, 17.0, hazards::ROCK_FALL / 4),
        ember(26.0, 11.0, 1),
    ];
    run.step_sim_for_test(&BTreeMap::new());
    for n in 0..4 {
        let mut shot = run.clone();
        shot.floor_results = None;
        save(&crate::render::overworld::arena::frame(&shot, 384, 224), &dir, &format!("hell_embers_{n}"));
        for _ in 0..4 {
            run.step_sim_for_test(&BTreeMap::new());
        }
    }
}

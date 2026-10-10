use super::*;
use crate::drive::together_shooter::{EnemyKind, Phase, RoomKind, Run};
use std::collections::BTreeMap;

#[test]
fn the_guardians_pass_the_checker_and_most_floors_have_two() {
    let bosses = builtin();
    assert_eq!(bosses.len(), 7);
    for (pack, guardians) in [
        (Pack::Crypt, 2),
        (Pack::Cavern, 2),
        (Pack::Archive, 2),
        (Pack::Fungal, 1),
    ] {
        assert_eq!(
            bosses.iter().filter(|b| b.only_in == pack).count(),
            guardians,
            "{pack:?}"
        );
    }
    for boss in &bosses {
        let width = boss.art[0].chars().count();
        assert!(
            boss.art.iter().all(|r| r.chars().count() == width),
            "{} art rows align",
            boss.id
        );
        let rate: f32 = boss.attacks.iter().map(Attack::per_second).sum();
        assert!(rate <= BULLET_BUDGET, "{} fires {rate}/s", boss.id);
    }
}

#[test]
fn a_greedy_boss_is_clamped_and_slowed_and_bad_words_rejected() {
    let raw = "name Storm\nwhere mines\nhp 99999\nattack spiral shots=4 every=6\nattack ring shots=20 every=45\nart\nHH\n";
    let (boss, notes) = check("storm", raw).unwrap();
    assert_eq!(boss.hp, 900);
    let rate: f32 = boss.attacks.iter().map(Attack::per_second).sum();
    assert!(rate <= BULLET_BUDGET + 0.5, "{rate}");
    assert!(notes.iter().any(|n| n.contains("budget")));
    let errors = check("x", "name X\nwhere moon\nattack laser\n")
        .unwrap_err()
        .errors
        .join(" | ");
    assert!(
        errors.contains("where `moon`") && errors.contains("attack `laser`"),
        "{errors}"
    );
}

#[test]
fn the_stairs_are_guarded_until_the_boss_falls_and_it_scatters_spoils() {
    let mut run = Run::new(0, 1, None);
    let stairs = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    run.enter_for_test(stairs);
    let pack = run.dungeon.pack;
    let boss = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Boss)
        .expect("a guardian");
    let def = &run.bosses[usize::from(boss.boss.unwrap())];
    assert_eq!(def.only_in, pack);
    assert!(run.barred());
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    assert!(
        run.projectiles.iter().any(|p| p.hostile),
        "the guardian attacks"
    );
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Exploring);
    let spoils = run
        .room()
        .items
        .iter()
        .filter(|i| run.book.get(&i.card).is_some_and(|c| c.is_spoil()))
        .count();
    assert!(spoils >= 3, "the guardian scatters its spoils ({spoils})");
    assert!(run.found_line().is_some_and(|l| l.ends_with("falls")));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_boss_shots -- --ignored`
#[test]
#[ignore]
fn write_boss_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (seed, name) in (0u64..12).map(|s| (s, s.to_string())) {
        let mut run = Run::new(seed, 1, Some("Squire"));
        let stairs = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
            .unwrap();
        run.enter_for_test(stairs);
        for _ in 0..140 {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
        }
        let img = crate::stage::world_viz::overworld::arena::frame(&run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(
            dir.join(format!(
                "boss_{}_{name}.ppm",
                run.dungeon.pack.name().replace(' ', "_")
            )),
            out,
        )
        .unwrap();
    }
}

#[test]
fn a_delve_meets_one_of_its_packs_guardians_and_keeps_to_it() {
    let mut met = std::collections::BTreeSet::new();
    for raid in 0..16 {
        let mut run = Run::new(3, raid, None);
        run.begin_in(Pack::Crypt);
        let stairs = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
            .unwrap();
        run.enter_for_test(stairs);
        let boss = run
            .enemies
            .iter()
            .find_map(|e| e.boss)
            .map(|i| run.bosses[usize::from(i)].id.clone())
            .expect("a guardian in the stairs room");
        // The same delve meets the same one again.
        let mut again = run.clone();
        again.enter_for_test(stairs);
        let twice = again
            .enemies
            .iter()
            .find_map(|e| e.boss)
            .map(|i| again.bosses[usize::from(i)].id.clone());
        assert_eq!(twice.as_deref(), Some(boss.as_str()));
        met.insert(boss);
    }
    assert_eq!(
        met.into_iter().collect::<Vec<_>>(),
        ["the-bone-choir", "waxen-warden"]
    );
}

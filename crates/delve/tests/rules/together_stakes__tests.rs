use super::*;
use crate::rules::stakes::{Mend, Stakes, landed};

/// A run on floor `depth`, the knight at 30 of their health, in a fight.
fn hurt_on(depth: u32) -> Run {
    let mut run = Run::new(5, 1, None);
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.cleared)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp = 30;
    run.step(&BTreeMap::new());
    run
}

#[test]
fn the_first_two_floors_mend_and_hit_as_they_always_did() {
    // Floor 1 is as ever; floor 2 starts the steady climb (`DEPTH_HIT`).
    for (depth, incoming) in [(1, 100), (2, 115)] {
        let run = hurt_on(depth);
        assert_eq!(run.players[&1].stakes, Stakes::Early);
        assert_eq!(run.toughness(), 100);
        assert_eq!(run.players[&1].incoming, incoming);
        assert_eq!(landed(40, Mend::Potion, Stakes::Early), 40);
    }
}

#[test]
fn a_draught_in_a_deep_fight_mends_half_and_a_second_draught_less() {
    let mut run = hurt_on(4);
    assert_eq!(run.players[&1].stakes, Stakes::Fight);
    let hero = run.players.get_mut(&1).unwrap();
    let first = hero.mend(40, Mend::Potion);
    assert_eq!(first, 20, "half, in a fight");
    let second = hero.mend(40, Mend::Potion);
    assert!(second < first, "{second} after {first}");
    let _ = &mut run;
    assert_eq!(landed(40, Mend::Potion, Stakes::Calm), 40, "calm: whole");
    assert!(
        landed(40, Mend::Potion, Stakes::Boss) < landed(40, Mend::Potion, Stakes::Fight),
        "less again at a guardian"
    );
}

#[test]
fn mends_that_fire_on_their_own_pass_a_budget_a_second() {
    let mut run = hurt_on(4);
    let hero = run.players.get_mut(&1).unwrap();
    let gained: u32 = (0..20).map(|_| hero.mend(10, Mend::Passive)).sum();
    assert_eq!(gained, 1, "one a second in a fight, however many hits");
    // A new second, a new budget.
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    let hero = run.players.get_mut(&1).unwrap();
    assert!(hero.mend(10, Mend::Passive) >= 1);
}

#[test]
fn a_party_wide_mend_is_cut_hard_in_a_fight() {
    assert!(landed(40, Mend::Party, Stakes::Fight) < 20);
    assert!(landed(40, Mend::Party, Stakes::Boss) <= 8);
}

#[test]
fn a_knights_health_stops_near_a_hundred_and_fifty() {
    let mut run = Run::new(5, 1, None);
    run.outfit(1, knights::knight("percival").unwrap());
    let hero = run.players.get_mut(&1).unwrap();
    hero.max_hp = 260;
    hero.hp = 260;
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].max_hp, 150);
    assert_eq!(run.players[&1].hp, 150);
    run.outfit(1, knights::knight("composer").unwrap());
    run.players.get_mut(&1).unwrap().max_hp = 400;
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].max_hp, 170, "a sturdier kit, a little more");
}

#[test]
fn mail_turns_aside_a_share_of_a_hit_not_a_fixed_piece() {
    let hit = |armor: u32, damage: u32| {
        let mut hero = Hero::new("K", 0.0, 0.0);
        hero.invulnerable = 0;
        hero.armor = armor;
        let before = hero.hp;
        hero.hurt(damage);
        before - hero.hp
    };
    // Every hit lands twice as hard (`HIT_SHARE`), then mail turns its share.
    assert_eq!(hit(0, 40), 80);
    assert_eq!(hit(1, 40), 75);
    assert_eq!(hit(2, 40), 71);
    assert!(hit(2, 100) > 80, "a heavy blow is not mostly turned aside");
    assert!(hit(2, 12) >= 4, "and a hit always stings");
}

#[test]
fn deeper_floors_make_monsters_tougher_and_hit_harder_and_a_stronger_party_tougher_still() {
    let weak = hurt_on(4);
    let mut strong = hurt_on(4);
    for hero in strong.players.values_mut() {
        hero.bonus.damage = 40;
        hero.bonus.rate = 30;
        hero.bonus.shots = 1;
    }
    assert!(strong.toughness() > weak.toughness());
    let deep = hurt_on(6);
    assert!(deep.toughness() > weak.toughness());
    assert!(deep.players[&1].incoming > weak.players[&1].incoming);
    // A steady climb down the deep floors, no jump.
    let hit = |depth: u32| hurt_on(depth).players[&1].incoming;
    assert!(hit(3) < hit(4) && hit(4) < hit(5) && hit(5) < hit(6));
}

#[test]
fn deep_rooms_have_elites_and_extra_gunners_and_the_early_floors_do_not() {
    let count = |depth: u32| {
        let (mut elites, mut foes, mut rooms) = (0usize, 0usize, 0usize);
        for seed in 0..20 {
            let mut run = Run::new(seed, 5, None);
            while run.dungeon.depth < depth {
                run.descend_for_test();
            }
            let fights: Vec<usize> = run
                .dungeon
                .rooms
                .iter()
                .enumerate()
                .filter(|(_, r)| r.kind == RoomKind::Fight && !r.cleared)
                .map(|(i, _)| i)
                .collect();
            for room in fights {
                run.enter_for_test(room);
                rooms += 1;
                foes += run.enemies.len();
                elites += run.enemies.iter().filter(|e| e.elite).count();
            }
        }
        (elites, foes * 10 / rooms.max(1))
    };
    let (early, early_foes) = count(2);
    let (deep, deep_foes) = count(5);
    assert_eq!(early, 0, "no elites on the first floors");
    assert!(deep > 0, "elites now and then deep down");
    assert!(
        deep_foes > early_foes + 20,
        "{deep_foes} against {early_foes}"
    );
}

#[test]
fn an_elite_is_larger_and_far_tougher() {
    for seed in 0..40 {
        let mut run = Run::new(seed, 5, None);
        while run.dungeon.depth < 5 {
            run.descend_for_test();
        }
        let rooms: Vec<usize> = (0..run.dungeon.rooms.len()).collect();
        for room in rooms {
            if run.dungeon.rooms[room].kind != RoomKind::Fight {
                continue;
            }
            run.enter_for_test(room);
            if let Some(elite) = run.enemies.iter().find(|e| e.elite) {
                assert!(elite.radius() > elite.kind.radius() * 1.2);
                assert!(elite.max_hp as f32 > elite.kind.hp() as f32 * 2.0);
                return;
            }
        }
    }
    panic!("no elite met in forty floors");
}

use super::*;
use crate::realm::Spoil;
use crate::rules::floor_plates::games_b_bend_cases_tests as cases;
use serde_json::json;

fn with_tallow(seed: u64, seats: u32) -> Run {
    let mut run = Run::new(seed, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.phase = Phase::Exploring;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    run.cat = Some(Cat {
        x: 25.4,
        y: 14.9,
        fetching: None,
        carrying: None,
        hiss: (seed % 3) as u32 * 2,
        hissed: 0,
        left: false,
        moving: false,
        fetched: (seed % 5) as u32 * 3,
        stuck: 0,
        petted: 0,
        pet_held: Default::default(),
    });
    run
}

fn drop_item(run: &mut Run, card: &str, x: f32, y: f32) {
    let room = run.at;
    run.dungeon.rooms[room].items.push(Item { card: card.into(), x, y, held_off: None, took: Vec::new(), dropped: 0, owner: None });
}

fn tick(run: &mut Run) {
    run.tick += 1;
    cases::call("cat.Run.tick_tallow", run, json!({}), |r| r.tick_tallow());
}

/// Hiss, fetch, carry home, follow, over varied floors, items and monsters.
#[test]
#[ignore]
fn record_cat() {
    for seed in 1..=8u64 {
        let seats = 1 + (seed % 2) as u32;
        let mut run = with_tallow(seed, seats);
        if seed % 4 == 0 {
            let room = run.at;
            for row in 5..9 {
                run.dungeon.rooms[room].set_for_test(15, row, Tile::Block);
            }
            run.cat.as_mut().unwrap().x = 28.0;
            run.cat.as_mut().unwrap().y = 13.0;
        }
        for (i, (card, x, y)) in [("gold", 33.0, 13.0), ("potion", 14.0, 8.0), ("gold", 27.0 + seed as f32, 18.0), ("gem", 40.0, 6.0)].into_iter().enumerate() {
            if (seed as usize + i) % 3 != 0 {
                drop_item(&mut run, card, x, y);
            }
        }
        for t in 0..140u32 {
            if t == 20 + seed as u32 && seed % 2 == 0 {
                run.spawn_at_for_test(EnemyKind::Skeleton, 27.0, 15.0);
                run.phase = Phase::Fighting;
            }
            if t % 50 == 49 {
                drop_item(&mut run, "gold", 20.0 + (t % 7) as f32, 12.0);
            }
            let hero = run.players.get_mut(&1).unwrap();
            if t % 40 < 30 {
                (hero.x, hero.y) = (24.0, 14.0);
            } else {
                (hero.x, hero.y) = (24.0 + (t % 40) as f32 * 0.7, 14.0);
            }
            tick(&mut run);
        }
    }
    // Petting: a press, a held key, a far knight, a knight at full health.
    for seed in 1..=6u64 {
        let mut run = with_tallow(seed, 2);
        run.players.get_mut(&1).unwrap().hp = 5;
        run.players.get_mut(&2).unwrap().hp = 3 + seed as u32;
        let cat = run.cat.as_ref().map(|c| (c.x, c.y)).unwrap();
        run.players.get_mut(&2).unwrap().x = cat.0 + if seed % 2 == 0 { 0.5 } else { 40.0 };
        run.players.get_mut(&2).unwrap().y = cat.1;
        for t in 0..16u64 {
            run.tick += 1;
            let mut keys = BTreeMap::new();
            if t % 5 < 3 {
                keys.insert(1, Input { interact: true, ..Default::default() });
            }
            if t % 4 < 2 {
                keys.insert(2, Input { interact: true, ..Default::default() });
            }
            cases::call_inputs("cat.Run.pet_tallow", &mut run, &keys, |r, k| r.pet_tallow(k));
        }
    }
    // Down the stair, and into a new room.
    for rung in 0..4u8 {
        for seed in 1..=2u64 {
            let mut home = home::Home::default();
            home.levels.insert(home::Station::Hearth, rung);
            let mut run = Run::at_home(seed, 1, None, home, Default::default());
            if seed == 2 {
                run.join_seat(2, "Two");
            }
            cases::call("cat.Run.bring_tallow", &mut run, json!({}), |r| r.bring_tallow());
            cases::call("cat.Run.tallow_follows", &mut run, json!({}), |r| r.tallow_follows());
        }
    }
    let mut run = with_tallow(3, 2);
    run.players.get_mut(&1).unwrap().stone = true;
    cases::call("cat.Run.tallow_follows", &mut run, json!({}), |r| r.tallow_follows());
    let _ = Spoil::Gold;
}

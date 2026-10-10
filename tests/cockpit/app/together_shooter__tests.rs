use super::*;

fn inputs(input: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, input)])
}

/// The first room of `kind`, preferring one screen-sized so geometry in a
/// test (walls, doors, exits) is where the screen puts it.
fn index_of(run: &Run, kind: RoomKind) -> usize {
    let rooms = &run.dungeon.rooms;
    rooms
        .iter()
        .position(|r| r.kind == kind && !r.great())
        .or_else(|| rooms.iter().position(|r| r.kind == kind))
        .unwrap_or_else(|| panic!("no {kind:?} room"))
}

/// Walk the party into the first fight room from its south side.
fn fight(run: &mut Run) {
    let room = index_of(run, RoomKind::Fight);
    run.enter_for_test(room); // combat fixture uses legacy roster/waves; ecology tested via production enter
    assert_eq!(run.phase, Phase::Fighting);
}

fn slay_all(run: &mut Run) {
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
}

fn shot(x: f32, y: f32, vx: f32, vy: f32, hostile: bool) -> Projectile {
    Projectile {
        x,
        y,
        vx,
        vy,
        hostile,
        look: None,
        kind: if hostile { Shot::Ember } else { Shot::Arrow },
        damage: if hostile { 12 } else { 32 },
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 50,
    }
}

#[test]
fn delve_time_moves_independently_of_input_and_is_reproducible() {
    let mut a = Run::new(19, 1, None);
    fight(&mut a);
    for enemy in &mut a.enemies {
        enemy.kind = EnemyKind::Skeleton;
    }
    let mut b = a.clone();
    for _ in 0..120 {
        a.step(&BTreeMap::new());
        b.step(&BTreeMap::new());
    }
    assert_eq!(a.tick, 120);
    assert!(a.projectiles.iter().any(|p| p.hostile));
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}

#[test]
fn every_raid_generates_a_new_well_formed_dungeon() {
    let layouts: std::collections::BTreeSet<String> = (0..8)
        .map(|raid| {
            let run = Run::new(7, raid, None);
            let floor = &run.dungeon;
            assert_eq!(floor.depth, 1);
            assert_eq!(
                floor
                    .rooms
                    .iter()
                    .filter(|r| r.kind != RoomKind::Secret)
                    .count(),
                10,
                "six chambers, up to three passages and a side-on hall (and a vault, some floors)"
            );
            assert_eq!(floor.rooms[0].kind, RoomKind::Start);
            for kind in [RoomKind::Stairs, RoomKind::Treasure] {
                assert_eq!(floor.rooms.iter().filter(|r| r.kind == kind).count(), 1);
            }
            for (i, room) in floor.rooms.iter().enumerate() {
                for (dir, &door) in room.doors.iter().enumerate() {
                    let next = floor.neighbour(i, dir);
                    assert_eq!(door, next.is_some(), "doors lead to rooms");
                    if let Some(next) = next {
                        // A vault's door waits on a cracked wall until a
                        // bomb finds it.
                        let crack = floor
                            .secret
                            .is_some_and(|s| s.vault == i && !s.found && s.host == next);
                        assert!(
                            crack || floor.rooms[next].doors[(dir + 2) % 4],
                            "doors pair up"
                        );
                    }
                }
                if room.kind == RoomKind::Hall {
                    assert!(
                        room.roster.iter().all(|&k| k == EnemyKind::Bat) && room.roster.len() <= 2
                    );
                    continue;
                }
                let fights = matches!(
                    room.kind,
                    RoomKind::Start
                        | RoomKind::Fight
                        | RoomKind::Stairs
                        | RoomKind::Ledge
                        | RoomKind::Pit
                );
                assert_eq!(!room.roster.is_empty(), fights);
                assert_eq!(room.cleared, !fights);
            }
            serde_json::to_string(&floor.rooms).unwrap()
        })
        .collect();
    assert!(layouts.len() >= 6, "raids differ: {}", layouts.len());
}

#[test]
fn delve_movement_is_normalized_and_stopped_by_walls() {
    let mut straight = Run::new(0, 1, None);
    let mut diagonal = straight.clone();
    let origin = (straight.players[&1].x, straight.players[&1].y);
    for _ in 0..30 {
        straight.step(&inputs(Input {
            move_x: 1,
            ..Default::default()
        }));
        diagonal.step(&inputs(Input {
            move_x: 1,
            move_y: -1,
            ..Default::default()
        }));
    }
    assert!((straight.players[&1].x - origin.0 - 13.0).abs() < 0.001);
    let h = &diagonal.players[&1];
    assert!(((h.x - origin.0).hypot(h.y - origin.1) - 13.0).abs() < 0.001);
    for _ in 0..120 {
        straight.step(&inputs(Input {
            move_x: 1,
            ..Default::default()
        }));
    }
    let x = straight.players[&1].x;
    assert!(
        x > WIDTH - 4.0 && x <= WIDTH - TILE_UNITS - HERO_RADIUS,
        "{x}"
    );
    straight.step(&inputs(Input {
        move_x: -127,
        ..Default::default()
    }));
    assert_eq!(straight.players[&1].x, x, "invalid frame is neutral");
}

#[test]
fn knights_loose_slow_heavy_single_shots() {
    let mut run = Run::new(0, 1, None);
    let start = run.players[&1].x;
    run.step(&inputs(Input {
        move_x: -1,
        aim_x: 1,
        fire: true,
        ..Default::default()
    }));
    assert!(run.players[&1].x < start);
    assert_eq!(run.projectiles.len(), 1);
    let arrow = &run.projectiles[0];
    assert_eq!((arrow.kind, arrow.vx, arrow.vy), (Shot::Arrow, 17.0, 0.0));
    assert_eq!(arrow.damage, 32);
    for _ in 0..14 {
        run.step(&inputs(Input {
            fire: true,
            ..Default::default()
        }));
    }
    assert_eq!(run.projectiles.len(), 1, "a bow draws for half a second");
    run.step(&inputs(Input {
        fire: true,
        ..Default::default()
    }));
    assert_eq!(run.projectiles.len(), 2);
}

#[test]
fn crossbow_bolts_pierce_and_walls_stop_shots() {
    let mut run = Run::new(3, 1, None);
    fight(&mut run);
    run.enemies.truncate(2);
    for (i, enemy) in run.enemies.iter_mut().enumerate() {
        (enemy.x, enemy.y, enemy.age) = (20.0 + i as f32 * 4.0, 14.0, 0);
        enemy.hp = 500;
    }
    let mut bolt = shot(16.0, 14.0, 15.0, 0.0, false);
    (bolt.kind, bolt.damage, bolt.pierce) = (Shot::Bolt, 75, 1);
    run.projectiles = vec![bolt];
    for _ in 0..30 {
        run.step(&BTreeMap::new());
        run.projectiles.retain(|p| !p.hostile);
    }
    assert!(
        run.enemies.iter().all(|e| e.hp == 500 - 75),
        "one bolt, two hits"
    );
    run.projectiles = vec![shot(WIDTH - 3.0, 14.0, 12.0, 0.0, false)];
    for _ in 0..10 {
        run.step(&BTreeMap::new());
        run.projectiles.retain(|p| !p.hostile);
    }
    assert!(run.projectiles.is_empty(), "the wall takes the arrow");
}

#[test]
fn hostile_hits_respect_grace_and_mail() {
    let mut run = Run::new(0, 1, None);
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    let (x, y) = (hero.x, hero.y);
    run.projectiles = vec![shot(x - 2.0, y, 120.0, 0.0, true); 3];
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].hp, 88, "one hit, then grace");
    assert!(run.projectiles.is_empty());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.invulnerable, hero.armor) = (0, 3);
    run.projectiles = vec![shot(x - 2.0, y, 120.0, 0.0, true)];
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].hp, 84, "mail turns aside all but the sting");
}

#[test]
fn delve_dodge_avoids_a_real_moving_projectile() {
    let mut still = Run::new(0, 1, None);
    still.players.get_mut(&1).unwrap().invulnerable = 0;
    let hero = &still.players[&1];
    still
        .projectiles
        .push(shot(hero.x, hero.y - 2.0, 0.0, 8.0, true));
    let mut dodged = still.clone();
    for _ in 0..10 {
        still.step(&BTreeMap::new());
        dodged.step(&inputs(Input {
            move_x: 1,
            ..Default::default()
        }));
    }
    assert_eq!(still.players[&1].hp, 88);
    assert_eq!(dodged.players[&1].hp, 100);
}

#[test]
fn dash_has_a_cooldown_and_bombs_are_charges() {
    let mut run = Run::new(0, 1, None);
    fight(&mut run);
    let start = run.players[&1].x;
    run.projectiles.push(shot(5.0, 15.0, 1.0, 0.0, true));
    for enemy in &mut run.enemies {
        enemy.hp = 200;
    }
    let input = inputs(Input {
        move_x: 1,
        dash: true,
        bomb: true,
        ..Default::default()
    });
    run.step(&input);
    assert!(run.projectiles.iter().all(|p| !p.hostile));
    assert_eq!(run.players[&1].bombs, 1);
    for _ in 0..5 {
        run.step(&input);
    }
    assert!(run.players[&1].x - start > 3.5, "a roll covers ground");
    assert_eq!(run.players[&1].dash_cooldown, 36 - 5);
    assert!(
        run.enemies.iter().all(|e| e.hp == 140),
        "a held key throws one bomb"
    );
    for _ in 0..2 * HZ {
        run.step(&input);
    }
    assert_eq!(
        run.players[&1].bombs, 0,
        "the second charge goes after re-arming"
    );
    run.step(&input);
    assert_eq!(run.players[&1].bombs, 0);
}

#[test]
fn barred_doors_hold_until_the_room_is_clear_then_lead_on() {
    let mut run = Run::new(5, 1, None);
    fight(&mut run);
    let here = run.at;
    assert!(run.barred());
    let dir = (0..4).find(|&d| run.room().doors[d]).unwrap();
    let (dx, dy) = DIRS[dir];
    let out = Input {
        move_x: dx as i8,
        move_y: dy as i8,
        ..Default::default()
    };
    {
        // Stand in the doorway lane and push out of the room.
        let hero = run.players.get_mut(&1).unwrap();
        hero.x = WIDTH / 2.0 + dx as f32 * (WIDTH / 2.0 - 3.0);
        hero.y = HEIGHT / 2.0 + dy as f32 * (HEIGHT / 2.0 - 3.0);
        hero.invulnerable = 9_999;
    }
    for _ in 0..30 {
        run.step(&inputs(out));
    }
    assert_eq!(run.at, here, "barred");
    let hero = &run.players[&1];
    assert!((0.0..WIDTH).contains(&hero.x) && (0.0..HEIGHT).contains(&hero.y));
    slay_all(&mut run);
    assert_eq!(run.phase, Phase::Exploring);
    assert!(run.dungeon.rooms[here].cleared);
    for _ in 0..30 {
        run.step(&inputs(out));
    }
    assert_ne!(run.at, here, "the open door leads on");
    assert!(run.room().visited);
    assert_eq!(run.dungeon.neighbour(run.at, (dir + 2) % 4), Some(here));
}

#[test]
fn monsters_drop_loot_and_knights_take_what_they_need() {
    let mut run = Run::new(2, 1, None);
    fight(&mut run);
    run.enemies.truncate(1);
    run.enemies[0].kind = EnemyKind::Demon;
    let (x, y) = (run.enemies[0].x, run.enemies[0].y);
    slay_all(&mut run);
    assert_eq!(
        run.room().items.len(),
        2,
        "a demon always drops a card and its delve's material"
    );
    let material = run.book.get(&run.room().items[1].card).unwrap();
    assert!(material.is_spoil() && material.only_in.is_none_or(|p| p == run.dungeon.pack));
    run.dungeon.rooms[run.at].items.truncate(1);
    assert_eq!((run.room().items[0].x, run.room().items[0].y), (x, y));
    let at = run.at;
    let place = |run: &mut Run, card: &str| {
        let hero = &run.players[&1];
        let (x, y) = (hero.x, hero.y);
        run.dungeon.rooms[at].items = vec![Item {
            card: card.into(),
            x,
            y,
            held_off: None,
        }];
    };
    place(&mut run, "gold");
    let score = run.score;
    run.step(&BTreeMap::new());
    assert_eq!(run.score, score + 50, "a take card is spent at once");
    place(&mut run, "potion");
    run.step(&BTreeMap::new());
    assert!(run.room().items.is_empty());
    assert_eq!(
        run.players[&1].hand,
        ["potion"],
        "a play card goes to the hand"
    );
    run.players.get_mut(&1).unwrap().hp = 50;
    run.step(&inputs(Input {
        play: 1,
        ..Default::default()
    }));
    assert_eq!(run.players[&1].hp, 90);
    assert!(run.players[&1].hand.is_empty());
    place(&mut run, "crossbow");
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].weapon, Weapon::Crossbow);
    assert_eq!(run.room().items[0].card, "bow");
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(
        run.players[&1].weapon,
        Weapon::Crossbow,
        "standing still never swaps back"
    );
    assert!(matches!(&run.found, Some((_, 1, line)) if line == "took Crossbow"));
}

#[test]
fn treasure_chests_open_into_a_prize_and_two_rolls() {
    let mut run = Run::new(4, 1, None);
    let treasure = index_of(&run, RoomKind::Treasure);
    run.enter(treasure, Some(2));
    assert_eq!(run.phase, Phase::Exploring);
    let chest = run.room().chest.unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (chest.x, chest.y + 1.5);
    run.step(&BTreeMap::new());
    assert!(run.room().chest.unwrap().open);
    let items = &run.room().items;
    assert!(items.len() >= 2, "{items:?}");
    let prize = |id: &str| run.book.get(id).is_some_and(|c| c.chest > 0);
    let taken = run.players[&1]
        .deck
        .iter()
        .chain(&run.players[&1].hand)
        .chain(&run.players[&1].arm);
    assert!(
        items.iter().any(|i| prize(&i.card))
            || taken.into_iter().any(|id| id != "bow" && prize(id))
    );
}

#[test]
fn stairs_descend_into_a_new_floor_and_revive_the_fallen() {
    let mut run = Run::new(0, 1, Some("Friend"));
    let stairs = index_of(&run, RoomKind::Stairs);
    run.enter_for_test(stairs); // this fixture exercises boss combat, not political population
    slay_all(&mut run);
    run.players.get_mut(&2).unwrap().hp = 0;
    let middle = (run.room().width() / 2.0, run.room().height() / 2.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = middle;
    let first = run.dungeon.pack;
    run.step(&BTreeMap::new());
    assert_eq!(run.floor(), 2);
    assert_ne!(run.dungeon.pack, first);
    assert_eq!(run.at, 0);
    // Ten rooms, and a Pit and a vault hung off them on some floors.
    assert_eq!(
        run.dungeon
            .rooms
            .iter()
            .filter(|r| r.kind != RoomKind::Pit && r.kind != RoomKind::Secret)
            .count(),
        10
    );
    assert!(
        run.players[&2].hp >= 50,
        "revived on the stairs, mended in the Sanctuary"
    );
    run.descend_for_test();
    assert_eq!(run.dungeon.pack, Pack::Hellforge);
    let lair = index_of(&run, RoomKind::Lair);
    assert!(run.dungeon.rooms.iter().all(|r| r.kind != RoomKind::Stairs));
    run.clear_prerequisites_for_test();
    run.enter(lair, Some(2));
    assert!(run.enemies.iter().any(|e| e.kind == EnemyKind::Dragon));
    slay_all(&mut run);
    assert_eq!(
        run.phase,
        Phase::Exploring,
        "the deep below, or the light home"
    );
    let (lx, ly) = run.light.unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (lx, ly);
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Won);
    let tick = run.tick;
    run.step(&inputs(Input {
        fire: true,
        ..Default::default()
    }));
    assert_eq!(run.tick, tick, "a finished run stands still");
    assert!(run.room().items.iter().any(|i| i.card == "heart"));
}

#[test]
fn delve_wipe_stops_simulation_but_one_living_knight_continues() {
    let mut run = Run::new(0, 1, Some("Friend"));
    run.players.get_mut(&1).unwrap().hp = 0;
    run.step(&BTreeMap::new());
    assert!(run.active());
    run.players.get_mut(&2).unwrap().hp = 0;
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
    let tick = run.tick;
    run.step(&BTreeMap::new());
    assert_eq!(run.tick, tick);
}

#[test]
fn delve_entity_work_stays_bounded_during_a_long_lair_fight() {
    let mut run = Run::new(0, 1, None);
    run.descend_for_test();
    run.descend_for_test();
    let lair = index_of(&run, RoomKind::Lair);
    run.clear_prerequisites_for_test();
    run.enter(lair, Some(2));
    for _ in 0..18_000 {
        run.players.get_mut(&1).unwrap().invulnerable = 999;
        run.step(&BTreeMap::new());
        assert!(run.projectiles.len() <= MAX_PROJECTILES);
        assert!(run.enemies.len() <= 3);
    }
    assert!(run.active());
}

#[test]
fn a_guest_joins_beside_the_host_and_leaves_without_stranding_loot() {
    let mut run = Run::new(5, 1, None);
    assert!(run.at_entrance());
    let host = (run.players[&1].x, run.players[&1].y);
    run.join("Guest\u{7}");
    let guest = &run.players[&2];
    assert_eq!(guest.name, "Guest");
    assert!((guest.x - host.0).hypot(guest.y - host.1) <= 2.5);
    run.join("Renamed");
    assert_eq!(
        (run.players.len(), run.players[&2].name.as_str()),
        (2, "Renamed")
    );

    fight(&mut run);
    assert!(!run.at_entrance());
    let kind = run.enemies[0].kind;
    assert_eq!(run.enemies[0].max_hp, kind.hp() * 2);
    run.enemies[0].hp = kind.hp();
    run.dungeon.rooms[run.at].items.push(Item {
        card: "gold".into(),
        x: 1.0,
        y: 1.0,
        held_off: Some(2),
    });
    run.leave(2);
    assert_eq!(run.players.len(), 1);
    assert_eq!(
        run.enemies[0].max_hp,
        kind.hp(),
        "monsters shrink with the party"
    );
    assert_eq!(
        run.enemies[0].hp,
        kind.hp().div_ceil(2),
        "keeping the damage share"
    );
    assert!(
        run.dungeon.rooms[run.at]
            .items
            .iter()
            .all(|item| item.held_off.is_none())
    );
    run.leave(2);
    assert_eq!(run.players.len(), 1);
}

#[test]
fn the_found_line_names_the_taker_for_three_seconds() {
    let mut run = Run::new(5, 1, Some("Ada"));
    assert!(run.found_line().is_none());
    run.found = Some((run.tick, 2, "took Potion".into()));
    assert_eq!(run.found_line().as_deref(), Some("Ada: took Potion"));
    for _ in 0..3 * HZ {
        run.step(&BTreeMap::new());
    }
    assert!(run.found_line().is_none());
}

#[test]
fn stone_host_is_inert_invulnerable_and_does_not_prevent_a_wipe() {
    let mut run = Run::new(42, 1, Some("Guest"));
    let host = run.players.get_mut(&1).unwrap();
    host.stone = true;
    host.invulnerable = 0;
    host.hurt(200);
    assert_eq!(host.hp, 100);
    let position = (host.x, host.y);
    run.step(&BTreeMap::from([(
        1,
        Input {
            move_x: 1,
            fire: true,
            dash: true,
            bomb: true,
            ..Default::default()
        },
    )]));
    assert_eq!((run.players[&1].x, run.players[&1].y), position);
    assert_eq!(run.players[&1].bombs, 2);
    assert!(run.projectiles.is_empty());
    run.players.get_mut(&2).unwrap().hp = 0;
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
}

#[test]
fn saved_run_roundtrips_and_continues_deterministically() {
    let mut run = Run::new(17, 2, Some("Guest"));
    fight(&mut run);
    run.players.get_mut(&1).unwrap().stone = true;
    for _ in 0..45 {
        run.step(&BTreeMap::new());
    }
    let encoded = serde_json::to_vec(&run).unwrap();
    let mut restored: Run = serde_json::from_slice(&encoded).unwrap();
    for _ in 0..90 {
        run.step(&BTreeMap::new());
        restored.step(&BTreeMap::new());
    }
    assert_eq!(
        serde_json::to_value(run).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
}

#[test]
fn stone_host_travels_with_party_without_taking_loot() {
    let mut run = Run::new(17, 3, Some("Guest"));
    run.players.get_mut(&1).unwrap().stone = true;
    let old = (run.players[&1].x, run.players[&1].y);
    fight(&mut run);
    assert!(run.players[&1].stone);
    assert_ne!((run.players[&1].x, run.players[&1].y), old);
    let host = &run.players[&1];
    run.drop_item("heart".into(), host.x, host.y, None);
    run.players.get_mut(&1).unwrap().hp = 50;
    run.players.get_mut(&2).unwrap().x = 35.0;
    run.gather();
    assert_eq!(run.players[&1].hp, 50);
}

#[test]
fn forged_sword_swings_once_and_bolts_keep_their_checked_look() {
    let mut run = Run::new(17, 4, None);
    fight(&mut run);
    run.enemies.truncate(1);
    let enemy = &mut run.enemies[0];
    enemy.x = 24.0;
    enemy.y = 14.0;
    enemy.hp = 100;
    enemy.max_hp = 100;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = 24.0;
    hero.y = 15.0;
    hero.forged = Some(
        crate::drive::together_forge::check(
            "name Sword\nlook steel blade\nmelee damage=34 reach=2 arc=120 every=18",
            6,
        )
        .unwrap()
        .weapon,
    );
    let input = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    run.step(&input);
    assert_eq!(run.enemies[0].hp, 66);
    assert_eq!(run.players[&1].swing, 6);
    run.step(&input);
    assert_eq!(run.enemies[0].hp, 66);
    let hero = run.players.get_mut(&1).unwrap();
    hero.forged = Some(
        crate::drive::together_forge::check(
            "look ice shard\nbolt damage=32 speed=12 every=15 range=24",
            6,
        )
        .unwrap()
        .weapon,
    );
    hero.fire_cooldown = 0;
    hero.aim_x = 1.0;
    hero.aim_y = 0.0;
    run.enemies[0].x = 38.0;
    run.step(&input);
    assert!(
        run.projectiles
            .iter()
            .any(|p| p.look == Some(("ice".into(), "shard".into())))
    );
}

#[test]
fn dungeon_opening_encounter_is_present_at_load_and_can_be_cleared() {
    for seed in 0..12 {
        let mut run = Run::new(seed, 1, None);
        assert!(run.at_entrance());
        assert_eq!(run.phase, Phase::Fighting);
        assert_eq!(run.enemies.len(), 2);
        assert!(run.barred());
        for e in &run.enemies {
            assert!((e.x - run.players[&1].x).hypot(e.y - run.players[&1].y) > 8.0);
        }
        run.join("Friend");
        assert_eq!(run.players.len(), 2);
        for e in &mut run.enemies {
            e.hp = 0;
        }
        run.step(&BTreeMap::new());
        assert!(!run.barred());
        assert!(run.room().cleared);
        assert_eq!(run.phase, Phase::Exploring);
    }
}

#[test]
fn spoils_bank_at_the_stairs_bonds_need_two_knights_and_a_wipe_keeps_half() {
    use crate::drive::together_realm::Spoil;
    let mut run = Run::new(0, 1, Some("Friend"));
    fight(&mut run);
    slay_all(&mut run);
    for hero in run.players.values() {
        assert_eq!(
            hero.carried.get(Spoil::Bond),
            1,
            "two knights cleared the room together"
        );
    }
    run.players.get_mut(&1).unwrap().carried.add(Spoil::Ore, 4);
    run.descend_for_test();
    assert!(run.players.values().all(|h| h.carried.is_empty()));
    let host = run.bank.iter().find(|h| h.hero == 1).unwrap();
    assert_eq!(
        (host.spoils.get(Spoil::Ore), host.spoils.get(Spoil::Bond)),
        (4, 1)
    );
    assert!(host.why.contains("floor 1 cleared"));
    assert_eq!(run.reclaimed.len(), 1);

    run.bank.clear();
    run.players.get_mut(&1).unwrap().carried.add(Spoil::Bone, 5);
    for hero in run.players.values_mut() {
        hero.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
    assert_eq!(
        run.bank[0].spoils.get(Spoil::Bone),
        2,
        "a fallen party keeps half"
    );
    assert!(run.bank[0].why.contains("fell"));
}

#[test]
fn solo_rooms_earn_no_bonds_and_the_dragon_gives_a_scale() {
    use crate::drive::together_realm::Spoil;
    let mut run = Run::new(0, 1, None);
    fight(&mut run);
    slay_all(&mut run);
    assert_eq!(run.players[&1].carried.get(Spoil::Bond), 0);
    run.lair_for_test();
    slay_all(&mut run);
    assert!(
        run.light.is_some(),
        "the way home opens beside the way down"
    );
    let haul = run.bank.last().unwrap();
    assert_eq!(haul.spoils.get(Spoil::Scale), 1);
    assert!(haul.why.contains("dragon"));
}

#[test]
fn monster_doors_glow_then_pour_and_the_room_holds_until_the_last_wave() {
    let mut run = Run::new(0, 1, None);
    run.descend_for_test();
    fight(&mut run);
    assert_eq!(run.waves.total, 2, "one wave a floor deep");
    // Slimes split as they fall: finish the halves too.
    for _ in 0..4 {
        if run.enemies.is_empty() {
            break;
        }
        for enemy in &mut run.enemies {
            enemy.hp = 0;
        }
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.phase, Phase::Fighting, "waves are still to come");
    assert!(run.barred());
    let mut glowed = false;
    for _ in 0..(3 * HZ) {
        run.step(&BTreeMap::new());
        glowed |= run.waves.gate.is_some();
        if !run.enemies.is_empty() {
            break;
        }
    }
    assert!(glowed, "a doorway glows before a wave");
    assert!(run.enemies.len() >= 3, "{} came through", run.enemies.len());
    let near_a_door = |e: &Enemy| {
        (0..4).filter(|&d| run.room().doors[d]).any(|d| {
            let ((dx, dy), _) = hazards::doorway(run.room(), d);
            (e.x - dx).hypot(e.y - dy) < 8.0
        })
    };
    assert!(
        run.enemies.iter().all(near_a_door),
        "they enter by a doorway"
    );
    for _ in 0..12 {
        for enemy in &mut run.enemies {
            enemy.hp = 0;
        }
        for _ in 0..(3 * HZ) {
            run.step(&BTreeMap::new());
            if !run.enemies.is_empty() || run.phase != Phase::Fighting {
                break;
            }
        }
    }
    assert_eq!(
        run.phase,
        Phase::Exploring,
        "the room opens after the last wave"
    );
    assert!(run.cues.iter().any(|c| c == "wave") || run.cues.is_empty());
}

#[test]
fn spikes_strike_on_their_beat_and_dashing_knights_pass() {
    let mut run = Run::new(1, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    run.traps = vec![Trap::Spikes {
        col: 4,
        row: 3,
        phase: 0,
    }];
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (9.5, 7.5, 0);
    let mut hurt_at = None;
    for _ in 0..hazards::SPIKE_PERIOD {
        run.players.get_mut(&1).unwrap().x = 9.5;
        run.players.get_mut(&1).unwrap().y = 7.5;
        run.step(&BTreeMap::new());
        if run.players[&1].hp < 100 && hurt_at.is_none() {
            hurt_at = Some(run.tick);
        }
    }
    let at = hurt_at.expect("the spikes struck");
    assert_eq!(hazards::spikes(at, 0), hazards::Spikes::Up);
}

#[test]
fn falling_rocks_warn_then_land_and_hurt_monsters_too() {
    let mut run = Run::new(1, 1, None);
    fight(&mut run);
    run.calm_for_test();
    let (x, y) = (run.enemies[0].x, run.enemies[0].y);
    let before = run.enemies[0].hp;
    run.rocks.push(Rock {
        x,
        y,
        fall: 3,
        dust: 0,
    });
    for _ in 0..4 {
        run.enemies[0].x = x;
        run.enemies[0].y = y;
        run.step(&BTreeMap::new());
    }
    assert!(
        run.enemies.first().is_none_or(|e| e.hp < before),
        "a landed rock crushes what is under it"
    );
}

#[test]
fn some_treasure_chests_have_teeth() {
    let mut woke = 0;
    for seed in 0..24 {
        let mut run = Run::new(seed, 1, None);
        let treasure = index_of(&run, RoomKind::Treasure);
        run.enter(treasure, Some(2));
        let chest = run.room().chest.unwrap();
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (chest.x, chest.y + 1.5);
        run.step(&BTreeMap::new());
        if run.enemies.iter().any(|e| e.kind == EnemyKind::Mimic) {
            woke += 1;
            assert_eq!(run.phase, Phase::Fighting);
            assert!(run.room().chest.is_none() && run.barred());
            assert!(run.cues.iter().any(|c| c == "mimic"));
            run.calm_for_test();
            for enemy in &mut run.enemies {
                enemy.hp = 0;
            }
            run.step(&BTreeMap::new());
            assert!(run.room().items.len() >= 2, "it coughs up its treasure");
        } else {
            assert!(run.room().chest.unwrap().open);
        }
    }
    assert!(
        (2..=14).contains(&woke),
        "about one chest in four bites ({woke}/24)"
    );
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_hazard_shots -- --ignored`
#[test]
#[ignore]
fn write_hazard_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, run: &Run| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(name), out).unwrap();
    };
    let keep_alive = |run: &mut Run| {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
    };
    // A wave about to come through a glowing doorway.
    let mut run = Run::new(0, 1, Some("Squire"));
    fight(&mut run);
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    for _ in 0..(3 * HZ) {
        keep_alive(&mut run);
        run.step(&BTreeMap::new());
        if run.waves.gate.is_some_and(|(_, t)| t < HZ / 2) {
            break;
        }
    }
    save("wave_gate.ppm", &run);
    for _ in 0..(HZ + 10) {
        keep_alive(&mut run);
        run.step(&BTreeMap::new());
    }
    save("wave_in.ppm", &run);
    // Spikes, vents, rocks: one room of each, at telling moments.
    let mut run = Run::new(0, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.traps = vec![
        Trap::Spikes {
            col: 4,
            row: 3,
            phase: 0,
        },
        Trap::Spikes {
            col: 17,
            row: 3,
            phase: 30,
        },
        Trap::Vent {
            side: 3,
            lane: 9.0,
            phase: 0,
        },
        Trap::Vent {
            side: 1,
            lane: 19.0,
            phase: 10,
        },
    ];
    run.rocks = vec![
        Rock {
            x: 14.0,
            y: 18.0,
            fall: 30,
            dust: 0,
        },
        Rock {
            x: 34.0,
            y: 18.0,
            fall: 6,
            dust: 0,
        },
    ];
    while (run.tick as u32) % hazards::SPIKE_PERIOD != 80 {
        keep_alive(&mut run);
        run.calm_for_test();
        run.traps = vec![
            Trap::Spikes {
                col: 4,
                row: 3,
                phase: 0,
            },
            Trap::Spikes {
                col: 17,
                row: 3,
                phase: 30,
            },
            Trap::Vent {
                side: 3,
                lane: 9.0,
                phase: 0,
            },
            Trap::Vent {
                side: 1,
                lane: 19.0,
                phase: 140,
            },
        ];
        run.step(&BTreeMap::new());
    }
    run.rocks = vec![
        Rock {
            x: 14.0,
            y: 18.0,
            fall: 30,
            dust: 0,
        },
        Rock {
            x: 34.0,
            y: 18.0,
            fall: 6,
            dust: 0,
        },
    ];
    save("traps.ppm", &run);
    run.traps = vec![
        Trap::Vent {
            side: 3,
            lane: 9.0,
            phase: 0,
        },
        Trap::Vent {
            side: 1,
            lane: 19.0,
            phase: 140,
        },
    ];
    while hazards::vent_beat(run.tick, 0) != 12 {
        keep_alive(&mut run);
        run.step(&BTreeMap::new());
    }
    save("vents.ppm", &run);
    // A mimic, mid-snap.
    for seed in 0..40 {
        let mut run = Run::new(seed, 1, None);
        let treasure = index_of(&run, RoomKind::Treasure);
        run.enter(treasure, Some(2));
        let chest = run.room().chest.unwrap();
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (chest.x - 4.0, chest.y + 5.0);
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (chest.x, chest.y + 1.5);
        run.step(&BTreeMap::new());
        if run.enemies.iter().any(|e| e.kind == EnemyKind::Mimic) {
            let hero = run.players.get_mut(&1).unwrap();
            (hero.x, hero.y) = (chest.x - 8.0, chest.y + 6.0);
            for _ in 0..50 {
                keep_alive(&mut run);
                run.step(&BTreeMap::new());
            }
            save("mimic.ppm", &run);
            break;
        }
    }
}

#[test]
fn vents_breathe_fire_that_crosses_the_room() {
    let mut run = Run::new(1, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    run.traps = vec![Trap::Vent {
        side: 3,
        lane: 9.0,
        phase: 0,
    }];
    while hazards::vent_beat(run.tick, 0) != 20 {
        run.step(&BTreeMap::new());
    }
    let fire: Vec<_> = run
        .projectiles
        .iter()
        .filter(|p| p.hostile && p.vx > 0.0)
        .collect();
    assert!(fire.len() >= 4, "a line of fire ({} embers)", fire.len());
    assert!(fire.iter().any(|p| p.x > 4.0), "and it leaves the wall");
}

#[test]
fn the_dodge_roll_carries_the_knight_through_shots_untouched() {
    let mut run = Run::new(1, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    let (x, y) = (hero.x, hero.y);
    run.step(&inputs(Input {
        move_y: -1,
        dash: true,
        ..Default::default()
    }));
    assert!(run.players[&1].rolling().is_some());
    // A shot right where the roll is going: it passes harmlessly.
    run.projectiles.push(shot(x, y - 2.0, 0.0, 1.0, true));
    for _ in 0..6 {
        run.step(&inputs(Input {
            move_y: -1,
            ..Default::default()
        }));
    }
    assert_eq!(run.players[&1].hp, 100);
    assert!(
        y - run.players[&1].y > 3.5,
        "a roll covers ground ({} → {})",
        y,
        run.players[&1].y
    );
    let rearm = run.players[&1].dash_cooldown;
    assert!(rearm > 0, "and needs a moment before the next");
    run.step(&inputs(Input {
        fire: true,
        dash: true,
        ..Default::default()
    }));
}

#[test]
fn a_raised_shield_turns_shots_from_the_front_only() {
    let mut run = Run::new(1, 1, None);
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    let hero = run.players.get_mut(&1).unwrap();
    hero.guard = cards::Guard::Shield;
    (hero.x, hero.y, hero.aim_x, hero.aim_y, hero.invulnerable) = (20.0, 14.0, 1.0, 0.0, 0);
    let guard = inputs(Input {
        dash: true,
        ..Default::default()
    });
    run.projectiles.push(shot(22.0, 14.0, -8.0, 0.0, true));
    for _ in 0..8 {
        run.step(&guard);
    }
    assert_eq!(run.players[&1].hp, 100, "blocked from the front");
    assert!(
        run.projectiles.iter().all(|p| !p.hostile),
        "and the shot is spent"
    );
    run.projectiles.push(shot(18.0, 14.0, 8.0, 0.0, true));
    for _ in 0..8 {
        run.step(&guard);
    }
    assert!(run.players[&1].hp < 100, "but not from behind");
    let before = run.projectiles.len();
    run.step(&inputs(Input {
        dash: true,
        aim_x: 1,
        fire: true,
        ..Default::default()
    }));
    assert_eq!(
        run.projectiles.len(),
        before,
        "no shooting behind a raised shield"
    );
}

#[test]
fn hallways_join_the_chambers() {
    let mut halls = 0;
    for seed in 0..12 {
        let run = Run::new(seed, 1, None);
        for room in run
            .dungeon
            .rooms
            .iter()
            .filter(|r| r.kind == RoomKind::Hall)
        {
            halls += 1;
            let walls = (1..ROWS as i32 - 1)
                .flat_map(|r| (1..COLS as i32 - 1).map(move |c| (c, r)))
                .filter(|&(c, r)| room.tile(c, r) == Tile::Wall)
                .count();
            assert!(
                walls > (COLS - 2) * (ROWS - 2) / 3,
                "a hall is mostly stone ({walls})"
            );
            assert_eq!(
                room.doors.iter().filter(|&&d| d).count(),
                2,
                "a passage runs between two doors"
            );
        }
    }
    assert!(halls >= 6, "floors have passages ({halls} in 12)");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_guard_shots -- --ignored`
#[test]
#[ignore]
fn write_guard_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, run: &Run| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(name), out).unwrap();
    };
    let mut shown = 0;
    for seed in 0..12 {
        let mut run = Run::new(seed, 1, None);
        if let Some(hall) = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Hall)
        {
            run.enter(hall, Some(2));
            save(&format!("hall_{shown}.ppm"), &run);
            shown += 1;
            if shown == 3 {
                break;
            }
        }
    }
    let mut run = Run::new(1, 1, Some("Squire"));
    fight(&mut run);
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 9999;
    }
    let shield = run.players.get_mut(&2).unwrap();
    shield.guard = cards::Guard::Shield;
    run.step(&BTreeMap::from([
        (
            1,
            Input {
                move_y: -1,
                dash: true,
                ..Default::default()
            },
        ),
        (
            2,
            Input {
                dash: true,
                aim_x: 1,
                aim_y: -1,
                ..Default::default()
            },
        ),
    ]));
    for _ in 0..3 {
        run.step(&BTreeMap::from([
            (
                1,
                Input {
                    move_y: -1,
                    ..Default::default()
                },
            ),
            (
                2,
                Input {
                    dash: true,
                    ..Default::default()
                },
            ),
        ]));
    }
    save("roll_and_shield.ppm", &run);
}

#[test]
fn every_knight_swings_a_sword_that_cuts_shots_from_the_air() {
    let mut run = Run::new(1, 1, None);
    fight(&mut run);
    run.calm_for_test();
    let hero = run.players[&1].clone();
    run.enemies.truncate(1);
    run.enemies[0].x = hero.x;
    run.enemies[0].y = hero.y - 1.8;
    run.enemies[0].age = TELEGRAPH;
    let before = run.enemies[0].hp;
    run.projectiles
        .push(shot(hero.x + 0.3, hero.y - 1.2, 0.0, 0.5, true));
    run.step(&inputs(Input {
        aim_y: -1,
        swing: true,
        ..Default::default()
    }));
    assert!(run.enemies[0].hp < before, "the blade bites");
    assert!(
        run.projectiles.iter().all(|p| !p.hostile),
        "and parries the shot"
    );
    assert!(run.sounds.contains(&"swing"));
    let rearm = run.players[&1].sword_cooldown;
    assert!(rearm > 0, "a swing needs a moment before the next");
}

#[test]
fn a_guardians_fall_shakes_the_view() {
    let mut run = Run::new(0, 1, None);
    let stairs = index_of(&run, RoomKind::Stairs);
    run.enter_for_test(stairs); // this fixture exercises boss combat, not political population
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert!(run.shake > 10);
    assert!(run.sounds.contains(&"boss_fall"));
}

fn trip_wall() -> cards::Card {
    let raw = "name Trip Wall\nkind guard\ntext A wide wall my friend shoots through.\nguard wall width=5 drain=10 empower=30\n";
    cards::check("trip-wall", raw).unwrap().card
}

#[test]
fn a_wall_card_is_clamped_and_costs_what_it_is_worth() {
    let checked = cards::check(
        "trip-wall",
        "name Trip Wall\nkind guard\nguard wall width=12 drain=1 empower=90\n",
    )
    .unwrap();
    let Some(cards::Guard::Wall(wall)) = checked.card.guard else {
        panic!("a wall")
    };
    assert_eq!((wall.width, wall.empower), (8, 40));
    assert!(
        wall.drain >= 8 * 5,
        "a wide strong wall drinks more ({})",
        wall.drain
    );
    assert!(checked.notes.iter().any(|n| n.contains("raised")));
}

#[test]
fn a_held_wall_stops_monster_shots_and_empowers_a_friends() {
    let mut run = Run::new(1, 1, Some("Matt"));
    fight(&mut run);
    run.calm_for_test();
    run.enemies.clear();
    let wall = trip_wall();
    let tank = run.players.get_mut(&1).unwrap();
    tank.guard = wall.guard.unwrap();
    (tank.x, tank.y, tank.aim_x, tank.aim_y) = (24.0, 20.0, 0.0, -1.0);
    let hold = Input {
        dash: true,
        ..Default::default()
    };
    run.step(&BTreeMap::from([(1, hold)]));
    assert!(run.players[&1].walling);
    // A monster's shot coming down at the wall, and a friend's going up through it.
    run.projectiles.push(shot(24.0, 17.0, 0.0, 9.0, true));
    run.projectiles.push(shot(24.5, 19.5, 0.0, -9.0, false));
    for _ in 0..8 {
        run.step(&BTreeMap::from([(1, hold)]));
    }
    assert!(
        run.projectiles.iter().all(|p| !p.hostile),
        "the wall stops the monster's shot"
    );
    let friendly = run
        .projectiles
        .iter()
        .find(|p| !p.hostile)
        .expect("the friend's shot flies on");
    assert!(friendly.empowered);
    assert!(run.players[&1].mana < MAX_MANA, "holding drains mana");
    let low = run.players[&1].mana;
    for _ in 0..(3 * HZ) {
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&1].mana > low, "and it flows back when let go");
}

#[test]
fn the_sanctuary_mends_and_allows_one_reforge_a_floor() {
    let mut run = Run::new(0, 1, None);
    run.players.get_mut(&1).unwrap().hp = 40;
    run.descend_for_test();
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    assert!(run.room().cleared && run.enemies.is_empty());
    assert!(run.players[&1].hp > 40, "a quiet room mends");
    assert!(run.cues.iter().any(|c| c == "sanctuary"));
    assert!(run.can_reforge(1));
    let bow = run.book.get("crossbow").unwrap().clone();
    assert!(
        run.reforge(1, knights::Part::Defense, bow).is_err(),
        "a weapon is not a guard"
    );
    run.reforge(1, knights::Part::Defense, trip_wall()).unwrap();
    assert!(matches!(run.players[&1].guard, cards::Guard::Wall(_)));
    assert!(!run.can_reforge(1), "once per floor");
    assert!(run.book.get("trip-wall").is_some());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_wall_shot -- --ignored`
#[test]
#[ignore]
fn write_wall_shot() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::new(1, 1, Some("Matt"));
    fight(&mut run);
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 9999;
    }
    let tank = run.players.get_mut(&1).unwrap();
    tank.guard = trip_wall().guard.unwrap();
    (tank.x, tank.y, tank.aim_x, tank.aim_y) = (24.0, 20.0, 0.0, -1.0);
    let matt = run.players.get_mut(&2).unwrap();
    (matt.x, matt.y, matt.aim_x, matt.aim_y) = (24.0, 23.0, 0.0, -1.0);
    for i in 0..10 {
        run.step(&BTreeMap::from([
            (
                1,
                Input {
                    dash: true,
                    ..Default::default()
                },
            ),
            (
                2,
                Input {
                    aim_y: -1,
                    fire: i % 3 == 0,
                    ..Default::default()
                },
            ),
        ]));
    }
    let img = crate::stage::world_viz::overworld::arena::frame(&run, 384, 224);
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join("wall.ppm"), out).unwrap();
}

#[test]
fn the_first_floors_guardian_leaves_a_sanctuary_before_the_stairs() {
    let mut run = Run::new(0, 1, None);
    let stairs = index_of(&run, RoomKind::Stairs);
    run.enter_for_test(stairs); // this fixture exercises boss combat, not political population
    run.calm_for_test();
    // The guardian falls with the knight standing on the stairs.
    let middle = (run.room().width() / 2.0, run.room().height() / 2.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = middle;
    hero.hp = 40;
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.floor(), 1, "no drop past the Sanctuary");
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    assert!(run.players[&1].hp > 40, "a quiet room mends");
    assert!(run.cues.iter().any(|c| c == "sanctuary"));
    assert_eq!(run.hallowed, Some(run.tick), "its sign goes up");
    assert!(run.can_reforge(1));
    assert_eq!(run.music(), Some("sanctuary"));
    let (_, altar_y) = run.room().altar();
    assert!(
        altar_y < middle.1 - 4.0,
        "the altar stands above the stairs"
    );
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert_eq!(
        run.floor(),
        1,
        "standing still on the stairs keeps you here"
    );
    // Off the stairs and back on: down.
    run.players.get_mut(&1).unwrap().y = middle.1 + 6.0;
    run.step(&BTreeMap::new());
    run.players.get_mut(&1).unwrap().y = middle.1;
    run.step(&BTreeMap::new());
    assert_eq!(run.floor(), 2);
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    assert!(
        run.can_reforge(1),
        "the next floor's Sanctuary forges again"
    );
}

#[test]
fn deeper_stairs_rooms_stay_stairs() {
    let mut run = Run::new(0, 1, None);
    run.descend_for_test();
    let stairs = index_of(&run, RoomKind::Stairs);
    run.enter_for_test(stairs); // this fixture exercises boss combat, not political population
    slay_all(&mut run);
    assert_eq!(run.room().kind, RoomKind::Stairs);
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_sanctuary_shots -- --ignored`
#[test]
#[ignore]
fn write_sanctuary_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::new(0, 1, Some("Matt"));
    let stairs = index_of(&run, RoomKind::Stairs);
    run.enter_for_test(stairs); // this fixture exercises boss combat, not political population
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    for (name, ticks) in [("sanctuary1_sign", 20), ("sanctuary1_after", 5 * HZ)] {
        for _ in 0..ticks {
            run.step(&BTreeMap::new());
        }
        let img = crate::stage::world_viz::overworld::arena::frame(&run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    }
}

#[test]
fn a_sanctuary_privy_hides_a_knight_and_lets_them_out_refreshed_once() {
    let mut run = Run::new(0, 1, Some("Matt"));
    run.descend_for_test();
    assert_eq!(run.room().kind, RoomKind::Sanctuary);
    let (px, py) = run.room().privy().expect("a privy in the Sanctuary");
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (px, py);
    hero.hp = 50;
    hero.mana = 10.0;
    run.step(&BTreeMap::new());
    assert!(run.players[&1].privy > 0, "in they go");
    assert!(run.cues.iter().any(|c| c == "privy"));
    assert_eq!(run.players[&2].privy, 0, "one at a time is up to them");
    let x = run.players[&1].x;
    for _ in 0..2 * HZ {
        run.step(&inputs(Input {
            move_x: 1,
            ..Default::default()
        }));
    }
    let hero = &run.players[&1];
    assert_eq!(hero.privy, 0, "and out");
    assert_eq!(hero.x, x, "nobody walks while inside");
    assert_eq!((hero.hp, hero.mana), (60, MAX_MANA));
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].privy, 0, "once a Sanctuary");
    // A fighting room has none.
    fight(&mut run);
    assert!(run.room().privy().is_none());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_privy_shots -- --ignored`
#[test]
#[ignore]
fn write_privy_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::stage::world_viz::overworld::arena::frame(run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut run = Run::new(0, 1, Some("Matt"));
    run.descend_for_test();
    for _ in 0..5 * HZ {
        run.step(&BTreeMap::new());
    }
    let (px, py) = run.room().privy().unwrap();
    let matt = run.players.get_mut(&2).unwrap();
    (matt.x, matt.y) = (px + 3.0, py + 1.0);
    save(&run, "privy_open");
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (px, py);
    run.step(&BTreeMap::new());
    save(&run, "privy_occupied");
    run.wishing = true;
    for _ in 0..7 {
        run.step(&BTreeMap::new());
    }
    save(&run, "dais_wishing");
    // Floor 1's Sanctuary, a great hall.
    let mut run = Run::new(0, 1, None);
    let stairs = index_of(&run, RoomKind::Stairs);
    run.enter_for_test(stairs); // this fixture exercises boss combat, not political population
    slay_all(&mut run);
    for _ in 0..5 * HZ {
        run.step(&BTreeMap::new());
    }
    let (px, py) = run.room().privy().unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (px + 4.0, py + 2.0);
    save(&run, "privy_floor1");
}

/// `cargo test --release frame_costs -- --ignored --nocapture`: what one
/// delve frame costs to draw and to encode, for the frame-rate budget.
#[test]
#[ignore]
fn frame_costs() {
    use image::ImageEncoder;
    let mut run = Run::new(0, 1, Some("Matt"));
    fight(&mut run);
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    let n = 120;
    let t = std::time::Instant::now();
    let mut img = crate::stage::world_viz::overworld::arena::frame(&run, 384, 224);
    for _ in 0..n {
        run.step(&BTreeMap::new());
        img = crate::stage::world_viz::overworld::arena::frame(&run, 384, 224);
    }
    let draw = t.elapsed() / n;
    let rgb = img.rgb_bytes();
    let t = std::time::Instant::now();
    let mut png = Vec::new();
    for _ in 0..20 {
        png.clear();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&rgb, 384, 224, image::ExtendedColorType::Rgb8)
            .unwrap();
    }
    let png_t = t.elapsed() / 20;
    let t = std::time::Instant::now();
    let mut z = Vec::new();
    for _ in 0..20 {
        z.clear();
        image::codecs::png::PngEncoder::new_with_quality(
            &mut z,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::NoFilter,
        )
        .write_image(&rgb, 384, 224, image::ExtendedColorType::Rgb8)
        .unwrap();
    }
    let z_t = t.elapsed() / 20;
    let state =
        serde_json::to_vec(&(&run.players, &run.enemies, &run.projectiles, run.tick)).unwrap();
    eprintln!(
        "FRAME draw={draw:?} png={png_t:?} ({} KB) png-fast={z_t:?} ({} KB) raw={} KB live-state={} B",
        png.len() / 1024,
        z.len() / 1024,
        rgb.len() / 1024,
        state.len()
    );
}

/// `cargo test --release scaled_frame_costs -- --ignored --nocapture`
#[test]
#[ignore]
fn scaled_frame_costs() {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let mut run = Run::new(0, 1, Some("Matt"));
    fight(&mut run);
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    let img = crate::stage::world_viz::overworld::arena::frame(&run, 384, 224);
    let src = img.rgb_bytes();
    for k in [1u32, 2, 3, 5] {
        let (w, h) = (384 * k, 224 * k);
        let mut out = vec![0u8; (w * h * 3) as usize];
        for y in 0..h {
            for x in 0..w {
                let s = (((y / k) * 384 + x / k) * 3) as usize;
                let d = ((y * w + x) * 3) as usize;
                out[d..d + 3].copy_from_slice(&src[s..s + 3]);
            }
        }
        for (name, c, f) in [
            ("fast-none", CompressionType::Fast, FilterType::NoFilter),
            ("fast-up", CompressionType::Fast, FilterType::Up),
            ("default-up", CompressionType::Default, FilterType::Up),
        ] {
            let t = std::time::Instant::now();
            let mut png = Vec::new();
            for _ in 0..5 {
                png.clear();
                PngEncoder::new_with_quality(&mut png, c, f)
                    .write_image(&out, w, h, image::ExtendedColorType::Rgb8)
                    .unwrap();
            }
            eprintln!(
                "SCALED k={k} {w}x{h} {name}: {:?} {} KB",
                t.elapsed() / 5,
                png.len() / 1024
            );
        }
    }
}

#[test]
fn spells_stay_slotted_recharge_independently_and_reuse_play_effects() {
    let mut run = Run::new(19, 1, None);
    run.calm_for_test();
    for (id, effect) in [
        ("salve", "heal 20"),
        ("ward", "ward 2"),
        ("bombs", "bombs 1"),
        ("overflow", "nova 80"),
    ] {
        let raw = format!("name {id}\nkind spell\ncooldown 2\n{effect}\n");
        assert!(run.add_card(cards::check(id, &raw).unwrap().card, 1));
    }
    assert_eq!(
        run.players[&1].spells,
        [
            Some("salve".into()),
            Some("ward".into()),
            Some("bombs".into())
        ]
    );
    assert!(run.room().items.iter().any(|i| i.card == "overflow"));
    assert_eq!(run.blasts.len(), 3);
    run.players.get_mut(&1).unwrap().hp = 40;
    let cast = BTreeMap::from([(
        1,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]);
    run.step(&cast);
    assert_eq!(run.players[&1].hp, 60);
    assert_eq!(run.players[&1].spell_cooldowns, [2 * HZ, 0, 0]);
    for _ in 0..(2 * HZ - 1) {
        run.step(&cast);
    }
    assert_eq!(run.players[&1].hp, 60, "held key cannot bypass recharge");
    run.step(&cast);
    assert_eq!(run.players[&1].hp, 80);
    assert_eq!(run.players[&1].spells[0].as_deref(), Some("salve"));
    let bombs = run.players[&1].bombs;
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 3,
            ..Default::default()
        },
    )]));
    assert_eq!(run.players[&1].bombs, bombs + 1);
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 2,
            ..Default::default()
        },
    )]));
    assert!(run.players[&1].invulnerable >= 2 * HZ);
    assert_eq!(run.players[&1].hand.len(), 0);
    assert!(
        !Input {
            cast: 4,
            ..Default::default()
        }
        .valid()
    );
}

#[test]
fn spell_shapes_launch_from_the_knight_with_owned_shot_traits() {
    let mut run = Run::new(19, 1, None);
    run.join_seat(2, "friend");
    let hero = run.players.get_mut(&2).unwrap();
    hero.bonus = cards::Bonus {
        damage: 25,
        bounce: 2,
        homing: 40,
        chain: 2,
        vamp: 20,
        mend: 10,
        orbit: 1,
        burst: 30,
        pierce: 1,
        ..Default::default()
    };
    for (shape, count) in [
        (cards::Shape::Bolt, 1),
        (cards::Shape::Ring, 12),
        (cards::Shape::Meteor, 1),
    ] {
        let mut shots = Vec::new();
        hero.spell_shots(shape, 2, &mut shots);
        assert_eq!(shots.len(), count);
        for shot in shots {
            assert!(!shot.hostile);
            assert!((shot.x - hero.x).hypot(shot.y - hero.y) < 0.7);
            assert_eq!(shot.traits.owner, 2);
            assert_eq!(shot.traits.bounce, 2);
            assert_eq!(shot.traits.homing, 8);
            assert_eq!(shot.traits.chain, 2);
            assert_eq!(shot.traits.mend, 10);
            assert_eq!(shot.traits.burst, 30);
            assert_eq!(shot.pierce, 1);
            assert!(shot.damage >= 25);
            if shape == cards::Shape::Meteor {
                assert_eq!(shot.kind, Shot::Ball);
            }
        }
    }
}

#[test]
fn a_vigil_wards_its_knight_and_mends_the_knights_beside_it() {
    let mut run = Run::new(0, 1, Some("Friend"));
    run.calm_for_test();
    let key = Input {
        vigil: true,
        ..Default::default()
    };
    // The key's edge raises it; holding the key on does not flip it back.
    for _ in 0..6 {
        run.step(&inputs(key));
    }
    assert!(run.players[&1].vigil && run.players[&1].stone);

    // Letting go keeps it: the composer, a nap, a lost connection.
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    let (x, y) = (hero.x, hero.y);
    run.projectiles = vec![shot(x - 2.0, y, 120.0, 0.0, true)];
    run.step(&BTreeMap::new());
    assert!(run.players[&1].vigil);
    assert_eq!(run.players[&1].hp, 100, "warded");

    // A hurt friend beside the statue mends; one across the room does not.
    let friend = run.players.get_mut(&2).unwrap();
    (friend.x, friend.y, friend.hp) = (x + 2.0, y, 40);
    for _ in 0..2 * HZ {
        run.step(&BTreeMap::new());
    }
    let mended = run.players[&2].hp;
    assert!(
        (44..=48).contains(&mended),
        "about three a second: {mended}"
    );
    run.players.get_mut(&2).unwrap().x = x + VIGIL_REACH + 3.0;
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&2].hp, mended, "out of reach");

    // The key again wakes the knight where it stood, then rests a while.
    run.step(&inputs(key));
    let hero = &run.players[&1];
    assert!(!hero.vigil && !hero.stone);
    assert_eq!((hero.x, hero.y), (x, y));
    run.step(&BTreeMap::new());
    run.step(&inputs(key));
    assert!(!run.players[&1].vigil, "not a panic button");
}

#[test]
fn controls_without_a_vigil_read_the_same_to_an_older_host() {
    let idle = serde_json::to_value(Input::default()).unwrap();
    assert!(idle.get("vigil").is_none(), "{idle}");
    let old: Input = serde_json::from_value(idle).unwrap();
    assert!(!old.vigil);
}

use super::*;
use crate::render::overworld::kingdom as geography;

fn surface(seats: u32) -> Run {
    let mut home = home::Home::default();
    home.levels.insert(home::Station::Wing, 1);
    let mut r = Run::at_home(41, 1, None, home, Default::default());
    for id in 2..=seats {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.walk_kingdom(Some(geography::keep_stand()));
    r.floor_plates.clear();
    r
}
fn visit(r: &mut Run, kind: RoomKind) {
    let geo = geography::geography(&r.kingdom.wishes);
    let root = if college::is_room(kind) {
        RoomKind::CollegeGrounds
    } else {
        kind
    };
    if let Some(site) = geo.sites.iter().position(|s| s.room == root) {
        r.enter_kingdom_place(site);
    }
    if r.room().kind != kind {
        let at = world::room_of(&r.dungeon, kind).unwrap();
        r.arrive(at, (12.0, 10.5));
    }
}
fn leave(r: &mut Run, seat: u32) {
    r.step(&BTreeMap::from([(
        seat,
        Input {
            leave_activity: true,
            interact: true,
            fire: true,
            dash: true,
            ..Default::default()
        },
    )]));
}
fn clean(r: &Run) {
    assert!(r.pastimes.round.is_none());
    assert!(r.pastimes.wager.is_none());
    assert!(r.joust.is_none() && r.chariot.is_none());
    assert_eq!(r.forest.hunt_left, 0);
    assert!(!r.forest.singing);
    assert!(r.college.owner.is_none());
    assert!(
        r.enemies.is_empty() && r.projectiles.is_empty(),
        "{:?}: {:?}",
        r.room().kind,
        r.enemies.iter().map(|e| e.kind).collect::<Vec<_>>()
    );
    assert!(!r.waves.pending());
    assert!(r.traps.is_empty() && r.rocks.is_empty() && r.slams.is_empty());
    assert!(r.players.values().all(|h| h.privy == 0
        && h.pit.stop.is_none()
        && h.dash_ticks == 0
        && !h.shielding
        && !h.walling));
}

#[test]
fn together_exits_every_surface_and_home_mode_before_and_during_combat() {
    let base = surface(1);
    let kinds: Vec<_> = base
        .dungeon
        .rooms
        .iter()
        .map(|r| r.kind)
        .filter(|&k| room(k))
        .collect();
    for &(kind, _) in &world::ROOMS {
        assert!(kinds.contains(&kind), "missing {kind:?}");
    }
    for kind in [
        RoomKind::Ranch,
        RoomKind::Pasture,
        RoomKind::Fortune,
        RoomKind::Yard,
        RoomKind::Trophies,
        RoomKind::RealmHall,
    ] {
        assert!(kinds.contains(&kind));
    }
    for kind in kinds {
        for active in [false, true] {
            let mut r = surface(1);
            let outside = (r.players[&1].x, r.players[&1].y);
            visit(&mut r, kind);
            assert!(r.can_leave_activity(1), "{kind:?}");
            if active {
                match kind {
                    RoomKind::Lists => r.mount_up(1),
                    RoomKind::Hippodrome => r.board_chariot(1),
                    RoomKind::Forest => {
                        r.forest.hunt_left = forest::HUNT_TIME;
                        r.forest.singing = true;
                        r.forest.show_left = HZ;
                    }
                    _ if college::is_room(kind) => {
                        r.college.owner = Some(1);
                        r.college.page = college::Page::Menu;
                    }
                    _ => {}
                }
                r.spawn(EnemyKind::Skeleton);
                r.phase = Phase::Fighting;
                assert!(r.barred());
            }
            leave(&mut r, 1);
            let mirror = r.wire_copy();
            assert_eq!(
                serde_json::to_value(&mirror.exits).unwrap(),
                serde_json::to_value(&r.exits).unwrap(),
                "mirror {kind:?}"
            );
            assert_ne!(r.room().kind, kind, "{kind:?} active={active}");
            if geography::outdoor(kind) || college::is_room(kind) {
                assert_eq!(r.room().kind, RoomKind::Kingdom, "{kind:?}");
                assert_eq!((r.players[&1].x, r.players[&1].y), outside);
            }
            clean(&r);
            assert!(r.valid_snapshot(), "snapshot {kind:?}");
            visit(&mut r, kind);
            assert!(
                r.joust.is_none() && r.chariot.is_none() && r.pastimes.round.is_none(),
                "re-entry {kind:?}"
            );
        }
    }
}

#[test]
fn together_exits_all_fifteen_pastimes_including_every_3d_stage() {
    for game in pastimes::Game::ALL {
        for seat in 1..=MAX_PLAYERS {
            let mut r = surface(4);
            visit(&mut r, game.room());
            let start = game.start();
            for (&id, h) in &mut r.players {
                (h.x, h.y) = if id == seat {
                    (start.0 * TILE_UNITS, start.1 * TILE_UNITS)
                } else {
                    (24.0, 22.0)
                };
            }
            r.step(&BTreeMap::new());
            assert_eq!(
                r.pastimes.round.as_ref().map(|r| r.game),
                Some(game),
                "start {game:?}"
            );
            // Everyone plays at once: the friends each have a heat of
            // their own, and play on when the starter steps out.
            let clock = |r: &Run| r.pastimes.heats.values().map(|h| h.left).min();
            let left = clock(&r).expect("friends join the heat");
            leave(&mut r, seat);
            assert!(r.exits.outside.contains_key(&seat), "{game:?} seat {seat}");
            assert!(clock(&r).unwrap() < left, "friends continue {game:?}");
            let view = r.view_for_seat(seat).unwrap();
            assert_eq!(view.room().kind, RoomKind::Kingdom);
            clean(&view);
            let hp = r.players[&seat].hp;
            for _ in 0..15 {
                r.step(&BTreeMap::new());
            }
            assert_eq!(r.players[&seat].hp, hp, "outside is safe {game:?}");
            let saved: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
            assert!(saved.valid_snapshot());
            for other in 1..=MAX_PLAYERS {
                if other != seat {
                    leave(&mut r, other);
                }
            }
            assert_eq!(r.room().kind, RoomKind::Kingdom);
            clean(&r);
            visit(&mut r, game.room());
            assert!(r.pastimes.round.is_none(), "clean re-entry {game:?}");
            for h in r.players.values_mut() {
                (h.x, h.y) = (start.0 * TILE_UNITS, start.1 * TILE_UNITS);
            }
            r.step(&BTreeMap::new());
            assert_eq!(r.pastimes.round.as_ref().map(|r| r.game), Some(game));
        }
    }
}

#[test]
fn together_exits_optional_delve_halls_and_dragon_retreat_without_a_reward() {
    for kind in [
        RoomKind::Pit,
        RoomKind::Wanted,
        RoomKind::Ledge,
        RoomKind::Secret,
        RoomKind::Lair,
    ] {
        let (mut r, at) = (0..64)
            .find_map(|seed| {
                let mut r = Run::new(seed, 1, None);
                if kind != RoomKind::Ledge {
                    r.descend_for_test();
                }
                if kind == RoomKind::Lair {
                    r.descend_for_test();
                }
                let at = r.dungeon.rooms.iter().position(|room| room.kind == kind)?;
                Some((r, at))
            })
            .unwrap_or_else(|| panic!("missing fixture {kind:?}"));
        r.clear_prerequisites_for_test();
        r.clear_for_test();
        let from = r.at;
        let position = (r.players[&1].x, r.players[&1].y);
        r.enter(at, None);
        assert!(r.can_leave_activity(1));
        r.step(&BTreeMap::new());
        let score = r.score;
        leave(&mut r, 1);
        assert_eq!(r.at, from, "{kind:?}");
        assert_eq!((r.players[&1].x, r.players[&1].y), position);
        assert_eq!(r.score, score);
        assert!(r.triumph.is_none());
        clean(&r);
        assert!(r.valid_snapshot(), "{kind:?}");
        r.enter(at, None);
        assert!(r.can_leave_activity(1));
        if kind == RoomKind::Lair {
            assert_eq!(
                r.enemies
                    .iter()
                    .filter(|e| e.kind == EnemyKind::Dragon)
                    .count(),
                1
            );
        }
    }
}

#[test]
fn together_exits_ordinary_fights_keep_their_barred_doors() {
    let mut r = Run::new(1, 1, None);
    let at = r
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    r.enter(at, None);
    assert!(r.barred());
    assert!(!r.can_leave_activity(1));
    leave(&mut r, 1);
    assert_eq!(r.at, at);
    assert!(r.barred());
}

#[test]
fn together_exits_pause_and_invalid_inputs_cannot_run_or_skip_the_activity() {
    let mut r = surface(1);
    visit(&mut r, RoomKind::Hippodrome);
    r.board_chariot(1);
    let before = serde_json::to_value(&r.chariot).unwrap();
    r.step(&BTreeMap::from([(
        1,
        Input {
            activity_menu: true,
            ..Default::default()
        },
    )]));
    assert_eq!(serde_json::to_value(&r.chariot).unwrap(), before);
    r.step(&BTreeMap::from([(
        1,
        Input {
            leave_activity: true,
            move_x: 2,
            ..Default::default()
        },
    )]));
    assert_eq!(r.room().kind, RoomKind::Hippodrome);
    leave(&mut r, 1);
    clean(&r);
}

#[test]
fn together_exits_privy_every_stage_is_personal_and_restores_armour() {
    for stage in pitstop::Stage::ALL {
        let mut r = Run::new(0, 1, Some("Friend"));
        r.descend_for_test();
        let at = r.at;
        let door = r.room().privy().unwrap();
        for h in r.players.values_mut() {
            (h.x, h.y) = door;
        }
        r.step(&BTreeMap::new());
        assert!(r.players[&1].pit.stop.is_some());
        r.players
            .get_mut(&1)
            .unwrap()
            .pit
            .stop
            .as_mut()
            .unwrap()
            .stage = stage;
        r.players.get_mut(&1).unwrap().gear = 2;
        leave(&mut r, 1);
        assert_eq!(r.at, at);
        assert!(r.players[&1].pit.stop.is_none() && r.players[&1].privy == 0);
        assert_eq!(r.players[&1].gear, 0);
        assert!(
            r.players[&2].pit.stop.is_some(),
            "friend keeps their stall {stage:?}"
        );
    }
}

#[test]
fn together_exits_dragon_attacks_are_finite_and_defeat_does_not_respawn_it() {
    let mut r = Run::new(31, 1, None);
    r.descend_for_test();
    r.descend_for_test();
    r.clear_prerequisites_for_test();
    let lair = r
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Lair)
        .unwrap();
    r.enter(lair, Some(2));
    assert_eq!(
        r.enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Dragon)
            .count(),
        1
    );
    assert!(r.waves.left <= 3);
    let from = r.exits.entry.as_ref().unwrap().at;
    for _ in 0..1200 {
        for enemy in &mut r.enemies {
            enemy.hp = 0;
        }
        r.step(&BTreeMap::new());
        if r.room().cleared {
            break;
        }
    }
    assert!(r.room().cleared);
    assert!(!r.barred());
    assert!(r.light.is_some());
    r.enter(from, None);
    r.enter(lair, Some(2));
    for _ in 0..300 {
        r.step(&BTreeMap::new());
    }
    assert!(r.enemies.is_empty());
    assert!(!r.barred());
}

#[test]
fn together_exits_fortune_finishes_only_its_chosen_wedge_and_never_rerolls() {
    let mut r = surface(1);
    visit(&mut r, RoomKind::Fortune);
    let (x, y, w, h) = fortune::LEVER;
    (
        r.players.get_mut(&1).unwrap().x,
        r.players.get_mut(&1).unwrap().y,
    ) = (
        (x as f32 + w as f32 / 2.0) * TILE_UNITS,
        (y as f32 + h as f32 / 2.0) * TILE_UNITS,
    );
    for _ in 0..home::BUY_HOLD + 1 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
    let spin = r.spin.unwrap();
    assert!(!spin.done(r.tick));
    leave(&mut r, 1);
    assert!(r.spin.unwrap().done(r.tick));
    assert_eq!(r.mode, fortune::Mode::WHEEL[usize::from(spin.wedge)]);
    visit(&mut r, RoomKind::Fortune);
    assert_eq!(r.spin.unwrap().wedge, spin.wedge);
}

#[test]
fn together_exits_forge_and_settlement_rooms_return_cleanly() {
    let mut r = surface(1);
    let from = r.at;
    for kind in [
        RoomKind::Forge,
        RoomKind::Workshop,
        RoomKind::Stockpile,
        RoomKind::Quarters,
    ] {
        let mut authored = barony::forge_room((30, 30), 2);
        authored.kind = kind;
        r.dungeon.rooms.push(authored);
        let at = r.dungeon.rooms.len() - 1;
        r.enter(at, None);
        assert!(r.can_leave_activity(1));
        leave(&mut r, 1);
        assert_eq!(r.at, from);
        clean(&r);
        r.enter(at, None);
        leave(&mut r, 1);
        clean(&r);
    }
}

#[test]
fn together_exits_outside_seats_walk_and_can_rejoin_without_capturing_friends() {
    let mut r = surface(2);
    visit(&mut r, RoomKind::Forest);
    r.forest.hunt_left = forest::HUNT_TIME;
    leave(&mut r, 1);
    let before = (r.players[&1].x, r.players[&1].y);
    r.step(&BTreeMap::from([(
        1,
        Input {
            move_x: 1,
            ..Default::default()
        },
    )]));
    assert_ne!((r.players[&1].x, r.players[&1].y), before);
    assert!(r.forest.hunt_left > 0);
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(!r.exits.outside.contains_key(&1));
    assert_eq!(r.room().kind, RoomKind::Forest);
    assert!(r.forest.hunt_left > 0);
}

#[test]
fn together_exits_real_door_retreat_does_not_bounce_back_into_the_hall() {
    let mut r = Run::new(0, 1, None);
    let hall = r
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Ledge)
        .unwrap();
    let back = r.dungeon.rooms[hall].way_back().unwrap();
    let parent = r.dungeon.neighbour(hall, back).unwrap();
    r.enter(parent, None);
    r.clear_for_test();
    let direction = (back + 2) % 4;
    let (w, h) = (r.room().width(), r.room().height());
    let hero = r.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = match direction {
        0 => (w / 2.0, 0.4),
        1 => (w - 0.4, h / 2.0),
        2 => (w / 2.0, h - 0.4),
        _ => (0.4, h / 2.0),
    };
    r.travel();
    assert_eq!(r.at, hall);
    leave(&mut r, 1);
    assert_eq!(r.at, parent);
    for _ in 0..30 {
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.at, parent, "must step back across the door deliberately");
}

#[test]
fn together_exits_disconnect_of_the_last_participant_cleans_up_for_returned_knights() {
    let mut r = surface(2);
    visit(&mut r, RoomKind::Hippodrome);
    r.board_chariot(1);
    leave(&mut r, 1);
    r.leave(2);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert!(r.exits.outside.is_empty());
    clean(&r);
}

#[test]
fn together_exits_optional_combat_targets_only_knights_still_inside() {
    for kind in [
        RoomKind::Ledge,
        RoomKind::Pit,
        RoomKind::Wanted,
        RoomKind::Lair,
    ] {
        let (mut r, at) = (0..64)
            .find_map(|seed| {
                let mut r = Run::new(seed, 1, Some("Couch or joined friend"));
                if kind != RoomKind::Ledge {
                    r.descend_for_test();
                }
                if kind == RoomKind::Lair {
                    r.descend_for_test();
                }
                let at = r.dungeon.rooms.iter().position(|r| r.kind == kind)?;
                Some((r, at))
            })
            .unwrap_or_else(|| panic!("missing fixture {kind:?}"));
        r.clear_prerequisites_for_test();
        r.clear_for_test();
        r.enter(at, None);
        for h in r.players.values_mut() {
            h.invulnerable = 1000;
        }
        leave(&mut r, 2);
        let hp = r.players[&2].hp;
        for _ in 0..180 {
            r.step(&BTreeMap::new());
        }
        assert_eq!(r.players[&2].hp, hp, "outside {kind:?}");
        assert_eq!(r.at, at);
        assert!(!r.enemies.is_empty(), "encounter still plays {kind:?}");
        clean(&r.view_for_seat(2).unwrap());
        leave(&mut r, 1);
        assert_ne!(r.at, at);
        clean(&r);
    }
}

#[test]
fn together_exits_missing_exit_tiles_cannot_prevent_leaving() {
    let mut r = surface(1);
    visit(&mut r, RoomKind::RealmHall);
    r.dungeon.rooms[r.at].doors = [false; 4];
    let room = &mut r.dungeon.rooms[r.at];
    for y in 0..room.rows {
        for x in 0..room.cols {
            if matches!(room.tile(x as i32, y as i32), Tile::Door | Tile::Stairs) {
                room.set(x, y, Tile::Wall);
            }
        }
    }
    leave(&mut r, 1);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    clean(&r);
}

#[test]
fn together_exits_privy_can_be_declined_before_input_capture_begins() {
    let mut r = Run::new(0, 1, None);
    r.descend_for_test();
    let door = r.room().privy().unwrap();
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = door;
    assert!(r.can_leave_activity(1));
    leave(&mut r, 1);
    assert!(r.players[&1].pit.stop.is_none());
    r.step(&BTreeMap::new());
    assert!(r.players[&1].pit.stop.is_none());
}

#[test]
fn together_exits_dragon_keep_surface_hall_returns_to_its_actual_door() {
    let mut r = surface(1);
    let geo = geography::geography(&r.kingdom.wishes);
    let site = geo
        .sites
        .iter()
        .position(|s| s.name.eq_ignore_ascii_case("Dragon Keep"))
        .unwrap();
    assert_eq!(geo.sites[site].room, RoomKind::RealmHall);
    let stand = geo.sites[site].stand;
    let position = (
        (stand.0 as f32 + 0.5) * TILE_UNITS,
        (stand.1 as f32 + 0.7) * TILE_UNITS,
    );
    (
        r.players.get_mut(&1).unwrap().x,
        r.players.get_mut(&1).unwrap().y,
    ) = position;
    r.enter_kingdom_place(site);
    assert_eq!(r.room().kind, RoomKind::RealmHall);
    assert!(r.enemies.is_empty());
    leave(&mut r, 1);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert_eq!((r.players[&1].x, r.players[&1].y), position);
    clean(&r);
}

#[test]
fn together_exits_new_seat_joins_inside_and_shared_floor_waits_for_returned_seats() {
    let mut r = surface(2);
    visit(&mut r, RoomKind::Forest);
    leave(&mut r, 1);
    let inside = (r.players[&2].x, r.players[&2].y);
    r.join_seat(3, "New arrival");
    assert!(r.players.contains_key(&3));
    assert!((r.players[&3].x - inside.0).hypot(r.players[&3].y - inside.1) < 4.0);
    let depth = r.floor();
    r.descend_for_test();
    assert_eq!(r.floor(), depth);
    assert_eq!(r.exits.outside.len(), 1);
    assert!(r.valid_snapshot());
}

#[test]
fn together_exits_dragon_hold_the_stair_waves_finish_through_normal_progression() {
    let mut r = Run::new(31, 1, None);
    r.descend_for_test();
    r.descend_for_test();
    r.boss_gates = None;
    r.mode = fortune::Mode::HoldTheStair;
    let lair = r
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Lair)
        .unwrap();
    r.enter(lair, None);
    assert_eq!(r.waves.left, 3);
    for h in r.players.values_mut() {
        h.invulnerable = 2000;
    }
    for _ in 0..1200 {
        for e in &mut r.enemies {
            e.hp = 0;
        }
        r.step(&BTreeMap::new());
        if r.room().cleared {
            break;
        }
    }
    assert!(r.room().cleared);
    assert_eq!(r.waves.left, 0);
    assert!(r.enemies.is_empty());
    assert!(!r.barred());
}

#[test]
fn together_exits_physical_surface_door_leaves_only_the_knight_who_crosses_it() {
    let mut r = surface(2);
    visit(&mut r, RoomKind::Forest);
    r.forest.hunt_left = forest::HUNT_TIME;
    r.players.get_mut(&1).unwrap().y = 0.4;
    r.travel();
    assert_eq!(r.room().kind, RoomKind::Forest);
    assert!(r.exits.outside.contains_key(&1));
    assert!(r.forest.hunt_left > 0);
    let view = r.view_for_seat(1).unwrap();
    assert_eq!(view.room().kind, RoomKind::Kingdom);
    leave(&mut r, 2);
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    clean(&r);
}

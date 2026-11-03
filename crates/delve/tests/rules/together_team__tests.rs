use super::*;

fn team_input() -> Input {
    let mut value = serde_json::to_value(Input::default()).unwrap();
    value["team"] = serde_json::json!(true);
    serde_json::from_value(value).expect("team key must be accepted")
}

#[test]
fn team_key_survives_command_and_held_repeat() {
    use angel_net::cmd::Command;
    let input = team_input();
    let cmd = command::DelveCmd {
        held: input,
        pressed: input,
    };
    let mut bytes = Vec::new();
    cmd.encode(&mut bytes);
    let decoded = command::DelveCmd::decode(&bytes).unwrap();
    assert_eq!(decoded, cmd);
    assert_eq!(decoded.held_only().input(), input);
    assert_eq!(decoded.input(), input);
}

fn party() -> Run {
    let mut r = Run::new(4, 1, None);
    let at = r
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    r.open_room_for_test(at);
    r.enter_for_test(at);
    r.calm_for_test();
    r.enemies.clear();
    r.works.perils.clear();
    r.window = None;
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    for (&id, h) in &mut r.players {
        (h.x, h.y) = (10.0 + (id - 1) as f32 * 4.0, 10.0);
        (h.aim_x, h.aim_y) = (if id == 1 { 1.0 } else { -1.0 }, 0.0);
        h.invulnerable = 100_000;
        h.mana = 100.0;
    }
    r
}
fn keys(ids: &[u32]) -> BTreeMap<u32, Input> {
    ids.iter()
        .map(|&id| {
            (
                id,
                Input {
                    team: true,
                    ..Input::default()
                },
            )
        })
        .collect()
}
fn tick(r: &mut Run, ids: &[u32], count: usize) {
    for _ in 0..count {
        r.step(&keys(ids));
    }
}
#[test]
fn tap_launches_at_nearest_friend_and_miss_drops() {
    let mut r = party();
    tick(&mut r, &[1], 1);
    tick(&mut r, &[], 1);
    assert_eq!(r.team.shields.len(), 1);
    assert_eq!(r.team.shields[0].pair, [1, 2]);
    assert_eq!(r.team.shields[0].target, 2);
    tick(&mut r, &[], 30);
    assert!(r.team.shields[0].dropped);
}
#[test]
fn mutual_hold_links_and_drains_both_not_bystanders() {
    let mut r = party();
    tick(&mut r, &[1, 2], 15);
    assert_eq!(r.team.links.len(), 1);
    assert_eq!(r.team.links[0].pair, [1, 2]);
    assert!(r.players[&1].mana < 99.0 && r.players[&2].mana < 99.0);
    assert_eq!(r.players[&3].mana, 100.0);
    tick(&mut r, &[], 1);
    assert!(r.team.links.is_empty());
    assert!(
        r.team.shields.is_empty(),
        "releasing tether must not throw a relay"
    );
}

#[test]
fn catching_grows_combo_and_passes_along_receiver_aim() {
    let mut r = party();
    tick(&mut r, &[1], 1);
    tick(&mut r, &[], 1);
    tick(&mut r, &[], 2);
    tick(&mut r, &[2], 1);
    let shield = &r.team.shields[0];
    assert_eq!(shield.owner, 2);
    assert_eq!(shield.target, 1);
    assert_eq!(shield.combo, 1);
    assert!(shield.vx < 0.0 && shield.speed() > skills::SHIELD_SPEED_L[1]);
    tick(&mut r, &[], 1);
    assert_eq!(
        r.team.shields.len(),
        1,
        "catch release must not start another shield"
    );
}

#[test]
fn relay_cuts_small_foes_but_only_chips_large_foes() {
    let mut r = party();
    r.spawn_at_for_test(EnemyKind::Skeleton, 12.0, 10.0);
    r.spawn_at_for_test(EnemyKind::Demon, 12.5, 10.0);
    let big = r.enemies.last().unwrap().id;
    let hp = r.enemies.last().unwrap().hp;
    tick(&mut r, &[1], 1);
    tick(&mut r, &[], 4);
    assert!(
        r.gore
            .iter()
            .any(|g| g.kind == EnemyKind::Skeleton && g.cut == gore::Cut::Behead)
    );
    let e = r.enemies.iter().find(|e| e.id == big).unwrap();
    assert!(e.hp > 0 && e.hp < hp);
    assert!(e.frozen > 0);
    assert!(
        r.sounds.iter().any(|s| s.contains("shield"))
            || r.gore.iter().any(|g| g.cut == gore::Cut::Behead)
    );
}

#[test]
fn walls_and_exhaustion_snap_links_and_hold_does_not_relink_during_rest() {
    let mut r = party();
    tick(&mut r, &[1, 2], 9);
    r.dungeon.rooms[r.at].set_for_test(6, 5, Tile::Wall);
    tick(&mut r, &[1, 2], 1);
    assert!(r.team.links.is_empty());
    r.dungeon.rooms[r.at].set_for_test(6, 5, Tile::Floor);
    tick(&mut r, &[1, 2], 2);
    assert!(r.team.links.is_empty());
    tick(&mut r, &[1, 2], HZ as usize);
    assert_eq!(r.team.links.len(), 1);
    r.players.get_mut(&2).unwrap().mana = 0.0;
    tick(&mut r, &[1, 2], 1);
    assert!(r.team.links.is_empty());
}

#[test]
fn old_saves_and_published_pair_state_restore() {
    let mut r = party();
    let mut old = serde_json::to_value(&r).unwrap();
    old.as_object_mut().unwrap().remove("team");
    let old: Run = serde_json::from_value(old).unwrap();
    assert!(old.team.links.is_empty() && old.team.shields.is_empty());
    tick(&mut r, &[1, 2], 10);
    let restored = crate::wire::read(&crate::wire::publish(&r).unwrap()).unwrap();
    assert_eq!(restored.team.links[0].pair, [1, 2]);
    let mut clone = r.clone();
    tick(&mut clone, &[1, 2], 10);
    tick(&mut r, &[1, 2], 10);
    assert_eq!(
        serde_json::to_value(&r.team).unwrap(),
        serde_json::to_value(&clone.team).unwrap()
    );
}

#[test]
fn miss_lands_near_receiver_and_third_knight_can_pick_it_up() {
    let mut r = party();
    tick(&mut r, &[1], 1);
    tick(&mut r, &[], 30);
    let shield = &r.team.shields[0];
    assert!(shield.dropped);
    assert!((shield.x - r.players[&2].x).hypot(shield.y - r.players[&2].y) < 4.0);
    let at = (shield.x, shield.y);
    let h = r.players.get_mut(&3).unwrap();
    (h.x, h.y) = at;
    tick(&mut r, &[3], 1);
    assert!(!r.team.shields[0].dropped);
    assert_eq!(r.team.shields[0].owner, 3);
    assert_eq!(r.team.shields[0].pair, [1, 3]);
}

#[test]
fn shield_and_link_do_not_trivialise_a_boss() {
    let mut r = party();
    r.spawn_at_for_test(EnemyKind::Boss, 12.0, 10.0);
    let id = r.enemies.last().unwrap().id;
    let hp = r.enemies.last().unwrap().hp;
    tick(&mut r, &[1, 2], 60);
    let e = r.enemies.iter().find(|e| e.id == id).unwrap();
    assert!(e.hp > hp * 9 / 10);
    assert!(r.gore.iter().all(|g| g.id != id));
}

#[test]
fn taut_crossing_adds_one_bonus_and_out_of_range_breaks() {
    let mut r = party();
    r.spawn_at_for_test(EnemyKind::Demon, 12.0, 10.0);
    tick(&mut r, &[1, 2], 9);
    let id = r.enemies.last().unwrap().id;
    let hp = r.enemies.last().unwrap().hp;
    r.players.get_mut(&2).unwrap().x = 24.0;
    tick(&mut r, &[1, 2], 1);
    assert!(r.team.links[0].taut);
    assert_eq!(hp - r.enemies.iter().find(|e| e.id == id).unwrap().hp, 5);
    r.players.get_mut(&2).unwrap().x = 27.0;
    tick(&mut r, &[1, 2], 1);
    assert!(r.team.links.is_empty());
}

#[test]
#[ignore = "exports team-skill pixel review frames"]
fn write_team_skill_shots() {
    use crate::render::overworld::arena;
    let dir = std::path::PathBuf::from(std::env::var_os("DELVE_TEAM_SHOTS").expect("set DELVE_TEAM_SHOTS to the output directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, r: &Run| {
        let im = arena::seat_frame(r, 640, 360, Some(1));
        image::save_buffer(
            dir.join(name),
            &im.rgba_scaled(2),
            im.w as u32 * 2,
            im.h as u32 * 2,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let mut r = party();
    for (id, (x, y)) in [
        (1, (12.0, 12.0)),
        (2, (22.0, 12.0)),
        (3, (10.0, 19.0)),
        (4, (20.0, 19.0)),
        (5, (10.0, 5.0)),
        (6, (22.0, 5.0)),
    ] {
        let h = r.players.get_mut(&id).unwrap();
        (h.x, h.y) = (x, y);
    }
    save("01-team-ready.png", &r);
    tick(&mut r, &[1], 1);
    tick(&mut r, &[], 5);
    assert_eq!(r.team.shields[0].pair, [1, 2]);
    save("02-shield-relay.png", &r);
    tick(&mut r, &[], 25);
    save("03-shield-dropped.png", &r);
    r.team = team::State::default();
    tick(&mut r, &[1, 2], 12);
    assert_eq!(r.team.links.len(), 1);
    save("04-lightning-link.png", &r);
    r.players.get_mut(&2).unwrap().x = 26.0;
    tick(&mut r, &[1, 2], 1);
    assert!(r.team.links[0].taut);
    save("05-lightning-taut.png", &r);
}

#[test]
fn mutual_holders_can_link_past_a_nearer_unwilling_bystander() {
    let mut r = party();
    r.players.get_mut(&3).unwrap().x = 12.0;
    tick(&mut r, &[1, 2], 10);
    assert_eq!(r.team.links.len(), 1);
    assert_eq!(r.team.links[0].pair, [1, 2]);
}

#[test]
fn link_cuts_minions_and_room_change_clears_party_skills() {
    let mut r = party();
    r.spawn_at_for_test(EnemyKind::Imp, 12.0, 10.0);
    tick(&mut r, &[1, 2], 12);
    assert!(
        r.gore
            .iter()
            .any(|g| g.kind == EnemyKind::Imp && g.cut == gore::Cut::Halve)
    );
    r.at = r
        .dungeon
        .rooms
        .iter()
        .enumerate()
        .find(|(i, room)| *i != r.at && room.kind == RoomKind::Fight)
        .unwrap()
        .0;
    tick(&mut r, &[], 1);
    assert!(r.team.links.is_empty() && r.team.shields.is_empty());
}

#[test]
fn already_relaying_knight_cannot_steal_a_second_shield() {
    let mut r = party();
    for (id, x, y, ax) in [(3, 10.0, 18.0, 1.0), (4, 14.0, 18.0, -1.0)] {
        let h = r.players.get_mut(&id).unwrap();
        (h.x, h.y, h.aim_x, h.aim_y) = (x, y, ax, 0.0);
    }
    tick(&mut r, &[1, 3], 1);
    tick(&mut r, &[], 30);
    assert_eq!(r.team.shields.len(), 2);
    let at = (r.team.shields[0].x, r.team.shields[0].y);
    let h = r.players.get_mut(&3).unwrap();
    (h.x, h.y) = at;
    tick(&mut r, &[3], 1);
    assert!(
        r.team.shields[0].dropped,
        "busy knight must not own two shields"
    );
}

#[test]
fn first_throw_targets_nearest_friend_inside_aim_cone() {
    let mut r = party();
    r.players.get_mut(&2).unwrap().y = 12.0;
    tick(&mut r, &[1], 1);
    tick(&mut r, &[], 1);
    assert_eq!(r.team.shields[0].target, 2);
    assert!(
        r.team.shields[0].vy > 0.0,
        "first throw should fly straight toward chosen teammate"
    );
}

#[test]
fn surface_fight_places_allow_team_skills_but_safe_home_does_not() {
    let mut r = party();
    r.dungeon.depth = 0;
    r.dungeon.rooms[r.at].kind = RoomKind::Yard;
    r.tick_team(&keys(&[1, 2]));
    for _ in 0..10 {
        r.tick += 1;
        r.tick_team(&keys(&[1, 2]));
    }
    assert_eq!(r.team.links.len(), 1);
    r.dungeon.rooms[r.at].kind = RoomKind::Home;
    r.tick_team(&keys(&[1, 2]));
    assert!(r.team.links.is_empty());
}

#[test]
fn active_team_effects_compose_from_scene_parts_for_every_seat() {
    use crate::render::overworld::arena;
    let mut r = party();
    tick(&mut r, &[1, 2], 12);
    assert!(r.valid_snapshot());
    for id in 1..=6 {
        let flat = arena::seat_frame(&r, 384, 264, Some(id));
        let scene = arena::seat_scene(&r, 384, 264, Some(id));
        assert_eq!(flat.rgba_scaled(1), scene.compose().rgba_scaled(1));
    }
}

#[test]
fn side_on_combat_keeps_team_skills_available() {
    let mut r = party();
    r.dungeon.rooms[r.at].kind = RoomKind::Wanted;
    assert!(r.side_on());
    assert!(r.team_available());
}

#[test]
fn repeated_taut_crossings_cannot_spam_bonus_damage() {
    let mut r = party();
    r.spawn_at_for_test(EnemyKind::Demon, 12.0, 10.0);
    tick(&mut r, &[1, 2], 9);
    r.players.get_mut(&2).unwrap().x = 24.0;
    tick(&mut r, &[1, 2], 1);
    let id = r.enemies.last().unwrap().id;
    let hp = r.enemies.last().unwrap().hp;
    r.players.get_mut(&2).unwrap().x = 14.0;
    tick(&mut r, &[1, 2], 1);
    r.players.get_mut(&2).unwrap().x = 24.0;
    tick(&mut r, &[1, 2], 1);
    assert!(hp - r.enemies.iter().find(|e| e.id == id).unwrap().hp <= 2);
}

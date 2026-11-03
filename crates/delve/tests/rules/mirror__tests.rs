use super::*;

fn room() -> Run {
    let mut r = Run::at_home(
        41,
        1,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    r.walk_alehouse();
    r.floor_plates.clear();
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (
        alehouse::MIRROR_STAND.0 * TILE_UNITS,
        alehouse::MIRROR_STAND.1 * TILE_UNITS,
    );
    r
}
fn command(r: &mut Run, n: u8) {
    r.step(&BTreeMap::from([(
        1,
        Input {
            mirror: n,
            ..Default::default()
        },
    )]));
}
fn open(r: &mut Run) {
    for _ in 0..HZ / 2 {
        r.step(&BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }
    assert!(r.players[&1].mirror.open);
}

#[test]
fn mirror_hold_only_opens_its_knight_and_never_fires() {
    let mut r = room();
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(!r.players[&1].mirror.open);
    r.step(&BTreeMap::new());
    open(&mut r);
    let before = (r.players[&1].x, r.players[&1].y);
    let hp = r.players[&1].max_hp;
    r.step(&BTreeMap::from([(
        1,
        Input {
            mirror: 4,
            fire: true,
            move_x: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(before, (r.players[&1].x, r.players[&1].y));
    assert_eq!(r.players[&1].look.body, 1);
    assert_eq!(r.players[&1].max_hp, hp);
    assert!(!r.players[&2].mirror.open);
    command(&mut r, 6);
    assert!(!r.players[&1].mirror.open);
}

#[test]
fn mirror_saves_cosmetics_by_seat_name_and_defaults_old_data() {
    let mut r = room();
    open(&mut r);
    command(&mut r, 4);
    for _ in 0..9 {
        command(&mut r, 2);
    }
    assert!(r.mirror_name(1, "Rowan\n"));
    assert_eq!(r.players[&1].shown_name(), "Rowan");
    assert_ne!(r.players[&1].name, "Rowan");
    let json = serde_json::to_string(&r).unwrap();
    let mut restored: Run = serde_json::from_str(&json).unwrap();
    restored.restore_gear(1);
    assert_eq!(restored.players[&1].look, r.players[&1].look);
    let old: Appearance = serde_json::from_str("{}").unwrap();
    assert_eq!(old, Appearance::default());
    let old: gear::Wardrobe = serde_json::from_str(r#"{"knights":{},"legacy":false}"#).unwrap();
    assert!(old.looks.is_empty());
}

#[test]
fn mirror_rejects_remote_names_and_bad_commands() {
    let mut r = room();
    assert!(!r.mirror_name(2, "Not yours"));
    assert!(
        !Input {
            mirror: 7,
            ..Default::default()
        }
        .valid()
    );
    open(&mut r);
    r.players.get_mut(&1).unwrap().x = 10.0;
    command(&mut r, 4);
    assert!(!r.players[&1].mirror.open);
    assert!(!r.players[&1].look.custom);
}

#[test]
fn mirror_defaults_leave_old_snapshot_fields_out() {
    let r = Run::new(7, 1, None);
    let h = serde_json::to_value(&r.players[&1]).unwrap();
    for field in ["look", "mirror", "mirror_wish"] {
        assert!(h.get(field).is_none(), "{field}");
    }
    let kept = serde_json::to_value(&r.home.wardrobe).unwrap();
    for field in ["looks", "mirror_seen", "mirror_looks"] {
        assert!(kept.get(field).is_none(), "{field}");
    }
}

#[test]
fn mirror_old_tavern_gains_a_walkable_washroom_and_nudges_once() {
    let mut r = room();
    let old = r.room().clone();
    let room = &mut r.dungeon.rooms[r.at];
    room.regrow(36, 20);
    for y in 0..20 {
        for x in 0..36 {
            room.set(
                x,
                y,
                if x == 35 {
                    Tile::Wall
                } else {
                    old.tile(x as i32, y as i32)
                },
            );
        }
    }
    r.step(&BTreeMap::new());
    assert_eq!(r.room().cols, alehouse::HALL.0);
    assert_eq!(r.room().tile(35, 5), Tile::Floor);
    assert_eq!(r.room().tile(39, 4), Tile::Floor);
    assert!(r.players[&1].mirror.nudge > 0);
    let seen = r.home.wardrobe.mirror_seen.clone();
    r.players.get_mut(&1).unwrap().mirror.nudge = 0;
    r.step(&BTreeMap::new());
    assert_eq!(r.players[&1].mirror.nudge, 0);
    assert_eq!(r.home.wardrobe.mirror_seen, seen);
}

#[test]
fn mirror_same_named_knights_keep_independent_looks() {
    let mut r = room();
    let name = r.players[&1].name.clone();
    r.join_seat(2, &name);
    assert_ne!(r.players[&1].mirror_owner(), r.players[&2].mirror_owner());
    for (id, body) in [(1, 1), (2, 2)] {
        r.players.get_mut(&id).unwrap().look.body = body;
        r.keep_look(id);
    }
    r.restore_look(1);
    r.restore_look(2);
    assert_eq!(r.players[&1].look.body, 1);
    assert_eq!(r.players[&2].look.body, 2);
}

use super::super::home::{DESCEND_HOLD, Home};
use super::super::world;
use super::*;
fn run() -> Run {
    let mut r = Run::at_home(17, 1, Some("Student"), Home::default(), Spoils::default());
    r.come_to_the_gate();
    r
}
fn keys(id: u32, i: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, i)])
}
fn tap(r: &mut Run, id: u32) {
    r.step(&BTreeMap::new());
    r.step(&keys(
        id,
        Input {
            interact: true,
            ..Default::default()
        },
    ));
}
fn go(r: &mut Run, kind: RoomKind, at: (f32, f32)) {
    r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
    r.step(&BTreeMap::new());
}
fn save(r: &mut Run) {
    let marks = std::mem::take(&mut r.marks);
    let mut c = r.home.college.clone();
    let mut treasury = r.treasury.clone();
    settle(&mut c, &mut treasury, &marks);
    let mut home = r.home.clone();
    home.college = c;
    r.rebuild_home(home, treasury);
}
fn accept(r: &mut Run, id: &str) {
    r.marks.insert(format!("college:accept:{id}"), 1);
    save(r);
}
#[test]
fn college_prompt_target_is_the_action_even_when_two_targets_are_in_reach() {
    let mut r = run();
    accept(&mut r, "sigils");
    r.home.college.familiar = true;
    go(&mut r, RoomKind::WizardStudy, (18.8, 9.3));
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (18.8 * TILE_UNITS, 9.3 * TILE_UNITS);
    assert_eq!(
        nearest(&r, &r.players[&1]).unwrap().0,
        Interaction::Familiar
    );
    tap(&mut r, 1);
    assert!(r.marks.contains_key("college:familiar"));
    assert!(!r.marks.contains_key("college:sigil:2"));
    go(&mut r, RoomKind::WizardStudy, SIGILS[2].1);
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (18.0 * TILE_UNITS, 8.5 * TILE_UNITS);
    assert_eq!(
        nearest(&r, &r.players[&1]).unwrap().0,
        Interaction::Sigil(2)
    );
    tap(&mut r, 1);
    assert!(r.marks.contains_key("college:sigil:2"));
    r.players.get_mut(&1).unwrap().stone = true;
    assert!(nearest(&r, &r.players[&1]).is_none());
}
#[test]
fn college_doors_connect_every_room_and_return_the_whole_company_to_the_gate() {
    let mut r = run();
    r.join_seat(3, "Scholar");
    r.join_seat(4, "Librarian");
    assert!(r.valid_snapshot());
    assert!(r.dungeon.rooms.len() <= crate::rules::world::HOME_ROOM_LIMIT);
    let mut visited = std::collections::HashSet::new();
    let mut todo = vec![RoomKind::Gate];
    while let Some(kind) = todo.pop() {
        if !visited.insert(kind) {
            continue;
        }
        for e in DOORS.iter().filter(|e| e.room == kind) {
            if let Dest::Room(next, _) = e.to {
                todo.push(next);
            }
        }
    }
    for kind in ROOMS {
        assert!(visited.contains(&kind), "{kind:?} is connected");
    }
    for e in &DOORS {
        go(&mut r, e.room, (12.0, 10.5));
        let (x, y) = world::entrance_centre(e);
        // A release tick off the stair arms the travel ring.
        for h in r.players.values_mut() {
            (h.x, h.y) = (x, y);
        }
        for _ in 0..DESCEND_HOLD + 3 {
            r.step(&BTreeMap::new());
            if r.room().kind != e.room {
                break;
            }
        }
        let Dest::Room(kind, _) = e.to else {
            unreachable!()
        };
        assert_eq!(r.room().kind, kind, "{}", e.id);
        for h in r.players.values() {
            assert!(
                matches!(
                    r.room().tile((h.x / 2.0) as i32, (h.y / 2.0) as i32),
                    Tile::Floor | Tile::Stairs | Tile::Door
                ),
                "{} arrival for {}",
                e.id,
                h.name
            );
        }
    }
    assert!(
        !world::raise_world(&mut r.dungeon),
        "migration is idempotent"
    );
}
#[test]
fn college_every_entrance_and_interaction_is_reachable_around_real_furniture() {
    let r = run();
    for kind in ROOMS {
        let room = &r.dungeon.rooms[world::room_of(&r.dungeon, kind).unwrap()];
        let mut seen = BTreeSet::new();
        let mut open = vec![(12, 10)];
        while let Some((x, y)) = open.pop() {
            if x < 0
                || y < 0
                || x >= room.cols as i32
                || y >= room.rows as i32
                || matches!(room.tile(x, y), Tile::Wall | Tile::Block | Tile::Hazard)
                || !seen.insert((x, y))
            {
                continue;
            }
            open.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
        for e in DOORS.iter().filter(|e| e.room == kind) {
            assert!(seen.contains(&(e.at.0, e.at.1)), "{}", e.id);
        }
        let points: Vec<_> = match kind {
            RoomKind::WizardStudy => vec![WIZARD, LECTERN, FAMILIAR, SIGILS[2].1],
            RoomKind::TowerEntry => vec![BOOKSHELF],
            RoomKind::TowerStars => vec![STAR_MAP, SIGILS[1].1],
            _ => vec![],
        };
        for (x, y) in points {
            assert!(seen.contains(&(x as i32, y as i32)), "{kind:?} {x},{y}");
        }
    }
}
#[test]
fn college_school_discoveries_are_reachable_interactive_and_saved() {
    let mut r = run();
    for (i, &(kind, at, key, note)) in CLUES.iter().enumerate() {
        go(&mut r, kind, at);
        assert_eq!(r.room().tile(at.0 as i32, at.1 as i32), Tile::Floor);
        tap(&mut r, 1);
        assert!(r.marks.contains_key(key));
        assert_eq!(r.found.as_ref().unwrap().2, note);
        save(&mut r);
        assert_eq!(r.home.college.clues, (1 << (i + 1)) - 1);
    }
    let restored: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
    assert!(restored.valid_snapshot());
    assert_eq!(restored.home.college.clues, 15);
}

#[test]
fn college_interact_opens_short_topics_and_accepts_a_research_mission() {
    let mut r = run();
    go(&mut r, RoomKind::WizardStudy, WIZARD);
    tap(&mut r, 1);
    assert_eq!(r.college.page, Page::Menu);
    // A held E cannot consume the newly opened menu.
    for _ in 0..20 {
        r.step(&keys(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        ));
    }
    assert_eq!(r.college.page, Page::Menu);
    tap(&mut r, 1);
    assert_eq!(r.college.page, Page::Topics);
    tap(&mut r, 1);
    assert_eq!(r.college.page, Page::Topic(0));
    assert!(TOPICS[0].1[0].contains("Bartosz"));
    r.step(&keys(
        1,
        Input {
            aim_x: -1,
            ..Default::default()
        },
    ));
    r.step(&keys(
        1,
        Input {
            aim_y: 1,
            ..Default::default()
        },
    ));
    tap(&mut r, 1);
    assert_eq!(r.college.page, Page::Quests);
    tap(&mut r, 1);
    tap(&mut r, 1);
    assert!(r.marks.contains_key("college:accept:map"));
    save(&mut r);
    assert!(r.home.college.active("map"));
    assert_eq!(r.wire_copy().college, r.college);
    r.players.get_mut(&1).unwrap().stone = true;
    r.step(&BTreeMap::new());
    assert_eq!(r.college.page, Page::Closed);
}
#[test]
fn college_all_quests_progress_hand_in_and_pay_fixed_rewards_once() {
    let mut r = run();
    for q in &QUESTS {
        accept(&mut r, q.id);
    }
    for n in 0..4 {
        r.marks.insert(format!("college:map:1:3:{n}:0"), 1);
    }
    for n in 0..3 {
        r.marks.insert(format!("college:sample:1:3:{n}:0"), 1);
        r.marks
            .insert(format!("college:observe:{}", OBSERVABLE[n]), 1);
        r.marks.insert(format!("college:sigil:{n}"), 1);
    }
    for m in ["college:tome", "college:together"] {
        r.marks.insert(m.into(), 1);
    }
    save(&mut r);
    assert!(r.home.college.done.is_empty(), "must return to Bartosz");
    for (i, q) in QUESTS.iter().enumerate() {
        go(&mut r, RoomKind::WizardStudy, WIZARD);
        r.college.owner = Some(1);
        r.college.page = Page::Quest(i as u8);
        tap(&mut r, 1);
        assert!(r.marks.contains_key(&format!("college:turn-in:{}", q.id)));
        save(&mut r);
        assert!(r.home.college.done.contains(q.id));
    }
    assert_eq!(r.treasury.get(Spoil::Gold), 325);
    assert_eq!(r.treasury.get(Spoil::Bond), 1);
    assert!(
        r.players
            .values()
            .all(|h| h.spells.iter().flatten().any(|s| s == "hedge-maze"))
    );
    let paid = r.treasury.clone();
    let all = BTreeMap::from_iter(QUESTS.iter().flat_map(|q| {
        [
            (format!("college:accept:{}", q.id), 1),
            (format!("college:turn-in:{}", q.id), 1),
        ]
    }));
    settle(&mut r.home.college, &mut r.treasury, &all);
    assert_eq!(r.treasury, paid);
    assert!(r.home.college.valid());
}
fn field(r: &mut Run, pack: Pack, kind: RoomKind, cell: (i32, i32)) {
    let mut room = super::room(kind, cell);
    room.cleared = true;
    r.dungeon.depth = 3;
    r.dungeon.pack = pack;
    r.dungeon.rooms = vec![room];
    r.at = 0;
    r.enemies.clear();
    r.phase = Phase::Exploring;
    r.college.close();
    let at = field_spot(r.room()).unwrap();
    for h in r.players.values_mut() {
        h.hp = h.max_hp;
        h.stone = false;
        (h.x, h.y) = (at.0 * 2.0, at.1 * 2.0);
    }
}
#[test]
fn college_fieldwork_observes_living_creatures_and_collects_distinct_patches() {
    let mut r = run();
    for id in ["map", "creatures", "tome", "spores"] {
        accept(&mut r, id);
    }
    field(&mut r, Pack::Archive, RoomKind::Treasure, (0, 0));
    tap(&mut r, 1);
    assert!(r.marks.contains_key("college:tome"));
    for (kind, name) in [
        (EnemyKind::Bat, "bat"),
        (EnemyKind::Skeleton, "skeleton"),
        (EnemyKind::Slime, "slime"),
    ] {
        r.enemies.clear();
        let h = &r.players[&1];
        r.spawn_at_for_test(kind, h.x + 2.0, h.y);
        tap(&mut r, 1);
        assert!(r.marks.contains_key(&format!("college:observe:{name}")));
    }
    for n in 0..3 {
        field(&mut r, Pack::Fungal, RoomKind::Fight, (n, 0));
        tap(&mut r, 1);
        let c = r.college_progress();
        assert_eq!(c.samples.len(), n as usize + 1);
        tap(&mut r, 1);
        assert_eq!(r.college_progress().samples.len(), n as usize + 1);
    }
    assert_eq!(
        r.college_progress().maps.len(),
        4,
        "different packs have distinct rooms; repeated visits still count once"
    );
    let marks = r.marks.clone();
    let mut c = r.home.college.clone();
    settle(&mut c, &mut r.treasury, &marks);
    settle(&mut c, &mut r.treasury, &marks);
    assert_eq!(c.observed.len(), 3);
    assert!(c.tome);
}
#[test]
fn college_study_together_needs_two_present_living_knights_at_the_field_marker() {
    let mut r = run();
    accept(&mut r, "together");
    field(&mut r, Pack::Cavern, RoomKind::Fight, (0, 0));
    r.players.remove(&2);
    tap(&mut r, 1);
    assert!(!r.marks.contains_key("college:together"));
    r.join_seat(2, "Partner");
    r.players.get_mut(&2).unwrap().stone = true;
    tap(&mut r, 1);
    assert!(!r.marks.contains_key("college:together"));
    let h = r.players.get_mut(&2).unwrap();
    h.stone = false;
    h.hp = 0;
    tap(&mut r, 1);
    assert!(!r.marks.contains_key("college:together"));
    let h = r.players.get_mut(&2).unwrap();
    h.hp = h.max_hp;
    h.x = 4.0;
    h.y = 4.0;
    tap(&mut r, 1);
    assert!(!r.marks.contains_key("college:together"));
    let (x, y) = (r.players[&1].x, r.players[&1].y);
    let h = r.players.get_mut(&2).unwrap();
    (h.x, h.y) = (x + 2.0, y);
    tap(&mut r, 1);
    assert!(r.marks.contains_key("college:together"));
}
#[test]
fn college_tower_puzzles_reveal_all_three_hidden_sigils() {
    let mut r = run();
    accept(&mut r, "sigils");
    for (i, (kind, puzzle)) in [
        (RoomKind::TowerEntry, BOOKSHELF),
        (RoomKind::TowerStars, STAR_MAP),
        (RoomKind::WizardStudy, FAMILIAR),
    ]
    .into_iter()
    .enumerate()
    {
        go(&mut r, kind, puzzle);
        tap(&mut r, 1);
        r.step(&BTreeMap::new());
        let at = SIGILS[i].1;
        // The shelf really moves its collision, rather than just its drawing.
        if i == 0 {
            assert_eq!(r.room().tile(5, 4), Tile::Floor);
        }
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y) = (at.0 * 2.0, at.1 * 2.0);
        tap(&mut r, 1);
        assert!(r.marks.contains_key(&format!("college:sigil:{i}")));
    }
    save(&mut r);
    assert_eq!(r.home.college.sigils, 7);
}
#[test]
fn college_save_round_trip_defaults_and_mirrored_dialogue_preserve_progress() {
    let old: Home = serde_json::from_str("{}").unwrap();
    assert_eq!(old.college, College::default());
    let mut r = run();
    accept(&mut r, "sigils");
    r.marks.insert("college:bookshelf".into(), 1);
    save(&mut r);
    go(&mut r, RoomKind::WizardStudy, WIZARD);
    tap(&mut r, 1);
    let bytes = serde_json::to_vec(&r).unwrap();
    let restored: Run = serde_json::from_slice(&bytes).unwrap();
    assert!(restored.valid_snapshot());
    assert_eq!(restored.home.college, r.home.college);
    assert_eq!(restored.college, r.college);
    let mut old_floor = restored.dungeon.clone();
    old_floor.rooms.retain(|room| !is_room(room.kind));
    assert!(world::raise_world(&mut old_floor));
    for kind in ROOMS {
        assert!(world::room_of(&old_floor, kind).is_some());
    }
    assert_eq!(r.wire_copy().college, r.college);
}

#[test]
fn college_migrates_a_full_old_home_without_losing_any_rooms() {
    let mut r = run();
    r.dungeon.rooms.retain(|room| !is_room(room.kind));
    while r.dungeon.rooms.len() < 64 {
        let n = r.dungeon.rooms.len() as i32;
        r.dungeon.rooms.push(Room::solid_rock((n + 20, 20)));
    }
    let old: Vec<_> = r.dungeon.rooms.iter().map(|room| room.cell).collect();
    assert!(world::raise_world(&mut r.dungeon));
    assert_eq!(r.dungeon.rooms.len(), 73);
    for cell in old {
        assert!(r.dungeon.rooms.iter().any(|room| room.cell == cell));
    }
    for kind in ROOMS {
        assert!(world::room_of(&r.dungeon, kind).is_some());
    }
    assert!(r.valid_snapshot());
    r.dungeon.depth = 1;
    assert!(
        !r.valid_snapshot(),
        "the underground room bound is still 64"
    );
}

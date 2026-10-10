use super::*;
use crate::drive::together_realm::{Spoil, Spoils};
use crate::drive::together_shooter::home::{BUY_HOLD, Home};
use crate::drive::together_shooter::world;

fn rich() -> Spoils {
    let mut s = Spoils::default();
    for (spoil, n) in [
        (Spoil::Gold, 9000),
        (Spoil::Ore, 200),
        (Spoil::Bone, 100),
        (Spoil::Gem, 40),
        (Spoil::Ember, 40),
    ] {
        s.add(spoil, n);
    }
    s
}

fn marks(list: &[(&str, u32)]) -> BTreeMap<String, u32> {
    list.iter().map(|&(m, n)| (m.to_string(), n)).collect()
}

fn who(id: u32) -> String {
    format!("Knight {id}")
}

/// A home whose King has come and been met.
fn met() -> Home {
    let mut home = Home::default();
    let mut t = Spoils::default();
    settle(&mut home, &mut t, 1, &marks(&[("barony:met", 1)]), &who);
    assert_eq!(home.barony.court, Court::Met);
    home
}

fn said(out: &Settled, cue: &str) -> bool {
    out.said.iter().any(|(c, _)| c.starts_with(cue))
}

#[test]
fn the_king_comes_once_a_floor_of_the_mines_is_cleared() {
    let mut home = Home::default();
    let mut t = Spoils::default();
    let out = settle(&mut home, &mut t, 0, &BTreeMap::new(), &who);
    assert_eq!(home.barony.court, Court::Absent);
    assert!(!out.changed);
    let out = settle(&mut home, &mut t, 1, &BTreeMap::new(), &who);
    assert_eq!(home.barony.court, Court::Camped);
    assert!(said(&out, "brannoc_arrives"));
    // A realm already below the Mines (an older save) meets him at once.
    let mut old = Home {
        deepest: 3,
        ..Home::default()
    };
    settle(&mut old, &mut t, 0, &BTreeMap::new(), &who);
    assert_eq!(old.barony.court, Court::Camped);
}

#[test]
fn meeting_him_brings_his_first_mission() {
    let home = met();
    let pinned = home.barony.mission.as_ref().unwrap();
    assert_eq!(pinned.id, "workings");
    assert_eq!(pinned.have, 0);
}

#[test]
fn clearing_the_mines_halls_wins_the_upper_workings_and_his_tribute() {
    let mut home = met();
    let mut t = Spoils::default();
    settle(
        &mut home,
        &mut t,
        1,
        &marks(&[("clear:Cavern", 5), ("clear:Crypt", 9)]),
        &who,
    );
    assert_eq!(
        home.barony.mission.as_ref().unwrap().have,
        5,
        "only the Mines count"
    );
    assert!(
        pay_tribute(&mut home, &mut t, 2).is_none(),
        "no tribute yet"
    );
    let out = settle(&mut home, &mut t, 1, &marks(&[("clear:Cavern", 4)]), &who);
    assert!(said(&out, "mission_done:workings"));
    assert!(home.barony.holds_workings());
    assert_eq!(t.get(Spoil::Gold), 150);
    assert_eq!(t.get(Spoil::Ore), 10);
    assert_eq!(home.barony.ledger.paid_back.get(Spoil::Gold), 150);
    assert_eq!(crew(&home.barony), 4, "a dwarf home from the workings");
    assert_eq!(home.barony.miners(), 3);
    // The First Fire waits for the Ore-Forge to burn.
    assert!(home.barony.mission.is_none());
    let paid = pay_tribute(&mut home, &mut t, 2).unwrap();
    assert_eq!(paid.get(Spoil::Ore), 6);
    assert_eq!(paid.get(Spoil::Gold), 60);
    assert_eq!(
        home.barony.ledger.lines.last().unwrap().what,
        "Tribute: 2 floors cleared"
    );
}

#[test]
fn the_kings_hall_costs_a_great_deal_and_is_built_over_time() {
    let mut home = met();
    let mut poor = Spoils::default();
    poor.add(Spoil::Gold, 2900);
    let out = settle(
        &mut home,
        &mut poor,
        1,
        &marks(&[("barony:order:hall:1", 1)]),
        &who,
    );
    assert!(
        out.said
            .iter()
            .any(|(c, l)| c == "cant_afford" && l.contains("need"))
    );
    assert!(home.barony.works.is_empty());
    let mut t = rich();
    let out = settle(
        &mut home,
        &mut t,
        1,
        &marks(&[("barony:order:hall:1", 1)]),
        &who,
    );
    assert!(said(&out, "commissioned:hall"));
    assert_eq!(t.get(Spoil::Gold), 9000 - 3000);
    assert_eq!(home.barony.ledger.paid_in.get(Spoil::Gold), 3000);
    assert!(home.barony.ledger.lines[0].what.contains("Knight 1"));
    // Not twice.
    let again = settle(
        &mut home,
        &mut t,
        1,
        &marks(&[("barony:order:hall:1", 1)]),
        &who,
    );
    assert!(
        again
            .said
            .iter()
            .any(|(_, l)| l.contains("paid for already"))
    );
    assert_eq!(t.get(Spoil::Gold), 6000);
    // Three dwarves for half an hour of realm time: ten-second shifts.
    let def = work("hall").unwrap();
    let shifts = def.labour / (SHIFT_SECS * crew(&home.barony));
    assert_eq!(shifts, 180, "half an hour, in ten-second shifts");
    let mut stages = 0;
    for _ in 0..shifts - 1 {
        let out = settle(
            &mut home,
            &mut t,
            1,
            &marks(&[("barony:labour", SHIFT_SECS)]),
            &who,
        );
        stages += out
            .said
            .iter()
            .filter(|(c, _)| c == "work_stage:hall")
            .count();
    }
    assert_eq!(stages, 3, "a word at each quarter");
    assert!(!home.barony.built("hall"));
    assert_eq!(home.barony.court, Court::Met);
    let out = settle(
        &mut home,
        &mut t,
        1,
        &marks(&[("barony:labour", SHIFT_SECS)]),
        &who,
    );
    assert!(home.barony.built("hall"));
    assert_eq!(home.barony.court, Court::Sworn);
    assert!(out.earned.contains(&"fealty"));
    assert!(said(&out, "brannoc_sworn"));
}

#[test]
fn the_ore_forge_is_found_rebuilt_relit_and_then_works() {
    let mut home = met();
    let mut t = rich();
    let order = marks(&[("barony:order:ore-forge:2", 1)]);
    let early = settle(&mut home, &mut t, 1, &order, &who);
    assert!(
        early
            .said
            .iter()
            .any(|(_, l)| l.contains("The Upper Workings"))
    );
    assert!(home.barony.labour("ore-forge").is_none());
    settle(&mut home, &mut t, 1, &marks(&[("clear:Cavern", 8)]), &who);
    settle(&mut home, &mut t, 1, &order, &who);
    assert_eq!(home.barony.labour("ore-forge"), Some(0));
    // Fire before it stands does nothing.
    let relight = marks(&[("barony:relight:ore-forge:1", 1)]);
    settle(&mut home, &mut t, 1, &relight, &who);
    assert!(!home.barony.is_lit("ore-forge"));
    let def = work("ore-forge").unwrap();
    let shift = SHIFT_SECS * crew(&home.barony);
    for _ in 0..def.labour.div_ceil(shift) {
        settle(
            &mut home,
            &mut t,
            1,
            &marks(&[("barony:labour", SHIFT_SECS)]),
            &who,
        );
    }
    assert!(home.barony.built("ore-forge"));
    let embers = t.get(Spoil::Ember);
    let out = settle(&mut home, &mut t, 1, &relight, &who);
    assert!(home.barony.is_lit("ore-forge"));
    assert_eq!(t.get(Spoil::Ember), embers - 8, "fire from Dragon Keep");
    assert!(out.earned.contains(&"hammers_ring"));
    assert!(said(&out, "forge_relit:ore-forge"));
    // Burning: more miners, a richer tribute, and the First Fire asked for.
    assert_eq!(home.barony.miners(), 5);
    let paid = pay_tribute(&mut home, &mut t, 1).unwrap();
    assert_eq!(
        (
            paid.get(Spoil::Ore),
            paid.get(Spoil::Gem),
            paid.get(Spoil::Gold)
        ),
        (3, 1, 90)
    );
    assert_eq!(home.barony.mission.as_ref().unwrap().id, "first-fire");
    settle(
        &mut home,
        &mut t,
        1,
        &marks(&[("trophy:the-foreman", 1)]),
        &who,
    );
    assert!(!home.barony.has_done("first-fire"));
    let gems = t.get(Spoil::Gem);
    settle(
        &mut home,
        &mut t,
        1,
        &marks(&[("trophy:cinderjaw", 1)]),
        &who,
    );
    assert!(home.barony.has_done("first-fire"));
    assert_eq!(t.get(Spoil::Gem), gems + 4);
}

#[test]
fn the_crew_builds_one_work_at_a_time_in_the_order_paid() {
    let mut home = met();
    let mut t = rich();
    settle(&mut home, &mut t, 1, &marks(&[("clear:Cavern", 8)]), &who);
    settle(
        &mut home,
        &mut t,
        1,
        &marks(&[("barony:order:ore-forge:1", 1), ("barony:order:hall:1", 1)]),
        &who,
    );
    // The marks come in name order: the hall, then the forge.
    assert_eq!(home.barony.building().unwrap().id, "hall");
    settle(
        &mut home,
        &mut t,
        1,
        &marks(&[("barony:labour", SHIFT_SECS)]),
        &who,
    );
    assert!(home.barony.labour("hall").unwrap() > 0);
    assert_eq!(home.barony.labour("ore-forge"), Some(0));
}

#[test]
fn the_ledger_keeps_its_last_lines_and_every_total() {
    let mut b = Barony::default();
    let mut gold = Spoils::default();
    gold.add(Spoil::Gold, 10);
    for k in 0..20 {
        b.ledger.write(format!("line {k}"), &gold, k % 2 == 0);
    }
    assert_eq!(b.ledger.lines.len(), LEDGER_LINES);
    assert_eq!(b.ledger.lines.last().unwrap().what, "line 19");
    assert_eq!(b.ledger.paid_in.get(Spoil::Gold), 100);
    assert_eq!(b.ledger.paid_back.get(Spoil::Gold), 100);
}

#[test]
fn an_older_realm_loads_without_a_barony_and_a_bad_one_resets_alone() {
    use crate::drive::together_realm::Realm;
    let dir = std::env::temp_dir().join(format!("barony-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rewards = dir.join("island.json");
    let path = rewards.with_extension("realm.json");
    std::fs::write(
        &path,
        r#"{"treasury":{"gold":5},"home":{"deepest":3,"levels":{"forge":2}}}"#,
    )
    .unwrap();
    let realm = Realm::beside(Some(&rewards));
    assert_eq!(realm.home.barony, Barony::default());
    assert_eq!(realm.home.level(home::Station::Forge), 2);
    std::fs::write(
        &path,
        r#"{"treasury":{"gold":5},"home":{"deepest":3,"barony":{"court":"emperor","works":7}}}"#,
    )
    .unwrap();
    let realm = Realm::beside(Some(&rewards));
    assert_eq!(realm.treasury.get(Spoil::Gold), 5, "the realm kept");
    assert_eq!(realm.home.barony, Barony::default());
    // And a good one goes round whole.
    let mut realm = realm;
    let mut t = rich();
    settle(
        &mut realm.home,
        &mut t,
        1,
        &marks(&[("barony:met", 1)]),
        &who,
    );
    settle(
        &mut realm.home,
        &mut t,
        1,
        &marks(&[("barony:order:hall:1", 1)]),
        &who,
    );
    realm.save().unwrap();
    let again = Realm::beside(Some(&rewards));
    assert_eq!(again.home.barony, realm.home.barony);
    // Station keys in the saved home are only the old ones: an older build
    // reading this realm loses the barony, never the realm.
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(
        raw["home"]["levels"]
            .as_object()
            .is_none_or(|l| l.is_empty())
    );
    std::fs::remove_dir_all(dir).unwrap();
}

// ── The run's half ───────────────────────────────────────────────────────

fn at(kind: RoomKind, home: Home, at: (f32, f32)) -> Run {
    let mut run = Run::at_home(7, 1, None, home, rich());
    let i = world::room_of(&run.dungeon, kind).unwrap();
    run.arrive(i, at);
    run.step(&BTreeMap::new());
    run
}

fn hold(run: &mut Run, plate: (i32, i32, i32, i32)) {
    let (c, r, w, h) = plate;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Input::default()
        },
    )]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&fire);
    }
    run.step(&BTreeMap::new());
}

#[test]
fn walking_up_to_the_camped_king_meets_him_once() {
    let mut home = Home::default();
    home.barony.court = Court::Camped;
    let mut run = at(RoomKind::MineHead, home, (12.0, 11.0));
    run.marks.clear();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (
        CAMP_KING.0 * TILE_UNITS + 2.0,
        CAMP_KING.1 * TILE_UNITS + 2.0,
    );
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.marks.get("barony:met"), Some(&1));
    assert_eq!(
        run.cues.iter().filter(|c| *c == "npc:brannoc_meet").count(),
        1
    );
}

#[test]
fn the_ruins_plate_pays_for_the_hall_once_he_is_met() {
    let mut camped = Home::default();
    camped.barony.court = Court::Camped;
    let mut run = at(RoomKind::MineHead, camped, (12.0, 11.0));
    run.marks.clear();
    hold(&mut run, HALL_PLATE);
    assert!(!run.marks.keys().any(|m| m.starts_with("barony:order")));
    let mut run = at(RoomKind::MineHead, met(), (12.0, 11.0));
    run.marks.clear();
    hold(&mut run, HALL_PLATE);
    assert!(run.marks.contains_key("barony:order:hall:1"));
}

#[test]
fn realm_time_marks_labour_while_a_work_goes_up_wherever_the_party_is() {
    let mut home = met();
    home.barony.works.push(Work {
        id: "hall".into(),
        labour: 0,
    });
    let mut run = at(RoomKind::Gate, home, (12.0, 10.0));
    run.marks.clear();
    for _ in 0..SHIFT_SECS * HZ * 3 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.marks.get("barony:labour"), Some(&(SHIFT_SECS * 3)));
}

#[test]
fn the_great_door_opens_only_once_the_hall_stands() {
    let mut home = met();
    home.barony.works.push(Work {
        id: "hall".into(),
        labour: 100,
    });
    let mut run = at(RoomKind::MineHead, home.clone(), (12.0, 11.0));
    let door = world::ENTRANCES.iter().find(|e| e.id == "hall").unwrap();
    let (x, y) = world::entrance_centre(door);
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (x, y);
    }
    for _ in 0..home::DESCEND_HOLD + 2 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(
        run.room().kind,
        RoomKind::MineHead,
        "a ruin's door is rubble"
    );
    home.barony.works[0].labour = work("hall").unwrap().labour;
    home.barony.court = Court::Sworn;
    let mut run = at(RoomKind::MineHead, home, (12.0, 11.0));
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (x, y);
    }
    for _ in 0..home::DESCEND_HOLD + 2 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.room().kind, RoomKind::KingsHall);
}

#[test]
fn a_floor_of_the_mines_has_its_forge_hall_once_the_workings_are_his() {
    let mut home = met();
    let mut run = Run::at_home(7, 1, None, home.clone(), Default::default());
    run.dungeon.pack = Pack::Cavern;
    run.descend_for_test();
    assert_eq!(run.dungeon.pack, Pack::Cavern);
    assert!(!run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Forge));
    home.barony.done.push("workings".into());
    for seed in 0..12 {
        let mut run = Run::at_home(seed, 1, None, home.clone(), Default::default());
        run.dungeon.pack = Pack::Cavern;
        run.descend_for_test();
        let forge = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Forge)
            .expect("a forge-hall");
        let room = &run.dungeon.rooms[forge];
        let door = (0..4).find(|&d| room.doors[d]).unwrap();
        let host = run
            .dungeon
            .neighbour(forge, door)
            .expect("joined to a hall");
        assert!(run.dungeon.rooms[host].doors[(door + 2) % 4]);
        assert!(run.valid_snapshot(), "seed {seed}");
        // The floor's politics are drawn as ever, the forge-hall with them.
        assert!(run.boss_gate_line().is_some(), "seed {seed}");
        // And a Crypt floor never has one.
        run.descend_for_test();
        assert!(!run.dungeon.rooms.iter().any(|r| r.kind == RoomKind::Forge));
    }
}

#[test]
fn the_forge_plate_pays_then_relights() {
    let mut home = met();
    home.barony.done.push("workings".into());
    let mut run = Run::at_home(7, 1, None, home.clone(), rich());
    run.dungeon.pack = Pack::Cavern;
    run.descend_for_test();
    let forge = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Forge)
        .unwrap();
    run.enter_for_test(forge);
    run.marks.clear();
    hold(&mut run, FORGE_PLATE);
    assert!(run.marks.contains_key("barony:order:ore-forge:1"));
    home.barony.works.push(Work {
        id: "ore-forge".into(),
        labour: work("ore-forge").unwrap().labour,
    });
    run.rebuild_home(home, rich());
    run.marks.clear();
    hold(&mut run, FORGE_PLATE);
    assert!(run.marks.contains_key("barony:relight:ore-forge:1"));
}

#[test]
fn a_lit_ore_forge_mails_every_knight() {
    let mut home = Home::default();
    home.barony.lit.push("ore-forge".into());
    let run = Run::at_home(7, 1, Some("Friend"), home, Default::default());
    for hero in run.players.values() {
        assert!(hero.deck.iter().any(|c| c == "home-mail"));
        assert_eq!(hero.armor, FORGED_MAIL);
    }
}

use super::*;
use crate::rules::barony;
use crate::rules::home::Home;
use crate::rules::world;

/// A home whose King has been met.
fn met() -> Home {
    let mut home = Home::default();
    home.barony.court = Court::Met;
    home
}

fn marks(list: &[&str]) -> BTreeMap<String, u32> {
    list.iter().map(|m| (m.to_string(), 1)).collect()
}

fn gold(n: u32) -> Spoils {
    let mut s = Spoils::default();
    s.add(Spoil::Gold, n);
    s
}

fn settle_into(home: &mut Home, treasury: &mut Spoils, list: &[&str]) -> Settled {
    let mut realm = Realm {
        home,
        treasury,
        reclaimed: &BTreeMap::new(),
        raids_won: 0,
    };
    settle(&mut realm, &marks(list))
}

fn said(out: &Settled, cue: &str) -> bool {
    out.said.iter().any(|(c, _)| c.starts_with(cue))
}

// ── The realm's half ─────────────────────────────────────────────────────

#[test]
fn the_king_sends_the_party_to_one_hall_at_a_time_in_order() {
    let mut home = met();
    let mut t = Spoils::default();
    // The second hall is not his to give before the first is won.
    let out = settle_into(&mut home, &mut t, &["reclaim:ask:1"]);
    assert!(!out.changed);
    assert_eq!(home.barony.halls.asked, None);
    let out = settle_into(&mut home, &mut t, &["reclaim:ask:0"]);
    assert!(out.changed && said(&out, "hall_asked:gallery"));
    assert_eq!(home.barony.halls.asked, Some(0));
    // And not two at once.
    let out = settle_into(&mut home, &mut t, &["reclaim:ask:1"]);
    assert!(!out.changed);
    assert_eq!(home.barony.halls.asked, Some(0));
}

#[test]
fn a_king_not_yet_met_sends_nobody() {
    let mut home = Home::default();
    home.barony.court = Court::Camped;
    let mut t = Spoils::default();
    settle_into(&mut home, &mut t, &["reclaim:ask:0"]);
    assert_eq!(home.barony.halls.asked, None);
}

#[test]
fn nubbins_toll_is_paid_from_the_treasury_and_he_stays_a_tenant() {
    let mut home = met();
    home.barony.halls.asked = Some(0);
    let mut poor = gold(TOLL - 1);
    let out = settle_into(&mut home, &mut poor, &["reclaim:toll:0"]);
    assert!(out.said.iter().any(|(c, l)| c == "cant_afford" && l.contains("need")));
    assert!(!home.barony.halls.is_held("gallery"));
    assert_eq!(poor.get(Spoil::Gold), TOLL - 1, "a short purse pays nothing");
    let mut t = gold(1000);
    let out = settle_into(&mut home, &mut t, &["reclaim:toll:0"]);
    assert!(said(&out, "hall_won:gallery"));
    assert_eq!(t.get(Spoil::Gold), 1000 - TOLL);
    assert_eq!(home.barony.halls.how("gallery"), Some("bargain"));
    assert_eq!(home.barony.halls.asked, None);
    assert_eq!(home.barony.ledger.paid_in.get(Spoil::Gold), TOLL);
    // The tenants pay ore as well as gold; no thanks for a hall bought.
    let rent = home.barony.halls.rent();
    assert_eq!((rent.get(Spoil::Gold), rent.get(Spoil::Ore)), (25, 2));
    assert_eq!(home.barony.ledger.paid_back.get(Spoil::Gold), 0);
    // Not twice.
    let before = t.clone();
    settle_into(&mut home, &mut t, &["reclaim:toll:0"]);
    assert_eq!(t, before);
}

#[test]
fn a_hall_won_by_fight_pays_his_thanks_once_and_rent_without_ore() {
    let mut home = met();
    home.barony.halls.asked = Some(0);
    let mut t = Spoils::default();
    let out = settle_into(&mut home, &mut t, &["reclaim:won:0:fight"]);
    assert!(said(&out, "hall_won:gallery"));
    assert_eq!(t.get(Spoil::Gold), 250);
    assert_eq!(home.barony.ledger.paid_back.get(Spoil::Gold), 250);
    let rent = home.barony.halls.rent();
    assert_eq!((rent.get(Spoil::Gold), rent.get(Spoil::Ore)), (25, 0));
    let before = t.clone();
    settle_into(&mut home, &mut t, &["reclaim:won:0:fight"]);
    assert_eq!(t, before, "his thanks come once");
}

#[test]
fn a_hall_can_only_be_won_the_ways_it_has() {
    let mut home = met();
    home.barony.halls.asked = Some(0);
    let mut t = Spoils::default();
    settle_into(&mut home, &mut t, &["reclaim:won:0:rite", "reclaim:won:0:bargain", "reclaim:won:1:rite"]);
    assert_eq!(home.barony.halls.count(), 0, "a bargain is only struck over the toll");
    // A hall not asked is not won.
    home.barony.halls.asked = None;
    settle_into(&mut home, &mut t, &["reclaim:won:0:fight"]);
    assert_eq!(home.barony.halls.count(), 0);
}

#[test]
fn the_chain_runs_gallery_lamps_anvils_and_the_dwarves_come_home() {
    let mut home = met();
    let mut t = Spoils::default();
    let base = barony::crew(&home.barony);
    for (i, how) in [(0, "fight"), (1, "rite"), (2, "freed")] {
        settle_into(&mut home, &mut t, &[&ask_mark(i)]);
        assert_eq!(home.barony.halls.asked, Some(i as u8));
        settle_into(&mut home, &mut t, &[&won_mark(i, how)]);
        assert!(home.barony.halls.is_held(HALLS[i].id));
    }
    assert_eq!(home.barony.halls.count(), 3);
    assert_eq!(barony::crew(&home.barony), base + 1 + 1 + 2);
    assert!(home.barony.halls.offer(&home.barony).is_none(), "no hall left to ask");
    // 250 gold, 3 gems, 150 gold and 10 ore in thanks.
    assert_eq!((t.get(Spoil::Gold), t.get(Spoil::Gem), t.get(Spoil::Ore)), (400, 3, 10));
}

#[test]
fn the_halls_rent_joins_the_tribute_for_every_floor_cleared() {
    let mut home = met();
    let mut t = Spoils::default();
    assert!(barony::pay_tribute(&mut home, &mut t, 2).is_none(), "no halls, no workings: no tribute");
    home.barony.halls.won.push(Won { id: "gallery".into(), how: "fight".into() });
    home.barony.halls.won.push(Won { id: "anvils".into(), how: "freed".into() });
    let paid = barony::pay_tribute(&mut home, &mut t, 2).expect("rent");
    assert_eq!(paid.get(Spoil::Gold), 2 * (25 + 20));
    assert_eq!(paid.get(Spoil::Ore), 2 * 4);
    assert_eq!(t, paid);
    assert_eq!(home.barony.ledger.lines.last().unwrap().what, "Tribute: 2 floors cleared");
}

#[test]
fn an_older_barony_loads_with_no_halls_and_a_new_one_round_trips() {
    let old = r#"{"court":"met","works":[],"done":[],"lit":[],"ledger":{"paid_in":{},"paid_back":{},"lines":[]}}"#;
    let b: Barony = serde_json::from_str(old).expect("old save");
    assert!(b.halls.is_empty());
    let json = serde_json::to_string(&b).unwrap();
    assert!(!json.contains("halls"), "an empty chain is not written");
    let mut b = b;
    b.halls.asked = Some(1);
    b.halls.won.push(Won { id: "gallery".into(), how: "bargain".into() });
    let back: Barony = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
    assert_eq!(back, b);
}

#[test]
fn the_rite_order_is_the_eldest_son_first_whatever_the_seed() {
    for seed in 0..40 {
        let ages = rooms::rite_ages(seed);
        let mut sorted = ages;
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3], "every son has a stone");
        let order = rooms::rite_order(seed);
        for (rank, &place) in order.iter().enumerate() {
            assert_eq!(ages[place], rank, "seed {seed}: the rank-{rank} son's stone");
        }
    }
    assert!((0..40).map(rooms::rite_order).collect::<std::collections::BTreeSet<_>>().len() > 3, "the seed moves them");
}

// ── Rooms on the floor ───────────────────────────────────────────────────

fn floor_of(pack: Pack, seed: u64, home: &Home) -> Run {
    let mut run = Run::at_home(seed, 1, None, home.clone(), Spoils::default());
    run.dungeon.pack = pack;
    run.descend_for_test();
    run
}

fn hall_room(run: &Run) -> Option<usize> {
    run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::DwarfHall)
}

#[test]
fn the_asked_hall_hangs_off_every_floor_of_its_delve_and_nowhere_else() {
    let mut home = met();
    for seed in 0..8 {
        let run = floor_of(Pack::Cavern, seed, &home);
        assert!(hall_room(&run).is_none(), "seed {seed}: nothing asked, nothing hangs");
    }
    home.barony.halls.asked = Some(0);
    for seed in 0..12 {
        let mut run = floor_of(Pack::Cavern, seed, &home);
        let i = hall_room(&run).unwrap_or_else(|| panic!("seed {seed}: the Gallery"));
        let room = &run.dungeon.rooms[i];
        assert_eq!(room.doors.iter().filter(|&&d| d).count(), 1, "one doorway");
        let door = (0..4).find(|&d| room.doors[d]).unwrap();
        let host = run.dungeon.neighbour(i, door).expect("joined to a room");
        assert!(run.dungeon.rooms[host].doors[(door + 2) % 4]);
        assert!(layout::connected(room));
        assert!(run.valid_snapshot(), "seed {seed}");
        // The Crypt below holds the Hall of Lamps, not the Gallery, and not yet.
        run.descend_for_test();
        assert_eq!(run.dungeon.pack, Pack::Crypt);
        assert!(hall_room(&run).is_none(), "seed {seed}: the Lamps are not asked yet");
    }
}

#[test]
fn a_hall_won_stays_on_the_floors_as_the_kings() {
    let mut home = met();
    home.barony.halls.won.push(Won { id: "gallery".into(), how: "fight".into() });
    let run = floor_of(Pack::Cavern, 3, &home);
    assert!(hall_room(&run).is_some());
    let mut unmet = home.clone();
    unmet.barony.court = Court::Camped;
    assert!(hall_room(&floor_of(Pack::Cavern, 3, &unmet)).is_none());
}

#[test]
fn the_floor_map_names_the_hall_the_king_asked_for() {
    let mut home = met();
    home.barony.halls.asked = Some(0);
    let run = floor_of(Pack::Cavern, 5, &home);
    let held = run.missions_held(1);
    assert!(held.iter().any(|h| h.name == "King Brannoc: The Delvers' Gallery" && h.place == missions::Place::Delve(Some(missions::Spot::Hall))));
}

// ── The run's half ───────────────────────────────────────────────────────

fn press() -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { interact: true, ..Input::default() })])
}

fn answer(pick: u8) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { cast: pick, ..Input::default() })])
}

/// A press of E, then a tick of nothing.
fn tap(run: &mut Run) {
    run.step(&press());
    run.step(&BTreeMap::new());
}

fn stand(run: &mut Run, at: (f32, f32)) {
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS);
}

/// A run in the asked hall's room, the party at its door.
fn in_hall(pack: Pack, home: Home, treasury: Spoils) -> Run {
    let mut run = Run::at_home(7, 1, None, home, treasury);
    run.dungeon.pack = pack;
    run.descend_for_test();
    let i = hall_room(&run).expect("a hall");
    run.enter_for_test(i);
    run.step(&BTreeMap::new());
    run
}

fn at_the_king(home: Home) -> Run {
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    let i = world::room_of(&run.dungeon, RoomKind::MineHead).unwrap();
    run.arrive(i, (12.0, 11.0));
    run.step(&BTreeMap::new());
    run.marks.clear();
    let (kx, ky) = barony::CAMP_KING;
    stand(&mut run, (kx + 1.0, ky + 0.5));
    run
}

#[test]
fn e_beside_the_king_opens_his_talk_and_the_ask_is_a_mark() {
    let mut run = at_the_king(met());
    tap(&mut run);
    let view = run.reclaim_view(1).expect("a talk opened");
    assert_eq!(view.who, Who::King);
    assert!(view.words.contains("Help me take them back"), "{}", view.words);
    // Page two is the ask, with the hall's name.
    tap(&mut run);
    let view = run.reclaim_view(1).unwrap();
    assert_eq!(view.words, HALLS[0].asks);
    assert_eq!(view.answers.len(), 3);
    assert!(view.answers[0].contains("DELVERS' GALLERY"));
    // The lore is one more page and back.
    run.step(&answer(2));
    assert_eq!(run.reclaim_view(1).unwrap().words, HALLS[0].lore);
    tap(&mut run);
    assert_eq!(run.reclaim_view(1).unwrap().words, HALLS[0].asks);
    // 1 takes it: a mark, and the key did not cast.
    run.step(&answer(1));
    assert_eq!(run.marks.get(&ask_mark(0)), Some(&1));
    assert!(run.reclaim_view(1).is_none());
}

#[test]
fn walking_off_ends_the_talk_and_the_pad_by_the_ruin_is_not_a_talk() {
    let mut run = at_the_king(met());
    tap(&mut run);
    assert!(run.reclaim_view(1).is_some());
    let (kx, ky) = barony::CAMP_KING;
    stand(&mut run, (kx + 9.0, ky + 1.0));
    run.step(&BTreeMap::new());
    assert!(run.reclaim_view(1).is_none());
    // Holding E on the ruin's plate buys the hall, not a conversation.
    let (c, r, w, h) = barony::HALL_PLATE;
    stand(&mut run, (c as f32 + w as f32 / 2.0, r as f32 + h as f32 / 2.0));
    tap(&mut run);
    assert!(run.reclaim_view(1).is_none());
}

#[test]
fn the_kings_words_change_as_the_halls_fall() {
    let mut home = met();
    let words = |home: &Home| {
        let mut run = at_the_king(home.clone());
        tap(&mut run);
        let greeting = run.reclaim_view(1).unwrap();
        tap(&mut run);
        (greeting.words, greeting.tag, run.reclaim_view(1).unwrap().words)
    };
    let mut seen = vec![words(&home)];
    for id in ["gallery", "lamps", "anvils"] {
        home.barony.halls.won.push(Won { id: id.into(), how: "fight".into() });
        seen.push(words(&home));
    }
    let greetings: std::collections::BTreeSet<_> = seen.iter().map(|w| w.0.clone()).collect();
    assert_eq!(greetings.len(), 4, "a new greeting for every hall");
    assert_eq!(seen[1].2, HALLS[1].asks, "the next ask follows");
    assert!(seen[3].1.contains("3 OF 3"));
    assert!(seen[3].2.contains("rent in me ledger"));
    assert!(!seen.iter().any(|w| w.0.contains("Great Forge") || w.2.contains("Great Forge")));
}

#[test]
fn nubbins_toll_is_asked_and_marked_only_when_the_purse_covers_it() {
    let mut home = met();
    home.barony.halls.asked = Some(0);
    let mut run = in_hall(Pack::Cavern, home.clone(), gold(TOLL - 1));
    run.marks.clear();
    let at = rooms::KEEPER_AT[0];
    stand(&mut run, (at.0, at.1 + 1.5));
    tap(&mut run);
    tap(&mut run);
    let view = run.reclaim_view(1).expect("Nubbin talks");
    assert_eq!(view.who, Who::Keeper);
    assert!(view.words.contains("300 gold"), "{}", view.words);
    run.step(&answer(1));
    assert!(!run.marks.contains_key(&toll_mark(0)), "a short purse pays nothing");
    assert_eq!(run.hall_play().unwrap().stage, 0);
    run.treasury = gold(TOLL);
    run.step(&answer(1));
    assert_eq!(run.marks.get(&toll_mark(0)), Some(&1));
    assert_eq!(run.hall_play().unwrap().how, "bargain");
    assert!(run.enemies.is_empty(), "no fight when the toll is paid");
}

#[test]
fn fighting_nubbin_raises_his_boar_and_kin_and_the_fall_of_all_wins_the_hall() {
    let mut home = met();
    home.barony.halls.asked = Some(0);
    let mut run = in_hall(Pack::Cavern, home, Spoils::default());
    run.marks.clear();
    let at = rooms::KEEPER_AT[0];
    stand(&mut run, (at.0, at.1 + 1.5));
    tap(&mut run);
    tap(&mut run);
    run.step(&answer(2));
    let boar = run.enemies.iter().find(|e| e.kind == EnemyKind::Warboar).expect("his boar");
    assert!(boar.elite);
    let guards = run.enemies.iter().filter(|e| e.kind != EnemyKind::Warboar).count();
    assert_eq!(guards, 3);
    assert_eq!(run.hall_play().unwrap().stage, 1);
    assert!(!run.marks.keys().any(|m| m.starts_with("reclaim:won")), "not won while any stand");
    for e in run.enemies.iter_mut().filter(|e| e.kind != EnemyKind::Warboar) {
        e.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.hall_play().unwrap().slain, 3);
    assert_eq!(run.hall_play().unwrap().stage, 1, "the boar still stands");
    for e in &mut run.enemies {
        e.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.hall_play().unwrap().stage, 2);
    assert_eq!(run.marks.get(&won_mark(0, "fight")), Some(&1));
}

#[test]
fn the_lamps_must_be_kindled_eldest_son_first_or_the_dead_stir() {
    let mut home = met();
    home.barony.halls.won.push(Won { id: "gallery".into(), how: "fight".into() });
    home.barony.halls.asked = Some(1);
    let mut run = in_hall(Pack::Crypt, home, Spoils::default());
    run.marks.clear();
    let order = rooms::rite_order(run.seed);
    let at = rooms::KEEPER_AT[1];
    stand(&mut run, (at.0, at.1 + 1.0));
    tap(&mut run);
    tap(&mut run);
    tap(&mut run);
    assert_eq!(run.hall_play().unwrap().stage, 1, "the rite is begun by the thane's ask");
    // The youngest first is wrong: all gutter, and wraiths come.
    stand(&mut run, rooms::LAMPS[order[3]]);
    tap(&mut run);
    assert!(run.hall_play().unwrap().lit.is_empty());
    assert_eq!(run.enemies.iter().filter(|e| e.kind == EnemyKind::Wraith).count(), 2);
    // Eldest first, the four in order, wins it.
    for (n, &lamp) in order.iter().enumerate() {
        stand(&mut run, rooms::LAMPS[lamp]);
        tap(&mut run);
        assert_eq!(run.hall_play().unwrap().lit.len(), n + 1);
    }
    assert_eq!(run.hall_play().unwrap().stage, 2);
    assert_eq!(run.marks.get(&won_mark(1, "rite")), Some(&1));
}

#[test]
fn every_wrong_lamp_costs_two_wraiths_up_to_a_cap() {
    let mut home = met();
    home.barony.halls.won.push(Won { id: "gallery".into(), how: "fight".into() });
    home.barony.halls.asked = Some(1);
    let mut run = in_hall(Pack::Crypt, home, Spoils::default());
    let at = rooms::KEEPER_AT[1];
    stand(&mut run, (at.0, at.1 + 1.0));
    tap(&mut run);
    tap(&mut run);
    tap(&mut run);
    let order = rooms::rite_order(run.seed);
    for _ in 0..6 {
        stand(&mut run, rooms::LAMPS[order[3]]);
        tap(&mut run);
    }
    assert_eq!(run.hall_play().unwrap().raised, play::RAISE_CAP);
}

#[test]
fn digging_olwen_out_takes_three_heaps_and_wakes_imps_at_each() {
    let mut home = met();
    for id in ["gallery", "lamps"] {
        home.barony.halls.won.push(Won { id: id.into(), how: "fight".into() });
    }
    home.barony.halls.asked = Some(2);
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    run.dungeon.pack = Pack::Cavern;
    run.descend_for_test();
    run.descend_for_test();
    run.descend_for_test();
    assert_eq!(run.dungeon.pack, Pack::Hellforge);
    let i = hall_room(&run).expect("the Anvil Gallery");
    run.enter_for_test(i);
    run.step(&BTreeMap::new());
    run.marks.clear();
    let at = rooms::KEEPER_AT[2];
    stand(&mut run, (at.0, at.1 + 1.5));
    tap(&mut run);
    tap(&mut run);
    tap(&mut run);
    assert_eq!(run.hall_play().unwrap().stage, 1);
    for (n, &(c, r, w, h)) in rooms::DIGS.iter().enumerate() {
        stand(&mut run, (c as f32 + w as f32 / 2.0, r as f32 + h as f32 / 2.0));
        for _ in 0..play::DIG_HOLD + 2 {
            run.step(&press());
        }
        run.step(&BTreeMap::new());
        assert_eq!(run.hall_play().unwrap().dug.count_ones() as usize, n + 1);
        assert_eq!(run.enemies.iter().filter(|e| e.kind == EnemyKind::Imp).count(), 2 * (n + 1));
    }
    assert_eq!(run.hall_play().unwrap().stage, 2);
    assert_eq!(run.marks.get(&won_mark(2, "freed")), Some(&1));
}

#[test]
fn a_hall_the_realm_holds_is_played_out_when_the_party_walks_in() {
    let mut home = met();
    home.barony.halls.won.push(Won { id: "gallery".into(), how: "bargain".into() });
    let mut run = in_hall(Pack::Cavern, home, Spoils::default());
    assert_eq!(run.hall_play().unwrap().stage, 2);
    let at = rooms::KEEPER_AT[0];
    stand(&mut run, (at.0, at.1 + 1.5));
    tap(&mut run);
    tap(&mut run);
    let view = run.reclaim_view(1).unwrap();
    assert!(view.words.contains("Rent's rent"), "{}", view.words);
    assert!(run.enemies.is_empty());
}

/// A run in the asked hall's room by the floor's own door, with the floor's
/// politics standing (`enter_for_test` takes them away).
fn in_hall_with_gates(pack: Pack, home: Home, treasury: Spoils) -> Run {
    let mut run = Run::at_home(7, 1, None, home, treasury);
    run.dungeon.pack = pack;
    run.descend_for_test();
    assert!(run.boss_gates.is_some());
    let i = hall_room(&run).expect("a hall");
    let door = (0..4).find(|&d| run.dungeon.rooms[i].doors[d]).unwrap();
    run.enter(i, Some(door));
    run.step(&BTreeMap::new());
    run
}

#[test]
fn the_floors_politics_hold_while_the_chief_fights_and_falls() {
    let mut home = met();
    home.barony.halls.asked = Some(0);
    for seed_run in 0..2 {
        let mut run = in_hall_with_gates(Pack::Cavern, home.clone(), Spoils::default());
        assert!(run.valid_snapshot(), "walking in");
        run.marks.clear();
        let at = rooms::KEEPER_AT[0];
        stand(&mut run, (at.0, at.1 + 1.5));
        tap(&mut run);
        tap(&mut run);
        run.step(&answer(2));
        assert_eq!(run.enemies.len(), 4);
        assert!(run.valid_snapshot(), "raised {seed_run}");
        // Walk out and back in: the clan is raised afresh, none counted fallen.
        let door = (0..4).find(|&d| run.dungeon.rooms[run.at].doors[d]).unwrap();
        let hall = run.at;
        let host = run.dungeon.neighbour(hall, door).unwrap();
        run.enter(host, Some((door + 2) % 4));
        run.step(&BTreeMap::new());
        assert!(run.enemies.iter().all(|e| e.kind != EnemyKind::Warboar));
        run.enter(hall, Some(door));
        run.step(&BTreeMap::new());
        assert_eq!(run.hall_play().unwrap().stage, 1);
        assert_eq!(run.enemies.len(), 4, "the clan stands as it was left");
        assert!(run.valid_snapshot(), "back in");
        for e in &mut run.enemies {
            e.hp = 0;
        }
        run.step(&BTreeMap::new());
        run.step(&BTreeMap::new());
        assert!(run.valid_snapshot(), "fallen");
        assert_eq!(run.hall_play().unwrap().stage, 2);
        assert_eq!(run.marks.get(&won_mark(0, "fight")), Some(&1));
        assert!(run.enemies.is_empty());
    }
}

#[test]
fn the_realm_settles_the_halls_through_the_activity_registry() {
    let mut home = met();
    let mut t = Spoils::default();
    let mut realm = Realm {
        home: &mut home,
        treasury: &mut t,
        reclaimed: &BTreeMap::new(),
        raids_won: 0,
    };
    let out = activity::settle_all(&mut realm, &marks(&["reclaim:ask:0"]));
    assert!(out.changed && out.said.iter().any(|(c, _)| c == "hall_asked:gallery"));
    assert_eq!(realm.home.barony.halls.asked, Some(0));
    assert!(!activity::settle_all(&mut realm, &BTreeMap::new()).changed, "nothing marked, nothing changed");
}

#[test]
fn a_run_mid_set_piece_survives_its_checkpoint() {
    let mut home = met();
    home.barony.halls.won.push(Won { id: "gallery".into(), how: "fight".into() });
    home.barony.halls.asked = Some(1);
    let mut run = in_hall(Pack::Crypt, home, Spoils::default());
    let at = rooms::KEEPER_AT[1];
    stand(&mut run, (at.0, at.1 + 1.0));
    tap(&mut run);
    tap(&mut run);
    tap(&mut run);
    let order = rooms::rite_order(run.seed);
    stand(&mut run, rooms::LAMPS[order[0]]);
    tap(&mut run);
    stand(&mut run, at);
    tap(&mut run);
    assert!(!run.reclaim.talks.is_empty() && run.reclaim.hall.as_ref().is_some_and(|h| h.lit.len() == 1));
    let json = serde_json::to_string(&run).expect("a checkpoint");
    let back: Run = serde_json::from_str(&json).expect("loads again");
    assert_eq!(back.reclaim, run.reclaim);
    assert!(back.valid_snapshot());
    // A run saved before the reclaiming, with no such field, loads with none.
    let mut tree: serde_json::Value = serde_json::from_str(&json).unwrap();
    tree.as_object_mut().unwrap().remove("reclaim");
    let old: Run = serde_json::from_value(tree).expect("an older checkpoint");
    assert!(old.reclaim.is_empty());
}

/// The cockpit's half, as the host does it: what the run marked is settled
/// into the realm and the run is told the realm's word.
fn settle_run(run: &mut Run, treasury: &mut Spoils) {
    let marks = std::mem::take(&mut run.marks);
    let mut home = run.home.clone();
    let mines = 1;
    barony::settle(&mut home, treasury, mines, &marks, &|id| format!("Knight {id}"));
    let mut realm = Realm {
        home: &mut home,
        treasury,
        reclaimed: &BTreeMap::new(),
        raids_won: 0,
    };
    activity::settle_all(&mut realm, &marks);
    run.rebuild_home(home, treasury.clone());
}

#[test]
fn the_road_from_the_camp_to_a_won_hall_and_its_rent_by_the_games_own_ways() {
    let mut treasury = gold(900);
    let mut camped = Home::default();
    camped.barony.court = Court::Camped;
    let mut run = Run::at_home(7, 1, None, camped, treasury.clone());
    let i = world::room_of(&run.dungeon, RoomKind::MineHead).unwrap();
    run.arrive(i, (12.0, 11.0));
    run.step(&BTreeMap::new());
    // He is met by walking up to him; the first mission is his, then his talk.
    let (kx, ky) = barony::CAMP_KING;
    stand(&mut run, (kx + 1.0, ky + 0.8));
    for _ in 0..4 {
        run.step(&BTreeMap::new());
    }
    settle_run(&mut run, &mut treasury);
    assert_eq!(run.home.barony.court, Court::Met);
    tap(&mut run);
    tap(&mut run);
    run.step(&answer(1));
    settle_run(&mut run, &mut treasury);
    assert_eq!(run.home.barony.halls.asked, Some(0));
    // The shaft takes the party down into the Mines.
    let shaft = world::ENTRANCES.iter().find(|e| e.id == "shaft").unwrap();
    let (x, y) = world::entrance_centre(shaft);
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (x, y);
    }
    for _ in 0..crate::rules::home::DESCEND_HOLD + 2 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.dungeon.pack, Pack::Cavern);
    assert_eq!(run.dungeon.depth, 1);
    // The floor map points at the hall, and the hall can be walked to from where the party stands.
    let hall = hall_room(&run).expect("the Gallery is off the floor");
    assert!(run.missions_held(1).iter().any(|h| h.place == missions::Place::Delve(Some(missions::Spot::Hall))));
    let mut seen = vec![false; run.dungeon.rooms.len()];
    let mut queue = std::collections::VecDeque::from([run.at]);
    seen[run.at] = true;
    while let Some(r) = queue.pop_front() {
        for dir in 0..4 {
            if let Some(n) = run.dungeon.neighbour(r, dir).filter(|&n| !seen[n]) {
                seen[n] = true;
                queue.push_back(n);
            }
        }
    }
    assert!(seen[hall], "the hall is joined to the floor");
    // In its doorway, Nubbin's toll is paid, and the realm takes it.
    let door = (0..4).find(|&d| run.dungeon.rooms[hall].doors[d]).unwrap();
    run.enter(hall, Some(door));
    run.step(&BTreeMap::new());
    let at = rooms::KEEPER_AT[0];
    stand(&mut run, (at.0, at.1 + 1.5));
    tap(&mut run);
    tap(&mut run);
    run.step(&answer(1));
    settle_run(&mut run, &mut treasury);
    assert!(run.home.barony.halls.is_held("gallery"));
    assert_eq!(treasury.get(Spoil::Gold), 900 - TOLL);
    assert_eq!(run.treasury.get(Spoil::Gold), 900 - TOLL);
    // The next floor cleared brings rent, gold and ore.
    let mut home = run.home.clone();
    let paid = barony::pay_tribute(&mut home, &mut treasury, 1).expect("rent");
    assert_eq!((paid.get(Spoil::Gold), paid.get(Spoil::Ore)), (25, 2));
    // His next ask is the Hall of Lamps.
    assert_eq!(home.barony.halls.offer(&home.barony).map(|(_, h)| h.id), Some("lamps"));
}

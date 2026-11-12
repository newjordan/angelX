//! The travellers on the kingdom's roads: sent by the clock of the party's
//! walking, never the same kind twice running, met with E, answered with 1,
//! 2 and 3, each answer doing what it said, and kept in the realm's ledger.
use super::*;
use crate::render::overworld::kingdom as geography;
use crate::rules::activity::Realm;
use crate::rules::holdings::{Built, Deed, Holding, ROOM_LABOUR};
use crate::rules::{Pack, home::Home, world};

fn kingdom() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_kingdom(Some(geography::keep_stand()));
    r.players.retain(|&id, _| id == 1);
    r
}

/// Where the first road is `along` tiles in, in arena units.
fn road_at(along: f32) -> (f32, f32) {
    let (x, y) = paths()[0].at(along);
    (x * TILE_UNITS, y * TILE_UNITS)
}

fn put(r: &mut Run, (x, y): (f32, f32)) {
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (x, y);
}

fn idle(r: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        r.step(&BTreeMap::new());
    }
}

fn press(r: &mut Run) {
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    r.step(&BTreeMap::new());
}

fn say(r: &mut Run, pick: u8) {
    r.step(&BTreeMap::from([(1, Input { cast: pick, ..Default::default() })]));
    r.step(&BTreeMap::new());
}

/// A traveller of `kind` stands in the road beside the knight, who is on it.
fn meet(r: &mut Run, kind: Kind) {
    let at = road_at(30.0);
    put(r, at);
    r.step(&BTreeMap::new());
    let (x, y) = road_at(31.0);
    r.wayfarers.on = Some(Wayfarer {
        kind,
        name: "Aldous".into(),
        x,
        y,
        east: false,
        road: 0,
        along: 31.0,
        dir: 1,
        born: r.tick,
        stage: Stage::Walking,
        moved: 0,
        still: 0,
        met: false,
        done: false,
        sold: 0,
        gang: Vec::new(),
    });
}

/// Meet, speak, and go on to what they offer.
fn open(r: &mut Run, kind: Kind) {
    meet(r, kind);
    press(r);
    assert_eq!(r.wayfarers.talks.get(&1).map(|t| t.page), Some(0), "E beside them opens a talk");
    press(r);
    assert_eq!(r.wayfarers.talks.get(&1).map(|t| t.page), Some(1), "E again goes on to their offer");
}

fn treasure(r: &mut Run, gold: u32) {
    r.treasury.add(Spoil::Gold, gold);
}

fn bed() -> Holding {
    Holding {
        key: Deed::Floor(Pack::Crypt).key(),
        rooms: vec![Built { id: "taproom".into(), labour: ROOM_LABOUR }],
        steward: Some("vell".into()),
        ..Holding::default()
    }
}

#[test]
fn every_road_is_walkable_ground_end_to_end() {
    let geo = geography::geography(&[]);
    let mut shut = Vec::new();
    for (i, path) in paths().iter().enumerate() {
        let mut along = 0.0;
        while along <= path.len() {
            let (x, y) = path.at(along);
            if geo.room.tile(x.floor() as i32, y.floor() as i32) != Tile::Floor {
                shut.push((i, along as i32));
            }
            along += 1.0;
        }
    }
    // A traveller steps only onto open ground (`walk_on`), so a stray tile
    // under a gate stops one, never strands it.
    let total: usize = paths().iter().map(|p| p.len() as usize).sum();
    assert!(shut.len() * 50 < total, "{} of {total} road tiles are shut: {shut:?}", shut.len());
}

#[test]
fn a_traveller_is_sent_after_the_party_has_walked_a_road_a_while() {
    let mut r = kingdom();
    let at = road_at(30.0);
    let walk = Input { move_x: 1, ..Default::default() };
    for _ in 0..FIRST_AFTER - 60 {
        put(&mut r, at);
        r.step(&BTreeMap::from([(1, walk)]));
    }
    assert!(r.wayfarers.on.is_none(), "not before the clock");
    for _ in 0..180 {
        put(&mut r, at);
        r.step(&BTreeMap::from([(1, walk)]));
    }
    let w = r.wayfarers.on.clone().expect("a traveller is on the road");
    assert_eq!(w.stage, Stage::Walking);
    assert_eq!(r.wayfarers.sent, 1);
    let tiles = (w.x / TILE_UNITS, w.y / TILE_UNITS);
    let (kx, ky) = (at.0 / TILE_UNITS, at.1 / TILE_UNITS);
    assert!((tiles.0 - kx).hypot(tiles.1 - ky) >= OUT_OF_SIGHT - 1.0, "starts out of sight");
    assert!(NAMES.contains(&w.name.as_str()));
    assert!(r.valid_snapshot());
    // Standing still, off the road, counts for nothing.
    let mut still = kingdom();
    idle(&mut still, FIRST_AFTER as u32 + 120);
    assert!(still.wayfarers.on.is_none(), "the clock runs on the road, walking");
}

#[test]
fn the_traveller_walks_to_the_knights_and_waits_for_a_word() {
    let mut r = kingdom();
    let at = road_at(30.0);
    let walk = Input { move_x: 1, ..Default::default() };
    for _ in 0..FIRST_AFTER + 180 {
        put(&mut r, at);
        r.step(&BTreeMap::from([(1, walk)]));
    }
    let start = r.wayfarers.on.clone().unwrap();
    // The knight now stands still on the road; the traveller comes up.
    let here = Input { aim_x: 1, ..Default::default() };
    for _ in 0..30 * HZ {
        put(&mut r, at);
        r.step(&BTreeMap::from([(1, here)]));
    }
    let w = r.wayfarers.on.clone().expect("still waiting");
    let hero = r.players[&1].clone();
    let near = (w.x - hero.x).hypot(w.y - hero.y) / TILE_UNITS;
    assert!(near <= REACH, "{near} tiles off after half a minute of walking");
    assert!(r.wayfarer_in_reach(1).is_some());
    assert_ne!((w.x, w.y), (start.x, start.y), "it walked");
    assert_eq!(w.name, start.name);
    // Nobody speaks: after a long wait it takes the road on.
    let mut left = false;
    for _ in 0..IDLE + 60 {
        put(&mut r, at);
        r.step(&BTreeMap::from([(1, here)]));
        left |= matches!(r.wayfarers.on.as_ref().map(|w| w.stage), Some(Stage::Leaving(_)));
    }
    assert!(left, "it took the road on");
}

#[test]
fn the_same_kind_never_comes_twice_running_and_every_kind_comes() {
    let mut r = kingdom();
    r.home.deepest = 3;
    r.home.bestiary.insert("bat".into(), 9);
    r.home.holdings.owned.push(bed());
    let at = road_at(30.0);
    put(&mut r, at);
    let mut seen = Vec::new();
    for _ in 0..60 {
        r.wayfarers.on = None;
        r.wayfarers.walked = THEN_AFTER;
        for _ in 0..30 {
            put(&mut r, at);
            r.step(&BTreeMap::new());
            if r.wayfarers.on.is_some() {
                break;
            }
        }
        seen.push(r.wayfarers.on.clone().expect("sent").kind);
    }
    assert!(seen.windows(2).all(|p| p[0] != p[1]), "{seen:?}");
    for kind in KINDS {
        assert!(seen.contains(&kind), "{kind:?} never came in {} travellers", seen.len());
    }
    // The same realm on the same seed sends the same traveller.
    let first = |home: &Home| {
        let mut run = kingdom();
        run.home = home.clone();
        run.wayfarers.walked = FIRST_AFTER;
        for _ in 0..30 {
            put(&mut run, at);
            run.step(&BTreeMap::new());
        }
        let w = run.wayfarers.on.unwrap();
        (w.kind, w.name, w.road)
    };
    assert_eq!(first(&r.home), first(&r.home));
}

#[test]
fn what_cannot_be_done_is_not_sent() {
    let mut r = kingdom();
    let geo = geography::geography(&[]);
    let lead = road_at(30.0);
    // A fresh realm has no songs to sing, no deeds to name and no bed.
    assert!(!r.could_send(Kind::Troubadour, &geo, lead));
    assert!(!r.could_send(Kind::Tollkeeper, &geo, lead));
    assert!(!r.could_send(Kind::Squire, &geo, lead));
    assert!(r.could_send(Kind::Pilgrim, &geo, lead) && r.could_send(Kind::Merchant, &geo, lead) && r.could_send(Kind::Gossip, &geo, lead));
    r.home.deepest = 2;
    assert!(r.could_send(Kind::Troubadour, &geo, lead) && r.could_send(Kind::Tollkeeper, &geo, lead));
}

#[test]
fn a_traveller_knows_who_the_party_last_met_from_the_realms_ledger() {
    let mut r = kingdom();
    meet(&mut r, Kind::Merchant);
    press(&mut r);
    assert!(!r.wayfarer_speech(1).unwrap().words.contains("Word on the road"), "a fresh realm has met nobody");
    r.home.wayfarers.lately.push("Brother Corwin".into());
    let words = r.wayfarer_speech(1).unwrap().words;
    assert!(words.contains("you met Brother Corwin"), "{words}");
    // Not told of themselves.
    r.home.wayfarers.lately.push("Peddler Aldous".into());
    assert!(!r.wayfarer_speech(1).unwrap().words.contains("Word on the road"));
}

#[test]
fn a_pilgrim_blesses_the_party_for_a_while_and_it_is_felt() {
    let mut r = kingdom();
    r.join_seat(2, "Two");
    open(&mut r, Kind::Pilgrim);
    let before = crate::rules::fx::ground_pace(&r.players[&1], &[], None, None, &r.tune);
    assert_eq!(r.players[&1].scaled(100), 100);
    say(&mut r, 1);
    for id in [1, 2] {
        let h = &r.players[&id];
        assert!(h.blessed > BLESS_TICKS - 5, "knight {id} is blessed");
        assert_eq!(h.scaled(100), 100 + BLESS_PERCENT, "damage rises {BLESS_PERCENT}%");
    }
    let after = crate::rules::fx::ground_pace(&r.players[&1], &[], None, None, &r.tune);
    assert!(after > before * 1.14, "pace rises: {before} -> {after}");
    let words = r.wayfarer_speech(1).unwrap().words;
    assert!(words.contains("15%") && words.contains("60 seconds"), "{words}");
    assert!(r.wayfarers.on.as_ref().unwrap().done);
    // It wears off.
    for _ in 0..BLESS_TICKS + 5 {
        r.players.get_mut(&1).unwrap().dress();
    }
    assert_eq!(r.players[&1].blessed, 0);
    assert_eq!(r.players[&1].scaled(100), 100);
    assert!(r.valid_snapshot());
}

#[test]
fn a_pilgrim_can_bless_one_knight_longer_and_mend_them() {
    let mut r = kingdom();
    r.join_seat(2, "Two");
    open(&mut r, Kind::Pilgrim);
    let max = r.players[&1].max_hp;
    r.players.get_mut(&1).unwrap().hp = 5;
    r.players.get_mut(&2).unwrap().hp = 5;
    say(&mut r, 2);
    assert_eq!(r.players[&1].hp, max, "the knight who asked is whole");
    assert_eq!(r.players[&2].hp, 5, "not the friend");
    assert!(r.players[&1].blessed > BLESS_ALONE_TICKS - 5, "and blessed three times as long");
    assert_eq!(r.players[&2].blessed, 0);
    assert_eq!(r.players[&1].scaled(100), 100 + BLESS_PERCENT);
    assert_eq!(r.wayfarers.talks[&1].page, 2);
    assert!(r.wayfarer_speech(1).unwrap().words.contains("180 seconds"));
}

#[test]
fn an_unhurt_knight_still_gets_the_longer_blessing() {
    let mut r = kingdom();
    open(&mut r, Kind::Pilgrim);
    say(&mut r, 2);
    assert!(r.players[&1].blessed > BLESS_ALONE_TICKS - 5);
    assert!(r.wayfarers.on.as_ref().unwrap().done, "one blessing, one use");
}

#[test]
fn the_peddler_sells_for_the_treasury_and_never_the_same_coin_twice() {
    let mut r = kingdom();
    treasure(&mut r, POTION + BOMB + 5);
    open(&mut r, Kind::Merchant);
    let hand = r.players[&1].hand.len();
    let belt = r.players[&1].bombs;
    say(&mut r, 1);
    assert_eq!(r.players[&1].hand.len(), hand + 1);
    assert_eq!(r.players[&1].hand.last().map(String::as_str), Some("potion"));
    assert_eq!(r.marks[PAID], POTION, "the save charges the treasury");
    assert_eq!(r.gold_spare(), BOMB + 5, "and the road counts it spent");
    say(&mut r, 2);
    assert_eq!(r.players[&1].bombs, belt + 1);
    assert_eq!(r.marks[PAID], POTION + BOMB);
    // The treasury has 5 gold left: a scroll is out of reach, and nothing moves.
    let hand = r.players[&1].hand.len();
    say(&mut r, 3);
    assert_eq!(r.players[&1].hand.len(), hand);
    assert_eq!(r.marks[PAID], POTION + BOMB);
    assert!(r.found.as_ref().unwrap().2.contains("CANNOT SPARE"), "{:?}", r.found);
    assert_eq!(r.wayfarers.talks[&1].page, 1, "he waits for something else");
    let speech = r.wayfarer_speech(1).unwrap();
    assert!(speech.words.contains("can spare 5 gold"), "{}", speech.words);
}

#[test]
fn the_peddler_runs_out_of_cart() {
    let mut r = kingdom();
    treasure(&mut r, 1000);
    open(&mut r, Kind::Merchant);
    for _ in 0..STOCK {
        r.players.get_mut(&1).unwrap().hand.clear();
        say(&mut r, 1);
    }
    assert_eq!(r.marks[PAID], POTION * u32::from(STOCK));
    assert_eq!(r.wayfarers.talks[&1].page, 2, "the cart is empty");
    assert!(r.wayfarers.on.as_ref().unwrap().done);
}

#[test]
fn a_full_hand_is_not_charged() {
    let mut r = kingdom();
    treasure(&mut r, 100);
    open(&mut r, Kind::Merchant);
    r.players.get_mut(&1).unwrap().hand = vec!["potion".into(); cards::HAND];
    say(&mut r, 1);
    assert!(!r.marks.contains_key(PAID));
    assert!(r.found.as_ref().unwrap().2.contains("HAND IS FULL"));
}

#[test]
fn the_minstrel_sings_what_the_party_has_really_done() {
    let mut r = kingdom();
    assert!(r.verses().is_empty(), "a fresh realm has nothing to sing");
    r.home.deepest = 4;
    r.home.bestiary.insert("bat".into(), 30);
    r.home.bestiary.insert("imp".into(), 12);
    r.home.trophies.insert("cinderjaw".into(), 3);
    let verses = r.verses();
    assert_eq!(verses, vec!["Cinderjaw, felled 3 times".to_string(), "floor 4, reached".to_string()]);
    open(&mut r, Kind::Troubadour);
    let (max, score) = (r.players[&1].max_hp, r.score);
    r.players.get_mut(&1).unwrap().hp = max / 2;
    say(&mut r, 1);
    assert_eq!(r.players[&1].hp, max / 2 + max * SONG_MEND / 100, "the song mends a fifth");
    assert_eq!(r.score, score + SONG_RENOWN, "and carries the party's name");
    let words = r.wayfarer_speech(1).unwrap().words;
    assert!(words.contains("Cinderjaw, felled 3 times") && words.contains("floor 4, reached"), "{words}");
    assert!(words.contains("mends the party"));
}

#[test]
fn a_song_for_nobody_hurt_says_it_mended_nobody_and_still_carries_the_name() {
    let mut r = kingdom();
    r.home.deepest = 1;
    open(&mut r, Kind::Troubadour);
    let score = r.score;
    say(&mut r, 1);
    let words = r.wayfarer_speech(1).unwrap().words;
    assert!(words.contains("mends no one") && !words.contains("mends the party"), "{words}");
    assert_eq!(r.score, score + SONG_RENOWN);
}

#[test]
fn a_tip_carries_the_name_to_the_taverns() {
    let mut r = kingdom();
    r.home.deepest = 2;
    treasure(&mut r, 100);
    open(&mut r, Kind::Troubadour);
    let score = r.score;
    say(&mut r, 2);
    assert_eq!(r.score, score + TIP_RENOWN);
    assert_eq!(r.marks[PAID], TIP);
}

#[test]
fn the_toll_can_be_paid_and_the_rope_drops() {
    let mut r = kingdom();
    r.home.deepest = 2;
    treasure(&mut r, TOLL + 3);
    open(&mut r, Kind::Tollkeeper);
    say(&mut r, 1);
    assert_eq!(r.marks[PAID], TOLL);
    assert!(r.enemies.is_empty(), "no fight");
    assert!(r.wayfarer_speech(1).unwrap().words.contains("step aside"));
    press(&mut r);
    idle(&mut r, 3);
    assert!(matches!(r.wayfarers.on.as_ref().map(|w| w.stage), Some(Stage::Leaving(_))), "they go on their way");
    idle(&mut r, LEAVE as u32 + 5);
    assert!(r.wayfarers.on.is_none());
}

#[test]
fn a_party_that_cannot_pay_the_toll_is_not_charged() {
    let mut r = kingdom();
    r.home.deepest = 2;
    treasure(&mut r, TOLL - 1);
    open(&mut r, Kind::Tollkeeper);
    say(&mut r, 1);
    assert!(!r.marks.contains_key(PAID));
    assert_eq!(r.wayfarers.talks[&1].page, 1);
    assert!(r.enemies.is_empty());
}

#[test]
fn naming_a_felled_guardian_walks_the_party_free_and_naming_none_draws_steel() {
    let mut free = kingdom();
    free.home.trophies.insert("cinderjaw".into(), 1);
    open(&mut free, Kind::Tollkeeper);
    say(&mut free, 2);
    assert!(free.enemies.is_empty());
    assert!(free.wayfarer_speech(1).unwrap().words.contains("heard of Cinderjaw"));
    assert!(!free.marks.contains_key(PAID));

    let mut r = kingdom();
    open(&mut r, Kind::Tollkeeper);
    say(&mut r, 2);
    assert_eq!(r.enemies.len(), GANG.len(), "the gang draws steel");
    assert!(matches!(r.wayfarers.on.as_ref().unwrap().stage, Stage::Fighting(_)));
}

#[test]
fn the_fight_is_real_combat_at_the_health_such_bodies_have_anywhere() {
    let mut r = kingdom();
    open(&mut r, Kind::Tollkeeper);
    say(&mut r, 3);
    assert_eq!(r.enemies.len(), GANG.len());
    // The same bodies, spawned the way any room spawns them, at the same
    // party and floor: the gang is not softened.
    let mut twin = kingdom();
    let spawned: Vec<(EnemyKind, u32)> = GANG
        .iter()
        .map(|&k| {
            let id = twin.spawn_body(k, 100.0, 100.0).unwrap();
            (k, twin.enemies.iter().find(|e| e.id == id).unwrap().hp)
        })
        .collect();
    for (e, (kind, hp)) in r.enemies.iter().zip(spawned) {
        assert_eq!(e.kind, kind);
        assert_eq!(e.hp, hp, "{kind:?} stands at full health");
        assert_eq!(e.hp, e.max_hp);
        assert!(e.hp >= kind.hp(), "never under the kind's own health");
    }
    assert!(r.valid_snapshot());
}

#[test]
fn a_gang_beaten_pays_its_purse_to_the_realm() {
    let mut r = kingdom();
    open(&mut r, Kind::Tollkeeper);
    say(&mut r, 3);
    // Felled by the knight's own shots would take a while; felled is felled.
    for e in &mut r.enemies {
        e.hp = 0;
    }
    idle(&mut r, 3);
    assert!(r.enemies.is_empty());
    assert!(r.wayfarers.on.is_none(), "nobody left on the road");
    assert_eq!(r.marks[OWED], PURSE);
    assert!(r.cues.iter().any(|c| c == "wayfarer_won"), "{:?}", r.cues);
    assert_ne!(r.phase, Phase::Wiped);
}

#[test]
fn a_party_beaten_on_the_road_is_knocked_down_not_killed() {
    let mut r = kingdom();
    treasure(&mut r, 500);
    open(&mut r, Kind::Tollkeeper);
    say(&mut r, 3);
    let max = r.players[&1].max_hp;
    r.players.get_mut(&1).unwrap().hp = 0;
    idle(&mut r, 3);
    assert_ne!(r.phase, Phase::Wiped, "a rout, not a wipe");
    assert_eq!(r.players[&1].hp, max / 4);
    assert!(r.enemies.is_empty(), "the gang goes with its toll");
    assert_eq!(r.marks[PAID], TOLL);
    assert!(r.wayfarers.on.is_none());
}

#[test]
fn a_rout_takes_only_what_the_treasury_can_spare() {
    let mut r = kingdom();
    treasure(&mut r, 7);
    open(&mut r, Kind::Tollkeeper);
    say(&mut r, 3);
    r.players.get_mut(&1).unwrap().hp = 0;
    idle(&mut r, 3);
    assert_eq!(r.marks[PAID], 7);
}

#[test]
fn a_lost_squire_follows_the_party_to_the_village_and_takes_a_bed() {
    let mut r = kingdom();
    r.home.holdings.owned.push(bed());
    open(&mut r, Kind::Squire);
    say(&mut r, 1);
    assert_eq!(r.wayfarers.on.as_ref().unwrap().stage, Stage::Following);
    let geo = geography::geography(&[]);
    let village = geo.sites.iter().find(|s| s.room == RoomKind::Village).unwrap();
    assert_eq!(r.kingdom_goal(1).unwrap().stand, village.stand, "the arrow points to the Village");
    // Walking the party to the door, she keeps close.
    let door = ((village.stand.0 as f32 + 0.5) * TILE_UNITS, (village.stand.1 as f32 + 0.7) * TILE_UNITS);
    let start = road_at(30.0);
    for k in 1..=40 {
        let t = k as f32 / 40.0;
        put(&mut r, (start.0 + (door.0 - start.0) * t, start.1 + (door.1 - start.1) * t));
        r.wayfarers.talks.clear();
        idle(&mut r, 10);
    }
    idle(&mut r, 6 * HZ);
    assert!(r.wayfarers.on.is_none(), "she is home");
    assert_eq!(r.marks[&format!("{SQUIRE}Aldous")], 1);
    assert!(r.found.as_ref().unwrap().2.contains("TAKES A BED"), "{:?}", r.found);
    assert_eq!(r.kingdom.moments.errand, None);

    // The save puts her in the holding as a villager.
    let mut home = r.home.clone();
    let mut treasury = r.treasury.clone();
    let settled = settle(
        &mut Realm { home: &mut home, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 },
        &r.marks,
    );
    assert!(settled.changed);
    let folk = &home.holdings.owned[0].folk;
    assert_eq!(folk.len(), 1);
    assert_eq!(folk[0].name, "Aldous");
    assert!(folk[0].valid());
    assert!(home.holdings.valid());
    assert!(settled.said.iter().any(|(_, l)| l.contains("Aldous")), "{:?}", settled.said);
}

#[test]
fn a_squire_comes_only_where_there_is_a_bed() {
    let mut r = kingdom();
    let geo = geography::geography(&[]);
    let lead = road_at(30.0);
    assert!(!r.could_send(Kind::Squire, &geo, lead));
    let mut unstewarded = bed();
    unstewarded.steward = None;
    r.home.holdings.owned.push(unstewarded);
    assert!(!r.could_send(Kind::Squire, &geo, lead), "a holding with no steward receives nobody");
    r.home.holdings.owned[0].steward = Some("vell".into());
    assert!(r.could_send(Kind::Squire, &geo, lead));
    r.home.holdings.owned[0].folk.push(crate::rules::villagers::Villager {
        name: "Col".into(),
        trade: crate::rules::villagers::Trade::Brewer,
        mood: 1,
        hunger: 0,
        skill: 0,
        practiced: 0,
        age: 30,
        seasons: 0,
    });
    assert!(!r.could_send(Kind::Squire, &geo, lead), "the one bed is taken");
}

#[test]
fn a_rumour_marks_a_real_entrance_and_pays_when_it_is_reached() {
    let mut r = kingdom();
    open(&mut r, Kind::Gossip);
    say(&mut r, 1);
    let room = r.wayfarers.rumoured.expect("a place is marked");
    assert_eq!(r.kingdom.moments.errand, Some(room));
    let geo = geography::geography(&[]);
    let site = geo.sites.iter().find(|s| s.room == room).expect("it is a real door");
    let goal = r.kingdom_goal(1).unwrap();
    assert_eq!(goal.stand, site.stand, "the goal arrow points at it");
    assert!(r.wayfarer_speech(1).unwrap().words.to_lowercase().contains(&site.name.to_lowercase()));
    let score = r.score;
    let index = geo.sites.iter().position(|s| s.room == room).unwrap();
    r.enter_kingdom_place(index);
    assert_eq!(r.score, score + RUMOUR_RENOWN, "the rumour was true");
    assert_eq!(r.wayfarers.rumoured, None);
}

#[test]
fn the_better_tale_costs_gold_and_names_work_that_wants_doing() {
    let mut r = kingdom();
    treasure(&mut r, 100);
    open(&mut r, Kind::Gossip);
    let errands = r.open_errands();
    assert!(!errands.is_empty());
    say(&mut r, 2);
    assert_eq!(r.marks[PAID], BETTER_TALE);
    assert_eq!(r.wayfarers.rumoured, Some(errands[0].0));
    let words = r.wayfarer_speech(1).unwrap().words;
    assert!(words.to_uppercase().contains(errands[0].1), "{words}");

    let mut poor = kingdom();
    open(&mut poor, Kind::Gossip);
    say(&mut poor, 2);
    assert!(!poor.marks.contains_key(PAID), "no gold, no tale");
    assert_eq!(poor.wayfarers.rumoured, None);
}

#[test]
fn walking_away_ends_the_talk_and_a_held_key_answers_once() {
    let mut r = kingdom();
    treasure(&mut r, 500);
    open(&mut r, Kind::Merchant);
    // The key held down answers once, not every tick.
    for _ in 0..20 {
        r.step(&BTreeMap::from([(1, Input { cast: 1, ..Default::default() })]));
    }
    assert_eq!(r.marks[PAID], POTION);
    r.step(&BTreeMap::new());
    put(&mut r, road_at(60.0));
    idle(&mut r, 3);
    assert!(r.wayfarers.talks.is_empty(), "out of earshot");
}

#[test]
fn a_talk_takes_the_spell_keys_so_nothing_is_cast() {
    let mut r = kingdom();
    open(&mut r, Kind::Pilgrim);
    r.wayfarers.keys.clear();
    let held = BTreeMap::from([(1, Input { cast: 1, ..Default::default() })]);
    let map = r.wayfarer_inputs(&held).expect("a talk is open");
    assert_eq!(map[&1].cast, 0, "the key is the talk's");
}

#[test]
fn indoors_the_road_forgets_everyone_but_the_squire() {
    let mut r = kingdom();
    meet(&mut r, Kind::Pilgrim);
    r.wayfarers.talks.insert(1, Talk { page: 1, opened: 0, said: String::new() });
    r.arrive(world::room_of(&r.dungeon, RoomKind::Gate).unwrap(), (12.0, 11.0));
    idle(&mut r, 2);
    assert!(r.wayfarers.on.is_none() && r.wayfarers.talks.is_empty());

    let mut s = kingdom();
    meet(&mut s, Kind::Squire);
    s.wayfarers.on.as_mut().unwrap().stage = Stage::Following;
    s.arrive(world::room_of(&s.dungeon, RoomKind::Gate).unwrap(), (12.0, 11.0));
    idle(&mut s, 2);
    assert!(s.wayfarers.on.is_some(), "she waits for the party to come out");
}

#[test]
fn the_ledger_keeps_who_was_met_and_a_save_without_it_still_loads() {
    let mut r = kingdom();
    meet(&mut r, Kind::Pilgrim);
    press(&mut r);
    assert!(r.wayfarers.on.as_ref().unwrap().met);
    assert_eq!(r.marks["wayfarer:met:pilgrim:Aldous"], 1);
    // A second word with the same traveller is not a second meeting.
    press(&mut r);
    press(&mut r);
    assert_eq!(r.marks["wayfarer:met:pilgrim:Aldous"], 1);

    let mut home = Home::default();
    let mut treasury = Spoils::default();
    let settled = settle(
        &mut Realm { home: &mut home, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 },
        &r.marks,
    );
    assert!(settled.changed);
    assert_eq!(home.wayfarers.met.get("pilgrim"), Some(&1));
    assert_eq!(home.wayfarers.lately, vec!["Brother Aldous".to_string()]);
    assert_eq!(home.wayfarers.last(), Some(Kind::Pilgrim));
    assert!(home.wayfarers.valid());

    let json = serde_json::to_value(&home).unwrap();
    assert_eq!(json["wayfarers"]["met"]["pilgrim"], 1);
    let back: Home = serde_json::from_value(json).unwrap();
    assert_eq!(back.wayfarers, home.wayfarers);
    // A realm saved before the roads had travellers loads, and writes nothing new.
    let mut old = serde_json::to_value(Home::default()).unwrap();
    old.as_object_mut().unwrap().remove("wayfarers");
    let loaded: Home = serde_json::from_value(old).unwrap();
    assert!(loaded.wayfarers.is_empty());
    assert!(serde_json::to_value(&loaded).unwrap().get("wayfarers").is_none());
}

#[test]
fn the_save_charges_the_treasury_only_what_it_holds() {
    let mut home = Home::default();
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 30);
    let marks = BTreeMap::from([(PAID.to_string(), 50), (OWED.to_string(), 60)]);
    let settled = settle(&mut Realm { home: &mut home, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 }, &marks);
    assert!(settled.changed);
    assert_eq!(treasury.get(Spoil::Gold), 60, "30 held, all 30 taken, the 60 purse paid in");
}

#[test]
fn a_run_with_a_traveller_on_the_road_saves_and_loads() {
    let mut r = kingdom();
    meet(&mut r, Kind::Pilgrim);
    press(&mut r);
    assert!(!r.wayfarers.quiet());
    let json = serde_json::to_string(&r).unwrap();
    let back: Run = serde_json::from_str(&json).unwrap();
    assert_eq!(back.wayfarers, r.wayfarers);
    assert!(back.valid_snapshot());
    // A quiet road adds nothing to a save.
    let fresh = serde_json::to_value(kingdom()).unwrap();
    assert!(fresh.get("wayfarers").is_none());
}

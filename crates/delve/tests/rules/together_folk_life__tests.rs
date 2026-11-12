//! The village's temper and craft: a feast when the granary is full, a
//! tantrum one step from walking away, a mourning, a fey craftsman's
//! masterwork a knight can take; who is where in the hall; a word with a
//! soul; the tavern board's tidings. All driven through the rent days the
//! cockpit calls and the steps the host takes.
use super::*;
use crate::realm::Spoils;
use crate::rules::fey::{self, Masterwork};
use crate::rules::holdings::{self, rent_day, Built, Deed, Holding, Holdings, ROOM_LABOUR, WALLS};
use crate::rules::home::Home;
use crate::rules::occasions::{self, Broke, Cause, Dying, Occasion};
use crate::rules::strolls::{self, Pose};
use crate::rules::villagers::{Trade, Villager};
use crate::rules::{Input, Pack};

fn soul(name: &str, trade: Trade, mood: i8, skill: u8) -> Villager {
    Villager { name: name.into(), trade, mood, hunger: 0, skill, practiced: skill * 6, age: 30, seasons: 0 }
}

fn village(rooms: &[&str], stores: u32) -> Holdings {
    let mut h = Holdings::default();
    h.stores = stores;
    h.owned.push(Holding {
        key: Deed::Floor(Pack::Crypt).key(),
        rooms: rooms.iter().map(|id| Built { id: id.to_string(), labour: ROOM_LABOUR }).collect(),
        steward: Some("vell".into()),
        walls: WALLS,
        stars: 1,
        ..Holding::default()
    });
    h
}

fn the(h: &Holdings) -> &Holding {
    &h.owned[0]
}

// ── Occasions ─────────────────────────────────────────────────────────────

#[test]
fn a_full_granary_is_a_feast_that_eats_its_sacks_and_cheers_the_table() {
    let rooms = ["taproom", "smithy", "shrine", "stall"];
    let mut feasting = village(&rooms, 40);
    let mut plain = village(&rooms, 31);
    for h in [&mut feasting, &mut plain] {
        for (name, trade) in [("Berta", Trade::Brewer), ("Col", Trade::Smith), ("Goda", Trade::Acolyte)] {
            h.owned[0].folk.push(soul(name, trade, 0, 0));
        }
    }
    rent_day(&mut feasting, 1);
    rent_day(&mut plain, 1);
    assert_eq!(the(&feasting).occasion, Some(Occasion::Feast));
    assert_eq!(the(&plain).occasion, None, "the granary fell under {}", occasions::FEAST_AT);
    assert_eq!(feasting.stores + occasions::FEAST_SACKS, plain.stores + 9, "a feast eats its sacks: 40-31 apart, less the three");
    let (a, b) = (the(&feasting).folk[0].mood, the(&plain).folk[0].mood);
    assert_eq!(a, b + occasions::FEAST_CHEER);
    assert!(feasting.news.iter().any(|n| n.contains("holds a feast") && n.contains("3 sacks")), "{:?}", feasting.news);
    assert!(feasting.tidings.iter().any(|t| t.contains("HOLDS A FEAST")), "{:?}", feasting.tidings);
    assert!(feasting.valid());
}

#[test]
fn no_feast_for_a_hungry_few_or_a_raided_holding() {
    let rooms = ["taproom", "smithy"];
    let mut few = village(&rooms, 40);
    few.owned[0].folk.push(soul("Berta", Trade::Brewer, 0, 0));
    rent_day(&mut few, 1);
    assert_eq!(the(&few).occasion, None, "one soul is no feast");

    let mut raided = village(&rooms, 40);
    for name in ["Berta", "Col", "Goda"] {
        raided.owned[0].folk.push(soul(name, Trade::Labourer, 0, 0));
    }
    raided.owned[0].raid = 1;
    rent_day(&mut raided, 1);
    assert!(!matches!(the(&raided).occasion, Some(Occasion::Feast)), "no feasting with raiders at the gate");
}

#[test]
fn a_soul_one_step_from_leaving_throws_a_tantrum_that_breaks_their_own_room() {
    // An empty granary: the smith goes hungry, and the smithy pays.
    let mut h = village(&["smithy"], 0);
    h.owned[0].folk.push(soul("Col", Trade::Smith, -2, 1));
    rent_day(&mut h, 1);
    let w = the(&h);
    let Some(Occasion::Tantrum { who, cause, broke }) = &w.occasion else {
        panic!("{:?} {:?}", w.occasion, w.folk);
    };
    assert_eq!((who.as_str(), *cause), ("Col", Cause::Hunger));
    assert_eq!(*broke, Broke::Course(0));
    assert!(!w.rooms[0].standing(), "a course of the smithy is broken");
    assert_eq!(w.rooms[0].labour, ROOM_LABOUR - 1, "one course, no more");
    assert!(h.news.iter().any(|n| n.contains("breaks a course of the")), "{:?}", h.news);
    assert!(h.valid());
}

#[test]
fn a_tantrum_with_no_room_spills_a_sack_and_names_what_soured_it() {
    // Two beds' worth of rooms for three souls: the last has none.
    let mut h = village(&["smithy"], 6);
    h.owned[0].folk.push(soul("Col", Trade::Smith, 3, 1));
    h.owned[0].folk.push(soul("Dunstan", Trade::Labourer, 3, 0));
    h.owned[0].folk.push(soul("Edith", Trade::Labourer, -3, 0));
    let before = h.stores;
    rent_day(&mut h, 1);
    let w = the(&h);
    let Some(Occasion::Tantrum { who, cause, broke }) = &w.occasion else {
        panic!("{:?}", w.occasion);
    };
    assert_eq!((who.as_str(), *cause, *broke), ("Edith", Cause::NoBed, Broke::Sack));
    // The steward's wage, dinner for three, and the one that was spilled.
    assert_eq!(h.stores, before - 1 - 1 - 1);
    assert!(h.news.iter().any(|n| n.contains("Edith") && n.contains("bedless") && n.contains("spills a sack")), "{:?}", h.news);
}

#[test]
fn a_tantrum_in_a_bare_house_finds_nothing_to_break_and_says_so() {
    let mut h = village(&["smithy"], 0);
    h.owned[0].folk.push(soul("Col", Trade::Smith, 3, 1));
    h.owned[0].folk.push(soul("Edith", Trade::Labourer, -1, 0));
    rent_day(&mut h, 1);
    let w = the(&h);
    assert!(matches!(&w.occasion, Some(Occasion::Tantrum { broke: Broke::Nothing, .. })), "{:?}", w.occasion);
    assert!(h.news.iter().any(|n| n.contains("finds nothing left to break")), "{:?}", h.news);
}

#[test]
fn the_old_the_frail_and_the_raided_die_and_the_village_mourns() {
    // Years.
    let mut h = village(&["taproom"], 20);
    h.owned[0].folk.push(Villager { age: 100, ..soul("Uda", Trade::Brewer, 3, 1) });
    h.owned[0].folk.push(soul("Col", Trade::Labourer, 3, 0));
    rent_day(&mut h, 1);
    assert!(matches!(&the(&h).occasion, Some(Occasion::Mourning { who, dying: Dying::Years, .. }) if who == "Uda"), "{:?}", the(&h).occasion);

    // An empty granary starves a child already at the end of their belly.
    let mut h = village(&["taproom"], 0);
    h.owned[0].folk.push(Villager { age: 6, hunger: 3, ..soul("Mabel", Trade::Child, 3, 0) });
    h.owned[0].folk.push(soul("Col", Trade::Labourer, 3, 0));
    rent_day(&mut h, 1);
    assert!(!the(&h).folk.iter().any(|f| f.name == "Mabel"), "the child starved");
    assert!(matches!(&the(&h).occasion, Some(Occasion::Mourning { dying: Dying::Hunger, .. })), "{:?}", the(&h).occasion);
    assert!(h.news.iter().any(|n| n.contains("Mabel") && n.contains("starved") && n.contains("mourned")), "{:?}", h.news);
    assert!(h.tidings.iter().any(|t| t.contains("MABEL")), "{:?}", h.tidings);

    // Hardy grown souls only walk away from hunger.
    let mut h = village(&["taproom"], 0);
    h.owned[0].folk.push(Villager { hunger: 3, ..soul("Col", Trade::Labourer, 3, 0) });
    rent_day(&mut h, 1);
    assert!(the(&h).folk.iter().any(|f| f.name == "Col"), "a grown soul does not starve in a day");

    // Raiders who have held the place two rent days kill a guard first.
    let mut h = village(&["barracks"], 20);
    h.owned[0].folk.push(soul("Col", Trade::Labourer, 3, 0));
    h.owned[0].folk.push(soul("Goda", Trade::Guard, 3, 0));
    h.owned[0].raid = 2;
    rent_day(&mut h, 1);
    assert!(!the(&h).folk.iter().any(|f| f.name == "Goda"), "the guard fell");
    assert!(the(&h).folk.iter().any(|f| f.name == "Col"));
    assert!(matches!(&the(&h).occasion, Some(Occasion::Mourning { dying: Dying::Raiders, .. })), "{:?}", the(&h).occasion);
    assert!(h.valid());
}

#[test]
fn an_occasion_lasts_a_rent_day_and_a_quiet_one_clears_it() {
    let mut h = village(&["taproom"], 20);
    h.owned[0].occasion = Some(Occasion::Feast);
    h.owned[0].folk.push(soul("Col", Trade::Labourer, 0, 0));
    rent_day(&mut h, 1);
    assert_eq!(the(&h).occasion, None, "yesterday's feast is over");
}

#[test]
fn the_boards_tidings_keep_four_and_forget_the_oldest() {
    let mut h = Holdings::default();
    for n in 0..6 {
        h.tell(&format!("line {n}"));
    }
    assert_eq!(h.tidings.len(), occasions::TIDINGS);
    assert_eq!(h.tidings[0], "LINE 2");
    assert_eq!(h.tidings[3], "LINE 5");
    h.tell(&"x".repeat(300));
    assert_eq!(h.tidings.last().map(String::len), Some(occasions::TIDING_LEN));
    assert!(h.valid());
}

#[test]
fn old_saves_without_any_of_it_read_and_new_state_round_trips() {
    let old: Holding = serde_json::from_slice(br#"{"key":"floor:Crypt","walls":10,"stars":1}"#).unwrap();
    assert!(old.occasion.is_none() && old.seized.is_none() && old.works.is_empty());
    let mut w = old;
    w.occasion = Some(Occasion::Tantrum { who: "Col".into(), cause: Cause::NoBed, broke: Broke::Course(2) });
    w.seized = Some(fey::Seizure { who: "Col".into(), trade: Trade::Smith, days: 2 });
    w.works.push(Masterwork { maker: "Berta".into(), trade: Trade::Brewer });
    let text = serde_json::to_string(&w).unwrap();
    let back: Holding = serde_json::from_str(&text).unwrap();
    assert_eq!(back, w);
    let mut h = Holdings::default();
    h.owned.push(w);
    assert!(h.valid());
    // A folk-less, event-less realm writes none of it.
    let quiet = serde_json::to_string(&Holding::default()).unwrap();
    assert!(!quiet.contains("occasion") && !quiet.contains("seized") && !quiet.contains("works"), "{quiet}");
}

// ── The fey craftsman ────────────────────────────────────────────────────

fn smithy_with_master(stores: u32) -> Holdings {
    let mut h = village(&["smithy", "taproom"], stores);
    h.owned[0].folk.push(soul("Col", Trade::Smith, 4, 2));
    h
}

#[test]
fn a_practiced_smith_seizes_the_workshop_pays_in_grain_and_makes_a_named_work() {
    let mut h = smithy_with_master(30);
    rent_day(&mut h, 1);
    let w = the(&h);
    let seized = w.seized.as_ref().expect("the fey mood takes Col");
    assert_eq!((seized.who.as_str(), seized.trade, seized.days), ("Col", Trade::Smith, fey::FEY_DAYS));
    assert!(fey::seized_bay(w, 0) && !fey::seized_bay(w, 1), "the smithy is Col's, the taproom is not");
    assert!(h.news.iter().any(|n| n.contains("Col") && n.contains("fey mood") && n.contains("4 sacks")), "{:?}", h.news);
    let after_seizing = h.stores;
    rent_day(&mut h, 1);
    assert!(the(&h).seized.is_some() && the(&h).works.is_empty(), "two rent days of work");
    assert!(h.stores < after_seizing, "only dinner and wages now, no second payment: {}", h.stores);
    rent_day(&mut h, 1);
    let w = the(&h);
    assert!(w.seized.is_none(), "the workshop is the village's again");
    let work = w.works.first().expect("the work stands finished");
    assert_eq!((work.maker.as_str(), work.trade), ("Col", Trade::Smith));
    let curio = work.curio().expect("a real curio");
    assert!(curio.name.starts_with("Col's "), "{}", curio.name);
    assert!(h.news.iter().any(|n| n.contains(curio.name) && n.contains("the knights may take it")), "{:?}", h.news);
    assert!(h.tidings.iter().any(|t| t.contains("FINISHES")), "{:?}", h.tidings);
    assert!(h.valid());
    // A craftsman makes one masterwork.
    for _ in 0..4 {
        rent_day(&mut h, 1);
    }
    assert!(the(&h).seized.is_none() && the(&h).works.len() == 1);
}

#[test]
fn the_fey_wait_on_the_granary_a_skilled_fed_content_hand_and_a_standing_workshop() {
    // Not enough grain to pay for it.
    let mut h = smithy_with_master(fey::FEY_SACKS + 1);
    rent_day(&mut h, 1);
    assert!(the(&h).seized.is_none(), "{} sacks cannot pay for the work", h.stores);
    // Not practiced enough.
    let mut h = village(&["smithy"], 30);
    h.owned[0].folk.push(soul("Col", Trade::Smith, 4, 1));
    rent_day(&mut h, 1);
    assert!(the(&h).seized.is_none());
    // No workshop standing.
    let mut h = village(&["taproom"], 30);
    h.owned[0].folk.push(soul("Col", Trade::Smith, 4, 2));
    rent_day(&mut h, 1);
    assert!(the(&h).seized.is_none());
    // A guard or a labourer has no workshop to seize.
    let mut h = village(&["barracks"], 30);
    h.owned[0].folk.push(soul("Col", Trade::Guard, 4, 3));
    rent_day(&mut h, 1);
    assert!(the(&h).seized.is_none());
}

#[test]
fn a_craftsman_who_walks_away_leaves_the_work_unmade() {
    let mut h = smithy_with_master(30);
    rent_day(&mut h, 1);
    assert!(the(&h).seized.is_some());
    h.owned[0].folk.clear();
    rent_day(&mut h, 1);
    assert!(the(&h).seized.is_none() && the(&h).works.is_empty());
    assert!(h.news.iter().any(|n| n.contains("unfinished work is left to rust")), "{:?}", h.news);
}

#[test]
fn every_artifact_card_is_written_out_of_the_rules_it_runs() {
    let mut seen = 0;
    for name in crate::rules::villagers::NAMES {
        for trade in [Trade::Brewer, Trade::Smith, Trade::Acolyte, Trade::Trader] {
            let curio = fey::artifact(&fey::id_of(name, trade)).expect("every name's every craft");
            seen += 1;
            assert!(!curio.rules.is_empty() && curio.words.is_empty());
            let (mut good, mut bad) = (0, 0);
            for rule in curio.rules {
                let parts = fey::describe(*rule);
                assert!(!parts.is_empty(), "{} has a rule its card cannot say: {rule:?}", curio.id);
                for (boon, text) in parts {
                    // The card carries the very words the rule gives.
                    assert!(if boon { curio.boon.contains(&text) } else { curio.bane.contains(&text) }, "{}: {text}", curio.id);
                    if boon { good += 1 } else { bad += 1 }
                }
            }
            assert!(good > 0 && bad > 0, "{}: a boon and a bane", curio.id);
            assert_eq!(curio.boon.split(", ").count(), good);
            assert_eq!(curio.bane.split(", ").count(), bad);
            assert!(curio.name.len() <= 40 && curio.name.starts_with(name), "{}", curio.name);
            assert!(crate::rules::cards::not_for_work(curio.name).is_none() && crate::rules::cards::not_for_work(curio.boon).is_none());
        }
    }
    assert_eq!(seen, 32 * 4);
    assert!(fey::artifact("fey-nobody-smith").is_none());
}

#[test]
fn a_masterwork_in_the_pockets_does_exactly_what_its_card_says() {
    let book = crate::rules::cards::Book::builtin();
    let mut h = crate::rules::Hero::new("Tester", 10.0, 10.0);
    let plain = (h.curio_damage(), h.curio_hurt(), h.curio_gold(100), h.curio_pace(), h.curio_wait());
    assert_eq!(plain, (100, 100, 100, 1.0, 100));
    for (name, trade) in [("Berta", Trade::Brewer), ("Col", Trade::Smith), ("Goda", Trade::Acolyte), ("Isolde", Trade::Trader)] {
        let id = fey::id_of(name, trade);
        let curio = fey::artifact(&id).unwrap();
        h.pockets.items = vec![id];
        h.rebonus(&book);
        assert!(h.pockets.valid());
        let share = |f: &dyn Fn(crate::rules::curios::Rule) -> i32| curio.rules.iter().map(|r| f(*r)).sum::<i32>();
        use crate::rules::curios::Rule::*;
        let pct = |shares: i32| (100 + shares).max(10) as u32;
        // At full health (the hero starts hale), Hale's first number applies.
        let dmg = share(&|r| match r { Damage(n) => n, Hale { full, .. } => full, _ => 0 });
        assert_eq!(h.curio_damage(), pct(dmg), "{}", curio.name);
        assert_eq!(h.curio_hurt(), pct(share(&|r| if let Hurt(n) = r { n } else { 0 })), "{}", curio.name);
        assert_eq!(h.curio_gold(100), pct(share(&|r| if let Gold(n) = r { n } else { 0 })), "{}", curio.name);
        assert!((h.curio_pace() - pct(share(&|r| if let Pace(n) = r { n } else { 0 })) as f32 / 100.0).abs() < 1e-6, "{}", curio.name);
        assert_eq!(h.curio_wait(), 10_000 / pct(share(&|r| if let Fire(n) = r { n } else { 0 })), "{}", curio.name);
        assert_eq!(h.curio_coupon(), share(&|r| if let Coupon(n) = r { n as i32 } else { 0 }).max(0) as u32, "{}", curio.name);
    }
}

// ── The hall ─────────────────────────────────────────────────────────────

fn below(holdings: Holdings) -> Run {
    let home = Home { holdings, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    run.dungeon.pack = Pack::Crypt;
    run.descend_for_test();
    run.dare = None;
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Holding).unwrap();
    run.arrive(i, (12.0, 8.0));
    run.enemies.clear();
    run
}

fn place(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let hero = run.players.get_mut(&id).unwrap();
    (hero.x, hero.y) = (x * TILE_UNITS, y * TILE_UNITS);
    hero.invulnerable = 0;
}

fn press(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { interact: true, ..Input::default() })])
}

fn tap(run: &mut Run, id: u32) {
    run.step(&BTreeMap::new());
    run.step(&press(id));
    run.step(&BTreeMap::new());
}

fn full_village() -> Holdings {
    let mut h = village(&["taproom", "smithy", "shrine", "stall", "barracks"], 20);
    h.owned[0].rooms.push(Built { id: "greathall".into(), labour: 2 });
    let w = &mut h.owned[0];
    w.folk.push(soul("Berta", Trade::Brewer, 3, 1));
    w.folk.push(soul("Col", Trade::Smith, 0, 1));
    w.folk.push(soul("Goda", Trade::Acolyte, -2, 0));
    w.folk.push(soul("Isolde", Trade::Trader, 1, 0));
    w.folk.push(soul("Harold", Trade::Guard, 1, 0));
    w.folk.push(soul("Jory", Trade::Labourer, 1, 0));
    w.folk.push(Villager { age: 7, ..soul("Mabel", Trade::Child, 2, 0) });
    w.folk.push(Villager { age: 70, ..soul("Uda", Trade::Brewer, 1, 2) });
    h
}

#[test]
fn everyone_about_the_hall_is_somebody_in_the_ledger_at_their_trade() {
    let h = full_village();
    let held = the(&h);
    for tick in [0u64, 37, 400, 1000, 5555] {
        let all = strolls::strollers(held, tick);
        assert_eq!(all.len(), held.folk.len(), "one figure a soul, no more");
        for (i, s) in all.iter().enumerate() {
            assert_eq!(s.who, i);
            assert!((1.0..23.0).contains(&s.x) && (1.0..13.0).contains(&s.y), "{:?} at {tick}", s);
        }
        assert_eq!(all, strolls::strollers(held, tick), "a pure function of the state and the tick");
    }
    // A craftsman works at their own room's bay: the smith at the smithy (bay 1).
    let spot = strolls::work_spot(1);
    let at_bench = (0..strolls::DAY).step_by(10).any(|t| {
        let s = strolls::strollers(held, t)[1];
        s.pose == Pose::Work && (s.x, s.y) == spot
    });
    assert!(at_bench, "Col is found at the smithy's bench");
    // A labourer carries timber to the room going up (bay 5), laden on the way.
    let laden = (0..strolls::DAY).step_by(10).any(|t| {
        let s = strolls::strollers(held, t)[5];
        s.laden && s.pose == Pose::Walk
    });
    assert!(laden, "Jory carries to the rising greathall");
    // The child plays; the old keep the fireside.
    assert!((0..300).any(|t| strolls::strollers(held, t)[6].pose == Pose::Play));
    assert!((0..300).all(|t| strolls::strollers(held, t)[7].pose == Pose::Seated));
}

#[test]
fn a_feast_seats_the_village_a_tantrum_stops_the_work_a_mourning_walks_the_bier() {
    let mut h = full_village();
    h.owned[0].occasion = Some(Occasion::Feast);
    let at = strolls::strollers(the(&h), 100);
    let seated = at.iter().filter(|s| matches!(s.pose, Pose::Seated | Pose::Toast)).count();
    assert!(seated >= 7, "the grown souls are at the table: {seated}");
    assert!((0..600).step_by(30).any(|t| strolls::strollers(the(&h), t).iter().any(|s| s.pose == Pose::Toast)), "cups are raised");

    h.owned[0].occasion = Some(Occasion::Tantrum { who: "Col".into(), cause: Cause::Hunger, broke: Broke::Course(1) });
    for t in (0..1200).step_by(7) {
        let all = strolls::strollers(the(&h), t);
        assert_eq!(all[1].pose, Pose::Rage, "Col rages at their bench");
        assert!(all.iter().enumerate().all(|(i, s)| i == 1 || s.pose != Pose::Work), "nobody else works while it goes on");
    }

    h.owned[0].occasion = Some(Occasion::Mourning { who: "Nyle".into(), trade: Trade::Smith, dying: Dying::Years });
    let all = strolls::strollers(the(&h), 90);
    let bearers: Vec<_> = all.iter().filter(|s| s.pose == Pose::Bear).collect();
    assert_eq!(bearers.len(), 2, "two bear the bier");
    assert!(all.iter().filter(|s| s.pose == Pose::Mourn).count() >= 2, "the rest walk behind");
    let ((bx, by), _) = strolls::bier(90);
    assert!(bearers.iter().all(|s| (s.x - bx).abs() < 1.2 && (s.y - by).abs() < 0.1));
    // The bier crosses the hall and comes back.
    let (a, _) = strolls::bier(0);
    let (b, _) = strolls::bier(14 * 30 - 1);
    assert!(b.0 > a.0 + 10.0);
    assert!(strolls::bier(14 * 30 + 5).0.0 < b.0);
}

#[test]
fn a_seized_craftsman_never_leaves_the_bench_and_the_maker_of_a_work_keeps_vigil() {
    let mut h = smithy_with_master(30);
    rent_day(&mut h, 1);
    let spot = strolls::work_spot(0);
    for t in (0..1200).step_by(13) {
        let s = strolls::strollers(the(&h), t)[0];
        assert_eq!((s.x, s.y, s.pose), (spot.0, spot.1, Pose::Work));
    }
    rent_day(&mut h, 2);
    for t in (0..1200).step_by(13) {
        let s = strolls::strollers(the(&h), t)[0];
        assert_eq!((s.x, s.y, s.pose), (spot.0, spot.1, Pose::Vigil));
    }
}

#[test]
fn a_word_with_a_soul_is_said_from_their_own_state() {
    let mut h = full_village();
    h.owned[0].folk[1].hunger = 2;
    h.stores = 0;
    let held = the(&h).clone();
    let col = holdings_words(&h, &held, 1);
    assert!(col.starts_with("COL THE SMITH: HUNGRY. THE GRANARY IS BARE"), "{col}");
    assert!(holdings_words(&h, &held, 6).contains("AT PLAY. 7 YEARS OLD"));
    let old = Holding { rooms: vec![Built { id: "taproom".into(), labour: ROOM_LABOUR }], folk: vec![Villager { age: 70, ..soul("Uda", Trade::Brewer, 1, 2) }], ..Holding::default() };
    assert!(holdings_words(&h, &old, 0).contains("BY THE FIRE"), "{}", holdings_words(&h, &old, 0));
    // Goda, glum, but with a bed, fed, and a calm realm.
    let goda = holdings_words(&h, &held, 2);
    assert!(goda.contains("GLUM"), "{goda}");
    // Souls past the standing rooms' beds have none.
    let mut few = held.clone();
    few.rooms.truncate(2);
    assert!(holdings_words(&h, &few, 4).contains("NO BED"), "{}", holdings_words(&h, &few, 4));
}

fn holdings_words(h: &Holdings, held: &Holding, i: usize) -> String {
    crate::rules::folk_talk::words(h, held, i)
}

#[test]
fn e_beside_a_soul_speaks_with_them_and_a_plate_keeps_its_own_press() {
    let mut run = below({
        let mut h = full_village();
        h.owned[0].rooms.pop();
        h
    });
    // Find a tick where Harold the guard is idle at an end of his watch, or
    // simply stand at his place whenever he is: ask the rules where he is.
    let tick = run.tick;
    let held = the(&run.home.holdings).clone();
    let s = strolls::strollers(&held, tick)[4];
    place(&mut run, 1, (s.x, s.y + 0.8));
    // Not on a plate: that tile must be free.
    tap(&mut run, 1);
    let line = run.found.as_ref().map(|f| f.2.clone()).unwrap_or_default();
    assert!(line.contains(":"), "a soul answered: {line:?}");
    // On the steward's plate the press is the plate's.
    run.found = None;
    place(&mut run, 1, (12.0, 7.5));
    run.step(&BTreeMap::new());
    run.step(&press(1));
    assert!(run.found.as_ref().is_none_or(|f| !f.2.contains("THE ")), "a plate keeps its press");
    assert!(run.valid_snapshot());
}

#[test]
fn a_knight_takes_the_masterwork_from_its_maker_one_at_a_time_and_it_is_back_next_run() {
    let mut h = smithy_with_master(30);
    rent_day(&mut h, 3);
    assert_eq!(the(&h).works.len(), 1);
    let id = fey::id_of("Col", Trade::Smith);
    let name = fey::artifact(&id).unwrap().name.to_uppercase();
    let mut run = below(h.clone());
    let held = the(&run.home.holdings).clone();
    let s = strolls::strollers(&held, run.tick)[0];
    place(&mut run, 1, (s.x, s.y + 1.1));
    assert!(run.players[&1].pockets.items.is_empty());
    tap(&mut run, 1);
    let line = run.found.as_ref().map(|f| f.2.clone()).unwrap_or_default();
    assert_eq!(run.players[&1].pockets.items, vec![id.clone()], "{line}");
    assert!(line.starts_with(&name) && line.contains("+ ") && line.contains("- "), "{line}");
    assert!(run.valid_snapshot());
    // It does what it says in the run.
    assert_ne!(run.players[&1].curio_damage() * run.players[&1].curio_hurt(), 100 * 100);

    // A friend finds it with the first knight.
    run.join_seat(2, "Friend");
    place(&mut run, 2, (s.x, s.y + 1.1));
    tap(&mut run, 2);
    let line = run.found.as_ref().map(|f| f.2.clone()).unwrap_or_default();
    assert!(run.players[&2].pockets.items.is_empty(), "{line}");
    assert!(line.contains("CARRIES THE"), "{line}");

    // A knight with full pockets is told so.
    let mut full = below(h.clone());
    full.players.get_mut(&1).unwrap().pockets.items = ["boots-mild-inconvenience", "spoon-unreasonable-confidence", "hat-also-pigeon", "cape-dramatic-entrances", "pendant-pessimist", "soup-pot-helm"].map(String::from).to_vec();
    let s = strolls::strollers(the(&full.home.holdings), full.tick)[0];
    place(&mut full, 1, (s.x, s.y + 1.1));
    tap(&mut full, 1);
    assert_eq!(full.players[&1].pockets.items.len(), 6);
    assert!(full.found.as_ref().is_some_and(|f| f.2.contains("POCKETS ARE FULL")));

    // Next run, the work is in the workshop again.
    let again = below(h);
    assert_eq!(the(&again.home.holdings).works.len(), 1);
    assert!(again.players[&1].pockets.items.is_empty());
}

#[test]
fn a_seized_workshop_gives_the_party_nothing_until_the_work_is_done() {
    let mut h = smithy_with_master(30);
    rent_day(&mut h, 1);
    let mut run = below(h.clone());
    run.players.get_mut(&1).unwrap().mana = 0.0;
    // The smithy is bay 0 (its boon is a heal); the taproom bay 1 (a brawler).
    let hurt = |run: &mut Run| run.players.get_mut(&1).unwrap().hp = 10;
    hurt(&mut run);
    for _ in 0..crate::rules::home::BUY_HOLD + 4 {
        place(&mut run, 1, (3.5, 3.5));
        run.step(&press(1));
    }
    assert_eq!(run.holding_used & 1, 0, "the seized smithy served nobody");
    assert_eq!(run.players[&1].hp, 10);
    rent_day(&mut h, 2);
    let mut run = below(h);
    hurt(&mut run);
    for _ in 0..crate::rules::home::BUY_HOLD + 4 {
        place(&mut run, 1, (3.5, 3.5));
        run.step(&press(1));
    }
    assert_eq!(run.holding_used & 1, 1, "finished, it serves again");
}

#[test]
fn walking_into_a_hall_in_the_middle_of_something_says_what_and_the_knights_chatter() {
    let mut h = full_village();
    h.owned[0].occasion = Some(Occasion::Mourning { who: "Nyle".into(), trade: Trade::Smith, dying: Dying::Years });
    let run = below(h);
    let line = run.found.as_ref().map(|f| f.2.clone()).unwrap_or_default();
    assert!(line.contains("THE VILLAGE MOURNS NYLE THE SMITH, DEAD OF OLD AGE"), "{line}");
    assert!(run.cues.iter().any(|c| c == "folk_mourning"));
    // A quiet village says nothing.
    let quiet = below(full_village());
    assert!(!quiet.cues.iter().any(|c| c.starts_with("folk_")));
}

// ── The tavern's board ───────────────────────────────────────────────────

fn tavern(h: Holdings) -> Run {
    let home = Home { holdings: h, ..Home::default() };
    let mut r = Run::at_home(41, 1, Some("Friend"), home, Spoils::default());
    r.walk_alehouse();
    r.floor_plates.clear();
    r
}

fn read_board(r: &mut Run) -> String {
    let at = crate::rules::alehouse::TOYS.iter().find(|(toy, _)| *toy == crate::rules::alehouse::Toy::Board).unwrap().1;
    place(r, 1, at);
    r.step(&BTreeMap::new());
    r.step(&press(1));
    r.step(&BTreeMap::new());
    r.alehouse.said.get(&1).map(|s| s.text.clone()).unwrap_or_default()
}

#[test]
fn the_taverns_board_pins_the_folks_news_over_its_old_notes_newest_first() {
    let mut h = village(&["taproom"], 40);
    h.tell("Mabel the child has starved at The Crypt, mourned at The Crypt");
    h.tell("The granary is full: The Crypt holds a feast, 3 sacks eaten");
    let mut r = tavern(h);
    let first = read_board(&mut r);
    assert!(first.starts_with("THE GRANARY IS FULL"), "{first}");
    let second = read_board(&mut r);
    assert!(second.starts_with("MABEL THE CHILD HAS STARVED"), "{second}");
    // Then the old notes, as ever.
    let third = read_board(&mut r);
    assert!(third.contains(" | ") && !third.contains("MABEL") && !third.contains("GRANARY IS FULL"), "{third}");
    // A realm with no news reads as it always did.
    let mut r = tavern(Holdings::default());
    let plain = read_board(&mut r);
    assert!(plain.contains(" | "), "{plain}");
    assert!(r.valid_snapshot());
}

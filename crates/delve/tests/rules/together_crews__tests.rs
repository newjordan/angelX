// The holdings' crews: hands who walk a trip a shift from the yard to the
// work, the steward's standing order that says which work comes first, and
// the one clock the rules and the picture keep.

use super::*;
use super::testing::*;
use crate::rules::home::{BUY_HOLD, Home};
use crate::rules::world;

fn built(ids: &[&str]) -> Vec<Built> {
    ids.iter().map(|id| Built { id: id.to_string(), labour: ROOM_LABOUR }).collect()
}

fn trips(h: &mut Holdings, key: &str, n: u32) -> Settled {
    let mut t = gold(0);
    settle(h, &mut t, &BTreeMap::from([(trip_mark(key), n)]))
}

// ── The crew ──────────────────────────────────────────────────────────────

#[test]
fn a_holding_is_bought_with_two_hands_and_rooms_that_quarter_folk_bring_more() {
    let (mut h, _) = held();
    assert_eq!(h.owned[0].crew(), HANDS);
    // Inn rooms going up quarter nobody yet.
    h.owned[0].rooms.push(Built { id: "inn".into(), labour: 2 });
    assert_eq!(h.owned[0].crew(), HANDS);
    h.owned[0].rooms.last_mut().unwrap().labour = ROOM_LABOUR;
    assert_eq!(h.owned[0].crew(), HANDS + 1);
    h.owned[0].rooms = built(&["inn", "barracks", "greathall", "garrison", "taproom"]);
    assert_eq!(h.owned[0].crew(), CREW_CAP);
    // Raiders in it: the crew has scattered.
    h.owned[0].raid = 1;
    assert_eq!(h.owned[0].crew(), 0);
    assert_eq!(h.owned[0].work(0), None);
}

#[test]
fn the_crew_works_its_order_first_and_falls_through_to_what_there_is() {
    let (mut h, _) = held();
    let w = &mut h.owned[0];
    // All whole, the granary bare: they stock it.
    assert_eq!(w.order, Work::Build);
    assert_eq!(w.work(0), Some(Work::Stock));
    // A room going up comes first on a building order.
    w.rooms.push(Built { id: "smithy".into(), labour: 0 });
    w.walls = 6;
    assert_eq!(w.work(0), Some(Work::Build));
    // Ordered to mend, the walls come before the room.
    w.order = Work::Mend;
    assert_eq!(w.work(0), Some(Work::Mend));
    w.walls = WALLS;
    assert_eq!(w.work(0), Some(Work::Build), "walls whole: on to the room");
    // Ordered to stock, the granary first, until it is full.
    w.order = Work::Stock;
    assert_eq!(w.work(0), Some(Work::Stock));
    assert_eq!(w.work(STORES_CAP), Some(Work::Build));
    // Nothing to do at all: they rest.
    w.rooms.last_mut().unwrap().labour = ROOM_LABOUR;
    assert_eq!(w.work(STORES_CAP), None);
}

#[test]
fn trips_add_up_to_courses_of_a_room_courses_of_wall_and_sacks() {
    let (mut h, _) = held();
    let key = crypt_key();
    h.owned[0].rooms.push(Built { id: "smithy".into(), labour: 0 });
    // A course of a room is four trips.
    trips(&mut h, &key, COURSE_TRIPS - 1);
    assert_eq!(h.owned[0].rooms[2].labour, 0);
    trips(&mut h, &key, 1);
    assert_eq!(h.owned[0].rooms[2].labour, 1);
    // The room stands at six courses, and says so.
    let out = trips(&mut h, &key, COURSE_TRIPS * (ROOM_LABOUR - 1));
    assert!(h.owned[0].rooms[2].standing());
    assert!(out.said.iter().any(|(_, l)| l.contains("Smithy stands at The Crypt")), "{:?}", out.said);
    // Walls: twelve trips a course.
    h.owned[0].walls = 6;
    trips(&mut h, &key, MEND_TRIPS * 2);
    assert_eq!(h.owned[0].walls, 8);
    // The granary: twelve trips a sack, once the walls are whole.
    h.owned[0].walls = WALLS;
    trips(&mut h, &key, STOCK_TRIPS * 3);
    assert_eq!(h.stores, 3);
    assert_eq!(h.owned[0].toil, 0);
    // Never gold, never the coffer.
    assert_eq!(h.owned[0].coffer, 0);
}

#[test]
fn a_shrewd_steward_drives_the_builders_twice_as_hard_and_no_one_else() {
    let (mut h, _) = held();
    let key = crypt_key();
    h.owned[0].steward = Some("vell".into());
    h.owned[0].rooms.push(Built { id: "smithy".into(), labour: 0 });
    trips(&mut h, &key, COURSE_TRIPS * ROOM_LABOUR / 2);
    assert!(h.owned[0].rooms[2].standing());
    h.owned[0].walls = 9;
    trips(&mut h, &key, MEND_TRIPS / 2);
    assert_eq!(h.owned[0].walls, 9, "she drives builders, not masons");
}

#[test]
fn unrest_halves_the_crew_and_a_riot_downs_tools() {
    let (mut h, _) = held();
    h.owned[0].rooms = built(&["inn", "barracks", "taproom"]);
    let w = h.owned[0].clone();
    assert_eq!(h.hands(&w), 4);
    h.unrest = RESTLESS;
    assert_eq!(h.hands(&w), 2);
    h.unrest = UNREST_CAP;
    assert_eq!(h.hands(&w), 0);
    assert_eq!(h.crew_work(&w), None);
}

// ── One clock ─────────────────────────────────────────────────────────────

#[test]
fn a_hand_is_back_at_the_work_laden_the_tick_its_trip_lands() {
    let shift = u64::from(SHIFT_SECS * HZ);
    for hands in 1..=CREW_CAP {
        for j in 0..hands {
            let landing: Vec<u64> = (0..shift * 3).filter(|&t| lands(t, j, hands)).collect();
            assert_eq!(landing.len(), 3, "once a shift: {j}/{hands}");
            for &t in &landing {
                let l = leg(t, j, hands);
                assert_eq!((l.along, l.laden, l.working), (0.0, true, true), "setting it down");
            }
            // Half a shift on, out at the yard or on the way.
            let l = leg(landing[0] + shift / 2, j, hands);
            assert!(l.along > 0.5 && !l.laden, "{l:?}");
            // Coming back: laden, nearer the work with every tick.
            let a = leg(landing[0] + shift * 7 / 10, j, hands);
            let b = leg(landing[0] + shift * 9 / 10, j, hands);
            assert!(a.laden && b.laden && b.along < a.along);
        }
        // The hands of a crew land apart, never in step.
        let mut at: Vec<u64> = (0..hands).map(|j| (0..shift).find(|&t| lands(t, j, hands)).unwrap()).collect();
        at.dedup();
        assert_eq!(at.len(), hands as usize);
    }
}

fn court(holdings: Holdings, purse: u32) -> Run {
    let home = Home { holdings, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, gold(purse));
    let i = world::room_of(&run.dungeon, RoomKind::LandOffice).expect("the Court on the home floor");
    run.arrive(i, (12.0, 12.0));
    run.step(&BTreeMap::new());
    run
}

fn cockpit(run: &mut Run) -> Settled {
    let marks = std::mem::take(&mut run.marks);
    let mut h = run.home.holdings.clone();
    let mut t = run.treasury.clone();
    let out = settle(&mut h, &mut t, &marks);
    run.home.holdings = h;
    run.treasury = t;
    out
}

#[test]
fn the_run_marks_each_hands_trip_as_it_lands_wherever_the_party_is() {
    let (mut h, _) = held();
    h.owned[0].rooms = built(&["inn", "taproom"]);
    h.owned[0].rooms.push(Built { id: "smithy".into(), labour: 0 });
    let mut run = court(h, 0);
    run.marks.clear();
    let start = run.tick;
    let shift = u64::from(SHIFT_SECS * HZ);
    for _ in 0..shift {
        run.step(&BTreeMap::new());
    }
    // Three hands (two, and the inn's), a trip each a shift.
    let key = crypt_key();
    assert_eq!(run.marks.get(&trip_mark(&key)), Some(&3), "{:?} from {start}", run.marks);
    cockpit(&mut run);
    assert_eq!(run.home.holdings.owned[0].toil, 3);
    // A crew with nothing to do marks nothing.
    let mut idle = run.home.holdings.clone();
    idle.owned[0].rooms.last_mut().unwrap().labour = ROOM_LABOUR;
    idle.stores = STORES_CAP;
    let mut run = court(idle, 0);
    run.marks.clear();
    for _ in 0..shift {
        run.step(&BTreeMap::new());
    }
    assert!(!run.marks.contains_key(&trip_mark(&key)));
}

#[test]
fn the_order_plate_turns_the_boards_holdings_order_and_says_it() {
    let (h, _) = held();
    let mut run = court(h, 0);
    let hold = |run: &mut Run| {
        let (c, r, w, hh) = ORDER;
        let hero = run.players.get_mut(&1).unwrap();
        hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
        hero.y = (r as f32 + hh as f32 / 2.0) * TILE_UNITS;
        let e = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
        for _ in 0..BUY_HOLD + 1 {
            run.step(&e);
        }
        run.step(&BTreeMap::new());
    };
    assert_eq!(plate_under(&run, 1), None);
    hold(&mut run);
    let out = cockpit(&mut run);
    assert_eq!(run.home.holdings.owned[0].order, Work::Mend);
    assert!(out.said.iter().any(|(_, l)| l == "The Crypt's crew will mend first"), "{:?}", out.said);
    hold(&mut run);
    cockpit(&mut run);
    assert_eq!(run.home.holdings.owned[0].order, Work::Stock);
    hold(&mut run);
    cockpit(&mut run);
    assert_eq!(run.home.holdings.owned[0].order, Work::Build);
    // A deed the realm does not hold has no crew to order.
    run.home.holdings.pick = 1;
    assert_eq!(plate_open(&run, Plate::Orders), Err("NOT OURS"));
}

#[test]
fn an_old_holding_loads_with_a_building_order_and_saves_as_it_was() {
    let old = r#"{"key":"floor:Crypt","fief":false,"rooms":[{"id":"taproom","labour":6}],"steward":null,"walls":7,"coffer":40,"stars":1,"raid":0}"#;
    let w: Holding = serde_json::from_str(old).unwrap();
    assert_eq!((w.order, w.toil), (Work::Build, 0));
    let again = serde_json::to_string(&w).unwrap();
    assert!(!again.contains("order") && !again.contains("toil"), "{again}");
    let mut w = w;
    w.order = Work::Mend;
    w.toil = 5;
    let saved = serde_json::to_string(&w).unwrap();
    assert!(saved.contains(r#""order":"mend""#) && saved.contains(r#""toil":5"#), "{saved}");
    assert_eq!(serde_json::from_str::<Holding>(&saved).unwrap(), w);
}

/// The evidence the crews' numbers were picked from: a floor takes a bot
/// four to eight minutes (BARONY.md), so a rent day is 24 to 48 shifts.
/// At six minutes a floor, a crew of two raises a room and more in a
/// floor, or six courses of wall, and the order decides which comes first
/// when a room and a broken wall want the same hands.
#[test]
fn a_floors_play_raises_a_room_or_mends_a_siege_and_the_order_says_which_first() {
    let floor = 36;
    let ride = |order: Work| {
        let (mut h, _) = held();
        let key = crypt_key();
        h.owned[0].order = order;
        h.owned[0].walls = 6;
        h.owned[0].rooms.push(Built { id: "smithy".into(), labour: 0 });
        let hands = h.hands(&h.owned[0].clone());
        let mut stood_at = None;
        let mut whole_at = None;
        for shift in 1..=floor {
            trips(&mut h, &key, hands);
            if stood_at.is_none() && h.owned[0].rooms[2].standing() {
                stood_at = Some(shift);
            }
            if whole_at.is_none() && h.owned[0].walls == WALLS {
                whole_at = Some(shift);
            }
        }
        (stood_at, whole_at)
    };
    let (room, walls) = ride(Work::Build);
    assert_eq!(room, Some(12), "a room in two minutes, the walls after");
    assert!(walls.is_some_and(|w| w > 12), "{walls:?}");
    let (room, walls) = ride(Work::Mend);
    assert_eq!(walls, Some(24), "four courses first");
    assert!(room.is_some_and(|r| r > 24 && r <= floor), "{room:?}");
}

// ── Pictures ──────────────────────────────────────────────────────────────

fn below(holdings: Holdings) -> Run {
    let home = Home { holdings, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, gold(0));
    run.dungeon.pack = Pack::Crypt;
    run.descend_for_test();
    run.dare = None;
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Holding).unwrap();
    run.arrive(i, (12.0, 8.0));
    run.enemies.clear();
    run
}

fn stand(run: &mut Run, (c, r, w, h): (i32, i32, i32, i32)) {
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
    hero.invulnerable = 0;
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_crew_shots -- --ignored`
#[test]
#[ignore]
fn write_crew_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("crews");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let shift = u64::from(SHIFT_SECS * HZ);
    // The Crypt with three rooms standing and an inn's crew of three
    // raising a smithy: beams from the yard in the next bay.
    let (mut h, _) = held();
    h.owned[0].rooms = built(&["taproom", "shrine", "inn"]);
    h.owned[0].rooms.push(Built { id: "smithy".into(), labour: 2 });
    h.owned[0].steward = Some("aled".into());
    h.stores = 12;
    let mut run = court(h.clone(), 3000);
    run.tick = shift * 40 + shift * 3 / 4;
    save(&run, "a-court-crew-building");
    // A strip of the board's bays across one shift, a frame a second.
    for k in 0..10u64 {
        run.tick = shift * 41 + k * shift / 10;
        save(&run, &format!("strip-{k}"));
    }
    // The same crew ordered to mend after a siege, a knight on the order
    // plate reading it.
    let mut m = h.clone();
    m.owned[0].walls = 6;
    m.owned[0].order = Work::Mend;
    let mut run = court(m.clone(), 3000);
    run.tick = shift * 40 + shift / 3;
    stand(&mut run, ORDER);
    run.step(&BTreeMap::new());
    run.players.get_mut(&1).unwrap().buying = BUY_HOLD / 2;
    save(&run, "b-court-order-plate-mend");
    // Restless: half the crew at work, the line in red.
    let mut r = m.clone();
    r.unrest = RESTLESS + 1;
    let mut run = court(r, 3000);
    run.tick = shift * 40 + shift * 4 / 5;
    save(&run, "c-court-restless-half-crew");
    // Below, in the Crypt's steward's hall: the crew building, then
    // mending at the door, then stocking the granary.
    for (name, order, walls, frac) in [
        ("d-hall-building", Work::Build, WALLS, 0.8),
        ("e-hall-mending", Work::Mend, 6, 0.75),
        ("f-hall-stocking", Work::Stock, WALLS, 0.3),
    ] {
        let mut b = h.clone();
        b.owned[0].order = order;
        b.owned[0].walls = walls;
        b.stores = 3;
        let mut run = below(b);
        stand(&mut run, STEWARD_PLATE);
        run.tick = shift * 50 + (shift as f64 * frac) as u64;
        save(&run, name);
    }
}

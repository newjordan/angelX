// Extensions to the pre-existing world host/mirror/save regression entry point.
fn northern_adventure_regression(mut run: Run) {
    use super::super::{countryside as c, pastimes::{self, Game}};
    use crate::realm::{Spoil, Spoils};
    use super::super::{Grid, HERO_RADIUS, Mover};
    let fire = |id| BTreeMap::from([(id, Input { fire: true, ..Default::default() })]);
    // The lenses and the clerk answer the interact key.
    let meet = |id| BTreeMap::from([(id, Input { interact: true, ..Default::default() })]);
    run.join_seat(3, "Postage");
    run.join_seat(4, "Stamp");
    run.home.countryside = c::Country::default();
    run.marks.clear();

    // Walk every new road using actual movement, not an arrive() shortcut.
    for &(a, b) in TRAILS.iter().filter(|(a, b)| c::NORTH_PLACES.contains(a) || c::NORTH_PLACES.contains(b)) {
        for (a, b) in [(a, b), (b, a)] {
            let ai = room_of(&run.dungeon, a).unwrap();
            let bi = room_of(&run.dungeon, b).unwrap();
            let ac = run.dungeon.rooms[ai].cell;
            let bc = run.dungeon.rooms[bi].cell;
            let side = DIRS.iter().position(|&(x, y)| (ac.0 + x, ac.1 + y) == bc).unwrap();
            run.enter(ai, Some((side + 2) % 4));
            let (x, y) = match side {
                0 => (run.room().width()/2.0, 1.9),
                1 => (run.room().width()-1.9, run.room().height()/2.0),
                2 => (run.room().width()/2.0, run.room().height()-1.9),
                _ => (1.9, run.room().height()/2.0),
            };
            // A doorway takes the whole party (`fellowship`): all four walk it.
            for h in run.players.values_mut() {
                (h.x, h.y) = (x, y);
            }
            let (mx, my) = DIRS[side];
            let input: BTreeMap<u32, Input> = run.players.keys().map(|&id| (id, Input { move_x: mx as i8, move_y: my as i8, ..Default::default() })).collect();
            for _ in 0..HZ {
                run.step(&input);
                if run.at == bi { break; }
            }
            assert_eq!(run.at, bi, "walk {a:?} -> {b:?}");
            for hero in run.players.values() {
                assert!(Grid { room: run.room(), barred:false }.clear(hero.x, hero.y, HERO_RADIUS, Mover::Hero), "{b:?} four-seat arrival");
            }
            let friend = run.wire_copy();
            assert_eq!(friend.at, bi);
        }
    }

    // Wrong-order lenses do nothing. Each teammate may do the next step.
    run.arrive(room_of(&run.dungeon, RoomKind::Observatory).unwrap(), c::FORECAST[4].1);
    run.tick_countryside(&meet(1));
    assert_eq!(c::forecast_stage(&run.home.countryside, &run.marks), 0);
    for (i, &(kind, at)) in c::FORECAST.iter().enumerate() {
        run.arrive(room_of(&run.dungeon, kind).unwrap(), at);
        for h in run.players.values_mut() { (h.x, h.y) = (at.0 * TILE_UNITS, at.1 * TILE_UNITS); }
        run.tick_countryside(&meet(i as u32 % 4 + 1));
        assert_eq!(c::forecast_stage(&run.home.countryside, &run.marks), i as u8 + 1);
        for _ in 0..3 { run.tick_countryside(&meet(1)); }
        assert_eq!(c::forecast_stage(&run.home.countryside, &run.marks), i as u8 + 1, "holding E cannot skip a lens");
        let restored: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
        assert!(restored.valid_snapshot());
        assert!(restored.marks.is_empty(), "raw Run omits private intents; the app journal tests restart durability");
        let mut committed = run.clone();
        c::settle(&mut committed.home.countryside, &mut committed.treasury, &run.marks);
        let restored: Run = serde_json::from_slice(&serde_json::to_vec(&committed).unwrap()).unwrap();
        assert_eq!(restored.home.countryside.forecast, i as u8 + 1);
    }
    let mut treasury = Spoils::default();
    let mut country = c::Country::default();
    c::settle(&mut country, &mut treasury, &run.marks);
    assert_eq!(country.forecast, 6);
    assert!(country.forecast_paid);
    assert_eq!(treasury.get(Spoil::Gold), 55);
    assert_eq!(treasury.get(Spoil::Ore), 3);
    let paid = treasury.clone();
    c::settle(&mut country, &mut treasury, &run.marks);
    assert_eq!(treasury, paid);
    let request = BTreeMap::from([("country:build:ropeway".into(), 1)]);
    let mut locked = c::Country::default();
    let mut rich = Spoils::default();
    rich.add(Spoil::Gold, 500); rich.add(Spoil::Ore, 20); rich.add(Spoil::Wax, 20);
    let untouched = rich.clone();
    c::settle(&mut locked, &mut rich, &request);
    assert!(!locked.has(6)); assert_eq!(rich, untouched);
    c::settle(&mut country, &mut treasury, &request);
    assert!(!country.has(6), "cannot afford ropeway yet");
    treasury.add(Spoil::Gold, 55); treasury.add(Spoil::Wax, 2);
    c::settle(&mut country, &mut treasury, &request);
    assert!(country.has(6)); assert!(country.valid());
    assert_eq!(treasury.get(Spoil::Gold), 0);
    assert_eq!(treasury.get(Spoil::Ore), 0);
    assert_eq!(treasury.get(Spoil::Wax), 0);
    c::settle(&mut country, &mut treasury, &request);
    assert_eq!(treasury.get(Spoil::Gold), 0, "no repeated charge");

    // Paid travel holds use the production stair handler. Both directions
    // arrive safely with four knights, and neutral controls cannot bounce.
    run.home.countryside = country;
    for id in ["observatory-ropeway", "mine-ropeway"] {
        let e = entrance(id);
        run.arrive(room_of(&run.dungeon, e.room).unwrap(), (12.0, 7.0));
        take(&mut run, e);
        let Dest::Room(target, _) = e.to else { unreachable!() };
        assert_eq!(run.room().kind, target);
        for h in run.players.values() {
            assert!(Grid { room: run.room(), barred:false }.clear(h.x, h.y, HERO_RADIUS, Mover::Hero));
        }
        for _ in 0..DESCEND_HOLD*2 { run.step(&BTreeMap::new()); }
        assert_eq!(run.room().kind, target);
    }
    let friend = run.wire_copy();
    assert_eq!(friend.home.countryside, run.home.countryside);

    // Slalom: do not teleport the squire across gates. Use only Input and step.
    run.arrive(room_of(&run.dungeon, RoomKind::Crag).unwrap(), Game::Sled.start());
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (Game::Sled.start().0 * TILE_UNITS, Game::Sled.start().1 * TILE_UNITS);
    for (&id, h) in &mut run.players { if id != 1 { (h.x, h.y) = (42.0, 18.0); } }
    run.step(&BTreeMap::new());
    assert_eq!(run.pastimes.round.as_ref().unwrap().sled.as_ref().unwrap().driver, 1);
    let records = run.home.countryside.games.clone();
    run.step(&BTreeMap::from([(2, Input { aim_y: -1, fire:true, ..Default::default() })]));
    let sled = run.pastimes.round.as_ref().unwrap().sled.as_ref().unwrap();
    assert_eq!((sled.x, sled.y, sled.dy), (3*256+12, 7*256, 0), "friend cannot steer or brake driver");
    let x = sled.x;
    run.step(&fire(1));
    assert_eq!(run.pastimes.round.as_ref().unwrap().sled.as_ref().unwrap().x, x+6);
    let score = run.score;
    let mut resumed: Option<Run> = None;
    for _ in 0..45*HZ {
        let Some(round) = run.pastimes.round.as_ref() else { break; };
        let sled = round.sled.as_ref().unwrap();
        let target = pastimes::SLED_GATES[usize::from(round.progress)].1 * 256;
        let steer = (target - sled.y - sled.dy*5).signum() as i8;
        run.step(&BTreeMap::from([(1, Input { aim_y:steer, ..Default::default() })]));
        assert!(run.valid_snapshot());
        let friend = run.wire_copy();
        assert_eq!(friend.pastimes, run.pastimes);
        if resumed.is_none() && run.pastimes.round.as_ref().is_some_and(|r| r.progress >= 2) {
            resumed = Some(serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap());
        }
    }
    assert_eq!(run.home.countryside.games.wins_for(Game::Sled), records.wins_for(Game::Sled)+1, "input-only slalom win: {:?}", run.found);
    assert_eq!(run.home.countryside.games.best, records.best, "legacy four-game array untouched");
    assert_eq!(run.home.countryside.games.cheese, records.cheese);
    assert_eq!(run.home.countryside.games.hobbyhorse, records.hobbyhorse);
    assert!(run.score > score);
    let result = run.home.countryside.games.sled.clone();
    let mut resumed = resumed.unwrap();
    for _ in 0..45*HZ {
        let Some(round) = resumed.pastimes.round.as_ref() else { break; };
        let sled = round.sled.as_ref().unwrap();
        let target = pastimes::SLED_GATES[usize::from(round.progress)].1 * 256;
        let steer = (target - sled.y - sled.dy*5).signum() as i8;
        resumed.step(&BTreeMap::from([(1, Input { aim_y:steer, ..Default::default() })]));
    }
    assert_eq!(resumed.home.countryside.games.sled, result, "mid-course checkpoint replays identically");
    let before = run.score;
    for _ in 0..3 { run.step(&BTreeMap::new()); }
    assert!(run.pastimes.round.is_none()); assert_eq!(run.score, before);
    let h = run.players.get_mut(&1).unwrap(); h.x += 5.0;
    run.step(&BTreeMap::new());
    let h = run.players.get_mut(&1).unwrap(); h.x -= 5.0;
    run.step(&BTreeMap::new());
    assert!(run.pastimes.round.is_some());
    for _ in 0..5*HZ { run.step(&BTreeMap::new()); if run.pastimes.round.is_none() { break; } }
    assert!(run.pastimes.round.is_none(), "missing gate ends round, not an unplayable stuck gate");
    assert_eq!(run.home.countryside.games.sled, pastimes::Score { plays:result.plays+1, ..result });
}

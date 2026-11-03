use super::*;
use crate::rules::floor_plates::games_b_bend_cases_tests as cases;
use crate::rules::world::room_of;
use serde_json::{Value, json};

fn rnd(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn axis(r: u64) -> i8 {
    ((r % 3) as i8) - 1
}

fn some_input(state: &mut u64) -> Input {
    let r = rnd(state);
    Input {
        move_x: axis(r),
        move_y: axis(r >> 8),
        aim_x: if r >> 20 & 1 == 0 { 0 } else { axis(r >> 16) },
        aim_y: if r >> 24 & 1 == 0 { 0 } else { axis(r >> 28) },
        fire: r >> 32 & 7 == 0,
        ..Default::default()
    }
}

/// The struct halves of the small games, stepped with random keys.
#[test]
#[ignore]
fn record_pastimes_structs() {
    for seed in 1..=4u64 {
        let mut st = 0x9e37_79b9_7f4a_7c15u64.wrapping_mul(seed);
        // Surf
        let mut s = Surf::new(1);
        let mut wave = 0u8;
        for n in 0..500u32 {
            let input = some_input(&mut st);
            let crest = s.phase >= WAVE_TICKS - CREST_WINDOW;
            let tapped = crest && rnd(&mut st) % 2 == 0;
            let input = if n % 3 == 0 { Input { move_x: if s.lean < WAVE_LEANS[usize::from(wave)] { 1 } else { -1 }, ..input } } else { input };
            let before = serde_json::to_value(&s).unwrap();
            let ret = s.step(input, tapped, wave);
            cases::record(
                "pastimes.surf.Surf.step",
                json!({"s": before, "input": serde_json::to_value(input).unwrap(), "tapped": tapped, "wave": wave}),
                json!({"s": serde_json::to_value(&s).unwrap(), "ret": ret}),
            );
            if ret == Some(true) {
                wave = (wave + 1) % 6;
            }
        }
        // Halfpipe
        let mut h = Halfpipe::new(1);
        let (mut progress, mut mistakes) = (0u8, 0u8);
        for n in 0..700u32 {
            let direction = Halfpipe::direction(progress, mistakes);
            let mut input = some_input(&mut st);
            if n % 2 == 0 {
                input.move_x = direction;
                input.aim_x = 0;
            }
            let tapped = (h.phase >= LIP_OPEN && rnd(&mut st) % 6 == 0) || rnd(&mut st) % 90 == 0;
            let before = serde_json::to_value(&h).unwrap();
            let ret = h.step(input, tapped, direction);
            cases::record(
                "pastimes.halfpipe.Halfpipe.step",
                json!({"s": before, "input": serde_json::to_value(input).unwrap(), "tapped": tapped, "direction": direction}),
                json!({"s": serde_json::to_value(&h).unwrap(), "ret": ret}),
            );
            match ret {
                Some(true) => progress += 1,
                Some(false) => mistakes += 1,
                None => {}
            }
            let before = serde_json::to_value(&h).unwrap();
            h.tick_vfx();
            cases::record("pastimes.halfpipe.Halfpipe.tick_vfx", json!({"s": before}), json!({"s": serde_json::to_value(&h).unwrap()}));
            if progress > 3 || mistakes > 2 {
                (progress, mistakes) = (0, 0);
            }
        }
        // Volley
        let mut v = Volley::new(1);
        for n in 0..900u32 {
            let input = some_input(&mut st);
            let tapped = v.ball.is_none() && rnd(&mut st) % 4 == 0 || rnd(&mut st) % 200 == 0;
            let before = serde_json::to_value(&v).unwrap();
            let ret = v.step(input, tapped);
            cases::record(
                "pastimes.volley.Volley.step",
                json!({"s": before, "input": serde_json::to_value(input).unwrap(), "tapped": tapped}),
                json!({"s": serde_json::to_value(&v).unwrap(), "ret": ret}),
            );
            if v.lives == 0 || n % 400 == 399 {
                v = Volley::new(1);
            }
        }
        // DuckShoot
        let mut d = DuckShoot::new(1);
        for n in 0..500u32 {
            let input = some_input(&mut st);
            let tapped = rnd(&mut st) % 3 == 0;
            let before = serde_json::to_value(&d).unwrap();
            let ret = d.step(input, tapped);
            cases::record(
                "pastimes.duckshoot.DuckShoot.step",
                json!({"s": before, "input": serde_json::to_value(input).unwrap(), "tapped": tapped}),
                json!({"s": serde_json::to_value(&d).unwrap(), "ret": ret}),
            );
            if n % 250 == 249 {
                d = DuckShoot::new(1);
            }
        }
        // Powder: three sappers in one yard.
        let mut y = Powder::new(&[1, 2, 3], (seed % 7) as u8);
        for n in 0..500u32 {
            let mut keys = BTreeMap::new();
            for id in 1..=3u32 {
                keys.insert(id, some_input(&mut st));
            }
            let tapped: Vec<u32> = (1..=3u32).filter(|_| rnd(&mut st) % 9 == 0).collect();
            let before = serde_json::to_value(&y).unwrap();
            let ret = y.step(&keys, &tapped);
            cases::record(
                "pastimes.powder.Powder.step",
                json!({"s": before, "inputs": cases::inputs(&keys), "tapped": tapped}),
                json!({"s": serde_json::to_value(&y).unwrap(), "ret": ret}),
            );
            if y.alive().len() <= 1 || n % 250 == 249 {
                y = Powder::new(&[1, 2, 3], ((seed + u64::from(n)) % 7) as u8);
            }
        }
        // Siege: the march, the walls, the stones.
        let mut z = Siege::new(1);
        for n in 0..2600u32 {
            let mut input = some_input(&mut st);
            if z.fighting && n % 2 == 0 {
                input.aim_y = if z.aim < 4 { -1 } else if z.aim > 7 { 1 } else { 0 };
                input.move_y = 0;
                input.aim_x = if z.power < 4 { 1 } else if z.power > 7 { -1 } else { 0 };
                input.move_x = 0;
            }
            let tapped = rnd(&mut st) % if z.fighting { 12 } else { 40 } == 0;
            let before = serde_json::to_value(&z).unwrap();
            let ret = z.step(input, tapped);
            cases::record(
                "pastimes.siege.Siege.step",
                json!({"s": before, "input": serde_json::to_value(input).unwrap(), "tapped": tapped}),
                json!({"s": serde_json::to_value(&z).unwrap(), "ret": ret}),
            );
            if ret.is_none() || z.leg > 5 || n % 1300 == 1299 {
                z = Siege::new(1);
            }
        }
        // Chariot
        let mut c = Chariot::new(1);
        for n in 0..1800u32 {
            let input = some_input(&mut st);
            let tapped = rnd(&mut st) % 25 == 0;
            let before = serde_json::to_value(&c).unwrap();
            let ret = c.step(input, tapped);
            cases::record(
                "pastimes.chariot.Chariot.step",
                json!({"s": before, "input": serde_json::to_value(input).unwrap(), "tapped": tapped}),
                json!({"s": serde_json::to_value(&c).unwrap(), "ret": ret}),
            );
            if ret != 0 || n % 900 == 899 {
                c = Chariot::new(1);
            }
        }
    }
}

fn place(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn fair(seed: u64, seats: u32, kind: RoomKind) -> Option<Run> {
    let mut run = Run::at_home(seed, 1, None, Default::default(), Default::default());
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let room = room_of(&run.dungeon, kind)?;
    run.arrive(room, (12.0, 11.0));
    Some(run)
}

fn tick(run: &mut Run, keys: &BTreeMap<u32, Input>, name: &str, record: bool) {
    run.tick += 1;
    if record {
        cases::call_inputs(name, run, keys, |r, k| r.tick_pastimes(k));
    } else {
        run.tick_pastimes(keys);
    }
}

/// Each game, started from its pad, played at random, won and timed out.
#[test]
#[ignore]
fn record_pastimes_rounds() {
    for game in Game::ALL {
        let name = if game.index() >= 16 { "pastimes.Run.tick_pastimes.arena" } else { "pastimes.Run.tick_pastimes" };
        for (seed, seats, ending) in [(1u64, 1u32, 0u32), (2, 3, 1), (3, 2, 2)] {
            let Some(mut run) = fair(seed + game.index() as u64 * 7, seats, game.room()) else { continue };
            let mut st = 0xdead_beef_u64.wrapping_mul(seed + game.index() as u64);
            tick(&mut run, &BTreeMap::new(), name, true);
            for id in 1..=seats {
                place(&mut run, id, (game.start().0 + (id - 1) as f32 * 0.4, game.start().1 + (id - 1) as f32 * 0.3));
            }
            tick(&mut run, &BTreeMap::new(), name, true);
            let limit = if game.index() >= 16 { 90 } else { 260 };
            for t in 0..limit {
                let mut keys = BTreeMap::new();
                for id in 1..=seats {
                    keys.insert(id, some_input(&mut st));
                }
                if t % 5 == 0 {
                    // Walk on to a target now and then so the position games score.
                    let r = rnd(&mut st);
                    let target = match game {
                        Game::Barrel => run.pastimes.round.as_ref().map(|r| FLAGS[usize::from(r.progress.min(5))]),
                        _ => None,
                    };
                    if let Some(p) = target {
                        place(&mut run, 1 + (r % u64::from(seats)) as u32, p);
                    }
                }
                let record = t < 10 || t % 40 == 0;
                tick(&mut run, &keys, name, record);
                if run.pastimes.round.is_none() && run.pastimes.heat.is_none() {
                    break;
                }
            }
            for leaving in 1..=seats {
                let mut state = run.pastimes.clone();
                let remaining: Vec<u32> = (1..=seats).filter(|&i| i != leaving).collect();
                let before = serde_json::to_value(&state).unwrap();
                state.depart(leaving, remaining.clone());
                cases::record(
                    "pastimes.State.depart",
                    json!({"s": before, "id": leaving, "remaining": remaining}),
                    json!({"s": serde_json::to_value(&state).unwrap()}),
                );
            }
            let forceable = matches!(game, Game::Broadsheet | Game::Turnip | Game::Cheese | Game::Halfpipe | Game::DuckShoot | Game::Trencher | Game::Powder);
            match if ending == 1 && !forceable { 0 } else { ending } {
                1 => {
                    if let Some(r) = run.pastimes.round.as_mut() {
                        r.progress = game.need();
                    }
                    for r in run.pastimes.heats.values_mut() {
                        r.progress = game.need();
                    }
                }
                2 => {
                    if let Some(r) = run.pastimes.round.as_mut() {
                        r.left = 2;
                    }
                    for r in run.pastimes.heats.values_mut() {
                        r.left = 2;
                    }
                }
                _ => {}
            }
            for _ in 0..6 {
                let mut keys = BTreeMap::new();
                for id in 1..=seats {
                    keys.insert(id, some_input(&mut st));
                }
                tick(&mut run, &keys, name, true);
            }
        }
    }
    // The crossroads plays Powder Keg and the Gander hall for the table.
    for (seed, seats) in [(31u64, 1u32), (32, 2), (33, 3)] {
        for game in [Game::Powder, Game::Gander] {
            let mut run = Run::at_home(seed, 1, None, Default::default(), Default::default());
            for id in 2..=seats {
                run.join_seat(id, &format!("Seat {id}"));
            }
            run.begin_crossroads(1, Pack::Crypt);
            if run.crossroads.is_none() {
                continue;
            }
            cases::call("pastimes.Run.start_crossroads_pastime", &mut run, json!({"game": game}), |r| r.start_crossroads_pastime(game));
            let mut st = 0xabcd_ef01_u64.wrapping_mul(seed);
            for t in 0..80 {
                let mut keys = BTreeMap::new();
                for id in 1..=seats {
                    keys.insert(id, some_input(&mut st));
                }
                tick(&mut run, &keys, "pastimes.Run.tick_pastimes.arena", t < 24 || t % 8 == 0);
            }
        }
    }
    // Rooms that hold no game: the state leaves.
    let mut run = Run::at_home(5, 1, None, Default::default(), Default::default());
    for _ in 0..3 {
        tick(&mut run, &BTreeMap::new(), "pastimes.Run.tick_pastimes", true);
    }
}


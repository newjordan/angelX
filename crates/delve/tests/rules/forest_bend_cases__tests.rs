use super::*;
use crate::rules::floor_plates::games_b_bend_cases_tests as cases;
use crate::rules::home::{DESCEND_HOLD, Home};
use crate::rules::world::{ENTRANCES, entrance_centre};
use crate::rules::{RoomKind, world::room_of};

/// A run that has taken the gate into the world, where the clearing is.
fn at_the_clearing(seed: u64) -> Run {
    let mut run = Run::at_home(seed, 1, Some("Friend"), Home::default(), Default::default());
    run.come_to_the_gate();
    run.step(&BTreeMap::new());
    let gate = ENTRANCES.iter().find(|e| e.id == "gate").unwrap();
    let (x, y) = entrance_centre(gate);
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (x, y);
    }
    for _ in 0..DESCEND_HOLD + 2 {
        run.step(&BTreeMap::new());
    }
    run
}

fn place(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x, y);
}

fn at_tile(p: (f32, f32)) -> (f32, f32) {
    (p.0 * TILE_UNITS, p.1 * TILE_UNITS)
}

fn mix(a: u64, b: u64) -> u64 {
    (a ^ b.wrapping_mul(0x9e37_79b9_7f4a_7c15)).wrapping_mul(0xbf58_476d_1ce4_e5b9) >> 17
}

fn step(run: &mut Run, record: bool) {
    run.tick += 1;
    if record {
        cases::call("forest.Run.tick_forest", run, serde_json::json!({}), |r| r.tick_forest());
    } else {
        run.tick_forest();
    }
}

/// The hunt and the song, with knights drifting over the pads and fireflies.
#[test]
#[ignore]
fn record_forest() {
    for seed in 1..=6u64 {
        let seats = 1 + (seed % 3) as u32;
        let mut run = at_the_clearing(seed);
        for id in 3..=seats {
            run.join_seat(id, &format!("Seat {id}"));
        }
        let seats = seats.max(2);
        let clearing = room_of(&run.dungeon, RoomKind::Forest).unwrap();
        run.arrive(clearing, LANTERN);
        if seed == 3 {
            run.players.get_mut(&2).unwrap().stone = true;
        }
        // A hunt: the lantern, then knights roam; odd seeds hunt together.
        for t in 0..90u64 {
            for id in 1..=seats {
                let pick = if t < 2 { 3 } else if seed % 2 == 1 { 1 } else { mix(seed, t * 7 + u64::from(id)) % 8 };
                let fly = firefly(((t + u64::from(id)) % 8) as usize, run.tick + 1);
                let pad = [at_tile(LANTERN), at_tile(MEMORY), at_tile(RUNES[(mix(seed, t) % 3) as usize])];
                let to = match pick {
                    1 | 2 => fly,
                    3 => pad[0],
                    4 => pad[1],
                    5 | 6 => pad[2],
                    7 => (4.0, 4.0),
                    _ => (run.players[&id].x, run.players[&id].y),
                };
                place(&mut run, id, to);
            }
            step(&mut run, true);
        }
        // The song: the memory stone, the show, then notes right and wrong.
        for id in 1..=seats {
            place(&mut run, id, (4.0, 4.0));
        }
        step(&mut run, true);
        place(&mut run, 1, at_tile(MEMORY));
        step(&mut run, true);
        for t in 0..160u64 {
            if t % 12 == 0 {
                step(&mut run, true);
            } else {
                step(&mut run, false);
            }
            if !run.forest.singing || run.forest.show_left == 0 {
                break;
            }
        }
        for round in 0..12u64 {
            for id in 1..=seats {
                place(&mut run, id, (4.0, 4.0));
            }
            step(&mut run, true);
            let right = run.forest.melody(run.forest.note);
            let wrong = (right + 1 + (round % 2) as usize) % 3;
            let pick = if seed % 3 == 0 && round % 4 == 3 { wrong } else { right };
            place(&mut run, 1, at_tile(RUNES[pick]));
            if seats > 1 && round == 5 {
                place(&mut run, 2, at_tile(RUNES[(pick + 1) % 3]));
            }
            step(&mut run, true);
            if !run.forest.singing {
                run.forest.songs += 1;
                place(&mut run, 1, at_tile(MEMORY));
                step(&mut run, true);
                for _ in 0..155 {
                    step(&mut run, false);
                }
            }
        }
    }
}

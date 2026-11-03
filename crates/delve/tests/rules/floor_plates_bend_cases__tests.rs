use super::*;
use crate::rules::{Phase, floor_plates::games_b_bend_cases_tests as cases};
use serde_json::json;

fn skip(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { skip_plate: true, ..Default::default() })])
}

/// begin / tick / skip over varied party sizes, kinds and ages.
#[test]
#[ignore]
fn record_floor_plates() {
    let kinds = [Kind::Gate, Kind::Crypt, Kind::Mines, Kind::Keep, Kind::Archive, Kind::Fungal, Kind::Unknown, Kind::Swamp];
    for seed in 1..=8u64 {
        let seats = 1 + (seed % 4) as u32;
        let mut run = Run::at_home(seed, 1, None, Default::default(), Default::default());
        for id in 2..=seats {
            run.join_seat(id, &format!("Seat {id}"));
        }
        let kind = kinds[seed as usize % kinds.len()];
        cases::call("floor_plates.Run.begin_floor_plate", &mut run, json!({"kind": kind}), |r| r.begin_floor_plate(kind));
        if seed % 2 == 0 {
            for p in run.floor_plates.values_mut() {
                p.results = [0, 7, 40, 250][seed as usize % 4];
            }
        }
        for step in 0..70u32 {
            let keys = if step % 23 == 22 { skip(1 + step % seats) } else { BTreeMap::new() };
            cases::call_inputs("floor_plates.Run.tick_floor_plates", &mut run, &keys, |r, k| r.tick_floor_plates(k));
            if step % 17 == 5 {
                run.phase = if run.phase == Phase::Fighting { Phase::Exploring } else { run.phase };
            }
        }
        let mut run2 = Run::at_home(seed, 1, None, Default::default(), Default::default());
        cases::call("floor_plates.Run.begin_floor_plate", &mut run2, json!({"kind": kind}), |r| r.begin_floor_plate(kind));
        for who in [1, 2] {
            cases::call("floor_plates.Run.skip_floor_plate", &mut run2, json!({"seat": who}), |r| r.skip_floor_plate(who));
        }
    }
}

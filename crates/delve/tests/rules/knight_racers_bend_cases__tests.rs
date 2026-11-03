//! Knight Racer cases for the Bend port (games_c). Run with `--features bend-cases-games-c --ignored`.
use crate::rules::hippodrome::games_c_bend_cases_tests as kit;
use super::*;
use serde_json::json;

#[test]
#[ignore]
fn record_knight_racer_cases() {
    let mut d = kit::Dice(0x4b52);
    for n in 0..40u64 {
        let ids: Vec<u32> = (1..=6u32).filter(|id| d.chance(70) || *id == 1 + (n % 6) as u32).collect();
        let mut s = Session { cars: BTreeMap::new(), ticks: 0, finished: false };
        let before = kit::js(&s);
        s.join_ids(ids.clone());
        kit::rec("knight_racers.Session.join_ids", json!({"s": before, "ids": ids}), json!({"s": kit::js(&s)}));
        // Drive the field: a few hundred ticks of mixed keys, a longer burst for some.
        let ticks = if n % 4 == 0 { 1500 } else { 260 };
        let mut every = [0i8; 6];
        for t in 0..ticks {
            let mut inputs = BTreeMap::new();
            for &id in &ids {
                if t % 15 == 0 {
                    every[(id - 1) as usize] = d.axis();
                }
                let go = n % 4 == 0 || d.chance(80);
                inputs.insert(id, Input {
                    move_y: if go { -1 } else { d.axis() },
                    move_x: if d.chance(30) { d.axis() } else { every[(id - 1) as usize] },
                    dash: d.chance(35),
                    interact: s.finished && d.chance(5),
                    ..Default::default()
                });
            }
            let before = kit::js(&s);
            s.step(&inputs);
            if kit::count("knight_racers.Session.step") < 4000 {
                kit::rec("knight_racers.Session.step", json!({"s": before, "inputs": kit::js(&inputs)}), json!({"s": kit::js(&s)}));
            }
            if s.finished && t % 40 == 39 && n % 4 != 0 {
                break;
            }
        }
    }
    for k in 0..300u32 {
        let s = (k as f32 - 40.0) * 3.3;
        kit::rec("knight_racers.pose", json!({"s": s}), json!({"ret": [pose(s).0, pose(s).1, pose(s).2]}));
        kit::rec("knight_racers.curve", json!({"s": s}), json!({"ret": curve(s)}));
    }
}

//! Case recorder for lanternward.rs `impl Run` hooks and the ADVENTURE /
//! ACTIVITY closures (Bend port, games_a). The Run is its rules checkpoint.
use super::*;
use crate::rules::runtime_state::capture_for_rules;
use serde_json::{Value, json};
#[path = "games_a_bend_cases__tests.rs"]
mod cases;

const CAP: usize = 14;

fn rj(run: &Run) -> Value {
    capture_for_rules(run).unwrap().run
}
fn rec(name: &str, input: Value, out: Value) {
    cases::rec_cap(name, &input, &out, CAP);
}

#[test]
#[ignore]
fn record_lanternward_run() {
    for seats in [1u32, 2, 3, 5] {
        let mut run = Run::lanternward_for_test();
        for id in 2..=seats {
            run.join_seat(id, &format!("Knight {id}"));
        }
        run.step(&BTreeMap::new());
        let mut closed = run.clone();
        closed.lanternward = None;
        rec("lanternward.Run.lanternward_showing", json!({"run": rj(&run)}), json!(run.lanternward_showing()));
        rec("lanternward.Run.lanternward_showing", json!({"run": rj(&closed)}), json!(closed.lanternward_showing()));
        // Open the table on a closed room.
        let mut c = closed.clone();
        c.start_lanternward();
        rec("lanternward.Run.start_lanternward", json!({"run": rj(&closed)}), json!({"run": rj(&c)}));
        // A table that was played before (a record to count on).
        let mut again = closed.clone();
        again.home.countryside.games.roundtable[SLOT].plays = 4;
        again.home.countryside.games.roundtable[SLOT].wins = 2;
        again.home.countryside.games.roundtable[SLOT].best = 600;
        let mut c = again.clone();
        c.start_lanternward();
        rec("lanternward.Run.start_lanternward", json!({"run": rj(&again)}), json!({"run": rj(&c)}));
        for tick in 0..3000u32 {
            let inputs = run.roundtable_inputs();
            if tick % 151 == 3 {
                let mut c = run.clone();
                c.tick_lanternward(&inputs);
                rec("lanternward.Run.tick_lanternward", json!({"run": rj(&run), "inputs": serde_json::to_value(&inputs).unwrap()}), json!({"run": rj(&c)}));
                rec("lanternward.ADVENTURE.valid", json!({"run": rj(&run)}), json!((ADVENTURE.valid)(&run)));
                rec("lanternward.ADVENTURE.seated", json!({"run": rj(&run)}), json!((ADVENTURE.seated)(&run)));
                if let Some(&id) = run.players.keys().next() {
                    rec(
                        "lanternward.ADVENTURE.bot",
                        json!({"run": rj(&run), "id": id}),
                        serde_json::to_value((ADVENTURE.bot)(&run, id)).unwrap(),
                    );
                }
                let mut c = run.clone();
                (ADVENTURE.clear)(&mut c);
                rec("lanternward.ADVENTURE.clear", json!({"run": rj(&run)}), json!({"run": rj(&c)}));
                rec("lanternward.ACTIVITY.on", json!({"run": rj(&run)}), json!((ACTIVITY.on)(&run)));
                let mut c = run.clone();
                if let Some(ride) = ACTIVITY.ride {
                    ride(&mut c, &inputs);
                }
                rec("lanternward.ACTIVITY.ride", json!({"run": rj(&run), "inputs": serde_json::to_value(&inputs).unwrap()}), json!({"run": rj(&c)}));
                let mut c = run.clone();
                (ACTIVITY.end)(&mut c);
                rec("lanternward.ACTIVITY.end", json!({"run": rj(&run)}), json!({"run": rj(&c)}));
            }
            run.step(&inputs);
        }
        // The Beacon burns: first time, and again.
        for (first, plays) in [(true, 1u32), (false, 3)] {
            let mut r2 = run.clone();
            r2.home.countryside.games.roundtable[SLOT].plays = plays;
            if !first {
                r2.marks.insert(CAMPAIGN_MARK.into(), 1);
                r2.home.roundtable[SLOT] |= BEGUN;
            }
            if let Some(mut s) = r2.lanternward.clone() {
                s.elapsed = 5400;
                for k in s.seats.values_mut() {
                    k.gems = 13;
                }
                if !first {
                    s.paid = 2;
                }
                let mut c = r2.clone();
                let mut s2 = s.clone();
                c.finish_lanternward(&mut s2);
                rec(
                    "lanternward.Run.finish_lanternward",
                    json!({"run": rj(&r2), "s": serde_json::to_value(&s).unwrap()}),
                    json!({"run": rj(&c), "s": serde_json::to_value(&s2).unwrap()}),
                );
            }
        }
    }
}

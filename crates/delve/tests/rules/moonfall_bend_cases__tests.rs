use super::*;
use crate::rules::floor_plates::games_b_bend_cases_tests as cases;
use serde_json::json;

fn rec(name: &str, input: serde_json::Value, ret: impl serde::Serialize) {
    cases::record(name, input, json!({"ret": ret}));
}

/// The town's clock, over the day lengths the knob allows.
#[test]
#[ignore]
fn record_moonfall_clock() {
    for day in [1800u32, 2400, 3000, 3601, 4500, 5999, 6000] {
        for h2 in (0..=170).step_by(5) {
            rec("moonfall.clock.ticks", json!({"day": day, "h2": h2}), clock::ticks(day, h2));
        }
        let end = clock::end(day);
        let mut c = 0u32;
        while c <= end + 10 {
            let input = json!({"clock": c, "day": day});
            rec("moonfall.clock.half", input.clone(), clock::half(c, day));
            let s = clock::stamp(c, day);
            rec("moonfall.clock.stamp", input.clone(), json!({"day": s.day, "hour": s.hour, "minute": s.minute}));
            rec("moonfall.clock.night", input.clone(), clock::night(c, day));
            rec("moonfall.clock.bridge_up", input.clone(), clock::bridge_up(c, day));
            rec("moonfall.clock.remaining_minutes", input.clone(), clock::remaining_minutes(c, day));
            rec("moonfall.clock.next_dusk", input.clone(), clock::next_dusk(c, day));
            rec("moonfall.clock.next_dawn", input.clone(), clock::next_dawn(c, day));
            rec("moonfall.clock.light", input, clock::light(c, day));
            c += day / 13 + 7;
        }
    }
}

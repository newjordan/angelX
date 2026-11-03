//! Latch Duel cases for the Bend port (games_c). Run with `--features bend-cases-games-c --ignored`.
use crate::rules::tune::Tune;
use crate::rules::hippodrome::games_c_bend_cases_tests as kit;
use super::*;
use serde_json::json;

fn input(d: &mut kit::Dice) -> Input {
    Input {
        fire: d.chance(40),
        move_y: d.axis(),
        latch_tick: if d.chance(25) { Some(d.below(40)) } else { None },
        ..Default::default()
    }
}

#[test]
#[ignore]
fn record_latch_duel_cases() {
    let tune = Tune::default();
    let mut d = kit::Dice(0x1a7c);
    for seed in 0..40u64 {
        let mut duel = Duel::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut now: u64 = 1 + d.below(10);
        for _ in 0..140 {
            let inp = input(&mut d);
            let allow = d.below(5);
            let stamped = d.chance(50);
            let at = if stamped { Some(now) } else { None };
            let before = duel.clone();
            let sound = duel.step_at(inp, &tune, at, allow);
            kit::rec(
                "latch.Duel.step_at",
                json!({"d": kit::js(&before), "input": kit::js(&inp), "tune": kit::js(&tune), "now": at, "allowance": allow}),
                json!({"d": kit::js(&duel), "ret": sound}),
            );
            // The next-height rule is read off the duel's seed and attack count.
            now += 1;
            if duel.beat == Beat::Defeat {
                break;
            }
        }
    }
    for n in 0..60u64 {
        let mut s = Session::new((1..=1 + (n % 5) as u32).into_iter(), n * 7919);
        for (i, duel) in s.duels.values_mut().enumerate() {
            duel.counters = ((n + i as u64) % 4) as u8;
            duel.misses = ((n * 3 + i as u64) % 5) as u32;
            duel.error = ((n * 5 + i as u64) % 6) as u32;
        }
        s.order = s.duels.keys().copied().collect();
        s.order.sort_by_key(|id| {
            let d = &s.duels[id];
            (std::cmp::Reverse(d.counters), d.misses, d.error, *id)
        });
        s.left = if n % 2 == 0 { 0 } else { 5 };
        for id in s.duels.keys().copied().collect::<Vec<_>>() {
            kit::rec("latch.Session.place", json!({"s": kit::js(&s), "id": id}), json!({"ret": s.place(id)}));
            kit::rec("latch.Session.payout", json!({"s": kit::js(&s), "id": id}), json!({"ret": s.payout(id)}));
        }
    }
    for t in 0..40u32 {
        kit::rec("latch.timing", json!({"ticks": t * 3}), json!({"ret": timing(t * 3)}));
    }
}

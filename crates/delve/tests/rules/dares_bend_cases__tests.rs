use super::*;
use crate::rules::floor_plates::games_b_bend_cases_tests as cases;
use serde_json::json;

fn dared(seed: u64, kind: DareKind) -> Run {
    let mut run = Run::new(seed, 1, None);
    if seed % 2 == 0 {
        run.join_seat(2, "Two");
    }
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.phase = Phase::Exploring;
    run.dare = Some(Dare { kind, since: run.tick, count: 0, kept: false, broken: false, rose: None });
    run
}

/// Every floor's call, the guardian question, the cues that keep or break a
/// dare, and the stairs.
#[test]
#[ignore]
fn record_dares() {
    for seed in 0..40u64 {
        let mut run = Run::new(seed, 1, None);
        for _ in 1..=DEEPEST {
            cases::call("dares.Run.floor_has_guardian", &mut run, json!({}), |r| r.floor_has_guardian());
            cases::call("dares.Run.call_dare", &mut run, json!({}), |r| r.call_dare());
            run.descend_for_test();
        }
    }
    let swamp = {
        let mut run = Run::new(5, 1, None);
        run.dungeon.pack = Pack::Swamp;
        run
    };
    let mut run = swamp;
    cases::call("dares.Run.floor_has_guardian", &mut run, json!({}), |r| r.floor_has_guardian());
    let kinds = [DareKind::Untouched, DareKind::StandFirm, DareKind::AgainstTheClock, DareKind::Pathfinder, DareKind::MakeItQuick, DareKind::Unstoppable];
    let cue_sets: [&[&str]; 9] = [
        &["flawless"],
        &["flawless", "flawless:2"],
        &["slay4", "slay4"],
        &["room_clear", "room_clear", "flawless"],
        &["boss_rise:3"],
        &["lair"],
        &["boss_fall:3"],
        &["victory"],
        &["nothing", "other:1"],
    ];
    for seed in 0..6u64 {
        for kind in kinds {
            let mut run = dared(seed, kind);
            for (n, cues) in cue_sets.iter().enumerate() {
                run.tick += 37 + (seed as u64 * 500) * (n as u64 % 3) + if kind == DareKind::AgainstTheClock && n == 7 { 7300 } else { 0 };
                let (heard, sounded) = (run.cues.len(), run.sounds.len());
                for c in *cues {
                    run.cues.push((*c).into());
                }
                if kind == DareKind::StandFirm && n % 3 == 1 {
                    run.sounds.push("roll");
                }
                if kind == DareKind::MakeItQuick && n == 6 {
                    run.tick += 1400 * (seed as u64 % 2);
                }
                cases::call("dares.Run.watch_dare", &mut run, json!({"heard": heard, "sounded": sounded}), |r| r.watch_dare(heard, sounded));
                if run.dare.as_ref().is_some_and(|d| d.kept || d.broken) {
                    cases::call("dares.Run.settle_dare", &mut run, json!({}), |r| r.settle_dare());
                    run.dare = Some(Dare { kind, since: run.tick, count: 0, kept: false, broken: false, rose: None });
                }
            }
            let mut stairs = dared(seed, kind);
            cases::call("dares.Run.settle_dare", &mut stairs, json!({}), |r| r.settle_dare());
            let mut broken = dared(seed, kind);
            broken.dare.as_mut().unwrap().broken = true;
            cases::call("dares.Run.settle_dare", &mut broken, json!({}), |r| r.settle_dare());
        }
    }
}

//! King Cinder's tick, held to the code it replaced.
//!
//! The tick lives in Bend now (`cockpit/live/bannerpeak-rules`, `boss.bend`) and
//! `crates/delve/src/rules/bannerpeak/boss.rs` keeps only his types and the
//! adapter that asks the leg. What is left of the native tick is
//! `crates/delve/src/rules/bannerpeak/oracle.rs`, included here verbatim, and
//! these tests run the shipped leg and that oracle side by side: every beat of
//! every crown, the warnings, the blows, the plates, and the state after every
//! tick.
//!
//! The old `boss.rs` tests moved here with the rules they check.

#[cfg(test)]
pub use angel_delve::rules::bannerpeak::{body, boss, layout, world};

#[cfg(test)]
#[path = "../../../crates/delve/src/rules/bannerpeak/oracle.rs"]
mod oracle;

#[cfg(test)]
mod tests {
    use super::boss::{Attack, Beat, Boss, Hazard, HEAD_DOWN, HEAD_UP, PHASES, floor, home};
    use super::oracle;
    use serde_json::{Value, json};

    fn ask(boss: &Boss, feet: &[(i32, i32)], seats: u8, elapsed: u32) -> Value {
        json!({"op": "boss", "elapsed": elapsed, "seats": seats, "feet": feet, "boss": boss})
    }

    /// The leg's answer for one tick of King Cinder.
    fn bent(boss: &Boss, feet: &[(i32, i32)], seats: u8, elapsed: u32) -> Boss {
        let answer: Value = angel_delve::scripted::step("bannerpeak-rules", &ask(boss, feet, seats, elapsed))
            .expect("the leg answers a boss tick");
        serde_json::from_value(answer["boss"].clone()).expect("the leg's boss is a boss")
    }

    /// The oracle's answer for the same tick.
    fn native(boss: &Boss, feet: &[(i32, i32)], seats: u8, elapsed: u32) -> Boss {
        let mut out = boss.clone();
        oracle::step(&mut out, feet, seats, elapsed);
        out
    }

    fn bent_slam(boss: &Boss, x: i32, y: i32) -> (bool, Boss) {
        let input = json!({"op": "slam", "x": x, "y": y, "boss": boss});
        let answer: Value = angel_delve::scripted::step("bannerpeak-rules", &input).expect("the leg answers a slam");
        (
            answer["hit"].as_bool().expect("a slam is a hit or it is not"),
            serde_json::from_value(answer["boss"].clone()).expect("a boss"),
        )
    }

    /// Feet walking about the summit, so a fight is driven by movement and not
    /// only by the clock: three knights and a leader circling the arena.
    fn feet(seats: u8, tick: u32) -> Vec<(i32, i32)> {
        let left = (392 + 6) * 16;
        let right = (392 + 34) * 16;
        (0..seats)
            .map(|seat| {
                let span = right - left;
                let phase = (tick as i32 * 7 + seat as i32 * 61) % (2 * span);
                let x = left + if phase < span { phase } else { 2 * span - phase };
                (x, floor() - if seat % 2 == 0 { 0 } else { 32 })
            })
            .collect()
    }

    #[test]
    fn every_tick_of_a_whole_fight_is_the_tick_the_oracle_makes() {
        crate::agent::harness::live_policy::install_scripted_rules();
        for seats in 1..=4u8 {
            let mut boss = Boss::new(seats);
            for tick in 0..2400u32 {
                let at = feet(seats, tick);
                let was = boss.beat;
                let shown = bent(&boss, &at, seats, tick);
                assert_eq!(shown, native(&boss, &at, seats, tick), "seats {seats} tick {tick} {:?}", was);
                boss = shown;
                // Every tick, a slam at the crown, at either edge of its box and
                // well off it: the leg's plate and the oracle's plate agree.
                for (x, y) in [
                    (boss.x, floor() - boss.head),
                    (boss.x + 23, floor() - boss.head + 3),
                    (boss.x - 24, floor() - boss.head - 3),
                    (boss.x + 40, floor() - boss.head),
                    (boss.x, floor() - HEAD_UP),
                ] {
                    let (hit, after) = bent_slam(&boss, x, y);
                    let mut want = boss.clone();
                    let native_hit = oracle::slam(&mut want, x, y);
                    assert_eq!((hit, &after), (native_hit, &want), "seats {seats} tick {tick} slam {x},{y}");
                    boss = after;
                }
            }
            assert_eq!(boss.phase, PHASES, "seats {seats}: three crowns fall");
            assert_eq!(boss.beat, Beat::Down);
        }
    }

    #[test]
    fn a_crown_needs_a_plate_for_every_seat_and_only_a_kneeling_head_takes_a_blow() {
        crate::agent::harness::live_policy::install_scripted_rules();
        let mut b = Boss::new(3);
        assert_eq!(b.plates, 3);
        let head_y = floor() - HEAD_UP;
        assert!(!b.slam(b.x, head_y), "not while he stands");
        b.beat = Beat::Wince;
        b.head = HEAD_DOWN;
        let y = floor() - HEAD_DOWN;
        assert!(!b.slam(b.x + 40, y), "off to the side");
        assert!(!b.slam(b.x, y - 30), "not high above");
        assert!(b.slam(b.x, y) && b.slam(b.x + 10, y));
        assert_eq!(b.plates, 1);
        assert!(b.slam(b.x, y));
        assert_eq!((b.phase, b.beat), (1, Beat::Rage), "the second crown");
        assert!(b.valid(6));
    }

    #[test]
    fn three_crowns_fall_in_turn_and_a_late_seat_brings_a_plate() {
        crate::agent::harness::live_policy::install_scripted_rules();
        let mut b = Boss::new(1);
        for crown in 0..3u8 {
            assert_eq!(b.phase, crown);
            b.beat = Beat::Wince;
            b.head = HEAD_DOWN;
            if crown == 1 {
                // Rage mended the crown for two seats; a third sits down.
                assert_eq!(b.plates, 2);
                b.seated(2, 3);
                assert_eq!(b.plates, 3, "a seat that sits down adds a plate");
            }
            let mut blows = 0;
            while b.phase == crown {
                assert!(b.slam(b.x, floor() - HEAD_DOWN));
                blows += 1;
            }
            assert_eq!(blows, [1, 3, 2][usize::from(crown)]);
            if b.beat == Beat::Rage {
                b.t = 1;
                let seats = if crown == 0 { 2 } else { 2 };
                b.step(&[], seats, 0);
            }
        }
        assert!(b.defeated() && b.phase == PHASES);
        assert!(b.valid(6));
    }

    #[test]
    fn the_pattern_tells_before_it_strikes_and_ends_kneeling() {
        crate::agent::harness::live_policy::install_scripted_rules();
        let mut b = Boss::new(1);
        let at = [(home() - 60, floor())];
        let mut told = Vec::new();
        let mut struck = false;
        let mut knelt = false;
        for t in 0..900 {
            let before = b.hazards.iter().filter(|h| h.hurts()).count();
            b.step(&at, 1, t);
            if b.beat == Beat::Tell && b.t > 8 {
                told.push(b.kind);
            }
            struck |= b.hazards.iter().filter(|h| h.hurts()).count() > before;
            knelt |= b.beat == Beat::Wince;
            if knelt {
                break;
            }
        }
        assert!(struck && knelt);
        assert!(told.contains(&Attack::Stomp) && told.contains(&Attack::Embers));
        assert!(b.valid(6));
    }

    #[test]
    fn more_seats_mean_more_embers_and_less_time_never_less_boss() {
        crate::agent::harness::live_policy::install_scripted_rules();
        let embers = |seats: u8| {
            let mut b = Boss::new(seats);
            b.beat = Beat::Rest;
            b.t = 0;
            b.count = 1;
            b.step(&[(home(), floor()), (home() + 50, floor())], seats, 3);
            b.hazards.len()
        };
        assert!(embers(6) > embers(1), "{} against {}", embers(6), embers(1));
        let wince = |seats: u8| {
            let mut b = Boss::new(seats);
            b.beat = Beat::Rest;
            b.t = 0;
            b.count = 9;
            b.step(&[(home(), floor())], seats, 1);
            b.t
        };
        assert!(wince(6) < wince(1));
        assert_eq!(Boss::new(6).plates, 6);
    }

    /// The hazards a tick raises are the hazards the oracle raises, field for
    /// field: a wave's speed, an ember column's box, a warning's count.
    #[test]
    fn every_raised_hazard_matches_the_oracle() {
        crate::agent::harness::live_policy::install_scripted_rules();
        for seats in 1..=4u8 {
            for kind in 0..3u32 {
                for phase in 0..PHASES {
                    let mut b = Boss::new(seats);
                    b.beat = Beat::Rest;
                    b.phase = phase;
                    b.t = 0;
                    b.count = kind as u8;
                    b.head = HEAD_UP;
                    for elapsed in 0..40u32 {
                        let at = feet(seats, elapsed);
                        let leg = bent(&b, &at, seats, elapsed);
                        let want = native(&b, &at, seats, elapsed);
                        assert_eq!(leg.hazards, want.hazards, "seats {seats} phase {phase} kind {kind} t {elapsed}");
                        b = leg;
                    }
                }
            }
        }
    }

    /// The adapter in `boss.rs` sends what it was given: the same feet, the same
    /// seats, the same tick number as the test's own wire, and applies what comes
    /// back. This is the seam the tick-level test cannot see.
    #[test]
    fn the_adapter_sends_the_feet_and_the_seats_it_was_given() {
        crate::agent::harness::live_policy::install_scripted_rules();
        for seats in 1..=4u8 {
            let mut through_boss = Boss::new(seats);
            let mut through_wire = Boss::new(seats);
            for tick in 0..200u32 {
                let at = feet(seats, tick);
                through_boss.step(&at, seats, tick);
                through_wire = bent(&through_wire, &at, seats, tick);
                assert_eq!(through_boss, through_wire, "seats {seats} tick {tick}");
                // And a slam through the adapter lands where the wire's does.
                let (x, y) = (through_wire.x, floor() - through_wire.head);
                let hit = through_boss.slam(x, y);
                let (want, after) = bent_slam(&through_wire, x, y);
                assert_eq!((hit, &through_boss), (want, &after), "seats {seats} tick {tick} slam");
            }
        }
    }

    /// What one tick costs at the whole table: the leg's call, its marshalling
    /// and its answer, timed the way the frame times everything (HZ = 30 is a
    /// 33.3 ms frame).
    #[test]
    fn a_tick_at_four_seats_costs_what_the_frame_allows() {
        crate::agent::harness::live_policy::install_scripted_rules();
        let seats = 4u8;
        let mut boss = Boss::new(seats);
        for tick in 0..60u32 {
            boss = bent(&boss, &feet(seats, tick), seats, tick);
        }
        let mut times = Vec::new();
        let (mut asked, mut answered) = (0usize, 0usize);
        for tick in 0..1200u32 {
            let at = feet(seats, tick);
            let input = ask(&boss, &at, seats, tick);
            asked = serde_json::to_vec(&input).expect("a tick is JSON").len();
            let start = std::time::Instant::now();
            let answer: Value = angel_delve::scripted::step("bannerpeak-rules", &input)
                .expect("the leg answers a boss tick");
            times.push(start.elapsed().as_secs_f64() * 1e6);
            answered = serde_json::to_vec(&answer).expect("a tick is JSON").len();
            boss = serde_json::from_value(answer["boss"].clone()).expect("a boss");
        }
        times.sort_by(|a, b| a.partial_cmp(b).expect("timed"));
        let median = times[times.len() / 2];
        let p95 = times[times.len() * 95 / 100];
        let frame = 1_000_000.0 / 30.0;
        eprintln!(
            "bannerpeak boss tick at {seats} seats: median {median:.1} us, p95 {p95:.1} us, \
             {:.4}% of the 30 Hz frame; json in {asked} B, out {answered} B",
            100.0 * median / frame
        );
        assert!(median < frame / 10.0, "a tick may not eat a tenth of the frame");
    }

    /// A hazard list is a list: the leg never loses one it kept, and the cap on
    /// what a fight carries is the oracle's own shape.
    #[test]
    fn a_long_fight_keeps_only_hazards_that_are_still_doing_something() {
        crate::agent::harness::live_policy::install_scripted_rules();
        let mut b = Boss::new(2);
        for tick in 0..1200u32 {
            b.step(&feet(2, tick), 2, tick);
            assert!(b.hazards.len() <= 64);
            assert!(
                b.hazards.iter().all(|h: &Hazard| h.live > 0 || h.tell > 0),
                "tick {tick}: a spent hazard was kept"
            );
        }
    }
}

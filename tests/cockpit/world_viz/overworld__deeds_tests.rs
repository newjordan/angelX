use super::*;

fn id(n: &str) -> ToolEventId {
    ToolEventId(n.to_string())
}

fn run(deeds: &mut Deeds, from: u64, ticks: u64) -> u64 {
    for t in from..from + ticks {
        deeds.step(t);
    }
    from + ticks
}

fn one(deeds: &Deeds, errand: Errand) -> Option<Wayfarer> {
    deeds.wayfarers().into_iter().find(|w| w.errand == errand)
}

#[test]
fn a_trial_takes_the_wagon_to_the_lists_and_hangs_its_verdict() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("t"), "shell", "cd /repo && cargo test", 0);
    let start = one(&deeds, Errand::Wagon).expect("the wagon leaves the Smithy");
    assert_eq!(
        (start.x, start.y),
        feet(Place::Smithy.stand_world()),
        "it sets out from the Smithy door"
    );
    assert!(deeds.trial_underway());

    let t = run(&mut deeds, 1, 4_000);
    let waiting = one(&deeds, Errand::Wagon).unwrap();
    assert_eq!(
        (waiting.x, waiting.y),
        feet(wagon_bay()),
        "it waits at the Lists gate for as long as the trial runs"
    );
    assert!(!waiting.moving && waiting.verdict.is_none());
    assert_ne!(
        wagon_bay(),
        Place::Lists.stand_world(),
        "the knight keeps his own stand"
    );
    assert!(deeds.record().pennants.is_empty());

    deeds.settle(&id("t"), true, 0);
    let t = run(&mut deeds, t, 1);
    assert_eq!(deeds.record().pennants, vec![true]);
    assert!(!deeds.trial_underway());
    assert_eq!(one(&deeds, Errand::Wagon).unwrap().verdict, Some(true));

    run(&mut deeds, t, 4_000);
    assert!(
        one(&deeds, Errand::Wagon).is_none(),
        "home again, the wagon is gone"
    );
    assert_eq!(deeds.record().pennants, vec![true], "the pennant stays");
}

#[test]
fn a_failed_trial_stops_the_wagon_where_it_stands() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("t"), "cargo", "check", 0);
    let t = run(&mut deeds, 1, 20);
    let rolling = one(&deeds, Errand::Wagon).unwrap();
    deeds.settle(&id("t"), false, 0);
    run(&mut deeds, t, 10);
    let broken = one(&deeds, Errand::Wagon).unwrap();
    assert_eq!(broken.verdict, Some(false));
    assert!(!broken.moving, "a verdict halts the wagon mid-road");
    assert!((broken.x - rolling.x).abs() < 1.5);
    assert_eq!(deeds.record().pennants, vec![false]);
}

#[test]
fn the_fence_keeps_only_the_latest_trials() {
    let mut deeds = Deeds::default();
    for n in 0..PENNANTS + 3 {
        let call = id(&format!("t{n}"));
        deeds.begin(&call, "run_tests", "", 0);
        deeds.settle(&call, n != PENNANTS + 2, 0);
    }
    let pennants = &deeds.record().pennants;
    assert_eq!(pennants.len(), PENNANTS);
    assert_eq!(pennants.last(), Some(&false), "newest last");
}

#[test]
fn the_courier_is_gone_exactly_as_long_as_the_answer() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("w"), "web_fetch", "url=https://docs.rs/ratatui", 0);
    assert!(one(&deeds, Errand::Courier).is_some());
    let t = run(&mut deeds, 1, 2_000);
    assert!(
        one(&deeds, Errand::Courier).is_none(),
        "out past the map edge while the request is in flight"
    );
    let t = run(&mut deeds, t, 2_000);
    assert!(one(&deeds, Errand::Courier).is_none(), "still waiting");

    deeds.settle(&id("w"), true, 0);
    let t = run(&mut deeds, t, 2);
    assert!(
        one(&deeds, Errand::Courier).is_none(),
        "still past the edge"
    );
    let t = run(&mut deeds, t, 14);
    let back = one(&deeds, Errand::Courier).expect("he rides back in");
    assert_eq!(back.verdict, Some(true));
    assert!(back.x < 2.0 * TILE as f32, "he comes in at the edge");
    run(&mut deeds, t, 2_000);
    assert!(
        one(&deeds, Errand::Courier).is_none(),
        "home at the Gatehouse"
    );
}

#[test]
fn a_sealed_commit_flies_a_while_then_perches() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("c"), "shell", "git commit -m 'x'", 0);
    let t = run(&mut deeds, 1, 5);
    deeds.settle(&id("c"), true, 0);
    let t = run(&mut deeds, t, FLIGHT / 2);
    let aloft = one(&deeds, Errand::Raven).expect("a quick commit still flies");
    assert!(aloft.moving);
    assert_eq!(deeds.record().ravens, 0);
    run(&mut deeds, t, 2_000);
    assert!(one(&deeds, Errand::Raven).is_none());
    assert_eq!(
        deeds.record().ravens,
        1,
        "one commit, one raven on the roof"
    );

    deeds.begin(&id("c2"), "git_commit", "", 3_000);
    deeds.settle(&id("c2"), false, 0);
    run(&mut deeds, 3_001, 2_000);
    assert_eq!(
        deeds.record().ravens,
        1,
        "a failed commit goes back in unsealed"
    );
}

#[test]
fn searches_are_worked_in_the_fields_and_carried_to_the_scriptorium() {
    let mut deeds = Deeds::default();
    for n in 0..4 {
        deeds.begin(
            &id(&format!("g{n}")),
            "grep",
            "pattern=herald, path=cockpit",
            0,
        );
    }
    let hands: Vec<_> = deeds
        .wayfarers()
        .into_iter()
        .filter(|w| w.errand == Errand::Hand)
        .collect();
    assert_eq!(
        hands.len(),
        3,
        "a crew of three; the fourth search is shared"
    );
    assert!(
        hands.iter().all(|h| (h.x, h.y) == feet(field_gate())),
        "they come out at the field crossroads"
    );
    let t = run(&mut deeds, 1, 300);
    for w in deeds.wayfarers() {
        let (tx, ty) = tile_of((w.x, w.y));
        assert!(
            (0..3).any(|slot| {
                let (a, b) = (row(slot, 0), row(slot, 1));
                ty == a.1 && (a.0.min(b.0)..=a.0.max(b.0)).contains(&tx)
            }),
            "at work on a crop row"
        );
    }
    deeds.settle(&id("g0"), true, t);
    assert_eq!(deeds.record().stooks, 1, "the harvest stands in the field");
    let t = run(&mut deeds, t, 2);
    let messenger = one(&deeds, Errand::Messenger).expect("the finding sets out");
    assert!(messenger.carrying);
    run(&mut deeds, t, 3_000);
    assert!(
        one(&deeds, Errand::Messenger).is_none(),
        "delivered at the Scriptorium"
    );
}

#[test]
fn a_search_that_fails_sends_no_messenger() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("g"), "grep", "pattern=x", 0);
    deeds.settle(&id("g"), false, 0);
    run(&mut deeds, 1, WORK + 5);
    assert!(one(&deeds, Errand::Messenger).is_none());
    assert_eq!(deeds.record().stooks, 0);
}

#[test]
fn villagers_carry_errands_to_the_granary() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("a"), "shell", "ls -la target", 0);
    deeds.begin(&id("b"), "shell", "python3 scripts/x.py", 0);
    let folk: Vec<_> = deeds
        .wayfarers()
        .into_iter()
        .filter(|w| w.errand == Errand::Villager)
        .collect();
    assert_eq!(folk.len(), 2, "each errand has its villager");
    assert!(folk.iter().all(|v| v.carrying));
    assert_ne!(
        (folk[0].x, folk[0].y),
        (folk[1].x, folk[1].y),
        "from their own doors"
    );
    deeds.settle(&id("a"), true, 1);
    deeds.settle(&id("b"), false, 1);
    assert_eq!(
        deeds.record().sacks,
        0,
        "a sack counts once it is delivered"
    );
    let t = run(&mut deeds, 1, 20);
    assert_eq!(
        deeds.wayfarers().len(),
        2,
        "they finish the walk to the granary first"
    );
    run(&mut deeds, t, 3_000);
    assert!(deeds.wayfarers().is_empty());
    assert_eq!(deeds.record().sacks, 1, "only the errand that passed");
}

#[test]
fn a_busy_villager_goes_home_before_it_comes_back_out() {
    let mut deeds = Deeds::default();
    for n in 0..4 {
        deeds.begin(&id(&format!("e{n}")), "shell", "ls", 0);
        deeds.settle(&id(&format!("e{n}")), true, 0);
    }
    // Everyone delivers and turns for home.
    let mut t = 1;
    while deeds
        .wayfarers()
        .iter()
        .any(|w| w.carrying || w.verdict.is_none())
    {
        deeds.step(t);
        t += 1;
        assert!(t < 3_000, "the crew never turned for home");
    }
    assert_eq!(deeds.record().sacks, 4);
    // A fifth errand while all four walk home: nobody turns round.
    deeds.begin(&id("late"), "shell", "echo late", t);
    let mut seen = deeds.wayfarers().len();
    let mut gone_in = false;
    let mut back_out = false;
    for tick in t..t + 3_000 {
        deeds.step(tick);
        let now = deeds.wayfarers();
        gone_in |= now.len() < seen;
        if gone_in && now.iter().any(|w| w.carrying && w.verdict.is_none()) {
            back_out = true;
            break;
        }
        assert!(
            now.iter().all(|w| !w.carrying),
            "nobody picks up a sack on the road home"
        );
        seen = now.len();
    }
    assert!(
        gone_in && back_out,
        "home, in the door, and out again with the sack"
    );
}

#[test]
fn a_failed_errand_comes_home_with_its_sack() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("a"), "shell", "false", 0);
    deeds.settle(&id("a"), false, 0);
    let mut seen_home = false;
    for t in 1..3_000 {
        deeds.step(t);
        if let Some(v) = one(&deeds, Errand::Villager)
            && v.verdict == Some(false)
        {
            assert!(v.carrying, "the sack goes home undelivered");
            seen_home = true;
        }
    }
    assert!(seen_home);
}

#[test]
fn research_sweeps_the_sky_and_an_owl_brings_the_finding() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("r"), "arxiv", "query=sparse attention", 0);
    assert!(deeds.stargazing());
    deeds.settle(&id("r"), true, 5);
    assert!(!deeds.stargazing());
    assert_eq!(deeds.record().stars, 1);
    let owl = one(&deeds, Errand::Owl).expect("an owl leaves the dome");
    assert_eq!((owl.x, owl.y), dome());
    run(&mut deeds, 6, 3_000);
    assert!(one(&deeds, Errand::Owl).is_none(), "delivered");
}

#[test]
fn memory_keeps_a_monk_at_vigil_and_lights_a_candle() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("m"), "memory_recall", "query=bench pin", 0);
    let t = run(&mut deeds, 1, 10);
    assert!(one(&deeds, Errand::Monk).is_some_and(|m| m.carrying));
    deeds.settle(&id("m"), true, t);
    assert_eq!(deeds.record().candles, 1);
    let t = run(&mut deeds, t, 10);
    assert!(
        one(&deeds, Errand::Monk).is_some(),
        "a quick recall still gets its vigil"
    );
    run(&mut deeds, t, 2_000);
    assert!(one(&deeds, Errand::Monk).is_none());
}

#[test]
fn a_new_call_of_the_same_kind_turns_the_party_back_out() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("a"), "cargo", "test", 0);
    deeds.begin(&id("b"), "cargo", "clippy", 0);
    assert_eq!(deeds.wayfarers().len(), 1, "one wagon carries both trials");
    deeds.settle(&id("a"), true, 0);
    run(&mut deeds, 1, 10);
    assert!(deeds.trial_underway(), "the second trial still runs");
    deeds.settle(&id("b"), false, 0);
    run(&mut deeds, 11, 1);
    assert_eq!(one(&deeds, Errand::Wagon).unwrap().verdict, Some(false));
    deeds.begin(&id("c"), "cargo", "test", 12);
    run(&mut deeds, 12, 1);
    assert!(deeds.trial_underway(), "a fresh trial sends it out again");
    assert_eq!(one(&deeds, Errand::Wagon).unwrap().verdict, None);
}

#[test]
fn turn_end_brings_everyone_home_with_nothing_recorded() {
    let mut deeds = Deeds::default();
    deeds.begin(&id("t"), "run_tests", "", 0);
    deeds.begin(&id("w"), "web_fetch", "url=https://example.com", 0);
    deeds.begin(&id("c"), "git_commit", "", 0);
    deeds.begin(&id("e"), "edit_file", "path=src/a.rs", 0);
    let t = run(&mut deeds, 1, 300);
    deeds.abandon();
    assert!(!deeds.trial_underway());
    run(&mut deeds, t, 4_000);
    assert!(deeds.wayfarers().is_empty());
    assert_eq!(*deeds.record(), Record::default());
    deeds.settle(&id("e"), true, 0);
    assert_eq!(deeds.record().blades, 0, "a late result is not a deed");
}

#[test]
fn records_count_distinct_files_and_only_what_succeeded() {
    let mut deeds = Deeds::default();
    for (n, (tool, args, passed)) in [
        ("edit_file", "path=src/a.rs", true),
        ("edit_file", "path=src/a.rs", true),
        ("write_file", "path=src/b.rs", true),
        ("edit_file", "path=src/c.rs", false),
        ("read_file", "path=src/a.rs", true),
        ("read_file", "path=README.md", true),
        ("read_file", "path=missing.md", false),
    ]
    .into_iter()
    .enumerate()
    {
        let call = id(&n.to_string());
        deeds.begin(&call, tool, args, n as u64);
        deeds.settle(&call, passed, 0);
    }
    assert_eq!(deeds.record().blades, 2);
    assert_eq!(deeds.record().books, 2);
    assert_eq!(deeds.sparks(6), Some(3), "the last edit struck at tick 3");
    assert_eq!(deeds.sparks(3 + SPARKS), None);
}

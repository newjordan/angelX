// Citizens are a pure function of (seed, bays, tick): the same holding or
// settlement always shows the same folk at the same tick, and pictures can
// never mint, move or lose a citizen by being drawn.
use super::{Bay, BayState, Job, MAX_CITIZENS, phase, roster};

fn bays(states: &[BayState]) -> Vec<Bay> {
    states
        .iter()
        .map(|&state| Bay { state, job: Job::Make })
        .collect()
}

#[test]
fn empty_places_have_no_citizens() {
    assert!(roster(7, &[]).is_empty());
    assert!(roster(7, &bays(&[BayState::Empty; 10])).is_empty());
}

#[test]
fn roster_is_deterministic_bounded_and_homed() {
    let place = bays(&[
        BayState::Standing,
        BayState::Building,
        BayState::Standing,
        BayState::Empty,
        BayState::Standing,
        BayState::Standing,
        BayState::Standing,
        BayState::Standing,
        BayState::Standing,
        BayState::Standing,
    ]);
    let a = roster(42, &place);
    assert_eq!(a, roster(42, &place), "same seed and bays, same folk");
    assert!(!a.is_empty() && a.len() <= MAX_CITIZENS);
    assert!(a.iter().all(|c| c.home < place.len()));
    assert!(a.iter().all(|c| place[c.home].state != BayState::Empty));
    // One builder works the one rising room; nobody builds a standing room.
    let builders: Vec<_> = a.iter().filter(|c| c.job == Job::Build).collect();
    assert_eq!(builders.len(), 1);
    assert_eq!(builders[0].home, 1);
    // Ids are unique: two citizens never share a walk phase by accident.
    let mut ids: Vec<_> = a.iter().map(|c| c.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), a.len());
}

#[test]
fn haulers_only_run_between_two_lived_in_bays() {
    let one = roster(3, &bays(&[BayState::Standing, BayState::Empty]));
    assert!(one.iter().all(|c| c.job != Job::Haul));
    let many = roster(3, &bays(&[BayState::Standing; 6]));
    let haulers: Vec<_> = many.iter().filter(|c| c.job == Job::Haul).collect();
    assert_eq!(haulers.len(), 2, "one hauler per three standing bays, capped");
    assert!(haulers.iter().all(|c| c.to > c.home));
}

#[test]
fn phase_walks_out_works_and_walks_back_within_span() {
    for span in [0u32, 1, 5, 40] {
        let mut saw_work = span == 0;
        let mut saw_back = span == 0;
        let mut saw_far = span == 0;
        for tick in 0..20_000u64 {
            let p = phase(9, 2, tick, span, 6);
            assert!(p.step <= span, "tick {tick} span {span}: {:?}", p);
            saw_work |= p.working;
            saw_back |= p.back;
            saw_far |= p.step == span;
            assert_eq!(p, phase(9, 2, tick, span, 6), "phase is a pure function");
        }
        assert!(saw_work && saw_back && saw_far, "span {span} never completes a round trip");
    }
    // Different citizens are staggered, not a marching column.
    let steps: Vec<_> = (0..6).map(|id| phase(9, id, 500, 40, 6).step).collect();
    assert!(steps.windows(2).any(|w| w[0] != w[1]), "{steps:?}");
    // A zero stride can never divide by zero or move anybody.
    assert_eq!(phase(9, 2, 500, 40, 0).step, 0);
}

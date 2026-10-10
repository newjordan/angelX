use super::*;

/// Floor one's entrance, quiet, with Fortune's `kind` dare just called.
fn dared(kind: DareKind) -> Run {
    let mut run = Run::new(9, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.phase = Phase::Exploring;
    run.dare = Some(Dare {
        kind,
        since: run.tick,
        count: 0,
        kept: false,
        broken: false,
        viewers: run.audience,
        rose: None,
    });
    run
}

/// The moment `cue` is called this tick, and the dare watches.
fn call(run: &mut Run, cue: &str) {
    let (heard, sounded) = (run.cues.len(), run.sounds.len());
    run.cues.push(cue.into());
    run.watch_dare(heard, sounded);
}

fn kept(run: &Run) -> bool {
    run.dare.as_ref().is_some_and(|d| d.kept)
}

fn broken(run: &Run) -> bool {
    run.dare.as_ref().is_some_and(|d| d.broken)
}

#[test]
fn every_floor_down_brings_a_dare_the_floor_allows() {
    for seed in 0..30 {
        let mut run = Run::new(seed, 1, None);
        for _ in 1..DEEPEST {
            run.descend_for_test();
            assert!(run.dare.is_some(), "floor {}", run.dungeon.depth);
        }
        // The Unknown has no stairs and no guardian.
        let kind = run.dare.as_ref().unwrap().kind;
        assert!(
            !matches!(
                kind,
                DareKind::StandFirm | DareKind::AgainstTheClock | DareKind::MakeItQuick
            ),
            "{kind:?} on the bottom floor"
        );
        assert!(run.cues.iter().any(|c| c.starts_with("dare:")));
    }
}

#[test]
fn two_flawless_rooms_keep_not_a_scratch_and_open_fortunes_purse() {
    let mut run = dared(DareKind::Untouched);
    let gold = run.players[&1].carried.get(Spoil::Gold);
    call(&mut run, "flawless");
    assert!(!kept(&run), "one is not two");
    call(&mut run, "flawless");
    assert!(kept(&run));
    assert_eq!(
        run.players[&1].carried.get(Spoil::Gold),
        gold + 60 + 20 * run.dungeon.depth
    );
    assert_eq!(run.players[&1].carried.get(Spoil::Gem), 1);
    assert!(run.feats.contains(&"daredevil"));
    assert!(run.cues.iter().any(|c| c == "dare_kept"));
    // Kept is kept: more moments pay nothing more.
    call(&mut run, "flawless");
    assert_eq!(run.dares_kept, 1);
}

#[test]
fn a_roll_breaks_stand_firm_and_the_stairs_keep_it_otherwise() {
    let mut run = dared(DareKind::StandFirm);
    run.settle_dare();
    assert!(kept(&run), "no roll by the stairs");
    let mut run = dared(DareKind::StandFirm);
    let (heard, sounded) = (run.cues.len(), run.sounds.len());
    run.sounds.push("roll");
    run.watch_dare(heard, sounded);
    assert!(broken(&run));
    assert!(run.cues.iter().any(|c| c == "dare_broken"));
    run.settle_dare();
    assert!(!kept(&run), "broken stays broken");
}

#[test]
fn the_clock_runs_out_on_against_the_clock() {
    let mut run = dared(DareKind::AgainstTheClock);
    run.tick += CLOCK + 1;
    run.watch_dare(run.cues.len(), run.sounds.len());
    assert!(broken(&run));
}

#[test]
fn a_guardian_felled_in_time_keeps_make_it_quick() {
    let mut run = dared(DareKind::MakeItQuick);
    call(&mut run, "boss_rise:cinderjaw");
    run.tick += 30 * HZ as u64;
    call(&mut run, "boss_fall:cinderjaw");
    assert!(kept(&run));
    let mut run = dared(DareKind::MakeItQuick);
    call(&mut run, "boss_rise:cinderjaw");
    run.tick += QUICK + 1;
    run.watch_dare(run.cues.len(), run.sounds.len());
    assert!(broken(&run), "too slow");
}

#[test]
fn a_show_off_needs_three_hundred_thousand_more() {
    let mut run = dared(DareKind::ShowOff);
    run.thrill(250);
    run.watch_dare(run.cues.len(), run.sounds.len());
    assert!(!kept(&run));
    run.thrill(60);
    run.watch_dare(run.cues.len(), run.sounds.len());
    assert!(kept(&run));
}

#[test]
fn the_stairs_judge_the_dare_before_the_floor_banks() {
    let mut run = dared(DareKind::StandFirm);
    run.descend_for_test();
    assert_eq!(run.dares_kept, 1, "kept at the stairs");
    assert!(
        run.dare.as_ref().is_some_and(|d| !d.kept),
        "a new dare below"
    );
}

#[test]
fn friends_see_the_dare() {
    let mut run = dared(DareKind::Unstoppable);
    call(&mut run, "slay4");
    let live = run.live();
    let mut friend: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    friend.dare = None;
    assert!(friend.apply_live(live));
    assert_eq!(friend.dare.as_ref().map(|d| d.count), Some(1));
    assert_eq!(
        friend
            .dare
            .as_ref()
            .unwrap()
            .standing(friend.tick, friend.audience),
        "1 of 2"
    );
}

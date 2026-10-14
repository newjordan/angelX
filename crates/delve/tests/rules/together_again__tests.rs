use super::*;
use std::collections::BTreeMap;

fn fallen() -> Run {
    let mut run = Run::new(5, 1, Some("Friend"));
    run.phase = Phase::Wiped;
    run
}

#[test]
fn only_a_finished_run_can_be_left_for_the_kingdom() {
    let mut run = Run::new(5, 1, Some("Friend"));
    run.ask_again(2);
    assert!(run.again.is_none(), "a run still playing is not left this way");
    assert!(!run.again_due());
}

#[test]
fn the_party_goes_once_every_player_here_has_asked() {
    let mut run = fallen();
    run.ask_again(2);
    let again = run.again.clone().unwrap();
    assert_eq!((again.asked.len(), again.left), (1, AGAIN_WAIT));
    assert!(!run.again_due(), "knight 1 has not asked");
    run.ask_again(1);
    assert!(run.again_due(), "both have");
}

#[test]
fn the_wait_runs_out_and_an_away_knight_holds_nobody() {
    let mut run = fallen();
    run.ask_again(2);
    let mut ticks = 0;
    while !run.again_due() {
        ticks += 1;
        assert!(ticks <= AGAIN_WAIT, "the wait ends");
    }
    assert_eq!(ticks + 1, AGAIN_WAIT, "five seconds from the first ask");
    // An away knight neither asks nor is waited for.
    let mut run = fallen();
    run.set_away(1, true);
    run.ask_again(1);
    assert!(run.again.is_none());
    run.ask_again(2);
    assert!(run.again_due(), "the one player here asked: the party goes");
}

#[test]
fn a_run_playing_again_forgets_the_asking() {
    let mut run = fallen();
    run.ask_again(2);
    run.phase = Phase::Exploring;
    assert!(!run.again_due());
    assert!(run.again.is_none());
    run.step(&BTreeMap::new());
}

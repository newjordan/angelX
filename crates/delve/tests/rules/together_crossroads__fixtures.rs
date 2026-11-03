//! Progression fixtures finish the interlude before checking the next floor.
use super::*;

pub fn finish(run: &mut Run) {
    if run.crossroads.is_none() { return; }
    let seats = run.crossroads.as_ref().unwrap().seats.clone();
    for &id in &seats {
        let serial = run.playtest.seats.get(&id).map_or(1, |s| s.serial + 1);
        run.vote_command(id, Command { serial, action: Action::Rate(1) });
    }
    run.step(&BTreeMap::new());
    run.finish_crossroads(seats);
    for _ in 0..RESULTS_TIME { run.step(&BTreeMap::new()); }
}

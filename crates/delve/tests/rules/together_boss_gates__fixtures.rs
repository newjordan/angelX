//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use fortune::Mode;

pub fn on_floor(depth: u32, seed: u64, mode: Mode) -> Run {
    let mut run = Run::new(seed, 17, None);
    run.mode = mode;
    let pack = Pack::at(depth, Pack::Crypt);
    run.dungeon = layout::floor_for(mode, depth, pack, &mut run.rng);
    run.populate_boss_gates();
    run.enter(0, None);
    run
}

pub fn finish_room(run: &mut Run) {
    for _ in 0..8 {
        run.clear_for_test();
        if run.room().cleared {
            return;
        }
    }
    panic!(
        "fixture failed to clear room: {:?}, {:?}",
        run.phase, run.enemies
    );
}

pub fn kill_leader(run: &mut Run, which: usize) {
    let w = run.boss_gates.as_ref().unwrap().leaders[which].clone();
    run.enter(w.room, Some(2));
    assert!(run.enemies.iter().any(|e| e.elite && run.carries_key(e)));
    assert!(!run.enemies.iter().any(|e| e.is_boss()));
    finish_room(run);
    assert!(run.boss_gates.as_ref().unwrap().leaders[which].defeated);
    assert!(run.room().cleared);
}

pub fn stronghold(depth: u32, required: usize) -> Run {
    for seed in 0..1000 {
        let run = on_floor(depth, seed, Mode::LongWayDown);
        let g = run.boss_gates.as_ref().unwrap();
        if g.access == boss_gates::Access::All
            && g.leaders.iter().filter(|l| l.prerequisite).count() == required
        {
            return run;
        }
    }
    panic!("missing structure fixture");
}

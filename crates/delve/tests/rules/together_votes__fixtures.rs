//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;

pub fn command(run: &mut Run, seat: u32, action: Action) {
    let serial = run.playtest.seats.get(&seat).map_or(1, |s| s.serial + 1);
    run.vote_command(seat, Command { serial, action });
}

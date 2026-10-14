//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;

pub fn evidence() -> SchoolSnapshot {
    SchoolSnapshot {
        active: true,
        observations: 12,
        checks: 5,
        contrasts: 2,
        inconclusive: 1,
    }
}

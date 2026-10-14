//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use crate::render::overworld::kingdom as geography;

pub fn kingdom() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_kingdom(Some(geography::keep_stand()));
    r
}

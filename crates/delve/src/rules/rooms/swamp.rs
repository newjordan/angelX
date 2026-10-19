//! The Swamp's hand-drawn rooms (see `rooms`): quadrants, `.` floor, `#`
//! a block, `~` a hazard pool.

use super::super::layout::Template;

/// One-screen rooms: 11 by 6 quadrants.
pub const SMALL: &[Template] = &[];

/// Bigger rooms: quadrants of the sizes in `rooms::SIZES`.
pub const GREAT: &[&[&str]] = &[];

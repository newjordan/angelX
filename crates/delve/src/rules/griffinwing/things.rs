//! The things that stand on the rail, as the pictures draw them; whether a
//! craft has flown through one is the module's (`cockpit/live/griffinwing-rules`).
use super::*;

/// A shutter's cycle, in ticks, and the narrowest its gap closes to (half).
pub const SHUTTER_PERIOD: u32 = 150;
pub const SHUTTER_MIN: i32 = dm(5);

/// A shutter's half gap at tick `t`: open and closing and opening again over
/// its cycle, the way it shows from the far end of the rail.
pub fn shutter_gap(p: &Prop, t: u32, wx: i32) -> i32 {
    let k = tri(t + u32::from(p.phase), SHUTTER_PERIOD, 1000) + 1000;
    SHUTTER_MIN + (wx + dm(10) - SHUTTER_MIN) * k / 2000
}

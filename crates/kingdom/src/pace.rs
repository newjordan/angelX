//! One named pace for an evening at the Keep. Off unless
//! `ANGEL_KINGDOM_PACE=evening`. The numbers are a fixed set, picked from the
//! scripted player's measured day pace (first stone near seventeen minutes,
//! the timber castle near seventy-nine), not a multiplier applied at runtime.

/// The evening set is on only for this exact word. Anything else, including
/// an empty value, is the day pace.
pub fn evening() -> bool {
    std::env::var("ANGEL_KINGDOM_PACE").ok().as_deref() == Some("evening")
}

/// Seconds between a works finishing a batch. A shift is ten seconds. The
/// evening set is two: a one-second shift did not bring the first stone
/// forward, and the shell keep's first course landed later.
pub fn job_every() -> u64 {
    if evening() { 2 } else { 10 }
}

/// Travel credit a vehicle gains each second, in tenths of a second. Ten is
/// one clear tile a second. Forty is the evening's cart, wagon and hand-chain.
/// Eighty brought the Keep's first course forward and pushed the first stone back.
pub fn travel_clock() -> u16 {
    if evening() { 40 } else { 10 }
}

/// How often the scripted player gives an order, in seconds. Five is the day.
/// One is the evening.
pub fn act_every() -> u64 {
    if evening() { 1 } else { 5 }
}

/// How often the scripted evening sends a delve home, in seconds of the same clock.
pub const DELVE_EVERY: u64 = 480;
/// The first return, earlier than the eight-minute cadence, so the yard has stone before five minutes.
pub const FIRST_DELVE: u64 = 180;

/// Cut stone a returning delve pays the Keep's yard, evening pace only.
/// Fixed numbers, not a formula. Day pace pays none this way.
/// Depth 1 = 300, 2 = 600, 3 = 1000, 4 = 1500, 5 = 2200, deepest or a boss cleared = 3000.
pub fn delve_pays(depth: u32, boss: bool) -> u32 {
    if !evening() {
        return 0;
    }
    if boss || depth >= 6 {
        return 3000;
    }
    match depth {
        1 => 300,
        2 => 600,
        3 => 1000,
        4 => 1500,
        5 => 2200,
        _ => 0,
    }
}

/// Stone a delve of this depth pays. 0 pale limestone, 1 green-veined, 2 dark basalt, 3 blue ice-stone.
/// Hand-cut block is limestone (depth 0). Depths 1-2 limestone, 3 green-veined, 4 basalt, 5 and deeper ice-stone.
pub fn delve_stone(depth: u32) -> u8 {
    match depth {
        0..=2 => 0,
        3 => 1,
        4 => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delve_pays_a_fixed_stockpile_only_in_the_evening() {
        let evening = std::env::var("ANGEL_KINGDOM_PACE").ok().as_deref() == Some("evening");
        if evening {
            assert_eq!(super::delve_pays(1, false), 300);
            assert_eq!(super::delve_pays(5, false), 2200);
            assert_eq!(super::delve_pays(6, false), 3000);
            assert_eq!(super::delve_pays(2, true), 3000);
        } else {
            assert_eq!(super::delve_pays(5, true), 0);
        }
        assert_eq!(super::delve_stone(0), 0);
        assert_eq!(super::delve_stone(2), 0);
        assert_eq!(super::delve_stone(3), 1);
        assert_eq!(super::delve_stone(4), 2);
        assert_eq!(super::delve_stone(5), 3);
    }

    #[test]
    fn the_day_pace_is_the_ten_second_shift_unless_the_switch_is_on() {
        if std::env::var("ANGEL_KINGDOM_PACE").ok().as_deref() == Some("evening") {
            assert_eq!(job_every(), 2);
            assert_eq!(travel_clock(), 40);
            assert_eq!(act_every(), 1);
        } else {
            assert!(!evening());
            assert_eq!(job_every(), 10);
            assert_eq!(travel_clock(), 10);
            assert_eq!(act_every(), 5);
        }
    }
}

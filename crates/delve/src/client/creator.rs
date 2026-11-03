//! The creator takes hands before gameplay, menus, or ballots do.
use angel_net::input::{Hands, button};

/// Only real button taps are explicit actions. Held axes travel separately
/// so the simulator can throttle navigation repeats.
pub(super) fn mirror_action(hands: &Hands) -> u8 {
    let on = |bits| hands.pressed & bits != 0;
    if on(button::MENU) {
        6
    } else if on(button::USE | button::OK) {
        5
    } else if on(button::UP | button::AIM_UP) {
        1
    } else if on(button::DOWN | button::AIM_DOWN) {
        2
    } else if on(button::LEFT | button::AIM_LEFT) {
        3
    } else if on(button::RIGHT | button::AIM_RIGHT) {
        4
    } else {
        0
    }
}

#[cfg(test)]
#[path = "../../tests/client_creator__tests.rs"]
mod tests;

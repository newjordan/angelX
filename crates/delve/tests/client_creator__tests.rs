use super::*;

#[test]
fn taps_route_walk_aim_confirm_and_close_without_held_gameplay() {
    for (bits, action) in [
        (button::UP, 1),
        (button::DOWN, 2),
        (button::LEFT, 3),
        (button::RIGHT, 4),
        (button::AIM_UP, 1),
        (button::AIM_DOWN, 2),
        (button::AIM_LEFT, 3),
        (button::AIM_RIGHT, 4),
        (button::USE, 5),
        (button::OK, 5),
        (button::MENU, 6),
        (button::FIRE | button::ALT | button::MAP, 0),
    ] {
        let hands = Hands {
            held: bits,
            pressed: bits,
            ..Hands::default()
        };
        assert_eq!(mirror_action(&hands), action);
        assert_eq!(
            mirror_action(&Hands {
                held: bits,
                ..Hands::default()
            }),
            0,
            "buttons are presses, not repeated gameplay holds"
        );
    }
}

#[test]
fn sticks_never_become_explicit_actions_and_buttons_keep_priority() {
    for (x, y, aim_x, aim_y) in [
        (-127, 0, 0, 0),
        (127, 0, 0, 0),
        (0, -127, 0, 0),
        (0, 127, 0, 0),
        (0, 0, -127, 0),
        (0, 0, 127, 0),
        (0, 0, 0, -127),
        (0, 0, 0, 127),
        (127, -127, -127, 127),
    ] {
        let hands = Hands {
            x,
            y,
            aim_x,
            aim_y,
            ..Hands::default()
        };
        assert_eq!(
            mirror_action(&hands),
            0,
            "sticks use the held-axis fallback"
        );
        for (pressed, action) in [
            (button::MENU | button::OK | button::UP, 6),
            (button::USE | button::DOWN, 5),
            (button::UP, 1),
            (button::DOWN, 2),
            (button::LEFT, 3),
            (button::RIGHT, 4),
        ] {
            assert_eq!(mirror_action(&Hands { pressed, ..hands }), action);
        }
    }
}

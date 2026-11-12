use super::*;

fn press(id: u32, input: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, input)])
}

fn holding(plain: u8, great: u8) -> Run {
    let mut run = Run::new(4, 2, Some("Matt"));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.wishes, hero.great_wishes) = (plain, great);
    run
}

#[test]
fn typing_every_letter_on_the_open_scroll_does_not_move_or_fire() {
    let mut run = holding(1, 0);
    assert!(run.open_wish_line(1));
    let x = run.players[&1].x;
    let y = run.players[&1].y;
    let shots = run.projectiles.len();
    let letters = "abcdefghijklmnopqrstuvwxyz";
    for c in letters.chars() {
        assert!(run.type_wish_letter(1, c), "{c}");
        let mut input = Input::default();
        input.move_x = if c == 'a' { -1 } else if c == 'd' { 1 } else { 0 };
        input.move_y = if c == 'w' { -1 } else if c == 's' { 1 } else { 0 };
        input.fire = c == 'f' || c == 'j' || c == ' ';
        input.swing = c == ' ';
        input.dash = c == ' ';
        run.step(&press(1, input));
    }
    assert_eq!(run.players[&1].wish_line, letters);
    assert_eq!(run.players[&1].x, x);
    assert_eq!(run.players[&1].y, y);
    assert_eq!(run.projectiles.len(), shots);
    assert!(!crate::rules::creation::wish_hub_enabled());
}

#[test]
fn lamp_line_stays_a_scroll_until_the_hub_switch() {
    let mut run = holding(1, 0);
    assert!(run.open_wish_line(1));
    assert!(run.players[&1].wish_open);
    assert!(run.players[&1].offer.is_none());
    assert!(!crate::rules::creation::wish_hub_enabled());
}

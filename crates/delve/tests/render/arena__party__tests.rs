use super::super::*;
use crate::rules::{Input, RoomKind};
use std::collections::BTreeMap;

/// Three knights in the first fight of the floor, `ticks` into it.
fn party_of_three(seed: u64, ticks: u32) -> Run {
    let mut run = Run::new(seed, 1, Some("Friend"));
    run.join_seat(3, "Wren");
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .expect("a fight on floor one");
    run.enter_for_test(index);
    let aim = BTreeMap::from([(1, Input { aim_y: -1, ..Default::default() })]);
    for _ in 0..ticks {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&aim);
    }
    run
}

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + BAND_H + HUD_H)
}

#[test]
fn a_seat_screen_is_the_band_over_the_room() {
    let run = party_of_three(11, 60);
    let (w, h) = seat();
    assert_eq!((band_rows(h), hud_rows(h), view_rows(h)), (BAND_H, HUD_H, NATIVE_H));
    let whole = seat_frame(&run, w, h, Some(1));
    assert_eq!((whole.w, whole.h), (w, h));
    // Below the band, the room's frame exactly as a seat's view draws it:
    // with nothing over it.
    let room = in_seat(|| frame_for(&run, w, NATIVE_H, Some(1)));
    for y in 0..NATIVE_H {
        for x in 0..w {
            assert_eq!(whole.get(x, y + BAND_H), room.get(x, y), "room pixel {x},{y}");
        }
    }
    // Under the view, the HUD's band exactly.
    let hud = hud_band(&run, Some(1));
    for y in 0..HUD_H {
        for x in 0..w {
            let want = hud.get(x, y).or(Some(super::super::super::ink::BLACK));
            assert_eq!(whole.get(x, y + BAND_H + NATIVE_H), want, "hud pixel {x},{y}");
        }
    }
}

#[test]
fn nothing_lies_over_a_seats_view() {
    let run = party_of_three(11, 60);
    // A terminal's frame lays the skills bar over its view's foot; a
    // seat's view is the room alone, the bar in the band under it.
    let terminal = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let seat = in_seat(|| frame_for(&run, NATIVE_W, NATIVE_H, Some(1)));
    let foot = |im: &Img| (NATIVE_H - 30..NATIVE_H).flat_map(|y| (0..NATIVE_W).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y) != terminal.get(x, y)).count();
    assert!(foot(&seat) > 500, "the bar is not on the seat's view");
    let hud = hud_band(&run, Some(1));
    let inked = (1..HUD_H).flat_map(|y| (0..NATIVE_W).map(move |x| (x, y))).filter(|&(x, y)| hud.get(x, y).is_some()).count();
    assert!(inked > 1000, "the bar is in the band ({inked})");
}

#[test]
fn a_seat_screen_in_parts_composes_to_its_frame() {
    let (w, h) = seat();
    for (seed, ticks) in [(11, 0), (11, 60), (3, 120), (8, 200)] {
        let run = party_of_three(seed, ticks);
        for focus in [Some(1), Some(3), None] {
            let parts = seat_scene(&run, w, h, focus);
            assert_eq!(parts.view, (w, h));
            assert!(!parts.is_flat());
            assert!(parts.veil.is_none(), "a seat's bar is in its band, on no veil over the view");
            let whole = seat_frame(&run, w, h, focus);
            assert!(parts.compose().words() == whole.words(), "seed {seed} at {ticks} for {focus:?}");
        }
    }
}

/// Where card `i` of `n` begins in the band.
fn card_x(i: i32) -> i32 {
    4 + i * 96
}

#[test]
fn every_knight_has_a_card_and_the_seat_its_own() {
    let run = party_of_three(11, 10);
    let band = party::band(&run, Some(2));
    let inked = |x0: i32, x1: i32| (x0..x1).flat_map(|x| (0..BAND_H - 1).map(move |y| (x, y))).filter(|&(x, y)| band.get(x, y).is_some()).count();
    let n = run.players.len() as i32;
    assert_eq!(n, 3);
    for i in 0..n {
        let x = card_x(i);
        assert!(inked(x, x + 92) > 500, "card {i} is drawn");
    }
    // Past the third card, black paper.
    assert_eq!(inked(card_x(3), NATIVE_W), 0);
    // The seat's own frame is rimmed in brass, the others not.
    let brass = |i: i32| band.get(card_x(i) + 4, 1) == Some(super::super::super::chrome::BRASS_RIM[0]);
    assert!(brass(1));
    assert!(!brass(0) && !brass(2));
}

/// The band's rows of a seat's screen, as `(x, y, pixel)` words.
fn band_words(screen: &Img) -> Vec<Option<super::super::super::ink::Rgb>> {
    (0..BAND_H).flat_map(|y| (0..screen.w).map(move |x| (x, y))).map(|(x, y)| screen.get(x, y)).collect()
}

#[test]
fn knights_in_other_rooms_keep_their_cards_in_every_seats_band() {
    let mut run = party_of_three(11, 10);
    let other = (0..run.dungeon.rooms.len()).find(|&i| i != run.at).expect("another room");
    let ids: Vec<u32> = run.players.keys().copied().collect();
    assert_eq!(ids.len(), 3);
    // One knight waits in another room, one is fallen out there, one stays.
    for &id in &ids[1..] {
        let entry = crate::rules::exits::Entry { at: other, party: BTreeMap::from([(id, (1.0, 1.0))]) };
        run.exits.outside.insert(id, entry);
    }
    run.players.get_mut(&ids[2]).unwrap().hp = 0;
    let (w, h) = seat();
    for &focus in &ids {
        let view = run.view_for_seat(focus).expect("a party apart has views");
        assert!(view.players.len() < ids.len(), "seat {focus}'s view leaves out the knights in the other room");
        let frame = frame_for(&view, w, h - band_rows(h), Some(focus));
        let full = party::lift_frame(&frame, &run, Some(focus));
        let alone = party::lift_frame(&frame, &view, Some(focus));
        assert!(band_words(&full) != band_words(&alone), "the view's own band is the bug: it misses cards");
        let map = MapView { party: Some(run.players.clone()), ..Default::default() };
        let lifted = party::lift_frame_with(&frame, &view, Some(focus), &map);
        assert!(band_words(&lifted) == band_words(&full), "seat {focus}: a card for every knight, wherever they stand");
        let scene = party::lift_with(scene_for(&view, w, h - band_rows(h), Some(focus)), &view, Some(focus), &map);
        assert!(band_words(&scene.compose()) == band_words(&full), "seat {focus}: the window's parts hold the same band");
    }
    // Every card is drawn, and the brass rim is the seat's own.
    let band = party::band_of(&run.view_for_seat(ids[0]).unwrap(), Some(ids[0]), Some(&run.players));
    for i in 0..3 {
        let x = card_x(i);
        let inked = (x..x + 92).flat_map(|x| (0..BAND_H - 1).map(move |y| (x, y))).filter(|&(x, y)| band.get(x, y).is_some()).count();
        assert!(inked > 500, "card {i} is drawn");
    }
    let brass = |i: i32| band.get(card_x(i) + 4, 1) == Some(super::super::super::chrome::BRASS_RIM[0]);
    assert!(brass(0) && !brass(1) && !brass(2));
}

#[test]
fn a_bust_is_the_knights_own_head_and_shoulders_enlarged() {
    let run = party_of_three(11, 10);
    let (id, hero) = run.players.iter().next().unwrap();
    let figure = super::super::knights::portrait(hero, *id);
    let band = party::band(&run, None);
    // Every pixel of the bust is a figure's pixel, in blocks of two.
    let (bx, by) = (card_x(0) + 1, 2);
    let mut seen = 0;
    for y in (0..20).step_by(2) {
        for x in (0..24).step_by(2) {
            let c = band.get(bx + x, by + y);
            assert_eq!(c, band.get(bx + x + 1, by + y + 1), "a block at {x},{y}");
            if let Some(c) = c {
                seen += usize::from((0..figure.h).any(|fy| (0..figure.w).any(|fx| figure.get(fx, fy) == Some(c))));
            }
        }
    }
    assert!(seen > 60, "the bust is the knight's own art ({seen} blocks)");
}

#[test]
fn health_and_mana_are_held_in_the_frame() {
    let mut run = party_of_three(11, 10);
    {
        let hero = run.players.get_mut(&1).unwrap();
        hero.hp = hero.max_hp / 2;
        hero.mana = crate::rules::MAX_MANA / 4.0;
    }
    let band = party::band(&run, None);
    let teal = super::super::super::ink::ink('2').unwrap();
    let blue = super::super::super::ink::ink('z').unwrap();
    let x = card_x(0);
    // The health along the frame's foot, half of it lit.
    let lit = (x + 1..x + 27).filter(|&px| band.get(px, 22) == Some(teal)).count();
    assert_eq!(lit, 13);
    // The mana down its right side, a quarter of it lit from the foot.
    let lit = (2..22).filter(|&py| band.get(x + 25, py) == Some(blue)).count();
    assert_eq!(lit, 5);
    assert_eq!(band.get(x + 25, 21), Some(blue));
}

#[test]
fn a_fallen_knight_has_no_bar_and_its_card_says_so() {
    let mut run = party_of_three(11, 10);
    run.players.get_mut(&1).unwrap().hp = 0;
    // No plates in the fallen knight's band; a standing friend's has them.
    let plates = |id: u32| {
        let hud = hud_band(&run, Some(id));
        (8..HUD_H - 8).flat_map(|y| (6..150).map(move |x| (x, y))).filter(|&(x, y)| hud.get(x, y).is_some()).count()
    };
    assert_eq!(plates(1), 0, "no plates");
    assert!(plates(2) > 300);
    let band = party::band(&run, Some(2));
    let ember = super::super::super::ink::ink('7').unwrap();
    let words = (card_x(0) + 28..card_x(0) + 92).flat_map(|x| (14..22).map(move |y| (x, y))).filter(|&(x, y)| band.get(x, y) == Some(ember)).count();
    assert!(words > 20, "FALLEN is written on the card");
    // The health bar is empty and the bust grey.
    let x = card_x(0);
    let teal = super::super::super::ink::ink('2').unwrap();
    assert!((x + 1..x + 27).all(|px| band.get(px, 22) != Some(teal)));
}

#[test]
fn the_bar_stays_on_the_palette_and_clear_of_the_band() {
    let run = party_of_three(11, 60);
    let (w, h) = seat();
    let parts = seat_scene(&run, w, h, Some(1));
    assert!(parts.veil.is_none(), "the bar lies in the band under the view, not on a veil");
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(whole.pixels().all(|c| c == super::super::super::ink::BLACK || super::super::super::ink::palette_index(c).is_some()));
}

/// A run with knights in seats 1 to `n`, and the first seat the host's
/// empty one when `host_absent`; every knight a long name.
fn party_of(n: u32, host_absent: bool) -> Run {
    let mut run = Run::new(11, 1, Some("Bartholomew"));
    for id in 3..=n {
        run.join_seat(id, &format!("Sir Galahad{id}"));
    }
    if host_absent {
        run.set_absent(1, true);
    }
    for hero in run.players.values_mut() {
        hero.name = format!("{}ABCDEFGHIJ", hero.name);
        hero.hp = hero.max_hp;
    }
    run
}

/// The band's cards for `n` knights: where each begins and ends.
fn spans(n: i32) -> Vec<(i32, i32)> {
    (0..n).map(|i| (super::card_x(i, n), super::card_x(i, n) + super::card_width(n))).collect()
}

/// That every card of a band of `n` is drawn, whole, inside its own span:
/// nothing of one card on the paper between it and the next, nor past the
/// band's margins.
fn assert_cards_fit(run: &Run, n: i32, focus: Option<u32>) {
    let band = party::band(run, focus);
    let spans = spans(n);
    assert_eq!(spans.len() as i32, n);
    let column_inked = |x: i32| (0..BAND_H - 1).any(|y| band.get(x, y).is_some());
    for (i, &(from, to)) in spans.iter().enumerate() {
        assert!(from >= 4, "card {i} begins past the margin");
        assert!(to <= NATIVE_W - 4, "card {i} ends before the margin: {to}");
        let inked = (from..to).flat_map(|x| (0..BAND_H - 1).map(move |y| (x, y))).filter(|&(x, y)| band.get(x, y).is_some()).count();
        assert!(inked > 300, "card {i} of {n} is drawn ({inked})");
        // The paper between two cards stays black.
        if let Some(&(next, _)) = spans.get(i + 1) {
            assert!(to <= next, "card {i} of {n} overlaps the next ({to} past {next})");
            for x in to..next {
                assert!(!column_inked(x), "card {i} of {n} spills onto the paper at {x}");
            }
        }
    }
    // Nothing before the first card or after the last.
    for x in (0..spans[0].0).chain(spans[spans.len() - 1].1..NATIVE_W) {
        assert!(!column_inked(x), "{n} cards: ink at {x} outside them");
    }
}

#[test]
fn five_friends_with_the_host_absent_fill_the_band_with_five_whole_cards() {
    let run = party_of(6, true);
    let n = party::band(&run, Some(2));
    // The absent host has no card.
    let shown = run.players.values().filter(|h| !h.absent).count();
    assert_eq!(shown, 5);
    assert_cards_fit(&run, 5, Some(2));
    drop(n);
    // Roomier than six, and no wider than the four it was.
    assert!(super::card_width(5) > super::card_width(6));
    assert!(super::card_width(5) < super::card_width(4));
}

#[test]
fn six_knights_fill_the_band_with_six_whole_cards() {
    let run = party_of(6, false);
    assert_eq!(run.players.len(), 6);
    assert_cards_fit(&run, 6, Some(1));
    assert_cards_fit(&run, 6, Some(6));
    assert_cards_fit(&run, 6, None);
}

#[test]
fn the_longest_words_stay_inside_the_narrowest_card() {
    // Everyone fallen: FALLEN (or its short form) on every card. Then
    // everyone away, in stone, and on a vigil.
    for n in [5u32, 6] {
        for state in 0..4 {
            let mut run = party_of(6, n == 5);
            for hero in run.players.values_mut() {
                match state {
                    0 => hero.hp = 0,
                    1 => hero.away = true,
                    2 => hero.stone = true,
                    _ => hero.vigil = true,
                }
            }
            assert_cards_fit(&run, n as i32, Some(2));
        }
    }
}

#[test]
fn six_cards_count_their_wishes_in_diamonds() {
    // Six across, WISH and its count will not fit: a gold diamond for each
    // wish held, a hollow one for none, and the own card's key beside them.
    let mut run = party_of(6, false);
    let wishes = [2u8, 0, 3, 1, 0, 3];
    for (hero, &n) in run.players.values_mut().zip(&wishes) {
        (hero.wishes, hero.great_wishes) = (n, 0);
    }
    let band = party::band(&run, Some(1));
    let (tip, hollow) = (super::super::super::ink::ink('t'), super::super::super::ink::ink('J'));
    for (i, &n) in wishes.iter().enumerate() {
        let from = super::card_x(i as i32, 6) + super::FRAME_W;
        let to = super::card_x(i as i32, 6) + super::card_width(6);
        let row = |y: i32, c| (from..to).filter(|&x| band.get(x, y) == c).count();
        // The diamonds' tips, one a wish, on the row under the name.
        assert_eq!(row(16, tip), usize::from(n), "card {i}");
        assert_eq!(row(16, hollow) > 0, n == 0, "card {i}");
    }
    assert_cards_fit(&run, 6, Some(1));
    // Four across still say it in words.
    let mut four = party_of(4, false);
    four.players.values_mut().for_each(|h| h.wishes = 2);
    let band = party::band(&four, Some(1));
    for i in 0..4 {
        let from = super::card_x(i, 4) + super::FRAME_W;
        assert!((from..from + 40).all(|x| band.get(x, 16) != tip), "WISH 2 in words");
    }
}

#[test]
fn a_band_of_up_to_four_is_laid_out_as_it_was() {
    for n in 1..=4 {
        assert_eq!(super::card_width(n), 92.min((NATIVE_W - 8 - 4 * (n - 1)) / n));
        for i in 0..n {
            assert_eq!(super::card_x(i, n), 4 + i * (super::card_width(n) + 4));
        }
    }
    // Four across were 91 wide, 4 apart.
    assert_eq!(super::card_width(4), 91);
}

#[test]
fn six_seats_wear_six_different_colours() {
    use super::super::super::ink::ink;
    let cloth: Vec<[u8; 3]> = (1..=6).map(|id| ink(super::super::knights::seat_cloth(id)).unwrap()).collect();
    for a in 0..6 {
        for b in a + 1..6 {
            let d: i32 = (0..3).map(|k| (i32::from(cloth[a][k]) - i32::from(cloth[b][k])).pow(2)).sum();
            assert!(d > 80 * 80, "seats {} and {} wear alike ({:?} {:?})", a + 1, b + 1, cloth[a], cloth[b]);
        }
    }
    // The knights themselves, each a different figure for its seat.
    let run = party_of(6, false);
    let figures: Vec<Img> = run.players.iter().map(|(&id, h)| super::super::knights::portrait(h, id)).collect();
    for a in 0..6 {
        for b in a + 1..6 {
            let same = figures[a].w == figures[b].w
                && figures[a].h == figures[b].h
                && (0..figures[a].h).all(|y| (0..figures[a].w).all(|x| figures[a].get(x, y) == figures[b].get(x, y)));
            assert!(!same, "seats {} and {} are drawn alike", a + 1, b + 1);
        }
    }
    // A seat's mark on a board or a card is its cloth (gold for the first).
    assert_eq!(super::super::knights::seat_mark(1), '5');
    for id in 2..=6 {
        assert_eq!(super::super::knights::seat_mark(id), super::super::knights::seat_cloth(id));
    }
    assert_eq!(party::seat_ink(5), super::super::knights::seat_cloth(5));
}

/// Knight 1 of a party of three holding `hand`.
fn holding(hand: &[&str]) -> Run {
    let mut run = party_of_three(11, 60);
    run.players.get_mut(&1).unwrap().hand = hand.iter().map(|id| id.to_string()).collect();
    run
}

#[test]
fn the_hand_plates_a_page_drags_are_where_the_band_draws_the_cards() {
    let ours = holding(&["potion", "holy-water", "thunder-scroll"]);
    // The same cards the other way round: only the pictures in the first
    // and last plates differ, every key and place as it was.
    let swapped = holding(&["thunder-scroll", "holy-water", "potion"]);
    for k in [1, 2] {
        let (w, h) = (NATIVE_W * k, (NATIVE_H + BAND_H + HUD_H) * k);
        let plates = party::hand_plates(&ours, Some(1), w, h);
        assert_eq!(plates.iter().map(|p| p.0).collect::<Vec<_>>(), [0, 1, 2], "at {k}x");
        assert_eq!(plates, party::hand_plates(&swapped, Some(1), w, h));
        let top = h - HUD_H * k;
        let (a, b) = (seat_frame(&ours, w, h, Some(1)), seat_frame(&swapped, w, h, Some(1)));
        let mut hit = [0; 3];
        for y in top..h {
            for x in 0..w {
                if a.get(x, y) == b.get(x, y) {
                    continue;
                }
                let inside = plates
                    .iter()
                    .position(|&(_, (px, py, pw, ph))| x >= px && x < px + pw && y >= py && y < py + ph);
                let Some(i) = inside else {
                    panic!("a card's pixel {x},{y} at {k}x lies off every plate {plates:?}");
                };
                hit[i] += 1;
            }
        }
        assert!(hit[0] > 0 && hit[2] > 0 && hit[1] == 0, "at {k}x: {hit:?}");
        // Each plate within the HUD's band, left to right, none over another.
        for pair in plates.windows(2) {
            let ((_, (x0, _, w0, _)), (_, (x1, _, _, _))) = (pair[0], pair[1]);
            assert!(x0 + w0 <= x1, "{plates:?}");
        }
        assert!(plates.iter().all(|&(_, (_, y, _, ph))| y >= top && y + ph <= h));
    }
    // Under a fallen knight's band, or a watcher's, there is nothing to drag.
    let mut fallen = ours.clone();
    fallen.players.get_mut(&1).unwrap().hp = 0;
    assert!(party::hand_plates(&fallen, Some(1), NATIVE_W, NATIVE_H + BAND_H + HUD_H).is_empty());
    let mut empty = ours;
    empty.players.get_mut(&1).unwrap().hand.clear();
    assert!(party::hand_plates(&empty, Some(1), NATIVE_W, NATIVE_H + BAND_H + HUD_H).is_empty());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_hand_shots --
/// --ignored`: a seat holding three cards, its whole screen at a browser's
/// size, and the HUD's band at 8x with the plates a page drags (`hand_plates`)
/// outlined in gold over it.
#[test]
#[ignore]
fn write_hand_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let (w, h) = seat();
    let run = holding(&["potion", "holy-water", "thunder-scroll"]);
    let whole = seat_frame(&run, w, h, Some(1));
    save("hand-seat1-browser", &whole, 3);
    let top = h - HUD_H;
    let mut band = Img::new(w, HUD_H);
    for y in 0..HUD_H {
        for x in 0..w {
            if let Some(c) = whole.get(x, top + y) {
                band.set(x, y, c);
            }
        }
    }
    for (_, (x, y, pw, ph)) in party::hand_plates(&run, Some(1), w, h) {
        let y = y - top;
        for dx in -1..=pw {
            band.put(x + dx, y - 1, 'O');
            band.put(x + dx, y + ph, 'O');
        }
        for dy in -1..=ph {
            band.put(x - 1, y + dy, 'O');
            band.put(x + pw, y + dy, 'O');
        }
    }
    save("hand-band-plates-8x", &band, 8);
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_party_shots -- --ignored`: a
/// party of three in a fight as each seat sees it, at native size, at a
/// browser's size and in a window's; then with one fallen and one away.
#[test]
#[ignore]
fn write_party_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let (w, h) = seat();
    let mut run = party_of_three(11, 90);
    run.players.get_mut(&1).unwrap().mana = crate::rules::MAX_MANA * 0.6;
    save("party-seat1-native", &seat_frame(&run, w, h, Some(1)), 1);
    save("party-seat1-browser", &seat_scene(&run, w, h, Some(1)).compose(), 4);
    save("party-seat2-window", &seat_scene(&run, w, h, Some(2)).compose(), 3);
    run.players.get_mut(&3).unwrap().hp = 0;
    run.players.get_mut(&1).unwrap().away = true;
    run.players.get_mut(&2).unwrap().hp /= 3;
    run.players.get_mut(&2).unwrap().gear = 1;
    save("party-fallen-away-browser", &seat_scene(&run, w, h, Some(2)).compose(), 4);
    save("party-fallen-away-window", &seat_scene(&run, w, h, Some(2)).compose(), 3);
    save("party-band-8x", &party::band(&run, Some(2)), 8);
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_integration_shots
/// -- --ignored`: the merged lines at a seat's whole screen, the party's band
/// over the room: a Bard singing in a fight, each knight's wish on its card,
/// a library guardian standing alone on the second floor, and the entrance
/// rooms at home.
#[test]
#[ignore]
fn write_integration_shots() {
    use crate::rules::{bosses, knights, phrasebook, ults};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, run: &Run| {
        let (w, h) = seat();
        let img = seat_scene(run, w, h, Some(1)).compose();
        let k = 3;
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // A Bard (seat 1) in the first fight, its notes streaming, then its
    // Lullaby sung.
    let mut run = party_of_three(5, 40);
    run.outfit(1, knights::knight("bard").unwrap());
    run.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    for n in 0..24 {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        let inputs = BTreeMap::from([
            (1, Input { aim_y: -1, fire: true, ult: n == 14, ..Default::default() }),
            (2, Input { aim_x: 1, fire: true, ..Default::default() }),
        ]);
        run.step_sim_for_test(&inputs);
    }
    save("bard-fight", &run);
    // The wish on each knight's card: a cleared room's window, one knight's
    // wish spent.
    let mut run = party_of_three(7, 10);
    for _ in 0..8 {
        run.clear_for_test();
        if run.room().cleared {
            break;
        }
    }
    let _ = run.grant(2, &phrasebook::get("volley").unwrap());
    save("wish-band", &run);
    // A library guardian on the second floor, alone in its room.
    let alone = (0..64u64).find_map(|seed| {
        let mut run = Run::new(seed, 1, Some("Friend"));
        run.join_seat(3, "Wren");
        run.descend_for_test();
        run.clear_prerequisites_for_test();
        let g = run.boss_gates.as_ref()?.guardian;
        run.enter(g, Some(2));
        let boss = run.enemies.iter().find(|e| e.is_boss())?;
        let art = bosses::Art::for_id(&run.bosses[usize::from(boss.boss?)].id)?;
        (run.dungeon.depth == 2 && run.enemies.len() == 1 && boss_art::library_registered(art)).then_some(run)
    });
    let mut run = alone.expect("a library guardian on floor two");
    // The party a few strides below it, so the camera has it in view, and
    // the floor's opening boards gone by.
    let (bx, by) = run.enemies.iter().find(|e| e.is_boss()).map(|e| (e.x, e.y)).unwrap();
    for (k, hero) in run.players.values_mut().enumerate() {
        (hero.x, hero.y) = (bx - 4.0 + 4.0 * k as f32, by + 7.0);
        hero.invulnerable = 100_000;
    }
    for _ in 0..240 {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        let aim = BTreeMap::from([(1, Input { aim_y: -1, fire: true, ..Default::default() })]);
        run.step_sim_for_test(&aim);
    }
    save("library-guardian-floor2", &run);
    // The entrance rooms at home: the Dented Anvil's smithy and the muster
    // house.
    save("entrance-smithy", &Run::latch_for_test());
    let mut run = Run::rafterwake_for_test();
    run.floor_plates.clear();
    save("entrance-workshop", &run);
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_six_seat_shots -- --ignored`:
/// the band for five friends (the host absent) and for six knights, at 8x,
/// and a six-seat fight as the first seat sees it.
#[test]
#[ignore]
fn write_six_seat_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let (w, h) = seat();
    let mut run = party_of(6, false);
    for hero in run.players.values_mut() {
        hero.name = hero.name.chars().take(6).collect();
    }
    save("six-band-8x", &party::band(&run, Some(1)), 8);
    run.players.get_mut(&3).unwrap().hp = 0;
    run.players.get_mut(&4).unwrap().away = true;
    save("six-band-fallen-away-8x", &party::band(&run, Some(2)), 8);
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.enter_for_test(index);
    for _ in 0..90 {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp.max(1);
        }
        run.step(&BTreeMap::new());
    }
    save("six-seat1-native", &seat_frame(&run, w, h, Some(1)), 3);
    let mut five = party_of(6, true);
    for hero in five.players.values_mut() {
        hero.name = hero.name.chars().take(7).collect();
    }
    save("five-band-8x", &party::band(&five, Some(2)), 8);
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_shield_bar_shots
/// -- --ignored`: a knight's kite shield raised and worn, then knocked
/// aside, at a seat's whole screen and its HUD's band at 6x: the meter on
/// the guard's plate and under the knight.
#[test]
#[ignore]
fn write_shield_bar_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let (w, h) = seat();
    let band = |whole: &Img| {
        let top = h - HUD_H;
        let mut band = Img::new(w, HUD_H);
        for y in 0..HUD_H {
            for x in 0..w {
                if let Some(c) = whole.get(x, top + y) {
                    band.set(x, y, c);
                }
            }
        }
        band
    };
    let mut run = party_of_three(11, 60);
    let card = run.book.get("kite-shield").cloned().unwrap();
    for id in [1, 2] {
        let hero = run.players.get_mut(&id).unwrap();
        hero.guard = card.guard.unwrap();
        hero.guard_card = Some(card.id.clone());
    }
    let most = crate::rules::shield::Wear::most(run.players[&1].max_hp);
    let hero = run.players.get_mut(&1).unwrap();
    hero.shielding = true;
    hero.shield_wear.worn = most * 45 / 100;
    let hero = run.players.get_mut(&2).unwrap();
    hero.shield_wear.down = crate::rules::shield::SHIELD_DOWN - 2;
    let whole = seat_frame(&run, w, h, Some(1));
    save("shield-raised-worn-seat1", &whole, 3);
    save("shield-raised-worn-band-6x", &band(&whole), 6);
    let whole = seat_frame(&run, w, h, Some(2));
    save("shield-knocked-aside-seat2", &whole, 3);
    save("shield-knocked-aside-band-6x", &band(&whole), 6);
}

/// The Fairground's Barrel round is a pastime room, and its six course
/// flags now stand among the crowd by their feet (`pastimes::crowd`). A
/// window tilts them with the room, and the terminal composes the same
/// pixels flat: `seat_scene(…).compose()` equals `seat_frame(…)` byte
/// for byte, and each flag arrives as a `Figure` with its own depth.
#[test]
fn a_fairground_barrel_round_composes_to_its_frame_and_stands_its_flags_up() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::{Game, FLAGS};
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let fair = world::room_of(&run.dungeon, Game::Barrel.room()).unwrap();
    run.arrive(fair, Game::Barrel.start());
    // A friend waits by the stall so only the starter arms the round.
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (21.0, 11.5);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Barrel),
        "the pad armed a Barrel round"
    );
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "a Barrel room's parts put back its frame"
    );
    // Each of the six course flags stands among the figures on its own
    // depth; none of them is baked into the room's floor plane any more.
    let flags_at = |d: f32| parts.figures.iter().filter(|f| (f.depth - d).abs() < 0.001).count();
    for &(_, ty) in FLAGS.iter() {
        assert!(flags_at(ty * 2.0) > 0, "a flag stands at depth {}", ty * 2.0);
    }
}

/// The Riverbank's Washtub rings drift every tick — `pastimes::wake` — so
/// each ring's crowd depth is recomputed on the frame. Same contract as
/// Barrel: parts compose to the flat frame byte-for-byte, and each ring
/// arrives as a `Figure` at the tile's own depth.
#[test]
fn a_riverbank_washtub_round_composes_to_its_frame_and_stands_its_rings_up() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::{self, Game};
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let river = world::room_of(&run.dungeon, Game::Washtub.room()).unwrap();
    run.arrive(river, Game::Washtub.start());
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (21.0, 11.5);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Washtub),
        "the pad armed a Washtub round"
    );
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "a Washtub room's parts put back its frame"
    );
    let ring_at = |d: f32| parts.figures.iter().filter(|f| (f.depth - d).abs() < 0.001).count();
    for i in 0..6 {
        let (_, ty) = pastimes::wake(i, run.tick);
        assert!(ring_at(ty * 2.0) > 0, "a ring stands at depth {}", ty * 2.0);
    }
}

/// The Fairground's Turnip ball has its feet on the ring but its bounce
/// baked into the sprite — the crowd entry stands the whole thing at the
/// ring tile's depth (`ty * 2.0`), so the picture leans with the room the
/// same way a Barrel flag does while the ball still rises and falls.
#[test]
fn a_fairground_turnip_round_composes_to_its_frame_and_stands_its_ball_up() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::Game;
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let fair = world::room_of(&run.dungeon, Game::Turnip.room()).unwrap();
    run.arrive(fair, Game::Turnip.start());
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (21.0, 11.5);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Turnip),
        "the pad armed a Turnip round"
    );
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "a Turnip room's parts put back its frame"
    );
    let (_, ty) = Game::Turnip.start();
    let ball_at = parts.figures.iter().filter(|f| (f.depth - ty * 2.0).abs() < 0.001).count();
    assert!(ball_at > 0, "the turnip stands at depth {}", ty * 2.0);
}

/// The Causeway's six Hobbyhorse hurdles stand along the riding lane at
/// its foot (`RIDING_LANE.1`). Each is a Figure now, all at that same
/// depth so they scale with the room's tilt as a row; the lane dashes and
/// the rider's own hobbyhorse stay with the flat scene.
#[test]
fn a_causeway_hobbyhorse_round_composes_to_its_frame_and_stands_its_hurdles_up() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::{Game, RIDING_LANE};
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let way = world::room_of(&run.dungeon, Game::Hobbyhorse.room()).unwrap();
    run.arrive(way, Game::Hobbyhorse.start());
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (21.0, 11.5);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Hobbyhorse),
        "the pad armed a Hobbyhorse round"
    );
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "a Hobbyhorse room's parts put back its frame"
    );
    let foot = RIDING_LANE.1 * 2.0;
    let at_foot = parts.figures.iter().filter(|f| (f.depth - foot).abs() < 0.001).count();
    assert!(at_foot >= 6, "six hurdles stand at the lane's foot ({at_foot})");
}

/// The Crag's six Sled gates alternate near and far down the hillside, so
/// a window leans them each by their own row's depth. Same contract as
/// Barrel: parts compose to the frame byte-for-byte, and every gate's
/// row shows up as a distinct figure depth.
#[test]
fn a_crag_sled_round_composes_to_its_frame_and_stands_its_gates_up() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::{Game, SLED_GATES};
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let crag = world::room_of(&run.dungeon, Game::Sled.room()).unwrap();
    run.arrive(crag, Game::Sled.start());
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (21.0, 11.5);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Sled),
        "the pad armed a Sled round"
    );
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "a Sled room's parts put back its frame"
    );
    let gate_at = |d: f32| parts.figures.iter().filter(|f| (f.depth - d).abs() < 0.001).count();
    for &(_, gy) in SLED_GATES.iter() {
        let d = (gy * 2) as f32;
        assert!(gate_at(d) > 0, "a gate stands at depth {d}");
    }
}

/// The Timberyard's four jump-logs stand at the lane's foot (row 10), so
/// a window leans them as the cart passes between. Cart itself stays flat
/// (the moving rider). All four logs share that foot depth; the check is
/// that no fewer than four figures stand there.
#[test]
fn a_timberyard_cart_round_composes_to_its_frame_and_stands_its_logs_up() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::Game;
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let yard = world::room_of(&run.dungeon, Game::Cart.room()).unwrap();
    run.arrive(yard, Game::Cart.start());
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (21.0, 11.5);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Cart),
        "the pad armed a Cart round"
    );
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "a Cart room's parts put back its frame"
    );
    let foot = 10.0 * 2.0;
    let at_foot = parts.figures.iter().filter(|f| (f.depth - foot).abs() < 0.001).count();
    assert!(at_foot >= 4, "four logs stand at the lane's foot ({at_foot})");
}

/// Tier 2's vertical slice: a Fairground DuckShoot round takes the canvas
/// for itself (a `three.rs` pinhole stage). Its parts (`stage::parts`) lay
/// the CPU backdrop on the view's floor and the cartridge HUD on the view's
/// screen — with the frame's tilt pinned to `0.0` (`pastime_scene` in
/// `stage_frame`) so the pinhole render is not tipped twice. The parity is
/// that Glass composes to the same pixels the flat path paints.
#[test]
fn a_fairground_duckshoot_stage_goes_out_in_parts_and_composes_to_its_frame() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::Game;
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let green = world::room_of(&run.dungeon, Game::DuckShoot.room()).unwrap();
    run.arrive(green, Game::DuckShoot.start());
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (11.5, 4.5);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::DuckShoot),
        "the pad armed a DuckShoot round"
    );
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "a take-over stage's parts put back its frame"
    );
    // The scaffold is real: the stage goes out as a backdrop laid under
    // the marks (`under` is set, `over` empty, no ground) rather than as
    // one flat picture wrapped by `RoomScene::flat`.
    assert!(!parts.is_flat(), "the stage went out in parts, not as one picture");
    assert!(parts.under.is_some(), "the CPU backdrop is a laid-under layer");
    assert!(parts.over.is_none(), "the stage paints nothing over it");
    assert!(parts.ground.is_none(), "the stage has no room still art");
    assert!(
        parts.figures.iter().any(|f| f.height > 0.0),
        "a duck lifts off the boards (drop-shadow)"
    );
    assert!(
        parts.figures.len() >= 3,
        "the three rails each stand a duck figure ({})",
        parts.figures.len()
    );
}

/// Tier 3's particles: a downed duck throws a spark burst at its own screen
/// rect, seen by a GPU window only. The CPU `RoomScene::compose()` does not
/// read `emitters`, so parts-to-frame parity is unaffected; this test drives
/// the runtime until a duck goes down and asserts both that the burst is
/// there AND that the parity invariant still holds over the whole window.
#[test]
fn a_duckshoot_duck_that_goes_down_throws_sparks_and_keeps_its_frame() {
    use crate::render::overworld::arena::room_scene::Motes;
    use crate::rules::home::Home;
    use crate::rules::pastimes::Game;
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let green = world::room_of(&run.dungeon, Game::DuckShoot.room()).unwrap();
    run.arrive(green, Game::DuckShoot.start());
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (11.5, 4.5);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::DuckShoot),
        "the pad armed a DuckShoot round"
    );
    // Down a rail's duck by hand so the burst is guaranteed on the very next
    // frame — the shot-to-hit dance is the rules' concern, and already
    // covered there; here we only need the render-side wiring.
    run.pastimes.round.as_mut().unwrap().duckshoot.as_mut().unwrap().ducks[1].down = 1;
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "sparks aside, the parts still put back the frame"
    );
    let burst = parts
        .emitters
        .iter()
        .find(|e| e.motes == Motes::Sparks && e.age.is_some());
    assert!(
        burst.is_some(),
        "a just-hit duck pushes a Spark burst ({} emitters)",
        parts.emitters.len()
    );
    let e = burst.unwrap();
    assert!(e.strength > 0.0 && e.strength <= 1.0, "burst strength 0..1, got {}", e.strength);
    assert!(e.rect[2] > 0.0 && e.rect[3] > 0.0, "burst rect has size");
}

/// Tier 3's drop-shadow plumbing: an active course marker (the Barrel's
/// "next" flag) is emitted as a `Figure` with `height > 0.0`, so a Glass
/// window draws a shadow under it. The lift is invisible to the CPU paths
/// (`expand()` and `RoomScene::compose()` do not read `height`) so the
/// parity gate stays green either way — this test just checks the plumbing
/// reaches the figure.
#[test]
fn a_fairground_barrel_round_lifts_its_next_flag_off_the_deck() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::Game;
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let green = world::room_of(&run.dungeon, Game::Barrel.room()).unwrap();
    run.arrive(green, Game::Barrel.start());
    let friend = run.players.get_mut(&3).unwrap();
    (friend.x, friend.y) = (2.5, 6.5);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Barrel),
        "the pad armed a Barrel round"
    );
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "a lift cannot shift a CPU pixel"
    );
    let lifted = parts.figures.iter().filter(|f| f.height > 0.0).count();
    assert!(lifted >= 1, "the next flag lifts off the deck ({lifted} lifted)");
    let glowing = parts.figures.iter().filter(|f| f.additive).count();
    assert!(glowing >= 1, "the next flag adds light on the compositor ({glowing} glowing)");
}

/// Halfpipe is the plan's next take-over stage. If its room happens to be
/// NATIVE-sized it goes out in parts and stays byte-exact; if not, it takes
/// today's flat path — which must still compose to its own frame. The
/// assertion is on the parity, not the split; the split is opportunistic
/// (`takeover_parts`).
#[test]
fn a_mummers_halfpipe_round_still_composes_to_its_frame() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::Game;
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let green = world::room_of(&run.dungeon, Game::Halfpipe.room()).unwrap();
    run.arrive(green, Game::Halfpipe.start());
    let friend = run.players.get_mut(&3).unwrap();
    let (fx, fy) = Game::Halfpipe.start();
    (friend.x, friend.y) = (fx, fy);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Halfpipe),
        "the pad armed a Halfpipe round"
    );
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "a halfpipe stage's parts put back its frame"
    );
}

/// Tier 3's particles on the Halfpipe: a landed trick (or a lip launch)
/// throws a spark burst at the skater's own screen rect, seen by a GPU
/// window only. The CPU `compose` does not read `emitters`, so parity is
/// unaffected; this drives the runtime to `trick_sparkle > 0` and asserts
/// both that a burst is there AND that the parts still put back the frame.
#[test]
fn a_mummers_halfpipe_landing_throws_sparks_and_keeps_its_frame() {
    use crate::render::overworld::arena::room_scene::Motes;
    use crate::rules::home::Home;
    use crate::rules::pastimes::Game;
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(11, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let green = world::room_of(&run.dungeon, Game::Halfpipe.room()).unwrap();
    run.arrive(green, Game::Halfpipe.start());
    let friend = run.players.get_mut(&3).unwrap();
    let (fx, fy) = Game::Halfpipe.start();
    (friend.x, friend.y) = (fx, fy);
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Halfpipe),
        "the pad armed a Halfpipe round"
    );
    run.pastimes.round.as_mut().unwrap().halfpipe.as_mut().unwrap().trick_sparkle = 5;
    let parts = seat_scene(&run, w, h, Some(1));
    let whole = seat_frame(&run, w, h, Some(1));
    assert!(
        parts.compose().words() == whole.words(),
        "sparks aside, the parts still put back the frame"
    );
    let burst = parts
        .emitters
        .iter()
        .find(|e| e.motes == Motes::Sparks && e.age.is_some());
    assert!(
        burst.is_some(),
        "a landed trick pushes a Spark burst ({} emitters)",
        parts.emitters.len()
    );
}

/// Tier 2's third take-over: a Pasture Siege round fires, and the shot in
/// the air is stood up as a lifted, glowing `Figure` so a GPU window
/// shadows the arc. The split is opportunistic — the sprite only lifts once
/// it clears the engine and the keep — but parity holds on every frame,
/// whether the shot lifts that tick or stays baked into the backdrop.
#[test]
fn a_pasture_siege_round_stands_its_shot_up_and_composes_to_its_frame() {
    use crate::rules::home::Home;
    use crate::rules::pastimes::{self, Game};
    use crate::rules::world;
    let (w, h) = seat();
    let mut run = Run::at_home(41, 1, Some("Friend"), Home::default(), Default::default());
    run.join_seat(3, "Wren");
    run.come_to_the_gate();
    let green = world::room_of(&run.dungeon, Game::Siege.room()).unwrap();
    run.arrive(green, Game::Siege.start());
    run.step(&BTreeMap::new());
    assert_eq!(
        run.pastimes.round.as_ref().map(|r| r.game),
        Some(Game::Siege),
        "the pad armed a Siege round"
    );
    // March the trail: one event, then mourn, until the walls.
    for _ in 0..(pastimes::EVENT_WAIT + pastimes::MOURN + 4) {
        run.tick_pastimes(&BTreeMap::new());
    }
    assert!(
        run.pastimes.round.as_ref().unwrap().siege.as_ref().unwrap().fighting,
        "the first castle begins"
    );
    // Dial aim and power to the top, one step a tick, then loose. The seat
    // that owns the engine takes the controls (owner 2 here).
    let dials = BTreeMap::from([(
        2u32,
        Input { aim_y: 1, move_x: 1, ..Default::default() },
    )]);
    for _ in 0..12 {
        run.tick_pastimes(&dials);
    }
    let fire = BTreeMap::from([(2u32, Input { fire: true, ..Default::default() })]);
    run.tick_pastimes(&fire);
    // Step the shot up into open sky; parity is the invariant, the lift is
    // opportunistic and must show at least once along the arc.
    let mut lifted = 0;
    for _ in 0..14 {
        run.tick_pastimes(&BTreeMap::new());
        let parts = seat_scene(&run, w, h, Some(1));
        let whole = seat_frame(&run, w, h, Some(1));
        assert!(
            parts.compose().words() == whole.words(),
            "a siege stage's parts put back its frame"
        );
        assert!(!parts.is_flat(), "the siege stage goes out in parts, not one picture");
        if parts.figures.iter().any(|f| f.height > 0.0 && f.additive) {
            lifted += 1;
        }
    }
    assert!(
        lifted >= 1,
        "the shot in the air lifts off the field as a glowing figure at least once ({lifted} frames)"
    );
}

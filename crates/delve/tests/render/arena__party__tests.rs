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

//! Pixel contracts independent of screenshot exports.
use super::*;
use crate::render::overworld::ink::ink;

fn region(im: &Img, x: i32, y: i32, w: i32, h: i32) -> Vec<Option<[u8; 3]>> {
    (y..y + h).flat_map(|yy| (x..x + w).map(move |xx| im.get(xx, yy))).collect()
}

fn party() -> Run {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    for id in 2..=6 { r.join_seat(id, &format!("Seat {id}")); }
    r.walk_alehouse();
    r
}

#[test]
fn render_readability_wall_has_six_separate_pegs_clear_of_tables_and_routes() {
    let mut cv = Img::new(576, 320);
    arms_rack(&mut cv);
    assert_eq!(a::home_arms().len(), 6);
    for slot in 0..6 {
        let x = (a::arm_stand(slot).0 * T as f32) as i32;
        assert_eq!(cv.get(x, 286), ink('t'), "brass peg {slot}");
        assert!(x + 10 < 17 * T, "wall stops before front door");
        assert!(region(&cv, x - 8, 292, 17, 22).iter().any(Option::is_some));
    }
    assert!(region(&cv, 0, 0, 576, 272).iter().all(Option::is_none), "wall must not cross tables or bar aisle");
    assert!(region(&cv, 17 * T, 272, 3 * T, 48).iter().all(Option::is_none), "front door stays clear");
}

#[test]
fn render_readability_prompts_are_compact_black_paper_and_screen_docked_for_six_seats() {
    let mut r = party();
    for slot in 0..6 {
        let (x, y) = a::arm_stand(slot);
        let h = r.players.get_mut(&(slot as u32 + 1)).unwrap();
        (h.x, h.y) = (x * 2.0, y * 2.0);
    }
    for id in 1..=6 {
        let lines = prompt(&r, id, Spot::Arm(id as usize - 1));
        assert!(lines.len() <= 3, "no giant prose per peg");
        let mut cv = Img::new(super::super::NATIVE_W, super::super::NATIVE_H);
        screen_boards(&mut cv, &r, Some(id));
        let board = prompt_board(&lines);
        let (x, y) = dock(&cv, &board);
        assert!(x >= 4 && y >= 4 && x + board.w <= cv.w - 4 && y + board.h <= cv.h - 4);
        assert_eq!(cv.get(x + 2, y + board.h - 3), Some([0, 0, 0]));
        for other in 1..=6 {
            let card = &a::home_arms()[other - 1];
            assert_eq!(lines.iter().any(|(s, _)| s == &card.name.to_uppercase()), other == id as usize);
        }
        let nonempty_rows = (0..cv.h).filter(|&yy| (0..cv.w).any(|xx| cv.get(xx, yy).is_some())).count();
        assert_eq!(nonempty_rows, board.h as usize, "one dock, not six colliding prompts");
        // Both actual hot paths must carry that same rectangle after the
        // room crop: flat/terminal frame layers and layered window scenes.
        let (_, over) = super::super::layers(&r, Some(id), true);
        assert_eq!(region(&over, x, y, board.w, board.h), region(&cv, x, y, board.w, board.h));
        let scene = super::super::scene_for(&r, super::super::NATIVE_W, super::super::NATIVE_H, Some(id));
        assert_eq!(region(&scene.screen, x, y, board.w, board.h), region(&cv, x, y, board.w, board.h));
    }
    let mut overview = Img::new(576, 320);
    boards(&mut overview, &r, None);
    let rows = (0..overview.h).filter(|&y| (0..overview.w).any(|x| overview.get(x, y).is_some())).count();
    let expected = prompt_board(&prompt(&r, 1, Spot::Arm(0)));
    assert_eq!(rows, expected.h as usize, "overview shows exactly one focused interaction");
    let (x, y) = dock(&overview, &expected);
    assert_eq!(region(&overview, x, y, expected.w, expected.h), region(&expected, 0, 0, expected.w, expected.h));
}

#[test]
fn render_readability_every_patron_is_in_reach_and_the_board_says_its_verb() {
    let mut r = party();
    let lines = prompt(&r, 1, Spot::Toy(Toy::Board));
    assert_eq!(lines[0].0, Spot::Toy(Toy::Board).verb());
    // The recruits are in reach in the muster tent (`muster_tent`).
    for i in (0..PATRONS.len()).filter(|&i| a::patron_room(i) == RoomKind::Alehouse) {
        let p = &PATRONS[i];
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y) = (p.at.0 * 2.0, p.at.1 * 2.0);
        assert_eq!(near_spot(&r, 1), Some(Spot::Patron(i)), "{} is in reach", p.kit);
    }
}

#[test]
fn cached_patron_bodies_match_builtin_art_for_every_kit() {
    for (i, p) in PATRONS.iter().enumerate() {
        let colours = crate::rules::knights::knight(p.kit).unwrap().colours;
        let original = match crate::rules::champions::Champion::of_kit(p.kit) {
            Some(c) => super::super::champions::standing(c),
            None => knights::of_kit(Some(p.kit), colours),
        };
        assert_eq!(patron_bodies()[i].words(), original.words(), "{} cache changed pixels", p.kit);
    }
}

#[test]
fn mounted_stock_handles_stay_readable_on_black_paper() {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_alehouse();
    let cv = scenery(&run);
    for slot in [4, 5] {
        let x = (a::arm_stand(slot).0 * T as f32) as i32;
        let art = super::super::cards::card_art(&a::home_arms()[slot]);
        for yy in 3..art.h {
            for xx in 0..art.w {
                let Some(source) = art.get(xx, yy).filter(|c| *c != BLACK) else { continue };
                if source[0].max(source[1]).max(source[2]) < 128 {
                    let c = cv.get(x - art.w / 2 + xx, 292 + yy).expect("solid handle");
                    let luma = (u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2])) / 10;
                    assert!(luma >= 80, "slot {slot} handle at {xx},{yy} is lost against black: {c:?}");
                }
            }
        }
    }
}

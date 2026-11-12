//! Home fixtures use the same seat-local interaction boards as the tavern.
use super::*;

#[test]
fn every_home_arm_prompt_names_the_arm_and_explains_the_free_hold_or_swap() {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_alehouse();
    for (slot, card) in a::home_arms().iter().enumerate() {
        r.players.get_mut(&1).unwrap().arm = None;
        let lines = prompt(&r, 1, Spot::Arm(slot));
        assert!(lines.iter().any(|(l, _)| l == &card.name.to_uppercase()));
        assert!(lines.iter().any(|(l, _)| l.contains("FREE")));
        assert!(lines.iter().any(|(l, _)| l.contains("HOLD E TO EQUIP")));
        r.players.get_mut(&1).unwrap().arm = Some(card.id.clone());
        let lines = prompt(&r, 1, Spot::Arm(slot));
        assert!(lines.iter().any(|(l, _)| l.contains("EQUIPPED") && l.contains("SWAP")));
    }
    assert!(crate::client::sheet(&r, 1).contains("FREE WEAPONS WALL"));
}

/// Local repeatable hot-path probe, not an exporter or a host workload.
#[test]
#[ignore]
fn measure_tavern_figures_hot_path() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_alehouse();
    let mut cv = Img::new(576, 320);
    figures(&mut cv, &r);
    let start = std::time::Instant::now();
    for _ in 0..240 {
        figures(&mut cv, &r);
        std::hint::black_box(&cv);
    }
    eprintln!("tavern figures cached: {:?} / 240 frames", start.elapsed());
    // Isolate the removed work on the same final visible page, independent
    // of which three kits were permanently visible in an earlier draft.
    let start = std::time::Instant::now();
    for _ in 0..240 {
        for p in PATRONS.iter().enumerate().filter(|(i, _)| a::patron_visible(0, *i)).map(|(_, p)| p) {
            let colours = crate::rules::knights::knight(p.kit).unwrap().colours;
            let body = match crate::rules::champions::Champion::of_kit(p.kit) {
                Some(c) => super::super::champions::standing(c),
                None => knights::of_kit(Some(p.kit), colours),
            };
            std::hint::black_box(body);
        }
    }
    eprintln!("removed builtin body construction: {:?} / 240 frames", start.elapsed());
}

/// The four code-drawn knights are picked at the pub like anyone else, and a
/// picker in any seat is drawn exactly as that design's own seat is plain.
#[test]
fn the_code_drawn_knights_are_at_the_pub_and_draw_as_their_seats_plain_knight() {
    use crate::rules::knights::plain_seat;
    let picks: Vec<(usize, u32)> = PATRONS
        .iter()
        .enumerate()
        .filter_map(|(i, p)| plain_seat(p.kit).map(|seat| (i, seat)))
        .collect();
    assert_eq!(picks.iter().map(|(_, s)| *s).collect::<Vec<_>>(), [1, 2, 3, 4]);
    for (i, seat) in picks {
        assert_eq!(a::patron_room(i), crate::rules::RoomKind::Alehouse, "{} sits in the pub", PATRONS[i].kit);
        let mut plain = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
        for id in 2..=4 {
            plain.join_seat(id, &format!("Seat {id}"));
        }
        let wanted = knights::figure_at(&plain.players[&seat], seat, false, Default::default(), false, 0);
        // Someone in the sixth seat, nothing like the design's own.
        let mut run = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
        run.join_seat(6, "Seat 6");
        run.walk_alehouse();
        run.pick_patron(6, i);
        let hero = &run.players[&6];
        assert_eq!(hero.knight.as_deref(), Some(PATRONS[i].kit));
        assert_eq!(hero.ult(), plain.players[&seat].ult(), "{} keeps a plain knight's ultimate", PATRONS[i].kit);
        let mut drawn = hero.clone();
        (drawn.x, drawn.y, drawn.aim_x, drawn.aim_y) = {
            let p = &plain.players[&seat];
            (p.x, p.y, p.aim_x, p.aim_y)
        };
        let got = knights::figure_at(&drawn, 6, false, Default::default(), false, 0);
        assert_eq!((got.w, got.h), (wanted.w, wanted.h), "{}", PATRONS[i].kit);
        for y in 0..got.h {
            for x in 0..got.w {
                assert_eq!(got.get(x, y), wanted.get(x, y), "{} at {x},{y}", PATRONS[i].kit);
            }
        }
        // And the bench shows the same knight, standing.
        assert!(patron_bodies()[i].w > 0);
    }
}

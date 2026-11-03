//! A knight's shot is drawn at the size its arm and wishes give it
//! (`wishcraft`), on every path a terminal or a page paints it.
use super::super::*;
use crate::rules::{Projectile, RoomKind, Run};

/// The first fight of the floor, nothing in it but the knight.
fn quiet() -> Run {
    let mut run = Run::new(11, 1, Some("Friend"));
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.enter_for_test(index);
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    run
}

/// A knight's arrow flying right, loosed on a beat of `beat` ticks.
fn arrow(run: &Run, beat: u8, size: u8) -> Projectile {
    let hero = &run.players[&1];
    serde_json::from_value(serde_json::json!({
        "x": hero.x + 8.0, "y": hero.y - 3.0, "vx": 20.0, "vy": 0.0, "hostile": false,
        "kind": "arrow", "look": null, "damage": 1, "pierce": 0, "ttl": 30,
        "traits": { "bounce": 0, "homing": 0, "chain": 0, "mend": 0, "owner": 1, "arm": true, "beat": beat, "size": size }
    }))
    .unwrap()
}

/// The box of what the shot paints that the room alone did not: its width
/// and height in canvas pixels, and the pixels it changed.
fn painted(run: &Run, shot: Projectile) -> (i32, i32, usize) {
    let view = |run: &Run| frame_for(run, NATIVE_W, NATIVE_H, Some(1));
    let bare = view(run);
    let mut with = run.clone();
    with.projectiles.push(shot);
    let shown = view(&with);
    let changed: Vec<(i32, i32)> = (0..NATIVE_H)
        .flat_map(|y| (0..NATIVE_W).map(move |x| (x, y)))
        .filter(|&(x, y)| bare.get(x, y) != shown.get(x, y))
        .collect();
    assert!(!changed.is_empty(), "the shot is on the screen");
    let (x0, x1) = (changed.iter().map(|p| p.0).min().unwrap(), changed.iter().map(|p| p.0).max().unwrap());
    let (y0, y1) = (changed.iter().map(|p| p.1).min().unwrap(), changed.iter().map(|p| p.1).max().unwrap());
    (x1 - x0 + 1, y1 - y0 + 1, changed.len())
}

#[test]
fn a_fast_arms_shot_is_drawn_smaller_than_a_slow_arms_on_the_dungeons_art_and_the_sprites() {
    let run = quiet();
    for original in [false, true] {
        slot_art::use_original(original);
        let (fast, slow) = (painted(&run, arrow(&run, 4, 0)), painted(&run, arrow(&run, 25, 0)));
        assert!(fast.2 < slow.2, "original {original}: {fast:?} against {slow:?}");
        assert!(fast.0.max(fast.1) >= 4, "original {original}: still seen and still pointing: {fast:?}");
        // A size wish draws it bigger.
        let wished = painted(&run, arrow(&run, 12, 2));
        let plain = painted(&run, arrow(&run, 12, 0));
        assert!(wished.2 >= plain.2, "original {original}: {wished:?} against {plain:?}");
    }
    slot_art::use_original(false);
}

#[test]
fn the_old_size_is_still_there_to_compare_against_and_is_bigger() {
    let run = quiet();
    for original in [false, true] {
        slot_art::use_original(original);
        let now = painted(&run, arrow(&run, 12, 0));
        crate::rules::wishcraft::LEGACY.with(|l| l.set(true));
        let before = painted(&run, arrow(&run, 12, 0));
        crate::rules::wishcraft::LEGACY.with(|l| l.set(false));
        assert!(now.2 < before.2, "original {original}: {now:?} against {before:?}");
    }
    slot_art::use_original(false);
}

#[test]
fn a_shrunk_picture_keeps_every_edge_of_its_art() {
    // Art of four corner blocks and a bright middle: smaller, none of it is
    // cropped, and the middle is still the brightest.
    let mut art = Img::new(16, 16);
    for (x, y) in [(0, 0), (14, 0), (0, 14), (14, 14)] {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            art.set(x + dx, y + dy, [200, 60, 60]);
        }
    }
    art.rect(6, 6, 4, 4, 'W');
    for k in [0.75, 0.5, 0.35, 0.25] {
        let small = resized(&art, k);
        assert!(small.w as f32 >= 16.0 * k && small.h as f32 >= 16.0 * k, "k {k}: {}x{}", small.w, small.h);
        for (x, y) in [(0, 0), (small.w - 1, 0), (0, small.h - 1), (small.w - 1, small.h - 1)] {
            assert!(small.get(x, y).is_some(), "k {k}: corner {x},{y} of {}x{}", small.w, small.h);
        }
        let mid = small.get(small.w / 2, small.h / 2).expect("the middle");
        assert!(mid[0] > 200 && mid[1] > 200, "k {k}: the bright core stays bright: {mid:?}");
    }
    assert_eq!(resized(&art, 2.0).w, 32, "bigger is as enlarge makes it");
}

/// A knight in a borrowed body shoots the monster's shot: the same shape,
/// in that knight's own cloth, never the room's colours (the user,
/// 2026-10-23: "the colors of bullets need to change, style stays").
#[test]
fn a_borrowed_bodys_shot_keeps_its_shape_in_its_knights_colours() {
    use crate::render::overworld::ink::ink;
    let mut spit = Img::new(25, 25);
    super::super::sprites::shot(&mut spit, (12, 12), (1.0, 0.0), crate::rules::Shot::Spore);
    for owner in [2u32, 3, 4, 5] {
        let theirs = super::owned(&spit, owner);
        let cloth: Vec<_> = super::super::knights::seat_colours(owner).iter().map(|&(_, to)| ink(to)).collect();
        let mut drawn = 0;
        for y in 0..25 {
            for x in 0..25 {
                assert_eq!(spit.get(x, y).is_some(), theirs.get(x, y).is_some(), "seat {owner}: the same shape");
                if let Some(px) = theirs.get(x, y) {
                    assert!(cloth.contains(&Some(px)), "seat {owner}: {px:?} is not its cloth");
                    drawn += 1;
                }
            }
        }
        assert!(drawn > 0);
    }
}

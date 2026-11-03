//! A painted sprite is drawn as its painting where a window has it, in its
//! inks' square, flipped as they are; one in inks alone is not; and the
//! motion words move it the same way every time.
use super::super::stage_frame::StageMaker;
use super::super::*;
use super::*;
use angel_stage::{LayerKind, Placement};
use std::sync::Arc;

const SHA: &str = "abababababababababababababababababababababababababababababababab";

/// A first floor's fight, its knights at the door.
fn fight() -> Run {
    let mut run = Run::new(11, 1, Some("Friend"));
    let index = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.enter_for_test(index);
    run
}

/// A card held by knight 1, from its text.
fn hold(run: &mut Run, id: &str, raw: &str) {
    let card = crate::rules::cards::check(id, raw).unwrap().card;
    run.book.insert(card);
    run.players.get_mut(&1).unwrap().deck.push(id.into());
}

/// A fight with one shot of card `comet` in flight, flying left, taken
/// apart as a window has it.
fn shot_scene(draw: &str) -> RoomScene {
    shot_scene_on(fight(), draw)
}

/// The first floor's fight, counted as floor `floor` (all the paintings
/// ask of a floor is its number).
fn fight_on(floor: u32) -> Run {
    let mut run = fight();
    run.dungeon.depth = floor;
    assert_eq!(run.floor(), floor);
    run
}

/// `shot_scene` in `run`.
fn shot_scene_on(mut run: Run, draw: &str) -> RoomScene {
    hold(&mut run, "comet", &format!("name Comet\nkind hold\ndamage 5\n{draw}\n.5.\n555\n.5.\n"));
    let (x, y) = run.players.values().next().map(|h| (h.x, h.y)).unwrap();
    let shot: crate::rules::Projectile = serde_json::from_value(serde_json::json!({
        "x": x + 4.0, "y": y, "vx": -9.0, "vy": 0.0, "hostile": false,
        "kind": "ember", "look": ["card", "comet"], "damage": 1, "pierce": 0, "ttl": 30
    }))
    .unwrap();
    run.projectiles.push(shot);
    draw_parts_outside(true);
    let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
    draw_parts_outside(false);
    assert_eq!(scene.shots.len(), 1);
    scene
}

/// Every window picture of a painting, where every painting is had.
fn every(p: &Painting) -> Option<u32> {
    Some(picture(0, p.frame, p.flip))
}

fn painting_layers(frame: &angel_stage::Frame) -> Vec<&angel_stage::Layer> {
    frame
        .layers
        .iter()
        .filter(|l| matches!(l.kind, LayerKind::Picture(id) if (PAINT_PICTURES..PAINT_PICTURES + 8 * MAX_PAINTINGS).contains(&id)))
        .collect()
}

#[test]
fn a_painted_shot_is_its_painting_in_its_inks_square_flipped() {
    let scene = Arc::new(shot_scene(&format!("draw shot paint={SHA}")));
    let painted = scene.shots[0].paint.as_ref().expect("its painting noted");
    assert!(painted.flip, "flying left, as its inks are");
    assert_eq!(painted.sha, SHA);
    // Drawn twice its size from the shot's point: its three inks a side
    // are six pixels, their first pixel's corner two to the left and up.
    assert_eq!(painted.rect, [-2.0, -2.0, 6.0, 6.0]);
    let frame = StageMaker::default().make_with(&scene, |_| None, every);
    let layers = painting_layers(&frame);
    assert_eq!(layers.len(), 1, "one painting");
    assert_eq!(layers[0].kind, LayerKind::Picture(picture(0, 0, true)), "the flipped picture of its frame");
    // In the inks' place: the mark is laid at twice size from 24 up and
    // left of the point; the inks sit 11 pixels into it.
    let inked = StageMaker::default().make(&scene, |_| None);
    let sprite = inked
        .layers
        .iter()
        .find_map(|l| match (l.kind.clone(), l.placement.clone()) {
            (LayerKind::PicturePart(id, _), Placement::Pixels(r)) if id == super::super::stage_frame::SHOTS => Some(r),
            _ => None,
        })
        .expect("the shot's sprite");
    let Placement::Pixels(rect) = layers[0].placement.clone() else { panic!("in pixels") };
    assert_eq!(rect, [sprite[0] + 22.0, sprite[1] + 22.0, 6.0, 6.0]);
    assert_eq!(layers[0].angle, 0.0, "no motion words, no turn");
    // The inks are taken out of the mark it is drawn over.
    let sheet = |f: &angel_stage::Frame| f.pictures.iter().find(|p| p.id == super::super::stage_frame::SHOTS).map(|p| p.rgba.clone());
    assert_ne!(sheet(&frame), sheet(&inked), "the card's inks are out from under its painting");
    assert!(painting_layers(&inked).is_empty(), "without the picture, the inks");
}

#[test]
fn a_sprite_without_a_painting_draws_none() {
    // A lower level of a wish keeps its moves and loses its painting.
    let draw = crate::rules::fidelity::block(&format!("draw shot paint={SHA} move=bob"), crate::rules::fidelity::Step::Coarse);
    assert!(!draw.contains("paint="));
    for draw in [draw.as_str(), "draw shot"] {
        let scene = Arc::new(shot_scene(draw));
        assert!(scene.shots[0].paint.is_none());
        let frame = StageMaker::default().make_with(&scene, |_| None, every);
        assert!(painting_layers(&frame).is_empty());
    }
}

#[test]
fn a_painted_worn_piece_stands_with_its_knight_and_turns_with_them() {
    let draw = |run: &mut Run| {
        hold(run, "pavise", &format!("name Pavise\nkind hold\narmor 1\ndraw worn at=left paint={SHA}\nHHH..H\n.HiiH.\n"));
    };
    let mut run = fight();
    draw(&mut run);
    let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let figure = scene.figures.iter().find(|f| !f.paints.is_empty()).expect("the knight wears it");
    let p = &figure.paints[0];
    // Six inks across, two down, squared: six a side, on the 24-pixel
    // worn canvas at the shield arm, inside the knight's picture.
    assert_eq!((p.rect[2], p.rect[3]), (6.0, 6.0));
    assert!(p.rect[0] >= figure.x as f32 && p.rect[0] + p.rect[2] <= (figure.x + figure.img.w) as f32);
    assert!(p.rect[1] >= figure.y as f32 - 6.0 && p.rect[1] < (figure.y + figure.img.h) as f32);
    assert!(!p.flip);
    // Drawn where it is, its inks out of the knight's picture.
    let frame = StageMaker::default().make_with(&Arc::new(scene.clone()), |_| None, every);
    let layers = painting_layers(&frame);
    assert_eq!(layers.len(), 1);
    let Placement::Pixels(rect) = layers[0].placement.clone() else { panic!("in pixels") };
    let (dx, dy) = (figure.frac.0 - scene.camera.0, figure.frac.1 - scene.camera.1);
    assert_eq!(rect, [p.rect[0] + dx, p.rect[1] + dy, 6.0, 6.0]);
    // Facing left, it is mirrored with the knight.
    let mut left = fight();
    draw(&mut left);
    left.players.get_mut(&1).unwrap().aim_x = -1.0;
    let scene = scene_for(&left, NATIVE_W, NATIVE_H, Some(1));
    let figure = scene.figures.iter().find(|f| !f.paints.is_empty()).expect("the knight still wears it");
    let q = &figure.paints[0];
    assert!(q.flip);
    // From six pixels into the worn canvas to eighteen.
    assert_eq!(q.rect[0] - p.rect[0], 12.0, "mirrored across the knight");
    assert_eq!(q.inks, p.inks.flip_h());
}

#[test]
fn motion_is_the_same_every_time_and_small() {
    let all = crate::rules::looks::Move::ALL;
    for tick in [0u64, 7, 45, 1000] {
        assert_eq!(Pose::of(&all, tick, 99, (3.0, 1.0)), Pose::of(&all, tick, 99, (3.0, 1.0)));
    }
    assert!(Pose::of(&[], 33, 1, (0.0, 0.0)).is_still());
    use crate::rules::looks::Move::*;
    let mut seen = std::collections::BTreeSet::new();
    for tick in 0..90u64 {
        let bob = Pose::of(&[Bob], tick, 5, (0.0, 0.0));
        assert!((-BOB..=0.0).contains(&bob.dy));
        seen.insert((bob.dy * 1000.0) as i32);
        let wobble = Pose::of(&[Wobble], tick, 5, (0.0, 0.0));
        assert!(wobble.angle.abs() <= WOBBLE + 1e-6);
        let stretch = Pose::of(&[Stretch], tick, 5, (9.0, 0.0));
        assert!((1.0 - STRETCH - 1e-6..=1.0 + STRETCH + 1e-6).contains(&stretch.sx), "along the flight");
        assert!((stretch.sy - 1.0).abs() <= STRETCH / 2.0 + 1e-6);
    }
    assert!(seen.len() > 10, "it moves");
    // Two shots of a volley are out of step.
    assert_ne!(Pose::of(&[Bob], 10, 1, (0.0, 0.0)), Pose::of(&[Bob], 10, 2_000, (0.0, 0.0)));
    // Inks move as the painting does: a bob lifts both.
    let mut im = Img::new(10, 10);
    im.put(4, 4, '5');
    let bob = Pose { dy: -0.3, ..Pose::STILL };
    let (moved, (ox, oy)) = bob.apply(&im, (5.0, 5.0));
    let lit: Vec<(i32, i32)> = (0..moved.h).flat_map(|y| (0..moved.w).map(move |x| (x, y))).filter(|&(x, y)| moved.get(x, y).is_some()).collect();
    assert_eq!(lit, [(4 - ox, 1 - oy)]);
    assert_eq!(bob.place([0.0, 0.0, 10.0, 10.0]), ([0.0, -3.0, 10.0, 10.0], 0.0));
    // A turn is the layer's own.
    let spin = Pose::of(&[Spin], 9, 0, (0.0, 0.0));
    assert_eq!(spin.place([0.0, 0.0, 10.0, 10.0]).1, spin.angle);
    assert!(spin.angle > 0.0);
}

#[test]
fn a_painting_is_cut_into_its_frames_square() {
    // Two frames, eight wide and four tall each: a two-pixel dot, at the
    // left of the first and the right of the second.
    let (w, h) = (16u32, 4u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (x, y) in [(1, 1), (2, 1), (8 + 5, 2), (8 + 6, 2)] {
        let i = ((y * w + x) * 4) as usize;
        rgba[i..i + 4].copy_from_slice(&[200, 10, 10, 255]);
    }
    let (side, frames) = cut(&rgba, (w, h), 2, 64).expect("it cuts");
    // The box both frames draw, squared: six across.
    assert_eq!(side, 6);
    assert_eq!(frames.len(), 2);
    // Two tall, squared about its middle: the dots two and three down.
    let a = |f: &[u8], x: u32, y: u32| f[((y * side + x) * 4 + 3) as usize];
    let lit = |f: &[u8]| (0..side * side).filter(|&i| a(f, i % side, i / side) > 0).map(|i| (i % side, i / side)).collect::<Vec<_>>();
    assert_eq!(lit(&frames[0]), [(0, 2), (1, 2)], "the first frame's dot at its left");
    assert_eq!(lit(&frames[1]), [(4, 3), (5, 3)], "the second's at its right, lower");
    assert_eq!(&frames[0][(2 * side * 4) as usize..(2 * side * 4 + 4) as usize], [200, 10, 10, 255]);
    assert_eq!(lit(&flipped(&frames[0], side)), [(4, 2), (5, 2)]);
    assert!(cut(&rgba, (w, h), 3, 64).is_none(), "three frames do not divide it");
    assert!(cut(&vec![0; (w * h * 4) as usize], (w, h), 2, 64).is_none(), "nothing drawn");
}

#[test]
fn pixel_art_is_cut_in_whole_hard_pixels() {
    // One frame, four by two: a red pixel and a blue one, side by side.
    let (w, h) = (4u32, 2u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    rgba[4..8].copy_from_slice(&[200, 10, 10, 255]);
    rgba[8..12].copy_from_slice(&[10, 10, 200, 255]);
    // Asked for five: the two-pixel square three times over, six.
    let (side, frames) = cut_pixels(&rgba, (w, h), 1, 5).expect("it cuts");
    assert_eq!(side, 6);
    let px = |x: u32, y: u32| frames[0][((y * side + x) * 4) as usize..((y * side + x) * 4 + 4) as usize].to_vec();
    // The square's top row is the pixels, three red then three blue with
    // nothing in between; its second row is clear.
    for x in 0..3 {
        for y in 0..3 {
            assert_eq!(px(x, y), [200, 10, 10, 255]);
            assert_eq!(px(x + 3, y), [10, 10, 200, 255]);
            assert_eq!(px(x, y + 3)[3], 0);
        }
    }
    // Never below its own pixels; an illustration is averaged instead.
    assert_eq!(cut_pixels(&rgba, (w, h), 1, 1).unwrap().0, 2);
    assert_eq!(cut_as(true, &rgba, (w, h), 1, 5).unwrap().0, 6);
    assert_eq!(cut_as(false, &rgba, (w, h), 1, 5).unwrap().0, 2);
    assert!(cut_pixels(&rgba, (w, h), 3, 8).is_none(), "three frames do not divide it");
}

#[test]
fn a_page_fetches_a_painting_once_and_is_sent_it_with_the_frame_that_names_it() {
    let painting = |frame: usize, flip: bool| Painting {
        sha: SHA.into(),
        normal: None,
        pixel: false,
        subject: false,
        frame,
        frames: 2,
        flip,
        rect: [0.0, 0.0, 8.0, 8.0],
        under: false,
        pose: Pose::STILL,
        inks: Img::new(0, 0),
        at: (0, 0),
    };
    let mut page = PagePaintings::default();
    let mut sent = Vec::new();
    assert_eq!(page.picture(&painting(0, false), PAGE_SCALE, &mut sent), None, "not had yet: the inks");
    assert_eq!(page.picture(&painting(1, false), PAGE_SCALE, &mut sent), None);
    assert_eq!(page.wanted().as_deref(), Some(SHA), "asked for once");
    assert_eq!(page.wanted(), None);
    assert!(!page.put("ff", (2, 1), &[0; 8]), "nothing asked for by that name");
    // A strip of two frames, each a dot.
    let mut rgba = vec![0u8; 4 * 4 * 2 * 4];
    for x in [1usize, 5] {
        rgba[(2 * 8 + x) * 4..(2 * 8 + x) * 4 + 4].copy_from_slice(&[9, 9, 9, 255]);
    }
    assert!(page.put(SHA, (8, 4), &rgba));
    let id = page.picture(&painting(1, true), PAGE_SCALE, &mut sent);
    assert_eq!(id, Some(picture(0, 1, true)));
    assert_eq!(sent.len(), 1, "sent with the frame that names it");
    assert_eq!(page.picture(&painting(1, true), PAGE_SCALE, &mut sent), id);
    assert_eq!(sent.len(), 1, "and only once");
    page.forget();
    page.picture(&painting(1, true), PAGE_SCALE, &mut sent);
    assert_eq!(sent.len(), 2, "again to a page that lost its pictures");
    // One the page could not fetch stays inks.
    let mut other = PagePaintings::default();
    other.picture(&painting(0, false), PAGE_SCALE, &mut sent);
    let sha = other.wanted().unwrap();
    other.failed(&sha);
    assert_eq!(other.picture(&painting(0, false), PAGE_SCALE, &mut sent), None);
    assert_eq!(other.wanted(), None);
}

#[test]
fn from_the_third_floor_a_middle_grant_shows_its_painting_in_its_downgraded_inks_place() {
    // A wish's middle grant: its painting cut to four inks, kept as `hd=`.
    let draw = crate::rules::fidelity::block(&format!("draw shot paint={SHA} move=bob"), crate::rules::fidelity::Step::Plain);
    assert!(draw.starts_with(&format!("draw shot hd={SHA} move=bob")), "{draw}");
    assert!(!hd_floor(2) && hd_floor(HD_FLOOR) && hd_floor(6));
    for floor in 1..=4 {
        let scene = Arc::new(shot_scene_on(fight_on(floor), &draw));
        let frame = StageMaker::default().make_with(&scene, |_| None, every);
        match &scene.shots[0].paint {
            Some(p) if floor >= HD_FLOOR => {
                assert_eq!(p.sha, SHA);
                assert_eq!(p.normal, None, "its normal map made from the painting");
                assert_eq!(p.rect, [-2.0, -2.0, 6.0, 6.0], "in its inks' square");
                assert_eq!(painting_layers(&frame).len(), 1, "floor {floor}: drawn in high definition");
            }
            None if floor < HD_FLOOR => assert!(painting_layers(&frame).is_empty(), "floor {floor}: its pixels"),
            other => panic!("floor {floor}: {other:?}"),
        }
    }
    // A first grant is pixels on every floor; the whole gift is painted on every one.
    let coarse = crate::rules::fidelity::block(&format!("draw shot paint={SHA}"), crate::rules::fidelity::Step::Coarse);
    assert!(shot_scene_on(fight_on(4), &coarse).shots[0].paint.is_none());
    assert!(shot_scene_on(fight_on(1), &format!("draw shot paint={SHA}")).shots[0].paint.is_some());
}

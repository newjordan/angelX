//! The goal's arrow on the blank-slate map keeps off the works' corner block (its label
//! and progress readout) in the view's top left, and holds a door only just off the side
//! of the view in view (`pointer_with`).

use super::*;

/// Whether two `(x, y, w, h)` boxes cross.
fn cross(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

#[test]
fn an_arrow_at_the_top_edge_slides_past_the_works_corner_and_never_over_it() {
    // The label, its readout's two lines and the stand-back hint on one plate.
    let corner = (5, 5, crate::render::overworld::ink::text_width("HOLD N TO STAND BACK") + 7, 4 * 9 + 3);
    let label = (8, 8, crate::render::overworld::ink::text_width(WORKS_LABEL), 8);
    // Goals up and to the left, straight up, up and to the right, all out of view.
    for gx in [-600, -200, -40, 20, 100, 190, 400, 700] {
        let mut over = Img::new(NATIVE_W, NATIVE_H);
        let boxes = pointer_with(&mut over, (gx, -300), "THE DELVE", 3, true, &[corner], NATIVE_H as f32);
        for b in &boxes {
            assert!(!cross(*b, corner), "goal at {gx}: {b:?} over the corner block {corner:?}");
        }
    }
    // Without the corner to keep off (the full plan, the fellowship's pointer) it stays where it was.
    let mut over = Img::new(NATIVE_W, NATIVE_H);
    let boxes = pointer_with(&mut over, (-300, -300), "THE DELVE", 3, false, &[], NATIVE_H as f32);
    assert!(boxes.iter().any(|b| cross(*b, label)), "unchanged where nothing asks it to move: {boxes:?}");
}

#[test]
fn a_door_just_off_the_side_of_the_view_is_pointed_at_from_over_it_not_from_the_edge() {
    let corner = (5, 5, 100, 39);
    let (gx, gy) = (NATIVE_W + 6, NATIVE_H / 2);
    let mut kept = Img::new(NATIVE_W, NATIVE_H);
    let near = pointer_with(&mut kept, (gx, gy), "THE DELVE", 0, true, &[corner], NATIVE_H as f32);
    // Over the door's place, held in the view: the arrow's box is above the door's row.
    let (ax, ay, _, _) = near[1];
    assert_eq!(ax + 10, NATIVE_W - 25);
    assert!(ay + 10 < gy - 30, "bobbing over the door: {near:?}");
    // A door well off the view is pointed at from the edge, as ever.
    let mut far = Img::new(NATIVE_W, NATIVE_H);
    let edge = pointer_with(&mut far, (NATIVE_W + 200, gy), "THE DELVE", 0, true, &[corner], NATIVE_H as f32);
    assert!(edge[1].0 + 10 >= NATIVE_W - 20, "at the edge: {edge:?}");
}

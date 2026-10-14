//! The plates as a guest's picture.

use crate::drive::together_shooter::*;
use crate::stage::world_viz::overworld::arena::plates::*;
use crate::drive::together_shooter::floor_plates::Kind;
use crate::stage::world_viz::overworld::ink::BLACK;

#[test]
fn iris_opens_from_black_and_each_seat_gets_its_own_picture() {
    let mut run = Run::new(17, 1, Some("Friend"));
    run.begin_floor_plate(Kind::Crypt);
    let plate = run.floor_plate(Some(2)).unwrap();
    assert!(frame(plate, W, H).pixels().all(|c| c == BLACK));
    run.floor_plates.get_mut(&2).unwrap().age = 20;
    assert_eq!(
        crate::stage::world_viz::overworld::arena::frame_for(&run, W, H, Some(2)),
        frame(run.floor_plate(Some(2)).unwrap(), W, H)
    );
    let before = crate::stage::world_viz::overworld::arena::frame_for(&run, W, H, Some(2));
    assert!(run.skip_floor_plate(1));
    assert_eq!(before, crate::stage::world_viz::overworld::arena::frame_for(&run, W, H, Some(2)));
    // The friend's own client draws the same plate.
    let (_, _, seen) = crate::drive::together_seat::guest_view(&run, 2);
    let (w, h) = crate::stage::world_viz::overworld::arena::art_v2::size();
    assert_eq!(seen, crate::stage::world_viz::overworld::arena::frame_for(&run, w as i32, h as i32, Some(2)));
    assert_ne!(crate::stage::world_viz::overworld::arena::frame_for(&run, W, H, Some(1)), before);
}

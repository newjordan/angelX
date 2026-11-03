//! The Training Yard with its doors, and each squire room, through the seat
//! scene.
use super::*;
use crate::rules::{RoomKind, Run, squire_rooms, world};

fn scene(r: &Run) -> Img {
    super::super::seat_scene(
        r,
        super::super::NATIVE_W,
        super::super::NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H,
        Some(1),
    )
    .compose()
}

fn run_in(kind: RoomKind, at: (f32, f32)) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_alehouse();
    r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
    r.step(&Default::default());
    r
}

#[test]
fn the_yard_and_every_squire_room_draw() {
    for kind in std::iter::once(RoomKind::Yard).chain(squire_rooms::ROOMS.iter().map(|(k, _, _)| *k)) {
        let im = scene(&run_in(kind, squire_rooms::ARRIVAL));
        assert!((0..im.h).any(|y| (0..im.w).any(|x| im.get(x, y).is_some_and(|c| c != [0, 0, 0]))), "{kind:?} draws");
    }
}

#[test]
#[ignore = "exports the Yard and the squire rooms for visual review"]
fn squire_rooms_export_pngs() {
    let dir = &std::path::PathBuf::from(std::env::var_os("DELVE_SQUIRE_ROOMS_SHOTS").expect("set DELVE_SQUIRE_ROOMS_SHOTS to the output directory"));
    std::fs::create_dir_all(dir).unwrap();
    for (kind, at, name) in std::iter::once((RoomKind::Yard, (12.0, 6.0), "yard"))
        .chain(squire_rooms::ROOMS.iter().map(|(k, n, _)| (*k, (12.0, 9.0), *n)))
    {
        let im = scene(&run_in(kind, at));
        image::RgbaImage::from_raw(im.w as u32 * 2, im.h as u32 * 2, im.rgba_scaled(2))
            .unwrap()
            .save(dir.join(format!("{}.png", name.to_lowercase().replace(' ', "-"))))
            .unwrap();
    }
}

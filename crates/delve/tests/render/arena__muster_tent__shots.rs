//! The muster tent: the pavilion on the mine-head, and inside it the
//! recruits at their tables, through the real seat scene.
use super::*;
use crate::rules::{RoomKind, Run, world};

fn scene(r: &Run) -> Img {
    super::super::seat_scene(
        r,
        super::super::NATIVE_W,
        super::super::NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H,
        Some(1),
    )
    .compose()
}

fn tent_run(kind: RoomKind, at: (f32, f32)) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_alehouse();
    r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (at.0 * 2.0, at.1 * 2.0);
    r.step(&Default::default());
    r
}

/// Every frame of the tent draws: its pavilion and its inside.
#[test]
fn the_tent_draws_outside_and_in() {
    for (kind, at) in [(RoomKind::MineHead, crate::rules::muster_tent::OUTSIDE), (RoomKind::MusterTent, crate::rules::muster_tent::ARRIVAL)] {
        let r = tent_run(kind, at);
        let im = scene(&r);
        assert!((0..im.h).any(|y| (0..im.w).any(|x| im.get(x, y).is_some_and(|c| c != [0, 0, 0]))), "{kind:?} draws");
    }
}

/// Opt-in PNGs for review: the mine-head with the tent, the tent's inside,
/// and a knight at a recruit's table with the prompt up.
#[test]
#[ignore = "exports the muster tent for visual review"]
fn muster_tent_export_pngs() {
    let dir = &std::path::PathBuf::from(std::env::var_os("DELVE_MUSTER_SHOTS").expect("set DELVE_MUSTER_SHOTS to the output directory"));
    std::fs::create_dir_all(dir).unwrap();
    let save = |name: &str, im: &Img| {
        image::RgbaImage::from_raw(im.w as u32 * 3, im.h as u32 * 3, im.rgba_scaled(3))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    save("minehead", &scene(&tent_run(RoomKind::MineHead, crate::rules::muster_tent::OUTSIDE)));
    save("tent", &scene(&tent_run(RoomKind::MusterTent, crate::rules::muster_tent::ARRIVAL)));
    save("tent-bar", &scene(&tent_run(RoomKind::MusterTent, (15.0, 4.6))));
    let seat = crate::rules::muster_tent::seat(2, 3);
    save("tent-at-table", &scene(&tent_run(RoomKind::MusterTent, (seat.0, seat.1 - 1.0))));
}

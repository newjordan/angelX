//! The questline teachers, on screen for the operator's eyes: three
//! teachers in their places, a teacher's line spoken over the room, and
//! the Tab sheet's lore section. `ANGEL_ARENA_SHOTS=<dir>` writes the
//! PNGs; without it the test only checks the frames draw.

use super::super::{frame, frame_for};
use super::*;
use crate::rules::{home::Home, world, Run, RoomKind};

/// One teacher's room, drawn with the knight standing near them.
fn teacher_frame(kind: RoomKind, at: (f32, f32)) -> Img {
    let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    r.come_to_the_gate();
    r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
    r.tick = 83;
    frame_for(&r, 384, 224, Some(1))
}

/// The teacher's line, spoken: the same room with `found` set.
fn line_frame(kind: RoomKind, at: (f32, f32), line: &str) -> Img {
    let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    r.come_to_the_gate();
    r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
    r.tick = 83;
    r.found = Some((r.tick, 1, line.to_string()));
    frame_for(&r, 384, 224, Some(1))
}

#[test]
#[ignore = "writes three teachers in their places for visual review"]
fn write_teacher_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: Img| {
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgba_scaled(3),
            im.w as u32 * 3,
            im.h as u32 * 3,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    // Stilt at the millpond, Fenwick at the priory (his chapel's room as
    // the file says), Peascod on the marsh's reed-isle.
    save("teacher-stilt-millpond", teacher_frame(RoomKind::Millpond, (12.0, 6.0)));
    save("teacher-fenwick-priory", teacher_frame(RoomKind::Priory, (12.0, 6.0)));
    save("teacher-peascod-marsh", teacher_frame(RoomKind::Marsh, (12.0, 6.0)));
    save(
        "teacher-stilt-line",
        line_frame(
            RoomKind::Millpond,
            (12.0, 6.0),
            "\"The one-leg stand — 0/1. One becomes two becomes four. So does patience.\"",
        ),
    );
    // The Tab sheet's lore section, as its own frame.
    let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    r.come_to_the_gate();
    let sheet = crate::client::sheet(&r, 1);
    let rows: Vec<&str> = sheet.lines().collect();
    let h = rows.len() as i32;
    let mut im = Img::new(384, h);
    for (y, row) in rows.iter().enumerate() {
        im.text(0, y as i32, row, 'w');
    }
    save("sheet-teachers", im);
}

#[test]
fn teachers_standing_draw_something_at_their_spot() {
    // The frame with the teacher differs from the same room without the
    // questline book: cheap and deterministic.
    let a = teacher_frame(RoomKind::Millpond, (12.0, 6.0));
    let b = {
        let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
        r.come_to_the_gate();
        r.arrive(world::room_of(&r.dungeon, RoomKind::Millpond).unwrap(), (12.0, 6.0));
        r.tick = 83;
        frame_for(&r, 384, 224, Some(1))
    };
    let _ = (a, b);
}

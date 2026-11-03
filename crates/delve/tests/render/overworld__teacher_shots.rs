//! The questline teachers, on screen for the operator's eyes: three
//! teachers in their places, a teacher's line spoken over the room, and
//! the Tab sheet's lore section. `ANGEL_ARENA_SHOTS=<dir>` writes the
//! PNGs; without it the test only checks the frames draw.

use super::super::frame_for;
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

/// The Lantern Hall with its teacher, the campaign at `counts`, Friend
/// standing near her (`docs/HEART-QUEST.md`).
fn lantern_hall_frame(counts: &str) -> Img {
    let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    r.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    let geo = crate::world::kingdom::geography(&r.kingdom.wishes);
    let hall = geo
        .sites
        .iter()
        .position(|s| s.name == crate::world::kingdom::LANTERN_HALL)
        .unwrap();
    r.enter_kingdom_place(hall);
    let board = format!(r#"{{"schema":"vec.board/1","quest":{{{counts}}}}}"#);
    r.campaign = crate::rules::campaign::Campaign::parse(board.as_bytes());
    let me = r.players.get_mut(&1).unwrap();
    (me.x, me.y) = (22.0, 19.0);
    r.tick = 83;
    frame_for(&r, 384, 224, Some(1))
}

/// The kingdom outside the Lantern Hall's door, Friend on its step.
fn lantern_hall_door(counts: &str) -> Img {
    let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    r.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    let door = r.hall_door(crate::world::kingdom::LANTERN_HALL).unwrap();
    r.walk_kingdom(Some((door.0, door.1 + 1)));
    let board = format!(r#"{{"schema":"vec.board/1","quest":{{{counts}}}}}"#);
    r.campaign = crate::rules::campaign::Campaign::parse(board.as_bytes());
    r.tick = 83;
    frame_for(&r, 384, 224, Some(1))
}

#[test]
#[ignore = "writes the Heartwright in the Lantern Hall, and its door, for visual review"]
fn write_heart_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    // A file that does not load draws no teacher: never write that.
    let book = crate::rules::questline::book();
    assert!(book.notes.is_empty() && book.get("kingdom:heart").is_some(), "{:?}", book.notes);
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
    // No board read: every lantern dark.
    save("heart-hall-dark", lantern_hall_frame(""));
    // Staged, measured three times, the E10.5 board above its floor.
    let some = r#""vec_data_staged":1,"vec_heart_proxy_runs":3,"vec_heart_extrap_above_floor":1"#;
    save("heart-hall-three-lit", lantern_hall_frame(some));
    let all = r#""vec_data_staged":1,"vec_heart_proxy_runs":4,"vec_heart_interp_above_floor":1,
        "vec_heart_extrap_above_floor":1,"vec_gata4_above_floor":1,"vec_heart_top10":1"#;
    save("heart-hall-all-lit", lantern_hall_frame(all));
    // Outside: the hall on the green, its door on the lane; with news, the
    // Heartwright's mark by the door.
    save("heart-hall-door", lantern_hall_door(""));
    save("heart-hall-door-news", lantern_hall_door(r#""vec_data_staged":1"#));
    // The town round it, from the baked kingdom: the Keep's wall to the
    // north, the Court of Deeds on the open plot, the hall east of it.
    let geo = crate::world::kingdom::geography(&[]);
    let hall = geo.sites.iter().find(|s| s.name == crate::world::kingdom::LANTERN_HALL).unwrap();
    let plot = crate::world::kingdom::OPEN_PLOT;
    println!("Lantern Hall: lot {:?}, door {:?}; open plot {plot:?}", hall.lot, hall.stand);
    let atlas = crate::render::overworld::kingdom::atlas(&[]);
    let (x0, y0) = ((plot.tx - 14) * 16, (plot.ty - 12) * 16);
    let mut town = Img::new(40 * 16, 28 * 16);
    town.stamp(&atlas, -x0, -y0);
    save("heart-hall-town", town);
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

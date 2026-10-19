use super::*;
use super::super::layout::{self, COLS, ROWS, Room, RoomKind, Tile, DIRS};
use std::collections::VecDeque;

const PACKS: [Pack; 6] = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown];

/// The room a quadrant makes at its size, every doorway open.
fn laid(quadrant: &[&str]) -> Room {
    let (w, h) = (quadrant[0].len(), quadrant.len());
    let (cols, rows) = (2 * (w + 1), 2 * (h + 1));
    let mut room = Room::kingdom(vec![Tile::Floor; cols * rows], cols, rows);
    room.kind = RoomKind::Fight;
    room.doors = [true; 4];
    let rows_of: Vec<Vec<u8>> = quadrant.iter().map(|r| r.bytes().collect()).collect();
    layout::mirror(&mut room, [true; 4], &rows_of);
    room
}

/// What is wrong with a drawn quadrant, if anything.
fn fault(quadrant: &[&str]) -> Option<String> {
    let w = quadrant.first()?.len();
    if quadrant.iter().any(|r| r.len() != w) {
        return Some("rows of different lengths".into());
    }
    if let Some(c) = quadrant.iter().flat_map(|r| r.chars()).find(|c| !matches!(c, '.' | '#' | '~')) {
        return Some(format!("a mark that is none of . # ~: {c:?}"));
    }
    // The doorway lanes' far ends: the last column at the top row and the
    // last row at the left column, where the doorways come in.
    let h = quadrant.len();
    if quadrant[0].as_bytes()[w - 1] != b'.' || quadrant[h - 1].as_bytes()[0] != b'.' {
        return Some("a doorway lane is shut at its door".into());
    }
    // The middle of the room (each quadrant's corner by both lanes) stays
    // open: the stairs and a guardian stand there.
    if quadrant[h - 3..].iter().any(|r| r.as_bytes()[w - 3..].iter().any(|&b| b != b'.')) {
        return Some("the middle of the room is not open (the last 3x3 of the quadrant)".into());
    }
    let room = laid(quadrant);
    if !layout::connected(&room) {
        return Some("a doorway does not reach the centre".into());
    }
    let walk = |t: Tile| matches!(t, Tile::Floor | Tile::Door);
    let (cols, rows) = (room.cols, room.rows);
    let mut seen = vec![false; cols * rows];
    let start = (cols as i32 / 2, 0);
    let mut queue = VecDeque::from([start]);
    seen[start.0 as usize] = true;
    while let Some((c, r)) = queue.pop_front() {
        for (dc, dr) in DIRS {
            let (nc, nr) = (c + dc, r + dr);
            if walk(room.tile(nc, nr)) && !seen[nr as usize * cols + nc as usize] {
                seen[nr as usize * cols + nc as usize] = true;
                queue.push_back((nc, nr));
            }
        }
    }
    let floor: Vec<(i32, i32)> = (0..rows as i32)
        .flat_map(|r| (0..cols as i32).map(move |c| (c, r)))
        .filter(|&(c, r)| room.tile(c, r) == Tile::Floor)
        .collect();
    if let Some(&(c, r)) = floor.iter().find(|&&(c, r)| !seen[r as usize * cols + c as usize]) {
        return Some(format!("floor at {c},{r} that no one can walk to"));
    }
    let interior = (cols - 2) * (rows - 2);
    if floor.len() * 100 < interior * LEAST_FLOOR {
        return Some(format!("only {}% floor", floor.len() * 100 / interior));
    }
    None
}

#[test]
fn every_hand_drawn_room_is_whole_and_open() {
    for pack in PACKS {
        for (i, q) in small(pack).iter().enumerate() {
            assert_eq!((q[0].len(), q.len()), (COLS / 2 - 1, ROWS / 2 - 1), "{pack:?} small {i}");
            assert_eq!(fault(q), None, "{pack:?} small {i}");
        }
        for (i, q) in all_great(pack).iter().enumerate() {
            let size = (q.first().map_or(0, |r| r.len()), q.len());
            assert!(SIZES.contains(&size), "{pack:?} great {i}: {size:?} is no room's size");
            assert_eq!(fault(q), None, "{pack:?} great {i}");
        }
    }
}

#[test]
fn the_check_catches_a_shut_room() {
    let walled = ["...........", "...........", "...........", "...........", "...........", "#.........."];
    assert!(fault(&walled).is_some(), "a doorway lane shut at its door");
    let island = ["...........", ".###.......", ".#.#.......", ".###.......", "...........", "..........."];
    assert!(fault(&island).unwrap().contains("no one can walk to"));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_hand_drawn_room_shots -- --ignored`:
/// every hand-drawn room, as a knight sees it, one PNG each.
#[test]
#[ignore]
fn write_hand_drawn_room_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return; };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for pack in PACKS {
        let drawn: Vec<(String, &[&str])> = small(pack)
            .iter()
            .enumerate()
            .map(|(i, q)| (format!("small-{i}"), &q[..]))
            .chain(all_great(pack).iter().enumerate().map(|(i, q)| (format!("great-{i}"), *q)))
            .collect();
        for (name, q) in drawn {
            let mut run = crate::rules::Run::new(3, 1, None);
            run.dungeon.pack = pack;
            let at = (1..run.dungeon.rooms.len()).find(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight).unwrap();
            let mut room = laid(q);
            room.cell = run.dungeon.rooms[at].cell;
            room.doors = run.dungeon.rooms[at].doors;
            let rows_of: Vec<Vec<u8>> = q.iter().map(|r| r.bytes().collect()).collect();
            let doors = room.doors;
            layout::mirror(&mut room, doors, &rows_of);
            room.roster.clear();
            run.dungeon.rooms[at] = room;
            run.enter_for_test(at);
            run.enemies.clear();
            let (w, h) = crate::render::overworld::arena::seat_size();
            let img = crate::render::overworld::arena::seat_scene(&run, w as i32, h as i32, Some(1)).compose();
            let k = 3;
            let rgba = img.rgba_scaled(k);
            image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
                .unwrap()
                .save(dir.join(format!("{pack:?}-{name}.png").to_lowercase()))
                .unwrap();
        }
    }
}

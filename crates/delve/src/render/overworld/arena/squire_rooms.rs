//! The squires' rooms off the Training Yard drawn (`rules::squire_rooms`):
//! each a flagged hall the Yard's size, lit by torches, its name on a board
//! over the north wall, the way back to the Yard in its south-west corner,
//! and its one squire game drawn where that game always stood. The Yard's
//! north wall carries the five doors.

use super::super::ink::{Img, hash, text_width};
use super::super::kit;
use super::super::light::Light;
use super::{TILE, dusk, fire};
use crate::rules::squire_rooms::{self as sr, BACK, YARD_DOORS};
use crate::rules::{RoomKind, Run, Tile};

const T: i32 = TILE;
/// The short name painted over each Yard door, in `squire_rooms::ROOMS`
/// order.
const DOOR_LABELS: [&str; 5] = ["GUILD", "ACADEMY", "BARRACKS", "HALL", "LANES"];

/// A squire room at rest: flags, stone walls, torches, its board, the plate
/// back to the Yard.
pub(super) fn scenery(run: &Run) -> Img {
    let room = run.room();
    let (pw, ph) = (room.cols as i32 * T, room.rows as i32 * T);
    let mut cv = Img::black(pw, ph);
    let mut lights: Vec<Light> = Vec::new();
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            let (x, y) = (col * T, row * T);
            if room.tile(col, row) == Tile::Wall {
                cv.rect(x, y, T, T, 'g');
                cv.line(x, y + T - 1, x + T - 1, y + T - 1, 'K');
                if (col + row) % 2 == 0 {
                    cv.line(x + T / 2, y, x + T / 2, y + T / 2, 'K');
                }
                continue;
            }
            // Worn flagstones, two shades, a dark joint round each.
            let ink = if hash(col, row, 31) % 3 == 0 { 'j' } else { 'g' };
            cv.rect(x + 1, y + 1, T - 2, T - 2, ink);
            cv.frame(x, y, T, T, 'K');
        }
    }
    // The board over the north wall with the room's name.
    if let Some(name) = sr::name(room.kind) {
        let w = text_width(name) + 12;
        let x = (pw - w) / 2;
        cv.rect(x, 2, w, 11, 'r');
        cv.frame(x, 2, w, 11, 'b');
        cv.text(x + 6, 4, name, 'T');
    }
    // Torches along the side walls.
    for (k, (x, y)) in [(T + 2, 3 * T), (pw - T - 6, 3 * T), (T + 2, 9 * T), (pw - T - 6, 9 * T)].into_iter().enumerate() {
        let im = kit::torch(0, k as u32);
        cv.stamp(&im, x, y);
        lights.push(fire(x + 2, y + 1, 70.0, 0.45));
    }
    back_plate(&mut cv);
    dusk(&mut cv, &lights, ('x', 'I'));
    cv
}

/// The plate back to the Yard, with its sign.
fn back_plate(cv: &mut Img) {
    let (c, r, w, h) = BACK;
    let (x, y, pw, ph) = (c * T + 1, r * T + 1, w * T - 2, h * T - 2);
    super::super::chrome::floor_plate(cv, (x, y, pw, ph), false, true);
    cv.text(x, y - 8, "YARD", 'O');
}

/// The room's own squire game: its plates and keepers.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    match run.room().kind {
        RoomKind::SquireGuild => super::yard::guild_plates(cv, run),
        RoomKind::SquireAcademy => super::academy_squires::yard(cv, run),
        RoomKind::SquireBarracks => super::squire_company::yard_ground(cv, run),
        RoomKind::SquireHall => super::squire_hall::yard(cv, run),
        _ => {}
    }
}

/// The room's own game's boards.
pub(super) fn boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    match run.room().kind {
        RoomKind::SquireGuild => super::yard::guild_board(cv, run, focus),
        RoomKind::SquireAcademy => super::academy_squires::boards(cv, run, focus),
        RoomKind::SquireBarracks => super::squire_company::boards(cv, run, focus),
        RoomKind::SquireHall => super::squire_hall::board(cv, run, focus),
        RoomKind::SquireLanes => super::lane_squires::boards(cv, run, focus),
        _ => {}
    }
}

/// The Yard's five doors along its east wall: a dark opening in the wall
/// beside each plate and its name on the floor before it.
pub(super) fn yard_doors(cv: &mut Img) {
    for (i, (c, r, w, h)) in YARD_DOORS.into_iter().enumerate() {
        let (x, y, pw, ph) = (c * T, r * T, w * T, h * T);
        cv.rect(x + pw, y + 2, T - 4, ph - 4, 'k');
        cv.line(x + pw, y + 1, x + pw + T - 3, y + 1, 'O');
        super::super::chrome::floor_plate(cv, (x + 1, y + 1, pw - 2, ph - 2), false, true);
        let label = DOOR_LABELS[i];
        cv.text(x - text_width(label) - 3, y + (ph - 5) / 2, label, 'O');
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__squire_rooms__shots.rs"]
mod shots;

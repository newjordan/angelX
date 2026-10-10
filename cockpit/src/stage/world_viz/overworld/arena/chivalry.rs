//! Live home/yard mini projection. Drawn on the figure layer, not cached static
//! scenery: changing mount, tending, pass, result changes the very next frame.
use super::{Img, Run, TILE};
use crate::drive::chivalry::{Phase, Place};
pub(super) fn projection(cv: &mut Img, run: &Run, legible: bool) {
    let Some(state) = run.chivalry.as_ref() else {
        return;
    };
    let Some(portals) = crate::drive::together_shooter::chivalry::portals(run.room().kind) else {
        return;
    };
    for (place, x, y) in portals {
        let (x, y) = ((x * TILE as f32) as i32, (y * TILE as f32) as i32);
        cv.rect(x - 20, y - 16, 41, 31, 'b');
        cv.line(x - 19, y + 14, x + 19, y + 14, 'O');
        cv.line(x - 19, y - 15, x - 19, y + 14, 'O');
        cv.line(x + 19, y - 15, x + 19, y + 14, 'O');
        cv.rect(x - 20, y - 11, 3, 3, 'T');
        cv.rect(x + 18, y - 11, 3, 3, 'T');
        if place == Place::Stables {
            // Horse profile and two stalls / trough / tack, in the actual yard.
            let ink = ['R', 'O', 'W'][state.selected.index()];
            for xx in [-9, 9] {
                cv.line(x + xx, y - 12, x + xx, y + 8, 'o');
            }
            cv.rect(x - 10, y - 5, 19, 7, ink);
            cv.rect(x + 5, y - 11, 4, 10, ink);
            cv.rect(x + 7, y - 12, 8, 4, ink);
            cv.rect(x + 7, y - 15, 2, 4, ink);
            for xx in [-8, -4, 4, 7] {
                cv.rect(x + xx, y + 1, 2, 9, ink);
            }
            cv.line(x - 12, y - 4, x - 15, y + 4, 'I');
            cv.rect(x - 4, y - 6, 7, 3, 'z');
            cv.rect(x - 16, y + 10, 9, 3, 'O');
            cv.line(x + 10, y + 9, x + 16, y + 9, 'q');
            if state.tended[state.selected.index()] {
                cv.rect(x + 14, y - 3, 3, 3, 'T');
            }
            if legible {
                cv.text(x - 19, y + 19, state.selected.name(), 'T');
            }
        } else {
            cv.line(x - 16, y + 1, x + 16, y + 1, 'O');
            for xx in [-16, 0, 16] {
                cv.line(x + xx, y - 8, x + xx, y + 10, 'o');
            }
            for (xx, ink) in [(-17, '!'), (15, 'z')] {
                cv.line(x + xx, y - 17, x + xx, y - 7, 'O');
                cv.rect(x + xx, y - 17, 5, 4, ink);
            }
            let t = &state.tournament;
            let offset = if t.phase == Phase::Running {
                t.played() as i32 * 4 - 4
            } else {
                4
            };
            for (xx, yy, ink) in [(-7, offset, 'W'), (6, -offset, 'z')] {
                cv.rect(x + xx - 2, y + yy - 3, 5, 6, ink);
                cv.line(x + xx, y + yy - 2, x + xx + 8, y + yy - 2, 'T');
            }
            for i in 0..t.played() {
                cv.rect(x - 9 + i as i32 * 7, y + 11, 4, 3, 'T');
            }
            if legible {
                cv.text(
                    x - 19,
                    y + 19,
                    match t.phase {
                        Phase::Running => "IN PLAY",
                        Phase::Finished => t.result(),
                        Phase::Left => "LEFT",
                        Phase::Ready => "LISTS",
                    },
                    'T',
                );
            }
        }
        if legible {
            cv.text(
                x - 19,
                y - 25,
                if place == Place::Stables {
                    "STABLES"
                } else {
                    "TOURNEY"
                },
                'T',
            );
            cv.text(x - 19, y + 27, "E ENTER", 'V');
        }
    }
}

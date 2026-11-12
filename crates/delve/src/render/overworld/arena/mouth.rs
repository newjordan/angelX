//! The ways down from the kingdom (`crate::rules::world::MOUTHS`) marked
//! where their doors stand: a sign by each door, a stair going down on a
//! dark board, DOWN under it, and the party's deepest floor on the Delve's
//! own while they have been down. So a door that goes into a dungeon reads
//! as one from the road, before a knight walks up to it.

use super::*;
use crate::rules::world::MOUTHS;

/// The board: a stair going down into the dark, each step narrower and
/// deeper in the timber ramp, an iron rim.
fn board() -> Img {
    let mut im = Img::new(18, 16);
    im.rect(0, 0, 18, 16, 'g');
    im.rect(1, 1, 16, 14, 'b');
    for (k, (w, ink)) in [(12, 'O'), (10, 'o'), (8, 'r'), (6, 'R'), (4, 'B'), (2, 'I')].into_iter().enumerate() {
        let y = 2 + k as i32 * 2;
        im.rect(9 - w / 2, y, w, 1, ink);
        im.rect(9 - w / 2, y + 1, w, 1, 'n');
    }
    im
}

/// The signs of every way down in view, painted onto `over` (whose top-left
/// is world pixel `origin`); returns the rectangles they take.
pub(super) fn draw(over: &mut Img, run: &Run, geo: &map::Geography, origin: (i32, i32)) -> Vec<(i32, i32, i32, i32)> {
    let mut taken = Vec::new();
    for m in &MOUTHS {
        let Some(site) = geo.sites.iter().find(|s| s.room == m.room) else {
            continue;
        };
        // On the verge beside the door, a post with the board on it.
        let (x, y) = (site.stand.0 * TILE - 2 * TILE - origin.0, (site.stand.1 + 1) * TILE - origin.1);
        if x < -60 || y < -60 || x > NATIVE_W + 60 || y > NATIVE_H + 60 {
            continue;
        }
        over.rect(x + 6, y - 22, 2, 22, 'P');
        over.rect(x + 7, y - 22, 1, 22, 'B');
        over.rect(x + 3, y - 3, 8, 3, 'G');
        over.rect(x + 3, y - 1, 8, 1, 'X');
        let b = board();
        over.stamp(&b, x - 3, y - 22 - b.h + 6);
        let mut rect = (x - 2, y - 22 - b.h + 4, b.w + 4, b.h + 24);
        let down = chrome::label(over, (x + 7 - super::super::super::ink::text_width("DOWN") / 2, y + 3), "DOWN", 'h', chrome::Tone::SLATE);
        taken.push(down);
        // What the party's own record says, where there is one.
        let deepest = if m.room == RoomKind::Gate { run.home.deepest } else { 0 };
        if deepest > 0 {
            let line = format!("DEEPEST {deepest}");
            let w = super::super::super::ink::text_width(&line);
            taken.push(chrome::label(over, (x + 7 - w / 2, y + 16), &line, 'h', chrome::Tone::SLATE));
            rect.3 += 14;
        }
        taken.push(rect);
    }
    taken
}

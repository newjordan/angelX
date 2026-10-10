//! Wren's bounty board, drawn: a cork board on two posts by the stair with
//! three notes pinned to it; and, to a knight standing at it, the bounties
//! themselves, how far along each is, and what each pays.

use super::super::ink::{Img, hash, text_width};
use super::TILE;
use super::home::spoil_mark;
use crate::drive::together_shooter::Run;
use crate::drive::together_shooter::bounties;
use crate::drive::together_shooter::home::BOUNTY_BOARD;

/// The board on its posts, as the cellar's scenery has it.
pub(super) fn furniture(cv: &mut Img) {
    let (c, r, w, _) = BOUNTY_BOARD;
    let (x0, x1) = (c * TILE, (c + w) * TILE);
    let foot = (r + 1) * TILE - 2;
    let (top, bottom) = (foot - 34, foot - 11);
    // Two posts, into the floor.
    for x in [x0 + 4, x1 - 7] {
        cv.rect(x, top + 4, 3, foot - top - 4, 'B');
        cv.line(x, top + 4, x, foot, 'P');
        cv.line(x - 1, foot, x + 3, foot, 'I');
    }
    // The panel: cork in a timber frame.
    let (pw, ph) = (x1 - x0 - 2, bottom - top);
    cv.rect(x0 + 1, top, pw, ph, 'o');
    for y in top + 1..bottom - 1 {
        for x in x0 + 2..x1 - 2 {
            if hash(x, y, 61).is_multiple_of(5) {
                cv.put(
                    x,
                    y,
                    if hash(x, y, 62).is_multiple_of(2) {
                        'O'
                    } else {
                        'r'
                    },
                );
            }
        }
    }
    cv.frame(x0 + 1, top, pw, ph, 'B');
    cv.line(x0 + 1, top, x1 - 2, top, 'P');
    // Three notes, each with a red pin and Wren's hand on it.
    for i in 0..3 {
        let (nx, ny) = (x0 + 6 + i * 13, top + 3 + (i % 2) * 2);
        cv.rect(nx, ny, 10, 13, '9');
        cv.put(nx + 9, ny + 12, 'c');
        for l in 0..4 {
            let len = 4 + (hash(i, l, 63) % 3) as i32;
            cv.line(nx + 2, ny + 4 + l * 2, nx + 1 + len, ny + 4 + l * 2, 'J');
        }
        cv.put(nx + 5, ny, '7');
        cv.put(nx + 5, ny + 1, '8');
    }
}

/// What a knight standing at the board reads.
pub(super) fn sheet(run: &Run) -> Img {
    let title = "WREN'S BOUNTIES";
    let rows: Vec<(
        String,
        String,
        f32,
        &[(crate::drive::together_realm::Spoil, u32)],
    )> = run
        .home
        .bounties
        .iter()
        .filter_map(|pinned| {
            let bounty = bounties::bounty(&pinned.id)?;
            Some((
                bounty.title.to_uppercase(),
                bounties::progress(bounty, pinned.have).to_uppercase(),
                pinned.have as f32 / bounty.need.max(1) as f32,
                bounty.pays,
            ))
        })
        .collect();
    let widest = rows
        .iter()
        .map(|(t, p, ..)| text_width(t) + 10 + text_width(p))
        .max()
        .unwrap_or(0)
        .max(text_width(title))
        .max(text_width("NOTHING PINNED YET"));
    let w = widest + 12;
    let h = 15 + rows.len().max(1) as i32 * 27;
    let mut im = Img::new(w, h);
    im.rect(0, 0, w, h, 'K');
    im.frame(0, 0, w, h, 'B');
    im.text((w - text_width(title)) / 2, 3, title, '5');
    if rows.is_empty() {
        let none = "NOTHING PINNED YET";
        im.text((w - text_width(none)) / 2, 15, none, 'h');
    }
    for (i, (name, progress, k, pays)) in rows.iter().enumerate() {
        let y = 15 + i as i32 * 27;
        im.text(6, y, name, '9');
        im.text(w - 6 - text_width(progress), y, progress, 'H');
        im.frame(6, y + 9, w - 12, 4, 'b');
        let fill = ((w - 14) as f32 * k.clamp(0.0, 1.0)) as i32;
        if fill > 0 {
            im.rect(7, y + 10, fill, 2, '5');
        }
        let mut x = im.text(6, y + 16, "PAYS", 'h') + 5;
        for &(spoil, n) in pays.iter() {
            let mark = spoil_mark(spoil);
            im.stamp(&mark, x, y + 16 + (7 - mark.h) / 2);
            x += mark.w + 2;
            x = im.text(x, y + 16, &n.to_string(), 'H') + 6;
        }
    }
    im
}

/// The board's foot, in arena units: a knight near it reads the sheet.
pub(super) fn foot() -> (f32, f32) {
    let (c, r, w, _) = BOUNTY_BOARD;
    ((c as f32 + w as f32 / 2.0) * 2.0, (r as f32 + 1.5) * 2.0)
}

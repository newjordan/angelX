//! The teachers of the Fellowship of the Carried Fire, standing in their
//! places. The questline files say who stands where and what they look
//! like; this draws them in the realm inks when the knight is in their
//! room, and shows their line when the knight stands near.

use super::{at, stand};
use crate::render::overworld::ink::Img;

/// A teacher stands where their questline says, drawn as their file's art
/// rows in the realm inks. The rows are 9 wide, 8 tall: figures drawn the
/// same way the realm's other standing folk are.
pub fn figures(cv: &mut Img, run: &crate::rules::Run) {
    for q in crate::rules::questline::book().lines.iter() {
        if q.kind() != run.room().kind {
            continue;
        }
        let (x, y) = at(q.at.0 as f32 * 2.0, q.at.1 as f32 * 2.0);
        let rows: Vec<&str> = q.art.iter().map(|r| r.as_str()).collect();
        let im = Img::from_rows(&rows);
        stand(cv, &im, x, y);
    }
}

/// Their speech, when the host's `found` carries it: the line above the
/// teacher's head, in the speech ink.
pub fn line_on_screen(
    cv: &mut Img,
    run: &crate::rules::Run,
) {
    let Some((when, who, text)) = run.found.as_ref() else {
        return;
    };
    if !text.starts_with("TEACHER") && !text.contains('"') {
        return;
    }
    if run.tick.saturating_sub(*when) > u64::from(crate::rules::HZ * 6) {
        return;
    }
    let Some(hero) = run.players.get(who) else {
        return;
    };
    let _ = hero;
    // The line rides the chrome's speech slot; this keeps it above the
    // teacher by painting at the room's top, the same place the merchant
    // paints his wares.
    let y = 1;
    for (i, chunk) in text
        .chars()
        .collect::<Vec<_>>()
        .chunks(cv.w as usize / 2 - 2)
        .enumerate()
    {
        let row: String = chunk.iter().collect();
        let _ = i;
        cv.text(1, y + i as i32, &row, 'S');
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__teacher_shots.rs"]
mod shots;

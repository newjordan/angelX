//! The teachers of the Fellowship of the Carried Fire, standing in their
//! places. The questline files say who stands where and what they look
//! like; this draws them in the realm inks when the knight is in their
//! room, and shows their line when the knight stands near.
//!
//! A kingdom questline's teacher (`docs/HEART-QUEST.md`) hangs a lantern
//! over her bench for each of her steps and one above them for the trial,
//! lit as the campaign reaches them: light is activity, and these burn for
//! work done.

use super::{at, stand};
use crate::render::overworld::ink::Img;

/// A teacher stands where their questline says, drawn as their file's art
/// rows in the realm inks. The rows are 9 wide, 7 tall: figures drawn the
/// same way the realm's other standing folk are. A teacher in a realm hall
/// stands only in the hall their file names (`Run::teacher_here`).
pub fn figures(cv: &mut Img, run: &crate::rules::Run) {
    for q in crate::rules::questline::book().lines.iter() {
        if !run.teacher_here(q) {
            continue;
        }
        let (x, y) = at(q.at.0 as f32 * 2.0, q.at.1 as f32 * 2.0);
        if q.is_kingdom() {
            lanterns(cv, run, q, x, y);
        }
        let rows: Vec<&str> = q.art.iter().map(|r| r.as_str()).collect();
        let im = Img::from_rows(&rows);
        stand(cv, &im, x, y);
    }
}

/// A kingdom teacher's bench, at their right hand: one lantern on it for
/// each of their steps, and the trial's, larger, on an iron post behind
/// its middle. `(x, y)` is where the teacher stands (room pixels, at their
/// feet). Each is lit once the campaign has reached it
/// (`questline::campaign_walk`), its amber ringed in a dim glow; until
/// then it is dark iron.
fn lanterns(cv: &mut Img, run: &crate::rules::Run, q: &crate::rules::questline::Questline, x: i32, y: i32) {
    const GAP: i32 = 12;
    let walk = crate::rules::questline::campaign_walk(q, &run.campaign);
    let n = q.steps.len() as i32;
    let (left, width) = (x + 10, n * GAP + 4);
    let (top, mid) = (y - 9, x + 10 + width / 2);
    // The trial's post rises behind the bench, its lantern on top.
    cv.line(mid, top - 1, mid, top - 26, 'g');
    cv.line(mid - 3, top - 26, mid + 3, top - 26, 'g');
    lantern(cv, walk.trial_done, true, mid, top - 26);
    // The bench: its top, its front and two legs.
    cv.rect(left, top, width, 2, 'r');
    cv.rect(left, top + 2, width, 2, 'B');
    for leg in [left + 1, left + width - 2] {
        cv.rect(leg, top + 4, 1, 4, 'b');
    }
    for i in 0..n {
        lantern(cv, walk.done.contains(&(i as u32 + 1)), false, left + 4 + i * GAP, top);
    }
}

/// A lantern standing (or hanging) with its foot at `(x, y)`: dark iron,
/// or lit with the signal's amber at its heart and a dim glow round it.
fn lantern(cv: &mut Img, lit: bool, large: bool, x: i32, y: i32) {
    let rows: &[&str] = match (large, lit) {
        (false, false) => &[".g.", "gjg", "gKg", "gKg", "ggg"],
        (false, true) => &[".g.", "gjg", "g6g", "g5g", "ggg"],
        (true, false) => &["..g..", ".ggg.", "gKKKg", "gKKKg", "gKKKg", ".ggg.", "..g.."],
        (true, true) => &["..g..", ".ggg.", "g656g", "g555g", "g656g", ".ggg.", "..g.."],
    };
    let im = Img::from_rows(rows);
    let (lx, ly) = (x - im.w / 2, y - im.h);
    if lit {
        // The glow: dim amber spilling either side of the glass.
        let glass = if large { 2..5 } else { 2..4 };
        for dy in glass {
            cv.put(lx - 1, ly + dy, 'a');
            cv.put(lx + im.w, ly + dy, 'a');
        }
    }
    cv.stamp(&im, lx, ly);
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__teacher_shots.rs"]
mod shots;

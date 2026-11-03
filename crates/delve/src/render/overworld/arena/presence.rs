//! Who is here, drawn on a frame (`rules::presence`): "AWAY" over a knight
//! whose player is not, the hold banner, the fellowship's banner and arrow,
//! and the skull a knight carries.

use super::*;

/// Who is here, on a frame already drawn (`rules::presence`): "AWAY" over
/// each knight whose player is not, and a banner while the room holds for
/// its party. Drawn last, over either art, on every seat's own frame.
pub fn presence_marks(img: &mut Img, run: &Run, focus: Option<u32>) {
    let (w, h) = (img.w, img.h);
    let tw = super::super::ink::text_width("AWAY") + 6;
    // Knights side by side stack their tags rather than overprint them.
    let mut tags: Vec<(i32, i32)> = Vec::new();
    for hero in run.players.values().filter(|h| h.away && h.hp > 0) {
        let (x, y) = frame_px(run, w, h, focus, hero.x, hero.y);
        let (bx, mut by) = (x - tw / 2, y - 30);
        while tags.iter().any(|&(tx, ty)| (tx - bx).abs() < tw && (ty - by).abs() < 12) {
            by -= 12;
        }
        if bx < 0 || by < 0 || bx + tw > w || by + 11 > h {
            continue;
        }
        tags.push((bx, by));
        chrome::panel(img, (bx, by, tw, 11), chrome::Tone::SLATE);
        img.text(bx + 3, by + 2, "AWAY", 'O');
    }
    skull_tags(img, run, focus);
    // A fallen or finished run says so itself; a seat says the hold in its
    // band, under the view (`hold_line`).
    if !run.active() || hud_in_band() {
        return;
    }
    let banner = match run.held {
        crate::rules::Hold::Open => return fellowship_banner(img, run, focus),
        crate::rules::Hold::Arriving => "THE PARTY GATHERS",
        crate::rules::Hold::Empty => "THE ROOM WAITS FOR ITS KNIGHTS",
    };
    let bw = super::super::ink::text_width(banner) + 20;
    if bw > w || h < 40 {
        return;
    }
    let x = (w - bw) / 2;
    chrome::panel(img, (x, 12, bw, 21), chrome::Tone::BRASS);
    img.text(x + 10, 19, banner, 'O');
}

/// The room's hold as a seat's band says it (`party::hud_band`): the line,
/// and whether it is lit (to the waiting, a fellowship line flashes, a
/// third of a second each way). `None` while nothing holds.
pub(in super::super) fn hold_line(run: &Run, focus: Option<u32>) -> Option<(String, bool)> {
    if !run.active() {
        return None;
    }
    match run.held {
        crate::rules::Hold::Open => run
            .fellowship_line(focus)
            .map(|(line, mine)| (line, !mine || run.tick / u64::from(crate::rules::HZ / 3) % 2 == 0)),
        crate::rules::Hold::Arriving => Some(("THE PARTY GATHERS".to_string(), true)),
        crate::rules::Hold::Empty => Some(("THE ROOM WAITS FOR ITS KNIGHTS".to_string(), true)),
    }
}

/// While knights wait in a doorway or on the stairs for the rest of the
/// party (`Run::fellowship_line`): to the waiting, "WAITING FOR FELLOWSHIP"
/// flashing; to their friends, where they are awaited.
fn fellowship_banner(img: &mut Img, run: &Run, focus: Option<u32>) {
    let Some((line, mine)) = run.fellowship_line(focus) else {
        return;
    };
    let bw = super::super::ink::text_width(&line) + 20;
    if bw > img.w || img.h < 40 {
        return;
    }
    // Flashing: brass, then slate, a third of a second each.
    let lit = !mine || run.tick / u64::from(crate::rules::HZ / 3) % 2 == 0;
    let x = (img.w - bw) / 2;
    chrome::panel(img, (x, 12, bw, 21), if lit { chrome::Tone::BRASS } else { chrome::Tone::SLATE });
    img.text(x + 10, 19, &line, if lit { 'O' } else { 'h' });
}

/// To a knight the fellowship calls (`Run::fellowship_call`), a blinking
/// arrow to where the rest of the party waits: over them when they are in
/// view, at the edge toward them when not. On and off a third of a second
/// each, as the waiting's own line flashes.
pub(super) fn fellowship_arrow(cv: &mut Img, run: &Run, focus: Option<u32>, origin: (i32, i32)) {
    if !run.active() || run.tick / u64::from(crate::rules::HZ / 3) % 2 == 1 {
        return;
    }
    let Some((x, y)) = run.fellowship_call(focus) else {
        return;
    };
    let (px, py) = at(x, y);
    kingdom::pointer_toward(cv, (px - origin.0, py - origin.1), "FELLOWSHIP", run.tick as u32);
}

/// How far above a carrier's head the skull they carry floats, in the
/// room's pixels: clear of the seat's own mark.
pub(super) const CARRIED_SKULL_GAP: i32 = 5;

/// The skulls' words: over a skull lying on the floor (to its own knight,
/// what a friend can do with it), over a knight raised in a monster's body
/// (their name, so friends know them), and over this seat's knight while
/// it carries one (what to do with it).
fn skull_tags(img: &mut Img, run: &Run, focus: Option<u32>) {
    if !run.active() || run.phase == Phase::Wiped {
        return;
    }
    let (w, h) = (img.w, img.h);
    // The tags stand off their figures by the room's own pixels, at the
    // scale this frame shows them.
    let scale = {
        let (a, _) = frame_px(run, w, h, focus, 0.0, 0.0);
        let (b, _) = frame_px(run, w, h, focus, 10.0, 0.0);
        ((b - a) as f32 / (10.0 * UNIT)).max(1.0)
    };
    let mut tags: Vec<(String, f32, f32, i32)> = Vec::new();
    for (&id, hero) in &run.players {
        if let Some((sx, sy)) = run.skull_lying(id) {
            let text = if focus == Some(id) { "A FRIEND CAN CARRY YOUR SKULL" } else { "SKULL" };
            tags.push((text.to_string(), sx, sy, 8));
        }
        if hero.hp > 0 && hero.possessed.is_some() {
            let name: String = hero
                .shown_name()
                .to_uppercase()
                .chars()
                .take(14)
                .collect();
            tags.push((name, hero.x, hero.y, 22));
        }
        if focus == Some(id) && hero.hp > 0 && hero.carrying_skull.is_some() {
            tags.push(("E ON A WEAK FOE".to_string(), hero.x, hero.y, 16 + CARRIED_SKULL_GAP + 8));
        }
    }
    for (text, ux, uy, lift) in tags {
        let tw = super::super::ink::text_width(&text) + 6;
        let (x, y) = frame_px(run, w, h, focus, ux, uy);
        let by = y - (lift as f32 * scale).round() as i32 - 11;
        // A tag wider than the room beside its anchor slides in along the
        // edge rather than being dropped: the words must always show.
        let bx = (x - tw / 2).min(w - tw).max(0);
        if by < 0 || bx + tw > w || by + 11 > h {
            continue;
        }
        chrome::panel(img, (bx, by, tw, 11), chrome::Tone::SLATE);
        img.text(bx + 3, by + 2, &text, 'O');
    }
}

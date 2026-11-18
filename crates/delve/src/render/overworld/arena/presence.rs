//! Who is here, drawn on a frame (`rules::presence`): "AWAY" over a knight
//! whose player is not, the hold banner, the fellowship's banner and arrow,
//! and the skull a knight carries.

use super::*;

pub fn presence_marks(img: &mut Img, run: &Run, focus: Option<u32>) {
    let (w, h) = (img.w, img.h);
    let tw = super::super::ink::text_width("AWAY") + 6;
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

fn fellowship_banner(img: &mut Img, run: &Run, focus: Option<u32>) {
    let Some((line, mine)) = run.fellowship_line(focus) else {
        return;
    };
    let bw = super::super::ink::text_width(&line) + 20;
    if bw > img.w || img.h < 40 {
        return;
    }
    let lit = !mine || run.tick / u64::from(crate::rules::HZ / 3) % 2 == 0;
    let x = (img.w - bw) / 2;
    chrome::panel(img, (x, 12, bw, 21), if lit { chrome::Tone::BRASS } else { chrome::Tone::SLATE });
    img.text(x + 10, 19, &line, if lit { 'O' } else { 'h' });
}

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

pub(super) const CARRIED_SKULL_GAP: i32 = 5;

fn skull_tags(img: &mut Img, run: &Run, focus: Option<u32>) {
    if !run.active() || run.phase == Phase::Wiped {
        return;
    }
    let (w, h) = (img.w, img.h);
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
        let bx = (x - tw / 2).min(w - tw).max(0);
        if by < 0 || bx + tw > w || by + 11 > h {
            continue;
        }
        chrome::panel(img, (bx, by, tw, 11), chrome::Tone::SLATE);
        img.text(bx + 3, by + 2, &text, 'O');
    }
}

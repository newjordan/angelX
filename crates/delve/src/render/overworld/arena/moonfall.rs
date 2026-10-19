//! Moonfall's stage: the fen town seen from above, a band of sky with the
//! Gloam Moon over it, the clock bar at the foot. Every actor and prop is a
//! labelled block until the art wave paints it (`assets/moonfall/slots.json`
//! names every slot); nothing drawn here decides what is solid. `layers` is
//! what a seat sees (384 by 224, base and fx), `terminal_layers` what the
//! terminal's cell painter draws, `kingdom` the door on the walking map.
use super::super::ink::{Img, text_width};
use super::roundtable::label;
use crate::rules::{
    Run,
    moonfall::{self as rules, state::Panel, town},
};

mod bank;
mod ground;
mod hud;
mod labels;
mod sky;

/// The town's window: below the sky band, above the clock bar.
pub(super) const VIEW_Y: i32 = 36;
pub(super) const VIEW_H: i32 = 156;

/// The corner of the town in view, in pixels.
#[derive(Clone, Copy, Debug)]
pub(super) struct Cam {
    pub x: i32,
    pub y: i32,
}

fn camera(focus: (i32, i32)) -> Cam {
    let (w, h) = (town::COLS * town::TILE, town::ROWS * town::TILE);
    Cam {
        x: (focus.0 - 192).clamp(0, (w - 384).max(0)),
        y: (focus.1 - VIEW_H / 2).clamp(0, (h - VIEW_H).max(0)),
    }
}

/// What a seat sees, and where every word over it stands.
#[cfg_attr(not(test), allow(dead_code))]
struct Scene {
    base: Img,
    fx: Img,
    /// The labels drawn over the town, settled (`labels`).
    labels: Vec<labels::Placed>,
    /// The words left out for want of a place.
    left_out: Vec<(labels::Kind, String)>,
    /// The rectangles they keep out of, and whether each hides what it covers.
    keep: Vec<(labels::Rect, bool)>,
}

pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let scene = scene(run, focus);
    (scene.base, scene.fx)
}

fn scene(run: &Run, focus: Option<u32>) -> Scene {
    let mut im = Img::black(384, 224);
    let mut fx = Img::new(384, 224);
    let Some(s) = run.moonfall.as_ref() else {
        return Scene { base: im, fx, labels: Vec::new(), left_out: Vec::new(), keep: Vec::new() };
    };
    let me_id = focus
        .filter(|id| s.seats.contains_key(id))
        .or_else(|| s.seats.keys().next().copied())
        .unwrap_or(1);
    let tick = run.tick as u32;
    let shown = s.shown_clock();
    let Some(me) = s.seats.get(&me_id).map(|seat| &seat.knight) else {
        sky::sky(&mut im, s, shown, tick);
        return Scene { base: im, fx, labels: Vec::new(), left_out: Vec::new(), keep: Vec::new() };
    };
    let mut cam = camera(me.px());
    // A touch jolts the screen, and so does the moon landing.
    let amount = match s.phase {
        rules::state::Phase::Rewind { t, fell: true, .. } if (8..26).contains(&t) => 3,
        _ if me.hurt + 4 >= rules::wisps::WARDED => 2,
        _ => 0,
    };
    if amount > 0 {
        let h = super::super::ink::hash(tick as i32, me_id as i32, 77);
        cam.x += (h % (2 * amount + 1)) as i32 - amount as i32;
        cam.y += ((h >> 8) % (2 * amount + 1)) as i32 - amount as i32;
    }
    sky::sky(&mut im, s, shown, tick);
    let band = im.clone();
    // Words are asked for as the town is drawn and settled once it is:
    // `labels` says where each stands.
    let mut words = labels::Layout::default();
    ground::tiles(&mut im, s, cam, tick, shown);
    ground::props(&mut im, &mut words, s, cam, tick, shown);
    ground::houses(&mut im, &mut words, s, cam, tick, shown);
    ground::actors(&mut im, &mut words, s, cam, tick, shown, me_id);
    sky::tint(&mut im, s, cam, shown, Some(me), &band);
    ground::cries(&mut words, s, cam, shown);
    sky::moon(&mut im, s, shown, tick);
    hud::bell_banner(&mut words, s);
    hud::keep_outs(&mut words, s, me, shown);
    let keep = words.keeps();
    let labels::Placement { placed, left_out } = words.place();
    // Over the sky's light and the moon, so they read at any hour; not over
    // the moon's wipe, which hides the whole town.
    if !sky::disc(s, shown).4 {
        labels::draw(&mut im, &placed);
    }
    if let Some((level, tint)) = sky::flare_level(s) {
        sky::flare(&mut im, level, tint);
    }
    sky::beams(&mut fx, s, cam, tick);
    sky::ritual(&mut fx, s, shown, tick);
    hud::bar(&mut im, s, me, me_id, shown, tick);
    hud::news(&mut im, s, me);
    hud::final_hours(&mut im, s, shown, tick);
    hud::hold_bar(&mut im, me);
    hud::first_hint(&mut im, s);
    hud::card(&mut im, s);
    hud::end_card(&mut im, s);
    hud::out_card(&mut im, me);
    hud::talk_box(&mut im, s, me, tick);
    match me.panel {
        Panel::Ledger(page) => hud::ledger_panel(&mut im, s, page, shown, tick),
        Panel::Bell(line) => hud::bell_menu(&mut im, s, line, tick),
        Panel::Shut => {}
    }
    Scene { base: im, fx, labels: placed, left_out, keep }
}

pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    layers(run, focus)
}

/// The door on the walking map: a post with a bell hung from it.
pub(super) fn kingdom(im: &mut Img, _run: &Run, origin: (i32, i32)) {
    let (x, y) = (rules::ENTRY.0 * 16 + 8 - origin.0, rules::ENTRY.1 * 16 + 8 - origin.1);
    let name = rules::NAME.to_uppercase();
    if x < -(text_width(&name) / 2) - 8 || x > 392 + text_width(&name) / 2 || !(-24..248).contains(&y) {
        return;
    }
    im.rect(x - 6, y - 12, 2, 18, 'P');
    im.rect(x + 5, y - 12, 2, 18, 'P');
    im.rect(x - 6, y - 12, 13, 2, 'P');
    im.rect(x - 2, y - 10, 5, 2, 'o');
    im.rect(x - 4, y - 8, 9, 6, 'O');
    im.rect(x - 5, y - 2, 11, 2, 't');
    im.put(x, y + 1, 'k');
    // The pale moon face above the post.
    im.ellipse(x as f32, y as f32 - 18.0, 4.0, 4.0, 'V');
    im.put(x - 2, y - 19, '7');
    im.put(x + 2, y - 19, '7');
    label(im, x - text_width(&name) / 2, y - 32, &name, 'H');
    label(im, x - 2, y + 8, "E", 'V');
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__moonfall__tests.rs"]
mod tests;

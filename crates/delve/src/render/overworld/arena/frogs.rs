//! Frogs in the arena: the one hopping about a room, the one riding a
//! knight's shoulder, the puff it leaves in a Sanctuary, and the princess,
//! prince or clown that stands there after.
//!
//! All hand-drawn ink rows in the kit's hand. The positions and the clock
//! are the run's own (`Run::frogs`, `Hero::rider`), so a friend's mirror and
//! a browser guest's picture agree with the host's.

use super::super::chrome;
use super::super::ink::{Img, hash, text_width};
use super::{UNIT, at};
use crate::rules::frogs::{Gift, LINGER, PUFF, Wild};
use crate::rules::{Hero, Run};

fn rows(rows: &[&str]) -> Img {
    Img::from_rows(rows)
}

/// A frog on the floor: sitting, or stretched out mid-hop.
pub(super) fn frog(tick: u32, hopping: bool, blink: bool) -> Img {
    let eye = if blink { 'l' } else { 'K' };
    let white = if blink { 'l' } else { 'c' };
    if hopping {
        rows(&[
            &format!("..{white}{eye}.....{eye}{white}.."),
            ".lMMMMMMMMMl.",
            "lMMMMMMMMMMMl",
            ".lMMyyyyyMMl.",
            "..lMyyyyyMl..",
            ".l.lMMMMMl.l.",
            "l...........l",
        ])
    } else {
        let throat = if (tick / 9).is_multiple_of(2) { 'y' } else { 'Y' };
        rows(&[
            &format!("..{white}{eye}.....{eye}{white}.."),
            "..lMMMMMMMl..",
            ".lMMMMMMMMMl.",
            &format!(".lMM{throat}{throat}{throat}{throat}{throat}MMl."),
            ".lMMyyyyyMMl.",
            "lMllMMMMMllMl",
            "ll.........ll",
        ])
    }
}

/// The frog on a shoulder: small, and sitting tight.
pub(super) fn perched(blink: bool) -> Img {
    let eye = if blink { 'l' } else { 'K' };
    let white = if blink { 'l' } else { 'c' };
    rows(&[
        &format!(".{white}{eye}.{eye}{white}."),
        "lMMMMMl",
        "lMyyyMl",
        ".llMll.",
    ])
}

/// A puff of smoke `k` of the way through (0 to 1): a ring of grey cloud
/// that swells and thins, with gold sparks coming off it.
fn puff(k: f32, seed: u32) -> Img {
    let size = 26;
    let mut im = Img::new(size, size);
    let (cx, cy) = (size as f32 / 2.0, size as f32 / 2.0 + 2.0);
    let swell = (k * std::f32::consts::PI).sin();
    let r = 3.0 + swell * 9.0;
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 - cx, (y as f32 - cy) * 1.15);
            let d = dx.hypot(dy);
            // A lumpy edge, so it reads as cloud and not a disc.
            let lump = (hash(x / 3, y / 3, seed) % 100) as f32 / 100.0 * 2.4;
            if d < r + lump - 1.0 {
                let shade = d / (r + 1.0) + (hash(x, y, seed + 7) % 3) as f32 * 0.08;
                let ink = if shade > 0.8 {
                    'J'
                } else if shade > 0.55 {
                    'h'
                } else if shade > 0.3 {
                    'i'
                } else {
                    'H'
                };
                im.put(x, y, ink);
            }
        }
    }
    for n in 0..6 {
        let a = n as f32 * 1.05 + k * 3.0;
        let reach = r + 2.0 + swell * 3.0;
        im.put(
            (cx + a.cos() * reach) as i32,
            (cy + a.sin() * reach * 0.8) as i32,
            if n % 2 == 0 { '6' } else { '5' },
        );
    }
    im
}

pub(super) fn princess(tick: u32) -> Img {
    let sway = (tick / 10).is_multiple_of(2);
    rows(&[
        "....5.5.5.5.....",
        "....55555555....",
        "...666666666....",
        "..66ccccccc66...",
        "..66cKccKcc66...",
        "..66cccccc.66...",
        "..66.c77cc.66...",
        "..66..cccc.66...",
        "...6.57775.6....",
        "....77777777....",
        "...7775775777...",
        "..777777777777..",
        ".77778777787777.",
        ".77777777777777.",
        if sway { "7787777777778777" } else { "7778777777787777" },
        "5555555555555555",
    ])
}

pub(super) fn prince(tick: u32) -> Img {
    let sway = (tick / 10).is_multiple_of(2);
    rows(&[
        "....5.5.5.5.....",
        "....55555555....",
        "....rrrrrrrr....",
        "...rrccccccrr...",
        "...rcKccKccr....",
        "...rccccccccr...",
        "....cc7777cc....",
        "7...ccccccc..7..",
        "77..QQ5QQ5QQ.77.",
        "777.QzzzzzzzQ777",
        "777.QzQzzQzzQ777",
        "77..QzzzzzzzQ.77",
        "7...QQQQQQQQQ..7",
        if sway { "....zz.zz.zz...." } else { "....zz..zz.zz..." },
        "....bb....bb....",
        "...bbb....bbb...",
    ])
}

pub(super) fn clown(tick: u32) -> Img {
    let flop = (tick / 8).is_multiple_of(2);
    rows(&[
        "..7..6..2..6..7.",
        ".77.66.22.66.77.",
        "7777.6622662.777",
        ".7777.6666.7777.",
        "..77cccccccc77..",
        "...ccKccccKcc...",
        "...cccc77cccc...",
        "...ccc7777ccc...",
        "....cc.77.cc....",
        "...zWWWWWWWWz...",
        "..44447444444...",
        ".444477444774...",
        ".44444444444444.",
        "..4411441144....",
        if flop { "...44......44..." } else { "...44.....44...." },
        if flop { "..bbbb....bbbb.." } else { "..bbbb...bbbb..." },
    ])
}

fn figure(gift: Gift, tick: u32) -> Img {
    match gift {
        Gift::Princess => princess(tick),
        Gift::Prince => prince(tick),
        Gift::Clown => clown(tick),
    }
}

/// A figure at a puff's stage: the frog first, the cloud swelling over the
/// swap, then the figure; the same in reverse as they leave.
pub(super) fn transform(gift: Gift, age: u64, tick: u32) -> (Img, Option<Img>) {
    let linger_end = PUFF + LINGER;
    if age < PUFF {
        let k = age as f32 / PUFF as f32;
        let body = if k < 0.5 {
            frog(tick, false, false)
        } else {
            figure(gift, tick)
        };
        (body, Some(puff(k, 11)))
    } else if age >= linger_end - PUFF {
        let k = (age - (linger_end - PUFF)) as f32 / PUFF as f32;
        let body = if k < 0.5 {
            figure(gift, tick)
        } else {
            return (Img::new(1, 1), Some(puff(k, 29)));
        };
        (body, Some(puff(k, 29)))
    } else {
        (figure(gift, tick), None)
    }
}

/// Frogs hopping about the room.
pub(super) fn wild(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for f in &run.frogs.wild {
        draw_wild(cv, f, tick);
    }
}

fn draw_wild(cv: &mut Img, f: &Wild, tick: u32) {
    let (x, y) = at(f.x, f.y);
    let lift = (f.lift() * UNIT) as i32;
    let hopping = f.hop > 0 || f.leap.is_some();
    let blink = tick % 70 < 3;
    let mut im = frog(tick, hopping, blink);
    if f.face < 0 {
        im = im.flip_h();
    }
    // Its shadow stays on the ground.
    cv.rect(x - 3, y, 7, 1, 'k');
    cv.stamp(&im, x - im.w / 2, y - im.h + 1 - lift);
}

/// A frog on a knight's shoulder, bobbing a little as they walk. `(x, y)`
/// is the knight's feet in native pixels.
pub(super) fn shoulder(cv: &mut Img, hero: &Hero, x: i32, y: i32, tick: u32) {
    if !hero.rider || hero.frog() {
        return;
    }
    // Walking moves the knight's place, and the bob follows their steps;
    // standing still, it sits.
    let step = ((hero.x + hero.y) * 2.2).floor() as i32;
    let bob = i32::from(step.rem_euclid(2) == 0 && hero.rolling().is_none());
    let side = if hero.aim_x < -0.3 { -1 } else { 1 };
    let im = perched(tick % 90 < 3);
    cv.stamp(&im, x + side * 4 - im.w / 2, y - 10 + bob);
}

/// The figures a frog became, standing in the Sanctuary with their puff.
pub(super) fn shown(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    for s in &run.frogs.shown {
        let age = run.tick.saturating_sub(s.born);
        let (x, y) = at(s.x, s.y);
        let (body, cloud) = transform(s.gift, age, tick);
        if body.w > 1 {
            cv.rect(x - 5, y, 11, 1, 'k');
            cv.stamp(&body, x - body.w / 2, y - body.h + 1);
        }
        if let Some(cloud) = cloud {
            cv.stamp(&cloud, x - cloud.w / 2, y - cloud.h + 6);
        }
    }
}

/// What each figure gives, on a plate over them while they stay.
pub(super) fn plates(cv: &mut Img, run: &Run) {
    for s in &run.frogs.shown {
        let age = run.tick.saturating_sub(s.born);
        if age < PUFF || age >= LINGER {
            continue;
        }
        let (x, y) = at(s.x, s.y);
        let text = s.gift.plate();
        let w = text_width(text) + 6;
        let mut plate = Img::new(w + 1, 12);
        chrome::panel(&mut plate, (0, 0, w, 11), chrome::Tone::SLATE.rimmed(chrome::COLD_RIM));
        plate.text(3, 2, text, if s.gift == Gift::Clown { '7' } else { 'O' });
        cv.stamp(&plate, x - w / 2, y - 32);
    }
}

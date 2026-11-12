//! What a shot does, in the room's pixels around the bats and the knights:
//! the white flash on a struck bat, feathers and sparks, the harm that
//! rises off it; a felled bat's silhouette squashed and thrown, a burst of
//! feathers, a ring in the dust; the flock dropping in from the rafters;
//! each knight's run of bats as a tag with a fuse; a bite's ring and sting.
//! Every one is a pure function of the show's ticks.

use super::super::super::ink::{Img, bayer};
use super::super::juice;
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::super::{at, figures};
use crate::rules::belfry_show::{CHAIN_GAP, Hit};
use crate::rules::folly::BatShoot;
use crate::rules::juice::Weight;
use crate::rules::{EnemyKind, Run};

/// Ticks a kill's effects last.
const KILL_LIFE: u32 = 40;
/// Ticks a strike's effects last.
const HIT_LIFE: u32 = 20;

/// A bat's middle (room pixels) for a hit recorded at its feet.
fn middle(h: &Hit) -> (i32, i32) {
    (h.at.0, h.at.1 - 5)
}

/// A bat's silhouette: the live one if it is still on the wing, else the
/// room's own bat art.
fn silhouette(run: &Run, h: &Hit, tick: u32) -> Img {
    match run.enemies.iter().find(|e| e.id == h.bat) {
        Some(e) => figures::foe_figure_with(run, e, tick),
        None => super::super::demonic::enemy(run, EnemyKind::Bat, tick),
    }
}

/// On the floor, under the figures: the flock dropping in, a ring of dust
/// where a bat fell.
pub(super) fn under(cv: &mut Img, run: &Run, shoot: &BatShoot, view: (i32, i32)) {
    let now = run.tick as u32;
    let tick = run.tick;
    for s in shoot.show.swoops.iter().filter(|s| super::near(s.at, view, 48)) {
        let age = now.saturating_sub(s.born);
        let (x, y) = s.at;
        // A smear of dark falling out of the rafters, shorter as it lands.
        if age < 8 {
            let len = 30 * (8 - age as i32) / 8;
            for yy in y - 5 - len..y - 5 {
                let fade = (yy - (y - 5 - len)) as f32 / len.max(1) as f32;
                for dx in [0, 1] {
                    if bayer(x + dx, yy) < fade * 0.85 {
                        cv.put(x + dx, yy, if dx == 0 { 'j' } else { 'g' });
                    }
                }
            }
        }
        // Dust and feathers where it lands.
        let mut pen = Pen::new(cv, 160);
        if age < 14 {
            pen.shock((x as f32, y as f32 + 1.0), 3.0 + age as f32 * 1.4, 1.1, 0.45, ramps::SMOKE, age as f32 / 14.0);
        }
        let feathers = Spray { gravity: 0.03, ..Spray::round(s.at.0 as u32 ^ s.born, 4, (x as f32, y as f32 - 5.0), (0.4, 1.2), (10.0, 18.0)) };
        feathers.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Petal, ramps::EARTH, 1, tick));
    }
    for h in shoot.show.hits.iter().filter(|h| h.kill && h.who != 0 && super::near(h.at, view, 48)) {
        let age = h.age(now);
        if age < 14 {
            let mut pen = Pen::new(cv, 160);
            pen.shock((h.at.0 as f32, h.at.1 as f32 + 2.0), 4.0 + age as f32 * 1.6, 1.2, 0.45, ramps::SMOKE, age as f32 / 14.0);
        }
    }
}

/// Over the figures: flashes, bursts, numbers, tags and stings.
pub(super) fn over(cv: &mut Img, run: &Run, shoot: &BatShoot, focus: Option<u32>, view: (i32, i32)) {
    let now = run.tick as u32;
    let tick = run.tick;
    for h in shoot.show.hits.iter().filter(|h| super::near(h.at, view, 60)) {
        let age = h.age(now);
        let (x, y) = middle(h);
        let seed = (h.bat << 3) ^ h.born;
        let mine = focus.is_some_and(|f| u32::from(h.who) == f);
        if !h.kill {
            if age >= HIT_LIFE {
                continue;
            }
            if age < h.weight.flash() {
                let flash = juice::impact_frame(&silhouette(run, h, now), 'H');
                if let Some(e) = run.enemies.iter().find(|e| e.id == h.bat) {
                    let (bx, by) = at(e.x, e.y);
                    cv.stamp(&flash, bx - flash.w / 2, by - flash.h + 1);
                } else {
                    cv.stamp(&flash, x - flash.w / 2, y - flash.h / 2);
                }
            }
            if age < 14 {
                let mut pen = Pen::new(cv, 200);
                let sparks = Spray { gravity: 0.1, ..Spray::round(seed, 5, (x as f32, y as f32), (0.8, 2.2), (5.0, 11.0)) };
                sparks.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::EMBER, 1, tick));
                let feathers = Spray { gravity: 0.05, ..Spray::round(seed ^ 0x33, 3, (x as f32, y as f32), (0.5, 1.4), (10.0, 18.0)) };
                feathers.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Petal, ramps::EARTH, 1, tick));
            }
            if mine {
                if age < 7 {
                    juice::hitmarker(cv, (x, y), age, false);
                }
                juice::pop_number(cv, &h.lost.to_string(), (x, y - 5), age, 18, 1, 'T');
            }
            continue;
        }
        if age >= KILL_LIFE {
            continue;
        }
        kill(cv, run, h, age, (x, y), seed);
    }
    tags(cv, run, shoot, now, view);
    bites(cv, run, shoot, now, view);
}

/// A bat felled: its silhouette flashes, squashes and is gone; feathers
/// fly and settle, sparks spit, a ring goes out; the flock's last and a
/// third in a row are bigger.
fn kill(cv: &mut Img, run: &Run, h: &Hit, age: u32, (x, y): (i32, i32), seed: u32) {
    let tick = run.tick;
    let big = h.last || h.chain >= 3;
    let nobody = h.who == 0;
    let weight = if big { Weight::Kill } else { h.weight };
    if age < 5 && !nobody {
        let flash = juice::impact_frame(&silhouette(run, h, run.tick as u32), 'H');
        let (img, grow) = juice::squash_stretch(&flash, age, weight.squash() + 0.2);
        let lift = ((age as f32) * 1.2) as i32;
        cv.stamp(&img, x - img.w / 2, y + flash.h / 2 - img.h - lift);
        let _ = grow;
    }
    let mut pen = Pen::new(cv, 900);
    let (n, speed) = if big { (22, 3.2) } else if nobody { (7, 1.4) } else { (12, 2.6) };
    let feathers = Spray { gravity: 0.045, drag: 0.1, ..Spray::round(seed, n, (x as f32, y as f32), (speed * 0.4, speed), (16.0, 34.0)) };
    feathers.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Petal, if nobody { ramps::SMOKE } else { ramps::EARTH }, 1, tick));
    if !nobody {
        let sparks = Spray { gravity: 0.14, ..Spray::round(seed ^ 0x77, if big { 14 } else { 7 }, (x as f32, y as f32), (1.2, speed + 1.0), (7.0, 14.0)) };
        sparks.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Spark, ramps::GOLD, 2, tick));
        if age < 7 {
            pfx::rays(&mut pen, (x as f32, y as f32), if big { 10 } else { 7 }, if big { 20.0 } else { 11.0 }, 1.0, seed as f32 * 0.31, ramps::GOLD, age as f32 / 7.0);
        }
        if age < 18 {
            pen.shock((x as f32, y as f32), 3.0 + age as f32 * if big { 2.6 } else { 1.7 }, 1.3, 0.8, if big { ramps::EMBER } else { ramps::SMOKE }, age as f32 / 18.0);
        }
    } else {
        let smoke = Spray { gravity: -0.03, ..Spray::round(seed ^ 0x19, 5, (x as f32, y as f32), (0.3, 0.9), (14.0, 26.0)) };
        smoke.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Smoke, ramps::SMOKE, 1, tick));
    }
}

/// Each knight's run of bats: `x3` over their head, its fuse burning down
/// the two seconds the run has left to go on.
fn tags(cv: &mut Img, run: &Run, shoot: &BatShoot, now: u32, view: (i32, i32)) {
    let lead = shoot.scores.values().copied().max().unwrap_or(0);
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && !h.stone) {
        let n = shoot.show.run_of(id, now);
        if n < 2 {
            continue;
        }
        let Some(&(last, _)) = shoot.show.combo.get(&id) else { continue };
        let left = CHAIN_GAP.saturating_sub(now.saturating_sub(last));
        let (x, y) = at(hero.x, hero.y);
        if !super::near((x, y), view, 40) {
            continue;
        }
        let hot = n >= 5;
        let fresh = now.saturating_sub(last) < 4;
        let ink = if fresh { 'H' } else if hot { '@' } else if shoot.scores.get(&id).copied() == Some(lead) { 'T' } else { 't' };
        let scale = if fresh || n >= 8 { 2 } else { 1 };
        let tag = juice::big_text_img(&format!("X{n}"), scale, ink, if hot { 'p' } else { 'B' });
        let top = y - 48 - i32::from(fresh) * 2;
        cv.stamp(&tag, x - tag.w / 2, top);
        // The fuse: a line of the tag's own colour shortening to nothing.
        let w = (22 * left / CHAIN_GAP) as i32;
        cv.rect(x - 11, top + tag.h + 1, 22, 2, 'k');
        cv.rect(x - 11, top + tag.h + 1, w, 2, if left < 20 && now / 3 % 2 == 0 { '7' } else { 'O' });
    }
}

/// A bite: a red ring where the knight stands and the harm in red over
/// their head.
fn bites(cv: &mut Img, run: &Run, shoot: &BatShoot, now: u32, view: (i32, i32)) {
    for b in shoot.show.bites.iter().filter(|b| super::near(b.at, view, 40)) {
        let age = now.saturating_sub(b.born);
        if age >= 26 {
            continue;
        }
        let (x, y) = (b.at.0, b.at.1);
        let mut pen = Pen::new(cv, 300);
        if age < 12 {
            pen.shock((x as f32, y as f32 - 4.0), 4.0 + age as f32 * 1.7, 1.4, 0.7, ramps::BLOOD, age as f32 / 12.0);
        }
        let drops = Spray { gravity: 0.2, ..Spray::round(b.born ^ u32::from(b.who), 6, (x as f32, y as f32 - 8.0), (0.8, 2.2), (8.0, 15.0)) };
        drops.each(age as f32, |m| pfx::mote(&mut pen, &m, Shape::Droplet, ramps::BLOOD, 1, run.tick));
        juice::pop_number(cv, &format!("-{}", b.lost), (x, y - 20), age, 24, 1, '7');
    }
    let _ = bayer(0, 0);
}

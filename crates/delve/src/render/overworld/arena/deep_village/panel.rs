//! The Deep's boards: in a village, the name of the place as the star
//! lands, how much hope it holds, and a knight's conversation with one of
//! its six (their portrait, their words, the answers on 1, 2 and 3); below
//! it, the errands' tracker and their signs on the floor (a cold lamp, a
//! relic glowing on the stone, one of the village's own, lost).

use super::super::super::chrome::{self, Tone};
use super::super::super::ink::{Img, hash, text_width};
use super::super::{TILE, at, near_prompt, resized, stand};
use super::{folk, village};
use crate::rules::{
    Run,
    deep::{self, Culture, Sign, missions::SignKind, village as v},
};

/// The screen's boards for the Deep, in view pixels.
pub fn screen(cv: &mut Img, run: &Run, focus: Option<u32>) {
    if let Some(vl) = village(run) {
        let since = run.tick.saturating_sub(vl.arrived);
        if (v::FALL + 4..v::FALL + 150).contains(&since) {
            arrival(cv, vl.culture, vl.floor);
        } else if since >= v::FALL + 150 {
            hope(cv, vl.hope, vl.culture);
        }
        let id = focus.unwrap_or(1);
        if let Some((name, words, answers)) = run.talk_view(id) {
            let who = vl.talks.get(&id).map_or(0, |t| usize::from(t.who));
            talk(cv, vl.culture, who, &name, &words, &answers);
        }
        return;
    }
    if let Some(floor) = run.deep_floor() {
        tracker(cv, run, floor);
    }
}

/// Words broken into lines of at most `n` characters.
fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut row = String::new();
    for word in s.split_whitespace() {
        if !row.is_empty() && row.len() + 1 + word.len() > n {
            out.push(std::mem::take(&mut row));
        }
        if !row.is_empty() {
            row.push(' ');
        }
        row.push_str(word);
    }
    if !row.is_empty() {
        out.push(row);
    }
    out
}

/// `text` at twice size, in `ink`.
fn big(text: &str, ink: char) -> Img {
    let mut im = Img::new(text_width(text) + 2, 9);
    im.text(1, 1, text, ink);
    resized(&im, 2.0)
}

/// The place's name as the star lands: its floor, its town, its people.
fn arrival(cv: &mut Img, culture: Culture, floor: u32) {
    let name = culture.town(deep::chapter(floor));
    let title = big(name, '9');
    let floor_line = format!("FLOOR {floor}  THE DEEP");
    let people = format!("A VILLAGE OF {}", culture.people());
    let w = title.w.max(text_width(&people)).max(text_width(&floor_line)) + 24;
    let x = (cv.w - w) / 2;
    chrome::panel(cv, (x, 10, w, 46), Tone::FRAMED);
    chrome::ring(cv, (x, 10, w, 46), 3);
    cv.text((cv.w - text_width(&floor_line)) / 2, 14, &floor_line, 'h');
    cv.stamp(&title, (cv.w - title.w) / 2, 22);
    cv.text((cv.w - text_width(&people)) / 2, 44, &people, 'O');
}

/// How much hope the village holds: a lamp a step, lit as it grows.
fn hope(cv: &mut Img, hope: u8, culture: Culture) {
    let full = hope >= v::HOPE_FULL;
    let label = if full { "THE VILLAGE SHINES" } else { "HOPE" };
    let pips = i32::from(v::HOPE_FULL);
    let w = text_width(label) + 12 + if full { 0 } else { pips * 7 + 4 };
    let x = (cv.w - w) / 2;
    chrome::panel(cv, (x, 4, w, 14), Tone::SLATE);
    cv.text(x + 6, 8, label, if full { '9' } else { 'h' });
    if !full {
        let glow = super::look(culture).glow[3];
        for k in 0..pips {
            let px_ = x + 10 + text_width(label) + k * 7;
            let on = k < i32::from(hope);
            cv.rect(px_, 8, 5, 6, if on { glow } else { 'g' });
            if on {
                cv.put(px_ + 2, 9, 'w');
            }
        }
    }
}

/// A knight's conversation: the speaker's portrait, their name and words,
/// and the answers.
fn talk(cv: &mut Img, culture: Culture, who: usize, name: &str, words: &str, answers: &[String]) {
    let lines = wrap(words, 50);
    let h = (lines.len() as i32 * 9 + answers.len() as i32 * 10 + 30).max(62);
    let y = cv.h - h - 6;
    chrome::panel(cv, (6, y, cv.w - 12, h), Tone::FRAMED);
    chrome::ring(cv, (6, y, cv.w - 12, h), 3);
    let face = folk::portrait(culture, who);
    chrome::inset_panel(cv, (12, y + 8, face.w + 6, face.h + 6), Tone::WELL);
    cv.stamp(&face, 15, y + 11);
    let tx = 24 + face.w;
    cv.text(tx, y + 7, name, '9');
    cv.text(tx + text_width(name) + 6, y + 7, culture.people(), 'J');
    for (i, line) in lines.iter().enumerate() {
        cv.text(tx, y + 19 + i as i32 * 9, line, 'i');
    }
    let ay = y + 22 + lines.len() as i32 * 9;
    for (i, answer) in answers.iter().enumerate() {
        let yy = ay + i as i32 * 10;
        let keyed = answer.as_bytes().first().is_some_and(u8::is_ascii_digit);
        if keyed {
            chrome::keycap(cv, (tx - 1, yy - 2, 9, 10));
            cv.text(tx + 1, yy, &answer[..1], 'K');
            cv.text(tx + 12, yy, answer[2..].trim_start(), 'O');
        } else {
            cv.text(tx, yy, answer, 'O');
        }
    }
}

/// The errands' tracker on a floor of the Deep: the floor, the village
/// they are for, and each one's count.
fn tracker(cv: &mut Img, run: &Run, floor: u32) {
    let mut lines: Vec<(String, char)> = Vec::new();
    let head = if deep::chapter(floor) == 0 {
        format!("THE DEEP ROAD  FLOOR {floor}")
    } else {
        format!("THE DEEP  FLOOR {floor}")
    };
    lines.push((head, '9'));
    let errands: Vec<_> = run.home.deep.open_on(floor).map(|(_, m)| m.clone()).collect();
    let from = deep::errands_from(floor);
    if let Some(at_floor) = from {
        let culture = run.home.deep.culture_at(at_floor);
        lines.push((format!("FOR {}", culture.town(deep::chapter(at_floor))), 'J'));
    }
    let done: Vec<_> = run
        .home
        .deep
        .missions
        .iter()
        .filter(|m| Some(m.village) == from && m.done())
        .cloned()
        .collect();
    for m in &errands {
        lines.push((m.line(), 'O'));
    }
    for m in &done {
        lines.push((format!("{}  DONE", m.line()), '2'));
    }
    if errands.is_empty() && done.is_empty() {
        let next = (deep::chapter(floor) + 1) * deep::CHAPTER;
        let culture = run.home.deep.culture_at(next);
        lines.push((format!("{} WAITS ON FLOOR {next}", culture.town(deep::chapter(next))), 'h'));
    }
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 12;
    let h = lines.len() as i32 * 9 + 7;
    let (x, y) = (4, 44);
    chrome::panel(cv, (x, y, w, h), Tone::SLATE);
    for (i, (s, ink)) in lines.iter().enumerate() {
        cv.text(x + 6, y + 4 + i as i32 * 9, s, *ink);
    }
}

/// The errands' signs in the room the party stands in, over its floor.
pub fn signs(cv: &mut Img, run: &Run) {
    let Some(d) = run.deep.as_ref().filter(|d| d.village.is_none()) else {
        return;
    };
    let tick = run.tick as u32;
    for sign in d.signs.iter().filter(|s| s.room == run.at) {
        let (x, y) = at(sign.point().0, sign.point().1);
        let (y, _) = (y + TILE / 2 - 2, 0);
        match sign.kind {
            SignKind::Lamp => lamp(cv, sign, (x, y), tick),
            SignKind::Relic => relic(cv, sign, (x, y), tick),
            SignKind::Lost => lost(cv, run, sign, (x, y), tick),
        }
        if let Some(age) = sign.at.map(|t| run.tick.saturating_sub(t)).filter(|&a| a < 40) {
            for k in 0..16 {
                let h = hash(k, 1, 9501);
                let r = age as f32 * (0.6 + (h % 10) as f32 / 10.0);
                let a = (h >> 4) as f32 % 628.0 / 100.0;
                cv.put(x + (a.cos() * r) as i32, y - 12 - age as i32 / 2 + (a.sin() * r * 0.5) as i32, ['w', '9', '6'][k as usize % 3]);
            }
        }
        if !sign.done {
            let (lamp_word, _, _) = sign.culture.sign_words();
            match sign.kind {
                SignKind::Lamp => near_prompt(cv, run, (x, y + 46), &format!("LIGHT THE {lamp_word}")),
                SignKind::Lost => near_prompt(cv, run, (x, y + 46), "SEND THEM HOME"),
                SignKind::Relic => {}
            }
        }
    }
}

/// A cold lamp of the village's, lit once the party has lit it: a stone
/// foot, a tall post, the people's own lamp at its head; lit, a flame and
/// a ring of light on the floor round it, cold, a faint blue ember.
fn lamp(cv: &mut Img, sign: &Sign, (x, y): (i32, i32), tick: u32) {
    let lit = sign.done;
    let look = super::look(sign.culture);
    super::super::super::material::shadow(cv, ((x + 4) as f32, (y + 1) as f32), (11.0, 3.0), &super::all);
    // The stone foot.
    cv.ellipse(x as f32, y as f32, 9.0, 3.5, look.rock[2]);
    cv.ellipse(x as f32, (y - 1) as f32, 7.0, 2.5, look.rock[4]);
    // The post.
    cv.rect(x - 2, y - 34, 5, 34, look.rock[3]);
    cv.line(x - 2, y - 34, x - 2, y - 1, look.rock[5]);
    cv.line(x + 2, y - 34, x + 2, y - 1, look.rock[1]);
    for ry in [y - 10, y - 22] {
        cv.line(x - 3, ry, x + 3, ry, look.rock[0]);
    }
    let head = y - 36;
    let (core, halo) = look.flame;
    match sign.culture {
        Culture::Capfolk => {
            cv.ellipse(x as f32, (head - 3) as f32, 10.0, 6.0, if lit { look.glow[2] } else { 'x' });
            cv.ellipse((x - 2) as f32, (head - 5) as f32, 5.0, 2.5, if lit { 'w' } else { 'q' });
            cv.line(x - 9, head + 1, x + 9, head + 1, if lit { look.glow[3] } else { 's' });
        }
        Culture::Glimmerkin => {
            cv.frame(x - 7, head - 14, 15, 15, 'G');
            cv.rect(x - 6, head - 13, 13, 13, if lit { look.glow[2] } else { 'S' });
            super::sky::crystal(cv, x, head - 1, 11, 9);
            if lit {
                cv.rect(x - 1, head - 9, 3, 4, 'w');
            }
        }
        Culture::Emberfolk => {
            cv.rect(x - 9, head - 2, 19, 5, 'X');
            cv.line(x - 10, head - 3, x + 10, head - 3, 'j');
            cv.line(x - 8, head + 3, x + 8, head + 3, 'K');
            if !lit {
                cv.line(x - 6, head - 4, x + 6, head - 4, '8');
            }
        }
        Culture::Lampwick => {
            cv.line(x, head, x + 8, head - 2, 'B');
            let lantern = super::super::super::kit::lantern(lit);
            cv.stamp(&lantern, x + 5, head - 3);
            cv.stamp(&lantern, x - 3, head - 12);
        }
        Culture::Roostfolk => super::roost::sign_lamp(cv, x, head, lit),
        Culture::Rootkin => super::roots::sign_lamp(cv, x, head, lit),
        Culture::Chimefolk => super::bells::sign_lamp(cv, x, head, lit),
        Culture::Saltkin => super::salt::sign_lamp(cv, x, head, lit),
    }
    if lit {
        // The flame, and the light it throws on the floor.
        for k in 0..9 {
            let h = hash(k, (tick / 3) as i32, 9502);
            let fx = x - 5 + (h % 11) as i32;
            let tall = 5 + (h >> 4) as i32 % 9;
            cv.line(fx, head - 4, fx + ((h >> 9) % 3) as i32 - 1, head - 4 - tall, if k % 3 == 0 { core } else { halo });
        }
        cv.put(x, head - 6, 'w');
        for (r, ink) in [(22.0f32, halo), (16.0, core)] {
            for k in 0..40 {
                let a = k as f32 / 40.0 * std::f32::consts::TAU;
                if (k + tick as usize / 3) % 4 != 0 {
                    cv.put(x + (a.cos() * r) as i32, y + (a.sin() * r * 0.35) as i32, ink);
                }
            }
        }
        // Sparks going up from it, a beacon for the village above.
        for k in 0..6u32 {
            let t = (tick + k * 11) % 50;
            cv.put(x - 3 + (hash(k as i32, 2, 9503) % 7) as i32, head - 10 - t as i32, if t < 20 { core } else { halo });
        }
    } else if (tick / 10) % 3 == 0 {
        cv.put(x, head - 4, 'q');
    }
}

/// A relic on the floor, glowing and turning, waiting to be picked up.
fn relic(cv: &mut Img, sign: &Sign, (x, y): (i32, i32), tick: u32) {
    if sign.done {
        return;
    }
    let bob = ((tick as f32 * 0.1).sin() * 2.0) as i32;
    let look = super::look(sign.culture);
    for k in 0..10 {
        let a = k as f32 / 10.0 * std::f32::consts::TAU + tick as f32 * 0.03;
        cv.put(x + (a.cos() * 9.0) as i32, y - 2 + (a.sin() * 3.0) as i32, look.glow[1]);
    }
    let cy = y - 10 + bob;
    match sign.culture {
        Culture::Capfolk => {
            cv.line(x, cy, x, cy + 5, 'T');
            cv.ellipse(x as f32, cy as f32, 5.0, 3.0, '2');
            cv.put(x - 2, cy - 1, 'w');
        }
        Culture::Glimmerkin => super::sky::crystal(cv, x, cy + 5, 9, 3),
        Culture::Emberfolk => {
            cv.ellipse(x as f32, cy as f32, 5.0, 4.0, 'j');
            cv.ellipse((x - 1) as f32, (cy - 1) as f32, 3.0, 2.0, 'J');
            cv.put(x + 2, cy + 1, '@');
            cv.put(x - 2, cy - 2, 'w');
        }
        Culture::Lampwick => {
            cv.ellipse(x as f32, cy as f32, 4.0, 4.0, 'W');
            cv.ellipse((x - 1) as f32, (cy - 1) as f32, 2.0, 2.0, 'w');
            cv.line(x, cy + 4, x + 3, cy + 8, 'h');
        }
        Culture::Roostfolk => super::roost::relic(cv, x, cy),
        Culture::Rootkin => super::roots::relic(cv, x, cy),
        Culture::Chimefolk => super::bells::relic(cv, x, cy),
        Culture::Saltkin => super::salt::relic(cv, x, cy),
    }
    // Motes rising off it: a thing of the village's, wanting finding.
    for k in 0..5u32 {
        let t = (tick + k * 13) % 40;
        let mx = x - 4 + (hash(k as i32, 7, 9504) % 9) as i32;
        cv.put(mx, cy - 4 - t as i32, if t < 14 { look.glow[3] } else { look.glow[2] });
    }
    if (tick / 6) % 8 == 0 {
        cv.put(x + 4, cy - 6, 'w');
    }
}

/// One of the village's own, lost: huddled in the dark and shivering
/// with nobody near, up and waving when a knight comes, a call over their
/// head; found, glad, hearts going up, and then away home up the stair as
/// a little light.
fn lost(cv: &mut Img, run: &Run, sign: &Sign, (x, y): (i32, i32), tick: u32) {
    let age = sign.at.map(|t| run.tick.saturating_sub(t));
    if let Some(age) = age {
        if age >= LOST_GOES {
            return;
        }
        let a = age as u32;
        if a < LOST_GLAD {
            let im = folk::lost_figure(sign.culture, folk::Lost::Glad, tick);
            let hop = [0, 2, 3, 2][(a / 3 % 4) as usize];
            stand(cv, &im, x, y - hop);
            for k in 0..4u32 {
                let t = (a + k * 7) % 24;
                let hx = x - 6 + k as i32 * 4 + ((t as f32 * 0.3).sin() * 2.0) as i32;
                heart(cv, hx, y - im.h - 2 - t as i32, t > 16);
            }
        } else {
            // Home: up the stair as a mote of light, a trail behind it.
            let t = (a - LOST_GLAD) as f32 / (LOST_GOES - u64::from(LOST_GLAD)) as f32;
            let (hx, hy) = (x as f32 + t * 30.0, y as f32 - 12.0 - t * t * 160.0);
            for k in 0..10 {
                let f = k as f32 / 10.0;
                cv.put((hx - f * 8.0) as i32, (hy + f * 20.0) as i32, if k < 2 { 'w' } else if k < 5 { '9' } else { '6' });
            }
            cv.rect(hx as i32 - 1, hy as i32 - 1, 3, 3, 'w');
        }
        return;
    }
    let near = run.players.values().any(|h| h.hp > 0 && {
        let (hx, hy) = at(h.x, h.y);
        (hx - x).abs() + (hy - y).abs() < 5 * TILE
    });
    let pose = if near { folk::Lost::Waving } else { folk::Lost::Huddled };
    let im = folk::lost_figure(sign.culture, pose, tick);
    // Shivering in the dark, now and then.
    let shiver = if !near && (tick / 3) % 9 < 2 { (tick % 2) as i32 } else { 0 };
    stand(cv, &im, x + shiver, y);
    // A call over their head: a word, or their song for the chorister.
    let (bx, by) = (x, y - im.h - 6);
    if near || (tick / 40) % 3 != 2 {
        cv.rect(bx - 2, by - 6, 5, 9, 'W');
        cv.rect(bx - 1, by - 5, 3, 5, '7');
        cv.put(bx, by + 1, '7');
    }
    if sign.culture == Culture::Chimefolk {
        for k in 0..2u32 {
            let t = ((tick + k * 20) % 40) as i32;
            note(cv, bx + 6 + t / 6, by - 2 - t / 3);
        }
    }
    if !near && (tick / 30) % 4 == 0 {
        // A tear.
        cv.put(x + 1, y - im.h + 8 + ((tick / 4) % 3) as i32, '3');
    }
}

/// How long a found one is glad before going, and before they are gone.
const LOST_GLAD: u32 = 36;
const LOST_GOES: u64 = 70;

/// A heart going up over the glad.
fn heart(cv: &mut Img, x: i32, y: i32, fading: bool) {
    let ink = if fading { 'c' } else { '7' };
    cv.put(x - 1, y, ink);
    cv.put(x + 1, y, ink);
    cv.rect(x - 2, y + 1, 5, 1, ink);
    cv.rect(x - 1, y + 2, 3, 1, ink);
    cv.put(x, y + 3, ink);
}

/// A note of a song, sung.
fn note(cv: &mut Img, x: i32, y: i32) {
    cv.rect(x, y + 3, 2, 2, 'W');
    cv.line(x + 1, y, x + 1, y + 3, 'W');
    cv.put(x + 2, y, 'W');
}

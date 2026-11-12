//! Brasswake's end of the round: the brewery darkened behind a podium, fireworks over it, paper
//! falling, each knight's work and falls, the pot and the crown, the best and the way back in.
use super::super::super::ink::{Img, bayer, text_width};
use super::super::juice::{self, Place};
use super::hud::{self, Rect, W, Word};
use super::moments;
use crate::rules::brasswake::{GOLD_PER_SEAT, Session, WORK_PER_RECORD};
use crate::rules::{HZ, Run};

const FLOOR: i32 = 34 + 134 - 6;
const EXTRA_Y: i32 = 186;
const STATS_Y: i32 = 196;
const PRIZE_Y: i32 = 206;
const KEYS_Y: i32 = 216;
const TITLE_Y: i32 = 8;

/// Ticks since the round ended.
pub(super) fn age(run: &Run, s: &Session) -> u32 {
    (run.tick as u32).saturating_sub(s.done)
}

fn secs(t: u32) -> String {
    format!("{}.{:02} S", t / HZ, (t % HZ) * 100 / HZ)
}

/// The title the round ended under.
pub(super) fn title(s: &Session) -> &'static str {
    if s.won { "BREWERY CLEARED" } else { "SHIFT FINISHED" }
}

fn podium_xs() -> [i32; 3] {
    [W / 2, W / 2 - 88, W / 2 + 88]
}

/// Where the podium's names and lines are drawn (the kit's own placement), for the claims.
fn podium_boxes(run: &Run, s: &Session) -> Vec<(&'static str, Rect)> {
    let mut out = Vec::new();
    for (i, &who) in s.order.iter().take(3).enumerate() {
        let x = podium_xs()[i];
        let name = hud::name_of(run, who, 8);
        let line = s.lanes[&who].score.to_string();
        out.push(("podium name", (x - text_width(&name) / 2, FLOOR + 4, text_width(&name), 7)));
        out.push(("podium line", (x - text_width(&line) / 2, FLOOR + 13, text_width(&line), 7)));
    }
    out
}

pub(super) fn words(run: &Run, s: &Session, id: u32) -> Vec<Word> {
    let mut out = Vec::new();
    let w = |x: i32, y: i32, text: String, ink: char| Word { x, y, text, ink };
    let centre = |y: i32, text: String, ink: char| Word { x: (W - text_width(&text)) / 2, y, text, ink };
    for (n, &who) in s.order.iter().enumerate().skip(3) {
        let lane = &s.lanes[&who];
        let text = format!("{} {} {}", s.place(who), hud::short(&hud::name_of(run, who, 12)), lane.score);
        out.push(w(8 + (n as i32 - 3) * 128, EXTRA_Y, text, 'i'));
    }
    let me = &s.lanes[&id];
    let time = me.finish.map_or("--".to_string(), secs);
    out.push(centre(STATS_Y, format!("YOU {} WORK  {} FALLS  {}", me.score, me.falls, time), 'h'));
    let lead = s.order[0];
    let tied = s.order.iter().filter(|&&who| s.score(who) == s.score(lead)).count() as u32;
    let pot = GOLD_PER_SEAT * s.lanes.len() as u32;
    let prize = if tied == 1 {
        format!("{} {}G + CROWN", hud::name_of(run, lead, 12), pot)
    } else {
        format!("{}G EACH + CROWN", pot / tied)
    };
    out.push(centre(PRIZE_Y, prize, 'T'));
    let best = u32::from(run.home.countryside.games.brasswake.best) * WORK_PER_RECORD;
    let flash = s.broke && (run.tick / 6) % 2 == 0;
    out.push(w(8, KEYS_Y, if s.broke { format!("NEW BEST {best}") } else { format!("BEST {best}") }, if flash { 'T' } else if s.broke { '6' } else { 'h' }));
    let blink = (run.tick / 10) % 2 == 0;
    out.push(centre(KEYS_Y, "E AGAIN".into(), if blink { 'T' } else { 'o' }));
    let place = format!("PLACE {}", s.place(id));
    out.push(Word { x: W - 8 - text_width(&place), y: KEYS_Y, text: place, ink: 'h' });
    out
}

/// Every box on the end screen: the title, the podium's names and lines, the words.
pub(super) fn boxes(run: &Run, s: &Session, id: u32) -> Vec<(&'static str, Rect)> {
    let a = age(run, s);
    let mut out = vec![("title", moments::banner_rect_at(TITLE_Y, title(s), "", a, 2))];
    out.extend(podium_boxes(run, s));
    out.extend(words(run, s, id).iter().map(|w| ("word", w.rect())));
    out
}

/// The end screen over `room`, the brewery as the knight last saw it.
pub(super) fn paint(room: &Img, run: &Run, s: &Session, id: u32, sprite: &dyn Fn(u32) -> Img, with_text: bool) -> Img {
    let a = age(run, s);
    let tick = run.tick;
    let mut im = Img::black(W, hud::H);
    im.stamp(room, 0, 0);
    // The room, dimmed to a stage by an ordered dither.
    for y in hud::TOP..hud::BOTTOM {
        for x in 0..W {
            if bayer(x, y) < 0.5 {
                im.put(x, y, 'k');
            }
        }
    }
    im.rect(0, 33, W, 1, 'S');
    // Fireworks over the aisle, five at a time, each on its own beat.
    for i in 0..5u32 {
        let beat = (a as i32 - i as i32 * 15).rem_euclid(96) as u32;
        let x = 40.0 + (hud::W as f32 - 80.0) * ((i * 37 + 11) % 100) as f32 / 100.0;
        let top = 52.0 + ((i * 53) % 40) as f32;
        let ramp = [super::super::pfx::ramps::GOLD, super::super::pfx::ramps::EMBER, super::super::pfx::ramps::HOLY][i as usize % 3];
        if a + 15 * i >= 15 {
            juice::firework(&mut im, x, 150.0, top, beat, 0x51 + i, ramp, tick);
        }
    }
    let places: Vec<Place> = s
        .order
        .iter()
        .take(3)
        .map(|&who| Place { sprite: sprite(who), name: hud::name_of(run, who, 8), line: s.lanes[&who].score.to_string() })
        .collect();
    let crown = juice::podium(&mut im, (0, 34, W, 134), &places, a);
    if a > 30 && !places.is_empty() {
        // The crown settles on the winner's helm and bobs; a glint crosses it.
        let drop = ((1.0 - ((a as f32 - 30.0) / 10.0).clamp(0.0, 1.0)) * 24.0) as i32;
        let bob = if a > 44 { ((a / 8) % 2) as i32 } else { 0 };
        let crown_img = Img::from_rows(&[
            "k..k.k.k..k",
            "kTkkTkTkkTk",
            "kTTTTTTTTTk",
            "kTO7TOT7OTk",
            "kOOOOOOOOOk",
            "kkkkkkkkkkk",
        ]);
        im.stamp(&crown_img, crown.0 - 5, crown.1 - 9 - drop - bob);
        if a > 44 && (a / 6) % 5 == 0 {
            im.put(crown.0 - 3 + (a as i32 / 6 % 3) * 3, crown.1 - 8 - bob, 'w');
        }
    }
    if a > 20 {
        juice::confetti(&mut im, (0, 34, W, 134), a, 0x42, 46);
    }
    juice::banner(&mut im, TITLE_Y, title(s), "", a, 9999, juice::Tone::Warm, 2);
    if with_text {
        for w in words(run, s, id) {
            im.text(w.x, w.y, &w.text, w.ink);
        }
    }
    im
}

//! The city's wishes, standing (`deep::city`): each visitor's wish in its
//! place in the village, as the city built it. A lantern tree, a glowing
//! garden, a statue of the knight in the village's stone, a fountain, a
//! bell tower, a market stall, a reading stand, a great hearth. A wish just
//! made rises out of the ground in a shower of sparks while the village
//! cheers (`city::RISE`); a knight who comes near reads the words on it.

use super::super::super::ink::{BLACK, Img, hash, text_width};
use super::super::super::material as mat;
use super::super::{TILE, at, knights};
use super::{all as keep_all, px};
use crate::rules::{
    Run,
    deep::{
        Town, Village,
        city::{CityWish, Gift, RISE},
    },
};

/// The six places a wish stands (tile points, its foot).
pub const SLOTS: [(f32, f32); 6] = [(13.4, 12.8), (23.0, 12.8), (8.8, 13.8), (27.4, 13.8), (14.2, 20.4), (22.4, 20.4)];

pub(super) fn all(cv: &mut Img, run: &Run, vl: &Village, town: Option<&Town>) {
    let Some(town) = town else {
        return;
    };
    let tick = run.tick as u32;
    for wish in &town.wishes {
        let (x, y) = px(SLOTS[usize::from(wish.slot) % SLOTS.len()]);
        let rose = vl.raised.iter().find(|(s, _)| *s == wish.slot).map(|&(_, t)| run.tick.saturating_sub(t));
        let mut im = Img::new(56, 64);
        draw(&mut im, run, wish, tick);
        let (fx, fy) = (28, 60);
        match rose.filter(|&age| age < RISE) {
            Some(age) => {
                // Rising out of the ground: only so much of it is up yet.
                let f = age as f32 / RISE as f32;
                let shown = (im.h as f32 * f) as i32;
                let mut part = Img::new(im.w, im.h);
                for yy in im.h - shown..im.h {
                    for xx in 0..im.w {
                        if let Some(c) = im.get(xx, yy - (im.h - shown) / 3) {
                            part.set(xx, yy, c);
                        }
                    }
                }
                cv.stamp(&part, x - fx, y - fy);
                sparkle(cv, (x, y), age as u32, f);
            }
            None => cv.stamp(&im, x - fx, y - fy),
        }
        if rose.is_some_and(|age| age < RISE * 3) {
            sparkle(cv, (x, y - 20), rose.unwrap_or(0) as u32 + 40, 1.0);
        }
    }
    // The words on the one wish a knight stands nearest, so two never
    // overlap.
    let nearest = town
        .wishes
        .iter()
        .flat_map(|w| {
            let (x, y) = px(SLOTS[usize::from(w.slot) % SLOTS.len()]);
            run.players.values().filter(|h| h.hp > 0).map(move |h| {
                let (hx, hy) = at(h.x, h.y);
                (((hx - x) as f32).hypot((hy - y) as f32), w, (x, y))
            })
        })
        .filter(|(d, _, _)| *d < 2.6 * TILE as f32)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, wish, at_)) = nearest {
        plaque(cv, wish, at_);
    }
}

/// Sparks thrown up round a wish as it rises.
fn sparkle(cv: &mut Img, (x, y): (i32, i32), age: u32, f: f32) {
    for k in 0..28 {
        let h = hash(k, 3, 9401);
        let life = 24 + h % 20;
        let t = (age + h % life) % life;
        let a = (h >> 6) as f32 % 628.0 / 100.0;
        let r = 4.0 + t as f32 * (0.8 + f);
        let (sx, sy) = (x + (a.cos() * r) as i32, y - 6 - (t as i32) + (a.sin() * r * 0.4) as i32);
        cv.put(sx, sy, ['w', '9', '6', '3'][(h >> 3) as usize % 4]);
        if t < 4 {
            cv.put(sx + 1, sy, '9');
        }
    }
}

/// The words on a wish, over it.
fn plaque(cv: &mut Img, wish: &CityWish, (x, y): (i32, i32)) {
    let words: String = wish.words.to_uppercase().chars().take(38).collect();
    let lines = [format!("\"{words}\""), format!("WISHED BY {}", wish.shown.to_uppercase())];
    let w = lines.iter().map(|l| text_width(l)).max().unwrap_or(0) + 10;
    let (bx, by) = ((x - w / 2).clamp(2, cv.w - w - 2), (y - 88).max(2));
    super::super::super::chrome::panel(cv, (bx, by, w, 24), super::super::super::chrome::Tone::SIGN);
    for (i, l) in lines.iter().enumerate() {
        cv.text(bx + (w - text_width(l)) / 2, by + 4 + i as i32 * 9, l, if i == 0 { '9' } else { 'O' });
    }
}

/// One wish, drawn on a 56 by 64 canvas, its foot at (28, 60).
fn draw(im: &mut Img, run: &Run, wish: &CityWish, tick: u32) {
    let (cx, foot) = (28, 60);
    match wish.gift {
        Gift::Lanterns => {
            // A tree of lanterns: a dark trunk branching, a lamp at every tip.
            mat::shadow(im, ((cx + 4) as f32, foot as f32), (14.0, 3.0), &keep_all);
            for (x0, y0, x1, y1) in [(cx, foot, cx, foot - 34), (cx, foot - 20, cx - 14, foot - 32), (cx, foot - 26, cx + 14, foot - 38), (cx, foot - 34, cx - 8, foot - 46), (cx, foot - 34, cx + 6, foot - 50)] {
                im.line(x0, y0, x1, y1, 'b');
                im.line(x0 + 1, y0, x1 + 1, y1, 'I');
            }
            for (k, (lx, ly)) in [(cx - 14, foot - 32), (cx + 14, foot - 38), (cx - 8, foot - 46), (cx + 6, foot - 50), (cx - 3, foot - 24)].into_iter().enumerate() {
                let sway = ((tick as f32 * 0.08 + k as f32).sin() * 1.2) as i32;
                im.line(lx, ly, lx + sway, ly + 4, 'K');
                im.rect(lx - 2 + sway, ly + 4, 5, 6, 'k');
                im.rect(lx - 1 + sway, ly + 5, 3, 4, '5');
                im.put(lx + sway, ly + 6, '6');
            }
        }
        Gift::Garden => {
            im.ellipse(cx as f32, (foot - 4) as f32, 24.0, 7.0, 'b');
            im.ellipse(cx as f32, (foot - 5) as f32, 22.0, 5.0, 'I');
            for k in 0..14 {
                let h = hash(k, 1, 9411);
                let (fx, fy) = (cx - 19 + (h % 38) as i32, foot - 6 - ((h >> 8) % 6) as i32);
                let tall = 4 + (h >> 16) as i32 % 6;
                im.line(fx, fy, fx, fy - tall, 'l');
                let bloom = ['3', '6', '2', '9', 'Y'][(h >> 4) as usize % 5];
                let open = (tick / 12 + h) % 5 != 0;
                im.put(fx, fy - tall - 1, bloom);
                if open {
                    im.put(fx - 1, fy - tall, bloom);
                    im.put(fx + 1, fy - tall, bloom);
                }
            }
            mat::broadleaf(im, cx + 10, foot - 6, 9.0, mat::ramp::FOCAL, 9412, 1, &keep_all);
        }
        Gift::Statue => {
            // The plinth, and the knight in stone on it.
            mat::ashlar(im, (cx - 12, foot - 12, 24, 12), &['g', 'j', 'G', 'J', 'h'], 9413);
            im.line(cx - 13, foot - 12, cx + 12, foot - 12, 'i');
            // The knight's own figure, where it can be had, else a knight
            // with the sword raised; twice their size, in the village's stone.
            let own = run
                .players
                .iter()
                .find(|(_, h)| crate::rules::talents::knight_key(h) == wish.knight)
                .map(|(&id, h)| knights::figure_at(h, id, false, Default::default(), false, 0))
                .filter(|f| f.pixels().filter(|&c| c != BLACK).count() > 60);
            let figure = trim(&own.unwrap_or_else(generic_knight));
            let statue = super::super::resized(&stone(&figure), if figure.h > 26 { 1.5 } else { 2.0 });
            im.stamp(&statue, cx - statue.w / 2, foot - 12 - statue.h);
            im.rect(cx - 6, foot - 8, 12, 4, 'O');
            im.line(cx - 5, foot - 6, cx + 5, foot - 6, 'o');
        }
        Gift::Fountain => {
            im.ellipse(cx as f32, (foot - 6) as f32, 22.0, 7.0, 'u');
            im.ellipse(cx as f32, (foot - 7) as f32, 19.0, 5.0, 'S');
            im.ellipse(cx as f32, (foot - 7) as f32, 17.0, 4.0, 'Q');
            im.rect(cx - 2, foot - 26, 5, 20, 'U');
            im.ellipse(cx as f32, (foot - 26) as f32, 7.0, 2.5, 'v');
            for k in 0..20 {
                let h = hash(k, 2, 9414);
                let t = (tick + h % 30) % 30;
                let side = if k % 2 == 0 { -1 } else { 1 };
                let x = cx + side * (t as i32 * (2 + (h % 3) as i32)) / 6;
                let y = foot - 28 - (t as i32 * (30 - t as i32)) / 20;
                im.put(x, y, if t < 10 { 'W' } else { 'z' });
            }
            for k in 0..4 {
                let r = ((tick / 4 + k * 5) % 18) as f32;
                im.put(cx - r as i32, foot - 7, 'V');
                im.put(cx + r as i32, foot - 7, 'V');
            }
        }
        Gift::Bell => {
            for px_ in [cx - 12, cx + 11] {
                im.rect(px_, foot - 40, 3, 40, 'B');
                im.line(px_, foot - 40, px_, foot, 'P');
            }
            mat::roof_hip(im, (cx - 16, foot - 50, 33, 11), &['n', 'b', 'B', 'p', 'R', 'r'], 9415, 8);
            im.line(cx - 12, foot - 39, cx + 13, foot - 39, 'P');
            let swing = ((tick as f32 * 0.12).sin() * 3.0) as i32;
            im.line(cx, foot - 38, cx + swing / 2, foot - 34, 'K');
            im.ellipse((cx + swing) as f32, (foot - 28) as f32, 6.0, 7.0, 'R');
            im.ellipse((cx + swing - 1) as f32, (foot - 29) as f32, 4.0, 5.0, 'O');
            im.line(cx + swing - 6, foot - 22, cx + swing + 6, foot - 22, 'o');
            im.put(cx + swing - 2, foot - 31, 't');
        }
        Gift::Market => {
            let stall = super::super::super::kit::market_stall(wish.slot as u32);
            let big = super::super::resized(&stall, 2.0);
            im.stamp(&big, cx - big.w / 2, foot - big.h);
            for (k, ink) in ['R', 'O', 'y', '6'].into_iter().enumerate() {
                im.ellipse((cx - 9 + k as i32 * 6) as f32, (foot - 9) as f32, 2.0, 2.0, ink);
            }
        }
        Gift::Library => {
            im.rect(cx - 2, foot - 20, 5, 20, 'B');
            im.rect(cx - 12, foot - 26, 25, 5, 'P');
            im.line(cx - 13, foot - 27, cx + 13, foot - 27, 'o');
            im.rect(cx - 10, foot - 31, 9, 4, 'T');
            im.rect(cx + 1, foot - 31, 9, 4, 't');
            im.line(cx, foot - 31, cx, foot - 27, 'B');
            for (k, ink) in ['R', 'q', 'l', 'p', 'Q'].into_iter().enumerate() {
                im.rect(cx + 10, foot - 4 - k as i32 * 3, 10, 3, ink);
                im.line(cx + 10, foot - 4 - k as i32 * 3, cx + 19, foot - 4 - k as i32 * 3, 'T');
            }
            let candle = super::super::super::kit::candles(2, tick);
            im.stamp(&candle, cx - 16, foot - 14);
        }
        Gift::Hearth => {
            for k in 0..10 {
                let a = k as f32 / 10.0 * std::f32::consts::TAU;
                im.ellipse(cx as f32 + a.cos() * 12.0, (foot - 6) as f32 + a.sin() * 4.0, 3.0, 2.0, if k % 2 == 0 { 'g' } else { 'j' });
            }
            for k in 0..6 {
                let h = hash(k, (tick / 3) as i32, 9416);
                let fx = cx - 5 + (h % 11) as i32;
                let tall = 6 + (h >> 4) as i32 % 9;
                im.line(fx, foot - 6, fx + ((h >> 8) % 3) as i32 - 1, foot - 6 - tall, if k % 3 == 0 { '6' } else { '@' });
                im.put(fx, foot - 7, '5');
            }
            for bx in [cx - 26, cx + 16] {
                mat::boards(im, (bx, foot - 6, 10, 4), false, 9417);
            }
        }
    }
}

/// `im` cut down to what is drawn in it.
fn trim(im: &Img) -> Img {
    let (mut x0, mut y0, mut x1, mut y1) = (im.w, im.h, -1, -1);
    for y in 0..im.h {
        for x in 0..im.w {
            if im.get(x, y).is_some_and(|c| c != BLACK) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    if x1 < x0 {
        return im.clone();
    }
    let mut out = Img::new(x1 - x0 + 1, y1 - y0 + 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if let Some(c) = im.get(x, y).filter(|&c| c != BLACK) {
                out.set(x - x0, y - y0, c);
            }
        }
    }
    out
}

/// A figure turned to the village's stone: every ink by its lightness,
/// stretched across the stone's whole ramp so its features stay, and a
/// dark edge round it.
fn stone(im: &Img) -> Img {
    const RAMP: [char; 6] = ['G', 'J', 'h', 'i', 'H', 'W'];
    let lum = |c: [u8; 3]| 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
    let lums: Vec<f32> = im.pixels().filter(|&c| c != BLACK).map(lum).collect();
    let (lo, hi) = lums.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &l| (lo.min(l), hi.max(l)));
    let span = (hi - lo).max(1.0);
    let mut out = Img::new(im.w + 2, im.h + 2);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y).filter(|&c| c != BLACK) {
                let t = (lum(c) - lo) / span;
                out.put(x + 1, y + 1, RAMP[((t * RAMP.len() as f32) as usize).min(RAMP.len() - 1)]);
            }
        }
    }
    let solid: Vec<bool> = (0..out.w * out.h).map(|i| out.get(i % out.w, i / out.w).is_some()).collect();
    for y in 0..out.h {
        for x in 0..out.w {
            let at = |dx: i32, dy: i32| {
                let (nx, ny) = (x + dx, y + dy);
                nx >= 0 && ny >= 0 && nx < out.w && ny < out.h && solid[(ny * out.w + nx) as usize]
            };
            if !at(0, 0) && (at(1, 0) || at(-1, 0) || at(0, 1) || at(0, -1)) {
                out.put(x, y, 'g');
            }
        }
    }
    out
}

/// A knight in stone: helm, cape and shield, the sword raised high.
fn generic_knight() -> Img {
    Img::from_rows(&[
        "........W.......",
        "........i.......",
        "........i.......",
        "........i.......",
        "........i.......",
        ".......hJh......",
        "......JhhhJ.....",
        ".....hiiiiih....",
        ".....hiKKKih....",
        ".....hiiiiih....",
        "....JJhhhhhJJ...",
        "...JhiiiiiiihJ..",
        "..JJhiiiiiiihhJ.",
        "..Jhhiiiihiih.J.",
        ".JhhJiiiiiiihJ..",
        ".JhhJhiiiiihJ...",
        ".JhhJhiiiiih....",
        "..JJ.hiihiih....",
        ".....hii.hii....",
        ".....hii.hii....",
        ".....hii.hii....",
        "....hhhh.hhhh...",
    ])
}

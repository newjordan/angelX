//! The end of the match: a crowd on its feet, three steps rising, the
//! winner crowned, fireworks and paper in pixels, the pot counting up.
//! All of it from the round's `ended` ticks, so every seat sees it alike.
use super::super::juice::{self, Place, Tone};
use super::super::pfx::ramps;
use super::moments::{FINALE, podium_open};
use super::{called, enlarge, knights, panel, toned};
use crate::render::overworld::ink::{Img, bayer, hash};
use crate::rules::deathmatch::{self as dm, Session};

/// One spectator, `arms` up or down, in body ink `body`.
fn spectator(body: char, arms: bool, cap: char) -> Img {
    let rows: &[&str] = if arms {
        &["a..cc..a", "a.cccc.a", "aa.cc.aa", ".abbbba.", "..bbbb..", "..bbbb..", "..bbbb.."]
    } else {
        &["........", "..cccc..", "..cccc..", "...cc...", ".bbbbbb.", "abbbbbba", "abbbbbba"]
    };
    let rows: Vec<String> = rows
        .iter()
        .map(|r| r.replace('a', &body.to_string()).replace('b', &body.to_string()).replace('c', &cap.to_string()))
        .collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    Img::from_rows(&refs)
}

/// Two tiers of spectators along the back wall, twice life size, cheering as the
/// reveal goes on: arms up in waves, bodies bobbing, a few with a warm cap.
fn crowd(im: &mut Img, age: u32, fever: f32) {
    for (tier, (base, ink, cap)) in [(108, 'g', 'j'), (126, 'j', 'G')].into_iter().enumerate() {
        let mut x = -((tier as i32) * 9);
        let mut n = 0;
        while x < im.w {
            let h = hash(n, tier as i32, 0xc7);
            let bob = if fever > 0.0 && (age / 4 + h % 5) % 2 == 0 { 2 } else { 0 };
            let up = fever > 0.3 && ((age + h % 11 * 2) / 5) % 3 != 0;
            let warm = h % 13 == 0;
            let sprite = enlarge(&spectator(ink, up, if warm { 'R' } else { cap }), 2.0);
            im.stamp(&sprite, x, base - 14 - bob);
            x += 17 + (h % 3) as i32;
            n += 1;
        }
    }
}

/// A beam of stage light down onto the winner: a dither, behind the steps.
fn spot(im: &mut Img, cx: i32, floor: i32, k: f32) {
    for y in 0..floor {
        let half = 22.0 + y as f32 * 0.5;
        for x in (cx - half as i32)..=(cx + half as i32) {
            let edge = 1.0 - (x - cx).abs() as f32 / half;
            if bayer(x, y) < edge * edge * 0.3 * k {
                im.put(x, y, 'J');
            }
        }
    }
}

/// The podium scene for a seat, `ended` ticks after the winning blow.
pub(super) fn scene(s: &Session, id: u32, ended: u32) -> Img {
    let age = ended.saturating_sub(FINALE);
    let mut im = Img::black(384, 224);
    // The floor of the stage: dark flags and a warm line at the steps' feet.
    for y in 110..224 {
        for x in 0..384 {
            let flag = ((x / 24) + (y - 110) / 12) % 2 == 0;
            if bayer(x, y) < 0.16 + if flag { 0.14 } else { 0.0 } {
                im.put(x, y, 'g');
            }
        }
    }
    let ranks = s.order();
    let winner = ranks.first().copied().unwrap_or(0);
    let cx = 192;
    let floor = 182;
    // Fireworks over the crowd, three launchers in turn.
    if age >= 8 {
        for (j, (x, top, ramp)) in [(72.0, 58.0, ramps::GOLD), (192.0, 36.0, ramps::EMBER), (312.0, 62.0, ramps::BONE)].into_iter().enumerate() {
            let t = age as i32 - 8 - j as i32 * 17;
            if t >= 0 {
                let round = (t / 52) as u32;
                juice::firework(&mut im, x + (round as f32 * 23.0) % 40.0 - 20.0, 112.0, top, (t % 52) as u32, 31 * j as u32 + round * 7, ramp, age as u64);
            }
        }
    }
    let fever = ((age as f32 - 14.0) / 20.0).clamp(0.0, 1.0);
    crowd(&mut im, age, fever);
    spot(&mut im, cx, floor, ((age as f32 - 6.0) / 12.0).clamp(0.0, 1.0));
    // The three on the steps.
    let places: Vec<Place> = ranks
        .iter()
        .take(3)
        .map(|&r| {
            let f = &s.fighters[&r];
            Place {
                sprite: toned(&knights::figure(&f.hero, r)),
                name: called(s, r, id, 9),
                line: format!("{} KILLS", f.kills),
            }
        })
        .collect();
    let head = juice::podium(&mut im, (0, 0, 384, floor + 6), &places, age);
    // The laurel: a warm wreath above the winner once they have stood a moment.
    if age >= 34 {
        let k = (((age - 34) as f32) / 10.0).min(1.0);
        let bob = ((age as f32 * 0.2).sin() * 1.5) as i32;
        let (x, y) = (head.0, head.1 - 8 + ((1.0 - k) * -20.0) as i32 + bob);
        for a in 0..11 {
            let t = a as f32 / 10.0;
            let (dx, dy) = ((t * 2.0 - 1.0) * 22.0, -(1.0 - (t * 2.0 - 1.0).powi(2)) * 9.0 + 6.0);
            im.rect(x + dx as i32 - 2, y + dy as i32, 5, 3, if a % 2 == 0 { 't' } else { 'O' });
            im.rect(x + dx as i32 - 1, y + dy as i32, 2, 1, 'T');
        }
        im.rect(x - 2, y - 10, 5, 6, '@');
        im.rect(x - 4, y - 7, 9, 3, '5');
    }
    // Rivals below the podium, small and plain.
    if ranks.len() > 3 && age >= 24 {
        panel(&mut im, 4, 46, 104, 8 + (ranks.len() as i32 - 3) * 10);
    }
    for (k, &r) in ranks.iter().enumerate().skip(3) {
        let line = format!("{} {} {}", k + 1, called(s, r, id, 8), s.fighters[&r].kills);
        let mut t = juice::big_text_img(&line, 1, if r == id { 'T' } else { 'h' }, 'k');
        juice::dissolve(&mut t, 1.0 - ((age as f32 - 24.0 - k as f32 * 3.0) / 6.0).clamp(0.0, 1.0));
        im.stamp(&t, 8, 49 + (k as i32 - 3) * 10);
    }
    if age >= 14 {
        juice::confetti(&mut im, (0, 0, 384, 224), age, 9, 170);
    }
    // The winner's name, big, off the top.
    if age >= 6 {
        let title = if winner == id { "YOU WIN".to_string() } else { format!("{} WINS", called(s, winner, id, 12)) };
        juice::banner(&mut im, 8, &title, "", age - 6, 400, Tone::Warm, 3);
    }
    // The pot, counting up, and the laurel.
    if age >= 24 {
        let n = (s.purse as f32 * crate::motion::ease::out(((age - 24) as f32 / 40.0).min(1.0))) as u32;
        let pot = juice::big_text_img(&format!("+{n} GOLD"), 2, 'T', 'B');
        im.stamp(&pot, 192 - pot.w / 2, 205);
        let pulse = juice::bump(age.saturating_sub(64), 8);
        if pulse > 0.0 {
            im.rect(192 + pot.w / 2 + 6, 209, 3, 3, '@');
        }
    }
    if age >= 30 {
        let (best, who) = s.fighters.keys().map(|&f| (s.show.best(f), f)).max().unwrap_or((0, 0));
        if best >= 3 {
            let line = format!("BEST STREAK {} {}", best, called(s, who, id, 8));
            let t = juice::big_text_img(&line, 1, 'o', 'k');
            im.stamp(&t, 8, 212);
        }
        let mine = if winner == id { "YOUR LAUREL" } else { "THEIR LAUREL" };
        let t = juice::big_text_img(mine, 1, if winner == id { 'T' } else { 'h' }, 'k');
        im.stamp(&t, 376 - t.w, 212);
    }
    // The way home: a thin bar filling along the bottom edge.
    let filled = (384 * ended.min(dm::PODIUM) / dm::PODIUM) as i32;
    for x in 0..filled {
        im.put(x, 223, 'o');
    }
    // The iris opening from the finale's black.
    juice::iris(&mut im, (192, 100), 1.0 - podium_open(ended), 'k');
    im
}

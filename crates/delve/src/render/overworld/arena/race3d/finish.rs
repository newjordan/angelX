//! The end of the race: the last frame held (the line, the paper, the
//! winner's name), an iris closing on it, then the podium: the field's best
//! three on rising steps with the winner laurelled, bunting, fireworks and
//! paper in pixels, the pot counting up. All of it from the ticks since the
//! race was done, so every seat sees it alike.
use super::super::juice::{self, Place, Tone};
use super::super::pfx::ramps;
use super::super::knights as kn;
use super::super::sprites;
use super::super::enlarge;
use crate::render::overworld::ink::{Img, bayer, hash};
use crate::rules::race3d::{self as r3, Race3d, TITLE};
use crate::rules::{HZ, Run};

/// Ticks the last frame is held before the iris closes.
pub(super) const FREEZE: u32 = 40;
/// Ticks the iris takes to close, and to open on the podium.
const IRIS: u32 = 12;
/// Where the podium's own clock starts, in ticks after the race was done.
pub(super) const OPENS: u32 = FREEZE + IRIS;

/// Ticks since the race was done, or `None` while it is on.
pub(super) fn ended(race: &Race3d) -> Option<u32> {
    race.done.then(|| race.t.saturating_sub(race.ended))
}

/// Whether the podium has the whole screen.
pub(super) fn on_podium(race: &Race3d) -> bool {
    ended(race).is_some_and(|a| a >= OPENS)
}

use super::moments::called;

/// The headline of a finish for the seat looking: `YOU WIN` for the knight
/// who won, `<NAME> WINS` for everyone else.
pub(super) fn headline(run: &Run, winner: u32, seat: Option<u32>) -> String {
    if Some(winner) == seat {
        "YOU WIN".to_string()
    } else {
        format!("{} WINS", called(run, winner, seat, 9))
    }
}

/// The field in order, each as the seat looking calls them: `YOU` is the
/// viewer's own place and no other.
pub(super) fn standings(run: &Run, race: &Race3d, seat: Option<u32>, take: usize) -> Vec<(u32, String)> {
    race.order().into_iter().map(|id| (id, called(run, id, seat, take))).collect()
}

/// Over the held frame: the call and the iris closing on it.
pub(super) fn frozen(over: &mut Img, run: &Run, race: &Race3d, seat: Option<u32>, age: u32) {
    let order = race.order();
    let winner = order.first().copied();
    let all_home = race.riders.values().all(|r| r.finish.is_some());
    let title = if all_home { "ALL HOME" } else { "TIME UP" };
    let sub = match winner.filter(|_| !race.practice) {
        Some(w) if race.riders.len() > 1 => {
            headline(run, w, seat)
        }
        _ => String::new(),
    };
    juice::banner(over, 34, title, &sub, age, FREEZE + 8, Tone::Warm, 3);
    if age >= FREEZE {
        juice::iris(over, (192, 100), ((age - FREEZE) as f32 / IRIS as f32).min(1.0), 'k');
    }
}

/// A figure of a knight to stand on a step.
fn figure(run: &Run, id: u32) -> Img {
    run.players.get(&id).map_or_else(|| sprites::knight(id), |h| kn::portrait(h, id))
}

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

/// Two tiers of spectators along the back wall, twice life size, on their
/// feet as the podium goes up: arms up in waves, bodies bobbing, a few in
/// a warm cap.
fn crowd(im: &mut Img, age: u32, fever: f32) {
    for (tier, (base, ink, cap)) in [(104, 'g', 'j'), (122, 'j', 'G')].into_iter().enumerate() {
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

/// A string of pennants along the top of the stage, in the seats' cloths.
fn bunting(im: &mut Img, age: u32) {
    let ink = ['O', 'z', 'R', 'M', 'i', 'o'];
    let mut x = -4;
    let mut n = 0usize;
    while x < im.w {
        let sag = ((x as f32 / im.w as f32 * std::f32::consts::PI).sin() * 7.0) as i32;
        let sway = if (age / 6 + n as u32) % 2 == 0 { 0 } else { 1 };
        im.rect(x, sag + 1, 13, 1, 'g');
        for row in 0..8 {
            let half = (8 - row) / 2;
            im.rect(x + 6 - half + sway, sag + 2 + row, half * 2 + 1, 1, ink[n % ink.len()]);
        }
        x += 15;
        n += 1;
    }
}

/// The wreath above the winner's head.
fn laurel(im: &mut Img, (x, y): (i32, i32), age: u32) {
    let k = ((age as f32) / 10.0).min(1.0);
    let bob = ((age as f32 * 0.2).sin() * 1.5) as i32;
    let y = y - 8 + ((1.0 - k) * -20.0) as i32 + bob;
    for a in 0..11 {
        let t = a as f32 / 10.0;
        let (dx, dy) = ((t * 2.0 - 1.0) * 22.0, -(1.0 - (t * 2.0 - 1.0).powi(2)) * 9.0 + 6.0);
        im.rect(x + dx as i32 - 2, y + dy as i32, 5, 3, if a % 2 == 0 { 't' } else { 'O' });
        im.rect(x + dx as i32 - 1, y + dy as i32, 2, 1, 'T');
    }
    im.rect(x - 2, y - 10, 5, 6, '@');
    im.rect(x - 4, y - 7, 9, 3, '5');
}

/// The podium scene for a seat, `age` ticks after the iris has closed.
pub(super) fn podium(run: &Run, race: &Race3d, seat: Option<u32>, age: u32) -> Img {
    let mut im = Img::black(384, 224);
    // The stage's floor: dark flags, as the deathmatch's.
    for y in 110..224 {
        for x in 0..384 {
            let flag = ((x / 24) + (y - 110) / 12) % 2 == 0;
            if bayer(x, y) < 0.16 + if flag { 0.14 } else { 0.0 } {
                im.put(x, y, 'g');
            }
        }
    }
    let order = race.order();
    let winner = order.first().copied().unwrap_or(0);
    let (cx, floor) = (192, 182);
    // Fireworks over the stage, three launchers in turn.
    if age >= 6 {
        for (j, (x, top, ramp)) in [(72.0, 52.0, ramps::GOLD), (192.0, 34.0, ramps::EMBER), (312.0, 58.0, ramps::BONE)].into_iter().enumerate() {
            let t = age as i32 - 6 - j as i32 * 17;
            if t >= 0 {
                let round = (t / 52) as u32;
                juice::firework(&mut im, x + (round as f32 * 23.0) % 40.0 - 20.0, 112.0, top, (t % 52) as u32, 31 * j as u32 + round * 7, ramp, u64::from(age));
            }
        }
    }
    crowd(&mut im, age, ((age as f32 - 12.0) / 20.0).clamp(0.0, 1.0));
    bunting(&mut im, age);
    // The beam of light onto the winner.
    let k = ((age as f32 - 4.0) / 12.0).clamp(0.0, 1.0);
    for y in 14..floor {
        let half = 22.0 + y as f32 * 0.5;
        for x in (cx - half as i32)..=(cx + half as i32) {
            let edge = 1.0 - (x - cx).abs() as f32 / half;
            if bayer(x, y) < edge * edge * 0.28 * k {
                im.put(x, y, 'J');
            }
        }
    }
    let places: Vec<Place> = standings(run, race, seat, 9)
        .into_iter()
        .take(3)
        .map(|(id, name)| {
            let r = &race.riders[&id];
            let time = r.finish.map_or_else(|| "OUT".to_string(), |f| format!("{}S", (f - TITLE).div_ceil(HZ)));
            Place { sprite: figure(run, id), name, line: time }
        })
        .collect();
    let head = juice::podium(&mut im, (0, 0, 384, floor + 6), &places, age);
    if age >= 30 && race.riders.len() > 1 && !race.practice && race.riders[&winner].finish.is_some() {
        laurel(&mut im, head, age - 30);
    }
    // The rest of the field, small and plain.
    if order.len() > 3 && age >= 24 {
        im.rect(4, 50, 112, 8 + (order.len() as i32 - 3) * 10, 'K');
        im.frame(4, 50, 112, 8 + (order.len() as i32 - 3) * 10, 'g');
    }
    for (k, (id, name)) in standings(run, race, seat, 8).into_iter().enumerate().skip(3) {
        let r = &race.riders[&id];
        let time = r.finish.map_or_else(|| "OUT".to_string(), |f| format!("{}S", (f - TITLE).div_ceil(HZ)));
        let line = format!("{} {} {}", k + 1, name, time);
        let mut t = juice::big_text_img(&line, 1, if Some(id) == seat { 'T' } else { 'h' }, 'k');
        juice::dissolve(&mut t, 1.0 - ((age as f32 - 24.0 - k as f32 * 3.0) / 6.0).clamp(0.0, 1.0));
        im.stamp(&t, 9, 54 + (k as i32 - 3) * 10);
    }
    if age >= 14 {
        juice::confetti(&mut im, (0, 0, 384, 224), age, 9, 150);
    }
    // The winner, big, off the top.
    if age >= 4 {
        let title = if race.practice { "FLIGHT DONE".to_string() } else { headline(run, winner, seat) };
        let sub = if race.practice { race.course.board() } else { race.course.name() };
        juice::banner(&mut im, 14, &title, sub, age - 4, 500, Tone::Warm, 3);
    }
    // What this seat takes home, counting up.
    if let Some(me) = seat.filter(|id| race.riders.contains_key(id)) {
        if age >= 22 && !race.practice {
            let gold = race.won.get(&me).copied().unwrap_or(0);
            let n = (gold as f32 * crate::motion::ease::out(((age - 22) as f32 / 36.0).min(1.0))) as u32;
            let line = juice::big_text_img(&format!("+{n} GOLD"), 2, 'T', 'B');
            im.stamp(&line, 192 - line.w / 2, 204);
            let haul = race.riders[&me].haul;
            if haul > 0 {
                let t = juice::big_text_img(&format!("HAUL {haul}"), 1, 'o', 'k');
                im.stamp(&t, 8, 212);
            }
            if winner == me && race.riders.len() > 1 {
                let t = juice::big_text_img("YOUR LAUREL", 1, 'T', 'k');
                im.stamp(&t, 376 - t.w, 212);
            }
        }
    }
    // The way home: a thin bar filling along the bottom edge.
    let since = OPENS + age;
    let filled = (384 * since.min(r3::PODIUM) / r3::PODIUM) as i32;
    for x in 0..filled {
        im.put(x, 223, 'o');
    }
    // The iris opening from black.
    juice::iris(&mut im, (192, 100), 1.0 - (age as f32 / IRIS as f32).min(1.0), 'k');
    im
}

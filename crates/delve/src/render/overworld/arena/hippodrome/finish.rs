//! The end of the race: the line held (the paper, the winner's name), an iris closing on it, then
//! the podium of the three best teams on rising steps under bunting and fireworks, the winner
//! laurelled, the rest in a plain ledger, the fastest lap on a plate. All of it is read from the
//! ticks since the race was done, so every seat sees it alike.
use super::super::super::ink::{Img, bayer, hash};
use super::super::juice::{self, Place, Tone};
use super::super::pfx::ramps;
use super::super::{enlarge, knights as kn, sprites};
use super::art;
use super::moments::{self, Plan, Rect, Word};
use super::view::View;
use crate::rules::chariot::{GATE_TICKS, Phase};
use crate::rules::HZ;

pub(super) const FREEZE: u32 = 40;
const IRIS: u32 = 12;
pub(super) const OPENS: u32 = FREEZE + IRIS;
const AREA: Rect = (0, 0, 384, 188);

pub(super) fn ended(v: &View) -> Option<u32> {
    (v.race.phase == Phase::Done).then_some(v.race.t)
}

pub(super) fn on_podium(v: &View) -> bool {
    ended(v).is_some_and(|a| a >= OPENS)
}

fn time_of(v: &View, i: usize) -> String {
    let r = &v.race.racers[i];
    let ticks = (r.line - GATE_TICKS as f32).max(0.0) as u32;
    format!("{}.{}S", ticks / HZ, ticks % HZ * 10 / HZ)
}

fn figure(v: &View, i: usize) -> Img {
    let team = v.race.racers[i].team;
    match v.seat_of(team).and_then(|s| v.run.players.get(&s).map(|h| (s, h))) {
        Some((seat, hero)) => kn::portrait(hero, seat),
        None => sprites::knight_in(&kn::realm_colours(u32::from(team) + 1)),
    }
}

pub(super) fn headline(v: &View, winner: usize) -> (String, Option<u8>) {
    let team = v.race.racers[winner].team;
    if v.mine && winner == v.me {
        ("YOU WIN".into(), Some(team))
    } else {
        (format!("{} WINS", v.called(team, 9)), Some(team))
    }
}

pub(super) fn frozen_banner(v: &View, age: u32) -> Option<moments::Banner> {
    let order = v.race.order();
    let winner = *order.first()?;
    let (title, who) = headline(v, winner);
    let sub = if v.race.seats().len() > 1 && v.race.racers[winner].place == Some(1) { "THE LAUREL" } else { "" };
    moments::banner_of(title, sub.into(), Tone::Warm, age, FREEZE + 8, 34, 3, 9, who)
}

fn places(v: &View) -> Vec<Place> {
    v.race
        .order()
        .into_iter()
        .take(3)
        .map(|i| Place { sprite: figure(v, i), name: v.called(v.race.racers[i].team, 13), line: time_of(v, i) })
        .collect()
}

fn ledger(v: &View) -> Vec<Word> {
    let mut out = Vec::new();
    for (k, i) in v.race.order().into_iter().enumerate().skip(3) {
        let r = &v.race.racers[i];
        let text = format!("{} {} {}", k + 1, v.called(r.team, 8), if r.place.is_some() { time_of(v, i) } else { "OUT".into() });
        out.push(Word { text, at: (9, 58 + (k as i32 - 3) * 11), ink: 'h', kind: "ledger", who: Some(r.team) });
    }
    let best = v.race.racers.iter().enumerate().filter(|(_, r)| r.best > 0).min_by_key(|(_, r)| r.best);
    if let Some((i, r)) = best {
        let who = v.called(r.team, 9);
        let text = format!("{} {}.{}S", who, r.best / HZ, r.best % HZ * 10 / HZ);
        out.push(Word { text: "FASTEST LAP".into(), at: (9, 206), ink: 't', kind: "fastest", who: None });
        out.push(Word { text, at: (9, 215), ink: v.ink(i), kind: "ledger", who: Some(r.team) });
    }
    out
}

pub(super) fn plan(v: &View, age: u32) -> Plan {
    let mut p = Plan::default();
    p.words = ledger(v);
    if let Some(winner) = v.race.order().first().copied() {
        let (title, who) = headline(v, winner);
        p.banner = moments::banner_of(title, String::new(), Tone::Warm, age.min(40), 100_000, 14, 2, 9, who);
    }
    let ps = places(v);
    for (stand, i) in juice::podium_stands((384, 224), AREA, &ps).into_iter().zip(v.race.order()) {
        let team = v.race.racers[i].team;
        p.podium.push(("podium name", stand.name_text.clone(), stand.name, Some(team)));
        p.podium.push(("podium line", stand.line_text.clone(), stand.line, None));
        p.plates.push(("podium rank", stand.rank));
    }
    p
}

pub(super) fn iris(over: &mut Img, age: u32) {
    if age >= FREEZE {
        juice::iris(over, (192, 100), ((age - FREEZE) as f32 / IRIS as f32).min(1.0), 'k');
    }
}

pub(super) fn paper(layer: &mut Img, at: (i32, i32), age: u32) {
    if age >= 70 {
        return;
    }
    const PAPER: [char; 5] = ['5', 'z', 'A', '7', 'o'];
    for k in 0..28u32 {
        let h = hash(k as i32, 3, 0x7a);
        let delay = h % 8;
        if age < delay {
            continue;
        }
        let t = (age - delay) as f32;
        let spread = ((h >> 4) % 100) as f32 / 100.0 - 0.5;
        let up = 2.6 + ((h >> 9) % 30) as f32 / 12.0;
        let x = at.0 as f32 + spread * t * 3.4 + (t * 0.35).sin() * 3.0;
        let y = at.1 as f32 - up * t + 0.07 * t * t * 6.0;
        layer.rect(x as i32, y as i32, 2, 2, PAPER[(h >> 14) as usize % PAPER.len()]);
    }
}

fn bunting(im: &mut Img, age: u32, cloths: &[char]) {
    let mut x = -4;
    let mut n = 0usize;
    while x < im.w {
        let sag = ((x as f32 / im.w as f32 * std::f32::consts::PI).sin() * 7.0) as i32;
        let sway = if (age / 6 + n as u32) % 2 == 0 { 0 } else { 1 };
        im.rect(x, sag + 1, 13, 1, 'g');
        for row in 0..8 {
            let half = (8 - row) / 2;
            im.rect(x + 6 - half + sway, sag + 2 + row, half * 2 + 1, 1, cloths[n % cloths.len()]);
        }
        x += 15;
        n += 1;
    }
}

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

fn crowd(im: &mut Img, age: u32, fever: f32) {
    for (tier, (base, coat)) in [(106, 'g'), (124, 'j')].into_iter().enumerate() {
        let mut x = -((tier as i32) * 9);
        let mut n = 0;
        while x < im.w {
            let h = hash(n, tier as i32, 0xc7);
            let bob = if fever > 0.0 && (age / 4 + h % 5) % 2 == 0 { 2 } else { 0 };
            let up = fever > 0.3 && ((age + h % 11 * 2) / 5) % 3 != 0;
            let warm = h % 13 == 0;
            let fig = art::spectator(if warm { 'R' } else { coat }, 'o', if warm { 'O' } else { 'n' }, if up { 2 } else { 0 });
            im.stamp(&enlarge(&fig, 2.0), x, base - 14 - bob);
            x += 17 + (h % 3) as i32;
            n += 1;
        }
    }
}

pub(super) fn scene(v: &View, age: u32) -> Img {
    let mut im = Img::black(384, 224);
    for y in 110..224 {
        for x in 0..384 {
            let flag = ((x / 24) + (y - 110) / 12) % 2 == 0;
            if bayer(x, y) < 0.16 + if flag { 0.14 } else { 0.0 } {
                im.put(x, y, 'g');
            }
        }
    }
    let (cx, floor) = (192, 182);
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
    let cloths: Vec<char> = v.race.racers.iter().map(|r| art::livery(&r.house).cloth).collect();
    bunting(&mut im, age, &cloths);
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
    let ps = places(v);
    let head = juice::podium(&mut im, AREA, &ps, age);
    if age >= 30 && v.race.racers[v.race.order()[0]].place == Some(1) {
        laurel(&mut im, head, age - 30);
        if age % 3 == 0 {
            juice::confetti(&mut im, (60, 20, 264, 150), age, 7, 18);
        }
    }
    let p = plan(v, age);
    if let Some(b) = &p.banner {
        moments::draw_banner(&mut im, b);
    }
    for w in &p.words {
        moments::draw_word(&mut im, w);
    }
    im
}

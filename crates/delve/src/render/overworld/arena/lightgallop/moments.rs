//! The calls and the flashes of the ride: the title that slams in, a banner
//! for a loop closed, a chain, a lantern lit and the warden down, a spark's
//! "+1" flying to the counter, a red rim when a knight is hit, and the dark
//! rim of speed. All of it is read off the session's records by their age.
use super::super::juice::{self, Tone};
use super::{Img, View};
use crate::rules::juice::Weight;
use crate::rules::lightgallop::{Session, level::level, show::Kind};

/// Ticks a banner stands.
const CALL_LIFE: u32 = 62;
/// The title card's.
pub(super) const TITLE_LIFE: u32 = 78;
/// Where the spark counter sits: what the flying "+1" lands on.
pub(super) const COUNTER: (i32, i32) = (46, 17);

/// A call: what it says, the line under it, its ink and how old it is.
pub(super) struct Call {
    pub title: String,
    pub sub: String,
    pub tone: Tone,
    pub age: u32,
    pub life: u32,
}

/// The call standing for `me`: the loudest of what happened lately, the
/// newest first among equals, or the title card as the ride begins.
pub(super) fn call(s: &Session, me: u32) -> Option<Call> {
    let lvl = level();
    let mut best: Option<(u8, u32, Call)> = None;
    let mut offer = |rank: u8, age: u32, c: Call| {
        if best.as_ref().is_none_or(|(r, a, _)| rank > *r || (rank == *r && age < *a)) {
            best = Some((rank, age, c));
        }
    };
    for f in &s.show.fx {
        let age = f.age(s.elapsed);
        let mine = u32::from(f.seat) == me;
        let make = |title: &str, sub: &str, tone: Tone| Call { title: title.into(), sub: sub.into(), tone, age, life: CALL_LIFE };
        match f.kind {
            Kind::WardenDown if age < CALL_LIFE => offer(9, age, make("WARDEN DOWN", "THE BELL IS OPEN", Tone::Warm)),
            Kind::LoopOut if mine && age < CALL_LIFE => offer(6, age, make("LOOP", "ROUND THE WHEEL", Tone::Warm)),
            Kind::Chain if mine && age < CALL_LIFE => offer(5 + (f.n as u8).min(3), age, make(&format!("CHAIN {}", f.n), "", Tone::Warm)),
            Kind::Lantern if age < CALL_LIFE => {
                offer(4, age, make("LANTERN LIT", &format!("{} OF {}", f.n + 1, lvl.posts.len()), Tone::Warm));
            }
            Kind::Fall if mine && age < 40 => offer(8, age, make("DOWN", "THE LANTERN LIFTS YOU", Tone::Hot)),
            _ => {}
        }
    }
    if s.elapsed < TITLE_LIFE {
        offer(2, s.elapsed, Call { title: "MOONWELL ROAD".into(), sub: "THE DIMMING COMES".into(), tone: Tone::Warm, age: s.elapsed, life: TITLE_LIFE });
    }
    best.map(|(_, _, c)| c)
}

/// The call, a banner in the sky under the heads-up display.
pub(super) fn banner(im: &mut Img, c: &Call) {
    juice::banner(im, 40, &c.title, &c.sub, c.age, c.life, c.tone, 3);
}

/// How long ago `me` last took a spark or popped a foe, if lately.
pub(super) fn last_gain(s: &Session, me: u32) -> Option<u32> {
    s.show
        .fx
        .iter()
        .filter(|f| u32::from(f.seat) == me && matches!(f.kind, Kind::Spark | Kind::Pop))
        .map(|f| f.age(s.elapsed))
        .filter(|&a| a < 18)
        .min()
}

/// Each spark and each foe `me` just took flies to the counter as a "+1".
pub(super) fn fly_ups(im: &mut Img, s: &Session, v: &View, me: u32) {
    let mut shown = 0;
    for f in s.show.fx.iter().rev() {
        let age = f.age(s.elapsed);
        if u32::from(f.seat) != me || !matches!(f.kind, Kind::Spark | Kind::Pop) || age >= 16 {
            continue;
        }
        // One flying at a time is a flourish; more is a crowd.
        if shown >= 1 {
            continue;
        }
        shown += 1;
        let from = (v.sx(f.x).clamp(8, 376), v.sy(f.y).clamp(34, 210));
        juice::fly_up(im, "+1", from, COUNTER, age, 16, '6');
    }
}

/// A red rim for a blow on `me`, and the dark rim of speed.
pub(super) fn rims(im: &mut Img, s: &Session, me: u32) {
    let Some(seat) = s.seats.get(&me) else { return };
    let area = (0, 31, 384, 193);
    for f in s.show.fx.iter().filter(|f| u32::from(f.seat) == me && matches!(f.kind, Kind::Hit | Kind::Fall)) {
        let age = f.age(s.elapsed);
        let life = if f.kind == Kind::Fall { 22 } else { 12 };
        if age == 0 && f.weight >= Weight::Kill {
            // A blow that big pales the whole picture for a tick, through a thin dither.
            for y in 31..224 {
                for x in 0..384 {
                    if crate::render::overworld::ink::bayer(x, y) < 0.2 {
                        im.put(x, y, 'V');
                    }
                }
            }
        }
        if age < life {
            rim(im, area, 1.0 - age as f32 / life as f32, if f.weight >= Weight::Kill { 16 } else { 11 }, '8');
        }
    }
    let sp = seat.rider.speed();
    if sp > 1500 && seat.rider.down == 0 {
        rim(im, area, ((sp - 1500) as f32 / 1800.0).clamp(0.0, 0.45), 6, 'k');
    }
}

/// The kit's `vignette` (a dithered rim of `fill` along the edges of a box, `k`
/// how strong, `depth` pixels deep), visiting only the rim's own pixels: the
/// rim of speed is drawn every frame at pace, and the kit's walks the whole box.
fn rim(im: &mut Img, (bx, by, bw, bh): (i32, i32, i32, i32), k: f32, depth: i32, fill: char) {
    if k <= 0.0 {
        return;
    }
    for y in by..by + bh {
        let dy = (y - by).min(by + bh - 1 - y);
        let mut row = |x: i32| {
            let edge = (x - bx).min(bx + bw - 1 - x).min(dy);
            if edge < depth {
                let near = 1.0 - edge as f32 / depth as f32;
                if crate::render::overworld::ink::bayer(x, y) < near * near * k {
                    im.put(x, y, fill);
                }
            }
        };
        if dy < depth {
            (bx..bx + bw).for_each(&mut row);
        } else {
            (bx..bx + depth).for_each(&mut row);
            (bx + bw - depth..bx + bw).for_each(&mut row);
        }
    }
}

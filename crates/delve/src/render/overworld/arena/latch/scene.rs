//! Latch Duel's screen, as a seat plays it: the bay (the smithy, the knight, Clasp and every spark
//! between them) on a layer that shakes and zooms with the blows, and over it, still, the bands of
//! words, the chain's pips, Clasp's bar, every knight's card and the moments. It reads the duel
//! and its marks and keeps nothing.
use super::super::super::ink::Img;
use super::super::juice;
use super::super::{NATIVE_H, NATIVE_W};
use super::{art, clasp_body, finish, fx, hud, knight_img, knight_pose, moments, room};
use crate::rules::Run;
use crate::rules::juice::Weight;
use crate::rules::latch::{Beat, Session};
use crate::rules::latch_show::{Kind, PERFECT};

/// Both layers of seat `focus`'s screen (or the first seat's): the picture, and an empty one over it.
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let s = run.latch.as_ref().expect("Latch showing");
    let me = focus.filter(|id| s.duels.contains_key(id));
    let id = me.or_else(|| s.duels.keys().next().copied()).unwrap();
    let ended = if s.left == 0 { (run.tick as u32).saturating_sub(s.done) } else { 0 };
    let hold = finish::held(s);
    let picture = if s.left == 0 && ended >= hold + finish::WIPE {
        finish::scene(run, s, id, me, ended - hold - finish::WIPE)
    } else {
        let mut im = play(run, s, id, me, ended);
        if s.left == 0 && ended >= hold {
            juice::iris(&mut im, (192, 112), (ended - hold) as f32 / finish::WIPE as f32, 'k');
        }
        im
    };
    (picture, Img::new(NATIVE_W, NATIVE_H))
}

/// The age of a mark for its picture: the realm's hold on the winning blow stops the clock but not
/// the burst, so a finale's age carries on through the hold.
fn felt(run: &Run, age: u32) -> u32 {
    let hold = run.tune.hitstop_boss_kill;
    if run.tune.hitstop == 0 { age } else { age + hold.saturating_sub(run.hitstop.left.min(hold)) }
}

/// A counter on show: how it weighed and how long ago.
struct Hit {
    weight: Weight,
    count: u8,
    age: u32,
}

fn hit_of(run: &Run, s: &Session, id: u32, clock: u32) -> Option<Hit> {
    let (m, age) = hud::latest_counter(s, id, clock)?;
    let Kind::Counter { count, .. } = m.kind else { return None };
    let age = if count >= 3 { felt(run, age) } else { age };
    (age < 56).then(|| Hit { weight: m.kind.weight(), count, age })
}

/// Seat `id`'s screen while the round is on, or in its last breath (`ended` ticks after it).
fn play(run: &Run, s: &Session, id: u32, me: Option<u32>, ended: u32) -> Img {
    let d = &s.duels[&id];
    let clock = s.elapsed(run.tick);
    let tick = run.tick;
    let a = art();
    let slot = super::super::slot_art::enabled();
    let slot_room = slot && super::super::slot_art::bytes("latch.room", "room").is_some();
    let lane = if d.height < 0 { 120 } else { 144 };
    let stop = run.tune.latch_stop;
    let mut world = room(false).clone();
    fx::embers(&mut world, tick, slot_room);
    if slot_room {
        fx::anvil(&mut world, tick);
    }
    let calm = crate::render::viewer::calm();
    // Breathing in the breath between arms; the arm shivering as the closing beat nears.
    let bob = i32::from(d.beat == Beat::Rest && !calm && (tick / 16) % 2 == 1);
    let shiver = if d.beat == Beat::Tell && !calm && d.ticks > run.tune.latch_window && d.ticks <= run.tune.latch_window + 10 { (tick / 2 % 2) as i32 * 2 - 1 } else { 0 };
    let hit = hit_of(run, s, id, clock);
    let miss = hud::latest_miss(s, id, clock).filter(|&(_, age)| age < 30);

    // Clasp: white where the blow lands, flung back along it, squashed and sprung home.
    let body = clasp_body(run, d, true);
    let white;
    let squashed;
    let mut figure = body;
    let mut at = (217 + shiver, 56 + bob);
    if let Some(h) = &hit {
        let since = h.age.saturating_sub(stop);
        let push = (juice::lurch(h.weight, since) * 28.0) as i32;
        at.0 += push;
        if h.age < h.weight.flash() {
            white = fx::flash_frame(body);
            figure = &white;
        }
        if h.count < 3 && since < 24 {
            let (img, _) = juice::squash_stretch(figure, since, h.weight.squash());
            at = (267 + push - img.w / 2, 156 - img.h);
            squashed = img;
            figure = &squashed;
        }
    }
    world.stamp(figure, at.0, at.1);

    // The knight, and the painted clash at the fist (the art the room already carries).
    let jitter = miss.filter(|&(_, age)| age < 12).map_or(0, |(_, age)| {
        let back = (7.0 * crate::motion::math::exp(-(age as f32) * 0.28)) as i32;
        -back + if age < 6 && age % 2 == 0 { 2 } else { 0 }
    });
    if let Some(fig) = knight_img(run, id, knight_pose(run, d), false) {
        world.stamp(&fig, 126 + jitter, 76 + bob + if d.beat == Beat::Miss { 3 } else { 0 });
    }
    if d.beat == Beat::Clean {
        let age = run.tune.latch_recover.saturating_sub(d.ticks);
        let frame = ((age * 4) / run.tune.latch_recover).min(3) as usize;
        let cx = at.0 + 15;
        world.stamp(&a.parry[frame], cx - 16, 114);
        world.stamp(&(if slot { &a.slot_strike } else { &a.strike })[frame], cx - 20, 114);
        world.stamp(&a.clash[frame], cx - 16, 114);
    }
    if d.beat == Beat::Miss {
        world.rect(192, lane, 25, 2, 'o');
        world.stamp(&a.clash[3], 147, lane - 16);
    }
    // The tell: the lane filling, the ring closing, and which way to hold your guard.
    if d.beat == Beat::Tell {
        fx::tell(&mut world, lane, d.ticks, d.windup, run.tune.latch_window, tick);
        if d.guard != d.height {
            fx::guard_hint(&mut world, d.height < 0, tick);
        }
    }
    for m in s.show.marks.iter().filter(|m| m.who == id) {
        let age = m.age(clock);
        let seed = m.born.wrapping_mul(0x9e37_79b1) ^ id.wrapping_mul(0x85eb_ca77);
        match m.kind {
            Kind::Counter { chain, count, error, .. } => {
                let t = if count >= 3 { felt(run, age) } else { age };
                if t < 36 {
                    fx::counter(&mut world, m.kind.weight(), chain, error <= PERFECT, t, seed, tick);
                }
                if count >= 3 && t < 58 {
                    fx::plates(&mut world, &a.states[2][5], (217, 56), t, seed);
                    fx::fell(&mut world, t, seed, tick);
                }
            }
            Kind::Miss { .. } => {
                if age < 14 {
                    fx::miss(&mut world, lane, age, seed, tick);
                }
            }
        }
    }
    // The winning blow's zoom, in hard and out slowly.
    if let Some(h) = hit.as_ref().filter(|h| h.count >= 3 && h.age < 36) {
        let z = fx::zoom(h.age, 0);
        if z > 1.01 {
            world = fx::punch(&world, (234, 118), z);
        }
    }

    // The shake: the strongest blow on the table, the heaviest of them being yours.
    let phase = tick + u64::from(run.hitstop.left);
    let blows = s
        .show
        .marks
        .iter()
        .filter(|m| matches!(m.kind, Kind::Counter { .. }) && (m.who == id || matches!(m.kind, Kind::Counter { count: 3, .. })))
        .map(|m| {
            let age = m.age(clock);
            (m.kind.weight(), if matches!(m.kind, Kind::Counter { count: 3, .. }) { felt(run, age) } else { age })
        });
    let (mut dx, mut dy) = if calm { (0, 0) } else { juice::shake(blows, phase) };
    if let (Some((_, age)), false, (0, 0)) = (miss.filter(|&(_, age)| age < 6), calm, (dx, dy)) {
        (dx, dy) = crate::motion::shake(tick, 6 - age, 6, 1.6);
    }
    let mut out = Img::black(NATIVE_W, NATIVE_H);
    out.stamp(&world, dx, dy);
    // The black bands the words stand on, over whatever the shake showed past the room.
    out.rect(0, 0, 384, 33, 'k');
    out.rect(0, 174, 384, 50, 'k');
    // A heavy counter lights the stage a beat in warm paper; a fold of the guard rims it in blood; the
    // last five seconds rim it red.
    if let (Some(h), false) = (&hit, calm) {
        if h.weight >= Weight::Crushing && h.age < 4 {
            juice::vignette(&mut out, (0, 33, 384, 155), 0.55 * (1.0 - h.age as f32 / 4.0), 40, 'o');
        }
    }
    if let (Some((_, age)), false) = (miss.filter(|&(_, age)| age < 10), calm) {
        juice::vignette(&mut out, (0, 33, 384, 155), 0.9 * (1.0 - age as f32 / 10.0), 14, 'R');
    }
    let rush = moments::hurry(s, tick);
    if rush > 0.0 && !calm {
        juice::vignette(&mut out, (0, 33, 384, 155), rush, 10, 'R');
    }
    hud::paint_top(&mut out, run, s, id, clock, tick);
    if let Some((m, age)) = hud::latest_counter(s, id, clock) {
        if let Kind::Counter { count, .. } = m.kind {
            let to = (hud::PIP.0 as f32 + (f32::from(count) - 1.0) * hud::PIP.1 as f32, hud::PIP.2 as f32);
            fx::carry(&mut out, (234.0, 126.0), to, age, hud::FLY);
        }
    }
    hud::paint_cards(&mut out, run, s, id, clock, tick);
    for w in hud::words(run, s, id, me, clock) {
        out.text(w.x, w.y, &w.text, w.ink);
    }
    moments::paint(&mut out, run, s, id, me, clock, ended);
    out
}

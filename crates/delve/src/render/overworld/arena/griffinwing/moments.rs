//! The calls and the flying numbers of the flight: a beat's title that slams
//! in, a banner for a volley, a chain, a wingman saved, a lamp out, the Wyrm
//! down and the beacon lit; a "+20" that flies up from each foe a knight
//! drops to the score, which then bumps; and a red rim for a blow taken.
//! All of it is read off the session's records by their age, and every plate
//! of words it puts up is a box the layout keeps the rest of the words off
//! (`boxes`), so nothing of it lies on anything else.
use super::super::juice::{self, Tone};
use super::hud::{self, Ctx};
use super::layout::{Box2, H, W};
use crate::motion::ease;
use crate::render::overworld::ink::{Img, text_width};
use crate::rules::griffinwing::show::{What, kind_n};
use crate::rules::griffinwing::{Kind, Mode, rail};
use crate::rules::juice::Weight;

/// Ticks a banner stands.
const CALL_LIFE: u32 = 64;
/// Ticks a number takes to fly to the score.
pub(super) const FLY_LIFE: u32 = 18;
/// Where a flying number lands: left of the score's plate, right of the
/// beat's name, so it never lies on either.
pub(super) const LAND: (i32, i32) = (279, 17);
/// The rows a banner may stand on, the first free of every other plate.
const SLOTS: [i32; 6] = [42, 68, 94, 122, 150, 174];
/// Ticks after the Wyrm's last lamp before its fall is spoken.
const WYRM_WORDS: u32 = 26;
/// When the beacon takes its light in the way home, ticks into the flight.
pub(super) const LIGHT_AT: u32 = 110;
/// How long after the flame takes that it is called: the comets have landed,
/// the beacon has lit and the light has flooded the canyon first.
const LIT_WORDS: u32 = 18;

/// A call: what it says, the line under it, its ink and how old it is.
pub(super) struct Call {
    pub title: String,
    pub sub: String,
    pub tone: Tone,
    pub age: u32,
    pub life: u32,
    /// The biggest scale its words may rest at (3 the loud ones, 2 the small).
    pub cap: i32,
}

/// The call standing for the viewer: the loudest of what happened lately, the
/// newest first among equals, or the beat's title as it opens.
pub(super) fn call(c: &Ctx) -> Option<Call> {
    let s = c.s;
    let (now, me) = (s.elapsed, c.seat as u8);
    let mut best: Option<(u8, u32, Call)> = None;
    let mut offer = |rank: u8, age: u32, title: String, sub: &str, tone: Tone, life: u32| {
        if age < life && best.as_ref().is_none_or(|(r, a, _)| rank > *r || (rank == *r && age < *a)) {
            // The loud calls (a beat's name, a lamp, the Wyrm, the beacon)
            // are big; a volley, a chain, a save are small, off the action.
            let cap = if rank >= 7 || rank == 2 { 3 } else { 2 };
            best = Some((
                rank,
                age,
                Call {
                    title,
                    sub: sub.into(),
                    tone,
                    age,
                    life,
                    cap,
                },
            ));
        }
    };
    let name = |id: u8| hud::who(c.run, id).0;
    for f in &s.show.fx {
        let age = f.age(now);
        let mine = f.seat == me;
        match f.what {
            // The blasts first: the words come when the Wyrm is going down.
            What::WyrmDown if age >= WYRM_WORDS => {
                offer(12, age - WYRM_WORDS, "THE WYRM FALLS".into(), "TO THE BEACON", Tone::Warm, 70)
            }
            What::LampOut => offer(
                9,
                age,
                "LAMP OUT".into(),
                &format!("{} OF 3", f.n),
                Tone::Warm,
                CALL_LIFE,
            ),
            What::Rescue if mine => offer(8, age, format!("SAVED {}", name(f.n as u8)), "", Tone::Warm, CALL_LIFE),
            What::Rescue => offer(5, age, format!("{} SAVED", name(f.n as u8)), "", Tone::Steel, 44),
            What::Beat => offer(
                7,
                age,
                "BEAT CLEAR".into(),
                rail::NAMES[usize::from(s.stage.min(2))],
                Tone::Warm,
                48,
            ),
            What::Starfall if mine => offer(6, age, "STARFALL".into(), "", Tone::Warm, 40),
            What::Volley if mine && f.n >= 3 => offer(6, age, format!("VOLLEY x{}", f.n), "", Tone::Warm, 50),
            What::Chain if mine && f.n >= 5 => offer(
                4 + (f.n as u8 / 5).min(2),
                age,
                format!("CHAIN {}", f.n),
                "",
                Tone::Warm,
                52,
            ),
            What::Fall if !mine && f.seat != 0 => offer(5, age, format!("{} DOWN", name(f.seat)), "", Tone::Steel, 40),
            What::Tell => {
                let (title, sub) = match f.n {
                    1 => ("COILS", "FIND THE GAP"),
                    2 => ("ROAR", "LEAVE THE LIGHT"),
                    _ => ("SPIT", "ROLL INTO IT"),
                };
                offer(3, age, title.into(), sub, Tone::Hot, 36);
            }
            _ => {}
        }
    }
    if s.mode == Mode::Victory && s.mode_t >= LIGHT_AT + LIT_WORDS {
        offer(11, s.mode_t - LIGHT_AT - LIT_WORDS, "BEACON LIT".into(), "THE PASS IS OURS", Tone::Warm, 100);
    }
    // A beat's own title, as it opens (the first ninety ticks of the beat).
    if s.mode == Mode::Play && s.t < 90 && !(s.stage == 2 && s.t >= rail::BOSS_AT) {
        let stage = usize::from(s.stage.min(2));
        offer(2, s.t, rail::NAMES[stage].into(), rail::WORDS[stage], Tone::Warm, 90);
    }
    best.map(|(_, _, c)| c)
}

/// The box a banner takes at row `y`, `age` ticks in: its rails, its words
/// (at the size it slams to) and the line under, a dot round.
pub(super) fn call_box(call: &Call, y: i32) -> Box2 {
    let tw = text_width(&call.title).max(1);
    let rest = ((W - 24) / tw).clamp(1, call.cap);
    let slam = (3 - call.age as i32).max(0);
    let scale = (rest + slam).min(176 / tw).max(rest);
    let top = y - (scale - rest) * 3;
    let words = Box2::new(W / 2 - (tw * scale + 4) / 2, top, tw * scale + 4, 7 * scale + 4);
    // The rails run out a little past the words, above and below.
    let rails = Box2::new(W / 2 - (tw * scale + 4) / 2 - 8, y - 6, tw * scale + 4 + 16, 7 * rest + 17);
    let mut b = words.union(&rails);
    if !call.sub.is_empty() {
        let sw = text_width(&call.sub) + 4;
        b = b.union(&Box2::new(W / 2 - sw / 2, y + 7 * rest + 12, sw, 11));
    }
    b.grow(1)
}

/// A number flying to the score.
pub(super) struct Flyer {
    pub text: String,
    pub from: (i32, i32),
    pub age: u32,
    pub plate: Box2,
}

/// Where a number that began at `from` is `age` ticks on: up and over, an
/// arc that peaks above the line between the two, easing in and out.
fn path(from: (i32, i32), age: u32) -> (i32, i32) {
    let t = ease::both((age as f32 / FLY_LIFE as f32).min(1.0));
    let lift = (t * (1.0 - t) * 4.0 * 22.0) as i32;
    (
        from.0 + ((LAND.0 - from.0) as f32 * t) as i32,
        from.1 + ((LAND.1 - from.1) as f32 * t) as i32 - lift,
    )
}

/// Where the banner stands: the first row whose banner lies on none of the
/// plates in `base`; failing that, none (a call that has no room is left
/// out, never laid on a word).
pub(super) fn banner(c: &Ctx, base: &[Box2]) -> Option<(Call, i32, Box2)> {
    let call = call(c)?;
    for y in SLOTS {
        let b = call_box(&call, y);
        if b.within(&Box2::new(0, 0, W, H)) && !base.iter().any(|t| t.hits(&b)) {
            return Some((call, y, b));
        }
    }
    None
}

/// The numbers in flight this tick, each laid clear of everything in `base`
/// and of the ones before it.
pub(super) fn flyers(c: &Ctx, base: &[Box2]) -> Vec<Flyer> {
    let mut taken: Vec<Box2> = base.to_vec();
    let mut out = Vec::new();
    let (now, me) = (c.s.elapsed, c.seat as u8);
    let mut gains: Vec<_> = c
        .s
        .show
        .fx
        .iter()
        .filter(|f| f.what == What::Pop && f.seat == me && f.age(now) < FLY_LIFE)
        .collect();
    gains.sort_by_key(|f| f.born);
    for f in gains.into_iter().rev().take(3) {
        let rescue = c
            .s
            .show
            .fx
            .iter()
            .any(|r| r.what == What::Rescue && r.born == f.born && (r.s, r.x, r.z) == (f.s, f.x, f.z));
        let kind = match f.n & 15 {
            0 => Kind::Wisp,
            1 => Kind::Kite,
            2 => Kind::Turret,
            3 => Kind::Bomber,
            _ => Kind::Lamp,
        };
        debug_assert_eq!(kind_n(kind), f.n & 15);
        let chain = u32::from(f.n >> 4);
        let mut value = kind.worth() + if rescue { 150 } else { 0 };
        if chain >= 3 {
            value += 10 * (chain - 2);
        }
        let text = format!("+{value}");
        let Some((sx, sy, _)) = c.at(f.s, f.x, f.z) else {
            continue;
        };
        let from = (
            (sx.round() as i32).clamp(16, W - 16),
            (sy.round() as i32).clamp(46, H - 24),
        );
        let age = f.age(now);
        let at = path(from, age);
        let (w, h) = (text_width(&text) + 4, 11);
        let plate = Box2::new(at.0 - w / 2, at.1 - h / 2, w, h);
        if plate.within(&Box2::new(0, 0, W, H)) && !taken.iter().any(|t| t.hits(&plate)) {
            taken.push(plate);
            out.push(Flyer {
                text,
                from,
                age,
                plate,
            });
        }
    }
    out
}

/// The numbers in flight, by name, for the layout to keep the rest off and
/// the tests to hold to it (`base` is everything they keep off).
pub(super) fn fly_boxes(c: &Ctx, base: &[Box2]) -> Vec<(&'static str, Box2)> {
    flyers(c, base).into_iter().map(|f| ("fly", f.plate)).collect()
}

/// How far the score's number has swelled (0 to 1): a number just landed.
pub(super) fn bump(c: &Ctx) -> f32 {
    let (now, me) = (c.s.elapsed, c.seat as u8);
    c.s.show
        .fx
        .iter()
        .filter(|f| f.what == What::Pop && f.seat == me)
        .map(|f| f.age(now))
        .filter(|&a| (FLY_LIFE - 1..FLY_LIFE + 7).contains(&a))
        .map(|a| juice::bump(a - (FLY_LIFE - 1), 8))
        .fold(0.0, f32::max)
}

/// The banner, and the numbers in flight (`base` is what they keep off).
pub(super) fn paint(im: &mut Img, c: &Ctx, plates: &[Box2], base: &[Box2]) {
    if let Some((call, y, _)) = banner(c, plates) {
        juice::banner(im, y, &call.title, &call.sub, call.age, call.life, call.tone, call.cap);
    }
    for f in flyers(c, base) {
        let t = f.age as f32 / FLY_LIFE as f32;
        let mut text = juice::big_text_img(&f.text, 1, if t < 0.5 { '6' } else { 'w' }, 'B');
        if f.age + 2 >= FLY_LIFE {
            juice::dissolve(&mut text, 0.5);
        }
        im.stamp(&text, f.plate.x, f.plate.y);
        // A few sparks trail it.
        for k in 1..4u32 {
            let (x, y) = path(f.from, f.age.saturating_sub(k));
            im.put(x, y + 5, ['@', 'o', 'R'][k as usize - 1]);
        }
    }
}

/// A red rim for a blow taken or a fall, and the white of a bomb or the
/// Wyrm's end.
pub(super) fn rims(im: &mut Img, c: &Ctx) {
    let (now, me) = (c.s.elapsed, c.seat as u8);
    for f in &c.s.show.fx {
        let age = f.age(now);
        if f.seat == me && matches!(f.what, What::Hurt | What::Fall) {
            let life = if f.what == What::Fall { 24 } else { 12 };
            if age < life {
                let k = 1.0 - age as f32 / life as f32;
                let heavy = f.weight >= Weight::Crushing;
                juice::vignette(im, (0, 0, W, H), k * if heavy { 1.0 } else { 0.8 }, if heavy { 18 } else { 12 }, '7');
            }
        }
        if f.what == What::Starfall && f.seat == me && age < 6 {
            let k = (1.0 - age as f32 / 6.0) * 0.6;
            juice::vignette(im, (0, 0, W, H), k, 30, 'W');
        }
    }
}

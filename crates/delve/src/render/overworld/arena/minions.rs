//! The Gravecaller's dead and his rites, drawn: his ghouls, wardens and
//! totems and a tombstone's zombies (each climbing out of the ground when
//! raised, a pillar of grave-fire over the spot), the Tombstone itself
//! slamming down and cracking as it is beaten, a soul rip's torn souls
//! streaming back to him, a warden's roar, and the mark over the foe a
//! summoner's minions go for. Grave-fire is the foliage's pale greens;
//! the mark and the roar, which are the game's state, are signal inks.
//! What his wishes for his dead put on them (`dead_wishes`) is drawn on
//! each, fitted to its body: the pieces they wear, and a colour wished
//! over their whole bodies in the wish's own inks.

use super::super::ink::{Img, Rgb, hash, ink, shade};
use super::knights::facing;
use super::paintings::{self, Painting};
use super::{UNIT, at, stand};
use crate::rules::hireling::{AllyKind, Hireling, ROAR_TICKS};
use crate::rules::looks::{Body, Sprite};
use crate::rules::minions::{AURA_REACH, AURA_SHOWN, CAST_SHOWN, Order, RIP_REACH, RISE, Rite, TOMB_HEALTH, TOMB_TICKS, Tombstone};
use crate::rules::{Run, dead_wishes};

/// A ghoul: hunched and grave-pale, long in the arm, bone at the claws.
pub(super) const GHOUL: [&str; 14] = [
    "......eee.....",
    ".....eCCCe....",
    "....eCkCkCe...",
    "....eCYCYCe...",
    "....eCCCCCe...",
    ".eee.eiei.....",
    "eCCeeCCCCe....",
    "eCe.eCCeCCe...",
    "eCe.eCCCeCCe..",
    "hCe.eCCCe.eCe.",
    "h.h.jjGjj.eCe.",
    "....jGjGj.hCh.",
    "....eC.eC.h.h.",
    "...eCC.eCC....",
];

/// A barrow warden: an old knight in grey plate gone to rust, a green
/// light in his visor, and his tall shield with a skull on its face.
pub(super) const WARDEN: [&str; 19] = [
    "....gjjjg..........",
    "...gGJJGjg.........",
    "...gJhJJGg.........",
    "...gYYgggg.........",
    "...gGJJGjg.........",
    "....gggg...xxSSxx..",
    "..gjGJJGjgxSuUUuSx.",
    ".gGJhJJGjgSuUUUUuS.",
    ".gJhJJGGjSuUhiihUuS",
    ".gGJJGGjgSUhkiikhUS",
    ".gGJJGGjgSUhiiiihUS",
    "..gjGGjggSUUhkkhUUS",
    "..gBjjBgxSuUhihhUuS",
    "..gjGGjgxSuUUUUUuS.",
    "..gGJGjg.xSuUUUuSx.",
    "..gjGjjg..xSuuuSx..",
    "..gj..gj...xSuSx...",
    "..jG..jG....xSx....",
    ".nnn..nnn..........",
];

/// A bone totem: two skulls on a stake with grave-light in their eyes, a
/// third at its foot, and grave-fire burning on top.
pub(super) const TOTEM: [&str; 21] = [
    "...e.C...",
    "..eCYCe..",
    "...CYC...",
    "..hiiih..",
    ".hiHiiih.",
    ".iKYiKYi.",
    ".hiikiih.",
    "..hjjjh..",
    "..hiiih..",
    ".hiHiiih.",
    ".iKYiKYi.",
    ".hiikiih.",
    "..hjjjh..",
    "...IBI...",
    "..hiih...",
    "..ikki...",
    "...IBI...",
    "...IpI...",
    "...BpB...",
    "..bIBIb..",
    ".bbbbbbb.",
];

/// A zombie in grey rags, its arms out before it.
pub(super) const ZOMBIE: [&str; 15] = [
    "....eeee......",
    "...eCyyCe.....",
    "...eYkYke.....",
    "...eCyyCe.....",
    "....ekke......",
    "...jGGGGj.....",
    "..jGJGGJGjeCCy",
    "..jGGJGGGjeeCC",
    "..jGPGGPGj....",
    "..jGGPGGGjeCCy",
    "..jGPGGJGjeeCC",
    "...jGjjGj.....",
    "...PB..PB.....",
    "...eC..eC.....",
    "..eeC.eeC.....",
];

const GOLEM: [&str; 22] = [
    "........gggg........",
    ".......gJhhJg.......",
    ".......gYkkYg.......",
    ".......gJhhJg.......",
    "........gggg........",
    "...gggggJhhJggggg...",
    "..gJhhJhhJJhhJhhJg..",
    ".gJhJgJhhhhhhJgJhJg.",
    ".gJhJgJhJJJJhJgJhJg.",
    ".gJhJgJhhJYJhhgJhJg.",
    ".gJhJgJhJJJJhJgJhJg.",
    ".gJhJgJhhhhhhJgJhJg.",
    ".gJhJggJhhhhJggJhJg.",
    ".gJhJg.gJJJJg.gJhJg.",
    ".gJJJg.gJhhJg.gJJJg.",
    "..ggg..gJhhJg..ggg..",
    "......gJhggJhJg.....",
    "......gJhgghhJg.....",
    "......gJhgghhJg.....",
    "......gJhgghhJg.....",
    ".....gJhhggJhhJg....",
    ".....ggggggggggg....",
];

/// Each golem tier's canvas, its body and its fit alike.
fn golem_size(kind: AllyKind) -> (i32, i32) {
    match kind {
        AllyKind::FleshGolem => (17, 20),
        AllyKind::BoneGolem => (22, 25),
        AllyKind::GraveGolem => (27, 30),
        _ => (34, 38),
    }
}

/// Nearest-neighbour resize: every enlarged texel stays crisp.
fn resized(im: &Img, w: i32, h: i32) -> Img {
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = im.get(x * im.w / w, y * im.h / h) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// The golems grow in breadth and weight with their tier. The dancers
/// share the room's beat: sidestep, raised claws, bob and shoulder shimmy.
pub(super) fn body(kind: AllyKind, tick: u64) -> Option<Img> {
    if kind.golem() {
        let (w, h) = golem_size(kind);
        let swaps: &[(char, char)] = match kind {
            AllyKind::FleshGolem => &[('g', 'B'), ('J', 'r'), ('h', 'o')],
            AllyKind::BoneGolem => &[('g', 'G'), ('J', 'i'), ('h', 'H')],
            AllyKind::GraveGolem => &[('g', 'x'), ('J', 'u'), ('h', 'V')],
            _ => &[('g', 'f'), ('J', 'e'), ('h', 'C')],
        };
        let mut im = resized(&Img::from_rows(&GOLEM).recolor(swaps), w, h);
        if kind == AllyKind::Colossus {
            im.rect(w / 2 - 2, 9, 4, 9, 'Y');
            im.rect(w / 2 - 1, 11, 2, 5, 'H');
        }
        return Some(im);
    }
    if kind != AllyKind::Skeleton {
        return None;
    }
    let beat = (tick / 6 % 8) as usize;
    let side = if beat < 4 { -1 } else { 1 };
    let sway = [-1, -2, -1, 0, 1, 2, 1, 0][beat];
    let bob = [0, 1, 0, -1, 0, 1, 0, -1][beat];
    let shimmy = if beat % 2 == 0 { -1 } else { 1 };
    let mut im = Img::new(24, 24);
    let cx = 12 + sway;
    let skull = Img::from_rows(&[".GiiiG.", "GiHHHiG", "GiKiKiG", ".iHkHi.", "..iii.."]);
    im.stamp(&skull, cx - 3 + shimmy, 2 + bob);
    im.rect(cx, 8, 1, 9, 'H');
    for row in [9, 11, 13] {
        im.rect(cx - 3, row, 7, 1, 'i');
        im.put(cx - 3, row + 1, 'G');
        im.put(cx + 3, row + 1, 'G');
    }
    im.rect(cx - 2, 16, 5, 2, 'i');
    for shoulder in [-3, 3] {
        let sx = cx + shoulder;
        let elbow = sx + side * 3;
        let sy = 9 + if shoulder < 0 { shimmy } else { -shimmy };
        im.rect(sx.min(elbow), sy, 4, 1, 'i');
        im.rect(elbow, sy - 4, 1, 5, 'H');
        for finger in [-1, 1] {
            im.put(elbow + finger, sy - 5, 'i');
            im.put(elbow + finger * 2, sy - 4, 'H');
        }
    }
    for leg in [-2, 2] {
        let lx = cx + leg;
        let step = if leg * side > 0 { 1 } else { 0 };
        im.rect(lx, 18, 1, 4 - step, 'H');
        im.rect(lx - 1 + side, 22 - step, 3, 1, 'i');
    }
    Some(im)
}

/// The Tombstone: a grey headstone with a cross cut in it, on a mound of
/// turned earth; the cut glows when it raises its dead.
const STONE: [&str; 24] = [
    ".......gggggggg.......",
    ".....ggJhhhhhhJgg.....",
    "....gJhHiiiiiiihJg....",
    "...gJhHiiiiiiiiihJg...",
    "..gJhHiiiiGGiiiiihJg..",
    "..gJhiiiiiGGiiiiihJg..",
    ".gJhiiiiiiGGiiiiiihJg.",
    ".gJhiiGGGGGGGGGGiihJg.",
    ".gJhiiGGGGGGGGGGiihJg.",
    ".gJhiiiiiiGGiiiiiihJg.",
    ".gJhiiiiiiGGiiiiiihJg.",
    ".gJhiiiiiiGGiiiiiihJg.",
    ".gJhiiiiiiGGiiiiiihJg.",
    ".gJhiiiiiiGGiiiiiihJg.",
    ".gJhiiiiiiGGiiiiiihJg.",
    ".gJhiiiiiiiiiiiiiihJg.",
    ".gJhihGhGhGhGhGhiihJg.",
    ".gJhiiiiiiiiiiiiiihJg.",
    ".gJhihGhGhGhGhiiiihJg.",
    ".gJhiiiiiiiiiiiiiihJg.",
    ".gGJhhhhhhhhhhhhhhJGg.",
    "bEggggggggggggggggggEb",
    "bFlEIbBbbIFlEIbbIFlEIb",
    ".bbnbbnnbbnbbnbbnnbbb.",
];

/// Where one of the dead keeps its body in its picture (drawn facing
/// right), for what his wishes put on it: its size beside a knight's
/// sixteen pixels; the middle of each place a worn piece goes
/// (`looks::PLACES`); and the inks of its body, dark to light, as shadow,
/// flesh and light, left of `reach`, which a colour wished over the whole
/// of it repaints. What is not its body (a warden's shield, a totem's
/// stake, rags, the grave-light in the eyes) keeps its own inks.
struct Fit {
    body: Body,
    shadow: &'static [char],
    flesh: &'static [char],
    light: &'static [char],
    reach: i32,
}

impl Fit {
    fn of(kind: AllyKind) -> Option<Fit> {
        Some(match kind {
            AllyKind::Ghoul => Fit {
                body: body_of(kind).unwrap(),
                shadow: &['e'],
                flesh: &['C'],
                light: &[],
                reach: i32::MAX,
            },
            AllyKind::Warden => Fit {
                body: body_of(kind).unwrap(),
                shadow: &['g', 'j'],
                flesh: &['G', 'J'],
                light: &['h'],
                // His shield, right of his plate, keeps its own.
                reach: 9,
            },
            AllyKind::Totem => Fit {
                body: body_of(kind).unwrap(),
                shadow: &['h'],
                flesh: &['i'],
                light: &['H'],
                reach: i32::MAX,
            },
            AllyKind::Zombie => Fit {
                body: body_of(kind).unwrap(),
                shadow: &['e'],
                flesh: &['C'],
                light: &['y'],
                reach: i32::MAX,
            },
            AllyKind::Skeleton => Fit {
                body: body_of(kind).unwrap(),
                shadow: &['G'],
                flesh: &['i'],
                light: &['H'],
                reach: i32::MAX,
            },
            kind if kind.golem() => {
                let (shadow, flesh, light): (&[char], &[char], &[char]) = match kind {
                    AllyKind::FleshGolem => (&['B'], &['r'], &['o']),
                    AllyKind::BoneGolem => (&['G'], &['i'], &['H']),
                    AllyKind::GraveGolem => (&['x'], &['u'], &['V']),
                    _ => (&['f'], &['e'], &['C']),
                };
                Fit {
                    body: body_of(kind).unwrap(),
                    shadow,
                    flesh,
                    light,
                    reach: i32::MAX,
                }
            }
            _ => return None,
        })
    }
}

/// Each minion owns its attachment positions, including the warden's
/// off-centre body beside his shield and the totem's narrow stake.
pub(super) fn body_of(kind: AllyKind) -> Option<Body> {
    let name = match kind {
        AllyKind::Ghoul => "ghoul", AllyKind::Warden => "warden", AllyKind::Totem => "totem",
        AllyKind::Zombie => "zombie", AllyKind::Skeleton => "skeleton",
        AllyKind::FleshGolem => "flesh_golem", AllyKind::BoneGolem => "bone_golem", AllyKind::GraveGolem => "grave_golem", AllyKind::Colossus => "colossus",
        _ => return None,
    };
    Some(crate::rules::looks::profile(name))
}

/// One of a summoner's dead as his wishes for them dress it
/// (`dead_wishes`): its body in a colour wished over the whole of it, and
/// the pieces his other wishes for them put on it, each fitted to its
/// body, oldest under newest; with the paintings of the pieces that have
/// them, in the dressed picture's pixels. The canvas grows for what stands
/// proud of the body at the sides and above; below its feet is cut. Any
/// other ally, or one he wished nothing on, as it is.
/// Skull and ribcage follow the same eight-drawing skeleton dance as `skeleton`.
pub(super) fn body_at(kind: AllyKind, tick: u64) -> Option<Body> {
    let mut fit = body_of(kind)?;
    if kind == AllyKind::Skeleton {
        let phase = (tick / 6) as usize % 8;
        let sway = [-1, -2, -1, 0, 1, 2, 1, 0][phase];
        let bob = [0, 1, 0, -1, 0, 1, 0, -1][phase];
        let shimmy = if phase % 2 == 0 { -1 } else { 1 };
        fit.head.anchor = (12 + sway + shimmy, 4 + bob);
        fit.face.anchor = (12 + sway + shimmy, 5 + bob);
        for part in [&mut fit.body, &mut fit.back, &mut fit.shoulders] { part.anchor.0 += sway; }
        let side = if phase < 4 { -1 } else { 1 };
        fit.left.anchor = (12 + sway - 3 + side * 3, 4 + shimmy);
        fit.right.anchor = (12 + sway + 3 + side * 3, 4 - shimmy);
        fit.feet.anchor.0 = 12 + sway + side;
        fit.aura.anchor.0 = 12 + sway;
    }
    Some(fit)
}

pub(super) fn dressed(run: &Run, hire: &Hireling, body: &Img) -> (Img, Vec<Painting>) {
    let plain = || (body.clone(), Vec::new());
    let (Some(mut fit), Some(id)) = (Fit::of(hire.kind), hire.summoner) else {
        return plain();
    };
    if let Some(body) = body_at(hire.kind, run.tick) { fit.body = body; }
    let Some(owner) = run.players.get(&id) else {
        return plain();
    };
    let worn = owner.dead_looks(&run.book);
    if worn.is_empty() {
        return plain();
    }
    let mut im = body.clone();
    if let Some(card) = worn.iter().rev().find(|c| dead_wishes::colours_body(c))
        && let Some(colour) = card.looks.worn.as_ref()
    {
        im = coloured(&im, &fit, colour, super::wear::dye_ramp(card).as_deref());
    }
    let pieces: Vec<&Sprite> = worn.iter().filter(|c| !dead_wishes::colours_body(c))
        .filter_map(|c| c.looks.worn.as_ref()).collect();
    super::knights::dressed_as(&im, fit.body, &pieces, run.tick,
        paintings::phase_of(&[u64::from(id), hire.kind as u64]))
}

/// `im` with its body repainted in the inks of `look`, a colour wished
/// over the whole of it: its flesh in the ink the look fills its inside
/// with most (its outline is not its colour), its shadow in the look's ink
/// nearest a shade under that (or, with none, a darker step of it on the
/// realm's plain inks), its light in the look's lightest. A look of one
/// ink still leaves the body its shape. A purple or pink wish drawn
/// without its dye (`wear::dye_ramp`) is that dye instead: its flesh the
/// dye's strongest colour, the rest of the ramp its shadow and light.
fn coloured(im: &Img, fit: &Fit, look: &Sprite, dye: Option<&[Rgb]>) -> Img {
    let mut counts: Vec<(Rgb, usize)> = Vec::new();
    let mut inside: Vec<(Rgb, usize)> = Vec::new();
    if let Some(ramp) = dye {
        let chroma = |c: &Rgb| c.iter().max().unwrap() - c.iter().min().unwrap();
        counts = ramp.iter().map(|&c| (c, 1)).collect();
        inside = ramp.iter().max_by_key(|c| chroma(c)).map(|&c| vec![(c, 1)]).unwrap_or_default();
    }
    let tally = |counts: &mut Vec<(Rgb, usize)>, rgb: Rgb| match counts.iter_mut().find(|(k, _)| *k == rgb) {
        Some((_, n)) => *n += 1,
        None => counts.push((rgb, 1)),
    };
    for frame in look.frames.iter().filter(|_| dye.is_none()) {
        let inked = |x: i32, y: i32| {
            y >= 0 && x >= 0 && frame.get(y as usize).and_then(|r| r.chars().nth(x as usize)).is_some_and(|c| look.rgb(c).is_some())
        };
        for (y, row) in frame.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                let Some(rgb) = look.rgb(c) else {
                    continue;
                };
                tally(&mut counts, rgb);
                let (x, y) = (x as i32, y as i32);
                if [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dy)| inked(x + dx, y + dy)) {
                    tally(&mut inside, rgb);
                }
            }
        }
    }
    let main = if inside.is_empty() { &counts } else { &inside };
    let Some(&(flesh, _)) = main.iter().max_by_key(|(_, n)| *n) else {
        return im.clone();
    };
    let lum = |[r, g, b]: Rgb| 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
    let body = lum(flesh).max(1.0);
    let shadow = counts
        .iter()
        .map(|&(c, _)| c)
        .filter(|&c| lum(c) < body && lum(c) >= body * 0.35)
        .min_by(|&a, &b| (lum(a) - body * 0.6).abs().total_cmp(&(lum(b) - body * 0.6).abs()))
        .unwrap_or_else(|| shade(flesh, 0.6));
    let light = counts
        .iter()
        .map(|&(c, _)| c)
        .filter(|&c| lum(c) > body)
        .max_by(|&a, &b| lum(a).total_cmp(&lum(b)))
        .unwrap_or(flesh);
    let swaps: Vec<(Rgb, Rgb)> = [(fit.shadow, shadow), (fit.flesh, flesh), (fit.light, light)]
        .into_iter()
        .flat_map(|(inks, to)| inks.iter().filter_map(move |&c| Some((ink(c)?, to))))
        .collect();
    let mut out = im.clone();
    for y in 0..out.h {
        for x in 0..out.w.min(fit.reach) {
            if let Some(p) = out.get(x, y)
                && let Some(&(_, to)) = swaps.iter().find(|(from, _)| *from == p)
            {
                out.set(x, y, to);
            }
        }
    }
    out
}

/// Whether knight `id` has just called, torn or planted: his staff is up.
pub(super) fn casting(run: &Run, id: u32) -> bool {
    run.minions
        .casts
        .get(&id)
        .is_some_and(|c| run.tick < c.tick + u64::from(CAST_SHOWN))
}

/// The graves, and every summoner's dead before them.
pub(super) fn delve(cv: &mut Img, run: &Run) {
    for stone in &run.minions.stones {
        grave(cv, run, stone);
    }
    for minion in &run.minions.raised {
        super::hireling::ally(cv, run, minion);
    }
}

/// A Tombstone: it drops into place over its first few ticks, its cut
/// glows as a zombie comes up and while it is fresh, cracks open as it is
/// beaten, and gutters in its last two seconds.
fn grave(cv: &mut Img, run: &Run, stone: &Tombstone) {
    let (x, y) = at(stone.x, stone.y);
    let fall = (6 - stone.age.min(6) as i32) * 6;
    let lit = stone.age < 20 || stone.raise + 8 >= 30 || (stone.left < 2 * 30 && (run.tick / 4) % 2 == 0);
    let mut im = Img::from_rows(&STONE);
    if lit {
        im = im.recolor(&[('G', 'C')]);
        for (cx, cy) in [(10, 7), (11, 7), (10, 8), (11, 8)] {
            im.put(cx, cy, 'Y');
        }
    }
    // Its wounds: one crack for each third of it beaten away.
    let lost = TOMB_HEALTH.saturating_sub(stone.hp) * 3 / TOMB_HEALTH.max(1);
    let cracks: [&[(i32, i32)]; 2] = [
        &[(5, 3), (5, 4), (6, 5), (5, 6), (6, 7), (7, 8), (6, 9), (7, 10)],
        &[(16, 10), (15, 11), (16, 12), (17, 13), (16, 14), (15, 15), (16, 16)],
    ];
    for crack in cracks.iter().take(lost as usize) {
        for &(cx, cy) in *crack {
            im.put(cx, cy, 'k');
        }
    }
    stand(cv, &im, x, y + 4 - fall);
}

/// A minion climbing out of the ground: only what is above it shows, and
/// clods of the turned earth thrown up about it.
pub(super) fn rising(cv: &mut Img, im: &Img, x: i32, y: i32, left: u32) {
    let gone = (RISE - left.min(RISE)) as i32;
    let up = (gone * im.h / RISE as i32).max(1);
    let shown = Img::from_view(im, 0, 0, im.w, up);
    stand(cv, &shown, x, y);
    for k in 0..8 {
        let side = if k % 2 == 0 { 1 } else { -1 };
        let dx = side * (3 + (k * 7 % 5) + gone / 2);
        let dy = -((gone * (6 - k % 3)) / 3) + gone * gone / 12;
        cv.rect(x + dx, y + dy, 2, 2, ['b', 'I', 'n'][(k % 3) as usize]);
    }
}

/// Under the figures: the broken earth where the dead come up, the ring at
/// the feet of each minion told to heel, the grave's ring and its slam,
/// and the ground a soul rip tears.
pub(super) fn under(cv: &mut Img, run: &Run) {
    let tick = run.tick;
    for pulse in run.minions.graves.values().filter_map(|g| g.pulse.as_ref()) {
        let age = tick.saturating_sub(pulse.tick);
        if age < u64::from(AURA_SHOWN) {
            let centre = at(pulse.x, pulse.y);
            let r = AURA_REACH * UNIT * (age as f32 + 1.0) / AURA_SHOWN as f32;
            ring(cv, centre, r, r * 0.6, if age < 12 { 'Y' } else { 'e' }, 1);
            ring(cv, centre, (r - 2.0).max(1.0), (r * 0.6 - 1.0).max(1.0), 'C', 2);
        }
    }
    for minion in run.minions.raised.iter().filter(|m| !m.down()) {
        let (x, y) = at(minion.x, minion.y);
        if minion.rising > 0 {
            // The earth broken open, dark, with grave-light in the cracks.
            for dy in -3..=3 {
                let half = 9 - dy * dy;
                for dx in -half..=half {
                    let crack = hash(dx, dy, 7) % 5 == 0;
                    cv.put(x + dx, y + dy, if crack { 'C' } else if (dx + dy) % 3 == 0 { 'b' } else { 'n' });
                }
            }
            continue;
        }
        let heel = minion.summoner.is_some_and(|id| run.minions.order(id) == Order::Heel)
            && minion.kind != AllyKind::Zombie;
        if heel {
            ring(cv, (x, y + 1), 8.0, 3.0, 'U', 1);
            ring(cv, (x, y + 1), 7.0, 2.4, 'u', 2);
        }
    }
    for stone in &run.minions.stones {
        let (x, y) = at(stone.x, stone.y);
        // Where its dead come up: a ring of grave-light on the ground.
        ring(cv, (x, y + 2), 1.6 * UNIT, 1.3 * UNIT * 0.6, 'e', 2);
        if stone.age < 16 {
            // The slam: a shock of dust thrown out, and clods.
            let r = 6.0 + stone.age as f32 * 2.6;
            let ink = if stone.age < 8 { 'i' } else { 'J' };
            ring(cv, (x, y + 2), r, r * 0.45, ink, 1);
            ring(cv, (x, y + 2), r + 1.5, r * 0.45 + 1.0, 'h', 2);
            for k in 0..10 {
                let a = k as f32 * 0.63;
                let d = r * 0.8;
                cv.rect(x + (a.cos() * d) as i32, y + 2 + (a.sin() * d * 0.45) as i32 - stone.age as i32 / 3, 2, 2, 'b');
            }
        }
    }
    for cast in run.minions.casts.values().filter(|c| c.rite == Rite::Rip) {
        let age = tick.saturating_sub(cast.tick) as f32;
        let (x, y) = at(cast.x, cast.y);
        let r = RIP_REACH * UNIT * (0.6 + age / CAST_SHOWN as f32 * 0.4);
        ring(cv, (x, y), r, r * 0.6, 'C', 1);
        ring(cv, (x, y), r + 1.0, r * 0.6 + 1.0, 'e', 1);
        ring(cv, (x, y), r * 0.6, r * 0.36, 'e', 2);
        // The ground torn toward its heart.
        for k in 0..12 {
            let a = k as f32 * std::f32::consts::TAU / 12.0;
            for d in [0.35, 0.5, 0.65] {
                cv.put(x + (a.cos() * r * d) as i32, y + (a.sin() * r * 0.6 * d) as i32, 'n');
            }
        }
    }
}

/// A soul torn out of a foe: a pale wisp with two dark eyes.
const SOUL: [&str; 7] = [
    "..WW..",
    ".WYYW.",
    "WYkYkW",
    "WYYYYW",
    ".YCYC.",
    ".C.C.C",
    "C.C...",
];

/// Over everything: the summoners' marks on their quarry, a warden's roar,
/// the grave-fire over the dead coming up, a soul rip's souls streaming
/// home, and the flare of a staff raised in a rite.
pub(super) fn over(cv: &mut Img, run: &Run) {
    let tick = run.tick;
    // The mark: four corners closing on the foe a knight's minions go for.
    for foe_id in run.minions.quarry.values() {
        let Some(foe) = run.enemies.iter().find(|e| e.id == *foe_id && e.hp > 0) else {
            continue;
        };
        let (x, y) = at(foe.x, foe.y);
        let half = (foe.radius() * UNIT) as i32 + 4 + ((tick / 6) % 2) as i32;
        let top = y - half * 2;
        for (sx, sy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let (cx, cy) = (x + sx * half, top + half + sy * half);
            cv.put(cx, cy, '3');
            for k in 1..4 {
                cv.put(cx - k * sx, cy, if k == 1 { '3' } else { '2' });
                cv.put(cx, cy - k * sy, if k == 1 { '3' } else { '2' });
            }
        }
    }
    for minion in run.minions.raised.iter().filter(|m| !m.down()) {
        let (x, y) = at(minion.x, minion.y);
        if minion.rising > 0 {
            // A pillar of grave-fire over the spot as the dead come up,
            // white at its heart, tapering as it climbs.
            let gone = (RISE - minion.rising.min(RISE)) as i32;
            let h = 18 + gone;
            for k in 0..h {
                let wave = ((k as u64 + tick) / 2 % 3) as i32 - 1;
                let half = (3 - k * 3 / h).max(0);
                for dx in -half..=half {
                    let ink = match (dx.abs(), k * 3 / h) {
                        (0, 0) => 'W',
                        (0, _) | (1, 0) => 'Y',
                        (1, _) | (2, 0) => 'C',
                        _ => 'e',
                    };
                    cv.put(x + wave + dx, y - k, ink);
                }
            }
            // Sparks going up off it.
            for k in 0..5 {
                let rise = ((tick as i32 + k * 7) % 20) as i32;
                let dx = (hash(k, 3, 9) % 9) as i32 - 4;
                cv.put(x + dx, y - 6 - rise * 2, if k % 2 == 0 { 'Y' } else { 'C' });
            }
            continue;
        }
        if minion.kind == AllyKind::Warden && minion.taunt > 0 {
            // The roar: rings going out from him as it begins, and his
            // shout over his head while it holds.
            let gone = ROAR_TICKS - minion.taunt;
            if gone < 14 {
                let r = 6.0 + gone as f32 * 3.0;
                ring(cv, (x, y - 8), r, r * 0.7, '7', 1);
                ring(cv, (x, y - 8), r - 3.0, (r - 3.0) * 0.7, '7', 3);
            }
            let top = y - 24;
            for (dx, dy) in [(-4, 1), (-3, 0), (-2, -1), (0, -2), (0, -3), (0, -4), (2, -1), (3, 0), (4, 1)] {
                cv.rect(x + dx, top + dy, 1, 1, '7');
            }
        }
    }
    for (&id, cast) in &run.minions.casts {
        let age = tick.saturating_sub(cast.tick) as i32;
        let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0) else {
            continue;
        };
        let (hx, hy) = at(hero.x, hero.y);
        let face = facing(hero.aim_x, hero.aim_y);
        let (mut sx, sy) = super::class_looks::gravecaller::floating_staff_head(face, true, (hero.aim_x, hero.aim_y));
        if hero.aim_x < -0.3 {
            sx = -sx;
        }
        let head = (hx + sx, hy + sy);
        // The flare at the headstone: a white heart and rays that grow
        // and go out.
        let reach = 4 + age.min(8);
        cv.rect(head.0 - 1, head.1 - 1, 3, 3, 'W');
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4 + age as f32 * 0.15;
            for r in 2..reach {
                let ink = if r < 4 { 'Y' } else if r < reach - 2 { 'C' } else { 'e' };
                cv.put(head.0 + (a.cos() * r as f32) as i32, head.1 + (a.sin() * r as f32) as i32, ink);
            }
        }
        ring(cv, head, reach as f32 + 2.0, reach as f32 + 2.0, 'e', 2);
        if cast.rite == Rite::Rip {
            // Each torn soul flies home to the staff, its trail behind it.
            let t = (age as f32 / CAST_SHOWN as f32).min(1.0);
            for (n, &(fx, fy)) in cast.torn.iter().enumerate() {
                let from = at(fx, fy);
                let from = (from.0, from.1 - 14);
                let along = |t: f32| {
                    let wobble = ((t * 7.0 + n as f32).sin() * 4.0 * (1.0 - t)) as i32;
                    (
                        from.0 + ((head.0 - from.0) as f32 * t) as i32,
                        from.1 + ((head.1 - from.1) as f32 * t) as i32 + wobble,
                    )
                };
                for k in 1..8 {
                    let (px, py) = along((t - k as f32 * 0.05).max(0.0));
                    cv.rect(px, py, 2, 2, if k < 3 { 'Y' } else if k < 6 { 'C' } else { 'e' });
                }
                let (px, py) = along(t);
                let soul = Img::from_rows(&SOUL);
                cv.stamp(&soul, px - soul.w / 2, py - soul.h / 2);
            }
        }
        if cast.rite == Rite::Tomb && age < 10 {
            // A bolt of grave-fire from the staff to where the stone lands.
            let to = at(cast.x, cast.y);
            for k in 0..24 {
                let t = k as f32 / 24.0;
                let px = head.0 + ((to.0 - head.0) as f32 * t) as i32;
                let py = head.1 + ((to.1 - 12 - head.1) as f32 * t) as i32;
                cv.rect(px, py, 2, 2, if (k + age) % 3 == 0 { 'Y' } else { 'C' });
            }
        }
    }
    for stone in &run.minions.stones {
        // Under the stone, a bar of its time left.
        let (x, y) = at(stone.x, stone.y);
        let w = 20 * stone.left as i32 / TOMB_TICKS as i32;
        cv.rect(x - 10, y + 7, 20, 2, 'x');
        cv.rect(x - 10, y + 7, w, 2, '2');
    }
}

/// A dotted ellipse of `ink`, every `every`th dot of its rim.
fn ring(cv: &mut Img, (cx, cy): (i32, i32), rx: f32, ry: f32, ink: char, every: i32) {
    let n = ((rx + ry) * 3.0).max(12.0) as i32;
    for k in (0..n).step_by(every.max(1) as usize) {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        cv.put(cx + (a.cos() * rx) as i32, cy + (a.sin() * ry) as i32, ink);
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__minions__shots.rs"]
mod shots;

#[cfg(test)]
#[path = "../../../../tests/render/arena__dead_wishes__shots.rs"]
mod dead_shots;

#[cfg(test)]
#[path = "../../../../tests/render/arena__gravecaller__shots.rs"]
mod gravecaller_shots;

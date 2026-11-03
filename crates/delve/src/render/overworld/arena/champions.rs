//! The champions at the bar, drawn (`rules::champions`): half a head taller
//! than the company and broad across the shoulders, each with their great
//! arm held out by the aim. The Hammerer's maul rests on his shoulder, comes up
//! overhead for the slam and sweeps round on the swing; the Butcher's
//! cleaver chops and his chain hangs off a shoulder; the Alchemist brings a
//! lit flask up over his hat to throw. Under and over the figures: the
//! slam's shockwave and cracks, flasks lobbed in their arcs, the hook on its
//! chain, the Earthsplitter's crack and the rubble it throws up, the rot.
//! These are live state, so their marks are drawn in signal inks where a
//! colour must say what something is.

use super::super::ink::{Img, hash};
use super::knights::{Facing, facing};
use super::{UNIT, at};
use crate::rules::champions::{self, Brew, Champion, RingKind, Thrown};
use crate::rules::{Hero, Run};

/// The figure's canvas: the body's twenty pixels with room either side
/// for the great arm's whole sweep (the maul's head stays on the picture
/// flung out to either side), and headroom over it for a maul raised
/// overhead.
const W: i32 = 62;
const BODY_X: i32 = 21;
const HEAD: i32 = 13;
const BODY_H: i32 = 22;

const HAMMER_FRONT: [&str; 18] = [
    ".......JHHHHJ.......",
    "..h...JHiHHiHJ...h..",
    "..Jhh.JiiiiiiJ.hhJ..",
    "....hhJGGGGGGJhh....",
    "......GtkttktG......",
    "......toTttTot......",
    "...vVVRRroorRRVVv...",
    ".vVWWVRRrRRrRRVWWVv.",
    ".vWVVvuRRrrRRuvVVWv.",
    ".uvvvuSRRRRRRSuvvvu.",
    ".otoSSBbbRRbbBSSoto.",
    ".tToSBBbIbbIbBBSoTt.",
    ".ttoSB77777777BSott.",
    ".toT.BB778877BB.Tot.",
    ".oto.BBBBbbBBBB.oto.",
    "..tt.BIBBIIBBIB.tt..",
    "..oo.bBBIbbIBBb.oo..",
    "....................",
];
const HAMMER_BACK: [&str; 18] = [
    ".......JHHHHJ.......",
    "..h...JHiiiiHJ...h..",
    "..Jhh.JiiiiiiJ.hhJ..",
    "....hhJGGGGGGJhh....",
    "......GJJJJJJG......",
    "......RRrRRrRR......",
    "...vVVRrRRRRrRVVv...",
    ".vVWWVRrrRRrrRVWWVv.",
    ".vWVVvuSSSSSSuvVVWv.",
    ".uvvvuSSSSSSSSuvvvu.",
    ".otoSSBBBBBBBBSSoto.",
    ".tToSBBIBBBBIBBSoTt.",
    ".ttoSB77777777BSott.",
    ".toT.BB777777BB.Tot.",
    ".oto.BBBBBBBBBB.oto.",
    "..tt.BIBBIIBBIB.tt..",
    "..oo.bBBIbbIBBb.oo..",
    "....................",
];
const BUTCHER_FRONT: [&str; 18] = [
    "........BBBB........",
    ".......BrrrrB.......",
    "......BrrrrrrB......",
    "......rkrrrrkr......",
    "......BrttttrB......",
    ".....ooRpRRpRoo.....",
    "...ooottttttttooo...",
    "..otttVVVVVVVVttto..",
    ".ottTtVVVVVVVVtTtto.",
    ".ottVVVVVVVVVVpVtto.",
    ".ottVVVVVVVVVVVVtto.",
    ".otVVpVVVVVVVVVVVto.",
    ".toVVVVVVVVVVVVVVot.",
    ".oo77777777777777oo.",
    ".tt.VVVVVV88VVVV.tt.",
    "....vVVVVVVVVpVv....",
    ".....vvvpvvvvvv.....",
    "....................",
];
const BUTCHER_BACK: [&str; 18] = [
    "........BBBB........",
    ".......BrrrrB.......",
    "......BrrrrrrB......",
    "......rrrrrrrr......",
    "......BrrrrrrB......",
    ".....ooBrrrrBoo.....",
    "...ooottttttttooo...",
    "..otttottTTttottto..",
    ".ottTtotTttTtottTto.",
    ".otttV7ottto7Vttto..",
    ".ottVVV7otto7VVVtto.",
    ".ottVVVV7oo7VVVVtto.",
    ".toVVVVVV77VVVVVVot.",
    ".ooVVVVV7777VVVVVoo.",
    ".tt.VVVVVVVVVVVV.tt.",
    "....vVVVVVVVVVVv....",
    ".....vvvvvvvvvv.....",
    "....................",
];
const ALCHEMIST_FRONT: [&str; 18] = [
    "........oOOo........",
    ".......rOrrOr.......",
    ".....PPPPPPPPPP.....",
    "...PPPPrrrrrrPPPP...",
    "......IIIIIIII......",
    ".oO....zVttVz....Oo.",
    ".oTo...tTttTt...oTo.",
    ".otOxSS.tttt.SSxOto.",
    ".oOxSSSuSSSSuSSSxOo.",
    "..xSSuSRSSSSRSuSSx..",
    "..xSuSSYRSSRYSSuSx..",
    "..xSSSSSASSASSSSSx..",
    "..tSSS777zz777SSSt..",
    "..otSSSS8SS8SSSSto..",
    "...oSSSSSSSSSSSSo...",
    "...xSSSSSSSSSSSSx...",
    "...xSSxSSSSSSxSSx...",
    "....xx.x....x.xx....",
];
const ALCHEMIST_BACK: [&str; 18] = [
    "........oOOo........",
    ".......oTttTo.......",
    ".....PPoOttOoPP.....",
    "...PPPPoTttToPPPP...",
    "......IoOttOoI......",
    ".......roorro.......",
    ".......oOttOo.......",
    "...xSSSoTttToSSSx...",
    "..xSSSuoOttOouSSSx..",
    "..xSSuSrOrrOrSuSSx..",
    "..xSuSSoorrooSSuSx..",
    "..xSSSSSSrrSSSSSSx..",
    "..tSSS77777777SSSt..",
    "..otSSSSSSSSSSSSto..",
    "...oSSSSSSSSSSSSo...",
    "...xSSSSSSSSSSSSx...",
    "...xSSxSSSSSSxSSx...",
    "....xx.x....x.xx....",
];

// The Reaver in his wolf pelt, ginger beard and a fur mantle over bare
// arms; the Lancer in plate under a tabard of blue and gold.
const REAVER_FRONT: [&str; 18] = [
    ".....J........J.....",
    ".....hJ......Jh.....",
    "....JhhJJJJJJhhJ....",
    "...JhhhhhhhhhhhhJ...",
    "...JhGGtkttktGGhJ...",
    "...JhGotTttTtoGhJ...",
    "..JhhJRRrooorRJhhJ..",
    ".hJhhhRRrRRRrRhhhJh.",
    ".tTJhhJRRrrrRJhhJTt.",
    ".tToJJbBRRRRBbJJoTt.",
    ".ottxbBBtRRtBBbxtto.",
    ".tTo.bBttttttBb.oTt.",
    ".tto.E77775777E.ott.",
    "..t..EEeEEEEeEE..t..",
    ".....EeEEeeEEeE.....",
    ".....BIBBIIBBIB.....",
    ".....bBBIbbIBBb.....",
    "....................",
];
const REAVER_BACK: [&str; 18] = [
    ".....J........J.....",
    ".....hJ......Jh.....",
    "....JhhJJJJJJhhJ....",
    "...JhhhhhhhhhhhhJ...",
    "...JhhhhJJJJhhhhJ...",
    "...JhhhJhhhhJhhhJ...",
    "..JhhJhhhhhhhhJhhJ..",
    ".hJhhhhhJhhJhhhhhJh.",
    ".tTJhhhhhJJhhhhhJTt.",
    ".tToJhhhhhhhhhhJoTt.",
    ".ottxbBBBBBBBBbxtto.",
    ".tTo.bBBbBBbBBb.oTt.",
    ".tto.E77777777E.ott.",
    "..t..EEeEEEEeEE..t..",
    ".....EeEEeeEEeE.....",
    ".....BIBBIIBBIB.....",
    ".....bBBIbbIBBb.....",
    "....................",
];
const LANCER_FRONT: [&str; 18] = [
    ".........1z.........",
    "........z1zz........",
    ".......JHHHHJz......",
    "......JHiHHiHJ......",
    "......JkkkkkkJ......",
    "......JHhHHhHJ......",
    "....hiJJGGGGJJih....",
    "..hHiHhqzzzzqhHiHh..",
    ".hHiihJqz44zqJhiiHh.",
    ".JhhJ.qzz44zzq.JhhJ.",
    ".Hih..qz4zz4zq..hiH.",
    ".JJ...q4zzzz4q...JJ.",
    "......q775577q......",
    "......qzzzzzzq......",
    "......qzq..qzq......",
    ".......hHhhHh.......",
    ".......JHJJHJ.......",
    "....................",
];
const LANCER_BACK: [&str; 18] = [
    ".........1z.........",
    "........z1zz........",
    ".......JHHHHJz......",
    "......JHiiiiHJ......",
    "......JHHHHHHJ......",
    "......JhHHHHhJ......",
    "....hiJJGGGGJJih....",
    "..hHiHhqqqqqqhHiHh..",
    ".hHiihJqzzzzqJhiiHh.",
    ".JhhJ.qzzzzzzq.JhhJ.",
    ".Hih..qzz44zzq..hiH.",
    ".JJ...qz4zz4zq...JJ.",
    "......q777777q......",
    "......qzzzzzzq......",
    "......qzq..qzq......",
    ".......hHhhHh.......",
    ".......JHJJHJ.......",
    "....................",
];

// Across, facing right: the Hammerer's horn and beard forward, the
// Butcher's belly before him, the Alchemist's still on his back.
const HAMMER_SIDE: [&str; 18] = [
    ".......JHHHJ........",
    "......JHiiiHJ..h....",
    ".....JHiiHiiJhhJ....",
    ".....JGGGGGGGGG.....",
    "......GJtttktG......",
    "......GJtoTttT......",
    "....vVVRRrRRRo......",
    "..vVWWVVRRrRR.......",
    ".vVWVVVvuRRRr.......",
    ".uvvvvvuSSRR........",
    "...SSSSSBBbbto......",
    "...SBBBBBBIoTto.....",
    "...S7777777BotT.....",
    "...BB77777BBott.....",
    "...BBBBBBBBBoo......",
    "...BIBBIBBIBB.......",
    "...bBBBIBBBb........",
    "....................",
];
const BUTCHER_SIDE: [&str; 18] = [
    "....................",
    ".........BBBB.......",
    "........BrrrrB......",
    ".......BrrrrkrB.....",
    ".......BrrrrttT.....",
    ".......BBrrtRpt.....",
    ".....oooottttt......",
    "....otttttVVVV......",
    "...ottTttVVVVVV.....",
    "...otttVVVVVpVVV....",
    "...ottVVVVVVVVVVV...",
    "...otVVVpVVVVVVVVt..",
    "...ooVVVVVVVVVVVoto.",
    "....77VVVVVVVVVVott.",
    "....8VVVVVVVpVVVoo..",
    ".....vVVVVVVVVVv....",
    "......vvvvvvvvv.....",
    "....................",
];
const ALCHEMIST_SIDE: [&str; 18] = [
    "..oO................",
    "..oTt...............",
    ".oOTt.PPPPP.........",
    ".otTtPPPrrrPPPPP....",
    ".oOto..IIIIIII......",
    ".roor...ttzVt.......",
    ".oOto...ttTtTt......",
    ".oTtoxSSStttt.......",
    ".rOoSSSuSSSS........",
    "..ooxSuSSRSSS.......",
    "...xSSSSSSARSto.....",
    "...xSSSSSSSSSoTo....",
    "...xS7777777zSoto...",
    "...xSSSSSSS8SSoo....",
    "...xSSSSSSSSSS......",
    "...xSSSSSSSSSx......",
    "...xSSxSSSxSSx......",
    "....xx.....xx.......",
];

const REAVER_SIDE: [&str; 18] = [
    "........J...........",
    ".......hJ...........",
    "......JhhJJJJ.......",
    ".....JhhhhhhhJ......",
    ".....JhhhhGtkt......",
    ".....JhhhGotTto.....",
    "....JhhhJRRroor.....",
    "...hJhhhJRRrRRr.....",
    "..hJhhhhJRRrRR......",
    "..JhhhhbBRRRtT......",
    "...hhbBBBttttTo.....",
    "...JbBBbttttoTt.....",
    "....E77775777oto....",
    "....EEeEEEEeEott....",
    "....EeEEeeEEeoo.....",
    "....BIBBIIBBIB......",
    "....bBBIbbIBBb......",
    "....................",
];
const LANCER_SIDE: [&str; 18] = [
    ".......1z...........",
    "......z1zz..........",
    "......JHHHHJ........",
    ".....JHiHHiHJ.......",
    ".....JHHHkkkJ.......",
    ".....JHhHHhHH.......",
    "......JGGGGJh.......",
    ".....hHiqzzzh.......",
    "....hHiiqz44z.......",
    "....JhhJqzz4z.......",
    "....HihJqz4zz.......",
    "....JJ.qz4zzz.......",
    ".......q7757q.......",
    ".......qzzzzq.......",
    ".......qzq.qz.......",
    "........hHhH........",
    "........JHJH........",
    "....................",
];

fn body(c: Champion, face: Facing) -> &'static [&'static str] {
    match (c, face) {
        (Champion::Hammerer, Facing::Down) => &HAMMER_FRONT,
        (Champion::Hammerer, Facing::Side) => &HAMMER_SIDE,
        (Champion::Hammerer, Facing::Up) => &HAMMER_BACK,
        (Champion::Butcher, Facing::Down) => &BUTCHER_FRONT,
        (Champion::Butcher, Facing::Side) => &BUTCHER_SIDE,
        (Champion::Butcher, Facing::Up) => &BUTCHER_BACK,
        (Champion::Alchemist, Facing::Down) => &ALCHEMIST_FRONT,
        (Champion::Alchemist, Facing::Side) => &ALCHEMIST_SIDE,
        (Champion::Alchemist, Facing::Up) => &ALCHEMIST_BACK,
        (Champion::Reaver, Facing::Down) => &REAVER_FRONT,
        (Champion::Reaver, Facing::Side) => &REAVER_SIDE,
        (Champion::Reaver, Facing::Up) => &REAVER_BACK,
        (Champion::Lancer, Facing::Down) => &LANCER_FRONT,
        (Champion::Lancer, Facing::Side) => &LANCER_SIDE,
        (Champion::Lancer, Facing::Up) => &LANCER_BACK,
    }
}

/// Where the great arm is in its movement.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Grip {
    /// Carried: the maul on the shoulder, the cleaver up, a flask at the hip.
    Rest,
    /// Groundslam's wind-up: straight up overhead.
    Overhead,
    /// The swing, 0 at its start to 1 at its end.
    Swing(f32),
    /// Brought down hard on the floor before them.
    Down,
    /// Thrown: the arm flung out along the aim.
    Fling,
}

fn grip(hero: &Hero, c: Champion) -> Grip {
    let s = &hero.champ;
    if s.windup > 0 {
        return Grip::Overhead;
    }
    // In the middle of a bound: the axes up for the leap, the lance couched
    // for a vault or a drive. The lance set while it is braced.
    if s.dash > 0 {
        return if s.dash_kind == champions::Dash::Leap { Grip::Overhead } else { Grip::Fling };
    }
    if s.brace > 0 {
        return Grip::Down;
    }
    if s.cast > 0 {
        return match (c, s.spell) {
            (Champion::Hammerer, 2) | (Champion::Hammerer, 4) => Grip::Down,
            (Champion::Reaver, 3) => Grip::Overhead,
            (Champion::Reaver, 4) | (Champion::Lancer, 3) => Grip::Down,
            _ => Grip::Fling,
        };
    }
    if hero.swing > 0 {
        return Grip::Swing(1.0 - hero.swing as f32 / 6.0);
    }
    if c == Champion::Alchemist && hero.fire_cooldown + 8 > champions::FLASK_REARM {
        return Grip::Fling;
    }
    Grip::Rest
}

/// The great arm's angle (radians, screen-wise: 0 along the aim, a
/// negative turn up) for a grip.
fn angle(c: Champion, g: Grip) -> f32 {
    let deg: f32 = match (c, g) {
        (Champion::Hammerer, Grip::Rest) => -118.0,
        (Champion::Butcher, Grip::Rest) => -60.0,
        (Champion::Alchemist, Grip::Rest) => 70.0,
        (Champion::Reaver, Grip::Rest) => -45.0,
        (Champion::Lancer, Grip::Rest) => -84.0,
        // The lance thrusts straight; it never sweeps.
        (Champion::Lancer, Grip::Swing(p)) => -14.0 + 10.0 * p,
        (_, Grip::Overhead) => -92.0,
        (_, Grip::Down) => 36.0,
        (Champion::Alchemist, Grip::Fling) => -100.0,
        (_, Grip::Fling) => -10.0,
        (_, Grip::Swing(p)) => {
            let eased = 1.0 - (1.0 - p) * (1.0 - p);
            -125.0 + 180.0 * eased
        }
    };
    deg.to_radians()
}

/// Draw a rotated bar of `len` pixels by `wide` from `(x, y)` along `a`:
/// lit along its upper edge, dark along its lower.
fn bar(im: &mut Img, (x, y): (f32, f32), a: f32, (from, len): (f32, f32), wide: f32, inks: [char; 3]) {
    let (dx, dy) = (a.cos(), a.sin());
    let (px, py) = (-dy, dx);
    let mut u = from;
    while u <= from + len {
        let mut v = -wide / 2.0;
        while v <= wide / 2.0 {
            let ink = if v < -wide / 2.0 + 1.0 {
                inks[0]
            } else if v > wide / 2.0 - 1.0 {
                inks[2]
            } else {
                inks[1]
            };
            im.put((x + dx * u + px * v).round() as i32, (y + dy * u + py * v).round() as i32, ink);
            v += 0.5;
        }
        u += 0.5;
    }
}

/// The great arm itself, from the hand at `(x, y)` along `a`.
fn arm(im: &mut Img, c: Champion, (x, y): (f32, f32), a: f32, fury: bool) {
    match c {
        Champion::Hammerer => {
            // An ash haft as long as a man, and an iron head like an anvil.
            bar(im, (x, y), a, (-2.0, 20.0), 2.0, ['O', 'B', 'b']);
            let head = if fury { ['T', 'R', 'p'] } else { ['W', 'i', 'G'] };
            bar(im, (x, y), a, (15.0, 6.5), 10.0, head);
            bar(im, (x, y), a, (15.0, 0.5), 10.0, ['h', 'h', 'h']);
            bar(im, (x, y), a, (21.0, 0.5), 10.0, ['J', 'J', 'J']);
        }
        Champion::Butcher => {
            bar(im, (x, y), a, (-2.0, 6.0), 2.0, ['O', 'B', 'b']);
            bar(im, (x, y), a, (4.0, 9.0), 7.0, ['W', 'V', 'u']);
            bar(im, (x, y), a, (7.0, 2.0), 3.0, ['p', 'R', 'p']);
        }
        Champion::Reaver => {
            // A short haft and a bearded head, its edge bright; red in the
            // bloodlust.
            bar(im, (x, y), a, (-2.0, 11.0), 2.0, ['O', 'B', 'b']);
            let head = if fury { ['T', 'R', 'p'] } else { ['H', 'i', 'J'] };
            bar(im, (x, y), a, (6.5, 4.0), 7.0, head);
            bar(im, (x, y), a, (10.5, 0.5), 7.0, ['h', 'h', 'h']);
        }
        Champion::Lancer => {
            // A long ash lance, the round guard at the hand, and a steel
            // point tapering out past it.
            bar(im, (x, y), a, (-4.0, 18.0), 2.0, ['O', 'B', 'b']);
            bar(im, (x, y), a, (1.5, 2.5), 6.0, ['H', 'i', 'J']);
            bar(im, (x, y), a, (14.0, 4.0), 3.0, ['H', 'i', 'h']);
            bar(im, (x, y), a, (18.0, 3.0), 1.0, ['H', 'H', 'H']);
        }
        Champion::Alchemist => {
            // A lit flask in the fist: the glass, the brew, the wick.
            let (dx, dy) = (a.cos(), a.sin());
            let (fx, fy) = ((x + dx * 3.0).round() as i32, (y + dy * 3.0).round() as i32);
            im.rect(fx - 2, fy - 2, 5, 5, 'R');
            im.rect(fx - 1, fy - 1, 3, 3, 'T');
            im.put(fx - 1, fy - 2, 'W');
            im.rect(fx, fy - 4, 1, 2, 'i');
            im.put(fx, fy - 5, 'T');
        }
    }
}

/// A champion's figure, facing right when across (the caller flips it for
/// left), feet on the bottom row and their middle at the canvas's middle.
pub(super) fn figure(hero: &Hero, c: Champion) -> Img {
    let face = facing(hero.aim_x, hero.aim_y);
    let stride = super::class_looks::stride(hero);
    let g = grip(hero, c);
    let fury = hero.champ.fury > 0 || hero.champ.lust > 0;
    let windup = hero.champ.windup > 0;
    draw(c, face, stride, g, fury, windup, hero.champ.fury.max(hero.champ.lust), None)
}

/// At rest, square on: for the bar and a ghost.
pub(super) fn standing(c: Champion) -> Img {
    draw(c, Facing::Down, None, Grip::Rest, false, false, 0, None)
}

/// Head and shoulders for a seat's frame: the body alone, so the crop
/// finds the head and not the arm held over it.
pub(super) fn bust(c: Champion) -> Img {
    Img::from_rows(body(c, Facing::Down))
}

fn draw(c: Champion, face: Facing, stride: Option<u8>, g: Grip, fury: bool, windup: bool, tick: u32, floating: Option<(f32, f32)>) -> Img {
    let mut im = Img::new(W, HEAD + BODY_H);
    let away = face == Facing::Up;
    let body = Img::from_rows(body(c, face));
    // A crouch under the wind-up; a bob on the stride.
    let sink = i32::from(windup) * 2 + i32::from(matches!(stride, Some(1 | 3)));
    // The hand: before the body across, at the right fist square on, the
    // left away.
    let (hx, hy) = match (face, c) {
        (Facing::Side, Champion::Hammerer) => (13, 11),
        (Facing::Side, Champion::Butcher) => (16, 12),
        (Facing::Side, Champion::Alchemist) => (15, 11),
        (Facing::Side, Champion::Reaver) => (14, 11),
        (Facing::Side, Champion::Lancer) => (14, 11),
        (Facing::Down, Champion::Hammerer) => (18, 13),
        (Facing::Down, Champion::Butcher) => (18, 12),
        (Facing::Down, Champion::Alchemist) => (17, 12),
        (Facing::Down, Champion::Reaver) => (18, 12),
        (Facing::Down, Champion::Lancer) => (18, 12),
        (Facing::Up, _) => (2, 13),
    };
    let hand = ((BODY_X + hx) as f32, (HEAD + hy + sink) as f32);
    let a = angle(c, g);
    // Square on, the arm goes up and out to the fist's side; away, it is
    // behind the body.
    let a = match face {
        Facing::Side => a,
        Facing::Down => a.clamp(-2.4, 1.2),
        Facing::Up => std::f32::consts::PI - a,
    };
    if away && floating.is_none() {
        arm(&mut im, c, hand, a, fury);
    }
    // The legs, on their stride.
    let lift = |left: bool| match stride {
        Some(0) => i32::from(left) * 2,
        Some(2) => i32::from(!left) * 2,
        _ => 0,
    };
    // Across, the legs stand close, one before the other.
    let (near, far) = if face == Facing::Side { (7, 10) } else { (6, 11) };
    for (left, x0) in [(true, BODY_X + near), (false, BODY_X + far)] {
        let top = HEAD + 17;
        let up = lift(left);
        im.rect(x0, top - up, 3, 2, 'B');
        im.rect(x0, top + 2 - up, 3, 3, 'I');
        im.put(if left { x0 - 1 } else { x0 + 3 }, top + 4 - up, 'I');
    }
    im.stamp(&body, BODY_X, HEAD + sink);
    // The Butcher's chain hangs off his shoulder, the hook at its end.
    if c == Champion::Butcher && face == Facing::Down {
        for k in 0..6 {
            im.put(BODY_X + 1 + k / 3, HEAD + 7 + k * 2 + sink, if k % 2 == 0 { 'h' } else { 'J' });
        }
        im.rect(BODY_X + 2, HEAD + 18 + sink, 3, 1, 'i');
        im.put(BODY_X + 4, HEAD + 17 + sink, 'i');
    }
    // The Reaver's other axe, hung at his left hip by its haft.
    if c == Champion::Reaver && face == Facing::Down {
        im.rect(BODY_X + 2, HEAD + 10 + sink, 1, 7, 'B');
        im.rect(BODY_X, HEAD + 14 + sink, 2, 4, 'i');
        im.rect(BODY_X - 1, HEAD + 15 + sink, 1, 2, 'H');
        im.put(BODY_X + 2, HEAD + 9 + sink, 'O');
    }
    if fury {
        // The giant's blood up: a red heat at the edges, flickering.
        let mut glow = im.clone();
        glow.outline_outside(if tick % 6 < 3 { 'R' } else { 'p' });
        glow.stamp(&im, 0, 0);
        im = glow;
    }
    if !away && floating.is_none() {
        arm(&mut im, c, hand, a, fury);
    }
    // The swing's sweep behind the maul's head.
    if let Some(aim) = floating {
        // One broadside drawing, pointing right, with its grip retained
        // through cropping. The live aim turns it, never the body's facing.
        let mut tile = Img::new(W, W);
        arm(&mut tile, c, (W as f32 / 2.0, W as f32 / 2.0), 0.0, fury);
        let mut bounds = (W, W, 0, 0);
        for y in 0..W { for x in 0..W {
            if tile.get(x, y).is_some() {
                bounds = (bounds.0.min(x), bounds.1.min(y), bounds.2.max(x + 1), bounds.3.max(y + 1));
            }
        } }
        let mut weapon = Img::new(bounds.2 - bounds.0, bounds.3 - bounds.1);
        weapon.stamp(&tile, -bounds.0, -bounds.1);
        return super::wear::clocked(&im, &weapon, (W / 2 - bounds.0, W / 2 - bounds.1),
            0.0, (BODY_X + 15, HEAD + 11 + sink), aim);
    }
    if let Grip::Swing(p) = g {
        let reach = match c {
            Champion::Hammerer => 19.0,
            Champion::Butcher => 12.0,
            Champion::Reaver => 11.0,
            Champion::Alchemist | Champion::Lancer => 0.0,
        };
        let head = angle(c, g);
        let mut t = -125.0f32.to_radians();
        while t < head && reach > 0.0 && p < 0.95 {
            let a = match face {
                Facing::Up => std::f32::consts::PI - t,
                _ => t,
            };
            im.put((hand.0 + a.cos() * reach).round() as i32, (hand.1 + a.sin() * reach).round() as i32, 'V');
            t += 0.06;
        }
    }
    im
}

// ─── under and over the figures ──────────────────────────────────────────────

/// Under the figures: the shockwaves and bursts, cracks, the rot, and the
/// lobbed flasks' shadows.
pub(super) fn under(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let f = &run.champions;
    for ring in &f.rings {
        let (cx, cy) = at(ring.x, ring.y);
        let k = (ring.age as f32 + 1.0) / ring.life as f32;
        let r = ring.r * UNIT * (0.35 + 0.65 * k.sqrt());
        let n = (r * 5.0) as i32 + 8;
        let (rim, inner) = match ring.kind {
            RingKind::Quake => ('i', 'J'),
            RingKind::Burst => ('6', 'R'),
            RingKind::Mend => ('2', 'C'),
            RingKind::Reap => ('7', '8'),
        };
        // A thick rim and a softer one inside it, broken here and there.
        let thick = if ring.kind == RingKind::Quake { 3.0 } else { 2.0 };
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            if hash(i, ring.age as i32 / 3, 7) % 6 == 0 {
                continue;
            }
            let mut d = 0.0;
            while d < thick {
                cv.put(cx + (a.cos() * (r - d)) as i32, cy + (a.sin() * (r - d) * 0.6) as i32, rim);
                d += 1.0;
            }
            cv.put(cx + (a.cos() * (r - thick - 2.0)) as i32, cy + (a.sin() * (r - thick - 2.0) * 0.6) as i32, inner);
        }
        match ring.kind {
            // The floor cracks out from where the maul fell.
            RingKind::Quake => {
                // Dust thrown up off the rim's front, and the floor split.
                for k in 0..10 {
                    let a = k as f32 * 0.63 + 0.2;
                    let (x, y) = (cx + (a.cos() * r) as i32, cy + (a.sin() * r * 0.6) as i32 - ring.age as i32 / 3);
                    let size = 3 - (ring.age as i32 / 5).min(2);
                    cv.rect(x - size / 2, y - size, size + 1, size, if k % 2 == 0 { 'J' } else { 'G' });
                }
                for ray in 0..9 {
                    let a = ray as f32 * 0.7 + 0.3;
                    let len = (ring.r * UNIT * 0.9).min(r + 4.0);
                    let (mut x, mut y) = (cx as f32, cy as f32);
                    let mut s = 0.0;
                    while s < len {
                        let jag = (hash(ray, s as i32, 13) % 3) as f32 - 1.0;
                        x += a.cos() * 2.0 - a.sin() * jag * 0.6;
                        y += (a.sin() * 2.0 + a.cos() * jag * 0.6) * 0.6;
                        cv.put(x as i32, y as i32, 'k');
                        cv.put(x as i32 + 1, y as i32, 'k');
                        cv.put(x as i32, y as i32 - 1, 'G');
                        s += 2.0;
                    }
                }
            }
            RingKind::Mend => {
                for k in 0..6 {
                    let a = k as f32 * 1.05 + ring.age as f32 * 0.08;
                    let (x, y) = (cx + (a.cos() * r * 0.6) as i32, cy + (a.sin() * r * 0.35) as i32 - ring.age as i32 / 2);
                    cv.line(x - 2, y, x + 2, y, '2');
                    cv.line(x, y - 2, x, y + 2, '2');
                }
            }
            RingKind::Burst => {}
            RingKind::Reap => {
                // The two axes' cuts, thrown wide round him, fading as it
                // spreads: crescents in blood and a bright edge.
                for k in 0..8 {
                    let a = k as f32 * 0.785 + ring.age as f32 * 0.12;
                    let (mut t, end) = (a - 0.35, a + 0.35);
                    while t < end {
                        let (x, y) = (cx + (t.cos() * r) as i32, cy + (t.sin() * r * 0.6) as i32);
                        cv.put(x, y, if ring.age < 5 { 'c' } else { '7' });
                        cv.put(x, y + 1, '8');
                        t += 0.05;
                    }
                }
            }
        }
    }
    // A bound kicks up dust where it leaves and along its way.
    for hero in run.players.values().filter(|h| h.hp > 0 && h.champ.dash > 0) {
        let (cx, cy) = at(hero.x, hero.y);
        for k in 0..5 {
            let back = (k * 5 + 4) as f32;
            let (x, y) = (cx - (hero.champ.dash_x * back) as i32, cy - (hero.champ.dash_y * back * 0.6) as i32);
            let j = (hash(k, tick as i32, 11) % 3) as i32 - 1;
            cv.rect(x + j, y - 1, 3 - k / 2, 2, if k % 2 == 0 { 'J' } else { 'G' });
        }
    }
    for c in &f.cracks {
        let (x0, y0) = at(c.x, c.y);
        let grown = (c.age as f32 / champions::CRACK_RUN as f32).min(1.0) * c.len;
        let erupted = c.age >= champions::CRACK_ERUPT;
        let steps = (grown * UNIT / 2.0) as i32;
        for s in 0..=steps {
            let along = s as f32 * 2.0;
            let jag = (hash(s, c.owner as i32, 31) % 5) as i32 - 2;
            let (x, y) = (
                x0 + (c.dx * along) as i32 - (c.dy * jag as f32) as i32,
                y0 + (c.dy * along * 0.6) as i32 + (c.dx * jag as f32 * 0.6) as i32,
            );
            // A molten seam before it goes; split stone after.
            let hot = if erupted { 'p' } else if (s + tick as i32 / 2) % 3 == 0 { '6' } else { 'R' };
            cv.rect(x - 1, y - 1, 3, 3, 'k');
            cv.put(x + 1, y - 2, hot);
            cv.put(x - 1, y + 1, hot);
            cv.put(x, y, hot);
            if erupted && c.age < champions::CRACK_LIFE - 4 && s % 2 == 0 {
                // Rubble thrown up along it: a jut of rock, lit from the
                // upper left, rising as it bursts and sinking after.
                let rise = (c.age - champions::CRACK_ERUPT + 2).min(4) as i32;
                let sink = (c.age as i32 - (champions::CRACK_LIFE as i32 - 14)).max(0);
                let h = (6 + (hash(s, 5, 17) % 7) as i32) * rise / 4 - sink;
                for k in 0..h.max(0) {
                    let half = (h - k + 1) / 2;
                    cv.line(x - half, y - k, x + half, y - k, 'G');
                    cv.put(x - half, y - k, 'h');
                    cv.put(x + half, y - k, 'j');
                }
                if h > 2 {
                    cv.put(x, y - h, 'h');
                }
            }
        }
    }
    for (_, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && h.champ.rot > 0) {
        let (cx, cy) = at(hero.x, hero.y);
        let r = champions::ROT_REACH * UNIT;
        // The stink's edge on the floor, where it stops eating.
        let n = (r * 5.0) as i32;
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU + tick as f32 * 0.02;
            if hash(i, tick as i32 / 8, 21) % 3 == 0 {
                continue;
            }
            cv.put(cx + (a.cos() * r) as i32, cy + (a.sin() * r * 0.6) as i32, 'm');
        }
        // Clots of it rolling off him and rising.
        for k in 0..36 {
            let a = k as f32 * 0.9 + tick as f32 * 0.05;
            let d = r * (0.25 + (hash(k, tick as i32 / 6, 3) % 75) as f32 / 100.0);
            let ink = if k % 3 == 0 { 'Y' } else if k % 3 == 1 { 'A' } else { 'm' };
            let rise = (tick as i32 / 2 + k * 3) % 14;
            let (x, y) = (cx + (a.cos() * d) as i32, cy + (a.sin() * d * 0.6) as i32 - rise);
            let size = if rise < 7 { 2 } else { 1 };
            cv.rect(x, y, size + 1, size, ink);
        }
    }
    for t in f.throws.iter().filter(|t| matches!(t.kind, Thrown::Flask(_))) {
        let (x, y) = at(t.x, t.y);
        cv.rect(x - 2, y, 5, 1, 'g');
    }
}

/// Over the figures: hammers and hooks in flight, chains, flasks in their
/// arcs, and the Butcher's grip.
pub(super) fn over(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let f = &run.champions;
    for t in &f.throws {
        let (x, y) = at(t.x, t.y);
        match t.kind {
            Thrown::Hammer => {
                // A little maul turning end over end, sparking.
                let a = t.age as f32 * 0.9;
                let mut im = Img::new(17, 17);
                bar(&mut im, (8.0, 8.0), a, (-6.0, 9.0), 2.0, ['O', 'B', 'b']);
                bar(&mut im, (8.0, 8.0), a, (2.0, 5.0), 8.0, ['W', 'i', 'G']);
                cv.stamp(&im, x - 8, y - 14);
                for k in 1..5 {
                    let (bx, by) = at(t.x - t.vx * 0.02 * k as f32, t.y - t.vy * 0.02 * k as f32);
                    cv.put(bx + (hash(k, tick as i32, 9) % 3) as i32 - 1, by - 6, if k % 2 == 0 { 'z' } else { 'V' });
                }
            }
            Thrown::Hook | Thrown::Haul => {
                let Some(owner) = run.players.get(&t.owner) else {
                    continue;
                };
                let (ox, oy) = at(owner.x, owner.y);
                let (ox, oy) = (ox, oy - 12);
                let (hx, hy) = (x, y - 6);
                let n = ((hx - ox).abs().max((hy - oy).abs()) / 2).max(1);
                for k in 0..=n {
                    let (lx, ly) = (ox + (hx - ox) * k / n, oy + (hy - oy) * k / n);
                    // Links, turned alternately: a ring, then one edge on.
                    if k % 2 == 0 {
                        cv.rect(lx - 1, ly - 1, 3, 3, 'h');
                        cv.put(lx, ly, 'g');
                    } else {
                        cv.rect(lx, ly - 1, 1, 3, 'J');
                    }
                }
                // The hook: a great barb turned back on itself.
                cv.rect(hx - 1, hy - 5, 3, 9, 'i');
                cv.line(hx - 1, hy - 5, hx - 1, hy + 3, 'W');
                cv.rect(hx - 1, hy + 3, 7, 2, 'i');
                cv.rect(hx + 4, hy - 1, 2, 5, 'i');
                cv.put(hx + 4, hy - 2, 'W');
                cv.put(hx + 3, hy - 1, 'W');
            }
            Thrown::Axe => {
                // An axe turning edge over haft, a red smear behind it.
                let a = t.age as f32 * 1.1;
                let mut im = Img::new(15, 15);
                bar(&mut im, (7.0, 7.0), a, (-5.0, 9.0), 2.0, ['O', 'B', 'b']);
                bar(&mut im, (7.0, 7.0), a, (1.5, 3.5), 6.0, ['H', 'i', 'J']);
                cv.stamp(&im, x - 7, y - 13);
                for k in 1..4 {
                    let back = if t.back { -1.0 } else { 1.0 } * 0.025 * k as f32;
                    let (bx, by) = at(t.x - t.vx * back, t.y - t.vy * back);
                    cv.put(bx, by - 6, if k == 1 { 'R' } else { 'p' });
                }
            }
            Thrown::Javelin => {
                // A long shaft along its flight, its point bright, and the air
                // torn behind it.
                let l = t.vx.hypot(t.vy).max(0.01);
                let (ux, uy) = (t.vx / l, t.vy / l * 0.6);
                let (tx, ty) = (x, y - 7);
                for k in 0..14 {
                    let (px, py) = (tx - (ux * k as f32) as i32, ty - (uy * k as f32) as i32);
                    cv.put(px, py, if k < 3 { 'H' } else if k < 4 { 'i' } else { 'O' });
                    if k >= 4 {
                        cv.put(px, py + 1, 'B');
                    }
                }
                for k in 0..3 {
                    let back = 16 + k * 3;
                    cv.put(tx - (ux * back as f32) as i32, ty - (uy * back as f32) as i32 - 1 + k as i32 % 2, 'V');
                }
            }
            Thrown::Flask(brew) => {
                let (_, high) = champions::lob(t);
                let lift = (high * UNIT) as i32;
                let (glass, lit) = match brew {
                    Brew::Fire => ('R', 'T'),
                    Brew::Quickfire => ('7', '6'),
                    Brew::Foul => ('A', 'Y'),
                };
                // A round-bellied flask turning over in the air, its wick lit.
                let (fx, fy) = (x, y - 8 - lift);
                cv.rect(fx - 3, fy - 2, 7, 5, glass);
                cv.rect(fx - 2, fy - 3, 5, 7, glass);
                cv.rect(fx - 1, fy - 1, 2, 2, lit);
                cv.put(fx - 2, fy - 2, 'W');
                let tip = if (t.age / 3) % 2 == 0 { (0, -6) } else { (3, -5) };
                cv.rect(fx, fy - 5, 1, 2, 'i');
                cv.put(fx + tip.0 / 2, fy + tip.1, 'T');
                if tick % 2 == 0 {
                    cv.put(fx + (hash(t.age as i32, 1, 3) % 3) as i32 - 1, fy - 7, 'o');
                }
            }
        }
    }
    for g in &f.grips {
        let (Some(owner), Some(foe)) = (run.players.get(&g.owner), run.enemies.iter().find(|e| e.id == g.foe)) else {
            continue;
        };
        let (ox, oy) = at(owner.x, owner.y);
        let (fx, fy) = at(foe.x, foe.y);
        let (hx, hy) = hoist(run, foe);
        let (fx, fy) = (fx + hx, fy + hy);
        // The chain up to the hook in it, taut, its links lit by turns.
        let l = owner.aim_x.hypot(owner.aim_y).max(0.01);
        let hand = (ox + (owner.aim_x / l * 6.0) as i32, oy - 12);
        let n = ((fx - hand.0).abs().max((fy - 9 - hand.1).abs())).max(1);
        for k in 0..=n {
            let (x, y) = (hand.0 + (fx - hand.0) * k / n, hand.1 + (fy - 9 - hand.1) * k / n);
            cv.put(x, y, if k % 3 == 0 { 'h' } else { 'G' });
            cv.put(x, y + 1, 'j');
        }
        // The cuts, the blood and what it gives him are the ultimate's
        // own (`ults`); the foe's wounds are its ailment (`ailments`).
    }
}

/// How far the Butcher's hook has hoisted the monster he holds (a
/// `Grip`), in pixels: up off the floor on his chain and out along his
/// aim, swaying, jerked at every cut, and let down again at the end. Only
/// the figure goes up; its shadow stays where it is held, and what is too
/// big to hold (a guardian, a dragon, a dummy, a ward) stays on its feet.
pub(super) fn hoist(run: &Run, e: &crate::rules::Enemy) -> (i32, i32) {
    use super::pfx::ease;
    use crate::rules::EnemyKind;
    let Some(g) = run.champions.grips.iter().find(|g| g.foe == e.id) else { return (0, 0) };
    if e.boss.is_some() || matches!(e.kind, EnemyKind::Dragon | EnemyKind::Dummy | EnemyKind::Ward) {
        return (0, 0);
    }
    let Some(owner) = run.players.get(&g.owner) else { return (0, 0) };
    let age = champions::GRIP_TICKS.saturating_sub(g.left) as f32;
    let up = ease::out(age / 6.0) * (g.left as f32 / 5.0).min(1.0);
    let l = owner.aim_x.hypot(owner.aim_y);
    let (ax, ay) = if l > 0.01 { (owner.aim_x / l, owner.aim_y / l) } else { (0.0, 1.0) };
    // The cleaver falls on the tens (`champions::GRIP_EVERY`): a jolt.
    let cut = (run.tick % 10) as f32;
    let jolt = (3.0 - cut).max(0.0);
    let sway = (run.tick as f32 * 0.3).sin();
    (((ax * 6.0 + sway) * up).round() as i32, ((ay * 3.0 - 10.0 + jolt) * up).round() as i32)
}

/// His attachments belong to the twenty-pixel torso, not the wide canvas
/// swept by his arm. The same crouch and stride as `draw` move them.
pub(super) fn fit(hero: &Hero) -> crate::rules::looks::Body {
    let sink = i32::from(hero.champ.windup > 0) * 2
        + i32::from(matches!(super::class_looks::stride(hero), Some(1 | 3)));
    let mut fit = crate::rules::looks::Body::human((BODY_X, HEAD + sink), (20, BODY_H - sink));
    fit.head.anchor.1 = HEAD + sink + 3;
    fit.face.anchor.1 = HEAD + sink + 5;
    fit.body.anchor.1 = HEAD + sink + 11;
    fit.back.anchor.1 = HEAD + sink + 11;
    fit.shoulders.anchor.1 = HEAD + sink + 7;
    let c = champions::of(hero).unwrap_or(Champion::Hammerer);
    let (hx, hy) = match (facing(hero.aim_x, hero.aim_y), c) {
        (Facing::Side, Champion::Hammerer) => (13, 11),
        (Facing::Side, Champion::Butcher) => (16, 12),
        (Facing::Side, Champion::Alchemist) => (15, 11),
        (Facing::Side, Champion::Reaver) => (14, 11),
        (Facing::Side, Champion::Lancer) => (14, 11),
        (Facing::Down, Champion::Hammerer) => (18, 13),
        (Facing::Down, Champion::Butcher) => (18, 12),
        (Facing::Down, Champion::Alchemist) => (17, 12),
        (Facing::Down, Champion::Reaver) => (18, 12),
        (Facing::Down, Champion::Lancer) => (18, 12),
        (Facing::Up, _) => (2, 13),
    };
    fit.right.anchor = (BODY_X + hx, HEAD + sink + hy);
    fit.left.anchor = (BODY_X + 2, HEAD + sink + 12);
    fit
}

pub(super) fn floating_figure(hero: &Hero, c: Champion, _tick: u64, _id: u32) -> Img {
    draw(c, facing(hero.aim_x, hero.aim_y), super::class_looks::stride(hero), grip(hero, c), hero.champ.fury > 0 || hero.champ.lust > 0, hero.champ.windup > 0, hero.champ.fury.max(hero.champ.lust), Some((hero.aim_x, hero.aim_y)))
}

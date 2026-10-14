//! The knights' bodies in the arena: the realm's own knight and the dwarf,
//! each whole, bare-headed and in smallclothes; the shield raised toward the
//! way it is held; the dwarf's axes in their spin; the helmet and tabard
//! flung off as health fails.
//!
//! Everything here is hand-drawn ink rows in the kit's hand, sixteen pixels
//! square. The shield a knight raises is the shield on their arm: the same
//! red field, gold rim and star, brought round to face the aim.

use super::super::ink::Img;
use super::super::kit;
use super::class_looks::{self, Look, Pose};
use crate::rules::imps::Behaviour;
use crate::rules::looks::Sprite;
use crate::rules::{Hero, gear, spin};

fn rows(rows: &[&str]) -> Img {
    Img::from_rows(rows)
}

/// A figure drawn as its left half, mirrored: the dwarf stands square on.
fn mirrored(halves: &[&str]) -> Img {
    let full: Vec<String> = halves
        .iter()
        .map(|half| {
            let back: String = half.chars().rev().collect();
            format!("{half}{back}")
        })
        .collect();
    let refs: Vec<&str> = full.iter().map(String::as_str).collect();
    rows(&refs)
}

/// The way a raised shield faces on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Facing {
    /// Across: held out toward the side the knight faces (drawn facing
    /// right; the figure is flipped for left).
    Side,
    /// Toward the viewer: the shield covers the knight's front.
    Down,
    /// Away: the shield is held ahead, behind the knight on the screen.
    Up,
}

pub(super) fn facing(ax: f32, ay: f32) -> Facing {
    if ay.abs() > ax.abs() * 1.25 {
        if ay < 0.0 { Facing::Up } else { Facing::Down }
    } else {
        Facing::Side
    }
}

// ─── the realm's knight ──────────────────────────────────────────────────────

/// The kit knight at a stage of dress: 0 whole, 1 bare-headed, 2 in
/// smallclothes with the shield still on his arm.
fn kit_body(stage: u8) -> Img {
    match stage {
        0 => kit::knight(),
        1 => rows(&[
            "................",
            "......BBBB......",
            ".....BRRRRB.....",
            "....BRttttRB....",
            "....BttttttB....",
            "....BtkttktB....",
            "....BttttttB....",
            "......oOOo......",
            "..iHh777777hHi..",
            ".4444477577hJ...",
            ".4757475557hJ...",
            ".4555477577GJ...",
            ".47574BB4BBG....",
            "..474.87778.....",
            "...4..hJ.hJ.....",
            "......GG.GG.....",
        ]),
        _ => rows(&[
            "................",
            "......BBBB......",
            ".....BRRRRB.....",
            "....BRttttRB....",
            "....BttttttB....",
            "....BtkttktB....",
            "....BttttttB....",
            "......oOOo......",
            "..oOtttttttOo...",
            ".44444tttttOo...",
            ".47574ttOttOo...",
            ".45554tttttOo...",
            ".47574VVVVVv....",
            "..474.VVvVV.....",
            "...4..tt.tt.....",
            "......OO.OO.....",
        ]),
    }
}

// ─── the dwarf ───────────────────────────────────────────────────────────────

/// Dame Thrudi, square on: a horned iron cap, a braided ginger beard to the
/// belt, an axe in each fist. Stage 1 trades the cap for her own hair; stage 2
/// leaves her bare-armed in her smallclothes, beard and axes kept.
fn dwarf_body(stage: u8, axes: bool) -> Img {
    let head: [&str; 4] = match stage {
        0 => ["....T...", "....TiHH", ".HH..iHh", "HWWi.Jjj"],
        _ => ["........", "....rRRr", ".HH.rRRR", "HWWirRRR"],
    };
    let face = ["HWWiptkt", "HWWipttO"];
    let arms: [&str; 3] = if stage >= 2 {
        ["..BttRoR", "..BOtRRr", "..BttRoR"]
    } else {
        ["..BhhRoR", "..BJhRRr", "..BhhRoR"]
    };
    let legs: [&str; 5] = if stage >= 2 {
        ["..B.VVVV", "..B.VvVV", "..b.tt..", "....tt..", "....OO.."]
    } else {
        ["..BjBBBT", "..B.JGGj", "..b.hh..", "....GG..", "....gg.."]
    };
    let mut halves: Vec<&str> = Vec::new();
    halves.extend(head);
    halves.extend(face);
    halves.extend([".hh.RoRo", "..B.RRoR"]);
    halves.extend(arms);
    halves.extend(legs);
    let mut im = mirrored(&halves);
    if !axes {
        // The spin draws them: clear the hafts and blades at her sides.
        for y in 0..16 {
            for x in (0..4).chain(12..16) {
                im.clear(x, y);
            }
        }
    }
    im
}

// ─── the raised shield ───────────────────────────────────────────────────────

/// The shield held to the front: the knight's own, brought round.
fn shield_front() -> Img {
    rows(&[
        "44444444", "47777774", "47755774", "47555574", ".475574.", "..4774..", "...44...",
    ])
}

/// Held out to the right, three-quarters on.
fn shield_side() -> Img {
    rows(&[
        "444444", "477774", "475574", "475574", "477774", ".4774.", ".4774.", "..44..",
    ])
}

/// Wide and low, to sit behind the figure and show at its shoulders.
fn shield_far() -> Img {
    rows(&[
        ".4444444444.",
        "447777777744",
        "477755557774",
        "477555555774",
        "477755557774",
        ".4777777774.",
    ])
}

/// The kit knight's shield arm with no shield on it: the rest shield
/// (columns 1..5 of rows 9..14) taken off, the arm bare until it comes round
/// or is thrown.
fn bare_arm(im: &mut Img) {
    for y in 9..15 {
        for x in 1..6 {
            im.clear(x, y);
        }
    }
    for y in 9..12 {
        im.put(2, y, 'J');
    }
}

/// The shield up toward `facing`, over `body`. The knight's own shield, at
/// rest on the left arm, is taken off the figure first (`own`: the kit
/// knight carries it in his drawing; the dwarf picks hers up).
pub(super) fn raised(body: &Img, facing: Facing, own: bool) -> Img {
    let mut im = body.clone();
    if own {
        bare_arm(&mut im);
    }
    match facing {
        Facing::Down => {
            im.stamp(&shield_front(), 4, 8);
        }
        Facing::Side => {
            for y in 8..13 {
                im.clear(10, y);
                im.clear(11, y);
            }
            im.stamp(&shield_side(), 9, 6);
        }
        Facing::Up => {
            let mut out = Img::new(16, 16);
            out.stamp(&shield_far(), 2, 3);
            out.stamp(&im, 0, 0);
            im = out;
        }
    }
    im
}

/// Colours a seat's knight wears when the company gave none.
pub(super) fn seat_colours(id: u32) -> &'static [(char, char)] {
    match id {
        1 => &[],
        2 => &[('7', '1'), ('8', '0'), ('5', '9')],
        3 => &[('7', 'A'), ('8', 'l'), ('5', '6')],
        _ => &[('7', '4'), ('8', 'a'), ('5', 'c')],
    }
}

/// The dwarf's kit id.
pub const DWARF: &str = "ironbraid";

/// A knight of the company as the portrait shows them: whole, at rest.
pub fn of_kit(id: Option<&str>, colours: &[(char, char)]) -> Img {
    // A kit that is a class stands as its class, in the first seat's trim.
    if let Some(look) = id.and_then(Look::of_kit) {
        return class_looks::figure(look, Facing::Down, Pose::Stand, true);
    }
    if id == Some(DWARF) {
        dwarf_body(0, true)
    } else {
        kit_body(0).recolor(colours)
    }
}

/// A knight standing square on for a bezel or a ghost: their class if they
/// have one, else the realm's knight in their colours.
pub(super) fn portrait(hero: &Hero, id: u32) -> Img {
    match Look::of(hero) {
        Some(look) => class_looks::figure(look, Facing::Down, Pose::Stand, true).recolor(seat_colours(id)),
        None if hero.colours().is_empty() => super::sprites::knight(id),
        None => super::sprites::knight_in(hero.colours()),
    }
}

/// A hero as the arena draws them, in the colours of their seat, at the
/// stage of dress their health leaves them, with their shield (if they
/// hold one up) toward its aim. `None` for a hero drawn another way.
pub fn figure(hero: &Hero, id: u32) -> Img {
    figure_with(hero, id, false)
}

/// As `figure`, with the shield thrown (`thrown`): the arm is bare and
/// nothing can be raised.
pub fn figure_with(hero: &Hero, id: u32, thrown: bool) -> Img {
    if let Some(look) = Look::of(hero) {
        return hatted(class_figure(hero, id, thrown, look), hero);
    }
    let dwarf = hero.knight.as_deref() == Some(DWARF);
    let stage = hero.gear.min(2);
    let mut im = if dwarf {
        dwarf_body(stage, hero.spin == 0 && !hero.unarmed())
    } else {
        kit_body(stage)
    };
    if hero.helmet_worn() && stage >= 1 {
        let head = if dwarf { dwarf_body(0, false) } else { kit_body(0) };
        for y in 0..8 { for x in 0..16 { im.clear(x, y); if let Some(c) = head.get(x, y) { im.set(x, y, c); } } }
    }
    if thrown || !hero.owned.shield {
        if !dwarf {
            bare_arm(&mut im);
            if stage >= 2 { for y in 9..12 { im.put(2, y, 't'); } }
        }
    } else if hero.shielding {
        im = raised(&im, facing(hero.aim_x, hero.aim_y), !dwarf);
    }
    let im = if dwarf {
        im
    } else if hero.colours().is_empty() {
        im.recolor(seat_colours(id))
    } else {
        im.recolor(hero.colours())
    };
    hatted(im, hero)
}

/// A knight of a class, as that class: facing the aim, on their stride or
/// in their strike, in the seat's trim. A class keeps its look as health
/// fails (the knight's helmet and tabard stay the knight's); the health bar
/// says how they fare.
fn class_figure(hero: &Hero, id: u32, thrown: bool, look: Look) -> Img {
    let face = facing(hero.aim_x, hero.aim_y);
    let raising = look.shielded() && hero.shielding && !thrown && hero.owned.shield;
    let held = if look.shielded() {
        !thrown && hero.owned.shield && !raising
    } else {
        !hero.unarmed() && !(look == Look::Berserker && hero.spin > 0)
    };
    let pose = if raising { Pose::Stand } else { class_looks::pose(hero) };
    let mut im = class_looks::figure(look, face, pose, held);
    if raising {
        // Her kite shield, brought round to the aim: steel-rimmed like the
        // one on her arm.
        let off = (class_looks::W - 16) / 2;
        match face {
            Facing::Down => im.stamp(&shield_front().recolor(&[('4', 'h')]), off + 4, 8),
            Facing::Side => im.stamp(&shield_side().recolor(&[('4', 'h')]), off + 10, 6),
            Facing::Up => {
                let mut out = Img::new(im.w, im.h);
                out.stamp(&shield_far().recolor(&[('4', 'h')]), off + 2, 4);
                out.stamp(&im, 0, 0);
                im = out;
            }
        }
    }
    im.recolor(seat_colours(id))
}

/// What a game against friends left on the knight's helm: the winner's gold
/// crown, or the game's booby prize until they win one back.
fn hat(hero: &Hero) -> Option<Img> {
    use crate::rules::pastimes::Game;
    if hero.laurel {
        return Some(rows(&[
            "..6..6..6...",
            "..66.66.66..",
            "..55555555..",
            "..44444444..",
        ]));
    }
    Some(match hero.booby? {
        Game::Pot => rows(&[
            "...HHHHHH...",
            "..HWWWWWWH..",
            "..HWWWWWWHH.",
            "..HWWWWWWH.H",
            "..HHHHHHHHH.",
        ]),
        Game::Chariot | Game::Turnip => rows(&[
            "...lmmml....",
            "..lmYmYml...",
            "..mYmmmYm...",
            "...lmmml....",
        ]),
        Game::DuckShoot => rows(&[
            "......55....",
            ".....5K5o...",
            "..55555.....",
            "...555......",
        ]),
        Game::Trencher | Game::Cheese => rows(&[
            "...tTTTTt...",
            "..tooooooT..",
            "..oooooooo..",
        ]),
        // An apple on the helm, an arrow clean through it.
        Game::Longbow => rows(&[
            "......e.....",
            "....7777....",
            "hoo777777otT",
            "....7777....",
        ]),
        Game::Surf | Game::Washtub => rows(&[
            "..l.l..l....",
            "..l.ll.l.l..",
            ".lllllllll..",
        ]),
        _ => rows(&[
            ".....7......",
            "....797.....",
            "...79997....",
            "..7999997...",
        ]),
    })
}

/// The knight with their hat on, standing on the same feet.
fn hatted(im: Img, hero: &Hero) -> Img {
    let Some(hat) = hat(hero) else {
        return im;
    };
    let lift = hat.h - 1;
    let mut out = Img::new(im.w.max(hat.w), im.h + lift);
    out.stamp(&im, 0, lift);
    out.stamp(&hat, (im.w - hat.w) / 2, 0);
    out
}

// ─── what cards put on the field ─────────────────────────────────────────────

/// A frame of a card's sprite, as an image.
pub fn frame_img(rows: &[String]) -> Img {
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    Img::from_rows(&refs)
}

/// The sprites a knight wears, over their body, oldest under newest: each
/// frame centred on the figure. The canvas grows for what stands proud of
/// the figure at the sides and above; below the feet is cut, so the knight
/// stands where they stood.
pub fn dress(body: &Img, worn: &[&Sprite], tick: u64) -> Img {
    if worn.is_empty() {
        return body.clone();
    }
    let frames: Vec<Img> = worn.iter().map(|s| frame_img(s.frame(tick))).collect();
    let pad_x = frames
        .iter()
        .map(|f| ((f.w - body.w).max(0) + 1) / 2)
        .max()
        .unwrap_or(0);
    let pad_top = frames
        .iter()
        .map(|f| ((f.h - body.h).max(0) + 1) / 2)
        .max()
        .unwrap_or(0);
    let mut out = Img::new(body.w + 2 * pad_x, body.h + pad_top);
    out.stamp(body, pad_x, pad_top);
    for f in &frames {
        out.stamp(f, pad_x + (body.w - f.w) / 2, pad_top + (body.h - f.h) / 2);
    }
    out
}

/// A knight who brought their own picture: the sprite at the arena's
/// sixteen pixels, with the shield brought round when it is raised (the
/// picture carries no shield of its own to take off).
pub fn avatar_figure(hero: &Hero, pixels: &[[u8; 4]], thrown: bool) -> Img {
    let mut im = Img::new(16, 16);
    for y in 0..16 {
        for x in 0..16 {
            if let Some(&[r, g, b, a]) = pixels.get((y * 2 * 32 + x * 2) as usize)
                && a >= 128
            {
                im.set(x, y, [r, g, b]);
            }
        }
    }
    if hero.shielding && !thrown {
        im = raised(&im, facing(hero.aim_x, hero.aim_y), false);
    }
    im
}

/// A summoned creature drawn by no card: a small horned imp for a swarm, a
/// lantern for a guard, a little bolt-thrower for a turret. Two frames.
pub fn imp_default(behaviour: Behaviour, tick: u64) -> Img {
    let up = (tick / 6) % 2 == 0;
    match behaviour {
        Behaviour::Swarm => rows(if up {
            &[
                "5.....5", ".5...5.", ".45554.", "4577754", ".45554.", "..4.4..", ".4...4.",
            ]
        } else {
            &[
                "5.....5", ".5...5.", ".45554.", "4577754", ".45554.", ".4...4.", "..4.4..",
            ]
        }),
        Behaviour::Guard => rows(if up {
            &["..H..", ".h3h.", "h323h", ".h3h.", "..h.."]
        } else {
            &["..h..", ".h2h.", "h232h", ".h2h.", "..h.."]
        }),
        Behaviour::Turret => rows(&[
            "...JJ..", "..JhhJ.", ".JhGGh4", "JhG..h4", ".JhGGh.", "..PPPP.", ".PPRRPP",
        ]),
    }
}

// ─── the spin ────────────────────────────────────────────────────────────────

/// The dwarf's axes in their spin, round the point `(x, y)` in native
/// pixels (her middle). Two axes opposite each other turn through the
/// reach of the cut, each trailing a band that fades behind it, and the
/// circle they cover lights as each cuts.
pub fn spin_marks(cv: &mut Img, x: i32, y: i32, left: u32, reach_px: f32) {
    let age = spin::TICKS.saturating_sub(left);
    let turn = age as f32 * 0.9;
    let r = reach_px * 0.82;
    let squash = 0.74;
    let at = |a: f32, radius: f32| {
        (
            x + (a.cos() * radius).round() as i32,
            y + (a.sin() * radius * squash).round() as i32,
        )
    };
    // The circle they cover: faint, and bright where a cut has just landed.
    let cutting = spin::sweeping(age);
    for k in 0..72 {
        let a = k as f32 / 72.0 * std::f32::consts::TAU;
        if cutting || k % 2 == 0 {
            let (px, py) = at(a, reach_px);
            cv.put(px, py, if cutting { 'i' } else { 'G' });
        }
    }
    for axe in 0..2 {
        let base = turn + axe as f32 * std::f32::consts::PI;
        // The trail: a band two thick, brightest at the head.
        const FADE: [char; 7] = ['H', 'i', 'h', 'J', 'G', 'j', 'g'];
        for (n, ink) in FADE.iter().enumerate() {
            for back in 0..2 {
                let a = base - (n as f32 * 2.0 + back as f32) * 0.11;
                for dr in [0.0, -1.0] {
                    let (px, py) = at(a, r + dr);
                    cv.put(px, py, *ink);
                }
            }
        }
        // The haft from her fist, two wide.
        let (c, s) = (base.cos(), base.sin() * squash);
        for k in 4..(r as i32 - 1) {
            let (px, py) = (
                x + (c * k as f32).round() as i32,
                y + (s * k as f32).round() as i32,
            );
            cv.put(px, py, 'B');
            cv.put(px + 1, py, 'r');
        }
        // The head: a blade across the haft, its edge forward, wider at the
        // cutting edge than where it meets the wood.
        let (hx, hy) = at(base, r);
        let (px, py) = (-s, c);
        for (back, half, ink) in [
            (0.0, 3.6, 'H'),
            (-1.0, 3.2, 'i'),
            (-2.0, 2.2, 'h'),
            (-3.0, 1.2, 'J'),
        ] {
            let mut t = -half;
            while t <= half {
                cv.put(
                    (hx as f32 + c * back + px * t).round() as i32,
                    (hy as f32 + s * back + py * t).round() as i32,
                    ink,
                );
                t += 0.5;
            }
        }
    }
}

// ─── the flung gear ──────────────────────────────────────────────────────────

fn helmet() -> Img {
    rows(&[
        "...77...", "..i77i..", ".iHHhhJ.", ".HhhhhhJ", ".hgggggG", ".JhhhhhG",
    ])
}

/// The dwarf's horned cap, and her mail with its belt.
fn horned_cap() -> Img {
    rows(&["T......T", "TiHHHHiT", ".iHhhHi.", ".JjjjjJ."])
}

fn mail_shirt() -> Img {
    rows(&[
        "hh.....hh",
        "hJhhhhhJh",
        ".hJhhhJh.",
        ".hhhhhhh.",
        ".BBBTBBB.",
    ])
}

fn tabard() -> Img {
    rows(&[
        "HH.....HH",
        "iH77777Hi",
        ".7755577.",
        ".7555557.",
        ".7755577.",
        "..77.77..",
    ])
}

/// What flew off as health failed, in the air for `gear::POP_TICKS`: the
/// helmet pops straight up and back, the tabard after it the other way.
/// `(x, y)` is the figure's feet; `away` is -1 or 1, the side it flies to.
pub(super) fn flung(cv: &mut Img, hero: &Hero, x: i32, y: i32, away: i32) {
    let dwarf = hero.knight.as_deref() == Some(DWARF);
    // A class keeps its look: nothing of it flies off.
    if hero.pop == 0 || Look::of(hero).is_some() {
        return;
    }
    let age = gear::POP_TICKS - hero.pop.min(gear::POP_TICKS);
    let helm = hero.owned.helmet && hero.gear_was == 0 && hero.gear >= 1;
    let coat = hero.owned.body && hero.gear >= 2 && hero.gear_was < 2;
    let piece = |im: Img, delay: u32, dir: i32, lift: f32, from: i32| {
        if age < delay {
            return None;
        }
        let t = (age - delay) as f32 / (gear::POP_TICKS - delay) as f32;
        let px = x + dir * (t * 15.0) as i32;
        let py = y - from + (t * t * (from as f32 - 2.0)) as i32
            - ((t * std::f32::consts::PI).sin() * lift) as i32;
        let mut turned = im;
        for _ in 0..(((age - delay) / 3) % 4) {
            turned = super::sprites::quarter_turn(&turned);
        }
        // It rests, then goes: blink out over the last few ticks.
        if t > 0.8 && (age / 2) % 2 == 0 {
            return None;
        }
        Some((turned, px, py))
    };
    if helm
        && let Some((im, px, py)) = piece(
            if dwarf { horned_cap() } else { helmet() },
            0,
            away,
            20.0,
            13,
        )
    {
        cv.stamp(&im, px - im.w / 2, py - im.h / 2);
    }
    if coat
        && let Some((im, px, py)) = piece(
            if dwarf { mail_shirt() } else { tabard() },
            if helm { 4 } else { 0 },
            -away,
            14.0,
            9,
        )
    {
        cv.stamp(&im, px - im.w / 2, py - im.h / 2);
    }
}

/// Six desperate poses and six waddle steps, using the cast's own face and
/// palette. Kit pieces follow their flight timestamps, rather than health.
pub(super) fn pit_figure(hero: &Hero, id: u32, tick: u32) -> Img {
    use crate::rules::pitstop::{FLIGHT_TICKS, Grade, Quality, Stage};
    let stop = hero.pit.stop.as_ref();
    let worn: [bool; 3] = std::array::from_fn(|piece| {
        stop.is_none_or(|s| {
            if s.stage == Stage::Strip {
                s.tossed[piece].is_none() && usize::from(s.straps) <= piece
            } else if s.stage == Stage::Dress {
                s.fitted[piece].is_some_and(|t| s.total.saturating_sub(t) >= FLIGHT_TICKS)
                    || (s.fitted[piece].is_none() && usize::from(s.straps) > 2 - piece)
            } else {
                false
            }
        })
    });
    let running = stop.is_some_and(|s| matches!(s.stage, Stage::RunIn | Stage::RunOut));
    let fumble =
        stop.is_some_and(|s| s.lock > 0 && s.last.is_some_and(|(g, _)| g == Grade::Missed));
    let frame = (tick / if running { 2 } else { 4 } % 6) as usize;
    let lean = if running {
        [0, 1, 1, 0, -1, -1][frame]
    } else {
        [0, 0, 1, 1, 0, -1][frame]
    };
    let mut im = Img::new(20, 22);
    let head = if hero.knight.as_deref() == Some(DWARF) {
        dwarf_body(if worn[0] { 0 } else { 2 }, false)
    } else {
        kit_body(if worn[0] { 0 } else { 2 })
    };
    for y in 0..8 {
        for x in 4..12 {
            if let Some(c) = head.get(x, y) {
                im.set(x + 2 + lean, y + 1, c);
            }
        }
    }
    let torso = if worn[2] {
        rows(&[
            "..iHHHHHi..",
            ".h7777777h.",
            ".h7757577h.",
            ".h7755577h.",
            "..G7757G..",
            "..BB4BB...",
        ])
    } else {
        rows(&[
            "..oOttOo..",
            ".OttttttO.",
            ".OttOtttO.",
            "..tttttt..",
            "..VVVVVv..",
            "..VVvVV...",
        ])
    };
    im.stamp(&torso, 5 + lean, 9);
    if worn[1] {
        im.stamp(&rows(&["iHHh", "hJJG", ".gg."]), 14 + lean, 9);
    }
    let arm = if worn[1] { 'h' } else { 't' };
    let quality = hero.pit.result.as_ref().map(|r| r.quality);
    if quality == Some(Quality::Royal) {
        im.line(5, 11, 2, 7, arm);
        im.rect(1, 5, 2, 2, 't');
        im.line(15, 11, 18, 7, arm);
        im.rect(17, 5, 2, 2, 't');
    } else if quality == Some(Quality::Fresh) {
        im.line(5, 10, 4, 14, arm);
        im.put(4, 14, 't');
        im.line(15, 11, 18, 10, arm);
        im.rect(17, 8, 2, 3, 't');
        im.put(18, 7, 'O');
    } else if fumble {
        im.line(5, 11, 13, 7, arm);
        im.line(15, 11, 7, 7, 'O');
        im.put(8, 7, 't');
        im.put(12, 7, 't');
        // A loose strap tangles around the elbows.
        im.line(3, 12, 7, 15, 'P');
        im.line(7, 15, 16, 8, 'P');
    } else if running {
        let reach = [-3, -1, 2, 3, 1, -2][frame];
        im.line(5 + lean, 10, 5 + reach, 14, arm);
        im.line(15 + lean, 10, 15 - reach, 13, arm);
        im.put(5 + reach, 14, 'O');
        im.put(15 - reach, 13, 'O');
    } else {
        let tug = [0, 1, 2, 1, 0, -1][frame];
        im.line(5 + lean, 10, 5, 13, arm);
        im.line(5, 13, 9 + tug, 14, 'O');
        im.line(15 + lean, 10, 15, 13, arm);
        im.line(15, 13, 12 - tug, 14, 't');
        im.put(10, 14, 'O');
    }
    let boot = if worn[2] { 'G' } else { 'O' };
    let leg = if worn[2] { 'h' } else { 't' };
    if running {
        let stride = [-3, -2, 1, 3, 2, -1][frame];
        let lift = [0, 1, 2, 0, 1, 2][frame];
        im.line(9, 15, 9 + stride, 18 - lift, leg);
        im.line(12, 15, 12 - stride, 18 - (2 - lift), leg);
        im.rect(7 + stride, 18 - lift, 4, 2, boot);
        im.rect(11 - stride, 18 - (2 - lift), 4, 2, boot);
    } else {
        let lift = [0, 1, 3, 2, 1, 0][frame];
        let left = frame < 3;
        im.line(
            9,
            15,
            if left { 8 } else { 10 },
            18 - if left { lift } else { 0 },
            leg,
        );
        im.line(
            12,
            15,
            if left { 11 } else { 13 },
            18 - if left { 0 } else { lift },
            leg,
        );
        im.rect(
            if left { 6 } else { 8 },
            19 - if left { lift } else { 0 },
            4,
            2,
            boot,
        );
        im.rect(
            if left { 10 } else { 12 },
            19 - if left { 0 } else { lift },
            4,
            2,
            boot,
        );
    }
    im.recolor(if hero.colours().is_empty() {
        seat_colours(id)
    } else {
        hero.colours()
    })
}

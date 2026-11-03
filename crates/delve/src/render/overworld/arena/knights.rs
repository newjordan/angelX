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
use super::super::kit::hero::{self as realm, Dress, Held, View};
use super::quick_rig::{self, Clip, Playback, Rig, Sample};
use super::class_looks::{self, Look, Pose};
use crate::rules::imps::Behaviour;
use crate::rules::looks::{Body, Sprite};
use crate::rules::identity::Shape;
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

fn view_of(facing: Facing) -> View {
    match facing {
        Facing::Down => View::Toward,
        Facing::Side => View::Across,
        Facing::Up => View::Away,
    }
}

/// Where the knight is in their stride, as a step of the hero's drawing:
/// 0 standing, then a foot planted, passing, the other foot, passing.
pub(super) fn stride_step(hero: &Hero) -> u8 {
    class_looks::stride(hero).map_or(0, |phase| phase + 1)
}

/// What the knight has in hand, and whether it is in its strike: the
/// blade while it swings, else the arm they carry (a forged blade is a
/// sword), drawn or fired while it rearms.
fn held_of(hero: &Hero) -> (Held, bool) {
    use crate::rules::Weapon;
    let blade = hero.forged.as_ref().is_some_and(|w| w.melee.is_some());
    if hero.swing > 0 && (hero.sword || blade) {
        return (Held::Sword, true);
    }
    let held = if blade {
        Held::Sword
    } else {
        match hero.weapon {
            Weapon::Fists | Weapon::Notes | Weapon::Stones => Held::Nothing,
            Weapon::Bow => Held::Bow,
            Weapon::Crossbow => Held::Crossbow,
            Weapon::Handgonne => Held::Handgonne,
        }
    };
    (held, held != Held::Nothing && (hero.fire_cooldown > 0 || hero.swing > 0))
}

/// The realm's knight as health, gear and aim leave them: helm on or off,
/// in harness or smallclothes, the shield on the arm (owned or held,
/// unless it is thrown or raised: `figure_with` brings it round), on their
/// stride, with what they hold.
fn realm_body(hero: &Hero, thrown: bool, floating: bool) -> Img {
    let stage = hero.gear.min(2);
    let (held, strike) = held_of(hero);
    realm::draw(Dress {
        view: view_of(facing(hero.aim_x, hero.aim_y)),
        helm: !hero.look.custom && (stage == 0 || hero.helmet_worn()),
        harness: stage < 2,
        shield: !thrown && (hero.owned.shield || hero.holds_shield()) && !hero.shielding,
        step: stride_step(hero),
        held: if floating { Held::Nothing } else { held },
        strike,
    })
}

// Quick rigs deliberately opt out of authored identities and missing-gear
// stages. Those paths carry their own silhouettes, helmets and save semantics.
// A plain knight is drawn as their seat's design (`quick_rig::Design`).
fn default_rig(hero: &Hero, seat: u32) -> Option<&'static Rig> {
    if hero.avatar.is_some()
        || hero.shape.is_some()
        || hero.pit.stop.is_some()
        || hero.possessed.is_some()
        || hero.frog()
        || hero.peasant()
        || hero.gear != 0
        || !hero.owned.helmet
        || !hero.owned.body
        || hero.knight.is_some()
        || crate::rules::champions::of(hero).is_some()
        || Look::of(hero).is_some()
        || hero.look.custom
    {
        return None;
    }
    quick_rig::seat_knight(seat)
}

/// Gameplay's countdowns give one-shot origins without storing animation in a
/// Hero. Locomotion uses distance travelled; casts use their event age.
pub(super) fn rig_sample<'a>(rig: &'a Rig, hero: &Hero, tick: u64, casting: bool) -> Sample<'a> {
    rig_sample_at(
        rig,
        hero,
        tick,
        casting.then_some(0),
        melee_clip(hero, None),
    )
}

// Cards/forge runes have names and melee arcs, but no typed animation class.
// Prefer a recognized equipped-card label, then the forge label, then geometry.
fn melee_clip(hero: &Hero, book: Option<&crate::rules::cards::Book>) -> Clip {
    if let Some(clip) = book.and_then(|book| super::factory::equipped_attack_clip(hero, book)) {
        return clip;
    }
    let named = |name: &str| {
        let name = name.to_ascii_lowercase();
        let words = || name.split(|c: char| !c.is_alphabetic());
        if words().any(|w| matches!(w, "flail" | "morningstar")) {
            Some(Clip::Flail)
        } else if words().any(|w| {
            matches!(
                w,
                "maul" | "hammer" | "warhammer" | "axe" | "battleaxe" | "greataxe" | "mace"
            )
        }) {
            Some(Clip::Overhead)
        } else if words().any(|w| matches!(w, "spear" | "pike" | "rapier" | "lance")) {
            Some(Clip::Thrust)
        } else if words().any(|w| matches!(w, "sword" | "blade" | "sabre" | "saber")) {
            Some(Clip::Slash)
        } else {
            None
        }
    };
    hero.arm
        .as_ref()
        .and_then(|id| book?.get(id))
        .filter(|c| c.arm.is_some())
        .and_then(|c| named(&c.name))
        .or_else(|| hero.forged.as_ref().and_then(|w| named(&w.name)))
        .unwrap_or_else(|| {
            if hero
                .forged
                .as_ref()
                .and_then(|w| w.melee)
                .is_some_and(|m| m.arc <= 45.0)
            {
                Clip::Thrust
            } else {
                Clip::Slash
            }
        })
}

/// One authoritative sample for the live image and all its attachments.
pub(super) fn rig_sample_event<'a>(
    rig: &'a Rig,
    hero: &Hero,
    book: &crate::rules::cards::Book,
    tick: u64,
    cast_age: Option<u64>,
) -> Sample<'a> {
    rig_sample_at(rig, hero, tick, cast_age, melee_clip(hero, Some(book)))
}

fn rig_sample_at<'a>(
    rig: &'a Rig,
    hero: &Hero,
    tick: u64,
    cast_age: Option<u64>,
    melee: Clip,
) -> Sample<'a> {
    if hero.hp == 0 {
        return rig.sample(
            Clip::Die,
            tick.saturating_sub(hero.fell),
            Playback::Once,
            false,
        );
    }
    if let Some(left) = hero.rolling() {
        return rig.action(Clip::Roll, left, 10, false);
    }
    if hero.struck > 0 {
        return rig.action(
            Clip::Hurt,
            hero.struck,
            crate::rules::spectacle::STRUCK,
            false,
        );
    }
    // The six-tick hit flash is not the attack period. Keep anticipation,
    // contact and recovery on the same cooldown, including slow heavy arms.
    if !hero.sword && (hero.swing > 0 || hero.fire_cooldown > 0) {
        if let Some(m) = hero
            .forged
            .as_ref()
            // A hybrid snapshot has no last-fired mode after swing expires.
            // Do not claim a ranged shot is a melee attack in that case.
            .filter(|w| w.bolt.is_none() || hero.swing > 0)
            .and_then(|w| w.melee)
        {
            let base = if hero.champ.fury > 0 {
                m.every / 2
            } else {
                m.every
            };
            let total = (base * 100
                / (100 + hero.bonus.rate + hero.boon.rate + hero.hero_kit.attack_rate())
                * hero.curio_wait()
                / 100)
                .max(1);
            return rig.action(melee, hero.fire_cooldown, total, false);
        }
    }
    if hero.swing > 0 {
        return rig.action(
            Clip::Slash,
            hero.swing,
            if hero.sword { 8 } else { 6 },
            false,
        );
    }
    if let Some(age) = cast_age {
        return rig.sample(Clip::Cast, age, Playback::Once, false);
    }
    if hero.fire_cooldown > 0 {
        let clip = match hero.weapon {
            crate::rules::Weapon::Bow => Clip::Bow,
            crate::rules::Weapon::Crossbow | crate::rules::Weapon::Handgonne => Clip::Gun,
            _ => Clip::Cast,
        };
        let base = hero
            .forged
            .as_ref()
            .and_then(|w| w.bolt)
            .map_or(hero.weapon.arms().cooldown, |b| b.every);
        let rate = 100 + hero.bonus.rate + hero.boon.rate + hero.hero_kit.attack_rate();
        let total = (base * 100 / rate * hero.curio_wait() / 100).max(1);
        // Runtime sheets and old looks use the gameplay rearm period; the
        // integer countdown stays stable across redraws and client mirrors.
        return rig.action(clip, hero.fire_cooldown, total, false);
    }
    if hero.stride > 0.0 && hero.stride.is_finite() {
        // One exported cycle per gameplay stride; no frame-rate dependent state.
        let fast = (100 + hero.bonus.speed) as f32 / 100.0 * hero.curio_pace();
        let clip = if hero.hero_kit.windrun > 0 || hero.flee_ticks > 0 || fast >= 1.35 {
            Clip::Run
        } else {
            Clip::Walk
        };
        let at = ((hero.stride / 3.4).fract() * rig.duration(clip) as f32) as u64;
        return rig.sample(clip, at, Playback::Loop, false);
    }
    rig.sample(Clip::Idle, tick, Playback::Loop, false)
}

// Odd, symmetric staging pins the exported pivot to the arena's centre column
// even for even-width or off-centre source canvases. Headroom accommodates gear.
fn stage(sample: &Sample<'_>) -> (i32, i32, i32) {
    let [w, _] = sample.image_size();
    let [px, _] = sample.pivot();
    let radius = px.max(w - 1 - px) + 12;
    (radius * 2 + 1, radius - px, 12)
}
fn staged_image(sample: &Sample<'_>) -> Img {
    let (w, dx, top) = stage(sample);
    let mut out = Img::new(w, sample.image_size()[1] + top);
    out.stamp(&sample.image(), dx, top);
    out
}
pub(super) fn staged_fit(sample: &Sample<'_>) -> Body {
    let (_, dx, top) = stage(sample);
    sample.body().shifted(dx, top)
}

fn rig_fit(sample: &Sample<'_>, hero: &Hero) -> Body {
    let mut fit = staged_fit(sample);
    // Match native cape depth: aiming away reveals the cape over the torso.
    fit.back.under = hero.aim_y >= 0.0;
    fit
}

fn knight_fit(sample: &Sample<'_>, hero: &Hero) -> Body {
    let mut fit = rig_fit(sample, hero);
    // The built-in knight is a native 16px character. Its idle ink bounds
    // exclude empty armour/hat space; they must not shrink ordinary gear.
    let native = crate::rules::looks::profile("realm_right");
    for (part, reference) in [
        (&mut fit.head, native.head),
        (&mut fit.face, native.face),
        (&mut fit.shoulders, native.shoulders),
        (&mut fit.body, native.body),
        (&mut fit.back, native.back),
        (&mut fit.left, native.left),
        (&mut fit.right, native.right),
        (&mut fit.feet, native.feet),
        (&mut fit.aura, native.aura),
    ] {
        part.size = reference.size;
        part.bottom = reference.bottom;
        part.stretch = reference.stretch;
    }
    // The shield sits outside the fist, as on the native knight. Its even
    // footprint straddles a socket pixel; choose the other inside edge when
    // mirrored so odd rig staging retains the native arm's 12px round-trip.
    fit.left.anchor.0 -= native.left.size.0 / 4;
    if hero.aim_x < -0.3 {
        fit.left.anchor.0 += 1;
    }
    fit
}

pub(super) fn rig_figure(
    hero: &Hero,
    id: u32,
    thrown: bool,
    struck: Struck,
    rig: &Rig,
    tick: u64,
    casting: bool,
) -> Img {
    rig_art(hero, thrown, struck, rig, tick, casting).recolor(if hero.colours().is_empty() {
        realm_colours(id)
    } else {
        hero.colours()
    })
}

fn rig_art(hero: &Hero, thrown: bool, struck: Struck, rig: &Rig, tick: u64, casting: bool) -> Img {
    let sample = rig_sample(rig, hero, tick, casting);
    rig_art_sample(hero, thrown, struck, &sample, true)
}

fn rig_art_sample(
    hero: &Hero,
    thrown: bool,
    struck: Struck,
    sample: &Sample<'_>,
    native_weapon: bool,
) -> Img {
    let mut im = staged_image(sample);
    let fit = rig_fit(sample, hero);
    let (held, _) = held_of(hero);
    // Keep old equipment paintings, but place their contact pixels at sockets.
    if !thrown && (hero.owned.shield || hero.holds_shield()) {
        let facing = facing(hero.aim_x, hero.aim_y);
        let (shield, contact) = if hero.shielding {
            match facing {
                Facing::Down => (shield_front(), [5, 3]),
                Facing::Side => (shield_side(), [2, 3]),
                Facing::Up => (shield_far(), [5, 3]),
            }
        } else {
            (realm::shield(view_of(facing)).0, [2, 3])
        };
        let a = fit.left.anchor;
        let (dx, dy, flash) = if hero.shielding {
            (struck.dx, struck.dy - 2, struck.flash)
        } else {
            (0, 0, 0)
        };
        let shield = flashed(&shield, flash);
        if hero.shielding && facing == Facing::Up {
            let mut under = Img::new(im.w, im.h);
            sample.attach(&mut under, &shield, contact, [a.0 + dx, a.1 + dy]);
            under.stamp(&im, 0, 0);
            im = under;
        } else {
            sample.attach(&mut im, &shield, contact, [a.0 + dx, a.1 + dy]);
        }
    }
    let a = fit.right.anchor;
    if native_weapon
        && (held == Held::Sword || hero.forged.as_ref().is_some_and(|w| w.melee.is_some()))
    {
        // The arm itself along the rig's grip-to-tip: blade, guard and
        // grip, or an axe's, a maul's, a spear's head. While the stroke
        // is young the sweep draws it out in the slash (`melee`).
        let sweeping = hero.swing >= if hero.sword { 6 } else { 4 };
        if !sweeping {
            let (_, dx, top) = stage(sample);
            let tip = sample.anchors().tip;
            let arm = super::blades::Arm::of(hero);
            let spear = arm == super::blades::Arm::Spear;
            // It goes where the rig's hand sends it; but an arm the rig
            // lets hang at rest or on the march is carried up and a little
            // forward instead, clear of the body, where its edge shows.
            let (mut tx, mut ty) = ((tip[0] + dx - a.0) as f32, (tip[1] + top - a.1) as f32);
            if ty > 0.0 && matches!(sample.clip, Clip::Idle | Clip::Walk | Clip::Run) {
                let out = (a.0 - im.w / 2).signum() as f32;
                (tx, ty) = (out * if spear { 0.12 } else { 0.75 }, -1.0);
            }
            let len = if spear { 15.0 } else { 11.0 };
            super::blades::draw(&mut im, arm, (a.0 as f32, a.1 as f32), (tx, ty), len, false);
        }
    } else if native_weapon {
        let equipment = if held == Held::Bow {
            // The quick-rig knights are smaller than the legacy canvas. Keep
            // the string behind the wooden curve and the grip on the fist.
            Some((rows(&["g.r.", "g..r", "r.Vr", "g..r", "g.r."]), (0, 2)))
        } else { realm::clock_equipment(held, held_of(hero).1) };
        if let Some((art, grip)) = equipment {
            im = super::wear::clocked(&im, &art, grip, 0.0,
                fit.right.anchor, (hero.aim_x, hero.aim_y));
        }
    }
    if let Some(hat) = hat(hero) {
        sample.attach(
            &mut im,
            &hat,
            [hat.w / 2, hat.h - 1],
            [fit.head.anchor.0, fit.head.anchor.1],
        );
    }
    im
}

/// What a rig knight carries over its body, as `rig_art_sample` stamps it,
/// for its name's dress (`figure_keys`): the arm in hand, the shield and the
/// way it faces, a hat; `bare` for none.
fn dress_of(hero: &Hero, thrown: bool, native_weapon: bool, struck: Struck) -> String {
    let mut parts: Vec<String> = Vec::new();
    let (held, _) = held_of(hero);
    if native_weapon && (held == Held::Sword || hero.forged.as_ref().is_some_and(|w| w.melee.is_some())) {
        // While the stroke is young the sweep draws the arm (`melee`).
        if hero.swing >= if hero.sword { 6 } else { 4 } {
            parts.push("sweep".into());
        } else {
            parts.push(format!("{:?}", super::blades::Arm::of(hero)).to_ascii_lowercase());
        }
    } else {
        match held {
            Held::Bow => parts.push("bow".into()),
            Held::Crossbow => parts.push("crossbow".into()),
            Held::Handgonne => parts.push("handgonne".into()),
            _ => {}
        }
    }
    if !thrown && (hero.owned.shield || hero.holds_shield()) {
        let view = match facing(hero.aim_x, hero.aim_y) {
            Facing::Down => "toward",
            Facing::Side => "across",
            Facing::Up => "away",
        };
        if hero.shielding {
            let knocked = struck.flash > 0 || struck.dx != 0 || struck.dy != 0;
            parts.push(format!("shield-up-{view}{}", if knocked { "-struck" } else { "" }));
        } else {
            parts.push(format!("shield-{view}"));
        }
    }
    if hero.laurel {
        parts.push("laurel".into());
    } else if let Some(game) = hero.booby.filter(|_| hat(hero).is_some()) {
        parts.push(format!("hat-{game:?}").to_ascii_lowercase());
    }
    if native_weapon && matches!(held, Held::Bow | Held::Crossbow | Held::Handgonne) {
        let (mut x, y) = (hero.aim_x, hero.aim_y);
        if x < -0.3 { x = -x; }
        let angle = if x.is_finite() && y.is_finite() && x.hypot(y) >= 0.001 {
            (y.atan2(x).to_degrees() * 10.0).round() as i32
        } else { 0 };
        parts.push(format!("aim-{angle}"));
    }
    if parts.is_empty() { "bare".into() } else { parts.join("+") }
}

fn wished_rig<'a>(
    hero: &Hero,
    book: &'a crate::rules::cards::Book,
) -> Option<(&'a Sprite, std::sync::Arc<Rig>)> {
    if hero.avatar.is_some()
        || hero.shape.is_some()
        || hero.pit.stop.is_some()
        || hero.possessed.is_some()
        || hero.frog()
    {
        return None;
    }
    // Only the newest whole-body sheet owns the body. An earlier registered
    // sheet must not jump over a newer, ordinary legacy wish.
    let sprite = hero
        .worn(book)
        .into_iter()
        .rev()
        .find(|s| s.at.as_deref().is_none_or(|at| at == "whole"))?;
    quick_rig::sprite_rig(sprite).map(|rig| (sprite, rig))
}

/// Inspect the body actually selected by composition, not its hidden base.
fn with_selected_rig<T>(
    hero: &Hero,
    book: &crate::rules::cards::Book,
    f: impl FnOnce(&Rig) -> T,
) -> Option<T> {
    // A newer ordinary whole sheet supersedes both an old registration and
    // the underlying default knight, including its roll and pivot metadata.
    if hero
        .worn(book)
        .into_iter()
        .any(|s| s.at.as_deref().is_none_or(|at| at == "whole"))
    {
        // Share composition's identity guards as well as its newest-sheet rule.
        return wished_rig(hero, book).as_ref().map(|(_, rig)| f(rig));
    }
    // Ordinary equipment does not replace the body or its animation metadata.
    // These callers measure geometry only (canvas, pivot, staging, whether a
    // roll is authored), which every seat's design shares with the first.
    default_rig(hero, 1).map(f)
}

/// The selected live body's capability, including ordinary worn equipment.
#[cfg(test)]
pub(super) fn rigged_roll(hero: &Hero, book: &crate::rules::cards::Book) -> bool {
    with_selected_rig(hero, book, |r| r.has(Clip::Roll)).unwrap_or(false)
}

pub(super) fn rigged_roll_in(im: &Img, hero: &Hero, book: &crate::rules::cards::Book) -> bool {
    with_selected_rig(hero, book, |r| {
        r.has(Clip::Roll)
            && im.h >= r.canvas()[1] + 12
            && im.w >= stage(&rig_sample(r, hero, 0, false)).0
    })
    .unwrap_or(false)
}

pub(super) fn rolled_hero(
    im: &Img,
    hero: &Hero,
    book: &crate::rules::cards::Book,
    left: u32,
) -> Img {
    if rigged_roll_in(im, hero, book) {
        im.clone()
    } else {
        rolled(im, left)
    }
}

/// Contact point in the actual composed canvas. Transform this point along
/// with pixels when tumbling/tossing; stone preserves it unchanged.
pub(super) fn rig_pivot(hero: &Hero, book: &crate::rules::cards::Book, im: &Img) -> [i32; 2] {
    with_selected_rig(hero, book, |rig| {
        let sample = rig_sample(rig, hero, 0, false);
        let (w, _, top) = stage(&sample);
        if im.w >= w && im.h >= rig.canvas()[1] + top {
            // wear::dressed pads sides symmetrically and only above the body.
            [im.w / 2, im.h - rig.canvas()[1] + rig.pivot()[1]]
        } else {
            [im.w / 2, im.h - 1]
        }
    })
    .unwrap_or([im.w / 2, im.h - 1])
}

/// Rotate pixels and their contact together. Non-rig legacy pictures retain
/// their historical bottom-centre placement after a tumble changes dimensions.
pub(super) fn tossed_figure(
    im: &Img,
    hero: &Hero,
    book: &crate::rules::cards::Book,
    turns: u32,
) -> (Img, [i32; 2]) {
    let rigged = with_selected_rig(hero, book, |r| {
        im.h >= r.canvas()[1] + 12 && im.w >= stage(&rig_sample(r, hero, 0, false)).0
    }).unwrap_or(false);
    tossed_at(im, rigged.then(|| rig_pivot(hero, book, im)), turns)
}

pub(super) fn tossed_at(im: &Img, pivot: Option<[i32; 2]>, turns: u32) -> (Img, [i32; 2]) {
    let rigged = pivot.is_some();
    let mut pivot = pivot.unwrap_or([im.w / 2, im.h - 1]);
    let mut turned = im.clone();
    for _ in 0..turns % 4 {
        pivot = quick_rig::quarter_turn_point(pivot, turned.h);
        turned = super::sprites::quarter_turn(&turned);
    }
    if !rigged {
        pivot = [turned.w / 2, turned.h - 1];
    }
    (turned, pivot)
}

/// Kingdom's legacy even-width art uses a fractional half; rig staging is
/// odd and pins the authored pivot to the arena's truncated centre column.
pub(super) fn rig_centre(hero: &Hero, book: &crate::rules::cards::Book, width: i32) -> f32 {
    with_selected_rig(hero, book, |_| (width / 2) as f32).unwrap_or(width as f32 / 2.0)
}

pub(super) fn rig_floor_offset(hero: &Hero, book: &crate::rules::cards::Book) -> i32 {
    with_selected_rig(hero, book, |r| r.canvas()[1] - 1 - r.pivot()[1]).unwrap_or(0)
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

/// The shield held to the front: the knight's own, brought round, gold rim
/// lit along its top and darker where it turns from the light.
fn shield_front() -> Img {
    rows(&[
        "64444444",
        "47777778",
        "47755778",
        "47755778",
        "a477778a",
        ".a4778a.",
        "...aa...",
    ])
}

/// Held out to the right, three-quarters on.
fn shield_side() -> Img {
    rows(&[
        ".6444.",
        "647778",
        "475578",
        "475578",
        "477778",
        "a4778a",
        ".a478.",
        "..aa..",
    ])
}

/// Held ahead, away from the viewer: its back, the wooden boards and the
/// strap, wide and low behind the figure to show at its shoulders.
fn shield_far() -> Img {
    rows(&[
        ".a44444444a.",
        "a4PPPPPPPP4a",
        "aPPPbbbbPPPa",
        "aPPPPPPPPPPa",
        ".aPPPPPPPPa.",
        "..aaaaaaaa..",
    ])
}

/// A raised shield just struck (`rules::shield`): how far the blow knocks
/// it back on the screen, in pixels, as the figure is drawn facing right,
/// and how bright its rim flashes (0 none, 2 the tick it lands).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Struck {
    pub dx: i32,
    pub dy: i32,
    pub flash: u8,
}

/// A shield's rim in its flash: gold or steel lit a step or two up its
/// ramp. The field keeps its seat's colours.
fn flashed(shield: &Img, flash: u8) -> Img {
    match flash {
        0 => shield.clone(),
        1 => shield.recolor(&[('6', 'c'), ('4', '6'), ('a', 'o'), ('W', '9'), ('v', 'W'), ('u', 'v')]),
        _ => shield.recolor(&[('6', '9'), ('4', 'c'), ('a', '6'), ('W', '9'), ('v', '9'), ('u', 'W')]),
    }
}

/// The shield up toward `facing`, over `body`, a figure drawn without its
/// shield at rest; knocked and flashing when it was just `struck`.
pub(super) fn raised(body: &Img, facing: Facing, struck: Struck) -> Img {
    let mut im = body.clone();
    let (dx, dy) = (struck.dx, struck.dy);
    match facing {
        Facing::Down => {
            im.stamp(&flashed(&shield_front(), struck.flash), 4 + dx, 8 + dy);
        }
        Facing::Side => {
            im.stamp(&flashed(&shield_side(), struck.flash), 9 + dx, 6 + dy);
        }
        Facing::Up => {
            let mut out = Img::new(im.w, im.h);
            out.stamp(&flashed(&shield_far(), struck.flash), 2 + dx, 3 + dy);
            out.stamp(&im, 0, 0);
            im = out;
        }
    }
    im
}

/// Colours a seat's knight wears when the company gave none: red, blue,
/// green, gold, sea-green and ivory, a seat each (six, `MAX_PLAYERS`).
pub(super) fn seat_colours(id: u32) -> &'static [(char, char)] {
    match id {
        1 => &[],
        2 => &[('7', '1'), ('8', '0'), ('5', '9')],
        3 => &[('7', 'A'), ('8', 'l'), ('5', '6')],
        4 => &[('7', '4'), ('8', 'a'), ('5', 'c')],
        5 => &[('7', '2'), ('8', 'S'), ('5', '3')],
        _ => &[('7', 'c'), ('8', 'P'), ('5', 'o')],
    }
}

/// The ink a seat is marked in, as its knight's cloth: a notch on a card, a
/// pip on a scoreboard, a tag over a rider.
pub(super) fn seat_cloth(id: u32) -> char {
    match id {
        1 => '7',
        2 => '1',
        3 => 'A',
        4 => '4',
        5 => '2',
        _ => 'c',
    }
}

/// A seat's pip on a board or the map: the cloth's ink, but for the first
/// seat, whose pip is gold.
pub(super) fn seat_mark(id: u32) -> char {
    if id == 1 { '5' } else { seat_cloth(id) }
}

/// The seat's colours on the realm's knight. The gold seat's field is the
/// rim's own gold, so there the rim (and the gold of buckle and hilt) goes
/// to bronze first: the shield's field stands off its rim and stays gold.
pub(super) fn realm_colours(id: u32) -> &'static [(char, char)] {
    match id {
        4 => &[('4', 'a'), ('7', '4'), ('8', 'a'), ('5', 'c')],
        _ => seat_colours(id),
    }
}

/// The dwarf's kit id.
pub const DWARF: &str = "ironbraid";

/// A knight of the company as the portrait shows them: whole, at rest.
pub fn of_kit(id: Option<&str>, colours: &[(char, char)]) -> Img {
    if super::three_heroes::owns(id) { return super::three_heroes::standing(id.unwrap()).recolor(colours); }
    if super::pack_heroes::owns(id) { return super::pack_heroes::standing(id.unwrap()); }
    // A champion as their body alone, near the size of a class's figure,
    // for a portrait's frame; the tavern stands them up whole.
    if let Some(c) = id.and_then(crate::rules::champions::Champion::of_kit) {
        return super::champions::bust(c);
    }
    // A kit that is a class stands as its class, in the first seat's trim.
    if let Some(look) = id.and_then(Look::of_kit) {
        return class_looks::figure(look, Facing::Down, Pose::Stand, true);
    }
    if id == Some(DWARF) {
        dwarf_body(0, true)
    } else {
        kit::knight().recolor(colours)
    }
}

/// A knight standing square on for a bezel or a ghost: their shape while
/// they wish themselves into being, their class if they have one, else the
/// realm's knight in their colours.
pub(super) fn portrait(hero: &Hero, id: u32) -> Img {
    if !hero.look.custom { return portrait_original(hero, id); }
    let visual = super::mirror::visual_hero(hero);
    let mut facing = visual.into_owned();
    facing.aim_x = 0.0;
    facing.aim_y = 1.0;
    let im = super::mirror::appearance(portrait_original(&facing, id), &facing);
    super::mirror::sized(im, Vec::new(), hero).0
}

fn portrait_original(hero: &Hero, id: u32) -> Img {
    if hero.look.custom && hero.shape.is_none() {
        let mut standing = hero.clone();
        standing.aim_x = 0.0;
        standing.aim_y = 1.0;
        standing.stride = 0.0;
        standing.shielding = false;
        return legacy_figure(&standing, id, false, Struck::default(), false);
    }
    if hero.shape.is_none() && super::three_heroes::owns(hero.knight.as_deref()) { return super::three_heroes::standing(hero.knight.as_deref().unwrap()); }
    if hero.shape.is_none() && super::pack_heroes::owns(hero.knight.as_deref()) { return super::pack_heroes::standing(hero.knight.as_deref().unwrap()); }
    if let Some(shape) = hero.shape {
        return shape_img(shape, 14, true);
    }
    if let Some(c) = crate::rules::champions::of(hero) {
        return super::champions::bust(c).recolor(seat_colours(id));
    }
    match Look::of(hero) {
        // The Gravecaller's bust is his hood, not the headstone over it.
        Some(look) => class_looks::figure(look, Facing::Down, Pose::Stand, look != Look::Gravecaller)
            .recolor(seat_colours(id)),
        None if hero.colours().is_empty() => super::sprites::knight(id),
        None => super::sprites::knight_in(hero.colours()),
    }
}

/// An untimed native hero (portraits and existing attachment callers), in the
/// colours of their seat, at the
/// stage of dress their health leaves them, with their shield (if they
/// hold one up) toward its aim. `None` for a hero drawn another way.
pub fn figure(hero: &Hero, id: u32) -> Img {
    figure_with(hero, id, false)
}

/// As `figure`, with the shield thrown (`thrown`): the arm is bare and
/// nothing can be raised.
pub fn figure_with(hero: &Hero, id: u32, thrown: bool) -> Img {
    figure_struck(hero, id, thrown, Struck::default())
}

/// As `figure_with`, a raised shield knocked and flashing when it was just
/// `struck`.
pub fn figure_struck(hero: &Hero, id: u32, thrown: bool, struck: Struck) -> Img {
    figure_casting(hero, id, thrown, struck, false)
}

/// As `figure_struck`, mid-cast when `casting`: a class's figure is in its
/// strike (the Gravecaller's staff raised over his head).
pub fn figure_casting(hero: &Hero, id: u32, thrown: bool, struck: Struck, casting: bool) -> Img {
    let visual = super::mirror::visual_hero(hero);
    let im = if hero.look.custom && (super::three_heroes::owns(visual.knight.as_deref()) || super::pack_heroes::owns(visual.knight.as_deref())) {
        let mut right = visual.into_owned();
        right.aim_x = right.aim_x.abs();
        legacy_figure(&right, id, thrown, struck, casting)
    } else {
        legacy_figure(&visual, id, thrown, struck, casting)
    };
    super::mirror::appearance(im, hero)
}

/// Timed quick-rig body entry. `tick` is authoritative render time, not the
/// wall clock. Live arena composition uses `dressed_frame`; untimed
/// `figure`/`body_of` callers retain the native profile contract.
pub(super) fn figure_at(
    hero: &Hero,
    id: u32,
    thrown: bool,
    struck: Struck,
    casting: bool,
    tick: u64,
) -> Img {
    if let Some(rig) = default_rig(hero, id) {
        return rig_figure(hero, id, thrown, struck, rig, tick, casting);
    }

    figure_timed(hero, id, thrown, struck, casting, Some(tick))
}

fn legacy_figure(hero: &Hero, id: u32, thrown: bool, struck: Struck, casting: bool) -> Img {
    legacy_timed(hero, id, thrown, struck, casting, None)
}

fn legacy_timed(hero: &Hero, id: u32, thrown: bool, struck: Struck, casting: bool, tick: Option<u64>) -> Img {
    if hero.shape.is_none() && super::three_heroes::owns(hero.knight.as_deref()) { return super::three_heroes::figure(hero); }
    if hero.shape.is_none() && super::pack_heroes::owns(hero.knight.as_deref()) { return super::pack_heroes::figure(hero); }
    if let Some(shape) = hero.shape {
        return hatted(shape_figure(hero, shape, thrown, struck), hero);
    }
    if let Some(c) = crate::rules::champions::of(hero) {
        let im = if let Some(tick) = tick { super::champions::floating_figure(hero, c, tick, id) } else { super::champions::figure(hero, c) };
        return if hero.look.custom { im } else { im.recolor(seat_colours(id)) };
    }
    if let Some(look) = Look::of(hero) {
        return hatted(class_figure(hero, id, thrown, look, struck, casting, tick), hero);
    }
    let raising = hero.shielding && !thrown && (hero.owned.shield || hero.holds_shield());
    let im = if hero.knight.as_deref() == Some(DWARF) {
        let stage = hero.gear.min(2);
        let mut im = dwarf_body(if hero.look.custom { stage.max(1) } else { stage }, tick.is_none() && hero.spin == 0 && !hero.unarmed());
        if !hero.look.custom && hero.helmet_worn() && stage >= 1 {
            let head = dwarf_body(0, false);
            for y in 0..8 {
                for x in 0..16 {
                    im.clear(x, y);
                    if let Some(c) = head.get(x, y) {
                        im.set(x, y, c);
                    }
                }
            }
        }
        if raising { raised(&im, facing(hero.aim_x, hero.aim_y), struck) } else { im }
    } else {
        let im = realm_body(hero, thrown, tick.is_some());
        let im = if let Some(tick) = tick { floating_equipment(im, hero, tick, id) } else { im };
        let im = if raising { raised(&im, facing(hero.aim_x, hero.aim_y), struck) } else { im };
        if hero.look.custom {
            im
        } else if hero.colours().is_empty() {
            im.recolor(realm_colours(id))
        } else {
            im.recolor(hero.colours())
        }
    };
    hatted(im, hero)
}

/// A knight of a class, as that class: facing the aim, on their stride or
/// in their strike, in the seat's trim. A class keeps its look as health
/// fails (the knight's helmet and tabard stay the knight's); the health bar
/// says how they fare.
fn class_figure(hero: &Hero, id: u32, thrown: bool, look: Look, struck: Struck, casting: bool, tick: Option<u64>) -> Img {
    let face = facing(hero.aim_x, hero.aim_y);
    // A shield held is always drawn: on the arm, or brought round to the
    // aim while it is up. The Shieldmaiden's is her held thing; any other
    // class wears the knight's own beside what they carry.
    let shield = !thrown && (hero.holds_shield() || (look.shielded() && hero.owned.shield));
    let raising = shield && hero.shielding;
    let held = if look.shielded() {
        shield && !raising
    } else {
        hero.flee_ticks == 0 && !hero.unarmed() && !(look == Look::Berserker && hero.spin > 0)
    };
    let pose = if raising {
        Pose::Stand
    } else if casting && held {
        Pose::Strike
    } else {
        class_looks::pose(hero)
    };
    let slung = shield && !raising && !look.shielded();
    let mut im = match tick {
        Some(tick) => class_looks::floating_figure(look, face, pose, held, slung, tick, id, (hero.aim_x, hero.aim_y)),
        None => class_looks::figure_shielded(look, face, pose, held, slung),
    };
    if raising {
        // The shield, brought round to the aim: hers steel-rimmed like the
        // one on her arm, the knight's own gold. Its foot stays on the
        // body's, however tall the class stands.
        let off = (im.w - 16) / 2;
        let (dx, dy) = (struck.dx, struck.dy + im.h - 16);
        let rim: &[(char, char)] = if look.shielded() { STEEL_RIM } else { &[] };
        let up = |shield: Img| flashed(&shield.recolor(rim), struck.flash);
        match face {
            Facing::Down => im.stamp(&up(shield_front()), off + 4 + dx, 8 + dy),
            Facing::Side => im.stamp(&up(shield_side()), off + 10 + dx, 6 + dy),
            Facing::Up => {
                let mut out = Img::new(im.w, im.h);
                out.stamp(&up(shield_far()), off + 2 + dx, 4 + dy);
                out.stamp(&im, 0, 0);
                im = out;
            }
        }
    }
    if hero.look.custom { im } else { im.recolor(seat_colours(id)) }
}

/// The Shieldmaiden's kite shield is rimmed in steel where the knight's is
/// gold.
const STEEL_RIM: &[(char, char)] = &[('6', 'W'), ('4', 'v'), ('a', 'u')];

/// What a game against friends left on the knight's helm: the winner's gold
/// crown, or the game's booby prize until they win one back.
fn hat(hero: &Hero) -> Option<Img> {
    use crate::rules::pastimes::Game;
    // Each in its own material, lit from the upper left like the knight:
    // the lit edge a step up its ramp, the far edge its darkest step.
    if hero.laurel {
        return Some(rows(&[
            "..6..6..6...",
            "..65.65.54..",
            "..65555554..",
            "..a44744a4a.",
        ]));
    }
    Some(match hero.booby? {
        Game::Pot => rows(&[
            "...vVVVVu...",
            "..vWVVVVvux.",
            "..VVvvvvUuxx",
            "..vVvvvvUx.x",
            "..uUUUUUuxx.",
        ]),
        Game::Chariot | Game::Turnip => rows(&[
            "...mAAml....",
            "..mYAmAmE...",
            "..lAmmmlE...",
            "...lllEE....",
        ]),
        Game::DuckShoot => rows(&[
            "......65....",
            ".....6K5o...",
            "..6555a.....",
            "...a44a.....",
        ]),
        Game::Trencher | Game::Cheese => rows(&[
            "...tTTTto...",
            "..tTtooooor.",
            "..rooooorrr.",
        ]),
        // An apple on the helm, an arrow clean through it.
        Game::Longbow => rows(&[
            "......e.....",
            "....@778....",
            "roo777778otT",
            "....7788....",
        ]),
        // A war-goose rearing, wings out; a knight's helm above the bill.
        Game::Gander => rows(&[
            ".....99.....",
            "..59h9hO....",
            ".9hhhhh.p...",
            "..hhhhh.O...",
        ]),
        Game::Surf | Game::Washtub => rows(&[
            "..m.m..l....",
            "..m.ml.l.E..",
            ".mlllllEEE..",
        ]),
        // The belfry's spare bell, worn clapper-down until won back.
        Game::Peal => rows(&[
            ".....77.....",
            "....7777....",
            "...777777...",
            "..77777777..",
            ".....o......",
        ]),
        _ => rows(&[
            ".....7......",
            "....798.....",
            "...79978....",
            "..7999988...",
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

// ─── the identity start's shapes ─────────────────────────────────────────────

/// A shape's inks, dark to light: its outline, its fill and the edge the
/// light catches (upper left, as the knights are lit). One hue a seat, the
/// seats' own cloths: red, blue, green, gold, sea-green and ivory.
fn shape_inks(shape: Shape) -> [char; 3] {
    match shape {
        Shape::Square => ['8', '7', 'R'],
        Shape::Triangle => ['0', '1', 'z'],
        Shape::Circle => ['F', 'A', 'Y'],
        Shape::Diamond => ['a', '4', '6'],
        Shape::Hexagon => ['S', '2', '3'],
        Shape::Star => ['J', 'c', '9'],
    }
}

/// Whether the point `(u, v)` (each -1 to 1 across the shape's box, v
/// down) is inside `shape`.
fn inside(shape: Shape, u: f32, v: f32) -> bool {
    match shape {
        Shape::Square => u.abs() <= 0.86 && v.abs() <= 0.86,
        Shape::Triangle => v >= -1.0 && v <= 0.9 && u.abs() <= (v + 1.0) * 0.5 + 0.04,
        Shape::Circle => u * u + v * v <= 1.0,
        Shape::Diamond => u.abs() + v.abs() <= 1.04,
        Shape::Hexagon => {
            let h = 3f32.sqrt() / 2.0;
            v.abs() <= h + 0.04 && u.abs() <= 1.02 - v.abs() / 3f32.sqrt()
        }
        Shape::Star => {
            // Five points, one up: inside the outer ring's star polygon.
            let (outer, inner) = (1.16, 0.5);
            let points: Vec<(f32, f32)> = (0..10)
                .map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
                    let r = if k % 2 == 0 { outer } else { inner };
                    (a.cos() * r, a.sin() * r + 0.1)
                })
                .collect();
            let mut odd = false;
            let mut j = points.len() - 1;
            for i in 0..points.len() {
                let ((xi, yi), (xj, yj)) = (points[i], points[j]);
                if (yi > v) != (yj > v) && u < (xj - xi) * (v - yi) / (yj - yi) + xi {
                    odd = !odd;
                }
                j = i;
            }
            odd
        }
    }
}

/// `shape` on a `side`-pixel square: a dark outline, its fill, and the
/// light along its upper left edge (`lit`; an emblem small enough to be a
/// mark is drawn flat).
pub fn shape_img(shape: Shape, side: i32, lit: bool) -> Img {
    let [outline, fill, light] = shape_inks(shape);
    let half = side as f32 / 2.0;
    let mask: Vec<bool> = (0..side * side)
        .map(|i| {
            let (x, y) = (i % side, i / side);
            inside(shape, (x as f32 + 0.5 - half) / half, (y as f32 + 0.5 - half) / half)
        })
        .collect();
    let at = |x: i32, y: i32| (0..side).contains(&x) && (0..side).contains(&y) && mask[(y * side + x) as usize];
    let mut im = Img::new(side, side);
    for y in 0..side {
        for x in 0..side {
            if !at(x, y) {
                continue;
            }
            let edge = !at(x - 1, y) || !at(x + 1, y) || !at(x, y - 1) || !at(x, y + 1);
            let ink = if edge {
                outline
            } else if lit && (!at(x - 2, y) || !at(x, y - 2)) && (x + y) < side {
                light
            } else {
                fill
            };
            im.put(x, y, ink);
        }
    }
    im
}

/// A knight still a shape, as the arena draws them: the shape standing on
/// the figure's feet, rising a pixel with each stride, its shield (a
/// guard wished or found) raised to the aim when it is up. The parts
/// wished go on over it (`dressed`), and once a body is wished the shape's
/// mark goes on its chest (`emblem`).
pub fn shape_figure(hero: &Hero, shape: Shape, thrown: bool, struck: Struck) -> Img {
    shape_struck(hero, shape, false, thrown, struck)
}

/// The shape's side as a torso: a little smaller than a knight, centred
/// where a knight's chest is, so a head wished goes on above it, arms at
/// its sides and feet below.
pub const SHAPE_SIDE: i32 = 12;

/// As `shape_figure`, with a body wished (`bodied`) or not: the shape
/// stays, the body wished is worn over it and the shape's mark goes on its
/// chest (`emblem`), so a whole body drawn over the shape still shows whose
/// it is. (Letting the shape give way left a knight smaller for wishing a
/// body: a body drawn small at its place is smaller than the shape.)
pub fn shape_struck(hero: &Hero, shape: Shape, _bodied: bool, thrown: bool, struck: Struck) -> Img {
    let mut im = Img::new(16, 16);
    let lift = i32::from(matches!(stride_step(hero), 1 | 3));
    im.stamp(&shape_img(shape, SHAPE_SIDE, true), 8 - SHAPE_SIDE / 2, 16 - SHAPE_SIDE - lift);
    if hero.shielding && !thrown && hero.holds_shield() {
        im = raised(&im, facing(hero.aim_x, hero.aim_y), struck);
    }
    im
}

/// A knight being made as the arena draws them, before what they wear.
pub fn identity_figure(hero: &Hero, shape: Shape, bodied: bool, thrown: bool, struck: Struck) -> Img {
    hatted(shape_struck(hero, shape, bodied, thrown, struck), hero)
}

/// A shape's mark on the chest of the body wished over it: the shape
/// small, on the dressed figure `im` whose feet are its bottom row.
pub fn emblem(im: &mut Img, shape: Shape) {
    let mark = shape_img(shape, 5, false);
    im.stamp(&mark, im.w / 2 - 3, im.h - 16 + 7);
}

// ─── what cards put on the field ─────────────────────────────────────────────

/// A frame of a card's sprite, as an image.
pub fn frame_img(rows: &[String]) -> Img {
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    Img::from_rows(&refs)
}

/// A wish sprite's frame at `tick`, its own inks (`pal=`) in their colours.
pub fn sprite_img(sprite: &Sprite, tick: u64) -> Img {
    let refs: Vec<&str> = sprite.frame(tick).iter().map(String::as_str).collect();
    Img::from_rows_pal(&refs, &sprite.pal)
}

/// The sprites a knight wears, over their body, oldest under newest: each
/// frame centred on the figure. The canvas grows for what stands proud of
/// the figure at the sides and above; below the feet is cut, so the knight
/// stands where they stood.
pub fn dress(body: &Img, worn: &[&Sprite], tick: u64) -> Img {
    dressed(body, worn, tick, 0).0
}

/// As `dress`, each piece moving by its motion words a step `phase` out
/// of another knight's; with the paintings of the pieces that have them,
/// in the dressed picture's pixels (`paintings`).
pub fn dressed(body: &Img, worn: &[&Sprite], tick: u64, phase: u32) -> (Img, Vec<super::Painting>) {
    dressed_as(body, crate::rules::looks::profile("knight"), worn, tick, phase)
}

pub fn dressed_as(body: &Img, fit: Body, worn: &[&Sprite], tick: u64, phase: u32) -> (Img, Vec<super::Painting>) {
    super::wear::dressed(body, fit, worn, tick, phase)
}

/// Rig-only composition: preserve native fitting/motion for every other slot,
/// but turn right-slot art about its existing Part contact. Unlike native
/// dressing, a swung weapon may extend below the feet: the returned pivot
/// records actual body padding, never the bottom of that weapon's rectangle.
pub(super) fn dressed_rig(
    body: &Img,
    fit: Body,
    worn: &[&Sprite],
    tick: u64,
    phase: u32,
    sample: &Sample<'_>,
) -> DressedFrame {
    dressed_rig_aimed(body, fit, worn, tick, phase, sample, None)
}

fn dressed_rig_aimed(
    body: &Img, fit: Body, worn: &[&Sprite], tick: u64, phase: u32,
    sample: &Sample<'_>, aim: Option<(f32, f32)>,
) -> DressedFrame {
    use super::paintings::{self, Painting, Pose};
    use crate::rules::looks::ink_bounds;

    let (_, dx, top) = stage(sample);
    if !worn
        .iter()
        .any(|s| s.at.as_deref().and_then(crate::rules::looks::place) == Some("right"))
    {
        let (image, paints) = dressed_as(body, fit, worn, tick, phase);
        let pivot = [image.w / 2, sample.pivot()[1] + top + image.h - body.h];
        return DressedFrame {
            image,
            paints,
            pivot: Some(pivot),
            authored_roll: sample.clip == Clip::Roll,
            named: None,
        };
    }
    let anchors = sample.anchors();
    let tip = aim.map_or([anchors.tip[0] + dx, anchors.tip[1] + top], |(x, y)| {
        let x = if x < -0.3 { -x } else { x };
        if !x.is_finite() || !y.is_finite() || x.hypot(y) < 0.001 {
            [fit.right.anchor.0 + 16, fit.right.anchor.1]
        } else {
            let length = x.hypot(y);
            [fit.right.anchor.0 + (x / length * 64.0).round() as i32,
             fit.right.anchor.1 + (y / length * 64.0).round() as i32]
        }
    });
    let mut pieces = Vec::new();
    for (n, original) in worn.iter().enumerate() {
        let part = original
            .at
            .as_deref()
            .and_then(|at| fit.part(at))
            .unwrap_or_else(|| fit.whole());
        let part = super::factory::weapon_part(original, part);
        let sprite = super::wear::fitted(original, part);
        let im = frame_img(sprite.frame(tick));
        let mut origin = super::factory::weapon_origin(original, &sprite, part)
            .unwrap_or_else(|| part.origin((im.w, im.h)));
        let pose = Pose::of(
            &sprite.moves,
            tick,
            phase.wrapping_add(n as u32 * 977),
            (0.0, 0.0),
        );
        let rect = if part.stretch {
            [0.0, 0.0, im.w as f32, im.h as f32]
        } else if let (Some([x, y, w, h]), Some(square)) = (
            ink_bounds(&original.frames),
            paintings::square(original, false, 1.0),
        ) {
            let sx = im.w as f32 / w as f32;
            let sy = im.h as f32 / h as f32;
            [
                (square[0] - x as f32) * sx,
                (square[1] - y as f32) * sy,
                square[2] * sx,
                square[3] * sy,
            ]
        } else {
            [0.0, 0.0, im.w as f32, im.h as f32]
        };
        let local_pivot = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
        let (mut moved, offset) = pose.apply(&im, local_pivot);
        let right = original.at.as_deref().and_then(crate::rules::looks::place) == Some("right");
        if !right {
            // Keep the unchanged native floor contract for hats/capes/shields.
            origin.1 -= (origin.1 + offset.1 + moved.h - body.h).max(0);
        }
        let mut at = [origin.0 + offset.0, origin.1 + offset.1];
        let mut paint = Painting::of(original, tick, false, (0.0, 0.0), 1.0, pose);
        if let Some(p) = &mut paint {
            p.rect = [
                origin.0 as f32 + rect[0],
                origin.1 as f32 + rect[1],
                rect[2],
                rect[3],
            ];
            p.subject = part.stretch;
            p.under = part.under;
            p.pixel = true;
        }
        if right {
            // Contact is exactly Part::origin's inverse, including bottom=true.
            // Motion words run first in source space; rig rotation runs last.
            let orientation = quick_rig::WeaponOrientation::new(
                [part.anchor.0 - at[0], part.anchor.1 - at[1]],
                [part.anchor.0, part.anchor.1],
                tip,
            );
            if let Some(p) = &mut paint {
                // Pose::place already accounts for authored motion, including
                // its own pivot. Rotate that resulting rectangle's centre
                // around the same pixel-centre contact as the hard inks.
                let (rect, angle) = p.placed();
                let centre = orientation.point([
                    rect[0] + rect[2] / 2.0 - at[0] as f32 - 0.5,
                    rect[1] + rect[3] / 2.0 - at[1] as f32 - 0.5,
                ]);
                p.rect = [
                    centre[0] + 0.5 - rect[2] / 2.0,
                    centre[1] + 0.5 - rect[3] / 2.0,
                    rect[2],
                    rect[3],
                ];
                p.pose = Pose {
                    angle: angle + orientation.angle,
                    ..Pose::STILL
                };
            }
            (moved, at) = orientation.stamp(&moved);
        }
        if let Some(p) = &mut paint {
            p.at = (at[0], at[1]);
            p.inks = moved.clone();
        }
        pieces.push((moved, at, part.under, paint));
    }
    let mut pad = 0;
    let mut above = 0;
    let mut below = 0;
    for (im, at, _, _) in &pieces {
        pad = pad.max(-at[0]).max(at[0] + im.w - body.w);
        above = above.max(-at[1]);
        below = below.max(at[1] + im.h - body.h);
    }
    let mut image = Img::new(body.w + pad * 2, body.h + above + below);
    let mut paints = Vec::new();
    for under in [true, false] {
        if !under {
            image.stamp(body, pad, above);
        }
        for (im, at, behind, paint) in &pieces {
            if *behind != under {
                continue;
            }
            image.stamp(im, at[0] + pad, at[1] + above);
            if let Some(p) = paint {
                let mut p = p.clone().shifted((pad, above));
                if under {
                    for y in 0..p.inks.h {
                        for x in 0..p.inks.w {
                            if body.get(at[0] + x, at[1] + y).is_some() {
                                p.inks.clear(x, y);
                            }
                        }
                    }
                }
                paints.push(p);
            }
        }
    }
    DressedFrame {
        image,
        paints,
        pivot: Some([
            sample.pivot()[0] + dx + pad,
            sample.pivot()[1] + top + above,
        ]),
        authored_roll: sample.clip == Clip::Roll,
        named: None,
    }
}

/// The figure's own attachment coordinates, independent of its weapon's
/// reach, hat padding and staff headroom.
pub fn body_of(hero: &Hero, figure: &Img) -> Body {
    let visual = super::mirror::visual_hero(hero);
    body_at(&visual, figure, 0, false, 1)
}

/// `seat` picks the design whose sockets a rigged figure is fitted by.
fn body_at(hero: &Hero, figure: &Img, tick: u64, casting: bool, seat: u32) -> Body {
    if let Some(rig) = default_rig(hero, seat) {
        let sample = rig_sample(rig, hero, tick, casting);
        if figure.h == rig.canvas()[1] + 12 && figure.w == stage(&sample).0 {
            return knight_fit(&sample, hero);
        }
    }

    if hero.shape.is_some() {
        let lift = i32::from(matches!(stride_step(hero), 1 | 3));
        return Body::human((2, figure.h - SHAPE_SIDE - lift), (SHAPE_SIDE, SHAPE_SIDE));
    }
    if crate::rules::champions::of(hero).is_some() {
        let animated = super::champions::fit(hero);
        let profile = if facing(hero.aim_x, hero.aim_y) == Facing::Up { "champion_back" } else { "champion" };
        let native = super::champions::figure(hero, crate::rules::champions::of(hero).unwrap());
        let shift = ((figure.w - native.w) / 2, figure.h - native.h);
        let mut fit = crate::rules::looks::profile(profile).shifted(animated.bounds[0], animated.bounds[1]);
        // The champion owns its arm animation; do not fork that animation
        // into the authoring tool or edit the parallel champion-arms work.
        fit.right.anchor = animated.right.anchor;
        fit.left.anchor = animated.left.anchor;
        return fit.shifted(shift.0, shift.1);
    }
    if let Some(look) = Look::of(hero) { return class_looks::fit(look, hero, figure); }
    let view = match facing(hero.aim_x, hero.aim_y) { Facing::Down => "front", Facing::Side => "right", Facing::Up => "back" };
    let origin = ((figure.w - 16) / 2, figure.h - 16);
    let mut fit = crate::rules::looks::profile(&format!("realm_{view}"))
        .shifted(origin.0, origin.1);
    let (held,strike) = held_of(hero);
    let (_shield,grip) = realm::sockets(view_of(facing(hero.aim_x,hero.aim_y)),held,strike);
    let dy = fit.bounds[1];
    fit.right.anchor = (origin.0 + grip.0, grip.1 + dy);
    if hero.shielding { fit.left.anchor = match facing(hero.aim_x, hero.aim_y) {
        Facing::Down => (8, 12), Facing::Side => (12, 10), Facing::Up => (6, 8),
    }; }
    fit
}

/// Dress the hero, including a whole-body colour wish that follows the
/// existing silhouette rather than replacing it with a rectangular swatch.
pub(super) fn dressed_hero(
    body: &Img,
    hero: &Hero,
    book: &crate::rules::cards::Book,
    tick: u64,
    phase: u32,
) -> (Img, Vec<super::Painting>) {
    dressed_casting(
        body,
        hero,
        book,
        tick,
        phase,
        false,
        false,
        Struck::default(),
    )
}

/// Compatibility entry for callers without a cast event: `casting` starts
/// at anticipation, rather than guessing an origin from the world tick.
pub(super) fn dressed_casting(
    body: &Img,
    hero: &Hero,
    book: &crate::rules::cards::Book,
    tick: u64,
    phase: u32,
    casting: bool,
    thrown: bool,
    struck: Struck,
) -> (Img, Vec<super::Painting>) {
    dressed_event(
        body,
        hero,
        book,
        tick,
        phase,
        casting.then_some(0),
        thrown,
        struck,
        None,
    )
}

/// One live frame, including its contact point after equipment padding.
pub(super) struct DressedFrame {
    pub image: Img,
    pub paints: Vec<super::Painting>,
    pub pivot: Option<[i32; 2]>,
    pub authored_roll: bool,
    /// What the frame is, when it is the rig's own knight as a seat wears
    /// it (`figure_keys`): its key, dress and feet, at the game's rung
    /// until the caller, who knows the run, says the knight's.
    pub named: Option<super::figure_keys::Named>,
}

pub(super) fn dressed_event(
    body: &Img,
    hero: &Hero,
    book: &crate::rules::cards::Book,
    tick: u64,
    phase: u32,
    cast_age: Option<u64>,
    thrown: bool,
    struck: Struck,
    seat: Option<u32>,
) -> (Img, Vec<super::Painting>) {
    let frame = dressed_frame(
        || body.clone(),
        hero,
        book,
        tick,
        phase,
        cast_age,
        thrown,
        struck,
        seat,
    );
    (frame.image, frame.paints)
}

/// Sample exactly once; the body and every socket use that timed sample.
/// Legacy body construction is lazy, never a throwaway for a rigged hero.
pub(super) fn dressed_frame(
    fallback: impl FnOnce() -> Img,
    hero: &Hero,
    book: &crate::rules::cards::Book,
    tick: u64,
    phase: u32,
    cast_age: Option<u64>,
    thrown: bool,
    struck: Struck,
    seat: Option<u32>,
) -> DressedFrame {
    let wished = wished_rig(hero, book);
    let worn = hero.worn(book);
    let whole = worn
        .iter()
        .any(|s| s.at.as_deref().is_none_or(|at| at == "whole"));
    let default = seat.and_then(|seat| default_rig(hero, seat)).filter(|_| !whole);
    let rig = wished.as_ref().map(|(_, r)| r.as_ref()).or(default);
    if let Some(rig) = rig {
        let sample = rig_sample_event(rig, hero, book, tick, cast_age);
        // A worn right sprite is the weapon, not a second blade laid over
        // the procedural grip-to-tip placeholder.
        let native_weapon = !worn
            .iter()
            .any(|s| s.at.as_deref().and_then(crate::rules::looks::place) == Some("right"));
        let mut body = rig_art_sample(hero, thrown, struck, &sample, native_weapon);
        if wished.is_none() {
            body = body.recolor(if hero.colours().is_empty() {
                realm_colours(seat.unwrap())
            } else {
                hero.colours()
            });
        }
        let fit = if wished.is_some() {
            rig_fit(&sample, hero)
        } else {
            knight_fit(&sample, hero)
        };
        let rest: Vec<_> = worn
            .into_iter()
            .filter(|s| wished.is_none() || s.at.as_deref().is_some_and(|at| at != "whole"))
            .collect();
        let tinted = hero
            .worn_colour(book)
            .map(|card| super::wear::tinted(&body, card, fit));
        let mut frame = dressed_rig_aimed(
            tinted.as_ref().unwrap_or(&body),
            fit,
            &rest,
            tick,
            phase,
            &sample,
            (matches!(held_of(hero).0, Held::Bow | Held::Crossbow | Held::Handgonne)
                && !hero.sword && hero.swing == 0
                && !hero.forged.as_ref().is_some_and(|w| w.melee.is_some())
                && super::factory::equipped_attack_clip(hero, book).is_none())
                .then_some((hero.aim_x, hero.aim_y)),
        );
        // The rig's own knight in a seat's colours, wearing nothing over
        // it: named, so a pack can draw it by what it is (`figure_keys`).
        if let (None, true, None, Some(seat), Some(design), Some(pivot)) =
            (&wished, rest.is_empty(), &tinted, seat, super::figure_keys::design_of(rig), frame.pivot)
        {
            let colours = super::figure_keys::colours_part(seat, hero.colours());
            // The body before what it carries: its pixels in the same
            // canvas, in the same colours (nothing worn: the frame is the
            // rig's image as `rig_art_sample` stamped it).
            let bare = staged_image(&sample).recolor(if hero.colours().is_empty() {
                realm_colours(seat)
            } else {
                hero.colours()
            });
            frame.named = Some(super::figure_keys::Named {
                key: super::figure_keys::knight_key(design, sample.clip, sample.index, &colours).into(),
                dress: dress_of(hero, thrown, native_weapon, struck).into(),
                pivot,
                rung: crate::rules::resolution::Rung::BASE,
                bare: ((bare.w, bare.h) == (frame.image.w, frame.image.h)).then(|| std::sync::Arc::new(bare)),
            });
        }
        return frame;
    }
    let body = if whole && seat.is_some_and(|seat| default_rig(hero, seat).is_some()) {
        legacy_figure(hero, seat.unwrap(), thrown, struck, cast_age.is_some())
    } else {
        fallback()
    };
    let body = if hero.shape.is_some() || hero.avatar.is_some() {
        floating_equipment(body, hero, tick, phase)
    } else { body };
    let fit = body_at(
        &super::mirror::visual_hero(hero),
        &body,
        tick,
        cast_age.is_some(),
        seat.unwrap_or(1),
    );
    let tinted = hero
        .worn_colour(book)
        .map(|card| super::wear::tinted(&body, card, fit));
    let (image, paints) = super::wear::dressed_aimed(
        tinted.as_ref().unwrap_or(&body),
        fit,
        &hero.worn(book),
        tick,
        phase,
        Some((hero.aim_x, hero.aim_y)),
    );
    let (image, paints) = super::mirror::sized(image, paints, hero);
    DressedFrame {
        image,
        paints,
        pivot: None,
        authored_roll: false,
        named: None,
    }
}

/// A knight who brought their own picture: the sprite at the arena's
/// sixteen pixels, with a held shield on the arm, brought round when it is
/// raised (the picture carries no shield of its own to take off).
pub fn avatar_figure(hero: &Hero, pixels: &[[u8; 4]], thrown: bool) -> Img {
    avatar_struck(hero, pixels, thrown, Struck::default())
}

/// As `avatar_figure`, a raised shield knocked and flashing when it was
/// just `struck`.
pub fn avatar_struck(hero: &Hero, pixels: &[[u8; 4]], thrown: bool, struck: Struck) -> Img {
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
        im = raised(&im, facing(hero.aim_x, hero.aim_y), struck);
    } else if hero.holds_shield() && !thrown {
        let (shield, (x, y)) = realm::shield(view_of(facing(hero.aim_x, hero.aim_y)));
        im.stamp(&shield, x, y);
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
        const FADE: [char; 7] = ['c', 'V', 'U', 'u', 'S', 'x', 's'];
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

// ─── the shadow ──────────────────────────────────────────────────────────────

/// A knight's contact shadow on the floor, under feet at native `(x, y)`:
/// a short navy pool, the row under the feet a little narrower, its ends
/// broken by the ordered dither so it lies soft. Drawn before the figure,
/// which stands over it; on black paper it is all but black.
pub(super) fn shadow(cv: &mut Img, x: i32, y: i32) {
    use super::super::ink::bayer;
    for (dy, half) in [(0, 5), (1, 4)] {
        for dx in -half..=half {
            let (px, py) = (x + dx, y + dy);
            if dx.abs() == half && bayer(px, py) < 0.5 {
                continue;
            }
            cv.put(px, py, 's');
        }
    }
}

// ─── the roll ────────────────────────────────────────────────────────────────

/// The knight tucked and tumbling with `left` ticks of the roll to go: a
/// full circle along the roll, the legs folded up under the body so the
/// knight turns over as a ball, not a plank.
pub(super) fn rolled(im: &Img, left: u32) -> Img {
    let quarter = (10 - left.min(10)) * 4 / 10;
    let mut turned = tucked(im);
    for _ in 0..quarter {
        turned = super::sprites::quarter_turn(&turned);
    }
    turned
}

/// `im` with its lower half folded up: the head and shoulders whole, every
/// other row below them.
fn tucked(im: &Img) -> Img {
    let keep: Vec<i32> = (0..im.h / 2).chain((im.h / 2..im.h).step_by(2)).collect();
    let mut out = Img::new(im.w, keep.len() as i32);
    for (y, &from) in keep.iter().enumerate() {
        for x in 0..im.w {
            if let Some(c) = im.get(x, from) {
                out.set(x, y as i32, c);
            }
        }
    }
    out
}

// ─── the flung gear ──────────────────────────────────────────────────────────

fn helmet() -> Img {
    realm::helmet()
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

/// The harness and tabard, off over the head.
fn tabard() -> Img {
    rows(&[
        "xVWv.vVvx",
        "uVv778vUx",
        "uvU778vUx",
        ".uv778ux.",
        "..BB4BB..",
        "...878...",
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
        realm::draw(Dress { helm: worn[0], ..Dress::WHOLE })
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
    im.stamp(&torso.recolor(realm::STEEL), 5 + lean, 9);
    if worn[1] {
        im.stamp(&rows(&["iHHh", "hJJG", ".gg."]).recolor(realm::STEEL), 14 + lean, 9);
    }
    let arm = if worn[1] { 'v' } else { 't' };
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
    let boot = if worn[2] { 'u' } else { 'O' };
    let leg = if worn[2] { 'v' } else { 't' };
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
        realm_colours(id)
    } else {
        hero.colours()
    })
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__hero_sheet.rs"]
mod hero_sheet;

pub(super) fn floating_equipment(body: Img, hero: &Hero, tick: u64, id: u32) -> Img {
    let (mut held, strike) = held_of(hero);
    if held == Held::Nothing && hero.sword { held = Held::Sword; }
    let _ = (tick, id);
    match realm::clock_equipment(held, strike) {
        Some((item, grip)) => super::wear::clocked(&body, &item, grip, 0.0,
            (body.w / 2 + 4, body.h - 6), (hero.aim_x, hero.aim_y)),
        None => body,
    }
}

pub(super) fn figure_timed(hero: &Hero, id: u32, thrown: bool, struck: Struck, casting: bool, tick: Option<u64>) -> Img {
    let visual = super::mirror::visual_hero(hero);
    let im = if hero.look.custom && (super::three_heroes::owns(visual.knight.as_deref()) || super::pack_heroes::owns(visual.knight.as_deref())) {
        let mut right = visual.into_owned();
        right.aim_x = right.aim_x.abs();
        legacy_timed(&right, id, thrown, struck, casting, tick)
    } else {
        legacy_timed(&visual, id, thrown, struck, casting, tick)
    };
    super::mirror::appearance(im, hero)
}

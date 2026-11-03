//! The world's rungs (docs/WORLD-TIERS.md): one room shot at every step of
//! the climb from the flat pixel Delve to the lit 3D look and past it to an
//! image-like one, by the machinery `delve_rooms` shoots the lit rooms with
//! (the Delve's `StageMaker`, the GPU core offscreen), so each rung is the
//! window's own frame with one set of dials moved:
//!
//! - **R0** flat, chunky: the room's still art, floor flat (tilt 0), at half
//!   the Delve's grain;
//! - **R1** tilted sprite cards: the still art on the tilted floor, knights
//!   and foes standing up on their feet (what the Delve shows with rooms in
//!   3D off);
//! - **R2** first light: the room stood up in blocks, one low sun with its
//!   shadows and nothing else, four colours (the room's brightness in a hue
//!   half the still art's), dithered between them;
//! - **R3** the monolith look as the branch has it: eight colours, the lamps
//!   burning as the room is fought, the ember ramp where they light;
//! - **R4** image-like: twice the Delve's grain (a stage of 768 x 448),
//!   sixteen colours, the finest weave, the light kept higher and softer.
//!
//! The lit rungs carry the two dials that follow the figures
//! (docs/WORLD-TIERS.md, "Lit cards" and "Depth of field"): the knights and
//! foes lit by the room as cards, and a depth of field round them. Both are
//! on as the room makes them; `--pairs` shoots each lit rung four times, both
//! off, each alone and both, and `--time N` draws each of those `N` times and
//! prints what a frame of the stage costs. `--normals` shoots each lit rung
//! with its cards' normal maps off and on (docs/WORLD-TIERS.md, "Normal
//! maps"), both dials on, times those with `--time`, and lays R3's cards
//! beside their maps (`<case>-maps.png`). The case `crypt-painted` (asked
//! for by name) is the brazier with its first knight wearing a painted
//! shield whose painter shipped its normal map: `-flat`, `-made` (a map made
//! from the painting) and `-authored`.
//!
//! `--hd <pack>` reads an HD art pack (docs/WORLD-TIERS.md, "HD art packs")
//! and draws every rung with it, as the window does: a sprite the pack has a
//! drawing of drawn in its place, and a room whose kit it has tiles for
//! composed of them at its scale in the still art's place.
//!
//! cargo run --release -p angel-glass --example delve_tiers -- <outdir> [--scale N] [--case NAME]... [--rung RN]... [--open SEED] [--set rung.dial=value]... [--pairs] [--normals] [--time N] [--hd PACK]
//!
//! Writes `<case>-a0.png` (the Atari board, floor one only; experimental) and
//! `<case>-r0.png` to `<case>-r4.png`, each the room's view at `N`
//! window pixels to a pixel of the Delve's 384 x 224 (4 unless said), and
//! prints each rung's dials. `--set` moves one (`r2.dither=none`, `r4.grain=3`,
//! `r3.lamps=1.5,1`, `r3.cards=0`, `r3.dof=2,32`, `r3.normals=0.5`; see
//! `set`). With `--pairs`, `<case>-<rung>-off.png`, `-cards.png`, `-dof.png`
//! and `-both.png` too; with `--normals`, `<case>-<rung>-flat.png` and
//! `-normals.png`.
//! `scripts/tiers-sheet.py` lays the shots out on a labelled contact sheet
//! (`--pairs`: the four of each lit rung side by side).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, NATIVE_H, NATIVE_W, UNIT, hd_pack, stage_frame::StageMaker};
use angel_delve::rules::{Pack, Rng, Room, RoomKind, Run, Tile, layout};
use angel_delve::world3d::{self, room, rooms_lit};
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};
use angel_stage::look::{Dither, Fog, Look};
use angel_stage::scene::{Node, Props};
use angel_stage::{LayerKind, Placement, Stand};

/// The stage's picture id, as the cockpit names the Delve's.
const STAGE: u32 = 0x00FA_57A4;
/// A tile, in the room's pixels.
const TILE: f32 = 16.0;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_tiers: {why}");
            ExitCode::FAILURE
        }
    }
}

/// How the lit look is moved from what the room made (`rooms_lit_look`).
#[derive(Clone, Copy)]
struct Dials {
    /// Entries of the ramps (the room makes eight; fewer are picked along
    /// them, more are mixed between).
    ramp: usize,
    /// How far the ramp's hue is the still art's (`still_ramp`: a mean of
    /// each part of its brightness) and not the room's, 0 to 1; its
    /// brightness stays the room's.
    still: f32,
    dither: Dither,
    /// Whether the sun casts shadows.
    shadows: bool,
    /// How strong the lamps burn and how far they reach, of what the room
    /// has (0: none, and no warm ramp).
    lamps: (f32, f32),
    /// The sun's strength and ambient, of the room's.
    sun: (f32, f32),
    /// How high the sky's light grows, in stage pixels: a face pales toward
    /// its top by it (`Sun::rise`); 0 is the same everywhere.
    rise: f32,
    /// Where the distance fades to the ramp's dark middle, in stage pixels
    /// from the eye: where it starts and where it is all fog; none, none.
    fog: Option<(f32, f32)>,
    /// Whether the figures take the room's light (`Look::cards`), and the
    /// least and most of it they take, when not the room's.
    cards: bool,
    card_light: Option<(f32, f32)>,
    /// Whether there is a depth of field (`Look::dof`), and its radius (in
    /// pixels of the Delve's grain) and margin, and its spread (in pixels of
    /// the same), when not the room's.
    dof: bool,
    dof_dial: Option<(f32, f32)>,
    spread: Option<f32>,
    /// How far the cards' normal maps turn them (`Cards::normals`), when
    /// not the room's: 0 each card one face, as before the maps.
    normals: Option<f32>,
}

/// How many colours the still art is boiled down to for the first lit rung.
const STILL: usize = 4;

/// A rung: the dials of one step.
#[derive(Clone)]
struct Rung {
    id: &'static str,
    name: &'static str,
    /// The room as blocks lit through a look (rooms in 3D), or its still art.
    lit: bool,
    /// How far the floor tips (`Frame::tilt`).
    tilt: f32,
    /// Stage pixels to a pixel of the Delve's own grain, a stage of 384 x 224
    /// at 1: 2 is a stage twice as fine, half the grain 0.5.
    grain: f32,
    /// A flat rung's grain is made by taking the most common colour of each
    /// square of this many pixels (the art redrawn at a coarser grain, which
    /// no asset is yet); 1 leaves it.
    chunk: u32,
    /// The first floor as the Atari board (`arena::atari`, experimental):
    /// its own field and blocky sprites, flat and unlit. Floor one only.
    atari: bool,
    dials: Dials,
}

const DIALS_OFF: Dials = Dials {
    ramp: 8,
    still: 0.0,
    dither: Dither::Bayer4,
    shadows: true,
    lamps: (1.0, 1.0),
    sun: (1.0, 1.0),
    rise: 0.0,
    fog: None,
    cards: true,
    card_light: None,
    dof: true,
    dof_dial: None,
    spread: None,
    normals: None,
};

fn rungs() -> Vec<Rung> {
    vec![
        Rung { id: "a0", name: "the Atari board", lit: false, tilt: 0.0, grain: 1.0, chunk: 1, atari: true, dials: DIALS_OFF },
        Rung { id: "r0", name: "flat pixel", lit: false, tilt: 0.0, grain: 1.0, chunk: 2, atari: false, dials: DIALS_OFF },
        Rung { id: "r1", name: "tilted cards", lit: false, tilt: arena::stage_frame::TILT, grain: 1.0, chunk: 1, atari: false, dials: DIALS_OFF },
        Rung {
            id: "r2",
            name: "first light",
            lit: true,
           
            tilt: arena::stage_frame::TILT,
            grain: 1.0,
            chunk: 1,
            atari: false,
            dials: Dials { ramp: STILL, still: 0.5, lamps: (0.0, 0.0), sun: (1.0, 2.5), ..DIALS_OFF },
        },
        Rung { id: "r3", name: "the monolith", lit: true, tilt: arena::stage_frame::TILT, grain: 1.0, chunk: 1, atari: false, dials: DIALS_OFF },
        Rung {
            id: "r4",
            name: "image-like",
            lit: true,
           
            tilt: arena::stage_frame::TILT,
            grain: 2.0,
            chunk: 1,
            atari: false,
            dials: Dials { ramp: 16, dither: Dither::Bayer8, lamps: (1.15, 1.3), sun: (1.0, 1.6), rise: 90.0, ..DIALS_OFF },
        },
    ]
}

/// A dial moved from the command line, `r2.dither=none`: the rung, the dial,
/// its value (pairs as `a,b`).
fn set(rungs: &mut [Rung], word: &str) -> Result<(), String> {
    let bad = || format!("`{word}`: --set takes rung.dial=value, as r2.dither=none");
    let (path, value) = word.split_once('=').ok_or_else(bad)?;
    let (id, dial) = path.split_once('.').ok_or_else(bad)?;
    let rung = rungs.iter_mut().find(|r| r.id == id).ok_or_else(bad)?;
    let nums: Vec<f32> = value.split(',').filter_map(|v| v.parse().ok()).collect();
    let (one, two) = (nums.first().copied().ok_or_else(bad), nums.get(1).copied().ok_or_else(bad));
    match dial {
        "tilt" => rung.tilt = one?,
        "grain" => rung.grain = one?,
        "chunk" => rung.chunk = one? as u32,
        "ramp" => rung.dials.ramp = one? as usize,
        "still" => rung.dials.still = one?,
        "shadows" => rung.dials.shadows = one? > 0.5,
        "rise" => rung.dials.rise = one?,
        "fog" => rung.dials.fog = Some((one?, two?)),
        "lamps" => rung.dials.lamps = (one?, two?),
        "sun" => rung.dials.sun = (one?, two?),
        "cards" => {
            rung.dials.cards = one? > 0.0;
            rung.dials.card_light = two.ok().map(|most| (nums[0], most));
        }
        "dof" => {
            rung.dials.dof = one? > 0.0;
            rung.dials.dof_dial = two.ok().map(|margin| (nums[0], margin));
        }
        "spread" => rung.dials.spread = Some(one?),
        "normals" => rung.dials.normals = Some(one?),
        "dither" => {
            rung.dials.dither = match value {
                "none" => Dither::None,
                "4" => Dither::Bayer4,
                "8" => Dither::Bayer8,
                _ => return Err(bad()),
            }
        }
        _ => return Err(bad()),
    }
    Ok(())
}

/// A shot: its name and the run as it stands.
struct Case {
    name: String,
    run: Run,
    /// A painted wish the first knight wears (`crypt-painted`).
    painting: Option<Shield>,
}

/// A painted wish for the proof of normal maps (docs/WORLD-TIERS.md,
/// "Normal maps"): a round bronze shield, painted with its own light, and
/// the normal map its painter would ship beside it (`normal=`), which knows
/// its raised rim, boss and rivets where a map made from the painting knows
/// only its outline and its brightness.
struct Shield {
    rgba: Vec<u8>,
    normal: Vec<u8>,
}

/// The painting's side, in its own pixels.
const SHIELD: u32 = 96;
/// Stand-ins for the painting's and its normal map's sha256.
const SHIELD_SHA: &str = "b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0";
const SHIELD_NORMAL_SHA: &str = "c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1";

impl Shield {
    fn new() -> Shield {
        let n = SHIELD as usize;
        let (mut rgba, mut normal) = (vec![0u8; n * n * 4], vec![0u8; n * n * 4]);
        let c = SHIELD as f32 / 2.0;
        let rivets: Vec<(f32, f32)> =
            (0..8).map(|k| (k as f32 * std::f32::consts::TAU / 8.0 + 0.39)).map(|a| (c + 31.0 * a.cos(), c + 31.0 * a.sin())).collect();
        for y in 0..n {
            for x in 0..n {
                let (px, py) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
                let r = (px * px + py * py).sqrt();
                if r > 44.0 {
                    continue;
                }
                // The height of the shield here, for its normal: a low dome,
                // a raised rim, a high boss, eight rivets.
                let height = |px: f32, py: f32| -> f32 {
                    let r = (px * px + py * py).sqrt();
                    let mut h = 4.0 * (1.0 - (r / 44.0).powi(2));
                    if (37.0..=44.0).contains(&r) {
                        h += 3.5 * (1.0 - ((r - 40.5) / 3.5).powi(2)).max(0.0).sqrt();
                    }
                    if r < 12.0 {
                        h += 9.0 * (1.0 - (r / 12.0).powi(2)).sqrt();
                    }
                    for &(rx, ry) in &rivets {
                        let d = ((px + c - rx).powi(2) + (py + c - ry).powi(2)).sqrt();
                        if d < 3.0 {
                            h += 2.5 * (1.0 - (d / 3.0).powi(2)).sqrt();
                        }
                    }
                    h
                };
                let (dx, dy) = ((height(px + 0.75, py) - height(px - 0.75, py)) / 1.5, (height(px, py + 0.75) - height(px, py - 0.75)) / 1.5);
                let nrm = angel_stage::normals::encode([-dx, dy, 1.0]);
                // Painted: bronze, a darker band inside the rim, its own
                // light from the upper left.
                let lambert = {
                    let l = [-0.5f32, 0.6, 0.62];
                    let len = (dx * dx + dy * dy + 1.0).sqrt();
                    ((-dx * l[0] + dy * l[1] + l[2]) / len).clamp(0.0, 1.0)
                };
                let base: [f32; 3] = if r >= 37.0 {
                    [150.0, 104.0, 52.0]
                } else if r < 12.0 {
                    [196.0, 150.0, 70.0]
                } else if (33.0..37.0).contains(&r) {
                    [92.0, 60.0, 34.0]
                } else {
                    [128.0, 82.0, 44.0]
                };
                let shade = 0.55 + 0.6 * lambert;
                let o = (y * n + x) * 4;
                for k in 0..3 {
                    rgba[o + k] = (base[k] * shade).clamp(0.0, 255.0) as u8;
                }
                rgba[o + 3] = 255;
                normal[o..o + 4].copy_from_slice(&nrm);
            }
        }
        Shield { rgba, normal }
    }

    /// The card a knight wears it by: a worn piece on the body, its inks a
    /// disc of bronze.
    fn card() -> String {
        let side = 12;
        let rows: Vec<String> = (0..side)
            .map(|y| {
                (0..side)
                    .map(|x| {
                        let (dx, dy) = (x as f32 + 0.5 - side as f32 / 2.0, y as f32 + 0.5 - side as f32 / 2.0);
                        if dx * dx + dy * dy <= 36.0 { 'O' } else { '.' }
                    })
                    .collect()
            })
            .collect();
        format!("name Bossed Shield\nkind hold\narmor 1\ndraw worn at=body paint={SHIELD_SHA} normal={SHIELD_NORMAL_SHA}\n{}\n", rows.join("\n"))
    }

    /// The window's pictures of painting `p`, `scale` window pixels to a
    /// room pixel, as the window cuts them (`paintings::cut`), and its
    /// normal map's, authored or made from it; the picture's id.
    fn pictures(&self, p: &arena::Painting, scale: f32, authored: bool, out: &mut Vec<angel_stage::Picture>) -> Option<u32> {
        let side = arena::paintings::side_for(p.rect[2].max(p.rect[3]) * scale);
        let size = (SHIELD, SHIELD);
        let (side, cut) = arena::paintings::cut(&self.rgba, size, 1, side)?;
        let normals = if authored {
            arena::paintings::cut_normals(&self.rgba, &self.normal, size, 1, side)?
        } else {
            arena::paintings::made_normals(&cut, side, p.rect[2].max(p.rect[3]))
        };
        let id = arena::paintings::picture(0, p.frame, p.flip);
        let (rgba, map) = if p.flip {
            (arena::paintings::flipped(&cut[0], side), angel_stage::normals::mirrored(&normals[0], (side, side)))
        } else {
            (cut[0].clone(), normals[0].clone())
        };
        out.push(angel_stage::Picture { id, width: side, height: side, rgba });
        out.push(angel_stage::Picture { id: angel_stage::normals::picture_of(id), width: side, height: side, rgba: map });
        Some(id)
    }
}

/// Where a knight stands in room `room` (on a floor of `pack`): on open floor
/// just behind (north of) the tallest thing its lit blockout stands near
/// `near`, so it hides the knight's legs; the open floor nearest `near` when
/// nothing stands. Its feet, in the room's pixels.
fn spot(room: &Room, pack: Pack, near: (f32, f32), reach: i32) -> (f32, f32) {
    let walk = |c: i32, r: i32| {
        (0..room.cols as i32).contains(&c) && (0..room.rows as i32).contains(&r) && matches!(room.tile(c, r), Tile::Floor | Tile::Door)
    };
    let shapes: Vec<angel_stage::set::Shape> = rooms_lit::lit_for(room, pack)
        .map(|lit| {
            let (c, r) = ((near.0 / TILE) as i32, (near.1 / TILE) as i32);
            let rect = [(c - reach) as f32 * TILE, (r - reach) as f32 * TILE, (c + reach) as f32 * TILE, (r + reach) as f32 * TILE];
            lit.near(rect).iter().flat_map(|p| p.set.shapes.clone()).collect()
        })
        .unwrap_or_default();
    let tall = |c: i32, r: i32| rooms_lit::over_tile(&shapes, (c, r)).map_or(0.0, |s| s.at[1] + s.size[1]);
    let (nc, nr) = ((near.0 / TILE) as i32, (near.1 / TILE) as i32);
    let mut best: Option<(f32, i32, i32)> = None;
    for r in (nr - reach).max(1)..(nr + reach).min(room.rows as i32 - 2) {
        for c in (nc - reach).max(1)..(nc + reach).min(room.cols as i32 - 1) {
            if !walk(c, r) || walk(c, r + 1) {
                continue;
            }
            let h = tall(c, r + 1);
            if h < 8.0 {
                continue;
            }
            let d = ((c - nc).pow(2) + (r - nr).pow(2)) as f32;
            let score = h - d * 0.5;
            if best.is_none_or(|b| score > b.0) {
                best = Some((score, c, r));
            }
        }
    }
    if let Some((_, c, r)) = best {
        return ((c as f32 + 0.5) * TILE, (r + 1) as f32 * TILE - 2.0);
    }
    let mut open = None;
    for d in 0..reach * 2 {
        for r in nr - d..=nr + d {
            for c in nc - d..=nc + d {
                if open.is_none() && walk(c, r) {
                    open = Some(((c as f32 + 0.5) * TILE, (r as f32 + 0.5) * TILE));
                }
            }
        }
    }
    open.unwrap_or(near)
}

/// Stand the run's knights at `feet` and beside it (the room's pixels), the
/// view on the first.
fn stand(run: &mut Run, feet: (f32, f32)) {
    for (k, hero) in run.players.values_mut().enumerate() {
        let dx = k as f32 * 22.0;
        (hero.x, hero.y) = ((feet.0 + dx) / UNIT, feet.1 / UNIT);
        hero.camera = Some((hero.x, hero.y - 1.0));
    }
}

/// A run on floor `depth` of `pack` (seed `seed`) standing in its first room
/// of kind `kind`, two knights in it.
fn seeded(pack: Pack, depth: u32, seed: u64, kind: RoomKind) -> Option<Run> {
    for seed in seed..seed + 8 {
        let floor = layout::floor_party(depth, pack, 2, &mut Rng::new(seed));
        for at in (0..floor.rooms.len()).filter(|&i| floor.rooms[i].kind == kind) {
            let mut run = Run::new(seed, seed, Some("Friend"));
            run.join_seat(2, "Goose Inspector");
            run.dungeon = floor.clone();
            run.enter_for_test(at);
            let room = run.room().clone();
            let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
            let feet = spot(&room, pack, (w / 2.0, h / 2.0), 6);
            stand(&mut run, feet);
            if stood(&run) {
                return Some(run);
            }
        }
    }
    None
}

/// Whether the window stands run `run`'s room up in 3D.
fn stood(run: &Run) -> bool {
    world3d::set_enabled(true);
    arena::props_meshes(true);
    let stood = scene(run).world.is_some();
    world3d::set_enabled(false);
    stood
}

/// The room's view as the window makes it, the seat's bands left off.
fn scene(run: &Run) -> arena::RoomScene {
    arena::in_seat(|| arena::scene_for(run, NATIVE_W, NATIVE_H, Some(1)))
}

/// `run` with its knights stood by the brightest lamp its lit room has in
/// view (a brazier), just south of it on open floor: the lit
/// cards' warm case.
fn by_lamp(run: &Run) -> Option<Run> {
    world3d::set_enabled(true);
    arena::props_meshes(true);
    let scene = Arc::new(scene(run));
    let frame = StageMaker::default().make(&scene, |_| None);
    world3d::set_enabled(false);
    let look = frame.props?.world?.look?;
    let lamp = look.lamps.iter().max_by(|a, b| a.strength.total_cmp(&b.strength))?;
    let (cx, cy) = scene.camera;
    let at = (lamp.at[0] + cx, lamp.at[2] + cy + TILE * 0.6);
    let room = run.room().clone();
    let walk = |c: i32, r: i32| {
        (1..room.cols as i32 - 1).contains(&c) && (1..room.rows as i32 - 1).contains(&r) && matches!(room.tile(c, r), Tile::Floor)
    };
    let (nc, nr) = ((at.0 / TILE) as i32, (at.1 / TILE) as i32);
    let (c, r) = (0..6).find_map(|d| (nr - d..=nr + d).find_map(|r| (nc - d..=nc + d).find(|&c| walk(c, r) && walk(c + 1, r)).map(|c| (c, r))))?;
    let mut run = run.clone();
    stand(&mut run, ((c as f32 + 0.5) * TILE, (r as f32 + 0.8) * TILE));
    Some(run)
}

fn cases(only: &[String], open_seed: u64) -> Vec<Case> {
    let mut out = Vec::new();
    for (name, pack, depth, seed, kind) in
        [
            ("crypt-fight", Pack::Crypt, 1u32, 3u64, RoomKind::Fight),
            ("crypt-passage", Pack::Crypt, 2, 9, RoomKind::Hall),
            // The first floor's way down, and the second floor's first fight:
            // the step off the Atari board (`--rung a0 --rung r1`).
            ("crypt-stairs", Pack::Crypt, 1, 5, RoomKind::Stairs),
            ("crypt-fight-two", Pack::Crypt, 2, 3, RoomKind::Fight),
        ]
    {
        let brazier = name == "crypt-fight"
            && (only.is_empty() || only.iter().any(|o| "crypt-brazier".contains(o.as_str()) || o == "crypt-painted"));
        if !brazier && !only.is_empty() && !only.iter().any(|o| name.contains(o.as_str())) {
            continue;
        }
        let Some(run) = seeded(pack, depth, seed, kind) else {
            eprintln!("{name}: no {kind:?} room on {pack:?} floor {depth}");
            continue;
        };
        if brazier {
            match by_lamp(&run) {
                Some(run) => {
                    // The same, its first knight wearing a painted wish.
                    if only.iter().any(|o| o == "crypt-painted") {
                        let mut painted = run.clone();
                        let card = angel_delve::rules::cards::check("bossed-shield", &Shield::card()).map(|c| c.card);
                        match (card, painted.players.values_mut().next()) {
                            (Ok(card), Some(hero)) => {
                                hero.deck.push("bossed-shield".into());
                                painted.book.insert(card);
                                out.push(Case { name: "crypt-painted".into(), run: painted, painting: Some(Shield::new()) });
                            }
                            (Err(why), _) => eprintln!("crypt-painted: {why:?}"),
                            (_, None) => eprintln!("crypt-painted: no knight"),
                        }
                    }
                    if only.is_empty() || only.iter().any(|o| "crypt-brazier".contains(o.as_str())) {
                        out.push(Case { name: "crypt-brazier".into(), run, painting: None });
                    }
                }
                None => eprintln!("crypt-brazier: no lamp in the fight room's view"),
            }
        }
        if only.is_empty() || only.iter().any(|o| name.contains(o.as_str())) {
            out.push(Case { name: name.to_string(), run, painting: None });
        }
    }
    // The Crypt's bone chapel, a room with a painted plate in a pack that has one.
    if only.is_empty() || only.iter().any(|o| "crypt-bone-chapel".contains(o.as_str())) {
        match room::proof("crypt-bone-chapel") {
            Some(proof) => out.push(Case { name: "crypt-bone-chapel".into(), run: proof.run, painting: None }),
            None => eprintln!("crypt-bone-chapel: no proof room"),
        }
    }
    // An open floor: the map round the arrival.
    if only.is_empty() || only.iter().any(|o| "crypt-open-floor".contains(o.as_str())) {
        let mut run = Run::new(open_seed + 1, open_seed, Some("Friend"));
        run.first = Pack::Crypt;
        run.open_floor_for_test(1);
        let pack = run.dungeon.pack;
        if let Some(open) = run.dungeon.open.clone() {
            let map = run.room().clone();
            let arrival = (open.arrival.0 * UNIT, open.arrival.1 * UNIT);
            stand(&mut run, spot(&map, pack, arrival, 5));
            out.push(Case { name: "crypt-open-floor".into(), run, painting: None });
        }
    }
    out
}

/// `k` times the size of everything in the frame, so the stage is `k` times
/// as many pixels with the same picture on it: the world's nodes (what hangs
/// from another keeps its place in it), the look's distances, the layers. The
/// stage's camera takes its size from the stage, so nothing else moves.
fn regrain(frame: &mut angel_stage::Frame, k: f32) {
    if (k - 1.0).abs() < 1e-3 {
        return;
    }
    let nodes = |nodes: &mut Vec<Node>| {
        for n in nodes.iter_mut().filter(|n| n.parent.is_none()) {
            n.transform.translation = n.transform.translation.map(|v| v * k);
            n.transform.scale = n.transform.scale.map(|v| v * k);
        }
    };
    frame.size = (((frame.size.0 as f32 * k).round() as u32).max(1), ((frame.size.1 as f32 * k).round() as u32).max(1));
    for layer in &mut frame.layers {
        let Placement::Pixels(rect) = &mut layer.placement else {
            continue;
        };
        *rect = rect.map(|v| v * k);
        if let Stand::Upright(foot) = &mut layer.stand {
            *foot = foot.map(|v| v * k);
        }
        layer.height *= k;
        layer.breathe *= k;
        if let LayerKind::Particles(p) = &mut layer.kind {
            (p.size, p.rise, p.sway) = (p.size * k, p.rise * k, p.sway * k);
        }
    }
    if let Some(Props { nodes: shapes, world, .. }) = &mut frame.props {
        nodes(shapes);
        if let Some(world) = world {
            nodes(&mut world.floor);
            nodes(&mut world.blocks);
            nodes(&mut world.extra);
            nodes(&mut world.figures);
            if let Some(look) = &mut world.look {
                resize(look, k);
            }
        }
    }
}

/// A look's distances `k` times as long.
fn resize(look: &mut Look, k: f32) {
    let up = |p: &mut [f32; 3]| *p = p.map(|v| v * k);
    for lamp in &mut look.lamps {
        up(&mut lamp.at);
        lamp.reach *= k;
    }
    if let Some(sun) = &mut look.sun {
        sun.rise *= k;
        if let Some(shadow) = &mut sun.shadow {
            up(&mut shadow.focus);
            shadow.reach *= k;
        }
    }
    for reveal in look.reveal.iter_mut().chain(&mut look.reveals) {
        up(&mut reveal.at);
        reveal.radius *= k;
    }
    for zone in &mut look.zones {
        zone.at = zone.at.map(|v| v * k);
        zone.half = zone.half.map(|v| v * k);
        (zone.radius, zone.feather) = (zone.radius * k, zone.feather * k);
    }
    if let Some(fog) = &mut look.fog {
        (fog.start, fog.end) = (fog.start * k, fog.end * k);
    }
    if let Some(dof) = &mut look.dof {
        dof.focus.iter_mut().for_each(up);
        (dof.margin, dof.spread) = (dof.margin * k, dof.spread * k);
        dof.radius = (dof.radius * k).min(angel_stage::look::DOF_RADIUS_MAX);
    }
    look.metre *= k;
}

/// `ramp` as `n` entries: picked and mixed along its length, dark to light.
fn resample(ramp: &[[u8; 3]], n: usize) -> Vec<[u8; 3]> {
    (0..n)
        .map(|i| {
            let at = i as f32 * (ramp.len() - 1) as f32 / (n - 1).max(1) as f32;
            let (lo, t) = (at.floor() as usize, at.fract());
            let (a, b) = (ramp[lo], ramp[(lo + 1).min(ramp.len() - 1)]);
            std::array::from_fn(|c| (f32::from(a[c]) + (f32::from(b[c]) - f32::from(a[c])) * t).round() as u8)
        })
        .collect()
}

/// `room`'s brightness in the hue `still` is `by` of the way from it.
fn carry(room: [u8; 3], still: [u8; 3], by: f32) -> [u8; 3] {
    let luma = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let mixed: [f32; 3] = std::array::from_fn(|c| f32::from(room[c]) + (f32::from(still[c]) - f32::from(room[c])) * by);
    let k = luma(room.map(f32::from)) / luma(mixed).max(1.0);
    mixed.map(|v| (v * k).round().clamp(0.0, 255.0) as u8)
}

/// The room's look moved by `d`; `unit` stage pixels to a pixel of the
/// Delve's, for the sizes `d` names.
fn dial(look: &mut Look, d: &Dials, unit: f32, still: Option<&[[u8; 3]]>) {
    look.ramp = resample(&look.ramp, d.ramp);
    if let Some(still) = still.filter(|s| d.still > 0.0 && s.len() == d.ramp) {
        for (c, s) in look.ramp.iter_mut().zip(still) {
            *c = carry(*c, *s, d.still);
        }
    }
    look.warm = look.warm.take().map(|w| resample(&w, d.ramp));
    look.water = look.water.take().map(|w| resample(&w, d.ramp));
    for zone in &mut look.zones {
        zone.ramp = resample(&zone.ramp, d.ramp);
    }
    look.dither = d.dither;
    look.fog = d.fog.map(|(start, end)| Fog { color: d.ramp / 4, start: start * unit, end: end * unit });
    if let Some(sun) = &mut look.sun {
        sun.strength *= d.sun.0;
        sun.ambient *= d.sun.1;
        sun.rise = d.rise * unit;
        if !d.shadows {
            sun.shadow = None;
        }
    }
    if d.lamps.0 <= 0.0 {
        look.lamps.clear();
        look.warm = None;
    }
    for lamp in &mut look.lamps {
        lamp.strength *= d.lamps.0;
        lamp.reach *= d.lamps.1;
    }
    if !d.cards {
        look.cards = None;
    }
    if let (Some(cards), Some((least, most))) = (&mut look.cards, d.card_light) {
        (cards.least, cards.most) = (least, most);
    }
    if let (Some(cards), Some(normals)) = (&mut look.cards, d.normals) {
        cards.normals = normals;
    }
    if !d.dof {
        look.dof = None;
    }
    if let Some(dof) = &mut look.dof {
        if let Some((radius, margin)) = d.dof_dial {
            (dof.radius, dof.margin) = ((radius * unit).min(angel_stage::look::DOF_RADIUS_MAX), margin * unit);
        }
        if let Some(spread) = d.spread {
            dof.spread = spread * unit;
        }
    }
}

/// The four ways a lit rung is shot with `--pairs`: both dials that follow
/// the figures off, the lit cards alone, the depth of field alone, both.
const PAIRS: [(&str, bool, bool); 4] = [("off", false, false), ("cards", true, false), ("dof", false, true), ("both", true, true)];

/// Every figure card of `frame` beside its normal map (`normals::
/// picture_of`), a row each, each pixel eight across, on black: how the
/// maps read, pixel by pixel.
fn maps_sheet(frame: &angel_stage::Frame) -> Option<((u32, u32), Vec<u8>)> {
    const K: u32 = 8;
    let by_id: std::collections::HashMap<u32, &angel_stage::Picture> = frame.pictures.iter().map(|p| (p.id, p)).collect();
    let pairs: Vec<(&angel_stage::Picture, &angel_stage::Picture)> = frame
        .pictures
        .iter()
        .filter_map(|p| Some((p, *by_id.get(&angel_stage::normals::picture_of(p.id))?)))
        .filter(|(p, _)| p.id < angel_stage::normals::picture_of(p.id))
        .take(8)
        .collect();
    if pairs.is_empty() {
        return None;
    }
    let w = pairs.iter().map(|(p, _)| p.width * 2 + 3).max()? * K;
    let h = pairs.iter().map(|(p, _)| p.height + 2).sum::<u32>() * K;
    let mut out = vec![0u8; (w * h * 4) as usize];
    let mut y0 = K;
    for (picture, map) in pairs {
        for (x0, p) in [(K, picture), ((picture.width + 2) * K, map)] {
            for y in 0..p.height {
                for x in 0..p.width {
                    let i = ((y * p.width + x) * 4) as usize;
                    // A map's clear pixels left as clear as its picture's.
                    if picture.rgba[i + 3] == 0 {
                        continue;
                    }
                    let px = &p.rgba[i..i + 4];
                    for dy in 0..K {
                        for dx in 0..K {
                            let o = (((y0 + y * K + dy) * w + x0 + x * K + dx) * 4) as usize;
                            out[o..o + 4].copy_from_slice(&[px[0], px[1], px[2], 255]);
                        }
                    }
                }
            }
        }
        y0 += (picture.height + 2) * K;
    }
    Some(((w, h), out))
}

/// `n` colours that stand for the frame: the mean of each `n`th of its
/// pixels by brightness, dark to light (the black above a tilted floor and
/// what is clear left out).
fn still_ramp(rgba: &[u8], n: usize) -> Vec<[u8; 3]> {
    let luma = |c: &[u8]| 0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2]);
    let mut px: Vec<&[u8]> = rgba.chunks_exact(4).filter(|c| c[3] > 0 && luma(c) > 6.0).collect();
    px.sort_by(|a, b| luma(a).total_cmp(&luma(b)));
    (0..n)
        .map(|i| {
            let part = &px[px.len() * i / n..px.len() * (i + 1) / n];
            std::array::from_fn(|c| (part.iter().map(|p| u32::from(p[c])).sum::<u32>() / part.len().max(1) as u32) as u8)
        })
        .collect()
}

/// The most common colour of each `n` by `n` square, the first met among
/// equals: a frame at a coarser grain with its colours kept.
fn chunked(rgba: &[u8], size: (u32, u32), n: u32) -> Vec<u8> {
    let (w, h) = (size.0 as usize, size.1 as usize);
    let n = n as usize;
    let mut out = rgba.to_vec();
    for by in (0..h).step_by(n) {
        for bx in (0..w).step_by(n) {
            let mut seen: Vec<([u8; 4], u32)> = Vec::new();
            for y in by..(by + n).min(h) {
                for x in bx..(bx + n).min(w) {
                    let px: [u8; 4] = rgba[(y * w + x) * 4..][..4].try_into().unwrap_or_default();
                    match seen.iter_mut().find(|(c, _)| *c == px) {
                        Some((_, count)) => *count += 1,
                        None => seen.push((px, 1)),
                    }
                }
            }
            let top = seen.iter().fold(seen[0], |best, &now| if now.1 > best.1 { now } else { best }).0;
            for y in by..(by + n).min(h) {
                for x in bx..(bx + n).min(w) {
                    out[(y * w + x) * 4..][..4].copy_from_slice(&top);
                }
            }
        }
    }
    out
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_tiers <outdir> [--scale N] [--case NAME]... [--rung RN]... [--open SEED] [--set rung.dial=value]... [--hd PACK]";
    let mut rungs_all = rungs();
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut only, mut rungs, mut open_seed) = (None, 4u32, Vec::new(), Vec::new(), 41u64);
    let (mut pairs, mut normals, mut time) = (false, false, 0u32);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => {
                scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (2..=8).contains(&s)).ok_or("--scale takes 2 to 8")?;
            }
            "--case" => only.push(words.next().ok_or(usage)?),
            "--open" => open_seed = words.next().and_then(|w| w.parse().ok()).ok_or("--open takes a seed")?,
            "--rung" => rungs.push(words.next().ok_or(usage)?),
            "--set" => set(&mut rungs_all, &words.next().ok_or(usage)?)?,
            "--pairs" => pairs = true,
            "--normals" => normals = true,
            "--time" => time = words.next().and_then(|w| w.parse().ok()).filter(|&n| n > 0).ok_or("--time takes a count")?,
            "--hd" => {
                let dir = PathBuf::from(words.next().ok_or(usage)?);
                let pack = hd_pack::Pack::open(&dir).ok_or_else(|| format!("{}: no HD art pack", dir.display()))?;
                for warning in pack.warnings() {
                    eprintln!("hd pack: left out {warning}");
                }
                println!("hd pack {}: {} drawings at {}", dir.display(), pack.len(), pack.scale());
                hd_pack::install(Some(Arc::new(pack)));
            }
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let out = out.ok_or(usage)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut gpu = Gpu::new()?;
    arena::draw_parts_outside(true);
    let cases = cases(&only, open_seed);
    for case in &cases {
        let mut still: Option<Vec<[u8; 3]>> = None;
        for rung in &rungs_all {
            // The Atari board is the first floor's alone.
            if rung.atari && case.run.dungeon.depth != arena::atari::FLOOR {
                if rungs.iter().any(|w| w == rung.id) {
                    eprintln!("{}: {} is floor one only", case.name, rung.id);
                }
                continue;
            }
            world3d::set_enabled(rung.lit);
            arena::props_meshes(true);
            arena::atari::set(rung.atari);
            let scene = Arc::new(scene(&case.run));
            arena::atari::set(false);
            if rung.lit && scene.world.is_none() {
                return Err(format!("{}: not lit at {}", case.name, rung.id));
            }
            // A painted case's painting, its normal map its own or made.
            let authored = std::cell::Cell::new(true);
            let make = |dials: &Dials| {
                let (mut painted, mut drawn) = (Vec::new(), Vec::new());
                // The pack's drawings, as the window sends them (none without `--hd`).
                let mut art = hd_pack::HdArt::default();
                art.normals = dials.normals != Some(0.0);
                let mut frame = StageMaker::default().make_with(
                    &scene,
                    |body| art.picture(body, &mut drawn),
                    |p| case.painting.as_ref()?.pictures(p, scale as f32 * rung.grain, authored.get(), &mut painted),
                );
                frame.pictures.extend(painted);
                frame.pictures.extend(drawn);
                frame.tilt = rung.tilt;
                regrain(&mut frame, rung.grain);
                if let Some(look) = frame.props.as_mut().and_then(|p| p.world.as_mut()).and_then(|w| w.look.as_mut()) {
                    dial(look, dials, rung.grain, still.as_deref());
                }
                frame
            };
            let frame = make(&rung.dials);
            if rung.atari {
                // The board makes its own frame: flat, unlit.
                assert!(scene.board && frame.tilt == 0.0, "{}: the board's frame", case.name);
            }
            let size = (NATIVE_W as u32 * scale, NATIVE_H as u32 * scale);
            let wanted = rungs.is_empty() || rungs.iter().any(|w| w == rung.id);
            if rung.lit && wanted && normals {
                if rung.id == "r3" {
                    let made = make(&rung.dials);
                    if let Some((size, rgba)) = maps_sheet(&made) {
                        write_png(&out.join(format!("{}-maps.png", case.name)), size, &rgba)?;
                    }
                    if time > 0 {
                        // What making the figures' maps costs when they are
                        // all new (a maker keeps each look's after).
                        let figures: Vec<&angel_stage::Picture> = made
                            .pictures
                            .iter()
                            .filter(|p| (arena::stage_frame::WORLD_CARDS..arena::stage_frame::PICKUP_CARDS).contains(&p.id))
                            .collect();
                        let start = std::time::Instant::now();
                        for _ in 0..time {
                            for p in &figures {
                                std::hint::black_box(angel_stage::normals::of_picture(&p.rgba, (p.width, p.height), 1.0));
                            }
                        }
                        let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(time);
                        println!("{}: {} figures' normal maps made: {us:.1} us for all {} (CPU, none kept)", case.name, rung.id, figures.len());
                    }
                }
                // Both dials on, the cards each one face turned to the eye
                // and turned pixel by pixel by their maps; a painted case's
                // painting by its own map, and by one made from it.
                let ways: &[(&str, bool, bool)] = if case.painting.is_some() {
                    &[("flat", false, true), ("made", true, false), ("authored", true, true)]
                } else {
                    &[("flat", false, false), ("normals", true, false)]
                };
                for &(name, on, own) in ways {
                    authored.set(own);
                    let dials = Dials { cards: true, dof: true, normals: if on { rung.dials.normals } else { Some(0.0) }, ..rung.dials };
                    let pixels = gpu.stage(make(&dials), size);
                    write_png(&out.join(format!("{}-{}-{name}.png", case.name, rung.id)), size, &pixels)?;
                    if time > 0 {
                        let ms = gpu.time(make(&dials), size, time);
                        let stage = ((NATIVE_W as f32 * rung.grain).round(), (NATIVE_H as f32 * rung.grain).round());
                        println!("{}: {} {name}: {ms:.3} ms a frame (stage {} x {}, shown {} x {}, {time} frames)", case.name, rung.id, stage.0, stage.1, size.0, size.1);
                    }
                }
                authored.set(true);
            }
            if rung.lit && wanted && (pairs || (time > 0 && !normals)) {
                for (name, cards, dof) in PAIRS {
                    let dials = Dials { cards, dof, ..rung.dials };
                    if pairs {
                        let pixels = gpu.stage(make(&dials), size);
                        write_png(&out.join(format!("{}-{}-{name}.png", case.name, rung.id)), size, &pixels)?;
                    }
                    if time > 0 {
                        let ms = gpu.time(make(&dials), size, time);
                        let stage = ((NATIVE_W as f32 * rung.grain).round(), (NATIVE_H as f32 * rung.grain).round());
                        println!("{}: {} {name}: {ms:.3} ms a frame (stage {} x {}, shown {} x {}, {time} frames)", case.name, rung.id, stage.0, stage.1, size.0, size.1);
                    }
                }
            }
            let mut pixels = gpu.stage(frame, size);
            if rung.chunk > 1 {
                // The art at a coarser grain, its colours kept.
                pixels = chunked(&pixels, size, rung.chunk * scale);
            }
            if rung.id == "r1" {
                still = Some(still_ramp(&pixels, STILL));
            }
            if !rungs.is_empty() && !rungs.iter().any(|w| w == rung.id) {
                continue;
            }
            write_png(&out.join(format!("{}-{}.png", case.name, rung.id)), size, &pixels)?;
            let stage = ((NATIVE_W as f32 * rung.grain).round(), (NATIVE_H as f32 * rung.grain).round());
            let d = &rung.dials;
            let how = if rung.lit {
                let hue = if d.still > 0.0 { format!(" ({} of the still art's hue)", d.still) } else { String::new() };
                format!(
                    "lit, {} colours{hue}, {:?}, shadows {}, lamps {:?}, sun {:?}, rise {}, fog {:?}",
                    d.ramp, d.dither, d.shadows, d.lamps, d.sun, d.rise, d.fog
                )
            } else if rung.atari {
                "the Atari board: its own field, blocky sprites, unlit".to_string()
            } else if rung.chunk > 1 {
                format!("the still art, its pixels {} to a pixel", rung.chunk)
            } else {
                "the still art".to_string()
            };
            println!("{}: {} {}: tilt {}, stage {} x {}, {how}", case.name, rung.id, rung.name, rung.tilt, stage.0, stage.1);
        }
    }
    world3d::set_enabled(false);
    println!("wrote the rungs to {}", out.display());
    Ok(())
}

struct Gpu {
    core: Core,
    device: wgpu::Device,
}

impl Gpu {
    fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("delve-tiers"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device })
    }

    /// What drawing `frame`'s stage costs a frame, in milliseconds: set up
    /// as `stage` does, then drawn `n` times, each waited for (one first,
    /// unmeasured, for the pipelines and targets it makes).
    fn time(&mut self, frame: angel_stage::Frame, size: (u32, u32), n: u32) -> f64 {
        self.set(frame, size);
        let wait = |device: &wgpu::Device| {
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(5)) });
        };
        self.core.begin_frame();
        self.core.render_stages((1, 1), 0.0);
        wait(&self.device);
        let start = std::time::Instant::now();
        for k in 0..n {
            self.core.begin_frame();
            self.core.render_stages((1, 1), f64::from(k) / 60.0);
            wait(&self.device);
        }
        start.elapsed().as_secs_f64() * 1000.0 / f64::from(n)
    }

    /// A Delve frame as the stage, its pictures uploaded, in a box `size`
    /// pixels.
    fn set(&mut self, frame: angel_stage::Frame, size: (u32, u32)) {
        for p in frame.pictures {
            self.core.upload(Picture { id: p.id, width: p.width, height: p.height, rgba: p.rgba, cols: 0, rows: 0 });
        }
        self.core.set_stage(
            STAGE,
            Some(Stage {
                id: STAGE,
                size: frame.size,
                cols: size.0 as u16,
                rows: size.1 as u16,
                layers: frame.layers,
                scene: frame.scene,
                enter: None,
                tilt: frame.tilt,
                props: frame.props,
                impact: frame.impact,
            }),
        );
    }

    /// A Delve frame as a stage, drawn into a texture `size` pixels as the
    /// window draws a stage in its cells, on black, and read back.
    fn stage(&mut self, frame: angel_stage::Frame, size: (u32, u32)) -> Vec<u8> {
        self.set(frame, size);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("delve-tiers-target"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.core.begin_frame();
        self.core.render_stages((1, 1), 0.0);
        let shown = Instance {
            rect: [0.0, 0.0, size.0 as f32, size.1 as f32],
            uv: [0.0, 0.0, size.0 as f32, size.1 as f32],
            color: [1.0; 4],
            kind: angel_gpu::PICTURE,
            ..bytemuck::Zeroable::zeroed()
        };
        let mut parts = vec![(Draw::Picture { id: STAGE, linear: false }, vec![shown])];
        let mut drawn = Frame::default();
        drawn.assemble_into(&mut parts, &mut Vec::new());
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-tiers");
        self.core.read_back(&texture, size)
    }
}

fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = std::fs::File::create(path).map_err(|e| fail(&e))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| fail(&e))?;
    writer.write_image_data(rgba).map_err(|e| fail(&e))
}

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
//! cargo run --release -p angel-glass --example delve_tiers -- <outdir> [--scale N] [--case NAME]... [--rung RN]... [--open SEED] [--set rung.dial=value]...
//!
//! Writes `<case>-r0.png` to `<case>-r4.png`, each the room's view at `N`
//! window pixels to a pixel of the Delve's 384 x 224 (4 unless said), and
//! prints each rung's dials. `--set` moves one (`r2.dither=none`, `r4.grain=3`,
//! `r3.lamps=1.5,1`; see `set`). `scripts/tiers-sheet.py` lays the shots out
//! on a labelled contact sheet.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, NATIVE_H, NATIVE_W, UNIT, stage_frame::StageMaker};
use angel_delve::rules::{Pack, Rng, Room, RoomKind, Run, Tile, layout};
use angel_delve::world3d::{self, plain, room, rooms_lit};
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
    /// The room as the blocks its item's painted plate is cast on: the
    /// painted path (`ANGEL_WORLD3D_PLATES=1`, a pack of plates), not lit.
    paint: bool,
    /// How far the floor tips (`Frame::tilt`).
    tilt: f32,
    /// Stage pixels to a pixel of the Delve's own grain, a stage of 384 x 224
    /// at 1: the second art kit's stage is 2, half the grain 0.5.
    grain: f32,
    /// A flat rung's grain is made by taking the most common colour of each
    /// square of this many pixels (the art redrawn at a coarser grain, which
    /// no asset is yet); 1 leaves it.
    chunk: u32,
    dials: Dials,
}

const DIALS_OFF: Dials =
    Dials { ramp: 8, still: 0.0, dither: Dither::Bayer4, shadows: true, lamps: (1.0, 1.0), sun: (1.0, 1.0), rise: 0.0, fog: None };

fn rungs() -> Vec<Rung> {
    vec![
        Rung { id: "r0", name: "flat pixel", lit: false, paint: false, tilt: 0.0, grain: 1.0, chunk: 2, dials: DIALS_OFF },
        Rung { id: "r1", name: "tilted cards", lit: false, paint: false, tilt: arena::stage_frame::TILT, grain: 1.0, chunk: 1, dials: DIALS_OFF },
        Rung {
            id: "r2",
            name: "first light",
            lit: true,
            paint: false,
            tilt: arena::stage_frame::TILT,
            grain: 1.0,
            chunk: 1,
            dials: Dials { ramp: STILL, still: 0.5, dither: Dither::Bayer4, shadows: true, lamps: (0.0, 0.0), sun: (1.0, 2.5), rise: 0.0, fog: None },
        },
        Rung { id: "r3", name: "the monolith", lit: true, paint: false, tilt: arena::stage_frame::TILT, grain: 1.0, chunk: 1, dials: DIALS_OFF },
        Rung {
            id: "r4",
            name: "image-like",
            lit: true,
            paint: false,
            tilt: arena::stage_frame::TILT,
            grain: 2.0,
            chunk: 1,
            dials: Dials { ramp: 16, still: 0.0, dither: Dither::Bayer8, shadows: true, lamps: (1.15, 1.3), sun: (1.0, 1.6), rise: 90.0, fog: None },
        },
        // Past the climb: the painted path, a plate of the item's room cast on
        // its blocks. Only where `ANGEL_WORLD3D_PLATES=1` and a pack have it.
        Rung { id: "r5", name: "painted plate", lit: false, paint: true, tilt: arena::stage_frame::TILT, grain: 1.0, chunk: 1, dials: DIALS_OFF },
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
    let tall = |c: i32, r: i32| plain::over_tile(&shapes, (c, r)).map_or(0.0, |s| s.at[1] + s.size[1]);
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

fn cases(only: &[String], open_seed: u64) -> Vec<Case> {
    let mut out = Vec::new();
    for (name, pack, depth, seed, kind) in
        [("crypt-fight", Pack::Crypt, 1u32, 3u64, RoomKind::Fight), ("crypt-passage", Pack::Crypt, 2, 9, RoomKind::Hall)]
    {
        if !only.is_empty() && !only.iter().any(|o| name.contains(o.as_str())) {
            continue;
        }
        match seeded(pack, depth, seed, kind) {
            Some(run) => out.push(Case { name: name.to_string(), run }),
            None => eprintln!("{name}: no {kind:?} room on {pack:?} floor {depth}"),
        }
    }
    // The Crypt's bone chapel, a room with a painted plate in a pack that has one.
    if only.is_empty() || only.iter().any(|o| "crypt-bone-chapel".contains(o.as_str())) {
        match room::proof("crypt-bone-chapel") {
            Some(proof) => out.push(Case { name: "crypt-bone-chapel".into(), run: proof.run }),
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
            out.push(Case { name: "crypt-open-floor".into(), run });
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
    let usage = "usage: delve_tiers <outdir> [--scale N] [--case NAME]... [--rung RN]... [--open SEED] [--set rung.dial=value]...";
    let mut rungs_all = rungs();
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut only, mut rungs, mut open_seed) = (None, 4u32, Vec::new(), Vec::new(), 41u64);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => {
                scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (2..=8).contains(&s)).ok_or("--scale takes 2 to 8")?;
            }
            "--case" => only.push(words.next().ok_or(usage)?),
            "--open" => open_seed = words.next().and_then(|w| w.parse().ok()).ok_or("--open takes a seed")?,
            "--rung" => rungs.push(words.next().ok_or(usage)?),
            "--set" => set(&mut rungs_all, &words.next().ok_or(usage)?)?,
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
            // The lit rungs and the painted one are two paths of one switch
            // (`rooms_lit::on`), set once for the process.
            if (rung.lit && !rooms_lit::on()) || (rung.paint && rooms_lit::on()) {
                if rungs.iter().any(|w| w == rung.id) {
                    eprintln!("{}: {} needs {}", case.name, rung.id, if rung.paint { "ANGEL_WORLD3D_PLATES=1" } else { "no ANGEL_WORLD3D_PLATES" });
                }
                continue;
            }
            world3d::set_enabled(rung.lit || rung.paint);
            arena::props_meshes(true);
            let scene = Arc::new(scene(&case.run));
            if rung.lit && scene.world.as_ref().and_then(|w| w.lit()).is_none() {
                return Err(format!("{}: not lit at {}", case.name, rung.id));
            }
            if rung.paint && scene.world.as_ref().is_none_or(|w| w.lit().is_some()) {
                eprintln!("{}: no painted plate for this room (a pack: ANGEL_WORLD3D_ASSETS)", case.name);
                continue;
            }
            let mut frame = StageMaker::default().make(&scene, |_| None);
            frame.tilt = rung.tilt;
            regrain(&mut frame, rung.grain);
            if let Some(look) = frame.props.as_mut().and_then(|p| p.world.as_mut()).and_then(|w| w.look.as_mut()) {
                dial(look, &rung.dials, rung.grain, still.as_deref());
            }
            let size = (NATIVE_W as u32 * scale, NATIVE_H as u32 * scale);
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
            let how = if rung.paint {
                "a painted plate cast on the room's blocks".to_string()
            } else if rung.lit {
                let hue = if d.still > 0.0 { format!(" ({} of the still art's hue)", d.still) } else { String::new() };
                format!(
                    "lit, {} colours{hue}, {:?}, shadows {}, lamps {:?}, sun {:?}, rise {}, fog {:?}",
                    d.ramp, d.dither, d.shadows, d.lamps, d.sun, d.rise, d.fog
                )
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

    /// A Delve frame as a stage, drawn into a texture `size` pixels as the
    /// window draws a stage in its cells, on black, and read back.
    fn stage(&mut self, frame: angel_stage::Frame, size: (u32, u32)) -> Vec<u8> {
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

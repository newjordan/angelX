//! The first stone, in the realm's own renderer (the Works): a knight goes
//! down the Delve, cuts a block at a limestone face, carries it up the stair
//! to the stockyard in the bailey, has it cut at the banker, hods it up the
//! scaffold, sets it in the wall with a true set and a mason's mark, and the
//! camera stands back and the castle is there. Then the same with three
//! knights working together, each cutting, hodding and setting a stone.
//!
//! Everything the knights do is keys through `Run::step`, the timing played by
//! the clock as the tests' bots play it. What is staged is said in the doc of
//! each scene: the seam is won before the reel begins (a floor with a won
//! charter, `industry::proof_face`), and the hired hands are held still so the
//! stones in the film are the knights'.
//!
//! ANGEL_REALM_PLAN=tabula cargo run --profile fast -p angel-glass --example first_stone -- <outdir> [--scale N] [--dry] <scene>
//!
//! Scenes: `one`, `three`. Each writes `NNNN.png` (3D) and `NNNN-flat.png`
//! frames into `<outdir>/<scene>/`. `--dry` runs the script without drawing
//! and prints where each act got to.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use angel_delve::render::overworld::arena::{self, stage_frame::StageMaker};
use angel_delve::rules::hands::{self, Carry, Work};
use angel_delve::rules::industry::proof_face;
use angel_delve::rules::trades::Trade;
use angel_delve::rules::{Input, Pack, Run};
use angel_delve::world::{works, works_life as life};
use angel_delve::world3d;
use angel_gpu::{Core, Draw, Frame, Instance, Picture, Stage};

const STAGE: u32 = 0x00FA_57A4;

fn castle() -> &'static works::Castle {
    works::castle(works::KEEP).expect("the great tower")
}

/// The map tile a point of the castle's frame (metres) lies in.
fn tile_at(p: [f32; 2]) -> (i32, i32) {
    (works::ORIGIN.0 + (p[0] / 1.5).floor() as i32, works::ORIGIN.1 + (-p[1] / 1.5).floor() as i32)
}

/// The middle of a tile, in the castle's frame.
fn middle_of(t: (i32, i32)) -> [f32; 2] {
    [((t.0 - works::ORIGIN.0) as f32 + 0.5) * 1.5, -((t.1 - works::ORIGIN.1) as f32 + 0.5) * 1.5]
}

/// Whether a knight can stand on tile `t` with `laid` stones up.
fn open(laid: u32, t: (i32, i32)) -> bool {
    let (x0, y0, w, h) = castle().tiles();
    t.0 >= x0 && t.1 >= y0 && t.0 < x0 + w && t.1 < y0 + h && !castle().blocked(t.0, t.1, laid)
}

/// [`spot_near`], at least two metres from every spot in `apart`.
fn spot_apart(laid: u32, p: [f32; 2], reach: f32, toward: [f32; 2], apart: &[[f32; 2]]) -> [f32; 2] {
    let (tx, ty) = tile_at(p);
    let mut best: Option<([f32; 2], f32)> = None;
    for dy in -5..=5 {
        for dx in -5..=5 {
            let t = (tx + dx, ty + dy);
            let m = middle_of(t);
            let d = (m[0] - p[0]).hypot(m[1] - p[1]);
            if !open(laid, t) || d > reach || apart.iter().any(|a| (a[0] - m[0]).hypot(a[1] - m[1]) < 2.0) {
                continue;
            }
            let score = (m[0] - toward[0]).hypot(m[1] - toward[1]);
            if best.is_none_or(|(_, s)| score < s) {
                best = Some((m, score));
            }
        }
    }
    best.map_or(p, |(m, _)| m)
}

/// A way on foot from `a` to `b`: the middles of open tiles, found breadth first.
fn path(laid: u32, a: [f32; 2], b: [f32; 2]) -> Vec<[f32; 2]> {
    use std::collections::{HashMap, VecDeque};
    let (from, to) = (tile_at(a), tile_at(b));
    let mut came: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut queue = VecDeque::from([from]);
    came.insert(from, from);
    while let Some(t) = queue.pop_front() {
        if t == to {
            break;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (t.0 + dx, t.1 + dy);
            if !came.contains_key(&n) && open(laid, n) {
                came.insert(n, t);
                queue.push_back(n);
            }
        }
    }
    let mut out = vec![b];
    let mut at = to;
    while at != from && came.contains_key(&at) {
        out.push(middle_of(at));
        at = came[&at];
    }
    out.reverse();
    out
}

/// A knight who walks a path to its end, a waypoint at a time (`at` is the
/// waypoint they are on); true when they are there.
fn walk(run: &Run, id: u32, way: &[[f32; 2]], at: &mut usize) -> (Input, bool) {
    while *at < way.len() {
        let last = *at + 1 == way.len();
        let (keys, there) = steer(run, id, way[*at], if last { 0.55 } else { 0.55 });
        if !there {
            return (keys, false);
        }
        *at += 1;
    }
    (Input::default(), true)
}

/// The keys of a knight walking to `to` (castle metres); true when they are there.
fn steer(run: &Run, id: u32, to: [f32; 2], stop: f32) -> (Input, bool) {
    let h = &run.players[&id];
    let (tx, ty) = hands::units(to);
    let (dx, dy) = (tx - h.x, ty - h.y);
    if dx.hypot(dy) <= stop {
        return (Input::default(), true);
    }
    let sign = |d: f32| if d > 0.35 { 1 } else if d < -0.35 { -1 } else { 0 };
    (Input { move_x: sign(dx), move_y: sign(dy), ..Default::default() }, false)
}

/// What a setter who knows the timing does: `tap_off` ticks late on the plumb
/// and the level (0 is dead on the mark, a few more is merely good, a lot is
/// a miss).
fn setter_keys(run: &Run, id: u32, tap_off: (u64, u64)) -> Input {
    let now = run.tick + 1;
    let press = match run.hands.seats.get(&id).and_then(|h| h.work) {
        None => run.tick % 2 == 0,
        Some(Work::Set { began, plumb: None, .. }) => {
            let since = now - began;
            since >= hands::FLINCH && hands::plumb_dev(since.saturating_sub(tap_off.0)) == 0 && since >= 18 + tap_off.0
        }
        Some(Work::Set { tapped, plumb: Some(_), lands: 0, .. }) => {
            let since = now - tapped;
            since >= 22 + tap_off.1 && hands::level_dev(since - tap_off.1) <= 1 && since < 22 + tap_off.1 + 2
        }
        _ => false,
    };
    Input { swing: press, ..Default::default() }
}

/// A knight striking every perfect beat of what they work at.
fn strike_keys(run: &Run, id: u32) -> Input {
    let now = run.tick + 1;
    let press = match run.hands.seats.get(&id).and_then(|h| h.work) {
        None => run.tick % 2 == 0,
        Some(Work::Strike { job, began, .. }) => now > began && hands::beat_dev(job, now - began) == 0,
        _ => false,
    };
    Input { swing: press, ..Default::default() }
}


fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("first_stone: {why}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    "usage: first_stone <outdir> [--scale N] [--dry] (one | three)".into()
}

/// Where knight `id` stands, in the castle's frame.
fn metres_of(run: &Run, id: u32) -> [f32; 2] {
    let h = &run.players[&id];
    hands::metres((h.x, h.y))
}

/// The keys of a knight walking to the kingdom point `to` (units); true when there.
fn steer_units(run: &Run, id: u32, to: (f32, f32), stop: f32) -> (Input, bool) {
    let h = &run.players[&id];
    let (dx, dy) = (to.0 - h.x, to.1 - h.y);
    if dx.hypot(dy) <= stop {
        return (Input::default(), true);
    }
    let sign = |d: f32| if d > 0.35 { 1 } else if d < -0.35 { -1 } else { 0 };
    (Input { move_x: sign(dx), move_y: sign(dy), ..Default::default() }, false)
}

/// Frames of a reel: a frame every `every` ticks, 3D and flat, into `dir`;
/// none when dry.
struct Reel {
    gpu: Option<Gpu>,
    dir: PathBuf,
    scale: u32,
    every: u64,
    n: u32,
    ticks: u64,
    size: (u32, u32),
    /// Where each act began: its name and its first frame, written to
    /// `acts.txt` for the cut that follows.
    acts: Vec<(&'static str, u32)>,
}

impl Reel {
    fn tick(&mut self, run: &mut Run, keys: &[(u32, Input)]) -> Result<(), String> {
        let inputs: BTreeMap<u32, Input> = keys.iter().copied().collect();
        // The hired hands are held still, so the stones are the knights'; the
        // mill's boy keeps the tub for the film.
        run.hands.yard.acc = [0; 8];
        run.hands.yard.mortar = run.hands.yard.mortar.max(12);
        run.step(&inputs);
        self.ticks += 1;
        if self.ticks % self.every == 0 {
            self.shoot(run, false)?;
        }
        Ok(())
    }

    /// A frame of both views now (stood back when `held`).
    fn shoot(&mut self, run: &Run, held: bool) -> Result<(), String> {
        if let Some(gpu) = self.gpu.as_mut() {
            for on in [true, false] {
                self.size = frame_png(gpu, &self.dir, &format!("{:04}", self.n), run, on, self.scale, held)?;
            }
        }
        self.n += 1;
        Ok(())
    }

    /// An act begins at the next frame.
    fn act(&mut self, name: &'static str) {
        self.acts.push((name, self.n));
    }

    /// The picture held for `frames`, as at the end of an act.
    fn hold(&mut self, run: &Run, frames: u32) -> Result<(), String> {
        for _ in 0..frames {
            self.shoot(run, false)?;
        }
        Ok(())
    }

    /// `frames` of black, as a cut between acts.
    fn cut(&mut self, frames: u32) -> Result<(), String> {
        for _ in 0..frames {
            if self.gpu.is_some() {
                let blank = vec![0u8; (self.size.0 * self.size.1 * 4) as usize];
                let blank: Vec<u8> = blank.chunks(4).flat_map(|_| [0, 0, 0, 255]).collect();
                for name in [format!("{:04}.png", self.n), format!("{:04}-flat.png", self.n)] {
                    write_png(&self.dir.join(name), self.size, &blank)?;
                }
            }
            self.n += 1;
        }
        Ok(())
    }

    /// `ticks` of nothing happening.
    fn wait(&mut self, run: &mut Run, ticks: u64) -> Result<(), String> {
        for _ in 0..ticks {
            self.tick(run, &[])?;
        }
        Ok(())
    }
}

fn run() -> Result<(), String> {
    let mut words = std::env::args().skip(1);
    let mut out = None;
    let mut scale = 3u32;
    let mut dry = false;
    let mut rest = Vec::new();
    while let Some(w) = words.next() {
        match w.as_str() {
            "--scale" => scale = words.next().and_then(|v| v.parse().ok()).filter(|s| (1..=8).contains(s)).ok_or("--scale takes 1 to 8")?,
            "--dry" => dry = true,
            _ if out.is_none() => out = Some(PathBuf::from(w)),
            _ => rest.push(w),
        }
    }
    let out = out.ok_or_else(usage)?;
    let scene = rest.first().cloned().ok_or_else(usage)?;
    let dir = out.join(&scene);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if !works::on_map() {
        return Err("needs the blank-slate plan: ANGEL_REALM_PLAN=tabula".into());
    }
    let gpu = if dry { None } else { Some(Gpu::new()?) };
    if !dry {
        arena::draw_parts_outside(true);
        arena::props_meshes(true);
    }
    let mut reel = Reel { gpu, dir, scale, every: 2, n: 0, ticks: 0, size: (768, 448), acts: Vec::new() };
    if scene == "map" {
        return map();
    }
    let knights = match scene.as_str() {
        "one" => 1,
        "three" => 3,
        _ => return Err(usage()),
    };
    first_stone(&mut reel, knights)?;
    let acts: String = reel.acts.iter().map(|(name, at)| format!("{name} {at}\n")).collect();
    std::fs::write(reel.dir.join("acts.txt"), format!("{acts}end {}\n", reel.n)).map_err(|e| e.to_string())?;
    println!("{scene}: {} frames", reel.n);
    Ok(())
}

/// The bailey round the Delve's gate at a few stages of the build, as the
/// walls and ditch shut it: `.` open, `#` shut, `D` ditch, `g` the gate's rock,
/// `s` the spawn, `e` the stand before the gate, `y` the stockyard, `w` a
/// winch. Prints, for choosing where the gate's fixtures stand.
fn map() -> Result<(), String> {
    let c = castle();
    let gate = angel_delve::world::wishes::delve_gate();
    let stand = angel_delve::world::wishes::delve_stand();
    let (yx, yy) = angel_delve::rules::quarrying::yard_spot();
    let yard = ((yx / 2.0).floor() as i32, (yy / 2.0).floor() as i32);
    for laid in [0u32, 1_839, 5_000, 8_304, 11_266] {
        println!("with {laid} stones laid (x 134..=147, y 80..=93):");
        for y in 80..=93 {
            let row: String = (134..=147)
                .map(|x| {
                    if (x, y) == angel_delve::world::plan::tabula::KEEP_STAND {
                        's'
                    } else if (x, y) == stand {
                        'e'
                    } else if (x, y) == yard {
                        'y'
                    } else if x >= gate.tx && x < gate.tx + gate.tw && y >= gate.ty && y < gate.ty + gate.th {
                        'g'
                    } else if c.is_ditch(x, y) {
                        'D'
                    } else if c.blocked(x, y, laid) {
                        '#'
                    } else {
                        '.'
                    }
                })
                .collect();
            println!("  {y:3} {row}");
        }
    }
    println!("gate {gate:?}, stand {stand:?}, yard {yard:?}");
    Ok(())
}

// ------------------------------------------------------------------ acts --

/// The stone's whole way, for `knights` knights each bringing up, cutting,
/// hodding and setting a stone of their own.
fn first_stone(reel: &mut Reel, knights: u32) -> Result<(), String> {
    let ids: Vec<u32> = (1..=knights).collect();
    // The first stone of the film goes where there is room for every knight to
    // stand within reach of it and the two after it.
    let laid = (1_837..2_600)
        .find(|&l| plain_cuts(l, ids.len()) && course_spots(l, ids.len(), life::YARD).len() == ids.len())
        .ok_or("no course has room for the party to stand at it")?;
    reel.act("below");
    let (blocks, home, treasury) = below(reel, &ids)?;
    reel.cut(6)?;
    reel.act("up the stair");
    let mut run = surface(&ids, home, treasury, &blocks, laid);
    reel.wait(&mut run, 6)?;
    stockyard(reel, &mut run, &ids)?;
    reel.act("the works");
    let before = run.sites.keep().expect("a site").laid;
    works_the_stone(reel, &mut run, &ids)?;
    let site = run.sites.keep().expect("a site");
    println!(
        "set: {} stones, {} true, tally {:?}, the yard {:?}",
        site.laid - before,
        site.true_count(),
        site.tally(),
        run.home.stockyard.stone
    );
    if site.laid < before + knights {
        return Err(format!(
            "only {} of {knights} stones were set (the set not yet laid {:?}, the hands {:?})",
            site.laid - before,
            run.hands.yard.done,
            ids.iter().map(|id| (run.deeds_of(*id, Trade::Setter), run.hands.seats[id].said.clone())).collect::<Vec<_>>()
        ));
    }
    // The stone settles, held a moment; then the camera stands back and the
    // castle is there, the crane turning on.
    reel.wait(&mut run, 60)?;
    reel.hold(&run, 12)?;
    reel.act("the castle");
    for k in 0..120 {
        run.step(&BTreeMap::new());
        if k % 2 == 0 {
            reel.shoot(&run, true)?;
        }
    }
    Ok(())
}

/// ACT 1, below: a limestone face on the Mines' third floor, its charter won
/// (staged: the guards and the guardian have fallen, a mine stands on it). The
/// knights walk to it and swing until each parts a block, and turn away with
/// it on their shoulders. Returns the blocks each carries, and the realm's
/// home and treasury as they stand.
fn below(reel: &mut Reel, ids: &[u32]) -> Result<(Vec<(u32, String)>, angel_delve::rules::home::Home, angel_delve::realm::Spoils), String> {
    let mut run = proof_face(Pack::Cavern, 3, 11.0);
    for &id in ids.iter().skip(1) {
        run.join_seat(id, ["Goose Inspector", "Wren", "Corvid"][id as usize - 2]);
    }
    let face = run.industry_deposits().into_iter().next().ok_or("no face")?;
    for (k, &id) in ids.iter().enumerate() {
        let h = run.players.get_mut(&id).ok_or("no seat")?;
        (h.x, h.y) = (face.x - 11.0 - k as f32 * 0.8, face.y + 0.6 + (k as f32 - 1.0) * 1.1);
        h.aim_x = 1.0;
        h.aim_y = 0.0;
        h.invulnerable = angel_delve::rules::HZ * 600;
        h.camera = Some((h.x, h.y - 1.0));
    }
    reel.wait(&mut run, 12)?;
    // Up to the face, then swings until each has a block.
    let mut away = vec![false; ids.len()];
    for _ in 0..900 {
        let mut keys = Vec::new();
        for (k, &id) in ids.iter().enumerate() {
            let h = &run.players[&id];
            let to = (face.x - h.x).hypot(face.y - h.y);
            let carrying = h.block.is_some();
            let key = if carrying {
                // Away from the face, toward the stair (west).
                away[k] = true;
                Input { move_x: -1, ..Default::default() }
            } else if to > 2.6 {
                Input { move_x: 1, ..Default::default() }
            } else {
                Input { swing: true, aim_x: 1, ..Default::default() }
            };
            keys.push((id, key));
        }
        reel.tick(&mut run, &keys)?;
        if ids.iter().all(|id| run.players[id].block.is_some()) && away.iter().all(|&a| a) {
            break;
        }
    }
    // A few steps with the blocks on their backs.
    for _ in 0..24 {
        let keys: Vec<(u32, Input)> = ids.iter().map(|&id| (id, Input { move_x: -1, ..Default::default() })).collect();
        reel.tick(&mut run, &keys)?;
    }
    reel.hold(&run, 10)?;
    let blocks: Vec<(u32, String)> = ids.iter().filter_map(|&id| run.players[&id].block.clone().map(|b| (id, b))).collect();
    if blocks.len() != ids.len() {
        return Err(format!("only {} of {} blocks were cut", blocks.len(), ids.len()));
    }
    println!("below: {} blocks cut ({:?}), the face has given {:?}", blocks.len(), blocks, run.kingdom.stir.cut);
    Ok((blocks, run.home.clone(), run.treasury.clone()))
}

/// The party at home again, as the host starts the next run: the realm's home,
/// the knights' blocks still on their backs (`CarryBlock`), standing at the
/// Delve's gate, the Keep `laid` stones up and its bays raised ahead.
fn surface(ids: &[u32], home: angel_delve::rules::home::Home, treasury: angel_delve::realm::Spoils, blocks: &[(u32, String)], laid: u32) -> Run {
    let mut run = Run::at_home(0x3D, 0x3D, None, home, treasury);
    for &id in ids.iter().skip(1) {
        run.join_seat(id, ["Goose Inspector", "Wren", "Corvid"][id as usize - 2]);
    }
    for (id, stone) in blocks {
        run.carry_block(*id, stone).expect("a block comes up on a back");
    }
    run.walk_kingdom(Some(angel_delve::world::plan::tabula::delve_stand()));
    let stand = {
        let h = &run.players[&1];
        (h.x, h.y)
    };
    for (k, id) in ids.iter().enumerate() {
        let h = run.players.get_mut(id).expect("a seat");
        (h.x, h.y) = (stand.0 + (k as f32 - 1.0) * 1.3, stand.1 + 0.2 * k as f32);
        h.camera = Some((h.x, h.y - 1.0));
        h.stone = false;
        h.idle = false;
        h.aim_x = 0.0;
        h.aim_y = 1.0;
    }
    run.lay_to("great-tower", laid);
    run.hands.yard.timber = 4_000;
    // Each knight has the trades the film uses, the banker's in hand: each
    // other tool is taken up at its own place, by a press there.
    for &id in ids {
        run.teach_trade(id, Trade::Setter, 34);
        run.teach_trade(id, Trade::Labourer, 12);
        run.teach_trade(id, Trade::BankerMason, 12);
    }
    run
}

/// ACT 2: they walk the last yards to the stockyard and set their blocks down.
fn stockyard(reel: &mut Reel, run: &mut Run, ids: &[u32]) -> Result<(), String> {
    let (yx, yy) = angel_delve::rules::quarrying::yard_spot();
    let stands: Vec<(f32, f32)> = ids.iter().enumerate().map(|(k, _)| (yx + (k as f32 - 1.0) * 1.3, yy)).collect();
    for _ in 0..600 {
        let mut keys = Vec::new();
        for (k, &id) in ids.iter().enumerate() {
            if run.players[&id].block.is_some() {
                keys.push((id, steer_units(run, id, stands[k], 0.5).0));
            }
        }
        reel.tick(run, &keys)?;
        if ids.iter().all(|id| run.players[id].block.is_none()) {
            break;
        }
    }
    reel.wait(run, 30)?;
    reel.hold(run, 10)?;
    let yard = &run.home.stockyard;
    println!("yard: {:?}, hauled {}", yard.stone, yard.hauled);
    if yard.held() < ids.len() as u32 {
        return Err(format!("the yard holds {} of {} blocks", yard.held(), ids.len()));
    }
    Ok(())
}

/// What one knight is at, in the chain from the stockyard to the course.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum At {
    GoBench,
    Cut,
    GoYard,
    Take,
    GoWall,
    Climb,
    Set,
    Done,
}

/// One knight's place in ACT 3.
struct Worker {
    id: u32,
    at: At,
    way: Vec<[f32; 2]>,
    step: usize,
    /// The bench they cut at, the heap's stand, the wall's stand.
    bench: usize,
    yard: [f32; 2],
    wall: [f32; 2],
    /// The deeds in the trade at hand when the act began: the act is done when
    /// they have grown.
    deeds: u32,
    since: u64,
}

/// Whether the next `n` stones from `laid` are plain cuts, none a voussoir or
/// chevron stone that waits on a template from the Master Mason.
fn plain_cuts(laid: u32, n: usize) -> bool {
    let c = castle();
    (laid..laid + n as u32 + 1).all(|i| !matches!(c.role(c.stones[i as usize].role), "voussoir" | "chevron"))
}

/// The tiles a knight can walk to from the spawn with `laid` stones standing.
fn reachable(laid: u32) -> std::collections::HashSet<(i32, i32)> {
    use std::collections::{HashSet, VecDeque};
    let start = angel_delve::world::plan::tabula::KEEP_STAND;
    let mut seen: HashSet<(i32, i32)> = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some((x, y)) = queue.pop_front() {
        for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if !seen.contains(&next) && open(laid, next) {
                seen.insert(next);
                queue.push_back(next);
            }
        }
    }
    seen
}

/// `n` places to stand at the course, each within reach of the next three
/// stones, on ground a knight can walk to, clear of one another.
fn course_spots(laid: u32, n: usize, toward: [f32; 2]) -> Vec<[f32; 2]> {
    let c = castle();
    let ground = reachable(laid);
    let stones = [hands::stone_at(c, laid), hands::stone_at(c, laid + 1), hands::stone_at(c, laid + 2)];
    let (tx, ty) = tile_at(stones[0]);
    let mut cands: Vec<([f32; 2], f32)> = Vec::new();
    for dy in -6..=6 {
        for dx in -6..=6 {
            let t = (tx + dx, ty + dy);
            if !ground.contains(&t) {
                continue;
            }
            let m = middle_of(t);
            let far = stones.iter().map(|s| (m[0] - s[0]).hypot(m[1] - s[1])).fold(0.0f32, f32::max);
            if far <= hands::REACH_WALL - 0.4 {
                cands.push((m, (m[0] - toward[0]).hypot(m[1] - toward[1])));
            }
        }
    }
    cands.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut out: Vec<[f32; 2]> = Vec::new();
    for (m, _) in cands {
        if out.iter().all(|o| (o[0] - m[0]).hypot(o[1] - m[1]) >= 1.4) {
            out.push(m);
        }
        if out.len() == n {
            break;
        }
    }
    out
}

/// ACT 3: each knight cuts their block at a banker, takes the cut stone off
/// the yard's heap on the hod, climbs the scaffold with it and sets it at the
/// pegged course, plumb and level on the beat.
fn works_the_stone(reel: &mut Reel, run: &mut Run, ids: &[u32]) -> Result<(), String> {
    let laid = run.sites.keep().expect("a site").laid;
    let c = castle();
    let heap = life::YARD;
    let mut yards: Vec<[f32; 2]> = Vec::new();
    for k in 0..ids.len() {
        let yard = spot_apart(laid, heap, 2.2, [heap[0] - 3.0 + k as f32, heap[1] - 6.0], &yards);
        yards.push(yard);
    }
    let walls = course_spots(laid, ids.len(), yards[0]);
    if walls.len() < ids.len() {
        return Err(format!("{} places to stand at the course for {} knights", walls.len(), ids.len()));
    }
    println!("course spots {walls:?} for the stone at {:?}; heap stands {yards:?}", hands::stone_at(c, laid));
    let mut workers: Vec<Worker> = ids
        .iter()
        .enumerate()
        .map(|(k, &id)| Worker {
            id,
            at: At::GoBench,
            way: Vec::new(),
            step: 0,
            bench: if ids.len() == 1 { 1 } else { k },
            yard: yards[k],
            wall: walls[k],
            deeds: 0,
            since: 0,
        })
        .collect();
    for w in &mut workers {
        let b = life::BANKERS[w.bench];
        w.way = path(laid, metres_of(run, w.id), [b[0], b[1] + 1.15]);
    }
    for _ in 0..4_000 {
        let mut keys = Vec::new();
        let laid_now = run.sites.keep().expect("a site").laid;
        for w in &mut workers {
            let key = drive(run, w, laid_now);
            keys.push((w.id, key));
        }
        reel.tick(run, &keys)?;
        if workers.iter().all(|w| w.at == At::Done) {
            return Ok(());
        }
    }
    Err(format!(
        "the knights did not finish: {:?}",
        workers.iter().map(|w| (w.id, w.at, metres_of(run, w.id), w.wall, w.way.len(), w.step, run.hands.seats[&w.id].said.clone())).collect::<Vec<_>>()
    ))
}

/// One tick of a worker's keys; moves them to their next act when this one is done.
fn drive(run: &Run, w: &mut Worker, laid: u32) -> Input {
    let id = w.id;
    w.since += 1;
    let go = |w: &mut Worker, to: At, from: [f32; 2], dest: [f32; 2]| {
        w.way = path(laid, from, dest);
        w.step = 0;
        w.at = to;
        w.since = 0;
    };
    let here = metres_of(run, id);
    let hand = &run.hands.seats[&id];
    match w.at {
        At::GoBench | At::GoYard | At::GoWall => {
            let (keys, there) = walk(run, id, &w.way, &mut w.step);
            if there {
                w.since = 0;
                w.at = match w.at {
                    At::GoBench => {
                        w.deeds = run.deeds_of(id, Trade::BankerMason);
                        At::Cut
                    }
                    At::GoYard => At::Take,
                    _ => At::Climb,
                };
            }
            keys
        }
        At::Cut => {
            // The knight's own stone is cut when their deeds at the bench have grown.
            if run.deeds_of(id, Trade::BankerMason) > w.deeds || w.since > 900 {
                println!("tick {}: knight {id} leaves the bench after {} ticks; the yard's cut {}, the bench said {:?}", run.tick, w.since, run.hands.yard.cut, hand.said);
                go(w, At::GoYard, here, w.yard);
                return Input::default();
            }
            strike_keys(run, id)
        }
        At::Take => {
            if hand.carry == Carry::Stone {
                go(w, At::GoWall, here, w.wall);
                return Input::default();
            }
            // E, held off and on: the first takes up the hod, the next a stone.
            Input { interact: w.since % 8 < 2, ..Default::default() }
        }
        At::Climb => {
            if hand.carry == Carry::Nothing && w.since > 4 {
                w.at = At::Set;
                w.since = 0;
                w.deeds = run.deeds_of(id, Trade::Setter);
                return Input::default();
            }
            Input { interact: true, ..Default::default() }
        }
        At::Set => {
            if run.deeds_of(id, Trade::Setter) > w.deeds && hand.work.is_none() {
                w.at = At::Done;
                return Input::default();
            }
            setter_keys(run, id, (0, 0))
        }
        At::Done => Input::default(),
    }
}


/// A frame of `run` at the stage's size, flat (`on` false) or in 3D, to `<name>.png`.
fn frame_png(gpu: &mut Gpu, out: &Path, name: &str, run: &angel_delve::rules::Run, on: bool, scale: u32, held: bool) -> Result<(u32, u32), String> {
    let (w, h) = arena::seat_size();
    world3d::set_enabled(on);
    let map = arena::MapView { full: held, ..Default::default() };
    let scene = Arc::new(arena::seat_scene_with(run, w as i32, h as i32, Some(1), &map));
    let mut maker = StageMaker::default();
    let mut frame = maker.make(&scene, |_| None);
    gpu.upload(std::mem::take(&mut frame.pictures));
    frame = maker.make(&scene, |_| None);
    let back = (frame.size.0 / 384).max(1);
        let size = (frame.size.0 * scale / back, frame.size.1 * scale / back);
    let pixels = gpu.stage(frame, size, true);
    write_png(&out.join(format!("{name}{}.png", if on { "" } else { "-flat" })), size, &pixels)?;
    world3d::set_enabled(false);
    Ok(size)
}

struct Gpu {
    core: Core,
    device: wgpu::Device,
    target: Option<((u32, u32), wgpu::Texture)>,
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
            label: Some("delve-kingdom"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let core = Core::new(device.clone(), queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        Ok(Self { core, device, target: None })
    }

    fn upload(&mut self, pictures: Vec<angel_stage::Picture>) {
        for p in pictures {
            self.core.upload(Picture { id: p.id, width: p.width, height: p.height, rgba: p.rgba, cols: 0, rows: 0 });
        }
    }

    /// A Delve frame as a stage, drawn into a texture `size` pixels as the
    /// window draws a stage in its cells, on black, and (when `read`) read
    /// back; else waited for.
    fn stage(&mut self, mut frame: angel_stage::Frame, size: (u32, u32), read: bool) -> Vec<u8> {
        self.upload(std::mem::take(&mut frame.pictures));
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
        if self.target.as_ref().is_none_or(|(s, _)| *s != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("delve-kingdom-target"),
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            self.target = Some((size, texture));
        }
        let texture = &self.target.as_ref().expect("made above").1;
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
        self.core.draw(&view, size, wgpu::Color::BLACK, &drawn, "delve-kingdom");
        if read {
            return self.core.read_back(texture, size);
        }
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(5)) });
        Vec::new()
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

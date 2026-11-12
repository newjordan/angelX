//! The Works in the realm's 3D (`world::works`): a site's stones, falsework
//! and ditch stood up from the run's state, lit in the monolith look.
//!
//! - **Stones** (`works_mesh`): each finished lift is one mesh a chunk, kept;
//!   the lift going up is blocks of [`BLOCK`] stones, kept as they fill, and
//!   the last few stones are made as they are laid. The newest blocks are
//!   wet: their joints dark, drying as the course above them is laid
//!   ([`wet_stones`] stones' worth).
//! - **The ditch** is cut in the real ground: a chunk's `ground` statement
//!   with the corners over the dug rings lowered ([`dig`]), made again for
//!   the earth lifts done, so the chunk is sent to a page only when one is.
//! - **The stone's tone**: the castle's own ramp (the hillside's brightness
//!   ladder in limestone) in a zone over the site ([`dye`]), so the stones
//!   are stone and not the green of the hill they stand on.
//! - **Lamps**: a brazier by the gate and a lantern at the working face, the
//!   realm's light = activity.

use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::look::{Look, ZONE_MAX, Zone};
use angel_stage::scene::{AlphaMode, Material, MeshRef, Node, Role, Transform, quat_axis_angle};
use angel_stage::set::{Kind, SetLamp};

use super::kingdom::Chunk;
use super::realm_look;
use super::works_mesh::{self, BLOCK, Part};
use crate::rules::Run;
use crate::world::works::{self, Castle, PX_PER_M};

/// How many stones after it is laid a block's mortar is wet: a few minutes at the pace the hands and the hired hands set (`rules::hands`).
pub const WET_STONES: u32 = 64;

/// What a window draws of a site this frame.
#[derive(Debug)]
pub struct WorksView {
    pub plan: &'static str,
    pub laid: u32,
    /// Stone meshes, each with how wet its mortar is, 0 (dry) to 1.
    pub parts: Vec<(Arc<Part>, f32)>,
    /// Stone kind of each part. Empty means all limestone.
    pub kinds: Vec<u8>,
    /// The falsework standing.
    pub falsework: Vec<Arc<Part>>,
    pub lamps: Vec<SetLamp>,
    /// The site alive: the crane and its stone, the benches, the dust, the
    /// pegs and marks, as nodes in kingdom pixels (ids from [`LIFE_IDS`]).
    pub life: Vec<Node>,
    /// The box the whole castle fills, kingdom pixels.
    pub bounds: ([f32; 3], [f32; 3]),
    /// The crane's yard in kingdom pixels, `[x0, z0, x1, z1]`: where the
    /// look dyes the timber ramp (`works_yard`).
    pub yard: Option<[f32; 4]>,
    /// The lane the carts come by to it, in the same ramp.
    pub lane: Option<[f32; 4]>,
}

/// The first id of the site's own nodes, over the parts' (which count from 0).
pub const LIFE_IDS: u64 = 0x1_0000;

/// The Keep's site as the run has it, or `None` (no site, no castle known):
/// as it stands, or as the Chronicle replays it; the marks near a knight
/// (`knight`, kingdom pixels) shown.
pub fn of(run: &Run, knight: Option<[f32; 2]>) -> Option<Arc<WorksView>> {
    if !works::on_map() {
        return None;
    }
    let site = run.sites.keep()?;
    let castle = site.castle()?;
    let shown = run.site_shown(site);
    // The carpenters' bays stand ahead of the masons; the Chronicle shows the
    // build as it was, no bay before its time. The crane turns on the site's
    // own clock, so a knight on the wheel turns it faster.
    let built = if run.site_replay.is_some() { shown } else { run.hands.yard.timber };
    let mut v = view_banded(castle, shown, built, &site.lost, &site.bands);
    let yard = super::works_yard::of(castle, run, shown, built, run.crane_seconds());
    v.life = life(castle, site, shown, knight, yard.nodes);
    v.lamps.extend(yard.lamps);
    v.yard = yard.zone;
    v.lane = yard.lane;
    Some(Arc::new(v))
}


fn push_run(
    parts: &mut Vec<(Arc<Part>, f32)>,
    kinds: &mut Vec<u8>,
    castle: &'static Castle,
    from: u32,
    to: u32,
    state: u32,
    lost: &[u32],
    bands: &[(u32, u8)],
    wet: f32,
) {
    let mut a = from;
    while a < to {
        let kind = crate::rules::site::Site::stone_kind(bands, a);
        let mut b = a + 1;
        while b < to && crate::rules::site::Site::stone_kind(bands, b) == kind {
            b += 1;
        }
        let meshes = if lost.is_empty() { works_mesh::stones(castle, a, b, state) } else { works_mesh::stones_gapped(castle, a, b, state, lost) };
        for mesh in meshes {
            parts.push((mesh, wet));
            kinds.push(kind);
        }
        a = b;
    }
}

/// A castle with `laid` stones standing, with nothing alive about it yet.
pub fn view(castle: &'static Castle, laid: u32) -> WorksView {
    view_built(castle, laid, laid)
}

/// [`view`] with the falsework raised as far as stone `built`.
pub fn view_built(castle: &'static Castle, laid: u32, built: u32) -> WorksView {
    view_lost(castle, laid, built, &[])
}

/// [`view_built`] with the stones `lost` (sorted) knocked out of the lift in hand by a raid.
pub fn view_lost(castle: &'static Castle, laid: u32, built: u32, lost: &[u32]) -> WorksView {
    view_banded(castle, laid, built, lost, &[])
}

pub fn view_banded(castle: &'static Castle, laid: u32, built: u32, lost: &[u32], bands: &[(u32, u8)]) -> WorksView {
    let laid = laid.min(castle.len());
    let mut parts = Vec::new();
    let mut kinds = Vec::new();
    for stage in castle.stages.iter().filter(|s| s.count > 0) {
        let (first, end) = (stage.first, (stage.first + stage.count).min(castle.len()));
        if first >= laid {
            break;
        }
        if end <= laid {
            // A lift done: one mesh a chunk. Its mortar is as dry as its last block's.
            let wet = wetness(laid, end);
            push_run(&mut parts, &mut kinds, castle, first, end, end, &[], bands, wet);
            continue;
        }
        // The lift going up: blocks, the last of them as far as it is laid.
        let mut from = first;
        while from < laid {
            let to = (from + BLOCK).min(end);
            let (to_laid, state) = if to <= laid { (to, to) } else { (laid, laid) };
            let wet = if to <= laid { wetness(laid, to) } else { 1.0 };
            push_run(&mut parts, &mut kinds, castle, from, to_laid, state, lost, bands, wet);
            from = to;
        }
    }
    let falsework = works_mesh::falsework_built(castle, laid, built);
    let b = castle.bounds;
    let (lo, hi) = (castle.world([b[0], b[4], b[2]]), castle.world([b[3], b[1], b[5]]));
    WorksView {
        plan: castle.id.as_str(),
        laid,
        parts,
        kinds,
        falsework: falsework.into_iter().chain(pegs(castle, laid)).collect(),
        lamps: lamps(castle, laid),
        life: Vec::new(),
        yard: None,
        lane: None,
        bounds: ([lo[0].min(hi[0]), lo[1].min(hi[1]), lo[2].min(hi[2])], [lo[0].max(hi[0]), lo[1].max(hi[1]), lo[2].max(hi[2])]),
    }
}

/// How wet the mortar of stones laid up to `end` is with `laid` standing:
/// 1 as it is laid, drying to 0 over [`WET_STONES`] stones.
pub fn wetness(laid: u32, end: u32) -> f32 {
    let age = laid.saturating_sub(end);
    (1.0 - age as f32 / WET_STONES as f32).clamp(0.0, 1.0)
}

/// The light a site gives: a brazier in the bailey beside the gate's mouth
/// and a lantern up at the working face (the last stone laid).
fn lamps(castle: &Castle, laid: u32) -> Vec<SetLamp> {
    let mut out = vec![SetLamp { at: castle.world([-2.6, -12.4, 1.6]), reach: 64.0, strength: 0.85 }];
    if laid > 0 {
        let last = &castle.stones[(laid - 1) as usize];
        let c = last.corners();
        let mid = [0, 1, 2].map(|k| c.iter().map(|p| p[k]).sum::<f32>() / 8.0);
        let at = castle.world([mid[0], mid[1], mid[2] + 1.2]);
        out.push(SetLamp { at, reach: 72.0, strength: 0.7 });
    }
    out
}

/// The next lift pegged out in line before it is laid: stakes at the first
/// course's corners once the lift going up is two thirds laid.
fn pegs(castle: &'static Castle, laid: u32) -> Vec<Arc<Part>> {
    if laid == 0 || laid >= castle.len() {
        return Vec::new();
    }
    let here = castle.stage_at(laid);
    let s = &castle.stages[here];
    let done = laid.saturating_sub(s.first) as f32 / s.count.max(1) as f32;
    if done < 0.66 {
        return Vec::new();
    }
    let Some(next) = (here + 1..castle.stages.len()).find(|&k| castle.stages[k].count > 0) else { return Vec::new() };
    works_mesh::pegs(castle, next)
}

/// The site alive, as nodes in kingdom pixels.
fn life(castle: &'static Castle, site: &crate::rules::site::Site, laid: u32, knight: Option<[f32; 2]>, yard: Vec<Node>) -> Vec<Node> {
    use crate::world::works_life as wl;
    let k = PX_PER_M;
    let mut out: Vec<Node> = Vec::new();
    let mut id = LIFE_IDS;
    let mut push = |out: &mut Vec<Node>, mut n: Node| {
        n.id = id;
        id += 1;
        out.push(n);
    };
    let px = |p: [f32; 3]| castle.world(p);
    let cube = |c: [f32; 3], size_m: [f32; 3], yaw: f32, tone: f32| -> Node {
        let mut n = Node::new(0, MeshRef::Cube, Material::lit([tone, tone, tone, 1.0], 0.0));
        n.transform = Transform { translation: px(c), rotation: quat_axis_angle([0.0, 1.0, 0.0], yaw), scale: size_m.map(|v| v * k) };
        n
    };
    // The cutting benches, a block on each.
    if laid < castle.len() {
        for b in wl::BANKERS {
            push(&mut out, cube([b[0], b[1], 0.45], [1.5, 0.7, 0.9], 0.0, 0.34));
            push(&mut out, cube([b[0] + 0.1, b[1], 1.05], [0.7, 0.45, 0.3], 0.0, 0.66));
        }
    }
    // The yard alive: the crane, the stack at its foot, the carts, the tubs, the ladder (`works_yard`).
    for n in yard {
        push(&mut out, n);
    }
    // Dust where the last stones landed.
    for p in wl::dust(castle, laid) {
        let mut n = Node::new(0, MeshRef::Sphere { segments: 8 }, Material { alpha: AlphaMode::Blend, ..Material::lit([0.86, 0.84, 0.78, p.thick], 0.0) });
        n.transform = Transform { translation: px(p.at), scale: [p.radius * k; 3], ..Transform::IDENTITY };
        push(&mut out, n);
    }
    // The fire in the bailey's brazier.
    let at = px([-2.6, -12.4, 0.8]);
    let [r, g, b] = Kind::Fire.capture();
    let mut fire = Node::new(0, MeshRef::Cone { segments: 6 }, Material { accent: true, ..Material::color([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0]) });
    fire.transform = Transform { translation: [at[0], at[1] + 6.0, at[2]], scale: [0.5 * k, 1.1 * k, 0.5 * k], ..Transform::IDENTITY };
    push(&mut out, fire);
    push(&mut out, cube([-2.6, -12.4, 0.35], [0.8, 0.8, 0.7], 0.0, 0.5));
    // Mason's marks: on the stones near the knight, cut by whoever set them.
    if let Some(feet) = knight {
        let covers = works_mesh::covers(castle);
        let (ox, oz) = works::origin_px();
        let near = |c: [f32; 3]| {
            let w = castle.world(c);
            (w[0] - feet[0]).hypot(w[2] - feet[1])
        };
        let mut cands: Vec<(f32, u32)> = Vec::new();
        for i in 0..laid {
            let b = castle.stones[i as usize].aabb();
            // A box standing off the ground whose south face is open to the air.
            if !matches!(castle.stones[i as usize].shape, works::Shape::Box(_)) || b[2] < 20 || covers.faces[i as usize][2] <= laid {
                continue;
            }
            let c = [f32::from(b[0] + b[3]) / 200.0, f32::from(b[1]) / 100.0, f32::from(b[2] + b[5]) / 200.0];
            let d = near(c);
            // On the floor this near and not far above it: marks a knight reads.
            if d < 46.0 && c[2] * k < 40.0 {
                cands.push((d, i));
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for &(_, i) in cands.iter().take(12) {
            let b = castle.stones[i as usize].aabb().map(|v| f32::from(v) / 100.0);
            let s = 0.34;
            let (cx, cz) = ((b[0] + b[3]) / 2.0, (b[2] + b[5]) / 2.0);
            let _ = (ox, oz);
            for &(a0, b0, a1, b1) in wl::mark_strokes(site.setter(i)) {
                let (x0, z0) = (cx + (a0 - 0.5) * s, cz - (b0 - 0.5) * s);
                let (x1, z1) = (cx + (a1 - 0.5) * s, cz - (b1 - 0.5) * s);
                let (mx, mz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
                let len = (x1 - x0).hypot(z1 - z0).max(0.04);
                let ang = (z1 - z0).atan2(x1 - x0);
                let p = px([mx, b[1] - 0.02, mz]);
                let mut n = Node::new(0, MeshRef::Cube, Material::lit([0.1, 0.1, 0.1, 1.0], 0.0));
                n.transform = Transform { translation: p, rotation: quat_axis_angle([0.0, 0.0, 1.0], ang), scale: [len * k, 0.045 * k, 0.03 * k] };
                push(&mut out, n);
            }
        }
    }
    let _ = Role::Plain;
    out
}

// ------------------------------------------------------------ the ramp ----

/// The castle's stone as a ramp: the hillside's brightness ladder, in
/// limestone (`realm_look::cloth_ramp`, so the brand's eight steps hold).
pub fn stone_ramp() -> &'static [[u8; 3]; 8] {
    static RAMP: OnceLock<[[u8; 3]; 8]> = OnceLock::new();
    RAMP.get_or_init(|| {
        use angel_stage::brand::luma;
        // Each step keeps the hillside's luminance and takes a warm grey's
        // hue: limestone, not the green of the grass under it.
        let warm = [1.07f32, 1.0, 0.86];
        let norm = 0.2126 * warm[0] + 0.7152 * warm[1] + 0.0722 * warm[2];
        realm_look::HILLSIDE.map(|e| {
            let l = luma(e);
            warm.map(|w| ((l * w / norm) * 255.0).round().clamp(0.0, 255.0) as u8)
        })
    })
}

/// Dye the look over the site with the stone's own ramp, as a zone as wide
/// as the site (the ditch's outer rim), when it is in view. The camps' zones
/// give way if the look is full.
pub fn dye(look: &mut Look, view: &realm_look::View, works: &WorksView) {
    let [ox, oz] = view.origin;
    let (lo, hi) = works.bounds;
    // Seen: the site's box touches the view, with the tipped floor's reach.
    let beyond = 96.0;
    if hi[0] + beyond < ox || lo[0] - beyond > ox + view.size[0] || hi[2] + beyond < oz || lo[2] - beyond > oz + view.size[1] {
        return;
    }
    // The curtain's reach, the castle's middle: the ditch and the hillside
    // outside it keep the ground's own ramp.
    let (cx, cz) = works::origin_px();
    let half = [17.4 * PX_PER_M; 2];
    // Three slots: the Keep's own, and over it the crane's yard and the lane to it (timber, in the Keep's limestone).
    look.zones.truncate(ZONE_MAX - 3);
    look.zones.push(Zone { ramp: stone_ramp().to_vec(), at: [cx - ox, cz - oz], half, radius: 18.0, feather: 22.0 });
    for [x0, z0, x1, z1] in works.yard.into_iter().chain(works.lane) {
        look.zones.push(Zone { ramp: super::works_look::ramp(super::works_look::Material::Timber).to_vec(), at: [(x0 + x1) / 2.0 - ox, (z0 + z1) / 2.0 - oz], half: [(x1 - x0) / 2.0, (z1 - z0) / 2.0], radius: 0.0, feather: 5.0 });
    }
}

// ------------------------------------------------------------ the ditch ----

/// A chunk of the kingdom with the site's ditch dug into its ground, as far
/// as the earth lifts done say; the chunk as it is where the ditch does not
/// reach it or none is dug.
pub fn dig(run: &Run, chunk: &Arc<Chunk>) -> Arc<Chunk> {
    if !works::on_map() {
        return Arc::clone(chunk);
    }
    let Some(site) = run.sites.keep() else { return Arc::clone(chunk) };
    let Some(castle) = site.castle() else { return Arc::clone(chunk) };
    let shown = run.site_shown(site);
    let done = castle.earth_done(shown);
    if done == 0 || !touches(castle, chunk) {
        return Arc::clone(chunk);
    }
    static KEPT: Mutex<Vec<((u64, usize), Arc<Chunk>)>> = Mutex::new(Vec::new());
    let key = (chunk.key, done);
    if let Some((_, c)) = KEPT.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
        return Arc::clone(c);
    }
    let dug = dig_text(castle, &chunk.text, shown)
        .and_then(|text| chunk.cut(text.into()).map_err(|why| tracing::warn!("works: a dug chunk does not read: {why}")).ok())
        .map_or_else(|| Arc::clone(chunk), Arc::new);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if kept.len() >= 24 {
        kept.remove(0);
    }
    kept.push((key, Arc::clone(&dug)));
    dug
}

/// Whether the castle's earthworks lie over chunk `chunk`.
fn touches(castle: &Castle, chunk: &Chunk) -> bool {
    let (cx, cz) = super::kingdom::cell(chunk.index);
    let (x0, z0) = (cx as f32 * 384.0, cz as f32 * 256.0);
    let (lo, hi) = {
        let b = castle.bounds;
        (castle.world([b[0], b[4], b[2]]), castle.world([b[3], b[1], b[5]]))
    };
    let (minx, maxx, minz, maxz) = (lo[0].min(hi[0]), lo[0].max(hi[0]), lo[2].min(hi[2]), lo[2].max(hi[2]));
    x0 < maxx + 16.0 && x0 + 384.0 > minx - 16.0 && z0 < maxz + 16.0 && z0 + 256.0 > minz - 16.0
}

/// A chunk's text with its `ground` statement's corners lowered where the
/// earth lifts done with `laid` stones standing dug the ring, and darkened
/// where it is dug.
fn dig_text(castle: &Castle, text: &str, laid: u32) -> Option<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let at = lines.iter().position(|l| l.starts_with("ground "))?;
    let line = lines[at].clone();
    let (head, rest) = line.split_once(" heights")?;
    let (heights, tones) = rest.split_once(" tones")?;
    let mut h: Vec<f32> = heights.split_whitespace().map(|v| v.parse().ok()).collect::<Option<_>>()?;
    let mut t: Vec<f32> = tones.split_whitespace().map(|v| v.parse().ok()).collect::<Option<_>>()?;
    // `ground <cols> <rows> step <s> at <x> <z>`.
    let words: Vec<&str> = head.split_whitespace().collect();
    let (cols, rows): (usize, usize) = (words.get(1)?.parse().ok()?, words.get(2)?.parse().ok()?);
    let step: f32 = words.get(4)?.parse().ok()?;
    let (x0, z0): (f32, f32) = (words.get(6)?.parse().ok()?, words.get(7)?.parse().ok()?);
    if h.len() != (cols + 1) * (rows + 1) || t.len() != h.len() {
        return None;
    }
    let (ox, oz) = works::origin_px();
    let mut changed = false;
    // A corner is a cell of `step` pixels about it: five points in it.
    let r = step * 0.34;
    for j in 0..=rows {
        for i in 0..=cols {
            let (x, z) = (x0 + i as f32 * step, z0 + j as f32 * step);
            let (mx, my) = ((x - ox) / PX_PER_M, -(z - oz) / PX_PER_M);
            if mx.abs() > 24.0 || my.abs() > 25.0 {
                continue;
            }
            let mut depth = 0.0f32;
            let mut covered = 0.0f32;
            for e in castle.earth.iter().filter(|e| e.at <= laid) {
                let mut inside = 0;
                for (dx, dz) in [(0.0, 0.0), (r, r), (-r, r), (r, -r), (-r, -r)] {
                    let (px, py) = (mx + dx / PX_PER_M, my - dz / PX_PER_M);
                    // The causeway before the gate stays whole.
                    if px.abs() < 1.5 && py < 0.0 {
                        continue;
                    }
                    if works::dug(e, px, py) {
                        inside += 1;
                    }
                }
                let f = inside as f32 / 5.0;
                let metres = f32::from(e.depth) / 100.0;
                depth += if e.raise { -metres * f } else { metres * f };
                covered = covered.max(f);
            }
            let k = j * (cols + 1) + i;
            if depth != 0.0 {
                h[k] -= depth * PX_PER_M;
                t[k] = (t[k] * (1.0 - 0.45 * covered)).max(0.0);
                changed = true;
            }
            // The bailey is trodden earth, not the dark of the hillside: lifted
            // to a pale packed floor inside the curtain, falling off to the green
            // at the ditch's inner lip, so what stands there is read against it.
            let reach = 15.6f32;
            let inside = ((reach - mx.abs().max(my.abs() * 0.98)) / 2.4).clamp(0.0, 1.0);
            if inside > 0.0 {
                t[k] = t[k].max(24.0 + 26.0 * inside);
                changed = true;
            }
        }
    }
    if !changed {
        return None;
    }
    use std::fmt::Write as _;
    let mut out = format!("{head} heights");
    for v in &h {
        let r = (v * 10.0).round() / 10.0;
        let _ = if r == r.trunc() { write!(out, " {}", r as i32) } else { write!(out, " {r:.1}") };
    }
    out.push_str(" tones");
    for v in &t {
        let _ = write!(out, " {}", v.round().clamp(0.0, 100.0) as i32);
    }
    lines[at] = out;
    Some(lines.join("\n") + "\n")
}

#[cfg(test)]
pub(crate) fn touches_for_test(castle: &Castle, chunk: &Chunk) -> bool {
    touches(castle, chunk)
}

#[cfg(test)]
pub(crate) fn dig_text_for_test(castle: &Castle, text: &str, laid: u32) -> Option<String> {
    dig_text(castle, text, laid)
}

#[cfg(test)]
#[path = "../../tests/world3d/works__tests.rs"]
mod tests;

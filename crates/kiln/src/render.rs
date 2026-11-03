//! Drawing a recipe: its parts into a raster at a grain, lit; the vote down
//! to the small grains; the line work; the decals.
//!
//! Grains 4 and 8 sample each pixel's centre. Grains 1 and 2 are drawn at
//! grain 4 and voted down, each pixel taking what most of its samples saw
//! (the middle four count twice), so a part the width of a pixel shows
//! where it mostly lies, not only where it crosses a pixel's centre. A part
//! marked `keep` wins its pixel with a quarter of the vote, so a plume or a
//! blade thinner than a pixel survives at 16 px a tile. On the floor row
//! under the pivot a fifth of the vote inks a pixel, so a foot within a
//! quarter of a pixel of the floor stays on it. The line work comes after
//! the vote, at the grain's own pixels. A pose's shifts land on whole pixels
//! of the grain drawn ([`Pose::snapped`]).

use std::collections::BTreeMap;

use crate::grain::Grain;
use crate::light::{self, Finish};
use crate::line::{Lines, Mark};
use crate::palette::{Ink, Ramp};
use crate::raster::{PartId, Raster};
use crate::recipe::{Clip, Paint, Recipe};
use crate::rig::{self, Pose, Xf};
use crate::shape::Solid;

/// The grain the small grains are drawn at before the vote.
pub const VOTE_GRAIN: Grain = Grain(4);

/// One drawn picture: its pixels, what laid each one, and where the
/// sockets were.
#[derive(Clone, Debug)]
pub struct Frame {
    pub grain: Grain,
    pub raster: Raster,
    /// Per pixel, what laid it.
    pub marks: Vec<Mark>,
    /// Each socket's point in model space.
    pub sockets: BTreeMap<String, [f32; 2]>,
    /// Each part's move from where it is written, by part index.
    pub moves: Vec<Xf>,
    /// Whether each part was drawn, by part index.
    pub shown: Vec<bool>,
}

impl Frame {
    /// A socket's pixel at this grain.
    pub fn socket_px(&self, name: &str) -> Option<[i32; 2]> {
        let g = self.grain.px();
        self.sockets.get(name).map(|[x, y]| [(x * g).floor() as i32, (y * g).floor() as i32])
    }
}

/// A picture drawn by something other than a figure (an effect, a
/// reference) as a frame the look tools can show: every inked pixel fill,
/// no sockets, no parts.
pub fn picture(raster: Raster, grain: Grain) -> Frame {
    let marks = raster.ink.iter().map(|i| if i.is_clear() { Mark::Clear } else { Mark::Fill }).collect();
    Frame { grain, raster, marks, sockets: BTreeMap::new(), moves: Vec::new(), shown: Vec::new() }
}

/// Which parts a clip draws: every part not hidden, plus those it shows,
/// less those it hides; without `gear`, no part held in a socket.
pub fn shown(recipe: &Recipe, clip: Option<&Clip>, gear: bool) -> Vec<bool> {
    recipe
        .src
        .parts
        .iter()
        .map(|p| {
            let mut on = !p.hidden;
            if let Some(c) = clip {
                if c.show.contains(&p.id) {
                    on = true;
                }
                if c.hide.contains(&p.id) {
                    on = false;
                }
            }
            on && (gear || p.socket.is_none())
        })
        .collect()
}

/// The figure at rest.
pub fn still(recipe: &Recipe, grain: Grain) -> Frame {
    draw(recipe, grain, &Pose::rest(recipe), &shown(recipe, None, true))
}

/// Frame `index` of a clip; none when the recipe has no such clip.
pub fn clip_frame(recipe: &Recipe, grain: Grain, clip: &str, index: u32, gear: bool) -> Option<Frame> {
    let c = recipe.clip(clip)?;
    let pose = Pose::of_clip(recipe, c, index as f32);
    Some(draw(recipe, grain, &pose, &shown(recipe, Some(c), gear)))
}

/// Every frame of a clip.
pub fn clip_frames(recipe: &Recipe, grain: Grain, clip: &str, gear: bool) -> Vec<Frame> {
    let n = recipe.clip(clip).map_or(0, |c| c.frames);
    (0..n).filter_map(|i| clip_frame(recipe, grain, clip, i, gear)).collect()
}

/// Each part's ramp by part id (0 unused); a flat in one ink has that ink
/// alone.
fn ramps(recipe: &Recipe) -> Vec<Ramp> {
    let mut out = vec![Ramp(vec![recipe.outline])];
    for p in &recipe.parts {
        out.push(match p.paint {
            Paint::Lit { mat } => recipe.materials[mat].1.clone(),
            Paint::Ink(ink) => Ramp(vec![ink]),
        });
    }
    out
}

/// The figure in a pose, with the parts `shown` drawn.
pub fn draw(recipe: &Recipe, grain: Grain, pose: &Pose, shown: &[bool]) -> Frame {
    let world = pose.snapped(grain).world(recipe);
    let moves: Vec<Xf> = (0..recipe.parts.len()).map(|i| rig::part_xf(recipe, i, &world)).collect();
    let solids: Vec<Option<Solid>> = recipe
        .src
        .parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let drawn = shown.get(i).copied().unwrap_or(true) && p.grains.as_ref().is_none_or(|gs| gs.contains(&grain.0));
            drawn.then(|| Solid::of(p, moves[i]))
        })
        .collect();
    let work = if grain < VOTE_GRAIN { VOTE_GRAIN } else { grain };
    let lift = if grain == Grain::GAME { recipe.src.lift } else { 0.0 };
    let (ras, base) = paint(recipe, work, &solids, lift);
    let mut ras = if work != grain {
        let keep: Vec<bool> = std::iter::once(false).chain(recipe.src.parts.iter().map(|p| p.keep)).collect();
        let floor = ((recipe.src.pivot[1] * grain.px()).ceil() as u32).checked_sub(1);
        let (mut ras, _) = vote(&ras, &base, work.0 as u32 / grain.0 as u32, &keep, floor);
        thin(recipe, grain, &solids, &mut ras, lift);
        ras
    } else {
        ras
    };
    let ramps = ramps(recipe);
    let mut lines = Lines::new(&mut ras, &ramps, recipe.light);
    let brk = recipe.src.outline.inner * if grain == Grain::GAME { 2.0 } else { 1.0 };
    let parts = &recipe.src.parts;
    lines.inner(brk, |id| id as usize >= 1 && parts.get(id as usize - 1).is_some_and(|p| p.line));
    if grain == Grain::GAME {
        // Twice: a two-pixel gap is a one-pixel hole after the first.
        lines.close();
        lines.close();
        lines.edge();
    } else {
        lines.outline(recipe.outline, recipe.src.outline.selout);
    }
    lines.tidy(grain > Grain::GAME);
    let mut marks = std::mem::take(&mut lines.marks);
    decals(recipe, grain, &world, &mut ras, &mut marks);
    Frame { grain, raster: ras, marks, sockets: rig::sockets(recipe, &world), moves, shown: shown.to_vec() }
}

/// The parts, lit (their tones lifted by `lift`), into a raster at grain
/// `work`; and per pixel the volume under any surface marks.
fn paint(recipe: &Recipe, work: Grain, solids: &[Option<Solid>], lift: f32) -> (Raster, Vec<PartId>) {
    let (w, h) = recipe.size_at(work);
    let mut ras = Raster::new(w, h);
    let mut base: Vec<PartId> = vec![0; (w * h) as usize];
    let g = work.px();
    // Volumes first, then the marks on them, each in the recipe's order.
    for pass_marks in [false, true] {
        for (i, solid) in solids.iter().enumerate() {
            let Some(solid) = solid else { continue };
            let part = &recipe.src.parts[i];
            let on = recipe.parts[i].on;
            if on.is_some() != pass_marks {
                continue;
            }
            let id = (i + 1) as PartId;
            let Some((px0, py0, px1, py1)) = pixel_box(solid, g, w, h) else { continue };
            for py in py0..=py1 {
                for px in px0..=px1 {
                    let at = (py as u32 * w + px as u32) as usize;
                    if let Some(t) = on {
                        let t = (t + 1) as PartId;
                        if ras.part[at] != t && base[at] != t {
                            continue;
                        }
                    }
                    let (mx, my) = work.sample(px as u32, py as u32);
                    let Some(hit) = solid.hit(mx, my) else { continue };
                    let ink = tone(recipe, i, hit.n, solid.is_lit(), lift);
                    if on.is_some() {
                        ras.ink[at] = ink;
                        ras.part[at] = id;
                    } else if ras.put(px, py, ink, hit.z + part.bias, id) {
                        base[at] = id;
                    }
                }
            }
        }
    }
    (ras, base)
}

/// The pixels a solid's box covers at `g` pixels a game pixel, clipped to
/// a `w` x `h` picture.
fn pixel_box(solid: &Solid, g: f32, w: u32, h: u32) -> Option<(i32, i32, i32, i32)> {
    let [x0, y0, x1, y1] = solid.bounds();
    if x0 > x1 {
        return None;
    }
    let px0 = ((x0 * g).floor() as i32 - 1).max(0);
    let py0 = ((y0 * g).floor() as i32 - 1).max(0);
    let px1 = ((x1 * g).ceil() as i32 + 1).min(w as i32 - 1);
    let py1 = ((y1 * g).ceil() as i32 + 1).min(h as i32 - 1);
    (px0 <= px1 && py0 <= py1).then_some((px0, py0, px1, py1))
}

/// Kept tubes thinner than a pixel and a half at this grain, drawn as one
/// pixel line along their axis over the voted picture, where they are
/// nearest: what a pixel artist does with a blade or a strand at 16 px.
fn thin(recipe: &Recipe, grain: Grain, solids: &[Option<Solid>], ras: &mut Raster, lift: f32) {
    let g = grain.px();
    for (i, solid) in solids.iter().enumerate() {
        let Some(solid) = solid else { continue };
        let part = &recipe.src.parts[i];
        if !part.keep || recipe.parts[i].on.is_some() {
            continue;
        }
        let Some(axis) = solid.axis() else { continue };
        if axis.iter().map(|a| a.1).fold(0.0, f32::max) * 2.0 * g >= 1.5 {
            continue;
        }
        // The axis's pixels, in order, one a step along its longer way.
        let mut path: Vec<(i32, i32, [f32; 3])> = Vec::new();
        for w in axis.windows(2) {
            let (a, b) = (w[0].0, w[1].0);
            let n = ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()) * g).ceil().max(1.0) as i32;
            for k in 0..=n {
                let t = k as f32 / n as f32;
                let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
                let px = ((p[0] * g).floor() as i32, (p[1] * g).floor() as i32);
                if path.last().is_none_or(|l| (l.0, l.1) != px) {
                    path.push((px.0, px.1, p));
                }
            }
        }
        // One pixel wide: drop a step that only turns an L-corner.
        let mut clean: Vec<(i32, i32, [f32; 3])> = Vec::with_capacity(path.len());
        for (k, &p) in path.iter().enumerate() {
            if let (Some(&a), Some(&b)) = (clean.last(), path.get(k + 1)) {
                let diag = (a.0 - b.0).abs() == 1 && (a.1 - b.1).abs() == 1;
                if diag && (a.0 == p.0 || a.1 == p.1) && (b.0 == p.0 || b.1 == p.1) {
                    continue;
                }
            }
            clean.push(p);
        }
        let id = (i + 1) as PartId;
        for (px, py, p) in clean {
            let Some(hit) = solid.hit(p[0], p[1]) else { continue };
            let ink = tone(recipe, i, hit.n, solid.is_lit(), lift);
            ras.put(px, py, ink, hit.z + part.bias, id);
        }
    }
}

/// A part's ink where its surface faces `n`, a lit tone lifted by `lift`.
fn tone(recipe: &Recipe, part: usize, n: [f32; 3], lit: bool, lift: f32) -> Ink {
    let src = &recipe.src.parts[part];
    match recipe.parts[part].paint {
        Paint::Ink(ink) => ink,
        Paint::Lit { mat } => {
            let (_, ramp, finish, shade) = &recipe.materials[mat];
            if lit {
                let v = light::tone(*finish, n, recipe.light) + shade + src.shade;
                ramp.at(lift + (1.0 - lift) * v)
            } else {
                ramp.at(src.tone.unwrap_or(if *finish == Finish::Glow { 1.0 } else { 0.5 }) + shade + src.shade)
            }
        }
    }
}

/// A raster drawn at `f` times the grain, voted down: each pixel takes what
/// most of its samples saw, the middle ones counting twice. A kept part
/// needs only a quarter, and anything on the `floor` row (the row the pivot
/// stands on) a fifth, so feet keep their contact. Returns the voted raster
/// and its volumes under marks.
pub fn vote(src: &Raster, base: &[PartId], f: u32, keep: &[bool], floor: Option<u32>) -> (Raster, Vec<PartId>) {
    let (w, h) = (src.w / f, src.h / f);
    let mut out = Raster::new(w, h);
    let mut out_base = vec![0; (w * h) as usize];
    // Samples, the middle first so they settle ties; and their weights.
    let mut order: Vec<(u32, u32, u32)> = Vec::new();
    for sy in 0..f {
        for sx in 0..f {
            let mid = f >= 4 && (1..f - 1).contains(&sx) && (1..f - 1).contains(&sy);
            order.push((sx, sy, if mid { 2 } else { 1 }));
        }
    }
    order.sort_by_key(|&(_, _, wt)| std::cmp::Reverse(wt));
    let total: u32 = order.iter().map(|o| o.2).sum();
    let mut parts: Vec<(PartId, u32, f32)> = Vec::with_capacity(16);
    let mut inks: Vec<(PartId, Ink, u32, usize)> = Vec::with_capacity(16);
    for oy in 0..h {
        for ox in 0..w {
            parts.clear();
            inks.clear();
            let mut inked = 0;
            for (n, &(sx, sy, wt)) in order.iter().enumerate() {
                let i = ((oy * f + sy) * src.w + ox * f + sx) as usize;
                let ink = src.ink[i];
                if ink.is_clear() {
                    continue;
                }
                inked += wt;
                let (part, depth) = (src.part[i], src.depth[i]);
                match parts.iter_mut().find(|p| p.0 == part) {
                    Some(p) => {
                        p.1 += wt;
                        p.2 = p.2.max(depth);
                    }
                    None => parts.push((part, wt, depth)),
                }
                match inks.iter_mut().find(|k| k.0 == part && k.1 == ink) {
                    Some(k) => k.2 += wt,
                    None => inks.push((part, ink, wt, n)),
                }
            }
            if parts.is_empty() {
                continue;
            }
            let better = |a: &(PartId, u32, f32), b: &(PartId, u32, f32)| a.1 > b.1 || (a.1 == b.1 && a.2 > b.2);
            let mut win = parts[0];
            for p in &parts[1..] {
                if better(p, &win) {
                    win = *p;
                }
            }
            let kept = keep.get(win.0 as usize).copied().unwrap_or(false);
            let mut forced = false;
            if !kept {
                let mut best: Option<(PartId, u32, f32)> = None;
                for p in &parts {
                    if keep.get(p.0 as usize).copied().unwrap_or(false) && p.1 * 4 >= total && best.is_none_or(|b| better(p, &b)) {
                        best = Some(*p);
                    }
                }
                if let Some(b) = best {
                    win = b;
                    forced = true;
                }
            }
            // A foot within a quarter pixel of the floor is on it: one row of
            // samples, a fifth of the vote.
            let on_floor = floor == Some(oy) && inked * 5 >= total;
            if inked * 2 < total && !forced && !(kept && win.1 * 4 >= total) && !on_floor {
                continue;
            }
            let mut ink: Option<(Ink, u32, usize)> = None;
            for &(p, k, wt, n) in &inks {
                if p == win.0 && ink.is_none_or(|(_, bw, bn)| wt > bw || (wt == bw && n < bn)) {
                    ink = Some((k, wt, n));
                }
            }
            let o = (oy * w + ox) as usize;
            out.ink[o] = ink.map_or(Ink::CLEAR, |k| k.0);
            out.depth[o] = win.2;
            out.part[o] = win.0;
            // The volume under it: the first sample of the winner.
            out_base[o] = order
                .iter()
                .map(|&(sx, sy, _)| ((oy * f + sy) * src.w + ox * f + sx) as usize)
                .find(|&i| src.part[i] == win.0)
                .map_or(win.0, |i| base[i]);
        }
    }
    (out, out_base)
}

/// The decals drawn for this grain, each riding its bone.
fn decals(recipe: &Recipe, grain: Grain, world: &[Xf], ras: &mut Raster, marks: &mut [Mark]) {
    let Some(list) = recipe.decals.get(&grain.0) else { return };
    let g = grain.px();
    for d in list {
        let (mut ox, mut oy) = (d.at[0], d.at[1]);
        if let Some(b) = d.bone {
            let mid = [(d.at[0] as f32 + d.w as f32 / 2.0) / g, (d.at[1] as f32 + d.h as f32 / 2.0) / g];
            let to = world[b].apply(mid);
            ox += ((to[0] - mid[0]) * g).round() as i32;
            oy += ((to[1] - mid[1]) * g).round() as i32;
        }
        let on = d.on.map(|p| (p + 1) as PartId);
        for y in 0..d.h {
            for x in 0..d.w {
                let ink = d.inks[(y * d.w + x) as usize];
                let (px, py) = (ox + x as i32, oy + y as i32);
                if ink.is_clear() || px < 0 || py < 0 || px as u32 >= ras.w || py as u32 >= ras.h {
                    continue;
                }
                let at = (py as u32 * ras.w + px as u32) as usize;
                if let Some(t) = on
                    && (ras.part[at] != t || marks[at] != Mark::Fill)
                {
                    continue;
                }
                ras.ink[at] = ink;
                marks[at] = Mark::Decal;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BALL: &str = r#"{ "kiln": 0, "name": "ball", "canvas": [8, 8], "pivot": [4, 8],
        "materials": { "steel": { "ramp": "sxSuUvVW", "finish": "metal" } },
        "parts": [ { "id": "ball", "shape": "ellipsoid", "at": [4, 4, 0], "r": [3, 3, 3], "mat": "steel" },
                   { "id": "pin", "shape": "capsule", "from": [4, 0.5, 2], "to": [4, 2, 2], "r": [0.2], "mat": "steel", "keep": true } ] }"#;

    #[test]
    fn the_same_recipe_draws_the_same_pixels() {
        let r = Recipe::parse(BALL).unwrap();
        for g in Grain::ALL {
            let a = still(&r, g);
            assert_eq!(a.raster, still(&r, g).raster);
            assert_eq!((a.raster.w, a.raster.h), (8 * g.0 as u32, 8 * g.0 as u32));
        }
    }

    #[test]
    fn a_kept_part_thinner_than_a_pixel_survives_the_vote() {
        let r = Recipe::parse(BALL).unwrap();
        let f = still(&r, Grain::GAME);
        assert!(f.raster.part.iter().any(|&p| p == 2), "the pin vanished:\n{}", f.raster.ink_rows(crate::Palette::realm()).join("\n"));
        let mut unkept = BALL.replace("\"keep\": true", "\"keep\": false");
        unkept = unkept.replace("\"r\": [0.2]", "\"r\": [0.2]");
        let r = Recipe::parse(&unkept).unwrap();
        assert!(!still(&r, Grain::GAME).raster.part.iter().any(|&p| p == 2));
    }

    #[test]
    fn grain_four_has_an_outline_and_grain_one_an_edge() {
        let r = Recipe::parse(BALL).unwrap();
        let hd = still(&r, Grain::HD);
        assert!(hd.marks.contains(&Mark::Outline));
        let game = still(&r, Grain::GAME);
        assert!(game.marks.contains(&Mark::Edge) && !game.marks.contains(&Mark::Outline));
    }
}

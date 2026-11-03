//! Lints: what a recipe's pictures get wrong, each with its grain, clip and
//! frame and the pixels it is about, as text and as JSON.
//!
//! - `palette`: an ink that is not one of the realm's (error).
//! - `lone`: specks, a pixel inside one part's surface none of whose eight
//!   neighbours shares its ink (warning past a few; a one-pixel part, an eye
//!   or a gem, is a detail and not counted).
//! - `jaggy`: a one-pixel nub on the silhouette, or a line pixel doubling an
//!   L-corner (warning past a few).
//! - `silhouette`: at grain 1, a figure too small to read, a silhouette that
//!   strays from grain 4's, or a kept part lost (error) or a large part lost
//!   (warning).
//! - `floor`: the feet off the floor row under the pivot, at rest and in
//!   every frame (error).
//! - `sockets`: a socket off the canvas in any frame (error); a quick-rig
//!   socket missing when the recipe has clips (warning).
//! - `jitter`: a part moved less than a quarter of a pixel between two
//!   frames (or not at all) whose pixels changed where what it meets did not
//!   move: shimmer (warning).

use std::fmt::Write;

use serde::Serialize;

use crate::grain::Grain;
use crate::line::Mark;
use crate::palette::Palette;
use crate::recipe::Recipe;
use crate::render::{self, Frame};
use crate::rig::QUICK_RIG_SOCKETS;

/// How bad a lint is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Warning,
    Error,
}

/// One finding.
#[derive(Clone, Debug, Serialize)]
pub struct Lint {
    pub level: Level,
    pub code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grain: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
    pub message: String,
    /// The pixels it is about (at most a dozen), at its grain.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub at: Vec<[u32; 2]>,
}

/// Every finding for a recipe.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub name: String,
    pub errors: usize,
    pub warnings: usize,
    pub lints: Vec<Lint>,
}

impl Report {
    /// The report as text, one lint a line, worst first.
    pub fn text(&self) -> String {
        let mut s = String::new();
        for l in &self.lints {
            let level = match l.level {
                Level::Error => "error",
                Level::Warning => "warning",
                Level::Info => "info",
            };
            let mut place = String::new();
            if let Some(g) = l.grain {
                let _ = write!(place, " g{g}");
            }
            if let Some(c) = &l.clip {
                let _ = write!(place, " {c}");
                if let Some(f) = l.frame {
                    let _ = write!(place, "#{f}");
                }
            }
            let at = if l.at.is_empty() {
                String::new()
            } else {
                format!(" at {}", l.at.iter().map(|[x, y]| format!("{x},{y}")).collect::<Vec<_>>().join(" "))
            };
            let _ = writeln!(s, "{level} {}{place}: {}{at}", l.code, l.message);
        }
        let _ = writeln!(s, "{}: {} errors, {} warnings", self.name, self.errors, self.warnings);
        s
    }
}

/// A moment to check: the still, or a clip's frame.
struct Moment<'a> {
    clip: Option<&'a str>,
    frame: Option<u32>,
    grain: Grain,
    f: Frame,
}

fn lint(level: Level, code: &'static str, m: &Moment, message: String, at: Vec<[u32; 2]>) -> Lint {
    Lint { level, code, grain: Some(m.grain.0), clip: m.clip.map(str::to_string), frame: m.frame, message, at: at.into_iter().take(12).collect() }
}

/// Check a recipe: the still at grains 1, 2 and 4, and every clip's frames
/// at grains 1 and 4.
pub fn check(recipe: &Recipe) -> Report {
    let mut lints = Vec::new();
    let mut moments = Vec::new();
    for g in [1u8, 2, 4] {
        moments.push(Moment { clip: None, frame: None, grain: Grain(g), f: render::still(recipe, Grain(g)) });
    }
    for name in recipe.src.clips.keys() {
        for g in [1u8, 4] {
            for (i, f) in render::clip_frames(recipe, Grain(g), name, true).into_iter().enumerate() {
                moments.push(Moment { clip: Some(name), frame: Some(i as u32), grain: Grain(g), f });
            }
        }
    }
    for m in &moments {
        pixels(recipe, m, &mut lints);
    }
    silhouette(recipe, &moments, &mut lints);
    jitter(recipe, &moments, &mut lints);
    if !recipe.src.clips.is_empty() {
        let missing: Vec<&str> = QUICK_RIG_SOCKETS.into_iter().filter(|s| !recipe.src.rig.sockets.contains_key(*s)).collect();
        if !missing.is_empty() {
            lints.push(Lint {
                level: Level::Warning,
                code: "sockets",
                grain: None,
                clip: None,
                frame: None,
                message: format!("the quick-rig reads six sockets; missing {}", missing.join(", ")),
                at: Vec::new(),
            });
        }
    }
    lints.sort_by(|a, b| b.level.cmp(&a.level));
    let errors = lints.iter().filter(|l| l.level == Level::Error).count();
    let warnings = lints.iter().filter(|l| l.level == Level::Warning).count();
    Report { name: recipe.src.name.clone(), errors, warnings, lints }
}

/// Palette, lone pixels, jaggies, floor contact and sockets in one moment.
fn pixels(recipe: &Recipe, m: &Moment, lints: &mut Vec<Lint>) {
    let r = &m.f.raster;
    let p = Palette::realm();
    let (w, h) = (r.w as i32, r.h as i32);
    let mark = |x: i32, y: i32| if x < 0 || y < 0 || x >= w || y >= h { Mark::Clear } else { m.f.marks[(y * w + x) as usize] };
    let off: Vec<[u32; 2]> = (0..r.ink.len())
        .filter(|&i| !r.ink[i].is_clear() && p.rgb(r.ink[i]).is_none())
        .map(|i| [i as u32 % r.w, i as u32 / r.w])
        .collect();
    if !off.is_empty() {
        lints.push(lint(Level::Error, "palette", m, format!("{} pixels in no realm ink", off.len()), off));
    }
    // Lone pixels.
    let mut lone = Vec::new();
    let mut figure = 0u32;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if mark(x, y) != Mark::Clear {
                figure += 1;
            }
            if mark(x, y) != Mark::Fill {
                continue;
            }
            // A speck: inside one part's surface, and no neighbour shares
            // its ink. A one-pixel part (a gem, an eye) is a detail, not a speck.
            let (ink, part) = (r.ink[i], r.part[i]);
            let mut same = false;
            let mut inside = true;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
                if mark(x + dx, y + dy) != Mark::Fill || r.part_at(x + dx, y + dy) != part {
                    inside = false;
                }
                if r.get(x + dx, y + dy) == ink {
                    same = true;
                }
            }
            if inside && !same {
                lone.push([x as u32, y as u32]);
            }
        }
    }
    let allowed = if m.grain == Grain::GAME { 6 } else { (figure / 150).max(4) as usize };
    if lone.len() > allowed {
        lints.push(lint(Level::Warning, "lone", m, format!("{} specks, lone pixels inside a surface (more than {allowed})", lone.len()), lone));
    }
    // Nubs on the silhouette, and doubled L-corners of lines.
    let mut jag = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let here = mark(x, y);
            if here == Mark::Clear {
                continue;
            }
            let clear4 = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|&&(dx, dy)| mark(x + dx, y + dy) == Mark::Clear).count();
            if clear4 == 3 {
                jag.push([x as u32, y as u32]);
                continue;
            }
            if here.is_line() {
                let same = |dx: i32, dy: i32| mark(x + dx, y + dy) == here;
                let around = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)].iter().filter(|&&(dx, dy)| same(dx, dy)).count();
                if here != Mark::Outline && around == 2 && (same(1, 0) || same(-1, 0)) && (same(0, 1) || same(0, -1)) {
                    jag.push([x as u32, y as u32]);
                }
            }
        }
    }
    let allowed = if m.grain == Grain::GAME { 4 } else { (figure / 200).max(6) as usize };
    if jag.len() > allowed {
        lints.push(lint(Level::Warning, "jaggy", m, format!("{} nubs or doubled corners (more than {allowed})", jag.len()), jag));
    }
    // Floor contact: the lowest figure row stands on the pivot's row.
    if let Some((_, y, _, bh)) = r.bounds() {
        let g = m.grain.px();
        let floor = ((recipe.src.pivot[1] * g).ceil() as i32 - 1).max(0);
        let low = (y + bh - 1) as i32;
        // At grain 2 and up the outline is the row under the feet.
        let slack = if m.grain == Grain::GAME { 0 } else { 1 };
        if low < floor - slack || low > floor + 1 {
            let how = if low < floor { "above" } else { "below" };
            lints.push(lint(
                Level::Error,
                "floor",
                m,
                format!("the feet end on row {low}, {} rows {how} the floor row {floor}", (low - floor).abs()),
                Vec::new(),
            ));
        }
    }
    // Sockets on the canvas.
    for (name, [x, y]) in &m.f.sockets {
        let [w, h] = recipe.src.canvas;
        if *x < 0.0 || *y < 0.0 || *x >= w as f32 || *y >= h as f32 {
            lints.push(lint(Level::Error, "sockets", m, format!("{name} at {x:.2},{y:.2} is off the {w}x{h} canvas"), Vec::new()));
        }
    }
}

/// The silhouette at grain 1 against grain 4's, the figure's size, and the
/// parts lost.
fn silhouette(recipe: &Recipe, moments: &[Moment], lints: &mut Vec<Lint>) {
    let (Some(g1), Some(g4)) = (
        moments.iter().find(|m| m.clip.is_none() && m.grain == Grain::GAME),
        moments.iter().find(|m| m.clip.is_none() && m.grain == Grain::HD),
    ) else {
        return;
    };
    let (a, b) = (&g1.f.raster, &g4.f.raster);
    if let Some((_, _, w, h)) = a.bounds()
        && (h < 8 || w < 4)
    {
        lints.push(lint(Level::Error, "silhouette", g1, format!("the figure is {w}x{h} at grain 1: too small to read"), Vec::new()));
    }
    // Grain 4's fill, voted down to grain 1 by half its samples.
    let (mut both, mut either) = (0u32, 0u32);
    let mut strays = Vec::new();
    for y in 0..a.h {
        for x in 0..a.w {
            let mut n = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let i = ((y * 4 + sy) * b.w + x * 4 + sx) as usize;
                    if matches!(g4.f.marks[i], Mark::Fill | Mark::Inner | Mark::Decal) {
                        n += 1;
                    }
                }
            }
            let (s1, s4) = (a.is_inked(x as i32, y as i32), n >= 8);
            if s1 && s4 {
                both += 1;
            }
            if s1 || s4 {
                either += 1;
            }
            if s1 != s4 {
                strays.push([x, y]);
            }
        }
    }
    let iou = if either == 0 { 1.0 } else { both as f32 / either as f32 };
    if iou < 0.75 {
        lints.push(lint(Level::Warning, "silhouette", g1, format!("grain 1's silhouette is {:.0}% of grain 4's (under 75%)", iou * 100.0), strays));
    } else {
        lints.push(lint(Level::Info, "silhouette", g1, format!("grain 1's silhouette is {:.0}% of grain 4's", iou * 100.0), Vec::new()));
    }
    let n = recipe.src.parts.len();
    let s1 = super::describe::seen(&g1.f, n);
    let s4 = super::describe::seen(&g4.f, n);
    for k in 0..n {
        let part = &recipe.src.parts[k];
        if part.grains.as_ref().is_some_and(|g| !g.contains(&1)) || s4[k].px == 0 || s1[k].px > 0 {
            continue;
        }
        if part.keep {
            lints.push(lint(Level::Error, "silhouette", g1, format!("{} is kept but lost at grain 1", part.id), Vec::new()));
        } else if s4[k].px >= 64 {
            lints.push(lint(
                Level::Warning,
                "silhouette",
                g1,
                format!("{} ({} px at grain 4) is lost at grain 1; keep it, or say grains", part.id, s4[k].px),
                Vec::new(),
            ));
        }
    }
}

/// Parts that shimmer: moved by less than a quarter of a pixel between two
/// frames (or not at all), yet their pixels change where what they meet did
/// not move.
fn jitter(recipe: &Recipe, moments: &[Moment], lints: &mut Vec<Lint>) {
    // Each part's box in its own space, to measure how far a move takes it.
    let own: Vec<[f32; 4]> = recipe.src.parts.iter().map(|p| crate::shape::Solid::of(p, crate::rig::Xf::ID).bounds()).collect();
    for name in recipe.src.clips.keys() {
        for g in [1u8, 4] {
            let frames: Vec<&Moment> = moments.iter().filter(|m| m.clip == Some(name.as_str()) && m.grain.0 == g).collect();
            if frames.len() < 2 {
                continue;
            }
            for i in 0..frames.len() {
                let (a, b) = (&frames[i].f, &frames[(i + 1) % frames.len()].f);
                // How far each part moves, in this grain's pixels.
                let far: Vec<f32> = (0..a.moves.len())
                    .map(|k| {
                        let [x0, y0, x1, y1] = own[k];
                        if x0 > x1 {
                            return 0.0;
                        }
                        [[x0, y0], [x1, y0], [x0, y1], [x1, y1]]
                            .iter()
                            .map(|&c| {
                                let (p, q) = (a.moves[k].apply(c), b.moves[k].apply(c));
                                ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt() * g as f32
                            })
                            .fold(0.0, f32::max)
                    })
                    .collect();
                let still = |p: u16| p >= 1 && (p as usize) <= far.len() && far[p as usize - 1] < 0.25;
                let unmoved = |p: u16| p == 0 || ((p as usize) <= far.len() && far[p as usize - 1] < 0.01);
                let mut shimmer: Vec<(usize, [u32; 2])> = Vec::new();
                for k in 0..a.moves.len() {
                    if !still((k + 1) as u16) || !a.shown[k] || !b.shown[k] {
                        continue;
                    }
                    let id = (k + 1) as u16;
                    for px in 0..a.raster.ink.len() {
                        let (pa, pb) = (a.raster.part[px], b.raster.part[px]);
                        if (pa == id) == (pb == id) {
                            continue;
                        }
                        // Covered or uncovered by something that moved, or at a line.
                        let other = if pa == id { pb } else { pa };
                        if !unmoved(other) || a.marks[px].is_line() || b.marks[px].is_line() {
                            continue;
                        }
                        shimmer.push((k, [px as u32 % a.raster.w, px as u32 / a.raster.w]));
                    }
                }
                if !shimmer.is_empty() {
                    let mut parts: Vec<&str> = shimmer.iter().map(|(k, _)| recipe.src.parts[*k].id.as_str()).collect();
                    parts.dedup();
                    let m = frames[i];
                    lints.push(lint(
                        Level::Warning,
                        "jitter",
                        m,
                        format!("{} px of {} changed to the next frame though they moved less than a quarter of a pixel", shimmer.len(), parts.join(", ")),
                        shimmer.into_iter().map(|(_, p)| p).collect(),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_knight_passes_its_checks() {
        let r = Recipe::parse(include_str!("../../recipes/knight.json")).unwrap();
        let report = check(&r);
        assert_eq!(report.errors, 0, "{}", report.text());
    }

    #[test]
    fn floating_feet_are_an_error() {
        let r = Recipe::parse(
            r#"{ "kiln": 0, "name": "ball", "canvas": [8, 8], "pivot": [4, 8],
            "materials": { "m": { "ramp": "sxSuUvVW", "finish": "metal" } },
            "parts": [ { "id": "b", "shape": "ellipsoid", "at": [4, 3, 0], "r": [2, 2, 2], "mat": "m" } ] }"#,
        )
        .unwrap();
        let report = check(&r);
        assert!(report.lints.iter().any(|l| l.code == "floor" && l.level == Level::Error), "{}", report.text());
    }
}

//! The recipe as text: what a model reads to know what it drew without
//! looking. The canvas and light; the figure's box and inks at each grain;
//! each part's box, coverage and inks at grains 1 and 4, nearest first;
//! parts lost at grain 1; the sockets; the rig; and each clip, with how
//! many pixels change from frame to frame.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::grain::Grain;
use crate::line::Mark;
use crate::palette::{Ink, Palette};
use crate::recipe::Recipe;
use crate::render::{self, Frame};

/// A part as drawn at one grain.
#[derive(Clone, Debug, Default)]
pub struct Seen {
    /// Pixels it shows (its fill and the lines drawn on it).
    pub px: u32,
    /// Its box, `(x, y, w, h)`.
    pub bounds: Option<(u32, u32, u32, u32)>,
    /// Its inks, most used first.
    pub inks: Vec<(Ink, u32)>,
    /// Its mean depth where it shows.
    pub depth: f32,
}

/// What each part shows in a frame, by part index.
pub fn seen(frame: &Frame, parts: usize) -> Vec<Seen> {
    let r = &frame.raster;
    let mut out = vec![Seen::default(); parts];
    let mut boxes = vec![(u32::MAX, u32::MAX, 0u32, 0u32); parts];
    let mut inks: Vec<BTreeMap<Ink, u32>> = vec![BTreeMap::new(); parts];
    let mut depth = vec![0f64; parts];
    for y in 0..r.h {
        for x in 0..r.w {
            let i = (y * r.w + x) as usize;
            let p = r.part[i];
            if p == 0 || p as usize > parts || !matches!(frame.marks[i], Mark::Fill | Mark::Inner | Mark::Edge) {
                continue;
            }
            let k = p as usize - 1;
            out[k].px += 1;
            let b = &mut boxes[k];
            *b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
            *inks[k].entry(r.ink[i]).or_default() += 1;
            depth[k] += r.depth[i] as f64;
        }
    }
    for k in 0..parts {
        if out[k].px > 0 {
            let b = boxes[k];
            out[k].bounds = Some((b.0, b.1, b.2 - b.0 + 1, b.3 - b.1 + 1));
            out[k].depth = (depth[k] / out[k].px as f64) as f32;
            let mut v: Vec<(Ink, u32)> = inks[k].iter().map(|(&i, &n)| (i, n)).collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            out[k].inks = v;
        }
    }
    out
}

/// Inks a frame uses, most first.
pub fn inks(frame: &Frame) -> Vec<(Ink, u32)> {
    let mut m: BTreeMap<Ink, u32> = BTreeMap::new();
    for &i in &frame.raster.ink {
        if !i.is_clear() {
            *m.entry(i).or_default() += 1;
        }
    }
    let mut v: Vec<(Ink, u32)> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

/// Ink keys, most used first.
pub fn keys(inks: &[(Ink, u32)], most: usize) -> String {
    let p = Palette::realm();
    inks.iter().take(most).map(|(i, _)| p.key(*i)).collect()
}

fn boxed(b: Option<(u32, u32, u32, u32)>) -> String {
    b.map_or("-".into(), |(x, y, w, h)| format!("{x},{y} {w}x{h}"))
}

/// Pixels that differ between two frames of one size.
pub fn changed(a: &Frame, b: &Frame) -> u32 {
    a.raster.ink.iter().zip(&b.raster.ink).filter(|(x, y)| x != y).count() as u32
}

/// The recipe as text.
pub fn describe(recipe: &Recipe) -> String {
    let src = &recipe.src;
    let p = Palette::realm();
    let mut s = String::new();
    let [w, h] = src.canvas;
    let l = recipe.light;
    let _ = writeln!(
        s,
        "{}: canvas {w}x{h} game px (g1 {w}x{h}, g4 {}x{}), pivot {},{}, light {:.2},{:.2},{:.2}",
        src.name,
        w * 4,
        h * 4,
        src.pivot[0],
        src.pivot[1],
        l[0],
        l[1],
        l[2]
    );
    let mats: Vec<String> = recipe.materials.iter().map(|(n, r, f, _)| format!("{n} {} {}", r.keys(p), f.name())).collect();
    let _ = writeln!(s, "materials: {}", mats.join("; "));
    let still: Vec<Frame> = [1u8, 2, 4].map(|g| render::still(recipe, Grain(g))).into_iter().collect();
    for f in &still {
        let i = inks(f);
        let _ = writeln!(s, "g{}: figure {}, {} inks: {}", f.grain.0, boxed(f.raster.bounds()), i.len(), keys(&i, 32));
    }
    let n = src.parts.len();
    let g1 = seen(&still[0], n);
    let g4 = seen(&still[2], n);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| g4[b].depth.partial_cmp(&g4[a].depth).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
    let _ = writeln!(s, "parts at rest, nearest first (px and box at g1 and g4; mean depth and inks at g4):");
    for &k in &order {
        let part = &src.parts[k];
        let mut hang = part.bone.clone().map(|b| format!("bone {b}")).unwrap_or_default();
        if let Some(sk) = &part.socket {
            hang = format!("socket {sk}");
        }
        if let Some(on) = &part.on {
            hang = format!("{hang} on {on}");
        }
        let mut flags = Vec::new();
        if part.keep {
            flags.push("keep".to_string());
        }
        if part.hidden {
            flags.push("hidden".to_string());
        }
        if let Some(gs) = &part.grains {
            flags.push(format!("grains {}", gs.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(",")));
        }
        let mat = part.mat.clone().or_else(|| part.ink.clone().map(|i| format!("ink {i}"))).unwrap_or_default();
        let _ = writeln!(
            s,
            "  {:<15} {:<9} {:<8} {:<18} g1 {:>3}px {:<11} g4 {:>4}px {:<12} z {:>5.2} {}{}",
            part.id,
            part.shape.name(),
            mat,
            hang.trim(),
            g1[k].px,
            boxed(g1[k].bounds),
            g4[k].px,
            boxed(g4[k].bounds),
            g4[k].depth,
            keys(&g4[k].inks, 8),
            if flags.is_empty() { String::new() } else { format!("  [{}]", flags.join(" ")) }
        );
    }
    let lost: Vec<&str> = (0..n)
        .filter(|&k| g4[k].px > 0 && g1[k].px == 0 && src.parts[k].grains.as_ref().is_none_or(|g| g.contains(&1)))
        .map(|k| src.parts[k].id.as_str())
        .collect();
    let _ = writeln!(s, "lost at g1: {}", if lost.is_empty() { "none".into() } else { lost.join(", ") });
    let socks: Vec<String> = still[0]
        .sockets
        .keys()
        .map(|k| {
            let [x, y] = still[0].socket_px(k).unwrap_or([0, 0]);
            format!("{k} {x},{y}")
        })
        .collect();
    let _ = writeln!(s, "sockets at rest (g1 px): {}", if socks.is_empty() { "none".into() } else { socks.join(", ") });
    if !src.rig.bones.is_empty() {
        let bones: Vec<String> = src
            .rig
            .bones
            .iter()
            .map(|b| match &b.parent {
                Some(p) => format!("{}<{p}", b.id),
                None => b.id.clone(),
            })
            .collect();
        let _ = writeln!(s, "rig: {} bones: {}", bones.len(), bones.join(" "));
    }
    for (name, clip) in &src.clips {
        let frames = render::clip_frames(recipe, Grain::GAME, name, true);
        let moved: std::collections::BTreeSet<&str> = clip.keys.iter().flat_map(|k| k.pose.keys().map(String::as_str)).collect();
        let deltas: Vec<String> = (0..frames.len()).map(|i| changed(&frames[i], &frames[(i + 1) % frames.len()]).to_string()).collect();
        let (mut top, mut bottom) = (u32::MAX, 0);
        for f in &frames {
            if let Some((_, y, _, h)) = f.raster.bounds() {
                top = top.min(y);
                bottom = bottom.max(y + h - 1);
            }
        }
        let _ = writeln!(
            s,
            "clip {name}: {} frames at {} fps{}, keys at {}, moves {}; g1 rows {top}..={bottom}; g1 px changed to the next frame: {}{}",
            clip.frames,
            clip.fps,
            if clip.looped { " looping" } else { " once" },
            clip.keys.iter().map(|k| format!("{}", k.t)).collect::<Vec<_>>().join(","),
            moved.into_iter().collect::<Vec<_>>().join(","),
            deltas.join(" "),
            if clip.show.is_empty() { String::new() } else { format!("; shows {}", clip.show.join(",")) }
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_knight_describes_itself() {
        let r = Recipe::parse(include_str!("../../recipes/knight.json")).unwrap();
        let text = describe(&r);
        assert!(text.starts_with("knight: canvas 28x20"), "{text}");
        assert!(text.contains("helm") && text.contains("sockets at rest"), "{text}");
    }
}

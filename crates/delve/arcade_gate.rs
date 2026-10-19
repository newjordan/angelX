//! Build-time arcade floor, also exercised against deliberately bad art.
use image::RgbaImage;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
type Point = (i32, i32);
type Rgb = [u8; 3];
const EXTRA: [&str; 8] = [
    "38212d", "61303b", "853d43", "ad4e49", "cc6854", "dc8768", "a34828", "d77b39",
];
fn rgb(s: &str) -> Rgb {
    let n = u32::from_str_radix(s, 16).expect("RGB hex");
    [(n >> 16) as u8, (n >> 8) as u8, n as u8]
}
fn neighbours((x, y): Point) -> [Point; 4] {
    [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
}
fn groups(points: &HashSet<Point>) -> Vec<HashSet<Point>> {
    let mut left = points.clone();
    let mut result = Vec::new();
    while let Some(&p) = left.iter().next() {
        left.remove(&p);
        let mut group = HashSet::from([p]);
        let mut todo = vec![p];
        while let Some((x, y)) = todo.pop() {
            for (dx, dy) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let q = (x + dx, y + dy);
                if left.remove(&q) {
                    group.insert(q);
                    todo.push(q);
                }
            }
        }
        result.push(group);
    }
    result
}
pub fn image_gate(
    im: &RgbaImage,
    spec: &Value,
    palette: &HashSet<Rgb>,
    contours: &HashSet<Rgb>,
    ramps: &Value,
) -> Vec<String> {
    let mut errors = Vec::new();
    let mut pixels = HashMap::new();
    for (x, y, p) in im.enumerate_pixels() {
        if p[3] == 0 {
            continue;
        }
        if p[3] != 255 {
            errors.push("alpha: nonbinary sample".into());
        }
        let c = [p[0], p[1], p[2]];
        if !palette.contains(&c) {
            errors.push("palette: foreign pigment".into());
        }
        if *c.iter().min().unwrap() >= 198 && *c.iter().max().unwrap() >= 220 {
            errors.push("near-white: pale core".into());
        }
        pixels.insert((x as i32, y as i32), c);
    }
    if pixels.is_empty() {
        return vec!["empty frame".into()];
    }
    let solid: HashSet<_> = pixels.keys().copied().collect();
    for (&p, c) in &pixels {
        if neighbours(p).iter().any(|q| !solid.contains(q)) && !contours.contains(c) {
            errors.push(format!("contour: open at {},{}", p.0, p.1));
        }
    }
    // Only named straight 2–3px features exempt tiny pigment clusters.
    let mut tagged = HashSet::new();
    for f in spec["features"].as_array().into_iter().flatten() {
        let points: HashSet<Point> = f["pixels"]
            .as_array()
            .expect("feature pixels")
            .iter()
            .map(|p| (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32))
            .collect();
        let xs: HashSet<_> = points.iter().map(|p| p.0).collect();
        let ys: HashSet<_> = points.iter().map(|p| p.1).collect();
        if f["name"].as_str().unwrap_or("").is_empty()
            || !(2..=3).contains(&points.len())
            || groups(&points).len() != 1
            || (xs.len() != 1 && ys.len() != 1)
            || points.iter().any(|p| !solid.contains(p))
            || points
                .iter()
                .filter_map(|p| pixels.get(p))
                .collect::<HashSet<_>>()
                .len()
                != 1
        {
            errors.push("feature: only named 2–3px same-pigment connected linear details".into());
        } else {
            tagged.extend(points);
        }
    }
    for c in pixels.values().copied().collect::<HashSet<_>>() {
        let same: HashSet<_> = pixels
            .iter()
            .filter_map(|(p, v)| (*v == c).then_some(*p))
            .collect();
        for group in groups(&same) {
            if group.len() < 4 && !group.is_subset(&tagged) {
                errors.push(format!(
                    "fleck: {}px pigment at {:?}",
                    group.len(),
                    group.iter().min().unwrap()
                ));
            }
        }
    }
    for group in groups(&solid) {
        if group.len() < 4 {
            errors.push("fleck: detached alpha island".into());
        }
    }
    // Repeated AB/BA tiles fail. A single cel corner is not a texture.
    let checker = |x, y| {
        let a = pixels.get(&(x, y));
        let b = pixels.get(&(x + 1, y));
        a.is_some()
            && b.is_some()
            && a != b
            && a == pixels.get(&(x + 1, y + 1))
            && b == pixels.get(&(x, y + 1))
    };
    for y in 0..im.height() as i32 - 2 {
        for x in 0..im.width() as i32 - 2 {
            if checker(x, y) && (checker(x + 1, y) || checker(x, y + 1)) {
                errors.push("dither: repeated checker".into());
            }
        }
    }
    for (material, minimum) in spec["required_materials"].as_object().into_iter().flatten() {
        let ramp: HashSet<_> = ramps[material]
            .as_array()
            .expect("material ramp")
            .iter()
            .map(|v| rgb(v.as_str().unwrap()))
            .collect();
        let mut tones = HashMap::<Rgb, usize>::new();
        for c in pixels.values().filter(|c| ramp.contains(*c)) {
            *tones.entry(*c).or_default() += 1;
        }
        if tones
            .values()
            .filter(|&&n| n >= if spec["knight_rules"] == true { 1 } else { 4 })
            .count()
            < minimum.as_u64().unwrap() as usize
        {
            errors.push(format!("tones: {material} needs {minimum} actual tones"));
        }
    }
    for region in spec["material_regions"].as_array().into_iter().flatten() {
        let material = region["material"].as_str().expect("material region");
        let ramp: HashSet<_> = ramps[material]
            .as_array()
            .expect("region ramp")
            .iter()
            .map(|v| rgb(v.as_str().unwrap()))
            .collect();
        let points = region_points(region, &solid);
        let mut tones = HashMap::<Rgb, usize>::new();
        for p in points {
            if let Some(&c) = pixels.get(&p) {
                if ramp.contains(&c) {
                    *tones.entry(c).or_default() += 1;
                }
            }
        }
        if tones
            .values()
            .filter(|&&n| n >= if spec["knight_rules"] == true { 1 } else { 4 })
            .count()
            < region["minimum_tones"].as_u64().unwrap() as usize
        {
            errors.push(format!(
                "region: {} needs its five actual tones",
                region["name"]
            ));
        }
    }
    if spec["knight_rules"].as_bool() == Some(true) {
        errors.extend(knight_image_floor(im, spec));
    }
    errors.sort();
    errors.dedup();
    errors
}
fn luma(c: Rgb) -> i32 {
    (299 * i32::from(c[0]) + 587 * i32::from(c[1]) + 114 * i32::from(c[2])) / 1000
}
fn tagged_points(v: &Value) -> HashSet<Point> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p[0].as_i64()? as i32, p[1].as_i64()? as i32)))
        .collect()
}
fn region_points(region: &Value, solid: &HashSet<Point>) -> HashSet<Point> {
    if region["pixels"].is_array() {
        tagged_points(&region["pixels"])
    } else if let Some(b) = region["box"].as_array() {
        solid
            .iter()
            .copied()
            .filter(|&(x, y)| {
                x >= b[0].as_i64().unwrap() as i32
                    && y >= b[1].as_i64().unwrap() as i32
                    && x < b[2].as_i64().unwrap() as i32
                    && y < b[3].as_i64().unwrap() as i32
            })
            .collect()
    } else {
        HashSet::new()
    }
}
/// Drawing floors apply to native knight pixels, before grade or seat swapping.
pub fn knight_image_floor(im: &RgbaImage, spec: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let pixels: HashMap<Point, Rgb> = im
        .enumerate_pixels()
        .filter(|(_, _, p)| p[3] > 0)
        .map(|(x, y, p)| ((x as i32, y as i32), [p[0], p[1], p[2]]))
        .collect();
    if pixels.is_empty() {
        return vec!["knight: empty".into()];
    }
    let solid: HashSet<_> = pixels.keys().copied().collect();
    let contour = HashSet::from([rgb("22303a"), rgb("322b2b")]);
    let near = rgb("0a0c12");
    let ink = pixels
        .values()
        .filter(|c| contour.contains(*c) || **c == near)
        .count();
    if ink * 100 > pixels.len() * 25 {
        errors.push("contour-fill: contour plus near-black exceeds 25%".into());
    }
    let visor = tagged_points(&spec["parts"]["visor"]);
    for (&(x, y), &c) in &pixels {
        if c == near && !visor.contains(&(x, y)) {
            errors.push("interior-dark: near-black outside visor slit".into());
        }
        if !contour.contains(&c) && c != near && luma(c) < luma(rgb("22303a")) + 12 {
            errors.push("interior-dark: less than 12 luma above contour".into());
        }
        if contour.contains(&c)
            && (-1..=1).all(|dy| {
                (-1..=1).all(|dx| {
                    pixels
                        .get(&(x + dx, y + dy))
                        .is_some_and(|c| contour.contains(c))
                })
            })
        {
            errors.push("contour-fill: solid 3x3 ink block".into());
        }
    }
    let mut exceptions = HashSet::new();
    for f in spec["silhouette_features"].as_array().into_iter().flatten() {
        let points = tagged_points(&f["pixels"]);
        let name = f["name"].as_str().unwrap_or("");
        // Tips must be named, drawn, short and exposed. A whole ragged edge
        // cannot be tagged away as a feature.
        if !["sword tip", "plume tip", "shield point", "boot joint"].contains(&name)
            || points.is_empty()
            || points.len() > 3
            || groups(&points).len() != 1
            || points
                .iter()
                .any(|p| neighbours(*p).iter().all(|q| solid.contains(q)))
        {
            errors.push("silhouette-tag: invalid feature".into());
        } else {
            exceptions.extend(points);
        }
    }
    for y in 0..im.height() as i32 {
        for x in 0..im.width() as i32 {
            let p = (x, y);
            let count = neighbours(p).iter().filter(|q| solid.contains(q)).count();
            if solid.contains(&p) && count <= 1 && !exceptions.contains(&p) {
                errors.push(format!("spur: untagged 1px at {x},{y}"));
            }
            if !solid.contains(&p) && count >= 3 && !exceptions.contains(&p) {
                errors.push(format!("notch: untagged 1px at {x},{y}"));
            }
        }
    }
    for region in spec["material_regions"].as_array().into_iter().flatten() {
        let material = region["material"].as_str().unwrap_or("");
        let ramp: Vec<_> = match material {
            "steel" => ["2f4a52", "456070", "6e8a8e", "8fa3a8", "b3c2c4"].to_vec(),
            "cloth" => ["61303b", "853d43", "ad4e49", "cc6854", "dc8768"].to_vec(),
            _ => continue,
        }
        .into_iter()
        .map(rgb)
        .collect();
        let mut counts = HashMap::<Rgb, usize>::new();
        for p in region_points(region, &solid) {
            if let Some(c) = pixels.get(&p) {
                if ramp.contains(c) {
                    *counts.entry(*c).or_default() += 1;
                }
            }
        }
        let mass: usize = counts.values().sum();
        if mass == 0
            || counts.values().filter(|&&n| n * 100 >= mass * 10).count() < 3
            || counts.values().any(|&n| n * 100 > mass * 35)
        {
            errors.push(format!(
                "tone-bands: {} needs three >=10% bands, none >35%",
                region["name"]
            ));
        }
        if material == "cloth" {
            if counts.get(&rgb("cc6854")).copied().unwrap_or(0) * 100 > mass * 10
                || counts.get(&rgb("dc8768")).copied().unwrap_or(0) * 100 > mass * 3
            {
                errors.push("cloth-light: rim >10% or glint >3%".into());
            }
        }
    }
    // A fold can turn; a repeated complementary 4x4 cloth tile is a weave
    // texture. This catches both the old diamonds and two-pixel checkers.
    let cloth: HashSet<_> = ["61303b", "853d43", "ad4e49", "cc6854", "dc8768"]
        .into_iter()
        .map(rgb)
        .collect();
    for y in 0..im.height() as i32 - 3 {
        for x in 0..im.width() as i32 - 3 {
            let rows: Vec<Vec<_>> = (0..4)
                .map(|dy| {
                    (0..4)
                        .filter_map(|dx| pixels.get(&(x + dx, y + dy)).copied())
                        .collect()
                })
                .collect();
            if rows
                .iter()
                .any(|r| r.len() != 4 || r.iter().any(|c| !cloth.contains(c)))
            {
                continue;
            }
            let kinds: HashSet<_> = rows.iter().collect();
            let inks: HashSet<_> = rows.iter().flatten().collect();
            if kinds.len() == 2 && inks.len() == 2 {
                let a = rows[0].clone();
                let b = rows.iter().find(|r| **r != a).unwrap();
                if rows.iter().filter(|r| **r == a).count() == 2
                    && a.iter().zip(b).all(|(a, b)| a != b)
                    && a.windows(2).any(|p| p[0] != p[1])
                {
                    errors.push("cloth-dither: repeated complementary cloth tile".into());
                }
            }
        }
    }
    let bright = |inks: &[Rgb]| {
        let points: HashSet<_> = pixels
            .iter()
            .filter_map(|(p, c)| inks.contains(c).then_some(*p))
            .collect();
        groups(&points).iter().map(HashSet::len).max().unwrap_or(0)
    };
    if bright(&[rgb("cc6854"), rgb("dc8768")]) >= bright(&[rgb("8fa3a8"), rgb("b3c2c4")]) {
        errors.push("bright-area: cloth dominates the steel highlight".into());
    }
    errors.sort();
    errors.dedup();
    errors
}
/// Contrast is checked after every seat mapping, before the unchanged grade.
pub fn seat_floor(im: &RgbaImage, bank: &Value) -> Vec<String> {
    let cloth: Vec<_> = bank["ramps"]["cloth"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| rgb(v.as_str().unwrap()))
        .collect();
    let mut errors = Vec::new();
    for (seat, ramp) in bank["seats"].as_array().unwrap().iter().enumerate() {
        for p in im.pixels().filter(|p| p[3] == 255) {
            let c = [p[0], p[1], p[2]];
            let mapped = cloth
                .iter()
                .position(|v| *v == c)
                .map(|i| rgb(ramp[i].as_str().unwrap()))
                .unwrap_or(c);
            if ![rgb("22303a"), rgb("322b2b"), rgb("0a0c12")].contains(&mapped)
                && luma(mapped) < luma(rgb("22303a")) + 12
            {
                errors.push(format!(
                    "seat-dark: seat {} interior is less than 12 luma above contour",
                    seat + 1
                ));
                break;
            }
        }
    }
    // L is the existing integer RGB luma used throughout this drawing
    // contract. Compare visible material means, never contour or empty space.
    let steel: HashSet<_> = bank["ramps"]["steel"]
        .as_array()
        .unwrap()
        .iter()
        .skip(1)
        .map(|v| rgb(v.as_str().unwrap()))
        .collect();
    let steel_samples: Vec<_> = im
        .pixels()
        .filter(|p| p[3] == 255 && steel.contains(&[p[0], p[1], p[2]]))
        .map(|p| luma([p[0], p[1], p[2]]))
        .collect();
    let mean = |v: &[i32]| -> i32 { v.iter().sum::<i32>() / v.len().max(1) as i32 };
    let hue = |c: Rgb| -> f64 {
        let r = f64::from(c[0]);
        let g = f64::from(c[1]);
        let b = f64::from(c[2]);
        let hi = r.max(g).max(b);
        let lo = r.min(g).min(b);
        let d = hi - lo;
        if d == 0.0 {
            return 0.0;
        }
        let h = if hi == r {
            (g - b) / d
        } else if hi == g {
            2.0 + (b - r) / d
        } else {
            4.0 + (r - g) / d
        };
        (h * 60.0).rem_euclid(360.0)
    };
    for (seat, ramp) in bank["seats"].as_array().unwrap().iter().enumerate() {
        let mapped: Vec<_> = im
            .pixels()
            .filter(|p| p[3] == 255)
            .filter_map(|p| cloth.iter().position(|c| *c == [p[0], p[1], p[2]]))
            .map(|i| rgb(ramp[i].as_str().unwrap()))
            .collect();
        let values: Vec<_> = mapped.iter().copied().map(luma).collect();
        if (mean(&steel_samples) - mean(&values)).abs() < 15 {
            errors.push(format!(
                "seat-contrast: seat {} coat/steel delta-L <15",
                seat + 1
            ));
        }
        // Steel hands are neutral; brass is the chromatic hand trim. Require
        // a real hue interval from that trim, not a same-orange value shift.
        let avg: Rgb = std::array::from_fn(|j| {
            (mapped.iter().map(|c| u32::from(c[j])).sum::<u32>() / mapped.len().max(1) as u32) as u8
        });
        let gap = (hue(avg) - hue(rgb("be914b"))).abs();
        if gap.min(360.0 - gap) < 25.0 {
            errors.push(format!(
                "seat-hue: seat {} coat/trim hue gap <25 degrees",
                seat + 1
            ));
        }
    }
    errors
}
/// Apply the same contrast/hue floor to the actual authored coat mask, so
/// plume/shield samples cannot conceal a coat that merges with the steel.
pub fn seat_coat_floor(im: &RgbaImage, spec: &Value, bank: &Value) -> Vec<String> {
    let solid: HashSet<_> = im
        .enumerate_pixels()
        .filter_map(|(x, y, p)| (p[3] == 255).then_some((x as i32, y as i32)))
        .collect();
    let coat: HashSet<_> = spec["material_regions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["material"] == "cloth")
        .flat_map(|r| region_points(r, &solid))
        .collect();
    let cloth: HashSet<_> = bank["ramps"]["cloth"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| rgb(v.as_str().unwrap()))
        .collect();
    if !coat.iter().any(|&(x, y)| {
        let p = im.get_pixel(x as u32, y as u32);
        p[3] == 255 && cloth.contains(&[p[0], p[1], p[2]])
    }) {
        return vec!["seat-contrast: missing actual cloth region".into()];
    }
    let mut masked = im.clone();
    for (x, y, p) in masked.enumerate_pixels_mut() {
        if cloth.contains(&[p[0], p[1], p[2]]) && !coat.contains(&(x as i32, y as i32)) {
            p[3] = 0;
        }
    }
    seat_floor(&masked, bank)
        .into_iter()
        .map(|e| format!("{e} (actual coat mask)"))
        .collect()
}
pub fn fx_image_floor(im: &RgbaImage, state: &str, index: usize) -> Vec<String> {
    if state != "explosion" || !(2..=4).contains(&index) {
        return Vec::new();
    }
    let hot: HashSet<_> = im
        .enumerate_pixels()
        .filter_map(|(x, y, p)| {
            (p[3] == 255 && [rgb("e2cb8b"), rgb("d8a95e")].contains(&[p[0], p[1], p[2]]))
                .then_some((x as i32, y as i32))
        })
        .collect();
    let mut errors = Vec::new();
    if groups(&hot).iter().map(HashSet::len).max().unwrap_or(0) < 40 {
        errors.push("fx-core: peak needs a connected >=40px honey core".into());
    }
    for ink in ["652c10", "a34828", "d77b39", "d8a95e", "e2cb8b"] {
        if im
            .pixels()
            .filter(|p| p[3] == 255 && [p[0], p[1], p[2]] == rgb(ink))
            .count()
            < 4
        {
            errors.push("fx-ramp: peak needs five fire tones >=4px each".into());
        }
    }
    errors
}
fn bounds(points: &HashSet<Point>) -> Option<(i32, i32, i32, i32)> {
    Some((
        points.iter().map(|p| p.0).min()?,
        points.iter().map(|p| p.1).min()?,
        points.iter().map(|p| p.0).max()? + 1,
        points.iter().map(|p| p.1).max()? + 1,
    ))
}
pub fn knight_consistency(manifest: &Value, images: &HashMap<String, RgbaImage>) -> Vec<String> {
    let mut errors = Vec::new();
    let mut areas = Vec::new();
    let mut run_y = Vec::new();
    for (state, anim) in manifest["states"].as_object().unwrap() {
        for (i, spec) in anim["frames"].as_array().unwrap().iter().enumerate() {
            let file = spec["file"].as_str().unwrap();
            let im = &images[file];
            let at = |p: Point| -> Option<Rgb> {
                if p.0 < 0 || p.1 < 0 || p.0 >= im.width() as i32 || p.1 >= im.height() as i32 {
                    return None;
                }
                let c = im.get_pixel(p.0 as u32, p.1 as u32);
                (c[3] == 255).then_some([c[0], c[1], c[2]])
            };
            let helmet = tagged_points(&spec["parts"]["helmet"]);
            let eye = tagged_points(&spec["parts"]["eye"]);
            if eye.len() != 6
                || eye
                    .iter()
                    .filter(|&&p| at(p) == Some(rgb("0a0c12")))
                    .count()
                    < 6
                || tagged_points(&spec["parts"]["face"])
                    .iter()
                    .filter(|&&p| {
                        at(p).is_some_and(|c| {
                            [rgb("684629"), rgb("a97639"), rgb("be914b")].contains(&c)
                        })
                    })
                    .count()
                    < 12
            {
                errors.push(format!("face: missing open eye/jaw in {file}"));
            }
            let moustache = tagged_points(&spec["parts"]["moustache"]);
            if moustache
                .iter()
                .filter(|&&p| at(p) == Some(rgb("22303a")))
                .count()
                < 4
            {
                errors.push(format!("face: missing dark moustache in {file}"));
            }
            if state == "defeat" {
                continue;
            }
            let Some((x, y, r, b)) = bounds(&helmet) else {
                errors.push(format!("helmet: missing {file}"));
                continue;
            };
            if helmet.iter().any(|&p| at(p).is_none()) {
                errors.push(format!("helmet: invalid mask {file}"));
            }
            let acting = spec["acting"].as_array().is_some_and(|tags| {
                tags.iter()
                    .any(|t| matches!(t.as_str(), Some("squash" | "stretch" | "lean")))
            });
            if !acting {
                areas.push((r - x) * (b - y));
                if state == "run" {
                    run_y.push(y);
                }
            }
            if state == "run" {
                let hip_x = spec["landmarks"]["hip"][0].as_i64();
                let hip_y = spec["landmarks"]["hip"][1].as_i64();
                let head_x2 = x + r - 1;
                if hip_x.zip(hip_y).is_none_or(|(hip, hy)| {
                    head_x2 - (hip as i32) * 2 < 6 || at((hip as i32, hy as i32)).is_none()
                }) {
                    errors.push(format!("run-lean: head must lead hips >=3px in {file}"));
                }
            }
            let glints = helmet
                .difference(&eye)
                .filter(|&&p| at(p) == Some(rgb("b3c2c4")))
                .count();
            if glints < 4 {
                errors.push(format!("glint: helmet missing {file}"));
            }
            for (part, min) in [("sword", 40), ("shield", 120)] {
                let points = tagged_points(&spec["parts"][part]);
                if points.len() < min || points.iter().any(|&p| at(p).is_none()) {
                    errors.push(format!("prop: missing {part} in {file}"));
                }
                if part == "sword" {
                    if !points.iter().any(|&p| at(p) == Some(rgb("b3c2c4"))) {
                        errors.push(format!("glint: blade missing {file}"));
                    }
                    if state == "victory" && i == 2 {
                        let plume = tagged_points(&spec["parts"]["plume"]);
                        if bounds(&points).is_none_or(|s| s.1 > y - 4)
                            || bounds(&plume)
                                .is_none_or(|p| bounds(&points).is_none_or(|s| s.1 > p.1 - 10))
                        {
                            errors.push("victory: sword tip must rise >=10px above plume".into());
                        }
                    }
                }
            }
        }
    }
    let poses = [
        ("idle", 0),
        ("guard", 0),
        ("hit", 0),
        ("attack", 2),
        ("victory", 2),
    ];
    for (a, &(state, index)) in poses.iter().enumerate() {
        let spec = &manifest["states"][state]["frames"][index];
        let Some(im) = spec["file"].as_str().and_then(|f| images.get(f)) else {
            continue;
        };
        let solid = |im: &RgbaImage| -> HashSet<Point> {
            im.enumerate_pixels()
                .filter_map(|(x, y, p)| (p[3] == 255).then_some((x as i32, y as i32)))
                .collect()
        };
        let left = solid(im);
        for &(other, j) in &poses[a + 1..] {
            let spec = &manifest["states"][other]["frames"][j];
            let Some(im) = spec["file"].as_str().and_then(|f| images.get(f)) else {
                continue;
            };
            let right = solid(im);
            let intersection = left.intersection(&right).count();
            let union = left.union(&right).count();
            if intersection * 100 > union * 70 {
                errors.push(format!(
                    "pose-diversity: {state}-{index}/{other}-{j} IoU exceeds 0.70"
                ));
            }
        }
    }
    if let (Some(lo), Some(hi)) = (areas.iter().min(), areas.iter().max()) {
        if hi * 100 > lo * 110 {
            errors.push("helmet: bounding-box area drifts >10%".into());
        }
    }
    if run_y.iter().max().unwrap_or(&0) - run_y.iter().min().unwrap_or(&0) > 2 {
        errors.push("head-bob: run moves more than 2px".into());
    }
    errors
}

pub fn frame_diff(a: &RgbaImage, b: &RgbaImage) -> bool {
    let changed = a.pixels().zip(b.pixels()).filter(|(a, b)| a != b).count();
    let mass = a
        .pixels()
        .filter(|p| p[3] > 0)
        .count()
        .max(b.pixels().filter(|p| p[3] > 0).count());
    changed >= 4 && changed * 100 >= mass * 2
}
pub fn animation_floor(manifest: &Value, kit: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let minimums: &[(&str, usize)] = if kit == "knight" {
        &[
            ("idle", 4),
            ("run", 6),
            ("jump", 2),
            ("fall", 1),
            ("land", 2),
            ("attack", 6),
            ("hit", 2),
            ("guard", 2),
            ("defeat", 4),
            ("victory", 3),
        ]
    } else {
        &[
            ("explosion", 10),
            ("muzzle", 3),
            ("impact", 4),
            ("dust", 5),
            ("spark", 3),
        ]
    };

    for &(name, n) in minimums {
        if manifest["states"][name]["frames"]
            .as_array()
            .map_or(0, Vec::len)
            < n
        {
            failures.push(format!("{kit}/{name}: frame minimum {n}"));
        }
    }
    failures
}

pub fn validate(root: &Path) -> Result<(), String> {
    let read = |p: &Path| {
        serde_json::from_slice::<Value>(&std::fs::read(p).expect("arcade manifest"))
            .expect("valid arcade JSON")
    };
    let bank = read(&root.join("palette.json"));
    if bank["extension"] != serde_json::json!(EXTRA) {
        return Err("arcade bank must equal the eight approved local pigments".into());
    }
    if bank["ramps"]["bandage"] != serde_json::json!(["7a7c72", "94968c", "b0b2a8"]) {
        return Err("bandage must use the three pinned shared structure inks".into());
    }
    let source = std::fs::read_to_string(root.join("../../src/render/overworld/ink.rs"))
        .expect("shared ink source");
    let table = source
        .split("pub const PALETTE:")
        .nth(1)
        .unwrap()
        .split("= [")
        .nth(1)
        .unwrap()
        .split("];")
        .next()
        .unwrap();
    let mut palette = HashSet::new();
    let mut entries = 0;
    for token in table.split(|c: char| !c.is_ascii_hexdigit() && c != 'x') {
        if let Some(s) = token.strip_prefix("0x") {
            if entries < 60 {
                palette.insert(rgb(s));
            }
            entries += 1;
        }
    }
    let contours: HashSet<_> = bank["contours"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| rgb(v.as_str().unwrap()))
        .collect();
    let mut local = contours.clone();
    local.insert(rgb("0a0c12"));
    for ramp in bank["ramps"]
        .as_object()
        .unwrap()
        .values()
        .chain(bank["seats"].as_array().unwrap())
    {
        for c in ramp.as_array().unwrap() {
            local.insert(rgb(c.as_str().unwrap()));
        }
    }
    local.insert(rgb("be914b"));
    palette.extend(EXTRA.map(rgb));
    if !local.is_subset(&palette) {
        return Err("arcade ramp escapes shared palette + exact local bank".into());
    }
    let mut failures = Vec::new();
    for kit in ["knight", "fx"] {
        let manifest = read(&root.join(kit).join("manifest.json"));
        let (canvas, anchor) = if kit == "knight" {
            (serde_json::json!([64, 56]), serde_json::json!([30, 51]))
        } else {
            (serde_json::json!([64, 64]), serde_json::json!([32, 54]))
        };
        if manifest["canvas"] != canvas || manifest["anchor"] != anchor {
            failures.push(format!("{kit}: native canvas/anchor changed"));
        }
        failures.extend(animation_floor(&manifest, kit));
        let mut images = HashMap::new();
        for (state, animation) in manifest["states"].as_object().unwrap() {
            let frames = animation["frames"].as_array().unwrap();
            let times = animation["durations_ms"].as_array().unwrap();
            if frames.len() != times.len() || times.iter().any(|t| t.as_u64().unwrap_or(0) == 0) {
                failures.push(format!("{state}: frame timing"));
            }
            let mut previous = None;
            for (index, frame) in frames.iter().enumerate() {
                let path = root.join(kit).join(frame["file"].as_str().unwrap());
                println!("cargo:rerun-if-changed={}", path.display());
                let im = image::open(&path).expect("arcade PNG").to_rgba8();
                if im.width() != manifest["canvas"][0].as_u64().unwrap() as u32
                    || im.height() != manifest["canvas"][1].as_u64().unwrap() as u32
                {
                    failures.push(format!("{}: native canvas", path.display()));
                }
                if kit == "knight"
                    && (frame["knight_rules"] != serde_json::json!(true)
                        || frame["required_materials"] != serde_json::json!({"steel":5,"cloth":5})
                        || frame["material_regions"].as_array().map_or(0, Vec::len) != 2)
                {
                    failures.push("knight must retain its two five-tone material regions".into());
                }
                if kit == "knight" {
                    failures.extend(seat_floor(&im, &bank));
                    failures.extend(seat_coat_floor(&im, frame, &bank));
                }
                if kit == "fx" {
                    failures.extend(fx_image_floor(&im, state, index));
                }
                for e in image_gate(&im, frame, &local, &contours, &bank["ramps"]) {
                    failures.push(format!("{kit}/{}: {e}", frame["file"]));
                }
                if previous.as_ref().is_some_and(|p| !frame_diff(p, &im)) {
                    failures.push(format!("{kit}/{state}: static animation"));
                }
                images.insert(frame["file"].as_str().unwrap().to_owned(), im.clone());
                previous = Some(im);
            }
        }
        if kit == "knight" {
            failures.extend(knight_consistency(&manifest, &images));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
pub fn cell_floor(proof: &Value) -> Result<(), String> {
    if proof["stage"] != serde_json::json!([384, 224])
        || proof["cells"] != serde_json::json!([96, 28])
    {
        return Err("real stage/cell dimensions required".into());
    }
    let frames = proof["frames"].as_array().ok_or("missing cell frames")?;
    let mut seen = HashSet::new();
    for f in frames {
        let file = f["file"].as_str().ok_or("cell file")?;
        let seat = f["seat"].as_u64().ok_or("cell seat")?;
        let need = if file.starts_with("knight/") { 8 } else { 1 };
        if f["presence_percent"].as_u64().unwrap_or(0) < 55
            || f["lit_cells"].as_u64().unwrap_or(0) < need
        {
            return Err(format!("unreadable cell evidence: {file}/seat{seat}"));
        }
        if file.starts_with("knight/")
            && ["eye_dark_pixels", "moustache_dark_pixels"]
                .iter()
                .any(|k| f[k].as_u64().unwrap_or(0) < 2)
        {
            return Err(format!("lost face mark in cells: {file}/seat{seat}"));
        }
        if !seen.insert((file, seat)) || f["cells_hash"].as_u64().unwrap_or(0) == 0 {
            return Err("duplicate/empty buffer evidence".into());
        }
    }
    Ok(())
}
pub fn validate_cells(root: &Path) -> Result<(), String> {
    let read = |p: &Path| -> Result<Value, String> {
        serde_json::from_slice(&std::fs::read(p).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    };
    let proof = read(&root.join("cell-proof.json"))?;
    cell_floor(&proof)?;
    let mut expected = HashSet::new();
    for kit in ["knight", "fx"] {
        let manifest = read(&root.join(kit).join("manifest.json"))?;
        for animation in manifest["states"].as_object().unwrap().values() {
            for frame in animation["frames"].as_array().unwrap() {
                let name = format!("{kit}/{}", frame["file"].as_str().unwrap());
                for seat in 0..if kit == "knight" { 4 } else { 1 } {
                    expected.insert((name.clone(), seat));
                }
            }
        }
    }
    for frame in proof["frames"].as_array().unwrap() {
        let file = frame["file"].as_str().unwrap();
        let seat = frame["seat"].as_u64().unwrap();
        if !expected.remove(&(file.to_owned(), seat)) {
            return Err(format!("unexpected buffer: {file}/{seat}"));
        }
        if fingerprint(&std::fs::read(root.join(file)).map_err(|e| e.to_string())?)
            != frame["hash"].as_u64().unwrap()
        {
            return Err(format!("stale buffer: {file}"));
        }
    }
    for kit in ["knight", "fx"] {
        let m = read(&root.join(kit).join("manifest.json"))?;
        for animation in m["states"].as_object().unwrap().values() {
            for seat in 0..if kit == "knight" { 4 } else { 1 } {
                let mut previous = None;
                for frame in animation["frames"].as_array().unwrap() {
                    let name = format!("{kit}/{}", frame["file"].as_str().unwrap());
                    let evidence = proof["frames"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|f| {
                            f["file"].as_str() == Some(&name) && f["seat"].as_u64() == Some(seat)
                        })
                        .ok_or("missing cell drawing")?;
                    let current = evidence["cells_hash"].as_u64().unwrap();
                    if previous == Some(current) {
                        return Err(format!("static real-cell animation: {name}/seat{seat}"));
                    }
                    previous = Some(current);
                }
            }
        }
    }
    if !expected.is_empty() {
        return Err("missing frame/seat cell evidence".into());
    }
    let repo = root.join("../../../..");
    let mut sources: HashSet<&str> = HashSet::from([
        "crates/delve/src/render/arcade.rs",
        "crates/delve/src/render/art.rs",
        "crates/delve/src/render/braille.rs",
        "crates/delve/src/render/overworld/ink.rs",
        "crates/delve/src/render/overworld/arena.rs",
        "crates/delve/src/render/overworld/arena/boss_art.rs",
        "cockpit/src/stage/world_viz/art.rs",
        "cockpit/src/stage/world_viz/art/cells.rs",
        "tests/cockpit/world_viz/arcade__tests.rs",
        "crates/delve/assets/arcade/knight/manifest.json",
        "crates/delve/assets/arcade/fx/manifest.json",
        "crates/delve/assets/arcade/palette.json",
    ]);
    for source in proof["sources"]
        .as_array()
        .ok_or("missing renderer evidence")?
    {
        let file = source["file"].as_str().unwrap();
        if !sources.remove(file) {
            return Err("unexpected renderer evidence".into());
        }
        println!("cargo:rerun-if-changed={}", repo.join(file).display());
        if fingerprint(&std::fs::read(repo.join(file)).map_err(|e| e.to_string())?)
            != source["hash"].as_u64().unwrap()
        {
            return Err(format!("stale renderer evidence: {file}"));
        }
    }
    if !sources.is_empty() {
        return Err("missing renderer fingerprint".into());
    }
    Ok(())
}

//! Silhouette-aware fit for the quick rig (port of `spriterig.smartfit`).
//!
//! `fit` (the proportional cut) slices a sprite at fixed fractions of its
//! height: that cuts big heads through the mouth, splits robes into two
//! scissoring legs and mis-cuts wide arms. This fit reads the drawing
//! instead — the dark outline the sprite already carries is the separator
//! between parts.
//!
//!   neck   the lowest strong material change under the head
//!   hips   the highest row from which a gap runs down between two legs;
//!          no gap at all means a robe (one lower mass, plan "robe")
//!   arms   interior masses beside the torso that the outline separates
//!   outer  masses further out are worn on the back: wings, a cape
//!
//! Returns the same rig schema as the proportional fit, so `build` is
//! unchanged. A sprite with no readable outline falls back to it.

use std::collections::BTreeMap;

use serde_json::json;

use super::auto::{contact_mask, defaults, median, np_clip, py_round, Fitted, PartDef, LAYOUT};

/// Outline inks (`OUTLINE`).
const OUTLINE: &str = "kKZXdnfs";
/// Material families (`FAMILIES`): ink -> family index.
fn family(c: char) -> Option<usize> {
    const FAMILIES: [&str; 10] = [
        "kKZXgjGJhiH",
        "fDFEelNmLAMyCY",
        "nbIBpPrRoOtT",
        "sxSuUvVW",
        "qQz0123w",
        "a4@56",
        "$c9",
        "87",
        "d()[]^",
        "!?;%+",
    ];
    FAMILIES.iter().position(|f| f.contains(c))
}

type Grid = Vec<Vec<char>>;

fn is_ink(c: char) -> bool {
    c != '.'
}

/// 4-connected components (`_components`): labels 1..=n, 0 = none.
fn components(mask: &[Vec<bool>]) -> (Vec<Vec<i64>>, usize) {
    let h = mask.len();
    let w = mask.first().map(|r| r.len()).unwrap_or(0);
    let mut lab = vec![vec![0i64; w]; h];
    let mut n = 0usize;
    for y in 0..h {
        for x in 0..w {
            if mask[y][x] && lab[y][x] == 0 {
                n += 1;
                let mut stack = vec![(y, x)];
                lab[y][x] = n as i64;
                while let Some((cy, cx)) = stack.pop() {
                    for (ny, nx) in [(cy as i64 - 1, cx as i64), (cy as i64 + 1, cx as i64), (cy as i64, cx as i64 - 1), (cy as i64, cx as i64 + 1)] {
                        if ny >= 0 && ny < h as i64 && nx >= 0 && nx < w as i64 && mask[ny as usize][nx as usize] && lab[ny as usize][nx as usize] == 0 {
                            lab[ny as usize][nx as usize] = n as i64;
                            stack.push((ny as usize, nx as usize));
                        }
                    }
                }
            }
        }
    }
    (lab, n)
}

/// Give every unlabelled pixel of the mask the label of a nearest labelled
/// neighbour (`_spread`); unreachable pixels become torso.
fn spread(mut labels: Vec<Vec<&'static str>>, mask: &[Vec<bool>]) -> Vec<Vec<&'static str>> {
    let h = mask.len();
    let w = mask.first().map(|r| r.len()).unwrap_or(0);
    let unlabelled = |labels: &Vec<Vec<&'static str>>| -> Vec<Vec<bool>> {
        (0..h)
            .map(|y| (0..w).map(|x| mask[y][x] && labels[y][x] == "").collect())
            .collect()
    };
    let mut todo = unlabelled(&labels);
    loop {
        if !todo.iter().any(|r| r.iter().any(|&b| b)) {
            break;
        }
        let mut grown = labels.clone();
        let mut any = false;
        for y in 0..h {
            for x in 0..w {
                if !todo[y][x] {
                    continue;
                }
                for (ny, nx) in [(y as i64, x as i64 - 1), (y as i64, x as i64 + 1), (y as i64 - 1, x as i64), (y as i64 + 1, x as i64), (y as i64 - 1, x as i64 - 1), (y as i64 - 1, x as i64 + 1), (y as i64 + 1, x as i64 - 1), (y as i64 + 1, x as i64 + 1)] {
                    if ny >= 0 && ny < h as i64 && nx >= 0 && nx < w as i64 && labels[ny as usize][nx as usize] != "" {
                        grown[y][x] = labels[ny as usize][nx as usize];
                        any = true;
                        break;
                    }
                }
            }
        }
        if !any {
            for y in 0..h {
                for x in 0..w {
                    if todo[y][x] {
                        labels[y][x] = "torso";
                    }
                }
            }
            break;
        }
        labels = grown;
        todo = unlabelled(&labels);
    }
    labels
}

/// The material-families vector of one row's interior pixels.
fn families(ink: &[Vec<char>], inner: &[Vec<bool>], y: usize, c0: usize, c1: usize) -> Option<Vec<f64>> {
    let mut out = vec![0.0f64; 10];
    let mut sum = 0.0;
    for x in c0..c1 {
        if inner[y][x] && is_ink(ink[y][x]) {
            if let Some(f) = family(ink[y][x]) {
                out[f] += 1.0;
                sum += 1.0;
            }
        }
    }
    if sum > 0.0 {
        Some(out.iter().map(|v| v / sum).collect())
    } else {
        None
    }
}

/// The plan of a fitted silhouette: `"biped"` or `"robe"`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plan {
    Biped,
    Robe,
}

impl Plan {
    pub fn as_str(self) -> &'static str {
        match self {
            Plan::Biped => "biped",
            Plan::Robe => "robe",
        }
    }
}

/// A silhouette fit: the base rig fields plus the reading plan.
pub struct SmartFitted {
    pub fit: Fitted,
    pub plan: Plan,
}

/// `smartfit.fit`: silhouette fit with the proportional fit as the base.
/// `None` means the Python would return the base rig unchanged (no readable
/// outline / no neck), which the caller turns into the proportional fit.
fn smart_fit(ink: &[Vec<char>], base: &Fitted) -> Option<SmartFitted> {
    let h = ink.len();
    let w = ink.first().map(|r| r.len()).unwrap_or(0);
    let m: Vec<Vec<bool>> = (0..h).map(|y| (0..w).map(|x| is_ink(ink[y][x])).collect()).collect();
    let dark: Vec<Vec<bool>> = (0..h)
        .map(|y| (0..w).map(|x| OUTLINE.contains(ink[y][x]) && m[y][x]).collect())
        .collect();
    // border = m & ~(shifted neighbours all inside)
    let mut border = vec![vec![false; w]; h];
    let mut border_sum = 0usize;
    let mut dark_border = 0usize;
    for y in 0..h {
        for x in 0..w {
            let inside = |ny: i64, nx: i64| ny >= 0 && ny < h as i64 && nx >= 0 && nx < w as i64 && m[ny as usize][nx as usize];
            if m[y][x] && !(inside(y as i64 - 1, x as i64) && inside(y as i64 + 1, x as i64) && inside(y as i64, x as i64 - 1) && inside(y as i64, x as i64 + 1)) {
                border[y][x] = true;
                border_sum += 1;
                if dark[y][x] {
                    dark_border += 1;
                }
            }
        }
    }
    if border_sum == 0 || (dark_border as f64) < 0.7 * border_sum as f64 {
        return None;
    }
    let inner: Vec<Vec<bool>> = (0..h).map(|y| (0..w).map(|x| m[y][x] && !dark[y][x]).collect()).collect();
    let mut yy: Vec<usize> = Vec::new();
    let mut xx: Vec<f64> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if m[y][x] {
                yy.push(y);
                xx.push(x as f64);
            }
        }
    }
    let y0 = *yy.first()? as i64;
    let y1 = *yy.last()? as i64;
    let height = (y1 - y0 + 1) as f64;
    // width[y] = inner row pixel count
    let width: Vec<usize> = (0..h).map(|y| (0..w).filter(|&x| inner[y][x]).count()).collect();

    // Neck: the lowest strong material change under the head.
    let cx = median(xx.clone());
    let half = std::cmp::max(2, py_round(height * 0.17) as i64) as usize;
    let c0 = std::cmp::max(0, cx as i64 - half as i64) as usize;
    let c1 = std::cmp::min(w as i64, cx as i64 + half as i64 + 1) as usize;
    let lo = (y0 + (height * 0.2) as i64) as usize;
    let hi = (y0 + (height * 0.55) as i64) as usize;
    let mut scores: Vec<(usize, f64)> = Vec::new();
    for y in lo..=hi {
        // An outline band has no interior ink. Python skips that row; `?`
        // would abandon the whole silhouette and fall back to proportions.
        let Some(below) = families(ink, &inner, y, c0, c1) else {
            continue;
        };
        let mut up = y as i64 - 1;
        while up > y0 && families(ink, &inner, up as usize, c0, c1).is_none() {
            up -= 1;
        }
        if let Some(above) = families(ink, &inner, up as usize, c0, c1) {
            let v: f64 = above.iter().zip(&below).map(|(a, b)| (a - b).abs()).sum();
            scores.push((y, v));
        }
    }
    let max_score = scores.iter().map(|(_, v)| *v).fold(0.0f64, f64::max);
    let strong: Vec<usize> = scores
        .iter()
        .filter(|(_, v)| *v >= 0.6 * max_score && *v > 0.5)
        .map(|(y, _)| *y)
        .collect();
    let neck = if !strong.is_empty() {
        *strong.last().unwrap() as i64
    } else {
        // One material head to toe (a skeleton, a slime): the neck is the
        // narrowest row.
        let full: Vec<usize> = (0..h).map(|y| (0..w).filter(|&x| m[y][x]).count()).collect();
        let rows: Vec<usize> = (lo..=hi).filter(|&y| full[y] > 0).collect();
        if rows.is_empty() {
            return None;
        }
        *rows.iter().min_by_key(|&&y| (full[y], y)).unwrap() as i64 + 1
    };

    // Hips: the highest row from which one gap between two legs runs to the floor.
    let gap = |y: i64, near: f64, reach: f64| -> Option<i64> {
        if y < 0 || y as usize >= h {
            return None;
        }
        let y = y as usize;
        let xs: Vec<usize> = (0..w).filter(|&x| inner[y][x]).collect();
        if xs.len() < 2 {
            return None;
        }
        let (xmin, xmax) = (*xs.first().unwrap() as i64, *xs.last().unwrap() as i64);
        let mut hits: Vec<i64> = Vec::new();
        for x in xmin + 1..xmax {
            if !inner[y][x as usize] && (x as f64 - near).abs() <= reach {
                hits.push(x);
            }
        }
        hits.into_iter()
            .min_by(|a, b| {
                (*a as f64 - near)
                    .abs()
                    .partial_cmp(&(*b as f64 - near).abs())
                    .unwrap()
            })
    };
    let floor_row = if width[y1 as usize] > 0 { y1 } else { y1 - 1 };
    let mut hip: Option<i64> = None;
    let mut split = gap(floor_row, cx, std::cmp::max(2, (height * 0.12) as i64) as f64);
    let mut y = floor_row;
    while let Some(s) = split {
        if !(y as f64 > y0 as f64 + height * 0.45) {
            break;
        }
        let g = match gap(y, s as f64, 1.0) {
            Some(g) => g,
            None => break,
        };
        hip = Some(y);
        let _ = g;
        y -= 1;
    }
    let mut robe = match hip {
        None => true,
        Some(hip) => ((y1 - hip + 1) as f64) < 2.0_f64.max((height * 0.12) as f64),
    };
    let mut split_i: i64;
    if robe {
        hip = Some(y1); // only the hem row belongs to the feet
        split_i = py_round(cx) as i64;
    } else {
        split_i = split.unwrap_or(py_round(cx) as i64);
    }
    let _ = &mut split_i;
    let hip = hip?;

    let mut labels: Vec<Vec<&'static str>> = vec![vec![""; w]; h];
    for y in 0..h {
        for x in 0..w {
            if inner[y][x] && (y as i64) < neck {
                labels[y][x] = "head";
            }
        }
    }

    // Arms: interior masses beside the torso, cut off from it by the outline.
    let mut arm: BTreeMap<char, Vec<Vec<bool>>> = BTreeMap::new();
    let mut wings: Option<Vec<Vec<bool>>> = None;
    let arm_floor = if !robe {
        std::cmp::min(y1, hip + (height * 0.12) as i64)
    } else {
        y0 + (height * 0.75) as i64
    };
    let start_max = std::cmp::min(neck + 4, arm_floor);
    for start in neck..start_max {
        let zone: Vec<Vec<bool>> = (0..h)
            .map(|y| (0..w).map(|x| inner[y][x] && y as i64 >= start && y as i64 <= arm_floor).collect())
            .collect();
        let (lab, n) = components(&zone);
        if n < 2 {
            continue;
        }
        let cx_i = py_round(cx) as usize;
        let mut mid: Vec<i64> = Vec::new();
        for yy_ in start as usize..=arm_floor as usize {
            if yy_ < h && cx_i < w && lab[yy_][cx_i] != 0 {
                mid.push(lab[yy_][cx_i]);
            }
        }
        if mid.is_empty() {
            continue;
        }
        // torso_id = the most frequent component under the centre line
        let mut torso_id = mid[0];
        let mut best_count = 0;
        for &cand in &mid {
            let count = mid.iter().filter(|&&v| v == cand).count();
            if count > best_count {
                best_count = count;
                torso_id = cand;
            }
        }
        let mut found: BTreeMap<char, Vec<(f64, i64)>> = BTreeMap::new();
        found.insert('b', Vec::new());
        found.insert('f', Vec::new());
        for i in 1..=n as i64 {
            if i == torso_id {
                continue;
            }
            let mut ys: Vec<usize> = Vec::new();
            let mut xs: Vec<usize> = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    if lab[y][x] == i {
                        ys.push(y);
                        xs.push(x);
                    }
                }
            }
            if ys.len() < 2 || (*ys.iter().min().unwrap() as f64) > start as f64 + height * 0.2 {
                continue; // a leg or a stray, not an arm
            }
            let med = median(xs.iter().map(|&v| v as f64).collect());
            let side = if med < cx { 'b' } else { 'f' };
            found.get_mut(&side).unwrap().push(((med - cx).abs(), i));
        }
        if !(found[&'b'].iter().any(|_| true) && found[&'f'].iter().any(|_| true)) && start < start_max - 1 {
            continue;
        }
        arm.clear();
        let mut wing_grid = vec![vec![false; w]; h];
        for (side, items) in &found {
            let mut items = items.clone();
            items.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for (rank, (_, i)) in items.iter().enumerate() {
                let mut piece: Vec<Vec<bool>> = (0..h)
                    .map(|y| (0..w).map(|x| lab[y][x] == *i).collect())
                    .collect();
                let mut xs: Vec<usize> = Vec::new();
                for y in 0..h {
                    for x in 0..w {
                        if lab[y][x] == *i {
                            xs.push(x);
                        }
                    }
                }
                let xmin = *xs.iter().min().unwrap();
                let xmax = *xs.iter().max().unwrap();
                for y in 0..h {
                    for x in 0..w {
                        // the shoulder pixels above the cut belong to the piece too
                        if inner[y][x]
                            && y as i64 >= neck
                            && (y as i64) < start
                            && x >= xmin
                            && x <= xmax
                        {
                            piece[y][x] = true;
                        }
                    }
                }
                if rank == 0 {
                    arm.insert(*side, piece); // the mass nearest the body is the arm
                } else {
                    for y in 0..h {
                        for x in 0..w {
                            wing_grid[y][x] |= piece[y][x];
                        }
                    }
                }
            }
        }
        wings = Some(wing_grid);
        break;
    }
    if let Some(wg) = &wings {
        for y in 0..h {
            for x in 0..w {
                if wg[y][x] {
                    labels[y][x] = "back";
                }
            }
        }
    }
    for (side, piece) in &arm {
        let ys: Vec<usize> = (0..h).filter(|&y| (0..w).any(|x| piece[y][x])).collect();
        let top = *ys.first().unwrap() as i64;
        let bot = *ys.last().unwrap() as i64;
        let length = (bot - top + 1) as f64;
        let hand = if length >= 6.0 { 2i64 } else { 1 };
        let elbow = top + (length as i64 - hand + 1) / 2;
        for y in 0..h {
            for x in 0..w {
                if piece[y][x] {
                    labels[y][x] = Box::leak(format!("upper_arm_{side}").into_boxed_str());
                    if y as i64 >= elbow {
                        labels[y][x] = Box::leak(format!("lower_arm_{side}").into_boxed_str());
                    }
                    if y as i64 > bot - hand {
                        labels[y][x] = Box::leak(format!("hand_{side}").into_boxed_str());
                    }
                }
            }
        }
    }

    // free = inner & unlabelled
    let free = |labels: &Vec<Vec<&'static str>>| -> Vec<Vec<bool>> {
        (0..h)
            .map(|y| (0..w).map(|x| inner[y][x] && labels[y][x] == "").collect())
            .collect()
    };
    let free_grid = free(&labels);
    if robe {
        let hem_row = if width[y1 as usize] > 0 { y1 } else { y1 - 1 };
        for y in 0..h {
            for x in 0..w {
                if free_grid[y][x] && y as i64 == hem_row {
                    labels[y][x] = if (x as f64) < cx {
                        "foot_b"
                    } else {
                        "foot_f"
                    };
                }
            }
        }
    } else {
        let length = (y1 - hip + 1) as f64;
        let foot = if length >= 6.0 { 2i64 } else { 1 };
        let knee = hip + (length as i64 - foot + 1) / 2;
        for (side, less) in [('b', true), ('f', false)] {
            for y in 0..h {
                for x in 0..w {
                    if free_grid[y][x] && y as i64 >= hip {
                        let side_mask = if less { (x as i64) < split_i } else { (x as i64) > split_i };
                        if side_mask {
                            labels[y][x] = Box::leak(format!("upper_leg_{side}").into_boxed_str());
                            if y as i64 >= knee {
                                labels[y][x] = Box::leak(format!("lower_leg_{side}").into_boxed_str());
                            }
                            if y as i64 > y1 - foot {
                                labels[y][x] = Box::leak(format!("foot_{side}").into_boxed_str());
                            }
                        }
                    }
                }
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            if inner[y][x] && labels[y][x] == "" {
                labels[y][x] = "torso";
            }
        }
    }
    // A side with no readable arm keeps the proportional cut.
    for side in ['b', 'f'] {
        if !arm.contains_key(&side) {
            for prefix in ["upper_arm_", "lower_arm_", "hand_"] {
                let name = format!("{prefix}{side}");
                let old = base_mask(base, &name);
                for y in 0..h {
                    for x in 0..w {
                        if old[y][x] && inner[y][x] && labels[y][x] == "torso" {
                            labels[y][x] = Box::leak(name.clone().into_boxed_str());
                        }
                    }
                }
            }
        }
    }
    let mut labels = spread(labels, &m);

    // Every contact part needs at least one pixel: a hidden hand or foot
    // takes the end of its limb, or the body pixel nearest where it would be.
    for s_ in ['b', 'f'] {
        for (distal, chain, joint) in [
            ("hand_", vec!["lower_arm_", "upper_arm_"], "wrist_"),
            ("foot_", vec!["lower_leg_", "upper_leg_"], "ankle_"),
        ] {
            let name = format!("{distal}{s_}");
            let has = (0..h).any(|y| (0..w).any(|x| labels[y][x] == name.as_str()));
            if has {
                continue;
            }
            let chain_names: Vec<String> = chain.iter().map(|c| format!("{c}{s_}")).collect();
            let mut pool: Vec<Vec<bool>> = (0..h)
                .map(|y| {
                    (0..w)
                        .map(|x| m[y][x] && chain_names.iter().any(|n| labels[y][x] == n.as_str()))
                        .collect()
                })
                .collect();
            let target = base.joints[&format!("{joint}{s_}")];
            let mut any = (0..h).any(|y| (0..w).any(|x| pool[y][x]));
            if !any {
                pool = (0..h).map(|y| (0..w).map(|x| m[y][x] && labels[y][x] == "torso").collect()).collect();
                let side = |x: usize| if s_ == 'b' { (x as f64) < cx } else { (x as f64) >= cx };
                let side_any = (0..h).any(|y| (0..w).any(|x| pool[y][x] && side(x)));
                if side_any {
                    for y in 0..h {
                        for x in 0..w {
                            if pool[y][x] && !side(x) {
                                pool[y][x] = false;
                            }
                        }
                    }
                }
                any = (0..h).any(|y| (0..w).any(|x| pool[y][x]));
                if !any {
                    return None;
                }
            }
            let [x, y] = contact_mask(&pool, target, true).ok()?;
            labels[y as usize][x as usize] = Box::leak(name.clone().into_boxed_str());
        }
    }

    let med = |name: &str, axis: usize, fallback: f64| -> f64 {
        let mut xs: Vec<f64> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if labels[y][x] == name {
                    xs.push(if axis == 0 { x as f64 } else { y as f64 });
                }
            }
        }
        if xs.is_empty() {
            fallback
        } else {
            median(xs)
        }
    };
    let top = |name: &str, fallback: f64| -> f64 {
        let mut ys: Vec<f64> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if labels[y][x] == name {
                    ys.push(y as f64);
                }
            }
        }
        if ys.is_empty() {
            fallback
        } else {
            ys.into_iter().fold(f64::INFINITY, f64::min)
        }
    };

    let mut joints: BTreeMap<String, [f64; 2]> = BTreeMap::new();
    joints.insert(
        "root".to_owned(),
        [cx, if !robe { hip as f64 } else { y0 as f64 + height * 0.6 }],
    );
    joints.insert("neck".to_owned(), [cx, neck as f64]);
    for (s, sign) in [('b', -1.0f64), ('f', 1.0f64)] {
        let old = &base.joints;
        let ax = med(&format!("upper_arm_{s}"), 0, old[&format!("shoulder_{s}")][0]);
        let mut ax_j = ax;
        {
            let mut xs: Vec<f64> = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    if labels[y][x] == format!("upper_arm_{s}").as_str() {
                        xs.push(x as f64);
                    }
                }
            }
            if !xs.is_empty() {
                // pivot on the torso side of a thick arm, so it swings
                // without leaving the body
                let wide = (xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min) + 1.0) as i64;
                let xmax = xs.iter().cloned().fold(f64::MIN, f64::max);
                let xmin = xs.iter().cloned().fold(f64::MAX, f64::min);
                ax_j = if s == 'b' {
                    xmax - std::cmp::min(1, wide / 3) as f64
                } else {
                    xmin + std::cmp::min(1, wide / 3) as f64
                };
            }
        }
        joints.insert(
            format!("shoulder_{s}"),
            [ax_j, top(&format!("upper_arm_{s}"), old[&format!("shoulder_{s}")][1]) + 1.0],
        );
        joints.insert(
            format!("elbow_{s}"),
            [med(&format!("lower_arm_{s}"), 0, ax), top(&format!("lower_arm_{s}"), old[&format!("elbow_{s}")][1])],
        );
        joints.insert(
            format!("wrist_{s}"),
            [med(&format!("hand_{s}"), 0, ax), top(&format!("hand_{s}"), old[&format!("wrist_{s}")][1])],
        );
        let lx = med(
            &format!("upper_leg_{s}"),
            0,
            med(&format!("foot_{s}"), 0, old[&format!("hip_{s}")][0]),
        );
        joints.insert(format!("hip_{s}"), [lx, top(&format!("upper_leg_{s}"), old[&format!("hip_{s}")][1])]);
        joints.insert(
            format!("knee_{s}"),
            [med(&format!("lower_leg_{s}"), 0, lx), top(&format!("lower_leg_{s}"), old[&format!("knee_{s}")][1])],
        );
        joints.insert(
            format!("ankle_{s}"),
            [med(&format!("foot_{s}"), 0, lx), top(&format!("foot_{s}"), old[&format!("ankle_{s}")][1])],
        );
    }
    let joints: BTreeMap<String, [f64; 2]> = joints
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                [
                    round3(np_clip(v[0], 0.0, w as f64 - 1.0)),
                    round3(np_clip(v[1], 0.0, h as f64 - 1.0)),
                ],
            )
        })
        .collect();
    let parts: Vec<PartDef> = LAYOUT
        .iter()
        .map(|&(name, parent, joint, z)| PartDef {
            name,
            parent,
            joint,
            z,
            mask: (0..h)
                .map(|y| (0..w).map(|x| m[y][x] && labels[y][x] == name).collect())
                .collect(),
        })
        .collect();

    let contact = |name: &str, target: [f64; 2], bottom: bool| -> Option<[i64; 2]> {
        let mask: Vec<Vec<bool>> = (0..h)
            .map(|y| (0..w).map(|x| m[y][x] && labels[y][x] == name).collect())
            .collect();
        contact_mask(&mask, target, bottom).ok()
    };
    let has = |n: &str| (0..h).any(|y| (0..w).any(|x| labels[y][x] == n));
    let floor = [base.origin[0] as f64, base.origin[1] as f64];
    let grip = if has("hand_f") {
        contact("hand_f", joints["wrist_f"], false)?
    } else {
        base.anchors.iter().find(|a| a.0 == "grip").map(|a| [a.2[0] as i64, a.2[1] as i64])?
    };
    let off = if has("hand_b") {
        contact("hand_b", joints["wrist_b"], false)?
    } else {
        base.anchors.iter().find(|a| a.0 == "off_hand").map(|a| [a.2[0] as i64, a.2[1] as i64])?
    };
    let foot_part: String = if has("foot_f") {
        "foot_f".to_owned()
    } else if has("foot_b") {
        "foot_b".to_owned()
    } else {
        base.anchors
            .iter()
            .find(|a| a.0 == "feet")
            .map(|a| a.1.clone())?
    };
    let back_part = if has("back") { "back" } else { "torso" };
    let head_p = contact("head", [cx, neck as f64], true)?;
    let back_p = contact(back_part, [cx, (neck as f64 + joints["root"][1]) / 2.0], false)?;
    let feet_p = if has(&foot_part) {
        contact(&foot_part, floor, true)?
    } else {
        base.anchors
            .iter()
            .find(|a| a.0 == "feet")
            .map(|a| [a.2[0] as i64, a.2[1] as i64])?
    };
    let tip_y = round3(grip[1] as f64 + height * 0.45);
    let _ = sign_iter();

    let fit = Fitted {
        src_w: w,
        src_h: h,
        canvas: base.canvas,
        pivot: base.pivot,
        origin: base.origin,
        joints,
        parts,
        seam_radius: base.seam_radius,
        anchors: vec![
            ("grip", "hand_f".to_owned(), [grip[0] as f64, grip[1] as f64]),
            ("off_hand", "hand_b".to_owned(), [off[0] as f64, off[1] as f64]),
            ("head", "head".to_owned(), [head_p[0] as f64, head_p[1] as f64]),
            ("back", back_part.to_owned(), [back_p[0] as f64, back_p[1] as f64]),
            ("feet", foot_part, [feet_p[0] as f64, feet_p[1] as f64]),
            ("tip", "hand_f".to_owned(), [grip[0] as f64, tip_y]),
        ],
        clips: defaults(height),
    };
    let _ = robe_flag(); // silence unused when robe branch above sets nothing
    robe = robe; // keep the final plan value
    Some(SmartFitted {
        fit,
        plan: if robe { Plan::Robe } else { Plan::Biped },
    })
}

fn sign_iter() {}
fn robe_flag() -> bool {
    false
}

fn round3(v: f64) -> f64 {
    format!("{:.3}", v).parse().unwrap_or(v)
}

fn base_mask(base: &Fitted, name: &str) -> Vec<Vec<bool>> {
    base.parts
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.mask.clone())
        .unwrap_or_default()
}

/// The Python `smartfit.fit` on a parsed ink grid: silhouette fit, or the
/// proportional base rig when the drawing has no readable silhouette.
pub fn fit_smart(grid: &[Vec<char>]) -> Result<SmartFitted, String> {
    let base = super::auto::fit_proportional(&grid.to_vec())?;
    match smart_fit(&grid, &base) {
        Some(smart) => Ok(smart),
        None => Ok(SmartFitted {
            fit: base,
            plan: Plan::Biped,
        }),
    }
}

/// The silhouette plan for these rows: "biped" or "robe".
pub fn plan_smart(grid: &[Vec<char>]) -> Result<&'static str, String> {
    Ok(fit_smart(grid)?.plan.as_str())
}

/// `rig.json` for a silhouette fit (schema + `"plan"`), matching the Python
/// `quick.save_rig(smartfit.fit(...))`.
pub fn rig_json_smart(fitted: &SmartFitted) -> Result<String, String> {
    let fit = &fitted.fit;
    let mut joints = serde_json::Map::new();
    for (k, v) in &fit.joints {
        joints.insert(k.clone(), json!([v[0], v[1]]));
    }
    let mut parts = serde_json::Map::new();
    for p in &fit.parts {
        parts.insert(
            p.name.to_owned(),
            json!({
                "parent": p.parent,
                "joint": p.joint,
                "z": p.z,
                "mask": p.mask.iter().map(|row| row.iter().map(|&b| if b { '#' } else { '.' }).collect::<String>()).collect::<Vec<_>>(),
            }),
        );
    }
    let mut anchors = serde_json::Map::new();
    for (name, part, point) in &fit.anchors {
        let px = if point[0].fract() == 0.0 {
            json!(point[0] as i64)
        } else {
            json!(point[0])
        };
        // tip y is Python round(..., 3): a whole value stays 22.0, not 22.
        let py = if *name == "tip" || point[1].fract() != 0.0 {
            json!(point[1])
        } else {
            json!(point[1] as i64)
        };
        anchors.insert(
            name.to_string(),
            json!({"part": part.clone(), "point": [px, py]}),
        );
    }
    let mut clips = serde_json::Map::new();
    for (name, poses) in &fit.clips {
        let mut arr = Vec::new();
        for p in poses {
            let mut angles = serde_json::Map::new();
            for (part, a) in &p.angles {
                angles.insert(part.to_string(), json!(a));
            }
            arr.push(json!({
                "ticks": p.ticks,
                "angles": angles,
                "offset": [p.offset[0], p.offset[1]],
                "rotation": p.rotation,
                "smear": p.smear,
            }));
        }
        clips.insert(name.to_string(), Value::Array(arr));
    }
    use serde_json::Value;
    let rig = json!({
        "version": 1,
        "source": {
            "size": [fit.src_w, fit.src_h],
            "cell": null,
            "cell_size": null,
            "facing": "right",
            "alpha_threshold": 128,
            "key_color": null,
        },
        "canvas": [fit.canvas[0], fit.canvas[1]],
        "pivot": [fit.pivot[0], fit.pivot[1]],
        "origin": fit.origin,
        "joints": Value::Object(joints),
        "seam_radius": fit.seam_radius,
        "anchors": Value::Object(anchors),
        "parts": Value::Object(parts),
        "clips": Value::Object(clips),
        "plan": fitted.plan.as_str(),
    });
    let mut text = serde_json::to_string_pretty(&rig).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

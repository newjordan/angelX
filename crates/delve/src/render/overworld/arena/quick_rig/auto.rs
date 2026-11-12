//! The quick auto-rigger, ported from `tools/sprite-rig/spriterig/quick.py`
//! (`fit` + `build`, plus `quick_motion.defaults`) so the game can rig any
//! wished right-facing sprite in-process, with no Python.
//!
//! Input is ink rows (`.` clear, one ink char per pixel, facing right); the
//! PNG/key-colour/cell handling of the Python tool is intentionally not
//! ported. All arithmetic is f64 in the Python's operation order, including
//! its roundings: `py_round` is banker's rounding (`round()`), `round_n`
//! matches `round(v, 3)`, and the smear polyline reproduces Pillow 13's
//! wide-line rasterization (`line()`/`ImagingDrawWideLine`/`polygon_generic`
//! from libImaging/Draw.c, f32 scanline math included) bit for bit.

use std::collections::BTreeMap;
use std::f64::consts::PI;

use serde_json::{Map, Value, json};

use super::Rig;
use crate::render::overworld::ink;

const CLIPS: [&str; 13] = [
    "idle", "walk", "run", "roll", "hurt", "die", "cast", "slash", "overhead", "thrust", "flail",
    "bow", "gun",
];
const ANCHOR_NAMES: [&str; 6] = ["grip", "off_hand", "head", "back", "feet", "tip"];
/// (name, parent, source joint, draw order); front means image-right.
pub(super) const LAYOUT: [(&str, Option<&str>, &str, i64); 15] = [
    ("back", Some("torso"), "neck", 0),
    ("upper_arm_b", Some("torso"), "shoulder_b", 1),
    ("lower_arm_b", Some("upper_arm_b"), "elbow_b", 2),
    ("hand_b", Some("lower_arm_b"), "wrist_b", 3),
    ("upper_leg_b", None, "hip_b", 4),
    ("lower_leg_b", Some("upper_leg_b"), "knee_b", 5),
    ("foot_b", Some("lower_leg_b"), "ankle_b", 6),
    ("upper_leg_f", None, "hip_f", 7),
    ("lower_leg_f", Some("upper_leg_f"), "knee_f", 8),
    ("foot_f", Some("lower_leg_f"), "ankle_f", 9),
    ("torso", None, "root", 10),
    ("head", Some("torso"), "neck", 11),
    ("upper_arm_f", Some("torso"), "shoulder_f", 12),
    ("lower_arm_f", Some("upper_arm_f"), "elbow_f", 13),
    ("hand_f", Some("lower_arm_f"), "wrist_f", 14),
];

const MAX_SIDE: i64 = 1024;

/// A char grid of realm inks (`.` is clear).
type Grid = Vec<Vec<char>>;
type Mat = [[f64; 2]; 2];

fn mat_vec(r: Mat, v: [f64; 2]) -> [f64; 2] {
    // NumPy `@` contracts with a fused multiply-add. A plain add is one ulp
    // low on some poses and drops a destination pixel across a half boundary.
    [
        r[0][0].mul_add(v[0], r[0][1] * v[1]),
        r[1][0].mul_add(v[0], r[1][1] * v[1]),
    ]
}

fn mat_mul(a: Mat, b: Mat) -> Mat {
    [
        [
            a[0][0].mul_add(b[0][0], a[0][1] * b[1][0]),
            a[0][0].mul_add(b[0][1], a[0][1] * b[1][1]),
        ],
        [
            a[1][0].mul_add(b[0][0], a[1][1] * b[1][0]),
            a[1][0].mul_add(b[0][1], a[1][1] * b[1][1]),
        ],
    ]
}

fn rotation(degrees: f64) -> Mat {
    let r = (degrees.rem_euclid(360.0)) * PI / 180.0;
    let (s, c) = (r.sin(), r.cos());
    [[c, -s], [s, c]]
}

/// Python `round()`: ties to even.
pub(super) fn py_round(v: f64) -> f64 {
    let f = v.floor();
    let diff = v - f;
    if diff > 0.5 {
        f + 1.0
    } else if diff < 0.5 {
        f
    } else if (f as i64) % 2 == 0 {
        f
    } else {
        f + 1.0
    }
}

/// Python `round(v, n)` for the joint decimals (n <= 3 here).
pub(super) fn round_n(v: f64, n: usize) -> f64 {
    format!("{:.*}", n, v).parse().unwrap_or(v)
}

/// `numpy.median` of a sample (mean of the two middle values when even).
pub(super) fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    }
}

/// `numpy.clip(v, lo, hi)`.
pub(super) fn np_clip(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

// ---------------------------------------------------------------------------
// Motion clips (quick_motion.defaults)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(super) struct Pose {
    pub(super) ticks: i64,
    pub(super) angles: Vec<(&'static str, f64)>,
    pub(super) offset: [f64; 2],
    pub(super) rotation: f64,
    pub(super) smear: bool,
}

impl Pose {
    fn angle(&self, part: &str) -> f64 {
        self.angles
            .iter()
            .find(|(n, _)| *n == part)
            .map(|(_, a)| *a)
            .unwrap_or(0.0)
    }
}

fn pose(ticks: i64, angles: Vec<(&'static str, f64)>) -> Pose {
    Pose {
        ticks,
        angles,
        offset: [0.0, 0.0],
        rotation: 0.0,
        smear: false,
    }
}

fn arms(f: f64, fore: f64, b: f64, back_fore: f64, body: Vec<(&'static str, f64)>) -> Vec<(&'static str, f64)> {
    let mut out = vec![
        ("upper_arm_f", f),
        ("lower_arm_f", fore),
        ("upper_arm_b", b),
        ("lower_arm_b", back_fore),
    ];
    out.extend(body);
    out
}

pub(super) fn defaults(h: f64) -> Vec<(&'static str, Vec<Pose>)> {
    let mut clips: Vec<(&'static str, Vec<Pose>)> = Vec::new();

    clips.push((
        "idle",
        [0.0, 0.7, 1.0, 0.0, -0.7, -1.0]
            .iter()
            .map(|&s| {
                pose(
                    6,
                    arms(
                        4.0 * s,
                        -3.0 * s.abs(),
                        -4.0 * s,
                        0.0,
                        vec![("torso", 4.0 * s), ("head", -s), ("back", 3.0 * s)],
                    ),
                )
            })
            .collect(),
    ));

    for &(name, swing, ticks) in [("walk", 25.0f64, 6i64), ("run", 48.0f64, 3i64)].iter() {
        let mut frames = Vec::new();
        for i in 0..8 {
            let t = 2.0 * PI * i as f64 / 8.0;
            let (s, c) = (t.sin(), t.cos());
            let fore = if name == "walk" { -12.0 } else { -55.0 };
            let a = arms(
                -s * swing * 0.8,
                fore,
                s * swing * 0.8,
                fore,
                vec![
                    ("upper_leg_f", s * swing),
                    ("upper_leg_b", -s * swing),
                    ("lower_leg_f", -f64::max(0.0, -s) * swing * 1.5),
                    ("lower_leg_b", -f64::max(0.0, s) * swing * 1.5),
                    ("foot_f", f64::max(0.0, -s) * 20.0),
                    ("foot_b", f64::max(0.0, s) * 20.0),
                    ("torso", if name == "walk" { 0.0 } else { 12.0 }),
                    ("head", -3.0),
                    ("back", -5.0 + 8.0 * c),
                ],
            );
            let lift = if name == "walk" { 0.025 } else { 0.06 };
            let mut p = pose(ticks, a);
            p.offset = [0.0, -c.abs() * h * lift];
            frames.push(p);
        }
        clips.push((name, frames));
    }

    let mut roll = vec![{
        let mut p = pose(
            9,
            arms(
                20.0,
                -45.0,
                -20.0,
                -45.0,
                vec![("upper_leg_f", 35.0), ("lower_leg_f", -65.0), ("torso", 20.0)],
            ),
        );
        p.offset = [0.0, h * 0.06];
        p
    }];
    for &degrees in [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0, 360.0].iter() {
        let s = ((degrees / 2.0) * PI / 180.0).sin().powi(2);
        let mut p = pose(
            3,
            arms(
                -130.0 * s,
                -50.0 * s,
                -120.0 * s,
                -50.0 * s,
                vec![
                    ("upper_leg_f", 65.0 * s),
                    ("lower_leg_f", -100.0 * s),
                    ("upper_leg_b", 50.0 * s),
                    ("lower_leg_b", -90.0 * s),
                    ("head", 20.0 * s),
                    ("back", 25.0 * s),
                ],
            ),
        );
        p.offset = [0.0, -h * 1.05 * s];
        p.rotation = degrees;
        roll.push(p);
    }
    roll.push(pose(6, vec![]));
    clips.push(("roll", roll));

    let kick = f64::max(1.0, py_round(h * 0.10));
    let mut hurt = vec![{
        let mut p = pose(
            3,
            arms(55.0, -65.0, 45.0, -65.0, vec![("torso", -24.0), ("head", -10.0)]),
        );
        p.offset = [-f64::max(2.0, py_round(h * 0.13)), 0.0];
        p
    }];
    hurt.push({
        let mut p = pose(9, arms(35.0, -50.0, 30.0, -50.0, vec![("torso", -14.0), ("head", 8.0)]));
        p.offset = [-kick / 2.0, 0.0];
        p
    });
    hurt.push(pose(6, vec![("torso", -6.0), ("head", 3.0)]));
    hurt.push(pose(9, vec![]));
    clips.push(("hurt", hurt));

    let mut die = vec![
        pose(6, vec![("torso", -15.0), ("head", -10.0)]),
        pose(9, arms(35.0, -45.0, 30.0, -35.0, vec![("torso", -20.0)])),
    ];
    for &a in [15.0, 35.0, 60.0, 82.0, 90.0].iter() {
        let mut p = pose(6, arms(15.0, -20.0, 25.0, -30.0, vec![("head", 10.0), ("back", -15.0)]));
        p.rotation = a;
        die.push(p);
    }
    die.push({
        let mut p = pose(30, arms(15.0, -20.0, 25.0, -30.0, vec![("head", 10.0), ("back", -15.0)]));
        p.rotation = 90.0;
        p
    });
    clips.push(("die", die));

    clips.push((
        "cast",
        vec![
            pose(9, arms(-30.0, -110.0, 30.0, 110.0, vec![("torso", -5.0), ("head", 5.0)])),
            pose(6, arms(-105.0, -50.0, 105.0, 50.0, vec![("head", -5.0)])),
            pose(9, arms(-155.0, -5.0, 155.0, 5.0, vec![("torso", -4.0), ("back", 12.0)])),
            pose(3, arms(-140.0, -10.0, 140.0, 10.0, vec![("torso", 8.0), ("head", -5.0)])),
            pose(9, arms(-120.0, -20.0, 120.0, 20.0, vec![])),
            pose(6, arms(-40.0, -45.0, 40.0, 45.0, vec![])),
            pose(9, vec![]),
        ],
    ));

    let mut slash = vec![
        pose(9, arms(55.0, -45.0, -20.0, -40.0, vec![("torso", -12.0), ("head", 5.0)])),
        pose(6, arms(75.0, -50.0, -25.0, -45.0, vec![("torso", -18.0), ("back", 10.0)])),
    ];
    for &(a, off, smear) in [
        (-65.0, 0.0, true),
        (-140.0, 0.0, true),
    ]
    .iter()
    {
        let mut p = pose(3, arms(a, if a == -65.0 { -15.0 } else { -5.0 }, if a == -65.0 { 20.0 } else { 30.0 }, if a == -65.0 { -50.0 } else { -45.0 }, vec![("torso", if a == -65.0 { 10.0 } else { 18.0 })]));
        p.offset = [h * 0.04 * if a == -65.0 { 1.0 } else { 1.5 }, 0.0];
        p.smear = smear;
        slash.push(p);
    }
    slash.push(pose(9, arms(-130.0, -15.0, 15.0, -35.0, vec![("torso", 12.0)])));
    slash.push(pose(6, arms(-55.0, -25.0, 5.0, -20.0, vec![("torso", 5.0)])));
    slash.push(pose(9, vec![]));
    clips.push(("slash", slash));

    let mut overhead = vec![
        pose(6, arms(-70.0, -75.0, -40.0, -85.0, vec![("torso", -5.0)])),
    ];
    {
        let mut p = pose(12, arms(-170.0, -15.0, -150.0, -25.0, vec![("torso", -12.0), ("head", -8.0)]));
        overhead.push(p);
    }
    {
        let mut p = pose(3, arms(-95.0, -5.0, -85.0, -15.0, vec![("torso", 12.0)]));
        p.smear = true;
        overhead.push(p);
    }
    {
        let mut p = pose(3, arms(-20.0, -10.0, -30.0, -15.0, vec![("torso", 22.0)]));
        p.offset = [h * 0.04, 0.0];
        p.smear = true;
        overhead.push(p);
    }
    overhead.push(pose(9, arms(-10.0, -20.0, -20.0, -25.0, vec![("torso", 18.0)])));
    overhead.push(pose(9, arms(-5.0, -10.0, -5.0, -10.0, vec![("torso", 7.0)])));
    overhead.push(pose(9, vec![]));
    clips.push(("overhead", overhead));

    let mut thrust = vec![
        pose(9, arms(30.0, -110.0, -30.0, -70.0, vec![("torso", -10.0)])),
    ];
    {
        let mut p = pose(6, arms(40.0, -120.0, -30.0, -80.0, vec![("torso", -15.0)]));
        p.offset = [-h * 0.04, 0.0];
        thrust.push(p);
    }
    for &torso in [12.0, 15.0].iter() {
        let mut p = pose(
            if torso == 12.0 { 3 } else { 6 },
            arms(-90.0, 0.0, -40.0, -45.0, vec![("torso", torso)]),
        );
        p.offset = [h * 0.08, 0.0];
        if torso == 12.0 {
            p.smear = true;
        }
        thrust.push(p);
    }
    thrust.push(pose(9, arms(-50.0, -55.0, -20.0, -45.0, vec![("torso", 5.0)])));
    thrust.push(pose(9, vec![]));
    clips.push(("thrust", thrust));

    let mut flail = vec![pose(9, arms(40.0, -50.0, -20.0, -30.0, vec![("torso", -10.0)]))];
    for &a in [0.0, -70.0, -140.0, -210.0, -280.0, -350.0].iter() {
        let mut p = pose(
            3,
            arms(a, -20.0, -a * 0.2, -40.0, vec![("torso", 8.0 * (a * PI / 180.0).sin())]),
        );
        p.smear = true;
        flail.push(p);
    }
    flail.push(pose(9, arms(-20.0, -35.0, 10.0, -30.0, vec![("torso", 10.0)])));
    flail.push(pose(9, vec![]));
    clips.push(("flail", flail));

    clips.push((
        "bow",
        vec![
            pose(6, arms(-45.0, -40.0, -65.0, -25.0, vec![("torso", 3.0)])),
            pose(9, arms(-90.0, 0.0, -85.0, -5.0, vec![("torso", 3.0), ("head", -3.0)])),
            pose(12, arms(-90.0, 0.0, -20.0, -155.0, vec![("torso", -12.0), ("head", 5.0)])),
            pose(3, arms(-90.0, 0.0, 30.0, -155.0, vec![("torso", 6.0), ("head", -3.0)])),
            pose(9, arms(-85.0, -5.0, 20.0, -100.0, vec![("torso", 3.0)])),
            pose(9, arms(-35.0, -30.0, 5.0, -35.0, vec![])),
            pose(9, vec![]),
        ],
    ));

    let mut gun = vec![
        pose(6, arms(-40.0, -40.0, -35.0, -50.0, vec![])),
        pose(12, arms(-90.0, 0.0, -65.0, -30.0, vec![("head", -3.0)])),
    ];
    {
        let mut p = pose(3, arms(-112.0, -12.0, -85.0, -35.0, vec![("torso", -14.0), ("head", 6.0)]));
        p.offset = [-kick, 0.0];
        gun.push(p);
    }
    {
        let mut p = pose(6, arms(-98.0, -5.0, -70.0, -30.0, vec![("torso", -6.0), ("head", 3.0)]));
        p.offset = [-kick / 2.0, 0.0];
        gun.push(p);
    }
    gun.push(pose(9, arms(-90.0, 0.0, -65.0, -30.0, vec![])));
    gun.push(pose(9, arms(-35.0, -25.0, -25.0, -30.0, vec![])));
    gun.push(pose(9, vec![]));
    clips.push(("gun", gun));

    CLIPS
        .iter()
        .map(|name| (*name, clips.iter().find(|(n, _)| n == name).unwrap().1.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// fit
// ---------------------------------------------------------------------------

pub(super) struct PartDef {
    pub(super) name: &'static str,
    pub(super) parent: Option<&'static str>,
    pub(super) joint: &'static str,
    pub(super) z: i64,
    pub(super) mask: Vec<Vec<bool>>,
}

pub(super) struct Fitted {
    pub(super) src_w: usize,
    pub(super) src_h: usize,
    pub(super) canvas: [i64; 2],
    pub(super) pivot: [i64; 2],
    pub(super) origin: [i64; 2],
    pub(super) joints: BTreeMap<String, [f64; 2]>,
    pub(super) parts: Vec<PartDef>,
    pub(super) seam_radius: i64,
    /// (anchor name, owning part, source point)
    pub(super) anchors: Vec<(&'static str, String, [f64; 2])>,
    pub(super) clips: Vec<(&'static str, Vec<Pose>)>,
}

/// `_contact`: real pixel of one owning mask, nearest to the target
/// (row-major scan; the first minimum wins, as numpy's argmin).
pub(super) fn contact_mask(
    mask: &[Vec<bool>],
    target: [f64; 2],
    bottom: bool,
) -> Result<[i64; 2], String> {
    let h = mask.len();
    let w = mask.first().map(|r| r.len()).unwrap_or(0);
    let mut ymax: i64 = -1;
    if bottom {
        for y in 0..h {
            for x in 0..w {
                if mask[y][x] && y as i64 > ymax {
                    ymax = y as i64;
                }
            }
        }
    }
    let mut best: Option<(f64, i64, i64)> = None;
    for y in 0..h {
        if bottom && y as i64 != ymax {
            continue;
        }
        for x in 0..w {
            if !mask[y][x] {
                continue;
            }
            let dx = x as f64 - target[0];
            let dy = y as f64 - target[1];
            let d = dx * dx + dy * dy;
            if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
                best = Some((d, x as i64, y as i64));
            }
        }
    }
    Ok(best.map(|(_, x, y)| [x, y]).unwrap_or_else(|| rounded_target(target)))
}

fn rounded_target(target: [f64; 2]) -> [i64; 2] {
    [(target[0] + 0.5).floor() as i64, (target[1] + 0.5).floor() as i64]
}

fn label_id(name: &str) -> usize {
    const LABELS: [&str; 15] = [
        "torso", "head", "back", "upper_leg_b", "lower_leg_b", "foot_b", "upper_leg_f",
        "lower_leg_f", "foot_f", "upper_arm_b", "lower_arm_b", "hand_b", "upper_arm_f",
        "lower_arm_f", "hand_f",
    ];
    LABELS.iter().position(|l| *l == name).unwrap()
}

/// Public shims for `smart` (same module tree).
pub(super) fn parse_rows_pub(rows: &[&str]) -> Result<Grid, String> {
    parse_rows(rows)
}

pub(super) fn fit_proportional(ink: &Grid) -> Result<Fitted, String> {
    fit(ink)
}

fn fit(ink: &Grid) -> Result<Fitted, String> {
    let h = ink.len();
    let w = ink[0].len();
    let m: Vec<Vec<bool>> = ink.iter().map(|r| r.iter().map(|&c| c != '.').collect()).collect();

    let mut y0 = i64::MAX;
    let mut y1 = i64::MIN;
    let mut x0 = i64::MAX;
    let mut x1 = i64::MIN;
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if m[y][x] {
                any = true;
                y0 = y0.min(y as i64);
                y1 = y1.max(y as i64);
                x0 = x0.min(x as i64);
                x1 = x1.max(x as i64);
            }
        }
    }
    if !any {
        return Err("source: no visible pixels after alpha/key filtering".to_owned());
    }
    let height = y1 - y0 + 1;
    if height < 4 || x1 - x0 < 2 {
        return Err("source: silhouette is too small to fit a humanoid (need 3x4)".to_owned());
    }

    let band_lo = y0 as f64 + height as f64 * 0.28;
    let band_hi = y0 as f64 + height as f64 * 0.57;
    let mut band_xs = Vec::new();
    let mut all_xs = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if m[y][x] {
                all_xs.push(x as f64);
                let yf = y as f64;
                if yf >= band_lo && yf < band_hi {
                    band_xs.push(x as f64);
                }
            }
        }
    }
    let cx = if !band_xs.is_empty() {
        median(band_xs)
    } else {
        median(all_xs)
    };

    let neck = y0 as f64 + height as f64 * 0.25;
    let hip = y0 as f64 + height as f64 * 0.60;
    let elbow = y0 as f64 + height as f64 * 0.44;
    let wrist = y0 as f64 + height as f64 * 0.60;
    let knee = y0 as f64 + height as f64 * 0.79;
    let ankle = y0 as f64 + height as f64 * 0.92;

    let mut runs: Vec<f64> = Vec::new();
    let start = (neck as i64) as usize;
    let stop = ((neck as i64 + 1).max(hip as i64)) as usize;
    for y in start..stop {
        let c = (py_round(cx) as i64).clamp(0, w as i64 - 1) as usize;
        if m[y][c] {
            let mut l = c;
            let mut r = c;
            while l > 0 && m[y][l - 1] {
                l -= 1;
            }
            while r + 1 < w && m[y][r + 1] {
                r += 1;
            }
            runs.push((r - l + 1) as f64 / 2.0);
        }
    }
    let torso_width = {
        let v = if !runs.is_empty() {
            median(runs)
        } else {
            height as f64 * 0.14
        };
        np_clip(
            v,
            f64::max(1.0, height as f64 * 0.08),
            f64::max(1.0, height as f64 * 0.17),
        )
    };

    let mut labels = vec![vec![label_id("torso"); w]; h];
    for y in 0..h {
        if (y as f64) < neck {
            for x in 0..w {
                labels[y][x] = label_id("head");
            }
        }
    }
    let mut set = |labels: &mut Vec<Vec<usize>>, y: usize, x: usize, id: usize| labels[y][x] = id;
    let _ = &mut set;
    for side in ["b", "f"].iter() {
        let leg_u = label_id(&format!("upper_leg_{side}"));
        let leg_l = label_id(&format!("lower_leg_{side}"));
        let foot = label_id(&format!("foot_{side}"));
        let arm_u = label_id(&format!("upper_arm_{side}"));
        let arm_l = label_id(&format!("lower_arm_{side}"));
        let hand = label_id(&format!("hand_{side}"));
        for y in 0..h {
            for x in 0..w {
                if !m[y][x] {
                    continue;
                }
                let xf = x as f64;
                let side_mask = if *side == "b" { xf < cx } else { xf >= cx };
                if (y as f64) >= hip && side_mask {
                    labels[y][x] = leg_u;
                    if (y as f64) >= knee {
                        labels[y][x] = leg_l;
                    }
                    if (y as f64) >= ankle {
                        labels[y][x] = foot;
                    }
                }
                if side_mask
                    && (xf - cx).abs() > torso_width
                    && (y as f64) >= neck
                    && (y as f64) < hip + height as f64 * 0.12
                {
                    labels[y][x] = arm_u;
                    if (y as f64) >= elbow {
                        labels[y][x] = arm_l;
                    }
                    if (y as f64) >= wrist {
                        labels[y][x] = hand;
                    }
                }
            }
        }
    }

    // Rear hanging strip (cape).
    {
        let mut tys: Vec<i64> = Vec::new();
        let mut txs: Vec<i64> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let xf = x as f64;
                let yf = y as f64;
                if m[y][x]
                    && xf < cx - torso_width - 1.0
                    && yf >= hip + height as f64 * 0.06
                    && yf < ankle
                {
                    tys.push(y as i64);
                    txs.push(x as i64);
                }
            }
        }
        if tys.len() >= 3 && (tys.iter().max().unwrap() - tys.iter().min().unwrap()) as f64 >= height as f64 * 0.12
        {
            let tx_max = *txs.iter().max().unwrap();
            for y in 0..h {
                for x in 0..w {
                    let yf = y as f64;
                    if m[y][x] && (x as i64) <= tx_max && yf >= neck && yf < ankle {
                        labels[y][x] = label_id("back");
                    }
                }
            }
        }
    }

    // Distal fixups.
    for side in ["b", "f"].iter() {
        for (prefix, chain) in [
            ("hand_", ["lower_arm_", "upper_arm_"]),
            ("foot_", ["lower_leg_", "upper_leg_"]),
        ]
        .iter()
        {
            let name = format!("{prefix}{side}");
            let id = label_id(&name);
            if (0..h).any(|y| (0..w).any(|x| m[y][x] && labels[y][x] == id)) {
                continue;
            }
            let chain_ids = [
                label_id(&format!("{}{side}", chain[0])),
                label_id(&format!("{}{side}", chain[1])),
            ];
            let mut candidates = vec![vec![false; w]; h];
            let mut any = false;
            for y in 0..h {
                for x in 0..w {
                    if m[y][x] && (labels[y][x] == chain_ids[0] || labels[y][x] == chain_ids[1]) {
                        candidates[y][x] = true;
                        any = true;
                    }
                }
            }
            if any {
                let [x, y] = contact_mask(&candidates, [cx, y1 as f64], true)?;
                labels[y as usize][x as usize] = id;
            }
        }
    }

    let point = |x: f64, y: f64| -> [f64; 2] {
        [
            round_n(np_clip(x, 0.0, w as f64 - 1.0), 3),
            round_n(np_clip(y, 0.0, h as f64 - 1.0), 3),
        ]
    };
    let mass = |name: &str, fallback: f64, axis: usize| -> f64 {
        let id = label_id(name);
        let mut xs = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if m[y][x] && labels[y][x] == id {
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

    let mut joints = BTreeMap::new();
    joints.insert("root".to_owned(), point(cx, hip));
    joints.insert("neck".to_owned(), point(cx, neck));
    for (s, sign) in [("b", -1.0f64), ("f", 1.0f64)].iter() {
        let arm_x = mass(&format!("upper_arm_{s}"), cx + sign * (torso_width + 0.5), 0);
        let fore_x = mass(&format!("lower_arm_{s}"), arm_x, 0);
        let hand_x = mass(&format!("hand_{s}"), fore_x, 0);
        let leg_x = mass(&format!("upper_leg_{s}"), cx + sign * height as f64 * 0.10, 0);
        let knee_x = mass(&format!("lower_leg_{s}"), leg_x, 0);
        joints.insert(format!("shoulder_{s}"), point(cx + sign * torso_width, neck + height as f64 * 0.04));
        joints.insert(format!("elbow_{s}"), point((arm_x + fore_x) / 2.0, elbow));
        joints.insert(format!("wrist_{s}"), point(hand_x, wrist));
        joints.insert(format!("hip_{s}"), point(leg_x, hip));
        joints.insert(format!("knee_{s}"), point(knee_x, knee));
        joints.insert(format!("ankle_{s}"), point(mass(&format!("foot_{s}"), knee_x, 0), ankle));
    }

    let label_mask = |name: &str| -> Vec<Vec<bool>> {
        let id = label_id(name);
        (0..h)
            .map(|y| (0..w).map(|x| m[y][x] && labels[y][x] == id).collect())
            .collect()
    };
    let parts: Vec<PartDef> = LAYOUT
        .iter()
        .map(|&(name, parent, joint, z)| PartDef {
            name,
            parent,
            joint,
            z,
            mask: label_mask(name),
        })
        .collect();

    let floor = [py_round(cx) as i64, y1];
    let pad = height;
    let canvas = [w as i64 + 2 * pad, h as i64 + 2 * pad];
    let pivot = [floor[0] + pad, floor[1] + pad];

    let contact = |name: &str, target: [f64; 2], bottom: bool| -> Result<[i64; 2], String> {
        contact_mask(&label_mask(name), target, bottom)
    };
    let grip = contact("hand_f", joints["wrist_f"], false)?;
    let off_hand = contact("hand_b", joints["wrist_b"], false)?;
    let head_p = contact("head", [cx, neck], true)?;
    let foot_part = if (0..h).any(|y| (0..w).any(|x| m[y][x] && labels[y][x] == label_id("foot_f"))) {
        "foot_f"
    } else {
        "foot_b"
    };
    let feet = contact(foot_part, [floor[0] as f64, floor[1] as f64], true)?;
    let back_part = if (0..h).any(|y| (0..w).any(|x| m[y][x] && labels[y][x] == label_id("back"))) {
        "back"
    } else {
        "torso"
    };
    let back_p = contact(back_part, [cx, (neck + hip) / 2.0], false)?;
    let tip_y = round_n(grip[1] as f64 + height as f64 * 0.45, 3);

    Ok(Fitted {
        src_w: w,
        src_h: h,
        canvas,
        pivot,
        origin: floor,
        joints,
        parts,
        seam_radius: if height < 32 { 1 } else { 2 },
        anchors: vec![
            ("grip", "hand_f".to_owned(), [grip[0] as f64, grip[1] as f64]),
            ("off_hand", "hand_b".to_owned(), [off_hand[0] as f64, off_hand[1] as f64]),
            ("head", "head".to_owned(), [head_p[0] as f64, head_p[1] as f64]),
            ("back", back_part.to_owned(), [back_p[0] as f64, back_p[1] as f64]),
            ("feet", foot_part.to_owned(), [feet[0] as f64, feet[1] as f64]),
            ("tip", "hand_f".to_owned(), [grip[0] as f64, tip_y]),
        ],
        clips: defaults(height as f64),
    })
}

// ---------------------------------------------------------------------------
// build
// ---------------------------------------------------------------------------

type Transform = (Mat, [f64; 2]);

fn part_index(fit: &Fitted, name: &str) -> Option<usize> {
    fit.parts.iter().position(|p| p.name == name)
}

fn resolve_transforms(fit: &Fitted, pose: &Pose) -> (BTreeMap<String, Transform>, Transform) {
    let root = rotation(pose.rotation);
    let joint_root = fit.joints["root"];
    let origin = [fit.origin[0] as f64, fit.origin[1] as f64];
    let delta = [
        fit.pivot[0] as f64 - origin[0] + pose.offset[0],
        fit.pivot[1] as f64 - origin[1] + pose.offset[1],
    ];
    let base = (
        root,
        [
            joint_root[0] + delta[0] - mat_vec(root, joint_root)[0],
            joint_root[1] + delta[1] - mat_vec(root, joint_root)[1],
        ],
    );

    fn resolve(
        fit: &Fitted,
        pose: &Pose,
        name: &str,
        base: &Transform,
        out: &mut BTreeMap<String, Transform>,
    ) -> Transform {
        if let Some(t) = out.get(name) {
            return *t;
        }
        let idx = part_index(fit, name).unwrap();
        let part = &fit.parts[idx];
        let (pr, pt) = match part.parent {
            Some(parent) => resolve(fit, pose, parent, base, out),
            None => *base,
        };
        let joint = fit.joints[part.joint];
        let r = mat_mul(pr, rotation(pose.angle(name)));
        let pj = mat_vec(pr, joint);
        let rj = mat_vec(r, joint);
        let t = (
            r,
            [
                pj[0] + pt[0] - rj[0],
                pj[1] + pt[1] - rj[1],
            ],
        );
        out.insert(name.to_owned(), t);
        t
    }

    let mut out = BTreeMap::new();
    for part in &fit.parts {
        resolve(fit, pose, part.name, &base, &mut out);
    }
    (out, base)
}

fn textures(fit: &Fitted, ink: &Grid) -> Result<Vec<Grid>, String> {
    let h = fit.src_h;
    let w = fit.src_w;
    let mut covered = vec![vec![false; w]; h];
    for part in &fit.parts {
        for y in 0..h {
            for x in 0..w {
                covered[y][x] |= part.mask[y][x];
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            if ink[y][x] != '.' && !covered[y][x] {
                return Err("parts: masks leave visible source pixels unassigned".to_owned());
            }
        }
    }
    let masks: Vec<&Vec<Vec<bool>>> = fit.parts.iter().map(|p| &p.mask).collect();
    let idx: BTreeMap<&str, usize> = fit
        .parts
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name, i))
        .collect();
    let mut out = Vec::new();
    for part in fit.parts.iter() {
        let mut mask = part.mask.clone();
        if let Some(parent) = part.parent {
            if fit.seam_radius != 0 {
                let [jx, jy] = fit.joints[part.joint];
                let sr2 = (fit.seam_radius * fit.seam_radius) as f64;
                let parent_mask = masks[idx[parent]];
                for y in 0..h {
                    for x in 0..w {
                        let d = (x as f64 - jx) * (x as f64 - jx) + (y as f64 - jy) * (y as f64 - jy);
                        if d <= sr2 && parent_mask[y][x] {
                            mask[y][x] = true;
                        }
                    }
                }
            }
        }
        out.push(
            (0..h)
                .map(|y| (0..w).map(|x| if mask[y][x] { ink[y][x] } else { '.' }).collect())
                .collect(),
        );
    }
    Ok(out)
}

fn stamp(out: &mut Grid, texture: &Grid, tr: &Transform) -> Result<(), String> {
    let th = texture.len();
    let tw = texture[0].len();
    let oh = out.len();
    let ow = out[0].len();
    let mut minx = i64::MAX;
    let mut maxx = i64::MIN;
    let mut miny = i64::MAX;
    let mut maxy = i64::MIN;
    let mut any = false;
    for y in 0..th {
        for x in 0..tw {
            if texture[y][x] != '.' {
                any = true;
                minx = minx.min(x as i64);
                maxx = maxx.max(x as i64);
                miny = miny.min(y as i64);
                maxy = maxy.max(y as i64);
            }
        }
    }
    if !any {
        return Ok(());
    }
    let (r, t) = *tr;
    let corners = [
        [minx as f64 - 0.5, miny as f64 - 0.5],
        [maxx as f64 + 0.5, miny as f64 - 0.5],
        [minx as f64 - 0.5, maxy as f64 + 0.5],
        [maxx as f64 + 0.5, maxy as f64 + 0.5],
    ];
    let mut lo = [f64::MAX, f64::MAX];
    let mut hi = [f64::MIN, f64::MIN];
    for c in corners {
        let mut p = mat_vec(r, c);
        p[0] += t[0];
        p[1] += t[1];
        lo[0] = lo[0].min(p[0]);
        lo[1] = lo[1].min(p[1]);
        hi[0] = hi[0].max(p[0]);
        hi[1] = hi[1].max(p[1]);
    }
    let x0 = lo[0].floor() as i64;
    let y0 = lo[1].floor() as i64;
    let x1 = hi[0].ceil() as i64 + 1;
    let y1 = hi[1].ceil() as i64 + 1;
    if (x1 - x0).max(y1 - y0) > 4 * MAX_SIDE {
        return Err("render: transformed part exceeds supported bounds".to_owned());
    }
    for y in y0..y1 {
        for x in x0..x1 {
            let v = [x as f64 - t[0], y as f64 - t[1]];
            // NumPy `(xy-t)@r` contracts the second product into the first with
            // an fma, then adds one half. Fusing the half into that fma is one ulp
            // high when the second product is tiny.
            let sx = (r[1][0].mul_add(v[1], v[0] * r[0][0]) + 0.5).floor() as i64;
            let sy = (r[1][1].mul_add(v[1], v[0] * r[0][1]) + 0.5).floor() as i64;
            if sx < 0 || sy < 0 || sx >= tw as i64 || sy >= th as i64 {
                continue;
            }
            let sample = texture[sy as usize][sx as usize];
            if sample == '.' {
                continue;
            }
            if x < 0 || y < 0 || x >= ow as i64 || y >= oh as i64 {
                return Err("render: canvas clips a part; enlarge canvas/reposition pivot in rig.json".to_owned());
            }
            out[y as usize][x as usize] = sample;
        }
    }
    Ok(())
}

fn ground(fit: &Fitted, transforms: &mut BTreeMap<String, Transform>, base: &mut Transform, texts: &[Grid]) {
    let mut bottom = f64::NEG_INFINITY;
    for (i, texture) in texts.iter().enumerate() {
        let th = texture.len();
        let tw = texture[0].len();
        let name = fit.parts[i].name;
        let (r, t) = transforms[name];
        for y in 0..th {
            for x in 0..tw {
                if texture[y][x] != '.' {
                    let p = mat_vec(r, [x as f64, y as f64]);
                    bottom = bottom.max(p[1] + t[1]);
                }
            }
        }
    }
    let delta = [0.0, fit.pivot[1] as f64 - (bottom + 0.5).floor()];
    for (_, t) in transforms.iter_mut() {
        t.1[1] += delta[1];
    }
    base.1[1] += delta[1];
}

fn anchors_of(
    fit: &Fitted,
    transforms: &BTreeMap<String, Transform>,
    base: &Transform,
    contacts: &BTreeMap<String, Vec<Vec<bool>>>,
) -> Result<BTreeMap<&'static str, [i64; 2]>, String> {
    let mut out: BTreeMap<&'static str, [i64; 2]> = BTreeMap::new();
    for (name, part, point) in &fit.anchors {
        let name: &'static str = name;
        let tr = transforms.get(part).copied().unwrap_or(*base);
        let rotated = mat_vec(tr.0, *point);
        let target = [rotated[0] + tr.1[0], rotated[1] + tr.1[1]];
        if name == "tip" {
            out.insert(name, rounded_target(target));
        } else if let Some(mask) = contacts.get(part) {
            out.insert(name, contact_mask(mask, target, name == "feet")?);
        } else {
            out.insert(name, rounded_target(target));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Pillow 13 line rasterization (libImaging/Draw.c), for smears.
// ---------------------------------------------------------------------------

struct Edge {
    x0: i64,
    y0: i64,
    xmin: i64,
    xmax: i64,
    ymin: i64,
    ymax: i64,
    dx: f32,
}

fn add_edge(x0: i64, y0: i64, x1: i64, y1: i64) -> Edge {
    let (xmin, xmax) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (ymin, ymax) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    let dx = if y0 == y1 {
        0.0
    } else {
        ((x1 - x0) as f32) / ((y1 - y0) as f32)
    };
    Edge { x0, y0, xmin, xmax, ymin, ymax, dx }
}

fn round_away_f32(v: f32) -> f32 {
    if v >= 0.0 {
        (v + 0.5).floor()
    } else {
        (v - 0.5).ceil()
    }
}

fn round_up_f64(v: f64) -> i64 {
    if v >= 0.0 {
        (v + 0.5).floor() as i64
    } else {
        -((v.abs() + 0.5).floor() as i64)
    }
}

fn round_down_f64(v: f64) -> i64 {
    if v >= 0.0 {
        (v - 0.5).ceil() as i64
    } else {
        -((v.abs() - 0.5).ceil() as i64)
    }
}

fn round_up_f32(v: f32) -> i64 {
    if v >= 0.0 {
        (v + 0.5).floor() as i64
    } else {
        -((v.abs() + 0.5).floor() as i64)
    }
}

fn round_down_f32(v: f32) -> i64 {
    if v >= 0.0 {
        (v - 0.5).ceil() as i64
    } else {
        -((v.abs() - 0.5).ceil() as i64)
    }
}

fn mask_point(mask: &mut [Vec<bool>], x: i64, y: i64) {
    let h = mask.len() as i64;
    let w = mask[0].len() as i64;
    if x >= 0 && x < w && y >= 0 && y < h {
        mask[y as usize][x as usize] = true;
    }
}

fn mask_hline(mask: &mut [Vec<bool>], mut x0: i64, y: i64, mut x1: i64) {
    let h = mask.len() as i64;
    let w = mask[0].len() as i64;
    if y >= 0 && y < h {
        if x0 < 0 {
            x0 = 0;
        } else if x0 >= w {
            return;
        }
        if x1 < 0 {
            return;
        } else if x1 >= w {
            x1 = w - 1;
        }
        if x0 <= x1 {
            for x in x0..=x1 {
                mask[y as usize][x as usize] = true;
            }
        }
    }
}

/// Pillow's `line()` (width 1, Bresenham with the horizontal special case).
fn draw_line(mask: &mut [Vec<bool>], x0: i64, mut y0: i64, x1: i64, y1: i64) {
    let dy = y1 - y0;
    let ys;
    if dy < 0 {
        ys = -1;
    } else if dy == 0 {
        if x1 > x0 {
            mask_hline(mask, x0, y0, x1 - 1);
        } else if x0 > x1 {
            mask_hline(mask, x1 + 1, y0, x0);
        }
        return;
    } else {
        ys = 1;
    }
    let dx = x1 - x0;
    let (adx, xs) = if dx < 0 { (-dx, -1) } else { (dx, 1) };
    let mut adx = adx;
    let mut ady = dy.abs();
    let mut x0 = x0;
    if adx == 0 {
        for _ in 0..ady {
            mask_point(mask, x0, y0);
            y0 += ys;
        }
    } else if adx > ady {
        let n = adx;
        ady *= 2;
        let mut e = ady - adx;
        adx *= 2;
        for _ in 0..n {
            mask_point(mask, x0, y0);
            if e >= 0 {
                y0 += ys;
                e -= adx;
            }
            e += ady;
            x0 += xs;
        }
    } else {
        let n = ady;
        adx *= 2;
        let mut e = adx - ady;
        ady *= 2;
        for _ in 0..n {
            mask_point(mask, x0, y0);
            if e >= 0 {
                x0 += xs;
                e -= ady;
            }
            e += adx;
            y0 += ys;
        }
    }
}

/// Pillow's `polygon_generic`, non-alpha branch (mode L, no mask).
fn polygon_fill(mask: &mut [Vec<bool>], edges: Vec<Edge>) {
    let h = mask.len() as i64;
    if edges.is_empty() {
        return;
    }
    let mut ymin = h - 1;
    let mut ymax = 0i64;
    for e in &edges {
        ymin = ymin.min(e.ymin);
        ymax = ymax.max(e.ymax);
    }
    let mut table: Vec<&Edge> = Vec::new();
    for e in &edges {
        if e.ymin == e.ymax {
            mask_hline(mask, e.xmin, e.ymin, e.xmax);
            continue;
        }
        table.push(e);
    }
    if ymin < 0 {
        ymin = 0;
    }
    if ymax > h {
        ymax = h;
    }
    let mut xx: Vec<f32> = vec![0.0; table.len() * 2];
    for y in ymin..=ymax {
        let mut j = 0usize;
        for i in 0..table.len() {
            let cur = table[i];
            if y >= cur.ymin && y <= cur.ymax {
                xx[j] = ((y - cur.y0) as f32) * cur.dx + cur.x0 as f32;
                j += 1;
                if y == cur.ymax && y < ymax {
                    xx[j] = xx[j - 1];
                    j += 1;
                } else if (y == cur.ymin || y == cur.ymax) && cur.dx != 0.0 {
                    for k in 0..i {
                        let other = table[k];
                        if (y != other.ymin && y != other.ymax) || other.dx == 0.0 {
                            continue;
                        }
                        let offset = if y == cur.ymax { -1 } else { 1 };
                        let adj = ((y + offset - cur.y0) as f32) * cur.dx + cur.x0 as f32;
                        if y + offset >= other.ymin && y + offset <= other.ymax {
                            let adj_other = ((y + offset - other.y0) as f32) * other.dx + other.x0 as f32;
                            if xx[j - 1] > adj + 1.0 && xx[j - 1] > adj_other + 1.0 {
                                xx[j - 1] = round_away_f32(adj.max(adj_other)) + 1.0;
                            } else if xx[j - 1] < adj - 1.0 && xx[j - 1] < adj_other - 1.0 {
                                xx[j - 1] = round_away_f32(adj.min(adj_other)) - 1.0;
                            }
                            break;
                        }
                    }
                }
            }
        }
        xx[..j].sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
        let mut i = 1usize;
        while i < j {
            mask_hline(mask, round_up_f32(xx[i - 1]), y, round_down_f32(xx[i]));
            i += 2;
        }
    }
}

/// Pillow's `ImagingDrawWideLine`.
fn wide_line(mask: &mut [Vec<bool>], x0: i64, y0: i64, x1: i64, y1: i64, width: i64) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    if dx == 0 && dy == 0 {
        mask_point(mask, x0, y0);
        return;
    }
    let big = (dx as f64).hypot(dy as f64);
    let small = (width as f64 - 1.0) / 2.0;
    let ratio_max = round_up_f64(small) as f64 / big;
    let ratio_min = round_down_f64(small) as f64 / big;
    let dxmin = round_down_f64(ratio_min * dy as f64);
    let dxmax = round_down_f64(ratio_max * dy as f64);
    let dymin = round_down_f64(ratio_min * dx as f64);
    let dymax = round_down_f64(ratio_max * dx as f64);
    let v = [
        (x0 - dxmin, y0 + dymax),
        (x1 - dxmin, y1 + dymax),
        (x1 + dxmax, y1 - dymin),
        (x0 + dxmax, y0 - dymin),
    ];
    let edges = vec![
        add_edge(v[0].0, v[0].1, v[1].0, v[1].1),
        add_edge(v[1].0, v[1].1, v[2].0, v[2].1),
        add_edge(v[2].0, v[2].1, v[3].0, v[3].1),
        add_edge(v[3].0, v[3].1, v[0].0, v[0].1),
    ];
    polygon_fill(mask, edges);
}

/// `ImageDraw.Draw(im).line(points, fill=255, width=width)` on an L mask.
fn pil_line(mask: &mut [Vec<bool>], points: &[[i64; 2]], width: i64) {
    if width == 1 {
        let mut last: Option<[i64; 2]> = None;
        for i in 0..points.len() - 1 {
            draw_line(mask, points[i][0], points[i][1], points[i + 1][0], points[i + 1][1]);
            last = Some(points[i + 1]);
        }
        if let Some([x, y]) = last {
            mask_point(mask, x, y);
        }
    } else {
        for i in 0..points.len() - 1 {
            wide_line(mask, points[i][0], points[i][1], points[i + 1][0], points[i + 1][1], width);
        }
    }
}

fn smear(
    out: &mut Grid,
    previous: &BTreeMap<&'static str, [i64; 2]>,
    current: &BTreeMap<&'static str, [i64; 2]>,
    ink_char: char,
    width: i64,
) -> Result<(), String> {
    let oh = out.len();
    let ow = out[0].len();
    let a = [previous["tip"][0] as f64, previous["tip"][1] as f64];
    let b = [current["tip"][0] as f64, current["tip"][1] as f64];
    let grip = [
        (previous["grip"][0] + current["grip"][0]) as f64 / 2.0,
        (previous["grip"][1] + current["grip"][1]) as f64 / 2.0,
    ];
    let norm = |v: [f64; 2]| (v[0] * v[0] + v[1] * v[1]).sqrt();
    if norm([b[0] - a[0], b[1] - a[1]]) < 1.0 {
        return Ok(());
    }
    let mut mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let radial = [mid[0] - grip[0], mid[1] - grip[1]];
    let rlen = norm(radial);
    if rlen > 0.01 {
        let radius = (norm([a[0] - grip[0], a[1] - grip[1]]) + norm([b[0] - grip[0], b[1] - grip[1]])) / 2.0;
        mid = [grip[0] + radial[0] / rlen * radius, grip[1] + radial[1] / rlen * radius];
    }
    let mut points: Vec<[i64; 2]> = Vec::new();
    for i in 0..9 {
        let f = i as f64 / 8.0;
        let px = (1.0 - f) * (1.0 - f) * a[0] + 2.0 * (1.0 - f) * f * mid[0] + f * f * b[0];
        let py = (1.0 - f) * (1.0 - f) * a[1] + 2.0 * (1.0 - f) * f * mid[1] + f * f * b[1];
        points.push([(px + 0.5).floor() as i64, (py + 0.5).floor() as i64]);
    }
    if points
        .iter()
        .any(|[x, y]| *x < 0 || *y < 0 || *x >= ow as i64 || *y >= oh as i64)
    {
        return Err("render: canvas clips swing smear; enlarge canvas".to_owned());
    }
    let mut mask = vec![vec![false; ow]; oh];
    pil_line(&mut mask, &points, width);
    for y in 0..oh {
        for x in 0..ow {
            if mask[y][x] {
                out[y][x] = ink_char;
            }
        }
    }
    Ok(())
}

pub(super) fn build_pub(fit: &Fitted, ink: &Grid) -> Result<Value, String> {
    build(fit, ink)
}

fn build(fit: &Fitted, ink: &Grid) -> Result<Value, String> {
    let texts = textures(fit, ink)?;
    let mut order: Vec<usize> = (0..fit.parts.len()).collect();
    order.sort_by(|&a, &b| (fit.parts[a].z, fit.parts[a].name).cmp(&(fit.parts[b].z, fit.parts[b].name)));

    let mut contact_parts: Vec<usize> = Vec::new();
    for (name, part, _) in &fit.anchors {
        if *name != "tip" {
            if let Some(i) = part_index(fit, part) {
                if !contact_parts.contains(&i) {
                    contact_parts.push(i);
                }
            }
        }
    }
    let contact_textures: Vec<(usize, Grid)> = contact_parts
        .iter()
        .map(|&i| {
            let mask = &fit.parts[i].mask;
            let grid: Grid = (0..fit.src_h)
                .map(|y| {
                    (0..fit.src_w)
                        .map(|x| if mask[y][x] { ink[y][x] } else { '.' })
                        .collect()
                })
                .collect();
            (i, grid)
        })
        .collect();

    // Smear ink: brightest visible source ink (r+g+b), ties to the smallest char.
    let mut visible: Vec<char> = Vec::new();
    for row in ink {
        for &c in row {
            if c != '.' && !visible.contains(&c) {
                visible.push(c);
            }
        }
    }
    visible.sort();
      // Python `max(visible, key=sum)` keeps the first maximal element.
      let smear_ink = visible
          .iter()
          .copied()
          .rev()
          .max_by(|&a, &b| {
              let sum = |c: char| {
                  let [r, g, b] = ink::ink(c).unwrap_or([0, 0, 0]);
                  r as i64 + g as i64 + b as i64
              };
              sum(a).cmp(&sum(b))
          })
          .unwrap_or('W');
    let smear_width = 1.max((fit.src_h as i64 + 8) / 16);

    let cw = fit.canvas[0] as usize;
    let ch = fit.canvas[1] as usize;
    let mut clips = Map::new();
    for (clip_name, poses) in &fit.clips {
        let mut frames: Vec<Value> = Vec::new();
        let mut previous: Option<BTreeMap<&'static str, [i64; 2]>> = None;
        for (i, pose) in poses.iter().enumerate() {
            let frame = (|| -> Result<Value, String> {
                let (mut transforms, mut base) = resolve_transforms(fit, pose);
                if *clip_name == "roll" || *clip_name == "die" {
                    ground(fit, &mut transforms, &mut base, &texts);
                }
                let mut contacts: BTreeMap<String, Vec<Vec<bool>>> = BTreeMap::new();
                for (i, text) in &contact_textures {
                    let mut layer: Grid = vec![vec!['.'; cw]; ch];
                    stamp(&mut layer, text, &transforms[fit.parts[*i].name])?;
                    contacts.insert(
                        fit.parts[*i].name.to_owned(),
                        layer.iter().map(|r| r.iter().map(|&c| c != '.').collect()).collect(),
                    );
                }
                let anchors = anchors_of(fit, &transforms, &base, &contacts)?;
                for p in anchors.values() {
                    if p[0] < 0 || p[1] < 0 || p[0] >= cw as i64 || p[1] >= ch as i64 {
                        return Err("anchor outside canvas; enlarge canvas/reposition pivot".to_owned());
                    }
                }
                let mut out: Grid = vec![vec!['.'; cw]; ch];
                if pose.smear {
                    if let Some(prev) = &previous {
                        smear(&mut out, prev, &anchors, smear_ink, smear_width)?;
                    }
                }
                for &pi in &order {
                    stamp(&mut out, &texts[pi], &transforms[fit.parts[pi].name])?;
                }
                let rows: Vec<Value> = out
                    .iter()
                    .map(|r| Value::String(r.iter().collect()))
                    .collect();
                let mut anchor_json = Map::new();
                for name in ANCHOR_NAMES {
                    anchor_json.insert(name.to_owned(), json!(anchors[name]));
                }
                Ok(json!({
                    "ticks": pose.ticks,
                    "rows": rows,
                    "anchors": Value::Object(anchor_json),
                }))
            })()
            .map_err(|e| format!("clips.{clip_name}[{i}]: {e}"))?;
            frames.push(frame);
            // Re-derive the anchors map for the next frame's smear
            // straight from the emitted JSON (authoritative integers).
            let mut prev: BTreeMap<&'static str, [i64; 2]> = BTreeMap::new();
            {
                let obj = &frames.last().unwrap()["anchors"];
                for name in ANCHOR_NAMES {
                    let p = &obj[name];
                    prev.insert(name, [p[0].as_i64().unwrap(), p[1].as_i64().unwrap()]);
                }
            }
            previous = Some(prev);
        }
        clips.insert(clip_name.to_string(), Value::Array(frames));
    }
    Ok(json!({
        "version": 1,
        "canvas": [fit.canvas[0], fit.canvas[1]],
        "pivot": [fit.pivot[0], fit.pivot[1]],
        "clips": Value::Object(clips),
    }))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

fn parse_rows(rows: &[&str]) -> Result<Grid, String> {
    if rows.is_empty() || rows[0].is_empty() {
        return Err("source: expected nonempty ink rows".to_owned());
    }
    let w = rows[0].len();
    if rows.len() as i64 > MAX_SIDE || w as i64 > MAX_SIDE {
        return Err("source: exceeds 1024 side limit".to_owned());
    }
    let mut grid = Vec::with_capacity(rows.len());
    for row in rows {
        if row.len() != w {
            return Err("source: ink rows must be rectangular".to_owned());
        }
        let mut cells = Vec::with_capacity(w);
        for c in row.chars() {
            if c != '.' && ink::ink(c).is_none() {
                return Err(format!("source: unknown ink {c:?}"));
            }
            cells.push(c);
        }
        grid.push(cells);
    }
    Ok(grid)
}

/// Ink rows -> the full game.json text (all 13 clips, Python-parity pixels).
pub fn auto_rig_json(rows: &[&str]) -> Result<String, String> {
    let grid = parse_rows(rows)?;
    let fitted = fit(&grid)?;
    let game = build(&fitted, &grid)?;
    let mut text = serde_json::to_string_pretty(&game).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

/// Ink rows -> the full game.json text using the SILHOUETTE fit
/// (falls back to the proportional fit exactly where the Python does).
pub fn auto_rig_smart_json(rows: &[&str]) -> Result<String, String> {
    let grid = parse_rows(rows)?;
    let fitted = super::smart::fit_smart(&grid)?.fit;
    let game = build_pub(&fitted, &grid)?;
    let mut text = serde_json::to_string_pretty(&game).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

/// The silhouette fit's plan for these rows: "biped" or "robe".
pub fn auto_rig_plan(rows: &[&str]) -> Result<&'static str, String> {
    let grid = parse_rows(rows)?;
    super::smart::plan_smart(&grid)
}

/// Ink rows -> a parsed [`Rig`] with all 13 clips (the game's own loader).
pub fn auto_rig(rows: &[&str]) -> Result<Rig, String> {
    let json = match auto_rig_smart_json(rows) {
        Ok(json) => json,
        Err(_) => auto_rig_json(rows)?,
    };
    Rig::parse(&json).map_err(|e| e.to_string())
}

/// The part map `auto_rig` used: smart when that fit returns Ok, else the
/// proportional cut. A standing figure has a head, a torso, and a leg, or
/// the robe plan.
pub struct FigureParts {
    pub plan: &'static str,
    pub head: bool,
    pub torso: bool,
    pub leg: bool,
}

fn mask_ink(mask: &[Vec<bool>]) -> bool {
    mask.iter().any(|row| row.iter().any(|cell| *cell))
}

fn parts_of(fit: &Fitted, plan: &'static str) -> FigureParts {
    let has = |name: &str| fit.parts.iter().any(|part| part.name == name && mask_ink(&part.mask));
    let leg = ["upper_leg_f", "upper_leg_b", "lower_leg_f", "lower_leg_b", "foot_f", "foot_b"]
        .into_iter()
        .any(has);
    FigureParts {
        plan,
        head: has("head"),
        torso: has("torso") || has("robe"),
        leg,
    }
}

pub fn figure_parts(rows: &[&str]) -> Result<FigureParts, String> {
    if let Ok(plan) = auto_rig_plan(rows) {
        if let Ok(grid) = parse_rows(rows) {
            if let Ok(fitted) = super::smart::fit_smart(&grid) {
                return Ok(parts_of(&fitted.fit, plan));
            }
        }
    }
    Ok(parts_of(&fit(&parse_rows(rows)?)?, "biped"))
}

/// The proportional fit stays callable: ink rows -> Rig (13 clips, no
/// silhouette reading).
pub fn auto_rig_proportional(rows: &[&str]) -> Result<Rig, String> {
    let json = auto_rig_json(rows)?;
    Rig::parse(&json).map_err(|e| e.to_string())
}

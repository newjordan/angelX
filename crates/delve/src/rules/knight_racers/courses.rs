//! The authored courses. `build` makes one on first use.
use super::course::*;
use std::f32::consts::{PI, TAU};

pub fn build(id: CourseId) -> Course {
    match id {
        CourseId::Proving => proving(CourseId::Proving),
        CourseId::Emberfall => super::emberfall::build(id),
        CourseId::Glassway => super::glassway::build(id),
        CourseId::Moatwalk => super::moatwalk::build(id),
    }
}

// ---- Course 0: the old Lanternward circuit --------------------------------

#[derive(Clone, Copy)]
struct Seg { len: f32, kappa: f32, wall: f32, kind: Stretch, name: &'static str }

fn arc(radius: f32, deg: f32) -> f32 { radius * deg * PI / 180. }

fn advance(th: f32, len: f32, kappa: f32) -> (f32, f32, f32) {
    if kappa.abs() < 1e-8 {
        (len * th.cos(), len * th.sin(), th)
    } else {
        let dth = kappa * len;
        let dx_l = dth.sin() / kappa;
        let dy_l = (1. - dth.cos()) / kappa;
        (dx_l * th.cos() - dy_l * th.sin(), dx_l * th.sin() + dy_l * th.cos(), th + dth)
    }
}

fn walk(segs: &[Seg]) -> (f32, f32, f32) {
    let (mut x, mut y, mut th) = (0., 0., 0.);
    for seg in segs {
        let (dx, dy, nth) = advance(th, seg.len, seg.kappa);
        x += dx; y += dy; th = nth;
    }
    (x, y, th)
}

/// Close the prefix with one straight and one constant-radius sweeper.
/// Heading is already fixed by the sweeper's angle, so length and radius
/// are the two unknowns that bring the centerline back to the origin.
fn solve_close(prefix: &[Seg], right: bool) -> Option<(f32, f32, f32)> {
    let (x, y, th) = walk(prefix);
    let thn = th.rem_euclid(TAU);
    let (ang, unit) = if right { (thn, -1.) } else { ((TAU - thn).rem_euclid(TAU), 1.) };
    if ang < 0.35 { return None; }
    let (ux, uy, _) = advance(th, ang, unit);
    let (dx, dy) = (th.cos(), th.sin());
    let det = dx * uy - dy * ux;
    if det.abs() < 1e-5 { return None; }
    let straight = ((-x) * uy - ux * (-y)) / det;
    let radius = (dx * (-y) - (-x) * dy) / det;
    if !(40. ..900.).contains(&straight) || !(28. ..340.).contains(&radius) { return None; }
    Some((straight, radius, ang))
}

fn shape_of(kind: Stretch) -> Shape {
    match kind { Stretch::Open | Stretch::Bowl => Shape::OPEN, Stretch::Canyon => Shape::CANYON, Stretch::Tunnel => Shape::TUNNEL }
}

fn proving(id: CourseId) -> Course {
    let seg = |len, kappa, wall, kind, name| Seg { len, kappa, wall, kind, name };
    let prefix = vec![
        seg(560., 0., 12.8, Stretch::Open, "KEEPWARD STRAIGHT"),
        seg(arc(110., 85.), 1. / 110., 11.0, Stretch::Canyon, "SUN BEND"),
        seg(100., 0., 11.2, Stretch::Open, "RIDGE"),
        seg(arc(36., 150.), 1. / 36., 8.15, Stretch::Canyon, "THE NEEDLE"),
        seg(80., 0., 7.9, Stretch::Tunnel, "CAVERN"),
        seg(arc(52., 90.), -1. / 52., 9.3, Stretch::Canyon, "THE CUT"),
        seg(120., 0., 11.0, Stretch::Open, "SLAG RUN"),
        seg(arc(32., 140.), 1. / 32., 7.9, Stretch::Canyon, "SKULL TURN"),
        seg(160., 0., 12.2, Stretch::Open, "RUINS"),
        seg(arc(70., 45.), -1. / 70., 10.4, Stretch::Canyon, "GATE BEND"),
    ];
    let (straight, radius, ang) = solve_close(&prefix, true)
        .or_else(|| solve_close(&prefix, false))
        .expect("proving circuit must close");
    let right = {
        let (_, _, th) = walk(&prefix);
        (th.rem_euclid(TAU) - ang).abs() < 0.02
    };
    let kappa = if right { -1. / radius } else { 1. / radius };
    let mut segs = prefix;
    segs.push(seg(straight, 0., 12.8, Stretch::Open, "RETURN"));
    segs.push(seg(radius * ang, kappa, 12.0, Stretch::Bowl, "THE BOWL"));
    let (ex, ey, eth) = walk(&segs);
    assert!(ex.hypot(ey) < 0.35 && eth.rem_euclid(TAU).min(TAU - eth.rem_euclid(TAU)) < 0.02, "circuit seam {ex},{ey} heading {eth}");
    let starts: Vec<f32> = segs.iter().scan(0., |acc, s| { let here = *acc; *acc += s.len; Some(here) }).collect();
    let at = |name: &str| starts[segs.iter().position(|s| s.name == name).unwrap()];
    let lap: f32 = segs.iter().map(|s| s.len).sum();

    // Half-width of the racing surface: each section's wall, blended over 18 m at the joins.
    let wall = |s: f32| -> f32 {
        let s = s.rem_euclid(lap);
        let mut acc = 0.;
        for (i, seg) in segs.iter().enumerate() {
            if s < acc + seg.len || i + 1 == segs.len() {
                let prev = if i == 0 { segs.len() - 1 } else { i - 1 };
                let next = (i + 1) % segs.len();
                let (into, out, blend) = (s - acc, seg.len - (s - acc), 18.);
                if into < blend && into <= out {
                    return segs[prev].wall + (seg.wall - segs[prev].wall) * (into / blend).clamp(0., 1.);
                }
                if out < blend {
                    return seg.wall + (segs[next].wall - seg.wall) * (1. - out / blend).clamp(0., 1.);
                }
                return seg.wall;
            }
            acc += seg.len;
        }
        12.
    };
    // Road height closes on itself so the lap seam is level.
    let height = |s: f32| -> f32 {
        let u = s.rem_euclid(lap);
        let wave = 3.4 * (u / lap * TAU).sin() + 1.3 * (u / lap * TAU * 2.).sin();
        let drop_in = ((u - 800.) / 55.).clamp(0., 1.);
        let drop_out = (1. - (u - 1000.) / 70.).clamp(0., 1.);
        wave - 4.4 * drop_in * drop_out
    };
    let pose = |s: f32| -> (f32, f32, f32, f32) {
        let s = s.rem_euclid(lap);
        let (mut x, mut y, mut th, mut left) = (0., 0., 0., s);
        for seg in &segs {
            if left <= seg.len {
                let (dx, dy, nth) = advance(th, left, seg.kappa);
                return (x + dx, y + dy, nth, seg.kappa);
            }
            let (dx, dy, nth) = advance(th, seg.len, seg.kappa);
            x += dx; y += dy; th = nth; left -= seg.len;
        }
        (x, y, th, 0.)
    };
    let n = lap.round() as usize;
    let step = lap / n as f32;
    let pts: Vec<Sample> = (0..n).map(|i| {
        let s = i as f32 * step;
        let (x, y, th, kappa) = pose(s);
        let w = wall(s);
        // The first renderer banked by the bend; keep that, now as the road's own roll.
        Sample { x, y, th, k: kappa, z: height(s), g: 0., bank: (kappa * 18.).clamp(-0.28, 0.28), wl: w, wr: w }
    }).collect();
    let (_, _, end_th, _) = pose(lap - 1e-3);
    let line = Line::from_samples(pts, lap, (end_th / TAU).round() * TAU);

    let mut zones = Vec::new();
    for (i, sg) in segs.iter().enumerate() {
        let biome = match sg.kind { Stretch::Tunnel => Biome::Cavern, Stretch::Canyon => Biome::Forge, _ => Biome::Proving };
        zones.push(Zone { s0: starts[i], s1: starts[i] + sg.len, biome, stretch: sg.kind, name: sg.name, shape: shape_of(sg.kind) });
    }
    let slag = |s: f32, y: f32, hl: f32, hw: f32| Hazard::block(HazardKind::Slag, s, y, hl, hw);
    let hazards = vec![
        slag(at("KEEPWARD STRAIGHT") + 170., 7.1, 2.3, 1.25),
        slag(at("KEEPWARD STRAIGHT") + 330., -7.0, 2.3, 1.25),
        slag(at("KEEPWARD STRAIGHT") + 470., 6.6, 2.1, 1.15),
        slag(at("SLAG RUN") + 48., -5.6, 2.0, 1.1),
        slag(at("RUINS") + 55., 6.4, 2.2, 1.2),
        slag(at("RUINS") + 115., -6.2, 2.2, 1.2),
        slag(at("RETURN") + 80., 7.2, 2.3, 1.25),
        slag(at("RETURN") + straight * 0.55, -6.8, 2.3, 1.25),
    ];
    let strips = vec![
        Strip { s0: at("KEEPWARD STRAIGHT") + 60., s1: at("KEEPWARD STRAIGHT") + 150., half_w: 3.1, y: 0. },
        Strip { s0: at("RUINS") + 18., s1: at("RUINS") + 78., half_w: 2.8, y: 0. },
        Strip { s0: at("RETURN") + 24., s1: at("RETURN") + 120., half_w: 3.2, y: 0. },
    ];
    let piece = |kind, s0, s1, side, seed| Piece { kind, s0, s1, side, seed };
    let pieces = vec![
        piece(PieceKind::Grandstand, 20., 140., -1., 3),
        piece(PieceKind::Tunnel, at("CAVERN"), at("CAVERN") + 80., 0., 1),
        piece(PieceKind::Foundry, at("SLAG RUN"), at("SLAG RUN") + 120., 1., 2),
        piece(PieceKind::Arch, at("THE BOWL"), at("THE BOWL") + 6., 0., 4),
    ];
    let landmarks = starts.iter().skip(1).copied().collect();
    Course::new(Spec {
        id, name: "LANTERNWARD PROVING RUN",
        blurb: "The old circuit: one long straight, a cavern and a bowl. The short practice run.",
        laps: 2, line, zones, hazards, strips, pieces, forks: Vec::new(), gaps: Vec::new(), landmarks,
    })
}

//! What moves with the race clock: timed hazards. Jets burn only while live
//! and glow as they near their strike; rockfalls drop and settle. Built fresh
//! each frame, for the few in range.
use super::geo::{Fr, Soup};
use super::noise::hash;
use super::palette::*;
use super::route::{Hazard, HazardKind, Route, Strip};
use super::frame::row_on;
use super::look::style;
use crate::render::world3d::math::v3;
use crate::render::world3d::mesh::Tri;

/// The same telegraph the sim reads (`rules::Hazard::warn`/`live`).
fn live(h: &Hazard, tick: u32) -> bool { h.period == 0 || (tick + h.phase) % h.period < h.on }
fn warn(h: &Hazard, tick: u32) -> f32 {
    if h.period == 0 { return 1.; }
    let at = (tick + h.phase) % h.period;
    if at < h.on { 1. } else { 1. - ((h.period - at) as f32 / 36.).min(1.) }
}

pub fn hazards(out: &mut Vec<Tri>, r: &dyn Route, s: f32, tick: u32) {
    let lap = r.lap();
    let here = s.rem_euclid(lap);
    let mut g = Soup::new();
    let mut each = |h: &Hazard, line: u8| {
        if h.period == 0 { return; }
        let d = (h.s - here + lap * 1.5).rem_euclid(lap) - lap * 0.5;
        if !(-60. ..=340.).contains(&d) { return; }
        let rw = super::frame::row_on(r, line, h.s);
        let fr = Fr { o: rw.pt(h.y, 0.), f: v3(rw.cs, rw.sn, 0.), l: v3(-rw.sn, rw.cs, 0.) };
        let (on, w) = (live(h, tick), warn(h, tick));
        match h.kind {
            HazardKind::Jet => jet(&mut g, &fr, h, on, w, tick),
            HazardKind::Rockfall => rockfall(&mut g, &fr, h, on, w, style(r.zone(h.s).0).rock, tick),
            _ => {}
        }
    };
    for h in r.hazards() { each(&h, 0); }
    for (i, f) in r.forks().iter().enumerate() { for h in &f.hazards { each(h, i as u8 + 1); } }
    out.extend(g.t);
}

fn jet(g: &mut Soup, fr: &Fr, h: &Hazard, on: bool, warn: f32, tick: u32) {
    let flick = 0.85 + 0.15 * ((tick as f32 * 0.9 + h.s).sin());
    let (hl, hw) = (h.half_l, h.half_w);
    if on {
        // A column of fire: a wide hot base and a tall tongue.
        let top = 7.5 * flick;
        let c = fr.pt(0., 0., 0.35);
        g.spike(c, c + v3(0., 0., top), hw.min(hl) * 0.95, LAVA);
        g.spike(c, c + v3(0., 0., top * 0.75), hw.min(hl) * 0.55, LAVA_HOT);
        g.bx(fr, (-hl * 0.8, hl * 0.8), (-hw * 0.8, hw * 0.8), (0.5, 0.9), LAVA_HOT);
    } else if warn > 0.05 {
        // A flicker of flame at the vent, taller as the strike nears.
        let c = fr.pt(0., 0., 0.35);
        g.spike(c, c + v3(0., 0., 0.4 + 2.2 * warn * warn * flick), hw.min(hl) * (0.3 + 0.4 * warn), LAVA);
    }
}

fn rockfall(g: &mut Soup, fr: &Fr, h: &Hazard, on: bool, warn: f32, rock: u8, tick: u32) {
    let (hl, hw) = (h.half_l, h.half_w);
    let n = 5;
    for i in 0..n {
        let (a, l) = ((hash(i, 1, h.phase) - 0.5) * 2. * hl * 0.8, (hash(i, 2, h.phase) - 0.5) * 2. * hw * 0.8);
        let r = 0.7 + 0.8 * hash(i, 3, h.phase);
        if on {
            // Settled: a heap of boulders on the road.
            let base = fr.pt(a, l, 0.04);
            g.spike(base, base + v3(0.3, 0., 1.5 + r), r * 1.1, rock);
        } else if warn > 0.12 {
            // Falling: from the lip of the canyon, quicker as it nears.
            let fall = 1. - warn;
            let z = 0.5 + 16. * fall * fall * (0.8 + 0.4 * hash(i, 4, h.phase));
            let base = fr.pt(a, l, z);
            let wob = ((tick as f32) * 0.4 + i as f32).sin() * 0.2;
            g.spike(base + v3(wob, 0., 0.), base + v3(wob, 0., r * 1.6), r * 0.6, rock);
        }
    }
}

/// Boost strips, built fresh each frame for the stretch in view: a gold bed with chevrons down it,
/// and a run of white-hot ones travelling the way the pods go, so a strip reads from far off.
pub fn strips(out: &mut Vec<Tri>, r: &dyn Route, s: f32, tick: u32) {
    let lap = r.lap();
    let here = s.rem_euclid(lap);
    let mut g = Soup::new();
    let mut one = |p: &Strip, line: u8| {
        for o in [-lap, 0., lap] {
            let (a, b) = ((p.s0 + o).max(here - 40.), (p.s1 + o).min(here + 300.));
            if a < b { strip(&mut g, r, line, a - o, b - o, here - o, p, tick); }
        }
    };
    for p in r.strips() { one(&p, 0); }
    for (i, f) in r.forks().iter().enumerate() { for p in &f.strips { one(p, i as u8 + 1); } }
    out.extend(g.t);
}

fn strip(g: &mut Soup, r: &dyn Route, line: u8, s0: f32, s1: f32, here: f32, p: &Strip, tick: u32) {
    let (half_w, y) = (p.half_w, p.y);
    let n = ((s1 - s0) / 3.).ceil() as i32;
    for k in 0..n {
        let (a, b) = (s0 + k as f32 * 3., (s0 + (k + 1) as f32 * 3.).min(s1));
        // Far down the road a step is two: the chevrons are specks there.
        if (a - here) > 120. && k % 2 == 1 { continue; }
        let (ra, rb) = (row_on(r, line, a), row_on(r, line, if (a - here) > 120. { b + 3. } else { b }));
        g.quad(ra.pt(y - half_w, 0.05), rb.pt(y - half_w, 0.05), rb.pt(y + half_w, 0.05), ra.pt(y + half_w, 0.05), LAVA_BRIGHT);
        // The run of light: every fourth chevron burns white, and the run moves on three steps a second.
        let lit = ((a / 3.).round() as i64 - (tick / 3) as i64).rem_euclid(4) == 0;
        let (rm, rt) = (row_on(r, line, a + 0.4), row_on(r, line, a + 2.5));
        // Bright rails down both edges, near ones.
        if (a - here) < 120. {
            for sd in [-1.0f32, 1.0] {
                let l = y + sd * half_w;
                g.quad(ra.pt(l - 0.1, 0.07), rb.pt(l - 0.1, 0.07), rb.pt(l + 0.1, 0.07), ra.pt(l + 0.1, 0.07), SPARK);
            }
        }
        let w = half_w * 0.62;
        let m = if lit { SPARK } else { HAZARD_GOLD };
        g.tri(rm.pt(y - w, 0.08), rm.pt(y - w * 0.5, 0.08), rt.pt(y, 0.08), m);
        g.tri(rm.pt(y + w, 0.08), rm.pt(y + w * 0.5, 0.08), rt.pt(y, 0.08), m);
    }
}

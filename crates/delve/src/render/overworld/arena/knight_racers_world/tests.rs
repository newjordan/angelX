//! The world renderer's guarantees: bounded geometry per frame, built once,
//! finite everywhere, the same every time.
use super::route::{self, Route};
use super::{TRIANGLE_BUDGET, Tally, build_at as build, gather};
use crate::rules::knight_racers as rules;

fn every_course() -> Vec<&'static dyn Route> {
    [rules::CourseId::Proving, rules::CourseId::Emberfall, rules::CourseId::Glassway, rules::CourseId::Moatwalk].into_iter().map(route::on).collect()
}

/// Twelve points around a lap, and two laps on.
fn around(r: &dyn Route) -> Vec<f32> { (0..12).map(|k| r.lap() * k as f32 / 12. + 3.).collect() }

#[test]
fn a_frame_of_world_stays_inside_the_triangle_budget_on_every_course() {
    for r in every_course() {
        let mut worst = (0, Tally::default());
        for s in around(r) {
            let mut tally = Tally::default();
            let n = gather(r, s, 0, &mut tally).tris.len();
            if n > worst.0 { worst = (n, tally); }
            assert!(n <= TRIANGLE_BUDGET, "course {} at s={s}: {n} triangles, budget {TRIANGLE_BUDGET}", r.key());
            assert!(n > 2_000, "course {} at s={s}: only {n} triangles, the world is missing", r.key());
        }
        // Laps repeat: the second lap is the first lap's geometry, not more of it.
        let (a, b) = (build(r, 100., 0).tris.len(), build(r, 100. + r.lap(), 0).tris.len());
        assert_eq!(a, b, "lap two draws a different amount of world than lap one");
        eprintln!("course {} worst frame {} triangles: {:?}", r.key(), worst.0, worst.1);
    }
}

#[test]
fn every_vertex_is_finite_and_the_world_is_deterministic() {
    for r in every_course() {
        for s in around(r).into_iter().step_by(3) {
            let (a, b) = (build(r, s, 0), build(r, s, 0));
            assert_eq!(a.tris.len(), b.tris.len());
            for (x, y) in a.tris.iter().zip(&b.tris) {
                for (p, q) in x.v.iter().zip(&y.v) {
                    assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite(), "course {} s={s}: non-finite vertex", r.key());
                    assert_eq!((p.x.to_bits(), p.y.to_bits(), p.z.to_bits()), (q.x.to_bits(), q.y.to_bits(), q.z.to_bits()));
                }
                assert!(x.normal.x.is_finite() && x.normal.y.is_finite() && x.normal.z.is_finite());
            }
        }
    }
}

/// A course under a key of its own, so other tests' frames never touch its chunks.
struct Private<'a>(&'a dyn Route);
impl Route for Private<'_> {
    fn key(&self) -> u32 { 7_001 }
    fn lap(&self) -> f32 { self.0.lap() }
    fn pose(&self, s: f32) -> (f32, f32, f32) { self.0.pose(s) }
    fn height(&self, s: f32) -> f32 { self.0.height(s) }
    fn width(&self, s: f32) -> (f32, f32) { self.0.width(s) }
    fn zone(&self, s: f32) -> (route::Look, &'static str) { self.0.zone(s) }
    fn sink(&self, s: f32) -> f32 { self.0.sink(s) }
    fn ground(&self, x: f32, y: f32) -> f32 { self.0.ground(x, y) }
    fn curve(&self, s: f32) -> f32 { self.0.curve(s) }
    fn zones(&self) -> Vec<(f32, f32, route::Look, &'static str)> { self.0.zones() }
    fn pieces(&self) -> &[route::Piece] { self.0.pieces() }
    fn forks(&self) -> &[route::ForkInfo] { self.0.forks() }
    fn frame(&self, line: u8, s: f32) -> route::RoadFrame { self.0.frame(line, s) }
    fn hazards(&self) -> Vec<route::Hazard> { self.0.hazards() }
    fn strips(&self) -> Vec<route::Strip> { self.0.strips() }
    fn gaps(&self) -> Vec<route::Gap> { self.0.gaps() }
    fn landmarks(&self) -> Vec<f32> { self.0.landmarks() }
}

#[test]
fn a_frame_gathers_chunks_it_does_not_rebuild_them() {
    let r = Private(route::on(rules::CourseId::Proving));
    let first = build(&r, 400., 0).tris.len();
    let before = super::cache::built_for(7_001);
    assert!(before > 20, "the first frame built only {before} chunks");
    // Two metres on is the next frame at speed: no more than a chunk or two new. The cache is the
    // process's, and a test beside this one may purge it (it drops its oldest half when full), so a
    // step that finds the cache emptied under it is stepped again, up to three times.
    let (mut at, mut grown) = (402., usize::MAX);
    for _ in 0..3 {
        let was = super::cache::built_for(7_001);
        let _ = build(&r, at, 0);
        grown = super::cache::built_for(7_001) - was;
        if grown <= 12 { break; }
        at += 2.;
    }
    assert!(grown <= 12, "{grown} new chunks for a two-metre step");
    let again = super::cache::built_for(7_001);
    let n = build(&r, at, 0).tris.len();
    assert_eq!(super::cache::built_for(7_001), again, "the same frame built something new");
    assert!(n.abs_diff(first) < first / 4, "a two-metre step changed the frame from {first} to {n} triangles");
}

#[test]
fn every_set_piece_builds_into_real_geometry() {
    for r in every_course() {
        let lines = 1 + r.forks().len();
        for line in 0..lines as u8 {
            for (i, p) in super::pieces::list(r, line).iter().enumerate() {
                let mut all = 0;
                for seg in 0..super::pieces::segments(p) {
                    let t = super::pieces::piece(r, line, i, seg);
                    assert!(t.iter().all(|t| t.v.iter().all(|v| v.x.is_finite() && v.y.is_finite() && v.z.is_finite())), "piece {i} ({:?}) has a non-finite vertex", p.kind);
                    assert!(t.len() < 6_000, "piece {i} ({:?}) is {} triangles in a stretch", p.kind, t.len());
                    all += t.len();
                }
                assert!(all > 0, "course {} road {line} piece {i} ({:?}) built nothing", r.key(), p.kind);
            }
        }
    }
}

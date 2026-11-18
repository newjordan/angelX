//! The world renderer's guarantees: bounded geometry per frame, built once,
//! finite everywhere, the same every time.
use super::route::{self, Route};
use super::{TRIANGLE_BUDGET, Tally, build_at as build, gather};
use crate::rules::knight_racers as rules;

fn every_course() -> Vec<&'static dyn Route> {
    rules::courses().iter().copied().map(route::on).collect()
}

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
    let (mut at, mut grown) = (402., usize::MAX);
    for _ in 0..12 {
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

const LOOKS: [route::Look; 6] = [route::Look::Dune, route::Look::Forge, route::Look::Ash, route::Look::Deep, route::Look::Castle, route::Look::Wash];

fn painted(material: u8) -> bool {
    let hit = std::cell::Cell::new(false);
    super::palette::painted().clone().patch(material, |_, _| { hit.set(true); None });
    hit.get()
}

#[test]
fn every_look_is_painted_with_every_material_it_names() {
    for look in LOOKS {
        let st = super::look::style(look);
        let named = [st.road[0], st.road[1], st.kerb[0], st.kerb[1], st.rail, st.rail_cap, st.post, st.lamp, st.ground, st.rock, st.plate, st.plate_b,
            st.masonry, st.slate, st.timber, st.cloth, st.cloth_b, st.glow, st.seam, st.hot, st.warn, st.lane, st.road_hot, st.pool, st.chevron[0], st.chevron[1]];
        for m in named.into_iter().chain(st.crowd).chain(st.liquid) {
            if m < 38 { continue; }
            if m >= 128 { assert!(super::palette::flat_kind(m).is_some_and(|k| k < 128), "{look:?}: light {m} is outside the sky table"); continue; }
            assert!(painted(m), "{look:?} names material {m}, which no table paints");
        }
    }
    for m in super::palette::WA_ROAD_A..=super::palette::WA_BARS { assert!(painted(m), "Moonward's material {m} is not painted"); }
}

#[test]
fn the_wash_has_a_haze_and_a_reach_of_its_own() {
    let r = route::on(rules::CourseId::Moonward);
    for (s0, _, look, name) in r.zones() { assert_eq!(look, route::Look::Wash, "{name} at {s0} is not the wash"); }
    for look in LOOKS.into_iter().filter(|l| *l != route::Look::Wash) {
        assert_ne!(super::fog_for(look), super::fog_for(route::Look::Wash), "{look:?} reaches as far as the wash");
        assert_ne!(super::haze(look), super::haze(route::Look::Wash), "{look:?} fades into the wash's haze");
        assert_ne!(super::look::style(look).road, super::look::style(route::Look::Wash).road);
    }
}

#[test]
fn every_look_has_a_sky_of_its_own() {
    use std::hash::{Hash, Hasher};
    let r = route::on(rules::CourseId::Proving);
    let eye = super::point(r, 100., 0., 3.4);
    let mut seen: Vec<(route::Look, u64)> = Vec::new();
    for look in [route::Look::Dune, route::Look::Forge, route::Look::Deep, route::Look::Castle, route::Look::Wash] {
        let mut tris = Vec::new();
        super::sky::dome(&mut tris, r, eye, 0.7, look, 300);
        assert!(tris.len() > 100, "{look:?}: a sky of {} triangles", tris.len());
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for t in &tris {
            t.mat.hash(&mut h);
            for uv in t.uv { uv[0].to_bits().hash(&mut h); uv[1].to_bits().hash(&mut h); }
        }
        let h = h.finish();
        for (other, o) in &seen { assert_ne!(*o, h, "{look:?} and {other:?} have the same sky"); }
        seen.push((look, h));
    }
}

#[test]
fn moonward_renders_inside_the_triangle_budget_at_twelve_points_round_its_lap() {
    let r = route::on(rules::CourseId::Moonward);
    for s in around(r) {
        let n = gather(r, s, 300, &mut Tally::default()).tris.len();
        assert!(n <= TRIANGLE_BUDGET, "Moonward at s={s}: {n} triangles, budget {TRIANGLE_BUDGET}");
        assert!(n > 2_000, "Moonward at s={s}: only {n} triangles, the world is missing");
    }
}

#[test]
fn each_new_set_piece_draws_something_in_every_place() {
    use super::route::{Kind, Piece};
    use super::shots::Gallery;
    let kinds = [Kind::PitLane, Kind::ForgeArches, Kind::Obelisks, Kind::Banners, Kind::Ribs, Kind::PoleLanterns];
    for (j, id) in [rules::CourseId::Proving, rules::CourseId::Emberfall, rules::CourseId::Glassway, rules::CourseId::Moatwalk, rules::CourseId::Moonward].into_iter().enumerate() {
        let base = route::on(id);
        for (n, kind) in kinds.into_iter().enumerate() {
            let piece = Piece { kind, s0: 120., s1: 210., side: if matches!(kind, Kind::PitLane | Kind::Ribs) { 1. } else { 0. }, seed: 5 };
            let g = Gallery { base, piece, key: 8_100 + (j * 10 + n) as u32 };
            let (mut all, mut worst) = (0, 0);
            for seg in 0..super::pieces::segments(&piece) {
                let t = super::pieces::piece(&g, 0, 0, seg);
                assert!(t.iter().all(|t| t.v.iter().all(|v| v.x.is_finite() && v.y.is_finite() && v.z.is_finite())), "{kind:?} on {id:?}: a non-finite vertex");
                all += t.len();
                worst = worst.max(t.len());
            }
            assert!(all >= 30, "{kind:?} on {id:?} drew only {all} triangles");
            assert!(worst < 4_000, "{kind:?} on {id:?} is {worst} triangles in one stretch");
        }
    }
}

#[test]
fn a_pit_lane_wears_the_materials_of_the_place_it_stands_in() {
    for &id in rules::courses() {
        let r = route::on(id);
        let Some((i, p)) = r.pieces().iter().enumerate().find(|(_, p)| p.kind == route::Kind::PitLane) else { panic!("{id:?} has no pit lane") };
        let st = super::look::style(r.zone((p.s0 + p.s1) * 0.5).0);
        let mut mats = std::collections::HashSet::new();
        for seg in 0..super::pieces::segments(p) { for t in super::pieces::piece(r, 0, i, seg).iter() { mats.insert(t.mat); } }
        for (role, m) in [("masonry", st.masonry), ("cloth", st.cloth), ("cloth_b", st.cloth_b), ("timber", st.timber), ("plate_b", st.plate_b)] {
            assert!(mats.contains(&m), "the pit lane on {id:?} does not wear its {role} ({m})");
        }
    }
}

#[test]
fn the_squires_wave_from_the_race_clock() {
    let r = route::on(rules::CourseId::Proving);
    let at = |ticks: u32| { let mut v = Vec::new(); super::pit::arms(&mut v, r, 150., ticks); v };
    let (a, b, c) = (at(100), at(100), at(113));
    assert!(a.len() > 20, "only {} triangles of waving arms", a.len());
    assert_eq!(a.len(), b.len());
    let bits = |v: &[crate::render::world3d::mesh::Tri]| v.iter().flat_map(|t| t.v.iter().map(|p| (p.x.to_bits(), p.y.to_bits(), p.z.to_bits()))).collect::<Vec<_>>();
    assert_eq!(bits(&a), bits(&b), "the same tick drew two waves");
    assert_ne!(bits(&a), bits(&c), "the arms did not move between two ticks");
}

#[test]
fn no_stretch_of_any_course_draws_more_than_the_budget() {
    for r in every_course() {
        let (mut worst, mut at) = (0, 0.);
        let mut s = 3.;
        while s < r.lap() {
            let n = gather(r, s, 300, &mut Tally::default()).tris.len();
            if n > worst { worst = n; at = s; }
            s += 50.;
        }
        eprintln!("course {} densest frame {worst} triangles at s={at}", r.key());
        assert!(worst <= TRIANGLE_BUDGET, "course {} at s={at}: {worst} triangles, budget {TRIANGLE_BUDGET}", r.key());
    }
}

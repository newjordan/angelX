//! The great tower stood up in meshes: the faces a stone shows, the meshes
//! kept and merged as lifts finish, the ditch cut in the ground, the stone's
//! own ramp.

use super as works_site;
use super::*;
use crate::world3d::{kingdom, realm_look};
use angel_stage::scene::MAX_MESH_VERTICES;

fn tower() -> &'static Castle {
    works::castle(works::KEEP).expect("the great tower")
}

#[test]
fn a_face_is_hidden_only_by_stones_laid_and_a_stone_never_shows_what_lies_under_it() {
    let c = tower();
    let covers = works_mesh::covers(c);
    assert_eq!(covers.faces.len(), c.stones.len());
    let (mut hidden, mut open) = (0usize, 0usize);
    for (i, f) in covers.faces.iter().enumerate() {
        for &cover in f {
            if cover == works::NEVER {
                open += 1;
            } else {
                hidden += 1;
            }
        }
        // A bottom on the ground is under it from the first.
        if let works::Shape::Box(b) = c.stones[i].shape
            && b[2] <= 5
        {
            assert_eq!(f[0], 0, "stone {i} stands on the ground");
        }
    }
    // Most of a shell wall's faces are covered by its neighbours; some are open to the air.
    assert!(hidden > open, "{hidden} hidden, {open} open");
    assert!(open > 15_000, "{open} faces are open to the air");
}

#[test]
fn every_part_is_a_mesh_the_gpu_takes_and_the_castle_fits_the_budget() {
    let c = tower();
    for laid in [0, 1, 975, 1_500, 4_000, 7_100, 11_000, c.len()] {
        let v = works_site::view(c, laid);
        let mut verts = 0usize;
        for (p, wet) in v.parts.iter().map(|(p, w)| (p, *w)).chain(v.falsework.iter().map(|p| (p, 0.0))) {
            p.mesh.validate().unwrap_or_else(|e| panic!("a part at {laid}: {e:?}"));
            assert!(p.mesh.positions.len() <= MAX_MESH_VERTICES / 2, "a part of {} vertices", p.mesh.positions.len());
            assert!((0.0..=1.0).contains(&wet));
            assert!(p.mesh.normals.iter().all(|n| (n[0] * n[0] + n[1] * n[1] + n[2] * n[2] - 1.0).abs() < 1e-3), "normals are unit");
            assert!(p.mesh.colors.iter().all(|c| c.iter().all(|v| (0.0..=1.0).contains(v))));
            verts += p.mesh.positions.len();
        }
        if laid == c.len() {
            // The whole castle: under a million vertices, a few hundred thousand a chunk at most.
            assert!(verts < 400_000, "{verts} vertices");
            println!("the finished castle: {} parts, {verts} vertices", v.parts.len());
        }
    }
}

#[test]
fn a_finished_lift_is_kept_and_only_the_last_few_stones_are_made_as_they_are_laid() {
    let c = tower();
    let first = c.stages.iter().filter(|s| s.count > 0).next().expect("a lift");
    let end = first.first + first.count;
    // Three stones into the second lift, and four.
    let a = works_site::view(c, end + 3);
    let b = works_site::view(c, end + 4);
    let ids = |v: &works_site::WorksView| v.parts.iter().map(|(p, _)| p.id).collect::<Vec<_>>();
    let (ia, ib) = (ids(&a), ids(&b));
    // The finished lift's meshes are the same ones; the one tail is made again.
    let kept = ia.iter().filter(|id| ib.contains(id)).count();
    assert!(kept >= ia.len().saturating_sub(4), "{kept} of {} kept", ia.len());
    assert!(kept < ib.len().max(ia.len()) || ia == ib);
    // A lift going up is in blocks: with enough laid there is a full block kept too.
    let c64 = works_site::view(c, end + works_mesh::BLOCK + 3);
    let d64 = works_site::view(c, end + works_mesh::BLOCK + 9);
    let (ic, id) = (ids(&c64), ids(&d64));
    assert!(ic.iter().filter(|x| id.contains(x)).count() >= ic.len().saturating_sub(4));
}

#[test]
fn the_newest_blocks_are_wet_and_dry_as_the_course_goes_on() {
    assert_eq!(works_site::wetness(1000, 1000), 1.0);
    assert!(works_site::wetness(1000 + works_site::WET_STONES / 2, 1000) > 0.4);
    assert_eq!(works_site::wetness(1000 + works_site::WET_STONES, 1000), 0.0);
    let c = tower();
    let v = works_site::view(c, 1_500);
    let wet: Vec<f32> = v.parts.iter().map(|(_, w)| *w).collect();
    assert!(wet.iter().any(|&w| w == 1.0), "the last stones are wet");
    assert!(wet.iter().any(|&w| w == 0.0), "the first lift is dry");
}

#[test]
fn the_stone_has_the_hillsides_brightness_ladder_in_its_own_hue() {
    use angel_stage::brand::luma;
    let ramp = works_site::stone_ramp();
    for (stone, grass) in ramp.iter().zip(realm_look::HILLSIDE.iter()) {
        assert!((luma(*stone) - luma(*grass)).abs() < 0.02, "{stone:?} vs {grass:?}");
    }
    // Warm: red over blue at the light end, where the grass is green over both.
    let top = ramp[7];
    assert!(top[0] > top[2] + 10, "{top:?}");
}

#[test]
fn the_ditch_is_cut_in_the_ground_of_the_chunks_it_crosses_and_the_causeway_is_left_whole() {
    let c = tower();
    let index = kingdom::index(5, 5).expect("a chunk");
    let chunk = kingdom::chunks().chunk(index).expect("the chunk");
    assert!(works_site::touches_for_test(c, &chunk), "the middle chunk holds the castle's earthworks");
    // The ditch is dug from the first stone: with none laid it is dug already.
    let dug = works_site::dig_text_for_test(c, &chunk.text, 0).expect("dug before a stone is laid");
    assert_eq!(Some(&dug), works_site::dig_text_for_test(c, &chunk.text, 1).as_ref());
    let (before, after) = (ground(&chunk.text), ground(&dug));
    assert_eq!(before.0.len(), after.0.len());
    let lowered = before.0.iter().zip(&after.0).filter(|(b, a)| **a < **b - 3.0).count();
    assert!(lowered > 30, "{lowered} corners lowered");
    // Nothing raised, and the corners of the causeway before the gate not lowered.
    assert!(before.0.iter().zip(&after.0).all(|(b, a)| *a <= *b + 0.11));
    let (ox, oz) = works::origin_px();
    let (cols, step) = (48usize, 8.0f32);
    let (cx0, cz0) = (5.0 * 384.0, 5.0 * 256.0);
    let at = |i: usize, j: usize| (cx0 + i as f32 * step, cz0 + j as f32 * step);
    for (k, (b, a)) in before.0.iter().zip(&after.0).enumerate() {
        let (x, z) = at(k % (cols + 1), k / (cols + 1));
        let (mx, my) = ((x - ox) / works::PX_PER_M, -(z - oz) / works::PX_PER_M);
        if mx.abs() < 0.9 && my < -17.0 && my > -22.0 {
            assert!((*a - *b).abs() < 0.11, "the causeway at {mx:.1},{my:.1} is cut: {b} -> {a}");
        }
    }
    // The cut chunk is a chunk the game takes.
    chunk.cut(dug.into()).expect("the dug chunk reads");
}

/// A chunk text's ground heights.
fn ground(text: &str) -> (Vec<f32>, Vec<f32>) {
    let line = text.lines().find(|l| l.starts_with("ground ")).expect("a ground");
    let (_, rest) = line.split_once(" heights").expect("heights");
    let (h, t) = rest.split_once(" tones").expect("tones");
    (h.split_whitespace().map(|v| v.parse().unwrap()).collect(), t.split_whitespace().map(|v| v.parse().unwrap()).collect())
}

/// A run out in the kingdom with the Keep `laid` stones up and a knight in its bailey.
fn run_at(laid: u32) -> crate::rules::Run {
    let mut run = kingdom::proof_run(&[[133.0 * 16.0, 87.0 * 16.0]]);
    run.lay_to("great-tower", laid);
    run
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world3d::works_site -- --include-ignored --test-threads=1"]
fn a_window_has_the_stones_with_the_world_and_a_page_across_a_wire_is_sent_them_once() {
    use crate::render::overworld::arena::{self, stage_frame::StageMaker};
    assert!(crate::world::plan::tabula(), "needs the blank-slate plan");
    arena::draw_parts_outside(true);
    arena::props_meshes(true);
    crate::world3d::set_enabled(true);
    let (w, h) = arena::seat_size();
    let run = run_at(3_000);
    let scene = std::sync::Arc::new(arena::seat_scene(&run, w as i32, h as i32, Some(1)));
    let works = scene.works.as_ref().expect("the site is in the scene");
    assert!(!works.parts.is_empty() && !works.life.is_empty());
    // A window: the stones' meshes with the world, drawn from its nodes.
    let frame = StageMaker::default().make(&scene, |_| None);
    let world = frame.props.as_ref().and_then(|p| p.world.as_ref()).expect("a world");
    let named = |id: u32| world.extra.iter().any(|n| n.mesh == Some(angel_stage::scene::MeshRef::Uploaded(id)));
    let drawn: Vec<_> = works.parts.iter().filter(|(p, _)| named(p.id)).collect();
    assert!(!drawn.is_empty(), "the stones are nodes among the blocks");
    assert!(drawn.iter().all(|(p, _)| world.meshes.iter().any(|(id, _)| *id == p.id)), "and their meshes go with them");
    // A page across a wire: the meshes in the frame, once.
    // A page that draws the stage's props (a browser's renderer does, once it says so).
    let mut maker = StageMaker::for_wire();
    maker.flat_shots = false;
    let first = maker.make(&scene, |_| None);
    let sent = first.meshes.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(!sent.is_empty(), "the stones are sent to a page");
    assert!(sent.len() <= angel_stage::wire::MAX_FRAME_MESHES);
    // Every mesh a node names has been sent by the time the nodes are drawn.
    let named_by_world: Vec<u32> = first
        .props
        .as_ref()
        .and_then(|p| p.world.as_ref())
        .map(|w| w.extra.iter().filter_map(|n| if let Some(angel_stage::scene::MeshRef::Uploaded(id)) = n.mesh { Some(id) } else { None }).collect())
        .unwrap_or_default();
    assert!(named_by_world.iter().all(|id| sent.contains(id)), "no node names a mesh the page has not been sent");
    let again = maker.make(&scene, |_| None);
    assert!(again.meshes.iter().all(|(id, _)| !sent.contains(id)), "a mesh is sent once");
    // A page that drew none for a while is sent them again.
    maker.forget();
    let all = maker.make(&scene, |_| None);
    assert!(!all.meshes.is_empty());
    crate::world3d::set_enabled(false);
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world3d::works_site -- --include-ignored --test-threads=1"]
fn the_chronicle_replays_the_build_for_everyone_and_the_walls_stay_as_they_are() {
    use std::collections::BTreeMap;
    assert!(crate::world::plan::tabula(), "needs the blank-slate plan");
    let mut run = run_at(9_000);
    for _ in 0..5 {
        run.step(&BTreeMap::new());
    }
    let site = run.sites.keep().expect("site").clone();
    // A knight holds E in the bailey; the replay begins after the hold.
    let hold = crate::rules::site::CHRONICLE_HOLD;
    let press = crate::rules::Input { interact: true, ..Default::default() };
    // The knight stands in the bailey (not at the Delve's door).
    for _ in 0..hold + 2 {
        run.step(&BTreeMap::from([(1, press)]));
    }
    let began = run.site_replay.expect("the Chronicle began");
    let shown_at = |run: &crate::rules::Run| run.site_shown(run.sites.keep().expect("site"));
    assert!(shown_at(&run) < site.laid, "it begins from bare hillside");
    // Over thirty seconds it rises to what stands; the stones laid are the run's, untouched.
    for _ in 0..crate::rules::site::CHRONICLE_TICKS {
        run.step(&BTreeMap::new());
    }
    assert!(run.tick >= began + crate::rules::site::CHRONICLE_TICKS);
    assert!(run.site_replay.is_none(), "it ends");
    assert!(shown_at(&run) >= site.laid, "and shows what stands");
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib world3d::works_site -- --include-ignored --test-threads=1"]
fn standing_back_takes_in_the_whole_castle_in_both_views_for_a_seat_that_asks_and_for_all_in_the_chronicle() {
    use crate::render::overworld::arena::{self, MapView, stand_back};
    assert!(crate::world::plan::tabula(), "needs the blank-slate plan");
    arena::draw_parts_outside(true);
    arena::props_meshes(true);
    let (w, h) = arena::seat_size();
    let mut run = run_at(6_000);
    let held = MapView { full: true, ..Default::default() };
    // Only when asked for, and only at the site.
    assert!(!stand_back::wanted(&run, Some(1), false));
    assert!(stand_back::wanted(&run, Some(1), true));
    let mut far = run.clone();
    far.players.get_mut(&1).expect("a knight").x = 20.0;
    assert!(!stand_back::wanted(&far, Some(1), true), "a knight far from the Keep has no castle to stand back from");
    // The 3D view: the stage three times the native size, and the castle all in it.
    crate::world3d::set_enabled(true);
    let near = arena::seat_scene(&run, w as i32, h as i32, Some(1));
    let back = arena::seat_scene_with(&run, w as i32, h as i32, Some(1), &held);
    assert_eq!((near.view.0 * 3, near.view.1 * 3), back.view, "the stage is three times the size, the bands with it");
    let (ox, oy) = stand_back::origin();
    assert_eq!(back.camera.0 as i32, ox);
    let castle = run.sites.keep().and_then(|s| s.castle()).expect("the great tower");
    let (lo, hi) = castle.stones.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), s| {
        s.corners().iter().fold((lo, hi), |(lo, hi), p| {
            let q = castle.world(*p);
            let (x, y) = (q[0], q[2] - q[1]);
            ([lo[0].min(x), lo[1].min(y)], [hi[0].max(x), hi[1].max(y)])
        })
    });
    assert!(lo[0] > ox as f32 && hi[0] < (ox + 1152) as f32, "across: {lo:?} {hi:?} in {ox}");
    assert!(lo[1] > oy as f32 && hi[1] < (oy + 672) as f32, "down: {lo:?} {hi:?} in {oy}");
    assert!(back.works.as_ref().is_some_and(|w| !w.parts.is_empty()), "the stones are in it");
    // Its figures stand in the wider window: the crew too.
    assert!(back.figures.len() >= near.figures.len());
    crate::world3d::set_enabled(false);
    // The flat view: the same frame size, with the castle on it small.
    let frame_near = arena::seat_frame(&run, w as i32, h as i32, Some(1));
    let frame_back = arena::seat_frame_with(&run, w as i32, h as i32, Some(1), &held);
    assert_eq!((frame_near.w, frame_near.h), (frame_back.w, frame_back.h));
    assert_ne!(frame_near.words(), frame_back.words());
    // The Chronicle stands everyone at the site back, no key held.
    assert!(!stand_back::wanted(&run, Some(1), false));
    run.site_replay = Some(run.tick);
    assert!(stand_back::wanted(&run, Some(1), false));
}

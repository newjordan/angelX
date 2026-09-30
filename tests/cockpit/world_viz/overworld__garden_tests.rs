use super::*;
use crate::agent::harness::{ExecutionOutcome, VerificationOutcome};
use crate::knowledge::graph_crop::{ChartSpec, DataPoint, GraphStore};
fn request(kind: ChartKind) -> GraphRequest {
    GraphRequest::Begin {
        plot: "harvest".into(),
        spec: ChartSpec {
            title: "Harvest".into(),
            kind,
            x_label: "Day".into(),
            y_label: "Bushels".into(),
            x_min: 0.0,
            x_max: 4.0,
            y_min: -10.0,
            y_max: 10.0,
            expected_points: 2,
        },
    }
}
fn point(generation: u64, index: u8, y: f64) -> GraphRequest {
    GraphRequest::Point {
        plot: "harvest".into(),
        generation,
        index,
        point: DataPoint {
            label: format!("day {index}"),
            x: f64::from(index) + 1.0,
            y,
        },
    }
}
fn ok() -> ToolOutcome {
    ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    }
}
fn apply(
    store: &mut GraphStore,
    garden: &mut Garden,
    request: GraphRequest,
    id: &str,
    tick: u64,
) -> u64 {
    let id = ToolEventId(id.into());
    garden.note(&id, &GraphEvent::Requested(request.clone()), tick);
    let receipt = store.apply(request).unwrap();
    let generation = receipt.chart.as_ref().map_or(0, |c| c.generation);
    garden.note(&id, &GraphEvent::Returned(receipt), tick);
    garden.settle(&id, ok(), tick);
    generation
}

#[test]
fn graph_crop_sprites_are_correlated_and_failed_calls_never_plant() {
    let mut store = GraphStore::default();
    let mut garden = Garden::default();
    let gen_id = apply(&mut store, &mut garden, request(ChartKind::Bar), "begin", 0);
    let request = point(gen_id, 0, 7.0);
    let id = ToolEventId("point".into());
    garden.note(&id, &GraphEvent::Requested(request.clone()), 1);
    let before = garden.frame(1);
    assert_eq!(before.sprites.len(), 2);
    assert!(before.bed.unwrap().chart.unwrap().points.is_empty());
    let receipt = store.apply(request).unwrap();
    garden.note(
        &ToolEventId("unknown".into()),
        &GraphEvent::Returned(receipt.clone()),
        1,
    );
    assert!(
        garden
            .frame(1)
            .bed
            .unwrap()
            .chart
            .unwrap()
            .points
            .is_empty()
    );
    garden.settle(
        &id,
        ToolOutcome {
            execution: ExecutionOutcome::Denied,
            verification: VerificationOutcome::NotApplicable,
        },
        1,
    );
    garden.note(&id, &GraphEvent::Returned(receipt), 2);
    assert!(
        garden
            .frame(2)
            .bed
            .unwrap()
            .chart
            .unwrap()
            .points
            .is_empty()
    );
    assert_eq!(
        garden
            .frame(2)
            .sprites
            .iter()
            .find(|j| j.id == id)
            .unwrap()
            .returned,
        Some((1, false))
    );
}

#[test]
fn graph_crop_rejected_out_of_domain_points_render_at_the_farmer_without_changing_crops() {
    let mut store = GraphStore::default();
    let mut garden = Garden::default();
    let generation = apply(&mut store, &mut garden, request(ChartKind::Bar), "begin", 0);
    apply(
        &mut store,
        &mut garden,
        point(generation, 0, 8.0),
        "valid",
        1,
    );
    let planted = garden.frame(1).bed.unwrap();
    for (n, x) in [1e12, -1e12].into_iter().enumerate() {
        let mut request = point(generation, 1, 3.0);
        if let GraphRequest::Point { point, .. } = &mut request {
            point.x = x;
        }
        let id = ToolEventId(format!("rejected-{n}"));
        garden.note(&id, &GraphEvent::Requested(request.clone()), 2);
        assert!(store.apply(request).is_err());
        garden.settle(
            &id,
            ToolOutcome {
                execution: ExecutionOutcome::Failed,
                verification: VerificationOutcome::NotApplicable,
            },
            2,
        );
    }
    let mut scene = super::super::Scene::resting();
    scene.garden = garden.frame(32);
    assert_eq!(scene.garden.bed, Some(planted));
    let (x, y) = origin();
    let image = super::super::render_view(
        &scene,
        View {
            x: x - 24,
            y: y - 24,
            w: W + 48,
            h: H + 48,
        },
    );
    assert_eq!((image.w, image.h), (W + 48, H + 48));
    let mut props = Vec::new();
    let mut cues = Vec::new();
    stage(&scene.garden, &mut props, &mut cues);
    let farmer = (x + 24, y + H - 12);
    for cue in cues.iter().filter(|c| c.img.w == 9) {
        assert!((x..x + W).contains(&cue.x));
        assert!((y..y + H).contains(&cue.base));
    }
    assert_eq!(
        cues.iter()
            .filter(|c| c.img.w == 9 && c.x == farmer.0 - 4 && c.base == farmer.1 - 6)
            .count(),
        2
    );
}

#[test]
fn graph_crop_refresh_removes_every_old_crop_and_old_receipts_cannot_restore_it() {
    let mut store = GraphStore::default();
    let mut garden = Garden::default();
    let generation = apply(&mut store, &mut garden, request(ChartKind::Bar), "begin", 0);
    apply(
        &mut store,
        &mut garden,
        point(generation, 0, 7.0),
        "point",
        1,
    );
    let stale_id = ToolEventId("stale-result".into());
    let old_request = point(generation, 1, 3.0);
    garden.note(&stale_id, &GraphEvent::Requested(old_request.clone()), 2);
    let stale = store.apply(old_request).unwrap();
    apply(
        &mut store,
        &mut garden,
        request(ChartKind::Scatter),
        "regrow",
        3,
    );
    garden.note(&stale_id, &GraphEvent::Returned(stale), 4);
    let bed = garden.frame(4).bed.unwrap();
    assert!(bed.chart.unwrap().points.is_empty());
    assert!(bed.planted.is_empty());
    let before = garden.epoch;
    apply(
        &mut store,
        &mut garden,
        GraphRequest::Clear {
            plot: "harvest".into(),
        },
        "clear",
        5,
    );
    assert!(garden.frame(5).bed.unwrap().chart.is_none());
    assert!(garden.epoch > before);
}

#[test]
fn graph_crop_growth_is_bounded_and_world_can_park_after_delivery() {
    let mut store = GraphStore::default();
    let mut garden = Garden::default();
    let generation = apply(&mut store, &mut garden, request(ChartKind::Bar), "begin", 0);
    apply(
        &mut store,
        &mut garden,
        point(generation, 0, 7.0),
        "point",
        1,
    );
    assert_eq!(growth(&garden.frame(1), 0), 0.0);
    assert!(garden.animating(40));
    assert_eq!(growth(&garden.frame(1000), 0), 1.0);
    assert!(!garden.animating(1000));
    let mut world = crate::stage::world_viz::World::new(7);
    world.note_graph_event(
        &ToolEventId("actual".into()),
        &GraphEvent::Requested(request(ChartKind::Bar)),
    );
    assert_eq!(world.overworld_goal(), super::super::map::Place::Fields);
    assert_eq!(world.visit_overworld("garden"), Some("GRAPH GARDEN"));
    assert_eq!(world.overworld_scene().camera, centre());
}

#[test]
fn graph_crop_hidden_stage_call_storms_keep_the_latest_data_and_bound_animations() {
    let mut store = GraphStore::default();
    let mut garden = Garden::default();
    let generation = apply(&mut store, &mut garden, request(ChartKind::Bar), "begin", 0);
    for index in 0..700 {
        apply(
            &mut store,
            &mut garden,
            point(generation, 0, f64::from(index % 10)),
            &format!("call-{index}"),
            0,
        );
    }
    assert_eq!(
        garden.frame(0).bed.unwrap().chart.unwrap().points[&0].y,
        9.0
    );
    assert!(garden.jobs.len() <= MAX_JOBS);
    assert!(garden.completed.len() <= 512);
    assert!(!garden.animating(1000));
}

#[test]
fn graph_crop_a_quick_call_keeps_the_knight_walking_until_he_reaches_the_fields() {
    let mut world = crate::stage::world_viz::World::new(7);
    let mut store = GraphStore::default();
    let id = ToolEventId("short-call".into());
    let request = request(ChartKind::Bar);
    world.note_graph_event(&id, &GraphEvent::Requested(request.clone()));
    world.note_graph_event(&id, &GraphEvent::Returned(store.apply(request).unwrap()));
    world.settle_graph_call(&id, ok());
    let mut ticks = 0;
    while world.animating() && ticks < 512 {
        world.tick();
        ticks += 1;
    }
    let target = super::super::scene::Knight::at_place(super::super::map::Place::Fields);
    assert_eq!(world.overworld_scene().knight.x, target.x);
    assert_eq!(world.overworld_scene().knight.y, target.y);
    assert!(
        !world.animating(),
        "garden must park after arrival and growth"
    );
    assert_eq!(world.overworld_goal(), super::super::map::Place::Fields);
    world.note_tool_call_event(
        ToolEventId("next-work".into()),
        "read_file",
        "path=README.md",
    );
    assert_eq!(
        world.overworld_goal(),
        super::super::map::Place::Scriptorium
    );
}

#[test]
fn graph_crop_duplicate_native_receipts_do_not_restart_growth_and_motion_off_is_settled() {
    let mut store = GraphStore::default();
    let mut garden = Garden::default();
    let generation = apply(&mut store, &mut garden, request(ChartKind::Bar), "begin", 0);
    let id = ToolEventId("point".into());
    let request = point(generation, 0, 7.0);
    garden.note(&id, &GraphEvent::Requested(request.clone()), 1);
    let receipt = store.apply(request).unwrap();
    garden.note(&id, &GraphEvent::Returned(receipt.clone()), 1);
    let planted = garden.frame(1).bed.unwrap().planted;
    let epoch = garden.epoch;
    garden.note(&id, &GraphEvent::Returned(receipt), 100);
    assert_eq!(garden.frame(100).bed.unwrap().planted, planted);
    assert_eq!(garden.epoch, epoch);
    let mut frame = garden.frame(1);
    frame.motion = MotionMode::Off;
    assert_eq!(growth(&frame, 0), 1.0);
}

#[test]
fn graph_crop_empty_and_settled_gardens_do_not_rekey_every_world_tick() {
    let mut garden = Garden::default();
    assert_eq!(garden.frame(0), garden.frame(10_000));
    let mut store = GraphStore::default();
    let generation = apply(&mut store, &mut garden, request(ChartKind::Bar), "begin", 0);
    apply(
        &mut store,
        &mut garden,
        point(generation, 0, 7.0),
        "point",
        1,
    );
    assert_eq!(garden.frame(1000), garden.frame(2000));
}

#[test]
fn graph_crop_geometry_uses_declared_scales_including_negative_bars() {
    for kind in [ChartKind::Bar, ChartKind::Line, ChartKind::Scatter] {
        let mut store = GraphStore::default();
        let mut garden = Garden::default();
        let generation = apply(&mut store, &mut garden, request(kind), "begin-kind", 0);
        apply(&mut store, &mut garden, point(generation, 0, 8.0), "p0", 1);
        apply(&mut store, &mut garden, point(generation, 1, -4.0), "p1", 2);
        let chart = garden.frame(1000).bed.unwrap().chart.unwrap();
        assert_eq!(point_position(&chart, 0.0, -10.0), (LEFT, BOTTOM));
        assert_eq!(point_position(&chart, 4.0, 10.0), (RIGHT, TOP));
        let (ox, oy) = origin();
        let view = View {
            x: ox,
            y: oy,
            w: W,
            h: H,
        };
        let mut image = Img::black(W, H);
        let frame = garden.frame(1000);
        paint_ground(&mut image, &frame, view);
        let mut second = Img::black(W, H);
        paint_ground(&mut second, &frame, view);
        assert_eq!(image.rgb_bytes(), second.rgb_bytes());
        for p in chart.points.values() {
            let (x, y) = point_position(&chart, p.x, p.y);
            assert!(image.get(x, y).is_some());
        }
    }
}

#[test]
fn graph_crop_preview_exports_real_world_frames_for_art_review() {
    let Ok(dir) = std::env::var("ANGEL_GRAPH_GARDEN_REVIEW") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    for kind in [ChartKind::Bar, ChartKind::Line, ChartKind::Scatter] {
        let mut store = GraphStore::default();
        let mut garden = Garden::default();
        let generation = apply(&mut store, &mut garden, request(kind), "begin", 0);
        apply(&mut store, &mut garden, point(generation, 0, 8.0), "p0", 1);
        apply(&mut store, &mut garden, point(generation, 1, -4.0), "p1", 2);
        for (label, tick) in [("growing", 35), ("grown", 150)] {
            let mut scene = super::super::Scene::resting();
            scene.garden = garden.frame(tick);
            scene.tick = tick as u32;
            scene.knight = super::super::scene::Knight::at_place(super::super::map::Place::Fields);
            let (x, y) = origin();
            let img = super::super::render_view(
                &scene,
                View {
                    x: x - 24,
                    y: y - 24,
                    w: W + 48,
                    h: H + 48,
                },
            );
            image::save_buffer(
                std::path::Path::new(&dir).join(format!("{kind:?}-{label}.png")),
                &img.rgba_scaled(3),
                (img.w * 3) as u32,
                (img.h * 3) as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
}

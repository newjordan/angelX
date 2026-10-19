//! The Orrery's pure geometry: panels, leans, segments, depth rows and
//! pulses, without a window. The layer assembly itself is glass-gated
//! and covered below when the feature runs.

use crate::agent::harness::{GraphFaninTruth, GraphNodePhase, GraphNodeSnap, GraphRunPhase, GraphSnapshot};
use crate::ui::orrery::{
    Grid, MAX_EDGES, MAX_LAYERS, MAX_NODES, Panel, leaned_point, node_depths, node_positions,
    phase_color, pulse_at, ribbon_points, segment_rect, tilted_rect,
};

fn node(id: &str, deps: &[&str], phase: GraphNodePhase) -> GraphNodeSnap {
    GraphNodeSnap {
        id: id.to_string(),
        deps: deps.iter().map(|d| d.to_string()).collect(),
        persona: "seeker".into(),
        club: "round".into(),
        grant: "none".into(),
        pool: None,
        pool_limit: None,
        lease_id: None,
        phase,
        elapsed_ms: 0,
        retries: 0,
        gate: None,
        output_chars: 0,
    }
}

fn snapshot(nodes: Vec<GraphNodeSnap>, phase: GraphRunPhase) -> GraphSnapshot {
    GraphSnapshot {
        episode_id: "e1".into(),
        workspace_key_sha256: String::new(),
        graph: "round".into(),
        task: "test the orrery".into(),
        phase,
        nodes,
        events: Vec::new(),
        final_answer: None,
        error: None,
        elapsed_ms: 0,
        fanin: GraphFaninTruth::default(),
        incomplete_reason: None,
    }
}

#[test]
fn the_panel_hides_on_small_grids_and_fits_large_ones() {
    assert_eq!(Panel::for_grid(Grid { width: 540, height: 1000 }), None);
    assert_eq!(Panel::for_grid(Grid { width: 1440, height: 525 }), None);
    let grid = Grid { width: 1440, height: 1000 };
    let panel = Panel::for_grid(grid).expect("a roomy window floats one");
    // Anchored top right, two cells in and below the header, clamped tall and wide.
    assert!((panel.x + panel.w - (1440.0 - 24.0)).abs() < 1e-4);
    assert!((panel.y - 110.0).abs() < 1e-4);
    assert!(panel.w <= 38.0 * 12.0 && panel.h <= 17.0 * 25.0);
    // The content sits inside the plate, clear of the chrome.
    let content = panel.content();
    assert!(content.x > panel.x && content.y > panel.y);
    assert!(content.x + content.w < panel.x + panel.w);
    assert!(content.y + content.h < panel.y + panel.h);
}

#[test]
fn a_leaned_rect_keeps_its_size_and_turns_its_centre() {
    let centre = [50.0, 10.0];
    let rect = [46.0, 6.0, 8.0, 4.0];
    let ([x, y, w, h], angle) = tilted_rect(rect, centre);
    // The quad is centred where the rect's centre lands, at its own size:
    // the window turns it about that centre by `angle`.
    let want = leaned_point([rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0], centre);
    assert!((x + w / 2.0 - want[0]).abs() < 1e-4 && (y + h / 2.0 - want[1]).abs() < 1e-4);
    assert_eq!(angle, crate::ui::orrery::TILT);
    assert_eq!((w, h), (rect[2], rect[3]), "a lean never resizes a rect");
    // A rect centred on the panel leans in place: its centre stays.
    let square = [centre[0] - 5.0, centre[1] - 5.0, 10.0, 10.0];
    let ([x, y, w, h], _) = tilted_rect(square, centre);
    assert!((x + w / 2.0 - centre[0]).abs() < 1e-4 && (y + h / 2.0 - centre[1]).abs() < 1e-4);
}

#[test]
fn a_lean_in_cells_turns_in_pixels() {
    // A cell is 12 x 25 px: turning a cell rect's centre in cell units
    // would skew it; in pixels it lands where the pixel lean puts it.
    let unit = (12.0, 25.0);
    let centre = [20.0, 10.0];
    let rect = [30.0, 12.0, 4.0, 2.0];
    let ([x, y, w, h], _) = crate::ui::orrery::tilted_rect_in(rect, centre, unit);
    let px = |r: [f32; 4]| [r[0] * unit.0, r[1] * unit.1, r[2] * unit.0, r[3] * unit.1];
    let (want, _) = tilted_rect(px(rect), [centre[0] * unit.0, centre[1] * unit.1]);
    let got = px([x, y, w, h]);
    for i in 0..4 {
        assert!((got[i] - want[i]).abs() < 1e-3, "{got:?} vs {want:?}");
    }
}

#[test]
fn a_segment_spans_its_endpoints_at_its_slope() {
    let (rect, angle) = segment_rect([10.0, 5.0], [20.0, 5.0], 0.07);
    assert!((rect[2] - 10.0).abs() < 1e-4, "length across");
    assert!((rect[3] - 0.07).abs() < 1e-4, "thickness across");
    assert!((rect[0] + rect[2] / 2.0 - 15.0).abs() < 1e-4, "midway");
    assert!(angle.abs() < 1e-4, "flat");
    let (rect, angle) = segment_rect([0.0, 0.0], [0.0, 8.0], 0.07);
    assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 1e-4, "plumb");
    assert!((rect[2] - 8.0).abs() < 1e-4 && (rect[3] - 0.07).abs() < 1e-4);
}

#[test]
fn a_pulse_travels_its_edge_end_to_end() {
    let (a, b) = ([4.0, 6.0], [10.0, 14.0]);
    let at = pulse_at(a, b, 0.0);
    assert!((at[0] - a[0]).abs() < 1e-4 && (at[1] - a[1]).abs() < 1e-4);
    let at = pulse_at(a, b, 1.0);
    assert!((at[0] - b[0]).abs() < 1e-4 && (at[1] - b[1]).abs() < 1e-4);
    let at = pulse_at(a, b, 0.5);
    assert!((at[0] - 7.0).abs() < 1e-4 && (at[1] - 10.0).abs() < 1e-4);
}

#[test]
fn depths_follow_chains_ignore_outsiders_and_survive_cycles() {
    let nodes = vec![
        node("a", &[], GraphNodePhase::Done),
        node("b", &["a"], GraphNodePhase::Running),
        node("c", &["b", "a"], GraphNodePhase::Pending),
        node("d", &["ghost"], GraphNodePhase::Pending),
    ];
    assert_eq!(node_depths(&nodes), vec![0, 1, 2, 0]);
    // A cycle is bounded, not a stack overflow, and stays finite.
    let cycle = vec![
        node("x", &["y"], GraphNodePhase::Running),
        node("y", &["x"], GraphNodePhase::Running),
    ];
    let depths = node_depths(&cycle);
    assert_eq!(depths.len(), 2);
    // The shown cap holds: a crowd of orphans all land at depth 0.
    let crowd: Vec<GraphNodeSnap> = (0..MAX_NODES + 3)
        .map(|i| node(&format!("n{i}"), &[], GraphNodePhase::Pending))
        .collect();
    assert_eq!(node_depths(&crowd).len(), MAX_NODES);
}

#[test]
fn positions_stay_inside_content_and_rows_spread() {
    let content = Panel { x: 700.0, y: 90.0, w: 360.0, h: 240.0 };
    let nodes = vec![
        node("a", &[], GraphNodePhase::Done),
        node("b", &[], GraphNodePhase::Done),
        node("c", &["a", "b"], GraphNodePhase::Running),
    ];
    let at = node_positions(&nodes, content);
    assert_eq!(at.len(), 3);
    for p in &at {
        assert!(
            content.contains(*p, 1e-4),
            "inside the content: {p:?}"
        );
    }
    // Two roots share the top row either side of its middle; their
    // dependent sits below them.
    assert!((at[0][1] - at[1][1]).abs() < 1e-4, "same row");
    assert!(at[2][1] > at[0][1] + 10.0, "below");
    assert!(at[0][0] < at[1][0], "spread across the row");
    // Deterministic: the same nodes give the same constellation.
    assert_eq!(node_positions(&nodes, content), at);
}

#[test]
fn the_idle_ribbon_keeps_to_the_content() {
    let content = Panel { x: 700.0, y: 90.0, w: 360.0, h: 240.0 };
    for t in [0.0f32, 2.7, 91.3] {
        let points = ribbon_points(content, t);
        assert_eq!(points.len(), 22);
        assert_eq!(points[0][0], content.x);
        assert!((points[21][0] - (content.x + content.w)).abs() < 1e-4);
        for p in &points {
            assert!(
                p[1] >= content.y - 1.0 && p[1] <= content.y + content.h + 1.0,
                "wave inside: {p:?}"
            );
        }
    }
}

#[test]
fn the_panel_recedes_above_its_centre() {
    let centre = [50.0, 12.0];
    // The centre itself is unmoved by depth.
    let at = crate::ui::orrery::deep_point(centre, centre);
    assert!((at[0] - centre[0]).abs() < 1e-4 && (at[1] - centre[1]).abs() < 1e-4);
    // A point above the centre draws nearer the centre and no further
    // out than its plain lean; one below draws larger, not smaller.
    let above = [50.0, 5.0];
    let deep = crate::ui::orrery::deep_point(above, centre);
    let lean = crate::ui::orrery::leaned_point(above, centre);
    assert!(deep[1] > lean[1], "drawn lower: closer to the eye's middle");
    let below = [50.0, 19.0];
    let deep_below = crate::ui::orrery::deep_point(below, centre);
    let lean_below = crate::ui::orrery::leaned_point(below, centre);
    assert!(deep_below[1] > lean_below[1], "drawn lower still: nearer");
    // Depth scales rects: the same rect further up the panel is smaller.
    let (high, _) = crate::ui::orrery::deep_rect([46.0, 2.0, 8.0, 4.0], centre);
    let (low, _) = crate::ui::orrery::deep_rect([46.0, 18.0, 8.0, 4.0], centre);
    assert!(low[2] > high[2] && low[3] > high[3], "nearer rows draw larger");
}

#[test]
fn every_phase_color_is_a_straight_rgba() {
    for phase in [
        GraphNodePhase::Pending,
        GraphNodePhase::Running,
        GraphNodePhase::Done,
        GraphNodePhase::Failed,
        GraphNodePhase::Skipped,
    ] {
        for v in phase_color(phase) {
            assert!((0.0..=1.0).contains(&v), "straight rgba in range");
        }
    }
}

#[cfg(feature = "glass")]
mod glass {
    use super::*;
    use crate::ui::orrery::layers;

    fn busy_snapshot() -> GraphSnapshot {
        let mut nodes = vec![
            node("a", &[], GraphNodePhase::Done),
            node("b", &["a"], GraphNodePhase::Done),
            node("c", &["a"], GraphNodePhase::Failed),
            node("d", &["b", "c"], GraphNodePhase::Running),
        ];
        // A crowd past the node cap, three woven deps each so the edges
        // blow past their own cap and it bites.
        for i in 0..MAX_NODES + 10 {
            nodes.push(node(&format!("n{i}"), &["a", "b", "c"], GraphNodePhase::Pending));
        }
        snapshot(nodes, GraphRunPhase::Running)
    }

    #[test]
    fn the_panel_is_sound_bounded_and_deterministic() {
        let busy = busy_snapshot();
        let grid = Grid { width: 1440, height: 1000 };
        let drawn = layers(1.0, grid, Some(&busy));
        assert!(drawn.len() > 60, "a busy graph draws its lights: {}", drawn.len());
        assert!(drawn.len() <= MAX_LAYERS, "{}", drawn.len());
        for layer in &drawn {
            assert!(layer.clone().sanitized().is_some(), "sound: {:?}", layer.id);
        }
        // The same moment is the same panel.
        assert_eq!(layers(1.0, grid, Some(&busy)), drawn);
        // Idle and hidden are sound too.
        let idle = layers(1.0, grid, None);
        assert!(!idle.is_empty() && idle.len() <= MAX_LAYERS);
        for layer in &idle {
            assert!(layer.clone().sanitized().is_some());
        }
        assert!(layers(1.0, Grid { width: 480, height: 500 }, Some(&busy)).is_empty());
        // The edge cap bites on a woven crowd: every shown crowd node
        // names three deps, far past the cap.
        let beams = drawn
            .iter()
            .filter(|l| matches!(l.kind, angel_glass::LayerKind::Solid([_, _, _, a]) if l.z == 5 && a < 0.6))
            .count();
        assert_eq!(beams, MAX_EDGES, "edges stop at the cap");
    }

    #[test]
    fn running_lights_lift_and_breathe() {
        let busy = busy_snapshot();
        let drawn = layers(0.0, Grid { width: 1440, height: 1000 }, Some(&busy));
        let lifting = drawn
            .iter()
            .filter(|l| l.height > 0.0 && l.breathe > 0.0 && l.flicker > 0.0)
            .count();
        assert!(lifting >= 1, "the running node flickers, lifts and breathes");
        for layer in &drawn {
            assert!(layer.height >= 0.0 && layer.height <= 6.0, "a small lift");
        }
    }
}

//! The Orrery: the live agent graph as a floating panel over the terminal
//! (docs/RENDER_FOUNDATION.md, "The layer sink").
//!
//! `ANGEL_GLASS_ORRERY=1` in the window spawns one thread that reads the
//! latest graph snapshot on its own cadence and hands the window a layer
//! set at 30 fps: a tilted glass plate, the nodes as breathing lights
//! arranged by dependency depth, edges with pulses travelling along them,
//! bracket chrome, a scan shimmer and drifting motes. The cockpit's loop
//! never draws it and never wakes for it — the window animates what it
//! can itself (`breathe`, `flicker`, `Particles`), and the thread only
//! re-describes what changed. In a terminal the module is inert.
//!
//! The panel is drawn in window pixels, not cells: a terminal cell is
//! twice as tall as it is wide, and a light meant to read as round must
//! be round. The panel's geometry, the depth rows and every pulse path
//! are pure functions of `(t, snapshot)` with no window types, so the
//! Orrery is testable in a plain test run; only the assembly into layers
//! needs the glass feature, and it, too, is deterministic.

use crate::agent::harness::{GraphNodePhase, GraphRunPhase, GraphSnapshot};

/// The Orrery's layer group ("ORRY"): replaced whole every frame it runs,
/// and cleared with an empty set when the panel hides (a small grid).
#[cfg(feature = "glass")]
const GROUP: u32 = 0x4F52_5259;

/// The most nodes the constellation draws; a larger graph keeps its first
/// nodes and dims the rest of the story. The panel is an overlay, not a
/// debugger.
pub(crate) const MAX_NODES: usize = 42;

/// The most edges drawn between the shown nodes.
pub(crate) const MAX_EDGES: usize = 96;

/// The whole panel's layers, whatever the graph: the chrome alone is a
/// dozen, the constellation is bounded above, and the cap keeps a wild
/// producer from turning the overlay into a particle blast.
pub(crate) const MAX_LAYERS: usize = 300;

/// The panel's lean, radians: enough to read as a held holo-plate, little
/// enough that rows still scan top-down. Everything except the floor
/// shadow and the scan sweep turns with it, about the panel's centre,
/// so the composition leans as one thing.
pub(crate) const TILT: f32 = -0.05;

/// How much smaller things far up the panel draw, per pixel above the
/// centre: the plate tips away, so its upper rows recede.
const DEPTH_PER_PX: f32 = 0.0005;

/// Frames a second the Orrery thread describes. The window presents at
/// its own pace; 30 is alive without costing like a game.
#[cfg(feature = "glass")]
const FPS: f32 = 30.0;

/// How often the thread re-reads the graph snapshot between frames.
#[cfg(feature = "glass")]
const REFRESH_SECS: f32 = 0.4;

/// Launch the Orrery thread if this launch asked for it
/// (`ANGEL_GLASS_ORRERY=1`). No-op in a terminal, twice-run safe.
pub(crate) fn launch_if_requested() {
    #[cfg(feature = "glass")]
    {
        use std::sync::OnceLock;
        static LAUNCHED: OnceLock<()> = OnceLock::new();
        if std::env::var("ANGEL_GLASS_ORRERY").is_ok_and(|v| v == "1")
            && LAUNCHED.set(()).is_ok()
            && let Some(host) = crate::ui::glass::host()
        {
            let host = host.clone();
            std::thread::Builder::new()
                .name("angel-orrery".into())
                .spawn(move || run(host))
                .ok();
        }
    }
}

#[cfg(feature = "glass")]
fn run(host: angel_glass::Host) {
    let started = std::time::Instant::now();
    let mut snapshot = crate::agent::harness::current_graph_snapshot();
    let mut refreshed = std::time::Instant::now();
    let frame = std::time::Duration::from_secs_f32(1.0 / FPS);
    // A hidden panel (a small grid) is sent once, not every frame: an
    // empty group on an empty group only wakes the window for nothing.
    let mut hidden_sent = false;
    loop {
        if refreshed.elapsed().as_secs_f32() >= REFRESH_SECS {
            snapshot = crate::agent::harness::current_graph_snapshot();
            refreshed = std::time::Instant::now();
        }
        let grid = host.grid_size();
        let t = started.elapsed().as_secs_f32();
        let layers = layers(
            t,
            Grid {
                width: grid.width_px,
                height: grid.height_px,
            },
            snapshot.as_ref(),
        );
        if layers.is_empty() {
            if !hidden_sent {
                host.set_layer_group(GROUP, layers);
                hidden_sent = true;
            }
        } else {
            hidden_sent = false;
            host.set_layer_group(GROUP, layers);
        }
        std::thread::sleep(frame);
    }
}

/// The window's size, in pixels: all the Orrery knows of the outside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct Grid {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// The panel plate, in window pixels: anchored to the top right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Panel {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
}

/// The panel's size in a 12 × 25 px cell world: these cells decide the
/// clamps, so the panel lands where the cell-space design put it while
/// everything inside it is free to be round.
const CELL: (f32, f32) = (12.0, 25.0);

impl Panel {
    /// The panel for a window, or `None` when it is too small to float
    /// one over its text without eating it.
    pub(crate) fn for_grid(grid: Grid) -> Option<Self> {
        let (cols, rows) = (grid.width as f32 / CELL.0, grid.height as f32 / CELL.1);
        if cols < 46.0 || rows < 22.0 {
            return None;
        }
        let w = (cols * 0.40).min(38.0) * CELL.0;
        let h = (rows * 0.34).min(17.0) * CELL.1;
        // Two cells clear of the right edge and below the header's three
        // rows: the lean lifts the top-right corner about a twentieth of
        // the width and the brackets reach past the plate, so a closer
        // anchor ran off the window and over the session's name.
        Some(Self {
            x: grid.width as f32 - w - 2.0 * CELL.0,
            y: 4.4 * CELL.1,
            w,
            h,
        })
    }

    /// Where the constellation draws: inside the chrome, below the title.
    pub(crate) fn content(self) -> Self {
        Self {
            x: self.x + 34.0,
            y: self.y + 64.0,
            w: (self.w - 68.0).max(40.0),
            h: (self.h - 108.0).max(40.0),
        }
    }

    pub(crate) fn centre(self) -> [f32; 2] {
        [self.x + self.w / 2.0, self.y + self.h / 2.0]
    }

    fn contains(self, p: [f32; 2], slack: f32) -> bool {
        p[0] >= self.x - slack
            && p[0] <= self.x + self.w + slack
            && p[1] >= self.y - slack
            && p[1] <= self.y + self.h + slack
    }
}

/// A colour per node phase: the palette the whole panel shares.
pub(crate) fn phase_color(phase: GraphNodePhase) -> [f32; 4] {
    match phase {
        GraphNodePhase::Pending => [0.22, 0.42, 0.66, 1.0],
        GraphNodePhase::Running => [1.0, 0.78, 0.34, 1.0],
        GraphNodePhase::Done => [0.20, 0.95, 0.72, 1.0],
        GraphNodePhase::Failed => [1.0, 0.24, 0.28, 1.0],
        GraphNodePhase::Skipped => [0.42, 0.47, 0.52, 1.0],
    }
}

/// The frame chrome's cyan and the plate it holds.
const CHROME: [f32; 4] = [0.35, 0.85, 1.0, 0.85];
const PLATE: [f32; 4] = [0.012, 0.03, 0.055, 0.72];

/// Each node's dependency depth in `snapshot`'s first [`MAX_NODES`]
/// nodes: the longest chain of dependencies above it, bounded like the
/// pane's own rows against a cyclic graph. Deps outside the shown set do
/// not count: an edge to nowhere has no depth.
pub(crate) fn node_depths(nodes: &[crate::agent::harness::GraphNodeSnap]) -> Vec<usize> {
    let shown = nodes.len().min(MAX_NODES);
    let index: std::collections::HashMap<&str, usize> = nodes[..shown]
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();
    let mut memo = vec![None; shown];
    fn depth_of(
        i: usize,
        nodes: &[crate::agent::harness::GraphNodeSnap],
        index: &std::collections::HashMap<&str, usize>,
        memo: &mut Vec<Option<usize>>,
        guard: usize,
    ) -> usize {
        if let Some(d) = memo[i] {
            return d;
        }
        let d = if guard == 0 {
            0
        } else {
            nodes[i]
                .deps
                .iter()
                .filter_map(|dep| index.get(dep.as_str()).copied())
                .map(|dep| depth_of(dep, nodes, index, memo, guard - 1) + 1)
                .max()
                .unwrap_or(0)
        };
        memo[i] = Some(d);
        d
    }
    (0..shown)
        .map(|i| depth_of(i, nodes, &index, &mut memo, shown))
        .collect()
}

/// Where each shown node's light sits in `content` (pixels): dependency
/// depth in rows top-down, a row's nodes spread across it. Pure: ranks
/// follow the node order, so the same snapshot gives the same
/// constellation.
pub(crate) fn node_positions(
    nodes: &[crate::agent::harness::GraphNodeSnap],
    content: Panel,
) -> Vec<[f32; 2]> {
    let depths = node_depths(nodes);
    let rows = depths.iter().copied().max().unwrap_or(0) + 1;
    let mut per_row = vec![0usize; rows];
    for &d in &depths {
        per_row[d] += 1;
    }
    let mut seen = vec![0usize; rows];
    depths
        .iter()
        .map(|&d| {
            let rank = seen[d];
            seen[d] += 1;
            [
                content.x + content.w * (rank + 1) as f32 / (per_row[d] + 1) as f32,
                content.y + content.h * (d as f32 + 0.5) / rows as f32,
            ]
        })
        .collect()
}

/// Where `[x, y, w, h]` goes when the panel leans [`TILT`] about `centre`,
/// with the turn to give a layer drawing it. The window turns a layer
/// about its own centre, so the layer keeps the rect's size and only its
/// centre turns round the panel's. (Placing the turned rect's bounding box
/// and turning that as well grew every quad by its lean: a 40-column plate
/// came out two rows taller and ran over the pane's own controls.)
pub(crate) fn tilted_rect(rect: [f32; 4], centre: [f32; 2]) -> ([f32; 4], f32) {
    tilted_rect_in(rect, centre, (1.0, 1.0))
}

/// [`tilted_rect`] for a rect in units of `unit` window pixels (cells for
/// a pane's frame): the turn happens in pixels, where the window turns the
/// quad, so a lean in cells of 12 x 25 px is the same lean as in pixels.
/// The quad keeps its own size: the window turns it about its own centre,
/// so only that centre moves round the panel's.
pub(crate) fn tilted_rect_in(rect: [f32; 4], centre: [f32; 2], unit: (f32, f32)) -> ([f32; 4], f32) {
    let [x, y, w, h] = rect;
    let (sin, cos) = TILT.sin_cos();
    let (dx, dy) = ((x + w / 2.0 - centre[0]) * unit.0, (y + h / 2.0 - centre[1]) * unit.1);
    let (rx, ry) = (
        centre[0] + (dx * cos - dy * sin) / unit.0,
        centre[1] + (dx * sin + dy * cos) / unit.1,
    );
    ([rx - w / 2.0, ry - h / 2.0, w, h], TILT)
}

/// Where point `p` lands when the panel leans [`TILT`] about `centre`:
/// the same turn [`tilted_rect`] puts a rect through. Segments lean by
/// leaning their endpoints, then measuring the line between them.
pub(crate) fn leaned_point(p: [f32; 2], centre: [f32; 2]) -> [f32; 2] {
    let (sin, cos) = TILT.sin_cos();
    let (dx, dy) = (p[0] - centre[0], p[1] - centre[1]);
    [centre[0] + dx * cos - dy * sin, centre[1] + dx * sin + dy * cos]
}

/// How much smaller things `p` far up the panel draw: the plate tips
/// away, so rows above its centre are further from the eye.
pub(crate) fn depth_scale(p: [f32; 2], centre: [f32; 2]) -> f32 {
    1.0 / (1.0 + DEPTH_PER_PX * (centre[1] - p[1]))
}

/// A point on the panel's own ground: turned with the lean and drawn at
/// its depth. The Orrery's interior (lights, orbits, edges) goes through
/// here, so the panel's content recedes with its outline; the chrome
/// stays on [`tilted_rect`], which only turns.
pub(crate) fn deep_point(p: [f32; 2], centre: [f32; 2]) -> [f32; 2] {
    let d = [p[0] - centre[0], p[1] - centre[1]];
    let s = depth_scale(p, centre);
    leaned_point([centre[0] + d[0] * s, centre[1] + d[1] * s], centre)
}

/// A rect drawn on the panel's ground: its centre at its depth, its
/// sides scaled by the same, then turned with the lean.
pub(crate) fn deep_rect(rect: [f32; 4], centre: [f32; 2]) -> ([f32; 4], f32) {
    let c = [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0];
    let s = depth_scale(c, centre);
    tilted_rect(
        [c[0] - rect[2] * s / 2.0, c[1] - rect[3] * s / 2.0, rect[2] * s, rect[3] * s],
        centre,
    )
}

/// A thin segment's rect and turn, from `a` to `b` (pixel points): a quad
/// the length of the line, turned to it, so a beam reads as a beam
/// however it slopes. `thick` is its height across, in pixels.
pub(crate) fn segment_rect(a: [f32; 2], b: [f32; 2], thick: f32) -> ([f32; 4], f32) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy).max(2.0);
    (
        [
            (a[0] + b[0]) / 2.0 - length / 2.0,
            (a[1] + b[1]) / 2.0 - thick / 2.0,
            length,
            thick,
        ],
        dy.atan2(dx),
    )
}

/// A pulse's place on the `a` → `b` edge at fraction `k` (0..1).
pub(crate) fn pulse_at(a: [f32; 2], b: [f32; 2], k: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k]
}

/// The idle ribbon's points across `content` at `t` (pixels): two slow
/// waves crossing, the Orrery's heartbeat while no graph has run.
pub(crate) fn ribbon_points(content: Panel, t: f32) -> Vec<[f32; 2]> {
    const DOTS: u16 = 22;
    (0..DOTS)
        .map(|i| {
            let k = f32::from(i) / f32::from(DOTS - 1);
            let x = content.x + content.w * k;
            let y = content.y + content.h * 0.62
                + ((x - content.x) * 0.02 - t * 1.4).sin() * content.h * 0.12
                + ((x - content.x) * 0.05 + t * 0.6).sin() * content.h * 0.05;
            [x, y]
        })
        .collect()
}

/// The whole panel at `t`. Deterministic: the same `t` and snapshot give
/// the same layers, in back-to-front order. An empty grid hides the
/// panel.
#[cfg(feature = "glass")]
fn layers(t: f32, grid: Grid, snapshot: Option<&GraphSnapshot>) -> Vec<angel_glass::Layer> {
    use angel_glass::{Layer, LayerKind, Placement};

    /// A quad in pixels, leaned with the panel.
    fn leaned(id: u64, z: i32, rect: [f32; 4], kind: LayerKind, centre: [f32; 2]) -> Layer {
        let (rect, angle) = tilted_rect(rect, centre);
        let mut layer = Layer::new(id, z, Placement::Pixels(rect), kind);
        layer.angle = angle;
        layer
    }

    let Some(panel) = Panel::for_grid(grid) else {
        return Vec::new();
    };
    let centre = panel.centre();
    let mut out: Vec<Layer> = Vec::with_capacity(140);

    // The cast shadow: on the terminal's floor, unturned, softer and
    // wider than the plate so the panel reads as held above the text.
    for (id, (dx, dy, w, h, a)) in [
        (0u64, (8.0f32, 24.0f32, panel.w, panel.h * 0.42, 0.16f32)),
        (1, (4.0, 12.0, panel.w, panel.h * 0.28, 0.22)),
    ] {
        out.push(Layer::new(
            id,
            0,
            Placement::Pixels([panel.x + dx, panel.y + dy, w, h]),
            LayerKind::Solid([0.0, 0.0, 0.0, a]),
        ));
    }
    // Light pooling under the plate's near edge: the panel's own glow
    // grounding it on the terminal's floor.
    let mut pool = Layer::new(
        62,
        0,
        Placement::Pixels([
            panel.x + 10.0,
            panel.y + panel.h + 2.0,
            panel.w - 20.0,
            panel.h * 0.38,
        ]),
        LayerKind::Glow([0.20, 0.55, 0.95, 0.15]),
    );
    pool.additive = true;
    out.push(pool);

    // The plate itself: leaned with everything on it, breathing a
    // little, translucent so the terminal's text shows through dimly,
    // with a rim of light all around reading as its edge.
    let mut plate = leaned(
        2,
        1,
        [panel.x, panel.y, panel.w, panel.h],
        LayerKind::Solid(PLATE),
        centre,
    );
    plate.breathe = 2.5;
    out.push(plate);
    let (bx, by) = (panel.x - 5.0, panel.y - 5.0);
    let (bw, bh) = (panel.w + 10.0, panel.h + 10.0);
    for (id, edge) in [
        (5u64, [bx, by, bw, 3.0]),
        (6, [bx, by + bh - 3.0, bw, 3.0]),
        (7, [bx, by, 3.0, bh]),
        (8, [bx + bw - 3.0, by, 3.0, bh]),
    ] {
        let mut rim = leaned(
            id,
            2,
            edge,
            LayerKind::Solid([CHROME[0], CHROME[1], CHROME[2], 0.65]),
            centre,
        );
        rim.additive = true;
        out.push(rim);
    }

    // A rim under the title strip: the panel's one steady light.
    let mut rim = leaned(
        3,
        2,
        [panel.x + 12.0, panel.y + 40.0, panel.w - 24.0, 2.5],
        LayerKind::Solid([CHROME[0], CHROME[1], CHROME[2], 0.4]),
        centre,
    );
    rim.additive = true;
    out.push(rim);

    // The run's status light, left of the title: gold and flickering
    // while work is under way, else a steady shard of the run's colour.
    let running = snapshot.is_some_and(|s| s.phase == GraphRunPhase::Running);
    let status = snapshot.map_or([0.3, 0.55, 0.8, 1.0], |s| match s.phase {
        GraphRunPhase::Running => [1.0, 0.78, 0.34, 1.0],
        GraphRunPhase::Done => [0.20, 0.95, 0.72, 1.0],
        GraphRunPhase::Failed => [1.0, 0.24, 0.28, 1.0],
        GraphRunPhase::Cancelled => [0.55, 0.55, 0.6, 1.0],
    });
    let mut light = leaned(
        4,
        3,
        [panel.x + 12.0, panel.y + 10.0, 30.0, 24.0],
        LayerKind::Glow(status),
        centre,
    );
    if running {
        light.flicker = 0.45;
    }
    out.push(light);

    // The content: the graph as a constellation, or an idle ribbon
    // waiting for one, inside the plate.
    let content = panel.content();
    match snapshot.map(|s| &s.nodes[..]).filter(|nodes| !nodes.is_empty()) {
        Some(nodes) => constellation(&mut out, nodes, content, centre, t),
        None => idle_ribbon(&mut out, content, centre, t),
    }

    // Bracket chrome: four L corners grown outward from the plate's
    // corners, leaned with it: the plate's frame, bright enough to read
    // as chrome and thin enough to stay light.
    let (x, y, w, h) = (panel.x, panel.y, panel.w, panel.h);
    let arm_w = (w * 0.20).min(66.0).max(24.0);
    let arm_h = (h * 0.20).min(44.0).max(14.0);
    let mut id = 600u64;
    for corner in [
        [x - 6.0, y - 6.0],
        [x + w + 6.0 - arm_w, y - 6.0],
        [x - 6.0, y + h + 4.0 - arm_h],
        [x + w + 6.0 - arm_w, y + h + 4.0 - arm_h],
    ] {
        for arm in [
            [corner[0], corner[1], arm_w, 3.0],
            [corner[0], corner[1], 3.0, arm_h],
        ] {
            let mut bracket = leaned(id, 7, arm, LayerKind::Solid(CHROME), centre);
            bracket.flicker = 0.08;
            out.push(bracket);
            id += 1;
        }
    }

    // The scan sweep: screen-space, top to bottom, a wide faint band of
    // light crossing the plate every few seconds.
    let sweep = (t * 0.14).fract();
    let mut sweep = Layer::new(
        60,
        9,
        Placement::Pixels([
            panel.x,
            panel.y + sweep * panel.h,
            panel.w,
            42.0_f32.min(panel.h * 0.16),
        ]),
        LayerKind::Solid([0.30, 0.70, 1.0, 0.14]),
    );
    sweep.additive = true;
    out.push(sweep);

    // Ambient motes rising through the panel, the window's own
    // animation: described once, moving without this thread.
    let mut motes = Layer::new(
        61,
        8,
        Placement::Pixels([
            panel.x + 8.0,
            panel.y + 8.0,
            panel.w - 16.0,
            panel.h - 16.0,
        ]),
        LayerKind::Particles(angel_glass::Particles {
            color: [0.45, 0.85, 1.0, 0.28],
            count: 14,
            size: 3.2,
            life: 5.0,
            rise: 22.0,
            sway: 4.0,
            seed: 0x0C5A_5EED,
            burst: None,
        }),
    );
    motes.additive = true;
    out.push(motes);

    out.truncate(MAX_LAYERS);
    out
}

/// The graph's nodes as lights and its edges as thin beams, `content` the
/// rect they draw in (pixels): dependency depth reads top-down, a row's
/// nodes spread across it, and every edge into a running node carries a
/// pulse while its source is done.
#[cfg(feature = "glass")]
fn constellation(
    out: &mut Vec<angel_glass::Layer>,
    nodes: &[crate::agent::harness::GraphNodeSnap],
    content: Panel,
    centre: [f32; 2],
    t: f32,
) {
    use angel_glass::{Layer, LayerKind, Placement};
    let at = node_positions(nodes, content);
    let index: std::collections::HashMap<&str, usize> = nodes
        .iter()
        .take(at.len())
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    // Edges first (under the lights): a dep's light to its node's, leaned
    // with the panel, brighter where work is arriving.
    let mut edges = 0usize;
    let mut pulses = 0usize;
    'edges: for (i, node) in nodes.iter().take(at.len()).enumerate() {
        for dep in &node.deps {
            let Some(&j) = index.get(dep.as_str()) else {
                continue;
            };
            if edges >= MAX_EDGES {
                break 'edges;
            }
            edges += 1;
            let lit = node.phase == GraphNodePhase::Running
                && nodes[j].phase == GraphNodePhase::Done;
            let color = phase_color(node.phase);
            let (a, b) = (deep_point(at[j], centre), deep_point(at[i], centre));
            let (rect, angle) = segment_rect(a, b, 2.4);
            let mut beam = Layer::new(
                100 + edges as u64,
                5,
                Placement::Pixels(rect),
                LayerKind::Solid([color[0], color[1], color[2], if lit { 0.55 } else { 0.30 }]),
            );
            beam.angle = angle.rem_euclid(std::f32::consts::TAU);
            beam.additive = true;
            out.push(beam);
            if lit && pulses < 24 {
                pulses += 1;
                let k = (t * 0.45 + 0.6180339 * ((i as u16 % 97) as f32)).fract();
                let p = pulse_at(a, b, k);
                let (rect, angle) = deep_rect([p[0] - 9.0, p[1] - 8.0, 18.0, 16.0], centre);
                let mut dot = Layer::new(
                    200 + pulses as u64,
                    6,
                    Placement::Pixels(rect),
                    LayerKind::Glow([0.85, 0.97, 1.0, 0.95]),
                );
                dot.angle = angle;
                dot.additive = true;
                out.push(dot);
            }
        }
    }

    // The lights: a soft halo in the phase's colour and a bright core;
    // running ones flicker, lift and breathe like held embers.
    for (i, node) in nodes.iter().take(at.len()).enumerate() {
        let color = phase_color(node.phase);
        let running = node.phase == GraphNodePhase::Running;
        let phase = ((i * 37) % 97) as f32 / 97.0;
        let (halo_rect, halo_angle) =
            deep_rect([at[i][0] - 26.0, at[i][1] - 13.0, 52.0, 26.0], centre);
        let mut halo = Layer::new(
            300 + i as u64,
            5,
            Placement::Pixels(halo_rect),
            LayerKind::Glow([color[0], color[1], color[2], if running { 0.9 } else { 0.6 }]),
        );
        halo.angle = halo_angle;
        halo.additive = true;
        let (core_rect, core_angle) =
            deep_rect([at[i][0] - 11.0, at[i][1] - 7.0, 22.0, 14.0], centre);
        let mut core = Layer::new(
            400 + i as u64,
            6,
            Placement::Pixels(core_rect),
            LayerKind::Glow([
                color[0] * 0.4 + 0.6,
                color[1] * 0.4 + 0.6,
                color[2] * 0.4 + 0.6,
                1.0,
            ]),
        );
        core.angle = core_angle;
        core.additive = true;
        if running {
            halo.flicker = 0.35;
            core.flicker = 0.5;
            halo.breathe = 1.5;
            core.height = ((t * 2.2 + phase * 9.0).sin() * 0.5 + 0.5) * 6.0;
            core.breathe = 2.0;
        }
        out.push(halo);
        out.push(core);
    }
}

/// The idle panel: a slow wave the width of the content under two orbit
/// rings turning at their own speeds — the Orrery's heartbeat while no
/// graph has run, and the shape it is named for. Round things are round:
/// everything here is sized in window pixels.
#[cfg(feature = "glass")]
fn idle_ribbon(out: &mut Vec<angel_glass::Layer>, content: Panel, centre: [f32; 2], t: f32) {
    use angel_glass::{Layer, LayerKind, Placement};
    let mid = [content.x + content.w / 2.0, content.y + content.h / 2.0];
    let points: Vec<[f32; 2]> = ribbon_points(content, t)
        .into_iter()
        .map(|p| deep_point(p, centre))
        .collect();
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            let q = points[i - 1];
            let (rect, angle) = segment_rect(q, *p, 2.6);
            let mut strand = Layer::new(
                500 + i as u64,
                5,
                Placement::Pixels(rect),
                LayerKind::Solid([0.35, 0.75, 1.0, 0.45]),
            );
            strand.angle = angle.rem_euclid(std::f32::consts::TAU);
            strand.additive = true;
            out.push(strand);
        }
        let (rect, lean) = deep_rect([p[0] - 11.0, p[1] - 8.0, 22.0, 16.0], centre);
        let mut dot = Layer::new(
            550 + i as u64,
            6,
            Placement::Pixels(rect),
            LayerKind::Glow([0.55, 0.88, 1.0, 0.95]),
        );
        dot.angle = lean;
        dot.additive = true;
        dot.flicker = 0.25;
        out.push(dot);
    }
    // The two orbit rings, above the wave, turning slowly: their paths
    // drawn as dim elliptical strands, small lights along them, and
    // every few a planet — a wide flickering glow. A warm sun holds the
    // middle; the wave runs beneath it like the ecliptic.
    let (sun_rect, sun_lean) = deep_rect([mid[0] - 44.0, mid[1] - 30.0, 88.0, 60.0], centre);
    let mut sun = Layer::new(
        619,
        6,
        Placement::Pixels(sun_rect),
        LayerKind::Glow([1.0, 0.68, 0.26, 0.55]),
    );
    sun.angle = sun_lean;
    sun.additive = true;
    sun.breathe = 2.5;
    sun.flicker = 0.12;
    out.push(sun);
    let (core_rect, core_lean) = deep_rect([mid[0] - 14.0, mid[1] - 10.0, 28.0, 20.0], centre);
    let mut sun_core = Layer::new(
        618,
        7,
        Placement::Pixels(core_rect),
        LayerKind::Glow([1.0, 0.9, 0.7, 0.95]),
    );
    sun_core.angle = core_lean;
    sun_core.additive = true;
    sun_core.flicker = 0.25;
    out.push(sun_core);
    for (ring, (rx, ry, speed, count)) in [
        (0u64, (content.w * 0.42, content.h * 0.26, 0.10f32, 30u16)),
        (1, (content.w * 0.27, content.h * 0.17, -0.16f32, 20u16)),
    ] {
        // The path itself: short strands end to end around the ellipse.
        const ARC: usize = 10;
        let point = |k: f32| deep_point([mid[0] + rx * k.cos(), mid[1] + ry * k.sin()], centre);
        for i in 0..ARC {
            let a0 = std::f32::consts::TAU * i as f32 / ARC as f32;
            let a1 = std::f32::consts::TAU * (i + 1) as f32 / ARC as f32;
            let (p, q) = (point(a0), point(a1));
            let (rect, angle) = segment_rect(p, q, 2.0);
            let mut strand = Layer::new(
                610 + ring * 30 + i as u64,
                5,
                Placement::Pixels(rect),
                LayerKind::Solid([0.30, 0.62, 0.95, 0.22]),
            );
            strand.angle = angle.rem_euclid(std::f32::consts::TAU);
            strand.additive = true;
            out.push(strand);
        }
        let turn = t * speed;
        for i in 0..count {
            let a = std::f32::consts::TAU * f32::from(i) / f32::from(count) + turn;
            let p = deep_point([mid[0] + rx * a.cos(), mid[1] + ry * a.sin()], centre);
            let fade = 0.4 + 0.5 * (a * 2.0).cos().abs();
            let planet = i % 7 == 3;
            let (w, h) = if planet { (34.0, 24.0) } else { (10.0, 8.0) };
            let (rect, lean) = deep_rect([p[0] - w / 2.0, p[1] - h / 2.0, w, h], centre);
            let mut mote = Layer::new(
                620 + ring * 40 + u64::from(i),
                6,
                Placement::Pixels(rect),
                LayerKind::Glow([0.45, 0.80, 1.0, if planet { 0.95 } else { fade }]),
            );
            mote.angle = lean;
            mote.additive = true;
            if planet {
                mote.flicker = 0.25;
                mote.breathe = 1.2;
            }
            out.push(mote);
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/orrery__tests.rs"]
mod tests;

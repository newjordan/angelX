//! Farmers commission actual graph calls; sprites deliver their native receipts.
use super::{
    View,
    ink::{Img, hash},
    kit,
    map::{TILE, place_px},
};
use crate::agent::harness::{ToolEventId, ToolOutcome};
use crate::knowledge::graph_crop::{Chart, ChartKind, GraphEvent, GraphRequest, MAX_PLOTS};
use crate::ui::viz::lifecycle_viz::MotionMode;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const GROW: u64 = 48;
const DELIVERY: u64 = 28;
const HOLD: u64 = 90;
const MAX_JOBS: usize = 128;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Bed {
    pub(crate) id: String,
    pub(crate) revision: u64,
    pub(crate) chart: Option<Chart>,
    pub(crate) reset_at: u64,
    pub(crate) planted: BTreeMap<u8, u64>,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpriteCall {
    pub(crate) id: ToolEventId,
    pub(crate) request: GraphRequest,
    pub(crate) started: u64,
    pub(crate) returned: Option<(u64, bool)>,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GardenFrame {
    pub(crate) bed: Option<Bed>,
    pub(crate) sprites: Vec<SpriteCall>,
    pub(crate) epoch: u64,
    pub(crate) tick: u64,
    pub(crate) motion: MotionMode,
}
impl Default for GardenFrame {
    fn default() -> Self {
        Self {
            bed: None,
            sprites: Vec::new(),
            epoch: 0,
            tick: 0,
            motion: MotionMode::Full,
        }
    }
}

#[derive(Default)]
pub(crate) struct Garden {
    beds: BTreeMap<String, Bed>,
    jobs: BTreeMap<ToolEventId, SpriteCall>,
    completed: BTreeSet<ToolEventId>,
    order: VecDeque<ToolEventId>,
    selected: Option<String>,
    epoch: u64,
}
impl Garden {
    pub(crate) fn note(&mut self, id: &ToolEventId, event: &GraphEvent, tick: u64) {
        self.prune(tick);
        match event {
            GraphEvent::Requested(request) => {
                if request.validate().is_err()
                    || self.completed.contains(id)
                    || self.jobs.contains_key(id)
                {
                    return;
                }
                // Hidden Stage time does not advance. Evict an already settled
                // animation before admitting another real call so data keeps
                // updating even after hundreds of calls while the pane is shut.
                if self.jobs.len() >= MAX_JOBS {
                    let oldest = self
                        .jobs
                        .iter()
                        .filter(|(_, j)| j.returned.is_some())
                        .min_by_key(|(_, j)| j.returned.unwrap().0)
                        .map(|(id, _)| id.clone());
                    if let Some(oldest) = oldest {
                        self.remember_completed(oldest);
                    } else {
                        return;
                    }
                }
                self.jobs.insert(
                    id.clone(),
                    SpriteCall {
                        id: id.clone(),
                        request: request.clone(),
                        started: tick,
                        returned: None,
                    },
                );
                self.epoch += 1;
            }
            GraphEvent::Returned(receipt) => {
                let Some(job) = self.jobs.get_mut(id) else {
                    return;
                };
                if job.returned.is_some() || job.request != receipt.request {
                    return;
                }
                job.returned = Some((tick, true));
                let born = tick.max(job.started.saturating_add(DELIVERY));
                self.epoch += 1;
                let plot = receipt.request.plot().to_string();
                if !self.beds.contains_key(&plot) && self.beds.len() >= MAX_PLOTS {
                    return;
                }
                let old = self.beds.get(&plot);
                if old.is_some_and(|bed| bed.revision >= receipt.revision) {
                    return;
                }
                let same_generation = old
                    .and_then(|b| b.chart.as_ref())
                    .zip(receipt.chart.as_ref())
                    .is_some_and(|(a, b)| a.generation == b.generation);
                let mut planted = if same_generation {
                    old.unwrap().planted.clone()
                } else {
                    BTreeMap::new()
                };
                if let Some(chart) = &receipt.chart {
                    for (&index, point) in &chart.points {
                        let unchanged = same_generation
                            && old
                                .and_then(|b| b.chart.as_ref())
                                .and_then(|c| c.points.get(&index))
                                == Some(point);
                        if !unchanged {
                            planted.insert(index, born);
                        }
                    }
                }
                let reset_at = if same_generation {
                    old.unwrap().reset_at
                } else {
                    tick
                };
                self.beds.insert(
                    plot.clone(),
                    Bed {
                        id: plot.clone(),
                        revision: receipt.revision,
                        chart: receipt.chart.clone(),
                        reset_at,
                        planted,
                    },
                );
                self.selected = Some(plot);
                self.epoch += 1;
            }
        }
    }
    /// The ordinary ToolResult also closes denied, cancelled, or failed requests.
    pub(crate) fn settle(&mut self, id: &ToolEventId, _outcome: ToolOutcome, tick: u64) {
        if let Some(job) = self.jobs.get_mut(id)
            && job.returned.is_none()
        {
            // Success without a native receipt is not enough to grow anything.
            job.returned = Some((tick, false));
            self.epoch += 1;
        }
    }
    pub(crate) fn abandon(&mut self, tick: u64) {
        for job in self.jobs.values_mut().filter(|j| j.returned.is_none()) {
            job.returned = Some((tick, false));
        }
        self.epoch += 1;
    }
    fn prune(&mut self, tick: u64) {
        let expired: Vec<_> = self
            .jobs
            .iter()
            .filter(|(_, j)| {
                j.returned
                    .is_some_and(|(at, _)| tick >= at.max(j.started + DELIVERY) + HOLD)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            self.remember_completed(id);
        }
    }
    fn remember_completed(&mut self, id: ToolEventId) {
        self.jobs.remove(&id);
        self.completed.insert(id.clone());
        self.order.push_back(id);
        while self.order.len() > 512 {
            if let Some(id) = self.order.pop_front() {
                self.completed.remove(&id);
            }
        }
    }
    pub(crate) fn working(&self) -> bool {
        self.jobs.values().any(|j| j.returned.is_none())
    }
    pub(crate) fn animating(&self, tick: u64) -> bool {
        self.jobs.values().any(|j| {
            j.returned
                .is_none_or(|(at, _)| tick < at.max(j.started + DELIVERY) + HOLD)
        }) || self
            .beds
            .values()
            .any(|b| tick < b.reset_at + GROW || b.planted.values().any(|at| tick < at + GROW))
    }
    pub(crate) fn frame(&self, tick: u64) -> GardenFrame {
        if self.beds.is_empty() {
            return GardenFrame::default();
        }
        let bed = self
            .selected
            .as_ref()
            .and_then(|id| self.beds.get(id))
            .cloned();
        let sprites = self
            .jobs
            .values()
            .filter(|j| {
                bed.as_ref().is_some_and(|b| b.id == j.request.plot())
                    && j.returned
                        .is_none_or(|(at, _)| tick < at.max(j.started + DELIVERY) + HOLD)
            })
            .cloned()
            .collect();
        let end = self
            .jobs
            .values()
            .filter_map(|j| {
                j.returned
                    .map(|(at, _)| at.max(j.started + DELIVERY) + HOLD)
            })
            .chain(self.beds.values().flat_map(|b| {
                std::iter::once(b.reset_at + GROW).chain(b.planted.values().map(|at| at + GROW))
            }))
            .max()
            .unwrap_or(0);
        // Match the world's restrained animation cadence and freeze the garden
        // fingerprint after delivery. An empty or grown garden adds no repaint tax.
        let shown_tick = if self.working() {
            tick / 4 * 4
        } else if tick >= end {
            end
        } else {
            tick / 4 * 4
        };
        GardenFrame {
            bed,
            sprites,
            epoch: self.epoch,
            tick: shown_tick,
            motion: MotionMode::Full,
        }
    }
    pub(crate) fn select(&mut self, id: &str) -> bool {
        if !self.beds.contains_key(id) {
            return false;
        }
        self.selected = Some(id.into());
        self.epoch += 1;
        true
    }
    pub(crate) fn report(&self) -> String {
        let Some(bed) = self.selected.as_ref().and_then(|id| self.beds.get(id)) else {
            return "Graph garden is bare. Ask the harness for a chart using graph: begin, one point call per observation, then finish. /world visit garden shows the fields.".into();
        };
        let mut out = format!(
            "Graph garden · plots: {} · selected {}\n",
            self.beds.keys().cloned().collect::<Vec<_>>().join(", "),
            bed.id
        );
        let Some(chart) = &bed.chart else {
            out.push_str("Bare dirt · waiting for a new graph generation.");
            return out;
        };
        out.push_str(&format!(
            "{} · {:?} · generation {} · {}/{} points · {}\n{}: {}..{} · {}: {}..{}\n",
            chart.spec.title,
            chart.spec.kind,
            chart.generation,
            chart.points.len(),
            chart.spec.expected_points,
            if chart.finished {
                "finished"
            } else {
                "growing; missing points stay empty"
            },
            chart.spec.x_label,
            chart.spec.x_min,
            chart.spec.x_max,
            chart.spec.y_label,
            chart.spec.y_min,
            chart.spec.y_max
        ));
        for (index, point) in &chart.points {
            out.push_str(&format!(
                "  [{index}] {} · x={} y={}\n",
                point.label, point.x, point.y
            ));
        }
        out
    }
}

/// The existing south-east field screen, with room for axes and a farmer.
pub(crate) fn origin() -> (i32, i32) {
    place_px(33 * TILE, 23 * TILE)
}
pub(crate) fn centre() -> (f32, f32) {
    let (x, y) = origin();
    ((x + 112) as f32, (y + 72) as f32)
}
const W: i32 = 224;
const H: i32 = 136;
const LEFT: i32 = 44;
const RIGHT: i32 = W - 16;
const TOP: i32 = 25;
const BOTTOM: i32 = 98;
fn px(chart: &Chart, x: f64) -> i32 {
    LEFT + (((x - chart.spec.x_min) / (chart.spec.x_max - chart.spec.x_min))
        * f64::from(RIGHT - LEFT))
    .round() as i32
}
fn py(chart: &Chart, y: f64) -> i32 {
    BOTTOM
        - (((y - chart.spec.y_min) / (chart.spec.y_max - chart.spec.y_min))
            * f64::from(BOTTOM - TOP))
        .round() as i32
}
pub(crate) fn point_position(chart: &Chart, x: f64, y: f64) -> (i32, i32) {
    (px(chart, x), py(chart, y))
}
fn short(text: &str, length: usize) -> String {
    text.chars()
        .take(length)
        .collect::<String>()
        .to_ascii_uppercase()
}
fn growth(frame: &GardenFrame, index: u8) -> f64 {
    if frame.motion != MotionMode::Full {
        return 1.0;
    }
    let at = frame
        .bed
        .as_ref()
        .and_then(|b| b.planted.get(&index))
        .copied()
        .unwrap_or(frame.tick);
    (frame.tick.saturating_sub(at) as f64 / GROW as f64).clamp(0.0, 1.0)
}

/// Paint on the actual terrain before figures; no independent chart panel.
pub(crate) fn paint_ground(cv: &mut Img, frame: &GardenFrame, view: View) {
    let Some(bed) = &frame.bed else {
        return;
    };
    let (wx, wy) = origin();
    if wx >= view.x + view.w || wy >= view.y + view.h || wx + W <= view.x || wy + H <= view.y {
        return;
    }
    let mut im = Img::black(W, H);
    for y in 0..H {
        for x in 0..W {
            if y % 5 == 0 && hash(x, y, 91).is_multiple_of(4) {
                im.put(x, y, 'B');
            }
        }
    }
    im.text(
        4,
        4,
        &short(
            bed.chart
                .as_ref()
                .map_or("BARE DIRT", |c| c.spec.title.as_str()),
            35,
        ),
        'H',
    );
    if let Some(chart) = &bed.chart {
        im.line(LEFT, TOP, LEFT, BOTTOM, 'G');
        im.line(LEFT, BOTTOM, RIGHT, BOTTOM, 'G');
        if chart.spec.y_min < 0.0 && chart.spec.y_max > 0.0 {
            let zero = py(chart, 0.0);
            im.line(LEFT, zero, RIGHT, zero, 'j');
            im.text(LEFT - 12, zero - 3, "0", 'i');
        }
        for (value, y) in [(chart.spec.y_min, BOTTOM), (chart.spec.y_max, TOP)] {
            im.line(LEFT - 3, y, LEFT, y, 'G');
            im.text(2, y - 3, &short(&format!("{value:.1}"), 6), 'i');
        }
        im.text(LEFT, BOTTOM + 10, &short(&chart.spec.x_label, 26), 'i');
        im.text(LEFT, 15, &short(&chart.spec.y_label, 26), 'i');
        // Explicit x scale end points, kept inside the bed.
        im.text(
            LEFT,
            BOTTOM + 2,
            &short(&format!("{}", chart.spec.x_min), 8),
            'i',
        );
        let max_text = short(&format!("{}", chart.spec.x_max), 8);
        im.text(
            RIGHT - (max_text.len() as i32) * 6,
            BOTTOM + 2,
            &max_text,
            'i',
        );
        let mut line_points: Vec<_> = chart.points.iter().collect();
        line_points.sort_by(|(ia, a), (ib, b)| a.x.total_cmp(&b.x).then(ia.cmp(ib)));
        let mut previous = None;
        for (&index, point) in line_points {
            let g = growth(frame, index);
            if g == 0.0 {
                previous = None;
                continue;
            }
            let (x, y) = point_position(chart, point.x, point.y);
            match chart.spec.kind {
                ChartKind::Bar => {
                    let zero = py(chart, 0.0);
                    let grown = zero + ((y - zero) as f64 * g).round() as i32;
                    let half =
                        (f64::from(RIGHT - LEFT) / f64::from(chart.spec.expected_points) / 3.0)
                            .floor()
                            .clamp(1.0, 9.0) as i32;
                    for by in grown.min(zero)..=grown.max(zero) {
                        for bx in (x - half).max(LEFT)..=(x + half).min(RIGHT) {
                            im.put(
                                bx,
                                by,
                                if (bx + by) % 4 == 0 {
                                    'Y'
                                } else if by % 3 == 0 {
                                    'E'
                                } else {
                                    'M'
                                },
                            );
                        }
                    }
                    im.line(
                        (x - half).max(LEFT),
                        grown,
                        (x + half).min(RIGHT),
                        grown,
                        'C',
                    );
                }
                ChartKind::Line => {
                    if let Some((ax, ay)) = previous {
                        im.line(ax, ay, x, y, 'C');
                    }
                    im.line(x, y + 1, x, (y + 4).min(BOTTOM), 'E');
                    im.put(x, y, 'Y');
                    if g > 0.5 {
                        for (dx, dy) in [(-1, 0), (1, 0), (0, -1)] {
                            im.put(
                                (x + dx).clamp(LEFT, RIGHT),
                                (y + dy).clamp(TOP, BOTTOM),
                                'Y',
                            );
                        }
                    }
                    previous = Some((x, y));
                }
                ChartKind::Scatter => {
                    im.line(x, y, x, (y + 4).min(BOTTOM), 'E');
                    im.put(x, y, 'Y');
                    if g > 0.5 {
                        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                            im.put(
                                (x + dx).clamp(LEFT, RIGHT),
                                (y + dy).clamp(TOP, BOTTOM),
                                'C',
                            );
                        }
                    }
                }
            }
        }
    }
    // A bounded reset spell: the old geometry is already gone, only dirt sparkles.
    if frame.motion == MotionMode::Full && frame.tick < bed.reset_at + DELIVERY {
        for i in 0..12 {
            let v = hash(i, frame.tick as i32, 93);
            im.put(
                LEFT + (v as i32).rem_euclid(RIGHT - LEFT),
                TOP + ((v >> 8) as i32).rem_euclid(BOTTOM - TOP),
                '2',
            );
        }
    }
    cv.stamp(&im, wx - view.x, wy - view.y);
}

pub(crate) fn stage(
    frame: &GardenFrame,
    props: &mut Vec<super::scene::Prop>,
    cues: &mut Vec<super::scene::Prop>,
) {
    let Some(bed) = &frame.bed else {
        return;
    };
    let (ox, oy) = origin();
    let farmer = (ox + 24, oy + H - 12);
    let active = !frame.sprites.is_empty();
    let img = kit::field_hand(if active && frame.motion == MotionMode::Full {
        (frame.tick / 8) as u32
    } else {
        0
    });
    props.push(super::scene::Prop {
        x: farmer.0 - img.w / 2,
        base: farmer.1,
        img,
    });
    let speaker = frame
        .sprites
        .iter()
        .rposition(|j| matches!(j.request, GraphRequest::Point { .. }))
        .unwrap_or(0);
    for (n, job) in frame.sprites.iter().enumerate() {
        let generation_matches = match &job.request {
            GraphRequest::Point { generation, .. } | GraphRequest::Finish { generation, .. } => bed
                .chart
                .as_ref()
                .is_some_and(|c| c.generation == *generation),
            _ => true,
        };
        if !generation_matches {
            continue;
        }
        let elapsed = frame.tick.saturating_sub(job.started);
        let destination = match (&job.request, &bed.chart) {
            (GraphRequest::Point { point, .. }, Some(chart))
                if (chart.spec.x_min..=chart.spec.x_max).contains(&point.x)
                    && (chart.spec.y_min..=chart.spec.y_max).contains(&point.y) =>
            {
                let (x, y) = point_position(chart, point.x, point.y);
                (ox + x, oy + y)
            }
            // Rejected points have no crop coordinate. Keep their sprite at
            // the farmer rather than projecting an out-of-domain request.
            (GraphRequest::Point { .. }, _) => farmer,
            _ => (ox + W / 2, oy + TOP),
        };
        let t = if frame.motion == MotionMode::Full {
            (elapsed.saturating_sub(8) as f32 / DELIVERY as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let x = farmer.0 as f32 + (destination.0 - farmer.0) as f32 * t;
        let y = farmer.1 as f32 + (destination.1 - farmer.1) as f32 * t - 6.0;
        let returned = job.returned;
        let sprite_frame = if frame.motion == MotionMode::Full {
            (frame.tick / 5 + n as u64) as u32
        } else {
            0
        };
        let glyph = kit::graph_sprite(sprite_frame, returned.map(|(_, ok)| ok));
        cues.push(super::scene::Prop {
            x: x as i32 - 4,
            base: y as i32,
            img: glyph,
        });
        // Actual x/y values, not invented dialogue or simulated work calls.
        if n == speaker && (elapsed < 40 || returned.is_some_and(|(at, _)| frame.tick < at + 24)) {
            let words = match (&job.request, returned) {
                (GraphRequest::Point { point, .. }, _) if elapsed < 8 => format!("X={}?", point.x),
                (GraphRequest::Point { point, .. }, None) => format!("X={}?", point.x),
                (GraphRequest::Point { point, .. }, Some((_, true))) => format!("Y={}", point.y),
                (_, Some((_, false))) => "NO DATA".into(),
                (GraphRequest::Begin { .. }, _) | (GraphRequest::Clear { .. }, _) => {
                    "SOIL SPELL".into()
                }
                _ => "SEAL GRAPH".into(),
            };
            let words = short(&words, 20);
            let mut bubble = Img::black(words.len() as i32 * 6 + 6, 13);
            bubble.text(3, 3, &words, 'H');
            let (x, base) = if returned.is_none() || elapsed < 8 {
                (farmer.0 - 6, farmer.1 - 14)
            } else {
                (x as i32, y as i32 - 10)
            };
            cues.push(super::scene::Prop {
                x,
                base,
                img: bubble,
            });
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/overworld__garden_tests.rs"]
mod tests;

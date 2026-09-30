//! Bounded chart data shared by the graph tool and its truthful world projection.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const MAX_PLOTS: usize = 4;
pub(crate) const MAX_POINTS: u8 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Bar,
    Line,
    Scatter,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartSpec {
    pub(crate) title: String,
    pub(crate) kind: ChartKind,
    pub(crate) x_label: String,
    pub(crate) y_label: String,
    pub(crate) x_min: f64,
    pub(crate) x_max: f64,
    pub(crate) y_min: f64,
    pub(crate) y_max: f64,
    pub(crate) expected_points: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataPoint {
    pub(crate) label: String,
    pub(crate) x: f64,
    pub(crate) y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum GraphRequest {
    Begin {
        plot: String,
        spec: ChartSpec,
    },
    Point {
        plot: String,
        generation: u64,
        index: u8,
        point: DataPoint,
    },
    Finish {
        plot: String,
        generation: u64,
    },
    Clear {
        plot: String,
    },
}

impl GraphRequest {
    pub(crate) fn plot(&self) -> &str {
        match self {
            Self::Begin { plot, .. }
            | Self::Point { plot, .. }
            | Self::Finish { plot, .. }
            | Self::Clear { plot } => plot,
        }
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        let plot = self.plot();
        if plot.is_empty()
            || plot.len() > 32
            || !plot
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err("plot must be 1-32 lowercase letters, digits, or hyphens".into());
        }
        match self {
            Self::Begin { spec, .. } => spec.validate(),
            Self::Point {
                generation,
                index,
                point,
                ..
            } => {
                if *generation == 0
                    || *index >= MAX_POINTS
                    || !text_valid(&point.label, 32)
                    || !number(point.x)
                    || !number(point.y)
                {
                    return Err("point needs a generation, index 0-31, a short label, and finite coordinates within +/-1e12".into());
                }
                Ok(())
            }
            Self::Finish { generation, .. } if *generation == 0 => {
                Err("generation must be positive".into())
            }
            _ => Ok(()),
        }
    }
}

fn number(n: f64) -> bool {
    n.is_finite() && n.abs() <= 1e12
}
pub(crate) fn text_valid(s: &str, max: usize) -> bool {
    !s.trim().is_empty()
        && s.len() <= max
        && !s.chars().any(|c| {
            c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
}
impl ChartSpec {
    fn validate(&self) -> Result<(), String> {
        if !text_valid(&self.title, 64)
            || !text_valid(&self.x_label, 32)
            || !text_valid(&self.y_label, 32)
            || !(1..=MAX_POINTS).contains(&self.expected_points)
            || ![self.x_min, self.x_max, self.y_min, self.y_max]
                .into_iter()
                .all(number)
            || self.x_min >= self.x_max
            || self.y_min >= self.y_max
        {
            return Err("chart needs short labels, 1-32 points, and finite increasing x/y domains within +/-1e12".into());
        }
        if self.kind == ChartKind::Bar && !(self.y_min <= 0.0 && self.y_max >= 0.0) {
            return Err("bar chart y domain must include zero".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chart {
    pub(crate) generation: u64,
    pub(crate) spec: ChartSpec,
    pub(crate) points: BTreeMap<u8, DataPoint>,
    pub(crate) finished: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphReceipt {
    pub(crate) revision: u64,
    pub(crate) request: GraphRequest,
    /// Authoritative post-call snapshot, including a tombstone after clear.
    pub(crate) chart: Option<Chart>,
}

#[derive(Default)]
pub(crate) struct GraphStore {
    plots: BTreeMap<String, Option<Chart>>,
    revision: u64,
}
impl GraphStore {
    pub(crate) fn apply(&mut self, request: GraphRequest) -> Result<GraphReceipt, String> {
        request.validate()?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("graph revision exhausted")?;
        let plot = request.plot().to_string();
        let chart = match &request {
            GraphRequest::Begin { spec, .. } => {
                if !self.plots.contains_key(&plot) && self.plots.len() >= MAX_PLOTS {
                    return Err("garden has four plots; reuse an existing plot id".into());
                }
                Some(Chart {
                    generation: revision,
                    spec: spec.clone(),
                    points: BTreeMap::new(),
                    finished: false,
                })
            }
            GraphRequest::Clear { .. } => {
                if !self.plots.contains_key(&plot) {
                    return Err("unknown plot".into());
                }
                None
            }
            GraphRequest::Point {
                generation,
                index,
                point,
                ..
            } => {
                let mut chart = self.current(&plot, *generation)?.clone();
                if chart.finished {
                    return Err("chart is finished; begin a new generation to regrow it".into());
                }
                if *index >= chart.spec.expected_points
                    || point.x < chart.spec.x_min
                    || point.x > chart.spec.x_max
                    || point.y < chart.spec.y_min
                    || point.y > chart.spec.y_max
                {
                    return Err("point lies outside the declared count or domain; begin with the correct scale".into());
                }
                chart.points.insert(*index, point.clone());
                Some(chart)
            }
            GraphRequest::Finish { generation, .. } => {
                let mut chart = self.current(&plot, *generation)?.clone();
                if chart.points.len() != usize::from(chart.spec.expected_points) {
                    return Err("chart still has missing data points".into());
                }
                chart.finished = true;
                Some(chart)
            }
        };
        self.revision = revision;
        self.plots.insert(plot, chart.clone());
        Ok(GraphReceipt {
            revision,
            request,
            chart,
        })
    }
    fn current(&self, plot: &str, generation: u64) -> Result<&Chart, String> {
        let chart = self
            .plots
            .get(plot)
            .and_then(Option::as_ref)
            .ok_or("prepare the plot with begin first")?;
        if generation != chart.generation {
            return Err("stale generation; use the generation returned by begin".into());
        }
        Ok(chart)
    }
}

/// Dedicated telemetry retains the real arguments and receipt, not truncated prose.
#[derive(Clone, Debug)]
pub enum GraphEvent {
    Requested(GraphRequest),
    Returned(GraphReceipt),
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/graph_crop__tests.rs"]
mod tests;

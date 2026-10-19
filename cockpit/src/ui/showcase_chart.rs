//! Charts for the showcase: one crisp picture per chart, drawn here at
//! the window's size and handed over as pixels, then swept in by the
//! wizard (`angel_glass`'s `wizard.rs`).
//!
//! A chart comes from the `graph` tool's finished plot, or is read out of
//! data the agent presents ("smart graphs"): a CSV or JSON file whose
//! columns are numbers becomes a chart instead of a page of text. Labels
//! against values make bars; numbers against numbers make lines, a few
//! series at once.
//!
//! The look is the panel's own: no paper (it is drawn on the showcase's
//! glass), tonal grid, grey ink, the
//! data in the one warm accent (further series in the cool greys beside
//! it). Strokes and dots are anti-aliased by distance, by hand; the text
//! is the window's font through the presentation surface's pager.

use crate::knowledge::graph_crop::{Chart, ChartKind};

/// One chart, however it was found.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Plot {
    pub(crate) title: String,
    pub(crate) kind: ChartKind,
    pub(crate) x_label: String,
    pub(crate) y_label: String,
    /// One label per point (bars name their bars with it).
    pub(crate) labels: Vec<String>,
    pub(crate) xs: Vec<f64>,
    /// Each series' name and values, one per x.
    pub(crate) series: Vec<(String, Vec<f64>)>,
    pub(crate) x_domain: (f64, f64),
    pub(crate) y_domain: (f64, f64),
}

/// The most points a smart graph draws, and the most bars and series.
const MOST_POINTS: usize = 4000;
const MOST_BARS: usize = 60;
const MOST_SERIES: usize = 4;

impl Plot {
    /// The `graph` tool's chart, as planted.
    pub(crate) fn from_chart(chart: &Chart) -> Self {
        let spec = &chart.spec;
        let points: Vec<_> = chart.points.values().collect();
        Self {
            title: spec.title.clone(),
            kind: spec.kind,
            x_label: spec.x_label.clone(),
            y_label: spec.y_label.clone(),
            labels: points.iter().map(|p| p.label.clone()).collect(),
            xs: points.iter().map(|p| p.x).collect(),
            series: vec![(spec.y_label.clone(), points.iter().map(|p| p.y).collect())],
            x_domain: (spec.x_min, spec.x_max),
            y_domain: (spec.y_min, spec.y_max),
        }
    }

    /// A chart read out of a data file's text, when it holds one: CSV
    /// (with a header row) or JSON (numbers, `[[x, y], ...]`, or rows of
    /// objects). `None` for anything that is not mostly numbers.
    pub(crate) fn from_data(title: &str, text: &str) -> Option<Self> {
        let trimmed = text.trim_start();
        let table = if trimmed.starts_with('[') || trimmed.starts_with('{') {
            json_table(trimmed)?
        } else {
            csv_table(text)?
        };
        Self::from_table(title, table)
    }

    fn from_table(title: &str, table: Table) -> Option<Self> {
        let Table { header, rows } = table;
        if rows.len() < 2 || header.is_empty() {
            return None;
        }
        let numeric = |col: usize| {
            let filled = rows.iter().filter(|r| r.get(col).is_some_and(|c| !c.is_empty())).count();
            let numbers = rows.iter().filter(|r| r.get(col).and_then(|c| number(c)).is_some()).count();
            filled > 0 && numbers * 10 >= filled * 9
        };
        let value_cols: Vec<usize> = (0..header.len()).filter(|&c| numeric(c)).collect();
        if value_cols.is_empty() {
            return None;
        }
        let first_is_value = value_cols.first() == Some(&0);
        let rows = &rows[..rows.len().min(MOST_POINTS)];
        let (kind, x_col, labels, xs) = if !first_is_value && header.len() > 1 {
            // Labels, then values: bars, one per label.
            let rows = &rows[..rows.len().min(MOST_BARS)];
            let labels: Vec<String> = rows.iter().map(|r| r.first().cloned().unwrap_or_default()).collect();
            let xs = (0..rows.len()).map(|i| i as f64).collect();
            (ChartKind::Bar, None, labels, xs)
        } else if value_cols.len() >= 2 {
            // Numbers against numbers: lines over the first column.
            let xs: Vec<f64> = rows.iter().map(|r| r.first().and_then(|c| number(c)).unwrap_or(f64::NAN)).collect();
            (ChartKind::Line, Some(0), xs.iter().map(|x| short(*x)).collect(), xs)
        } else {
            // One column of numbers: a line over their order.
            let xs: Vec<f64> = (0..rows.len()).map(|i| i as f64 + 1.0).collect();
            (ChartKind::Line, None, xs.iter().map(|x| short(*x)).collect(), xs)
        };
        let n = xs.len();
        let series: Vec<(String, Vec<f64>)> = value_cols
            .iter()
            .filter(|&&c| Some(c) != x_col)
            .take(MOST_SERIES)
            .map(|&c| {
                let values = rows[..n]
                    .iter()
                    .map(|r| r.get(c).and_then(|v| number(v)).unwrap_or(f64::NAN))
                    .collect();
                (header[c].clone(), values)
            })
            .collect();
        if series.is_empty() {
            return None;
        }
        let finite = |v: &f64| v.is_finite();
        let (mut lo, mut hi) = series
            .iter()
            .flat_map(|(_, v)| v.iter().copied().filter(finite))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
        if !lo.is_finite() {
            return None;
        }
        if kind == ChartKind::Bar {
            lo = lo.min(0.0);
            hi = hi.max(0.0);
        }
        let (lo, hi) = nice_domain(lo, hi);
        let (xlo, xhi) = xs
            .iter()
            .copied()
            .filter(finite)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
        let x_domain = if xlo.is_finite() && xhi > xlo { (xlo, xhi) } else { (0.0, 1.0) };
        Some(Self {
            title: title.to_string(),
            kind,
            x_label: match x_col {
                Some(c) => header[c].clone(),
                None if kind == ChartKind::Bar => header[0].clone(),
                None => String::new(),
            },
            y_label: if series.len() == 1 { series[0].0.clone() } else { String::new() },
            labels,
            xs,
            series,
            x_domain,
            y_domain: (lo, hi),
        })
    }
}

impl Plot {
    /// The chart's data as CSV: the x (or the bar's name), then each
    /// series, a header row first.
    pub(crate) fn csv(&self) -> String {
        let field = |text: &str| {
            if text.contains([',', '"', '\n']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text.to_string() }
        };
        let x_name = if self.x_label.is_empty() { "x".to_string() } else { self.x_label.clone() };
        let mut out = std::iter::once(field(&x_name))
            .chain(self.series.iter().map(|(name, _)| field(name)))
            .collect::<Vec<_>>()
            .join(",");
        out.push('\n');
        for i in 0..self.xs.len() {
            let x = if self.kind == ChartKind::Bar {
                field(self.labels.get(i).map_or("", String::as_str))
            } else {
                short(self.xs[i])
            };
            let row = std::iter::once(x)
                .chain(self.series.iter().map(|(_, v)| v.get(i).filter(|v| v.is_finite()).map(|v| v.to_string()).unwrap_or_default()))
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&row);
            out.push('\n');
        }
        out
    }
}

struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Comma, tab or semicolon separated, the first row the header. Quotes
/// around a field are dropped (no embedded separators: data, not prose).
fn csv_table(text: &str) -> Option<Table> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let head = lines.next()?;
    let sep = [',', '\t', ';']
        .into_iter()
        .max_by_key(|s| head.matches(*s).count())
        .filter(|s| head.contains(*s))?;
    let split = |line: &str| -> Vec<String> {
        line.split(sep)
            .map(|f| f.trim().trim_matches('"').trim().to_string())
            .collect()
    };
    let header = split(head);
    let rows: Vec<Vec<String>> = lines.take(MOST_POINTS).map(split).collect();
    Some(Table { header, rows })
}

/// JSON shapes a chart can come from: `[n, ...]`, `[[x, y], ...]`,
/// `[{"k": v, ...}, ...]` (object keys are the columns), or one object of
/// equal-length arrays.
fn json_table(text: &str) -> Option<Table> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let cell = |v: &serde_json::Value| match v {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    match value {
        serde_json::Value::Array(items) if !items.is_empty() => match &items[0] {
            serde_json::Value::Number(_) => Some(Table {
                header: vec!["value".into()],
                rows: items.iter().take(MOST_POINTS).map(|v| vec![cell(v)]).collect(),
            }),
            serde_json::Value::Array(first) => Some(Table {
                header: (0..first.len()).map(|i| if i == 0 { "x".into() } else { format!("y{i}") }).collect(),
                rows: items
                    .iter()
                    .take(MOST_POINTS)
                    .filter_map(|v| v.as_array().map(|a| a.iter().map(cell).collect()))
                    .collect(),
            }),
            serde_json::Value::Object(first) => {
                let header: Vec<String> = first.keys().cloned().collect();
                let rows = items
                    .iter()
                    .take(MOST_POINTS)
                    .filter_map(|v| v.as_object())
                    .map(|o| header.iter().map(|k| o.get(k).map(cell).unwrap_or_default()).collect())
                    .collect();
                Some(Table { header, rows })
            }
            _ => None,
        },
        serde_json::Value::Object(map) => {
            let header: Vec<String> = map.keys().cloned().collect();
            let columns: Vec<&Vec<serde_json::Value>> = map.values().filter_map(|v| v.as_array()).collect();
            if columns.len() != header.len() || columns.is_empty() {
                return None;
            }
            let n = columns.iter().map(|c| c.len()).min()?.min(MOST_POINTS);
            let rows = (0..n).map(|i| columns.iter().map(|c| cell(&c[i])).collect()).collect();
            Some(Table { header, rows })
        }
        _ => None,
    }
}

fn number(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches('%').replace('_', "");
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A domain padded out to round ticks.
fn nice_domain(lo: f64, hi: f64) -> (f64, f64) {
    let (lo, hi) = if hi > lo { (lo, hi) } else { (lo - 1.0, hi + 1.0) };
    let step = nice_step((hi - lo) / 5.0);
    ((lo / step).floor() * step, (hi / step).ceil() * step)
}

fn nice_step(raw: f64) -> f64 {
    let raw = raw.abs().max(1e-12);
    let mag = 10f64.powf(raw.log10().floor());
    let f = raw / mag;
    let nice = if f <= 1.0 { 1.0 } else if f <= 2.0 { 2.0 } else if f <= 2.5 { 2.5 } else if f <= 5.0 { 5.0 } else { 10.0 };
    nice * mag
}

/// A number as a tick reads: short, no trailing zeros.
fn short(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let a = v.abs();
    let s = if a >= 1e9 {
        format!("{:.1}B", v / 1e9)
    } else if a >= 1e6 {
        format!("{:.1}M", v / 1e6)
    } else if a >= 1e4 {
        format!("{:.1}k", v / 1e3)
    } else if a >= 100.0 || v.fract() == 0.0 {
        format!("{v:.0}")
    } else if a >= 1.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.3}")
    };
    if s.contains('.') && !s.ends_with(['k', 'M', 'B']) {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.replace(".0k", "k").replace(".0M", "M").replace(".0B", "B")
    }
}

// ------------------------------------------------------------- drawing ----

/// The hollow of a line's points: the glass's own dark.
const PAPER: [u8; 3] = [12, 14, 18];
const GRID: [u8; 4] = [255, 255, 255, 14];
const AXIS: [u8; 4] = [255, 255, 255, 40];
const INK: [u8; 3] = [150, 156, 166];
/// The data: the warm accent first, then the cool greys of the realm.
const SERIES: [[u8; 3]; 4] = [[196, 134, 92], [143, 163, 168], [128, 140, 196], [176, 170, 150]];
const BAR: [u8; 3] = [66, 80, 132];

struct Canvas {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

impl Canvas {
    /// Clear: the chart is drawn on the showcase's glass, not on paper.
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, rgba: vec![0; w * h * 4] }
    }

    /// `[r, g, b]` at coverage `a` over what is there, straight alpha.
    fn blend(&mut self, x: i64, y: i64, [r, g, b]: [u8; 3], a: f32) {
        if a <= 0.0 || x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return;
        }
        let o = (y as usize * self.w + x as usize) * 4;
        let a = a.min(1.0);
        let below = f32::from(self.rgba[o + 3]) / 255.0;
        let out = a + below * (1.0 - a);
        if out <= 0.0 {
            return;
        }
        for (i, c) in [r, g, b].into_iter().enumerate() {
            let d = f32::from(self.rgba[o + i]);
            self.rgba[o + i] = ((f32::from(c) * a + d * below * (1.0 - a)) / out).round().clamp(0.0, 255.0) as u8;
        }
        self.rgba[o + 3] = (out * 255.0).round() as u8;
    }

    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 3], a: f32) {
        let (xa, xb) = (x0.min(x1), x0.max(x1));
        let (ya, yb) = (y0.min(y1), y0.max(y1));
        for y in ya.floor() as i64..yb.ceil() as i64 {
            let cy = ((y as f32 + 1.0).min(yb) - (y as f32).max(ya)).clamp(0.0, 1.0);
            for x in xa.floor() as i64..xb.ceil() as i64 {
                let cx = ((x as f32 + 1.0).min(xb) - (x as f32).max(xa)).clamp(0.0, 1.0);
                self.blend(x, y, color, a * cx * cy);
            }
        }
    }

    /// A round-capped stroke from `p` to `q`, `width` across.
    fn line(&mut self, p: [f32; 2], q: [f32; 2], width: f32, color: [u8; 3], a: f32) {
        let r = width / 2.0;
        let (x0, x1) = ((p[0].min(q[0]) - r - 1.0).floor() as i64, (p[0].max(q[0]) + r + 1.0).ceil() as i64);
        let (y0, y1) = ((p[1].min(q[1]) - r - 1.0).floor() as i64, (p[1].max(q[1]) + r + 1.0).ceil() as i64);
        let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
        let len = dx * dx + dy * dy;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let t = if len > 0.0 { (((px - p[0]) * dx + (py - p[1]) * dy) / len).clamp(0.0, 1.0) } else { 0.0 };
                let d = ((px - p[0] - dx * t).powi(2) + (py - p[1] - dy * t).powi(2)).sqrt();
                let cover = (r + 0.5 - d).clamp(0.0, 1.0);
                if cover > 0.0 {
                    self.blend(x, y, color, a * cover);
                }
            }
        }
    }

    fn disc(&mut self, c: [f32; 2], r: f32, color: [u8; 3], a: f32) {
        for y in (c[1] - r - 1.0).floor() as i64..=(c[1] + r + 1.0).ceil() as i64 {
            for x in (c[0] - r - 1.0).floor() as i64..=(c[0] + r + 1.0).ceil() as i64 {
                let d = ((x as f32 + 0.5 - c[0]).powi(2) + (y as f32 + 0.5 - c[1]).powi(2)).sqrt();
                self.blend(x, y, color, a * (r + 0.5 - d).clamp(0.0, 1.0));
            }
        }
    }

    /// Straight-RGBA text pixels at `(x, y)`.
    fn text(&mut self, (w, h, rgba): &(u32, u32, Vec<u8>), x: f32, y: f32) {
        let (x, y) = (x.round() as i64, y.round() as i64);
        for ty in 0..*h as usize {
            for tx in 0..*w as usize {
                let o = (ty * *w as usize + tx) * 4;
                let a = f32::from(rgba[o + 3]) / 255.0;
                self.blend(x + tx as i64, y + ty as i64, [rgba[o], rgba[o + 1], rgba[o + 2]], a);
            }
        }
    }
}

/// Text rendered once in the window's font: `(width, height, rgba)`.
fn words(text: &str, ink: [u8; 3], cell_height: u16) -> Option<(u32, u32, Vec<u8>)> {
    let line: String = text.chars().filter(|c| !c.is_control()).take(80).collect();
    if line.trim().is_empty() {
        return None;
    }
    crate::ui::present::with_pager(cell_height, |pager| {
        pager.render(&[line.as_str()], angel_glass::page::PLAIN, ink, [0, 0, 0, 0])
    })
    .flatten()
}

/// A bar or point as the pointer finds it: where it answers and where it
/// is, in the chart's pixels, and what its tip says.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChartMark {
    pub(crate) rect: [f32; 4],
    pub(crate) anchor: [f32; 2],
    pub(crate) text: Vec<String>,
}

/// The plot drawn at `size`, with text one window cell tall: straight
/// RGBA, and the marks its bars and points leave for the pointer.
pub(crate) fn render(plot: &Plot, size: (u32, u32), cell_height: u16) -> Option<(Vec<u8>, Vec<ChartMark>)> {
    let mut marks = Vec::new();
    let (w, h) = (size.0 as usize, size.1 as usize);
    if w < 120 || h < 90 || plot.series.is_empty() || plot.xs.is_empty() {
        return None;
    }
    let mut canvas = Canvas::new(w, h);
    let s = h as f32 / 600.0;
    let ch = f32::from(cell_height);
    // The frame of the plot: room for the title above, ticks to the left
    // and labels below.
    let tick_w = plot_ticks(plot)
        .iter()
        .filter_map(|(_, label)| words(label, INK, cell_height).map(|t| t.0))
        .max()
        .unwrap_or(0) as f32;
    let left = (tick_w + 22.0 * s).max(48.0 * s);
    // The panel's own bar carries the title; the top holds the legend
    // and the y axis's name.
    let top = ch * 2.2;
    let bottom = ch * 2.8;
    let right = 28.0 * s;
    let area = [left, top, w as f32 - left - right, h as f32 - top - bottom];
    if area[2] < 40.0 || area[3] < 40.0 {
        return None;
    }
    let (y0, y1) = plot.y_domain;
    let (x0, x1) = plot.x_domain;
    let ys = |v: f64| area[1] + area[3] * (1.0 - ((v - y0) / (y1 - y0)) as f32);
    let n = plot.xs.len();
    let bars = plot.kind == ChartKind::Bar;
    let xs = |i: usize, v: f64| {
        if bars {
            area[0] + area[2] * (i as f32 + 0.5) / n as f32
        } else {
            area[0] + area[2] * ((v - x0) / (x1 - x0)) as f32
        }
    };
    // The y axis's name over its ticks.
    if let Some(t) = words(&plot.y_label, INK, cell_height) {
        canvas.text(&t, (left - tick_w - 10.0 * s).max(4.0), top - ch * 1.75);
    }
    // The grid and its ticks.
    for (v, label) in plot_ticks(plot) {
        let y = ys(v).round() + 0.5;
        let zero = v == 0.0 && y0 < 0.0;
        let [r, g, b, a] = if zero { AXIS } else { GRID };
        canvas.rect(area[0], y - 0.5, area[0] + area[2], y + 0.5, [r, g, b], f32::from(a) / 255.0);
        if let Some(t) = words(&label, INK, cell_height) {
            canvas.text(&t, left - 10.0 * s - t.0 as f32, y - t.1 as f32 / 2.0);
        }
    }
    let base = ys(if y0 <= 0.0 && y1 >= 0.0 { 0.0 } else { y0 });
    canvas.rect(area[0], base - 0.5, area[0] + area[2], base + 0.5, [AXIS[0], AXIS[1], AXIS[2]], f32::from(AXIS[3]) / 255.0);
    match plot.kind {
        ChartKind::Bar => {
            let slot = area[2] / n as f32;
            let bw = (slot * 0.62).max(2.0);
            let values = &plot.series[0].1;
            for (i, v) in values.iter().enumerate() {
                if !v.is_finite() {
                    continue;
                }
                let cx = xs(i, 0.0);
                let top_y = ys(*v);
                // A cool bar, a little lighter toward its end, capped warm.
                let (ya, yb) = (top_y.min(base), top_y.max(base));
                let steps = ((yb - ya) as usize).max(1);
                for k in 0..steps {
                    let f = k as f32 / steps as f32;
                    let lift = if *v >= 0.0 { 1.0 - f } else { f };
                    let c = [
                        (f32::from(BAR[0]) + 22.0 * lift) as u8,
                        (f32::from(BAR[1]) + 22.0 * lift) as u8,
                        (f32::from(BAR[2]) + 26.0 * lift) as u8,
                    ];
                    canvas.rect(cx - bw / 2.0, ya + k as f32, cx + bw / 2.0, (ya + k as f32 + 1.0).min(yb), c, 1.0);
                }
                marks.push(ChartMark {
                    rect: [cx - slot / 2.0, area[1], slot, area[3]],
                    anchor: [cx, top_y],
                    text: vec![
                        plot.labels.get(i).cloned().unwrap_or_default(),
                        format!("{} {}", short(*v), plot.y_label).trim().to_string(),
                    ],
                });
                let cap = (2.5 * s).max(2.0);
                let cap_y = if *v >= 0.0 { top_y } else { top_y - cap };
                canvas.rect(cx - bw / 2.0, cap_y, cx + bw / 2.0, cap_y + cap, SERIES[0], 1.0);
                if n <= 24
                    && let Some(t) = words(&short(*v), INK, cell_height)
                {
                    let ty = if *v >= 0.0 { top_y - t.1 as f32 - 4.0 * s } else { top_y + 4.0 * s };
                    canvas.text(&t, cx - t.0 as f32 / 2.0, ty);
                }
            }
            // Bar names under the axis, as many as fit.
            let every = label_stride(plot, slot, cell_height);
            for (i, label) in plot.labels.iter().enumerate().step_by(every) {
                if let Some(t) = words(label, INK, cell_height) {
                    canvas.text(&t, xs(i, 0.0) - t.0 as f32 / 2.0, area[1] + area[3] + ch * 0.5);
                }
            }
        }
        ChartKind::Line | ChartKind::Scatter => {
            for (si, (_, values)) in plot.series.iter().enumerate().rev() {
                let color = SERIES[si % SERIES.len()];
                let pts: Vec<[f32; 2]> = plot
                    .xs
                    .iter()
                    .zip(values)
                    .enumerate()
                    .filter(|(_, (x, y))| x.is_finite() && y.is_finite())
                    .map(|(i, (x, y))| [xs(i, *x), ys(*y)])
                    .collect();
                // Each point answers the pointer across its share of the
                // width (sparse points; a dense line keeps every one).
                let reach = (area[2] / pts.len().max(1) as f32).clamp(6.0 * s, 40.0 * s);
                let name = &plot.series[si].0;
                for ((x, y), p) in plot.xs.iter().zip(values).filter(|(x, y)| x.is_finite() && y.is_finite()).zip(&pts) {
                    let x_name = if plot.x_label.is_empty() { String::new() } else { format!("{} ", plot.x_label) };
                    marks.push(ChartMark {
                        rect: [p[0] - reach / 2.0, p[1] - reach, reach, 2.0 * reach],
                        anchor: *p,
                        text: vec![format!("{x_name}{}", short(*x)), format!("{name} {}", short(*y)).trim().to_string()],
                    });
                }
                if plot.kind == ChartKind::Line {
                    // A faint fall of colour under the first series.
                    if si == 0 && pts.len() > 1 {
                        for pair in pts.windows(2) {
                            let [a, b] = [pair[0], pair[1]];
                            // Each column once: a seam drawn twice shows.
                            for x in a[0].round() as i64..b[0].round() as i64 {
                                let t = ((x as f32 + 0.5 - a[0]) / (b[0] - a[0]).max(1e-3)).clamp(0.0, 1.0);
                                let y = a[1] + (b[1] - a[1]) * t;
                                let depth = (base - y).max(0.0);
                                for yy in y.ceil() as i64..base as i64 {
                                    let f = 1.0 - (yy as f32 - y) / depth.max(1.0);
                                    canvas.blend(x, yy, color, 0.16 * f * f);
                                }
                            }
                        }
                    }
                    let width = (2.6 * s).max(1.5);
                    for pair in pts.windows(2) {
                        canvas.line(pair[0], pair[1], width, color, 1.0);
                    }
                    if pts.len() <= 40 {
                        for p in &pts {
                            canvas.disc(*p, 4.2 * s, color, 1.0);
                            canvas.disc(*p, 2.2 * s, PAPER, 1.0);
                        }
                    }
                } else {
                    for p in &pts {
                        canvas.disc(*p, 9.0 * s, color, 0.12);
                        canvas.disc(*p, 4.5 * s, color, 1.0);
                    }
                }
            }
            // X ticks along the bottom.
            let step = nice_step((x1 - x0) / 6.0);
            let mut v = (x0 / step).ceil() * step;
            while v <= x1 + step * 1e-6 {
                if let Some(t) = words(&short(v), INK, cell_height) {
                    canvas.text(&t, xs(0, v) - t.0 as f32 / 2.0, area[1] + area[3] + ch * 0.5);
                }
                v += step;
            }
            // A legend when there is more than one series.
            if plot.series.len() > 1 {
                let mut x = area[0] + area[2];
                for (si, (name, _)) in plot.series.iter().enumerate().rev() {
                    if let Some(t) = words(name, INK, cell_height) {
                        x -= t.0 as f32;
                        canvas.text(&t, x, ch * 0.7);
                        x -= 14.0 * s;
                        canvas.disc([x + 5.0 * s, ch * 0.7 + t.1 as f32 / 2.0], 4.5 * s, SERIES[si % SERIES.len()], 1.0);
                        x -= 22.0 * s;
                    }
                }
            }
        }
    }
    if let Some(t) = words(&plot.x_label, INK, cell_height) {
        canvas.text(&t, area[0] + area[2] - t.0 as f32, h as f32 - ch * 1.25);
    }
    Some((canvas.rgba, marks))
}

/// Every how-many bar names fit side by side.
fn label_stride(plot: &Plot, slot: f32, cell_height: u16) -> usize {
    let widest = plot
        .labels
        .iter()
        .filter_map(|l| words(l, INK, cell_height).map(|t| t.0))
        .max()
        .unwrap_or(0) as f32;
    ((widest + 12.0) / slot.max(1.0)).ceil().max(1.0) as usize
}

fn plot_ticks(plot: &Plot) -> Vec<(f64, String)> {
    let (lo, hi) = plot.y_domain;
    let step = nice_step((hi - lo) / 5.0);
    let mut out = Vec::new();
    let mut v = (lo / step).ceil() * step;
    while v <= hi + step * 1e-6 && out.len() < 12 {
        let v0 = if v.abs() < step * 1e-9 { 0.0 } else { v };
        out.push((v0, short(v0)));
        v += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_against_values_make_bars() {
        let plot = Plot::from_data("crops", "day,bushels\nmon,7\ntue,4\nwed,9\n").unwrap();
        assert_eq!(plot.kind, ChartKind::Bar);
        assert_eq!(plot.labels, vec!["mon", "tue", "wed"]);
        assert_eq!(plot.series[0].1, vec![7.0, 4.0, 9.0]);
        assert!(plot.y_domain.0 <= 0.0 && plot.y_domain.1 >= 9.0);
    }

    #[test]
    fn numbers_against_numbers_make_lines_with_every_series() {
        let plot = Plot::from_data("run", "step,loss,acc\n1,2.0,0.1\n2,1.5,0.3\n3,1.1,0.5\n").unwrap();
        assert_eq!(plot.kind, ChartKind::Line);
        assert_eq!(plot.xs, vec![1.0, 2.0, 3.0]);
        assert_eq!(plot.series.len(), 2);
        assert_eq!(plot.series[1].0, "acc");
    }

    #[test]
    fn json_shapes_chart_too_and_prose_does_not() {
        assert_eq!(Plot::from_data("t", "[3, 1, 4, 1, 5]").unwrap().series[0].1.len(), 5);
        assert_eq!(Plot::from_data("t", "[[0, 1], [1, 3], [2, 2]]").unwrap().xs, vec![0.0, 1.0, 2.0]);
        let rows = r#"[{"city": "a", "pop": 3}, {"city": "b", "pop": 5}]"#;
        assert_eq!(Plot::from_data("t", rows).unwrap().kind, ChartKind::Bar);
        assert!(Plot::from_data("t", "# Notes\n\nSome words, and more words.\n").is_none());
        assert!(Plot::from_data("t", "{\"name\": \"x\"}").is_none());
    }

    #[test]
    fn a_chart_copies_as_its_data() {
        let plot = Plot::from_data("crops", "day,bushels\nmon,7\ntue,4\n").unwrap();
        assert_eq!(plot.csv(), "day,bushels\nmon,7\ntue,4\n");
        let lines = Plot::from_data("run", "step,loss\n1,2.5\n2,1.5\n").unwrap();
        assert_eq!(lines.csv(), "step,loss\n1,2.5\n2,1.5\n");
    }

    #[test]
    fn ticks_read_short_and_domains_round_out() {
        assert_eq!(short(1200.0), "1200");
        assert_eq!(short(25_000.0), "25k");
        assert_eq!(short(2.5), "2.5");
        assert_eq!(short(0.125), "0.125");
        assert_eq!(nice_domain(0.3, 9.2), (0.0, 10.0));
    }

    #[test]
    fn a_plot_draws_to_its_size() {
        let plot = Plot::from_data("t", "x,y\n1,2\n2,3\n3,1\n").unwrap();
        // No font in a bare test is fine: the shapes still draw.
        let (rgba, marks) = render(&plot, (640, 360), 16).unwrap();
        assert_eq!(rgba.len(), 640 * 360 * 4);
        assert_eq!(marks.len(), 3, "one mark per point");
        assert!(marks[0].anchor[0] < marks[2].anchor[0]);
        assert_eq!(marks[1].text[1], "y 3");
        let warm = rgba.chunks(4).filter(|p| p[0] > 150 && p[1] > 100 && p[2] < 110).count();
        assert!(warm > 50, "the line is drawn in the warm accent");
        assert_eq!(rgba[3], 0, "no paper: the corner is clear");
    }
}

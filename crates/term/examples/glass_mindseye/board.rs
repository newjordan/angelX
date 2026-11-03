//! The numbers, in the film: glass against kitty from the bench, and this
//! run's own, graphed as it goes, as faint light among the other layers
//! rather than panels over them.
//!
//! The bench is read from a scoreboard file (`MINDSEYE_BENCH`: the one the
//! bench harness wrote; unset, the film plays without it): medians of its rounds (`rounds`,
//! three if it does not say) of each workload, timed until the terminal
//! answers DA1. Each measure is a spoke of one web that drifts behind the
//! geometry (see [`Board::web`]). This run is three traces across the foot
//! of the frame: frames a second, MB/s of escape codes and how long glass
//! takes to answer a frame, over the last twelve seconds. Both sit faint
//! and now and then surface. Their words are pictures, drawn once.

use std::collections::{HashMap, VecDeque};

use super::surreal::{render_text, upload};
use std::f32::consts::{FRAC_PI_2, TAU};

use super::{Canvas, Rgb, W, clamp01, hex, mix, smooth};

/// Program picture ids for the graphs' words.
const LABEL_BASE: u32 = 1600;
/// The live graphs' span, seconds.
const SPAN: f32 = 12.0;
/// The web, behind the geometry (-1) and the front scene, in front of
/// the back scene (-3).
const Z_WEB: i32 = -2;
/// The traces, with the geometry.
const Z_TRACES: i32 = -1;
/// What a spoke's length means.
const SPOKES: &str = "on each spoke the better of the two reaches the rim";

/// One measure, both terminals, and whether less is better.
struct Pair {
    name: String,
    glass: f32,
    kitty: f32,
    lower_better: bool,
    unit: &'static str,
}

pub struct Board {
    title: String,
    note: String,
    /// How many rounds the medians are of, in words.
    heading: String,
    throughput: Vec<Pair>,
    others: Vec<Pair>,
    /// Text drawn as a picture: (id, width, height).
    labels: HashMap<String, (u32, u32, u32)>,
}

/// This run's frames: (show time, frames a second, MB/s, answer ms).
pub type Samples = VecDeque<(f32, f32, f32, f32)>;

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - clamp01(t)).powi(3)
}

/// Faint most of the time; now and then the numbers surface for a few
/// seconds, then sink back.
fn surfacing(t: f32, salt: f32) -> f32 {
    let a = (0.5 + 0.5 * (t * 0.41 + salt).sin()).powi(10);
    let b = (0.5 + 0.5 * (t * 0.23 + salt * 2.3).sin()).powi(14);
    0.3 + 0.6 * a.max(b)
}

fn number(v: f32) -> String {
    if v >= 100.0 { format!("{v:.0}") } else if v >= 10.0 { format!("{v:.1}") } else { format!("{v:.2}") }
}

impl Board {
    pub fn load(path: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        let f = |v: &serde_json::Value| v.as_f64().map(|x| x as f32);
        let throughput = v["throughput"]
            .as_array()?
            .iter()
            .filter_map(|w| {
                Some(Pair {
                    name: w["name"].as_str()?.to_string(),
                    glass: f(&w["glass"])?,
                    kitty: f(&w["kitty"])?,
                    lower_better: false,
                    unit: "MB/s",
                })
            })
            .collect();
        let pair = |key: &str, name: &str, lower_better: bool, unit: &'static str| {
            Some(Pair { name: name.into(), glass: f(&v[key]["glass"])?, kitty: f(&v[key]["kitty"])?, lower_better, unit })
        };
        let others = [
            pair("tui_fps", "full-screen TUI", false, "frames/s"),
            pair("cpu_s", "CPU, all six workloads", true, "s"),
            pair("vram_mib", "VRAM, idle window", true, "MiB"),
            pair("first_frame_ms", "window on screen", true, "ms"),
            pair("startup_ms", "startup, to a running shell", true, "ms"),
        ]
        .into_iter()
        .flatten()
        .collect();
        Some(Self {
            title: v["title"].as_str().unwrap_or("glass vs kitty").to_string(),
            note: v["note"].as_str().unwrap_or("").to_string(),
            heading: format!("medians of {} rounds", v["rounds"].as_u64().unwrap_or(3)),
            throughput,
            others,
            labels: HashMap::new(),
        })
    }

    /// Every word the graphs show, as pictures.
    pub fn setup(&mut self, out: &mut Vec<u8>) {
        let mut words: Vec<(String, u32, &'static str)> = vec![
            (self.title.clone(), 30, "#ffffff"),
            (self.heading.clone(), 20, "#8fa3c8"),
            (SPOKES.into(), 18, "#8fa3c8"),
            ("glass".into(), 20, "#00e5ff"),
            ("kitty".into(), 20, "#e0b070"),
            ("frames/s".into(), 17, "#00e5ff"),
            ("MB/s of escape codes".into(), 17, "#ff3fa4"),
            ("glass answers, ms".into(), 17, "#ffc861"),
            ("60".into(), 13, "#8fa3c8"),
        ];
        for chunk in self.note.split("; ") {
            words.push((chunk.to_string(), 14, "#7f8faf"));
        }
        for p in self.throughput.iter().chain(&self.others) {
            words.push((p.name.clone(), 22, "#dfe6f5"));
            words.push((format!("{} {}", number(p.glass), p.unit), 19, "#00e5ff"));
            words.push((format!("{} {}", number(p.kitty), p.unit), 19, "#e0b070"));
        }
        let mut id = LABEL_BASE;
        for (text, points, color) in words {
            if self.labels.contains_key(&text) {
                continue;
            }
            if let Some((w, h, rgba)) = render_text(&text, "Berkeley-Mono-Regular", points, color) {
                upload(out, id, w, h, &rgba);
                self.labels.insert(text, (id, w, h));
                id += 1;
            }
        }
    }

    /// Text as a picture, its middle at `y`; `align` 0 puts its left
    /// edge at `x`, 1 its right edge, between the two in between.
    #[allow(clippy::too_many_arguments)]
    fn word(&self, cv: &mut Canvas, text: &str, x: f32, y: f32, align: f32, o: f32, z: i32) {
        if let Some(&(id, w, h)) = self.labels.get(text) {
            cv.picture(id, x + w as f32 * (0.5 - align), y, w as f32, h as f32, 0.0, o, z);
        }
    }

    pub fn draw(&self, cv: &mut Canvas, t: f32, live: &Samples, end: f32) {
        let k = smooth(3.5, 5.0, t) * (1.0 - smooth(end - 1.2, end - 0.2, t));
        if k <= 0.0 {
            return;
        }
        self.traces(cv, t, live, k * surfacing(t, 1.7) * 0.85);
        self.web(cv, t, k * surfacing(t, 0.0));
    }

    /// Every measure as a spoke of one web. On each spoke the better of
    /// the two terminals reaches the rim and the other falls short by
    /// their ratio, so where glass wins its line runs round the rim and
    /// kitty's is the star inside it.
    fn web(&self, cv: &mut Canvas, t: f32, k: f32) {
        let pairs: Vec<&Pair> = self.throughput.iter().chain(&self.others).collect();
        let n = pairs.len();
        if n < 3 || k < 0.004 {
            return;
        }
        let (cx, cy) = (W / 2.0 + 200.0 * (t * 0.031).sin(), 520.0 + 50.0 * (t * 0.043).cos());
        let grow = ease_out_cubic((t - 4.0) / 2.5);
        let r = 300.0 * grow * (1.0 + 0.015 * (t * 0.9).sin());
        let angle = |i: usize| -FRAC_PI_2 + TAU * i as f32 / n as f32;
        let at = |i: usize, f: f32| (cx + angle(i).cos() * r * f, cy + angle(i).sin() * r * f);
        let reach = |p: &Pair, v: f32| {
            let f = if p.lower_better { p.glass.min(p.kitty) / v.max(1e-6) } else { v / p.glass.max(p.kitty).max(1e-6) };
            clamp01(f)
        };
        let faint = hex("#8fa3c8");
        // Rings at the quarters, and the spokes.
        for q in [0.25, 0.5, 0.75, 1.0] {
            for i in 0..n {
                let (a, b) = (at(i, q), at((i + 1) % n, q));
                cv.line_at(Z_WEB, a.0, a.1, b.0, b.1, faint, k * if q == 1.0 { 0.2 } else { 0.1 }, 1.0, 2.0);
            }
        }
        for i in 0..n {
            let e = at(i, 1.06);
            cv.line_at(Z_WEB, cx, cy, e.0, e.1, faint, k * 0.14, 1.0, 2.0);
        }
        // Kitty's line, then glass's over it.
        let kitty: Vec<(f32, f32)> = pairs.iter().enumerate().map(|(i, p)| at(i, reach(p, p.kitty))).collect();
        let glass: Vec<(f32, f32)> = pairs.iter().enumerate().map(|(i, p)| at(i, reach(p, p.glass))).collect();
        let amber = hex("#e0b070");
        let hue = |i: usize| mix(hex("#00e5ff"), hex("#ff3fa4"), i as f32 / n as f32);
        for i in 0..n {
            let (a, b) = (kitty[i], kitty[(i + 1) % n]);
            cv.line_at(Z_WEB, a.0, a.1, b.0, b.1, amber, k * 0.7, 2.5, 7.0);
            cv.glow_at(Z_WEB, a.0, a.1, 7.0, amber, k * 0.7);
        }
        for i in 0..n {
            let (a, b) = (glass[i], glass[(i + 1) % n]);
            cv.line_at(Z_WEB, a.0, a.1, b.0, b.1, hue(i), k * 0.95, 3.5, 10.0);
            cv.glow_at(Z_WEB, a.0, a.1, 10.0, hue(i), k * 0.9);
        }
        // A glint runs round glass's line.
        let u = (t * 0.11).fract() * n as f32;
        let (i, f) = (u.floor() as usize % n, u.fract());
        let (a, b) = (glass[i], glass[(i + 1) % n]);
        cv.glow_at(Z_WEB, a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f, 20.0, hue(i), k * 0.8);
        // Each spoke's words at its end, leaning away from the middle.
        for (i, p) in pairs.iter().enumerate() {
            let (c, s) = (angle(i).cos(), angle(i).sin());
            let (x, y) = (cx + c * (r + 30.0), cy + s * (r + 30.0) + s * 26.0);
            let align = if c.abs() < 0.2 { 0.5 } else if c > 0.0 { 0.0 } else { 1.0 };
            self.word(cv, &p.name, x, y - 21.0, align, k * 0.7, Z_WEB);
            self.word(cv, &format!("{} {}", number(p.glass), p.unit), x, y, align, k * 0.85, Z_WEB);
            self.word(cv, &format!("{} {}", number(p.kitty), p.unit), x, y + 21.0, align, k * 0.7, Z_WEB);
        }
        // What it is and how it was measured, in a corner, faintly.
        let words = [self.title.as_str(), self.heading.as_str(), SPOKES];
        for (j, w) in words.iter().enumerate() {
            self.word(cv, w, 40.0, 70.0 + j as f32 * 30.0, 0.0, k * if j == 0 { 0.6 } else { 0.5 }, Z_WEB);
        }
        self.word(cv, "glass", 40.0, 160.0, 0.0, k * 0.7, Z_WEB);
        self.word(cv, "kitty", 120.0, 160.0, 0.0, k * 0.7, Z_WEB);
        for (j, chunk) in self.note.split("; ").enumerate() {
            self.word(cv, chunk, 40.0, 186.0 + j as f32 * 16.0, 0.0, k * 0.42, Z_WEB);
        }
    }

    /// This run, as three lines of light across the foot of the frame:
    /// frames a second, MB/s of escape codes, how long glass takes to
    /// answer. No axes; the frames line has 60 drawn faintly behind it.
    fn traces(&self, cv: &mut Canvas, t: f32, live: &Samples, k: f32) {
        if k < 0.004 {
            return;
        }
        let (gx, gw, gh) = (60.0, W - 120.0, 110.0);
        let graphs: [(&str, Rgb, usize, f32, f32); 3] = [
            ("frames/s", hex("#00e5ff"), 1, 70.0, 960.0),
            ("MB/s of escape codes", hex("#ff3fa4"), 2, live.iter().map(|s| s.2).fold(10.0, f32::max) * 1.15, 990.0),
            ("glass answers, ms", hex("#ffc861"), 3, live.iter().map(|s| s.3).fold(20.0, f32::max) * 1.15, 1020.0),
        ];
        for (label, color, field, top, base) in graphs {
            self.word(cv, label, gx, base + 14.0, 0.0, k * 0.55, Z_TRACES);
            if field == 1 {
                let y60 = base - gh * 60.0 / top;
                cv.line_at(Z_TRACES, gx, y60, gx + gw, y60, hex("#3a4a6a"), k * 0.35, 1.0, 0.0);
                self.word(cv, "60", gx + gw, y60 - 10.0, 1.0, k * 0.4, Z_TRACES);
            }
            let value = |s: &(f32, f32, f32, f32)| match field {
                1 => s.1,
                2 => s.2,
                _ => s.3,
            };
            let point = |s: &(f32, f32, f32, f32)| (gx + gw * (1.0 - (t - s.0) / SPAN), base - gh * clamp01(value(s) / top));
            let mut last: Option<(f32, f32)> = None;
            for s in live.iter().filter(|s| t - s.0 <= SPAN) {
                let p = point(s);
                match last {
                    Some(q) if (p.0 - q.0).abs() >= 3.0 => {
                        cv.line_at(Z_TRACES, q.0, q.1, p.0, p.1, color, k, 2.2, 6.0);
                        last = Some(p);
                    }
                    None => last = Some(p),
                    _ => {}
                }
            }
            if let Some(q) = last {
                cv.glow_at(Z_TRACES, q.0, q.1, 12.0, color, k);
            }
        }
    }
}

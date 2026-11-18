//! The Heartglass: the Virtual Embryo campaign as a holo-panel over the
//! terminal (docs/HEARTGLASS.md).
//!
//! `ANGEL_GLASS_HEART=1` in the window, or `/heart`, floats its modules
//! over the window by default: the mouse heart as a turning cloud of cells (the
//! cartridge's MERFISH stages, then our predicted stage, rebuilt by a scan
//! between them), beating; under it the five boards as tracks against the
//! floor, the VE leader and 10th place, the partner team and the Yukon
//! frontier, with our best official and proxy scores, the day's budget and
//! a sparkline each; a ticker line and the countdown to the end of the
//! collaboration phase. A newly landed official score flares the heart.
//!
//! One thread reads the cartridge's folder at most once a second
//! (`campaign::Watcher`) and hands the window a scene and a layer set at
//! 30 fps; the cockpit's loop never draws it. Its geometry, the reader,
//! the meshes and the plate's drawing are pure functions, tested without a
//! window. In a terminal the module is inert and `/heart` says where the
//! panel lives.

#![cfg_attr(not(feature = "glass"), allow(dead_code))]

pub(crate) mod campaign;
pub(crate) mod demo;
pub(crate) mod heart;
pub(crate) mod hud;
#[cfg(feature = "glass")]
pub(crate) mod layers;
#[cfg(feature = "glass")]
pub(crate) mod plate;

use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) use crate::ui::orrery::{Grid, Panel};

pub(crate) const CELL: (f32, f32) = (12.0, 25.0);
pub(crate) const TOP_ROWS: f32 = 4.4;
pub(crate) const BOTTOM_ROWS: f32 = 4.8;
pub(crate) const EDGE_COLS: f32 = 2.0;
pub(crate) const GAP_COLS: f32 = 4.0;
pub(crate) const MIN_COLS: f32 = 80.0;
pub(crate) const MIN_ROWS: f32 = 24.0;

pub(crate) const TILT: f32 = -0.025;

pub(crate) const PAD: f32 = 16.0;
pub(crate) const TITLE_H: f32 = 46.0;
pub(crate) const STAGES_H: f32 = 26.0;
pub(crate) const AXIS_H: f32 = 18.0;
pub(crate) const ROW_H: f32 = 42.0;
pub(crate) const LEGEND_H: f32 = 20.0;
pub(crate) const TICKER_H: f32 = 30.0;
pub(crate) const MIN_HEART: f32 = 150.0;
pub(crate) const MAX_HEART: f32 = 380.0;
pub(crate) const DEFAULT_BOARDS: usize = 5;

pub(crate) fn leaned(rect: [f32; 4], centre: [f32; 2]) -> ([f32; 4], f32) {
    crate::ui::orrery::tilted_rect_by(rect, centre, (1.0, 1.0), TILT)
}

pub(crate) fn leaned_point(p: [f32; 2], centre: [f32; 2]) -> [f32; 2] {
    crate::ui::orrery::leaned_point_by(p, centre, TILT)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub(crate) panel: Panel,
    pub(crate) title: Panel,
    pub(crate) heart: Panel,
    pub(crate) stages: Panel,
    pub(crate) axis: Panel,
    pub(crate) rows: Vec<Panel>,
    pub(crate) legend: Panel,
    pub(crate) ticker: Panel,
}

pub(crate) fn fixed_height(boards: usize) -> f32 {
    6.0 + TITLE_H
        + 6.0
        + 2.0
        + STAGES_H
        + 2.0
        + AXIS_H
        + boards as f32 * ROW_H
        + 2.0
        + LEGEND_H
        + 2.0
        + TICKER_H
        + 7.0
}

impl Layout {
    pub(crate) fn for_grid(grid: Grid, orrery: Option<Panel>, boards: usize) -> Option<Self> {
        let (cols, rows) = (grid.width as f32 / CELL.0, grid.height as f32 / CELL.1);
        if cols < MIN_COLS || rows < MIN_ROWS {
            return None;
        }
        let boards = boards.clamp(1, campaign::MAX_BOARDS);
        let w = (cols * 0.40).clamp(36.0, 48.0) * CELL.0;
        let fixed = fixed_height(boards);
        let bottom = grid.height as f32 - BOTTOM_ROWS * CELL.1;
        let fit = |right: f32, y: f32| -> Option<(f32, f32, f32)> {
            let heart = (bottom - y - fixed).min(MAX_HEART);
            let lean = (fixed + heart) * 0.5 * TILT.abs().sin();
            let x = right - w - lean;
            (x >= EDGE_COLS * CELL.0 && heart >= MIN_HEART).then_some((x, y, heart))
        };
        let edge = grid.width as f32 - EDGE_COLS * CELL.0;
        let top = TOP_ROWS * CELL.1;
        let (x, y, heart) = match orrery {
            None => fit(edge, top)?,
            Some(o) => {
                fit(o.x - GAP_COLS * CELL.0, top).or_else(|| fit(edge, o.y + o.h + 1.4 * CELL.1))?
            }
        };
        let inner = |y: f32, h: f32| Panel {
            x: x + PAD,
            y,
            w: w - 2.0 * PAD,
            h,
        };
        let title = inner(y + 6.0, TITLE_H);
        let heart = inner(title.y + title.h + 6.0, heart);
        let stages = inner(heart.y + heart.h + 2.0, STAGES_H);
        let axis = inner(stages.y + stages.h + 2.0, AXIS_H);
        let rows: Vec<Panel> = (0..boards)
            .map(|i| inner(axis.y + axis.h + i as f32 * ROW_H, ROW_H))
            .collect();
        let legend = inner(axis.y + axis.h + boards as f32 * ROW_H + 2.0, LEGEND_H);
        let ticker = inner(legend.y + legend.h + 2.0, TICKER_H);
        let h = ticker.y + ticker.h + 7.0 - y;
        Some(Self {
            panel: Panel { x, y, w, h },
            title,
            heart,
            stages,
            axis,
            rows,
            legend,
            ticker,
        })
    }
}

static VISIBLE: AtomicBool = AtomicBool::new(false);

pub(crate) const DEFAULT_FLOATING: bool = true;
static FLOAT: AtomicBool = AtomicBool::new(DEFAULT_FLOATING);

static RESET: AtomicBool = AtomicBool::new(false);

static DEMO: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Toggled {
    Shown,
    Hidden,
    NoWindow,
}

pub(crate) fn visible() -> bool {
    VISIBLE.load(Ordering::Relaxed)
}

pub(crate) fn floating() -> bool {
    FLOAT.load(Ordering::Relaxed)
}

pub(crate) fn demo() -> bool {
    DEMO.load(Ordering::Relaxed)
}

pub(crate) fn take_reset() -> bool {
    RESET.swap(false, Ordering::Relaxed)
}

pub(crate) fn reset_layout() -> Toggled {
    if !crate::ui::glass::active() {
        return Toggled::NoWindow;
    }
    if let Err(error) = crate::platform::workspace_store::remove_durable(&hud::saved_path()) {
        eprintln!("heartglass: {error}");
    }
    RESET.store(true, Ordering::Relaxed);
    show_as(true)
}

fn launch_look(wanted: &str) -> Option<bool> {
    match wanted {
        "1" | "float" => Some(true),
        "plate" => Some(false),
        _ => None,
    }
}

pub(crate) fn launch_if_requested() {
    let wanted = std::env::var("ANGEL_GLASS_HEART").unwrap_or_default();
    if wanted == "demo" && crate::ui::glass::active() {
        DEMO.store(true, Ordering::Relaxed);
        FLOAT.store(DEFAULT_FLOATING, Ordering::Relaxed);
        VISIBLE.store(true, Ordering::Relaxed);
        launch();
        return;
    }
    if let Some(float) = launch_look(&wanted)
        && crate::ui::glass::active()
    {
        FLOAT.store(float, Ordering::Relaxed);
        VISIBLE.store(true, Ordering::Relaxed);
        launch();
    }
}

pub(crate) fn show_demo() -> Toggled {
    if !crate::ui::glass::active() {
        return Toggled::NoWindow;
    }
    DEMO.store(true, Ordering::Relaxed);
    FLOAT.store(DEFAULT_FLOATING, Ordering::Relaxed);
    VISIBLE.store(true, Ordering::Relaxed);
    launch();
    Toggled::Shown
}

pub(crate) fn show_as(float: bool) -> Toggled {
    if !crate::ui::glass::active() {
        return Toggled::NoWindow;
    }
    DEMO.store(false, Ordering::Relaxed);
    FLOAT.store(float, Ordering::Relaxed);
    VISIBLE.store(true, Ordering::Relaxed);
    launch();
    Toggled::Shown
}

pub(crate) fn toggle() -> Toggled {
    if !crate::ui::glass::active() {
        return Toggled::NoWindow;
    }
    let shown = !VISIBLE.fetch_xor(true, Ordering::Relaxed);
    if shown {
        launch();
        Toggled::Shown
    } else {
        Toggled::Hidden
    }
}

fn launch() {
    #[cfg(feature = "glass")]
    {
        static LAUNCHED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        if let Some(host) = crate::ui::glass::host()
            && LAUNCHED.set(()).is_ok()
        {
            let host = host.clone();
            std::thread::Builder::new()
                .name("angel-heartglass".into())
                .spawn(move || layers::run(host))
                .ok();
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/heartglass__tests.rs"]
mod tests;

#[cfg(all(test, feature = "glass"))]
#[path = "../../../tests/cockpit/app/heartglass_plate__tests.rs"]
mod plate_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/heartglass_hud__tests.rs"]
mod hud_tests;

#[cfg(all(test, feature = "glass"))]
#[path = "../../../tests/cockpit/app/heartglass_gpu__tests.rs"]
mod gpu_tests;

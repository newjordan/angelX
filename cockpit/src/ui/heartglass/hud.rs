//! The floating Heartglass: the panel's parts laid over the whole window
//! as a heads-up display (`ANGEL_GLASS_HEART=float`, `/heart float`).
//!
//! Each part is a module (the heartbeat strip, the heart, the stage rail,
//! the boards, their legend and the ticker) that the pointer can drag,
//! resize from its corner and hide. Where the operator put them is kept in
//! `~/.angelX/heartglass-hud.json`; a module the file does not name, or
//! names with a value out of bounds, stands where it was designed, and
//! `/heart reset` brings every module back there.
//!
//! Everything here is pure: the layout for a window, the saved places,
//! the hit-test and what a drag does. The window's thread
//! (`heartglass::layers`) draws it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{AXIS_H, Grid, LEGEND_H, Layout, Panel, ROW_H, STAGES_H, TICKER_H, TITLE_H};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Module {
    Title,
    Heart,
    Stages,
    Boards,
    Legend,
    Ticker,
}

impl Module {
    pub(crate) const ALL: [Module; 6] = [
        Module::Title,
        Module::Heart,
        Module::Stages,
        Module::Boards,
        Module::Legend,
        Module::Ticker,
    ];

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Module::Title => "title",
            Module::Heart => "heart",
            Module::Stages => "stages",
            Module::Boards => "boards",
            Module::Legend => "legend",
            Module::Ticker => "ticker",
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Module::Title => "HEARTBEAT",
            Module::Heart => "HEART",
            Module::Stages => "STAGES",
            Module::Boards => "BOARDS",
            Module::Legend => "LEGEND",
            Module::Ticker => "TICKER",
        }
    }
}

pub(crate) const MARGIN: f32 = 24.0;
pub(crate) const OUTLINE: f32 = 6.0;
pub(crate) const TAB_H: f32 = 17.0;
pub(crate) const GRIP: f32 = 18.0;
pub(crate) const MIN_SCALE: f32 = 0.6;
pub(crate) const MAX_SCALE: f32 = 2.2;
pub(crate) const MIN_W: f32 = 640.0;
pub(crate) const MIN_H: f32 = 420.0;
pub(crate) const WIDE: f32 = 1180.0;
pub(crate) const SNAP: f32 = 10.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Anchored {
    pub(crate) anchor: f32,
    pub(crate) offset: f32,
}

impl Anchored {
    pub(crate) fn of(start: f32, size: f32, extent: f32) -> Self {
        let middle = start + size / 2.0;
        let anchor = if middle < extent / 3.0 {
            0.0
        } else if middle > extent * 2.0 / 3.0 {
            1.0
        } else {
            0.5
        };
        Self::at(start, size, extent, anchor)
    }

    pub(crate) fn at(start: f32, size: f32, extent: f32, anchor: f32) -> Self {
        Self {
            anchor,
            offset: start - anchor * (extent - size),
        }
    }

    pub(crate) fn start(self, size: f32, extent: f32) -> f32 {
        self.anchor * (extent - size) + self.offset
    }

    fn sane(self) -> Option<Self> {
        (self.anchor.is_finite()
            && self.offset.is_finite()
            && (0.0..=1.0).contains(&self.anchor)
            && self.offset.abs() <= 16_384.0)
            .then_some(self)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Place {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) x: Option<Anchored>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) y: Option<Anchored>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scale: Option<f32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) hidden: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Saved {
    pub(crate) version: u32,
    pub(crate) modules: BTreeMap<String, Place>,
}

pub(crate) const VERSION: u32 = 1;

const MAX_FILE: u64 = 64 * 1024;

impl Saved {
    pub(crate) fn place(&self, module: Module) -> Place {
        let p = self.modules.get(module.key()).copied().unwrap_or_default();
        Place {
            x: p.x.and_then(Anchored::sane),
            y: p.y.and_then(Anchored::sane),
            scale: p
                .scale
                .filter(|s| s.is_finite())
                .map(|s| s.clamp(MIN_SCALE, MAX_SCALE)),
            hidden: p.hidden,
        }
    }

    fn set(&mut self, module: Module, place: Place) {
        self.modules.insert(module.key().to_string(), place);
    }

    pub(crate) fn parse(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap_or_default()
    }

    pub(crate) fn to_json(&self) -> Vec<u8> {
        let mut saved = self.clone();
        saved.version = VERSION;
        serde_json::to_vec_pretty(&saved).unwrap_or_default()
    }

    pub(crate) fn load(path: &std::path::Path) -> Self {
        match std::fs::metadata(path) {
            Ok(meta) if meta.len() <= MAX_FILE => {
                std::fs::read(path).map_or_else(|_| Self::default(), |b| Self::parse(&b))
            }
            _ => Self::default(),
        }
    }

    pub(crate) fn save(&self, path: &std::path::Path) -> Result<(), String> {
        crate::platform::workspace_store::write_private_atomic(path, &self.to_json())
    }
}

pub(crate) fn saved_path() -> PathBuf {
    crate::platform::workspace_store::angel_subdir("heartglass-hud.json")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Designed {
    pub(crate) size: (f32, f32),
    pub(crate) x: Anchored,
    pub(crate) y: Anchored,
}

fn cell_or_default(cell: (f32, f32)) -> (f32, f32) {
    (
        if cell.0 >= 4.0 { cell.0 } else { super::CELL.0 },
        if cell.1 >= 8.0 { cell.1 } else { super::CELL.1 },
    )
}

pub(crate) fn design(grid: Grid, cell: (f32, f32), boards: usize) -> Option<[Designed; 6]> {
    let (w, h) = (grid.width as f32, grid.height as f32);
    if w < MIN_W || h < MIN_H {
        return None;
    }
    let cell = cell_or_default(cell);
    let boards = boards.clamp(1, super::campaign::MAX_BOARDS);
    let m = MARGIN;
    let edge = 2.0 * cell.0;
    let top = super::TOP_ROWS * cell.1;
    let bottom = h - super::BOTTOM_ROWS * cell.1;
    let wide = w >= WIDE;

    let band = (w - 9.0 * cell.0, (w * 0.40).max(600.0));
    let in_header = wide && band.0 - band.1 >= 560.0;
    let (title_x, title_y, title_w) = if in_header {
        let tw = (band.0 - band.1).min(900.0);
        (band.0 - tw, 0.65 * cell.1, tw)
    } else {
        let tw = (w - 2.0 * edge).min(980.0);
        ((w - tw) / 2.0, top, tw)
    };
    let below_title = if in_header { top } else { top + TITLE_H + 16.0 };

    let board_w = if wide {
        (w * 0.34).clamp(440.0, 580.0)
    } else {
        (w - 2.0 * edge).min(540.0)
    };
    let board_h = AXIS_H + boards as f32 * ROW_H;
    let ticker_w = if wide {
        (w * 0.42).clamp(420.0, 720.0)
    } else {
        w - 2.0 * edge
    };
    let heart_w = if wide {
        (w * 0.26).clamp(320.0, 480.0)
    } else {
        (w * 0.36).clamp(240.0, 360.0)
    };

    let ticker_y = h - 4.52 * cell.1 - 14.5;
    let floor = if wide {
        bottom
    } else {
        ticker_y.min(bottom) - 4.0
    };
    let legend_y = floor - LEGEND_H;
    let boards_y = legend_y - 2.0 - board_h;
    let heart_y = below_title;
    let right = w - edge;
    let room = boards_y - 18.0 - STAGES_H - 4.0 - heart_y;
    let (heart_x, heart_y, heart_w, heart_h) = if room >= 150.0 {
        let hh = (heart_w * 0.86).min(room);
        (right - heart_w, heart_y + (room - hh) / 3.0, heart_w, hh)
    } else {
        let heart_right = right - board_w - 24.0;
        let hw = heart_w.min(heart_right - edge).max(120.0);
        let hh = (hw * 0.86)
            .min(floor - 8.0 - STAGES_H - 4.0 - heart_y)
            .max(100.0);
        (heart_right - hw, heart_y, hw, hh)
    };
    let stages_y = heart_y + heart_h + 4.0;

    let module = |x: f32, y: f32, cw: f32, ch: f32, ax: f32, ay: f32| {
        let size = (cw + 2.0 * m, ch + 2.0 * m);
        Designed {
            size,
            x: Anchored::at(x - m, size.0, w, ax),
            y: Anchored::at(y - m, size.1, h, ay),
        }
    };
    Some([
        module(
            title_x,
            title_y,
            title_w,
            TITLE_H,
            if in_header { 1.0 } else { 0.5 },
            0.0,
        ),
        module(heart_x, heart_y, heart_w, heart_h, 1.0, 0.0),
        module(heart_x, stages_y, heart_w, STAGES_H, 1.0, 0.0),
        module(right - board_w, boards_y, board_w, board_h, 1.0, 1.0),
        module(right - board_w, legend_y, board_w, LEGEND_H, 1.0, 1.0),
        module(edge, ticker_y, ticker_w, TICKER_H, 0.0, 1.0),
    ])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placed {
    pub(crate) module: Module,
    pub(crate) rect: Panel,
    pub(crate) natural: (f32, f32),
    pub(crate) scale: f32,
}

impl Placed {
    pub(crate) fn local_content(&self) -> Panel {
        Panel {
            x: MARGIN,
            y: MARGIN,
            w: self.natural.0 - 2.0 * MARGIN,
            h: self.natural.1 - 2.0 * MARGIN,
        }
    }

    pub(crate) fn at(&self, p: [f32; 2]) -> [f32; 2] {
        [
            self.rect.x + p[0] * self.scale,
            self.rect.y + p[1] * self.scale,
        ]
    }

    pub(crate) fn rect_at(&self, r: [f32; 4]) -> [f32; 4] {
        let [x, y] = self.at([r[0], r[1]]);
        [x, y, r[2] * self.scale, r[3] * self.scale]
    }

    pub(crate) fn content(&self) -> Panel {
        let c = self.local_content();
        let [x, y, w, h] = self.rect_at([c.x, c.y, c.w, c.h]);
        Panel { x, y, w, h }
    }

    pub(crate) fn outline(&self) -> Panel {
        let c = self.content();
        Panel {
            x: c.x - OUTLINE,
            y: c.y - OUTLINE,
            w: c.w + 2.0 * OUTLINE,
            h: c.h + 2.0 * OUTLINE,
        }
    }

    pub(crate) fn tab(&self) -> Panel {
        let o = self.outline();
        let y = if o.y >= TAB_H + 2.0 {
            o.y - TAB_H - 1.0
        } else {
            o.y + o.h + 1.0
        };
        Panel {
            x: o.x,
            y,
            w: o.w,
            h: TAB_H,
        }
    }

    pub(crate) fn close(&self) -> Panel {
        let t = self.tab();
        Panel {
            x: t.x + t.w - TAB_H,
            y: t.y,
            w: TAB_H,
            h: TAB_H,
        }
    }

    pub(crate) fn grip(&self) -> Panel {
        let o = self.outline();
        Panel {
            x: o.x + o.w - GRIP * 0.6,
            y: o.y + o.h - GRIP * 0.6,
            w: GRIP,
            h: GRIP,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hud {
    pub(crate) grid: Grid,
    pub(crate) cell: (f32, f32),
    pub(crate) modules: Vec<Placed>,
}

pub(crate) fn arrange(grid: Grid, cell: (f32, f32), boards: usize, saved: &Saved) -> Option<Hud> {
    let cell = cell_or_default(cell);
    let designed = design(grid, cell, boards)?;
    let (w, h) = (grid.width as f32, grid.height as f32);
    let mut modules = Vec::with_capacity(Module::ALL.len());
    for module in Module::ALL {
        let d = designed[module.index()];
        let place = saved.place(module);
        if place.hidden {
            continue;
        }
        let scale = place
            .scale
            .unwrap_or(1.0)
            .min(w / d.size.0)
            .min(h / d.size.1)
            .max(0.05);
        let size = (d.size.0 * scale, d.size.1 * scale);
        let x = place.x.unwrap_or(d.x).start(size.0, w);
        let y = place.y.unwrap_or(d.y).start(size.1, h);
        let (x, y) = inside(x, y, size, scale, (w, h));
        modules.push(Placed {
            module,
            rect: Panel {
                x,
                y,
                w: size.0,
                h: size.1,
            },
            natural: d.size,
            scale,
        });
    }
    Some(Hud {
        grid,
        cell,
        modules,
    })
}

fn inside(x: f32, y: f32, size: (f32, f32), scale: f32, window: (f32, f32)) -> (f32, f32) {
    let m = MARGIN * scale;
    let keep = |v: f32, size: f32, extent: f32| {
        let (lo, hi) = (-m, extent - size + m);
        if hi < lo {
            (extent - size) / 2.0
        } else {
            v.clamp(lo, hi)
        }
    };
    (keep(x, size.0, window.0), keep(y, size.1, window.1))
}

impl Hud {
    pub(crate) fn get(&self, module: Module) -> Option<&Placed> {
        self.modules.iter().find(|p| p.module == module)
    }

    pub(crate) fn hit(&self, p: [f32; 2], hovered: Option<Module>) -> Option<(Module, Part)> {
        self.zones(hovered)
            .into_iter()
            .find(|(_, r, _)| p[0] >= r.x && p[0] < r.x + r.w && p[1] >= r.y && p[1] < r.y + r.h)
            .map(|(m, _, part)| (m, part))
    }

    pub(crate) fn zones(&self, hovered: Option<Module>) -> Vec<(Module, Panel, Part)> {
        let mut out = Vec::with_capacity(self.modules.len() * 2 + 2);
        if let Some(m) = hovered.and_then(|m| self.get(m)) {
            out.push((m.module, m.close(), Part::Close));
            out.push((m.module, m.tab(), Part::Body));
        }
        for m in self.modules.iter().rev() {
            out.push((m.module, m.grip(), Part::Resize));
        }
        for m in self.modules.iter().rev() {
            out.push((m.module, m.outline(), Part::Body));
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Body,
    Resize,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Grab {
    pub(crate) module: Module,
    pub(crate) part: Part,
    pub(crate) from: [f32; 2],
    pub(crate) start: Placed,
}

pub(crate) fn press(
    hud: &Hud,
    at: [f32; 2],
    resize: bool,
    hovered: Option<Module>,
) -> Option<Grab> {
    let (module, part) = hud.hit(at, hovered)?;
    let part = if part == Part::Body && resize {
        Part::Resize
    } else {
        part
    };
    Some(Grab {
        module,
        part,
        from: at,
        start: *hud.get(module)?,
    })
}

pub(crate) fn drag(saved: &mut Saved, hud: &Hud, grab: &Grab, at: [f32; 2]) {
    let (w, h) = (hud.grid.width as f32, hud.grid.height as f32);
    let s = grab.start;
    let (dx, dy) = (at[0] - grab.from[0], at[1] - grab.from[1]);
    let mut place = saved.place(grab.module);
    match grab.part {
        Part::Close => return,
        Part::Body => {
            let (x, y) = snap(
                hud,
                s.rect.x + dx,
                s.rect.y + dy,
                s.rect.w,
                s.rect.h,
                s.natural.0,
            );
            place.x = Some(Anchored::of(x, s.rect.w, w));
            place.y = Some(Anchored::of(y, s.rect.h, h));
        }
        Part::Resize => {
            let grow = ((s.rect.w + dx) / s.rect.w).max((s.rect.h + dy) / s.rect.h);
            let fits = ((w - s.rect.x) / (s.natural.0 - MARGIN))
                .min((h - s.rect.y) / (s.natural.1 - MARGIN));
            let scale = (s.scale * grow)
                .clamp(MIN_SCALE, MAX_SCALE)
                .min(fits.max(MIN_SCALE));
            let size = (s.natural.0 * scale, s.natural.1 * scale);
            place.x = Some(Anchored::of(s.rect.x, size.0, w));
            place.y = Some(Anchored::of(s.rect.y, size.1, h));
            place.scale = Some(scale);
        }
    }
    saved.set(grab.module, place);
}

fn snap(hud: &Hud, x: f32, y: f32, w: f32, h: f32, natural_w: f32) -> (f32, f32) {
    let (ww, wh) = (hud.grid.width as f32, hud.grid.height as f32);
    let edge = 2.0 * hud.cell.0.max(4.0) - MARGIN;
    let top = super::TOP_ROWS * hud.cell.1.max(8.0) - MARGIN;
    let bottom = wh - super::BOTTOM_ROWS * hud.cell.1.max(8.0) + MARGIN;
    let settle = |v: f32, lines: [f32; 2]| {
        lines
            .into_iter()
            .find(|line| (v - line).abs() < SNAP)
            .unwrap_or(v)
    };
    let x = settle(x, [edge, ww - edge - w]);
    let y = settle(y, [top, bottom - h]);
    inside(x, y, (w, h), w / natural_w, (ww, wh))
}

pub(crate) fn release(
    saved: &mut Saved,
    hud: &Hud,
    grab: &Grab,
    at: [f32; 2],
    click: bool,
) -> bool {
    match grab.part {
        Part::Close => {
            let still = hud.hit(at, Some(grab.module)) == Some((grab.module, Part::Close));
            if click || still {
                let mut place = saved.place(grab.module);
                place.hidden = true;
                saved.set(grab.module, place);
                return true;
            }
            false
        }
        _ if click => false,
        _ => {
            drag(saved, hud, grab, at);
            true
        }
    }
}

pub(crate) fn local_layout(placed: &Placed, boards: usize) -> Layout {
    let c = placed.local_content();
    let rows: Vec<Panel> = (0..boards.clamp(1, super::campaign::MAX_BOARDS))
        .map(|i| Panel {
            x: c.x,
            y: c.y + AXIS_H + i as f32 * ROW_H,
            w: c.w,
            h: ROW_H,
        })
        .collect();
    Layout {
        panel: Panel {
            x: 0.0,
            y: 0.0,
            w: placed.natural.0,
            h: placed.natural.1,
        },
        title: c,
        heart: c,
        stages: c,
        axis: Panel { h: AXIS_H, ..c },
        rows,
        legend: c,
        ticker: c,
    }
}

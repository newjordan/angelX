//! What is printed on the Heartglass's plate: the title, the stage rail,
//! the lineage key, the five board tracks with their ticks, budget pips and
//! sparklines, the legend and the ticker line. One picture, drawn on the
//! CPU at twice the shown size (so a leaned plate's text stays sharp) and
//! drawn again only when the campaign, the cloud or the window changes.
//!
//! The parts that move with the clock (the collaboration countdown, the
//! budget reset, the ticker's age) are small pictures of their own, and
//! everything that glows or beats is a layer over the plate
//! (`heartglass::layers`), so a second of time costs a few kilobytes, not
//! the whole plate.
//!
//! Text comes through [`Typeset`], so the drawing is tested without a
//! font: the window's own font stack in the window, blocks in a test.

use super::campaign::{Board, Campaign, Cloud};
use super::heart::Built;
use super::{Layout, Panel};
use crate::ui::showcase_chart::Canvas;

/// Straight RGBA text pixels: `(width, height, rgba)`.
pub(crate) type Ink = (u32, u32, Vec<u8>);

/// Text rendered in the window's font, `size` its cell height in pixels.
pub(crate) trait Typeset {
    fn text(&mut self, line: &str, size: f32, ink: [u8; 3], bold: bool) -> Option<Ink>;
}

/// Shown text sizes (cell heights), before the plate's scale.
pub(crate) const TINY: f32 = 12.5;
pub(crate) const SMALL: f32 = 14.5;
pub(crate) const MEDIUM: f32 = 18.0;
pub(crate) const LARGE: f32 = 26.0;

/// The plate's palette.
pub(crate) const ROSE: [u8; 3] = [255, 76, 108];
pub(crate) const ROSE_DIM: [u8; 3] = [160, 52, 76];
pub(crate) const CYAN: [u8; 3] = [90, 216, 255];
pub(crate) const GOLD: [u8; 3] = [255, 198, 72];
pub(crate) const SILVER: [u8; 3] = [214, 222, 236];
pub(crate) const VIOLET: [u8; 3] = [184, 124, 255];
pub(crate) const INK: [u8; 3] = [196, 214, 232];
pub(crate) const DIM: [u8; 3] = [112, 132, 156];
pub(crate) const FAINT: [u8; 3] = [64, 80, 102];

/// What the plate shows of the cloud: its stages and lineage key.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CloudView {
    /// Each stage's label and whether it is ours.
    pub(crate) stages: Vec<(String, bool)>,
    /// The cardiac lineages, in palette order: name and colour.
    pub(crate) lineages: Vec<(String, [u8; 3])>,
}

impl CloudView {
    pub(crate) fn of(cloud: &Cloud) -> Self {
        let mut cardiac = vec![false; cloud.groups.len()];
        for stage in &cloud.stages {
            for p in &stage.points {
                if p.cardiac
                    && let Some(c) = cardiac.get_mut(usize::from(p.group))
                {
                    *c = true;
                }
            }
        }
        Self {
            stages: cloud
                .stages
                .iter()
                .map(|s| (s.label.clone(), s.predicted))
                .collect(),
            lineages: cloud
                .groups
                .iter()
                .zip(&cardiac)
                .filter(|(_, c)| **c)
                .map(|(g, _)| (g.name.clone(), g.rgb))
                .take(6)
                .collect(),
        }
    }
}

/// Everything the plate is drawn from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct View<'a> {
    pub(crate) campaign: Option<&'a Campaign>,
    pub(crate) cloud: Option<&'a CloudView>,
    /// The folder being watched, for the empty plate.
    pub(crate) dir: &'a str,
}

/// The score range the tracks share: the floor and everything known,
/// rounded out to fives, at least 45 to 80.
pub(crate) fn domain(campaign: Option<&Campaign>) -> (f64, f64) {
    let mut lo: f64 = 45.0;
    let mut hi: f64 = 80.0;
    if let Some(c) = campaign {
        for b in &c.boards {
            let known = [
                Some(b.floor),
                b.ve_leader,
                b.ve_top10,
                b.ve_partner,
                b.yukon_frontier,
                b.ours_best.as_ref().map(|x| x.score),
                b.proxy_best.as_ref().map(|x| x.score),
            ];
            for v in known
                .into_iter()
                .flatten()
                .chain(b.history.iter().map(|m| m.score))
            {
                lo = lo.min(((v - 1.0) / 5.0).floor() * 5.0);
                hi = hi.max(((v + 1.0) / 5.0).ceil() * 5.0);
            }
        }
    }
    (
        lo.clamp(0.0, 95.0),
        hi.clamp(lo.clamp(0.0, 95.0) + 5.0, 100.0),
    )
}

/// Where `score` sits on a rail from `x0` to `x1` over `domain`, clamped to
/// the rail.
pub(crate) fn score_x(score: f64, domain: (f64, f64), x0: f32, x1: f32) -> f32 {
    let k = ((score - domain.0) / (domain.1 - domain.0).max(1e-6)).clamp(0.0, 1.0) as f32;
    x0 + (x1 - x0) * k
}

/// A track row's parts, in shown pixels: the rail from `x0` to `x1` at
/// height `y`, and the right column from `right`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Track {
    pub(crate) x0: f32,
    pub(crate) x1: f32,
    pub(crate) y: f32,
    pub(crate) right: f32,
}

/// Width of a row's right column (our score, budget pips, sparkline).
pub(crate) const RIGHT: f32 = 100.0;

pub(crate) fn track(row: Panel) -> Track {
    Track {
        x0: row.x + 6.0,
        x1: row.x + row.w - RIGHT - 12.0,
        y: row.y + 30.0,
        right: row.x + row.w - RIGHT,
    }
}

/// Where the heartbeat's trace runs in the title strip: right of the
/// title, left of the phase and the countdown.
pub(crate) fn ecg_strip(title: Panel) -> Panel {
    let x = title.x + 168.0;
    Panel {
        x,
        y: title.y + 2.0,
        w: (title.x + title.w - 196.0 - x).max(20.0),
        h: 20.0,
    }
}

/// Where stage `i` of `n` sits on the stage rail: its slot's middle.
pub(crate) fn stage_slot(rail: Panel, i: usize, n: usize) -> [f32; 2] {
    let n = n.max(1) as f32;
    [
        rail.x + rail.w * (i as f32 + 0.5) / n,
        rail.y + rail.h * 0.5,
    ]
}

/// A colour mixed toward another, `k` of the way.
fn mix(a: [u8; 3], b: [u8; 3], k: f32) -> [u8; 3] {
    [0, 1, 2].map(|i| (f32::from(a[i]) * (1.0 - k) + f32::from(b[i]) * k).round() as u8)
}

/// How a line of text is set: its size (cell height, shown pixels), ink
/// and weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Style {
    pub(crate) size: f32,
    pub(crate) ink: [u8; 3],
    pub(crate) bold: bool,
}

impl Style {
    pub(crate) fn new(size: f32, ink: [u8; 3]) -> Self {
        Self {
            size,
            ink,
            bold: false,
        }
    }

    pub(crate) fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    pub(crate) fn bold_if(self, bold: bool) -> Self {
        Self { bold, ..self }
    }
}

/// The plate's canvas at `scale` pixels to a shown pixel, with helpers that
/// take shown coordinates relative to the plate.
pub(crate) struct Print<'t> {
    pub(crate) canvas: Canvas,
    s: f32,
    origin: [f32; 2],
    typeset: &'t mut dyn Typeset,
}

impl<'t> Print<'t> {
    pub(crate) fn new(panel: Panel, scale: f32, typeset: &'t mut dyn Typeset) -> Self {
        let s = scale.max(0.5);
        Self {
            canvas: Canvas::new((panel.w * s).ceil() as usize, (panel.h * s).ceil() as usize),
            s,
            origin: [panel.x, panel.y],
            typeset,
        }
    }

    fn p(&self, x: f32, y: f32) -> [f32; 2] {
        [(x - self.origin[0]) * self.s, (y - self.origin[1]) * self.s]
    }

    pub(crate) fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: [u8; 3], a: f32) {
        let [x0, y0] = self.p(x, y);
        self.canvas
            .rect(x0, y0, x0 + w * self.s, y0 + h * self.s, c, a);
    }

    pub(crate) fn line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, c: [u8; 3], alpha: f32) {
        let (pa, pb) = (self.p(a[0], a[1]), self.p(b[0], b[1]));
        self.canvas.line(pa, pb, width * self.s, c, alpha);
    }

    pub(crate) fn disc(&mut self, c: [f32; 2], r: f32, colour: [u8; 3], a: f32) {
        let pc = self.p(c[0], c[1]);
        self.canvas.disc(pc, r * self.s, colour, a);
    }

    /// A ring: a disc's outline, `width` across.
    pub(crate) fn ring(&mut self, c: [f32; 2], r: f32, width: f32, colour: [u8; 3], a: f32) {
        let pc = self.p(c[0], c[1]);
        let (r, half) = (r * self.s, width * self.s * 0.5);
        for y in (pc[1] - r - half - 1.0).floor() as i64..=(pc[1] + r + half + 1.0).ceil() as i64 {
            for x in
                (pc[0] - r - half - 1.0).floor() as i64..=(pc[0] + r + half + 1.0).ceil() as i64
            {
                let d =
                    ((x as f32 + 0.5 - pc[0]).powi(2) + (y as f32 + 0.5 - pc[1]).powi(2)).sqrt();
                let cover = (half + 0.5 - (d - r).abs()).clamp(0.0, 1.0);
                self.canvas.blend(x, y, colour, a * cover);
            }
        }
    }

    /// A soft round glow: strongest in the middle, gone at `r`.
    pub(crate) fn glow(&mut self, c: [f32; 2], r: f32, colour: [u8; 3], a: f32) {
        let pc = self.p(c[0], c[1]);
        let r = r * self.s;
        for y in (pc[1] - r).floor() as i64..=(pc[1] + r).ceil() as i64 {
            for x in (pc[0] - r).floor() as i64..=(pc[0] + r).ceil() as i64 {
                let d = ((x as f32 + 0.5 - pc[0]).powi(2) + (y as f32 + 0.5 - pc[1]).powi(2))
                    .sqrt()
                    / r;
                if d < 1.0 {
                    self.canvas.blend(x, y, colour, a * (1.0 - d) * (1.0 - d));
                }
            }
        }
    }

    /// A filled triangle, its edges smoothed.
    pub(crate) fn triangle(
        &mut self,
        a: [f32; 2],
        b: [f32; 2],
        c: [f32; 2],
        colour: [u8; 3],
        alpha: f32,
    ) {
        let (a, b, c) = (self.p(a[0], a[1]), self.p(b[0], b[1]), self.p(c[0], c[1]));
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if area.abs() < 1e-3 {
            return;
        }
        let sign = area.signum();
        let edge = |p: [f32; 2], q: [f32; 2], x: f32, y: f32| {
            let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
            sign * ((x - p[0]) * dy - (y - p[1]) * dx) / dx.hypot(dy).max(1e-6)
        };
        let (x0, x1) = (
            a[0].min(b[0]).min(c[0]).floor() as i64 - 1,
            a[0].max(b[0]).max(c[0]).ceil() as i64 + 1,
        );
        let (y0, y1) = (
            a[1].min(b[1]).min(c[1]).floor() as i64 - 1,
            a[1].max(b[1]).max(c[1]).ceil() as i64 + 1,
        );
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let d = edge(a, b, px, py)
                    .max(edge(b, c, px, py))
                    .max(edge(c, a, px, py));
                let cover = (0.5 - d).clamp(0.0, 1.0);
                if cover > 0.0 {
                    self.canvas.blend(x, y, colour, alpha * cover);
                }
            }
        }
    }

    /// A dashed line from `a` to `b`.
    pub(crate) fn dashed(
        &mut self,
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        dash: f32,
        c: [u8; 3],
        alpha: f32,
    ) {
        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
        let steps = (len / (2.0 * dash)).ceil().max(1.0) as usize;
        for i in 0..steps {
            let (k0, k1) = (
                (i as f32 * 2.0 * dash / len).min(1.0),
                ((i as f32 * 2.0 + 1.0) * dash / len).min(1.0),
            );
            let p = |k: f32| [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
            self.line(p(k0), p(k1), width, c, alpha);
        }
    }

    /// Text set at `size`, its top left at `(x, y)` (shown pixels); returns
    /// its shown width. `align`: 0 left, 0.5 centred, 1 right of `x`.
    pub(crate) fn text(&mut self, line: &str, [x, y]: [f32; 2], style: Style, align: f32) -> f32 {
        let Some(set) = self
            .typeset
            .text(line, style.size * self.s, style.ink, style.bold)
        else {
            return 0.0;
        };
        let w = set.0 as f32 / self.s;
        let [px, py] = self.p(x - w * align, y);
        self.canvas.text(&set, px, py);
        w
    }

    /// Text cut with an ellipsis to fit `max` shown pixels.
    pub(crate) fn text_fit(
        &mut self,
        line: &str,
        at: [f32; 2],
        style: Style,
        max: f32,
        align: f32,
    ) -> f32 {
        let Some(set) = self
            .typeset
            .text(line, style.size * self.s, style.ink, style.bold)
        else {
            return 0.0;
        };
        let w = set.0 as f32 / self.s;
        if w <= max {
            let [px, py] = self.p(at[0] - w * align, at[1]);
            self.canvas.text(&set, px, py);
            return w;
        }
        let chars: Vec<char> = line.chars().collect();
        let keep = ((chars.len() as f32 * max / w).floor() as usize).saturating_sub(1);
        let cut: String = chars[..keep.min(chars.len())].iter().collect::<String>() + "…";
        self.text(&cut, at, style, align)
    }

    /// The finished picture: `(width, height, straight rgba)`.
    pub(crate) fn finish(self) -> Ink {
        (self.canvas.w as u32, self.canvas.h as u32, self.canvas.rgba)
    }
}

/// A number as the boards show it: one decimal.
pub(crate) fn score_text(v: Option<f64>) -> String {
    v.map_or_else(|| "—".to_string(), |v| format!("{v:.1}"))
}

/// The plate for `layout` and `view`, at `scale` pixels a shown pixel.
pub(crate) fn draw(layout: &Layout, view: &View<'_>, scale: f32, typeset: &mut dyn Typeset) -> Ink {
    let mut print = Print::new(layout.panel, scale, typeset);
    title(&mut print, layout.title, view);
    viewport(&mut print, layout.heart, view);
    rail(&mut print, layout.stages, view);
    let domain = domain(view.campaign);
    boards(&mut print, layout, view, domain);
    if let Some(campaign) = view.campaign {
        ticker(&mut print, layout.ticker, campaign);
    }
    legend(&mut print, layout.legend);
    print.finish()
}

/// One module of the floating HUD (`hud::Module`) on its own picture: the
/// part of the plate it carries, from `layout` (the module's own, at its
/// origin), at `scale` pixels a shown pixel.
pub(crate) fn module(
    module: super::hud::Module,
    layout: &Layout,
    view: &View<'_>,
    scale: f32,
    typeset: &mut dyn Typeset,
) -> Ink {
    use super::hud::Module;
    let mut print = Print::new(layout.panel, scale, typeset);
    match module {
        Module::Title => title(&mut print, layout.title, view),
        Module::Heart => viewport(&mut print, layout.heart, view),
        Module::Stages => rail(&mut print, layout.stages, view),
        Module::Boards => boards(&mut print, layout, view, domain(view.campaign)),
        Module::Legend => legend(&mut print, layout.legend),
        Module::Ticker => {
            if let Some(campaign) = view.campaign {
                ticker(&mut print, layout.ticker, campaign);
            }
        }
    }
    let mut ink = print.finish();
    lift(&mut ink.2);
    ink
}

/// Ink lifted for the floating HUD, which has no plate under it: its dim
/// greys and faint rules brightened toward its lights (a gamma of 0.68),
/// so they hold over the text below; the bright inks barely move, and
/// none reaches white.
pub(crate) fn lift(rgba: &mut [u8]) {
    static CURVE: std::sync::OnceLock<[u8; 256]> = std::sync::OnceLock::new();
    let curve = CURVE.get_or_init(|| {
        std::array::from_fn(|v| ((v as f32 / 255.0).powf(0.68) * 255.0).round() as u8)
    });
    for px in rgba.chunks_exact_mut(4) {
        for c in &mut px[..3] {
            *c = curve[usize::from(*c)];
        }
    }
}

/// The score axis and the board rows under it: faint empty tracks with no
/// campaign.
fn boards(print: &mut Print<'_>, layout: &Layout, view: &View<'_>, domain: (f64, f64)) {
    axis(print, layout, domain);
    if let Some(campaign) = view.campaign {
        for (row, board) in layout.rows.iter().zip(&campaign.boards) {
            board_row(print, *row, board, domain);
        }
    } else {
        for row in &layout.rows {
            let t = track(*row);
            print.line([t.x0, t.y], [t.x1, t.y], 1.0, FAINT, 0.6);
        }
    }
}

fn title(print: &mut Print<'_>, t: Panel, view: &View<'_>) {
    print.text(
        "HEARTGLASS",
        [t.x, t.y + 2.0],
        Style::new(MEDIUM, [255, 228, 234]).bold(),
        0.0,
    );
    let quest = view.campaign.map(|c| {
        let q = c.quest;
        format!(
            "virtual embryo · {} proxy · {} official · {}/3 heart boards in the top 10",
            q.heart_proxy_runs, q.heart_submitted, q.heart_top10
        )
    });
    let sub =
        quest.unwrap_or_else(|| "virtual embryo · the mouse heart, E8.25 to E10.5".to_string());
    print.text_fit(
        &sub,
        [t.x, t.y + 25.0],
        Style::new(TINY, DIM),
        t.w - 30.0,
        0.0,
    );
    // The trace's floor: the beat itself is a layer.
    let e = ecg_strip(t);
    print.dashed(
        [e.x, e.y + e.h * 0.62],
        [e.x + e.w, e.y + e.h * 0.62],
        0.8,
        2.0,
        ROSE_DIM,
        0.45,
    );
    if let Some(c) = view.campaign
        && !c.phase.name.is_empty()
    {
        // The phase chip; the countdown beside it is the clock's picture.
        let x = t.x + t.w - 168.0;
        let w = print.text(
            &c.phase.name,
            [x, t.y + 3.0],
            Style::new(SMALL, GOLD).bold(),
            0.5,
        );
        print.ring([x, t.y + 3.0 + SMALL * 0.5], w * 0.5 + 8.0, 1.2, GOLD, 0.55);
        print.text("COLLAB ENDS", [t.x + t.w, t.y], Style::new(TINY, DIM), 1.0);
    }
    // The rim under the title strip.
    print.line(
        [t.x, t.y + t.h - 1.0],
        [t.x + t.w, t.y + t.h - 1.0],
        1.0,
        ROSE_DIM,
        0.7,
    );
}

fn viewport(print: &mut Print<'_>, v: Panel, view: &View<'_>) {
    // Corner ticks round the viewport: a frame without a box.
    let arm = 12.0;
    for (cx, cy, dx, dy) in [
        (v.x, v.y, 1.0, 1.0),
        (v.x + v.w, v.y, -1.0, 1.0),
        (v.x, v.y + v.h, 1.0, -1.0),
        (v.x + v.w, v.y + v.h, -1.0, -1.0),
    ] {
        print.line([cx, cy], [cx + dx * arm, cy], 1.2, CYAN, 0.55);
        print.line([cx, cy], [cx, cy + dy * arm], 1.2, CYAN, 0.55);
    }
    let Some(campaign) = view.campaign else {
        let mid = [v.x + v.w / 2.0, v.y + v.h * 0.42];
        print.text(
            "NO CAMPAIGN YET",
            [mid[0], mid[1] - 10.0],
            Style::new(MEDIUM, ROSE).bold(),
            0.5,
        );
        let watch = format!("watching {}/board.json", view.dir);
        print.text_fit(
            &watch,
            [mid[0], mid[1] + 14.0],
            Style::new(TINY, DIM),
            v.w - 24.0,
            0.5,
        );
        return;
    };
    let _ = campaign;
    let Some(cloud) = view.cloud else {
        let mid = [v.x + v.w / 2.0, v.y + v.h * 0.42];
        print.text(
            "NO CLOUD YET",
            [mid[0], mid[1] - 10.0],
            Style::new(MEDIUM, CYAN).bold(),
            0.5,
        );
        print.text_fit(
            "`vec cloud` writes heart_cloud.bin",
            [mid[0], mid[1] + 14.0],
            Style::new(TINY, DIM),
            v.w - 24.0,
            0.5,
        );
        return;
    };
    // The lineage key, top right inside the viewport.
    let mut y = v.y + 10.0;
    for (name, rgb) in &cloud.lineages {
        print.glow([v.x + v.w - 14.0, y + 6.0], 7.0, *rgb, 0.5);
        print.disc([v.x + v.w - 14.0, y + 6.0], 3.0, *rgb, 1.0);
        print.text(
            name,
            [v.x + v.w - 24.0, y],
            Style::new(TINY, mix(*rgb, INK, 0.35)),
            1.0,
        );
        y += 14.0;
    }
    print.disc([v.x + v.w - 14.0, y + 6.0], 2.2, [96, 108, 134], 1.0);
    print.text(
        "other tissue",
        [v.x + v.w - 24.0, y],
        Style::new(TINY, DIM),
        1.0,
    );
}

/// The stage rail under the viewport: each stage's slot, joined. Only
/// with a campaign and its cloud.
fn rail(print: &mut Print<'_>, rail: Panel, view: &View<'_>) {
    let (Some(_), Some(cloud)) = (view.campaign, view.cloud) else {
        return;
    };
    let n = cloud.stages.len();
    for i in 0..n {
        let [x, y] = stage_slot(rail, i, n);
        if i + 1 < n {
            let [nx, _] = stage_slot(rail, i + 1, n);
            print.dashed([x + 28.0, y], [nx - 28.0, y], 1.0, 3.0, FAINT, 0.9);
        }
        let (label, ours) = &cloud.stages[i];
        let ink = if *ours { GOLD } else { mix(CYAN, INK, 0.4) };
        print.text(
            label,
            [x, y - SMALL * 0.5],
            Style::new(SMALL, ink).bold_if(*ours),
            0.5,
        );
    }
}

fn axis(print: &mut Print<'_>, layout: &Layout, domain: (f64, f64)) {
    let a = layout.axis;
    let Some(first) = layout.rows.first() else {
        return;
    };
    let t = track(*first);
    print.text("BOARD", [a.x, a.y + 3.0], Style::new(TINY, DIM), 0.0);
    print.text(
        "OURS · BUDGET",
        [t.right + RIGHT, a.y + 3.0],
        Style::new(TINY, DIM),
        1.0,
    );
    let mut v = (domain.0 / 10.0).ceil() * 10.0;
    while v <= domain.1 + 1e-9 {
        let x = score_x(v, domain, t.x0, t.x1);
        if x > t.x0 + 48.0 || (v - 50.0).abs() < 1e-9 {
            print.text(
                &format!("{v:.0}"),
                [x, a.y + 3.0],
                Style::new(TINY, if (v - 50.0).abs() < 1e-9 { INK } else { DIM }),
                0.5,
            );
        }
        v += 10.0;
    }
}

fn board_row(print: &mut Print<'_>, row: Panel, b: &Board, domain: (f64, f64)) {
    let t = track(row);
    let accent = if b.heart { ROSE } else { CYAN };
    // The heart boards' rows are faintly warm; every row ends in a rule.
    if b.heart {
        print.rect(
            row.x - 6.0,
            row.y + 1.0,
            row.w + 12.0,
            row.h - 2.0,
            ROSE,
            0.035,
        );
    }
    print.line(
        [row.x - 6.0, row.y + row.h - 0.5],
        [row.x + row.w + 6.0, row.y + row.h - 0.5],
        0.8,
        FAINT,
        0.45,
    );

    // Line one: the board's tag and name, and the gap to the 10th place.
    let x = row.x;
    let w = print.text(
        &b.short,
        [x, row.y + 4.0],
        Style::new(SMALL, accent).bold(),
        0.0,
    );
    let gap = match (b.ours_best.as_ref(), b.ve_top10) {
        (Some(o), Some(top)) if o.score >= top => {
            Some((format!("+{:.1} over 10th", o.score - top), GOLD))
        }
        (Some(o), Some(top)) => Some((format!("{:.1} to 10th", top - o.score), DIM)),
        _ => None,
    };
    let gap_w = gap
        .as_ref()
        .map_or(0.0, |g| g.0.chars().count() as f32 * TINY * 0.62 + 12.0);
    print.text_fit(
        &b.label,
        [x + w + 8.0, row.y + 6.0],
        Style::new(TINY, DIM),
        (t.x1 - gap_w - (x + w + 8.0)).max(10.0),
        0.0,
    );
    if let Some((text, ink)) = gap {
        print.text(&text, [t.x1, row.y + 6.0], Style::new(TINY, ink), 1.0);
    }

    // Line two: the rail and its ticks.
    let y = t.y;
    print.line([t.x0, y], [t.x1, y], 1.4, [44, 60, 84], 0.95);
    for k in 0..=((domain.1 - domain.0) / 5.0).round() as i32 {
        let v = domain.0 + f64::from(k) * 5.0;
        let tx = score_x(v, domain, t.x0, t.x1);
        let major = (v / 10.0).fract().abs() < 1e-9;
        let r = if major { 2.5 } else { 1.5 };
        print.line([tx, y - r], [tx, y + r], 1.0, FAINT, 0.9);
    }
    // The floor: a dashed line across the row's rail.
    let fx = score_x(b.floor, domain, t.x0, t.x1);
    print.dashed([fx, y - 9.0], [fx, y + 9.0], 1.0, 2.0, INK, 0.55);
    if let Some(base) = b.baseline
        && (base - b.floor).abs() > 0.05
    {
        let bx = score_x(base, domain, t.x0, t.x1);
        print.line([bx, y - 3.0], [bx, y + 3.0], 1.0, DIM, 0.8);
    }
    // Proxy (ghost) and official (solid) bars from the floor.
    if let Some(p) = &b.proxy_best {
        let px = score_x(p.score, domain, t.x0, t.x1);
        let (l, r) = (fx.min(px), fx.max(px));
        print.rect(l, y - 4.5, (r - l).max(1.0), 9.0, accent, 0.10);
        print.line([l, y - 4.5], [r, y - 4.5], 1.0, accent, 0.45);
        print.line([l, y + 4.5], [r, y + 4.5], 1.0, accent, 0.45);
        print.ring([px, y], 3.8, 1.2, accent, 0.8);
    }
    if let Some(o) = &b.ours_best {
        let ox = score_x(o.score, domain, t.x0, t.x1);
        let (l, r) = (fx.min(ox), fx.max(ox));
        let fill = if o.score >= b.floor {
            accent
        } else {
            mix(accent, [60, 60, 70], 0.5)
        };
        print.glow([ox, y], 11.0, fill, 0.55);
        print.rect(l, y - 2.5, (r - l).max(1.0), 5.0, fill, 0.92);
        print.disc([ox, y], 3.8, [255, 240, 244], 1.0);
    }
    // The references: leader and 10th above, the team and Yukon below.
    if let Some(v) = b.ve_leader {
        let x = score_x(v, domain, t.x0, t.x1);
        print.line([x, y - 6.0], [x, y + 3.0], 1.2, GOLD, 0.9);
        print.triangle(
            [x - 4.5, y - 12.5],
            [x + 4.5, y - 12.5],
            [x, y - 6.0],
            GOLD,
            1.0,
        );
    }
    if let Some(v) = b.ve_top10 {
        let x = score_x(v, domain, t.x0, t.x1);
        print.line([x, y - 6.0], [x, y + 3.0], 1.0, SILVER, 0.75);
        print.triangle(
            [x - 3.6, y - 11.5],
            [x + 3.6, y - 11.5],
            [x, y - 6.0],
            SILVER,
            0.9,
        );
    }
    if let Some(v) = b.ve_partner {
        let x = score_x(v, domain, t.x0, t.x1);
        print.line([x, y - 3.0], [x, y + 6.0], 1.0, VIOLET, 0.8);
        print.triangle(
            [x - 3.6, y + 9.6],
            [x + 3.6, y + 9.6],
            [x, y + 6.0],
            VIOLET,
            1.0,
        );
        print.triangle(
            [x - 3.6, y + 9.6],
            [x, y + 13.2],
            [x + 3.6, y + 9.6],
            VIOLET,
            1.0,
        );
    }
    if let Some(v) = b.yukon_frontier {
        let x = score_x(v, domain, t.x0, t.x1);
        print.line([x, y - 3.0], [x, y + 6.0], 1.0, CYAN, 0.8);
        print.triangle(
            [x, y + 6.0],
            [x + 4.0, y + 12.5],
            [x - 4.0, y + 12.5],
            CYAN,
            1.0,
        );
    }

    // The right column: our best over the budget's pips and the sparkline.
    let ours = b.ours_best.as_ref().map(|o| o.score);
    let ink = match (ours, b.ve_top10) {
        (Some(o), Some(top)) if o >= top => GOLD,
        (Some(o), _) if o > b.floor => [255, 236, 240],
        (Some(_), _) => DIM,
        (None, _) => FAINT,
    };
    let right = t.right + RIGHT;
    let w = print.text(
        &score_text(ours),
        [right, row.y + 2.0],
        Style::new(MEDIUM, ink).bold(),
        1.0,
    );
    if ours.is_none()
        && let Some(p) = &b.proxy_best
    {
        print.text(
            &format!("proxy {:.1}", p.score),
            [right - w - 8.0, row.y + 6.0],
            Style::new(TINY, ROSE_DIM),
            1.0,
        );
    }
    let limit = b.budget.limit.min(4);
    for i in 0..limit {
        let c = [t.right + 6.0 + i as f32 * 10.0, y];
        if i < b.budget.limit.saturating_sub(b.budget.used) {
            print.glow(c, 6.5, accent, 0.35);
            print.disc(c, 3.0, accent, 1.0);
        } else {
            print.ring(c, 2.8, 1.0, DIM, 0.8);
        }
    }
    sparkline(
        print,
        [t.right + 6.0 + 4.0 * 10.0, y - 7.0, RIGHT - 46.0, 14.0],
        b,
        domain,
        accent,
    );
}

fn sparkline(
    print: &mut Print<'_>,
    [x, y, w, h]: [f32; 4],
    b: &Board,
    domain: (f64, f64),
    accent: [u8; 3],
) {
    if b.history.is_empty() {
        print.dashed([x, y + h / 2.0], [x + w, y + h / 2.0], 1.0, 2.0, FAINT, 0.8);
        return;
    }
    let n = b.history.len();
    let at = |i: usize, v: f64| {
        let k = ((v - domain.0) / (domain.1 - domain.0).max(1e-6)).clamp(0.0, 1.0) as f32;
        [
            x + if n > 1 {
                w * i as f32 / (n - 1) as f32
            } else {
                w / 2.0
            },
            y + h - h * k,
        ]
    };
    // The floor as a faint line through the spark.
    let floor = at(0, b.floor)[1];
    print.line([x, floor], [x + w, floor], 0.8, FAINT, 0.8);
    let mut last: Option<[f32; 2]> = None;
    for (i, m) in b.history.iter().enumerate() {
        let p = at(i, m.score);
        if m.official {
            if let Some(q) = last {
                print.line(q, p, 1.2, accent, 0.85);
            }
            last = Some(p);
            print.disc(p, 1.7, accent, 1.0);
        } else {
            print.disc(p, 1.2, accent, 0.35);
        }
    }
}

fn legend(print: &mut Print<'_>, l: Panel) {
    let y = l.y + l.h / 2.0;
    let mut x = l.x;
    let step = |print: &mut Print<'_>, x: &mut f32, label: &str| {
        let w = print.text(label, [*x, y - TINY / 2.0], Style::new(TINY, DIM), 0.0);
        *x += w + 11.0;
    };
    print.rect(x, y - 2.5, 12.0, 5.0, ROSE, 0.92);
    x += 16.0;
    step(print, &mut x, "ours");
    print.rect(x, y - 4.0, 12.0, 8.0, ROSE, 0.10);
    print.ring([x + 10.0, y], 3.0, 1.0, ROSE, 0.75);
    x += 17.0;
    step(print, &mut x, "proxy");
    print.dashed([x + 2.0, y - 6.0], [x + 2.0, y + 6.0], 1.0, 2.0, INK, 0.55);
    x += 7.0;
    step(print, &mut x, "50");
    print.triangle(
        [x - 4.0, y - 3.5],
        [x + 4.0, y - 3.5],
        [x, y + 3.0],
        GOLD,
        1.0,
    );
    x += 7.0;
    step(print, &mut x, "1st");
    print.triangle(
        [x - 3.5, y - 3.0],
        [x + 3.5, y - 3.0],
        [x, y + 2.5],
        SILVER,
        0.85,
    );
    x += 7.0;
    step(print, &mut x, "10th");
    print.triangle([x - 3.6, y], [x + 3.6, y], [x, y - 3.6], VIOLET, 1.0);
    print.triangle([x - 3.6, y], [x, y + 3.6], [x + 3.6, y], VIOLET, 1.0);
    x += 7.0;
    step(print, &mut x, "team");
    print.triangle(
        [x, y - 3.5],
        [x + 4.0, y + 3.0],
        [x - 4.0, y + 3.0],
        CYAN,
        1.0,
    );
    x += 7.0;
    step(print, &mut x, "Yukon");
}

fn ticker(print: &mut Print<'_>, t: Panel, campaign: &Campaign) {
    print.line([t.x, t.y], [t.x + t.w, t.y], 1.0, ROSE_DIM, 0.35);
    let Some(event) = campaign.events.last() else {
        print.text(
            "no events yet",
            [t.x, t.y + 8.0],
            Style::new(TINY, FAINT),
            0.0,
        );
        return;
    };
    let ink = match event.kind.as_str() {
        "scored" | "official" => ROSE,
        "proxy" => [230, 150, 170],
        "leaderboard" | "frontier" => GOLD,
        "cloud" => CYAN,
        _ => INK,
    };
    print.triangle(
        [t.x, t.y + 10.0],
        [t.x, t.y + 19.0],
        [t.x + 6.0, t.y + 14.5],
        ink,
        0.95,
    );
    // The age, right, is the clock's picture.
    print.text_fit(
        &event.text,
        [t.x + 13.0, t.y + 7.0],
        Style::new(SMALL, ink),
        t.w - 13.0 - 76.0,
        0.0,
    );
}

/// The clock's strings at `now` (Unix seconds): the countdown to the end
/// of collaboration, the time to the budget's reset, and the ticker's age.
pub(crate) fn clock_text(campaign: &Campaign, now: i64) -> (String, String, String) {
    let countdown = campaign.phase.collab_ends.map_or_else(String::new, |end| {
        let left = end - now;
        if left <= 0 {
            "ended".to_string()
        } else {
            let (d, h, m, s) = (left / 86_400, left / 3_600 % 24, left / 60 % 60, left % 60);
            format!("{d}d {h:02}:{m:02}:{s:02}")
        }
    });
    let reset = campaign.next_reset().map_or_else(String::new, |at| {
        let left = (at - now).max(0);
        format!("budget resets {}h{:02}m", left / 3_600, left / 60 % 60)
    });
    let age = campaign
        .events
        .last()
        .and_then(|e| e.at)
        .map_or_else(String::new, |at| ago(now - at));
    (countdown, reset, age)
}

/// "now", "42s", "7m", "3h", "2d" ago.
pub(crate) fn ago(secs: i64) -> String {
    match secs.max(0) {
        0..=9 => "now".to_string(),
        s @ 10..=59 => format!("{s}s ago"),
        s @ 60..=3_599 => format!("{}m ago", s / 60),
        s @ 3_600..=86_399 => format!("{}h ago", s / 3_600),
        s => format!("{}d ago", s / 86_400),
    }
}

/// A small picture of one line of text, `size` shown pixels at `scale`.
pub(crate) fn label(
    line: &str,
    size: f32,
    ink: [u8; 3],
    bold: bool,
    scale: f32,
    typeset: &mut dyn Typeset,
) -> Option<Ink> {
    if line.is_empty() {
        return None;
    }
    typeset.text(line, size * scale.max(0.5), ink, bold)
}

/// The short tag of the board `key`: the campaign's own, else the
/// contract's for its five boards, else the key's last part.
pub(crate) fn board_tag(key: &str, campaign: Option<&Campaign>) -> String {
    if let Some(b) = campaign.and_then(|c| c.boards.iter().find(|b| b.key == key)) {
        return b.short.clone();
    }
    match key {
        "T1:val" => "T1",
        "T2:heart:val_interp" => "HI",
        "T2:heart:val_extrap" => "HX",
        "T2:embryo:val_interp" => "EI",
        "T3:gata4" => "G4",
        other => other.rsplit(':').next().unwrap_or(other),
    }
    .chars()
    .take(12)
    .collect()
}

/// A stage's title for the top left of the viewport: its label large, and
/// what it is beneath (`board` is the tag of the board a prediction
/// answers).
pub(crate) fn stage_title(
    stage: &Built,
    board: &str,
    width: f32,
    scale: f32,
    typeset: &mut dyn Typeset,
) -> Ink {
    let (label, predicted, cells, cardiac) = (
        stage.label.as_str(),
        stage.predicted,
        stage.cells,
        stage.cardiac,
    );
    let panel = Panel {
        x: 0.0,
        y: 0.0,
        w: width,
        h: 46.0,
    };
    let mut print = Print::new(panel, scale, typeset);
    let ink = if predicted { GOLD } else { [236, 246, 255] };
    let w = print.text(label, [0.0, 0.0], Style::new(LARGE, ink).bold(), 0.0);
    let tag = if predicted {
        "OUR PREDICTION"
    } else {
        "OBSERVED · MERFISH"
    };
    print.text(
        tag,
        [w + 10.0, 7.0],
        Style::new(TINY, if predicted { GOLD } else { CYAN }).bold(),
        0.0,
    );
    let what = if predicted && !board.is_empty() {
        format!("{board} · {cells} cells · {cardiac} cardiac")
    } else {
        format!("{cells} cells · {cardiac} cardiac")
    };
    print.text(&what, [0.0, 27.0], Style::new(TINY, DIM), 0.0);
    print.finish()
}

/// The banner a newly landed official score raises.
pub(crate) fn flare_banner(
    short: &str,
    score: f64,
    scale: f32,
    typeset: &mut dyn Typeset,
) -> Option<Ink> {
    typeset.text(
        &format!("NEW OFFICIAL SCORE · {short} {score:.1}"),
        SMALL * scale.max(0.5),
        [255, 236, 240],
        true,
    )
}

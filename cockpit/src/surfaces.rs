//! Modular cockpit surfaces: text-critical vs visual backdrop.
//!
//! Goal: disentangle the **priority text path** (transcript, composer, tool
//! strip, reasoning *text*) from **backdrop visuals** (Scryglass world, agent
//! bay portrait chrome) so we can:
//! - keep interactive typing / stream paint lean
//! - later host backdrops out-of-process for quality
//! - stop copy selection from reading cells that belong to neighboring panels
//!
//! Phase 1 (this module): pure layout + mode flags. In-process paint still
//! happens in `draw.rs`, but every rect is tagged by [`SurfaceRole`] and
//! clipboard eligibility is role-driven.

use crate::mouse::{self, PaneId};
use crate::panels::PanelKind;
use ratatui::layout::{Constraint, Layout, Rect};

/// Priority class for a surface.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SurfaceRole {
    /// Transcript, input, tool strip — never drop for visuals.
    TextCritical,
    /// Scryglass / agent bay chrome — may be deferred, thinned, or externalized.
    Backdrop,
}

/// How backdrop surfaces are hosted.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum BackdropMode {
    /// Paint agent bay + Scryglass in-process (today's default).
    #[default]
    InProcess,
    /// Skip backdrop paint; transcript owns the full cockpit body (speed / headless).
    Off,
    /// Reserved: external backdrop process (future). Behaves as InProcess until wired.
    Lazy,
}

#[cfg(not(test))]
use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(not(test))]
const BACKDROP_UNCACHED: u8 = 0;
#[cfg(not(test))]
const BACKDROP_IN_PROCESS: u8 = 1;
#[cfg(not(test))]
const BACKDROP_OFF: u8 = 2;
#[cfg(not(test))]
const BACKDROP_LAZY: u8 = 3;

#[cfg(not(test))]
static CACHED_BACKDROP: AtomicU8 = AtomicU8::new(BACKDROP_UNCACHED);

pub(crate) fn invalidate_backdrop_cache() {
    #[cfg(not(test))]
    CACHED_BACKDROP.store(BACKDROP_UNCACHED, Ordering::Release);
}

impl BackdropMode {
    /// `ANGEL_BACKDROP=off|0|false` → Off; `lazy` → Lazy; else InProcess.
    ///
    /// Production path caches the first read (process-lifetime knobs). Tests
    /// re-read so `TestEnvGuard` can exercise synonyms.
    pub fn from_env() -> Self {
        #[cfg(not(test))]
        {
            match CACHED_BACKDROP.load(Ordering::Acquire) {
                BACKDROP_IN_PROCESS => return Self::InProcess,
                BACKDROP_OFF => return Self::Off,
                BACKDROP_LAZY => return Self::Lazy,
                _ => {}
            }
            let mode = Self::from_env_uncached();
            let code = match mode {
                Self::InProcess => BACKDROP_IN_PROCESS,
                Self::Off => BACKDROP_OFF,
                Self::Lazy => BACKDROP_LAZY,
            };
            CACHED_BACKDROP.store(code, Ordering::Release);
            mode
        }
        #[cfg(test)]
        Self::from_env_uncached()
    }

    fn from_env_uncached() -> Self {
        match std::env::var("ANGEL_BACKDROP") {
            Ok(v) => {
                let v = v.trim().to_ascii_lowercase();
                if matches!(v.as_str(), "0" | "false" | "off" | "no" | "text") {
                    Self::Off
                } else if matches!(v.as_str(), "lazy" | "process" | "external") {
                    Self::Lazy
                } else {
                    Self::InProcess
                }
            }
            Err(_) => Self::InProcess,
        }
    }

    pub fn paints_in_process(self) -> bool {
        matches!(self, Self::InProcess | Self::Lazy)
    }

    pub fn shows_side_column(self) -> bool {
        self.paints_in_process()
    }
}

/// Whether panel borders **overlap** (shared junction cells) or sit with a gap.
/// Shared junctions look dense but let reverse-video / native selection bleed
/// across columns; gapped layout isolates clipboard surfaces cleanly.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum SurfaceJoin {
    /// Historical condensed chrome: `spacing(-1)`.
    #[default]
    SharedBorder,
    /// One empty cell between panels so selection cannot straddle columns.
    Gap,
}

impl SurfaceJoin {
    /// Production caches the first env read; tests re-read for guard coverage.
    pub fn from_env() -> Self {
        #[cfg(not(test))]
        {
            static CACHED: std::sync::OnceLock<SurfaceJoin> = std::sync::OnceLock::new();
            *CACHED.get_or_init(Self::from_env_uncached)
        }
        #[cfg(test)]
        Self::from_env_uncached()
    }

    fn from_env_uncached() -> Self {
        match std::env::var("ANGEL_SURFACE_GAPS") {
            Ok(v) => {
                let v = v.trim().to_ascii_lowercase();
                if matches!(v.as_str(), "1" | "true" | "yes" | "on" | "gap") {
                    Self::Gap
                } else {
                    Self::SharedBorder
                }
            }
            Err(_) => Self::SharedBorder,
        }
    }

    pub fn spacing(self) -> i32 {
        match self {
            Self::SharedBorder => -1,
            Self::Gap => 1,
        }
    }
}

/// One laid-out surface for this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceRect {
    pub kind: PanelKind,
    pub role: SurfaceRole,
    /// Outer framed rect (includes border).
    pub frame: Rect,
    /// Clipboard-eligible content rect known at layout time (transcript prose).
    /// AgentBay is a backdrop here; its renderer separately registers the exact
    /// text viewport after reserving portraits and controls.
    pub copy_rect: Option<Rect>,
}

impl SurfaceRect {
    /// Map to mouse pane id (backdrop host / hit-test).
    #[allow(dead_code)]
    pub fn pane_id(&self) -> Option<PaneId> {
        match self.kind {
            PanelKind::Transcript => Some(PaneId::Transcript),
            PanelKind::AgentBay => Some(PaneId::AgentBay),
            PanelKind::Artifacts => Some(PaneId::Artifacts),
            PanelKind::Input => Some(PaneId::Input),
            PanelKind::Shell => Some(PaneId::Shell),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn accepts_clipboard_selection(&self) -> bool {
        matches!(self.role, SurfaceRole::TextCritical)
            && matches!(self.kind, PanelKind::Transcript)
            && self.copy_rect.is_some()
    }
}

/// Full cockpit-body layout for one frame (not header/input).
#[derive(Clone, Debug, Default)]
pub struct CockpitSurfacePlan {
    pub surfaces: Vec<SurfaceRect>,
    pub backdrop_mode: BackdropMode,
}

impl CockpitSurfacePlan {
    pub fn transcript_frame(&self) -> Option<Rect> {
        self.surfaces
            .iter()
            .find(|s| s.kind == PanelKind::Transcript)
            .map(|s| s.frame)
    }

    #[cfg(test)]
    pub fn transcript_copy_rect(&self) -> Option<Rect> {
        self.surfaces
            .iter()
            .find(|s| s.kind == PanelKind::Transcript)
            .and_then(|s| s.copy_rect)
    }

    pub fn agent_frame(&self) -> Option<Rect> {
        self.surfaces
            .iter()
            .find(|s| s.kind == PanelKind::AgentBay)
            .map(|s| s.frame)
    }

    pub fn artifacts_frame(&self) -> Option<Rect> {
        self.surfaces
            .iter()
            .find(|s| s.kind == PanelKind::Artifacts)
            .map(|s| s.frame)
    }
}

/// Inputs needed to split the cockpit body without touching `App`.
#[derive(Clone, Copy, Debug)]
pub struct CockpitLayoutInput {
    pub area: Rect,
    pub agent_active: bool,
    pub artifacts_active: bool,
    /// Trace pane has content (live or previous-turn reasoning, or a bg job) —
    /// agent bay takes up to 80% of the side column.
    pub reasoning_visible: bool,
    pub side_width: u16,
    /// Cap for idle agent bay (portrait card) height before min with column.
    pub idle_agent_height_cap: u16,
    pub backdrop: BackdropMode,
    pub join: SurfaceJoin,
}

/// Agent-bay height while the trace pane has content (up to 80% of the side
/// column, leaving a compact miniviz plate).
pub(crate) fn reasoning_agent_height(column_height: u16) -> u16 {
    if column_height == 0 {
        return 0;
    }
    let shared_height = column_height.saturating_add(1);
    let min_scryglass_h = if column_height >= 50 {
        18
    } else if column_height >= 40 {
        14
    } else if column_height >= 30 {
        10
    } else if column_height >= 20 {
        8
    } else {
        3.min(column_height)
    };
    let target = shared_height.saturating_mul(4) / 5;
    target
        .min(shared_height.saturating_sub(min_scryglass_h))
        .max(3.min(column_height))
        .min(column_height)
}

/// Minimum outer size for a bordered side panel (border + one content cell).
/// Below this the column is useless and used to emit zero-width/height frames
/// that still registered as AgentBay/Artifacts hit targets.
const MIN_SIDE_OUTER: u16 = 3;

/// Pure layout: cockpit body → transcript (+ optional side column surfaces).
pub fn plan_cockpit_surfaces(input: CockpitLayoutInput) -> CockpitSurfacePlan {
    plan_cockpit_surfaces_at_height(input, None)
}

/// Animated geometry shares exactly the same clamping, copy and hit surfaces.
pub(crate) fn plan_cockpit_surfaces_at_height(
    input: CockpitLayoutInput,
    agent_height: Option<u16>,
) -> CockpitSurfacePlan {
    let mut plan = CockpitSurfacePlan {
        surfaces: Vec::with_capacity(3),
        backdrop_mode: input.backdrop,
    };
    let area = input.area;
    if area.width == 0 || area.height == 0 {
        return plan;
    }

    // Clamp requested side width to the room left of the transcript Min(24).
    // A zero (or sub-border) request must not still open the column — that
    // produced AgentBay/Artifacts frames with width/height 0 (A6).
    let side_w = input
        .side_width
        .min(area.width.saturating_sub(24))
        .min(area.width);

    let show_side = input.backdrop.shows_side_column()
        && area.width >= 100
        && area.height >= MIN_SIDE_OUTER
        && side_w >= MIN_SIDE_OUTER
        && (input.agent_active || input.artifacts_active);

    if !show_side {
        plan.surfaces.push(transcript_surface(area));
        return plan;
    }

    let [left_area, right_area] =
        Layout::horizontal([Constraint::Min(24), Constraint::Length(side_w)])
            .spacing(input.join.spacing())
            .areas(area);

    // If layout still collapsed the column (pathological spacing / clamp), fall
    // back to full-body transcript rather than publishing empty / sub-min
    // backdrop rects (A6: width 1–2 used to slip past the zero-only check).
    if right_area.width < MIN_SIDE_OUTER || right_area.height < MIN_SIDE_OUTER {
        plan.surfaces.push(transcript_surface(area));
        return plan;
    }

    plan.surfaces.push(transcript_surface(left_area));

    let (agent_area, artifacts_area) = match (input.agent_active, input.artifacts_active) {
        (true, true) => {
            // Need room for two min-height plates (and optional shared junction).
            let min_split = match input.join {
                SurfaceJoin::SharedBorder => 5, // 3 + 3 − 1
                SurfaceJoin::Gap => 7,          // 3 + 3 + 1
            };
            if right_area.height < min_split {
                // Prefer one full-height plate over two empty/inverted bands.
                (Some(right_area), None)
            } else {
                let agent_h = if let Some(height) = agent_height {
                    height
                } else if input.reasoning_visible {
                    reasoning_agent_height(right_area.height)
                } else {
                    let max_agent_h = right_area.height.saturating_sub(19).max(3);
                    input
                        .idle_agent_height_cap
                        .min(max_agent_h)
                        .min(right_area.height)
                };
                // Reserve the other plate's minimum outer height, plus the Gap
                // spacer row when join is Gap. SharedBorder overlaps by one so
                // only the sibling Min(3) needs reserving (A6).
                let reserve_other = match input.join {
                    SurfaceJoin::SharedBorder => 3,
                    SurfaceJoin::Gap => 4, // Min(3) + spacing(1)
                };
                let agent_h = agent_h
                    .max(3)
                    .min(right_area.height.saturating_sub(reserve_other).max(3));
                let [agent_area, artifacts_area] =
                    Layout::vertical([Constraint::Length(agent_h), Constraint::Min(3)])
                        .spacing(input.join.spacing())
                        .areas(right_area);
                (Some(agent_area), Some(artifacts_area))
            }
        }
        (true, false) => (Some(right_area), None),
        (false, true) => (None, Some(right_area)),
        (false, false) => (None, None),
    };

    if let Some(frame) = agent_area {
        push_backdrop(&mut plan, PanelKind::AgentBay, frame);
    }
    if let Some(frame) = artifacts_area {
        push_backdrop(&mut plan, PanelKind::Artifacts, frame);
    }

    // If both backdrop pushes were skipped (empty / sub-min geometry), reclaim
    // full body rather than leaving a narrowed transcript with no side column.
    if plan.surfaces.len() == 1 {
        // only transcript from left_area — widen back to full cockpit body
        plan.surfaces.clear();
        plan.surfaces.push(transcript_surface(area));
    }

    plan
}

fn push_backdrop(plan: &mut CockpitSurfacePlan, kind: PanelKind, frame: Rect) {
    // A6: never publish a backdrop hit-target that cannot host Borders::ALL +
    // one content cell (or is sub-min outer).
    if frame.width < MIN_SIDE_OUTER || frame.height < MIN_SIDE_OUTER {
        return;
    }
    let inner = mouse::inner_border(frame);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    plan.surfaces.push(SurfaceRect {
        kind,
        role: SurfaceRole::Backdrop,
        frame,
        copy_rect: None,
    });
}

fn transcript_copy_rect(frame: Rect) -> Option<Rect> {
    // Panel-level prose (no tool-strip reservation). Live registration must call
    // [`transcript_live_copy_rect`] with the current strip height so selection
    // cannot cover the status/loading strip chrome.
    transcript_live_copy_rect(frame, 0)
}

/// Clipboard prose inside a transcript outer frame after reserving `strip_h`
/// bottom rows for the live tool strip and one column for the scroll rail.
///
/// A6: `strip_h == 0` matches the static plan copy rect; a positive strip must
/// shrink the copy surface so reverse-video / OSC-52 never sample strip cells.
pub fn transcript_live_copy_rect(frame: Rect, strip_h: u16) -> Option<Rect> {
    let inner = mouse::inner_border(frame);
    if inner.width == 0 || inner.height == 0 {
        return None;
    }
    let strip_h = strip_h.min(inner.height);
    let body_h = inner.height - strip_h;
    if body_h == 0 {
        return None;
    }
    let body = Rect {
        height: body_h,
        ..inner
    };
    if body.width < 2 {
        return Some(body);
    }
    let text_width = body.width.saturating_sub(1);
    Some(Rect {
        width: text_width,
        ..body
    })
}

fn transcript_surface(frame: Rect) -> SurfaceRect {
    SurfaceRect {
        kind: PanelKind::Transcript,
        role: SurfaceRole::TextCritical,
        frame,
        copy_rect: transcript_copy_rect(frame),
    }
}

/// Whether a pane's registered text viewport may own clipboard selection.
/// AgentBay's outer frame is a backdrop and is NOT eligible: its renderer
/// registers only the reasoning/process-output text, without portrait or chrome.
pub fn pane_accepts_clipboard(pane: PaneId) -> bool {
    matches!(pane, PaneId::Transcript | PaneId::AgentBay)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/surfaces__tests.rs"]
mod tests;

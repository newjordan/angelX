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

use crate::ui::mouse::{self, PaneId};
use crate::ui::panels::PanelKind;
use ratatui::layout::{Constraint, Layout, Rect};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SurfaceRole {
    TextCritical,
    Backdrop,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum BackdropMode {
    #[default]
    InProcess,
    Off,
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum SurfaceJoin {
    #[default]
    SharedBorder,
    Gap,
}

impl SurfaceJoin {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceRect {
    pub kind: PanelKind,
    pub role: SurfaceRole,
    pub frame: Rect,
    pub copy_rect: Option<Rect>,
}

impl SurfaceRect {
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

#[derive(Clone, Copy, Debug)]
pub struct CockpitLayoutInput {
    pub area: Rect,
    pub agent_active: bool,
    pub artifacts_active: bool,
    pub reasoning_visible: bool,
    pub side_width: u16,
    pub idle_agent_height_cap: u16,
    pub backdrop: BackdropMode,
    pub join: SurfaceJoin,
}

#[cfg(test)]
pub(crate) fn reasoning_agent_height(column_height: u16) -> u16 {
    reasoning_agent_height_percent(column_height, 80)
}

fn reasoning_agent_height_percent(column_height: u16, percent: u16) -> u16 {
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
    let target = (u32::from(shared_height) * u32::from(percent) / 100) as u16;
    target
        .min(shared_height.saturating_sub(min_scryglass_h))
        .max(3.min(column_height))
        .min(column_height)
}

const MIN_SIDE_OUTER: u16 = 3;

pub fn plan_cockpit_surfaces(input: CockpitLayoutInput) -> CockpitSurfacePlan {
    plan_cockpit_surfaces_at_height(input, None)
}

pub(crate) fn plan_cockpit_surfaces_at_height(
    input: CockpitLayoutInput,
    agent_height: Option<u16>,
) -> CockpitSurfacePlan {
    static DEFAULT: std::sync::OnceLock<angel_live::UiPolicy> = std::sync::OnceLock::new();
    plan_cockpit_surfaces_with_policy(input, agent_height, DEFAULT.get_or_init(Default::default))
}

pub(crate) fn plan_cockpit_surfaces_with_policy(
    mut input: CockpitLayoutInput,
    agent_height: Option<u16>,
    policy: &angel_live::UiPolicy,
) -> CockpitSurfacePlan {
    input.agent_active &= policy.show_agent;
    input.artifacts_active &= policy.show_artifacts;
    let mut plan = CockpitSurfacePlan {
        surfaces: Vec::with_capacity(3),
        backdrop_mode: input.backdrop,
    };
    let area = input.area;
    if area.width == 0 || area.height == 0 {
        return plan;
    }

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

    if right_area.width < MIN_SIDE_OUTER || right_area.height < MIN_SIDE_OUTER {
        plan.surfaces.push(transcript_surface(area));
        return plan;
    }

    plan.surfaces.push(transcript_surface(left_area));

    let (agent_area, artifacts_area) = match (input.agent_active, input.artifacts_active) {
        (true, true) => {
            let min_split = match input.join {
                SurfaceJoin::SharedBorder => 5,
                SurfaceJoin::Gap => 7,
            };
            if right_area.height < min_split {
                (Some(right_area), None)
            } else {
                let agent_h = if let Some(height) = agent_height {
                    height
                } else if input.reasoning_visible {
                    reasoning_agent_height_percent(right_area.height, policy.reasoning_percent)
                } else {
                    let max_agent_h = right_area.height.saturating_sub(19).max(3);
                    input
                        .idle_agent_height_cap
                        .min(max_agent_h)
                        .min(right_area.height)
                };
                let reserve_other = match input.join {
                    SurfaceJoin::SharedBorder => 3,
                    SurfaceJoin::Gap => 4,
                };
                let agent_h = agent_h
                    .max(3)
                    .min(right_area.height.saturating_sub(reserve_other).max(3));
                let [mut agent_area, mut artifacts_area] =
                    Layout::vertical([Constraint::Length(agent_h), Constraint::Min(3)])
                        .spacing(input.join.spacing())
                        .areas(right_area);
                if !policy.agent_first {
                    agent_area.y = right_area.y + right_area.height - agent_area.height;
                    artifacts_area.y = right_area.y;
                }
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

    if plan.surfaces.len() == 1 {
        plan.surfaces.clear();
        plan.surfaces.push(transcript_surface(area));
    }

    plan
}

fn push_backdrop(plan: &mut CockpitSurfacePlan, kind: PanelKind, frame: Rect) {
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
    transcript_live_copy_rect(frame, 0)
}

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

pub fn pane_accepts_clipboard(pane: PaneId) -> bool {
    matches!(pane, PaneId::Transcript | PaneId::AgentBay)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/surfaces__tests.rs"]
mod tests;

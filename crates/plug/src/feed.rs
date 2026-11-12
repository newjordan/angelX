use angel_live::telemetry::{
    AtlasHealth, ClerkHealth, GraphEvent, Measurement, MemoryHealth, ToolEventId, ToolOutcome,
};
use crossterm::event::KeyCode;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentKey {
    Codex,
    Luna,
    Astra,
    Daybreak,
    Glm,
    Kimi,
    Qwen,
    LongCat,
    Muse,
    Hy,
    Nemotron,
    Grok,
    DeepSeek,
    Gemma,
    Inkling,
    Laguna,
    North,
    Unknown,
}

#[derive(PartialEq)]
pub struct Route {
    pub label: String,
    pub driver: String,
    pub model: Option<String>,
    pub key: AgentKey,
}

#[derive(PartialEq)]
pub struct Serving {
    pub lead: Route,
    pub seats: Vec<Route>,
    pub turn: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Evidence {
    pub active: bool,
    pub observations: u32,
    pub checks: u32,
    pub contrasts: u32,
    pub inconclusive: u32,
}

#[derive(PartialEq)]
pub struct Health {
    pub memory: MemoryHealth,
    pub atlas: AtlasHealth,
    pub atlas_reviews: usize,
    pub clerk: ClerkHealth,
    pub clerk_queue: usize,
    pub clerk_conflicts: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopPhase {
    #[default]
    Idle,
    Baselining,
    Running,
    Verifying,
    AwaitingApproval,
    Paused,
    Done,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopTier {
    #[default]
    Local,
    Swarm,
    Sota,
}

#[derive(PartialEq)]
pub struct Checkpoint {
    pub loop_id: String,
    pub workspace: PathBuf,
    pub rows: Vec<(usize, bool)>,
    pub plans: Vec<(usize, serde_json::Value)>,
}

#[derive(PartialEq)]
pub struct LoopBudget {
    pub max_iters: usize,
    pub tokens_spent: usize,
    pub token_budget: usize,
    pub elapsed_secs: u64,
    pub deadline_secs: u64,
}

#[derive(PartialEq)]
pub struct LoopFacts {
    pub active: bool,
    pub has_id: bool,
    pub phase: LoopPhase,
    pub iteration: usize,
    pub tier: LoopTier,
    pub submissions: usize,
    pub podrace: bool,
    pub pivot: usize,
    pub measured_candidates: usize,
    pub verifier_blocked: Option<String>,
    pub task: String,
    pub stale_count: usize,
    pub checkpoint: Option<Checkpoint>,
    pub last_log_all_errors: bool,
    pub idle_log_tail: usize,
    pub promotions: usize,
    pub last_promotion_score: Option<String>,
    pub promotion_failures: usize,
    pub last_promotion_failure_score: Option<String>,
    pub budget: LoopBudget,
}

#[derive(PartialEq)]
pub enum Competition<'a> {
    Started(&'a str),
    Pending(&'a str),
    Reported { label: &'a str, score: &'a str },
    Complete(&'a str),
    Error { label: &'a str, reason: &'a str },
    TimedOut(&'a str),
}

#[derive(PartialEq)]
pub enum Feed<'a> {
    Ticks(u32),
    ToolCall {
        id: &'a ToolEventId,
        name: &'a str,
        args: &'a str,
        painted: bool,
    },
    ToolResult {
        id: &'a ToolEventId,
        name: &'a str,
        summary: &'a str,
        outcome: ToolOutcome,
        painted: bool,
    },
    Notice {
        text: &'a str,
        painted: bool,
    },
    Compacted,
    Evidence(Evidence),
    Lesson(&'a str),
    LoopIdle,
    TurnEnded {
        ok: bool,
        painted: bool,
    },
    Workspace(&'a Path),
}

#[derive(Default)]
pub struct StageAct {
    pub handled: bool,
    pub open_assets: bool,
    pub open_catalog: bool,
    pub enter_interior: bool,
    pub toggle_camera: bool,
    pub say: Option<String>,
}

pub trait FeedWorld {
    fn feed(&mut self, _event: Feed<'_>) {}

    fn graph_event(&mut self, _id: &ToolEventId, _event: &GraphEvent) {}

    fn competition(&mut self, _event: Competition<'_>) {}

    fn health(&mut self, _read: &dyn Fn() -> Health) {}

    fn serving(&mut self, _read: &dyn Fn() -> Serving) {}

    fn horse_service(&mut self, _read: &dyn Fn() -> (Route, Serving), _calls: u32) {}

    fn measured(&mut self, _read: &dyn Fn() -> Vec<(String, Measurement)>) {}

    fn adventure(&mut self, _read: &dyn Fn() -> LoopFacts, _party: u8) {}

    fn loop_noted(&mut self, read: &dyn Fn() -> LoopFacts) -> bool {
        read().active
    }

    fn loop_snapshot(&mut self, _read: &dyn Fn() -> LoopFacts) {}

    fn wake_village(&mut self) {}

    fn drain_village(&mut self) -> usize {
        0
    }
}

pub trait FeedRules {
    fn first_person_key(&mut self, _code: KeyCode) -> StageAct {
        StageAct::default()
    }

    fn map_key(&mut self, _code: KeyCode) -> StageAct {
        StageAct::default()
    }

    fn leave_interior(&mut self) -> Option<bool> {
        None
    }
}

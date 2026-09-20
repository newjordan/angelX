use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Set while Math God owns process env (role pins + Sol ultra / Grok xhigh).
/// Other formations must clear it so those pins cannot leak into a later mode.
static MATH_GOD_PINS_ARMED: AtomicBool = AtomicBool::new(false);

pub(crate) const SELECTION_DISSOLVE: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum FormationId {
    GpuComp,
    SoloStrike,
    Recon,
    Duel,
    Council,
    AllIn,
    /// Grok-managed frontier trio: GLM 5.2 + DeepSeek v4-pro + OpenAI Sol@max.
    GrokWar,
    /// Local pair carries the turn — Spark's Leanstral math engine leads and
    /// synthesizes, Turbo is the second corner; an OpenAI Sol seat is tagged in
    /// for advice only when the two locals disagree.
    TagTeam,
    /// Lean/math solver: one Sol@ultra head plus GLM-5.3 and DeepSeek v4 Pro
    /// in the mix; Grok xhigh scouts and weighs in. Leanstral is a send-to
    /// specialist tool, not a seat.
    MathGod,
    /// The standing bag mixture, expressed as a formation so the auto sota-moa
    /// and the roster deck are two doors to one engine. Its seats are NOT
    /// hand-pinned: they resolve to the live intelligence order over whatever
    /// SOTA links are configured and reachable right now — the exact behavior
    /// the bag's implicit `sota_moa_club_with_breadth` path has always had.
    AutoMoa,
}

impl FormationId {
    pub(crate) const fn cache_key(self) -> u64 {
        match self {
            Self::GpuComp => 0x51_00,
            Self::SoloStrike => 0x51_01,
            Self::Recon => 0x51_02,
            Self::Duel => 0x51_03,
            Self::Council => 0x51_04,
            Self::AllIn => 0x51_05,
            Self::GrokWar => 0x51_06,
            Self::TagTeam => 0x51_07,
            Self::MathGod => 0x51_08,
            Self::AutoMoa => 0x51_09,
        }
    }

    #[cfg(test)]
    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::GpuComp => "gpu-comp",
            Self::SoloStrike => "solo-strike",
            Self::Recon => "recon",
            Self::Duel => "duel",
            Self::Council => "council",
            Self::AllIn => "all-in",
            Self::GrokWar => "grok-war",
            Self::TagTeam => "tag-team",
            Self::MathGod => "math-god",
            Self::AutoMoa => "auto-moa",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum FormationRole {
    Scout,
    Propose,
    Judge,
    Verify,
    Aggregate,
}

impl FormationRole {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Scout => "SCOUT",
            Self::Propose => "PROPOSE",
            Self::Judge => "JUDGE",
            Self::Verify => "VERIFY",
            Self::Aggregate => "SYNTH",
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Scout => "S",
            Self::Propose => "P",
            Self::Judge => "J",
            Self::Verify => "V",
            Self::Aggregate => "A",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormationSlot {
    pub(crate) role: FormationRole,
    pub(crate) ordinal: usize,
    /// Adaptive proposer capacity beyond the first wave. It still needs a model
    /// before engagement because the pipeline may activate it on a hard turn.
    pub(crate) reserve: bool,
}

impl FormationSlot {
    pub(crate) fn label(&self) -> String {
        format!("{}{}", self.role.code(), self.ordinal + 1)
    }

    pub(crate) fn graph_label(&self) -> String {
        if self.reserve {
            format!("{}·R", self.label())
        } else {
            self.label()
        }
    }
}

/// Stable-enough reference to one concrete Bag route for the lifetime of the
/// cockpit. Agent/slot indices are append-only during fleet discovery; the
/// labels are retained for receipts and for a useful stale-route error.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MoaModelRef {
    pub(crate) agent_index: usize,
    pub(crate) slot_index: usize,
    pub(crate) agent: String,
    pub(crate) driver: String,
    pub(crate) model: String,
    pub(crate) route_id: crate::backplane::RouteId,
    pub(crate) expected_revision: crate::backplane::ModelRevision,
    pub(crate) metered: bool,
}

impl MoaModelRef {
    pub(crate) fn display_label(&self) -> String {
        if self.agent.eq_ignore_ascii_case(&self.model) {
            self.model.clone()
        } else {
            format!("{} / {}", self.agent, self.model)
        }
    }

    fn matches(&self, selector: &str) -> bool {
        let selector = selector.trim().to_ascii_lowercase();
        if selector.is_empty() {
            return false;
        }
        [&self.agent, &self.driver, &self.model]
            .iter()
            .any(|field| {
                let field = field.to_ascii_lowercase();
                field == selector || field.contains(&selector)
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MoaModelChoice {
    pub(crate) route: MoaModelRef,
    pub(crate) available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormationRoster {
    formation: FormationId,
    assignments: Vec<Option<MoaModelRef>>,
    /// Per-role reasoning-effort overrides staged in the deck (the roster
    /// card's THINK column). A staged role wins its seat at engagement;
    /// unset roles keep the env-derived seat policy.
    role_efforts: HashMap<FormationRole, String>,
}

impl FormationRoster {
    pub(crate) fn new(formation: FormationId, models: &[MoaModelChoice]) -> Self {
        let slots = crate::formations::formation(formation).slots();
        let assignments = slots
            .iter()
            .map(|slot| recommended_model(formation, slot, models))
            .collect();
        let mut role_efforts = HashMap::new();
        if formation == FormationId::MathGod {
            role_efforts.insert(FormationRole::Propose, "ultra".into());
            role_efforts.insert(FormationRole::Aggregate, "ultra".into());
            role_efforts.insert(FormationRole::Judge, "xhigh".into());
        }
        Self {
            formation,
            assignments,
            role_efforts,
        }
    }

    pub(crate) fn role_effort(&self, role: FormationRole) -> Option<&str> {
        self.role_efforts.get(&role).map(String::as_str)
    }

    /// Stage (or clear, with `None`) one role's effort override. Values are
    /// requests — the engaged seat's club validates its own dialect.
    pub(crate) fn set_role_effort(&mut self, role: FormationRole, effort: Option<&str>) {
        match effort.map(str::trim).filter(|effort| !effort.is_empty()) {
            Some(effort) => {
                self.role_efforts.insert(role, effort.to_ascii_lowercase());
            }
            None => {
                self.role_efforts.remove(&role);
            }
        }
    }

    /// The staged THINK column as the swarm's per-seat effort overrides — the
    /// one roster → [`crate::swarm::SeatEfforts`] mapping, consumed at arming
    /// (the receipt names each staged seat) and mirrored by Bag activation.
    /// Scout has no seat here: the swarm carries no scout effort policy, so
    /// the picker refuses that seat aloud rather than dropping a value silently.
    pub(crate) fn seat_efforts(&self) -> crate::swarm::SeatEfforts {
        crate::swarm::SeatEfforts {
            propose: self.role_effort(FormationRole::Propose).map(str::to_string),
            judge: self.role_effort(FormationRole::Judge).map(str::to_string),
            verify: self.role_effort(FormationRole::Verify).map(str::to_string),
            aggregate: self
                .role_effort(FormationRole::Aggregate)
                .map(str::to_string),
        }
    }

    pub(crate) fn formation(&self) -> FormationId {
        self.formation
    }

    pub(crate) fn assignments(&self) -> &[Option<MoaModelRef>] {
        &self.assignments
    }

    pub(crate) fn assignment(&self, index: usize) -> Option<&MoaModelRef> {
        self.assignments.get(index).and_then(Option::as_ref)
    }

    pub(crate) fn assign(&mut self, index: usize, model: MoaModelRef) -> bool {
        let Some(slot) = self.assignments.get_mut(index) else {
            return false;
        };
        *slot = Some(model);
        true
    }

    pub(crate) fn assigned_count(&self) -> usize {
        self.assignments
            .iter()
            .filter(|model| model.is_some())
            .count()
    }

    pub(crate) fn slot_count(&self) -> usize {
        self.assignments.len()
    }

    pub(crate) fn is_ready(&self) -> bool {
        !self.assignments.is_empty() && self.assigned_count() == self.slot_count()
    }

    pub(crate) fn metered_count(&self) -> usize {
        self.assignments
            .iter()
            .flatten()
            .filter(|model| model.metered)
            .count()
    }

    pub(crate) fn local_count(&self) -> usize {
        self.assigned_count().saturating_sub(self.metered_count())
    }

    pub(crate) fn missing_labels(&self) -> Vec<String> {
        crate::formations::formation(self.formation)
            .slots()
            .into_iter()
            .zip(&self.assignments)
            .filter_map(|(slot, model)| model.is_none().then(|| slot.label()))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MoaEngagement {
    pub(crate) id: u64,
    pub(crate) formation: FormationId,
    pub(crate) roster: FormationRoster,
}

impl MoaEngagement {
    pub(crate) fn new(formation: FormationId, roster: FormationRoster) -> Option<Self> {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        (roster.formation() == formation).then(|| Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            formation,
            roster,
        })
    }

    /// Roster-less engagement for tests that only need a formation identity.
    #[cfg(test)]
    pub(crate) fn unassigned(formation: FormationId) -> Self {
        Self {
            id: 0,
            formation,
            roster: FormationRoster::new(formation, &[]),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Formation {
    pub(crate) id: FormationId,
    pub(crate) name: &'static str,
    pub(crate) flavor: &'static str,
    pub(crate) width: usize,
    pub(crate) max_width: usize,
    pub(crate) layers: usize,
    pub(crate) samples: usize,
    pub(crate) judge: bool,
    pub(crate) judge_panel: usize,
    pub(crate) verify: usize,
    pub(crate) verify_guard: bool,
    pub(crate) reflect: bool,
    pub(crate) scout: bool,
    pub(crate) est_cost_x: f32,
    asset_rel: &'static str,
}

const BUILT_INS: &[Formation] = &[
    Formation {
        id: FormationId::SoloStrike,
        name: "Solo Strike",
        flavor: "Coordinator only. Resting stance, no fan-out.",
        width: 2,
        max_width: 2,
        layers: 1,
        samples: 1,
        judge: false,
        judge_panel: 1,
        verify: 0,
        verify_guard: false,
        reflect: false,
        scout: false,
        est_cost_x: 1.0,
        asset_rel: "assets/moa-cards/solo-strike.png",
    },
    Formation {
        id: FormationId::Recon,
        name: "Outriders",
        flavor: "Coordinator plus live scout. Best for fresh context before committing spend.",
        width: 2,
        max_width: 2,
        layers: 1,
        samples: 1,
        judge: false,
        judge_panel: 1,
        verify: 0,
        verify_guard: false,
        reflect: false,
        scout: true,
        est_cost_x: 2.0,
        asset_rel: "assets/moa-cards/recon.png",
    },
    Formation {
        id: FormationId::Duel,
        name: "Errant",
        flavor: "Two proposers, one judge, then synthesis. Good for tradeoffs.",
        width: 2,
        max_width: 2,
        layers: 1,
        samples: 1,
        judge: true,
        judge_panel: 1,
        verify: 0,
        verify_guard: false,
        reflect: false,
        scout: false,
        est_cost_x: 5.0,
        asset_rel: "assets/moa-cards/duel.png",
    },
    Formation {
        id: FormationId::Council,
        name: "Council",
        flavor: "Classic mixture: proposer panel, judge, verify guard, aggregate.",
        width: 3,
        max_width: 3,
        layers: 1,
        samples: 1,
        judge: true,
        judge_panel: 1,
        verify: 1,
        verify_guard: true,
        reflect: false,
        scout: false,
        est_cost_x: 8.0,
        asset_rel: "assets/moa-cards/council.png",
    },
    Formation {
        id: FormationId::AllIn,
        name: "Full Muster",
        flavor: "Full ritual: panel judge, verify guard, reflect, scout, multiple samples.",
        width: 3,
        max_width: 4,
        layers: 2,
        samples: 2,
        judge: true,
        judge_panel: 3,
        verify: 1,
        verify_guard: true,
        reflect: true,
        scout: true,
        est_cost_x: 15.0,
        asset_rel: "assets/moa-cards/all-in.png",
    },
    Formation {
        id: FormationId::GpuComp,
        name: "GPU Night Shift",
        flavor: "Kernel MoA: Turbo proposes (12 lanes); Grok scouts + fix critique; Leanstral math; DICE CUDA advise; OpenAI audit 45m.",
        width: 2,
        max_width: 3,
        layers: 1,
        samples: 1,
        judge: true,
        judge_panel: 1,
        verify: 1,
        verify_guard: true,
        reflect: true,
        // Grok research scout + first-class grok_research tool on kernel turns.
        scout: true,
        est_cost_x: 7.5,
        asset_rel: "assets/moa-cards/all-in.png",
    },
    Formation {
        id: FormationId::GrokWar,
        name: "Grok War",
        flavor: "Grok manages the SOTA trio: GLM-5.2 + DeepSeek v4-pro + OpenAI Sol@max propose; Grok scouts, judges, and synthesizes.",
        width: 3,
        max_width: 3,
        layers: 1,
        samples: 1,
        judge: true,
        judge_panel: 1,
        verify: 0,
        verify_guard: false,
        reflect: false,
        // Grok research scout + grok_research tool; aggregate/judge seats also Grok.
        scout: true,
        est_cost_x: 12.0,
        asset_rel: "assets/moa-cards/council.png",
    },
    Formation {
        id: FormationId::TagTeam,
        name: "Tag Team",
        flavor: "Local pair carries the turn: Spark's Leanstral math engine leads and synthesizes, Turbo works the second corner. Sol is tagged in for advice only when the two drafts disagree.",
        width: 2,
        max_width: 2,
        layers: 1,
        samples: 1,
        // The Sol seat. The dissent gate is what makes it *advice on demand*:
        // agreeing local drafts relax the judge away, diverging ones call it in.
        judge: true,
        judge_panel: 1,
        verify: 0,
        verify_guard: false,
        reflect: false,
        scout: false,
        // Two fleet calls cost nothing; the metered Sol judge fires only on
        // disagreement, so the expected spend sits well under one frontier turn.
        est_cost_x: 2.0,
        asset_rel: "assets/moa-cards/duel.png",
    },
    Formation {
        id: FormationId::MathGod,
        name: "Math God",
        flavor: "Lean/math solver: one Sol@ultra head plus GLM-5.3 and DeepSeek v4 Pro in the mix; Grok xhigh scouts and weighs in. Leanstral is a send-to tool for lemmas/formulas, not a conversation seat.",
        width: 3,
        max_width: 3,
        layers: 1,
        samples: 1,
        judge: true,
        judge_panel: 1,
        verify: 0,
        verify_guard: false,
        reflect: false,
        scout: true,
        // Sol head + GLM + DeepSeek mix + Grok weigh-in. Leanstral is a tool, not a seat.
        est_cost_x: 10.0,
        asset_rel: "assets/moa-cards/council.png",
    },
    Formation {
        id: FormationId::AutoMoa,
        name: "Auto MoA",
        flavor: "The standing club mixture: every configured SOTA link in intelligence order — propose, judge, verify, synthesize — re-resolved from whatever is reachable each engagement.",
        // Width mirrors the historical auto path: it forms whatever it has.
        width: 3,
        max_width: 4,
        layers: 1,
        samples: 1,
        judge: true,
        judge_panel: 1,
        verify: 1,
        verify_guard: true,
        reflect: false,
        scout: false,
        est_cost_x: 12.0,
        asset_rel: "assets/moa-cards/council.png",
    },
];

pub(crate) fn built_in_deck() -> &'static [Formation] {
    BUILT_INS
}

pub(crate) fn formation(id: FormationId) -> &'static Formation {
    BUILT_INS
        .iter()
        .find(|formation| formation.id == id)
        .expect("formation id is in built-in deck")
}

impl Formation {
    pub(crate) fn is_resting(self) -> bool {
        self.id == FormationId::SoloStrike
    }

    pub(crate) fn asset_path(self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(self.asset_rel)
    }

    pub(crate) fn est_cost_label(self) -> String {
        if (self.est_cost_x.fract()).abs() < f32::EPSILON {
            format!("{:.0}x", self.est_cost_x)
        } else {
            format!("{:.1}x", self.est_cost_x)
        }
    }

    pub(crate) fn stage_summary(self) -> String {
        let mut stages = vec![format!("propose x{}", self.width)];
        if self.layers > 1 {
            stages.push(format!("layers x{}", self.layers));
        }
        if self.judge {
            stages.push(if self.judge_panel > 1 {
                format!("judge panel x{}", self.judge_panel)
            } else {
                "judge".to_string()
            });
        }
        if self.verify > 0 {
            stages.push(format!("verify x{}", self.verify));
        }
        if self.reflect {
            stages.push("reflect".to_string());
        }
        if self.scout {
            stages.push("scout".to_string());
        }
        stages.push("aggregate".to_string());
        stages.join(" -> ")
    }

    /// Concrete operator-selectable execution seats. `max_width` is used for
    /// proposer seats so adaptive fan-out cannot introduce an unrostered model;
    /// synthesis samples receive independent seats for the same reason.
    pub(crate) fn slots(self) -> Vec<FormationSlot> {
        if self.is_resting() {
            return Vec::new();
        }
        let mut slots = Vec::new();
        if self.scout {
            slots.push(FormationSlot {
                role: FormationRole::Scout,
                ordinal: 0,
                reserve: false,
            });
        }
        slots.extend((0..self.max_width).map(|ordinal| FormationSlot {
            role: FormationRole::Propose,
            ordinal,
            reserve: ordinal >= self.width,
        }));
        if self.judge {
            slots.extend((0..self.judge_panel).map(|ordinal| FormationSlot {
                role: FormationRole::Judge,
                ordinal,
                reserve: false,
            }));
        }
        slots.extend((0..self.verify).map(|ordinal| FormationSlot {
            role: FormationRole::Verify,
            ordinal,
            reserve: false,
        }));
        slots.extend((0..self.samples.max(1)).map(|ordinal| FormationSlot {
            role: FormationRole::Aggregate,
            ordinal,
            reserve: false,
        }));
        slots
    }

    pub(crate) fn apply_sota_env(self) {
        // The formation's own name, so the MoA ledgers can attribute per-turn
        // economics to the exact formation that spent them. The resting stance
        // owns no turns, so disengaging clears the marker.
        if self.is_resting() {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var("ANGEL_SOTA_MOA_FORMATION") };
        } else {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_FORMATION", self.name) };
        }
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_MOA_MODE", "manual") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_MOA_ALWAYS", "0") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_MOA_MAX", "0") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_MOA_WIDTH", self.width.to_string()) };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_MOA_MAX_WIDTH", self.max_width.to_string()) };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_MOA_LAYERS", self.layers.to_string()) };
        // Refinement layers fan out at the formation's own width, not the
        // generic 6-seat default: layers=2 with width=3 must spend 3 aggregator
        // calls per layer, each carrying the full history plus every draft.
        // Formations with a single layer never refine, so the knob is cleared
        // (the disengage path lands here too — SoloStrike layers=1).
        if self.layers > 1 {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SWARM_REFINE_WIDTH", self.width.to_string()) };
        } else {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var("ANGEL_SWARM_REFINE_WIDTH") };
        }
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_MOA_SAMPLES", self.samples.to_string()) };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_MOA_JUDGE_PANEL", self.judge_panel.to_string()) };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SOTA_MOA_VERIFY", self.verify.to_string()) };
        set_flag("ANGEL_SOTA_MOA_JUDGE", self.judge);
        set_flag("ANGEL_SOTA_MOA_VERIFY_GUARD", self.verify_guard);
        set_flag("ANGEL_SOTA_MOA_REFLECT", self.reflect);
        set_flag("ANGEL_SOTA_MOA_GROK_RESEARCH", self.scout);
        // The deck's scout is the configured Grok-style SOTA scout. SearXNG
        // search stays off unless a future user deck explicitly asks for it.
        set_flag("ANGEL_SOTA_MOA_RESEARCH", false);
        self.apply_specialized_env();
        crate::club::resync_reasoning_effort_env_from_env();
    }

    fn apply_specialized_env(self) {
        if self.id != FormationId::MathGod {
            clear_math_god_pins();
        }
        if self.id == FormationId::GpuComp {
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_GPU_COMP_LOCAL_MOA", "1") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("GPU_COMP_TURBO_MAX_AGENTS", "12") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("GPU_COMP_TURBO_ROLE", "driver") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("GPU_COMP_LEANSTRAL_ROLE", "formula-math") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("GPU_COMP_DICE_ROLE", "advisor") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("GPU_COMP_GROK_ROLE", "kernel-fix-scout") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("GPU_COMP_OPENAI_INTERVAL_MIN", "45") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("GPU_COMP_MOA_PROFILE", "gpu-comp-local-moa") };
            // Kernel MoA: Grok as live research scout + on-demand grok_research tool
            // for fixes while Turbo explores. Explicit ANGEL_SOTA_MOA_GROK_RESEARCH=0
            // / ANGEL_GROK_TOOL=0 still win if the operator set them *after* engage
            // (apply_sota_env already set Grok research from scout=true).
            if std::env::var_os("ANGEL_GROK_TOOL").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_GROK_TOOL", "1") };
            }
            // Living Treebeard/HiQ subject: large kernels under handles, popcorn
            // measure, strategy-only roots for Forge. Explicit ANGEL_LANE wins;
            // ANGEL_TASK_TREEBEARD=0 keeps classic ReAct ablation control.
            if std::env::var_os("ANGEL_LANE").is_none()
                && std::env::var_os("ANGEL_TASK_TREEBEARD").is_none()
            {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_LANE", "treebeard") };
            }
            if std::env::var_os("ANGEL_TRAJECTORY_LOG").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_TRAJECTORY_LOG", "1") };
            }
            if std::env::var_os("ANGEL_ROOT_TRAJECTORY").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_ROOT_TRAJECTORY", "1") };
            }
            // Pin living peer log for popcorn-submit-hiq / peer-relative rewards.
            if std::env::var_os("POPCORN_PEER_LOG").is_none()
                && let Some((_geo, _name, Some(path))) = crate::harness::load_living_peer_snapshot()
                && std::path::Path::new(&path).is_file()
            {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("POPCORN_PEER_LOG", path) };
            }
            // Phase-4 RL: coding seats score vs living peer (PopcornPeerReward).
            // Explicit ANGEL_RL_REWARD wins; unset → popcorn_peer under gpu-comp.
            if std::env::var_os("ANGEL_RL_REWARD").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_RL_REWARD", "popcorn_peer") };
            }
            if std::env::var_os("FORGE_COMP_ADVANTAGE").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("FORGE_COMP_ADVANTAGE", "1") };
            }
            // Leaving Grok War should drop its role pins so gpu-comp owns routing.
            clear_grok_war_role_pins();
            clear_tag_team_pins();
        } else if self.id == FormationId::TagTeam {
            // Local-first posture. The roster supplies the concrete seats (two
            // fleet boxes + one Sol); what the env owns here is *when* the
            // metered seat is allowed to fire.
            //
            // The dissent gate is the whole mechanism: proposer disagreement is
            // a free local signal, so agreeing drafts relax the judge away
            // entirely (zero metered calls) and diverging drafts escalate —
            // which is exactly "the locals call Sol for advice when stuck".
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_DISSENT_GATE", "1") };
            // A fleet box is allowed to be slow, thrashing, or terse — and the
            // frontier formations' 2-draft quorum turns any one of those into a
            // dead turn ("kept 0/2 usable drafts"), throwing away the partner's
            // perfectly good draft. Tagging out is the point: if one corner
            // doesn't answer, the other's draft carries the turn.
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_ALLOW_DEGRADED", "1") };
            // A cheap-profile boundary would be about *link* selection; this
            // formation's seats are explicit roster routes, so leave whatever
            // spend boundary the operator set alone.
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_OPENAI_MODEL", "gpt-5.6-sol") };
            // Sol advises rather than drives here, so it does not need the war
            // posture's max effort — but an operator pin still wins.
            if std::env::var_os("ANGEL_OPENAI_REASONING_EFFORT").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_OPENAI_REASONING_EFFORT", "high") };
            }
            // Role pins owned by other formations would fight the roster seats.
            clear_grok_war_role_pins();
            clear_gpu_comp_pins();
        } else if self.id == FormationId::GrokWar {
            // Frontier war posture: leave the cheap LongCat profile so the
            // GLM / DeepSeek / OpenAI / Grok seats can actually fire.
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var("ANGEL_SOTA_MOA_COST_PROFILE") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_PROPOSE_CLUB", "glm") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_EXTRA_PROPOSERS", "deepseek,openai") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_JUDGE_CLUB", "grok") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_AGG_CLUB", "grok") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_VERIFY_CLUB", "none") };
            // Grok is the manager: research tool + high effort synthesis.
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_GROK_TOOL", "1") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_GROK_RESEARCH", "1") };
            if std::env::var_os("ANGEL_GROK_REASONING_EFFORT").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_GROK_REASONING_EFFORT", "high") };
            }
            // OpenAI Sol seat rides at MAX reasoning for the war window.
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_OPENAI_MODEL", "gpt-5.6-sol") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_OPENAI_REASONING_EFFORT", "max") };
            // Kernel competition still wants Treebeard / popcorn scoring when
            // the operator did not pin a different lane.
            if std::env::var_os("ANGEL_LANE").is_none()
                && std::env::var_os("ANGEL_TASK_TREEBEARD").is_none()
            {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_LANE", "treebeard") };
            }
            if std::env::var_os("ANGEL_TRAJECTORY_LOG").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_TRAJECTORY_LOG", "1") };
            }
            if std::env::var_os("ANGEL_RL_REWARD").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_RL_REWARD", "popcorn_peer") };
            }
            if std::env::var_os("POPCORN_PEER_LOG").is_none()
                && let Some((_geo, _name, Some(path))) = crate::harness::load_living_peer_snapshot()
                && std::path::Path::new(&path).is_file()
            {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("POPCORN_PEER_LOG", path) };
            }
            // Drop gpu-comp-only markers so they do not leak into war turns.
            clear_gpu_comp_pins();
            clear_tag_team_pins();
        } else if self.id == FormationId::MathGod {
            // One Sol@ultra head, GLM-5.3 + DeepSeek v4 Pro mix, Grok xhigh
            // weigh-in. Leanstral is a send-to tool, not a conversation seat.
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var("ANGEL_SOTA_MOA_COST_PROFILE") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_PROPOSE_CLUB", "openai") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_EXTRA_PROPOSERS", "glm,deepseek") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_JUDGE_CLUB", "grok") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_AGG_CLUB", "openai") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_VERIFY_CLUB", "none") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_GROK_TOOL", "1") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_SOTA_MOA_GROK_RESEARCH", "1") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_GROK_REASONING_EFFORT", "xhigh") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_OPENAI_MODEL", "gpt-5.6-sol") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_OPENAI_REASONING_EFFORT", "ultra") };
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_GLM_MODEL", "glm-5.3") };
            if std::env::var_os("ANGEL_GLM_REASONING_EFFORT").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_GLM_REASONING_EFFORT", "high") };
            }
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var("ANGEL_DEEPSEEK_MODEL", "deepseek-v4-pro") };
            if std::env::var_os("ANGEL_DEEPSEEK_REASONING_EFFORT").is_none() {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::set_var("ANGEL_DEEPSEEK_REASONING_EFFORT", "high") };
            }
            clear_gpu_comp_pins();
            clear_tag_team_pins();
            MATH_GOD_PINS_ARMED.store(true, Ordering::Relaxed);
        } else {
            clear_gpu_comp_pins();
            clear_grok_war_role_pins();
            clear_tag_team_pins();
            // Drop formation-default popcorn scorer so non-gpu seats fall back
            // to code_health. Explicit operator pins of other reward labels stay.
            if std::env::var("ANGEL_RL_REWARD").as_deref() == Ok("popcorn_peer") {
                // TODO: Audit that the environment access only happens in single-threaded code.
                unsafe { std::env::remove_var("ANGEL_RL_REWARD") };
            }
            // Do not clobber a user-pinned treebeard lane when leaving gpu-comp.
            // POPCORN_PEER_LOG stays if the operator set it for other work.
        }
    }
}

/// Drop Math God role routing and the effort pins it writes unconditionally.
/// No-op when Math God never armed this process, so an operator's `.angel.env`
/// OpenAI/Grok effort is not deleted by engaging Council or Grok War.
fn clear_math_god_pins() {
    if !MATH_GOD_PINS_ARMED.swap(false, Ordering::Relaxed) {
        return;
    }
    clear_grok_war_role_pins();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_OPENAI_REASONING_EFFORT") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_GROK_REASONING_EFFORT") };
    crate::club::resync_reasoning_effort_env_from_env();
}

fn clear_gpu_comp_pins() {
    for key in [
        "ANGEL_GPU_COMP_LOCAL_MOA",
        "GPU_COMP_TURBO_MAX_AGENTS",
        "GPU_COMP_TURBO_ROLE",
        "GPU_COMP_LEANSTRAL_ROLE",
        "GPU_COMP_DICE_ROLE",
        "GPU_COMP_GROK_ROLE",
        "GPU_COMP_OPENAI_INTERVAL_MIN",
        "GPU_COMP_MOA_PROFILE",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
}

/// Tag Team owns two knobs: the dissent gate that keeps its metered advice seat
/// conditional, and the degraded-quorum allowance that lets one live corner
/// carry a turn. Leaving the formation drops both, so another formation (or the
/// operator's `.angel.env` on the next launch) decides those policies — a
/// frontier formation must keep its strict quorum. The Sol model/effort pins are
/// deliberately left alone: an operator who set them outside engagement keeps them.
fn clear_tag_team_pins() {
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_MOA_DISSENT_GATE") };
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_SOTA_MOA_ALLOW_DEGRADED") };
}

fn clear_grok_war_role_pins() {
    // Only clear pins this formation owns. Leave operator-pinned OpenAI model /
    // Grok effort alone when they were set outside engagement.
    for key in [
        "ANGEL_SOTA_MOA_PROPOSE_CLUB",
        "ANGEL_SOTA_MOA_EXTRA_PROPOSERS",
        "ANGEL_SOTA_MOA_JUDGE_CLUB",
        "ANGEL_SOTA_MOA_AGG_CLUB",
        "ANGEL_SOTA_MOA_VERIFY_CLUB",
    ] {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(key) };
    }
}

fn recommended_model(
    formation: FormationId,
    slot: &FormationSlot,
    models: &[MoaModelChoice],
) -> Option<MoaModelRef> {
    let available = |selector: &str| {
        models
            .iter()
            .find(|choice| choice.available && choice.route.matches(selector))
            .map(|choice| choice.route.clone())
    };

    // Tag Team's local seats resolve against the fleet only. Falling through to
    // the intelligence order the way every other formation does would quietly
    // swap a metered frontier model into a mode whose entire premise is that
    // the two fleet boxes carry the turn — better to leave the seat visibly
    // unassigned (the deck blocks engagement and names it) than to bill for it.
    if formation == FormationId::TagTeam {
        // Unmetered is the structural half of the guard: a selector can drift
        // (a rig serving a model whose id happens to contain "turbo"), the
        // billing flag cannot.
        let pick = |chain: &[&str], exclude: Option<&MoaModelRef>| {
            chain.iter().find_map(|selector| {
                models
                    .iter()
                    .find(|choice| {
                        choice.available
                            && !choice.route.metered
                            && choice.route.matches(selector)
                            && exclude.is_none_or(|taken| taken != &choice.route)
                    })
                    .map(|choice| choice.route.clone())
            })
        };
        // Leanstral leads. It is the fleet's mathematics/formula engine, so the
        // derivation and optimization work belongs on it rather than in a
        // reviewing seat — and it is the larger model (119B-A6B vs 35B-A3B) and
        // measured ~57x faster per token than turbo, which also decides who can
        // afford to hold the synthesis seat.
        // Turbo trails both chains so that if Leanstral is down the lead seat
        // degrades to whatever fleet box is live, rather than pinning the
        // formation to a corner it cannot fill.
        const LEAD: &[&str] = &["leanstral", "spark", "gemma", "turbo"];
        const PARTNER: &[&str] = &["turbo", "gemma", "spark"];
        match slot.role {
            FormationRole::Propose if slot.ordinal == 0 => return pick(LEAD, None),
            // The partner seat must be a *different* route than the lead. Left
            // to fall through it would happily nominate the lead's own box a
            // second time, which reads as a tag team in the deck while actually
            // being one model arguing with itself. An empty seat that names the
            // missing partner is the honest state.
            FormationRole::Propose => return pick(PARTNER, pick(LEAD, None).as_ref()),
            FormationRole::Aggregate => return pick(LEAD, None),
            // The advice seat is only worth its call if it is an outside
            // opinion: Sol first, then any other configured frontier link.
            // Requiring a metered route keeps a local box — the very model
            // being reviewed — from quietly landing here when Sol is absent.
            FormationRole::Judge => {
                let advisor = |selector: &str| {
                    models
                        .iter()
                        .find(|choice| {
                            choice.available
                                && choice.route.metered
                                && choice.route.matches(selector)
                        })
                        .map(|choice| choice.route.clone())
                };
                return advisor("openai").or_else(|| {
                    crate::club::SOTA_MOA_INTELLIGENCE_ORDER
                        .iter()
                        .find_map(|selector| advisor(selector))
                });
            }
            _ => {}
        }
    }

    if formation == FormationId::MathGod {
        // Sol is the only Sol head. GLM-5.3 and DeepSeek v4 Pro are mix seats.
        // Do not fall through to the intelligence order or a later P-seat
        // becomes a second copy of some other frontier model.
        return match slot.role {
            FormationRole::Propose if slot.ordinal == 0 => available("openai"),
            FormationRole::Propose if slot.ordinal == 1 => available("glm"),
            FormationRole::Propose => available("deepseek-v4-pro"),
            FormationRole::Aggregate => available("openai"),
            FormationRole::Judge | FormationRole::Scout => available("grok"),
            _ => None,
        };
    }

    if formation == FormationId::AutoMoa {
        // The standing mixture: no seat pinning. Every seat resolves from the
        // intelligence order over the live model list — the same ordering the
        // bag's implicit `sota_moa_club_with_breadth` wrapper has always used —
        // so the deck shows what the auto path would actually run: smartest
        // available first, nth-distinct model per seat. A single configured
        // provider legitimately fills every seat (the swarm pipeline tolerates
        // a homogeneous roster; the bag's ANGEL_SOTA_MOA_ALLOW_SINGLE path
        // already does this today).
        let seat = |ordinal: usize| -> Option<MoaModelRef> {
            crate::club::SOTA_MOA_INTELLIGENCE_ORDER
                .iter()
                .filter_map(|selector| available(selector))
                .nth(ordinal)
                .or_else(|| {
                    models
                        .iter()
                        .find(|choice| choice.available)
                        .map(|choice| choice.route.clone())
                })
        };
        return match slot.role {
            FormationRole::Propose => seat(slot.ordinal),
            FormationRole::Judge => seat(1),
            FormationRole::Verify => seat(2),
            FormationRole::Aggregate => seat(0),
            _ => None,
        };
    }

    let specialist = match (formation, slot.role) {
        (FormationId::GpuComp, FormationRole::Propose) => Some("turbo"),
        (FormationId::GpuComp, FormationRole::Judge) => Some("leanstral"),
        (FormationId::GpuComp, FormationRole::Verify) => Some("dice"),
        (FormationId::GpuComp, FormationRole::Aggregate) => Some("openai"),
        // Kernel MoA scout seat: Grok research / fix critique (live web + SOTA).
        (FormationId::GpuComp, FormationRole::Scout) => Some("grok"),
        // Grok War: fixed trio under Grok command. Ordinals are stable seats —
        // P1 GLM, P2 DeepSeek, P3 OpenAI Sol; Grok owns scout/judge/aggregate.
        (FormationId::GrokWar, FormationRole::Propose) => match slot.ordinal {
            0 => Some("glm"),
            1 => Some("deepseek"),
            _ => Some("openai"),
        },
        (FormationId::GrokWar, FormationRole::Judge) => Some("grok"),
        (FormationId::GrokWar, FormationRole::Aggregate) => Some("grok"),
        (FormationId::GrokWar, FormationRole::Scout) => Some("grok"),
        (_, FormationRole::Scout) => Some("grok"),
        _ => None,
    };
    if let Some(model) = specialist.and_then(available) {
        return Some(model);
    }
    for preferred in crate::club::SOTA_MOA_INTELLIGENCE_ORDER {
        if let Some(model) = available(preferred) {
            return Some(model);
        }
    }
    models
        .iter()
        .find(|choice| choice.available)
        .map(|choice| choice.route.clone())
}

fn set_flag(name: &str, on: bool) {
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(name, if on { "1" } else { "0" }) };
}

#[derive(Clone, Debug)]
pub(crate) struct MoaDeckState {
    selected: usize,
    previous: Option<usize>,
    changed_at: Instant,
    focus: MoaDeckFocus,
    selected_slot: usize,
    selected_model: usize,
    /// THINK picker cursor plus the ladder captured when it opened (the focused
    /// seat's route-declared reasoning levels; the trailing virtual row is the
    /// env-default clear action).
    selected_effort: usize,
    efforts: Vec<String>,
    models: Vec<MoaModelChoice>,
    rosters: HashMap<FormationId, FormationRoster>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MoaDeckFocus {
    Formations,
    Slots,
    Models,
    /// The THINK picker: staging one role's reasoning effort over the focused
    /// seat's route-declared ladder.
    Efforts,
}

impl MoaDeckState {
    pub(crate) fn new(models: Vec<MoaModelChoice>) -> Self {
        let selected = BUILT_INS
            .iter()
            .position(|formation| formation.id == FormationId::SoloStrike)
            .unwrap_or(0);
        let rosters = BUILT_INS
            .iter()
            .map(|formation| (formation.id, FormationRoster::new(formation.id, &models)))
            .collect();
        Self {
            selected,
            previous: None,
            changed_at: Instant::now(),
            focus: MoaDeckFocus::Formations,
            selected_slot: 0,
            selected_effort: 0,
            efforts: Vec::new(),
            selected_model: models
                .iter()
                .position(|choice| choice.available)
                .unwrap_or(0),
            models,
            rosters,
        }
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected.min(BUILT_INS.len().saturating_sub(1))
    }

    pub(crate) fn selected(&self) -> &'static Formation {
        &BUILT_INS[self.selected_index()]
    }

    pub(crate) fn focus(&self) -> MoaDeckFocus {
        self.focus
    }

    pub(crate) fn selected_slot_index(&self) -> usize {
        let len = self.selected().slots().len();
        self.selected_slot.min(len.saturating_sub(1))
    }

    pub(crate) fn selected_model_index(&self) -> usize {
        self.selected_model.min(self.models.len().saturating_sub(1))
    }

    pub(crate) fn models(&self) -> &[MoaModelChoice] {
        &self.models
    }

    pub(crate) fn selected_roster(&self) -> &FormationRoster {
        self.rosters
            .get(&self.selected().id)
            .expect("every built-in formation owns a roster")
    }

    pub(crate) fn unavailable_slot_labels(&self) -> Vec<String> {
        self.selected()
            .slots()
            .into_iter()
            .zip(self.selected_roster().assignments())
            .filter_map(|(slot, assignment)| {
                let route = assignment.as_ref()?;
                let available = self
                    .models
                    .iter()
                    .any(|choice| choice.available && choice.route == *route);
                (!available).then(|| slot.label())
            })
            .collect()
    }

    pub(crate) fn selected_roster_online(&self) -> bool {
        self.unavailable_slot_labels().is_empty()
    }

    fn selected_roster_mut(&mut self) -> &mut FormationRoster {
        let id = self.selected().id;
        self.rosters
            .get_mut(&id)
            .expect("every built-in formation owns a roster")
    }

    pub(crate) fn engagement(&self) -> Option<MoaEngagement> {
        let roster = self.selected_roster().clone();
        MoaEngagement::new(self.selected().id, roster)
    }

    pub(crate) fn load_engagement(&mut self, engagement: &MoaEngagement) {
        self.rosters
            .insert(engagement.formation, engagement.roster.clone());
        self.select(engagement.formation);
    }

    pub(crate) fn move_selection(&mut self, delta: isize) {
        self.move_selection_at(delta, Instant::now());
    }

    pub(crate) fn select(&mut self, id: FormationId) {
        if let Some(next) = BUILT_INS.iter().position(|formation| formation.id == id) {
            let delta = next as isize - self.selected_index() as isize;
            self.move_selection(delta);
        }
    }

    fn move_selection_at(&mut self, delta: isize, now: Instant) {
        let len = BUILT_INS.len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        let next = (self.selected as isize + delta).rem_euclid(len as isize) as usize;
        if next != self.selected {
            self.previous = Some(self.selected_index());
            self.selected = next;
            self.changed_at = now;
            self.selected_slot = self
                .selected_slot
                .min(self.selected().slots().len().saturating_sub(1));
        }
    }

    pub(crate) fn focus_slots(&mut self) -> bool {
        if self.selected().slots().is_empty() {
            return false;
        }
        self.focus = MoaDeckFocus::Slots;
        true
    }

    pub(crate) fn focus_formations(&mut self) {
        self.focus = MoaDeckFocus::Formations;
    }

    pub(crate) fn move_slot(&mut self, delta: isize) {
        let len = self.selected().slots().len();
        if len == 0 {
            return;
        }
        self.selected_slot =
            (self.selected_slot as isize + delta).rem_euclid(len as isize) as usize;
        self.focus = MoaDeckFocus::Slots;
    }

    pub(crate) fn select_slot(&mut self, index: usize) -> bool {
        if index >= self.selected().slots().len() {
            return false;
        }
        self.selected_slot = index;
        self.focus = MoaDeckFocus::Slots;
        true
    }

    pub(crate) fn open_model_picker(&mut self) -> bool {
        if self.models.is_empty() || self.selected().slots().is_empty() {
            return false;
        }
        let selected_route = self
            .selected_roster()
            .assignment(self.selected_slot_index());
        self.selected_model = selected_route
            .and_then(|route| self.models.iter().position(|choice| &choice.route == route))
            .or_else(|| self.models.iter().position(|choice| choice.available))
            .unwrap_or(0);
        self.focus = MoaDeckFocus::Models;
        true
    }

    pub(crate) fn close_model_picker(&mut self) {
        if self.focus == MoaDeckFocus::Models {
            self.focus = MoaDeckFocus::Slots;
        }
    }

    pub(crate) fn move_model(&mut self, delta: isize) {
        let available = self
            .models
            .iter()
            .enumerate()
            .filter_map(|(index, choice)| choice.available.then_some(index))
            .collect::<Vec<_>>();
        if available.is_empty() {
            return;
        }
        let position = available
            .iter()
            .position(|index| *index == self.selected_model)
            .unwrap_or(0);
        let next = (position as isize + delta).rem_euclid(available.len() as isize) as usize;
        self.selected_model = available[next];
    }

    pub(crate) fn assign_model(&mut self, model_index: usize) -> bool {
        let Some(choice) = self
            .models
            .get(model_index)
            .filter(|choice| choice.available)
        else {
            return false;
        };
        let model = choice.route.clone();
        let slot = self.selected_slot_index();
        let assigned = self.selected_roster_mut().assign(slot, model);
        if assigned {
            self.selected_model = model_index;
            self.focus = MoaDeckFocus::Slots;
        }
        assigned
    }

    pub(crate) fn assign_selected_model(&mut self) -> bool {
        self.assign_model(self.selected_model_index())
    }

    fn selected_slot_role(&self) -> Option<FormationRole> {
        self.selected()
            .slots()
            .get(self.selected_slot_index())
            .map(|slot| slot.role)
    }

    /// Open the THINK picker for the focused seat over `levels` — the assigned
    /// route's own declared reasoning ladder. An empty ladder still opens (the
    /// picker shows the n/a line and offers only the env-default row); a Scout
    /// seat refuses, because the swarm carries no scout effort policy.
    pub(crate) fn open_effort_picker(&mut self, levels: Vec<String>) -> bool {
        let Some(role) = self.selected_slot_role() else {
            return false;
        };
        if role == FormationRole::Scout {
            return false;
        }
        let staged_position = self.selected_roster().role_effort(role).and_then(|staged| {
            levels
                .iter()
                .position(|level| level.eq_ignore_ascii_case(staged))
        });
        // Nothing staged (or a value the ladder no longer lists) rests the
        // cursor on the env-default row.
        self.selected_effort = staged_position.unwrap_or(levels.len());
        self.efforts = levels;
        self.focus = MoaDeckFocus::Efforts;
        true
    }

    pub(crate) fn close_effort_picker(&mut self) {
        if self.focus == MoaDeckFocus::Efforts {
            self.focus = MoaDeckFocus::Slots;
        }
    }

    pub(crate) fn effort_options(&self) -> &[String] {
        &self.efforts
    }

    /// Picker cursor over `efforts` plus the trailing env-default row.
    pub(crate) fn selected_effort_index(&self) -> usize {
        self.selected_effort.min(self.efforts.len())
    }

    pub(crate) fn move_effort(&mut self, delta: isize) {
        let len = self.efforts.len() + 1; // ladder rows + the env-default row
        self.selected_effort =
            (self.selected_effort_index() as isize + delta).rem_euclid(len as isize) as usize;
    }

    /// Commit picker row `option_index` onto the focused seat's role: a ladder
    /// row stages that level, the trailing row clears back to the env seat
    /// policy. Returns the role and the canonically staged value (`None` =
    /// cleared) so the caller can voice the result.
    pub(crate) fn stage_effort(
        &mut self,
        option_index: usize,
    ) -> Option<(FormationRole, Option<String>)> {
        let role = self.selected_slot_role()?;
        if role == FormationRole::Scout || option_index > self.efforts.len() {
            return None;
        }
        let effort = self.efforts.get(option_index).cloned();
        self.selected_roster_mut()
            .set_role_effort(role, effort.as_deref());
        self.selected_effort = option_index;
        self.focus = MoaDeckFocus::Slots;
        Some((
            role,
            self.selected_roster().role_effort(role).map(str::to_string),
        ))
    }

    #[cfg(test)]
    pub(crate) fn stage_selected_effort(&mut self) -> Option<(FormationRole, Option<String>)> {
        self.stage_effort(self.selected_effort_index())
    }

    pub(crate) fn transition(&self) -> Option<(FormationId, f32)> {
        self.transition_at(Instant::now())
    }

    fn transition_at(&self, now: Instant) -> Option<(FormationId, f32)> {
        let previous = self.previous?;
        let elapsed = now
            .checked_duration_since(self.changed_at)
            .unwrap_or_default();
        let progress = elapsed.as_secs_f32() / SELECTION_DISSOLVE.as_secs_f32();
        (progress < 1.0).then(|| (BUILT_INS[previous].id, progress.clamp(0.0, 1.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_change_starts_250ms_dissolve_and_increments_epoch() {
        let now = Instant::now();
        let mut deck = MoaDeckState::new(Vec::new());
        deck.move_selection_at(1, now);
        assert_eq!(deck.selected().id, FormationId::Recon);
        assert_eq!(
            deck.transition_at(now),
            Some((FormationId::SoloStrike, 0.0))
        );
        let halfway = now + SELECTION_DISSOLVE / 2;
        let (_, progress) = deck.transition_at(halfway).unwrap();
        assert!((progress - 0.5).abs() < 0.01);
        assert!(deck.transition_at(now + SELECTION_DISSOLVE).is_none());
    }

    #[test]
    fn selection_wrap_preserves_formation_data_behavior() {
        let mut deck = MoaDeckState::new(Vec::new());
        let original = *deck.selected();
        deck.move_selection_at(-1, Instant::now());
        // Wrapping backwards from the first card lands on the last one in the
        // built-in deck order (Auto MoA was appended last).
        assert_eq!(deck.selected().id, FormationId::AutoMoa);
        assert_eq!(original.width, formation(FormationId::SoloStrike).width);
        assert_eq!(
            original.stage_summary(),
            formation(FormationId::SoloStrike).stage_summary()
        );
    }

    #[test]
    fn grok_war_rosters_trio_under_grok_command() {
        let model = |agent: &str, model: &str| MoaModelChoice {
            route: MoaModelRef {
                agent_index: 0,
                slot_index: 0,
                agent: agent.into(),
                driver: model.into(),
                model: model.into(),
                route_id: crate::backplane::RouteId::chat(agent, model, None),
                expected_revision: crate::backplane::ModelRevision::chat(model),
                metered: true,
            },
            available: true,
        };
        let models = vec![
            model("glm", "glm-5.2"),
            model("deepseek", "deepseek-v4-pro"),
            model("openai", "gpt-5.6-sol"),
            model("grok", "grok-4.5"),
        ];
        let roster = FormationRoster::new(FormationId::GrokWar, &models);
        let slots = formation(FormationId::GrokWar).slots();
        assert_eq!(slots.len(), 6); // scout + 3 propose + judge + aggregate
        let labels: Vec<_> = roster
            .assignments()
            .iter()
            .map(|a| a.as_ref().map(|r| r.model.as_str()).unwrap_or("?"))
            .collect();
        assert_eq!(
            labels,
            vec![
                "grok-4.5",        // scout
                "glm-5.2",         // P1
                "deepseek-v4-pro", // P2
                "gpt-5.6-sol",     // P3
                "grok-4.5",        // judge
                "grok-4.5",        // aggregate / manager
            ]
        );
        assert!(roster.is_ready());
    }

    #[test]
    fn math_god_rosters_sol_glm_deepseek_with_grok_weigh_in() {
        let model = |agent: &str, model: &str, metered: bool| MoaModelChoice {
            route: MoaModelRef {
                agent_index: 0,
                slot_index: 0,
                agent: agent.into(),
                driver: model.into(),
                model: model.into(),
                route_id: crate::backplane::RouteId::chat(agent, model, None),
                expected_revision: crate::backplane::ModelRevision::chat(model),
                metered,
            },
            available: true,
        };
        let models = vec![
            model("glm", "glm-5.3", true),
            model("openai", "gpt-5.6-sol", true),
            model("grok", "grok-4.6", true),
            model("spark", "leanstral-24b", false),
            model("deepseek", "deepseek-v4-pro", true),
        ];
        let roster = FormationRoster::new(FormationId::MathGod, &models);
        let slots = formation(FormationId::MathGod).slots();
        assert_eq!(slots.len(), 6); // scout + 3 propose + judge + aggregate
        let labels: Vec<_> = roster
            .assignments()
            .iter()
            .map(|a| a.as_ref().map(|r| r.model.as_str()).unwrap_or("?"))
            .collect();
        assert_eq!(
            labels,
            vec![
                "grok-4.6",        // scout weigh-in
                "gpt-5.6-sol",     // P1 — Sol head
                "glm-5.3",         // P2 — GLM mix
                "deepseek-v4-pro", // P3 — DeepSeek mix
                "grok-4.6",        // judge weigh-in
                "gpt-5.6-sol",     // aggregate
            ]
        );
        assert!(!labels.iter().any(|label| label.contains("leanstral")));
        assert!(!labels.iter().any(|label| label.contains("flash")));
        assert_eq!(
            labels.iter().filter(|label| label.contains("sol")).count(),
            2
        );
        assert_eq!(roster.role_effort(FormationRole::Propose), Some("ultra"));
        assert_eq!(roster.role_effort(FormationRole::Aggregate), Some("ultra"));
        assert_eq!(roster.role_effort(FormationRole::Judge), Some("xhigh"));
        assert!(roster.is_ready());
    }

    #[test]
    fn math_god_is_ready_without_leanstral_on_the_roster() {
        let model = |agent: &str, model: &str, metered: bool| MoaModelChoice {
            route: MoaModelRef {
                agent_index: 0,
                slot_index: 0,
                agent: agent.into(),
                driver: model.into(),
                model: model.into(),
                route_id: crate::backplane::RouteId::chat(agent, model, None),
                expected_revision: crate::backplane::ModelRevision::chat(model),
                metered,
            },
            available: true,
        };
        let models = vec![
            model("glm", "glm-5.3", true),
            model("openai", "gpt-5.6-sol", true),
            model("grok", "grok-4.6", true),
            model("deepseek", "deepseek-v4-pro", true),
        ];
        let roster = FormationRoster::new(FormationId::MathGod, &models);
        let assigned: Vec<_> = roster
            .assignments()
            .iter()
            .map(|a| a.as_ref().map(|r| r.model.clone()))
            .collect();
        assert_eq!(
            assigned,
            vec![
                Some("grok-4.6".into()),
                Some("gpt-5.6-sol".into()),
                Some("glm-5.3".into()),
                Some("deepseek-v4-pro".into()),
                Some("grok-4.6".into()),
                Some("gpt-5.6-sol".into()),
            ]
        );
        assert!(roster.is_ready());
    }

    #[test]
    fn math_god_does_not_fill_deepseek_seat_from_intelligence_order() {
        let model = |agent: &str, model: &str, metered: bool| MoaModelChoice {
            route: MoaModelRef {
                agent_index: 0,
                slot_index: 0,
                agent: agent.into(),
                driver: model.into(),
                model: model.into(),
                route_id: crate::backplane::RouteId::chat(agent, model, None),
                expected_revision: crate::backplane::ModelRevision::chat(model),
                metered,
            },
            available: true,
        };
        let models = vec![
            model("glm", "glm-5.3", true),
            model("openai", "gpt-5.6-sol", true),
            model("grok", "grok-4.6", true),
            model("kimi", "kimi-k3", true),
            model("deepseek-flash", "deepseek-v4-flash", true),
        ];
        let roster = FormationRoster::new(FormationId::MathGod, &models);
        let assigned: Vec<_> = roster
            .assignments()
            .iter()
            .map(|a| a.as_ref().map(|r| r.model.clone()))
            .collect();
        assert_eq!(
            assigned,
            vec![
                Some("grok-4.6".into()),
                Some("gpt-5.6-sol".into()),
                Some("glm-5.3".into()),
                None,
                Some("grok-4.6".into()),
                Some("gpt-5.6-sol".into()),
            ]
        );
        assert!(!roster.is_ready());
    }

    #[test]
    fn tag_team_rosters_two_fleet_boxes_with_sol_as_the_advice_seat() {
        let model = |agent: &str, model: &str, metered: bool| MoaModelChoice {
            route: MoaModelRef {
                agent_index: 0,
                slot_index: 0,
                agent: agent.into(),
                driver: model.into(),
                model: model.into(),
                route_id: crate::backplane::RouteId::chat(agent, model, None),
                expected_revision: crate::backplane::ModelRevision::chat(model),
                metered,
            },
            available: true,
        };
        let models = vec![
            model("turbo", "qwen3-30b-a3b", false),
            model("spark", "leanstral-24b", false),
            model("openai", "gpt-5.6-sol", true),
            model("glm", "glm-5.2", true),
        ];
        let roster = FormationRoster::new(FormationId::TagTeam, &models);
        let slots = formation(FormationId::TagTeam).slots();
        assert_eq!(slots.len(), 4); // 2 propose + judge + aggregate
        let labels: Vec<_> = roster
            .assignments()
            .iter()
            .map(|a| a.as_ref().map(|r| r.model.as_str()).unwrap_or("?"))
            .collect();
        assert_eq!(
            labels,
            vec![
                "leanstral-24b", // P1 — the math engine leads
                "qwen3-30b-a3b", // P2 — turbo works the second corner
                "gpt-5.6-sol",   // J1 — the advice seat
                "leanstral-24b", // A1 — the math engine also synthesizes
            ]
        );
        assert!(roster.is_ready());
    }

    /// Refinement layers must follow the formation's own width: Full Muster
    /// (layers=2, width=3) pins `ANGEL_SWARM_REFINE_WIDTH` to 3 — each refine
    /// layer carries the full history plus every draft, so the generic 6-seat
    /// default doubles the formation's per-layer input spend. Single-layer
    /// formations never refine; the disengage path (SoloStrike) clears it.
    #[test]
    fn apply_sota_env_pins_refine_width_to_the_formation_width() {
        let _lock = crate::tests::env_lock();
        const TOUCHED: &[&str] = &[
            "ANGEL_SOTA_MOA_MODE",
            "ANGEL_SOTA_MOA_ALWAYS",
            "ANGEL_SOTA_MOA_MAX",
            "ANGEL_SOTA_MOA_WIDTH",
            "ANGEL_SOTA_MOA_MAX_WIDTH",
            "ANGEL_SOTA_MOA_LAYERS",
            "ANGEL_SOTA_MOA_SAMPLES",
            "ANGEL_SOTA_MOA_JUDGE_PANEL",
            "ANGEL_SOTA_MOA_VERIFY",
            "ANGEL_SOTA_MOA_JUDGE",
            "ANGEL_SOTA_MOA_VERIFY_GUARD",
            "ANGEL_SOTA_MOA_REFLECT",
            "ANGEL_SOTA_MOA_GROK_RESEARCH",
            "ANGEL_SOTA_MOA_RESEARCH",
            "ANGEL_SWARM_REFINE_WIDTH",
            "ANGEL_SOTA_MOA_FORMATION",
        ];
        let _restore: Vec<_> = TOUCHED
            .iter()
            .map(|key| crate::tests::TestEnvGuard::unset(key))
            .collect();
        let full_muster = formation(FormationId::AllIn);
        assert!(full_muster.layers > 1, "Full Muster refines");
        full_muster.apply_sota_env();
        assert_eq!(
            std::env::var("ANGEL_SWARM_REFINE_WIDTH"),
            Ok(full_muster.width.to_string())
        );
        assert_eq!(
            std::env::var("ANGEL_SOTA_MOA_FORMATION").as_deref(),
            Ok("Full Muster"),
            "the engaged formation names itself for the ledgers"
        );
        // Disengage: the resting formation owns no refine width and no name.
        formation(FormationId::SoloStrike).apply_sota_env();
        assert!(std::env::var_os("ANGEL_SWARM_REFINE_WIDTH").is_none());
        assert!(std::env::var_os("ANGEL_SOTA_MOA_FORMATION").is_none());
    }

    /// The conditional-advice posture lives in one env knob, and it must not
    /// outlive the engagement that set it.
    #[test]
    fn tag_team_pins_the_dissent_gate_and_leaving_clears_it() {
        let _lock = crate::tests::env_lock();
        // `apply_sota_env` writes the whole MoA knob set as a side effect, and
        // tests that never take this lock still read those knobs. Snapshot every
        // key either formation can touch so this test restores the process
        // environment exactly as it found it.
        const TOUCHED: &[&str] = &[
            "ANGEL_SOTA_MOA_MODE",
            "ANGEL_SOTA_MOA_ALWAYS",
            "ANGEL_SOTA_MOA_MAX",
            "ANGEL_SOTA_MOA_WIDTH",
            "ANGEL_SOTA_MOA_MAX_WIDTH",
            "ANGEL_SOTA_MOA_LAYERS",
            "ANGEL_SOTA_MOA_SAMPLES",
            "ANGEL_SOTA_MOA_JUDGE_PANEL",
            "ANGEL_SOTA_MOA_VERIFY",
            "ANGEL_SOTA_MOA_JUDGE",
            "ANGEL_SOTA_MOA_VERIFY_GUARD",
            "ANGEL_SOTA_MOA_REFLECT",
            "ANGEL_SOTA_MOA_GROK_RESEARCH",
            "ANGEL_SOTA_MOA_RESEARCH",
            "ANGEL_SWARM_REFINE_WIDTH",
            "ANGEL_SOTA_MOA_FORMATION",
            "ANGEL_SOTA_MOA_DISSENT_GATE",
            "ANGEL_SOTA_MOA_ALLOW_DEGRADED",
            "ANGEL_SOTA_MOA_PROPOSE_CLUB",
            "ANGEL_SOTA_MOA_EXTRA_PROPOSERS",
            "ANGEL_SOTA_MOA_JUDGE_CLUB",
            "ANGEL_SOTA_MOA_AGG_CLUB",
            "ANGEL_SOTA_MOA_VERIFY_CLUB",
            "ANGEL_OPENAI_MODEL",
            "ANGEL_OPENAI_REASONING_EFFORT",
            "ANGEL_RL_REWARD",
            "ANGEL_GPU_COMP_LOCAL_MOA",
            "GPU_COMP_TURBO_MAX_AGENTS",
            "GPU_COMP_TURBO_ROLE",
            "GPU_COMP_LEANSTRAL_ROLE",
            "GPU_COMP_DICE_ROLE",
            "GPU_COMP_GROK_ROLE",
            "GPU_COMP_OPENAI_INTERVAL_MIN",
            "GPU_COMP_MOA_PROFILE",
        ];
        let _restore: Vec<_> = TOUCHED
            .iter()
            .map(|key| crate::tests::TestEnvGuard::unset(key))
            .collect();

        formation(FormationId::TagTeam).apply_sota_env();
        assert_eq!(
            std::env::var("ANGEL_SOTA_MOA_DISSENT_GATE").as_deref(),
            Ok("1")
        );
        // One live corner must be able to carry a turn; the strict 2-draft
        // quorum is what turned a slow box into a dead turn.
        assert_eq!(
            std::env::var("ANGEL_SOTA_MOA_ALLOW_DEGRADED").as_deref(),
            Ok("1")
        );
        assert_eq!(
            std::env::var("ANGEL_OPENAI_MODEL").as_deref(),
            Ok("gpt-5.6-sol")
        );
        // A frontier formation must get its strict quorum back.
        formation(FormationId::Council).apply_sota_env();
        assert!(std::env::var_os("ANGEL_SOTA_MOA_DISSENT_GATE").is_none());
        assert!(std::env::var_os("ANGEL_SOTA_MOA_ALLOW_DEGRADED").is_none());
    }

    /// Draw-path effort env is seed-once. Engaging Math God must still move
    /// the observed OpenAI / GLM effort after the cache has been seeded, and
    /// leaving must drop the unconditional OpenAI pin.
    #[test]
    fn apply_sota_env_math_god_pins_live_after_effort_env_seed() {
        use crate::club::{Club, HttpClub};
        let _lock = crate::tests::env_lock();
        const TOUCHED: &[&str] = &[
            "ANGEL_SOTA_MOA_MODE",
            "ANGEL_SOTA_MOA_ALWAYS",
            "ANGEL_SOTA_MOA_MAX",
            "ANGEL_SOTA_MOA_WIDTH",
            "ANGEL_SOTA_MOA_MAX_WIDTH",
            "ANGEL_SOTA_MOA_LAYERS",
            "ANGEL_SOTA_MOA_SAMPLES",
            "ANGEL_SOTA_MOA_JUDGE_PANEL",
            "ANGEL_SOTA_MOA_VERIFY",
            "ANGEL_SOTA_MOA_JUDGE",
            "ANGEL_SOTA_MOA_VERIFY_GUARD",
            "ANGEL_SOTA_MOA_REFLECT",
            "ANGEL_SOTA_MOA_GROK_RESEARCH",
            "ANGEL_SOTA_MOA_RESEARCH",
            "ANGEL_SWARM_REFINE_WIDTH",
            "ANGEL_SOTA_MOA_FORMATION",
            "ANGEL_SOTA_MOA_DISSENT_GATE",
            "ANGEL_SOTA_MOA_ALLOW_DEGRADED",
            "ANGEL_SOTA_MOA_PROPOSE_CLUB",
            "ANGEL_SOTA_MOA_EXTRA_PROPOSERS",
            "ANGEL_SOTA_MOA_JUDGE_CLUB",
            "ANGEL_SOTA_MOA_AGG_CLUB",
            "ANGEL_SOTA_MOA_VERIFY_CLUB",
            "ANGEL_SOTA_MOA_COST_PROFILE",
            "ANGEL_GROK_TOOL",
            "ANGEL_GROK_REASONING_EFFORT",
            "ANGEL_OPENAI_MODEL",
            "ANGEL_OPENAI_REASONING_EFFORT",
            "ANGEL_GLM_MODEL",
            "ANGEL_GLM_REASONING_EFFORT",
            "ANGEL_DEEPSEEK_MODEL",
            "ANGEL_DEEPSEEK_REASONING_EFFORT",
            "ANGEL_REASONING_EFFORT",
            "ANGEL_RL_REWARD",
            "ANGEL_GPU_COMP_LOCAL_MOA",
            "GPU_COMP_TURBO_MAX_AGENTS",
            "GPU_COMP_TURBO_ROLE",
            "GPU_COMP_LEANSTRAL_ROLE",
            "GPU_COMP_DICE_ROLE",
            "GPU_COMP_GROK_ROLE",
            "GPU_COMP_OPENAI_INTERVAL_MIN",
            "GPU_COMP_MOA_PROFILE",
        ];
        let _restore: Vec<_> = TOUCHED
            .iter()
            .map(|key| crate::tests::TestEnvGuard::unset(key))
            .collect();
        crate::club::resync_reasoning_effort_env_from_env();

        let glm = HttpClub::new(
            "glm",
            "https://api.z.ai/api/coding/paas/v4",
            "glm-5.3",
            None,
        );
        let openai = HttpClub::new("openai", "https://api.openai.com/v1", "gpt-5.6-sol", None);

        assert!(openai.reasoning_effort().is_none());
        assert_eq!(glm.reasoning_effort().as_deref(), Some("low"));
        assert_eq!(
            crate::club::reasoning_env_var("ANGEL_OPENAI_REASONING_EFFORT"),
            None
        );
        assert_eq!(
            crate::club::reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"),
            None
        );

        formation(FormationId::MathGod).apply_sota_env();
        assert_eq!(
            crate::club::reasoning_env_var("ANGEL_OPENAI_REASONING_EFFORT").as_deref(),
            Some("ultra")
        );
        assert_eq!(
            crate::club::reasoning_env_var("ANGEL_GROK_REASONING_EFFORT").as_deref(),
            Some("xhigh")
        );
        assert_eq!(
            openai.reasoning_effort().as_deref(),
            Some("ultra"),
            "Math God OpenAI pin must land after the draw-path cache has seeded"
        );
        assert_eq!(
            glm.reasoning_effort().as_deref(),
            Some("high"),
            "Math God GLM pin must land after the draw-path cache has seeded"
        );

        formation(FormationId::SoloStrike).apply_sota_env();
        assert_eq!(
            crate::club::reasoning_env_var("ANGEL_OPENAI_REASONING_EFFORT"),
            None
        );
        assert_eq!(
            crate::club::reasoning_env_var("ANGEL_GROK_REASONING_EFFORT"),
            None
        );
        assert!(openai.reasoning_effort().is_none());
        crate::club::resync_reasoning_effort_env_from_env();
    }

    /// A tag team needs two corners. With only one fleet box online the partner
    /// seat must stay empty and say so, rather than nominating the lead's own
    /// box again — that renders as a pair in the deck while actually being one
    /// model arguing with itself.
    #[test]
    fn tag_team_partner_seat_never_doubles_up_on_the_lead_box() {
        let turbo = MoaModelChoice {
            route: MoaModelRef {
                agent_index: 3,
                slot_index: 0,
                agent: "turbo".into(),
                driver: "turbo".into(),
                model: "qwen3-30b-a3b".into(),
                route_id: crate::backplane::RouteId::chat("turbo", "turbo", None),
                expected_revision: crate::backplane::ModelRevision::chat("qwen3-30b-a3b"),
                metered: false,
            },
            available: true,
        };
        let roster = FormationRoster::new(FormationId::TagTeam, &[turbo]);
        let assigned: Vec<_> = roster
            .assignments()
            .iter()
            .map(|a| a.as_ref().map(|r| r.model.clone()))
            .collect();
        assert_eq!(
            assigned,
            vec![
                Some("qwen3-30b-a3b".to_string()), // P1 — the one live box leads
                None,                              // P2 — partner is missing, not faked
                None,                              // J1 — no Sol configured here
                Some("qwen3-30b-a3b".to_string()), // A1 — local synthesis is fine
            ]
        );
        // With Leanstral down, turbo inherits the lead rather than the roster
        // stalling — the chain degrades, it does not pin one box.
        assert!(!roster.is_ready());
    }

    /// The failure this guards is silent, expensive, and easy to ship: with no
    /// fleet box online the generic fallback would fill the "local" seats from
    /// the intelligence order, turning a local mode into a frontier one.
    #[test]
    fn tag_team_leaves_local_seats_empty_rather_than_billing_a_frontier_model() {
        let frontier = MoaModelChoice {
            route: MoaModelRef {
                agent_index: 0,
                slot_index: 0,
                agent: "openai".into(),
                driver: "gpt-5.6-sol".into(),
                model: "gpt-5.6-sol".into(),
                route_id: crate::backplane::RouteId::chat("openai", "gpt-5.6-sol", None),
                expected_revision: crate::backplane::ModelRevision::chat("gpt-5.6-sol"),
                metered: true,
            },
            available: true,
        };
        let roster = FormationRoster::new(FormationId::TagTeam, &[frontier]);
        let assigned: Vec<_> = roster
            .assignments()
            .iter()
            .map(|a| a.as_ref().map(|r| r.model.clone()))
            .collect();
        // Only the judge seat fills; both proposers and the synthesis seat stay
        // empty, so the deck refuses to engage instead of quietly spending.
        assert_eq!(
            assigned,
            vec![None, None, Some("gpt-5.6-sol".to_string()), None,]
        );
        assert!(!roster.is_ready());
    }

    #[test]
    fn all_in_rosters_every_adaptive_execution_slot() {
        let formation = *formation(FormationId::AllIn);
        let slots = formation.slots();
        assert_eq!(
            slots
                .iter()
                .filter(|slot| slot.role == FormationRole::Propose)
                .count(),
            formation.max_width
        );
        assert_eq!(
            slots
                .iter()
                .filter(|slot| slot.role == FormationRole::Aggregate)
                .count(),
            formation.samples
        );
        assert!(slots.iter().any(|slot| slot.reserve));
    }

    #[test]
    fn roster_requires_every_slot_and_assigns_per_slot() {
        let model = MoaModelChoice {
            route: MoaModelRef {
                agent_index: 1,
                slot_index: 2,
                agent: "spark".into(),
                driver: "qwen".into(),
                model: "qwen-32b".into(),
                route_id: crate::backplane::RouteId::chat("spark", "qwen", None),
                expected_revision: crate::backplane::ModelRevision::chat("qwen-32b"),
                metered: false,
            },
            available: true,
        };
        let roster = FormationRoster::new(FormationId::Council, &[model]);
        assert!(roster.is_ready());
        assert_eq!(
            roster.assigned_count(),
            formation(FormationId::Council).slots().len()
        );
        assert_eq!(roster.local_count(), roster.slot_count());
    }

    #[test]
    fn think_picker_stages_and_clears_the_focused_seats_role_effort() {
        let model = |agent_index, agent: &str, model: &str| MoaModelChoice {
            route: MoaModelRef {
                agent_index,
                slot_index: 0,
                agent: agent.into(),
                driver: model.into(),
                model: model.into(),
                route_id: crate::backplane::RouteId::chat(agent, model, None),
                expected_revision: crate::backplane::ModelRevision::chat(model),
                metered: false,
            },
            available: true,
        };
        let mut deck = MoaDeckState::new(vec![
            model(0, "alpha", "model-a"),
            model(1, "beta", "model-b"),
        ]);
        deck.select(FormationId::Duel);
        assert!(deck.select_slot(2)); // Duel graph order: P1 P2 J1 A1
        let ladder = || vec!["low".to_string(), "medium".to_string(), "high".to_string()];
        assert!(deck.open_effort_picker(ladder()));
        assert_eq!(deck.focus(), MoaDeckFocus::Efforts);
        // Nothing staged yet: the cursor rests on the env-default row.
        assert_eq!(deck.selected_effort_index(), 3);
        deck.move_effort(-1); // wraps up onto "high"
        assert_eq!(deck.selected_effort_index(), 2);
        assert_eq!(
            deck.stage_selected_effort(),
            Some((FormationRole::Judge, Some("high".to_string())))
        );
        assert_eq!(deck.focus(), MoaDeckFocus::Slots);
        assert_eq!(
            deck.selected_roster().role_effort(FormationRole::Judge),
            Some("high")
        );
        // The engagement snapshot carries the staged THINK column.
        let engagement = deck.engagement().expect("duel roster is ready");
        assert_eq!(
            engagement.roster.role_effort(FormationRole::Judge),
            Some("high")
        );
        // Reopening lands the cursor on the staged level; the env-default row
        // clears the seat back to env policy.
        assert!(deck.open_effort_picker(ladder()));
        assert_eq!(deck.selected_effort_index(), 2);
        deck.move_effort(1);
        assert_eq!(
            deck.stage_selected_effort(),
            Some((FormationRole::Judge, None))
        );
        assert!(
            deck.selected_roster()
                .role_effort(FormationRole::Judge)
                .is_none()
        );
    }

    /// The verification chain of the roster THINK column: stage an effort on a
    /// seat through the picker, activate the roster on a real Bag, and the
    /// staged value is exactly what [`FormationRoster::seat_efforts`] hands the
    /// engaged wrapper (Bag activation consumes this same roster surface).
    #[test]
    fn staged_think_effort_rides_activation_into_seat_efforts() {
        let _lock = crate::tests::env_lock();
        let mut bag = crate::club::Bag::for_reasoning_render_test();
        let mut deck = MoaDeckState::new(bag.moa_model_choices());
        deck.select(FormationId::Duel);
        assert!(deck.select_slot(2)); // J1
        let sol = deck
            .models()
            .iter()
            .position(|choice| choice.route.model == "gpt-5.6-sol")
            .expect("ladder-declaring route");
        assert!(deck.assign_model(sol));
        // The ladder exactly as the deck opener resolves it from the Bag.
        let route = deck
            .selected_roster()
            .assignment(2)
            .expect("J1 assigned")
            .clone();
        let levels = bag
            .route_choices()
            .iter()
            .find(|choice| {
                choice.agent_index == route.agent_index && choice.slot_index == route.slot_index
            })
            .map(|choice| choice.reasoning_levels.to_vec())
            .expect("route row in the deck");
        assert_eq!(levels, vec!["low", "medium", "high"]);
        assert!(deck.open_effort_picker(levels));
        deck.move_effort(-1); // env-default row -> "high"
        assert_eq!(
            deck.stage_selected_effort(),
            Some((FormationRole::Judge, Some("high".to_string())))
        );
        let engagement = deck.engagement().expect("duel roster is ready");
        bag.activate_sota_moa_with_roster(&engagement.roster)
            .expect("staged roster activates");
        assert_eq!(bag.in_hand_label(), "sota");
        let seats = engagement.roster.seat_efforts();
        assert_eq!(seats.judge.as_deref(), Some("high"));
        assert!(seats.propose.is_none());
        assert!(seats.verify.is_none());
        assert!(seats.aggregate.is_none());
    }

    /// Capability truth: a ladder-less route opens the picker with no invented
    /// levels — only the env-default clear row remains actionable.
    #[test]
    fn think_picker_without_a_ladder_offers_only_the_env_default_row() {
        let model = MoaModelChoice {
            route: MoaModelRef {
                agent_index: 0,
                slot_index: 0,
                agent: "alpha".into(),
                driver: "model-a".into(),
                model: "model-a".into(),
                route_id: crate::backplane::RouteId::chat("alpha", "model-a", None),
                expected_revision: crate::backplane::ModelRevision::chat("model-a"),
                metered: false,
            },
            available: true,
        };
        let mut deck = MoaDeckState::new(vec![model]);
        deck.select(FormationId::Duel);
        assert!(deck.select_slot(0));
        assert!(deck.open_effort_picker(Vec::new()));
        assert!(deck.effort_options().is_empty());
        assert_eq!(deck.selected_effort_index(), 0); // the env-default row
        assert_eq!(
            deck.stage_selected_effort(),
            Some((FormationRole::Propose, None))
        );
    }

    /// The swarm carries no scout seat effort, so the picker refuses the seat
    /// instead of staging a value activation would silently drop.
    #[test]
    fn think_picker_refuses_the_scout_seat() {
        let mut deck = MoaDeckState::new(Vec::new());
        deck.select(FormationId::Recon);
        assert!(deck.select_slot(0)); // Recon's S1
        assert!(!deck.open_effort_picker(vec!["low".to_string()]));
        assert_ne!(deck.focus(), MoaDeckFocus::Efforts);
        assert!(deck.stage_effort(0).is_none());
    }

    #[test]
    fn deck_assignment_changes_only_the_focused_slot() {
        let model = |agent_index, agent: &str, model: &str| MoaModelChoice {
            route: MoaModelRef {
                agent_index,
                slot_index: 0,
                agent: agent.into(),
                driver: model.into(),
                model: model.into(),
                route_id: crate::backplane::RouteId::chat(agent, model, None),
                expected_revision: crate::backplane::ModelRevision::chat(model),
                metered: false,
            },
            available: true,
        };
        let mut deck = MoaDeckState::new(vec![
            model(0, "alpha", "model-a"),
            model(1, "beta", "model-b"),
        ]);
        deck.select(FormationId::Duel);
        assert!(deck.select_slot(0));
        assert!(deck.assign_model(1));
        assert_eq!(
            deck.selected_roster()
                .assignment(0)
                .map(|route| route.model.as_str()),
            Some("model-b")
        );
        assert_eq!(
            deck.selected_roster()
                .assignment(1)
                .map(|route| route.model.as_str()),
            Some("model-a")
        );
    }
}

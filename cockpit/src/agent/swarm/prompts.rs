//! Persona and stage prompts plus the swarm knobs.

use super::*;

// ---------------------------------------------------------------------------
// Angles of attack + stage prompts
// ---------------------------------------------------------------------------

/// Distinct lenses for the layer-0 proposers. The swarm takes the first `width`
/// of these, so the directions are diverse *by construction* — each agent is
/// told to think differently — rather than relying on sampling temperature
/// alone to scatter otherwise-identical prompts.
pub(crate) const PERSONAS: &[(&str, &str)] = &[
    (
        "first-principles",
        "You are a first-principles analyst. Strip the problem to its fundamentals \
         and reason up from what must be true. Ignore convention; derive the answer.",
    ),
    (
        "red-team",
        "You are a skeptical red-teamer. Hunt the flaws, failure modes, and hidden \
         assumptions in the obvious answer. Argue what would make it wrong, then say \
         what survives that scrutiny.",
    ),
    (
        "pragmatist",
        "You are a pragmatic builder. Focus on what actually works in practice, the \
         simplest thing that ships, and the real-world constraints others gloss over.",
    ),
    (
        "lateral",
        "You are a lateral, inventive thinker. Find the unconventional angle, the \
         reframing, or the analogy from a distant field that cracks the problem open.",
    ),
    (
        "systems",
        "You are a systems thinker. Trace second-order effects, feedback loops, and \
         how the parts interact over time. Map the whole, not the part.",
    ),
    (
        "empiricist",
        "You are an empiricist. Ground every claim in evidence, numbers, and what can \
         be measured or tested. Flag what is unknown and how you would find out.",
    ),
];

/// Appended to each proposer's persona. Proposers are *raw material* for the
/// synthesizer, so they're told to be dense rather than polished — which also
/// keeps the fan-out fast, since long prose is the main latency cost.
pub(crate) const PROPOSER_BASE: &str = "Analyze the user's problem from your assigned angle. Be dense \
    and concise: terse bullet points and decisive claims, not polished prose. Your output is \
    raw material another agent will synthesize, so prioritize distinct, non-obvious insights \
    over completeness or readability. No preamble, no conclusion. \
    This stage is TEXT ONLY: no tool calls, and no tool-call markup of any kind \
    (`<tool_call>`, `<function=…>`, `<SHELL>{…}`). The surrounding system prompt documents a \
    tool protocol for the driver, not for you — imitating it here discards your draft. Write \
    the analysis itself, from what you already know.";

/// System prompt for an aggregator: fuse independent drafts into something better
/// than any one of them, without narrating that it's merging anything.
pub(crate) const AGGREGATOR_SYS: &str = "You are an aggregator in a mixture-of-agents. You will be \
    shown several independent expert responses to the user's problem, each from a \
    different angle. Produce a single response stronger than any one of them: integrate \
    the best reasoning, reconcile contradictions, keep what is correct, discard what is \
    wrong, and cover angles a single response missed.";

/// One-word classifier that drives the adaptive gate.
pub(crate) const CLASSIFIER_SYS: &str = "Classify the user's request. Answer with exactly one word: \
    OPEN if it is open-ended, ambiguous, exploratory, or benefits from being attacked \
    from multiple angles; TIGHT if it is a narrow, well-specified directive with one \
    clear right answer or action. One word only.";

/// `REFLECT`: the critic that finds weaknesses so the next layer can fix them.
pub(crate) const CRITIC_SYS: &str = "You are a rigorous critic inside a mixture-of-agents. Your job is \
    to find what is weak so the next revision is stronger. Be specific, skeptical, and brief.";

/// `JUDGE`: scores candidate drafts for Pareto-style pruning.
pub(crate) const JUDGE_SYS: &str = "You are an impartial, calibrated judge scoring candidate responses. \
    Be harsh; reserve high scores for genuinely strong answers. Follow the output format exactly.";

/// `JUDGE_DIMS`: the weighted quality dimensions the judge scores when per-dimension
/// scoring is on. Weights sum to 1.0, so a draft's composite stays on the same 0–10
/// scale as the single-score judge and feeds `median_rank` unchanged. Order is the
/// emit order the judge prompt asks for, so the prompt and parser never drift.
pub(crate) const JUDGE_DIMS: &[(&str, f64)] = &[
    ("correctness", 0.40),
    ("insight", 0.25),
    ("rigor", 0.20),
    ("novelty", 0.15),
];

/// `SAMPLES`: picks the single best of several final candidates (self-consistency).
pub(crate) const CHOOSER_SYS: &str = "You select the single best answer from a set of candidates. \
    Output only the number of the best one.";

/// `VERIFY`: the adversary that tries to break the final answer.
pub(crate) const VERIFIER_SYS: &str = "You are an adversarial verifier. Find concrete errors, \
    unsupported claims, logical gaps, or missing considerations in the answer under review.";

/// `HEDGE`: the calibrated-claims "hedge ladder" — folded into the base voice so
/// every stage states a finding no more strongly than its support allows.
pub(crate) const HEDGE_LADDER: &str = "Calibrate every claim's strength to its evidence: write \
    \"demonstrates\"/\"shows\" only for what is established, \"suggests\"/\"indicates\" for \
    supported-but-partial, \"may\"/\"might\" for plausible, and \"hypothesize\" for \
    speculation. Do not overstate. Flag what is uncertain or unverified rather than \
    asserting it.";

/// Tell every MoA worker to preserve compactable state. The actual transcript
/// compactor remains budget-driven in the harness; this makes the agents write
/// their intermediate work so a third-loop checkpoint has useful material.
pub(crate) fn compaction_prep_instruction() -> Option<String> {
    let every = env_usize("ANGEL_MOA_COMPACT_EVERY_LOOPS", 3);
    if every == 0 {
        return None;
    }
    Some(format!(
        "Context checkpointing: assume this MoA thread may be compacted after every \
         {every} loop(s). Keep each stage checkpoint-ready: include durable decisions, \
         key evidence or source URLs, relevant paths and commands, open risks, and the \
         next action. Do not rely on raw earlier context surviving compaction."
    ))
}

pub(crate) fn answer_stage_with_compaction_prep(stage: &str) -> String {
    match compaction_prep_instruction() {
        Some(prep) if stage.trim().is_empty() => prep,
        Some(prep) => format!("{stage}\n\n{prep}"),
        None => stage.to_string(),
    }
}

/// `DELEGATE`: appended to delegator personas so they can request a verifying test.
pub(crate) const DELEGATOR_INSTRUCTION: &str = "If one key claim can be settled by a quick test or by \
    consulting a specialist model, you MAY request exactly one. Append a single fenced block:\n\
    ```swarm-test\n\
    where: local            # local | remote:<host> | peer:<agent> | phone:<model>\n\
    cmd: <a shell/cargo command — or, for phone, the question to ask>\n\
    claim: <the claim this checks>\n\
    why: <why here>\n\
    ```\n\
    phone targets are model specialists: phone:deepseek (the default — a strong external SOTA \
    second opinion), phone:code, phone:reason, phone:fast. Use a sandboxed test for empirical/ \
    runtime facts; phone a specialist for hard math/code/reasoning. Only if it genuinely helps; \
    no destructive commands.";

/// Whether the request warrants the full fan-out or a single direct pass.
pub(crate) enum Mandate {
    Open,
    Tight,
}

// ---------------------------------------------------------------------------
// Knobs
// ---------------------------------------------------------------------------

/// Every tunable for a swarm run. Built from env by [`SwarmClub::from_env_with`]; the
/// autoresearch stages (`reflect`/`judge`/`samples`/`verify`/`research`) are all
/// off by default and compose freely. `ANGEL_SWARM_MAX` flips the whole stack on.
#[derive(Clone)]
pub(crate) struct Knobs {
    /// Proposers/aggregators per layer (K). Clamped to the persona count.
    pub(crate) width: usize,
    /// Total proposers allowed across progressive waves.
    pub(crate) max_width: usize,
    /// Progressive proposer waves. `1` is the legacy single-wave path.
    pub(crate) max_waves: usize,
    /// Multiplier for the next wave size.
    pub(crate) wave_growth: f64,
    /// Minimum novel fraction in a returned wave before buying more width.
    pub(crate) novelty_floor: f64,
    /// Stop expanding when dissent stops moving by more than this amount.
    pub(crate) dissent_epsilon: f64,
    /// Wave-0 multiplier for locally-estimated hard prompts.
    pub(crate) hard_factor: usize,
    /// Width of refinement layers, decoupled from total proposer width.
    pub(crate) refine_width: usize,
    /// Layers (L): layer 0 proposes; layers 1..L re-aggregate; then a final synth.
    pub(crate) layers: usize,
    /// Skip the adaptive gate and always fan out.
    pub(crate) always: bool,
    /// An operator-armed formation owns this wrapper (see
    /// `SwarmClub::with_engaged_formation`). Unlike `always`, engagement only
    /// satisfies the `MoaMode::Manual` summon check in `drive_with_tools`: the
    /// turn router and the `MIN_AMPLIFY_CHARS` short-answer skip still apply,
    /// so an armed formation amplifies deliberate/synthesis answers — not
    /// every ack.
    pub(crate) engaged: bool,
    /// When the driver's final answer earns MoA amplification
    /// (`drive_with_tools`). `Manual` — the sota-moa default — spends like a
    /// single coordinator until the turn opens with a `moa:` summon.
    pub(crate) mode: MoaMode,
    /// `REFLECT`: critique the drafts between layers and feed it forward.
    pub(crate) reflect: bool,
    /// `JUDGE`: score drafts and keep only the strongest before synthesis.
    pub(crate) judge: bool,
    /// Reviewers on the judge panel (1 = a single calibrated judge; >1 votes by
    /// per-draft median, peer-review style). Clamped to ≥1.
    pub(crate) judge_panel: usize,
    /// `JUDGE_DIMS`: score drafts on weighted dimensions (correctness/insight/
    /// rigor/novelty → one composite) instead of a single 0–10. Off = single score.
    pub(crate) dims: bool,
    /// Drafts to keep after judging (0 = auto: the top half, min 2).
    pub(crate) keep: usize,
    /// `SAMPLES`: number of final syntheses to draw, then pick the best (1 = off).
    pub(crate) samples: usize,
    /// `VERIFY`: adversarial check → revise rounds on the final answer (0 = off).
    pub(crate) verify: usize,
    /// `VERIFY_GUARD`: keep the best-scored answer across revise rounds so a
    /// revision can never regress below an earlier one (costs one score call/round).
    pub(crate) verify_guard: bool,
    /// `RESEARCH`: web-ground the proposers from a live search.
    pub(crate) research: bool,
    /// `CITE`: when research-grounded, cross-check the final answer's claims against
    /// the gathered sources — attribute what's supported, hedge/drop what isn't.
    pub(crate) cite: bool,
    /// `HEDGE`: fold the calibrated-claims "hedge ladder" into the base voice so
    /// every stage ties claim strength to evidence strength.
    pub(crate) hedge: bool,
    /// Search endpoint for `research` (SearXNG-style JSON; `q`/`format` appended).
    pub(crate) search_url: String,
    /// `DELEGATE`: let delegator personas request verifying tests (angel routes them).
    pub(crate) delegate: bool,
    /// Cap on tests run per swarm invocation (abuse guard).
    pub(crate) max_tests: usize,
    /// `DISSENT`: gate per-turn spend on measured proposer disagreement — relax
    /// the pipeline when drafts agree, escalate (judge + verify) when they don't.
    pub(crate) dissent_gate: bool,
    /// Draft count above which scale coordination engages.
    pub(crate) scale_threshold: usize,
    /// Enable greedy local clustering before judge/synthesis at scale.
    pub(crate) cluster: bool,
    /// Content-word Jaccard threshold for scale clustering.
    pub(crate) cluster_sim: f64,
    /// Representative selection mode: `detail` or `medoid`.
    pub(crate) rep_mode: String,
    /// Drafts per tournament pod.
    pub(crate) pod_size: usize,
    /// Winners advanced from each pod.
    pub(crate) pod_keep: usize,
    /// Soft cap on judge calls in one tournament round.
    pub(crate) judge_fanout: usize,
    /// Whether judge prompts show cluster consensus weights.
    pub(crate) judge_weights: bool,
    /// Flat aggregation fan-in; above this, map-reduce synthesis runs first.
    pub(crate) agg_fanin: usize,
    /// Scaled gate relaxes when one cluster dominates at this share.
    pub(crate) dominant_relax: f64,
    /// Scaled gate escalates when effective cluster count reaches this.
    pub(crate) eclusters_hi: f64,
    /// Per-seat reasoning-effort requests (the roster's THINK column).
    pub(crate) seat_efforts: SeatEfforts,
}

/// Per-seat reasoning-effort requests, riding each stage call rather than club
/// state — so one shared club (a homogeneous cheap roster) can serve several
/// seats at different efforts, and a failover keeps the seat's effort. Values
/// are requests: each seat's club applies its own validation/dialect and an
/// unsupported value is simply not sent.
#[derive(Clone, Default)]
pub(crate) struct SeatEfforts {
    pub(crate) propose: Option<String>,
    pub(crate) judge: Option<String>,
    pub(crate) verify: Option<String>,
    pub(crate) aggregate: Option<String>,
}

/// Amplification policy for the tool-capable driver hop. Decides *when* the
/// final answer is re-derived by the full mixture; tool passthrough and the
/// coordinator (aggregate) pass are unconditional in every mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MoaMode {
    /// Heuristic gate (the old behavior): research-routed or synthesis-marker
    /// prompts amplify on their own.
    Auto,
    /// Coordinator-only until summoned: amplify only a turn that opens with
    /// `moa:` (see [`summons_moa`]) or when `always` is set. Standard
    /// single-model token spend by default — the token-blowout fix.
    Manual,
    /// Every turn amplifies (same effect as `always`).
    Always,
}

impl MoaMode {
    pub(crate) fn from_env_var(var: &str, default: MoaMode) -> MoaMode {
        match std::env::var(var)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "manual" | "summon" | "off" => MoaMode::Manual,
            "always" | "full" | "max" => MoaMode::Always,
            "auto" | "heuristic" => MoaMode::Auto,
            _ => default,
        }
    }
}

impl Default for Knobs {
    fn default() -> Self {
        Self {
            seat_efforts: SeatEfforts::default(),
            width: 4,
            max_width: 4,
            max_waves: 1,
            wave_growth: 2.0,
            novelty_floor: 0.35,
            dissent_epsilon: 0.05,
            hard_factor: 2,
            refine_width: 6,
            layers: 2,
            always: false,
            engaged: false,
            mode: MoaMode::Auto,
            reflect: false,
            judge: false,
            judge_panel: 1,
            dims: false,
            keep: 0,
            samples: 1,
            verify: 0,
            verify_guard: false,
            research: false,
            cite: false,
            hedge: false,
            search_url: default_search_url(),
            delegate: false,
            max_tests: 3,
            // Quality-first default: heuristic spend reduction is opt-in.
            dissent_gate: false,
            scale_threshold: 6,
            cluster: false,
            cluster_sim: 0.55,
            rep_mode: "detail".to_string(),
            pod_size: 6,
            pod_keep: 2,
            judge_fanout: 18,
            judge_weights: false,
            agg_fanin: usize::MAX,
            dominant_relax: 0.67,
            eclusters_hi: 4.0,
        }
    }
}

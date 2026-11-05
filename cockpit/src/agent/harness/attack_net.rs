//! The Attack Network: plan a fan-out as a lineage chart before it runs.
//!
//! An objective sits at the root; under it, **leaders** (a role, a mandate, a
//! club — Sonnet-class by default); under each leader, **subtasks** (a prompt,
//! a club — Haiku-class by default, a tool grant). The operator reviews the
//! chart, revises leader roles, and every revision marks that leader's subtree
//! *stale*: the subtasks no longer follow from the role, so launch is blocked
//! until they are regenerated (one scoped planner call per revised leader) and
//! the plan is approved again.
//!
//! Approval compiles the tree into an ordinary [`GraphSpec`] and the existing
//! [`AgentGraphEngine`] runs it — no second executor. Planning reads top-down;
//! execution runs bottom-up: subtasks fan out in parallel, each leader
//! synthesizes and checks its own subtasks, and the root folds the leaders.
//!
//! Everything here is pure (prompts, parsing, revision state, compilation,
//! layout, spline geometry) except [`plan_with`]/[`regenerate_with`], which
//! make exactly one `Club::respond` call each.

use super::{GraphNodeSpec, GraphSpec};
use crate::agent::club::Club;
use serde::{Deserialize, Serialize};

/// Leaders and subtasks are capped so one planner reply cannot become an
/// unbounded wave of metered seats.
pub(crate) const MAX_LEADERS: usize = 6;
pub(crate) const MAX_SUBTASKS_PER_LEADER: usize = 6;
const ROLE_CAP: usize = 80;
const TEXT_CAP: usize = 2_000;

/// Default seat for leaders and the root synthesis (Sonnet-class).
pub(crate) const DEFAULT_LEADER_CLUB: &str = "anthropic";
/// Default seat for subtasks (Haiku-class).
pub(crate) const DEFAULT_WORKER_CLUB: &str = "anthropic-fast";

pub(crate) fn leader_club_default() -> String {
    env_or("ANGEL_ATTACK_LEADER_CLUB", DEFAULT_LEADER_CLUB)
}

pub(crate) fn worker_club_default() -> String {
    env_or("ANGEL_ATTACK_WORKER_CLUB", DEFAULT_WORKER_CLUB)
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum LeaderState {
    /// As the planner drafted it.
    Drafted,
    /// The operator changed the role/mandate; subtasks are stale.
    Revised,
    /// Subtasks were regenerated against the current role.
    Regenerated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Subtask {
    pub id: String,
    pub prompt: String,
    pub club: String,
    /// none | read_only | code | research
    pub tools: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Leader {
    pub id: String,
    pub role: String,
    pub mandate: String,
    pub persona: Option<String>,
    pub club: String,
    pub state: LeaderState,
    pub subtasks: Vec<Subtask>,
}

impl Leader {
    pub(crate) fn stale(&self) -> bool {
        self.state == LeaderState::Revised
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AttackPlan {
    pub objective: String,
    pub synth_club: String,
    pub leaders: Vec<Leader>,
    /// Bumped on every edit; approval is bound to one revision.
    pub revision: u64,
    pub approved_revision: Option<u64>,
}

/// Where a selection cursor can sit in the chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NodeRef {
    Root,
    Leader(usize),
    Subtask(usize, usize),
}

impl AttackPlan {
    pub(crate) fn stale_leaders(&self) -> Vec<usize> {
        (0..self.leaders.len())
            .filter(|&i| self.leaders[i].stale())
            .collect()
    }

    /// Launch needs a non-empty plan, nothing stale, and approval of exactly
    /// the current revision.
    pub(crate) fn launch_blocker(&self) -> Option<String> {
        if self.leaders.is_empty() {
            return Some("the plan has no leaders".to_string());
        }
        if let Some(i) = self.leaders.iter().position(|l| l.subtasks.is_empty()) {
            return Some(format!("leader '{}' has no subtasks", self.leaders[i].id));
        }
        let stale = self.stale_leaders();
        if !stale.is_empty() {
            let ids: Vec<&str> = stale.iter().map(|&i| self.leaders[i].id.as_str()).collect();
            return Some(format!(
                "regenerate subtasks for revised leader(s): {}",
                ids.join(", ")
            ));
        }
        if self.approved_revision != Some(self.revision) {
            return Some("approve the current plan before launch".to_string());
        }
        None
    }

    /// Revise a leader's role (and optionally its mandate). A real change
    /// marks the subtree stale and withdraws approval; a no-op edit does not.
    pub(crate) fn revise_leader(
        &mut self,
        index: usize,
        role: &str,
        mandate: Option<&str>,
    ) -> Result<bool, String> {
        let leader = self
            .leaders
            .get_mut(index)
            .ok_or_else(|| format!("no leader at {index}"))?;
        let role = cap(role.trim(), ROLE_CAP);
        if role.is_empty() {
            return Err("a leader role cannot be empty".to_string());
        }
        let mandate = mandate.map(|m| cap(m.trim(), TEXT_CAP));
        let changed = leader.role != role || mandate.as_ref().is_some_and(|m| *m != leader.mandate);
        if !changed {
            return Ok(false);
        }
        leader.role = role;
        if let Some(m) = mandate {
            leader.mandate = m;
        }
        leader.state = LeaderState::Revised;
        self.touch();
        Ok(true)
    }

    /// Re-point a leader or subtask at another club (tier swap). Does not
    /// stale the subtree: the work is unchanged, only who does it.
    pub(crate) fn set_club(&mut self, node: NodeRef, club: &str) -> Result<(), String> {
        let club = club.trim();
        if club.is_empty() {
            return Err("club cannot be empty".to_string());
        }
        match node {
            NodeRef::Root => self.synth_club = club.to_string(),
            NodeRef::Leader(i) => {
                self.leaders
                    .get_mut(i)
                    .ok_or_else(|| format!("no leader at {i}"))?
                    .club = club.to_string()
            }
            NodeRef::Subtask(i, j) => {
                self.leaders
                    .get_mut(i)
                    .and_then(|l| l.subtasks.get_mut(j))
                    .ok_or_else(|| format!("no subtask at {i}.{j}"))?
                    .club = club.to_string()
            }
        }
        self.touch();
        Ok(())
    }

    /// Install regenerated subtasks for one leader.
    pub(crate) fn apply_subtasks(
        &mut self,
        index: usize,
        drafts: Vec<SubtaskDraft>,
        worker_club: &str,
    ) -> Result<(), String> {
        if drafts.is_empty() {
            return Err("the planner returned no subtasks".to_string());
        }
        let leader = self
            .leaders
            .get_mut(index)
            .ok_or_else(|| format!("no leader at {index}"))?;
        leader.subtasks = build_subtasks(&leader.id, drafts, worker_club);
        leader.state = LeaderState::Regenerated;
        self.touch();
        Ok(())
    }

    pub(crate) fn approve(&mut self) -> Result<(), String> {
        let stale = self.stale_leaders();
        if !stale.is_empty() {
            return Err(format!(
                "{} leader(s) still stale — regenerate before approving",
                stale.len()
            ));
        }
        self.approved_revision = Some(self.revision);
        Ok(())
    }

    fn touch(&mut self) {
        self.revision += 1;
        self.approved_revision = None;
    }

    pub(crate) fn node_count(&self) -> usize {
        1 + self
            .leaders
            .iter()
            .map(|l| 1 + l.subtasks.len())
            .sum::<usize>()
    }

    /// Compile the approved tree into a runnable graph: subtasks (no deps) →
    /// leader (depends on its subtasks, synthesizes + checks) → root (depends
    /// on every leader, folds the final answer).
    pub(crate) fn to_graph_spec(&self) -> Result<GraphSpec, String> {
        if let Some(blocker) = self.launch_blocker() {
            return Err(blocker);
        }
        let mut nodes = Vec::with_capacity(self.node_count());
        let mut leader_ids = Vec::with_capacity(self.leaders.len());
        for leader in &self.leaders {
            let mut sub_ids = Vec::with_capacity(leader.subtasks.len());
            for sub in &leader.subtasks {
                nodes.push(node(
                    &sub.id,
                    format!(
                        "You are one worker on a team led by the {role}.\n\
                         Overall objective:\n{{task}}\n\n\
                         Your leader's mandate:\n{mandate}\n\n\
                         Your subtask (do only this, completely and concretely):\n{prompt}",
                        role = leader.role,
                        mandate = leader.mandate,
                        prompt = sub.prompt,
                    ),
                    None,
                    &sub.club,
                    &sub.tools,
                    Vec::new(),
                ));
                sub_ids.push(sub.id.clone());
            }
            nodes.push(node(
                &leader.id,
                format!(
                    "You are the {role}. Your mandate:\n{mandate}\n\n\
                     Overall objective:\n{{task}}\n\n\
                     Your workers' results are below. Check them against your mandate, \
                     resolve conflicts, fill gaps yourself, and deliver one consolidated \
                     result for your area. Name anything you could not verify.",
                    role = leader.role,
                    mandate = leader.mandate,
                ),
                leader.persona.clone(),
                &leader.club,
                "none",
                sub_ids,
            ));
            leader_ids.push(leader.id.clone());
        }
        nodes.push(node(
            "objective",
            "You command this effort. Objective:\n{task}\n\n\
             Each leader's consolidated result is below. Fold them into one final \
             deliverable: complete, consistent, no filler. Flag any open risk."
                .to_string(),
            None,
            &self.synth_club,
            "none",
            leader_ids,
        ));
        Ok(GraphSpec {
            name: "attack".to_string(),
            description: cap(&self.objective, 160),
            deadline_secs: None,
            token_budget: None,
            nodes,
        })
    }
}

fn node(
    id: &str,
    prompt: String,
    persona: Option<String>,
    club: &str,
    tools: &str,
    depends_on: Vec<String>,
) -> GraphNodeSpec {
    GraphNodeSpec {
        id: id.to_string(),
        prompt,
        persona,
        club: Some(club.to_string()),
        tools: Some(tools.to_string()),
        effort: None,
        trainable: false,
        pool: None,
        pool_max_inflight: None,
        depends_on,
        gate: None,
    }
}

// ---------------------------------------------------------------------------
// Planner contract (JSON in, typed tree out)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SubtaskDraft {
    pub prompt: String,
    #[serde(default)]
    pub tools: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct LeaderDraft {
    pub role: String,
    #[serde(default)]
    pub mandate: String,
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub subtasks: Vec<SubtaskDraft>,
}

#[derive(Clone, Debug, Deserialize)]
struct PlanDraft {
    leaders: Vec<LeaderDraft>,
}

#[derive(Clone, Debug, Deserialize)]
struct SubtaskReply {
    subtasks: Vec<SubtaskDraft>,
}

const TOOL_GRANTS: &[&str] = &["none", "read_only", "code", "research"];

pub(crate) fn draft_prompt(objective: &str) -> String {
    format!(
        "Plan a team to attack this objective. Return ONLY a JSON object, no prose:\n\
         {{\"leaders\":[{{\"role\":\"short role title\",\"mandate\":\"what this leader owns and \
         what done means\",\"subtasks\":[{{\"prompt\":\"one self-contained unit of work\",\
         \"tools\":\"none|read_only|code|research\"}}]}}]}}\n\
         Rules: 2-{MAX_LEADERS} leaders with non-overlapping mandates; 1-{MAX_SUBTASKS_PER_LEADER} \
         subtasks each; every subtask independently executable in parallel by a fast model \
         that sees only the objective, its leader's mandate and its own prompt.\n\n\
         Objective:\n{objective}"
    )
}

pub(crate) fn regenerate_prompt(plan: &AttackPlan, index: usize) -> Option<String> {
    let leader = plan.leaders.get(index)?;
    let siblings: Vec<String> = plan
        .leaders
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, l)| format!("- {}: {}", l.role, l.mandate))
        .collect();
    Some(format!(
        "A team leader's role was revised. Write fresh subtasks for it. Return ONLY JSON:\n\
         {{\"subtasks\":[{{\"prompt\":\"...\",\"tools\":\"none|read_only|code|research\"}}]}}\n\
         1-{MAX_SUBTASKS_PER_LEADER} subtasks, each independently executable in parallel, all \
         inside this leader's mandate and not overlapping the other leaders.\n\n\
         Objective:\n{objective}\n\nRevised leader: {role}\nMandate: {mandate}\n\n\
         Other leaders (do not overlap):\n{siblings}",
        objective = plan.objective,
        role = leader.role,
        mandate = leader.mandate,
        siblings = if siblings.is_empty() {
            "(none)".to_string()
        } else {
            siblings.join("\n")
        },
    ))
}

/// Pull the first balanced JSON object out of a model reply (fences, preamble
/// and trailing prose tolerated).
pub(crate) fn extract_json_object(reply: &str) -> Option<&str> {
    let start = reply.find('{')?;
    let bytes = reply.as_bytes();
    let (mut depth, mut in_str, mut escaped) = (0usize, false, false);
    for (offset, &b) in bytes[start..].iter().enumerate() {
        if in_str {
            match (escaped, b) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&reply[start..=start + offset]);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn parse_plan(
    objective: &str,
    reply: &str,
    leader_club: &str,
    worker_club: &str,
) -> Result<AttackPlan, String> {
    let json = extract_json_object(reply).ok_or("planner reply held no JSON object")?;
    let draft: PlanDraft =
        serde_json::from_str(json).map_err(|e| format!("planner JSON did not parse: {e}"))?;
    let mut leaders = Vec::new();
    for (i, d) in draft.leaders.into_iter().take(MAX_LEADERS).enumerate() {
        let role = cap(d.role.trim(), ROLE_CAP);
        if role.is_empty() {
            continue;
        }
        let id = format!("L{}", i + 1);
        let subtasks = build_subtasks(&id, d.subtasks, worker_club);
        leaders.push(Leader {
            id,
            role,
            mandate: cap(d.mandate.trim(), TEXT_CAP),
            persona: d.persona.filter(|p| !p.trim().is_empty()),
            club: leader_club.to_string(),
            state: LeaderState::Drafted,
            subtasks,
        });
    }
    if leaders.is_empty() {
        return Err("planner proposed no usable leaders".to_string());
    }
    Ok(AttackPlan {
        objective: objective.trim().to_string(),
        synth_club: leader_club.to_string(),
        leaders,
        revision: 0,
        approved_revision: None,
    })
}

pub(crate) fn parse_subtasks(reply: &str) -> Result<Vec<SubtaskDraft>, String> {
    let json = extract_json_object(reply).ok_or("planner reply held no JSON object")?;
    let parsed: SubtaskReply =
        serde_json::from_str(json).map_err(|e| format!("planner JSON did not parse: {e}"))?;
    Ok(parsed.subtasks)
}

fn build_subtasks(leader_id: &str, drafts: Vec<SubtaskDraft>, worker_club: &str) -> Vec<Subtask> {
    drafts
        .into_iter()
        .filter(|d| !d.prompt.trim().is_empty())
        .take(MAX_SUBTASKS_PER_LEADER)
        .enumerate()
        .map(|(j, d)| Subtask {
            id: format!("{leader_id}.{}", j + 1),
            prompt: cap(d.prompt.trim(), TEXT_CAP),
            club: worker_club.to_string(),
            tools: d
                .tools
                .map(|t| t.trim().to_ascii_lowercase())
                .filter(|t| TOOL_GRANTS.contains(&t.as_str()))
                .unwrap_or_else(|| "none".to_string()),
        })
        .collect()
}

/// Draft a fresh plan with one planner call.
pub(crate) fn plan_with(
    planner: &dyn Club,
    objective: &str,
    leader_club: &str,
    worker_club: &str,
) -> Result<AttackPlan, String> {
    let reply = planner.respond(&draft_prompt(objective))?;
    parse_plan(objective, &reply, leader_club, worker_club)
}

/// Regenerate one revised leader's subtasks with one planner call.
pub(crate) fn regenerate_with(
    planner: &dyn Club,
    plan: &mut AttackPlan,
    index: usize,
    worker_club: &str,
) -> Result<(), String> {
    let prompt = regenerate_prompt(plan, index).ok_or_else(|| format!("no leader at {index}"))?;
    let reply = planner.respond(&prompt)?;
    plan.apply_subtasks(index, parse_subtasks(&reply)?, worker_club)
}

fn cap(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

// ---------------------------------------------------------------------------
// Layout + spline geometry (pure; the chart widget draws these)
// ---------------------------------------------------------------------------

/// A laid-out node: center in abstract chart units (x grows right, y grows
/// down; root row 0, leaders row 1, subtasks row 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placed {
    pub node: NodeRef,
    pub x: f64,
    pub y: f64,
}

/// Genealogy layout: subtasks get evenly spaced leaf slots, each leader sits
/// centered over its children, the root centered over its leaders. Returns
/// nodes plus parent→child edges as index pairs into the node list. Width is
/// normalized to `[0, 1]`; rows sit at y = 0, 0.5, 1.
pub(crate) fn layout(plan: &AttackPlan) -> (Vec<Placed>, Vec<(usize, usize)>) {
    let leaves: usize = plan.leaders.iter().map(|l| l.subtasks.len().max(1)).sum();
    let slot = |k: f64| {
        if leaves <= 1 {
            0.5
        } else {
            k / (leaves - 1) as f64
        }
    };
    let mut nodes = vec![Placed {
        node: NodeRef::Root,
        x: 0.5,
        y: 0.0,
    }];
    let mut edges = Vec::new();
    let mut next_leaf = 0usize;
    let mut leader_xs = Vec::new();
    for (i, leader) in plan.leaders.iter().enumerate() {
        let leader_index = nodes.len();
        nodes.push(Placed {
            node: NodeRef::Leader(i),
            x: 0.0,
            y: 0.5,
        });
        edges.push((0, leader_index));
        let first = next_leaf;
        if leader.subtasks.is_empty() {
            next_leaf += 1;
        }
        for j in 0..leader.subtasks.len() {
            let idx = nodes.len();
            nodes.push(Placed {
                node: NodeRef::Subtask(i, j),
                x: slot(next_leaf as f64),
                y: 1.0,
            });
            edges.push((leader_index, idx));
            next_leaf += 1;
        }
        let x = (slot(first as f64) + slot((next_leaf - 1) as f64)) / 2.0;
        nodes[leader_index].x = x;
        leader_xs.push(x);
    }
    if !leader_xs.is_empty() {
        nodes[0].x = leader_xs.iter().sum::<f64>() / leader_xs.len() as f64;
    }
    (nodes, edges)
}

/// A genealogy-chart spline from parent (above) to child (below): a cubic
/// Bézier whose control points drop vertically from each end, so the curve
/// leaves the parent straight down and enters the child straight down.
pub(crate) fn spline(from: (f64, f64), to: (f64, f64), samples: usize) -> Vec<(f64, f64)> {
    let samples = samples.max(2);
    let mid_y = (from.1 + to.1) / 2.0;
    let (p0, p1, p2, p3) = (from, (from.0, mid_y), (to.0, mid_y), to);
    (0..samples)
        .map(|k| {
            let t = k as f64 / (samples - 1) as f64;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            (
                a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
                a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/attack_net__tests.rs"]
mod tests;

//! `/graph` command state: launch, watch, and stop agent-graph runs.
//!
//! The engine lives in `harness::agent_graph` and publishes a lock-and-swap
//! `GraphSnapshot` the Round Table stage renders each tick; this module only
//! owns the worker thread and its cancel flag. One run at a time — a graph is
//! an org chart, not a job queue.

use crate::agent::club::Club;
use crate::agent::harness::{AgentGraphEngine, GraphRunError, find_graph, load_graphs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct GraphState {
    cancel: Option<Arc<AtomicBool>>,
    worker: Option<std::thread::JoinHandle<()>>,
    preflight_error: Arc<Mutex<Option<(String, String)>>>,
    run_name: Option<String>,
    previous_episode: Option<String>,
}

impl GraphState {
    pub(crate) fn running(&self) -> bool {
        self.worker.as_ref().is_some_and(|w| !w.is_finished())
    }

    pub(crate) fn start(
        &mut self,
        workspace: PathBuf,
        self_club: Option<Arc<dyn Club>>,
        roster: Vec<Arc<dyn Club>>,
        graph_name: &str,
        task: &str,
    ) -> Result<String, String> {
        if self.running() {
            return Err(
                "a graph run is already active — /graph status to watch it, /graph stop to end it"
                    .to_string(),
            );
        }
        let catalog = load_graphs();
        let spec = find_graph(&catalog, graph_name)?.clone();
        let task = task.trim();
        if task.is_empty() {
            return Err(format!("usage: /graph run {} <task>", spec.name));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let node_count = spec.nodes.len();
        let name = spec.name.clone();
        let task_owned = task.to_string();
        let previous_episode =
            crate::agent::harness::current_graph_snapshot().map(|snapshot| snapshot.episode_id);
        // Keep failures that occur before the engine publishes a snapshot
        // scoped to this run rather than displaying an earlier run's status.
        let preflight_error = Arc::new(Mutex::new(None));
        let worker_error = Arc::clone(&preflight_error);
        let worker_name = name.clone();
        let worker = std::thread::Builder::new()
            .name(format!("graph-run-{name}"))
            .spawn(move || {
                let engine = AgentGraphEngine::new(workspace, self_club, roster);
                // Outcome (answer or speaking failure) lands in the published
                // snapshot + the persisted run record; the stage shows both.
                if let Err(GraphRunError::Preflight(error)) =
                    engine.run(&spec, &task_owned, &worker_cancel)
                    && let Ok(mut slot) = worker_error.lock()
                {
                    *slot = Some((worker_name, error));
                }
            })
            .map_err(|e| format!("graph worker thread failed to start: {e}"))?;
        self.cancel = Some(cancel);
        self.worker = Some(worker);
        self.preflight_error = preflight_error;
        self.run_name = Some(name.clone());
        self.previous_episode = previous_episode;
        Ok(format!(
            "graph · '{name}' launched — {node_count} node(s) · watch the Round Table stage; /graph status · /graph stop"
        ))
    }

    pub(crate) fn stop(&mut self) -> bool {
        if !self.running() {
            return false;
        }
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        true
    }
}

/// `/graph list` — the installed catalog, broken specs included with their
/// reason (a silently missing graph reads as nonexistence).
pub(crate) fn list_text() -> String {
    let catalog = load_graphs();
    if catalog.is_empty() {
        return "graph · no graphs installed — add TOML specs to ~/.angelX/graphs (see cockpit/graphs/ for the shape)"
            .to_string();
    }
    let mut lines = vec![format!("graph · {} installed:", catalog.len())];
    for entry in catalog {
        match (&entry.spec, &entry.error) {
            (Some(spec), _) => lines.push(format!(
                "  {} · {} node(s){} · {}",
                spec.name,
                spec.nodes.len(),
                if spec.description.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", spec.description)
                },
                entry.source.display()
            )),
            (None, Some(error)) => lines.push(format!(
                "  {} · BROKEN · {} · {}",
                entry.name,
                error,
                entry.source.display()
            )),
            (None, None) => lines.push(format!("  {} · empty", entry.name)),
        }
    }
    lines.push("run one: /graph run <name> <task>".to_string());
    lines.join("\n")
}

/// `/graph status` — worker liveness plus the latest published snapshot.
pub(crate) fn status_text(state: &GraphState) -> String {
    use crate::agent::harness::{GraphNodePhase, GraphRunPhase, current_graph_snapshot};
    let mut lines = Vec::new();
    if let Ok(slot) = state.preflight_error.lock()
        && let Some((name, error)) = slot.as_ref()
    {
        return format!("graph · '{name}' · FAILED\n  ✗ {error}");
    }
    let snapshot = current_graph_snapshot();
    if let Some(name) = state.run_name.as_deref()
        && snapshot.as_ref().map(|snapshot| &snapshot.episode_id) == state.previous_episode.as_ref()
    {
        return if state.running() {
            format!("graph · '{name}' · starting")
        } else {
            format!("graph · '{name}' · FAILED\n  ✗ worker ended before publishing a snapshot")
        };
    }
    let Some(snapshot) = snapshot else {
        return "graph · idle — no run yet (/graph list to see the catalog)".to_string();
    };
    let status = match snapshot.phase {
        GraphRunPhase::Running if state.running() => "running",
        GraphRunPhase::Running => "running (worker gone — treat as failed)",
        GraphRunPhase::Done => "DONE",
        GraphRunPhase::Failed => "FAILED",
        GraphRunPhase::Cancelled => "cancelled",
    };
    lines.push(format!(
        "graph · '{}' · {status} · {:.1}s",
        snapshot.graph,
        snapshot.elapsed_ms as f64 / 1000.0
    ));
    for node in &snapshot.nodes {
        let phase = match node.phase {
            GraphNodePhase::Pending => "pending",
            GraphNodePhase::Running => "running",
            GraphNodePhase::Done => "done",
            GraphNodePhase::Failed => "failed",
            GraphNodePhase::Skipped => "skipped",
        };
        lines.push(format!(
            "  {} · {phase}{}{}",
            node.id,
            if node.elapsed_ms > 0 {
                format!(" · {:.1}s", node.elapsed_ms as f64 / 1000.0)
            } else {
                String::new()
            },
            node.gate
                .as_ref()
                .map(|gate| format!(" · gate {gate}"))
                .unwrap_or_default()
        ));
    }
    if let Some(error) = &snapshot.error {
        lines.push(format!("  ✗ {error}"));
    }
    if let Some(answer) = &snapshot.final_answer {
        lines.push("  ─ answer ─".to_string());
        lines.push(format!("  {}", answer.replace('\n', "\n  ")));
    }
    lines.join("\n")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/graph_ctl__tests.rs"]
mod tests;

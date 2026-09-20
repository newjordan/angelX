//! One process-local, persisted, idempotent Living Atlas clerk worker.

use crate::backplane::{BackplaneRegistry, LeaseMode, ModelRevision, RouteId, WorkloadRole};
use crate::club::Club;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

#[derive(Clone)]
pub(crate) struct ClerkRoute {
    pub(crate) route_id: RouteId,
    pub(crate) model_revision: ModelRevision,
    pub(crate) resource_group: String,
    pub(crate) club: Arc<dyn Club>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClerkHealth {
    #[default]
    Idle,
    Running,
    WaitingForRoute,
    ResourceConflict,
    Degraded,
}

impl ClerkHealth {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::WaitingForRoute => "waiting",
            Self::ResourceConflict => "conflict",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct ClerkStatus {
    pub(crate) health: ClerkHealth,
    pub(crate) queue_depth: usize,
    pub(crate) oldest_age_secs: Option<u64>,
    pub(crate) selected_model_revision: Option<ModelRevision>,
    pub(crate) last_result: Option<String>,
}

#[derive(Default)]
struct WorkerState {
    in_flight: bool,
    status: ClerkStatus,
}

pub(crate) struct AtlasClerkWorker {
    atlas: Arc<crate::atlas::AtlasService>,
    state: Mutex<WorkerState>,
}

impl AtlasClerkWorker {
    pub(crate) fn shared(atlas: Arc<crate::atlas::AtlasService>) -> Arc<Self> {
        static WORKERS: OnceLock<Mutex<HashMap<String, Weak<AtlasClerkWorker>>>> = OnceLock::new();
        let workers = WORKERS.get_or_init(|| Mutex::new(HashMap::new()));
        match workers.lock() {
            Ok(mut workers) => {
                if let Some(worker) = workers.get(atlas.project_key()).and_then(Weak::upgrade) {
                    return worker;
                }
                let worker = Arc::new(Self {
                    atlas: Arc::clone(&atlas),
                    state: Mutex::new(WorkerState::default()),
                });
                workers.insert(atlas.project_key().to_string(), Arc::downgrade(&worker));
                worker
            }
            _ => Arc::new(Self {
                atlas,
                state: Mutex::new(WorkerState::default()),
            }),
        }
    }

    pub(crate) fn status(&self) -> ClerkStatus {
        let (queue_depth, oldest_ms) = self.atlas.harvest_queue_status();
        let mut status = self
            .state
            .lock()
            .map(|state| state.status.clone())
            .unwrap_or_default();
        status.queue_depth = queue_depth;
        status.oldest_age_secs = oldest_ms.map(|created| now_ms().saturating_sub(created) / 1_000);
        status
    }

    pub(crate) fn tick(
        self: &Arc<Self>,
        foreground_idle: bool,
        route: impl FnOnce() -> Option<ClerkRoute>,
        registry: Arc<BackplaneRegistry>,
    ) {
        let (queue_depth, oldest_ms) = self.atlas.harvest_queue_status();
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.status.queue_depth = queue_depth;
        state.status.oldest_age_secs =
            oldest_ms.map(|created| now_ms().saturating_sub(created) / 1_000);
        if !foreground_idle || queue_depth == 0 || state.in_flight {
            return;
        }
        // Resolved only past the bails above: the route walk hashes every
        // fleet candidate (SHA-256 ×3 apiece) — far too heavy per idle frame.
        let Some(route) = route() else {
            state.status.health = ClerkHealth::WaitingForRoute;
            state.status.selected_model_revision = None;
            state.status.last_result = Some("queued: no available 30B-class teacher".to_string());
            return;
        };
        let lease = match registry.acquire_scoped(
            &route.resource_group,
            LeaseMode::Serve,
            WorkloadRole::Teacher,
            Some(route.route_id.clone()),
            false,
        ) {
            Ok(lease) => lease,
            Err(error) => {
                state.status.health = ClerkHealth::ResourceConflict;
                state.status.last_result = Some(error);
                return;
            }
        };
        let Some(harvest) = self.atlas.next_harvest() else {
            return;
        };
        state.in_flight = true;
        state.status.health = ClerkHealth::Running;
        state.status.selected_model_revision = Some(route.model_revision.clone());
        drop(state);

        let worker = Arc::clone(self);
        let spawn_result = std::thread::Builder::new()
            .name("atlas-clerk".to_string())
            .spawn(move || {
                let result = crate::term::catch_background_unwind(|| {
                    run_one(&worker.atlas, &route, &lease, &harvest)
                })
                .unwrap_or_else(|_| Err("atlas clerk worker panicked".to_string()));
                if let Ok(mut state) = worker.state.lock() {
                    state.in_flight = false;
                    match result {
                        Ok(result) => {
                            state.status.health = ClerkHealth::Idle;
                            state.status.last_result = Some(result);
                        }
                        Err(error) => {
                            state.status.health = ClerkHealth::Degraded;
                            state.status.last_result = Some(error);
                        }
                    }
                    let (depth, oldest) = worker.atlas.harvest_queue_status();
                    state.status.queue_depth = depth;
                    state.status.oldest_age_secs =
                        oldest.map(|created| now_ms().saturating_sub(created) / 1_000);
                }
            });
        if let Err(error) = spawn_result
            && let Ok(mut state) = self.state.lock()
        {
            state.in_flight = false;
            state.status.health = ClerkHealth::Degraded;
            state.status.last_result = Some(format!("could not start Atlas clerk: {error}"));
        }
    }
}

fn run_one(
    atlas: &crate::atlas::AtlasService,
    route: &ClerkRoute,
    lease: &crate::backplane::ResourceLease,
    harvest: &crate::atlas::HarvestPack,
) -> Result<String, String> {
    let harvest_json = serde_json::to_string(harvest)
        .map_err(|error| format!("could not encode Atlas harvest: {error}"))?;
    let prompt = format!(
        "You are the Living Atlas teacher. Draft zero to three review candidates from \
         the bounded harvest below. Output only strict JSON matching \
         {{\"schema\":\"angel-atlas-clerk/v1\",\"action\":\"propose|merge|abstain\",\
         \"candidates\":[{{\"kind\":\"fact|decision|procedure|note|preference|open_thread|entity|artifact\",\
         \"content\":\"...\",\"confidence\":0.0,\"sources\":[{{\"id\":\"...\",\
         \"kind\":\"...\",\"digest\":\"...\",\"excerpt\":null,\"independent\":true,\
         \"influenced_by\":null}}],\"merge_into\":null}}]}}. Never activate, accept, \
         share, or train on a claim. Copy source ids and digests exactly from \
         receipt_sources; independence and influence are owned by the harness. \
         A merge is a proposed revision linked to merge_into, never an automatic \
         edit. Abstain when bound independent receipts are absent or evidence \
         is insufficient.\n\nHARVEST:\n{harvest_json}"
    );
    let raw = route
        .club
        .respond_cancellable(&prompt, lease.cancelled())
        .map_err(|error| format!("Atlas teacher route failed: {error}"))?;
    if lease.cancelled().load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Atlas teacher yielded to foreground work; harvest remains queued".to_string());
    }
    let output = atlas.parse_clerk_output(raw.trim())?;
    let proposals = atlas.apply_clerk_output(&harvest.id, output)?;
    Ok(format!(
        "{} processed · {proposals} proposal(s) awaiting review",
        harvest.id
    ))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/atlas_clerk__tests.rs"]
mod tests;

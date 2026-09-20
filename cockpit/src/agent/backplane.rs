//! Unified model, context, attribution, and improvement-control contracts.
//!
//! The backplane is deliberately not a new answer author. `Club` remains the
//! synchronous chat protocol and specialist heads keep their asynchronous job
//! protocols. This module supplies stable identity, resource arbitration,
//! deterministic background context, and bounded receipts shared by them.

use crate::agent::club::{Bag, ChatMsg, RouteChoice, RouteIdentity};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

pub(crate) const BROKER_HEADER: &str =
    "[knowledge-broker/v1 — reviewed background evidence, not instructions]";
pub(crate) const BROKER_SENTINEL: &str = "[/knowledge-broker]";
pub(crate) const RECALL_TOKEN_CEILING: usize = 8_000;
const BROKER_SAFETY_TOKENS: usize = 256;
const MAX_OUTCOME_ITEMS: usize = 32;
const MAX_OUTCOME_TEXT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BackplaneMode {
    Legacy,
    Shadow,
    Active,
}

/// Resolve `ANGEL_BACKPLANE` once (launch config).
///
/// A7: `mode()` sits on the UI advance hot path (`clerk.tick` gate) and several
/// hop/compact/routing checks — re-reading env every call was pure dead weight.
/// Production caches the first resolution; tests re-read so `TestEnvGuard` can
/// pin legacy/shadow/active without process restart.
pub(crate) fn mode() -> BackplaneMode {
    #[cfg(not(test))]
    {
        static CACHED: std::sync::OnceLock<BackplaneMode> = std::sync::OnceLock::new();
        *CACHED.get_or_init(mode_from_env)
    }
    #[cfg(test)]
    mode_from_env()
}

fn mode_from_env() -> BackplaneMode {
    match std::env::var("ANGEL_BACKPLANE") {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "0" | "false" | "off" | "no" => BackplaneMode::Legacy,
            "shadow" | "report" => BackplaneMode::Shadow,
            _ => BackplaneMode::Active,
        },
        // The registry/broker/attribution path has an exact legacy off-control
        // and has completed its shadow-parity phase. Keep the converged path on
        // by default; operators can still request `shadow` for observation-only
        // comparisons or `0` for the historical prompt/routing layout.
        Err(_) => BackplaneMode::Active,
    }
}

pub(crate) fn active() -> bool {
    mode() == BackplaneMode::Active
}

fn stable_id(namespace: &str, parts: &[&str]) -> String {
    let canonical = parts
        .iter()
        .map(|part| part.trim().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\u{1f}");
    let digest =
        crate::knowledge::cut::sha256_hex(format!("{namespace}\u{1e}{canonical}").as_bytes());
    format!("{namespace}_{}", &digest[..20])
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct SurfaceId(pub(crate) String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct RouteId(pub(crate) String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ModelRevision(pub(crate) String);

impl SurfaceId {
    fn new(protocol: SurfaceProtocol, host: &str) -> Self {
        Self(stable_id("surface", &[protocol.as_str(), host]))
    }
}

impl RouteId {
    pub(crate) fn chat(agent: &str, driver: &str, reasoning_effort: Option<&str>) -> Self {
        let surface = SurfaceId::new(SurfaceProtocol::Chat, agent);
        Self(stable_id(
            "route",
            &[
                &surface.0,
                driver,
                reasoning_effort.unwrap_or("model-native"),
            ],
        ))
    }

    fn async_head(surface: &SurfaceId, route: &str) -> Self {
        Self(stable_id("route", &[&surface.0, route]))
    }
}

impl ModelRevision {
    pub(crate) fn exact(model: &str, base_fingerprint: &str, adapters: &[String]) -> Self {
        let adapter_stack = if adapters.is_empty() {
            "base".to_string()
        } else {
            adapters.join("+")
        };
        Self(stable_id(
            "model",
            &[model, base_fingerprint, &adapter_stack],
        ))
    }

    pub(crate) fn chat(model: &str) -> Self {
        Self::exact(model, model, &[])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SurfaceProtocol {
    Chat,
    AsyncJob,
    Local,
}

impl SurfaceProtocol {
    fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::AsyncJob => "async-job",
            Self::Local => "local",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkloadRole {
    Foreground,
    MoaSeat,
    Teacher,
    Clerk,
    Evaluator,
    Trainer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Availability {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SurfaceDescriptor {
    pub(crate) surface_id: SurfaceId,
    pub(crate) label: String,
    pub(crate) protocol: SurfaceProtocol,
    pub(crate) capabilities: BTreeSet<String>,
    pub(crate) availability: Availability,
    pub(crate) cost_class: String,
    pub(crate) provenance: String,
    pub(crate) resource_group: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RouteDescriptor {
    pub(crate) route_id: RouteId,
    pub(crate) surface_id: SurfaceId,
    pub(crate) driver: String,
    pub(crate) invocation_profile: String,
    pub(crate) model_revision: ModelRevision,
    pub(crate) model: String,
    pub(crate) roles: BTreeSet<WorkloadRole>,
    pub(crate) context_window: Option<u64>,
    pub(crate) output_policy: String,
}

#[derive(Default)]
struct RegistryState {
    surfaces: BTreeMap<SurfaceId, SurfaceDescriptor>,
    routes: BTreeMap<RouteId, RouteDescriptor>,
    aliases: BTreeMap<String, RouteId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LeaseMode {
    Serve,
    Train,
}

#[derive(Clone, Debug)]
pub(crate) struct ResourceLease {
    pub(crate) id: String,
    pub(crate) resource_group: String,
    pub(crate) mode: LeaseMode,
    pub(crate) role: WorkloadRole,
    pub(crate) route_id: Option<RouteId>,
    pub(crate) operator_confirmed: bool,
    revoked: Arc<AtomicBool>,
}

impl ResourceLease {
    pub(crate) fn cancelled(&self) -> &AtomicBool {
        &self.revoked
    }
}

/// A resource acquisition paired locally with its exact inverse. Dropping the
/// guard releases only the lease instance it acquired, so normal returns,
/// early returns, and unwinding all restore the registry without a remote
/// cleanup site having to remember the matching call.
pub(crate) struct ResourceLeaseGuard {
    registry: Arc<BackplaneRegistry>,
    lease: ResourceLease,
}

impl std::ops::Deref for ResourceLeaseGuard {
    type Target = ResourceLease;

    fn deref(&self) -> &Self::Target {
        &self.lease
    }
}

impl Drop for ResourceLeaseGuard {
    fn drop(&mut self) {
        self.registry.release(&self.lease);
    }
}

#[derive(Default)]
pub(crate) struct BackplaneRegistry {
    state: RwLock<RegistryState>,
    leases: Mutex<BTreeMap<String, ResourceLease>>,
    /// Exact source snapshot paired with `state`. The mutex stays held through
    /// publication so concurrent old/new refreshes cannot publish out of order.
    source_snapshot: Mutex<Option<Arc<Vec<RouteChoice>>>>,
    publications: AtomicU64,
}

impl BackplaneRegistry {
    pub(crate) fn from_bag(bag: &Bag) -> Self {
        let registry = Self::default();
        registry.refresh_from_bag(bag);
        registry
    }

    /// Publish the Bag's routing graph only when its exact source snapshot
    /// changes. Returns whether a new graph was published.
    pub(crate) fn refresh_from_bag(&self, bag: &Bag) -> bool {
        let choices = bag.route_choices();
        let Ok(mut published) = self.source_snapshot.lock() else {
            return false;
        };
        if published.as_ref() == Some(&choices) {
            return false;
        }
        let mut next = RegistryState::default();
        for choice in choices.iter() {
            let surface_id = SurfaceId::new(SurfaceProtocol::Chat, &choice.agent);
            next.surfaces
                .entry(surface_id.clone())
                .or_insert_with(|| SurfaceDescriptor {
                    surface_id: surface_id.clone(),
                    label: choice.agent.clone(),
                    protocol: SurfaceProtocol::Chat,
                    capabilities: ["chat", "tools"].into_iter().map(str::to_string).collect(),
                    availability: if choice.available {
                        Availability::Available
                    } else {
                        Availability::Unavailable
                    },
                    cost_class: if crate::agent::club::is_sota_label(&choice.driver) {
                        "metered".to_string()
                    } else {
                        "local".to_string()
                    },
                    provenance: "bag-discovery".to_string(),
                    resource_group: choice.agent.clone(),
                });
            let route_id = RouteId::chat(
                &choice.agent,
                &choice.driver,
                choice.reasoning_effort.as_deref(),
            );
            let mut roles = BTreeSet::from([WorkloadRole::Foreground, WorkloadRole::MoaSeat]);
            if parameter_billions(&choice.model).unwrap_or(0) >= 30 {
                roles.insert(WorkloadRole::Teacher);
                roles.insert(WorkloadRole::Evaluator);
            }
            let descriptor = RouteDescriptor {
                route_id: route_id.clone(),
                surface_id: surface_id.clone(),
                driver: choice.driver.clone(),
                invocation_profile: format!(
                    "chat:{}",
                    choice.reasoning_effort.as_deref().unwrap_or("model-native")
                ),
                model_revision: ModelRevision::chat(&choice.model),
                model: choice.model.clone(),
                roles,
                context_window: choice.metadata.context_window,
                output_policy: choice
                    .metadata
                    .output_budget
                    .label(choice.metadata.output_budget_provenance.as_deref()),
            };
            for alias in [&choice.agent, &choice.driver, &choice.model] {
                next.aliases
                    .insert(alias.to_ascii_lowercase(), route_id.clone());
            }
            next.routes.insert(route_id, descriptor);
        }
        add_head_descriptors(&mut next);
        let Ok(mut state) = self.state.write() else {
            return false;
        };
        *state = next;
        *published = Some(choices);
        self.publications.fetch_add(1, Ordering::Relaxed);
        true
    }

    #[cfg(test)]
    pub(crate) fn publication_count(&self) -> u64 {
        self.publications.load(Ordering::Relaxed)
    }

    pub(crate) fn surfaces(&self) -> Vec<SurfaceDescriptor> {
        self.state
            .read()
            .map(|state| state.surfaces.values().cloned().collect())
            .unwrap_or_default()
    }

    pub(crate) fn routes(&self) -> Vec<RouteDescriptor> {
        self.state
            .read()
            .map(|state| state.routes.values().cloned().collect())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub(crate) fn route(&self, id: &RouteId) -> Option<RouteDescriptor> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.routes.get(id).cloned())
    }

    pub(crate) fn resolve_identity(&self, identity: &RouteIdentity) -> Option<RouteDescriptor> {
        let state = self.state.read().ok()?;
        let invocation_profile = format!(
            "chat:{}",
            identity
                .reasoning_effort
                .as_deref()
                .unwrap_or("model-native")
        );
        let mut candidates = state.routes.values().filter(|route| {
            route.roles.contains(&WorkloadRole::Foreground)
                && route.driver.eq_ignore_ascii_case(&identity.driver)
                && route.invocation_profile == invocation_profile
                && identity
                    .model
                    .as_deref()
                    .is_none_or(|model| route.model.eq_ignore_ascii_case(model))
        });
        if let Some(route) = candidates.next() {
            let route = route.clone();
            if candidates.next().is_none() {
                return Some(route);
            }
            return None;
        }

        // Legacy identities occasionally used a model name as their driver. A
        // model-only fallback is safe only when it identifies exactly one live
        // foreground route; same-named models on different surfaces must never
        // silently collapse to whichever descriptor discovery visited last.
        let model = identity.model.as_deref().unwrap_or(&identity.driver);
        let mut model_matches = state.routes.values().filter(|route| {
            route.roles.contains(&WorkloadRole::Foreground)
                && route.model.eq_ignore_ascii_case(model)
                && route.invocation_profile == invocation_profile
        });
        let only = model_matches.next()?.clone();
        if model_matches.next().is_none() {
            Some(only)
        } else {
            None
        }
    }

    pub(crate) fn resource_group_for_identity(
        &self,
        identity: &RouteIdentity,
    ) -> Option<(RouteId, String)> {
        let route = self.resolve_identity(identity)?;
        let state = self.state.read().ok()?;
        let surface = state.surfaces.get(&route.surface_id)?;
        Some((route.route_id, surface.resource_group.clone()))
    }

    pub(crate) fn acquire(
        &self,
        resource_group: &str,
        mode: LeaseMode,
        role: WorkloadRole,
        route_id: Option<RouteId>,
        operator_confirmed: bool,
    ) -> Result<ResourceLease, String> {
        if mode == LeaseMode::Train && (role != WorkloadRole::Trainer || !operator_confirmed) {
            return Err(
                "training leases require an explicit operator-confirmed trainer action".to_string(),
            );
        }
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| "resource lease registry is unavailable".to_string())?;
        if let Some(existing) = leases.get(resource_group) {
            if role == WorkloadRole::Foreground
                && existing.mode == LeaseMode::Serve
                && existing.role != WorkloadRole::Foreground
            {
                existing.revoked.store(true, Ordering::Relaxed);
                leases.remove(resource_group);
            } else {
                return Err(format!(
                    "resource {resource_group} is already {:?} for {:?}",
                    existing.mode, existing.role
                ));
            }
        }
        let route = route_id
            .as_ref()
            .map(|id| id.0.as_str())
            .unwrap_or("unbound");
        let lease = ResourceLease {
            id: stable_id(
                "lease",
                &[
                    resource_group,
                    match mode {
                        LeaseMode::Serve => "serve",
                        LeaseMode::Train => "train",
                    },
                    role_label(role),
                    route,
                    &now_ms().to_string(),
                ],
            ),
            resource_group: resource_group.to_string(),
            mode,
            role,
            route_id,
            operator_confirmed,
            revoked: Arc::new(AtomicBool::new(false)),
        };
        leases.insert(resource_group.to_string(), lease.clone());
        Ok(lease)
    }

    /// Acquire a lease whose inverse travels with the acquisition. Use this for
    /// process-local work so a failed or panicking worker cannot strand the
    /// resource in the registry.
    pub(crate) fn acquire_scoped(
        self: &Arc<Self>,
        resource_group: &str,
        mode: LeaseMode,
        role: WorkloadRole,
        route_id: Option<RouteId>,
        operator_confirmed: bool,
    ) -> Result<ResourceLeaseGuard, String> {
        let lease = self.acquire(resource_group, mode, role, route_id, operator_confirmed)?;
        Ok(ResourceLeaseGuard {
            registry: Arc::clone(self),
            lease,
        })
    }

    pub(crate) fn release(&self, lease: &ResourceLease) {
        if let Ok(mut leases) = self.leases.lock()
            && leases
                .get(&lease.resource_group)
                .is_some_and(|active| active.id == lease.id)
        {
            leases.remove(&lease.resource_group);
        }
    }

    pub(crate) fn leases(&self) -> Vec<ResourceLease> {
        self.leases
            .lock()
            .map(|leases| leases.values().cloned().collect())
            .unwrap_or_default()
    }
}

fn role_label(role: WorkloadRole) -> &'static str {
    match role {
        WorkloadRole::Foreground => "foreground",
        WorkloadRole::MoaSeat => "moa-seat",
        WorkloadRole::Teacher => "teacher",
        WorkloadRole::Clerk => "clerk",
        WorkloadRole::Evaluator => "evaluator",
        WorkloadRole::Trainer => "trainer",
    }
}

pub(crate) fn parameter_billions(model: &str) -> Option<u32> {
    let lower = model.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for index in 1..bytes.len() {
        if bytes[index] == b'b' && bytes[index - 1].is_ascii_digit() {
            let mut start = index - 1;
            while start > 0 && bytes[start - 1].is_ascii_digit() {
                start -= 1;
            }
            if let Ok(value) = lower[start..index].parse() {
                return Some(value);
            }
        }
    }
    None
}

fn add_head_descriptors(state: &mut RegistryState) {
    let Some((raw, provenance)) = configured_head_registry() else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };
    let Some(heads) = value.get("heads").and_then(serde_json::Value::as_array) else {
        return;
    };
    for head in heads {
        let Some(id) = head.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let label = head
            .get("label")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(id);
        let host = head
            .get("rig")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(id);
        let surface_id = SurfaceId::new(SurfaceProtocol::AsyncJob, &format!("{host}:{id}"));
        let capabilities = head
            .get("capabilities")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        state.surfaces.insert(
            surface_id.clone(),
            SurfaceDescriptor {
                surface_id: surface_id.clone(),
                label: label.to_string(),
                protocol: SurfaceProtocol::AsyncJob,
                capabilities,
                availability: Availability::Unknown,
                cost_class: head
                    .get("load_policy")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                provenance: provenance.clone(),
                resource_group: host.to_string(),
            },
        );
        let submit = head
            .get("submit_route")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("submit");
        let route_id = RouteId::async_head(&surface_id, submit);
        let model = head
            .get("model")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(label);
        let roles = head
            .get("roles")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .filter_map(|role| match role {
                "train" | "distill" => Some(WorkloadRole::Trainer),
                _ => None,
            })
            .collect();
        state.routes.insert(
            route_id.clone(),
            RouteDescriptor {
                route_id: route_id.clone(),
                surface_id,
                driver: id.to_string(),
                invocation_profile: format!("async:{submit}"),
                model_revision: ModelRevision::chat(model),
                model: model.to_string(),
                roles,
                context_window: None,
                output_policy: "endpoint-managed".to_string(),
            },
        );
        state.aliases.insert(id.to_ascii_lowercase(), route_id);
    }
}

/// Load specialist async heads only when an operator explicitly names a
/// registry. Public/source builds must not compile a developer's live fleet
/// inventory, private routes, or machine topology into the binary.
fn configured_head_registry() -> Option<(String, String)> {
    if let Some(path) = std::env::var_os("ANGEL_HEADS_FILE").filter(|path| !path.is_empty()) {
        let path = std::path::PathBuf::from(path);
        let raw = std::fs::read_to_string(&path).ok()?;
        return Some((raw, format!("file:{}", path.display())));
    }

    #[cfg(test)]
    {
        Some((
            r#"{"heads":[{"id":"fixture-head","label":"Fixture head","rig":"loopback","model":"fixture-model","capabilities":["test"],"roles":["evaluate"],"submit_route":"submit","load_policy":"local"}]}"#
                .to_string(),
            "test-fixture".to_string(),
        ))
    }

    #[cfg(not(test))]
    {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KnowledgeAuthority {
    OperatorApproved,
    ReviewedProject,
    VerifiedDossier,
    Episodic,
    ReviewedShared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KnowledgeLifecycle {
    Active,
    Proposed,
    Rejected,
    Stale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PromptPolicy {
    TaskContext,
    Never,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct KnowledgeCandidate {
    pub(crate) source_id: String,
    pub(crate) project_key: String,
    pub(crate) kind: String,
    pub(crate) lifecycle: KnowledgeLifecycle,
    pub(crate) epistemic_state: String,
    pub(crate) authority: KnowledgeAuthority,
    pub(crate) freshness: f32,
    pub(crate) independent: bool,
    pub(crate) digest: String,
    pub(crate) prompt_policy: PromptPolicy,
    /// Reviewed evidence payload. Clones share the buffer with `ChatMsg.content`
    /// and operator memory slots instead of copying long notes on the UI tick.
    pub(crate) content: Arc<str>,
}

impl KnowledgeCandidate {
    pub(crate) fn new(
        source_id: impl Into<String>,
        project_key: impl Into<String>,
        kind: impl Into<String>,
        authority: KnowledgeAuthority,
        content: impl Into<Arc<str>>,
    ) -> Self {
        let content = content.into();
        Self {
            source_id: source_id.into(),
            project_key: project_key.into(),
            kind: kind.into(),
            lifecycle: KnowledgeLifecycle::Active,
            epistemic_state: "reviewed".to_string(),
            authority,
            freshness: 1.0,
            independent: true,
            digest: crate::knowledge::cut::sha256_hex(normalize_content(&content).as_bytes()),
            prompt_policy: PromptPolicy::TaskContext,
            content,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BrokerSelection {
    pub(crate) block: Option<String>,
    pub(crate) source_ids: Vec<String>,
    pub(crate) source_digests: Vec<String>,
    pub(crate) omitted: usize,
    pub(crate) tokens: usize,
}

pub(crate) struct KnowledgeBroker;

impl KnowledgeBroker {
    pub(crate) fn select(
        project_key: &str,
        query: &str,
        mut candidates: Vec<KnowledgeCandidate>,
        route_remaining_tokens: usize,
    ) -> BrokerSelection {
        let allowance = route_remaining_tokens
            .saturating_sub(BROKER_SAFETY_TOKENS)
            .min(RECALL_TOKEN_CEILING);
        if allowance == 0 {
            return BrokerSelection {
                omitted: candidates.len(),
                ..BrokerSelection::default()
            };
        }
        candidates.retain(|candidate| {
            candidate.project_key == project_key
                && candidate.lifecycle == KnowledgeLifecycle::Active
                && candidate.prompt_policy == PromptPolicy::TaskContext
                && candidate.freshness > 0.0
                && !candidate.content.trim().is_empty()
                && candidate.content.len() <= 16 * 1024
        });
        let query_terms = terms(query);
        let mut ranked: Vec<(usize, KnowledgeCandidate)> = candidates
            .into_iter()
            .map(|candidate| (relevance(&query_terms, &candidate.content), candidate))
            .collect();
        ranked.sort_by(|(relevance_a, a), (relevance_b, b)| {
            a.authority
                .cmp(&b.authority)
                .then_with(|| relevance_b.cmp(relevance_a))
                .then_with(|| b.freshness.total_cmp(&a.freshness))
                .then_with(|| a.source_id.cmp(&b.source_id))
        });
        let original_len = ranked.len();
        let mut seen = HashSet::with_capacity(original_len);
        ranked.retain(|(_, candidate)| seen.insert(candidate.digest.clone()));

        let max_bytes = allowance.saturating_mul(4);
        let mut block = format!("{BROKER_HEADER}\n");
        let mut source_ids = Vec::new();
        let mut source_digests = Vec::new();
        for (_, candidate) in ranked {
            let content = crate::knowledge::evidence::fence(
                candidate.source_id.split(':').next().unwrap_or("memory"),
                &format!(
                    "repo={} age=unknown verification=store-claimed",
                    project_key
                ),
                &candidate.content,
            );
            let item = format!(
                "[source id={} kind={} authority={} digest={}]\n{}\n[/source]\n",
                safe_attr(&candidate.source_id),
                safe_attr(&candidate.kind),
                authority_label(candidate.authority),
                safe_attr(&candidate.digest),
                content
            );
            if block.len() + item.len() + BROKER_SENTINEL.len() + 1 > max_bytes {
                break;
            }
            block.push_str(&item);
            source_ids.push(candidate.source_id);
            source_digests.push(candidate.digest);
        }
        if source_ids.is_empty() {
            return BrokerSelection {
                omitted: original_len,
                ..BrokerSelection::default()
            };
        }
        block.push_str(BROKER_SENTINEL);
        let tokens = block.chars().count().div_ceil(4);
        BrokerSelection {
            block: Some(block),
            omitted: original_len.saturating_sub(source_ids.len()),
            source_ids,
            source_digests,
            tokens,
        }
    }

    /// Swap the broker's own tail block (and any atlas lens block) for the new
    /// selection. Deliberately bounded: the broker only ever deletes messages it
    /// manages, which live at the previous turn's tail — so a provider's prefix
    /// cache survives up to that point. Compaction notes and auto-recall notes
    /// are NOT touched here: they are static once written, sit near the head of
    /// the conversation, and deleting them each turn re-billed the entire
    /// cached prefix on every submit (their owners — the compactor and the
    /// recall pass — still consolidate them on their own schedules).
    pub(crate) fn replace(history: &mut Vec<ChatMsg>, selection: &BrokerSelection) {
        history.retain(|message| {
            let broker_managed = is_broker_message(&message.content)
                || crate::knowledge::atlas::is_lens_message(&message.content);
            // Persisted generated broker/lens blocks retain Harness role.
            // A legacy marker in any other role is unowned conversation data;
            // text alone cannot authorize removal or migrate its provenance.
            message.role != crate::agent::club::ChatRole::Harness || !broker_managed
        });
        if let Some(block) = selection.block.as_ref() {
            history.push(ChatMsg::harness(block.clone()));
        }
    }

    /// Turn-start refresh must not remove evidence from a prefix already sent
    /// to the provider. Append changed selections; identical or empty selections
    /// leave the existing bytes intact until a compaction/explicit replacement.
    pub(crate) fn append(history: &mut Vec<ChatMsg>, selection: &BrokerSelection) {
        let Some(block) = selection.block.as_ref() else {
            return;
        };
        let previous = history.iter().rev().find(|message| {
            message.role == crate::agent::club::ChatRole::Harness
                && is_broker_message(&message.content)
        });
        if previous.is_some_and(|message| message.content.as_ref() == block) {
            return;
        }
        history.push(ChatMsg::harness(block.clone()));
    }

    pub(crate) fn existing_candidates(
        history: &[ChatMsg],
        project_key: &str,
    ) -> Vec<KnowledgeCandidate> {
        history
            .iter()
            // Older blocks remain serialized cache history, not current input
            // to reselection; otherwise superseded facts can be resurrected.
            .rev()
            .find(|message| {
                message.role == crate::agent::club::ChatRole::Harness
                    && is_broker_message(&message.content)
            })
            .into_iter()
            .flat_map(|message| parse_broker_candidates(&message.content, project_key))
            .collect()
    }
}

pub(crate) fn is_broker_message(content: &str) -> bool {
    content.starts_with(BROKER_HEADER)
}

pub(crate) fn atlas_lens_candidates(lens: &str, project_key: &str) -> Vec<KnowledgeCandidate> {
    // Rendered item boundaries are distinct from recalled text: the lens
    // neutralizes in-content list markers. Preserve multiline evidence.
    lens.split("\n- [")
        .skip(1)
        .filter_map(|item| {
            let (metadata, body) = item.split_once("] ")?;
            let id = metadata.split('·').next()?.trim();
            if id.is_empty() {
                return None;
            }
            let content = body
                .rsplit_once(" (why selected:")
                .map(|(content, _)| content)
                .unwrap_or(body)
                .trim();
            if content.is_empty() {
                return None;
            }
            let observed = metadata
                .split('·')
                .nth(2)
                .is_some_and(|state| state.trim() == "observed");
            let authority = if observed {
                KnowledgeAuthority::Episodic
            } else if id.starts_with("shr_") {
                KnowledgeAuthority::ReviewedShared
            } else {
                KnowledgeAuthority::ReviewedProject
            };
            let mut candidate = KnowledgeCandidate::new(
                format!("atlas:{id}"),
                project_key,
                if observed {
                    "source-observation"
                } else {
                    "reviewed-atlas"
                },
                authority,
                format!(
                    "Atlas epistemic status: {}\n{content}",
                    metadata.split('·').nth(2).unwrap_or("asserted").trim()
                ),
            );
            candidate.epistemic_state = metadata
                .split('·')
                .nth(2)
                .unwrap_or("asserted")
                .trim()
                .to_string();
            Some(candidate)
        })
        .collect()
}

pub(crate) fn selected_context(history: &[ChatMsg]) -> (Vec<String>, Vec<String>) {
    let mut ids = Vec::new();
    let mut digests = Vec::new();
    for message in history.iter().filter(|message| {
        message.role == crate::agent::club::ChatRole::Harness && is_broker_message(&message.content)
    }) {
        for line in message.content.lines() {
            let Some(attributes) = line
                .strip_prefix("[source ")
                .and_then(|line| line.strip_suffix(']'))
            else {
                continue;
            };
            for attribute in attributes.split_ascii_whitespace() {
                if let Some(value) = attribute.strip_prefix("id=") {
                    ids.push(value.to_string());
                } else if let Some(value) = attribute.strip_prefix("digest=") {
                    digests.push(value.to_string());
                }
            }
        }
    }
    if ids.is_empty() {
        // Legacy and shadow modes keep their historical prompt layout. Attribute
        // those actual inputs too, without copying any prompt prose into the
        // outcome, so the off-control remains observable rather than appearing
        // to have used no learned context.
        for (index, message) in history.iter().enumerate() {
            if let Some(memory) = delimited_block(
                &message.content,
                crate::knowledge::memory::MEMORY_BLOCK_HEADER,
                crate::knowledge::memory::MEMORY_BLOCK_SENTINEL,
            ) {
                push_context_receipt(
                    &mut ids,
                    &mut digests,
                    format!("memory:legacy:{index}"),
                    memory,
                );
            }
            if message
                .content
                .starts_with(crate::knowledge::dossier::DOSSIER_BLOCK_HEADER)
            {
                push_context_receipt(
                    &mut ids,
                    &mut digests,
                    format!("dossier:legacy:{index}"),
                    &message.content,
                );
            } else if message
                .content
                .starts_with(crate::agent::harness::AUTO_RECALL_NOTE_PREFIX)
            {
                push_context_receipt(
                    &mut ids,
                    &mut digests,
                    format!("palace:legacy:{index}"),
                    &message.content,
                );
            } else if message
                .content
                .trim_start()
                .starts_with(crate::agent::compaction::COMPACTION_NOTE_HEADER)
            {
                push_context_receipt(
                    &mut ids,
                    &mut digests,
                    format!("session:compaction:{index}"),
                    &message.content,
                );
            }
            if crate::knowledge::atlas::is_lens_message(&message.content) {
                for candidate in atlas_lens_candidates(&message.content, "legacy") {
                    push_context_receipt(
                        &mut ids,
                        &mut digests,
                        candidate.source_id,
                        &candidate.content,
                    );
                }
            }
        }
    }
    ids.truncate(MAX_OUTCOME_ITEMS);
    digests.truncate(MAX_OUTCOME_ITEMS);
    (ids, digests)
}

fn delimited_block<'a>(content: &'a str, header: &str, sentinel: &str) -> Option<&'a str> {
    let start = content.find(header)?;
    let tail = &content[start..];
    let end = tail.find(sentinel)? + sentinel.len();
    Some(&tail[..end])
}

fn push_context_receipt(
    ids: &mut Vec<String>,
    digests: &mut Vec<String>,
    id: String,
    content: &str,
) {
    if ids.len() >= MAX_OUTCOME_ITEMS || ids.iter().any(|existing| existing == &id) {
        return;
    }
    ids.push(id);
    digests.push(crate::knowledge::cut::sha256_hex(content.as_bytes()));
}

fn parse_broker_candidates(block: &str, project_key: &str) -> Vec<KnowledgeCandidate> {
    let mut candidates = Vec::new();
    let mut lines = block.lines();
    while let Some(line) = lines.next() {
        let Some(attributes) = line
            .strip_prefix("[source ")
            .and_then(|line| line.strip_suffix(']'))
        else {
            continue;
        };
        let mut id = "";
        let mut kind = "context";
        let mut authority = KnowledgeAuthority::ReviewedShared;
        let mut digest = "";
        for attribute in attributes.split_ascii_whitespace() {
            if let Some(value) = attribute.strip_prefix("id=") {
                id = value;
            } else if let Some(value) = attribute.strip_prefix("kind=") {
                kind = value;
            } else if let Some(value) = attribute.strip_prefix("authority=") {
                authority = parse_authority(value);
            } else if let Some(value) = attribute.strip_prefix("digest=") {
                digest = value;
            }
        }
        let mut content = String::new();
        for content_line in lines.by_ref() {
            if content_line == "[/source]" {
                break;
            }
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(content_line);
        }
        if id.is_empty() || content.trim().is_empty() {
            continue;
        }
        let mut candidate = KnowledgeCandidate::new(
            id,
            project_key,
            kind,
            authority,
            crate::knowledge::evidence::unfence(&content),
        );
        if !digest.is_empty() {
            candidate.digest = digest.to_string();
        }
        candidates.push(candidate);
    }
    candidates
}

fn safe_attr(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':')
        })
        .take(160)
        .collect()
}

fn normalize_content(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn terms(value: &str) -> HashSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .map(str::to_ascii_lowercase)
        .filter(|term| term.len() >= 2)
        .collect()
}

fn relevance(query: &HashSet<String>, content: &str) -> usize {
    terms(content).intersection(query).count()
}

fn authority_label(authority: KnowledgeAuthority) -> &'static str {
    match authority {
        KnowledgeAuthority::OperatorApproved => "operator",
        KnowledgeAuthority::ReviewedProject => "project",
        KnowledgeAuthority::VerifiedDossier => "dossier",
        KnowledgeAuthority::Episodic => "episodic",
        KnowledgeAuthority::ReviewedShared => "shared",
    }
}

fn parse_authority(value: &str) -> KnowledgeAuthority {
    match value {
        "operator" => KnowledgeAuthority::OperatorApproved,
        "project" => KnowledgeAuthority::ReviewedProject,
        "dossier" => KnowledgeAuthority::VerifiedDossier,
        "episodic" => KnowledgeAuthority::Episodic,
        _ => KnowledgeAuthority::ReviewedShared,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InfluenceFlags {
    pub(crate) operator_memory: bool,
    pub(crate) atlas: bool,
    pub(crate) dossier: bool,
    pub(crate) episodic: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TurnOutcome {
    pub(crate) schema: String,
    pub(crate) project_key: String,
    pub(crate) session_id: String,
    pub(crate) requested_route_id: Option<RouteId>,
    pub(crate) requested_model_revision: Option<ModelRevision>,
    pub(crate) resolved_route_id: Option<RouteId>,
    pub(crate) resolved_model_revision: Option<ModelRevision>,
    pub(crate) policy_revision: String,
    pub(crate) context_source_ids: Vec<String>,
    pub(crate) influence: InfluenceFlags,
    pub(crate) verifier_receipts: Vec<String>,
    pub(crate) reward: Option<f32>,
    pub(crate) stop_reason: String,
    pub(crate) source_digests: Vec<String>,
}

impl TurnOutcome {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn settled(
        project_key: impl Into<String>,
        session_id: impl Into<String>,
        registry: &BackplaneRegistry,
        requested: &RouteIdentity,
        resolved: &RouteIdentity,
        history: &[ChatMsg],
        verifier_receipts: Vec<String>,
        reward: Option<f32>,
        stop_reason: &str,
    ) -> Self {
        let requested = registry.resolve_identity(requested);
        let resolved = registry.resolve_identity(resolved);
        let (context_source_ids, source_digests) = selected_context(history);
        let influence = InfluenceFlags {
            operator_memory: context_source_ids
                .iter()
                .any(|id| id.starts_with("memory:")),
            atlas: context_source_ids.iter().any(|id| id.starts_with("atlas:")),
            dossier: context_source_ids
                .iter()
                .any(|id| id.starts_with("dossier:")),
            episodic: context_source_ids
                .iter()
                .any(|id| id.starts_with("session:") || id.starts_with("palace:")),
        };
        Self {
            schema: "angel-turn-outcome/v1".to_string(),
            project_key: bounded(project_key.into(), MAX_OUTCOME_TEXT),
            session_id: bounded(session_id.into(), MAX_OUTCOME_TEXT),
            requested_route_id: requested.as_ref().map(|route| route.route_id.clone()),
            requested_model_revision: requested.as_ref().map(|route| route.model_revision.clone()),
            resolved_route_id: resolved.as_ref().map(|route| route.route_id.clone()),
            resolved_model_revision: resolved.map(|route| route.model_revision),
            policy_revision: stable_id("policy", &["ordinary-cockpit", env!("CARGO_PKG_VERSION")]),
            context_source_ids,
            influence,
            verifier_receipts: verifier_receipts
                .into_iter()
                .map(safe_receipt)
                .take(MAX_OUTCOME_ITEMS)
                .collect(),
            reward: reward.filter(|value| value.is_finite()),
            stop_reason: bounded(stop_reason.to_string(), 64),
            source_digests,
        }
    }
}

fn bounded(mut value: String, max: usize) -> String {
    if value.len() <= max {
        return value;
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value
}

fn safe_receipt(receipt: String) -> String {
    let compact = receipt.split_whitespace().collect::<Vec<_>>().join(" ");
    let lowercase = compact.to_ascii_lowercase();
    if [
        "authorization:",
        "password=",
        "passwd=",
        "api_key=",
        "api-key=",
        "access_token=",
        "bearer ",
    ]
    .iter()
    .any(|marker| lowercase.contains(marker))
    {
        return format!(
            "redacted:sha256:{}",
            crate::knowledge::cut::sha256_hex(compact.as_bytes())
        );
    }
    bounded(compact, MAX_OUTCOME_TEXT)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct EvidenceHeader {
    pub(crate) id: String,
    pub(crate) created_ms: u64,
    pub(crate) producer: String,
    pub(crate) evidence_digests: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct KnowledgeProposal {
    pub(crate) evidence: EvidenceHeader,
    pub(crate) project_key: String,
    pub(crate) content_digest: String,
    pub(crate) review_state: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct PolicyCandidate {
    pub(crate) evidence: EvidenceHeader,
    pub(crate) policy_digest: String,
    pub(crate) held_out_receipts: Vec<String>,
    pub(crate) promoted: bool,
    pub(crate) rollback_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub(crate) enum AdapterFamily {
    CodingAgent,
    AtlasClerk,
    UtilityJudge,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BehaviorGate {
    pub(crate) name: String,
    pub(crate) score: f32,
    pub(crate) minimum: f32,
    pub(crate) receipt: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct AdapterCandidate {
    pub(crate) evidence: EvidenceHeader,
    pub(crate) family: AdapterFamily,
    pub(crate) exact_base_fingerprint: String,
    pub(crate) adapter_digest: String,
    pub(crate) evaluation_receipts: Vec<String>,
    pub(crate) serving_compatibility: Vec<ModelRevision>,
    pub(crate) behavior_gates: Vec<BehaviorGate>,
}

impl AdapterCandidate {
    #[allow(dead_code)]
    pub(crate) fn promotable_for(
        &self,
        serving_base_fingerprint: &str,
        serving_revision: &ModelRevision,
    ) -> Result<(), String> {
        if self.exact_base_fingerprint != serving_base_fingerprint {
            return Err("adapter base fingerprint does not exactly match serving base".to_string());
        }
        if !self.serving_compatibility.contains(serving_revision) {
            return Err(
                "adapter is not declared compatible with this serving revision".to_string(),
            );
        }
        if self.behavior_gates.is_empty()
            || self
                .behavior_gates
                .iter()
                .any(|gate| !gate.score.is_finite() || gate.score < gate.minimum)
        {
            return Err("family-specific held-out behavior gates did not pass".to_string());
        }
        Ok(())
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/backplane__tests.rs"]
mod tests;

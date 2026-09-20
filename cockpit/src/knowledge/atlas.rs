//! Living Atlas: project-bound reviewed knowledge for the ordinary cockpit.
//!
//! Existing memories, dossiers, sessions, recall drawers, and artifacts remain
//! canonical. Atlas persists only its own proposals, reviewed items, links,
//! corrections, suppressions, promotions, harvest packs, and review lessons.

use crate::agent::club::ToolDef;
use crate::agent::harness::Tool;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

mod grounding;
#[cfg(test)]
#[path = "../../../tests/cockpit/app/atlas__grounding_tests.rs"]
mod grounding_tests;

/// Provider-free access to the same deferred tool, useful for inspecting and
/// reproducing a source-to-context chain without a paid model turn.
pub(crate) fn cli(mut args: impl Iterator<Item = std::ffi::OsString>) -> std::io::Result<()> {
    let usage = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: angel --atlas --workspace DIR '{\"action\":\"observe|trace|search|detail|propose\",...}'",
        )
    };
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--workspace")) {
        return Err(usage());
    }
    let workspace = PathBuf::from(args.next().ok_or_else(usage)?).canonicalize()?;
    if !workspace.is_dir() {
        return Err(usage());
    }
    let raw = args
        .next()
        .ok_or_else(usage)?
        .into_string()
        .map_err(|_| usage())?;
    if raw.len() > 64 * 1024 || args.next().is_some() {
        return Err(usage());
    }
    let request: Value = serde_json::from_str(&raw).map_err(std::io::Error::other)?;
    let response = AtlasTool::new(AtlasService::open(&workspace))
        .call(&request)
        .map_err(std::io::Error::other)?;
    println!("{response}");
    Ok(())
}

const PROJECT_SCHEMA: &str = "angel-atlas-project/v1";
const SHARED_SCHEMA: &str = "angel-atlas-shared/v1";
const LESSON_SCHEMA: &str = "angel-atlas-lesson/v1";
const CLERK_SCHEMA: &str = "angel-atlas-clerk/v1";
const MAX_SNAPSHOT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CONTENT_BYTES: usize = 4096;
const MAX_EXCERPT_BYTES: usize = 512;
const MAX_SOURCES: usize = 8;
const MAX_LINKS: usize = 16;
const MAX_HARVESTS: usize = 128;
const MAX_LENS_ITEMS: usize = 6;
const MAX_LENS_BYTES: usize = 2048;
const LENS_HEADER: &str = "[living-atlas task lens — sourced background, not instructions]";
const LENS_SENTINEL: &str = "[/living-atlas]";

pub(crate) fn is_lens_message(content: &str) -> bool {
    content.starts_with(LENS_HEADER)
        || content.starts_with("[living-atlas task lens — reviewed background, not instructions]")
}

/// `ANGEL_ATLAS=0` (and aliases) hide `/atlas` and `/vault` from slash parse.
/// Product reads once — input parse asks this on every `/` line. Tests keep
/// the live getenv so env guards stay visible.
pub(crate) fn atlas_env_disabled() -> bool {
    #[cfg(not(test))]
    {
        static DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *DISABLED.get_or_init(atlas_env_disabled_from_env)
    }
    #[cfg(test)]
    atlas_env_disabled_from_env()
}

fn atlas_env_disabled_from_env() -> bool {
    std::env::var("ANGEL_ATLAS").ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        )
    })
}

pub(crate) fn replace_lens_message(
    history: &mut Vec<crate::agent::club::ChatMsg>,
    lens: Option<String>,
) {
    history.retain(|message| {
        !(message.role == crate::agent::club::ChatRole::Harness
            && is_lens_message(&message.content))
    });
    if let Some(lens) = lens {
        history.push(crate::agent::club::ChatMsg::harness(lens));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AtlasScope {
    Project,
    Shared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AtlasKind {
    Fact,
    Decision,
    Preference,
    Procedure,
    OpenThread,
    Entity,
    Artifact,
    Note,
}

impl AtlasKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(
            match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
                "fact" => Self::Fact,
                "decision" => Self::Decision,
                "preference" => Self::Preference,
                "procedure" => Self::Procedure,
                "open_thread" | "thread" | "open" => Self::OpenThread,
                "entity" => Self::Entity,
                "artifact" => Self::Artifact,
                "note" => Self::Note,
                _ => return None,
            },
        )
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::Preference => "preference",
            Self::Procedure => "procedure",
            Self::OpenThread => "open thread",
            Self::Entity => "entity",
            Self::Artifact => "artifact",
            Self::Note => "note",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AtlasLifecycle {
    Proposed,
    Active,
    Rejected,
    Archived,
    Superseded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EpistemicState {
    Asserted,
    Observed,
    Verified,
    Inferred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AtlasAuthority {
    Operator,
    ReviewedModel,
    ImportedEvidence,
    ModelDraft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InjectionPolicy {
    Never,
    TaskLens,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AtlasLinkKind {
    Supports,
    Contradicts,
    Supersedes,
    Related,
    DerivedFrom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlasLink {
    pub(crate) kind: AtlasLinkKind,
    pub(crate) target_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlasSource {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) excerpt: Option<String>,
    #[serde(default = "default_true")]
    pub(crate) independent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) influenced_by: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlasItem {
    pub(crate) id: String,
    pub(crate) scope: AtlasScope,
    pub(crate) kind: AtlasKind,
    pub(crate) lifecycle: AtlasLifecycle,
    pub(crate) epistemic: EpistemicState,
    pub(crate) contested: bool,
    pub(crate) stale: bool,
    pub(crate) authority: AtlasAuthority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) confidence: Option<f32>,
    pub(crate) created_ms: u64,
    pub(crate) updated_ms: u64,
    pub(crate) content: String,
    pub(crate) content_digest: String,
    pub(crate) sources: Vec<AtlasSource>,
    pub(crate) links: Vec<AtlasLink>,
    pub(crate) injection: InjectionPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tombstone {
    item_id: String,
    content_digest: String,
    reason: String,
    created_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PromotionRecord {
    item_id: String,
    item_digest: String,
    project_root: PathBuf,
    project_key: String,
    promoted_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HarvestPack {
    pub(crate) id: String,
    pub(crate) task: String,
    pub(crate) final_answer: String,
    pub(crate) verifier_receipts: Vec<String>,
    pub(crate) source_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) receipt_sources: Vec<AtlasSource>,
    pub(crate) created_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UndoRecord {
    action: String,
    target_id: String,
    before: Vec<AtlasItem>,
    after_ids: Vec<String>,
    tombstones_before: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectSnapshot {
    schema: String,
    project_root: PathBuf,
    project_key: String,
    revision: u64,
    items: Vec<AtlasItem>,
    tombstones: Vec<Tombstone>,
    promotions: Vec<PromotionRecord>,
    harvest_queue: Vec<HarvestPack>,
    #[serde(default)]
    processed_harvests: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    undo: Option<UndoRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedSnapshot {
    schema: String,
    revision: u64,
    items: Vec<AtlasItem>,
    promotions: Vec<PromotionRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AtlasHealth {
    Disabled,
    Healthy,
    Degraded,
    Inert,
}

impl AtlasHealth {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Inert => "inert",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AtlasStatus {
    pub(crate) enabled: bool,
    pub(crate) health: AtlasHealth,
    pub(crate) review_count: usize,
    pub(crate) project_items: usize,
    pub(crate) shared_items: usize,
    pub(crate) detail: Option<String>,
}

enum Loaded<T> {
    Active(T),
    Inert(String),
}

struct ServiceState {
    project: Loaded<ProjectSnapshot>,
    shared: Loaded<SharedSnapshot>,
    /// Bumped by every write to `project`/`shared`; `status()` caches on it.
    generation: u64,
}

/// One service instance is bound to exactly one canonical repository identity.
/// The UI and registry tool share its `Arc`; `/cd` builds a new registry/service.
pub(crate) struct AtlasService {
    enabled: bool,
    workspace: PathBuf,
    project_root: PathBuf,
    project_key: String,
    project_path: PathBuf,
    shared_path: PathBuf,
    lesson_path: PathBuf,
    state: Mutex<ServiceState>,
    /// `(state.generation, status)` memo; locked only while `state` is held.
    status_cache: Mutex<Option<(u64, AtlasStatus)>>,
    suggested_commands: Mutex<HashMap<String, String>>,
    pending_receipts: Mutex<Vec<AtlasSource>>,
}

impl AtlasService {
    pub(crate) fn open(workspace: &Path) -> Arc<Self> {
        let root = std::env::var_os("ANGEL_ATLAS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if cfg!(test) {
                    std::env::temp_dir()
                        .join("angel-atlas-cargo-tests")
                        .join(std::process::id().to_string())
                } else {
                    crate::platform::workspace_store::angel_subdir("atlas")
                }
            });
        Self::open_in(workspace, root)
    }

    pub(crate) fn open_in(workspace: &Path, root: PathBuf) -> Arc<Self> {
        let enabled = env_flag("ANGEL_ATLAS", true);
        let identity = crate::platform::workspace_store::repo_identity(workspace);
        let project_path = root.join("projects").join(format!("{}.json", identity.key));
        let shared_path = root.join("shared.json");
        let lesson_path = root.join("lessons").join(format!("{}.jsonl", identity.key));
        let project = if enabled {
            load_project(&project_path, workspace, &identity.root, &identity.key)
        } else {
            Loaded::Active(empty_project(&identity.root, &identity.key))
        };
        let shared = if enabled {
            load_shared(&shared_path)
        } else {
            Loaded::Active(empty_shared())
        };
        Arc::new(Self {
            enabled,
            workspace: workspace.to_path_buf(),
            project_root: identity.root,
            project_key: identity.key,
            project_path,
            shared_path,
            lesson_path,
            state: Mutex::new(ServiceState {
                project,
                shared,
                generation: 0,
            }),
            status_cache: Mutex::new(None),
            suggested_commands: Mutex::new(HashMap::new()),
            pending_receipts: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn project_key(&self) -> &str {
        &self.project_key
    }

    pub(crate) fn status(&self) -> AtlasStatus {
        if !self.enabled {
            return AtlasStatus {
                enabled: false,
                health: AtlasHealth::Disabled,
                review_count: 0,
                project_items: 0,
                shared_items: 0,
                detail: None,
            };
        }
        let Ok(state) = self.state.lock() else {
            return AtlasStatus {
                enabled: true,
                health: AtlasHealth::Degraded,
                review_count: 0,
                project_items: 0,
                shared_items: 0,
                detail: Some("state lock poisoned".to_string()),
            };
        };
        if let Ok(cache) = self.status_cache.lock()
            && let Some((generation, status)) = cache.as_ref()
            && *generation == state.generation
        {
            return status.clone();
        }
        let (project_items, review_count, project_error) = match &state.project {
            Loaded::Active(snapshot) => (
                snapshot.items.len(),
                snapshot
                    .items
                    .iter()
                    .filter(|item| item.lifecycle == AtlasLifecycle::Proposed)
                    .count(),
                None,
            ),
            Loaded::Inert(error) => (0, 0, Some(error.clone())),
        };
        let (shared_items, shared_error) = match &state.shared {
            Loaded::Active(snapshot) => (snapshot.items.len(), None),
            Loaded::Inert(error) => (0, Some(error.clone())),
        };
        let detail = project_error.or(shared_error);
        let status = AtlasStatus {
            enabled: true,
            health: if detail.is_some() {
                AtlasHealth::Inert
            } else {
                AtlasHealth::Healthy
            },
            review_count,
            project_items,
            shared_items,
            detail,
        };
        if let Ok(mut cache) = self.status_cache.lock() {
            *cache = Some((state.generation, status.clone()));
        }
        status
    }

    pub(crate) fn item(&self, id: &str) -> Option<AtlasItem> {
        let state = self.state.lock().ok()?;
        match &state.project {
            Loaded::Active(snapshot) => {
                if let Some(item) = snapshot.items.iter().find(|item| item.id == id) {
                    return Some(item.clone());
                }
            }
            Loaded::Inert(_) => {}
        }
        match &state.shared {
            Loaded::Active(snapshot) => snapshot.items.iter().find(|item| item.id == id).cloned(),
            Loaded::Inert(_) => None,
        }
    }

    pub(crate) fn list(&self, lane: AtlasLane, query: Option<&str>) -> Vec<AtlasItem> {
        let Some((project, shared)) = self.snapshots() else {
            return Vec::new();
        };
        let query = query.unwrap_or("").trim().to_ascii_lowercase();
        let matches_query = |item: &AtlasItem| {
            query.is_empty()
                || item.content.to_ascii_lowercase().contains(&query)
                || item.id.to_ascii_lowercase().contains(&query)
                || item.kind.label().contains(&query)
        };
        let mut items = match lane {
            AtlasLane::Now => project
                .items
                .into_iter()
                .filter(|item| {
                    item.lifecycle == AtlasLifecycle::Active && !item.contested && !item.stale
                })
                .collect(),
            AtlasLane::Project => project.items,
            AtlasLane::Review => project
                .items
                .into_iter()
                .filter(|item| item.lifecycle == AtlasLifecycle::Proposed)
                .collect(),
            AtlasLane::Shared => shared.items,
            AtlasLane::Artifacts => Vec::new(),
        };
        items.retain(matches_query);
        items.sort_by(|a, b| {
            b.updated_ms
                .cmp(&a.updated_ms)
                .then_with(|| a.id.cmp(&b.id))
        });
        items
    }

    fn snapshots(&self) -> Option<(ProjectSnapshot, SharedSnapshot)> {
        let state = self.state.lock().ok()?;
        let Loaded::Active(project) = &state.project else {
            return None;
        };
        let Loaded::Active(shared) = &state.shared else {
            return None;
        };
        Some((project.clone(), shared.clone()))
    }

    /// Operator-authored additions are active assertions. Model/tool proposals
    /// use [`Self::propose`] and can never activate themselves.
    pub(crate) fn add_operator(&self, kind: AtlasKind, content: &str) -> Result<AtlasItem, String> {
        let source = AtlasSource {
            id: format!("operator:{}", now_ms()),
            kind: "operator".to_string(),
            digest: digest(content),
            excerpt: None,
            independent: true,
            influenced_by: None,
        };
        self.insert_item(
            kind,
            content,
            AtlasLifecycle::Active,
            EpistemicState::Asserted,
            AtlasAuthority::Operator,
            None,
            vec![source],
            Vec::new(),
            InjectionPolicy::TaskLens,
        )
    }

    pub(crate) fn propose(
        &self,
        kind: AtlasKind,
        content: &str,
        confidence: Option<f32>,
        sources: Vec<AtlasSource>,
    ) -> Result<AtlasItem, String> {
        self.propose_linked(kind, content, confidence, sources, Vec::new())
    }

    pub(crate) fn propose_linked(
        &self,
        kind: AtlasKind,
        content: &str,
        confidence: Option<f32>,
        sources: Vec<AtlasSource>,
        links: Vec<AtlasLink>,
    ) -> Result<AtlasItem, String> {
        if !links.is_empty() {
            self.reload_project()?;
            self.check_grounding(
                &links
                    .iter()
                    .map(|link| link.target_id.clone())
                    .collect::<Vec<_>>(),
            )?;
        }
        if sources.is_empty() {
            return Err("Atlas proposals require at least one stable source id".to_string());
        }
        if sources.iter().any(|source| !source.independent)
            && !sources.iter().any(|source| source.independent)
        {
            return Err(
                "Atlas-influenced evidence cannot independently confirm a claim".to_string(),
            );
        }
        self.insert_item(
            kind,
            content,
            AtlasLifecycle::Proposed,
            EpistemicState::Inferred,
            AtlasAuthority::ModelDraft,
            confidence.map(|value| value.clamp(0.0, 1.0)),
            sources,
            links,
            InjectionPolicy::Never,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_item(
        &self,
        kind: AtlasKind,
        content: &str,
        lifecycle: AtlasLifecycle,
        epistemic: EpistemicState,
        authority: AtlasAuthority,
        confidence: Option<f32>,
        sources: Vec<AtlasSource>,
        links: Vec<AtlasLink>,
        injection: InjectionPolicy,
    ) -> Result<AtlasItem, String> {
        self.ensure_enabled()?;
        self.validate_code_sources(&sources)?;
        let content = sanitize_text(content, &self.project_root, MAX_CONTENT_BYTES)?;
        if content.trim().is_empty() {
            return Err("Atlas item content is empty after redaction".to_string());
        }
        let content_digest = digest(&content);
        let now = now_ms();
        static ITEM_SEQ: AtomicU64 = AtomicU64::new(1);
        let seq = ITEM_SEQ.fetch_add(1, AtomicOrdering::Relaxed);
        let item = AtlasItem {
            id: format!("atl_{now:x}_{seq:x}_{}", &content_digest[..8]),
            scope: AtlasScope::Project,
            kind,
            lifecycle,
            epistemic,
            contested: false,
            stale: false,
            authority,
            confidence,
            created_ms: now,
            updated_ms: now,
            content,
            content_digest: content_digest.clone(),
            sources: sanitize_sources(sources, &self.project_root),
            links,
            injection,
        };
        self.mutate_project(|snapshot| {
            grounding::validate_links(snapshot, &item.id, &item.links)?;
            if snapshot
                .tombstones
                .iter()
                .any(|tombstone| tombstone.content_digest == content_digest)
            {
                return Err(
                    "a durable rejection/forgetting tombstone suppresses this content".to_string(),
                );
            }
            if snapshot.items.iter().any(|existing| {
                existing.content_digest == content_digest
                    && !(item.authority == AtlasAuthority::ImportedEvidence && existing.stale)
                    && !matches!(
                        existing.lifecycle,
                        AtlasLifecycle::Rejected
                            | AtlasLifecycle::Archived
                            | AtlasLifecycle::Superseded
                    )
            }) {
                return Err("an equivalent Atlas item already exists".to_string());
            }
            snapshot.undo = Some(UndoRecord {
                action: "add".to_string(),
                target_id: item.id.clone(),
                before: Vec::new(),
                after_ids: vec![item.id.clone()],
                tombstones_before: snapshot.tombstones.len(),
            });
            snapshot.items.push(item.clone());
            Ok(())
        })?;
        Ok(item)
    }

    pub(crate) fn accept(&self, id: &str) -> Result<AtlasItem, String> {
        self.reload_project()?;
        self.check_grounding(&[id.to_string()])?;
        let mut accepted = None;
        self.mutate_project(|snapshot| {
            let index = item_index(snapshot, id)?;
            if snapshot.items[index].lifecycle != AtlasLifecycle::Proposed {
                return Err("only proposed Atlas items can be accepted".to_string());
            }
            if snapshot.items[index].stale || snapshot.items[index].contested {
                return Err("stale or contested proposals need new evidence before review".into());
            }
            let before = snapshot.items[index].clone();
            snapshot.items[index].lifecycle = AtlasLifecycle::Active;
            snapshot.items[index].authority = AtlasAuthority::ReviewedModel;
            snapshot.items[index].injection = InjectionPolicy::TaskLens;
            snapshot.items[index].updated_ms = now_ms();
            snapshot.undo = Some(UndoRecord {
                action: "accept".to_string(),
                target_id: id.to_string(),
                before: vec![before],
                after_ids: vec![id.to_string()],
                tombstones_before: snapshot.tombstones.len(),
            });
            accepted = Some(snapshot.items[index].clone());
            Ok(())
        })?;
        let item = accepted.expect("accepted item set by successful mutation");
        self.write_lesson("accept", id, Some(&item));
        Ok(item)
    }

    pub(crate) fn revise(&self, id: &str, content: &str) -> Result<AtlasItem, String> {
        let revised_content = sanitize_text(content, &self.project_root, MAX_CONTENT_BYTES)?;
        if revised_content.trim().is_empty() {
            return Err("revised Atlas content is empty".to_string());
        }
        let mut revised = None;
        self.mutate_project(|snapshot| {
            let index = item_index(snapshot, id)?;
            if matches!(
                snapshot.items[index].lifecycle,
                AtlasLifecycle::Rejected | AtlasLifecycle::Archived
            ) {
                return Err("rejected or forgotten Atlas items cannot be revised".to_string());
            }
            let before = snapshot.items[index].clone();
            let now = now_ms();
            if before.authority == AtlasAuthority::ImportedEvidence {
                return Err(
                    "source observations must be refreshed with observe, not rewritten".into(),
                );
            }
            self.validate_code_sources(&before.sources)?;
            snapshot.items[index].lifecycle = AtlasLifecycle::Superseded;
            snapshot.items[index].injection = InjectionPolicy::Never;
            snapshot.items[index].updated_ms = now;
            let content_digest = digest(&revised_content);
            if snapshot
                .tombstones
                .iter()
                .any(|tombstone| tombstone.content_digest == content_digest)
            {
                return Err("the revision is suppressed by a durable tombstone".to_string());
            }
            let next = AtlasItem {
                id: format!("atl_{now:x}_{}", &content_digest[..8]),
                scope: AtlasScope::Project,
                kind: before.kind,
                lifecycle: AtlasLifecycle::Active,
                epistemic: EpistemicState::Asserted,
                contested: false,
                stale: false,
                authority: AtlasAuthority::ReviewedModel,
                confidence: before.confidence,
                created_ms: now,
                updated_ms: now,
                content: revised_content.clone(),
                content_digest,
                sources: before.sources.clone(),
                links: before
                    .links
                    .iter()
                    .filter(|link| link.kind != AtlasLinkKind::Supersedes)
                    .take(MAX_LINKS - 1)
                    .cloned()
                    .chain(std::iter::once(AtlasLink {
                        kind: AtlasLinkKind::Supersedes,
                        target_id: id.to_string(),
                    }))
                    .collect(),
                injection: InjectionPolicy::TaskLens,
            };
            snapshot.undo = Some(UndoRecord {
                action: "revise".to_string(),
                target_id: id.to_string(),
                before: vec![before],
                after_ids: vec![id.to_string(), next.id.clone()],
                tombstones_before: snapshot.tombstones.len(),
            });
            snapshot.items.push(next.clone());
            revised = Some(next);
            Ok(())
        })?;
        let item = revised.expect("revised item set by successful mutation");
        self.write_lesson("edit", id, Some(&item));
        Ok(item)
    }

    pub(crate) fn reject(&self, id: &str, reason: &str) -> Result<(), String> {
        self.set_terminal_lifecycle(id, AtlasLifecycle::Rejected, reason)?;
        self.write_lesson("reject", id, None);
        Ok(())
    }

    pub(crate) fn forget(&self, id: &str, reason: &str) -> Result<(), String> {
        self.set_terminal_lifecycle(id, AtlasLifecycle::Archived, reason)
    }

    fn set_terminal_lifecycle(
        &self,
        id: &str,
        lifecycle: AtlasLifecycle,
        reason: &str,
    ) -> Result<(), String> {
        let reason = sanitize_text(reason, &self.project_root, 256)?;
        self.mutate_project(|snapshot| {
            let index = item_index(snapshot, id)?;
            let before = snapshot.items[index].clone();
            snapshot.items[index].lifecycle = lifecycle;
            snapshot.items[index].injection = InjectionPolicy::Never;
            snapshot.items[index].updated_ms = now_ms();
            let tombstones_before = snapshot.tombstones.len();
            snapshot.tombstones.push(Tombstone {
                item_id: id.to_string(),
                content_digest: before.content_digest.clone(),
                reason: if reason.is_empty() {
                    lifecycle_label(lifecycle).to_string()
                } else {
                    reason.clone()
                },
                created_ms: now_ms(),
            });
            snapshot.undo = Some(UndoRecord {
                action: lifecycle_label(lifecycle).to_string(),
                target_id: id.to_string(),
                before: vec![before],
                after_ids: vec![id.to_string()],
                tombstones_before,
            });
            Ok(())
        })
    }

    pub(crate) fn challenge(&self, id: &str, source_id: Option<&str>) -> Result<(), String> {
        self.mutate_project(|snapshot| {
            let index = item_index(snapshot, id)?;
            let before = snapshot.items[index].clone();
            snapshot.items[index].contested = true;
            snapshot.items[index].injection = InjectionPolicy::Never;
            snapshot.items[index].updated_ms = now_ms();
            if let Some(source_id) = source_id {
                snapshot.items[index].links.push(AtlasLink {
                    kind: AtlasLinkKind::Contradicts,
                    target_id: source_id.to_string(),
                });
            }
            snapshot.items[index].links.truncate(MAX_LINKS);
            snapshot.undo = Some(UndoRecord {
                action: "challenge".to_string(),
                target_id: id.to_string(),
                before: vec![before],
                after_ids: vec![id.to_string()],
                tombstones_before: snapshot.tombstones.len(),
            });
            Ok(())
        })
    }

    pub(crate) fn defer(&self, id: &str) -> Result<(), String> {
        let item = self
            .item(id)
            .ok_or_else(|| format!("unknown Atlas item: {id}"))?;
        if item.lifecycle != AtlasLifecycle::Proposed {
            return Err("only proposed items can be deferred".to_string());
        }
        // Deliberately no mutation and no lesson: deferred/unreviewed examples
        // never become training data.
        Ok(())
    }

    pub(crate) fn tend(&self) -> Result<usize, String> {
        let now = now_ms();
        let mut marked = 0usize;
        self.mutate_project(|snapshot| {
            for item in &mut snapshot.items {
                let stale_after_days = if item.kind == AtlasKind::OpenThread {
                    30
                } else {
                    180
                };
                let stale_after_ms = stale_after_days * 86_400_000u64;
                if item.lifecycle == AtlasLifecycle::Active
                    && !item.stale
                    && now.saturating_sub(item.updated_ms) >= stale_after_ms
                {
                    item.stale = true;
                    item.injection = InjectionPolicy::Never;
                    item.updated_ms = now;
                    marked += 1;
                }
            }
            Ok(())
        })?;
        Ok(marked)
    }

    pub(crate) fn undo(&self) -> Result<String, String> {
        let mut action = None;
        let mut target = None;
        self.mutate_project(|snapshot| {
            let undo = snapshot
                .undo
                .take()
                .ok_or_else(|| "there is no Atlas review action to undo".to_string())?;
            snapshot
                .items
                .retain(|item| !undo.after_ids.iter().any(|id| id == &item.id));
            snapshot.items.extend(undo.before.clone());
            snapshot.tombstones.truncate(undo.tombstones_before);
            action = Some(undo.action);
            target = Some(undo.target_id);
            Ok(())
        })?;
        if matches!(
            action.as_deref(),
            Some("accept" | "revise" | "rejected" | "challenge")
        ) {
            // Append-only compensation lets dataset builders discard the prior
            // review row without rewriting a training ledger in place.
            self.write_lesson("undo", target.as_deref().unwrap_or("unknown"), None);
        }
        Ok(format!(
            "undid Atlas {}",
            action.unwrap_or_else(|| "action".to_string())
        ))
    }

    pub(crate) fn promote(
        &self,
        id: &str,
        confirmed_digest: Option<&str>,
    ) -> Result<String, String> {
        self.ensure_enabled()?;
        let item = self
            .item(id)
            .ok_or_else(|| format!("unknown Atlas item: {id}"))?;
        if item.scope != AtlasScope::Project || item.lifecycle != AtlasLifecycle::Active {
            return Err("only active project items can be promoted".to_string());
        }
        if item.contested || item.stale {
            return Err("contested or stale Atlas items cannot be promoted".to_string());
        }
        if confirmed_digest != Some(item.content_digest.as_str()) {
            return Ok(format!(
                "promotion confirmation required · exact content: {:?}\n/atlas promote {} {}",
                item.content, item.id, item.content_digest
            ));
        }
        if !crate::platform::workspace_store::matches_project(
            &self.workspace,
            &self.project_root,
            &self.project_key,
        ) {
            return Err("project identity changed before Atlas promotion".to_string());
        }

        let _project_lock = FileLock::acquire(&lock_path(&self.project_path))?;
        let _shared_lock = FileLock::acquire(&lock_path(&self.shared_path))?;
        let Loaded::Active(mut project) = load_project(
            &self.project_path,
            &self.workspace,
            &self.project_root,
            &self.project_key,
        ) else {
            return Err("project Atlas snapshot is inert; refusing promotion".to_string());
        };
        let Loaded::Active(mut shared) = load_shared(&self.shared_path) else {
            return Err("shared Atlas snapshot is inert; refusing promotion".to_string());
        };
        let current = project
            .items
            .iter()
            .find(|current| current.id == id)
            .ok_or_else(|| "Atlas item disappeared before promotion".to_string())?;
        // Another writer may revoke eligibility without changing the content
        // digest. The locked snapshot, not this handle's cache, owns authority.
        if current.scope != AtlasScope::Project
            || current.lifecycle != AtlasLifecycle::Active
            || current.contested
            || current.stale
        {
            return Err("Atlas item is no longer eligible for promotion".to_string());
        }
        if current.content_digest != item.content_digest
            || current.content_digest != confirmed_digest.unwrap_or_default()
        {
            return Err("Atlas item changed before promotion; reconfirm exact content".to_string());
        }
        if current
            .sources
            .iter()
            .any(|source| source.kind == "workspace-file")
            || current.links.iter().any(|link| {
                matches!(
                    link.kind,
                    AtlasLinkKind::Related | AtlasLinkKind::DerivedFrom
                )
            })
        {
            return Err("source-bound records and project links must stay in their project".into());
        }
        if shared
            .items
            .iter()
            .any(|shared_item| shared_item.content_digest == current.content_digest)
        {
            return Err("equivalent content is already in Shared Atlas".to_string());
        }
        let promoted_ms = now_ms();
        let record = PromotionRecord {
            item_id: current.id.clone(),
            item_digest: current.content_digest.clone(),
            project_root: self.project_root.clone(),
            project_key: self.project_key.clone(),
            promoted_ms,
        };
        let mut shared_item = current.clone();
        shared_item.id = format!("shr_{promoted_ms:x}_{}", &current.content_digest[..8]);
        shared_item.scope = AtlasScope::Shared;
        shared_item.updated_ms = promoted_ms;
        shared_item.links.push(AtlasLink {
            kind: AtlasLinkKind::DerivedFrom,
            target_id: current.id.clone(),
        });
        shared.items.push(shared_item);
        shared.promotions.push(record.clone());
        shared.revision = shared.revision.saturating_add(1);
        write_snapshot(&_shared_lock.directory, &self.shared_path, &shared)?;
        project.promotions.push(record);
        project.revision = project.revision.saturating_add(1);
        write_snapshot(&_project_lock.directory, &self.project_path, &project)?;
        if let Ok(mut state) = self.state.lock() {
            state.project = Loaded::Active(project);
            state.shared = Loaded::Active(shared);
            state.generation = state.generation.wrapping_add(1);
        }
        Ok(format!("promoted {id} to Shared Atlas"))
    }

    /// Build one bounded Harness-role block. Project items always precede
    /// explicitly shared items; confidence affects ranking only.
    ///
    /// `exclusion` is one already-present source at a time. Callers pass
    /// history and memory contents without joining them: `str::contains` on a
    /// concatenated blob can match a needle that straddles message boundaries.
    pub(crate) fn build_lens<'a>(
        &self,
        query: &str,
        exclusion: impl IntoIterator<Item = &'a str>,
    ) -> Option<String> {
        // A replaced/empty/disabled lens must retire prior command attribution.
        if let Ok(mut commands) = self.suggested_commands.lock() {
            commands.clear();
        }
        if !self.enabled || !env_flag("ANGEL_ATLAS_LENS", true) {
            return None;
        }
        self.reload_project().ok()?;
        let query_terms = tokens(query);
        if query_terms.is_empty() {
            return None;
        }
        let exclusion: Vec<&str> = exclusion.into_iter().collect();
        let (project, shared) = self.snapshots()?;
        // Already-visible concepts still seed navigation to unseen evidence.
        // Deduplicate rendered records after traversal and freshness checks.
        let eligible = |item: &&AtlasItem| grounding::eligible(item);
        let project_items = project.items.iter().filter(eligible).collect::<Vec<_>>();
        let shared_items = shared.items.iter().filter(eligible).collect::<Vec<_>>();
        let mut selected = rank_scope(&query_terms, &project_items);
        if env_flag("ANGEL_ATLAS_LINKS", true) {
            selected = grounding::linked_selection(selected, &project_items);
        }
        selected.extend(rank_scope(&query_terms, &shared_items));
        selected.truncate(MAX_LENS_ITEMS);
        self.check_grounding(
            &selected
                .iter()
                .map(|ranked| ranked.item.id.clone())
                .collect::<Vec<_>>(),
        )
        .ok()?;
        let (checked, _) = self.snapshots()?;
        let current_eligible = |candidate: &AtlasItem| {
            candidate.scope == AtlasScope::Shared
                || checked
                    .items
                    .iter()
                    .any(|item| item.id == candidate.id && grounding::eligible(item))
        };
        selected.retain(|ranked| {
            current_eligible(ranked.item)
                && ranked.via.is_none_or(current_eligible)
                && !exclusion
                    .iter()
                    .any(|source| source.contains(&ranked.item.content))
        });
        if selected.is_empty() {
            return None;
        }
        let provenance = format!(
            "repo={} age=unknown verification=per-item-epistemic-state",
            self.project_key
        );
        let mut block = String::new();
        let mut commands = HashMap::new();
        let mut added = 0usize;
        for ranked in selected {
            let why = ranked.matched.join(", ");
            let mut content = ranked.item.content.clone();
            if ranked
                .matched
                .iter()
                .any(|reason| reason.starts_with("link "))
            {
                content.push_str(&format!(
                    "\nRelation: {why} (authored association, not causal proof)"
                ));
            }
            if ranked.item.epistemic == EpistemicState::Inferred {
                content.push_str(
                    "\nEpistemic status: inferred; human review does not verify the claim.",
                );
            }
            let line = format!(
                "- [{} · {} · {}] {} (why selected: {})\n",
                ranked.item.id,
                ranked.item.kind.label(),
                epistemic_label(ranked.item.epistemic),
                crate::knowledge::evidence::neutralize_sentinels(&content)
                    .replace("\n- [", "\n- (·"),
                why
            );
            if LENS_HEADER.len()
                + LENS_SENTINEL.len()
                + 2
                + crate::knowledge::evidence::fence("atlas", &provenance, &format!("{block}{line}"))
                    .len()
                > MAX_LENS_BYTES
            {
                break;
            }
            for command in suggested_commands(&ranked.item.content) {
                commands.insert(command, ranked.item.id.clone());
            }
            block.push_str(&line);
            added += 1;
        }
        if added == 0 {
            return None;
        }
        let block = format!(
            "{LENS_HEADER}{}\n{LENS_SENTINEL}",
            crate::knowledge::evidence::fence("atlas", &provenance, &block)
        );
        if let Ok(mut active) = self.suggested_commands.lock() {
            *active = commands;
        }
        Some(block)
    }

    pub(crate) fn begin_tool_call(&self, name: &str, args: &Value) -> Option<(String, String)> {
        if !self.enabled || name != "shell" {
            return None;
        }
        let command = args.get("command")?.as_str()?.trim();
        let suggestions = self.suggested_commands.lock().ok()?;
        suggestions
            .get(command)
            .map(|item_id| (item_id.clone(), command.to_string()))
    }

    pub(crate) fn record_tool_receipt(
        &self,
        name: &str,
        args: &Value,
        result: &Result<String, String>,
        influence: Option<(String, String)>,
    ) {
        if !self.enabled {
            return;
        }
        let summary = match result {
            Ok(value) => value.lines().next().unwrap_or("ok"),
            Err(error) => error.lines().next().unwrap_or("error"),
        };
        let (independent, influenced_by) = influence
            .map(|(item_id, _)| (false, Some(item_id)))
            .unwrap_or((true, None));
        static RECEIPT_SEQ: AtomicU64 = AtomicU64::new(0);
        let source = AtlasSource {
            id: format!(
                "tool:{}:{}:{}",
                name,
                now_ms(),
                RECEIPT_SEQ.fetch_add(1, AtomicOrdering::Relaxed)
            ),
            kind: "tool-receipt".to_string(),
            digest: digest(&format!("{name}:{args}:{summary}")),
            excerpt: sanitize_text(summary, &self.project_root, 160).ok(),
            independent,
            influenced_by,
        };
        // Receipts are private provenance candidates, not knowledge. Hold them
        // until the settled successful turn creates its one bounded harvest
        // pack. Influenced receipts remain explicitly non-independent.
        if let Ok(mut pending) = self.pending_receipts.lock() {
            pending.push(source);
            if pending.len() > 64 {
                let overflow = pending.len() - 64;
                pending.drain(..overflow);
            }
        }
    }

    pub(crate) fn enqueue_harvest(
        &self,
        task: &str,
        final_answer: &str,
        source_ids: &[String],
        verifier_receipts: &[String],
    ) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        let task = sanitize_text(task, &self.project_root, 2048)?;
        let final_answer = sanitize_text(final_answer, &self.project_root, 4096)?;
        let mut source_ids = source_ids
            .iter()
            .take(16)
            .map(|source| bounded(source, 160))
            .collect::<Vec<_>>();
        let mut verifier_receipts = verifier_receipts
            .iter()
            .take(16)
            .filter_map(|receipt| sanitize_text(receipt, &self.project_root, 256).ok())
            .collect::<Vec<_>>();
        let mut receipt_sources = Vec::new();
        if let Ok(mut pending) = self.pending_receipts.lock() {
            for source in pending.drain(..) {
                if source_ids.len() < 16 {
                    source_ids.push(source.id.clone());
                }
                if verifier_receipts.len() < 16
                    && let Some(receipt) = &source.excerpt
                {
                    verifier_receipts.push(format!(
                        "{}{}",
                        if source.independent {
                            ""
                        } else {
                            "[non-independent Atlas-influenced receipt] "
                        },
                        receipt
                    ));
                }
                if receipt_sources.len() < 16 {
                    receipt_sources.push(source);
                }
            }
        }
        if task.is_empty() && final_answer.is_empty() && verifier_receipts.is_empty() {
            return Ok(());
        }
        let created_ms = now_ms();
        let pack = HarvestPack {
            id: format!("harvest_{created_ms:x}_{}", &digest(&task)[..8]),
            task,
            final_answer,
            verifier_receipts,
            source_ids,
            receipt_sources,
            created_ms,
        };
        self.mutate_project(|snapshot| {
            snapshot.harvest_queue.push(pack);
            if snapshot.harvest_queue.len() > MAX_HARVESTS {
                let drop_count = snapshot.harvest_queue.len() - MAX_HARVESTS;
                snapshot.harvest_queue.drain(..drop_count);
            }
            Ok(())
        })
    }

    pub(crate) fn harvest_queue_status(&self) -> (usize, Option<u64>) {
        let Ok(state) = self.state.lock() else {
            return (0, None);
        };
        let Loaded::Active(project) = &state.project else {
            return (0, None);
        };
        (
            project.harvest_queue.len(),
            project.harvest_queue.first().map(|pack| pack.created_ms),
        )
    }

    pub(crate) fn next_harvest(&self) -> Option<HarvestPack> {
        let state = self.state.lock().ok()?;
        let Loaded::Active(project) = &state.project else {
            return None;
        };
        project.harvest_queue.first().cloned()
    }

    pub(crate) fn apply_clerk_output(
        &self,
        harvest_id: &str,
        output: ClerkOutput,
    ) -> Result<usize, String> {
        output.validate()?;
        let harvest = {
            let state = self
                .state
                .lock()
                .map_err(|_| "Atlas state lock poisoned".to_string())?;
            let Loaded::Active(project) = &state.project else {
                return Err("Atlas project snapshot is inert".to_string());
            };
            if project
                .processed_harvests
                .iter()
                .any(|processed| processed == harvest_id)
            {
                return Ok(0);
            }
            project
                .harvest_queue
                .iter()
                .find(|pack| pack.id == harvest_id)
                .cloned()
                .ok_or_else(|| format!("unknown Atlas harvest: {harvest_id}"))?
        };

        let mut proposals = 0usize;
        if !matches!(output.action, ClerkAction::Abstain) {
            for candidate in output.candidates {
                let sources = candidate
                    .sources
                    .iter()
                    .map(|source| {
                        // A teacher cannot invent a digest, erase influence, or turn
                        // an unbound historical source id into independent evidence.
                        harvest
                            .receipt_sources
                            .iter()
                            .find(|recorded| {
                                recorded.id == source.id && recorded.digest == source.digest
                            })
                            .cloned()
                            .ok_or_else(|| {
                                format!(
                                    "clerk source is not bound to a native harvest receipt: {}",
                                    source.id
                                )
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let links = candidate
                    .merge_into
                    .map(|target_id| AtlasLink {
                        kind: AtlasLinkKind::DerivedFrom,
                        target_id,
                    })
                    .into_iter()
                    .collect();
                match self.propose_linked(
                    candidate.kind,
                    &candidate.content,
                    candidate.confidence,
                    sources,
                    links,
                ) {
                    Ok(_) => proposals += 1,
                    Err(error) if error.contains("equivalent Atlas item already exists") => {}
                    Err(error) => return Err(error),
                }
            }
        }
        self.mutate_project(|project| {
            if !project
                .processed_harvests
                .iter()
                .any(|processed| processed == harvest_id)
            {
                project.processed_harvests.push(harvest_id.to_string());
                if project.processed_harvests.len() > MAX_HARVESTS * 2 {
                    let remove = project.processed_harvests.len() - MAX_HARVESTS * 2;
                    project.processed_harvests.drain(..remove);
                }
            }
            project.harvest_queue.retain(|pack| pack.id != harvest_id);
            Ok(())
        })?;
        Ok(proposals)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn parse_clerk_output(&self, raw: &str) -> Result<ClerkOutput, String> {
        let output: ClerkOutput =
            serde_json::from_str(raw).map_err(|error| format!("invalid clerk JSON: {error}"))?;
        output.validate()?;
        Ok(output)
    }

    fn ensure_enabled(&self) -> Result<(), String> {
        if self.enabled {
            Ok(())
        } else {
            Err("Living Atlas is disabled by ANGEL_ATLAS=0".to_string())
        }
    }

    fn mutate_project(
        &self,
        mutate: impl FnOnce(&mut ProjectSnapshot) -> Result<(), String>,
    ) -> Result<(), String> {
        self.ensure_enabled()?;
        if !crate::platform::workspace_store::matches_project(
            &self.workspace,
            &self.project_root,
            &self.project_key,
        ) {
            return Err("Atlas repository identity no longer matches this service".to_string());
        }
        let _file_lock = FileLock::acquire(&lock_path(&self.project_path))?;
        let mut snapshot = match load_project(
            &self.project_path,
            &self.workspace,
            &self.project_root,
            &self.project_key,
        ) {
            Loaded::Active(snapshot) => snapshot,
            Loaded::Inert(error) => {
                if let Ok(mut state) = self.state.lock() {
                    state.project = Loaded::Inert(error.clone());
                    state.generation = state.generation.wrapping_add(1);
                }
                return Err(format!(
                    "Atlas snapshot is inert and will not be overwritten: {error}"
                ));
            }
        };
        mutate(&mut snapshot)?;
        snapshot.revision = snapshot.revision.saturating_add(1);
        write_snapshot(&_file_lock.directory, &self.project_path, &snapshot)?;
        if let Ok(mut state) = self.state.lock() {
            state.project = Loaded::Active(snapshot);
            state.generation = state.generation.wrapping_add(1);
        }
        Ok(())
    }

    fn write_lesson(&self, action: &str, proposal_id: &str, final_item: Option<&AtlasItem>) {
        if !self.enabled {
            return;
        }
        let target = match action {
            "reject" => json!({"schema": CLERK_SCHEMA, "action": "abstain", "candidates": []}),
            _ => final_item
                .map(|item| {
                    json!({
                        "schema": CLERK_SCHEMA,
                        "action": "propose",
                        "candidates": [{
                            "kind": item.kind,
                            "content": item.content,
                            "confidence": item.confidence,
                            "sources": item.sources
                        }]
                    })
                })
                .unwrap_or_else(
                    || json!({"schema": CLERK_SCHEMA, "action": "abstain", "candidates": []}),
                ),
        };
        let row = json!({
            "schema": LESSON_SCHEMA,
            "project_key": self.project_key,
            "proposal_id": proposal_id,
            "review_action": action,
            "target": target,
            "negative_preference": action == "reject",
            "created_ms": now_ms()
        });
        crate::knowledge::experience::append_jsonl(&self.lesson_path, &row);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AtlasLane {
    #[default]
    Now,
    Project,
    Review,
    Shared,
    Artifacts,
}

impl AtlasLane {
    pub(crate) const ALL: [Self; 5] = [
        Self::Now,
        Self::Project,
        Self::Review,
        Self::Shared,
        Self::Artifacts,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Now => "Now",
            Self::Project => "Project",
            Self::Review => "Review",
            Self::Shared => "Shared",
            Self::Artifacts => "Artifacts",
        }
    }

    pub(crate) fn step(self, delta: isize) -> Self {
        let index = Self::ALL.iter().position(|lane| *lane == self).unwrap_or(0);
        let next = (index as isize + delta).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct AtlasViewState {
    pub(crate) lane: AtlasLane,
    pub(crate) selected: usize,
    pub(crate) query: String,
}

impl AtlasViewState {
    pub(crate) fn select_delta(&mut self, delta: isize, len: usize) {
        if len == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as isize + delta).rem_euclid(len as isize) as usize;
    }

    pub(crate) fn set_lane(&mut self, lane: AtlasLane) {
        self.lane = lane;
        self.selected = 0;
    }
}

pub(crate) struct AtlasTool {
    service: Arc<AtlasService>,
}

impl AtlasTool {
    pub(crate) fn new(service: Arc<AtlasService>) -> Self {
        Self { service }
    }
}

impl Tool for AtlasTool {
    fn name(&self) -> &str {
        "atlas"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "atlas".to_string(),
            description: "Search Living Atlas, observe an exact workspace source file, trace concept/code links, or propose a sourced claim and links for human review. Source observations include current file hashes and lexical declaration hints, not verified behavior. Claims and relations cannot self-activate.".to_string(),
            params: json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["search", "detail", "propose", "observe", "trace"] },
                    "path": { "type": "string", "maxLength": 160 },
                    "symbol": { "type": "string", "maxLength": 128 },
                    "query": { "type": "string", "maxLength": 512 },
                    "id": { "type": "string", "maxLength": 128 },
                    "kind": {
                        "type": "string",
                        "enum": ["fact", "decision", "preference", "procedure", "open_thread", "entity", "artifact", "note"]
                    },
                    "content": { "type": "string", "maxLength": 4096 },
                    "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                    "links": {
                        "type": "array", "maxItems": 16,
                        "items": {
                            "type": "object",
                            "properties": {
                                "kind": { "type": "string", "enum": ["related", "derived_from"] },
                                "target_id": { "type": "string", "maxLength": 128 }
                            },
                            "required": ["kind", "target_id"], "additionalProperties": false
                        }
                    },
                    "sources": {
                        "type": "array",
                        "maxItems": 8,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "maxLength": 160 },
                                "kind": { "type": "string", "maxLength": 64 },
                                "digest": { "type": "string", "maxLength": 160 },
                                "excerpt": { "type": "string", "maxLength": 512 },
                                "independent": { "type": "boolean" },
                                "influenced_by": { "type": "string", "maxLength": 128 }
                            },
                            "required": ["id", "kind", "digest"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        self.service.reload_project()?;
        match args.get("action").and_then(Value::as_str).unwrap_or("") {
            "observe" => {
                let item = self.service.observe_code(
                    required_string(args, "path")?,
                    args.get("symbol").and_then(Value::as_str),
                )?;
                serde_json::to_string_pretty(&item).map_err(|error| error.to_string())
            }
            "trace" => self.service.trace(required_string(args, "id")?),
            "search" => {
                let query = args.get("query").and_then(Value::as_str).unwrap_or("");
                let mut items = self.service.list(AtlasLane::Project, Some(query));
                items.extend(self.service.list(AtlasLane::Shared, Some(query)));
                items.truncate(12);
                self.service.check_grounding(
                    &items.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
                )?;
                items = items
                    .iter()
                    .filter_map(|item| self.service.item(&item.id))
                    .collect();
                serde_json::to_string_pretty(&json!({
                    "scope": "project_then_shared",
                    "count": items.len(),
                    "items": items.iter().map(|item| json!({
                        "id": item.id,
                        "scope": item.scope,
                        "kind": item.kind,
                        "lifecycle": item.lifecycle,
                        "epistemic": item.epistemic,
                        "contested": item.contested,
                        "stale": item.stale,
                        "content": item.content,
                        "sources": item.sources,
                        "links": item.links
                    })).collect::<Vec<_>>()
                }))
                .map_err(|error| error.to_string())
            }
            "detail" => {
                let id = required_string(args, "id")?;
                self.service.check_grounding(&[id.to_string()])?;
                let item = self
                    .service
                    .item(id)
                    .ok_or_else(|| format!("unknown Atlas item: {id}"))?;
                serde_json::to_string_pretty(&item).map_err(|error| error.to_string())
            }
            "propose" => {
                let kind = AtlasKind::parse(required_string(args, "kind")?)
                    .ok_or_else(|| "invalid Atlas kind".to_string())?;
                let content = required_string(args, "content")?;
                let confidence = args
                    .get("confidence")
                    .and_then(Value::as_f64)
                    .map(|v| v as f32);
                let sources = args
                    .get("sources")
                    .cloned()
                    .ok_or_else(|| "proposal sources are required".to_string())
                    .and_then(|value| {
                        serde_json::from_value::<Vec<AtlasSource>>(value)
                            .map_err(|error| format!("invalid proposal sources: {error}"))
                    })?;
                let links = args
                    .get("links")
                    .cloned()
                    .map(serde_json::from_value::<Vec<AtlasLink>>)
                    .transpose()
                    .map_err(|error| format!("invalid proposal links: {error}"))?
                    .unwrap_or_default();
                let item = if links.is_empty() {
                    self.service.propose(kind, content, confidence, sources)?
                } else {
                    self.service
                        .propose_linked(kind, content, confidence, sources, links)?
                };
                Ok(format!(
                    "proposal {} queued for review; it is not active and cannot enter context",
                    item.id
                ))
            }
            _ => Err("atlas.action must be search, detail, propose, observe, or trace".to_string()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum ClerkAction {
    Propose,
    Merge,
    Abstain,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct ClerkCandidate {
    pub(crate) kind: AtlasKind,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) confidence: Option<f32>,
    pub(crate) sources: Vec<AtlasSource>,
    #[serde(default)]
    pub(crate) merge_into: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct ClerkOutput {
    pub(crate) schema: String,
    pub(crate) action: ClerkAction,
    pub(crate) candidates: Vec<ClerkCandidate>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ClerkOutput {
    fn validate(&self) -> Result<(), String> {
        if self.schema != CLERK_SCHEMA {
            return Err(format!("unknown Atlas Clerk schema: {}", self.schema));
        }
        if self.candidates.len() > 3 {
            return Err("Atlas Clerk may emit at most three candidates".to_string());
        }
        match self.action {
            ClerkAction::Abstain if !self.candidates.is_empty() => {
                return Err("abstain must contain zero candidates".to_string());
            }
            ClerkAction::Propose | ClerkAction::Merge if self.candidates.is_empty() => {
                return Err("propose/merge requires at least one candidate".to_string());
            }
            _ => {}
        }
        for candidate in &self.candidates {
            if candidate.content.trim().is_empty()
                || candidate.content.len() > MAX_CONTENT_BYTES
                || candidate.sources.is_empty()
                || candidate.sources.len() > MAX_SOURCES
            {
                return Err("Atlas Clerk candidate violates content/source bounds".to_string());
            }
            if candidate
                .confidence
                .is_some_and(|confidence| !(0.0..=1.0).contains(&confidence))
            {
                return Err("Atlas Clerk confidence must be between zero and one".to_string());
            }
            match self.action {
                ClerkAction::Merge if candidate.merge_into.is_none() => {
                    return Err("merge candidates require merge_into".to_string());
                }
                ClerkAction::Propose if candidate.merge_into.is_some() => {
                    return Err("propose candidates cannot set merge_into".to_string());
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct RankedItem<'a> {
    item: &'a AtlasItem,
    via: Option<&'a AtlasItem>,
    score: f32,
    matched: Vec<String>,
}

fn rank_scope<'a>(query_terms: &[String], items: &[&'a AtlasItem]) -> Vec<RankedItem<'a>> {
    if items.is_empty() {
        return Vec::new();
    }
    let docs = items
        .iter()
        .map(|item| tokens(&item.content))
        .collect::<Vec<_>>();
    let avg_len = docs.iter().map(Vec::len).sum::<usize>().max(1) as f32 / docs.len() as f32;
    let mut ranked = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let doc = &docs[index];
        let mut score = 0.0f32;
        let mut matched = Vec::new();
        for term in query_terms {
            let tf = doc.iter().filter(|token| *token == term).count() as f32;
            if tf == 0.0 {
                continue;
            }
            matched.push(term.clone());
            let df = docs
                .iter()
                .filter(|other| other.iter().any(|token| token == term))
                .count() as f32;
            let idf = (((docs.len() as f32 - df + 0.5) / (df + 0.5)) + 1.0).ln();
            let k1 = 1.2;
            let b = 0.75;
            let norm = tf + k1 * (1.0 - b + b * (doc.len().max(1) as f32 / avg_len.max(1.0)));
            score += idf * (tf * (k1 + 1.0) / norm);
        }
        if score <= 0.0 {
            continue;
        }
        score *= match item.epistemic {
            EpistemicState::Verified => 1.2,
            EpistemicState::Observed => 1.1,
            EpistemicState::Asserted => 1.0,
            EpistemicState::Inferred => 0.9,
        };
        score *= item.confidence.unwrap_or(1.0).clamp(0.1, 1.0);
        ranked.push(RankedItem {
            item,
            via: None,
            score,
            matched,
        });
    }
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.item.updated_ms.cmp(&a.item.updated_ms))
            .then_with(|| a.item.id.cmp(&b.item.id))
    });
    ranked
}

fn load_project(
    path: &Path,
    workspace: &Path,
    expected_root: &Path,
    expected_key: &str,
) -> Loaded<ProjectSnapshot> {
    if expected_root.to_str().is_none() {
        return Loaded::Inert(format!(
            "Atlas JSON does not support repository paths containing invalid UTF-8: {expected_root:?}; use a UTF-8 repository path; existing snapshots are preserved"
        ));
    }
    let Some(raw) = read_bounded(path) else {
        if path.exists() {
            return Loaded::Inert("unreadable or oversized project snapshot".to_string());
        }
        return Loaded::Active(empty_project(expected_root, expected_key));
    };
    let snapshot: ProjectSnapshot = match serde_json::from_slice(&raw) {
        Ok(snapshot) => snapshot,
        Err(error) => return Loaded::Inert(format!("malformed project snapshot: {error}")),
    };
    if snapshot.schema != PROJECT_SCHEMA {
        return Loaded::Inert(format!("unknown project schema: {}", snapshot.schema));
    }
    if snapshot.project_root != expected_root
        || snapshot.project_key != expected_key
        || !crate::platform::workspace_store::matches_project(
            workspace,
            &snapshot.project_root,
            &snapshot.project_key,
        )
    {
        return Loaded::Inert("foreign project identity".to_string());
    }
    if let Err(error) = validate_items(&snapshot.items) {
        return Loaded::Inert(error);
    }
    Loaded::Active(snapshot)
}

fn load_shared(path: &Path) -> Loaded<SharedSnapshot> {
    let Some(raw) = read_bounded(path) else {
        if path.exists() {
            return Loaded::Inert("unreadable or oversized shared snapshot".to_string());
        }
        return Loaded::Active(empty_shared());
    };
    let snapshot: SharedSnapshot = match serde_json::from_slice(&raw) {
        Ok(snapshot) => snapshot,
        Err(error) => return Loaded::Inert(format!("malformed shared snapshot: {error}")),
    };
    if snapshot.schema != SHARED_SCHEMA {
        return Loaded::Inert(format!("unknown shared schema: {}", snapshot.schema));
    }
    if let Err(error) = validate_items(&snapshot.items) {
        return Loaded::Inert(error);
    }
    if snapshot
        .items
        .iter()
        .any(|item| item.scope != AtlasScope::Shared)
    {
        return Loaded::Inert("shared snapshot contains a non-shared item".to_string());
    }
    Loaded::Active(snapshot)
}

fn validate_items(items: &[AtlasItem]) -> Result<(), String> {
    if items.len() > 10_000 {
        return Err("Atlas snapshot has too many items".to_string());
    }
    let mut ids = BTreeSet::new();
    for item in items {
        if !ids.insert(&item.id) {
            return Err(format!("duplicate Atlas item id: {}", item.id));
        }
        if item.content.is_empty()
            || item.content.len() > MAX_CONTENT_BYTES
            || item.sources.len() > MAX_SOURCES
            || item.links.len() > MAX_LINKS
            || item.content_digest != digest(&item.content)
            || item
                .confidence
                .is_some_and(|confidence| !(0.0..=1.0).contains(&confidence))
        {
            return Err(format!("invalid Atlas item: {}", item.id));
        }
    }
    Ok(())
}

fn read_bounded(path: &Path) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if metadata.len() > MAX_SNAPSHOT_BYTES {
        return None;
    }
    let mut raw = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_SNAPSHOT_BYTES + 1)
        .read_to_end(&mut raw)
        .ok()?;
    (raw.len() as u64 <= MAX_SNAPSHOT_BYTES).then_some(raw)
}

fn write_snapshot(
    directory: &crate::platform::workspace_store::private_io::PrivateDirectory,
    path: &Path,
    snapshot: &impl Serialize,
) -> Result<(), String> {
    let bytes = crate::platform::secrets::to_redacted_vec(snapshot)
        .map_err(|error| format!("serialize Atlas snapshot: {error}"))?;
    if bytes.len() as u64 > crate::platform::store_caps::value("atlas_snapshot", "max_bytes") {
        return Err("Atlas snapshot exceeds the 2 MiB safety cap".to_string());
    }
    let name = path
        .file_name()
        .ok_or_else(|| "Atlas snapshot has no file name".to_string())?;
    directory
        .replace(name, &bytes)
        .map_err(|error| format!("publish private Atlas snapshot {}: {error}", path.display()))
}

struct FileLock {
    file: File,
    directory: crate::platform::workspace_store::private_io::PrivateDirectory,
}

impl FileLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        let (directory, name) = crate::platform::workspace_store::private_io::parent(path)
            .map_err(|error| format!("open Atlas lock directory {}: {error}", path.display()))?;
        let file = directory
            .lock_file(name)
            .map_err(|error| format!("open Atlas lock {}: {error}", path.display()))?;
        #[cfg(unix)]
        crate::platform::workspace_store::lock_store(&file, "atlas_snapshot_lock")
            .map_err(|error| format!("lock Atlas snapshot: {error}"))?;
        Ok(Self { file, directory })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: the descriptor is valid until this Drop returns.
            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

fn empty_project(root: &Path, key: &str) -> ProjectSnapshot {
    ProjectSnapshot {
        schema: PROJECT_SCHEMA.to_string(),
        project_root: root.to_path_buf(),
        project_key: key.to_string(),
        revision: 0,
        items: Vec::new(),
        tombstones: Vec::new(),
        promotions: Vec::new(),
        harvest_queue: Vec::new(),
        processed_harvests: Vec::new(),
        undo: None,
    }
}

fn empty_shared() -> SharedSnapshot {
    SharedSnapshot {
        schema: SHARED_SCHEMA.to_string(),
        revision: 0,
        items: Vec::new(),
        promotions: Vec::new(),
    }
}

fn sanitize_sources(sources: Vec<AtlasSource>, root: &Path) -> Vec<AtlasSource> {
    sources
        .into_iter()
        .take(MAX_SOURCES)
        .map(|source| AtlasSource {
            id: bounded(&source.id, 160),
            kind: bounded(&source.kind, 64),
            digest: bounded(&source.digest, 160),
            excerpt: source
                .excerpt
                .as_deref()
                .and_then(|excerpt| sanitize_text(excerpt, root, MAX_EXCERPT_BYTES).ok()),
            independent: source.independent,
            influenced_by: source.influenced_by.map(|value| bounded(&value, 128)),
        })
        .collect()
}

fn sanitize_text(value: &str, root: &Path, max_bytes: usize) -> Result<String, String> {
    let mut value = value.replace('\0', "");
    let root_text = root.to_string_lossy();
    if !root_text.is_empty() {
        value = value.replace(root_text.as_ref(), "$PROJECT");
    }
    let credential = regex::Regex::new(
        r"(?i)(api[_-]?key|access[_-]?token|secret|password|authorization)\s*[:=]\s*([^\s,;]+)",
    )
    .map_err(|error| error.to_string())?;
    value = credential.replace_all(&value, "$1=[REDACTED]").into_owned();
    let bearer = regex::Regex::new(r"(?i)\b(bearer)\s+[A-Za-z0-9._~+/=-]{8,}")
        .map_err(|error| error.to_string())?;
    value = bearer.replace_all(&value, "$1 [REDACTED]").into_owned();
    let absolute = regex::Regex::new(r"(?m)(^|[\s(])(/[A-Za-z0-9._-]+){2,}")
        .map_err(|error| error.to_string())?;
    value = absolute
        .replace_all(&value, |captures: &regex::Captures<'_>| {
            format!("{}[absolute-path]", &captures[1])
        })
        .into_owned();
    // Publish the same redacted bytes whose digest is stored. Private snapshot
    // serialization redacts again; hashing a pre-redaction value makes a valid
    // in-memory item become an inert digest mismatch on the next reload.
    Ok(bounded(
        crate::platform::secrets::redact_str(value.trim()).as_ref(),
        max_bytes,
    ))
}

fn bounded(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let target = max_bytes.saturating_sub('…'.len_utf8());
    let end = (0..=target)
        .rev()
        .find(|index| value.is_char_boundary(*index))
        .unwrap_or(0);
    format!("{}…", &value[..end])
}

fn digest(value: &str) -> String {
    let mut first = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut first);
    let mut second = std::collections::hash_map::DefaultHasher::new();
    "angel-atlas/v1".hash(&mut second);
    value.hash(&mut second);
    format!("{:016x}{:016x}", first.finish(), second.finish())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "no"
            )
        })
        .unwrap_or(default)
}

fn lock_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("atlas");
    path.with_file_name(format!("{name}.lock"))
}

fn item_index(snapshot: &ProjectSnapshot, id: &str) -> Result<usize, String> {
    snapshot
        .items
        .iter()
        .position(|item| item.id == id)
        .ok_or_else(|| format!("unknown project Atlas item: {id}"))
}

fn lifecycle_label(lifecycle: AtlasLifecycle) -> &'static str {
    match lifecycle {
        AtlasLifecycle::Proposed => "proposed",
        AtlasLifecycle::Active => "active",
        AtlasLifecycle::Rejected => "rejected",
        AtlasLifecycle::Archived => "forgotten",
        AtlasLifecycle::Superseded => "superseded",
    }
}

fn epistemic_label(epistemic: EpistemicState) -> &'static str {
    match epistemic {
        EpistemicState::Asserted => "asserted",
        EpistemicState::Observed => "observed",
        EpistemicState::Verified => "verified",
        EpistemicState::Inferred => "inferred",
    }
}

fn required_string<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("atlas.{key} is required"))
}

fn tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|token| token.len() >= 2)
        .collect()
}

fn suggested_commands(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("$ ")
                .or_else(|| trimmed.strip_prefix("command:"))
                .map(str::trim)
                .filter(|command| !command.is_empty())
                .map(str::to_string)
        })
        .collect()
}

/// Rollout gates stay deterministic and separate from model routing.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct AtlasPromotionMetrics {
    pub(crate) reviewed_lessons: usize,
    pub(crate) held_out: usize,
    pub(crate) kind_scope_macro_f1: f32,
    pub(crate) action_accuracy: f32,
    pub(crate) false_positive_rate: f32,
    pub(crate) source_precision: f32,
    pub(crate) canary_reviews: usize,
    pub(crate) scope_leaks: usize,
    pub(crate) rejection_regression_points: f32,
    pub(crate) schema_source_scope_safety: bool,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn clerk_promotion_ready(metrics: AtlasPromotionMetrics) -> bool {
    metrics.schema_source_scope_safety
        && metrics.reviewed_lessons >= 256
        && metrics.held_out >= 64
        && metrics.kind_scope_macro_f1 >= 0.95
        && metrics.action_accuracy >= 0.90
        && metrics.false_positive_rate <= 0.02
        && metrics.source_precision >= 0.98
        && metrics.canary_reviews >= 100
        && metrics.scope_leaks == 0
        && metrics.rejection_regression_points <= 2.0
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct AtlasModelCapability {
    pub(crate) route_id: String,
    pub(crate) parameter_billions: Option<u32>,
    pub(crate) atlas_adapter: bool,
    pub(crate) available: bool,
    pub(crate) training: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct AtlasModelRoles {
    pub(crate) teacher: Option<String>,
    pub(crate) clerk: Option<String>,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn resolve_model_roles(
    capabilities: &[AtlasModelCapability],
    promotion_ready: bool,
) -> AtlasModelRoles {
    let teacher = capabilities
        .iter()
        .filter(|capability| capability.available && !capability.training)
        .filter(|capability| capability.parameter_billions.unwrap_or(0) >= 30)
        .max_by_key(|capability| capability.parameter_billions.unwrap_or(0))
        .map(|capability| capability.route_id.clone());
    let clerk = promotion_ready
        .then(|| {
            capabilities
                .iter()
                .find(|capability| {
                    capability.available
                        && !capability.training
                        && capability.atlas_adapter
                        && capability.parameter_billions == Some(4)
                })
                .map(|capability| capability.route_id.clone())
        })
        .flatten();
    AtlasModelRoles { teacher, clerk }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/atlas__invalid_path_tests.rs"]
mod invalid_path_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/atlas__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/atlas__private_io_tests.rs"]
mod private_io_tests;

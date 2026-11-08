//! Hot-swappable application behavior. `system` supervises executable systems
//! with checkpoint handoff; the JS registry captures immutable pure transforms
//! over the cockpit's V8 engine. The native host owns sessions and effects.
pub mod bj;
mod bundle;
mod contracts;
pub mod delta;
pub mod host;
pub mod kernel;
mod outcome;
pub mod realm;
#[cfg(feature = "v8")]
pub mod shipped;
pub mod telemetry;
#[cfg(feature = "v8")]
pub mod v8;
mod watch;
pub use contracts::{GamePolicy, UiBand, UiPolicy};
use outcome::Scan;
pub use outcome::{Board, Outcome, WINDOW};

use bundle::Bundle;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, mpsc};
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(100);
const DEBOUNCE: Duration = Duration::from_millis(120);
/// Live modules one loader holds. Authored behavior moves into modules leg by
/// leg, so this bounds a directory, not a feature.
const MAX_MODULES: usize = 256;
const MAX_OUTPUT: usize = 64 * 1024;
const MAX_INPUT: usize = 512 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RealmStyle {
    pub scale: f32,
    pub notice_ticks: u64,
    pub sparkle_ticks: u64,
    pub sleeping: String,
    pub watching: String,
    pub resurrecting: String,
    pub recovered: String,
    pub accounted: String,
    pub absent: String,
}

impl Default for RealmStyle {
    fn default() -> Self {
        Self {
            scale: 1.0,
            notice_ticks: 72,
            sparkle_ticks: 30,
            sleeping: "a giant sleeps by the keep".into(),
            watching: "the giant stirs — a knight has fallen silent".into(),
            resurrecting: "the giant kneels by the fallen knight".into(),
            recovered: "the knight breathes again — the giant sleeps".into(),
            accounted: "all knights accounted for — the giant sleeps".into(),
            absent: "the giant has left the realm".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub id: String,
    pub generation: u64,
    pub digest: String,
    pub error: Option<String>,
    /// Source and output digests from the active Bend generation's receipt.
    pub build: Option<BuildReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildReceipt {
    pub source_digest: String,
    pub output_digest: String,
}

/// Immutable content-addressed bytes. Holding the Arc leases a generation's
/// resource across replacement; no consumer reopens an edited source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub id: String,
    pub bytes: Vec<u8>,
}

/// Splits a render script into its per-generation setup and its call; see
/// `Bundle::script`.
pub const CALL_MARKER: &str = "/*angel:call:";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub resources: BTreeMap<String, kernel::Resource>,
    pub assets: BTreeMap<String, Arc<Asset>>,
    pub revision: u64,
    pub statuses: Vec<Status>,
    pub realm: Option<RealmStyle>,
    pub agent_note: Option<(String, String)>, // source digest, text
    pub ui: Option<Arc<UiPolicy>>,
    pub game: Option<Arc<GamePolicy>>,
    pub tool_router: Option<Program>,
    /// Named decision legs: every active `policy` module, keyed by its id.
    /// Callers pin one map per turn and keep their native leg as fallback.
    pub policies: BTreeMap<String, Program>,
    pub error: Option<String>,
    pub routes: Vec<kernel::Route>,
    pub surfaces: BTreeMap<String, kernel::Surface>,
}

impl Status {
    /// One `Snapshot::text` line. An edit's outcome also carries how long the
    /// loader took to reach its verdict.
    pub fn line(&self, took: Option<Duration>) -> String {
        format!(
            "{} · generation {} · {}{}{}",
            self.id,
            self.generation,
            self.digest.get(..12).unwrap_or(&self.digest),
            took.map(|took| format!(" · {} ms", took.as_millis()))
                .unwrap_or_default(),
            self.error
                .as_ref()
                .map(|e| format!(" · kept previous: {e}"))
                .unwrap_or_default()
        )
    }
}

impl Snapshot {
    pub fn heading(root: &Path) -> String {
        format!("live modules · {}", root.display())
    }

    pub fn text(&self, root: &Path) -> String {
        let mut lines = vec![Self::heading(root)];
        if let Some(error) = &self.error {
            lines.push(error.clone());
        }
        if self.statuses.is_empty() {
            lines.push("no live modules loaded".into());
        }
        for status in &self.statuses {
            lines.push(status.line(None));
        }
        lines.join("\n")
    }
}

struct Module {
    captured: Option<Arc<Bundle>>,
    active: Option<Arc<Bundle>>,
    previous: Option<(Arc<Bundle>, Value)>,
    generation: u64,
    staged: Option<(String, Instant)>,
    rejected: Option<String>,
    error: Option<String>,
    cached: Option<Value>,
    /// False while a captured candidate awaits its debounce or handoff; true
    /// once the scan has activated or rejected it, or has nothing to judge.
    settled: bool,
}

impl Default for Module {
    fn default() -> Self {
        Self {
            captured: None,
            active: None,
            previous: None,
            generation: 0,
            staged: None,
            rejected: None,
            error: None,
            cached: None,
            settled: true,
        }
    }
}

/// The evaluator supplies an isolated JavaScript call with a finite deadline.
pub type Evaluator = fn(&str) -> Result<String, String>;
pub type Validator = fn(&str, &Value) -> Result<(), String>;
pub type ResourceBinder = fn(
    &str,
    &str,
    &Value,
    &BTreeMap<String, Arc<Asset>>,
    &kernel::Candidate,
    &kernel::Resources,
) -> Result<kernel::Binding, String>;
fn no_resource_binding(
    _: &str,
    _: &str,
    _: &Value,
    _: &BTreeMap<String, Arc<Asset>>,
    _: &kernel::Candidate,
    _: &kernel::Resources,
) -> Result<kernel::Binding, String> {
    Ok(Arc::new(()))
}

fn no_host_validation(_: &str, _: &Value) -> Result<(), String> {
    Ok(())
}

/// An immutable generation, pinned for an entire native operation. Reloading
/// the registry cannot change the policy of a turn already in progress.
#[derive(Clone, Debug)]
pub struct Program {
    bundle: Arc<Bundle>,
    evaluator: Evaluator,
    validator: Validator,
}

/// The policy generation a host ships inside its binary, checked like a
/// loaded one (receipt, probe, contract). It serves when no live generation
/// is active, so a policy never lacks an implementation.
pub fn shipped_program(
    id: &str,
    files: &[(&str, &str)],
    evaluator: Evaluator,
    validator: Validator,
) -> Result<Program, String> {
    let bundle = Arc::new(Bundle::from_files(id, files)?);
    let program = Program { bundle, evaluator, validator };
    program.render(&program.bundle.manifest.probe)?;
    Ok(program)
}

impl PartialEq for Program {
    fn eq(&self, other: &Self) -> bool {
        self.digest() == other.digest()
    }
}

impl Program {
    pub fn digest(&self) -> &str {
        &self.bundle.digest
    }

    /// Call off the UI/simulation thread. No file reads and no changing
    /// dependency imports occur here; source was captured at activation.
    pub fn render(&self, input: &Value) -> Result<Value, String> {
        let value = evaluate(&self.bundle, input, self.evaluator)
            .and_then(|value| validate(&self.bundle.manifest.kind, value))?;
        (self.validator)(&self.bundle.manifest.kind, &value)?;
        Ok(value)
    }
}

/// Single owner of published generations. Reloads never repeat a model/tool
/// request; candidate probes and fallback transforms have no external effects.
pub struct Registry {
    evaluator: Evaluator,
    validator: Validator,
    root: PathBuf,
    modules: BTreeMap<String, Module>,
    revision: u64,
    error: Option<String>,
    binder: ResourceBinder,
    kernel: kernel::Kernel,
    work: std::collections::VecDeque<kernel::IssuedEffect>,
    completions: std::collections::VecDeque<(kernel::Job, Value)>,
    failed_system_transaction: Option<Vec<(String, String, u64)>>,
    scans: u64,
    /// Lets a module whose capture matched the disk skip the metadata
    /// comparison until something under it changes. Only the loader's worker
    /// holds one; a registry driven by hand compares every scan.
    watch: Option<watch::Watch>,
}

impl Registry {
    pub fn new(root: PathBuf, evaluator: Evaluator) -> Self {
        Self::validated(root, evaluator, no_host_validation)
    }

    pub fn validated(root: PathBuf, evaluator: Evaluator, validator: Validator) -> Self {
        Self::with_resources(root, evaluator, validator, no_resource_binding)
    }

    pub fn with_resources(
        root: PathBuf,
        evaluator: Evaluator,
        validator: Validator,
        binder: ResourceBinder,
    ) -> Self {
        Self {
            evaluator,
            validator,
            root,
            modules: BTreeMap::new(),
            revision: 0,
            error: None,
            binder,
            kernel: Default::default(),
            work: Default::default(),
            completions: Default::default(),
            failed_system_transaction: None,
            scans: 0,
            watch: None,
        }
    }

    pub fn refresh(&mut self, force: bool) {
        self.refresh_snapshot(force);
    }

    /// [`Registry::refresh`], returning the snapshot of the registry as it
    /// stands after it: the worker publishes that instead of taking another.
    fn refresh_snapshot(&mut self, force: bool) -> Snapshot {
        if let Some(watch) = &mut self.watch {
            watch.drain();
        }
        let before = self.snapshot();
        self.refresh_inner(force);
        self.scans += 1;
        let mut after = self.snapshot();
        if before != after {
            self.revision += 1;
            after.revision = self.revision;
        }
        after
    }

    /// Whether the last scan left no captured candidate unjudged.
    fn settled(&self, id: &str) -> bool {
        self.modules.get(id).is_none_or(|module| module.settled)
    }

    fn refresh_inner(&mut self, force: bool) {
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) => {
                self.error = Some(format!("module directory unavailable: {error}"));
                return;
            }
        };
        self.error = None;
        let mut ids = std::collections::BTreeSet::new();
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if id.starts_with('.') {
                continue;
            }
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            if !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            {
                self.error = Some("module ids use letters, digits, - and _".into());
                continue;
            }
            ids.insert(id);
            if ids.len() > MAX_MODULES {
                self.error = Some(format!("at most {MAX_MODULES} live modules are supported"));
                return;
            }
        }
        // A missing source is a failed candidate, not an implicit unload.
        ids.extend(self.modules.keys().cloned());
        let mut systems = Vec::new();
        for id in ids {
            if !self.modules.contains_key(&id) && self.modules.len() >= MAX_MODULES {
                self.error = Some(format!("at most {MAX_MODULES} retained live modules are supported"));
                continue;
            }
            let module = self.modules.entry(id.clone()).or_default();
            module.settled = true;
            let root = self.root.join(&id);
            // A capture that matched the disk at its last full comparison, with
            // nothing changed under the module since, is still current.
            let trusted = !force && self.watch.as_ref().is_some_and(|watch| watch.trusted(&id));
            let candidate = match module.captured.as_ref().filter(|bundle| {
                !force
                    && (trusted || {
                        if let Some(watch) = self.watch.as_mut() {
                            watch.arm(&id, &root);
                        }
                        let fresh = bundle.fresh(&root);
                        if let Some(watch) = self.watch.as_mut() {
                            watch.confirm(&id, fresh);
                        }
                        fresh
                    })
            }) {
                Some(cached) => Arc::clone(cached),
                None => match Bundle::read(&root, &id) {
                    Ok(mut candidate) => {
                        // Content-addressed bytes can be shared across source
                        // generations and metadata-only edits. A recapture must
                        // not keep another complete copy of an unchanged pack.
                        for (name, asset) in &mut candidate.assets {
                            let existing = module
                                .captured
                                .as_ref()
                                .and_then(|bundle| bundle.assets.get(name))
                                .filter(|existing| existing.id == asset.id)
                                .or_else(|| {
                                    module
                                        .active
                                        .as_ref()
                                        .and_then(|bundle| bundle.assets.get(name))
                                        .filter(|existing| existing.id == asset.id)
                                });
                            if let Some(existing) = existing {
                                *asset = Arc::clone(existing);
                            }
                        }
                        let candidate = Arc::new(candidate);
                        module.captured = Some(Arc::clone(&candidate));
                        candidate
                    }
                    Err(error) => {
                        module.error = Some(error);
                        continue;
                    }
                },
            };
            if let Some(error) = &candidate.build_error {
                module.error = Some(error.clone());
                continue;
            }
            if module
                .active
                .as_ref()
                .is_some_and(|active| active.manifest.kind != candidate.manifest.kind)
            {
                module.error = Some(
                    "changing an active module's runtime kind requires a new module identity"
                        .into(),
                );
                continue;
            }
            if module
                .active
                .as_ref()
                .is_some_and(|active| active.digest == candidate.digest)
            {
                module.error = None;
                module.staged = None;
                module.rejected = None;
                continue;
            }
            if !force
                && candidate.manifest.kind != "system"
                && module.rejected.as_deref() == Some(&candidate.digest)
            {
                continue;
            }
            if !force && module.active.is_some() {
                match &module.staged {
                    Some((digest, since))
                        if digest == &candidate.digest && since.elapsed() >= DEBOUNCE => {}
                    Some((digest, _)) if digest == &candidate.digest => {
                        module.settled = false;
                        continue;
                    }
                    _ => {
                        module.staged = Some((candidate.digest.clone(), Instant::now()));
                        module.settled = false;
                        continue;
                    }
                }
            }
            if candidate.manifest.kind == "system" {
                module.settled = false;
                systems.push((id, candidate));
                continue;
            }
            let probe = evaluate(&candidate, &candidate.manifest.probe, self.evaluator)
                .and_then(|value| validate(&candidate.manifest.kind, value))
                .and_then(|value| {
                    (self.validator)(&candidate.manifest.kind, &value)?;
                    Ok(value)
                });
            match probe {
                Ok(value) => {
                    module.generation += 1;
                    module.previous = module.active.take().zip(module.cached.take());
                    module.active = Some(candidate);
                    module.cached = Some(value);
                    module.staged = None;
                    module.rejected = None;
                    module.error = None;
                }
                Err(error) => {
                    module.rejected = Some(candidate.digest.clone());
                    module.error = Some(error);
                }
            }
        }
        if !systems.is_empty() {
            let transaction = systems
                .iter()
                .map(|(id, bundle)| {
                    (
                        id.clone(),
                        bundle.digest.clone(),
                        self.kernel.state_revision(id),
                    )
                })
                .chain(self.modules.iter().filter_map(|(id, module)| {
                    module
                        .active
                        .as_ref()
                        .map(|b| (id.clone(), b.digest.clone(), self.kernel.state_revision(id)))
                }))
                .collect::<Vec<_>>();
            if !force && self.failed_system_transaction.as_ref() == Some(&transaction) {
                for (id, _) in &systems {
                    if let Some(module) = self.modules.get_mut(id) {
                        module.settled = true;
                    }
                }
                return;
            }
            let candidates = systems
                .iter()
                .map(|(_, bundle)| kernel::Candidate {
                    manifest: bundle.manifest.system_manifest(),
                    identity: bundle.digest.clone(),
                    implementation: Arc::new(SystemProgram::new(
                        Arc::clone(bundle),
                        self.evaluator,
                        self.validator,
                        self.binder,
                    )),
                })
                .collect();
            let result = self
                .kernel
                .stage(candidates)
                .and_then(|group| self.kernel.commit(group));
            self.failed_system_transaction = match &result {
                Err(error) if !error.contains("handoff deferred") => Some(transaction),
                _ => None,
            };
            for (id, bundle) in systems {
                let module = self.modules.get_mut(&id).unwrap();
                match &result {
                    Ok(()) => {
                        module.generation = self.kernel.generation(&id).unwrap();
                        module.active = Some(bundle);
                        module.previous = None;
                        module.cached = None;
                        module.staged = None;
                        module.rejected = None;
                        module.error = None;
                        module.settled = true;
                    }
                    Err(error) => {
                        if !error.contains("handoff deferred") {
                            module.rejected = Some(bundle.digest.clone());
                            module.settled = true;
                        }
                        module.error = Some(error.clone());
                    }
                }
            }
        }
    }

    pub fn dispatch(
        &mut self,
        kind: kernel::Kind,
        name: &str,
        event: &Value,
    ) -> Result<Value, String> {
        let response = self.kernel.dispatch(kind, name, event)?;
        self.work.extend(response.effects.iter().cloned());
        self.revision += 1;
        serde_json::to_value(response).map_err(|e| e.to_string())
    }

    pub fn complete(&mut self, job: &kernel::Job, result: Value) -> Result<Value, String> {
        let response = self.kernel.complete(job, result)?;
        self.work.extend(response.effects.iter().cloned());
        self.revision += 1;
        serde_json::to_value(response).map_err(|e| e.to_string())
    }

    pub fn cancel(&mut self, request: u64) -> Result<Value, String> {
        let job = self
            .completions
            .iter()
            .map(|(job, _)| job)
            .chain(self.work.iter().map(|issued| &issued.job))
            .find(|job| job.request == request)
            .cloned()
            .ok_or("unknown module request")?;
        self.kernel.cancel(&job)?;
        self.completions.retain(|(other, _)| other != &job);
        self.work.retain(|issued| issued.job != job);
        self.error = None;
        self.revision += 1;
        Ok(json!(format!(
            "abandoned module request {request}; no effect replayed"
        )))
    }

    /// One bounded service call per worker iteration. No effect is replayed if
    /// a provider fails. Failed completion reducers retain the result for the
    /// operator to inspect/recover rather than rerunning the service.
    pub fn advance(&mut self) {
        if let Some(issued) = self.work.pop_front() {
            let result = match self.kernel.invoke(&issued) {
                Ok(response) => {
                    self.work.extend(response.effects);
                    json!({"ok":true,"value":response.output})
                }
                Err(error) => json!({"ok":false,"error":error}),
            };
            self.completions.push_back((issued.job, result));
            self.revision += 1;
        }
        if let Some((job, result)) = self.completions.pop_front() {
            if let Err(error) = self.complete(&job, result.clone()) {
                self.error = Some(format!(
                    "module request {} completion retained: {error}; /reload cancel {} abandons it without replay",
                    job.request, job.request
                ));
                self.completions.push_back((job, result));
            }
        }
    }

    pub fn render(&mut self, id: &str, input: &Value) -> Result<Value, String> {
        if let Some(route) = self.kernel.routes().into_iter().find(|route| {
            route.module == id
                && !matches!(
                    route.export.kind,
                    kernel::Kind::Surface | kernel::Kind::Resource
                )
        }) {
            return self.dispatch(route.export.kind, &route.export.name, input);
        }
        let module = self
            .modules
            .get_mut(id)
            .ok_or_else(|| format!("unknown live module {id}"))?;
        let active = module
            .active
            .as_ref()
            .ok_or_else(|| format!("{id}: no working generation"))?;
        let result = evaluate(active, input, self.evaluator)
            .and_then(|value| validate(&active.manifest.kind, value));
        if let Err(error) = &result {
            module.error = Some(format!("request failed: {error}"));
            module.rejected = Some(active.digest.clone());
            self.revision += 1;
            if let Some((previous, cached)) = module.previous.take() {
                // These modules are pure projections: retrying their transform
                // cannot repeat a model call, tool execution or external effect.
                let fallback = evaluate(&previous, input, self.evaluator)
                    .and_then(|value| validate(&previous.manifest.kind, value));
                module.active = Some(previous);
                module.cached = Some(cached);
                module.generation += 1;
                return fallback;
            }
        }
        result
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut snapshot = Snapshot {
            revision: self.revision,
            error: self.error.clone(),
            routes: self.kernel.routes(),
            resources: self.kernel.resources(),
            surfaces: self.kernel.surfaces(),
            ..Snapshot::default()
        };
        for (id, module) in &self.modules {
            if let Some(active) = &module.active {
                for asset in active.assets.values() {
                    snapshot.assets.insert(asset.id.clone(), Arc::clone(asset));
                }
            }
            snapshot.statuses.push(Status {
                id: id.clone(),
                generation: self.kernel.generation(id).unwrap_or(module.generation),
                digest: module
                    .active
                    .as_ref()
                    .map(|b| b.digest.clone())
                    .unwrap_or_default(),
                error: module.error.clone(),
                build: module.active.as_ref().and_then(|b| b.build_receipt()),
            });
            if id == "realm" {
                snapshot.realm = module
                    .cached
                    .clone()
                    .and_then(|v| serde_json::from_value(v).ok());
            }
            if id == "ui" {
                snapshot.ui = module
                    .cached
                    .clone()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .map(Arc::new);
            }
            if id == "game" {
                snapshot.game = module
                    .cached
                    .clone()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .map(Arc::new);
            }
            if let Some(bundle) = module.active.as_ref().filter(|b| b.manifest.kind == "policy") {
                snapshot.policies.insert(
                    id.clone(),
                    Program {
                        bundle: Arc::clone(bundle),
                        evaluator: self.evaluator,
                        validator: self.validator,
                    },
                );
            }
            if id == "tool-router" {
                snapshot.tool_router = module.active.as_ref().map(|bundle| Program {
                    bundle: Arc::clone(bundle),
                    evaluator: self.evaluator,
                    validator: self.validator,
                });
            }
            if id == "agent"
                && let Some(note) = module
                    .cached
                    .as_ref()
                    .and_then(|v| v.get("turn_note"))
                    .and_then(Value::as_str)
            {
                snapshot.agent_note = module
                    .active
                    .as_ref()
                    .map(|b| (b.digest.clone(), note.to_string()));
            }
        }
        snapshot
    }
}

fn evaluate(bundle: &Bundle, input: &Value, evaluator: Evaluator) -> Result<Value, String> {
    if input.to_string().len() > MAX_INPUT {
        return Err("live module input exceeds 512 KiB".into());
    }
    evaluate_script(&bundle.script(input), evaluator)
}

fn evaluate_script(script: &str, evaluator: Evaluator) -> Result<Value, String> {
    let result = evaluator(script)?;
    if result.len() > MAX_OUTPUT {
        return Err("live module output exceeds 64 KiB".into());
    }
    let envelope: Value = serde_json::from_str(&result).map_err(|e| e.to_string())?;
    envelope
        .get("value")
        .cloned()
        .ok_or_else(|| "live module returned no value".into())
}

#[derive(Clone)]
struct SystemProgram {
    bundle: Arc<Bundle>,
    evaluator: Evaluator,
    validator: Validator,
    binder: ResourceBinder,
    resources: kernel::Resources,
    resource_json: Arc<str>,
    /// The module setup for `resource_json`, built once per resource set. Each
    /// call appends its own part, so an evaluator that keeps the setup's
    /// context per generation and resource set runs only that part.
    setup: Arc<str>,
}
impl SystemProgram {
    fn new(
        bundle: Arc<Bundle>,
        evaluator: Evaluator,
        validator: Validator,
        binder: ResourceBinder,
    ) -> Self {
        let mut program = Self {
            bundle,
            evaluator,
            validator,
            binder,
            resources: Default::default(),
            resource_json: Arc::from("{}"),
            setup: Arc::from(""),
        };
        program.setup = program.build_setup();
        program
    }
    fn build_setup(&self) -> Arc<str> {
        let key = bundle::resource_identity(&self.resource_json);
        Arc::from(self.bundle.system_setup(&self.resource_json, &key))
    }
    fn evaluate(&self, method: &str, previous: &Value, event: &Value) -> Result<Value, String> {
        let call = self.bundle.system_call(method, previous, event);
        let mut script = String::with_capacity(self.setup.len() + call.len());
        script.push_str(&self.setup);
        script.push_str(&call);
        let result = (self.evaluator)(&script)?;
        if result.len() > 1152 * 1024 {
            return Err("system state/view/resource budget exceeded".into());
        }
        let value: Value = serde_json::from_str(&result).map_err(|e| e.to_string())?;
        let value = value.get("value").ok_or("system returned no value")?;
        if let Some(resources) = value.get("resources").and_then(Value::as_object) {
            for (name, resource) in resources {
                let export = self
                    .bundle
                    .manifest
                    .exports
                    .iter()
                    .find(|e| e.kind == kernel::Kind::Resource && e.name == *name)
                    .ok_or("unregistered resource")?;
                (self.validator)(&export.contract, resource)?;
            }
        }
        Ok(value.clone())
    }
}
impl kernel::Implementation for SystemProgram {
    fn with_resources(
        &self,
        resources: &kernel::Resources,
    ) -> Result<Option<Arc<dyn kernel::Implementation>>, String> {
        let values = resources.iter().map(|(key, resource)| {
            (key, serde_json::json!({
                "module":resource.module,"name":resource.name,"generation":resource.generation,
                "revision":resource.revision,"identity":resource.identity,
                "contract":resource.contract,"value":resource.value.as_ref()
            }))
        }).collect::<BTreeMap<_, _>>();
        let resource_json = serde_json::to_string(&values).map_err(|e| e.to_string())?;
        if resource_json.len() > MAX_INPUT {
            return Err("imported resource metadata exceeds 512 KiB".into());
        }
        let mut program = Self {
            resources: resources.clone(),
            resource_json: Arc::from(resource_json),
            ..self.clone()
        };
        program.setup = program.build_setup();
        Ok(Some(Arc::new(program)))
    }

    fn rebind(&self, previous: &kernel::Checkpoint) -> Result<kernel::PreparedState, String> {
        let previous = serde_json::to_value(previous).map_err(|e| e.to_string())?;
        let value = self.evaluate("rebind", &previous, &Value::Null)?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
    fn instantiate(&self, context: &Value) -> Result<kernel::Checkpoint, String> {
        let value = self.evaluate("instantiate", &Value::Null, context)?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
    fn reduce_instance(
        &self,
        checkpoint: &kernel::Checkpoint,
        event: &Value,
    ) -> Result<kernel::Transition, String> {
        let previous = serde_json::to_value(checkpoint).map_err(|e| e.to_string())?;
        let value = self.evaluate("instance", &previous, event)?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    fn bind_resource(&self, name: &str, value: &Value) -> Result<kernel::Binding, String> {
        let export = self
            .bundle
            .manifest
            .exports
            .iter()
            .find(|e| e.kind == kernel::Kind::Resource && e.name == name)
            .ok_or("unregistered module resource")?;
        (self.binder)(
            &export.contract,
            name,
            value,
            &self.bundle.assets,
            &kernel::Candidate {
                manifest: self.bundle.manifest.system_manifest(),
                identity: self.bundle.digest.clone(),
                implementation: Arc::new(self.clone()),
            },
            &self.resources,
        )
    }
    fn prepare(
        &self,
        previous: Option<&kernel::Checkpoint>,
    ) -> Result<kernel::PreparedState, String> {
        let previous = serde_json::to_value(previous).map_err(|e| e.to_string())?;
        let value = self.evaluate("prepare", &previous, &Value::Null)?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
    fn reduce(
        &self,
        checkpoint: &kernel::Checkpoint,
        event: &Value,
    ) -> Result<kernel::Transition, String> {
        if event.to_string().len() > MAX_INPUT {
            return Err("module event exceeds 512 KiB".into());
        }
        let previous = serde_json::to_value(checkpoint).map_err(|e| e.to_string())?;
        let value = self.evaluate("reduce", &previous, event)?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
}

fn validate(kind: &str, value: Value) -> Result<Value, String> {
    match kind {
        "text" if value.is_string() => Ok(value),
        "ui" => {
            serde_json::from_value::<UiPolicy>(value.clone())
                .map_err(|e| e.to_string())?
                .validate()?;
            Ok(value)
        }
        "game" => {
            serde_json::from_value::<GamePolicy>(value.clone())
                .map_err(|e| e.to_string())?
                .validate()?;
            Ok(value)
        }
        "tool-router"
            if value.as_array().is_some_and(|names| {
                names.len() <= 48
                    && names.iter().all(|name| {
                        name.as_str()
                            .is_some_and(|name| !name.is_empty() && name.len() <= 128)
                    })
            }) =>
        {
            Ok(value)
        }
        "realm" => {
            let style: RealmStyle =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            if !(0.25..=3.0).contains(&style.scale)
                || style.notice_ticks > 600
                || style.sparkle_ticks > 600
            {
                return Err(
                    "realm scale must be 0.25–3; animation durations at most 600 ticks".into(),
                );
            }
            for text in [
                &style.sleeping,
                &style.watching,
                &style.resurrecting,
                &style.recovered,
                &style.accounted,
                &style.absent,
            ] {
                if text.len() > 512 {
                    return Err("realm labels exceed 512 bytes".into());
                }
            }
            Ok(value)
        }
        "agent"
            if value
                .get("turn_note")
                .and_then(Value::as_str)
                .is_some_and(|v| v.len() <= 8192) =>
        {
            Ok(value)
        }
        // Leg-specific shape belongs to the native caller, which falls back on
        // any mismatch; the host only bounds the value.
        "policy" if !value.is_null() => Ok(value),
        _ => Err(format!("invalid {kind} module result")),
    }
}

enum Request {
    Render(String, Value, mpsc::SyncSender<Result<Value, String>>),
    Reload(mpsc::SyncSender<Result<Value, String>>),
    Dispatch(
        kernel::Kind,
        String,
        Value,
        mpsc::SyncSender<Result<Value, String>>,
    ),
    Complete(kernel::Job, Value, mpsc::SyncSender<Result<Value, String>>),
    Cancel(u64, mpsc::SyncSender<Result<Value, String>>),
}

/// UI-facing mailbox. Polling never performs filesystem access or JavaScript.
pub struct Client {
    pub root: PathBuf,
    tx: Option<mpsc::SyncSender<Request>>,
    shared: Arc<RwLock<Snapshot>>,
    board: Arc<Board>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Client {
    pub fn disabled() -> Self {
        Self {
            root: PathBuf::new(),
            tx: None,
            shared: Default::default(),
            board: Default::default(),
            worker: None,
        }
    }

    pub fn start(root: PathBuf, evaluator: Evaluator) -> Self {
        Self::start_validated(root, evaluator, no_host_validation)
    }

    pub fn start_validated(root: PathBuf, evaluator: Evaluator, validator: Validator) -> Self {
        Self::start_with_resources(root, evaluator, validator, no_resource_binding)
    }
    pub fn start_with_resources(
        root: PathBuf,
        evaluator: Evaluator,
        validator: Validator,
        binder: ResourceBinder,
    ) -> Self {
        let (tx, rx) = mpsc::sync_channel(8);
        let shared = Arc::new(RwLock::new(Snapshot::default()));
        let published = Arc::clone(&shared);
        let board = Arc::new(Board::default());
        let outcomes = Arc::clone(&board);
        let worker_root = root.clone();
        let worker = std::thread::Builder::new()
            .name("angel-live-modules".into())
            .spawn(move || {
                let mut registry =
                    Registry::with_resources(worker_root, evaluator, validator, binder);
                registry.watch = watch::Watch::new();
                registry.refresh(true);
                // Even an empty directory has completed discovery. Revision
                // zero is reserved for the UI's initial, unpublished catalogue.
                registry.revision = registry.revision.max(1);
                let mut last_scan = Instant::now();
                let snapshot = registry.snapshot();
                let scan = Scan::of(&registry, &snapshot);
                *published.write().unwrap_or_else(|p| p.into_inner()) = snapshot;
                outcomes.publish(scan);
                loop {
                    let response = match rx.recv_timeout(if registry.work.is_empty() {
                        POLL
                    } else {
                        Duration::from_millis(1)
                    }) {
                        Ok(Request::Render(id, input, reply)) => {
                            Some((reply, registry.render(&id, &input)))
                        }
                        Ok(Request::Reload(reply)) => {
                            registry.refresh(true);
                            Some((reply, Ok(json!(registry.snapshot().text(&registry.root)))))
                        }
                        Ok(Request::Dispatch(kind, name, event, reply)) => {
                            Some((reply, registry.dispatch(kind, &name, &event)))
                        }
                        Ok(Request::Complete(job, result, reply)) => {
                            Some((reply, registry.complete(&job, result)))
                        }
                        Ok(Request::Cancel(request, reply)) => {
                            Some((reply, registry.cancel(request)))
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let mut scanned = None;
                    if last_scan.elapsed() >= POLL {
                        scanned = Some(registry.refresh_snapshot(false));
                        last_scan = Instant::now();
                    }
                    // With nothing queued, `advance` changes nothing, and a
                    // scan just ended is already the registry as it stands.
                    let idle = response.is_none()
                        && registry.work.is_empty()
                        && registry.completions.is_empty();
                    registry.advance();
                    let snapshot = match scanned {
                        Some(snapshot) if idle => snapshot,
                        _ => registry.snapshot(),
                    };
                    let scan = Scan::of(&registry, &snapshot);
                    *published.write().unwrap_or_else(|p| p.into_inner()) = snapshot;
                    outcomes.publish(scan);
                    // An acknowledged generation is already observable to the
                    // UI, including request-triggered restoration of old code.
                    if let Some((reply, result)) = response {
                        let _ = reply.send(result);
                    }
                }
            });
        match worker {
            Ok(worker) => Self {
                root,
                tx: Some(tx),
                shared,
                board,
                worker: Some(worker),
            },
            Err(error) => {
                let mut snapshot = shared.write().unwrap_or_else(|p| p.into_inner());
                snapshot.error = Some(error.to_string());
                snapshot.revision = 1;
                drop(snapshot);
                Self {
                    root,
                    tx: None,
                    shared,
                    board,
                    worker: None,
                }
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.tx.is_some()
    }

    pub fn snapshot(&self) -> Option<Snapshot> {
        self.shared.try_read().ok().map(|s| s.clone())
    }

    /// What this client's edits became; see [`Board`].
    pub fn board(&self) -> Arc<Board> {
        Arc::clone(&self.board)
    }

    pub fn snapshot_after(&self, revision: u64) -> Option<Snapshot> {
        let snapshot = self.shared.try_read().ok()?;
        (snapshot.revision != revision).then(|| snapshot.clone())
    }

    pub fn request(
        &self,
        id: Option<&str>,
        input: Value,
    ) -> Result<mpsc::Receiver<Result<Value, String>>, String> {
        let tx = self.tx.as_ref().ok_or("live modules are disabled")?;
        let (reply, rx) = mpsc::sync_channel(1);
        let request = match id {
            Some(id) => Request::Render(id.into(), input, reply),
            None => Request::Reload(reply),
        };
        tx.try_send(request)
            .map_err(|_| "live module mailbox is busy".to_string())?;
        Ok(rx)
    }

    pub fn dispatch(
        &self,
        kind: kernel::Kind,
        name: String,
        event: Value,
    ) -> Result<mpsc::Receiver<Result<Value, String>>, String> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.tx
            .as_ref()
            .ok_or("module host is disabled")?
            .try_send(Request::Dispatch(kind, name, event, reply))
            .map_err(|_| "module mailbox is busy")?;
        Ok(rx)
    }

    pub fn cancel(&self, request: u64) -> Result<mpsc::Receiver<Result<Value, String>>, String> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.tx
            .as_ref()
            .ok_or("module host is disabled")?
            .try_send(Request::Cancel(request, reply))
            .map_err(|_| "module mailbox is busy")?;
        Ok(rx)
    }

    pub fn complete(
        &self,
        job: kernel::Job,
        result: Value,
    ) -> Result<mpsc::Receiver<Result<Value, String>>, String> {
        let (reply, rx) = mpsc::sync_channel(1);
        self.tx
            .as_ref()
            .ok_or("module host is disabled")?
            .try_send(Request::Complete(job, result, reply))
            .map_err(|_| "module mailbox is busy")?;
        Ok(rx)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests;

pub mod system;

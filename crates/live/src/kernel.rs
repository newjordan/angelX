//! Runtime-independent module ownership and registration. Feature names are
//! data: adding a command, surface or service does not add a host enum branch.
//! Implementations stage copied state; only commit changes live dispatch.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub const API: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Command,
    Surface,
    Service,
    Tool,
    Resource,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub kind: Kind,
    pub name: String,
    pub contract: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Import {
    pub module: String,
    pub contract: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub state_schema: u32,
    pub exports: Vec<Export>,
    #[serde(default)]
    pub imports: Vec<Import>,
}

impl Manifest {
    pub fn validate(&self) -> Result<(), String> {
        if !name(&self.id)
            || self.state_schema == 0
            || self.exports.is_empty()
            || self.exports.len() > 64
            || self.imports.len() > 64
        {
            return Err("invalid module identity, schema or export/import count".into());
        }
        let mut seen = BTreeSet::new();
        for export in &self.exports {
            if !name(&export.name)
                || !contract(&export.contract)
                || !seen.insert((&export.kind, &export.name))
            {
                return Err(format!(
                    "{}: invalid or duplicate export {}",
                    self.id, export.name
                ));
            }
        }
        let mut seen = BTreeSet::new();
        for import in &self.imports {
            if !name(&import.module)
                || !contract(&import.contract)
                || !seen.insert((&import.module, &import.contract))
            {
                return Err(format!("{}: invalid or duplicate import", self.id));
            }
        }
        Ok(())
    }
}
fn name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}
fn contract(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && s.contains('/') && !s.chars().any(char::is_whitespace)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub schema: u32,
    pub state: Value,
}

/// A backend-neutral surface. Glass/native terminal adapters consume a cached
/// view; module evaluation never runs while a frame is being painted.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PreparedState {
    #[serde(default)]
    pub resources: BTreeMap<String, Value>,
    pub checkpoint: Checkpoint,
    #[serde(default)]
    pub surfaces: BTreeMap<String, Surface>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub service: String,
    pub method: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    #[serde(default)]
    pub resources: BTreeMap<String, Value>,
    pub checkpoint: Checkpoint,
    pub output: Value,
    #[serde(default)]
    pub surfaces: BTreeMap<String, Surface>,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

pub type Binding = Arc<dyn std::any::Any + Send + Sync>;

/// A captured, declared dependency closure. Keys are module/resource names;
/// resources retain native bindings and the code/assets behind their owner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resources(Arc<BTreeMap<String, Resource>>);
impl Resources {
    pub fn get(&self, module: &str, name: &str, contract: &str) -> Result<&Resource, String> {
        self.0
            .get(&resource_key(module, name))
            .filter(|resource| resource.contract == contract)
            .ok_or_else(|| {
                format!("resource {module}/{name} with contract {contract} is not imported")
            })
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Resource)> {
        self.0.iter()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn resolve(manifest: &Manifest, available: &BTreeMap<String, Resource>) -> Self {
        Self(Arc::new(
            available
                .iter()
                .filter(|(_, resource)| {
                    manifest.imports.iter().any(|import| {
                        import.module == resource.module && import.contract == resource.contract
                    })
                })
                .map(|(name, resource)| (name.clone(), resource.clone()))
                .collect(),
        ))
    }
}
pub fn resource_key(module: &str, name: &str) -> String {
    format!("{module}/{name}")
}

pub trait Implementation: Send + Sync {
    /// Return a backend bound to exactly this immutable dependency snapshot.
    /// None means the implementation is independent of dependency contents.
    /// Kernel resources/instances retain these leases even for such backends.
    fn with_resources(
        &self,
        _resources: &Resources,
    ) -> Result<Option<Arc<dyn Implementation>>, String> {
        Ok(None)
    }

    /// Host adapters prepare resources on the worker before a state/code
    /// commit. A binding leases decoded data/bytes for its consumers.
    fn bind_resource(&self, _name: &str, _value: &Value) -> Result<Binding, String> {
        Ok(Arc::new(()))
    }

    /// Initialize/migrate a COPY, without executing host effects.
    fn prepare(&self, previous: Option<&Checkpoint>) -> Result<PreparedState, String>;
    /// Recompute a copied state/view after an imported resource changes. This
    /// must not execute effects or silently rerun code migration.
    fn rebind(&self, _previous: &Checkpoint) -> Result<PreparedState, String> {
        Err("backend does not implement resource dependency rebinding".into())
    }
    fn reduce(&self, checkpoint: &Checkpoint, event: &Value) -> Result<Transition, String>;
    fn instantiate(&self, _context: &Value) -> Result<Checkpoint, String> {
        self.prepare(None).map(|prepared| prepared.checkpoint)
    }
    fn reduce_instance(
        &self,
        checkpoint: &Checkpoint,
        event: &Value,
    ) -> Result<Transition, String> {
        self.reduce(checkpoint, event)
    }
}

#[derive(Clone)]
pub struct Candidate {
    pub manifest: Manifest,
    pub identity: String,
    pub implementation: Arc<dyn Implementation>,
}

struct Active {
    code: Candidate,
    generation: u64,
    revision: u64,
    checkpoint: Checkpoint,
    surfaces: BTreeMap<String, Surface>,
    resources: BTreeMap<String, Resource>,
    dependencies: Resources,
    jobs: BTreeSet<u64>,
}

pub struct PreparedGroup {
    epoch: u64,
    revisions: BTreeMap<String, u64>,
    modules: Vec<PreparedModule>,
}
struct PreparedModule {
    code: Candidate,
    generation: u64,
    revision: u64,
    checkpoint: Checkpoint,
    surfaces: BTreeMap<String, Surface>,
    resources: BTreeMap<String, Resource>,
    dependencies: Resources,
    replace: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Route {
    pub module: String,
    pub generation: u64,
    pub export: Export,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Job {
    pub module: String,
    pub generation: u64,
    pub request: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IssuedEffect {
    pub job: Job,
    pub target: Route,
    pub effect: Effect,
}

struct Work {
    owner: Job,
    target: Route,
    effect: Effect,
    started: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub module: String,
    pub generation: u64,
    pub output: Value,
    pub effects: Vec<IssuedEffect>,
}

#[derive(Clone)]
pub struct Resource {
    pub module: String,
    pub name: String,
    pub generation: u64,
    pub revision: u64,
    pub identity: String,
    pub contract: String,
    pub value: Arc<Value>,
    binding: Binding,
    owner: Candidate,
    dependencies: Resources,
}

impl Resource {
    pub fn instantiate(&self, context: &Value) -> Result<Instance, String> {
        self.owner
            .instance(context, self.generation, self.dependencies.clone())
    }
    pub fn imports(&self) -> &Resources {
        &self.dependencies
    }
    pub fn binding<T: std::any::Any>(&self) -> Option<&T> {
        self.binding.downcast_ref()
    }
}
impl std::fmt::Debug for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resource")
            .field("module", &self.module)
            .field("name", &self.name)
            .field("generation", &self.generation)
            .field("revision", &self.revision)
            .field("contract", &self.contract)
            .finish()
    }
}
impl PartialEq for Resource {
    fn eq(&self, other: &Self) -> bool {
        self.module == other.module
            && self.name == other.name
            && self.generation == other.generation
            && self.revision == other.revision
            && self.identity == other.identity
            && self.contract == other.contract
            && self.value == other.value
    }
}

impl Candidate {
    /// Prepare a computational instance for an adapter's admission probe.
    /// Generation zero is unpublished; live instances come from Resource.
    pub fn probe_instance(&self, context: &Value) -> Result<Instance, String> {
        self.instance(context, 0, Resources::default())
    }
    fn instance(
        &self,
        context: &Value,
        generation: u64,
        dependencies: Resources,
    ) -> Result<Instance, String> {
        if context.to_string().len() > 512 * 1024 {
            return Err("instance context budget exceeded".into());
        }
        let checkpoint = self.implementation.instantiate(context)?;
        validate_state(
            &self.manifest,
            &checkpoint,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )?;
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let serial = SERIAL
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |id| id.checked_add(1),
            )
            .map_err(|_| "instance identities exhausted")?;
        Ok(Instance {
            owner: self.clone(),
            generation,
            serial,
            revision: 0,
            checkpoint,
            _dependencies: dependencies,
        })
    }
}

/// Computational state owned by a host adapter (a cast, animation or task).
/// Stage, validate adapter output, then commit alongside the host mutation.
/// Native service effects continue through the Kernel's issued-job protocol.
pub struct Instance {
    owner: Candidate,
    generation: u64,
    serial: u64,
    revision: u64,
    checkpoint: Checkpoint,
    _dependencies: Resources,
}
pub struct InstanceTransition {
    serial: u64,
    revision: u64,
    checkpoint: Checkpoint,
    pub output: Value,
}
/// An exclusively borrowed instance whose transition and next revision have
/// already been validated. Dropping the guard aborts without changing state.
/// Native adapters can hold it while committing their staged host mutation,
/// then apply the instance transition without a further fallible check.
///
/// The exclusive borrow prevents an intervening instance mutation:
/// ```compile_fail
/// use angel_live::kernel::{Instance, InstanceTransition};
/// fn intervening(instance: &mut Instance, transition: InstanceTransition) {
///     let prepared = instance.prepare_commit(transition).unwrap();
///     let other = instance.stage(&serde_json::Value::Null).unwrap();
///     prepared.commit();
/// }
/// ```
/// A committed guard cannot be committed again:
/// ```compile_fail
/// use angel_live::kernel::{Instance, InstanceTransition};
/// fn twice(instance: &mut Instance, transition: InstanceTransition) {
///     let prepared = instance.prepare_commit(transition).unwrap();
///     prepared.commit();
///     prepared.commit();
/// }
/// ```
#[must_use = "dropping a prepared instance commit aborts the transition"]
pub struct PreparedInstanceCommit<'a> {
    instance: &'a mut Instance,
    transition: InstanceTransition,
    next_revision: u64,
}
impl PreparedInstanceCommit<'_> {
    /// Release the borrow without changing the instance, retaining the exact
    /// computation for retry after an uncertain external acknowledgement.
    pub fn abort(self) -> InstanceTransition {
        self.transition
    }

    /// Apply exactly the prevalidated transition. This consumes the exclusive
    /// guard, so it cannot commit twice or be reused after aborting.
    pub fn commit(self) -> Value {
        let Self {
            instance,
            transition,
            next_revision,
        } = self;
        instance.checkpoint = transition.checkpoint;
        instance.revision = next_revision;
        transition.output
    }
}

impl Instance {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn checkpoint(&self) -> &Checkpoint {
        &self.checkpoint
    }
    pub fn stage(&self, event: &Value) -> Result<InstanceTransition, String> {
        if event.to_string().len() > 512 * 1024 {
            return Err("instance event budget exceeded".into());
        }
        let transition = self
            .owner
            .implementation
            .reduce_instance(&self.checkpoint, event)?;
        validate_state(
            &self.owner.manifest,
            &transition.checkpoint,
            &transition.surfaces,
            &transition.resources,
        )?;
        if !transition.effects.is_empty()
            || !transition.surfaces.is_empty()
            || !transition.resources.is_empty()
            || transition.output.to_string().len() > 64 * 1024
        {
            return Err(
                "computational instances return state and bounded adapter output only".into(),
            );
        }
        Ok(InstanceTransition {
            serial: self.serial,
            revision: self.revision,
            checkpoint: transition.checkpoint,
            output: transition.output,
        })
    }
    /// Validate before a host effect is committed, retaining exclusive access
    /// until this transition is either committed or dropped.
    pub fn prepare_commit(
        &mut self,
        transition: InstanceTransition,
    ) -> Result<PreparedInstanceCommit<'_>, String> {
        self.prepare_commit_recoverable(transition).map_err(|(error, _)| error)
    }
    /// Keep the exact computation on validation failure so an adapter can
    /// quarantine an unresolved frame without losing its retained preparation.
    pub fn prepare_commit_recoverable(
        &mut self,
        transition: InstanceTransition,
    ) -> Result<PreparedInstanceCommit<'_>, (String, InstanceTransition)> {
        if transition.serial != self.serial || transition.revision != self.revision {
            return Err(("stale or foreign instance transition".into(), transition));
        }
        let Some(next_revision) = self.revision.checked_add(1) else {
            return Err(("instance revision exhausted".into(), transition));
        };
        Ok(PreparedInstanceCommit {
            instance: self,
            transition,
            next_revision,
        })
    }
    pub fn commit(&mut self, transition: InstanceTransition) -> Result<Value, String> {
        Ok(self.prepare_commit(transition)?.commit())
    }
}

#[derive(Default)]
pub struct Kernel {
    epoch: u64,
    next_request: u64,
    modules: BTreeMap<String, Active>,
    routes: BTreeMap<(Kind, String), Route>,
    work: BTreeMap<u64, Work>,
}

impl Kernel {
    pub fn routes(&self) -> Vec<Route> {
        self.routes.values().cloned().collect()
    }
    pub fn surfaces(&self) -> BTreeMap<String, Surface> {
        self.modules
            .values()
            .flat_map(|m| m.surfaces.iter().map(|(k, v)| (k.clone(), v.clone())))
            .collect()
    }
    pub fn resources(&self) -> BTreeMap<String, Resource> {
        self.modules
            .values()
            .flat_map(|module| {
                module.resources.values().map(|resource| {
                    (
                        resource_key(&resource.module, &resource.name),
                        resource.clone(),
                    )
                })
            })
            .collect()
    }
    pub fn identity(&self, id: &str) -> Option<&str> {
        self.modules.get(id).map(|m| m.code.identity.as_str())
    }
    pub fn generation(&self, id: &str) -> Option<u64> {
        self.modules.get(id).map(|m| m.generation)
    }
    pub fn state_revision(&self, id: &str) -> u64 {
        self.modules.get(id).map_or(0, |module| module.revision)
    }
    pub fn checkpoint(&self, id: &str) -> Option<Checkpoint> {
        self.modules.get(id).map(|m| m.checkpoint.clone())
    }

    pub fn stage(&self, candidates: Vec<Candidate>) -> Result<PreparedGroup, String> {
        if candidates.is_empty() || candidates.len() > 64 {
            return Err("invalid module transaction size".into());
        }
        let mut replacements = BTreeMap::new();
        for candidate in candidates {
            candidate.manifest.validate()?;
            if candidate.identity.is_empty()
                || candidate.identity.len() > 128
                || replacements
                    .insert(candidate.manifest.id.clone(), candidate)
                    .is_some()
            {
                return Err("invalid/duplicate module generation".into());
            }
        }
        self.prepare_graph(replacements, None)
    }

    fn prepare_graph(
        &self,
        replacements: BTreeMap<String, Candidate>,
        root: Option<PreparedModule>,
    ) -> Result<PreparedGroup, String> {
        let mut codes = self
            .modules
            .iter()
            .map(|(id, module)| (id.clone(), module.code.clone()))
            .collect::<BTreeMap<_, _>>();
        codes.extend(
            replacements
                .iter()
                .map(|(id, code)| (id.clone(), code.clone())),
        );
        if codes.len() > 64 {
            return Err("module registry exceeds 64 entries".into());
        }
        let manifests = codes
            .iter()
            .map(|(id, code)| (id.clone(), &code.manifest))
            .collect();
        let order = validate_graph(&manifests)?;
        let mut available = self.resources();
        let root_id = root.as_ref().map(|root| root.code.manifest.id.clone());
        let mut modules = Vec::new();
        let mut revisions = BTreeMap::new();
        if let Some(root) = root {
            remember(
                &mut revisions,
                &root.code.manifest.id,
                &root.dependencies,
                &self.modules,
            );
            overlay_resources(&mut available, &root.code.manifest.id, &root.resources);
            modules.push(root);
        }
        for id in order {
            if root_id.as_ref() == Some(&id) {
                continue;
            }
            let previous = self.modules.get(&id);
            let mut code = codes[&id].clone();
            let dependencies = Resources::resolve(&code.manifest, &available);
            let replaced = replacements.contains_key(&id);
            if !replaced && previous.is_some_and(|old| old.dependencies == dependencies) {
                continue;
            }
            if previous.is_some_and(|old| !old.jobs.is_empty()) {
                return Err(format!(
                    "{id}: handoff deferred until active work completes"
                ));
            }
            if let Some(bound) = code.implementation.with_resources(&dependencies)? {
                code.implementation = bound;
            }
            let prepared = if replaced {
                code.implementation
                    .prepare(previous.map(|old| &old.checkpoint))?
            } else {
                code.implementation.rebind(&previous.unwrap().checkpoint)?
            };
            validate_state(
                &code.manifest,
                &prepared.checkpoint,
                &prepared.surfaces,
                &prepared.resources,
            )?;
            let generation = match previous {
                Some(old) => old
                    .generation
                    .checked_add(1)
                    .ok_or("module generation exhausted")?,
                None => 1,
            };
            let resources = bind_resources(
                &code,
                generation,
                0,
                &dependencies,
                prepared.resources,
                None,
            )?;
            remember(&mut revisions, &id, &dependencies, &self.modules);
            overlay_resources(&mut available, &id, &resources);
            modules.push(PreparedModule {
                code,
                generation,
                revision: 0,
                checkpoint: prepared.checkpoint,
                surfaces: prepared.surfaces,
                resources,
                dependencies,
                replace: true,
            });
        }
        Ok(PreparedGroup {
            epoch: self.epoch,
            revisions,
            modules,
        })
    }

    pub fn commit(&mut self, group: PreparedGroup) -> Result<(), String> {
        if group.epoch != self.epoch
            || group
                .revisions
                .iter()
                .any(|(id, revision)| self.state_revision(id) != *revision)
            || group.modules.iter().any(|module| {
                module.replace
                    && self
                        .modules
                        .get(&module.code.manifest.id)
                        .is_some_and(|old| !old.jobs.is_empty())
            })
        {
            return Err(
                "module transaction is stale; live state/work changed during preparation".into(),
            );
        }
        let changed_code = group.modules.iter().any(|module| module.replace);
        let next_epoch = if changed_code {
            self.epoch.checked_add(1).ok_or("module epoch exhausted")?
        } else {
            self.epoch
        };
        for module in group.modules {
            let id = module.code.manifest.id.clone();
            let jobs = if module.replace {
                BTreeSet::new()
            } else {
                self.modules
                    .get_mut(&id)
                    .map(|old| std::mem::take(&mut old.jobs))
                    .unwrap_or_default()
            };
            self.modules.insert(
                id,
                Active {
                    code: module.code,
                    generation: module.generation,
                    revision: module.revision,
                    checkpoint: module.checkpoint,
                    surfaces: module.surfaces,
                    resources: module.resources,
                    dependencies: module.dependencies,
                    jobs,
                },
            );
        }
        self.epoch = next_epoch;
        self.routes.clear();
        for (id, module) in &self.modules {
            for export in &module.code.manifest.exports {
                self.routes.insert(
                    (export.kind.clone(), export_key(id, export)),
                    Route {
                        module: id.clone(),
                        generation: module.generation,
                        export: export.clone(),
                    },
                );
            }
        }
        Ok(())
    }

    pub fn dispatch(&mut self, kind: Kind, name: &str, event: &Value) -> Result<Response, String> {
        let route = self
            .routes
            .get(&(kind, name.to_owned()))
            .ok_or_else(|| format!("unregistered module export {name}"))?
            .clone();
        if event.to_string().len() > 512 * 1024 {
            return Err("module event budget exceeded".into());
        }
        let module = &self.modules[&route.module];
        let transition = module
            .code
            .implementation
            .reduce(&module.checkpoint, event)?;
        validate_state(
            &module.code.manifest,
            &transition.checkpoint,
            &transition.surfaces,
            &transition.resources,
        )?;
        if transition.output.to_string().len() > 64 * 1024
            || transition.effects.len() > 32
            || self.work.len() + transition.effects.len() > 128
        {
            return Err("module output/work queue budget exceeded".into());
        }
        let mut targets = Vec::new();
        for effect in &transition.effects {
            let target = self
                .routes
                .get(&(Kind::Service, effect.method.clone()))
                .filter(|target| target.module == effect.service)
                .ok_or("effect service/method is not registered")?;
            if !module.code.manifest.imports.iter().any(|import| {
                import.module == effect.service && import.contract == target.export.contract
            }) {
                return Err("effect does not name a declared service contract".into());
            }
            targets.push(target.clone());
        }
        self.next_request
            .checked_add(transition.effects.len() as u64)
            .ok_or("module request ids exhausted")?;
        let generation = module.generation;
        let revision = module
            .revision
            .checked_add(1)
            .ok_or("module revision exhausted")?;
        let resources = bind_resources(
            &module.code,
            generation,
            revision,
            &module.dependencies,
            transition.resources,
            Some(&module.resources),
        )?;
        let root = PreparedModule {
            code: module.code.clone(),
            generation,
            revision,
            checkpoint: transition.checkpoint,
            surfaces: transition.surfaces,
            resources,
            dependencies: module.dependencies.clone(),
            replace: false,
        };
        let group = self.prepare_graph(BTreeMap::new(), Some(root))?;
        for target in &mut targets {
            if let Some(prepared) = group
                .modules
                .iter()
                .find(|module| module.code.manifest.id == target.module)
            {
                target.generation = prepared.generation;
            }
        }
        self.commit(group)?;
        let mut effects = Vec::new();
        for (effect, target) in transition.effects.into_iter().zip(targets) {
            self.next_request += 1; // Entire range was checked before any commit.
            let job = Job {
                module: route.module.clone(),
                generation,
                request: self.next_request,
            };
            self.modules
                .get_mut(&route.module)
                .unwrap()
                .jobs
                .insert(job.request);
            self.modules
                .get_mut(&target.module)
                .unwrap()
                .jobs
                .insert(job.request);
            self.work.insert(
                job.request,
                Work {
                    owner: job.clone(),
                    target: target.clone(),
                    effect: effect.clone(),
                    started: false,
                },
            );
            effects.push(IssuedEffect {
                job,
                target,
                effect,
            });
        }
        Ok(Response {
            module: route.module,
            generation,
            output: transition.output,
            effects,
        })
    }

    /// A completion can only enter the generation/request that issued it.
    /// Keep the job until its owner consumes the result successfully.
    pub fn complete(&mut self, job: &Job, result: Value) -> Result<Response, String> {
        let work = self
            .work
            .get(&job.request)
            .ok_or("stale or duplicate module completion")?;
        if work.owner != *job || !work.started {
            return Err("completion generation/owner differs or service has not started".into());
        }
        let target = work.target.module.clone();
        let module = self
            .modules
            .get(&job.module)
            .ok_or("completion owner no longer exists")?;
        if module.generation != job.generation {
            return Err("stale module completion".into());
        }
        let route = module
            .code
            .manifest
            .exports
            .iter()
            .find(|e| !matches!(e.kind, Kind::Surface | Kind::Resource))
            .unwrap_or(&module.code.manifest.exports[0])
            .clone();
        let key = export_key(&job.module, &route);
        let response = self.dispatch(
            route.kind,
            &key,
            &serde_json::json!({"type":"completion","request":job.request,"result":result}),
        )?;
        self.work.remove(&job.request);
        self.modules
            .get_mut(&job.module)
            .unwrap()
            .jobs
            .remove(&job.request);
        self.modules
            .get_mut(&target)
            .unwrap()
            .jobs
            .remove(&job.request);
        Ok(response)
    }

    /// Explicitly abandon a failed job without replaying its effects. The host
    /// records/inspects the failure before using this recovery boundary.
    pub fn cancel(&mut self, job: &Job) -> Result<(), String> {
        let work = self
            .work
            .get(&job.request)
            .ok_or("module work no longer exists")?;
        if work.owner != *job {
            return Err("cancellation generation/owner differs".into());
        }
        let work = self.work.remove(&job.request).unwrap();
        self.modules
            .get_mut(&job.module)
            .unwrap()
            .jobs
            .remove(&job.request);
        self.modules
            .get_mut(&work.target.module)
            .unwrap()
            .jobs
            .remove(&job.request);
        Ok(())
    }

    /// Dispatch an issued service once, against the provider generation pinned
    /// at issue time. Both provider and caller remain loaded until completion.
    pub fn invoke(&mut self, issued: &IssuedEffect) -> Result<Response, String> {
        let work = self
            .work
            .get_mut(&issued.job.request)
            .ok_or("module work no longer exists")?;
        if work.owner != issued.job
            || work.target != issued.target
            || work.effect != issued.effect
            || work.started
        {
            return Err("stale or already dispatched module service".into());
        }
        work.started = true;
        self.dispatch(
            Kind::Service,
            &issued.target.export.name,
            &serde_json::json!({
                "type":"service", "caller":issued.job.module, "request":issued.job.request,
                "method":issued.effect.method, "arguments":issued.effect.arguments
            }),
        )
    }
}

fn export_key(module: &str, export: &Export) -> String {
    if export.kind == Kind::Resource {
        resource_key(module, &export.name)
    } else {
        export.name.clone()
    }
}
fn overlay_resources(
    available: &mut BTreeMap<String, Resource>,
    module: &str,
    resources: &BTreeMap<String, Resource>,
) {
    available.retain(|_, resource| resource.module != module);
    available.extend(
        resources
            .values()
            .map(|resource| (resource_key(module, &resource.name), resource.clone())),
    );
}
fn remember(
    revisions: &mut BTreeMap<String, u64>,
    id: &str,
    dependencies: &Resources,
    modules: &BTreeMap<String, Active>,
) {
    for module in std::iter::once(id).chain(
        dependencies
            .iter()
            .map(|(_, resource)| resource.module.as_str()),
    ) {
        revisions.insert(
            module.to_owned(),
            modules.get(module).map_or(0, |active| active.revision),
        );
    }
}
fn bind_resources(
    code: &Candidate,
    generation: u64,
    revision: u64,
    dependencies: &Resources,
    resources: BTreeMap<String, Value>,
    previous: Option<&BTreeMap<String, Resource>>,
) -> Result<BTreeMap<String, Resource>, String> {
    resources
        .into_iter()
        .map(|(name, value)| {
            if let Some(old) = previous.and_then(|previous| previous.get(&name))
                && old.generation == generation
                && old.identity == code.identity
                && *old.value == value
            {
                return Ok((name, old.clone()));
            }
            let export = code
                .manifest
                .exports
                .iter()
                .find(|export| export.kind == Kind::Resource && export.name == name)
                .ok_or("unregistered module resource")?;
            let binding = code.implementation.bind_resource(&name, &value)?;
            let resource = Resource {
                module: code.manifest.id.clone(),
                name: name.clone(),
                generation,
                revision,
                identity: code.identity.clone(),
                contract: export.contract.clone(),
                value: Arc::new(value),
                binding,
                owner: code.clone(),
                dependencies: dependencies.clone(),
            };
            Ok((name, resource))
        })
        .collect()
}

fn validate_state(
    manifest: &Manifest,
    checkpoint: &Checkpoint,
    surfaces: &BTreeMap<String, Surface>,
    resources: &BTreeMap<String, Value>,
) -> Result<(), String> {
    if checkpoint.schema != manifest.state_schema || checkpoint.state.to_string().len() > 512 * 1024
    {
        return Err("invalid/incompatible module checkpoint".into());
    }
    let mut size = 0;
    for (name, surface) in surfaces {
        if !manifest
            .exports
            .iter()
            .any(|e| e.kind == Kind::Surface && e.name == *name)
            || surface.title.len() > 256
            || surface.lines.len() > 512
        {
            return Err("invalid/unregistered module surface".into());
        }
        size += surface.title.len() + surface.lines.iter().map(String::len).sum::<usize>();
    }
    if size > 64 * 1024 {
        return Err("module surface budget exceeded".into());
    }
    let mut size = 0;
    for (name, value) in resources {
        if !manifest
            .exports
            .iter()
            .any(|e| e.kind == Kind::Resource && e.name == *name)
        {
            return Err("unregistered module resource".into());
        }
        size += value.to_string().len();
    }
    if size > 512 * 1024 {
        return Err("module resource budget exceeded".into());
    }
    Ok(())
}

fn validate_graph(all: &BTreeMap<String, &Manifest>) -> Result<Vec<String>, String> {
    let mut routes = BTreeSet::new();
    for manifest in all.values() {
        for export in &manifest.exports {
            if export.kind != Kind::Resource && !routes.insert((&export.kind, &export.name)) {
                return Err(format!("duplicate export {}", export.name));
            }
        }
        for import in &manifest.imports {
            if !all
                .get(&import.module)
                .is_some_and(|m| m.exports.iter().any(|e| e.contract == import.contract))
            {
                return Err(format!(
                    "{} requires {} contract {}",
                    manifest.id, import.module, import.contract
                ));
            }
        }
    }
    fn visit(
        id: &str,
        all: &BTreeMap<String, &Manifest>,
        visiting: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        if done.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id.to_owned()) {
            return Err(format!("module dependency cycle at {id}"));
        }
        for import in &all[id].imports {
            visit(&import.module, all, visiting, done, order)?;
        }
        visiting.remove(id);
        done.insert(id.to_owned());
        order.push(id.to_owned());
        Ok(())
    }
    let (mut visiting, mut done) = (BTreeSet::new(), BTreeSet::new());
    let mut order = Vec::new();
    for id in all.keys() {
        visit(id, all, &mut visiting, &mut done, &mut order)?;
    }
    Ok(order)
}

#[cfg(test)]
mod tests;

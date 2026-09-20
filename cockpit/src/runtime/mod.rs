//! Module runtime for the Rust cockpit host.
//!
//! This layer keeps presentation lifecycle separate from the agent execution
//! spine. The harness, sessions, approvals, tool registry, and loop controller
//! keep their existing contracts; modules describe independently managed
//! cockpit surfaces that can be activated, suspended, focused, and persisted.

use crate::harness::coeffect::{Change, CoeffectStore, Key, Requirement, Value};
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

const DEFAULT_MANIFESTS: &[&str] = &[
    include_str!("../../modules/core.toml"),
    include_str!("../../modules/agent.toml"),
    include_str!("../../modules/artifacts.toml"),
    include_str!("../../modules/shell.toml"),
    include_str!("../../modules/image.toml"),
    include_str!("../../modules/graph.toml"),
];

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModuleId(String);

impl ModuleId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ModuleId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleKind {
    Chat,
    AgentIdentity,
    Media,
    Terminal,
    Image,
    Graph,
    #[serde(rename = "3d")]
    ThreeD,
    Widget,
    PassiveMonitor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleCapability {
    Chat,
    AgentIdentity,
    Media,
    Graph,
    Terminal,
    #[serde(rename = "3d")]
    ThreeD,
    Widget,
    PassiveMonitor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleActivation {
    Startup,
    OnDemand,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleManifest {
    pub id: ModuleId,
    pub title: String,
    pub kind: ModuleKind,
    pub default_rect: WindowRect,
    pub activation: ModuleActivation,
    #[serde(default)]
    pub capabilities: Vec<ModuleCapability>,
    #[serde(default)]
    pub data_sources: Vec<String>,
}

impl ModuleManifest {
    pub fn parse_toml(input: &str) -> Result<Self, String> {
        let manifest: ModuleManifest = toml::from_str(input).map_err(|e| e.to_string())?;
        if manifest.id.as_str().trim().is_empty() {
            return Err("module id cannot be empty".to_string());
        }
        if manifest.title.trim().is_empty() {
            return Err(format!("module {} title cannot be empty", manifest.id));
        }
        if manifest.default_rect.width == 0 || manifest.default_rect.height == 0 {
            return Err(format!(
                "module {} default_rect must have non-zero size",
                manifest.id
            ));
        }
        Ok(manifest)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleState {
    Dormant,
    /// Read-only compatibility with saved layouts from the retired web bridge.
    Loading,
    Active,
    Suspended,
    Failed,
}

impl ModuleState {
    pub fn is_running(self) -> bool {
        matches!(self, Self::Loading | Self::Active)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl WindowRect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn clamp_to(self, bounds: WindowRect) -> Self {
        let width = self.width.min(bounds.width).max(1);
        let height = self.height.min(bounds.height).max(1);
        let max_x = bounds.x.saturating_add(bounds.width.saturating_sub(width));
        let max_y = bounds
            .y
            .saturating_add(bounds.height.saturating_sub(height));
        Self {
            x: self.x.clamp(bounds.x, max_x),
            y: self.y.clamp(bounds.y, max_y),
            width,
            height,
        }
    }
}

impl From<WindowRect> for Rect {
    fn from(value: WindowRect) -> Self {
        Rect::new(value.x, value.y, value.width, value.height)
    }
}

impl From<Rect> for WindowRect {
    fn from(value: Rect) -> Self {
        Self::new(value.x, value.y, value.width, value.height)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleWindow {
    pub rect: WindowRect,
    pub z: u16,
    pub focused: bool,
    pub snapped: bool,
}

impl ModuleWindow {
    fn new(rect: WindowRect, z: u16) -> Self {
        Self {
            rect,
            z,
            focused: false,
            snapped: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleStatus {
    pub id: ModuleId,
    pub title: String,
    pub kind: ModuleKind,
    pub state: ModuleState,
    pub rect: WindowRect,
}

#[derive(Clone, Debug)]
struct ModuleInstance {
    manifest: ModuleManifest,
    state: ModuleState,
    /// What was asked of this module. The *state* is derived from this intent and
    /// the declared data sources holding, never assigned directly.
    intent: ModuleIntent,
    window: ModuleWindow,
}

/// What a context asked of a module — the once-owned input to a derived state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModuleIntent {
    /// Never asked for: dormant, and dormant again if it stops being available.
    Idle,
    /// Asked for: active while its declared sources are present, suspended when
    /// one leaves, and active again when it returns (no restart, no reload).
    Requested,
    /// Stopped by the operator: stays stopped while the sources come and go.
    Stopped,
}

#[derive(Clone, Debug, Default)]
pub struct ModuleHost {
    modules: BTreeMap<ModuleId, ModuleInstance>,
    z_order: Vec<ModuleId>,
    focused: Option<ModuleId>,
    next_z: u16,
    /// `Σ` for the cockpit surfaces: the data sources the host has been told are
    /// present. A manifest's `data_sources` used to be an annotation; here it is the
    /// spec its state is derived from (§3.2 Def 19/21/22).
    data_sources: CoeffectStore,
}

impl ModuleHost {
    pub fn from_default_manifests() -> Result<Self, String> {
        let manifests = DEFAULT_MANIFESTS
            .iter()
            .map(|m| ModuleManifest::parse_toml(m))
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_manifests(manifests)
    }

    pub fn from_manifests(manifests: Vec<ModuleManifest>) -> Result<Self, String> {
        let mut host = Self::default();
        for manifest in manifests {
            host.register(manifest)?;
        }
        let startup_ids = host
            .modules
            .values()
            .filter(|m| m.manifest.activation == ModuleActivation::Startup)
            .map(|m| m.manifest.id.clone())
            .collect::<Vec<_>>();
        for id in startup_ids {
            // A startup module whose declared feed this host has been told is gone
            // stays dormant instead of opening onto nothing; declaring the source
            // later brings it up with no restart.
            if host.available(&id) {
                host.activate(&id)?;
            }
        }
        Ok(host)
    }

    /// The spec a manifest declares: each `data_sources` entry is a required key,
    /// namespaced so a source name cannot collide with a tool/provider key.
    fn spec_of(manifest: &ModuleManifest) -> Requirement {
        Requirement::any(
            manifest
                .data_sources
                .iter()
                .map(|source| Key::resource(&format!("data:{source}"))),
        )
    }

    fn spec(&self, id: &ModuleId) -> Requirement {
        Self::spec_of(&self.modules[id].manifest)
    }

    /// Declare whether a data source is present. `false` is a positive statement
    /// that the feed is gone (the module's dependents suspend); never mentioning a
    /// source leaves it *unknown*, and an unknown source does not disable anything —
    /// a host that has not been told about a feed behaves exactly as it did before
    /// this transfer.
    pub fn declare_data_source(
        &mut self,
        source: &str,
        available: bool,
    ) -> Vec<(ModuleId, ModuleState)> {
        let key = Key::resource(&format!("data:{source}"));
        let change = self.data_sources.bind(key, Value::Flag(available));
        self.react(change)
    }

    /// Whether the sources a module declares are all not-declined.
    fn available(&self, id: &ModuleId) -> bool {
        self.data_sources.declined_keys(&self.spec(id)).is_empty()
    }

    /// The declared sources this host is missing — named in the refusal, so a pane
    /// that does not open can say why.
    fn missing_sources(&self, id: &ModuleId) -> Vec<String> {
        let spec = self.spec(id);
        self.data_sources
            .declined_keys(&spec)
            .into_iter()
            .map(|key| {
                key.as_str()
                    .trim_start_matches("resource:data:")
                    .to_string()
            })
            .collect()
    }

    /// §3.2's reacting (Def 22): classify one change against every module's spec and
    /// move only the modules that must be re-derived. A module that does not name the
    /// changed key is neutral **by independence** (§3.4) and is left alone.
    fn react(&mut self, change: Change) -> Vec<(ModuleId, ModuleState)> {
        let ids: Vec<ModuleId> = self.modules.keys().cloned().collect();
        let mut moved = Vec::new();
        for id in ids {
            // Boundness *and* readiness: a module that names the key re-derives even
            // when a first declaration arrives already declined, because for a surface
            // unknown and declined are different states (`react` calls that step
            // neutral — right for a spec-satisfaction consumer, wrong here).
            if !change.touches(&self.spec(&id)) {
                continue;
            }
            let before = self.modules[&id].state;
            self.derive(&id);
            let after = self.modules[&id].state;
            if before != after {
                moved.push((id, after));
            }
        }
        moved
    }

    /// Derive a module's state from its intent and whether its declared sources are
    /// present. Nothing else assigns `state` — that is what makes the permutations in
    /// the order-independence test converge on the same statuses.
    fn derive(&mut self, id: &ModuleId) {
        let available = self.available(id);
        let intent = self.modules[id].intent;
        let state = match (intent, available) {
            (ModuleIntent::Requested, true) => ModuleState::Active,
            (ModuleIntent::Requested, false) => ModuleState::Suspended,
            (ModuleIntent::Stopped, _) => ModuleState::Suspended,
            (ModuleIntent::Idle, _) => ModuleState::Dormant,
        };
        if let Some(module) = self.modules.get_mut(id) {
            module.state = state;
        }
    }

    pub fn register(&mut self, manifest: ModuleManifest) -> Result<(), String> {
        if self.modules.contains_key(&manifest.id) {
            return Err(format!("duplicate module id {}", manifest.id));
        }
        let z = self.bump_z();
        let id = manifest.id.clone();
        let instance = ModuleInstance {
            intent: ModuleIntent::Idle,
            window: ModuleWindow::new(manifest.default_rect, z),
            manifest,
            state: ModuleState::Dormant,
        };
        self.z_order.push(id.clone());
        self.modules.insert(id, instance);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn ids(&self) -> impl Iterator<Item = &ModuleId> {
        self.modules.keys()
    }

    pub fn status(&self, id: &ModuleId) -> Option<ModuleStatus> {
        let m = self.modules.get(id)?;
        Some(ModuleStatus {
            id: m.manifest.id.clone(),
            title: m.manifest.title.clone(),
            kind: m.manifest.kind,
            state: m.state,
            rect: m.window.rect,
        })
    }

    pub fn statuses(&self) -> Vec<ModuleStatus> {
        self.z_order
            .iter()
            .filter_map(|id| self.status(id))
            .collect::<Vec<_>>()
    }

    pub fn state(&self, id: impl AsRef<str>) -> Option<ModuleState> {
        self.modules
            .get(&ModuleId::new(id.as_ref()))
            .map(|m| m.state)
    }

    #[allow(dead_code)]
    pub fn is_active(&self, id: impl AsRef<str>) -> bool {
        self.state(id) == Some(ModuleState::Active)
    }

    pub fn is_running(&self, id: impl AsRef<str>) -> bool {
        self.state(id).is_some_and(ModuleState::is_running)
    }

    pub fn focused(&self) -> Option<&ModuleId> {
        self.focused.as_ref()
    }

    pub fn activate(&mut self, id: &ModuleId) -> Result<(), String> {
        if !self.modules.contains_key(id) {
            return Err(format!("unknown module '{id}'"));
        }
        let missing = self.missing_sources(id);
        if !missing.is_empty() {
            return Err(format!(
                "module '{id}' needs data source(s) {} that this host does not have",
                missing.join(", ")
            ));
        }
        if let Some(module) = self.modules.get_mut(id) {
            module.intent = ModuleIntent::Requested;
        }
        self.derive(id);
        self.focus(id)?;
        Ok(())
    }

    pub fn suspend(&mut self, id: &ModuleId) -> Result<(), String> {
        if id.as_str() == "core" {
            return Err("core module stays available".to_string());
        }
        {
            let module = self
                .modules
                .get_mut(id)
                .ok_or_else(|| format!("unknown module '{id}'"))?;
            module.intent = ModuleIntent::Stopped;
            module.window.focused = false;
        }
        self.derive(id);
        if self.focused.as_ref() == Some(id) {
            self.focused = Some(ModuleId::new("core"));
            self.mark_focused("core");
        }
        Ok(())
    }

    pub fn focus(&mut self, id: &ModuleId) -> Result<(), String> {
        if !self.modules.contains_key(id) {
            return Err(format!("unknown module '{id}'"));
        }
        let z = self.bump_z();
        if let Some(module) = self.modules.get_mut(id) {
            module.window.z = z;
        }
        self.z_order.retain(|existing| existing != id);
        self.z_order.push(id.clone());
        self.focused = Some(id.clone());
        self.mark_focused(id.as_str());
        Ok(())
    }

    #[allow(dead_code)] // reserved for Phase-B window-drag snapping
    pub fn snap_to_bounds(&mut self, id: &ModuleId, bounds: WindowRect) -> Result<(), String> {
        let module = self
            .modules
            .get_mut(id)
            .ok_or_else(|| format!("unknown module '{id}'"))?;
        module.window.rect = module.window.rect.clamp_to(bounds);
        module.window.snapped = true;
        Ok(())
    }

    pub fn set_rect(&mut self, id: impl AsRef<str>, rect: Rect) {
        let id = ModuleId::new(id.as_ref());
        if let Some(module) = self.modules.get_mut(&id) {
            module.window.rect = rect.into();
        }
    }

    pub fn layout_text(&self) -> String {
        self.statuses()
            .into_iter()
            .map(|s| {
                format!(
                    "{}  {:?}  {:?}  {}x{}+{}+{}",
                    s.id, s.kind, s.state, s.rect.width, s.rect.height, s.rect.x, s.rect.y
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn save_layout(&self, name: &str) -> Result<PathBuf, String> {
        let name = clean_layout_name(name)?;
        let dir = layout_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let path = dir.join(format!("{name}.toml"));
        let profile = LayoutProfile::from_host(self, &name);
        let body = toml::to_string_pretty(&profile).map_err(|e| e.to_string())?;
        std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
        Ok(path)
    }

    pub fn load_layout(&mut self, name: &str) -> Result<PathBuf, String> {
        let name = clean_layout_name(name)?;
        let path = layout_dir().join(format!("{name}.toml"));
        let body =
            std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let profile: LayoutProfile = toml::from_str(&body).map_err(|e| e.to_string())?;
        // Reconcile into a candidate first. Parsing already happens before any
        // mutation; the clone keeps future validation/normalization failures
        // from ever exposing a half-applied frame to the draw loop.
        let mut candidate = self.clone();
        candidate.reconcile_layout(profile)?;
        *self = candidate;
        Ok(path)
    }

    fn reconcile_layout(&mut self, profile: LayoutProfile) -> Result<(), String> {
        // A layout is keyed by stable module id. If a hand-edited or legacy
        // profile repeats one, the final declaration wins deterministically.
        let saved = profile
            .modules
            .into_iter()
            .map(|module| (ModuleId::new(module.id.as_str()), module))
            .collect::<BTreeMap<_, _>>();

        for module in self.modules.values_mut() {
            module.window.focused = false;
        }

        let mut restore_ids: Vec<ModuleId> = Vec::new();
        let mut focus_requests: Vec<(u16, ModuleId)> = Vec::new();
        for (id, saved) in saved {
            let Some(module) = self.modules.get_mut(&id) else {
                continue; // retired/foreign module ids remain inert
            };
            if saved.rect.width == 0 || saved.rect.height == 0 {
                return Err(format!("layout module {id} has a zero-sized rectangle"));
            }
            // A saved layout records what was *asked for*, not what is possible now.
            // The state is re-derived once every intent is restored, so a surface whose
            // declared feed this host no longer has comes back suspended instead of
            // resurrected (§3.2 — this is why the path no longer assigns `state`).
            module.intent = match restored_layout_state(&id, saved.state) {
                ModuleState::Active => ModuleIntent::Requested,
                ModuleState::Suspended => ModuleIntent::Stopped,
                ModuleState::Dormant => ModuleIntent::Idle,
                ModuleState::Loading | ModuleState::Failed => ModuleIntent::Stopped,
            };
            module.window.rect = saved.rect;
            module.window.z = saved.z;
            module.window.snapped = saved.snapped;
            if saved.focused {
                focus_requests.push((saved.z, id.clone()));
            }
            restore_ids.push(id);
        }

        // Every restored intent is in place: derive, so availability decides the
        // state rather than the saved layout.
        for id in &restore_ids {
            self.derive(id);
        }

        // Serialized z values are ordering hints, not trusted counters. Sort
        // with an id tie-break, then make the one valid focus owner topmost.
        self.z_order = self.modules.keys().cloned().collect();
        self.z_order.sort_by(|left, right| {
            let left_z = self.modules.get(left).map(|m| m.window.z).unwrap_or(0);
            let right_z = self.modules.get(right).map(|m| m.window.z).unwrap_or(0);
            (left_z, left).cmp(&(right_z, right))
        });
        // Highest-z request among the surfaces that can actually run wins. Filtering
        // happens here, on the *derived* state: a saved surface whose declared feed is
        // declined derives to Suspended, so it must not take focus from a lower-z
        // surface that is live (the pre-derive state would have answered with what the
        // layout file claimed, not what this host has).
        focus_requests.sort();
        let focused = focus_requests
            .iter()
            .rev()
            .find(|(_, id)| self.is_running(id.as_str()))
            .map(|(_, id)| id.clone())
            .or_else(|| self.is_running("core").then(|| ModuleId::new("core")));
        if let Some(id) = &focused {
            self.z_order.retain(|existing| existing != id);
            self.z_order.push(id.clone());
        }

        // Compact to a fresh monotonic sequence so a hostile u16::MAX or
        // duplicate z cannot make future focus bumps inert.
        for (index, id) in self.z_order.iter().enumerate() {
            if let Some(module) = self.modules.get_mut(id) {
                module.window.z = u16::try_from(index + 1).unwrap_or(u16::MAX);
            }
        }
        self.next_z = u16::try_from(self.z_order.len()).unwrap_or(u16::MAX);

        self.focused = focused.clone();
        if let Some(id) = focused {
            self.mark_focused(id.as_str());
        }
        Ok(())
    }

    fn mark_focused(&mut self, id: &str) {
        for (module_id, module) in &mut self.modules {
            module.window.focused = module_id.as_str() == id;
        }
    }

    fn bump_z(&mut self) -> u16 {
        self.next_z = self.next_z.saturating_add(1);
        self.next_z
    }
}

fn restored_layout_state(id: &ModuleId, saved: ModuleState) -> ModuleState {
    if id.as_str() == "core" {
        return ModuleState::Active;
    }
    match saved {
        // Loading and failure are observations of a prior runtime, not desired
        // configuration. Never resurrect either as a live cockpit surface.
        ModuleState::Loading | ModuleState::Failed => ModuleState::Suspended,
        state => state,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LayoutProfile {
    name: String,
    modules: Vec<LayoutModule>,
}

impl LayoutProfile {
    fn from_host(host: &ModuleHost, name: &str) -> Self {
        let modules = host
            .z_order
            .iter()
            .filter_map(|id| host.modules.get(id))
            .map(|module| LayoutModule {
                id: module.manifest.id.as_str().to_string(),
                state: module.state,
                rect: module.window.rect,
                z: module.window.z,
                focused: module.window.focused,
                snapped: module.window.snapped,
            })
            .collect();
        Self {
            name: name.to_string(),
            modules,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LayoutModule {
    id: String,
    state: ModuleState,
    rect: WindowRect,
    z: u16,
    focused: bool,
    snapped: bool,
}

fn layout_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("ANGEL_LAYOUT_DIR") {
        return PathBuf::from(path);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".angel0").join("layouts")
}

fn clean_layout_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("layout name is required".to_string());
    }
    if trimmed
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
    {
        return Err("layout name may contain only letters, numbers, '.', '-' and '_'".to_string());
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Delegates to the crate-wide test env lock (process env is global — a
    /// module-local lock can't serialize against other modules' env tests).
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::tests::env_lock()
    }

    fn tiny_manifest(id: &str, activation: ModuleActivation) -> ModuleManifest {
        ModuleManifest {
            id: ModuleId::new(id),
            title: id.to_string(),
            kind: ModuleKind::Widget,
            default_rect: WindowRect::new(1, 2, 10, 4),
            activation,
            capabilities: vec![ModuleCapability::Widget],
            data_sources: Vec::new(),
        }
    }

    #[test]
    fn manifest_toml_parses_public_fields() {
        let manifest = ModuleManifest::parse_toml(
            r#"
id = "core"
title = "Core"
kind = "chat"
activation = "startup"
capabilities = ["chat", "widget"]
data_sources = ["harness"]

[default_rect]
x = 1
y = 2
width = 80
height = 20
"#,
        )
        .unwrap();
        assert_eq!(manifest.id.as_str(), "core");
        assert_eq!(manifest.kind, ModuleKind::Chat);
        assert_eq!(manifest.activation, ModuleActivation::Startup);
        assert!(manifest.capabilities.contains(&ModuleCapability::Chat));
        assert_eq!(manifest.default_rect, WindowRect::new(1, 2, 80, 20));
    }

    #[test]
    fn built_in_manifests_load_all_first_party_modules() {
        let host = ModuleHost::from_default_manifests().unwrap();
        for id in ["core", "agent", "artifacts", "shell", "image", "graph"] {
            assert!(host.state(id).is_some(), "missing built-in module {id}");
        }
        assert_eq!(host.state("core"), Some(ModuleState::Active));
        assert_eq!(host.state("shell"), Some(ModuleState::Dormant));
    }

    #[test]
    fn startup_modules_activate_and_lifecycle_transitions_hold_state() {
        let mut host = ModuleHost::from_manifests(vec![
            tiny_manifest("core", ModuleActivation::Startup),
            tiny_manifest("side", ModuleActivation::OnDemand),
        ])
        .unwrap();
        assert_eq!(host.state("core"), Some(ModuleState::Active));
        assert_eq!(host.state("side"), Some(ModuleState::Dormant));

        let side = ModuleId::new("side");
        host.activate(&side).unwrap();
        host.set_rect("side", Rect::new(4, 5, 20, 8));
        host.suspend(&side).unwrap();
        assert_eq!(host.state("side"), Some(ModuleState::Suspended));
        assert_eq!(
            host.status(&side).unwrap().rect,
            WindowRect::new(4, 5, 20, 8)
        );
    }

    #[test]
    fn focus_geometry_z_order_and_snap_are_stable() {
        let mut host = ModuleHost::from_manifests(vec![
            tiny_manifest("a", ModuleActivation::Startup),
            tiny_manifest("b", ModuleActivation::OnDemand),
        ])
        .unwrap();
        let a = ModuleId::new("a");
        let b = ModuleId::new("b");
        host.activate(&b).unwrap();
        assert_eq!(host.focused(), Some(&b));
        host.focus(&a).unwrap();
        assert_eq!(host.focused(), Some(&a));
        host.set_rect("a", Rect::new(99, 99, 50, 50));
        host.snap_to_bounds(&a, WindowRect::new(0, 0, 80, 24))
            .unwrap();
        let rect = host.status(&a).unwrap().rect;
        assert!(rect.x + rect.width <= 80);
        assert!(rect.y + rect.height <= 24);
    }

    #[test]
    fn retired_loading_layout_rows_deserialize_but_cannot_restore_a_web_module() {
        let _guard = env_lock();
        let dir = std::env::temp_dir().join(format!("angel-legacy-layout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_LAYOUT_DIR", &dir) };
        let profile = LayoutProfile {
            name: "legacy".to_string(),
            modules: vec![LayoutModule {
                id: "web-panels".to_string(),
                state: ModuleState::Loading,
                rect: WindowRect::new(4, 5, 60, 20),
                z: 99,
                focused: true,
                snapped: false,
            }],
        };
        std::fs::write(
            dir.join("legacy.toml"),
            toml::to_string_pretty(&profile).unwrap(),
        )
        .unwrap();

        let mut host = ModuleHost::from_manifests(vec![
            tiny_manifest("core", ModuleActivation::Startup),
            tiny_manifest("side", ModuleActivation::OnDemand),
        ])
        .unwrap();
        host.load_layout("legacy").unwrap();
        assert_eq!(host.state("web-panels"), None);
        let side = ModuleId::new("side");
        host.activate(&side).unwrap();
        assert_eq!(host.state("side"), Some(ModuleState::Active));

        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_LAYOUT_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_persistence_roundtrips_module_state() {
        let _guard = env_lock();
        let dir = std::env::temp_dir().join(format!("angel-layout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_LAYOUT_DIR", &dir) };

        let mut host = ModuleHost::from_manifests(vec![
            tiny_manifest("core", ModuleActivation::Startup),
            tiny_manifest("side", ModuleActivation::OnDemand),
        ])
        .unwrap();
        let side = ModuleId::new("side");
        host.activate(&side).unwrap();
        host.set_rect("side", Rect::new(3, 4, 30, 12));
        let path = host.save_layout("night").unwrap();
        assert!(path.exists());

        host.suspend(&side).unwrap();
        host.set_rect("side", Rect::new(1, 1, 5, 5));
        host.load_layout("night").unwrap();
        assert_eq!(host.state("side"), Some(ModuleState::Active));
        assert_eq!(
            host.status(&side).unwrap().rect,
            WindowRect::new(3, 4, 30, 12)
        );

        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_LAYOUT_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A saved layout records what was asked for. A module whose declared feed this
    /// host has been told is gone comes back suspended, not resurrected — the reason
    /// the restore path moves intent and then derives instead of assigning state.
    #[test]
    fn a_restored_layout_cannot_resurrect_a_module_whose_feed_is_declined() {
        let _guard = env_lock();
        let dir = std::env::temp_dir().join(format!("angel-layout-feed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_LAYOUT_DIR", &dir) };

        let manifests = || {
            vec![
                tiny_manifest("core", ModuleActivation::Startup),
                module_with_sources("image", ModuleActivation::Startup, &["viewer"]),
            ]
        };
        let saved = ModuleHost::from_manifests(manifests()).unwrap();
        assert_eq!(saved.state("image"), Some(ModuleState::Active));
        saved.save_layout("feeds").unwrap();

        let mut restored = ModuleHost::from_manifests(manifests()).unwrap();
        restored.declare_data_source("viewer", false);
        restored.load_layout("feeds").unwrap();
        assert_eq!(
            restored.state("image"),
            Some(ModuleState::Suspended),
            "a saved Active row is a request, not a resurrection"
        );
        assert_eq!(restored.state("core"), Some(ModuleState::Active));

        // Declaring the feed present afterwards brings it up, still with no restart.
        restored.declare_data_source("viewer", true);
        assert_eq!(restored.state("image"), Some(ModuleState::Active));

        unsafe { std::env::remove_var("ANGEL_LAYOUT_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_reconciliation_normalizes_runtime_state_focus_and_z_order() {
        let _guard = env_lock();
        let dir =
            std::env::temp_dir().join(format!("angel-layout-reconcile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_LAYOUT_DIR", &dir) };

        let profile = LayoutProfile {
            name: "reconcile".to_string(),
            modules: vec![
                LayoutModule {
                    id: "core".to_string(),
                    state: ModuleState::Failed,
                    rect: WindowRect::new(0, 0, 80, 24),
                    z: u16::MAX - 2,
                    focused: true,
                    snapped: false,
                },
                LayoutModule {
                    id: "side".to_string(),
                    state: ModuleState::Active,
                    rect: WindowRect::new(1, 2, 20, 8),
                    z: 7,
                    focused: true,
                    snapped: false,
                },
                LayoutModule {
                    id: "side".to_string(),
                    state: ModuleState::Loading,
                    rect: WindowRect::new(3, 4, 30, 12),
                    z: u16::MAX,
                    focused: true,
                    snapped: true,
                },
                LayoutModule {
                    id: "other".to_string(),
                    state: ModuleState::Active,
                    rect: WindowRect::new(5, 6, 22, 9),
                    z: u16::MAX - 1,
                    focused: true,
                    snapped: false,
                },
            ],
        };
        std::fs::write(
            dir.join("reconcile.toml"),
            toml::to_string_pretty(&profile).unwrap(),
        )
        .unwrap();

        let mut host = ModuleHost::from_manifests(vec![
            tiny_manifest("core", ModuleActivation::Startup),
            tiny_manifest("side", ModuleActivation::OnDemand),
            tiny_manifest("other", ModuleActivation::OnDemand),
        ])
        .unwrap();
        host.activate(&ModuleId::new("side")).unwrap();
        host.load_layout("reconcile").unwrap();

        assert_eq!(host.state("core"), Some(ModuleState::Active));
        assert_eq!(host.state("side"), Some(ModuleState::Suspended));
        assert_eq!(host.state("other"), Some(ModuleState::Active));
        assert_eq!(
            host.focused(),
            Some(&ModuleId::new("other")),
            "highest-z running focus request wins; stale Loading focus is ignored"
        );
        assert_eq!(
            host.modules
                .values()
                .filter(|module| module.window.focused)
                .count(),
            1,
            "a loaded layout must publish one focus owner"
        );
        assert_eq!(host.next_z, 3, "serialized z values must be compacted");
        assert_eq!(
            host.z_order.last(),
            Some(&ModuleId::new("other")),
            "the unique focus owner must also be topmost"
        );

        host.activate(&ModuleId::new("side")).unwrap();
        assert_eq!(host.next_z, 4, "focus must advance beyond restored z order");
        assert_eq!(host.z_order.last(), Some(&ModuleId::new("side")));

        let before_text = host.layout_text();
        let before_focus = host.focused().cloned();
        let before_z_order = host.z_order.clone();
        let before_next_z = host.next_z;
        let invalid = LayoutProfile {
            name: "invalid".to_string(),
            modules: vec![LayoutModule {
                id: "side".to_string(),
                state: ModuleState::Active,
                rect: WindowRect::new(9, 9, 0, 12),
                z: 9,
                focused: false,
                snapped: false,
            }],
        };
        std::fs::write(
            dir.join("invalid.toml"),
            toml::to_string_pretty(&invalid).unwrap(),
        )
        .unwrap();
        assert!(
            host.load_layout("invalid")
                .expect_err("invalid geometry must be rejected")
                .contains("zero-sized")
        );
        assert_eq!(host.layout_text(), before_text);
        assert_eq!(host.focused(), before_focus.as_ref());
        assert_eq!(host.z_order, before_z_order);
        assert_eq!(host.next_z, before_next_z);

        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_LAYOUT_DIR") };
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn module_with_sources(
        id: &str,
        activation: ModuleActivation,
        sources: &[&str],
    ) -> ModuleManifest {
        ModuleManifest {
            data_sources: sources.iter().map(|source| source.to_string()).collect(),
            ..tiny_manifest(id, activation)
        }
    }

    /// A declared feed that goes away suspends exactly its dependents; declaring it
    /// again brings them back with no restart — and never mentioning a source leaves
    /// it unknown, which does not disable anything.
    #[test]
    fn declining_a_declared_source_suspends_exactly_its_dependents() {
        let mut host = ModuleHost::from_manifests(vec![
            module_with_sources("core", ModuleActivation::Startup, &["harness"]),
            module_with_sources("image", ModuleActivation::Startup, &["viewer"]),
            tiny_manifest("graph", ModuleActivation::Startup),
        ])
        .unwrap();
        assert_eq!(host.state("image"), Some(ModuleState::Active));

        let moved = host.declare_data_source("viewer", false);
        assert_eq!(host.state("image"), Some(ModuleState::Suspended));
        assert_eq!(
            host.state("core"),
            Some(ModuleState::Active),
            "unrelated feed"
        );
        assert_eq!(
            host.state("graph"),
            Some(ModuleState::Active),
            "no declaration, no gate"
        );
        assert_eq!(
            moved,
            vec![(ModuleId::new("image"), ModuleState::Suspended)]
        );

        let moved = host.declare_data_source("viewer", true);
        assert_eq!(host.state("image"), Some(ModuleState::Active));
        assert_eq!(moved, vec![(ModuleId::new("image"), ModuleState::Active)]);
    }

    /// An unavailable feed is named when a module is asked to open: the refusal is the
    /// receipt, and intent is untouched, so the module is still dormant afterwards.
    #[test]
    fn activating_a_module_with_a_declined_source_refuses_and_names_the_feed() {
        let mut host = ModuleHost::from_manifests(vec![
            tiny_manifest("core", ModuleActivation::Startup),
            module_with_sources("image", ModuleActivation::OnDemand, &["viewer"]),
        ])
        .unwrap();
        host.declare_data_source("viewer", false);
        let image = ModuleId::new("image");
        let err = host.activate(&image).unwrap_err();
        assert!(err.contains("viewer"), "refusal must name the feed: {err}");
        assert_eq!(host.state("image"), Some(ModuleState::Dormant));

        host.declare_data_source("viewer", true);
        host.activate(&image).unwrap();
        assert_eq!(host.state("image"), Some(ModuleState::Active));
    }

    /// One feed, two dependents: sharing a source couples exactly those modules and
    /// nothing else — the `viewer` shape `image` and `artifacts` declare.
    #[test]
    fn a_shared_source_suspends_both_of_its_dependents_only() {
        let mut host = ModuleHost::from_manifests(vec![
            tiny_manifest("core", ModuleActivation::Startup),
            module_with_sources("image", ModuleActivation::Startup, &["viewer"]),
            module_with_sources("artifacts", ModuleActivation::Startup, &["viewer"]),
            module_with_sources("shell", ModuleActivation::Startup, &["portable-pty"]),
        ])
        .unwrap();

        let moved = host.declare_data_source("viewer", false);
        assert_eq!(host.state("image"), Some(ModuleState::Suspended));
        assert_eq!(host.state("artifacts"), Some(ModuleState::Suspended));
        assert_eq!(host.state("shell"), Some(ModuleState::Active));
        assert_eq!(host.state("core"), Some(ModuleState::Active));
        assert_eq!(moved.len(), 2, "only the two dependents moved");

        host.declare_data_source("viewer", true);
        assert_eq!(host.state("image"), Some(ModuleState::Active));
        assert_eq!(host.state("artifacts"), Some(ModuleState::Active));
    }

    /// Availability moves the state; it does not rewrite intent. A module the operator
    /// stopped stays stopped across an outage and its return.
    #[test]
    fn an_outage_does_not_resurrect_a_stopped_module() {
        let mut host = ModuleHost::from_manifests(vec![
            tiny_manifest("core", ModuleActivation::Startup),
            module_with_sources("image", ModuleActivation::Startup, &["viewer"]),
        ])
        .unwrap();
        let image = ModuleId::new("image");
        host.suspend(&image).unwrap();
        assert_eq!(host.state("image"), Some(ModuleState::Suspended));

        host.declare_data_source("viewer", false);
        host.declare_data_source("viewer", true);
        assert_eq!(
            host.state("image"),
            Some(ModuleState::Suspended),
            "the feed returned; the operator's stop stands"
        );

        host.activate(&image).unwrap();
        assert_eq!(host.state("image"), Some(ModuleState::Active));
    }

    /// Order independence (§3.4): changes to distinct keys commute, and a repeated key
    /// converges on its last value — so a script whose permutations all *finish* with
    /// the same value per key reaches the same statuses in every order. (Reordering a
    /// script that ends on different values for one key is a different script; the fixed
    /// suffix below is what makes the orders comparable.)
    #[test]
    fn declaration_scripts_converge_in_every_order() {
        fn script(order: &[u8]) -> Vec<(String, ModuleState)> {
            let mut host = ModuleHost::from_manifests(vec![
                module_with_sources("core", ModuleActivation::Startup, &["harness"]),
                module_with_sources("image", ModuleActivation::Startup, &["viewer", "harness"]),
                module_with_sources("graph", ModuleActivation::Startup, &["dotmax"]),
            ])
            .unwrap();
            for step in order {
                let source = match step {
                    0 => "viewer",
                    1 => "harness",
                    2 => "dotmax",
                    other => unreachable!("unknown step {other}"),
                };
                host.declare_data_source(source, false);
            }
            // A fixed suffix, in a fixed order: every permutation of the prefix ends
            // with the same value in `Σ` for every key, so the statuses must agree.
            host.declare_data_source("harness", false);
            host.declare_data_source("harness", true);
            ["core", "image", "graph"]
                .iter()
                .map(|id| (id.to_string(), host.state(id).unwrap()))
                .collect()
        }

        let expected = vec![
            // `harness` was turned off and on again by the fixed suffix: the last
            // value wins, and the outage in between leaves no trace.
            ("core".to_string(), ModuleState::Active),
            // `viewer` is still declined, and no order of the prefix may lose that.
            ("image".to_string(), ModuleState::Suspended),
            ("graph".to_string(), ModuleState::Suspended),
        ];
        assert_eq!(script(&[0, 1, 2]), expected);
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            assert_eq!(script(&order), expected, "order {order:?} diverged");
        }
    }

    /// The shipped manifests are the real data this leg runs on: `artifacts` (startup)
    /// and `image` (on-demand) both name the `viewer` feed, and no other module does.
    /// A host told the viewer feed is gone suspends exactly `artifacts`, refuses to open
    /// `image` by naming the feed, and leaves the surfaces whose feeds it was never told
    /// about (`core`'s harness/session/transcript, `agent`'s bag/overwatch/agent-profile,
    /// `graph`'s dotmax) exactly as they were. Declaring the feed back restores both with
    /// no restart.
    #[test]
    fn the_shipped_manifests_couple_image_and_artifacts_through_the_viewer_feed() {
        let mut host = ModuleHost::from_default_manifests().unwrap();
        let image = ModuleId::new("image");
        assert_eq!(host.state("artifacts"), Some(ModuleState::Active));
        assert_eq!(host.state("agent"), Some(ModuleState::Active));
        assert_eq!(host.state("image"), Some(ModuleState::Dormant));

        let moved = host.declare_data_source("viewer", false);
        assert_eq!(
            moved,
            vec![(ModuleId::new("artifacts"), ModuleState::Suspended)],
            "only the startup dependent of the viewer feed moves"
        );
        assert_eq!(host.state("agent"), Some(ModuleState::Active));
        let refusal = host.activate(&image).unwrap_err();
        assert!(
            refusal.contains("viewer") && refusal.contains("image"),
            "the refusal names surface and feed: {refusal}"
        );
        assert_eq!(host.state("image"), Some(ModuleState::Dormant));

        host.declare_data_source("viewer", true);
        assert_eq!(host.state("artifacts"), Some(ModuleState::Active));
        host.activate(&image).unwrap();
        assert_eq!(host.state("image"), Some(ModuleState::Active));
    }
}

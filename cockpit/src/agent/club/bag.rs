//! The Bag orchestrator: tabs, slots, agents, and driver resolution.

use super::*;

/// Process-wide `ANGEL_FALLBACK` gate. The draw path asks every frame; the
/// turn path asks on submit. Live toggles via `/experimental fallback` update
/// both the env var and this bit so neither path re-reads the process env.
static FALLBACK_ARMED: AtomicBool = AtomicBool::new(false);
static FALLBACK_SEEDED: AtomicBool = AtomicBool::new(false);

fn seed_fallback_from_env() {
    if FALLBACK_SEEDED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        FALLBACK_ARMED.store(
            std::env::var_os("ANGEL_FALLBACK").is_some(),
            Ordering::Relaxed,
        );
    }
}

/// Whether failover is armed (exact selection off).
pub(crate) fn fallback_armed() -> bool {
    seed_fallback_from_env();
    FALLBACK_ARMED.load(Ordering::Relaxed)
}

/// Sync the process-wide bit when `/experimental fallback` flips the env.
/// Call after `set_var` / `remove_var` so the draw path sees the change.
pub(crate) fn set_fallback_armed(on: bool) {
    FALLBACK_SEEDED.store(true, Ordering::Relaxed);
    FALLBACK_ARMED.store(on, Ordering::Relaxed);
}

fn fallback_include_sota_from_env() -> bool {
    std::env::var("ANGEL_FALLBACK_INCLUDE_SOTA")
        .map(|v| {
            matches!(
                v.trim(),
                "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
            )
        })
        .unwrap_or(false)
}

fn fallback_include_sota() -> bool {
    #[cfg(not(test))]
    {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(fallback_include_sota_from_env)
    }
    #[cfg(test)]
    fallback_include_sota_from_env()
}

/// One entry in the live tab strip — an **agent** (a box/PC), not a raw model.
/// Built by [`Bag::tabs`] from live availability; offline boxes are simply absent.
/// The bag is two-level: `Tab` cycles agents (this strip), and within an agent a
/// subcontrol cycles its modes — so one machine that serves several models/modes
/// shows up as ONE tab, not a cluttered row of near-duplicates.
#[derive(Debug, Clone)]
pub struct ClubTab {
    /// The box name (e.g. `spark`, `atlas`).
    pub label: String,
    /// The active mode within this box, shown only when the box has more than one
    /// (e.g. `spark` → `swarm`). `None` for single-model boxes — no clutter.
    pub mode: Option<String>,
    /// The agent currently in hand (where a message would go).
    pub in_hand: bool,
    /// Box reachable per the last probe (any of its modes up). Always `true`
    /// except the in-hand box, shown even while down so the strip never hides it.
    pub available: bool,
}

/// Mode/effort snapshot for one frame of header chrome. Cached on the Bag as
/// an `Arc` so `FrameChrome::compute` is a pointer clone, not two String clones.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InHandChrome {
    pub mode: Option<String>,
    pub effort: Option<String>,
    pub effort_selectable: bool,
    /// Spawn identity for the in-hand club. Built on the chrome miss so Enter
    /// can clone it instead of taking model/effort locks again.
    pub route: RouteIdentity,
}

/// Generation, agent, slot, route revision, and cached chrome.
type InHandChromeCacheEntry = (usize, usize, usize, u64, std::sync::Arc<InHandChrome>);

/// One row in the operator-facing Brain Route deck. Indices are stable only for
/// the current Bag snapshot and are validated again by [`Bag::select_route`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RouteChoice {
    pub(crate) agent_index: usize,
    pub(crate) slot_index: usize,
    pub(crate) agent: String,
    pub(crate) driver: String,
    pub(crate) model: String,
    pub(crate) available: bool,
    pub(crate) selected: bool,
    pub(crate) reasoning_effort: Option<String>,
    /// Shared ladder slice — `Arc` so a route-choices cache miss does not
    /// allocate a fresh `Vec` of effort strings per fleet slot every rebuild
    /// (the common levels live in process-static OnceLocks on HttpClub).
    pub(crate) reasoning_levels: Arc<[String]>,
    pub(crate) metadata: RouteMetadata,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RouteChoiceStamp {
    available: bool,
    route_state_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TabStamp {
    available: bool,
    in_hand: bool,
    active: usize,
    route_state_revision: u64,
}

/// One selectable backend within an agent — a model or a mode (e.g. `swarm`).
pub(crate) struct Slot {
    pub(crate) label: String,
    pub(crate) club: Arc<dyn Club>,
    /// Live reachability, flipped off the UI thread by the prober.
    pub(crate) available: Arc<AtomicBool>,
}

impl Slot {
    /// What the UI calls this slot: the live checkpoint id (short form) when the
    /// club follows its backend — so the strip shows what a rig serves *now*,
    /// not a stale mode hint — else the static label (`swarm`, env-pinned modes,
    /// SOTA aliases).
    pub(crate) fn display_label(&self) -> String {
        self.club
            .live_model_name()
            .map(|id| short_model_label(&id))
            .unwrap_or_else(|| self.label.clone())
    }

    fn model_identity(&self) -> String {
        self.club
            .model_identity()
            .unwrap_or_else(|| self.display_label())
    }
}

/// Boxes the automated selection paths (delegation roster, consult/spawn
/// panels, failover) must never route to. Interactive Tab selection is
/// untouched — an operator can still point at any box deliberately.
/// `ANGEL_BANNED_BOXES` (comma-separated box names) sets the list; empty
/// clears. There is no compiled exclusion list.
fn banned_boxes_from_env() -> std::sync::Arc<[String]> {
    match std::env::var("ANGEL_BANNED_BOXES") {
        Ok(list) => list
            .split(',')
            .map(|name| name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .into(),
        Err(_) => std::sync::Arc::from([]),
    }
}

pub(crate) fn banned_boxes() -> std::sync::Arc<[String]> {
    #[cfg(not(test))]
    {
        static BOXES: std::sync::OnceLock<std::sync::Arc<[String]>> = std::sync::OnceLock::new();
        std::sync::Arc::clone(BOXES.get_or_init(banned_boxes_from_env))
    }
    #[cfg(test)]
    banned_boxes_from_env()
}

/// An **agent** = a box/PC. `Tab` cycles agents; within one, the subcontrol
/// (`←/→`) cycles `slots` (the box's models/modes). Related routes share one
/// agent entry.
pub(crate) struct Agent {
    pub(crate) name: String,
    pub(crate) slots: Vec<Slot>,
    /// Index into `slots` — the model/mode this box would answer with.
    pub(crate) active: usize,
}

impl Agent {
    /// Reachable if *any* of its modes is up.
    pub(crate) fn available(&self) -> bool {
        self.slots
            .iter()
            .any(|s| s.available.load(Ordering::Relaxed))
    }

    fn active_available(&self) -> bool {
        self.slots
            .get(self.active)
            .map(|s| s.available.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    /// If the active mode went offline, drop to the first reachable one (so a box
    /// that's up via a different port is still usable). No-op if active is up.
    fn settle_active(&mut self) {
        if self.active_available() {
            return;
        }
        if let Some(si) = self
            .slots
            .iter()
            .position(|s| s.available.load(Ordering::Relaxed))
        {
            self.active = si;
        }
    }

    /// Move to the next/previous *reachable* mode, skipping offline ones. Single-
    /// mode boxes stay put.
    fn cycle_active(&mut self, forward: bool) {
        let n = self.slots.len();
        if n <= 1 {
            return;
        }
        for step in 1..=n {
            let idx = if forward {
                (self.active + step) % n
            } else {
                (self.active + n - step) % n
            };
            if self.slots[idx].available.load(Ordering::Relaxed) {
                self.active = idx;
                return;
            }
        }
    }

    fn active_club(&self) -> Arc<dyn Club> {
        let i = self.active.min(self.slots.len().saturating_sub(1));
        Arc::clone(&self.slots[i].club)
    }

    fn active_club_ref(&self) -> &dyn Club {
        let i = self.active.min(self.slots.len().saturating_sub(1));
        &*self.slots[i].club
    }

    fn active_label(&self) -> String {
        let i = self.active.min(self.slots.len().saturating_sub(1));
        self.slots[i].display_label()
    }
}

/// Resolve `ANGEL_DRIVER` to a starting box, setting that box's active mode. A
/// value can name a mode (`swarm`/`gemma`/`coder`/`r1`/`turbo`) or a box; Atlas
/// is only available when `ANGEL_ATLAS_MODEL_SERVING=1`. `deli` folds into
/// `swarm`, `spark-r1` into `r1`. Returns the agent index.
pub(crate) fn resolve_driver(agents: &mut [Agent], pref: &str) -> Option<usize> {
    let raw_pref = pref.trim();
    let normalized = raw_pref.to_ascii_lowercase().replace(['_', ' '], "-");
    let pref = match normalized.as_str() {
        "deli" => "swarm",
        "spark-r1" => "r1",
        // Local ds4-on-spark DeepSeek-V4-Flash (distinct from cloud deepseek-flash).
        "ds4" | "ds-flash" | "dsflash-local" | "spark-flash" | "deepseek-flash-local" => "dsflash",
        "gpu" | "gpu-comp" | "gpu-comp-local-moa" | "gpu-local-moa" | "overnight" => {
            "gpu-comp-local-moa"
        }
        "math" | "math-god" | "mathgod" => "mathgod",
        "or" | "open-router" | "openrouter-free" => "openrouter",
        _ => raw_pref,
    };
    // Every Codex catalog slot has club label "openai". Both the explicit
    // OAuth route and the implicit interactive default must preserve the slot
    // selected from env/config, rather than electing the first cache entry.
    if pref == "openai"
        && let Some(ai) = agents.iter().position(|a| a.name == "openai")
    {
        return Some(ai);
    }
    for (ai, a) in agents.iter_mut().enumerate() {
        // Match the static mode label, the club's own label, or the live
        // checkpoint id (full or short form) — so `ANGEL_DRIVER` can name a
        // model the operator just pulled up, not only a baked-in mode hint.
        if let Some(si) = a.slots.iter().position(|s| {
            s.label == pref
                || s.club.label() == pref
                || s.club
                    .live_model_name()
                    .is_some_and(|id| id == pref || short_model_label(&id) == pref)
        }) {
            a.active = si;
            return Some(ai);
        }
    }
    agents.iter().position(|a| a.name == pref)
}

#[cfg(test)]
include!("../../../../tests/cockpit/club/bag__standalone_tests.rs");

pub(crate) fn direct_sota_agent(
    agent_index: usize,
    links: &[(String, Arc<dyn Club>, Arc<AtomicBool>)],
    meta: &mut Vec<SlotMeta>,
) -> Agent {
    let slots = links
        .iter()
        .enumerate()
        .map(|(slot, (alias, club, available))| {
            if alias == "longcat" {
                meta.push(SlotMeta {
                    agent: agent_index,
                    slot,
                    model: club.label().to_string(),
                    n_params: None,
                    is_swarm: false,
                    auto: true,
                    available: Arc::clone(available),
                });
            }
            Slot {
                label: alias.clone(),
                club: Arc::clone(club),
                available: Arc::clone(available),
            }
        })
        .collect();
    Agent {
        name: "sota".to_string(),
        slots,
        active: 0,
    }
}

pub struct Bag {
    /// Agents (boxes), each owning its modes. `Tab` cycles these.
    pub(crate) agents: Vec<Agent>,
    /// Index into `agents` — the box in hand.
    pub(crate) in_hand: usize,
    /// Boxes discovered on the tailnet AFTER startup by the background scanner,
    /// streamed in so `Bag::standard()` stays instant (the <50ms bootstrap). Drained
    /// onto the UI thread by [`Bag::drain_discovered`], so the bag stays single-
    /// threaded and the `&str` accessors keep working. `None` for the test/practice
    /// constructors (no scanner, nothing to drain). Each carries its model id so the
    /// brain election can score it.
    pub(crate) discovery_rx: Option<std::sync::mpsc::Receiver<(Agent, String)>>,
    /// Per-slot scoring metadata for the brain election (smartest *reachable* model).
    /// Grows as discovered boxes fold in. Only `auto` slots are election-eligible —
    /// practice and the cloud tabs are excluded so the auto-pick never jumps to them.
    pub(crate) meta: Vec<SlotMeta>,
    /// Explicit `ANGEL_DRIVER` preference, honored once it's reachable.
    pub(crate) driver_pref: Option<String>,
    /// True until the brain has committed to a real reachable model. While true, each
    /// tick re-elects the smartest reachable model the moment one confirms up — so a
    /// launch sits on the local practice floor (never a broken endpoint) and upgrades
    /// to a real model as soon as one loads. Cleared on commit and on a manual `Tab`
    /// (the user's pick is then respected); re-armed if the brain drifts back to the
    /// floor because everything real went down.
    pub(crate) pending_brain: bool,
    /// Generation counter incremented whenever agents or selection change,
    /// so we can memoize `route_choices`.
    pub(crate) generation: usize,
    /// Cached route choices: `(generation, per-route stamps, choices)`.
    #[allow(clippy::type_complexity)]
    pub(crate) cached_route_choices: std::sync::Mutex<
        Option<(
            usize,
            Vec<RouteChoiceStamp>,
            std::sync::Arc<Vec<RouteChoice>>,
        )>,
    >,
    /// Cached tab strip: `(generation, per-box stamps, tabs)`. The bay title
    /// asks every frame; an unchanged snapshot is an `Arc` clone.
    #[allow(clippy::type_complexity)]
    pub(crate) cached_tabs:
        std::sync::Mutex<Option<(usize, Vec<TabStamp>, std::sync::Arc<Vec<ClubTab>>)>>,
    /// Cached mode/effort chrome. Keyed on generation, in-hand box, active
    /// slot, and that slot's route revision so THINK and follow-backend
    /// changes still invalidate. Stored as `Arc` so one frame snapshot is a
    /// pointer clone, not two String clones.
    pub(crate) cached_in_hand_mode: std::sync::Mutex<Option<InHandChromeCacheEntry>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BootstrapSlotSpec {
    label: &'static str,
    env_key: &'static str,
    port: u16,
    is_swarm: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BootstrapBoxSpec {
    name: String,
    host: String,
    fallback_ip: String,
    slots: Vec<BootstrapSlotSpec>,
}

impl Bag {
    /// Build the portable local bootstrap and explicitly configured legacy
    /// slots without consulting the live network. Legacy machine names and
    /// port maps are intentionally absent unless their URL is pinned.
    fn bootstrap_specs(getenv: impl Fn(&str) -> Option<String>) -> Vec<BootstrapBoxSpec> {
        let gemma_configured = getenv("ANGEL_GEMMA_URL").is_some_and(|url| !url.trim().is_empty());
        let local_slots = vec![
            BootstrapSlotSpec {
                label: "local",
                env_key: "LOCAL",
                port: 8080,
                is_swarm: false,
            },
            BootstrapSlotSpec {
                label: if gemma_configured {
                    "local-swarm"
                } else {
                    "swarm"
                },
                env_key: "LOCAL",
                port: 8080,
                is_swarm: true,
            },
        ];
        let mut boxes = vec![BootstrapBoxSpec {
            name: "local".to_string(),
            host: String::new(),
            fallback_ip: "127.0.0.1".to_string(),
            slots: local_slots,
        }];

        // Preserve the old selectable agent groups only when an operator pins
        // their endpoint. Explicit URLs own routing, so these groups have no
        // machine host or guessed port and cannot probe a private default.
        let mut add_group = |name: &str, entries: &[(&'static str, &'static str, bool)]| {
            let slots = entries
                .iter()
                .filter(|(_, env_key, _)| {
                    let key = format!("ANGEL_{env_key}_URL");
                    getenv(&key).is_some_and(|url| !url.trim().is_empty())
                })
                .map(|&(label, env_key, is_swarm)| BootstrapSlotSpec {
                    label,
                    env_key,
                    port: 0,
                    is_swarm,
                })
                .collect::<Vec<_>>();
            if !slots.is_empty() {
                boxes.push(BootstrapBoxSpec {
                    name: name.to_string(),
                    host: String::new(),
                    fallback_ip: String::new(),
                    slots,
                });
            }
        };
        add_group(
            "spark",
            &[
                ("spark", "SPARK", false),
                ("swarm", "GEMMA", true),
                ("gemma", "GEMMA", false),
                ("dsflash", "DSFLASH", false),
                ("qwen38", "QWEN38", false),
                ("coder", "SPARK", false),
                ("r1", "SPARK_R1", false),
                ("leanstral", "LEANSTRAL", false),
            ],
        );
        add_group("turbo", &[("turbo", "TURBO", false)]);
        add_group("toymaker", &[("ornith", "ORNITH", false)]);
        boxes
    }

    /// Build the standard bag from env and live `/models` discovery: one **agent
    /// per box/PC**, each owning the model endpoints that box serves. `Tab` cycles
    /// boxes; `←/→` cycles a box's modes.
    pub fn standard() -> Self {
        let mut boxes = Self::bootstrap_specs(|key| std::env::var(key).ok());
        if atlas_model_serving_enabled() && env_first(&["ANGEL_ATLAS_URL"]).is_some() {
            // Retain the legacy route when explicitly configured. Atlas project
            // memory is independent of this optional model endpoint.
            boxes.push(BootstrapBoxSpec {
                name: "atlas".to_string(),
                host: String::new(),
                fallback_ip: String::new(),
                slots: vec![BootstrapSlotSpec {
                    label: "atlas",
                    env_key: "ATLAS",
                    port: 0,
                    is_swarm: false,
                }],
            });
        }

        // Optional bearer key from the env ONLY — never bake secrets into source.
        let key = std::env::var("ANGEL_BRAIN_KEY").ok();
        let longcat_configured = env_first(&["ANGEL_LONGCAT_KEY", "LONGCAT_API_KEY"]).is_some();
        // The live tailnet, resolved once. Boxes are addressed by tailnet host.
        let tailnet = tailnet_hosts();
        // Which box/mode lands in hand. `ANGEL_DRIVER` is an explicit override
        // (honored only when that box/mode is actually reachable); UNSET falls
        // back to a preference chain — OpenAI ChatGPT-OAuth, then LongCat —
        // and finally to the smartest-available heuristic, so a stale default
        // can never pin the brain to a dead box. OpenRouter free seats are
        // selectable on the sota box; they are not auto-elected.
        let openai_oauth_available = crate::agent::openai_codex::ChatGptAuth::load().is_some()
            && crate::agent::openai_codex::CodexClub::default_model().is_some();
        let driver_pref = std::env::var("ANGEL_DRIVER")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| openai_oauth_available.then(|| "openai".to_string()))
            .or_else(|| longcat_configured.then(|| "longcat".to_string()));
        // Probing every endpoint on launch makes the TUI hang on dead boxes, so it's
        // OFF by default (the background prober corrects availability within a sweep).
        let probe = env_flag("ANGEL_PROBE", false);

        let mut agents: Vec<Agent> = Vec::new();
        // Every (club, availability-bit) the prober should refresh, flattened.
        let mut probe_targets: Vec<(Arc<dyn Club>, Arc<AtomicBool>)> = Vec::new();
        // Metered SOTA links that can feed the explicit SOTA-MOA option. These are
        // config-present, not auto-elected, and cloud HTTP links are not prober
        // targets so the cockpit never pings token-plan APIs every sweep.
        let mut sota_links: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = Vec::new();
        // Swarm-flagged slots to wrap in a second pass: (agent idx, slot idx, label,
        // raw inner club) — deferred so a cross-model mixture can see every box.
        let mut swarm_slots: Vec<(usize, usize, String, Arc<dyn Club>)> = Vec::new();
        // Per-slot metadata for the smartest-available driver pick, and the
        // resolved IP of each static box so a discovered surface on a known box
        // folds in as another mode instead of a duplicate tab. `auto` excludes
        // practice/openai from the auto-pick (still Tab-selectable).
        let mut slot_meta: Vec<SlotMeta> = Vec::new();
        let mut ip_to_box: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut box_ports: std::collections::HashMap<usize, Vec<u16>> =
            std::collections::HashMap::new();
        // One probe per *endpoint*: slots that share a URL (e.g. `swarm` and
        // `gemma`, two modes of the same :8000 server) share one availability
        // bit, so a prober sweep hits each endpoint once instead of once per
        // slot — half the GET /models traffic on a multi-mode box.
        let mut probed_endpoints: std::collections::HashMap<String, Arc<AtomicBool>> =
            std::collections::HashMap::new();

        for spec in &boxes {
            let box_name = &spec.name;
            let host = &spec.host;
            let fallback_ip = &spec.fallback_ip;
            let slots = &spec.slots;
            // Cheap, no-network online check seeds the first frame; the prober
            // refines it. A box offline per Tailscale is hidden immediately.
            let online = host_online(host, &tailnet);
            // This box's index (pushed once at the end of the iteration) and its
            // resolved IP, so a discovered surface on the same machine folds into
            // this box rather than spawning a duplicate tab.
            let ai = agents.len();
            let box_ip = tailnet_lookup(host, &tailnet)
                .map(|h| h.ip.clone())
                .unwrap_or_else(|| fallback_ip.to_string());
            let mut built: Vec<Slot> = Vec::new();
            for slot_spec in slots {
                let label = slot_spec.label;
                let env_key = slot_spec.env_key;
                let port = slot_spec.port;
                let is_swarm = slot_spec.is_swarm;
                let url = resolve_club_url(env_key, host, port, fallback_ip, &tailnet);
                let model = std::env::var(format!("ANGEL_{env_key}_MODEL"))
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default();
                // Build the raw endpoint first; swarm-flagged slots are wrapped in a
                // second pass once every box exists, so a cross-model mixture can
                // resolve sibling clubs by label. Without an env pin the club
                // follows the rig's live checkpoint — swapping models on a box
                // renames and re-scores the slot without a cockpit restart.
                let mut http = HttpClub::new(label, url.clone(), model.clone(), key.clone());
                if model.is_empty() {
                    http = http.follow_backend();
                }
                let club: Arc<dyn Club> = Arc::new(http);
                // Load pessimistically: a fleet model does NOT appear or become
                // selectable until a probe confirms it's reachable, so a broken
                // connection / dead box never loads. The background prober flips it
                // on within a sweep once it answers. `ANGEL_PROBE` opts into a
                // synchronous startup probe instead (blocks the first frame).
                let available = match probed_endpoints.get(&url) {
                    Some(bit) => Arc::clone(bit),
                    None => {
                        let seed = probe && online && club.is_available();
                        let bit = Arc::new(AtomicBool::new(seed));
                        probe_targets.push((Arc::clone(&club), Arc::clone(&bit)));
                        probed_endpoints.insert(url.clone(), Arc::clone(&bit));
                        bit
                    }
                };
                slot_meta.push(SlotMeta {
                    agent: ai,
                    slot: built.len(),
                    model,
                    n_params: None,
                    is_swarm,
                    auto: true,
                    available: Arc::clone(&available),
                });
                if is_swarm {
                    swarm_slots.push((ai, built.len(), label.to_string(), Arc::clone(&club)));
                }
                built.push(Slot {
                    label: label.to_string(),
                    club,
                    available,
                });
            }
            let ports = slots
                .iter()
                .filter_map(|slot| (slot.port != 0).then_some(slot.port))
                .collect::<Vec<_>>();
            if !ports.is_empty() {
                box_ports.insert(ai, ports);
            }
            if !box_ip.is_empty() {
                ip_to_box.insert(box_ip, ai);
            }
            agents.push(Agent {
                name: box_name.to_string(),
                slots: built,
                active: 0,
            });
        }

        // OpenAI via ChatGPT OAuth — a remote SOTA agent that reuses the tokens
        // the Codex CLI already obtained (~/.codex/auth.json) and speaks the
        // ChatGPT-backed Responses API. Present only when a usable token is on
        // disk (so a signed-out machine never shows a dead tab); the prober keeps
        // it honest via the same cheap local-token check.
        if let (Some(auth), Some(_model)) = (
            crate::agent::openai_codex::ChatGptAuth::load(),
            crate::agent::openai_codex::CodexClub::default_model(),
        ) {
            let shared = crate::agent::openai_codex::CodexClub::shared_state(auth);
            let available = Arc::new(AtomicBool::new(true));
            let selection = crate::agent::openai_codex::CodexClub::resolve_selection();
            let model = selection.model.clone();
            let configured_effort = Some(selection.effort.clone());
            // Drop private-test Codex Spark surfaces so ←/→ cannot select them.
            let mut catalog: Vec<_> = crate::agent::openai_codex::CodexClub::model_catalog()
                .into_iter()
                .filter(|c| !crate::agent::openai_codex::is_private_test_codex_model(&c.slug))
                .collect();
            if !catalog.iter().any(|candidate| candidate.slug == model) {
                catalog.insert(
                    0,
                    crate::agent::openai_codex::CodexModelInfo {
                        slug: model.clone(),
                        display_name: model.clone(),
                        default_reasoning_level: configured_effort.clone().unwrap_or_else(|| {
                            crate::agent::openai_codex::OPENAI_LUNA_EFFORT.to_string()
                        }),
                        supported_reasoning_levels: Vec::new(),
                        visibility: String::new(),
                        priority: 0,
                        ..crate::agent::openai_codex::CodexModelInfo::default()
                    },
                );
            }
            let mut slots = Vec::with_capacity(catalog.len().max(1));
            let mut active = 0usize;
            for candidate in catalog {
                if crate::agent::openai_codex::is_private_test_codex_model(&candidate.slug)
                    && candidate.slug != model
                {
                    continue;
                }
                let levels = candidate
                    .supported_reasoning_levels
                    .iter()
                    .map(|level| level.effort.clone())
                    .collect::<Vec<_>>();
                let preferred = (candidate.slug == model)
                    .then(|| configured_effort.clone())
                    .flatten();
                let effort = preferred
                    .filter(|effort| {
                        levels.is_empty()
                            || levels
                                .iter()
                                .any(|level| level.eq_ignore_ascii_case(effort))
                    })
                    .or_else(|| {
                        levels
                            .iter()
                            .find(|level| {
                                level.eq_ignore_ascii_case(&candidate.default_reasoning_level)
                            })
                            .cloned()
                    })
                    .or_else(|| levels.first().cloned());
                let metadata = candidate.route_metadata();
                let club = crate::agent::openai_codex::CodexClub::new_with_route_metadata_shared(
                    "openai",
                    candidate.slug.clone(),
                    Arc::clone(&shared),
                    effort,
                    levels,
                    metadata,
                );
                let club = if candidate.slug == model {
                    club.with_selection(selection.clone())
                } else {
                    club
                };
                let club: Arc<dyn Club> = Arc::new(club.sota_tuned());
                if candidate.slug == model {
                    active = slots.len();
                }
                slots.push(Slot {
                    label: candidate.slug,
                    club,
                    available: Arc::clone(&available),
                });
            }
            let club = slots
                .get(active)
                .map(|slot| Arc::clone(&slot.club))
                .expect("Codex catalog always contains the configured model");
            sota_links.push((
                "codex-run".to_string(),
                Arc::clone(&club),
                Arc::clone(&available),
            ));
            probe_targets.push((Arc::clone(&club), Arc::clone(&available)));
            agents.push(Agent {
                name: "openai".to_string(),
                slots,
                active,
            });
        }

        // The remaining SOTA links are pushed in the canonical intelligence
        // order (smartest first, see SOTA_MOA_INTELLIGENCE_ORDER): codex-run
        // above, then Kimi → deepseek → GLM → qwen → grok → longcat →
        // openrouter/hy3 → cerebras. This order is load-bearing: it is the SOTA-MOA quota
        // failover bench walk and the `links[0]` last-resort seat pick, so a
        // capped frontier link degrades to the *next smartest* model — never
        // straight to a free breadth-tier one.
        for link in [
            optional_openai_api_http_club(),
            optional_sota_http_club(
                "kimi",
                "kimi-k3",
                &["ANGEL_KIMI_URL", "KIMI_API_URL", "MOONSHOT_API_URL"],
                "https://api.moonshot.ai/v1",
                &["ANGEL_KIMI_MODEL", "KIMI_MODEL", "MOONSHOT_MODEL"],
                Some("kimi-k3"),
                &["ANGEL_KIMI_KEY", "KIMI_API_KEY", "MOONSHOT_API_KEY"],
            ),
            optional_sota_http_club(
                "deepseek",
                "deepseek-v4-pro",
                &["ANGEL_DEEPSEEK_URL"],
                "https://api.deepseek.com/v1",
                &["ANGEL_DEEPSEEK_MODEL", "DEEPSEEK_MODEL"],
                Some("deepseek-v4-pro"),
                &["ANGEL_DEEPSEEK_KEY", "DEEPSEEK_API_KEY"],
            ),
            optional_sota_http_club(
                "deepseek-flash",
                "deepseek-flash",
                &["ANGEL_DEEPSEEK_URL"],
                "https://api.deepseek.com/v1",
                &["ANGEL_DEEPSEEK_FLASH_MODEL", "DEEPSEEK_FLASH_MODEL"],
                Some("deepseek-flash"),
                &["ANGEL_DEEPSEEK_KEY", "DEEPSEEK_API_KEY"],
            ),
        ]
        .into_iter()
        .flatten()
        {
            sota_links.push(link);
        }

        // Keep the configurable/default flagship GLM seat first and expose every
        // other built-in GLM model beside it without duplicates.
        sota_links.extend(optional_glm_http_clubs());

        for link in [
            // Alibaba's Qwen Coding Plan — a subscription seat, not metered
            // DashScope. It only accepts plan keys (`sk-sp-…`); a general
            // DashScope key (`sk-ws-…`/`sk-…`) is rejected by this endpoint, and
            // the plan endpoint is the whole point (fixed monthly cost). The
            // route also fronts non-Qwen models (kimi-k2.5, glm-5,
            // MiniMax-M2.5), so ANGEL_QWEN_MODEL is worth pinning per campaign.
            optional_sota_http_club(
                "qwen",
                "qwen3.7-plus",
                &["ANGEL_QWEN_URL", "QWEN_API_URL", "DASHSCOPE_API_URL"],
                "https://coding-intl.dashscope.aliyuncs.com/v1",
                &["ANGEL_QWEN_MODEL", "QWEN_MODEL", "DASHSCOPE_MODEL"],
                Some("qwen3.7-plus"),
                &["ANGEL_QWEN_KEY", "QWEN_API_KEY", "DASHSCOPE_API_KEY"],
            ),
        ]
        .into_iter()
        .flatten()
        {
            sota_links.push(link);
        }

        // Grok account OAuth and optional CLI research are separate from the
        // API-key route. Each uses its configured authentication method.
        let mut grok_probe_registered = false;
        for (alias, club, available) in crate::agent::club::grok::grok_http_clubs() {
            let uses_oauth = alias != "grok-api";
            sota_links.push((alias, Arc::clone(&club), Arc::clone(&available)));
            if uses_oauth && !grok_probe_registered {
                // Every concrete Grok model shares one endpoint, OAuth authority,
                // and availability bit. Probe it once per interval, not once per
                // catalog row.
                probe_targets.push((club, available));
                grok_probe_registered = true;
            }
        }

        for link in [
            optional_sota_http_club(
                "longcat",
                "longcat",
                &["ANGEL_LONGCAT_URL", "LONGCAT_URL", "LONGCAT_API_URL"],
                "https://api.longcat.chat/openai/v1",
                &["ANGEL_LONGCAT_MODEL", "LONGCAT_MODEL"],
                Some("LongCat-2.0"),
                &["ANGEL_LONGCAT_KEY", "LONGCAT_API_KEY"],
            ),
            optional_sota_http_club(
                "cerebras",
                "cerebras",
                &["ANGEL_CEREBRAS_URL", "CEREBRAS_API_URL"],
                "https://api.cerebras.ai/v1",
                &["ANGEL_CEREBRAS_MODEL", "CEREBRAS_MODEL"],
                None,
                &["ANGEL_CEREBRAS_KEY", "CEREBRAS_API_KEY"],
            ),
        ]
        .into_iter()
        .flatten()
        {
            sota_links.push(link);
        }
        // OpenRouter requires credentials and a model pin and follows the
        // operator's optional provider allowlist.
        if openrouter_configured() {
            sota_links.extend(optional_openrouter_http_clubs());
        }

        // Free local breadth for the cheap MoA profile: historical Gemma swarm
        // surface plus the Spark ds4 DeepSeek-V4-Flash seat when that is the
        // resident checkpoint on :8000.
        let cheap_breadth: Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> = agents
            .iter()
            .flat_map(|agent| agent.slots.iter())
            .filter(|slot| slot.label == "gemma" || slot.label == "dsflash")
            .map(|slot| {
                (
                    slot.label.clone(),
                    Arc::clone(&slot.club),
                    Arc::clone(&slot.available),
                )
            })
            .collect();
        match sota_moa_club_with_breadth(&sota_links, &cheap_breadth) {
            Some(moa) => {
                let ai = agents.len();
                let mut slots = vec![Slot {
                    label: "sota-moa".to_string(),
                    club: moa,
                    available: Arc::new(AtomicBool::new(true)),
                }];
                for (alias, club, available) in &sota_links {
                    let si = slots.len();
                    slots.push(Slot {
                        label: alias.clone(),
                        club: Arc::clone(club),
                        available: Arc::clone(available),
                    });
                    if alias == "longcat" {
                        slot_meta.push(SlotMeta {
                            agent: ai,
                            slot: si,
                            model: club.label().to_string(),
                            n_params: None,
                            is_swarm: false,
                            auto: true,
                            available: Arc::clone(available),
                        });
                    }
                }
                agents.push(Agent {
                    name: "sota".to_string(),
                    slots,
                    // The MoA wrapper is an internal engagement target. Ordinary
                    // Tab/model navigation lands on a concrete provider; the
                    // Formation board is the only UI that puts the wrapper in hand.
                    active: 1,
                });
            }
            _ => {
                if !sota_links.is_empty() {
                    // A single configured provider cannot form an MoA, but it is still
                    // a complete direct driver.  Keep it in the bag so an explicit
                    // `ANGEL_DRIVER=longcat` (and the unset-provider preference chain)
                    // never falls through to the local practice floor.
                    let ai = agents.len();
                    agents.push(direct_sota_agent(ai, &sota_links, &mut slot_meta));
                }
            }
        }

        // Permanent logical configuration for the overnight GPU competition loop.
        // This is intentionally not auto-elected as the brain: it is a visible
        // control surface selected explicitly. Native formations and declared
        // agent graphs own multi-agent dispatch.
        let (gpu_comp_driver, gpu_comp_avail) = agents
            .iter()
            .find(|a| a.name == "turbo")
            .and_then(|a| a.slots.first())
            .map(|s| (Arc::clone(&s.club), Arc::clone(&s.available)))
            .unwrap_or_else(|| {
                (
                    Arc::new(PracticeClub::new()) as Arc<dyn Club>,
                    Arc::new(AtomicBool::new(false)),
                )
            });
        let gpu_comp_club: Arc<dyn Club> = Arc::new(GpuCompLocalMoaClub::new(gpu_comp_driver));
        agents.push(Agent {
            name: "gpu-comp".to_string(),
            slots: vec![Slot {
                label: "local-moa".to_string(),
                club: gpu_comp_club,
                available: gpu_comp_avail,
            }],
            active: 0,
        });

        // Math God: first-class bag club. Sol@ultra is the only Sol head;
        // GLM-5.3 and DeepSeek v4 Pro are extra proposers when configured;
        // Grok weighs in.
        if let Some((club, available)) = mathgod_club(&sota_links) {
            agents.push(Agent {
                name: "mathgod".to_string(),
                slots: vec![Slot {
                    label: "mathgod".to_string(),
                    club,
                    available,
                }],
                active: 0,
            });
        }

        // Second pass: wrap each swarm-flagged slot now that every box exists. The
        // resolver lets a cross-model mixture route a role to any sibling fleet club
        // by label via ANGEL_SWARM_{PROPOSE,JUDGE,VERIFY,AGG}_CLUB (e.g. propose on
        // gemma, judge on turbo, verify on atlas). With no role env set every role
        // falls back to the inner worker — the default homogeneous swarm.
        let roster: Vec<Arc<dyn Club>> = agents
            .iter()
            .flat_map(|a| a.slots.iter().map(|s| Arc::clone(&s.club)))
            .collect();
        for (ai, si, label, inner) in &swarm_slots {
            let roster = roster.clone();
            let resolve = move |lbl: &str| roster.iter().find(|c| c.label() == lbl).map(Arc::clone);
            agents[*ai].slots[*si].club = Arc::new(crate::agent::swarm::SwarmClub::from_env_with(
                label,
                Arc::clone(inner),
                resolve,
            ));
        }

        // The practice swing: the always-available local floor, its own box.
        let practice_avail = Arc::new(AtomicBool::new(true));
        let practice_club: Arc<dyn Club> = Arc::new(PracticeClub::new());
        probe_targets.push((Arc::clone(&practice_club), Arc::clone(&practice_avail)));
        agents.push(Agent {
            name: "practice".to_string(),
            slots: vec![Slot {
                label: "practice".to_string(),
                club: practice_club,
                available: practice_avail,
            }],
            active: 0,
        });
        let practice_idx = agents.len() - 1;

        // Brain selection. An explicit `ANGEL_DRIVER` wins — but ONLY if that
        // box/mode is actually reachable, so a named-but-dead preference can never
        // pin the brain to a corpse (the bug that made the cockpit land on a dead
        // Spark). Otherwise the smartest *available* fleet model goes in hand —
        // biggest model, nudged for the swarm and reasoning/coder heads — then the
        // first reachable box, then the always-on practice swing.
        // An explicit ANGEL_DRIVER wins if reachable; else the smartest *reachable*
        // fleet model. With pessimistic loading nothing is confirmed up yet at
        // construction, so this normally finds nothing and lands on the local
        // practice floor with `pending_brain` set — each tick then upgrades to the
        // smartest model the instant it loads, and a broken endpoint never gets
        // picked because it never becomes available. (ANGEL_PROBE, the synchronous
        // startup probe, can commit a real model right away.)
        //
        // Headless / bench path: when the operator pins ANGEL_DRIVER, do a
        // one-shot probe of that preferred slot only. Without this, fleet seats
        // stay unavailable under pessimistic load and the bag silently falls
        // through to a configured SOTA seat (e.g. longcat) even though the
        // pinned dsflash endpoint is healthy — which zeros entire action-agent
        // cohorts on unrelated quota errors. Full-bag ANGEL_PROBE remains the
        // broader "probe everything" switch for interactive cold starts.
        if let Some(pref) = driver_pref.as_deref()
            && let Some(ai) = resolve_driver(&mut agents, pref)
        {
            let si = agents[ai].active;
            if let Some(slot) = agents[ai].slots.get(si)
                && !slot.available.load(Ordering::Relaxed)
            {
                let up = slot.club.is_available();
                slot.available.store(up, Ordering::Relaxed);
            }
        }
        let elected = driver_pref
            .as_deref()
            .and_then(|pref| resolve_driver(&mut agents, pref))
            .filter(|&ai| agents[ai].available())
            .or_else(|| smartest_available(&mut agents, &slot_meta));
        let in_hand = elected.unwrap_or(practice_idx);
        let pending_brain = elected.is_none();
        agents[in_hand].settle_active();

        // Fleet scanning is a deliberate operator action. When disabled, do not
        // even leave a dormant discovery thread behind. When enabled it still runs
        // off the startup path and may independently opt into Hydra reads/publishes.
        let discovery_rx = if fleet_scan_enabled() {
            let brain_ep = box_ports
                .get(&in_hand)
                .and_then(|ports| ports.get(agents[in_hand].active).copied())
                .and_then(|port| {
                    ip_to_box
                        .iter()
                        .find(|&(_, &bi)| bi == in_hand)
                        .map(|(ip, _)| (ip.clone(), port))
                });
            // The (ip, port) pairs the static specs already cover, so the scanner
            // only streams in models the bag does not already show.
            let mut known: std::collections::HashSet<(String, u16)> =
                std::collections::HashSet::new();
            for (ip, &bi) in &ip_to_box {
                if let Some(ports) = box_ports.get(&bi) {
                    for &port in ports {
                        known.insert((ip.clone(), port));
                    }
                }
            }
            let (disc_tx, disc_rx) = std::sync::mpsc::channel();
            spawn_fleet_discovery(brain_ep, known, key.clone(), disc_tx);
            Some(disc_rx)
        } else {
            None
        };

        // Keep reachability fresh off the UI thread (never blocks a draw). Disable
        // with ANGEL_BAG_PROBE=0; tune the cadence with ANGEL_BAG_PROBE_SECS.
        if prober_enabled() {
            spawn_prober(probe_targets, probe_interval());
        }
        Self {
            agents,
            in_hand,
            discovery_rx,
            meta: slot_meta,
            driver_pref,
            pending_brain,
            generation: 0,
            cached_route_choices: std::sync::Mutex::new(None),
            cached_tabs: std::sync::Mutex::new(None),
            cached_in_hand_mode: std::sync::Mutex::new(None),
        }
    }

    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Fold in any boxes the background scanner discovered since the last call.
    /// Called from the run loop on the UI thread, so the bag stays single-threaded
    /// (no lock; the `&str` accessors keep working). New boxes append at the end —
    /// `in_hand` (an index) stays valid — and each gets its own prober so its
    /// availability stays honest. A surface on a host already shown folds in as an
    /// extra mode, not a duplicate tab. The in-hand box is never switched out from
    /// under the user; a freshly-discovered model is reached with `Tab`.
    pub fn drain_discovered(&mut self) {
        let mut fresh: Vec<(Agent, String)> = Vec::new();
        if let Some(rx) = self.discovery_rx.as_ref() {
            while let Ok(item) = rx.try_recv() {
                fresh.push(item);
            }
        }
        if fresh.is_empty() {
            return;
        }
        self.bump_generation();
        for (agent, model_id) in fresh {
            if prober_enabled() {
                let targets: Vec<(Arc<dyn Club>, Arc<AtomicBool>)> = agent
                    .slots
                    .iter()
                    .map(|s| (Arc::clone(&s.club), Arc::clone(&s.available)))
                    .collect();
                spawn_prober(targets, probe_interval());
            }
            // Fold into an existing box of the same machine as an extra mode, else a
            // new box — recording election metadata either way so the smartest-
            // reachable pick can choose the discovered model.
            let ai = self
                .agents
                .iter()
                .position(|a| a.name == agent.name)
                .unwrap_or(self.agents.len());
            for slot in agent.slots {
                let si = self.agents.get(ai).map(|a| a.slots.len()).unwrap_or(0);
                self.meta.push(SlotMeta {
                    agent: ai,
                    slot: si,
                    model: model_id.clone(),
                    n_params: None,
                    is_swarm: false,
                    auto: true,
                    available: Arc::clone(&slot.available),
                });
                match self.agents.get_mut(ai) {
                    Some(existing) => existing.slots.push(slot),
                    None => self.agents.push(Agent {
                        name: agent.name.clone(),
                        slots: vec![slot],
                        active: 0,
                    }),
                }
            }
        }
    }

    /// One brain tick, called from the run loop when idle. While the brain hasn't
    /// committed to a real model (still on the local floor), elect the smartest
    /// *reachable* model the instant one loads — a broken endpoint is never picked
    /// because it never becomes reachable, and the brain upgrades off the floor on
    /// its own. Once committed, just keep the chosen box honest (drift only if it
    /// dies); if that drift lands back on the floor, re-arm election.
    pub fn settle_brain(&mut self) {
        if self.pending_brain {
            self.elect_brain();
        } else {
            self.settle_in_hand();
            if self.agents[self.in_hand].name == "practice" {
                self.pending_brain = true;
            }
        }
    }

    /// Refresh election metadata from each slot's live checkpoint id, so the
    /// smartest-available pick scores what a rig serves *now* — not what env
    /// said at startup, or an empty id for slots that resolve their model from
    /// the live `/models` response.
    fn refresh_meta_models(&mut self) {
        for m in &mut self.meta {
            if let Some(id) = self
                .agents
                .get(m.agent)
                .and_then(|a| a.slots.get(m.slot))
                .and_then(|s| s.club.live_model_name())
            {
                m.model = id;
            }
        }
    }

    /// Put the smartest reachable real model in hand, honoring an explicit reachable
    /// `ANGEL_DRIVER` first. A no-op (stays on the floor) until something loads;
    /// clears `pending_brain` once it commits to a real box.
    fn elect_brain(&mut self) {
        let before = self.selected_route_indices();
        self.refresh_meta_models();
        if let Some(pref) = self.driver_pref.clone()
            && let Some(ai) = resolve_driver(&mut self.agents, &pref)
            && self.agents[ai].available()
        {
            self.in_hand = ai;
            self.agents[ai].settle_active();
            self.pending_brain = false;
            if self.selected_route_indices() != before {
                self.bump_generation();
            }
            return;
        }
        if let Some(ai) = smartest_available(&mut self.agents, &self.meta) {
            self.in_hand = ai;
            self.pending_brain = false;
        }
        if self.selected_route_indices() != before {
            self.bump_generation();
        }
    }

    pub(crate) fn practice_only() -> Self {
        let club: Arc<dyn Club> = Arc::new(PracticeClub {
            latency: Duration::ZERO,
        });
        Self {
            agents: vec![Agent {
                name: "practice".to_string(),
                slots: vec![Slot {
                    label: "practice".to_string(),
                    club,
                    available: Arc::new(AtomicBool::new(true)),
                }],
                active: 0,
            }],
            in_hand: 0,
            discovery_rx: None,
            meta: Vec::new(),
            driver_pref: None,
            pending_brain: false,
            generation: 0,
            cached_route_choices: std::sync::Mutex::new(None),
            cached_tabs: std::sync::Mutex::new(None),
            cached_in_hand_mode: std::sync::Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub(crate) fn practice_for_test() -> Self {
        Self::practice_only()
    }

    /// The club currently in hand — the in-hand box's active mode (cheap `Arc`
    /// clone, for handing to a worker).
    pub fn in_hand(&self) -> Arc<dyn Club> {
        self.agents[self.in_hand].active_club()
    }

    /// Warm the in-hand club (and a couple of neighbors) off the UI thread so
    /// the first Enter after launch does not pay cold DNS/TLS/metadata on the
    /// submit path. Fire-and-forget; failures are silent — the real hop still
    /// probes.
    pub(crate) fn warm_background(&self) {
        let mut clubs: Vec<Arc<dyn Club>> = Vec::with_capacity(3);
        clubs.push(self.in_hand());
        if let Some(agent) = self.agents.get(self.in_hand) {
            for slot in &agent.slots {
                if clubs.len() >= 3 {
                    break;
                }
                if !clubs.iter().any(|c| Arc::ptr_eq(c, &slot.club)) {
                    clubs.push(Arc::clone(&slot.club));
                }
            }
        }
        for club in clubs {
            let _ = std::thread::Builder::new()
                .name("angel-club-warm".into())
                .spawn(move || club.warm_up());
        }
    }

    /// The in-hand club, optionally wrapped in a failover chain. With
    /// `ANGEL_FALLBACK` set, a turn that can't reach the chosen mode falls through
    /// to every other mode across the fleet (the always-on practice swing last)
    /// instead of failing — at the cost of possibly answering from a different
    /// model than the label shows. Off by default so selection stays exact.
    pub fn in_hand_with_fallback(&self) -> Arc<dyn Club> {
        let active = self.in_hand();
        if !fallback_armed() {
            return active;
        }
        let include_sota = fallback_include_sota();
        // Active mode first, then every other mode across every box. Each link
        // carries its prober-maintained availability bit so the chain walk can
        // skip boxes already known dead instead of paying connect-timeout ×
        // retries per corpse. The active club always gets tried (gate None):
        // it's what the user picked, and a stale bit must never mute it.
        let mut chain: Vec<Arc<dyn Club>> = vec![Arc::clone(&active)];
        let mut gates: Vec<Option<Arc<AtomicBool>>> = vec![None];
        for a in &self.agents {
            for s in &a.slots {
                if !Arc::ptr_eq(&s.club, &active) {
                    if !include_sota && is_sota_label(s.club.label()) {
                        continue;
                    }
                    chain.push(Arc::clone(&s.club));
                    gates.push(Some(Arc::clone(&s.available)));
                }
            }
        }
        Arc::new(FallbackClub::with_gates(chain, gates))
    }

    /// Concrete, non-recursive model routes available to formation seats.
    pub(crate) fn moa_model_choices(&self) -> Vec<crate::agent::formations::MoaModelChoice> {
        self.agents
            .iter()
            .enumerate()
            .filter(|(_, agent)| {
                !crate::agent::club::is_logical_wrapper_label(&agent.name)
                    && agent.name != "practice"
            })
            .flat_map(|(agent_index, agent)| {
                agent
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(_, slot)| {
                        let slot_label = slot.label.to_ascii_lowercase();
                        let club_label = slot.club.label().to_ascii_lowercase();
                        slot_label != "swarm"
                            && !slot_label.contains("moa")
                            && club_label != "swarm"
                            && !club_label.contains("moa")
                    })
                    .map(move |(slot_index, slot)| {
                        let route = slot.club.route_identity();
                        let driver = route.driver;
                        let model = route.model.unwrap_or_else(|| slot.model_identity());
                        let metered = [&agent.name, &driver, &model]
                            .iter()
                            .any(|label| is_sota_label(label));
                        crate::agent::formations::MoaModelChoice {
                            route: crate::agent::formations::MoaModelRef {
                                agent_index,
                                slot_index,
                                agent: agent.name.clone(),
                                driver,
                                route_id: crate::agent::backplane::RouteId::chat(
                                    &agent.name,
                                    &slot.club.route_identity().driver,
                                    slot.club.reasoning_effort().as_deref(),
                                ),
                                expected_revision: crate::agent::backplane::ModelRevision::chat(
                                    &model,
                                ),
                                model,
                                metered,
                            },
                            available: slot.available.load(Ordering::Relaxed),
                        }
                    })
            })
            .collect()
    }

    pub(crate) fn atlas_teacher_route(&self) -> Option<crate::knowledge::atlas_clerk::ClerkRoute> {
        let mut best: Option<(u32, crate::knowledge::atlas_clerk::ClerkRoute)> = None;
        for agent in &self.agents {
            if agent.name == "practice" {
                continue;
            }
            for slot in &agent.slots {
                if !slot.available.load(Ordering::Relaxed)
                    || slot.label.eq_ignore_ascii_case("swarm")
                    || slot.label.to_ascii_lowercase().contains("moa")
                {
                    continue;
                }
                let route = slot.club.route_identity();
                let model = route.model.clone().unwrap_or_else(|| slot.model_identity());
                let parameters = crate::agent::backplane::parameter_billions(&model).unwrap_or(0);
                if parameters < 30 {
                    continue;
                }
                let candidate = crate::knowledge::atlas_clerk::ClerkRoute {
                    route_id: crate::agent::backplane::RouteId::chat(
                        &agent.name,
                        &route.driver,
                        route.reasoning_effort.as_deref(),
                    ),
                    model_revision: crate::agent::backplane::ModelRevision::chat(&model),
                    resource_group: agent.name.clone(),
                    club: Arc::clone(&slot.club),
                };
                if best
                    .as_ref()
                    .is_none_or(|(current, _)| parameters > *current)
                {
                    best = Some((parameters, candidate));
                }
            }
        }
        best.map(|(_, route)| route)
    }

    /// Build an MoA wrapper from the operator's exact graph roster. Unlike the
    /// automatic SOTA formation, this path does not attach an out-of-roster
    /// quota fallback bench: a hidden substitution would defeat the budget and
    /// model choices the roster editor exists to enforce.
    pub(crate) fn activate_sota_moa_with_roster(
        &mut self,
        roster: &crate::agent::formations::FormationRoster,
    ) -> Result<usize, String> {
        use crate::agent::formations::FormationRole;

        if !roster.is_ready() {
            return Err(format!(
                "roster is incomplete (missing {})",
                roster.missing_labels().join(", ")
            ));
        }
        let slots = crate::agent::formations::formation(roster.formation()).slots();
        if slots.len() != roster.assignments().len() {
            return Err("formation roster no longer matches its graph".to_string());
        }

        type RoutedSeat = (Arc<dyn Club>, Option<Arc<AtomicBool>>);
        let mut roles: std::collections::HashMap<FormationRole, Vec<RoutedSeat>> =
            std::collections::HashMap::new();
        for (formation_slot, selected) in slots.iter().zip(roster.assignments()) {
            let selected = selected
                .as_ref()
                .ok_or_else(|| format!("{} has no model", formation_slot.label()))?;
            let agent = self
                .agents
                .get(selected.agent_index)
                .ok_or_else(|| format!("{} route disappeared", selected.display_label()))?;
            let slot = agent
                .slots
                .get(selected.slot_index)
                .ok_or_else(|| format!("{} route disappeared", selected.display_label()))?;
            if agent.name == "practice"
                || agent.name.eq_ignore_ascii_case("mathgod")
                || slot.label.eq_ignore_ascii_case("mathgod")
                || slot.label.eq_ignore_ascii_case("swarm")
                || slot.label.to_ascii_lowercase().contains("moa")
            {
                return Err(format!(
                    "{} is not a concrete MoA seat model",
                    selected.display_label()
                ));
            }
            if !slot.available.load(Ordering::Relaxed) {
                return Err(format!("{} is offline", selected.display_label()));
            }
            let live = slot
                .club
                .route_identity()
                .model
                .unwrap_or_else(|| slot.model_identity());
            let live_route = crate::agent::backplane::RouteId::chat(
                &agent.name,
                &slot.club.route_identity().driver,
                slot.club.reasoning_effort().as_deref(),
            );
            let live_revision = crate::agent::backplane::ModelRevision::chat(&live);
            if crate::agent::backplane::active()
                && (live_route != selected.route_id || live_revision != selected.expected_revision)
            {
                return Err(format!(
                    "{} now serves {live}; reopen Formation to confirm the changed model",
                    selected.display_label()
                ));
            }
            roles
                .entry(formation_slot.role)
                .or_default()
                // Exact roster seats are not optional breadth. Keep their
                // positions if a route drops after engagement so failure is
                // visible instead of shifting a different-cost model in.
                .push((Arc::clone(&slot.club), None));
        }

        let mut proposers = roles.remove(&FormationRole::Propose).unwrap_or_default();
        let mut judges = roles.remove(&FormationRole::Judge).unwrap_or_default();
        let mut verifiers = roles.remove(&FormationRole::Verify).unwrap_or_default();
        let mut aggregators = roles.remove(&FormationRole::Aggregate).unwrap_or_default();
        let scouts = roles.remove(&FormationRole::Scout).unwrap_or_default();
        let (propose, _) = proposers
            .first()
            .ok_or_else(|| "formation has no proposer seat".to_string())?;
        let propose = Arc::clone(propose);
        let (aggregate, _) = aggregators
            .first()
            .ok_or_else(|| "formation has no synthesis seat".to_string())?;
        let aggregate = Arc::clone(aggregate);
        let judge = judges
            .first()
            .map(|(club, _)| Arc::clone(club))
            .unwrap_or_else(|| Arc::clone(&aggregate));
        let verify = verifiers
            .first()
            .map(|(club, _)| Arc::clone(club))
            .unwrap_or_else(|| Arc::clone(&aggregate));
        let research = scouts.first().map(|(club, _)| Arc::clone(club));
        let proposer_extras = proposers.drain(1..).collect();
        let judge_extras = if judges.is_empty() {
            Vec::new()
        } else {
            judges.drain(1..).collect()
        };
        let verifier_extras = if verifiers.is_empty() {
            Vec::new()
        } else {
            verifiers.drain(1..).collect()
        };
        let aggregator_extras = aggregators.drain(1..).collect();

        let moa: Arc<dyn Club> = Arc::new(
            crate::agent::swarm::SwarmClub::from_sota_env_roles(
                "sota-moa", propose, judge, verify, aggregate,
            )
            .with_extra_proposers(proposer_extras)
            .with_extra_judges(judge_extras)
            .with_extra_verifiers(verifier_extras)
            .with_extra_aggregators(aggregator_extras)
            .with_research_role(research)
            // Reaching this builder means the operator staged and confirmed
            // this exact roster. Encode engagement in the wrapper itself
            // rather than rewriting the operator's next message with `moa:`.
            .with_engaged_formation()
            // Deck-staged per-role efforts outrank the env seat policy.
            .with_role_effort_overrides(crate::agent::swarm::SeatEfforts {
                propose: roster
                    .role_effort(FormationRole::Propose)
                    .map(str::to_string),
                judge: roster.role_effort(FormationRole::Judge).map(str::to_string),
                verify: roster
                    .role_effort(FormationRole::Verify)
                    .map(str::to_string),
                aggregate: roster
                    .role_effort(FormationRole::Aggregate)
                    .map(str::to_string),
            }),
        );

        let (agent_index, slot_index) =
            if let Some(agent_index) = self.agents.iter().position(|agent| agent.name == "sota") {
                if let Some(slot_index) = self.agents[agent_index]
                    .slots
                    .iter()
                    .position(|slot| slot.label == "sota-moa")
                {
                    self.agents[agent_index].slots[slot_index].club = moa;
                    self.agents[agent_index].slots[slot_index]
                        .available
                        .store(true, Ordering::Relaxed);
                    (agent_index, slot_index)
                } else {
                    let slot_index = self.agents[agent_index].slots.len();
                    self.agents[agent_index].slots.push(Slot {
                        label: "sota-moa".to_string(),
                        club: moa,
                        available: Arc::new(AtomicBool::new(true)),
                    });
                    (agent_index, slot_index)
                }
            } else {
                let agent_index = self.agents.len();
                self.agents.push(Agent {
                    name: "sota".to_string(),
                    slots: vec![Slot {
                        label: "sota-moa".to_string(),
                        club: moa,
                        available: Arc::new(AtomicBool::new(true)),
                    }],
                    active: 0,
                });
                (agent_index, 0)
            };
        self.agents[agent_index].active = slot_index;
        self.in_hand = agent_index;
        self.pending_brain = false;
        Ok(roster.slot_count())
    }

    /// The in-hand **box** name (e.g. `spark`) — what the header/avatar key off.
    pub fn in_hand_label(&self) -> &str {
        &self.agents[self.in_hand].name
    }

    /// The in-hand box's active **mode** — the live checkpoint id for
    /// follow-backend slots (e.g. `qwen3.6-27b-mtp-pi-tune`), else the static
    /// mode label (e.g. `swarm`). Shown when the box has more than one mode,
    /// and also on single-model boxes whose checkpoint differs from the box
    /// name — models churn on a rig, so `apollo` alone says nothing about
    /// what's loaded. `None` only when it would repeat the box name.
    pub fn in_hand_mode(&self) -> Option<String> {
        self.in_hand_chrome().mode.clone()
    }

    /// Exact in-hand spawn identity. Cached with header chrome so Enter-after-echo
    /// does not re-take model/effort locks already paid on the last draw.
    pub fn in_hand_route_identity(&self) -> RouteIdentity {
        self.in_hand_chrome().route.clone()
    }

    /// One lookup for header chrome: mode label, effort, and THINK ladder.
    pub fn in_hand_chrome(&self) -> std::sync::Arc<InHandChrome> {
        let a = &self.agents[self.in_hand];
        let club = a.active_club_ref();
        let rev = club.route_state_revision();
        if let Ok(guard) = self.cached_in_hand_mode.lock()
            && let Some((generation, in_hand, active, cached_rev, chrome)) = guard.as_ref()
            && *generation == self.generation
            && *in_hand == self.in_hand
            && *active == a.active
            && *cached_rev == rev
        {
            return std::sync::Arc::clone(chrome);
        }
        let label = a.active_label();
        let effort = club.reasoning_effort();
        let chrome = std::sync::Arc::new(InHandChrome {
            mode: (a.slots.len() > 1 || label != a.name).then_some(label),
            effort: effort.clone(),
            effort_selectable: !club.reasoning_levels().is_empty(),
            route: RouteIdentity {
                driver: club.label().to_string(),
                model: club.model_identity(),
                reasoning_effort: effort,
            },
        });
        if let Ok(mut guard) = self.cached_in_hand_mode.lock() {
            *guard = Some((
                self.generation,
                self.in_hand,
                a.active,
                rev,
                std::sync::Arc::clone(&chrome),
            ));
        }
        chrome
    }

    /// Every club across every box (cheap `Arc` clones) — the orchestrator's roster.
    pub fn roster(&self) -> Vec<Arc<dyn Club>> {
        let banned = banned_boxes();
        self.agents
            .iter()
            .filter(|agent| !banned.iter().any(|b| agent.name.eq_ignore_ascii_case(b)))
            .flat_map(|a| a.slots.iter().map(|s| Arc::clone(&s.club)))
            .collect()
    }

    /// The live tab strip — one entry per **box**, offline boxes omitted, so `Tab`
    /// only ever offers a machine that's actually up. The in-hand box is always
    /// included (even while down) so the strip never hides what you're pointed at,
    /// and it carries its active mode for boxes that have more than one.
    pub fn tabs(&self) -> std::sync::Arc<Vec<ClubTab>> {
        let stamps: Vec<TabStamp> = self
            .agents
            .iter()
            .enumerate()
            .filter(|(_, a)| a.name != "practice")
            .map(|(i, a)| {
                let slot = a.slots.get(a.active);
                TabStamp {
                    available: a.available(),
                    in_hand: i == self.in_hand,
                    active: a.active,
                    route_state_revision: slot
                        .map(|slot| slot.club.route_state_revision())
                        .unwrap_or(0),
                }
            })
            .collect();
        if let Some((generation, cached_stamps, tabs)) = &*self.cached_tabs.lock().unwrap()
            && *generation == self.generation
            && cached_stamps == &stamps
        {
            return std::sync::Arc::clone(tabs);
        }

        let tabs: Vec<ClubTab> = self
            .agents
            .iter()
            .enumerate()
            .filter(|(i, a)| {
                a.name != "practice"
                    && (*i == self.in_hand || !crate::agent::club::is_mathgod_label(&a.name))
            })
            .filter_map(|(i, a)| {
                let available = a.available();
                let in_hand = i == self.in_hand;
                (available || in_hand).then(|| {
                    // Mode shown for multi-mode boxes AND whenever the live
                    // checkpoint differs from the box name (see in_hand_mode).
                    let label = a.active_label();
                    ClubTab {
                        label: a.name.clone(),
                        mode: (a.slots.len() > 1 || label != a.name).then_some(label),
                        in_hand,
                        available,
                    }
                })
            })
            .collect();
        let tabs = std::sync::Arc::new(tabs);
        if let Ok(mut cache) = self.cached_tabs.lock() {
            *cache = Some((self.generation, stamps, std::sync::Arc::clone(&tabs)));
        }
        tabs
    }

    pub fn sota_moa_status(&self) -> String {
        let Some(sota) = self.agents.iter().find(|a| a.name == "sota") else {
            return "sota-moa: unavailable (no SOTA links configured)".to_string();
        };
        let modes = sota
            .slots
            .iter()
            .map(|s| {
                // A quota-benched link is more specific than a bare "down" mark:
                // show how long until the next probe re-checks the provider.
                if let Some(left) = s.club.quota_cooldown() {
                    return format!("{}(quota {}m)", s.label, left.as_secs().div_ceil(60).max(1));
                }
                format!(
                    "{}{}",
                    s.label,
                    if s.available.load(Ordering::Relaxed) {
                        ""
                    } else {
                        "!"
                    }
                )
            })
            .collect::<Vec<_>>();
        let has_moa = sota.slots.iter().any(|s| s.label == "sota-moa");
        let reason = if has_moa {
            "available"
        } else if modes.len() < 2 {
            "unavailable (need at least two configured SOTA links)"
        } else {
            "unavailable"
        };
        format!("sota-moa: {reason} · modes {}", modes.join(" "))
    }

    /// `Tab`: cycle to the next *reachable* box, skipping offline ones, and land on
    /// one of its live modes. A box that's fully down is never selected (practice
    /// is the always-available floor).
    pub fn cycle(&mut self) {
        self.bump_generation();
        // A manual Tab is the user taking the wheel — stop auto-electing the brain
        // so their pick is respected (it's only re-armed if it later goes down).
        self.pending_brain = false;
        let n = self.agents.len();
        if n == 0 {
            return;
        }
        for step in 1..=n {
            let idx = (self.in_hand + step) % n;
            if self.agents[idx].name != "practice"
                && !crate::agent::club::is_mathgod_label(&self.agents[idx].name)
                && self.agents[idx].available()
            {
                self.in_hand = idx;
                self.agents[idx].settle_active();
                return;
            }
        }
    }

    /// Subcontrol (`←/→`): cycle the in-hand box's active mode, skipping offline
    /// modes. A no-op on single-mode boxes.
    pub fn cycle_sub(&mut self, forward: bool) {
        self.bump_generation();
        self.pending_brain = false;
        if let Some(a) = self.agents.get_mut(self.in_hand) {
            a.cycle_active(forward);
        }
    }

    /// Every concrete route, including temporarily unavailable rows so the deck
    /// explains the whole fleet instead of making offline models disappear.
    pub(crate) fn route_choices(&self) -> std::sync::Arc<Vec<RouteChoice>> {
        if let Some((generation, stamps, choices)) = &*self.cached_route_choices.lock().unwrap()
            && *generation == self.generation
        {
            let mut match_ok = true;
            let mut idx = 0;
            for agent in &self.agents {
                if agent.name == "practice" || crate::agent::club::is_mathgod_label(&agent.name) {
                    continue;
                }
                for slot in &agent.slots {
                    if slot.label == "sota-moa" || crate::agent::club::is_mathgod_label(&slot.label)
                    {
                        continue;
                    }
                    let current = RouteChoiceStamp {
                        available: slot.available.load(Ordering::Relaxed),
                        route_state_revision: slot.club.route_state_revision(),
                    };
                    if stamps.get(idx) != Some(&current) {
                        match_ok = false;
                        break;
                    }
                    idx += 1;
                }
                if !match_ok {
                    break;
                }
            }
            if match_ok && idx == stamps.len() {
                return std::sync::Arc::clone(choices);
            }
        }

        let mut choices = Vec::new();
        let mut stamps = Vec::new();
        for (agent_index, agent) in self.agents.iter().enumerate() {
            if agent.name == "practice" || crate::agent::club::is_mathgod_label(&agent.name) {
                continue;
            }
            for (slot_index, slot) in agent.slots.iter().enumerate() {
                if slot.label == "sota-moa" || crate::agent::club::is_mathgod_label(&slot.label) {
                    continue;
                }
                // Sample each mutable fact once. The same availability value now
                // backs both the rendered row and its cache stamp, so a prober
                // transition during a miss cannot publish mismatched state.
                let available = slot.available.load(Ordering::Relaxed);
                let route_state_revision = slot.club.route_state_revision();
                let route = slot.club.route_identity();
                choices.push(RouteChoice {
                    agent_index,
                    slot_index,
                    agent: agent.name.clone(),
                    driver: route.driver,
                    model: route.model.unwrap_or_else(|| slot.model_identity()),
                    available,
                    selected: self.in_hand == agent_index && agent.active == slot_index,
                    reasoning_effort: route.reasoning_effort,
                    reasoning_levels: Arc::from(slot.club.reasoning_levels()),
                    metadata: slot.club.route_metadata(),
                });
                stamps.push(RouteChoiceStamp {
                    available,
                    route_state_revision,
                });
            }
        }

        // A direct provider can also be exposed through a SOTA alias. Present
        // that exact backend once, preserving its selected alias if it is in
        // hand. Equal model names on different connections remain distinct.
        let mut owners = std::collections::HashMap::new();
        for (index, choice) in choices.iter().enumerate() {
            let club = &self.agents[choice.agent_index].slots[choice.slot_index].club;
            let identity = Arc::as_ptr(club) as *const () as usize;
            let owner = owners.entry(identity).or_insert(index);
            if choice.selected {
                *owner = index;
            }
        }
        let choices = choices
            .into_iter()
            .enumerate()
            .filter_map(|(index, choice)| {
                let club = &self.agents[choice.agent_index].slots[choice.slot_index].club;
                let identity = Arc::as_ptr(club) as *const () as usize;
                (owners.get(&identity) == Some(&index)).then_some(choice)
            })
            .collect();
        let choices_arc = std::sync::Arc::new(choices);

        if let Ok(mut cache) = self.cached_route_choices.lock() {
            *cache = Some((self.generation, stamps, std::sync::Arc::clone(&choices_arc)));
        }

        choices_arc
    }

    /// Whether the Brain Route deck has anything concrete to inspect. This is
    /// deliberately broader than "can switch": a single current route still
    /// owns useful model, effort, context, and evidence detail.
    pub(crate) fn has_route_choices(&self) -> bool {
        self.agents
            .iter()
            .any(|agent| agent.name != "practice" && !agent.slots.is_empty())
    }

    /// Select a route from a recent deck snapshot. Availability and practice
    /// exclusion are checked again so stale popup rows cannot route a turn.
    pub(crate) fn select_route(&mut self, agent_index: usize, slot_index: usize) -> bool {
        self.bump_generation();
        let Some(agent) = self.agents.get(agent_index) else {
            return false;
        };
        if agent.name == "practice"
            || !agent
                .slots
                .get(slot_index)
                .is_some_and(|slot| slot.available.load(Ordering::Relaxed))
        {
            return false;
        }
        self.in_hand = agent_index;
        self.agents[agent_index].active = slot_index;
        self.pending_brain = false;
        // New selection: warm the new in-hand (and neighbors) before the next
        // turn so a MODEL flip is not a cold first hop.
        self.warm_background();
        true
    }

    /// Exact current selection for a temporary mode that must hand control
    /// back after its worker has finished cloning the active club.
    pub(crate) fn selected_route_indices(&self) -> (usize, usize) {
        (self.in_hand, self.agents[self.in_hand].active)
    }

    /// Restore a previously selected route, including the practice floor.
    /// Unlike model-deck selection this is internal bookkeeping, so practice is
    /// valid; a route that went offline is rejected and the caller can settle.
    pub(crate) fn restore_route(&mut self, agent_index: usize, slot_index: usize) -> bool {
        self.bump_generation();
        let Some(agent) = self.agents.get(agent_index) else {
            return false;
        };
        let Some(slot) = agent.slots.get(slot_index) else {
            return false;
        };
        if !slot.available.load(Ordering::Relaxed) {
            return false;
        }
        self.in_hand = agent_index;
        self.agents[agent_index].active = slot_index;
        self.pending_brain = false;
        true
    }

    /// Apply a route and one of that route's own reasoning levels as one UI
    /// transaction. The club validates the effort before the Bag moves in hand,
    /// so a stale/unsupported THINK choice never partially changes the route.
    pub(crate) fn select_route_with_effort(
        &mut self,
        agent_index: usize,
        slot_index: usize,
        effort: &str,
    ) -> bool {
        self.bump_generation();
        let Some(agent) = self.agents.get(agent_index) else {
            return false;
        };
        if agent.name == "practice" {
            return false;
        }
        let Some(slot) = agent.slots.get(slot_index) else {
            return false;
        };
        if !slot.available.load(Ordering::Relaxed)
            || slot.club.set_reasoning_effort(effort).is_none()
        {
            return false;
        }
        self.in_hand = agent_index;
        self.agents[agent_index].active = slot_index;
        self.pending_brain = false;
        true
    }

    pub(crate) fn in_hand_club_ref(&self) -> &dyn Club {
        self.agents[self.in_hand].active_club_ref()
    }

    pub fn reasoning_effort(&self) -> Option<String> {
        self.in_hand_chrome().effort.clone()
    }

    pub fn reasoning_levels(&self) -> &[String] {
        self.in_hand_club_ref().reasoning_levels()
    }

    pub fn set_reasoning_effort(&self, effort: &str) -> Option<String> {
        let selected = self.in_hand_club_ref().set_reasoning_effort(effort)?;
        if let Ok(mut cache) = self.cached_route_choices.lock() {
            *cache = None;
        }
        if let Ok(mut cache) = self.cached_in_hand_mode.lock() {
            *cache = None;
        }
        Some(selected)
    }

    /// Keep the selection live: if the in-hand box's active mode died, drop to
    /// another mode on the same box; if the whole box is down, move to a reachable
    /// box. A no-op while the active mode is up (never overrides a working choice).
    /// The caller gates this on being idle so a turn is never yanked mid-flight.
    pub fn settle_in_hand(&mut self) {
        if self.agents.is_empty() {
            return;
        }
        let before = self.selected_route_indices();
        self.settle_in_hand_inner();
        if self.selected_route_indices() != before {
            self.bump_generation();
        }
    }

    fn settle_in_hand_inner(&mut self) {
        let n = self.agents.len();
        if n == 0 {
            return;
        }
        if self.agents[self.in_hand].available() {
            // Box is reachable — just make sure the active mode is one that's up.
            self.agents[self.in_hand].settle_active();
            return;
        }
        for step in 1..=n {
            let idx = (self.in_hand + step) % n;
            if self.agents[idx].available()
                && !crate::agent::club::is_mathgod_label(&self.agents[idx].name)
            {
                self.in_hand = idx;
                self.agents[idx].settle_active();
                return;
            }
        }
    }
}

#[cfg(test)]
struct TestRenderClub {
    model: String,
}

#[cfg(test)]
impl Club for TestRenderClub {
    fn label(&self) -> &str {
        "practice"
    }

    fn model_identity(&self) -> Option<String> {
        Some(self.model.clone())
    }

    fn respond(&self, prompt: &str) -> Result<String, String> {
        PracticeClub {
            latency: Duration::ZERO,
        }
        .respond(prompt)
    }
}

#[cfg(test)]
struct TestReasoningClub {
    label: String,
    effort: Mutex<String>,
    levels: Vec<String>,
    metadata: RouteMetadata,
}

#[cfg(test)]
impl Club for TestReasoningClub {
    fn respond(&self, prompt: &str) -> Result<String, String> {
        Ok(prompt.to_string())
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn live_model_name(&self) -> Option<String> {
        Some(self.label.clone())
    }

    fn reasoning_effort(&self) -> Option<String> {
        self.effort.lock().ok().map(|effort| effort.clone())
    }

    fn reasoning_levels(&self) -> &[String] {
        &self.levels
    }

    fn route_metadata(&self) -> RouteMetadata {
        self.metadata.clone()
    }

    fn header_route_metadata(&self) -> RouteMetadata {
        self.metadata.clone()
    }

    fn set_reasoning_effort(&self, requested: &str) -> Option<String> {
        let selected = self
            .levels
            .iter()
            .find(|level| level.eq_ignore_ascii_case(requested))?
            .clone();
        *self.effort.lock().ok()? = selected.clone();
        Some(selected)
    }
}

#[cfg(test)]
impl Bag {
    /// Deterministic multi-box bag for render/UI tests (no network, no prober).
    /// Each entry is `(box_name, &[(mode_label, available)])`; in hand on box 0,
    /// each box's active mode settled onto its first reachable mode.
    pub(crate) fn replace_in_hand_club_for_test(&mut self, club: Arc<dyn Club>) {
        let agent = &mut self.agents[self.in_hand];
        let slot = agent.slots.get_mut(agent.active).expect("in-hand slot");
        slot.club = club;
        self.bump_generation();
    }

    pub(crate) fn for_render_test(spec: &[(&str, &[(&str, bool)])]) -> Self {
        let mut agents: Vec<Agent> = spec
            .iter()
            .map(|(name, modes)| Agent {
                name: name.to_string(),
                slots: modes
                    .iter()
                    .map(|(label, available)| Slot {
                        label: label.to_string(),
                        club: Arc::new(TestRenderClub {
                            model: label.to_string(),
                        }),
                        available: Arc::new(AtomicBool::new(*available)),
                    })
                    .collect(),
                active: 0,
            })
            .collect();
        for a in &mut agents {
            a.settle_active();
        }
        Self {
            agents,
            in_hand: 0,
            discovery_rx: None,
            meta: Vec::new(),
            driver_pref: None,
            pending_brain: false,
            generation: 0,
            cached_route_choices: std::sync::Mutex::new(None),
            cached_tabs: std::sync::Mutex::new(None),
            cached_in_hand_mode: std::sync::Mutex::new(None),
        }
    }

    pub(crate) fn for_reasoning_render_test() -> Self {
        let selectable: Arc<dyn Club> = Arc::new(TestReasoningClub {
            label: "gpt-5.6-sol".to_string(),
            effort: Mutex::new("medium".to_string()),
            levels: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            metadata: RouteMetadata {
                description: Some("Latest frontier agentic coding model.".to_string()),
                context_window: Some(372_000),
                input_modalities: vec!["text".to_string(), "image".to_string()],
                speed_tiers: vec!["fast".to_string()],
                reasoning_descriptions: vec![
                    (
                        "low".to_string(),
                        "Fast responses for straightforward tasks.".to_string(),
                    ),
                    (
                        "medium".to_string(),
                        "Balances speed and reasoning depth.".to_string(),
                    ),
                    (
                        "high".to_string(),
                        "Greater reasoning depth for complex problems.".to_string(),
                    ),
                ],
                ..RouteMetadata::default()
            },
        });
        Self {
            agents: vec![
                Agent {
                    name: "openai".to_string(),
                    slots: vec![Slot {
                        label: "gpt-5.6-sol".to_string(),
                        club: selectable,
                        available: Arc::new(AtomicBool::new(true)),
                    }],
                    active: 0,
                },
                Agent {
                    name: "beta".to_string(),
                    slots: vec![Slot {
                        label: "model-b".to_string(),
                        club: Arc::new(TestRenderClub {
                            model: "model-b".to_string(),
                        }),
                        available: Arc::new(AtomicBool::new(true)),
                    }],
                    active: 0,
                },
            ],
            in_hand: 0,
            discovery_rx: None,
            meta: Vec::new(),
            driver_pref: None,
            pending_brain: false,
            generation: 0,
            cached_route_choices: std::sync::Mutex::new(None),
            cached_tabs: std::sync::Mutex::new(None),
            cached_in_hand_mode: std::sync::Mutex::new(None),
        }
    }

    pub(crate) fn set_route_available_for_test(
        &self,
        agent_index: usize,
        slot_index: usize,
        available: bool,
    ) {
        if let Some(slot) = self
            .agents
            .get(agent_index)
            .and_then(|agent| agent.slots.get(slot_index))
        {
            slot.available.store(available, Ordering::Relaxed);
        }
    }

    pub(crate) fn for_dual_reasoning_render_test() -> Self {
        let first: Arc<dyn Club> = Arc::new(TestReasoningClub {
            label: "gpt-5.6-sol".to_string(),
            effort: Mutex::new("medium".to_string()),
            levels: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            metadata: RouteMetadata::default(),
        });
        let second: Arc<dyn Club> = Arc::new(TestReasoningClub {
            label: "model-b".to_string(),
            effort: Mutex::new("shallow".to_string()),
            levels: vec!["shallow".to_string(), "deep".to_string()],
            metadata: RouteMetadata {
                description: Some("Independent long-context reasoning route.".to_string()),
                context_window: Some(196_000),
                input_modalities: vec!["text".to_string()],
                speed_tiers: Vec::new(),
                reasoning_descriptions: vec![
                    (
                        "shallow".to_string(),
                        "Short deliberation for routine work.".to_string(),
                    ),
                    (
                        "deep".to_string(),
                        "Sustained deliberation for difficult work.".to_string(),
                    ),
                ],
                ..RouteMetadata::default()
            },
        });
        Self {
            agents: vec![
                Agent {
                    name: "openai".to_string(),
                    slots: vec![Slot {
                        label: "gpt-5.6-sol".to_string(),
                        club: first,
                        available: Arc::new(AtomicBool::new(true)),
                    }],
                    active: 0,
                },
                Agent {
                    name: "beta".to_string(),
                    slots: vec![Slot {
                        label: "model-b".to_string(),
                        club: second,
                        available: Arc::new(AtomicBool::new(true)),
                    }],
                    active: 0,
                },
            ],
            in_hand: 0,
            discovery_rx: None,
            meta: Vec::new(),
            driver_pref: None,
            pending_brain: false,
            generation: 0,
            cached_route_choices: std::sync::Mutex::new(None),
            cached_tabs: std::sync::Mutex::new(None),
            cached_in_hand_mode: std::sync::Mutex::new(None),
        }
    }

    pub(crate) fn for_fixed_reasoning_render_test() -> Self {
        let fixed: Arc<dyn Club> = Arc::new(TestReasoningClub {
            label: "reasoner-fixed".to_string(),
            effort: Mutex::new("high".to_string()),
            levels: Vec::new(),
            metadata: RouteMetadata::default(),
        });
        Self {
            agents: vec![
                Agent {
                    name: "remote".to_string(),
                    slots: vec![Slot {
                        label: "reasoner-fixed".to_string(),
                        club: fixed,
                        available: Arc::new(AtomicBool::new(true)),
                    }],
                    active: 0,
                },
                Agent {
                    name: "beta".to_string(),
                    slots: vec![Slot {
                        label: "model-b".to_string(),
                        club: Arc::new(PracticeClub {
                            latency: Duration::ZERO,
                        }),
                        available: Arc::new(AtomicBool::new(true)),
                    }],
                    active: 0,
                },
            ],
            in_hand: 0,
            discovery_rx: None,
            meta: Vec::new(),
            driver_pref: None,
            pending_brain: false,
            generation: 0,
            cached_route_choices: std::sync::Mutex::new(None),
            cached_tabs: std::sync::Mutex::new(None),
            cached_in_hand_mode: std::sync::Mutex::new(None),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TaskDriverSelectionError {
    Unconfigured,
    Unavailable,
}

impl Bag {
    /// Headless explicit selection must not inherit interactive fallback.
    /// Resolve with the same alias/mode rules, then require that exact slot.
    pub(crate) fn require_task_driver(
        &mut self,
        requested: &str,
    ) -> Result<Arc<dyn Club>, TaskDriverSelectionError> {
        let index = resolve_driver(&mut self.agents, requested)
            .ok_or(TaskDriverSelectionError::Unconfigured)?;
        let agent = &self.agents[index];
        let slot = agent
            .slots
            .get(agent.active)
            .ok_or(TaskDriverSelectionError::Unavailable)?;
        if !slot.available.load(Ordering::Relaxed) {
            return Err(TaskDriverSelectionError::Unavailable);
        }
        self.in_hand = index;
        Ok(self.in_hand())
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/bag__task_driver_selection_tests.rs"]
mod task_driver_selection_tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/bag__bootstrap_spec_tests.rs"]
mod bootstrap_spec_tests;

//! The village — the fleet mirrored into the miniworld, "as above, so below".
//!
//! Every structure corresponds to real, observable fleet state; nothing is
//! decorative. The **forge** is head #2 (the ABT self-training loop on Atlas,
//! polled at `/health`): it glows and smokes while `training` is true, and an
//! adapter promotion fires the town's celebration machinery plus a *permanent*
//! smithy upgrade. The **apprentice** is the trainee model (angel-head2 /
//! qwen3.5:4b) — its level is the adapter version, a failed eval gate is a
//! soot puff, never doom. The **granary** fills with `dataset.total` on a log
//! scale so early samples visibly matter. The **cottages** are the other fleet
//! heads (from an explicit `ANGEL_HEADS_FILE`): a window is lit while the head's
//! `/health` answers, dark while it doesn't. Occasional one-line villager
//! chatter comes from the 4B itself (ollama on Atlas), cached into the
//! persisted state with canned fallbacks so the village still talks offline.
//!
//! All sensing lives on ONE background thread (`spawn_poller`) with staggered
//! probes and bounded timeouts; it reports over an mpsc channel drained in
//! `App::advance`, so nothing here can ever block the UI. Atlas fully down →
//! the village renders from `~/.angel0/village.json` and the buildings go dark.
//!
//! Persistence follows the `world_rewards.json` pattern: versioned JSON,
//! atomic tmp+rename writes, corrupt/absent file = a fresh village, never a
//! crash. The drawing itself lives in `world_viz::World::draw_village`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// The serving model the villager chatter is generated on — the apprentice's
/// own base weights (see `docs`: the ABT loop promotes adapters over it).
const VOICE_MODEL: &str = "qwen3.5:4b";

/// Seconds between full sensing cycles. Generous: the forge trains for
/// minutes-to-hours, so a sub-minute cadence gains nothing.
const POLL_SECS: u64 = 45;

/// Pause between successive head probes inside one cycle, so a sweep never
/// bursts the whole tailnet at once.
const STAGGER_MS: u64 = 750;

/// Longest chatter line the village will display or cache.
const CHATTER_MAX_CHARS: usize = 72;

// ─── env knobs (all documented in docs/ENV.md) ───────────────────────────────

fn env_on(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

/// `ANGEL_VILLAGE` — the whole layer. Default off so an ordinary launch never
/// polls fleet endpoints. `1` enables polling, persistence, and drawing.
pub(crate) fn enabled() -> bool {
    env_on("ANGEL_VILLAGE", false)
}

/// `ANGEL_VILLAGE_FLAVOR` — local-model chatter. `0` keeps the village talking
/// from the canned lines only (no ollama calls).
pub(crate) fn flavor_enabled() -> bool {
    env_on("ANGEL_VILLAGE_FLAVOR", true)
}

/// `ANGEL_FORGE_URL` — the ABT trainer sidecar. Defaults to loopback; remote
/// deployments must provide their route explicitly.
pub(crate) fn forge_url() -> String {
    std::env::var("ANGEL_FORGE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8017".to_string())
}

/// `ANGEL_VILLAGE_VOICE_URL` — the OpenAI-compatible endpoint chatter is
/// generated on. Defaults to a local Ollama-compatible port.
pub(crate) fn voice_url() -> String {
    std::env::var("ANGEL_VILLAGE_VOICE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_string())
}

/// `ANGEL_VILLAGE_STATE` override, else `~/.angel0/village.json`. `None` with no
/// HOME — the village then runs unpersisted rather than crashing.
pub(crate) fn state_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ANGEL_VILLAGE_STATE") {
        return Some(PathBuf::from(p));
    }
    std::env::var_os("HOME").map(|h| Path::new(&h).join(".angel0").join("village.json"))
}

// ─── persisted state: the village survives restarts ─────────────────────────

/// What outlives a session: permanent upgrades, the apprentice's identity,
/// the last-seen dataset size (so the granary renders offline), cached
/// chatter, and the celebration history. Session moods (soot, chatter
/// windows, which lights are lit *right now*) deliberately die with the
/// session — they are re-sensed within a poll cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct VillageState {
    pub(crate) version: u32,
    /// Permanent forge upgrade tier — never decreases, even across a rollback.
    pub(crate) smithy_level: u32,
    /// Last `current_adapter` seen (e.g. "v2"); advancement past it = promotion.
    pub(crate) last_adapter: Option<String>,
    /// The trainee's villager name, generated once then kept forever.
    pub(crate) apprentice_name: String,
    pub(crate) dataset_total: u64,
    pub(crate) chatter: Vec<String>,
    pub(crate) celebrations: Vec<String>,
}

impl Default for VillageState {
    fn default() -> Self {
        VillageState {
            version: 1,
            smithy_level: 0,
            last_adapter: None,
            apprentice_name: String::new(),
            dataset_total: 0,
            chatter: Vec::new(),
            celebrations: Vec::new(),
        }
    }
}

/// Load the persisted village — any failure (no path, missing file, corrupt
/// JSON) reads as a fresh village, never a crash.
pub(crate) fn load_state(path: Option<&Path>) -> VillageState {
    let Some(path) = path else {
        return VillageState::default();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return VillageState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Atomic best-effort save (tmp + rename): a crash mid-write can never leave a
/// half-written village behind, and a read-only disk never takes the town down.
pub(crate) fn save_state(path: &Path, state: &VillageState) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(raw) = crate::secrets::to_redacted_vec(state) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, raw).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

// ─── the live view the world renders ─────────────────────────────────────────

/// Persisted `state` + this session's fleet telemetry + the hamlet layout.
/// Built by `World::enable_village`; a `World` without one (unit tests,
/// `ANGEL_VILLAGE=0`) renders exactly as before.
pub(crate) struct Village {
    pub(crate) state: VillageState,
    pub(crate) path: Option<PathBuf>,
    // Hamlet layout, snapped to land once at enable time.
    pub(crate) forge_pos: (usize, usize),
    pub(crate) granary_pos: (usize, usize),
    pub(crate) cottages: Vec<(String, (usize, usize))>,
    // Live telemetry (re-sensed every poll cycle; dark until the first pulse).
    pub(crate) forge_up: bool,
    pub(crate) training: bool,
    pub(crate) new_samples: u64,
    pub(crate) gpu_free_mib: Option<u64>,
    pub(crate) ollama_models: usize,
    pub(crate) eval_adapter: Option<f64>,
    pub(crate) eval_base: Option<f64>,
    pub(crate) heads_up: Vec<(String, bool)>,
    pub(crate) last_gate_pass: Option<bool>,
    // Bounded beats (tick windows, so `animating()` can settle).
    pub(crate) soot_until: u64,
    pub(crate) chatter_note: String,
    pub(crate) chatter_until: u64,
}

impl Village {
    pub(crate) fn apprentice_level(&self) -> u32 {
        self.state
            .last_adapter
            .as_deref()
            .map(adapter_level)
            .unwrap_or(0)
    }

    pub(crate) fn head_up(&self, id: &str) -> bool {
        self.heads_up
            .iter()
            .find(|(h, _)| h == id)
            .is_some_and(|&(_, up)| up)
    }

    pub(crate) fn apprentice_display_name(&self) -> &str {
        if self.state.apprentice_name.is_empty() {
            "the apprentice"
        } else {
            &self.state.apprentice_name
        }
    }
}

// ─── forge health parsing (pure — unit tested offline) ──────────────────────

/// One `/health` reading from the forge sidecar.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ForgeSnapshot {
    pub(crate) training: bool,
    pub(crate) dataset_total: u64,
    pub(crate) new_samples: u64,
    pub(crate) adapter: Option<String>,
    pub(crate) gate_pass: Option<bool>,
    pub(crate) promoted: Option<bool>,
    pub(crate) eval_loss_adapter: Option<f64>,
    pub(crate) eval_loss_base: Option<f64>,
    pub(crate) gpu_free_mib: Option<u64>,
    pub(crate) ollama_models: usize,
}

/// Tolerant fold of the forge's `/health` JSON — any missing field degrades to
/// its default, never a panic, so a sidecar schema tweak dims a light instead
/// of breaking the town.
pub(crate) fn parse_forge_health(v: &serde_json::Value) -> ForgeSnapshot {
    let cycle = &v["last_cycle"];
    ForgeSnapshot {
        training: v["training"].as_bool().unwrap_or(false),
        dataset_total: v["dataset"]["total"].as_u64().unwrap_or(0),
        new_samples: v["dataset"]["new_since_last_train"].as_u64().unwrap_or(0),
        adapter: v["current_adapter"].as_str().map(str::to_string),
        gate_pass: cycle["gate_pass"].as_bool(),
        promoted: cycle["promoted"].as_bool(),
        eval_loss_adapter: cycle["eval_loss_adapter"].as_f64(),
        eval_loss_base: cycle["eval_loss_base"].as_f64(),
        gpu_free_mib: v["gpu_free_mib"].as_u64(),
        ollama_models: v["ollama_models"].as_array().map_or(0, Vec::len),
    }
}

/// Path to free-train last-cycle handoff (`FORGE_LAST_CYCLE` or
/// `~/.angel0/forge-last-cycle.json`).
pub(crate) fn forge_last_cycle_path() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("FORGE_LAST_CYCLE") {
        let p = PathBuf::from(raw);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    std::env::var_os("HOME").map(|h| Path::new(&h).join(".angel0").join("forge-last-cycle.json"))
}

/// Merge free-train local handoff into a forge snapshot (AUTOPROMOTE=0 path).
///
/// Remote `/health` often has empty `last_cycle` / no ollama pin when free-train
/// leaves adapters on disk only. Local `forge-last-cycle.json` still has version
/// + gate so the village can advance the apprentice level and soot/gate lights.
pub(crate) fn merge_local_last_cycle(mut snap: ForgeSnapshot) -> ForgeSnapshot {
    let Some(path) = forge_last_cycle_path() else {
        return snap;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return snap;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return snap;
    };
    if snap.gate_pass.is_none() {
        snap.gate_pass = v.get("gate_pass").and_then(|x| x.as_bool());
    }
    if snap.promoted.is_none() {
        snap.promoted = v.get("promoted").and_then(|x| x.as_bool());
    }
    if snap.eval_loss_adapter.is_none() {
        snap.eval_loss_adapter = v.get("eval_loss_adapter").and_then(|x| x.as_f64());
    }
    if snap.eval_loss_base.is_none() {
        snap.eval_loss_base = v.get("eval_loss_base").and_then(|x| x.as_f64());
    }
    // Prefer remote current_adapter; fall back to free-train version tag (vN).
    if snap.adapter.is_none()
        && let Some(ver) = v.get("version").and_then(|x| x.as_str())
        && !ver.is_empty()
    {
        snap.adapter = Some(ver.to_string());
    }
    snap
}

/// Path to free-train live status (`FORGE_WHEN_FREE_STATUS` or
/// `~/.angel0/forge-when-free-status.json`).
pub(crate) fn forge_when_free_status_path() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("FORGE_WHEN_FREE_STATUS") {
        let p = PathBuf::from(raw);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    std::env::var_os("HOME").map(|h| {
        Path::new(&h)
            .join(".angel0")
            .join("forge-when-free-status.json")
    })
}

/// Merge free-train pulse/finalize status so the village forge glows mid-LoRA.
///
/// Turbo free-train often leaves Atlas `/health.training=false` while the job
/// runs on a different box; local status from forge-train-pulse is authoritative
/// for "is free-train still cooking".
pub(crate) fn merge_local_free_train_status(mut snap: ForgeSnapshot) -> ForgeSnapshot {
    let Some(path) = forge_when_free_status_path() else {
        return snap;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return snap;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return snap;
    };
    let state = v
        .get("state")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let job_state = v
        .get("job_state")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let live = matches!(state.as_str(), "training" | "reclaiming" | "densifying")
        || matches!(job_state.as_str(), "running" | "queued");
    if live {
        snap.training = true;
    }
    if snap.gpu_free_mib.is_none() {
        snap.gpu_free_mib = v.get("gpu_free_mib").and_then(|x| x.as_u64()).or_else(|| {
            v.get("gpu_free_mib")
                .and_then(|x| x.as_i64())
                .map(|i| i as u64)
        });
    }
    // In-flight job may already know the version once harvest starts writing.
    if snap.adapter.is_none()
        && let Some(ver) = v.get("version").and_then(|x| x.as_str())
        && !ver.is_empty()
    {
        snap.adapter = Some(ver.to_string());
    }
    if snap.gate_pass.is_none() {
        snap.gate_pass = v.get("gate_pass").and_then(|x| x.as_bool());
    }
    if snap.promoted.is_none() {
        snap.promoted = v.get("promoted").and_then(|x| x.as_bool());
    }
    snap
}

/// Adapter tag → level: the LAST run of digits in the string ("v2" → 2,
/// "angel-head2:v13" → 13, no digits → 0).
pub(crate) fn adapter_level(adapter: &str) -> u32 {
    let mut best = 0u32;
    let mut cur = 0u32;
    let mut in_num = false;
    for c in adapter.chars() {
        if let Some(d) = c.to_digit(10) {
            cur = cur.saturating_mul(10).saturating_add(d);
            in_num = true;
        } else {
            if in_num {
                best = cur;
            }
            cur = 0;
            in_num = false;
        }
    }
    if in_num {
        best = cur;
    }
    best
}

/// Granary fill fraction from `dataset.total`, log-scaled so the first
/// samples visibly move the bar (10 samples ≈ 22%, saturating near 50k).
pub(crate) fn granary_fill(total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    let f = ((total as f64) + 1.0).ln() / (50_000f64 + 1.0).ln();
    f.clamp(0.0, 1.0) as f32
}

// ─── the pulse: what one sensing cycle reports to the UI thread ──────────────

#[derive(Clone, Debug)]
pub(crate) struct HeadLight {
    pub(crate) id: String,
    pub(crate) up: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VillagePulse {
    /// `None` = the forge didn't answer this cycle (village goes dark, state
    /// persists untouched).
    pub(crate) forge: Option<ForgeSnapshot>,
    pub(crate) heads: Vec<HeadLight>,
    /// A fresh chatter line (already clipped), generated rarely — on promotion
    /// or first village load — with canned fallbacks when the 4B is offline.
    pub(crate) chatter: Option<String>,
    /// The apprentice's name, generated once when the state carries none.
    pub(crate) apprentice_name: Option<String>,
}

// ─── explicit head registry (JSON → cottages) ────────────────────────────────

/// `(id, health_url)` per non-forge head. Reads only the explicit
/// `ANGEL_HEADS_FILE`; unset, unreadable, or invalid input falls back to
/// loopback-only service ports rather than a particular operator's fleet.
pub(crate) fn discover_heads() -> Vec<(String, String)> {
    if let Some(path) = std::env::var_os("ANGEL_HEADS_FILE").filter(|path| !path.is_empty())
        && let Ok(text) = std::fs::read_to_string(path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
    {
        let heads = heads_from_json(&value);
        if !heads.is_empty() {
            return heads;
        }
    }
    vec![
        ("dice".into(), "http://127.0.0.1:8008/health".into()),
        ("math".into(), "http://127.0.0.1:8011/health".into()),
        ("ocr".into(), "http://127.0.0.1:8018/health".into()),
    ]
}

/// Fold a head-registry JSON value into `(id, health_url)`. The forge is excluded — it is the
/// forge building, polled in full via `ANGEL_FORGE_URL` (env wins over file).
pub(crate) fn heads_from_json(v: &serde_json::Value) -> Vec<(String, String)> {
    let Some(heads) = v["heads"].as_array() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for h in heads {
        let Some(id) = h["id"].as_str() else { continue };
        if id == "forge" {
            continue;
        }
        let Some(base) = h["baseURL"].as_str() else {
            continue;
        };
        let health = h["health"].as_str().unwrap_or("/health");
        out.push((
            id.to_string(),
            format!("{}{}", base.trim_end_matches('/'), health),
        ));
    }
    out
}

// ─── chatter: the 4B's own voice, with canned lines for the silence ──────────

/// Offline small-talk, hash-picked so the village never goes mute.
const FALLBACK_LINES: &[&str] = &[
    "the forge sleeps, but the anvil remembers",
    "fair skies over the granary today",
    "the apprentice sharpens the tongs and waits",
    "word from Atlas comes slow this season",
    "the wheat is in and the samples are counted",
    "smoke on the hill means the smith is learning",
];

/// Offline apprentice names, hash-picked when the 4B can't be asked.
const FALLBACK_NAMES: &[&str] = &["Wat", "Tam", "Perkin", "Rowan", "Dickon", "Elric", "Nell"];

fn time_roll() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn fallback_line() -> String {
    FALLBACK_LINES[(time_roll() % FALLBACK_LINES.len() as u64) as usize].to_string()
}

fn fallback_name() -> String {
    FALLBACK_NAMES[(time_roll() % FALLBACK_NAMES.len() as u64) as usize].to_string()
}

/// First line only, quotes shed, hard-capped — whatever the model says, the
/// status line gets at most one short utterance.
pub(crate) fn clip_line(text: &str, max: usize) -> String {
    let first = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .trim_matches(|c| c == '"' || c == '\u{201C}' || c == '\u{201D}' || c == '\'')
        .trim();
    if first.chars().count() <= max {
        return first.to_string();
    }
    let mut out: String = first.chars().take(max.saturating_sub(1)).collect();
    out.push('~');
    out
}

/// Pull the utterance out of an OpenAI-compatible chat response. qwen3.5 is a
/// reasoning model: with tight budgets `message.content` comes back empty and
/// the text sits in a `reasoning` field — fall back to its last non-empty line.
pub(crate) fn flavor_from_response(v: &serde_json::Value) -> Option<String> {
    let msg = &v["choices"][0]["message"];
    let content = msg["content"].as_str().unwrap_or("").trim();
    let text = if !content.is_empty() {
        content.to_string()
    } else {
        let reasoning = msg["reasoning"]
            .as_str()
            .or_else(|| msg["reasoning_content"].as_str())
            .unwrap_or("");
        reasoning
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())?
            .trim()
            .to_string()
    };
    let clipped = clip_line(&text, CHATTER_MAX_CHARS);
    (!clipped.is_empty()).then_some(clipped)
}

// ─── the poller: one background thread, staggered probes, bounded timeouts ───

/// Everything the sensing thread needs, handed over at spawn so it never
/// touches `App` or the environment again.
pub(crate) struct PollerSeed {
    pub(crate) forge_url: String,
    pub(crate) voice_url: String,
    pub(crate) flavor: bool,
    pub(crate) heads: Vec<(String, String)>,
    pub(crate) last_adapter: Option<String>,
    pub(crate) need_name: bool,
    pub(crate) need_boot_chatter: bool,
    pub(crate) town: String,
}

/// Short, hard-capped probe client — a wedged head can cost one cycle at most.
fn probe_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .user_agent("angel0-village/0.1")
        .build()
}

/// The chatter client gets more read headroom — a 4B takes a moment to speak —
/// but still a hard total cap. Background thread only; the UI never waits.
fn voice_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(4))
        .timeout(Duration::from_secs(30))
        .user_agent("angel0-village/0.1")
        .build()
}

fn fetch_forge(agent: &ureq::Agent, base: &str) -> Option<ForgeSnapshot> {
    let url = format!("{}/health", base.trim_end_matches('/'));
    let remote_ok = agent
        .get(&url)
        .call()
        .ok()
        .and_then(|r| r.into_json::<serde_json::Value>().ok());
    let mut snap = remote_ok
        .as_ref()
        .map(parse_forge_health)
        .unwrap_or_default();
    snap = merge_local_last_cycle(snap);
    snap = merge_local_free_train_status(snap);
    // Keep the forge lit offline when free-train handoff exists (remote dark).
    if remote_ok.is_some()
        || snap.adapter.is_some()
        || snap.gate_pass.is_some()
        || snap.training
        || snap.dataset_total > 0
    {
        Some(snap)
    } else {
        None
    }
}

/// Reachability: any HTTP answer counts as "the light is on" — a 404 means the
/// process is alive; only a transport failure darkens the window.
fn probe(agent: &ureq::Agent, url: &str) -> bool {
    match agent.get(url).call() {
        Ok(_) => true,
        Err(ureq::Error::Status(_, _)) => true,
        Err(_) => false,
    }
}

/// One line from the 4B, or `None` (caller falls back to the canned lines).
fn generate_flavor(agent: &ureq::Agent, voice_url: &str, prompt: &str) -> Option<String> {
    let url = format!("{}/v1/chat/completions", voice_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": VOICE_MODEL,
        "messages": [{"role": "user", "content": prompt}],
        // A reasoning model with a tight budget returns empty content — 200
        // leaves room for the think AND the line (see flavor_from_response).
        "max_tokens": 200,
        "temperature": 0.9,
    });
    let v: serde_json::Value = agent.post(&url).send_json(body).ok()?.into_json().ok()?;
    flavor_from_response(&v)
}

/// Spawn the single village sensing thread. Cycle: forge `/health`, then each
/// head staggered, then (rarely) a chatter/name generation; one `VillagePulse`
/// per cycle over the channel. Exits when the receiver drops.
pub(crate) fn spawn_poller(seed: PollerSeed) -> mpsc::Receiver<VillagePulse> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("village-poll".into())
        .spawn(move || {
            let probes = probe_agent();
            let voice = voice_agent();
            let mut last_adapter = seed.last_adapter.clone();
            let mut named = !seed.need_name;
            let mut boot_chatter = seed.need_boot_chatter;
            loop {
                let forge = fetch_forge(&probes, &seed.forge_url);
                let mut heads = Vec::with_capacity(seed.heads.len());
                for (id, url) in &seed.heads {
                    heads.push(HeadLight {
                        id: id.clone(),
                        up: probe(&probes, url),
                    });
                    std::thread::sleep(Duration::from_millis(STAGGER_MS));
                }
                // Name the apprentice once, ever.
                let mut apprentice_name = None;
                if !named {
                    let asked = seed.flavor.then(|| {
                        generate_flavor(
                            &voice,
                            &seed.voice_url,
                            "Invent one medieval peasant first name for a blacksmith's \
                             apprentice. Reply with the name only.",
                        )
                    });
                    let name = asked.flatten().map(|n| clip_line(&n, 12));
                    apprentice_name = Some(name.unwrap_or_else(fallback_name));
                    named = true;
                }
                // Chatter fires on promotion or the village's first load only.
                let promoted_now = forge
                    .as_ref()
                    .is_some_and(|f| f.adapter.is_some() && f.adapter != last_adapter);
                let mut chatter = None;
                if promoted_now || boot_chatter {
                    let prompt = if promoted_now {
                        format!(
                            "You are a blacksmith's apprentice in the castle town of {}. \
                             You were just promoted to a new adapter version. Say one short \
                             joyful line about it in a medieval villager voice, at most 12 \
                             words. No quotes.",
                            seed.town
                        )
                    } else {
                        format!(
                            "You are a villager in the castle town of {}, a tiny world \
                             mirroring a real GPU fleet. Say one short line of village \
                             small-talk in a medieval voice, at most 12 words. No quotes.",
                            seed.town
                        )
                    };
                    let line = seed
                        .flavor
                        .then(|| generate_flavor(&voice, &seed.voice_url, &prompt))
                        .flatten();
                    chatter = Some(line.unwrap_or_else(fallback_line));
                }
                if promoted_now {
                    last_adapter = forge.as_ref().and_then(|f| f.adapter.clone());
                }
                boot_chatter = false;
                if tx
                    .send(VillagePulse {
                        forge,
                        heads,
                        chatter,
                        apprentice_name,
                    })
                    .is_err()
                {
                    return; // UI gone — stand down
                }
                std::thread::sleep(Duration::from_secs(POLL_SECS));
            }
        });
    rx
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/village__tests.rs"]
mod tests;

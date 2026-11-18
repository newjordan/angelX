//! The village — the fleet mirrored into the miniworld, "as above, so below".
//!
//! Every structure corresponds to real, observable fleet state; nothing is
//! decorative. The **forge** is head #2 (the ABT self-training loop on the
//! forge host, polled at `/health`): it glows and smokes while `training` is true, and an
//! adapter promotion fires the town's celebration machinery plus a *permanent*
//! smithy upgrade. The **apprentice** is the trainee model (angel-head2 /
//! qwen3.5:4b) — its level is the adapter version, a failed eval gate is a
//! soot puff, never doom. The **granary** fills with `dataset.total` on a log
//! scale so early samples visibly matter. The **cottages** are the other fleet
//! heads (from an explicit `ANGEL_HEADS_FILE`): a window is lit while the head's
//! `/health` answers, dark while it doesn't. Occasional one-line villager
//! chatter comes from the 4B itself (ollama on the workstation), cached into the
//! persisted state with canned fallbacks so the village still talks offline.
//!
//! All sensing lives on ONE background thread (`spawn_poller`) with staggered
//! probes and bounded timeouts; it reports over an mpsc channel drained in
//! `App::advance`, so nothing here can ever block the UI. The workstation fully down →
//! the village renders from `~/.angelX/village.json` and the buildings go dark.
//!
//! Persistence follows the `world_rewards.json` pattern: versioned JSON,
//! atomic tmp+rename writes, corrupt/absent file = a fresh village, never a
//! crash. The drawing itself lives in `world_viz::World::draw_village`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

const VOICE_MODEL: &str = "qwen3.5:4b";

const POLL_SECS: u64 = 45;

const STAGGER_MS: u64 = 750;

const CHATTER_MAX_CHARS: usize = 72;

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

pub fn enabled() -> bool {
    env_on("ANGEL_VILLAGE", false)
}

pub fn flavor_enabled() -> bool {
    env_on("ANGEL_VILLAGE_FLAVOR", true)
}

pub fn forge_url() -> String {
    std::env::var("ANGEL_FORGE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8017".to_string())
}

pub fn voice_url() -> String {
    std::env::var("ANGEL_VILLAGE_VOICE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_string())
}

pub fn state_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ANGEL_VILLAGE_STATE") {
        return Some(PathBuf::from(p));
    }
    std::env::var_os("HOME").map(|h| Path::new(&h).join(".angelX").join("village.json"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VillageState {
    pub version: u32,
    pub smithy_level: u32,
    pub last_adapter: Option<String>,
    pub apprentice_name: String,
    pub dataset_total: u64,
    pub chatter: Vec<String>,
    pub celebrations: Vec<String>,
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

pub fn load_state(path: Option<&Path>) -> VillageState {
    let Some(path) = path else {
        return VillageState::default();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return VillageState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_state(path: &Path, state: &VillageState) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(raw) = angel_live::host::secrets::to_redacted_vec(state) else {
        return;
    };
    let _ = angel_live::host::workspace_store::write_atomic(path, &raw);
}

pub struct Village {
    pub state: VillageState,
    pub path: Option<PathBuf>,
    pub forge_pos: (usize, usize),
    pub granary_pos: (usize, usize),
    pub cottages: Vec<(String, (usize, usize))>,
    pub forge_up: bool,
    pub training: bool,
    pub new_samples: u64,
    pub gpu_free_mib: Option<u64>,
    pub ollama_models: usize,
    pub eval_adapter: Option<f64>,
    pub eval_base: Option<f64>,
    pub heads_up: Vec<(String, bool)>,
    pub last_gate_pass: Option<bool>,
    pub soot_until: u64,
    pub chatter_note: String,
    pub chatter_until: u64,
}

impl Village {
    pub fn apprentice_level(&self) -> u32 {
        self.state
            .last_adapter
            .as_deref()
            .map(adapter_level)
            .unwrap_or(0)
    }

    pub fn head_up(&self, id: &str) -> bool {
        self.heads_up
            .iter()
            .find(|(h, _)| h == id)
            .is_some_and(|&(_, up)| up)
    }

    pub fn apprentice_display_name(&self) -> &str {
        if self.state.apprentice_name.is_empty() {
            "the apprentice"
        } else {
            &self.state.apprentice_name
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForgeSnapshot {
    pub training: bool,
    pub dataset_total: u64,
    pub new_samples: u64,
    pub adapter: Option<String>,
    pub gate_pass: Option<bool>,
    pub promoted: Option<bool>,
    pub eval_loss_adapter: Option<f64>,
    pub eval_loss_base: Option<f64>,
    pub gpu_free_mib: Option<u64>,
    pub ollama_models: usize,
}

pub fn parse_forge_health(v: &serde_json::Value) -> ForgeSnapshot {
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

pub fn forge_last_cycle_path() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("FORGE_LAST_CYCLE") {
        let p = PathBuf::from(raw);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    std::env::var_os("HOME").map(|h| Path::new(&h).join(".angelX").join("forge-last-cycle.json"))
}

pub fn merge_local_last_cycle(mut snap: ForgeSnapshot) -> ForgeSnapshot {
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
    if snap.adapter.is_none()
        && let Some(ver) = v.get("version").and_then(|x| x.as_str())
        && !ver.is_empty()
    {
        snap.adapter = Some(ver.to_string());
    }
    snap
}

pub fn forge_when_free_status_path() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("FORGE_WHEN_FREE_STATUS") {
        let p = PathBuf::from(raw);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    std::env::var_os("HOME").map(|h| {
        Path::new(&h)
            .join(".angelX")
            .join("forge-when-free-status.json")
    })
}

pub fn merge_local_free_train_status(mut snap: ForgeSnapshot) -> ForgeSnapshot {
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

pub fn adapter_level(adapter: &str) -> u32 {
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

pub fn granary_fill(total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    let f = ((total as f64) + 1.0).ln() / (50_000f64 + 1.0).ln();
    f.clamp(0.0, 1.0) as f32
}

#[derive(Clone, Debug)]
pub struct HeadLight {
    pub id: String,
    pub up: bool,
}

#[derive(Clone, Debug, Default)]
pub struct VillagePulse {
    pub forge: Option<ForgeSnapshot>,
    pub heads: Vec<HeadLight>,
    pub chatter: Option<String>,
    pub apprentice_name: Option<String>,
}

pub fn discover_heads() -> Vec<(String, String)> {
    if let Some(path) = std::env::var_os("ANGEL_HEADS_FILE").filter(|path| !path.is_empty())
        && let Ok(text) = std::fs::read_to_string(path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
    {
        let heads = heads_from_json(&value);
        if !heads.is_empty() {
            return heads;
        }
    }
    Vec::new()
}

pub fn heads_from_json(v: &serde_json::Value) -> Vec<(String, String)> {
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

const FALLBACK_LINES: &[&str] = &[
    "the forge sleeps, but the anvil remembers",
    "fair skies over the granary today",
    "the apprentice sharpens the tongs and waits",
    "word from the far hall comes slow this season",
    "the wheat is in and the samples are counted",
    "smoke on the hill means the smith is learning",
];

const FALLBACK_NAMES: &[&str] = &["Wat", "Tam", "Perkin", "Rowan", "Dickon", "Elric", "Nell"];

fn time_roll() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn fallback_line() -> String {
    FALLBACK_LINES[(time_roll() % FALLBACK_LINES.len() as u64) as usize].to_string()
}

fn fallback_name() -> String {
    FALLBACK_NAMES[(time_roll() % FALLBACK_NAMES.len() as u64) as usize].to_string()
}

pub fn clip_line(text: &str, max: usize) -> String {
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

pub fn flavor_from_response(v: &serde_json::Value) -> Option<String> {
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

pub struct PollerSeed {
    pub forge_url: String,
    pub voice_url: String,
    pub flavor: bool,
    pub heads: Vec<(String, String)>,
    pub last_adapter: Option<String>,
    pub need_name: bool,
    pub need_boot_chatter: bool,
    pub town: String,
}

fn fetch_forge(base: &str) -> Option<ForgeSnapshot> {
    let url = format!("{}/health", base.trim_end_matches('/'));
    let remote_ok = crate::hooks::http_get_json(&url);
    let mut snap = remote_ok
        .as_ref()
        .map(parse_forge_health)
        .unwrap_or_default();
    snap = merge_local_last_cycle(snap);
    snap = merge_local_free_train_status(snap);
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

fn probe(url: &str) -> bool {
    crate::hooks::http_probe(url)
}

fn generate_flavor(voice_url: &str, prompt: &str) -> Option<String> {
    let url = format!("{}/v1/chat/completions", voice_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": VOICE_MODEL,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 200,
        "temperature": 0.9,
    });
    let v = crate::hooks::http_post_json(&url, body)?;
    flavor_from_response(&v)
}

pub fn spawn_poller(seed: PollerSeed) -> mpsc::Receiver<VillagePulse> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("village-poll".into())
        .spawn(move || {
            let mut last_adapter = seed.last_adapter.clone();
            let mut named = !seed.need_name;
            let mut boot_chatter = seed.need_boot_chatter;
            loop {
                let forge = fetch_forge(&seed.forge_url);
                let mut heads = Vec::with_capacity(seed.heads.len());
                for (id, url) in &seed.heads {
                    heads.push(HeadLight {
                        id: id.clone(),
                        up: probe(url),
                    });
                    std::thread::sleep(Duration::from_millis(STAGGER_MS));
                }
                let mut apprentice_name = None;
                if !named {
                    let asked = seed.flavor.then(|| {
                        generate_flavor(
                            &seed.voice_url,
                            "Invent one medieval peasant first name for a blacksmith's \
                             apprentice. Reply with the name only.",
                        )
                    });
                    let name = asked.flatten().map(|n| clip_line(&n, 12));
                    apprentice_name = Some(name.unwrap_or_else(fallback_name));
                    named = true;
                }
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
                        .then(|| generate_flavor(&seed.voice_url, &prompt))
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
                    return;
                }
                std::thread::sleep(Duration::from_secs(POLL_SECS));
            }
        });
    rx
}

#[cfg(test)]
#[path = "../tests/village__tests.rs"]
mod tests;

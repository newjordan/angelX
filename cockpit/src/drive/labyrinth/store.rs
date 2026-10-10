//! Curated knowledge stays intact. Automatic observations are immutable local
//! fragments, merged on read by both the Rust adapter and bundled Python engine.

use super::{MAX_NODES, Map};
use crate::agent::harness::{confined_publish_new_no_symlinks, confined_read_limited_no_symlinks};
use crate::knowledge::cut::sha256_hex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const KNOWLEDGE: &str = "labyrinth/knowledge.json";
const OBSERVATIONS: &str = "labyrinth/angel/observations";
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_FRAGMENTS: usize = 2048;

fn ignore_observations(workspace: &Path) -> Result<(), String> {
    let path = Path::new("labyrinth/angel/.gitignore");
    if !exists(workspace, path) {
        // This ignores itself as well as future immutable fragments. Automatic
        // research telemetry must not look like a source edit to /loop's Git
        // progress detector or become part of an RL candidate snapshot.
        match confined_publish_new_no_symlinks(workspace, path, b"*\n") {
            Ok(()) => {}
            Err(_) if read(workspace, path, 4096).is_ok_and(|bytes| bytes == b"*\n") => {}
            Err(error) => return Err(error),
        }
    }
    if read(workspace, path, 4096)? != b"*\n" {
        return Err("labyrinth/angel/.gitignore must contain only '*' to keep automatic observations out of candidate progress".into());
    }
    Ok(())
}

fn read(workspace: &Path, relative: &Path, limit: usize) -> Result<Vec<u8>, String> {
    confined_read_limited_no_symlinks(workspace, relative, limit)?
        .ok_or_else(|| format!("{} exceeds labyrinth byte budget", relative.display()))
}

fn exists(workspace: &Path, relative: &Path) -> bool {
    std::fs::symlink_metadata(workspace.join(relative)).is_ok()
}

pub(crate) fn initialize(workspace: &Path) -> Result<(), String> {
    // Validate an existing map first. Never replace curated knowledge or an
    // operator's engine/dashboard with our copies.
    if exists(workspace, Path::new(KNOWLEDGE)) {
        load(workspace)?;
    }
    for (name, bytes) in [
        (
            "labyrinth/lab.py",
            include_bytes!("../../../research/labyrinth/lab.py").as_slice(),
        ),
        (
            "labyrinth/dashboard/template.html",
            include_bytes!("../../../research/labyrinth/dashboard.html").as_slice(),
        ),
        (
            KNOWLEDGE,
            b"{\"schema\":1,\"meta\":{\"title\":\"Angel research labyrinth\"},\"nodes\":[]}\n"
                .as_slice(),
        ),
    ] {
        if !exists(workspace, Path::new(name)) {
            confined_publish_new_no_symlinks(workspace, Path::new(name), bytes)?;
        } else {
            read(workspace, Path::new(name), 16 * 1024 * 1024)?;
        }
    }
    ignore_observations(workspace)?;
    crate::agent::harness::confined_open_rw_no_symlinks(
        workspace,
        Path::new("labyrinth/angel/workflow.lock"),
        true,
    )?;
    super::resources::initialize(workspace)?;
    Ok(())
}

pub(crate) fn load(workspace: &Path) -> Result<Option<Map>, String> {
    let _lease = super::workflow::read_lease(workspace)?;
    super::workflow::ensure_ready(workspace)?;
    if !exists(workspace, Path::new(KNOWLEDGE)) {
        return Ok(None);
    }
    let mut document: Value =
        serde_json::from_slice(&read(workspace, Path::new(KNOWLEDGE), MAX_BYTES)?)
            .map_err(|error| format!("invalid labyrinth knowledge: {error}"))?;
    // Native reads never execute a workspace-authored Python blueprint. Make
    // unsupported imports explicit instead of showing an incomplete map.
    if exists(workspace, Path::new("tools/blueprint_data.py"))
        || document["blueprint_overrides"]
            .as_object()
            .is_some_and(|overrides| !overrides.is_empty())
    {
        return Err("native labyrinth requires materialized blueprint nodes in knowledge.json (Python engine can import blueprints)".into());
    }
    let nodes = document["nodes"]
        .as_array()
        .ok_or("labyrinth needs a nodes array")?;
    if nodes.len() > MAX_NODES {
        return Err("labyrinth node budget exceeded".into());
    }
    let mut merged = BTreeMap::<String, Value>::new();
    let mut curated = BTreeSet::new();
    for node in nodes {
        let id = node["id"].as_str().ok_or("node needs an id")?.to_string();
        curated.insert(id.clone());
        if merged.insert(id.clone(), node.clone()).is_some() {
            return Err(format!("duplicate node {id}"));
        }
    }
    let dir = workspace.join(OBSERVATIONS);
    if exists(workspace, Path::new(OBSERVATIONS)) {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&dir).map_err(|error| error.to_string())? {
            let name = entry.map_err(|error| error.to_string())?.file_name();
            let Some(name) = name.to_str().map(str::to_string) else {
                continue;
            };
            if !name.ends_with(".json") {
                continue;
            }
            if name.len() != 69 || !name.as_bytes()[..64].iter().all(|b| b.is_ascii_hexdigit()) {
                return Err("invalid labyrinth observation filename".into());
            }
            names.push(name);
            if names.len() > MAX_FRAGMENTS {
                return Err("labyrinth observation budget exceeded".into());
            }
        }
        names.sort();
        let mut bytes_total = 0usize;
        for name in names {
            let bytes = read(workspace, &Path::new(OBSERVATIONS).join(name), 64 * 1024)?;
            bytes_total += bytes.len();
            if bytes_total > 8 * MAX_BYTES {
                return Err("labyrinth observation byte budget exceeded".into());
            }
            let fragment: Value =
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            if fragment["schema"] != 1 {
                return Err("invalid labyrinth observation schema".into());
            }
            let additions = fragment["nodes"]
                .as_array()
                .filter(|nodes| nodes.len() <= 32)
                .ok_or("invalid labyrinth observation nodes")?;
            for node in additions {
                let id = node["id"]
                    .as_str()
                    .ok_or("observation node needs an id")?
                    .to_string();
                if curated.contains(&id) {
                    continue;
                }
                if let Some(existing) = merged.get_mut(&id) {
                    // A task's doorway is shared by every observation. Merge
                    // links without changing any existing claim or tier.
                    let links = existing["links"]
                        .as_array_mut()
                        .ok_or("invalid observation links")?;
                    for link in node["links"].as_array().into_iter().flatten() {
                        if !links.contains(link) {
                            links.push(link.clone());
                        }
                    }
                } else {
                    merged.insert(id, node.clone());
                }
                if merged.len() > MAX_NODES {
                    return Err("labyrinth node budget exceeded".into());
                }
            }
        }
    }
    document["nodes"] = json!(merged.into_values().collect::<Vec<_>>());
    Map::parse(document).map(Some)
}

fn bounded(value: &str) -> String {
    value.chars().take(2000).collect()
}

pub(super) fn task_id(task: &str) -> String {
    let task = task.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("q.angel-{}", &sha256_hex(task.as_bytes())[..24])
}

fn task_node(task: &str, links: Vec<Value>) -> Value {
    json!({"id":task_id(task),"kind":"question","status":"open", "title":bounded(task),
        "statement":bounded(task),"links":links,"source":"angel"})
}

/// Failures are visible diagnostics and cannot break the original Deli/RL run.
fn publish(
    workspace: &Path,
    nodes: Vec<Value>,
    event_type: &str,
    summary: &str,
    evidence: Vec<String>,
) -> Result<(), String> {
    let Some(map) = load(workspace)? else {
        return Ok(());
    };
    ignore_observations(workspace)?;
    let mut document = map.document;
    let mut all = document["nodes"].as_array().unwrap().clone();
    for node in &nodes {
        if let Some(existing) = all.iter_mut().find(|old| old["id"] == node["id"]) {
            if existing["source"] == "angel" {
                let links = existing["links"]
                    .as_array_mut()
                    .ok_or("invalid observation links")?;
                for link in node["links"].as_array().into_iter().flatten() {
                    if !links.contains(link) {
                        links.push(link.clone());
                    }
                }
            }
        } else {
            all.push(node.clone());
        }
    }
    document["nodes"] = json!(all);
    Map::parse(document)?;
    let key = sha256_hex(
        serde_json::to_string(&nodes)
            .map_err(|error| error.to_string())?
            .as_bytes(),
    );
    let name = Path::new(OBSERVATIONS).join(format!("{key}.json"));
    if exists(workspace, &name) {
        return Ok(());
    }
    if exists(workspace, Path::new(OBSERVATIONS))
        && std::fs::read_dir(workspace.join(OBSERVATIONS))
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .take(MAX_FRAGMENTS)
            .count()
            >= MAX_FRAGMENTS
    {
        return Err(
            "labyrinth observation budget exceeded; curate/archive older observations".into(),
        );
    }
    let fragment = json!({"schema":1,"nodes":nodes,"event":{
        "type":event_type,"summary":summary,"nodes":nodes.iter().map(|node|node["id"].clone()).collect::<Vec<_>>(),
        "evidence":evidence,"ts":timestamp()}});
    let bytes = serde_json::to_vec_pretty(&fragment).map_err(|error| error.to_string())?;
    if bytes.len() > 64 * 1024 {
        return Err("labyrinth observation too large".into());
    }
    match confined_publish_new_no_symlinks(workspace, &name, &bytes) {
        Ok(()) => Ok(()),
        // Concurrent identical observations may already have published it.
        Err(_)
            if read(workspace, &name, 64 * 1024).is_ok_and(|old| {
                serde_json::from_slice::<Value>(&old)
                    .is_ok_and(|old| old["nodes"] == fragment["nodes"])
            }) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn timestamp() -> String {
    // ISO UTC with no process or timezone dependency.
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86400) as i64;
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

pub(crate) fn observe_iteration(
    workspace: &Path,
    source: &str,
    task: &str,
    findings: &[String],
    hypotheses: &[String],
) -> Result<(), String> {
    if !exists(workspace, Path::new(KNOWLEDGE)) {
        return Ok(());
    }
    let root = task_id(task);
    let mut nodes = Vec::new();
    let mut links = Vec::new();
    for (items, grounded, reported) in [
        (findings, source == "loop", true),
        (hypotheses, false, false),
    ] {
        for text in items.iter().take(8).filter(|text| !text.trim().is_empty()) {
            let statement = bounded(text);
            let digest =
                sha256_hex(format!("{root}\n{source}\n{grounded}\n{statement}").as_bytes());
            let (kind, tier, prefix) = if grounded {
                ("evidence", "T4", "ev")
            } else if reported {
                ("conjecture", "T5", "k")
            } else {
                ("hunch", "T6", "h")
            };
            let id = format!("{prefix}.angel-{}", &digest[..24]);
            links.push(json!({"to":id,"rel":"suggests"}));
            nodes.push(json!({"id":id,"kind":kind,"tier":tier,"status":if grounded {"observed"} else if reported {"open"} else {"live"},
                "title":statement,"statement":statement,"angel_task":root,"source":source,
                "links":[],"evidence":if grounded { vec![statement.clone()] } else {vec![]},
                "test":"Independently check this claim against the stated objective and retain the verifier/referee evidence.",
                "review":{"state":"under-review"}}));
        }
    }
    if nodes.is_empty() {
        return Ok(());
    }
    nodes.insert(0, task_node(task, links));
    publish(
        workspace,
        nodes,
        "proposed",
        &format!("{source} research observations (under review)"),
        Vec::new(),
    )
}

pub(crate) fn observe_measurement(
    workspace: &Path,
    observation: &Value,
    route: &Value,
) -> Result<(), String> {
    if !exists(workspace, Path::new(KNOWLEDGE)) {
        return Ok(());
    }
    let task = observation["task"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or("measurement needs a task")?;
    for key in ["source_sha256", "receipt_sha256", "evidence_sha256"] {
        let digest = observation[key]
            .as_str()
            .ok_or("measurement needs receipt digests")?;
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("invalid measurement {key}"));
        }
    }
    if !observation["passed"].is_boolean() {
        return Err("measurement needs a boolean verdict".into());
    }
    let root = task_id(task);
    let key = sha256_hex(json!([observation, route]).to_string().as_bytes());
    let id = format!("ev.angel-measured-{}", &key[..24]);
    let idea = bounded(observation["idea"].as_str().unwrap_or("measured attempt"));
    let node = json!({"id":id,"kind":"evidence","tier":"T4","status":"observed",
        "title":idea,"statement":format!("Measured attempt: {idea}. Recorded {} verdict passed: {}. This is evidence for this run, not a general proof or refutation.",observation["approach"].as_str().unwrap_or("experiment"),observation["passed"]),
        "angel_task":root,"links":[{"to":root,"rel":"tests"}],"review":{"state":"under-review"},
        "evidence":[format!("receipt-sha256:{}",observation["receipt_sha256"].as_str().unwrap())],
        "angel_measurement":observation,"angel_route":route});
    publish(
        workspace,
        vec![
            task_node(task, vec![json!({"to":id,"rel":"suggests"})]),
            node,
        ],
        "computed",
        "Measured RL/Sloptomizer observation (T4; under review)",
        vec![observation["receipt_sha256"].as_str().unwrap().to_string()],
    )
}

/// Mirror exact terminal board receipts for every cartridge. Pending states,
/// fixture prose and a rejection for not beating the board are not refutations.
pub(crate) fn observe_submission(
    workspace: &Path,
    notify: &crate::agent::harness::WatchNotify,
) -> Result<(), String> {
    if !exists(workspace, Path::new(KNOWLEDGE)) {
        return Ok(());
    }
    let Some(receipt) = notify.receipt.as_ref() else {
        return Ok(());
    };
    let Some(official) = receipt.official.as_ref() else {
        return Ok(());
    };
    if official.id != notify.id || official.benchmark_id != receipt.benchmark_id {
        return Err("labyrinth submission receipt identity mismatch".into());
    }
    if !matches!(
        official.status.trim().to_ascii_lowercase().as_str(),
        "accepted"
            | "rejected"
            | "failed"
            | "error"
            | "cancelled"
            | "canceled"
            | "timeout"
            | "timed_out"
            | "timed-out"
            | "superseded"
            | "promoted"
    ) {
        return Ok(());
    }
    let task = format!("Benchmark {}", official.benchmark_id);
    let root = task_id(&task);
    let data = serde_json::to_value(official).map_err(|error| error.to_string())?;
    // Poll timestamps are transport metadata, not a distinct research result.
    // Keep the first exact snapshot, and add a new observation only when the
    // board's semantic result changes (for example, rejection or promotion).
    let mut identity = data.clone();
    identity.as_object_mut().unwrap().remove("fetched_at_ms");
    let digest = sha256_hex(json!([receipt.source_id, identity]).to_string().as_bytes());
    let id = format!("ev.angel-submission-{}", &digest[..24]);
    if load(workspace)?.is_some_and(|map| map.nodes.iter().any(|node| node["id"] == id)) {
        return Ok(());
    }
    let node = json!({"id":id,"kind":"evidence","tier":"T4","status":"observed",
        "title":format!("Submission {}: {}",official.id,official.status),
        "statement":"Exact terminal board receipt. Verification, score, rejection and promotion are retained separately; this is not a general proof or an automatic reward.",
        "angel_task":root,"angel_submission":data,"angel_receipt_source":receipt.source_id,
        "links":[{"to":root,"rel":"tests"}],"review":{"state":"under-review"},
        "evidence":[format!("board-receipt-sha256:{digest}")]});
    publish(
        workspace,
        vec![
            task_node(&task, vec![json!({"to":id,"rel":"suggests"})]),
            node,
        ],
        "computed",
        "Exact terminal submission receipt (T4; under review)",
        vec![digest],
    )
}

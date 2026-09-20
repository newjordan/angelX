//! The MoA outcome ledger: one local jsonl line per swarm turn.
//!
//! The pipeline never learned whether any seat earned its cost — every turn's
//! economics evaporated when the turn ended. The ledger keeps them: route,
//! dissent score, what the gate decided, the knobs that actually ran, draft
//! counts, per-club token deltas, latency, and whether the turn succeeded.
//! Pure local writes (no model calls, best-effort I/O), so a broken disk can
//! never fail a turn. `/moa` renders the report; the recorded dissent
//! distribution is how `ANGEL_MOA_DISSENT_HI/LO` get calibrated from data
//! instead of guessed.

use super::*;
use std::path::PathBuf;

/// Whether turns are recorded (`ANGEL_MOA_LEDGER`, default on).
pub(crate) fn ledger_enabled() -> bool {
    env_flag_or("ANGEL_MOA_LEDGER", true)
}

/// Ledger file: `~/.angel0/moa/ledger.jsonl`, or `ANGEL_MOA_LEDGER_PATH`.
pub(crate) fn ledger_path() -> PathBuf {
    match std::env::var("ANGEL_MOA_LEDGER_PATH") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::workspace_store::angel_subdir("moa").join("ledger.jsonl"),
    }
}

/// What one `run()` learned about itself, filled in as the turn unfolds.
#[derive(Default)]
pub(crate) struct TurnTrace {
    /// "direct" (tight/single pass), "deliberate" (full pipeline), or
    /// "fallback" (the pipeline degraded to a direct pass mid-turn).
    pub(crate) route: &'static str,
    /// Mean pairwise proposer dissent, when the gate measured it.
    pub(crate) dissent: Option<f64>,
    /// What the gate did: "escalate" | "hold" | "relax".
    pub(crate) gate: Option<&'static str>,
    /// Proposer drafts collected / surviving dedup collapse.
    pub(crate) proposed: usize,
    pub(crate) kept: usize,
    /// The knobs that actually ran after gating: (layers, judge, verify, samples).
    pub(crate) effective: Option<(usize, bool, usize, usize)>,
    /// Progressive proposer-wave telemetry.
    pub(crate) waves: Vec<WaveTrace>,
    /// Scale coordination telemetry, absent for small turns.
    pub(crate) scale: Option<ScaleTrace>,
    /// The engaged formation's name, when a formation owned the turn — so
    /// per-formation economics are measurable from the ledger rows alone.
    pub(crate) formation: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WaveTrace {
    pub(crate) launched: usize,
    pub(crate) returned: usize,
    pub(crate) novel: usize,
    pub(crate) dissent: Option<f64>,
    pub(crate) decision: String,
    pub(crate) ms: u128,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ScaleTrace {
    pub(crate) clusters: usize,
    pub(crate) dominant: f64,
    pub(crate) eclusters: f64,
    pub(crate) judge_rounds: usize,
    pub(crate) reduce_rounds: usize,
    pub(crate) gate_src: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MoaModelTokenRow {
    pub(crate) label: String,
    pub(crate) last_input: u64,
    pub(crate) last_output: u64,
    pub(crate) total_input: u64,
    pub(crate) total_output: u64,
}

impl MoaModelTokenRow {
    pub(crate) fn last_total(&self) -> u64 {
        self.last_input + self.last_output
    }

    pub(crate) fn session_total(&self) -> u64 {
        self.total_input + self.total_output
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MoaTokenReport {
    pub(crate) turns: usize,
    pub(crate) latest_route: String,
    pub(crate) latest_ok: bool,
    pub(crate) latest_ms: u64,
    pub(crate) rows: Vec<MoaModelTokenRow>,
}

/// Snapshot of cumulative token usage per unique club label. Taken before and
/// after a turn; the difference is that turn's spend. (Cumulative counters are
/// process-wide, so traffic from a concurrent turn on the same club would land
/// in whichever snapshot window it overlaps — close enough for economics.)
pub(crate) fn usage_by_club(
    clubs: &RoleClubs,
    fallbacks: &[Arc<dyn Club>],
) -> Vec<(String, u64, u64)> {
    let mut out: Vec<(String, u64, u64)> = Vec::new();
    // Raw failover links first. Cheap-role wrappers each expose the whole same
    // fallback chain through `token_usage`; counting the wrappers first would
    // multiply real provider spend by the number of role labels.
    for club in fallbacks
        .iter()
        .chain([
            &clubs.propose,
            &clubs.judge,
            &clubs.verify,
            &clubs.aggregate,
        ])
        .chain(clubs.propose_extra.iter().map(|(club, _)| club))
        .chain(clubs.judge_extra.iter().map(|(club, _)| club))
        .chain(clubs.verify_extra.iter().map(|(club, _)| club))
        .chain(clubs.aggregate_extra.iter().map(|(club, _)| club))
        .chain(clubs.research.iter())
    {
        if out.iter().any(|(label, ..)| label == club.label()) {
            continue;
        }
        let Some(u) = club.token_usage() else {
            continue;
        };
        out.push((club.label().to_string(), u.total_input, u.total_output));
    }
    out
}

/// Per-club (input, output) token spend between two snapshots, zero rows dropped.
pub(crate) fn usage_delta(
    before: &[(String, u64, u64)],
    after: &[(String, u64, u64)],
) -> Vec<(String, u64, u64)> {
    after
        .iter()
        .map(|(label, a_in, a_out)| {
            let (b_in, b_out) = before
                .iter()
                .find(|(b, ..)| b == label)
                .map(|(_, i, o)| (*i, *o))
                .unwrap_or((0, 0));
            (
                label.clone(),
                a_in.saturating_sub(b_in),
                a_out.saturating_sub(b_out),
            )
        })
        .filter(|(_, i, o)| i + o > 0)
        .collect()
}

/// Serialize one finished turn. Pure — the write happens in [`record_turn`].
pub(crate) fn turn_record(
    club: &str,
    trace: &TurnTrace,
    ms: u128,
    ok: bool,
    tokens: &[(String, u64, u64)],
) -> serde_json::Value {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut spend = serde_json::Map::new();
    for (label, t_in, t_out) in tokens {
        spend.insert(
            label.clone(),
            serde_json::json!({ "in": t_in, "out": t_out }),
        );
    }
    let effective = trace.effective.map(|(layers, judge, verify, samples)| {
        serde_json::json!({
            "layers": layers, "judge": judge, "verify": verify, "samples": samples,
        })
    });
    let mut record = serde_json::json!({
        "ts": ts,
        "club": club,
        "formation": trace.formation,
        "route": if trace.route.is_empty() { "direct" } else { trace.route },
        "ok": ok,
        "ms": ms,
        "dissent": trace.dissent,
        "gate": trace.gate,
        "drafts": { "proposed": trace.proposed, "kept": trace.kept },
        "knobs": effective,
        "waves": trace.waves.iter().map(|w| serde_json::json!({
            "launched": w.launched,
            "returned": w.returned,
            "novel": w.novel,
            "dissent": w.dissent,
            "decision": w.decision,
            "ms": w.ms,
        })).collect::<Vec<_>>(),
        "tokens": spend,
    });
    if let (Some(obj), Some(scale)) = (record.as_object_mut(), trace.scale.as_ref()) {
        obj.insert(
            "scale".to_string(),
            serde_json::json!({
                "clusters": scale.clusters,
                "dominant": scale.dominant,
                "eclusters": scale.eclusters,
                "judge_rounds": scale.judge_rounds,
                "reduce_rounds": scale.reduce_rounds,
                "gate_src": scale.gate_src,
            }),
        );
    }
    record
}

/// Append one turn to the ledger. Best-effort and test-silent: mock-club test
/// runs must never touch the user's real `~/.angel`.
pub(crate) fn record_turn(
    club: &str,
    trace: &TurnTrace,
    ms: u128,
    ok: bool,
    tokens: &[(String, u64, u64)],
) {
    if cfg!(test) || !ledger_enabled() {
        return;
    }
    let Some(repo) = crate::experience::current_turn_repo_value() else {
        return;
    };
    let mut record = turn_record(club, trace, ms, ok, tokens);
    record["repo"] = repo;
    if let Some(budget) = crate::harness::formation_budget::snapshot() {
        record["formation_budget"] = budget;
    }
    crate::experience::append_jsonl(&ledger_path(), &record);
}

// ---------------------------------------------------------------------------
// The /moa report
// ---------------------------------------------------------------------------

/// The `/moa` command body: a summary of the ledger (or `clear` to reset it).
fn record_matches_workspace(record: &serde_json::Value, workspace: &std::path::Path) -> bool {
    let Some(root) = record["repo"]["root"].as_str() else {
        return false;
    };
    let Some(key) = record["repo"]["key"].as_str() else {
        return false;
    };
    crate::workspace_store::matches_project(workspace, std::path::Path::new(root), key)
}

pub(crate) fn report_text(arg: Option<&str>, workspace: &std::path::Path) -> String {
    let path = ledger_path();
    if matches!(arg.map(str::trim), Some("clear")) {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return format!(
                "moa ledger already empty for this project · {}",
                path.display()
            );
        };
        let retained = raw
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|record| !record_matches_workspace(record, workspace))
            .collect::<Vec<_>>();
        return if crate::experience::replace_jsonl(&path, &retained).is_ok() {
            format!("moa ledger cleared for this project · {}", path.display())
        } else {
            "/moa clear: could not rewrite the project-filtered ledger".to_string()
        };
    }
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return format!(
            "no moa ledger yet · will appear at {} after the first sota-moa/swarm turn",
            path.display()
        );
    };
    let records: Vec<serde_json::Value> = raw
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .filter(|record| record_matches_workspace(record, workspace))
        .collect();
    if records.is_empty() {
        return format!("moa ledger is empty · {}", path.display());
    }
    format!("{}\n({})", summarize(&records), path.display())
}

/// Latest-turn + cumulative per-model token report for the live agent panel.
///
/// The draw loop calls this every frame while the agent bay shows metrics, and
/// the ledger is append-only for the life of the install — an uncached full
/// read + per-line parse would become a growing render-thread stall. The parsed
/// report is keyed on (mtime, len) so the file is only re-read when a turn
/// actually appended to it.
pub(crate) fn token_report(workspace: &std::path::Path) -> Option<MoaTokenReport> {
    type CacheKey = (std::time::SystemTime, u64, std::path::PathBuf);
    static CACHE: std::sync::Mutex<Option<(CacheKey, Option<MoaTokenReport>)>> =
        std::sync::Mutex::new(None);
    let path = ledger_path();
    let meta = std::fs::metadata(&path).ok()?;
    let mtime = meta.modified().ok()?;
    let len = meta.len();
    if let Ok(guard) = CACHE.lock()
        && let Some(((cached_mtime, cached_len, cached_workspace), report)) = guard.as_ref()
        && *cached_mtime == mtime
        && *cached_len == len
        && cached_workspace == workspace
    {
        return report.clone();
    }
    let raw = std::fs::read_to_string(&path).ok()?;
    let records: Vec<serde_json::Value> = raw
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .filter(|record| record_matches_workspace(record, workspace))
        .collect();
    let report = token_report_from_records(&records);
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(((mtime, len, workspace.to_path_buf()), report.clone()));
    }
    report
}

/// Build a compact per-model report from ledger records. Pure (testable).
pub(crate) fn token_report_from_records(records: &[serde_json::Value]) -> Option<MoaTokenReport> {
    let latest = records
        .iter()
        .rev()
        .find(|r| r.get("tokens").and_then(|t| t.as_object()).is_some())?;
    let latest_tokens = latest.get("tokens").and_then(|t| t.as_object())?;

    let mut rows: Vec<MoaModelTokenRow> = latest_tokens
        .iter()
        .map(|(label, v)| MoaModelTokenRow {
            label: label.clone(),
            last_input: v.get("in").and_then(|x| x.as_u64()).unwrap_or(0),
            last_output: v.get("out").and_then(|x| x.as_u64()).unwrap_or(0),
            total_input: 0,
            total_output: 0,
        })
        .collect();

    for r in records {
        let Some(tokens) = r.get("tokens").and_then(|t| t.as_object()) else {
            continue;
        };
        for (label, v) in tokens {
            let t_in = v.get("in").and_then(|x| x.as_u64()).unwrap_or(0);
            let t_out = v.get("out").and_then(|x| x.as_u64()).unwrap_or(0);
            match rows.iter_mut().find(|row| row.label == *label) {
                Some(row) => {
                    row.total_input += t_in;
                    row.total_output += t_out;
                }
                None => rows.push(MoaModelTokenRow {
                    label: label.clone(),
                    last_input: 0,
                    last_output: 0,
                    total_input: t_in,
                    total_output: t_out,
                }),
            }
        }
    }

    rows.retain(|row| row.last_total() > 0 || row.session_total() > 0);
    rows.sort_by(|a, b| {
        b.last_total()
            .cmp(&a.last_total())
            .then_with(|| b.session_total().cmp(&a.session_total()))
            .then_with(|| a.label.cmp(&b.label))
    });

    Some(MoaTokenReport {
        turns: records.len(),
        latest_route: latest
            .get("route")
            .and_then(|x| x.as_str())
            .unwrap_or("?")
            .to_string(),
        latest_ok: latest.get("ok").and_then(|x| x.as_bool()).unwrap_or(false),
        latest_ms: latest.get("ms").and_then(|x| x.as_u64()).unwrap_or(0),
        rows,
    })
}

/// Aggregate ledger records into the report body. Pure (testable).
pub(crate) fn summarize(records: &[serde_json::Value]) -> String {
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    let count_by = |key: &str| -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for r in records {
            let Some(val) = s(r, key) else { continue };
            match counts.iter_mut().find(|(k, _)| *k == val) {
                Some((_, n)) => *n += 1,
                None => counts.push((val, 1)),
            }
        }
        counts
    };
    let fmt_counts = |counts: &[(String, usize)]| -> String {
        counts
            .iter()
            .map(|(k, n)| format!("{k} {n}"))
            .collect::<Vec<_>>()
            .join(" · ")
    };

    let ok = records
        .iter()
        .filter(|r| r.get("ok").and_then(|x| x.as_bool()).unwrap_or(false))
        .count();
    let dissents: Vec<f64> = records
        .iter()
        .filter_map(|r| r.get("dissent").and_then(|x| x.as_f64()))
        .collect();
    let mut out = format!(
        "moa ledger · {} turn(s) · ok {ok}/{}\nroutes: {}",
        records.len(),
        records.len(),
        fmt_counts(&count_by("route")),
    );
    let gates = count_by("gate");
    if !gates.is_empty() {
        out.push_str(&format!("\ngate: {}", fmt_counts(&gates)));
    }
    if !dissents.is_empty() {
        let mean = dissents.iter().sum::<f64>() / dissents.len() as f64;
        let (lo, hi) = dissents
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), d| (l.min(*d), h.max(*d)));
        out.push_str(&format!(
            "\ndissent: mean {mean:.2} · range {lo:.2}–{hi:.2} (gate thresholds: \
             relax ≤ {:.2}, escalate ≥ {:.2})",
            dissent_lo(),
            dissent_hi()
        ));
    }
    let wave_sets: Vec<&Vec<serde_json::Value>> = records
        .iter()
        .filter_map(|r| r.get("waves").and_then(|w| w.as_array()))
        .filter(|w| !w.is_empty())
        .collect();
    if !wave_sets.is_empty() {
        let mean_waves =
            wave_sets.iter().map(|w| w.len()).sum::<usize>() as f64 / wave_sets.len() as f64;
        let mut stops: Vec<(String, usize)> = Vec::new();
        let mut by_idx: Vec<(usize, f64, usize)> = Vec::new();
        for waves in &wave_sets {
            if let Some(decision) = waves
                .last()
                .and_then(|w| w.get("decision"))
                .and_then(|d| d.as_str())
                && let Some(reason) = decision.strip_prefix("stop:")
            {
                match stops.iter_mut().find(|(k, _)| k == reason) {
                    Some((_, n)) => *n += 1,
                    None => stops.push((reason.to_string(), 1)),
                }
            }
            for (idx, wave) in waves.iter().enumerate() {
                if by_idx.len() <= idx {
                    by_idx.push((0, 0.0, 0));
                }
                by_idx[idx].0 += wave.get("novel").and_then(|n| n.as_u64()).unwrap_or(0) as usize;
                if let Some(d) = wave.get("dissent").and_then(|d| d.as_f64()) {
                    by_idx[idx].1 += d;
                    by_idx[idx].2 += 1;
                }
            }
        }
        out.push_str(&format!("\nwaves: mean {mean_waves:.2}"));
        if !stops.is_empty() {
            stops.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            out.push_str(&format!(" · stops {}", fmt_counts(&stops)));
        }
        let wave_stats = by_idx
            .iter()
            .enumerate()
            .map(|(idx, (novel, dissent_sum, dissent_n))| {
                let d = if *dissent_n > 0 {
                    format!("{:.2}", dissent_sum / *dissent_n as f64)
                } else {
                    "n/a".to_string()
                };
                format!("w{} novel {} d {}", idx + 1, novel, d)
            })
            .collect::<Vec<_>>()
            .join(" · ");
        if !wave_stats.is_empty() {
            out.push_str(&format!("\nwave detail: {wave_stats}"));
        }
    }
    let scales: Vec<&serde_json::Map<String, serde_json::Value>> = records
        .iter()
        .filter_map(|r| r.get("scale").and_then(|s| s.as_object()))
        .collect();
    if !scales.is_empty() {
        let mean = |key: &str| -> f64 {
            let vals = scales
                .iter()
                .filter_map(|s| s.get(key).and_then(|v| v.as_f64()))
                .collect::<Vec<_>>();
            if vals.is_empty() {
                0.0
            } else {
                vals.iter().sum::<f64>() / vals.len() as f64
            }
        };
        out.push_str(&format!(
            "\nscale: mean clusters {:.2} · dominant {:.2} · eclusters {:.2}",
            mean("clusters"),
            mean("dominant"),
            mean("eclusters")
        ));
    }
    // Token spend per club, summed across every recorded turn.
    let mut spend: Vec<(String, u64, u64)> = Vec::new();
    for r in records {
        let Some(tokens) = r.get("tokens").and_then(|t| t.as_object()) else {
            continue;
        };
        for (label, v) in tokens {
            let t_in = v.get("in").and_then(|x| x.as_u64()).unwrap_or(0);
            let t_out = v.get("out").and_then(|x| x.as_u64()).unwrap_or(0);
            match spend.iter_mut().find(|(k, ..)| k == label) {
                Some((_, i, o)) => {
                    *i += t_in;
                    *o += t_out;
                }
                None => spend.push((label.clone(), t_in, t_out)),
            }
        }
    }
    if !spend.is_empty() {
        spend.sort_by_key(|entry| std::cmp::Reverse(entry.1 + entry.2));
        out.push_str("\ntokens in/out by club:");
        for (label, t_in, t_out) in &spend {
            out.push_str(&format!(
                "\n  {label:<12} {} / {}",
                kfmt(*t_in),
                kfmt(*t_out)
            ));
        }
    }
    let recent: Vec<String> = records
        .iter()
        .rev()
        .take(5)
        .map(|r| {
            let d = r
                .get("dissent")
                .and_then(|x| x.as_f64())
                .map(|d| format!(" d={d:.2}"))
                .unwrap_or_default();
            let gate = s(r, "gate").map(|g| format!(" {g}")).unwrap_or_default();
            let ms = r.get("ms").and_then(|x| x.as_u64()).unwrap_or(0);
            format!(
                "  {}{gate}{d} · {:.1}s · {}",
                s(r, "route").unwrap_or_else(|| "?".into()),
                ms as f64 / 1000.0,
                if r.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
                    "ok"
                } else {
                    "err"
                }
            )
        })
        .collect();
    out.push_str("\nrecent (newest first):\n");
    out.push_str(&recent.join("\n"));
    out.push_str("\n— /moa clear resets");
    out
}

/// `12345` → `12.3k`; keeps small counts exact.
fn kfmt(n: u64) -> String {
    if n >= 10_000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod project_scope_tests {
    use super::*;

    #[test]
    fn legacy_and_foreign_rows_are_inert_for_project_reports() {
        let alpha = std::path::Path::new("/tmp/moa-alpha/repo");
        let beta = std::path::Path::new("/tmp/moa-beta/repo");
        let identity = crate::workspace_store::repo_identity(alpha);
        let bound = serde_json::json!({
            "repo": {"root": identity.root, "key": identity.key},
            "tokens": {},
        });
        assert!(record_matches_workspace(&bound, alpha));
        assert!(!record_matches_workspace(&bound, beta));
        assert!(!record_matches_workspace(
            &serde_json::json!({"tokens": {}}),
            alpha
        ));
    }
}

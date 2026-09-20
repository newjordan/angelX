//! Active harness lane — product policy that shapes root LID behavior.
//!
//! Treebeard is the default RLM/HiQ lane: strategy-only root preamble, handle
//! disclosure, aggressive offload floors, eager bulk veto, and bounded
//! recursive subcall depth. `ANGEL_LANE=default` remains the explicit classic
//! ReAct ablation.

use super::*;
use std::cell::Cell;

/// Product lane for the agent harness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Lane {
    /// Explicit classic coding-agent ReAct ablation.
    Default,
    /// Zhang/Khattab-style RLM: strategy-only root, bulk under handles.
    #[default]
    Treebeard,
}

impl Lane {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Treebeard => "treebeard",
        }
    }

    pub(crate) fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "treebeard" | "rlm" | "hiq" | "hi/q" => Self::Treebeard,
            _ => Self::Default,
        }
    }
}

/// Active lane from `ANGEL_LANE` (unset → Treebeard/HiQ).
///
/// The historical ReAct path remains available as the explicit
/// `ANGEL_LANE=default` off-control. Unknown explicit values fail conservative
/// through [`Lane::parse`] to that same control rather than silently opting in.
///
/// Product reads once: Treebeard header paint asks this every frame.
/// Tests keep the live getenv so `EnvGuard` overrides stay visible.
pub(crate) fn active_lane() -> Lane {
    #[cfg(not(test))]
    {
        static LANE: std::sync::OnceLock<Lane> = std::sync::OnceLock::new();
        *LANE.get_or_init(lane_from_env)
    }
    #[cfg(test)]
    lane_from_env()
}

fn lane_from_env() -> Lane {
    std::env::var("ANGEL_LANE")
        .map(|v| Lane::parse(&v))
        .unwrap_or(Lane::Treebeard)
}

#[inline]
pub(crate) fn is_treebeard() -> bool {
    active_lane() == Lane::Treebeard
}

/// Treebeard decomposition / LID contract appended to the system preamble when
/// the lane is active. Kept short so it stays in the stable cache prefix.
pub(crate) fn treebeard_system_block() -> &'static str {
    "\n[treebeard lane — RLM / Hi/Q]\n\
     You are running under the Treebeard harness lane. Generalization is your \
     job as a *program*, not only the model's: keep every root observation \
     locally in-distribution.\n\
     - **Decompose first.** State a short plan (map/filter/reduce, search→edit→verify, \
     or fan-out→synthesize) before bulk inspection. Longer tasks mean more subcalls, \
     not a fatter root transcript.\n\
     - **Strategy in root; bulk under handles.** Prefer handle receipts over pasting \
     tool bodies. Large inspection results may already be handle receipts — treat them \
     as addressable evidence. Use `code_mode` for programmatic batching (`handle_put` / \
     `handle_get` keep intermediates out of the return value), `spawn`/`delegate` for \
     nested seats (bounded depth), and `handle_read` only for a capped slice when a \
     receipt is insufficient.\n\
     - **Do not re-hydrate OOD context.** If intermediate text is large, leave it \
     offloaded and continue from the receipt. Train-friendly trajectories look the \
     same at the root for short and long instances of the same strategy.\n"
}

/// Optional system-prompt suffix for the active lane (empty for default).
pub(crate) fn lane_system_suffix() -> &'static str {
    if is_treebeard() {
        treebeard_system_block()
    } else {
        ""
    }
}

/// Path to living B200 peer state (`POPCORN_PEER_STATE` or `~/.angel0/popcorn-peer.json`).
pub(crate) fn popcorn_peer_state_path() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("POPCORN_PEER_STATE") {
        let p = PathBuf::from(raw);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join(".angel0").join("popcorn-peer.json"))
}

/// Parsed JSON keyed on path+mtime+len. Treebeard header paint re-queries living
/// peer and forge snaps every frame; stat and reuse instead of reopening.
/// Hits return `Arc` so the draw path does not deep-clone the document.
fn load_json_cached(path: &Path) -> Option<std::sync::Arc<serde_json::Value>> {
    type CacheKey = (PathBuf, std::time::SystemTime, u64);
    static CACHE: std::sync::Mutex<Vec<(CacheKey, std::sync::Arc<serde_json::Value>)>> =
        std::sync::Mutex::new(Vec::new());

    let meta = std::fs::metadata(path).ok()?;
    let key: Option<CacheKey> = meta
        .modified()
        .ok()
        .map(|mtime| (path.to_path_buf(), mtime, meta.len()));
    if let Some(ref key) = key
        && let Ok(guard) = CACHE.lock()
        && let Some((_, value)) = guard.iter().find(|(cached, _)| cached == key)
    {
        return Some(std::sync::Arc::clone(value));
    }
    let text = std::fs::read_to_string(path).ok()?;
    let value = std::sync::Arc::new(serde_json::from_str(&text).ok()?);
    if let Some(key) = key
        && let Ok(mut guard) = CACHE.lock()
    {
        if let Some(slot) = guard
            .iter_mut()
            .find(|(cached, _)| cached.0.as_path() == path)
        {
            *slot = (key, std::sync::Arc::clone(&value));
        } else {
            guard.push((key, std::sync::Arc::clone(&value)));
        }
    }
    Some(value)
}

fn load_peer_json() -> Option<std::sync::Arc<serde_json::Value>> {
    load_json_cached(&popcorn_peer_state_path()?)
}

type PeerFileKey = (PathBuf, std::time::SystemTime, u64);

fn file_ident(path: &Path) -> Option<PeerFileKey> {
    let meta = std::fs::metadata(path).ok()?;
    Some((path.to_path_buf(), meta.modified().ok()?, meta.len()))
}

fn peer_file_key() -> Option<PeerFileKey> {
    file_ident(&popcorn_peer_state_path()?)
}

#[derive(Clone, Debug)]
struct HeaderPeerPack {
    snapshot: Option<(f64, String, Option<String>)>,
    p1_us: Option<f64>,
    top_open: Option<(String, f64)>,
}

fn header_peer_pack() -> HeaderPeerPack {
    static CACHE: std::sync::Mutex<Option<(PeerFileKey, HeaderPeerPack)>> =
        std::sync::Mutex::new(None);
    let Some(key) = peer_file_key() else {
        return HeaderPeerPack {
            snapshot: None,
            p1_us: None,
            top_open: None,
        };
    };
    if let Ok(guard) = CACHE.lock()
        && let Some((cached, pack)) = guard.as_ref()
        && cached == &key
    {
        return pack.clone();
    }
    let pack = match load_json_cached(&key.0) {
        Some(v) => HeaderPeerPack {
            snapshot: snapshot_from_peer_json(&v),
            p1_us: p1_from_peer_json(&v),
            top_open: load_living_peer_open_levers(1)
                .into_iter()
                .next()
                .map(|lever| (lever.key, lever.board_us)),
        },
        None => HeaderPeerPack {
            snapshot: None,
            p1_us: None,
            top_open: None,
        },
    };
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((key, pack.clone()));
    }
    pack
}

fn snapshot_from_peer_json(v: &serde_json::Value) -> Option<(f64, String, Option<String>)> {
    let geo = v.get("geomean_us")?.as_f64()?;
    if !(geo.is_finite() && geo > 0.0) {
        return None;
    }
    let name = v
        .get("name")
        .and_then(|x| x.as_str())
        .or_else(|| {
            v.get("path").and_then(|x| x.as_str()).map(|p| {
                Path::new(p)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(p)
            })
        })
        .unwrap_or("peer")
        .to_string();
    let path_s = v
        .get("path")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    Some((geo, name, path_s))
}

fn p1_from_peer_json(v: &serde_json::Value) -> Option<f64> {
    if let Some(us) = v.get("p1_us").and_then(|x| x.as_f64())
        && us.is_finite()
        && us > 0.0
    {
        return Some(us);
    }
    let shapes = v.get("shapes")?.as_object()?;
    for key in ["512x640", "512X640", "512·640"] {
        if let Some(us) = shapes
            .get(key)
            .and_then(|x| x.as_f64().or_else(|| x.get("us").and_then(|u| u.as_f64())))
            && us.is_finite()
            && us > 0.0
        {
            return Some(us);
        }
    }
    None
}

/// Read living peer geomean / name from popcorn-promote-peer state (best-effort).
pub(crate) fn load_living_peer_snapshot() -> Option<(f64, String, Option<String>)> {
    header_peer_pack().snapshot
}

/// Living-peer baseline µs for one shape key (`32768x1`, …).
///
/// Prefer `shape_bests` HOLD floor (what PRIMARY attacks must beat), else the
/// board peer `shapes` entry. Used by [`crate::reinforce::PopcornPeerReward`]
/// so GpuComp coding seats score against the measured attack floor, not only
/// the board geomean.
pub(crate) fn load_living_peer_shape_baseline(shape_key: &str) -> Option<f64> {
    let v = load_peer_json()?;
    let key = shape_key.trim();
    if key.is_empty() {
        return None;
    }
    // Normalize mid-dot / case so 512·640 and 512X640 match board keys.
    let norm = key.to_ascii_lowercase().replace('·', "x");
    let candidates = [key.to_string(), norm.clone(), norm.replace('x', "·")];
    if let Some(bests) = v.get("shape_bests").and_then(|x| x.as_object()) {
        for c in &candidates {
            if let Some(us) = bests.get(c).and_then(shape_us_from_value) {
                return Some(us);
            }
            // Case-insensitive scan when exact key missing.
            for (k, bv) in bests {
                if k.eq_ignore_ascii_case(c)
                    && let Some(us) = shape_us_from_value(bv)
                {
                    return Some(us);
                }
            }
        }
    }
    if let Some(shapes) = v.get("shapes").and_then(|x| x.as_object()) {
        for c in &candidates {
            if let Some(us) = shapes.get(c).and_then(shape_us_from_value) {
                return Some(us);
            }
            for (k, bv) in shapes {
                if k.eq_ignore_ascii_case(c)
                    && let Some(us) = shape_us_from_value(bv)
                {
                    return Some(us);
                }
            }
        }
    }
    None
}

/// P1 (512·b640) living-peer shape µs from promote-peer state (best-effort).
///
/// Prefer stamped `p1_us`, else `shapes["512x640"]`. Used so Treebeard root
/// names the real open lever instead of a hardcoded 1685 folklore constant.
pub(crate) fn load_living_peer_p1_us() -> Option<f64> {
    header_peer_pack().p1_us
}

/// Rank-1 open lever (board µs) for the Treebeard header strip.
pub(crate) fn load_living_peer_top_open() -> Option<(String, f64)> {
    header_peer_pack().top_open
}

/// One shape where a measured log beat the living *board* peer shape µs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LivingShapeHold {
    pub key: String,
    pub best_us: f64,
    pub board_us: f64,
    /// Percent faster than board peer (positive = hold win).
    pub pct: f64,
}

/// Board-peer shape ranked by absolute µs (equal-weight geomean levers).
///
/// Holds are *wins* vs board; open levers are the slowest board shapes that
/// still dominate geomean — P1 / huge singles — even when a tiny hold exists.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LivingOpenLever {
    pub key: String,
    pub board_us: f64,
    pub best_us: Option<f64>,
    /// Percent best beats board when a shape_best exists; else 0.
    pub hold_pct: f64,
    /// Equal-weight geomean drop (%) if this shape were 2× faster: (1 − 0.5^(1/n))·100.
    pub geo_drop_if_half_pct: f64,
    /// Optional shape_bests source name (e.g. b200_r7_…).
    pub best_name: Option<String>,
}

fn shape_us_from_value(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.get("us").and_then(|u| u.as_f64()))
        .filter(|u| u.is_finite() && *u > 0.0)
}

/// Shape_bests that beat board peer shapes (popcorn-promote-peer LID holds).
///
/// Sorted by ratio win descending. Caps at `max` so Treebeard root stays short.
pub(crate) fn load_living_peer_shape_holds(max: usize) -> Vec<LivingShapeHold> {
    let Some(v) = load_peer_json() else {
        return Vec::new();
    };
    let Some(board) = v.get("shapes").and_then(|x| x.as_object()) else {
        return Vec::new();
    };
    let Some(bests) = v.get("shape_bests").and_then(|x| x.as_object()) else {
        return Vec::new();
    };
    let mut holds: Vec<LivingShapeHold> = Vec::new();
    for (key, best_v) in bests {
        let Some(best_us) = shape_us_from_value(best_v) else {
            continue;
        };
        let Some(board_us) = board.get(key).and_then(shape_us_from_value) else {
            continue;
        };
        if best_us + 1e-9 >= board_us {
            continue;
        }
        let pct = (board_us - best_us) / board_us * 100.0;
        if pct < 0.4 {
            continue; // noise floor — matches forge-hiq-ingest seed_shape_holds
        }
        holds.push(LivingShapeHold {
            key: key.clone(),
            best_us,
            board_us,
            pct,
        });
    }
    holds.sort_by(|a, b| {
        b.pct
            .partial_cmp(&a.pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if max > 0 && holds.len() > max {
        holds.truncate(max);
    }
    holds
}

/// Top open levers: board shapes sorted by absolute µs descending.
///
/// Geomean weights each of 15 shapes equally, so a 38ms huge single and a
/// 1.7ms P1 both matter — rank by board µs, not hold gap. Caps at `max`.
pub(crate) fn load_living_peer_open_levers(max: usize) -> Vec<LivingOpenLever> {
    let Some(v) = load_peer_json() else {
        return Vec::new();
    };
    let Some(board) = v.get("shapes").and_then(|x| x.as_object()) else {
        return Vec::new();
    };
    let bests = v
        .get("shape_bests")
        .and_then(|x| x.as_object())
        .cloned()
        .unwrap_or_default();
    let n_shapes = board
        .iter()
        .filter(|(_, bv)| shape_us_from_value(bv).is_some())
        .count()
        .max(1);
    // Equal-weight: half any one shape multiplies geomean by 0.5^(1/n).
    let geo_drop_half = (1.0 - 0.5_f64.powf(1.0 / n_shapes as f64)) * 100.0;
    let mut levers: Vec<LivingOpenLever> = Vec::new();
    for (key, board_v) in board {
        let Some(board_us) = shape_us_from_value(board_v) else {
            continue;
        };
        let best_entry = bests.get(key);
        let best_us = best_entry.and_then(shape_us_from_value);
        let best_name = best_entry.and_then(|bv| {
            bv.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
        });
        let hold_pct = match best_us {
            Some(b) if b + 1e-9 < board_us => (board_us - b) / board_us * 100.0,
            _ => 0.0,
        };
        levers.push(LivingOpenLever {
            key: key.clone(),
            board_us,
            best_us,
            hold_pct,
            geo_drop_if_half_pct: geo_drop_half,
            best_name,
        });
    }
    levers.sort_by(|a, b| {
        b.board_us
            .partial_cmp(&a.board_us)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if max > 0 && levers.len() > max {
        levers.truncate(max);
    }
    levers
}

/// Dynamic living-frontier line for Treebeard/GpuComp (Zhang/Khattab LID root).
///
/// Injects the current popcorn peer geomean so the root strategy names the
/// frontier number without stuffing bulk logs. Empty when peer state missing
/// or lane is not Treebeard.
pub(crate) fn living_competition_system_suffix() -> String {
    if !is_treebeard() || !env_flag("ANGEL_GPU_COMP_LOCAL_MOA", false) {
        return String::new();
    }
    let Some((geo, name, path)) = load_living_peer_snapshot() else {
        return String::new();
    };
    let p1 = load_living_peer_p1_us().unwrap_or(1685.0);
    let mut out = format!(
        "\n[living B200 peer — Hi/Q frontier]\n\
         Peer geomean **{geo:.2} µs** (`{name}`). Beat this with `popcorn-submit-hiq` \
         before rank; peer-relative rewards only mint goldens when score_us < peer. \
         P1 open lever remains 512·b640 (~{p1:.0} µs peer shape, need ~3–5× for top-10). \
         multi128 wins dense n=128; do not regress lean stack invariants.\n"
    );
    let levers = load_living_peer_open_levers(4);
    if let Some(top) = levers.first() {
        // Primary attack: highest board µs (equal geomean weight) — usually huge 32k.
        let half = top.geo_drop_if_half_pct;
        let best_s = match (top.best_us, top.best_name.as_deref()) {
            (Some(b), Some(n)) => format!(" best {:.0}µs (`{n}`)", b),
            (Some(b), None) => format!(" best {:.0}µs", b),
            _ => String::new(),
        };
        out.push_str(&format!(
            " **Primary attack:** `{}` board ~{:.0}µs{best_s} — half-cut ≈ **{half:.2}%** peer geomean \
             (equal weight). Prefer measure+promote over folklore; keep P1 closed avenues closed.\n",
            top.key, top.board_us
        ));
        // Explicit NEW_HOLD densify floor so root LID matches free-train / strip H38k.
        if let Some(b) = top.best_us
            && b.is_finite()
            && b > 0.0
            && b + 1e-9 < top.board_us
        {
            let src = top.best_name.as_deref().unwrap_or("shape_best");
            out.push_str(&format!(
                " **PRIMARY HOLD floor:** `{key}` @ **{b:.0}µs** (`{src}`) — densify \
                     Hi/Q only on NEW_HOLD when cand µs < {b:.0}; do not re-mine flat board \
                     folklore at {board:.0}µs.\n",
                key = top.key,
                board = top.board_us,
            ));
        }
    }
    if !levers.is_empty() {
        out.push_str(" Open levers (highest board µs — equal geomean weight):\n");
        for l in &levers {
            let half = l.geo_drop_if_half_pct;
            match l.best_us {
                Some(b) if l.hold_pct >= 0.4 => {
                    out.push_str(&format!(
                        "  - {} board {:.0}µs best {:.0}µs (−{:.1}% hold, ½geo↓ {:.2}%)\n",
                        l.key, l.board_us, b, l.hold_pct, half
                    ));
                }
                Some(b) => {
                    out.push_str(&format!(
                        "  - {} board {:.0}µs best {:.0}µs (flat, ½geo↓ {:.2}%)\n",
                        l.key, l.board_us, b, half
                    ));
                }
                None => {
                    out.push_str(&format!(
                        "  - {} board {:.0}µs (no shape_best yet, ½geo↓ {:.2}%)\n",
                        l.key, l.board_us, half
                    ));
                }
            }
        }
    }
    let holds = load_living_peer_shape_holds(3);
    if !holds.is_empty() {
        out.push_str(" Shape holds (beat board peer — keep dispatch):\n");
        for h in &holds {
            out.push_str(&format!(
                "  - {} best {:.1}µs vs board {:.1}µs (−{:.1}%)\n",
                h.key, h.best_us, h.board_us, h.pct
            ));
        }
    }
    if let Some(p) = path
        && std::env::var_os("POPCORN_PEER_LOG").is_none()
    {
        // Soft hint only — formation may also pin POPCORN_PEER_LOG.
        out.push_str(&format!(" Peer log path (for measure tools): `{p}`.\n"));
    }
    out
}

/// Free-train / last LoRA cycle line for Treebeard root (Zhang/Khattab LID).
///
/// Names adapter version + gate without bulk forge logs so morning turns share
/// an isomorphic strategy root. Empty when no completed cycle handoff.
pub(crate) fn free_train_system_suffix() -> String {
    if !is_treebeard() {
        return String::new();
    }
    let Some(snap) = load_forge_train_snap() else {
        return String::new();
    };
    if snap.state == "training" || snap.state == "reclaiming" || snap.state == "densifying" {
        if let (Some(step), Some(total)) = (snap.train_step, snap.train_total) {
            let mut s = format!(
                "\n[free-train — in flight]\n\
                 LoRA cycle running **{step}/{total}**"
            );
            if let Some(eta) = snap.train_eta_sec
                && eta > 0
            {
                s.push_str(&format!(" (~{}m left)", eta / 60));
            }
            if let Some(loss) = snap.train_loss
                && loss.is_finite()
                && loss > 0.0
            {
                s.push_str(&format!(" loss={loss:.3}"));
            }
            match (snap.train_loss_min, snap.train_loss_max) {
                (Some(lo), Some(hi))
                    if lo.is_finite() && hi.is_finite() && lo > 0.0 && hi >= lo =>
                {
                    s.push_str(&format!(" Lrange={lo:.3}–{hi:.3}"));
                }
                _ => {}
            }
            if let Some(prior) = snap.prior_train_loss
                && prior.is_finite()
                && prior > 0.0
            {
                s.push_str(&format!(" Lprior={prior:.3}"));
            }
            if let Some(imp) = snap.train_loss_improvement
                && imp.is_finite()
            {
                s.push_str(&format!(" LΔ={:+.0}%", imp * 100.0));
            }
            // VRAM trail for OOM-aware strategy root (pulse free_mib collapse).
            if let Some(ref w) = snap.vram_warn {
                s.push_str(&format!(" VRAM!={w}"));
            } else if let Some(fmin) = snap.free_mib_min {
                if fmin.is_finite() && fmin >= 0.0 {
                    s.push_str(&format!(" freemin={fmin:.0}MiB"));
                }
            } else if let Some(free) = snap.gpu_free_mib
                && free.is_finite()
                && free >= 0.0
            {
                s.push_str(&format!(" free={free:.0}MiB"));
            }
            // Pulse stamps predicted harvest version (max remote vN+1) mid-train.
            if let Some(ref ver) = snap.version
                && ver.starts_with('v')
            {
                s.push_str(&format!(" adapter→{ver}"));
            }
            // Pulse stamps next_PRIMARY mid-train so root LID still names attack surface.
            if let Some(ref open) = snap.open_lever_top {
                s.push_str(&format!(" next_PRIMARY={open}"));
            } else if let Some(top) = load_living_peer_open_levers(1).into_iter().next() {
                s.push_str(&format!(" next_PRIMARY={}", top.key));
            }
            if let Some(n) = snap.free_train_primary_n
                && n > 0
            {
                s.push_str(&format!(" prior_free_train_PRIMARY_rows={n}"));
            }
            if let Some(h) = snap.measured_hold_us
                && h.is_finite()
                && h > 0.0
            {
                s.push_str(&format!(" PRIMARY_HOLD_floor={h:.0}us"));
            }
            if let Some(p) = snap.preference_n
                && p > 0
            {
                s.push_str(&format!(" preference_n={p}"));
            }
            if let Some(c) = snap.coding_eval_n
                && c > 0
            {
                s.push_str(&format!(" coding_eval_n={c}"));
                if let Some(cp) = snap.coding_eval_primary_n
                    && cp > 0
                {
                    s.push_str(&format!(" coding_eval_primary_n={cp}"));
                }
            }
            s.push_str(
                ". Do not fight B70 GPU; prefer densify/ingest/popcorn offline work \
                 (attack next_PRIMARY offline; finalize harvests PRIMARY goldens when done).\n",
            );
            return s;
        }
        return "\n[free-train — in flight]\n\
                Forge free-train is reclaiming/densifying — leave turbo GPU alone.\n"
            .into();
    }
    if snap.state != "done" {
        return String::new();
    }
    let ver = snap.version.as_deref().unwrap_or("?");
    let gate = match snap.gate_pass {
        Some(true) => "pass",
        Some(false) => "fail",
        None => "?",
    };
    let mut s = format!(
        "\n[free-train — last cycle]\n\
         Adapter **{ver}** gate={gate}"
    );
    if snap.promoted == Some(true) {
        s.push_str(" promoted");
    } else if snap.adapter_local {
        s.push_str(" local-disk (AUTOPROMOTE=0 — disk is the artifact)");
    }
    if let Some(loss) = snap.train_loss
        && loss.is_finite()
        && loss > 0.0
    {
        s.push_str(&format!(" train_loss={loss:.3}"));
    }
    // Harvest extrema (pulse/finalize) — morning LID shares isomorphic Lrange.
    match (snap.train_loss_min, snap.train_loss_max) {
        (Some(lo), Some(hi)) if lo.is_finite() && hi.is_finite() && lo > 0.0 && hi >= lo => {
            s.push_str(&format!(" Lrange={lo:.3}–{hi:.3}"));
        }
        _ => {}
    }
    if let Some(prior) = snap.prior_train_loss
        && prior.is_finite()
        && prior > 0.0
    {
        s.push_str(&format!(" Lprior={prior:.3}"));
    }
    if let Some(imp) = snap.train_loss_improvement
        && imp.is_finite()
    {
        s.push_str(&format!(" LΔ={:+.0}%", imp * 100.0));
    }
    if let Some(ref open) = snap.open_lever_top {
        s.push_str(&format!(" next_PRIMARY={open}"));
    } else if let Some(top) = load_living_peer_open_levers(1).into_iter().next() {
        s.push_str(&format!(" next_PRIMARY={}", top.key));
    }
    if let Some(n) = snap.free_train_primary_n
        && n > 0
    {
        s.push_str(&format!(" free_train_PRIMARY_rows={n}"));
    }
    if let Some(h) = snap.measured_hold_us
        && h.is_finite()
        && h > 0.0
    {
        s.push_str(&format!(
            " PRIMARY_HOLD_floor={h:.0}us (attack only if NEW_HOLD cand µs < {h:.0})"
        ));
    }
    if let Some(p) = snap.preference_n
        && p > 0
    {
        s.push_str(&format!(" preference_n={p}"));
    }
    if let Some(c) = snap.coding_eval_n
        && c > 0
    {
        s.push_str(&format!(" coding_eval_n={c}"));
        if let Some(cp) = snap.coding_eval_primary_n
            && cp > 0
        {
            s.push_str(&format!(" coding_eval_primary_n={cp}"));
        }
    }
    s.push_str(
        ". Morning: `python3 scripts/forge-adapter-ready.py`; \
         free_train Hi/Q goldens land in angel-forge/inbox/hiq-free-train.jsonl. \
         Prefer attack `next_PRIMARY` (highest board µs / equal geomean weight) over reopening P1 closed avenues.\n",
    );
    s
}

/// Whether the model-facing `handle_read` tool should register.
/// Treebeard enables it by default; `ANGEL_HANDLE_READ_TOOL=0` forces off.
pub(crate) fn handle_read_tool_enabled() -> bool {
    if !handle_store_enabled() {
        return false;
    }
    match std::env::var("ANGEL_HANDLE_READ_TOOL") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => is_treebeard(),
    }
}

/// Optional env `usize` — `None` when unset or unparsable.
fn env_usize_opt(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
}

/// Floor for offloading successful `code_mode` bodies from root history.
/// Explicit `ANGEL_HANDLE_CODE_MODE_MIN_BYTES` always wins; Treebeard defaults
/// lower (1024) so more intermediate bulk leaves the root.
pub(crate) fn code_mode_offload_min_bytes() -> usize {
    env_usize_opt("ANGEL_HANDLE_CODE_MODE_MIN_BYTES").unwrap_or(if is_treebeard() {
        1024
    } else {
        4096
    })
}

/// Floor for offloading spawn/delegate digests. Treebeard default 512.
pub(crate) fn lane_subcall_offload_min_bytes() -> usize {
    env_usize_opt("ANGEL_HANDLE_SUBCALL_MIN_BYTES").unwrap_or(if is_treebeard() {
        512
    } else {
        2048
    })
}

/// Floor for *eager* tool-result offload before the body enters root history.
///
/// Keep the implicit floor at one complete `handle_read` disclosure. Parking a
/// smaller result only makes the root spend another provider hop to recover the
/// same bytes, which is strictly worse for both latency and history size. An
/// explicit env value remains an experiment/operator override.
pub(crate) fn eager_tool_offload_min_bytes() -> usize {
    env_usize_opt("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES").unwrap_or(16 * 1024)
}

/// Maximum nested LM depth for spawn/delegate seats.
/// Depth 0 = root turn. Default: Treebeard allows two nested levels (root→seat→seat);
/// other lanes allow only root-level spawn (max depth 1 means seats cannot re-spawn).
pub(crate) fn treebeard_max_depth() -> usize {
    env_usize_opt("ANGEL_TREEBEARD_MAX_DEPTH").unwrap_or(if is_treebeard() { 2 } else { 1 })
}

thread_local! {
    static SUBCALL_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Current nested-LM depth (0 at the root driver turn).
pub(crate) fn subcall_depth() -> usize {
    SUBCALL_DEPTH.with(|c| c.get())
}

/// Whether a new spawn/delegate nesting level is allowed from the current depth.
pub(crate) fn spawn_nesting_allowed() -> bool {
    subcall_depth() < treebeard_max_depth()
}

/// RAII bump for seat/delegate threads so nested depth is restored on exit.
pub(crate) struct SubcallDepthGuard {
    prev: usize,
}

impl SubcallDepthGuard {
    pub(crate) fn enter() -> Self {
        let prev = SUBCALL_DEPTH.with(|c| {
            let cur = c.get();
            c.set(cur.saturating_add(1));
            cur
        });
        Self { prev }
    }
}

impl Drop for SubcallDepthGuard {
    fn drop(&mut self) {
        SUBCALL_DEPTH.with(|c| c.set(self.prev));
    }
}

/// Compact free-train / forge-when-free snapshot for the Treebeard strip.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ForgeTrainSnap {
    pub state: String,
    pub train_step: Option<u64>,
    pub train_total: Option<u64>,
    pub train_eta_sec: Option<u64>,
    /// Live scraped loss from forge.log (optional).
    pub train_loss: Option<f64>,
    /// Min/max live loss across this free-train job (pulse extrema).
    pub train_loss_min: Option<f64>,
    pub train_loss_max: Option<f64>,
    /// Prior free-train cycle loss baseline (Lprior) for RL densify.
    pub prior_train_loss: Option<f64>,
    /// Relative improvement vs prior (positive = better). LΔ.
    pub train_loss_improvement: Option<f64>,
    /// Live free VRAM MiB (pulse health scrape).
    pub gpu_free_mib: Option<f64>,
    /// Min free VRAM across this free-train job (OOM early-warning).
    pub free_mib_min: Option<f64>,
    /// `low` | `crit` when free collapses (see FORGE_VRAM_*_MIB).
    pub vram_warn: Option<String>,
    /// `train` | `post_train` | `adapter_eval` | `base_eval` | `save` | `gate` …
    pub train_phase: Option<String>,
    pub version: Option<String>,
    pub gate_pass: Option<bool>,
    pub promoted: Option<bool>,
    /// Local rsync harvest under `~/angel-forge/adapters/vN` (AUTOPROMOTE=0 path).
    pub adapter_local: bool,
    /// Top open lever key stamped on free-train / last-cycle (e.g. 32768x1).
    pub open_lever_top: Option<String>,
    /// Dual-lane free-train harvest rows that name living PRIMARY (usually 4).
    pub free_train_primary_n: Option<u64>,
    /// Measured PRIMARY HOLD floor µs (usually r7@38300) — NEW_HOLD densify only if cand < this.
    pub measured_hold_us: Option<f64>,
    /// Phase-4 RL preference pair mass in SFT (chosen/rejected µs lessons).
    pub preference_n: Option<u64>,
    /// GpuComp coding_eval faucet mass (popcorn_peer µs vs HOLD) in SFT.
    pub coding_eval_n: Option<u64>,
    /// Of those, PRIMARY 32k / primary_hold rows.
    pub coding_eval_primary_n: Option<u64>,
}

fn forge_when_free_status_path() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("FORGE_WHEN_FREE_STATUS") {
        let p = PathBuf::from(raw);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(
        Path::new(&home)
            .join(".angel0")
            .join("forge-when-free-status.json"),
    )
}

fn forge_last_cycle_path() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("FORGE_LAST_CYCLE") {
        let p = PathBuf::from(raw);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(
        Path::new(&home)
            .join(".angel0")
            .join("forge-last-cycle.json"),
    )
}

/// Prefer live free-train status; fall back to last completed cycle handoff.
pub(crate) fn load_forge_train_snap() -> Option<ForgeTrainSnap> {
    type ForgeTrainCacheEntry = (
        Option<PeerFileKey>,
        Option<PeerFileKey>,
        Option<ForgeTrainSnap>,
    );
    static CACHE: std::sync::Mutex<Option<ForgeTrainCacheEntry>> = std::sync::Mutex::new(None);
    let live = forge_when_free_status_path().and_then(|path| file_ident(&path));
    let cycle = forge_last_cycle_path().and_then(|path| file_ident(&path));
    if let Ok(guard) = CACHE.lock()
        && let Some((cached_live, cached_cycle, snap)) = guard.as_ref()
        && cached_live == &live
        && cached_cycle == &cycle
    {
        return snap.clone();
    }
    let snap = load_forge_train_snap_from_json();
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((live, cycle, snap.clone()));
    }
    snap
}

fn load_forge_train_snap_from_json() -> Option<ForgeTrainSnap> {
    if let Some(path) = forge_when_free_status_path()
        && let Some(v) = load_json_cached(&path)
    {
        let state = v
            .get("state")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        // Only surface active training / reclaim / densify — not idle polling.
        if matches!(
            state.as_str(),
            "training" | "reclaiming" | "densifying" | "failed"
        ) {
            let step = v.get("train_step").and_then(|x| x.as_u64()).or_else(|| {
                v.get("train_step")
                    .and_then(|x| x.as_i64())
                    .map(|i| i as u64)
            });
            let total = v.get("train_total").and_then(|x| x.as_u64()).or_else(|| {
                v.get("train_total")
                    .and_then(|x| x.as_i64())
                    .map(|i| i as u64)
            });
            let eta = v.get("train_eta_sec").and_then(|x| x.as_u64()).or_else(|| {
                v.get("train_eta_sec")
                    .and_then(|x| x.as_i64())
                    .map(|i| i as u64)
            });
            let loss = v
                .get("train_loss_live")
                .and_then(|x| x.as_f64())
                .or_else(|| v.get("train_loss").and_then(|x| x.as_f64()));
            let loss_min = v.get("train_loss_min").and_then(|x| x.as_f64());
            let loss_max = v.get("train_loss_max").and_then(|x| x.as_f64());
            let prior_loss = v.get("prior_train_loss").and_then(|x| x.as_f64());
            let loss_imp = v.get("train_loss_improvement").and_then(|x| x.as_f64());
            let phase = v
                .get("train_phase")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    // Infer post_train when steps complete but job still running.
                    match (step, total) {
                        (Some(s), Some(t)) if t > 0 && s >= t => Some("post_train".into()),
                        _ => None,
                    }
                });
            return Some(ForgeTrainSnap {
                state,
                train_step: step,
                train_total: total,
                train_eta_sec: eta,
                train_loss: loss,
                train_loss_min: loss_min,
                train_loss_max: loss_max,
                prior_train_loss: prior_loss,
                train_loss_improvement: loss_imp,
                gpu_free_mib: v.get("gpu_free_mib").and_then(|x| x.as_f64()),
                free_mib_min: v.get("free_mib_min").and_then(|x| x.as_f64()),
                vram_warn: v
                    .get("vram_warn")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                train_phase: phase,
                version: v
                    .get("version")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                gate_pass: v.get("gate_pass").and_then(|x| x.as_bool()),
                promoted: v.get("promoted").and_then(|x| x.as_bool()),
                adapter_local: false,
                open_lever_top: v
                    .get("open_lever_top")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                free_train_primary_n: v
                    .get("free_train_primary_n")
                    .and_then(|x| x.as_u64())
                    .or_else(|| {
                        v.get("free_train_primary_n")
                            .and_then(|x| x.as_i64())
                            .map(|i| i as u64)
                    }),
                measured_hold_us: v.get("measured_hold_us").and_then(|x| x.as_f64()),
                preference_n: v.get("preference_n").and_then(|x| x.as_u64()).or_else(|| {
                    v.get("preference_n")
                        .and_then(|x| x.as_i64())
                        .map(|i| i as u64)
                }),
                coding_eval_n: v.get("coding_eval_n").and_then(|x| x.as_u64()).or_else(|| {
                    v.get("coding_eval_n")
                        .and_then(|x| x.as_i64())
                        .map(|i| i as u64)
                }),
                coding_eval_primary_n: v
                    .get("coding_eval_primary_n")
                    .and_then(|x| x.as_u64())
                    .or_else(|| {
                        v.get("coding_eval_primary_n")
                            .and_then(|x| x.as_i64())
                            .map(|i| i as u64)
                    }),
            });
        }
    }
    // Completed cycle handoff (gate / adapter on disk). Prefer status when
    // finalize wrote done with next_PRIMARY/ftP/HOLD (fresher than last-cycle).
    if let Some(status_path) = forge_when_free_status_path()
        && let Some(v) = load_json_cached(&status_path)
    {
        let state = v.get("state").and_then(|x| x.as_str()).unwrap_or("");
        if (state == "done" || state == "failed")
            && let Some(snap) = forge_train_snap_from_done_value(&v, state)
        {
            return Some(snap);
        }
    }
    let path = forge_last_cycle_path()?;
    let v = load_json_cached(&path)?;
    forge_train_snap_from_done_value(&v, "done")
}

/// Build a done/failed snap from status or last-cycle JSON.
fn forge_train_snap_from_done_value(v: &serde_json::Value, state: &str) -> Option<ForgeTrainSnap> {
    let version = v
        .get("version")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let gate = v.get("gate_pass").and_then(|x| x.as_bool());
    let promoted = v.get("promoted").and_then(|x| x.as_bool());
    let adapter_local = v
        .get("adapter_local")
        .and_then(|x| x.as_str())
        .map(|s| !s.is_empty())
        .or_else(|| v.get("adapter_local").and_then(|x| x.as_bool()))
        .unwrap_or(false);
    let loss = v.get("train_loss").and_then(|x| x.as_f64());
    let open_lever_top = v
        .get("open_lever_top")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            // Fall back to living board rank when cycle lacks stamp.
            load_living_peer_open_levers(1)
                .into_iter()
                .next()
                .map(|l| l.key)
        });
    let free_train_primary_n = v
        .get("free_train_primary_n")
        .and_then(|x| x.as_u64())
        .or_else(|| {
            v.get("free_train_primary_n")
                .and_then(|x| x.as_i64())
                .map(|i| i as u64)
        });
    let measured_hold_us = v.get("measured_hold_us").and_then(|x| x.as_f64());
    let preference_n = v.get("preference_n").and_then(|x| x.as_u64()).or_else(|| {
        v.get("preference_n")
            .and_then(|x| x.as_i64())
            .map(|i| i as u64)
    });
    let coding_eval_n = v.get("coding_eval_n").and_then(|x| x.as_u64()).or_else(|| {
        v.get("coding_eval_n")
            .and_then(|x| x.as_i64())
            .map(|i| i as u64)
    });
    let coding_eval_primary_n = v
        .get("coding_eval_primary_n")
        .and_then(|x| x.as_u64())
        .or_else(|| {
            v.get("coding_eval_primary_n")
                .and_then(|x| x.as_i64())
                .map(|i| i as u64)
        });
    if version.is_none() && gate.is_none() {
        return None;
    }
    Some(ForgeTrainSnap {
        state: if state == "failed" {
            "failed".into()
        } else {
            "done".into()
        },
        train_step: None,
        train_total: None,
        train_eta_sec: None,
        train_loss: loss,
        train_loss_min: v.get("train_loss_min").and_then(|x| x.as_f64()),
        train_loss_max: v.get("train_loss_max").and_then(|x| x.as_f64()),
        prior_train_loss: v.get("prior_train_loss").and_then(|x| x.as_f64()),
        train_loss_improvement: v.get("train_loss_improvement").and_then(|x| x.as_f64()),
        gpu_free_mib: v.get("gpu_free_mib").and_then(|x| x.as_f64()),
        free_mib_min: v.get("free_mib_min").and_then(|x| x.as_f64()),
        vram_warn: v
            .get("vram_warn")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        train_phase: None,
        version,
        gate_pass: gate,
        promoted,
        adapter_local,
        open_lever_top,
        free_train_primary_n,
        measured_hold_us,
        preference_n,
        coding_eval_n,
        coding_eval_primary_n,
    })
}

/// Short strip fragment: `forge 40/200 ~38m` or `forge v1 gate✓ local`.
pub(crate) fn forge_train_strip_fragment(snap: &ForgeTrainSnap) -> String {
    if snap.state == "training" || snap.state == "reclaiming" || snap.state == "densifying" {
        if let (Some(step), Some(total)) = (snap.train_step, snap.train_total)
            && total > 0
        {
            let mut s = format!("forge {step}/{total}");
            let phase = snap.train_phase.as_deref().unwrap_or("");
            let post = step >= total
                || matches!(
                    phase,
                    "post_train" | "adapter_eval" | "base_eval" | "eval" | "save" | "gate"
                );
            if post {
                let short = match phase {
                    "adapter_eval" => "eval",
                    "base_eval" => "base",
                    "post_train" | "" => "post",
                    "save" => "save",
                    "gate" => "gate",
                    other if !other.is_empty() => other,
                    _ => "post",
                };
                s.push(' ');
                s.push_str(short);
            } else if let Some(eta) = snap.train_eta_sec
                && eta > 0
            {
                s.push_str(&format!(" ~{}m", eta / 60));
            }
            if let Some(loss) = snap.train_loss
                && loss.is_finite()
                && loss > 0.0
            {
                s.push_str(&format!(" L{loss:.2}"));
            }
            // Loss range across job (pulse extrema) when it spreads.
            match (snap.train_loss_min, snap.train_loss_max) {
                (Some(lo), Some(hi))
                    if lo.is_finite() && hi.is_finite() && lo > 0.0 && hi > lo + 0.02 =>
                {
                    s.push_str(&format!(" L↓{lo:.2}"));
                }
                _ => {}
            }
            // Live LΔ vs prior cycle when improving (pulse stamps).
            if let Some(imp) = snap.train_loss_improvement
                && imp.is_finite()
                && imp >= 0.05
            {
                s.push_str(&format!(" LΔ{:+.0}%", imp * 100.0));
            }
            // VRAM OOM risk (pulse free_mib collapse) — compact free15G / VRAM!.
            if let Some(ref w) = snap.vram_warn {
                if w == "crit" {
                    s.push_str(" VRAM!");
                } else if w == "low" {
                    s.push_str(" VRAM↓");
                }
            } else if let Some(free) = snap.gpu_free_mib
                && free.is_finite()
                && free >= 1024.0
            {
                s.push_str(&format!(" f{:.0}G", free / 1024.0));
            }
            // Predicted harvest adapter (pulse stamps next_adapter_version → version).
            if let Some(ref ver) = snap.version
                && ver.starts_with('v')
            {
                s.push_str(&format!(" →{ver}"));
            }
            // Mid-train: name living PRIMARY attack surface (pulse stamps it).
            if let Some(ref open) = snap.open_lever_top {
                let k = open.replace('·', "x").to_ascii_lowercase();
                let short = if k.starts_with("32768") {
                    "→32k"
                } else if k.starts_with("16384") {
                    "→16k"
                } else if k.starts_with("8192") {
                    "→8k"
                } else {
                    open.as_str()
                };
                s.push(' ');
                s.push_str(short);
            }
            // Prior harvest / peer HOLD floor so mid-train strip mirrors morning.
            if let Some(h) = snap.measured_hold_us
                && h.is_finite()
                && h > 0.0
            {
                if h >= 10_000.0 {
                    s.push_str(&format!(" H{:.0}k", h / 1000.0));
                } else {
                    s.push_str(&format!(" H{h:.0}"));
                }
            }
            // Phase-4 preference + coding_eval densify (pulse stamps both).
            if let Some(p) = snap.preference_n
                && p > 0
            {
                s.push_str(&format!(" pref{p}"));
            }
            if let Some(c) = snap.coding_eval_n
                && c > 0
            {
                s.push_str(&format!(" ce{c}"));
            }
            return s;
        }
        return format!("forge {}", snap.state);
    }
    if snap.state == "failed" {
        return "forge fail".into();
    }
    // done / last cycle
    let mut s = String::from("forge");
    if let Some(ref v) = snap.version {
        s.push(' ');
        s.push_str(v);
    }
    if let Some(g) = snap.gate_pass {
        s.push_str(if g { " gate✓" } else { " gate✗" });
    }
    if snap.promoted == Some(true) {
        s.push_str(" prom");
    } else if snap.adapter_local {
        // AUTOPROMOTE=0: local rsync is the morning artifact.
        s.push_str(" local");
    }
    // Harvest loss extrema when spread (same L↓ chip as mid-train strip).
    match (snap.train_loss_min, snap.train_loss_max) {
        (Some(lo), Some(hi)) if lo.is_finite() && hi.is_finite() && lo > 0.0 && hi > lo + 0.02 => {
            s.push_str(&format!(" L↓{lo:.2}"));
        }
        _ => {}
    }
    if let Some(ref open) = snap.open_lever_top {
        // Compact: 32768x1 → →32k so strip stays short beside open lever peer chip.
        let k = open.replace('·', "x").to_ascii_lowercase();
        let short = if k.starts_with("32768") {
            "→32k"
        } else if k.starts_with("16384") {
            "→16k"
        } else if k.starts_with("8192") {
            "→8k"
        } else {
            open.as_str()
        };
        s.push(' ');
        s.push_str(short);
    }
    // Harvest dual-lane PRIMARY goldens (insurance finalize stamps this).
    if let Some(n) = snap.free_train_primary_n
        && n > 0
    {
        s.push_str(&format!(" ftP{n}"));
    }
    // Measured PRIMARY HOLD floor (r7@38300 → H38k) — NEW_HOLD densify gate.
    if let Some(h) = snap.measured_hold_us
        && h.is_finite()
        && h > 0.0
    {
        if h >= 10_000.0 {
            s.push_str(&format!(" H{:.0}k", h / 1000.0));
        } else {
            s.push_str(&format!(" H{h:.0}"));
        }
    }
    // Phase-4 preference + coding_eval densify.
    if let Some(p) = snap.preference_n
        && p > 0
    {
        s.push_str(&format!(" pref{p}"));
    }
    if let Some(c) = snap.coding_eval_n
        && c > 0
    {
        s.push_str(&format!(" ce{c}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::tests::env_lock()
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let prev = std::env::var_os(key);
            match value {
                // TODO: Audit that the environment access only happens in single-threaded code.
                Some(v) => unsafe { std::env::set_var(key, v) },
                // TODO: Audit that the environment access only happens in single-threaded code.
                None => unsafe { std::env::remove_var(key) },
            }
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                // TODO: Audit that the environment access only happens in single-threaded code.
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                // TODO: Audit that the environment access only happens in single-threaded code.
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    fn file_identity(path: &Path) -> (std::time::SystemTime, u64) {
        let meta = std::fs::metadata(path).unwrap();
        (meta.modified().unwrap(), meta.len())
    }

    fn overwrite_preserving_identity(path: &Path, bytes: &[u8]) {
        let (mtime, len) = file_identity(path);
        assert_eq!(len, bytes.len() as u64, "replacement must keep len");
        std::fs::write(path, bytes).unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        assert_eq!(file_identity(path), (mtime, len));
    }

    fn bump_mtime(path: &Path) {
        let bumped = file_identity(path).0 + std::time::Duration::from_secs(2);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(bumped)
            .unwrap();
    }

    #[test]
    fn parses_treebeard_aliases() {
        assert_eq!(Lane::parse("treebeard"), Lane::Treebeard);
        assert_eq!(Lane::parse("RLM"), Lane::Treebeard);
        assert_eq!(Lane::parse("hi/q"), Lane::Treebeard);
        assert_eq!(Lane::parse("default"), Lane::Default);
        assert_eq!(Lane::parse(""), Lane::Default);
    }

    #[test]
    fn treebeard_enables_handle_read_unless_forced_off() {
        let _g = env_lock();
        let _lane = EnvGuard::set("ANGEL_LANE", Some("treebeard"));
        let _store = EnvGuard::set("ANGEL_HANDLE_STORE", Some("1"));
        let _read = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", None);
        assert!(handle_read_tool_enabled());

        let _off = EnvGuard::set("ANGEL_HANDLE_READ_TOOL", Some("0"));
        assert!(!handle_read_tool_enabled());
    }

    #[test]
    fn treebeard_lowers_offload_floors_when_env_unset() {
        let _g = env_lock();
        let _cm = EnvGuard::set("ANGEL_HANDLE_CODE_MODE_MIN_BYTES", None);
        let _sc = EnvGuard::set("ANGEL_HANDLE_SUBCALL_MIN_BYTES", None);

        let _def = EnvGuard::set("ANGEL_LANE", Some("default"));
        assert_eq!(code_mode_offload_min_bytes(), 4096);
        assert_eq!(lane_subcall_offload_min_bytes(), 2048);

        let _tb = EnvGuard::set("ANGEL_LANE", None);
        assert_eq!(code_mode_offload_min_bytes(), 1024);
        assert_eq!(lane_subcall_offload_min_bytes(), 512);

        let _explicit = EnvGuard::set("ANGEL_HANDLE_CODE_MODE_MIN_BYTES", Some("9999"));
        assert_eq!(code_mode_offload_min_bytes(), 9999);
    }

    #[test]
    fn treebeard_system_block_is_nonempty_only_on_lane() {
        let _g = env_lock();
        let _off = EnvGuard::set("ANGEL_LANE", Some("default"));
        assert!(lane_system_suffix().is_empty());
        let _on = EnvGuard::set("ANGEL_LANE", None);
        assert!(lane_system_suffix().contains("treebeard lane"));
        assert!(lane_system_suffix().contains("handle_read"));
        assert!(lane_system_suffix().contains("handle_put"));
        assert!(!lane_system_suffix().contains("popcorn-submit-hiq"));
        assert!(!lane_system_suffix().contains("B6LDP"));
    }

    #[test]
    fn living_competition_suffix_reads_peer_state() {
        let _g = env_lock();
        let _lane_off = EnvGuard::set("ANGEL_LANE", Some("default"));
        assert!(living_competition_system_suffix().is_empty());

        let peer = std::env::temp_dir().join(format!(
            "angel-peer-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(
            &peer,
            r#"{"geomean_us":867.912,"name":"b200_c3_peer.txt","path":"/tmp/c3.txt","p1_us":1684.5,"shapes":{"32768x1":38800.0,"512x640":1684.5,"128x256":61.2,"256x64":144.0},"shape_bests":{"32768x1":{"us":38300.0,"name":"b200_r7"},"128x256":{"us":60.9,"name":"multi128"},"256x64":{"us":130.0,"name":"r7v2"}}}"#,
        )
        .unwrap();
        let _peer = EnvGuard::set(
            "POPCORN_PEER_STATE",
            Some(peer.to_str().expect("utf8 path")),
        );
        let _lane = EnvGuard::set("ANGEL_LANE", None);
        let _gpu = EnvGuard::set("ANGEL_GPU_COMP_LOCAL_MOA", Some("1"));
        let s = living_competition_system_suffix();
        assert_eq!(
            load_living_peer_p1_us().map(|u| (u * 10.0).round() / 10.0),
            Some(1684.5)
        );
        let holds = load_living_peer_shape_holds(3);
        assert!(
            holds.iter().any(|h| h.key == "256x64"),
            "shape holds include 256x64: {holds:?}"
        );
        let open = load_living_peer_open_levers(3);
        assert!(
            !open.is_empty() && open[0].key == "32768x1",
            "highest board µs first: {open:?}"
        );
        assert!(
            s.contains("PRIMARY HOLD floor") && s.contains("38300") && s.contains("NEW_HOLD"),
            "root names PRIMARY HOLD densify floor: {s}"
        );
        assert!(
            open[0].geo_drop_if_half_pct > 0.0,
            "half-geo impact on open levers: {open:?}"
        );
        let _ = std::fs::remove_file(&peer);
        assert!(s.contains("867.91"), "got: {s}");
        assert!(s.contains("living B200 peer"), "got: {s}");
        assert!(s.contains("512"), "P1 open lever mentioned: {s}");
        assert!(
            s.contains("Primary attack") && s.contains("512x640"),
            "root names primary attack lever: {s}"
        );
        assert!(
            s.contains("½geo↓") || s.contains("half-cut"),
            "root names geomean impact: {s}"
        );
        assert!(
            s.contains("1684") || s.contains("1685"),
            "dynamic P1 peer shape in suffix: {s}"
        );
        assert!(
            s.contains("Open levers") && s.contains("512x640"),
            "root names open levers by board µs: {s}"
        );
        assert!(
            s.contains("Shape holds") && s.contains("256x64"),
            "root names shape holds: {s}"
        );
    }

    #[test]
    fn living_peer_json_caches_by_mtime_len() {
        let _g = env_lock();
        let peer = std::env::temp_dir().join(format!(
            "angel-peer-cache-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(
            &peer,
            r#"{"geomean_us":867.912,"name":"b200_c3_peer.txt","path":"/tmp/c3.txt","p1_us":1684.5,"shapes":{"32768x1":38800.0,"512x640":1684.5}}"#,
        )
        .unwrap();
        let _peer = EnvGuard::set(
            "POPCORN_PEER_STATE",
            Some(peer.to_str().expect("utf8 path")),
        );
        let first = load_living_peer_snapshot().expect("first load");
        assert_eq!(first.1, "b200_c3_peer.txt");
        let first_p1 = load_living_peer_p1_us();
        let first_open = load_living_peer_open_levers(1);

        let original = std::fs::read(&peer).unwrap();
        let mut garbage = vec![b'x'; original.len()];
        let last = garbage.len() - 1;
        garbage[0] = b'{';
        garbage[last] = b'}';
        overwrite_preserving_identity(&peer, &garbage);
        assert_eq!(
            load_living_peer_snapshot().expect("cache hit"),
            first,
            "unchanged mtime/len must keep the cached snapshot even if bytes changed"
        );
        assert_eq!(load_living_peer_p1_us(), first_p1);
        assert_eq!(load_living_peer_open_levers(1), first_open);

        std::fs::write(
            &peer,
            r#"{"geomean_us":900.25,"name":"updated_peer.txt","path":"/tmp/c4.txt","p1_us":1700.0,"shapes":{"32768x1":40000.0,"512x640":1700.0}}"#,
        )
        .unwrap();
        bump_mtime(&peer);
        let busted = load_living_peer_snapshot().expect("mtime/len change");
        assert_eq!(busted.1, "updated_peer.txt");
        assert!(
            (busted.0 - 900.25).abs() < 1e-9,
            "busted geo: {:?}",
            busted.0
        );
        assert_eq!(
            load_living_peer_p1_us().map(|u| (u * 10.0).round() / 10.0),
            Some(1700.0)
        );
        let open = load_living_peer_open_levers(1);
        assert_eq!(open[0].key, "32768x1");
        assert!((open[0].board_us - 40000.0).abs() < 1e-9);

        std::fs::remove_file(&peer).unwrap();
        assert!(
            load_living_peer_snapshot().is_none(),
            "deleted peer file must not leak a previous cache"
        );
    }

    #[test]
    fn forge_train_snap_reads_status_and_formats_fragment() {
        let _g = env_lock();
        let id = std::process::id();
        let status = std::env::temp_dir().join(format!("forge-when-free-status-{id}.json"));
        let cycle = std::env::temp_dir().join(format!("forge-last-cycle-{id}.json"));
        let idle = std::env::temp_dir().join(format!("forge-when-free-idle-{id}.json"));
        std::fs::write(
            &status,
            r#"{"state":"training","train_step":40,"train_total":200,"train_eta_sec":2280}"#,
        )
        .unwrap();
        let _s = EnvGuard::set(
            "FORGE_WHEN_FREE_STATUS",
            Some(status.to_str().expect("utf8")),
        );
        let snap = load_forge_train_snap().expect("snap");
        assert_eq!(snap.state, "training");
        assert_eq!(snap.train_step, Some(40));
        assert_eq!(snap.train_total, Some(200));
        let frag = forge_train_strip_fragment(&snap);
        assert_eq!(frag, "forge 40/200 ~38m");
        // Mid-train PRIMARY attack surface from pulse stamp.
        std::fs::write(
            &status,
            r#"{"state":"training","train_step":40,"train_total":200,"train_eta_sec":2280,"open_lever_top":"32768x1","train_loss_live":0.2,"measured_hold_us":38300.0,"preference_n":96,"coding_eval_n":24,"coding_eval_primary_n":4,"version":"v3","next_adapter_version":"v3","gpu_free_mib":15000.0,"free_mib_min":14900.0}"#,
        )
        .unwrap();
        let snap_p = load_forge_train_snap().expect("primary mid-train");
        assert_eq!(snap_p.open_lever_top.as_deref(), Some("32768x1"));
        assert_eq!(snap_p.measured_hold_us, Some(38300.0));
        assert_eq!(snap_p.preference_n, Some(96));
        assert_eq!(snap_p.coding_eval_n, Some(24));
        assert_eq!(snap_p.coding_eval_primary_n, Some(4));
        assert_eq!(snap_p.version.as_deref(), Some("v3"));
        assert_eq!(snap_p.gpu_free_mib, Some(15000.0));
        let frag_p = forge_train_strip_fragment(&snap_p);
        assert!(
            frag_p.contains("→32k")
                && frag_p.contains("L0.20")
                && frag_p.contains("H38k")
                && frag_p.contains("pref96")
                && frag_p.contains("ce24")
                && frag_p.contains("→v3")
                && frag_p.contains("f15G"),
            "mid-train strip names adapter+PRIMARY+HOLD+pref+ce+free: {frag_p}"
        );
        // Steps complete → post phase even without train_phase field.
        std::fs::write(
            &status,
            r#"{"state":"training","train_step":200,"train_total":200,"train_phase":"adapter_eval"}"#,
        )
        .unwrap();
        let snap_post = load_forge_train_snap().expect("post");
        assert_eq!(snap_post.train_phase.as_deref(), Some("adapter_eval"));
        assert_eq!(forge_train_strip_fragment(&snap_post), "forge 200/200 eval");
        // last cycle when not training
        std::fs::write(
            &cycle,
            r#"{"version":"v1","gate_pass":true,"promoted":false,"adapter_local":"/home/u/angel-forge/adapters/v1","open_lever_top":"32768x1","free_train_primary_n":4,"measured_hold_us":38300.0,"train_loss":0.4,"train_loss_min":0.25,"train_loss_max":0.53,"preference_n":96,"coding_eval_n":24,"coding_eval_primary_n":4}"#,
        )
        .unwrap();
        std::fs::write(&idle, r#"{"state":"polling","note":"waiting"}"#).unwrap();
        let _s2 = EnvGuard::set("FORGE_WHEN_FREE_STATUS", Some(idle.to_str().expect("utf8")));
        let _c = EnvGuard::set("FORGE_LAST_CYCLE", Some(cycle.to_str().expect("utf8")));
        let done = load_forge_train_snap().expect("cycle");
        assert_eq!(done.state, "done");
        assert!(done.adapter_local);
        assert_eq!(done.open_lever_top.as_deref(), Some("32768x1"));
        assert_eq!(done.free_train_primary_n, Some(4));
        assert_eq!(done.measured_hold_us, Some(38300.0));
        assert_eq!(done.coding_eval_n, Some(24));
        assert_eq!(done.train_loss_min, Some(0.25));
        assert_eq!(done.train_loss_max, Some(0.53));
        let frag = forge_train_strip_fragment(&done);
        assert!(
            frag.contains("forge v1 gate✓ local")
                && frag.contains("L↓0.25")
                && frag.contains("→32k")
                && frag.contains("ftP4")
                && frag.contains("H38k")
                && frag.contains("pref96")
                && frag.contains("ce24"),
            "got: {frag}"
        );
        // Done status breadcrumb (finalize) preferred over last-cycle when present.
        std::fs::write(
            &status,
            r#"{"state":"done","version":"v2","gate_pass":true,"promoted":false,"adapter_local":"/home/u/angel-forge/adapters/v2","open_lever_top":"32768x1","free_train_primary_n":4,"measured_hold_us":38300.0,"train_loss":0.4,"ok":true}"#,
        )
        .unwrap();
        let _s_done = EnvGuard::set(
            "FORGE_WHEN_FREE_STATUS",
            Some(status.to_str().expect("utf8")),
        );
        let done_st = load_forge_train_snap().expect("done status");
        assert_eq!(done_st.version.as_deref(), Some("v2"));
        assert_eq!(done_st.measured_hold_us, Some(38300.0));
        let frag_st = forge_train_strip_fragment(&done_st);
        assert!(
            frag_st.contains("forge v2") && frag_st.contains("H38k"),
            "done status strip: {frag_st}"
        );
        // Root LID suffix names adapter when Treebeard + last-cycle (not live done status).
        std::fs::write(&idle, r#"{"state":"polling","note":"waiting"}"#).unwrap();
        let _s_idle_root =
            EnvGuard::set("FORGE_WHEN_FREE_STATUS", Some(idle.to_str().expect("utf8")));
        let _lane = EnvGuard::set("ANGEL_LANE", Some("treebeard"));
        let root = free_train_system_suffix();
        assert!(root.contains("free-train"), "got: {root}");
        assert!(root.contains("v1"), "got: {root}");
        assert!(root.contains("gate=pass"), "got: {root}");
        assert!(
            root.contains("local-disk") || root.contains("AUTOPROMOTE"),
            "got: {root}"
        );
        assert!(
            root.contains("next_PRIMARY=32768x1") || root.contains("32768"),
            "root names next PRIMARY lever: {root}"
        );
        assert!(
            root.contains("free_train_PRIMARY_rows=4"),
            "root names free_train PRIMARY mass: {root}"
        );
        assert!(
            root.contains("PRIMARY_HOLD_floor=38300us") || root.contains("NEW_HOLD"),
            "root names PRIMARY HOLD floor: {root}"
        );
        assert!(
            root.contains("Lrange=0.250–0.530") || root.contains("Lrange=0.25"),
            "last-cycle root names Lrange: {root}"
        );
        // In-flight training suffix (restore mid-train status after post-phase write).
        std::fs::write(
            &status,
            r#"{"state":"training","train_step":40,"train_total":200,"train_eta_sec":2280}"#,
        )
        .unwrap();
        let _s3 = EnvGuard::set(
            "FORGE_WHEN_FREE_STATUS",
            Some(status.to_str().expect("utf8")),
        );
        let flying = free_train_system_suffix();
        assert!(flying.contains("in flight"), "got: {flying}");
        assert!(flying.contains("40/200"), "got: {flying}");
        // Mid-train status with PRIMARY stamp names attack surface in root LID.
        std::fs::write(
            &status,
            r#"{"state":"training","train_step":90,"train_total":200,"train_eta_sec":1200,"open_lever_top":"32768x1","free_train_primary_n":4,"train_loss_live":0.2,"train_loss_min":0.15,"train_loss_max":0.45}"#,
        )
        .unwrap();
        let _s4 = EnvGuard::set(
            "FORGE_WHEN_FREE_STATUS",
            Some(status.to_str().expect("utf8")),
        );
        let flying_p = free_train_system_suffix();
        assert!(
            flying_p.contains("next_PRIMARY=32768x1"),
            "in-flight root names PRIMARY: {flying_p}"
        );
        assert!(
            flying_p.contains("prior_free_train_PRIMARY_rows=4"),
            "in-flight root names prior densify: {flying_p}"
        );
        assert!(
            flying_p.contains("Lrange=0.150–0.450") || flying_p.contains("Lrange=0.15"),
            "in-flight root names Lrange: {flying_p}"
        );
        let frag_l = forge_train_strip_fragment(&load_forge_train_snap().expect("flying snap"));
        assert!(
            frag_l.contains("L↓0.15") && frag_l.contains("L0.20"),
            "mid-train strip L↓ chip: {frag_l}"
        );
        let _ = std::fs::remove_file(&status);
        let _ = std::fs::remove_file(&cycle);
        let _ = std::fs::remove_file(&idle);
    }

    #[test]
    fn forge_train_snap_caches_by_mtime_len() {
        let _g = env_lock();
        let id = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let status =
            std::env::temp_dir().join(format!("forge-snap-cache-status-{id}-{nanos}.json"));
        let missing =
            std::env::temp_dir().join(format!("forge-snap-cache-missing-{id}-{nanos}.json"));
        std::fs::write(
            &status,
            r#"{"state":"training","train_step":40,"train_total":200,"train_eta_sec":2280}"#,
        )
        .unwrap();
        let _s = EnvGuard::set(
            "FORGE_WHEN_FREE_STATUS",
            Some(status.to_str().expect("utf8")),
        );
        let _c = EnvGuard::set("FORGE_LAST_CYCLE", Some(missing.to_str().expect("utf8")));
        let first = load_forge_train_snap().expect("first load");
        assert_eq!(first.train_step, Some(40));

        let original = std::fs::read(&status).unwrap();
        let mut garbage = vec![b'x'; original.len()];
        let last = garbage.len() - 1;
        garbage[0] = b'{';
        garbage[last] = b'}';
        overwrite_preserving_identity(&status, &garbage);
        assert_eq!(
            load_forge_train_snap().expect("cache hit"),
            first,
            "unchanged mtime/len must keep the cached snap even if bytes changed"
        );

        std::fs::write(
            &status,
            r#"{"state":"training","train_step":90,"train_total":200,"train_eta_sec":1200}"#,
        )
        .unwrap();
        bump_mtime(&status);
        let busted = load_forge_train_snap().expect("mtime/len change");
        assert_eq!(busted.train_step, Some(90));
        assert_ne!(busted, first);

        std::fs::remove_file(&status).unwrap();
        assert!(
            load_forge_train_snap().is_none(),
            "deleted status must not leak a previous cache"
        );
    }

    #[test]
    fn subcall_depth_guard_restores_and_gates_spawn() {
        let _g = env_lock();
        let _depth = EnvGuard::set("ANGEL_TREEBEARD_MAX_DEPTH", Some("2"));
        let _lane = EnvGuard::set("ANGEL_LANE", Some("treebeard"));
        assert_eq!(subcall_depth(), 0);
        assert!(spawn_nesting_allowed());
        {
            let _d1 = SubcallDepthGuard::enter();
            assert_eq!(subcall_depth(), 1);
            assert!(spawn_nesting_allowed());
            {
                let _d2 = SubcallDepthGuard::enter();
                assert_eq!(subcall_depth(), 2);
                assert!(!spawn_nesting_allowed());
            }
            assert_eq!(subcall_depth(), 1);
        }
        assert_eq!(subcall_depth(), 0);
    }

    #[test]
    fn eager_offload_floor_respects_lane_and_env() {
        let _g = env_lock();
        let _e = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", None);
        let _def = EnvGuard::set("ANGEL_LANE", Some("default"));
        assert_eq!(eager_tool_offload_min_bytes(), 16 * 1024);
        let _tb = EnvGuard::set("ANGEL_LANE", None);
        assert_eq!(eager_tool_offload_min_bytes(), 16 * 1024);
        let _x = EnvGuard::set("ANGEL_HANDLE_EAGER_TOOL_MIN_BYTES", Some("100"));
        assert_eq!(eager_tool_offload_min_bytes(), 100);
    }
}

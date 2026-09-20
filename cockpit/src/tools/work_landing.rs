//! Work landing — the agent's **first-contact onboarding** for a workspace.
//!
//! When a conversation starts, the agent should establish its WORK CONTEXT before
//! doing substantive work: (1) what folder/project are we in, (2) what's the GitHub
//! repo, and (3) is it private. Those three answers pick a BEHAVIOR MODE:
//!   * `internal-dev`  — private or purely local work: optimize for velocity and
//!     iteration, internal tooling, fewer public-exposure worries.
//!   * `public-facing` — a public repo: extra care — never commit secrets, be
//!     deliberate about what's pushed/exposed, quality-conscious, professional
//!     commit hygiene, assume the world reads it.
//!
//! Surfaced two ways (mirroring `self_model`):
//!   * [`work_context_block`] — a concise block injected into the system preamble by
//!     `bootstrap`. If a CONFIRMED context exists it states the active mode + its
//!     behavioral guidance; otherwise it instructs the agent to self-direct the
//!     landing (call `work_landing`, then confirm with the user and record it).
//!   * [`WorkLandingTool`] — the `work_landing` tool the agent calls to DETECT the
//!     folder/repo/visibility (gracefully — never panics, never blocks on the
//!     network beyond a short timeout) and to RECORD the user's confirmation.
//!
//! The established context is persisted per-workspace to
//! `~/.angel0/work-context/<key>.json` (override the dir with
//! `ANGEL_WORK_CONTEXT_DIR`), so the mode survives restarts and a `/cd` to a new
//! repo lands a fresh (unconfirmed) context that re-triggers onboarding.
//!
//! The detection / normalization / mode-inference / persistence logic is factored
//! into PURE functions ([`normalize_remote`], [`parse_gh_visibility`],
//! [`infer_mode`], [`assemble_context`], [`save_context_in`]/[`load_context_in`])
//! that take the git/gh outputs as plain strings, so they are headless-testable
//! without spawning a single subprocess.

use crate::club::ToolDef;
use crate::harness::{Tool, env_flag};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Repository visibility, as established for the workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
    /// Couldn't determine (no remote, `gh` missing/unauthed, or a non-GitHub host).
    Unknown,
}

impl Visibility {
    /// Human/agent-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Unknown => "unknown",
        }
    }

    /// Parse a user-supplied visibility override (case-insensitive). `None` on an
    /// unrecognized value so the caller can keep the detected value.
    pub fn parse_label(s: &str) -> Option<Visibility> {
        match s.trim().to_ascii_lowercase().as_str() {
            "public" | "pub" => Some(Visibility::Public),
            "private" | "priv" | "internal" => Some(Visibility::Private),
            "unknown" | "?" | "" => Some(Visibility::Unknown),
            _ => None,
        }
    }
}

/// The behavior mode the established context selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Private/local: velocity-focused, internal tooling.
    #[serde(rename = "internal-dev")]
    InternalDev,
    /// Public repo: extra care — no secrets, mindful of exposure, assume external readers.
    #[serde(rename = "public-facing")]
    PublicFacing,
    /// Not yet inferable — the agent must ask the user.
    #[serde(rename = "unset")]
    Unset,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::InternalDev => "internal-dev",
            Mode::PublicFacing => "public-facing",
            Mode::Unset => "unset",
        }
    }

    pub fn parse_label(s: &str) -> Option<Mode> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "internal-dev" | "internal" | "internaldev" | "dev" => Some(Mode::InternalDev),
            "public-facing" | "public" | "publicfacing" => Some(Mode::PublicFacing),
            "unset" | "none" | "" => Some(Mode::Unset),
            _ => None,
        }
    }

    /// The 1–3 line behavioral guidance injected into the system prompt for this
    /// mode (empty for `Unset` — the agent is told to ask instead).
    pub fn guidance(self) -> &'static str {
        match self {
            Mode::InternalDev => {
                "Optimize for velocity and iteration — this is internal/local tooling. \
                 Fewer public-exposure worries; move fast. (Still don't hardcode real \
                 production secrets, but you're not writing for the world.)"
            }
            Mode::PublicFacing => {
                "Extra care — this repo is public. NEVER commit secrets, keys, or \
                 credentials. Be deliberate about what you push or expose; assume external \
                 readers see every line and every commit. Stay quality-conscious with \
                 professional commit hygiene."
            }
            Mode::Unset => "",
        }
    }
}

/// The established (or being-established) work context for one workspace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkContext {
    /// The active workspace folder (absolute path string).
    pub folder: String,
    /// The GitHub repo as `owner/repo`, or `None` (local / no remote / non-GitHub).
    pub repo: Option<String>,
    /// Repository visibility.
    pub visibility: Visibility,
    /// The behavior mode this context selects.
    pub mode: Mode,
    /// `true` only once the USER has confirmed the context (detection alone is a
    /// proposal). The system-prompt block treats only a confirmed context as
    /// "established".
    pub confirmed: bool,
    /// Unix seconds of the last update (provenance / staleness; 0 if unknown).
    #[serde(default)]
    pub updated_at: u64,
}

impl WorkContext {
    /// `owner/repo`, or a readable placeholder when no GitHub remote was found.
    fn repo_label(&self) -> String {
        self.repo
            .clone()
            .unwrap_or_else(|| "(none — no GitHub remote)".to_string())
    }
}

// ---------------------------------------------------------------------------
// Pure detection helpers (mock the git/gh outputs as string inputs).
// ---------------------------------------------------------------------------

/// A parsed GitHub (or git-host) remote: `owner/repo` on some `host`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoSlug {
    pub host: String,
    pub owner: String,
    pub repo: String,
}

impl RepoSlug {
    /// `owner/repo`.
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// Whether this looks like a github.com remote (vs an enterprise/other host).
    pub fn is_github(&self) -> bool {
        self.host == "github.com" || self.host.ends_with(".github.com")
    }
}

/// Normalize a `git remote get-url origin` value (ssh **or** https) into a
/// `host` + `owner/repo`. Handles the scp-like `git@host:owner/repo.git`, the
/// `https://host/owner/repo(.git)` and `ssh://git@host/owner/repo.git` URL forms,
/// trailing `.git` and slashes. Returns `None` for empty input, a local path, or
/// anything that doesn't resolve to an `owner/repo` pair — callers treat `None`
/// as "repo unknown" and ask the user.
pub fn normalize_remote(raw: &str) -> Option<RepoSlug> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let (host, path) = if let Some(idx) = s.find("://") {
        // scheme://[user@]host[:port]/owner/repo(.git)
        let rest = &s[idx + 3..];
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = authority.rsplit('@').next().unwrap_or(authority);
        let host = host.split(':').next().unwrap_or(host); // drop :port
        (host.to_string(), path.to_string())
    } else {
        // No URL scheme — try the scp-like `[user@]host:owner/repo` form.
        scp_split(s)?
    };

    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segs: Vec<&str> = path.split('/').filter(|x| !x.is_empty()).collect();
    if segs.len() < 2 {
        return None;
    }
    let owner = segs[segs.len() - 2].to_string();
    let repo = segs[segs.len() - 1].to_string();
    if owner.is_empty() || repo.is_empty() || host.is_empty() {
        return None;
    }
    Some(RepoSlug {
        host: host.to_ascii_lowercase(),
        owner,
        repo,
    })
}

/// Split the scp-like `[user@]host:owner/repo` form (no URL scheme). Requires a
/// dotted host so a Windows drive path (`C:\…`) or a bare `localhost:` is rejected.
fn scp_split(s: &str) -> Option<(String, String)> {
    let colon = s.find(':')?;
    let authority = &s[..colon];
    let path = &s[colon + 1..];
    if authority.is_empty() || path.is_empty() {
        return None;
    }
    let host = authority.rsplit('@').next().unwrap_or(authority);
    if !host.contains('.') {
        return None; // not a hostname (e.g. a drive letter) — not a remote slug
    }
    Some((host.to_string(), path.to_string()))
}

/// Map `gh repo view --json visibility -q .visibility` output to a [`Visibility`].
/// GitHub yields `PUBLIC` / `PRIVATE` / `INTERNAL`; INTERNAL (org-only, not
/// world-readable) is treated as `Private`. Empty/garbage → `Unknown`.
pub fn parse_gh_visibility(out: &str) -> Visibility {
    match out.trim().to_ascii_uppercase().as_str() {
        "PUBLIC" => Visibility::Public,
        "PRIVATE" => Visibility::Private,
        "INTERNAL" => Visibility::Private,
        _ => Visibility::Unknown,
    }
}

/// Infer the behavior mode from visibility + whether a remote exists.
///   * public                     → `public-facing`
///   * private                    → `internal-dev`
///   * unknown **with** a remote  → `unset` (a remote we can't classify — ask)
///   * unknown **without** a remote (purely local) → `internal-dev`
///
/// The only path that auto-selects `internal-dev` without GitHub confirmation is
/// local-only work (nothing is pushed anywhere), which is the safe default; an
/// unclassified remote stays `unset` so the agent must ask before assuming.
pub fn infer_mode(visibility: Visibility, has_remote: bool) -> Mode {
    match visibility {
        Visibility::Public => Mode::PublicFacing,
        Visibility::Private => Mode::InternalDev,
        Visibility::Unknown => {
            if has_remote {
                Mode::Unset
            } else {
                Mode::InternalDev
            }
        }
    }
}

/// Assemble a [`WorkContext`] from the raw command outputs — **pure**, so the whole
/// detection pipeline is testable by passing canned git/gh strings:
///   * `remote_out`        — `git remote get-url origin` stdout, or `None` when not
///     a repo / no remote / the command failed.
///   * `gh_visibility_out` — `gh … visibility` stdout, or `None` when `gh` is
///     missing/unauthed or the host isn't GitHub.
///   * `prior`             — the previously-persisted context, if any. A confirmed
///     prior whose repo+visibility still match is preserved (stays established, and
///     keeps the user's mode override); otherwise the result is unconfirmed and the
///     mode is freshly inferred (so a `/cd` to a new repo re-triggers onboarding).
///
/// `updated_at` is left at 0 here (pure); the live tool stamps it before saving.
pub fn assemble_context(
    folder: &str,
    remote_out: Option<&str>,
    gh_visibility_out: Option<&str>,
    prior: Option<&WorkContext>,
) -> WorkContext {
    let has_remote = remote_out.map(|s| !s.trim().is_empty()).unwrap_or(false);
    let repo = remote_out
        .and_then(normalize_remote)
        .map(|slug| slug.slug());
    let visibility = gh_visibility_out
        .map(parse_gh_visibility)
        .unwrap_or(Visibility::Unknown);
    let mode = infer_mode(visibility, has_remote);

    // Preserve an unchanged, already-confirmed context: same repo + same
    // visibility means nothing relevant moved, so keep `confirmed` and the user's
    // (possibly overridden) mode. Any change resets to unconfirmed → re-land.
    if let Some(p) = prior
        && p.confirmed
        && p.repo == repo
        && p.visibility == visibility
    {
        return WorkContext {
            folder: folder.to_string(),
            repo,
            visibility,
            mode: p.mode,
            confirmed: true,
            updated_at: 0,
        };
    }

    WorkContext {
        folder: folder.to_string(),
        repo,
        visibility,
        mode,
        confirmed: false,
        updated_at: 0,
    }
}

// ---------------------------------------------------------------------------
// Persistence — `~/.angel0/work-context/<key>.json` (per workspace).
// ---------------------------------------------------------------------------

/// The directory that holds per-workspace context files. `ANGEL_WORK_CONTEXT_DIR`
/// overrides it (tests point this at a tempdir); default `~/.angel0/work-context`.
pub fn context_base_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ANGEL_WORK_CONTEXT_DIR") {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".angel0/work-context")
}

/// A stable, filesystem-safe key for a workspace path: a sanitized, length-bounded
/// prefix plus a deterministic hash suffix (so distinct paths never collide and the
/// same path always maps to the same file).
///
/// One implementation, in [`crate::workspace_store::workspace_key`] — this used to
/// be a byte-for-byte copy of it, and two copies of a hash scheme that every
/// per-repo store joins on is one copy too many.
pub fn workspace_key(workspace: &Path) -> String {
    crate::workspace_store::workspace_key(workspace)
}

/// The JSON file path for `workspace` under `dir`.
pub fn context_path_in(dir: &Path, workspace: &Path) -> PathBuf {
    dir.join(format!("{}.json", workspace_key(workspace)))
}

/// Persist `ctx` for `workspace` under `dir`. Creates `dir` if needed. Pure
/// serialization (does not mutate `ctx` / does not stamp time — the caller does).
pub fn save_context_in(
    dir: &Path,
    workspace: &Path,
    ctx: &WorkContext,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = context_path_in(dir, workspace);
    let json = serde_json::to_string_pretty(ctx)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let relative = path.strip_prefix(dir).map_err(std::io::Error::other)?;
    crate::harness::confined_write(dir, relative, json.as_bytes())
        .map_err(std::io::Error::other)?;
    Ok(path)
}

/// Load the persisted context for `workspace` from `dir`. `None` when the file is
/// missing or unparseable (graceful — a corrupt file just re-triggers onboarding).
pub fn load_context_in(dir: &Path, workspace: &Path) -> Option<WorkContext> {
    let path = context_path_in(dir, workspace);
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Persist `ctx` for `workspace` in the default (env-resolved) dir.
pub fn save_context(workspace: &Path, ctx: &WorkContext) -> std::io::Result<PathBuf> {
    save_context_in(&context_base_dir(), workspace, ctx)
}

/// Load the persisted context for `workspace` from the default (env-resolved) dir.
pub fn load_context(workspace: &Path) -> Option<WorkContext> {
    load_context_in(&context_base_dir(), workspace)
}

// ---------------------------------------------------------------------------
// System-prompt injection — rendered from the (loaded) context.
// ---------------------------------------------------------------------------

/// Render the work-context block from an already-loaded context (pure → testable).
/// A confirmed context yields the "Active work context" block + the mode's
/// behavioral guidance; otherwise (no context, or unconfirmed) it yields the
/// self-direct onboarding instruction.
pub fn render_block(ctx: Option<&WorkContext>) -> String {
    match ctx {
        Some(c) if c.confirmed && c.mode != Mode::Unset => format!(
            "\n\n# Active work context\n\
             folder={folder} · repo={repo} · visibility={vis} · mode={mode}\n\
             {guidance}\n",
            folder = c.folder,
            repo = c.repo_label(),
            vis = c.visibility.label(),
            mode = c.mode.label(),
            guidance = c.mode.guidance(),
        ),
        other => {
            let mut s = String::from(
                "\n\n# Work context (not established)\n\
                 Work context is not established for this workspace. Before substantive \
                 work, call `work_landing` to detect the folder / GitHub repo / visibility, \
                 then CONFIRM with the user — e.g. \"Working in X · GitHub Y · private? — \
                 correct?\" — and record it by calling `work_landing` with confirm=true. \
                 This sets your behavior mode: internal-dev (private/local → velocity) vs \
                 public-facing-care (public repo → no secrets, mindful of exposure).\n",
            );
            // If a snapshot exists but isn't confirmed, surface what was detected so
            // the agent can lead with it.
            if let Some(c) = other {
                s.push_str(&format!(
                    "Detected so far (unconfirmed): folder={folder} · repo={repo} · \
                     visibility={vis} · proposed mode={mode}.\n",
                    folder = c.folder,
                    repo = c.repo_label(),
                    vis = c.visibility.label(),
                    mode = c.mode.label(),
                ));
            }
            s
        }
    }
}

/// The work-context block injected into the system preamble by `bootstrap`. Gated
/// by `ANGEL_WORK_LANDING` (default on). Reads the persisted context for
/// `workspace` and renders it via [`render_block`].
pub fn work_context_block(workspace: &Path) -> String {
    if !env_flag("ANGEL_WORK_LANDING", true) {
        return String::new();
    }
    render_block(load_context(workspace).as_ref())
}

// ---------------------------------------------------------------------------
// Live detection (spawns git/gh, bounded by a short timeout) — the tool side.
// ---------------------------------------------------------------------------

/// Run `program args` in `cwd` through the shared repository-probe boundary.
/// The deadline, process-tree cleanup, and output cap are in-process, so this
/// stays bounded on macOS where the coreutils `timeout` binary is absent.
fn run_capture(program: &str, args: &[&str], cwd: &Path, secs: u64) -> Option<String> {
    crate::workspace_store::capture_repo_probe(program, args, cwd, secs)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// What the live detection found, plus whether the prior persisted context was
/// already confirmed (for the summary's "already established" line).
struct Detection {
    ctx: WorkContext,
    was_established: bool,
    is_git_repo: bool,
}

/// Detect the work context for `workspace`: the folder is the workspace path; the
/// repo comes from `git remote get-url origin`; visibility from `gh` (only when the
/// remote parses to a GitHub slug and `gh` is available+authed). Graceful at every
/// tier — not a repo → folder-only; no remote → repo unknown; gh missing → unknown.
fn detect(workspace: &Path) -> Detection {
    let ws_str = workspace.to_string_lossy().to_string();
    let prior = load_context(workspace);
    let was_established = prior.as_ref().map(|c| c.confirmed).unwrap_or(false);

    let is_git_repo = run_capture(
        "git",
        &["-C", &ws_str, "rev-parse", "--is-inside-work-tree"],
        workspace,
        5,
    )
    .map(|s| s.trim() == "true")
    .unwrap_or(false);

    let remote_out = run_capture(
        "git",
        &["-C", &ws_str, "remote", "get-url", "origin"],
        workspace,
        5,
    );

    // Only spend a network call on `gh` when the remote parses to a GitHub slug.
    let is_github = remote_out
        .as_deref()
        .and_then(normalize_remote)
        .map(|s| s.is_github())
        .unwrap_or(false);
    let gh_out = if is_github {
        run_capture(
            "gh",
            &["repo", "view", "--json", "visibility", "-q", ".visibility"],
            workspace,
            6,
        )
    } else {
        None
    };

    let mut ctx = assemble_context(
        &ws_str,
        remote_out.as_deref(),
        gh_out.as_deref(),
        prior.as_ref(),
    );
    ctx.updated_at = now_secs();
    Detection {
        ctx,
        was_established,
        is_git_repo,
    }
}

/// One-line detection caveat for the human-facing summary.
fn detection_note(d: &Detection) -> &'static str {
    if d.ctx.repo.is_none() {
        if d.is_git_repo {
            "No `origin` remote — repo unknown. Ask the user for the GitHub repo (or confirm it's local-only)."
        } else {
            "Not a git repo — folder-only context. Confirm with the user whether this work is internal-only."
        }
    } else if d.ctx.visibility == Visibility::Unknown {
        "Couldn't determine visibility (gh missing/unauthed). Ask the user: is this repo private or public?"
    } else {
        "Visibility confirmed from GitHub via gh."
    }
}

/// The structured summary the detect path returns so the agent can confirm + record.
fn detect_summary(d: &Detection, saved: &Path) -> String {
    format!(
        "Work landing — detected context (a PROPOSAL; confirm with the user before relying on it).\n\n\
         folder:     {folder}\n\
         repo:       {repo}\n\
         visibility: {vis}\n\
         mode:       {mode} (proposed)\n\
         already established: {est}\n\n\
         note: {note}\n\n\
         Next: CONFIRM with the user — e.g. \"Working in {folder} · GitHub {repo} · {vis} — correct?\". \
         Once they agree, call `work_landing` with confirm=true (and repo / visibility / mode overrides \
         if they corrected anything) to record it; that sets internal-dev vs public-facing-care mode.\n\
         Snapshot saved: {saved}",
        folder = d.ctx.folder,
        repo = d.ctx.repo_label(),
        vis = d.ctx.visibility.label(),
        mode = d.ctx.mode.label(),
        est = if d.was_established {
            "yes (previously confirmed)"
        } else {
            "no"
        },
        note = detection_note(d),
        saved = saved.display(),
    )
}

/// The summary returned once the context is confirmed + recorded.
fn confirm_summary(ctx: &WorkContext, saved: &Path) -> String {
    let guidance = if ctx.mode == Mode::Unset {
        "Mode is still unset — visibility is unknown; ask the user to pin it (private→internal-dev, public→public-facing)."
    } else {
        ctx.mode.guidance()
    };
    format!(
        "Work landing — context CONFIRMED and recorded.\n\n\
         folder:     {folder}\n\
         repo:       {repo}\n\
         visibility: {vis}\n\
         mode:       {mode}\n\n\
         {guidance}\n\n\
         Persisted to {saved}. This now drives your behavior mode for this workspace.",
        folder = ctx.folder,
        repo = ctx.repo_label(),
        vis = ctx.visibility.label(),
        mode = ctx.mode.label(),
        guidance = guidance,
        saved = saved.display(),
    )
}

/// `work_landing` — establish the agent's work context for this workspace.
pub struct WorkLandingTool {
    workspace: PathBuf,
}

impl WorkLandingTool {
    pub fn new(workspace: PathBuf) -> Self {
        WorkLandingTool { workspace }
    }
}

impl Tool for WorkLandingTool {
    fn name(&self) -> &str {
        "work_landing"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "work_landing".to_string(),
            description: "Establish your WORK CONTEXT for this workspace at the start of a \
                          conversation. No args: DETECT the folder, the GitHub repo (git \
                          remote origin), and visibility (private/public via gh) — gracefully, \
                          returning a proposed behavior mode (internal-dev for private/local, \
                          public-facing-care for a public repo). Then ASK the user to confirm. \
                          Call again with confirm=true (plus repo / visibility / mode overrides \
                          if they corrected anything) to RECORD the confirmed context, which is \
                          persisted and drives your behavior mode for this workspace."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "confirm": {
                        "type": "boolean",
                        "description": "record the user's confirmation (sets the context as established)"
                    },
                    "repo": {
                        "type": "string",
                        "description": "owner/repo override when confirming (if detection missed/misread it)"
                    },
                    "visibility": {
                        "type": "string",
                        "enum": ["public", "private", "unknown"],
                        "description": "visibility override when confirming"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["internal-dev", "public-facing", "unset"],
                        "description": "behavior-mode override when confirming (else inferred from visibility)"
                    }
                }
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        crate::harness::hardlink_result(|| {
            let confirm = args
                .get("confirm")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !confirm {
                let d = detect(&self.workspace);
                let saved = save_context(&self.workspace, &d.ctx)
                    .map_err(|e| format!("persist work-context: {e}"))?;
                return Ok(detect_summary(&d, &saved));
            }

            // Confirm path: build on the last detected snapshot (or detect fresh), apply
            // any user corrections, mark established, persist.
            let mut ctx =
                load_context(&self.workspace).unwrap_or_else(|| detect(&self.workspace).ctx);

            if let Some(r) = args.get("repo").and_then(Value::as_str) {
                let r = r.trim();
                ctx.repo = if r.is_empty() {
                    None
                } else {
                    Some(r.to_string())
                };
            }
            let mut vis_overridden = false;
            if let Some(v) = args.get("visibility").and_then(Value::as_str)
                && let Some(vis) = Visibility::parse_label(v)
            {
                ctx.visibility = vis;
                vis_overridden = true;
            }
            if let Some(m) = args.get("mode").and_then(Value::as_str) {
                if let Some(mode) = Mode::parse_label(m) {
                    ctx.mode = mode;
                }
            } else if vis_overridden {
                // No explicit mode but visibility changed — re-infer from the new value.
                ctx.mode = infer_mode(ctx.visibility, ctx.repo.is_some());
            }

            ctx.confirmed = true;
            ctx.updated_at = now_secs();
            let saved = save_context(&self.workspace, &ctx)
                .map_err(|e| format!("persist work-context: {e}"))?;
            Ok(confirm_summary(&ctx, &saved))
        })
    }
}

// ---------------------------------------------------------------------------
// Tests — the pure functions, exercised with canned git/gh strings + a tempdir.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn live_detection_capture_uses_the_in_process_deadline() {
        let started = std::time::Instant::now();
        assert!(run_capture("sh", &["-c", "sleep 30"], Path::new("."), 0).is_none());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "work landing must not fall back to an unbounded child"
        );
    }

    /// A unique throwaway dir under the system temp dir (no tempfile dep, no env).
    fn temp_dir(tag: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("angel-worklanding-{tag}-{pid}-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    // --- git remote normalization (ssh + https → owner/repo). ---

    #[test]
    fn normalize_remote_ssh_scp_form() {
        let s = normalize_remote("git@github.com:newjordan/angel0.git").unwrap();
        assert_eq!(s.slug(), "newjordan/angel0");
        assert_eq!(s.host, "github.com");
        assert!(s.is_github());
    }

    #[test]
    fn normalize_remote_https_form_with_and_without_dot_git() {
        assert_eq!(
            normalize_remote("https://github.com/newjordan/angel0.git")
                .unwrap()
                .slug(),
            "newjordan/angel0"
        );
        assert_eq!(
            normalize_remote("https://github.com/newjordan/angel0")
                .unwrap()
                .slug(),
            "newjordan/angel0"
        );
    }

    #[test]
    fn normalize_remote_ssh_url_form_and_trailing_slash() {
        assert_eq!(
            normalize_remote("ssh://git@github.com/acme/Tool.git")
                .unwrap()
                .slug(),
            "acme/Tool"
        );
        assert_eq!(
            normalize_remote("https://github.com/acme/Tool/")
                .unwrap()
                .slug(),
            "acme/Tool"
        );
    }

    #[test]
    fn normalize_remote_enterprise_host_is_not_github() {
        let s = normalize_remote("git@git.corp.internal:team/svc.git").unwrap();
        assert_eq!(s.slug(), "team/svc");
        assert_eq!(s.host, "git.corp.internal");
        assert!(!s.is_github());
    }

    #[test]
    fn normalize_remote_rejects_garbage_and_local_paths() {
        assert!(normalize_remote("").is_none());
        assert!(normalize_remote("   ").is_none());
        assert!(normalize_remote("not a url").is_none());
        assert!(normalize_remote("/srv/git/repo.git").is_none()); // local path, no host
        assert!(normalize_remote("https://github.com/only-owner").is_none()); // no repo segment
    }

    // --- gh visibility parsing. ---

    #[test]
    fn parse_gh_visibility_maps_known_values() {
        assert_eq!(parse_gh_visibility("PUBLIC\n"), Visibility::Public);
        assert_eq!(parse_gh_visibility("private"), Visibility::Private);
        assert_eq!(parse_gh_visibility("INTERNAL"), Visibility::Private);
        assert_eq!(parse_gh_visibility(""), Visibility::Unknown);
        assert_eq!(parse_gh_visibility("huh?"), Visibility::Unknown);
    }

    // --- visibility → mode inference. ---

    #[test]
    fn infer_mode_covers_all_cases() {
        assert_eq!(infer_mode(Visibility::Public, true), Mode::PublicFacing);
        assert_eq!(infer_mode(Visibility::Private, true), Mode::InternalDev);
        // unknown but a remote exists → ask (unset)
        assert_eq!(infer_mode(Visibility::Unknown, true), Mode::Unset);
        // unknown and purely local → internal-dev (nothing is exposed)
        assert_eq!(infer_mode(Visibility::Unknown, false), Mode::InternalDev);
    }

    // --- assemble_context graceful tiers (mock git/gh as strings). ---

    #[test]
    fn assemble_public_repo_is_public_facing() {
        let ctx = assemble_context(
            "/w",
            Some("git@github.com:org/app.git"),
            Some("PUBLIC"),
            None,
        );
        assert_eq!(ctx.repo.as_deref(), Some("org/app"));
        assert_eq!(ctx.visibility, Visibility::Public);
        assert_eq!(ctx.mode, Mode::PublicFacing);
        assert!(!ctx.confirmed);
    }

    #[test]
    fn assemble_private_repo_is_internal_dev() {
        let ctx = assemble_context(
            "/w",
            Some("https://github.com/org/secret.git"),
            Some("PRIVATE"),
            None,
        );
        assert_eq!(ctx.visibility, Visibility::Private);
        assert_eq!(ctx.mode, Mode::InternalDev);
    }

    #[test]
    fn assemble_missing_gh_with_remote_is_unset() {
        // Remote exists but gh is unavailable → can't classify → ask.
        let ctx = assemble_context("/w", Some("git@github.com:org/app.git"), None, None);
        assert_eq!(ctx.repo.as_deref(), Some("org/app"));
        assert_eq!(ctx.visibility, Visibility::Unknown);
        assert_eq!(ctx.mode, Mode::Unset);
    }

    #[test]
    fn assemble_no_remote_is_local_internal_dev() {
        // No remote at all (not a repo / no origin) → folder-only, local internal-dev.
        let ctx = assemble_context("/w", None, None, None);
        assert!(ctx.repo.is_none());
        assert_eq!(ctx.visibility, Visibility::Unknown);
        assert_eq!(ctx.mode, Mode::InternalDev);
    }

    #[test]
    fn assemble_non_github_remote_skips_visibility_and_asks() {
        // A local-path remote doesn't parse to a slug → repo unknown, but a remote
        // string is present → mode unset (we shouldn't assume internal).
        let ctx = assemble_context("/w", Some("/srv/git/repo.git"), None, None);
        assert!(ctx.repo.is_none());
        assert_eq!(ctx.mode, Mode::Unset);
    }

    #[test]
    fn assemble_preserves_confirmed_prior_when_unchanged() {
        let prior = WorkContext {
            folder: "/old".into(),
            repo: Some("org/app".into()),
            visibility: Visibility::Public,
            mode: Mode::PublicFacing,
            confirmed: true,
            updated_at: 123,
        };
        let ctx = assemble_context(
            "/w",
            Some("git@github.com:org/app.git"),
            Some("PUBLIC"),
            Some(&prior),
        );
        assert!(
            ctx.confirmed,
            "unchanged repo+visibility keeps confirmation"
        );
        assert_eq!(ctx.mode, Mode::PublicFacing);
        assert_eq!(ctx.folder, "/w", "folder still refreshed to the live path");
    }

    #[test]
    fn assemble_resets_confirmation_when_repo_changes() {
        let prior = WorkContext {
            folder: "/old".into(),
            repo: Some("org/old".into()),
            visibility: Visibility::Public,
            mode: Mode::PublicFacing,
            confirmed: true,
            updated_at: 1,
        };
        // /cd into a different repo → must re-land.
        let ctx = assemble_context(
            "/w",
            Some("git@github.com:org/new.git"),
            Some("PRIVATE"),
            Some(&prior),
        );
        assert!(!ctx.confirmed, "a different repo re-triggers onboarding");
        assert_eq!(ctx.repo.as_deref(), Some("org/new"));
        assert_eq!(ctx.mode, Mode::InternalDev);
    }

    // --- persistence roundtrip (explicit dir → no env mutation). ---

    #[test]
    fn save_load_roundtrip() {
        let dir = temp_dir("roundtrip");
        let ws = Path::new("/home/u/projects/widget");
        let ctx = WorkContext {
            folder: ws.to_string_lossy().to_string(),
            repo: Some("u/widget".into()),
            visibility: Visibility::Private,
            mode: Mode::InternalDev,
            confirmed: true,
            updated_at: 42,
        };
        let path = save_context_in(&dir, ws, &ctx).expect("save");
        assert!(path.exists());
        let loaded = load_context_in(&dir, ws).expect("load");
        assert_eq!(loaded, ctx);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_context_is_none() {
        let dir = temp_dir("missing");
        assert!(load_context_in(&dir, Path::new("/no/such/ws")).is_none());
    }

    #[test]
    fn workspace_key_is_deterministic_and_distinct() {
        let a = workspace_key(Path::new("/home/u/alpha"));
        let a2 = workspace_key(Path::new("/home/u/alpha"));
        let b = workspace_key(Path::new("/home/u/beta"));
        assert_eq!(a, a2, "same path → same key (stable file)");
        assert_ne!(a, b, "different paths → different keys (no collision)");
        assert!(!a.contains('/'), "key is filesystem-safe");
    }

    // --- system-prompt block rendering. ---

    #[test]
    fn render_block_active_for_confirmed_context() {
        let ctx = WorkContext {
            folder: "/w".into(),
            repo: Some("org/app".into()),
            visibility: Visibility::Public,
            mode: Mode::PublicFacing,
            confirmed: true,
            updated_at: 0,
        };
        let block = render_block(Some(&ctx));
        assert!(block.contains("Active work context"));
        assert!(block.contains("mode=public-facing"));
        assert!(
            block.contains("NEVER commit secrets"),
            "injects mode guidance"
        );
    }

    #[test]
    fn render_block_internal_dev_guidance() {
        let ctx = WorkContext {
            folder: "/w".into(),
            repo: None,
            visibility: Visibility::Private,
            mode: Mode::InternalDev,
            confirmed: true,
            updated_at: 0,
        };
        let block = render_block(Some(&ctx));
        assert!(block.contains("mode=internal-dev"));
        assert!(block.contains("velocity"), "internal-dev guidance present");
    }

    #[test]
    fn render_block_not_established_for_none() {
        let block = render_block(None);
        assert!(block.contains("not established"));
        assert!(
            block.contains("work_landing"),
            "tells the agent to self-direct"
        );
        assert!(block.contains("confirm=true"));
    }

    #[test]
    fn render_block_unconfirmed_shows_detected_hint() {
        let ctx = WorkContext {
            folder: "/w".into(),
            repo: Some("org/app".into()),
            visibility: Visibility::Unknown,
            mode: Mode::Unset,
            confirmed: false,
            updated_at: 0,
        };
        let block = render_block(Some(&ctx));
        assert!(block.contains("not established"));
        assert!(block.contains("Detected so far"));
        assert!(block.contains("org/app"));
    }

    #[test]
    fn mode_and_visibility_label_parse_roundtrip() {
        assert_eq!(Mode::parse_label("public-facing"), Some(Mode::PublicFacing));
        assert_eq!(Mode::parse_label("internal_dev"), Some(Mode::InternalDev));
        assert_eq!(Mode::parse_label("nonsense"), None);
        assert_eq!(
            Visibility::parse_label("Private"),
            Some(Visibility::Private)
        );
        assert_eq!(
            Visibility::parse_label("internal"),
            Some(Visibility::Private)
        );
        assert_eq!(Visibility::parse_label("weird"), None);
    }
}

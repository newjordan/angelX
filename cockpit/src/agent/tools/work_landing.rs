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
//! `~/.angelX/work-context/<key>.json` (override the dir with
//! `ANGEL_WORK_CONTEXT_DIR`), so the mode survives restarts and a `/cd` to a new
//! repo lands a fresh (unconfirmed) context that re-triggers onboarding.
//!
//! The detection / normalization / mode-inference / persistence logic is factored
//! into PURE functions ([`normalize_remote`], [`parse_gh_visibility`],
//! [`infer_mode`], [`assemble_context`], [`save_context_in`]/[`load_context_in`])
//! that take the git/gh outputs as plain strings, so they are headless-testable
//! without spawning a single subprocess.

use crate::agent::club::ToolDef;
use crate::agent::harness::{Tool, env_flag};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
    Unknown,
}

impl Visibility {
    pub fn label(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Unknown => "unknown",
        }
    }

    pub fn parse_label(s: &str) -> Option<Visibility> {
        match s.trim().to_ascii_lowercase().as_str() {
            "public" | "pub" => Some(Visibility::Public),
            "private" | "priv" | "internal" => Some(Visibility::Private),
            "unknown" | "?" | "" => Some(Visibility::Unknown),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "internal-dev")]
    InternalDev,
    #[serde(rename = "public-facing")]
    PublicFacing,
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

    pub fn guidance(self) -> String {
        use crate::agent::harness::book::d46_recovery::{Page, run};
        use crate::agent::harness::book::n_environment::{WORK_INTERNAL, WORK_PUBLIC};
        let (route, pages) = match self {
            Mode::InternalDev => (WORK_INTERNAL, 3..=5),
            Mode::PublicFacing => (WORK_PUBLIC, 3..=6),
            Mode::Unset => return String::new(),
        };
        run(&pages.map(|n| Page::new(route, n)).collect::<Vec<_>>())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkContext {
    pub folder: String,
    pub repo: Option<String>,
    pub visibility: Visibility,
    pub mode: Mode,
    pub confirmed: bool,
    #[serde(default)]
    pub updated_at: u64,
}

impl WorkContext {
    fn repo_label(&self) -> String {
        self.repo
            .clone()
            .unwrap_or_else(|| "(none — no GitHub remote)".to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoSlug {
    pub host: String,
    pub owner: String,
    pub repo: String,
}

impl RepoSlug {
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    pub fn is_github(&self) -> bool {
        self.host == "github.com" || self.host.ends_with(".github.com")
    }
}

pub fn normalize_remote(raw: &str) -> Option<RepoSlug> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let (host, path) = if let Some(idx) = s.find("://") {
        let rest = &s[idx + 3..];
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = authority.rsplit('@').next().unwrap_or(authority);
        let host = host.split(':').next().unwrap_or(host);
        (host.to_string(), path.to_string())
    } else {
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

fn scp_split(s: &str) -> Option<(String, String)> {
    let colon = s.find(':')?;
    let authority = &s[..colon];
    let path = &s[colon + 1..];
    if authority.is_empty() || path.is_empty() {
        return None;
    }
    let host = authority.rsplit('@').next().unwrap_or(authority);
    if !host.contains('.') {
        return None;
    }
    Some((host.to_string(), path.to_string()))
}

pub fn parse_gh_visibility(out: &str) -> Visibility {
    match out.trim().to_ascii_uppercase().as_str() {
        "PUBLIC" => Visibility::Public,
        "PRIVATE" => Visibility::Private,
        "INTERNAL" => Visibility::Private,
        _ => Visibility::Unknown,
    }
}

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

pub fn context_base_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ANGEL_WORK_CONTEXT_DIR") {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".angelX/work-context")
}

pub fn workspace_key(workspace: &Path) -> String {
    crate::platform::workspace_store::workspace_key(workspace)
}

pub fn context_path_in(dir: &Path, workspace: &Path) -> PathBuf {
    dir.join(format!("{}.json", workspace_key(workspace)))
}

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
    crate::agent::harness::confined_write(dir, relative, json.as_bytes())
        .map_err(std::io::Error::other)?;
    Ok(path)
}

pub fn load_context_in(dir: &Path, workspace: &Path) -> Option<WorkContext> {
    let path = context_path_in(dir, workspace);
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_context(workspace: &Path, ctx: &WorkContext) -> std::io::Result<PathBuf> {
    save_context_in(&context_base_dir(), workspace, ctx)
}

pub fn load_context(workspace: &Path) -> Option<WorkContext> {
    load_context_in(&context_base_dir(), workspace)
}

pub fn render_block(ctx: Option<&WorkContext>) -> String {
    use crate::agent::harness::book::n_environment::{WORK_INTERNAL, WORK_PUBLIC, WORK_UNSET};
    match ctx {
        Some(c) if c.confirmed && c.mode != Mode::Unset => format!(
            "\n\n{route}\nfolder={folder} · repo={repo} · visibility={vis} · mode={mode}\n",
            route = if c.mode == Mode::PublicFacing {
                WORK_PUBLIC.cells()
            } else {
                WORK_INTERNAL.cells()
            },
            folder = c.folder,
            repo = c.repo_label(),
            vis = c.visibility.label(),
            mode = c.mode.label(),
        ),
        other => {
            let mut s = format!("\n\n{}\n", WORK_UNSET.cells());
            if let Some(c) = other {
                s.push_str(&format!(
                    "folder={folder} · repo={repo} · visibility={vis} · proposed mode={mode}\n",
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

pub fn work_context_block(workspace: &Path) -> String {
    if !env_flag("ANGEL_WORK_LANDING", true) {
        return String::new();
    }
    render_block(load_context(workspace).as_ref())
}

fn run_capture(program: &str, args: &[&str], cwd: &Path, secs: u64) -> Option<String> {
    crate::platform::workspace_store::capture_repo_probe(program, args, cwd, secs)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

struct Detection {
    ctx: WorkContext,
    was_established: bool,
    is_git_repo: bool,
}

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

fn detection_note(d: &Detection) -> String {
    use crate::agent::harness::book::d56_replies as replies;
    let (fact, ask) = if d.ctx.repo.is_none() {
        if d.is_git_repo {
            (
                "No `origin` remote — repo unknown.",
                replies::LANDING_ASK_REPO,
            )
        } else {
            (
                "Not a git repo — folder-only context.",
                replies::LANDING_ASK_INTERNAL,
            )
        }
    } else if d.ctx.visibility == Visibility::Unknown {
        (
            "Couldn't determine visibility (gh missing/unauthed).",
            replies::LANDING_ASK_VISIBILITY,
        )
    } else {
        return "Visibility confirmed from GitHub via gh.".to_string();
    };
    format!("{fact} {}", ask.cells())
}

fn detect_summary(d: &Detection, saved: &Path) -> String {
    use crate::agent::harness::book::d56_replies as replies;
    format!(
        "{detected}\n\n\
         folder:     {folder}\n\
         repo:       {repo}\n\
         visibility: {vis}\n\
         mode:       {mode} (proposed)\n\
         already established: {est}\n\n\
         note: {note}\n\n\
         {next}\n\
         Snapshot saved: {saved}",
        detected = replies::LANDING_DETECTED.cells(),
        next = crate::agent::harness::book::d46_recovery::run(&[
            replies::LANDING_CONFIRM,
            replies::LANDING_RECORD,
        ]),
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

fn confirm_summary(ctx: &WorkContext, saved: &Path) -> String {
    use crate::agent::harness::book::d56_replies as replies;
    let guidance = if ctx.mode == Mode::Unset {
        format!(
            "Mode is still unset — visibility is unknown\n{}",
            replies::LANDING_PIN_MODE.cells()
        )
    } else {
        ctx.mode.guidance()
    };
    format!(
        "{confirmed}\n\n\
         folder:     {folder}\n\
         repo:       {repo}\n\
         visibility: {vis}\n\
         mode:       {mode}\n\n\
         {guidance}\n\n\
         Persisted to {saved}.\n{drives}",
        confirmed = replies::LANDING_CONFIRMED.cells(),
        drives = replies::LANDING_DRIVES.cells(),
        folder = ctx.folder,
        repo = ctx.repo_label(),
        vis = ctx.visibility.label(),
        mode = ctx.mode.label(),
        guidance = guidance,
        saved = saved.display(),
    )
}

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
        crate::agent::harness::core_catalog::baseline_def("work_landing")
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        crate::agent::harness::hardlink_result(|| {
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

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/work_landing__tests.rs"]
mod tests;

//! Self-repair tools for broken command surfaces.
//!
//! This is intentionally tool-agnostic: the model should ask for a repair of the
//! failing command surface, not memorize one-off fixes like "GitHub auth". The
//! first built-in profile handles `gh` because snap launcher + auth config was
//! the live failure, but the shell/path/snap repair primitive is generic. This is
//! break-fix machinery, not a startup health-check ritual.

use crate::club::ToolDef;
use crate::harness::{Tool, output_timed_fixed_captured};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const REPAIR_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct ToolRepairTool {
    pub(crate) workspace: PathBuf,
    /// Repair invocations so far this session. The session budget
    /// (`ANGEL_TOOL_REPAIR_LIMIT`, default 2) counts against this so a drifted
    /// model can't hammer the tool — which mutates `~/.local/bin` and global git
    /// config — unbounded.
    calls: AtomicU32,
}

impl ToolRepairTool {
    pub(crate) fn new(workspace: PathBuf) -> Self {
        Self {
            workspace,
            calls: AtomicU32::new(0),
        }
    }
}

impl Tool for ToolRepairTool {
    fn workspace_write_scope_is_opaque(&self, _args: &Value) -> bool {
        // Even diagnosis invokes external CLI probes without a read-only
        // filesystem ceiling. repair=false is intent, not a write audit.
        true
    }

    fn name(&self) -> &str {
        "tool_repair"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "tool_repair".to_string(),
            description: "Diagnose a broken CLI surface (gh, git auth, PATH, snap launchers). \
                          Call only when the user has explicitly asked to investigate or fix a \
                          broken tool in this session. Diagnose-only by default; pass repair=true \
                          only after the user approves making changes. It checks PATH resolution, \
                          detects snap launchers that fail in sandboxed agents, links usable \
                          payload binaries into ~/.local/bin, and runs profile probes. Built-in \
                          profiles: auto, gh."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "tool": {
                        "type": "string",
                        "description": "command/profile to repair, e.g. auto or gh (default auto)"
                    },
                    "repair": {
                        "type": "boolean",
                        "description": "apply local repairs (mutates ~/.local/bin symlinks and global git config); default false — diagnose only. Set true ONLY after the user approves making changes."
                    }
                },
                "required": []
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        // Session call budget: even once the user has approved a repair, cap the
        // number of attempts so a drifted model can't loop on it. Default 2;
        // ANGEL_TOOL_REPAIR_LIMIT=0 makes it unlimited. Counted per session (per
        // tool instance), mirroring the ANGEL_ERROR_LIMIT env style.
        let limit = if crate::yolo::enabled() {
            0
        } else {
            crate::harness::env_usize("ANGEL_TOOL_REPAIR_LIMIT", 2)
        };
        if limit > 0 && self.calls.fetch_add(1, Ordering::Relaxed) >= limit as u32 {
            return Err(
                "tool_repair session budget exhausted; ask the user before further repair attempts"
                    .to_string(),
            );
        }
        let tool = args.get("tool").and_then(|v| v.as_str()).unwrap_or("auto");
        // Diagnose-only by default — mutations require an explicit repair=true, which
        // the description gates on user approval.
        let repair = args
            .get("repair")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(tool_repair_report(&self.workspace, tool, repair))
    }
}

fn tool_repair_report(workspace: &Path, tool: &str, repair: bool) -> String {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let normalized = normalize_tool(tool);
    let mut out = Vec::new();
    out.push(format!("tool repair diagnosis · profile={normalized}"));
    match normalized.as_str() {
        "auto" => {
            out.extend(generic_command_report("gh", &home, repair));
            out.extend(gh_profile_report(workspace, &home, repair));
        }
        "gh" | "github" => {
            out.extend(generic_command_report("gh", &home, repair));
            out.extend(gh_profile_report(workspace, &home, repair));
        }
        cmd => out.extend(generic_command_report(cmd, &home, repair)),
    }
    out.join("\n")
}

fn normalize_tool(tool: &str) -> String {
    let t = tool.trim().trim_start_matches('/').to_ascii_lowercase();
    if t.is_empty() { "auto".to_string() } else { t }
}

fn generic_command_report(command: &str, home: &Path, repair: bool) -> Vec<String> {
    let mut out = Vec::new();
    let path_cmd = find_on_path(command);
    out.push(format!(
        "{command} on PATH: {}",
        path_cmd
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not found)".to_string())
    ));
    if path_cmd.as_deref().is_some_and(is_snap_launcher) {
        out.push(format!(
            "detected snap {command} launcher; sandboxed agents may fail at snap-confine"
        ));
    }

    let payload = find_snap_payload(command);
    if let Some(p) = &payload {
        out.push(format!("snap {command} payload: {}", p.display()));
    }

    if repair {
        if let Some(payload) = payload.as_deref() {
            out.extend(link_payload_repair(home, command, payload));
        } else {
            out.push(format!("repair: no snap payload found for {command}"));
        }
    } else {
        out.push("repair: skipped (repair=false)".to_string());
    }
    out
}

fn gh_profile_report(workspace: &Path, home: &Path, repair: bool) -> Vec<String> {
    let mut out = Vec::new();
    let config = find_gh_config_dir(home);
    if let Some(c) = &config {
        out.push(format!("gh auth config: {}", c.display()));
    } else {
        out.push("gh auth config: (not found)".to_string());
    }

    if repair {
        if let Some(config) = config.as_deref() {
            out.extend(link_gh_config_repair(home, config));
        }
        out.extend(configure_github_git_helper());
    }

    let gh = home
        .join(".local/bin/gh")
        .is_file()
        .then(|| home.join(".local/bin/gh"))
        .or_else(|| find_snap_payload("gh"))
        .or_else(|| find_on_path("gh"));
    match gh
        .as_ref()
        .map(|p| command_text(p, &["auth", "status"], home))
    {
        Some(Ok(text)) => out.push(format!(
            "gh auth status: ok\n{}",
            indent(&mask_token(&text))
        )),
        Some(Err(e)) => out.push(format!("gh auth status: failed\n{}", indent(&e))),
        None => out.push("gh auth status: failed (no gh executable found)".to_string()),
    }

    match git_credential_smoke(workspace, home) {
        Ok(()) => out.push(
            "git credential helper: ok (username/password returned; token hidden)".to_string(),
        ),
        Err(e) => out.push(format!("git credential helper: failed\n{}", indent(&e))),
    }
    out
}

fn link_payload_repair(home: &Path, command: &str, payload: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let local_bin = home.join(".local/bin");
    let target = local_bin.join(command);
    match std::fs::create_dir_all(&local_bin) {
        Ok(()) => match replace_symlink(payload, &target) {
            Ok(()) => out.push(format!(
                "repair: {} -> {}",
                target.display(),
                payload.display()
            )),
            Err(e) => out.push(format!("repair: failed to link {}: {e}", target.display())),
        },
        Err(e) => out.push(format!(
            "repair: failed to create {}: {e}",
            local_bin.display()
        )),
    }
    out
}

fn link_gh_config_repair(home: &Path, config: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let standard = home.join(".config/gh");
    if !standard.exists() {
        if let Some(parent) = standard.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            out.push(format!(
                "repair: failed to create {}: {e}",
                parent.display()
            ));
        }
        match replace_symlink(config, &standard) {
            Ok(()) => out.push(format!(
                "repair: {} -> {}",
                standard.display(),
                config.display()
            )),
            Err(e) => out.push(format!(
                "repair: failed to link {}: {e}",
                standard.display()
            )),
        }
    } else {
        out.push(format!("repair: {} already exists", standard.display()));
    }
    out
}

fn configure_github_git_helper() -> Vec<String> {
    let mut command = Command::new("git");
    command.args([
        "config",
        "--global",
        "credential.https://github.com.helper",
        "!gh auth git-credential",
    ]);
    match bounded_repair_output(command, REPAIR_PROBE_TIMEOUT) {
        Ok(o) if o.status.success() => {
            vec!["repair: git credential helper set for https://github.com".to_string()]
        }
        Ok(o) => vec![format!(
            "repair: git credential helper failed\n{}",
            String::from_utf8_lossy(&o.stderr).trim()
        )],
        Err(e) => vec![format!("repair: git config failed: {e}")],
    }
}

fn replace_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(link) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::remove_file(link)?,
        Ok(_) => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    std::os::unix::fs::symlink(target, link)
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn is_snap_launcher(path: &Path) -> bool {
    path.starts_with("/snap/bin")
        || std::fs::read_link(path).is_ok_and(|target| target == Path::new("/usr/bin/snap"))
}

fn find_snap_payload(command: &str) -> Option<PathBuf> {
    for candidate in [
        PathBuf::from(format!("/snap/{command}/current/{command}")),
        PathBuf::from(format!("/snap/{command}/640/{command}")),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let dirs = std::fs::read_dir(format!("/snap/{command}")).ok()?;
    let mut candidates: Vec<PathBuf> = dirs
        .flatten()
        .map(|e| e.path().join(command))
        .filter(|p| p.is_file())
        .collect();
    candidates.sort();
    candidates.pop()
}

fn find_gh_config_dir(home: &Path) -> Option<PathBuf> {
    [
        home.join(".config/gh"),
        home.join("snap/gh/current/.config/gh"),
        home.join("snap/gh/640/.config/gh"),
    ]
    .into_iter()
    .find(|candidate| candidate.join("hosts.yml").is_file())
}

fn command_text(program: &Path, args: &[&str], home: &Path) -> Result<String, String> {
    command_text_with_timeout(program, args, home, REPAIR_PROBE_TIMEOUT)
}

fn command_text_with_timeout(
    program: &Path,
    args: &[&str],
    home: &Path,
    timeout: Duration,
) -> Result<String, String> {
    let mut command = Command::new(program);
    command.args(args).env("PATH", repaired_path(home));
    let output = bounded_repair_output(command, timeout)
        .map_err(|e| format!("{}: {e}", program.display()))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(stderr.trim());
    }
    if output.status.success() {
        Ok(text.trim().to_string())
    } else {
        Err(text.trim().to_string())
    }
}

fn git_credential_smoke(workspace: &Path, home: &Path) -> Result<(), String> {
    // The request is fixed source text rather than model input. A shell pipe is
    // used solely to feed Git because the shared fixed probe deliberately owns
    // stdin as null; the private process group still contains Git and any
    // configured credential-helper descendants.
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "printf 'protocol=https\\nhost=github.com\\n\\n' | git credential fill",
        ])
        .current_dir(workspace)
        .env("PATH", repaired_path(home))
        .env("GIT_TERMINAL_PROMPT", "0");
    let output = bounded_repair_output(command, REPAIR_PROBE_TIMEOUT)
        .map_err(|e| format!("git credential fill: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success()
        && stdout.lines().any(|l| l.starts_with("username="))
        && stdout.lines().any(|l| l.starts_with("password="))
    {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(mask_token(
            format!("{}\n{}", stdout.trim(), stderr.trim()).trim(),
        ))
    }
}

fn bounded_repair_output(
    command: Command,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    let captured = output_timed_fixed_captured(command, timeout)?;
    if captured.timed_out {
        return Err(format!("timed out after {}ms", timeout.as_millis()));
    }
    if captured.stdout_truncated || captured.stderr_truncated {
        return Err("output exceeded the bounded diagnostic capture".to_string());
    }
    Ok(captured.output)
}

fn repaired_path(home: &Path) -> std::ffi::OsString {
    let current = std::env::var_os("PATH").unwrap_or_default();
    repaired_path_from(home, current)
}

fn repaired_path_from(home: &Path, current: std::ffi::OsString) -> std::ffi::OsString {
    let mut paths = vec![home.join(".local/bin"), home.join("bin")];
    paths.extend(std::env::split_paths(&current));
    std::env::join_paths(paths).unwrap_or(current)
}

fn mask_token(text: impl AsRef<str>) -> String {
    text.as_ref()
        .lines()
        .map(|line| {
            if line.contains("password=") || line.contains("Token:") {
                line.split_once('=')
                    .map(|(k, _)| format!("{k}=<hidden>"))
                    .unwrap_or_else(|| "  - Token: <hidden>".to_string())
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_masking_hides_command_credentials() {
        let masked = mask_token("username=newjordan\npassword=gho_secret\n  - Token: gho_secret");
        assert!(masked.contains("username=newjordan"));
        assert!(masked.contains("password=<hidden>"));
        assert!(masked.contains("Token: <hidden>"));
        assert!(!masked.contains("gho_secret"));
    }

    #[cfg(unix)]
    #[test]
    fn repair_command_text_has_a_fixed_hung_tree_deadline() {
        let started = std::time::Instant::now();
        let error = command_text_with_timeout(
            Path::new("sh"),
            &["-c", "sleep 30 & wait"],
            Path::new("/tmp"),
            Duration::from_millis(50),
        )
        .unwrap_err();

        assert!(error.contains("timed out after 50ms"), "{error}");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "repair probe did not clean up its hung tree: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn snap_launcher_detection_handles_real_snap_symlink_shape() {
        let dir = std::env::temp_dir().join(format!("angel_snap_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let gh = dir.join("gh");
        std::os::unix::fs::symlink("/usr/bin/snap", &gh).unwrap();
        assert!(is_snap_launcher(&gh));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repaired_path_puts_user_bins_first() {
        let home = std::env::temp_dir().join(format!("angel_home_{}", std::process::id()));
        let current =
            std::env::join_paths([Path::new("/snap/bin"), Path::new("/usr/bin")]).unwrap();
        let path = repaired_path_from(&home, current);
        let parts: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(parts.first(), Some(&home.join(".local/bin")));
        assert_eq!(parts.get(1), Some(&home.join("bin")));
        assert!(parts.contains(&PathBuf::from("/snap/bin")));
    }

    #[test]
    fn normalizes_empty_tool_to_auto() {
        assert_eq!(normalize_tool(""), "auto");
        assert_eq!(normalize_tool("/gh"), "gh");
    }

    #[test]
    fn default_call_is_diagnose_only() {
        // No repair arg → diagnose only: the generic report says repairs were
        // skipped, and no mutation branch runs.
        let tool = ToolRepairTool::new(std::env::temp_dir());
        let out = tool
            .call(&serde_json::json!({ "tool": "gh" }))
            .expect("diagnose runs");
        assert!(
            out.contains("repair: skipped (repair=false)"),
            "default must be diagnose-only:\n{out}"
        );
    }

    #[test]
    fn session_budget_stops_repeated_calls() {
        // Default limit is 2: calls 1 and 2 run, the 3rd is refused. Set the env
        // explicitly (save/restore) so an external value can't skew the count.
        let _guard = crate::tests::env_lock();
        let prev = std::env::var_os("ANGEL_TOOL_REPAIR_LIMIT");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TOOL_REPAIR_LIMIT", "2") };
        let tool = ToolRepairTool::new(std::env::temp_dir());
        let args = serde_json::json!({ "tool": "gh" });
        assert!(tool.call(&args).is_ok(), "call 1 within budget");
        assert!(tool.call(&args).is_ok(), "call 2 within budget");
        let third = tool.call(&args);
        match prev {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var("ANGEL_TOOL_REPAIR_LIMIT", v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var("ANGEL_TOOL_REPAIR_LIMIT") },
        }
        let err = third.expect_err("call 3 exhausts the session budget");
        assert!(
            err.contains("session budget exhausted"),
            "budget error text: {err}"
        );
    }
}

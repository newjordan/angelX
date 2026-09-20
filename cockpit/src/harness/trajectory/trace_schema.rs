//! Additive trace envelope. Unknown observations are null, never fabricated zeroes.
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

pub(super) fn defaults(record: &mut Value) {
    record["trace_schema"] = json!("angel-trace/v2");
    record["turn"] = json!({"operator_turn_id":null,"session":null,
        "ts":record["ts_ms"],"hop_count":record["hops"],"stop_reason":null});
    record["tools"] = json!([]);
    record["usage"] = Value::Null;
    record["timing"] = Value::Null;
    record["tools_output"] = Value::Null;
    record["store_rotations"] = json!([]);
    record["parking_events"] = json!([]);
    record["verifier"] = json!([]);
    record["artifacts"] = json!([]);
    record["lease"] = json!({"kind":null,"host":null,"seat":null,"lease_id":null});
    // Cost placeholders keep basis/source/model explicit so every record is
    // schema-valid before the real usage-based binding runs; the basis
    // explains the nulls rather than fabricating zeroes.
    record["cost"] = json!({"paid":null,"cached":null,"currency":null,"price_date":null,
        "basis":"unreported-usage","source":"no-usage-yet","model":null});
    record["workspace_state"] = unbound("unbound");
}

fn unbound(reason: &str) -> Value {
    json!({"head":reason,"tree_sha256":reason,"dirty_paths_sha256":reason,
        "head_tree":"unbound","start_tree_sha256":"unbound","start_dirty_paths_sha256":"unbound"})
}

/// Read the working tree, never mutate the Git index. Content digests are over
/// sorted length-delimited path/type/content-digest entries, including untracked
/// nonignored files. Symlinks hash link text and are never followed.
pub(crate) fn workspace_state(workspace: Option<&Path>) -> Value {
    workspace_state_inner(workspace, None)
}

/// Optional startup indexing only: an exhausted walk never supplies a complete
/// tree identity and never cancels a turn or its verifier.
pub(crate) fn startup_workspace_state(workspace: &Path) -> Value {
    let budget_ms = std::env::var("ANGEL_STARTUP_WALK_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(500);
    let started = Instant::now();
    let deadline = started
        .checked_add(Duration::from_millis(budget_ms))
        .unwrap_or(started);
    let mut state = workspace_state_inner(Some(workspace), Some(deadline));
    if state.get("startup_index").is_none() {
        state["startup_index"] = json!({"complete":false,
            "reason": if state["head"] == "not a git repo" { "not a git repo" }
                      else { "metadata unavailable, over capacity, or deadline exhausted" },
            "indexed_paths":0, "total_paths":null});
    }
    if state["startup_index"]["complete"] == false && state["head"] != "not a git repo" {
        eprintln!(
            "[startup-index] partial: {}; indexed {} paths (budget {} ms)",
            state["startup_index"]["reason"], state["startup_index"]["indexed_paths"], budget_ms
        );
    }
    state["startup_walk_ms"] = json!(started.elapsed().as_millis());
    state["startup_walk_budget_ms"] = json!(budget_ms);
    state
}

fn workspace_state_inner(workspace: Option<&Path>, deadline: Option<Instant>) -> Value {
    let Some(root) = workspace else {
        return unbound("unbound");
    };
    let probe_failed = std::cell::Cell::new(false);
    let git = |args: &[&str]| {
        let mut command = std::process::Command::new("git");
        command
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(root)
            .args(args);
        // P04b: the startup deadline bounds every probe; R04e: an unavailable probe
        // (timeout/overflow/io) is reported as partial evidence and the turn continues.
        if let Some(end) = deadline {
            // Startup index: the whole-tree deadline bounds the probe and the larger
            // capture allowance admits tracked path lists from big repositories.
            let remaining = end.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                probe_failed.set(true);
                return None;
            }
            return match crate::workspace_store::capture_startup_repo_command(command, remaining) {
                Ok(output) => output,
                Err(_) => {
                    probe_failed.set(true);
                    None
                }
            };
        }
        match crate::workspace_store::capture_optional_probe(
            command,
            std::time::Duration::from_millis(300),
        ) {
            Ok(output) => output,
            Err(error) => {
                probe_failed.set(true);
                eprintln!(
                    "[turn-phase-partial] step=workspace_identity status=partial io={:?} action=continue_with_unbound_identity",
                    error.kind()
                );
                None
            }
        }
    };
    if git(&["rev-parse", "--is-inside-work-tree"]).is_none() {
        // One reason string for every unavailable probe (deadline or not): the
        // partial notice and the receipts key on it.
        return unbound(if probe_failed.get() {
            "workspace identity probe unavailable"
        } else {
            "not a git repo"
        });
    }
    let Some(head) = git(&["rev-parse", "HEAD"]) else {
        // An unborn HEAD is a valid negative (fresh repository); only an
        // unavailable probe is reported as such.
        return unbound(if probe_failed.get() {
            "workspace identity probe unavailable"
        } else {
            "unbound"
        });
    };
    let head_tree = git(&["rev-parse", "HEAD^{tree}"])
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
        .unwrap_or_else(|| "unbound".to_owned());
    // The active workspace contract excludes quarantines at Git's pathspec layer.
    let paths = [".", ":(exclude)off-limits/**"];
    let Some(names) = git(&[
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "-z",
        "--",
        paths[0],
        paths[1],
    ]) else {
        return unbound("workspace identity probe unavailable");
    };
    let Some(dirty) = git(&[
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--",
        paths[0],
        paths[1],
    ]) else {
        return unbound("workspace identity probe unavailable");
    };
    let Some(tree) = hash_tree_with_deadline(root, &names, deadline) else {
        return unbound("unbound");
    };
    let mut state = json!({"head":String::from_utf8_lossy(&head).trim(),
        "head_tree":head_tree,
        "start_tree_sha256":"unbound","start_dirty_paths_sha256":"unbound",
        "tree_sha256": if tree.complete { tree.sha256.clone() } else { "partial-startup-index".into() },
        "dirty_paths_sha256":fast_sha256(&dirty)});
    if deadline.is_some() {
        state["startup_index"] = json!({"complete":tree.complete,
            "indexed_paths":tree.paths, "total_paths":tree.total,
            "reason": if tree.complete { "complete" } else { "wall-time budget" }});
        if !tree.complete {
            state["partial_tree_sha256"] = json!(tree.sha256);
        }
    }
    state
}

thread_local! {
    static START_STATE: std::cell::RefCell<Option<Value>> = const { std::cell::RefCell::new(None) };
}

/// Restore outer turns on nested execution and clear the snapshot on every exit.
pub(crate) struct WorkspaceStartScope(Option<Value>);

impl WorkspaceStartScope {
    pub(crate) fn enter(root: &Path) -> Self {
        let state = startup_workspace_state(root);
        Self(START_STATE.with(|cell| cell.replace(Some(state))))
    }

    pub(crate) fn partial_notice(&self) -> Option<&'static str> {
        START_STATE.with(|cell| {
            cell.borrow().as_ref().is_some_and(|state|
                state["head"] == "workspace identity probe unavailable"
            ).then_some("workspace_identity: partial; Git metadata probe unavailable; turn continues with unbound identity")
        })
    }
}

impl Drop for WorkspaceStartScope {
    fn drop(&mut self) {
        START_STATE.with(|cell| cell.replace(self.0.take()));
    }
}

pub(super) fn attach_start(state: &mut Value) {
    // head_tree belongs to START, even if a tool committed a new HEAD.
    state["head_tree"] = json!("unbound");
    START_STATE.with(|cell| {
        if let Some(start) = cell.borrow().as_ref() {
            state["head_tree"] = start["head_tree"].clone();
            for key in [
                "startup_index",
                "startup_walk_ms",
                "startup_walk_budget_ms",
                "partial_tree_sha256",
            ] {
                if let Some(value) = start.get(key) {
                    state[format!("start_{key}")] = value.clone();
                }
            }
            for (target, source) in [
                ("start_tree_sha256", "tree_sha256"),
                ("start_dirty_paths_sha256", "dirty_paths_sha256"),
            ] {
                let value = start[source].as_str().unwrap_or("unbound");
                state[target] = json!(if value == "not a git repo" {
                    "unbound"
                } else {
                    value
                });
            }
        }
    });
}

fn fast_sha256(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(64);
    for byte in ring::digest::digest(&ring::digest::SHA256, bytes).as_ref() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

struct TreeHash {
    sha256: String,
    paths: usize,
    total: usize,
    complete: bool,
}

fn expired(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|end| Instant::now() >= end)
}

// None denotes a cooperative deadline; errors remain unavailable evidence.
fn file_digest(path: &Path, deadline: Option<Instant>) -> std::io::Result<Option<String>> {
    use std::fmt::Write;
    let mut file = std::fs::File::open(path)?;
    let mut digest = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        if expired(deadline) {
            return Ok(None);
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    let mut hex = String::with_capacity(64);
    for byte in digest.finish().as_ref() {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(Some(hex))
}

#[cfg(test)]
fn hash_tree(root: &Path, names: &[u8]) -> Option<String> {
    hash_tree_with_deadline(root, names, None).map(|tree| tree.sha256)
}

fn hash_tree_with_deadline(
    root: &Path,
    names: &[u8],
    deadline: Option<Instant>,
) -> Option<TreeHash> {
    let mut names: Vec<_> = names.split(|b| *b == 0).filter(|p| !p.is_empty()).collect();
    names.sort_unstable();
    names.dedup();
    let mut tree = Vec::new();
    let total = names.len();
    let mut paths = 0;
    for name in names {
        if expired(deadline) {
            break;
        }
        let Ok(name_text) = std::str::from_utf8(name) else {
            return None;
        };
        let path = root.join(name_text);
        if path
            .ancestors()
            .skip(1)
            .take_while(|parent| *parent != root)
            .any(|parent| {
                std::fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink())
            })
        {
            return None;
        }
        let (kind, digest) = match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let Ok(target) = std::fs::read_link(&path) else {
                    return None;
                };
                ("link", fast_sha256(target.as_os_str().as_encoded_bytes()))
            }
            Ok(meta) if meta.is_file() => {
                let digest = file_digest(&path, deadline).ok()?;
                let Some(digest) = digest else {
                    break;
                };
                ("file", digest)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ("deleted", String::new()),
            _ => return None,
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::symlink_metadata(&path)
                .map(|m| m.permissions().mode() & 0o777)
                .unwrap_or(0);
            tree.extend_from_slice(&mode.to_be_bytes());
        }
        #[cfg(not(unix))]
        tree.extend_from_slice(&0u32.to_be_bytes());
        tree.extend_from_slice(&(name.len() as u64).to_be_bytes());
        tree.extend_from_slice(name);
        tree.push(0);
        tree.extend_from_slice(kind.as_bytes());
        tree.push(0);
        tree.extend_from_slice(digest.as_bytes());
        tree.push(0);
        paths += 1;
    }
    Some(TreeHash {
        sha256: fast_sha256(&tree),
        paths,
        total,
        complete: paths == total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_walk_deadline_is_partial_and_preserves_full_digest() {
        let _guard = crate::tests::env_lock();
        let root =
            std::env::temp_dir().join(format!("startup-deadline-{}", super::super::now_ms()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a"), b"first").unwrap();
        let partial = hash_tree_with_deadline(&root, b"a\0", Some(Instant::now())).unwrap();
        assert!(!partial.complete);
        assert_eq!((partial.paths, partial.total), (0, 1));
        let full = hash_tree_with_deadline(&root, b"a\0", None).unwrap();
        assert!(full.complete);
        assert_eq!(full.paths, 1);
        assert_ne!(partial.sha256, full.sha256);
        let bytes = vec![0x5a; 150_000];
        std::fs::write(root.join("large"), &bytes).unwrap();
        assert_eq!(
            file_digest(&root.join("large"), None).unwrap().unwrap(),
            crate::cut::sha256_hex(&bytes)
        );
        assert_eq!(
            file_digest(&root.join("large"), Some(Instant::now())).unwrap(),
            None
        );
        assert_eq!(fast_sha256(b"abc"), crate::cut::sha256_hex(b"abc"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_walk_honors_gitignore_and_info_exclude_and_labels_exhaustion() {
        let _guard = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!("startup-ignore-{}", super::super::now_ms()));
        std::fs::create_dir_all(&root).unwrap();
        let git = |args: &[&str]| {
            assert!(
                std::process::Command::new("git")
                    .arg("-C")
                    .arg(&root)
                    .args(args)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        };
        git(&["init", "-q"]);
        std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        std::fs::write(root.join("visible"), "tracked").unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "fixture",
        ]);
        std::fs::write(root.join(".git/info/exclude"), "excluded/\n").unwrap();
        let before =
            workspace_state_inner(Some(&root), Some(Instant::now() + Duration::from_secs(10)));
        assert_eq!(before["startup_index"]["complete"], true);
        for directory in ["ignored", "excluded"] {
            std::fs::create_dir(root.join(directory)).unwrap();
            for n in 0..100 {
                std::fs::write(root.join(directory).join(n.to_string()), b"litter").unwrap();
            }
        }
        let after =
            workspace_state_inner(Some(&root), Some(Instant::now() + Duration::from_secs(10)));
        assert_eq!(before["tree_sha256"], after["tree_sha256"]);
        assert_eq!(before["startup_index"], after["startup_index"]);
        std::fs::write(root.join("visible"), "changed").unwrap();
        assert_ne!(
            workspace_state(Some(&root))["tree_sha256"],
            before["tree_sha256"]
        );
        let previous = std::env::var_os("ANGEL_STARTUP_WALK_MS");
        unsafe {
            std::env::set_var("ANGEL_STARTUP_WALK_MS", "0");
        }
        let scope = WorkspaceStartScope::enter(&root);
        let mut attached = json!({});
        attach_start(&mut attached);
        if let Some(value) = previous {
            unsafe {
                std::env::set_var("ANGEL_STARTUP_WALK_MS", value);
            }
        } else {
            unsafe {
                std::env::remove_var("ANGEL_STARTUP_WALK_MS");
            }
        }
        assert_eq!(attached["start_startup_index"]["complete"], false);
        assert_ne!(attached["start_tree_sha256"], before["tree_sha256"]);
        drop(scope);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trace_schema_tree_digest_tracks_content_deletion_and_order() {
        let _guard = crate::tests::env_lock();
        let root =
            std::env::temp_dir().join(format!("trace-schema-content-{}", super::super::now_ms()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a"), b"first").unwrap();
        std::fs::write(root.join("b"), b"second").unwrap();
        let before = hash_tree(&root, b"a\0b\0").unwrap();
        assert_eq!(hash_tree(&root, b"b\0a\0a\0").unwrap(), before);
        std::fs::write(root.join("a"), b"changed").unwrap();
        let after = hash_tree(&root, b"a\0b\0").unwrap();
        assert_ne!(after, before);
        std::fs::remove_file(root.join("a")).unwrap();
        assert_ne!(hash_tree(&root, b"a\0b\0").unwrap(), after);
    }

    #[test]
    fn trace_schema_start_survives_edits_commits_nested_turns_and_scope_exit() {
        let _guard = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!("trace-start-{}", super::super::now_ms()));
        std::fs::create_dir_all(&root).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args([
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q"]);
        std::fs::write(root.join("a"), "before").unwrap();
        git(&["add", "a"]);
        git(&["commit", "-qm", "fixture start"]);
        let before = workspace_state(Some(&root));
        let scope = WorkspaceStartScope::enter(&root);
        std::fs::write(root.join("a"), "after").unwrap();
        let mut after = workspace_state(Some(&root));
        attach_start(&mut after);
        assert_eq!(after["start_tree_sha256"], before["tree_sha256"]);
        assert_eq!(
            after["start_dirty_paths_sha256"],
            before["dirty_paths_sha256"]
        );
        assert_ne!(after["tree_sha256"], after["start_tree_sha256"]);
        assert_ne!(
            after["dirty_paths_sha256"],
            after["start_dirty_paths_sha256"]
        );
        {
            let _nested = WorkspaceStartScope::enter(&root);
            attach_start(&mut after);
            assert_eq!(after["start_tree_sha256"], after["tree_sha256"]);
        }
        git(&["add", "a"]);
        git(&["commit", "-qm", "fixture end"]);
        after = workspace_state(Some(&root));
        assert_ne!(after["head_tree"], before["head_tree"]);
        attach_start(&mut after);
        assert_eq!(after["head_tree"], before["head_tree"]);
        assert_eq!(after["start_tree_sha256"], before["tree_sha256"]);
        assert_ne!(after["head"], before["head"]);
        drop(scope);
        after = workspace_state(Some(&root));
        attach_start(&mut after);
        assert_eq!(after["start_tree_sha256"], "unbound");
        assert_eq!(after["head_tree"], "unbound");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trace_schema_unknown_fields_are_explicit() {
        let mut record = json!({"ts_ms":1,"hops":2});
        defaults(&mut record);
        assert_eq!(record["turn"]["hop_count"], 2);
        assert!(record["cost"]["paid"].is_null());
        assert_eq!(record["verifier"], json!([]));
        assert_eq!(workspace_state(None)["head"], "unbound");
    }
    #[test]
    fn trace_schema_workspace_non_git_and_unborn() {
        let _guard = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!("trace-schema-nongit-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(
            workspace_state(Some(&root.join("absent")))["head"],
            "not a git repo"
        );
        let startup = startup_workspace_state(&root);
        assert_eq!(startup["head"], "not a git repo");
        assert_eq!(startup["startup_index"]["reason"], "not a git repo");
        assert!(WorkspaceStartScope::enter(&root).partial_notice().is_none());
        let git = |args: &[&str]| {
            assert!(
                std::process::Command::new("git")
                    .arg("-C")
                    .arg(&root)
                    .args(args)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        };
        git(&["init", "-q"]);
        // Empty repository has no HEAD; use the existing object's commit only in
        // real runs. No test creates a commit (worker policy).
        assert_eq!(workspace_state(Some(&root))["head"], "unbound");
        assert_eq!(startup_workspace_state(&root)["head"], "unbound");
        assert!(WorkspaceStartScope::enter(&root).partial_notice().is_none());
    }
}

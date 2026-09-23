//! Enforced per-call shell capability; path claims alone are not write evidence.
use super::*;
use std::collections::BTreeSet;

pub(super) struct ExecutionScope {
    pub policy: SandboxPolicy,
    // None is unrestricted/unknown; Some is the complete enforced source grant.
    pub paths: Option<Vec<String>>,
}

impl ShellTool {
    pub(super) fn scope(&self, args: &Value) -> Result<ExecutionScope, String> {
        let read_only = match args.get("read_only") {
            None => false,
            Some(Value::Bool(value)) => *value,
            _ => return Err("read_only must be a boolean".into()),
        };
        let paths = match args.get("write_paths") {
            None => None,
            Some(Value::Array(paths)) if paths.len() <= 128 => Some(paths),
            _ => {
                return Err(
                    "write_paths must be an array of at most 128 existing source files".into(),
                );
            }
        };
        if self.read_only || read_only {
            if paths.is_some_and(|paths| !paths.is_empty()) {
                return Err("read-only execution cannot grant write_paths".into());
            }
            return Ok(ExecutionScope {
                policy: self.write_scope_policy(Vec::new()),
                paths: Some(Vec::new()),
            });
        }
        let Some(paths) = paths else {
            return Ok(ExecutionScope {
                policy: self.policy.clone(),
                paths: None,
            });
        };
        if paths.is_empty() {
            return Ok(ExecutionScope {
                policy: self.write_scope_policy(Vec::new()),
                paths: Some(Vec::new()),
            });
        }
        let root = self
            .cwd
            .as_ref()
            .ok_or("write_paths requires an explicit workspace")?;
        reject_redirected_path(root)?;
        let root = root
            .canonicalize()
            .map_err(|error| format!("workspace identity: {error}"))?;
        let mut relative = BTreeSet::new();
        let mut writable = BTreeSet::new();
        let mut bytes = 0usize;
        for value in paths {
            let raw = value
                .as_str()
                .ok_or("write_paths entries must be strings")?;
            bytes = bytes.saturating_add(raw.len());
            if raw.is_empty() || bytes > 16 * 1024 {
                return Err("write_paths must be nonempty and bounded to 16 KiB".into());
            }
            let supplied = Path::new(raw);
            let rel = if supplied.is_absolute() {
                supplied
                    .strip_prefix(&root)
                    .map_err(|_| "write_paths must stay inside the workspace")?
            } else {
                supplied
            };
            for part in rel.components() {
                let std::path::Component::Normal(part) = part else {
                    return Err("write_paths cannot contain path traversal".into());
                };
                let part = part.to_str().ok_or("write_paths must be UTF-8")?;
                if part.starts_with('.')
                    || part.eq_ignore_ascii_case("off-limits")
                    || [
                        "AGENTS.md",
                        "CLAUDE.md",
                        "ROUTER.md",
                        "SKILL.md",
                        "benchmark.json",
                        "Cargo.toml",
                        "Cargo.lock",
                        "rust-toolchain",
                        "rust-toolchain.toml",
                    ]
                    .iter()
                    .any(|name| part.eq_ignore_ascii_case(name))
                {
                    return Err(
                        "write_paths cannot grant control, hidden or quarantined paths".into(),
                    );
                }
            }
            let path = root.join(rel);
            reject_redirected_path(&path)?;
            let meta = std::fs::symlink_metadata(&path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "write_paths requires existing files: {rel:?} does not exist. \
                         Create it with write_file/apply_patch first, or omit write_paths \
                         when the command needs normal workspace writes. write_paths is \
                         not an output-file list; no command was started."
                    )
                } else {
                    format!("write_paths requires existing files: {error}")
                }
            })?;
            if !meta.is_file() {
                return Err("write_paths accepts existing regular files, not directories".into());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if meta.nlink() != 1 {
                    return Err("write_paths cannot grant multiply linked files".into());
                }
            }
            let canonical = path
                .canonicalize()
                .map_err(|error| format!("write_paths identity: {error}"))?;
            if !canonical.starts_with(&root) {
                return Err("write_paths escaped workspace".into());
            }
            relative.insert(
                canonical
                    .strip_prefix(&root)
                    .expect("checked scope")
                    .to_str()
                    .ok_or("write_paths must be UTF-8")?
                    .to_owned(),
            );
            writable.insert(canonical);
        }
        Ok(ExecutionScope {
            policy: self.write_scope_policy(writable.into_iter().collect()),
            paths: Some(relative.into_iter().collect()),
        })
    }

    /// The enforced policy for a narrowed write scope. A read-only seat (a
    /// reviewer, `read_only_in_dir`) keeps its full ceiling, network included.
    /// A scope the model picks per call narrows writes only: network follows
    /// the shell's normal policy. Coupling the two cut a model off the network
    /// when it marked a lookup `read_only` (a `yukon submissions` fetch failed
    /// with "Unable to connect").
    fn write_scope_policy(&self, writable_roots: Vec<PathBuf>) -> SandboxPolicy {
        SandboxPolicy {
            writable_roots,
            allow_network: !self.read_only && self.policy.allow_network,
            enforce: true,
            mandatory: true,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        }
    }
}

fn reject_redirected_path(path: &Path) -> Result<(), String> {
    if path
        .components()
        .any(|part| part.as_os_str() == "off-limits")
    {
        return Err("quarantined source scope".into());
    }
    for parent in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        if parent.is_symlink() {
            return Err("write_paths cannot traverse symlink ancestors".into());
        }
    }
    Ok(())
}

//! Changed Rust paths and dependency evidence from this check's private target.
use super::*;
use std::collections::BTreeSet;
mod evidence;
pub(super) use evidence::cargo_evidence;

const MAX_PATHS: usize = 1024;
const MAX_DEP_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TOTAL_DEP_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TOTAL_PATHS: usize = 16 * MAX_PATHS;

#[derive(Default)]
struct MutationState {
    paths: BTreeSet<String>,
    opaque: u64,
}

#[derive(Default)]
pub(crate) struct MutationTargets(Mutex<MutationState>);
impl MutationTargets {
    pub(crate) fn record(&self, tool: &str, args: &Value) {
        if let Ok(mut state) = self.0.lock() {
            let paths = &mut state.paths;
            crate::cut::for_each_mutation_target_path(tool, args, |path| {
                if path.ends_with(".rs") && paths.len() <= MAX_PATHS {
                    paths.insert(path.to_owned());
                }
                false
            });
        }
    }
    pub(crate) fn mark_opaque(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.opaque = state.opaque.saturating_add(1);
        }
    }
    pub(crate) fn opaque_generation(&self) -> u64 {
        self.0.lock().map_or(u64::MAX, |state| state.opaque)
    }
    pub(crate) fn is_opaque(&self) -> bool {
        self.opaque_generation() != 0
    }
    pub(crate) fn record_scoped_paths(&self, paths: &[String]) {
        for path in paths {
            self.record("write_file", &serde_json::json!({"path":path,"content":""}));
        }
    }
    pub(crate) fn snapshot(&self) -> Option<Vec<String>> {
        let state = self.0.lock().ok()?;
        (state.opaque == 0 && state.paths.len() <= MAX_PATHS)
            .then(|| state.paths.iter().cloned().collect())
    }
}

pub(super) struct Required {
    pub paths: BTreeSet<PathBuf>,
    pub known: bool,
    pub opaque: bool,
}

pub(super) fn required(workspace: &Path, direct: &MutationTargets) -> Required {
    let opaque = direct.is_opaque();
    let direct = direct.snapshot();
    let git = crate::harness::changed_rust_paths_for_verification(workspace);
    let known = git.is_some() && direct.is_some();
    let mut paths = BTreeSet::new();
    for path in direct
        .into_iter()
        .flatten()
        .chain(git.into_iter().flatten())
    {
        let rel = Path::new(&path);
        if rel.is_absolute()
            || rel
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            || rel.components().any(|c| c.as_os_str() == "off-limits")
        {
            return Required {
                paths,
                known: false,
                opaque,
            };
        }
        let candidate = workspace.join(rel);
        if has_symlink_ancestor(&candidate) {
            return Required {
                paths,
                known: false,
                opaque,
            };
        }
        match std::fs::canonicalize(candidate) {
            Ok(path) if path.starts_with(workspace) => {
                paths.insert(path);
            }
            _ => {
                return Required {
                    paths,
                    known: false,
                    opaque,
                };
            }
        }
        if paths.len() > MAX_PATHS {
            return Required {
                paths,
                known: false,
                opaque,
            };
        }
    }
    Required {
        paths,
        known,
        opaque,
    }
}

pub(super) fn annotate(
    summary: String,
    required: &Required,
    checked: Option<&BTreeSet<PathBuf>>,
    explicit: bool,
) -> String {
    let state = if !required.known || checked.is_none() {
        "unknown"
    } else if required
        .paths
        .iter()
        .all(|p| checked.is_some_and(|set| set.contains(p)))
    {
        "complete"
    } else {
        "incomplete"
    };
    let header = if state == "incomplete" && !explicit {
        "verification inconclusive: Cargo command passed but default targets exclude changed Rust files".to_owned()
    } else {
        summary.lines().next().unwrap_or("").to_owned()
    };
    let rest = summary.split_once('\n').map_or("", |(_, rest)| rest);
    let missing = required
        .paths
        .iter()
        .filter(|p| !checked.is_some_and(|set| set.contains(*p)))
        .take(8)
        .map(|p| crate::harness::cap_text_owned(p.display().to_string(), 240, 0))
        .collect::<Vec<_>>();
    let digest = |paths: &BTreeSet<PathBuf>| {
        crate::cut::sha256_hex(
            &paths
                .iter()
                .flat_map(|p| {
                    let mut bytes = p.as_os_str().as_encoded_bytes().to_vec();
                    bytes.push(0);
                    bytes
                })
                .collect::<Vec<_>>(),
        )
    };
    let details = serde_json::json!({"basis":"changed-rust-paths-and-current-compiler-artifacts", "required_count":required.paths.len(), "required_set_sha256":digest(&required.paths), "checked_source_count":checked.map(BTreeSet::len), "checked_set_sha256":checked.map(digest), "missing_preview":missing, "command_status":"passed"});
    let guidance = match state {
        "complete" => "",
        "incomplete" => {
            "\nThe selected targets omit known changed Rust paths. Select the required package; unrelated packages need not be rebuilt."
        }
        _ if required.opaque => {
            "\nEarlier unrestricted writable execution left the source inventory unknown. This result is useful command-local evidence; repeating a check or selecting another package cannot attest those earlier writes. Report the scope limitation or use independently pinned task acceptance. Use read_only/write_paths for future commands; neither resets prior uncertainty."
        }
        _ if !required.known => {
            "\nThe required source inventory could not be established from bounded workspace evidence. Keep this result command-local and report the scope limitation; package selection alone cannot restore missing inventory evidence."
        }
        _ => {
            "\nCurrent compiler dependency evidence could not establish source coverage. Keep this result command-local and report the evidence limitation; do not repeat unchanged checks as a cure."
        }
    };
    format!("{header}; coverage={state}\n{details}\n{rest}{guidance}")
}

// Package/target choices are command-local evidence even when successful.
pub(super) fn explicit_selection(argv: &[String]) -> bool {
    argv.iter().skip(1).any(|arg| {
        arg.starts_with("-p")
            || arg == "--package"
            || arg.starts_with("--package=")
            || arg == "--workspace"
            || arg == "--all"
            || arg == "--lib"
            || arg == "--bin"
            || arg.starts_with("--bin=")
            || arg == "--bins"
            || arg == "--test"
            || arg.starts_with("--test=")
            || arg == "--tests"
            || arg == "--example"
            || arg.starts_with("--example=")
            || arg == "--examples"
            || arg == "--all-targets"
    })
}

fn has_symlink_ancestor(path: &Path) -> bool {
    // Check parents first: inspecting a child of a symlink would already follow
    // the redirected parent before discovering that it is outside the scope.
    path.ancestors()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .any(Path::is_symlink)
}

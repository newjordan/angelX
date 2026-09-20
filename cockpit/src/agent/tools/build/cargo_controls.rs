//! Registry-time Cargo/rustup control-file identity. Keep lookup names as well
//! as canonical targets so additions, removals and symlink retargets fail closed.
use super::*;

#[derive(Default)]
pub(super) struct CargoControls {
    workspace: PathBuf,
    lookup: Vec<PathBuf>,
    files: Vec<ControlFile>,
}

struct ControlFile {
    path: PathBuf,
    canonical: PathBuf,
    sha256: String,
    config: Option<toml::Value>,
}

fn lookup_paths(workspace: &Path) -> Vec<PathBuf> {
    workspace
        .ancestors()
        .flat_map(|dir| {
            [
                dir.join(".cargo/config"),
                dir.join(".cargo/config.toml"),
                dir.join("rust-toolchain"),
                dir.join("rust-toolchain.toml"),
            ]
        })
        .collect()
}

impl CargoControls {
    pub(super) fn capture(cwd: &Path, workspace: &Path) -> Result<Self, String> {
        let cwd = std::fs::canonicalize(cwd).map_err(|e| {
            format!(
                "resolve Cargo control lookup directory {}: {e}",
                cwd.display()
            )
        })?;
        let lookup = lookup_paths(&cwd);
        let mut files = Vec::new();
        for path in &lookup {
            let bytes = match std::fs::read(path) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(format!("read control file {}: {e}", path.display())),
            };
            let canonical = std::fs::canonicalize(path)
                .map_err(|e| format!("resolve control file {}: {e}", path.display()))?;
            let sha256 = crate::agent::harness::sha256_reader_hex(&mut bytes.as_slice())
                .map_err(|e| format!("hash control file {}: {e}", path.display()))?;
            let config = if matches!(
                path.file_name().and_then(OsStr::to_str),
                Some("config" | "config.toml")
            ) {
                Some(
                    String::from_utf8(bytes)
                        .map_err(|e| format!("decode control file {}: {e}", path.display()))?
                        .parse::<toml::Value>()
                        .map_err(|e| format!("parse control file {}: {e}", path.display()))?,
                )
            } else {
                None
            };
            files.push(ControlFile {
                path: path.clone(),
                canonical,
                sha256,
                config,
            });
        }
        Ok(Self {
            workspace: workspace.to_path_buf(),
            lookup,
            files,
        })
    }

    pub(super) fn attach(
        self,
        mut executable: PinnedCargoExecutable,
    ) -> Result<PinnedCargoExecutable, String> {
        if crate::platform::yolo::enabled() {
            for file in &self.files {
                let Some(config) = &file.config else {
                    continue;
                };
                validate_values(config, "", file, &self.workspace)?;
            }
            // Cargo environment is sealed at dispatch. Preserve an explicitly
            // configured compiler by resolving and hashing it into that seal.
            for file in &self.files {
                if file.path.file_name() == Some(OsStr::new("config.toml"))
                    && self
                        .files
                        .iter()
                        .any(|other| other.path == file.path.with_file_name("config"))
                {
                    continue;
                }
                let Some(raw) = file
                    .config
                    .as_ref()
                    .and_then(|v| v.get("build"))
                    .and_then(|v| v.get("rustc"))
                    .and_then(toml::Value::as_str)
                else {
                    continue;
                };
                let path = Path::new(raw);
                let resolved = if path.is_absolute() {
                    path.to_path_buf()
                } else if raw.contains('/') {
                    file.path
                        .parent()
                        .and_then(Path::parent)
                        .unwrap_or(Path::new("/"))
                        .join(path)
                } else {
                    std::env::split_paths(&executable.trusted_path)
                        .map(|dir| dir.join(path))
                        .find(|path| path.is_file())
                        .ok_or_else(|| {
                            format!(
                                "control file {}: build.rustc cannot resolve on pinned PATH",
                                file.path.display()
                            )
                        })?
                };
                executable.rustc =
                    capture_toolchain_executable(resolved, "configured rustc", &self.workspace)?;
                break;
            }
        }
        self.revalidate()?;
        executable.controls = self;
        Ok(executable)
    }

    pub(super) fn has_toolchain(&self) -> bool {
        self.files.iter().any(|file| file.config.is_none())
    }

    pub(super) fn receipt(&self) -> Value {
        Value::Array(
            self.files
                .iter()
                .map(|file| {
                    serde_json::json!({
                        "path": file.path, "canonical_path": file.canonical, "sha256": file.sha256,
                    })
                })
                .collect(),
        )
    }

    pub(super) fn revalidate(&self) -> Result<(), String> {
        for path in &self.lookup {
            let pinned = self.files.iter().find(|file| &file.path == path);
            let current = std::fs::read(path);
            let unchanged = match (pinned, current) {
                (None, Err(e)) if e.kind() == std::io::ErrorKind::NotFound => true,
                (Some(file), Ok(bytes)) => {
                    std::fs::canonicalize(path).ok().as_ref() == Some(&file.canonical)
                        && crate::agent::harness::sha256_reader_hex(&mut bytes.as_slice())
                            .ok()
                            .as_ref()
                            == Some(&file.sha256)
                }
                _ => false,
            };
            if !unchanged {
                return Err(format!(
                    "control file {} changed since pin (content or lookup identity rule)",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    pub(super) fn validate_dispatch(&self, cwd: &Path) -> Result<(), String> {
        // Explicit crate routing may introduce controls that were not visible
        // when the registry selected its default crate. Never adopt them late.
        for path in lookup_paths(cwd) {
            if path.exists() && !self.lookup.contains(&path) {
                return Err(format!(
                    "trusted Cargo verification refuses workspace-controlled Cargo/toolchain semantics at {}: control file absent from registry pin",
                    path.display()
                ));
            }
        }
        for file in &self.files {
            if let Some(config) = &file.config {
                validate_values(config, "", file, &self.workspace)?;
            }
        }
        self.revalidate()
    }

    pub(super) fn config_args(&self) -> Vec<OsString> {
        // Cargo gives nearer configuration precedence; legacy `config` wins
        // over `config.toml` when both names occur in the same directory.
        self.files
            .iter()
            .rev()
            .filter(|file| {
                file.config.is_some()
                    && file.path.parent() != Some(Path::new("/.cargo"))
                    && !(file.path.file_name() == Some(OsStr::new("config.toml"))
                        && self
                            .files
                            .iter()
                            .any(|other| other.path == file.path.with_file_name("config")))
            })
            .flat_map(|file| {
                [
                    OsString::from("--config"),
                    file.path.clone().into_os_string(),
                ]
            })
            .collect()
    }
}

fn validate_values(
    value: &toml::Value,
    key: &str,
    file: &ControlFile,
    workspace: &Path,
) -> Result<(), String> {
    match value {
        toml::Value::Table(table) => {
            for (name, value) in table {
                let key = if key.is_empty() {
                    name.clone()
                } else {
                    format!("{key}.{name}")
                };
                validate_values(value, &key, file, workspace)?;
            }
        }
        toml::Value::Array(values) => {
            for value in values {
                validate_values(value, key, file, workspace)?;
            }
        }
        toml::Value::String(raw) => {
            let leaf = key.rsplit('.').next().unwrap_or(key);
            let redirect = matches!(
                leaf,
                "rustc"
                    | "rustdoc"
                    | "rustc-wrapper"
                    | "rustc-workspace-wrapper"
                    | "runner"
                    | "linker"
                    | "paths"
                    | "directory"
                    | "local-registry"
                    | "credential-provider"
                    | "global-credential-providers"
            ) || key.contains("rustflags")
                || key.starts_with("credential-alias.")
                || key.starts_with("env.RUSTC")
                || key.starts_with("env.CARGO_BUILD_RUSTC")
                || (key.starts_with("env.CARGO_TARGET_")
                    && (key.contains("RUNNER") || key.contains("LINKER")));
            if redirect {
                let executable = !matches!(leaf, "paths" | "directory" | "local-registry");
                for (index, word) in raw.split_whitespace().enumerate() {
                    if key.contains("rustflags") && !word.contains("linker=") && !word.contains("/")
                    {
                        continue;
                    }
                    if executable && !key.contains("rustflags") && index > 0 {
                        break;
                    }
                    let word = word
                        .strip_prefix("linker=")
                        .or_else(|| word.strip_prefix("-Clinker="))
                        .unwrap_or(word);
                    if word.starts_with('-') {
                        continue;
                    }
                    if executable && !word.contains('/') {
                        // Bare executable names use the sealed host PATH.
                        continue;
                    }
                    let path = Path::new(word);
                    let base = file
                        .path
                        .parent()
                        .and_then(Path::parent)
                        .unwrap_or(Path::new("/"));
                    let candidate = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        base.join(path)
                    };
                    let candidate = normalize_path(&candidate);
                    let candidate = std::fs::canonicalize(&candidate).unwrap_or(candidate);
                    if let Some(root) = task_writable_roots(workspace)
                        .iter()
                        .find(|root| path_within(&candidate, root))
                    {
                        return Err(format!(
                            "control file {}: {key} points execution at task-writable path {} (root {})",
                            file.path.display(),
                            candidate.display(),
                            root.display()
                        ));
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/tools/build__cargo_controls__tests.rs"]
mod tests;

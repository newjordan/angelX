//! Bounded dependency parsing inside the current Cargo target lifetime.
use super::*;
use std::io::Read;

/// Parse only the first Make dependency rule; phony rules cannot add coverage.
fn dependencies(bytes: &[u8]) -> Option<Vec<PathBuf>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let line = text.lines().next()?;
    let (_, rhs) = line.split_once(": ")?;
    let mut result = Vec::new();
    let mut word = String::new();
    let mut chars = rhs.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                let escaped = chars.next()?;
                if !matches!(escaped, ' ' | '\t' | '\\' | '#' | ':') {
                    return None;
                }
                word.push(escaped);
            }
            '$' | '#' => return None,
            ' ' | '\t' => {
                if !word.is_empty() {
                    result.push(PathBuf::from(std::mem::take(&mut word)));
                }
            }
            c if c.is_control() => return None,
            c => word.push(c),
        }
        if result.len() > MAX_PATHS {
            return None;
        }
    }
    if !word.is_empty() {
        result.push(PathBuf::from(word));
    }
    (!result.is_empty() && result.len() <= MAX_PATHS).then_some(result)
}

fn bounded_dep(path: &Path, target: &Path, remaining_bytes: &mut u64) -> Option<Vec<PathBuf>> {
    if !path.starts_with(target)
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        || has_symlink_ancestor(path)
    {
        return None;
    }
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_DEP_BYTES || metadata.len() >= *remaining_bytes {
        *remaining_bytes = 0;
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(metadata.len() + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    *remaining_bytes = remaining_bytes.saturating_sub(bytes.len() as u64);
    if bytes.len() as u64 != metadata.len() {
        return None;
    }
    dependencies(&bytes)
}

/// Cargo owns the JSON records; read only depfiles derived from their artifacts
/// while the fresh private target still exists. Never enumerate a target tree.
pub(crate) fn cargo_evidence(
    stdout: &str,
    private_target: &Path,
    workspace: &Path,
) -> (Option<BTreeSet<PathBuf>>, String) {
    let mut sources = BTreeSet::new();
    let mut diagnostics = String::new();
    let mut finished = false;
    let mut artifacts = 0;
    let mut valid = true;
    let mut remaining_bytes = MAX_TOTAL_DEP_BYTES;
    let mut remaining_paths = MAX_TOTAL_PATHS;
    for line in stdout.lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            valid = false;
            continue;
        };
        match message.get("reason").and_then(Value::as_str) {
            Some("compiler-message") => {
                if let Some(rendered) = message.pointer("/message/rendered").and_then(Value::as_str)
                {
                    let room = (32usize * 1024).saturating_sub(diagnostics.len());
                    let mut end = room.min(rendered.len());
                    while !rendered.is_char_boundary(end) {
                        end -= 1;
                    }
                    diagnostics.push_str(&rendered[..end]);
                }
            }
            Some("build-finished") => {
                finished = message.get("success").and_then(Value::as_bool) == Some(true);
            }
            Some("compiler-artifact") => {
                artifacts += 1;
                if artifacts > MAX_PATHS || remaining_bytes == 0 || remaining_paths == 0 {
                    valid = false;
                    continue;
                }
                let source = message
                    .pointer("/target/src_path")
                    .and_then(Value::as_str)
                    .map(PathBuf::from);
                let Some(source) = source else {
                    valid = false;
                    continue;
                };
                if source.components().any(|c| {
                    c.as_os_str() == "off-limits" || matches!(c, std::path::Component::ParentDir)
                }) || has_symlink_ancestor(&source)
                {
                    valid = false;
                    continue;
                }
                let Ok(source) = std::fs::canonicalize(source) else {
                    valid = false;
                    continue;
                };
                let Some(files) = message.get("filenames").and_then(Value::as_array) else {
                    valid = false;
                    continue;
                };
                let mut artifact_sources = None;
                for file in files.iter().filter_map(Value::as_str).map(PathBuf::from) {
                    let Some(stem) = file.file_stem().and_then(OsStr::to_str) else {
                        continue;
                    };
                    let stem = if matches!(
                        file.extension().and_then(OsStr::to_str),
                        Some("rmeta" | "rlib" | "so" | "dylib")
                    ) {
                        stem.strip_prefix("lib").unwrap_or(stem)
                    } else {
                        stem
                    };
                    let dep = file.with_file_name(format!("{stem}.d"));
                    let Some(deps) = bounded_dep(&dep, private_target, &mut remaining_bytes) else {
                        if remaining_bytes == 0 {
                            break;
                        }
                        continue;
                    };
                    let mut bases = BTreeSet::new();
                    for path in &deps {
                        if !path.is_absolute() && source.ends_with(path) {
                            let mut base = source.clone();
                            for _ in path.components() {
                                base.pop();
                            }
                            bases.insert(base);
                        }
                    }
                    let base = if deps.iter().all(|p| p.is_absolute()) {
                        Some(workspace.to_path_buf())
                    } else if bases.len() == 1 {
                        bases.into_iter().next()
                    } else {
                        None
                    };
                    let Some(base) = base else {
                        continue;
                    };
                    let mut resolved = BTreeSet::new();
                    let mut good = true;
                    for path in deps {
                        if remaining_paths == 0 {
                            good = false;
                            break;
                        }
                        remaining_paths -= 1;
                        let path = if path.is_absolute() {
                            path
                        } else {
                            base.join(path)
                        };
                        if path.components().any(|c| c.as_os_str() == "off-limits")
                            || has_symlink_ancestor(&path)
                        {
                            good = false;
                            break;
                        }
                        match std::fs::canonicalize(path) {
                            Ok(path) => {
                                resolved.insert(path);
                            }
                            Err(_) => {
                                good = false;
                                break;
                            }
                        }
                    }
                    if good && resolved.contains(&source) {
                        artifact_sources = Some(resolved);
                        break;
                    }
                }
                if let Some(found) = artifact_sources {
                    sources.extend(found);
                } else {
                    valid = false;
                }
            }
            Some("build-script-executed") => {}
            _ => {
                valid = false;
            }
        }
    }
    (
        if valid && finished && artifacts > 0 && remaining_bytes > 0 && remaining_paths > 0 {
            Some(sources)
        } else {
            None
        },
        diagnostics,
    )
}

#[cfg(test)]
#[path = "../../../../../../tests/cockpit/tools/build__targets__evidence__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../../../tests/cockpit/tools/build__targets__evidence__budget_tests.rs"]
mod budget_tests;

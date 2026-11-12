//! The plugin tree digest: one SHA-256 over a plugin's files, their kinds and
//! their paths, the same whether read from git objects or from a folder.
//!
//! ```text
//! angelx-plugin-tree/v1\n
//! <kind> <path>\0<sha256 of content, hex>\n      one line per file, by path bytes
//! ```
//!
//! `kind` is `f` (a regular file), `x` (an executable file) or `l` (a symlink,
//! whose content is its target). Paths are relative and `/`-separated.
//! Directories count only through the files in them, as in git.
//! `scripts/plugins/check-registry.py` computes the same digest for review.

use ring::digest::{Context, SHA256};
use std::path::Path;

pub(crate) const SCHEME: &str = "angelx-plugin-tree/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    File,
    Exec,
    Link,
}

impl Kind {
    fn mark(self) -> char {
        match self {
            Kind::File => 'f',
            Kind::Exec => 'x',
            Kind::Link => 'l',
        }
    }
}

/// One file of a plugin tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    /// Relative, `/`-separated.
    pub(crate) path: String,
    pub(crate) kind: Kind,
    /// The file's bytes, or a symlink's target.
    pub(crate) bytes: Vec<u8>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex(ring::digest::digest(&SHA256, bytes).as_ref())
}

/// The digest of `entries`, in any order.
pub(crate) fn digest(entries: &[Entry]) -> String {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    let mut context = Context::new(&SHA256);
    context.update(SCHEME.as_bytes());
    context.update(b"\n");
    for entry in sorted {
        context.update(
            format!(
                "{} {}\0{}\n",
                entry.kind.mark(),
                entry.path,
                sha256_hex(&entry.bytes)
            )
            .as_bytes(),
        );
    }
    hex(context.finish().as_ref())
}

/// Every file under `dir`, as the digest reads it. A `.git` folder and the
/// install receipt are not part of a plugin.
pub(crate) fn read_folder(dir: &Path) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    walk(dir, "", &mut entries)?;
    Ok(entries)
}

/// The digest of the files under `dir`.
pub(crate) fn digest_folder(dir: &Path) -> Result<String, String> {
    Ok(digest(&read_folder(dir)?))
}

fn walk(dir: &Path, prefix: &str, entries: &mut Vec<Entry>) -> Result<(), String> {
    let listing = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for item in listing {
        let item = item.map_err(|e| format!("{}: {e}", dir.display()))?;
        let name = item
            .file_name()
            .into_string()
            .map_err(|name| format!("{}: a name that is not UTF-8: {name:?}", dir.display()))?;
        if name == ".git" || (prefix.is_empty() && name == super::install::RECEIPT) {
            continue;
        }
        let path = item.path();
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let meta =
            std::fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            let target =
                std::fs::read_link(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let target = target
                .into_os_string()
                .into_string()
                .map_err(|_| format!("{}: a link target that is not UTF-8", path.display()))?;
            entries.push(Entry {
                path: rel,
                kind: Kind::Link,
                bytes: target.into_bytes(),
            });
        } else if kind.is_dir() {
            walk(&path, &rel, entries)?;
        } else if kind.is_file() {
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            entries.push(Entry {
                path: rel,
                kind: if executable(&meta) {
                    Kind::Exec
                } else {
                    Kind::File
                },
                bytes,
            });
        } else {
            return Err(format!("{}: not a file, folder or symlink", path.display()));
        }
    }
    Ok(())
}

/// Git's rule: a file is executable when its owner may run it.
#[cfg(unix)]
fn executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o100 != 0
}

#[cfg(not(unix))]
fn executable(_meta: &std::fs::Metadata) -> bool {
    false
}

/// Write `entries` under `dir`, which must not exist yet.
pub(crate) fn write_folder(dir: &Path, entries: &[Entry]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        if !safe_path(&entry.path) {
            return Err(format!("`{}` is not a path inside the plugin", entry.path));
        }
        let path = dir.join(&entry.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        match entry.kind {
            Kind::Link => {
                let target = std::str::from_utf8(&entry.bytes)
                    .map_err(|_| format!("{}: a link target that is not UTF-8", entry.path))?;
                #[cfg(unix)]
                std::os::unix::fs::symlink(target, &path)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                #[cfg(not(unix))]
                return Err(format!(
                    "{}: symlinks need a unix system ({target})",
                    entry.path
                ));
            }
            Kind::File | Kind::Exec => {
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(if entry.kind == Kind::Exec {
                        0o755
                    } else {
                        0o644
                    });
                }
                use std::io::Write;
                options
                    .open(&path)
                    .and_then(|mut file| file.write_all(&entry.bytes))
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
    }
    Ok(())
}

/// A relative path of plain names: no `..`, no `.`, nothing absolute, and no
/// `.git` (the place git keeps its own files).
pub(crate) fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part != ".git"
                && !part.contains('\0')
                && !part.contains('\\')
        })
}

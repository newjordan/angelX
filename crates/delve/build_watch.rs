//! Emit each Cargo build input once. Directory watches are recursive; an
//! explicit canonical descendant adds no coverage. Keep aliases, symlinks and
//! missing paths explicit rather than guessing what their spelling means.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub fn minimal_paths(cwd: &Path, inputs: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let paths: BTreeSet<_> = inputs.into_iter().collect();
    // Resolve relative inputs against the build-script directory, but retain
    // their original spelling in the emitted directive. A canonical equality
    // check deliberately rejects `..` and paths through symlinks.
    let absolute = |path: &Path| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        }
    };
    let canonical: BTreeSet<_> = paths
        .iter()
        .map(|p| absolute(p))
        .filter(|p| {
            p.canonicalize()
                .is_ok_and(|resolved| resolved.as_os_str() == p.as_os_str())
        })
        .collect();
    let directories: BTreeSet<_> = canonical.iter().filter(|p| p.is_dir()).collect();
    paths
        .into_iter()
        .filter(|path| {
            let full = absolute(path);
            !canonical.contains(&full)
                || !full
                    .ancestors()
                    .skip(1)
                    .any(|parent| directories.contains(&parent.to_path_buf()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_children_collapse_but_missing_and_alias_paths_remain() {
        let root = std::env::temp_dir().join(format!("angel-delve-watch-{}", std::process::id()));
        std::fs::create_dir_all(root.join("assets/kit")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("assets/kit/frame.png"), b"pixels").unwrap();
        let paths = [
            "assets",
            "assets/kit",
            "assets/kit/frame.png",
            "assets/missing",
            "assets/kit/../kit/frame.png",
            "assets",
        ]
        .map(PathBuf::from);
        let result = minimal_paths(&root, paths);
        assert_eq!(
            result,
            ["assets", "assets/kit/../kit/frame.png", "assets/missing"].map(PathBuf::from)
        );
        // An absolute descendant is covered by the same relative parent.
        assert_eq!(
            minimal_paths(
                &root,
                [PathBuf::from("assets"), root.join("assets/kit/frame.png")]
            ),
            [PathBuf::from("assets")]
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("kit", root.join("assets/alias")).unwrap();
            assert_eq!(
                minimal_paths(
                    &root,
                    [
                        PathBuf::from("assets"),
                        PathBuf::from("assets/alias/frame.png")
                    ]
                ),
                [
                    PathBuf::from("assets"),
                    PathBuf::from("assets/alias/frame.png")
                ]
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn distinct_files_and_missing_roots_do_not_collapse() {
        let root = std::env::current_dir().unwrap();
        let paths =
            ["uncreated-root", "uncreated-root/frame.png", "another.png"].map(PathBuf::from);
        let mut expected = paths.to_vec();
        expected.sort();
        assert_eq!(minimal_paths(&root, paths), expected);
    }
}

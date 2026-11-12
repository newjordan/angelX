//! Decisions the cockpit's build script makes so a working build stays short.
//!
//! Every normal build keeps the browser page. Working builds compile the same
//! page without fat LTO; they must not appear fast by silently removing it.
//! An explicit development-only opt-out remains available for focused work.
//!
//! This toolchain's `PROFILE` is only `debug` or `release`, even for
//! `--profile fast` (it inherits release). The target directory names the
//! real profile (`.../fast/build/.../out`).

use std::path::{Path, PathBuf};
use std::process::Command;

/// A nested WASM build must not reuse a build engine's `CARGO`/`RUSTC`
/// aliases: they may be script launchers requiring private protocol state.
/// Use both tools from the sysroot already queried from the outer compiler.
pub fn wasm_cargo(sysroot: &Path) -> Command {
    let bin = sysroot.join("bin");
    let mut cargo = Command::new(bin.join(format!("cargo{}", std::env::consts::EXE_SUFFIX)));
    cargo.env_clear();
    for key in ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "TMPDIR", "TEMP", "TMP", "SystemRoot", "USERPROFILE"] {
        if let Some(value) = std::env::var_os(key) {
            cargo.env(key, value);
        }
    }
    cargo.env("RUSTC", bin.join(format!("rustc{}", std::env::consts::EXE_SUFFIX)));
    cargo
}

/// Profile the nested browser-page cargo uses.
///
/// `profile_env` is cargo's `PROFILE`. `out_dir` is `OUT_DIR`. Release and
/// bench keep fat LTO. Everything rebuilt often (`fast`, `dev`, `debug`) does not.
pub fn nested_web_profile(profile_env: &str, out_dir: &str) -> &'static str {
    let name = profile_from_out_dir(out_dir).unwrap_or(profile_env);
    match name {
        "release" | "bench" => "web",
        "fast" => "web-size",
        _ => "web-fast",
    }
}

/// All normal builds include browser packaging. Only `Some("1")` skips it,
/// and only in a working profile. Release artifacts reject an explicit skip.
pub fn skip_web_bundle(
    requested: Option<&str>,
    profile_env: &str,
    out_dir: &str,
) -> Result<bool, &'static str> {
    let profile = profile_from_out_dir(out_dir).unwrap_or(profile_env);
    let allowed = profile == "fast"
        || (matches!(profile, "debug" | "dev") && matches!(profile_env, "debug" | "dev"));
    if requested == Some("1") && !allowed {
        return Err("ANGEL_DEV_SKIP_WEB_BUNDLE is allowed only for debug, dev or fast builds");
    }
    Ok(requested == Some("1"))
}

/// The profile directory in cargo's layout: `$target/$profile/build/$pkg/out`,
/// or `$target/$triple/$profile/build/$pkg/out`.
pub fn profile_from_out_dir(out_dir: &str) -> Option<&str> {
    let parts: Vec<&str> = out_dir.split(['/', '\\']).filter(|part| !part.is_empty()).collect();
    let build = parts.iter().rposition(|part| *part == "build")?;
    if build == 0 {
        return None;
    }
    Some(parts[build - 1])
}

/// Cargo treats a `rerun-if-changed` path that is not there as changed, and
/// reruns the build script on every build. Only watch a path that exists.
pub fn should_watch(path: &Path) -> bool {
    path.exists()
}

/// Keep the same freshness coverage without asking Cargo to traverse an input
/// twice. Cargo watches directories recursively, so an explicit child of a
/// watched directory adds no coverage. Do not normalize aliases: symlinks and
/// paths containing `..` stay explicit, including their own identity changes.
/// Missing inputs stay explicit too; their policy belongs to the caller.
pub fn minimal_watch_paths(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    use std::collections::BTreeSet;
    let paths: BTreeSet<_> = paths.into_iter().collect();
    let canonical: BTreeSet<_> = paths.iter()
        .filter(|p| p.canonicalize().is_ok_and(|resolved| resolved == **p))
        .cloned().collect();
    let directories: BTreeSet<_> = canonical.iter().filter(|p| p.is_dir()).cloned().collect();
    paths.iter().filter(|path| {
        !canonical.contains(*path)
            || !path.ancestors().skip(1).any(|parent| directories.contains(parent))
    }).cloned().collect()
}

/// Cargo's aggregate `.d` file includes transitive sources, compile-time assets
/// and build-script inputs. Cargo escapes spaces, but leaves hashes, dollars
/// and other backslashes literal (this is not fully Make-escaped text). The
/// first rule is the artifact's complete input list.
pub fn nested_dependency_paths(dep_info: &str) -> Result<Vec<String>, &'static str> {
    let (_, inputs) = dep_info.split_once(": ").ok_or("nested Cargo dep-info has no dependency rule")?;
    let mut paths = Vec::new();
    let mut path = String::new();
    let mut chars = inputs.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.peek() {
                Some(' ') => { chars.next(); path.push(' '); },
                Some('\n') => { chars.next(); },
                Some('\r') => {
                    chars.next();
                    if chars.peek() == Some(&'\n') { chars.next(); }
                },
                Some(_) => path.push('\\'),
                None => return Err("nested Cargo dep-info ends with an escape"),
            },
            '\n' | '\r' => break,
            ch if ch.is_whitespace() => {
                if !path.is_empty() { paths.push(std::mem::take(&mut path)); }
            },
            _ => path.push(ch),
        }
    }
    if !path.is_empty() { paths.push(path); }
    if paths.is_empty() { return Err("nested Cargo dep-info has no inputs"); }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn recursive_watch_minimization_keeps_aliases_and_external_inputs() {
        let root = std::env::temp_dir().join(format!("angel-watch-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let watched = root.join("assets");
        let sub = watched.join("kit");
        std::fs::create_dir_all(&sub).unwrap();
        let child = sub.join("sprite.png");
        let external = root.join("registry.json");
        std::fs::write(&child, b"sprite").unwrap();
        std::fs::write(&external, b"{}").unwrap();
        let missing = watched.join("not-created");
        let alias = sub.join("..").join("kit/sprite.png");
        let mut inputs = vec![child.clone(), sub, watched.clone(), external.clone(),
            watched.clone(), missing.clone(), alias.clone()];
        let mut expected = vec![watched, external, missing, alias];
        #[cfg(unix)]
        {
            let link = root.join("linked.png");
            std::os::unix::fs::symlink(&child, &link).unwrap();
            inputs.push(link.clone());
            expected.push(link);
        }
        expected.sort();
        assert_eq!(minimal_watch_paths(inputs), expected);
        assert!(minimal_watch_paths([]).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn nested_dependencies_preserve_cargo_paths_and_ignore_phony_rules() {
        // Cargo 1.95's aggregate .d escapes spaces only. Dollars and hashes
        // are literal, including repeated dollars and a backslash before '#'.
        // Captured real Cargo probes: web-bundle-{escape,literal}-depinfo receipts.
        let info = "C:\\target\\game.wasm: /repo/a\\ b.rs /repo/c#d.png /repo/cost$$.png /repo/back\\slash.txt /repo/back\\#hash.txt /repo/back\\\\ space.txt \\\n /repo/last.rs\n\n/repo/phony.rs:\n";
        assert_eq!(nested_dependency_paths(info).unwrap(), ["/repo/a b.rs", "/repo/c#d.png", "/repo/cost$$.png", "/repo/back\\slash.txt", "/repo/back\\#hash.txt", "/repo/back\\ space.txt", "/repo/last.rs"]);
        assert!(nested_dependency_paths("no rule").is_err());
        assert!(nested_dependency_paths("out: ").is_err());
        assert!(nested_dependency_paths("out: unfinished\\").is_err());
    }

    #[test]
    fn nested_wasm_uses_sysroot_tools_not_build_engine_aliases() {
        let sysroot = Path::new("/pinned/toolchain");
        let cargo = wasm_cargo(sysroot);
        assert_eq!(cargo.get_program(), sysroot.join("bin").join(format!("cargo{}", std::env::consts::EXE_SUFFIX)));
        let vars: std::collections::BTreeMap<_, _> = cargo.get_envs().collect();
        let rustc = sysroot.join("bin").join(format!("rustc{}", std::env::consts::EXE_SUFFIX));
        assert_eq!(vars.get(std::ffi::OsStr::new("RUSTC")), Some(&Some(rustc.as_os_str())));
        for key in ["CARGO", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RA_JIT_BUILD_SCRIPT", "RA_JIT_SCRIPT_RUSTC", "CARGO_TARGET_DIR", "RUSTFLAGS"] {
            assert!(!vars.contains_key(std::ffi::OsStr::new(key)), "nested compiler must not receive {key}");
        }
    }

    #[test]
    fn working_builds_skip_fat_lto() {
        assert_eq!(nested_web_profile("fast", ""), "web-size");
        assert_eq!(nested_web_profile("dev", ""), "web-fast");
        assert_eq!(nested_web_profile("release", ""), "web");
        assert_eq!(nested_web_profile("bench", ""), "web");
        // `--profile fast` still reports PROFILE=release on this toolchain.
        assert_eq!(
            nested_web_profile("release", "/work/targets/s9/fast/build/angelX-cockpit-hash/out"),
            "web-size"
        );
        assert_eq!(
            nested_web_profile("release", "/work/targets/s9/release/build/angelX-cockpit-hash/out"),
            "web"
        );
        assert_eq!(
            nested_web_profile(
                "debug",
                "/work/targets/s9/wasm32-unknown-unknown/release/build/pkg/out"
            ),
            "web"
        );
        assert_eq!(
            nested_web_profile("debug", "/work/targets/s9/debug/build/pkg/out"),
            "web-fast"
        );
    }

    #[test]
    fn normal_builds_keep_browser_functionality_and_skip_requires_explicit_opt_out() {
        assert_eq!(skip_web_bundle(None, "debug", ""), Ok(false));
        assert_eq!(skip_web_bundle(None, "release", "/tmp/target/fast/build/pkg/out"), Ok(false));
        assert_eq!(skip_web_bundle(None, "release", ""), Ok(false));
        assert_eq!(skip_web_bundle(None, "bench", ""), Ok(false));
        assert_eq!(skip_web_bundle(Some("0"), "debug", ""), Ok(false));
        assert_eq!(skip_web_bundle(Some("0"), "release", "/tmp/target/fast/build/pkg/out"), Ok(false));
        assert_eq!(skip_web_bundle(Some("1"), "debug", ""), Ok(true));
        assert_eq!(
            skip_web_bundle(Some("1"), "release", "/tmp/target/fast/build/pkg/out"),
            Ok(true)
        );
        assert!(
            skip_web_bundle(Some("1"), "release", "/tmp/target/release/build/pkg/out").is_err()
        );
        assert!(skip_web_bundle(Some("1"), "bench", "").is_err());
        assert!(
            skip_web_bundle(Some("1"), "release", "/tmp/target/production/build/pkg/out").is_err()
        );
        assert!(
            skip_web_bundle(Some("1"), "release", "/tmp/target/debug/build/pkg/out").is_err()
        );
        assert!(
            skip_web_bundle(
                Some("1"),
                "release",
                "/tmp/target/fast/prod-linux-x/build/pkg/out"
            )
            .is_err()
        );
        assert!(skip_web_bundle(Some("1"), "", "").is_err());
    }

    #[test]
    fn a_missing_path_is_not_watched() {
        let missing = Path::new("/this/path/does/not/exist/angel-build-watch");
        assert!(!should_watch(missing));
        assert!(should_watch(Path::new(env!("CARGO_MANIFEST_DIR"))));
    }
}

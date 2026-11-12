//! The policy generations compiled into the binary (feature `v8`): every
//! `kind: policy` module of `cockpit/live`, embedded by `build.rs` with the
//! live loader's capture rules. A process with no live generation active serves
//! these, so a policy never lacks an implementation. Both the cockpit and the
//! Delve rules worker carry this table and evaluate it on the Kernel's V8
//! backend.
use crate::{Program, shipped_program};
use serde_json::Value;
use std::sync::{LazyLock, OnceLock};

include!(concat!(env!("OUT_DIR"), "/shipped_legs.rs"));

static PROGRAMS: LazyLock<Vec<OnceLock<Program>>> =
    LazyLock::new(|| SHIPPED_LEGS.iter().map(|_| OnceLock::new()).collect());

/// The ids of the shipped modules, in id order.
pub fn ids() -> impl Iterator<Item = &'static str> {
    SHIPPED_LEGS.iter().map(|(id, _)| *id)
}

/// The shipped generation of module `id`, checked like a loaded one (receipt,
/// probe, contract) on first use. A shipped module that cannot load is a build
/// defect, not a runtime condition.
pub fn program(id: &str) -> Option<&'static Program> {
    let index = SHIPPED_LEGS.iter().position(|(name, _)| *name == id)?;
    Some(PROGRAMS[index].get_or_init(|| {
        shipped_program(
            id,
            SHIPPED_LEGS[index].1,
            crate::v8::evaluate,
            |_, _| Ok(()),
        )
        .unwrap_or_else(|error| panic!("shipped leg {id} does not load: {error}"))
    }))
}

/// One render of the shipped module `id`, as an `angel_delve::scripted::Engine`
/// (`fn(&str, &Value) -> Result<Value, String>`). Shipped generation only: a
/// process with a live-module loader prefers its newest live generation and
/// uses this as the fallback.
pub fn engine(id: &str, input: &Value) -> Result<Value, String> {
    let program = program(id).ok_or_else(|| format!("no shipped module named {id}"))?;
    // One retry: a run terminated by the evaluator's deadline rebuilds its isolate.
    program.render(input).or_else(|_| program.render(input))
}

#[cfg(test)]
mod capture_contract_tests {
    use super::*;

    #[test]
    fn build_capture_rejects_the_same_unshippable_trees_as_live_capture() {
        let root =
            std::env::temp_dir().join(format!("angel-shipped-capture-{}", std::process::id()));
        assert!(!root.exists(), "fixture must never overwrite user files");
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let seed = || {
            std::fs::write(
                root.join("module.json"),
                r#"{"api_version":1,"id":"fixture","kind":"policy","entry":"index.js"}"#,
            )
            .unwrap();
            std::fs::write(root.join("index.js"), "module.exports={}").unwrap();
        };
        let reject = |expected: &str| {
            assert_eq!(
                super::build_capture::capture_paths(&root).unwrap_err(),
                expected
            );
            assert_eq!(
                crate::bundle::Bundle::read(&root, "fixture").unwrap_err(),
                expected
            );
        };
        seed();
        for n in 0..63 {
            std::fs::write(root.join(format!("{n}.json")), "{}").unwrap();
        }
        reject("too many module files");
        for n in 0..63 {
            std::fs::remove_file(root.join(format!("{n}.json"))).unwrap();
        }
        std::fs::write(root.join("large.json"), " ".repeat(1024 * 1024)).unwrap();
        reject("module generation exceeds 1024 KiB");
        std::fs::remove_file(root.join("large.json")).unwrap();
        for n in 0..255 {
            std::fs::write(root.join(format!(".ignored{n}")), "").unwrap();
        }
        reject("module dependency tree exceeds 256 entries");
        for n in 0..255 {
            std::fs::remove_file(root.join(format!(".ignored{n}"))).unwrap();
        }
        let mut deep = root.clone();
        for _ in 0..9 {
            deep.push("nested");
            std::fs::create_dir(&deep).unwrap();
        }
        reject("module dependency directory is too deep");
        std::fs::remove_dir_all(root.join("nested")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("index.js"), root.join("alias.js")).unwrap();
            reject("module inputs must be regular files");
            std::fs::remove_file(root.join("alias.js")).unwrap();
            std::os::unix::fs::symlink(&root, root.join("uncaptured-directory-link")).unwrap();
            assert!(super::build_capture::capture_paths(&root).is_ok());
            assert!(crate::bundle::Bundle::read(&root, "fixture").is_ok());
        }
    }

    #[test]
    fn policy_selection_requires_stable_manifest_and_matches_captured_bytes() {
        let policy =
            r#"{"api_version":1,"id":"fixture","kind":"policy","entry":"index.js"}"#;
        let non_policy =
            r#"{"api_version":1,"id":"fixture","kind":"text","entry":"index.js"}"#;
        assert_eq!(
            super::build_capture::stable_policy_manifest(policy, policy).unwrap(),
            Some(policy.to_owned())
        );
        assert_eq!(
            super::build_capture::stable_policy_manifest(non_policy, non_policy).unwrap(),
            None
        );
        for (first, second) in [(policy, non_policy), (non_policy, policy)] {
            assert!(super::build_capture::stable_policy_manifest(first, second)
                .unwrap_err()
                .contains("manifest changed"));
        }
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            "module.json".to_owned(),
            (std::path::PathBuf::from("module.json"), policy.to_owned()),
        );
        assert!(super::build_capture::captured_manifest_matches(policy, &files));
        assert!(!super::build_capture::captured_manifest_matches(non_policy, &files));
    }

    #[test]
    fn non_policy_module_is_skipped_before_policy_file_and_byte_budgets() {
        let root = std::env::temp_dir().join(format!(
            "angel-non-policy-capture-{}",
            std::process::id()
        ));
        assert!(!root.exists(), "fixture must never overwrite user files");
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        std::fs::write(
            root.join("module.json"),
            r#"{"api_version":1,"id":"fixture","kind":"text","entry":"index.js"}"#,
        )
        .unwrap();
        std::fs::write(root.join("index.js"), " ".repeat(1024 * 1024 + 1)).unwrap();
        for n in 0..64 {
            std::fs::write(root.join(format!("{n}.json")), "{}").unwrap();
        }
        assert!(super::build_capture::capture_policy_sources(&root)
            .unwrap()
            .is_none());
    }

    #[test]
    fn embedded_policy_inputs_match_live_capture_including_json_dependencies() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/live");
        let mut json_dependencies = 0;
        for (id, files) in SHIPPED_LEGS {
            let embedded = crate::bundle::Bundle::from_files(id, files).unwrap();
            let directory = crate::bundle::Bundle::read(&root.join(id), id).unwrap();
            assert_eq!(
                embedded.digest, directory.digest,
                "shipped/live policy capture differs for {id}; JSON dependencies belong to the generation too"
            );
            json_dependencies += files
                .iter()
                .filter(|(name, _)| {
                    name.ends_with(".json") && *name != "module.json" && *name != "bend-build.json"
                })
                .count();
        }
        assert!(
            json_dependencies > 0,
            "exercise real policy JSON dependencies rather than only module/build manifests"
        );
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../build.rs"]
mod build_capture;

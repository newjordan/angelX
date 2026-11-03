//! Adapt the independent live-module host to the cockpit's existing V8 engine.
pub(crate) use angel_live::{Client, RealmStyle, Snapshot, UiPolicy};

/// The module root a cockpit loads: `ANGEL_LIVE_DIR`, else the resource
/// bundle's `cockpit/live`.
pub(crate) fn root() -> std::path::PathBuf {
    std::env::var_os("ANGEL_LIVE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| crate::platform::runtime_paths::cockpit_dir().join("live"))
}

pub(crate) fn enabled() -> bool {
    std::env::var("ANGEL_HOTLOAD").as_deref() != Ok("0")
}

pub(crate) fn start(root: std::path::PathBuf) -> Client {
    Client::start_with_resources(root, evaluate, validate_host_contract, bind_resource)
}

/// Record the active generations for run identity. Whoever owns a loader calls
/// this when its snapshot changes; turns pin what was published at their start.
pub(crate) fn publish_legs(root: &std::path::Path, snapshot: &Snapshot) {
    super::harness::run_identity::publish_live(
        super::harness::run_identity::LiveSet::from_statuses(Some(root), &snapshot.statuses),
    );
}

/// Headless `--task` runs have no App to own a loader. An explicit
/// `ANGEL_LIVE_DIR` starts one (unless `ANGEL_HOTLOAD=0`), so a bench run names
/// the exact legs it measures; unset, headless runs load no live modules.
static HEADLESS: std::sync::OnceLock<Client> = std::sync::OnceLock::new();
const HEADLESS_DISCOVERY: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) fn start_headless() {
    let Some(root) = std::env::var_os("ANGEL_LIVE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    if std::env::var("ANGEL_HOTLOAD").as_deref() != Ok("0") {
        HEADLESS.get_or_init(|| start(root));
    }
}

/// At each turn start: pin the loader's latest snapshot onto the registry's
/// tool router and into run identity. The first call waits (bounded) for the
/// initial discovery, so the turn never starts on a half-loaded directory.
pub(crate) fn pin_headless_legs(registry: &super::harness::ToolRegistry) {
    let Some(client) = HEADLESS.get() else {
        return;
    };
    let until = std::time::Instant::now() + HEADLESS_DISCOVERY;
    let snapshot = loop {
        match client.snapshot() {
            Some(snapshot) if snapshot.revision > 0 => break Some(snapshot),
            _ if std::time::Instant::now() >= until => break None,
            _ => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    };
    match snapshot {
        Some(snapshot) => {
            registry.set_live_tool_router(snapshot.tool_router.clone());
            registry.set_live_policies(snapshot.policies.clone());
            super::harness::live_policy::offer(&snapshot.policies);
            if registry.live_root().is_none() {
                registry.set_live_outcomes(Some((client.root.clone(), client.board())));
            }
            publish_legs(&client.root, &snapshot);
        }
        None => eprintln!(
            "[angel] live modules at {} did not finish discovery in {}s; this turn runs without them",
            client.root.display(),
            HEADLESS_DISCOVERY.as_secs()
        ),
    }
}

pub(crate) fn bind_resource(
    contract: &str,
    name: &str,
    value: &serde_json::Value,
    assets: &std::collections::BTreeMap<String, std::sync::Arc<angel_live::Asset>>,
    factory: &angel_live::kernel::Candidate,
    imports: &angel_live::kernel::Resources,
) -> Result<angel_live::kernel::Binding, String> {
    if let Some(binding) =
        crate::drive::together_loadouts::bind(contract, name, value, assets, factory, imports)?
    {
        Ok(binding)
    } else if contract == crate::drive::together_appearance::CONTRACT {
        character_resource(name, value, assets)
    } else if contract == crate::drive::together_abilities::CONTRACT {
        crate::drive::together_abilities::bind(name, value, assets, factory)
    } else if [
        "character-appearance/",
        "game-abilities/",
        "character-loadout/",
        "ability-pack/",
        "character-art-pack/",
        "sound-pack/",
        "effect-pack/",
    ]
    .iter()
    .any(|prefix| contract.starts_with(prefix))
    {
        Err(format!("unsupported host resource contract: {contract}"))
    } else {
        Ok(std::sync::Arc::new(()))
    }
}
fn character_resource(
    name: &str,
    value: &serde_json::Value,
    assets: &std::collections::BTreeMap<String, std::sync::Arc<angel_live::Asset>>,
) -> Result<angel_live::kernel::Binding, String> {
    if value["player"]
        .as_u64()
        .is_none_or(|player| name != format!("character-seat-{player}"))
    {
        return Err("character resource name must match its player seat".into());
    }
    crate::drive::together_appearance::bind(value, assets)
}

fn validate_host_contract(kind: &str, value: &serde_json::Value) -> Result<(), String> {
    if kind == "game" {
        let policy = serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        crate::drive::together_sim::validate_live_rules(&policy)?;
    }
    Ok(())
}

/// Live-module evaluation is the Kernel's V8 backend: a small pool of threads
/// that each keep one warm isolate; every call still runs in a fresh context
/// with the same 200 ms and 32 MiB bounds. Callers on any thread wait for
/// their own reply.
pub(crate) use angel_live::v8::evaluate;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn character_module_publishes_prepared_native_resources_through_its_registered_command() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
        let mut registry = angel_live::Registry::with_resources(
            root,
            evaluate,
            validate_host_contract,
            bind_resource,
        );
        registry.refresh(true);
        let response = registry
            .dispatch(
                angel_live::kernel::Kind::Command,
                "character",
                &serde_json::json!({"type":"command","name":"character","arguments":"gold 2"}),
            )
            .unwrap();
        assert_eq!(response["output"], "character gold selected for player 2");
        let snapshot = registry.snapshot();
        let appearance = snapshot.resources["abilities/character-seat-2"]
            .binding::<crate::drive::together_abilities::Loadout>()
            .unwrap()
            .appearance
            .as_ref()
            .unwrap();
        let mut run = angel_delve::rules::Run::new(3, 9, Some("Two"));
        assert!(appearance.install(&mut run));
        assert_eq!(
            run.players[&2].avatar.as_ref().unwrap().colour,
            [225, 165, 50]
        );
        assert!(
            character_resource(
                "another-seat",
                &serde_json::json!({"player":2,"disabled":true}),
                &Default::default()
            )
            .is_err()
        );
    }

    fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    #[test]
    fn a_hot_edited_leg_changes_the_run_identity_and_bend_legs_name_their_receipt() {
        use crate::agent::harness::run_identity::{LiveSet, combined_sha256, static_identity};
        let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
        let root = std::env::temp_dir().join(format!("angel-leg-identity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        copy_tree(&shipped, &root);
        let mut registry =
            angel_live::Registry::validated(root.clone(), evaluate, validate_host_contract);
        registry.refresh(true);
        let before = LiveSet::from_statuses(Some(&root), &registry.snapshot().statuses);
        // Every Bend leg names the receipt its emitted code was built from.
        for leg in &before.legs {
            let receipt = shipped.join(&leg.id).join("bend-build.json");
            assert_eq!(leg.bend.is_some(), receipt.exists(), "{}", leg.id);
            if let Some(bend) = &leg.bend {
                let receipt: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(receipt).unwrap()).unwrap();
                assert_eq!(bend.source_digest, receipt["source_digest"], "{}", leg.id);
                assert_eq!(bend.output_digest, receipt["output_digest"], "{}", leg.id);
            }
        }
        assert!(before.legs.iter().any(|leg| leg.id == "ability-code" && leg.bend.is_some()));
        let router = root.join("tool-router/index.js");
        let edited = std::fs::read_to_string(&router)
            .unwrap()
            .replace("slice(0, max)", "slice(0, 1)");
        std::fs::write(&router, edited).unwrap();
        registry.refresh(true);
        let after = LiveSet::from_statuses(Some(&root), &registry.snapshot().statuses);
        let _ = std::fs::remove_dir_all(&root);
        let build = static_identity().unwrap();
        assert_ne!(before.sha256, after.sha256);
        assert_ne!(
            combined_sha256(build, &before),
            combined_sha256(build, &after)
        );
        let changed: Vec<_> = before
            .legs
            .iter()
            .zip(&after.legs)
            .filter(|(old, new)| old.digest != new.digest)
            .map(|(old, _)| old.id.as_str())
            .collect();
        assert_eq!(changed, ["tool-router"]);
        assert_eq!(
            after
                .legs
                .iter()
                .find(|leg| leg.id == "tool-router")
                .unwrap()
                .generation,
            2
        );
        assert_eq!(
            before.legs.iter().find(|leg| leg.id == "ability-code"),
            after.legs.iter().find(|leg| leg.id == "ability-code")
        );
    }

    // The loader is process-global once started, so the headless path runs in a
    // child of this binary that owns ANGEL_LIVE_DIR.
    #[test]
    fn headless_turns_pin_the_directory_named_by_angel_live_dir() {
        use crate::agent::club::{ChatMsg, Club};
        use crate::agent::harness::{ToolRegistry, TurnEvent};
        let _env = crate::tests::env_lock();
        let name =
            "agent::hotload::tests::headless_turns_pin_the_directory_named_by_angel_live_dir";
        if std::env::var("ANGEL_T_HEADLESS_LIVE").as_deref() != Ok("1") {
            let root =
                std::env::temp_dir().join(format!("angel-headless-legs-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
            copy_tree(&shipped.join("tool-router"), &root.join("live/tool-router"));
            let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
            cmd.args(["--exact", name, "--nocapture"])
                .current_dir(&root)
                .env("ANGEL_T_HEADLESS_LIVE", "1")
                .env("ANGEL_LIVE_DIR", root.join("live"))
                .env("ANGEL_EXPERIENCE", "0")
                .env("ANGEL_SKILL_HINT", "0")
                .env("ANGEL_AUTO_RECALL", "0")
                .env("ANGEL_AUTO_COMPACT", "0")
                .env("ANGEL_HARNESS_ROLLOUTS", "off")
                .env("ANGEL_CONTEXT_BUDGET_TOKENS", "4096")
                .env("ANGEL_NEEDS_PRO", "0")
                .env_remove("ANGEL_TASK_ACCEPT_CMD")
                .env_remove("ANGEL_FIRST_WRITE_CALLS");
            for key in [
                "ANGEL_ATLAS_DIR",
                "ANGEL_CADDY_DIR",
                "ANGEL_DOSSIER_DIR",
                "ANGEL_TRAJECTORY_DIR",
            ] {
                cmd.env(key, root.join(key));
            }
            let output = cmd.output().unwrap();
            let _ = std::fs::remove_dir_all(&root);
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        struct Scripted;
        impl Club for Scripted {
            fn respond(&self, _: &str) -> Result<String, String> {
                Ok("done".into())
            }
            fn label(&self) -> &str {
                "legs-scripted"
            }
            fn model_identity(&self) -> Option<String> {
                Some("legs-fixture".into())
            }
        }
        let live = std::path::PathBuf::from(std::env::var_os("ANGEL_LIVE_DIR").unwrap());
        start_headless();
        let registry = ToolRegistry::new();
        assert!(!registry.tool_router_changed());
        let turn = || {
            let mut history = vec![ChatMsg::user("Say done.")];
            crate::agent::harness::run_turn_observed(
                &Scripted,
                &registry,
                &mut history,
                &std::sync::atomic::AtomicBool::new(false),
                Some(3),
                &std::sync::mpsc::channel::<TurnEvent>().0,
            )
            .unwrap();
            crate::agent::harness::eval_trajectory_record(
                "legs-scripted",
                &history,
                "done",
                1.0,
                "fixture",
                1,
            )["identity"]
                .clone()
        };
        let first = turn();
        assert!(
            registry.tool_router_changed(),
            "the loaded router is installed"
        );
        let leg = &first["live"]["legs"][0];
        assert_eq!(first["live"]["legs"].as_array().unwrap().len(), 1);
        assert_eq!(first["live"]["root"], live.to_string_lossy().as_ref());
        assert_eq!(
            (leg["id"].as_str(), leg["generation"].as_u64()),
            (Some("tool-router"), Some(1))
        );
        assert_eq!(leg["digest"].as_str().unwrap().len(), 64);
        assert!(first["combined_sha256"].as_str().unwrap().len() == 64);
        let router = live.join("tool-router/index.js");
        let edited = std::fs::read_to_string(&router)
            .unwrap()
            .replace("slice(0, max)", "slice(0, 1)");
        std::fs::write(&router, edited).unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        let next = loop {
            let identity = turn();
            if identity["live"]["legs"][0]["generation"] == 2 {
                break identity;
            }
            assert!(
                Instant::now() < until,
                "the edited leg never reached a later turn"
            );
            std::thread::sleep(Duration::from_millis(100));
        };
        assert_ne!(next["live"]["sha256"], first["live"]["sha256"]);
        assert_ne!(next["combined_sha256"], first["combined_sha256"]);
        assert_eq!(next["executable_sha256"], first["executable_sha256"]);
        assert_eq!(
            next["cockpit_source_sha256"],
            first["cockpit_source_sha256"]
        );
    }

    #[test]
    fn shipped_generations_execute_in_the_embedded_engine() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
        let shipped = std::fs::read_dir(&root)
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().path().join("module.json").exists())
            .count();
        let mut registry = angel_live::Registry::validated(root, evaluate, validate_host_contract);
        registry.refresh(true);
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.statuses.len(), shipped);
        for status in &snapshot.statuses {
            assert_eq!(status.generation, 1, "{status:?}");
            assert!(status.error.is_none(), "{status:?}");
        }
        assert_eq!(snapshot.realm, Some(RealmStyle::default()));
        assert_eq!(
            registry
                .render(
                    "peers",
                    &serde_json::json!({"peers":[],"records":[],"directory":"test","now_ms":0})
                )
                .unwrap(),
            "peer directory test\nno peer angelX instances are publishing heartbeats here"
        );
    }
}

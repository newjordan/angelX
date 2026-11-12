//! Adapt the independent live-module host to the cockpit's existing V8 engine.
#[cfg(feature = "glass")]
pub(crate) mod showcase_squire;
use crate::app::control::realm::GameHost;
use angel_plug::drive::DriveWorld;
pub(crate) use angel_live::{Client, Snapshot, UiPolicy};
#[cfg(test)]
pub(crate) use angel_live::RealmStyle;

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
        <GameHost as DriveWorld>::bind_resource(contract, name, value, assets, factory, imports)?
    {
        return Ok(binding);
    }
    if contract == "glass-card-motion/1" {
        #[cfg(feature = "glass")]
        { return showcase_squire::bind_motion(value); }
        #[cfg(not(feature = "glass"))]
        { return Ok(std::sync::Arc::new(())); }
    }
    if matches!(contract, "glass-squire-clip/1" | "glass-squire-clip/2") {
        return bind_showcase_squire_versioned(contract, name, value, factory);
    }
    if [
        "glass-squire-clip/",
        "glass-card-motion/",
        "character-appearance/",
        "game-abilities/",
        "character-loadout/",
        "ability-pack/",
        "character-art-pack/",
        "character-animation-pack/",
        "character-animation-graph-pack/",
        "character-graph-loadout/",
        "character-animated-loadout/",
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
fn bind_showcase_squire_versioned(
    contract: &str,
    name: &str,
    value: &serde_json::Value,
    factory: &angel_live::kernel::Candidate,
) -> Result<angel_live::kernel::Binding, String> {
    #[cfg(feature = "glass")]
    {
        showcase_squire::bind_live_versioned(contract, name, value, &factory.manifest)
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = (contract, name, value, factory);
        Ok(std::sync::Arc::new(()))
    }
}

#[cfg(feature = "glass")]
fn bind_showcase_squire(
    name: &str,
    value: &serde_json::Value,
) -> Result<angel_live::kernel::Binding, String> {
    showcase_squire::bind(name, value)
}

#[cfg(not(feature = "glass"))]
fn bind_showcase_squire(
    _name: &str,
    _value: &serde_json::Value,
) -> Result<angel_live::kernel::Binding, String> {
    Ok(std::sync::Arc::new(()))
}

pub(crate) fn validate_host_contract(
    kind: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    <GameHost as DriveWorld>::validate_contract(kind, value)
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

    #[cfg(feature = "glass")]
    #[test]
    fn bad_hot_edited_squire_clip_keeps_the_previous_prepared_generation() {
        use std::sync::Arc;
        let root = std::env::temp_dir().join(format!("angel-squire-pack-{}", std::process::id()));
        let module = root.join("showcase-squire");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(module.join("module.json"), r#"{"api_version":2,"id":"showcase-squire","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"glass-squire-clip/1"}]}"#).unwrap();
        std::fs::write(module.join("pack.json"), serde_json::json!({
            "palette": vec![[0,0,0]; 12],
            "frames": (0..9).map(|_| serde_json::json!({"pixels": vec![1; 320]})).collect::<Vec<_>>(),
            "idle": [{"frame":5,"millis":4500}],
            "wave": [{"frame":8,"millis":1000}]
        }).to_string()).unwrap();
        let source = "const pack=require('./pack.json'); module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state}),resources:()=>({pack})}";
        std::fs::write(module.join("index.js"), source).unwrap();
        let mut registry = angel_live::Registry::with_resources(
            root.clone(), evaluate, validate_host_contract, bind_resource,
        );
        registry.refresh(true);
        let first = registry.snapshot().resources["showcase-squire/pack"].clone();
        let first_clip = first.binding::<Arc<angel_glass::ShowcaseSquireClip>>().unwrap().clone();

        let mut bad: serde_json::Value = serde_json::from_slice(&std::fs::read(module.join("pack.json")).unwrap()).unwrap();
        bad["frames"][0]["pixels"] = serde_json::json!(vec![1; 319]);
        std::fs::write(module.join("pack.json"), bad.to_string()).unwrap();
        registry.refresh(true);
        let current = registry.snapshot().resources["showcase-squire/pack"].clone();
        let current_clip = current.binding::<Arc<angel_glass::ShowcaseSquireClip>>().unwrap();
        assert_eq!(current.generation, first.generation);
        assert!(Arc::ptr_eq(&first_clip, current_clip), "invalid pixels cannot replace the old resource lease");
        drop(registry);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "glass")]
    #[test]
    fn authored_card_track_edits_reject_invalid_candidates_and_retain_missing_sources() {
        use std::sync::Arc;
        let root = std::env::temp_dir().join(format!("angel-card-track-{}", std::process::id()));
        let module = root.join("showcase-squire");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(
            module.join("module.json"),
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("live/showcase-squire/module.json"),
            )
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            module.join("index.js"),
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/showcase-squire/index.js"),
            )
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            module.join("pack.json"),
            include_str!("../../live/showcase-squire/pack.json"),
        )
        .unwrap();
        let motion = root.join("showcase-card-motion");
        std::fs::create_dir_all(&motion).unwrap();
        std::fs::write(
            motion.join("module.json"),
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("live/showcase-card-motion/module.json"),
            )
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            motion.join("index.js"),
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("live/showcase-card-motion/index.js"),
            )
            .unwrap(),
        )
        .unwrap();
        let mut value: serde_json::Value = serde_json::json!({"card":serde_json::from_str::<serde_json::Value>(include_str!("../../live/showcase-card-motion/pack.json")).unwrap()});
        std::fs::write(motion.join("pack.json"), value["card"].to_string()).unwrap();
        let mut registry = angel_live::Registry::with_resources(
            root.clone(),
            evaluate,
            validate_host_contract,
            bind_resource,
        );
        registry.refresh(true);
        let first = registry.snapshot().resources["showcase-squire/pack"].clone();
        let clip = first
            .binding::<Arc<angel_glass::ShowcaseSquireClip>>()
            .unwrap()
            .clone();
        let (host, _hub) = angel_glass::remote::server::headless("card-tracks", Default::default());
        host.showcase_squire_clip(
            Some(angel_glass::ShowcaseClipIdentity {
                module: first.module.clone(),
                name: first.name.clone(),
                generation: first.generation,
                revision: first.revision,
                digest: first.identity.clone(),
            }),
            Some(clip.clone()),
        );
        for step in value["card"]["states"]["busy"]["steps"]
            .as_array_mut()
            .unwrap()
        {
            step["offset"][0] = serde_json::json!(17);
        }
        std::fs::write(motion.join("pack.json"), value["card"].to_string()).unwrap();
        registry.refresh(true);
        let edited = registry.snapshot().resources["showcase-squire/pack"].clone();
        let edited_clip = edited
            .binding::<Arc<angel_glass::ShowcaseSquireClip>>()
            .unwrap()
            .clone();
        assert!(edited.generation > first.generation);
        assert_eq!(edited_clip.frames, clip.frames);
        assert_ne!(edited_clip.card, clip.card);
        // Wire publication is bounded after prepared-data reserialization.
        let edited_identity = angel_glass::ShowcaseClipIdentity {
            module: edited.module.clone(),
            name: edited.name.clone(),
            generation: edited.generation,
            revision: edited.revision,
            digest: edited.identity.clone(),
        };
        let update = angel_glass::remote::wire::SquireClipUpdate::capture(Some(&(
            edited_identity.clone(),
            edited_clip.clone(),
        )))
        .unwrap();
        assert!(update.pack.as_ref().unwrap().len() <= 64 * 1024);
        host.showcase_squire_clip(Some(edited_identity), Some(edited_clip.clone()));
        let working = value.clone();
        value["card"]["states"]["busy"]["steps"][0]["millis"] = serde_json::json!(0);
        std::fs::write(motion.join("pack.json"), value["card"].to_string()).unwrap();
        registry.refresh(true);
        let retained = registry.snapshot().resources["showcase-squire/pack"].clone();
        assert_eq!(retained.generation, edited.generation);
        assert!(Arc::ptr_eq(
            retained
                .binding::<Arc<angel_glass::ShowcaseSquireClip>>()
                .unwrap(),
            &edited_clip
        ));
        // A structurally valid oversized track also fails before replacing the lease.
        value["card"]["states"]["busy"]["steps"][0]["millis"] = serde_json::json!(1);
        let steps = value["card"]["states"]["busy"]["steps"]
            .as_array_mut()
            .unwrap();
        let step = steps[0].clone();
        steps.resize(1500, step);
        let mut oversized: serde_json::Value =
            serde_json::from_str(include_str!("../../live/showcase-squire/pack.json")).unwrap();
        oversized["card"] = value["card"].clone();
        assert!(
            showcase_squire::prepare(&oversized)
                .unwrap_err()
                .contains("64 KiB")
        );
        std::fs::write(motion.join("pack.json"), value["card"].to_string()).unwrap();
        registry.refresh(true);
        assert_eq!(
            registry.snapshot().resources["showcase-squire/pack"].generation,
            edited.generation
        );
        std::fs::remove_dir_all(&motion).unwrap();
        registry.refresh(true);
        assert_eq!(
            registry.snapshot().resources["showcase-squire/pack"].generation,
            edited.generation,
            "required dependency disappearance retains the whole last-good /2 lease"
        );
        assert!(registry.snapshot().statuses.iter().any(|status| status.id == "showcase-card-motion" && status.error.is_some()));
        std::fs::create_dir_all(&motion).unwrap();
        std::fs::write(
            motion.join("module.json"),
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("live/showcase-card-motion/module.json"),
            )
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            motion.join("index.js"),
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("live/showcase-card-motion/index.js"),
            )
            .unwrap(),
        )
        .unwrap();
        std::fs::write(motion.join("pack.json"), working["card"].to_string()).unwrap();
        registry.refresh(true);
        assert_eq!(
            registry.snapshot().resources["showcase-squire/pack"]
                .binding::<Arc<angel_glass::ShowcaseSquireClip>>()
                .unwrap()
                .card,
            edited_clip.card
        );
        std::fs::remove_dir_all(&module).unwrap();
        registry.refresh(true);
        let missing = registry.snapshot();
        assert!(missing.resources.contains_key("showcase-squire/pack"), "source disappearance is not withdrawal");
        assert!(missing.statuses.iter().any(|status| status.id == "showcase-squire" && status.error.is_some()));
        host.showcase_squire_clip(None, None);
        std::fs::remove_dir_all(root).unwrap();
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

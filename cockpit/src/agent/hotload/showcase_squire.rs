use angel_glass::{ShowcaseClipStep, ShowcaseSquireClip};
use serde_json::Value;

pub(super) const CONTRACT: &str = "glass-squire-clip/2";
pub(super) const LEGACY: &str = "glass-squire-clip/1";
const MAX_STARTUP_PACK_BYTES: u64 = 1024 * 1024;

const MAX_STARTUP_MANIFEST_BYTES: u64 = 16 * 1024;
const MAX_STARTUP_ENTRIES: usize = 128;
const MAX_STARTUP_DISCOVERY_BYTES: usize = 8 * 1024 * 1024;
const STARTUP_DISCOVERY_TIME: std::time::Duration = std::time::Duration::from_millis(50);

struct StartupBudget { bytes: usize, started: std::time::Instant }
impl StartupBudget {
    fn new() -> Self {Self{bytes:MAX_STARTUP_DISCOVERY_BYTES,started:std::time::Instant::now()}}
    fn read(&mut self,path:&std::path::Path,limit:u64)->Result<Option<Vec<u8>>,()> {
        use std::io::Read;
        if self.started.elapsed()>STARTUP_DISCOVERY_TIME {return Err(())}
        let file=match std::fs::File::open(path) {Ok(file)=>file,Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(None),Err(_)=>return Err(())};
        let mut bytes=Vec::new();file.take(limit+1).read_to_end(&mut bytes).map_err(|_|())?;
        self.bytes=self.bytes.checked_sub(bytes.len()).ok_or(())?;
        if bytes.len() as u64>limit || self.started.elapsed()>STARTUP_DISCOVERY_TIME {return Err(())}
        Ok(Some(bytes))
    }
}
fn startup_entries(root: &std::path::Path) -> Option<Vec<std::path::PathBuf>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(root).ok()?.take(MAX_STARTUP_ENTRIES + 1) {
        let entry = entry.ok()?;
        entries.push(entry.path());
    }
    if entries.len() > MAX_STARTUP_ENTRIES {
        return None;
    }
    entries.sort();
    Some(entries)
}

/// Bootstrap is an asset-only preview, replaced by the Registry's validated
/// publication. Discovery is bounded and checks both selected bytes and the
/// complete directory/manifest witness before installing a preview.
pub(crate) fn startup_clip(
    root: &std::path::Path,
) -> Option<(
    angel_glass::ShowcaseClipIdentity,
    std::sync::Arc<ShowcaseSquireClip>,
)> {
    startup_clip_observing(root, || {})
}
fn startup_clip_observing(
    root: &std::path::Path,
    before_verify: impl FnOnce(),
) -> Option<(
    angel_glass::ShowcaseClipIdentity,
    std::sync::Arc<ShowcaseSquireClip>,
)> {
    let mut budget = StartupBudget::new();
    let entries = startup_entries(root)?;
    let path = root.join("showcase-squire/pack.json");
    let art_bytes = budget.read(&path, MAX_STARTUP_PACK_BYTES).ok()??;
    let mut value: Value = serde_json::from_slice(&art_bytes).ok()?;
    let mut assets = vec![(path, Some(art_bytes))];
    let mut manifests = Vec::new();
    for entry in &entries {
        if !entry.is_dir() {
            continue;
        }
        let path = entry.join("module.json");
        let bytes = budget.read(&path, MAX_STARTUP_MANIFEST_BYTES).ok()?;
        manifests.push((path, bytes));
    }
    let art_manifest_path = root.join("showcase-squire/module.json");
    let manifest = manifests
        .iter()
        .find(|(path, _)| path == &art_manifest_path)
        .and_then(|(_, bytes)| bytes.as_ref())
        .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok())
        .unwrap_or(Value::Null);
    let needs_motion = manifest["exports"]
        .as_array()
        .is_some_and(|exports| exports.iter().any(|export| export["contract"] == CONTRACT));
    if !needs_motion {value.as_object_mut()?.remove("card");}
    if value.get("card").is_none() {
        let requested: Vec<_> = manifest["imports"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|import| import["contract"] == "glass-card-motion/1")
            .filter_map(|import| import["module"].as_str())
            .take(2)
            .collect();
        if requested.len() > 1 {
            return None;
        }
        let requested = requested.first().copied();
        let mut candidates = Vec::new();
        for (path, bytes) in &manifests {
            if requested.is_some_and(|name| {
                path.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    != Some(name)
            }) {
                continue;
            }
            let Some(bytes) = bytes else { continue };
            let Ok(manifest) = serde_json::from_slice::<Value>(bytes) else {
                continue;
            };
            if manifest["exports"].as_array().is_some_and(|exports| {
                exports.iter().any(|export| {
                    export["kind"] == "resource"
                        && export["name"] == "pack"
                        && export["contract"] == "glass-card-motion/1"
                })
            }) {
                candidates.push(path.parent()?.join("pack.json"));
            }
        }
        if let [motion_path] = candidates.as_slice() {
            let motion_bytes = budget.read(motion_path, MAX_STARTUP_PACK_BYTES).ok()?;
            assets.push((motion_path.clone(), motion_bytes.clone()));
            let motion = motion_bytes.and_then(|bytes| {
                let value: Value = serde_json::from_slice(&bytes).ok()?;
                bind_motion(&value).ok()?;
                Some((value, bytes))
            });
            if let Some((motion, _bytes)) = motion {
                value["card"] = motion;
            } else if needs_motion {
                return None;
            }
        } else if needs_motion {
            return None;
        }
    }
    before_verify();
    if startup_entries(root)? != entries {
        return None;
    }
    for (path, bytes) in &manifests {
        if budget.read(path, MAX_STARTUP_MANIFEST_BYTES).ok()? != *bytes {
            return None;
        }
    }
    for (path, bytes) in &assets {
        if budget.read(path, MAX_STARTUP_PACK_BYTES).ok()? != *bytes {
            return None;
        }
    }
    let clip = std::sync::Arc::new(prepare(&value).ok()?);
    if budget.started.elapsed() > STARTUP_DISCOVERY_TIME {
        return None;
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    for (path, bytes) in &assets {
        path.hash(&mut hash);
        bytes.hash(&mut hash)
    }
    for (path, bytes) in &manifests {
        path.hash(&mut hash);
        bytes.hash(&mut hash)
    }
    Some((
        angel_glass::ShowcaseClipIdentity {
            module: "showcase-squire".into(),
            name: "pack".into(),
            generation: 0,
            revision: 0,
            digest: format!("startup-{:016x}", hash.finish()),
        },
        clip,
    ))
}

/// Compose compatibility policy at admission, using exact identities of both
/// validated resources. Explicit imports take precedence; old /1 packs without
/// imports use the unique registered compatibility resource, regardless of ID.
/// A declared but absent optional motion import keeps art-only fallback.
pub(crate) fn live_clip(
    host: &angel_glass::Host,
    resources: &std::collections::BTreeMap<String, angel_live::kernel::Resource>,
) -> Option<(
    angel_glass::ShowcaseClipIdentity,
    std::sync::Arc<ShowcaseSquireClip>,
)> {
    let art = resources.values().find(|r| {
        r.module == "showcase-squire"
            && r.name == "pack"
            && [CONTRACT, LEGACY].contains(&r.contract.as_str())
    })?;
    let explicit_motion = art.binding::<ExplicitMotionArt>();
    let clip = explicit_motion.map(|bound| &bound.0)
        .or_else(|| art.binding::<std::sync::Arc<ShowcaseSquireClip>>())?;
    let mut identity = angel_glass::ShowcaseClipIdentity {
        module: art.module.clone(),
        name: art.name.clone(),
        generation: art.generation,
        revision: art.revision,
        digest: art.identity.clone(),
    };
    if clip.card.is_some() {
        return Some((identity, clip.clone()));
    }
    let imported: Vec<_> = art
        .imports()
        .iter()
        .map(|(_, r)| r)
        .filter(|r| r.contract == "glass-card-motion/1")
        .collect();
    let candidates: Vec<_> = if imported.is_empty() {
        if explicit_motion.is_some() {
            return Some((identity, clip.clone()));
        }
        resources
            .values()
            .filter(|r| r.contract == "glass-card-motion/1")
            .collect()
    } else {
        imported
    };
    let [motion] = candidates.as_slice() else {
        return Some((identity, clip.clone()));
    };
    let motion = *motion;
    let Some(prepared) = motion.binding::<angel_glass::CardMotion>() else {
        return Some((identity, clip.clone()));
    };
    let stamp = serde_json::json!([
        [
            art.module,
            art.name,
            art.identity,
            art.generation,
            art.revision
        ],
        [
            motion.module,
            motion.name,
            motion.identity,
            motion.generation,
            motion.revision
        ]
    ])
    .to_string();
    identity.digest = angel_live::host::sha256_hex(stamp.as_bytes());
    let current = host.showcase_squire_resource();
    if current.as_ref().is_some_and(|(old, _)| old == &identity) {
        return current;
    }
    let mut combined = (**clip).clone();
    combined.card = Some(prepared.clone());
    let combined = std::sync::Arc::new(combined);
    // Individually valid resources can exceed the legacy transport budget when
    // combined. Refuse motion and publish only the current validated art;
    // retaining an unrelated older composite would retain its retired owners.
    if let Err(error) = angel_glass::remote::wire::SquireClipUpdate::capture(Some(&(
        identity.clone(),
        combined.clone(),
    ))) {
        eprintln!("GLASS: card compatibility activation refused: {error}");
        return Some((
            angel_glass::ShowcaseClipIdentity {
                module: art.module.clone(),
                name: art.name.clone(),
                generation: art.generation,
                revision: art.revision,
                digest: art.identity.clone(),
            },
            clip.clone(),
        ));
    }
    Some((identity, combined))
}

pub(super) fn bind_motion(value: &Value) -> Result<angel_live::kernel::Binding, String> {
    let motion = angel_glass::CardMotion::from_json(value)?;
    // Reserve 16 KiB for the maximum canonical /1 palette, indexed art and
    // two 64-step clips. This is checked using prepared wire serialization.
    if serde_json::to_value(&motion)
        .map_err(|error| error.to_string())?
        .to_string()
        .len()
        > 48 * 1024
    {
        return Err("prepared card motion exceeds its 48 KiB compatibility budget".into());
    }
    Ok(std::sync::Arc::new(motion))
}

// api_version 2 manifests declare intent even when an optional provider is
// absent. Keep that declaration with the host binding; Glass's shared clip
// format and old /1 packs do not need a new field or wire version.
struct ExplicitMotionArt(std::sync::Arc<ShowcaseSquireClip>);

pub(super) fn bind_live_versioned(
    contract: &str,
    name: &str,
    value: &Value,
    manifest: &angel_live::kernel::Manifest,
) -> Result<angel_live::kernel::Binding, String> {
    let bound = bind_versioned(contract, name, value)?;
    if contract == LEGACY && manifest.imports.iter().any(|import| import.contract.starts_with("glass-card-motion/")) {
        let clip = bound.downcast_ref::<std::sync::Arc<ShowcaseSquireClip>>()
            .ok_or("legacy art binding has an unexpected host type")?;
        return Ok(std::sync::Arc::new(ExplicitMotionArt(clip.clone())));
    }
    Ok(bound)
}

pub(super) fn bind_versioned(contract:&str,name:&str,value:&Value)->Result<angel_live::kernel::Binding,String> {
    if contract==LEGACY {
        // /1 ignored unknown fields; a coincidental old editor `card` field
        // must not opt into /2 policy or disable an otherwise valid art pack.
        let mut art=value.clone();art.as_object_mut().ok_or("squire art needs an object")?.remove("card");
        return bind(name,&art);
    }
    if contract!=CONTRACT || value.get("card").is_none(){return Err("squire clip version and card motion disagree".into())}
    bind(name,value)
}

pub(super) fn bind(name: &str, value: &Value) -> Result<angel_live::kernel::Binding, String> {
    if name != "pack" {
        return Err("showcase squire resource must be named `pack`".into());
    }
    let clip = prepare(value)?;
    Ok(std::sync::Arc::new(std::sync::Arc::new(clip)))
}

pub(crate) fn prepare(value: &Value) -> Result<ShowcaseSquireClip, String> {
    ShowcaseSquireClip::from_json(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> Value {
        serde_json::json!({
            "palette": vec![[0,0,0]; 12],
            "frames": (0..9).map(|_| serde_json::json!({"pixels": vec![0; 320]})).collect::<Vec<_>>(),
            "idle": [{"frame":5,"millis":4500}],
            "wave": [{"frame":8,"millis":1000}]
        })
    }

    #[test]
    fn card_contract_versions_and_missing_startup_are_explicit() {
        let mut value: Value =
            serde_json::from_str(include_str!("../../../live/showcase-squire/pack.json")).unwrap();
        value["card"] =
            serde_json::from_str(include_str!("../../../live/showcase-card-motion/pack.json")).unwrap();
        assert!(bind_versioned(CONTRACT, "pack", &value).is_ok());
        let legacy=bind_versioned(LEGACY,"pack",&value).unwrap();
        assert!(legacy.downcast_ref::<std::sync::Arc<ShowcaseSquireClip>>().unwrap().card.is_none());
        assert!(bind_versioned(CONTRACT, "pack", &valid()).is_err());
        assert!(bind_versioned(LEGACY, "pack", &valid()).is_ok());
        assert!(bind_versioned("glass-squire-clip/3", "pack", &value).is_err());
        assert!(startup_clip(&std::env::temp_dir().join("angel-card-missing-root")).is_none());
    }

    #[test]
    fn startup_legacy_art_composes_unique_module_owned_motion_and_handles_removal() {
        let root =
            std::env::temp_dir().join(format!("angel-card-compat-startup-{}", std::process::id()));
        let art = root.join("showcase-squire");
        std::fs::create_dir_all(&art).unwrap();
        std::fs::write(art.join("pack.json"), valid().to_string()).unwrap();
        let neutral = startup_clip(&root).unwrap();
        assert!(neutral.1.card.is_none());
        let motion = root.join("custom-reactions");
        std::fs::create_dir_all(&motion).unwrap();
        std::fs::write(
            motion.join("module.json"),
            r#"{"exports":[{"kind":"resource","name":"pack","contract":"glass-card-motion/1"}]}"#,
        )
        .unwrap();
        let mut value: Value =
            serde_json::from_str(include_str!("../../../live/showcase-card-motion/pack.json")).unwrap();
        std::fs::write(motion.join("pack.json"), value.to_string()).unwrap();
        let composed = startup_clip(&root).unwrap();
        assert!(composed.1.card.is_some());
        assert_eq!(composed.1.frames, neutral.1.frames);
        value["states"]["guard"]["steps"][0]["frame"] = serde_json::json!(8);
        std::fs::write(motion.join("pack.json"), value.to_string()).unwrap();
        let edited = startup_clip(&root).unwrap();
        assert_ne!(edited.0, composed.0);
        assert_ne!(edited.1.card, composed.1.card);
        std::fs::write(motion.join("pack.json"), "{}").unwrap();
        assert!(
            startup_clip(&root).unwrap().1.card.is_none(),
            "cold malformed compatibility leaves legacy art available"
        );
        std::fs::remove_dir_all(&motion).unwrap();
        assert!(startup_clip(&root).unwrap().1.card.is_none());
        std::fs::write(art.join("module.json"),r#"{"exports":[{"kind":"resource","name":"pack","contract":"glass-squire-clip/2"}],"imports":[{"module":"custom-reactions","contract":"glass-card-motion/1"}]}"#).unwrap();
        assert!(
            startup_clip(&root).is_none(),
            "required /2 dependency fails closed at cold start"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_discovery_fails_closed_on_membership_or_manifest_races_and_bounds() {
        let root = std::env::temp_dir().join(format!("angel-card-discovery-{}", std::process::id()));
        let art = root.join("showcase-squire");
        std::fs::create_dir_all(&art).unwrap();
        std::fs::write(art.join("pack.json"), valid().to_string()).unwrap();
        assert!(
            startup_clip_observing(&root, || {
                std::fs::create_dir(root.join("late-provider")).unwrap()
            })
            .is_none()
        );
        std::fs::remove_dir(root.join("late-provider")).unwrap();
        assert!(
            startup_clip_observing(&root, || {
                std::fs::write(art.join("module.json"), "{}").unwrap()
            })
            .is_none(),
            "manifest absence is witnessed too"
        );
        let motion = root.join("selected-motion");
        std::fs::create_dir_all(&motion).unwrap();
        std::fs::write(
            motion.join("module.json"),
            r#"{"exports":[{"kind":"resource","name":"pack","contract":"glass-card-motion/1"}]}"#,
        )
        .unwrap();
        let bytes = include_bytes!("../../../live/showcase-card-motion/pack.json");
        std::fs::write(motion.join("pack.json"), bytes).unwrap();
        assert!(
            startup_clip_observing(&root, || std::fs::remove_file(motion.join("pack.json"))
                .unwrap())
            .is_none(),
            "selected asset removal is witnessed"
        );
        assert!(
            startup_clip_observing(&root, || std::fs::write(motion.join("pack.json"), bytes)
                .unwrap())
            .is_none(),
            "selected asset absence is witnessed before late appearance"
        );
        assert!(
            startup_clip_observing(&root, || std::fs::remove_dir_all(&motion).unwrap()).is_none(),
            "selected provider removal is witnessed"
        );
        std::fs::write(
            art.join("module.json"),
            vec![b' '; MAX_STARTUP_MANIFEST_BYTES as usize + 1],
        )
        .unwrap();
        assert!(startup_clip(&root).is_none());
        std::fs::remove_file(art.join("module.json")).unwrap();
        for index in 0..MAX_STARTUP_ENTRIES {
            std::fs::create_dir(root.join(format!("extra-{index}"))).unwrap()
        }
        assert!(
            startup_clip(&root).is_none(),
            "directory discovery cannot scan an unbounded module root"
        );
        let path = root.join("budget.bin");
        std::fs::write(&path, vec![0; 1024]).unwrap();
        let mut budget = StartupBudget::new();
        budget.bytes = 100;
        assert!(budget.read(&path, 1024).is_err());
        let mut expired = StartupBudget::new();
        expired.started -= STARTUP_DISCOVERY_TIME + std::time::Duration::from_millis(1);
        assert!(expired.read(&path, 1024).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "manual cold-bootstrap measurement; run without competing builds"]
    fn measure_module_owned_card_bootstrap() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
        let start = std::time::Instant::now();
        let clip = startup_clip(&root).expect("shipped declared dependency bootstraps");
        eprintln!(
            "card bootstrap first-call elapsed_us={} card_tracks={} bounded_entries={} manifest_limit={} total_read_limit={} cooperative_deadline_ms={}",
            start.elapsed().as_micros(),
            clip.1.card.is_some(),
            MAX_STARTUP_ENTRIES,
            MAX_STARTUP_MANIFEST_BYTES,
            MAX_STARTUP_DISCOVERY_BYTES,
            STARTUP_DISCOVERY_TIME.as_millis()
        );
    }

    #[test]
    fn rejects_incomplete_clips_before_a_binding_can_be_published() {
        let mut value = valid();
        value["frames"][4]["pixels"] = serde_json::json!(vec![13; 320]);
        assert!(prepare(&value).is_err());
        assert!(prepare(&valid()).is_ok());
    }

    #[test]
    fn startup_pack_uses_the_same_module_owned_resource_shape() {
        let root = std::env::temp_dir().join(format!("angel-squire-startup-{}", std::process::id()));
        let module = root.join("showcase-squire");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(module.join("pack.json"), valid().to_string()).unwrap();
        let (identity, clip) = startup_clip(&root).expect("valid module pack is available before hotload");
        assert_eq!((identity.module.as_str(), identity.name.as_str()), ("showcase-squire", "pack"));
        assert_eq!(clip.frames.len(), 9);
        std::fs::write(module.join("pack.json"), "{}").unwrap();
        assert!(startup_clip(&root).is_none(), "invalid authored data is safely absent");
        std::fs::write(module.join("pack.json"), vec![b' '; MAX_STARTUP_PACK_BYTES as usize + 1]).unwrap();
        assert!(startup_clip(&root).is_none(), "oversized startup packs are rejected before parsing");
        let _ = std::fs::remove_dir_all(root);
    }
}

use super::dossier::{AnchorFreshnessV1, DossierV1, EvidenceAnchorV1};
use super::dossier_validation::{
    anchor_shards_are_canonical, generation_revision, shard_count, state_sha,
};
use super::schema_validation::validate_sha256;
use super::store_fs::{
    OpenKind, StoreLease, open_private, read_bounded, reject_symlink, secure_dir, secure_file,
    sync_dir,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
const POINTER_SCHEMA_V1: &str = "angel.competition-dossier-pointer/v1";
const MANIFEST_SCHEMA_V1: &str = "angel.competition-dossier-manifest/v1";
const ANCHORS_PER_SHARD: usize = 64;
const MAX_POINTER_BYTES: u64 = 4 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_ANCHOR_BYTES: u64 = 64 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(crate) type DossierStoreError = String;
#[derive(Clone, Debug)]
pub(crate) struct DossierStoreV1 {
    root: PathBuf,
    generations: PathBuf,
    current: PathBuf,
    lock: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentPointerV1 {
    schema: String,
    generation: String,
    manifest_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DossierManifestV1 {
    schema: String,
    core: DossierV1,
    anchor_count: u64,
    shard_count: u64,
    state_sha256: String,
}
impl DossierStoreV1 {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            generations: root.join("generations"),
            current: root.join("current.json"),
            lock: root.join("dossier.lock"),
            root,
        }
    }
    pub(crate) fn sync(&self, dossier: &DossierV1) -> Result<(), DossierStoreError> {
        dossier.validate()?;
        self.prepare_root()?;
        let _lock = StoreLease::acquire(&self.lock)?;
        if let Some((_pointer, existing)) = self.load_locked()? {
            if dossier.dossier_revision < existing.dossier_revision {
                return Err("dossier revision rollback refused".into());
            }
            if dossier.dossier_revision == existing.dossier_revision {
                if dossier != &existing {
                    return Err("same dossier revision has conflicting content".into());
                }
                secure_file(&self.current)?;
                return sync_dir(&self.root);
            }
        }
        let state_sha256 = state_sha(dossier)?;
        let generation = format!("{:020}-{state_sha256}", dossier.dossier_revision);
        let manifest_sha256 = self.write_generation(dossier, &generation, &state_sha256)?;
        self.publish_current(&CurrentPointerV1 {
            schema: POINTER_SCHEMA_V1.into(),
            generation: generation.clone(),
            manifest_sha256,
        })?;
        Ok(())
    }
    pub(crate) fn load(&self) -> Result<Option<DossierV1>, DossierStoreError> {
        self.prepare_root()?;
        let _lock = StoreLease::acquire(&self.lock)?;
        self.load_locked()
            .map(|loaded| loaded.map(|(_, dossier)| dossier))
    }
    fn prepare_root(&self) -> Result<(), DossierStoreError> {
        if self.root.exists() {
            reject_symlink(&self.root)?;
        }
        fs::create_dir_all(&self.generations)
            .map_err(|error| format!("create dossier store: {error}"))?;
        secure_dir(&self.root)?;
        secure_dir(&self.generations)
    }
    fn load_locked(&self) -> Result<Option<(CurrentPointerV1, DossierV1)>, DossierStoreError> {
        if !self.current.exists() {
            return Ok(None);
        }
        reject_symlink(&self.current)?;
        let raw = read_bounded(&self.current, MAX_POINTER_BYTES)?;
        let pointer: CurrentPointerV1 = serde_json::from_slice(&raw)
            .map_err(|error| format!("parse dossier pointer: {error}"))?;
        if pointer.schema != POINTER_SCHEMA_V1
            || generation_revision(&pointer.generation).is_none()
            || validate_sha256(&pointer.manifest_sha256).is_err()
        {
            return Err("invalid dossier pointer".into());
        }
        let dossier = self.load_generation(&pointer)?;
        Ok(Some((pointer, dossier)))
    }
    fn load_generation(&self, pointer: &CurrentPointerV1) -> Result<DossierV1, DossierStoreError> {
        let root = self.generations.join(&pointer.generation);
        reject_symlink(&root)?;
        let raw = read_bounded(&root.join("manifest.json"), MAX_MANIFEST_BYTES)?;
        if crate::knowledge::cut::sha256_hex(&raw) != pointer.manifest_sha256 {
            return Err("dossier manifest digest mismatch".into());
        }
        let mut manifest: DossierManifestV1 = serde_json::from_slice(&raw)
            .map_err(|error| format!("parse dossier manifest: {error}"))?;
        if manifest.schema != MANIFEST_SCHEMA_V1
            || !manifest.core.anchors.is_empty()
            || manifest.shard_count != shard_count(manifest.anchor_count as usize)
            || validate_sha256(&manifest.state_sha256).is_err()
        {
            return Err("invalid dossier manifest".into());
        }
        manifest.core.anchors = self.load_anchors(&root, &manifest)?;
        if let Some(checkpoint) = manifest.core.checkpoint.as_mut() {
            checkpoint.stale_anchor_ids = manifest
                .core
                .anchors
                .values()
                .filter(|anchor| matches!(anchor.freshness, AnchorFreshnessV1::Stale { .. }))
                .map(|anchor| anchor.anchor_id.clone())
                .collect();
        }
        manifest.core.validate()?;
        if generation_revision(&pointer.generation) != Some(manifest.core.dossier_revision)
            || state_sha(&manifest.core)? != manifest.state_sha256
        {
            return Err("dossier state digest mismatch".into());
        }
        Ok(manifest.core)
    }
    fn load_anchors(
        &self,
        root: &Path,
        manifest: &DossierManifestV1,
    ) -> Result<BTreeMap<String, EvidenceAnchorV1>, DossierStoreError> {
        let mut located = BTreeMap::new();
        let anchor_root = root.join("anchors");
        reject_symlink(&anchor_root)?;
        let mut shard_names = fs::read_dir(&anchor_root)
            .map_err(|error| format!("read anchor root: {error}"))?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("read anchor shard name: {error}"))?;
        shard_names.sort();
        let expected = (0..manifest.shard_count)
            .map(|index| format!("shard-{index:06}").into())
            .collect::<Vec<std::ffi::OsString>>();
        if shard_names != expected {
            return Err("missing or undeclared dossier anchor shard".into());
        }
        for index in 0..manifest.shard_count {
            let shard = anchor_root.join(format!("shard-{index:06}"));
            reject_symlink(&shard)?;
            let mut paths = fs::read_dir(&shard)
                .map_err(|error| format!("read anchor shard: {error}"))?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("read anchor entry: {error}"))?;
            paths.sort();
            let remaining = manifest.anchor_count as usize - index as usize * ANCHORS_PER_SHARD;
            if paths.len() != remaining.min(ANCHORS_PER_SHARD) {
                return Err("invalid dossier anchor shard size".into());
            }
            for path in paths {
                reject_symlink(&path)?;
                let raw = read_bounded(&path, MAX_ANCHOR_BYTES)?;
                let anchor: EvidenceAnchorV1 = serde_json::from_slice(&raw)
                    .map_err(|error| format!("parse dossier anchor: {error}"))?;
                let expected_name = format!("{}.json", anchor.anchor_id);
                if path.file_name().and_then(|name| name.to_str()) != Some(&expected_name)
                    || located
                        .insert(anchor.anchor_id.clone(), (anchor, index))
                        .is_some()
                {
                    return Err("invalid or duplicate dossier anchor".into());
                }
            }
        }
        if located.len() as u64 != manifest.anchor_count
            || !anchor_shards_are_canonical(&located, ANCHORS_PER_SHARD)
        {
            return Err("dossier anchor count mismatch".into());
        }
        Ok(located
            .into_iter()
            .map(|(id, (anchor, _))| (id, anchor))
            .collect())
    }
    fn write_generation(
        &self,
        dossier: &DossierV1,
        generation: &str,
        state_sha256: &str,
    ) -> Result<String, DossierStoreError> {
        let final_path = self.generations.join(generation);
        if final_path.exists() {
            let manifest = read_bounded(&final_path.join("manifest.json"), MAX_MANIFEST_BYTES)?;
            let manifest_sha256 = crate::knowledge::cut::sha256_hex(&manifest);
            let existing = self.load_generation(&CurrentPointerV1 {
                schema: POINTER_SCHEMA_V1.into(),
                generation: generation.into(),
                manifest_sha256: manifest_sha256.clone(),
            })?;
            if existing != *dossier || state_sha(&existing)? != state_sha256 {
                return Err("conflicting dossier generation collision".into());
            }
            return Ok(manifest_sha256);
        }
        let temp = self.generations.join(format!(
            ".{generation}.{}.{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = self.populate_generation(&temp, dossier, state_sha256);
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&temp);
            return Err(error);
        }
        fs::rename(&temp, &final_path)
            .map_err(|error| format!("publish dossier generation: {error}"))?;
        sync_dir(&self.generations)?;
        let manifest = read_bounded(&final_path.join("manifest.json"), MAX_MANIFEST_BYTES)?;
        Ok(crate::knowledge::cut::sha256_hex(&manifest))
    }
    fn populate_generation(
        &self,
        root: &Path,
        dossier: &DossierV1,
        state_sha256: &str,
    ) -> Result<(), DossierStoreError> {
        let anchor_root = root.join("anchors");
        fs::create_dir_all(&anchor_root)
            .map_err(|error| format!("create dossier generation: {error}"))?;
        secure_dir(root)?;
        secure_dir(&anchor_root)?;
        for (index, anchor) in dossier.anchors.values().enumerate() {
            let shard = anchor_root.join(format!("shard-{:06}", index / ANCHORS_PER_SHARD));
            if !shard.exists() {
                fs::create_dir(&shard).map_err(|error| format!("create anchor shard: {error}"))?;
                secure_dir(&shard)?;
            }
            write_new(&shard.join(format!("{}.json", anchor.anchor_id)), anchor)?;
        }
        for index in 0..shard_count(dossier.anchors.len()) {
            sync_dir(&anchor_root.join(format!("shard-{index:06}")))?;
        }
        let mut core = dossier.clone();
        core.anchors.clear();
        if let Some(checkpoint) = core.checkpoint.as_mut() {
            checkpoint.stale_anchor_ids.clear();
        }
        let manifest = DossierManifestV1 {
            schema: MANIFEST_SCHEMA_V1.into(),
            core,
            anchor_count: dossier.anchors.len() as u64,
            shard_count: shard_count(dossier.anchors.len()),
            state_sha256: state_sha256.into(),
        };
        write_new(&root.join("manifest.json"), &manifest)?;
        sync_dir(&anchor_root)?;
        sync_dir(root)
    }
    fn publish_current(&self, pointer: &CurrentPointerV1) -> Result<(), DossierStoreError> {
        let temp = self.root.join(format!(
            ".current.{}.{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        write_new(&temp, pointer)?;
        fs::rename(&temp, &self.current)
            .map_err(|error| format!("publish dossier pointer: {error}"))?;
        secure_file(&self.current)?;
        sync_dir(&self.root)
    }
}
fn write_new<T: Serialize>(path: &Path, value: &T) -> Result<(), DossierStoreError> {
    let body =
        serde_json::to_vec(value).map_err(|error| format!("encode dossier state: {error}"))?;
    let mut file = open_private(path, OpenKind::CreateNew)?;
    file.write_all(&body)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("write dossier state: {error}"))
}

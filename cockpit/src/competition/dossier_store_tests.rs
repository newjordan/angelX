use super::dossier::{AnchorFreshnessV1, DossierV1};
use super::dossier_store::DossierStoreV1;
use super::dossier_tests::{dossier, relevant, stored_one};
use std::path::{Path, PathBuf};

fn current_generation(root: &Path) -> String {
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("current.json")).unwrap()).unwrap();
    value["generation"].as_str().unwrap().to_string()
}

fn anchor_path(root: &Path, generation: &str, anchor: &str) -> PathBuf {
    root.join("generations")
        .join(generation)
        .join("anchors/shard-000000")
        .join(format!("{anchor}.json"))
}

fn assert_load_fails(root: &Path) {
    assert!(DossierStoreV1::new(root.into()).load().is_err());
}

#[test]
fn existing_generation_collision_is_validated_before_publish() {
    let (dir, mut requested, _, _) = stored_one("generation-collision");
    requested
        .admit_relevant(relevant("requested", "revision-one", "requested"))
        .unwrap();
    let state_sha = crate::cut::sha256_hex(&serde_json::to_vec(&requested).unwrap());
    let target = dir
        .path()
        .join("generations")
        .join(format!("{:020}-{state_sha}", requested.dossier_revision));
    let current_path = dir.path().join("current.json");
    let before = std::fs::read(&current_path).unwrap();

    let (other_dir, mut other, _, _) = stored_one("generation-other");
    other
        .admit_relevant(relevant("other", "revision-one", "other"))
        .unwrap();
    DossierStoreV1::new(other_dir.path().into())
        .sync(&other)
        .unwrap();
    let other_generation = current_generation(other_dir.path());
    std::fs::rename(
        other_dir.path().join("generations").join(other_generation),
        &target,
    )
    .unwrap();
    let store = DossierStoreV1::new(dir.path().into());
    assert!(store.sync(&requested).is_err());
    assert_eq!(std::fs::read(&current_path).unwrap(), before);

    std::fs::remove_dir_all(&target).unwrap();
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("manifest.json"), br#"{"schema":"torn""#).unwrap();
    assert!(store.sync(&requested).is_err());
    assert_eq!(std::fs::read(&current_path).unwrap(), before);
}

#[test]
fn exact_generation_anchor_extension_and_shards_are_enforced() {
    let (dir, _, generation, anchor) = stored_one("extension");
    let path = anchor_path(dir.path(), &generation, &anchor);
    std::fs::rename(&path, path.with_extension("json.bak")).unwrap();
    assert_load_fails(dir.path());

    let (dir, _, generation, _) = stored_one("extra-shard");
    std::fs::create_dir(
        dir.path()
            .join("generations")
            .join(generation)
            .join("anchors/shard-000001"),
    )
    .unwrap();
    assert_load_fails(dir.path());

    let (dir, _, generation, _) = stored_one("missing-shard");
    std::fs::remove_dir_all(
        dir.path()
            .join("generations")
            .join(generation)
            .join("anchors/shard-000000"),
    )
    .unwrap();
    assert_load_fails(dir.path());

    let (dir, _, _, _) = stored_one("generation-format");
    let current = dir.path().join("current.json");
    let mut pointer: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&current).unwrap()).unwrap();
    pointer["generation"] = "1-deadbeef".into();
    std::fs::write(current, serde_json::to_vec(&pointer).unwrap()).unwrap();
    assert_load_fails(dir.path());
}

#[cfg(unix)]
#[test]
fn intermediate_anchor_symlink_is_rejected() {
    use std::os::unix::fs::symlink;
    let (dir, _, generation, _) = stored_one("anchor-symlink");
    let root = dir.path().join("generations").join(generation);
    std::fs::rename(root.join("anchors"), root.join("anchors-real")).unwrap();
    symlink("anchors-real", root.join("anchors")).unwrap();
    assert_load_fails(dir.path());
}

#[test]
fn dossier_revision_bounds_and_stale_order_are_enforced() {
    let (dir, mut state, _, _) = stored_one("revision-bounds");
    state.dossier_revision = 0;
    let store = DossierStoreV1::new(dir.path().into());
    assert!(store.sync(&state).is_err());

    let mut state: DossierV1 = dossier();
    state
        .admit_relevant(relevant("source", "revision-one", "one"))
        .unwrap();
    state
        .admit_relevant(relevant("source", "revision-two", "two"))
        .unwrap();
    store.sync(&state).unwrap();
    let current_path = dir.path().join("current.json");
    let before = std::fs::read(&current_path).unwrap();
    for stale_revision in [u64::MAX, 1] {
        let mut invalid = state.clone();
        let stale = invalid
            .anchors
            .values_mut()
            .find(|anchor| matches!(anchor.freshness, AnchorFreshnessV1::Stale { .. }))
            .unwrap();
        if let AnchorFreshnessV1::Stale {
            stale_at_revision, ..
        } = &mut stale.freshness
        {
            *stale_at_revision = stale_revision;
        }
        assert!(store.sync(&invalid).is_err());
        assert_eq!(std::fs::read(&current_path).unwrap(), before);
    }
}

#[test]
fn swapped_anchors_between_valid_shards_are_rejected() {
    let mut state = dossier();
    for index in 0..65 {
        state
            .admit_relevant(relevant(
                &format!("source-{index:03}"),
                "revision-one",
                &format!("content-{index}"),
            ))
            .unwrap();
    }
    let (dir, _, _, _) = stored_one("swapped-shards");
    let store = DossierStoreV1::new(dir.path().into());
    store.sync(&state).unwrap();
    let anchors = dir
        .path()
        .join("generations")
        .join(current_generation(dir.path()))
        .join("anchors");
    let first = |shard: &str| {
        let mut paths = std::fs::read_dir(anchors.join(shard))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        paths.remove(0)
    };
    let left = first("shard-000000");
    let right = first("shard-000001");
    let temporary = anchors.join("swap.tmp");
    let left_name = left.file_name().unwrap().to_owned();
    let right_name = right.file_name().unwrap().to_owned();
    std::fs::rename(&left, &temporary).unwrap();
    std::fs::rename(&right, left.parent().unwrap().join(right_name)).unwrap();
    std::fs::rename(&temporary, right.parent().unwrap().join(left_name)).unwrap();
    assert_load_fails(dir.path());
}

use super::*;
use crate::agent::club::ChatMsg;
use crate::agent::harness::book::{connect, continuity, introduction, ledger};

#[test]
fn conversation_generation_survives_reload_and_restored_history() {
    let _lock = crate::tests::env_lock();
    let root = crate::tests::TestGitWorkspace::new("book-generation-reload");
    let resources = root.path().join("resources");
    let path = resources.join("cockpit/book/gh_navigation.tsv");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let book = path.parent().unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in SOURCES {
        std::fs::copy(embedded.join(name), book.join(name)).unwrap();
    }
    let manifest_path = book.join("catalog-manifest.json");
    std::fs::copy(embedded.join("catalog-manifest.json"), &manifest_path).unwrap();
    let _home = crate::tests::TestEnvGuard::set("HOME", root.path().to_str().unwrap());
    let _store =
        super::super::generation_store::TestRootGuard::enter(root.path().join("generations"));
    let _resources =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", resources.to_str().unwrap());
    let mut history = vec![ChatMsg::user("find code"), ChatMsg::harness("⠣⠁⠁")];
    let first = bind_turn(&mut history).unwrap();
    let digest = continuity::generation_digest(&history).unwrap().to_owned();
    let original;
    let intros;
    {
        let _scope = enter(first);
        original = ledger::read(root.path(), "⠣⠁⠁").unwrap();
        intros = introduction::introductions(&history, None);
        assert!(original.contains("path inside the workspace"));
    }
    let changed = BASELINE.replace(
        "path inside the workspace (relative or absolute)",
        "changed navigation page",
    );
    std::fs::write(&path, changed).unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["chapters"]["gh_navigation.tsv"]["sha256"] =
        crate::knowledge::cut::sha256_hex(&std::fs::read(&path).unwrap()).into();
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&manifest_path, published).unwrap();
    let saved = serde_json::to_vec(&history).unwrap();
    let mut restored: Vec<ChatMsg> = serde_json::from_slice(&saved).unwrap();
    let restored_catalog = bind_turn(&mut restored).unwrap();
    assert_eq!(
        continuity::generation_digest(&restored),
        Some(digest.as_str())
    );
    {
        let _scope = enter(restored_catalog);
        assert_eq!(ledger::read(root.path(), "⠣⠁⠁").unwrap(), original);
        assert_eq!(introduction::introductions(&restored, None), intros);
        assert_eq!(
            connect::recite("⠣⠁⠁"),
            "path inside the workspace (relative or absolute)"
        );
    }
    let mut new_history = vec![ChatMsg::user("new task")];
    let next = bind_turn(&mut new_history).unwrap();
    let _scope = enter(next);
    assert_eq!(connect::recite("⠣⠁⠁"), "changed navigation page");
}

#[test]
fn catalog_rejects_missing_or_extra_addresses_and_noncanonical_snapshots() {
    assert!(Catalog::parse_source("1\tx\n").is_err());
    let baseline = Catalog::baseline();
    assert!(Catalog::from_bytes(&baseline.bytes()).is_ok());
    let mut bad = baseline.bytes();
    bad.push(b' ');
    assert!(Catalog::from_bytes(&bad).is_err());
}

#[test]
fn baseline_captures_every_unlocked_native_page_and_excludes_locked_advisories() {
    let baseline = Catalog::baseline();
    let shape: std::collections::BTreeMap<_, std::collections::BTreeMap<_, _>> = baseline
        .chapters
        .iter()
        .map(|(cell, routes)| {
            (
                cell.clone(),
                routes
                    .iter()
                    .map(|(route, pages)| (route.clone(), pages.len()))
                    .collect(),
            )
        })
        .collect();
    let frozen: std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>> =
        serde_json::from_str(V1_SHAPE).unwrap();
    assert_eq!(
        shape, frozen,
        "change snapshot schema when the TOC shape changes"
    );
    assert!(
        baseline
            .chapters
            .get(&super::super::d3456_advisories::CELL.to_string())
            .is_none()
    );
    for primary in super::super::TOC {
        if primary.cell == super::super::d3456_advisories::CELL {
            continue;
        }
        let chapter = baseline.chapters.get(&primary.cell.to_string()).unwrap();
        for sub in primary.subs {
            let pages = chapter.get(&sub.route.sub.to_string()).unwrap();
            assert_eq!(
                pages.iter().map(String::as_str).collect::<Vec<_>>(),
                sub.pages
            );
        }
    }
}

#[test]
fn restored_catalog_uses_its_saved_non_navigation_pages() {
    let route = super::super::z_brevity::MARKER;
    let mut older = Catalog::baseline();
    older
        .chapters
        .get_mut(&route.primary.to_string())
        .unwrap()
        .get_mut(&route.sub.to_string())
        .unwrap()[0] = "older saved wording".into();
    let restored = Catalog::from_bytes(&older.bytes()).unwrap();
    assert_ne!(&route.sub().pages[0], "older saved wording");
    let _scope = enter(Arc::new(restored));
    assert_eq!(&route.sub().pages[0], "older saved wording");
}

#[test]
fn older_snapshot_keeps_pages_when_current_toc_has_additions() {
    let route = super::super::z_brevity::FULL;
    let mut old = Catalog::baseline();
    let removed_chapter = old
        .chapters
        .keys()
        .find(|key| **key != route.primary.to_string())
        .unwrap()
        .clone();
    let removed_sub = old.chapters[&removed_chapter]
        .keys()
        .next()
        .unwrap()
        .chars()
        .next()
        .unwrap();
    let removed_route =
        super::super::Route::new(removed_chapter.chars().next().unwrap(), removed_sub);
    old.chapters.remove(&removed_chapter);
    let pages = old
        .chapters
        .get_mut(&route.primary.to_string())
        .unwrap()
        .get_mut(&route.sub.to_string())
        .unwrap();
    assert!(pages.len() > 1);
    pages[0] = "retained old page".into();
    pages.pop();
    let saved_pages = pages.clone();
    assert!(Catalog::from_bytes(&old.bytes()).is_err());
    let simulated_old_shape = old
        .chapters
        .iter()
        .map(|(cell, routes)| {
            (
                cell.clone(),
                routes
                    .iter()
                    .map(|(digit, pages)| (digit.clone(), pages.len()))
                    .collect(),
            )
        })
        .collect();
    old.validate_shape(&simulated_old_shape).unwrap();
    assert_eq!(old.pages(route).unwrap(), saved_pages);
    let _scope = enter(Arc::new(old));
    assert_eq!(&route.sub().pages[0], "retained old page");
    assert_eq!(route.sub().pages.len(), 1);
    assert!(super::super::find(removed_route).is_none());
    assert!(!removed_route.name().is_empty());
    let present = super::super::Raise::page(route, 0, None);
    let absent_page = super::super::Raise::page(route, 1, None);
    let absent_route = super::super::Raise::new(removed_route, None);
    assert!(present.available());
    assert!(!absent_page.available());
    assert!(!absent_route.available());
    let expected = present.cells();
    let raises = [present, absent_page, absent_route];
    assert_eq!(
        super::super::warpath(std::path::Path::new("."), &raises),
        expected
    );
    assert_eq!(
        super::super::advice_turn(std::path::Path::new("."), &raises, false),
        expected
    );
    assert_eq!(
        super::super::sign_lines(&[route, removed_route]),
        route.cells()
    );
}

#[test]
fn published_catalog_rejects_unpublished_and_interleaved_changes() {
    let temp = crate::tests::TestGitWorkspace::new("book-manifest-interleaving");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in SOURCES {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let manifest = root.join("catalog-manifest.json");
    std::fs::copy(embedded.join("catalog-manifest.json"), &manifest).unwrap();
    assert_eq!(
        load_published(&root, &manifest).unwrap().bytes(),
        Catalog::baseline().bytes()
    );
    let later = root.join(SOURCES[1].0);
    let original = std::fs::read(&later).unwrap();
    std::fs::write(&later, b"unpublished change").unwrap();
    assert!(load_published(&root, &manifest).is_err());
    std::fs::write(&later, &original).unwrap();
    assert!(
        load_published_with(&root, &manifest, |index| {
            if index == 0 {
                std::fs::write(&later, b"interleaved edit").unwrap();
            }
        })
        .is_err()
    );
    std::fs::write(&later, &original).unwrap();
    let original_manifest = std::fs::read(&manifest).unwrap();
    assert!(
        load_published_with(&root, &manifest, |index| {
            if index == 0 {
                std::fs::write(&manifest, b"changed publication").unwrap();
            }
        })
        .is_err()
    );
    let mut noncanonical = original_manifest.clone();
    noncanonical.push(b' ');
    std::fs::write(&manifest, noncanonical).unwrap();
    assert!(load_published(&root, &manifest).is_err());
}

#[test]
fn legacy_v1_publication_remains_loadable_after_v2_expansion() {
    let temp = crate::tests::TestGitWorkspace::new("book-v1-compatibility");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in &SOURCES[..V1_SOURCE_COUNT] {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(embedded.join("catalog-manifest.json")).unwrap())
            .unwrap();
    manifest["schema"] = MANIFEST_V1_SCHEMA.into();
    manifest["chapters"]
        .as_object_mut()
        .unwrap()
        .retain(|name, _| {
            SOURCES[..V1_SOURCE_COUNT]
                .iter()
                .any(|(old, _)| name.as_str() == *old)
        });
    let path = root.join("catalog-manifest.json");
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&path, &published).unwrap();
    assert_eq!(
        load_published(&root, &path).unwrap().bytes(),
        Catalog::baseline().bytes()
    );

    // A partial v2 publication must not be mistaken for a valid legacy bundle.
    manifest["schema"] = MANIFEST_SCHEMA.into();
    let mut incomplete = serde_json::to_vec(&manifest).unwrap();
    incomplete.push(b'\n');
    std::fs::write(&path, incomplete).unwrap();
    assert!(load_published(&root, &path).is_err());
}

#[test]
fn legacy_v2_publication_remains_loadable_after_v3_expansion() {
    let temp = crate::tests::TestGitWorkspace::new("book-v2-compatibility");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in &SOURCES[..V2_SOURCE_COUNT] {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(embedded.join("catalog-manifest.json")).unwrap())
            .unwrap();
    manifest["schema"] = MANIFEST_V2_SCHEMA.into();
    manifest["chapters"]
        .as_object_mut()
        .unwrap()
        .retain(|name, _| {
            SOURCES[..V2_SOURCE_COUNT]
                .iter()
                .any(|(old, _)| name.as_str() == *old)
        });
    let path = root.join("catalog-manifest.json");
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&path, published).unwrap();
    assert_eq!(
        load_published(&root, &path).unwrap().bytes(),
        Catalog::baseline().bytes()
    );

    manifest["schema"] = MANIFEST_SCHEMA.into();
    let mut incomplete = serde_json::to_vec(&manifest).unwrap();
    incomplete.push(b'\n');
    std::fs::write(&path, incomplete).unwrap();
    assert!(load_published(&root, &path).is_err());
}

#[test]
fn legacy_v3_publication_remains_loadable_after_v4_expansion() {
    let temp = crate::tests::TestGitWorkspace::new("book-v3-compatibility");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in &SOURCES[..V3_SOURCE_COUNT] {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(embedded.join("catalog-manifest.json")).unwrap())
            .unwrap();
    manifest["schema"] = MANIFEST_V3_SCHEMA.into();
    manifest["chapters"]
        .as_object_mut()
        .unwrap()
        .retain(|name, _| {
            SOURCES[..V3_SOURCE_COUNT]
                .iter()
                .any(|(old, _)| name.as_str() == *old)
        });
    let path = root.join("catalog-manifest.json");
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&path, published).unwrap();
    assert_eq!(
        load_published(&root, &path).unwrap().bytes(),
        Catalog::baseline().bytes()
    );

    manifest["schema"] = MANIFEST_SCHEMA.into();
    let mut incomplete = serde_json::to_vec(&manifest).unwrap();
    incomplete.push(b'\n');
    std::fs::write(&path, incomplete).unwrap();
    assert!(load_published(&root, &path).is_err());
}

#[test]
fn legacy_v4_publication_remains_loadable_after_v5_expansion() {
    let temp = crate::tests::TestGitWorkspace::new("book-v4-compatibility");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in &SOURCES[..V4_SOURCE_COUNT] {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(embedded.join("catalog-manifest.json")).unwrap())
            .unwrap();
    manifest["schema"] = MANIFEST_V4_SCHEMA.into();
    manifest["chapters"]
        .as_object_mut()
        .unwrap()
        .retain(|name, _| {
            SOURCES[..V4_SOURCE_COUNT]
                .iter()
                .any(|(old, _)| name.as_str() == *old)
        });
    let path = root.join("catalog-manifest.json");
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&path, published).unwrap();
    assert_eq!(
        load_published(&root, &path).unwrap().bytes(),
        Catalog::baseline().bytes()
    );

    manifest["schema"] = MANIFEST_SCHEMA.into();
    let mut incomplete = serde_json::to_vec(&manifest).unwrap();
    incomplete.push(b'\n');
    std::fs::write(&path, incomplete).unwrap();
    assert!(load_published(&root, &path).is_err());
}

#[test]
fn new_chapter_publication_changes_new_generations_only() {
    let temp = crate::tests::TestGitWorkspace::new("book-v2-publication");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in SOURCES {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let manifest_path = root.join("catalog-manifest.json");
    std::fs::copy(embedded.join("catalog-manifest.json"), &manifest_path).unwrap();
    let original = load_published(&root, &manifest_path).unwrap();
    let old_bytes = original.bytes();
    let path = root.join("m_method.book");
    let source = std::fs::read_to_string(&path).unwrap();
    let changed = source.replacen("This is a rapid-fire solve.", "Edited task pace.", 1);
    assert_ne!(source, changed);
    std::fs::write(&path, changed).unwrap();
    assert!(load_published(&root, &manifest_path).is_err());

    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["chapters"]["m_method.book"]["sha256"] =
        crate::knowledge::cut::sha256_hex(&std::fs::read(path).unwrap()).into();
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&manifest_path, published).unwrap();
    let newer = load_published(&root, &manifest_path).unwrap();
    let old = Catalog::from_bytes(&old_bytes).unwrap();
    let route = super::super::Route::new('⠍', '⠁');
    assert!(newer.pages(route).unwrap()[0].contains("Edited task pace."));
    assert!(old.pages(route).unwrap()[0].contains("This is a rapid-fire solve."));
}

#[test]
fn v3_chapter_publication_keeps_pinned_session_pages() {
    let temp = crate::tests::TestGitWorkspace::new("book-v3-publication");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in SOURCES {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let manifest_path = root.join("catalog-manifest.json");
    std::fs::copy(embedded.join("catalog-manifest.json"), &manifest_path).unwrap();
    let original = load_published(&root, &manifest_path).unwrap();
    let old_bytes = original.bytes();
    let path = root.join("d3_roles.book");
    let source = std::fs::read_to_string(&path).unwrap();
    let changed = source.replacen(
        "Analyze the user's problem from your assigned angle.",
        "A changed proposer instruction.",
        1,
    );
    assert_ne!(source, changed);
    std::fs::write(&path, changed).unwrap();
    assert!(load_published(&root, &manifest_path).is_err());
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["chapters"]["d3_roles.book"]["sha256"] =
        crate::knowledge::cut::sha256_hex(&std::fs::read(path).unwrap()).into();
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&manifest_path, published).unwrap();
    let newer = load_published(&root, &manifest_path).unwrap();
    let old = Catalog::from_bytes(&old_bytes).unwrap();
    let route = super::super::Route::new('⠄', '⠁');
    assert_eq!(
        newer.pages(route).unwrap()[0],
        "A changed proposer instruction."
    );
    assert_eq!(
        old.pages(route).unwrap()[0],
        "Analyze the user's problem from your assigned angle."
    );
}

#[test]
fn v4_seat_profile_publication_keeps_pinned_session_pages() {
    let temp = crate::tests::TestGitWorkspace::new("book-v4-publication");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in SOURCES {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let manifest_path = root.join("catalog-manifest.json");
    std::fs::copy(embedded.join("catalog-manifest.json"), &manifest_path).unwrap();
    let original = load_published(&root, &manifest_path).unwrap();
    let old_bytes = original.bytes();
    let path = root.join("codex_sol_primary.book");
    let source = std::fs::read_to_string(&path).unwrap();
    let changed = source.replacen(
        "You are Codex, an agent based on GPT-6.",
        "Changed seat profile for the test.",
        1,
    );
    assert_ne!(source, changed);
    std::fs::write(&path, changed).unwrap();
    assert!(load_published(&root, &manifest_path).is_err());
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["chapters"]["codex_sol_primary.book"]["sha256"] =
        crate::knowledge::cut::sha256_hex(&std::fs::read(path).unwrap()).into();
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&manifest_path, published).unwrap();
    let newer = load_published(&root, &manifest_path).unwrap();
    let old = Catalog::from_bytes(&old_bytes).unwrap();
    let route = super::super::Route::new('⡞', '⠁');
    assert_eq!(
        newer.pages(route).unwrap()[0],
        "Changed seat profile for the test."
    );
    assert_eq!(
        old.pages(route).unwrap()[0],
        "You are Codex, an agent based on GPT-6."
    );
}

#[test]
fn labyrinth_pages_match_pre_migration_bytes() {
    for (primary, digest) in [
        (
            &super::super::labyrinth_campaign::PRIMARY,
            "98b1090a194264ab92c9eb2087839dbb637d9823091193865ffcf1a631f49ec7",
        ),
        (
            &super::super::labyrinth_campaign::SHELF,
            "91154d3eba95ed1370a6df91f386b3d17085a202067917a9f7ab4ffc98cd397a",
        ),
    ] {
        let mut framed = Vec::new();
        for page in primary.subs.iter().flat_map(|sub| sub.pages.iter()) {
            framed.extend_from_slice(&(page.len() as u64).to_be_bytes());
            framed.extend_from_slice(page.as_bytes());
        }
        assert_eq!(crate::knowledge::cut::sha256_hex(&framed), digest);
    }
}

#[test]
fn legacy_v5_publication_remains_loadable_after_v6_expansion() {
    let temp = crate::tests::TestGitWorkspace::new("book-v5-compatibility");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in &SOURCES[..V5_SOURCE_COUNT] {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(embedded.join("catalog-manifest.json")).unwrap())
            .unwrap();
    manifest["schema"] = MANIFEST_V5_SCHEMA.into();
    manifest["chapters"]
        .as_object_mut()
        .unwrap()
        .retain(|name, _| {
            SOURCES[..V5_SOURCE_COUNT]
                .iter()
                .any(|(old, _)| name == old)
        });
    let path = root.join("catalog-manifest.json");
    let mut bytes = serde_json::to_vec(&manifest).unwrap();
    bytes.push(b'\n');
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        load_published(&root, &path).unwrap().bytes(),
        Catalog::baseline().bytes()
    );
    manifest["schema"] = MANIFEST_SCHEMA.into();
    let mut bytes = serde_json::to_vec(&manifest).unwrap();
    bytes.push(b'\n');
    std::fs::write(&path, bytes).unwrap();
    assert!(load_published(&root, &path).is_err());
}

#[test]
fn labyrinth_publication_preserves_pinned_generations() {
    let temp = crate::tests::TestGitWorkspace::new("book-labyrinth-publication");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in SOURCES {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let manifest_path = root.join("catalog-manifest.json");
    std::fs::copy(embedded.join("catalog-manifest.json"), &manifest_path).unwrap();
    let old = load_published(&root, &manifest_path).unwrap();
    let path = root.join("labyrinth_primary.book");
    let source = std::fs::read_to_string(&path).unwrap();
    let original = "Read the local research map shared with Deli, loop_research and rl_campaign.";
    let changed = source.replacen(original, "Changed research guidance.", 1);
    assert_ne!(source, changed);
    std::fs::write(&path, changed).unwrap();
    assert!(load_published(&root, &manifest_path).is_err());
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["chapters"]["labyrinth_primary.book"]["sha256"] =
        crate::knowledge::cut::sha256_hex(&std::fs::read(path).unwrap()).into();
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&manifest_path, published).unwrap();
    let newer = load_published(&root, &manifest_path).unwrap();
    let route = super::super::labyrinth_campaign::MAP;
    assert_eq!(old.pages(route).unwrap()[0], original);
    assert_eq!(newer.pages(route).unwrap()[0], "Changed research guidance.");
    assert_eq!(
        Catalog::from_bytes(&old.bytes())
            .unwrap()
            .pages(route)
            .unwrap()[0],
        original
    );
}

#[test]
fn v5_shelf_publication_keeps_pinned_session_pages() {
    let temp = crate::tests::TestGitWorkspace::new("book-v5-publication");
    let root = temp.path().join("book");
    std::fs::create_dir_all(&root).unwrap();
    let embedded = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("book");
    for (name, _) in SOURCES {
        std::fs::copy(embedded.join(name), root.join(name)).unwrap();
    }
    let manifest_path = root.join("catalog-manifest.json");
    std::fs::copy(embedded.join("catalog-manifest.json"), &manifest_path).unwrap();
    let old_bytes = load_published(&root, &manifest_path).unwrap().bytes();
    let path = root.join("competition_primary.book");
    let source = std::fs::read_to_string(&path).unwrap();
    let changed = source.replacen(
        "COMPETITION CHALLENGE PACE — RAPID. ALWAYS BE IMPROVING.",
        "Edited competition guidance.",
        1,
    );
    assert_ne!(source, changed);
    std::fs::write(&path, changed).unwrap();
    assert!(load_published(&root, &manifest_path).is_err());
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["chapters"]["competition_primary.book"]["sha256"] =
        crate::knowledge::cut::sha256_hex(&std::fs::read(path).unwrap()).into();
    let mut published = serde_json::to_vec(&manifest).unwrap();
    published.push(b'\n');
    std::fs::write(&manifest_path, published).unwrap();
    let newer = load_published(&root, &manifest_path).unwrap();
    let old = Catalog::from_bytes(&old_bytes).unwrap();
    let route = super::super::Route::new('⠅', '⠁');
    assert_eq!(
        newer.pages(route).unwrap()[0],
        "Edited competition guidance."
    );
    assert_eq!(
        old.pages(route).unwrap()[0],
        "COMPETITION CHALLENGE PACE — RAPID. ALWAYS BE IMPROVING."
    );
}

#[test]
fn missing_bundle_uses_legacy_baseline_but_partial_bundle_fails() {
    let _lock = crate::tests::env_lock();
    let temp = crate::tests::TestGitWorkspace::new("book-partial-bundle");
    let _resources =
        crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", temp.path().to_str().unwrap());
    assert_eq!(
        Catalog::fresh().unwrap().bytes(),
        Catalog::baseline().bytes()
    );
    let root = temp.path().join("cockpit/book");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("gh_navigation.tsv"), BASELINE).unwrap();
    assert!(
        Catalog::fresh()
            .unwrap_err()
            .contains("manifest is missing")
    );
    #[cfg(unix)]
    {
        std::fs::remove_file(root.join("gh_navigation.tsv")).unwrap();
        std::os::unix::fs::symlink("missing-target", root.join("gh_navigation.tsv")).unwrap();
        assert!(Catalog::fresh().is_err());
        std::fs::remove_file(root.join("gh_navigation.tsv")).unwrap();
        std::os::unix::fs::symlink("missing-target", root.join("catalog-manifest.json")).unwrap();
        assert!(Catalog::fresh().is_err());
    }
}

#[test]
fn empty_nested_turn_rejects_an_edited_parent_generation() {
    let edited = Catalog::parse_source(&BASELINE.replace(
        "path inside the workspace (relative or absolute)",
        "edited page",
    ))
    .unwrap();
    let _scope = enter(Arc::new(edited));
    let mut history = vec![ChatMsg::system("system only")];
    assert!(
        bind_turn(&mut history)
            .unwrap_err()
            .contains("without a conversation message")
    );
}

#[test]
fn concurrent_owners_never_cross_generations_and_ambiguous_owner_fails_closed() {
    let first = Arc::new(Catalog::baseline());
    let second = Arc::new(
        Catalog::parse_source(&BASELINE.replace(
            "path inside the workspace (relative or absolute)",
            "another owner's page",
        ))
        .unwrap(),
    );
    let owner_a = 0x501ausize;
    let owner_b = 0x501busize;
    let _a = publish_owner(owner_a, &first);
    let _b = publish_owner(owner_b, &second);
    assert!(Arc::ptr_eq(&for_owner(owner_a).unwrap(), &first));
    assert!(Arc::ptr_eq(&for_owner(owner_b).unwrap(), &second));
    let collision = publish_owner(owner_a, &second);
    assert!(for_owner(owner_a).is_none());
    assert!(Arc::ptr_eq(&for_owner(owner_b).unwrap(), &second));
    drop(collision);
    assert!(Arc::ptr_eq(&for_owner(owner_a).unwrap(), &first));
}

#[test]
#[ignore = "measurement; not a timing gate"]
fn measure_catalog_binding_and_route_lookup() {
    let root = crate::tests::TestGitWorkspace::new("book-generation-measure");
    let _store =
        super::super::generation_store::TestRootGuard::enter(root.path().join("generations"));
    let mut history = vec![ChatMsg::user("benchmark")];
    let start = std::time::Instant::now();
    let catalog = bind_turn(&mut history).unwrap();
    let first = start.elapsed();
    let bytes = catalog.bytes().len();
    let start = std::time::Instant::now();
    for _ in 0..50 {
        std::hint::black_box(bind_turn(&mut history).unwrap());
    }
    let repeat = start.elapsed();
    let _scope = enter(catalog);
    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        std::hint::black_box(super::super::find(super::super::Route::new('⠣', '⠁')).unwrap());
    }
    let lookups = start.elapsed();
    eprintln!(
        "book catalog bytes={bytes} first_us={} repeated_avg_us={} lookup_avg_ns={}",
        first.as_micros(),
        repeat.as_micros() / 50,
        lookups.as_nanos() / 10_000
    );
}
#[test]
fn migrated_tool_chapters_match_compiled_original_catalog() {
    verify_original_catalog(include_str!("../fixtures/book-migration-baseline.json"));
}

#[test]
fn additional_chapters_match_compiled_original_catalog() {
    verify_original_catalog(include_str!("../fixtures/book-migration-batch2.json"));
}

#[test]
fn final_chapters_match_compiled_original_catalog() {
    verify_original_catalog(include_str!("../fixtures/book-migration-batch3.json"));
}

#[test]
fn vendor_shelves_match_compiled_original_catalog() {
    verify_original_catalog(include_str!("../fixtures/book-migration-vendor.json"));
}

#[test]
fn competition_and_connected_shelves_match_compiled_original_catalog() {
    verify_original_catalog(include_str!("../fixtures/book-migration-batch5.json"));
}

fn verify_original_catalog(original: &str) {
    let fixture: serde_json::Value = serde_json::from_str(original).unwrap();
    assert_eq!(fixture["schema"], "angel.book-migration-golden/v1");
    for chapter in fixture["chapters"].as_array().unwrap() {
        let cell = chapter["cell"].as_str().unwrap().chars().next().unwrap();
        let primary = super::super::primary(cell).unwrap();
        assert_eq!(primary.name, chapter["name"].as_str().unwrap());
        assert_eq!(primary.surface, chapter["surface"].as_str().unwrap());
        let sections = chapter["sections"].as_array().unwrap();
        assert_eq!(
            primary.subs.len(),
            chapter["section_count"].as_u64().unwrap() as usize
        );
        assert_eq!(primary.subs.len(), sections.len());
        for (sub, expected) in primary.subs.iter().zip(sections) {
            assert_eq!(
                sub.route.primary.to_string(),
                expected["primary"].as_str().unwrap()
            );
            assert_eq!(sub.route.sub.to_string(), expected["sub"].as_str().unwrap());
            assert_eq!(sub.name, expected["name"].as_str().unwrap());
            assert_eq!(sub.signal, expected["signal"].as_str().unwrap());
            assert_eq!(sub.action, expected["action"].as_str().unwrap());
            assert_eq!(sub.ideas, expected["ideas"].as_str().unwrap());
            let pages = expected["pages"].as_array().unwrap();
            assert_eq!(
                sub.pages.len(),
                expected["page_count"].as_u64().unwrap() as usize
            );
            assert_eq!(sub.pages.len(), pages.len());
            for (actual, expected) in sub.pages.iter().zip(pages) {
                assert_eq!(*actual, expected.as_str().unwrap());
            }
        }
    }
}

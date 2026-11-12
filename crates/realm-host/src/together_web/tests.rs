//! The browser page's music comes from disk, not from the binary.

use super::*;

/// Places that find nothing and say nothing.
fn deaf() -> Places {
    Places {
        resource: |_| None,
        missing: |_| {},
    }
}

#[test]
fn a_folder_beside_the_binary_joins_the_page_by_name() {
    let dir = std::env::temp_dir().join(format!("angel-web-beside-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("inner")).unwrap();
    std::fs::write(dir.join("crypt.mp3"), b"ID3crypt").unwrap();
    std::fs::write(dir.join("a-boss.mp3"), b"ID3boss").unwrap();
    // A name the page could not carry is left out, and so is a folder.
    std::fs::write(dir.join("two words.mp3"), b"ID3").unwrap();
    let mut files = Vec::new();
    let sounds = add_folder(&dir, "music-", &mut files, &deaf());
    assert_eq!(
        sounds,
        [
            "\"music-a-boss\":\"{{asset:music-a-boss.mp3}}\"",
            "\"music-crypt\":\"{{asset:music-crypt.mp3}}\""
        ]
    );
    assert_eq!(files.len(), 2);
    assert_eq!(files[1].0, "music-crypt.mp3");
    assert_eq!(files[1].1, b"ID3crypt");
    assert_eq!(files[1].2, "audio/mpeg");
    // A folder that is not there adds nothing.
    assert!(add_folder(&dir.join("gone"), "music-", &mut files, &deaf()).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_page_takes_its_beside_folders_from_the_places_it_is_lent() {
    use std::sync::Mutex;
    static SAID: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let Some((_, embedded)) = BUNDLE else {
        return;
    };
    assert!(
        embedded.iter().all(|(name, _)| !name.starts_with("music-")),
        "no music is compiled in"
    );
    assert_eq!(BESIDE, &[("music-", "cockpit/assets/dungeon/music")]);
    // Nowhere to look: the page opens without music and a line says so.
    SAID.lock().unwrap().clear();
    let bare = bundle(&Places {
        resource: |_| None,
        missing: |line| SAID.lock().unwrap().push(line.to_string()),
    })
    .expect("the page");
    assert!(
        bare.files
            .iter()
            .all(|(name, _, _)| !name.starts_with("music-"))
    );
    assert_eq!(SAID.lock().unwrap().len(), 1);
    assert!(SAID.lock().unwrap()[0].contains("cockpit/assets/dungeon/music"));
    // This checkout has the music beside the host: every track in the folder
    // joins the page's sounds and is served as the file read off disk.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = repo.join("cockpit/assets/dungeon/music");
    assert!(dir.is_dir(), "the checkout carries its music");
    let page = bundle(&Places {
        resource: |place| {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(place);
            dir.is_dir().then_some(dir)
        },
        missing: |line| panic!("{line}"),
    })
    .expect("the page");
    let json = page
        .index_html
        .split("id=\"sounds\">")
        .nth(1)
        .and_then(|rest| rest.split("</script>").next())
        .expect("the page carries its sounds");
    let manifest: serde_json::Value = serde_json::from_str(json).expect("the sounds are JSON");
    let mut served = 0;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        let file = format!("music-{}", path.file_name().unwrap().to_string_lossy());
        if !path.is_file()
            || !file
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            continue;
        }
        let stem = file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s);
        assert_eq!(
            manifest[stem].as_str(),
            Some(format!("{{{{asset:{file}}}}}").as_str()),
            "{file}"
        );
        let served_file = page
            .files
            .iter()
            .find(|(f, _, _)| *f == file)
            .unwrap_or_else(|| panic!("{file} is served"));
        assert_eq!(
            served_file.1,
            std::fs::read(&path).unwrap(),
            "{file} is the file on disk"
        );
        served += 1;
    }
    assert!(served > 0, "the music folder has tracks");
    // The effects stay compiled in.
    assert!(manifest["hit"].is_string());
}

#[test]
fn worker_capture_pins_bytes_and_bounds_disk_sources_before_publication() {
    use angel_serve::http::preparation::{MAX_FILE_BYTES, MAX_FILES};
    use std::sync::atomic::AtomicBool;
    if BUNDLE.is_none() { return; }
    let root = std::env::temp_dir().join(format!("angel-browser-capture-{}", std::process::id()));
    let dir = root.join("cockpit/assets/dungeon/music");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("generation.mp3"), b"old immutable music").unwrap();
    let cancel = AtomicBool::new(false);
    let started = std::time::Instant::now();
    let bundle = capture(std::slice::from_ref(&root), &cancel).unwrap();
    let capture_time = started.elapsed();
    std::fs::write(dir.join("generation.mp3"), b"replacement music").unwrap();
    assert_eq!(bundle.files.iter().find(|(name, _, _)| name == "music-generation.mp3").unwrap().1, b"old immutable music");
    let started = std::time::Instant::now();
    let assets = angel_serve::http::Assets::prepare(&bundle).unwrap();
    let prepare_time = started.elapsed();
    assert!(assets.files.values().any(|body| body.plain == b"old immutable music"));
    assert!(!assets.index.plain.windows(8).any(|s| s == b"{{asset:"));
    let file = std::fs::File::create(dir.join("oversized.mp3")).unwrap();
    file.set_len(MAX_FILE_BYTES as u64 + 1).unwrap();
    assert!(capture(std::slice::from_ref(&root), &cancel).is_err());
    std::fs::remove_file(dir.join("oversized.mp3")).unwrap();
    for n in 0..=MAX_FILES { std::fs::create_dir(dir.join(format!("ignored-{n}"))).unwrap(); }
    assert!(capture(std::slice::from_ref(&root), &cancel).is_err());
    std::fs::remove_dir_all(&root).unwrap();
    cancel.store(true, std::sync::atomic::Ordering::Release);
    assert!(capture(&[], &cancel).is_err());
    eprintln!("immutable browser capture {capture_time:?}; embedded + one music preparation {prepare_time:?}; {} bytes, {} files", bundle.files.iter().map(|(_,b,_)| b.len()).sum::<usize>(), bundle.files.len());
}

#[test]
fn capture_preserves_symlink_policy_and_refuses_mid_capture_mutation() {
    if BUNDLE.is_none() { return; }
    let root = std::env::temp_dir().join(format!("angel-browser-parity-{}", std::process::id()));
    let dir = root.join("cockpit/assets/dungeon/music");
    std::fs::create_dir_all(&dir).unwrap();
    let original = dir.join("original.mp3");
    std::fs::write(&original, b"original bytes").unwrap();
    #[cfg(unix)] std::os::unix::fs::symlink(&original, dir.join("linked.mp3")).unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let bundle = capture(std::slice::from_ref(&root), &cancel).unwrap();
    #[cfg(unix)] assert_eq!(bundle.files.iter().find(|(n,_,_)| n == "music-linked.mp3").unwrap().1, b"original bytes");
    let changed = capture_with_hook(std::slice::from_ref(&root), &cancel, |_| {
        std::fs::write(&original, b"replacement bytes with another length").unwrap();
    });
    assert!(changed.is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn capture_preserves_skip_and_log_for_unreadable_files_and_folders() {
    use std::os::unix::fs::PermissionsExt;
    if BUNDLE.is_none() { return; }
    let root = std::env::temp_dir().join(format!("angel-browser-unreadable-{}", std::process::id()));
    let dir = root.join("cockpit/assets/dungeon/music");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("unreadable.mp3");
    std::fs::write(&path, b"unreadable bytes").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0)).unwrap();
    if std::fs::File::open(&path).is_err() {
        let captured = capture(std::slice::from_ref(&root), &std::sync::atomic::AtomicBool::new(false)).unwrap();
        assert!(!captured.files.iter().any(|(n,_,_)| n == "music-unreadable.mp3"));
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0)).unwrap();
    if std::fs::read_dir(&dir).is_err() {
        let captured = capture(std::slice::from_ref(&root), &std::sync::atomic::AtomicBool::new(false)).unwrap();
        assert!(!captured.files.iter().any(|(n,_,_)| n == "music-unreadable.mp3"));
    }
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn complete_capture_matches_legacy_manifest_and_resource_root_precedence() {
    let _guard = crate::tests::env_lock();
    if BUNDLE.is_none() { return; }
    crate::lend_source_tree();
    let root = std::env::temp_dir().join(format!("angel-browser-root-parity-{}", std::process::id()));
    let dir = root.join("cockpit/assets/dungeon/music");
    std::fs::create_dir_all(&dir).unwrap();
    let lower = crate::together_audio::resource("cockpit/assets/dungeon/music");
    let name = lower.as_ref().and_then(|dir| std::fs::read_dir(dir).ok()).and_then(|entries| entries.flatten().map(|e| e.path()).find(|p| p.is_file())).and_then(|p| p.file_name().map(|n| n.to_owned())).unwrap_or_else(|| "priority.mp3".into());
    std::fs::write(dir.join(&name), b"environment root wins").unwrap();
    std::fs::write(dir.join("environment-only.mp3"), b"only environment").unwrap();
    let _resource = crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", root.to_str().unwrap());
    let roots: Vec<_> = std::env::var_os("ANGEL_RESOURCE_DIR").map(PathBuf::from).into_iter()
        .chain(angel_live::host::runtime_paths::source_cockpit_dir().and_then(Path::parent).map(Path::to_path_buf))
        .chain(Some(crate::root())).collect();
    let captured = capture(&roots, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    let legacy = bundle(&Places { resource: crate::together_audio::resource, missing: |_| {} }).unwrap();
    assert_eq!(captured.index_html, legacy.index_html);
    assert_eq!(captured.files, legacy.files);
    let music: Vec<_> = captured.files.iter().filter(|(n,_,_)| n.starts_with("music-")).collect();
    assert_eq!(music.len(), 2, "lower-priority folder must not merge into selected folder");
    let expected = format!("music-{}", name.to_string_lossy());
    assert_eq!(music.iter().find(|(n,_,_)| n == &expected).unwrap().1, b"environment root wins");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn full_shipped_capture_and_preparation_cost_against_legacy() {
    let _guard = crate::tests::env_lock();
    if BUNDLE.is_none() { return; }
    crate::lend_source_tree();
    let roots: Vec<_> = std::env::var_os("ANGEL_RESOURCE_DIR").map(PathBuf::from).into_iter()
        .chain(angel_live::host::runtime_paths::source_cockpit_dir().and_then(Path::parent).map(Path::to_path_buf))
        .chain(Some(crate::root())).collect();
    let before = std::time::Instant::now();
    let captured = capture(&roots, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    let capture_time = before.elapsed();
    let bytes = captured.files.iter().map(|(_,b,_)| b.len()).sum::<usize>();
    let before = std::time::Instant::now();
    let assets = angel_serve::http::Assets::prepare_static(&captured, PREPARED_FILES).unwrap();
    let preparation = before.elapsed();
    assert_eq!(PREPARED_FILES.len(), BUNDLE.unwrap().1.len());
    let before = std::time::Instant::now();
    let legacy = shipped().unwrap();
    let legacy_capture = before.elapsed();
    assert_eq!(captured.index_html, legacy.index_html);
    assert_eq!(captured.files, legacy.files);
    drop(captured);
    let before = std::time::Instant::now();
    let baseline = angel_serve::http::Assets::prepare(&legacy).unwrap();
    let legacy_preparation = before.elapsed();
    assert_eq!(assets.index.plain, baseline.index.plain);
    assert_eq!(assets.index.gzip, baseline.index.gzip);
    assert_eq!(assets.index.etag, baseline.index.etag);
    assert_eq!(assets.files.len(), baseline.files.len());
    for (name, body) in &assets.files {
        let old=&baseline.files[name];
        assert_eq!(body.mime, old.mime); assert_eq!(body.plain, old.plain);
        assert_eq!(body.gzip, old.gzip); assert_eq!(body.etag, old.etag);
    }
    eprintln!("full shipped {bytes} bytes, {} files: bounded capture {capture_time:?}, prebuilt reuse + dynamic hash/gzip {preparation:?}; legacy capture {legacy_capture:?}, hash/gzip {legacy_preparation:?}; prepared {} files", legacy.files.len(), assets.files.len());
}

#[test]
fn prebuilt_embedded_files_do_not_cache_edited_or_missing_beside_music() {
    if BUNDLE.is_none() { return; }
    let root=std::env::temp_dir().join(format!("browser-dynamic-prebuilt-{}",std::process::id()));
    let dir=root.join("cockpit/assets/dungeon/music"); std::fs::create_dir_all(&dir).unwrap();
    let file=dir.join("editable.mp3");std::fs::write(&file,b"old dynamic music").unwrap();
    let cancel=std::sync::atomic::AtomicBool::new(false);
    let bundle=capture(std::slice::from_ref(&root),&cancel).unwrap();
    let old=angel_serve::http::Assets::prepare_static(&bundle,PREPARED_FILES).unwrap();
    let original=old.files.iter().find(|(_,body)| body.plain==b"old dynamic music").unwrap().0.clone();
    std::fs::write(&file,b"new dynamic music").unwrap();
    let bundle=capture(std::slice::from_ref(&root),&cancel).unwrap();
    let edited=angel_serve::http::Assets::prepare_static(&bundle,PREPARED_FILES).unwrap();
    let replacement=edited.files.iter().find(|(_,body)| body.plain==b"new dynamic music").unwrap().0;
    assert_ne!(&original,replacement);assert_eq!(old.files[&original].plain,b"old dynamic music");
    std::fs::remove_dir_all(&dir).unwrap();
    let missing=capture(std::slice::from_ref(&root),&cancel).unwrap();
    assert!(!missing.files.iter().any(|(name,_,_)|name=="music-editable.mp3"));
    let missing=angel_serve::http::Assets::prepare_static(&missing,PREPARED_FILES).unwrap();
    assert!(!missing.files.contains_key(&original));assert!(!missing.files.contains_key(replacement));
    std::fs::remove_dir_all(root).unwrap();
}

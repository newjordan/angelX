use super::check::{self, Level};
use super::cli::{self, Surface};
use super::digest::{self, Kind};
use super::install::{self, Origin, Output};
use super::registry::{self, Change, Entry, Registry};
use super::*;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A scratch folder for one test, removed afterwards.
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "angel-plugins-{name}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir.canonicalize().unwrap())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Tester",
            "-c",
            "user.email=tester@example.com",
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn paths(root: &Path) -> Paths {
    Paths {
        cartridges: root.join("cartridges"),
        state: root.join("state"),
        registry: root.join("registry.toml").display().to_string(),
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[cfg(unix)]
fn chmod_x(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A git repository at `root/repo` holding the new plugin `id` at
/// `plugins/<id>` and a file beside it, committed; with a setup when asked.
fn published(root: &Path, id: &str, setup: bool) -> (PathBuf, PathBuf, String) {
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    write(&repo.join("README.md"), "not the plugin\n");
    let folder = repo.join("plugins").join(id);
    scaffold::create(id, Some(&folder), "Tester").unwrap();
    if setup {
        let toml = folder.join("cartridge.toml");
        let text = std::fs::read_to_string(&toml).unwrap().replace(
            "license = \"MIT\"\n",
            "license = \"MIT\"\nsetup = \"setup.sh\"\n",
        );
        write(&toml, &text);
        write(
            &folder.join("setup.sh"),
            "#!/bin/sh\nset -e\n[ \"$1\" = \"$ANGEL_CARTRIDGE_DIR\" ]\nmkdir -p runtime\nif [ -f runtime/made ]; then echo kept; else echo made > runtime/made; echo set up; fi\n",
        );
        chmod_x(&folder.join("setup.sh"));
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "first"]);
    let commit = git(&repo, &["rev-parse", "HEAD"]);
    (repo, folder, commit)
}

/// The registry entry for the plugin at `folder`, committed at `commit` in `repo`.
fn entry_for(repo: &Path, folder: &Path, commit: &str) -> Entry {
    let config = check::folder(folder, Level::Publish).config.unwrap();
    let plugin = config.plugin.unwrap();
    Entry {
        id: config.id,
        label: config.label,
        version: plugin.version,
        description: plugin.description,
        repository: format!("file://{}", repo.display()),
        commit: commit.to_owned(),
        path: folder.strip_prefix(repo).unwrap().display().to_string(),
        sha256: digest::digest_folder(folder).unwrap(),
        license: plugin.license,
        authors: plugin.authors,
        ..Entry::default()
    }
}

fn registry_file(paths: &Paths, entries: &[&Entry]) {
    let mut text = String::from("# test registry\n\n");
    for entry in entries {
        text = registry::upsert(&text, entry).unwrap().0;
    }
    write(Path::new(&paths.registry), &text);
}

fn collect() -> (std::sync::Arc<Mutex<Vec<u8>>>, impl Fn(&[u8]) + Send + Sync) {
    let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
    let sink = {
        let seen = std::sync::Arc::clone(&seen);
        move |bytes: &[u8]| seen.lock().unwrap().extend_from_slice(bytes)
    };
    (seen, sink)
}

#[test]
fn versions_order_by_number_and_a_pre_release_comes_first() {
    let v = |text| Version::parse(text).unwrap();
    assert!(v("0.2.7") < v("0.2.10"));
    assert!(v("1.0.0-rc.1") < v("1.0.0"));
    assert!(v("1.0.0") < v("1.0.1-beta"));
    assert_eq!(v("2.3.4"), v("2.3.4"));
    for bad in [
        "1.2",
        "1.2.3.4",
        "01.2.3",
        "1.2.x",
        "1.2.3-",
        "",
        "v1.2.3",
        "1.2.3-a b",
    ] {
        assert!(Version::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn ids_are_short_lowercase_words() {
    for good in ["vfx", "lean-kernel", "a1", "dig"] {
        assert!(valid_id(good), "{good}");
    }
    for bad in [
        "x",
        "Vfx",
        "1vfx",
        "vfx-",
        "v_fx",
        "../x",
        "a".repeat(33).as_str(),
        "",
    ] {
        assert!(!valid_id(bad), "{bad}");
    }
}

#[test]
fn the_tree_digest_matches_its_spec_vector() {
    // The vector scripts/plugins/check-registry.py and a plain sha256sum of
    // the spec give (docs/PLUGINS.md, "The digest").
    let entries = vec![
        digest::Entry {
            path: "link".into(),
            kind: Kind::Link,
            bytes: b"a.txt".to_vec(),
        },
        digest::Entry {
            path: "bin/run".into(),
            kind: Kind::Exec,
            bytes: b"#!/bin/sh\necho hi\n".to_vec(),
        },
        digest::Entry {
            path: "a.txt".into(),
            kind: Kind::File,
            bytes: b"hello\n".to_vec(),
        },
    ];
    assert_eq!(
        digest::digest(&entries),
        "d2f3d9d9350d8e1555cc17cf052c71c89c0216c7b813d2407fc69b1a063754f5"
    );
    assert_eq!(
        digest::digest(&[]),
        "887dd9868d358ebe96f1be0d915f3540bf64d61384c4dc84e7ba4adb0b50d006"
    );
}

#[test]
fn a_folder_and_its_commit_digest_the_same_and_the_fetch_takes_only_that_folder() {
    let temp = Temp::new("digest");
    let (repo, folder, commit) = published(temp.path(), "hello", true);
    std::os::unix::fs::symlink("README.md", folder.join("readme-link")).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "link"]);
    let commit2 = git(&repo, &["rev-parse", "HEAD"]);
    assert_ne!(commit, commit2);
    let files = git::fetch(
        &format!("file://{}", repo.display()),
        &commit2,
        "plugins/hello",
        &temp.path().join("scratch.git"),
        None,
    )
    .unwrap();
    let mut names: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "LICENSE",
            "README.md",
            "bin/hello",
            "cartridge.toml",
            "readme-link",
            "setup.sh",
            "skills/hello/SKILL.md"
        ]
    );
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == "bin/hello")
            .unwrap()
            .kind,
        Kind::Exec
    );
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == "readme-link")
            .unwrap()
            .kind,
        Kind::Link
    );
    assert_eq!(
        digest::digest(&files),
        digest::digest_folder(&folder).unwrap()
    );
    // Written back out, the files digest the same again.
    let out = temp.path().join("out");
    digest::write_folder(&out, &files).unwrap();
    assert_eq!(digest::digest_folder(&out).unwrap(), digest::digest(&files));
}

#[test]
fn a_fetch_refuses_what_the_repository_does_not_hold() {
    let temp = Temp::new("fetch-refuse");
    let (repo, _, commit) = published(temp.path(), "hello", false);
    let url = format!("file://{}", repo.display());
    let missing = "0123456789abcdef0123456789abcdef01234567";
    assert!(git::fetch(&url, missing, "", &temp.path().join("a.git"), None).is_err());
    let error = git::fetch(
        &url,
        &commit,
        "plugins/nope",
        &temp.path().join("b.git"),
        None,
    )
    .unwrap_err();
    assert!(error.contains("no folder"), "{error}");
    assert!(
        git::fetch(&url, "HEAD", "", &temp.path().join("c.git"), None)
            .unwrap_err()
            .contains("full commit id")
    );
    assert!(
        git::fetch(
            "--upload-pack=touch /tmp/x",
            &commit,
            "",
            &temp.path().join("d.git"),
            None
        )
        .is_err()
    );
    assert!(
        git::fetch(
            "ext::sh -c touch% /tmp/x",
            &commit,
            "",
            &temp.path().join("e.git"),
            None
        )
        .is_err()
    );
}

fn sample() -> Entry {
    Entry {
        id: "vfx".into(),
        label: "Bend Pixel VFX".into(),
        version: "1.0.0".into(),
        description: "Pixel-art VFX sheets rendered locally with Bend.".into(),
        repository: "https://github.com/newjordan/angelX".into(),
        commit: "5874990e43a2b1c3d4e5f60718293a4b5c6d7e8f".into(),
        path: "docs/examples/cartridges/vfx".into(),
        sha256: "a".repeat(64),
        license: "MIT".into(),
        authors: vec!["Jordan Newman".into()],
        ..Entry::default()
    }
}

#[test]
fn registry_entries_are_held_to_the_public_rules() {
    assert!(
        sample().problems(true).is_empty(),
        "{:?}",
        sample().problems(true)
    );
    let with = |change: fn(&mut Entry)| {
        let mut entry = sample();
        change(&mut entry);
        entry.problems(true)
    };
    assert!(!with(|e| e.repository = "http://example.com/x".into()).is_empty());
    assert!(!with(|e| e.repository = "https://user:token@github.com/x/y".into()).is_empty());
    assert!(!with(|e| e.commit = "5874990e4".into()).is_empty());
    assert!(!with(|e| e.path = "../outside".into()).is_empty());
    assert!(!with(|e| e.sha256 = "A".repeat(64)).is_empty());
    assert!(!with(|e| e.version = "1.0".into()).is_empty());
    assert!(!with(|e| e.description = "two\nlines".into()).is_empty());
    assert!(!with(|e| e.authors.clear()).is_empty());
    assert!(!with(|e| e.license.clear()).is_empty());
    // A private registry may name a file repository.
    let mut private = sample();
    private.repository = "file:///srv/plugins/repo".into();
    assert!(!private.problems(true).is_empty() && private.problems(false).is_empty());
    // Two ids may not claim one folder.
    let text = registry::upsert(
        &registry::upsert("", &sample()).unwrap().0,
        &Entry {
            id: "vfx-two".into(),
            ..sample()
        },
    )
    .unwrap()
    .0;
    let parsed = Registry::parse(&text, "test").unwrap();
    assert!(
        parsed
            .problems(true)
            .iter()
            .any(|problem| problem.contains("same folder"))
    );
    // Unknown fields are refused.
    assert!(Registry::parse("[x]\nlabel = \"X\"\nmystery = 1\n", "test").is_err());
}

#[test]
fn upsert_adds_in_id_order_and_keeps_every_comment() {
    let header = "# The angelX plugin registry.\n# One table per plugin.\n\n";
    let alpha = Entry {
        id: "alpha".into(),
        path: "a".into(),
        ..sample()
    };
    let gamma = Entry {
        id: "gamma".into(),
        path: "g".into(),
        ..sample()
    };
    let beta = Entry {
        id: "beta".into(),
        path: "b".into(),
        ..sample()
    };
    let (text, change) = registry::upsert(header, &gamma).unwrap();
    assert_eq!(change, Change::Added);
    let text = text.replace("[gamma]", "# gamma's own note\n[gamma]");
    let (text, _) = registry::upsert(&text, &alpha).unwrap();
    let (text, change) = registry::upsert(&text, &beta).unwrap();
    assert_eq!(change, Change::Added);
    assert!(text.starts_with(header), "{text}");
    let order: Vec<usize> = ["[alpha]", "[beta]", "# gamma's own note", "[gamma]"]
        .iter()
        .map(|key| text.find(key).unwrap())
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
    let parsed = Registry::parse(&text, "test").unwrap();
    assert_eq!(
        parsed
            .entries
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "beta", "gamma"]
    );
    assert_eq!(parsed.get("beta").unwrap(), &beta);
}

#[test]
fn upsert_updates_its_own_entry_and_refuses_anothers_id() {
    let text = registry::upsert("# head\n\n", &sample()).unwrap().0;
    let other = Entry {
        id: "zeta".into(),
        path: "z".into(),
        ..sample()
    };
    let text = registry::upsert(&text, &other).unwrap().0;
    assert_eq!(
        registry::upsert(&text, &sample()).unwrap().1,
        Change::Unchanged
    );
    let newer = Entry {
        version: "1.1.0".into(),
        commit: "f".repeat(40),
        ..sample()
    };
    let (updated, change) = registry::upsert(&text, &newer).unwrap();
    assert_eq!(change, Change::Updated("1.0.0".into()));
    let parsed = Registry::parse(&updated, "test").unwrap();
    assert_eq!(parsed.get("vfx").unwrap(), &newer);
    assert_eq!(parsed.get("zeta").unwrap(), &other);
    assert!(updated.starts_with("# head\n\n"));
    let squatter = Entry {
        repository: "https://github.com/someone/else".into(),
        ..sample()
    };
    assert!(
        registry::upsert(&text, &squatter)
            .unwrap_err()
            .contains("taken")
    );
}

#[test]
fn a_new_plugin_works_and_passes_its_own_check() {
    let temp = Temp::new("scaffold");
    let folder = scaffold::create(
        "hello-world",
        Some(&temp.path().join("hello-world")),
        "Tester",
    )
    .unwrap();
    let report = check::folder(&folder, Level::Publish);
    assert!(report.ok(), "{:?}", report.lines());
    assert!(report.warnings.is_empty(), "{:?}", report.lines());
    let config = report.config.unwrap();
    assert_eq!(config.commands[0].name, "hello-world");
    assert_eq!(config.tools[0].name, "hello_world_hello");
    // Its tool runs as a cartridge tool does.
    let output = std::process::Command::new(folder.join("bin/hello"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(br#"{"name": "Ada"}"#)?;
            child.wait_with_output()
        })
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "Hello, Ada, from the hello-world plugin."
    );
    // A folder in use is never written over; an angelX command is no id.
    assert!(
        scaffold::create("hello-world", Some(&folder), "Tester")
            .unwrap_err()
            .contains("not empty")
    );
    assert!(scaffold::create("help", Some(&temp.path().join("help")), "Tester").is_err());
    assert!(scaffold::create("Bad_Id", Some(&temp.path().join("bad")), "Tester").is_err());
}

#[test]
fn the_check_names_what_would_break_a_plugin() {
    let temp = Temp::new("check");
    let fresh =
        |name: &str| scaffold::create(name, Some(&temp.path().join(name)), "Tester").unwrap();
    let says = |folder: &Path, level: Level, words: &str| {
        let report = check::folder(folder, level);
        assert!(
            !report.ok() && report.errors.iter().any(|error| error.contains(words)),
            "{words}: {:?}",
            report.lines()
        );
    };
    let skill = fresh("no-skill");
    std::fs::remove_dir_all(skill.join("skills")).unwrap();
    says(&skill, Level::Load, "skill `no-skill` is not in skills/");
    let mode = fresh("no-exec");
    std::fs::set_permissions(
        mode.join("bin/hello"),
        std::os::unix::fs::PermissionsExt::from_mode(0o644),
    )
    .unwrap();
    says(&mode, Level::Load, "not executable");
    let source = fresh("source");
    write(&source.join("src/mod.rs"), "");
    says(&source, Level::Load, "config-only");
    let builtin = fresh("builtin");
    let toml = builtin.join("cartridge.toml");
    write(
        &toml,
        &std::fs::read_to_string(&toml)
            .unwrap()
            .replace("name = \"builtin\"", "name = \"model\""),
    );
    says(
        &builtin,
        Level::Load,
        "/model is one of angelX's own commands",
    );
    let outside = fresh("outside");
    std::os::unix::fs::symlink("../../../etc/passwd", outside.join("bin/escape")).unwrap();
    says(&outside, Level::Load, "outside the plugin");
    let bare = fresh("bare");
    let toml = bare.join("cartridge.toml");
    let text = std::fs::read_to_string(&toml).unwrap();
    let start = text.find("[plugin]").unwrap();
    let end = text.find("# `/bare").unwrap();
    write(&toml, &format!("{}{}", &text[..start], &text[end..]));
    assert!(
        check::folder(&bare, Level::Load).ok(),
        "a cartridge without [plugin] still loads"
    );
    says(&bare, Level::Publish, "no [plugin] table");
    let misplaced = temp.path().join("misplaced");
    write(
        &misplaced.join("cartridge.toml"),
        "id = \"misplaced\"\nlabel = \"M\"\n[plugin]\nversion = \"1.0.0\"\ndescription = \"d\"\nworker = \"w\"\n",
    );
    says(
        &misplaced,
        Level::Load,
        "put [plugin] after the top-level keys",
    );
    let empty = temp.path().join("empty");
    write(
        &empty.join("cartridge.toml"),
        "id = \"empty\"\nlabel = \"Empty\"\n",
    );
    says(&empty, Level::Load, "carries nothing");
    assert!(check::link_inside("bin/x", "../README.md"));
    assert!(!check::link_inside("bin/x", "../../x"));
    assert!(!check::link_inside("x", "/etc/passwd"));
}

#[test]
fn install_verifies_the_digest_then_writes_the_plugin_beside_its_receipt() {
    let temp = Temp::new("install");
    let (repo, folder, commit) = published(temp.path(), "hello", true);
    let paths = paths(temp.path());
    let entry = entry_for(&repo, &folder, &commit);
    let (seen, sink) = collect();
    let outcome = install::install(
        &paths,
        &entry,
        "test registry",
        false,
        false,
        Output::Sink(&sink),
        None,
    )
    .unwrap();
    assert_eq!(outcome.folder, paths.cartridges.join("hello"));
    assert_eq!(outcome.setup.as_ref().map(Result::is_ok), Some(true));
    assert!(String::from_utf8_lossy(&seen.lock().unwrap()).contains("set up"));
    let dest = &outcome.folder;
    assert!(
        dest.join("runtime/made").is_file(),
        "the setup ran in the plugin's folder"
    );
    assert!(
        !dest.join("README.md").exists()
            || std::fs::read_to_string(dest.join("README.md")).unwrap() != "not the plugin\n"
    );
    let found = install::find(&paths, "hello").unwrap();
    let Origin::Registry(receipt) = &found.origin else {
        panic!("{:?}", found.origin)
    };
    assert_eq!(
        (receipt.commit.as_str(), receipt.setup_ok),
        (commit.as_str(), Some(true))
    );
    assert!(receipt.files.contains(&"cartridge.toml".to_owned()));
    assert_eq!(found.config.unwrap().id, "hello");
    // The files it wrote are the reviewed tree, byte for byte.
    let written: Vec<digest::Entry> = digest::read_folder(dest)
        .unwrap()
        .into_iter()
        .filter(|file| !file.path.starts_with("runtime/"))
        .collect();
    assert_eq!(digest::digest(&written), entry.sha256);
    // No staging is left behind.
    let staging = paths.cartridges.join(".staging");
    assert!(std::fs::read_dir(&staging).map_or(true, |mut listing| listing.next().is_none()));
}

#[test]
fn a_digest_that_does_not_match_installs_nothing() {
    let temp = Temp::new("mismatch");
    let (repo, folder, commit) = published(temp.path(), "hello", true);
    let paths = paths(temp.path());
    let entry = Entry {
        sha256: "0".repeat(64),
        ..entry_for(&repo, &folder, &commit)
    };
    let error =
        install::install(&paths, &entry, "test", false, false, Output::Inherit, None).unwrap_err();
    assert!(
        error.contains("does not match the registry") && error.contains("nothing was installed"),
        "{error}"
    );
    assert!(!paths.cartridges.join("hello").exists());
    assert!(
        !folder.join("runtime").exists()
            && !paths
                .cartridges
                .join(".staging")
                .read_dir()
                .is_ok_and(|mut l| l.next().is_some())
    );
    // A registry that names another version than the plugin is refused too.
    let entry = Entry {
        version: "9.9.9".into(),
        ..entry_for(&repo, &folder, &commit)
    };
    assert!(
        install::install(&paths, &entry, "test", false, false, Output::Inherit, None)
            .unwrap_err()
            .contains("says 0.1.0")
    );
}

#[test]
fn an_update_replaces_the_files_and_keeps_what_the_setup_made() {
    let temp = Temp::new("update");
    let (repo, folder, first) = published(temp.path(), "hello", true);
    let paths = paths(temp.path());
    let (_, sink) = collect();
    install::install(
        &paths,
        &entry_for(&repo, &folder, &first),
        "test",
        false,
        false,
        Output::Sink(&sink),
        None,
    )
    .unwrap();
    let dest = paths.cartridges.join("hello");
    write(&dest.join("index/data.db"), "the plugin's own index");
    // The next version drops LICENSE, adds NOTES.md and bumps the version.
    std::fs::remove_file(folder.join("LICENSE")).unwrap();
    write(&folder.join("NOTES.md"), "new\n");
    let toml = folder.join("cartridge.toml");
    write(
        &toml,
        &std::fs::read_to_string(&toml)
            .unwrap()
            .replace("version = \"0.1.0\"", "version = \"0.2.0\""),
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "second"]);
    let second = git(&repo, &["rev-parse", "HEAD"]);
    let (seen, sink) = collect();
    let outcome = install::install(
        &paths,
        &entry_for(&repo, &folder, &second),
        "test",
        false,
        false,
        Output::Sink(&sink),
        None,
    )
    .unwrap();
    assert_eq!(
        (outcome.replaced.as_deref(), outcome.version.as_str()),
        (Some("0.1.0"), "0.2.0")
    );
    assert!(
        !dest.join("LICENSE").exists(),
        "a file the new version dropped is gone"
    );
    assert!(dest.join("NOTES.md").is_file());
    assert_eq!(
        std::fs::read_to_string(dest.join("index/data.db")).unwrap(),
        "the plugin's own index"
    );
    assert!(dest.join("runtime/made").is_file());
    assert!(
        String::from_utf8_lossy(&seen.lock().unwrap()).contains("kept"),
        "the setup found its work done"
    );
}

#[test]
fn a_failing_setup_leaves_the_files_and_says_so() {
    let temp = Temp::new("setup-fails");
    let (repo, folder, _) = published(temp.path(), "hello", true);
    write(
        &folder.join("setup.sh"),
        "#!/bin/sh\necho trying >&2\nexit 3\n",
    );
    chmod_x(&folder.join("setup.sh"));
    git(&repo, &["commit", "-q", "-am", "broken setup"]);
    let commit = git(&repo, &["rev-parse", "HEAD"]);
    let paths = paths(temp.path());
    let (seen, sink) = collect();
    let outcome = install::install(
        &paths,
        &entry_for(&repo, &folder, &commit),
        "test",
        false,
        false,
        Output::Sink(&sink),
        None,
    )
    .unwrap();
    let error = outcome.setup.unwrap().unwrap_err();
    assert!(error.contains("exited"), "{error}");
    assert!(String::from_utf8_lossy(&seen.lock().unwrap()).contains("trying"));
    assert!(paths.cartridges.join("hello/cartridge.toml").is_file());
    let Origin::Registry(receipt) = install::find(&paths, "hello").unwrap().origin else {
        panic!()
    };
    assert_eq!(receipt.setup_ok, Some(false));
    assert!(cli::list(&paths, Surface::Shell).contains("setup did not finish"));
}

#[test]
fn install_will_not_replace_a_hand_placed_cartridge_unless_forced() {
    let temp = Temp::new("placed");
    let (repo, folder, commit) = published(temp.path(), "hello", false);
    let paths = paths(temp.path());
    write(
        &paths.cartridges.join("hello/cartridge.toml"),
        "id = \"hello\"\nlabel = \"Mine\"\n[[commands]]\nname = \"hello\"\nskills = [\"x\"]\n",
    );
    let entry = entry_for(&repo, &folder, &commit);
    let error =
        install::install(&paths, &entry, "test", false, false, Output::Inherit, None).unwrap_err();
    assert!(error.contains("--force"), "{error}");
    assert!(
        std::fs::read_to_string(paths.cartridges.join("hello/cartridge.toml"))
            .unwrap()
            .contains("Mine")
    );
    install::install(&paths, &entry, "test", true, false, Output::Inherit, None).unwrap();
    assert!(matches!(
        install::find(&paths, "hello").unwrap().origin,
        Origin::Registry(_)
    ));
}

#[test]
fn link_and_remove_never_touch_the_linked_folder() {
    let temp = Temp::new("link");
    let folder = scaffold::create("mine", Some(&temp.path().join("work/mine")), "Tester").unwrap();
    let paths = paths(temp.path());
    let (line, report) = install::link(&paths, &folder, false).unwrap();
    assert!(line.contains("linked") && report.ok(), "{line}");
    let found = install::find(&paths, "mine").unwrap();
    assert!(
        matches!(&found.origin, Origin::Linked(target) if target == &folder.canonicalize().unwrap())
    );
    assert!(
        install::link(&paths, &folder, false)
            .unwrap()
            .0
            .contains("already linked")
    );
    let said = install::remove(&paths, "mine", false).unwrap();
    assert!(said.contains("unlinked"), "{said}");
    assert!(
        folder.join("cartridge.toml").is_file(),
        "the work folder stays"
    );
    assert!(install::find(&paths, "mine").is_none());
}

#[test]
fn remove_takes_only_folders_directly_in_the_plugin_folder() {
    let temp = Temp::new("remove");
    let paths = paths(temp.path());
    write(
        &temp.path().join("precious/cartridge.toml"),
        "id = \"precious\"\nlabel = \"P\"\n",
    );
    std::fs::create_dir_all(&paths.cartridges).unwrap();
    for id in ["../precious", "..", ".", ".staging", "a/b", ""] {
        assert!(install::remove(&paths, id, true).is_err(), "{id}");
    }
    assert!(temp.path().join("precious/cartridge.toml").is_file());
    write(
        &paths.cartridges.join("placed/cartridge.toml"),
        "id = \"placed\"\nlabel = \"P\"\n",
    );
    assert!(
        install::remove(&paths, "placed", false)
            .unwrap_err()
            .contains("--force")
    );
    assert!(install::remove(&paths, "placed", true).is_ok());
    assert!(!paths.cartridges.join("placed").exists());
}

#[test]
fn an_angelx_requirement_above_this_version_refuses_the_install() {
    let temp = Temp::new("needs");
    let (repo, folder, _) = published(temp.path(), "hello", false);
    let toml = folder.join("cartridge.toml");
    write(
        &toml,
        &std::fs::read_to_string(&toml).unwrap().replace(
            "license = \"MIT\"\n",
            "license = \"MIT\"\nangelx = \"999.0.0\"\n",
        ),
    );
    git(&repo, &["commit", "-q", "-am", "needs the future"]);
    let commit = git(&repo, &["rev-parse", "HEAD"]);
    let paths = paths(temp.path());
    let error = install::install(
        &paths,
        &entry_for(&repo, &folder, &commit),
        "test",
        false,
        false,
        Output::Inherit,
        None,
    )
    .unwrap_err();
    assert!(error.contains("needs angelX 999.0.0 or newer"), "{error}");
    assert!(!paths.cartridges.join("hello").exists());
}

#[test]
fn the_cli_searches_shows_installs_lists_updates_and_removes() {
    let temp = Temp::new("cli");
    let (repo, folder, commit) = published(temp.path(), "hello", true);
    let paths = paths(temp.path());
    let entry = entry_for(&repo, &folder, &commit);
    registry_file(&paths, &[&entry]);
    let run = |words: &[&str]| {
        let words: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        let (_, sink) = collect();
        cli::run(
            &paths,
            &words,
            Surface::Shell,
            &|_| {},
            Output::Sink(&sink),
            None,
        )
    };
    let text = |words: &[&str]| match run(words) {
        Ok(text) => text,
        Err(cli::Failure::Usage(text) | cli::Failure::Failed(text)) => panic!("{words:?}: {text}"),
    };
    assert!(text(&[]).contains("no plugins in"));
    let found = text(&["search", "line", "hello"]);
    assert!(found.contains("1 plugin matches `line hello`"), "{found}");
    assert!(text(&["search", "nothing-like-it"]).contains("no plugin in the registry matches"));
    let shown = text(&["info", "hello"]);
    assert!(
        shown.contains("digest matches the registry")
            && shown.contains("/hello")
            && shown.contains("setup.sh"),
        "{shown}"
    );
    assert!(
        shown.contains("`angelX plugin install hello` installs it"),
        "{shown}"
    );
    let installed = text(&["install", "hello"]);
    assert!(
        installed.contains("installed hello 0.1.0") && installed.contains("setup finished"),
        "{installed}"
    );
    let listed = text(&["list"]);
    assert!(
        listed.contains("hello") && listed.contains("registry at"),
        "{listed}"
    );
    assert!(text(&["search"]).contains("installed"));
    assert!(text(&["update"]).contains("up to date"));
    assert!(text(&["setup", "hello"]).contains("setup finished"));
    assert!(
        matches!(run(&["install", "nope"]), Err(cli::Failure::Failed(text)) if text.contains("no plugin `nope`"))
    );
    // A malformed entry is refused before anything is fetched for it.
    let saved = std::fs::read_to_string(&paths.registry).unwrap();
    write(
        Path::new(&paths.registry),
        &saved.replace(&entry.commit, "not-a-commit"),
    );
    assert!(
        matches!(run(&["info", "hello"]), Err(cli::Failure::Failed(text)) if text.contains("malformed"))
    );
    write(Path::new(&paths.registry), &saved);
    assert!(matches!(run(&["install"]), Err(cli::Failure::Usage(_))));
    assert!(matches!(run(&["frobnicate"]), Err(cli::Failure::Usage(_))));
    assert!(matches!(
        run(&["install", "hello", "--yes-please"]),
        Err(cli::Failure::Usage(_))
    ));
    let removed = text(&["remove", "hello"]);
    assert!(removed.contains("removed hello"), "{removed}");
    assert!(text(&[]).contains("no plugins in"));
    let checked = text(&["check", folder.to_str().unwrap()]);
    assert!(checked.contains("ready for the registry"), "{checked}");
}

#[test]
fn submit_builds_the_entry_from_the_pushed_commit_and_writes_it_into_a_registry() {
    let temp = Temp::new("submit");
    let (repo, folder, commit) = published(temp.path(), "hello", false);
    // The author's working clone; the first repository stands in for GitHub.
    let clone = temp.path().join("clone");
    git(
        temp.path(),
        &[
            "clone",
            "-q",
            repo.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    let work = clone.join("plugins/hello");
    let paths = paths(temp.path());
    let prepared = submit::prepare(&paths, &work, None, None, false, None).unwrap();
    let entry = &prepared.entry;
    assert_eq!(
        (
            entry.id.as_str(),
            entry.commit.as_str(),
            entry.path.as_str()
        ),
        ("hello", commit.as_str(), "plugins/hello")
    );
    assert_eq!(
        entry.repository,
        repo.display().to_string(),
        "the remote of the clone"
    );
    assert_eq!(entry.sha256, digest::digest_folder(&folder).unwrap());
    // The public registry would want an https repository.
    let public = submit::prepare(&paths, &work, None, None, true, None);
    assert!(public.err().unwrap().contains("public https"));
    // An uncommitted change stops a HEAD submission.
    write(&work.join("NOTES.md"), "draft\n");
    assert!(
        submit::prepare(&paths, &work, None, None, false, None)
            .err()
            .unwrap()
            .contains("not committed")
    );
    std::fs::remove_file(work.join("NOTES.md")).unwrap();
    // A commit that was never pushed cannot be fetched back.
    write(&work.join("NOTES.md"), "local\n");
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "local only"]);
    assert!(
        submit::prepare(&paths, &work, None, None, false, None)
            .err()
            .unwrap()
            .contains("push the commit")
    );
    // An explicit commit is taken as it is in the repository.
    let pinned = submit::prepare(&paths, &work, Some(&commit), None, false, None).unwrap();
    assert_eq!(pinned.entry.sha256, entry.sha256);
    assert!(submit::prepare(&paths, &work, Some("--upload-pack=x"), None, false, None).is_err());
    // Into a registry file, then installed from it.
    let file = Path::new(&paths.registry);
    write(file, "# test registry\n\n");
    assert!(
        submit::into_file(&prepared, file)
            .unwrap()
            .contains("plugins: add hello 0.1.0")
    );
    assert!(
        submit::into_file(&prepared, file)
            .unwrap()
            .contains("already lists")
    );
    let registry = registry::load(&paths).unwrap();
    assert!(
        registry.problems(false).is_empty(),
        "{:?}",
        registry.problems(false)
    );
    let entry = registry.get("hello").unwrap();
    install::install(
        &paths,
        entry,
        &registry.source,
        false,
        false,
        Output::Inherit,
        None,
    )
    .unwrap();
    assert!(
        paths
            .cartridges
            .join("hello/skills/hello/SKILL.md")
            .is_file()
    );
}

#[test]
fn remotes_become_https_addresses() {
    for (remote, url) in [
        (
            "git@github.com:me/plugin.git",
            "https://github.com/me/plugin",
        ),
        ("git@github.com:me/plugin", "https://github.com/me/plugin"),
        (
            "ssh://git@gitlab.com/me/group/plugin.git",
            "https://gitlab.com/me/group/plugin",
        ),
        (
            "ssh://git@codeberg.org:22/me/plugin",
            "https://codeberg.org/me/plugin",
        ),
        (
            "https://me:token@github.com/me/plugin.git",
            "https://github.com/me/plugin",
        ),
        (
            "https://github.com/me/plugin/",
            "https://github.com/me/plugin",
        ),
        ("/srv/git/plugin", "/srv/git/plugin"),
    ] {
        assert_eq!(submit::public_url(remote), url, "{remote}");
    }
}

#[test]
fn a_pull_request_goes_through_the_github_cli() {
    let _env = crate::tests::env_lock();
    let temp = Temp::new("gh");
    let registry_text = registry::upsert(
        "# The angelX plugin registry.\n\n",
        &Entry {
            id: "zeta".into(),
            path: "z".into(),
            ..sample()
        },
    )
    .unwrap()
    .0;
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(registry_text.as_bytes());
    write(&temp.path().join("registry.b64"), &encoded);
    let gh = temp.path().join("gh");
    write(
        &gh,
        r#"#!/bin/sh
dir=$(dirname "$0")
printf '%s\n' "$*" >> "$dir/calls"
case "$1 $2" in
  "auth status") exit 0 ;;
  "api user") echo tester ;;
  "repo fork") echo "! tester/angelX already exists" >&2 ;;
  "api repos/newjordan/angelX/git/ref/heads/main") echo 1111111111111111111111111111111111111111 ;;
  "api repos/tester/angelX") echo newjordan/angelX ;;
  "pr create") echo https://github.com/newjordan/angelX/pull/77 ;;
  "api -X")
    if [ "$3" = PUT ]; then cat > "$dir/put.json"; fi
    echo '{}' ;;
  *)
    case "$2" in
      repos/newjordan/angelX/contents/*) printf '{"sha": "blobsha", "content": "%s"}' "$(cat "$dir/registry.b64")" ;;
      *) echo "unexpected: $*" >&2; exit 1 ;;
    esac ;;
esac
"#,
    );
    chmod_x(&gh);
    let _gh = crate::tests::TestEnvGuard::set("ANGEL_PLUGIN_GH", gh.to_str().unwrap());
    let (repo, folder, commit) = published(temp.path(), "hello", false);
    let prepared = submit::Prepared {
        entry: Entry {
            repository: "https://example.com/tester/hello".into(),
            ..entry_for(&repo, &folder, &commit)
        },
        config: check::folder(&folder, Level::Publish).config.unwrap(),
        report: check::Report::default(),
    };
    let url = submit::open_pull_request(&prepared).unwrap();
    assert_eq!(url, "https://github.com/newjordan/angelX/pull/77");
    let calls = std::fs::read_to_string(temp.path().join("calls")).unwrap();
    assert!(
        calls.contains("repo fork newjordan/angelX --clone=false --remote=false"),
        "{calls}"
    );
    assert!(calls.contains("api -X POST repos/tester/angelX/git/refs -f ref=refs/heads/plugin/hello-0.1.0 -f sha=1111111111111111111111111111111111111111"), "{calls}");
    assert!(calls.contains("pr create --repo newjordan/angelX --base main --head tester:plugin/hello-0.1.0 --title plugins: add hello 0.1.0"), "{calls}");
    let put: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(temp.path().join("put.json")).unwrap())
            .unwrap();
    assert_eq!(
        (put["sha"].as_str(), put["branch"].as_str()),
        (Some("blobsha"), Some("plugin/hello-0.1.0"))
    );
    let sent = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(put["content"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    let parsed = Registry::parse(&sent, "sent").unwrap();
    assert_eq!(
        parsed
            .entries
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["hello", "zeta"]
    );
    assert_eq!(parsed.get("hello").unwrap(), &prepared.entry);
    assert!(sent.starts_with("# The angelX plugin registry."));
}

#[test]
fn a_reload_plugs_in_a_new_cartridge_and_unplugs_a_removed_one() {
    let _env = crate::tests::env_lock();
    use crate::agent::harness::cartridges;
    let temp = Temp::new("reload");
    let root = temp.path().join("cartridges");
    scaffold::create("reload-me", Some(&root.join("reload-me")), "Tester").unwrap();
    let _roots = crate::tests::TestEnvGuard::set("ANGEL_CARTRIDGES", root.to_str().unwrap());
    let before: Vec<String> = cartridges::all()
        .iter()
        .map(|c| c.id().to_owned())
        .collect();
    cartridges::reload();
    assert!(cartridges::command("reload-me").is_some());
    assert!(crate::ui::input::cartridge_command("reload-me").is_some());
    assert_eq!(
        cartridges::all().first().map(|c| c.id()),
        before.first().map(String::as_str),
        "the cartridges already in keep their places"
    );
    std::fs::remove_dir_all(root.join("reload-me")).unwrap();
    cartridges::reload();
    assert!(cartridges::command("reload-me").is_none());
    drop(_roots);
    cartridges::reload();
}

#[test]
fn the_registry_in_this_repository_passes_its_own_rules() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plugins/registry.toml"
    ))
    .unwrap();
    let registry = Registry::parse(&text, "plugins/registry.toml").unwrap();
    assert!(
        registry.problems(true).is_empty(),
        "{:?}",
        registry.problems(true)
    );
}

#[test]
fn the_cockpit_reads_folders_in_the_workspace() {
    let anchored = |line: &str| {
        let mut words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        crate::app::control::plugins::anchor_folders(&mut words, Path::new("/work/space"));
        words.join(" ")
    };
    assert_eq!(anchored("check"), "check /work/space");
    assert_eq!(anchored("check mine"), "check /work/space/mine");
    assert_eq!(anchored("link /abs/mine --force"), "link /abs/mine --force");
    assert_eq!(anchored("new demo"), "new demo /work/space/demo");
    assert_eq!(
        anchored("new demo lab/demo"),
        "new demo /work/space/lab/demo"
    );
    assert_eq!(
        anchored("submit --commit abc mine --into reg.toml"),
        "submit --commit abc /work/space/mine --into /work/space/reg.toml"
    );
    assert_eq!(anchored("install vfx"), "install vfx");
}

#[test]
fn the_cockpit_answers_local_commands_at_once_and_fetches_in_the_background() {
    let _env = crate::tests::env_lock();
    let temp = Temp::new("cockpit");
    let _dir = crate::tests::TestEnvGuard::set(
        "ANGEL_PLUGIN_DIR",
        temp.path().join("cartridges").to_str().unwrap(),
    );
    let _registry = crate::tests::TestEnvGuard::set(
        "ANGEL_PLUGIN_REGISTRY",
        temp.path().join("no-registry.toml").to_str().unwrap(),
    );
    let folder = scaffold::create(
        "cockpit-demo",
        Some(&temp.path().join("cockpit-demo")),
        "Tester",
    )
    .unwrap();
    let mut app = crate::app::App::new(
        crate::agent::club::Bag::practice_for_test(),
        crate::ui::viewer::Viewer::new(),
    );
    let mut say = |line: &str| {
        app.input = line.to_owned();
        app.submit();
        assert!(app.thinking.is_none(), "{line} sent no model turn");
        app.messages.last().unwrap().text.clone()
    };
    assert!(say("/plugins").contains("no plugins in"));
    assert!(say("/plugin remove nope").contains("not installed"));
    let checked = say(&format!("/plugins check {}", folder.display()));
    assert!(checked.contains("ready for the registry"), "{checked}");
    let linked = say(&format!("/plugins link {}", folder.display()));
    assert!(linked.contains("linked"), "{linked}");
    assert!(say("/plugins").contains("cockpit-demo"));
    assert!(say("/plugins remove cockpit-demo").contains("its commands are gone now"));
    let started = say("/plugins search");
    assert!(started.contains("Esc cancels"), "{started}");
    assert!(
        app.bg_job.is_some(),
        "the registry is read off the UI thread"
    );
}

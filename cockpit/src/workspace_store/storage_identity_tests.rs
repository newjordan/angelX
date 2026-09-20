//! Staged for cockpit/src/workspace_store/storage_identity_tests.rs.
use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-storage-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn owned_git(command: Command) {
    assert!(
        capture_repo_command(command, Duration::from_secs(5)).is_some(),
        "owned Git fixture command failed or exceeded its deadline"
    );
}
fn init_repo(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    let mut command = Command::new("git");
    command.args(["init", "-q"]).current_dir(path);
    owned_git(command);
}

fn identity_env(fixture: &Fixture) -> [crate::tests::TestEnvGuard; 5] {
    [
        crate::tests::TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        crate::tests::TestEnvGuard::set(
            "ANGEL_WORK_CONTEXT_DIR",
            fixture.0.join("context").to_str().unwrap(),
        ),
        crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "1"),
        crate::tests::TestEnvGuard::unset("GIT_DIR"),
        crate::tests::TestEnvGuard::unset("GIT_WORK_TREE"),
    ]
}

fn assert_isolated_child(command: Command) {
    let captured =
        capture_command_with_deadline(command, Duration::from_secs(5), GIT_CAPTURE_LIMIT_BYTES)
            .expect("isolated identity child must launch")
            .expect("isolated identity child exceeded its deadline");
    let (status, output) = captured;
    let stdout = String::from_utf8_lossy(&output.bytes);
    assert!(
        status.success(),
        "isolated identity child failed:\n{stdout}"
    );
    assert!(
        stdout.contains("test result: ok. 1 passed; 0 failed;"),
        "isolated identity child did not run exactly one case:\n{stdout}"
    );
}

#[test]
fn ordinary_repo_subdirs_share_the_main_identity() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let _env = identity_env(&fixture);
    let repo = fixture.0.join("main");
    let subdir = repo.join("cockpit");
    init_repo(&repo);
    std::fs::create_dir(&subdir).unwrap();

    let from_subdir = repo_identity(&subdir);
    let from_root = repo_identity(&repo);
    assert_eq!(from_subdir.root, repo);
    assert_eq!(from_subdir.key, from_root.key);
    assert_eq!(from_root.key, workspace_key(&repo));
}

#[test]
fn identity_cache_keeps_legacy_and_repository_modes_separate() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let _env = identity_env(&fixture);
    let repo = fixture.0.join("main");
    let subdir = repo.join("cockpit");
    init_repo(&repo);
    std::fs::create_dir(&subdir).unwrap();

    let legacy = {
        let _legacy = crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "0");
        repo_identity(&subdir)
    };
    assert_eq!(legacy.root, subdir);

    let repository = repo_identity(&subdir);
    assert_eq!(repository.root, repo);
    assert_ne!(legacy.key, repository.key);
}

#[test]
fn local_checkout_fast_path_stops_at_gitfile_or_junk_marker() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let _env = identity_env(&fixture);
    let outer = fixture.0.join("outer");
    init_repo(&outer);

    let gitfile = outer.join("linked");
    std::fs::create_dir(&gitfile).unwrap();
    std::fs::write(gitfile.join(".git"), "gitdir: /does/not/exist\n").unwrap();
    assert_eq!(local_checkout_root(&gitfile), None);

    let junk = fixture.0.join("junk");
    std::fs::create_dir_all(junk.join(".git")).unwrap();
    assert_eq!(local_checkout_root(&junk), None);
}

#[test]
fn explicit_git_directory_overrides_bypass_the_local_fast_path() {
    const CHILD: &str = "ANGEL_T_IDENTITY_GIT_OVERRIDE_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        let root = PathBuf::from(root);
        assert_eq!(
            canonical_repo_root(&root.join("workspace/cockpit")),
            root.join("selected")
        );
        return;
    }

    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let selected = fixture.0.join("selected");
    let workspace_repo = fixture.0.join("workspace");
    let workspace = workspace_repo.join("cockpit");
    init_repo(&selected);
    init_repo(&workspace_repo);
    std::fs::create_dir(&workspace).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "workspace_store::storage_identity_tests::explicit_git_directory_overrides_bypass_the_local_fast_path",
            "--nocapture",
        ])
        .env(CHILD, &fixture.0)
        .env("ANGEL_REPO_IDENTITY", "1")
        .env("HOME", &fixture.0)
        .env("ANGEL_WORK_CONTEXT_DIR", fixture.0.join("context"))
        .env("GIT_DIR", selected.join(".git"))
        .env("GIT_WORK_TREE", &selected);
    assert_isolated_child(child);
}

#[cfg(unix)]
#[test]
fn timed_out_identity_probe_is_not_memoized_as_workspace_only() {
    const CHILD: &str = "ANGEL_T_IDENTITY_TIMEOUT_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        let root = PathBuf::from(root);
        let repo = root.join("repo");
        let subdir = repo.join("cockpit");
        let first = repo_identity(&subdir);
        assert_eq!(first.root, subdir);

        std::fs::remove_file(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join(".git/objects")).unwrap();
        std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(
            repo.join(".git/config"),
            "[core]\nrepositoryformatversion = 0\n",
        )
        .unwrap();
        let recovered = repo_identity(&subdir);
        assert_eq!(recovered.root, repo);
        assert_ne!(first.key, recovered.key);
        return;
    }

    use std::os::unix::fs::PermissionsExt as _;

    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let repo = fixture.0.join("repo");
    let subdir = repo.join("cockpit");
    std::fs::create_dir_all(&subdir).unwrap();
    // A gitfile routes this first lookup through the bounded subprocess path.
    std::fs::write(repo.join(".git"), "gitdir: /does/not/exist\n").unwrap();
    let bin = fixture.0.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let fake_git = bin.join("git");
    std::fs::write(&fake_git, "#!/bin/sh\nsleep 1\n").unwrap();
    let mut permissions = std::fs::metadata(&fake_git).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_git, permissions).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "workspace_store::storage_identity_tests::timed_out_identity_probe_is_not_memoized_as_workspace_only",
            "--nocapture",
        ])
        .env(CHILD, &fixture.0)
        .env("PATH", path)
        .env("ANGEL_REPO_IDENTITY", "1")
        .env("HOME", &fixture.0)
        .env("ANGEL_WORK_CONTEXT_DIR", fixture.0.join("context"))
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE");
    assert_isolated_child(child);
}

#[test]
fn valid_utf8_workspace_keys_keep_their_pre_migration_goldens() {
    for (path, expected) in [
        ("/home/u/alpha", "home-u-alpha-0223363ac31fcc24"),
        ("/tmp/naïve 🦀", "tmp-na-ve-b49af554cd0e5e54"),
        ("/", "ws-cf1d3eb93fcb5a7a"),
        ("", "ws-30406ea523c53def"),
        (" leading/trailing ", "leading-trailing-5d39ff866524be3a"),
    ] {
        assert_eq!(workspace_key(Path::new(path)), expected, "{path:?}");
    }
}

#[test]
fn nul_worktree_parser_preserves_whitespace_and_rejects_incomplete_records() {
    let raw = b"worktree /tmp/ leading\ntrailing \0HEAD abc\0\0worktree /tmp/second\0";
    assert_eq!(
        main_worktree_from_porcelain_z(raw),
        Some(PathBuf::from("/tmp/ leading\ntrailing "))
    );
    assert_eq!(main_worktree_from_porcelain_z(b"worktree \0"), None);
    assert_eq!(
        main_worktree_from_porcelain_z(b"worktree /tmp/incomplete"),
        None
    );
    assert_eq!(main_worktree_from_porcelain_z(b"HEAD abc\0"), None);
}

#[cfg(not(unix))]
#[test]
fn nul_worktree_parser_rejects_non_utf8_without_guessing() {
    assert_eq!(
        main_worktree_from_porcelain_z(b"worktree /tmp/bad-\xff\0"),
        None
    );
}

#[cfg(unix)]
#[test]
fn real_git_roots_keep_raw_names_distinct_and_do_not_alias_legacy_lossy_keys() {
    use std::os::unix::ffi::OsStringExt as _;
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let _env = [
        crate::tests::TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        crate::tests::TestEnvGuard::set(
            "ANGEL_WORK_CONTEXT_DIR",
            fixture.0.join("context").to_str().unwrap(),
        ),
        crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "1"),
        crate::tests::TestEnvGuard::unset("GIT_DIR"),
        crate::tests::TestEnvGuard::unset("GIT_WORK_TREE"),
    ];
    let names = [
        b"raw-\xff".as_slice(),
        b"raw-\xfe",
        b"trailing ",
        b"with\nnewline",
    ];
    let mut identities = Vec::new();
    for name in names {
        let path = fixture.0.join(std::ffi::OsString::from_vec(name.to_vec()));
        init_repo(&path);
        assert_eq!(
            canonical_repo_root(&path),
            path,
            "Git parsing must preserve the actual directory bytes"
        );
        let identity = repo_identity(&path);
        assert_eq!(identity.root, path);
        assert_eq!(identity.key, workspace_key(&path));
        assert!(matches_project(&path, &path, &identity.key));
        identities.push(identity);
    }
    assert_eq!(
        identities[0].root.to_string_lossy(),
        identities[1].root.to_string_lossy()
    );
    assert_ne!(
        identities[0].key, identities[1].key,
        "distinct real invalid-UTF8 repos cannot share memory"
    );
    assert!(!matches_project(
        &identities[0].root,
        &identities[1].root,
        &identities[1].key
    ));
    let old_ambiguous_key = workspace_key(Path::new(identities[0].root.to_string_lossy().as_ref()));
    for identity in &identities[..2] {
        assert_ne!(identity.key, old_ambiguous_key);
        assert!(
            !matches_project(&identity.root, &identity.root, &old_ambiguous_key),
            "ambiguous legacy state cannot be silently attributed to one raw path"
        );
    }
}

#[cfg(unix)]
#[test]
fn linked_worktree_subdir_and_alias_preserve_an_unusual_main_root() {
    let _lock = crate::tests::env_lock();
    let fixture = Fixture::new();
    let _env = [
        crate::tests::TestEnvGuard::set("HOME", fixture.0.to_str().unwrap()),
        crate::tests::TestEnvGuard::set(
            "ANGEL_WORK_CONTEXT_DIR",
            fixture.0.join("context").to_str().unwrap(),
        ),
        crate::tests::TestEnvGuard::set("ANGEL_REPO_IDENTITY", "1"),
        crate::tests::TestEnvGuard::unset("GIT_DIR"),
        crate::tests::TestEnvGuard::unset("GIT_WORK_TREE"),
    ];
    let main = fixture.0.join("main\nwith trailing ");
    let linked = fixture.0.join("linked\nworktree ");
    init_repo(&main);
    let mut command = Command::new("git");
    command
        .args([
            "-c",
            "user.name=Owned fixture",
            "-c",
            "user.email=owned@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "--allow-empty",
            "-qm",
            "owned fixture",
        ])
        .current_dir(&main);
    owned_git(command);
    let mut command = Command::new("git");
    command
        .args(["worktree", "add", "--detach", "-q"])
        .arg(&linked)
        .arg("HEAD")
        .current_dir(&main);
    owned_git(command);
    let subdir = linked.join("subdir");
    std::fs::create_dir(&subdir).unwrap();
    for path in [&main, &linked, &subdir] {
        assert_eq!(canonical_repo_root(path), main);
        assert_eq!(repo_identity(path).key, workspace_key(&main));
    }
    let alias = fixture.0.join("alias");
    std::os::unix::fs::symlink(&main, &alias).unwrap();
    assert_eq!(
        canonical_repo_root(&alias),
        alias,
        "same-directory caller spelling stays compatible"
    );
}

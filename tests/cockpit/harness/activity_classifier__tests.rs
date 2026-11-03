use super::*;
use std::path::Path;

fn compile_shared(path: &Path, value: i32, version: u32) {
    let src = path.with_extension("c");
    std::fs::write(&src, format!(r#"
#include <stddef.h>
#include <stdint.h>
typedef int (*predicate)(const void *, int, const unsigned char *, size_t);
typedef int (*spacefn)(uint32_t);
uint32_t angel_activity_classifier_abi_version(void) {{ return {version}; }}
int angel_activity_git_v1(const unsigned char *s, size_t n, int mode, predicate f, const void *ctx, spacefn space) {{
    (void)s; (void)n; (void)mode; (void)f; (void)ctx; (void)space; return {value};
}}
"#)).unwrap();
    let mut command = std::process::Command::new("cc");
    if cfg!(target_os = "macos") {
        command.arg("-dynamiclib");
    } else {
        command.args(["-shared", "-fPIC"]);
    }
    assert!(
        command
            .args(["-std=c11", "-O2"])
            .arg(src)
            .arg("-o")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}

fn compile_competition_shared(path: &Path, value: i32) {
    let src = path.with_extension("c");
    std::fs::write(&src, format!(r#"
#include <stddef.h>
#include <stdint.h>
typedef int (*predicate)(const void *, int, const unsigned char *, size_t);
typedef int (*spacefn)(uint32_t);
typedef int (*unicodefn)(uint32_t, int);
uint32_t angel_activity_classifier_abi_version(void) {{ return 1; }}
int angel_activity_git_v1(const unsigned char *s, size_t n, int mode, predicate f, const void *ctx, spacefn space) {{
    (void)s; (void)n; (void)mode; (void)f; (void)ctx; (void)space; return 0;
}}
int angel_activity_competition_v1(const unsigned char *s, size_t n, int mode, unicodefn classify) {{
    (void)s; (void)n; (void)mode; (void)classify; return {value};
}}
"#)).unwrap();
    let mut command = std::process::Command::new("cc");
    if cfg!(target_os = "macos") {
        command.arg("-dynamiclib");
    } else {
        command.args(["-shared", "-fPIC"]);
    }
    assert!(
        command
            .args(["-std=c11", "-O2"])
            .arg(src)
            .arg("-o")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn competition_inventory_preserves_unicode_and_listing_rules() {
    let generation = baseline();
    for (source, inventory, listing) in [
        ("cat LIVING_HANDOFF.md", false, false),
        ("git show HEAD:board.md", false, false),
        ("git show --stat HEAD:board.md", true, false),
        ("git show HEAD -- board.md", true, false),
        ("git status -- board.md", true, false),
        ("FOO=1\u{2003}git\u{85}show HEAD:board.md", false, false),
        ("FOO=1\u{a0}/bin/ls\u{2003}-lar board.md", true, true),
        ("ls --recursive=1 board.md", true, true),
        ("ls --color=auto board.md", false, false),
        ("find . -name board.md", true, false),
        ("printf 'hello\0world'", false, false),
        ("git\0 show HEAD:board.md", false, false),
    ] {
        assert_eq!(
            competition_with(&generation, source, 0),
            Some(inventory),
            "{source:?}"
        );
        assert_eq!(
            competition_with(&generation, source, 1),
            Some(listing),
            "{source:?}"
        );
    }
}

#[test]
fn competition_classifier_is_optional_pins_and_falls_back() {
    let temp = crate::tests::TestGitWorkspace::new("competition-classifier-native-reload");
    let path = temp.path().join("module.so");
    let next = temp.path().join("next.so");
    compile_shared(&next, 0, 1);
    std::fs::rename(&next, &path).unwrap();
    let host = new_host(Some(path.clone()));
    let base = baseline();
    wait_until(|| !Arc::ptr_eq(&base, &host.current()));
    let legacy = host.current();
    assert_eq!(
        competition_with(&legacy, "git status -- board.md", 0),
        Some(true)
    );
    compile_competition_shared(&next, 0);
    std::fs::rename(&next, &path).unwrap();
    wait_until(|| !Arc::ptr_eq(&legacy, &host.current()));
    let first = host.current();
    assert_eq!(
        competition_with(&first, "git status -- board.md", 0),
        Some(false)
    );
    compile_competition_shared(&next, 1);
    std::fs::rename(&next, &path).unwrap();
    wait_until(|| !Arc::ptr_eq(&first, &host.current()));
    let second = host.current();
    assert_eq!(
        competition_with(&first, "git status -- board.md", 0),
        Some(false)
    );
    assert_eq!(
        competition_with(&second, "git status -- board.md", 0),
        Some(true)
    );
    compile_competition_shared(&next, 3);
    std::fs::rename(&next, &path).unwrap();
    wait_until(|| !Arc::ptr_eq(&second, &host.current()));
    let invalid = host.current();
    assert_eq!(
        competition_with(&invalid, "git status -- board.md", 0),
        None
    );
    assert!(competition_or_baseline(
        &invalid,
        "git status -- board.md",
        0
    ));
}
fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "activity classifier publication timed out"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}
#[test]
fn activity_classifier_preserves_modes_and_unicode_boundaries() {
    for (source, expected) in [
        ("git log -- src/kernel.cu", [true, false, false]),
        ("git grep TODO -- src/kernel.cu", [false, true, false]),
        ("git show HEAD:src/kernel.cu", [false, false, true]),
        (
            "FOO=1\u{2003}git\u{85}log\u{2003}--\u{a0}src/λ.rs",
            [true, false, false],
        ),
        ("git grep TODO -- living_handoff.md", [false, false, false]),
        ("git show --stat HEAD:src/kernel.cu", [false, false, false]),
        ("git show HEAD:'.angelX/notes/a", [false, false, false]),
        ("git show HEAD:'.angelX/handoff/a", [false, false, false]),
        ("git log -- src/file && echo x", [false, false, false]),
        ("git grep TODO", [false, false, false]),
    ] {
        let generation = baseline();
        for (mode, result) in expected.into_iter().enumerate() {
            assert_eq!(
                classify_with(&generation, source.trim(), mode as i32),
                Some(result),
                "{source:?} {mode}"
            );
        }
        assert_eq!(
            classify_with(&generation, source.trim(), -1),
            Some(expected.into_iter().any(|b| b)),
            "{source:?}"
        );
    }
}
#[test]
fn activity_callback_validates_ranges_and_allows_concurrent_reads() {
    let input = "git log -- src/λ.rs";
    let context = (&input as *const &str).cast();
    assert_eq!(predicate(context, 0, input.as_ptr(), input.len()), 0);
    assert_eq!(
        predicate(context, 0, unsafe { input.as_ptr().add(input.len()) }, 1),
        -1
    );
    let lambda = input.find('λ').unwrap();
    assert_eq!(
        predicate(context, 0, unsafe { input.as_ptr().add(lambda + 1) }, 1),
        -1
    );
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                let context = (&input as *const &str).cast();
                for _ in 0..500 {
                    assert_eq!(predicate(context, 0, input.as_ptr(), input.len()), 0);
                }
            });
        }
    });
}
#[test]
fn activity_classifier_reload_pins_generations_and_rejects_invalid_results() {
    let temp = crate::tests::TestGitWorkspace::new("activity-classifier-native-reload");
    let path = temp.path().join("module.so");
    let next = temp.path().join("next.so");
    compile_shared(&next, 1, 1);
    std::fs::rename(&next, &path).unwrap();
    let host = new_host(Some(path.clone()));
    let base = baseline();
    wait_until(|| !Arc::ptr_eq(&base, &host.current()));
    let first = host.current();
    assert_eq!(classify_with(&first, "git status", -1), Some(true));
    compile_shared(&next, 0, 1);
    std::fs::rename(&next, &path).unwrap();
    wait_until(|| !Arc::ptr_eq(&first, &host.current()));
    let second = host.current();
    assert_eq!(classify_with(&first, "git status", -1), Some(true));
    assert_eq!(classify_with(&second, "git status", -1), Some(false));
    compile_shared(&next, 3, 1);
    std::fs::rename(&next, &path).unwrap();
    wait_until(|| !Arc::ptr_eq(&second, &host.current()));
    let invalid = host.current();
    assert_eq!(classify_with(&invalid, "git show HEAD:src/a", -1), None);
    assert!(classify_or_baseline(&invalid, "git show HEAD:src/a", -1));
    compile_shared(&next, 1, 2);
    std::fs::rename(&next, &path).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    assert!(Arc::ptr_eq(&invalid, &host.current()));
}

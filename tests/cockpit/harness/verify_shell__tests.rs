use super::*;
use std::path::Path;

fn compile_shared(path: &Path, value: i32, version: u32) {
    let source = path.with_extension("c");
    std::fs::write(
        &source,
        format!(
            r#"
#include <stddef.h>
#include <stdint.h>
uint32_t angel_verify_shell_abi_version(void) {{ return {version}; }}
int angel_verify_shell_v1(const unsigned char *s, size_t n, int (*space)(uint32_t), int mode) {{
  (void)s; (void)n; (void)space; (void)mode; return {value};
}}
"#
        ),
    )
    .unwrap();
    let mut command = std::process::Command::new("cc");
    if cfg!(target_os = "macos") {
        command.arg("-dynamiclib");
    } else {
        command.args(["-shared", "-fPIC"]);
    }
    assert!(
        command
            .args(["-std=c11", "-O2"])
            .arg(source)
            .arg("-o")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}
fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "verifier shell reload timed out"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}
#[test]
fn verifier_shell_preserves_control_unicode_and_progress_modes() {
    let baseline = baseline();
    for (command, expected) in [
        ("cargo test", [true, true, false]),
        ("A=1\u{2003}cargo\u{85}check", [true, true, false]),
        ("cargo test; echo done", [false, true, false]),
        ("echo prep; cargo test", [true, true, false]),
        ("cargo test || true", [false, true, false]),
        ("echo 'cargo test'", [false, false, false]),
        ("cargo test &", [false, true, false]),
        ("echo $(cargo test)", [false, false, false]),
        ("cargo test # note", [false, false, false]),
        ("cat <<EOF\ncargo test\nEOF", [false, false, false]),
        ("ssh -p 22 host 'cargo test --all'", [true, true, false]),
        ("ssh -\0 host cargo test", [true, true, false]),
        ("make build", [false, false, true]),
        ("npm run build", [false, false, true]),
        ("CARGO BUILD", [false, false, false]),
    ] {
        for (mode, want) in expected.into_iter().enumerate() {
            assert_eq!(
                classify_with(&baseline, command, mode as i32),
                Some(want),
                "{command:?} mode {mode}"
            );
        }
    }
    assert_eq!(
        fast_assignment_cargo("A=1 cargo test --workspace"),
        Some(true)
    );
    assert_eq!(fast_assignment_cargo("A=1 cargo fmt --check"), Some(true));
    assert_eq!(fast_assignment_cargo("A=1 cargo test; echo done"), None);
}
#[test]
fn deep_ssh_verifier_chain_preserves_results_without_a_depth_cutoff() {
    for n in [127, 128, 129, 300, 1000] {
        let command = format!("{}cargo test", "ssh x ".repeat(n));
        assert_eq!(
            classify_with(&baseline(), &command, 0),
            Some(true),
            "depth {n}"
        );
        assert_eq!(
            classify_with(&baseline(), &command, 1),
            Some(true),
            "depth {n}"
        );
    }
}
#[test]
fn verifier_shell_reload_pins_and_rejects_bad_results() {
    let temp = crate::tests::TestGitWorkspace::new("verify-shell-reload");
    let path = temp.path().join("module.so");
    let next = temp.path().join("next.so");
    compile_shared(&next, 1, 1);
    std::fs::rename(&next, &path).unwrap();
    let host = new_host(Some(path.clone()));
    let base = baseline();
    wait_until(|| !Arc::ptr_eq(&base, &host.current()));
    let first = host.current();
    assert_eq!(classify_with(&first, "echo hi", 0), Some(true));
    compile_shared(&next, 0, 1);
    std::fs::rename(&next, &path).unwrap();
    wait_until(|| !Arc::ptr_eq(&first, &host.current()));
    let second = host.current();
    assert_eq!(classify_with(&first, "echo hi", 0), Some(true));
    assert_eq!(classify_with(&second, "echo hi", 0), Some(false));
    compile_shared(&next, 7, 1);
    std::fs::rename(&next, &path).unwrap();
    wait_until(|| !Arc::ptr_eq(&second, &host.current()));
    let bad = host.current();
    assert_eq!(classify_with(&bad, "cargo test", 0), None);
    assert_eq!(
        classify_with(&bad, "cargo test", 0).or_else(|| classify_with(&base, "cargo test", 0)),
        Some(true)
    );
    compile_shared(&next, 1, 2);
    std::fs::rename(&next, &path).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    assert!(Arc::ptr_eq(&bad, &host.current()));
}

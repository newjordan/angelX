use super::*;
use std::path::Path;

#[test]
fn native_routing_matches_original_golden_cases() {
    let bytes = include_bytes!("../fixtures/swarm-routing-golden.bin");
    let mut pos = 0;
    let mut count = 0;
    while pos < bytes.len() {
        let n = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let text = std::str::from_utf8(&bytes[pos..pos + n]).unwrap();
        pos += n;
        let bits = bytes[pos];
        pos += 1;
        for mode in 0..6 {
            assert_eq!(
                classify(text, mode),
                bits & (1 << mode) != 0,
                "{text:?} mode={mode}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 4009);
}

fn compile(path: &Path, result: i32) {
    let source = path.with_extension("c");
    std::fs::write(&source, format!("#include <stdint.h>\n#include <stddef.h>\nuint32_t angel_moa_route_abi_version(void) {{ return 1; }}\nint angel_moa_route_v1(const unsigned char *p,size_t n,int m,int (*s)(uint32_t)) {{ (void)p;(void)n;(void)m;(void)s; return {result}; }}\n")).unwrap();
    let mut cmd = std::process::Command::new("cc");
    if cfg!(target_os = "macos") {
        cmd.arg("-dynamiclib");
    } else {
        cmd.args(["-shared", "-fPIC"]);
    }
    assert!(
        cmd.args(["-std=c11", "-O2"])
            .arg(source)
            .arg("-o")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn publication_pins_generation_and_invalid_output_uses_builtin() {
    let temp = crate::tests::TestGitWorkspace::new("moa-routing-native");
    let path = temp.path().join("route.so");
    let next = temp.path().join("next.so");
    compile(&next, 1);
    std::fs::rename(&next, &path).unwrap();
    let host = new_host(Some(path.clone()));
    let base = baseline();
    let wait = |predicate: &dyn Fn() -> bool| {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !predicate() {
            assert!(
                std::time::Instant::now() < deadline,
                "router reload timed out"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    };
    wait(&|| !Arc::ptr_eq(&base, &host.current()));
    let first = host.current();
    assert!(classify_with(&host, "ordinary text", 0));
    compile(&next, 7);
    std::fs::rename(&next, &path).unwrap();
    wait(&|| !Arc::ptr_eq(&first, &host.current()));
    assert_eq!(run(&first, "ordinary text", 0), Some(true));
    assert!(!classify_with(&host, "ordinary text", 0));
    let valid = host.current();
    std::fs::write(&next, b"not a shared library").unwrap();
    std::fs::rename(&next, &path).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    assert!(Arc::ptr_eq(&valid, &host.current()));
}

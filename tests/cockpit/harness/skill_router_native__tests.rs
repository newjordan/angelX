use super::*;
use std::path::Path;

fn skill(name: &str, description: &str) -> SkillSummary {
    SkillSummary {
        name: name.into(),
        description: description.into(),
    }
}

fn compile_shared(path: &Path, source: &str) {
    let src = path.with_extension("c");
    std::fs::write(&src, source).unwrap();
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
fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "skill module publication timed out"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn skill_route_handles_empty_unicode_and_ambiguous_candidates() {
    let g = baseline();
    let cases = [
        (
            vec![skill(
                "systematic-debugging",
                "Debug broken code and errors",
            )],
            "🐦 systematic-debugging please",
            Some(0),
        ),
        (
            vec![skill(
                "test-driven-development",
                "test first then make green",
            )],
            "the tests all pass",
            None,
        ),
        (
            vec![skill(
                "test-driven-development",
                "test first then make green",
            )],
            "Use test-driven-development please",
            Some(0),
        ),
        (
            vec![
                skill("debug-tool", "bug crash"),
                skill("debug-tool", "bug crash"),
            ],
            "debug-tool bug crash",
            None,
        ),
        (
            vec![skill("unsafe name", "bug crash")],
            "unsafe name bug crash",
            None,
        ),
        (
            vec![skill("research-answer", "research question")],
            "\u{2002}answer the question",
            Some(0),
        ),
        (
            vec![skill("navigate-repo", "navigation and exploring")],
            "🐦",
            None,
        ),
    ];
    for (skills, prompt, expected) in cases {
        assert_eq!(do_route(&g, &skills, prompt), Some(expected), "{prompt:?}");
    }
    let skills = vec![skill("debug-tool", "bug crash")];
    for n in [4094, 4095, 4096, 4097] {
        assert_eq!(
            do_route(&g, &skills, &format!("{} debug-tool", "🐦".repeat(n))),
            Some(None)
        );
    }
}

#[test]
fn skill_router_reloads_and_pins_generations_and_rejects_bad_modules() {
    let temp = crate::tests::TestGitWorkspace::new("skill-route-native-reload");
    let path = temp.path().join("module.so");
    let next = temp.path().join("next.so");
    let source = include_str!("../../../cockpit/native/skill_router.c");
    compile_shared(&next, source);
    std::fs::rename(&next, &path).unwrap();
    let host = new_host(Some(path.clone()));
    let built_in = baseline();
    wait_until(|| !Arc::ptr_eq(&built_in, &host.current()));
    let first = host.current();
    let skills = vec![skill("systematic-debugging", "debug crashes and bugs")];
    assert_eq!(
        do_route(&first, &skills, "systematic-debugging"),
        do_route(&built_in, &skills, "systematic-debugging")
    );
    std::fs::write(&next, b"not a shared library").unwrap();
    std::fs::rename(&next, &path).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    assert!(Arc::ptr_eq(&first, &host.current()));
    compile_shared(
        &next,
        "#include <stdint.h>\nuint32_t angel_skill_route_abi_version(void) { return 1; }\n",
    );
    std::fs::rename(&next, &path).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    assert!(Arc::ptr_eq(&first, &host.current()));
    compile_shared(&next, source);
    std::fs::rename(&next, &path).unwrap();
    wait_until(|| !Arc::ptr_eq(&first, &host.current()));
    assert_eq!(
        do_route(&first, &skills, "systematic-debugging"),
        Some(Some(0))
    );
    assert_eq!(
        do_route(&host.current(), &skills, "systematic-debugging"),
        Some(Some(0))
    );
}

#[test]
fn skill_router_detects_invalid_results_and_falls_back() {
    unsafe extern "C" fn bad(
        _: Span,
        _: *const Skill,
        _: usize,
        _: CharTest,
        _: CharTest,
    ) -> usize {
        usize::MAX - 1
    }
    let bad = Generation {
        route: bad,
        _library: None,
    };
    let skills = vec![skill("systematic-debugging", "debug crashes and bugs")];
    let selected = do_route(&bad, &skills, "systematic-debugging")
        .or_else(|| do_route(&baseline(), &skills, "systematic-debugging"));
    assert_eq!(selected, Some(Some(0)));
}

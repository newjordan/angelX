//! Workspace language / test-runner detection shared by `run_tests` and the caddy.
//!
//! One shallow scan (root + one level of subdirectories, skipping vendored and
//! build directories) answers two questions the cockpit used to guess at
//! separately: which languages live in this workspace, and which command runs its
//! tests. Before this module `run_tests` was Cargo-only — on a JS workspace it
//! spent a whole model hop failing at `cargo test` before the model fell back to
//! `node --test` by hand (seen in the 2026-09-06 arena capture) — and the caddy
//! read only root markers, so angelX itself (Cargo.toml under `cockpit/`) was
//! reported as `js`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Rust,
    Js,
    Python,
    Go,
    Swift,
    Cpp,
}

impl Lang {
    pub fn name(self) -> &'static str {
        match self {
            Lang::Rust => "rust",
            Lang::Js => "js",
            Lang::Python => "python",
            Lang::Go => "go",
            Lang::Swift => "swift",
            Lang::Cpp => "cpp",
        }
    }
}

/// One detected language and the evidence that placed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangHit {
    pub lang: Lang,
    /// Where the marker sits relative to the workspace root (`.` for the root).
    pub dir: PathBuf,
    /// The marker file or test-file pattern that produced the hit.
    pub marker: String,
    /// True when the marker was a manifest (Cargo.toml, package.json, …) rather
    /// than loose test files.
    pub manifest: bool,
}

/// The command `run_tests` should run for a workspace, in argv form, plus how to
/// read its output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPlan {
    pub lang: Lang,
    /// Program (resolved on `PATH` by the caller) and arguments.
    pub program: &'static str,
    pub args: Vec<String>,
    /// Directory to run in, relative to the workspace root.
    pub dir: PathBuf,
    /// Human label for receipts (`node --test`, `pytest -q`, …).
    pub label: String,
    /// Why this plan was chosen (marker path), for the receipt.
    pub because: String,
    /// The package.json `test` script when the plan is `npm test` (so a caller
    /// with explicit file/name args can bypass a script that hardcodes globs).
    pub script: Option<String>,
}

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "vendor",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".home",
    ".angelX",
    "off-limits",
];

fn is_js_test_file(name: &str) -> bool {
    let stem_ok = |s: &str| {
        s == "test"
            || s.ends_with(".test")
            || s.ends_with("_test")
            || s.ends_with("-test")
            || s.starts_with("test-")
    };
    for ext in [".mjs", ".cjs", ".js", ".ts", ".mts"] {
        if let Some(stem) = name.strip_suffix(ext) {
            return stem_ok(stem);
        }
    }
    false
}

fn is_py_test_file(name: &str) -> bool {
    name.ends_with(".py")
        && (name.starts_with("test_") || name.ends_with("_test.py") || name == "test.py")
}

fn is_go_test_file(name: &str) -> bool {
    name.ends_with("_test.go")
}

fn scan_dir(root: &Path, rel: &Path, hits: &mut Vec<LangHit>) {
    let dir = root.join(rel);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut js_tests = 0usize;
    let mut py_tests = 0usize;
    let mut go_tests = 0usize;
    let push = |lang: Lang, marker: &str, manifest: bool, hits: &mut Vec<LangHit>| {
        hits.push(LangHit {
            lang,
            dir: rel.to_path_buf(),
            marker: marker.to_string(),
            manifest,
        });
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
        if !is_file {
            continue;
        }
        match name.as_ref() {
            "Cargo.toml" => push(Lang::Rust, "Cargo.toml", true, hits),
            "package.json" => push(Lang::Js, "package.json", true, hits),
            "pyproject.toml" => push(Lang::Python, "pyproject.toml", true, hits),
            "setup.py" => push(Lang::Python, "setup.py", true, hits),
            "setup.cfg" => push(Lang::Python, "setup.cfg", true, hits),
            "pytest.ini" | "tox.ini" | "conftest.py" => push(Lang::Python, &name, true, hits),
            "go.mod" => push(Lang::Go, "go.mod", true, hits),
            "Package.swift" => push(Lang::Swift, "Package.swift", true, hits),
            "CMakeLists.txt" => push(Lang::Cpp, "CMakeLists.txt", true, hits),
            _ => {
                if is_js_test_file(&name) {
                    js_tests += 1;
                } else if is_py_test_file(&name) {
                    py_tests += 1;
                } else if is_go_test_file(&name) {
                    go_tests += 1;
                }
            }
        }
    }
    if js_tests > 0 {
        push(
            Lang::Js,
            &format!("{js_tests} js test file(s)"),
            false,
            hits,
        );
    }
    if py_tests > 0 {
        push(
            Lang::Python,
            &format!("{py_tests} python test file(s)"),
            false,
            hits,
        );
    }
    if go_tests > 0 {
        push(
            Lang::Go,
            &format!("{go_tests} go test file(s)"),
            false,
            hits,
        );
    }
}

/// Shallow scan: the root, then each first-level directory (and `test`/`tests`
/// one level deeper, since suites often live there). Results are ordered
/// root-first, manifests before loose test files, so `hits[0]` is the best
/// single answer.
pub fn detect(workspace: &Path) -> Vec<LangHit> {
    let mut hits = Vec::new();
    scan_dir(workspace, Path::new("."), &mut hits);
    let mut subdirs: Vec<PathBuf> = std::fs::read_dir(workspace)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| PathBuf::from(e.file_name()))
                .filter(|p| {
                    let n = p.to_string_lossy();
                    !SKIP_DIRS.contains(&n.as_ref()) && !n.starts_with('.')
                })
                .collect()
        })
        .unwrap_or_default();
    subdirs.sort();
    for sub in &subdirs {
        let before = hits.len();
        scan_dir(workspace, sub, &mut hits);
        let is_test_dir = matches!(
            sub.to_string_lossy().as_ref(),
            "test" | "tests" | "__tests__" | "spec"
        );
        if is_test_dir && hits.len() == before {
            // e.g. tests/unit/*.test.mjs
            if let Ok(rd) = std::fs::read_dir(workspace.join(sub)) {
                let mut inner: Vec<PathBuf> = rd
                    .flatten()
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .map(|e| sub.join(e.file_name()))
                    .collect();
                inner.sort();
                for d in inner {
                    scan_dir(workspace, &d, &mut hits);
                }
            }
        }
    }
    // Root-first, manifests-first within a directory; stable otherwise. A
    // CMakeLists.txt beside another language's manifest (a native addon, a
    // bundled C library) never outranks that language's own runner.
    hits.sort_by_key(|h| (h.dir != Path::new("."), !h.manifest, h.lang == Lang::Cpp));
    hits
}

/// Distinct language names in detection order (what the caddy card prints).
pub fn lang_names(hits: &[LangHit]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for h in hits {
        let n = h.lang.name().to_string();
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

fn package_json_test_script(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let script = v.get("scripts")?.get("test")?.as_str()?.trim().to_string();
    if script.is_empty() || script.contains("no test specified") {
        return None;
    }
    Some(script)
}

fn has_pytest_config(dir: &Path) -> bool {
    if dir.join("pytest.ini").is_file() || dir.join("conftest.py").is_file() {
        return true;
    }
    for f in ["pyproject.toml", "setup.cfg", "tox.ini"] {
        if let Ok(t) = std::fs::read_to_string(dir.join(f))
            && (t.contains("[tool.pytest") || t.contains("[pytest]") || t.contains("[tool:pytest]"))
        {
            return true;
        }
    }
    false
}

/// Choose the test command for a workspace. `prefer` pins a language when the
/// workspace holds several (the caller passes it from the tool args); otherwise
/// the first detected hit wins. Rust is deliberately *not* planned here — the
/// Cargo path keeps its own pinned, trusted verifier — so a Rust-first workspace
/// returns `None` and the caller takes the Cargo route.
pub fn plan_tests(workspace: &Path, hits: &[LangHit], prefer: Option<Lang>) -> Option<TestPlan> {
    let hit = match prefer {
        Some(p) => hits.iter().find(|h| h.lang == p)?,
        None => hits.first()?,
    };
    let dir = hit.dir.clone();
    let abs = workspace.join(&dir);
    let because = if dir == Path::new(".") {
        hit.marker.clone()
    } else {
        format!("{}/{}", dir.display(), hit.marker)
    };
    match hit.lang {
        Lang::Rust => None,
        Lang::Js => {
            // package.json with a real test script wins; otherwise node's
            // built-in runner discovers *.test.{js,mjs,cjs}, test/**, test.js.
            if let Some(script) = package_json_test_script(&abs.join("package.json")) {
                return Some(TestPlan {
                    lang: Lang::Js,
                    program: "npm",
                    args: vec!["test".into(), "--silent".into()],
                    dir,
                    label: format!("npm test ({script})"),
                    because,
                    script: Some(script),
                });
            }
            Some(TestPlan {
                lang: Lang::Js,
                program: "node",
                args: vec!["--test".into()],
                dir,
                label: "node --test".into(),
                because,
                script: None,
            })
        }
        Lang::Python => {
            if has_pytest_config(&abs) {
                Some(TestPlan {
                    lang: Lang::Python,
                    program: "python3",
                    args: vec!["-m".into(), "pytest".into(), "-q".into()],
                    dir,
                    label: "pytest -q".into(),
                    because,
                    script: None,
                })
            } else {
                Some(TestPlan {
                    lang: Lang::Python,
                    program: "python3",
                    // `-p *test*.py` matches both `test_foo.py` and the
                    // exercism-style `foo_test.py` suffix; the unittest default
                    // `test*.py` silently finds zero suffix-named suites.
                    args: vec![
                        "-m".into(),
                        "unittest".into(),
                        "discover".into(),
                        "-v".into(),
                        "-p".into(),
                        "*test*.py".into(),
                    ],
                    dir,
                    label: "python3 -m unittest discover -v".into(),
                    because,
                    script: None,
                })
            }
        }
        Lang::Go => Some(TestPlan {
            lang: Lang::Go,
            program: "go",
            // -v so the receipt counts test functions (--- PASS/FAIL lines), not
            // packages: a 3-test package read "1 passed" in the 2026-09-07 dogfood.
            args: vec!["test".into(), "-v".into(), "./...".into()],
            dir,
            label: "go test -v ./...".into(),
            because,
            script: None,
        }),
        Lang::Swift => Some(TestPlan {
            lang: Lang::Swift,
            program: "swift",
            args: vec!["test".into()],
            dir,
            label: "swift test".into(),
            because,
            script: None,
        }),
        // Configure, build, then run whatever the project registered with CTest.
        // A build that runs its own tests (Exercism's C++ track wires the test
        // binary into the default target) fails at the build step instead;
        // `--no-tests=ignore` keeps such a project green when CTest has nothing.
        Lang::Cpp => Some(TestPlan {
            lang: Lang::Cpp,
            program: "sh",
            args: vec!["-c".into(), CMAKE_TEST_SCRIPT.into()],
            dir,
            label: "cmake -S . -B build && cmake --build build && ctest".into(),
            because,
            script: Some(CMAKE_TEST_SCRIPT.into()),
        }),
    }
}

const CMAKE_TEST_SCRIPT: &str = "cmake -S . -B build && cmake --build build && \
     ctest --test-dir build --output-on-failure --no-tests=ignore";

/// Resolve a program on `PATH` (or verify an explicit path). Shared by
/// `run_tests`' non-Cargo runners and the shell tool's exit-127 hint.
pub(crate) fn resolve_on_path(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        let p = PathBuf::from(program);
        return p.is_file().then_some(p);
    }
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|d| d.join(program))
            .find(|p| p.is_file())
    })
}

const RUNTIME_PROBES: &[&str] = &[
    "python3", "python", "pytest", "uv", "node", "npm", "bun", "cargo", "go", "swift",
];

/// Runtime shims appended to the *end* of child PATH: real programs always win.
/// Reconcile against current HOME/PATH on each use with filesystem probes only;
/// permanent positive/negative caching hides removed homes and installed runtimes.
/// Off with `ANGEL_RUNTIME_SHIMS=0`.
pub(crate) fn runtime_shims_dir() -> Option<PathBuf> {
    if !crate::agent::harness::env_flag("ANGEL_RUNTIME_SHIMS", true) {
        return None;
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    runtime_shims_in(&home, &std::env::var_os("PATH")?)
}

fn runtime_shims_in(home: &Path, search: &std::ffi::OsStr) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let paths: Vec<_> = std::env::split_paths(search).collect();
    let resolve = |name| paths.iter().map(|dir| dir.join(name)).find(|p| p.is_file());
    let links: Vec<_> = [("python", "python3"), ("pip", "pip3")]
        .into_iter()
        .filter_map(|(missing, real)| {
            if resolve(missing).is_some() {
                return None;
            }
            // Absolute targets remain valid when the child changes directory.
            resolve(real)?
                .canonicalize()
                .ok()
                .map(|target| (missing, target))
        })
        .collect();
    if links.is_empty() {
        return None;
    }
    // A new runtime selection gets a new directory, never replacement/deletion
    // of a link or ordinary file owned by the operator or another live process.
    let mut identity = std::collections::hash_map::DefaultHasher::new();
    links.hash(&mut identity);
    let dir = home
        .join(".angelX/shims")
        .join(format!("{:016x}", identity.finish()));
    if !dir.is_dir() {
        std::fs::create_dir_all(&dir).ok()?;
    }
    let mut made = false;
    for (name, target) in links {
        let link = dir.join(name);
        if std::fs::read_link(&link).is_ok_and(|existing| existing == target) {
            made = true;
            continue;
        }
        #[cfg(unix)]
        if std::os::unix::fs::symlink(&target, &link).is_ok()
            || std::fs::read_link(&link).is_ok_and(|existing| existing == target)
        {
            made = true;
        }
    }
    made.then_some(dir)
}

/// Which language runtimes this host actually has on `PATH`, rendered once per
/// process for the pre-turn card: e.g. `python3 (no python) · node · npm · cargo
/// · go`. Existence checks only — nothing is executed. Measured 2026-09-07 in the
/// arena: every python task lost its first shell hop to `python -m unittest`
/// (exit 127) on a host that only has `python3`; the model had no way to know.
pub(crate) fn host_runtimes() -> &'static str {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE.get_or_init(|| {
        let present: Vec<&str> = RUNTIME_PROBES
            .iter()
            .copied()
            .filter(|p| resolve_on_path(p).is_some())
            .collect();
        let shims = runtime_shims_dir();
        let shimmed = |name: &str| {
            shims
                .as_ref()
                .map(|d| d.join(name).exists())
                .unwrap_or(false)
        };
        let mut parts: Vec<String> = Vec::new();
        for p in &present {
            if *p == "python3" && !present.contains(&"python") {
                parts.push(if shimmed("python") {
                    "python3 (`python` → python3 shim)".to_string()
                } else {
                    "python3 (no `python`)".to_string()
                });
            } else if *p == "python" && !present.contains(&"python3") {
                parts.push("python (no `python3`)".to_string());
            } else {
                parts.push((*p).to_string());
            }
        }
        parts.join(" · ")
    })
}

/// For a shell command that exited 127 ("command not found"): name the program
/// that is missing and the sibling that exists, so the next hop is the fix and
/// not a guess. Looks at the first word of each `&&`/`;`/`|`-separated segment,
/// skipping `cd`, `env`, `VAR=value` prefixes and shell builtins.
pub(crate) fn missing_program_hint(command: &str) -> Option<String> {
    const BUILTINS: &[&str] = &[
        "cd", "env", "export", "set", "echo", "test", "[", "true", "false", "exit", "source", ".",
        "if", "then", "else", "fi", "for", "while", "do", "done", "time", "exec", "eval", "printf",
        "read", "pwd", "shift", "trap", "wait", "unset", "alias", "command", "type", "hash",
        "ulimit", "umask",
    ];
    const SIBLINGS: &[(&str, &str)] = &[
        ("python", "python3"),
        ("python2", "python3"),
        ("pip", "pip3"),
        ("pip", "python3 -m pip"),
        ("pytest", "python3 -m pytest"),
        ("nodejs", "node"),
        ("node", "nodejs"),
        ("yarn", "npm"),
        ("pnpm", "npm"),
        ("bun", "node"),
        ("gcc", "cc"),
        ("clang", "cc"),
        ("make", "cmake"),
    ];
    for segment in command.split([';', '|', '\n']).flat_map(|s| s.split("&&")) {
        let mut words = segment.split_whitespace().peekable();
        while let Some(w) = words.peek() {
            if w.contains('=') && !w.starts_with('=')
                || *w == "env"
                || *w == "sudo"
                || *w == "nice"
                || *w == "time"
            {
                words.next();
            } else {
                break;
            }
        }
        let Some(program) = words.next() else {
            continue;
        };
        let program = program.trim_matches(|c| c == '(' || c == '"' || c == '\'' || c == '`');
        if program.is_empty() || BUILTINS.contains(&program) || program.starts_with('$') {
            continue;
        }
        if resolve_on_path(program).is_some() {
            continue;
        }
        let base = program.rsplit('/').next().unwrap_or(program);
        let mut hint = format!("`{program}` is not on PATH");
        let alternatives: Vec<String> = SIBLINGS
            .iter()
            .filter(|(missing, _)| *missing == base)
            .map(|(_, alt)| alt.to_string())
            .filter(|alt| resolve_on_path(alt.split_whitespace().next().unwrap_or(alt)).is_some())
            .collect();
        if !alternatives.is_empty() {
            hint.push_str(&format!(" — use `{}` instead", alternatives.join("` or `")));
        }
        hint.push_str(&format!(" (host runtimes: {})", host_runtimes()));
        return Some(hint);
    }
    None
}

/// Parse `lang` from a `run_tests` `runner`/`lang` argument.
pub fn parse_lang(s: &str) -> Option<Lang> {
    match s.trim().to_ascii_lowercase().as_str() {
        "rust" | "cargo" => Some(Lang::Rust),
        "js" | "javascript" | "node" | "npm" | "ts" | "typescript" => Some(Lang::Js),
        "python" | "py" | "pytest" | "unittest" => Some(Lang::Python),
        "go" | "golang" => Some(Lang::Go),
        "swift" => Some(Lang::Swift),
        "cpp" | "c++" | "cxx" | "c" | "cmake" | "ctest" => Some(Lang::Cpp),
        _ => None,
    }
}

/// Counts parsed from a non-Cargo test runner's output. Recognises node's TAP
/// and spec reporters, jest, vitest, mocha, pytest, unittest and `go test`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunnerCounts {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

fn num_before(tokens: &[&str], idx: usize) -> Option<usize> {
    idx.checked_sub(1).and_then(|i| tokens[i].parse().ok())
}
fn num_after(tokens: &[&str], idx: usize) -> Option<usize> {
    tokens.get(idx + 1).and_then(|t| t.parse().ok())
}

pub fn parse_runner_output(lang: Lang, stdout: &str, stderr: &str) -> RunnerCounts {
    let mut c = RunnerCounts::default();
    let text = format!("{stdout}\n{stderr}");
    match lang {
        Lang::Js => {
            let mut saw_summary = false;
            for line in text.lines() {
                let t = line.trim();
                // node TAP: "# pass 3" / "# fail 1" / "# skipped 0"; spec: "ℹ pass 3"
                let stripped = t.trim_start_matches('#').trim_start_matches('ℹ').trim();
                let toks: Vec<&str> = stripped.split_whitespace().collect();
                if toks.len() == 2
                    && let Ok(n) = toks[1].parse::<usize>()
                {
                    match toks[0] {
                        "pass" => {
                            c.passed = n;
                            saw_summary = true;
                            continue;
                        }
                        "fail" => {
                            c.failed = n;
                            saw_summary = true;
                            continue;
                        }
                        "skipped" | "todo" => {
                            c.skipped += n;
                            continue;
                        }
                        _ => {}
                    }
                }
                // jest: "Tests:       1 failed, 3 passed, 4 total"
                if t.starts_with("Tests:") {
                    let toks: Vec<&str> = t
                        .split(|ch: char| ch == ',' || ch.is_whitespace())
                        .filter(|s| !s.is_empty())
                        .collect();
                    for (i, w) in toks.iter().enumerate() {
                        match *w {
                            "passed" => c.passed = num_before(&toks, i).unwrap_or(0),
                            "failed" => c.failed = num_before(&toks, i).unwrap_or(0),
                            "skipped" | "todo" => c.skipped += num_before(&toks, i).unwrap_or(0),
                            _ => {}
                        }
                    }
                    saw_summary = true;
                    continue;
                }
                // vitest: "Tests  3 passed | 1 failed (4)"
                if t.starts_with("Tests ") && (t.contains(" passed") || t.contains(" failed")) {
                    let toks: Vec<&str> = t
                        .split(|ch: char| ch == '|' || ch == '(' || ch == ')' || ch.is_whitespace())
                        .filter(|s| !s.is_empty())
                        .collect();
                    for (i, w) in toks.iter().enumerate() {
                        match *w {
                            "passed" => c.passed = num_before(&toks, i).unwrap_or(0),
                            "failed" => c.failed = num_before(&toks, i).unwrap_or(0),
                            "skipped" => c.skipped += num_before(&toks, i).unwrap_or(0),
                            _ => {}
                        }
                    }
                    saw_summary = true;
                    continue;
                }
                // mocha: "3 passing (12ms)" / "1 failing" / "2 pending"
                let toks: Vec<&str> = t.split_whitespace().collect();
                if toks.len() >= 2
                    && let Ok(n) = toks[0].parse::<usize>()
                {
                    match toks[1] {
                        "passing" => {
                            c.passed = n;
                            saw_summary = true;
                        }
                        "failing" => {
                            c.failed = n;
                            saw_summary = true;
                        }
                        "pending" => c.skipped += n,
                        _ => {}
                    }
                }
            }
            if !saw_summary {
                // TAP without a summary (single file, `ok 1 - x` / `not ok 2 - y`)
                for line in text.lines() {
                    let t = line.trim();
                    if t.starts_with("not ok ") {
                        c.failed += 1;
                    } else if t.starts_with("ok ") {
                        c.passed += 1;
                    }
                }
            }
        }
        Lang::Python => {
            for line in text.lines() {
                let t = line.trim().trim_matches('=').trim();
                // pytest: "3 passed, 1 failed, 2 skipped in 0.12s" (also "error(s)")
                if t.contains(" in ")
                    && (t.contains("passed") || t.contains("failed") || t.contains("error"))
                {
                    let toks: Vec<&str> = t
                        .split(|ch: char| ch == ',' || ch.is_whitespace())
                        .filter(|s| !s.is_empty())
                        .collect();
                    let mut hit = false;
                    for (i, w) in toks.iter().enumerate() {
                        match *w {
                            "passed" => {
                                c.passed = num_before(&toks, i).unwrap_or(0);
                                hit = true;
                            }
                            "failed" => {
                                c.failed = num_before(&toks, i).unwrap_or(0);
                                hit = true;
                            }
                            "error" | "errors" => {
                                c.failed += num_before(&toks, i).unwrap_or(0);
                                hit = true;
                            }
                            "skipped" | "xfailed" | "deselected" => {
                                c.skipped += num_before(&toks, i).unwrap_or(0)
                            }
                            _ => {}
                        }
                    }
                    if hit {
                        continue;
                    }
                }
                // unittest: "Ran 4 tests in 0.001s" then "OK" / "OK (skipped=1)" / "FAILED (failures=1, errors=1)"
                if t.starts_with("Ran ") {
                    let toks: Vec<&str> = t.split_whitespace().collect();
                    if let Some(n) = num_after(&toks, 0) {
                        c.passed = n;
                    }
                    continue;
                }
                if t.starts_with("FAILED") || t.starts_with("OK") {
                    let mut bad = 0usize;
                    let mut skipped = 0usize;
                    for part in t
                        .trim_start_matches("FAILED")
                        .trim_start_matches("OK")
                        .trim()
                        .trim_matches(|ch| ch == '(' || ch == ')')
                        .split(',')
                    {
                        let mut kv = part.trim().splitn(2, '=');
                        let k = kv.next().unwrap_or("").trim();
                        let v: usize = kv.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                        match k {
                            "failures" | "errors" | "unexpected successes" => bad += v,
                            "skipped" | "expected failures" => skipped += v,
                            _ => {}
                        }
                    }
                    if c.passed >= bad + skipped {
                        c.passed -= bad + skipped;
                    }
                    c.failed += bad;
                    c.skipped += skipped;
                }
            }
        }
        Lang::Go => {
            for line in text.lines() {
                let t = line.trim();
                if t.starts_with("--- PASS") {
                    c.passed += 1;
                } else if t.starts_with("--- FAIL") {
                    c.failed += 1;
                } else if t.starts_with("--- SKIP") {
                    c.skipped += 1;
                }
            }
            if c.passed == 0 && c.failed == 0 {
                // non-verbose: "ok  pkg 0.01s" / "FAIL pkg 0.01s" per package
                for line in text.lines() {
                    let t = line.trim();
                    if t.starts_with("ok ") || t.starts_with("ok\t") {
                        c.passed += 1;
                    } else if t.starts_with("FAIL") && t.split_whitespace().count() >= 2 {
                        c.failed += 1;
                    }
                }
            }
        }
        Lang::Cpp => {
            // CTest's own summary counts registered tests and wins when present:
            // "67% tests passed, 1 tests failed out of 3". Otherwise the test
            // binary's summary: Catch2 "All tests passed (N assertions in M test
            // cases)" / "test cases: 5 | 4 passed | 1 failed", GoogleTest
            // "[  PASSED  ] 3 tests." / "[  FAILED  ] 1 test, listed below:".
            for line in text.lines() {
                let t = line.trim();
                if let Some(rest) = t.split_once("tests passed, ").map(|(_, rest)| rest)
                    && t.contains("% tests passed")
                {
                    let toks: Vec<&str> = rest.split_whitespace().collect();
                    let failed = toks.first().and_then(|n| n.parse().ok()).unwrap_or(0);
                    let total = toks.last().and_then(|n| n.parse().ok()).unwrap_or(0);
                    return RunnerCounts {
                        passed: usize::saturating_sub(total, failed),
                        failed,
                        skipped: 0,
                    };
                }
            }
            for line in text.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("All tests passed (") {
                    let toks: Vec<&str> = rest.split_whitespace().collect();
                    if let Some(i) = toks.iter().position(|w| *w == "test") {
                        c.passed += num_before(&toks, i).unwrap_or(0);
                    }
                } else if let Some(rest) = t.strip_prefix("test cases:") {
                    let fields: Vec<&str> = rest.split('|').map(str::trim).collect();
                    for field in fields.iter().skip(1) {
                        let toks: Vec<&str> = field.split_whitespace().collect();
                        match toks.get(1) {
                            Some(&"passed") => c.passed += num_before(&toks, 1).unwrap_or(0),
                            Some(&"failed") => c.failed += num_before(&toks, 1).unwrap_or(0),
                            _ => {}
                        }
                    }
                } else if let Some(rest) = t.strip_prefix("[  PASSED  ]") {
                    c.passed += rest
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(0);
                } else if let Some(rest) = t.strip_prefix("[  FAILED  ]")
                    && t.ends_with("listed below:")
                {
                    // The count line; the other FAILED lines name single tests.
                    c.failed += rest
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(0);
                }
            }
        }
        Lang::Rust | Lang::Swift => {
            // libtest-shaped summary lines ("test result: ok. 3 passed; 0 failed;")
            // and swift-pm's "Executed 3 tests, with 0 failures".
            for line in text.lines() {
                let t = line.trim();
                if t.starts_with("Executed ") {
                    let toks: Vec<&str> = t
                        .split(|ch: char| ch == ',' || ch.is_whitespace())
                        .filter(|s| !s.is_empty())
                        .collect();
                    let total = num_after(&toks, 0).unwrap_or(0);
                    let failures = toks
                        .iter()
                        .position(|w| w.starts_with("failure"))
                        .and_then(|i| num_before(&toks, i))
                        .unwrap_or(0);
                    c.failed = failures;
                    c.passed = total.saturating_sub(failures);
                }
            }
        }
    }
    c
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/workspace_lang__tests.rs"]
mod tests;

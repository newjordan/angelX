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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangHit {
    pub lang: Lang,
    pub dir: PathBuf,
    pub marker: String,
    pub manifest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPlan {
    pub lang: Lang,
    pub program: &'static str,
    pub args: Vec<String>,
    pub dir: PathBuf,
    pub label: String,
    pub because: String,
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
    hits.sort_by_key(|h| (h.dir != Path::new("."), !h.manifest, h.lang == Lang::Cpp));
    hits
}

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

pub fn plan_tests(workspace: &Path, hits: &[LangHit], prefer: Option<Lang>) -> Option<TestPlan> {
    plan_tests_for(workspace, hits, prefer, None)
}

pub fn plan_tests_for(
    workspace: &Path,
    hits: &[LangHit],
    prefer: Option<Lang>,
    entrypoint: Option<&str>,
) -> Option<TestPlan> {
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
            if entrypoint != Some("node")
                && let Some(script) = package_json_test_script(&abs.join("package.json"))
            {
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
            if entrypoint == Some("pytest") {
                Some(TestPlan {
                    lang: Lang::Python,
                    program: "python3",
                    args: vec!["-m".into(), "pytest".into()],
                    dir,
                    label: "python3 -m pytest".into(),
                    because,
                    script: None,
                })
            } else if entrypoint != Some("unittest") && has_pytest_config(&abs) {
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
            resolve(real)?
                .canonicalize()
                .ok()
                .map(|target| (missing, target))
        })
        .collect();
    if links.is_empty() {
        return None;
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnerCommand {
    pub lang: Lang,
    pub entrypoint: Option<&'static str>,
    pub words: usize,
}

pub fn parse_runner_command(words: &[&str]) -> Option<RunnerCommand> {
    let word = |i: usize| {
        words
            .get(i)
            .map(|w| w.rsplit('/').next().unwrap_or(w).to_ascii_lowercase())
    };
    let (first, second, third) = (word(0)?, word(1), word(2));
    let command = |lang, entrypoint, words| {
        Some(RunnerCommand {
            lang,
            entrypoint,
            words,
        })
    };
    match (first.as_str(), second.as_deref(), third.as_deref()) {
        ("cargo", Some("test"), _) => command(Lang::Rust, None, 2),
        ("npm" | "pnpm" | "yarn" | "bun", Some("run"), Some("test")) => command(Lang::Js, None, 3),
        ("npm" | "pnpm" | "yarn" | "bun", Some("test" | "t"), _) => command(Lang::Js, None, 2),
        ("npx" | "bunx", Some("jest" | "vitest" | "mocha"), _) => command(Lang::Js, None, 2),
        ("jest" | "vitest" | "mocha", _, _) => command(Lang::Js, None, 1),
        ("node", Some("--test"), _) => command(Lang::Js, Some("node"), 2),
        ("python" | "python3", Some("-m"), Some("pytest")) => {
            command(Lang::Python, Some("pytest"), 3)
        }
        ("python" | "python3", Some("-m"), Some("unittest")) => {
            command(Lang::Python, Some("unittest"), 3)
        }
        ("pytest", _, _) => command(Lang::Python, Some("pytest"), 1),
        ("go", Some("test"), _) => command(Lang::Go, None, 2),
        ("swift", Some("test"), _) => command(Lang::Swift, None, 2),
        ("ctest", _, _) => command(Lang::Cpp, None, 1),
        _ => None,
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunnerCounts {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

fn inline_runner_generation_compatible(wrapper: &str, module: &str) -> bool {
    crate::knowledge::cut::sha256_hex(wrapper.as_bytes())
        == "e6b79c0d45276480fc1ffbada85391b11e68be4ad9464a64f6b3f60af7b3fd65"
        && crate::knowledge::cut::sha256_hex(module.as_bytes())
            == "6ff2ccf127ca74b79c84d7a40c480483f7e671831c2db9b7e2eb9bb5ff152abb"
}

pub fn parse_runner_output(
    lang: Lang,
    stdout: &str,
    stderr: &str,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<RunnerCounts, String> {
    parse_runner_output_with(
        lang,
        stdout,
        stderr,
        cancel,
        crate::agent::club::transform::evaluate,
    )
}

fn parse_runner_output_with(
    lang: Lang,
    stdout: &str,
    stderr: &str,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    evaluate: impl FnOnce(
        &str,
        &serde_json::Value,
        Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<serde_json::Value, String>,
) -> Result<RunnerCounts, String> {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    const MAX_TRANSCRIPT_BYTES: usize = 8 * 1024 * 1024;
    const MAX_REQUEST_BYTES: usize = 48 * 1024 * 1024;
    if stdout.len().saturating_add(stderr.len()) > MAX_TRANSCRIPT_BYTES {
        return Err("test runner transcript exceeds its byte limit".into());
    }
    let stopped = || cancel.is_some_and(|flag| flag.load(Ordering::Acquire));
    if stopped() {
        return Err("test summary interrupted".into());
    }
    let request = serde_json::json!({
        "version": 1,
        "lang": lang.name(),
        "stdout": stdout,
        "stderr": stderr,
        "max": usize::MAX.to_string(),
    });
    let input =
        serde_json::to_vec(&request).map_err(|error| format!("encode test summary: {error}"))?;
    const WRAPPER: &str = include_str!("../../../scripts/runtime/test-runner-report.mjs");
    const MODULE: &str = include_str!("../../../lib/test-runner/report.mjs");
    const MAX_INLINE_INPUT: usize = 1024 * 1024;
    static INLINE_COMPATIBLE: OnceLock<bool> = OnceLock::new();
    let node_pin = std::env::var_os("ANGEL_NODE_BIN").is_some();
    let pinned_node = if node_pin {
        Some(
            crate::agent::tools::runtime_missing::resolve("node").map_err(|_| {
                "Node.js unavailable (install Node or set ANGEL_NODE_BIN)".to_string()
            })?,
        )
    } else {
        None
    };
    let snapshot = crate::platform::worker_snapshot::Snapshot::capture(
        &crate::platform::runtime_paths::root(),
        &[
            "scripts/runtime/test-runner-report.mjs",
            "lib/test-runner/report.mjs",
        ],
        "test-runner",
    )?;
    let inline = if *INLINE_COMPATIBLE
        .get_or_init(|| inline_runner_generation_compatible(WRAPPER, MODULE))
        && !node_pin
        && input.len() <= MAX_INLINE_INPUT
    {
        let wrapper = crate::platform::worker_snapshot::read(
            &snapshot.dir.join("scripts/runtime/test-runner-report.mjs"),
            "test-runner",
        )?;
        let module = crate::platform::worker_snapshot::read(
            &snapshot.dir.join("lib/test-runner/report.mjs"),
            "test-runner",
        )?;
        wrapper == WRAPPER.as_bytes() && module == MODULE.as_bytes()
    } else {
        false
    };
    let projected = if inline {
        let source = MODULE.replacen("export function project(", "function runnerReport(", 1);
        let source = format!(
            "{source}\nfunction project(input) {{ return runnerReport({{...input, version: 1}}); }}"
        );
        match evaluate(&source, &request, cancel) {
            Ok(value) => Some(value),
            Err(error) if stopped() || error == "projection interrupted" => {
                return Err("test summary interrupted".into());
            }
            Err(error)
                if matches!(
                    error.as_str(),
                    "projection capacity timed out" | "projection timed out"
                ) =>
            {
                None
            }
            Err(error) => return Err(format!("test summary unavailable: {error}")),
        }
    } else {
        None
    };
    let value: serde_json::Value = if let Some(value) = projected {
        value
    } else {
        if stopped() {
            return Err("test summary interrupted".into());
        }
        let node = match pinned_node {
            Some(node) => node,
            None => crate::agent::tools::runtime_missing::resolve("node").map_err(|_| {
                "Node.js unavailable (install Node or set ANGEL_NODE_BIN)".to_string()
            })?,
        };
        let mut command = std::process::Command::new(node);
        command
            .arg(snapshot.dir.join("scripts/runtime/test-runner-report.mjs"))
            .env_clear();
        let output = crate::platform::worker::run_cancellable(
            command,
            &input,
            Duration::from_secs(10),
            MAX_REQUEST_BYTES,
            4096,
            stopped,
        )?;
        serde_json::from_slice(&output).map_err(|_| "invalid test summary response")?
    };
    if value.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err("unsupported test summary response".into());
    }
    let count = |name: &str| -> Result<usize, String> {
        let value = value
            .get(name)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("invalid test summary {name}"))?;
        if value.is_empty() || !value.bytes().all(|ch| ch.is_ascii_digit()) {
            return Err(format!("invalid test summary {name}"));
        }
        value
            .parse()
            .map_err(|_| format!("invalid test summary {name}"))
    };
    let counts = RunnerCounts {
        passed: count("passed")?,
        failed: count("failed")?,
        skipped: count("skipped")?,
    };
    counts
        .passed
        .checked_add(counts.failed)
        .ok_or("test summary count overflow")?;
    Ok(counts)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/workspace_lang__tests.rs"]
mod tests;

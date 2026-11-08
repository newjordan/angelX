use super::{Kind, Runtime};
use crate::agent::tools::build::{parse_direct_argv, path_within};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

const MAX_MANIFEST_BYTES: u64 = 128 * 1024;
const MAX_FILES: usize = 256;
const MAX_ENTRIES: usize = 10_000;

/// The live leg that owns the verifier's planning decision. Every filesystem and
/// lexical job stays in Rust: manifest reads, confinement, globbing, source
/// discovery, argv text and the byte limits become the facts below, and the leg
/// answers which step runs and which request is refused.
const VERIFY_PLAN: &str = "verify-plan";

/// Step codes shared with `cockpit/live/verify-plan`; `step_contract` keeps a
/// live answer inside this table.
const ROUTE_RUST: u32 = 0;
const ROUTE_GO: u32 = 1;
const ROUTE_VERIFY: u32 = 2;
const ROUTE_REJECT: u32 = 3;

const RUNTIME_NONE: u32 = 0;
const RUNTIME_NODE: u32 = 1;
const RUNTIME_PYTHON: u32 = 2;
const RUNTIME_GO: u32 = 3;

const MODE_NONE: u32 = 0;
const MODE_CHECK: u32 = 1;
const MODE_TESTS: u32 = 2;

const DRIVER_NONE: u32 = 0;
const DRIVER_NODE_TESTS: u32 = 1;
const DRIVER_PYTEST: u32 = 2;
const DRIVER_UNITTEST: u32 = 3;

const REJECT_NONE: u32 = 0;
const REJECT_AMBIGUOUS_LANGUAGE: u32 = 1;
const REJECT_LINT_UNSUPPORTED: u32 = 2;
const REJECT_GO_NEEDS_TESTS: u32 = 3;

const RUNTIME_REQUEST: &str = "runtime must be auto, rust, node, or python";
const AMBIGUOUS_LANGUAGE: &str = "root language is absent or ambiguous; set runtime explicitly to rust, node, or python (subprojects need their own workspace)";
const LINT_UNSUPPORTED: &str = "no trusted {} linter adapter is installed; syntax checking is available through check, while custom linters can run through shell without trusted verification credit";
const GO_NEEDS_TESTS: &str = "Go adapter supports tests only";

/// One planning decision's inputs: the request the tool call made, and the
/// workspace facts the native planner would have read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub(super) struct PlanFacts {
    pub(super) requested: &'static str,
    pub(super) entrypoint: &'static str,
    pub(super) kind: &'static str,
    pub(super) has_cargo_toml: bool,
    pub(super) has_package_json: bool,
    pub(super) has_pyproject_toml: bool,
    pub(super) has_python_setup: bool,
    pub(super) has_pytest_config: bool,
}

/// The `verify-plan` leg's answer: which route the plan takes, and the step it
/// runs when there is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(super) struct Step {
    pub(super) route: u32,
    pub(super) runtime: u32,
    pub(super) mode: u32,
    pub(super) driver: u32,
    pub(super) reject: u32,
}

impl Step {
    fn run(route: u32, runtime: u32, mode: u32, driver: u32) -> Self {
        Self {
            route,
            runtime,
            mode,
            driver,
            reject: REJECT_NONE,
        }
    }

    fn refuse(reject: u32, runtime: u32) -> Self {
        Self {
            route: ROUTE_REJECT,
            runtime,
            mode: MODE_NONE,
            driver: DRIVER_NONE,
            reject,
        }
    }
}

/// The language a plan may run in, or the fact that the workspace settles none.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Language {
    Rust,
    Go,
    Node,
    Python,
    Ambiguous,
}

fn requested_code(args: &Value) -> Result<&'static str, String> {
    let Some(value) = args.get("runtime") else {
        return Ok("auto");
    };
    match value.as_str() {
        Some("auto") => Ok("auto"),
        Some("rust") => Ok("rust"),
        Some("go") => Ok("go"),
        Some("node") => Ok("node"),
        Some("python") => Ok("python"),
        _ => Err(RUNTIME_REQUEST.into()),
    }
}

fn entrypoint_code(args: &Value) -> &'static str {
    match args.get("entrypoint") {
        None => "none",
        Some(Value::String(value)) => match value.as_str() {
            "node" => "node",
            "unittest" => "unittest",
            "pytest" => "pytest",
            _ => "other",
        },
        Some(_) => "other",
    }
}

fn kind_code(kind: Kind) -> &'static str {
    match kind {
        Kind::Tests => "tests",
        Kind::Check => "check",
        Kind::Lint => "lint",
    }
}

/// A workspace with exactly one recognizable language plans for it; none at all,
/// or several, is refused rather than guessed.
fn detected(facts: &PlanFacts) -> Language {
    let python = facts.has_pyproject_toml || facts.has_python_setup;
    match (facts.has_cargo_toml, facts.has_package_json, python) {
        (true, false, false) => Language::Rust,
        (false, true, false) => Language::Node,
        (false, false, true) => Language::Python,
        _ => Language::Ambiguous,
    }
}

fn auto_language(facts: &PlanFacts) -> Language {
    match facts.entrypoint {
        "node" => Language::Node,
        "unittest" | "pytest" => Language::Python,
        _ => detected(facts),
    }
}

/// pytest is selected by an explicit request, or by a configured pytest when the
/// entrypoint is not an explicit unittest one.
fn python_driver(facts: &PlanFacts) -> u32 {
    if facts.entrypoint == "pytest"
        || (facts.entrypoint != "unittest" && facts.has_pytest_config)
    {
        DRIVER_PYTEST
    } else {
        DRIVER_UNITTEST
    }
}

/// Native oracle: the planning decision the live leg must reproduce. The
/// language is settled first, then the kind; an explicit `rust` request never
/// reaches a verifier plan.
#[cfg(test)]
fn step_native(facts: &PlanFacts) -> Step {
    let language = match facts.requested {
        "rust" => Language::Rust,
        "go" => Language::Go,
        "node" => Language::Node,
        "python" => Language::Python,
        _ => auto_language(facts),
    };
    match language {
        Language::Rust => Step::run(ROUTE_RUST, RUNTIME_NONE, MODE_NONE, DRIVER_NONE),
        Language::Ambiguous => Step::refuse(REJECT_AMBIGUOUS_LANGUAGE, RUNTIME_NONE),
        Language::Go => match facts.kind {
            "tests" => Step::run(ROUTE_GO, RUNTIME_GO, MODE_TESTS, DRIVER_NONE),
            _ => Step::refuse(REJECT_GO_NEEDS_TESTS, RUNTIME_NONE),
        },
        Language::Node | Language::Python => {
            let runtime = if language == Language::Node {
                RUNTIME_NODE
            } else {
                RUNTIME_PYTHON
            };
            match facts.kind {
                "lint" => Step::refuse(REJECT_LINT_UNSUPPORTED, runtime),
                "check" => Step::run(ROUTE_VERIFY, runtime, MODE_CHECK, DRIVER_NONE),
                _ => Step::run(
                    ROUTE_VERIFY,
                    runtime,
                    MODE_TESTS,
                    if runtime == RUNTIME_NODE {
                        DRIVER_NODE_TESTS
                    } else {
                        python_driver(facts)
                    },
                ),
            }
        }
    }
}

/// Reject a live step the facts cannot justify, so a bad edit keeps the native
/// plan instead of steering a run. Each clause states an invariant the native
/// planner already holds, without restating how it decides.
fn step_contract(facts: &PlanFacts, step: &Step) -> Result<(), String> {
    if step.route > ROUTE_REJECT
        || step.runtime > RUNTIME_GO
        || step.mode > MODE_TESTS
        || step.driver > DRIVER_UNITTEST
        || step.reject > REJECT_GO_NEEDS_TESTS
    {
        return Err(format!(
            "verify plan returned a step outside its code table: {step:?}"
        ));
    }
    if (step.route == ROUTE_REJECT) != (step.reject != REJECT_NONE) {
        return Err("verify plan did not tie its refusal to a reject code".into());
    }
    match step.route {
        ROUTE_RUST => {
            if step.runtime != RUNTIME_NONE
                || step.mode != MODE_NONE
                || step.driver != DRIVER_NONE
                || facts.requested == "go"
            {
                return Err("verify plan routed an explicit non-Cargo request to Cargo".into());
            }
        }
        ROUTE_GO => {
            if step.runtime != RUNTIME_GO
                || step.mode != MODE_TESTS
                || step.driver != DRIVER_NONE
                || facts.requested != "go"
                || facts.kind != "tests"
            {
                return Err("verify plan gave Go a step its adapter cannot run".into());
            }
        }
        ROUTE_VERIFY => {
            if facts.requested == "rust" || facts.requested == "go" || facts.kind == "lint" {
                return Err("verify plan ran a verifier for an explicit non-verifier request".into());
            }
            match (step.mode, step.runtime, step.driver) {
                (MODE_CHECK, RUNTIME_NODE | RUNTIME_PYTHON, DRIVER_NONE) => {}
                (MODE_TESTS, RUNTIME_NODE, DRIVER_NODE_TESTS) => {}
                (MODE_TESTS, RUNTIME_PYTHON, DRIVER_PYTEST | DRIVER_UNITTEST) => {}
                (MODE_TESTS, RUNTIME_GO, DRIVER_NONE) => {}
                _ => {
                    return Err(
                        "verify plan paired a mode with an impossible runtime or driver".into(),
                    );
                }
            }
            if (step.mode == MODE_CHECK && facts.kind != "check")
                || (step.mode == MODE_TESTS && facts.kind != "tests")
            {
                return Err("verify plan ran a step the caller's tool did not ask for".into());
            }
        }
        _ => {
            let allowed = match step.reject {
                REJECT_AMBIGUOUS_LANGUAGE => facts.requested == "auto",
                REJECT_LINT_UNSUPPORTED => {
                    facts.kind == "lint" && matches!(step.runtime, RUNTIME_NODE | RUNTIME_PYTHON)
                }
                REJECT_GO_NEEDS_TESTS => facts.requested == "go" && facts.kind != "tests",
                _ => false,
            };
            if !allowed || step.mode != MODE_NONE || step.driver != DRIVER_NONE {
                return Err("verify plan refused a request its facts allow".into());
            }
        }
    }
    Ok(())
}

fn refusal(step: &Step) -> String {
    match step.reject {
        REJECT_AMBIGUOUS_LANGUAGE => AMBIGUOUS_LANGUAGE.into(),
        REJECT_LINT_UNSUPPORTED => {
            let runtime = if step.runtime == RUNTIME_NODE {
                Runtime::Node
            } else {
                Runtime::Python
            };
            LINT_UNSUPPORTED.replace("{}", runtime.label())
        }
        _ => GO_NEEDS_TESTS.into(),
    }
}

/// The shipped leg's answer for one feature record, for the parity tests.
#[cfg(test)]
pub(super) fn plan_step_with(
    facts: &PlanFacts,
    program: Option<angel_live::Program>,
) -> (Step, crate::agent::harness::live_policy::LegSource) {
    let input = serde_json::to_value(facts).unwrap_or(Value::Null);
    crate::agent::harness::live_policy::decide_with(
        program,
        VERIFY_PLAN,
        &input,
        |step: &Step| step_contract(facts, step),
        || step_native(facts),
    )
}

/// The native oracle alone, for comparison.
#[cfg(test)]
pub(super) fn plan_step_native(facts: &PlanFacts) -> Step {
    step_native(facts)
}

pub(super) enum Selection {
    Rust,
    Native(Plan),
}

pub(super) struct Plan {
    pub runtime: Runtime,
    pub kind: Kind,
    pub argvs: Vec<Vec<String>>,
    pub checked_files: usize,
}

/// Bytecode is read only from the interpreter's own library: the entries on
/// `sys.path` before the bootstrap inserts anything. Every other module, the
/// project's first of all, compiles from its source, so a stale or planted
/// project `.pyc` cannot stand in for it; `-B` keeps the run from writing any.
/// The library itself keeps its install-time bytecode instead of recompiling
/// unittest and its imports on every run (~150 ms of a ~190 ms run).
macro_rules! source_only_prelude {
    () => {
        "import sys, importlib.machinery as m; assert sys.dont_write_bytecode, 'isolated bytecode flags unavailable'; std=tuple(p for p in sys.path if p)\n\
         class S(m.SourceFileLoader):\n    def get_code(self, name): return compile(self.get_data(self.path), self.path, 'exec', dont_inherit=True)\n\
         def h(p):\n    if any(p == s or p.startswith(s + '/') for s in std): raise ImportError\n    return m.FileFinder(p, (m.ExtensionFileLoader, m.EXTENSION_SUFFIXES), (S, m.SOURCE_SUFFIXES), (m.SourcelessFileLoader, m.BYTECODE_SUFFIXES))\n\
         sys.path_hooks.insert(0, h); sys.path_importer_cache.clear()\n"
    };
}
const UNITTEST_BOOTSTRAP: &str = concat!(
    source_only_prelude!(),
    "import unittest; root=sys.argv.pop(1); sys.path.insert(0,root); unittest.main(module=None)"
);
const SYNTAX_BOOTSTRAP: &str =
    "import sys; [compile(open(p,'rb').read(),p,'exec') for p in sys.argv[1:]]";

fn manifest(root: &Path, name: &str) -> Result<Option<String>, String> {
    let path = root.join(name);
    if !path.exists() {
        return Ok(None);
    }
    let path = confined(root, &path)?;
    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return Err(format!("{name} is not a bounded regular project manifest"));
    }
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(format!("{name} grew beyond its manifest byte limit"));
    }
    Ok(Some(text))
}

pub(super) fn select(kind: Kind, args: &Value, root: &Path) -> Result<Selection, String> {
    let requested = requested_code(args)?;
    let entrypoint = entrypoint_code(args);
    // An explicit language settles the plan without reading the workspace; only
    // `auto` reads the manifests it decides from.
    let mut facts = PlanFacts {
        requested,
        entrypoint,
        kind: kind_code(kind),
        has_cargo_toml: false,
        has_package_json: false,
        has_pyproject_toml: false,
        has_python_setup: false,
        has_pytest_config: false,
    };
    let mut workspace = None;
    if !matches!(requested, "rust" | "go") {
        let resolved =
            std::fs::canonicalize(root).map_err(|e| format!("resolve verifier workspace: {e}"))?;
        let package = manifest(&resolved, "package.json")?
            .map(|text| serde_json::from_str::<Value>(&text))
            .transpose()
            .map_err(|e| format!("invalid package.json: {e}"))?;
        let pyproject = manifest(&resolved, "pyproject.toml")?
            .map(|text| text.parse::<toml::Value>())
            .transpose()
            .map_err(|e| format!("invalid pyproject.toml: {e}"))?;
        facts.has_cargo_toml = resolved.join("Cargo.toml").is_file();
        facts.has_package_json = package.is_some();
        facts.has_pyproject_toml = pyproject.is_some();
        facts.has_python_setup = ["setup.py", "setup.cfg", "pytest.ini"]
            .iter()
            .any(|name| resolved.join(name).is_file());
        // pytest is configured by an ini, a conftest, or a pyproject table.
        facts.has_pytest_config = resolved.join("pytest.ini").exists()
            || resolved.join("conftest.py").exists()
            || pyproject
                .as_ref()
                .and_then(|value| value.get("tool"))
                .and_then(|value| value.get("pytest"))
                .is_some();
        workspace = Some((resolved, package));
    }
    let (step, _source) = crate::agent::harness::live_policy::decide_shipped(
        VERIFY_PLAN,
        &serde_json::to_value(facts).unwrap_or(Value::Null),
        |step: &Step| step_contract(&facts, step),
    );
    if step.route == ROUTE_RUST {
        return Ok(Selection::Rust);
    }
    if step.route == ROUTE_REJECT {
        return Err(refusal(&step));
    }
    if step.route == ROUTE_GO {
        let extra =
            crate::agent::tools::build::parse_direct_argv(args["args"].as_str().unwrap_or(""))?;
        let mut argv = vec!["test".into(), "-json".into()];
        let mut value_next = false;
        for value in extra {
            if value_next {
                argv.push(value);
                value_next = false;
                continue;
            }
            if matches!(value.as_str(), "-run" | "-count" | "-skip") {
                value_next = true;
            } else if value != "-v" && value != "." && value != "./..." && !value.starts_with("./")
            {
                return Err("unsupported Go test option or package selector".into());
            }
            if value.starts_with("./") {
                let path = value.trim_end_matches("/...");
                confined(root, &root.join(path))?;
            }
            argv.push(value);
        }
        if value_next {
            return Err("Go test option needs a value".into());
        }
        discover_sources(root, root, Runtime::Go, true)?;
        return Ok(Selection::Native(Plan {
            runtime: Runtime::Go,
            kind,
            argvs: vec![argv],
            checked_files: 0,
        }));
    }
    let Some((root, package)) = workspace else {
        return Err("verifier planning read no workspace for a plan that needs one".into());
    };
    let runtime = match step.runtime {
        RUNTIME_NODE => Runtime::Node,
        RUNTIME_PYTHON => Runtime::Python,
        RUNTIME_GO => Runtime::Go,
        _ => return Err("verifier planning lost the runtime its step needs".into()),
    };
    let extra = match args.get("args") {
        None => Vec::new(),
        Some(Value::String(raw)) => parse_direct_argv(raw)?,
        _ => return Err("args must be a direct argument string".into()),
    };
    let mut plan = Plan {
        runtime,
        kind,
        argvs: Vec::new(),
        checked_files: 0,
    };
    if step.mode == MODE_CHECK {
        let files = sources(&root, runtime, &extra)?;
        plan.checked_files = files.len();
        plan.argvs = match runtime {
            Runtime::Node => files
                .into_iter()
                .map(|p| vec!["--check".into(), p])
                .collect(),
            Runtime::Go => return Err("Go syntax checks are unsupported".into()),
            Runtime::Python => vec![
                [
                    vec![
                        "-I".into(),
                        "-S".into(),
                        "-c".into(),
                        SYNTAX_BOOTSTRAP.into(),
                    ],
                    files,
                ]
                .concat(),
            ],
        };
    } else {
        plan.argvs.push(match runtime {
            Runtime::Node => node_tests(
                &root,
                if args["entrypoint"] == "node" {
                    None
                } else {
                    package.as_ref()
                },
                extra,
            )?,
            Runtime::Go => unreachable!("Go selected before manifest discovery"),
            Runtime::Python => match step.driver {
                DRIVER_PYTEST => python_pytest(&root, extra)?,
                _ => python_tests(&root, extra)?,
            },
        });
    }
    let argv_bytes = plan.argvs.iter().flatten().map(String::len).sum::<usize>();
    if argv_bytes > crate::agent::tools::build::MAX_DIRECT_ARG_BYTES {
        return Err(
            "planned native verifier argv exceeds the 64 KiB limit; select fewer files".into(),
        );
    }
    Ok(Selection::Native(plan))
}

fn node_tests(
    root: &Path,
    package: Option<&Value>,
    extra: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut selected = Vec::new();
    if let Some(script) = package
        .and_then(|v| v.get("scripts"))
        .and_then(|v| v.get("test"))
    {
        let script = script
            .as_str()
            .ok_or("package test script must be a string")?;
        let script = parse_direct_argv(script)?;
        if script.first().map(String::as_str) != Some("node")
            || script.get(1).map(String::as_str) != Some("--test")
        {
            return Err("configured test script is not the supported `node --test` entrypoint; custom package scripts cannot claim trusted verification".into());
        }
        selected.extend_from_slice(&script[2..]);
    }
    selected.extend(extra);
    if selected.is_empty() {
        // Avoid an unbounded Node-owned recursive search (which could enter
        // generated or quarantined directories). Explicit selectors remain
        // available for projects with a different test naming convention.
        selected = sources(root, Runtime::Node, &[])?
            .into_iter()
            .filter(|name| {
                let path = Path::new(name);
                let stem = path
                    .file_stem()
                    .and_then(|part| part.to_str())
                    .unwrap_or("");
                stem == "test"
                    || stem.starts_with("test-")
                    || stem.ends_with(".test")
                    || stem.ends_with("_test")
            })
            .collect();
        if selected.is_empty() {
            return Err(
                "no Node builtin test files discovered; select known test files explicitly".into(),
            );
        }
    }
    let mut argv = vec!["--test".into(), "--test-reporter=tap".into()];
    let mut index = 0;
    let mut has_files = false;
    while index < selected.len() {
        let value = &selected[index];
        if ["--test-name-pattern", "--test-skip-pattern"].contains(&value.as_str()) {
            index += 1;
            let pattern = selected
                .get(index)
                .ok_or("test pattern option needs a value")?;
            argv.extend([value.clone(), pattern.clone()]);
        } else if value.starts_with("--test-name-pattern=")
            || value.starts_with("--test-skip-pattern=")
        {
            argv.push(value.clone());
        } else if value.starts_with('-') {
            return Err(format!(
                "unsupported Node verifier option {value:?}; loaders, reporters, eval and custom startup flags are not accepted"
            ));
        } else {
            argv.extend(expand(root, value)?);
            has_files = true;
        }
        index += 1;
        if argv.len() > MAX_FILES + 8 {
            return Err("Node verifier selection exceeds its argument/file limit".into());
        }
    }
    if !has_files {
        // Pattern-only options must not hand recursive discovery to Node.
        // Reuse the bounded no-argument file selection, then retain the flags.
        let discovered = node_tests(root, None, Vec::new())?;
        argv.extend_from_slice(&discovered[2..]);
        if argv.len() > MAX_FILES + 8 {
            return Err("Node verifier selection exceeds its argument/file limit".into());
        }
    }
    Ok(argv)
}

fn python_tests(root: &Path, extra: Vec<String>) -> Result<Vec<String>, String> {
    let mut start = if root.join("tests").is_dir() {
        root.join("tests")
    } else {
        root.to_path_buf()
    };
    // Exercism-style suites name tests `foo_test.py` (suffix), not
    // `test_foo.py` (prefix); unittest's default `test*.py` finds zero of
    // them, exits 5, and the run_tests receipt degrades to "no tests ran".
    // Default to the pattern that matches both shapes.
    let mut pattern = "*test*.py".to_string();
    let mut top = None;
    let mut modules = Vec::new();
    let explicit_discover = extra.first().is_some_and(|s| s == "discover");
    let mut flags = Vec::new();
    let mut index = 0;
    while index < extra.len() {
        match extra[index].as_str() {
            "-v" | "--verbose" | "-q" | "--quiet" | "-f" | "--failfast" => {
                flags.push(extra[index].clone())
            }
            "discover" if index == 0 => {}
            "-k" => {
                index += 1;
                flags.extend([
                    "-k".into(),
                    extra.get(index).ok_or("unittest -k needs a value")?.clone(),
                ]);
            }
            "-t" | "--top-level-directory" => {
                index += 1;
                let path = confined(
                    root,
                    &root.join(extra.get(index).ok_or("top directory needs a value")?),
                )?;
                if !path.is_dir() {
                    return Err("unittest top path must be a directory".into());
                }
                top = Some(path);
            }
            "-s" | "--start-directory" => {
                index += 1;
                start = confined(
                    root,
                    &root.join(extra.get(index).ok_or("start directory needs a value")?),
                )?;
                if !start.is_dir() {
                    return Err("unittest start path must be a directory".into());
                }
            }
            "-p" | "--pattern" => {
                index += 1;
                pattern = extra
                    .get(index)
                    .ok_or("test pattern needs a value")?
                    .clone();
                if pattern.is_empty() || pattern.contains(['/', '\\']) {
                    return Err("unittest pattern must be a nonempty filename pattern".into());
                }
            }
            module if !module.starts_with('-') && !explicit_discover => {
                let normalized = module.trim_end_matches(".py").replace('/', ".");
                if normalized.is_empty()
                    || normalized.split('.').any(|part| {
                        part.is_empty()
                            || !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    })
                {
                    return Err("invalid unittest module/file selector".into());
                }
                modules.push(normalized);
            }
            option => {
                return Err(format!(
                    "unsupported unittest argument {option:?}; supported selectors are -k expression, -s directory, -p filename-pattern, -v, -q and -f"
                ));
            }
        }
        index += 1;
    }
    let start = confined(root, &start)?;
    // unittest owns recursive discovery after launch. Preflight its entire
    // selected tree, since silently skipping a path here would not stop the
    // runner from entering it later.
    let files = discover_sources(root, &start, Runtime::Python, true)?;
    validate_source_files(&files, Runtime::Python, true)?;
    let mut argv = vec![
        "-I".into(),
        "-S".into(),
        "-c".into(),
        UNITTEST_BOOTSTRAP.into(),
        root.to_string_lossy().into_owned(),
        "discover".into(),
        "-s".into(),
        start.to_string_lossy().into_owned(),
        "-p".into(),
        pattern.clone(),
    ];
    if !modules.is_empty() {
        if top.is_some()
            || pattern != "*test*.py"
            || start
                != confined(
                    root,
                    &if root.join("tests").is_dir() {
                        root.join("tests")
                    } else {
                        root.to_path_buf()
                    },
                )?
        {
            return Err(
                "unittest module selectors cannot be combined with discovery selectors".into(),
            );
        }
        // Validate imported workspace modules as strictly as discovery.
        discover_sources(root, root, Runtime::Python, true)?;
        argv.truncate(5);
        argv.extend(modules);
    } else if let Some(top) = top {
        argv.extend(["-t".into(), top.to_string_lossy().into_owned()]);
    }
    argv.extend(flags);
    Ok(argv)
}

fn confined(root: &Path, path: &Path) -> Result<PathBuf, String> {
    if path
        .components()
        .any(|part| part == Component::Normal("off-limits".as_ref()))
    {
        return Err("verifier selection enters a quarantined off-limits directory".into());
    }
    let path =
        std::fs::canonicalize(path).map_err(|e| format!("resolve verifier selection: {e}"))?;
    if path
        .components()
        .any(|part| part == Component::Normal("off-limits".as_ref()))
    {
        return Err("verifier selection resolves into a quarantined off-limits directory".into());
    }
    if !path_within(&path, root) {
        return Err("verifier selection escapes the workspace".into());
    }
    Ok(path)
}

fn expand(root: &Path, value: &str) -> Result<Vec<String>, String> {
    let path = root.join(value);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid verifier filename")?;
    if value.contains("**") || value.contains(['[', ']']) {
        return Err(
            "recursive or bracket globs are unsupported; use explicit bounded files".into(),
        );
    }
    if !name.contains(['*', '?']) {
        let path = confined(root, &path)?;
        if !path.is_file() {
            return Err("verifier selector must name regular files, not a directory".into());
        }
        return Ok(vec![path.to_string_lossy().into_owned()]);
    }
    let parent = confined(root, path.parent().ok_or("glob has no parent")?)?;
    let regex = regex::Regex::new(&format!(
        "^{}$",
        regex::escape(name).replace("\\*", ".*").replace("\\?", ".")
    ))
    .map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for (index, entry) in std::fs::read_dir(parent)
        .map_err(|e| e.to_string())?
        .enumerate()
    {
        if index >= MAX_ENTRIES {
            return Err("verifier glob exceeds the entry limit".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if regex.is_match(&entry.file_name().to_string_lossy()) {
            let path = confined(root, &entry.path())?;
            if !path.is_file() {
                return Err("verifier glob matched a non-file".into());
            }
            files.push(path.to_string_lossy().into_owned());
            if files.len() > MAX_FILES {
                return Err("verifier selection exceeds 256 files".into());
            }
        }
    }
    files.sort();
    if files.is_empty() {
        return Err(format!("verifier glob {value:?} matched no files"));
    }
    Ok(files)
}

fn source_extension(path: &Path, runtime: Runtime) -> bool {
    match runtime {
        Runtime::Node => matches!(
            path.extension().and_then(|p| p.to_str()),
            Some("js" | "mjs" | "cjs")
        ),
        Runtime::Go => path.extension().is_some_and(|p| p == "go"),
        Runtime::Python => path.extension().is_some_and(|p| p == "py"),
    }
}

fn sources(root: &Path, runtime: Runtime, extra: &[String]) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    if !extra.is_empty() {
        for value in extra {
            if value.starts_with('-') {
                return Err("syntax check accepts only confined source filenames/globs".into());
            }
            files.extend(expand(root, value)?);
        }
    } else {
        files = discover_sources(root, root, runtime, false)?;
    }
    files.sort();
    files.dedup();
    validate_source_files(&files, runtime, false)?;
    Ok(files)
}

fn discover_sources(
    root: &Path,
    start: &Path,
    runtime: Runtime,
    strict_tree: bool,
) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    let mut stack = vec![(start.to_path_buf(), 0)];
    let mut entries = 0;
    while let Some((dir, depth)) = stack.pop() {
        if depth > 16 {
            return Err("source discovery exceeds depth 16; narrow the selection".into());
        }
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            entries += 1;
            if entries > MAX_ENTRIES {
                return Err("source discovery exceeds 10000 entries; narrow the selection".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // Check the namespace before querying metadata or descending.
            if name == "off-limits" {
                if strict_tree {
                    return Err("unittest discovery contains a quarantined off-limits entry; select a narrower start directory".into());
                }
                continue;
            }
            if !strict_tree
                && (name.starts_with('.')
                    || [
                        "node_modules",
                        "target",
                        "vendor",
                        "dist",
                        "build",
                        "venv",
                        "__pycache__",
                    ]
                    .contains(&name.as_ref()))
            {
                continue;
            }
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                if strict_tree {
                    return Err("unittest discovery contains a symlink; select a symlink-free start directory".into());
                }
                continue;
            }
            if kind.is_dir() {
                stack.push((entry.path(), depth + 1));
            } else if kind.is_file() && source_extension(&entry.path(), runtime) {
                files.push(
                    confined(root, &entry.path())?
                        .to_string_lossy()
                        .into_owned(),
                );
                if files.len() > MAX_FILES {
                    return Err("source discovery exceeds 256 files; narrow the selection".into());
                }
            } else if strict_tree && !kind.is_file() {
                return Err("unittest discovery contains a non-regular entry".into());
            }
        }
    }
    Ok(files)
}

fn validate_source_files(
    files: &[String],
    runtime: Runtime,
    allow_empty: bool,
) -> Result<(), String> {
    if (!allow_empty && files.is_empty()) || files.len() > MAX_FILES {
        return Err("syntax check needs 1..256 source files".into());
    }
    let mut bytes = 0u64;
    for file in files {
        let path = Path::new(file);
        if !source_extension(path, runtime) {
            return Err("syntax selection contains an unsupported source extension".into());
        }
        let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
        bytes = bytes.saturating_add(metadata.len());
        if !metadata.is_file() || metadata.len() > 8 * 1024 * 1024 || bytes > 64 * 1024 * 1024 {
            return Err("syntax selection exceeds regular-file or byte limits".into());
        }
    }
    Ok(())
}

fn python_pytest(root: &Path, extra: Vec<String>) -> Result<Vec<String>, String> {
    let mut candidates = vec![PathBuf::from("/usr/lib/python3/dist-packages")];
    for minor in 10..=15 {
        candidates.push(PathBuf::from(format!(
            "/usr/local/lib/python3.{minor}/site-packages"
        )));
    }
    let installed = candidates.into_iter().find(|path| {
        let Ok(path) = std::fs::canonicalize(path) else { return false; };
        path.join("pytest/__init__.py").is_file()
            && !crate::agent::tools::build::task_writable_roots(root).iter().any(|r| path_within(&path, r))
    }).ok_or("no immutable system pytest installation is available; user-site pytest cannot supply typed evidence")?;
    let mut selected = Vec::new();
    let mut index = 0;
    while index < extra.len() {
        let value = &extra[index];
        match value.as_str() {
            "-k" => {
                index += 1;
                selected.extend([
                    "-k".into(),
                    extra.get(index).ok_or("pytest -k needs a value")?.clone(),
                ]);
            }
            "-q" | "-v" | "--verbose" | "--quiet" => {}
            value if !value.starts_with('-') => {
                let (path, _) = value.split_once("::").unwrap_or((value, ""));
                confined(root, &root.join(path))?;
                selected.push(value.into());
            }
            _ => {
                return Err(
                    "unsupported pytest option; only paths, -k, -q and -v are routed".into(),
                );
            }
        }
        index += 1;
    }
    discover_sources(root, root, Runtime::Python, true)?;
    let bootstrap = concat!(
        source_only_prelude!(),
        "site=sys.argv.pop(1); root=sys.argv.pop(1); sys.path.insert(0,site); import pytest; sys.path.insert(0,root); sys.exit(pytest.main(sys.argv[1:]))"
    );
    let mut argv = vec![
        "-I".into(),
        "-S".into(),
        "-c".into(),
        bootstrap.into(),
        installed.to_string_lossy().into_owned(),
        root.to_string_lossy().into_owned(),
        "-q".into(),
        "-p".into(),
        "no:cacheprovider".into(),
        "--color=no".into(),
    ];
    argv.extend(selected);
    Ok(argv)
}

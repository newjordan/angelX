use super::{Kind, Runtime};
use crate::agent::tools::build::{parse_direct_argv, path_within};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

const MAX_MANIFEST_BYTES: u64 = 128 * 1024;
const MAX_FILES: usize = 256;
const MAX_ENTRIES: usize = 10_000;

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

const UNITTEST_BOOTSTRAP: &str = "import sys, unittest; assert sys.dont_write_bytecode and sys.pycache_prefix, 'isolated bytecode flags unavailable'; root=sys.argv.pop(1); sys.path.insert(0,root); unittest.main(module=None)";
const SYNTAX_BOOTSTRAP: &str =
    "import pathlib, sys; [compile(pathlib.Path(p).read_bytes(),p,'exec') for p in sys.argv[1:]]";

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
    let requested = match args.get("runtime") {
        None => "auto",
        Some(Value::String(value)) => value.as_str(),
        _ => return Err("runtime must be auto, rust, node, or python".into()),
    };
    if requested == "rust" {
        return Ok(Selection::Rust);
    }
    if requested == "go" {
        if kind != Kind::Tests {
            return Err("Go adapter supports tests only".into());
        }
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
    let root =
        std::fs::canonicalize(root).map_err(|e| format!("resolve verifier workspace: {e}"))?;
    let package = manifest(&root, "package.json")?
        .map(|text| serde_json::from_str::<Value>(&text))
        .transpose()
        .map_err(|e| format!("invalid package.json: {e}"))?;
    let pyproject = manifest(&root, "pyproject.toml")?
        .map(|text| text.parse::<toml::Value>())
        .transpose()
        .map_err(|e| format!("invalid pyproject.toml: {e}"))?;
    let runtime = match requested {
        "node" => Runtime::Node,
        "python" => Runtime::Python,
        "auto" if args["entrypoint"] == "node" => Runtime::Node,
        "auto" if matches!(args["entrypoint"].as_str(), Some("unittest" | "pytest")) => {
            Runtime::Python
        }
        "auto" => {
            let rust = root.join("Cargo.toml").is_file();
            let node = package.is_some();
            let python = pyproject.is_some()
                || ["setup.py", "setup.cfg", "pytest.ini"]
                    .iter()
                    .any(|name| root.join(name).is_file());
            if usize::from(rust) + usize::from(node) + usize::from(python) != 1 {
                return Err("root language is absent or ambiguous; set runtime explicitly to rust, node, or python (subprojects need their own workspace)".into());
            }
            if rust {
                return Ok(Selection::Rust);
            }
            if node { Runtime::Node } else { Runtime::Python }
        }
        _ => return Err("runtime must be auto, rust, node, or python".into()),
    };
    if kind == Kind::Lint {
        return Err(format!(
            "no trusted {} linter adapter is installed; syntax checking is available through check, while custom linters can run through shell without trusted verification credit",
            runtime.label()
        ));
    }
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
    if kind == Kind::Check {
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
            Runtime::Python => {
                let configured_pytest = root.join("pytest.ini").exists()
                    || root.join("conftest.py").exists()
                    || pyproject
                        .as_ref()
                        .and_then(|v| v.get("tool"))
                        .and_then(|v| v.get("pytest"))
                        .is_some();
                let pytest = args["entrypoint"] == "pytest"
                    || (args["entrypoint"] != "unittest" && configured_pytest);
                if pytest {
                    python_pytest(&root, extra)?
                } else {
                    python_tests(&root, extra)?
                }
            }
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
    let bootstrap = "import sys; site=sys.argv.pop(1); root=sys.argv.pop(1); sys.path.insert(0,site); import pytest; sys.path.insert(0,root); sys.exit(pytest.main(sys.argv[1:]))";
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

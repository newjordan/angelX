//! The project brief: facts the harness gathers when a loop starts so the first
//! iteration begins oriented instead of spending its turns rediscovering the
//! machine, the benchmark and the workspace. The apollo and turbo comps read
//! all night (2026-09-24) for what a few probes answer: which GPU, where nvcc
//! lives, what the benchmark measures, which repositories sit inside the
//! workspace and which research notes already exist.
//!
//! Gathered off the UI thread (`gather` is blocking) and refreshed every
//! [`BRIEF_REFRESH_MS`]. Every section has a fixed bound. The brief rides the
//! iteration's system message, so it is stable between refreshes.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Whole-brief ceiling in characters.
pub(crate) const BRIEF_MAX_CHARS: usize = 7_000;
/// A loop's first iteration waits this long for its brief, then starts without it.
pub(crate) const BRIEF_WAIT_SECS: u64 = 20;
/// Age at which a live run gathers a fresh brief (in the background).
pub(crate) const BRIEF_REFRESH_MS: u64 = 2 * 60 * 60 * 1000;
const PROBE_SECS: u64 = 5;
/// Filesystem entries visited while mapping the workspace.
const WALK_LIMIT: usize = 20_000;
const WALK_DEPTH: usize = 3;
const REPOS_SHOWN: usize = 8;
const TOP_LEVEL_SHOWN: usize = 24;
const ENTRY_POINTS_SHOWN: usize = 12;
const NOTES_SHOWN: usize = 12;
const COMMITS_SHOWN: usize = 8;
const TRACK_TEXT_CHARS: usize = 400;
const SCORE_TEXT_CHARS: usize = 240;

/// Directories that are build output, caches or vendored environments.
const SKIP_DIRS: [&str; 12] = [
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
    ".mypy_cache",
    ".pytest_cache",
    ".tox",
    ".next",
    "dist",
    ".cache",
];

/// Files that say how a project is built, run or benchmarked.
const ENTRY_POINTS: [&str; 14] = [
    "benchmark.sh",
    "bench.sh",
    "run.sh",
    "setup.sh",
    "build.sh",
    "test.sh",
    "Makefile",
    "justfile",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "CMakeLists.txt",
    "go.mod",
];

/// Whether loops gather a brief. On by default; `ANGEL_LOOP_BRIEF=0` turns it
/// off. Unit tests opt in explicitly so they never probe the host.
pub(crate) fn enabled() -> bool {
    #[cfg(test)]
    {
        crate::agent::harness::env_flag("ANGEL_LOOP_BRIEF", false)
    }
    #[cfg(not(test))]
    {
        crate::agent::harness::env_flag("ANGEL_LOOP_BRIEF", true)
    }
}

/// Gather the brief for `workspace`. Blocking: shell probes and a bounded
/// filesystem walk. Never fails; an unanswered probe is reported as such.
pub(crate) fn gather(workspace: &Path) -> String {
    let now = now_secs();
    let walk = walk(workspace);
    let mut out = String::new();
    section(&mut out, "machine", &machine(workspace, &walk), 1_000);
    section(
        &mut out,
        "benchmark",
        &benchmark(workspace, &walk, now),
        2_000,
    );
    section(
        &mut out,
        "workspace",
        &workspace_map(workspace, &walk),
        1_800,
    );
    section(
        &mut out,
        "notes in the workspace, newest first",
        &notes(&walk, now),
        900,
    );
    section(&mut out, "recent commits", &commits(workspace), 900);
    clip(&out, BRIEF_MAX_CHARS)
}

/// Append `body` under `title`, clipped at a line boundary to `max` chars so
/// one long section never crowds out the rest.
fn section(out: &mut String, title: &str, body: &str, max: usize) {
    let body = clip_lines(body.trim_end(), max);
    if body.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(title);
    out.push_str(":\n");
    out.push_str(&body);
    out.push('\n');
}

// --- machine -----------------------------------------------------------------

fn machine(workspace: &Path, walk: &Walk) -> String {
    let mut lines = gpu_lines(workspace);
    lines.extend(cuda_compiler_lines(workspace, walk));
    let toolchains = toolchains(workspace);
    if !toolchains.is_empty() {
        lines.push(format!("toolchains: {}", toolchains.join(", ")));
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let cpu = cpu_model().unwrap_or_else(|| "CPU".to_string());
    let mut host = format!("{cpu}, {threads} threads");
    if let Some((total, available)) = memory_gib() {
        host.push_str(&format!("; RAM {total} GiB ({available} GiB available)"));
    }
    if let Some(free) = disk_free(workspace) {
        host.push_str(&format!("; {free} free on the workspace disk"));
    }
    lines.push(host);
    bullets(&lines)
}

fn gpu_lines(workspace: &Path) -> Vec<String> {
    let query = probe(
        "nvidia-smi",
        &[
            "--query-gpu=index,name,memory.total,memory.used,driver_version",
            "--format=csv,noheader,nounits",
        ],
        workspace,
    );
    if let Some(csv) = query {
        // `CUDA Version:` on older drivers, `CUDA UMD Version:` on newer ones.
        let cuda = probe("nvidia-smi", &[], workspace).and_then(|table| {
            let (_, rest) = table
                .split_once("CUDA Version:")
                .or_else(|| table.split_once("CUDA UMD Version:"))?;
            rest.split_whitespace().next().map(str::to_string)
        });
        let mut lines = Vec::new();
        for row in csv.lines() {
            let fields: Vec<&str> = row.split(',').map(str::trim).collect();
            if let [index, name, total, used, driver] = fields[..] {
                let cuda = cuda
                    .as_deref()
                    .map(|v| format!(", CUDA {v}"))
                    .unwrap_or_default();
                lines.push(format!(
                    "GPU {index}: {name}, {total} MiB ({used} MiB in use), driver {driver}{cuda}"
                ));
            }
        }
        if !lines.is_empty() {
            return lines;
        }
    }
    if let Some(rocm) = probe("rocm-smi", &["--showproductname"], workspace) {
        let cards: Vec<&str> = rocm
            .lines()
            .filter(|line| line.contains("Card series") || line.contains("Card model"))
            .map(str::trim)
            .take(4)
            .collect();
        if !cards.is_empty() {
            return cards.iter().map(|card| format!("GPU: {card}")).collect();
        }
    }
    vec!["GPU: none reported (nvidia-smi and rocm-smi did not answer)".to_string()]
}

/// Where a CUDA compiler lives. The PATH answer first, then every usual
/// install place: `CUDA_HOME`/`CUDA_PATH`, `/usr/local/cuda*`, `/opt/cuda`,
/// conda, toolkits unpacked under `/tmp` (earlier runs have done that), and
/// `nvcc`/`ptxas` inside the workspace's own environments. A pip
/// `nvidia-cuda-nvcc` wheel ships `ptxas` but no `nvcc`; saying so saves the
/// hunt.
fn cuda_compiler_lines(workspace: &Path, walk: &Walk) -> Vec<String> {
    let on_path = which("nvcc");
    let mut candidates: Vec<PathBuf> = Vec::new();
    for var in ["CUDA_HOME", "CUDA_PATH"] {
        if let Some(home) = std::env::var_os(var) {
            candidates.push(PathBuf::from(home).join("bin/nvcc"));
        }
    }
    candidates.push(PathBuf::from("/usr/local/cuda/bin/nvcc"));
    candidates.push(PathBuf::from("/opt/cuda/bin/nvcc"));
    candidates.extend(children_with("/usr/local", "cuda-", "bin/nvcc"));
    candidates.extend(children_with("/tmp", "", "bin/nvcc"));
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for conda in ["miniconda3", "anaconda3", "miniforge3", ".conda"] {
            candidates.push(home.join(conda).join("bin/nvcc"));
        }
    }
    let mut ptxas = Vec::new();
    for entry in walk
        .entries
        .iter()
        .filter(|entry| entry.depth == 1 && entry.dir)
    {
        let env = workspace.join(&entry.rel);
        candidates.push(env.join("bin/nvcc"));
        for site in children_with(
            &env.join("lib"),
            "python",
            "site-packages/nvidia/cuda_nvcc/bin",
        ) {
            candidates.push(site.join("nvcc"));
            if site.join("ptxas").is_file() {
                ptxas.push(site.join("ptxas"));
            }
        }
    }
    let mut found: Vec<PathBuf> = Vec::new();
    for path in on_path.iter().cloned().chain(candidates) {
        let real = path.canonicalize().unwrap_or(path.clone());
        if path.is_file()
            && !found
                .iter()
                .any(|seen| seen.canonicalize().ok() == Some(real.clone()))
        {
            found.push(path);
        }
    }
    let mut lines: Vec<String> = found
        .iter()
        .take(3)
        .map(|path| {
            let release = probe(&path.to_string_lossy(), &["--version"], workspace)
                .and_then(|text| {
                    let (_, rest) = text.split_once("release ")?;
                    rest.split(',').next().map(|v| v.trim().to_string())
                })
                .map(|v| format!(" (release {v})"))
                .unwrap_or_default();
            let place = if on_path.as_deref() == Some(path.as_path()) {
                ", on PATH"
            } else {
                ", not on PATH"
            };
            format!("nvcc: {}{release}{place}", path.display())
        })
        .collect();
    if lines.is_empty() {
        lines.push(
            "nvcc: none found (PATH, CUDA_HOME, /usr/local/cuda*, /opt/cuda, conda, /tmp/*/bin, \
             workspace environments)"
                .to_string(),
        );
    }
    if let Some(ptxas) = ptxas.first()
        && found.iter().all(|path| path.parent() != ptxas.parent())
    {
        lines.push(format!(
            "ptxas: {} (a pip nvidia-cuda-nvcc wheel: ptxas only, no nvcc)",
            ptxas.display()
        ));
    }
    lines
}

/// `<dir>/<child>/<tail>` for every child of `dir` whose name starts with
/// `prefix` (all children for an empty prefix), newest name first.
fn children_with(dir: impl AsRef<Path>, prefix: &str, tail: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir.as_ref()) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
        .map(|entry| entry.path().join(tail))
        .filter(|path| path.exists())
        .collect();
    out.sort();
    out.reverse();
    out
}

fn toolchains(workspace: &Path) -> Vec<String> {
    const PROGRAMS: [(&str, &str, &str); 9] = [
        ("rustc", "rustc", "--version"),
        ("cargo", "cargo", "--version"),
        ("python", "python3", "--version"),
        ("uv", "uv", "--version"),
        ("gcc", "gcc", "--version"),
        ("clang", "clang", "--version"),
        ("cmake", "cmake", "--version"),
        ("node", "node", "--version"),
        ("go", "go", "version"),
    ];
    PROGRAMS
        .iter()
        .filter(|(_, program, _)| which(program).is_some())
        .filter_map(|(label, program, arg)| {
            let text = probe(program, &[arg], workspace)?;
            let version = version_token(text.lines().next().unwrap_or_default())?;
            Some(format!("{label} {version}"))
        })
        .collect()
}

/// The first dotted version in a `--version` line (`rustc 1.95.0 (…)`,
/// `v24.1.0`, `go1.24.1`).
fn version_token(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|token| {
        let token = token.trim_start_matches(|c: char| !c.is_ascii_digit());
        let version: String = token
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        version.contains('.').then_some(version)
    })
}

fn cpu_model() -> Option<String> {
    let info = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    info.lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, model)| model.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn memory_gib() -> Option<(u64, u64)> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let field = |name: &str| {
        info.lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kib| kib.parse::<u64>().ok())
    };
    let gib = |kib: u64| (kib + (1 << 19)) >> 20;
    Some((gib(field("MemTotal:")?), gib(field("MemAvailable:")?)))
}

fn disk_free(workspace: &Path) -> Option<String> {
    let out = probe("df", &["-Ph", &workspace.to_string_lossy()], workspace)?;
    out.lines()
        .nth(1)
        .and_then(|row| row.split_whitespace().nth(3))
        .map(str::to_string)
}

// --- benchmark ---------------------------------------------------------------

fn benchmark(workspace: &Path, walk: &Walk, now: u64) -> String {
    let mut specs: Vec<&WalkEntry> = walk
        .entries
        .iter()
        .filter(|entry| !entry.dir && entry.depth <= 2 && entry.name() == "benchmark.json")
        .collect();
    specs.sort_by_key(|entry| entry.depth);
    // Clones of one challenge carry the same spec: one card, with its copies.
    let mut groups: Vec<(String, Vec<PathBuf>)> = Vec::new();
    for spec in specs {
        let text = std::fs::read_to_string(workspace.join(&spec.rel)).unwrap_or_default();
        match groups.iter_mut().find(|(seen, _)| *seen == text) {
            Some((_, copies)) => copies.push(spec.rel.clone()),
            None => groups.push((text, vec![spec.rel.clone()])),
        }
    }
    let mut out = Vec::new();
    for (_, copies) in groups.iter().take(3) {
        out.extend(benchmark_card(workspace, copies, now));
    }
    out.join("\n")
}

/// One `benchmark.json` (the Yukon/Hilbert benchmark spec) as prompt lines;
/// `copies` are the workspace paths holding this same spec, shallowest first.
fn benchmark_card(workspace: &Path, copies: &[PathBuf], now: u64) -> Vec<String> {
    let Some(rel) = copies.first() else {
        return Vec::new();
    };
    let path = workspace.join(rel);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(spec) = serde_json::from_str::<serde_json::Value>(&text) else {
        return vec![format!("{} (not valid JSON)", rel.display())];
    };
    let dir = rel.parent().unwrap_or(Path::new(""));
    let name = spec.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let mut lines = vec![if name.is_empty() {
        format!("{}:", rel.display())
    } else {
        format!("{} ({name}):", rel.display())
    }];
    if copies.len() > 1 {
        let others: Vec<String> = copies[1..]
            .iter()
            .take(8)
            .map(|copy| display_dir(copy.parent().unwrap_or(Path::new(""))))
            .collect();
        let more = copies.len().saturating_sub(9);
        let more = if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        };
        lines.push(format!(
            "the same spec is in {} more copies: {}{more}",
            copies.len() - 1,
            others.join(", ")
        ));
    }
    let tracks = spec
        .get("tracks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for track in tracks.iter().take(6) {
        let field = |key: &str| track.get(key).and_then(|v| v.as_str()).unwrap_or("");
        let direction = match field("direction") {
            "+" => " (higher is better)",
            "-" => " (lower is better)",
            _ => "",
        };
        let description = clip(field("description"), TRACK_TEXT_CHARS);
        lines.push(format!(
            "- track {}{direction}: {description}",
            field("name")
        ));
        let edit = track
            .get("editablePaths")
            .and_then(|v| v.as_array())
            .map(|paths| {
                paths
                    .iter()
                    .filter_map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        if !edit.is_empty() {
            lines.push(format!("  editable: {edit}"));
        }
        for (label, key) in [("setup", "setupCommand"), ("run", "benchmarkCommand")] {
            if let Some(command) = track.get(key).and_then(command_text) {
                lines.push(format!("  {label}: {command} (in {})", display_dir(dir)));
            }
        }
        if let Some(bips) = track
            .get("minScoreImprovementBips")
            .and_then(|v| v.as_u64())
        {
            lines.push(format!(
                "  a submission must beat the current score by {bips} bips ({:.2}%)",
                bips as f64 / 100.0
            ));
        }
        let score = field("scorePath");
        if !score.is_empty() {
            // The newest score file across every copy of this spec.
            let newest = copies
                .iter()
                .map(|copy| {
                    let dir = copy.parent().unwrap_or(Path::new(""));
                    (dir, workspace.join(dir).join(score))
                })
                .filter_map(|(dir, file)| {
                    let modified = std::fs::metadata(&file).ok()?.modified().ok()?;
                    Some((unix_secs(modified), dir, file))
                })
                .max_by_key(|(modified, _, _)| *modified);
            lines.push(match newest {
                Some((_, dir, file)) => format!(
                    "  score file {score} in {}: {}",
                    display_dir(dir),
                    local_score(&file, now).unwrap_or_default()
                ),
                None => format!("  score file {score}: not written yet"),
            });
        }
    }
    lines
}

fn command_text(value: &serde_json::Value) -> Option<String> {
    let parts: Vec<&str> = value
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    match parts[..] {
        [] => None,
        // ["bash", "-lc", "<script>"] reads as the script itself.
        [_, flag, script] if flag.starts_with('-') && flag.ends_with('c') => {
            Some(script.to_string())
        }
        _ => Some(parts.join(" ")),
    }
}

fn local_score(file: &Path, now: u64) -> Option<String> {
    let modified = std::fs::metadata(file).ok()?.modified().ok()?;
    let text = std::fs::read_to_string(file).ok()?;
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(format!(
        "{}, {}",
        age(now, unix_secs(modified)),
        clip(&flat, SCORE_TEXT_CHARS)
    ))
}

fn display_dir(dir: &Path) -> String {
    if dir.as_os_str().is_empty() {
        "the workspace root".to_string()
    } else {
        format!("{}/", dir.display())
    }
}

// --- workspace ---------------------------------------------------------------

struct WalkEntry {
    rel: PathBuf,
    depth: usize,
    dir: bool,
    /// A directory with its own `.git`.
    repo: bool,
    bytes: u64,
    modified: u64,
}

impl WalkEntry {
    fn name(&self) -> String {
        self.rel
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

struct Walk {
    entries: Vec<WalkEntry>,
    truncated: bool,
}

/// Breadth-first, `WALK_DEPTH` deep, at most `WALK_LIMIT` entries. Hidden and
/// build/vendor directories are not entered; symlinks are listed, not followed.
fn walk(root: &Path) -> Walk {
    let mut entries = Vec::new();
    let mut queue = std::collections::VecDeque::from([(PathBuf::new(), 0usize)]);
    let mut truncated = false;
    'walk: while let Some((rel, depth)) = queue.pop_front() {
        let Ok(read) = std::fs::read_dir(root.join(&rel)) else {
            continue;
        };
        let mut children: Vec<_> = read.flatten().collect();
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            if entries.len() >= WALK_LIMIT {
                truncated = true;
                break 'walk;
            }
            let name = child.file_name().to_string_lossy().into_owned();
            let Ok(meta) = child.path().symlink_metadata() else {
                continue;
            };
            let child_rel = rel.join(&name);
            let dir = meta.is_dir();
            let repo = dir && child.path().join(".git").exists();
            entries.push(WalkEntry {
                rel: child_rel.clone(),
                depth: depth + 1,
                dir,
                repo,
                bytes: meta.len(),
                modified: meta.modified().map(unix_secs).unwrap_or(0),
            });
            if dir
                && depth + 1 < WALK_DEPTH
                && !name.starts_with('.')
                && !SKIP_DIRS.contains(&name.as_str())
            {
                queue.push_back((child_rel, depth + 1));
            }
        }
    }
    Walk { entries, truncated }
}

fn workspace_map(workspace: &Path, walk: &Walk) -> String {
    let mut lines = vec![root_line(workspace)];
    let repos: Vec<&WalkEntry> = walk.entries.iter().filter(|entry| entry.repo).collect();
    if !repos.is_empty() {
        lines.push(format!(
            "{} repositories inside the workspace (their changes do not show in the top-level git diff):",
            repos.len()
        ));
        for repo in repos.iter().take(REPOS_SHOWN) {
            let last = git(
                &workspace.join(&repo.rel),
                &["log", "-1", "--format=%h, %ar: %s"],
            )
            .map(|line| format!(" ({})", clip(&line, 72)))
            .unwrap_or_default();
            lines.push(format!("  {}/{last}", repo.rel.display()));
        }
        if repos.len() > REPOS_SHOWN {
            lines.push(format!("  … and {} more", repos.len() - REPOS_SHOWN));
        }
    }
    lines.push("top level:".to_string());
    lines.extend(top_level(walk).into_iter().map(|line| format!("  {line}")));
    // One line per build file name, with the directories that carry it
    // (clones of one challenge all carry the same `benchmark.sh`).
    let mut entry_points: Vec<(String, Vec<String>)> = Vec::new();
    for entry in walk
        .entries
        .iter()
        .filter(|entry| !entry.dir && entry.depth <= 2)
        .filter(|entry| ENTRY_POINTS.contains(&entry.name().as_str()))
    {
        let dir = display_dir(entry.rel.parent().unwrap_or(Path::new("")));
        match entry_points
            .iter_mut()
            .find(|(name, _)| *name == entry.name())
        {
            Some((_, dirs)) => dirs.push(dir),
            None => entry_points.push((entry.name(), vec![dir])),
        }
    }
    if !entry_points.is_empty() {
        let listed: Vec<String> = entry_points
            .iter()
            .take(ENTRY_POINTS_SHOWN)
            .map(|(name, dirs)| {
                let more = dirs.len().saturating_sub(4);
                let more = if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                };
                let shown: Vec<&str> = dirs.iter().take(4).map(String::as_str).collect();
                format!("{name} (in {}{more})", shown.join(", "))
            })
            .collect();
        lines.push(format!("build and run files: {}", listed.join("; ")));
    }
    if walk.truncated {
        lines.push(format!(
            "(mapped the first {WALK_LIMIT} entries, {WALK_DEPTH} levels deep)"
        ));
    }
    lines.join("\n")
}

fn root_line(workspace: &Path) -> String {
    let Some(head) = git(workspace, &["rev-parse", "--short", "HEAD"]) else {
        return format!("{} (not a git repository)", workspace.display());
    };
    let branch = git(workspace, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let dirty = git(
        workspace,
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .map(|status| status.lines().count())
    .unwrap_or(0);
    let dirty = match dirty {
        0 => String::new(),
        1 => ", 1 file with uncommitted changes".to_string(),
        n => format!(", {n} files with uncommitted changes"),
    };
    format!("{}: git {branch} at {head}{dirty}", workspace.display())
}

/// Top-level directories, with runs of four or more same-prefix siblings
/// (`bench-iteration1`, `bench-iteration2`, …) folded into one line. Loose
/// files are listed when few, else counted by type with the largest named.
fn top_level(walk: &Walk) -> Vec<String> {
    const LOOSE_FILES_LISTED: usize = 8;
    let top: Vec<&WalkEntry> = walk
        .entries
        .iter()
        .filter(|entry| entry.depth == 1)
        .filter(|entry| {
            let name = entry.name();
            !name.starts_with('.') && !(entry.dir && SKIP_DIRS.contains(&name.as_str()))
        })
        .collect();
    // `bench-iteration7` → `bench-`, `run12` → `run`: up to and including
    // the first separator, or up to the first digit.
    let prefix = |name: &str| -> String {
        match name.find(|c: char| c == '-' || c == '_' || c == '.' || c.is_ascii_digit()) {
            Some(at) if name[at..].starts_with(|c: char| c.is_ascii_digit()) => {
                name[..at].to_string()
            }
            Some(at) => name[..=at].to_string(),
            None => String::new(),
        }
    };
    let dirs: Vec<&&WalkEntry> = top.iter().filter(|entry| entry.dir).collect();
    let mut groups: std::collections::BTreeMap<String, usize> = Default::default();
    for entry in &dirs {
        let key = prefix(&entry.name());
        if !key.is_empty() && key.len() < entry.name().len() {
            *groups.entry(key).or_default() += 1;
        }
    }
    let children = |entry: &WalkEntry| {
        walk.entries
            .iter()
            .filter(|other| other.depth == 2 && other.rel.starts_with(&entry.rel))
            .count()
    };
    let mut lines = Vec::new();
    let mut folded = std::collections::BTreeSet::new();
    for entry in &dirs {
        let name = entry.name();
        let key = prefix(&name);
        if groups.get(&key).is_some_and(|n| *n >= 4) {
            if folded.insert(key.clone()) {
                lines.push(format!("{key}* ({} directories)", groups[&key]));
            }
            continue;
        }
        let n = children(entry);
        let noun = if n == 1 { "entry" } else { "entries" };
        let repo = if entry.repo { ", a git repository" } else { "" };
        lines.push(format!("{name}/ ({n} {noun}{repo})"));
    }
    let files: Vec<&&WalkEntry> = top.iter().filter(|entry| !entry.dir).collect();
    if files.len() <= LOOSE_FILES_LISTED {
        for file in &files {
            lines.push(format!("{} ({})", file.name(), size(file.bytes)));
        }
    } else {
        let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
        for file in &files {
            let name = file.name();
            let kind = match name.rsplit_once('.') {
                Some((stem, ext)) if !stem.is_empty() => format!(".{ext}"),
                _ => "no extension".to_string(),
            };
            *kinds.entry(kind).or_default() += 1;
        }
        let mut kinds: Vec<(String, usize)> = kinds.into_iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let kinds: Vec<String> = kinds
            .iter()
            .take(6)
            .map(|(kind, n)| format!("{n} {kind}"))
            .collect();
        let mut largest: Vec<&&&WalkEntry> = files.iter().collect();
        largest.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        let largest: Vec<String> = largest
            .iter()
            .take(3)
            .map(|file| format!("{} {}", file.name(), size(file.bytes)))
            .collect();
        lines.push(format!(
            "{} loose files ({}); largest: {}",
            files.len(),
            kinds.join(", "),
            largest.join(", ")
        ));
    }
    if lines.len() > TOP_LEVEL_SHOWN {
        let more = lines.len() - TOP_LEVEL_SHOWN;
        lines.truncate(TOP_LEVEL_SHOWN);
        lines.push(format!("… and {more} more"));
    }
    lines
}

// --- notes and history -------------------------------------------------------

fn notes(walk: &Walk, now: u64) -> String {
    let mut notes: Vec<&WalkEntry> = walk
        .entries
        .iter()
        .filter(|entry| !entry.dir)
        .filter(|entry| {
            let name = entry.name().to_ascii_lowercase();
            name.ends_with(".md")
                || name.ends_with(".markdown")
                || name == "notes"
                || name == "todo"
        })
        .collect();
    notes.sort_by(|a, b| b.modified.cmp(&a.modified));
    let lines: Vec<String> = notes
        .iter()
        .take(NOTES_SHOWN)
        .map(|note| {
            format!(
                "{} ({}, {})",
                note.rel.display(),
                size(note.bytes),
                age(now, note.modified)
            )
        })
        .collect();
    bullets(&lines)
}

fn commits(workspace: &Path) -> String {
    git(
        workspace,
        &["log", &format!("-{COMMITS_SHOWN}"), "--format=%h %ar: %s"],
    )
    .map(|log| {
        log.lines()
            .map(|line| format!("- {}", clip(line, 140)))
            .collect::<Vec<_>>()
            .join("\n")
    })
    .unwrap_or_default()
}

// --- helpers -----------------------------------------------------------------

fn probe(program: &str, args: &[&str], cwd: &Path) -> Option<String> {
    crate::platform::workspace_store::capture_repo_probe(program, args, cwd, PROBE_SECS)
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut full: Vec<&str> = crate::agent::harness::GIT_NO_WORKSPACE_EXEC.to_vec();
    full.extend_from_slice(args);
    probe("git", &full, dir)
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

fn bullets(lines: &[String]) -> String {
    lines
        .iter()
        .map(|line| format!("- {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn size(bytes: u64) -> String {
    match bytes {
        0..=1_023 => format!("{bytes} B"),
        1_024..=1_048_575 => format!("{} KB", bytes / 1_024),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

/// Compact "how long ago" (`just now`, `12 min ago`, `3 h ago`, `2 days ago`).
pub(crate) fn age(now: u64, then: u64) -> String {
    let secs = now.saturating_sub(then);
    match secs {
        0..=59 => "just now".to_string(),
        60..=3_599 => format!("{} min ago", secs / 60),
        3_600..=86_399 => format!("{} h ago", secs / 3_600),
        86_400..=172_799 => "1 day ago".to_string(),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// Whole lines of `text` up to `max` chars, noting how many were left out.
fn clip_lines(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out = String::new();
    let mut kept = 0;
    let lines: Vec<&str> = text.lines().collect();
    for line in &lines {
        if out.chars().count() + line.chars().count() + 1 > max.saturating_sub(40) {
            break;
        }
        out.push_str(line);
        out.push('\n');
        kept += 1;
    }
    out.push_str(&format!("… {} more lines", lines.len() - kept));
    out
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn unix_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn now_secs() -> u64 {
    unix_secs(SystemTime::now())
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/loop_ctl/brief__tests.rs"]
mod tests;

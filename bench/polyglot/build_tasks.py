"""Build the polyglot-v1 task catalog from Aider's polyglot-benchmark.

These are blank Exercism stubs: the agent writes each solution from scratch.
The build follows Aider's own benchmark runner:
  * prompt = .docs/introduction.md + instructions.md + instructions.append.md,
    then Aider's instructions addendum, plus one agentic line saying to run the tests;
  * the agent edits only the exercise's solution files. Tests and build files are
    protected: the grader rebuilds the exercise from the pristine fixture and
    carries over only the solution files;
  * every test runs. JS `xtest(` becomes `test(` (Aider's npm-test.sh), Rust
    `#[ignore]` is removed (Aider: cargo test -- --include-ignored), and C++ test
    files get EXERCISM_RUN_ALL_TESTS defined at the top (Aider: cmake
    -DEXERCISM_RUN_ALL_TESTS=1). Without that define a Catch2 suite runs only its
    first test case;
  * .meta/ (reference solutions) and .docs/ never reach the agent's workspace.

Gold is the exercise's own reference solution mapped onto its solution files. It is
used only by `validate` to prove each task is solvable. The six Rust references
that need crates also carry their official Cargo-example.toml for that proof;
agents keep the pristine Cargo.toml.
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
from pathlib import Path

from common import KIT, LANG_ORDER, UPSTREAM_COMMIT, UPSTREAM_URL, catalog_digest, work_dir

# Aider benchmark/prompts.py instructions_addendum, verbatim.
AIDER_ADDENDUM = (
    "\n####\n\n"
    "Use the above instructions to modify the supplied files: {file_list}\n"
    "Don't change the names of existing functions or classes, as they may be "
    "referenced from other code like unit tests, etc.\n"
    "Only use standard libraries, don't suggest installing any packages.\n"
)
AGENT_LINE = (
    "\nThe tests in {test_list} are correct; do not change them or the build files. "
    "Run the tests with `{test_cmd}` and finish only after they pass.\n"
)

GITIGNORE = "node_modules\nbuild/\ntarget/\n__pycache__/\n.pytest_cache/\n"

LANGS = {
    "javascript": {
        "prefix": "js",
        "test_cmd": "npm test",
        "protect_extra": ["package.json", "babel.config.js"],
    },
    "python": {
        "prefix": "py",
        "test_cmd": "python3 -m pytest -q",
        "protect_extra": [],
    },
    "rust": {
        "prefix": "rust",
        "test_cmd": "cargo test",
        "protect_extra": ["Cargo.toml"],
    },
    "cpp": {
        "prefix": "cpp",
        "test_cmd": "cmake -S . -B build && cmake --build build",
        "protect_extra": ["CMakeLists.txt", "test/catch.hpp", "test/tests-main.cpp"],
    },
}

# Fixed for every machine, so everyone runs the same 136 tasks: a task whose own
# reference solution cannot pass its tests on a stock toolchain is left out.
EXCLUDED = {
    "cpp-gigasecond": "reference needs Boost.DateTime",
    "cpp-meetup": "reference needs Boost.DateTime",
    "js-ledger": "reference fails 10 of 11 tests under Node 26's Intl currency formatting",
}

RUST_IGNORE = re.compile(r"^[ \t]*#\[ignore\][ \t]*\r?\n", re.MULTILINE)
JS_XTEST = re.compile(r"\bxtest\(")
CPP_RUN_ALL = "#ifndef EXERCISM_RUN_ALL_TESTS\n#define EXERCISM_RUN_ALL_TESTS\n#endif\n"


def enable_all_tests(lang: str, text: str) -> str:
    if lang == "javascript":
        return JS_XTEST.sub("test(", text)
    if lang == "rust":
        return RUST_IGNORE.sub("", text)
    if lang == "cpp":
        return CPP_RUN_ALL + text
    return text


def instructions(ex: Path) -> str:
    parts = []
    for name in ("introduction.md", "instructions.md", "instructions.append.md"):
        path = ex / ".docs" / name
        if path.exists():
            parts.append(path.read_text().strip() + "\n")
    return "\n".join(parts)


def gold_mapping(lang: str, solution: list[str], example: list[str]) -> dict[str, str | None]:
    """Map each solution file to the reference file that implements it. None keeps
    the fixture's own file (a header-only reference leaves its .cpp stub as is)."""
    if lang == "rust":
        rs = [p for p in solution if p.endswith(".rs")]
        if len(rs) != 1 or len(example) != 1:
            raise ValueError(f"unexpected rust layout: {solution} / {example}")
        return {rs[0]: example[0]}
    if len(solution) == 1 and len(example) == 1:
        return {solution[0]: example[0]}
    by_ext: dict[str, list[str]] = {}
    for ex in example:
        by_ext.setdefault(Path(ex).suffix, []).append(ex)
    mapping: dict[str, str | None] = {}
    for sol in solution:
        candidates = by_ext.get(Path(sol).suffix, [])
        if len(candidates) > 1:
            raise ValueError(f"ambiguous reference for {sol}: {example}")
        mapping[sol] = candidates[0] if candidates else None
    if not any(mapping.values()):
        raise ValueError(f"no reference maps onto {solution}: {example}")
    return mapping


def fetch_upstream(dest: Path) -> Path:
    """Clone Aider's polyglot-benchmark at the pinned commit (once)."""
    if not (dest / ".git").exists():
        dest.parent.mkdir(parents=True, exist_ok=True)
        print(f"cloning {UPSTREAM_URL} into {dest}")
        subprocess.run(["git", "clone", "--quiet", UPSTREAM_URL, str(dest)], check=True)
    head = subprocess.run(
        ["git", "-C", str(dest), "rev-parse", "HEAD"], capture_output=True, text=True, check=True
    ).stdout.strip()
    if head != UPSTREAM_COMMIT:
        subprocess.run(["git", "-C", str(dest), "fetch", "--quiet", "origin"], check=False)
        subprocess.run(
            ["git", "-C", str(dest), "checkout", "--quiet", "--detach", UPSTREAM_COMMIT], check=True
        )
    return dest


def build(upstream: Path | None = None) -> dict:
    work = work_dir()
    upstream = upstream or fetch_upstream(work / "upstream" / "polyglot-benchmark")
    for out in ("fixtures", "gold"):
        if (work / out).exists():
            shutil.rmtree(work / out)
    per_lang: dict[str, list[dict]] = {}
    grading: dict[str, dict] = {}
    for lang in LANG_ORDER:
        cfg = LANGS[lang]
        rows = []
        for ex in sorted((upstream / lang / "exercises" / "practice").iterdir()):
            name = f"{cfg['prefix']}-{ex.name}"
            if name in EXCLUDED:
                continue
            meta = json.loads((ex / ".meta" / "config.json").read_text())["files"]
            solution = [p for p in meta["solution"] if p != "Cargo.toml"]
            tests = list(meta["test"])
            if lang == "rust":
                # Aider runs `cargo test -- --include-ignored`, which covers every
                # integration test file, listed in config.json or not.
                extra = sorted(p.relative_to(ex).as_posix() for p in (ex / "tests").rglob("*.rs"))
                tests += [p for p in extra if p not in tests]
            fixture = work / "fixtures" / name
            gold = work / "gold" / name
            for src in sorted(ex.rglob("*")):
                rel = src.relative_to(ex)
                if rel.parts[0] in (".meta", ".docs") or src.is_dir():
                    continue
                dst = fixture / rel
                dst.parent.mkdir(parents=True, exist_ok=True)
                text = src.read_text()
                if rel.as_posix() in tests:
                    text = enable_all_tests(lang, text)
                dst.write_text(text)
            (fixture / ".gitignore").write_text(GITIGNORE)
            for rel, ref in gold_mapping(lang, solution, meta["example"]).items():
                dst = gold / rel
                dst.parent.mkdir(parents=True, exist_ok=True)
                dst.write_text((ex / ref).read_text() if ref else (fixture / rel).read_text())
            cargo_example = ex / ".meta" / "Cargo-example.toml"
            gold_extra = []
            if lang == "rust" and cargo_example.exists():
                (gold / "Cargo.toml").write_text(cargo_example.read_text())
                gold_extra = ["Cargo.toml"]
            protected = sorted(set(tests) | {p for p in cfg["protect_extra"] if (fixture / p).exists()})
            missing = [p for p in protected + solution if not (fixture / p).exists()]
            if missing:
                raise ValueError(f"{name}: fixture lacks {missing}")
            prompt = (
                instructions(ex)
                + AIDER_ADDENDUM.format(file_list=" ".join(solution))
                + AGENT_LINE.format(test_list=" ".join(tests), test_cmd=cfg["test_cmd"])
            )
            rows.append(
                {
                    "name": name,
                    "language": lang,
                    "exercise": ex.name,
                    "prompt": prompt,
                    "solution_paths": solution,
                    "protected_paths": protected,
                    "test_cmd": cfg["test_cmd"],
                    "source_url": f"{UPSTREAM_URL}/tree/{UPSTREAM_COMMIT}/{lang}/exercises/practice/{ex.name}",
                }
            )
            grading[name] = {
                "language": lang,
                "exercise": ex.name,
                "fixture": f"fixtures/{name}",
                "solution_paths": solution,
                "gold_extra_paths": gold_extra,
                "test_cmd": cfg["test_cmd"],
            }
        per_lang[lang] = rows
    # Round-robin the languages so a partial run still sees a mix.
    catalog: list[dict] = []
    queues = [list(per_lang[lang]) for lang in LANG_ORDER]
    while any(queues):
        for queue in queues:
            if queue:
                catalog.append(queue.pop(0))
    (work / "tasks.json").write_text(json.dumps(catalog, indent=1) + "\n")
    (work / "grading.json").write_text(json.dumps(grading, indent=1, sort_keys=True) + "\n")
    digest = catalog_digest(catalog)
    expected = (KIT / "catalog.sha256").read_text().split()[0]
    return {
        "tasks": len(catalog),
        "by_language": {lang: len(rows) for lang, rows in per_lang.items()},
        "excluded": EXCLUDED,
        "catalog_sha256": digest,
        "matches_published": digest == expected,
    }

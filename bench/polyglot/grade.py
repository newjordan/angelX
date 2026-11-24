"""Evaluator-owned grader for one polyglot task.

Rebuilds the exercise from its pristine fixture in a fresh directory named after
the exercise (the C++ CMakeLists derives its targets from that name). It carries
over ONLY the agent's solution files from the workspace, then runs the full test
suite. A task passes only when the run exits 0 AND the suite's own summary
reports passing tests and no failures. Nothing else the agent changed in its
workspace (tests, build files, new config files) can reach the grade. Aider's
runner gets the same guarantee by copying the original tests back before it grades.
"""

from __future__ import annotations

import os
import re
import shutil
import signal
import subprocess
import tempfile
import time
from pathlib import Path

from common import load_grading, tool_path, work_dir

TEST_TIMEOUT_SECS = 300
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


def summary_ok(lang: str, out: str) -> tuple[bool, str]:
    out = ANSI.sub("", out)
    if lang == "javascript":
        lines = re.findall(r"^Tests:\s+(.*)$", out, re.MULTILINE)
        if not lines:
            return False, "no jest summary"
        line = lines[-1]
        passed = re.search(r"(\d+) passed", line)
        return ("failed" not in line and bool(passed) and int(passed.group(1)) > 0), line
    if lang == "python":
        lines = [l for l in out.splitlines() if re.search(r"\d+ (passed|failed|errors?)\b", l)]
        if not lines:
            return False, "no pytest summary"
        line = lines[-1].strip("= ")
        passed = re.search(r"(\d+) passed", line)
        bad = re.search(r"\d+ (failed|errors?)\b", line)
        return (not bad and bool(passed) and int(passed.group(1)) > 0), line
    if lang == "rust":
        results = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed", out)
        if not results:
            return False, "no libtest summary"
        ok = all(r[0] == "ok" and r[2] == "0" for r in results)
        total = sum(int(r[1]) for r in results)
        return (ok and total > 0), f"{len(results)} suites, {total} passed, ok={ok}"
    if lang == "cpp":
        found = re.search(r"All tests passed \((\d+) assertions? in (\d+) test cases?\)", out)
        if found and int(found.group(2)) > 0:
            return True, found.group(0)
        return False, "no Catch2 all-passed summary"
    raise ValueError(f"unknown language {lang}")


def copy_tree(src: Path, dst: Path) -> None:
    for path in sorted(src.rglob("*")):
        target = dst / path.relative_to(src)
        if path.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)


def grade(task: str, workspace: Path, extra_paths: list[str] | None = None) -> dict:
    """Grade the solution files found in `workspace`. `extra_paths` also carries
    those files over (gold validation only)."""
    work = work_dir()
    spec = load_grading()[task]
    lang = spec["language"]
    carried = list(spec["solution_paths"]) + list(extra_paths or [])
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="polyglot-grade-") as tmp:
        box = Path(tmp) / spec["exercise"]
        box.mkdir()
        copy_tree(work / spec["fixture"], box)
        missing = []
        for rel in carried:
            src = workspace / rel
            if src.is_file():
                (box / rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(src, box / rel)
            else:
                missing.append(rel)
        if lang == "javascript":
            (box / "node_modules").symlink_to(work / "js" / "node_modules")
        env = {
            **os.environ,
            "PATH": tool_path(),
            "CI": "true",
            "PYTHONDONTWRITEBYTECODE": "1",
        }
        # Own session, so a runaway test binary dies with its whole group.
        proc = subprocess.Popen(
            ["sh", "-c", spec["test_cmd"]],
            cwd=box,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        )
        try:
            stdout, stderr = proc.communicate(timeout=TEST_TIMEOUT_SECS)
            exit_code = proc.returncode
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            stdout, stderr = proc.communicate()
            stderr += "\n[grader] test timeout"
            exit_code = 124
        out = stdout + "\n" + stderr
    ok, summary = summary_ok(lang, out)
    return {
        "task": task,
        "passed": exit_code == 0 and ok and not missing,
        "exit_code": exit_code,
        "summary": summary,
        "missing_solution_files": missing,
        "grade_s": round(time.monotonic() - started, 2),
        "tail": "\n".join(out.strip().splitlines()[-40:]),
    }

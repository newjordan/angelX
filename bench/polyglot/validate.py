"""Model-free proof of every task through the real grader.

For each task, the untouched stub must grade RED and the exercise's own reference
solution must grade GREEN. Run this once per machine before any agent run: a
task that cannot go green here is a toolchain problem, not an agent failure.
"""

from __future__ import annotations

import json
import shutil
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from common import load_grading, work_dir
from grade import grade


def validate_one(task: str, spec: dict) -> dict:
    work = work_dir()
    with tempfile.TemporaryDirectory(prefix="polyglot-validate-") as tmp:
        red_ws = Path(tmp) / "red" / spec["exercise"]
        shutil.copytree(work / spec["fixture"], red_ws)
        red = grade(task, red_ws)
        green_ws = Path(tmp) / "green" / spec["exercise"]
        shutil.copytree(work / spec["fixture"], green_ws)
        shutil.copytree(work / "gold" / task, green_ws, dirs_exist_ok=True)
        green = grade(task, green_ws, spec["gold_extra_paths"])
    return {
        "task": task,
        "ok": (not red["passed"]) and green["passed"],
        "red": red["summary"],
        "green": green["summary"],
        "green_tail": None if green["passed"] else green["tail"],
    }


def validate(tasks: list[str], jobs: int) -> int:
    grading = load_grading()
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        results = list(pool.map(lambda t: validate_one(t, grading[t]), tasks))
    (work_dir() / "validation.json").write_text(json.dumps(results, indent=1) + "\n")
    bad = [r for r in results if not r["ok"]]
    for row in bad:
        print(f"FAILED {row['task']}: red={row['red']!r} green={row['green']!r}")
        if row["green_tail"]:
            print("  " + row["green_tail"][-1200:].replace("\n", "\n  "))
    print(json.dumps({"validated": len(results), "ok": len(results) - len(bad), "failed": [r["task"] for r in bad]}))
    return 1 if bad else 0

#!/usr/bin/env bash
# The fleet line for the Lean Kernel cartridge: each problem's latest local
# evaluation, from the files evaluation/run.py writes.
exec python3 - <<'PY'
import json
from pathlib import Path

root = Path("results/local-evaluation")
problems = sorted(p for p in root.iterdir() if p.is_dir()) if root.is_dir() else []
rows, failed = [], 0
for problem in problems:
    runs = sorted(problem.glob("*.json"), key=lambda f: f.stat().st_mtime)
    if not runs:
        continue
    try:
        verdict = json.loads(runs[-1].read_text())
    except (OSError, ValueError):
        failed += 1
        continue
    rows.append({"benchmark": problem.name, "id": runs[-1].stem[:12],
                 "status": str(verdict.get("status", "unknown")), "score": None})
print(json.dumps({"benchmarks": len(problems), "failed": failed, "submissions": rows}))
PY

#!/usr/bin/env python3
"""extract_tokens_time.py <angelx-run-dir> <opencode-run-dir> <out.json> [<angelx-run-2-dir>]
— per task, in the suite's order: [name, solved, agent seconds, input tokens (uncached +
cached)], read from each run's traces.jsonl, for tokens-time.html."""
import json, sys
from pathlib import Path
RR = Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921")
order = [t["name"] for t in json.load(open(RR / "tasks-polyglot-v1.json"))]

def per_task(run):
    rows = {}
    for line in open(Path(run) / "traces.jsonl"):
        for t in json.loads(line)["traces"]:
            a = t["timing"]["agent"]
            usage = [c.get("usage") or {} for c in t.get("calls") or []]
            tokens = sum((u.get("prompt_tokens") or 0) + (u.get("cached_input_tokens") or 0) for u in usage)
            rows[t["task"]["data"]["name"]] = [t["rewards"]["technical_outcome"]["score"] == 1,
                                               round(a["end"] - a["start"], 2), tokens]
    return [[n] + rows[n] for n in order]

out = {"angelx": per_task(sys.argv[1]), "opencode": per_task(sys.argv[2])}
if len(sys.argv) > 4:
    out["angelx2"] = per_task(sys.argv[4])
json.dump(out, open(sys.argv[3], "w"))

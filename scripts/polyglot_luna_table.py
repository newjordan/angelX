#!/usr/bin/env python3
"""README-style polyglot table for the Luna cell.

Codex posts to chatgpt.com directly, so proxy call usage is usually empty.
Token and cache numbers come from angel.usage on each trace.
"""
from __future__ import annotations

import json
import statistics
import sys
from pathlib import Path


def newest() -> Path:
    root = Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921/runs/luna/angelx")
    runs = [p for p in root.glob("seed0-*") if (p / "traces.jsonl").is_file()]
    if not runs:
        raise SystemExit(f"no luna runs under {root}")
    return max(runs, key=lambda p: p.stat().st_mtime)


RUN = Path(sys.argv[1]) if len(sys.argv) > 1 else newest()


def num(value):
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    return None


def load_rows(run: Path):
    rows = []
    path = run / "traces.jsonl"
    if not path.is_file():
        return rows
    for line in path.read_text().splitlines():
        if not line.strip():
            continue
        rec = json.loads(line)
        task = (rec.get("task") or {}).get("data") or {}
        for trace in rec.get("traces") or []:
            metrics = trace.get("metrics") or {}
            calls = trace.get("calls") or []
            angel = (trace.get("info") or {}).get("angel") or {}
            usage = angel.get("usage") or {}
            score = ((trace.get("rewards") or {}).get("technical_outcome") or {}).get("score")
            rows.append(
                {
                    "idx": task.get("idx"),
                    "task": task.get("name"),
                    "lang": (task.get("name") or "?").split("-")[0],
                    "score": score,
                    "solved": score == 1,
                    "wall_s": (metrics.get("wall_ms") or 0) / 1000.0,
                    "hops": metrics.get("hops") or len(calls) or angel.get("hops") or 0,
                    "input_tokens": num(usage.get("input")),
                    "output_tokens": num(usage.get("output")),
                    "reasoning_tokens": num(usage.get("reasoning")),
                    "cached_tokens": num(usage.get("cache_read")),
                    "core": usage.get("core_complete") is True,
                    "model": angel.get("model") or (calls[0].get("model") if calls else None),
                    "effort": angel.get("reasoning_effort"),
                    "stop": angel.get("stop_reason") or angel.get("status"),
                }
            )
    return rows


def main():
    rows = load_rows(RUN)
    print(f"run={RUN}")
    print(f"scored={len(rows)}")
    if not rows:
        return
    solved = sum(1 for r in rows if r["solved"])
    walls = [r["wall_s"] for r in rows]
    hops = [r["hops"] for r in rows]
    reported = [r for r in rows if r["input_tokens"] is not None]
    missing = len(rows) - len(reported)
    inputs = sum(r["input_tokens"] or 0 for r in reported)
    cached = sum(r["cached_tokens"] or 0 for r in reported)
    outputs = sum(r["output_tokens"] or 0 for r in reported)
    reasons = sum(r["reasoning_tokens"] or 0 for r in reported)
    hit = "n/a" if not inputs else f"{100 * cached / inputs:.0f}%"
    if missing:
        hit = f"{hit} ({missing} tasks unreported)"
    print(f"model={rows[0]['model']} effort={rows[0]['effort']} usage_tasks={len(reported)}/{len(rows)}")
    print()
    print("| model | harness | solved | wall (median) | calls / task | input tokens | cache hit | output tokens | reasoning tokens |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    print(
        f"| {rows[0]['model']}, reasoning {rows[0]['effort']} | angelX | "
        f"{solved} / {len(rows)} | {statistics.median(walls):.1f} s | "
        f"{statistics.mean(hops):.1f} | {inputs:.0f} | {hit} | {outputs:.0f} | {reasons:.0f} |"
    )
    print()
    print("| language | solved | n | median wall |")
    print("|---|---:|---:|---:|")
    by = {}
    for r in rows:
        by.setdefault(r["lang"], []).append(r)
    for lang in ("js", "py", "rust", "cpp"):
        group = by.get(lang) or []
        if not group:
            continue
        print(
            f"| {lang} | {sum(1 for r in group if r['solved'])} | {len(group)} | "
            f"{statistics.median([r['wall_s'] for r in group]):.1f} s |"
        )
    failed = [r for r in rows if not r["solved"]]
    if failed:
        print()
        print("unsolved:")
        for r in failed:
            print(f"  {r['idx']} {r['task']} score={r['score']} wall={r['wall_s']:.1f}s stop={r['stop']}")


if __name__ == "__main__":
    main()

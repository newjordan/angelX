#!/usr/bin/env python3
"""README-style polyglot table from Verifiers traces."""
from __future__ import annotations
import json, statistics, sys
from pathlib import Path

def newest():
    root = Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921/runs/grok/angelx")
    runs = [p for p in root.glob("seed0-*") if (p / "traces.jsonl").is_file()]
    return max(runs, key=lambda p: p.stat().st_mtime)

RUN = Path(sys.argv[1]) if len(sys.argv) > 1 else newest()

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
            cached = 0
            usage_calls = 0
            for call in calls:
                usage = call.get("usage") or {}
                if usage:
                    usage_calls += 1
                    cached += usage.get("cached_input_tokens") or 0
            score = ((trace.get("rewards") or {}).get("technical_outcome") or {}).get("score")
            rows.append({
                "idx": task.get("idx"),
                "task": task.get("name"),
                "lang": (task.get("name") or "?").split("-")[0],
                "score": score,
                "solved": score == 1,
                "wall_s": (metrics.get("wall_ms") or 0) / 1000.0,
                "hops": metrics.get("hops") or len(calls),
                "input_tokens": metrics.get("input_tokens") or 0,
                "output_tokens": metrics.get("output_tokens") or 0,
                "reasoning_tokens": metrics.get("reasoning_tokens") or 0,
                "cached_tokens": cached,
                "usage_calls": usage_calls,
                "model": (calls[0].get("model") if calls else None),
                "effort": ((calls[0].get("sampling") or {}).get("reasoning_effort") if calls else None),
            })
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
    inputs = sum(r["input_tokens"] for r in rows)
    cached = sum(r["cached_tokens"] for r in rows)
    outputs = sum(r["output_tokens"] for r in rows)
    reasons = sum(r["reasoning_tokens"] for r in rows)
    usage_calls = sum(r["usage_calls"] for r in rows)
    missing = usage_calls == 0
    hit = "unreported" if missing or not inputs else f"{100 * cached / inputs:.0f}%"
    def tok(n):
        return "unreported" if missing else f"{n:.0f}"
    print(f"model={rows[0]['model']} effort={rows[0]['effort']}")
    print()
    print("| model | harness | solved | wall (median) | calls / task | input tokens | cache hit | output tokens | reasoning tokens |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    print(
        "| Grok 4.7, reasoning low | angelX | "
        f"{solved} / {len(rows)} | {statistics.median(walls):.1f} s | "
        f"{statistics.mean(hops):.1f} | {tok(inputs)} | {hit} | {tok(outputs)} | {tok(reasons)} |"
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
        print(f"| {lang} | {sum(1 for r in group if r['solved'])} | {len(group)} | {statistics.median([r['wall_s'] for r in group]):.1f} s |")
    failed = [r for r in rows if not r["solved"]]
    if failed:
        print()
        print("unsolved:")
        for r in failed:
            print(f"  {r['idx']} {r['task']} score={r['score']} wall={r['wall_s']:.1f}s")

if __name__ == "__main__":
    main()

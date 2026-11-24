"""Summaries and regression comparisons of polyglot runs.

A run is a run directory (holding results.jsonl and run.json), a results.jsonl,
or a baseline .jsonl from baselines/.

`compare BASE HEAD` lists every task that flipped, then compares time and tokens
on the tasks both runs attempted. It exits 1 when HEAD loses a task BASE solved
(beyond --allow-regressions) or, with --max-time-ratio, when HEAD's agent time
grows past that ratio. That makes it usable as a CI gate.
"""

from __future__ import annotations

import json
import statistics
from pathlib import Path

from common import LANG_ORDER, LANG_SHORT, read_jsonl

# Tasks whose own tests are nondeterministic. A flip here is listed, but it does
# not fail the gate unless --strict-flaky is passed.
KNOWN_FLAKY = {
    "cpp-robot-name": "random robot-name collision test; in the published runs it failed in the grader after passing in the agent's own build",
}


def load_run(path: str | Path) -> tuple[list[dict], dict]:
    path = Path(path)
    if path.is_dir():
        rows, meta_path = read_jsonl(path / "results.jsonl"), path / "run.json"
    else:
        rows, meta_path = read_jsonl(path), path.with_suffix(".json")
        if not meta_path.exists() or meta_path == path:
            meta_path = path.parent / "run.json"
    meta = json.loads(meta_path.read_text()) if meta_path.exists() else {}
    if not rows:
        raise SystemExit(f"no results in {path}")
    # A resumed or rerun task keeps its last row.
    by_task = {r["task"]: r for r in rows}
    return list(by_task.values()), meta


def pct(values: list[float], q: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    idx = min(len(ordered) - 1, max(0, round(q * (len(ordered) - 1))))
    return ordered[idx]


def stats(rows: list[dict]) -> dict:
    times = [float(r.get("agent_s") or 0) for r in rows]
    out = {
        "tasks": len(rows),
        "solved": sum(1 for r in rows if r.get("passed")),
        "timeouts": sum(1 for r in rows if r.get("timed_out")),
        "agent_min": sum(times) / 60,
        "median_s": statistics.median(times) if times else 0.0,
        "p90_s": pct(times, 0.9),
        "grade_min": sum(float(r.get("grade_s") or 0) for r in rows) / 60,
    }
    if any("calls" in r for r in rows):
        out.update(
            calls=sum(int(r.get("calls") or 0) for r in rows),
            input=sum(int(r.get("input") or 0) for r in rows),
            cached=sum(int(r.get("cached") or 0) for r in rows),
            output=sum(int(r.get("output") or 0) for r in rows),
            upstream_errors=sum(int(r.get("upstream_errors") or 0) for r in rows),
        )
    return out


def cost(s: dict, prices: dict | None) -> float | None:
    if not prices or "input" not in s:
        return None
    uncached = s["input"] - s["cached"]
    return (
        uncached * prices.get("input_uncached", 0)
        + s["cached"] * prices.get("input_cached", 0)
        + s["output"] * prices.get("output", 0)
    ) / 1e6


def summary_text(path: str | Path) -> str:
    rows, meta = load_run(path)
    s = stats(rows)
    lines = [f"# {meta.get('label') or Path(path).name}", ""]
    cfg = meta.get("config") or {}
    if cfg:
        lines.append(f"model {cfg.get('model')} via {cfg.get('upstream')}, wall {meta.get('wall_secs')} s, jobs {meta.get('jobs')}")
    elapsed = sum(seg.get("elapsed_s", 0) for seg in meta.get("segments", []))
    lines += [
        "",
        f"- solved: **{s['solved']}/{s['tasks']}**" + (f" ({s['timeouts']} hit the wall clock)" if s["timeouts"] else ""),
        f"- agent time: **{s['agent_min']:.1f} min** (sum of each task's agent process time; median {s['median_s']:.1f} s, p90 {s['p90_s']:.1f} s)",
        f"- grading time: {s['grade_min']:.1f} min (the grader's own builds and test runs, not agent time)",
    ]
    if elapsed:
        lines.append(f"- elapsed wall clock: {elapsed / 60:.1f} min (setup + agent + grading)")
    if "calls" in s:
        share = s["cached"] / s["input"] * 100 if s["input"] else 0
        lines.append(
            f"- model calls: {s['calls']} ({s['calls'] / max(1, s['tasks']):.1f}/task); input {s['input'] / 1e6:.2f} M "
            f"({share:.0f}% cached), output {s['output'] / 1e3:.0f}k"
        )
        if s["upstream_errors"]:
            lines.append(f"- upstream errors: {s['upstream_errors']} (HTTP 4xx/5xx or connection failures; see calls/)")
        dollars = cost(s, cfg.get("prices"))
        if dollars is not None:
            lines.append(f"- cost at the config's list prices: ${dollars:.2f}")
    lines += ["", "| language | solved | agent min | median s |" + (" input M |" if "calls" in s else ""),
              "|---|---:|---:|---:|" + ("---:|" if "calls" in s else "")]
    for lang in LANG_ORDER:
        part = [r for r in rows if r["language"] == lang]
        if not part:
            continue
        ls = stats(part)
        cells = f"| {LANG_SHORT[lang]} | {ls['solved']}/{ls['tasks']} | {ls['agent_min']:.1f} | {ls['median_s']:.1f} |"
        if "calls" in s:
            cells += f" {ls['input'] / 1e6:.2f} |"
        lines.append(cells)
    failed = sorted(r["task"] for r in rows if not r.get("passed"))
    if failed:
        lines += ["", "failed: " + ", ".join(failed)]
    return "\n".join(lines) + "\n"


def compare(base_path: str, head_path: str, allow_regressions: int = 0,
            max_time_ratio: float | None = None, strict_flaky: bool = False) -> int:
    base_rows, base_meta = load_run(base_path)
    head_rows, head_meta = load_run(head_path)
    base = {r["task"]: r for r in base_rows}
    head = {r["task"]: r for r in head_rows}
    common = [t for t in base if t in head]
    if not common:
        raise SystemExit("the two runs share no tasks")
    b = stats([base[t] for t in common])
    h = stats([head[t] for t in common])
    base_name = base_meta.get("label") or Path(base_path).name
    head_name = head_meta.get("label") or Path(head_path).name

    def row(label, bv, hv, fmt="{:.1f}", lower_better=True):
        delta = ""
        if isinstance(bv, (int, float)) and bv:
            change = (hv - bv) / bv * 100
            better = change < 0 if lower_better else change > 0
            delta = f"{change:+.0f}%" + (" better" if better and abs(change) >= 1 else "")
        return f"| {label} | {fmt.format(bv)} | {fmt.format(hv)} | {delta} |"

    print(f"# {len(common)} shared tasks\n\n- base: {base_name}\n- head: {head_name}\n")
    print("| | base | head | change |\n|---|---:|---:|---|")
    print(f"| solved | {b['solved']}/{b['tasks']} | {h['solved']}/{h['tasks']} | {h['solved'] - b['solved']:+d} |")
    print(row("agent min", b["agent_min"], h["agent_min"]))
    print(row("median task s", b["median_s"], h["median_s"]))
    print(row("p90 task s", b["p90_s"], h["p90_s"]))
    if "calls" in b and "calls" in h:
        print(row("model calls", b["calls"], h["calls"], "{:.0f}"))
        print(row("input M", b["input"] / 1e6, h["input"] / 1e6, "{:.2f}"))
        print(row("uncached input M", (b["input"] - b["cached"]) / 1e6, (h["input"] - h["cached"]) / 1e6, "{:.2f}"))
        print(row("output k", b["output"] / 1e3, h["output"] / 1e3, "{:.0f}"))
    both = [t for t in common if base[t].get("passed") and head[t].get("passed")]
    ratios = [
        float(head[t]["agent_s"]) / float(base[t]["agent_s"])
        for t in both if float(base[t].get("agent_s") or 0) > 0
    ]
    if ratios:
        faster = sum(1 for r in ratios if r < 1)
        print(f"\nOn the {len(ratios)} tasks both solved, head was faster on {faster}; "
              f"median per-task time ratio {statistics.median(ratios):.2f} (below 1 = faster).")

    print("\n| language | base solved / agent min | head solved / agent min |\n|---|---:|---:|")
    for lang in LANG_ORDER:
        tasks = [t for t in common if base[t]["language"] == lang]
        if tasks:
            bs, hs = stats([base[t] for t in tasks]), stats([head[t] for t in tasks])
            print(f"| {LANG_SHORT[lang]} | {bs['solved']}/{bs['tasks']} / {bs['agent_min']:.1f} | "
                  f"{hs['solved']}/{hs['tasks']} / {hs['agent_min']:.1f} |")

    lost = [t for t in common if base[t].get("passed") and not head[t].get("passed")]
    gained = [t for t in common if head[t].get("passed") and not base[t].get("passed")]
    counted = [t for t in lost if strict_flaky or t not in KNOWN_FLAKY]
    if lost:
        print("\nregressions (solved in base, failed in head):")
        for t in lost:
            note = f"  [known flaky: {KNOWN_FLAKY[t]}]" if t in KNOWN_FLAKY else ""
            r = head[t]
            why = "wall clock" if r.get("timed_out") else r.get("summary", "")
            print(f"  - {t}: {why}{note}")
    if gained:
        print("\nfixes (failed in base, solved in head): " + ", ".join(gained))
    only_base = sorted(set(base) - set(head))
    if only_base:
        print(f"\nnot in head ({len(only_base)}): " + ", ".join(only_base[:20]) + (" ..." if len(only_base) > 20 else ""))

    failing = []
    if len(counted) > allow_regressions:
        failing.append(f"{len(counted)} regression(s), {allow_regressions} allowed")
    if max_time_ratio and b["agent_min"] and h["agent_min"] / b["agent_min"] > max_time_ratio:
        failing.append(f"agent time ratio {h['agent_min'] / b['agent_min']:.2f} > {max_time_ratio}")
    print("\ngate: " + ("FAIL: " + "; ".join(failing) if failing else "pass"))
    return 1 if failing else 0

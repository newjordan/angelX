#!/usr/bin/env python3
"""Read historical Polyglot traces into a body-free cross-harness engineering board."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import statistics
import tomllib


def number(value):
    return value if isinstance(value, (int, float)) and not isinstance(value, bool) else None


def elapsed(timing):
    start, end = number(timing.get("start")), number(timing.get("end"))
    return end - start if start is not None and end is not None else None


def milliseconds(value):
    value = number(value)
    return value / 1000 if value is not None else None


def token_sum(calls, field):
    values = [number((call.get("usage") or {}).get(field)) for call in calls]
    known = [value for value in values if value is not None]
    return sum(known) if known else None


def extract_row(record, trace, run):
    info = trace.get("info") or {}
    angel = info.get("angel") or {}
    usage = angel.get("usage") or {}
    timing = angel.get("timing") or {}
    clock = (trace.get("timing") or {}).get("agent") or {}
    calls = trace.get("calls") or []
    proxy = bool(calls)
    fields = {key: token_sum(calls, wire) for key, wire in (
        ("uncached_input", "prompt_tokens"), ("cached_input", "cached_input_tokens"),
        ("output", "completion_tokens"), ("reasoning", "reasoning_tokens"),
    )}
    fields["input"] = (fields["uncached_input"] + fields["cached_input"]
                       if fields["uncached_input"] is not None and fields["cached_input"] is not None
                       else None)
    # Prefer per-call evaluator accounting for proxied peers. Direct Codex calls
    # bypass that proxy; use Angel's reporting counters instead of placeholder 0.
    if not proxy:
        reported = usage.get("reported_attempts") or {}
        for key, source in (("input", "input"), ("uncached_input", "uncached_input"),
                            ("cached_input", "cache_read"), ("output", "output"),
                            ("reasoning", "reasoning")):
            fields[key] = (number(usage.get(source)) if reported.get(source, 0) > 0 else None)
    model_s = number((clock.get("model") or {}).get("duration")) if proxy else milliseconds(timing.get("model_ms"))
    wall = elapsed(clock)
    score = number(((trace.get("rewards") or {}).get("technical_outcome") or {}).get("score"))
    verifier = info.get("heldout_verification") or {}
    exits = info.get("agent_exit") or {}
    task = (record.get("task") or {}).get("data") or {}
    return {
        "task": task.get("name"), "run": run, "score": score, "solved": score == 1,
        "wall_s": wall, "model_s": model_s, "tool_s": milliseconds(timing.get("tool_ms")),
        "non_model_s": max(0, wall - model_s) if wall is not None and model_s is not None else None,
        "verifier_s": milliseconds(verifier.get("verifier_elapsed_ms")),
        "calls": len(calls) if proxy else number(timing.get("model_calls")),
        "hops": number(angel.get("hops")), "tool_calls": number(timing.get("tool_calls")),
        **fields, "usage_source": "evaluator-proxy" if proxy else "angel-envelope",
        "usage_calls_reported": sum(bool(call.get("usage")) for call in calls) if proxy else usage.get("reported_attempts", {}).get("input"),
        "timeouts": exits.get("timed_out") is True, "exit_code": exits.get("exit_code"),
        "integrity_failure": verifier.get("integrity_pass") is False,
        "call_errors": sum(bool(call.get("error")) for call in calls),
        "trace_errors": len(trace.get("errors") or []), "stop_reason": angel.get("stop_reason"),
    }


METRICS = ("wall_s", "model_s", "tool_s", "non_model_s", "verifier_s", "calls", "hops",
           "tool_calls", "input", "cached_input", "uncached_input", "output", "reasoning")


def percentile(values, fraction):
    values = sorted(values)
    index = (len(values) - 1) * fraction
    lower = int(index)
    upper = min(lower + 1, len(values) - 1)
    return values[lower] + (values[upper] - values[lower]) * (index - lower)


def aggregate(rows):
    metrics = {}
    for key in METRICS:
        values = [row[key] for row in rows if row[key] is not None]
        metrics[key] = {"reported": len(values), "total": sum(values) if values else None,
                        "median": statistics.median(values) if values else None,
                        "p90": percentile(values, .9) if values else None,
                        "mean": statistics.mean(values) if values else None}
    cache = [row for row in rows if row["input"] is not None and row["cached_input"] is not None]
    inputs = sum(row["input"] for row in cache)
    return {
        "n": len(rows), "unique_tasks": len({row["task"] for row in rows}),
        "solved": sum(row["solved"] for row in rows),
        "unscored": sum(row["score"] is None for row in rows),
        "timeouts": sum(row["timeouts"] for row in rows),
        "integrity_failures": sum(row["integrity_failure"] for row in rows),
        "call_errors": sum(row["call_errors"] for row in rows),
        "trace_errors": sum(row["trace_errors"] for row in rows),
        "cache_ratio": sum(row["cached_input"] for row in cache) / inputs if inputs else None,
        "cache_reported_tasks": len(cache), "metrics": metrics,
        "failures": [{key: row[key] for key in ("task", "score", "wall_s", "stop_reason", "exit_code", "timeouts", "integrity_failure")}
                     for row in rows if not row["solved"]],
    }


def read_run(root, path):
    relative = str(path.parent.relative_to(root))
    config_path = path.parent / "configs/eval.toml"
    config = tomllib.loads(config_path.read_text()) if config_path.exists() else {}
    contract = {"model": config.get("model"), "sampling": config.get("sampling")}
    rows, pins = [], {}
    with path.open() as stream:
        for line in stream:
            if not line.strip():
                continue
            record = json.loads(line)
            for trace in record.get("traces") or []:
                rows.append(extract_row(record, trace, relative))
                if not pins:
                    info = trace.get("info") or {}
                    pins = {key: info.get(key) for key in ("angel_binary_contract", "opencode_binary_contract", "omp_binary_contract")
                            if info.get(key) is not None}
                    exits = info.get("agent_exit") or {}
                    contract["wall_secs"] = (exits.get("wall_cap") or {}).get("secs")
    return {"id": relative, "paths": [relative], "model_family": path.parents[2].name,
            "harness": path.parents[1].name, "contract": contract, "pins": pins,
            "rows": rows, "summary": aggregate(rows)}


def pair_cells(angel, peer, identity="task"):
    own = {row[identity]: row for row in angel["rows"]}
    other = {row[identity]: row for row in peer["rows"]}
    shared = sorted(own.keys() & other.keys())
    solved = [task for task in shared if own[task]["solved"] and other[task]["solved"]]
    deltas = []
    for task in solved:
        a, p = own[task], other[task]
        deltas.append({"task": task, **{key: a[key] - p[key] if a[key] is not None and p[key] is not None else None
                                      for key in METRICS}})
    delta_summary = {}
    for key in METRICS:
        values = [row[key] for row in deltas if row[key] is not None]
        delta_summary[key] = {"n": len(values), "total": sum(values) if values else None,
                              "median": statistics.median(values) if values else None}
    return {"angel": angel["id"], "peer": peer["id"], "same_recorded_settings": angel["contract"] == peer["contract"],
            "angel_contract": angel["contract"], "peer_contract": peer["contract"],
            "shared": len(shared), "both_solved": len(solved),
            "angel_only_solved": [task for task in shared if own[task]["solved"] and not other[task]["solved"]],
            "peer_only_solved": [task for task in shared if other[task]["solved"] and not own[task]["solved"]],
            "delta_angel_minus_peer": delta_summary,
            "largest_wall_gaps": sorted(deltas, key=lambda row: row["wall_s"] or 0, reverse=True)[:15]}


def display(value, digits=1):
    return "—" if value is None else f"{value:,.{digits}f}"


def markdown(result):
    lines = ["# Historical Polyglot harness engineering board", "",
             "Raw trace summaries. Unknown telemetry stays unknown. Complete and partial cohorts are labeled; quarantined, aborted and smoke runs are excluded.", "",
             "| Model / cohort | Harness | Solved | Wall median / p90 s | Calls/task | Input / uncached M | Cache | Output k | Model / non-model s total |",
             "|---|---|---:|---:|---:|---:|---:|---:|---:|"]
    for cell in result["primary"]:
        s, m = cell["summary"], cell["summary"]["metrics"]
        ratio = s["cache_ratio"]
        cache_display = f"{display(ratio * 100)}%" if ratio is not None else "—"
        scale = lambda key, divisor: m[key]["total"] / divisor if m[key]["total"] is not None else None
        lines.append(f"| {cell['model_family']} · {cell['id'].split('/')[0]} | {cell['harness']} | {s['solved']}/{s['n']} | "
                     f"{display(m['wall_s']['median'])} / {display(m['wall_s']['p90'])} | {display(m['calls']['mean'])} | "
                     f"{display(scale('input', 1e6), 2)} / {display(scale('uncached_input', 1e6), 2)} | "
                     f"{cache_display} | {display(scale('output', 1000), 1)} | "
                     f"{display(m['model_s']['total'])} / {display(m['non_model_s']['total'])} |")
    lines += ["", "## Matched tasks solved by both harnesses", "",
              "Deltas are angelX minus the peer. Negative wall deltas favor angelX. Same recorded settings means evaluator model ID, sampling configuration and wall cap match. It does not establish identical native requests, serving builds or output-limit enforcement; runs were not simultaneous.", "",
              "| Model | Peer | Same recorded settings | Shared / both solved | Median wall delta s | Total model delta s | Total non-model delta s |",
              "|---|---|---|---:|---:|---:|---:|"]
    for pair in result["pairings"]:
        d = pair["delta_angel_minus_peer"]
        lines.append(f"| {pair['angel'].split('/')[1]} | {pair['peer'].split('/')[2]} | {pair['same_recorded_settings']} | {pair['shared']} / {pair['both_solved']} | "
                     f"{display(d['wall_s']['median'])} | {display(d['model_s']['total'])} | {display(d['non_model_s']['total'])} |")
    lines += ["", "## Provenance and gaps", "", "Primary cohorts:"]
    for cell in result["primary"]:
        lines.append(f"- `{cell['id']}`: `{json.dumps(cell['contract'], sort_keys=True)}`")
    lines += ["", "OpenCode and omp do not expose Angel's separate tool timer; their non-model residual includes tool work and harness overhead. Angel's Qwen proxy calls carry no usage fields, so its token/cache entries stay unknown; the Qwen peers do report usage. The Muse contributor model ID differs from the angelX Muse model ID. The JSON retains every eligible historical run, explicit failures, reporting coverage, source pins and per-task metrics."]
    return "\n".join(lines) + "\n"


def repair_history(root):
    """Keep the earlier 18-task, three-repeat corpus separate from Polyglot."""
    cells = [read_run(root, path) for path in sorted((root / "runs").glob("*/*/seed*/traces.jsonl"))]
    primary = []
    for family in ("deepseek", "glm"):
        for harness in ("angelx", "opencode", "omp"):
            parts = [cell for cell in cells if cell["model_family"] == family and cell["harness"] == harness]
            if not parts:
                continue
            if any(cell["contract"] != parts[0]["contract"] for cell in parts):
                raise ValueError(f"Repair {family}/{harness} settings drift")
            rows = [{**row, "attempt": row["task"] + "@" + cell["id"].split("/")[-1].split("-")[0]}
                    for cell in parts for row in cell["rows"]]
            if len({row["attempt"] for row in rows}) != len(rows):
                raise ValueError(f"Repair {family}/{harness} duplicate task/repetition")
            primary.append({**parts[0], "id": f"repair/{family}/{harness}/three-repetitions",
                            "paths": [path for cell in parts for path in cell["paths"]],
                            "pins_by_run": {cell["id"]: cell["pins"] for cell in parts},
                            "rows": rows, "summary": aggregate(rows)})
    pairs = [pair_cells(angel, peer, "attempt") for angel in primary for peer in primary
             if angel["harness"] == "angelx" and peer["harness"] != "angelx"
             and angel["model_family"] == peer["model_family"]]
    return {"schema": "angelx-repair-history/v1", "root": str(root), "primary": primary,
            "pairings": pairs, "all_runs": cells, "pairing_unit": "task and repetition index"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repair-root", type=Path,
                        help="Also summarize the separate 18-task repeated repair benchmark")
    args = parser.parse_args()
    root = args.root.resolve()
    cells = []
    for directory in ("runs", "runs-qwen", "runs-compact-full"):
        for path in sorted((root / directory).glob("*/*/seed*/traces.jsonl")):
            cells.append(read_run(root, path))
    # Qwen's rotation ran disjoint chunks, one attempt per task/harness. Combine
    # only matching settings and reject overlaps rather than selecting successes.
    for harness in ("angelx", "opencode", "omp"):
        parts = [cell for cell in cells if cell["id"].startswith(f"runs-qwen/qwen/{harness}/")]
        if not parts:
            continue
        if any(cell["contract"] != parts[0]["contract"] for cell in parts):
            raise ValueError(f"Qwen {harness} chunk settings drift")
        rows = [row for cell in parts for row in cell["rows"]]
        duplicate = [name for name, n in Counter(row["task"] for row in rows).items() if n > 1]
        if duplicate:
            raise ValueError(f"Qwen {harness} duplicate task attempts: {duplicate}")
        cells.append({**parts[0], "id": f"runs-qwen/qwen/{harness}/combined-disjoint-chunks",
                      "paths": [p for cell in parts for p in cell["paths"]], "rows": rows, "summary": aggregate(rows)})
    primary = []
    for family in ("deepseek", "glm", "grok", "luna", "muse", "qwen"):
        for harness in ("angelx", "opencode", "omp", "hermes", "primebash"):
            candidates = [cell for cell in cells if cell["model_family"] == family and cell["harness"] == harness
                          and (cell["id"].startswith("runs/") or cell["id"].endswith("combined-disjoint-chunks"))]
            if candidates:
                full = [cell for cell in candidates if cell["summary"]["n"] == 136]
                primary.append(max(full or candidates, key=lambda cell: (cell["summary"]["n"] if not full else 0, cell["id"])))
        if family == "qwen":
            primary += [cell for cell in cells if cell["id"].startswith("runs-compact-full/")]
    pairings = []
    for angel in primary:
        if angel["harness"] != "angelx":
            continue
        for peer in primary:
            if peer["model_family"] == angel["model_family"] and peer["harness"] != "angelx":
                pairings.append(pair_cells(angel, peer))
    result = {"schema": "angelx-polyglot-history/v1", "root": str(root), "primary": primary,
              "pairings": pairings, "all_runs": cells,
              "exclusions": ["QUARANTINE-*", "archive-*", "smoke-*", "aborted-*", "calibration", "escalation", "selective reruns"]}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    args.output.with_suffix(".md").write_text(markdown(result))
    print(f"eligible_runs={len(cells)} primary_cells={len(primary)} pairings={len(pairings)} output={args.output}")
    if args.repair_root:
        repair = repair_history(args.repair_root.resolve())
        repair_output = args.output.with_name(args.output.stem + "-repair.json")
        repair_output.write_text(json.dumps(repair, indent=2) + "\n")
        body = markdown(repair).replace("# Historical Polyglot harness engineering board",
                                        "# Historical repository-repair harness engineering board")
        body = body.replace("Raw trace summaries.", "Separate 18-task corpus with three repetitions; pairing uses task and repetition index. Raw trace summaries.")
        repair_output.with_suffix(".md").write_text(body)
        print(f"repair_runs={len(repair['all_runs'])} output={repair_output}")


if __name__ == "__main__":
    main()

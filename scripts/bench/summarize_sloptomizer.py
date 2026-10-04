#!/usr/bin/env python3
"""Summarize the four-cell Sloptomizer pilot without copying model prose.

Preserves incomplete cohorts and missing telemetry. Pair each on/off attempt
within its own round; only then aggregate the two rounds. No provider calls.
"""
from __future__ import annotations

import argparse
from collections import Counter
import importlib.util
import json
from pathlib import Path


spec = importlib.util.spec_from_file_location(
    "polyglot_summary", Path(__file__).resolve().parents[1] / "summarize-polyglot-loop.py"
)
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)

ORDER = ("r1-off", "r1-on", "r2-on", "r2-off")
CONTRACT = (
    "provider", "route", "model", "effort", "endpoint", "config_sha256",
    "selected_rows_sha256", "catalog_sha256", "grading_sha256", "grade_script_sha256",
    "task_names", "wall_secs", "max_hops", "max_tokens", "attempts", "concurrency",
    "binary", "sandbox_sha256", "resource_source_identity", "resource_files_sha256",
    "adapter_files", "fixture_files", "gold_files", "evaluator_files_sha256", "runner_sha256",
)


def texts(content):
    if isinstance(content, str):
        yield content
    elif isinstance(content, list):
        for part in content:
            if isinstance(part, dict) and isinstance(part.get("text"), str):
                yield part["text"]


def prompt_ancestry(trace):
    """Bind input nodes to recorded provider calls, excluding orphan messages.

    A trace can retain nodes that were never sent. Missing call/parent receipts
    make exposure unknown rather than turning an absent card into a measured 0.
    """
    nodes, calls = trace.get("nodes"), trace.get("calls")
    if not isinstance(nodes, list) or not isinstance(calls, list):
        return set(), False
    angel = (trace.get("info") or {}).get("angel") or {}
    model_calls = (angel.get("timing") or {}).get("model_calls")
    complete = type(model_calls) is int and model_calls == len(calls)
    observed = set()
    for call in calls:
        index = call.get("node") if isinstance(call, dict) else None
        seen = set()
        if (type(index) is not int or not 0 <= index < len(nodes)
                or not isinstance(nodes[index], dict) or nodes[index].get("sampled") is not True):
            complete = False
            continue
        while index is not None:
            if (type(index) is not int or not 0 <= index < len(nodes) or index in seen
                    or not isinstance(nodes[index], dict)):
                complete = False
                break
            seen.add(index)
            node = nodes[index]
            if node.get("sampled") is False:
                observed.add(index)
            index = node.get("parent")
    return observed, complete


def delivery(trace):
    nodes = trace.get("nodes")
    input_indices, input_complete = prompt_ancestry(trace)
    cards = set()
    if isinstance(nodes, list):
        for index in input_indices:
            node = nodes[index]
            message = node.get("message") or {}
            if message.get("role") not in ("system", "developer", "user"):
                continue
            for text in texts(message.get("content")):
                if text.startswith("⚠⡫"):
                    cards.add(text)
    stamps = Counter()
    for card in cards:
        first = card.splitlines()[0]
        for stamp, name in (
            ("⡫⠁", "context"), ("⡫⠃", "identical_receipt"),
            ("⡫⠉", "contrast"), ("⡫⠙", "inconclusive"),
            ("⡫⠑", "unavailable"), ("⡫⠋", "handoff"), ("⡫⠛", "failure_run"),
        ):
            if stamp in first:
                stamps[name] += 1
    angel = (trace.get("info") or {}).get("angel") or {}
    tools = angel.get("tools")
    counts = Counter(tool.get("verify") for tool in tools) if isinstance(tools, list) else None
    usage = angel.get("usage") or {}
    values, _, coverage = shared.measured_usage(usage)
    inp, cached = values["input_tokens"], values["cached_input_tokens"]
    included = (usage.get("cache_convention_attempts") or {}).get("included")
    uncached = None
    if (coverage["input_tokens"]["complete"] and coverage["cached_input_tokens"]["complete"]
            and included == usage.get("attempts") and 0 <= cached <= inp):
        uncached = inp - cached
    return {
        "prompt_nodes_present": isinstance(nodes, list),
        "input_call_coverage_complete": input_complete,
        "unique_cards": len(cards) if input_complete else None,
        "observed_unique_cards": len(cards),
        "card_routes": dict(stamps) if input_complete else None,
        "typed_checks": ({key: counts[key] for key in ("passed", "failed", "inconclusive")}
                         if counts is not None else None),
        "derived_uncached_input_tokens": uncached,
        "output_includes_reasoning": (
            (usage.get("reasoning_convention_attempts") or {}).get("included") == usage.get("attempts")
            if coverage["output_tokens"]["complete"] and coverage["reasoning_tokens"]["complete"]
            else None
        ),
    }


def outcomes(rows):
    """Keep adverse outcomes and missing receipts visible, without dropping rows."""
    return {
        "integrity_pass": sum(r["integrity_pass"] is True for r in rows),
        "integrity_fail": sum(r["integrity_pass"] is False for r in rows),
        "integrity_unknown": sum(r["integrity_pass"] is None for r in rows),
        "timed_out": sum(r["timed_out"] is True for r in rows),
        "timeout_unknown": sum(r["timed_out"] is None for r in rows),
        "nonzero_exit": sum(type(r["exit_code"]) is int and r["exit_code"] != 0 for r in rows),
        "exit_unknown": sum(r["exit_code"] is None for r in rows),
        "trace_error_attempts": sum(r["trace_error_count"] > 0 for r in rows),
        "trace_completed": sum((r.get("trace_status") or {}).get("completed") is True for r in rows),
        "trace_ok": sum((r.get("trace_status") or {}).get("ok") is True for r in rows),
    }


def summarize(root):
    manifest = json.loads((root / "manifest.json").read_text())
    issues = []
    if manifest.get("order") != list(ORDER) or manifest.get("mode") != "execute":
        issues.append("expected an executed off/on, on/off pilot")
    cells, raw_manifests = {}, {}
    for name in ORDER:
        path = root / name
        if not (path / "manifest.json").exists():
            issues.append(f"{name}: cell has not started")
            continue
        cm = json.loads((path / "manifest.json").read_text())
        raw_manifests[name] = cm
        for field in CONTRACT:
            if manifest.get(field) is None or cm.get(field) != manifest[field]:
                issues.append(f"{name}: missing or mismatched contract field {field}")
        if cm.get("exit_code") != 0:
            issues.append(f"{name}: evaluator is unfinished or failed")
        env = cm.get("harness_env") or {}
        if env.get("ANGEL_SLOPTOMIZER_LIVE") != ("1" if name.endswith("-on") else "0"):
            issues.append(f"{name}: live-memory flag does not match arm")
        expected_wire = str(path / "run/wire")
        if env.get("ANGEL_WIRE_LOG_DIR") != expected_wire:
            issues.append(f"{name}: wire output is not isolated to this cell")
        cell = shared.load_cell(path)
        if cell["coverage"]["cohort_complete"] is not True:
            issues.append(f"{name}: cohort is incomplete")
        traces = path / "run/traces.jsonl"
        features, trace_status = {}, {}
        for line, text in enumerate(traces.read_text().splitlines() if traces.exists() else [], 1):
            if not text.strip():
                continue
            record = json.loads(text)
            for attempt, trace in enumerate(record.get("traces") or [], 1):
                features[line, attempt] = delivery(trace)
                trace_status[line, attempt] = {
                    "completed": trace.get("is_completed"), "ok": trace.get("ok"),
                    "recorded_calls": len(trace["calls"]) if isinstance(trace.get("calls"), list) else None,
                    "sampling_matches": True if isinstance(trace.get("calls"), list) else None,
                }
                for call in trace.get("calls") or []:
                    expected = {"temperature": 0.0, "reasoning_effort": manifest["effort"],
                                "max_tokens": manifest["max_tokens"]}
                    if call.get("model") != manifest["model"] or any(
                        (call.get("sampling") or {}).get(k) != v for k, v in expected.items()
                    ):
                        trace_status[line, attempt]["sampling_matches"] = False
                        issues.append(f"{name}: observed model/sampling differs from configured pilot")
        for row in cell["rows"]:
            row["delivery"] = features.get((row["line"], row["attempt"]))
            row["trace_status"] = trace_status.get((row["line"], row["attempt"]))
            if name.endswith("-off") and (row["delivery"] or {}).get("observed_unique_cards", 0):
                issues.append(f"{name}: disabled arm contains observed Sloptomizer input cards")
            if row["score"] is None:
                issues.append(f"{name}: at least one attempt has no recorded score")
            if row["integrity_pass"] is not True:
                issues.append(f"{name}: at least one grading-integrity receipt is missing or failed")
        cell["outcomes"] = outcomes(cell["rows"])
        cells[name] = cell
    common_envs = [{k: v for k, v in m["harness_env"].items()
                   if k not in ("ANGEL_SLOPTOMIZER_LIVE", "ANGEL_WIRE_LOG_DIR")}
                  for m in raw_manifests.values()]
    if common_envs and any(e != common_envs[0] for e in common_envs[1:]):
        issues.append("non-experimental harness settings differ across cells")
    comparisons = []
    for round_name in ("r1", "r2"):
        off, on = f"{round_name}-off", f"{round_name}-on"
        if off in cells and on in cells:
            pair = shared.paired(cells[off], cells[on])
            pair["round"] = round_name
            # The full contract check above accounts for exactly the live flag
            # and the per-cell wire path; retain the raw helper's mismatch list.
            expected_difference = {"ANGEL_SLOPTOMIZER_LIVE", "ANGEL_WIRE_LOG_DIR"}
            envs = [{k: v for k, v in raw_manifests[n]["harness_env"].items()
                     if k not in expected_difference} for n in (off, on)]
            pair["only_expected_contract_differences"] = (
                pair["contract_mismatches"] == ["harness_env"]
                and not pair["contract_unknown_fields"] and envs[0] == envs[1]
                and raw_manifests[off]["harness_env"].get("ANGEL_SLOPTOMIZER_LIVE") == "0"
                and raw_manifests[on]["harness_env"].get("ANGEL_SLOPTOMIZER_LIVE") == "1"
                and all(raw_manifests[n]["harness_env"].get("ANGEL_WIRE_LOG_DIR")
                        == str(root / n / "run/wire") for n in (off, on))
                and all(raw_manifests[n].get(k) == manifest.get(k)
                        for n in (off, on) for k in CONTRACT)
            )
            comparisons.append(pair)
    arms = {}
    for arm in ("off", "on"):
        rows = [row for name, cell in cells.items() if name.endswith("-" + arm)
                for row in cell["rows"]]
        derived = [(r.get("delivery") or {}).get("derived_uncached_input_tokens") for r in rows]
        arms[arm] = {
            **shared.aggregate(rows),
            "outcomes": outcomes(rows),
            "attempts_with_cards": sum((r.get("delivery") or {}).get("unique_cards", 0) > 0
                                       for r in rows if (r.get("delivery") or {}).get("unique_cards") is not None),
            "card_coverage_attempts": sum((r.get("delivery") or {}).get("unique_cards") is not None
                                          for r in rows),
            "derived_uncached_input_tokens": (sum(derived) if derived and all(v is not None for v in derived)
                                               else None),
        }
    return {"schema": "angel.sloptomizer-summary/v1", "complete": not issues,
            "issues": sorted(set(issues)), "source_manifest": str(root / "manifest.json"),
            "scope": "Descriptive six-task pilot, two paired rounds; no statistical efficacy claim.",
            "card_scope": "Unique unsampled cards in recorded provider-call ancestry; unknown when input/call coverage is incomplete; no prose retained.",
            "uncached_scope": "Derived input minus cache_read only with complete included-cache receipts.",
            "arms": arms, "comparisons": comparisons, "cells": cells}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = summarize(args.root.resolve(strict=True))
    rendered = json.dumps(result, indent=2, allow_nan=False) + "\n"
    if args.output:
        args.output.write_text(rendered)
    else:
        print(rendered, end="")


if __name__ == "__main__":
    main()

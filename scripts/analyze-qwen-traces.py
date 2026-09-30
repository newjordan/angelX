#!/usr/bin/env python3
"""Summarize request/response sizes without exporting Qwen trace message bodies."""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import statistics


def size(value):
    if value is None:
        return 0
    return len(value) if isinstance(value, str) else len(json.dumps(value, ensure_ascii=False))


def ancestors(nodes, index):
    visited, messages = set(), []
    while index is not None:
        if not valid_index(nodes, index):
            raise ValueError("Trace graph contains an invalid ancestor reference")
        if index in visited:
            raise ValueError("Trace graph contains a cycle")
        visited.add(index)
        node = nodes[index]
        if not isinstance(node, dict) or not isinstance(node.get("message"), dict):
            raise ValueError("Trace graph contains an invalid ancestor message")
        messages.append(node["message"])
        index = node.get("parent")
    return messages[::-1]


def valid_index(nodes, index):
    return isinstance(index, int) and not isinstance(index, bool) and 0 <= index < len(nodes)


REFERENCE_COVERAGE = ("missing_response_calls", "invalid_response_calls", "invalid_ancestry_calls")


def trace_shapes(trace):
    """Count calls[].node only: nodes[] also contains replayed branch history."""
    calls, settings, tools = [], Counter(), Counter()
    attempted = trace.get("calls") or []
    nodes = trace.get("nodes") or []
    coverage = dict.fromkeys(REFERENCE_COVERAGE, 0)
    for call in attempted:
        settings[json.dumps(call.get("sampling"), sort_keys=True)] += 1
        index = call.get("node")
        if index is None:
            coverage["missing_response_calls"] += 1
            continue
        if not valid_index(nodes, index):
            coverage["invalid_response_calls"] += 1
            continue
        node = nodes[index]
        message = node.get("message") if isinstance(node, dict) else None
        if not isinstance(message, dict) or message.get("role") != "assistant":
            coverage["invalid_response_calls"] += 1
            continue
        try:
            history = ancestors(nodes, node.get("parent"))
        except ValueError:
            coverage["invalid_ancestry_calls"] += 1
            continue
        emitted = message.get("tool_calls") or []
        for tool in emitted:
            tools[tool.get("name") or (tool.get("function") or {}).get("name")] += 1
        row = {
            "wall_s": call["time"]["end"] - call["time"]["start"],
            "content_chars": size(message.get("content")),
            "reasoning_chars": size(message.get("reasoning_content")),
            "tool_argument_chars": sum(size(tool.get("arguments") if "arguments" in tool else
                                             (tool.get("function") or {}).get("arguments")) for tool in emitted),
            "request_message_chars": sum(size(item) for item in history),
            "system_chars": sum(size(item.get("content")) for item in history if item.get("role") == "system"),
            "tools_schema_chars": size(trace.get("tools")),
            "history_reasoning_chars": sum(size(item.get("reasoning_content")) for item in history),
            "usage_reported": bool(call.get("usage")),
        }
        row["generation_chars"] = row["content_chars"] + row["reasoning_chars"] + row["tool_argument_chars"]
        calls.append(row)
    return {"calls": calls, "attempted_calls": len(attempted),
            "unobserved_response_calls": len(attempted) - len(calls), **coverage}, settings, tools


def aggregate(rows):
    calls = [call for row in rows for call in row["calls"]]
    metrics = {}
    for key in calls[0] if calls else []:
        values = [call[key] for call in calls]
        metrics[key] = {"sum": sum(values), "median": statistics.median(values), "mean": statistics.mean(values)}
    return {"tasks": len(rows), "attempted_calls": sum(row["attempted_calls"] for row in rows),
            "observed_response_calls": len(calls),
            "unobserved_response_calls": sum(row["unobserved_response_calls"] for row in rows),
            **{key: sum(row[key] for row in rows) for key in REFERENCE_COVERAGE},
            "metrics": metrics}


def read_cohort(paths):
    rows, settings, tools = [], Counter(), Counter()
    for path in sorted(paths):
        with path.open() as stream:
            for line in stream:
                if not line.strip():
                    continue
                record = json.loads(line)
                for trace in record.get("traces") or []:
                    shape, sampling, tool_counts = trace_shapes(trace)
                    rows.append({"task": record["task"]["data"]["name"],
                                 "score": ((trace.get("rewards") or {}).get("technical_outcome") or {}).get("score"),
                                 **shape})
                    settings.update(sampling)
                    tools.update(tool_counts)
    if len({row["task"] for row in rows}) != len(rows):
        raise ValueError("Cohort contains repeated task attempts")
    return {"summary": aggregate(rows), "sampling_counts": dict(settings), "tool_call_counts": dict(tools), "rows": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    cohorts = {"angel-original": "runs-qwen/qwen/angelx", "angel-compact": "runs-compact-full/qwen/angelx",
               "opencode": "runs-qwen/qwen/opencode", "omp": "runs-qwen/qwen/omp"}
    result = {label: read_cohort((args.root / directory).glob("seed*/traces.jsonl")) for label, directory in cohorts.items()}
    own, peer = result["angel-compact"]["rows"], result["opencode"]["rows"]
    shared = {row["task"] for row in own if row["score"] == 1} & {row["task"] for row in peer if row["score"] == 1}
    result["jointly_solved"] = {"tasks": len(shared),
        "angel-compact": aggregate([row for row in own if row["task"] in shared]),
        "opencode": aggregate([row for row in peer if row["task"] in shared])}
    result["methodology"] = "Character counts from normalized evaluator nodes, not token counts or raw HTTP sizes; only sampled assistant responses with valid acyclic ancestry are measured, with omitted responses, invalid response references/messages and invalid ancestry counted separately. Tool schemas are serialized once per request. Historical reasoning counts replayed input, not additional model generation."
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(f"jointly_solved={len(shared)} output={args.output}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""run_summary.py out/<label>.jsonl... — one line per run: requests (turns),
ledger reads (turns spent decoding stamps), tool calls by name, provider
tokens (prompt / cached / completion, when the provider reports them) and
wall time. Reads stub_model.py and proxy_log.py logs alike."""
import argparse
import collections
import importlib.util
import json
from pathlib import Path


spec = importlib.util.spec_from_file_location("book_summary_receipt_usage", Path(__file__).with_name("receipt_usage.py"))
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)


def summary(path):
    rows = receipt.read_receipts(path)
    tools = collections.Counter()
    reads = 0
    body = rows[-1].get("body") if rows and isinstance(rows[-1].get("body"), dict) else {}
    calls = []
    for message in body.get("messages", []):
        calls += [call.get("function", {}) for call in message.get("tool_calls") or []]
    # Messages API: each call is a `tool_use` block in an assistant message.
    for message in body.get("messages", []):
        for block in message.get("content") if isinstance(message.get("content"), list) else []:
            if isinstance(block, dict) and block.get("type") == "tool_use":
                calls.append({"name": block.get("name"), "arguments": json.dumps(block.get("input"))})
    # Responses API: the history is `input`, each call a `function_call` item.
    calls += [item for item in body.get("input", []) if isinstance(item, dict)
              and item.get("type") == "function_call"]
    for fn in calls:
        tools[fn.get("name", "?")] += 1
        if "ledger://" in fn.get("arguments", ""):
            reads += 1
    measured = receipt.usage_summary(rows)
    start = receipt.number(rows[0].get("t")) if rows else None
    end = receipt.number(rows[-1].get("done", rows[-1].get("t"))) if rows else None
    wall = end - start if start is not None and end is not None and end >= start else None

    def metric(key):
        value = measured[key]
        if value is not None:
            return str(value)
        coverage = measured["usage_metric_coverage"][key]
        subtotal = measured["usage_reported_subtotals"][key]
        return f"unknown[reported_subtotal={subtotal};coverage={coverage['reported']}/{coverage['attempts']}]"

    wall_text = f"{wall:.1f}s" if wall is not None else "unknown"
    return (f"{path}: requests={len(rows)} ledger_reads={reads} prompt={metric('prompt')} "
            f"cached={metric('cached')} completion={metric('completion')} wall={wall_text} tools={dict(tools)}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args(argv)
    try:
        summaries = [summary(path) for path in args.paths]
    except (ValueError, OSError) as error:
        parser.error(str(error))
    for text in summaries:
        print(text)


if __name__ == "__main__":
    main()

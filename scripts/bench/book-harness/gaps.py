"""Between-request wall gaps grouped by the observed next action.

These intervals include response latency, tool execution and request building;
proxy gaps are not measurements of harness-only time. Missing action or clock
observations remain explicit. Both Chat Completions and Responses are supported.
"""
import argparse
import collections
import importlib.util
import json
from pathlib import Path
import statistics


spec = importlib.util.spec_from_file_location("gap_receipt", Path(__file__).with_name("receipt_usage.py"))
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)
UNKNOWN = "(unknown)"


def assistant_action(message):
    calls = message.get("tool_calls")
    if calls:
        if not isinstance(calls, list) or not isinstance(calls[0], dict):
            return UNKNOWN
        function = calls[0].get("function")
        name = function.get("name") if isinstance(function, dict) else None
        return name if isinstance(name, str) and name else UNKNOWN
    if calls is not None and not isinstance(calls, list):
        return UNKNOWN
    content = message.get("content")
    if isinstance(content, str) or isinstance(content, list) or calls == []:
        return "final"
    return UNKNOWN


def action_of(rows, index):
    """The first tool in the newest observed reply, or an explicit answer."""
    row = rows[index]
    if "action" in row:
        action = row["action"]
        if action is None or action == []:
            return "final"
        if isinstance(action, (list, tuple)) and action and isinstance(action[0], str) and action[0]:
            return action[0]
        return UNKNOWN
    if index + 1 >= len(rows) or not isinstance(rows[index + 1].get("body"), dict):
        return UNKNOWN
    body = rows[index + 1]["body"]
    previous = row.get("body") if isinstance(row.get("body"), dict) else {}
    messages = body.get("messages")
    if isinstance(messages, list):
        old = previous.get("messages")
        for position in range(len(messages) - 1, -1, -1):
            message = messages[position]
            if isinstance(message, dict) and message.get("role") == "assistant":
                if isinstance(old, list) and position < len(old) and old[position] == message:
                    return UNKNOWN
                return assistant_action(message)
    items = body.get("input")
    if isinstance(items, list):
        # A Responses tool batch has separate function_call items. Choose its
        # first call, preserving the original first-call grouping convention.
        batch = []
        for position in range(len(items) - 1, -1, -1):
            item = items[position]
            if not isinstance(item, dict):
                return UNKNOWN
            if item.get("type") == "function_call":
                batch.append((position, item))
            elif batch:
                break
            elif item.get("role") == "assistant":
                old = previous.get("input")
                if isinstance(old, list) and position < len(old) and old[position] == item:
                    return UNKNOWN
                return assistant_action(item)
        if batch:
            old = previous.get("input")
            if isinstance(old, list) and all(position < len(old) and old[position] == item
                                             for position, item in batch):
                return UNKNOWN
            name = batch[-1][1].get("name")
            return name if isinstance(name, str) and name else UNKNOWN
    return UNKNOWN


def analyze(rows):
    groups = collections.defaultdict(list)
    for index, (before, after) in enumerate(zip(rows, rows[1:])):
        start, end = receipt.number(before.get("t")), receipt.number(after.get("t"))
        gap = receipt.number((end - start) * 1000) if start is not None and end is not None and end >= start else None
        groups[action_of(rows, index)].append(gap)
    first = receipt.number(rows[0].get("t")) if len(rows) >= 2 else None
    last = receipt.number(rows[-1].get("t")) if len(rows) >= 2 else None
    total = receipt.number(last - first) if first is not None and last is not None and last >= first else None
    return {"requests": len(rows), "first_to_last_seconds": total, "groups": dict(groups),
            "scope": "between request arrivals, including response latency, tool execution and request building"}


def render(path, result):
    total = result["first_to_last_seconds"]
    duration = f"{total:.1f}s" if total is not None else "—"
    lines = [f"{path}: {result['requests']} requests, {duration} first→last", "  scope: " + result["scope"]]
    for name, values in sorted(result["groups"].items(), key=lambda item: -sum(v for v in item[1] if v is not None)):
        observed = sorted(value for value in values if value is not None)
        complete = len(observed) == len(values)
        median_value = receipt.number(statistics.median(observed)) if complete else None
        median = f"{median_value:7.1f}ms" if median_value is not None else "      —"
        p90 = f"{observed[int(len(observed) * .9)]:7.1f}ms" if complete else "      —"
        subtotal = receipt.number(sum(value / 1000 for value in observed)) if observed else None
        summed = f"{subtotal:6.1f}s" if complete and subtotal is not None else "     —"
        reported = f"{subtotal:.1f}s" if subtotal is not None else "—"
        lines.append(f"  {name:12s} n={len(values):4d} median={median} p90={p90} sum={summed} "
                     f"[clocked={len(observed)}/{len(values)}; reported_sum={reported}]")
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", type=Path)
    parser.add_argument("--audit-json", action="store_true")
    args = parser.parse_args(argv)
    results = []
    try:
        for path in args.paths:
            with path.open() as source:
                rows = [json.loads(line) for line in source if line.strip()]
            if any(not isinstance(row, dict) for row in rows):
                raise ValueError(f"capture rows must be objects: {path}")
            results.append({"path": str(path), **analyze(rows)})
    except (OSError, ValueError) as error:
        parser.error(str(error))
    if args.audit_json:
        print(json.dumps(results, indent=1, allow_nan=False))
    else:
        for result in results:
            print(render(result["path"], result), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

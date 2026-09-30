"""Per-hop harness time from a stub log: the gap between one request's arrival
and the next, grouped by the tool the stub asked for in between (the stub
answers at once, so the gap is tool execution plus request building)."""
import json, sys, statistics as st
from collections import defaultdict


def action_of(rows, i):
    """The tool the model called after request i: the stub logs it; for a
    proxy_log.py run it is the last assistant tool call in request i+1."""
    row = rows[i]
    if "action" in row:
        return row["action"][0] if row["action"] else "final"
    if i + 1 < len(rows):
        for message in reversed(rows[i + 1]["body"].get("messages", [])):
            if message.get("role") == "assistant" and message.get("tool_calls"):
                return message["tool_calls"][0]["function"]["name"]
    return "final"

for path in sys.argv[1:]:
    rows = [json.loads(l) for l in open(path)]
    by = defaultdict(list)
    for i, (a, b) in enumerate(zip(rows, rows[1:])):
        name = action_of(rows, i)
        by[name].append((b["t"] - a["t"]) * 1000)
    total = (rows[-1]["t"] - rows[0]["t"])
    print(f"{path}: {len(rows)} requests, {total:.1f}s first→last")
    for name, gaps in sorted(by.items(), key=lambda kv: -sum(kv[1])):
        print(f"  {name:12s} n={len(gaps):4d} median={st.median(gaps):7.1f}ms p90={sorted(gaps)[int(len(gaps)*0.9)]:7.1f}ms sum={sum(gaps)/1000:6.1f}s")

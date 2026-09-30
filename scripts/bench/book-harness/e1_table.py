#!/usr/bin/env python3
"""e1_table.py [prefix] — E1 per model and arm: median and range of requests
(turns), fresh prompt tokens (prompt - cached), total prompt tokens, ledger
reads and wall, plus the solved count, from out/<prefix>-<route>-<arm>-<n>.*"""
import collections, glob, json, re, statistics, sys

PREFIX = sys.argv[1] if len(sys.argv) > 1 else "e1"
rows = collections.defaultdict(list)
for log in sorted(glob.glob(f"out/{PREFIX}-*.jsonl")):
    m = re.match(rf"out/{PREFIX}-(.+)-(intro|bare|prebook|base|head)-(\d+)\.jsonl$", log)
    if not m:
        continue
    route, arm, _ = m.groups()
    reqs = [json.loads(line) for line in open(log)]
    prompt = cached = 0
    for r in reqs:
        u = r.get("usage") or {}
        prompt += u.get("prompt_tokens") or u.get("input_tokens") or 0
        d = u.get("prompt_tokens_details") or u.get("input_tokens_details") or {}
        cached += d.get("cached_tokens") or u.get("prompt_cache_hit_tokens") or 0
    body = reqs[-1]["body"] if reqs else {}
    args = [c.get("function", {}).get("arguments", "") for msg in body.get("messages", []) for c in msg.get("tool_calls") or []]
    args += [i.get("arguments", "") for i in body.get("input", []) if isinstance(i, dict) and i.get("type") == "function_call"]
    run = open(log[:-6] + ".run").read()
    rows[(route, arm)].append({
        "turns": len(reqs), "fresh": prompt - cached, "total": prompt,
        "reads": sum("ledger://" in a for a in args),
        "wall": float(re.search(r"wall=([\d.]+)", run).group(1)),
        "solved": "solved=yes" in run})

def fmt(xs):
    med = statistics.median(xs)
    return f"{med:,.0f} ({min(xs):,.0f}–{max(xs):,.0f})" if max(xs) >= 100 else f"{med:g} ({min(xs):g}–{max(xs):g})"

print("| model | arm | n | solved | turns | fresh tokens | total tokens | ledger reads | wall s |")
print("|---|---|---|---|---|---|---|---|---|")
for (route, arm), rs in sorted(rows.items(), key=lambda kv: (kv[0][0], ["prebook", "base", "bare", "intro", "head"].index(kv[0][1]))):
    print(f"| {route} | {arm} | {len(rs)} | {sum(r['solved'] for r in rs)} | {fmt([r['turns'] for r in rs])} | "
          f"{fmt([r['fresh'] for r in rs])} | {fmt([r['total'] for r in rs])} | {fmt([r['reads'] for r in rs])} | "
          f"{fmt([round(r['wall'], 1) for r in rs])} |")

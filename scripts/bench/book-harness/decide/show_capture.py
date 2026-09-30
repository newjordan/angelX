#!/usr/bin/env python3
"""show_capture.py <capture.jsonl> — per request: the new messages since the last
request, with every line-start braille stamp and any introduction text."""
import json, re, sys
STAMP = re.compile(r"(?m)^([⠀-⣿]+)")
prev = 0
for n, line in enumerate(open(sys.argv[1]), 1):
    row = json.loads(line)
    body = row.get("body", row)
    msgs = body.get("messages", [])
    print(f"== request {n}: {len(msgs)} messages, tools={len(body.get('tools') or [])}")
    for m in msgs[max(prev - 1, 0):]:
        c = m.get("content")
        if isinstance(c, list):
            c = "".join(p.get("text", "") for p in c if isinstance(p, dict))
        c = c or ""
        calls = [t["function"]["name"] for t in m.get("tool_calls") or []]
        print(f"  [{m['role']}] calls={calls} stamps={STAMP.findall(c)[:12]} len={len(c)}")
        if m["role"] != "system":
            print("    | " + c[-700:].replace("\n", "\n    | "))
    prev = len(msgs)

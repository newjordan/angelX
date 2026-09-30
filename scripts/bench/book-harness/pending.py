#!/usr/bin/env python3
"""pending.py <run-dir>: print the newest unanswered request as a model-facing
transcript (system prompt, tool schemas, conversation), or 'DONE <rc>' when the
run has exited, or 'WAIT' when angel has not sent its next request yet."""
import glob
import json
import os
import sys

run = sys.argv[1]
reqs = sorted(glob.glob(os.path.join(run, "*.req.json")), key=lambda p: int(os.path.basename(p).split(".")[0]))
pending = [p for p in reqs if not os.path.exists(p.replace(".req.json", ".resp.json"))]
if not pending:
    rc = os.path.join(run, "rc")
    print(f"DONE {open(rc).read().strip()}" if os.path.exists(rc) else "WAIT")
    sys.exit(0)
body = json.load(open(pending[0]))["body"]


def text(content):
    if isinstance(content, list):
        return "".join(part.get("text", "") for part in content if isinstance(part, dict))
    return content or ""


out = [f"REQUEST {os.path.basename(pending[0]).split('.')[0]}", "", "=== TOOLS (JSON schema you may call) ==="]
for tool in body.get("tools", []):
    fn = tool["function"]
    out.append(json.dumps({"name": fn["name"], "description": fn.get("description", ""),
                           "parameters": fn.get("parameters", {})}, ensure_ascii=False))
out.append("")
out.append("=== CONVERSATION ===")
for message in body["messages"]:
    role = message["role"]
    if role == "tool":
        out.append(f"[tool result for {message.get('tool_call_id')}]\n{text(message.get('content'))}")
    elif role == "assistant":
        parts = []
        if text(message.get("content")):
            parts.append(text(message.get("content")))
        for call in message.get("tool_calls") or []:
            parts.append(f"<tool_call id={call['id']}> {call['function']['name']} {call['function']['arguments']}")
        out.append("[assistant]\n" + "\n".join(parts))
    else:
        out.append(f"[{role}]\n{text(message.get('content'))}")
    out.append("")
print("\n".join(out))

#!/usr/bin/env python3
"""run_summary.py out/<label>.jsonl... — one line per run: requests (turns),
ledger reads (turns spent decoding stamps), tool calls by name, provider
tokens (prompt / cached / completion, when the provider reports them) and
wall time. Reads stub_model.py and proxy_log.py logs alike."""
import collections
import json
import sys


def summary(path):
    rows = [json.loads(line) for line in open(path)]
    tools = collections.Counter()
    reads = 0
    body = rows[-1]["body"] if rows else {}
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
    prompt = cached = completion = 0
    for row in rows:
        usage = row.get("usage") or {}
        messages_cache = (usage.get("cache_read_input_tokens") or 0) + (usage.get("cache_creation_input_tokens") or 0)
        prompt += usage.get("prompt_tokens") or ((usage.get("input_tokens") or 0) + messages_cache)
        completion += usage.get("completion_tokens") or usage.get("output_tokens") or 0
        details = usage.get("prompt_tokens_details") or usage.get("input_tokens_details") or {}
        cached += details.get("cached_tokens") or usage.get("prompt_cache_hit_tokens") or usage.get("cache_read_input_tokens") or 0
    wall = (rows[-1].get("done", rows[-1]["t"]) - rows[0]["t"]) if rows else 0
    return (f"{path}: requests={len(rows)} ledger_reads={reads} prompt={prompt} "
            f"cached={cached} completion={completion} wall={wall:.1f}s tools={dict(tools)}")


for path in sys.argv[1:]:
    print(summary(path))

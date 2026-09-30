#!/usr/bin/env python3
"""Pins route_action.py on a hand-built log: python3 test_route_action.py"""
import json
import os
import tempfile

import route_action


def call(name, **args):
    return {"id": name, "type": "function", "function": {"name": name, "arguments": json.dumps(args)}}


def chat_log():
    system = {"role": "system", "content": "core"}
    user = {"role": "user", "content": "loop iteration 1"}
    a1 = {"role": "assistant", "content": None, "tool_calls": [call("run_tests")]}
    t1 = {"role": "tool", "tool_call_id": "run_tests", "content": "red\n⡪⠊ the loop stalled → loop_research suggest"}
    a2 = {"role": "assistant", "content": None, "tool_calls": [call("loop_research", action="suggest")]}
    t2 = {"role": "tool", "tool_call_id": "loop_research", "content": "⡪⠙ finished unverified"}
    a3 = {"role": "assistant", "content": None, "tool_calls": [call("read_file", path="p.diff")]}
    t3 = {"role": "tool", "tool_call_id": "read_file", "content": "⡪⠛⡪⠋ live, learning error"}
    a4 = {"role": "assistant", "content": None, "tool_calls": [call("run_tests")]}
    t4 = {"role": "tool", "tool_call_id": "run_tests", "content": "⡪⠙ again"}
    a5 = {"role": "assistant", "content": None, "tool_calls": [call("loop_research", action="run", compare=True)]}
    turns = [system, user, a1, t1, a2, t2, a3, t3, a4, t4, a5]
    cuts = [4, 6, 8, 10, 11]  # each request ends on a tool result; the last carries a5
    return [{"body": {"messages": turns[:n]}} for n in cuts]


def responses_log():
    base = [{"role": "user", "content": [{"type": "input_text", "text": "go"}]}]
    c1 = {"type": "function_call", "name": "run_tests", "arguments": "{}", "call_id": "1"}
    o1 = {"type": "function_call_output", "call_id": "1", "output": "⡪⠉ failed"}
    c2 = {"type": "function_call", "name": "loop_research", "arguments": '{"action":"suggest"}', "call_id": "2"}
    return [{"body": {"input": base + [c1, o1]}}, {"body": {"input": base + [c1, o1, c2]}}]


def write(rows):
    fd, path = tempfile.mkstemp(suffix=".jsonl")
    with os.fdopen(fd, "w") as f:
        for row in rows:
            f.write(json.dumps(row) + "\n")
    return path


chat = write(chat_log())
raised, judged, matched = route_action.tally([chat])
assert raised == {"⡪⠊": 1, "⡪⠙": 2, "⡪⠛": 1, "⡪⠋": 1}, raised
assert judged == {"⡪⠊": 1, "⡪⠙": 2, "⡪⠛": 1}, judged
assert matched == {"⡪⠊": 1, "⡪⠙": 1, "⡪⠛": 1}, matched  # the read_file after ⡪⠙ misses

resp = write(responses_log())
raised, judged, matched = route_action.tally([resp])
assert raised == {"⡪⠉": 1} and matched == {"⡪⠉": 1}, (raised, matched)
os.remove(chat)
os.remove(resp)
print("route_action: ok")

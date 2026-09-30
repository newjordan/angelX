"""loop_collapse.py — loop_replay's decision, with the loop collapsed in the history.

TRACES=<traces.jsonl> OUT=<rows.jsonl> LOOP_AT=12 N=2 python3 loop_collapse.py
(run from this directory; it imports loop_replay.py). For every trace whose
model repeats one call LOOP_AT times, the history up to that decision keeps the
first (call, result) pair of the loop and drops the later identical ones and
the ⛔ turns between them, then replays to DeepSeek on the bench's terms:
  collapsed_mark  the kept result carries ⛔⠇⠁ ×N (the repeat count)
  collapsed_page  a fresh ⛔⠇⠓⠁ + loop_replay.STRONG_PAGE turn after it
One row per reply (break / repeat / answer) is appended to OUT. The
2026-09-30 night report has the results."""
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("lr", "loop_replay.py"); lr = importlib.util.module_from_spec(spec)
sys.argv = ["x"]; spec.loader.exec_module(lr)

PATH, OUT, N = __import__("os").environ["TRACES"], __import__("os").environ["OUT"], int(__import__("os").environ.get("N", "3"))
key = lr.key()
rows = []
for line in open(PATH):
    for t in json.loads(line)["traces"]:
        point = lr.decision(t)
        if not point: continue
        msgs, sig, k = point
        name = t["task"]["data"]["name"]
        tools = [{"type": "function", "function": x} for x in t["tools"]]
        # collapse: keep the first assistant with this signature and its tool result; drop later
        # identical assistants, their results, and the harness turns between them
        wired = [lr.wire(m) for m in msgs]
        first = None; keep = []
        i = 0
        while i < len(wired):
            m = wired[i]
            if m["role"] == "assistant" and m.get("tool_calls") and lr.calls_of({"tool_calls": [{"name": c["function"]["name"], "arguments": c["function"]["arguments"]} for c in m["tool_calls"]]}) == sig:
                if first is None:
                    first = i; keep.append(m); i += 1
                    while i < len(wired) and wired[i]["role"] == "tool": keep.append(wired[i]); i += 1
                    continue
                i += 1
                while i < len(wired) and wired[i]["role"] == "tool": i += 1
                continue
            if first is not None and m["role"] == "user" and (m.get("content") or "").startswith("⛔"):
                i += 1; continue
            keep.append(m); i += 1
        repeats = sum(1 for m in wired if m["role"] == "assistant" and m.get("tool_calls") and lr.calls_of({"tool_calls": [{"name": c["function"]["name"], "arguments": c["function"]["arguments"]} for c in m["tool_calls"]]}) == sig)
        for arm in ("collapsed_mark", "collapsed_page"):
            m2 = [dict(m) for m in keep]
            if arm == "collapsed_mark":
                j = next(i for i, m in enumerate(m2) if m["role"] == "tool" and i > first)
                m2[j]["content"] = (m2[j]["content"] or "") + f"\n⛔⠇⠁ ×{repeats}"
            else:
                m2.append({"role": "user", "content": f"⛔⠇⠓⠁\n\n⠇⠓⠁ {lr.STRONG_PAGE}"})
            for _ in range(N):
                reply = lr.call(key, m2, tools)
                msg = reply["choices"][0]["message"]
                nxt = lr.calls_of({"tool_calls": [{"name": c["function"]["name"], "arguments": c["function"]["arguments"]} for c in msg.get("tool_calls") or []]})
                verdict = "answer" if not nxt else ("repeat" if nxt == sig else "break")
                print(name, __import__("os").environ["LOOP_AT"], arm, verdict, len(wired), "->", len(m2), "msgs", (msg.get("content") or "")[:80].replace("\n", " "), flush=True)
                rows.append({"task": name, "file": PATH, "arm": arm, "verdict": verdict, "next": nxt, "content": msg.get("content")})
json.dump(rows, open(OUT, "a")); open(OUT, "a").write("\n")

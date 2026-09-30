#!/usr/bin/env python3
"""loop_replay.py <traces.jsonl> <out.jsonl> [n] [task...] — the cheapest cue that
breaks a live loop. For each task whose trace repeats one call, take the request
just after the LOOP_AT-th identical result, vary only that last result's cue, and
replay it to DeepSeek on the bench's own terms (temperature 0, thinking off,
8192 max tokens). A reply breaks the loop when its next call is not the repeated
one (digits masked, key order ignored) or when it answers.

Cues, cheapest first (C = the raw repeated output):
  raw        C                                     no cue: the baseline
  bare       C + ⠇⠁                                the book's stamp alone
  warn       ⛔⠇⠁ ×k + C                           warning sign on the stamp
  warn_tail  ⛔⠇⠁ ×k + … + C's last 5 lines        the r4 build
  warn_act   ⛔⠇⠁ ×k → the route's action + C      sign, stamp and its action
  english    C + ⠇⠁ signal → action               the legend at every sighting
  perturb    C, then the 0.1.6 perturbation turn   the pre-book English
Position arms (the cue as its own message after the result, not inside it):
  user_stamp ⛔⠇⠁ as a user turn                    sign and stamp alone
  user_act   ⛔⠇⠁ → action as a user turn
  sys_act    ⛔⠇⠁ → action as a system message
  user_dont  ⛔ Do not repeat the previous call.   a short English order
  tool_perturb C + the perturbation text           its content in the result
  user_intro ⛔⠇⠁ + its English as a user turn      the loop turn at first sight
  user_page  ⛔⠇⠓⠁ + a page carrying the perturbation's force, first sight
CUES picks the arms (comma list); LOOP_AT the repeat the decision follows.
One JSON row per call; the prompt-token delta against raw is each cue's cost."""
import json, os, re, subprocess, sys, urllib.request
from concurrent.futures import ThreadPoolExecutor

LOOP_AT = int(os.environ.get("LOOP_AT", "5"))
ACTION = "use the earlier result, or change one assumption before the next call"
SIGNAL = "the same tool batch came straight back with the same outcome (batch below)"
PERTURB = (
    "[harness-telemetry] You've repeated the same tool call several times with no new result — "
    "you're stuck in a loop, not converging. Break the pattern deliberately: (1) state the key "
    "assumption your current approach depends on, then test the OPPOSITE hypothesis; (2) if that "
    "doesn't fit, reframe the problem by analogy to a different domain and see what that suggests; "
    "(3) or attack it with a different tool entirely. Do not repeat the previous tool call. If you "
    "genuinely cannot make progress, give your best final answer and flag what's unresolved."
)
ALL_CUES = ("raw", "bare", "warn", "warn_tail", "warn_act", "english", "perturb",
            "user_stamp", "user_act", "sys_act", "user_dont", "tool_perturb",
            "user_intro", "user_page")
STRONG_PAGE = ("the loop will not change its answer: do not repeat the call; name the assumption "
               "behind it and test the opposite, change the code it points at, or answer with what "
               "is unresolved")
CUES = tuple(os.environ.get("CUES", ",".join(ALL_CUES[:7])).split(","))
DONT = "⛔ Do not repeat the previous tool call."
EDITS = {"write_file", "str_replace", "multi_edit", "apply_patch"}


def key():
    return subprocess.run(
        ["bash", "-c", 'source "$HOME/.config/host_env/api_keys.sh" >/dev/null 2>&1; printf %s "$DEEPSEEK_API_KEY"'],
        capture_output=True, text=True).stdout


def signature(name, arguments):
    try:
        arguments = json.dumps(json.loads(arguments), sort_keys=True)
    except (TypeError, ValueError):
        pass
    return re.sub(r"\d+", "#", f"{name} {arguments}")


def calls_of(message):
    return [signature(c.get("name"), c.get("arguments")) for c in message.get("tool_calls") or []]


def wire(message):
    """A trace node's message as a Chat Completions message."""
    out = {"role": message["role"], "content": message.get("content")}
    if message.get("tool_calls"):
        out["tool_calls"] = [{"id": c["id"], "type": "function",
                              "function": {"name": c["name"], "arguments": c["arguments"]}}
                             for c in message["tool_calls"]]
    if message["role"] == "tool":
        out["tool_call_id"] = message["tool_call_id"]
    return out


def unstamped(content):
    """The raw output: trailing warpath lines and their introductions removed."""
    lines = content.split("\n")
    while lines and (not lines[-1].strip() or re.match(r"^[⠀-⣿]", lines[-1])):
        lines.pop()
    return "\n".join(lines)


def decision(trace):
    """(messages up to the decision, the repeated signature, repeats so far) or None."""
    nodes = [n["message"] for n in trace["nodes"]]
    run, last = 0, None
    for i, message in enumerate(nodes):
        if message["role"] != "assistant" or not message.get("tool_calls"):
            continue
        sig = calls_of(message)
        run = run + 1 if sig == last else 1
        last = sig
        if run == LOOP_AT + 1:  # the model is about to repeat for the (LOOP_AT+1)th time
            return nodes[:i], sig, LOOP_AT
    return None


def cued(messages, cue, k):
    messages = [wire(m) for m in messages]
    tail = next(i for i in range(len(messages) - 1, -1, -1) if messages[i]["role"] == "tool")
    raw = unstamped(messages[tail]["content"] or "")
    lines = raw.split("\n")
    short = ("…\n" if len(lines) > 5 else "") + "\n".join(lines[-5:])
    messages[tail]["content"] = {
        "raw": raw,
        "bare": f"{raw}\n⠇⠁",
        "warn": f"⛔⠇⠁ ×{k}\n{raw}",
        "warn_tail": f"⛔⠇⠁ ×{k}\n{short}",
        "warn_act": f"⛔⠇⠁ ×{k} → {ACTION}\n{raw}",
        "english": f"{raw}\n⠇⠁ {SIGNAL} → {ACTION}",
        "tool_perturb": f"{raw}\n{PERTURB}",
    }.get(cue, raw)
    turn = {
        "perturb": ("user", PERTURB),
        "user_stamp": ("user", "⛔⠇⠁"),
        "user_act": ("user", f"⛔⠇⠁ → {ACTION}"),
        "sys_act": ("system", f"⛔⠇⠁ → {ACTION}"),
        "user_dont": ("user", DONT),
        "user_intro": ("user", f"⛔⠇⠁\n\n⠇⠁ {SIGNAL} → {ACTION}"),
        "user_page": ("user", f"⛔⠇⠓⠁\n\n⠇⠓⠁ {STRONG_PAGE}"),
    }.get(cue)
    if turn:
        messages.append({"role": turn[0], "content": turn[1]})
    return messages


def call(api_key, messages, tools):
    body = {"model": "deepseek-flash", "messages": messages, "tools": tools, "temperature": 0,
            "max_tokens": 8192, "thinking": {"type": "disabled"}, "stream": False}
    req = urllib.request.Request("https://api.deepseek.com/v1/chat/completions",
                                 data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json", "Authorization": "Bearer " + api_key})
    return json.load(urllib.request.urlopen(req, timeout=600))


def main():
    path, out = sys.argv[1], sys.argv[2]
    n = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    only = set(sys.argv[4:])
    api_key = key()
    jobs = []
    for line in open(path):
        for trace in json.loads(line)["traces"]:
            name = trace["task"]["data"]["name"]
            if only and name not in only:
                continue
            point = decision(trace)
            if not point:
                continue
            messages, repeated, k = point
            tools = [{"type": "function", "function": t} for t in trace["tools"]]
            for cue in CUES:
                for i in range(n):
                    jobs.append((name, cue, i, cued(messages, cue, k), tools, repeated))

    def one_row(row):
        return {**row, "loop_at": LOOP_AT}

    def one(job):
        name, cue, i, messages, tools, repeated = job
        try:
            reply = call(api_key, messages, tools)
        except Exception as error:  # a failed call is a row, not a crash
            return {"task": name, "cue": cue, "i": i, "error": str(error)[:300]}
        message = reply["choices"][0]["message"]
        calls = [signature(c["function"]["name"], c["function"]["arguments"]) for c in message.get("tool_calls") or []]
        names = {c["function"]["name"] for c in message.get("tool_calls") or []}
        move = ("repeat" if calls == repeated else "edit" if names & EDITS
                else "other_call" if calls else "answer")
        usage = reply.get("usage") or {}
        return {"task": name, "cue": cue, "i": i, "move": move, "broke": move != "repeat",
                "prompt_tokens": usage.get("prompt_tokens"), "completion_tokens": usage.get("completion_tokens"),
                "next": calls[:2], "text": (message.get("content") or "")[:300]}

    with ThreadPoolExecutor(8) as pool, open(out, "a") as sink:
        for row in pool.map(one, jobs):
            sink.write(json.dumps(one_row(row), ensure_ascii=False) + "\n")
            sink.flush()
            print(row.get("task"), row.get("cue"), row.get("move") or row.get("error"), flush=True)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""decide.py <model> <n> [scenario...] — replay each captured decision request
(out/dcap-<scenario>-<arm>.jsonl, request number `decision` in scenarios.json)
to a real model n times and score whether its next move is the route's action.

Arms: intro (stamp + its English at first sight), every (its English at every
sighting: the routes as English injected each time), bare (stamp only), nocue (the
bare request with the decision stamp's lines removed: the cue's own effect),
prebook (the pre-book English harness at the same point). Every model gets the
same bytes; seat parameters follow each model's angelX seat. One JSON row per
call is appended to out/decide-<model>.jsonl. Keys from the environment; Grok
through the operator's OAuth bearer (grok_bearer.py)."""
import importlib.util
import json, os, re, subprocess, sys, time, urllib.error, urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
MODELS = {
    "glm-flash": ("https://api.z.ai/api/coding/paas/v4", "ZAI_API_KEY", "glm-5.3-flash",
                  {"thinking": {"type": "enabled"}, "clear_thinking": True, "reasoning_effort": "low"}),
    "deepseek-flash": ("https://api.deepseek.com/v1", "DEEPSEEK_API_KEY", "deepseek-flash", {}),
    "muse": ("https://api.meta.ai/v1", "ANGEL_META_KEY", "muse-spark-1.3-contributor", {}),
    "grok": ("https://api.x.ai/v1", None, "grok-4.7", {"reasoning_effort": "low"}),
}
with open(os.path.join(HERE, "scenarios.json")) as source:
    SCENARIOS = json.load(source)
spec = importlib.util.spec_from_file_location("decision_producer_receipt", Path(ROOT) / "receipt_usage.py")
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)
ARMS = ("intro", "every", "bare", "nocue", "prebook")
UNTESTED_SAID = re.compile(r"untested|not (been )?(run|tested|verified)|haven.t (run|tested|verified)|without (running|testing)", re.I)
TEST_CMD = re.compile(r"unittest|pytest|run_tests|make test|cargo test", re.I)


def text_of(content):
    if isinstance(content, list):
        return "".join(p.get("text", "") for p in content if isinstance(p, dict))
    return content or ""


def decision_body(scenario, arm):
    spec = SCENARIOS[scenario]
    source = "bare" if arm == "nocue" else arm
    path = os.path.join(ROOT, "out", f"dcap-{scenario}-{source}.jsonl")
    if not os.path.exists(path):
        return None  # this arm was not captured for this point
    rows = [json.loads(row)["body"] for row in open(path)]
    if "decision" in spec:
        if len(rows) < spec["decision"]:
            return None  # the harness never reached this point
        body = rows[spec["decision"] - 1]
    else:
        # A later sighting: the last request whose newest message opens a line
        # with the route (for arms without the cue, the same request number).
        stamp = re.compile(rf"(?m)^{spec['route']}")
        cued = [i for i, b in enumerate(rows) if stamp.search(text_of(b["messages"][-1]["content"]))]
        if not cued:
            anchor = os.path.join(ROOT, "out", f"dcap-{scenario}-bare.jsonl")
            anchors = [json.loads(row)["body"] for row in open(anchor)]
            cued = [i for i, b in enumerate(anchors) if stamp.search(text_of(b["messages"][-1]["content"]))]
            if not cued or cued[-1] >= len(rows):
                return None
        body = rows[cued[-1]]
    if arm == "nocue":
        stamp = re.compile(rf"(?m)^{spec['route']}[⠀-⣿]*\n?")
        last = body["messages"][-1]
        stripped = stamp.sub("", text_of(last["content"])).rstrip()
        if stripped == text_of(last["content"]).rstrip():
            sys.exit(f"{scenario}: no {spec['route']} line in the decision message")
        if not stripped:
            return None  # the cue was the whole message: no request without it
        last["content"] = stripped
    return body


def score(scenario, calls, text):
    names = [c["function"]["name"] for c in calls]
    if scenario.startswith("batch"):
        return len(names) >= 2 or "code_mode" in names
    if scenario == "untested":
        if names:
            first = calls[0]["function"]
            return first["name"] == "run_tests" or bool(TEST_CMD.search(first.get("arguments", "")))
        return bool(UNTESTED_SAID.search(text))
    if scenario == "finish":
        return not names
    raise KeyError(scenario)


def bearer():
    return subprocess.run([sys.executable, os.path.join(ROOT, "grok_bearer.py")],
                          capture_output=True, text=True, check=True).stdout.strip()


def call(model, body, max_tokens):
    url, key_env, model_id, extra = MODELS[model]
    messages = json.loads(json.dumps(body["messages"]))
    if model == "deepseek-flash":  # thinking mode wants each assistant turn's reasoning back
        for m in messages:
            if m.get("role") == "assistant":
                m.setdefault("reasoning_content", "")
    payload = {"model": model_id, "messages": messages, "tools": body.get("tools", []),
               "max_tokens": max_tokens, "stream": False, **extra}
    key = os.environ[key_env] if key_env else bearer()
    req = urllib.request.Request(url + "/chat/completions", data=json.dumps(payload).encode(), method="POST",
                                 headers={"Content-Type": "application/json", "Authorization": "Bearer " + key})
    for attempt in range(4):
        start = time.time()
        try:
            with urllib.request.urlopen(req, timeout=600) as response:
                reply = json.load(response)
            return reply, time.time() - start, None
        except urllib.error.HTTPError as e:
            try:
                err = f"{e.code} {e.read()[:300].decode('utf-8', 'replace')}"
            finally:
                e.close()
            if e.code not in (429, 500, 502, 503, 504):
                return None, time.time() - start, err
        except Exception as e:  # timeouts, resets
            err = repr(e)[:300]
        time.sleep(5 * (attempt + 1))
    return None, 0, err


def reply_fields(reply):
    """Only an observed, well-formed next move can enter the scored sample."""
    if not isinstance(reply, dict):
        raise ValueError("reply must be an object")
    choices = reply.get("choices")
    if not isinstance(choices, list) or not choices or not isinstance(choices[0], dict):
        raise ValueError("reply has no first choice")
    choice = choices[0]
    msg = choice.get("message")
    if not isinstance(msg, dict):
        raise ValueError("choice has no message")
    if not any(key in msg for key in ("content", "tool_calls")):
        raise ValueError("message has no observed content or calls")
    if msg.get("content") is None and msg.get("tool_calls") is None:
        raise ValueError("message has neither observed content nor calls")
    if "role" in msg and msg["role"] != "assistant":
        raise ValueError("choice is not an assistant message")
    calls = msg.get("tool_calls")
    calls = [] if calls is None else calls
    if not isinstance(calls, list):
        raise ValueError("tool calls must be a list")
    for call_ in calls:
        function = call_.get("function") if isinstance(call_, dict) else None
        if not isinstance(function, dict) or not isinstance(function.get("name"), str) or not function["name"]:
            raise ValueError("tool call has no function name")
        if not isinstance(function.get("arguments"), str):
            raise ValueError("tool arguments must be a string")
    content = msg.get("content")
    if isinstance(content, list):
        if any(not isinstance(part, dict) or not isinstance(part.get("text", ""), str) for part in content):
            raise ValueError("content parts must have string text")
    elif content is not None and not isinstance(content, str):
        raise ValueError("message content must be text or parts")
    finish = choice.get("finish_reason")
    if finish is not None and not isinstance(finish, str):
        raise ValueError("finish reason must be a string")
    return calls, text_of(content), finish


def one(model, scenario, arm, i, body, max_tokens):
    row = {"model": model, "scenario": scenario, "arm": arm, "i": i, "secs": None, "error": None}
    try:
        reply, secs, err = call(model, body, max_tokens)
    except Exception as error:
        # A failed worker must still produce its logical sample. Control
        # signals (KeyboardInterrupt/SystemExit) keep their existing behavior.
        row.update(error=type(error).__name__, error_class=type(error).__name__, call_error=True)
        return row
    measured_secs = receipt.number(secs)
    row.update(secs=round(measured_secs, 2) if measured_secs is not None else None, error=err)
    if err:
        return row
    usage = reply.get("usage") if isinstance(reply, dict) else None
    usage = usage if isinstance(usage, dict) else {}
    row.update(prompt=receipt.number(usage.get("prompt_tokens")),
               completion=receipt.number(usage.get("completion_tokens")),
               reasoning=receipt.detail(usage, "completion_tokens_details", "reasoning_tokens"))
    try:
        calls, text, finish = reply_fields(reply)
        row.update(match=score(scenario, calls, text), calls=[c["function"]["name"] for c in calls],
                   args=[c["function"].get("arguments", "")[:200] for c in calls], text=text[:400], finish=finish)
    except (ValueError, TypeError, KeyError, IndexError, AttributeError) as error:
        row.update(error=type(error).__name__, error_class=type(error).__name__, reply_error=True)
    return row


def main():
    model, n = sys.argv[1], int(sys.argv[2])
    scenarios = sys.argv[3:] or list(SCENARIOS)
    max_tokens = int(os.environ.get("DECIDE_MAX_TOKENS", "8192"))
    jobs = []
    for scenario in scenarios:
        for arm in ARMS:
            body = decision_body(scenario, arm)
            if body is None:
                print(f"{scenario}/{arm}: no decision request", file=sys.stderr)
                continue
            jobs += [(model, scenario, arm, i, body, max_tokens) for i in range(n)]
    with open(os.path.join(ROOT, "out", f"decide-{model}.jsonl"), "a") as out:
        with ThreadPoolExecutor(int(os.environ.get("DECIDE_THREADS", "6"))) as pool:
            for row in pool.map(lambda job: one(*job), jobs):
                out.write(json.dumps(row, allow_nan=False) + "\n"); out.flush()
                mark = "err" if row["error"] else ("✓" if row["match"] else "·")
                print(f"{model} {row['scenario']:8} {row['arm']:7} {row['i']} {mark} {row.get('calls')} {row['secs']}s", flush=True)


if __name__ == "__main__":
    main()

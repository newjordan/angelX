#!/usr/bin/env python3
"""route_action.py out/<label>.jsonl... — E3a: does the model's next call act
on the ⡪ route it was just shown?

For every logged request whose newest model-facing message (a tool result or
a loop prompt) carries a ⡪ route, the next request's history holds the model's
reply to it; the reply's first tool call is checked against the route's action
(`d2467_research.rs`). Routes whose action names no tool (⡪⠋, ⡪⠓, ⡪⠚) are
counted as raised only. Reads Chat Completions and Responses API logs alike.

  python3 route_action.py out/e3-intro-*.jsonl      # one table over all runs
  python3 route_action.py --each out/e3-*.jsonl     # one table per run
"""
import collections
import json
import re
import sys

ROUTE = re.compile("⡪[⠁-⣿]")  # ⡪ and its sub cell


def research(action, **extra):
    def match(name, args):
        if name != "loop_research" or args.get("action") != action:
            return False
        return all(args.get(key) == value for key, value in extra.items())
    return match


def applies_patch(name, args):
    return (name == "loop_research" and args.get("action") == "results") or name in {
        "apply_patch", "str_replace", "write_file", "edit_file", "multi_edit"}


def not_research(name, args):
    return name not in {"loop_research", "rl_campaign"}


def not_polling(name, args):
    return not (name in {"loop_research", "rl_campaign"} and args.get("action") == "status")


# route → (action as the book states it, predicate on the next tool call)
ACTIONS = {
    "⡪⠁": ("apply its patch from results, then run the loop's verifier", applies_patch),
    "⡪⠃": ("apply this idea first", applies_patch),
    "⡪⠉": ("suggest again before the next run", research("suggest")),
    "⡪⠙": ("rerun it with a verifier (compare: true)", research("run", compare=True)),
    "⡪⠑": ("fix the verifier or the baseline on the main workspace", not_research),
    "⡪⠛": ("keep working; check status only when it would change the next step", not_polling),
    "⡪⠊": ("loop_research suggest, then run its top idea with compare", research("suggest")),
    "⡪⠋": (None, None),
    "⡪⠓": (None, None),
    "⡪⠚": (None, None),
}


def text_of(content):
    if isinstance(content, list):
        return "".join(part.get("text", "") or part.get("output", "") for part in content
                       if isinstance(part, dict))
    return content or ""


def history(body):
    """[(role, text, calls)] in order, for either wire format."""
    out = []
    for message in body.get("messages", []):
        calls = [(c.get("function", {}).get("name", "?"), c.get("function", {}).get("arguments", ""))
                 for c in message.get("tool_calls") or []]
        out.append((message.get("role"), text_of(message.get("content")), calls))
    for item in body.get("input", []) if isinstance(body.get("input"), list) else []:
        kind = item.get("type")
        if kind == "function_call":
            call = (item.get("name", "?"), item.get("arguments", ""))
            if out and out[-1][0] == "assistant":
                out[-1][2].append(call)
            else:
                out.append(("assistant", "", [call]))
        elif kind == "function_call_output":
            out.append(("tool", text_of(item.get("output")), []))
        else:
            out.append((item.get("role", kind), text_of(item.get("content")), []))
    return out


def newest_routes(turns):
    """Routes in the model-facing messages after the last assistant turn."""
    tail = []
    for role, text, _ in reversed(turns):
        if role == "assistant":
            break
        tail.append(text)
    return sorted(set(ROUTE.findall("".join(tail))))


def first_call(turns, start):
    for role, _, calls in turns[start:]:
        if role == "assistant" and calls:
            name, raw = calls[0]
            try:
                args = json.loads(raw) if raw else {}
            except ValueError:
                args = {}
            return name, args if isinstance(args, dict) else {}
        if role == "assistant":
            return "(answer)", {}
    return None


def tally_bodies(bodies):
    """Count one already-loaded request snapshot without reopening its wire."""
    raised = collections.Counter()
    matched = collections.Counter()
    judged = collections.Counter()
    for now, after in zip(bodies, bodies[1:]):
        turns = history(now)
        routes = newest_routes(turns)
        if not routes:
            continue
        call = first_call(history(after), len(turns))
        for route in routes:
            raised[route] += 1
            predicate = ACTIONS.get(route, (None, None))[1]
            if predicate is None or call is None:
                continue
            judged[route] += 1
            matched[route] += bool(predicate(*call))
    return raised, judged, matched


def tally(paths):
    raised = collections.Counter()
    matched = collections.Counter()
    judged = collections.Counter()
    for path in paths:
        with open(path) as stream:
            bodies = [json.loads(line)["body"] for line in stream]
        run_raised, run_judged, run_matched = tally_bodies(bodies)
        raised.update(run_raised)
        judged.update(run_judged)
        matched.update(run_matched)
    return raised, judged, matched


def report(title, raised, judged, matched):
    print(title)
    print(f"  {'route':6} {'raised':>6} {'judged':>6} {'matched':>7}  action")
    for route in sorted(raised):
        action = ACTIONS.get(route, ("?", None))[0] or "(names no tool)"
        print(f"  {route:6} {raised[route]:>6} {judged[route]:>6} {matched[route]:>7}  {action}")
    total_j, total_m = sum(judged.values()), sum(matched.values())
    rate = f"{100 * total_m / total_j:.0f}%" if total_j else "n/a"
    print(f"  all    {sum(raised.values()):>6} {total_j:>6} {total_m:>7}  match rate {rate}")


if __name__ == "__main__":
    args = sys.argv[1:]
    if args and args[0] == "--each":
        for path in args[1:]:
            report(path, *tally([path]))
    else:
        report(" ".join(args), *tally(args))

#!/usr/bin/env python3
"""wire_connections.py <label> [<label> ...] — did each approved piece reach each
seat's wire? Reads out/wire-<label>-<seat>-<scenario>.jsonl (wire_connections.sh)
and prints one grid per label: a row per piece, a column per seat."""
import glob
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SEATS = ["deepseek", "glm", "grok", "muse", "local"]


def text(content):
    if isinstance(content, list):
        return "".join(p.get("text", "") for p in content if isinstance(p, dict))
    return content or ""


def normalize(body):
    """(messages as (role, text), tools as (name, description)) for either API."""
    msgs, tools = [], []
    if "messages" in body:
        for m in body["messages"]:
            msgs.append((m.get("role"), text(m.get("content"))))
        for t in body.get("tools") or []:
            f = t.get("function") or t
            tools.append((f.get("name"), json.dumps(f)))
    else:
        if body.get("instructions"):
            msgs.append(("system", body["instructions"]))
        for item in body.get("input") or []:
            kind = item.get("type")
            if kind == "message":
                msgs.append((item.get("role"), text(item.get("content"))))
            elif kind == "function_call_output":
                msgs.append(("tool", item.get("output") or ""))
            elif kind == "function_call":
                msgs.append(("assistant-call", item.get("name") or ""))
        for t in body.get("tools") or []:
            tools.append((t.get("name"), json.dumps(t)))
    return msgs, tools


def load(label, seat, scenario):
    path = os.path.join(HERE, "out", f"wire-{label}-{seat}-{scenario}.jsonl")
    if not os.path.exists(path):
        return None
    return [json.loads(line) for line in open(path) if line.strip()]


def checks(fix, loop):
    """Each piece: True/False, or None when the scenario did not run."""
    out = {}
    if fix:
        first_msgs, first_tools = normalize(fix[0]["body"])
        system = "\n".join(t for r, t in first_msgs if r in ("system", "developer"))
        first_all = "\n".join(t for _, t in first_msgs)
        out["entry warpath ⠽ in system"] = system.lstrip().startswith("⠽")
        out["brevity system message ⠵"] = any(r == "system" and t.lstrip().startswith("⠵") for r, t in first_msgs)
        card = next((t for _, t in first_msgs if "⠥⠃ the playbook referral card" in t), "")
        out["playbook card wording"] = "0.1.8" if "Use `tool_search` to discover" in card else ("0.1.9" if "Call `skill(name)`" in card else "none")
        out["skill hint ⠥⠉ (synthetic prompt)"] = "⠥⠉" in first_all
        out["legend English for the entry"] = "You are Angel" in first_all
        out["caveman line (method page)"] = "caveman line" in first_all
        out["read_file carries ledger direction"] = any(n == "read_file" and "ledger://" in d for n, d in first_tools)
        second = normalize(fix[1]["body"])[0] if len(fix) > 1 else []
        out["hygiene ⠺⠁ on first result"] = any("⠺⠁" in t for r, t in second if r in ("tool",))
        edit_at = next((i for i, r in enumerate(fix) if (r.get("action") or [None])[0] == "str_replace"), None)
        after_edit = fix[edit_at + 1:] if edit_at is not None else []
        out["post-edit ⠼⠁ voiced"] = any("⠼⠁" in t and "Review the edit" in t
                                         for r in after_edit for _, t in normalize(r["body"])[0])
        last = "\n".join(t for _, t in normalize(fix[-1]["body"])[0])
        out["green cue ⠺⠓"] = "⠺⠓" in last
        out["first request bytes"] = fix[0].get("bytes")
    if loop:
        allm = [m for r in loop for m in normalize(r["body"])[0]]
        out["loop ⛔ own turn"] = any(r in ("user", "system", "developer") and t.lstrip().startswith("⛔") for r, t in allm)
        out["loop ladder ⠇⠓"] = any("⠇⠓" in t for _, t in allm)
    return out


def main(labels):
    for label in labels:
        grid = {s: checks(load(label, s, "fix"), load(label, s, "loop")) for s in SEATS}
        rows = []
        for s in SEATS:
            for k in grid[s]:
                if k not in rows:
                    rows.append(k)
        print(f"\n== {label}")
        print(f"{'piece':38}" + "".join(f"{s:>10}" for s in SEATS))
        for k in rows:
            cells = []
            for s in SEATS:
                v = grid[s].get(k)
                cells.append("—" if v is None else ("yes" if v is True else ("NO" if v is False else str(v))))
            print(f"{k:38}" + "".join(f"{c:>10}" for c in cells))


if __name__ == "__main__":
    main(sys.argv[1:] or sorted({os.path.basename(p).split("-")[1] for p in glob.glob(os.path.join(HERE, "out", "wire-*.jsonl"))}))

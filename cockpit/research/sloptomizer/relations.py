"""Sloptomizer's live evidence relationships, separate from fitness rewards.

Tool receipts describe checks, not task completion. This projection keeps both
sides of a changed verdict and offers discriminating next experiments without
making choices, running providers, or imposing a research budget.
"""

from __future__ import annotations

import copy


def _append(checks, event):
    key = event["check"]
    row = checks.setdefault(key, {"check": key, "count": 0, "repeat": 0})
    previous = row.get("latest")
    same_verdict = previous and all(previous.get(k) == event.get(k) for k in
                                   ("verdict", "hypothesis", "expected", "route"))
    same = same_verdict and previous.get("receipt") == event.get("receipt")
    row["repeat"] = row["repeat"] + 1 if same else 1
    # Verifier output often includes volatile clocks or changing diagnostics.
    # Keep that evidence distinct; a repeated verdict does not establish that
    # the failure is identical or that the intervening work made no progress.
    row["verdict_run"] = row.get("verdict_run", 0) + 1 if same_verdict else 1
    row["count"] += 1
    row["latest"] = event
    if event["verdict"] in ("passed", "failed"):
        row[event["verdict"]] = event["id"]
        row["conclusive"] = event["verdict"]
    # Order by last evidence, not dictionary insertion of the first check.
    checks.pop(key)
    checks[key] = row


def project(events, limit=6):
    checks = {}
    for event in events:
        _append(checks, event)
    return list(checks.values())[-limit:][::-1]


def apply(state, incoming):
    events = state.setdefault("relations", [])
    known = {event["id"]: event for event in events}
    changed = False
    signals = []
    checks = {row["check"]: row for row in project(events, len(events) + 1)[::-1]}
    for raw in incoming:
        event = copy.deepcopy(raw)
        if not isinstance(event, dict):
            raise ValueError("relation requires a controller receipt")
        for key in ("id", "check", "receipt", "tool", "route"):
            if not isinstance(event.get(key), str) or not event[key]:
                raise ValueError(f"relation requires {key}")
        if event.get("verdict") not in ("passed", "failed", "inconclusive"):
            raise ValueError("relation requires a typed verifier verdict")
        for key in ("hypothesis", "expected", "check_note", "receipt_excerpt"):
            if key in event and (not isinstance(event[key], str) or len(event[key]) > 240):
                raise ValueError(f"relation {key} exceeds its compact field")
        if event["id"] in known:
            if known[event["id"]] != event:
                raise ValueError("conflicting relation identity; original preserved")
            continue
        previous = checks.get(event["check"], {})
        before = previous.get("latest")
        conclusive = previous.get("conclusive")
        events.append(event)
        known[event["id"]] = event
        changed = True
        _append(checks, event)
        row = checks[event["check"]]
        kind = None
        if (conclusive
                and event["verdict"] in ("passed", "failed")
                and conclusive != event["verdict"]):
            kind = "contrast"
        elif row["repeat"] >= 2 and row["repeat"] & (row["repeat"] - 1) == 0:
            kind = "repeat"
        elif (event["verdict"] == "failed" and row["verdict_run"] >= 4
              and row["verdict_run"] & (row["verdict_run"] - 1) == 0):
            kind = "failure-run"
        elif not before and event["verdict"] == "inconclusive":
            kind = "unbound"
        if kind:
            signals.append({"kind": kind, "event": event["id"], "check": event["check"],
                            "repeat": row["repeat"], "verdict_run": row["verdict_run"],
                            "verdict": event["verdict"]})
    return changed, signals


def advice(state, signals=()):
    events = state.get("relations", [])
    checks = project(events, len(events) + 1)
    signals = list(signals)[-6:]
    frontier = checks[:6]
    if signals:
        # A burst can contain quiet checks after its last warning. Keep the
        # warning's actual check beside the cue, even outside the recent six.
        trigger = next(row for row in checks if row["check"] == signals[-1]["check"])
        frontier = [trigger] + [row for row in frontier if row is not trigger][:5]
    return {"engine": "sloptomizer", "advisory": True,
            "relation_count": len(events), "checks": frontier,
            "check_count": len(checks),
            "contrast_count": sum("passed" in row and "failed" in row for row in checks),
            "inconclusive_count": sum(row["verdict"] == "inconclusive" for row in events),
            "signals": signals,
            "evidence_scope": "individual checks; candidate and objective fitness unbound"}

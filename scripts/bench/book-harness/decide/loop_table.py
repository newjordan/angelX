#!/usr/bin/env python3
"""loop_table.py <replay.jsonl>... — per cue: loops broken, the move, and the
median prompt-token cost over raw at the same point (same task, depth, sample)."""
import argparse
import collections
import importlib.util
import json
from pathlib import Path


spec = importlib.util.spec_from_file_location("loop_decision_table", Path(__file__).with_name("decide_table.py"))
decision = importlib.util.module_from_spec(spec)
spec.loader.exec_module(decision)
MOVES = ("repeat", "edit", "other_call", "answer")


def depth_of(row):
    value = row.get("loop_at", 5)
    return value if isinstance(value, int) and not isinstance(value, bool) and value > 0 else None


def cue_of(row):
    value = row.get("cue")
    return value if isinstance(value, str) and value else "unknown"


def identity(row):
    task, sample, depth = row.get("task"), row.get("i"), depth_of(row)
    if (not isinstance(task, str) or not task or depth is None or not isinstance(sample, int)
            or isinstance(sample, bool) or sample < 0):
        return None
    return task, depth, sample


def reply_observed(row):
    move = row.get("move")
    return (not row.get("error") and move in MOVES and isinstance(row.get("broke"), bool)
            and row["broke"] == (move != "repeat"))


def upper_median(values):
    """Preserve the legacy table's upper-middle convention for even cohorts."""
    return sorted(values)[len(values) // 2] if values else None


def aggregate(rows):
    raw = collections.defaultdict(list)
    samples = collections.Counter()
    for row in rows:
        key = identity(row)
        if key is not None:
            samples[(cue_of(row), key)] += 1
            if cue_of(row) == "raw":
                raw[key].append(row)
    cells = collections.defaultdict(list)
    for row in rows:
        cells[(depth_of(row), cue_of(row))].append(row)
    depths = sorted({key[0] for key in cells}, key=lambda value: (value is None, value or 0))
    out = []
    for depth in depths:
        for (current_depth, cue), rs in cells.items():
            if current_depth != depth:
                continue
            replies = [row for row in rs if reply_observed(row)]
            moves = collections.Counter(row["move"] for row in replies)
            costs, issues = [], []
            for index, row in enumerate(rs, 1):
                key = identity(row)
                reason = None
                if key is None:
                    reason = "missing/invalid task/depth/sample identity"
                elif samples[(cue, key)] > 1 or len(raw[key]) > 1:
                    reason = "ambiguous raw/cue sample identity"
                elif not raw[key]:
                    reason = "missing raw sample"
                else:
                    prompt = decision.usage.number(row.get("prompt_tokens"))
                    baseline = decision.usage.number(raw[key][0].get("prompt_tokens"))
                    if prompt is None or baseline is None:
                        reason = "missing/invalid prompt measurement"
                    elif row.get("error") or raw[key][0].get("error"):
                        reason = "failed sample has no eligible paired cost"
                    else:
                        costs.append(prompt - baseline)
                if reason:
                    issues.append({"row": index, "task": key[0] if key else None,
                                   "sample": key[2] if key else None, "reason": reason})
            complete = bool(rs) and len(costs) == len(rs)
            out.append({"loop_at": depth, "cue": cue, "attempts": len(rs), "replies": len(replies),
                        "no_reply": len(rs) - len(replies), "errors": sum(bool(row.get("error")) for row in rs),
                        "broke": sum(row["broke"] for row in replies), "moves": dict(moves),
                        "cost": upper_median(costs) if complete else None, "reported_cost": upper_median(costs),
                        "cost_coverage": {"reported": len(costs), "attempts": len(rs), "complete": complete},
                        "issues": issues})
    return out


def table(cells):
    lines, previous = [], object()
    for cell in cells:
        depth, cue = cell["loop_at"], cell["cue"]
        if depth != previous:
            lines.extend((f"\nafter repeat {depth if depth is not None else 'unknown'}",
                          f"{'cue':<13}{'broke':>9}{'edit':>6}{'other':>7}{'answer':>8}{'cost (tok)':>12}"))
            previous = depth
        moves = cell["moves"]
        text = (f"{cue:<13}{cell['broke']:>5}/{cell['replies']:<3}{moves.get('edit', 0):>6}"
                f"{moves.get('other_call', 0):>7}{moves.get('answer', 0):>8}"
                f"{(cell['cost'] if cell['cost'] is not None else '—'):>12}")
        if cell["replies"] != cell["attempts"] or not cell["cost_coverage"]["complete"]:
            text += (f" [attempts={cell['attempts']}; replies={cell['replies']}; errors={cell['errors']}; "
                     f"no_reply={cell['no_reply']}; cost pairs={cell['cost_coverage']['reported']}/{cell['attempts']}; "
                     f"reported cost={cell['reported_cost']}; issues={dict(collections.Counter(i['reason'] for i in cell['issues']))}]")
        lines.append(text)
    return "\n".join(lines) + ("\n" if lines else "")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", type=Path)
    parser.add_argument("--audit-json", action="store_true")
    args = parser.parse_args(argv)
    try:
        cells = aggregate(decision.read_rows(args.paths))
    except (OSError, ValueError) as error:
        parser.exit(2, f"{error}\n")
    if args.audit_json:
        print(json.dumps(cells, indent=2, allow_nan=False))
    else:
        print(table(cells), end="")


if __name__ == "__main__":
    main()

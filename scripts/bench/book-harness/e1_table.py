#!/usr/bin/env python3
"""E1 per model/arm: checked receipt tokens, turns, ledger reads and wall."""
import argparse
import collections
import importlib.util
import json
from pathlib import Path
import re
import statistics


spec = importlib.util.spec_from_file_location("book_e1_receipt_usage", Path(__file__).with_name("receipt_usage.py"))
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)
ARM_ORDER = ("prebook", "base", "bare", "intro", "head")


def ledger_reads(body):
    if not isinstance(body, dict):
        return None
    args = [call.get("function", {}).get("arguments", "")
            for message in body.get("messages", []) for call in message.get("tool_calls") or []]
    args += [item.get("arguments", "") for item in body.get("input", [])
             if isinstance(item, dict) and item.get("type") == "function_call"]
    return sum("ledger://" in argument for argument in args if isinstance(argument, str))


def load_rows(prefix="e1", out_dir=Path("out")):
    rows = collections.defaultdict(list)
    pattern = re.compile(rf"{re.escape(prefix)}-(.+)-(intro|bare|prebook|base|head)-(\d+)\.jsonl$")
    for log in sorted(Path(out_dir).glob(prefix + "-*.jsonl")):
        match = pattern.fullmatch(log.name)
        if not match:
            continue
        route, arm, round_number = match.groups()
        requests = receipt.read_receipts(log)
        run_path = log.with_suffix(".run")
        text = run_path.read_text() if run_path.exists() else ""
        wall = re.search(r"(?:^|\s)wall=([\d.]+)(?=\s|$)", text)
        try:
            wall_value = receipt.number(float(wall.group(1))) if wall else None
        except ValueError:
            wall_value = None
        solved = re.search(r"(?:^|\s)solved=(yes|no)(?=\s|$)", text)
        body = requests[-1].get("body") if requests else None
        rows[(route, arm)].append({"round": round_number, "turns": len(requests),
                                  **receipt.usage_summary(requests), "reads": ledger_reads(body),
                                  "wall": wall_value, "solved": solved.group(1) == "yes" if solved else None})
    return rows


def fmt(values):
    if not values:
        return "—"
    median = statistics.median(values)
    return (f"{median:,.0f} ({min(values):,.0f}–{max(values):,.0f})" if max(values) >= 100
            else f"{median:g} ({min(values):g}–{max(values):g})")


def metric(rows, key, *, rounded=False):
    values = [row[key] for row in rows if receipt.number(row.get(key)) is not None]
    result = fmt([round(value, 1) for value in values] if rounded else values)
    if key in receipt.FIELDS:
        coverage = [row["usage_metric_coverage"][key] for row in rows]
        reported = sum(field["reported"] for field in coverage)
        attempts = sum(field["attempts"] for field in coverage)
        if len(values) != len(rows) or reported != attempts:
            result += f" [runs {len(values)}/{len(rows)}; receipts {reported}/{attempts}]"
    elif len(values) != len(rows):
        result += f" [{len(values)}/{len(rows)} reported]"
    return result


def table(rows):
    lines = ["| model | arm | n | solved | turns | fresh tokens | total tokens | ledger reads | wall s |",
             "|---|---|---|---|---|---|---|---|---|"]
    for (route, arm), runs in sorted(rows.items(), key=lambda item: (item[0][0], ARM_ORDER.index(item[0][1]))):
        solved = sum(run["solved"] is True for run in runs)
        scored = sum(run["solved"] is not None for run in runs)
        solved_text = str(solved) if scored == len(runs) else f"{solved} [{scored}/{len(runs)} scored]"
        lines.append(f"| {route} | {arm} | {len(runs)} | {solved_text} | {metric(runs, 'turns')} | "
                     f"{metric(runs, 'fresh')} | {metric(runs, 'prompt')} | {metric(runs, 'reads')} | "
                     f"{metric(runs, 'wall', rounded=True)} |")
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("prefix", nargs="?", default="e1")
    parser.add_argument("--out-dir", type=Path, default=Path("out"))
    args = parser.parse_args(argv)
    if not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9._-]*", args.prefix):
        parser.error("prefix must be an alphanumeric label with dot, underscore or dash")
    try:
        result = table(load_rows(args.prefix, args.out_dir))
    except (ValueError, OSError) as error:
        parser.error(str(error))
    print(result, end="")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""e3_table.py — E3 and E4 per model and arm, from out/e3-<route>-<arm>-<n>.*

E3: sessions, iterations reached, requests, prompt tokens (cached share), the
⡪ routes raised / judged / matched (route_action.py) and the match rate.
Heesch: the hc_verified of each session's final submission/best.heesch
(re-verified with HEESCH's venv), and whether it changed from the frontier.
E4 (intro arm): loop-iteration observations in the learner's state against
the iterations the loop record verified."""
import argparse
import collections
import importlib.util
import glob
import json
import os
from pathlib import Path
import re
import subprocess


def load_helper(name):
    spec = importlib.util.spec_from_file_location("book_e3_" + name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


route_action = load_helper("route_action")
receipt = load_helper("receipt_usage")


def hc_of(path, heesch):
    if not os.path.exists(path):
        return None
    frontier = (Path(heesch) / "submission/best.heesch").read_text()
    if Path(path).read_text() == frontier:
        return "frontier"
    run = subprocess.run([os.path.join(heesch, ".venv-bench/bin/python"), "-m", "heesch_verify", path],
                         capture_output=True, text=True, cwd=heesch)
    try:
        d = json.loads(run.stdout)
    except ValueError:
        return "rejected"
    return d.get("hc_verified", d.get("error", "?"))


def learner_observations(state_dir):
    n = 0
    for path in glob.glob(os.path.join(state_dir, "learning-*.json")):
        text = Path(path).read_text()
        n += text.count('"loop-iteration"')
    return n


def load_sessions(out_dir=Path("out"), heesch=None):
    heesch = heesch or os.environ.get("HEESCH", os.path.expanduser("~/comps/heesch"))
    sessions = collections.defaultdict(list)
    for log in sorted(Path(out_dir).glob("e3-*-*-[0-9]*.jsonl")):
        match = re.fullmatch(r"e3-(.+)-(intro|bare)-(\d+)\.jsonl", log.name)
        if not match:
            continue
        route, arm, round_number = match.groups()
        base = log.with_suffix("")
        rows = receipt.read_receipts(log)
        loop = {}
        for path in Path(str(base) + ".state/loops").glob("*.json"):
            loop = json.loads(path.read_text())
        raised, judged, matched = route_action.tally_bodies([row["body"] for row in rows])
        run_path = log.with_suffix(".run")
        text = run_path.read_text() if run_path.exists() else ""
        status = re.search(r"(?:^|\s)(status=\S+)", text)
        sessions[(route, arm)].append({
            "n": round_number, "requests": len(rows), **receipt.usage_summary(rows),
            "iterations": loop.get("iteration", 0), "status": status.group(1) if status else "status=unknown",
            "directions": len(loop.get("directions_tried", [])),
            "raised": raised, "judged": judged, "matched": matched,
            "hc": hc_of(str(base) + ".state/submission/best.heesch", heesch),
            "observed": learner_observations(str(base) + ".state"),
        })
    return sessions


def tokens(sessions):
    complete = all(session["prompt"] is not None and session["cached"] is not None for session in sessions)
    invalid = sum(session["cache_invalid_receipts"] for session in sessions)
    if complete and not invalid:
        prompt = sum(session["prompt"] for session in sessions)
        cached = sum(session["cached"] for session in sessions)
        share = f"{100 * cached / prompt:.0f}%" if prompt else "n/a"
        return f"{prompt / 1e6:.1f}M ({share})"
    fields = []
    for key in ("prompt", "cached"):
        coverage = [session["usage_metric_coverage"][key] for session in sessions]
        reported = sum(field["reported"] for field in coverage)
        attempts = sum(field["attempts"] for field in coverage)
        known = [session["usage_reported_subtotals"][key] for session in sessions
                 if session["usage_reported_subtotals"][key] is not None]
        subtotal = str(sum(known)) if known else "unknown"
        fields.append(f"{key} receipts {reported}/{attempts}, reported subtotal {subtotal}")
    if invalid:
        fields.append(f"invalid cache receipts {invalid}")
    return "— [" + "; ".join(fields) + "]"


def table(sessions):
    lines = ["| model | arm | sessions | iterations | requests | prompt tokens (cached) | routes raised | judged | matched | match rate | final hc |",
             "|---|---|---|---|---|---|---|---|---|---|---|"]
    for (route, arm), runs in sorted(sessions.items()):
        raised = sum((run["raised"] for run in runs), collections.Counter())
        judged = sum(sum(run["judged"].values()) for run in runs)
        matched = sum(sum(run["matched"].values()) for run in runs)
        rate = f"{100 * matched / judged:.0f}%" if judged else "n/a"
        lines.append(f"| {route} | {arm} | {len(runs)} | {', '.join(str(run['iterations']) for run in runs)} | "
                     f"{sum(run['requests'] for run in runs):,} | {tokens(runs)} | "
                     f"{' '.join(f'{key}×{value}' for key, value in sorted(raised.items())) or '—'} | {judged} | {matched} | {rate} | "
                     f"{', '.join(str(run['hc']) for run in runs)} |")
    lines += ["", "E4 (intro arm): learner loop-iteration observations vs loop iterations, per session"]
    for (route, arm), runs in sorted(sessions.items()):
        if arm == "intro":
            lines.append(f"  {route}: " + "; ".join(f"#{run['n']} obs={run['observed']} iters={run['iterations']} dirs={run['directions']} {run['status']}" for run in runs))
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", type=Path, default=Path("out"))
    parser.add_argument("--heesch", type=Path, default=None)
    args = parser.parse_args(argv)
    try:
        result = table(load_sessions(args.out_dir, args.heesch))
    except (ValueError, OSError) as error:
        parser.error(str(error))
    print(result, end="")


if __name__ == "__main__":
    main()

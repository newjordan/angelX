"""Harness reply handling and post-tool time from recorded response completion.

Legacy logs without done can be estimated with --assume-instant-stub;
that assumption is visible and is never reported as observed response latency.
Response latency includes proxy/transport work and completion logging.
"""
import argparse
import importlib.util
import json
from pathlib import Path


spec = importlib.util.spec_from_file_location("harness_phase_accounting", Path(__file__).with_name("phase_accounting.py"))
phase = importlib.util.module_from_spec(spec)
spec.loader.exec_module(phase)


def total(label, out_dir=Path("out"), assume_instant_stub=False):
    result = phase.load(label, out_dir, assume_instant_stub)
    return result["totals"]["harness"], result["totals"]["tool"]


def seconds(value):
    return f"{value:6.2f}s" if value is not None else "     —"


def summary(label, result):
    totals, coverage = result["totals"], result["metric_coverage"]
    estimated = coverage["harness"]["estimated"]
    name = "harness_estimate" if estimated else "harness"
    scopes = sorted({hop["response_scope"] for hop in result["hops"]}) or ["no-between-request-hops"]
    text = (f"{label:12s} {name}={seconds(totals['harness'])} tools={seconds(totals['tool'])} "
            f"response_latency={seconds(totals['response_latency'])}")
    counts = "; ".join(f"{field}={coverage[field]['reported']}/{coverage[field]['attempts']}"
                       for field in ("harness", "tool", "response_latency"))
    unknown = coverage["harness"]["attempts"] - coverage["harness"]["reported"]
    text += f" [hops={len(result['hops'])}; {counts}; unknown_harness_hops={unknown}; scope={','.join(scopes)}"
    if not coverage["harness"]["complete"]:
        text += f"; reported_harness_subtotal={seconds(result['reported_subtotals']['harness']).strip()}"
    return text + "]"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("labels", nargs="*")
    parser.add_argument("--out-dir", type=Path, default=Path("out"))
    parser.add_argument("--assume-instant-stub", action="store_true", help="explicitly estimate missing done clocks as arrival")
    parser.add_argument("--audit-json", action="store_true")
    args = parser.parse_args(argv)
    try:
        results = [{"label": label, **phase.load(label, args.out_dir, args.assume_instant_stub)} for label in args.labels]
    except (OSError, ValueError) as error:
        parser.error(str(error))
    if args.audit_json:
        print(json.dumps(results, indent=1, allow_nan=False))
    else:
        for result in results:
            print(summary(result["label"], result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Checked response latency, reply→call, tool-span and done→next-request phases.

--assume-instant-stub explicitly estimates missing legacy response clocks.
Timestamped tool names support elapsed spans, not invocation-level timings.
Response latency includes proxy/transport work and completion logging.
"""
import argparse
import importlib.util
import json
from pathlib import Path


spec = importlib.util.spec_from_file_location("grouped_phase_accounting", Path(__file__).with_name("phase_accounting.py"))
phase = importlib.util.module_from_spec(spec)
spec.loader.exec_module(phase)


def summary(group):
    fields = " ".join(f"{field}={value * 1000:.1f}" if value is not None else f"{field}=—"
                      for field, value in sorted(group["medians"].items()))
    coverage = "; ".join(f"{field} {value['reported']}/{value['attempts']} observed={value['observed']} estimated={value['estimated']}"
                         for field, value in sorted(group["metric_coverage"].items()))
    return f"{group['tool']} {fields} [hops={group['hops']}; {coverage}]"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("label")
    parser.add_argument("--out-dir", type=Path, default=Path("out"))
    parser.add_argument("--assume-instant-stub", action="store_true", help="explicitly estimate missing done clocks as arrival")
    parser.add_argument("--audit-json", action="store_true")
    args = parser.parse_args(argv)
    try:
        result = phase.load(args.label, args.out_dir, args.assume_instant_stub)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    groups = phase.phase_groups(result)
    if args.audit_json:
        print(json.dumps({"label": args.label, **result, "groups": groups}, indent=1, allow_nan=False))
    else:
        print(f"{args.label}: {len(result['hops'])} between-request hops; assume_instant_stub={args.assume_instant_stub}")
        for group in groups:
            print(summary(group))
        if not groups:
            print("no between-request phases observed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

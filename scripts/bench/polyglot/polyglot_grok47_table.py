#!/usr/bin/env python3
"""README-style Grok Polyglot table with per-field receipt coverage."""
from __future__ import annotations

import argparse
import importlib.util
from pathlib import Path


spec = importlib.util.spec_from_file_location(
    "polyglot_luna_table", Path(__file__).with_name("polyglot_luna_table.py"))
table = importlib.util.module_from_spec(spec)
spec.loader.exec_module(table)


def newest():
    root = Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921/runs/grok/angelx")
    runs = [p for p in root.glob("seed0-*") if (p / "traces.jsonl").is_file()]
    if not runs:
        raise SystemExit(f"no grok runs under {root}")
    return max(runs, key=lambda p: p.stat().st_mtime)


def load_rows(run: Path):
    return table.load_rows(run, prefer_proxy_metadata=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", type=Path, nargs="?", help="run directory; defaults to newest Grok seed0")
    args = parser.parse_args(argv)
    run = args.run if args.run is not None else newest()
    table.print_table(run, load_rows(run))


if __name__ == "__main__":
    main()

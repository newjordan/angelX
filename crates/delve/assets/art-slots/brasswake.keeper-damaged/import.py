#!/usr/bin/env python3
"""Reproduce the reviewed supplemental Keeper bank with the shared converter."""
import argparse
import importlib.util
import json
from pathlib import Path

root = Path(__file__).resolve().parents[5]
spec = importlib.util.spec_from_file_location("slot_import", root / "scripts/slot-art-import.py")
converter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(converter)
parser = argparse.ArgumentParser()
parser.add_argument("--check", action="store_true")
args = parser.parse_args()
args.source_dir = None
args.v1_height = args.v2_height = args.terminal_metrics = None
request = json.loads(Path(__file__).with_name("request.json").read_text())
converter.run(request["id"], request, args, "OK", "")

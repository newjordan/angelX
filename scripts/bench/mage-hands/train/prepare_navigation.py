"""Quarantine shared transition frames across splits; retain raw traces separately.

This prepares future collection only, never mutates a frozen GPU dataset. A shared
catalog/checkout/completion view is not independent held-out grounding evidence.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import importlib.util
import json
from pathlib import Path


def eligible_pages(pages):
    """Input rows carry a content hash; remove ALL sides of cross-split matches."""
    memberships = defaultdict(set)
    for page in pages:
        memberships[page["sha256"]].add(page["split"])
    contaminated = {key for key, splits in memberships.items() if len(splits) > 1}
    eligible = [page for page in pages if page["sha256"] not in contaminated]
    quarantined = [page for page in pages if page["sha256"] in contaminated]
    return eligible, quarantined


def prepare(manifests, output):
    output = Path(output)
    if output.exists():
        raise ValueError("new immutable output directory required")
    pages = []
    groups = {}
    for name in manifests:
        path = Path(name).resolve()
        for item in json.loads(path.read_text()):
            page = dict(item)
            shot = path.parent / page["shot"]
            page["shot"] = str(shot)
            page["sha256"] = hashlib.sha256(shot.read_bytes()).hexdigest()
            old = groups.setdefault(page["group"], page["split"])
            if old != page["split"]:
                raise ValueError("scenario crosses split")
            pages.append(page)
    eligible, quarantined = eligible_pages(pages)
    spec = importlib.util.spec_from_file_location("regime", Path(__file__).with_name("regime.py"))
    regime = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(regime)
    audit = regime.audit_pages(eligible)
    output.mkdir(parents=True)
    (output / "pages.json").write_text(json.dumps(eligible, indent=2) + "\n")
    (output / "quarantined.json").write_text(json.dumps(quarantined, indent=2) + "\n")
    counts = Counter(page["family"] + "/" + page["split"] for page in eligible)
    receipt = {"claim": "scripted genuine transition collection, not autonomous-agent results",
               "in_frozen_gpu_pilot": False, "raw_frames": len(pages), "eligible_frames": len(eligible),
               "quarantined_frames": len(quarantined), "counts": dict(counts), "audit": audit,
               "raw_manifests_sha256": {str(Path(p).resolve()): hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in manifests},
               "policy": "all sides of exact cross-split frame reuse quarantined; raw traces unchanged"}
    (output / "audit.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))
    return receipt


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", action="append", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    prepare(args.manifest, args.output)

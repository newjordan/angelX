#!/usr/bin/env python3
"""Independent audit of the assembled Mage training dataset.

Re-implements every check from the raw files on disk. It does not import the
builder's validator, so a bug in the builder cannot validate itself.
"""
from __future__ import annotations

import hashlib
import json
import math
import re
import sys
from collections import Counter
from pathlib import Path

SPLITS = ("train", "validation", "test")


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def rows_of(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def clicked_point(row: dict) -> tuple[float, float]:
    w, h = row["width"], row["height"]
    if type(w) is not int or type(h) is not int or w < 2 or h < 2:
        raise ValueError("bad dimensions")
    zoom, crop = row["zoom"], row["crop"]
    if type(zoom) is not int or zoom not in (1, 2, 3):
        raise ValueError("bad zoom")
    if crop is None:
        if zoom != 1:
            raise ValueError("zoom without crop")
    else:
        if len(crop) != 4 or any(type(v) is not int for v in crop):
            raise ValueError("bad crop")
        x, y, cw, ch = crop
        if (x < 0 or y < 0 or x + cw > w or y + ch > h
                or cw != w // zoom or ch != h // zoom):
            raise ValueError("crop out of bounds or wrong scale")
    box = row["box"]
    if (len(box) != 4 or any(type(v) not in (int, float) or not math.isfinite(v) for v in box)
            or not (0 <= box[0] < box[2] <= w and 0 <= box[1] < box[3] <= h)):
        raise ValueError("bad box")
    if not isinstance(row["target"], str) or not row["target"].strip():
        raise ValueError("empty target")
    calls = re.findall(r"<tool_call>\s*(.*?)\s*</tool_call>", row["answer"], re.S)
    if len(calls) != 1 or row["answer"].count("<tool_call>") != 1:
        raise ValueError("not exactly one tool call")
    call = json.loads(calls[0])
    a = call.get("arguments", {})
    if call.get("name") != "computer_use" or a.get("action") != "click" or a.get("button") != "left":
        raise ValueError("not a single left click")
    pt = a.get("coordinate")
    if len(pt) != 2 or any(type(v) is not int or not 0 <= v <= 999 for v in pt):
        raise ValueError("bad coordinate")
    px, py = pt[0] / 999 * (w - 1), pt[1] / 999 * (h - 1)
    if crop is not None:
        px, py = x + px / w * cw, y + py / h * ch
    if not (box[0] <= px <= box[2] and box[1] <= py <= box[3]):
        raise ValueError("click outside target box")
    return px, py


def main(argv=None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if len(argv) != 2:
        raise SystemExit("usage: verify_training_set.py <dataset> <baseline>")
    root, base = Path(argv[0]).resolve(), Path(argv[1]).resolve()
    fail = []
    checks = []

    def check(name, fn):
        try:
            checks.append({"name": name, "pass": True, "detail": fn()})
        except Exception as exc:  # noqa: BLE001 - report and continue
            checks.append({"name": name, "pass": False, "error": f"{type(exc).__name__}: {exc}"})
            fail.append(name)

    manifest = json.loads((root / "manifest.json").read_text())

    def inventory():
        expected = {f"{s}.jsonl" for s in SPLITS} | {"system.txt", "manifest.json"}
        present = {p.name for p in root.iterdir() if p.is_file()}
        if present != expected:
            raise AssertionError(f"unexpected root files: {present ^ expected}")
        shots = [p for p in (root / "screens").iterdir() if p.is_file()]
        if any(p.suffix != ".png" for p in shots):
            raise AssertionError("non-png in screens/")
        return {"root_files": len(present), "screens": len(shots)}

    def system():
        if (root / "system.txt").read_bytes() != (base / "system.txt").read_bytes():
            raise AssertionError("system prompt differs from baseline")
        return {"system_sha256": sha((root / "system.txt").read_bytes())}

    loaded = {s: rows_of(root / f"{s}.jsonl") for s in SPLITS}

    def counts():
        if {s: len(v) for s, v in loaded.items()} != manifest["counts"]:
            raise AssertionError("row counts disagree with manifest")
        return manifest["counts"]

    def geometry():
        checked = 0
        for split in SPLITS:
            for row in loaded[split]:
                if row.get("split") != split:
                    raise AssertionError(f"split field mismatch in {split}")
                clicked_point(row)
                checked += 1
        return {"rows_checked": checked}

    pixel_hashes: dict[str, set[str]] = {s: set() for s in SPLITS}
    scene_hashes: dict[str, set[str]] = {s: set() for s in SPLITS}

    def screens():
        from PIL import Image
        seen: dict[str, str] = {}
        for split in SPLITS:
            for row in loaded[split]:
                rel = row["screen"]
                if not rel.startswith("screens/"):
                    raise AssertionError(f"unsafe screen path: {rel}")
                path = root / rel
                if not path.is_file() or path.is_symlink():
                    raise AssertionError(f"missing/linked screen: {rel}")
                if rel not in seen:
                    data = path.read_bytes()
                    digest = sha(data)
                    if Path(rel).stem != digest:
                        raise AssertionError(f"screen name is not its content hash: {rel}")
                    with Image.open(path) as image:
                        pixels = sha(image.convert("RGB").tobytes())
                    seen[rel] = pixels
                scene_hashes[split].add(rel)
                pixel_hashes[split].add(seen[rel])
        return {"decoded": len(seen)}

    def preserved():
        for split in ("validation", "test"):
            if loaded[split] != rows_of(base / f"{split}.jsonl"):
                raise AssertionError(f"{split} rows were changed")
        base_train = rows_of(base / "train.jsonl")
        if loaded["train"][:len(base_train)] != base_train:
            raise AssertionError("baseline train rows are not preserved in order")
        return {"val_test_identical": True, "baseline_train_rows": len(base_train)}

    def augmented():
        added = loaded["train"][len(rows_of(base / "train.jsonl")):]
        if len(added) != manifest["added_rows"]:
            raise AssertionError("added row count disagrees with manifest")
        kinds = Counter()
        for row in added:
            aug = row.get("augmentation")
            if not isinstance(aug, dict):
                raise AssertionError("added row without provenance")
            kinds[aug.get("kind")] += 1
            if aug.get("kind") == "element_dropout":
                if not aug.get("dropped"):
                    raise AssertionError("added row dropped nothing")
            elif aug.get("kind") == "osatlas_grounding":
                if row.get("source") != "OS-Copilot/OS-Atlas-data" or not aug.get("stem"):
                    raise AssertionError("osatlas row without source provenance")
                if row.get("crop") is not None or row.get("zoom") != 1:
                    raise AssertionError("osatlas row is not a full-frame view")
            else:
                raise AssertionError(f"added row with unknown provenance kind: {aug.get('kind')!r}")
            if row.get("split") != "train":
                raise AssertionError("added row outside train")
        parents = {r["parent_label_id"] for r in added if r.get("parent_label_id")}
        return {"added": len(added), "kinds": dict(kinds), "parents": len(parents)}

    def no_cross_split():
        for a in SPLITS:
            for b in SPLITS:
                if a >= b:
                    continue
                for kind, cache in (("scene", scene_hashes), ("pixel", pixel_hashes)):
                    shared = cache[a] & cache[b]
                    if shared:
                        raise AssertionError(f"{kind} shared by {a}/{b}: {len(shared)}")
        groups = {s: {r["group"] for r in loaded[s]} for s in SPLITS}
        for a in SPLITS:
            for b in SPLITS:
                if a < b and groups[a] & groups[b]:
                    raise AssertionError(f"group shared by {a}/{b}")
        return {"scenes": {s: len(v) for s, v in scene_hashes.items()},
                "pixels": {s: len(v) for s, v in pixel_hashes.items()}}

    def dedup():
        for split in SPLITS:
            keys = [(r["screen"], r["target"], tuple(r["box"]), r["answer"]) for r in loaded[split]]
            if len(keys) != len(set(keys)):
                raise AssertionError(f"duplicate supervision rows in {split}")
        # A label_id is an annotation, not a row: the 1x/2x/3x zoom cascade
        # renders the same annotation several times. That is intended, but the
        # views must agree on what is being clicked and how it is supervised.
        views = Counter()
        for split in SPLITS:
            by_id: dict[str, list[dict]] = {}
            for row in loaded[split]:
                by_id.setdefault(row["label_id"], []).append(row)
            for label_id, group in by_id.items():
                views[len(group)] += 1
                identity = {(r["target"], tuple(r["box"]), r["group"], r["family"]) for r in group}
                if len(identity) != 1:
                    raise AssertionError(f"label_id {label_id} spans different targets")
                zooms = [(tuple(r["crop"]) if r.get("crop") is not None else None, r["zoom"])
                         for r in group]
                if len(zooms) != len(set(zooms)):
                    raise AssertionError(f"label_id {label_id} repeats an identical view")
                if any(crop is None and zoom != 1 for crop, zoom in zooms):
                    raise AssertionError(f"label_id {label_id} has a zoom without a crop")
        return {"unique_rows": {s: len(v) for s, v in loaded.items()},
                "labels": sum(views.values()), "views_per_label": dict(sorted(views.items()))}

    def families():
        if manifest["families"] != {s: dict(Counter(r["family"] for r in loaded[s])) for s in SPLITS}:
            raise AssertionError("family counts disagree with manifest")
        return {"families": len({r["family"] for rows in loaded.values() for r in rows})}

    def actions():
        kinds = Counter()
        for split in SPLITS:
            for row in loaded[split]:
                call = json.loads(re.search(r"<tool_call>\s*(.*?)\s*</tool_call>", row["answer"], re.S)[1])
                kinds[call["arguments"]["action"]] += 1
        return dict(kinds)

    check("file_inventory", inventory)
    check("system_prompt", system)
    check("row_counts", counts)
    check("geometry_and_answers", geometry)
    check("screens_decode", screens)
    check("baseline_preserved", preserved)
    check("augmentation", augmented)
    check("no_cross_split_leakage", no_cross_split)
    check("no_duplicates", dedup)
    check("families", families)
    check("action_coverage", actions)

    verdict = {"dataset": str(root), "baseline": str(base), "checks": checks,
               "pass": not fail, "failed": fail,
               "manifest_sha256": sha((root / "manifest.json").read_bytes())}
    print(json.dumps(verdict, indent=2))
    return 0 if not fail else 1


if __name__ == "__main__":
    sys.exit(main())

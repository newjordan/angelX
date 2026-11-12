#!/usr/bin/env python3
"""Assemble a Mage training dataset: frozen baseline + sampled OS-Atlas rows.

Inputs (read only):
  baseline:    mage-train-20261101-v1 shaped dir (train/validation/test.jsonl,
               system.txt, screens/<content-sha>.png)
  osatlas:     convert_osatlas.py output JSONL (screen=bare stem, box_norm)
  images:      one or more dirs of extracted OS-Atlas PNGs named <stem>.png

Output: a NEW directory in exactly the baseline shape, plus a manifest.
Baseline train rows stay first and byte-identical in order; validation and
test are copied untouched, so the 1,258-row holdout remains the promotion
measure. OS-Atlas rows are upgraded to the current schema (pixel box,
width/height, content-hash screen name, label_id) so verify_training_set.py
audits them with the same geometry rules as every other row.

Sampling: at most --per-screen rows per unique screen *content* (duplicate
archive members collapse first), interactive a11y roles before static text,
deterministic hash order inside a role tier. Drops are counted, never silent:
missing image, oversized box, quantized centre outside the pixel box,
duplicate (screen, target), screen colliding with a baseline screen.

Nothing is uploaded, trained, or deployed here.
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path

SPLITS = ("train", "validation", "test")

# Interactive roles first: production targets are clickable widgets.
# Static text / labels still ground (drivers name text too), just last.
STATIC_TYPES = {"AXStaticText", "label", "AXGroup", "canvas", "icon"}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_rows(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def tier(data_type) -> int:
    return 1 if data_type in STATIC_TYPES else 0


def order_key(stem: str, target: str) -> str:
    return hashlib.sha256(f"{stem}|{target}".encode()).hexdigest()


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--baseline", required=True)
    ap.add_argument("--osatlas-rows", required=True)
    ap.add_argument("--images", action="append", required=True,
                    help="dir of <stem>.png files; repeat per platform")
    ap.add_argument("--per-screen", type=int, default=16)
    ap.add_argument("--max-area", type=float, default=0.25)
    ap.add_argument("--output", required=True)
    ap.add_argument("--name", required=True)
    args = ap.parse_args(argv)

    baseline = Path(args.baseline).resolve()
    out = Path(args.output).resolve()
    if out.exists():
        raise SystemExit(f"output already exists: {out}")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{2,60}", args.name):
        raise SystemExit("--name must be a short lowercase slug")
    if args.per_screen < 1:
        raise SystemExit("--per-screen must be >= 1")

    from PIL import Image

    base = {split: read_rows(baseline / f"{split}.jsonl") for split in SPLITS}
    base_counts = {split: len(rows) for split, rows in base.items()}
    base_screens = {row["screen"] for rows in base.values() for row in rows}

    # Index extracted images by stem; a stem in two dirs is an input error.
    stem_file: dict[str, Path] = {}
    for image_dir in args.images:
        for path in sorted(p for p in Path(image_dir).glob("*.png") if p.is_file()):
            if path.stem in stem_file:
                raise SystemExit(f"duplicate image stem across --images dirs: {path.stem}")
            stem_file[path.stem] = path

    drop = collections.Counter()
    raw = read_rows(Path(args.osatlas_rows))

    # Resolve every referenced stem to content identity first, so duplicate
    # archive members (same bytes, different names) share one scene and the
    # per-screen cap applies to screens, not filenames.
    stem_meta: dict[str, dict] = {}
    for row in raw:
        stem = row["screen"]
        if stem in stem_meta:
            continue
        path = stem_file.get(stem)
        if path is None:
            stem_meta[stem] = {}
            continue
        sha = digest(path)
        try:
            with Image.open(path) as image:
                image.convert("RGB").load()  # header alone passes truncated PNGs
                width, height = image.size
        except Exception:  # noqa: BLE001 - counted as missing below, by row
            stem_meta[stem] = {}
            continue
        stem_meta[stem] = {"path": path, "sha": sha, "width": width, "height": height}

    candidates: dict[str, list[dict]] = collections.defaultdict(list)  # by content sha
    for row in raw:
        meta = stem_meta[row["screen"]]
        if not meta:
            drop["missing_image"] += 1
            continue
        x0, y0, x1, y1 = row["box_norm"]
        if (x1 - x0) * (y1 - y0) > args.max_area:
            drop["box_too_large"] += 1
            continue
        candidates[meta["sha"]].append(row)

    kept: list[dict] = []
    origins: dict[str, Path] = {}
    seen_keys: set[tuple] = set()
    for sha, screen_rows in sorted(candidates.items()):
        rel = f"screens/{sha}.png"
        if rel in base_screens:
            drop["screen_collides_with_baseline"] += len(screen_rows)
            continue
        meta = next(stem_meta[row["screen"]] for row in screen_rows)
        width, height = meta["width"], meta["height"]
        ranked = sorted(screen_rows,
                        key=lambda r: (tier(r.get("data_type")),
                                       order_key(r["screen"], r["target"])))
        taken = 0
        for row in ranked:
            if taken >= args.per_screen:
                drop["per_screen_cap"] += 1
                continue
            x0, y0, x1, y1 = row["box_norm"]
            box = [x0 * width, y0 * height, x1 * width, y1 * height]
            cx = max(0, min(999, round((x0 + x1) / 2 * 999)))
            cy = max(0, min(999, round((y0 + y1) / 2 * 999)))
            # Same projection verify_training_set.py applies: the quantized
            # centre must land inside the pixel box or the row teaches a miss.
            px, py = cx / 999 * (width - 1), cy / 999 * (height - 1)
            if not (box[0] <= px <= box[2] and box[1] <= py <= box[3]):
                drop["centre_outside_box"] += 1
                continue
            key = (rel, row["target"], tuple(box))
            if key in seen_keys:
                drop["duplicate_screen_target"] += 1
                continue
            seen_keys.add(key)
            call = {"name": "computer_use",
                    "arguments": {"action": "click", "coordinate": [cx, cy],
                                  "button": "left"}}
            kept.append({
                "answer": f"Action: Click {row['target']}.\n\n<tool_call>\n"
                          f"{json.dumps(call)}\n</tool_call>",
                "augmentation": {"kind": "osatlas_grounding",
                                 "stem": row["screen"],
                                 "platform": row["family"].removeprefix("osatlas-")},
                "box": box,
                "crop": None,
                "data_type": row.get("data_type"),
                "entities": [],
                "family": row["family"],
                "group": f"{row['family']}:{sha[:12]}",
                "height": height,
                "label_id": hashlib.sha256(
                    f"osatlas|{sha}|{row['target']}|{row['box_norm']}".encode()).hexdigest(),
                "screen": rel,
                "source": "OS-Copilot/OS-Atlas-data",
                "split": "train",
                "target": row["target"],
                "width": width,
                "zoom": 1,
            })
            origins[rel] = meta["path"]
            taken += 1

    rows = {"train": base["train"] + kept,
            "validation": base["validation"], "test": base["test"]}
    for split in ("validation", "test"):
        for row in rows[split]:
            origins.setdefault(row["screen"], baseline / row["screen"])
    for row in base["train"]:
        origins.setdefault(row["screen"], baseline / row["screen"])

    staging = out.with_name(out.name + ".building")
    if staging.exists():
        raise SystemExit(f"staging path exists: {staging}")
    (staging / "screens").mkdir(parents=True)
    installed = {}
    for split in SPLITS:
        for row in rows[split]:
            rel = row["screen"]
            if rel in installed:
                continue
            destination = staging / rel
            shutil.copyfile(origins[rel], destination)
            installed[rel] = digest(destination)
            if Path(rel).stem != installed[rel]:
                raise SystemExit(f"copied screen hash mismatch: {rel}")
    for split in SPLITS:
        with (staging / f"{split}.jsonl").open("w", encoding="utf-8") as handle:
            for row in rows[split]:
                handle.write(json.dumps(row, sort_keys=True, ensure_ascii=False) + "\n")
    shutil.copyfile(baseline / "system.txt", staging / "system.txt")

    def unique_scenes(split: str) -> int:
        return len({row["screen"] for row in rows[split]})

    counts = {split: len(rows[split]) for split in SPLITS}
    manifest = {
        "schema": "mage-training-dataset-v1",
        "name": args.name,
        "baseline": {"path": str(baseline),
                     "manifest_sha256": digest(baseline / "manifest.json"),
                     "counts": base_counts},
        "augmentation": {
            "kind": "osatlas_grounding",
            "rows_file": str(Path(args.osatlas_rows).resolve()),
            "rows_file_sha256": digest(Path(args.osatlas_rows)),
            "per_screen_cap": args.per_screen,
            "max_box_area": args.max_area,
            "drops": dict(sorted(drop.items())),
            "source_dataset": "OS-Copilot/OS-Atlas-data (Apache-2.0)",
        },
        "counts": counts,
        "added_rows": len(kept),
        "unique_scenes": {split: unique_scenes(split) for split in SPLITS},
        "families": {split: dict(sorted(collections.Counter(
            row["family"] for row in rows[split]).items())) for split in SPLITS},
        "screens": len(installed),
        "system_sha256": digest(staging / "system.txt"),
        "files": {split + ".jsonl": digest(staging / f"{split}.jsonl") for split in SPLITS},
        "rows_per_unique_scene": {split: round(counts[split] / unique_scenes(split), 2)
                                  for split in SPLITS},
        "claim": ("supervised single-click grounding rows; OS-Atlas rows are "
                  "public a11y-tree annotations, not verified live interactions"),
        "training_started": False,
        "deployment_changed": False,
    }
    (staging / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    staging.rename(out)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())

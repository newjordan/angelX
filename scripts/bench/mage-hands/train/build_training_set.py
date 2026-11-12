#!/usr/bin/env python3
"""Assemble the final Mage training dataset from the frozen baseline.

Inputs (read only):
  baseline:  frozen hands dataset with train/validation/test.jsonl + screens/
  addon:     optional element-dropout positives (train-only JSONL + screens/)

Output: a NEW directory shaped exactly as modal_regime.py consumes it
(train.jsonl, validation.jsonl, test.jsonl, system.txt, screens/) plus a
manifest. Nothing is uploaded, trained, or deployed here.

Failure policy: refuse a new artifact unless every row is provably a single
in-box click on an installed screen, no augmented screen leaks into a held-out
split, and no duplicate supervision row is introduced.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import shutil
import sys
from pathlib import Path

SPLITS = ("train", "validation", "test")
ZOOMS = {1, 2, 3}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_rows(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def project_click(row: dict) -> tuple[float, float]:
    """Validate one supervision row and return the clicked pixel point."""
    width, height = row["width"], row["height"]
    if type(width) is not int or type(height) is not int or width < 2 or height < 2:
        raise ValueError("invalid dimensions")
    zoom, crop = row["zoom"], row["crop"]
    if type(zoom) is not int or zoom not in ZOOMS:
        raise ValueError("invalid zoom")
    if crop is not None:
        if (not isinstance(crop, list) or len(crop) != 4
                or any(type(v) is not int for v in crop)):
            raise ValueError("invalid crop values")
        x, y, cw, ch = crop
        if (x < 0 or y < 0 or cw < 1 or ch < 1
                or x + cw > width or y + ch > height
                or cw != width // zoom or ch != height // zoom):
            raise ValueError("invalid crop bounds or scale")
    elif zoom != 1:
        raise ValueError("crop missing at zoom > 1")
    box = row["box"]
    if (len(box) != 4
            or any(type(v) not in (float, int) or not math.isfinite(v) for v in box)
            or not (0 <= box[0] < box[2] <= width and 0 <= box[1] < box[3] <= height)):
        raise ValueError("invalid target box")
    if not isinstance(row["target"], str) or not row["target"].strip():
        raise ValueError("empty target")
    calls = re.findall(r"<tool_call>\s*(.*?)\s*</tool_call>", row["answer"], re.S)
    if len(calls) != 1 or row["answer"].count("<tool_call>") != 1:
        raise ValueError("answer must have exactly one complete tool call")
    call = json.loads(calls[0])
    args = call.get("arguments", {})
    if (call.get("name") != "computer_use" or args.get("action") != "click"
            or args.get("button") != "left"):
        raise ValueError("answer is not a single left click")
    point = args.get("coordinate")
    if (not isinstance(point, list) or len(point) != 2
            or any(type(v) is not int or not 0 <= v <= 999 for v in point)):
        raise ValueError("invalid answer coordinate")
    px = point[0] / 999 * (width - 1)
    py = point[1] / 999 * (height - 1)
    if crop is not None:
        px, py = x + px / width * cw, y + py / height * ch
    if not (box[0] <= px <= box[2] and box[1] <= py <= box[3]):
        raise ValueError("answer outside annotated target box")
    return px, py


def screen_digest(root: Path, row: dict, *, name_is_content: bool = True) -> str:
    rel = row["screen"]
    if not isinstance(rel, str) or not rel.startswith("screens/"):
        raise ValueError(f"unsafe screen path: {rel!r}")
    path = root / rel
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"missing or linked screen: {rel}")
    observed = digest(path)
    if name_is_content and Path(rel).stem != observed:
        raise ValueError(f"screen content does not match its name: {rel}")
    return observed


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--baseline", required=True)
    ap.add_argument("--addon", default="")
    ap.add_argument("--addon-rows", default="train_dropout.jsonl")
    ap.add_argument("--output", required=True)
    ap.add_argument("--name", required=True)
    args = ap.parse_args(argv)

    baseline = Path(args.baseline).resolve()
    addon = Path(args.addon).resolve() if args.addon else None
    out = Path(args.output).resolve()
    if out.exists():
        raise SystemExit(f"output already exists: {out}")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{2,60}", args.name):
        raise SystemExit("--name must be a short lowercase slug")

    rows = {split: read_rows(baseline / f"{split}.jsonl") for split in SPLITS}
    base_counts = {split: len(value) for split, value in rows.items()}

    # Validate and fingerprint every baseline row/screen before touching disk.
    base_screen_sha: dict[str, str] = {}
    for split in SPLITS:
        for row in rows[split]:
            if row.get("split") != split:
                raise SystemExit(f"{split} row declares split={row.get('split')!r}")
            project_click(row)
            base_screen_sha[row["screen"]] = screen_digest(baseline, row)
    # Every installed screen is re-keyed by its content hash; the addon names
    # its images by variant id, so merge through this single map.
    origins = {rel: baseline / rel for rel in base_screen_sha}

    added = []
    addon_meta = None
    if addon is not None:
        addon_rows = read_rows(addon / args.addon_rows)
        manifest = json.loads((addon / "manifest.json").read_text())
        audit = json.loads((addon / "AUDIT.json").read_text())
        source = manifest.get("source") or {}
        # The addon records the source hash it observed before and after its
        # build; both must name this baseline, and its own audit must have passed.
        base_manifest_sha = digest(baseline / "manifest.json")
        observed = {source.get("manifest_sha256_before"),
                    source.get("manifest_sha256_after"),
                    source.get("manifest_sha256_expected"),
                    audit.get("source_manifest_sha256")}
        if observed != {base_manifest_sha}:
            raise SystemExit(f"addon was built from a different baseline manifest: {observed}")
        if audit.get("pass") is not True:
            raise SystemExit("addon artifact audit did not pass")
        addon_meta = {
            "artifact": str(addon),
            "manifest_sha256": digest(addon / "manifest.json"),
            "audit_manifest_sha256": audit.get("manifest_sha256"),
            "audit_verdict": audit.get("verdict"),
            "rows_file": args.addon_rows,
            "source_manifest_sha256": base_manifest_sha,
        }
        if addon_meta["audit_manifest_sha256"] != addon_meta["manifest_sha256"]:
            raise SystemExit("addon manifest changed after its own audit passed")
        if source.get("system_sha256") not in (None, digest(baseline / "system.txt")):
            raise SystemExit("addon was built against a different system prompt")
        if str(addon).startswith(str(baseline)):
            raise SystemExit("addon must not live inside the baseline dataset")

        # Held-out screens and pixels from the untouched splits set the gate.
        heldout_screens = {row["screen"] for split in ("validation", "test") for row in rows[split]}
        from PIL import Image
        heldout_pixels = set()
        for rel in sorted(heldout_screens):
            with Image.open(baseline / rel) as image:
                heldout_pixels.add(hashlib.sha256(image.convert("RGB").tobytes()).hexdigest())

        for row in addon_rows:
            if row.get("split") != "train":
                raise SystemExit(f"addon row is not train: {row.get('split')!r}")
            augmentation = row.get("augmentation")
            if not isinstance(augmentation, dict) or augmentation.get("kind") != "element_dropout":
                raise SystemExit("addon row is not a declared element-dropout augmentation")
            project_click(row)
            old_rel = row["screen"]
            sha = screen_digest(addon, row, name_is_content=False)
            if row.get("variant_id") and Path(old_rel).stem != row["variant_id"]:
                raise SystemExit(f"addon screen does not match its variant id: {old_rel}")
            row["screen"] = f"screens/{sha}.png"
            # The addon reused its parent's label_id. In the merged set every
            # supervision row needs its own identity while staying traceable.
            row["parent_label_id"] = row.get("parent_label_id") or row["label_id"]
            row["label_id"] = hashlib.sha256(
                f"variant|{row.get('variant_id') or sha}".encode()).hexdigest()
            if row["screen"] in base_screen_sha or sha in set(base_screen_sha.values()):
                raise SystemExit(f"addon screen already exists in baseline: {old_rel}")
            if old_rel in heldout_screens:
                raise SystemExit(f"addon screen is held out: {old_rel}")
            with Image.open(addon / old_rel) as image:
                pixels = hashlib.sha256(image.convert("RGB").tobytes()).hexdigest()
            if pixels in heldout_pixels:
                raise SystemExit(f"addon pixels leak a held-out screen: {old_rel}")
            origins[row["screen"]] = addon / old_rel
            added.append((row, sha, pixels))

        # No exact duplicate supervision, and no duplicate screen among addons.
        seen_keys, seen_sha = set(), set()
        for row, sha, _ in added:
            key = (sha, row["target"], tuple(row["box"]), row["answer"])
            if key in seen_keys:
                raise SystemExit(f"duplicate addon supervision: {row['screen']} {row['target']}")
            if sha in seen_sha:
                raise SystemExit(f"addon screen reused: {row['screen']}")
            seen_keys.add(key)
            seen_sha.add(sha)
        rows["train"] = rows["train"] + [row for row, _, _ in added]

    # Unique-scene accounting: row counts overstate diversity; report both.
    def unique_scenes(split: str) -> int:
        return len({row["screen"] for row in rows[split]})

    counts = {split: len(rows[split]) for split in SPLITS}
    families = {
        split: {key: sum(1 for row in rows[split] if row["family"] == key)
                for key in sorted({row["family"] for row in rows[split]})}
        for split in SPLITS
    }

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
            source = origins[rel]
            destination = staging / rel
            if destination.exists():
                raise SystemExit(f"screen collision: {rel}")
            shutil.copyfile(source, destination)
            installed[rel] = digest(destination)
            if Path(rel).stem != installed[rel]:
                raise SystemExit(f"copied screen hash mismatch: {rel}")

    for split in SPLITS:
        with (staging / f"{split}.jsonl").open("w", encoding="utf-8") as handle:
            for row in rows[split]:
                handle.write(json.dumps(row, sort_keys=True, ensure_ascii=False) + "\n")
    shutil.copyfile(baseline / "system.txt", staging / "system.txt")

    manifest = {
        "schema": "mage-training-dataset-v1",
        "name": args.name,
        "baseline": {"path": str(baseline), "manifest_sha256": digest(baseline / "manifest.json"),
                     "counts": base_counts},
        "augmentation": addon_meta,
        "counts": counts,
        "added_rows": len(added),
        "unique_scenes": {split: unique_scenes(split) for split in SPLITS},
        "families": families,
        "screens": len(installed),
        "system_sha256": digest(staging / "system.txt"),
        "files": {split + ".jsonl": digest(staging / f"{split}.jsonl") for split in SPLITS},
        "rows_per_unique_scene": {split: round(counts[split] / unique_scenes(split), 2)
                                  for split in SPLITS},
        "claim": ("supervised single-click grounding rows; augmented rows are synthetic "
                  "screenshots, not verified live interactions"),
        "training_started": False,
        "deployment_changed": False,
    }
    (staging / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    staging.rename(out)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())

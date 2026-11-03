#!/usr/bin/env python3
"""Build an isolated train-only element-dropout positive augmentation set.

The source frozen dataset is read-only.  This script copies selected parent
screens, exact parent rows/label records/frame records and the source manifest
into a new artifact, then emits full-frame element-dropout variants.  It never
merges into the baseline, never touches validation/test rows, never emits a
negative/abstention example and never invokes a model, GPU job or network
service.
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import math
import shutil
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageStat

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import common  # noqa: E402

MAGE_ROOT = Path(os.environ.get("MAGE_ROOT") or Path.home() / "mage")
DEFAULT_SOURCE = MAGE_ROOT / "regime-recovery-v1/dataset-20261101-native-v1"
DEFAULT_OUTPUT = MAGE_ROOT / "dropout-positives-20261101-v1"

# Native families are small and all suitable labels are included.  Non-native
# families are represented proportionally but capped so one family cannot
# dominate the addition set.  These are selection quotas, not claims of
# evidence; the manifest reports actual generated counts.
NON_NATIVE_QUOTAS = {
    "replay-web": 70,
    "steam-navigation": 70,
    "shopping": 70,
    "shopping-transition": 70,
    "steam-synthetic": 70,
    "replay-desk": 69,
    "steam-store": 66,
}

CLAIM = "synthetic element dropout only; not a new verified native app interaction"


def fail(message: str) -> None:
    raise SystemExit(f"dropout-positives build failed: {message}")


def load_source(source: Path):
    manifest_path = source / "manifest.json"
    expected = common.EXPECTED_SOURCE_MANIFEST_SHA256
    before = common.sha256_file(manifest_path)
    if before != expected:
        fail(f"source manifest SHA256 {before} != expected {expected}")
    frames = json.loads((source / "frames.json").read_text(encoding="utf-8"))
    train_rows = common.read_jsonl_with_raw(source / "train.jsonl")
    labels = {}
    for record, raw in common.read_jsonl_with_raw(source / "labels.jsonl"):
        labels[record["id"]] = (record, raw)
    heldout_frames = [frame for frame in frames if frame.get("split") != "train"]
    heldout_image_hashes = {frame["image"] for frame in heldout_frames}
    heldout_pixel_hashes = set()
    for frame in heldout_frames:
        path = source / "screens" / f"{frame['image']}.png"
        if path.exists():
            with Image.open(path) as image:
                heldout_pixel_hashes.add(common.sha256_bytes(image.convert(common.RAW_PIXEL_MODE).tobytes()))
    heldout_groups = {frame["group"] for frame in heldout_frames}
    heldout_label_ids = {
        label_id for label_id, (record, _raw) in labels.items() if record.get("split") != "train"
    }
    return {
        "screen_pixel_cache": {},
        "manifest_sha256_before": before,
        "frames": frames,
        "train_rows": train_rows,
        "labels": labels,
        "heldout_image_hashes": heldout_image_hashes,
        "heldout_pixel_hashes": heldout_pixel_hashes,
        "heldout_groups": heldout_groups,
        "heldout_label_ids": heldout_label_ids,
    }


class PageCache:
    def __init__(self, source: Path):
        self.source = source
        self.cache: dict[str, object] = {}
        self.hashes: dict[str, str] = {}

    def get(self, relative: str):
        if relative in self.cache:
            return self.cache[relative]
        path = self.source / relative
        if not path.is_file() or not path.resolve().is_relative_to(self.source.resolve()):
            raise ValueError(f"unsafe/missing source page: {relative}")
        payload = json.loads(path.read_text(encoding="utf-8"))
        self.cache[relative] = payload
        self.hashes[relative] = common.sha256_file(path)
        return payload


def frame_candidates_for(row: dict, frames: list[dict], source: Path, pages: PageCache):
    screen_hash = Path(row["screen"]).name.removesuffix(".png")
    matches = [
        frame for frame in frames
        if frame.get("image") == screen_hash
        and frame.get("split") == "train"
        and frame.get("group") == row["group"]
        and frame.get("family") == row["family"]
    ]
    matches.sort(key=lambda f: (
        common.stable_key(common.SEED, f.get("state"), f.get("origin")),
        f.get("origin", {}).get("file", ""),
        int(f.get("origin", {}).get("index", -1)),
    ))
    chosen = []
    for frame in matches:
        try:
            page = pages.get(frame["origin"]["file"])
            page_record = page[frame["origin"]["index"]]
            matched = common.find_target_element(
                page_record.get("elements", []), row["box"], row["width"], row["height"]
            )
        except (KeyError, IndexError, TypeError, ValueError):
            matched = None
        if matched is not None:
            chosen.append((frame, page_record, matched))
    return chosen


def validate_parent(row: dict, raw: str, source: Path, data: dict, pages: PageCache):
    """Return a parent context or ``(None, reason)``.  Never guesses."""
    if row.get("split") != "train":
        return None, "row_not_train"
    if row.get("crop") is not None or row.get("zoom") != 1:
        return None, "not_full_frame"
    if not isinstance(row.get("width"), int) or not isinstance(row.get("height"), int):
        return None, "invalid_dimensions"
    if row["width"] < 2 or row["height"] < 2:
        return None, "invalid_dimensions"
    label_id = row.get("label_id")
    if label_id not in data["labels"]:
        return None, "missing_label_record"
    label_record, label_raw = data["labels"][label_id]
    if label_record.get("split") != "train":
        return None, "label_record_not_train"
    for key in ("screen", "target", "box", "family", "group", "entities"):
        if label_record.get(key) != row.get(key):
            return None, f"label_record_mismatch:{key}"
    origin = label_record.get("origin") or {}
    if origin.get("split") != "train":
        return None, "label_origin_not_train"
    if label_id in data["heldout_label_ids"]:
        return None, "label_id_in_heldout"
    if row["group"] in data["heldout_groups"]:
        return None, "group_in_heldout"
    screen_path = source / row["screen"]
    if not screen_path.is_file():
        return None, "missing_screen"
    screen_hash = screen_path.name.removesuffix(".png")
    if common.sha256_file(screen_path) != screen_hash:
        return None, "screen_filename_hash_mismatch"
    if screen_hash in data["heldout_image_hashes"]:
        return None, "screen_in_heldout"
    if screen_hash not in data["screen_pixel_cache"]:
        with Image.open(screen_path) as image:
            if image.mode != common.RAW_PIXEL_MODE:
                image = image.convert(common.RAW_PIXEL_MODE)
            data["screen_pixel_cache"][screen_hash] = (image.size, common.sha256_bytes(image.tobytes()))
    screen_size, parent_pixel_sha = data["screen_pixel_cache"][screen_hash]
    if screen_size != (row["width"], row["height"]):
        return None, "screen_dimensions_mismatch"
    if parent_pixel_sha in data["heldout_pixel_hashes"]:
        return None, "parent_pixels_in_heldout"
    try:
        if not common.click_inside_box(row):
            return None, "parent_click_outside_box"
    except ValueError:
        return None, "parent_answer_invalid"
    frame_options = frame_candidates_for(row, data["frames"], source, pages)
    if not frame_options:
        return None, "source_target_unmatched"
    frame, page_record, matched = frame_options[0]
    target_index, target_element, target_iou = matched
    if target_iou < 0.50:
        return None, "source_target_unmatched"
    if common.find_target_element(page_record.get("elements", []), row["box"], row["width"], row["height"]) is None:
        return None, "source_target_ambiguous"
    return {
        "row": row,
        "row_raw": raw,
        "row_sha256": common.sha256_bytes(raw.encode("utf-8")),
        "label": label_record,
        "label_raw": label_raw,
        "label_sha256": common.sha256_bytes(label_raw.encode("utf-8")),
        "frame": frame,
        "frame_sha256": common.sha256_json(frame),
        "source_page": {
            "file": frame["origin"]["file"],
            "index": int(frame["origin"]["index"]),
            "sha256": pages.hashes[frame["origin"]["file"]],
        },
        "page_record": page_record,
        "target_index": target_index,
        "target_element": target_element,
        "target_iou": target_iou,
        "screen_hash": screen_hash,
        "parent_path": screen_path,
        "parent_pixel_sha256": parent_pixel_sha,
    }, None


def control_type_for(ctx: dict) -> str:
    return common.control_type(ctx["row"]["target"], ctx["target_element"])


def stratified_select(candidates: list[dict], quota: int) -> list[dict]:
    """Deterministic proportional selection across control types."""
    if quota >= len(candidates):
        return sorted(candidates, key=lambda c: (
            control_type_for(c), common.stable_key(common.SEED, c["row"]["family"], c["row"]["label_id"])
        ))
    if quota <= 0:
        return []
    groups: dict[str, list[dict]] = collections.defaultdict(list)
    for candidate in candidates:
        groups[control_type_for(candidate)].append(candidate)
    for group in groups.values():
        group.sort(key=lambda c: common.stable_key(common.SEED, c["row"]["family"], c["row"]["label_id"]))
    types = sorted(groups, key=lambda name: (-len(groups[name]), common.stable_key(common.SEED, "type", name)))
    if quota < len(types):
        return [groups[name][0] for name in types[:quota]]
    total = sum(len(group) for group in groups.values())
    alloc = {name: max(1, int(math.floor(quota * len(groups[name]) / total))) for name in types}
    while sum(alloc.values()) > quota:
        reducible = [name for name in types if alloc[name] > 1]
        if not reducible:
            break
        name = max(reducible, key=lambda n: (alloc[n] - quota * len(groups[n]) / total, alloc[n]))
        alloc[name] -= 1
    while sum(alloc.values()) < quota:
        addable = [name for name in types if alloc[name] < len(groups[name])]
        if not addable:
            break
        name = max(addable, key=lambda n: (
            quota * len(groups[n]) / total - alloc[n],
            common.stable_key(common.SEED, "alloc", n),
        ))
        alloc[name] += 1
    chosen = []
    for name in types:
        chosen.extend(groups[name][:alloc[name]])
    selected_ids = {id(candidate) for candidate in chosen}
    if len(chosen) < quota:
        for candidate in sorted(candidates, key=lambda c: common.stable_key(common.SEED, c["row"]["family"], c["row"]["label_id"])):
            if id(candidate) in selected_ids:
                continue
            chosen.append(candidate)
            selected_ids.add(id(candidate))
            if len(chosen) >= quota:
                break
    chosen.sort(key=lambda c: (
        control_type_for(c), common.stable_key(common.SEED, c["row"]["family"], c["row"]["label_id"])
    ))
    return chosen[:quota]


def median_ring_color(image: Image.Image, box, width: int, height: int):
    x0, y0, x1, y1 = box
    ring = (max(0, x0 - 2), max(0, y0 - 2), min(width, x1 + 2), min(height, y1 + 2))
    if ring[0] >= ring[2] or ring[1] >= ring[3]:
        return (0, 0, 0)
    pixels = []
    for y in range(ring[1], ring[3]):
        for x in range(ring[0], ring[2]):
            if x0 <= x < x1 and y0 <= y < y1:
                continue
            pixels.append(image.getpixel((x, y))[:3])
    if not pixels:
        return (0, 0, 0)
    channels = []
    for channel in range(3):
        values = sorted(pixel[channel] for pixel in pixels)
        channels.append(values[len(values) // 2])
    return tuple(channels)


def diff_stats(parent: Image.Image, generated: Image.Image, dropped_boxes: list[tuple]):
    diff = ImageChops.difference(parent.convert("RGB"), generated.convert("RGB"))
    red, green, blue = diff.split()
    max_diff = ImageChops.lighter(ImageChops.lighter(red, green), blue)
    changed = max_diff.point(lambda value: 255 if value else 0)
    allowed = Image.new("L", generated.size, 0)
    draw = ImageDraw.Draw(allowed)
    for x0, y0, x1, y1 in dropped_boxes:
        draw.rectangle([x0, y0, max(x0, x1 - 1), max(y0, y1 - 1)], fill=255)
    leak = ImageChops.subtract(changed, allowed)
    changed_count = changed.histogram()[255]
    return changed, leak, changed_count


def render_variant(ctx: dict, drop_spec: dict, staging: Path, used_pixel_hashes: dict):
    row = ctx["row"]
    width, height = row["width"], row["height"]
    elements = ctx["page_record"].get("elements", [])
    candidates_by_index = {c["element_index"]: c for c in ctx["candidates"]}
    drops = [candidates_by_index[d["element_index"]] for d in drop_spec["drops"]]
    if not drops:
        return None, "empty_drop_set"
    dropped_boxes = [tuple(drop["box"]) for drop in drops]
    if any(box[2] <= box[0] or box[3] <= box[1] for box in dropped_boxes):
        return None, "degenerate_drop_box"
    for left in range(len(dropped_boxes)):
        for right in range(left + 1, len(dropped_boxes)):
            if common.boxes_intersect(dropped_boxes[left], dropped_boxes[right]):
                return None, "overlapping_drop_boxes"
    protected = [tuple(region["box"]) for region in ctx["protected"]]
    for dropped in dropped_boxes:
        if any(common.boxes_intersect(dropped, guard) for guard in protected):
            return None, "drop_overlaps_protected"
    variant_id = common.sha256_bytes((
        f"{common.SEED}|{row['label_id']}|{drop_spec['scope']}|"
        + ",".join(str(drop["element_index"]) for drop in drops)
        + "|" + ",".join(str(v) for box in dropped_boxes for v in box)
        + "|median_ring_v1"
    ).encode("utf-8"))
    out_path = staging / "screens" / f"{variant_id}.png"
    if out_path.exists():
        return None, "duplicate_variant_id"
    with Image.open(ctx["parent_path"]) as source_image:
        parent = source_image.convert("RGB")
    generated = parent.copy()
    for drop in drops:
        color = median_ring_color(parent, tuple(drop["box"]), width, height)
        draw = ImageDraw.Draw(generated)
        x0, y0, x1, y1 = drop["box"]
        draw.rectangle([x0, y0, max(x0, x1 - 1), max(y0, y1 - 1)], fill=color)
    changed, leak, changed_count = diff_stats(parent, generated, dropped_boxes)
    if leak.getbbox() is not None:
        return None, "change_outside_drop_boxes"
    if changed_count <= 0:
        return None, "no_pixel_change"
    changed_fraction = changed_count / (width * height)
    if changed_fraction > 0.35:
        return None, "drop_area_too_large"
    stddev = ImageStat.Stat(generated.convert("L")).stddev[0]
    if stddev < 1.0:
        return None, "blank_or_flat_output"
    pixel_sha = common.sha256_bytes(generated.tobytes())
    if pixel_sha in used_pixel_hashes:
        return None, "duplicate_generated_pixels"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    generated.save(out_path, format="PNG", optimize=False)
    used_pixel_hashes[pixel_sha] = variant_id
    remaining = 0
    for index, element in enumerate(elements):
        if index == ctx["target_index"]:
            continue
        element_box = common.pixel_box(element.get("box"), width, height, tolerance=2.0)
        if element_box is None:
            continue
        if any(common.boxes_intersect(element_box, dropped) for dropped in dropped_boxes):
            continue
        remaining += 1
    augmentation = {
        "kind": "element_dropout",
        "scope": drop_spec["scope"],
        "synthetic": True,
        "claim": CLAIM,
        "seed": common.SEED,
        "fill": "median_border_rgb",
        "parent_screen": row["screen"],
        "parent_screen_sha256": ctx["screen_hash"],
        "parent_pixel_sha256": ctx["parent_pixel_sha256"],
        "parent_row_sha256": ctx["row_sha256"],
        "parent_label_sha256": ctx["label_sha256"],
        "parent_frame_sha256": ctx["frame_sha256"],
        "source_page": ctx["source_page"],
        "target_element_index": ctx["target_index"],
        "target_match_iou": round(ctx["target_iou"], 6),
        "dropped": [{
            "element_index": drop["element_index"],
            "source_box": drop["source_box"],
            "fill_box": drop["box"],
            "role": drop["role"],
            "tag": drop["tag"],
            "text": drop["text"],
        } for drop in drops],
        "protected": ctx["protected"],
        "remaining_distractors": remaining,
        "changed_pixels": changed_count,
        "changed_fraction": round(changed_fraction, 8),
    }
    variant_row = {
        "screen": f"screens/{variant_id}.png",
        "target": row["target"],
        "box": row["box"],
        "family": row["family"],
        "group": row["group"],
        "split": "train",
        "width": width,
        "height": height,
        "crop": None,
        "zoom": 1,
        "answer": row["answer"],
        "entities": row["entities"],
        "source": row.get("source"),
        "label_id": row["label_id"],
        "parent_label_id": row["label_id"],
        "variant_id": variant_id,
        "synthetic": True,
        "augmentation": augmentation,
    }
    return variant_row, None


def copy_exact(source_path: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source_path, destination)


def write_artifact_readme(path: Path, counts: dict, source: Path) -> None:
    text = f"""# Mage element-dropout positives, isolated train add-on

This directory is a synthetic augmentation artifact, not a new native-app
interaction capture and not a model result.

- Source frozen dataset: `{source}`
- Source manifest SHA256: `{common.EXPECTED_SOURCE_MANIFEST_SHA256}`
- Parents selected: {counts['parents_selected']}
- Parents with generated variants: {counts['parents_with_variants']}
- Generated variants: {counts['variants']}
- Unchanged parent controls/provenance: {counts['controls']}
- No validation/test rows were used, no negatives were emitted, and the frozen
  baseline was not modified or merged.

The generated rows keep the parent screenshot geometry, target, box, answer and
label identity.  Only genuine source-element boxes outside protected target
context were filled with local border-median background.  See `manifest.json`
and `AUDIT.json` (written by `verify.py`) for hashes and checks.
"""
    path.write_text(text, encoding="utf-8")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args(argv)
    source = args.source.resolve()
    output = args.output
    if not source.is_dir():
        fail(f"source dataset missing: {source}")
    if output.exists():
        fail(f"output already exists (fresh output required): {output}")
    staging = output.with_name(output.name + ".building")
    if staging.exists():
        fail(f"staging path already exists: {staging}")

    data = load_source(source)
    pages = PageCache(source)
    exclusions = []
    clean = []
    for row, raw in data["train_rows"]:
        if row.get("crop") is not None:
            continue
        context, reason = validate_parent(row, raw, source, data, pages)
        if context is None:
            exclusions.append({
                "scope": "parent_selection",
                "parent_label_id": row.get("label_id"),
                "screen": row.get("screen"),
                "family": row.get("family"),
                "group": row.get("group"),
                "reason": reason,
            })
            continue
        context["control_type"] = control_type_for(context)
        clean.append(context)

    families = collections.defaultdict(list)
    for context in clean:
        families[context["row"]["family"]].append(context)
    selected = []
    selection_quota = {}
    not_selected = collections.Counter()
    for family in sorted(families):
        candidates = families[family]
        if family.startswith("native-"):
            quota = len(candidates)
        else:
            quota = min(len(candidates), NON_NATIVE_QUOTAS.get(family, len(candidates)))
        selection_quota[family] = quota
        chosen = stratified_select(candidates, quota)
        selected.extend(chosen)
        not_selected[family] = len(candidates) - len(chosen)
    if len(selected) > common.MAX_PARENTS:
        fail(f"selected {len(selected)} parents exceeds cap {common.MAX_PARENTS}")

    staging.mkdir(parents=True)
    (staging / "screens").mkdir()
    (staging / "parents" / "images").mkdir(parents=True)
    (staging / "provenance" / "source_pages").mkdir(parents=True)

    variant_rows = []
    control_rows = []
    selected_parent_rows = []
    selected_label_rows = []
    selected_frame_rows = []
    selected_frame_keys = set()
    copied_parent_images = set()
    copied_source_pages = {}
    used_pixel_hashes: dict[str, str] = {}
    parents_with_variants = set()
    parents_one_variant = set()
    parents_zero_variants = set()

    for context in selected:
        row = context["row"]
        label = context["label"]
        frame = context["frame"]
        selected_parent_rows.append(context["row_raw"])
        selected_label_rows.append(context["label_raw"])
        frame_key = common.sha256_json(frame)
        if frame_key not in selected_frame_keys:
            selected_frame_keys.add(frame_key)
            selected_frame_rows.append(frame)
        parent_image_name = f"{context['screen_hash']}.png"
        if parent_image_name not in copied_parent_images:
            copy_exact(context["parent_path"], staging / "parents" / "images" / parent_image_name)
            copied_parent_images.add(parent_image_name)
        copied_source_pages[context["source_page"]["file"]] = context["source_page"]["sha256"]
        control_rows.append({
            "screen": f"parents/images/{parent_image_name}",
            "original_screen": row["screen"],
            "target": row["target"],
            "box": row["box"],
            "family": row["family"],
            "group": row["group"],
            "split": "train",
            "width": row["width"],
            "height": row["height"],
            "crop": None,
            "zoom": 1,
            "answer": row["answer"],
            "entities": row["entities"],
            "source": row.get("source"),
            "label_id": row["label_id"],
            "parent_label_id": row["label_id"],
            "control": True,
            "synthetic": False,
            "augmentation": None,
        })
        context["candidates"], context["protected"] = common.candidate_elements(
            context["page_record"].get("elements", []),
            context["target_index"],
            row["box"],
            row["target"],
            row["width"],
            row["height"],
        )
        drop_sets = common.choose_drop_sets(
            context["candidates"], row["width"], row["height"],
            len(context["page_record"].get("elements", [])),
        )
        generated_for_parent = 0
        for drop_spec in drop_sets:
            variant_row, reason = render_variant(context, drop_spec, staging, used_pixel_hashes)
            if variant_row is None:
                exclusions.append({
                    "scope": "variant_generation",
                    "parent_label_id": row["label_id"],
                    "family": row["family"],
                    "group": row["group"],
                    "scope_detail": drop_spec.get("scope"),
                    "reason": reason,
                })
                continue
            variant_rows.append(variant_row)
            generated_for_parent += 1
        if generated_for_parent:
            parents_with_variants.add(row["label_id"])
            if generated_for_parent == 1:
                parents_one_variant.add(row["label_id"])
        else:
            parents_zero_variants.add(row["label_id"])
            if not any(
                e.get("scope") == "variant_generation" and e.get("parent_label_id") == row["label_id"]
                for e in exclusions
            ):
                exclusions.append({
                    "scope": "variant_generation",
                    "parent_label_id": row["label_id"],
                    "family": row["family"],
                    "group": row["group"],
                    "reason": "no_safe_dropout_candidate",
                })

    # Exact parent records and unchanged controls/provenance.
    (staging / "parents" / "rows.jsonl").write_text(
        "\n".join(selected_parent_rows) + "\n", encoding="utf-8"
    )
    (staging / "parents" / "labels.jsonl").write_text(
        "\n".join(selected_label_rows) + "\n", encoding="utf-8"
    )
    with (staging / "parents" / "frames.jsonl").open("w", encoding="utf-8") as handle:
        for frame in sorted(selected_frame_rows, key=lambda f: (f.get("family", ""), f.get("group", ""), f.get("image", ""))):
            handle.write(json.dumps(frame, sort_keys=True, ensure_ascii=False) + "\n")
    common.write_jsonl(staging / "controls.jsonl", control_rows)
    common.write_jsonl(staging / "train_dropout.jsonl", variant_rows)

    # Source provenance copies.  These are byte copies, never links.
    copy_exact(source / "manifest.json", staging / "provenance" / "source_manifest.json")
    copy_exact(source / "system.txt", staging / "provenance" / "system.txt")
    copy_exact(source / "provenance.json", staging / "provenance" / "provenance.json")
    source_page_records = []
    for relative in sorted(copied_source_pages):
        source_path = source / relative
        copy_exact(source_path, staging / "provenance" / "source_pages" / Path(relative).name)
        source_page_records.append({
            "source_relative_path": relative,
            "artifact_path": f"provenance/source_pages/{Path(relative).name}",
            "sha256": copied_source_pages[relative],
        })
    common.write_jsonl(staging / "provenance" / "source_pages.jsonl", source_page_records)
    common.write_jsonl(staging / "exclusions.jsonl", exclusions)

    by_family = collections.Counter(row["family"] for row in variant_rows)
    by_scope = collections.Counter(row["augmentation"]["scope"] for row in variant_rows)
    # Control type of a variant is the parent target's control type.  Recompute
    # from the selected contexts rather than trusting a row field.
    context_by_label = {c["row"]["label_id"]: c for c in selected}
    by_control = collections.Counter(
        context_by_label[row["label_id"]]["control_type"] for row in variant_rows
    )
    parents_by_family = collections.Counter(c["row"]["family"] for c in selected)
    parents_by_control = collections.Counter(c["control_type"] for c in selected)
    by_family_scope = collections.Counter(
        (row["family"], row["augmentation"]["scope"]) for row in variant_rows
    )
    exclusion_reasons = collections.Counter(e["reason"] for e in exclusions)

    after = common.sha256_file(source / "manifest.json")
    if after != common.EXPECTED_SOURCE_MANIFEST_SHA256:
        fail(f"source manifest changed during build: {after}")
    if after != data["manifest_sha256_before"]:
        fail("source manifest SHA256 changed during build")

    counts = {
        "parents_selected": len(selected),
        "parents_with_variants": len(parents_with_variants),
        "parents_zero_variants": len(parents_zero_variants),
        "parents_one_variant": len(parents_one_variant),
        "variants": len(variant_rows),
        "controls": len(control_rows),
    }
    write_artifact_readme(staging / "README.md", counts, source)

    # Manifest file list excludes itself and the later verifier audit.
    files = {}
    screens = {}
    parent_screens = {}
    for path in sorted(staging.rglob("*")):
        if not path.is_file():
            continue
        relative = str(path.relative_to(staging))
        if relative in ("manifest.json", "AUDIT.json"):
            continue
        digest = common.sha256_file(path)
        files[relative] = digest
        if relative.startswith("screens/"):
            screens[relative] = digest
        if relative.startswith("parents/images/"):
            parent_screens[relative] = digest
    manifest = {
        "version": "mage-element-dropout-positives-20261101-v1",
        "role": "Mage hands only; AngelX primary driver plans",
        "scope": "train-only isolated synthetic element-dropout positive add-on",
        "claims": {
            "synthetic_augmentation": True,
            "new_native_interaction": False,
            "negative_or_abstention_labels": False,
            "heldout_changed": False,
            "model_run_or_training": False,
            "baseline_merged": False,
        },
        "source": {
            "dataset": str(source),
            "manifest_sha256_expected": common.EXPECTED_SOURCE_MANIFEST_SHA256,
            "manifest_sha256_before": data["manifest_sha256_before"],
            "manifest_sha256_after": after,
            "system_sha256": common.sha256_file(source / "system.txt"),
            "copied_manifest_path": "provenance/source_manifest.json",
        },
        "selection": {
            "seed": common.SEED,
            "max_parents": common.MAX_PARENTS,
            "max_variants_per_parent": common.MAX_VARIANTS_PER_PARENT,
            "quotas": selection_quota,
            "not_selected_due_to_quota": dict(sorted(not_selected.items())),
            "parents_by_family": dict(sorted(parents_by_family.items())),
            "parents_by_control_type": dict(sorted(parents_by_control.items())),
        },
        "counts": {
            **counts,
            "by_family": dict(sorted(by_family.items())),
            "by_control_type": dict(sorted(by_control.items())),
            "by_scope": dict(sorted(by_scope.items())),
            "by_family_scope": {f"{family}/{scope}": n for (family, scope), n in sorted(by_family_scope.items())},
            "exclusions": len(exclusions),
            "exclusion_reasons": dict(sorted(exclusion_reasons.items())),
        },
        "holdout": {
            "validation_test_frame_count": sum(1 for f in data["frames"] if f.get("split") != "train"),
            "image_sha256": sorted(data["heldout_image_hashes"]),
            "pixel_sha256": sorted(data["heldout_pixel_hashes"]),
        },
        "screens": screens,
        "parent_screens": parent_screens,
        "source_pages": source_page_records,
        "files": files,
        "limitations": [
            "These are synthetic element-dropout edits of frozen train screenshots; they are not new verified native app interactions.",
            "Only genuine source element boxes outside protected target context were dropped; no background-only dropout variants were emitted.",
            "The edit fills dropped boxes with a local border-median colour and may look artificial; it is augmentation, not a new environment.",
            "Selection is a bounded deterministic representative subset, not a claim that every family/control type is fully covered.",
            "No model training, inference, GPU job, cloud upload, serving change or heldout merge was performed or is claimed.",
        ],
    }
    (staging / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    manifest_sha = common.sha256_file(staging / "manifest.json")
    try:
        staging.rename(output)
    except OSError as exc:
        fail(f"could not publish artifact: {exc}")
    print(json.dumps({
        "artifact": str(output),
        "manifest_sha256": manifest_sha,
        "counts": counts,
        "by_family": dict(sorted(by_family.items())),
        "by_control_type": dict(sorted(by_control.items())),
        "by_scope": dict(sorted(by_scope.items())),
        "exclusions": len(exclusions),
        "source_manifest_sha256": after,
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

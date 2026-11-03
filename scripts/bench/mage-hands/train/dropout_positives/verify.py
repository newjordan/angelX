#!/usr/bin/env python3
"""End-to-end audit of the generated element-dropout artifact.

Fail closed.  The audit decodes every generated image and every copied parent
image, recomputes hashes and pixels, re-derives target/context protection from
the frozen source element tree, and rejects any output that changes protected
pixels, uses heldout ancestry, duplicates a target image, or disagrees with the
artifact manifest.
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import re
import sys
import traceback
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageStat

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import common  # noqa: E402

MAGE_ROOT = Path(os.environ.get("MAGE_ROOT") or Path.home() / "mage")
DEFAULT_ARTIFACT = MAGE_ROOT / "dropout-positives-20261101-v1"
DEFAULT_SOURCE = MAGE_ROOT / "regime-recovery-v1/dataset-20261101-native-v1"
CAUTION_PHRASE = re.compile(r"\b(abstain|target absent|not visible|cannot see|no target|negative)\b", re.I)


class AuditFailure(Exception):
    pass


def require(condition, message: str) -> None:
    if not condition:
        raise AuditFailure(message)


def load_json(path: Path):
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def safe_child(root: Path, relative: str) -> Path:
    path = (root / relative).resolve()
    root_resolved = root.resolve()
    require(path.is_relative_to(root_resolved), f"unsafe path escapes artifact/source: {relative}")
    return path


def raw_pixel_sha(image: Image.Image) -> str:
    return common.sha256_bytes(image.convert(common.RAW_PIXEL_MODE).tobytes())


def source_context(source: Path):
    source_manifest = load_json(source / "manifest.json")
    frames = load_json(source / "frames.json")
    source_rows = {}
    for row, raw in common.read_jsonl_with_raw(source / "train.jsonl"):
        if row.get("crop") is None:
            key = row["label_id"]
            require(key not in source_rows, f"duplicate full-frame source row for label {key}")
            source_rows[key] = (row, raw)
    labels = {}
    for record, raw in common.read_jsonl_with_raw(source / "labels.jsonl"):
        require(record["id"] not in labels, f"duplicate source label record {record['id']}")
        labels[record["id"]] = (record, raw)
    heldout_frames = [frame for frame in frames if frame.get("split") != "train"]
    heldout_images = {frame["image"] for frame in heldout_frames}
    heldout_pixels = set()
    for frame in heldout_frames:
        path = source / "screens" / f"{frame['image']}.png"
        if path.exists():
            with Image.open(path) as image:
                heldout_pixels.add(raw_pixel_sha(image))
    heldout_groups = {frame["group"] for frame in heldout_frames}
    heldout_labels = {
        label_id for label_id, (record, _raw) in labels.items() if record.get("split") != "train"
    }
    frame_by_sha = {}
    for frame in frames:
        frame_by_sha[common.sha256_json(frame)] = frame
    return {
        "manifest": source_manifest,
        "frames": frames,
        "frame_by_sha": frame_by_sha,
        "source_rows": source_rows,
        "labels": labels,
        "heldout_images": heldout_images,
        "heldout_pixels": heldout_pixels,
        "heldout_groups": heldout_groups,
        "heldout_labels": heldout_labels,
    }


def artifact_context(artifact: Path, source: Path):
    manifest = load_json(artifact / "manifest.json")
    rows = common.read_jsonl(artifact / "train_dropout.jsonl")
    controls = common.read_jsonl(artifact / "controls.jsonl")
    parent_rows = {}
    parent_row_raw = {}
    for row, raw in common.read_jsonl_with_raw(artifact / "parents" / "rows.jsonl"):
        label_id = row.get("label_id")
        require(label_id not in parent_rows, f"duplicate parent row {label_id}")
        parent_rows[label_id] = row
        parent_row_raw[label_id] = raw
    parent_labels = {}
    for record, raw in common.read_jsonl_with_raw(artifact / "parents" / "labels.jsonl"):
        label_id = record.get("id")
        require(label_id not in parent_labels, f"duplicate parent label {label_id}")
        parent_labels[label_id] = (record, raw)
    parent_frames = common.read_jsonl(artifact / "parents" / "frames.jsonl")
    exclusions = common.read_jsonl(artifact / "exclusions.jsonl")
    source_pages = common.read_jsonl(artifact / "provenance" / "source_pages.jsonl")
    return {
        "manifest": manifest,
        "rows": rows,
        "controls": controls,
        "parent_rows": parent_rows,
        "parent_row_raw": parent_row_raw,
        "parent_labels": parent_labels,
        "parent_frames": parent_frames,
        "exclusions": exclusions,
        "source_pages": source_pages,
    }


def check_file_inventory(artifact: Path, manifest: dict):
    expected_files = manifest.get("files", {})
    expected = set(expected_files)
    actual = set()
    for path in sorted(artifact.rglob("*")):
        if path.is_symlink():
            raise AuditFailure(f"artifact contains a symlink: {path.relative_to(artifact)}")
        if not path.is_file():
            continue
        relative = str(path.relative_to(artifact))
        if relative in ("manifest.json", "AUDIT.json"):
            continue
        actual.add(relative)
        require(relative in expected, f"unmanifested artifact file: {relative}")
        digest = common.sha256_file(path)
        require(digest == expected_files[relative], f"artifact file hash mismatch: {relative}")
    missing = expected - actual
    require(not missing, f"manifest lists missing files: {sorted(missing)[:5]}")
    for relative, digest in manifest.get("screens", {}).items():
        require(relative in expected, f"screen not in file list: {relative}")
        require(expected_files[relative] == digest, f"screen manifest hash mismatch: {relative}")
    for relative, digest in manifest.get("parent_screens", {}).items():
        require(relative in expected, f"parent screen not in file list: {relative}")
        require(expected_files[relative] == digest, f"parent screen manifest hash mismatch: {relative}")
    return {"files": len(actual), "screens": len(manifest.get("screens", {})),
            "parent_screens": len(manifest.get("parent_screens", {}))}


def check_source_manifest(source: Path, artifact: Path, manifest: dict, src: dict):
    expected = common.EXPECTED_SOURCE_MANIFEST_SHA256
    observed = common.sha256_file(source / "manifest.json")
    require(observed == expected, f"source manifest SHA256 changed: {observed}")
    copied = common.sha256_file(artifact / "provenance" / "source_manifest.json")
    require(copied == expected, "artifact source manifest copy is not byte-identical")
    require(manifest["source"]["manifest_sha256_expected"] == expected, "manifest expected SHA mismatch")
    require(manifest["source"]["manifest_sha256_before"] == expected, "manifest before SHA mismatch")
    require(manifest["source"]["manifest_sha256_after"] == expected, "manifest after SHA mismatch")
    require(manifest["source"]["system_sha256"] == common.sha256_file(source / "system.txt"),
            "source system.txt digest mismatch")
    # Check the source manifest's own file table for the records this build used.
    source_manifest = src["manifest"]
    for relative in ("train.jsonl", "labels.jsonl", "frames.json", "provenance.json", "system.txt"):
        declared = source_manifest.get("files", {}).get(relative)
        if declared is not None:
            require(common.sha256_file(source / relative) == declared,
                    f"frozen source file digest mismatch: {relative}")
    return {"source_manifest_sha256": observed}


def check_source_pages(source: Path, artifact: Path, act: dict):
    records = act["source_pages"]
    require(bool(records), "no copied source pages")
    seen = set()
    for record in records:
        relative = record["source_relative_path"]
        require(relative not in seen, f"duplicate source page record: {relative}")
        seen.add(relative)
        source_path = safe_child(source, relative)
        require(source_path.is_file(), f"missing source page: {relative}")
        require(common.sha256_file(source_path) == record["sha256"],
                f"source page digest mismatch: {relative}")
        artifact_path = safe_child(artifact, record["artifact_path"])
        require(artifact_path.is_file(), f"missing copied source page: {record['artifact_path']}")
        require(common.sha256_file(artifact_path) == record["sha256"],
                f"copied source page digest mismatch: {record['artifact_path']}")
    page_dir = artifact / "provenance" / "source_pages"
    actual_copies = {
        str(path.relative_to(artifact)) for path in page_dir.glob("*") if path.is_file()
    }
    recorded_copies = {record["artifact_path"] for record in records}
    require(actual_copies == recorded_copies, "source page copy directory does not match provenance records")
    return {"source_pages": len(records)}


def exact_parent_check(label_id: str, row: dict, act: dict, src: dict, artifact: Path, source: Path):
    require(label_id in src["source_rows"], f"variant label_id not in source train full-frame rows: {label_id}")
    source_row, source_raw = src["source_rows"][label_id]
    require(label_id in act["parent_rows"], f"variant parent row missing from artifact: {label_id}")
    artifact_parent = act["parent_rows"][label_id]
    artifact_raw = act["parent_row_raw"][label_id]
    require(artifact_raw == source_raw, f"artifact parent row is not the exact source line: {label_id}")
    require(artifact_parent == source_row, f"artifact parent row object mismatch: {label_id}")
    require(source_row.get("split") == "train", f"parent row is not train: {label_id}")
    require(source_row.get("crop") is None and source_row.get("zoom") == 1,
            f"parent row is not full-frame: {label_id}")
    require(label_id in act["parent_labels"], f"parent label record missing: {label_id}")
    label_record, label_raw = act["parent_labels"][label_id]
    require(label_id in src["labels"], f"parent label not in frozen source: {label_id}")
    source_label_record, source_label_raw = src["labels"][label_id]
    require(label_raw == source_label_raw, f"artifact parent label is not the exact source line: {label_id}")
    require(label_record == source_label_record, f"artifact parent label object mismatch: {label_id}")
    require(source_label_record.get("split") == "train", f"parent label is not train: {label_id}")
    require((source_label_record.get("origin") or {}).get("split") == "train",
            f"parent label origin is not train: {label_id}")
    require(label_id not in src["heldout_labels"], f"parent label is in heldout: {label_id}")
    require(source_row["group"] not in src["heldout_groups"], f"parent group is in heldout: {source_row['group']}")
    screen_hash = Path(source_row["screen"]).name.removesuffix(".png")
    require(screen_hash not in src["heldout_images"], f"parent screen is in heldout: {screen_hash}")
    parent_image_path = artifact / "parents" / "images" / f"{screen_hash}.png"
    require(parent_image_path.is_file(), f"copied parent image missing: {parent_image_path.name}")
    require(common.sha256_file(parent_image_path) == screen_hash,
            f"copied parent image hash does not match its filename: {parent_image_path.name}")
    source_screen = source / source_row["screen"]
    require(source_screen.is_file(), f"source parent screen missing: {source_row['screen']}")
    require(common.sha256_file(source_screen) == screen_hash, "source parent image filename/hash mismatch")
    with Image.open(parent_image_path) as parent_image:
        require(parent_image.size == (source_row["width"], source_row["height"]),
                f"parent image dimensions mismatch: {label_id}")
        require(raw_pixel_sha(parent_image) not in src["heldout_pixels"],
                f"parent pixels match a heldout image: {label_id}")
    frame_matches = [
        frame for frame in act["parent_frames"]
        if frame.get("image") == screen_hash
        and frame.get("group") == source_row["group"]
        and frame.get("family") == source_row["family"]
        and frame.get("split") == "train"
    ]
    require(frame_matches, f"parent frame record missing or ancestry-mismatched: {label_id}")
    for frame in frame_matches:
        require(common.sha256_json(frame) in src["frame_by_sha"],
                f"parent frame is not the exact frozen source frame: {label_id}")
        require(src["frame_by_sha"][common.sha256_json(frame)] == frame,
                f"parent frame object mismatch: {label_id}")
    return source_row, source_label_record, source_raw, frame_matches


def check_parents(artifact: Path, source: Path, act: dict, src: dict):
    """Check every selected parent, including parents that yielded no variant."""
    require(act["parent_rows"], "no selected parent rows")
    for label_id in act["parent_rows"]:
        exact_parent_check(label_id, act["parent_rows"][label_id], act, src, artifact, source)
    seen_frames = set()
    for frame in act["parent_frames"]:
        digest = common.sha256_json(frame)
        require(digest in src["frame_by_sha"], f"artifact parent frame is not in frozen source: {digest}")
        require(src["frame_by_sha"][digest] == frame, f"artifact parent frame changed: {digest}")
        require(frame.get("split") == "train", f"artifact parent frame is not train: {digest}")
        seen_frames.add(digest)
    return {"parents_checked": len(act["parent_rows"]), "parent_frames": len(seen_frames)}


def check_controls(artifact: Path, act: dict, src: dict):
    controls = act["controls"]
    require(len(controls) == len(act["parent_rows"]), "controls/parents count mismatch")
    seen = set()
    for control in controls:
        label_id = control.get("label_id")
        require(label_id not in seen, f"duplicate control for {label_id}")
        seen.add(label_id)
        require(control.get("control") is True, f"control flag missing for {label_id}")
        require(control.get("augmentation") is None and control.get("synthetic") is False,
                f"control is marked as augmentation: {label_id}")
        require(label_id in act["parent_rows"], f"control has no parent row: {label_id}")
        parent = act["parent_rows"][label_id]
        for key in ("target", "box", "family", "group", "split", "width", "height",
                    "answer", "entities", "label_id", "source"):
            require(control.get(key) == parent.get(key), f"control changed {key} for {label_id}")
        require(control.get("crop") is None and control.get("zoom") == 1, f"control not full-frame: {label_id}")
        screen_hash = Path(parent["screen"]).name.removesuffix(".png")
        require(control.get("screen") == f"parents/images/{screen_hash}.png",
                f"control screen path mismatch: {label_id}")
        require((artifact / control["screen"]).is_file(), f"control parent image missing: {label_id}")
    require(seen == set(act["parent_rows"]), "controls do not cover exactly the selected parents")
    return {"controls": len(controls)}


def diff_mask_and_leak(parent: Image.Image, generated: Image.Image, dropped_boxes: list[tuple]):
    diff = ImageChops.difference(parent.convert("RGB"), generated.convert("RGB"))
    red, green, blue = diff.split()
    max_diff = ImageChops.lighter(ImageChops.lighter(red, green), blue)
    changed = max_diff.point(lambda value: 255 if value else 0)
    allowed = Image.new("L", generated.size, 0)
    draw = ImageDraw.Draw(allowed)
    for x0, y0, x1, y1 in dropped_boxes:
        draw.rectangle([x0, y0, max(x0, x1 - 1), max(y0, y1 - 1)], fill=255)
    leak = ImageChops.subtract(changed, allowed)
    return changed, leak, changed.histogram()[255]


def check_variant(label_id: str, row: dict, parent_row: dict, label_record: dict,
                  frame_matches: list[dict], page: dict, parent_image: Image.Image,
                  artifact: Path, src: dict, generated_pixel_hashes: dict,
                  generated_file_hashes: dict):
    require(row.get("split") == "train", "variant split is not train")
    require(row.get("synthetic") is True, "variant is not marked synthetic")
    require(row.get("crop") is None and row.get("zoom") == 1, "variant is not a full-frame row")
    require(row.get("parent_label_id") == row.get("label_id") == label_id, "variant parent label mismatch")
    require(row.get("target") == parent_row["target"], "variant target changed")
    require(row.get("box") == parent_row["box"], "variant box changed")
    require(row.get("family") == parent_row["family"], "variant family changed")
    require(row.get("group") == parent_row["group"], "variant group changed")
    require(row.get("width") == parent_row["width"], "variant width changed")
    require(row.get("height") == parent_row["height"], "variant height changed")
    require(row.get("answer") == parent_row["answer"], "variant answer changed")
    require(row.get("entities") == parent_row["entities"], "variant entities changed")
    require(row.get("source") == parent_row.get("source"), "variant source changed")
    require(not CAUTION_PHRASE.search(str(row.get("target", ""))), "variant target looks negative/abstention")
    require(isinstance(row.get("variant_id"), str) and len(row["variant_id"]) == 64,
            "variant_id is not a sha256 hex string")
    require(row.get("screen") == f"screens/{row['variant_id']}.png",
            "variant screen path does not match variant_id")
    try:
        common.parse_click(row["answer"])
        require(common.click_inside_box(row), "variant click is outside its original box")
    except Exception as exc:
        raise AuditFailure(f"variant click contract invalid: {exc}")
    width, height = row["width"], row["height"]
    require(isinstance(width, int) and isinstance(height, int) and width >= 2 and height >= 2,
            "variant dimensions invalid")

    augmentation = row.get("augmentation") or {}
    require(augmentation.get("kind") == "element_dropout", "variant augmentation kind is not element_dropout")
    require(augmentation.get("synthetic") is True, "variant augmentation is not marked synthetic")
    require(augmentation.get("claim") == "synthetic element dropout only; not a new verified native app interaction",
            "variant synthetic claim changed")
    require(augmentation.get("scope") in ("compact", "context_group"), "variant scope invalid")
    require(augmentation.get("seed") == common.SEED, "variant seed changed")
    require(augmentation.get("fill") == "median_border_rgb", "variant fill method changed")
    require(augmentation.get("parent_screen") == parent_row["screen"], "variant parent screen changed")
    parent_screen_hash = Path(parent_row["screen"]).name.removesuffix(".png")
    require(augmentation.get("parent_screen_sha256") == parent_screen_hash,
            "variant parent screen digest changed")
    require(augmentation.get("parent_pixel_sha256") == raw_pixel_sha(parent_image),
            "variant parent pixel digest changed")
    require(augmentation.get("parent_row_sha256") == common.sha256_bytes(act_raw(parent_row, src)),
            "parent row digest mismatch")
    require(augmentation.get("parent_label_sha256") == common.sha256_bytes(act_raw(label_record, src, kind="label")),
            "parent label digest mismatch")

    page_record = page
    elements = page_record.get("elements", [])
    matched = common.find_target_element(elements, parent_row["box"], width, height)
    require(matched is not None, "source target element no longer uniquely matches")
    target_index, target_element, target_iou = matched
    require(target_index == augmentation.get("target_element_index"), "target element index changed")
    require(abs(round(target_iou, 6) - augmentation.get("target_match_iou", -1)) < 1e-6,
            "target match IoU changed")
    recomputed_protected = common.protected_regions(
        elements, target_index, parent_row["box"], parent_row["target"], width, height
    )
    stored_protected = augmentation.get("protected") or []
    require(stored_protected == recomputed_protected, "protected regions changed or not re-derivable")
    protected_boxes = [tuple(region["box"]) for region in recomputed_protected]
    target_px = common.pixel_box(parent_row["box"], width, height, tolerance=0.0)
    require(target_px is not None, "target box is not in bounds")

    dropped = augmentation.get("dropped") or []
    require(dropped, "variant has no dropped elements")
    dropped_indices = set()
    dropped_boxes = []
    for drop in dropped:
        element_index = drop.get("element_index")
        require(type(element_index) is int and 0 <= element_index < len(elements),
                f"dropped element index invalid: {element_index}")
        require(element_index not in dropped_indices, f"duplicate dropped element index: {element_index}")
        dropped_indices.add(element_index)
        require(element_index != target_index, "target element was dropped")
        element = elements[element_index]
        source_px = common.pixel_box(element.get("box"), width, height, tolerance=2.0)
        require(source_px is not None, "dropped element source box invalid")
        require(drop.get("source_box") == element.get("box"), "dropped source box changed")
        require(drop.get("fill_box") == list(source_px), "dropped fill box is not the source box projection")
        fill_box = tuple(drop["fill_box"])
        require(fill_box == tuple(source_px), "dropped fill box mismatch")
        fill_guard = common.clip_box(common.expand_box(target_px, common.FILL_GUARD_PX), width, height)
        require(not common.boxes_intersect(fill_box, fill_guard), "drop touches target fill guard")
        for protected_box in protected_boxes:
            require(not common.boxes_intersect(fill_box, protected_box),
                    "drop overlaps protected target/context region")
        dropped_boxes.append(fill_box)
    for left in range(len(dropped_boxes)):
        for right in range(left + 1, len(dropped_boxes)):
            require(not common.boxes_intersect(dropped_boxes[left], dropped_boxes[right]),
                    "dropped boxes overlap each other")

    remaining = 0
    for index, element in enumerate(elements):
        if index == target_index:
            continue
        element_box = common.pixel_box(element.get("box"), width, height, tolerance=2.0)
        if element_box is None:
            continue
        if any(common.boxes_intersect(element_box, dropped_box) for dropped_box in dropped_boxes):
            continue
        remaining += 1
    require(remaining == augmentation.get("remaining_distractors"),
            "remaining distractor count mismatch")
    require(remaining >= common.MIN_REMAINING_DISTRACTORS, "too few distractors retained")

    generated_path = safe_child(artifact, row["screen"])
    require(generated_path.is_file(), f"generated image missing: {row['screen']}")
    with Image.open(generated_path) as generated_image:
        generated = generated_image.convert("RGB")
    require(generated.size == (width, height), "generated image dimensions changed")
    changed, leak, changed_count = diff_mask_and_leak(parent_image, generated, dropped_boxes)
    require(leak.getbbox() is None, "generated image changed outside dropped boxes")
    require(changed_count > 0, "generated image did not change any pixels")
    require(changed_count == augmentation.get("changed_pixels"), "changed pixel count mismatch")
    require(abs(changed_count / (width * height) - augmentation.get("changed_fraction", -1)) < 1e-7,
            "changed fraction mismatch")
    require(changed_count / (width * height) <= 0.35, "generated image changed too much")
    require(ImageStat.Stat(generated.convert("L")).stddev[0] >= 1.0, "generated image is blank/flat")

    # Exact target and protected-context pixels must be unchanged.
    for name, box in [("target", target_px)] + [("protected", b) for b in protected_boxes]:
        if ImageChops.difference(parent_image.crop(box), generated.crop(box)).getbbox() is not None:
            raise AuditFailure(f"generated image changed {name} pixels at {box}")
    # Redundant explicit mask intersection: changed pixels must not touch any
    # protected region, including sub-pixel rounding guards.
    protected_mask = Image.new("L", generated.size, 0)
    draw = ImageDraw.Draw(protected_mask)
    for box in [target_px] + protected_boxes:
        draw.rectangle([box[0], box[1], max(box[0], box[2] - 1), max(box[1], box[3] - 1)], fill=255)
    require(ImageChops.multiply(changed, protected_mask).getbbox() is None,
            "changed pixels overlap protected regions")

    generated_file_sha = common.sha256_file(generated_path)
    generated_pixel_sha = raw_pixel_sha(generated)
    require(generated_file_sha not in src["heldout_images"], "generated image hash leaks a heldout image")
    require(generated_pixel_sha not in src["heldout_pixels"], "generated pixels leak a heldout image")
    require(generated_pixel_sha != raw_pixel_sha(parent_image), "generated image equals its parent pixels")
    require(generated_file_sha not in generated_file_hashes, "duplicate generated image file hash")
    require(generated_pixel_sha not in generated_pixel_hashes,
            f"duplicate generated image pixels for target {label_id}")
    generated_file_hashes[generated_file_sha] = label_id
    generated_pixel_hashes[generated_pixel_sha] = label_id
    return {
        "changed_pixels": changed_count,
        "remaining_distractors": remaining,
        "dropped": len(dropped),
    }


def act_raw(record: dict, src: dict, kind: str = "row") -> bytes:
    """Return exact source raw bytes for a parent row/label record."""
    if kind == "label":
        label_id = record["id"]
        return src["labels"][label_id][1].encode("utf-8")
    label_id = record["label_id"]
    return src["source_rows"][label_id][1].encode("utf-8")


def check_variants(artifact: Path, source: Path, act: dict, src: dict):
    rows = act["rows"]
    require(rows, "artifact has no generated variants")
    groups = collections.OrderedDict()
    for row in rows:
        groups.setdefault(row["label_id"], []).append(row)
    require(len(groups) <= common.MAX_PARENTS, "distinct generated parents exceed cap")
    generated_pixel_hashes: dict[str, str] = {}
    generated_file_hashes: dict[str, str] = {}
    by_family = collections.Counter()
    by_scope = collections.Counter()
    by_control = collections.Counter()
    stats = []
    for label_id, group in groups.items():
        source_row, label_record, _source_raw, frame_matches = exact_parent_check(label_id, group[0], act, src, artifact, source)
        require(len(group) <= common.MAX_VARIANTS_PER_PARENT, f"too many variants for {label_id}")
        recorded = group[0]["augmentation"]["source_page"]
        recorded_frame = None
        for frame in frame_matches:
            origin = frame.get("origin") or {}
            if origin.get("file") == recorded.get("file") and int(origin.get("index", -1)) == int(recorded.get("index", -2)):
                recorded_frame = frame
                break
        require(recorded_frame is not None, f"recorded source page is not a parent frame: {label_id}")
        require(common.sha256_json(recorded_frame) == group[0]["augmentation"].get("parent_frame_sha256"),
                f"recorded parent frame digest mismatch: {label_id}")
        page_path = safe_child(source, recorded["file"])
        require(page_path.is_file(), f"recorded source page missing: {recorded['file']}")
        require(common.sha256_file(page_path) == recorded["sha256"],
                f"recorded source page digest mismatch: {recorded['file']}")
        copied_page = safe_child(artifact, f"provenance/source_pages/{Path(recorded['file']).name}")
        require(copied_page.is_file(), f"copied recorded source page missing: {recorded['file']}")
        require(common.sha256_file(copied_page) == recorded["sha256"],
                f"copied recorded source page digest mismatch: {recorded['file']}")
        page_payload = load_json(page_path)
        require(isinstance(page_payload, list), f"source page is not a list: {recorded['file']}")
        require(0 <= int(recorded["index"]) < len(page_payload),
                f"source page index out of range: {recorded['file']}")
        page_record = page_payload[int(recorded["index"])]
        matched = common.find_target_element(page_record.get("elements", []), source_row["box"],
                                             source_row["width"], source_row["height"])
        require(matched is not None, f"recorded source page has no confident target match: {label_id}")
        target_index, target_element, _target_iou = matched
        require(target_index == group[0]["augmentation"].get("target_element_index"),
                f"recorded target element index mismatch: {label_id}")
        parent_screen_hash = Path(source_row["screen"]).name.removesuffix(".png")
        parent_image_path = artifact / "parents" / "images" / f"{parent_screen_hash}.png"
        with Image.open(parent_image_path) as parent_image:
            parent_image = parent_image.convert("RGB")
            for row in group:
                require(row["augmentation"].get("source_page") == group[0]["augmentation"].get("source_page"),
                        f"variant source page disagrees within parent {label_id}")
                require(row["augmentation"].get("parent_frame_sha256")
                        == group[0]["augmentation"].get("parent_frame_sha256"),
                        f"variant parent frame disagrees within parent {label_id}")
                stats.append(check_variant(label_id, row, source_row, label_record, frame_matches,
                                           page_record, parent_image, artifact, src,
                                           generated_pixel_hashes, generated_file_hashes))
                by_family[row["family"]] += 1
                by_scope[row["augmentation"]["scope"]] += 1
                by_control[common.control_type(source_row["target"], target_element)] += 1
    counts = {
        "variants": len(rows),
        "parents_with_variants": len(groups),
        "by_family": dict(sorted(by_family.items())),
        "by_scope": dict(sorted(by_scope.items())),
        "by_control_type": dict(sorted(by_control.items())),
        "changed_pixels": sum(item["changed_pixels"] for item in stats),
        "min_remaining_distractors": min(item["remaining_distractors"] for item in stats),
        "max_remaining_distractors": max(item["remaining_distractors"] for item in stats),
        "unique_generated_file_hashes": len(generated_file_hashes),
        "unique_generated_pixel_hashes": len(generated_pixel_hashes),
    }
    return counts


def check_manifest_counts(manifest: dict, act: dict, variant_counts: dict, artifact: Path):
    counts = manifest.get("counts", {})
    require(counts.get("variants") == variant_counts["variants"], "manifest variant count mismatch")
    require(counts.get("parents_with_variants") == variant_counts["parents_with_variants"],
            "manifest parents_with_variants count mismatch")
    require(counts.get("parents_selected") == len(act["parent_rows"]), "manifest parents_selected mismatch")
    require(counts.get("controls") == len(act["controls"]), "manifest controls count mismatch")
    require(counts.get("by_family") == variant_counts["by_family"], "manifest by_family mismatch")
    require(counts.get("by_scope") == variant_counts["by_scope"], "manifest by_scope mismatch")
    require(counts.get("by_control_type") == variant_counts["by_control_type"],
            "manifest by_control_type mismatch")
    require(counts.get("exclusions") == len(act["exclusions"]), "manifest exclusion count mismatch")
    one_variant = sum(1 for group in collections.Counter(row["label_id"] for row in act["rows"]).values()
                      if group == 1)
    require(counts.get("parents_one_variant") == one_variant, "manifest one-variant count mismatch")
    require(counts.get("parents_zero_variants") == len(act["parent_rows"]) - len(
        {row["label_id"] for row in act["rows"]}), "manifest zero-variant count mismatch")
    return {"manifest_counts_checked": True}


def check_holdout_manifest(manifest: dict, src: dict):
    holdout = manifest.get("holdout", {})
    require(sorted(holdout.get("image_sha256", [])) == sorted(src["heldout_images"]),
            "manifest heldout image hash set mismatch")
    require(sorted(holdout.get("pixel_sha256", [])) == sorted(src["heldout_pixels"]),
            "manifest heldout pixel hash set mismatch")
    return {"heldout_images": len(src["heldout_images"]), "heldout_pixels": len(src["heldout_pixels"])}


def check_exclusions(act: dict):
    allowed_scopes = {"parent_selection", "variant_generation"}
    allowed_reasons = {
        "row_not_train", "not_full_frame", "invalid_dimensions", "missing_label_record",
        "label_record_not_train", "label_origin_not_train", "label_id_in_heldout",
        "group_in_heldout", "missing_screen", "screen_filename_hash_mismatch",
        "screen_in_heldout", "screen_dimensions_mismatch", "parent_pixels_in_heldout",
        "parent_click_outside_box", "parent_answer_invalid", "source_target_unmatched",
        "source_target_ambiguous", "no_safe_dropout_candidate", "empty_drop_set",
        "degenerate_drop_box", "overlapping_drop_boxes", "drop_overlaps_protected",
        "duplicate_variant_id", "change_outside_drop_boxes", "no_pixel_change",
        "drop_area_too_large", "blank_or_flat_output", "duplicate_generated_pixels",
    }
    reasons = collections.Counter()
    for record in act["exclusions"]:
        require(record.get("scope") in allowed_scopes, f"invalid exclusion scope: {record.get('scope')}")
        require(record.get("reason") in allowed_reasons, f"invalid exclusion reason: {record.get('reason')}")
        reasons[record["reason"]] += 1
    return {"excluded_cases": len(act["exclusions"]), "reasons": dict(sorted(reasons.items()))}


def audit(artifact: Path, source: Path):
    artifact = artifact.resolve()
    source = source.resolve()
    require(artifact.is_dir(), f"artifact missing: {artifact}")
    require(source.is_dir(), f"source missing: {source}")
    src = source_context(source)
    act = artifact_context(artifact, source)
    checks = []
    result = {
        "audit_version": "mage-element-dropout-positives-audit-v1",
        "artifact": str(artifact),
        "source": str(source),
        "pass": False,
        "error": None,
        "checks": checks,
        "counts": {},
    }

    def run(name, function):
        try:
            detail = function()
            checks.append({"name": name, "pass": True, "detail": detail})
            return detail
        except Exception as exc:
            checks.append({"name": name, "pass": False,
                           "error": f"{type(exc).__name__}: {exc}"})
            raise

    try:
        run("file_inventory", lambda: check_file_inventory(artifact, act["manifest"]))
        run("source_manifest_unchanged", lambda: check_source_manifest(source, artifact, act["manifest"], src))
        run("source_pages", lambda: check_source_pages(source, artifact, act))
        run("controls", lambda: check_controls(artifact, act, src))
        run("parents", lambda: check_parents(artifact, source, act, src))
        variant_counts = run("variants", lambda: check_variants(artifact, source, act, src))
        run("manifest_counts", lambda: check_manifest_counts(act["manifest"], act, variant_counts, artifact))
        run("holdout_manifest", lambda: check_holdout_manifest(act["manifest"], src))
        run("exclusions", lambda: check_exclusions(act))
        result["counts"] = {
            **variant_counts,
            "parents_selected": len(act["parent_rows"]),
            "controls": len(act["controls"]),
            "exclusions": len(act["exclusions"]),
        }
        result["manifest_sha256"] = common.sha256_file(artifact / "manifest.json")
        result["source_manifest_sha256"] = common.sha256_file(source / "manifest.json")
        result["pass"] = True
        result["verdict"] = "PASS: generated element-dropout positives satisfy the isolated train-only audit"
    except Exception as exc:
        result["error"] = f"{type(exc).__name__}: {exc}"
        result["verdict"] = "FAIL: artifact did not satisfy the isolated train-only audit"
        result["traceback"] = traceback.format_exc()
    return result


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path, default=DEFAULT_ARTIFACT)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--no-write-audit", action="store_true")
    args = parser.parse_args(argv)
    result = audit(args.artifact, args.source)
    if not args.no_write_audit and args.artifact.is_dir():
        (args.artifact / "AUDIT.json").write_text(
            json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
    print(json.dumps({
        "pass": result["pass"],
        "verdict": result["verdict"],
        "artifact": result["artifact"],
        "manifest_sha256": result.get("manifest_sha256"),
        "counts": result.get("counts"),
        "error": result.get("error"),
    }, indent=2, sort_keys=True))
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

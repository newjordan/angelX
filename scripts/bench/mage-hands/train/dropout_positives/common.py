"""Deterministic helpers for isolated element-dropout positive augmentation.

This module is deliberately side-effect free.  It never calls a model, a
network service, a GPU job, or the frozen source dataset for writes.  Build,
verify and focused regression code share these exact rules so the audit and the
builder cannot silently diverge.
"""
from __future__ import annotations

import hashlib
import json
import math
import re
from pathlib import Path

EXPECTED_SOURCE_MANIFEST_SHA256 = (
    "be4a519a04f6fee2d82259315eeebb1e5403016e74d1207220caa42aba4f20e0"
)
SEED = 20261101
MAX_PARENTS = 512
MAX_VARIANTS_PER_PARENT = 2

# The fill guard keeps generated infill a few pixels clear of the exact target
# box.  It is intentionally small: adjacent, unrelated controls are useful
# distractors and may be dropped.
FILL_GUARD_PX = 4
CONTEXT_GUARD_PX = 2
MIN_CANDIDATE_AREA_PX = 64
MAX_ELEMENT_AREA_FRAC = 0.12
MAX_DROP_AREA_FRAC = 0.25
MAX_ELEMENT_WIDTH_FRAC = 0.50
MAX_ELEMENT_HEIGHT_FRAC = 0.50
MIN_REMAINING_DISTRACTORS = 2

# Role words are not identifying context.  They are removed before matching an
# element's visible name/text against the target phrase.
STOPWORDS = {
    "a", "an", "and", "at", "button", "cell", "close", "current", "dialog",
    "field", "for", "from", "icon", "in", "item", "link", "menu", "of", "on",
    "open", "push", "region", "tab", "text", "the", "to", "toggle", "with",
}
INTERACTIVE_ROLE_PARTS = {
    "a", "button", "cell", "check", "checkbox", "combo", "entry", "icon",
    "image", "img", "input", "item", "link", "list", "menu", "menuitem",
    "page tab", "push button", "radio", "row", "select", "slider", "switch",
    "tab", "table cell", "text", "textarea", "toggle", "toggle button",
}

RAW_PIXEL_MODE = "RGB"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path | str) -> str:
    return sha256_bytes(Path(path).read_bytes())


def canonical_json(value) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_json(value) -> str:
    return sha256_bytes(canonical_json(value).encode("utf-8"))


def read_jsonl(path: Path | str) -> list[dict]:
    records = []
    with Path(path).open("r", encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if line:
                records.append(json.loads(line))
    return records


def read_jsonl_with_raw(path: Path | str) -> list[tuple[dict, str]]:
    records = []
    with Path(path).open("r", encoding="utf-8") as handle:
        for line in handle:
            raw = line.rstrip("\n")
            if raw:
                records.append((json.loads(raw), raw))
    return records


def write_jsonl(path: Path | str, rows: list[dict], *, sort_keys: bool = True) -> None:
    path = Path(path)
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=sort_keys, ensure_ascii=False) + "\n")


def stable_key(*parts) -> str:
    return sha256_bytes(("|".join(str(p) for p in parts)).encode("utf-8"))


def clamp(value: int, low: int, high: int) -> int:
    return max(low, min(high, value))


def finite_number(value) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def pixel_box(box, width: int, height: int, *, tolerance: float = 1.0):
    """Return a clipped integer ``(x0, y0, x1, y1)`` or ``None``.

    Source element boxes are sometimes fractional.  Fill boxes use floor/ceil
    so an element is fully covered.  A small tolerance admits boxes that touch
    the screen edge after browser/AT-SPI rounding; materially out-of-bounds
    boxes are rejected rather than clipped silently.
    """
    if not isinstance(box, (list, tuple)) or len(box) != 4:
        return None
    if not all(finite_number(v) for v in box):
        return None
    x0, y0, x1, y1 = (float(v) for v in box)
    if not (x0 < x1 and y0 < y1):
        return None
    if x0 < -tolerance or y0 < -tolerance or x1 > width + tolerance or y1 > height + tolerance:
        return None
    return (
        clamp(math.floor(x0), 0, width),
        clamp(math.floor(y0), 0, height),
        clamp(math.ceil(x1), 0, width),
        clamp(math.ceil(y1), 0, height),
    )


def box_area(box) -> int:
    return max(0, int(box[2]) - int(box[0])) * max(0, int(box[3]) - int(box[1]))


def boxes_intersect(a, b) -> bool:
    return a[0] < b[2] and b[0] < a[2] and a[1] < b[3] and b[1] < a[3]


def expand_box(box, pixels: int):
    return (box[0] - pixels, box[1] - pixels, box[2] + pixels, box[3] + pixels)


def clip_box(box, width: int, height: int):
    return (clamp(box[0], 0, width), clamp(box[1], 0, height),
            clamp(box[2], 0, width), clamp(box[3], 0, height))


def box_iou(a, b) -> float:
    ix = max(0.0, min(a[2], b[2]) - max(a[0], b[0]))
    iy = max(0.0, min(a[3], b[3]) - max(a[1], b[1]))
    inter = ix * iy
    union = box_area(a) + box_area(b) - inter
    return float(inter / union) if union > 0 else 0.0


def normalize_text(value) -> str:
    return re.sub(r"\s+", " ", str(value or "")).strip()


def text_tokens(value) -> set[str]:
    return {
        token for token in re.findall(r"[a-z0-9]+", normalize_text(value).lower())
        if len(token) >= 2 and token not in STOPWORDS
    }


def element_role(element: dict) -> str:
    return normalize_text(element.get("role") or element.get("tag") or "").lower()


def element_tag(element: dict) -> str:
    return normalize_text(element.get("tag") or "").lower()


def element_context_text(element: dict) -> str:
    """Visible/accessible text used for target-context token protection.

    Native AT-SPI elements expose the short accessible ``name`` (the actual
    button label) plus a longer ``target`` sentence that includes the app name.
    Using ``name`` first prevents every calculator control from matching the
    app name in the target phrase.
    """
    for key in ("name", "text", "label", "target"):
        value = normalize_text(element.get(key))
        if value:
            return value
    return ""


def element_display_text(element: dict) -> str:
    parts = [normalize_text(element.get(key)) for key in ("target", "name", "text", "label")]
    return " | ".join(part for part in parts if part)


def meaningful_role(element: dict) -> bool:
    role = element_role(element)
    tag = element_tag(element)
    haystack = f"{role} {tag}"
    return any(part in haystack for part in INTERACTIVE_ROLE_PARTS)


def find_target_element(elements: list[dict], target_box, width: int, height: int):
    """Return ``(index, element, iou)`` for a unique confident target match."""
    target_px = pixel_box(target_box, width, height, tolerance=0.0)
    if target_px is None:
        return None
    scored = []
    for index, element in enumerate(elements):
        source_box = pixel_box(element.get("box"), width, height, tolerance=2.0)
        if source_box is None:
            continue
        scored.append((box_iou(target_px, source_box), index, element))
    if not scored:
        return None
    scored.sort(key=lambda item: (-item[0], item[1]))
    best_iou, best_index, best_element = scored[0]
    if best_iou < 0.50:
        return None
    if len(scored) > 1 and abs(best_iou - scored[1][0]) < 1e-9:
        return None
    return best_index, best_element, best_iou


def protected_regions(elements: list[dict], target_index: int, target_box,
                      target_text: str, width: int, height: int) -> list[dict]:
    """Protected pixel regions that must remain byte-identical.

    The target's exact box and a 4 px fill-clearance guard are always
    protected.  Visible/accessible text that shares a non-role token with the
    target phrase, an element overlapping the target box, or a text-bearing
    element touching the target box is also protected so identifying labels
    are not used as dropout distractors.  The proximity check is deliberately
    tight (4 px) so adjacent unrelated controls remain usable distractors.
    """
    target_px = pixel_box(target_box, width, height, tolerance=0.0)
    if target_px is None:
        raise ValueError("target box is not a valid in-bounds box")
    guard = clip_box(expand_box(target_px, FILL_GUARD_PX), width, height)
    regions = [{"reason": "target_guard", "box": list(guard), "element_index": None}]
    target_tokens = text_tokens(target_text)
    target_proximity = clip_box(expand_box(target_px, FILL_GUARD_PX), width, height)
    for index, element in enumerate(elements):
        source_px = pixel_box(element.get("box"), width, height, tolerance=2.0)
        if source_px is None:
            continue
        # Do not protect an entire window/panel merely because a token appears
        # somewhere inside it; the target guard already covers the target.
        if box_area(source_px) > MAX_ELEMENT_AREA_FRAC * width * height:
            continue
        context_text = element_context_text(element)
        context_tokens = text_tokens(context_text)
        token_hit = bool(context_tokens & target_tokens)
        overlap_hit = boxes_intersect(source_px, target_px)
        adjacent_text_hit = bool(context_text) and boxes_intersect(source_px, target_proximity)
        if overlap_hit or token_hit or adjacent_text_hit:
            context_px = clip_box(expand_box(source_px, CONTEXT_GUARD_PX), width, height)
            regions.append({
                "reason": "target_element_context" if index == target_index else "identifying_context",
                "box": list(context_px),
                "element_index": index,
            })
    # Stable order is part of the generated-row contract.
    regions.sort(key=lambda r: (r["reason"], tuple(r["box"]), -1 if r["element_index"] is None else r["element_index"]))
    return regions


def candidate_elements(elements: list[dict], target_index: int, target_box,
                       target_text: str, width: int, height: int):
    """Return genuine source-element dropout candidates, deterministically ordered."""
    protected = protected_regions(elements, target_index, target_box, target_text, width, height)
    protected_boxes = [tuple(region["box"]) for region in protected]
    target_px = pixel_box(target_box, width, height, tolerance=0.0)
    if target_px is None:
        raise ValueError("target box is not a valid in-bounds box")
    fill_guard = clip_box(expand_box(target_px, FILL_GUARD_PX), width, height)
    candidates = []
    for index, element in enumerate(elements):
        if index == target_index:
            continue
        source_box = pixel_box(element.get("box"), width, height, tolerance=2.0)
        if source_box is None:
            continue
        area = box_area(source_box)
        if area < MIN_CANDIDATE_AREA_PX or area > MAX_ELEMENT_AREA_FRAC * width * height:
            continue
        if (source_box[2] - source_box[0]) > MAX_ELEMENT_WIDTH_FRAC * width:
            continue
        if (source_box[3] - source_box[1]) > MAX_ELEMENT_HEIGHT_FRAC * height:
            continue
        if boxes_intersect(source_box, fill_guard):
            continue
        if any(boxes_intersect(source_box, protected_box) for protected_box in protected_boxes):
            continue
        if not element_context_text(element) and not meaningful_role(element):
            continue
        candidates.append({
            "element_index": index,
            "source_box": list(element.get("box")),
            "box": list(source_box),
            "role": element_role(element),
            "tag": element_tag(element),
            "text": element_display_text(element),
            "order_key": stable_key(SEED, target_index, index, tuple(source_box)),
        })
    candidates.sort(key=lambda c: (0 if c["text"] else 1, c["order_key"], c["element_index"]))
    return candidates, protected


def choose_drop_sets(candidates: list[dict], width: int, height: int,
                     element_count: int) -> list[dict]:
    """Choose 1 or 2 deterministic, disjoint-element dropout subsets.

    ``compact`` drops one or two source elements.  ``context_group`` drops a
    small spatial cluster (up to five) while always leaving at least two other
    source elements visible.  Both are genuine source-element dropout; no
    background-region infill is invented here.
    """
    if not candidates:
        return []
    max_drops = max(0, element_count - 1 - MIN_REMAINING_DISTRACTORS)
    if max_drops < 1:
        return []
    area_cap = MAX_DROP_AREA_FRAC * width * height

    def greedy(seq, limit):
        selected = []
        for candidate in seq:
            if len(selected) >= limit:
                break
            box = tuple(candidate["box"])
            if any(boxes_intersect(box, tuple(other["box"])) for other in selected):
                continue
            if sum(box_area(tuple(other["box"])) for other in selected) + box_area(box) > area_cap:
                continue
            selected.append(candidate)
        return selected

    # Prefer a spatial cluster for the context variant.  Coarse 3x3 cells are
    # enough to make "drop a toolbar/sidebar area" distinct from "drop one or
    # two controls".
    cells: dict[tuple[int, int], list[dict]] = {}
    for candidate in candidates:
        box = candidate["box"]
        cx = (box[0] + box[2]) / 2.0
        cy = (box[1] + box[3]) / 2.0
        cell = (min(2, int(cx * 3 / width)), min(2, int(cy * 3 / height)))
        cells.setdefault(cell, []).append(candidate)
    cell_rank = sorted(
        cells.items(),
        key=lambda item: (-len(item[1]), stable_key(SEED, "cell", item[0])),
    )
    context = []
    for _cell, members in cell_rank:
        context = greedy(members, min(5, max_drops))
        if len(context) >= 2:
            break
    if len(context) < 2:
        context = greedy(candidates, min(5, max_drops))
    if not context:
        return []

    context_ids = {c["element_index"] for c in context}
    compact_pool = [c for c in candidates if c["element_index"] not in context_ids]
    compact = greedy(compact_pool, min(2, max_drops))
    if not compact:
        # Only one region has candidates: make the compact variant a strict
        # subset, never an identical image.
        compact = context[:1]
    compact_ids = {c["element_index"] for c in compact}
    if compact_ids == context_ids:
        if len(context) > 1:
            compact = context[:1]
            compact_ids = {compact[0]["element_index"]}
        else:
            return [{"scope": "compact", "drops": compact}]

    drop_sets = [{"scope": "compact", "drops": compact}]
    drop_sets.append({"scope": "context_group", "drops": context})
    return drop_sets


def _parse_click(answer: str) -> dict:
    matches = re.findall(r"<tool_call>\s*(.*?)\s*</tool_call>", answer or "", re.S)
    if len(matches) != 1 or (answer or "").count("<tool_call>") != 1:
        raise ValueError("answer must contain exactly one complete tool call")
    call = json.loads(matches[0])
    if call.get("name") != "computer_use":
        raise ValueError("answer is not a computer_use call")
    args = call.get("arguments") or {}
    if args.get("action") != "click" or args.get("button") != "left":
        raise ValueError("answer is not a single left click")
    point = args.get("coordinate")
    if (not isinstance(point, list) or len(point) != 2
            or any(type(v) is not int or not 0 <= v <= 999 for v in point)):
        raise ValueError("answer coordinate is not two integers in [0, 999]")
    return call


def parse_click(answer: str) -> dict:
    return _parse_click(answer)


def projected_click(row: dict) -> tuple[float, float]:
    """Project a row's normalized click into original full-frame coordinates."""
    call = _parse_click(row["answer"])
    point = call["arguments"]["coordinate"]
    width, height = int(row["width"]), int(row["height"])
    crop = row.get("crop")
    px = point[0] / 999 * (width - 1)
    py = point[1] / 999 * (height - 1)
    if crop is not None:
        x, y, cw, ch = crop
        px = x + px / width * cw
        py = y + py / height * ch
    return px, py


def click_inside_box(row: dict) -> bool:
    px, py = projected_click(row)
    box = row["box"]
    return (box[0] <= px <= box[2] and box[1] <= py <= box[3])


def control_type(target: str, element: dict | None = None) -> str:
    text = normalize_text(target).lower()
    role = element_role(element or {})
    tag = element_tag(element or {})
    haystack = f"{role} {tag}"
    if "button" in haystack or "button" in text:
        return "button"
    if tag == "a" or "link" in haystack or "link" in text:
        return "link"
    if any(part in haystack for part in ("entry", "text", "input", "textarea")) \
            or any(part in text for part in ("field", "text box", "textbox", "input")):
        return "text_field"
    if any(part in haystack for part in ("check", "toggle", "switch", "radio")) \
            or any(part in text for part in ("checkbox", "toggle", "switch")):
        return "toggle"
    if "tab" in haystack or "tab" in text:
        return "tab"
    if "menu" in haystack or "menu" in text:
        return "menu"
    if any(part in haystack for part in ("combo", "popup", "select", "dropdown")) \
            or any(part in text for part in ("dropdown", "drop-down", "select", "combo")):
        return "dropdown"
    if "slider" in haystack or "slider" in text:
        return "slider"
    if any(part in haystack for part in ("cell", "item", "row")) \
            or any(part in text for part in (" item", " row", " cell")):
        return "item"
    if any(part in haystack for part in ("icon", "image", "img")) or "icon" in text:
        return "icon"
    return "other"

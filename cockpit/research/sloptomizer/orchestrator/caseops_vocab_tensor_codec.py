"""Canonical compact codec for CaseOps vocab tensor rows."""

from __future__ import annotations

import math
import re
from typing import Any, Iterable


_SECTION_RE = re.compile(
    r"\bcaseops[ _]vocab[ _]tensors?(?:_feedback)?\s*[:=]\s*([^\n]+)",
    re.IGNORECASE,
)
_SCORE_RE = re.compile(
    r"[-+]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:e[-+]?[0-9]+)?",
    re.IGNORECASE,
)


def caseops_vocab_tensor_piece(value: Any) -> str:
    """Normalize one compact vocab-tensor atom."""
    return re.sub(r"[^a-z0-9_.:-]+", "_", str(value).lower()).strip("_")


def encode_caseops_vocab_tensor(
    row: dict[str, Any],
    *,
    limit: int = 8,
    include_magnitude: bool = True,
) -> str:
    """Encode a signed sparse vocab tensor as ``kind:value coords=axis:+weight``."""
    kind = caseops_vocab_tensor_piece(row.get("kind") or "")
    value = caseops_vocab_tensor_piece(row.get("value") or "")
    if not kind or not value:
        return ""
    coordinates = _caseops_vocab_tensor_coordinates(row, limit=limit)
    if not coordinates:
        return ""
    coord_text = ",".join(
        f"{axis}:{weight:+.3f}"
        for axis, weight in coordinates
    )
    bits = [f"{kind}:{value}", f"coords={coord_text}"]
    if include_magnitude:
        bits.append(f"mag={_caseops_vocab_tensor_magnitude(row, coordinates):.3f}")
    dimensions = _caseops_vocab_tensor_dimensions(row, coordinates)
    if dimensions:
        bits.append(f"dims={dimensions}")
    layers = _caseops_vocab_tensor_layers(row)
    if layers:
        bits.append("layers=" + ",".join(layers))
    links = _caseops_vocab_tensor_links(row)
    if links:
        bits.append("links=" + ",".join(links))
    metadata = _caseops_vocab_tensor_metadata(row)
    if metadata:
        bits.append("meta=" + ",".join(
            f"{key}:{value}"
            for key, value in metadata
        ))
    return " ".join(bits)


def parse_caseops_vocab_tensors(text: str) -> tuple[dict[str, Any], ...]:
    """Decode compact vocab-tensor rows from text."""
    rows: list[dict[str, Any]] = []
    for match in _SECTION_RE.finditer(str(text or "")):
        for item in match.group(1).split(";"):
            parsed = parse_caseops_vocab_tensor_item(item)
            if parsed:
                rows.append(parsed)
    return _dedupe_vocab_tensor_rows(rows)


def parse_caseops_vocab_tensor_item(item: str) -> dict[str, Any] | None:
    """Decode one compact vocab-tensor item."""
    text = str(item or "").strip().strip(",")
    if not text:
        return None
    fields = [field for field in text.split() if field.strip()]
    if not fields:
        return None
    head = fields[0]
    kind, sep, value = head.partition(":")
    clean_kind = caseops_vocab_tensor_piece(kind)
    clean_value = caseops_vocab_tensor_piece(value)
    if not sep or not clean_kind or not clean_value:
        return None
    row = {
        "kind": clean_kind,
        "value": clean_value,
        "coordinates": {},
        "magnitude": 0.0,
    }
    for field in fields[1:]:
        key, field_sep, raw_value = field.partition("=")
        if not field_sep:
            continue
        clean_key = key.strip().lower()
        if clean_key in {"coord", "coords", "coordinates"}:
            row["coordinates"] = dict(_caseops_vocab_tensor_coord_csv(raw_value))
        elif clean_key in {"mag", "magnitude"}:
            row["magnitude"] = _safe_float(raw_value, 0.0)
        elif clean_key in {"dim", "dims", "dimensions"}:
            row["dimensions"] = _safe_int(raw_value, 0)
        elif clean_key in {"layer", "layers", "encoding_layers"}:
            row["layers"] = _caseops_vocab_tensor_csv(raw_value)
        elif clean_key in {"link", "links", "math_language_links"}:
            row["math_language_links"] = _caseops_vocab_tensor_link_csv(raw_value)
        elif clean_key in {"meta", "metadata"}:
            row["metadata"] = dict(_caseops_vocab_tensor_metadata_csv(raw_value))
    if not row["coordinates"]:
        return None
    if not row["magnitude"]:
        row["magnitude"] = math.sqrt(
            sum(value * value for value in row["coordinates"].values())
        )
    if not row.get("dimensions"):
        row["dimensions"] = len(row["coordinates"])
    row["positive_axes"] = tuple(
        axis for axis, value in row["coordinates"].items() if value > 0.0
    )
    row["negative_axes"] = tuple(
        axis for axis, value in row["coordinates"].items() if value < 0.0
    )
    return row


def _caseops_vocab_tensor_dimensions(
    row: dict[str, Any],
    coordinates: Iterable[tuple[str, float]],
) -> int:
    value = _safe_int(row.get("dimensions"), 0)
    if value > 0:
        return value
    if row.get("layers") or row.get("encoding_layers"):
        return len(tuple(coordinates))
    return 0


def _caseops_vocab_tensor_coordinates(
    row: dict[str, Any],
    *,
    limit: int,
) -> tuple[tuple[str, float], ...]:
    coordinates = row.get("coordinates")
    out: list[tuple[str, float]] = []
    if isinstance(coordinates, dict):
        for axis, value in coordinates.items():
            clean_axis = caseops_vocab_tensor_piece(axis)
            weight = _safe_float(value, 0.0)
            if clean_axis and weight != 0.0:
                out.append((clean_axis, weight))
    else:
        for axis in tuple(row.get("positive_axes", ()) or ()):
            clean_axis = caseops_vocab_tensor_piece(axis)
            if clean_axis:
                out.append((clean_axis, 1.0))
        for axis in tuple(row.get("negative_axes", ()) or ()):
            clean_axis = caseops_vocab_tensor_piece(axis)
            if clean_axis:
                out.append((clean_axis, -1.0))
    deduped: dict[str, float] = {}
    for axis, value in out:
        deduped[axis] = deduped.get(axis, 0.0) + float(value)
    rows = [
        (axis, value)
        for axis, value in deduped.items()
        if axis and value != 0.0
    ]
    rows.sort(key=lambda item: (
        0 if item[1] < 0.0 else 1,
        item[0],
    ))
    return tuple(rows[: max(1, int(limit))])


def _caseops_vocab_tensor_coord_csv(value: str) -> tuple[tuple[str, float], ...]:
    rows: list[tuple[str, float]] = []
    for part in str(value or "").split(","):
        axis, sep, raw_score = part.rpartition(":")
        clean_axis = caseops_vocab_tensor_piece(axis)
        if not sep or not clean_axis or not _SCORE_RE.fullmatch(raw_score.strip()):
            continue
        weight = _safe_float(raw_score, 0.0)
        if weight != 0.0:
            rows.append((clean_axis, weight))
    return tuple(dict(rows).items())


def _caseops_vocab_tensor_layers(row: dict[str, Any]) -> tuple[str, ...]:
    raw = (
        row.get("layers")
        or row.get("encoding_layers")
        or row.get("extra_layers")
        or ()
    )
    if isinstance(raw, str):
        values = raw.split(",")
    else:
        values = tuple(raw or ())
    return tuple(dict.fromkeys(
        clean
        for clean in (caseops_vocab_tensor_piece(value) for value in values)
        if clean
    ))


def _caseops_vocab_tensor_links(row: dict[str, Any]) -> tuple[str, ...]:
    raw = (
        row.get("math_language_links")
        or row.get("links")
        or row.get("semantic_links")
        or ()
    )
    if isinstance(raw, dict):
        raw = (raw,)
    out: list[str] = []
    for link in raw or ():
        if isinstance(link, str):
            parsed = _caseops_vocab_tensor_link(link)
        elif isinstance(link, dict):
            parsed = _caseops_vocab_tensor_link_from_row(link)
        else:
            parsed = ""
        if parsed:
            out.append(parsed)
    return tuple(dict.fromkeys(out))


def _caseops_vocab_tensor_link_from_row(row: dict[str, Any]) -> str:
    source = caseops_vocab_tensor_piece(row.get("source") or row.get("formula") or "")
    target = caseops_vocab_tensor_piece(row.get("target") or row.get("language") or "")
    if not source or not target:
        return ""
    relation = caseops_vocab_tensor_piece(row.get("relation") or "")
    weight = _safe_float(row.get("weight", row.get("score", 0.0)), 0.0)
    bits = [f"{source}->{target}"]
    if relation:
        bits.append(relation)
    if weight != 0.0:
        bits.append(f"{weight:+.3f}")
    return "~".join(bits)


def _caseops_vocab_tensor_link(value: Any) -> str:
    text = str(value or "").strip()
    if not text:
        return ""
    link, *rest = text.split("~")
    source, sep, target = link.partition("->")
    clean_source = caseops_vocab_tensor_piece(source)
    clean_target = caseops_vocab_tensor_piece(target)
    if not sep or not clean_source or not clean_target:
        return ""
    bits = [f"{clean_source}->{clean_target}"]
    if rest:
        relation = caseops_vocab_tensor_piece(rest[0])
        if relation:
            bits.append(relation)
    if len(rest) > 1 and _SCORE_RE.fullmatch(rest[1].strip()):
        bits.append(f"{_safe_float(rest[1], 0.0):+.3f}")
    return "~".join(bits)


def _caseops_vocab_tensor_link_csv(
    value: str,
) -> tuple[dict[str, Any], ...]:
    rows: list[dict[str, Any]] = []
    for part in str(value or "").split(","):
        encoded = _caseops_vocab_tensor_link(part)
        if not encoded:
            continue
        link, *rest = encoded.split("~")
        source, _, target = link.partition("->")
        row: dict[str, Any] = {
            "source": source,
            "target": target,
        }
        if rest:
            row["relation"] = rest[0]
        if len(rest) > 1:
            row["weight"] = _safe_float(rest[1], 0.0)
        rows.append(row)
    return tuple(rows)


def _caseops_vocab_tensor_metadata(
    row: dict[str, Any],
) -> tuple[tuple[str, str], ...]:
    raw = row.get("metadata") or row.get("meta") or {}
    if not isinstance(raw, dict):
        return ()
    rows: list[tuple[str, str]] = []
    for key, value in raw.items():
        clean_key = caseops_vocab_tensor_piece(key)
        clean_value = caseops_vocab_tensor_piece(value)
        if clean_key and clean_value:
            rows.append((clean_key, clean_value))
    rows.sort(key=lambda item: item[0])
    return tuple(rows)


def _caseops_vocab_tensor_metadata_csv(value: str) -> tuple[tuple[str, str], ...]:
    rows: list[tuple[str, str]] = []
    for part in str(value or "").split(","):
        key, sep, raw_value = part.partition(":")
        clean_key = caseops_vocab_tensor_piece(key)
        clean_value = caseops_vocab_tensor_piece(raw_value)
        if sep and clean_key and clean_value:
            rows.append((clean_key, clean_value))
    return tuple(dict(rows).items())


def _caseops_vocab_tensor_csv(value: str) -> tuple[str, ...]:
    return tuple(dict.fromkeys(
        clean
        for clean in (
            caseops_vocab_tensor_piece(part)
            for part in str(value or "").split(",")
        )
        if clean
    ))


def _caseops_vocab_tensor_magnitude(
    row: dict[str, Any],
    coordinates: Iterable[tuple[str, float]],
) -> float:
    value = _safe_float(row.get("magnitude"), 0.0)
    if value:
        return value
    return math.sqrt(sum(weight * weight for _, weight in coordinates))


def _safe_float(value: Any, default: float) -> float:
    try:
        return float(value)
    except Exception:  # noqa: BLE001
        return float(default)


def _safe_int(value: Any, default: int) -> int:
    try:
        return int(value)
    except Exception:  # noqa: BLE001
        return int(default)


def _dedupe_vocab_tensor_rows(
    rows: Iterable[dict[str, Any]],
) -> tuple[dict[str, Any], ...]:
    out: list[dict[str, Any]] = []
    seen: set[tuple[Any, ...]] = set()
    for row in rows:
        coordinates = row.get("coordinates") or {}
        coord_key = tuple(
            (str(axis), round(float(value), 6))
            for axis, value in sorted(coordinates.items())
        )
        key = (row.get("kind"), row.get("value"), coord_key)
        if row.get("layers"):
            key = (*key, tuple(row.get("layers", ()) or ()))
        if row.get("math_language_links"):
            key = (*key, tuple(
                (
                    link.get("source"),
                    link.get("target"),
                    link.get("relation"),
                    round(float(link.get("weight") or 0.0), 6),
                )
                for link in row.get("math_language_links", ())
                if isinstance(link, dict)
            ))
        if row.get("metadata"):
            key = (*key, tuple(sorted((row.get("metadata") or {}).items())))
        if key in seen:
            continue
        seen.add(key)
        out.append(dict(row))
    return tuple(out)

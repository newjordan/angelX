"""Canonical compact codec for CaseOps vocab-analogy rows."""

from __future__ import annotations

import re
from typing import Any, Iterable


_SECTION_RE = re.compile(
    r"\bcaseops[ _]vocab[ _]analog(?:y|ies)(?:_feedback)?\s*[:=]\s*([^\n]+)",
    re.IGNORECASE,
)
_ITEM_RE = re.compile(
    r"\s*([a-z0-9_.-]+):([a-z0-9_.:-]+?):"
    r"([-+]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:e[-+]?[0-9]+)?)"
    r"\s+positive=([^;\n\s]*)"
    r"(?:\s+negative=([^;\n\s]*))?"
    r"(?:\s+layers=([^;\n\s]*))?"
    r"(?:\s+meta=([^;\n\s]*))?"
    r"(?=(?:[,\s;]+[a-z0-9_.-]+:[a-z0-9_.:-]+:"
    r"[-+]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:e[-+]?[0-9]+)?"
    r"\s+positive=)|[;\n]|$)",
    re.IGNORECASE,
)


def caseops_vocab_analogy_piece(value: Any) -> str:
    """Normalize one compact vocab-analogy atom."""
    return re.sub(r"[^a-z0-9_.:-]+", "_", str(value).lower()).strip("_")


def encode_caseops_vocab_analogy(
    row: dict[str, Any],
    *,
    limit: int = 5,
    include_score: bool = True,
) -> str:
    """Encode a vocab-analogy row as ``kind:value:score positive=...``."""
    kind = caseops_vocab_analogy_piece(row.get("kind") or "")
    value = caseops_vocab_analogy_piece(row.get("value") or "")
    if not kind or not value:
        return ""
    score = _caseops_vocab_analogy_score(row)
    bits = [f"{kind}:{value}:{score:.3f}" if include_score else f"{kind}:{value}"]
    positive = ",".join(_caseops_vocab_analogy_values(
        row.get("positive_axes"),
        limit=limit,
    ))
    negative = ",".join(_caseops_vocab_analogy_values(
        row.get("negative_axes"),
        limit=limit,
    ))
    if positive:
        bits.append(f"positive={positive}")
    if negative:
        bits.append(f"negative={negative}")
    layers = _caseops_vocab_analogy_layers(row)
    if layers:
        bits.append("layers=" + ",".join(layers))
    metadata = _caseops_vocab_analogy_metadata(row)
    if metadata:
        bits.append("meta=" + ",".join(
            f"{key}:{value}"
            for key, value in metadata
        ))
    return " ".join(bits)


def parse_caseops_vocab_analogies(text: str) -> tuple[dict[str, Any], ...]:
    """Decode compact vocab-analogy rows from text."""
    raw = str(text or "")
    rows: list[dict[str, Any]] = []
    for match in _SECTION_RE.finditer(raw):
        rows.extend(_parse_items(match.group(1)))
    return _dedupe_vocab_analogy_rows(rows)


def parse_caseops_vocab_analogy_item(item: str) -> dict[str, Any] | None:
    """Decode one compact vocab-analogy item."""
    rows = _parse_items(str(item or ""))
    return rows[0] if rows else None


def _parse_items(raw: str) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for match in _ITEM_RE.finditer(str(raw or "")):
        kind = caseops_vocab_analogy_piece(match.group(1))
        value = caseops_vocab_analogy_piece(match.group(2))
        if not kind or not value:
            continue
        rows.append({
            "kind": kind,
            "value": value,
            "score": _safe_float(match.group(3), 0.0),
            "positive_axes": tuple(_caseops_vocab_analogy_csv(match.group(4))),
            "negative_axes": tuple(_caseops_vocab_analogy_csv(match.group(5))),
            "layers": tuple(_caseops_vocab_analogy_csv(match.group(6))),
            "metadata": dict(_caseops_vocab_analogy_metadata_csv(match.group(7))),
        })
    return rows


def _caseops_vocab_analogy_values(
    values: Iterable[Any] | None,
    *,
    limit: int,
) -> tuple[str, ...]:
    out = []
    for value in values or ():
        clean = caseops_vocab_analogy_piece(value)
        if clean:
            out.append(clean)
        if len(out) >= max(1, int(limit)):
            break
    return tuple(dict.fromkeys(out))


def _caseops_vocab_analogy_csv(raw: str | None) -> tuple[str, ...]:
    if raw is None:
        return ()
    values = []
    for part in str(raw or "").split(","):
        clean = caseops_vocab_analogy_piece(part)
        if clean:
            values.append(clean)
    return tuple(dict.fromkeys(values))


def _caseops_vocab_analogy_score(row: dict[str, Any]) -> float:
    return _safe_float(
        row.get("score")
        or row.get("effective_weight")
        or row.get("weight")
        or 0.0,
        0.0,
    )


def _caseops_vocab_analogy_layers(row: dict[str, Any]) -> tuple[str, ...]:
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
        for clean in (caseops_vocab_analogy_piece(value) for value in values)
        if clean
    ))


def _caseops_vocab_analogy_metadata(
    row: dict[str, Any],
) -> tuple[tuple[str, str], ...]:
    raw = row.get("metadata") or row.get("meta") or {}
    if not isinstance(raw, dict):
        return ()
    rows: list[tuple[str, str]] = []
    for key, value in raw.items():
        clean_key = caseops_vocab_analogy_piece(key)
        clean_value = caseops_vocab_analogy_piece(value)
        if clean_key and clean_value:
            rows.append((clean_key, clean_value))
    rows.sort(key=lambda item: item[0])
    return tuple(rows)


def _caseops_vocab_analogy_metadata_csv(
    raw: str | None,
) -> tuple[tuple[str, str], ...]:
    if raw is None:
        return ()
    rows: list[tuple[str, str]] = []
    for part in str(raw or "").split(","):
        key, sep, value = part.partition(":")
        clean_key = caseops_vocab_analogy_piece(key)
        clean_value = caseops_vocab_analogy_piece(value)
        if sep and clean_key and clean_value:
            rows.append((clean_key, clean_value))
    return tuple(dict(rows).items())


def _safe_float(value: Any, default: float) -> float:
    try:
        return float(value)
    except Exception:  # noqa: BLE001
        return float(default)


def _dedupe_vocab_analogy_rows(
    rows: Iterable[dict[str, Any]],
) -> tuple[dict[str, Any], ...]:
    out: list[dict[str, Any]] = []
    seen: set[tuple[Any, ...]] = set()
    for row in rows:
        key = (
            row.get("kind"),
            row.get("value"),
            tuple(row.get("positive_axes", ()) or ()),
            tuple(row.get("negative_axes", ()) or ()),
            tuple(row.get("layers", ()) or ()),
            tuple(sorted((row.get("metadata") or {}).items())),
            round(float(row.get("score") or 0.0), 6),
        )
        if key in seen:
            continue
        seen.add(key)
        out.append(dict(row))
    return tuple(out)

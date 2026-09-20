"""Canonical compact codec for CaseOps crystal-primer rows."""

from __future__ import annotations

import re
from typing import Any, Iterable


_EQUALS_RE = re.compile(
    r"\bcaseops_crystal_primers?(?:_feedback)?=([^\n;]+)",
    re.IGNORECASE,
)
_LABEL_RE = re.compile(
    r"\b(?:learned\s+)?caseops\s+crystal\s+primers?:\s*([^\n]+)",
    re.IGNORECASE,
)


def caseops_crystal_piece(value: Any) -> str:
    """Normalize one compact crystal-primer atom."""
    return re.sub(r"[^a-z0-9_]+", "_", str(value).lower()).strip("_")


def encode_caseops_crystal_primer(
    row: dict[str, Any],
    *,
    limit: int = 4,
    include_score: bool = True,
) -> str:
    """Encode a crystal-primer row as the private pipe codec."""
    card_id = caseops_crystal_piece(row.get("card_id") or row.get("id") or "")
    if not card_id:
        return ""
    kind = caseops_crystal_piece(row.get("kind") or "card") or "card"
    formulas = ",".join(_caseops_crystal_values(row.get("formulas"), limit=limit))
    concepts = ",".join(_caseops_crystal_values(row.get("concepts"), limit=limit))
    stacks = ",".join(_caseops_crystal_stack_values(
        row.get("stacks") or row.get("stack_codes"),
        limit=limit,
    ))
    bits = [kind, card_id]
    if formulas:
        bits.append(f"f={formulas}")
    if concepts:
        bits.append(f"c={concepts}")
    if stacks:
        bits.append(f"s={stacks}")
    if include_score:
        bits.append(f"score={_caseops_crystal_score(row):.3f}")
    return "|".join(bits)


def parse_caseops_crystal_primers(text: str) -> tuple[dict[str, Any], ...]:
    """Decode compact crystal-primer notes from text."""
    raw = str(text or "")
    rows: list[dict[str, Any]] = []
    for match in _EQUALS_RE.finditer(raw):
        for item in match.group(1).split():
            parsed = parse_caseops_crystal_primer_item(item)
            if parsed:
                rows.append(parsed)
    for match in _LABEL_RE.finditer(raw):
        for item in match.group(1).split(";"):
            parsed = parse_caseops_crystal_primer_item(item)
            if parsed:
                rows.append(parsed)
    return _dedupe_crystal_rows(rows)


def parse_caseops_crystal_primer_item(item: str) -> dict[str, Any] | None:
    """Decode one pipe or legacy colon crystal-primer item."""
    text = str(item or "").strip().strip(",")
    if not text:
        return None
    if "|" in text:
        return _parse_pipe_item(text)
    return _parse_legacy_item(text)


def caseops_crystal_stack_pairs(values: Iterable[Any] | None) -> tuple[tuple[str, str], ...]:
    """Return normalized ``(layer, code)`` stack pairs from compact values."""
    pairs: list[tuple[str, str]] = []
    for value in values or ():
        layer, sep, code = str(value or "").partition(":")
        clean_layer = caseops_crystal_piece(layer)
        clean_code = caseops_crystal_piece(code)
        if sep and clean_layer and clean_code:
            pairs.append((clean_layer, clean_code))
    return tuple(dict.fromkeys(pairs))


def _parse_pipe_item(text: str) -> dict[str, Any] | None:
    parts = [part.strip() for part in str(text or "").split("|") if part.strip()]
    if len(parts) < 2:
        return None
    row = _empty_row(parts[0], parts[1])
    if row is None:
        return None
    _parse_fields_into_row(parts[2:], row)
    return row


def _parse_legacy_item(text: str) -> dict[str, Any] | None:
    parts = [part.strip() for part in str(text or "").split(":") if part.strip()]
    if len(parts) < 2:
        return None
    row = _empty_row(parts[0], parts[1])
    if row is None:
        return None
    _parse_fields_into_row(parts[2:], row)
    return row


def _empty_row(kind: Any, card_id: Any) -> dict[str, Any] | None:
    clean_card_id = caseops_crystal_piece(card_id)
    if not clean_card_id:
        return None
    return {
        "kind": caseops_crystal_piece(kind) or "card",
        "card_id": clean_card_id,
        "formulas": (),
        "concepts": (),
        "stacks": (),
        "score": 0.0,
    }


def _parse_fields_into_row(fields: Iterable[str], row: dict[str, Any]) -> None:
    for field in fields:
        key, sep, value = str(field or "").partition("=")
        if not sep:
            continue
        clean_key = key.strip().lower()
        if clean_key in {"f", "formula", "formulas"}:
            row["formulas"] = tuple(_caseops_crystal_csv(value))
        elif clean_key in {"c", "concept", "concepts"}:
            row["concepts"] = tuple(_caseops_crystal_csv(value))
        elif clean_key in {"s", "stack", "stacks", "stack_codes"}:
            row["stacks"] = tuple(_caseops_crystal_stack_csv(value))
        elif clean_key == "score":
            try:
                row["score"] = float(value)
            except (TypeError, ValueError):
                row["score"] = 0.0


def _caseops_crystal_values(value: Any, *, limit: int) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, str):
        items: Iterable[Any] = value.split(",")
    else:
        items = value if isinstance(value, Iterable) else (value,)
    rows = [
        caseops_crystal_piece(item)
        for item in tuple(items)[: max(0, int(limit))]
    ]
    return tuple(dict.fromkeys(row for row in rows if row and row != "-"))


def _caseops_crystal_stack_values(value: Any, *, limit: int) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, str):
        items: Iterable[Any] = value.split(",")
    else:
        items = value if isinstance(value, Iterable) else (value,)
    rows: list[str] = []
    for item in tuple(items)[: max(0, int(limit))]:
        if isinstance(item, dict):
            layer = item.get("layer")
            code = item.get("code")
        elif isinstance(item, (list, tuple)) and len(item) >= 2:
            layer, code = item[0], item[1]
        else:
            layer, sep, code = str(item or "").partition(":")
            if not sep:
                continue
        clean_layer = caseops_crystal_piece(layer)
        clean_code = caseops_crystal_piece(code)
        if clean_layer and clean_code:
            rows.append(f"{clean_layer}:{clean_code}")
    return tuple(dict.fromkeys(rows))


def _caseops_crystal_csv(value: str) -> tuple[str, ...]:
    return tuple(dict.fromkeys(
        item for item in (
            caseops_crystal_piece(part)
            for part in str(value or "").split(",")
        )
        if item and item != "-"
    ))


def _caseops_crystal_stack_csv(value: str) -> tuple[str, ...]:
    return _caseops_crystal_stack_values(str(value or ""), limit=64)


def _caseops_crystal_score(row: dict[str, Any]) -> float:
    try:
        return float(
            row.get("score")
            or row.get("effective_weight")
            or row.get("weight")
            or 0.0
        )
    except (TypeError, ValueError):
        return 0.0


def _dedupe_crystal_rows(rows: Iterable[dict[str, Any]]) -> tuple[dict[str, Any], ...]:
    out: list[dict[str, Any]] = []
    seen: set[tuple[Any, ...]] = set()
    for row in rows:
        key = (
            row.get("kind"),
            row.get("card_id"),
            tuple(row.get("formulas", ()) or ()),
            tuple(row.get("concepts", ()) or ()),
            tuple(row.get("stacks", ()) or ()),
        )
        if key in seen:
            continue
        seen.add(key)
        out.append(dict(row))
    return tuple(out)

"""Compact codec for CaseOps math-language taxonomy rows."""

from __future__ import annotations

import re
from typing import Any, Iterable


_SECTION_RE = re.compile(
    r"\bcaseops[ _]taxonomy(?:_feedback)?\s*[:=]\s*([^\n]+)",
    re.IGNORECASE,
)


def caseops_taxonomy_piece(value: Any) -> str:
    """Normalize one compact taxonomy atom."""
    return re.sub(r"[^a-z0-9_.:-]+", "_", str(value).lower()).strip("_")


def encode_caseops_taxonomy(
    row: dict[str, Any],
    *,
    limit: int = 6,
) -> str:
    """Encode taxonomy context as ``family:<id> english=... pathways=...``."""
    family = row.get("family") if isinstance(row.get("family"), dict) else {}
    family_id = caseops_taxonomy_piece(
        row.get("family_id") or family.get("id") or ""
    )
    if not family_id:
        return ""
    bits = [f"family:{family_id}"]
    values = (
        ("english", family.get("english_structures")),
        ("math", family.get("math_structures")),
        ("glyph", family.get("glyph_attributes")),
        ("related", family.get("related_families")),
    )
    for key, raw in values:
        encoded = _caseops_taxonomy_csv(raw, limit=limit)
        if encoded:
            bits.append(f"{key}={encoded}")
    examples = _caseops_taxonomy_examples(row.get("examples"), limit=limit)
    if examples:
        bits.append("examples=" + ",".join(examples))
    pathways = _caseops_taxonomy_pathways(row.get("examples"), limit=limit)
    if pathways:
        bits.append("pathways=" + ",".join(pathways))
    relations = _caseops_taxonomy_relations(row.get("relation_types"), limit=limit)
    if relations:
        bits.append("relations=" + ",".join(relations))
    layers = _caseops_taxonomy_csv(
        row.get("layers") or ("caseops", "math_language", "taxonomy"),
        limit=limit,
    )
    if layers:
        bits.append(f"layers={layers}")
    return " ".join(bits)


def parse_caseops_taxonomies(text: str) -> tuple[dict[str, Any], ...]:
    """Decode compact taxonomy rows from text."""
    rows: list[dict[str, Any]] = []
    for match in _SECTION_RE.finditer(str(text or "")):
        for item in match.group(1).split(";"):
            parsed = parse_caseops_taxonomy_item(item)
            if parsed:
                rows.append(parsed)
    return _dedupe_taxonomy_rows(rows)


def parse_caseops_taxonomy_item(item: str) -> dict[str, Any] | None:
    """Decode one compact taxonomy row."""
    fields = [field for field in str(item or "").strip().split() if field.strip()]
    if not fields:
        return None
    kind, sep, value = fields[0].partition(":")
    if caseops_taxonomy_piece(kind) != "family" or not sep:
        return None
    family_id = caseops_taxonomy_piece(value)
    if not family_id:
        return None
    row: dict[str, Any] = {
        "family_id": family_id,
        "english_structures": (),
        "math_structures": (),
        "glyph_attributes": (),
        "related_families": (),
        "examples": (),
        "pathways": (),
        "relation_types": (),
        "layers": (),
    }
    for field in fields[1:]:
        key, field_sep, raw_value = field.partition("=")
        if not field_sep:
            continue
        clean_key = caseops_taxonomy_piece(key)
        if clean_key == "english":
            row["english_structures"] = _parse_csv(raw_value)
        elif clean_key == "math":
            row["math_structures"] = _parse_csv(raw_value)
        elif clean_key == "glyph":
            row["glyph_attributes"] = _parse_csv(raw_value)
        elif clean_key == "related":
            row["related_families"] = _parse_csv(raw_value)
        elif clean_key == "examples":
            row["examples"] = _parse_csv(raw_value)
        elif clean_key == "pathways":
            row["pathways"] = _parse_pathways(raw_value)
        elif clean_key == "relations":
            row["relation_types"] = _parse_relations(raw_value)
        elif clean_key == "layers":
            row["layers"] = _parse_csv(raw_value)
    return row


def _caseops_taxonomy_csv(values: Iterable[Any] | None, *, limit: int) -> str:
    if isinstance(values, str):
        values = values.split(",")
    out: list[str] = []
    for value in values or ():
        clean = caseops_taxonomy_piece(value)
        if clean:
            out.append(clean)
        if len(out) >= max(1, int(limit)):
            break
    return ",".join(dict.fromkeys(out))


def _caseops_taxonomy_examples(
    examples: Any,
    *,
    limit: int,
) -> tuple[str, ...]:
    out: list[str] = []
    for row in examples or ():
        if not isinstance(row, dict):
            continue
        example_id = caseops_taxonomy_piece(row.get("id") or "")
        if example_id:
            out.append(example_id)
        if len(out) >= max(1, int(limit)):
            break
    return tuple(dict.fromkeys(out))


def _caseops_taxonomy_pathways(
    examples: Any,
    *,
    limit: int,
) -> tuple[str, ...]:
    out: list[str] = []
    for row in examples or ():
        if not isinstance(row, dict):
            continue
        for pathway in row.get("pathways", ()) or ():
            if not isinstance(pathway, dict):
                continue
            source = caseops_taxonomy_piece(pathway.get("source") or "")
            target = caseops_taxonomy_piece(pathway.get("target") or "")
            relation = caseops_taxonomy_piece(pathway.get("relation") or "")
            if source and target:
                encoded = f"{source}->{target}"
                if relation:
                    encoded = f"{encoded}~{relation}"
                out.append(encoded)
            if len(out) >= max(1, int(limit)):
                return tuple(dict.fromkeys(out))
    return tuple(dict.fromkeys(out))


def _caseops_taxonomy_relations(
    relation_types: Any,
    *,
    limit: int,
) -> tuple[str, ...]:
    out: list[str] = []
    for row in relation_types or ():
        if not isinstance(row, dict):
            continue
        relation_id = caseops_taxonomy_piece(row.get("id") or "")
        direction = caseops_taxonomy_piece(row.get("directionality") or "")
        if relation_id:
            out.append(relation_id + (f":{direction}" if direction else ""))
        if len(out) >= max(1, int(limit)):
            break
    return tuple(dict.fromkeys(out))


def _parse_csv(raw: str) -> tuple[str, ...]:
    return tuple(dict.fromkeys(
        clean
        for clean in (caseops_taxonomy_piece(part) for part in str(raw or "").split(","))
        if clean
    ))


def _parse_pathways(raw: str) -> tuple[dict[str, str], ...]:
    out: list[dict[str, str]] = []
    for part in str(raw or "").split(","):
        link, _, relation = part.partition("~")
        source, sep, target = link.partition("->")
        clean_source = caseops_taxonomy_piece(source)
        clean_target = caseops_taxonomy_piece(target)
        if not sep or not clean_source or not clean_target:
            continue
        row = {"source": clean_source, "target": clean_target}
        clean_relation = caseops_taxonomy_piece(relation)
        if clean_relation:
            row["relation"] = clean_relation
        out.append(row)
    return tuple(out)


def _parse_relations(raw: str) -> tuple[dict[str, str], ...]:
    out: list[dict[str, str]] = []
    for part in str(raw or "").split(","):
        relation_id, _, direction = part.partition(":")
        clean_relation_id = caseops_taxonomy_piece(relation_id)
        if not clean_relation_id:
            continue
        row = {"id": clean_relation_id}
        clean_direction = caseops_taxonomy_piece(direction)
        if clean_direction:
            row["directionality"] = clean_direction
        out.append(row)
    return tuple(out)


def _dedupe_taxonomy_rows(
    rows: Iterable[dict[str, Any]],
) -> tuple[dict[str, Any], ...]:
    out: list[dict[str, Any]] = []
    seen: set[tuple[Any, ...]] = set()
    for row in rows:
        key = (
            row.get("family_id"),
            tuple(row.get("english_structures", ()) or ()),
            tuple(row.get("math_structures", ()) or ()),
            tuple(row.get("glyph_attributes", ()) or ()),
            tuple(row.get("examples", ()) or ()),
            tuple(
                (
                    path.get("source"),
                    path.get("target"),
                    path.get("relation"),
                )
                for path in row.get("pathways", ()) or ()
                if isinstance(path, dict)
            ),
        )
        if key in seen:
            continue
        seen.add(key)
        out.append(dict(row))
    return tuple(out)

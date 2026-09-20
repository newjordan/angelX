"""A small continual-learning kernel for Sloptomizer.

This module uses the paper "Learning, Fast and Slow: Towards LLMs That
Adapt Continually" (arXiv:2605.12484 v2, revised 2026-05-14) as a design
reference, but keeps the implementation intentionally practical and tiny.
The "slow" learner is a lightweight token model; the "fast" learner is a
pool of scored textual context snippets that can be retrieved at inference
time.
"""

from __future__ import annotations

import base64
import hashlib
import json
import math
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

from orchestrator.caseops_codec import caseops_decode as caseops_layer_decode
from orchestrator.caseops_crystal_codec import caseops_crystal_stack_pairs
from orchestrator.caseops_crystal_codec import parse_caseops_crystal_primers
from orchestrator.caseops_taxonomy_codec import parse_caseops_taxonomies
from orchestrator.caseops_vocab_analogy_codec import parse_caseops_vocab_analogies
from orchestrator.caseops_vocab_tensor_codec import encode_caseops_vocab_tensor
from orchestrator.caseops_vocab_tensor_codec import parse_caseops_vocab_tensors


_TOKEN_RE = re.compile(r"[a-z0-9_]+")
_CASEOPS_RE = re.compile(
    r"SLOP1~([A-Za-z0-9_]+)~([A-Za-z0-9_.]+)~([A-Za-z0-9_-]+)"
)
_CASEOPS_PATHWAY_RE = re.compile(
    r"\b([a-z][a-z0-9_.-]*)\s*->\s*([a-z][a-z0-9_.-]*)\b",
    re.IGNORECASE,
)
_CASEOPS_MATH_LANGUAGE_PATHWAY_RE = re.compile(
    r"\b([a-z][a-z0-9_.-]*)\s*->\s*(lang\.[a-z0-9_.-]+)\s*:\s*([a-z0-9_][a-z0-9_. -]*)",
    re.IGNORECASE,
)
_CASEOPS_ROUTE_SCORE = r"[-+]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:e[-+]?[0-9]+)?"
_CASEOPS_CODEC_GAP_RE = re.compile(
    r"\bfuture\.codec_gap:([a-z0-9_.-]+)\b",
    re.IGNORECASE,
)
_CASEOPS_GAP_RE = re.compile(
    r"\bcaseops_gap(?:_feedback)?="
    r"([a-z0-9_.-]+)\|([^|\s]+)\|([a-z0-9_.-]+)\|"
    rf"({_CASEOPS_ROUTE_SCORE})(?:\|([^|\s]+))?",
    re.IGNORECASE,
)
_PREDICTIVE_GUESS_LEDGER_RE = re.compile(
    r"\bpredictive_guess_ledger(?:_feedback)?=([^\s;]+)",
    re.IGNORECASE,
)
_CASEOPS_VOCAB_VECTOR_RE = re.compile(
    r"\bcaseops vocab vectors:\s*([^\n|]+)",
    re.IGNORECASE,
)
_CASEOPS_VOCAB_VECTOR_ITEM_RE = re.compile(
    r"\s*([a-z0-9_.-]+):([a-z0-9_.:-]+)\s+axes=([^;]+)",
    re.IGNORECASE,
)
_CASEOPS_STACK_CODES_RE = re.compile(
    r"\bcaseops stack codes:\s*([^\n|]+)",
    re.IGNORECASE,
)
_CASEOPS_STACK_CODE_HEAD_RE = re.compile(
    r"^\s*([a-z0-9_.-]+):([a-z0-9_.:-]+)\s+code=([0-9]+|-)(.*)$",
    re.IGNORECASE,
)
_KNOWLEDGE_CARD_RE = re.compile(
    r"^#\s+KnowledgeCard\s+`([^`]+)`\s+\(([^)]+)\)",
    re.IGNORECASE | re.MULTILINE,
)
_KNOWLEDGE_CARD_FIELD_RE = re.compile(
    r"^\*\*(SLOP-IR hash|Stack codes):\*\*\s*(.+)$",
    re.IGNORECASE | re.MULTILINE,
)
_KNOWLEDGE_CARD_SECTION_RE = re.compile(r"^##\s+(.+?)\s*$", re.MULTILINE)
_KNOWLEDGE_CARD_TICK_RE = re.compile(r"`([^`]+)`")
_KNOWLEDGE_CARD_TAG_RE = re.compile(r"`([^`]+)`\s*=\s*`([^`]+)`")
_CASEOPS_LINGUISTIC_LINK_RE = re.compile(
    r"\bling\.(concept|role|predicate|idiom|quantifier):"
    r"([a-z0-9_.-]+)(?::([a-z0-9_.-]+))?\b",
    re.IGNORECASE,
)
_CASEOPS_REPLAY_SCHEMA_RE = re.compile(
    r"caseops replay schema:\s*pathways/([0-9]+)",
    re.IGNORECASE,
)
_CASEOPS_TENSOR_CODEBOOK_PACKET_RE = re.compile(
    r"\bCTV1\|([^|\s]+)\|([a-z0-9_.-]+):([a-z0-9_.:-]+)\|([^\s|]+)",
    re.IGNORECASE,
)
_CASEOPS_TENSOR_CODEBOOK_ROUTE_PACKET_RE = re.compile(
    r"\bCVR1\|([a-z0-9_.:-]+)\|([a-z0-9_.:-]+)\|([a-z0-9_.:-]+)\|"
    r"(positive|negative|neutral)\|([a-z0-9_.:-]+)\|([a-z0-9_.:-]+)",
    re.IGNORECASE,
)
_CASEOPS_TENSOR_CODEBOOK_AXIS_RE = re.compile(
    r"\b(ax\.[a-z0-9_.-]+)=([a-z0-9_.:-]+)\b",
    re.IGNORECASE,
)
_CASEOPS_MODEL_TRAIN_RE = re.compile(
    r"\bcaseops_model_train\s+"
    r"([a-z0-9_.-]+):([a-z0-9_.-]+):([a-z0-9_.:-]+)"
    r"\s*->\s*([^\n;]+)",
    re.IGNORECASE,
)
_CASEOPS_MODEL_TRAIN_TARGET_RE = re.compile(
    r"([a-z0-9_.-]+):([a-z0-9_.-]+):([a-z0-9_.:-]+)",
    re.IGNORECASE,
)
_CASEOPS_ACTIVATION_ROUTE_RE = re.compile(
    r"\b((?:(?:formula|language)_[a-z0-9_]+)"
    r"(?:=>(?:(?:formula|language)_[a-z0-9_]+))+)"
    rf"(?::({_CASEOPS_ROUTE_SCORE}))?\b",
    re.IGNORECASE,
)
_STATE_VERSION = 1
_MODEL_LANGUAGE_VERSION = 1
_DEFAULT_MODEL_LANGUAGE_LAYERS: tuple[dict[str, Any], ...] = (
    {
        "layer": "surface",
        "depth": 0,
        "kernel": False,
        "stability": "ephemeral",
        "max_tokens": 512,
        "train_when_oversized": False,
        "consolidates_to": (),
    },
    {
        "layer": "system",
        "depth": 0,
        "kernel": True,
        "stability": "locked_kernel",
        "max_tokens": 512,
        "train_when_oversized": False,
        "consolidates_to": (),
    },
    {
        "layer": "math",
        "depth": 1,
        "kernel": True,
        "stability": "foundation_kernel",
        "max_tokens": 4096,
        "train_when_oversized": False,
        "consolidates_to": (),
    },
    {
        "layer": "language",
        "depth": 1,
        "kernel": True,
        "stability": "foundation_kernel",
        "max_tokens": 4096,
        "train_when_oversized": False,
        "consolidates_to": (),
    },
    {
        "layer": "mathlang",
        "depth": 2,
        "kernel": False,
        "stability": "bridge",
        "max_tokens": 2048,
        "train_when_oversized": True,
        "consolidates_to": ("math", "language"),
    },
    {
        "layer": "crystal",
        "depth": 3,
        "kernel": False,
        "stability": "learned_memory",
        "max_tokens": 2048,
        "train_when_oversized": True,
        "consolidates_to": ("mathlang", "math", "language"),
    },
    {
        "layer": "future:*",
        "depth": 9,
        "kernel": False,
        "stability": "reserved_open_layer",
        "max_tokens": 0,
        "train_when_oversized": False,
        "consolidates_to": (),
    },
)


def _tokens(text: str) -> list[str]:
    """Tokenize ordinary text plus Sloptomizer-private CaseOps symbols."""
    raw = str(text or "")
    tokens = _TOKEN_RE.findall(raw.lower())
    pending_formula = ""
    for match in _CASEOPS_RE.finditer(raw):
        layer, kind, payload = match.groups()
        tokens.extend(_caseops_feature_tokens(layer, kind, payload))
        clean_kind = _normalize_feature_piece(kind)
        clean_value = _normalize_feature_piece(_decode_caseops_payload(payload))
        if clean_kind.endswith("_formula") and clean_value:
            pending_formula = clean_value
        elif clean_kind.endswith("_family") and clean_value:
            if pending_formula:
                tokens.extend(_caseops_formula_family_feature_tokens(
                    pending_formula,
                    clean_value,
                ))
                pending_formula = ""
    for match in _CASEOPS_CODEC_GAP_RE.finditer(raw):
        tokens.extend(_caseops_codec_gap_feature_tokens(match.group(1)))
    for match in _CASEOPS_GAP_RE.finditer(raw):
        tokens.extend(_caseops_gap_feature_tokens(
            match.group(1),
            match.group(2),
            match.group(3),
            match.group(5),
        ))
    for row in _parse_predictive_guess_ledgers(raw):
        tokens.extend(_predictive_guess_ledger_feature_tokens(row))
    for kind, value, axes in _parse_caseops_vocab_vector_items(raw):
        tokens.extend(_caseops_vocab_vector_feature_tokens(kind, value, axes))
    for row in _parse_caseops_vocab_analogy_items(raw):
        tokens.extend(_caseops_vocab_analogy_feature_tokens(row))
    for row in _parse_caseops_vocab_tensor_items(raw):
        tokens.extend(_caseops_vocab_tensor_feature_tokens(row))
    for row in _parse_caseops_vocab_tensor_codebook_items(raw):
        tokens.extend(_caseops_vocab_tensor_codebook_feature_tokens(row))
    for row in _parse_caseops_vocab_tensor_codebook_route_items(raw):
        tokens.extend(_caseops_vocab_tensor_codebook_route_feature_tokens(row))
    for row in _parse_caseops_layered_vocab_items(raw):
        tokens.extend(_caseops_layered_vocab_feature_tokens(row))
    for row in _parse_caseops_model_train_items(raw):
        tokens.extend(_caseops_model_train_feature_tokens(row))
    for row in _parse_caseops_taxonomy_items(raw):
        tokens.extend(_caseops_taxonomy_feature_tokens(row))
    for row in _parse_caseops_stack_code_items(raw):
        tokens.extend(_caseops_stack_code_feature_tokens(row))
    for row in _parse_knowledge_card_items(raw):
        tokens.extend(_knowledge_card_feature_tokens(row))
    for row in _parse_caseops_crystal_primer_items(raw):
        tokens.extend(_knowledge_card_feature_tokens(row))
    for match in _CASEOPS_LINGUISTIC_LINK_RE.finditer(raw):
        tokens.extend(_caseops_linguistic_link_feature_tokens(*match.groups()))
    math_language_spans: list[tuple[int, int]] = []
    for match in _CASEOPS_MATH_LANGUAGE_PATHWAY_RE.finditer(raw):
        tokens.extend(_caseops_math_language_feature_tokens(*match.groups()))
        math_language_spans.append(match.span())
    for match in _CASEOPS_ACTIVATION_ROUTE_RE.finditer(raw):
        tokens.extend(_caseops_activation_route_feature_tokens(match.group(1)))
    for match in _CASEOPS_PATHWAY_RE.finditer(raw):
        start, end = match.span()
        if any(start < span_end and end > span_start for span_start, span_end in math_language_spans):
            continue
        tokens.extend(_caseops_pathway_feature_tokens(*match.groups()))
    return tokens


def _caseops_feature_tokens(layer: str, kind: str, payload: str) -> tuple[str, ...]:
    clean_layer = _normalize_feature_piece(layer)
    clean_kind = _normalize_feature_piece(kind)
    decoded = _decode_caseops_payload(payload)
    value_tokens = _TOKEN_RE.findall(decoded.lower())
    features = [
        "caseops",
        f"caseops_layer_{clean_layer}",
        f"caseops_kind_{clean_kind}",
        f"caseops_pair_{clean_layer}_{clean_kind}",
    ]
    if value_tokens:
        value = "_".join(value_tokens[:8])
        features.append(f"caseops_value_{value}")
        features.extend(_split_value_tokens(value_tokens))
        if clean_kind.endswith("_formula"):
            features.append(f"caseops_formula_{value}")
        elif clean_kind.endswith("_family"):
            features.append(f"caseops_family_{value}")
    return tuple(features)


def _caseops_formula_family_feature_tokens(
    formula_id: str,
    family_id: str,
) -> tuple[str, ...]:
    clean_formula = _normalize_feature_piece(formula_id)
    clean_family = _normalize_feature_piece(family_id)
    if not clean_formula or not clean_family:
        return ()
    return (
        "caseops",
        "caseops_belongs_to_family_synapse",
        f"caseops_formula_{clean_formula}",
        f"caseops_family_{clean_family}",
        f"caseops_belongs_to_family_pair_{clean_formula}__{clean_family}",
    )


def _caseops_pathway_feature_tokens(source: str, target: str) -> tuple[str, ...]:
    if str(source).lower().startswith("lang.") or str(target).lower().startswith("lang."):
        return ()
    clean_source = _normalize_feature_piece(source)
    clean_target = _normalize_feature_piece(target)
    if not clean_source or not clean_target:
        return ()
    ordered = "__".join(sorted((clean_source, clean_target)))
    return (
        "caseops",
        "caseops_pathway",
        f"caseops_formula_{clean_source}",
        f"caseops_formula_{clean_target}",
        f"caseops_pathway_directed_{clean_source}__{clean_target}",
        f"caseops_pathway_pair_{ordered}",
    )


def _caseops_codec_gap_feature_tokens(value: str) -> tuple[str, ...]:
    clean_value = _normalize_feature_piece(value)
    value_tokens = _TOKEN_RE.findall(str(value or "").lower())
    if not clean_value:
        return ()
    return (
        "caseops",
        "caseops_codec_gap",
        "caseops_open_layer",
        "caseops_future_layer_codec_gap",
        f"caseops_codec_gap_{clean_value}",
        *tuple(_split_value_tokens(value_tokens)),
    )


def _caseops_gap_feature_tokens(
    kind: str,
    target: str,
    reason: str,
    source_token: str | None = None,
) -> tuple[str, ...]:
    clean_kind = _normalize_feature_piece(kind)
    clean_target = _normalize_feature_piece(target)
    clean_reason = _normalize_feature_piece(reason)
    clean_source = _normalize_feature_piece(source_token) if source_token else ""
    value_tokens = _TOKEN_RE.findall(" ".join((
        str(kind or ""),
        str(target or ""),
        str(reason or ""),
        str(source_token or ""),
    )).lower())
    if not clean_kind or not clean_target or not clean_reason:
        return ()
    features = [
        "caseops",
        "caseops_gap",
        f"caseops_gap_kind_{clean_kind}",
        f"caseops_gap_target_{clean_target}",
        f"caseops_gap_reason_{clean_reason}",
        f"caseops_gap_repair_{clean_kind}__{clean_target}__{clean_reason}",
        *tuple(_split_value_tokens(value_tokens)),
    ]
    if clean_source and clean_source != "unknown":
        features.append(f"caseops_gap_source_{clean_source}")
    return tuple(features)


def _parse_predictive_guess_ledgers(raw: str) -> tuple[dict[str, str], ...]:
    rows: list[dict[str, str]] = []
    for match in _PREDICTIVE_GUESS_LEDGER_RE.finditer(raw):
        fields: dict[str, str] = {}
        for part in match.group(1).split("|"):
            key, sep, value = part.partition("=")
            if not sep:
                continue
            clean_key = _normalize_feature_piece(key)
            clean_value = str(value or "").strip()
            if clean_key and clean_value:
                fields[clean_key] = clean_value
        if fields:
            rows.append(fields)
    return tuple(rows)


def _predictive_guess_ledger_feature_tokens(row: dict[str, str]) -> tuple[str, ...]:
    mode = _normalize_feature_piece(row.get("mode", ""))
    object_type = _normalize_feature_piece(row.get("object", ""))
    ambiguity = _normalize_feature_piece(row.get("ambiguity", ""))
    competition = _normalize_feature_piece(row.get("competition", ""))
    resolution = _normalize_feature_piece(row.get("resolution", ""))
    family = _normalize_feature_piece(row.get("family", ""))
    verdict = _normalize_feature_piece(row.get("verdict", ""))
    outcome = _normalize_feature_piece(row.get("outcome", ""))
    surprise_label = _normalize_feature_piece(row.get("surprise_label", ""))
    tokens = [
        "predictive_guess_ledger",
        "caseops_predictive_guess_ledger",
    ]
    if mode:
        tokens.append(f"predictive_guess_mode_{mode}")
    if object_type:
        tokens.append(f"predictive_guess_object_{object_type}")
    if mode and object_type:
        tokens.append(f"predictive_guess_mode_object_{mode}__{object_type}")
    if ambiguity:
        tokens.append(f"predictive_guess_ambiguity_{ambiguity}")
    if mode and ambiguity:
        tokens.append(f"predictive_guess_mode_ambiguity_{mode}__{ambiguity}")
    if competition:
        tokens.append(f"predictive_guess_competition_{competition}")
    if object_type and competition:
        tokens.append(
            f"predictive_guess_object_competition_{object_type}__{competition}"
        )
    if resolution:
        tokens.append(f"predictive_guess_resolution_{resolution}")
    if mode and resolution:
        tokens.append(f"predictive_guess_mode_resolution_{mode}__{resolution}")
    if object_type and resolution:
        tokens.append(
            f"predictive_guess_object_resolution_{object_type}__{resolution}"
        )
    if family:
        tokens.append(f"predictive_guess_family_{family}")
    if verdict:
        tokens.append(f"predictive_guess_verdict_{verdict}")
        if verdict == "pass":
            tokens.append("predictive_guess_success")
        elif verdict == "fail":
            tokens.append("predictive_guess_failure")
    if outcome:
        tokens.append(f"predictive_guess_outcome_{outcome}")
    if surprise_label:
        tokens.append(f"predictive_guess_surprise_{surprise_label}")
    posterior = _safe_float(row.get("calibrated") or row.get("posterior"))
    if posterior is not None:
        tokens.append(_probability_bucket_token("posterior", posterior))
    risk = _safe_float(row.get("risk"))
    if risk is not None:
        tokens.append(_probability_bucket_token("risk", risk, invert=True))
    neig = _safe_float(row.get("neig"))
    if neig is not None:
        tokens.append(_probability_bucket_token("neig", neig))
    surprise = _safe_float(row.get("surprise"))
    if surprise is not None:
        tokens.append(_surprise_bucket_token(surprise))
    brier = _safe_float(row.get("brier"))
    if brier is not None:
        tokens.append(_probability_bucket_token("brier_loss", brier, invert=True))
    residual = _safe_float(row.get("residual"))
    if residual is not None:
        if residual > 0:
            tokens.append("predictive_guess_underconfident")
        elif residual < 0:
            tokens.append("predictive_guess_overconfident")
    reward = _safe_float(row.get("reward"))
    if reward is not None:
        if reward > 0:
            tokens.append("predictive_guess_positive_reward")
        elif reward < 0:
            tokens.append("predictive_guess_negative_reward")
    return tuple(token for token in tokens if token)


def _probability_bucket_token(
    label: str,
    value: float,
    *,
    invert: bool = False,
) -> str:
    clean_label = _normalize_feature_piece(label)
    bounded = max(0.0, min(1.0, float(value)))
    if invert:
        bounded = 1.0 - bounded
    if bounded >= 0.75:
        bucket = "high"
    elif bounded >= 0.40:
        bucket = "medium"
    else:
        bucket = "low"
    return f"predictive_guess_{clean_label}_{bucket}"


def _surprise_bucket_token(value: float) -> str:
    surprise = max(0.0, float(value))
    if surprise >= 2.0:
        bucket = "high"
    elif surprise >= 0.75:
        bucket = "medium"
    else:
        bucket = "low"
    return f"predictive_guess_surprise_bits_{bucket}"


def _safe_float(value: Any) -> float | None:
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(parsed):
        return None
    return parsed


def _parse_caseops_vocab_vector_items(
    text: str,
) -> tuple[tuple[str, str, tuple[str, ...]], ...]:
    rows: list[tuple[str, str, tuple[str, ...]]] = []
    for line_match in _CASEOPS_VOCAB_VECTOR_RE.finditer(str(text or "")):
        body = line_match.group(1)
        for item_match in _CASEOPS_VOCAB_VECTOR_ITEM_RE.finditer(body):
            axes = tuple(
                axis.strip().lower()
                for axis in item_match.group(3).split(",")
                if axis.strip()
            )
            rows.append((
                item_match.group(1).lower(),
                item_match.group(2).lower(),
                axes,
            ))
    return tuple(rows)


def _caseops_vocab_vector_feature_tokens(
    kind: str,
    value: str,
    axes: Iterable[str],
) -> tuple[str, ...]:
    clean_kind = _normalize_feature_piece(kind)
    clean_value = _normalize_feature_piece(value)
    if not clean_kind or not clean_value:
        return ()
    features = [
        "caseops",
        "caseops_vocab_vector",
        f"caseops_vocab_kind_{clean_kind}",
        f"caseops_vocab_value_{clean_value}",
        f"caseops_vocab_pair_{clean_kind}__{clean_value}",
        *tuple(_split_value_tokens(_TOKEN_RE.findall(str(value or "").lower()))),
    ]
    for axis in axes:
        clean_axis = _normalize_feature_piece(axis)
        if not clean_axis:
            continue
        axis_root = _normalize_feature_piece(str(axis).split(":", 1)[0])
        features.append(f"caseops_vocab_axis_{clean_axis}")
        features.append(
            f"caseops_vocab_axis_pair_{clean_kind}__{clean_value}__{clean_axis}"
        )
        if axis_root:
            features.append(f"caseops_vocab_axis_root_{axis_root}")
        if str(axis).startswith("layer:"):
            _, _, layer = str(axis).partition(":")
            clean_layer = _normalize_feature_piece(layer)
            if clean_layer:
                features.append(f"caseops_layer_{clean_layer}")
        if str(axis).startswith("kind:"):
            _, _, axis_kind = str(axis).partition(":")
            clean_axis_kind = _normalize_feature_piece(axis_kind)
            if clean_axis_kind:
                features.append(f"caseops_kind_{clean_axis_kind}")
    clean_axes = sorted({
        _normalize_feature_piece(axis)
        for axis in axes
        if _normalize_feature_piece(axis)
    })
    for index, left in enumerate(clean_axes):
        for right in clean_axes[index + 1:]:
            features.append(f"caseops_vocab_axis_pathway_{left}__{right}")
            features.append(
                f"caseops_vocab_kind_axis_pathway_{clean_kind}__{left}__{right}"
            )
    return tuple(features)


def _parse_caseops_vocab_analogy_items(text: str) -> tuple[dict[str, Any], ...]:
    return parse_caseops_vocab_analogies(text)


def _caseops_vocab_analogy_feature_tokens(row: dict[str, Any]) -> tuple[str, ...]:
    kind = str(row.get("kind") or "")
    value = str(row.get("value") or "")
    clean_kind = _normalize_feature_piece(kind)
    clean_value = _normalize_feature_piece(value)
    if not clean_kind or not clean_value:
        return ()
    positive_axes = tuple(str(axis) for axis in row.get("positive_axes", ()) or ())
    negative_axes = tuple(str(axis) for axis in row.get("negative_axes", ()) or ())
    features = [
        "caseops",
        "caseops_vocab_analogy",
        f"caseops_vocab_analogy_kind_{clean_kind}",
        f"caseops_vocab_analogy_value_{clean_value}",
        f"caseops_vocab_analogy_pair_{clean_kind}__{clean_value}",
        f"caseops_vocab_pair_{clean_kind}__{clean_value}",
        *tuple(_split_value_tokens(_TOKEN_RE.findall(value.lower()))),
    ]
    for role, axes in (("positive", positive_axes), ("negative", negative_axes)):
        for axis in axes:
            clean_axis = _normalize_feature_piece(axis)
            if not clean_axis:
                continue
            axis_root = _normalize_feature_piece(str(axis).split(":", 1)[0])
            features.append(f"caseops_vocab_analogy_{role}_axis_{clean_axis}")
            features.append(
                f"caseops_vocab_analogy_{role}_axis_pair_"
                f"{clean_kind}__{clean_value}__{clean_axis}"
            )
            features.append(f"caseops_vocab_axis_{clean_axis}")
            features.append(
                f"caseops_vocab_axis_pair_{clean_kind}__{clean_value}__{clean_axis}"
            )
            if axis_root:
                features.append(f"caseops_vocab_analogy_{role}_axis_root_{axis_root}")
                features.append(f"caseops_vocab_axis_root_{axis_root}")
    clean_positive = sorted({
        _normalize_feature_piece(axis)
        for axis in positive_axes
        if _normalize_feature_piece(axis)
    })
    for index, left in enumerate(clean_positive):
        for right in clean_positive[index + 1:]:
            features.append(f"caseops_vocab_axis_pathway_{left}__{right}")
            features.append(
                f"caseops_vocab_analogy_positive_pathway_{left}__{right}"
            )
    for positive in clean_positive:
        for negative in sorted({
            _normalize_feature_piece(axis)
            for axis in negative_axes
            if _normalize_feature_piece(axis)
        }):
            features.append(
                f"caseops_vocab_analogy_contrast_{positive}__{negative}"
            )
            features.append(
                f"caseops_vocab_analogy_contrast_pair_"
                f"{clean_kind}__{clean_value}__{positive}__{negative}"
            )
    return tuple(features)


def _parse_caseops_vocab_tensor_items(text: str) -> tuple[dict[str, Any], ...]:
    return parse_caseops_vocab_tensors(text)


def _caseops_vocab_tensor_feature_tokens(row: dict[str, Any]) -> tuple[str, ...]:
    kind = str(row.get("kind") or "")
    value = str(row.get("value") or "")
    clean_kind = _normalize_feature_piece(kind)
    clean_value = _normalize_feature_piece(value)
    if not clean_kind or not clean_value:
        return ()
    coordinates = row.get("coordinates") or {}
    if not isinstance(coordinates, dict):
        return ()
    features = [
        "caseops",
        "caseops_vocab_tensor",
        f"caseops_vocab_tensor_kind_{clean_kind}",
        f"caseops_vocab_tensor_value_{clean_value}",
        f"caseops_vocab_tensor_pair_{clean_kind}__{clean_value}",
        f"caseops_vocab_pair_{clean_kind}__{clean_value}",
        *tuple(_split_value_tokens(_TOKEN_RE.findall(value.lower()))),
    ]
    positive_axes: list[str] = []
    negative_axes: list[str] = []
    for axis, raw_weight in coordinates.items():
        clean_axis = _normalize_feature_piece(str(axis))
        if not clean_axis:
            continue
        try:
            weight = float(raw_weight)
        except Exception:  # noqa: BLE001
            continue
        if weight == 0.0:
            continue
        axis_root = _normalize_feature_piece(str(axis).split(":", 1)[0])
        role = "positive" if weight > 0.0 else "negative"
        if role == "positive":
            positive_axes.append(clean_axis)
            features.append(
                f"caseops_vocab_axis_pair_{clean_kind}__{clean_value}__{clean_axis}"
            )
        else:
            negative_axes.append(clean_axis)
            features.append(
                f"caseops_vocab_analogy_negative_axis_pair_"
                f"{clean_kind}__{clean_value}__{clean_axis}"
            )
        features.append(f"caseops_vocab_tensor_axis_{clean_axis}")
        features.append(f"caseops_vocab_tensor_{role}_axis_{clean_axis}")
        features.append(
            f"caseops_vocab_tensor_{role}_axis_pair_"
            f"{clean_kind}__{clean_value}__{clean_axis}"
        )
        features.append(
            f"caseops_vocab_tensor_coordinate_pair_"
            f"{clean_kind}__{clean_value}__{clean_axis}"
        )
        features.append(f"caseops_vocab_axis_{clean_axis}")
        if axis_root:
            features.append(f"caseops_vocab_tensor_axis_root_{axis_root}")
            features.append(f"caseops_vocab_axis_root_{axis_root}")
    if negative_axes:
        features.append(f"caseops_vocab_analogy_pair_{clean_kind}__{clean_value}")
        for positive in positive_axes:
            for negative in negative_axes:
                features.append(
                    f"caseops_vocab_analogy_contrast_pair_"
                    f"{clean_kind}__{clean_value}__{positive}__{negative}"
                )
    for axes in (positive_axes, negative_axes):
        for index, left in enumerate(sorted(dict.fromkeys(axes))):
            for right in sorted(dict.fromkeys(axes))[index + 1:]:
                features.append(f"caseops_vocab_tensor_axis_pathway_{left}__{right}")
    return tuple(features)


def _parse_caseops_vocab_tensor_codebook_items(text: str) -> tuple[dict[str, Any], ...]:
    raw = str(text or "")
    axis_map = {
        match.group(1).lower(): match.group(2).lower()
        for match in _CASEOPS_TENSOR_CODEBOOK_AXIS_RE.finditer(raw)
    }
    rows: list[dict[str, Any]] = []
    for match in _CASEOPS_TENSOR_CODEBOOK_PACKET_RE.finditer(raw):
        coordinates: dict[str, float] = {}
        decoded_axes: dict[str, str] = {}
        for item in match.group(4).split(","):
            axis_code, sep, raw_weight = item.rpartition(":")
            if not sep:
                continue
            axis_code = axis_code.lower()
            try:
                weight = float(raw_weight)
            except Exception:  # noqa: BLE001
                continue
            if not axis_code or weight == 0.0:
                continue
            coordinates[axis_code] = weight
            if axis_code in axis_map:
                decoded_axes[axis_code] = axis_map[axis_code]
        if not coordinates:
            continue
        rows.append({
            "tensor_code": match.group(1).lower(),
            "kind": match.group(2).lower(),
            "value": match.group(3).lower(),
            "coordinates": coordinates,
            "decoded_axes": decoded_axes,
        })
    return tuple(rows)


def _caseops_vocab_tensor_codebook_feature_tokens(row: dict[str, Any]) -> tuple[str, ...]:
    tensor_code = _normalize_feature_piece(str(row.get("tensor_code") or ""))
    kind = _normalize_feature_piece(str(row.get("kind") or ""))
    value = _normalize_feature_piece(str(row.get("value") or ""))
    if not tensor_code or not kind or not value:
        return ()
    features = [
        "caseops",
        "caseops_vocab_tensor_codebook",
        f"caseops_vocab_tensor_codebook_tensor_{tensor_code}",
        f"caseops_vocab_tensor_codebook_kind_{kind}",
        f"caseops_vocab_tensor_codebook_value_{value}",
        f"caseops_vocab_tensor_codebook_pair_{kind}__{value}",
        f"caseops_vocab_tensor_codebook_code_pair_{tensor_code}__{kind}__{value}",
    ]
    coordinates = row.get("coordinates") or {}
    decoded_axes = row.get("decoded_axes") or {}
    if not isinstance(coordinates, dict):
        return tuple(features)
    for axis_code, raw_weight in coordinates.items():
        clean_axis_code = _normalize_feature_piece(str(axis_code))
        if not clean_axis_code:
            continue
        try:
            weight = float(raw_weight)
        except Exception:  # noqa: BLE001
            continue
        if weight == 0.0:
            continue
        role = "positive" if weight > 0.0 else "negative"
        features.append(f"caseops_vocab_tensor_codebook_axis_{clean_axis_code}")
        features.append(
            f"caseops_vocab_tensor_codebook_{role}_axis_{clean_axis_code}"
        )
        features.append(
            f"caseops_vocab_tensor_codebook_coordinate_pair_"
            f"{tensor_code}__{clean_axis_code}"
        )
        axis = str(decoded_axes.get(str(axis_code), "") or "")
        clean_axis = _normalize_feature_piece(axis) if axis else ""
        if clean_axis:
            features.append(
                f"caseops_vocab_tensor_codebook_axis_decode_"
                f"{clean_axis_code}__{clean_axis}"
            )
            features.append(f"caseops_vocab_axis_{clean_axis}")
            features.append(
                f"caseops_vocab_tensor_codebook_{role}_axis_pair_"
                f"{kind}__{value}__{clean_axis}"
            )
    return tuple(features)


def _parse_caseops_vocab_tensor_codebook_route_items(
    text: str,
) -> tuple[dict[str, Any], ...]:
    rows: list[dict[str, Any]] = []
    for match in _CASEOPS_TENSOR_CODEBOOK_ROUTE_PACKET_RE.finditer(str(text or "")):
        route_code, tensor_code, axis_code, polarity, source, target = match.groups()
        axis = ""
        if target.lower().startswith("axis:"):
            axis = target.split(":", 1)[1]
        rows.append({
            "route_code": route_code.lower(),
            "tensor_code": tensor_code.lower(),
            "axis_code": axis_code.lower(),
            "polarity": polarity.lower(),
            "source": source.lower(),
            "target": target.lower(),
            "axis": axis.lower(),
        })
    return tuple(rows)


def _caseops_vocab_tensor_codebook_route_feature_tokens(
    row: dict[str, Any],
) -> tuple[str, ...]:
    route_code = _normalize_feature_piece(str(row.get("route_code") or ""))
    tensor_code = _normalize_feature_piece(str(row.get("tensor_code") or ""))
    axis_code = _normalize_feature_piece(str(row.get("axis_code") or ""))
    polarity = _normalize_feature_piece(str(row.get("polarity") or "neutral"))
    source = str(row.get("source") or "")
    target = str(row.get("target") or "")
    source_key = _normalize_feature_piece(source)
    target_key = _normalize_feature_piece(target)
    if not route_code or not tensor_code or not axis_code or not source_key or not target_key:
        return ()
    features = [
        "caseops",
        "caseops_vocab_tensor_codebook_route",
        f"caseops_vocab_tensor_codebook_route_{route_code}",
        f"caseops_vocab_tensor_codebook_route_tensor_{tensor_code}",
        f"caseops_vocab_tensor_codebook_route_axis_{axis_code}",
        f"caseops_vocab_tensor_codebook_route_polarity_{polarity}",
        f"caseops_vocab_tensor_codebook_route_source_{source_key}",
        f"caseops_vocab_tensor_codebook_route_target_{target_key}",
        f"caseops_vocab_tensor_codebook_route_pair_{source_key}__{target_key}",
        f"caseops_vocab_tensor_codebook_route_tensor_axis_{tensor_code}__{axis_code}",
        (
            "caseops_vocab_tensor_codebook_route_"
            f"{polarity}_pair_{source_key}__{target_key}"
        ),
    ]
    axis = str(row.get("axis") or "")
    if not axis and target.lower().startswith("axis:"):
        axis = target.split(":", 1)[1]
    clean_axis = _normalize_feature_piece(axis)
    if clean_axis:
        features.append(f"caseops_vocab_axis_{clean_axis}")
    if source.lower().startswith("term:") and clean_axis:
        term = source.split(":", 1)[1]
        kind, sep, value = term.partition(":")
        clean_kind = _normalize_feature_piece(kind)
        clean_value = _normalize_feature_piece(value)
        if sep and clean_kind and clean_value:
            features.append(
                "caseops_vocab_tensor_codebook_route_term_axis_pair_"
                f"{clean_kind}__{clean_value}__{clean_axis}"
            )
            features.append(
                "caseops_vocab_tensor_codebook_route_"
                f"{polarity}_axis_pair_{clean_kind}__{clean_value}__{clean_axis}"
            )
    return tuple(features)


def _parse_caseops_layered_vocab_items(text: str) -> tuple[dict[str, Any], ...]:
    try:
        return caseops_layer_decode(
            text,
            layers=("surface", "system", "math", "language", "mathlang", "glyph"),
        )
    except Exception:  # noqa: BLE001
        return ()


def _caseops_layered_vocab_feature_tokens(row: dict[str, Any]) -> tuple[str, ...]:
    layer = _normalize_feature_piece(str(row.get("layer") or ""))
    kind = _normalize_feature_piece(str(row.get("kind") or ""))
    value = _normalize_feature_piece(str(row.get("value") or ""))
    if not layer or not kind or not value:
        return ()
    features: list[str] = [
        "caseops",
        "caseops_layered_vocab",
        f"caseops_layer_{layer}",
        f"caseops_layered_layer_{layer}",
        f"caseops_layered_kind_{kind}",
        f"caseops_layered_value_{value}",
        f"caseops_layered_pair_{layer}__{kind}__{value}",
        *tuple(_split_value_tokens(_TOKEN_RE.findall(str(row.get("value") or "").lower()))),
    ]
    if layer in {"system", "math", "language"} and kind == "kernel":
        features.extend((
            "caseops_model_kernel",
            f"caseops_model_kernel_layer_{layer}",
            f"caseops_model_kernel_value_{value}",
        ))
    if layer == "math" and kind == "formula":
        features.append(f"caseops_formula_{value}")
    elif layer == "system":
        features.extend((
            "caseops_model_kernel",
            f"caseops_model_kernel_kind_{kind}",
            f"caseops_model_kernel_value_{value}",
        ))
    elif layer == "language":
        features.extend((
            "caseops_language",
            f"caseops_language_kind_{kind}",
            f"caseops_language_value_{value}",
        ))
    elif layer == "mathlang":
        features.extend((
            "caseops_mathlang",
            f"caseops_mathlang_kind_{kind}",
            f"caseops_mathlang_value_{value}",
        ))
        if kind == "formula":
            features.append(f"caseops_formula_{value}")
    elif layer == "glyph":
        features.append(f"caseops_taxonomy_glyph_{value}")

    attributes = row.get("attributes")
    if isinstance(attributes, dict):
        for key, raw_value in attributes.items():
            clean_key = _normalize_feature_piece(str(key))
            clean_attr = _normalize_feature_piece(str(raw_value))
            if not clean_key or not clean_attr:
                continue
            features.append(f"caseops_layered_attr_{clean_key}_{clean_attr}")
            features.append(
                f"caseops_layered_attr_pair_{layer}__{kind}__{value}__{clean_key}__{clean_attr}"
            )
            if clean_key == "family":
                features.append(f"caseops_family_{clean_attr}")

    for link in tuple(row.get("links", ()) or ()):
        if not isinstance(link, dict):
            continue
        source = str(link.get("source") or "")
        target = str(link.get("target") or "")
        relation = _normalize_feature_piece(str(link.get("relation") or ""))
        clean_source = _normalize_feature_piece(source)
        clean_target = _normalize_feature_piece(target)
        if not clean_source or not clean_target:
            continue
        features.append("caseops_layered_link")
        features.append(f"caseops_layered_link_pair_{clean_source}__{clean_target}")
        if relation and relation != "unknown":
            features.append(f"caseops_layered_link_relation_{relation}")
            features.append(
                f"caseops_layered_link_relation_pair_{relation}__{clean_source}__{clean_target}"
            )
        features.extend(_caseops_taxonomy_node_feature_tokens(source))
        features.extend(_caseops_taxonomy_node_feature_tokens(target))
    return tuple(dict.fromkeys(features))


def _parse_caseops_model_train_items(text: str) -> tuple[dict[str, Any], ...]:
    rows: list[dict[str, Any]] = []
    for match in _CASEOPS_MODEL_TRAIN_RE.finditer(str(text or "")):
        targets: list[dict[str, str]] = []
        for target_match in _CASEOPS_MODEL_TRAIN_TARGET_RE.finditer(match.group(4)):
            targets.append({
                "layer": target_match.group(1).lower(),
                "kind": target_match.group(2).lower(),
                "value": target_match.group(3).lower(),
            })
        rows.append({
            "source_layer": match.group(1).lower(),
            "source_kind": match.group(2).lower(),
            "source_value": match.group(3).lower(),
            "targets": tuple(targets),
        })
    return tuple(rows)


def _caseops_model_train_feature_tokens(row: dict[str, Any]) -> tuple[str, ...]:
    source_layer = _normalize_feature_piece(str(row.get("source_layer") or ""))
    source_kind = _normalize_feature_piece(str(row.get("source_kind") or ""))
    source_value = _normalize_feature_piece(str(row.get("source_value") or ""))
    if not source_layer or not source_kind or not source_value:
        return ()
    features: list[str] = [
        "caseops",
        "caseops_model_language",
        "caseops_model_train",
        f"caseops_model_train_source_layer_{source_layer}",
        f"caseops_model_train_source_kind_{source_kind}",
        f"caseops_model_train_source_value_{source_value}",
        f"caseops_model_train_source_{source_layer}__{source_kind}__{source_value}",
        *tuple(_split_value_tokens(_TOKEN_RE.findall(str(row.get("source_value") or "").lower()))),
    ]
    if source_layer in {"system", "math", "language"}:
        features.append(f"caseops_model_train_kernel_source_{source_layer}")
    if source_kind.endswith("codec_gap"):
        features.extend(_caseops_codec_gap_feature_tokens(str(row.get("source_value") or "")))

    for target in tuple(row.get("targets", ()) or ()):
        if not isinstance(target, dict):
            continue
        target_layer = _normalize_feature_piece(str(target.get("layer") or ""))
        target_kind = _normalize_feature_piece(str(target.get("kind") or ""))
        target_value = _normalize_feature_piece(str(target.get("value") or ""))
        if not target_layer or not target_kind or not target_value:
            continue
        features.extend((
            f"caseops_model_train_target_layer_{target_layer}",
            f"caseops_model_train_target_kind_{target_kind}",
            f"caseops_model_train_target_value_{target_value}",
            f"caseops_model_train_target_{target_layer}__{target_kind}__{target_value}",
            f"caseops_model_train_layer_route_{source_layer}__{target_layer}",
            f"caseops_model_train_kind_route_{source_kind}__{target_kind}",
            (
                "caseops_model_train_route_"
                f"{source_layer}__{source_kind}__{source_value}__"
                f"{target_layer}__{target_kind}__{target_value}"
            ),
            *tuple(_split_value_tokens(
                _TOKEN_RE.findall(str(target.get("value") or "").lower())
            )),
        ))
        if target_layer in {"system", "math", "language"}:
            features.append(f"caseops_model_train_kernel_target_{target_layer}")
        if target_layer == "math" or target_kind.endswith("formula"):
            features.append(f"caseops_formula_{target_value}")
        if target_kind.endswith("family"):
            features.append(f"caseops_family_{target_value}")
        if target_layer == "language" or target_kind.startswith(("lang_", "language_")):
            features.extend((
                "caseops_language",
                f"caseops_language_kind_{target_kind}",
                f"caseops_language_value_{target_value}",
            ))
        if target_layer == "mathlang":
            features.append(f"caseops_mathlang_value_{target_value}")
            if target_kind.endswith("alias"):
                features.append(f"caseops_mathlang_alias_{target_value}")
        if target_layer == "crystal":
            features.append(f"caseops_crystal_target_{target_value}")
    return tuple(dict.fromkeys(features))


def _parse_caseops_taxonomy_items(text: str) -> tuple[dict[str, Any], ...]:
    return parse_caseops_taxonomies(text)


def _caseops_taxonomy_feature_tokens(row: dict[str, Any]) -> tuple[str, ...]:
    family_id = _normalize_feature_piece(str(row.get("family_id") or ""))
    if not family_id or family_id == "unknown":
        return ()
    features: list[str] = [
        "caseops",
        "caseops_taxonomy",
        f"caseops_taxonomy_family_{family_id}",
        f"caseops_family_{family_id}",
    ]
    field_map = (
        ("english", "english_structures"),
        ("math", "math_structures"),
        ("glyph", "glyph_attributes"),
        ("related_family", "related_families"),
    )
    values_by_kind: dict[str, list[str]] = {}
    for feature_kind, row_key in field_map:
        clean_values: list[str] = []
        for value in tuple(row.get(row_key, ()) or ()):
            clean_value = _normalize_feature_piece(str(value))
            if not clean_value or clean_value == "unknown":
                continue
            clean_values.append(clean_value)
            features.append(f"caseops_taxonomy_{feature_kind}_{clean_value}")
            features.append(
                f"caseops_taxonomy_{feature_kind}_pair_{family_id}__{clean_value}"
            )
            if feature_kind == "related_family":
                features.append(
                    f"caseops_taxonomy_related_family_pair_{family_id}__{clean_value}"
                )
                features.append(f"caseops_family_{clean_value}")
            features.extend(_split_value_tokens(_TOKEN_RE.findall(str(value).lower())))
        values_by_kind[feature_kind] = clean_values

    for math_value in values_by_kind.get("math", ())[:6]:
        for english_value in values_by_kind.get("english", ())[:6]:
            features.append(
                f"caseops_taxonomy_math_english_pair_{math_value}__{english_value}"
            )
            features.append(
                f"caseops_taxonomy_family_math_english_pair_"
                f"{family_id}__{math_value}__{english_value}"
            )
    for glyph_value in values_by_kind.get("glyph", ())[:6]:
        for math_value in values_by_kind.get("math", ())[:6]:
            features.append(
                f"caseops_taxonomy_glyph_math_pair_{glyph_value}__{math_value}"
            )

    for example in tuple(row.get("examples", ()) or ()):
        clean_example = _normalize_feature_piece(str(example))
        if not clean_example or clean_example == "unknown":
            continue
        features.append(f"caseops_taxonomy_example_{clean_example}")
        features.append(f"caseops_taxonomy_example_pair_{family_id}__{clean_example}")

    for relation in tuple(row.get("relation_types", ()) or ()):
        if not isinstance(relation, dict):
            continue
        relation_id = _normalize_feature_piece(str(relation.get("id") or ""))
        direction = _normalize_feature_piece(str(relation.get("directionality") or ""))
        if not relation_id or relation_id == "unknown":
            continue
        features.append(f"caseops_taxonomy_relation_{relation_id}")
        features.append(f"caseops_taxonomy_relation_pair_{family_id}__{relation_id}")
        if direction and direction != "unknown":
            features.append(
                f"caseops_taxonomy_relation_direction_{relation_id}__{direction}"
            )

    for pathway in tuple(row.get("pathways", ()) or ()):
        if not isinstance(pathway, dict):
            continue
        features.extend(_caseops_taxonomy_pathway_feature_tokens(family_id, pathway))
    return tuple(dict.fromkeys(features))


def _caseops_taxonomy_pathway_feature_tokens(
    family_id: str,
    pathway: dict[str, Any],
) -> tuple[str, ...]:
    source = str(pathway.get("source") or "")
    target = str(pathway.get("target") or "")
    relation = _normalize_feature_piece(str(pathway.get("relation") or ""))
    clean_source = _normalize_feature_piece(source)
    clean_target = _normalize_feature_piece(target)
    if not clean_source or not clean_target:
        return ()
    features: list[str] = [
        "caseops",
        "caseops_taxonomy_pathway",
        f"caseops_taxonomy_pathway_directed_{clean_source}__{clean_target}",
        f"caseops_taxonomy_pathway_pair_{family_id}__{clean_source}__{clean_target}",
    ]
    if relation and relation != "unknown":
        features.append(f"caseops_taxonomy_pathway_relation_{relation}")
        features.append(
            f"caseops_taxonomy_pathway_relation_pair_"
            f"{family_id}__{relation}__{clean_source}__{clean_target}"
        )
    features.extend(_caseops_taxonomy_node_feature_tokens(source))
    features.extend(_caseops_taxonomy_node_feature_tokens(target))
    return tuple(features)


def _caseops_taxonomy_node_feature_tokens(node: str) -> tuple[str, ...]:
    raw = str(node or "").strip()
    clean = _normalize_feature_piece(raw)
    if not clean or clean == "unknown":
        return ()
    features = [
        f"caseops_taxonomy_node_{clean}",
    ]
    head, sep, value = raw.partition(":")
    clean_value = _normalize_feature_piece(value) if sep else ""
    normalized_head = head.strip().lower().replace("language.", "lang.")
    clean_head = _normalize_feature_piece(normalized_head)
    if normalized_head == "formula" and clean_value:
        features.append(f"caseops_formula_{clean_value}")
    elif normalized_head.startswith("lang.") and clean_value:
        features.extend((
            "caseops_language",
            f"caseops_language_kind_{clean_head}",
            f"caseops_language_value_{clean_value}",
            f"caseops_taxonomy_language_node_{clean_head}_{clean_value}",
        ))
        if head.strip().lower().startswith("language."):
            features.append(
                f"caseops_language_kind_{_normalize_feature_piece(head)}"
            )
    elif normalized_head in {"concept", "role", "predicate", "idiom", "quantifier"} and clean_value:
        features.extend((
            "caseops_language",
            f"caseops_language_kind_{clean_head}",
            f"caseops_language_value_{clean_value}",
            f"caseops_taxonomy_language_node_{clean_head}_{clean_value}",
        ))
    elif normalized_head in {"glyph", "glyph_attribute"} and clean_value:
        features.append(f"caseops_taxonomy_glyph_{clean_value}")
    return tuple(features)


def _parse_caseops_stack_code_items(text: str) -> tuple[dict[str, Any], ...]:
    rows: list[dict[str, Any]] = []
    for line_match in _CASEOPS_STACK_CODES_RE.finditer(str(text or "")):
        body = line_match.group(1)
        for item in body.split(";"):
            head = _CASEOPS_STACK_CODE_HEAD_RE.match(item.strip())
            if not head:
                continue
            kind, value, code, rest = head.groups()
            axes = _parse_caseops_stack_axis_codes(
                _caseops_stack_code_section(rest, "axes")
            )
            links = _parse_caseops_stack_axis_codes(
                _caseops_stack_code_section(rest, "links")
            )
            pathways = _parse_caseops_stack_pathway_codes(
                _caseops_stack_code_section(rest, "pathways")
            )
            rows.append({
                "kind": kind.lower(),
                "value": value.lower(),
                "code": code,
                "axes": axes,
                "links": links,
                "pathways": pathways,
            })
    return tuple(rows)


def _caseops_stack_code_section(rest: str, key: str) -> str:
    marker = f" {key}="
    text = " " + str(rest or "").strip()
    start = text.find(marker)
    if start < 0:
        return ""
    value_start = start + len(marker)
    value_end = len(text)
    for next_key in ("axes", "links", "pathways"):
        if next_key == key:
            continue
        next_pos = text.find(f" {next_key}=", value_start)
        if next_pos >= 0:
            value_end = min(value_end, next_pos)
    return text[value_start:value_end].strip()


def _parse_caseops_stack_axis_codes(raw: str) -> tuple[tuple[str, str], ...]:
    rows: list[tuple[str, str]] = []
    for item in str(raw or "").split(","):
        text = item.strip()
        if not text:
            continue
        axis, sep, code = text.rpartition(":")
        if not sep or not axis:
            rows.append((text.lower(), ""))
            continue
        rows.append((axis.lower(), code.strip()))
    return tuple(rows)


def _parse_caseops_stack_pathway_codes(raw: str) -> tuple[tuple[str, str, str], ...]:
    rows: list[tuple[str, str, str]] = []
    for item in str(raw or "").split(","):
        text = item.strip()
        if not text:
            continue
        pathway, sep, code = text.rpartition(":")
        if not sep or ">" not in pathway:
            continue
        left, _, right = pathway.partition(">")
        if left.strip() and right.strip():
            rows.append((left.strip().lower(), right.strip().lower(), code.strip()))
    return tuple(rows)


def _caseops_stack_code_feature_tokens(row: dict[str, Any]) -> tuple[str, ...]:
    clean_kind = _normalize_feature_piece(row.get("kind"))
    clean_value = _normalize_feature_piece(row.get("value"))
    clean_code = _normalize_feature_piece(row.get("code"))
    if not clean_kind or not clean_value:
        return ()
    features = [
        "caseops",
        "caseops_stack_code",
        f"caseops_stack_kind_{clean_kind}",
        f"caseops_stack_value_{clean_value}",
        f"caseops_stack_pair_{clean_kind}__{clean_value}",
        f"caseops_vocab_pair_{clean_kind}__{clean_value}",
        *tuple(_split_value_tokens(_TOKEN_RE.findall(str(row.get("value") or "").lower()))),
    ]
    if clean_code and clean_code != "-":
        features.append(f"caseops_stack_vocab_code_{clean_code}")
        features.append(
            f"caseops_stack_vocab_code_pair_{clean_kind}__{clean_value}__{clean_code}"
        )
    for axis, axis_code in tuple(row.get("axes", ()) or ()):
        clean_axis = _normalize_feature_piece(axis)
        clean_axis_code = _normalize_feature_piece(axis_code)
        if not clean_axis:
            continue
        features.append(f"caseops_stack_axis_{clean_axis}")
        features.append(f"caseops_vocab_axis_{clean_axis}")
        features.append(
            f"caseops_stack_axis_pair_{clean_kind}__{clean_value}__{clean_axis}"
        )
        if clean_axis_code and clean_axis_code != "-":
            features.append(f"caseops_stack_axis_code_{clean_axis_code}")
            features.append(
                f"caseops_stack_axis_code_pair_{clean_axis}__{clean_axis_code}"
            )
    for axis, link_code in tuple(row.get("links", ()) or ()):
        clean_axis = _normalize_feature_piece(axis)
        clean_link_code = _normalize_feature_piece(link_code)
        if not clean_axis:
            continue
        features.append(
            f"caseops_stack_link_pair_{clean_kind}__{clean_value}__{clean_axis}"
        )
        if clean_link_code and clean_link_code != "-":
            features.append(f"caseops_stack_link_code_{clean_link_code}")
            features.append(
                f"caseops_stack_link_code_pair_{clean_axis}__{clean_link_code}"
            )
    for left, right, pathway_code in tuple(row.get("pathways", ()) or ()):
        clean_left = _normalize_feature_piece(left)
        clean_right = _normalize_feature_piece(right)
        clean_pathway_code = _normalize_feature_piece(pathway_code)
        if not clean_left or not clean_right:
            continue
        ordered = "__".join(sorted((clean_left, clean_right)))
        features.append(f"caseops_stack_axis_pathway_{ordered}")
        features.append(f"caseops_vocab_axis_pathway_{ordered}")
        if clean_pathway_code and clean_pathway_code != "-":
            features.append(f"caseops_stack_pathway_code_{clean_pathway_code}")
            features.append(
                f"caseops_stack_pathway_code_pair_{ordered}__{clean_pathway_code}"
            )
    return tuple(features)


def _parse_knowledge_card_items(text: str) -> tuple[dict[str, Any], ...]:
    raw = str(text or "")
    matches = list(_KNOWLEDGE_CARD_RE.finditer(raw))
    rows: list[dict[str, Any]] = []
    for index, match in enumerate(matches):
        start = match.start()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(raw)
        block = raw[start:end]
        card_id = match.group(1).strip()
        kind = match.group(2).strip()
        fields = _knowledge_card_fields(block)
        rows.append({
            "id": card_id,
            "kind": kind,
            "text": block.strip(),
            "slop_hash": fields.get("slop_hash", ""),
            "stack_codes": _knowledge_card_stack_codes(fields.get("stack_codes", "")),
            "concepts": _knowledge_card_section_ticks(block, "Linguistic concepts"),
            "formulas": _knowledge_card_section_ticks(block, "Matched formulas"),
            "suggestions": _knowledge_card_section_ticks(block, "Top suggestions"),
            "neighbors": _knowledge_card_section_ticks(block, "Glyph-space neighbors"),
            "tags": _knowledge_card_section_tags(block, "Auto-tags"),
        })
    return tuple(rows)


def _parse_caseops_crystal_primer_items(text: str) -> tuple[dict[str, Any], ...]:
    """Decode compact crystal-primer provenance back into card-like rows."""
    rows: list[dict[str, Any]] = []
    for row in parse_caseops_crystal_primers(text):
        rows.append({
            "id": row.get("card_id", ""),
            "kind": row.get("kind", "card"),
            "text": "",
            "slop_hash": "",
            "stack_codes": caseops_crystal_stack_pairs(row.get("stacks", ())),
            "concepts": tuple(row.get("concepts", ()) or ()),
            "formulas": tuple(row.get("formulas", ()) or ()),
            "suggestions": (),
            "neighbors": (),
            "tags": (),
        })
    return tuple(rows)


def extract_caseops_knowledge_cards(text: str) -> tuple[dict[str, Any], ...]:
    """Extract mathlang crystallize KnowledgeCards from markdown text."""
    return tuple(dict(row) for row in _parse_knowledge_card_items(text))


def ingest_caseops_knowledge_cards(
    learner: "MicroLearner",
    text: str,
    *,
    source: str = "",
    reward: float = 0.5,
    tags: Iterable[str] = ("math-language", "caseops", "crystallize"),
) -> dict[str, Any]:
    """Observe mathlang crystallize KnowledgeCards into the learner."""
    cards = extract_caseops_knowledge_cards(text)
    tag_tuple = tuple(dict.fromkeys(str(tag) for tag in tags if str(tag)))
    observed = 0
    for row in cards:
        metadata = {
            "source": str(source or ""),
            "card_id": str(row.get("id") or ""),
            "kind": str(row.get("kind") or ""),
            "slop_hash": str(row.get("slop_hash") or ""),
        }
        learner.observe(
            RolloutExample(
                prompt=(
                    "caseops crystallized KnowledgeCard"
                    f" kind={metadata['kind']} card_id={metadata['card_id']}"
                    f" source={metadata['source'] or '-'}"
                ),
                response=str(row.get("text") or ""),
                metadata=metadata,
            ),
            Feedback(
                reward=float(reward),
                notes="ingested mathlang crystallize KnowledgeCard",
                tags=tag_tuple,
            ),
        )
        observed += 1
    return {
        "source": str(source or ""),
        "cards": len(cards),
        "observed": observed,
        "reward": float(reward),
        "card_ids": tuple(str(row.get("id") or "") for row in cards),
        "formulas": sum(len(row.get("formulas", ()) or ()) for row in cards),
        "concepts": sum(len(row.get("concepts", ()) or ()) for row in cards),
        "stack_codes": sum(len(row.get("stack_codes", ()) or ()) for row in cards),
    }


def _knowledge_card_fields(block: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for match in _KNOWLEDGE_CARD_FIELD_RE.finditer(block):
        key = match.group(1).lower().replace("-", "_").replace(" ", "_")
        value = match.group(2).strip()
        if key == "slop_ir_hash":
            tick = _KNOWLEDGE_CARD_TICK_RE.search(value)
            out["slop_hash"] = tick.group(1) if tick else value
        elif key == "stack_codes":
            out["stack_codes"] = value
    return out


def _knowledge_card_section(block: str, section: str) -> str:
    section_lower = section.lower()
    matches = list(_KNOWLEDGE_CARD_SECTION_RE.finditer(block))
    for index, match in enumerate(matches):
        if not match.group(1).strip().lower().startswith(section_lower):
            continue
        start = match.end()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(block)
        return block[start:end]
    return ""


def _knowledge_card_section_ticks(block: str, section: str) -> tuple[str, ...]:
    body = _knowledge_card_section(block, section)
    return tuple(dict.fromkeys(
        value.strip()
        for value in _KNOWLEDGE_CARD_TICK_RE.findall(body)
        if value.strip()
    ))


def _knowledge_card_section_tags(block: str, section: str) -> tuple[tuple[str, str], ...]:
    body = _knowledge_card_section(block, section)
    return tuple(dict.fromkeys(
        (left.strip(), right.strip())
        for left, right in _KNOWLEDGE_CARD_TAG_RE.findall(body)
        if left.strip() and right.strip()
    ))


def _knowledge_card_stack_codes(raw: str) -> tuple[tuple[str, str], ...]:
    value = str(raw or "").replace("`", "")
    rows: list[tuple[str, str]] = []
    for match in re.finditer(r"([a-z0-9_ -]+)=([0-9,\s]+)(?=,\s*[a-z0-9_ -]+=|$)", value, re.IGNORECASE):
        layer = match.group(1).strip()
        codes = match.group(2)
        if not layer:
            continue
        for code in str(codes or "").split(","):
            if code.strip():
                rows.append((layer, code.strip()))
    return tuple(rows)


def _knowledge_card_feature_tokens(row: dict[str, Any]) -> tuple[str, ...]:
    card_id = _normalize_feature_piece(row.get("id"))
    kind = _normalize_feature_piece(row.get("kind"))
    if not card_id:
        return ()
    features = [
        "caseops",
        "caseops_crystal_card",
        f"caseops_crystal_id_{card_id}",
        f"caseops_crystal_kind_{kind}",
        f"caseops_crystal_pair_{kind}__{card_id}",
        *tuple(_split_value_tokens(_TOKEN_RE.findall(str(row.get("id") or "").lower()))),
    ]
    slop_hash = _normalize_feature_piece(row.get("slop_hash"))
    if slop_hash and slop_hash != "unknown":
        features.append(f"caseops_crystal_slop_hash_{slop_hash}")
        features.append(f"caseops_crystal_hash_pair_{card_id}__{slop_hash}")
    for formula in tuple(row.get("formulas", ()) or ()):
        clean_formula = _normalize_feature_piece(formula)
        if clean_formula:
            features.append(f"caseops_formula_{clean_formula}")
            features.append(f"caseops_crystal_formula_pair_{card_id}__{clean_formula}")
    for formula in tuple(row.get("suggestions", ()) or ()):
        clean_formula = _normalize_feature_piece(formula)
        if clean_formula:
            features.append(f"caseops_crystal_suggestion_pair_{card_id}__{clean_formula}")
    for formula in tuple(row.get("neighbors", ()) or ()):
        clean_formula = _normalize_feature_piece(formula)
        if clean_formula:
            features.append(f"caseops_crystal_neighbor_pair_{card_id}__{clean_formula}")
    for concept in tuple(row.get("concepts", ()) or ()):
        clean_concept = _normalize_feature_piece(concept)
        if clean_concept:
            features.append(f"caseops_crystal_concept_{clean_concept}")
            features.append(f"caseops_crystal_concept_pair_{card_id}__{clean_concept}")
    for dim, value in tuple(row.get("tags", ()) or ()):
        clean_dim = _normalize_feature_piece(dim)
        clean_value = _normalize_feature_piece(value)
        if clean_dim and clean_value:
            features.append(f"caseops_crystal_tag_{clean_dim}_{clean_value}")
            features.append(f"caseops_crystal_tag_pair_{card_id}__{clean_dim}__{clean_value}")
    for layer, code in tuple(row.get("stack_codes", ()) or ()):
        clean_layer = _normalize_feature_piece(layer)
        clean_code = _normalize_feature_piece(code)
        if clean_layer and clean_code:
            features.append(f"caseops_crystal_stack_code_{clean_layer}_{clean_code}")
            features.append(f"caseops_crystal_stack_pair_{card_id}__{clean_layer}__{clean_code}")
    return tuple(features)


def _caseops_linguistic_link_feature_tokens(
    kind: str,
    value: str,
    detail: str | None = None,
) -> tuple[str, ...]:
    clean_kind = _normalize_feature_piece(kind)
    clean_value = _normalize_feature_piece(value)
    clean_detail = _normalize_feature_piece(detail) if detail else ""
    value_tokens = _TOKEN_RE.findall(" ".join((
        str(kind or ""),
        str(value or ""),
        str(detail or ""),
    )).lower())
    if not clean_kind or not clean_value:
        return ()
    pieces = (clean_kind, clean_value, clean_detail) if clean_detail else (clean_kind, clean_value)
    return (
        "caseops",
        "caseops_linguistic_link",
        f"caseops_linguistic_kind_{clean_kind}",
        f"caseops_linguistic_value_{clean_value}",
        *((
            f"caseops_linguistic_value_{clean_detail}",
        ) if clean_detail else ()),
        f"caseops_linguistic_pair_{'__'.join(pieces)}",
        *tuple(_split_value_tokens(value_tokens)),
    )


def _caseops_math_language_feature_tokens(
    formula_id: str,
    language_kind: str,
    language_value: str,
) -> tuple[str, ...]:
    clean_formula = _normalize_feature_piece(formula_id)
    clean_kind = _normalize_feature_piece(language_kind)
    value_tokens = _TOKEN_RE.findall(str(language_value or "").lower())
    clean_value = "_".join(value_tokens[:8]) if value_tokens else _normalize_feature_piece(language_value)
    if not clean_formula or not clean_kind or not clean_value:
        return ()
    return (
        "caseops",
        "caseops_math_language_pathway",
        f"caseops_formula_{clean_formula}",
        "caseops_language",
        f"caseops_language_kind_{clean_kind}",
        f"caseops_language_value_{clean_value}",
        *tuple(_split_value_tokens(value_tokens)),
        f"caseops_math_language_pair_{clean_formula}__{clean_kind}_{clean_value}",
    )


def _caseops_activation_route_feature_tokens(route: str) -> tuple[str, ...]:
    nodes = [
        _normalize_feature_piece(node)
        for node in str(route or "").split("=>")
        if str(node or "").strip()
    ]
    if len(nodes) < 2:
        return ()
    features: list[str] = [
        "caseops",
        "caseops_activation_route",
        f"caseops_activation_route_pair_{nodes[0]}__{nodes[-1]}",
    ]
    for node in nodes:
        if node.startswith("formula_"):
            formula_id = node.removeprefix("formula_")
            features.append(f"caseops_formula_{formula_id}")
            features.extend(_split_value_tokens([formula_id]))
        elif node.startswith("language_"):
            language_value = node.removeprefix("language_")
            features.append("caseops_language")
            features.append(f"caseops_language_value_{language_value}")
            if language_value.startswith("lang_"):
                parts = [part for part in language_value.split("_") if part]
                if len(parts) >= 3:
                    features.append(f"caseops_language_kind_{parts[0]}_{parts[1]}")
                    features.append(f"caseops_language_value_{'_'.join(parts[2:])}")
            features.extend(_split_value_tokens([language_value]))
    for left, right in zip(nodes, nodes[1:]):
        ordered = "__".join(sorted((left, right)))
        features.append(f"caseops_activation_route_step_{left}__{right}")
        if left.startswith("formula_") and right.startswith("formula_"):
            features.append(
                "caseops_pathway_pair_"
                + "__".join(sorted((
                    left.removeprefix("formula_"),
                    right.removeprefix("formula_"),
                )))
            )
        if left.startswith("formula_") and right.startswith("language_"):
            features.append(_caseops_route_math_language_pair(left, right))
        elif left.startswith("language_") and right.startswith("formula_"):
            features.append(_caseops_route_math_language_pair(right, left))
        features.append(f"caseops_activation_route_step_pair_{ordered}")
    return tuple(feature for feature in features if feature)


def _caseops_route_math_language_pair(formula_node: str, language_node: str) -> str:
    formula_id = formula_node.removeprefix("formula_")
    language_value = language_node.removeprefix("language_")
    if language_value.startswith("lang_"):
        return f"caseops_math_language_pair_{formula_id}__{language_value}"
    return f"caseops_math_language_pair_{formula_id}__language_{language_value}"


def parse_caseops_activation_routes(text: str) -> tuple[dict[str, Any], ...]:
    """Decode persisted CaseOps activation-route text into structured rows."""
    rows: list[dict[str, Any]] = []
    for match in _CASEOPS_ACTIVATION_ROUTE_RE.finditer(str(text or "")):
        encoded = match.group(1)
        encoded_nodes = tuple(
            _normalize_feature_piece(node)
            for node in encoded.split("=>")
            if str(node or "").strip()
        )
        nodes = tuple(
            _decode_caseops_activation_route_node(node)
            for node in encoded_nodes
        )
        nodes = tuple(node for node in nodes if node)
        if len(nodes) < 2:
            continue
        relations = tuple(
            _caseops_activation_route_relation(left, right)
            for left, right in zip(nodes, nodes[1:])
        )
        try:
            contribution = float(match.group(2) or 0.0)
        except Exception:  # noqa: BLE001
            contribution = 0.0
        rows.append({
            "source": nodes[0],
            "target": nodes[-1],
            "nodes": nodes,
            "encoded_nodes": encoded_nodes,
            "relations": relations,
            "hops": max(0, len(nodes) - 1),
            "contribution": contribution,
            "encoded": encoded,
        })
    return tuple(rows)


def _caseops_activation_route_rows(
    unigrams: dict[str, float],
    prefix: str,
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    tokens = {
        token for token in unigrams
        if token.startswith(prefix)
    }
    if include_penalties and penalties:
        tokens.update(
            token for token in penalties
            if token.startswith(prefix)
        )
    for token in tokens:
        if not token.startswith(prefix):
            continue
        if token.startswith("caseops_activation_route_step_pair_"):
            continue
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        effective_weight = weight - penalty
        value = token.removeprefix(prefix)
        encoded_nodes = tuple(
            _normalize_feature_piece(node)
            for node in value.split("__")
            if str(node or "").strip()
        )
        nodes = tuple(
            _decode_caseops_activation_route_node(node)
            for node in encoded_nodes
        )
        nodes = tuple(node for node in nodes if node)
        if len(nodes) < 2:
            continue
        relations = tuple(
            _caseops_activation_route_relation(left, right)
            for left, right in zip(nodes, nodes[1:])
        )
        rows.append({
            "value": value,
            "token": token,
            "weight": weight,
            "penalty": penalty,
            "effective_weight": effective_weight,
            "source": nodes[0],
            "target": nodes[-1],
            "nodes": nodes,
            "encoded_nodes": encoded_nodes,
            "relations": relations,
            "hops": max(0, len(nodes) - 1),
            "contribution": effective_weight,
            "encoded": "=>".join(encoded_nodes),
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["value"]),
    ))
    return rows[:top_n]


def _decode_caseops_activation_route_node(node: str) -> str:
    clean = _normalize_feature_piece(node)
    if clean.startswith("formula_"):
        formula_id = clean.removeprefix("formula_")
        return f"formula:{formula_id}" if formula_id else ""
    if not clean.startswith("language_"):
        return ""
    value = clean.removeprefix("language_")
    pieces = [piece for piece in value.split("_") if piece]
    if len(pieces) >= 3 and pieces[0] == "lang":
        return f"language:lang.{pieces[1]}:{'_'.join(pieces[2:])}"
    return f"language:{value}" if value else ""


def _caseops_activation_route_relation(left: str, right: str) -> str:
    left_kind = left.partition(":")[0]
    right_kind = right.partition(":")[0]
    if left_kind == "formula" and right_kind == "formula":
        return "caseops_formula_synapse"
    if {left_kind, right_kind} == {"formula", "language"}:
        return "caseops_math_language_synapse"
    return "caseops_activation_route_step"


def _normalize_feature_piece(value: str) -> str:
    piece = re.sub(r"[^a-z0-9_]+", "_", str(value).lower()).strip("_")
    return piece or "unknown"


def _split_value_tokens(tokens: list[str]) -> list[str]:
    out: list[str] = []
    for token in tokens:
        out.append(token)
        if "_" in token:
            out.extend(part for part in token.split("_") if part)
    return out


def _decode_caseops_payload(value: str) -> str:
    try:
        padded = value + ("=" * (-len(value) % 4))
        return base64.urlsafe_b64decode(padded.encode("ascii")).decode("utf-8")
    except Exception:  # noqa: BLE001
        return ""


def _default_model_language_state() -> dict[str, Any]:
    layers = tuple(dict(row) for row in _DEFAULT_MODEL_LANGUAGE_LAYERS)
    return _normalize_model_language_state({
        "version": _MODEL_LANGUAGE_VERSION,
        "model_language": "caseops_model_language/1",
        "layers": layers,
    })


def _normalize_model_language_state(raw: object) -> dict[str, Any]:
    source = raw if isinstance(raw, dict) else {}
    layer_rows = source.get("layers") if isinstance(source, dict) else ()
    if not isinstance(layer_rows, (list, tuple)) or not layer_rows:
        layer_rows = _DEFAULT_MODEL_LANGUAGE_LAYERS
    by_layer: dict[str, dict[str, Any]] = {}
    for row in layer_rows:
        if not isinstance(row, dict):
            continue
        layer = _normalize_model_layer(str(row.get("layer") or ""))
        if not layer:
            continue
        by_layer[layer] = _normalize_model_layer_row(row, layer=layer)
    for row in _DEFAULT_MODEL_LANGUAGE_LAYERS:
        layer = str(row["layer"])
        by_layer.setdefault(layer, _normalize_model_layer_row(row, layer=layer))
    layers = tuple(
        by_layer[layer]
        for layer in sorted(
            by_layer,
            key=lambda name: (
                int(by_layer[name].get("depth") or 0),
                0 if bool(by_layer[name].get("kernel")) else 1,
                name,
            ),
        )
    )
    kernel_layers = tuple(
        row["layer"]
        for row in layers
        if bool(row.get("kernel"))
    )
    trainable_layers = tuple(
        row["layer"]
        for row in layers
        if bool(row.get("train_when_oversized"))
    )
    return {
        "version": int(source.get("version") or _MODEL_LANGUAGE_VERSION)
        if isinstance(source, dict)
        else _MODEL_LANGUAGE_VERSION,
        "model_language": str(
            source.get("model_language") or "caseops_model_language/1"
        ) if isinstance(source, dict) else "caseops_model_language/1",
        "layers": layers,
        "kernel_layers": kernel_layers,
        "trainable_layers": trainable_layers,
        "protected_prefixes": _model_language_protected_prefixes(kernel_layers),
    }


def _normalize_model_layer_row(row: dict[str, Any], *, layer: str) -> dict[str, Any]:
    consolidates_to = row.get("consolidates_to", ()) or ()
    if isinstance(consolidates_to, str):
        consolidates_to = consolidates_to.split(",")
    return {
        "layer": layer,
        "depth": int(row.get("depth") or 0),
        "kernel": bool(row.get("kernel")),
        "stability": str(row.get("stability") or "unknown"),
        "max_tokens": int(row.get("max_tokens") or 0),
        "train_when_oversized": bool(row.get("train_when_oversized")),
        "consolidates_to": tuple(
            _normalize_model_layer(str(item))
            for item in consolidates_to
            if _normalize_model_layer(str(item))
        ),
    }


def _normalize_model_layer(value: str) -> str:
    return re.sub(r"[^a-z0-9_*:-]+", "_", str(value).lower()).strip("_")


def _model_language_protected_prefixes(kernel_layers: Iterable[str]) -> tuple[str, ...]:
    prefixes = [
        "caseops_model_kernel",
        "caseops_model_train_kernel_",
    ]
    for layer in kernel_layers:
        clean_layer = _normalize_feature_piece(str(layer))
        if not clean_layer or clean_layer == "unknown":
            continue
        prefixes.extend((
            f"caseops_layer_{clean_layer}",
            f"caseops_layered_layer_{clean_layer}",
            f"caseops_model_kernel_layer_{clean_layer}",
            f"caseops_model_train_kernel_source_{clean_layer}",
            f"caseops_model_train_kernel_target_{clean_layer}",
        ))
    return tuple(dict.fromkeys(prefixes))


def _is_protected_feature_token(token: str, prefixes: Iterable[str]) -> bool:
    tok = str(token or "")
    return any(tok == prefix or tok.startswith(prefix) for prefix in prefixes)


_KERNEL_ROUTE_TOKEN_PREFIXES: tuple[tuple[str, str], ...] = (
    ("caseops_model_train_layer_route_", "model_train_layer_route"),
    ("caseops_model_train_route_", "model_train_route"),
    ("caseops_taxonomy_math_english_pair_", "taxonomy_math_language_pair"),
    ("caseops_taxonomy_glyph_math_pair_", "taxonomy_glyph_math_pair"),
    ("caseops_taxonomy_pathway_", "taxonomy_pathway"),
    ("caseops_taxonomy_family_", "taxonomy_family"),
    ("caseops_activation_route_pair_", "activation_route_pair"),
    ("caseops_formula_", "formula"),
    ("caseops_language_value_", "language_value"),
    ("caseops_mathlang_value_", "mathlang_value"),
)


def _caseops_kernel_route_prior(
    prompt: str,
    candidate: str,
    *,
    unigrams: dict[str, float],
    penalties: dict[str, float],
    model_language: dict[str, Any],
    top_n: int = 6,
) -> dict[str, Any]:
    """Score learned CaseOps kernel/route evidence as a bounded prior."""
    protected_prefixes = tuple(model_language.get("protected_prefixes", ()) or ())
    prompt_tokens = set(_tokens(prompt))
    evidence: list[dict[str, Any]] = []
    raw_score = 0.0
    for token in tuple(dict.fromkeys(_tokens(candidate))):
        kind = _kernel_route_token_kind(token, protected_prefixes)
        if not kind:
            continue
        learned = float(unigrams.get(token, 0.0) or 0.0)
        penalty = float(penalties.get(token, 0.0) or 0.0)
        if learned <= 0.0 and penalty <= 0.0:
            continue
        contribution = math.log1p(max(0.0, learned)) - penalty
        if token in prompt_tokens and learned > 0.0:
            contribution += min(0.15, math.log1p(learned) * 0.1)
        raw_score += contribution
        evidence.append({
            "token": token,
            "kind": kind,
            "weight": learned,
            "penalty": penalty,
            "protected": _is_protected_feature_token(token, protected_prefixes),
            "contribution": contribution,
        })
    if not evidence:
        return {"score": 0.0, "evidence": ()}
    evidence.sort(
        key=lambda row: (
            _kernel_route_evidence_priority(str(row.get("kind") or "")),
            -abs(float(row.get("contribution") or 0.0)),
            str(row.get("token") or ""),
        )
    )
    normalizer = max(1.0, math.sqrt(len(evidence)))
    return {
        "score": math.tanh(raw_score / normalizer),
        "evidence": tuple(evidence[: max(1, int(top_n))]),
    }


def _kernel_route_token_kind(
    token: str,
    protected_prefixes: Iterable[str],
) -> str:
    if _is_protected_feature_token(token, protected_prefixes):
        return "protected_kernel"
    for prefix, kind in _KERNEL_ROUTE_TOKEN_PREFIXES:
        if token.startswith(prefix):
            return kind
    return ""


def _kernel_route_evidence_priority(kind: str) -> int:
    if kind in {"model_train_route", "model_train_layer_route"}:
        return 0
    if kind.endswith("_pair") or kind == "taxonomy_pathway":
        return 1
    if kind in {"taxonomy_family", "formula", "language_value", "mathlang_value"}:
        return 2
    return 3


def _model_language_pressure(
    model_language: dict[str, Any],
    unigrams: dict[str, float],
    snippets: Iterable[dict[str, Any]],
    *,
    top_n: int,
) -> dict[str, Any]:
    layers = tuple(
        row for row in model_language.get("layers", ()) or ()
        if isinstance(row, dict)
    )
    fast_rows = tuple(snippets or ())
    layer_rows: list[dict[str, Any]] = []
    for row in layers:
        layer = str(row.get("layer") or "")
        if not layer:
            continue
        token_count, token_weight = _model_language_layer_slow_pressure(
            layer,
            unigrams,
        )
        fast_hits = _model_language_layer_fast_hits(layer, fast_rows)
        pressure = int(token_count) + int(fast_hits)
        max_tokens = int(row.get("max_tokens") or 0)
        ratio = (
            float(pressure) / float(max_tokens)
            if max_tokens > 0
            else 0.0
        )
        trainable = bool(row.get("train_when_oversized"))
        kernel = bool(row.get("kernel"))
        oversized = bool(max_tokens > 0 and pressure > max_tokens)
        if oversized and trainable:
            action = "consolidate_fast_layer"
        elif oversized and kernel:
            action = "manual_kernel_review"
        elif trainable:
            action = "monitor_trainable_layer"
        elif kernel:
            action = "locked_kernel"
        else:
            action = "monitor"
        layer_rows.append({
            "layer": layer,
            "depth": int(row.get("depth") or 0),
            "kernel": kernel,
            "trainable": trainable,
            "max_tokens": max_tokens,
            "slow_tokens": int(token_count),
            "slow_weight": float(token_weight),
            "fast_hits": int(fast_hits),
            "pressure": pressure,
            "pressure_ratio": ratio,
            "oversized": oversized,
            "action": action,
            "consolidates_to": tuple(row.get("consolidates_to", ()) or ()),
        })
    layer_rows.sort(key=lambda item: (
        -bool(item.get("oversized")),
        -float(item.get("pressure_ratio") or 0.0),
        int(item.get("depth") or 0),
        str(item.get("layer") or ""),
    ))
    oversized_layers = tuple(
        str(row["layer"])
        for row in layer_rows
        if bool(row.get("oversized"))
    )
    trainable_pressure = sum(
        int(row.get("pressure") or 0)
        for row in layer_rows
        if bool(row.get("trainable"))
    )
    kernel_pressure = sum(
        int(row.get("pressure") or 0)
        for row in layer_rows
        if bool(row.get("kernel"))
    )
    return {
        "layers": tuple(layer_rows[: max(1, int(top_n))]),
        "oversized_layers": oversized_layers,
        "trainable_pressure": trainable_pressure,
        "kernel_pressure": kernel_pressure,
        "total_pressure": sum(int(row.get("pressure") or 0) for row in layer_rows),
    }


def _model_language_layer_slow_pressure(
    layer: str,
    unigrams: dict[str, float],
) -> tuple[int, float]:
    normalized = _normalize_feature_piece(layer.replace("*", "star"))
    if not normalized or normalized == "unknown":
        return 0, 0.0
    prefixes = (
        f"caseops_layer_{normalized}",
        f"caseops_layered_layer_{normalized}",
        f"caseops_model_kernel_layer_{normalized}",
        f"caseops_model_train_source_layer_{normalized}",
        f"caseops_model_train_target_layer_{normalized}",
        f"caseops_model_train_layer_route_{normalized}__",
    )
    count = 0
    weight = 0.0
    for token, value in unigrams.items():
        if any(token == prefix or token.startswith(prefix) for prefix in prefixes):
            count += 1
            weight += float(value)
            continue
        if f"__{normalized}" in token and token.startswith("caseops_model_train_layer_route_"):
            count += 1
            weight += float(value)
    return count, weight


def _model_language_layer_fast_hits(
    layer: str,
    snippets: Iterable[dict[str, Any]],
) -> int:
    normalized = _normalize_feature_piece(layer.replace("*", "star"))
    if not normalized or normalized == "unknown":
        return 0
    markers = (
        f"caseops_{normalized}_vocab",
        f"caseops_layer_{normalized}",
        f"caseops_model_kernel_layer_{normalized}",
        f"caseops_model_train_source_layer_{normalized}",
        f"caseops_model_train_target_layer_{normalized}",
        f"caseops_model_train_layer_route_{normalized}__",
        f"__{normalized}",
        f"SLOP1~{layer.replace('*', '')}",
    )
    hits = 0
    for item in snippets:
        text = str(item.get("text") or "")
        if any(marker and marker in text for marker in markers):
            hits += 1
    return hits


def _clamp(value: float, low: float, high: float) -> float:
    return max(low, min(high, value))


def _newer_caseops_replay_text(current: str, incoming: str) -> bool:
    """Prefer newer CaseOps replay schemas when similar fast snippets merge."""
    current_versions = [
        int(match.group(1))
        for match in _CASEOPS_REPLAY_SCHEMA_RE.finditer(str(current or ""))
    ]
    incoming_versions = [
        int(match.group(1))
        for match in _CASEOPS_REPLAY_SCHEMA_RE.finditer(str(incoming or ""))
    ]
    if not incoming_versions:
        return False
    current_version = max(current_versions) if current_versions else -1
    return max(incoming_versions) > current_version


@dataclass(frozen=True)
class RolloutExample:
    """One observed attempt by the system."""

    prompt: str
    response: str
    metadata: dict[str, str] = field(default_factory=dict)

    def text(self) -> str:
        return f"{self.prompt}\n{self.response}".strip()


@dataclass(frozen=True)
class Feedback:
    """Rich rollout feedback used by both fast and slow learners.

    Reward is expected in [-1, 1]. Positive reward strengthens both the
    textual memory and token statistics; negative reward records cautionary
    memory and mildly penalizes response tokens.
    """

    reward: float
    notes: str = ""
    tags: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if not -1.0 <= self.reward <= 1.0:
            raise ValueError("feedback reward must be in [-1.0, 1.0]")


@dataclass(frozen=True)
class RankedCandidate:
    """A candidate response with component scores."""

    text: str
    score: float
    slow_score: float
    fast_score: float
    kernel_score: float = 0.0
    mlp_score: float = 0.0
    kernel_evidence: tuple[dict[str, Any], ...] = ()
    matched_context: tuple[str, ...] = ()


@dataclass
class _ContextSnippet:
    text: str
    score: float
    uses: int = 0
    tags: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "text": self.text,
            "score": self.score,
            "uses": self.uses,
            "tags": list(self.tags),
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "_ContextSnippet":
        return cls(
            text=str(data["text"]),
            score=float(data["score"]),
            uses=int(data.get("uses", 0)),
            tags=tuple(str(t) for t in data.get("tags", [])),
        )


class FastContextPool:
    """Bounded textual fast weights updated directly from feedback."""

    def __init__(
        self,
        capacity: int = 32,
        *,
        snippet_chars: int = 320,
        merge_threshold: float = 0.7,
    ) -> None:
        if capacity < 1:
            raise ValueError("capacity must be positive")
        if snippet_chars < 40:
            raise ValueError("snippet_chars must be at least 40")
        self.capacity = capacity
        self.snippet_chars = snippet_chars
        self.merge_threshold = merge_threshold
        self._snippets: list[_ContextSnippet] = []

    @property
    def snippets(self) -> tuple[str, ...]:
        return tuple(item.text for item in self._snippets)

    def observe(self, example: RolloutExample, feedback: Feedback) -> None:
        """Add or update a text memory from one rollout."""

        strength = abs(feedback.reward)
        if strength == 0.0 and not feedback.notes:
            return
        marker = "prefer" if feedback.reward >= 0.0 else "avoid"
        text = self._make_snippet(marker, example, feedback)
        delta = feedback.reward if feedback.reward != 0.0 else 0.1

        best_i = None
        best_overlap = 0.0
        new_tokens = set(_tokens(text))
        for i, item in enumerate(self._snippets):
            overlap = _jaccard(new_tokens, set(_tokens(item.text)))
            if overlap > best_overlap:
                best_i = i
                best_overlap = overlap

        if best_i is not None and best_overlap >= self.merge_threshold:
            item = self._snippets[best_i]
            item.score = _clamp(item.score + delta, -3.0, 3.0)
            item.uses += 1
            item.tags = tuple(sorted(set(item.tags).union(feedback.tags)))
            if feedback.reward >= 0.0 and (
                len(text) > len(item.text)
                or _newer_caseops_replay_text(item.text, text)
            ):
                item.text = text
        else:
            self._snippets.append(
                _ContextSnippet(
                    text=text,
                    score=_clamp(delta, -3.0, 3.0),
                    uses=1,
                    tags=tuple(feedback.tags),
                )
            )
        self._prune()

    def retrieve(
        self, query: str, *, limit: int = 5, tags: Iterable[str] = ()
    ) -> list[str]:
        """Return high-scoring context snippets relevant to a query."""

        if limit < 1:
            return []
        wanted_tags = set(tags)
        query_tokens = set(_tokens(query))
        ranked: list[tuple[float, str]] = []
        for item in self._snippets:
            if wanted_tags and not wanted_tags.intersection(item.tags):
                continue
            relevance = _jaccard(query_tokens, set(_tokens(item.text)))
            value = item.score + relevance + math.log1p(item.uses) * 0.05
            ranked.append((value, item.text))
        ranked.sort(key=lambda pair: (-pair[0], pair[1]))
        return [text for _, text in ranked[:limit]]

    def score_text(self, query: str, candidate: str) -> tuple[float, tuple[str, ...]]:
        """Score a candidate by overlap with retrieved positive/negative memory."""

        text_tokens = set(_tokens(f"{query} {candidate}"))
        if not text_tokens:
            return 0.0, ()
        total = 0.0
        matched: list[str] = []
        for item in self._snippets:
            overlap = _jaccard(text_tokens, set(_tokens(item.text)))
            if overlap <= 0.0:
                continue
            total += item.score * overlap
            if item.score > 0.0:
                matched.append(item.text)
        matched.sort()
        return total, tuple(matched[:3])

    def consolidation_candidates(self, *, min_score: float = 0.5) -> tuple[str, ...]:
        """Positive snippets strong enough to promote into slow weights."""
        return tuple(
            item.text for item in self._snippets
            if item.score >= float(min_score)
        )

    def decay(
        self,
        *,
        factor: float = 0.85,
        min_abs_score: float = 0.05,
    ) -> dict[str, int]:
        """Decay fast-context scores and drop near-zero snippets."""
        if not 0.0 <= factor <= 1.0:
            raise ValueError("factor must be in [0.0, 1.0]")
        if min_abs_score < 0.0:
            raise ValueError("min_abs_score must be non-negative")
        before = len(self._snippets)
        for item in self._snippets:
            item.score *= factor
        self._snippets = [
            item for item in self._snippets
            if abs(item.score) >= min_abs_score
        ]
        self._prune()
        after = len(self._snippets)
        return {"before": before, "after": after, "removed": before - after}

    def to_dict(self) -> dict[str, Any]:
        return {
            "capacity": self.capacity,
            "snippet_chars": self.snippet_chars,
            "merge_threshold": self.merge_threshold,
            "snippets": [item.to_dict() for item in self._snippets],
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "FastContextPool":
        pool = cls(
            capacity=int(data.get("capacity", 32)),
            snippet_chars=int(data.get("snippet_chars", 320)),
            merge_threshold=float(data.get("merge_threshold", 0.7)),
        )
        pool._snippets = [
            _ContextSnippet.from_dict(item) for item in data.get("snippets", [])
        ]
        pool._prune()
        return pool

    def _make_snippet(
        self, marker: str, example: RolloutExample, feedback: Feedback
    ) -> str:
        pieces = [
            f"{marker}: {example.response.strip()}",
            f"when: {example.prompt.strip()}",
        ]
        if feedback.notes:
            pieces.append(f"because: {feedback.notes.strip()}")
        if feedback.tags:
            pieces.append("tags: " + ", ".join(feedback.tags))
        return " | ".join(pieces)[: self.snippet_chars]

    def _prune(self) -> None:
        self._snippets.sort(
            key=lambda item: (abs(item.score), item.uses, item.text), reverse=True
        )
        del self._snippets[self.capacity :]


class SlowTokenModel:
    """A gradual unigram/bigram scorer used as persistent slow weights."""

    def __init__(
        self,
        *,
        learning_rate: float = 0.25,
        penalty_rate: float = 0.08,
        smoothing: float = 0.5,
    ) -> None:
        if learning_rate <= 0.0:
            raise ValueError("learning_rate must be positive")
        if penalty_rate < 0.0:
            raise ValueError("penalty_rate must be non-negative")
        if smoothing <= 0.0:
            raise ValueError("smoothing must be positive")
        self.learning_rate = learning_rate
        self.penalty_rate = penalty_rate
        self.smoothing = smoothing
        self.unigrams: dict[str, float] = {}
        self.bigrams: dict[str, dict[str, float]] = {}
        self.penalties: dict[str, float] = {}

    def observe(
        self,
        example: RolloutExample,
        feedback: Feedback,
        *,
        protected_prefixes: Iterable[str] = (),
    ) -> None:
        """Update slowly from a rollout without overwriting existing counts."""

        toks = _tokens(example.response)
        if not toks:
            return
        if feedback.reward >= 0.0:
            amount = self.learning_rate * max(feedback.reward, 0.1)
            self._add_counts(toks, amount)
        else:
            amount = self.penalty_rate * abs(feedback.reward)
            protected = tuple(str(prefix) for prefix in protected_prefixes if str(prefix))
            for tok in toks:
                if _is_protected_feature_token(tok, protected):
                    continue
                self.penalties[tok] = self.penalties.get(tok, 0.0) + amount

    def score(self, prompt: str, candidate: str) -> float:
        """Return average log-likelihood adjusted by learned penalties."""

        candidate_tokens = _tokens(candidate)
        if not candidate_tokens:
            return -math.inf

        context_tokens = _tokens(prompt)
        prev = context_tokens[-1] if context_tokens else "<s>"
        vocab = max(1, len(self.unigrams))
        total_tokens = sum(self.unigrams.values())
        total_logp = 0.0
        total_penalty = 0.0

        for tok in candidate_tokens:
            next_counts = self.bigrams.get(prev, {})
            denom = sum(next_counts.values()) + self.smoothing * vocab
            if tok in next_counts or denom > 0.0:
                prob = (next_counts.get(tok, 0.0) + self.smoothing) / denom
            else:
                prob = (self.unigrams.get(tok, 0.0) + self.smoothing) / (
                    total_tokens + self.smoothing * vocab
                )
            total_logp += math.log(prob)
            total_penalty += self.penalties.get(tok, 0.0)
            prev = tok

        return (total_logp / len(candidate_tokens)) - total_penalty

    def explain(self, prompt: str, candidate: str) -> dict[str, Any]:
        """Return token-level evidence used by the slow scorer."""
        candidate_tokens = _tokens(candidate)
        context_tokens = _tokens(prompt)
        prev = context_tokens[-1] if context_tokens else "<s>"
        known_tokens: list[str] = []
        penalized_tokens: list[str] = []
        bigram_hits: list[str] = []
        for tok in candidate_tokens:
            count = self.unigrams.get(tok, 0.0)
            if count > 0.0:
                known_tokens.append(f"{tok}:{count:.3f}")
            penalty = self.penalties.get(tok, 0.0)
            if penalty > 0.0:
                penalized_tokens.append(f"{tok}:{penalty:.3f}")
            bigram_count = self.bigrams.get(prev, {}).get(tok, 0.0)
            if bigram_count > 0.0:
                bigram_hits.append(f"{prev}->{tok}:{bigram_count:.3f}")
            prev = tok
        return {
            "tokens": tuple(candidate_tokens),
            "known_tokens": tuple(known_tokens),
            "penalized_tokens": tuple(penalized_tokens),
            "bigram_hits": tuple(bigram_hits),
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            "learning_rate": self.learning_rate,
            "penalty_rate": self.penalty_rate,
            "smoothing": self.smoothing,
            "unigrams": self.unigrams,
            "bigrams": self.bigrams,
            "penalties": self.penalties,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "SlowTokenModel":
        model = cls(
            learning_rate=float(data.get("learning_rate", 0.25)),
            penalty_rate=float(data.get("penalty_rate", 0.08)),
            smoothing=float(data.get("smoothing", 0.5)),
        )
        model.unigrams = {str(k): float(v) for k, v in data.get("unigrams", {}).items()}
        model.bigrams = {
            str(prev): {str(tok): float(count) for tok, count in counts.items()}
            for prev, counts in data.get("bigrams", {}).items()
        }
        model.penalties = {
            str(k): float(v) for k, v in data.get("penalties", {}).items()
        }
        return model

    def _add_counts(self, toks: list[str], amount: float) -> None:
        prev = "<s>"
        for tok in toks:
            self.unigrams[tok] = self.unigrams.get(tok, 0.0) + amount
            bucket = self.bigrams.setdefault(prev, {})
            bucket[tok] = bucket.get(tok, 0.0) + amount
            prev = tok


def _mlp_initial_weight(*parts: object, scale: float = 0.08) -> float:
    raw = "|".join(str(part) for part in parts)
    digest = hashlib.blake2b(raw.encode("utf-8"), digest_size=8).digest()
    unit = int.from_bytes(digest, "big") / float(2**64 - 1)
    return (unit - 0.5) * scale


def _mlp_hash_index(label: str, input_dim: int) -> int:
    digest = hashlib.blake2b(label.encode("utf-8"), digest_size=8).digest()
    return int.from_bytes(digest, "big") % max(1, int(input_dim))


def _mlp_add_feature(
    vector: list[float],
    labels: dict[int, list[str]],
    label: str,
    value: float,
) -> None:
    if not label or value == 0.0:
        return
    index = _mlp_hash_index(label, len(vector))
    vector[index] += float(value)
    bucket = labels.setdefault(index, [])
    if len(bucket) < 4:
        bucket.append(label)


def _mlp_feature_vector(
    prompt: str,
    candidate: str,
    *,
    input_dim: int,
) -> tuple[list[float], dict[int, list[str]]]:
    vector = [0.0 for _ in range(max(1, int(input_dim)))]
    labels: dict[int, list[str]] = {}
    prompt_tokens = _tokens(prompt)
    candidate_tokens = _tokens(candidate)
    for tok in dict.fromkeys(candidate_tokens):
        _mlp_add_feature(vector, labels, f"candidate:{tok}", 1.0)
    for tok in dict.fromkeys(prompt_tokens):
        _mlp_add_feature(vector, labels, f"prompt:{tok}", 0.35)
    for tok in set(prompt_tokens).intersection(candidate_tokens):
        _mlp_add_feature(vector, labels, f"overlap:{tok}", 0.75)
    for left, right in zip(candidate_tokens, candidate_tokens[1:]):
        _mlp_add_feature(vector, labels, f"candidate_bigram:{left}->{right}", 0.5)
    if candidate_tokens:
        length_bucket = min(8, max(1, int(math.log2(len(candidate_tokens))) + 1))
        _mlp_add_feature(vector, labels, f"candidate_length_bucket:{length_bucket}", 0.5)
    norm = math.sqrt(sum(value * value for value in vector))
    if norm > 0.0:
        vector = [value / norm for value in vector]
    return vector, labels


def _caseops_mlp_row_text(row: dict[str, Any], row_kind: str) -> str:
    fields = (
        "id",
        "label",
        "source",
        "target",
        "formula_id",
        "language_kind",
        "language_value",
        "family_id",
        "outcome",
        "kind",
        "value",
        "detail",
        "encoded",
        "motif",
        "relation",
        "target_kind",
        "target_value",
        "card_id",
        "gap",
        "reason",
        "status",
        "layer",
        "repair_query",
        "axis",
        "axis_a",
        "axis_b",
        "route",
        "route_code",
        "packet",
        "tensor_code",
        "axis_code",
        "polarity",
        "target_source",
        "neighbor_source",
        "encoding",
        "codec",
        "primer",
        "hops",
    )
    sequence_fields = (
        "nodes",
        "relations",
        "matched_terms",
        "axes",
        "positive_axes",
        "negative_axes",
        "neutral_axes",
        "shared_axes",
        "opposing_axes",
        "axis_roots",
        "formulas",
        "concepts",
        "tags",
        "stacks",
        "suggestions",
        "neighbors",
        "stack_codes",
        "packets",
        "reasons",
    )
    mapping_fields = (
        "coordinates",
        "coded_coordinates",
        "decoded_axes",
    )
    parts = [f"caseops_mlp_row={row_kind}"]
    for field_name in fields:
        value = row.get(field_name)
        if value not in (None, "", ()):
            parts.append(f"{field_name}={value}")
    for field_name in sequence_fields:
        value = row.get(field_name)
        if isinstance(value, (list, tuple, set)) and value:
            pieces: list[str] = []
            for item in tuple(value)[:12]:
                if isinstance(item, dict):
                    pieces.extend(
                        str(item.get(key) or "")
                        for key in (
                            "id",
                            "value",
                            "target",
                            "axis",
                            "code",
                            "layer",
                            "relation",
                            "route_code",
                            "packet",
                        )
                        if item.get(key) not in (None, "")
                    )
                else:
                    pieces.append(str(item))
            if pieces:
                parts.append(f"{field_name}=" + " ".join(pieces))
    for field_name in mapping_fields:
        value = row.get(field_name)
        if isinstance(value, dict) and value:
            pieces = [
                f"{key}:{raw_value}"
                for key, raw_value in tuple(value.items())[:12]
                if str(key).strip()
            ]
            if pieces:
                parts.append(f"{field_name}=" + " ".join(pieces))
    return " ".join(parts)


def _annotate_caseops_mlp_scores(
    learner: "MicroLearner",
    query: str,
    rows: list[dict[str, Any]],
    *,
    row_kind: str,
    score_field: str = "score",
) -> None:
    if not rows or learner.mlp_model.trained_examples <= 0:
        return
    for row in rows:
        text = _caseops_mlp_row_text(row, row_kind)
        mlp_score = learner.mlp_model.score(str(query or ""), text)
        symbolic_score = float(row.get(score_field, row.get("score", 0.0)) or 0.0)
        row["symbolic_score"] = symbolic_score
        row["mlp_score"] = mlp_score
        row["score"] = symbolic_score + learner.mlp_weight * mlp_score


@dataclass
class TinyMLPScorer:
    """Bounded online MLP used as a shared neural prior for candidate ranking."""

    input_dim: int = 96
    hidden_dim: int = 16
    learning_rate: float = 0.05
    trained_examples: int = 0
    w1: list[list[float]] = field(default_factory=list)
    b1: list[float] = field(default_factory=list)
    w2: list[float] = field(default_factory=list)
    b2: float = 0.0

    def __post_init__(self) -> None:
        self.input_dim = max(8, int(self.input_dim))
        self.hidden_dim = max(2, int(self.hidden_dim))
        self.learning_rate = max(0.0, float(self.learning_rate))
        if (
            len(self.w1) != self.input_dim
            or any(len(row) != self.hidden_dim for row in self.w1)
        ):
            self.w1 = [
                [
                    _mlp_initial_weight("w1", i, j)
                    for j in range(self.hidden_dim)
                ]
                for i in range(self.input_dim)
            ]
        if len(self.b1) != self.hidden_dim:
            self.b1 = [0.0 for _ in range(self.hidden_dim)]
        if len(self.w2) != self.hidden_dim:
            self.w2 = [
                _mlp_initial_weight("w2", j)
                for j in range(self.hidden_dim)
            ]
        self.b2 = float(self.b2)
        self.trained_examples = max(0, int(self.trained_examples))

    def observe(self, example: RolloutExample, feedback: Feedback) -> None:
        if self.learning_rate <= 0.0:
            return
        x, _ = _mlp_feature_vector(
            example.prompt,
            example.response,
            input_dim=self.input_dim,
        )
        if not any(x):
            return
        target = max(-1.0, min(1.0, float(feedback.reward)))
        pred, hidden = self._forward(x)
        error = pred - target
        output_grad = error * (1.0 - pred * pred)
        old_w2 = list(self.w2)
        for j, hidden_value in enumerate(hidden):
            self.w2[j] -= self.learning_rate * output_grad * hidden_value
        self.b2 -= self.learning_rate * output_grad
        for j, hidden_value in enumerate(hidden):
            hidden_grad = output_grad * old_w2[j] * (1.0 - hidden_value * hidden_value)
            if hidden_grad == 0.0:
                continue
            self.b1[j] -= self.learning_rate * hidden_grad
            for i, value in enumerate(x):
                if value:
                    self.w1[i][j] -= self.learning_rate * hidden_grad * value
        self.trained_examples += 1

    def score(self, prompt: str, candidate: str) -> float:
        if self.trained_examples <= 0:
            return 0.0
        x, _ = _mlp_feature_vector(prompt, candidate, input_dim=self.input_dim)
        if not any(x):
            return 0.0
        pred, _ = self._forward(x)
        return float(pred)

    def explain(self, prompt: str, candidate: str, *, top_n: int = 8) -> dict[str, Any]:
        x, labels = _mlp_feature_vector(prompt, candidate, input_dim=self.input_dim)
        score = self.score(prompt, candidate)
        active = sorted(
            (
                (abs(value), index, value)
                for index, value in enumerate(x)
                if value != 0.0
            ),
            reverse=True,
        )[: max(1, int(top_n))]
        features = []
        for _, index, value in active:
            names = ",".join(labels.get(index, ())[:4]) or f"hash:{index}"
            features.append(f"{names}:{value:.3f}")
        return {
            "score": score,
            "trained_examples": self.trained_examples,
            "input_dim": self.input_dim,
            "hidden_dim": self.hidden_dim,
            "features": tuple(features),
        }

    def _forward(self, x: list[float]) -> tuple[float, list[float]]:
        hidden: list[float] = []
        for j in range(self.hidden_dim):
            total = self.b1[j]
            for i, value in enumerate(x):
                if value:
                    total += value * self.w1[i][j]
            hidden.append(math.tanh(total))
        output = self.b2
        for j, hidden_value in enumerate(hidden):
            output += hidden_value * self.w2[j]
        return math.tanh(output), hidden

    def to_dict(self) -> dict[str, Any]:
        return {
            "input_dim": self.input_dim,
            "hidden_dim": self.hidden_dim,
            "learning_rate": self.learning_rate,
            "trained_examples": self.trained_examples,
            "w1": self.w1,
            "b1": self.b1,
            "w2": self.w2,
            "b2": self.b2,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any] | None) -> "TinyMLPScorer":
        if not isinstance(data, dict):
            return cls()
        input_dim = int(data.get("input_dim", 96))
        hidden_dim = int(data.get("hidden_dim", 16))
        w1_raw = data.get("w1")
        w1: list[list[float]] = []
        if isinstance(w1_raw, list):
            for row in w1_raw:
                if isinstance(row, list):
                    w1.append([float(value) for value in row])
        b1_raw = data.get("b1")
        b1 = [float(value) for value in b1_raw] if isinstance(b1_raw, list) else []
        w2_raw = data.get("w2")
        w2 = [float(value) for value in w2_raw] if isinstance(w2_raw, list) else []
        return cls(
            input_dim=input_dim,
            hidden_dim=hidden_dim,
            learning_rate=float(data.get("learning_rate", 0.05)),
            trained_examples=int(data.get("trained_examples", 0)),
            w1=w1,
            b1=b1,
            w2=w2,
            b2=float(data.get("b2", 0.0)),
        )


def _top_weighted_prefix(
    weights: dict[str, float],
    prefix: str,
    *,
    top_n: int,
) -> list[dict[str, Any]]:
    rows = [
        (token.removeprefix(prefix), token, weight)
        for token, weight in weights.items()
        if token.startswith(prefix)
    ]
    rows.sort(key=lambda item: (-item[2], item[0]))
    return [
        {"value": value, "token": token, "weight": weight}
        for value, token, weight in rows[:top_n]
    ]


def caseops_neuron_graph(
    learner: "MicroLearner",
    *,
    top_n: int = 12,
    min_weight: float = 0.0,
) -> dict[str, Any]:
    """Return learned CaseOps features as formula/language nodes and synapses."""
    return _caseops_neuron_graph_from_unigrams(
        learner.slow_model.unigrams,
        top_n=top_n,
        min_weight=min_weight,
    )


def caseops_semantic_pathways(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned formula-to-semantic-language CaseOps pathways."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_semantic_pathway_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_semantic_pathway_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="semantic_pathway",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0),
        str(row.get("formula_id") or ""),
        str(row.get("language_kind") or ""),
        str(row.get("language_value") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "pathways": selected,
        "summary": {
            "pathways": len(candidates),
            "shown": len(selected),
            "roles": sum(1 for row in candidates if row.get("language_kind") == "lang.role"),
            "concepts": sum(1 for row in candidates if row.get("language_kind") == "lang.concept"),
            "predicates": sum(1 for row in candidates if row.get("language_kind") == "lang.predicate"),
            "arguments": sum(1 for row in candidates if row.get("language_kind") == "lang.argument"),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_semantic_primer(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 6,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Summarize learned formula semantics as compact math-language facts."""
    top_n = max(1, int(top_n))
    recalled = caseops_semantic_pathways(
        learner,
        query,
        top_n=max(top_n * 4, 12),
        min_score=min_score,
    )
    rows = recalled.get("pathways") if isinstance(recalled, dict) else ()
    grouped: dict[str, dict[str, Any]] = {}
    if not isinstance(rows, (list, tuple)):
        rows = ()
    for row in rows:
        if not isinstance(row, dict):
            continue
        formula_id = str(row.get("formula_id") or "").strip()
        language_kind = str(row.get("language_kind") or "").strip()
        language_value = str(row.get("language_value") or "").strip()
        if not formula_id or not language_kind or not language_value:
            continue
        item = grouped.setdefault(
            formula_id,
            {
                "formula_id": formula_id,
                "score": 0.0,
                "effective_weight": 0.0,
                "roles": {},
                "concepts": {},
                "predicates": {},
                "arguments": {},
                "pathways": [],
            "matched_terms": set(),
            "mlp_score": 0.0,
        },
    )
        score = float(row.get("score") or 0.0)
        effective = float(
            row.get("effective_weight", row.get("weight", 0.0)) or 0.0
        )
        item["score"] = float(item["score"]) + score
        item["effective_weight"] = float(item["effective_weight"]) + effective
        item["mlp_score"] = float(item["mlp_score"]) + float(row.get("mlp_score") or 0.0)
        bucket_name = {
            "lang.role": "roles",
            "lang.concept": "concepts",
            "lang.predicate": "predicates",
            "lang.argument": "arguments",
        }.get(language_kind)
        if bucket_name:
            bucket = item[bucket_name]
            bucket[language_value] = max(float(bucket.get(language_value, 0.0)), score)
        item["pathways"].append(dict(row))
        item["matched_terms"].update(str(term) for term in row.get("matched_terms", ()))

    primers: list[dict[str, Any]] = []
    for item in grouped.values():
        roles = _caseops_primer_values(item["roles"])
        concepts = _caseops_primer_values(item["concepts"])
        predicates = _caseops_primer_values(item["predicates"])
        arguments = _caseops_primer_values(item["arguments"])
        facts = []
        if roles:
            facts.append("roles=" + ",".join(roles))
        if concepts:
            facts.append("concepts=" + ",".join(concepts))
        if predicates:
            facts.append("predicates=" + ",".join(predicates))
        if arguments:
            facts.append("arguments=" + ",".join(arguments))
        primer_text = (
            f"{item['formula_id']}: "
            + ("; ".join(facts) if facts else "semantic links=-")
        )
        primers.append({
            "formula_id": item["formula_id"],
            "score": float(item["score"]),
            "effective_weight": float(item["effective_weight"]),
            "mlp_score": float(item["mlp_score"]),
            "roles": roles,
            "concepts": concepts,
            "predicates": predicates,
            "arguments": arguments,
            "matched_terms": tuple(sorted(item["matched_terms"])),
            "pathways": tuple(item["pathways"][:top_n]),
            "primer": primer_text,
        })
    primers.sort(key=lambda row: (
        -float(row["score"]),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row["effective_weight"])),
        str(row["formula_id"]),
    ))
    selected = tuple(dict(row) for row in primers[:top_n])
    return {
        "query": str(query or ""),
        "terms": recalled.get("terms", ()) if isinstance(recalled, dict) else (),
        "primers": selected,
        "summary": {
            "formulas": len(primers),
            "shown": len(selected),
            "pathways": len(rows),
            "roles": sum(len(row["roles"]) for row in primers),
            "concepts": sum(len(row["concepts"]) for row in primers),
            "predicates": sum(len(row["predicates"]) for row in primers),
            "arguments": sum(len(row["arguments"]) for row in primers),
            "mlp_scored": sum(1 for row in primers if float(row.get("mlp_score") or 0.0) != 0.0),
        },
    }


def _caseops_primer_values(values: dict[str, float]) -> tuple[str, ...]:
    rows = sorted(values.items(), key=lambda item: (-float(item[1]), item[0]))
    return tuple(value for value, _ in rows[:5])


def caseops_family_outcomes(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall reward/penalty evidence accumulated for CaseOps formula families."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_family_outcome_rows(
        learner.slow_model.unigrams,
        penalties=learner.slow_model.penalties,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_family_outcome_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="family_outcome",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("family_id") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "families": selected,
        "summary": {
            "families": len(candidates),
            "shown": len(selected),
            "positive": sum(1 for row in candidates if float(row.get("weight") or 0.0) > 0.0),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "net_positive": sum(1 for row in candidates if float(row.get("effective_weight") or 0.0) > 0.0),
            "net_negative": sum(1 for row in candidates if float(row.get("effective_weight") or 0.0) < 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_linguistic_links(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned CaseOps linguistic links such as ling.concept:*."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_linguistic_link_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_linguistic_link_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="linguistic_link",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
        str(row.get("detail") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "links": selected,
        "summary": {
            "links": len(candidates),
            "shown": len(selected),
            "concepts": sum(1 for row in candidates if row.get("kind") == "concept"),
            "roles": sum(1 for row in candidates if row.get("kind") == "role"),
            "predicates": sum(1 for row in candidates if row.get("kind") == "predicate"),
            "idioms": sum(1 for row in candidates if row.get("kind") == "idiom"),
            "quantifiers": sum(1 for row in candidates if row.get("kind") == "quantifier"),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_consolidation_evidence(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_support: int = 2,
    min_abs_weight: float = 0.0,
) -> dict[str, Any]:
    """Report evidence-gated CaseOps links ready for durable consolidation."""
    top_n = max(1, int(top_n))
    min_support = max(1, int(min_support))
    min_abs_weight = max(0.0, float(min_abs_weight))
    query_terms = _activation_terms(query)
    snippets = list(learner.context_pool.to_dict()["snippets"])
    evidence = _caseops_consolidation_snippet_index(snippets)
    raw_rows: list[dict[str, Any]] = []
    raw_rows.extend(
        _caseops_consolidation_source_rows(
            "semantic",
            _caseops_semantic_pathway_rows(
                learner.slow_model.unigrams,
                top_n=max(len(learner.slow_model.unigrams), top_n * 8, 64),
                penalties=learner.slow_model.penalties,
                include_penalties=True,
            ),
        )
    )
    raw_rows.extend(
        _caseops_consolidation_source_rows(
            "family",
            _caseops_family_outcome_rows(
                learner.slow_model.unigrams,
                top_n=max(len(learner.slow_model.unigrams), top_n * 8, 64),
                penalties=learner.slow_model.penalties,
                include_penalties=True,
            ),
        )
    )
    raw_rows.extend(
        _caseops_consolidation_source_rows(
            "linguistic",
            _caseops_linguistic_link_rows(
                learner.slow_model.unigrams,
                top_n=max(len(learner.slow_model.unigrams), top_n * 8, 64),
                penalties=learner.slow_model.penalties,
                include_penalties=True,
            ),
        )
    )

    rows: list[dict[str, Any]] = []
    for raw in raw_rows:
        effective = float(
            raw.get("effective_weight", raw.get("weight", 0.0)) or 0.0
        )
        if abs(effective) < min_abs_weight:
            continue
        matched = _caseops_consolidation_query_match(raw, query_terms)
        if query_terms and not matched:
            continue
        rows.append(
            _caseops_consolidation_evidence_row(
                raw,
                evidence,
                matched_terms=matched,
                min_support=min_support,
            )
        )
    _annotate_caseops_mlp_scores(
        learner,
        query,
        rows,
        row_kind="consolidation_evidence",
        score_field="gated_score",
    )
    rows.sort(key=lambda row: (
        0 if row.get("status") == "ready" else 1,
        0 if row.get("status") == "contested" else 1,
        -int(row.get("support_count") or 0),
        -float(row.get("score") or 0.0),
        -abs(float(row.get("effective_weight") or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("target") or ""),
    ))
    selected = tuple(dict(row) for row in rows[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "min_support": min_support,
        "min_abs_weight": min_abs_weight,
        "links": selected,
        "summary": {
            "links": len(rows),
            "shown": len(selected),
            "ready": sum(1 for row in rows if row.get("status") == "ready"),
            "needs_evidence": sum(
                1 for row in rows if row.get("status") == "needs_evidence"
            ),
            "contested": sum(1 for row in rows if row.get("status") == "contested"),
            "penalized": sum(1 for row in rows if row.get("status") == "penalized"),
            "fast_evidence": len(evidence),
            "mlp_scored": sum(1 for row in rows if "mlp_score" in row),
        },
    }


def _caseops_consolidation_source_rows(
    kind: str,
    rows: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        item = dict(row)
        item["kind"] = kind
        if kind == "semantic":
            item["target"] = (
                f"{row.get('formula_id')}->{row.get('language_kind')}:"
                f"{row.get('language_value')}"
            )
        elif kind == "family":
            item["target"] = (
                f"family:{row.get('family_id')}:{row.get('outcome') or 'mixed'}"
            )
        elif kind == "linguistic":
            item["target"] = row.get("target")
        out.append(item)
    return out


def _caseops_consolidation_snippet_index(
    snippets: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for index, item in enumerate(snippets):
        if not isinstance(item, dict):
            continue
        text = str(item.get("text") or "")
        tokens = set(_tokens(text))
        if not any(token == "caseops" or token.startswith("caseops_") for token in tokens):
            continue
        try:
            score = float(item.get("score") or 0.0)
        except Exception:  # noqa: BLE001
            score = 0.0
        out.append({
            "index": index,
            "text": " ".join(text.split())[:180],
            "score": score,
            "uses": int(item.get("uses") or 0),
            "tags": tuple(str(tag) for tag in item.get("tags", ()) or ()),
            "tokens": tokens,
        })
    return out


def _caseops_consolidation_query_match(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[str, ...]:
    if not query_terms:
        return ()
    formulas = row.get("formulas")
    formula_text = ""
    if isinstance(formulas, (list, tuple)):
        formula_text = " ".join(
            str(item.get("formula_id") or "")
            for item in formulas
            if isinstance(item, dict)
        )
    row_terms = _activation_terms(" ".join((
        str(row.get("kind") or ""),
        str(row.get("target") or ""),
        str(row.get("token") or ""),
        str(row.get("family_id") or ""),
        str(row.get("formula_id") or ""),
        str(row.get("language_kind") or ""),
        str(row.get("language_value") or ""),
        str(row.get("value") or ""),
        str(row.get("detail") or ""),
        formula_text,
    )))
    return tuple(sorted(query_terms.intersection(row_terms)))


def _caseops_consolidation_evidence_row(
    row: dict[str, Any],
    evidence: list[dict[str, Any]],
    *,
    matched_terms: tuple[str, ...],
    min_support: int,
) -> dict[str, Any]:
    token = str(row.get("token") or "")
    matches = [
        item for item in evidence
        if token and token in item.get("tokens", set())
    ]
    positive = [
        item for item in matches
        if float(item.get("score") or 0.0) > 0.0
    ]
    negative = [
        item for item in matches
        if float(item.get("score") or 0.0) < 0.0
    ]
    effective = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    support_count = len(matches)
    if positive and negative:
        status = "contested"
    elif effective < 0.0:
        status = "penalized"
    elif support_count >= min_support:
        status = "ready"
    else:
        status = "needs_evidence"
    gated_score = effective * min(1.0, support_count / max(1, min_support))
    return {
        "kind": row.get("kind"),
        "target": row.get("target"),
        "token": token,
        "weight": float(row.get("weight") or 0.0),
        "penalty": float(row.get("penalty") or 0.0),
        "effective_weight": effective,
        "gated_score": gated_score,
        "status": status,
        "support_count": support_count,
        "positive_evidence": len(positive),
        "negative_evidence": len(negative),
        "matched_terms": matched_terms,
        "evidence": tuple(
            {
                "index": item.get("index"),
                "score": item.get("score"),
                "uses": item.get("uses"),
                "tags": item.get("tags"),
                "text": item.get("text"),
            }
            for item in matches[:3]
        ),
    }


def _caseops_consolidated_target_token(row: dict[str, Any]) -> str:
    target = _normalize_feature_piece(str(row.get("target") or ""))
    if not target:
        return ""
    return f"caseops_consolidated_target_{target}"


def _caseops_consolidated_response(row: dict[str, Any], token: str) -> str:
    target = str(row.get("target") or "").strip()
    kind = _normalize_feature_piece(str(row.get("kind") or "unknown"))
    if not target or not token:
        return ""
    support = int(row.get("support_count") or 0)
    gated = float(row.get("gated_score") or 0.0)
    pieces = [
        "caseops consolidated evidence",
        "caseops_consolidated_link",
        "caseops_consolidated_status_ready",
        f"caseops_consolidated_kind_{kind}",
        token,
        f"target={target}",
        f"support={support}",
        f"gated={gated:.3f}",
    ]
    if target.startswith("ling."):
        pieces.append(target)
    elif "->lang." in target:
        formula_id, _, language = target.partition("->")
        kind_part, _, value = language.partition(":")
        if formula_id and kind_part and value:
            pieces.append(f"{formula_id}->{kind_part}:{value}")
    elif target.startswith("family:"):
        _, _, rest = target.partition(":")
        family_id, _, outcome = rest.partition(":")
        clean_family = _normalize_feature_piece(family_id)
        if clean_family:
            pieces.append(f"caseops_family_{clean_family}")
        if outcome:
            pieces.append(f"caseops_family_outcome_{_normalize_feature_piece(outcome)}")
    return " ".join(piece for piece in pieces if piece)


def caseops_consolidated_targets(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall durable CaseOps links promoted by evidence consolidation."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_consolidated_target_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_consolidated_target_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="consolidated_target",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("target") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "targets": selected,
        "summary": {
            "targets": len(candidates),
            "shown": len(selected),
            "linguistic": sum(1 for row in candidates if row.get("kind") == "linguistic"),
            "semantic": sum(1 for row in candidates if row.get("kind") == "semantic"),
            "family": sum(1 for row in candidates if row.get("kind") == "family"),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def _caseops_consolidated_target_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    prefix = "caseops_consolidated_target_"
    tokens = {
        token for token in unigrams
        if token.startswith(prefix)
    }
    if include_penalties and penalties:
        tokens.update(
            token for token in penalties
            if token.startswith(prefix)
        )
    rows: list[dict[str, Any]] = []
    for token in tokens:
        value = token.removeprefix(prefix)
        target, kind = _decode_caseops_consolidated_target(value)
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        effective_weight = weight - penalty
        rows.append({
            "kind": kind,
            "value": value,
            "target": target,
            "token": token,
            "weight": weight,
            "penalty": penalty,
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["kind"]),
        str(row["target"]),
    ))
    return rows[: max(1, int(top_n))]


def _decode_caseops_consolidated_target(value: str) -> tuple[str, str]:
    clean = _normalize_feature_piece(value)
    if clean.startswith("ling_"):
        parts = [part for part in clean.split("_") if part]
        if len(parts) >= 3:
            kind = parts[1]
            target = f"ling.{kind}:{parts[2]}"
            if len(parts) > 3:
                target += ":" + "_".join(parts[3:])
            return (target, "linguistic")
    if "_lang_" in clean:
        formula, _, rest = clean.partition("_lang_")
        parts = [part for part in rest.split("_") if part]
        if formula and len(parts) >= 2:
            return (f"{formula}->lang.{parts[0]}:{'_'.join(parts[1:])}", "semantic")
    if clean.startswith("family_"):
        rest = clean.removeprefix("family_")
        outcomes = (
            "mixed_promote",
            "mixed_avoid",
            "promote",
            "avoid",
            "mixed",
            "neutral",
        )
        for outcome in outcomes:
            suffix = f"_{outcome}"
            if rest.endswith(suffix):
                return (f"family:{rest.removesuffix(suffix)}:{outcome}", "family")
        return (f"family:{rest}", "family")
    return (clean, "target")


def _caseops_consolidated_target_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    row_terms = _activation_terms(" ".join((
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
        str(row.get("target") or ""),
        str(row.get("token") or ""),
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_bonus = 0.5 if any(term in str(row.get("target") or "") for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def caseops_codec_gaps(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned open CaseOps codec gaps such as future.codec_gap:*."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_codec_gap_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 4, len(learner.slow_model.unigrams), 32),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_codec_gap_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="codec_gap",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("gap") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "gaps": selected,
        "summary": {
            "gaps": len(candidates),
            "shown": len(selected),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_gap_repair_hints(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
    hint_limit: int = 4,
    formula_store: str | Path | None = None,
) -> dict[str, Any]:
    """Recall learned mathlang gap targets and turn them into repair hints."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    hint_limit = max(1, int(hint_limit))
    query_terms = _activation_terms(query)
    slow_rows = _caseops_gap_repair_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    fast_rows = _caseops_gap_repair_rows_from_snippets(
        learner.context_pool.to_dict().get("snippets", ())
    )
    rows = _merge_caseops_gap_repair_rows((*slow_rows, *fast_rows))
    candidates: list[dict[str, Any]] = []
    for row in rows:
        enriched = _caseops_gap_repair_enriched_row(
            row,
            formula_store=formula_store,
            hint_limit=hint_limit,
        )
        score, matched = _caseops_gap_repair_score(enriched, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="gap_repair_hint",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("target") or ""),
        str(row.get("reason") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "hints": selected,
        "summary": {
            "hints": len(candidates),
            "shown": len(selected),
            "formula_anchors": sum(1 for row in candidates if row.get("status") == "formula_anchor"),
            "mathlang_aliases": sum(1 for row in candidates if row.get("status") == "mathlang_alias"),
            "semantic_pathways": sum(1 for row in candidates if row.get("status") == "mathlang_pathway"),
            "family_maps": sum(1 for row in candidates if row.get("status") == "mathlang_family"),
            "open_layers": sum(1 for row in candidates if row.get("status") == "open_layer"),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_vocab_vector_axes(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned multidimensional CaseOps vocab vector axes."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_vocab_vector_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_vocab_vector_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="vocab_vector_axis",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "vectors": selected,
        "summary": {
            "vectors": len(candidates),
            "shown": len(selected),
            "axes": sum(len(row.get("axes", ())) for row in candidates),
            "repair_targets": sum(
                1
                for row in candidates
                if any(str(axis).startswith("repair:") for axis in row.get("axes", ()))
            ),
            "learned_rules": sum(
                1
                for row in candidates
                if "learned:rule" in set(str(axis) for axis in row.get("axes", ()))
            ),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_vocab_tensor(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Project learned CaseOps vocab into signed sparse multidimensional rows."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_vocab_tensor_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_vocab_tensor_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        enriched["codec"] = encode_caseops_vocab_tensor(enriched)
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="vocab_tensor",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -float(row.get("magnitude") or 0.0),
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    basis = tuple(sorted({
        axis
        for row in selected
        for axis in (row.get("coordinates") or {}).keys()
        if str(axis).strip()
    }, key=_caseops_vocab_axis_sort_key))
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "basis": basis,
        "tensors": selected,
        "summary": {
            "tensors": len(candidates),
            "shown": len(selected),
            "basis_axes": len(basis),
            "coordinates": sum(
                len(row.get("coordinates") or {})
                for row in candidates
            ),
            "positive_axes": sum(
                len(row.get("positive_axes", ()) or ())
                for row in candidates
            ),
            "negative_axes": sum(
                len(row.get("negative_axes", ()) or ())
                for row in candidates
            ),
            "analogized": sum(
                1 for row in candidates if row.get("analogies")
            ),
            "penalized": sum(
                1 for row in candidates
                if float(row.get("penalty") or 0.0) > 0.0
            ),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_vocab_tensor_primer(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 6,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Summarize signed CaseOps vocab tensors as compact internal facts."""
    top_n = max(1, int(top_n))
    recalled = caseops_vocab_tensor(
        learner,
        query,
        top_n=max(top_n * 4, 12),
        min_score=min_score,
    )
    rows = recalled.get("tensors") if isinstance(recalled, dict) else ()
    if not isinstance(rows, (list, tuple)):
        rows = ()
    primers: list[dict[str, Any]] = []
    for row in rows[:top_n]:
        if not isinstance(row, dict):
            continue
        coordinates = row.get("coordinates") or {}
        if not isinstance(coordinates, dict) or not coordinates:
            continue
        ordered = tuple(sorted(
            (
                (str(axis), float(value))
                for axis, value in coordinates.items()
                if str(axis).strip() and float(value) != 0.0
            ),
            key=lambda item: (
                -abs(item[1]),
                _caseops_vocab_axis_sort_key(item[0]),
                item[0],
            ),
        ))
        positive = tuple(axis for axis, value in ordered if value > 0.0)[:4]
        negative = tuple(axis for axis, value in ordered if value < 0.0)[:3]
        kind = str(row.get("kind") or "")
        value = str(row.get("value") or "")
        magnitude = float(row.get("magnitude") or 0.0)
        dimensions = int(row.get("dimensions") or len(coordinates))
        primer = (
            f"{kind}:{value}: mag={magnitude:.3f} dims={dimensions} "
            f"positive={','.join(positive) if positive else '-'} "
            f"negative={','.join(negative) if negative else '-'}"
        )
        primers.append({
            "kind": kind,
            "value": value,
            "score": float(row.get("score") or 0.0),
            "magnitude": magnitude,
            "dimensions": dimensions,
            "positive_axes": positive,
            "negative_axes": negative,
            "coordinates": dict(ordered[:6]),
            "codec": row.get("codec") or encode_caseops_vocab_tensor(row),
            "primer": primer,
            "matched_terms": tuple(str(term) for term in row.get("matched_terms", ())),
        })
    basis = tuple(recalled.get("basis", ()) if isinstance(recalled, dict) else ())
    return {
        "query": str(query or ""),
        "terms": tuple(recalled.get("terms", ()) if isinstance(recalled, dict) else ()),
        "basis": basis,
        "primers": tuple(primers),
        "summary": {
            "primers": len(primers),
            "basis_axes": len(basis),
            "positive_axes": sum(len(row.get("positive_axes", ())) for row in primers),
            "negative_axes": sum(len(row.get("negative_axes", ())) for row in primers),
            "coordinates": sum(len(row.get("coordinates", {})) for row in primers),
        },
    }


def caseops_vocab_tensor_neighbors(
    learner: "MicroLearner",
    target: str,
    *,
    top_n: int = 8,
    min_similarity: float = 0.0,
) -> dict[str, Any]:
    """Find nearest learned vocab tensors by signed sparse-coordinate cosine."""
    top_n = max(1, int(top_n))
    min_similarity = float(min_similarity)
    rows = _caseops_vocab_tensor_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 16, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    target_row = _caseops_vocab_tensor_target(rows, target)
    if target_row is None:
        return {
            "target": str(target or ""),
            "target_tensor": None,
            "neighbors": (),
            "summary": {
                "candidates": 0,
                "shown": 0,
                "basis_axes": 0,
                "shared_axes": 0,
                "opposing_axes": 0,
            },
        }
    target_key = _caseops_vocab_tensor_key(target_row)
    target_coords = target_row.get("coordinates") or {}
    candidates: list[dict[str, Any]] = []
    for row in rows:
        if _caseops_vocab_tensor_key(row) == target_key:
            continue
        coordinates = row.get("coordinates") or {}
        if not isinstance(coordinates, dict):
            continue
        similarity = _caseops_sparse_cosine(target_coords, coordinates)
        if similarity < min_similarity:
            continue
        shared_axes = _caseops_tensor_shared_axes(target_coords, coordinates)
        opposing_axes = _caseops_tensor_opposing_axes(target_coords, coordinates)
        enriched = dict(row)
        enriched["similarity"] = similarity
        enriched["shared_axes"] = shared_axes
        enriched["opposing_axes"] = opposing_axes
        enriched["codec"] = encode_caseops_vocab_tensor(enriched)
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        target,
        candidates,
        row_kind="vocab_tensor_neighbor",
        score_field="similarity",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or row.get("similarity") or 0.0),
        -float(row.get("similarity") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -len(row.get("shared_axes", ()) or ()),
        len(row.get("opposing_axes", ()) or ()),
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    basis = tuple(sorted({
        axis
        for row in (target_row, *selected)
        for axis in (row.get("coordinates") or {}).keys()
        if str(axis).strip()
    }, key=_caseops_vocab_axis_sort_key))
    target_copy = dict(target_row)
    target_copy["codec"] = encode_caseops_vocab_tensor(target_copy)
    return {
        "target": str(target or ""),
        "target_tensor": target_copy,
        "basis": basis,
        "neighbors": selected,
        "summary": {
            "candidates": len(candidates),
            "shown": len(selected),
            "basis_axes": len(basis),
            "shared_axes": sum(len(row.get("shared_axes", ()) or ()) for row in candidates),
            "opposing_axes": sum(len(row.get("opposing_axes", ()) or ()) for row in candidates),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_vocab_tensor_codebook(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    axis_limit: int = 24,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Build a deterministic symbol table for signed vocab tensor packets."""
    top_n = max(1, int(top_n))
    axis_limit = max(1, int(axis_limit))
    recalled = caseops_vocab_tensor(
        learner,
        query,
        top_n=max(top_n * 4, axis_limit, 16),
        min_score=min_score,
    )
    rows = recalled.get("tensors") if isinstance(recalled, dict) else ()
    if not isinstance(rows, (list, tuple)):
        rows = ()
    axis_stats: dict[str, dict[str, Any]] = {}
    for row in rows:
        if not isinstance(row, dict):
            continue
        coordinates = row.get("coordinates") or {}
        if not isinstance(coordinates, dict):
            continue
        tensor_label = f"{row.get('kind')}:{row.get('value')}"
        row_score = max(
            1.0,
            abs(float(row.get("score") or 0.0)),
            abs(float(row.get("magnitude") or 0.0)),
        )
        for axis, raw_value in coordinates.items():
            axis_text = str(axis)
            if not axis_text.strip():
                continue
            try:
                value = float(raw_value)
            except Exception:  # noqa: BLE001
                continue
            if value == 0.0:
                continue
            stats = axis_stats.setdefault(axis_text, {
                "axis": axis_text,
                "root": axis_text.split(":", 1)[0] if ":" in axis_text else axis_text,
                "mass": 0.0,
                "positive": 0,
                "negative": 0,
                "tensors": set(),
            })
            stats["mass"] = float(stats["mass"]) + abs(value) * row_score
            stats["positive"] = int(stats["positive"]) + (1 if value > 0.0 else 0)
            stats["negative"] = int(stats["negative"]) + (1 if value < 0.0 else 0)
            stats["tensors"].add(tensor_label)
    axis_items = sorted(
        axis_stats.values(),
        key=lambda stats: (
            -float(stats.get("mass") or 0.0),
            _caseops_vocab_axis_sort_key(str(stats.get("axis") or "")),
        ),
    )[:axis_limit]
    axis_entries: list[dict[str, Any]] = []
    axis_code_by_name: dict[str, str] = {}
    for stats in axis_items:
        axis = str(stats.get("axis") or "")
        code = _caseops_tensor_codebook_axis_code(axis)
        axis_code_by_name[axis] = code
        axis_entries.append({
            "code": code,
            "axis": axis,
            "root": str(stats.get("root") or ""),
            "mass": float(stats.get("mass") or 0.0),
            "positive": int(stats.get("positive") or 0),
            "negative": int(stats.get("negative") or 0),
            "tensor_count": len(stats.get("tensors") or ()),
            "tensors": tuple(sorted(str(item) for item in stats.get("tensors") or ())),
        })
    tensor_entries: list[dict[str, Any]] = []
    packets: list[str] = []
    for row in tuple(row for row in rows if isinstance(row, dict))[:top_n]:
        coordinates = row.get("coordinates") or {}
        if not isinstance(coordinates, dict) or not coordinates:
            continue
        kind = str(row.get("kind") or "")
        value = str(row.get("value") or "")
        tensor_code = _caseops_tensor_codebook_tensor_code(kind, value)
        coded_coordinates: dict[str, float] = {}
        decoded_axes: dict[str, str] = {}
        for axis, raw_weight in sorted(
            coordinates.items(),
            key=lambda item: (
                -abs(float(item[1]) if _is_number_like(item[1]) else 0.0),
                _caseops_vocab_axis_sort_key(str(item[0])),
            ),
        ):
            axis_text = str(axis)
            axis_code = axis_code_by_name.get(axis_text)
            if not axis_code:
                continue
            try:
                weight = float(raw_weight)
            except Exception:  # noqa: BLE001
                continue
            if weight == 0.0:
                continue
            coded_coordinates[axis_code] = weight
            decoded_axes[axis_code] = axis_text
        if not coded_coordinates:
            continue
        packet = _caseops_tensor_codebook_packet(tensor_code, kind, value, coded_coordinates)
        packets.append(packet)
        tensor_entries.append({
            "code": tensor_code,
            "kind": kind,
            "value": value,
            "label": f"{kind}:{value}",
            "magnitude": float(row.get("magnitude") or 0.0),
            "dimensions": int(row.get("dimensions") or len(coordinates)),
            "coded_coordinates": coded_coordinates,
            "decoded_axes": decoded_axes,
            "codec": row.get("codec") or encode_caseops_vocab_tensor(row),
            "packet": packet,
        })
    return {
        "schema": "caseops_vocab_tensor_codebook/1",
        "query": str(query or ""),
        "terms": tuple(recalled.get("terms", ()) if isinstance(recalled, dict) else ()),
        "axes": tuple(axis_entries),
        "tensors": tuple(tensor_entries),
        "packets": tuple(packets),
        "summary": {
            "axes": len(axis_entries),
            "tensors": len(tensor_entries),
            "packets": len(packets),
            "positive_edges": sum(int(row.get("positive") or 0) for row in axis_entries),
            "negative_edges": sum(int(row.get("negative") or 0) for row in axis_entries),
        },
    }


def decode_caseops_vocab_tensor_codebook(text: str) -> dict[str, Any]:
    """Decode ``CTV1`` tensor codebook packets into inspectable tensor rows."""
    packets = _parse_caseops_vocab_tensor_codebook_items(text)
    decoded: list[dict[str, Any]] = []
    for packet in packets:
        coordinates = packet.get("coordinates") or {}
        decoded_axes = packet.get("decoded_axes") or {}
        if not isinstance(coordinates, dict):
            coordinates = {}
        if not isinstance(decoded_axes, dict):
            decoded_axes = {}
        decoded_coordinates: dict[str, float] = {}
        for axis_code, raw_weight in coordinates.items():
            try:
                weight = float(raw_weight)
            except Exception:  # noqa: BLE001
                continue
            if weight == 0.0:
                continue
            axis = str(decoded_axes.get(str(axis_code), "") or str(axis_code))
            decoded_coordinates[axis] = weight
        row = {
            "tensor_code": packet.get("tensor_code"),
            "kind": packet.get("kind"),
            "value": packet.get("value"),
            "coded_coordinates": {
                str(axis_code): float(weight)
                for axis_code, weight in coordinates.items()
            },
            "decoded_axes": {
                str(axis_code): str(axis)
                for axis_code, axis in decoded_axes.items()
            },
            "coordinates": decoded_coordinates,
        }
        row["codec"] = encode_caseops_vocab_tensor(row)
        decoded.append(row)
    axis_entries = tuple(
        {
            "code": axis_code,
            "axis": axis,
        }
        for axis_code, axis in sorted({
            (axis_code, axis)
            for row in decoded
            for axis_code, axis in row.get("decoded_axes", {}).items()
        })
    )
    return {
        "schema": "caseops_vocab_tensor_codebook_decode/1",
        "packets": packets,
        "axes": axis_entries,
        "tensors": tuple(decoded),
        "summary": {
            "packets": len(packets),
            "axes": len(axis_entries),
            "tensors": len(decoded),
            "decoded_coordinates": sum(
                len(row.get("coordinates", {}))
                for row in decoded
            ),
        },
    }


def decode_caseops_vocab_tensor_codebook_routes(text: str) -> dict[str, Any]:
    """Decode ``CVR1`` route packets into inspectable codebook pathways."""
    packets = _parse_caseops_vocab_tensor_codebook_route_items(text)
    routes: list[dict[str, Any]] = []
    for packet in packets:
        polarity = str(packet.get("polarity") or "neutral")
        tensor_code = str(packet.get("tensor_code") or "")
        axis_code = str(packet.get("axis_code") or "")
        source = str(packet.get("source") or "")
        target = str(packet.get("target") or "")
        axis = str(packet.get("axis") or "")
        if not axis and target.startswith("axis:"):
            axis = target.split(":", 1)[1]
        axis_relation = (
            "codebook_tensor_positive_axis"
            if polarity == "positive" else
            "codebook_tensor_negative_axis"
            if polarity == "negative" else
            "codebook_tensor_axis"
        )
        route = {
            "route_code": packet.get("route_code"),
            "source": source,
            "target": target,
            "tensor_code": tensor_code,
            "axis_code": axis_code,
            "axis": axis,
            "polarity": polarity,
            "relations": (
                "codebook_tensor_identity",
                axis_relation,
                "codebook_axis_decode",
            ),
            "nodes": (
                source,
                f"tensor_code:{tensor_code}",
                f"axis_code:{axis_code}",
                target,
            ),
        }
        route["packet"] = _caseops_tensor_codebook_route_packet(route)
        routes.append(route)
    axis_entries = tuple(
        {
            "code": axis_code,
            "axis": axis,
        }
        for axis_code, axis in sorted({
            (str(row.get("axis_code") or ""), str(row.get("axis") or ""))
            for row in routes
            if row.get("axis_code") and row.get("axis")
        })
    )
    return {
        "schema": "caseops_vocab_tensor_codebook_route_decode/1",
        "packets": packets,
        "routes": tuple(routes),
        "axes": axis_entries,
        "summary": {
            "packets": len(packets),
            "routes": len(routes),
            "axes": len(axis_entries),
            "positive": sum(1 for row in routes if row.get("polarity") == "positive"),
            "negative": sum(1 for row in routes if row.get("polarity") == "negative"),
            "neutral": sum(1 for row in routes if row.get("polarity") == "neutral"),
        },
    }


def caseops_vocab_tensor_codebook_graph(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 12,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall a graph over learned ``CTV1`` tensor and axis-code symbols."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_vocab_tensor_codebook_edge_rows(
        learner.slow_model.unigrams,
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_vocab_tensor_codebook_edge_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="vocab_tensor_codebook_edge",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        str(row.get("source") or ""),
        str(row.get("target") or ""),
        str(row.get("relation") or ""),
    ))
    selected = tuple(candidates[:top_n])
    nodes: dict[str, dict[str, Any]] = {}
    for edge in selected:
        for side in ("source", "target"):
            node_id = str(edge.get(side) or "")
            if not node_id:
                continue
            nodes.setdefault(node_id, _caseops_vocab_tensor_codebook_node(node_id, edge))
            nodes[node_id]["weight"] = float(nodes[node_id].get("weight") or 0.0) + abs(
                float(edge.get("weight") or 0.0)
            )
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "nodes": tuple(sorted(nodes.values(), key=lambda row: (
            str(row.get("kind") or ""),
            str(row.get("id") or ""),
        ))),
        "edges": selected,
        "summary": {
            "edges": len(candidates),
            "shown": len(selected),
            "nodes": len(nodes),
            "decoded_axes": sum(
                1
                for row in candidates
                if str(row.get("relation") or "") == "codebook_axis_decode"
            ),
            "tensor_axis_edges": sum(
                1
                for row in candidates
                if str(row.get("relation") or "").startswith("codebook_tensor_")
            ),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_vocab_tensor_codebook_routes(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Collapse learned ``CTV1`` graph edges into term -> decoded-axis routes."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    edge_rows = _caseops_vocab_tensor_codebook_edge_rows(
        learner.slow_model.unigrams,
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    identity_by_tensor: dict[str, dict[str, Any]] = {}
    axis_edges_by_tensor: dict[str, list[dict[str, Any]]] = {}
    decode_by_axis: dict[str, dict[str, Any]] = {}
    score_by_edge: dict[tuple[str, str, str], tuple[float, tuple[str, ...]]] = {}
    for row in edge_rows:
        key = (
            str(row.get("source") or ""),
            str(row.get("target") or ""),
            str(row.get("relation") or ""),
        )
        score_by_edge[key] = _caseops_vocab_tensor_codebook_edge_score(
            row,
            query_terms,
        )
        relation = str(row.get("relation") or "")
        tensor_code = str(row.get("tensor_code") or "")
        axis_code = str(row.get("axis_code") or "")
        if relation == "codebook_tensor_identity" and tensor_code:
            identity_by_tensor[tensor_code] = row
        elif (
            relation.startswith("codebook_tensor_")
            and relation != "codebook_tensor_identity"
            and tensor_code
            and axis_code
        ):
            axis_edges_by_tensor.setdefault(tensor_code, []).append(row)
        elif relation == "codebook_axis_decode" and axis_code:
            decode_by_axis[axis_code] = row

    routes: list[dict[str, Any]] = []
    for tensor_code, identity in identity_by_tensor.items():
        identity_key = (
            str(identity.get("source") or ""),
            str(identity.get("target") or ""),
            str(identity.get("relation") or ""),
        )
        identity_score, identity_matched = score_by_edge.get(identity_key, (0.0, ()))
        for axis_edge in axis_edges_by_tensor.get(tensor_code, ()):
            axis_code = str(axis_edge.get("axis_code") or "")
            decode = decode_by_axis.get(axis_code)
            if not decode:
                continue
            axis_key = (
                str(axis_edge.get("source") or ""),
                str(axis_edge.get("target") or ""),
                str(axis_edge.get("relation") or ""),
            )
            decode_key = (
                str(decode.get("source") or ""),
                str(decode.get("target") or ""),
                str(decode.get("relation") or ""),
            )
            axis_score, axis_matched = score_by_edge.get(axis_key, (0.0, ()))
            decode_score, decode_matched = score_by_edge.get(decode_key, (0.0, ()))
            matched_terms = tuple(sorted({
                *identity_matched,
                *axis_matched,
                *decode_matched,
            }))
            if query_terms and not matched_terms:
                continue
            score = abs(float(identity_score or 0.0))
            score += abs(float(axis_score or 0.0))
            score += abs(float(decode_score or 0.0))
            if score < min_score:
                continue
            relation = str(axis_edge.get("relation") or "")
            polarity = (
                "positive"
                if relation == "codebook_tensor_positive_axis" else
                "negative"
                if relation == "codebook_tensor_negative_axis" else
                "neutral"
            )
            axis = str(decode.get("axis") or axis_edge.get("axis") or "")
            if not axis:
                axis = str(decode.get("target") or "").removeprefix("axis:")
            source = str(identity.get("target") or "")
            target = str(decode.get("target") or "")
            route = {
                "source": source,
                "target": target,
                "tensor_code": tensor_code,
                "axis_code": axis_code,
                "axis": axis,
                "polarity": polarity,
                "relations": (
                    "codebook_tensor_identity",
                    relation,
                    "codebook_axis_decode",
                ),
                "nodes": (
                    source,
                    str(identity.get("source") or f"tensor_code:{tensor_code}"),
                    str(axis_edge.get("target") or f"axis_code:{axis_code}"),
                    target,
                ),
                "score": score,
                "matched_terms": matched_terms,
                "weight": (
                    abs(float(identity.get("weight") or 0.0))
                    + abs(float(axis_edge.get("weight") or 0.0))
                    + abs(float(decode.get("weight") or 0.0))
                ),
                "effective_weight": (
                    abs(float(identity.get("effective_weight") or 0.0))
                    + abs(float(axis_edge.get("effective_weight") or 0.0))
                    + abs(float(decode.get("effective_weight") or 0.0))
                ),
            }
            route["route_code"] = _caseops_tensor_codebook_route_code(route)
            route["packet"] = _caseops_tensor_codebook_route_packet(route)
            routes.append(route)

    _annotate_caseops_mlp_scores(
        learner,
        query,
        routes,
        row_kind="vocab_tensor_codebook_route",
    )
    routes.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        str(row.get("source") or ""),
        str(row.get("target") or ""),
        str(row.get("tensor_code") or ""),
        str(row.get("axis_code") or ""),
    ))
    selected = tuple(routes[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "routes": selected,
        "summary": {
            "routes": len(routes),
            "shown": len(selected),
            "positive": sum(
                1 for row in routes if row.get("polarity") == "positive"
            ),
            "negative": sum(
                1 for row in routes if row.get("polarity") == "negative"
            ),
            "neutral": sum(
                1 for row in routes if row.get("polarity") == "neutral"
            ),
            "mlp_scored": sum(1 for row in routes if "mlp_score" in row),
        },
    }


def caseops_vocab_tensor_codebook_route_primer(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 5,
    axis_limit: int = 6,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Group learned ``CVR1`` route packets into compact term-axis facts."""
    top_n = max(1, int(top_n))
    axis_limit = max(1, int(axis_limit))
    recalled = caseops_vocab_tensor_codebook_routes(
        learner,
        query,
        top_n=max(top_n * axis_limit * 2, 24),
        min_score=min_score,
    )
    rows = recalled.get("routes") if isinstance(recalled, dict) else ()
    if not isinstance(rows, (list, tuple)):
        rows = ()
    groups: dict[str, dict[str, Any]] = {}
    for row in rows:
        if not isinstance(row, dict):
            continue
        source = str(row.get("source") or "")
        if not source:
            continue
        polarity = str(row.get("polarity") or "neutral")
        axis = str(row.get("axis") or "")
        if not axis:
            axis = str(row.get("target") or "").removeprefix("axis:")
        route = {
            "axis": axis,
            "target": str(row.get("target") or ""),
            "polarity": polarity,
            "tensor_code": str(row.get("tensor_code") or ""),
            "axis_code": str(row.get("axis_code") or ""),
            "route_code": str(row.get("route_code") or ""),
            "packet": str(row.get("packet") or ""),
            "score": float(row.get("score") or 0.0),
        }
        group = groups.setdefault(source, {
            "source": source,
            "score": 0.0,
            "routes": [],
            "positive_routes": [],
            "negative_routes": [],
            "neutral_routes": [],
            "packets": [],
        })
        group["score"] = float(group.get("score") or 0.0) + float(route["score"])
        group["routes"].append(route)
        group[f"{polarity}_routes"].append(route)
        if route["packet"]:
            group["packets"].append(route["packet"])

    primers: list[dict[str, Any]] = []
    for group in groups.values():
        for key in ("positive_routes", "negative_routes", "neutral_routes"):
            group[key].sort(key=lambda row: (
                -float(row.get("score") or 0.0),
                str(row.get("axis") or ""),
                str(row.get("axis_code") or ""),
            ))
            group[key] = tuple(group[key][:axis_limit])
        packets = tuple(dict.fromkeys(str(packet) for packet in group.get("packets", ())))
        positive_axes = tuple(
            str(row.get("axis") or "")
            for row in group["positive_routes"]
            if row.get("axis")
        )
        negative_axes = tuple(
            str(row.get("axis") or "")
            for row in group["negative_routes"]
            if row.get("axis")
        )
        neutral_axes = tuple(
            str(row.get("axis") or "")
            for row in group["neutral_routes"]
            if row.get("axis")
        )
        primer = (
            f"{group['source']}: "
            f"positive={','.join(positive_axes) if positive_axes else '-'} "
            f"negative={','.join(negative_axes) if negative_axes else '-'} "
            f"neutral={','.join(neutral_axes) if neutral_axes else '-'} "
            f"packets={' '.join(packets[:3]) if packets else '-'}"
        )
        primers.append({
            "source": group["source"],
            "score": float(group.get("score") or 0.0),
            "positive_axes": positive_axes,
            "negative_axes": negative_axes,
            "neutral_axes": neutral_axes,
            "positive_routes": group["positive_routes"],
            "negative_routes": group["negative_routes"],
            "neutral_routes": group["neutral_routes"],
            "packets": packets,
            "primer": primer,
        })
    primers.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        str(row.get("source") or ""),
    ))
    selected = tuple(primers[:top_n])
    return {
        "schema": "caseops_vocab_tensor_codebook_route_primer/1",
        "query": str(query or ""),
        "terms": tuple(recalled.get("terms", ()) if isinstance(recalled, dict) else ()),
        "primers": selected,
        "summary": {
            "primers": len(primers),
            "shown": len(selected),
            "routes": sum(
                len(row.get("positive_routes", ()))
                + len(row.get("negative_routes", ()))
                + len(row.get("neutral_routes", ()))
                for row in primers
            ),
            "positive_axes": sum(len(row.get("positive_axes", ())) for row in primers),
            "negative_axes": sum(len(row.get("negative_axes", ())) for row in primers),
            "neutral_axes": sum(len(row.get("neutral_axes", ())) for row in primers),
            "packets": sum(len(row.get("packets", ())) for row in primers),
        },
    }


def caseops_vocab_tensor_codebook_route_neighbors(
    learner: "MicroLearner",
    target: str = "",
    *,
    top_n: int = 8,
    min_similarity: float = 0.0,
) -> dict[str, Any]:
    """Find nearby private vocab terms by signed ``CVR1`` route geometry."""
    top_n = max(1, int(top_n))
    min_similarity = float(min_similarity)
    routes = caseops_vocab_tensor_codebook_routes(
        learner,
        "",
        top_n=max(top_n * 32, len(learner.slow_model.unigrams), 64),
    ).get("routes", ())
    if not isinstance(routes, (list, tuple)):
        routes = ()
    tensors = _caseops_vocab_tensor_codebook_route_tensors(routes)
    target_row = _caseops_vocab_tensor_codebook_route_target(tensors, target)
    if target_row is None:
        return {
            "schema": "caseops_vocab_tensor_codebook_route_neighbors/1",
            "target": str(target or ""),
            "target_route_tensor": None,
            "basis": (),
            "neighbors": (),
            "summary": {
                "candidates": 0,
                "shown": 0,
                "basis_axes": 0,
                "shared_axes": 0,
                "opposing_axes": 0,
            },
        }
    target_source = str(target_row.get("source") or "")
    target_coords = target_row.get("coordinates") or {}
    candidates: list[dict[str, Any]] = []
    for row in tensors:
        if str(row.get("source") or "") == target_source:
            continue
        coordinates = row.get("coordinates") or {}
        if not isinstance(coordinates, dict):
            continue
        similarity = _caseops_sparse_cosine(target_coords, coordinates)
        if similarity < min_similarity:
            continue
        shared_axes = _caseops_tensor_shared_axes(target_coords, coordinates)
        opposing_axes = _caseops_tensor_opposing_axes(target_coords, coordinates)
        enriched = dict(row)
        enriched["similarity"] = similarity
        enriched["shared_axes"] = shared_axes
        enriched["opposing_axes"] = opposing_axes
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        target,
        candidates,
        row_kind="vocab_tensor_codebook_route_neighbor",
        score_field="similarity",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or row.get("similarity") or 0.0),
        -float(row.get("similarity") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -len(row.get("shared_axes", ()) or ()),
        len(row.get("opposing_axes", ()) or ()),
        str(row.get("source") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    basis = tuple(sorted({
        axis
        for row in (target_row, *selected)
        for axis in (row.get("coordinates") or {}).keys()
        if str(axis).strip()
    }, key=_caseops_vocab_axis_sort_key))
    return {
        "schema": "caseops_vocab_tensor_codebook_route_neighbors/1",
        "target": str(target or ""),
        "target_route_tensor": dict(target_row),
        "basis": basis,
        "neighbors": selected,
        "summary": {
            "candidates": len(candidates),
            "shown": len(selected),
            "basis_axes": len(basis),
            "shared_axes": sum(len(row.get("shared_axes", ()) or ()) for row in candidates),
            "opposing_axes": sum(len(row.get("opposing_axes", ()) or ()) for row in candidates),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_vocab_tensor_codebook_route_transfers(
    learner: "MicroLearner",
    target: str = "",
    *,
    top_n: int = 8,
    neighbor_limit: int = 8,
    min_similarity: float = 0.0,
) -> dict[str, Any]:
    """Suggest missing ``CVR1`` axis routes a target can borrow from neighbors."""
    top_n = max(1, int(top_n))
    neighbor_limit = max(1, int(neighbor_limit))
    neighbors = caseops_vocab_tensor_codebook_route_neighbors(
        learner,
        target,
        top_n=max(neighbor_limit, top_n),
        min_similarity=min_similarity,
    )
    target_row = neighbors.get("target_route_tensor")
    if not isinstance(target_row, dict):
        return {
            "schema": "caseops_vocab_tensor_codebook_route_transfers/1",
            "target": str(target or ""),
            "target_source": "",
            "transfers": (),
            "summary": {
                "transfers": 0,
                "shown": 0,
                "neighbors": 0,
                "missing_axes": 0,
            },
        }
    target_source = str(target_row.get("source") or "")
    target_coords = target_row.get("coordinates") or {}
    if not isinstance(target_coords, dict):
        target_coords = {}
    target_tensor_code = _caseops_route_tensor_code_for_source(target_source, target_row)
    transfers: list[dict[str, Any]] = []
    for neighbor in tuple(neighbors.get("neighbors", ()) or ())[:neighbor_limit]:
        if not isinstance(neighbor, dict):
            continue
        neighbor_source = str(neighbor.get("source") or "")
        similarity = float(neighbor.get("similarity") or 0.0)
        for route in tuple(neighbor.get("routes", ()) or ()):
            if not isinstance(route, dict):
                continue
            axis = str(route.get("axis") or "")
            if not axis:
                axis = str(route.get("target") or "").removeprefix("axis:")
            if not axis or axis in target_coords:
                continue
            polarity = str(route.get("polarity") or "neutral")
            axis_code = str(route.get("axis_code") or "")
            if not axis_code:
                axis_code = _caseops_tensor_codebook_axis_code(axis)
            candidate = {
                "source": target_source,
                "target": f"axis:{axis}",
                "tensor_code": target_tensor_code,
                "axis_code": axis_code,
                "axis": axis,
                "polarity": polarity,
            }
            candidate["route_code"] = _caseops_tensor_codebook_route_code(candidate)
            candidate["packet"] = _caseops_tensor_codebook_route_packet(candidate)
            route_score = abs(float(route.get("score") or 0.0)) or 1.0
            score = similarity * route_score
            transfers.append({
                "target_source": target_source,
                "neighbor_source": neighbor_source,
                "axis": axis,
                "polarity": polarity,
                "tensor_code": target_tensor_code,
                "axis_code": axis_code,
                "route_code": candidate["route_code"],
                "packet": candidate["packet"],
                "score": score,
                "neighbor_similarity": similarity,
                "neighbor_route_code": str(route.get("route_code") or ""),
                "neighbor_packet": str(route.get("packet") or ""),
                "reason": "missing_axis",
            })
    _annotate_caseops_mlp_scores(
        learner,
        target,
        transfers,
        row_kind="vocab_tensor_codebook_route_transfer",
    )
    transfers.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        str(row.get("axis") or ""),
        str(row.get("neighbor_source") or ""),
    ))
    selected = tuple(transfers[:top_n])
    return {
        "schema": "caseops_vocab_tensor_codebook_route_transfers/1",
        "target": str(target or ""),
        "target_source": target_source,
        "target_tensor_code": target_tensor_code,
        "transfers": selected,
        "summary": {
            "transfers": len(transfers),
            "shown": len(selected),
            "neighbors": len(neighbors.get("neighbors", ()) or ()),
            "missing_axes": len({
                str(row.get("axis") or "")
                for row in transfers
                if row.get("axis")
            }),
            "mlp_scored": sum(1 for row in transfers if "mlp_score" in row),
        },
    }


def learn_caseops_vocab_tensor_codebook_route_transfers(
    learner: "MicroLearner",
    target: str = "",
    *,
    top_n: int = 8,
    neighbor_limit: int = 8,
    min_similarity: float = 0.0,
    reward: float = 0.35,
    tags: Iterable[str] = (
        "caseops",
        "caseops-vocab-codebook",
        "caseops-route-transfer",
    ),
) -> dict[str, Any]:
    """Observe suggested ``CVR1`` route transfers back into the learner."""
    if not -1.0 <= float(reward) <= 1.0:
        raise ValueError("reward must be in [-1, 1]")
    transfers = caseops_vocab_tensor_codebook_route_transfers(
        learner,
        target,
        top_n=top_n,
        neighbor_limit=neighbor_limit,
        min_similarity=min_similarity,
    )
    tag_tuple = tuple(dict.fromkeys(str(tag) for tag in tags if str(tag).strip()))
    observed = 0
    for row in transfers.get("transfers", ()) or ():
        if not isinstance(row, dict):
            continue
        packet = str(row.get("packet") or "")
        if not packet:
            continue
        response = (
            "caseops vocab tensor codebook route transfer: "
            f"{row.get('target_source')}<={row.get('axis')} "
            f"from={row.get('neighbor_source')} "
            f"polarity={row.get('polarity')} "
            f"score={float(row.get('score') or 0.0):.3f}\n"
            f"caseops vocab tensor codebook routes: {packet}"
        )
        learner.observe(
            RolloutExample(
                prompt=f"caseops route transfer learn target={target}",
                response=response,
                metadata={
                    "source": "caseops_route_transfer",
                    "target_source": str(row.get("target_source") or ""),
                    "neighbor_source": str(row.get("neighbor_source") or ""),
                    "axis": str(row.get("axis") or ""),
                },
            ),
            Feedback(
                reward=float(reward),
                notes="learned CVR1 route transfer",
                tags=tag_tuple,
            ),
        )
        observed += 1
    return {
        "schema": "caseops_vocab_tensor_codebook_route_transfer_learn/1",
        "target": str(target or ""),
        "target_source": transfers.get("target_source") or "",
        "transfers": transfers.get("transfers", ()),
        "observed": observed,
        "reward": float(reward),
        "tags": tag_tuple,
        "summary": {
            "transfers": int((transfers.get("summary") or {}).get("transfers") or 0),
            "shown": int((transfers.get("summary") or {}).get("shown") or 0),
            "observed": observed,
        },
    }


def caseops_vocab_axis_pathways(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned coactivation pathways between CaseOps vocab axes."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_vocab_axis_pathway_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_vocab_axis_pathway_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="vocab_axis_pathway",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("axis_a") or ""),
        str(row.get("axis_b") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "pathways": selected,
        "summary": {
            "pathways": len(candidates),
            "shown": len(selected),
            "repair_links": sum(
                1
                for row in candidates
                if str(row.get("axis_a") or "").startswith("repair:")
                or str(row.get("axis_b") or "").startswith("repair:")
            ),
            "learned_links": sum(
                1
                for row in candidates
                if str(row.get("axis_a") or "").startswith("learned:")
                or str(row.get("axis_b") or "").startswith("learned:")
            ),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_vocab_analogies(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned CaseOps vocab vector-analogy results and contrasts."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_vocab_analogy_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_vocab_analogy_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="vocab_analogy",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "analogies": selected,
        "summary": {
            "analogies": len(candidates),
            "shown": len(selected),
            "positive_axes": sum(len(row.get("positive_axes", ())) for row in candidates),
            "negative_axes": sum(len(row.get("negative_axes", ())) for row in candidates),
            "contrasts": sum(len(row.get("contrasts", ())) for row in candidates),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_vocab_analogy_primer(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 6,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Summarize learned CaseOps vocab analogies as compact algebra facts."""
    top_n = max(1, int(top_n))
    recalled = caseops_vocab_analogies(
        learner,
        query,
        top_n=max(top_n * 4, 12),
        min_score=min_score,
    )
    rows = recalled.get("analogies") if isinstance(recalled, dict) else ()
    if not isinstance(rows, (list, tuple)):
        rows = ()
    grouped: dict[tuple[str, str], dict[str, Any]] = {}
    for row in rows:
        if not isinstance(row, dict):
            continue
        kind = str(row.get("kind") or "").strip()
        value = str(row.get("value") or "").strip()
        if not kind or not value:
            continue
        item = grouped.setdefault(
            (kind, value),
            {
                "kind": kind,
                "value": value,
                "score": 0.0,
                "effective_weight": 0.0,
                "positive_axes": {},
                "negative_axes": {},
                "contrasts": {},
                "matched_terms": set(),
                "analogies": [],
            },
        )
        score = float(row.get("score") or 0.0)
        effective = float(
            row.get("effective_weight", row.get("weight", 0.0)) or 0.0
        )
        item["score"] = float(item["score"]) + score
        item["effective_weight"] = float(item["effective_weight"]) + effective
        for axis in tuple(row.get("positive_axes", ()) or ()):
            axis_text = str(axis)
            if axis_text:
                item["positive_axes"][axis_text] = max(
                    float(item["positive_axes"].get(axis_text, 0.0)),
                    abs(score) or abs(effective),
                )
        for axis in tuple(row.get("negative_axes", ()) or ()):
            axis_text = str(axis)
            if axis_text:
                item["negative_axes"][axis_text] = max(
                    float(item["negative_axes"].get(axis_text, 0.0)),
                    abs(score) or abs(effective),
                )
        for contrast in tuple(row.get("contrasts", ()) or ()):
            if not isinstance(contrast, dict):
                continue
            positive = str(contrast.get("positive_axis") or "")
            negative = str(contrast.get("negative_axis") or "")
            if positive and negative:
                key = f"{positive}>{negative}"
                item["contrasts"][key] = max(
                    float(item["contrasts"].get(key, 0.0)),
                    abs(float(contrast.get("effective_weight") or 0.0)),
                )
        item["matched_terms"].update(str(term) for term in row.get("matched_terms", ()))
        item["analogies"].append(dict(row))

    primers: list[dict[str, Any]] = []
    for item in grouped.values():
        positive_axes = _caseops_primer_values(item["positive_axes"])
        negative_axes = _caseops_primer_values(item["negative_axes"])
        contrasts = _caseops_primer_values(item["contrasts"])
        facts = []
        if positive_axes:
            facts.append("positive=" + ",".join(positive_axes))
        if negative_axes:
            facts.append("negative=" + ",".join(negative_axes))
        if contrasts:
            facts.append("contrast=" + ",".join(contrasts))
        primer_text = (
            f"{item['kind']}:{item['value']}: "
            + ("; ".join(facts) if facts else "analogy axes=-")
        )
        primers.append({
            "kind": item["kind"],
            "value": item["value"],
            "score": float(item["score"]),
            "effective_weight": float(item["effective_weight"]),
            "positive_axes": positive_axes,
            "negative_axes": negative_axes,
            "contrasts": contrasts,
            "matched_terms": tuple(sorted(item["matched_terms"])),
            "analogies": tuple(item["analogies"][:top_n]),
            "primer": primer_text,
        })
    primers.sort(key=lambda row: (
        -float(row["score"]),
        -abs(float(row["effective_weight"])),
        str(row["kind"]),
        str(row["value"]),
    ))
    selected = tuple(dict(row) for row in primers[:top_n])
    return {
        "query": str(query or ""),
        "terms": recalled.get("terms", ()) if isinstance(recalled, dict) else (),
        "primers": selected,
        "summary": {
            "primers": len(primers),
            "shown": len(selected),
            "analogies": len(rows),
            "positive_axes": sum(len(row["positive_axes"]) for row in primers),
            "negative_axes": sum(len(row["negative_axes"]) for row in primers),
            "contrasts": sum(len(row["contrasts"]) for row in primers),
        },
    }


def caseops_stack_code_paths(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned CaseOps fractal-stack code paths."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_stack_code_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_stack_code_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="stack_code_path",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "paths": selected,
        "summary": {
            "paths": len(candidates),
            "shown": len(selected),
            "coded": sum(1 for row in candidates if row.get("stack_codes")),
            "axis_links": sum(len(row.get("axes", ())) for row in candidates),
            "pathways": sum(len(row.get("axis_pathways", ())) for row in candidates),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_crystallized_cards(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned mathlang KnowledgeCards from crystallize output."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_crystal_card_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 8, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_crystal_card_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="crystallized_card",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("card_id") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "cards": selected,
        "summary": {
            "cards": len(candidates),
            "shown": len(selected),
            "formula_links": sum(len(row.get("formulas", ())) for row in candidates),
            "concept_links": sum(len(row.get("concepts", ())) for row in candidates),
            "stack_codes": sum(len(row.get("stack_codes", ())) for row in candidates),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_crystal_pathways(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned graph links inside crystallized KnowledgeCards."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    query_terms = _activation_terms(query)
    rows = _caseops_crystal_pathway_rows(
        learner.slow_model.unigrams,
        top_n=max(top_n * 12, len(learner.slow_model.unigrams), 64),
        penalties=learner.slow_model.penalties,
        include_penalties=True,
    )
    candidates: list[dict[str, Any]] = []
    for row in rows:
        score, matched = _caseops_crystal_pathway_score(row, query_terms)
        if query_terms and not matched:
            continue
        if score < min_score:
            continue
        enriched = dict(row)
        enriched["score"] = score
        enriched["matched_terms"] = matched
        candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="crystal_pathway",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)),
        str(row.get("card_id") or ""),
        str(row.get("target") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "pathways": selected,
        "summary": {
            "pathways": len(candidates),
            "shown": len(selected),
            "formula_links": sum(1 for row in candidates if row.get("target_kind") == "formula"),
            "concept_links": sum(1 for row in candidates if row.get("target_kind") == "concept"),
            "tag_links": sum(1 for row in candidates if row.get("target_kind") == "tag"),
            "stack_links": sum(1 for row in candidates if row.get("target_kind") == "stack"),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def caseops_crystal_primer(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 6,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Summarize crystallized KnowledgeCard graph links as compact facts."""
    top_n = max(1, int(top_n))
    recalled = caseops_crystal_pathways(
        learner,
        query,
        top_n=max(top_n * 6, 18),
        min_score=min_score,
    )
    rows = recalled.get("pathways") if isinstance(recalled, dict) else ()
    grouped: dict[str, dict[str, Any]] = {}
    if not isinstance(rows, (list, tuple)):
        rows = ()
    for row in rows:
        if not isinstance(row, dict):
            continue
        card_id = str(row.get("card_id") or "").strip()
        target_kind = str(row.get("target_kind") or "").strip()
        target_value = str(row.get("target_value") or "").strip()
        if not card_id or not target_kind or not target_value:
            continue
        item = grouped.setdefault(
            card_id,
            {
                "card_id": card_id,
                "kind": str(row.get("kind") or ""),
                "score": 0.0,
                "effective_weight": 0.0,
                "formulas": {},
                "concepts": {},
                "tags": {},
                "stacks": {},
                "suggestions": {},
                "neighbors": {},
                "pathways": [],
                "matched_terms": set(),
            },
        )
        score = float(row.get("score") or 0.0)
        effective = float(
            row.get("effective_weight", row.get("weight", 0.0)) or 0.0
        )
        item["score"] = float(item["score"]) + score
        item["effective_weight"] = float(item["effective_weight"]) + effective
        bucket_score = float(row.get("symbolic_score", row.get("score", 0.0)) or 0.0)
        bucket_name = {
            "formula": "formulas",
            "concept": "concepts",
            "tag": "tags",
            "stack": "stacks",
            "suggestion": "suggestions",
            "neighbor": "neighbors",
        }.get(target_kind)
        if bucket_name:
            bucket = item[bucket_name]
            bucket[target_value] = max(
                float(bucket.get(target_value, 0.0)),
                bucket_score,
            )
        item["pathways"].append(dict(row))
        item["matched_terms"].update(str(term) for term in row.get("matched_terms", ()))

    primers: list[dict[str, Any]] = []
    for item in grouped.values():
        formulas = _caseops_primer_values(item["formulas"])
        concepts = _caseops_primer_values(item["concepts"])
        tags = _caseops_primer_values(item["tags"])
        stacks = _caseops_primer_values(item["stacks"])
        suggestions = _caseops_primer_values(item["suggestions"])
        neighbors = _caseops_primer_values(item["neighbors"])
        facts = []
        if formulas:
            facts.append("formulas=" + ",".join(formulas))
        if concepts:
            facts.append("concepts=" + ",".join(concepts))
        if tags:
            facts.append("tags=" + ",".join(tags))
        if stacks:
            facts.append("stacks=" + ",".join(stacks))
        if suggestions:
            facts.append("suggestions=" + ",".join(suggestions))
        if neighbors:
            facts.append("neighbors=" + ",".join(neighbors))
        primer_text = (
            f"{item['kind']}:{item['card_id']}: "
            + ("; ".join(facts) if facts else "crystal links=-")
        )
        primers.append({
            "card_id": item["card_id"],
            "kind": item["kind"],
            "score": float(item["score"]),
            "effective_weight": float(item["effective_weight"]),
            "formulas": formulas,
            "concepts": concepts,
            "tags": tags,
            "stacks": stacks,
            "suggestions": suggestions,
            "neighbors": neighbors,
            "matched_terms": tuple(sorted(item["matched_terms"])),
            "pathways": tuple(item["pathways"][:top_n]),
            "primer": primer_text,
        })
    primers.sort(key=lambda row: (
        -float(row["score"]),
        -abs(float(row["effective_weight"])),
        str(row["kind"]),
        str(row["card_id"]),
    ))
    selected = tuple(dict(row) for row in primers[:top_n])
    return {
        "query": str(query or ""),
        "terms": recalled.get("terms", ()) if isinstance(recalled, dict) else (),
        "primers": selected,
        "summary": {
            "cards": len(primers),
            "shown": len(selected),
            "pathways": len(rows),
            "formulas": sum(len(row["formulas"]) for row in primers),
            "concepts": sum(len(row["concepts"]) for row in primers),
            "tags": sum(len(row["tags"]) for row in primers),
            "stacks": sum(len(row["stacks"]) for row in primers),
            "suggestions": sum(len(row["suggestions"]) for row in primers),
            "neighbors": sum(len(row["neighbors"]) for row in primers),
        },
    }


def caseops_route_motifs(
    learner: "MicroLearner",
    query: str = "",
    *,
    top_n: int = 8,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Recall learned CaseOps activation-route motifs relevant to a query."""
    top_n = max(1, int(top_n))
    min_score = float(min_score)
    unigrams = learner.slow_model.unigrams
    penalties = learner.slow_model.penalties
    query_terms = _activation_terms(query)
    candidates: list[dict[str, Any]] = []
    for motif_kind, rows in (
        (
            "pair",
            _caseops_activation_route_rows(
                unigrams,
                "caseops_activation_route_pair_",
                top_n=max(top_n * 8, len(unigrams), 64),
                penalties=penalties,
                include_penalties=True,
            ),
        ),
        (
            "step",
            _caseops_activation_route_rows(
                unigrams,
                "caseops_activation_route_step_",
                top_n=max(top_n * 8, len(unigrams), 64),
                penalties=penalties,
                include_penalties=True,
            ),
        ),
    ):
        for row in rows:
            score, matched = _caseops_route_motif_score(row, query_terms)
            if query_terms and not matched:
                continue
            if score < min_score:
                continue
            enriched = dict(row)
            enriched["motif"] = motif_kind
            enriched["score"] = score
            enriched["matched_terms"] = matched
            candidates.append(enriched)
    _annotate_caseops_mlp_scores(
        learner,
        query,
        candidates,
        row_kind="route_motif",
    )
    candidates.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -float(row.get("weight") or 0.0),
        str(row.get("motif") or ""),
        str(row.get("encoded") or ""),
    ))
    selected = tuple(dict(row) for row in candidates[:top_n])
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "routes": selected,
        "summary": {
            "routes": len(candidates),
            "shown": len(selected),
            "pairs": sum(1 for row in candidates if row.get("motif") == "pair"),
            "steps": sum(1 for row in candidates if row.get("motif") == "step"),
            "penalized": sum(1 for row in candidates if float(row.get("penalty") or 0.0) > 0.0),
            "mlp_scored": sum(1 for row in candidates if "mlp_score" in row),
        },
    }


def _caseops_route_motif_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight") or 0.0) or 0.0)
    if not query_terms:
        return (weight, ())
    text = " ".join((
        str(row.get("source") or ""),
        str(row.get("target") or ""),
        str(row.get("encoded") or ""),
        " ".join(str(node) for node in row.get("nodes", ())),
        " ".join(str(relation) for relation in row.get("relations", ())),
    ))
    row_terms = _activation_terms(text)
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    overlap = len(matched) / max(1, len(row_terms))
    exact_bonus = 0.5 if any(term in str(row.get("encoded") or "") for term in matched) else 0.0
    return (weight * (overlap + exact_bonus), matched)


_CASEOPS_SEMANTIC_LANGUAGE_KINDS = {
    "lang.argument",
    "lang.concept",
    "lang.predicate",
    "lang.role",
}


def _caseops_semantic_pathway_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    prefix = "caseops_math_language_pair_"
    tokens = {
        token for token in unigrams
        if token.startswith(prefix)
    }
    if include_penalties and penalties:
        tokens.update(
            token for token in penalties
            if token.startswith(prefix)
        )
    rows: list[dict[str, Any]] = []
    for token in tokens:
        formula_id, language_kind, language_value = _caseops_math_language_parts(
            token.removeprefix(prefix)
        )
        if language_kind not in _CASEOPS_SEMANTIC_LANGUAGE_KINDS:
            continue
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        effective_weight = weight - penalty
        rows.append({
            "value": token.removeprefix(prefix),
            "token": token,
            "formula_id": formula_id,
            "language_kind": language_kind,
            "language_value": language_value,
            "source": f"formula:{formula_id}",
            "target": f"language:{language_kind}:{language_value}",
            "relation": "caseops_semantic_language_synapse",
            "weight": weight,
            "penalty": penalty,
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["formula_id"]),
        str(row["language_kind"]),
        str(row["language_value"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_semantic_pathway_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    text = " ".join((
        str(row.get("formula_id") or ""),
        str(row.get("language_kind") or ""),
        str(row.get("language_value") or ""),
        str(row.get("source") or ""),
        str(row.get("target") or ""),
        str(row.get("relation") or ""),
    ))
    row_terms = _activation_terms(text)
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("formula_id") or ""),
        str(row.get("language_value") or ""),
    ))
    overlap = len(matched) / max(1, len(row_terms))
    exact_bonus = 0.5 if any(term in exact_text for term in matched) else 0.0
    return (weight * (overlap + exact_bonus), matched)


def _caseops_family_outcome_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    family_prefix = "caseops_family_"
    pair_prefix = "caseops_belongs_to_family_pair_"
    family_tokens = {
        token for token in unigrams
        if token.startswith(family_prefix)
    }
    if include_penalties and penalties:
        family_tokens.update(
            token for token in penalties
            if token.startswith(family_prefix)
        )
    formula_rows: dict[str, list[dict[str, Any]]] = {}
    pair_tokens = {
        token for token in unigrams
        if token.startswith(pair_prefix)
    }
    if include_penalties and penalties:
        pair_tokens.update(
            token for token in penalties
            if token.startswith(pair_prefix)
        )
    for token in pair_tokens:
        formula_id, family_id = _caseops_pair_parts(token.removeprefix(pair_prefix))
        if not formula_id or not family_id:
            continue
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        formula_rows.setdefault(family_id, []).append({
            "formula_id": formula_id,
            "token": token,
            "weight": weight,
            "penalty": penalty,
            "effective_weight": weight - penalty,
        })
    rows: list[dict[str, Any]] = []
    for token in family_tokens:
        family_id = token.removeprefix(family_prefix)
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        effective_weight = weight - penalty
        formulas = tuple(sorted(
            formula_rows.get(family_id, ()),
            key=lambda row: (
                -abs(float(row["effective_weight"])),
                str(row["formula_id"]),
            ),
        )[:8])
        rows.append({
            "family_id": family_id,
            "value": family_id,
            "token": token,
            "weight": weight,
            "penalty": penalty,
            "positive_weight": weight,
            "negative_weight": penalty,
            "effective_weight": effective_weight,
            "outcome": _caseops_outcome_label(weight, penalty),
            "formulas": formulas,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["family_id"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_outcome_label(weight: float, penalty: float) -> str:
    effective = float(weight) - float(penalty)
    if effective > 0.0 and penalty > 0.0:
        return "mixed_promote"
    if effective > 0.0:
        return "promote"
    if effective < 0.0 and weight > 0.0:
        return "mixed_avoid"
    if effective < 0.0:
        return "avoid"
    if weight > 0.0 or penalty > 0.0:
        return "mixed"
    return "neutral"


def _caseops_family_outcome_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    formulas = row.get("formulas")
    formula_text = ""
    if isinstance(formulas, (list, tuple)):
        formula_text = " ".join(
            str(item.get("formula_id") or "")
            for item in formulas
            if isinstance(item, dict)
        )
    row_terms = _activation_terms(" ".join((
        str(row.get("family_id") or ""),
        str(row.get("outcome") or ""),
        formula_text,
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    family_id = str(row.get("family_id") or "")
    exact_bonus = 0.5 if any(term in family_id for term in matched) else 0.0
    formula_bonus = 0.25 if any(term in formula_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus + formula_bonus), matched)


def _caseops_linguistic_link_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    prefix = "caseops_linguistic_pair_"
    tokens = {
        token for token in unigrams
        if token.startswith(prefix)
    }
    if include_penalties and penalties:
        tokens.update(
            token for token in penalties
            if token.startswith(prefix)
        )
    rows: list[dict[str, Any]] = []
    for token in tokens:
        kind, value, detail = _caseops_linguistic_link_parts(token.removeprefix(prefix))
        if not kind or not value:
            continue
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        effective_weight = weight - penalty
        target = f"ling.{kind}:{value}" + (f":{detail}" if detail else "")
        rows.append({
            "kind": kind,
            "value": value,
            "detail": detail,
            "token": token,
            "target": target,
            "weight": weight,
            "penalty": penalty,
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["kind"]),
        str(row["value"]),
        str(row["detail"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_linguistic_link_parts(value: str) -> tuple[str, str, str]:
    parts = [part for part in str(value or "").split("__") if part]
    if len(parts) < 2:
        return ("", "", "")
    kind = parts[0]
    link_value = parts[1]
    detail = "__".join(parts[2:]) if len(parts) > 2 else ""
    return (kind, link_value, detail)


def _caseops_linguistic_link_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    row_terms = _activation_terms(" ".join((
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
        str(row.get("detail") or ""),
        str(row.get("target") or ""),
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("value") or ""),
        str(row.get("detail") or ""),
    ))
    exact_bonus = 0.5 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def _caseops_codec_gap_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    prefix = "caseops_codec_gap_"
    tokens = {
        token for token in unigrams
        if token.startswith(prefix)
    }
    if include_penalties and penalties:
        tokens.update(
            token for token in penalties
            if token.startswith(prefix)
        )
    rows: list[dict[str, Any]] = []
    for token in tokens:
        gap = token.removeprefix(prefix)
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        effective_weight = weight - penalty
        rows.append({
            "gap": gap,
            "value": gap,
            "token": token,
            "layer": "future.codec_gap",
            "repair_query": f"future.codec_gap:{gap}",
            "weight": weight,
            "penalty": penalty,
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["gap"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_codec_gap_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    row_terms = _activation_terms(" ".join((
        str(row.get("gap") or ""),
        str(row.get("layer") or ""),
        str(row.get("repair_query") or ""),
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_bonus = 0.5 if any(term in str(row.get("gap") or "") for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def _caseops_gap_repair_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    prefix = "caseops_gap_repair_"
    tokens = {
        token for token in unigrams
        if token.startswith(prefix)
    }
    if include_penalties and penalties:
        tokens.update(
            token for token in penalties
            if token.startswith(prefix)
        )
    rows: list[dict[str, Any]] = []
    for token in tokens:
        value = token.removeprefix(prefix)
        pieces = value.split("__")
        if len(pieces) < 3:
            continue
        kind = _normalize_feature_piece(pieces[0])
        target_value = _normalize_feature_piece(pieces[1])
        reason = _normalize_feature_piece("__".join(pieces[2:]))
        target = _decode_caseops_gap_target(target_value)
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        effective_weight = weight - penalty
        rows.append({
            "kind": kind,
            "target": target,
            "target_value": target_value,
            "reason": reason,
            "token": token,
            "source": "slow",
            "sources": ("slow",),
            "support_count": 0,
            "weight": weight,
            "penalty": penalty,
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["kind"]),
        str(row["target"]),
        str(row["reason"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_gap_repair_rows_from_snippets(
    snippets: Iterable[dict[str, Any]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for item in snippets:
        if not isinstance(item, dict):
            continue
        text = str(item.get("text") or "")
        try:
            snippet_score = float(item.get("score") or 0.0)
        except Exception:  # noqa: BLE001
            snippet_score = 0.0
        try:
            uses = max(1, int(item.get("uses") or 1))
        except Exception:  # noqa: BLE001
            uses = 1
        seen: set[tuple[str, str, str, str]] = set()
        for parsed in _parse_caseops_gap_rows(text):
            key = (
                str(parsed["kind"]),
                str(parsed["target"]),
                str(parsed["reason"]),
                str(parsed.get("token") or ""),
            )
            if key in seen:
                continue
            seen.add(key)
            embedded_score = float(parsed.get("gap_score") or 0.0)
            contribution = snippet_score
            if contribution == 0.0:
                contribution = embedded_score
            elif embedded_score:
                contribution += math.copysign(abs(embedded_score) * 0.05, contribution)
            contribution += math.copysign(math.log1p(uses) * 0.05, contribution or 1.0)
            weight = contribution if contribution >= 0.0 else 0.0
            penalty = abs(contribution) if contribution < 0.0 else 0.0
            rows.append({
                **parsed,
                "source": "fast",
                "sources": ("fast",),
                "support_count": uses,
                "weight": weight,
                "penalty": penalty,
                "effective_weight": weight - penalty,
                "contribution": contribution,
            })
    rows.sort(key=lambda row: (
        -abs(float(row.get("effective_weight") or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("target") or ""),
        str(row.get("reason") or ""),
    ))
    return rows


def _parse_caseops_gap_rows(text: str) -> tuple[dict[str, Any], ...]:
    rows: list[dict[str, Any]] = []
    for match in _CASEOPS_GAP_RE.finditer(str(text or "")):
        kind = _normalize_feature_piece(match.group(1))
        target = str(match.group(2) or "").strip().lower()
        reason = _normalize_feature_piece(match.group(3))
        try:
            gap_score = float(match.group(4) or 0.0)
        except Exception:  # noqa: BLE001
            gap_score = 0.0
        source_token = str(match.group(5) or "").strip()
        target_value = _normalize_feature_piece(target)
        rows.append({
            "kind": kind,
            "target": target,
            "target_value": target_value,
            "gap": _caseops_gap_term(target),
            "reason": reason,
            "token": source_token,
            "gap_score": gap_score,
        })
    return tuple(rows)


def _merge_caseops_gap_repair_rows(
    rows: Iterable[dict[str, Any]],
) -> list[dict[str, Any]]:
    merged: dict[tuple[str, str, str], dict[str, Any]] = {}
    for row in rows:
        kind = str(row.get("kind") or "target")
        target = str(row.get("target") or "")
        reason = str(row.get("reason") or "unresolved")
        if not target:
            continue
        key = (kind, target, reason)
        item = merged.setdefault(
            key,
            {
                "kind": kind,
                "target": target,
                "target_value": _normalize_feature_piece(target),
                "gap": _caseops_gap_term(target),
                "reason": reason,
                "token": "",
                "tokens": set(),
                "sources": set(),
                "support_count": 0,
                "gap_score": 0.0,
                "weight": 0.0,
                "penalty": 0.0,
                "effective_weight": 0.0,
                "contribution": 0.0,
            },
        )
        token = str(row.get("token") or "")
        if token:
            item["token"] = item["token"] or token
            tokens = item["tokens"]
            if isinstance(tokens, set):
                tokens.add(token)
        sources = item["sources"]
        if isinstance(sources, set):
            sources.update(str(source) for source in row.get("sources", ()) or ())
            source = str(row.get("source") or "")
            if source:
                sources.add(source)
        item["support_count"] = int(item["support_count"]) + int(row.get("support_count") or 0)
        item["gap_score"] = max(
            float(item["gap_score"]),
            abs(float(row.get("gap_score") or 0.0)),
        )
        item["weight"] = float(item["weight"]) + float(row.get("weight") or 0.0)
        item["penalty"] = float(item["penalty"]) + float(row.get("penalty") or 0.0)
        item["effective_weight"] = float(item["weight"]) - float(item["penalty"])
        item["contribution"] = float(item["contribution"]) + float(row.get("contribution") or 0.0)
    out: list[dict[str, Any]] = []
    for item in merged.values():
        tokens = item.get("tokens")
        sources = item.get("sources")
        item["tokens"] = tuple(sorted(tokens)) if isinstance(tokens, set) else ()
        item["sources"] = tuple(sorted(sources)) if isinstance(sources, set) else ()
        out.append(item)
    out.sort(key=lambda row: (
        -abs(float(row.get("effective_weight") or 0.0)),
        -float(row.get("weight") or 0.0),
        str(row.get("kind") or ""),
        str(row.get("target") or ""),
    ))
    return out


def _caseops_gap_repair_enriched_row(
    row: dict[str, Any],
    *,
    formula_store: str | Path | None,
    hint_limit: int,
) -> dict[str, Any]:
    target = str(row.get("target") or "")
    reason = str(row.get("reason") or "")
    formulas = _caseops_gap_formula_matches(
        target,
        formula_store=formula_store,
        hint_limit=hint_limit,
    )
    status, layer, repair_query = _caseops_gap_repair_action(
        target,
        reason,
        formulas=formulas,
    )
    enriched = dict(row)
    enriched.update({
        "status": status,
        "layer": layer,
        "repair_query": repair_query,
        "formulas": formulas,
    })
    return enriched


def _caseops_gap_formula_matches(
    target: str,
    *,
    formula_store: str | Path | None,
    hint_limit: int,
) -> tuple[dict[str, Any], ...]:
    query = _caseops_gap_formula_query(target)
    if not query:
        return ()
    try:
        from orchestrator.math_language import formula_catalog
        from orchestrator.math_language import load_formula_jsonl
        from orchestrator.math_language import search_formulas

        formulas = (
            formula_catalog()
            if formula_store is None
            else formula_catalog(load_formula_jsonl(formula_store))
        )
        matches = search_formulas(query, formulas=formulas, limit=hint_limit)
    except Exception:  # noqa: BLE001
        return ()
    return tuple(
        {
            "id": formula.id,
            "name": formula.name,
            "family": formula.family,
            "score": float(score),
            "concepts": formula.concepts[:4],
        }
        for score, formula in matches
    )


def _caseops_gap_repair_action(
    target: str,
    reason: str,
    *,
    formulas: tuple[dict[str, Any], ...],
) -> tuple[str, str, str]:
    if formulas:
        repair = " ".join(
            str(row.get("id") or "")
            for row in formulas[:3]
            if isinstance(row, dict)
        ).strip()
        return ("formula_anchor", "math.formula", repair)
    term = _caseops_gap_term(target)
    if reason == "formula_not_in_catalog":
        return ("catalog_formula", "mathlang.formula_catalog", f"mathlang.formula:{target}")
    if reason == "no_formula_match":
        if target.startswith("ling."):
            return ("mathlang_alias", "mathlang.linguistic_alias", f"mathlang.alias:{target}")
        if "->lang." in target:
            return ("mathlang_pathway", "mathlang.semantic_pathway", f"mathlang.pathway:{target}")
        if target.startswith("family:"):
            return ("mathlang_family", "mathlang.family", f"mathlang.family:{target}")
        return ("open_layer", "future.codec_gap", f"future.codec_gap:{term}")
    return ("caseops_codec_rule", "caseops.codec", f"caseops.codec_rule:{target}")


def _caseops_gap_repair_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    formula_ids = " ".join(
        str(item.get("id") or "")
        for item in row.get("formulas", ()) or ()
        if isinstance(item, dict)
    )
    row_terms = _activation_terms(" ".join((
        str(row.get("kind") or ""),
        str(row.get("target") or ""),
        str(row.get("target_value") or ""),
        str(row.get("gap") or ""),
        str(row.get("reason") or ""),
        str(row.get("status") or ""),
        str(row.get("layer") or ""),
        str(row.get("repair_query") or ""),
        formula_ids,
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("target") or ""),
        str(row.get("gap") or ""),
        str(row.get("repair_query") or ""),
    ))
    exact_bonus = 0.6 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def _caseops_vocab_vector_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    pair_prefix = "caseops_vocab_pair_"
    axis_prefix = "caseops_vocab_axis_pair_"
    pair_tokens = {
        token for token in unigrams
        if token.startswith(pair_prefix)
    }
    if include_penalties and penalties:
        pair_tokens.update(
            token for token in penalties
            if token.startswith(pair_prefix)
        )
    axis_tokens = {
        token for token in unigrams
        if token.startswith(axis_prefix)
    }
    if include_penalties and penalties:
        axis_tokens.update(
            token for token in penalties
            if token.startswith(axis_prefix)
        )
    axes_by_pair: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for token in axis_tokens:
        value = token.removeprefix(axis_prefix)
        pieces = value.split("__")
        if len(pieces) < 3:
            continue
        kind = _normalize_feature_piece(pieces[0])
        vocab_value = _normalize_feature_piece(pieces[1])
        axis = _decode_caseops_vocab_axis("__".join(pieces[2:]))
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        axes_by_pair.setdefault((kind, vocab_value), []).append({
            "axis": axis,
            "axis_value": "__".join(pieces[2:]),
            "axis_root": axis.split(":", 1)[0] if ":" in axis else axis,
            "token": token,
            "weight": weight,
            "penalty": penalty,
            "effective_weight": weight - penalty,
        })

    rows: list[dict[str, Any]] = []
    for token in pair_tokens:
        value = token.removeprefix(pair_prefix)
        pieces = value.split("__")
        if len(pieces) < 2:
            continue
        kind = _normalize_feature_piece(pieces[0])
        vocab_value = _normalize_feature_piece("__".join(pieces[1:]))
        axis_rows = sorted(
            axes_by_pair.get((kind, vocab_value), ()),
            key=lambda row: (
                -abs(float(row.get("effective_weight") or 0.0)),
                str(row.get("axis") or ""),
            ),
        )
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        axis_effective = sum(float(row.get("effective_weight") or 0.0) for row in axis_rows)
        effective_weight = weight - penalty + axis_effective * 0.25
        rows.append({
            "kind": _decode_caseops_vocab_kind(kind),
            "kind_value": kind,
            "value": _decode_caseops_vocab_value(vocab_value),
            "value_key": vocab_value,
            "token": token,
            "axes": tuple(str(row["axis"]) for row in axis_rows),
            "axis_rows": tuple(dict(row) for row in axis_rows[:8]),
            "axis_roots": tuple(sorted({
                str(row.get("axis_root") or "")
                for row in axis_rows
                if str(row.get("axis_root") or "")
            })),
            "weight": weight,
            "penalty": penalty,
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["kind"]),
        str(row["value"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_vocab_axis_pathway_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    prefix = "caseops_vocab_axis_pathway_"
    tokens = {
        token for token in unigrams
        if token.startswith(prefix)
        and not token.startswith("caseops_vocab_axis_pathway_root_")
    }
    if include_penalties and penalties:
        tokens.update(
            token for token in penalties
            if token.startswith(prefix)
            and not token.startswith("caseops_vocab_axis_pathway_root_")
        )
    rows: list[dict[str, Any]] = []
    for token in tokens:
        value = token.removeprefix(prefix)
        pieces = value.split("__")
        if len(pieces) != 2:
            continue
        axis_a = _decode_caseops_vocab_axis(pieces[0])
        axis_b = _decode_caseops_vocab_axis(pieces[1])
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        effective_weight = weight - penalty
        rows.append({
            "value": value,
            "token": token,
            "axis_a": axis_a,
            "axis_b": axis_b,
            "axis_roots": tuple(sorted({
                axis_a.split(":", 1)[0] if ":" in axis_a else axis_a,
                axis_b.split(":", 1)[0] if ":" in axis_b else axis_b,
            })),
            "source": f"axis:{axis_a}",
            "target": f"axis:{axis_b}",
            "relation": "caseops_vocab_axis_synapse",
            "weight": weight,
            "penalty": penalty,
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["axis_a"]),
        str(row["axis_b"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_vocab_axis_pathway_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    row_terms = _activation_terms(" ".join((
        str(row.get("axis_a") or ""),
        str(row.get("axis_b") or ""),
        str(row.get("value") or ""),
        " ".join(str(root) for root in row.get("axis_roots", ())),
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("axis_a") or ""),
        str(row.get("axis_b") or ""),
    ))
    exact_bonus = 0.5 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def _caseops_vocab_tensor_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    vector_rows = _caseops_vocab_vector_rows(
        unigrams,
        top_n=top_n,
        penalties=penalties,
        include_penalties=include_penalties,
    )
    analogy_rows = _caseops_vocab_analogy_rows(
        unigrams,
        top_n=top_n,
        penalties=penalties,
        include_penalties=include_penalties,
    )
    negative_axes_by_key: dict[tuple[str, str], set[str]] = {}
    for row in analogy_rows:
        key = (
            _normalize_feature_piece(str(row.get("kind_value") or row.get("kind") or "")),
            _normalize_feature_piece(str(row.get("value_key") or row.get("value") or "")),
        )
        if not key[0] or not key[1]:
            continue
        negative_axes_by_key.setdefault(key, set()).update(
            str(axis)
            for axis in row.get("negative_axes", ()) or ()
            if str(axis).strip()
        )
    grouped: dict[tuple[str, str], dict[str, Any]] = {}
    for row in vector_rows:
        key = (
            _normalize_feature_piece(str(row.get("kind_value") or row.get("kind") or "")),
            _normalize_feature_piece(str(row.get("value_key") or row.get("value") or "")),
        )
        if not key[0] or not key[1]:
            continue
        item = grouped.setdefault(key, _empty_vocab_tensor_row(row))
        item["weight"] = float(item["weight"]) + float(row.get("weight") or 0.0)
        item["penalty"] = float(item["penalty"]) + float(row.get("penalty") or 0.0)
        item["effective_weight"] = (
            float(item["effective_weight"])
            + float(row.get("effective_weight") or 0.0)
        )
        item["source_vectors"].append(dict(row))
        for axis_row in tuple(row.get("axis_rows", ()) or ()):
            if not isinstance(axis_row, dict):
                continue
            axis = str(axis_row.get("axis") or "")
            if not axis:
                continue
            if axis in negative_axes_by_key.get(key, set()):
                continue
            delta = float(
                axis_row.get("effective_weight", axis_row.get("weight", 0.0))
                or 0.0
            )
            if delta == 0.0:
                delta = 1.0
            _add_vocab_tensor_coordinate(item, axis, delta)
    for row in analogy_rows:
        key = (
            _normalize_feature_piece(str(row.get("kind_value") or row.get("kind") or "")),
            _normalize_feature_piece(str(row.get("value_key") or row.get("value") or "")),
        )
        if not key[0] or not key[1]:
            continue
        item = grouped.setdefault(key, _empty_vocab_tensor_row(row))
        item["weight"] = float(item["weight"]) + float(row.get("weight") or 0.0)
        item["penalty"] = float(item["penalty"]) + float(row.get("penalty") or 0.0)
        item["effective_weight"] = (
            float(item["effective_weight"])
            + float(row.get("effective_weight") or 0.0)
        )
        item["analogies"].append(dict(row))
        default_delta = abs(float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0))
        if default_delta == 0.0:
            default_delta = 1.0
        positive_rows = row.get("positive_axis_rows", ()) or ()
        negative_rows = row.get("negative_axis_rows", ()) or ()
        for axis_row in tuple(positive_rows):
            if not isinstance(axis_row, dict):
                continue
            axis = str(axis_row.get("axis") or "")
            if axis:
                delta = abs(float(axis_row.get("effective_weight") or default_delta))
                _add_vocab_tensor_coordinate(item, axis, delta or default_delta)
        for axis_row in tuple(negative_rows):
            if not isinstance(axis_row, dict):
                continue
            axis = str(axis_row.get("axis") or "")
            if axis:
                delta = abs(float(axis_row.get("effective_weight") or default_delta))
                _add_vocab_tensor_coordinate(item, axis, -(delta or default_delta))
    rows = [_finalize_vocab_tensor_row(row) for row in grouped.values()]
    rows.sort(key=lambda row: (
        -float(row.get("magnitude") or 0.0),
        -abs(float(row.get("effective_weight") or 0.0)),
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
    ))
    return rows[: max(1, int(top_n))]


def _empty_vocab_tensor_row(seed: dict[str, Any]) -> dict[str, Any]:
    kind_value = _normalize_feature_piece(
        str(seed.get("kind_value") or seed.get("kind") or "")
    )
    value_key = _normalize_feature_piece(
        str(seed.get("value_key") or seed.get("value") or "")
    )
    return {
        "kind": str(seed.get("kind") or _decode_caseops_vocab_kind(kind_value)),
        "kind_value": kind_value,
        "value": str(seed.get("value") or _decode_caseops_vocab_value(value_key)),
        "value_key": value_key,
        "coordinates": {},
        "axis_sources": {},
        "source_vectors": [],
        "analogies": [],
        "weight": 0.0,
        "penalty": 0.0,
        "effective_weight": 0.0,
    }


def _add_vocab_tensor_coordinate(
    row: dict[str, Any],
    axis: str,
    delta: float,
) -> None:
    clean_axis = _decode_caseops_vocab_axis(_normalize_feature_piece(axis))
    if not clean_axis:
        return
    coordinates = row.setdefault("coordinates", {})
    coordinates[clean_axis] = float(coordinates.get(clean_axis, 0.0)) + float(delta)
    sources = row.setdefault("axis_sources", {})
    sources[clean_axis] = int(sources.get(clean_axis, 0)) + 1


def _finalize_vocab_tensor_row(row: dict[str, Any]) -> dict[str, Any]:
    coordinates = {
        str(axis): float(value)
        for axis, value in (row.get("coordinates") or {}).items()
        if str(axis).strip() and float(value) != 0.0
    }
    ordered = dict(sorted(coordinates.items(), key=lambda item: (
        _caseops_vocab_axis_sort_key(item[0]),
        item[0],
    )))
    positive_axes = tuple(axis for axis, value in ordered.items() if value > 0.0)
    negative_axes = tuple(axis for axis, value in ordered.items() if value < 0.0)
    axis_roots = tuple(sorted({
        axis.split(":", 1)[0] if ":" in axis else axis
        for axis in ordered
    }))
    magnitude = math.sqrt(sum(value * value for value in ordered.values()))
    out = dict(row)
    out["coordinates"] = ordered
    out["positive_axes"] = positive_axes
    out["negative_axes"] = negative_axes
    out["axis_roots"] = axis_roots
    out["dimensions"] = len(ordered)
    out["magnitude"] = magnitude
    out["contribution"] = magnitude + abs(float(row.get("effective_weight") or 0.0))
    out["source_vectors"] = tuple(dict(item) for item in row.get("source_vectors", ())[:8])
    out["analogies"] = tuple(dict(item) for item in row.get("analogies", ())[:8])
    out["encoding"] = "caseops_vocab_tensor/1"
    return out


def _caseops_vocab_tensor_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    base = float(row.get("contribution", row.get("magnitude", 0.0)) or 0.0)
    if not query_terms:
        return (base, ())
    row_terms = _activation_terms(" ".join((
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
        " ".join(str(axis) for axis in (row.get("coordinates") or {}).keys()),
        " ".join(str(root) for root in row.get("axis_roots", ())),
        "tensor multidimensional encoding coordinate axis",
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
        " ".join(str(axis) for axis in (row.get("coordinates") or {}).keys()),
    ))
    exact_bonus = 0.5 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (base * (overlap + exact_bonus), matched)


def _caseops_vocab_tensor_target(
    rows: list[dict[str, Any]],
    target: str,
) -> dict[str, Any] | None:
    clean = str(target or "").strip().lower()
    if not clean:
        return rows[0] if rows else None
    clean_key = clean.replace(" ", "_")
    for row in rows:
        kind = str(row.get("kind") or "").lower()
        value = str(row.get("value") or "").lower()
        if clean in {value, f"{kind}:{value}"}:
            return row
        if clean_key in {
            str(row.get("value_key") or ""),
            f"{row.get('kind_value')}:{row.get('value_key')}",
        }:
            return row
    query_terms = _activation_terms(clean)
    scored: list[tuple[float, dict[str, Any]]] = []
    for row in rows:
        score, matched = _caseops_vocab_tensor_score(row, query_terms)
        if matched:
            scored.append((score, row))
    scored.sort(key=lambda item: (
        -item[0],
        str(item[1].get("kind") or ""),
        str(item[1].get("value") or ""),
    ))
    return scored[0][1] if scored else None


def _caseops_vocab_tensor_key(row: dict[str, Any]) -> tuple[str, str]:
    return (
        _normalize_feature_piece(str(row.get("kind_value") or row.get("kind") or "")),
        _normalize_feature_piece(str(row.get("value_key") or row.get("value") or "")),
    )


def _caseops_vocab_tensor_codebook_route_tensors(
    routes: Iterable[dict[str, Any]],
) -> list[dict[str, Any]]:
    groups: dict[str, dict[str, Any]] = {}
    for route in routes:
        if not isinstance(route, dict):
            continue
        source = str(route.get("source") or "")
        axis = str(route.get("axis") or "")
        if not axis:
            axis = str(route.get("target") or "").removeprefix("axis:")
        if not source or not axis:
            continue
        polarity = str(route.get("polarity") or "neutral")
        score = abs(float(route.get("score") or route.get("effective_weight") or 0.0))
        if score == 0.0:
            score = 1.0
        sign = -1.0 if polarity == "negative" else 0.25 if polarity == "neutral" else 1.0
        group = groups.setdefault(source, {
            "source": source,
            "coordinates": {},
            "positive_axes": [],
            "negative_axes": [],
            "neutral_axes": [],
            "routes": [],
            "packets": [],
            "score": 0.0,
        })
        coordinates = group["coordinates"]
        coordinates[axis] = float(coordinates.get(axis, 0.0)) + sign * score
        group["score"] = float(group.get("score") or 0.0) + score
        if polarity == "positive":
            group["positive_axes"].append(axis)
        elif polarity == "negative":
            group["negative_axes"].append(axis)
        else:
            group["neutral_axes"].append(axis)
        group["routes"].append({
            "axis": axis,
            "target": str(route.get("target") or ""),
            "polarity": polarity,
            "tensor_code": str(route.get("tensor_code") or ""),
            "axis_code": str(route.get("axis_code") or ""),
            "route_code": str(route.get("route_code") or ""),
            "packet": str(route.get("packet") or ""),
            "score": float(route.get("score") or 0.0),
        })
        if route.get("packet"):
            group["packets"].append(str(route.get("packet")))
    out: list[dict[str, Any]] = []
    for group in groups.values():
        coordinates = {
            str(axis): float(value)
            for axis, value in (group.get("coordinates") or {}).items()
            if str(axis).strip() and float(value) != 0.0
        }
        if not coordinates:
            continue
        out.append({
            "source": str(group.get("source") or ""),
            "coordinates": coordinates,
            "positive_axes": tuple(sorted(
                dict.fromkeys(str(axis) for axis in group.get("positive_axes", ())),
                key=_caseops_vocab_axis_sort_key,
            )),
            "negative_axes": tuple(sorted(
                dict.fromkeys(str(axis) for axis in group.get("negative_axes", ())),
                key=_caseops_vocab_axis_sort_key,
            )),
            "neutral_axes": tuple(sorted(
                dict.fromkeys(str(axis) for axis in group.get("neutral_axes", ())),
                key=_caseops_vocab_axis_sort_key,
            )),
            "routes": tuple(group.get("routes", ())),
            "packets": tuple(dict.fromkeys(str(packet) for packet in group.get("packets", ()))),
            "score": float(group.get("score") or 0.0),
            "dimensions": len(coordinates),
            "magnitude": math.sqrt(sum(value * value for value in coordinates.values())),
        })
    out.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        str(row.get("source") or ""),
    ))
    return out


def _caseops_vocab_tensor_codebook_route_target(
    rows: list[dict[str, Any]],
    target: str,
) -> dict[str, Any] | None:
    clean = str(target or "").strip().lower()
    if not clean:
        return rows[0] if rows else None
    clean_key = _normalize_feature_piece(clean)
    for row in rows:
        source = str(row.get("source") or "").lower()
        if clean == source or clean_key == _normalize_feature_piece(source):
            return row
        if source.startswith("term:"):
            term = source.removeprefix("term:")
            pieces = term.split(":")
            value = pieces[-1] if pieces else term
            if clean in {term, value}:
                return row
            if clean_key in {
                _normalize_feature_piece(term),
                _normalize_feature_piece(value),
            }:
                return row
    query_terms = _activation_terms(clean)
    scored: list[tuple[float, dict[str, Any]]] = []
    for row in rows:
        row_terms = _activation_terms(" ".join((
            str(row.get("source") or ""),
            " ".join(str(axis) for axis in (row.get("coordinates") or {}).keys()),
            " ".join(str(packet) for packet in row.get("packets", ())),
        )))
        matched = query_terms.intersection(row_terms)
        if not matched:
            continue
        score = len(matched) + float(row.get("score") or 0.0) * 0.001
        scored.append((score, row))
    scored.sort(key=lambda item: (
        -item[0],
        str(item[1].get("source") or ""),
    ))
    return scored[0][1] if scored else None


def _caseops_route_tensor_code_for_source(
    source: str,
    row: dict[str, Any],
) -> str:
    for route in tuple(row.get("routes", ()) or ()):
        if isinstance(route, dict) and str(route.get("tensor_code") or ""):
            return str(route.get("tensor_code") or "")
    if source.startswith("term:"):
        term = source.removeprefix("term:")
        kind, sep, value = term.partition(":")
        if sep and kind and value:
            return _caseops_tensor_codebook_tensor_code(kind, value)
    digest = hashlib.sha1(str(source or "route").encode("utf-8")).hexdigest()[:10]
    return f"tv.route.{digest}"


def _caseops_vocab_tensor_codebook_route_axis_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for polarity in ("positive", "negative", "neutral"):
        prefix = f"caseops_vocab_tensor_codebook_route_{polarity}_axis_pair_"
        for token in _caseops_weight_tokens(
            unigrams,
            penalties,
            prefix,
            include_penalties,
        ):
            value = token.removeprefix(prefix)
            pieces = value.split("__")
            if len(pieces) < 3:
                continue
            kind = _decode_caseops_vocab_kind(pieces[0])
            vocab_value = _decode_caseops_vocab_value(pieces[1])
            axis = _decode_caseops_vocab_axis("__".join(pieces[2:]))
            source = f"term:{kind}:{vocab_value}"
            target = f"axis:{axis}"
            fields = _caseops_token_weight_fields(token, unigrams, penalties)
            rows.append({
                "kind": kind,
                "value": vocab_value,
                "axis": axis,
                "polarity": polarity,
                "source": source,
                "target": target,
                "route": f"{source}<={axis}",
                "axis_code": _caseops_tensor_codebook_axis_code(axis),
                "token": token,
                **fields,
            })
    rows.sort(key=lambda row: (
        -abs(float(row.get("effective_weight") or 0.0)),
        -float(row.get("weight") or 0.0),
        str(row.get("source") or ""),
        str(row.get("axis") or ""),
        str(row.get("polarity") or ""),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_tensor_codebook_axis_code(axis: str) -> str:
    root = _normalize_feature_piece(str(axis).split(":", 1)[0])[:8] or "axis"
    digest = hashlib.sha1(str(axis).encode("utf-8")).hexdigest()[:8]
    return f"ax.{root}.{digest}"


def _caseops_tensor_codebook_tensor_code(kind: str, value: str) -> str:
    clean_kind = _normalize_feature_piece(kind).replace("_", "")[:8] or "term"
    digest = hashlib.sha1(f"{kind}:{value}".encode("utf-8")).hexdigest()[:10]
    return f"tv.{clean_kind}.{digest}"


def _caseops_tensor_codebook_route_code(route: dict[str, Any]) -> str:
    polarity = _normalize_feature_piece(str(route.get("polarity") or "route"))[:3]
    digest = hashlib.sha1("|".join((
        str(route.get("source") or ""),
        str(route.get("target") or ""),
        str(route.get("tensor_code") or ""),
        str(route.get("axis_code") or ""),
        str(route.get("polarity") or ""),
    )).encode("utf-8")).hexdigest()[:10]
    return f"rv.{polarity or 'rt'}.{digest}"


def _caseops_tensor_codebook_route_packet(route: dict[str, Any]) -> str:
    route_code = _caseops_codebook_packet_piece(route.get("route_code") or "")
    tensor_code = _caseops_codebook_packet_piece(route.get("tensor_code") or "")
    axis_code = _caseops_codebook_packet_piece(route.get("axis_code") or "")
    polarity = _caseops_codebook_packet_piece(route.get("polarity") or "neutral")
    source = _caseops_codebook_packet_piece(route.get("source") or "")
    target = _caseops_codebook_packet_piece(route.get("target") or "")
    if not all((route_code, tensor_code, axis_code, polarity, source, target)):
        return ""
    return f"CVR1|{route_code}|{tensor_code}|{axis_code}|{polarity}|{source}|{target}"


def _caseops_codebook_packet_piece(value: Any) -> str:
    return re.sub(r"[^a-z0-9_.:-]+", "_", str(value).lower()).strip("_")


def _caseops_tensor_codebook_packet(
    tensor_code: str,
    kind: str,
    value: str,
    coded_coordinates: dict[str, float],
) -> str:
    coord_text = ",".join(
        f"{axis_code}:{float(weight):+.3f}"
        for axis_code, weight in coded_coordinates.items()
    )
    return f"CTV1|{tensor_code}|{kind}:{value}|{coord_text}"


def _caseops_vocab_tensor_codebook_edge_rows(
    unigrams: dict[str, float],
    *,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    code_pair_prefix = "caseops_vocab_tensor_codebook_code_pair_"
    coord_prefix = "caseops_vocab_tensor_codebook_coordinate_pair_"
    axis_decode_prefix = "caseops_vocab_tensor_codebook_axis_decode_"
    positive_prefix = "caseops_vocab_tensor_codebook_positive_axis_"
    negative_prefix = "caseops_vocab_tensor_codebook_negative_axis_"
    token_keys = set(unigrams)
    if include_penalties and penalties:
        token_keys.update(penalties)

    tensor_identity: dict[str, dict[str, Any]] = {}
    for token in token_keys:
        if not token.startswith(code_pair_prefix):
            continue
        value = token.removeprefix(code_pair_prefix)
        pieces = value.split("__")
        if len(pieces) < 3:
            continue
        tensor_code = "__".join(pieces[:-2])
        kind_key = pieces[-2]
        value_key = pieces[-1]
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        tensor_identity[tensor_code] = {
            "tensor_code": tensor_code,
            "kind": _decode_caseops_vocab_kind(kind_key),
            "value": _decode_caseops_vocab_value(value_key),
            "weight": weight,
            "penalty": penalty,
            "effective_weight": weight - penalty,
            "token": token,
        }

    axis_decodes: dict[str, dict[str, Any]] = {}
    for token in token_keys:
        if not token.startswith(axis_decode_prefix):
            continue
        value = token.removeprefix(axis_decode_prefix)
        axis_code, sep, axis_value = value.partition("__")
        if not sep or not axis_code or not axis_value:
            continue
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        axis_decodes[axis_code] = {
            "axis_code": axis_code,
            "axis": _decode_caseops_vocab_axis(axis_value),
            "weight": weight,
            "penalty": penalty,
            "effective_weight": weight - penalty,
            "token": token,
        }

    axis_polarity: dict[str, dict[str, float]] = {}
    for token in token_keys:
        role = ""
        value = ""
        if token.startswith(positive_prefix):
            role = "positive"
            value = token.removeprefix(positive_prefix)
        elif token.startswith(negative_prefix):
            role = "negative"
            value = token.removeprefix(negative_prefix)
        if not role or "__" in value:
            continue
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        axis_polarity.setdefault(value, {"positive": 0.0, "negative": 0.0})[role] += (
            weight - penalty
        )

    rows: list[dict[str, Any]] = []
    for tensor_code, identity in tensor_identity.items():
        effective = float(identity.get("effective_weight") or 0.0)
        rows.append({
            "source": f"tensor_code:{tensor_code}",
            "target": f"term:{identity.get('kind')}:{identity.get('value')}",
            "relation": "codebook_tensor_identity",
            "tensor_code": tensor_code,
            "kind": identity.get("kind"),
            "value": identity.get("value"),
            "weight": float(identity.get("weight") or 0.0),
            "penalty": float(identity.get("penalty") or 0.0),
            "effective_weight": effective,
        })
    for token in token_keys:
        if not token.startswith(coord_prefix):
            continue
        value = token.removeprefix(coord_prefix)
        tensor_code, sep, axis_code = value.partition("__")
        if not sep or not tensor_code or not axis_code:
            continue
        identity = tensor_identity.get(tensor_code, {})
        decode = axis_decodes.get(axis_code, {})
        polarity = axis_polarity.get(axis_code, {})
        positive = float(polarity.get("positive") or 0.0)
        negative = float(polarity.get("negative") or 0.0)
        relation = (
            "codebook_tensor_negative_axis"
            if negative > positive else
            "codebook_tensor_positive_axis"
            if positive > negative else
            "codebook_tensor_axis"
        )
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        rows.append({
            "source": f"tensor_code:{tensor_code}",
            "target": f"axis_code:{axis_code}",
            "relation": relation,
            "tensor_code": tensor_code,
            "axis_code": axis_code,
            "axis": decode.get("axis"),
            "kind": identity.get("kind"),
            "value": identity.get("value"),
            "weight": weight,
            "penalty": penalty,
            "effective_weight": weight - penalty,
        })
    for axis_code, decode in axis_decodes.items():
        rows.append({
            "source": f"axis_code:{axis_code}",
            "target": f"axis:{decode.get('axis')}",
            "relation": "codebook_axis_decode",
            "axis_code": axis_code,
            "axis": decode.get("axis"),
            "weight": float(decode.get("weight") or 0.0),
            "penalty": float(decode.get("penalty") or 0.0),
            "effective_weight": float(decode.get("effective_weight") or 0.0),
        })
    rows.sort(key=lambda row: (
        -abs(float(row.get("effective_weight") or 0.0)),
        str(row.get("source") or ""),
        str(row.get("target") or ""),
    ))
    return rows


def _caseops_vocab_tensor_codebook_edge_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    base = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (base, ())
    row_terms = _activation_terms(" ".join((
        str(row.get("source") or ""),
        str(row.get("target") or ""),
        str(row.get("relation") or ""),
        str(row.get("tensor_code") or ""),
        str(row.get("axis_code") or ""),
        str(row.get("axis") or ""),
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
        "codebook tensor ctv1 private symbol axis decode",
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("source") or ""),
        str(row.get("target") or ""),
        str(row.get("axis") or ""),
        str(row.get("value") or ""),
    ))
    exact_bonus = 0.5 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (base * (overlap + exact_bonus), matched)


def _caseops_vocab_tensor_codebook_node(
    node_id: str,
    edge: dict[str, Any],
) -> dict[str, Any]:
    prefix, _, value = node_id.partition(":")
    if prefix == "tensor_code":
        return {
            "id": node_id,
            "kind": "tensor_code",
            "label": value,
            "weight": 0.0,
        }
    if prefix == "axis_code":
        return {
            "id": node_id,
            "kind": "axis_code",
            "label": value,
            "axis": edge.get("axis"),
            "weight": 0.0,
        }
    if prefix == "axis":
        return {
            "id": node_id,
            "kind": "axis",
            "label": value,
            "weight": 0.0,
        }
    if prefix == "term":
        return {
            "id": node_id,
            "kind": "term",
            "label": value,
            "weight": 0.0,
        }
    return {
        "id": node_id,
        "kind": prefix or "unknown",
        "label": value or node_id,
        "weight": 0.0,
    }


def _is_number_like(value: Any) -> bool:
    try:
        float(value)
    except Exception:  # noqa: BLE001
        return False
    return True


def _caseops_sparse_cosine(left: Any, right: Any) -> float:
    if not isinstance(left, dict) or not isinstance(right, dict):
        return 0.0
    left_values = {
        str(axis): float(value)
        for axis, value in left.items()
        if str(axis).strip() and float(value) != 0.0
    }
    right_values = {
        str(axis): float(value)
        for axis, value in right.items()
        if str(axis).strip() and float(value) != 0.0
    }
    if not left_values or not right_values:
        return 0.0
    dot = sum(
        left_values.get(axis, 0.0) * right_values.get(axis, 0.0)
        for axis in set(left_values).intersection(right_values)
    )
    left_mag = math.sqrt(sum(value * value for value in left_values.values()))
    right_mag = math.sqrt(sum(value * value for value in right_values.values()))
    if left_mag == 0.0 or right_mag == 0.0:
        return 0.0
    return dot / (left_mag * right_mag)


def _caseops_tensor_shared_axes(left: Any, right: Any) -> tuple[str, ...]:
    if not isinstance(left, dict) or not isinstance(right, dict):
        return ()
    axes: list[str] = []
    for axis in set(left).intersection(right):
        try:
            left_value = float(left.get(axis) or 0.0)
            right_value = float(right.get(axis) or 0.0)
        except Exception:  # noqa: BLE001
            continue
        if left_value * right_value > 0.0:
            axes.append(str(axis))
    return tuple(sorted(axes, key=_caseops_vocab_axis_sort_key))


def _caseops_tensor_opposing_axes(left: Any, right: Any) -> tuple[str, ...]:
    if not isinstance(left, dict) or not isinstance(right, dict):
        return ()
    axes: list[str] = []
    for axis in set(left).intersection(right):
        try:
            left_value = float(left.get(axis) or 0.0)
            right_value = float(right.get(axis) or 0.0)
        except Exception:  # noqa: BLE001
            continue
        if left_value * right_value < 0.0:
            axes.append(str(axis))
    return tuple(sorted(axes, key=_caseops_vocab_axis_sort_key))


def _caseops_vocab_axis_sort_key(axis: str) -> tuple[int, str]:
    text = str(axis)
    for rank, prefix in enumerate((
        "formula:",
        "concept:",
        "stack:",
        "crystal:",
        "domain:crystal",
        "repair:",
        "learned:",
        "domain:",
        "layer:",
        "kind:",
        "value_token:",
    )):
        if text.startswith(prefix):
            return (rank, text)
    return (99, text)


def _caseops_vocab_analogy_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    pair_prefix = "caseops_vocab_analogy_pair_"
    positive_prefix = "caseops_vocab_analogy_positive_axis_pair_"
    negative_prefix = "caseops_vocab_analogy_negative_axis_pair_"
    contrast_prefix = "caseops_vocab_analogy_contrast_pair_"
    pair_tokens = {
        token for token in unigrams
        if token.startswith(pair_prefix)
    }
    if include_penalties and penalties:
        pair_tokens.update(
            token for token in penalties
            if token.startswith(pair_prefix)
        )
    positive_tokens = {
        token for token in unigrams
        if token.startswith(positive_prefix)
    }
    negative_tokens = {
        token for token in unigrams
        if token.startswith(negative_prefix)
    }
    contrast_tokens = {
        token for token in unigrams
        if token.startswith(contrast_prefix)
    }
    if include_penalties and penalties:
        positive_tokens.update(
            token for token in penalties
            if token.startswith(positive_prefix)
        )
        negative_tokens.update(
            token for token in penalties
            if token.startswith(negative_prefix)
        )
        contrast_tokens.update(
            token for token in penalties
            if token.startswith(contrast_prefix)
        )

    positive_by_pair = _caseops_vocab_analogy_axis_rows(
        positive_tokens,
        positive_prefix,
        unigrams,
        penalties,
        role="positive",
    )
    negative_by_pair = _caseops_vocab_analogy_axis_rows(
        negative_tokens,
        negative_prefix,
        unigrams,
        penalties,
        role="negative",
    )
    contrasts_by_pair = _caseops_vocab_analogy_contrast_rows(
        contrast_tokens,
        contrast_prefix,
        unigrams,
        penalties,
    )
    rows: list[dict[str, Any]] = []
    for token in pair_tokens:
        value = token.removeprefix(pair_prefix)
        pieces = value.split("__")
        if len(pieces) < 2:
            continue
        kind = _normalize_feature_piece(pieces[0])
        vocab_value = _normalize_feature_piece("__".join(pieces[1:]))
        key = (kind, vocab_value)
        positive_axes = sorted(
            positive_by_pair.get(key, ()),
            key=lambda row: (
                -abs(float(row.get("effective_weight") or 0.0)),
                str(row.get("axis") or ""),
            ),
        )
        negative_axes = sorted(
            negative_by_pair.get(key, ()),
            key=lambda row: (
                -abs(float(row.get("effective_weight") or 0.0)),
                str(row.get("axis") or ""),
            ),
        )
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        positive_effective = sum(float(row.get("effective_weight") or 0.0) for row in positive_axes)
        negative_effective = sum(float(row.get("effective_weight") or 0.0) for row in negative_axes)
        contrast_rows = contrasts_by_pair.get(key, ())
        contrast_effective = sum(float(row.get("effective_weight") or 0.0) for row in contrast_rows)
        effective_weight = weight - penalty + positive_effective * 0.25 - negative_effective * 0.15 + contrast_effective * 0.05
        rows.append({
            "kind": _decode_caseops_vocab_kind(kind),
            "kind_value": kind,
            "value": _decode_caseops_vocab_value(vocab_value),
            "value_key": vocab_value,
            "token": token,
            "positive_axes": tuple(str(row["axis"]) for row in positive_axes),
            "negative_axes": tuple(str(row["axis"]) for row in negative_axes),
            "positive_axis_rows": tuple(dict(row) for row in positive_axes[:8]),
            "negative_axis_rows": tuple(dict(row) for row in negative_axes[:8]),
            "contrasts": tuple(dict(row) for row in contrast_rows[:8]),
            "weight": weight,
            "penalty": penalty,
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["kind"]),
        str(row["value"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_vocab_analogy_axis_rows(
    tokens: set[str],
    prefix: str,
    unigrams: dict[str, float],
    penalties: dict[str, float] | None,
    *,
    role: str,
) -> dict[tuple[str, str], list[dict[str, Any]]]:
    rows: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for token in tokens:
        value = token.removeprefix(prefix)
        pieces = value.split("__")
        if len(pieces) < 3:
            continue
        kind = _normalize_feature_piece(pieces[0])
        vocab_value = _normalize_feature_piece(pieces[1])
        axis = _decode_caseops_vocab_axis("__".join(pieces[2:]))
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        rows.setdefault((kind, vocab_value), []).append({
            "role": role,
            "axis": axis,
            "axis_value": "__".join(pieces[2:]),
            "axis_root": axis.split(":", 1)[0] if ":" in axis else axis,
            "token": token,
            "weight": weight,
            "penalty": penalty,
            "effective_weight": weight - penalty,
        })
    return rows


def _caseops_vocab_analogy_contrast_rows(
    tokens: set[str],
    prefix: str,
    unigrams: dict[str, float],
    penalties: dict[str, float] | None,
) -> dict[tuple[str, str], list[dict[str, Any]]]:
    rows: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for token in tokens:
        pieces = token.removeprefix(prefix).split("__")
        if len(pieces) != 4:
            continue
        kind = _normalize_feature_piece(pieces[0])
        vocab_value = _normalize_feature_piece(pieces[1])
        positive = _decode_caseops_vocab_axis(pieces[2])
        negative = _decode_caseops_vocab_axis(pieces[3])
        weight = float(unigrams.get(token) or 0.0)
        penalty = float((penalties or {}).get(token) or 0.0)
        rows.setdefault((kind, vocab_value), []).append({
            "positive_axis": positive,
            "negative_axis": negative,
            "token": token,
            "weight": weight,
            "penalty": penalty,
            "effective_weight": weight - penalty,
        })
    for key, values in rows.items():
        rows[key] = sorted(values, key=lambda row: (
            -abs(float(row.get("effective_weight") or 0.0)),
            str(row.get("positive_axis") or ""),
            str(row.get("negative_axis") or ""),
        ))
    return rows


def _caseops_vocab_analogy_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    positive_text = " ".join(str(axis) for axis in row.get("positive_axes", ()))
    negative_text = " ".join(str(axis) for axis in row.get("negative_axes", ()))
    contrast_text = " ".join(
        " ".join((str(item.get("positive_axis") or ""), str(item.get("negative_axis") or "")))
        for item in row.get("contrasts", ())
        if isinstance(item, dict)
    )
    row_terms = _activation_terms(" ".join((
        str(row.get("kind") or ""),
        str(row.get("kind_value") or ""),
        str(row.get("value") or ""),
        str(row.get("value_key") or ""),
        positive_text,
        negative_text,
        contrast_text,
        "analogy",
        "positive",
        "negative",
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
        positive_text,
    ))
    exact_bonus = 0.6 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def _caseops_stack_code_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    pair_prefix = "caseops_stack_pair_"
    code_prefix = "caseops_stack_vocab_code_pair_"
    axis_prefix = "caseops_stack_axis_pair_"
    link_prefix = "caseops_stack_link_pair_"
    pathway_prefix = "caseops_stack_axis_pathway_"
    tokens = {
        token for token in unigrams
        if token.startswith(pair_prefix)
    }
    if include_penalties and penalties:
        tokens.update(
            token for token in penalties
            if token.startswith(pair_prefix)
        )

    stack_codes: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for token in _caseops_weight_tokens(unigrams, penalties, code_prefix, include_penalties):
        value = token.removeprefix(code_prefix)
        pieces = value.split("__")
        if len(pieces) < 3:
            continue
        kind = _normalize_feature_piece(pieces[0])
        code = _normalize_feature_piece(pieces[-1])
        vocab_value = _normalize_feature_piece("__".join(pieces[1:-1]))
        stack_codes.setdefault((kind, vocab_value), []).append(
            _caseops_stack_code_weight_row(token, code, unigrams, penalties)
        )

    axes_by_pair: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for token in _caseops_weight_tokens(unigrams, penalties, axis_prefix, include_penalties):
        value = token.removeprefix(axis_prefix)
        pieces = value.split("__")
        if len(pieces) < 3:
            continue
        kind = _normalize_feature_piece(pieces[0])
        vocab_value = _normalize_feature_piece(pieces[1])
        axis_value = "__".join(pieces[2:])
        axis = _decode_caseops_vocab_axis(axis_value)
        axes_by_pair.setdefault((kind, vocab_value), []).append({
            "axis": axis,
            "axis_value": axis_value,
            "token": token,
            **_caseops_token_weight_fields(token, unigrams, penalties),
        })

    links_by_pair: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for token in _caseops_weight_tokens(unigrams, penalties, link_prefix, include_penalties):
        value = token.removeprefix(link_prefix)
        pieces = value.split("__")
        if len(pieces) < 3:
            continue
        kind = _normalize_feature_piece(pieces[0])
        vocab_value = _normalize_feature_piece(pieces[1])
        axis_value = "__".join(pieces[2:])
        axis = _decode_caseops_vocab_axis(axis_value)
        links_by_pair.setdefault((kind, vocab_value), []).append({
            "axis": axis,
            "axis_value": axis_value,
            "token": token,
            **_caseops_token_weight_fields(token, unigrams, penalties),
        })

    pathway_rows = []
    for token in _caseops_weight_tokens(unigrams, penalties, pathway_prefix, include_penalties):
        value = token.removeprefix(pathway_prefix)
        pieces = value.split("__")
        if len(pieces) != 2:
            continue
        pathway_rows.append({
            "axis_a": _decode_caseops_vocab_axis(pieces[0]),
            "axis_b": _decode_caseops_vocab_axis(pieces[1]),
            "value": value,
            "token": token,
            **_caseops_token_weight_fields(token, unigrams, penalties),
        })

    rows: list[dict[str, Any]] = []
    for token in tokens:
        value = token.removeprefix(pair_prefix)
        pieces = value.split("__")
        if len(pieces) < 2:
            continue
        kind = _normalize_feature_piece(pieces[0])
        vocab_value = _normalize_feature_piece("__".join(pieces[1:]))
        axes = sorted(
            axes_by_pair.get((kind, vocab_value), ()),
            key=lambda row: (-abs(float(row.get("effective_weight") or 0.0)), str(row.get("axis") or "")),
        )
        links = sorted(
            links_by_pair.get((kind, vocab_value), ()),
            key=lambda row: (-abs(float(row.get("effective_weight") or 0.0)), str(row.get("axis") or "")),
        )
        axis_set = {str(row.get("axis") or "") for row in axes}
        matching_pathways = tuple(
            dict(row)
            for row in pathway_rows
            if str(row.get("axis_a") or "") in axis_set
            and str(row.get("axis_b") or "") in axis_set
        )[:8]
        weight_fields = _caseops_token_weight_fields(token, unigrams, penalties)
        effective_weight = (
            float(weight_fields["effective_weight"])
            + sum(float(row.get("effective_weight") or 0.0) for row in axes) * 0.2
            + sum(float(row.get("effective_weight") or 0.0) for row in links) * 0.1
        )
        rows.append({
            "kind": _decode_caseops_vocab_kind(kind),
            "kind_value": kind,
            "value": _decode_caseops_vocab_value(vocab_value),
            "value_key": vocab_value,
            "token": token,
            "stack_codes": tuple(dict(row) for row in stack_codes.get((kind, vocab_value), ())[:4]),
            "axes": tuple(dict(row) for row in axes[:8]),
            "links": tuple(dict(row) for row in links[:8]),
            "axis_pathways": matching_pathways,
            "axis_roots": tuple(sorted({
                str(row.get("axis") or "").split(":", 1)[0]
                for row in axes
                if str(row.get("axis") or "")
            })),
            "weight": float(weight_fields["weight"]),
            "penalty": float(weight_fields["penalty"]),
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["kind"]),
        str(row["value"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_weight_tokens(
    unigrams: dict[str, float],
    penalties: dict[str, float] | None,
    prefix: str,
    include_penalties: bool,
) -> set[str]:
    tokens = {token for token in unigrams if token.startswith(prefix)}
    if include_penalties and penalties:
        tokens.update(token for token in penalties if token.startswith(prefix))
    return tokens


def _caseops_token_weight_fields(
    token: str,
    unigrams: dict[str, float],
    penalties: dict[str, float] | None,
) -> dict[str, float]:
    weight = float(unigrams.get(token) or 0.0)
    penalty = float((penalties or {}).get(token) or 0.0)
    return {
        "weight": weight,
        "penalty": penalty,
        "effective_weight": weight - penalty,
        "contribution": weight - penalty,
    }


def _caseops_stack_code_weight_row(
    token: str,
    code: str,
    unigrams: dict[str, float],
    penalties: dict[str, float] | None,
) -> dict[str, Any]:
    return {
        "code": code,
        "token": token,
        **_caseops_token_weight_fields(token, unigrams, penalties),
    }


def _caseops_stack_code_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    axis_text = " ".join(str(item.get("axis") or "") for item in row.get("axes", ()) if isinstance(item, dict))
    code_text = " ".join(str(item.get("code") or "") for item in row.get("stack_codes", ()) if isinstance(item, dict))
    pathway_text = " ".join(
        f"{item.get('axis_a')} {item.get('axis_b')}"
        for item in row.get("axis_pathways", ())
        if isinstance(item, dict)
    )
    row_terms = _activation_terms(" ".join((
        str(row.get("kind") or ""),
        str(row.get("kind_value") or ""),
        str(row.get("value") or ""),
        str(row.get("value_key") or ""),
        axis_text,
        code_text,
        pathway_text,
        " ".join(str(root) for root in row.get("axis_roots", ())),
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
        axis_text,
        code_text,
    ))
    exact_bonus = 0.5 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def _caseops_crystal_card_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    pair_prefix = "caseops_crystal_pair_"
    tokens = {
        token for token in unigrams
        if token.startswith(pair_prefix)
    }
    if include_penalties and penalties:
        tokens.update(
            token for token in penalties
            if token.startswith(pair_prefix)
        )
    formulas = _caseops_crystal_link_rows(
        unigrams,
        penalties,
        "caseops_crystal_formula_pair_",
        include_penalties,
        value_key="formula",
    )
    concepts = _caseops_crystal_link_rows(
        unigrams,
        penalties,
        "caseops_crystal_concept_pair_",
        include_penalties,
        value_key="concept",
    )
    suggestions = _caseops_crystal_link_rows(
        unigrams,
        penalties,
        "caseops_crystal_suggestion_pair_",
        include_penalties,
        value_key="formula",
    )
    neighbors = _caseops_crystal_link_rows(
        unigrams,
        penalties,
        "caseops_crystal_neighbor_pair_",
        include_penalties,
        value_key="formula",
    )
    tags = _caseops_crystal_tag_rows(unigrams, penalties, include_penalties)
    stack_codes = _caseops_crystal_stack_rows(unigrams, penalties, include_penalties)

    rows: list[dict[str, Any]] = []
    for token in tokens:
        value = token.removeprefix(pair_prefix)
        pieces = value.split("__")
        if len(pieces) < 2:
            continue
        kind = _normalize_feature_piece(pieces[0])
        card_id = _normalize_feature_piece("__".join(pieces[1:]))
        key = card_id
        weight_fields = _caseops_token_weight_fields(token, unigrams, penalties)
        formula_rows = tuple(dict(row) for row in formulas.get(key, ())[:8])
        concept_rows = tuple(dict(row) for row in concepts.get(key, ())[:8])
        stack_rows = tuple(dict(row) for row in stack_codes.get(key, ())[:8])
        tag_rows = tuple(dict(row) for row in tags.get(key, ())[:8])
        effective_weight = (
            float(weight_fields["effective_weight"])
            + sum(float(row.get("effective_weight") or 0.0) for row in formula_rows) * 0.2
            + sum(float(row.get("effective_weight") or 0.0) for row in concept_rows) * 0.2
            + sum(float(row.get("effective_weight") or 0.0) for row in stack_rows) * 0.1
            + sum(float(row.get("effective_weight") or 0.0) for row in tag_rows) * 0.1
        )
        rows.append({
            "card_id": _decode_caseops_vocab_value(card_id),
            "card_key": card_id,
            "kind": _decode_caseops_vocab_kind(kind),
            "kind_value": kind,
            "token": token,
            "formulas": formula_rows,
            "concepts": concept_rows,
            "suggestions": tuple(dict(row) for row in suggestions.get(key, ())[:8]),
            "neighbors": tuple(dict(row) for row in neighbors.get(key, ())[:8]),
            "tags": tag_rows,
            "stack_codes": stack_rows,
            "weight": float(weight_fields["weight"]),
            "penalty": float(weight_fields["penalty"]),
            "effective_weight": effective_weight,
            "contribution": effective_weight,
        })
    rows.sort(key=lambda row: (
        -abs(float(row["effective_weight"])),
        -float(row["weight"]),
        str(row["kind"]),
        str(row["card_id"]),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_crystal_link_rows(
    unigrams: dict[str, float],
    penalties: dict[str, float] | None,
    prefix: str,
    include_penalties: bool,
    *,
    value_key: str,
) -> dict[str, list[dict[str, Any]]]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    for token in _caseops_weight_tokens(unigrams, penalties, prefix, include_penalties):
        value = token.removeprefix(prefix)
        card_id, sep, target = value.partition("__")
        if not sep or not card_id or not target:
            continue
        grouped.setdefault(card_id, []).append({
            value_key: _decode_caseops_vocab_value(target),
            "value": _decode_caseops_vocab_value(target),
            "value_key": target,
            "token": token,
            **_caseops_token_weight_fields(token, unigrams, penalties),
        })
    for rows in grouped.values():
        rows.sort(key=lambda row: (-abs(float(row.get("effective_weight") or 0.0)), str(row.get("value") or "")))
    return grouped


def _caseops_crystal_tag_rows(
    unigrams: dict[str, float],
    penalties: dict[str, float] | None,
    include_penalties: bool,
) -> dict[str, list[dict[str, Any]]]:
    prefix = "caseops_crystal_tag_pair_"
    grouped: dict[str, list[dict[str, Any]]] = {}
    for token in _caseops_weight_tokens(unigrams, penalties, prefix, include_penalties):
        pieces = token.removeprefix(prefix).split("__")
        if len(pieces) < 3:
            continue
        card_id = pieces[0]
        dim = _decode_caseops_vocab_value(pieces[1])
        value = _decode_caseops_vocab_value("__".join(pieces[2:]))
        grouped.setdefault(card_id, []).append({
            "dimension": dim,
            "value": value,
            "token": token,
            **_caseops_token_weight_fields(token, unigrams, penalties),
        })
    for rows in grouped.values():
        rows.sort(key=lambda row: (-abs(float(row.get("effective_weight") or 0.0)), str(row.get("dimension") or ""), str(row.get("value") or "")))
    return grouped


def _caseops_crystal_stack_rows(
    unigrams: dict[str, float],
    penalties: dict[str, float] | None,
    include_penalties: bool,
) -> dict[str, list[dict[str, Any]]]:
    prefix = "caseops_crystal_stack_pair_"
    grouped: dict[str, list[dict[str, Any]]] = {}
    for token in _caseops_weight_tokens(unigrams, penalties, prefix, include_penalties):
        pieces = token.removeprefix(prefix).split("__")
        if len(pieces) != 3:
            continue
        card_id, layer, code = pieces
        grouped.setdefault(card_id, []).append({
            "layer": _decode_caseops_vocab_value(layer),
            "code": code,
            "token": token,
            **_caseops_token_weight_fields(token, unigrams, penalties),
        })
    for rows in grouped.values():
        rows.sort(key=lambda row: (-abs(float(row.get("effective_weight") or 0.0)), str(row.get("layer") or ""), str(row.get("code") or "")))
    return grouped


def _caseops_crystal_pathway_rows(
    unigrams: dict[str, float],
    *,
    top_n: int,
    penalties: dict[str, float] | None = None,
    include_penalties: bool = False,
) -> list[dict[str, Any]]:
    cards = _caseops_crystal_card_rows(
        unigrams,
        top_n=max(top_n * 4, len(unigrams), 64),
        penalties=penalties,
        include_penalties=include_penalties,
    )
    rows: list[dict[str, Any]] = []
    for card in cards:
        card_id = str(card.get("card_id") or "")
        kind = str(card.get("kind") or "")
        card_weight = float(card.get("effective_weight", card.get("weight", 0.0)) or 0.0)
        source = f"card:{card_id}" if card_id else "card:-"

        for target_kind, relation, field in (
            ("formula", "card_formula", "formulas"),
            ("suggestion", "card_suggestion", "suggestions"),
            ("neighbor", "card_neighbor", "neighbors"),
            ("concept", "card_concept", "concepts"),
        ):
            for item in card.get(field, ()) or ():
                if not isinstance(item, dict):
                    continue
                value = str(item.get("value") or "")
                if not value:
                    continue
                rows.append(_caseops_crystal_pathway_row(
                    card=card,
                    item=item,
                    source=source,
                    target=f"{target_kind}:{value}",
                    target_kind=target_kind,
                    target_value=value,
                    relation=relation,
                    card_weight=card_weight,
                    kind=kind,
                    card_id=card_id,
                ))

        for item in card.get("tags", ()) or ():
            if not isinstance(item, dict):
                continue
            dimension = str(item.get("dimension") or "")
            value = str(item.get("value") or "")
            if not value:
                continue
            target_value = f"{dimension}:{value}" if dimension else value
            rows.append(_caseops_crystal_pathway_row(
                card=card,
                item=item,
                source=source,
                target=f"tag:{target_value}",
                target_kind="tag",
                target_value=target_value,
                relation="card_tag",
                card_weight=card_weight,
                kind=kind,
                card_id=card_id,
                dimension=dimension,
            ))

        for item in card.get("stack_codes", ()) or ():
            if not isinstance(item, dict):
                continue
            layer = str(item.get("layer") or "")
            code = str(item.get("code") or "")
            if not layer or not code:
                continue
            target_value = f"{layer}:{code}"
            rows.append(_caseops_crystal_pathway_row(
                card=card,
                item=item,
                source=source,
                target=f"stack:{target_value}",
                target_kind="stack",
                target_value=target_value,
                relation="card_stack_code",
                card_weight=card_weight,
                kind=kind,
                card_id=card_id,
                layer=layer,
                code=code,
            ))
    rows.sort(key=lambda row: (
        -abs(float(row.get("effective_weight") or 0.0)),
        -float(row.get("weight") or 0.0),
        str(row.get("card_id") or ""),
        str(row.get("target") or ""),
    ))
    return rows[: max(1, int(top_n))]


def _caseops_crystal_pathway_row(
    *,
    card: dict[str, Any],
    item: dict[str, Any],
    source: str,
    target: str,
    target_kind: str,
    target_value: str,
    relation: str,
    card_weight: float,
    kind: str,
    card_id: str,
    dimension: str = "",
    layer: str = "",
    code: str = "",
) -> dict[str, Any]:
    weight = float(item.get("weight") or 0.0)
    penalty = float(item.get("penalty") or 0.0)
    link_effective = float(item.get("effective_weight", weight - penalty) or 0.0)
    effective_weight = link_effective + card_weight * 0.1
    return {
        "card_id": card_id,
        "card_key": card.get("card_key", ""),
        "kind": kind,
        "kind_value": card.get("kind_value", ""),
        "source": source,
        "target": target,
        "target_kind": target_kind,
        "target_value": target_value,
        "relation": relation,
        "dimension": dimension,
        "layer": layer,
        "code": code,
        "token": item.get("token", ""),
        "weight": weight,
        "penalty": penalty,
        "effective_weight": effective_weight,
        "card_weight": card_weight,
        "contribution": effective_weight,
    }


def _caseops_crystal_card_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    formula_text = " ".join(str(item.get("value") or "") for item in row.get("formulas", ()) if isinstance(item, dict))
    concept_text = " ".join(str(item.get("value") or "") for item in row.get("concepts", ()) if isinstance(item, dict))
    suggestion_text = " ".join(str(item.get("value") or "") for item in row.get("suggestions", ()) if isinstance(item, dict))
    neighbor_text = " ".join(str(item.get("value") or "") for item in row.get("neighbors", ()) if isinstance(item, dict))
    tag_text = " ".join(
        f"{item.get('dimension')} {item.get('value')}"
        for item in row.get("tags", ())
        if isinstance(item, dict)
    )
    stack_text = " ".join(
        f"{item.get('layer')} {item.get('code')}"
        for item in row.get("stack_codes", ())
        if isinstance(item, dict)
    )
    row_terms = _activation_terms(" ".join((
        str(row.get("card_id") or ""),
        str(row.get("card_key") or ""),
        str(row.get("kind") or ""),
        str(row.get("kind_value") or ""),
        formula_text,
        concept_text,
        suggestion_text,
        neighbor_text,
        tag_text,
        stack_text,
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("card_id") or ""),
        formula_text,
        concept_text,
        tag_text,
    ))
    exact_bonus = 0.5 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def _caseops_crystal_pathway_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    row_terms = _activation_terms(" ".join((
        str(row.get("card_id") or ""),
        str(row.get("card_key") or ""),
        str(row.get("kind") or ""),
        str(row.get("kind_value") or ""),
        str(row.get("source") or ""),
        str(row.get("target") or ""),
        str(row.get("target_kind") or ""),
        str(row.get("target_value") or ""),
        str(row.get("relation") or ""),
        str(row.get("dimension") or ""),
        str(row.get("layer") or ""),
        str(row.get("code") or ""),
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("card_id") or ""),
        str(row.get("target") or ""),
        str(row.get("target_value") or ""),
    ))
    exact_bonus = 0.5 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def _decode_caseops_vocab_kind(value: str) -> str:
    clean = _normalize_feature_piece(value)
    known_roots = ("future", "mathlang", "system", "lang", "math")
    for root in known_roots:
        prefix = f"{root}_"
        if clean.startswith(prefix):
            return root + "." + clean.removeprefix(prefix)
    return clean


def _decode_caseops_vocab_value(value: str) -> str:
    clean = _normalize_feature_piece(value)
    if clean.startswith("ling_concept_"):
        return "ling.concept:" + clean.removeprefix("ling_concept_")
    if clean.startswith("ling_role_"):
        return "ling.role:" + clean.removeprefix("ling_role_")
    if clean.startswith("ling_predicate_"):
        return "ling.predicate:" + clean.removeprefix("ling_predicate_")
    return clean


def _decode_caseops_vocab_axis(value: str) -> str:
    clean = _normalize_feature_piece(value)
    if "_" not in clean:
        return clean
    for root in (
        "concept",
        "crystal",
        "domain",
        "formula",
        "kind_root",
        "kind",
        "layer",
        "learned",
        "link",
        "repair",
        "source",
        "stack_code",
        "stack_layer",
        "stack",
        "value_token",
    ):
        prefix = f"{root}_"
        if clean.startswith(prefix):
            return f"{root}:{clean.removeprefix(prefix)}"
    return clean


def _caseops_vocab_vector_score(
    row: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    weight = float(row.get("effective_weight", row.get("weight", 0.0)) or 0.0)
    if not query_terms:
        return (weight, ())
    axis_text = " ".join(str(axis) for axis in row.get("axes", ()))
    root_text = " ".join(str(root) for root in row.get("axis_roots", ()))
    row_terms = _activation_terms(" ".join((
        str(row.get("kind") or ""),
        str(row.get("kind_value") or ""),
        str(row.get("value") or ""),
        str(row.get("value_key") or ""),
        axis_text,
        root_text,
    )))
    matched = tuple(sorted(query_terms.intersection(row_terms)))
    if not matched:
        return (0.0, ())
    exact_text = " ".join((
        str(row.get("kind") or ""),
        str(row.get("value") or ""),
        axis_text,
    ))
    exact_bonus = 0.6 if any(term in exact_text for term in matched) else 0.0
    overlap = len(matched) / max(1, len(row_terms))
    return (weight * (overlap + exact_bonus), matched)


def _decode_caseops_gap_target(value: str) -> str:
    target, _ = _decode_caseops_consolidated_target(value)
    return target


def _caseops_gap_formula_query(target: str) -> str:
    raw = str(target or "").strip()
    if not raw:
        return ""
    if "->lang." in raw:
        left, _, right = raw.partition("->")
        _, _, value = right.partition(":")
        raw = " ".join((left, value or right))
    elif raw.startswith("ling."):
        _, _, raw = raw.partition(":")
    elif raw.startswith("family:"):
        _, _, raw = raw.partition(":")
        raw = raw.split(":", 1)[0]
    return " ".join(_TOKEN_RE.findall(raw.lower()))


def _caseops_gap_term(target: str) -> str:
    query = _caseops_gap_formula_query(target)
    terms = _TOKEN_RE.findall(query)
    if terms:
        return "_".join(terms[:4])
    return _normalize_feature_piece(target)


def _caseops_gap_repair_token(row: dict[str, Any]) -> str:
    kind = _normalize_feature_piece(str(row.get("kind") or "target"))
    target = _normalize_feature_piece(str(row.get("target") or ""))
    reason = _normalize_feature_piece(str(row.get("reason") or "unresolved"))
    if not kind or not target or not reason:
        return ""
    return f"caseops_gap_repair_{kind}__{target}__{reason}"


def _caseops_gap_repair_response(row: dict[str, Any], token: str) -> str:
    target = str(row.get("target") or "").strip()
    kind = _normalize_feature_piece(str(row.get("kind") or "target"))
    reason = _normalize_feature_piece(str(row.get("reason") or "unresolved"))
    if not target or not token:
        return ""
    score = float(row.get("effective_weight", row.get("score", 0.0)) or 0.0)
    repair = str(row.get("repair_query") or "")
    pieces = [
        "caseops gap repair consolidation",
        f"caseops_gap_feedback={kind}|{target}|{reason}|{score:.3f}|{token}",
        f"caseops_gap_repair_status={row.get('status') or 'unknown'}",
        f"caseops_gap_repair_layer={row.get('layer') or 'unknown'}",
    ]
    if repair:
        pieces.append(f"caseops_gap_repair_query={repair}")
    return " ".join(pieces)[:1000]


def activate_caseops_neurons(
    learner: "MicroLearner",
    query: str,
    *,
    top_n: int = 8,
    spread: float = 0.35,
    depth: int = 1,
    min_score: float = 0.0,
) -> dict[str, Any]:
    """Activate learned CaseOps neurons for a query and spread over synapses."""
    top_n = max(1, int(top_n))
    spread = max(0.0, float(spread))
    depth = max(1, min(4, int(depth)))
    min_score = float(min_score)
    unigrams = learner.slow_model.unigrams
    graph = _caseops_neuron_graph_from_unigrams(
        unigrams,
        top_n=max(len(unigrams), top_n * 8, 64),
        min_weight=0.0,
    )
    node_by_id = {
        str(row["id"]): dict(row)
        for row in graph.get("nodes", ())
        if isinstance(row, dict)
    }
    query_terms = _activation_terms(query)
    scores: dict[str, dict[str, Any]] = {}

    def bucket(node_id: str) -> dict[str, Any]:
        node = node_by_id.get(node_id)
        if node is None:
            node = {"id": node_id, "kind": "unknown", "label": node_id, "weight": 0.0}
        return scores.setdefault(node_id, {
            "id": node_id,
            "kind": node.get("kind", "unknown"),
            "label": node.get("label", node_id),
            "weight": float(node.get("weight") or 0.0),
            "direct_score": 0.0,
            "spread_score": 0.0,
            "score": 0.0,
            "reasons": [],
        })

    for node_id, node in node_by_id.items():
        direct, matched = _caseops_node_direct_activation(node, query_terms)
        if direct <= 0.0:
            continue
        row = bucket(node_id)
        row["direct_score"] = float(row["direct_score"]) + direct
        row["score"] = float(row["score"]) + direct
        reasons = row["reasons"]
        if isinstance(reasons, list):
            reasons.extend(f"match:{term}" for term in matched[:4])

    adjacency: dict[str, list[tuple[str, dict[str, Any]]]] = {}
    for edge in graph.get("edges", ()):
        if not isinstance(edge, dict):
            continue
        source = str(edge.get("source") or "")
        target = str(edge.get("target") or "")
        if not source or not target:
            continue
        weight = float(edge.get("weight") or 0.0)
        if weight <= 0.0:
            continue
        for left, right in ((source, target), (target, source)):
            adjacency.setdefault(left, []).append((right, edge))

    activated_edges: list[dict[str, Any]] = []
    activation_routes: list[dict[str, Any]] = []
    frontier_items = [
        {
            "node": node_id,
            "score": float(row.get("direct_score") or 0.0),
            "nodes": (node_id,),
            "relations": (),
        }
        for node_id, row in scores.items()
        if float(row.get("direct_score") or 0.0) > 0.0
    ]
    seed_nodes = {str(item["node"]) for item in frontier_items}
    for hop in range(1, depth + 1):
        if not frontier_items:
            break
        seen_before_hop = set(scores)
        next_items: list[dict[str, Any]] = []
        for item in frontier_items:
            left = str(item["node"])
            base_score = float(item["score"])
            path_nodes = tuple(str(node) for node in item["nodes"])
            path_relations = tuple(str(rel) for rel in item["relations"])
            if base_score <= 0.0:
                continue
            for right, edge in adjacency.get(left, ()):
                if right in path_nodes:
                    continue
                if hop > 1 and (right in seed_nodes or right in seen_before_hop):
                    continue
                weight = float(edge.get("weight") or 0.0)
                contribution = base_score * weight * spread
                if contribution <= 0.0:
                    continue
                relation = str(edge.get("relation") or "caseops_synapse")
                route_nodes = (*path_nodes, right)
                route_relations = (*path_relations, relation)
                target_row = bucket(right)
                target_row["spread_score"] = float(target_row["spread_score"]) + contribution
                target_row["score"] = float(target_row["score"]) + contribution
                reasons = target_row["reasons"]
                if isinstance(reasons, list):
                    reasons.append(f"spread{hop}:{left}")
                next_items.append({
                    "node": right,
                    "score": contribution,
                    "nodes": route_nodes,
                    "relations": route_relations,
                })
                activated_edges.append({
                    "source": left,
                    "target": right,
                    "relation": relation,
                    "weight": weight,
                    "contribution": contribution,
                    "hop": hop,
                })
                activation_routes.append({
                    "source": route_nodes[0],
                    "target": right,
                    "nodes": route_nodes,
                    "relations": route_relations,
                    "hops": hop,
                    "contribution": contribution,
                })
        frontier_items = next_items

    activated = [
        row for row in scores.values()
        if float(row.get("score") or 0.0) >= min_score
    ]
    _annotate_caseops_mlp_scores(
        learner,
        query,
        activated,
        row_kind="activation_node",
    )
    _annotate_caseops_mlp_scores(
        learner,
        query,
        activated_edges,
        row_kind="activation_synapse",
        score_field="contribution",
    )
    _annotate_caseops_mlp_scores(
        learner,
        query,
        activation_routes,
        row_kind="activation_route",
        score_field="contribution",
    )
    activated.sort(key=lambda row: (
        -float(row.get("score") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        -float(row.get("direct_score") or 0.0),
        str(row.get("id") or ""),
    ))
    activated_edges.sort(key=lambda row: (
        -float(row.get("score") or row.get("contribution") or 0.0),
        -float(row.get("contribution") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        str(row.get("source") or ""),
        str(row.get("target") or ""),
    ))
    activation_routes.sort(key=lambda row: (
        -float(row.get("score") or row.get("contribution") or 0.0),
        -float(row.get("contribution") or 0.0),
        -float(row.get("mlp_score") or 0.0),
        int(row.get("hops") or 0),
        str(row.get("source") or ""),
        str(row.get("target") or ""),
    ))
    return {
        "query": str(query or ""),
        "terms": tuple(sorted(query_terms)),
        "activated": tuple(dict(row) for row in activated[:top_n]),
        "synapses": tuple(dict(row) for row in activated_edges[:top_n]),
        "routes": tuple(dict(row) for row in activation_routes[:top_n]),
        "summary": {
            "activated": len(activated),
            "synapses": len(activated_edges),
            "routes": len(activation_routes),
            "direct": sum(1 for row in activated if float(row.get("direct_score") or 0.0) > 0.0),
            "spread": sum(1 for row in activated if float(row.get("spread_score") or 0.0) > 0.0),
            "depth": depth,
            "mlp_scored": sum(1 for row in activated if "mlp_score" in row),
            "mlp_scored_synapses": sum(1 for row in activated_edges if "mlp_score" in row),
            "mlp_scored_routes": sum(1 for row in activation_routes if "mlp_score" in row),
        },
    }


def _caseops_neuron_graph_from_unigrams(
    unigrams: dict[str, float],
    *,
    top_n: int,
    min_weight: float,
) -> dict[str, Any]:
    top_n = max(1, int(top_n))
    min_weight = float(min_weight)
    nodes: dict[str, dict[str, Any]] = {}
    edges: list[dict[str, Any]] = []

    def ensure_node(node_id: str, kind: str, label: str, weight: float) -> None:
        existing = nodes.get(node_id)
        if existing is None:
            nodes[node_id] = {
                "id": node_id,
                "kind": kind,
                "label": label,
                "weight": float(weight),
            }
            return
        existing["weight"] = max(float(existing["weight"]), float(weight))

    for token, weight in unigrams.items():
        if weight < min_weight:
            continue
        if token.startswith("caseops_formula_"):
            formula_id = token.removeprefix("caseops_formula_")
            ensure_node(f"formula:{formula_id}", "formula", formula_id, weight)
        elif token.startswith("caseops_family_"):
            family_id = token.removeprefix("caseops_family_")
            ensure_node(f"family:{family_id}", "family", family_id, weight)
        elif token.startswith("caseops_language_value_"):
            value = token.removeprefix("caseops_language_value_")
            ensure_node(f"language:{value}", "language", value, weight)

    for token, weight in unigrams.items():
        if weight < min_weight:
            continue
        if token.startswith("caseops_pathway_pair_"):
            left, right = _caseops_pair_parts(token.removeprefix("caseops_pathway_pair_"))
            if not left or not right:
                continue
            source = f"formula:{left}"
            target = f"formula:{right}"
            ensure_node(source, "formula", left, weight)
            ensure_node(target, "formula", right, weight)
            edges.append({
                "source": source,
                "target": target,
                "relation": "caseops_formula_synapse",
                "weight": float(weight),
                "token": token,
            })
        elif token.startswith("caseops_belongs_to_family_pair_"):
            formula_id, family_id = _caseops_pair_parts(
                token.removeprefix("caseops_belongs_to_family_pair_")
            )
            if not formula_id or not family_id:
                continue
            source = f"formula:{formula_id}"
            target = f"family:{family_id}"
            ensure_node(source, "formula", formula_id, weight)
            ensure_node(target, "family", family_id, weight)
            edges.append({
                "source": source,
                "target": target,
                "relation": "caseops_formula_family_synapse",
                "weight": float(weight),
                "token": token,
            })
        elif token.startswith("caseops_math_language_pair_"):
            formula_id, language_kind, language_value = _caseops_math_language_parts(
                token.removeprefix("caseops_math_language_pair_")
            )
            if not formula_id or not language_kind or not language_value:
                continue
            source = f"formula:{formula_id}"
            target = f"language:{language_kind}:{language_value}"
            ensure_node(source, "formula", formula_id, weight)
            ensure_node(
                target,
                "language",
                f"{language_kind}:{language_value}",
                weight,
            )
            edges.append({
                "source": source,
                "target": target,
                "relation": "caseops_math_language_synapse",
                "weight": float(weight),
                "token": token,
                "target_kind": language_kind,
                "target_value": language_value,
            })

    node_rows = sorted(
        nodes.values(),
        key=lambda row: (-float(row["weight"]), str(row["kind"]), str(row["id"])),
    )[:top_n]
    edge_rows = sorted(
        edges,
        key=lambda row: (
            -float(row["weight"]),
            str(row["relation"]),
            str(row["source"]),
            str(row["target"]),
        ),
    )[:top_n]
    return {
        "nodes": tuple(dict(row) for row in node_rows),
        "edges": tuple(dict(row) for row in edge_rows),
        "summary": {
            "nodes": len(nodes),
            "synapses": len(edges),
            "formulas": sum(1 for row in nodes.values() if row["kind"] == "formula"),
            "families": sum(1 for row in nodes.values() if row["kind"] == "family"),
            "language": sum(1 for row in nodes.values() if row["kind"] == "language"),
        },
    }


def _caseops_pair_parts(value: str) -> tuple[str, str]:
    left, sep, right = str(value or "").partition("__")
    if not sep or not left or not right:
        return ("", "")
    return (left, right)


def _caseops_math_language_parts(value: str) -> tuple[str, str, str]:
    formula_id, sep, rest = str(value or "").partition("__")
    if not sep or not formula_id or not rest.startswith("lang_"):
        return ("", "", "")
    pieces = [piece for piece in rest.split("_") if piece]
    if len(pieces) < 3 or pieces[0] != "lang":
        return ("", "", "")
    language_kind = f"lang.{pieces[1]}"
    language_value = "_".join(pieces[2:])
    return (formula_id, language_kind, language_value)


def _activation_terms(value: str) -> set[str]:
    terms = set(_tokens(value))
    for token in list(terms):
        if "_" in token:
            terms.update(part for part in token.split("_") if part)
    return {
        term for term in terms
        if term not in {"caseops", "formula", "language", "lang", "concept", "structure"}
    }


def _caseops_node_direct_activation(
    node: dict[str, Any],
    query_terms: set[str],
) -> tuple[float, tuple[str, ...]]:
    if not query_terms:
        return (0.0, ())
    label = str(node.get("label") or "")
    node_id = str(node.get("id") or "")
    weight = float(node.get("weight") or 0.0)
    node_terms = _activation_terms(" ".join((label, node_id.replace(":", " "))))
    matched = tuple(sorted(query_terms.intersection(node_terms)))
    if not matched:
        return (0.0, ())
    exact_bonus = 1.0 if label.lower() in query_terms or label.lower() in " ".join(query_terms) else 0.0
    overlap = len(matched) / max(1, len(node_terms))
    score = weight * (overlap + exact_bonus)
    return (score, matched)


def _caseops_audit(
    unigrams: dict[str, float],
    penalties: dict[str, float],
    snippets: list[dict[str, Any]],
    *,
    top_n: int,
) -> dict[str, Any]:
    caseops_fast = [
        item for item in snippets
        if "caseops" in set(str(tag) for tag in item.get("tags", ()))
        or "SLOP1~" in str(item.get("text", ""))
    ]
    return {
        "slow_features": sum(
            1 for token in unigrams
            if token == "caseops" or token.startswith("caseops_")
        ),
        "penalized_features": sum(
            1 for token in penalties
            if token == "caseops" or token.startswith("caseops_")
        ),
        "fast_snippets": len(caseops_fast),
        "top_layers": _top_weighted_prefix(
            unigrams, "caseops_layer_", top_n=top_n
        ),
        "top_kinds": _top_weighted_prefix(
            unigrams, "caseops_kind_", top_n=top_n
        ),
        "top_formulas": _top_weighted_prefix(
            unigrams, "caseops_formula_", top_n=top_n
        ),
        "top_families": _top_weighted_prefix(
            unigrams, "caseops_family_", top_n=top_n
        ),
        "top_values": _top_weighted_prefix(
            unigrams, "caseops_value_", top_n=top_n
        ),
        "top_formula_families": _top_weighted_prefix(
            unigrams, "caseops_belongs_to_family_pair_", top_n=top_n
        ),
        "top_family_outcomes": _caseops_family_outcome_rows(
            unigrams,
            top_n=top_n,
            penalties=penalties,
            include_penalties=True,
        ),
        "top_linguistic_links": _caseops_linguistic_link_rows(
            unigrams,
            top_n=top_n,
            penalties=penalties,
            include_penalties=True,
        ),
        "top_consolidated_targets": _top_weighted_prefix(
            unigrams, "caseops_consolidated_target_", top_n=top_n
        ),
        "top_pathways": _top_weighted_prefix(
            unigrams, "caseops_pathway_pair_", top_n=top_n
        ),
        "top_language": _top_weighted_prefix(
            unigrams, "caseops_language_value_", top_n=top_n
        ),
        "top_math_language_pathways": _top_weighted_prefix(
            unigrams, "caseops_math_language_pair_", top_n=top_n
        ),
        "top_semantic_language_pathways": _caseops_semantic_pathway_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_codec_gaps": _caseops_codec_gap_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_gap_repair_hints": _caseops_gap_repair_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_vocab_vectors": _caseops_vocab_vector_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_vocab_axes": _top_weighted_prefix(
            unigrams, "caseops_vocab_axis_", top_n=top_n
        ),
        "top_vocab_axis_pathways": _caseops_vocab_axis_pathway_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_vocab_tensors": _caseops_vocab_tensor_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_vocab_analogies": _caseops_vocab_analogy_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_vocab_route_transfers": _caseops_vocab_tensor_codebook_route_axis_rows(
            unigrams,
            top_n=top_n,
            penalties=penalties,
            include_penalties=True,
        ),
        "top_model_kernel_layers": _top_weighted_prefix(
            unigrams, "caseops_model_kernel_layer_", top_n=top_n
        ),
        "top_model_training_layer_routes": _top_weighted_prefix(
            unigrams, "caseops_model_train_layer_route_", top_n=top_n
        ),
        "top_model_training_routes": _top_weighted_prefix(
            unigrams, "caseops_model_train_route_", top_n=top_n
        ),
        "top_stack_code_paths": _caseops_stack_code_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_crystallized_cards": _caseops_crystal_card_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_crystal_pathways": _caseops_crystal_pathway_rows(
            unigrams,
            top_n=top_n,
        ),
        "top_activation_route_pairs": _caseops_activation_route_rows(
            unigrams,
            "caseops_activation_route_pair_",
            top_n=top_n,
        ),
        "top_activation_route_steps": _caseops_activation_route_rows(
            unigrams,
            "caseops_activation_route_step_",
            top_n=top_n,
        ),
        "neuron_graph": _caseops_neuron_graph_from_unigrams(
            unigrams,
            top_n=top_n,
            min_weight=0.0,
        ),
    }


class MicroLearner:
    """Facade combining slow token weights and fast textual context."""

    def __init__(
        self,
        slow_model: SlowTokenModel | None = None,
        context_pool: FastContextPool | None = None,
        mlp_model: TinyMLPScorer | None = None,
        *,
        slow_weight: float = 0.65,
        fast_weight: float = 0.35,
        kernel_weight: float = 0.15,
        mlp_weight: float = 0.10,
        model_language: dict[str, Any] | None = None,
    ) -> None:
        if (
            slow_weight < 0.0
            or fast_weight < 0.0
            or kernel_weight < 0.0
            or mlp_weight < 0.0
        ):
            raise ValueError("weights must be non-negative")
        self.slow_model = slow_model or SlowTokenModel()
        self.context_pool = context_pool or FastContextPool()
        self.mlp_model = mlp_model or TinyMLPScorer()
        self.slow_weight = slow_weight
        self.fast_weight = fast_weight
        self.kernel_weight = kernel_weight
        self.mlp_weight = mlp_weight
        self.model_language = _normalize_model_language_state(model_language)

    def observe(self, example: RolloutExample, feedback: Feedback) -> None:
        self.slow_model.observe(
            example,
            feedback,
            protected_prefixes=self.model_language.get("protected_prefixes", ()),
        )
        self.context_pool.observe(example, feedback)
        self.mlp_model.observe(example, feedback)

    def retrieve_context(
        self, query: str, *, limit: int = 5, tags: Iterable[str] = ()
    ) -> list[str]:
        return self.context_pool.retrieve(query, limit=limit, tags=tags)

    def score_candidate(self, prompt: str, candidate: str) -> RankedCandidate:
        slow_score = self.slow_model.score(prompt, candidate)
        fast_score, matched = self.context_pool.score_text(prompt, candidate)
        kernel = _caseops_kernel_route_prior(
            prompt,
            candidate,
            unigrams=self.slow_model.unigrams,
            penalties=self.slow_model.penalties,
            model_language=self.model_language,
        )
        kernel_score = float(kernel.get("score", 0.0) or 0.0)
        mlp_score = self.mlp_model.score(prompt, candidate)
        score = (
            self.slow_weight * slow_score
            + self.fast_weight * fast_score
            + self.kernel_weight * kernel_score
            + self.mlp_weight * mlp_score
        )
        return RankedCandidate(
            text=candidate,
            score=score,
            slow_score=slow_score,
            fast_score=fast_score,
            kernel_score=kernel_score,
            mlp_score=mlp_score,
            kernel_evidence=tuple(kernel.get("evidence", ()) or ()),
            matched_context=matched,
        )

    def explain_candidate(self, prompt: str, candidate: str) -> dict[str, Any]:
        ranked = self.score_candidate(prompt, candidate)
        slow = self.slow_model.explain(prompt, candidate)
        return {
            "candidate": candidate,
            "score": ranked.score,
            "slow_score": ranked.slow_score,
            "fast_score": ranked.fast_score,
            "kernel_score": ranked.kernel_score,
            "mlp_score": ranked.mlp_score,
            "mlp": self.mlp_model.explain(prompt, candidate),
            "kernel_evidence": ranked.kernel_evidence,
            "matched_context": ranked.matched_context,
            "tokens": slow["tokens"],
            "known_tokens": slow["known_tokens"],
            "penalized_tokens": slow["penalized_tokens"],
            "bigram_hits": slow["bigram_hits"],
        }

    def audit(
        self,
        *,
        min_consolidation_score: float = 0.5,
        top_n: int = 5,
    ) -> dict[str, Any]:
        """Summarize learner health and likely maintenance actions."""
        if min_consolidation_score < 0.0:
            raise ValueError("min_consolidation_score must be non-negative")
        if top_n < 1:
            top_n = 1

        snippets = list(self.context_pool.to_dict()["snippets"])
        positive = [item for item in snippets if float(item.get("score", 0.0)) > 0.0]
        negative = [item for item in snippets if float(item.get("score", 0.0)) < 0.0]
        ready = [
            item for item in positive
            if float(item.get("score", 0.0)) >= min_consolidation_score
        ]
        top_fast = sorted(
            snippets,
            key=lambda item: (
                -abs(float(item.get("score", 0.0))),
                -int(item.get("uses", 0)),
                str(item.get("text", "")),
            ),
        )[:top_n]
        top_tokens = sorted(
            self.slow_model.unigrams.items(),
            key=lambda pair: (-pair[1], pair[0]),
        )[:top_n]
        top_penalties = sorted(
            self.slow_model.penalties.items(),
            key=lambda pair: (-pair[1], pair[0]),
        )[:top_n]
        bigram_count = sum(len(bucket) for bucket in self.slow_model.bigrams.values())
        fast_capacity = self.context_pool.capacity
        recommendations: list[str] = []
        if not self.slow_model.unigrams:
            recommendations.append("Replay history or observe positive rollouts to build slow weights.")
        if not snippets:
            recommendations.append("Observe rollouts to build fast context.")
        if ready:
            recommendations.append("Run consolidate or rely on auto-maintenance to promote strong fast context.")
        if snippets and len(snippets) >= fast_capacity:
            recommendations.append("Fast context is at capacity; consolidate or decay before adding more feedback.")
        if self.slow_model.penalties and not positive:
            recommendations.append("Current fast context is only negative; add positive examples before ranking.")
        if not recommendations:
            recommendations.append("Learner has slow weights and fast context; continue collecting feedback.")

        return {
            "slow": {
                "vocab": len(self.slow_model.unigrams),
                "bigrams": bigram_count,
                "penalties": len(self.slow_model.penalties),
                "top_tokens": [
                    {"token": token, "weight": weight}
                    for token, weight in top_tokens
                ],
                "top_penalties": [
                    {"token": token, "weight": weight}
                    for token, weight in top_penalties
                ],
            },
            "caseops": _caseops_audit(
                self.slow_model.unigrams,
                self.slow_model.penalties,
                snippets,
                top_n=top_n,
            ),
            "model_language": {
                **self.model_language,
                "pressure": _model_language_pressure(
                    self.model_language,
                    self.slow_model.unigrams,
                    snippets,
                    top_n=top_n,
                ),
            },
            "mlp": {
                "weight": self.mlp_weight,
                "trained_examples": self.mlp_model.trained_examples,
                "input_dim": self.mlp_model.input_dim,
                "hidden_dim": self.mlp_model.hidden_dim,
                "learning_rate": self.mlp_model.learning_rate,
            },
            "fast": {
                "snippets": len(snippets),
                "capacity": fast_capacity,
                "positive": len(positive),
                "negative": len(negative),
                "consolidation_ready": len(ready),
                "top_snippets": top_fast,
            },
            "recommendations": tuple(recommendations),
        }

    def consolidate_fast_context(
        self,
        *,
        min_score: float = 0.5,
        reward: float = 0.5,
        decay: float = 0.85,
        min_abs_score: float = 0.05,
    ) -> dict[str, int]:
        """Promote strong fast memories into slow weights, then decay fast state."""
        if min_score < 0.0:
            raise ValueError("min_score must be non-negative")
        if not 0.0 <= reward <= 1.0:
            raise ValueError("reward must be in [0.0, 1.0]")
        candidates = self.context_pool.consolidation_candidates(
            min_score=min_score
        )
        protected_prefixes = tuple(self.model_language.get("protected_prefixes", ()))
        protected_seen = sum(
            1
            for text in candidates
            for tok in set(_tokens(text))
            if _is_protected_feature_token(tok, protected_prefixes)
        )
        for text in candidates:
            self.slow_model.observe(
                RolloutExample(
                    prompt="micro_llm consolidation",
                    response=text,
                    metadata={"source": "fast_context"},
                ),
                Feedback(
                    reward=reward,
                    notes="promoted from fast context",
                    tags=("consolidated",),
                ),
                protected_prefixes=protected_prefixes,
            )
        decay_stats = self.context_pool.decay(
            factor=decay,
            min_abs_score=min_abs_score,
        )
        return {
            "promoted": len(candidates),
            "kernel_tokens_seen": protected_seen,
            "fast_before": decay_stats["before"],
            "fast_after": decay_stats["after"],
            "fast_removed": decay_stats["removed"],
        }

    def consolidate_caseops_evidence(
        self,
        *,
        min_support: int = 2,
        reward: float = 0.35,
        min_abs_weight: float = 0.0,
        limit: int = 8,
    ) -> dict[str, int]:
        """Promote evidence-gated CaseOps links into durable slow tokens."""
        if min_support < 1:
            raise ValueError("min_support must be positive")
        if not 0.0 <= reward <= 1.0:
            raise ValueError("reward must be in [0.0, 1.0]")
        if min_abs_weight < 0.0:
            raise ValueError("min_abs_weight must be non-negative")
        limit = max(1, int(limit))
        evidence = caseops_consolidation_evidence(
            self,
            top_n=limit,
            min_support=min_support,
            min_abs_weight=min_abs_weight,
        )
        rows = evidence.get("links") if isinstance(evidence, dict) else ()
        if not isinstance(rows, (list, tuple)):
            rows = ()
        promoted = 0
        skipped_existing = 0
        skipped_not_ready = 0
        for row in rows:
            if not isinstance(row, dict):
                continue
            if row.get("status") != "ready":
                skipped_not_ready += 1
                continue
            token = _caseops_consolidated_target_token(row)
            if not token:
                skipped_not_ready += 1
                continue
            if token in self.slow_model.unigrams:
                skipped_existing += 1
                continue
            response = _caseops_consolidated_response(row, token)
            if not response:
                skipped_not_ready += 1
                continue
            self.slow_model.observe(
                RolloutExample(
                    prompt="micro_llm caseops evidence consolidation",
                    response=response,
                    metadata={
                        "source": "caseops_consolidation_evidence",
                        "target": str(row.get("target") or ""),
                    },
                ),
                Feedback(
                    reward=reward,
                    notes="promoted from evidence-gated CaseOps links",
                    tags=("caseops", "caseops-consolidated", "consolidated"),
                ),
            )
            promoted += 1
        return {
            "caseops_promoted": promoted,
            "caseops_considered": len(rows),
            "caseops_skipped_existing": skipped_existing,
            "caseops_skipped_not_ready": skipped_not_ready,
        }

    def consolidate_caseops_gap_repairs(
        self,
        *,
        min_support: int = 1,
        reward: float = 0.25,
        min_abs_weight: float = 0.0,
        limit: int = 8,
    ) -> dict[str, int]:
        """Promote recalled CaseOps gap repairs into durable slow tokens."""
        if min_support < 1:
            raise ValueError("min_support must be positive")
        if not 0.0 <= reward <= 1.0:
            raise ValueError("reward must be in [0.0, 1.0]")
        if min_abs_weight < 0.0:
            raise ValueError("min_abs_weight must be non-negative")
        limit = max(1, int(limit))
        recalled = caseops_gap_repair_hints(
            self,
            top_n=limit,
            min_score=min_abs_weight,
        )
        rows = recalled.get("hints") if isinstance(recalled, dict) else ()
        if not isinstance(rows, (list, tuple)):
            rows = ()
        promoted = 0
        skipped_existing = 0
        skipped_not_ready = 0
        for row in rows:
            if not isinstance(row, dict):
                continue
            if int(row.get("support_count") or 0) < min_support:
                skipped_not_ready += 1
                continue
            effective = float(
                row.get("effective_weight", row.get("score", 0.0)) or 0.0
            )
            if effective < min_abs_weight:
                skipped_not_ready += 1
                continue
            token = _caseops_gap_repair_token(row)
            if not token:
                skipped_not_ready += 1
                continue
            if token in self.slow_model.unigrams:
                skipped_existing += 1
                continue
            response = _caseops_gap_repair_response(row, token)
            if not response:
                skipped_not_ready += 1
                continue
            self.slow_model.observe(
                RolloutExample(
                    prompt="micro_llm caseops gap repair consolidation",
                    response=response,
                    metadata={
                        "source": "caseops_gap_repair_hints",
                        "target": str(row.get("target") or ""),
                    },
                ),
                Feedback(
                    reward=reward,
                    notes="promoted from learned CaseOps gap repair hints",
                    tags=("caseops", "caseops-gap", "consolidated"),
                ),
            )
            promoted += 1
        return {
            "caseops_gap_repairs_promoted": promoted,
            "caseops_gap_repairs_considered": len(rows),
            "caseops_gap_repairs_skipped_existing": skipped_existing,
            "caseops_gap_repairs_skipped_not_ready": skipped_not_ready,
        }

    def rank_candidates(
        self, prompt: str, candidates: Iterable[str]
    ) -> list[RankedCandidate]:
        ranked = [self.score_candidate(prompt, candidate) for candidate in candidates]
        ranked.sort(key=lambda item: (-item.score, item.text))
        return ranked

    def save(self, path: str | Path) -> None:
        target = Path(path)
        target.write_text(json.dumps(self.to_dict(), indent=2, sort_keys=True), "utf-8")

    @classmethod
    def load(cls, path: str | Path) -> "MicroLearner":
        data = json.loads(Path(path).read_text("utf-8"))
        return cls.from_dict(data)

    def to_dict(self) -> dict[str, Any]:
        return {
            "version": _STATE_VERSION,
            "slow_weight": self.slow_weight,
            "fast_weight": self.fast_weight,
            "kernel_weight": self.kernel_weight,
            "mlp_weight": self.mlp_weight,
            "model_language": self.model_language,
            "slow_model": self.slow_model.to_dict(),
            "context_pool": self.context_pool.to_dict(),
            "mlp_model": self.mlp_model.to_dict(),
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "MicroLearner":
        version = int(data.get("version", 0))
        if version != _STATE_VERSION:
            raise ValueError(f"unsupported micro_llm state version: {version}")
        return cls(
            slow_model=SlowTokenModel.from_dict(data["slow_model"]),
            context_pool=FastContextPool.from_dict(data["context_pool"]),
            mlp_model=TinyMLPScorer.from_dict(data.get("mlp_model")),
            slow_weight=float(data.get("slow_weight", 0.65)),
            fast_weight=float(data.get("fast_weight", 0.35)),
            kernel_weight=float(data.get("kernel_weight", 0.15)),
            mlp_weight=float(data.get("mlp_weight", 0.10)),
            model_language=data.get("model_language"),
        )


def _jaccard(left: set[str], right: set[str]) -> float:
    if not left or not right:
        return 0.0
    return len(left.intersection(right)) / len(left.union(right))

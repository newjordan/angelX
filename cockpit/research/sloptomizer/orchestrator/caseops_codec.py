"""Layered CaseOps internal vocabulary codec registry."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Callable, Iterable, Mapping

from orchestrator.caseops_crystal_codec import encode_caseops_crystal_primer
from orchestrator.caseops_crystal_codec import parse_caseops_crystal_primers
from orchestrator.caseops_taxonomy_codec import encode_caseops_taxonomy
from orchestrator.caseops_taxonomy_codec import parse_caseops_taxonomies
from orchestrator.caseops_vocab_analogy_codec import encode_caseops_vocab_analogy
from orchestrator.caseops_vocab_analogy_codec import parse_caseops_vocab_analogies
from orchestrator.caseops_vocab_tensor_codec import encode_caseops_vocab_tensor
from orchestrator.caseops_vocab_tensor_codec import parse_caseops_vocab_tensors


EncodeFn = Callable[[Mapping[str, Any]], str]
DecodeFn = Callable[[str], Iterable[Mapping[str, Any]]]

_SECTION_RE_TEMPLATE = r"\bcaseops[ _]{layer}[ _]vocab(?:_feedback)?\s*[:=]\s*([^\n]+)"
_SCORE_RE = re.compile(
    r"[-+]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:e[-+]?[0-9]+)?",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class CaseOpsLayerCodec:
    """Encoder/decoder pair for one CaseOps vocabulary layer."""

    name: str
    encode: EncodeFn
    decode: DecodeFn
    aliases: tuple[str, ...] = ()


class CaseOpsCodecRegistry:
    """Registry for layered CaseOps internal vocabulary codecs."""

    def __init__(self, codecs: Iterable[CaseOpsLayerCodec] = ()) -> None:
        self._codecs: dict[str, CaseOpsLayerCodec] = {}
        self._aliases: dict[str, str] = {}
        for codec in codecs:
            self.register(codec)

    def register(self, codec: CaseOpsLayerCodec) -> None:
        """Register or replace a layer codec."""
        name = caseops_codec_piece(codec.name)
        if not name:
            raise ValueError("CaseOps layer codec name must not be empty")
        normalized = CaseOpsLayerCodec(
            name=name,
            encode=codec.encode,
            decode=codec.decode,
            aliases=tuple(
                alias
                for alias in (caseops_codec_piece(item) for item in codec.aliases)
                if alias
            ),
        )
        self._codecs[name] = normalized
        self._aliases[name] = name
        for alias in normalized.aliases:
            self._aliases[alias] = name

    def get(self, layer: str) -> CaseOpsLayerCodec:
        """Return the codec for ``layer`` or raise ``KeyError``."""
        key = self._aliases[caseops_codec_piece(layer)]
        return self._codecs[key]

    def layers(self) -> tuple[str, ...]:
        """Return registered canonical layer names."""
        return tuple(self._codecs)

    def encode(self, row: Mapping[str, Any], *, layer: str | None = None) -> str:
        """Encode one row using its named layer codec."""
        layer_name = caseops_codec_piece(layer or row.get("layer") or "")
        if not layer_name:
            raise ValueError("CaseOps row must include a layer")
        return self.get(layer_name).encode(row)

    def decode(
        self,
        text: str,
        *,
        layers: Iterable[str] | None = None,
    ) -> tuple[dict[str, Any], ...]:
        """Decode rows from all selected layers in ``text``."""
        selected = tuple(layers or self.layers())
        rows: list[dict[str, Any]] = []
        for layer in selected:
            codec = self.get(layer)
            for payload in codec.decode(text):
                row = dict(payload)
                row.setdefault("layer", codec.name)
                row.setdefault("codec", codec.name)
                rows.append(row)
        return tuple(rows)


def caseops_codec_piece(value: Any) -> str:
    """Normalize one layer, key, or vocabulary atom."""
    return re.sub(r"[^a-z0-9_.:-]+", "_", str(value).lower()).strip("_")


def default_caseops_codec_registry() -> CaseOpsCodecRegistry:
    """Return the built-in CaseOps layered codec registry."""
    registry = CaseOpsCodecRegistry()
    for layer in ("surface", "system", "math", "language", "mathlang", "glyph"):
        registry.register(_generic_vocab_codec(layer))
    registry.register(CaseOpsLayerCodec(
        name="taxonomy",
        encode=_encode_taxonomy_layer,
        decode=_decode_taxonomy_layer,
        aliases=("taxonomies", "math_language_taxonomy"),
    ))
    registry.register(CaseOpsLayerCodec(
        name="tensor",
        encode=_encode_tensor_layer,
        decode=_decode_tensor_layer,
        aliases=("vocab_tensor", "vocab_tensors"),
    ))
    registry.register(CaseOpsLayerCodec(
        name="analogy",
        encode=_encode_analogy_layer,
        decode=_decode_analogy_layer,
        aliases=("vocab_analogy", "vocab_analogies"),
    ))
    registry.register(CaseOpsLayerCodec(
        name="crystal",
        encode=_encode_crystal_layer,
        decode=_decode_crystal_layer,
        aliases=("crystal_primer", "crystal_primers"),
    ))
    return registry


def caseops_encode(
    row: Mapping[str, Any],
    *,
    layer: str | None = None,
    registry: CaseOpsCodecRegistry | None = None,
) -> str:
    """Encode one CaseOps row through the layered registry."""
    return (registry or default_caseops_codec_registry()).encode(row, layer=layer)


def caseops_decode(
    text: str,
    *,
    layers: Iterable[str] | None = None,
    registry: CaseOpsCodecRegistry | None = None,
) -> tuple[dict[str, Any], ...]:
    """Decode CaseOps rows from text through the layered registry."""
    return (registry or default_caseops_codec_registry()).decode(text, layers=layers)


def _generic_vocab_codec(layer: str) -> CaseOpsLayerCodec:
    return CaseOpsLayerCodec(
        name=layer,
        encode=lambda row, layer=layer: _encode_generic_vocab(layer, row),
        decode=lambda text, layer=layer: _decode_generic_vocab(layer, text),
    )


def _encode_generic_vocab(layer: str, row: Mapping[str, Any]) -> str:
    kind = caseops_codec_piece(row.get("kind") or "term")
    value = caseops_codec_piece(row.get("value") or row.get("id") or "")
    if not kind or not value:
        return ""
    bits = [f"{kind}:{value}"]
    score = _safe_float(row.get("score"), 0.0)
    if score:
        bits.append(f"score={score:.3f}")
    attrs = _generic_pairs(row.get("attributes") or row.get("attrs") or {})
    if attrs:
        bits.append("attrs=" + ",".join(f"{key}:{value}" for key, value in attrs))
    links = _generic_links(row.get("links") or row.get("relations") or ())
    if links:
        bits.append("links=" + ",".join(links))
    sublayers = _generic_values(row.get("layers") or (layer,))
    if sublayers:
        bits.append("layers=" + ",".join(sublayers))
    meta = _generic_pairs(row.get("metadata") or row.get("meta") or {})
    if meta:
        bits.append("meta=" + ",".join(f"{key}:{value}" for key, value in meta))
    return " ".join(bits)


def _decode_generic_vocab(layer: str, text: str) -> tuple[dict[str, Any], ...]:
    section_re = re.compile(
        _SECTION_RE_TEMPLATE.format(layer=re.escape(layer).replace("\\_", "[ _]")),
        re.IGNORECASE,
    )
    rows: list[dict[str, Any]] = []
    for match in section_re.finditer(str(text or "")):
        for item in match.group(1).split(";"):
            parsed = _parse_generic_vocab_item(layer, item)
            if parsed:
                rows.append(parsed)
    return _dedupe_generic_rows(rows)


def _parse_generic_vocab_item(
    layer: str,
    item: str,
) -> dict[str, Any] | None:
    fields = [field for field in str(item or "").strip().split() if field.strip()]
    if not fields:
        return None
    kind, sep, value = fields[0].partition(":")
    clean_kind = caseops_codec_piece(kind)
    clean_value = caseops_codec_piece(value)
    if not sep or not clean_kind or not clean_value:
        return None
    row: dict[str, Any] = {
        "layer": layer,
        "kind": clean_kind,
        "value": clean_value,
        "attributes": {},
        "links": (),
        "layers": (),
        "metadata": {},
    }
    for field in fields[1:]:
        key, field_sep, raw_value = field.partition("=")
        if not field_sep:
            continue
        clean_key = caseops_codec_piece(key)
        if clean_key == "score":
            row["score"] = _safe_float(raw_value, 0.0)
        elif clean_key in {"attr", "attrs", "attributes"}:
            row["attributes"] = dict(_parse_pairs(raw_value))
        elif clean_key in {"link", "links", "relations"}:
            row["links"] = _parse_links(raw_value)
        elif clean_key in {"layer", "layers"}:
            row["layers"] = _parse_values(raw_value)
        elif clean_key in {"meta", "metadata"}:
            row["metadata"] = dict(_parse_pairs(raw_value))
    return row


def _encode_taxonomy_layer(row: Mapping[str, Any]) -> str:
    return encode_caseops_taxonomy(dict(row))


def _decode_taxonomy_layer(text: str) -> tuple[dict[str, Any], ...]:
    return tuple(dict(row, layer="taxonomy") for row in parse_caseops_taxonomies(text))


def _encode_tensor_layer(row: Mapping[str, Any]) -> str:
    return encode_caseops_vocab_tensor(dict(row))


def _decode_tensor_layer(text: str) -> tuple[dict[str, Any], ...]:
    return tuple(dict(row, layer="tensor") for row in parse_caseops_vocab_tensors(text))


def _encode_analogy_layer(row: Mapping[str, Any]) -> str:
    return encode_caseops_vocab_analogy(dict(row))


def _decode_analogy_layer(text: str) -> tuple[dict[str, Any], ...]:
    return tuple(dict(row, layer="analogy") for row in parse_caseops_vocab_analogies(text))


def _encode_crystal_layer(row: Mapping[str, Any]) -> str:
    return encode_caseops_crystal_primer(dict(row))


def _decode_crystal_layer(text: str) -> tuple[dict[str, Any], ...]:
    return tuple(dict(row, layer="crystal") for row in parse_caseops_crystal_primers(text))


def _generic_values(values: Iterable[Any] | str) -> tuple[str, ...]:
    if isinstance(values, str):
        values = values.split(",")
    return tuple(dict.fromkeys(
        clean
        for clean in (caseops_codec_piece(value) for value in values or ())
        if clean
    ))


def _generic_pairs(values: Mapping[Any, Any]) -> tuple[tuple[str, str], ...]:
    if not isinstance(values, Mapping):
        return ()
    rows = []
    for key, value in values.items():
        clean_key = caseops_codec_piece(key)
        clean_value = caseops_codec_piece(value)
        if clean_key and clean_value:
            rows.append((clean_key, clean_value))
    rows.sort(key=lambda item: item[0])
    return tuple(rows)


def _generic_links(values: Iterable[Any] | Mapping[str, Any]) -> tuple[str, ...]:
    if isinstance(values, Mapping):
        values = (values,)
    rows: list[str] = []
    for value in values or ():
        encoded = _generic_link(value)
        if encoded:
            rows.append(encoded)
    return tuple(dict.fromkeys(rows))


def _generic_link(value: Any) -> str:
    if isinstance(value, str):
        link, *rest = value.split("~")
        source, sep, target = link.partition("->")
        clean_source = caseops_codec_piece(source)
        clean_target = caseops_codec_piece(target)
        if not sep or not clean_source or not clean_target:
            return ""
        bits = [f"{clean_source}->{clean_target}"]
        if rest:
            relation = caseops_codec_piece(rest[0])
            if relation:
                bits.append(relation)
        if len(rest) > 1 and _SCORE_RE.fullmatch(rest[1].strip()):
            bits.append(f"{_safe_float(rest[1], 0.0):+.3f}")
        return "~".join(bits)
    if not isinstance(value, Mapping):
        return ""
    source = caseops_codec_piece(value.get("source") or "")
    target = caseops_codec_piece(value.get("target") or "")
    if not source or not target:
        return ""
    bits = [f"{source}->{target}"]
    relation = caseops_codec_piece(value.get("relation") or "")
    if relation:
        bits.append(relation)
    weight = _safe_float(value.get("weight"), 0.0)
    if weight:
        bits.append(f"{weight:+.3f}")
    return "~".join(bits)


def _parse_values(raw: str) -> tuple[str, ...]:
    return _generic_values(str(raw or "").split(","))


def _parse_pairs(raw: str) -> tuple[tuple[str, str], ...]:
    rows: list[tuple[str, str]] = []
    for part in str(raw or "").split(","):
        key, sep, value = part.partition(":")
        clean_key = caseops_codec_piece(key)
        clean_value = caseops_codec_piece(value)
        if sep and clean_key and clean_value:
            rows.append((clean_key, clean_value))
    return tuple(dict(rows).items())


def _parse_links(raw: str) -> tuple[dict[str, Any], ...]:
    rows: list[dict[str, Any]] = []
    for part in str(raw or "").split(","):
        encoded = _generic_link(part)
        if not encoded:
            continue
        link, *rest = encoded.split("~")
        source, _, target = link.partition("->")
        row: dict[str, Any] = {"source": source, "target": target}
        if rest:
            row["relation"] = rest[0]
        if len(rest) > 1:
            row["weight"] = _safe_float(rest[1], 0.0)
        rows.append(row)
    return tuple(rows)


def _dedupe_generic_rows(
    rows: Iterable[dict[str, Any]],
) -> tuple[dict[str, Any], ...]:
    out: list[dict[str, Any]] = []
    seen: set[tuple[Any, ...]] = set()
    for row in rows:
        key = (
            row.get("layer"),
            row.get("kind"),
            row.get("value"),
            tuple(sorted((row.get("attributes") or {}).items())),
            tuple(
                (
                    link.get("source"),
                    link.get("target"),
                    link.get("relation"),
                    round(float(link.get("weight") or 0.0), 6),
                )
                for link in row.get("links", ())
                if isinstance(link, Mapping)
            ),
            tuple(row.get("layers", ()) or ()),
            tuple(sorted((row.get("metadata") or {}).items())),
            round(float(row.get("score") or 0.0), 6),
        )
        if key in seen:
            continue
        seen.add(key)
        out.append(dict(row))
    return tuple(out)


def _safe_float(value: Any, default: float) -> float:
    try:
        return float(value)
    except Exception:  # noqa: BLE001
        return float(default)

#!/usr/bin/env python3
"""Offline card-art review sheet. No provider calls or third-party dependencies.

python3 scripts/delve/card_gallery.py --output /tmp/delve-cards.svg
Open the SVG at 100% zoom: each card has a native 16px sample and an 8x sample.
This is a review aid, not visual approval or a gameplay screenshot.
"""
import argparse
import html
import json
from pathlib import Path

from inks_hex import INK_RS, inks

ROOT = Path(__file__).resolve().parents[2]


def load_card(path, palette):
    text = path.read_text(encoding="utf-8")
    header, separator, art = text.partition("\nart\n")
    if not separator:
        raise ValueError(f"{path}: missing art")
    rows = art.splitlines()
    if not rows or len(rows) > 16 or any(not row or len(row) > 16 for row in rows):
        raise ValueError(f"{path}: expected nonempty art no larger than 16x16")
    unknown = set("".join(rows)) - set(palette) - {"."}
    if unknown:
        raise ValueError(f"{path}: unknown inks {sorted(unknown)!r}")
    fields = dict(line.split(None, 1) for line in header.splitlines() if len(line.split(None, 1)) == 2)
    return fields.get("name", path.stem), fields.get("kind", "?"), rows


def sprite(rows, palette, x, y, scale):
    parts = [f'<g transform="translate({x} {y}) scale({scale})" shape-rendering="crispEdges">']
    for j, row in enumerate(rows):
        for i, key in enumerate(row):
            if key != ".":
                parts.append(f'<rect x="{i}" y="{j}" width="1" height="1" fill="#{palette[key]}"/>')
    return "".join(parts) + "</g>"


def gallery(paths, palette):
    cards = [(p, *load_card(p, palette)) for p in sorted(paths)]
    width, cell_w, cell_h, columns = 1200, 300, 220, 4
    height = 70 + ((len(cards) + columns - 1) // columns) * cell_h
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
           '<rect width="100%" height="100%" fill="#171b25"/>',
           '<g font-family="monospace" fill="#eee">',
           '<text x="16" y="24">Card art review — native 16px + nearest-neighbour 8x</text>',
           f'<text x="16" y="46">{len(cards)} cards; actual game inks. Review at 100% zoom. Not an approval.</text>']
    for n, (path, name, kind, rows) in enumerate(cards):
        x, y = (n % columns) * cell_w, 70 + (n // columns) * cell_h
        label = html.escape(name)
        out.append(f'<g><title>{html.escape(path.name)}: {label} ({html.escape(kind)})</title>')
        out.append(f'<text x="{x+12}" y="{y+18}" font-size="12">{label}</text>')
        out.append(f'<text x="{x+12}" y="{y+36}" font-size="11">{html.escape(kind)}</text>')
        # Compare dark and light backdrops without changing sprite colours.
        for dx, bg in ((12, "#090e18"), (154, "#c9c3b6")):
            out.append(f'<rect x="{x+dx}" y="{y+48}" width="132" height="132" fill="{bg}"/>')
            out.append(sprite(rows, palette, x+dx+2, y+50, 8))
        # Native-size contrast matters too: enlarged previews can hide weak silhouettes.
        for dx, bg in ((12, "#090e18"), (154, "#c9c3b6")):
            out.append(f'<rect x="{x+dx}" y="{y+188}" width="20" height="20" fill="{bg}"/>')
            out.append(sprite(rows, palette, x+dx+2, y+190, 1))
        out.append('</g>')
    return "\n".join(out + ['</g></svg>'])


def audit(paths, palette):
    """Deterministic review hints, not aesthetic scores. Compare rendered RGB art."""
    records, signatures, masks = [], {}, []
    for path in sorted(paths):
        name, kind, rows = load_card(path, palette)
        pixels = {(x, y): palette[key].lower()
                  for y, row in enumerate(rows) for x, key in enumerate(row) if key != "."}
        remaining = set(pixels)
        components = []
        while remaining:
            seed = min(remaining)
            remaining.remove(seed)
            stack, size = [seed], 0
            while stack:
                x, y = stack.pop()
                size += 1
                for dx in (-1, 0, 1):
                    for dy in (-1, 0, 1):
                        neighbour = (x + dx, y + dy)
                        if neighbour in remaining:
                            remaining.remove(neighbour)
                            stack.append(neighbour)
            components.append(size)
        # The renderer places every icon on the same 16x16 canvas. Missing
        # trailing cells/rows are transparent, not a visual distinction.
        signature = tuple(tuple(pixels.get((x, y)) for x in range(16))
                          for y in range(16))
        signatures.setdefault(signature, []).append(str(path))
        masks.append((str(path), frozenset(pixels), signature))
        records.append({"path": str(path), "name": name, "kind": kind,
                        "opaque_pixels": len(pixels), "colours": len(set(pixels.values())),
                        "components_8_connected": sorted(components, reverse=True),
                        "review_hints": (["fully transparent"] if not pixels else []) +
                                        (["isolated pixels (may be intentional sparks)"]
                                         if len(components) > 1 and 1 in components else [])})
    # Fixed-canvas Jaccard overlap deliberately ignores colour: a recolour
    # should not hide a recycled silhouette. Do not align or shift sprites,
    # and exclude tiny glyphs where overlap is not useful evidence.
    similar = []
    translated = []

    def normalized(mask):
        if not mask:
            return frozenset()
        x0 = min(x for x, _ in mask)
        y0 = min(y for _, y in mask)
        return frozenset((x - x0, y - y0) for x, y in mask)

    for i, (left, a, image_a) in enumerate(masks):
        for right, b, image_b in masks[i + 1:]:
            if min(len(a), len(b)) < 16 or image_a == image_b:
                continue
            na, nb = normalized(a), normalized(b)
            ni, nu = len(na & nb), len(na | nb)
            # Separate signal: placement on the canvas can be intentional.
            if na != a or nb != b:
                if ni * 100 >= nu * 90 and len(a & b) * 100 < len(a | b) * 90:
                    translated.append({"paths": [left, right],
                                       "intersection_pixels": ni,
                                       "union_pixels": nu,
                                       "same_silhouette": na == nb})
            intersection, union = len(a & b), len(a | b)
            if intersection * 100 >= union * 90:
                similar.append({"paths": [left, right],
                                "intersection_pixels": intersection,
                                "union_pixels": union,
                                "same_silhouette": a == b})
    return {"schema": "delve.card-art-audit/v1", "approval": False,
            "similar_silhouettes": similar,
            "translated_silhouettes": translated,
            "cards": records,
            "exact_rendered_duplicates": [v for v in signatures.values() if len(v) > 1]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cards", nargs="*", type=Path, help="Specific cards; default: entire library")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--audit", type=Path, help="Optional JSON review hints; not visual approval")
    args = parser.parse_args()
    palette = {key: rgb for key, rgb, _ in inks(INK_RS.read_text())}
    paths = args.cards or list((ROOT / "cockpit/assets/dungeon/cards").glob("*.card"))
    if not paths:
        parser.error("no cards found")
    result = gallery(paths, palette)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(result, encoding="utf-8")
    if args.audit:
        args.audit.parent.mkdir(parents=True, exist_ok=True)
        args.audit.write_text(json.dumps(audit(paths, palette), indent=2) + "\n", encoding="utf-8")
    print(f"Rendered {len(paths)} cards to {args.output}")


if __name__ == "__main__":
    main()

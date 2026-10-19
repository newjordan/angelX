#!/usr/bin/env python3
"""Rebuild the six-game art inventory using the request blocks and local sources.

No rendering, image processing, third-party packages, or runtime changes. Request
prose belongs in docs/ART-REQUESTS.md; dimensions, recipes, constants, enums and
timer expressions are read from the repository. --check never writes files.
"""

import argparse
import ast
import fnmatch
import hashlib
import json
import operator
from pathlib import Path
import re
import struct

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "crates/delve/assets"
DOC = ROOT / "docs/ART-REQUESTS.md"
OUT = ASSETS / "art-requests.json"
GAMES = ("cinder", "latch", "mossbell", "brasswake", "rafterwake", "pennon")
OPS = {ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul}


def reference(path, line=1):
    return {"path": str(path.relative_to(ROOT)), "line": line}


def number(expression, names):
    """Evaluate only literal arithmetic used by scalar Rust constants."""
    def visit(node):
        if isinstance(node, ast.Constant) and type(node.value) in (int, float):
            return node.value
        if isinstance(node, ast.Name):
            return names[node.id]
        if isinstance(node, (ast.Tuple, ast.List)):
            return [visit(n) for n in node.elts]
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.USub):
            return -visit(node.operand)
        if isinstance(node, ast.BinOp) and type(node.op) in OPS:
            return OPS[type(node.op)](visit(node.left), visit(node.right))
        raise ValueError(expression)
    try:
        return visit(ast.parse(expression.replace(" as i32", ""), mode="eval").body)
    except (ValueError, SyntaxError, KeyError):
        return None  # Unsupported Rust syntax and dynamic expressions stay verbatim.


def facts(path, names):
    text = path.read_text()
    constants = {}
    for m in re.finditer(r"(?:pub(?:\([^)]*\))?\s+)?const\s+(\w+)\s*:\s*([^=]+?)=\s*([^;]+);", text):
        name, rust_type, expression = (v.strip() for v in m.groups())
        value = number(expression, names)
        constants[name] = {"rust_type": rust_type, "expression": expression,
                           "value": value, "source": reference(path, text.count("\n", 0, m.start()) + 1)}
        if value is not None:
            names[name] = value
    enums = {}
    for m in re.finditer(r"pub enum (\w+)\s*\{([^}]+)\}", text):
        body = re.sub(r"#\[[^]]+\]|//[^\n]*", "", m[2])
        enums[m[1]] = {"names": re.findall(r"\b(\w+)\s*,", body),
                        "source": reference(path, text.count("\n", 0, m.start()) + 1)}
    timers = []
    fields = "ticks|windup|tell|attack|flash|freeze|flinch|effect|hurt|down|save|burst|cooldown|pose_ticks|beat_left|brick_tell|brick_burst|hit|impact|retry|left|right"
    for m in re.finditer(rf"\b(?:self|e|f|c)\.({fields})\s*=(?!=)\s*([^;]+);", text):
        # These are evidence, not an inferred animation scheduler. Keep formulas.
        timers.append({"field": m[1], "expression": " ".join(m[2].split()),
                       "source": reference(path, text.count("\n", 0, m.start()) + 1)})
    initializers = [{"field": m[1], "expression": m[2].strip(),
                     "source": reference(path, text.count("\n", 0, m.start()) + 1)}
                    for m in re.finditer(rf"^\s*({fields}):\s*([^,\n]+),", text, re.M)]
    return {"constants": constants, "enums": enums, "timer_assignments": timers,
            "timer_initializers": initializers}


def png(path):
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"Not a PNG: {path}")
    width, height = struct.unpack(">II", data[16:24])
    return {"name": path.stem, "canvas_px": [width, height],
            "sha256": hashlib.sha256(data).hexdigest(), "source": reference(path)}


def recipes(game):
    path = ASSETS / game / "native.json"
    if not path.exists():
        return {}
    recipe = json.loads(path.read_text())
    result = {}
    for bank in ("assets", "states", "tells"):
        for name, asset in recipe.get(bank, {}).items():
            if "rows" not in asset:
                continue
            rows = asset["rows"]
            if not rows or len({len(r) for r in rows}) != 1:
                raise ValueError(f"Invalid recipe rows: {game}/{name}")
            result[name] = {"canvas_px": [len(rows[0]), len(rows)],
                            "source": {**reference(path), "json_pointer": f"/{bank}/{name}"},
                            "damage_variant_count": len(asset.get("damage_variants", [])),
                            "damage_variants": [
                                {"stage": i + 1,
                                 "canvas_px": [len(v["rows"][0]), len(v["rows"])],
                                 "json_pointer": f"/{bank}/{name}/damage_variants/{i}"}
                                for i, v in enumerate(asset.get("damage_variants", []))],
                            "linear_feature_names": [f["name"] for f in asset.get("linear_features", [])],
                            "signature_lights": [
                                {"name": light["name"], "pixel_count": len(light.get("pixels", [])),
                                 "json_pointer": f"/{bank}/{name}/signature_lights/{i}"}
                                for i, light in enumerate(asset.get("signature_lights", []))]}
    return result


def request_blocks():
    text = DOC.read_text()
    blocks = []
    for m in re.finditer(r"^### ([a-z]+\.[a-z0-9-]+) — ([^\n]+)\n(.*?)(?=^###? |\Z)", text, re.M | re.S):
        asset_id, title, body = m.groups()
        fields = dict(re.findall(r"^- ([A-Za-z]+): (.+)$", body, re.M))
        required = {"Role", "View", "Files", "States", "Timing", "Fit", "Geometry", "Brief", "Need", "Slot"}
        if set(fields) != required:
            raise ValueError(f"{asset_id}: missing/extra fields {required ^ set(fields)}")
        blocks.append({"id": asset_id, "name": title, "game": asset_id.split(".")[0],
                       "request_source": reference(DOC, text.count("\n", 0, m.start()) + 1),
                       **{k.lower(): v for k, v in fields.items()}})
    if len({b["id"] for b in blocks}) != len(blocks):
        raise ValueError("Duplicate request id")
    if {b["game"] for b in blocks} != set(GAMES):
        raise ValueError("Request blocks must cover all six games")
    return blocks


def build():
    rule_root = ROOT / "crates/delve/src/rules"
    render_root = ROOT / "crates/delve/src/render/overworld/arena"
    hz_path = ROOT / "crates/delve/src/rules.rs"
    hz = int(re.search(r"pub const HZ: u32 = (\d+);", hz_path.read_text())[1])
    blocks = request_blocks()
    games = []
    inputs = {DOC, hz_path, Path(__file__).resolve(), ASSETS / 'art-slots/cell-layouts.json'}
    for game in GAMES:
        rule = rule_root / f"{game}.rs"
        render = render_root / f"{game}.rs"
        inputs.update((rule, render))
        names = {"HZ": hz}
        rule_facts = facts(rule, names)
        for module, imports in re.findall(r"pub use super::(\w+)::\{([^}]+)\};", rule.read_text()):
            parent = rule_root / f"{module}.rs"
            inherited = facts(parent, {"HZ": hz})
            inputs.add(parent)
            rule_facts[f"inherited_from_{module}"] = {
                name.strip(): inherited["enums"].get(name.strip(), inherited["constants"].get(name.strip()))
                for name in imports.split(",") if name.strip()}
        render_facts = facts(render, {"HZ": hz})
        recipe = recipes(game)
        files = {p.stem: png(p) for p in sorted((ASSETS / game).glob("*.png"))}
        inputs.update((ASSETS / game).glob("*.png"))
        inputs.update((ASSETS / game).glob("*.json"))
        covered = set()
        assets = []
        for block in (b for b in blocks if b["game"] == game):
            selected = {}
            for pattern in block["files"].split(", "):
                if pattern == "none":
                    continue
                matches = [p for p in sorted(ASSETS.glob(pattern)) if p.suffix == ".png"]
                if not matches:
                    raise ValueError(f"{block['id']}: empty file pattern {pattern}")
                for path in matches:
                    selected[path.stem] = files[path.stem]
            if covered.intersection(selected):
                raise ValueError(f"{block['id']}: PNG assigned to multiple requests")
            covered.update(selected)
            groups = []
            for spec in block["states"].split("; "):
                label, pattern = spec.split("=", 1)
                if pattern.startswith("@"):
                    count = None if pattern == "@unknown" else int(pattern[1:])
                    groups.append({"state": label, "procedural_pose_count": count,
                                   "evidence": block["geometry"]})
                    continue
                matched = sorted(n for n in selected if fnmatch.fnmatchcase(n, pattern))
                if not matched:
                    raise ValueError(f"{block['id']}: empty state pattern {pattern}")
                groups.append({"state": label, "source_pose_count": len(matched), "names": matched,
                               "kind": "variants" if "variants" in label else "poses"})
            grouped = {n for group in groups for n in group.get("names", [])}
            if grouped != set(selected):
                raise ValueError(f"{block['id']}: source poses missing from States")
            for name in selected.keys() & recipe.keys():
                if selected[name]["canvas_px"] != recipe[name]["canvas_px"]:
                    raise ValueError(f"{block['id']}: PNG/recipe canvas mismatch for {name}")
            assets.append({**block, "slot_kind": block["slot"].split(";", 1)[0],
                           "frame_groups": groups, "source_frames": list(selected.values()),
                           "recipe_frames": {n: recipe[n] for n in selected if n in recipe},
                           "larger_canvas_without_integration_changes": "unknown, test"})
        # A newly added sprite must acquire a request block rather than disappear.
        if set(files) != covered:
            raise ValueError(f"{game}: unassigned PNGs {sorted(set(files) - covered)}")
        games.append({"id": game, "rules": reference(rule), "renderer": reference(render),
                      "rule_facts": rule_facts, "render_facts": render_facts,
                      "registration_metadata": json.loads((ASSETS / game / "anchors.json").read_text())
                      if (ASSETS / game / "anchors.json").exists() else None,
                      "request_count": len(assets), "png_count": len(files), "assets": assets})
    ink = ROOT / "crates/delve/src/render/overworld/ink.rs"
    inputs.add(ink)
    colours = re.findall(r"'([^']+)' => 0x([0-9a-fA-F]{6}),", ink.read_text())
    sections = dict(re.findall(r"^## ([^\n]+)\n(.*?)(?=^## |\Z)", DOC.read_text(), re.M | re.S))
    runtime_slots = []
    for path in sorted((ASSETS / 'art-slots').glob('*/*.map.json')):
        mapping = json.loads(path.read_text())
        if not mapping.get('runtime'):
            continue
        frames = []
        for piece in mapping['pieces']:
            if not (piece.get('approved') and piece.get('runtime')):
                continue
            native = path.parent / 'v1' / (piece['name'] + '.png')
            if not native.exists():
                continue  # The optional loader keeps the original fallback.
            frame = png(native)
            mask = native.with_name(native.stem + '-seat-mask.png')
            if mask.exists():
                frame['seat_mask_sha256'] = hashlib.sha256(mask.read_bytes()).hexdigest()
            frames.append(frame)
        controls = [p for p in sorted(path.parent.iterdir()) if p.suffix in ('.json', '.py')]
        # Includes reviewed sparse erase/material data, preparation recipes,
        # registration and grades; final in-memory banks are audited separately.
        runtime_slots.append({'id': path.parent.name, 'frames': frames,
                              'review_inputs': [{**reference(p), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in controls]})
    return {"schema_version": 1, "status": "request-specification-not-a-runtime-loader",
            "engine_hz": hz, "hz_source": reference(hz_path),
            "manual_field_policy": "Identity, fit, geometry, timing prose and procedural counts require source review; extracted facts and hashes detect drift but are not a Rust parser.",
            "palette": {"source": reference(ink), "ink_rgb_hex": dict(colours)},
            "shared_specification": {title: body.strip() for title, body in sections.items()
                                     if not any(title.startswith(g + " —") for g in GAMES)},
            "provenance": [{**reference(p), "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
                           for p in sorted(inputs)],
            "runtime_slots": runtime_slots,
            "request_count": len(blocks), "games": games}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if the committed JSON is stale")
    args = parser.parse_args()
    result = build()
    output = json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    if args.check:
        if not OUT.exists() or OUT.read_text() != output:
            raise SystemExit("Art requests are stale; run python3 scripts/art-requests.py")
    else:
        OUT.write_text(output)
    print(f"{result['request_count']} request slots; " + ", ".join(
        f"{g['id']} {g['request_count']} slots/{g['png_count']} PNGs" for g in result["games"]))


if __name__ == "__main__":
    main()

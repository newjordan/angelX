#!/usr/bin/env python3
"""Build brief 3 sprites; preserve earlier assets, the chest base and sconce metal.

Requires Pillow. Retained imagegen originals, crops and ink patches are sufficient
to reproduce the final assets offline. --from-ink preserves direct ink edits.
"""

import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

from build import PALETTE, SIGNAL, nearest
from build_sprites import braille_preview

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / "tools"
SPRITES = ROOT / "sprites"
LAYOUT = json.loads((TOOLS / "sprite-3-layout.json").read_text())
FIRE = "4@567a8"
GOLD = "4@56$"
GEM = "2"
IRON_COLUMNS = (4, 5, 14, 15)
BASE_ROW = 8
FLAME_END = 8
SCALE = 6


def read_ink(path):
    return [list(row) for row in path.read_text().splitlines()]


def render(grid):
    image = Image.new("RGBA", (len(grid[0]), len(grid)))
    image.putdata([
        (0, 0, 0, 0) if ink == "." else (*PALETTE[ink], 255)
        for row in grid for ink in row
    ])
    return image


def fitted_source(name):
    spec = LAYOUT[name]
    canvas = Image.new("RGBA", tuple(spec["size"]))
    pieces = spec.get("pieces", [spec])
    with Image.open(SPRITES / "sources" / f"{name}.png") as original:
        for piece in pieces:
            left, top, right, bottom = piece["box"]
            source = original.convert("RGBA").crop(tuple(piece["source_box"]))
            source = source.resize((right - left, bottom - top), Image.Resampling.BOX)
            canvas.paste(source, (left, top))
    return canvas


def quantize(name):
    image = fitted_source(name)
    grid = []
    for y in range(image.height):
        row = []
        for x in range(image.width):
            r, g, b, alpha = image.getpixel((x, y))
            if alpha < 100:
                row.append(".")
                continue
            allowed = LAYOUT[name]["inks"]
            if name.startswith("sconce") and y < FLAME_END:
                allowed = FIRE
            elif name == "chest_open" and 5 <= y < BASE_ROW:
                if g > r * 1.1 and b > r:
                    allowed = GEM
                elif r >= 145 and g >= 100 and r > b * 1.4:
                    allowed = GOLD
            elif name == "banner":
                # The pale heraldry and iron rod are matte gray. Rust-red cloth
                # uses legal nonemissive inks, never fire's brighter red 7.
                if max(r, g, b) - min(r, g, b) < 35:
                    allowed = "kKXgjGJhi"
                else:
                    allowed = "XbIBpR"
            row.append(nearest((r, g, b), allowed))
        grid.append(row)
    return grid


def retouch(grids):
    # Literal ink patches: '_' preserves the source, '.' erases it.
    operations = json.loads((TOOLS / "sprite-3-retouch.json").read_text())
    for name, patches in operations.items():
        for patch in patches:
            for dy, row in enumerate(patch["rows"]):
                for dx, ink in enumerate(row):
                    if ink != "_":
                        assert ink == "." or ink in PALETTE, (name, ink)
                        grids[name][patch["y"] + dy][patch["x"] + dx] = ink


def lock_animation_parts(grids):
    chest = read_ink(SPRITES / "chest.ink")
    opened = grids["chest_open"]
    for y in range(BASE_ROW, 16):
        opened[y] = chest[y][:]
    # Preserve the original bands' position, width, highlights and shadows.
    for y in range(1, 15):
        for x in IRON_COLUMNS:
            opened[y][x] = chest[y][x]
    # One authoritative fixture beneath the flame: no head/shaft/bracket drift.
    for y in range(FLAME_END, 20):
        grids["sconce_b"][y] = grids["sconce_a"][y][:]


def wall(size):
    texture = render(read_ink(ROOT / "wall_stone.ink")).convert("RGB")
    image = Image.new("RGB", size)
    for y in range(0, size[1], texture.height):
        for x in range(0, size[0], texture.width):
            image.paste(texture, (x, y))
    return image.convert("RGBA")


def context(images):
    image = wall((176, 56))
    draw = ImageDraw.Draw(image)
    draw.rectangle((0, 48, 175, 55), fill=(*PALETTE["K"], 255))
    draw.line((0, 48, 175, 48), fill=(*PALETTE["X"], 255))
    for name, xy in (
        ("sconce_a", (34, 8)), ("banner", (82, 4)),
        ("sconce_b", (130, 8)), ("chest", (62, 40)),
        ("chest_open", (94, 40)),
    ):
        image.alpha_composite(images[name], xy)
    return image.convert("RGB")


def review_sheet(images):
    sheet = Image.new("RGB", (1152, 816), PALETTE["k"])
    draw = ImageDraw.Draw(sheet)
    title = ImageFont.load_default(size=25)
    font = ImageFont.load_default(size=19)
    small = ImageFont.load_default(size=16)
    draw.text((24, 17), "CRAWL / OPEN CHEST AND WALL FIXTURES / 6x", font=title, fill=PALETTE["i"])
    for index, name in enumerate(("chest", *LAYOUT)):
        x = 24 + 216 * index
        sprite = images[name]
        draw.text((x, 60), f"{name} / {sprite.width}x{sprite.height}", font=font, fill=PALETTE["h"])
        if name.startswith("chest"):
            tile = Image.new("RGBA", (32, 32), (*PALETTE["K"], 255))
            tile.alpha_composite(sprite, (6, 8))
        else:
            tile = wall((32, 32))
            tile.alpha_composite(sprite, (10, 3 if name == "banner" else 6))
        sheet.paste(tile.convert("RGB").resize((192, 192), Image.Resampling.NEAREST), (x, 92))
    draw.text((24, 307), "Chest: fixed base and iron bands.  Sconces: only the flame changes.", font=small, fill=PALETTE["G"])
    draw.text((24, 343), "Stone-wall context / native sprites composited on the existing wall_stone / 6x", font=font, fill=PALETTE["h"])
    sheet.paste(context(images).resize((1056, 336), Image.Resampling.NEAREST), (48, 376))
    draw.text((24, 739), "Binary transparency / exact brief palette / matte rust-red cloth and pale sword sigil", font=small, fill=PALETTE["G"])
    draw.text((24, 767), "All sprite pixels enlarged with nearest-neighbor; labels are review-only.", font=small, fill=PALETTE["j"])
    sheet.save(SPRITES / "sheet-3.png")


def qa(images):
    directory = TOOLS / "sprite-3-qa"
    directory.mkdir(exist_ok=True)
    # All four sprites on plain charcoal for unobstructed silhouette inspection.
    strip = Image.new("RGB", (672, 216), PALETTE["k"])
    draw = ImageDraw.Draw(strip)
    font = ImageFont.load_default(size=16)
    for index, name in enumerate(LAYOUT):
        x = 12 + index * 168
        sprite = images[name]
        draw.text((x, 10), name, font=font, fill=PALETTE["h"])
        tile = Image.new("RGBA", sprite.size, (*PALETTE["K"], 255))
        tile.alpha_composite(sprite)
        strip.paste(tile.convert("RGB").resize((sprite.width * SCALE, sprite.height * SCALE), Image.Resampling.NEAREST), (x, 46))
    strip.save(directory / "sprites.png")
    braille_preview(context(images)).save(directory / "dots-2x4.png")


def validate(grids, images):
    report = []
    for name, spec in LAYOUT.items():
        width, height = spec["size"]
        grid = read_ink(SPRITES / f"{name}.ink")
        assert len(grid) == height and all(len(row) == width for row in grid), name
        assert grid == grids[name]
        assert all(ink == "." or ink in PALETTE for row in grid for ink in row), name
        with Image.open(SPRITES / "png" / f"{name}.png") as png:
            assert png.mode == "RGBA" and png.size == (width, height), name
            assert png.tobytes() == render(grid).tobytes(), f"{name}: ink/PNG mismatch"
            assert set(png.getchannel("A").tobytes()) == {0, 255}, name
        signals = [(x, y, ink) for y, row in enumerate(grid) for x, ink in enumerate(row) if ink in SIGNAL]
        for x, y, ink in signals:
            if name.startswith("sconce"):
                assert y < FLAME_END and ink in FIRE, (name, x, y, ink)
            elif name == "chest_open":
                assert 5 <= y < BASE_ROW and ink in GOLD + GEM, (name, x, y, ink)
            else:
                raise AssertionError(f"{name}: nonemissive sprite uses signal ink")
        report.append({
            "name": name, "size": [width, height],
            "opaque_pixels": sum(ink != "." for row in grid for ink in row),
            "inks": "".join(sorted(set("".join("".join(row) for row in grid)))),
            "signal_pixels": len(signals), "exact_palette_and_png_match": True,
            "binary_transparency": True,
        })
    chest = read_ink(SPRITES / "chest.ink")
    assert grids["chest_open"][BASE_ROW:] == chest[BASE_ROW:], "chest base drift"
    assert all(grids["chest_open"][y][x] == chest[y][x] for y in range(1, 15) for x in IRON_COLUMNS), "chest strap drift"
    treasure = {ink for row in grids["chest_open"] for ink in row}
    assert len(treasure & set(GOLD)) >= 2 and GEM in treasure, "missing readable treasure"
    a, b = grids["sconce_a"], grids["sconce_b"]
    assert a[FLAME_END:] == b[FLAME_END:], "sconce substrate drift"
    changes = [(x, y) for y in range(20) for x in range(12) if a[y][x] != b[y][x]]
    assert len(changes) >= 6, "flicker does not visibly change"
    assert all(y < FLAME_END and a[y][x] in FIRE + "." and b[y][x] in FIRE + "." for x, y in changes), "non-flame pixel changed"
    with Image.open(SPRITES / "sheet-3.png") as sheet:
        assert sheet.size == (1152, 816)
    baseline = json.loads((TOOLS / "sprite-3-baseline.json").read_text())
    preserved = 0
    for relative, digest in baseline.items():
        if relative == "sprites/NOTES.md":
            continue
        assert hashlib.sha256((ROOT / relative).read_bytes()).hexdigest() == digest, f"prior asset changed: {relative}"
        preserved += 1
    result = {
        "sprites": report,
        "chest_base_rows_8_through_15_identical": True,
        "chest_strap_columns_4_5_14_15_rows_1_through_14_identical": True,
        "sconce_nonflame_rows_8_through_19_identical": True,
        "sconce_changed_pixels": len(changes),
        "sconce_changes_only_flame": True,
        "banner_nonemissive": True,
        "prior_files_preserved_except_authorized_notes": preserved,
        "sheet_scale": SCALE, "sheet_size": [1152, 816],
        "context_native_size": [176, 56], "context_texture": "wall_stone",
    }
    (TOOLS / "sprite-3-validation.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"Validated {len(report)} sprites: exact dimensions/palette/PNG agreement, binary alpha, chest base and straps, flame-only flicker, matte banner; {preserved} prior files unchanged.")


def source_manifest():
    prompts = json.loads((TOOLS / "sprite-3-prompts.json").read_text())
    entries = []
    for name in LAYOUT:
        path = SPRITES / "sources" / f"{name}.png"
        with Image.open(path) as image:
            size = list(image.size)
        prompt = prompts["common"] + "\n" + prompts["sprites"][name]
        entries.append({
            "name": name, "source": str(path.relative_to(ROOT)),
            "source_size": size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
            "reference": prompts["references"].get(name),
            "generation": "built-in image_gen; transparent_background=true",
            "original_visually_inspected": True,
        })
    (TOOLS / "sprite-3-source-manifest.json").write_text(json.dumps(entries, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--from-ink", action="store_true", help="Rebuild PNGs and review sheet from existing inks")
    parser.add_argument("--qa", action="store_true", help="Write a 6x plain-background strip and 2x4-dot approximation")
    args = parser.parse_args()
    if args.from_ink:
        grids = {name: read_ink(SPRITES / f"{name}.ink") for name in LAYOUT}
    else:
        grids = {name: quantize(name) for name in LAYOUT}
        retouch(grids)
        lock_animation_parts(grids)
    images = {name: render(grid) for name, grid in grids.items()}
    images["chest"] = render(read_ink(SPRITES / "chest.ink"))
    for name, grid in grids.items():
        (SPRITES / f"{name}.ink").write_text("".join("".join(row) + "\n" for row in grid))
        images[name].save(SPRITES / "png" / f"{name}.png")
    review_sheet(images)
    if args.qa:
        qa(images)
    validate(grids, images)
    source_manifest()


if __name__ == "__main__":
    main()

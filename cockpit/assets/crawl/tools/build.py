#!/usr/bin/env python3
"""Downsample imagegen originals, quantize, retouch, export inks, and verify.

Run: UV_OFFLINE=1 uv run --quiet --with pillow python cockpit/assets/crawl/tools/build.py
The retained sources and prompts make this independent of imagegen after generation.
"""

import argparse
import json
from collections import Counter
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
PALETTE_TEXT = """
k 0b0a08 K 151614 Z 25140b X 322b2b g 3d3a36 j 515349 G 63655c
J 7a7c72 h 94968c i b0b2a8 H cdcfc6
f 0d1a0c D 182414 F 263b1c E 335022 e 35543f l 3f6428 N 4b591f
m 5a7330 L 6a7b24 A 6f8738 M 768e39 y 8aa34a C 6fa46d Y a8c05f
n 1a0e07 b 331a0c I 452712 B 582b13 p 652c10 P 684629 r 784620
R 924f1d o a97639 O be914b t d8a95e T e2cb8b
s 192934 x 22303a S 2f4a52 u 456070 U 6e8a8e v 8fa3a8 V b3c2c4
W d5dee0 q 12516a Q 527597 z 639dc2
0 12414f 1 2079a6 2 50c6b3 3 8fe8dc w e4fff8 a 7a4a12 4 ecb64a
@ f6b450 5 f7ca58 6 fbd070 $ c7b48f c e3d2c3 9 f4e1cb 8 431e0f 7 a8341f
""".split()
PALETTE = {
    key: tuple(bytes.fromhex(value))
    for key, value in zip(PALETTE_TEXT[::2], PALETTE_TEXT[1::2])
}
SIGNAL = set("0123wa4@56$c987")
NAMES = list(json.loads((ROOT / "tools/prompts.json").read_text())["textures"])
ALLOWED = {
    "wall_stone": "kKXgjGJ",
    "wall_crypt": "kKXgjGJ",
    "wall_mine": "kKXgjGnbIBPr",
    "wall_keep": "kKZXgjbIBa874",
    "wall_archive": "kKXgjGDFEeNbIBPrsxS",
    "wall_fungal": "kKXgjGDFEelmS01",
    "wall_unknown": "kKXgjG0",
    "door": "kKXgjGJnbIBPr",
    "stairs": "kKXgjGJ",
    "floor_flags": "kKXgjG",
    "ceiling": "kKXg",
}


def nearest(rgb, keys):
    return min(sorted(keys), key=lambda key: sum((a - b) ** 2 for a, b in zip(rgb, PALETTE[key])))


def quantize(name):
    with Image.open(ROOT / "sources" / f"{name}.png") as source:
        image = source.convert("RGB").resize((32, 32), Image.Resampling.BOX)
    return [[nearest(image.getpixel((x, y)), ALLOWED[name]) for x in range(32)] for y in range(32)]


def simplify(grid, wrap_y=False):
    """Drop isolated flecks surrounded by a clear majority; protect real light."""
    result = [row[:] for row in grid]
    for y in range(32):
        for x in range(32):
            if grid[y][x] in SIGNAL:
                continue
            neighbors = [
                grid[(y + dy) % 32][(x + dx) % 32]
                for dy in (-1, 0, 1) for dx in (-1, 0, 1)
                if (dx or dy) and (wrap_y or 0 <= y + dy < 32)
            ]
            key, count = Counter(neighbors).most_common(1)[0]
            if count >= 6 and key not in SIGNAL:
                result[y][x] = key
    return result


def retouch(name, grid):
    """Apply retained hand-painted ink patches; '.' in patches means keep."""
    path = ROOT / "tools/retouch.json"
    operations = json.loads(path.read_text()).get(name, []) if path.exists() else []
    for op in operations:
        for dy, row in enumerate(op["rows"]):
            for dx, key in enumerate(row):
                if key != ".":
                    assert key in PALETTE
                    grid[op["y"] + dy][op["x"] + dx] = key
    return grid


def match_edges(grid, vertical=False):
    """Use a shared boundary ink, not an alpha/gradient blend or mirror tile."""
    for y in range(32):
        a, b = grid[y][0], grid[y][-1]
        rgb = tuple((v + w) // 2 for v, w in zip(PALETTE[a], PALETTE[b]))
        key = nearest(rgb, set((a, b)))
        grid[y][0] = grid[y][-1] = key
    if vertical:
        for x in range(32):
            a, b = grid[0][x], grid[-1][x]
            rgb = tuple((v + w) // 2 for v, w in zip(PALETTE[a], PALETTE[b]))
            key = nearest(rgb, set((a, b)))
            grid[0][x] = grid[-1][x] = key
    return grid


def render(grid):
    image = Image.new("RGB", (32, 32))
    image.putdata([PALETTE[key] for row in grid for key in row])
    return image


def sheet(images):
    width, row_height = 800, 296
    planar = {"floor_flags", "ceiling"}
    height = sum(552 if name in planar else row_height for name in NAMES) + 32
    output = Image.new("RGB", (width, height), "black")
    draw = ImageDraw.Draw(output)
    font = ImageFont.load_default(size=18)
    small = ImageFont.load_default(size=13)
    y = 16
    for name in NAMES:
        image = images[name]
        big = image.resize((256, 256), Image.Resampling.NEAREST)
        draw.text((16, y), name, font=font, fill=PALETTE["J"])
        draw.text((200, y + 3), "8x / 2 by 2 repeat" if name in planar else "8x / three horizontal repeats", font=small, fill=PALETTE["j"])
        output.paste(image, (752, y - 1))
        for ty in range(2 if name in planar else 1):
            for tx in range(2 if name in planar else 3):
                output.paste(big, (16 + 256 * tx, y + 24 + 256 * ty))
        y += 552 if name in planar else row_height
    output.save(ROOT / "sheet.png")


def validate(images, grids):
    rows = []
    for name in NAMES:
        grid = grids[name]
        lines = (ROOT / f"{name}.ink").read_text().splitlines()
        assert len(lines) == 32 and all(len(row) == 32 for row in lines), name
        assert all(key in PALETTE for row in lines for key in row), name
        with Image.open(ROOT / "png" / f"{name}.png") as png:
            assert png.size == (32, 32) and png.convert("RGB").tobytes() == images[name].tobytes(), name
        assert all(row[0] == row[-1] for row in grid), f"{name}: horizontal seam"
        if name in ("floor_flags", "ceiling"):
            assert grid[0] == grid[-1], f"{name}: vertical seam"
        signal_count = sum(key in SIGNAL for row in grid for key in row)
        assert not signal_count or name in ("wall_keep", "wall_fungal", "wall_unknown"), name
        rows.append({"name": name, "size": [32, 32], "inks": "".join(sorted(set("".join(lines)))), "signal_pixels": signal_count, "horizontal_edges_match": True, "vertical_edges_match": name in ("floor_flags", "ceiling")})
    (ROOT / "tools/validation.json").write_text(json.dumps(rows, indent=2) + "\n")
    print(f"Validated {len(rows)} textures: exact palette, 32x32 inks/PNGs, required matching edges.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--from-ink", action="store_true", help="Rebuild PNGs and sheet from edited inks, preserving every pixel")
    parser.add_argument("--qa", action="store_true", help="Save manageable sheet crops for visual inspection")
    args = parser.parse_args()
    (ROOT / "png").mkdir(exist_ok=True)
    images, grids = {}, {}
    for name in NAMES:
        if args.from_ink:
            grid = [list(row) for row in (ROOT / f"{name}.ink").read_text().splitlines()]
        else:
            grid = simplify(quantize(name), name in ("floor_flags", "ceiling"))
            grid = match_edges(retouch(name, grid), name in ("floor_flags", "ceiling"))
            (ROOT / f"{name}.ink").write_text("".join("".join(row) + "\n" for row in grid))
        grids[name] = grid
        images[name] = render(grid)
        images[name].save(ROOT / "png" / f"{name}.png")
    sheet(images)
    validate(images, grids)
    if args.qa:
        qa = ROOT / "tools/qa"
        qa.mkdir(exist_ok=True)
        with Image.open(ROOT / "sheet.png") as image:
            for index, (top, bottom) in enumerate(((0, 904), (904, 1792), (1792, 2680), (2680, image.height))):
                image.crop((0, top, image.width, bottom)).save(qa / f"sheet-{index + 1}.png")


if __name__ == "__main__":
    main()

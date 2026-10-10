#!/usr/bin/env python3
"""Build transparent crawl sprites from retained imagegen originals or edited inks.

Run with Pillow: python cockpit/assets/crawl/tools/build_sprites.py --qa
The generator needs no network, model, or API key after source generation.
"""

import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

from build import PALETTE, SIGNAL, nearest

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / "tools"
SPRITES = ROOT / "sprites"
LAYOUT = json.loads((TOOLS / "sprite-layout.json").read_text())
FIRE = "4@567a8"
SWORDS = ("hand_sword_idle", "hand_sword_raise", "hand_sword_swing")


def flame_region(name, x, y):
    if name.startswith("hand_torch"):
        return y < 18
    if name.startswith("campfire"):
        return y < 14
    if name == "lectern":
        return x >= 19 and y < 5
    return False


def quantize(name):
    spec = LAYOUT[name]
    with Image.open(SPRITES / "sources" / f"{name}.png") as original:
        source = original.convert("RGBA")
        # Ignore low-alpha fringe specks when fitting the actual silhouette.
        box = source.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()
        assert box, f"{name}: empty source"
        source = source.crop(box)
        left, top, right, bottom = spec["box"]
        source = source.resize((right - left, bottom - top), Image.Resampling.BOX)
        fitted = Image.new("RGBA", tuple(spec["size"]))
        fitted.paste(source, (left, top))
    grid = []
    for y in range(fitted.height):
        row = []
        for x in range(fitted.width):
            *rgb, alpha = fitted.getpixel((x, y))
            allowed = spec["inks"] + (FIRE if flame_region(name, x, y) else "")
            if (name.startswith("hand_torch") and y < 16) or (name.startswith("campfire") and y < 10):
                allowed = FIRE
            row.append(nearest(rgb, allowed) if alpha >= 100 else ".")
        grid.append(row)
    return grid


def cuff_anchors(grids):
    # A four-row common mail cuff guarantees an exact animation attachment.
    cuff = ["KXgjgXgjgXgjgXK", "KgjXgjXgjXgjXgK", "KXgjgXgjgXgjgXK", "KgjXgjXgjXgjXgK"]
    for name in SWORDS:
        grid = grids[name]
        if name == "hand_sword_raise":
            for y in range(40, 44):
                for x in range(25, 40):
                    grid[y][x] = "K" if x in (25, 39) else "gjX"[(x + y) % 3]
        for y, row in enumerate(cuff, 44):
            grid[y][25:] = list(row)


def stable_flickers(grids):
    # Reuse exactly the same non-flame substrate between animation frames.
    for prefix, stable_row in (("hand_torch", 18), ("campfire", 14)):
        a, b = grids[f"{prefix}_a"], grids[f"{prefix}_b"]
        for y in range(stable_row, len(a)):
            b[y] = a[y][:]
        # Restrict differences inside the flame region to actual fire pixels.
        for y in range(stable_row):
            for x in range(len(a[y])):
                if a[y][x] not in FIRE and b[y][x] not in FIRE:
                    b[y][x] = a[y][x]


def retouch(grids):
    # Each patch is literal ink data: '.' erases; '_' preserves a pixel.
    operations = json.loads((TOOLS / "sprite-retouch.json").read_text())
    for name, patches in operations.items():
        for patch in patches:
            for dy, row in enumerate(patch["rows"]):
                for dx, ink in enumerate(row):
                    if ink != "_":
                        assert ink == "." or ink in PALETTE, (name, ink)
                        grids[name][patch["y"] + dy][patch["x"] + dx] = ink


def render(grid):
    image = Image.new("RGBA", (len(grid[0]), len(grid)))
    image.putdata([(0, 0, 0, 0) if ink == "." else (*PALETTE[ink], 255) for row in grid for ink in row])
    return image


def corridor():
    """A restrained 288x160 context plate; it is only a review background."""
    image = Image.new("RGB", (288, 160), PALETTE["k"])
    draw = ImageDraw.Draw(image)
    draw.polygon([(0, 0), (92, 48), (92, 106), (0, 160)], fill=PALETTE["X"])
    draw.polygon([(288, 0), (196, 48), (196, 106), (288, 160)], fill=PALETTE["X"])
    draw.polygon([(0, 160), (92, 106), (196, 106), (288, 160)], fill=PALETTE["K"])
    draw.rectangle((92, 48, 195, 105), fill=PALETTE["K"])
    draw.rectangle((126, 59, 161, 105), fill=PALETTE["k"])
    for y0, y1 in ((29, 58), (64, 78), (103, 94)):
        draw.line((0, y0, 92, y1), fill=PALETTE["K"])
        draw.line((287, y0, 196, y1), fill=PALETTE["K"])
    for x0, x1 in ((26, 100), (62, 118), (100, 133), (188, 155), (226, 170), (262, 188)):
        draw.line((x0, 159, x1, 106), fill=PALETTE["X"])
    for y in (115, 130, 151):
        draw.line((0, y, 287, y), fill=PALETTE["X"])
    for y in (62, 78, 94):
        draw.line((93, y, 125, y), fill=PALETTE["X"])
        draw.line((162, y, 194, y), fill=PALETTE["X"])
    return image


def context(images, sword="hand_sword_idle", torch="hand_torch_a", props=True):
    image = corridor().convert("RGBA")
    if props:
        for name, xy in (("boulder", (59, 112)), ("chest", (179, 116)), ("lectern", (202, 96)), ("campfire_a", (132, 111))):
            image.alpha_composite(images[name], xy)
    image.alpha_composite(images[torch], (0, 120))
    image.alpha_composite(images[sword], (248, 112))
    return image.convert("RGB")


def review_sheet(images):
    sheet = Image.new("RGB", (1728, 1728), PALETTE["k"])
    draw = ImageDraw.Draw(sheet)
    title_font = ImageFont.load_default(size=25)
    font = ImageFont.load_default(size=19)
    draw.text((24, 16), "CRAWL / HANDS AND PROPS / 6x nearest-neighbor", font=title_font, fill=PALETTE["i"])
    draw.text((24, 52), "Sword frames: same bottom-right cuff anchor / gray motion streaks", font=font, fill=PALETTE["G"])
    for index, name in enumerate(SWORDS):
        x = 160 + index * 550
        draw.text((x, 87), f"{name} / 40x48", font=font, fill=PALETTE["h"])
        tile = Image.new("RGBA", (40, 48), (*PALETTE["K"], 255))
        tile.alpha_composite(images[name])
        sheet.paste(tile.resize((240, 288), Image.Resampling.NEAREST).convert("RGB"), (x, 118))
        draw.line((x + 150, 413, x + 239, 413), fill=PALETTE["G"], width=2)
    others = [name for name in LAYOUT if name not in SWORDS]
    for index, name in enumerate(others):
        x = 32 + index * 240
        image = images[name]
        draw.text((x, 450), name, font=font, fill=PALETTE["h"])
        draw.text((x, 474), f"{image.width}x{image.height}", font=font, fill=PALETTE["G"])
        tile = Image.new("RGBA", image.size, (*PALETTE["K"], 255))
        tile.alpha_composite(image)
        sheet.paste(tile.resize((image.width * 6, image.height * 6), Image.Resampling.NEAREST).convert("RGB"), (x, 504))
    draw.text((24, 748), "288x160 context at 6x / torch left, idle sword right / cuffs cropped at bottom", font=font, fill=PALETTE["h"])
    sheet.paste(context(images).resize((1728, 960), Image.Resampling.NEAREST), (0, 768))
    sheet.save(SPRITES / "sheet.png")
    return sheet


def braille_preview(view):
    """Review only: quantize each 2x4 cell to one ink and eight dot positions."""
    scale = 3
    output = Image.new("RGB", (view.width * scale, view.height * scale), PALETTE["k"])
    draw = ImageDraw.Draw(output)
    for y in range(0, view.height, 4):
        for x in range(0, view.width, 2):
            samples = [(dx, dy, view.getpixel((x + dx, y + dy))) for dy in range(4) for dx in range(2)]
            visible = [rgb for _, _, rgb in samples if max(rgb) >= 58]
            if not visible:
                continue
            average = tuple(sum(rgb[c] for rgb in visible) // len(visible) for c in range(3))
            ink = nearest(average, PALETTE)
            for dx, dy, rgb in samples:
                if max(rgb) >= 58:
                    px, py = (x + dx) * scale, (y + dy) * scale
                    draw.rectangle((px, py, px + 1, py + 1), fill=PALETTE[ink])
    return output


def validate(grids, images):
    report = []
    for name, spec in LAYOUT.items():
        width, height = spec["size"]
        lines = (SPRITES / f"{name}.ink").read_text().splitlines()
        assert len(lines) == height and all(len(row) == width for row in lines), name
        assert all(ink == "." or ink in PALETTE for row in lines for ink in row), name
        assert any("." in row for row in lines), f"{name}: missing transparency"
        with Image.open(SPRITES / "png" / f"{name}.png") as png:
            assert png.mode == "RGBA" and png.size == (width, height), name
            assert png.tobytes() == images[name].tobytes(), f"{name}: ink/PNG disagreement"
            assert set(png.getchannel("A").tobytes()) == {0, 255}, name
        fire_pixels = [(x, y) for y, row in enumerate(lines) for x, ink in enumerate(row) if ink in SIGNAL]
        assert all(lines[y][x] in FIRE and flame_region(name, x, y) for x, y in fire_pixels), name
        if name.startswith("hand_"):
            assert any(ink != "." for ink in lines[-1]), f"{name}: cuff does not leave bottom edge"
        report.append({"name": name, "size": [width, height], "opaque_pixels": sum(ink != "." for row in lines for ink in row), "inks": "".join(sorted(set("".join(lines)))), "signal_pixels": len(fire_pixels), "exact_palette_and_png_match": True, "binary_transparency": True})
    cuffs = [[row[25:] for row in grids[name][44:]] for name in SWORDS]
    assert cuffs[0] == cuffs[1] == cuffs[2], "sword cuff drift"
    assert all(all(ink == "." for ink in grids[name][-1][:25]) for name in SWORDS), "sword bottom attachment drift"
    for prefix, stable_row in (("hand_torch", 18), ("campfire", 14)):
        a, b = grids[f"{prefix}_a"], grids[f"{prefix}_b"]
        assert a[stable_row:] == b[stable_row:], f"{prefix}: substrate drift"
        changed = sum(u != v for ar, br in zip(a, b) for u, v in zip(ar, br))
        assert changed >= 6, f"{prefix}: flicker not visible"
        for y, (ar, br) in enumerate(zip(a, b)):
            for x, (u, v) in enumerate(zip(ar, br)):
                assert u == v or (y < stable_row and (u in FIRE or v in FIRE)), (prefix, x, y)
    with Image.open(SPRITES / "sheet.png") as sheet:
        assert sheet.size == (288 * 6, 288 * 6)
    (TOOLS / "sprite-validation.json").write_text(json.dumps({"sprites": report, "sword_bottom_four_cuff_rows_identical": True, "flicker_substrates_identical": True, "context_native_size": [288, 160], "sheet_scale": 6}, indent=2) + "\n")
    print(f"Validated {len(report)} sprites: exact dimensions/palette/PNG agreement, binary alpha, anchored cuffs, stable flicker bases, fire-only signals.")


def source_manifest():
    prompts = json.loads((TOOLS / "sprite-prompts.json").read_text())
    entries = []
    references = {"hand_sword_raise": "hand_sword_idle", "hand_sword_swing": "hand_sword_idle", "hand_torch_b": "hand_torch_a", "campfire_b": "campfire_a"}
    for name in LAYOUT:
        path = SPRITES / "sources" / f"{name}.png"
        with Image.open(path) as image:
            size = list(image.size)
        prompt = prompts["common"] + "\n" + prompts["sprites"][name]
        entries.append({"name": name, "source": str(path.relative_to(ROOT)), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "source_size": size, "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(), "generation": "built-in image_gen; transparent_background=true", "reference": references.get(name), "original_visually_inspected": True})
    (TOOLS / "sprite-source-manifest.json").write_text(json.dumps(entries, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--from-ink", action="store_true", help="Preserve edited inks and rebuild only PNGs, sheet, validation")
    parser.add_argument("--qa", action="store_true", help="Write readable sheet crops, three context poses, and a 2x4 dot approximation")
    args = parser.parse_args()
    (SPRITES / "png").mkdir(parents=True, exist_ok=True)
    if args.from_ink:
        grids = {name: [list(row) for row in (SPRITES / f"{name}.ink").read_text().splitlines()] for name in LAYOUT}
    else:
        grids = {name: quantize(name) for name in LAYOUT}
        cuff_anchors(grids)
        stable_flickers(grids)
        retouch(grids)
    images = {name: render(grid) for name, grid in grids.items()}
    for name, grid in grids.items():
        (SPRITES / f"{name}.ink").write_text("".join("".join(row) + "\n" for row in grid))
        images[name].save(SPRITES / "png" / f"{name}.png")
    sheet = review_sheet(images)
    validate(grids, images)
    source_manifest()
    if args.qa:
        qa = TOOLS / "sprite-qa"
        qa.mkdir(exist_ok=True)
        sheet.crop((0, 0, 1728, 768)).save(qa / "sprites.png")
        sheet.crop((0, 768, 1728, 1728)).save(qa / "corridor.png")
        for name in SWORDS:
            context(images, sword=name, props=False).resize((1728, 960), Image.Resampling.NEAREST).save(qa / f"context-{name}.png")
        braille_preview(context(images)).save(qa / "dots-2x4.png")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Furnish a delve pack's rooms from the Demonic Dungeon (VEXED, CC BY 4.0).

    python3 scripts/delve/pack_kit.py crypt teal
    python3 scripts/delve/pack_kit.py keep red

With `pack_tiles.py` (floors and walls) this makes a whole room kit of the
pack's own pieces, so nothing of the earlier kit stands among them: the
obstacles (`block`), props and lights (`decor`), the gates and doors
(`gate`, `odoor`, `sidedoor-<pack>`) and the pits (`hazard`), each at the size
and name its slot already has, every pixel its nearest game ink.
"""
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from pack_tiles import ROCK, ROCK_EDGE, ROCK_LIP, ROOT, SLOTS, STYLES, ink_table, inked, rgb  # noqa: E402

PACK = ROOT / "cockpit/assets/dungeon/packs/demonic-dungeon"

# Sprites on the sheets: (sheet, x, y, w, h). Doors and stairs come in both
# styles; the furniture is the same wood in either.
DOOR = {
    "teal": {"open": (81, 32), "barred": (97, 32), "locked": (113, 32), "stairs": (1, 97), "steps": (18, 97)},
    "red": {"open": (81, 80), "barred": (97, 80), "locked": (113, 80), "stairs": (33, 97), "steps": (50, 97)},
}
# The cave's way in is its mouth (Cave.png); its way down a hole with a ladder.
CAVE_MOUTH = (81, 33, 14, 14)
CAVE_HOLE = (1, 96, 14, 15)
CAVE_LADDER = (35, 96, 10, 14)
ROCKS = {"teal": [(96, 65, 16, 14), (81, 66, 14, 13)], "red": [(96, 17, 16, 14), (81, 18, 14, 13)]}
SHELVES = [(1, 128), (17, 128), (33, 128), (49, 128), (65, 128), (81, 128)]
CHESTS = [(1, 145), (17, 145), (33, 145), (49, 145)]


def crop(sheet, x, y, w, h):
    return sheet.crop((x, y, x + w, y + h))


def canvas(w, h):
    return Image.new("RGBA", (w, h), (0, 0, 0, 0))


def stand(sprite, w=16, h=24):
    """A sprite on its feet, centred, at the bottom of a w x h canvas."""
    im = canvas(w, h)
    im.alpha_composite(sprite, ((w - sprite.width) // 2, h - sprite.height))
    return im


def main(pack, style):
    s = {k: (rgb(v) if isinstance(v, str) else v) for k, v in STYLES[style].items()}
    o, t, hl, f, d = s["o"], s["t"], s["h"], s["f"], s["d"]
    dun = Image.open(PACK / "Dungeon.png").convert("RGBA")
    cave = Image.open(PACK / "Cave.png").convert("RGBA")
    bg = Image.open(PACK / "Background.png").convert("RGBA")
    table = ink_table()
    out = {}  # (slot, name) -> image

    def door(kind):
        if style == "cave":
            if kind == "stairs":
                return crop(cave, *CAVE_HOLE)
            if kind == "steps":
                return crop(cave, *CAVE_LADDER)
            mouth = crop(cave, *CAVE_MOUTH)
            if kind in ("barred", "locked"):
                # Timbers across the mouth; a locked one shows its hasp.
                for y in range(5, 14):
                    for x in (3, 6, 9, 12):
                        if mouth.getpixel((x, y))[3]:
                            mouth.putpixel((x, y), rgb("904B41"))
                if kind == "locked":
                    for x, y in ((7, 9), (8, 9), (7, 10), (8, 10)):
                        mouth.putpixel((x, y), rgb("EF9E4E"))
            return mouth
        x, y = DOOR[style][kind]
        w = 12 if kind == "steps" else 14
        return crop(dun, x, y, w, 15 if kind in ("open", "barred", "locked") else 14)

    # ── obstacles ───────────────────────────────────────────────────────
    blk = f"dgn.block.{pack}"
    if style == "cave":
        # A mine's obstacles: stalagmites, crystal clusters, rock and barrels.
        stalag = [crop(cave, 49, 224, 14, 17), crop(cave, 65, 224, 14, 16)]
        out[(blk, "pillar-0")] = stand(stalag[0])
        out[(blk, "pillar-1")] = stand(stalag[1])
        out[(blk, "pillar-2")] = stand(stalag[0].transpose(Image.FLIP_LEFT_RIGHT))
        for i in range(3):
            out[(blk, f"tomb-{i}")] = stand(crop(cave, i * 16, 177, 16, 14))
    else:
        for i in range(3):
            out[(blk, f"pillar-{i}")] = stand(crop(dun, *SHELVES[i], 14, 15))
    # The Crypt's tombs are its coffers and slab in the wall's stone, not wood:
    # a room of chests reads as a treasury.
    stone = {"D1935F": "A69998", "904B41": "5F575E", "56212A": "2F3D3A", "EF9E4E": "8EB89E"}

    def tomb(sprite):
        if style != "teal":
            return sprite
        im = sprite.copy()
        for y in range(im.height):
            for x in range(im.width):
                r, g, b, a = im.getpixel((x, y))
                k = f"{r:02X}{g:02X}{b:02X}"
                if a and k in stone:
                    im.putpixel((x, y), rgb(stone[k]))
        return im

    if style != "cave":
        for i, (x, y) in enumerate(CHESTS[:2]):
            out[(blk, f"tomb-{i}")] = stand(tomb(crop(dun, x, y, 14, 13)))
        out[(blk, "tomb-2")] = stand(tomb(crop(dun, 1, 113, 14, 14)))
    big, small = ROCKS["red" if style == "cave" else style]
    out[(blk, "boulder-0")] = stand(crop(cave, *big))
    out[(blk, "boulder-1")] = stand(crop(cave, *small))
    out[(blk, "boulder-2")] = stand(crop(cave, *big).transpose(Image.FLIP_LEFT_RIGHT))
    if style == "cave":
        out[(blk, "broken-0")] = stand(crop(cave, 3, 129, 9, 13))
        out[(blk, "broken-1")] = stand(crop(cave, *small).transpose(Image.FLIP_LEFT_RIGHT))
        out[(blk, "broken-2")] = stand(crop(cave, 18, 129, 12, 15))
    else:
        out[(blk, "broken-0")] = stand(crop(dun, *SHELVES[5], 14, 15))
        out[(blk, "broken-1")] = stand(crop(cave, *small).transpose(Image.FLIP_LEFT_RIGHT))
        out[(blk, "broken-2")] = stand(crop(dun, *CHESTS[3], 14, 13))

    # ── props and lights ────────────────────────────────────────────────
    dec = f"dgn.decor.{pack}"
    for i, x in enumerate([5, 21, 37]):
        out[(dec, f"flame-{i}")] = stand(crop(cave, x, 144, 7, 16))
    out[(dec, "lamp")] = stand(crop(dun, 4, 292, 6, 10))
    out[(dec, "barrel")] = stand(crop(cave, 18, 129, 12, 15))
    out[(dec, "bones")] = stand(crop(dun, 84, 244, 8, 8))
    out[(dec, "web")] = crop(cave, 0, 200, 16, 24)
    if style == "cave":
        out[(dec, "bones")] = stand(crop(cave, 16, 192, 9, 10))
        out[(dec, "prop-0")] = stand(crop(cave, 0, 192, 9, 10))
        out[(dec, "prop-1")] = stand(crop(cave, 32, 192, 9, 10))
        out[(dec, "decal-0")] = stand(crop(cave, 23, 200, 8, 8))
        out[(dec, "decal-1")] = stand(crop(cave, 39, 200, 8, 8))
    else:
        out[(dec, "prop-0")] = stand(crop(dun, 5, 224, 7, 13))
        out[(dec, "prop-1")] = stand(crop(dun, 52, 308, 8, 8))
        out[(dec, "decal-0")] = stand(crop(dun, 6, 244, 4, 6))
        out[(dec, "decal-1")] = stand(crop(dun, 54, 260, 5, 7))
    # A light's pool: the pack's dither, densest at the heart, on nothing.
    glow = canvas(64, 64)
    bayer = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
    for y in range(64):
        for x in range(64):
            r = ((x - 31.5) ** 2 + (y - 31.5) ** 2) ** 0.5 / 32
            if r < 1 and bayer[y % 4][x % 4] < int(16 * (1 - r) ** 2 * 0.9):
                glow.putpixel((x, y), d)
    out[(dec, "glow")] = glow

    # ── gates and doors ─────────────────────────────────────────────────
    def face(w, h):
        """The north wall as `pack_tiles` lays it: outline, lit top, its face."""
        im = canvas(w, h)
        for x in range(w):
            im.putpixel((x, 0), o)
        for y in range(1, h - 8):
            for x in range(w):
                im.putpixel((x, y), t)
        if style == "cave":
            for i, row in enumerate(ROCK_EDGE):
                for x in range(w):
                    c = row[x % 4]
                    im.putpixel((x, 1 + i), d if c == "d" else f if c == "a" else t)
            for x in range(w):
                im.putpixel((x, h - 8), f if ROCK_LIP[x % 4] == "a" else t)
            for i, row in enumerate(ROCK):
                for x in range(w):
                    c = (row * 8)[x]
                    im.putpixel((x, h - 7 + i), o if c == "b" else d if c == "d" else f)
            return im
        for x in range(w):
            im.putpixel((x, 1), hl)
        mortar = ["........", "...#####", "........", "#####...", "........", "#####...", "........"]
        for x in range(w):
            im.putpixel((x, h - 8), d)
        for i, row in enumerate(mortar):
            for x in range(w):
                im.putpixel((x, h - 7 + i), o if (row * 8)[x] == "#" else f)
        return im

    def gate_with(kind):
        im = face(32, 24)
        sp = door(kind)
        im.alpha_composite(sp, ((32 - sp.width) // 2, 24 - sp.height))
        return im

    for slot in (f"dgn.gate.{pack}", f"dgn.odoor.{pack}"):
        out[(slot, "barred")] = gate_with("barred")
        out[(slot, "open")] = gate_with("open")
        out[(slot, "opening")] = gate_with("open")
        out[(slot, "boss")] = gate_with("locked")
        out[(slot, "seal")] = gate_with("locked")
        out[(slot, "stairs-down")] = gate_with("stairs")
    gate = f"dgn.gate.{pack}"
    out[(gate, "alcove")] = gate_with("open")
    out[(gate, "stairs-up")] = gate_with("steps")
    for side in ("left", "right"):
        out[(gate, f"side-{side}-barred")] = gate_with("barred")
        out[(gate, f"side-{side}-open")] = gate_with("open")

    def side_wall(w, h, room_side):
        im = canvas(w, h)
        for y in range(h):
            for x in range(w):
                im.putpixel((x, y), t)
            e = [w - 3, w - 2, w - 1] if room_side == "east" else [2, 1, 0]
            im.putpixel((e[0], y), o)
            im.putpixel((e[1], y), f)
            im.putpixel((e[2], y), d)
        return im

    def side_door(room_side, state):
        """A door in a side wall, 16 x 32: its dark mouth, planks or bars."""
        im = side_wall(16, 32, room_side)
        x0, x1 = (3, 15) if room_side == "east" else (1, 13)
        for y in range(8, 26):
            for x in range(x0, x1):
                im.putpixel((x, y), o)
        for x in range(x0 - 1, x1 + 1):
            im.putpixel((x, 7), d)
        if state in ("closed", "locked", "barred"):
            wood = rgb("904B41") if state != "barred" else f
            for y in range(9, 25):
                for x in range(x0 + 1, x1 - 1):
                    if state == "barred":
                        if (x - x0) % 3 == 1:
                            im.putpixel((x, y), wood)
                    elif (y - 9) % 4 != 3:
                        im.putpixel((x, y), wood)
            if state == "locked":
                cx = (x0 + x1) // 2
                for yy, xx in [(16, cx), (17, cx), (16, cx - 1), (17, cx - 1)]:
                    im.putpixel((xx, yy), rgb("EF9E4E"))
        return im

    od = f"dgn.odoor.{pack}"
    out[(od, "west-barred")] = side_door("east", "barred")
    out[(od, "west-open")] = side_door("east", "open")
    out[(od, "east-barred")] = side_door("west", "barred")
    out[(od, "east-open")] = side_door("west", "open")
    for state in ("barred", "open"):
        im = canvas(32, 16)
        for x in range(32):
            im.putpixel((x, 0), o)
            im.putpixel((x, 1), hl)
        for y in range(2, 16):
            for x in range(32):
                im.putpixel((x, y), t)
        for y in range(0, 16):
            for x in range(9, 23):
                im.putpixel((x, y), o)
        if state == "barred":
            for y in range(1, 16):
                for x in range(10, 22, 3):
                    im.putpixel((x, y), f)
        out[(od, f"south-{state}")] = im
    sd = f"dgn.sidedoor-{pack}"
    for side, room_side in (("west", "east"), ("east", "west")):
        for state in ("closed", "open", "locked"):
            out[(sd, f"{side}-{state}")] = side_door(room_side, state)

    # ── pits ────────────────────────────────────────────────────────────
    hz = f"dgn.hazard.{pack}"
    col = s["floor"]

    def pit(row):
        im = Image.new("RGBA", (16, 16), o)
        cell = crop(bg, col * 16, row * 16, 16, 16)
        darker = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
        for y in range(16):
            for x in range(16):
                if cell.getpixel((x, y))[3] and (x + y) % 2 == 0:
                    darker.putpixel((x, y), d)
        im.alpha_composite(darker)
        return im

    for i, row in enumerate([3, 4, 6, 7]):
        out[(hz, f"fill-{i}")] = pit(row)

    def rimmed(edges):
        im = pit(3)
        if "n" in edges:  # the far wall of the pit shows its brick face
            for x in range(16):
                im.putpixel((x, 0), hl)
                im.putpixel((x, 1), d)
            for y in range(2, 6):
                for x in range(16):
                    im.putpixel((x, y), o if (y == 3 and x % 8 < 5) or (y == 5 and (x + 4) % 8 < 5) else f)
        if "s" in edges:
            for x in range(16):
                im.putpixel((x, 15), hl)
        if "w" in edges:
            for y in range(16):
                im.putpixel((0, y), hl)
        if "e" in edges:
            for y in range(16):
                im.putpixel((15, y), hl)
        return im

    out[(hz, "edge-n")] = rimmed("n")
    out[(hz, "edge-s")] = rimmed("s")
    out[(hz, "edge-w")] = rimmed("w")
    out[(hz, "edge-e")] = rimmed("e")
    out[(hz, "outer-nw")] = rimmed("nw")
    out[(hz, "outer-se")] = rimmed("se")
    out[(hz, "inner-nw")] = rimmed("n")
    out[(hz, "inner-se")] = rimmed("s")

    written = 0
    for (slot, name), im in out.items():
        dest = SLOTS / slot / "v1" / f"{name}.png"
        old = Image.open(dest) if dest.exists() else None
        if old is None:
            raise SystemExit(f"{slot}/{name} has no piece to replace")
        inked(im, table).save(dest)
        written += 1
    print(f"furnished {pack} in the {style} style: {written} pieces")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])

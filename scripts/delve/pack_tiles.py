#!/usr/bin/env python3
"""Lay a delve pack's floors and walls in the Demonic Dungeon style (VEXED, CC BY 4.0).

    python3 scripts/delve/pack_tiles.py crypt teal
    python3 scripts/delve/pack_tiles.py keep red

Writes the kit's floor (`dgn.floor.<pack>`) and open-room walls
(`dgn.owall.<pack>`) under crates/delve/assets/art-slots, each piece at the
size and name the kit already has. The walls are built from the pack's own
parts (`Dungeon.png`): the flat top with its lit edge, the outline, and the
brick face below it; the floors are the pack's sparse dither tiles
(`Background.png`) on its darkest ink. Every pixel is then its nearest game
ink, as the kit's banks require.
"""
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
PACK = ROOT / "cockpit/assets/dungeon/packs/demonic-dungeon"
SLOTS = ROOT / "crates/delve/assets/art-slots"
INK_RS = ROOT / "crates/delve/src/render/overworld/ink.rs"


def ink_table():
    import re

    s = INK_RS.read_text()
    i = s.index("pub fn ink(ch: char) -> Option<Rgb> {")
    body = s[i : s.index("\n}\n", i)]
    return [
        tuple(int(h[k : k + 2], 16) for k in (0, 2, 4))
        for chars, h in re.findall(r"((?:'.'\s*\|\s*)*'.')\s*=>\s*0x([0-9a-fA-F]{6})", body)
    ]


def inked(im, table, near={}):
    """Every pixel as its nearest game ink: the kit's banks take inks only."""
    out = im.copy()
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = im.getpixel((x, y))
            if a:
                if (r, g, b) not in near:
                    near[(r, g, b)] = min(table, key=lambda c: sum((p - q) ** 2 for p, q in zip((r, g, b), c)))
                out.putpixel((x, y), near[(r, g, b)] + (255,))
    return out


def rgb(h):
    return tuple(int(h[k : k + 2], 16) for k in (0, 2, 4)) + (255,)


# The wall's inks, by style: outline, top, lit edge, brick face, the dark line.
STYLES = {
    "teal": dict(o="160D13", t="4D6660", h="8EB89E", f="5F575E", d="31293E", floor=2),
    "red": dict(o="160D13", t="AD4030", h="E79B7C", f="904B41", d="56212A", floor=1),
    # The cave (`Cave.png`): a sandy top with a rough lip, a face of rock.
    "cave": dict(o="160D13", t="D1935F", h="904B41", f="904B41", d="56212A", floor=1, rock=True),
}
# The floor names, each with its dither tile's row in Background.png, from the
# sparsest up: a floor reads as dark stone with a little grain.
FLOORS = {
    "plain": 19, "worn": 20, "cracked": 21, "patched": 23, "stained": 14, "rich": 17,
    "trim-0": 18, "trim-1": 22, "crack": 11, "puddle": 10, "debris": 9, "detail": 15,
}
# The brick face's mortar, as the pack lays it: dashes of five in eight, the
# rows between them plain, each course offset.
MORTAR = ["........", "...#####", "........", "#####...", "........", "#####...", "........"]


# The cave's face as the pack lays it: `f` its rock, `o` the cracks, `d` shade.
ROCK = ["daadaaaa", "baabbaab", "abbadbbd", "aaaadaad", "baabbaab", "dbbdabba", "daadaaaa"]
# The lip where the top meets the face, and the rough lit edge of the top.
ROCK_LIP = "aacc"
ROCK_EDGE = ["daad", "acca"]


def face_rows(phase, rock=False):
    """The seven rows of face under the dark line, shifted by `phase`."""
    if rock:
        return [(row * 4)[phase : phase + 16].replace("a", ".").replace("b", "#") for row in ROCK]
    return [(row * 3)[phase : phase + 16] for row in MORTAR]


def piece(w, h):
    return Image.new("RGBA", (w, h), (0, 0, 0, 0))


def main(pack, style):
    s = {k: (rgb(v) if isinstance(v, str) else v) for k, v in STYLES[style].items()}
    o, t, hl, f, d = s["o"], s["t"], s["h"], s["f"], s["d"]

    rock = STYLES[style].get("rock", False)

    def put_face(im, y0, phase, x0=0, x1=16):
        for x in range(x0, x1):
            im.putpixel((x, y0), (f if ROCK_LIP[x % 4] == "a" else t) if rock else d)
        for i, row in enumerate(face_rows(phase, rock)):
            for x in range(x0, x1):
                c = row[x]
                im.putpixel((x, y0 + 1 + i), o if c == "#" else d if c == "d" else f)

    def top(im, y0, y1, x0=0, x1=16, lit=True):
        for y in range(y0, y1):
            for x in range(x0, x1):
                im.putpixel((x, y), t)
        if lit and rock:
            for i, row in enumerate(ROCK_EDGE):
                for x in range(x0, x1):
                    c = row[x % 4]
                    im.putpixel((x, y0 + i), d if c == "d" else f if c == "a" else t)
        elif lit:
            for x in range(x0, x1):
                im.putpixel((x, y0), hl)

    def band(im, room_side, y0=0, y1=16):
        """A side wall's flat top, its outline and a sliver of face toward the room."""
        for y in range(y0, y1):
            for x in range(16):
                im.putpixel((x, y), t)
            edge = [13, 14, 15] if room_side == "east" else [2, 1, 0]
            im.putpixel((edge[0], y), o)
            im.putpixel((edge[1], y), f)
            im.putpixel((edge[2], y), d)
            lit = 0 if room_side == "east" else 15
            im.putpixel((lit, y), hl)

    out = {}
    for i, name in enumerate(["face-0", "face-1", "face-2"]):
        im = piece(16, 24)
        for x in range(16):
            im.putpixel((x, 0), o)
        top(im, 1, 16)
        put_face(im, 16, [0, 4, 2][i])
        out[name] = im
    for name, side in [("corner-nw", "east"), ("corner-ne", "west")]:
        im = piece(16, 24)
        for x in range(16):
            im.putpixel((x, 0), o)
        top(im, 1, 16)
        band(im, side, 16, 24)
        out[name] = im
    for name, side in [("west-0", "east"), ("west-1", "east"), ("east-0", "west"), ("east-1", "west")]:
        im = piece(16, 16)
        band(im, side)
        out[name] = im
    im = piece(16, 16)
    for x in range(16):
        im.putpixel((x, 0), o)
    top(im, 1, 8)
    put_face(im, 8, 0)
    out["south"] = im
    for name, side in [("corner-sw", "east"), ("corner-se", "west")]:
        im = piece(16, 16)
        band(im, side, 0, 8)
        put_face(im, 8, 0)
        out[name] = im
    table = ink_table()
    dest = SLOTS / f"dgn.owall.{pack}" / "v1"
    for name, im in out.items():
        inked(im, table).save(dest / f"{name}.png")

    bg = Image.open(PACK / "Background.png").convert("RGBA")
    col = s["floor"]
    dest = SLOTS / f"dgn.floor.{pack}" / "v1"
    for name, row in FLOORS.items():
        tile = Image.new("RGBA", (16, 16), o)
        cell = bg.crop((col * 16, row * 16, col * 16 + 16, row * 16 + 16))
        tile.alpha_composite(cell)
        inked(tile, table).save(dest / f"{name}.png")
    print(f"laid dgn.owall.{pack} ({len(out)}) and dgn.floor.{pack} ({len(FLOORS)}) in the {style} style")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])

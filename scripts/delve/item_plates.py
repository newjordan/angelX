"""The art team's item drawings -> the Delve's pickup plate sheet.

    uv run --no-project --with pillow scripts/delve/item_plates.py CROPS_DIR OUT.png

Every name in PLATES (the order `item_pack.rs` reads them in) is CROPS_DIR/NAME.png,
a drawing on transparency. Each is fitted into a CELL x CELL cell of a 6 x 6
sheet, centred, its alpha made hard (a plate stands over the floor with no
halo), and the sheet is written as one RGBA PNG.
"""
import sys
from PIL import Image

CELL = 96
PLATES = [
    "bow", "sword", "axe", "staff", "spear", "hammer",
    "dagger", "crossbow", "handgonne", "mace", "shield", "key",
    "spell", "flask", "food", "heart", "gold", "bone",
    "wax", "ore", "gem", "ember", "scale", "gear",
    "rune-haste", "rune-double-damage", "rune-regeneration", "rune-arcane", "rune-invisibility", "rune-illusion",
    "rune-bounty", "rune-wisdom", "chest", "pouch", "tome-fire", "tome-frost",
]


def main():
    crops, out = sys.argv[1], sys.argv[2]
    sheet = Image.new("RGBA", (CELL * 6, CELL * 6), (0, 0, 0, 0))
    for i, name in enumerate(PLATES):
        im = Image.open(f"{crops}/{name}.png").convert("RGBA")
        box = im.getchannel("A").point(lambda a: 255 if a > 24 else 0).getbbox()
        im = im.crop(box)
        fit = (CELL - 6) / max(im.size)
        im = im.resize((max(1, round(im.width * fit)), max(1, round(im.height * fit))), Image.LANCZOS)
        a = im.getchannel("A").point(lambda v: 255 if v >= 128 else 0)
        im.putalpha(a)
        x = (i % 6) * CELL + (CELL - im.width) // 2
        y = (i // 6) * CELL + (CELL - im.height) // 2
        sheet.paste(im, (x, y), im)
    sheet.save(out, optimize=True)
    print(out, sheet.size)


if __name__ == "__main__":
    main()

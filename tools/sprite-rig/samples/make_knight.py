"""Procedurally draw a simple side-ish knight sprite (arms/legs separated from torso)."""
import sys
from PIL import Image, ImageDraw


def knight(path, W=96, H=144, bg=None):
    im = Image.new("RGBA", (W, H), bg or (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    steel, dark, red, gold = (170, 175, 190, 255), (70, 72, 85, 255), (150, 20, 25, 255), (210, 170, 60, 255)
    cx = W // 2
    # legs (gap between them)
    for x in (cx - 13, cx + 3):
        d.rectangle([x, 80, x + 10, 112], fill=steel, outline=dark)
        d.rectangle([x, 112, x + 10, 136], fill=steel, outline=dark)
        d.rectangle([x - 1, 134, x + 12, 141], fill=dark)
    # torso
    d.rectangle([cx - 15, 36, cx + 15, 81], fill=steel, outline=dark)
    d.polygon([(cx - 15, 40), (cx + 15, 40), (cx, 70)], fill=red)
    d.rectangle([cx - 15, 74, cx + 15, 81], fill=gold)
    # arms (gap from torso)
    for x in (cx - 27, cx + 19):
        d.rectangle([x, 37, x + 8, 60], fill=steel, outline=dark)
        d.rectangle([x, 60, x + 8, 80], fill=steel, outline=dark)
        d.rectangle([x - 1, 79, x + 9, 86], fill=dark)
    # sword in front hand
    d.rectangle([cx + 22, 86, cx + 25, 120], fill=(220, 225, 235, 255), outline=dark)
    d.rectangle([cx + 17, 84, cx + 30, 87], fill=gold)
    # helmet
    d.rectangle([cx - 11, 8, cx + 11, 34], fill=steel, outline=dark)
    d.rectangle([cx - 8, 18, cx + 10, 21], fill=(20, 20, 25, 255))
    d.polygon([(cx - 3, 8), (cx + 3, 8), (cx - 6, 1)], fill=red)
    im.save(path)


if __name__ == "__main__":
    knight(sys.argv[1] if len(sys.argv) > 1 else "knight.png")
    if len(sys.argv) > 2:
        knight(sys.argv[2], bg=(40, 140, 60, 255))

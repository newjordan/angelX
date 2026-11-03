"""Generate a native 24px right-facing quick-rig input and a 2-cell PNG sheet.

Run from tools/sprite-rig: python samples/quick/make_sample.py
No resized stock art: every rectangle is authored at its final pixel size;
colours come from the checkout's real Delve ink table.
"""
from pathlib import Path
import sys

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from spriterig.delve.source import palette


def make_sample():
    pal = palette()
    im = Image.new('RGBA', (24, 24))
    draw = ImageDraw.Draw(im)

    def box(bounds, ink):
        draw.rectangle(bounds, fill=(*pal[ink], 255))

    # Cape, rear limbs, torso and front limbs, deliberately distinct silhouettes.
    box((5, 7, 7, 19), '8')
    box((6, 9, 8, 12), 'q')
    box((6, 13, 7, 15), 'Q')
    box((6, 16, 7, 17), 'c')
    box((9, 14, 10, 20), 'q')
    box((8, 21, 11, 22), 'K')
    box((12, 14, 14, 20), 'Q')
    box((12, 21, 16, 22), 'K')
    box((9, 7, 14, 13), 'Q')
    box((10, 7, 11, 11), 'z')
    box((9, 13, 14, 14), 'a')
    box((15, 8, 16, 11), 'Q')
    box((16, 12, 17, 14), 'z')
    box((17, 15, 18, 16), 'c')
    # Helm profile and forward nose identify the facing without resampling.
    box((8, 2, 14, 6), 'Q')
    box((9, 2, 13, 3), 'V')
    box((14, 4, 16, 5), 'Q')
    box((11, 4, 14, 4), 'K')
    return im


if __name__ == '__main__':
    directory = Path(__file__).resolve().parent
    image = make_sample()
    image.save(directory/'squire.png')
    sheet = Image.new('RGBA', (48, 24))
    sheet.paste(image.transpose(Image.Transpose.FLIP_LEFT_RIGHT), (0, 0))
    sheet.paste(image, (24, 0))
    sheet.save(directory/'squire_sheet.png')

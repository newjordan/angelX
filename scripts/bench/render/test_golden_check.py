#!/usr/bin/env python3
"""Tests for golden_check.py: synthetic composites, no seat needed.

The sRGB-blended scene must pass; the same scene blended in linear
light (Kitty's convention) must fail — that discrimination is the
whole point of the contract.
"""
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from PIL import Image  # noqa: E402

import golden_check  # noqa: E402


def scene(blend):
    im = Image.new("RGB", (720, 480), (0, 0, 0))
    px = im.load()
    for y in range(400):
        for x in range(640):
            sx, sy = x + 40, y + 40
            if x < 320:
                t = x / 319.0
                if blend == "srgb":
                    red = round(255 * t)
                else:  # linear light: blend, then re-encode
                    red = round(255 * (t ** (1 / 2.2)))
                px[sx, sy] = (red, 0, 0)
            else:
                c = ((x // 32) + (y // 32)) % 2
                px[sx, sy] = (0, 220, 0) if c else (0, 0, 220)
    return im


def main():
    with tempfile.TemporaryDirectory() as d:
        good = os.path.join(d, "srgb.png")
        scene("srgb").save(good)
        result = golden_check.check(good)
        assert result["ok"], result

        bad = os.path.join(d, "linear.png")
        scene("linear").save(bad)
        result = golden_check.check(bad)
        assert not result["ok"], result
        assert any("ramp" in f for f in result["failures"]), result

        blank = os.path.join(d, "blank.png")
        Image.new("RGB", (64, 64), (0, 0, 0)).save(blank)
        assert not golden_check.check(blank)["ok"]
    print("test_golden_check: ok")


if __name__ == "__main__":
    main()

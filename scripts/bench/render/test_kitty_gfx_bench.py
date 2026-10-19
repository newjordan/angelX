#!/usr/bin/env python3
"""Durable checks for the Board A Kitty harness: the PNG encoder must
produce bytes a real decoder accepts, the pixel formula must match the
glass bench byte for byte (same constant asserted in stream-bench.rs),
and percentiles must be the order statistics they claim to be.

Run: python3 -m pytest scripts/bench/render/test_kitty_gfx_bench.py
     (or plain python3; plain asserts, no framework needed)
"""
import struct
import zlib

from kitty_gfx_bench import frame_bytes, percentiles, png_bytes

# frame_bytes(4, 2, n=1, seed=7): the cross-language constant.
FRAME_4X2 = [
    10, 5, 1, 255, 11, 5, 0, 255, 12, 5, 3, 255, 13, 5, 2, 255,
    10, 6, 0, 255, 11, 6, 1, 255, 12, 6, 2, 255, 13, 6, 3, 255,
]


def test_frame_formula_constant():
    assert list(frame_bytes(4, 2, 1, 7)) == FRAME_4X2
    # Frames must differ: a renderer may skip unchanged uploads.
    assert frame_bytes(8, 8, 0, 1) != frame_bytes(8, 8, 1, 1)


def test_png_round_trip():
    rgba = frame_bytes(5, 3, 2, 9)
    png = png_bytes(5, 3, rgba)
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, ihdr = 8, b"", None
    while pos < len(png):
        (size,) = struct.unpack(">I", png[pos:pos + 4])
        kind = png[pos + 4:pos + 8]
        data = png[pos + 8:pos + 8 + size]
        (crc,) = struct.unpack(">I", png[pos + 8 + size:pos + 12 + size])
        assert crc == zlib.crc32(kind + data) & 0xFFFFFFFF
        if kind == b"IHDR":
            ihdr = struct.unpack(">IIBBBBB", data)
        elif kind == b"IDAT":
            idat += data
        pos += 12 + size
    assert ihdr == (5, 3, 8, 6, 0, 0, 0)
    raw = zlib.decompress(idat)
    stride = 5 * 4
    for y in range(3):
        assert raw[y * (stride + 1)] == 0
        assert raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)] == rgba[y * stride:(y + 1) * stride]


def test_percentiles():
    p = percentiles([0.001 * i for i in range(1, 101)])
    assert p["frames"] == 100
    assert 49.0 <= p["p50_ms"] <= 51.0
    assert 94.0 <= p["p95_ms"] <= 96.0
    assert p["worst_ms"] == 100.0


if __name__ == "__main__":
    test_frame_formula_constant()
    test_png_round_trip()
    test_percentiles()
    print("test_kitty_gfx_bench: ok")

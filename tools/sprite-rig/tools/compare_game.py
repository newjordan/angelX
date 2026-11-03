"""Compare Rust native knight frames with exported sprite-rig PNGs, exactly.

Both inputs are mandatory. No arcade art, procedural substitute, independent
subject scaling, alpha cropping, tolerance, or silently omitted references.
"""
import argparse
from pathlib import Path
import sys

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from spriterig import native as N
from spriterig.delve.source import palette


def compare(native, actual, root, out, zoom=8, kit=None):
    if zoom < 1:
        raise ValueError('zoom must be a positive integer')
    N.load_fits(root)  # fail missing/invalid canonical table, even on a pure preview
    pal = palette(root)
    refs = N.select_frames(N.load_native(native, pal), kit=kit)
    actual = Path(actual)
    manifest = actual / 'frames.json' if actual.is_dir() else actual
    candidates = N.load_native(manifest, pal)
    by_key = {N.frame_key(f): f for f in candidates}
    expected_keys = {N.frame_key(f) for f in refs}
    extra = {N.frame_key(f) for f in candidates if kit is None or f['kit'] == kit} - expected_keys
    if extra:
        raise ValueError(f'unexpected actual native frames: {sorted(extra)}')
    pairs, failures = [], []
    for ref in refs:
        key = N.frame_key(ref)
        if key not in by_key:
            raise ValueError(f'missing actual native frame {key}')
        got = by_key[key]
        filename = got.get('image')
        if not filename:
            raise ValueError(f'actual frame {key} has no PNG image')
        with Image.open(manifest.parent / filename) as image:
            pixels = np.array(image.convert('RGBA'))
        expected = N.rows_rgba(ref['rows'], pal)
        equal = pixels.shape == expected.shape and np.array_equal(pixels, expected)
        # The manifest must describe those same pixels and resolved native fit.
        equal = equal and (got['w'], got['h']) == (ref['w'], ref['h'])
        equal = equal and got['rows'] == ref['rows'] and got['fit'] == ref['fit'] and got['mirror'] == ref['mirror']
        if not equal:
            failures.append(list(key))
        pairs.append((ref, expected, pixels, equal))
    # Same uncropped coordinate system, same zoom for both subjects and diff.
    shown = pairs[:16]
    cw = max(max(a.shape[1], b.shape[1]) for _, a, b, _ in shown) * zoom + 20
    ch = max(max(a.shape[0], b.shape[0]) for _, a, b, _ in shown) * zoom + 36
    sheet = Image.new('RGB', (cw * 3, ch * len(shown) + 24), (26, 26, 34))
    draw = ImageDraw.Draw(sheet)
    for col, name in enumerate(('Rust native game', 'sprite-rig native export', 'exact RGBA difference')):
        draw.text((col * cw + 4, 4), name, fill='white')
    for row, (ref, expected, got, equal) in enumerate(shown):
        h, w = max(expected.shape[0], got.shape[0]), max(expected.shape[1], got.shape[1])
        a, b = np.zeros((h, w, 4), np.uint8), np.zeros((h, w, 4), np.uint8)
        a[:expected.shape[0], :expected.shape[1]] = expected
        b[:got.shape[0], :got.shape[1]] = got
        diff = np.zeros_like(a)
        diff[np.any(a != b, axis=2)] = (255, 40, 100, 255)
        y = 24 + row * ch
        for col, pixels in enumerate((expected, got, diff)):
            im = Image.fromarray(pixels)
            im = im.resize((im.width * zoom, im.height * zoom), Image.Resampling.NEAREST)
            sheet.paste(im, (col * cw + 8, y + 28), im)
            draw.text((col * cw + 4, y + 2),
                      f"{ref['view']} {ref['pose']} t={ref['tick']} {'PASS' if equal else 'FAIL'}", fill='white')
    out = Path(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out)
    report = dict(compared=len(pairs), failed=len(failures), failures=failures,
                  zoom=zoom, parity=not failures, native=str(native), actual=str(manifest))
    N.write_json(out.with_suffix('.json'), report)
    if failures:
        raise ValueError(f'{len(failures)}/{len(pairs)} frames differ; see {out}')
    print(f"PASS: {len(pairs)} native {kit} frames equal, exact RGBA + fit; common {zoom}x zoom -> {out}")
    return report


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--native', required=True, help='Rust-exported native game frames JSON')
    ap.add_argument('--actual', required=True, help='sprite-rig export directory or frames.json')
    ap.add_argument('--angelx', help='checkout owning canonical fit table and ink palette')
    ap.add_argument('--out', default='out/native-parity.png')
    ap.add_argument('--zoom', type=int, default=8)
    ap.add_argument('--kit', default=None)
    a = ap.parse_args(argv)
    try:
        return compare(a.native, a.actual, N.root_path(a.angelx), a.out, a.zoom, a.kit)
    except (OSError, ValueError, KeyError, TypeError) as exc:
        ap.error(str(exc))


if __name__ == '__main__':
    main()

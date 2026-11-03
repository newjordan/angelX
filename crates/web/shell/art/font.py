#!/usr/bin/env python3
"""The page's own hand: a pixel font, built to knight.ttf beside the shell.

    python3 crates/web/shell/art/font.py      (needs fontTools)

The capitals, figures and marks are the Delve's own 5x7 hand
(crates/delve/src/render/overworld/ink.rs, `glyph`), so the page's words
and the picture's are one hand; the small letters, with their ascenders
and descenders, are drawn here to go with them. Each glyph is set as wide
as its ink and one pixel apart, a space three.

A pixel is 100 units and the em ten pixels (seven above the line, a
headroom pixel, two below), so at 20px a pixel is two of the page's and at
30px three: the shell sets it at whole multiples and on line heights that
keep the baseline on a whole pixel.
"""

import os
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

PX = 100
ASCENT = 8 * PX
DESCENT = 2 * PX

# The Delve's capitals, figures and marks: seven rows, five bits a row.
INK = {
    'A': [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
    'B': [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
    'C': [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
    'D': [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
    'E': [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
    'F': [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
    'G': [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
    'H': [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
    'I': [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
    'J': [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
    'K': [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
    'L': [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
    'M': [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
    'N': [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
    'O': [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
    'P': [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
    'Q': [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
    'R': [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
    'S': [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
    'T': [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
    'U': [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
    'V': [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
    'W': [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
    'X': [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
    'Y': [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
    'Z': [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
    '0': [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
    '1': [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
    '2': [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
    '3': [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
    '4': [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
    '5': [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
    '6': [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
    '7': [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
    '8': [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
    '9': [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
    '-': [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
    '_': [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
    '.': [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
    ',': [0x00, 0x00, 0x00, 0x00, 0x0c, 0x04, 0x08],
    ':': [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x0c, 0x00],
    '·': [0x00, 0x00, 0x00, 0x0c, 0x0c, 0x00, 0x00],
    '/': [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
    '%': [0x19, 0x1a, 0x02, 0x04, 0x08, 0x0b, 0x13],
    '+': [0x00, 0x04, 0x04, 0x1f, 0x04, 0x04, 0x00],
    '>': [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
    '<': [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
    '!': [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
    '?': [0x0e, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
    "'": [0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
    '(': [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
    ')': [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
    '#': [0x0a, 0x0a, 0x1f, 0x0a, 0x1f, 0x0a, 0x0a],
    '×': [0x00, 0x00, 0x11, 0x0a, 0x04, 0x0a, 0x11],
    '[': [0x0e, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0e],
    ']': [0x0e, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0e],
    '\\': [0x10, 0x08, 0x08, 0x04, 0x02, 0x02, 0x01],
    '=': [0x00, 0x00, 0x1f, 0x00, 0x1f, 0x00, 0x00],
    ';': [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x04, 0x08],
    '`': [0x08, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00],
}

# Drawn here: the small letters (x-height five, ascenders to the capitals'
# height, descenders two below the line) and the marks the Delve's hand
# lacks. Rows from the capitals' top; '#' ink, '.' none.
DRAWN = {
    'a': ['', '', '.###.', '....#', '.####', '#...#', '.####'],
    'b': ['#....', '#....', '####.', '#...#', '#...#', '#...#', '####.'],
    'c': ['', '', '.###', '#...', '#...', '#...', '.###'],
    'd': ['....#', '....#', '.####', '#...#', '#...#', '#...#', '.####'],
    'e': ['', '', '.###.', '#...#', '#####', '#....', '.###.'],
    'f': ['..##', '.#..', '###.', '.#..', '.#..', '.#..', '.#..'],
    'g': ['', '', '.####', '#...#', '#...#', '#...#', '.####', '....#', '.###.'],
    'h': ['#....', '#....', '####.', '#...#', '#...#', '#...#', '#...#'],
    'i': ['#', '', '#', '#', '#', '#', '#'],
    'j': ['..#', '', '..#', '..#', '..#', '..#', '..#', '#.#', '.#.'],
    'k': ['#...', '#...', '#..#', '#.#.', '##..', '#.#.', '#..#'],
    'l': ['#.', '#.', '#.', '#.', '#.', '#.', '.#'],
    'm': ['', '', '##.#.', '#.#.#', '#.#.#', '#.#.#', '#.#.#'],
    'n': ['', '', '####.', '#...#', '#...#', '#...#', '#...#'],
    'o': ['', '', '.###.', '#...#', '#...#', '#...#', '.###.'],
    'p': ['', '', '####.', '#...#', '#...#', '#...#', '####.', '#....', '#....'],
    'q': ['', '', '.####', '#...#', '#...#', '#...#', '.####', '....#', '....#'],
    'r': ['', '', '#.##', '##..', '#...', '#...', '#...'],
    's': ['', '', '.###', '#...', '.##.', '...#', '###.'],
    't': ['.#..', '.#..', '###.', '.#..', '.#..', '.#..', '..##'],
    'u': ['', '', '#...#', '#...#', '#...#', '#...#', '.####'],
    'v': ['', '', '#...#', '#...#', '#...#', '.#.#.', '..#..'],
    'w': ['', '', '#...#', '#...#', '#.#.#', '#.#.#', '.#.#.'],
    'x': ['', '', '#...#', '.#.#.', '..#..', '.#.#.', '#...#'],
    'y': ['', '', '#...#', '#...#', '#...#', '#...#', '.####', '....#', '.###.'],
    'z': ['', '', '#####', '...#.', '..#..', '.#...', '#####'],
    '"': ['#.#', '#.#', '', '', '', '', ''],
    '’': ['.#', '.#', '#.', '', '', '', ''],
    '‘': ['.#', '#.', '#.', '', '', '', ''],
    '“': ['.#.#', '#.#.', '#.#.', '', '', '', ''],
    '”': ['.#.#', '.#.#', '#.#.', '', '', '', ''],
    '&': ['.##..', '#..#.', '#.#..', '.#...', '#.#.#', '#..#.', '.##.#'],
    '*': ['', '#.#', '.#.', '###', '.#.', '#.#', ''],
    '@': ['.###.', '#...#', '#.###', '#.#.#', '#.###', '#....', '.###.'],
    '$': ['..#..', '.####', '#.#..', '.###.', '..#.#', '####.', '..#..'],
    '^': ['..#..', '.#.#.', '#...#', '', '', '', ''],
    '~': ['', '', '.#...', '#.#.#', '...#.', '', ''],
    '{': ['..#', '.#.', '.#.', '#..', '.#.', '.#.', '..#'],
    '}': ['#..', '.#.', '.#.', '..#', '.#.', '.#.', '#..'],
    '|': ['#', '#', '#', '#', '#', '#', '#'],
    '—': ['', '', '', '#######', '', '', ''],
    '–': ['', '', '', '#####', '', '', ''],
    '…': ['', '', '', '', '', '', '#.#.#'],
    '♪': ['..##', '..#.', '..#.', '..#.', '###.', '###.', '.#..'],
    '◆': ['', '..#..', '.###.', '#####', '.###.', '..#..', ''],
    '◇': ['', '..#..', '.#.#.', '#...#', '.#.#.', '..#..', ''],
    '█': ['#####'] * 7,
    '░': ['#.#.#', '.#.#.', '#.#.#', '.#.#.', '#.#.#', '.#.#.', '#.#.#'],
    '▲': ['', '', '..#..', '.###.', '#####', '', ''],
    '▼': ['', '', '#####', '.###.', '..#..', '', ''],
    '◀': ['', '...#', '..##', '.###', '..##', '...#', ''],
    '▶': ['', '#...', '##..', '###.', '##..', '#...', ''],
    'é': ['...#.', '..#..', '.###.', '#...#', '#####', '#....', '.###.'],
}


def rows_of(ch):
    """A glyph's rows as strings of '#' and '.', and its width."""
    if ch in DRAWN:
        rows = DRAWN[ch]
        w = max((len(r) for r in rows), default=0)
        return [r.ljust(w, '.') for r in rows], w
    bits = INK[ch]
    rows = [''.join('#' if b & (1 << (4 - x)) else '.' for x in range(5)) for b in bits]
    # Trim the empty columns either side, so the hand sets proportionally.
    used = [x for x in range(5) if any(r[x] == '#' for r in rows)]
    lo, hi = (used[0], used[-1]) if used else (0, 0)
    return [r[lo:hi + 1] for r in rows], hi - lo + 1


def draw(rows, pen):
    """Each row's runs of ink as one rectangle; row 0 is the capitals' top."""
    for y, row in enumerate(rows):
        top = (7 - y) * PX
        x = 0
        while x < len(row):
            if row[x] != '#':
                x += 1
                continue
            start = x
            while x < len(row) and row[x] == '#':
                x += 1
            l, r = start * PX, x * PX
            pen.moveTo((l, top - PX))
            pen.lineTo((l, top))
            pen.lineTo((r, top))
            pen.lineTo((r, top - PX))
            pen.closePath()


def main():
    chars = sorted(set(INK) | set(DRAWN))
    names = ['.notdef', 'space', 'nbsp'] + [f'g{ord(c):04x}' for c in chars]
    fb = FontBuilder(1000, isTTF=True)
    fb.setupGlyphOrder(names)
    cmap = {32: 'space', 0xa0: 'nbsp'}
    glyphs, metrics = {}, {}
    empty = TTGlyphPen(None)
    glyphs['space'] = empty.glyph()
    glyphs['nbsp'] = TTGlyphPen(None).glyph()
    metrics['space'] = metrics['nbsp'] = (3 * PX, 0)
    nd = TTGlyphPen(None)
    draw(['###', '#.#', '#.#', '#.#', '#.#', '#.#', '###'], nd)
    glyphs['.notdef'] = nd.glyph()
    metrics['.notdef'] = (4 * PX, 0)
    for c in chars:
        name = f'g{ord(c):04x}'
        rows, w = rows_of(c)
        pen = TTGlyphPen(None)
        draw(rows, pen)
        glyphs[name] = pen.glyph()
        metrics[name] = ((w + 1) * PX, 0)
        cmap[ord(c)] = name
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(glyphs)
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=ASCENT, descent=-DESCENT, lineGap=0)
    fb.setupNameTable({'familyName': 'Knight Hand', 'styleName': 'Regular'})
    fb.setupOS2(sTypoAscender=ASCENT, sTypoDescender=-DESCENT, sTypoLineGap=0,
                usWinAscent=ASCENT, usWinDescent=DESCENT, fsSelection=0x40 | 0x80,
                sxHeight=5 * PX, sCapHeight=7 * PX)
    fb.setupPost()
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'knight.ttf')
    fb.save(out)
    print(out, os.path.getsize(out), 'bytes,', len(chars), 'glyphs')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""The Delve's inks as a pxl palette, for the art team's HD packs.

The game paints every sprite and tile in its inks: one character a colour,
the table in crates/delve/src/render/overworld/ink.rs (`fn ink`, seventy-four
`'c' => 0xRRGGBB` arms: the realm's five banks and the two dyes, violet and
pink). pxl (pixel art as code,
github.com/newjordan/claude-pxl) reads a `.hex` palette a colour a line,
`rrggbb k comment`, the one character after the colour its key. This writes
that file from the table, each ink keyed by its own character and its bank
as the comment, so a drawing written in pxl uses the game's colours by the
game's names:

    python3 scripts/delve/inks_hex.py > inks.hex        # beside the .px.js
    pal(use("inks.hex"))                                # in the .px.js

Digits and `w a @ $ c` are the signal bank: game state (a knight's mark, a
foe's eyes), never decoration. The dyes are `d ( ) [ ] ^` (violet, deep to
pale) and `! ? ; % +` (pink): plain material like the realm's own. `.`,
`_`, `#` and space are pxl's own and no ink uses them. Read again whenever
the table changes; nothing is cached.

The wish painter reads its own copy (~/.angelX/wish-paint/inks.hex): write
this there too when the table changes.
"""
import re
import sys
from pathlib import Path

INK_RS = Path(__file__).resolve().parents[2] / "crates/delve/src/render/overworld/ink.rs"
# pxl keeps these for itself (transparent, erase, space, a hex colour), and
# the game's own readers keep `|` (frames), `/` (an icon's rows) and the
# header's `,` `:` `=`.
RESERVED = set("._ #|/,:=")


def inks(text):
    """The table's arms in order: (key, rrggbb, bank)."""
    body = text[text.index("pub fn ink(ch: char)"):]
    body = body[: body.index("_ => return None")]
    bank, out = "", []
    for line in body.splitlines():
        line = line.strip()
        comment = re.fullmatch(r"//\s*(.+)", line)
        if comment:
            bank = comment.group(1)
            continue
        arm = re.fullmatch(r"'(.)'\s*=>\s*0x([0-9a-fA-F]{6}),", line)
        if arm:
            out.append((arm.group(1), arm.group(2).lower(), bank))
    return out


def main():
    table = inks(INK_RS.read_text())
    keys = [k for k, _, _ in table]
    if len(set(keys)) != len(keys):
        sys.exit("inks_hex: an ink character is used twice")
    if RESERVED & set(keys):
        sys.exit("inks_hex: an ink uses a character pxl or the game keeps for itself")
    for key, rgb, bank in table:
        print(f"{rgb} {key} {bank}")
    print(f"{len(table)} inks", file=sys.stderr)


if __name__ == "__main__":
    main()

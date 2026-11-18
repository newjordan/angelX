#!/usr/bin/env python3
import re
import sys
from pathlib import Path

INK_RS = Path(__file__).resolve().parents[2] / "crates/delve/src/render/overworld/ink.rs"
RESERVED = set("._ #|/,:=")


def inks(text):
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

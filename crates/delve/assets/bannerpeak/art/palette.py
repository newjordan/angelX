"""The realm's ink table, read straight from ink.rs so previews match the game."""
import re
from pathlib import Path

SRC = Path(__file__).resolve().parents[3] / "src/render/overworld/ink.rs"


def load():
    text = SRC.read_text()
    start = text.index("pub fn ink(ch: char)")
    body = text[start : text.index("_ => return None", start)]
    pal = {}
    for m in re.finditer(r"'(.)' => 0x([0-9a-fA-F]{6})", body):
        pal[m.group(1)] = tuple(int(m.group(2)[i : i + 2], 16) for i in (0, 2, 4))
    return pal


PAL = load()

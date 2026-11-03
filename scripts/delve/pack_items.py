#!/usr/bin/env python3
"""Draw cards with the Demonic Dungeon item icons (VEXED, CC BY 4.0).

    python3 scripts/delve/pack_items.py CARD_ID...

Each named card under `cockpit/assets/dungeon/cards/` whose name names a
thing the pack draws takes that icon as its 16x16 art, each
colour as its nearest ink. Cards that share an icon take it in turn across
the category's icons, then tinted, so no two cards in the book look alike.
A card the pack has nothing for, or whose category's looks are all spoken
for, keeps its art.
"""
import re
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SHEET = ROOT / "cockpit/assets/dungeon/packs/demonic-dungeon/Items.png"
INK_RS = ROOT / "crates/delve/src/render/overworld/ink.rs"
CARDS = ROOT / "cockpit/assets/dungeon/cards"

# Category: (sheet row, columns, words in a card's name). First match wins. A
# card's text is not read: "a ring of shots" is no ring.
CATEGORIES = [
    ("crossbow", 2, [0, 1], ["crossbow"]),
    ("quiver", 2, [4, 5, 6, 7], ["quiver"]),
    ("arrow", 2, [8, 9], ["arrow", "bolt", "dart"]),
    ("bow", 2, [2, 3], ["bow"]),
    ("potion", 0, [0, 1, 2, 3, 4], ["potion", "flask", "ampoule", "tonic", "draught", "elixir", "phial", "decanter", "vial", "bottle", "cordial", "brew", "antidote"]),
    ("axe", 1, [0, 1], ["axe", "cleaver", "hatchet"]),
    ("hammer", 1, [2, 3], ["hammer", "maul", "gavel", "mallet"]),
    ("sword", 1, [4, 5], ["sword", "blade", "sabre", "rapier", "dagger", "bayonet", "knife"]),
    ("staff", 1, [6, 7], ["staff", "wand", "sceptre", "scepter", "rod", "barspoon", "ladle", "fork"]),
    ("armour", 3, [0, 1, 2, 3, 4, 5], ["mail", "armour", "armor", "harness", "cuirass", "vest", "coat", "pauldron", "plate", "jerkin"]),
    ("ring", 4, [0, 1, 2], ["ring", "band", "signet"]),
    ("amulet", 4, [3, 4, 5, 6, 7, 8], ["amulet", "locket", "pendant", "medallion", "necklace", "rosary", "charm", "talisman"]),
    ("coin", 5, [0, 1, 2, 3, 4, 5], ["coin", "mint", "purse", "tithe", "gold", "mortgage"]),
    ("key", 6, [0, 1], ["key"]),
    ("lock", 6, [2, 3], ["lock", "padlock"]),
    ("tome", 7, [0, 1, 2, 3], ["book", "tome", "testament", "codex", "almanac", "ledger", "hymnal"]),
    ("scroll", 8, list(range(10)), ["scroll", "writ", "letter", "ticket", "postmark", "decree", "receipt", "map", "punchcard", "menu", "rebuttal"]),
    ("mushroom", 9, [3, 4, 5], ["mushroom", "spore", "fungus", "toadstool"]),
    ("food", 9, [0, 1, 2], ["bread", "meat", "pie", "cake", "tart", "cheese", "bisque", "fig", "pear", "cracker", "dumpling", "sorbet", "honey", "tea", "skewer", "stew", "soup", "ham", "plum"]),
    ("light", 10, [0, 1], ["torch", "candle", "wick", "votive"]),
    ("gem", 11, list(range(10)), ["gem", "crystal", "shard", "prism", "jewel", "geode", "diamond"]),
]
# Tints for a repeat, after the category's own icons are used up.
TINTS = [[], [("7", "u"), ("R", "B")], [("7", "Y"), ("@", "t")], [("7", "%"), ("@", "9")], [("7", "h"), ("R", "G")], [("@", "4"), ("7", "(")]]


def inks():
    s = INK_RS.read_text()
    i = s.index("pub fn ink(ch: char) -> Option<Rgb> {")
    body = s[i : s.index("\n}\n", i)]
    out = {}
    for chars, h in re.findall(r"((?:'.'\s*\|\s*)*'.')\s*=>\s*0x([0-9a-fA-F]{6})", body):
        for c in re.findall(r"'(.)'", chars):
            out[c] = tuple(int(h[k : k + 2], 16) for k in (0, 2, 4))
    return out


def icon(im, table, near, row, col):
    rows = []
    for y in range(16):
        line = ""
        for x in range(16):
            r, g, b, a = im.getpixel((col * 16 + x, row * 16 + y))
            if not a:
                line += "."
                continue
            if (r, g, b) not in near:
                near[(r, g, b)] = min(table, key=lambda c: sum((p - q) ** 2 for p, q in zip((r, g, b), table[c])))
            line += near[(r, g, b)]
        rows.append(line)
    return rows


def tint(rows, pairs):
    m = dict(pairs)
    return ["".join(m.get(c, c) for c in r) for r in rows]


def art_of(text):
    lines = text.split("\n")
    i = lines.index("art")
    return lines[i + 1 :]


def category(card_text):
    def field(name):
        m = re.search(rf"(?m)^{name} (.*)$", card_text)
        return m.group(1).lower() if m else ""

    words = field("name")
    for cat, row, cols, keys in CATEGORIES:
        if any(re.search(rf"\b{k}s?\b", words) for k in keys):
            return cat, row, cols
    return None


def main(ids):
    table = inks()
    near = {}
    im = Image.open(SHEET).convert("RGBA")
    taken = {"\n".join(r for r in art_of(p.read_text()) if r.strip()) for p in CARDS.glob("*.card") if p.stem not in ids}
    turn = {}
    drawn = []
    for cid in ids:
        path = CARDS / f"{cid}.card"
        text = path.read_text()
        hit = category(text)
        if not hit:
            continue
        cat, row, cols = hit
        n = turn.get(cat, 0)
        rows = None
        while n < len(cols) * len(TINTS):
            col = cols[n % len(cols)]
            candidate = tint(icon(im, table, near, row, col), TINTS[n // len(cols)])
            n += 1
            if "\n".join(candidate) not in taken:
                rows = candidate
                break
        turn[cat] = n
        if rows is None:
            continue  # every look in the category is spoken for
        key = "\n".join(rows)
        taken.add(key)
        head = text[: text.index("\nart\n") + len("\nart\n")]
        path.write_text(head + "\n".join(rows) + "\n")
        drawn.append((cid, cat))
    for cid, cat in drawn:
        print(f"{cat:9s} {cid}")
    print(f"{len(drawn)} of {len(ids)} cards drawn from the pack")


if __name__ == "__main__":
    main(sys.argv[1:])

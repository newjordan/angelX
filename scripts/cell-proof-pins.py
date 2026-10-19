#!/usr/bin/env python3
"""Recompute the renderer-source pins in crates/delve/assets/arcade/cell-proof.json.

The delve build (crates/delve/build.rs -> arcade_gate::validate_cells) hashes
the renderer files listed under "sources" (FNV-1a, 64 bit, over the file's
bytes) and panics on a stale pin. Editing any of them, for instance
crates/delve/src/render/overworld/arena.rs or
cockpit/src/stage/world_viz/art.rs, needs this run before the next build:

    python3 scripts/cell-proof-pins.py            # rewrite the stale pins
    python3 scripts/cell-proof-pins.py --check    # list them, change nothing

Only the "sources" hashes move. The knight and effect frame evidence above them
is the arcade art's own (docs/ART-ARCADE.md) and is left exactly as it is: a
change to a pinned file that changes how those frames draw needs the full
arcade-review export instead.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROOF = ROOT / "crates/delve/assets/arcade/cell-proof.json"


def fingerprint(data: bytes) -> int:
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def main() -> int:
    check = "--check" in sys.argv[1:]
    text = PROOF.read_text()
    sources = json.loads(text)["sources"]
    head, sep, tail = text.partition('"sources"')
    stale = []
    for source in sources:
        name = source["file"]
        now = fingerprint((ROOT / name).read_bytes())
        if now != source["hash"]:
            stale.append(name)
            pattern = re.compile(r'("file":\s*"%s",\s*"hash":\s*)\d+' % re.escape(name))
            tail, n = pattern.subn(lambda m: m.group(1) + str(now), tail, count=1)
            assert n == 1, name
    for name in stale:
        print(("stale  " if check else "pinned ") + name)
    if not stale:
        print("all %d pins fresh" % len(sources))
    if stale and not check:
        PROOF.write_text(head + sep + tail)
    return 1 if stale and check else 0


if __name__ == "__main__":
    sys.exit(main())

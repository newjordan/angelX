#!/usr/bin/env python3
"""A placeholder HD pack from an art request: never art.

The Delve's art request (`ANGEL_HD_REQUEST=<dir> cargo test -p angel-delve
write_hd_request -- --ignored`) is a folder of 1x looks and a `pack.json`
listing them. This makes a pack of the same entries mechanically, to check
the HD path end to end before the art team's drawings land: each look
brought up to the pack's scale, nearest, with one pixel of every art pixel's
square lightened, so a pixel finer than the game's shows as a dot. A
mirrored entry keeps its `flip` and its file. The manifest says it is a
placeholder.

    python3 scripts/delve/hd_placeholder.py <request-dir> <pack-dir> [--only PREFIX]...

`--only knight-p1/whole-idle` makes just the entries whose names start so
(and the files their mirrors read). Then `ANGEL_GLASS_ART=<pack-dir>`.
"""
import json
import os
import sys

from PIL import Image

# How far the marked pixel of each art pixel's square goes toward white.
MARK = 0.45


def placeholder(src, scale):
    im = Image.open(src).convert("RGBA")
    big = im.resize((im.width * scale, im.height * scale), Image.NEAREST)
    px = big.load()
    for y in range(1, big.height, scale):
        for x in range(1, big.width, scale):
            r, g, b, a = px[x, y]
            if a:
                px[x, y] = tuple(int(c + (255 - c) * MARK) for c in (r, g, b)) + (a,)
    return big


def main(argv):
    only = [argv[i + 1] for i, a in enumerate(argv) if a == "--only" and i + 1 < len(argv)]
    rest = [a for i, a in enumerate(argv) if a != "--only" and (i == 0 or argv[i - 1] != "--only")]
    if len(rest) != 2:
        sys.exit(__doc__)
    request, pack = rest
    manifest = json.load(open(os.path.join(request, "pack.json")))
    scale = manifest.get("scale", 4)
    wanted = [e for e in manifest["art"] if not only or any(e["name"].startswith(p) for p in only)]
    # A mirror reads another entry's file: that one is made too.
    files = {e.get("file", e["name"] + ".png") for e in wanted}
    made = 0
    for name in sorted(files):
        src = os.path.join(request, name)
        if not os.path.isfile(src):
            sys.exit(f"hd_placeholder: {src} is not in the request")
        out = os.path.join(pack, name)
        os.makedirs(os.path.dirname(out), exist_ok=True)
        placeholder(src, scale).save(out)
        made += 1
    note = "PLACEHOLDER, not art: each look brought up nearest with a dot an art pixel (scripts/delve/hd_placeholder.py)"
    json.dump({"note": note, "scale": scale, "art": wanted}, open(os.path.join(pack, "pack.json"), "w"), indent=2)
    print(f"{len(wanted)} entries, {made} placeholder drawings in {pack}")


if __name__ == "__main__":
    main(sys.argv[1:])

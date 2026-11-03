#!/usr/bin/env python3
"""Build a named HD pack for the resolution ladder (docs/LADDER.md).

A knight climbs from the game's 16 px to 32, 64 and 128 as a run goes on
(`rules::resolution`). A window shows a figure's finer drawing when the
knight's rung allows and the pack (`ANGEL_GLASS_ART`, `arena::hd_pack`) has
one, found by the figure's name (`arena::figure_keys`):

    knight/<design>/<clip>/<frame>/seat<N>/<r|l>

This writes such a pack, every seat's colours made from the one drawing by
the game's own seat swaps (`realm_colours` in arena/knights.rs, read from the
source, applied in order as `Img::recolor` does), and each facing left the
right one turned over (`flip`, the file shared). The designs and the seats
that wear them are the game's (`quick_rig::Design`, read from quick_rig.rs:
seat 1 and 5 the great helm, 2 the sugarloaf crusader, 3 and 6 the plumed
bucket, 4 the kettle hat), each design's rig its `knight-<design>.json` in
crates/delve/assets/quick-rig.

  stills  Sol's touched stills, `<design>-<16|32|64|128>.png` in realm inks,
          the knight standing whole with its arm and shield: each one is the
          idle clip's every frame (as many as the design's rig has), drawn for
          whatever the knight carries (a portrait: the whole figure, no
          `dress`), in the colours of each seat that wears the design.

              ladder_pack.py stills <stills-dir> <pack-dir>
                  [--rungs 32,64,128] [--designs a,b] [--seats 1-6]
                  [--stand-in realm] [--rigs crates/delve/assets/quick-rig]

          `--seats` gives every design those seats' colours. `--stand-in
          realm` names each seat's still as the old knight too (`knight.json`,
          what a design that fails to read falls back to).

  rig     A rig JSON at any rung (version 1's shape, `quick_rig.rs`; its
          canvas the rung's, e.g. 184 x 184 for 64: no size limit here): each
          clip's frames, rows of realm inks, its pivot the feet. The body
          alone (`covers: body`): what the knight carries stays the game's,
          drawn over it. The frames must be the 16 px rig's, clip for clip,
          frame for frame.

              ladder_pack.py rig <rig.json> <pack-dir> --design NAME --rung 64
                  [--clips idle,walk] [--seats 1-6]

  designs Every design's animated frames at the higher rungs, from rigs at
          those rungs beside the 16 px ones (`knight-<design>@<rung>.json` in
          `--rigs`), each as `rig` makes them, for the seats that wear it.
          `--placeholder` stands each design's 16 px rig in where a rung has
          no rig of its own, brought up nearest with one pixel of every art
          pixel lightened (never art: it proves the frames land).

              ladder_pack.py designs <pack-dir> [--rungs 32,64,128]
                  [--rigs DIR] [--clips idle,walk] [--placeholder]

  enlarge A 16 px rig brought up to a rung, nearest, its pivot and sockets
          with it: the canvas to redraw a design's frames at that rung on.

              ladder_pack.py enlarge <rig.json> <out.json> --rung 64

Run any of them again into the same folder and it adds to the pack (an entry
of the same key, rung, dress and cover is replaced). Then
`ANGEL_GLASS_ART=<pack-dir>`, and `ANGEL_DELVE_LADDER=64` to see the 64 rung
on floor one.
"""
import argparse
import json
import re
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
KNIGHTS_RS = ROOT / "crates/delve/src/render/overworld/arena/knights.rs"
QUICK_RIG_RS = ROOT / "crates/delve/src/render/overworld/arena/quick_rig.rs"
RIGS = ROOT / "crates/delve/assets/quick-rig"
MANIFEST = "pack.json"
RUNGS = (16, 32, 64, 128)
SEATS = range(1, 7)
# How far the placeholder's marked pixel goes toward white.
MARK = 0.45

sys.path.insert(0, str(Path(__file__).resolve().parent))
from inks_hex import INK_RS, inks  # noqa: E402  (the game's ink table)


def ink_table():
    """The game's inks: character to (r, g, b)."""
    return {k: tuple(bytes.fromhex(rgb)) for k, rgb, _ in inks(INK_RS.read_text())}


def fn_body(text, name):
    start = text.index(f"fn {name}(")
    end = text.index("\n}\n", start)
    return text[start:end]


def arms(body):
    """A seat match's arms: (pattern, [(from, to)...] or the function it defers to)."""
    out = []
    for m in re.finditer(r"(\d+|_)\s*=>\s*(&\[(.*?)\]|(\w+)\(id\))", body):
        pat = m.group(1)
        if m.group(3) is not None:
            out.append((pat, re.findall(r"\('(.)',\s*'(.)'\)", m.group(3))))
        else:
            out.append((pat, m.group(4)))
    return out


def designs_table():
    """`quick_rig::Design`: each design's name, and the seats that wear it
    (`Design::of_seat`), in the game's order."""
    text = QUICK_RIG_RS.read_text()
    names = dict(re.findall(r"Self::(\w+) => \"([a-z-]+)\"", fn_body(text, "name")))
    order = re.findall(r"Self::(\w+)", text[text.index("pub const ALL: [Self; 4]"):text.index("];", text.index("pub const ALL: [Self; 4]"))])
    seats = {}
    rest = None
    for pats, variant in re.findall(r"([\d |_]+?)\s*=>\s*Self::(\w+)", fn_body(text, "of_seat")):
        for pat in pats.split("|"):
            pat = pat.strip()
            if pat == "_":
                rest = variant
            elif pat:
                seats[int(pat)] = variant
    table = {}
    for seat in SEATS:
        table.setdefault(names[seats.get(seat, rest)], []).append(seat)
    return [(names[v], table.get(names[v], [])) for v in order]


def design_rig(name, rung=16, rigs=RIGS):
    """A design's rig JSON at a rung (`knight-<design>.json`, or
    `knight-<design>@<rung>.json` above 16), if there is one."""
    path = Path(rigs) / (f"knight-{name}.json" if rung == 16 else f"knight-{name}@{rung}.json")
    return json.loads(path.read_text()) if path.is_file() else None


def seat_swaps(seat):
    """The swaps the game makes on the rig's knight for `seat`
    (`realm_colours`, deferring to `seat_colours`), in order."""
    text = KNIGHTS_RS.read_text()
    tables = {name: arms(fn_body(text, name)) for name in ("realm_colours", "seat_colours")}

    def pick(name):
        for pat, value in tables[name]:
            if pat == "_" or int(pat) == seat:
                return pick(value) if isinstance(value, str) else value
        return []

    return pick("realm_colours")


def recolor(im, swaps, table):
    """`Img::recolor`: each pair in order, every pixel of exactly the
    first ink's colour (opaque) becomes the second's."""
    out = im.copy()
    px = out.load()
    for a, b in swaps:
        fa, fb = table.get(a), table.get(b)
        if fa is None or fb is None:
            continue
        for y in range(out.height):
            for x in range(out.width):
                r, g, bb, al = px[x, y]
                if al == 255 and (r, g, bb) == fa:
                    px[x, y] = fb + (255,)
    return out


def parse_seats(text):
    seats = set()
    for part in text.split(","):
        if "-" in part:
            a, b = part.split("-")
            seats.update(range(int(a), int(b) + 1))
        else:
            seats.add(int(part))
    return sorted(seats)


def feet_x(im):
    """The middle between a standing figure's two feet: the two widest runs
    of ink across the lowest eighth of it (at least two rows)."""
    w, h = im.size
    px = im.load()
    band = max(2, h // 8)
    cols = [any(px[x, y][3] for y in range(h - band, h)) for x in range(w)]
    runs, start = [], None
    for x, on in enumerate(cols + [False]):
        if on and start is None:
            start = x
        elif not on and start is not None:
            runs.append((start, x - 1))
            start = None
    if not runs:
        return w // 2
    widest = sorted(runs, key=lambda r: r[1] - r[0], reverse=True)[:2]
    centres = [(a + b) / 2 for a, b in widest]
    return round(sum(centres) / len(centres))


def lowest(im):
    w, h = im.size
    px = im.load()
    for y in range(h - 1, -1, -1):
        if any(px[x, y][3] for x in range(w)):
            return y
    return h - 1


class Pack:
    def __init__(self, out):
        self.out = Path(out)
        self.out.mkdir(parents=True, exist_ok=True)
        path = self.out / MANIFEST
        self.manifest = json.loads(path.read_text()) if path.is_file() else {"scale": 4, "art": []}
        self.manifest.setdefault("art", [])

    def put(self, entry):
        same = lambda e: (e.get("key"), e.get("rung"), e.get("dress"), e.get("covers")) == (
            entry.get("key"), entry.get("rung"), entry.get("dress"), entry.get("covers"))
        self.manifest["art"] = [e for e in self.manifest["art"] if not same(e)] + [entry]

    def save_png(self, rel, im):
        path = self.out / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        im.save(path)

    def write(self):
        self.manifest["art"].sort(key=lambda e: (e.get("key", ""), e.get("rung", 0)))
        # One entry a line: small to send to a page, and still read by eye.
        art = ",\n ".join(json.dumps(e, separators=(",", ":")) for e in self.manifest["art"])
        head = {k: v for k, v in self.manifest.items() if k != "art"}
        text = json.dumps(head, separators=(",", ":"))[:-1] + (',"art":[\n ' + art + "\n]}" if art else ',"art":[]}')
        (self.out / MANIFEST).write_text(text + "\n")
        return len(self.manifest["art"])


def both_ways(pack, key_right, rung, file, pivot, extra=None):
    """An entry facing right and its mirror facing left, one file."""
    for facing, flip in (("r", False), ("l", True)):
        entry = {"key": key_right[:-1] + facing, "rung": rung, "file": file, "pivot": pivot}
        if flip:
            entry["flip"] = True
        entry.update(extra or {})
        pack.put(entry)


def rig_frames(path):
    rig = json.loads(Path(path).read_text())
    return rig


def stills(args):
    table = ink_table()
    pack = Pack(args.pack)
    rungs = [int(r) for r in args.rungs.split(",")]
    wearers = dict(designs_table())
    designs = args.designs.split(",") if args.designs else list(wearers)
    made = 0
    for design in designs:
        rig16 = design_rig(design, 16, args.rigs)
        if rig16 is None:
            sys.exit(f"{design}: no rig knight-{design}.json in {args.rigs}")
        idle = len(rig16["clips"]["idle"])
        seats = parse_seats(args.seats) if args.seats else wearers.get(design, [])
        small = Image.open(Path(args.stills) / f"{design}-16.png").convert("RGBA")
        x16 = feet_x(small)
        for rung in rungs:
            src = Path(args.stills) / f"{design}-{rung}.png"
            if not src.is_file():
                print(f"{design}: no {rung} still", file=sys.stderr)
                continue
            im = Image.open(src).convert("RGBA")
            # Its feet: between its two feet at the bottom, checked against
            # the 16's feet carried up by its width (a still's own feet are
            # read where they are clear; a weapon by them is not a foot).
            mapped = round((x16 + 0.5) / small.width * im.width - 0.5)
            own = feet_x(im)
            x = own if abs(own - mapped) <= im.width // 10 else mapped
            pivot = [x, lowest(im)]
            print(f"{design} {rung}: {im.width}x{im.height}, feet {pivot} (own {own}, from the 16 {mapped})")
            for seat in seats:
                tinted = recolor(im, seat_swaps(seat), table)
                file = f"knight/{design}/idle/seat{seat}@{rung}.png"
                pack.save_png(file, tinted)
                names = [design] + ([args.stand_in] if args.stand_in and seat in wearers.get(design, []) else [])
                for name in names:
                    for frame in range(idle):
                        both_ways(pack, f"knight/{name}/idle/{frame}/seat{seat}/r", rung, file, pivot)
                made += 1
    print(f"{made} drawings; {pack.write()} entries in {pack.out / MANIFEST}")


def rig(args):
    pack = Pack(args.pack)
    rung = int(args.rung)
    if rung not in RUNGS[1:]:
        sys.exit(f"--rung {rung}: 32, 64 or 128")
    clips = args.clips.split(",") if args.clips else None
    made = body_frames(pack, rig_frames(args.rig), args.design, rung, clips, parse_seats(args.seats))
    print(f"{made} drawings; {pack.write()} entries in {pack.out / MANIFEST}")


def enlarged(data, scale):
    """A rig brought up `scale` times, nearest: each pixel a square, the
    pivot and sockets the middle-bottom of theirs (`[x * s + s / 2, y * s + s - 1]`
    for the feet, the middle for the rest)."""
    w, h = data["canvas"]
    px, py = data["pivot"]
    out = {"version": data.get("version", 1), "canvas": [w * scale, h * scale], "pivot": [px * scale + scale // 2, py * scale + scale - 1], "clips": {}}
    for clip, frames in data["clips"].items():
        out["clips"][clip] = [{
            "ticks": f["ticks"],
            "rows": [r for row in f["rows"] for r in [("".join(c * scale for c in row))] * scale],
            "anchors": {k: [x * scale + scale // 2, y * scale + (scale - 1 if k == "feet" else scale // 2)] for k, (x, y) in f["anchors"].items()},
        } for f in frames]
    return out


def enlarge(args):
    rung = int(args.rung)
    if rung not in RUNGS[1:]:
        sys.exit(f"--rung {rung}: 32, 64 or 128")
    out = enlarged(rig_frames(args.rig), rung // 16)
    Path(args.out).write_text(json.dumps(out, separators=(",", ":")) + "\n")
    print(f"{args.out}: {out['canvas'][0]} x {out['canvas'][1]}, pivot {out['pivot']}")


def designs(args):
    pack = Pack(args.pack)
    rungs = [int(r) for r in args.rungs.split(",")]
    clips = args.clips.split(",") if args.clips else None
    made = 0
    for design, seats in designs_table():
        for rung in rungs:
            data = design_rig(design, rung, args.rigs)
            placeholder = False
            if data is None and args.placeholder:
                data, placeholder = enlarged(design_rig(design, 16, args.rigs), rung // 16), True
            if data is None:
                print(f"{design} {rung}: no knight-{design}@{rung}.json", file=sys.stderr)
                continue
            n = body_frames(pack, data, design, rung, clips, seats, mark=rung // 16 if placeholder else 0)
            print(f"{design} {rung}: {n} drawings{' (placeholder)' if placeholder else ''}, seats {seats}")
            made += n
    print(f"{made} drawings; {pack.write()} entries in {pack.out / MANIFEST}")


def body_frames(pack, data, design, rung, clips, seats, mark=0):
    """Each clip's frames of rig `data` (at `rung`) as body-alone drawings of
    `design` in each of `seats`' colours, both facings; with `mark`, a
    placeholder's dot on every `mark`-square. How many drawings."""
    table = ink_table()
    w, h = data["canvas"]
    pivot = list(data["pivot"])
    made = 0
    for clip in clips or list(data["clips"]):
        for i, frame in enumerate(data["clips"].get(clip, [])):
            rows = frame["rows"]
            im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
            px = im.load()
            for y, row in enumerate(rows):
                for x, c in enumerate(row):
                    if c != "." and c in table:
                        px[x, y] = table[c] + (255,)
            # Cut to its ink, its feet moved with it: a drawing lands by its
            # feet whatever its size, and the clear round it is wire and
            # texture for nothing.
            box = im.getbbox()
            if box is None:
                continue
            im = im.crop(box)
            feet = [pivot[0] - box[0], pivot[1] - box[1]]
            if not (0 <= feet[0] < im.width and 0 <= feet[1] < im.height):
                # Feet outside the ink (a leap): kept, the cut grown to them.
                grown = Image.new("RGBA", (max(im.width, feet[0] + 1) - min(0, feet[0]), max(im.height, feet[1] + 1) - min(0, feet[1])))
                grown.paste(im, (-min(0, feet[0]), -min(0, feet[1])))
                im, feet = grown, [max(0, feet[0]), max(0, feet[1])]
            for seat in seats:
                file = f"knight/{design}/{clip}/{i}/seat{seat}@{rung}.png"
                drawn = recolor(im, seat_swaps(seat), table)
                if mark:
                    dot(drawn, mark, (box[0] % mark, box[1] % mark))
                pack.save_png(file, drawn)
                both_ways(pack, f"knight/{design}/{clip}/{i}/seat{seat}/r", rung, file, feet, {"covers": "body"})
                made += 1
    return made


def dot(im, n, origin):
    """One pixel of every n-square lightened (a placeholder: never art)."""
    px = im.load()
    for y in range((1 - origin[1]) % n, im.height, n):
        for x in range((1 - origin[0]) % n, im.width, n):
            r, g, b, a = px[x, y]
            if a:
                px[x, y] = (int(r + (255 - r) * MARK), int(g + (255 - g) * MARK), int(b + (255 - b) * MARK), a)


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="how", required=True)
    s = sub.add_parser("stills")
    s.add_argument("stills")
    s.add_argument("pack")
    s.add_argument("--rungs", default="32,64,128")
    s.add_argument("--designs")
    s.add_argument("--seats")
    s.add_argument("--stand-in")
    s.add_argument("--rigs", default=str(RIGS))
    r = sub.add_parser("rig")
    r.add_argument("rig")
    r.add_argument("pack")
    r.add_argument("--design", required=True)
    r.add_argument("--rung", required=True)
    r.add_argument("--clips")
    r.add_argument("--seats", default="1-6")
    d = sub.add_parser("designs")
    d.add_argument("pack")
    d.add_argument("--rungs", default="32,64,128")
    d.add_argument("--rigs", default=str(RIGS))
    d.add_argument("--clips")
    d.add_argument("--placeholder", action="store_true")
    e = sub.add_parser("enlarge")
    e.add_argument("rig")
    e.add_argument("out")
    e.add_argument("--rung", required=True)
    args = ap.parse_args(argv)
    {"stills": stills, "rig": rig, "designs": designs, "enlarge": enlarge}[args.how](args)


if __name__ == "__main__":
    main(sys.argv[1:])

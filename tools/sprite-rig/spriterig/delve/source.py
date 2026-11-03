"""Read delve's own art straight out of the angelX Rust source: the ink palette
(render/overworld/ink.rs) and ink-row drawings (`const NAME: [&str; N] = [...]`,
the hero's `Parts` layers). Nothing is redrawn by hand: if the art changes in the
game, re-running picks it up. This parser is for creative cut-outs only, NOT native game parity. Use
spriterig.native and a Rust frame export for authoritative body art."""
import json, os, re

def _find_root():
    """ANGELX_ROOT env, else the repo this tool lives in (tools/sprite-rig/../..), else /workspace/angelX."""
    if os.environ.get("ANGELX_ROOT"):
        return os.environ["ANGELX_ROOT"]
    here = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
    for r in (here, "/workspace/angelX"):
        if os.path.isdir(os.path.join(r, "crates", "delve")):
            return r
    return here


DEFAULT_ROOT = _find_root()
OVERWORLD = "crates/delve/src/render/overworld"
_STR = re.compile(r'"((?:[^"\\]|\\.)*)"')


def _read(root, rel):
    with open(os.path.join(root, OVERWORLD, rel)) as f:
        return f.read()


def palette(root=DEFAULT_ROOT):
    """ink char -> (r, g, b), exactly as delve's `ink()`."""
    src = _read(root, "ink.rs")
    body = src[src.index("pub fn ink(ch: char)"):]
    body = body[:body.index("_ => return None")]
    return {c: tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))
            for c, h in re.findall(r"'(.)' => 0x([0-9a-fA-F]{6})", body)}


def const_rows(root, rel, name):
    src = _read(root, rel)
    m = re.search(r"const %s: \[&str; \d+\] = \[(.*?)\];" % re.escape(name), src, re.S)
    if not m:
        raise KeyError(f"{name} not found in {rel}")
    return _STR.findall(m.group(1))


def _field(block, name):
    m = re.search(r"\b%s: &\[(.*?)\]," % name, block, re.S)
    return _STR.findall(m.group(1))


def _layers(block):
    """Every `Layer { at: (x, y), rows: &[...] }` in order."""
    out = []
    for m in re.finditer(r"Layer \{\s*at: \((-?\d+), (-?\d+)\),\s*rows: &\[(.*?)\],?\s*\}", block, re.S):
        out.append({"at": [int(m.group(1)), int(m.group(2))], "rows": _STR.findall(m.group(3))})
    return out


def hero_parts(root=DEFAULT_ROOT, view="ACROSS"):
    """The realm's hero, layer by layer (kit/hero.rs). ACROSS is the side view,
    facing right: helm 8 rows at y0, harness 6 rows at y8, legs 2 rows at y14."""
    src = _read(root, "kit/hero.rs")
    a = src.index("const %s: Parts = Parts {" % view)
    nxt = src.find("\nconst ", a + 10)
    block = src[a:nxt if nxt > 0 else None]
    steel = re.search(r"steel: \[(.*?)\],\s*skin:", block, re.S).group(1)
    steel = [_STR.findall(s) for s in re.findall(r"&\[(.*?)\]", steel, re.S)]
    layers = _layers(block)  # shield, then held[bow rest, bow strike, xbow.., gonne.., sword rest, sword strike]
    return {"helm": _field(block, "helm"), "bare": _field(block, "bare"),
            "harness": _field(block, "harness"), "steel": steel, "shield": layers[0],
            "bow": layers[1:3], "crossbow": layers[3:5], "handgonne": layers[5:7],
            "sword": layers[7:9]}


def seat_colours(root=DEFAULT_ROOT, fn="seat_colours"):
    """knights.rs `seat_colours` (or `realm_colours`): seat id -> [(from, to)] ink swaps.

    Literal arms (`4 => &[('7', '4'), ...]`) are copied as written. A call arm such as
    `_ => seat_colours(id)` is resolved by parsing that function and filling any key it
    already defined. An arm that still cannot be read is left out, so a missing pattern
    does not raise.
    """
    src = _read(root, "arena/knights.rs")
    return _colour_arms(src, fn)


def _colour_arms(src, fn, seen=None):
    seen = set() if seen is None else set(seen)
    if fn in seen:
        return {}
    seen.add(fn)
    try:
        body = src[src.index("fn " + fn):]
        body = body[:body.index("\n}\n")]
    except ValueError:
        return {}
    out = {}
    for arm in re.finditer(r"(\d+|_) => &\[(.*?)\]", body, re.S):
        out[arm.group(1)] = re.findall(r"\('(.)', '(.)'\)", arm.group(2))
    # Non-literal arms: `_ => seat_colours(id)`, or a seat that forwards to another fn.
    for arm in re.finditer(r"(\d+|_) =>\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(", body):
        key, callee = arm.group(1), arm.group(2)
        other = _colour_arms(src, callee, seen)
        if key == "_":
            for k, swaps in other.items():
                out.setdefault(k, swaps)
        elif key not in out:
            if key in other:
                out[key] = other[key]
            elif "_" in other:
                out[key] = other["_"]
    return out


def goblin_rows(root=DEFAULT_ROOT, running=False, step=True):
    """sprites.rs goblin_raw's body with its standing (or running) legs."""
    src = _read(root, "arena/sprites.rs")
    f = src[src.index("fn goblin_raw"):]
    legs = re.search(r"let legs: \[&str; 2\] = match \(running, step\) \{(.*?)\};", f, re.S).group(1)
    arms = {k: _STR.findall(v) for k, v in re.findall(r"\(([^)]*)\) => \[(.*?)\]", legs)}
    key = "false, _" if not running else ("true, true" if step else "true, false")
    body = re.search(r"let body = rows\(&\[(.*?)\]\);", f, re.S).group(1)
    rows = _STR.findall(body)
    return rows + arms[key]


def dwarf_rows(root=DEFAULT_ROOT):
    """knights.rs dwarf_body(0, axes): left half mirrored to sixteen."""
    src = _read(root, "arena/knights.rs")
    f = src[src.index("fn dwarf_body"):]
    f = f[:f.index("let mut halves")]
    get = lambda pat: _STR.findall(re.search(pat, f, re.S).group(1))
    head = get(r"0 => \[(.*?)\],")
    face = get(r"let face = \[(.*?)\];")
    arms = get(r"\} else \{\s*\[(.*?)\]\s*\};\s*let legs")
    legs = get(r"let legs.*?\} else \{\s*\[(.*?)\]\s*\};")
    mid = [".hh.RoRo", "..B.RRoR"]
    halves = head + face + mid + arms + legs

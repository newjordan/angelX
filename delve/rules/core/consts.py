#!/usr/bin/env python3
"""Emit Bend zero-argument defs for the scalar `const`s of a group's Rust modules.

    python3 delve/rules/core/consts.py <out.bend> <module-file.rs>...

`const NAME: u32 = 4 * HZ;` in crates/delve/src/rules/foo.rs becomes
`def foo.NAME() -> U32:`. Integer and f32 arithmetic is evaluated with Rust's
rules (f32 rounding after every operation); anything else (arrays, tuples,
structs) is listed as skipped, to be written by hand.
"""
import ast, pathlib, re, struct, sys

SRC = pathlib.Path(__file__).resolve().parents[3] / "crates/delve/src"


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


class F(float):
    pass


def module_name(path):
    rel = path.relative_to(SRC).with_suffix("")
    parts = list(rel.parts)
    if parts[0] == "rules" and len(parts) > 1:
        parts = parts[1:]
    return ".".join(parts)


ENV_GLOBAL = {"HZ": 30, "TILE_UNITS": F(2.0), "MAX_PLAYERS": 6, "COLS": 24, "ROWS": 14}


def ev(node, env):
    if isinstance(node, ast.Constant):
        v = node.value
        if isinstance(v, float):
            return F(f32(v))
        return v
    if isinstance(node, ast.Name):
        if node.id in env:
            return env[node.id]
        raise KeyError(node.id)
    if isinstance(node, ast.UnaryOp):
        v = ev(node.operand, env)
        if isinstance(node.op, ast.USub):
            return F(-v) if isinstance(v, F) else -v
        raise KeyError("unary")
    if isinstance(node, ast.BinOp):
        a, b = ev(node.left, env), ev(node.right, env)
        if isinstance(a, F) or isinstance(b, F):
            a, b = float(a), float(b)
            r = {ast.Add: a + b, ast.Sub: a - b, ast.Mult: a * b, ast.Div: a / b if b else float("nan")}[type(node.op)]
            return F(f32(r))
        op = type(node.op)
        if op is ast.Add: return a + b
        if op is ast.Sub: return a - b
        if op is ast.Mult: return a * b
        if op is ast.Div: return a // b
        if op is ast.Mod: return a % b
        if op is ast.LShift: return a << b
        if op is ast.RShift: return a >> b
        if op is ast.BitOr: return a | b
        if op is ast.BitAnd: return a & b
        raise KeyError("op")
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "AS":
        return node.args[0] and ev(node.args[0], env)
    raise KeyError(type(node).__name__)


def rust_expr(e):
    e = re.sub(r"\bsuper::", "", e)
    e = re.sub(r"\bcrate::rules::", "", e)
    e = re.sub(r"\b\w+::(?=[A-Z_]+\b)", "", e)  # mod::NAME -> NAME
    e = re.sub(r"(\d)_(\d)", r"\1\2", e)
    e = re.sub(r"(\d)_(\d)", r"\1\2", e)
    e = re.sub(r"(\b0x[0-9a-fA-F]+)(?:u\d+|usize|i\d+)\b", r"\1", e)
    e = re.sub(r"\b(\d+(?:\.\d+)?)(?:_?f32|_?u32|_?u8|_?usize|_?i32)\b", r"\1", e)
    e = re.sub(r"\bas\s+(usize|u32|u8|u16|i32|u64|i64)\b", "", e)
    m = re.search(r"\bas\s+f32\b", e)
    while m:
        # `X as f32` -> float(X)
        pre = e[: m.start()].rstrip()
        mm = re.search(r"(\(?[\w.]+\)?)$", pre)
        e = pre[: mm.start()] + f"FLOAT({mm.group(1)})" + e[m.end():]
        m = re.search(r"\bas\s+f32\b", e)
    return e


def bend_ty(t):
    return {"f32": "F32", "bool": "Bool", "&str": "String", "&'static str": "String", "u64": "W.W", "i64": "W.W"}.get(t, "U32")


def lit(v, t):
    if t in ("&str", "&'static str"):
        return '"' + v.replace("\\", "\\\\").replace('"', '\\"') + '"'
    if t == "bool":
        return "True{}" if v else "False{}"
    if t == "f32":
        s = repr(float(v))
        if "e" in s or "inf" in s or "nan" in s:
            raise KeyError("float form")
        return s if "." in s else s + ".0"
    v = int(v)
    if t in ("u64", "i64"):
        v &= (1 << 64) - 1
        return "W.W{%d, %d}" % (v >> 32, v & 0xFFFFFFFF)
    if t in ("i32", "i8", "i16", "isize"):
        v &= 0xFFFFFFFF
    if not 0 <= v < 1 << 32:
        raise KeyError("wide")
    return str(v)


def main():
    out, files = sys.argv[1], sys.argv[2:]
    body, skipped = [], []
    for f in files:
        p = pathlib.Path(f).resolve()
        mod = module_name(p)
        text = p.read_text()
        env = dict(ENV_GLOBAL)
        for m in re.finditer(r"^(?:pub(?:\([a-z]+\))? )?(?:const|static) (\w+): ([^=;]+?) = ([^;]+);", text, re.M):
            name, ty, expr = m.group(1), m.group(2).strip(), m.group(3).strip()
            try:
                if ty in ("&str", "&'static str"):
                    sm = re.fullmatch(r'"((?:[^"\\]|\\.)*)"', expr)
                    if not sm:
                        raise KeyError("str")
                    v = bytes(sm.group(1), "utf-8").decode("unicode_escape").encode("latin-1").decode("utf-8")
                elif ty == "bool":
                    v = {"true": True, "false": False}[expr]
                else:
                    e = rust_expr(expr)
                    e = re.sub(r"\bFLOAT\(", "AS(", e) if False else e
                    tree = ast.parse(e, mode="eval").body
                    env2 = dict(env)
                    v = ev_float(tree, env2) if "FLOAT(" in e else ev(tree, env2)
                    if ty == "f32":
                        v = F(f32(float(v)))
                body.append(f"def {mod}.{name}() -> {bend_ty(ty)}:\n  {lit(v, ty)}\n")
                env[name] = v
            except Exception as ex:
                skipped.append(f"{mod}.{name}: {ty} = {expr}  ({ex!r})")
    pathlib.Path(out).write_text("# Generated by delve/tools/consts.py from the Rust consts: zero-argument defs.\n\n" + "\n".join(body))
    print(f"{len(body)} consts written, {len(skipped)} skipped")
    for s in skipped:
        print("  skip", s)


def ev_float(node, env):
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "FLOAT":
        return F(f32(float(ev_float(node.args[0], env))))
    if isinstance(node, ast.BinOp):
        a, b = ev_float(node.left, env), ev_float(node.right, env)
        if isinstance(a, F) or isinstance(b, F):
            a, b = float(a), float(b)
            r = {ast.Add: a + b, ast.Sub: a - b, ast.Mult: a * b, ast.Div: a / b if b else float("nan")}[type(node.op)]
            return F(f32(r))
        return ev(ast.BinOp(ast.Constant(a), node.op, ast.Constant(b)), env)
    return ev(node, env)


if __name__ == "__main__":
    main()

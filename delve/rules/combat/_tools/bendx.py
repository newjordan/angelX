#!/usr/bin/env python3
"""bendx: structured Bend for the combat port.

Rust function bodies are statement-shaped (early return, if/else, loops with
continue/break, rebinding). Bend 2 has none of that: a `match` takes only a
parameter, there is no `if`, and a pair is split only by a def that receives it.
bendx takes a def body written with those statements and emits plain Bend, one
def per control point (continuation passing), so the translation can follow the
Rust line by line. Expressions are Bend, passed through untouched.

    python3 delve/rules/combat/_tools/bendx.py [file.bendx ...]    (default: all beside it)

writes foo.bend next to foo.bendx.  Source forms (inside `def f(a: T, b: U) -> R:`):

    x: T = EXPR            new variable      x = EXPR       rebind (type kept)
    (a: T, b: U) = EXPR    split a pair      (a, b: U) = ... rebinds a
    if C: / elif C: / else:                  return EXPR
    for e: T in LIST: / for (a: T, b: U) in LIST:   continue / break
    while C:
    match (EXPR : T):  with  case Pat{x: T, y: U}:  blocks
    EXPR                   (the last statement) the result

Types: `(A, B, C)` a Data tuple, `[T]` List<&2, T>, `T?` Maybe<&2, T>.
Every parameter is reusable (`+`). Lines starting with `#` are comments.
"""
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent.parent
IDENT = re.compile(r"[A-Za-z_][A-Za-z_0-9]*(?:\.[A-Za-z_0-9?]+)*")
STRUCTS = {}   # name -> {field: (kind, skip)}
KIND_TYPE = {"f32": "F32", "u32": "U32", "bool": "Bool", "str": "String", "w64": "W.W", "json": "J.Val",
             "pair": "Sigma<&2, &2, F32, _ => F32>", "u32?": "Maybe<&2, U32>", "f32?": "Maybe<&2, F32>", "str?": "Maybe<&2, String>",
             "pair?": "Maybe<&2, Sigma<&2, &2, F32, _ => F32>>", "w64?": "Maybe<&2, W.W>"}
GET = {"f32": "cb.gf", "u32": "cb.gu", "bool": "cb.gb", "str": "cb.gs", "w64": "cb.gw", "json": "cb.gj", "pair": "cb.gp",
       "u32?": "cb.gou", "f32?": "cb.gof", "str?": "cb.gos", "pair?": "cb.gop", "w64?": "cb.gow"}
SET = {"f32": "cb.sf", "u32": "cb.su", "bool": "cb.sb", "str": "cb.ss", "w64": "cb.sw", "json": "cb.sj", "pair": "cb.sp2",
       "u32?": "cb.sou", "f32?": "cb.sof", "str?": "cb.sos", "pair?": "cb.sop", "w64?": "cb.sow"}
SKIPSET = {("u32", "skip0"): "cb.su0", ("bool", "skipf"): "cb.sb0", ("json", "skipnull"): "cb.sjn",
           ("json", "skipempty"): "cb.sje", ("u32?", "skipnone"): "cb.sou0", ("w64", "skip0"): "cb.sw0",
           ("pair", "skip0"): "cb.sp0", ("str?", "skipnone"): "cb.sos0", ("f32", "skip0"): "cb.sf0"}


def load_structs(text):
    cur = None
    for raw in text.split("\n"):
        line = raw.split("#")[0].rstrip()
        if not line.strip():
            continue
        if not raw.startswith(" "):
            m = re.match(r"^struct\s+(\w+)$", line.strip())
            if not m:
                raise Err("bad struct line %r" % raw)
            cur = STRUCTS.setdefault(m.group(1), {})
        else:
            parts = line.split()
            if len(parts) < 2:
                raise Err("bad field line %r" % raw)
            name, kind = parts[0], parts[1]
            if kind not in KIND_TYPE and kind not in STRUCTS:
                raise Err("field %s: unknown kind %s" % (name, kind))
            cur[name] = (kind, parts[2] if len(parts) > 2 else None)


class Err(Exception):
    pass


# ---------------------------------------------------------------- types

def split_top(s, sep=","):
    out, depth, cur, q = [], 0, [], False
    for ch in s:
        if ch == '"':
            q = not q
        if not q:
            if ch in "([{<":
                depth += 1
            elif ch in ")]}>":
                depth -= 1
            elif ch == sep and depth == 0:
                out.append("".join(cur)); cur = []
                continue
        cur.append(ch)
    out.append("".join(cur))
    return [p.strip() for p in out]


def ty(s):
    """Sugar -> Bend type text."""
    s = s.strip()
    if " & " in s:
        parts = split_top(s, "&")
        if len(parts) > 1:
            return " & ".join(ty(p) for p in parts)
    if s in STRUCTS:
        return "J.Val"
    if s.endswith("?") and not s.endswith("??"):
        return "Maybe<&2, %s>" % ty(s[:-1])
    if s.startswith("[") and s.endswith("]") and balanced(s[1:-1]):
        return "List<&2, %s>" % ty(s[1:-1])
    if s.startswith("(") and s.endswith(")") and balanced(s[1:-1]):
        parts = split_top(s[1:-1])
        if len(parts) == 1:
            return ty(parts[0])
        return sigma([ty(p) for p in parts])
    m = re.match(r"^([A-Za-z_][\w.]*)<(.*)>$", s)
    if m:
        return "%s<%s>" % (m.group(1), ", ".join(ty(p) if not p.startswith("&") else p for p in split_top(m.group(2))))
    return s


def balanced(s):
    d = 0
    for ch in s:
        if ch in "([{<":
            d += 1
        elif ch in ")]}>":
            d -= 1
            if d < 0:
                return False
    return d == 0


def sigma(ts):
    if len(ts) == 1:
        return ts[0]
    return "Sigma<&2, &2, %s, _ => %s>" % (ts[0], sigma(ts[1:]))


# ---------------------------------------------------------------- lines

class Line:
    def __init__(self, ind, text, no):
        self.ind, self.text, self.no = ind, text, no


def depth_of(text):
    d, q = 0, False
    for ch in text:
        if ch == '"':
            q = not q
        elif not q:
            if ch in "([{":
                d += 1
            elif ch in ")]}":
                d -= 1
    return d


def logical(src):
    out, cur, ind0, no0 = [], None, 0, 0
    for no, raw in enumerate(src.split("\n"), 1):
        if cur is not None:
            cur += " " + raw.strip()
            if depth_of(cur) <= 0:
                out.append(Line(ind0, cur, no0)); cur = None
            continue
        if not raw.strip():
            continue
        if raw.strip().startswith("#"):
            if not raw.startswith(" "):
                out.append(Line(0, raw.rstrip(), no))
            continue
        ind = len(raw) - len(raw.lstrip(" "))
        text = raw.strip()
        if depth_of(text) > 0:
            cur, ind0, no0 = text, ind, no
        else:
            out.append(Line(ind, text, no))
    if cur is not None:
        raise Err("unbalanced brackets at line %d" % no0)
    return out


# ---------------------------------------------------------------- parse

def find_eq(text):
    d, q = 0, False
    for i, ch in enumerate(text):
        if ch == '"':
            q = not q
        elif not q:
            if ch in "([{":
                d += 1
            elif ch in ")]}":
                d -= 1
            elif ch == "=" and d == 0:
                nxt = text[i + 1] if i + 1 < len(text) else ""
                prv = text[i - 1] if i else ""
                if nxt in ">=" or prv in "<>=!":
                    continue
                return i
    return -1


def parse_pat(s):
    s = s.strip()
    aff = s.startswith("&")
    if aff:
        s = s[1:].strip()
    if s.startswith("(") and s.endswith(")"):
        parts = split_top(s[1:-1])
        if len(parts) > 1:
            return ("t", [parse_pat(p) for p in parts], aff)
        return parse_pat(parts[0])
    if re.match(r"^[A-Za-z_]\w*(\.\w+)+$", s):
        return ("f", s)
    m = re.match(r"^([A-Za-z_]\w*)\s*(?::\s*(.+))?$", s)
    if not m:
        raise Err("bad pattern %r" % s)
    raw = m.group(2).strip() if m.group(2) else None
    return ("v", m.group(1), ty(raw) if raw else None, raw if raw in STRUCTS else None)


def parse_block(lines, i, ind):
    stmts = []
    while i < len(lines) and lines[i].ind >= ind:
        ln = lines[i]
        if ln.ind != ind:
            raise Err("line %d: bad indentation" % ln.no)
        t = ln.text
        if t.startswith("if ") and t.endswith(":"):
            branches, els = [], None
            cond = t[3:-1].strip()
            i += 1
            blk, i = parse_block(lines, i, child_ind(lines, i, ind, ln))
            branches.append((cond, blk))
            while i < len(lines) and lines[i].ind == ind and (lines[i].text.startswith("elif ") or lines[i].text == "else:"):
                l2 = lines[i]
                i += 1
                blk, i = parse_block(lines, i, child_ind(lines, i, ind, l2))
                if l2.text == "else:":
                    els = blk
                    break
                branches.append((l2.text[5:-1].strip(), blk))
            stmts.append(("if", branches, els, ln.no))
            continue
        if t.startswith("for ") and t.endswith(":"):
            m = re.match(r"^for\s+(.+?)\s+in\s+(.+):$", t)
            if not m:
                raise Err("line %d: bad for" % ln.no)
            i += 1
            blk, i = parse_block(lines, i, child_ind(lines, i, ind, ln))
            stmts.append(("for", parse_pat(m.group(1)), m.group(2), blk, ln.no))
            continue
        if t.startswith("while ") and t.endswith(":"):
            i += 1
            blk, i = parse_block(lines, i, child_ind(lines, i, ind, ln))
            stmts.append(("while", t[6:-1].strip(), blk, ln.no))
            continue
        if t.startswith("match ") and t.endswith(":"):
            m = re.match(r"^match\s+\((.+)\s:\s(.+)\):$", t)
            if not m:
                raise Err("line %d: match needs `match (EXPR : TYPE):`" % ln.no)
            i += 1
            cind = child_ind(lines, i, ind, ln)
            cases = []
            while i < len(lines) and lines[i].ind == cind and lines[i].text.startswith("case ") and lines[i].text.endswith(":"):
                cl = lines[i]
                i += 1
                blk, i = parse_block(lines, i, child_ind(lines, i, cind, cl))
                cases.append((cl.text[5:-1].strip(), blk))
            stmts.append(("match", m.group(1).strip(), ty(m.group(2)), cases, ln.no))
            continue
        i += 1
        if t.startswith("return "):
            stmts.append(("return", t[7:].strip(), ln.no))
        elif t == "continue":
            stmts.append(("continue", ln.no))
        elif t == "break":
            stmts.append(("break", ln.no))
        elif re.match(r"^[A-Za-z_]\w*(\.\w+)*\s*[-+*/]=\s*\S", t):
            m = re.match(r"^([A-Za-z_]\w*(?:\.\w+)*)\s*([-+*/])=\s*(.+)$", t)
            stmts.append(("aug", m.group(1), m.group(2), m.group(3), ln.no))
        elif re.match(r"^[A-Za-z_]\w*(\.\w+)+\s*=[^=>]", t):
            e = find_eq(t)
            stmts.append(("set", t[:e].strip(), t[e + 1:].strip(), ln.no))
        else:
            e = find_eq(t)
            if e > 0 and re.match(r"^(&?\(|[A-Za-z_]\w*\s*(:|=))", t):
                lhs, rhs = t[:e].strip(), t[e + 1:].strip()
                stmts.append(("let", parse_pat(lhs), rhs, ln.no))
            else:
                stmts.append(("expr", t, ln.no))
    return stmts, i


def child_ind(lines, i, ind, ln):
    if i >= len(lines) or lines[i].ind <= ind:
        raise Err("line %d: empty block" % ln.no)
    return lines[i].ind


# ---------------------------------------------------------------- analysis

def idents(text):
    text = re.sub(r'"(?:[^"\\]|\\.)*"', '""', text)
    return {m.split(".")[0] for m in IDENT.findall(text) if not m[0].isupper() or "." not in m}


def pat_names(p):
    if p[0] == "v":
        return [p[1]]
    if p[0] == "f":
        return [p[1].split(".")[0]]
    return [n for q in p[1] for n in pat_names(q)]


def uses(stmts, lc=None, nested=False):
    u = set()
    for s in stmts:
        k = s[0]
        if k == "let":
            u |= idents(s[2])
            for q in pat_flat(s[1]):
                if q[0] == "f":
                    u |= idents(q[1])
        elif k == "set":
            u |= idents(s[1]) | idents(s[2])
        elif k == "aug":
            u |= idents(s[1]) | idents(s[3])
        elif k in ("return", "expr"):
            u |= idents(s[1])
        elif k == "continue":
            if lc and not nested:
                u |= set(lc["cont"].params)
        elif k == "break":
            if lc and not nested:
                u |= set(lc["brk"].params)
        elif k == "if":
            for c, b in s[1]:
                u |= idents(c) | uses(b, lc, nested)
            if s[2]:
                u |= uses(s[2], lc, nested)
        elif k == "for":
            u |= idents(s[2]) | uses(s[3], lc, True)
        elif k == "while":
            u |= idents(s[1]) | uses(s[2], lc, True)
        elif k == "match":
            u |= idents(s[1])
            for _, b in s[3]:
                u |= uses(b, lc, nested)
    return u


def pat_flat(p):
    if p[0] == "t":
        return [x for q in p[1] for x in pat_flat(q)]
    return [p]


def assigned(stmts):
    a = set()
    for s in stmts:
        k = s[0]
        if k in ("set", "aug"):
            a.add(s[1].split(".")[0])
        elif k == "let":
            a |= set(pat_names(s[1]))
        elif k == "if":
            for _, b in s[1]:
                a |= assigned(b)
            if s[2]:
                a |= assigned(s[2])
        elif k == "for":
            a |= assigned(s[3])
        elif k == "while":
            a |= assigned(s[2])
        elif k == "match":
            for _, b in s[3]:
                a |= assigned(b)
    return a


# ---------------------------------------------------------------- emit

class Cont:
    def __init__(self, name, params):
        self.name, self.params = name, params

    def call(self):
        return "%s(%s)" % (self.name, ", ".join(self.params))


class Ctx:
    def __init__(self, base, ret):
        self.base, self.ret, self.n, self.defs = base, ret, 0, []
        self.tags = {}

    def fresh(self, tag):
        self.n += 1
        base = "cb." + self.base if self.base.startswith("case.") else self.base
        return "%s.%s%d" % (base, tag, self.n)


def ordered(names, env):
    return [v for v in env if v in names]


def params_text(env, names, first=None):
    ps = [first] if first else []
    for v in names:
        t = env[v]
        if t is None:
            raise Err("type of %s unknown where it crosses a control point" % v)
        ps.append("%s%s: %s" % ("" if " & " in t else "+", v, t))
    return ", ".join(ps)


def needed(stmts, env, k, lc):
    u = uses(stmts, lc)
    if k:
        u |= set(k.params)
    return ordered(u, env)


def add_def(ctx, name, params, body, ret):
    ctx.defs.append("def %s(%s) -> %s:\n%s" % (name, params, ret, "\n".join(body)))


def make_cont(ctx, rest, env, k, lc, tag="k"):
    if not rest:
        return k
    live = needed(rest, env, k, lc)
    name = ctx.fresh(tag)
    body = block(ctx, rest, env, k, lc, "  ")
    add_def(ctx, name, params_text(env, live), body, ctx.ret)
    return Cont(name, live)


def pat_text(p):
    if p[0] == "v":
        return "+" + p[1]
    parts = [pat_text(q) for q in p[1]]
    out = parts[-1]
    for q in reversed(parts[:-1]):
        out = "(%s, %s)" % (q, out)
    return out


def pat_type(p, env):
    if p[0] == "v":
        t = p[2] or env.get(p[1])
        if t is None:
            raise Err("pair component %s has no type" % p[1])
        return t
    if p[2]:
        return " & ".join(pat_type(q, env) for q in p[1])
    return sigma([pat_type(q, env) for q in p[1]])


def bind_types(p, env):
    if p[0] == "v":
        env[p[1]] = p[2] or env.get(p[1])
    else:
        for q in p[1]:
            bind_types(q, env)


def case_pattern(text, env, tags=None):
    """`Some{x: F32}` -> (`Some{+x}`, {x: F32})."""
    bound = {}
    tags = tags if tags is not None else {}

    def conv(s):
        s = s.strip()
        m = re.match(r"^([A-Za-z_][\w.]*)\{(.*)\}$", s, re.S)
        if m:
            inner = m.group(2).strip()
            if not inner:
                return "%s{}" % m.group(1)
            return "%s{%s}" % (m.group(1), ", ".join(conv(p) for p in split_top(inner)))
        if s == "_":
            return "_"
        m = re.match(r"^([A-Za-z_]\w*)\s*(?::\s*(.+))?$", s)
        if m:
            bound[m.group(1)] = ty(m.group(2)) if m.group(2) else None
            if m.group(2) and m.group(2).strip() in STRUCTS:
                tags[m.group(1)] = m.group(2).strip()
            return "+" + m.group(1)
        if re.match(r"^\(.*\)$", s):
            return "(%s)" % ", ".join(conv(p) for p in split_top(s[1:-1]))
        return s

    return conv(text), bound



FIELD = re.compile(r"(?<![\w.])([A-Za-z_]\w*)((?:\.[A-Za-z_]\w*)+)")


def chain(ctx, var, fields):
    """(getter expr text, kind or tag) for var.f1.f2..."""
    if var not in ctx.tags:
        raise Err("%s is not a struct variable" % var)
    tag, expr = ctx.tags[var], var
    kind = None
    for f in fields:
        if tag is None:
            raise Err("%s.%s: %s is not a struct" % (var, ".".join(fields), f))
        if f not in STRUCTS[tag]:
            raise Err("struct %s has no field %s" % (tag, f))
        kind, _ = STRUCTS[tag][f]
        if kind in STRUCTS:
            expr = 'cb.gj(%s, "%s")' % (expr, f)
            tag = kind
        else:
            expr = '%s(%s, "%s")' % (GET[kind], expr, f)
            tag = None
    return expr, kind


def rw(ctx, expr):
    """Struct field reads `var.field` -> getter calls (outside string literals)."""
    parts = re.split(r'("(?:[^"\\]|\\.)*")', expr)
    out = []
    for i, part in enumerate(parts):
        if i % 2:
            out.append(part)
            continue

        def sub(m):
            var = m.group(1)
            if var not in ctx.tags:
                return m.group(0)
            return chain(ctx, var, m.group(2)[1:].split("."))[0]
        out.append(FIELD.sub(sub, part))
    return "".join(out)


def setter_text(ctx, var, fields, value):
    """The new value of `var` with var.f1.f2 = value (value already rewritten)."""
    if var not in ctx.tags:
        raise Err("%s is not a struct variable" % var)

    def go(cur_expr, tag, fs):
        f = fs[0]
        if f not in STRUCTS[tag]:
            raise Err("struct %s has no field %s" % (tag, f))
        kind, skip = STRUCTS[tag][f]
        if len(fs) > 1:
            if kind not in STRUCTS:
                raise Err("%s is not a struct" % f)
            inner = go('cb.gj(%s, "%s")' % (cur_expr, f), kind, fs[1:])
            return 'cb.sj(%s, "%s", %s)' % (cur_expr, f, inner)
        if skip and skip.startswith("skipfn="):
            return '%s.set(%s, "%s", %s)' % (skip[7:], cur_expr, f, value)
        if skip and skip.startswith("skip=") and kind == "str":
            return 'cb.sskip(%s, "%s", %s, "%s")' % (cur_expr, f, value, skip[5:])
        fn = SKIPSET.get((kind, skip)) if skip else None
        if skip and fn is None:
            raise Err("no skip setter for %s %s" % (kind, skip))
        if kind in STRUCTS:
            fn = "cb.sj"
        fn = fn or SET[kind]
        return '%s(%s, "%s", %s)' % (fn, cur_expr, f, value)
    return go(var, ctx.tags[var], fields)


def field_type(ctx, var, fields):
    tag, kind = ctx.tags[var], None
    for f in fields:
        kind = STRUCTS[tag][f][0]
        tag = kind if kind in STRUCTS else None
    return "J.Val" if kind in STRUCTS else KIND_TYPE[kind]


def tag_of(raw):
    return raw if raw in STRUCTS else None


def block(ctx, stmts, env, k, lc, ind):
    lines = []
    env = dict(env)
    stmts = list(stmts)
    i = -1
    while i + 1 < len(stmts):
        i += 1
        s = stmts[i]
        kind = s[0]
        rest = stmts[i + 1:]
        if kind in ("return", "expr"):
            if kind == "expr" and rest:
                raise Err("line %d: bare expression must be last (use return)" % s[-1])
            lines.append(ind + rw(ctx, s[1]))
            return lines
        if kind in ("set", "aug"):
            tgt = s[1]
            m = re.match(r"^([A-Za-z_]\w*)((?:\.\w+)*)$", tgt)
            var, fields = m.group(1), [f for f in m.group(2).split(".") if f]
            if kind == "set":
                val = rw(ctx, s[2])
            else:
                op = s[2]
                if fields:
                    cur, kd = chain(ctx, var, fields)
                    T = field_type(ctx, var, fields)
                else:
                    cur, T = var, env.get(var)
                    if T is None:
                        raise Err("line %d: %s has no type" % (s[-1], var))
                val = "(%s %s %s : %s)" % (cur, op, rw(ctx, s[3]), T)
            if fields:
                lines.append("%s+%s = {%s : J.Val}" % (ind, var, setter_text(ctx, var, fields, val)))
            else:
                lines.append("%s+%s = {%s : %s}" % (ind, var, val, env[var]))
            continue
        if kind == "continue":
            if not lc:
                raise Err("line %d: continue outside a loop" % s[1])
            lines.append(ind + lc["cont"].call())
            return lines
        if kind == "break":
            if not lc:
                raise Err("line %d: break outside a loop" % s[1])
            if lc["brk"] is None:
                raise Err("line %d: loop has nothing after it to break to" % s[1])
            lines.append(ind + lc["brk"].call())
            return lines
        if kind == "let":
            pat, expr = s[1], rw(ctx, s[2])
            if pat[0] == "t" and any(q[0] == "f" for q in pat[1]):
                comps, sets, nth = [], [], 0
                for q in pat[1]:
                    if q[0] != "f":
                        comps.append(q)
                        continue
                    m = re.match(r"^([A-Za-z_]\w*)((?:\.\w+)+)$", q[1])
                    var, fields = m.group(1), m.group(2)[1:].split(".")
                    nth += 1
                    tmp = "_f%d_%d" % (ctx.n, nth)
                    comps.append(("v", tmp, field_type(ctx, var, fields), None))
                    sets.append(("set", q[1], tmp, s[3]))
                pat = ("t", comps, pat[2])
                stmts[i + 1:i + 1] = sets
                rest = stmts[i + 1:]
            if pat[0] == "v":
                t = pat[2] or env.get(pat[1])
                if t is None:
                    raise Err("line %d: %s needs a type" % (s[3], pat[1]))
                if pat[3]:
                    ctx.tags[pat[1]] = pat[3]
                lines.append("%s+%s = {%s : %s}" % (ind, pat[1], expr, t))
                env[pat[1]] = t
                continue
            for q in pat[1]:
                if q[0] == "v" and q[3]:
                    ctx.tags[q[1]] = q[3]
            tT = pat_type(pat, env)
            env2 = dict(env)
            bind_types(pat, env2)
            names = set(pat_names(pat))
            live = [v for v in needed(rest, env2, k, lc) if v in env and v not in names]
            name = ctx.fresh("k")
            if pat[2]:
                comps = pat[1]
                if any(q[0] != "v" for q in comps):
                    raise Err("line %d: an & pair splits into plain names" % s[3])
                lines.append("%s%s(%s)" % (ind, name, ", ".join([expr] + live)))
                cur_name, cur_type, bound = name, tT, []
                for idx, q in enumerate(comps):
                    last = idx == len(comps) - 2
                    qt = pat_type(q, env)
                    restt = " & ".join(pat_type(r, env) for r in comps[idx + 1:])
                    extra = "".join(", +%s: %s" % (b, env2[b]) for b in bound)
                    hdr = "+_t: %s" % cur_type
                    others = params_text(env, live)
                    full = hdr + (", " + others if others else "") + extra
                    if last:
                        body = ["  (+%s, +%s) = _t" % (q[1], comps[idx + 1][1])]
                        body += block(ctx, rest, env2, k, lc, "  ") if rest else tail(k, s)
                        add_def(ctx, cur_name, full, body, ctx.ret)
                        break
                    nxt = ctx.fresh("k")
                    body = ["  (+%s, _t2) = _t" % q[1], "  %s(%s)" % (nxt, ", ".join(["_t2"] + live + bound + [q[1]]))]
                    add_def(ctx, cur_name, full, body, ctx.ret)
                    bound.append(q[1])
                    cur_name, cur_type = nxt, restt
                return lines
            body = ["  %s = _t" % pat_text(pat)] + (block(ctx, rest, env2, k, lc, "  ") if rest else tail(k, s))
            add_def(ctx, name, params_text(env, live, "+_t: " + tT), body, ctx.ret)
            lines.append("%s%s(%s)" % (ind, name, ", ".join([expr] + live)))
            return lines
        if kind == "if":
            kn = make_cont(ctx, rest, env, k, lc)
            branches, els = s[1], s[2]
            lines += lift_if(ctx, branches, els, env, kn, lc, ind)
            return lines
        if kind == "match":
            kn = make_cont(ctx, rest, env, k, lc)
            expr, T, cases = s[1], s[2], s[3]
            conv = []
            usedset = set()
            for ptext, blk in cases:
                ptxt, bound = case_pattern(ptext, env, ctx.tags)
                e2 = dict(env)
                e2.update(bound)
                conv.append((ptxt, bound, blk))
                usedset |= uses(blk, lc)
            if kn:
                usedset |= set(kn.params)
            live = ordered(usedset, env)
            name = ctx.fresh("m")
            body = ["  match _s:"]
            for ptxt, bound, blk in conv:
                body.append("    case %s:" % ptxt)
                e2 = dict(env)
                e2.update(bound)
                body += block(ctx, blk, e2, kn, lc, "      ") if blk else tail(kn, s, "      ")
            add_def(ctx, name, params_text(env, live, "+_s: " + T), body, ctx.ret)
            lines.append("%s%s(%s)" % (ind, name, ", ".join([rw(ctx, expr)] + live)))
            return lines
        if kind in ("for", "while"):
            kn = make_cont(ctx, rest, env, k, lc)
            lines += lift_loop(ctx, s, env, kn, ind)
            return lines
        raise Err("unknown statement %r" % (kind,))
    if k is None:
        raise Err("function %s falls off its end without a result" % ctx.base)
    lines.append(ind + k.call())
    return lines


def tail(k, s, ind="  "):
    if k is None:
        raise Err("line %d: falls off the end of the function" % s[-1])
    return [ind + k.call()]


def lift_if(ctx, branches, els, env, kn, lc, ind):
    cond, blk = branches[0]
    if len(branches) > 1:
        else_stmts = [("if", branches[1:], els, 0)]
    else:
        else_stmts = els or []
    usedset = uses(blk, lc) | uses(else_stmts, lc)
    if kn:
        usedset |= set(kn.params)
    live = ordered(usedset, env)
    name = ctx.fresh("if")
    body = ["  match _c:", "    case True{}:"]
    body += block(ctx, blk, env, kn, lc, "      ") if blk else tail(kn, ("x", 0), "      ")
    body.append("    case False{}:")
    body += block(ctx, else_stmts, env, kn, lc, "      ") if else_stmts else tail(kn, ("x", 0), "      ")
    add_def(ctx, name, params_text(env, live, "_c: Bool"), body, ctx.ret)
    return ["%s%s(%s)" % (ind, name, ", ".join([rw(ctx, cond)] + live))]


def lift_loop(ctx, s, env, kn, ind):
    if s[0] == "for":
        _, pat, lst, blk, no = s
        elem_t = pat_type(pat, env)
        pnames = set(pat_names(pat))
    else:
        _, cond, blk, no = s
        pat, lst, elem_t, pnames = None, None, None, set()
    carried = assigned(blk) & set(env) - pnames
    p0 = uses(blk, None, True) | carried | (set(kn.params) if kn else set())
    # a nested loop's continue/break stay inside it; ordinary ones count here
    if s[0] == "while":
        p0 |= idents(cond)
    P = ordered(p0 - pnames, env)
    name = ctx.fresh("loop")
    tailv = "_t%d" % ctx.n
    env2 = dict(env)
    if s[0] == "for":
        cont = Cont(name, [tailv] + P)
        env2[tailv] = "List<&2, %s>" % elem_t
    else:
        cont = Cont(name, P)
    lc2 = {"cont": cont, "brk": kn}
    header = params_text(env, P, ("+_xs: List<&2, %s>" % elem_t) if s[0] == "for" else None)
    if s[0] == "for":
        bind_types(pat, env2)
        for q in pat_flat(pat):
            if q[0] == "v" and q[3]:
                ctx.tags[q[1]] = q[3]
        headv = "_h%d" % ctx.n
        body = ["  match _xs:", "    case Nil{}:"]
        body += tail(kn, s, "      ")
        if pat[0] == "v":
            body.append("    case Con{+%s, +%s}:" % (pat[1], tailv))
            body += block(ctx, blk, env2, cont, lc2, "      ")
        else:
            body.append("    case Con{+%s, +%s}:" % (headv, tailv))
            body.append("      %s = %s" % (pat_text(pat), headv))
            body += block(ctx, blk, env2, cont, lc2, "      ")
        add_def(ctx, name, header, body, ctx.ret)
        return ["%s%s(%s)" % (ind, name, ", ".join([rw(ctx, lst)] + P))]
    # while: loop header is the lifted if on the condition
    inner = [("if", [(cond, blk + [("continue", no)])], [("break", no)], no)]
    body = block(ctx, inner, env2, None, lc2, "  ")
    add_def(ctx, name, header, body, ctx.ret)
    return ["%s%s(%s)" % (ind, name, ", ".join(P))]


# ---------------------------------------------------------------- top level

def parse_params(s):
    out = []
    for p in split_top(s):
        if not p:
            continue
        m = re.match(r"^\+?([A-Za-z_]\w*)\s*:\s*(.+)$", p)
        if not m:
            raise Err("bad parameter %r" % p)
        out.append((m.group(1), ty(m.group(2)), tag_of(m.group(2).strip())))
    return out


def compile_def(head, body_lines, no):
    m = re.match(r"^def\s+([^\s(]+)\s*\((.*)\)\s*->\s*(.+):$", head, re.S)
    if not m:
        raise Err("line %d: bad def header" % no)
    name, params, ret = m.group(1), parse_params(m.group(2)), ty(m.group(3))
    stmts, _ = parse_block(body_lines, 0, body_lines[0].ind)
    ctx = Ctx(name, ret)
    env = {p: t for p, t, _g in params}
    for p, _t, g in params:
        if g:
            ctx.tags[p] = g
    lines = block(ctx, stmts, env, None, None, "  ")
    plus = lambda t: "" if " & " in t else "+"
    main = "def %s(%s) -> %s:\n%s" % (name, ", ".join("%s%s: %s" % (plus(t), p, t) for p, t, _g in params), ret, "\n".join(lines))
    return "\n\n".join([main] + ctx.defs)


def compile_text(src, origin):
    lines = logical(src)
    out, i = [], 0
    while i < len(lines):
        ln = lines[i]
        j = i + 1
        while j < len(lines) and lines[j].ind > ln.ind:
            j += 1
        if ln.text.startswith("def ") and ln.text.endswith(":") and ln.ind == 0:
            try:
                out.append(compile_def(ln.text, lines[i + 1:j], ln.no))
            except Err as e:
                raise Err("%s: %s: %s" % (origin, ln.text.split("(")[0], e))
        else:
            blk = "\n".join(" " * l.ind + l.text for l in lines[i:j])
            if out and out[-1].startswith("#") and blk.startswith("#") and not out[-1].endswith("\n\n"):
                out[-1] += "\n" + blk
            else:
                out.append(blk)
        i = j
    return "\n\n".join(out) + "\n"


def keep_comments(src):
    """Top-level comment blocks survive in front of the def that follows."""
    return src


def main():
    sf = HERE / "structs.bx"
    if sf.exists():
        load_structs(sf.read_text())
    files = [pathlib.Path(a) for a in sys.argv[1:]] or sorted(HERE.glob("*.bendx"))
    for f in files:
        text = f.read_text()
        try:
            out = compile_text(text, f.name)
        except Err as e:
            print("bendx: %s" % e, file=sys.stderr)
            sys.exit(1)
        head = "# GENERATED by _tools/bendx.py from %s. Edit the .bendx, not this file.\n\n" % f.name
        dest = f.with_suffix(".bend")
        m = re.match(r"^# bendx-out: (\S+)", text)
        if m:
            dest = (f.parent / m.group(1)).resolve()
        dest.write_text(head + out)
        print("%s -> %s" % (f.name, dest))


if __name__ == "__main__":
    main()

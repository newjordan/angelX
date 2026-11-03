#!/usr/bin/env python3
"""bendx (the core group's copy): structured Bend. `(A, B)` is the affine pair `A & B` (what PORTING.md says a
function hands back); inside `[..]`/`..?` it is the reusable Sigma. Split a tuple two at a time.

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


def ty(s, data=False):
    """Sugar -> Bend type text. `(A, B)` is the affine pair `A & B` (a function
    result), or the reusable Sigma inside `[..]` / `..?` (a thing in a list)."""
    s = s.strip()
    if s.endswith("?") and not s.endswith("??"):
        return "Maybe<&2, %s>" % ty(s[:-1], True)
    if s.startswith("[") and s.endswith("]") and balanced(s[1:-1]):
        return "List<&2, %s>" % ty(s[1:-1], True)
    if s.startswith("(") and s.endswith(")") and balanced(s[1:-1]):
        parts = split_top(s[1:-1])
        if len(parts) == 1:
            return ty(parts[0], data)
        return sigma([ty(p, data) for p in parts], data)
    m = re.match(r"^([A-Za-z_][\w.]*)<(.*)>$", s)
    if m:
        return "%s<%s>" % (m.group(1), ", ".join(ty(p, True) if not p.startswith("&") else p for p in split_top(m.group(2))))
    return s


def is_data(t):
    """Whether values of the type may be reused (a `+` parameter): not an affine pair."""
    d = 0
    for i, ch in enumerate(t):
        if ch in "(<[{":
            d += 1
        elif ch in ")>]}":
            d -= 1
        elif ch == "&" and d == 0 and t[i - 1:i] == " " and t[i + 1:i + 2] == " ":
            return False
    return True


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


def sigma(ts, data=False):
    if len(ts) == 1:
        return ts[0]
    rest = sigma(ts[1:], data)
    if data:
        return "Sigma<&2, &2, %s, _ => %s>" % (ts[0], rest)
    if not is_data(rest):
        rest = "(%s)" % rest
    return "%s & %s" % (ts[0], rest)


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
    if s.startswith("(") and s.endswith(")"):
        parts = split_top(s[1:-1])
        if len(parts) > 1:
            return ("t", [parse_pat(p) for p in parts])
        return parse_pat(parts[0])
    m = re.match(r"^([A-Za-z_]\w*)\s*(?::\s*(.+))?$", s)
    if not m:
        raise Err("bad pattern %r" % s)
    return ("v", m.group(1), ty(m.group(2)) if m.group(2) else None)


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
        else:
            e = find_eq(t)
            if e > 0 and re.match(r"^(\(|[A-Za-z_]\w*\s*(:|=))", t):
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
    return {m for m in IDENT.findall(text) if "." not in m}


def pat_names(p):
    if p[0] == "v":
        return [p[1]]
    return [n for q in p[1] for n in pat_names(q)]


def uses(stmts, lc=None, nested=False):
    u = set()
    for s in stmts:
        k = s[0]
        if k == "let":
            u |= idents(s[2])
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


def assigned(stmts):
    a = set()
    for s in stmts:
        k = s[0]
        if k == "let":
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

    def fresh(self, tag):
        self.n += 1
        return "%s.%s%d" % (self.base, tag, self.n)


def ordered(names, env):
    return [v for v in env if v in names]


def params_text(env, names, first=None):
    ps = [first] if first else []
    for v in names:
        t = env[v]
        if t is None:
            raise Err("type of %s unknown where it crosses a control point" % v)
        ps.append(("+%s: %s" if is_data(t) else "%s: %s") % (v, t))
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


def pat_text(p, env=None):
    if p[0] == "v":
        t = (p[2] or (env or {}).get(p[1])) or ""
        return ("+" if is_data(t) else "") + p[1]
    if len(p[1]) != 2:
        raise Err("split a tuple two at a time: (a: A, rest: B & C) = ..., then (b: B, c: C) = rest")
    return "(%s, %s)" % (pat_text(p[1][0], env), pat_text(p[1][1], env))


def pat_text_data(p):
    if p[0] == "v":
        return "+" + p[1]
    parts = [pat_text_data(q) for q in p[1]]
    out = parts[-1]
    for q in reversed(parts[:-1]):
        out = "(%s, %s)" % (q, out)
    return out


def pat_type(p, env, data=False):
    if p[0] == "v":
        t = p[2] or env.get(p[1])
        if t is None:
            raise Err("pair component %s has no type" % p[1])
        return t
    return sigma([pat_type(q, env, data) for q in p[1]], data)


def bind_types(p, env):
    if p[0] == "v":
        env[p[1]] = p[2] or env.get(p[1])
    else:
        for q in p[1]:
            bind_types(q, env)


def case_pattern(text, env):
    """`Some{x: F32}` -> (`Some{+x}`, {x: F32})."""
    bound = {}

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
            bound[m.group(1)] = ty(m.group(2), True) if m.group(2) else None
            return "+" + m.group(1)
        if re.match(r"^\(.*\)$", s):
            return "(%s)" % ", ".join(conv(p) for p in split_top(s[1:-1]))
        return s

    return conv(text), bound


def block(ctx, stmts, env, k, lc, ind):
    lines = []
    env = dict(env)
    for i, s in enumerate(stmts):
        kind = s[0]
        rest = stmts[i + 1:]
        if kind in ("return", "expr"):
            if kind == "expr" and rest:
                raise Err("line %d: bare expression must be last (use return)" % s[-1])
            lines.append(ind + s[1])
            return lines
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
            pat, expr = s[1], s[2]
            if pat[0] == "v":
                t = pat[2] or env.get(pat[1])
                if t is None:
                    raise Err("line %d: %s needs a type" % (s[3], pat[1]))
                lines.append("%s+%s = {%s : %s}" % (ind, pat[1], expr, t))
                env[pat[1]] = t
                continue
            tT = pat_type(pat, env)
            if re.fullmatch(r'[A-Za-z_]\w*', expr.strip()) and (env.get(expr.strip()) or '').startswith('Sigma<&2'):
                tT = env[expr.strip()]
            env2 = dict(env)
            bind_types(pat, env2)
            names = set(pat_names(pat))
            live = [v for v in needed(rest, env2, k, lc) if v in env and v not in names]
            name = ctx.fresh("k")
            body = ["  %s = t" % pat_text(pat, env2)] + (block(ctx, rest, env2, k, lc, "  ") if rest else tail(k, s))
            add_def(ctx, name, params_text(env, live, ("+t: " if is_data(tT) else "t: ") + tT), body, ctx.ret)
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
                ptxt, bound = case_pattern(ptext, env)
                e2 = dict(env)
                e2.update(bound)
                conv.append((ptxt, bound, blk))
                usedset |= uses(blk, lc)
            if kn:
                usedset |= set(kn.params)
            live = ordered(usedset, env)
            name = ctx.fresh("m")
            body = ["  match s:"]
            for ptxt, bound, blk in conv:
                body.append("    case %s:" % ptxt)
                e2 = dict(env)
                e2.update(bound)
                body += block(ctx, blk, e2, kn, lc, "      ") if blk else tail(kn, s, "      ")
            add_def(ctx, name, params_text(env, live, ("+s: " if is_data(T) else "s: ") + T), body, ctx.ret)
            lines.append("%s%s(%s)" % (ind, name, ", ".join([expr] + live)))
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
    return ["%s%s(%s)" % (ind, name, ", ".join([cond] + live))]


def lift_loop(ctx, s, env, kn, ind):
    if s[0] == "for":
        _, pat, lst, blk, no = s
        elem_t = pat_type(pat, env, True)
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
        headv = "_h%d" % ctx.n
        body = ["  match _xs:", "    case Nil{}:"]
        body += tail(kn, s, "      ")
        if pat[0] == "v":
            body.append("    case Con{+%s, +%s}:" % (pat[1], tailv))
            body += block(ctx, blk, env2, cont, lc2, "      ")
        else:
            body.append("    case Con{+%s, +%s}:" % (headv, tailv))
            body.append("      %s = %s" % (pat_text_data(pat), headv))
            body += block(ctx, blk, env2, cont, lc2, "      ")
        add_def(ctx, name, header, body, ctx.ret)
        return ["%s%s(%s)" % (ind, name, ", ".join([lst] + P))]
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
        out.append((m.group(1), ty(m.group(2))))
    return out


def compile_def(head, body_lines, no):
    m = re.match(r"^def\s+([^\s(]+)\s*\((.*)\)\s*->\s*(.+):$", head, re.S)
    if not m:
        raise Err("line %d: bad def header" % no)
    name, params, ret = m.group(1), parse_params(m.group(2)), ty(m.group(3))
    stmts, _ = parse_block(body_lines, 0, body_lines[0].ind)
    ctx = Ctx(name, ret)
    env = {p: t for p, t in params}
    lines = block(ctx, stmts, env, None, None, "  ")
    main = "def %s(%s) -> %s:\n%s" % (name, ", ".join(("+%s: %s" if is_data(t) else "%s: %s") % (p, t) for p, t in params), ret, "\n".join(lines))
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
            out.append("\n".join(" " * l.ind + l.text for l in lines[i:j]))
        i = j
    return "\n\n".join(out) + "\n"


def keep_comments(src):
    """Top-level comment blocks survive in front of the def that follows."""
    return src


def main():
    files = [pathlib.Path(a) for a in sys.argv[1:]] or sorted(HERE.glob("*.bendx"))
    for f in files:
        text = f.read_text()
        try:
            out = compile_text(text, f.name)
        except Err as e:
            print("bendx: %s" % e, file=sys.stderr)
            sys.exit(1)
        head = "# GENERATED by _tools/bendx.py from %s. Edit the .bendx, not this file.\n\n" % f.name
        f.with_suffix(".bend").write_text(head + out)
        print("%s -> %s" % (f.name, f.with_suffix(".bend").name))


if __name__ == "__main__":
    main()

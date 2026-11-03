#!/usr/bin/env python3
"""bpx: a small typed layer over Bend for the bannerpeak port. `src/X.bpx` -> `X.bend`.

Bend 2 refuses a `match` on a computed value, wants every reused value marked `+`, and has
no signed integers, so a faithful translation of an imperative Rust module is mostly
plumbing. bpx writes the plumbing. The source is Rust-shaped, indented like Python:

    module bannerpeak.body                  every `fn` below is named bannerpeak.body.<name>
    use layout.WIDTH layout.HEIGHT          consts of other modules, by short name
    const Q: i32 = 256                      a const (its value is inlined; a def is written too)
    enum Move: Ground Air Wall              a unit enum: a String in the J tree
    struct Body:                            a J object in serde shape: its fields' types
      x: i32
      plank: Option<u16>
    extern fn Body.valid(self: Body) -> bool = bannerpeak.body.Body.valid
    fn Body.step(self: Body mut, c: Cmd, out: Vec<Ev> mut) -> bool:
      let (fx, fy) = self.foot()
      self.vx += 3
      if self.vy < 0 && !c.jump:
        return false
      out.push(Ev{k: "Skid", s: "", a: 0, b: 0})
      true

Types: i32 i16 i8 (signed, a two's-complement U32), u32 u16 u8 usize (U32), bool, str,
J (a J.Val), unit, structs (J objects), Option<T>, Vec<T> (a J array), Map<T> (a J object
keyed by decimal id), (A, B) tuples, list<T> (a Bend List of u32). A `mut` parameter is
`&mut` in Rust: the def returns (result, mut1, mut2, ...) and callers rebind. Control flow:
`if / elif / else`, `return`, calls, assignments, `let`. There are no loops: recurse. An
`if` becomes its own def over the live variables (Bend cannot match a computed value).

Run:  python3 bpx.py [src/X.bpx ...]     (default: every src/*.bpx, writing X.bend beside)
"""
import pathlib
import re
import sys

MASK = 0xFFFFFFFF


class Err(Exception):
    pass


# ---------------------------------------------------------------- types

SIGNED = {"i32", "i16", "i8", "isize"}
UNSIGNED = {"u32", "u16", "u8", "usize"}


def is_int(t):
    return t in ("i", "u")


class Reg:
    """Everything the sources declare."""

    def __init__(self):
        self.structs = {}  # name -> {field: type}
        self.enums = {}  # name -> [variants]
        self.consts = {}  # full name -> (type, value)
        self.short_consts = {}  # short name -> [full]
        self.fns = {}  # full bend name -> Sig
        self.methods = {}  # (Type, method) -> full bend name

    def type_of(self, s):
        s = s.strip()
        if s in SIGNED:
            return "i"
        if s in UNSIGNED:
            return "u"
        if s in ("bool", "str", "J", "unit"):
            return s
        m = re.fullmatch(r"(Option|Vec|Map|list)<(.*)>", s)
        if m:
            return (m.group(1).lower(), self.type_of(m.group(2)))
        if s.startswith("(") and s.endswith(")"):
            return ("tup", [self.type_of(p) for p in split_top(s[1:-1])])
        if s in self.enums:
            return "str"
        if s in self.structs or re.fullmatch(r"[A-Z]\w*", s):
            return ("struct", s)
        raise Err(f"unknown type {s!r}")


def bt(t):
    """The Bend type text."""
    if t in ("i", "u"):
        return "U32"
    if t == "bool":
        return "Bool"
    if t == "str":
        return "String"
    if t == "unit":
        return "Unit"
    if t == "J":
        return "J.Val"
    if isinstance(t, tuple):
        if t[0] in ("struct", "option", "vec", "map"):
            return "J.Val"
        if t[0] == "list":
            return f"List<&2, {bt(t[1])}>"
        if t[0] == "tup":
            return nest_type([bt(x) for x in t[1]])
    raise Err(f"no bend type for {t!r}")


def nest_type(parts):
    if len(parts) == 1:
        return parts[0]
    return f"Sigma<&2, &2, {parts[0]}, _ => {nest_type(parts[1:])}>"


def tshow(t):
    return str(t)


# ---------------------------------------------------------------- lexing

TOKEN = re.compile(r"""
    \s+ |
    (?P<num>0x[0-9a-fA-F_]+|\d[\d_]*) |
    (?P<chr>'(?:\\.|[^'\\])') |
    (?P<str>"(?:[^"\\]|\\.)*") |
    (?P<id>[A-Za-z_]\w*) |
    (?P<op>\.\.=|\.\.|<<=|>>=|::|->|=>|==|!=|<=|>=|&&|\|\||<<|>>|\+=|-=|\*=|/=|%=|\|=|&=|\^=|[-+*/%<>=!&|^()\[\]{},.:;?])
""", re.X)


def lex(s):
    out, i = [], 0
    while i < len(s):
        m = TOKEN.match(s, i)
        if not m:
            raise Err(f"bad character {s[i]!r} in {s!r}")
        i = m.end()
        if m.lastgroup is None:
            continue
        out.append((m.lastgroup, m.group(m.lastgroup)))
    return out


def split_top(s, sep=","):
    out, depth, cur, i, q = [], 0, "", 0, None
    while i < len(s):
        ch = s[i]
        if q:
            cur += ch
            if ch == "\\":
                cur += s[i + 1]
                i += 1
            elif ch == q:
                q = None
        elif ch in "\"'":
            q = ch
            cur += ch
        elif ch in "([{<" and not (ch == "<" and s[i - 1:i + 1] in ("<=", "<<") or s[i:i + 2] in ("<=", "<<")):
            depth += 1
            cur += ch
        elif ch in ")]}>" and not (s[i - 1:i + 1] == "->" or s[i:i + 2] in (">=", ">>")):
            depth -= 1
            cur += ch
        elif ch == sep and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
        i += 1
    if cur.strip():
        out.append(cur.strip())
    return out


# ---------------------------------------------------------------- expression parser

PREC = [["||"], ["&&"], ["==", "!=", "<", ">", "<=", ">="], ["|"], ["^"], ["&"], ["<<", ">>"], ["+", "-"], ["*", "/", "%"]]


class P:
    def __init__(self, toks, src):
        self.t, self.i, self.src = toks, 0, src

    def peek(self, k=0):
        return self.t[self.i + k] if self.i + k < len(self.t) else (None, None)

    def eat(self, v=None):
        tok = self.peek()
        if tok[0] is None:
            raise Err(f"unexpected end of {self.src!r}")
        if v is not None and tok[1] != v:
            raise Err(f"expected {v!r}, got {tok[1]!r} in {self.src!r}")
        self.i += 1
        return tok

    def accept(self, v):
        if self.peek()[1] == v and self.peek()[0] in ("op", "id"):
            self.i += 1
            return True
        return False

    def expr(self, lvl=0, no_struct=False):
        if lvl == len(PREC):
            return self.unary(no_struct)
        left = self.expr(lvl + 1, no_struct)
        while self.peek()[0] == "op" and self.peek()[1] in PREC[lvl]:
            op = self.eat()[1]
            right = self.expr(lvl + 1, no_struct)
            left = ("bin", op, left, right)
        return left

    def unary(self, ns):
        k, v = self.peek()
        if k == "op" and v in ("-", "!"):
            self.eat()
            e = self.unary(ns)
            if v == "-" and e[0] == "num":
                return ("num", -e[1])
            return ("un", v, e)
        if k == "op" and v == "&":
            self.eat()
            if self.peek()[1] == "mut":
                self.eat()
                return ("mutref", self.unary(ns))
            return self.unary(ns)
        return self.postfix(ns)

    def postfix(self, ns):
        e = self.atom(ns)
        while True:
            k, v = self.peek()
            if k == "op" and v == ".":
                self.eat()
                k2, v2 = self.eat()
                if k2 == "num":
                    e = ("tfield", e, int(v2))
                elif self.peek()[1] == "(":
                    self.eat("(")
                    args = self.args(")")
                    e = ("mcall", e, v2, args)
                else:
                    e = ("field", e, v2)
            elif k == "op" and v == "[":
                self.eat()
                i = self.expr()
                self.eat("]")
                e = ("index", e, i)
            elif k == "id" and v == "as":
                self.eat()
                ty = self.eat()[1]
                e = ("as", e, ty)
            else:
                break
        return e

    def args(self, close):
        out = []
        while self.peek()[1] != close:
            out.append(self.expr())
            if not self.accept(","):
                break
        self.eat(close)
        return out

    def atom(self, ns):
        k, v = self.eat()
        if k == "num":
            return ("num", int(v.replace("_", ""), 0))
        if k == "chr":
            c = v[1:-1]
            c = {"\\n": "\n", "\\'": "'", "\\\\": "\\"}.get(c, c)
            return ("num", ord(c))
        if k == "str":
            return ("str", bytes(v[1:-1], "utf8").decode("unicode_escape"))
        if k == "op" and v == "(":
            if self.accept(")"):
                return ("unit",)
            e = self.expr()
            if self.accept(","):
                items = [e]
                while self.peek()[1] != ")":
                    items.append(self.expr())
                    if not self.accept(","):
                        break
                self.eat(")")
                return ("tuple", items)
            self.eat(")")
            return ("paren", e)
        if k == "id":
            if v in ("true", "false"):
                return ("bool", v == "true")
            if v == "if":
                c = self.expr(no_struct=True)
                self.eat("{")
                a = self.expr()
                self.eat("}")
                self.eat("else")
                self.eat("{")
                b = self.expr()
                self.eat("}")
                return ("if", c, a, b)
            path = [v]
            while self.peek()[1] == "::":
                self.eat()
                path.append(self.eat()[1])
            name = ".".join(path)
            if self.peek()[1] == "(":
                self.eat()
                return ("call", name, self.args(")"))
            if self.peek()[1] == "{" and not ns and v[0].isupper() and len(path) == 1:
                self.eat("{")
                fields = []
                while self.peek()[1] != "}":
                    f = self.eat()[1]
                    if self.accept(":"):
                        fields.append((f, self.expr()))
                    else:
                        fields.append((f, ("var", f)))
                    if not self.accept(","):
                        break
                self.eat("}")
                return ("structlit", v, fields)
            if len(path) > 1:
                return ("path", name)
            return ("var", v)
        raise Err(f"unexpected {v!r} in {self.src!r}")


def parse_expr(src):
    p = P(lex(src), src)
    e = p.expr()
    if p.peek()[0] is not None:
        raise Err(f"trailing {p.peek()[1]!r} in {src!r}")
    return e


# ---------------------------------------------------------------- statements

class Fn:
    def __init__(self, name, params, ret, body, extern=None, line=0):
        self.name, self.params, self.ret, self.body = name, params, ret, body
        self.extern, self.line = extern, line
        self.module = None


class Sig:
    def __init__(self, bend, params, ret):
        self.bend, self.params, self.ret = bend, params, ret  # params: [(name, type, mut)]
        self.muts = [i for i, p in enumerate(params) if p[2]]


def indent_of(line):
    return len(line) - len(line.lstrip(" "))


def strip_comment(line):
    out, q = "", None
    i = 0
    while i < len(line):
        ch = line[i]
        if q:
            out += ch
            if ch == "\\":
                out += line[i + 1]
                i += 1
            elif ch == q:
                q = None
        elif ch == '"':
            q = ch
            out += ch
        elif ch == "'" and re.match(r"'(?:\\.|[^'\\])'", line[i:]):
            m = re.match(r"'(?:\\.|[^'\\])'", line[i:])
            out += m.group(0)
            i += len(m.group(0)) - 1
        elif ch == "#":
            break
        else:
            out += ch
        i += 1
    return out.rstrip()


def logical_lines(text):
    """(indent, text, lineno) with open brackets joined."""
    out, buf, bind, bno = [], "", 0, 0
    depth = 0
    for no, raw in enumerate(text.splitlines(), 1):
        line = strip_comment(raw)
        if not line.strip():
            continue
        if depth == 0:
            buf, bind, bno = line.strip(), indent_of(line), no
        else:
            buf += " " + line.strip()
        depth = sum(1 for ch in strip_str(buf) if ch in "([{") - sum(1 for ch in strip_str(buf) if ch in ")]}")
        if depth <= 0:
            out.append((bind, buf, bno))
            buf, depth = "", 0
    if depth:
        raise Err("unbalanced brackets at end")
    return out


def strip_str(s):
    s = re.sub(r'"(?:[^"\\]|\\.)*"', '""', s)
    return re.sub(r"'(?:\\.|[^'\\])'", "''", s)


def find_assign(s):
    """Index of a top-level assignment operator in a statement, with its operator text."""
    depth = 0
    t = strip_str(s)
    for i, ch in enumerate(t):
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        elif depth == 0 and ch == "=":
            prev = t[i - 1] if i else ""
            nxt = t[i + 1] if i + 1 < len(t) else ""
            if nxt == "=" or prev in "=!<>" and prev != "":
                if prev in "<>" and t[i - 2:i] in ("<<", ">>"):
                    return i - 2, t[i - 2:i + 1]
                continue
            if prev in "+-*/%|&^":
                return i - 1, prev + "="
            return i, "="
    return None


def parse_block(lines, i, ind):
    """Statements at indent `ind` starting at lines[i]; returns (stmts, next i)."""
    out = []
    while i < len(lines) and lines[i][0] >= ind:
        if lines[i][0] > ind:
            raise Err(f"line {lines[i][2]}: unexpected indent")
        ind_, text, no = lines[i]
        try:
            st, i = parse_stmt(lines, i, ind)
        except Err as e:
            raise Err(f"line {no}: {e}")
        out.append(st)
    return out, i


def parse_stmt(lines, i, ind):
    _, text, no = lines[i]
    if text.startswith("if ") or text.startswith("if("):
        return parse_if(lines, i, ind)
    if text == "pass":
        return ("pass",), i + 1
    if text.startswith("return"):
        rest = text[6:].strip()
        return ("return", parse_expr(rest) if rest else None), i + 1
    if text.startswith("let "):
        body = text[4:]
        if body.startswith("mut "):
            body = body[4:]
        k = find_assign(body)
        if k is None:
            raise Err("let needs =")
        lhs, rhs = body[:k[0]].strip(), body[k[0] + len(k[1]):].strip()
        ty = None
        if lhs.startswith("(") and lhs.endswith(")"):
            names = [n.strip().removeprefix("mut ").strip() for n in split_top(lhs[1:-1])]
            return ("letpat", names, parse_expr(rhs)), i + 1
        m = re.fullmatch(r"(\w+)\s*(?::\s*(.+))?", lhs)
        if not m:
            raise Err(f"bad let {lhs!r}")
        return ("let", m.group(1), m.group(2), parse_expr(rhs)), i + 1
    k = find_assign(text)
    if k is not None:
        lhs, op = text[:k[0]].strip(), k[1]
        rhs = parse_expr(text[k[0] + len(op):].strip())
        return ("assign", parse_expr(lhs), op, rhs), i + 1
    return ("expr", parse_expr(text)), i + 1


def parse_if(lines, i, ind):
    _, text, no = lines[i]
    cond = text[2:].strip()
    if not cond.endswith(":"):
        raise Err("if needs a colon")
    cond = parse_expr(cond[:-1].strip())
    i += 1
    then, i = parse_sub(lines, i, ind)
    els = []
    if i < len(lines) and lines[i][0] == ind and lines[i][1].startswith("elif "):
        lines[i] = (ind, "if " + lines[i][1][5:], lines[i][2])
        st, i = parse_if(lines, i, ind)
        els = [st]
    elif i < len(lines) and lines[i][0] == ind and lines[i][1] == "else:":
        els, i = parse_sub(lines, i + 1, ind)
    return ("if", cond, then, els), i


def parse_sub(lines, i, ind):
    if i >= len(lines) or lines[i][0] <= ind:
        raise Err("empty block")
    return parse_block(lines, i, lines[i][0])


# ---------------------------------------------------------------- the compiler

class Compiler:
    def __init__(self, reg):
        self.reg = reg
        self.out = []  # finished bend text blocks
        self.counter = 0
        self.ntmp = 0
        self.nif = 0
        self.last_type = None
        self.module = None

    # --- names
    def resolve_const(self, name, module):
        reg = self.reg
        cands = [f"{module}.{name}", f"bannerpeak.{name}", name]
        for c in cands:
            if c in reg.consts:
                return c
        short = name.split(".")[-1]
        if "." not in name and short in reg.short_consts and len(reg.short_consts[short]) == 1:
            return reg.short_consts[short][0]
        if "." in name:
            pre, last = name.rsplit(".", 1)
            for full in reg.short_consts.get(last, []):
                if full.endswith("." + pre + "." + last) or full == pre + "." + last:
                    return full
        return None

    def resolve_fn(self, name, module):
        reg = self.reg
        for c in (f"{module}.{name}", f"bannerpeak.{name}", name):
            if c in reg.fns:
                return reg.fns[c]
        if "." in name:
            ty, m = name.rsplit(".", 1)
            if (ty, m) in reg.methods:
                return reg.fns[reg.methods[(ty, m)]]
            # module-qualified: layout.movers -> bannerpeak.layout.movers
            for full, sig in reg.fns.items():
                if full.endswith("." + name):
                    return sig
        return None

    # --- compile one fn
    def compile_fn(self, fn):
        reg = self.reg
        self.fn = fn
        self.module = fn.module
        sig = reg.fns[fn.bend]
        self.sig = sig
        self.lifted = []
        self.nif = 0
        self.ntmp = 0
        env = {p[0]: p[1] for p in sig.params}
        lines = self.block(fn.body, env, "fn", None)
        params = ", ".join(f"+{n}: {bt(t)}" for n, t, _ in sig.params)
        head = f"def {fn.bend}({params}) -> {bt(self.ret_type(sig))}:"
        self.out.append(head + "\n" + "\n".join("  " + l for l in lines) + "\n")
        self.out.extend(self.lifted)

    def ret_type(self, sig):
        parts = self.ret_parts_types(sig)
        return ("tup", parts) if len(parts) > 1 else (parts[0] if parts else "unit")

    def ret_parts_types(self, sig):
        parts = []
        if sig.ret != "unit":
            parts.append(sig.ret)
        parts += [sig.params[i][1] for i in sig.muts]
        return parts

    def tmp(self, base="t"):
        self.ntmp += 1
        return f"_{base}{self.ntmp}"

    # --- projections out of a call's result
    def proj(self, name, parts_types, idx):
        n = len(parts_types)
        if n == 1:
            return name
        text = name
        for k in range(idx):
            text = f"bannerpeak.u.snd({bt(parts_types[k])}, {nest_type([bt(x) for x in parts_types[k + 1:]])}, {text})"
        if idx < n - 1:
            text = f"bannerpeak.u.fst({bt(parts_types[idx])}, {nest_type([bt(x) for x in parts_types[idx + 1:]])}, {text})"
        return text

    def mk_tuple(self, texts):
        if len(texts) == 1:
            return texts[0]
        return "(" + texts[0] + ", " + self.mk_tuple(texts[1:]) + ")"

    # --- blocks
    def block(self, stmts, env, mode, mvars):
        env = dict(env)
        lines = []
        for idx, st in enumerate(stmts):
            kind = st[0]
            if kind == "pass":
                continue
            if kind == "let":
                _, name, ty, e = st
                pre = []
                text, t = self.expr(e, env, pre, expect=reg_type(self.reg, ty) if ty else None)
                if ty:
                    t = self.reg.type_of(ty)
                    text = self.coerce(text, self.last_type, t)
                lines += pre
                lines.append(f"+{name} : {bt(t)} = {text}")
                env[name] = t
            elif kind == "letpat":
                _, names, e = st
                pre = []
                text, t = self.expr(e, env, pre)
                if not (isinstance(t, tuple) and t[0] == "tup" and len(t[1]) == len(names)):
                    raise Err(f"let pattern {names} against {tshow(t)}")
                lines += pre
                tn = self.tmp()
                lines.append(f"+{tn} : {bt(t)} = {text}")
                for k, n in enumerate(names):
                    if n != "_":
                        lines.append(f"+{n} = {self.proj(tn, t[1], k)}")
                        env[n] = t[1][k]
            elif kind == "assign":
                _, lhs, op, rhs = st
                pre = []
                self.assign(lhs, op, rhs, env, pre)
                lines += pre
            elif kind == "expr":
                pre = []
                text, t = self.expr(st[1], env, pre, stmt=True)
                lines += pre
                # a value-less statement: a call already rebound what it mutates
            elif kind == "return":
                pre = []
                e = st[1]
                val = None
                if e is not None:
                    text, t = self.expr(e, env, pre, expect=self.sig.ret)
                    text = self.coerce(text, self.last_type, self.sig.ret)
                    val = text
                lines += pre
                lines.append(self.ret_text(val, env))
                return lines
            elif kind == "if":
                rest = stmts[idx + 1:]
                if has_return([st]):
                    if mode != "fn":
                        raise Err("return inside an if that merges")
                    lines += self.return_if(st, rest, env)
                    return lines
                lines += self.merge_if(st, env)
            else:
                raise Err(f"statement {kind}")
        # fell off the end
        if mode == "fn":
            if self.sig.ret != "unit":
                raise Err(f"{self.fn.name}: ends without a value")
            lines.append(self.ret_text(None, env))
        else:
            lines.append(self.mk_tuple(list(mvars)) if mvars else "Unit{}")
        return lines

    def ret_text(self, val, env):
        sig = self.sig
        texts = []
        if sig.ret != "unit":
            if val is None:
                raise Err(f"{self.fn.name}: return needs a value")
            texts.append(val)
        for i in sig.muts:
            texts.append(sig.params[i][0])
        if not texts:
            return "Unit{}"
        return self.mk_tuple(texts)

    # --- if
    def used_vars(self, env, text):
        t = strip_str(text)
        return [v for v in env if re.search(r"(?<![\w.])" + re.escape(v) + r"(?![\w])", t)]

    def lift(self, kind, cond_text, arms, env, rtype_bend):
        """A def that matches `c` and runs one arm; returns the call text."""
        self.nif += 1
        name = f"{self.fn.bend}.if{self.nif}"
        body = "\n".join(arms[0]) + "\n" + "\n".join(arms[1])
        used = self.used_vars(env, body)
        params = ", ".join(f"+{v}: {bt(env[v])}" for v in used)
        sep = ", " if used else ""
        text = [f"def {name}(bpc: Bool{sep}{params}) -> {rtype_bend}:", "  match bpc:", "    case True{}:"]
        text += ["      " + l for l in arms[0]]
        text += ["    case False{}:"] + ["      " + l for l in arms[1]]
        self.lifted.append("\n".join(text) + "\n")
        return f"{name}({cond_text}{sep}{', '.join(used)})"

    def return_if(self, st, rest, env):
        _, cond, then, els = st
        pre = []
        ctext, ct = self.expr(cond, env, pre)
        then_s = then if always_returns(then) else then + rest
        else_s = els if always_returns(els) else els + rest
        a = self.block(then_s, env, "fn", None)
        b = self.block(else_s, env, "fn", None)
        call = self.lift("ret", ctext, (a, b), env, bt(self.ret_type(self.sig)))
        return pre + [call]

    def merge_if(self, st, env):
        _, cond, then, els = st
        pre = []
        ctext, ct = self.expr(cond, env, pre)
        mv = sorted(assigned(then + els, env, self), key=str)
        mv = [v for v in mv if v in env]
        tys = [env[v] for v in mv]
        a = self.block(then, env, "merge", mv)
        b = self.block(els, env, "merge", mv)
        rt = nest_type([bt(t) for t in tys]) if mv else "Unit"
        call = self.lift("merge", ctext, (a, b), env, rt)
        lines = list(pre)
        if not mv:
            lines.append(f"+{self.tmp('u')} = {call}")
        elif len(mv) == 1:
            lines.append(f"+{mv[0]} = {call}")
        else:
            tn = self.tmp()
            lines.append(f"+{tn} = {call}")
            for k, v in enumerate(mv):
                lines.append(f"+{v} = {self.proj(tn, tys, k)}")
        return lines

    # --- assignment
    def assign(self, lhs, op, rhs, env, pre):
        if op != "=":
            bop = op[:-1]
            rhs = ("bin", bop, lhs, rhs)
        root, steps = self.lvalue(lhs)
        # type of the target
        t = self.path_type(root, steps, env)
        text, rt = self.expr(rhs, env, pre, expect=t)
        text = self.coerce(text, self.last_type, t)
        self.store(root, steps, text, t, env, pre)

    def lvalue(self, e):
        steps = []
        while True:
            if e[0] == "var":
                return e[1], list(reversed(steps))
            if e[0] == "field":
                steps.append(("field", e[2]))
                e = e[1]
            elif e[0] == "index":
                steps.append(("index", e[2]))
                e = e[1]
            elif e[0] == "paren":
                e = e[1]
            else:
                raise Err(f"not an assignable place: {e}")

    def path_type(self, root, steps, env):
        if root not in env:
            raise Err(f"unknown variable {root}")
        t = env[root]
        for kind, x in steps:
            if kind == "field":
                t = self.field_type(t, x)
            else:
                t = self.index_type(t)
        return t

    def field_type(self, t, f):
        if isinstance(t, tuple) and t[0] == "struct":
            sc = self.reg.structs.get(t[1])
            if sc is None:
                raise Err(f"unknown struct {t[1]}")
            if f not in sc:
                raise Err(f"{t[1]} has no field {f}")
            return sc[f]
        raise Err(f"field {f} of {tshow(t)}")

    def index_type(self, t):
        if isinstance(t, tuple) and t[0] in ("vec", "map"):
            return t[1]
        raise Err(f"index into {tshow(t)}")

    def store(self, root, steps, value_text, t, env, pre):
        """root.steps = value (a bend value of type t)."""
        if not steps:
            pre.append(f"+{root} : {bt(env[root])} = {value_text}")
            return
        wtext = self.to_j(value_text, t, pre)
        pre.append(f"+{root} = {self.set_path(root, steps, 0, env[root], wtext, pre, env)}")

    def set_path(self, cur, steps, k, cur_t, wtext, pre, env):
        if k == len(steps):
            return wtext
        kind, x = steps[k]
        if kind == "field":
            ft = self.field_type(cur_t, x)
            inner = self.set_path(f"J.get({cur}, \"{x}\")", steps, k + 1, ft, wtext, pre, env)
            return f"J.set({cur}, \"{x}\", {inner})"
        et = self.index_type(cur_t)
        ipre = []
        itext, it = self.expr(x, env, ipre)
        pre += ipre
        if cur_t[0] == "vec":
            inner = self.set_path(f"J.nth({cur}, {itext})", steps, k + 1, et, wtext, pre, env)
            return f"J.set_nth({cur}, {itext}, {inner})"
        inner = self.set_path(f"bannerpeak.u.at({cur}, {itext})", steps, k + 1, et, wtext, pre, env)
        return f"bannerpeak.u.put({cur}, {itext}, {inner})"

    # --- conversions between bend values and J
    def to_j(self, text, t, pre=None):
        if t == "i":
            return f"J.of_i32({text})"
        if t == "u":
            return f"J.JU{{{text}}}"
        if t == "bool":
            return f"J.JBool{{{text}}}"
        if t == "str":
            return f"J.JS{{{text}}}"
        if t == "J" or (isinstance(t, tuple) and t[0] in ("struct", "option", "vec", "map")):
            return text
        if isinstance(t, tuple) and t[0] == "tup":
            if pre is None:
                raise Err("a tuple needs a statement to bind in")
            nm = self.tmp("j")
            pre.append(f"+{nm} : {bt(t)} = {text}")
            items = [self.to_j(self.proj(nm, t[1], k), x, pre) for k, x in enumerate(t[1])]
            return "J.JA{[" + ", ".join(items) + "]}"
        raise Err(f"to_j {tshow(t)}")

    def from_j(self, text, t):
        if t in ("i", "u"):
            return f"J.u32({text})"
        if t == "bool":
            return f"J.bool({text})"
        if t == "str":
            return f"J.str({text})"
        if t == "J" or (isinstance(t, tuple) and t[0] in ("struct", "option", "vec", "map")):
            return text
        if isinstance(t, tuple) and t[0] == "tup":
            return self.mk_tuple([self.from_j(f"J.nth({text}, {k})", x) for k, x in enumerate(t[1])])
        raise Err(f"from_j {tshow(t)}")

    def coerce(self, text, have, want):
        """Literals and Nones adapt to the wanted type."""
        if have == "lit" and want in ("i", "u"):
            return text
        if want == "unit":
            return text
        return text

    # --- expressions
    def expr(self, e, env, pre, expect=None, stmt=False):
        """Returns (bend text, type); hoisted statements go to `pre`. self.last_type: the
        type of a literal before it adapted ('lit') or the type."""
        text, t = self.expr_(e, env, pre, expect, stmt)
        self.last_type = t
        if t == "lit":
            t2 = expect if expect in ("i", "u") else "u"
            # a negative literal in an unsigned context is a two's complement i32
            return text, t2
        return text, t

    def lit(self, v, signed):
        return str(v & MASK)

    def expr_(self, e, env, pre, expect, stmt):
        k = e[0]
        reg = self.reg
        if k == "num":
            return str(e[1] & MASK), "lit"
        if k == "bool":
            return ("True{}" if e[1] else "False{}"), "bool"
        if k == "str":
            return '"' + e[1].replace("\\", "\\\\").replace('"', '\\"') + '"', "str"
        if k == "unit":
            return "Unit{}", "unit"
        if k == "paren":
            return self.expr(e[1], env, pre, expect)
        if k == "var":
            name = e[1]
            if name in env:
                return name, env[name]
            c = self.resolve_const(name, self.module)
            if c:
                t, v = reg.consts[c]
                return str(v & MASK), (t if t in ("i", "u") else t)
            if name == "None":
                return "J.JNull{}", ("option", "J")
            raise Err(f"unknown name {name}")
        if k == "path":
            parts = e[1].split(".")
            if len(parts) == 2 and parts[0] in reg.enums:
                if parts[1] not in reg.enums[parts[0]]:
                    raise Err(f"enum {parts[0]} has no {parts[1]}")
                return '"' + parts[1] + '"', "str"
            c = self.resolve_const(e[1], self.module)
            if c:
                t, v = reg.consts[c]
                return str(v & MASK), t
            raise Err(f"unknown path {e[1]}")
        if k == "tuple":
            texts, ts = [], []
            for x in e[1]:
                tx, tt = self.expr(x, env, pre)
                texts.append(tx)
                ts.append(tt)
            return self.mk_tuple(texts), ("tup", ts)
        if k == "tfield":
            tx, tt = self.expr(e[1], env, pre)
            if not (isinstance(tt, tuple) and tt[0] == "tup"):
                raise Err(f"tuple field of {tshow(tt)}")
            nm = self.tmp()
            pre.append(f"+{nm} : {bt(tt)} = {tx}")
            return self.proj(nm, tt[1], e[2]), tt[1][e[2]]
        if k == "field":
            tx, tt = self.expr(e[1], env, pre)
            ft = self.field_type(tt, e[2])
            return self.from_j(f"J.get({tx}, \"{e[2]}\")", ft), ft
        if k == "index":
            tx, tt = self.expr(e[1], env, pre)
            ix, it = self.expr(e[2], env, pre)
            et = self.index_type(tt)
            if tt[0] == "vec":
                return self.from_j(f"J.nth({tx}, {ix})", et), et
            return self.from_j(f"bannerpeak.u.at({tx}, {ix})", et), et
        if k == "un":
            tx, tt = self.expr(e[2], env, pre, expect)
            if e[1] == "!":
                return f"Bool.not({tx})", "bool"
            if tt == "u" and expect == "i":
                tt = "i"
            return f"RS.i_neg({tx})", tt
        if k == "bin":
            return self.binop(e, env, pre, expect)
        if k == "as":
            tx, tt = self.expr(e[1], env, pre)
            return self.cast(tx, tt, e[2])
        if k == "if":
            ctx_, ct = self.expr(e[1], env, pre)
            at, att = self.expr(e[2], env, pre, expect)
            bt_, btt = self.expr(e[3], env, pre, expect if expect else att)
            t = att if att != "lit" else btt
            tb = bt(t if t != "lit" else "u")
            return f"bannerpeak.u.ife({tb}, {ctx_}, _ => {at}, _ => {bt_})", t
        if k == "structlit":
            return self.structlit(e, env, pre)
        if k == "mutref":
            raise Err("&mut outside a call argument")
        if k == "call":
            return self.call(e, env, pre, expect, stmt)
        if k == "mcall":
            return self.mcall(e, env, pre, expect, stmt)
        raise Err(f"expression {k}")

    def cast(self, tx, tt, to):
        if to in ("i32", "isize", "u32", "usize", "u64", "i64"):
            if tt == "bool":
                return f"bannerpeak.u.b2u({tx})", ("i" if to in SIGNED else "u")
            return tx, ("i" if to in SIGNED else "u")
        if to == "u8":
            return f"RS.as_u8({tx})", "u"
        if to == "u16":
            return f"RS.as_u16({tx})", "u"
        if to == "i8":
            return f"RS.as_i8({tx})", "i"
        if to == "i16":
            return f"RS.as_i16({tx})", "i"
        raise Err(f"cast to {to}")

    CMP = {"<": "lt", "<=": "le", ">": "gt", ">=": "ge"}

    def binop(self, e, env, pre, expect):
        _, op, a, b = e
        if op in ("&&", "||"):
            return self.logic(e, env, pre)
        arith = op in ("+", "-", "*", "/", "%", "<<", ">>", "&", "|", "^")
        # evaluate operands; literals adapt to the other side
        ex = expect if arith else None
        ta, tta = self.expr(a, env, pre, ex)
        la = self.last_type
        tb, ttb = self.expr(b, env, pre, ex if op not in ("<<", ">>") else None)
        lb = self.last_type
        if la == "lit" and lb != "lit":
            tta = ttb if op not in ("<<", ">>") else tta
        if la != "lit" and lb == "lit":
            ttb = tta
        t = tta if tta in ("i", "u") else ttb
        if op in ("==", "!="):
            if t == "bool" or tta == "bool":
                r = f"Bool.xor({ta}, {tb})"
                return (f"Bool.not({r})" if op == "==" else r), "bool"
            if tta == "str" or ttb == "str":
                r = f"String.eq({ta}, {tb})"
                return (r if op == "==" else f"Bool.not({r})"), "bool"
            if t in ("i", "u"):
                r = f"U32.is_eq({ta}, {tb})"
                return (r if op == "==" else f"U32.is_ne({ta}, {tb})"), "bool"
            raise Err(f"== on {tshow(tta)}")
        if op in self.CMP:
            if t == "i":
                return f"RS.i_{self.CMP[op]}({ta}, {tb})", "bool"
            return f"U32.is_{self.CMP[op]}({ta}, {tb})", "bool"
        if t not in ("i", "u"):
            raise Err(f"arithmetic {op} on {tshow(tta)} {tshow(ttb)}")
        if op in "+-*":
            return f"({ta} {op} {tb} : U32)", t
        if op == "/":
            return (f"RS.i_div({ta}, {tb})" if t == "i" else f"U32.div({ta}, {tb})"), t
        if op == "%":
            return (f"RS.i_rem({ta}, {tb})" if t == "i" else f"U32.mod({ta}, {tb})"), t
        if op == "<<":
            return (f"RS.i_shl({ta}, {tb})" if t == "i" else f"RS.u_shl({ta}, {tb})"), t
        if op == ">>":
            return (f"RS.i_shr({ta}, {tb})" if t == "i" else f"RS.u_shr({ta}, {tb})"), t
        if op == "&":
            return f"U32.and({ta}, {tb})", t
        if op == "|":
            return f"U32.or({ta}, {tb})", t
        if op == "^":
            return f"U32.xor({ta}, {tb})", t
        raise Err(f"operator {op}")

    def logic(self, e, env, pre):
        _, op, a, b = e
        ta, tta = self.expr(a, env, pre)
        if self.effects(b):
            tn = self.tmp("b")
            pre.append(f"+{tn} = {ta}")
            env2 = dict(env)
            env2[tn] = "bool"
            cond = ("var", tn) if op == "&&" else ("un", "!", ("var", tn))
            st = ("if", cond, [("assign", ("var", tn), "=", b)], [])
            env2.update({v: env[v] for v in env})
            lines = self.merge_if(st, env2)
            # merge_if rebinds tn (and whatever the effects changed) in env's scope
            pre += lines
            env[tn] = "bool"
            return tn, "bool"
        tb, ttb = self.expr(b, env, pre)
        if op == "&&":
            return f"Bool.and({ta}, {tb})", "bool"
        return f"Bool.or({ta}, {tb})", "bool"

    def effects(self, e):
        """Whether evaluating the expression can change a variable."""
        if not isinstance(e, tuple):
            return False
        k = e[0]
        if k == "call":
            sig = self.resolve_fn(e[1], self.module)
            if sig is not None and sig.muts:
                return True
            return any(self.effects(x) for x in e[2])
        if k == "mcall":
            if e[2] in MUT_BUILTINS:
                return True
            recv_sig = self.method_sig_guess(e[2])
            if recv_sig is not None and recv_sig.muts:
                return True
            return self.effects(e[1]) or any(self.effects(x) for x in e[3])
        if k == "mutref":
            return True
        for x in e[1:]:
            if isinstance(x, tuple) and self.effects(x):
                return True
            if isinstance(x, list):
                for y in x:
                    if isinstance(y, tuple) and (self.effects(y) or (len(y) == 2 and isinstance(y[1], tuple) and self.effects(y[1]))):
                        return True
        return False

    def method_sig_guess(self, name):
        sigs = [self.reg.fns[f] for (ty, m), f in self.reg.methods.items() if m == name]
        if sigs and all(s.muts and s.params[0][2] for s in sigs):
            return sigs[0]
        if sigs and any(s.muts and s.params[0][2] for s in sigs):
            return [s for s in sigs if s.muts][0]
        return None

    def structlit(self, e, env, pre):
        _, name, fields = e
        sc = self.reg.structs.get(name)
        if sc is None:
            raise Err(f"unknown struct {name}")
        text = "J.obj()"
        given = {}
        for f, x in fields:
            if f not in sc:
                raise Err(f"{name} has no field {f}")
            tx, tt = self.expr(x, env, pre, expect=sc[f])
            given[f] = self.to_j(tx, sc[f], pre)
        for f in sc:
            if f in given:
                text = f"J.set({text}, \"{f}\", {given[f]})"
            else:
                raise Err(f"{name} literal misses {f}")
        return text, ("struct", name)

    # --- calls
    def call(self, e, env, pre, expect, stmt):
        _, name, args = e
        # builtins
        b = self.builtin_call(name, args, env, pre, expect)
        if b is not None:
            return b
        sig = self.resolve_fn(name, self.module)
        if sig is None:
            raise Err(f"unknown function {name}")
        return self.apply(sig, args, None, env, pre, stmt)

    def mcall(self, e, env, pre, expect, stmt):
        _, recv, name, args = e
        # type of the receiver
        rt = self.peek_type(recv, env)
        b = self.builtin_method(recv, rt, name, args, env, pre, expect)
        if b is not None:
            return b
        if isinstance(rt, tuple) and rt[0] == "struct":
            full = self.reg.methods.get((rt[1], name))
            if full is None:
                raise Err(f"{rt[1]} has no method {name}")
            return self.apply(self.reg.fns[full], args, recv, env, pre, stmt)
        raise Err(f"method {name} on {tshow(rt)}")

    def peek_type(self, e, env):
        """The static type of an expression without emitting it."""
        tmp = []
        saved = (self.ntmp, self.nif, self.last_type if hasattr(self, "last_type") else None)
        text, t = self.expr(e, env, tmp)
        self.ntmp, self.nif = saved[0], saved[1]
        return t

    def apply(self, sig, args, recv, env, pre, stmt):
        """Call a declared fn. recv: the method receiver expression, if any."""
        full_args = ([recv] if recv is not None else []) + list(args)
        if len(full_args) != len(sig.params):
            raise Err(f"{sig.bend}: {len(sig.params)} arguments, got {len(full_args)}")
        texts, places = [], []
        for (pn, pt, pm), a in zip(sig.params, full_args):
            if pm:
                inner = a[1] if a[0] == "mutref" else a
                root, steps = self.lvalue(inner)
                cur, ct = self.expr(inner, env, pre, expect=pt)
                texts.append(cur)
                places.append((root, steps, pt))
            else:
                if a[0] == "mutref":
                    a = a[1]
                tx, tt = self.expr(a, env, pre, expect=pt)
                texts.append(tx)
                places.append(None)
        call = f"{sig.bend}({', '.join(texts)})"
        parts = [sig.ret] if sig.ret != "unit" else []
        parts += [sig.params[i][1] for i in sig.muts]
        if not sig.muts:
            return call, sig.ret
        tn = self.tmp()
        pre.append(f"+{tn} = {call}")
        base = 1 if sig.ret != "unit" else 0
        for j, i in enumerate(sig.muts):
            root, steps, pt = places[i]
            val = self.proj(tn, parts, base + j)
            self.store(root, steps, val, pt, env, pre)
        if sig.ret == "unit":
            return "Unit{}", "unit"
        return self.proj(tn, parts, 0), sig.ret

    # --- builtins
    def builtin_call(self, name, args, env, pre, expect):
        if name in ("min", "max") and len(args) == 2:
            return self.minmax(name, args[0], args[1], env, pre, expect)
        if name in ("inr", "inc") and len(args) == 3:
            x, tx = self.expr(args[0], env, pre)
            lx = self.last_type
            lo, tlo = self.expr(args[1], env, pre, tx if tx in ("i", "u") else None)
            hi, thi = self.expr(args[2], env, pre, tx if tx in ("i", "u") else None)
            t = tx if tx in ("i", "u") else tlo
            f = "RS.i_" if t == "i" else "U32.is_"
            up = "lt" if name == "inr" else "le"
            return f"Bool.and({f}ge({x}, {lo}), {f}{up}({x}, {hi}))", "bool"
        if name in ("Map.new", "Map::new") and not args:
            return "J.obj()", (expect if isinstance(expect, tuple) and expect[0] == "map" else ("map", "J"))
        if name in ("Vec.new", "Vec::new") and not args:
            return "bannerpeak.u.vec_new()", (expect if isinstance(expect, tuple) and expect[0] == "vec" else ("vec", "J"))
        if name == "Some" and len(args) == 1:
            tx, tt = self.expr(args[0], env, pre)
            return self.to_j(tx, tt, pre), ("option", tt)
        return None

    def minmax(self, name, a, b, env, pre, expect):
        ta, tta = self.expr(a, env, pre, expect)
        la = self.last_type
        tb, ttb = self.expr(b, env, pre, expect)
        lb = self.last_type
        t = tta if la != "lit" else ttb
        if la == "lit" and lb == "lit":
            t = expect if expect in ("i", "u") else "u"
        f = ("RS.i_" if t == "i" else "RS.u_") + name
        return f"{f}({ta}, {tb})", t

    def minmax_txt(self, name, ta, tta, la, tb, ttb, lb, expect):
        t = tta if la != "lit" else ttb
        if la == "lit" and lb == "lit":
            t = expect if expect in ("i", "u") else "u"
        f = ("RS.i_" if t == "i" else "RS.u_") + name
        return f"{f}({ta}, {tb})", t

    def builtin_method(self, recv, rt, name, args, env, pre, expect):
        if rt in ("i", "u", "lit"):
            tx, _ = self.expr(recv, env, pre, expect if expect in ("i", "u") else None)
            t = rt if rt != "lit" else (expect if expect in ("i", "u") else "i")
            if name == "abs":
                return f"RS.i_abs({tx})", "i"
            if name == "signum":
                return f"RS.i_signum({tx})", "i"
            if name in ("min", "max"):
                a0, ta0 = self.expr(args[0], env, pre, t)
                la0 = "lit" if args[0][0] == "num" else "typed"
                lr = "lit" if recv[0] == "num" else "typed"
                return self.minmax_txt(name, tx, t, lr, a0, ta0, la0, t)
            if name == "clamp":
                lo, _ = self.expr(args[0], env, pre, t)
                hi, _ = self.expr(args[1], env, pre, t)
                f = "RS.i_clamp" if t == "i" else "RS.u_clamp"
                return f"{f}({tx}, {lo}, {hi})", t
            if name in ("rem_euclid", "div_euclid"):
                a0, _ = self.expr(args[0], env, pre, t)
                f = ("RS.i_" if t == "i" else "bannerpeak.u.") + name
                if t == "u":
                    return (f"U32.mod({tx}, {a0})" if name == "rem_euclid" else f"U32.div({tx}, {a0})"), t
                return f"{f}({tx}, {a0})", t
            if name == "saturating_sub":
                a0, _ = self.expr(args[0], env, pre, t)
                return f"RS.u_sat_sub({tx}, {a0})", "u"
            if name == "saturating_add":
                a0, _ = self.expr(args[0], env, pre, t)
                return f"RS.u_sat_add({tx}, {a0})", "u"
            if name in ("wrapping_add", "wrapping_sub", "wrapping_mul"):
                a0, _ = self.expr(args[0], env, pre, t)
                op = {"wrapping_add": "+", "wrapping_sub": "-", "wrapping_mul": "*"}[name]
                return f"({tx} {op} {a0} : U32)", t
            if name == "count_ones":
                return f"RS.u_count_ones({tx})", "u"
            if name == "unsigned_abs":
                return f"RS.i_abs({tx})", "u"
            raise Err(f"int method {name}")
        if isinstance(rt, tuple) and rt[0] == "option":
            tx, _ = self.expr(recv, env, pre)
            if name == "is_none":
                return f"J.is_null({tx})", "bool"
            if name == "is_some":
                return f"Bool.not(J.is_null({tx}))", "bool"
            if name == "unwrap":
                return self.from_j(tx, rt[1]), rt[1]
            raise Err(f"Option method {name}")
        if isinstance(rt, tuple) and rt[0] == "vec":
            if name == "len":
                tx, _ = self.expr(recv, env, pre)
                return f"J.len({tx})", "u"
            if name == "is_empty":
                tx, _ = self.expr(recv, env, pre)
                return f"U32.is_zero(J.len({tx}))", "bool"
            if name == "get":
                tx, _ = self.expr(recv, env, pre)
                ix, _ = self.expr(args[0], env, pre)
                return f"J.nth({tx}, {ix})", ("option", rt[1])
            if name in ("push", "remove", "truncate", "clear"):
                root, steps = self.lvalue(recv)
                cur, _ = self.expr(recv, env, pre)
                if name == "push":
                    v, vt = self.expr(args[0], env, pre, expect=rt[1])
                    new = f"J.push({cur}, {self.to_j(v, vt, pre)})"
                elif name == "remove":
                    v, vt = self.expr(args[0], env, pre)
                    new = f"bannerpeak.u.vec_remove({cur}, {v})"
                elif name == "truncate":
                    v, vt = self.expr(args[0], env, pre)
                    new = f"J.truncate({cur}, {v})"
                else:
                    new = "bannerpeak.u.vec_new()"
                self.store_raw(root, steps, new, recv, env, pre)
                return "Unit{}", "unit"
            raise Err(f"Vec method {name}")
        if isinstance(rt, tuple) and rt[0] == "map":
            tx, _ = self.expr(recv, env, pre)
            if name == "contains_key":
                ix, _ = self.expr(args[0], env, pre)
                return f"bannerpeak.u.has({tx}, {ix})", "bool"
            if name == "len":
                return f"J.len_obj({tx})", "u"
            if name == "ids":
                return f"bannerpeak.u.ids({tx})", ("list", "u")
            if name == "get":
                ix, _ = self.expr(args[0], env, pre)
                return f"bannerpeak.u.at({tx}, {ix})", ("option", rt[1])
            if name in ("insert", "remove"):
                root, steps = self.lvalue(recv)
                ix, _ = self.expr(args[0], env, pre)
                if name == "insert":
                    v, vt = self.expr(args[1], env, pre, expect=rt[1])
                    new = f"bannerpeak.u.put({tx}, {ix}, {self.to_j(v, vt, pre)})"
                else:
                    new = f"bannerpeak.u.del({tx}, {ix})"
                self.store_raw(root, steps, new, recv, env, pre)
                return "Unit{}", "unit"
            raise Err(f"Map method {name}")
        if isinstance(rt, tuple) and rt[0] == "list":
            tx, _ = self.expr(recv, env, pre)
            if name == "is_empty":
                return f"bannerpeak.u.is_empty({tx})", "bool"
            if name == "head":
                return f"bannerpeak.u.head({tx})", rt[1]
            if name == "tail":
                return f"bannerpeak.u.tail({tx})", rt
            raise Err(f"list method {name}")
        if name == "set_has" or name == "set_insert":
            pass
        return None

    def store_raw(self, root, steps, new_text, recv, env, pre):
        pre.append(f"+{root} = {self.set_path(root, steps, 0, env[root], new_text, pre, env)}")


MUT_BUILTINS = {"push", "remove", "truncate", "clear", "insert"}


def reg_type(reg, ty):
    return reg.type_of(ty) if ty else None


def has_return(stmts):
    for st in stmts:
        if st[0] == "return":
            return True
        if st[0] == "if" and (has_return(st[2]) or has_return(st[3])):
            return True
    return False


def always_returns(stmts):
    for st in stmts:
        if st[0] == "return":
            return True
        if st[0] == "if" and always_returns(st[2]) and always_returns(st[3]):
            return True
    return False


def assigned(stmts, env, comp):
    """Names of variables from outside the block that the statements assign."""
    out = set()
    declared = set()

    def root_of(e):
        while e[0] in ("field", "index", "paren"):
            e = e[1]
        return e[1] if e[0] == "var" else None

    def walk_expr(e):
        if not isinstance(e, tuple):
            return
        k = e[0]
        if k == "call":
            sig = comp.resolve_fn(e[1], comp.module)
            for j, a in enumerate(e[2]):
                walk_expr(a)
                if sig is not None and j < len(sig.params) and sig.params[j][2]:
                    r = root_of(a[1] if a[0] == "mutref" else a)
                    if r and r not in declared:
                        out.add(r)
            return
        if k == "mcall":
            recv = e[1]
            walk_expr(recv)
            for a in e[3]:
                walk_expr(a)
            mut = e[2] in MUT_BUILTINS or any(
                s.params[0][2] for (ty, m), f in comp.reg.methods.items() if m == e[2] for s in [comp.reg.fns[f]]
            )
            if mut:
                r = root_of(recv)
                if r and r not in declared:
                    out.add(r)
            return
        for x in e[1:]:
            if isinstance(x, tuple):
                walk_expr(x)
            elif isinstance(x, list):
                for y in x:
                    if isinstance(y, tuple):
                        walk_expr(y)
                        if len(y) == 2 and isinstance(y[1], tuple):
                            walk_expr(y[1])

    def walk(ss):
        for st in ss:
            k = st[0]
            if k == "let":
                walk_expr(st[3])
                declared.add(st[1])
            elif k == "letpat":
                walk_expr(st[2])
                declared.update(st[1])
            elif k == "assign":
                walk_expr(st[3])
                walk_expr(st[1])
                r = root_of(st[1])
                if r and r not in declared:
                    out.add(r)
            elif k == "expr":
                walk_expr(st[1])
            elif k == "return":
                if st[1] is not None:
                    walk_expr(st[1])
            elif k == "if":
                walk_expr(st[1])
                walk(st[2])
                walk(st[3])

    walk(stmts)
    # `&&`/`||` with effects introduce temporaries; they are local
    return {v for v in out if v in env}


def compile_case(comp, reg, spec, module):
    """case NAME: key=param, ... => key=part, ...  (NAME a fn; unnamed keys are the names)."""
    if ":" in spec:
        name, _, rest = spec.partition(":")
        name, rest = name.strip(), rest.strip()
    else:
        name, rest = spec.strip(), ""
        if " " in name and " via " not in name:
            name, _, rest = name.partition(" ")
    ins, _, outs = rest.partition("=>")
    key_of = {}
    for item in split_top(ins):
        if item:
            k, p = item.split("=")
            key_of[p.strip()] = k.strip()
    out_key = {}
    for item in split_top(outs):
        if item:
            k, p = item.split("=")
            out_key[p.strip()] = k.strip()
    wrapper = None
    if " via " in name:
        name, _, via = name.partition(" via ")
        name, via = name.strip(), via.strip()
        wrapper = f"{module}.{name}"
        name = via
    sig = comp.resolve_fn(name, module)
    if sig is None:
        raise Err(f"case {name}: unknown function")
    wrapper = wrapper or sig.bend
    comp.fn = type("F", (), {"name": name, "bend": sig.bend})()
    args = []
    for pn, pt, pm in sig.params:
        key = key_of.get(pn, pn)
        j = f'J.get(input, "{key}")'
        if isinstance(pt, tuple) and pt[0] == "vec":
            j = f"bannerpeak.u.vec_or({j})"
        args.append(comp.from_j(j, pt))
    call = f"{sig.bend}({', '.join(args)})"
    parts = ([("ret", sig.ret)] if sig.ret != "unit" else []) + [(sig.params[i][0], sig.params[i][1]) for i in sig.muts]
    lines = []
    if not parts:
        body = "J.JNull{}"
    elif len(parts) == 1:
        body = encode(comp, call, parts[0][1], lines)
    else:
        lines.append(f"+r = {call}")
        types = [t for _, t in parts]
        obj = "J.obj()"
        for idx, (pn, pt) in enumerate(parts):
            key = out_key.get(pn, pn)
            obj = f'J.set({obj}, "{key}", {encode(comp, comp.proj("r", types, idx), pt, lines)})'
        body = obj
    text = [f"def case.{wrapper}(+input: J.Val) -> J.Val:"] + ["  " + l for l in lines] + ["  " + body]
    return "\n".join(text) + "\n"


def encode(comp, text, t, lines):
    """A bend value of type t as a J value (tuples as arrays)."""
    return comp.to_j(text, t, lines)


# ---------------------------------------------------------------- files

class Source:
    def __init__(self, reg):
        self.reg = reg
        self.fns = []
        self.cases = []
        self.consts_out = []

    def parse(self, path):
        text = pathlib.Path(path).read_text()
        lines = logical_lines(text)
        module = None
        uses = {}
        i = 0
        while i < len(lines):
            ind, t, no = lines[i]
            try:
                if ind != 0:
                    raise Err("unexpected indent")
                if t.startswith("module "):
                    module = t[7:].strip()
                    i += 1
                elif t.startswith("use "):
                    for u in t[4:].split():
                        self.reg.short_consts.setdefault(u.split(".")[-1], [])
                        uses[u.split(".")[-1]] = u
                    i += 1
                elif t.startswith("const "):
                    m = re.fullmatch(r"const (\w+): ([^=]+)=(.*)", t)
                    if not m:
                        raise Err("bad const")
                    name, ty, val = m.group(1), m.group(2).strip(), m.group(3).strip()
                    rt = self.reg.type_of(ty)
                    v = self.const_eval(val, module)
                    full = f"{module}.{name}"
                    self.reg.consts[full] = (rt, v)
                    self.reg.short_consts.setdefault(name, []).append(full)
                    self.consts_out.append(f"def {full}() -> {bt(rt)}:\n  {v & MASK}\n")
                    i += 1
                elif t.startswith("case "):
                    self.cases.append((t[5:].strip(), module, no))
                    i += 1
                elif t.startswith("enum "):
                    m = re.fullmatch(r"enum (\w+):(.*)", t)
                    self.reg.enums[m.group(1)] = m.group(2).split()
                    i += 1
                elif t.startswith("struct "):
                    name = t[7:].rstrip(":").strip()
                    fields = {}
                    i += 1
                    while i < len(lines) and lines[i][0] > 0:
                        f, ty = lines[i][1].split(":", 1)
                        fields[f.strip()] = ("deferred", ty.strip())
                        i += 1
                    self.reg.structs[name] = fields
                elif t.startswith("fn ") or t.startswith("extern fn "):
                    i = self.parse_fn(lines, i, module)
                else:
                    raise Err(f"unknown top-level form {t!r}")
            except Err as e:
                raise Err(f"{path}:{no}: {e}")

    def const_eval(self, s, module):
        e = parse_expr(s)

        def ev(e):
            k = e[0]
            if k == "num":
                return e[1]
            if k == "paren":
                return ev(e[1])
            if k == "un" and e[1] == "-":
                return -ev(e[2])
            if k == "bin":
                a, b = ev(e[2]), ev(e[3])
                return {"+": a + b, "-": a - b, "*": a * b, "/": int(a / b) if b else 0, "%": a % b if b else 0,
                        "<<": a << b, ">>": a >> b}[e[1]]
            if k in ("var", "path"):
                name = e[1]
                for c in (f"{module}.{name}", f"bannerpeak.{name}", name):
                    if c in self.reg.consts:
                        return self.reg.consts[c][1]
                for full in self.reg.short_consts.get(name.split(".")[-1], []):
                    return self.reg.consts[full][1]
            raise Err(f"const expression {s!r}")
        return ev(e)

    def parse_fn(self, lines, i, module):
        ind, t, no = lines[i]
        extern = t.startswith("extern ")
        if extern:
            t = t[7:]
        t = t[3:]
        j = t.index("(")
        name = t[:j].strip()
        depth, k = 0, j
        while True:
            if t[k] == "(":
                depth += 1
            elif t[k] == ")":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        params = t[j + 1:k]
        tail = t[k + 1:].strip()
        ret, ext = None, None
        if extern and "=" in tail:
            tail, ext = tail.rsplit("=", 1)
            tail, ext = tail.strip(), ext.strip()
        if not extern:
            if not tail.endswith(":"):
                raise Err(f"bad fn header {t!r}")
            tail = tail[:-1].strip()
        if tail.startswith("->"):
            ret = tail[2:].strip()
        elif tail:
            raise Err(f"bad fn header {t!r}")
        bend = ext if ext else f"{module}.{name}"
        ps = []
        for p in split_top(params):
            pm = re.fullmatch(r"(\w+):\s*(.+?)(\s+mut)?", p)
            if not pm:
                raise Err(f"bad parameter {p!r}")
            ps.append((pm.group(1), pm.group(2), bool(pm.group(3))))
        fn = Fn(name, ps, ret or "unit", None, extern, no)
        fn.module, fn.bend = module, bend
        i += 1
        if not extern:
            body, i = parse_sub(lines, i, 0)
            fn.body = body
        self.fns.append(fn)
        return i


def resolve_types(reg, fns):
    for name, fields in reg.structs.items():
        for f, ty in list(fields.items()):
            if isinstance(ty, tuple) and ty[0] == "deferred":
                fields[f] = reg.type_of(ty[1])
    for fn in fns:
        ps = [(n, reg.type_of(t), m) for n, t, m in fn.params]
        sig = Sig(fn.bend, ps, reg.type_of(fn.ret))
        reg.fns[fn.bend] = sig
        alias = f"{fn.module}.{fn.name}"
        if alias != fn.bend:
            reg.fns[alias] = sig
        head = fn.name.split(".")[0]
        if "." in fn.name and head in reg.structs:
            reg.methods[(head, fn.name.split(".", 1)[1])] = fn.bend


def main(argv):
    here = pathlib.Path(__file__).resolve().parent
    files = [pathlib.Path(a) for a in argv] or sorted((here / "src").glob("*.bpx"))
    # every source is read first: a call may name a fn declared in a later file
    reg = Reg()
    sources = []
    allfiles = sorted((here / "src").glob("*.bpx"), key=lambda p: (p.stem != "world", p.stem != "layout", p.stem))
    for f in allfiles:
        s = Source(reg)
        s.parse(f)
        sources.append((f, s))
    resolve_types(reg, [fn for _, s in sources for fn in s.fns])
    for f, s in sources:
        if files and f.resolve() not in [x.resolve() for x in files]:
            continue
        comp = Compiler(reg)
        for fn in s.fns:
            if fn.extern:
                continue
            try:
                comp.compile_fn(fn)
            except Err as e:
                raise Err(f"{f.name}: fn {fn.name} (line {fn.line}): {e}")
        for spec, module, no in s.cases:
            try:
                comp.out.append(compile_case(comp, reg, spec, module))
            except Err as e:
                raise Err(f"{f.name}:{no}: case {spec}: {e}")
        if not comp.out and not s.consts_out:
            continue
        out = here / (f.stem + ".bend")
        header = f"# Generated by bpx.py from src/{f.name}. Do not edit; edit the .bpx.\n\n"
        out.write_text(header + "\n".join(s.consts_out) + ("\n" if s.consts_out else "") + "\n".join(comp.out))
        print(f"{out.name}: {len(comp.out)} defs")


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except Err as e:
        print("bpx:", e, file=sys.stderr)
        sys.exit(1)

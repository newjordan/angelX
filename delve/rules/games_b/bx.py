#!/usr/bin/env python3
"""bx: a thin layer over Bend for the world port. `X.bx` -> `X.bend`.

Bend 2 refuses `match` on anything but a parameter or a pattern-bound name, so
every Rust `if`, `match`, and tuple `let` over a computed value becomes its own
def that takes the live locals as parameters. bx writes those defs. Everything
that is not a `def` passes through untouched. Inside a bx `def` the statements:

    name: Type = expr                      a let (always reusable: `+name = expr`)
    (a: A, b: B) = expr                    a pair destructure of a computed value
    K{a: A, b: B} of T = expr              a constructor destructure of a computed value
    name: Type = if cond:                  a value-producing if (then-block, `else:`)
    name: Type = match e as E:             a value-producing match
    if cond:  ... else: ...                last statement: branches, each a block
    match e as E:                          last statement; `match e as E, f as F:`
      case Pat:                            pattern binders are `x` or `x: Type`
    expr                                   last statement: the result

Every block ends in an expression. Binders that a later lifted def needs must
carry a type (a let, a parameter, or `x: Type` in a pattern). Run:
    python3 delve/rules/world/bx.py [file.bx ...]
"""
import pathlib
import re
import sys


def strip_lits(s):
    s = re.sub(r'"(?:[^"\\]|\\.)*"', '""', s)
    return re.sub(r"'(?:[^'\\]|\\.|)'", "''", s)


def idents(s):
    """Plain (undotted) names used in `s`, outside literals and field paths."""
    s = strip_lits(s)
    out = set()
    for m in re.finditer(r"(?<![A-Za-z0-9_.])[A-Za-z_][A-Za-z0-9_]*(?![A-Za-z0-9_]*[.{])", s):
        out.add(m.group(0))
    return out


def split_top(s, sep=","):
    out, depth, cur, i = [], 0, "", 0
    while i < len(s):
        ch = s[i]
        if s[i:i + 2] == "->":
            cur += "->"
            i += 2
            continue
        if ch in "([{<":
            depth += 1
        elif ch in ")]}>":
            depth -= 1
        if ch == sep and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
        i += 1
    if cur.strip():
        out.append(cur.strip())
    return out


def split_ws_top(s):
    out, depth, cur = [], 0, ""
    i = 0
    while i < len(s):
        ch = s[i]
        if ch == "'" and i + 2 < len(s):
            j = s.index("'", i + 1)
            if s[i + 1] == "\\":
                j = s.index("'", i + 3)
            cur += s[i:j + 1]
            i = j + 1
            continue
        if ch in "([{<":
            depth += 1
        elif ch in ")]}>":
            depth -= 1
        if ch == " " and depth == 0:
            if cur:
                out.append(cur)
            cur = ""
        else:
            cur += ch
        i += 1
    if cur:
        out.append(cur)
    return out


class Err(Exception):
    pass


# ---- patterns ---------------------------------------------------------------

def parse_pat(s, i=0):
    """Returns (bend text, [(name, type|None)], next index)."""
    while i < len(s) and s[i] == " ":
        i += 1
    binders = []
    if s[i] == "(":
        items, i = [], i + 1
        while True:
            t, b, i = parse_pat(s, i)
            items.append(t)
            binders += b
            while s[i] == " ":
                i += 1
            if s[i] == ",":
                i += 1
                continue
            if s[i] == ")":
                i += 1
                break
            raise Err(f"bad tuple pattern {s!r} at {i}")
        return "(" + ", ".join(items) + ")", binders, i
    if s[i] == "'":
        j = s.index("'", i + 1)
        if s[i + 1] == "\\":
            j = s.index("'", i + 3)
        return s[i:j + 1], binders, j + 1
    if s[i].isdigit():
        m = re.match(r"[0-9]+n?(\+[a-z_][A-Za-z0-9_]*)?", s[i:])
        return m.group(0), binders, i + len(m.group(0))
    m = re.match(r"[A-Za-z_][A-Za-z0-9_.]*", s[i:])
    if not m:
        raise Err(f"bad pattern {s!r} at {i}")
    word = m.group(0)
    i += len(word)
    if word[0].isupper():
        if i < len(s) and s[i] == "{":
            i += 1
            items = []
            while True:
                while s[i] == " ":
                    i += 1
                if s[i] == "}":
                    i += 1
                    break
                t, b, i = parse_pat(s, i)
                items.append(t)
                binders += b
                while s[i] == " ":
                    i += 1
                if s[i] == ",":
                    i += 1
            return word + "{" + ", ".join(items) + "}", binders, i
        return word, binders, i
    if word == "_":
        return "_", binders, i
    j = i
    while j < len(s) and s[j] == " ":
        j += 1
    ty = None
    if j < len(s) and s[j] == ":":
        j += 1
        depth, k = 0, j
        while k < len(s):
            if s[k:k + 2] == "->":
                k += 2
                continue
            ch = s[k]
            if ch in "([{<":
                depth += 1
            elif ch in ")]}>":
                if depth == 0:
                    break
                depth -= 1
            elif ch == "," and depth == 0:
                break
            k += 1
        ty = s[j:k].strip()
        i = k
    binders.append((word, ty))
    return "+" + word, binders, i


def parse_case_pats(text):
    """`P1 P2` -> bend text, binders."""
    out, binders = [], []
    for part in split_ws_top(text):
        t, b, i = parse_pat(part, 0)
        if part[i:].strip():
            raise Err(f"trailing text in pattern {part!r}")
        out.append(t)
        binders += b
    return " ".join(out), binders


# ---- reading blocks -----------------------------------------------------------

def logical_lines(body_lines):
    """Join continuation lines (open brackets); drop blanks and comments."""
    out, buf, ind = [], "", None
    for raw in body_lines:
        if not raw.strip() or (buf == "" and raw.strip().startswith("#")):
            continue
        if buf == "":
            ind = len(raw) - len(raw.lstrip())
            buf = raw.strip()
        else:
            buf += " " + raw.strip()
        s = strip_lits(buf)
        depth = sum(s.count(c) for c in "([{") - sum(s.count(c) for c in ")]}")
        if depth <= 0:
            out.append((ind, buf))
            buf = ""
    if buf:
        raise Err("unbalanced brackets near: " + buf[:80])
    return out


LET = re.compile(r"^(?P<lhs>.+?) = (?P<rhs>.+)$")


def parse_block(lines, i, indent):
    """Statements at exactly `indent`; returns (stmts, next i)."""
    stmts = []
    while i < len(lines) and lines[i][0] >= indent:
        ind, text = lines[i]
        if ind != indent:
            raise Err(f"unexpected indent: {text!r}")
        if text == "else:" or text.startswith("case "):
            break
        i += 1
        m = re.match(r"^if (?P<c>.+):$", text)
        if m:
            s, i = parse_if(lines, i, indent, m.group("c"))
            stmts.append(s)
            continue
        m = re.match(r"^match (?P<h>.+):$", text)
        if m:
            s, i = parse_match(lines, i, indent, m.group("h"))
            stmts.append(s)
            continue
        m = LET.match(text)
        if m:
            lhs, rhs = m.group("lhs"), m.group("rhs")
            inner = None
            mi = re.match(r"^if (?P<c>.+):$", rhs)
            mm = re.match(r"^match (?P<h>.+):$", rhs)
            if mi:
                inner, i = parse_if(lines, i, indent, mi.group("c"))
            elif mm:
                inner, i = parse_match(lines, i, indent, mm.group("h"))
            if lhs.startswith("("):
                _, binders, _ = parse_pat(lhs, 0)
                if any(t is None for _, t in binders):
                    raise Err(f"pair binders need types: {text!r}")
                stmts.append({"k": "plet", "binders": binders, "rhs": rhs, "inner": inner})
                continue
            mo = re.match(r"^(?P<p>.+?) of (?P<t>.+)$", lhs)
            if mo:
                stmts.append({"k": "clet", "pat": mo.group("p"), "ty": mo.group("t"), "rhs": rhs, "inner": inner})
                continue
            mn = re.match(r"^(?P<n>[a-z_][A-Za-z0-9_]*): (?P<t>.+)$", lhs)
            if not mn:
                raise Err(f"let needs `name: Type`: {text!r}")
            if inner is not None:
                stmts.append({"k": "blet", "name": mn.group("n"), "ty": mn.group("t"), "inner": inner})
            else:
                stmts.append({"k": "let", "name": mn.group("n"), "ty": mn.group("t"), "rhs": rhs})
            continue
        stmts.append({"k": "expr", "text": text})
    return stmts, i


def parse_if(lines, i, indent, cond):
    if i >= len(lines) or lines[i][0] <= indent:
        raise Err(f"empty if {cond!r}")
    then, i = parse_block(lines, i, lines[i][0])
    els = None
    if i < len(lines) and lines[i][0] == indent and lines[i][1] == "else:":
        i += 1
        els, i = parse_block(lines, i, lines[i][0])
    if els is None:
        raise Err(f"if without else: {cond!r}")
    return {"k": "if", "cond": cond, "then": then, "else": els}, i


def parse_match(lines, i, indent, head):
    scr = []
    for part in split_top(head):
        m = re.match(r"^(?P<e>.+) as (?P<t>.+)$", part)
        if m:
            scr.append((m.group("e"), m.group("t")))
        elif re.match(r"^[a-z_][A-Za-z0-9_]*$", part):
            scr.append((part, None))
        else:
            raise Err(f"match needs `e as Type` unless it is a plain name: {head!r}")
    cases = []
    while i < len(lines) and lines[i][0] > indent and lines[i][1].startswith("case "):
        cind, text = lines[i]
        i += 1
        pat = text[len("case "):]
        if not pat.endswith(":"):
            raise Err(f"case needs a colon: {text!r}")
        body, i = parse_block(lines, i, lines[i][0])
        cases.append((pat[:-1], body))
    return {"k": "match", "scr": scr, "cases": cases}, i


# ---- emitting -------------------------------------------------------------------

def stmts_text(stmts):
    out = []
    for s in stmts:
        k = s["k"]
        if k == "expr":
            out.append(s["text"])
        elif k == "let":
            out.append(s["rhs"])
        elif k in ("plet", "clet"):
            out.append(s["rhs"])
            if s.get("inner"):
                out.append(stmts_text([s["inner"]]))
        elif k == "blet":
            out.append(stmts_text([s["inner"]]))
        elif k == "if":
            out += [s["cond"], stmts_text(s["then"]), stmts_text(s["else"])]
        elif k == "match":
            for e, _ in s["scr"]:
                out.append(e)
            for _, body in s["cases"]:
                out.append(stmts_text(body))
    return "\n".join(out)


class Gen:
    def __init__(self, name):
        self.name = name
        self.n = 0
        self.helpers = []

    def fresh(self):
        self.n += 1
        return f"{self.name}.k{self.n}"

    def fv(self, text, gamma, exclude=()):
        used = idents(text)
        return [v for v in gamma if v in used and v not in exclude]

    def sig(self, names, gamma):
        out = []
        for v in names:
            if gamma[v] is None:
                raise Err(f"local {v} needs a type to cross a lifted def")
            out.append(f"+{v}: {gamma[v]}")
        return out

    def emit(self, stmts, gamma, ret, top=False):
        lines = []
        gamma = dict(gamma)
        for idx, s in enumerate(stmts):
            rest = stmts[idx + 1:]
            k = s["k"]
            if k == "expr":
                if rest:
                    raise Err(f"statements after the result: {s['text']!r}")
                lines.append(s["text"])
                return lines
            if k == "let":
                lines.append(f"+{s['name']} = {s['rhs']}")
                gamma[s["name"]] = s["ty"]
            elif k == "blet":
                call = self.lift(s["inner"], gamma, s["ty"])
                lines.append(f"+{s['name']} = {call}")
                gamma[s["name"]] = s["ty"]
            elif k in ("plet", "clet"):
                if not rest:
                    raise Err("let with no body")
                if k == "plet":
                    binders = s["binders"]
                    if len(binders) != 2:
                        raise Err("pair destructure takes exactly two binders")
                    pty = " & ".join(t for _, t in binders)
                    pat = "(" + ", ".join("+" + n for n, _ in binders) + ")"
                else:
                    pat, binders, _ = parse_pat(s["pat"], 0)
                    pty = s["ty"]
                if s.get("inner"):
                    rhs = self.lift(s["inner"], gamma, pty)
                else:
                    rhs = s["rhs"]
                h = self.fresh()
                bound = [n for n, _ in binders]
                names = self.fv(stmts_text(rest), gamma, exclude=bound)
                if "pr" in gamma:
                    raise Err("local named pr")
                inner_gamma = {v: gamma[v] for v in names}
                for n, t in binders:
                    inner_gamma[n] = t
                body = self.emit(rest, inner_gamma, ret)
                params = [f"pr: {pty}"] + self.sig(names, gamma)
                head = f"def {h}({', '.join(params)}) -> {ret}:"
                if k == "plet":
                    inner = [f"{pat} = pr"] + body
                else:
                    inner = ["match pr:", f"  case {pat}:"] + ["    " + b for b in body]
                self.helpers.append([head] + ["  " + b for b in inner])
                lines.append(f"{h}({', '.join([rhs] + names)})")
                return lines
            elif k in ("if", "match"):
                if rest:
                    raise Err(f"statements after a tail {k}")
                if k == "match" and idx == 0 and top and all(t is None and e in gamma for e, t in s["scr"]):
                    lines += self.match_lines(s, gamma, ret, [e for e, _ in s["scr"]])
                    return lines
                lines.append(self.lift(s, gamma, ret))
                return lines
        raise Err("block has no result expression")

    def match_lines(self, s, gamma, ret, heads):
        body = [f"match {' '.join(heads)}:"]
        for pat, cbody in s["cases"]:
            ptxt, binders = parse_case_pats(pat)
            inner_gamma = dict(gamma)
            for n, t in binders:
                inner_gamma[n] = t
            lines = self.emit(cbody, inner_gamma, ret)
            body.append(f"  case {ptxt}:")
            body += ["    " + b for b in lines]
        return body

    def lift(self, s, gamma, ret):
        """A branching statement as its own def; returns the call text."""
        h = self.fresh()
        text = stmts_text([s])
        if s["k"] == "if":
            names = self.fv(text, gamma)
            if "cv" in gamma:
                raise Err("local named cv")
            inner_gamma = {v: gamma[v] for v in names}
            then = self.emit(s["then"], inner_gamma, ret)
            els = self.emit(s["else"], inner_gamma, ret)
            body = ["match cv:", "  case True{}:"] + ["    " + b for b in then] + ["  case False{}:"] + ["    " + b for b in els]
            params = ["cv: Bool"] + self.sig(names, gamma)
            self.helpers.append([f"def {h}({', '.join(params)}) -> {ret}:"] + ["  " + b for b in body])
            return f"{h}({', '.join([s['cond']] + names)})"
        scr = [(e, t if t is not None else gamma.get(e)) for e, t in s["scr"]]
        for e, t in scr:
            if t is None:
                raise Err(f"match on {e}: give its type with `as`")
        sv = [f"sv{j}" for j in range(len(scr))]
        for v in sv:
            if v in gamma:
                raise Err(f"local named {v}")
        names = self.fv(text, gamma)
        inner_gamma0 = {v: gamma[v] for v in names}
        body = self.match_lines(s, inner_gamma0, ret, sv)
        params = [f"{sv[j]}: {scr[j][1]}" for j in range(len(scr))] + self.sig(names, gamma)
        self.helpers.append([f"def {h}({', '.join(params)}) -> {ret}:"] + ["  " + b for b in body])
        return f"{h}({', '.join([e for e, _ in scr] + names)})"


def compile_def(header, body_lines):
    m = re.match(r"^def\s+(?P<name>[^\s(]+)\((?P<params>.*)\)\s*->\s*(?P<ret>.+):$", header)
    if not m:
        raise Err(f"bad def header {header!r}")
    name, ret = m.group("name"), m.group("ret").strip()
    gamma = {}
    for p in split_top(m.group("params")):
        pm = re.match(r"^[+-]?(?P<n>[a-z_][A-Za-z0-9_]*)\s*:\s*(?P<t>.+)$", p)
        if pm:
            gamma[pm.group("n")] = pm.group("t").strip()
    lines = logical_lines(body_lines)
    if not lines:
        raise Err("empty body")
    stmts, i = parse_block(lines, 0, lines[0][0])
    if i != len(lines):
        raise Err(f"could not parse near {lines[i][1]!r}")
    g = Gen(name)
    out = g.emit(stmts, gamma, ret, top=True)
    res = [header] + ["  " + b for b in out]
    for h in g.helpers:
        res.append("")
        res += h
    return "\n".join(res) + "\n"


def compile_text(text):
    out, lines, i = [], text.split("\n"), 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("def ") and line.rstrip().endswith(":"):
            j = i + 1
            while j < len(lines) and (lines[j].startswith(" ") or not lines[j].strip()):
                j += 1
            body = lines[i + 1:j]
            while body and not body[-1].strip():
                body.pop()
            try:
                out.append(compile_def(line.rstrip(), body))
            except Err as e:
                raise Err(f"{line.strip()}: {e}")
            i = j
            continue
        out.append(line + "\n")
        i += 1
    return "".join(out)


def main():
    root = pathlib.Path(__file__).resolve().parent
    files = [pathlib.Path(a) for a in sys.argv[1:]] or sorted(root.rglob("*.bx"))
    bad = 0
    for f in files:
        try:
            text = compile_text(f.read_text())
        except Err as e:
            print(f"{f}: {e}", file=sys.stderr)
            bad += 1
            continue
        f.with_suffix(".bend").write_text(
            f"# generated by bx.py from {f.name}: edit the .bx, not this file\n" + text)
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()

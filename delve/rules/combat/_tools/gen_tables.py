#!/usr/bin/env python3
"""Rust enum tables -> Bend.

    gen_tables.py ENUM_RS_FILE ENUM_NAME TYPE_PREFIX  FN_RS_FILE:fn[:ret][:argname] ...

Writes (stdout) the ADT for ENUM_NAME (snake_case serde names), its `of` and
`name` converters, and, for each listed Rust function whose body is a
`match <arg> { Kind::A | Kind::B => value, ... }` (or a `matches!(...)`), a Bend
def `<module>.<Enum>.<fn>(+k: String) -> RET` that answers the table's value.
RET is guessed from the Rust signature (f32 F32, u32/usize/u8/i32 U32, bool Bool,
&str String).
"""
import re
import sys
import pathlib

SRC = pathlib.Path(__file__).resolve().parents[4] / "crates/delve/src"
CT = {"f32": "F32", "u32": "U32", "usize": "U32", "u8": "U32", "u16": "U32", "i32": "U32", "bool": "Bool", "&str": "String", "&'static str": "String"}


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def enum_variants(path, enum):
    t = (SRC / path).read_text()
    m = re.search(r"pub enum %s\s*\{(.*?)\n\}" % enum, t, re.S)
    body = re.sub(r"///.*", "", m.group(1))
    body = re.sub(r"//.*", "", body)
    return re.findall(r"\b([A-Z][A-Za-z0-9]*)\b\s*(?:,|$)", body.replace("\n", " ").replace(",", ",\n"), re.M)


def split_top(s, sep=","):
    out, d, cur = [], 0, []
    for ch in s:
        if ch in "([{":
            d += 1
        elif ch in ")]}":
            d -= 1
        if ch == sep and d == 0:
            out.append("".join(cur)); cur = []
        else:
            cur.append(ch)
    if "".join(cur).strip():
        out.append("".join(cur))
    return out


def value(v, ret, consts):
    v = v.strip()
    if v in ("true", "false"):
        return "True{}" if v == "true" else "False{}"
    if v.startswith('"'):
        return v
    if v == "u8::MAX":
        return "255"
    if ret == "F32":
        if re.fullmatch(r"-?\d+(\.\d+)?", v):
            return v if "." in v else v + ".0"
        if v.startswith("-") and re.fullmatch(r"-\d+(\.\d+)?", v):
            return "F32.neg(%s)" % v[1:]
    if re.fullmatch(r"\d+", v):
        return v
    # const references and arithmetic
    expr = re.sub(r"\b(?:super|crate::rules)::HZ\b", "rules.HZ()", v)
    expr = re.sub(r"\bHZ\b", "rules.HZ()", expr)
    expr = re.sub(r"\b([a-z_]+)::([A-Z][A-Z_0-9]*)\b", lambda m: "%s.%s()" % (m.group(1), m.group(2)), expr)
    expr = re.sub(r"\b(?<![.\w])([A-Z][A-Z_0-9]{2,})\b(?!\()", lambda m: consts + "." + m.group(1) + "()", expr)
    return "(%s : %s)" % (expr, ret)


def main():
    efile, enum, prefix, ctor = sys.argv[1:5]
    mod = prefix
    variants = enum_variants(efile, enum)
    tname = prefix.capitalize() + enum
    out = []
    out.append("type %s is Data:\n%s" % (tname, "\n".join("  %s.%s{}" % (ctor, v) for v in variants)))
    # of: chain over string equality
    out.append("def %s.%s.of(+s: String) -> %s:\n  %s.%s.of.n0(s)" % (mod, enum, tname, mod, enum))
    for i, v in enumerate(variants):
        nxt = "%s.%s{}" % (ctor, variants[0]) if i + 1 == len(variants) else "%s.%s.of.n%d(s)" % (mod, enum, i + 1)
        out.append("def %s.%s.of.n%d(+s: String) -> %s:\n  %s.%s.of.k%d(String.eq(s, \"%s\"), s)" % (mod, enum, i, tname, mod, enum, i, snake(v)))
        out.append("def %s.%s.of.k%d(hit: Bool, +s: String) -> %s:\n  match hit:\n    case True{}:\n      %s.%s{}\n    case False{}:\n      %s" % (mod, enum, i, tname, ctor, v, nxt))
    out.append("def %s.%s.str(+k: %s) -> String:\n  match k:\n%s" % (mod, enum, tname, "\n".join("    case %s.%s{}:\n      \"%s\"" % (ctor, v, snake(v)) for v in variants)))
    for spec in sys.argv[5:]:
        parts = spec.split(":")
        fpath, fn = parts[0], parts[1]
        text = (SRC / fpath).read_text()
        m = re.search(r"fn %s\(([^)]*)\)\s*->\s*([^{]+)\{" % fn, text)
        if not m:
            raise SystemExit("no fn %s in %s" % (fn, fpath))
        ret = CT[m.group(2).strip()]
        start = m.end()
        d, i = 1, start
        while d:
            d += {"{": 1, "}": -1}.get(text[i], 0)
            i += 1
        body = text[start:i - 1]
        body = re.sub(r"//.*", "", body).strip()
        cases = []
        mm = re.match(r"matches!\(\s*\w+\s*,\s*(.*)\)\s*$", body, re.S)
        if mm:
            pats = [p.strip() for p in mm.group(1).split("|")]
            names = [re.sub(r"^\w+::", "", p) for p in pats]
            cases = [(names, "true")]
            default = "false"
        else:
            mm = re.match(r"match\s+[\w.]+\s*\{(.*)\}\s*$", body, re.S)
            arms = split_top(mm.group(1))
            default = None
            for arm in arms:
                pat, val = arm.split("=>", 1)
                pat = pat.strip()
                if pat == "_":
                    default = val.strip()
                    continue
                names = [re.sub(r"^\w+::", "", p.strip()) for p in pat.split("|")]
                cases.append((names, val.strip()))
        covered = {n for ns, _ in cases for n in ns}
        mod_of = pathlib.Path(fpath).stem
        free = len(parts) > 2 and parts[2] == "free"
        qual = "%s.%s" % (mod_of, fn) if free else "%s.%s.%s" % (mod_of, enum, fn)
        lines = ["def %s.go(k: %s) -> %s:\n  match k:" % (qual, tname, ret)]
        for names, val in cases:
            for n in names:
                lines.append("    case %s.%s{}:\n      %s" % (ctor, n, value(val, ret, mod_of)))
        if default is not None:
            lines.append("    case _:\n      %s" % value(default, ret, mod_of))
        elif covered != set(variants):
            missing = set(variants) - covered
            if "matches" in body:
                lines.append("    case _:\n      %s" % value("false", ret, mod_of))
            else:
                raise SystemExit("%s: arms miss %s" % (fn, sorted(missing)))
        out.append("\n".join(lines))
        out.append("def %s(+k: String) -> %s:\n  %s.go(%s.%s.of(k))" % (qual, ret, qual, mod, enum))
    print("\n\n".join(out))


if __name__ == "__main__":
    main()

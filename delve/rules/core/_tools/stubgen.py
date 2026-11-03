#!/usr/bin/env python3
"""stubgen: Bend stubs from Rust signatures, by PORTING.md's value mapping.

    python3 stubgen.py <rust-src-dir> NAME...        NAME = a `Run` method or `module::fn`

Prints `def <module>.Run.<m>(...)` stubs (harmless defaults) for the named
methods of every `impl Run` in crates/delve/src, found by name.
"""
import re, sys, pathlib

def split_top(s, sep=","):
    out, d, cur = [], 0, ""
    i = 0
    while i < len(s):
        ch = s[i]
        if s[i:i + 2] == "->":
            cur += "->"; i += 2; continue
        if ch in "([{<": d += 1
        elif ch in ")]}>": d -= 1
        if ch == sep and d == 0:
            out.append(cur.strip()); cur = ""
        else:
            cur += ch
        i += 1
    if cur.strip(): out.append(cur.strip())
    return out

INTS = {"u8","u16","u32","usize","i8","i16","i32","isize"}
def bty(t, ret=False):
    t = t.strip()
    t = re.sub(r"^&(?:'\w+\s+)?(?:mut\s+)?", "", t)
    if t in INTS: return "U32"
    if t in ("u64","i64"): return "W.W"
    if t == "f32": return "F32"
    if t == "bool": return "Bool"
    if t in ("str","String"): return "String"
    m = re.match(r"^Option<(.*)>$", t)
    if m:
        inner = bty(m.group(1), ret)
        return "Maybe<&2, %s>" % inner
    if t.startswith("(") and t.endswith(")"):
        parts = split_top(t[1:-1])
        if len(parts) > 1 and all(bty(p) in ("U32","F32","Bool","String","W.W") for p in parts):
            return " & ".join(bty(p) for p in parts) if len(parts) == 2 else parts and bty(parts[0]) + " & (" + " & ".join(bty(p) for p in parts[1:]) + ")"
    return "J.Val"

def default(t):
    return {"U32":"0","F32":"0.0","Bool":"False{}","String":'""',"J.Val":"J.JNull{}","W.W":"W.zero()"}.get(t) or ("None{}" if t.startswith("Maybe") else "J.JNull{}")

def main():
    src = pathlib.Path(sys.argv[1])
    want = set(sys.argv[2:])
    seen = set()
    for f in sorted(src.rglob("*.rs")):
        rel = f.relative_to(src).with_suffix("")
        parts = list(rel.parts)
        if parts[0] == "rules": parts = parts[1:] or ["rules"]
        mod = ".".join(parts)
        text = f.read_text()
        for m in re.finditer(r"^\s*pub(?:\([a-z]+\))?\s+fn\s+(\w+)\s*(?:<[^>]*>)?\s*\(([^{;]*?)\)\s*(?:->\s*([^{;]+?))?\s*(?:where[^{]*)?\{", text, re.M | re.S):
            name = m.group(1)
            if name not in want: continue
            before = text[:m.start()]
            im = list(re.finditer(r"^impl(?:<[^>]*>)?\s+(?:[\w:<>]+\s+for\s+)?(\w+)", before, re.M))
            owner = im[-1].group(1) if im else None
            if owner is None or (mod, owner, name) in seen: continue
            params = split_top(m.group(2))
            ret = (m.group(3) or "").strip()
            if not params: continue
            first = params[0]
            if not re.match(r"^(&\s*(mut\s+)?self|self|mut self)", first): continue
            mut = "mut" in first
            args = ["+%s: J.Val" % ("run" if owner == "Run" else "self")]
            for p in params[1:]:
                pm = re.match(r"^(?:mut\s+)?(\w+)\s*:\s*(.+)$", p, re.S)
                if not pm: continue
                n = re.sub(r"^_", "", pm.group(1)) or "x"
                args.append("+%s: %s" % (n, bty(pm.group(2))))
            r = bty(ret) if ret and ret != "()" else None
            if mut:
                rt = "J.Val" if r is None else "J.Val & %s" % r
                recv = "run" if owner == "Run" else "self"
                body = recv if r is None else "(%s, %s)" % (recv, default(r))
            else:
                rt = r or "J.Val"
                body = default(rt) if r else "J.JNull{}"
            seen.add((mod, owner, name))
            print("def %s.%s.%s(%s) -> %s:\n  %s\n" % (mod, owner, name, ", ".join(args), rt, body))
main()

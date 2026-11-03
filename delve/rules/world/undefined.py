#!/usr/bin/env python3
"""List the dotted names the assembled world.bend calls and nothing defines."""
import re, pathlib
t = pathlib.Path(__file__).resolve().parents[2].joinpath("build/world.bend").read_text()
defs = set(re.findall(r"^def ([^\s(?]+)\??\(", t, re.M)) | set(re.findall(r"^type (\w+)", t, re.M))
cons = set(re.findall(r"^  (\w+)\{", t, re.M))
known = defs | cons
refs = {}
for i, line in enumerate(t.splitlines()):
    if line.startswith("def ") or line.startswith("import") or line.startswith("type "):
        pass
    for m in re.finditer(r"(?<![A-Za-z0-9_.'\"])([a-z_]+(?:\.[A-Za-z_][A-Za-z0-9_]*)+)\(", line):
        n = m.group(1)
        if n not in known and n.split(".")[0] not in ("J", "W", "RS", "B", "D", "U32", "F32", "List", "String", "Maybe", "Bool", "Char", "Map", "Nat", "IO", "File", "Cmp", "Sigma", "Array"):
            refs.setdefault(n, i)
for n in sorted(refs):
    print(n)

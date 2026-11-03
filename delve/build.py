#!/usr/bin/env python3
"""Concatenate Delve's Bend fragments into one program: delve/build/rules.bend.

Fragments (delve/rules/<group>/**/*.bend) hold only types and defs; every def
becomes @unsafe so order and mutual recursion are free inside the one file. A
stub (delve/stubs/*.bend) is kept only while no fragment defines its name.
`case.<name>` wrappers are gathered into `cases.run` for delve/tests/cases.py.

    python3 delve/build.py [--only GROUP ...]
"""
import argparse
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
LIBS = [("j", "J"), ("bj", "B"), ("delta", "D"), ("w64", "W"), ("rs", "RS")]
DEF = re.compile(r"^def\s+([^\s(?]+)(\??)\s*\(", re.M)
TOP = re.compile(r"^(def|type)\s", re.M)


def blocks(text):
    """Top-level `def`/`type` blocks with their leading comments."""
    starts = [m.start() for m in TOP.finditer(text)]
    out = []
    for i, s in enumerate(starts):
        e = starts[i + 1] if i + 1 < len(starts) else len(text)
        out.append(text[s:e].rstrip() + "\n")
    return out


def unsafe(block):
    return DEF.sub(lambda m: f"def {m.group(1)}?(", block, count=1)


def name(block):
    m = DEF.match(block)
    return m.group(1) if m else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*", default=None)
    args = ap.parse_args()
    rules = HERE / "rules"
    groups = sorted(p.name for p in rules.iterdir() if p.is_dir()) if rules.is_dir() else []
    chosen = groups if args.only is None else [g for g in groups if g in set(args.only)]
    defined, out, errors = {}, [], []
    for g in chosen:
        for f in sorted((rules / g).rglob("*.bend")):
            text = f.read_text()
            if re.search(r"^import\s", text, re.M):
                errors.append(f"{f}: fragments have no imports")
                continue
            for b in blocks(text):
                n = name(b)
                if n is not None:
                    if n in defined:
                        errors.append(f"{n}: defined in {defined[n]} and {f}")
                        continue
                    defined[n] = f
                    b = unsafe(b)
                out.append(b)
    stubbed = set()
    for f in sorted((HERE / "stubs").glob("*.bend")) if (HERE / "stubs").is_dir() else []:
        for b in blocks(f.read_text()):
            n = name(b)
            if n is None or n in defined or n in stubbed:
                continue
            stubbed.add(n)
            out.append(unsafe(b))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)
    cases = sorted(n[len("case."):] for n in defined if n.startswith("case."))
    out.append(dispatch(cases))
    head = ["import Base"]
    for lib, alias in LIBS:
        if (HERE / "lib" / f"{lib}.bend").is_file():
            head.append(f"import ../lib/{lib}.bend as {alias}")
    (HERE / "build").mkdir(exist_ok=True)
    (HERE / "build" / "rules.bend").write_text("\n".join(head) + "\n\n" + "\n".join(out))
    print(f"delve/build/rules.bend: {len(defined)} defs from {len(chosen)} groups, "
          f"{len(stubbed)} stubs, {len(cases)} case wrappers")


def dispatch(cases):
    """`cases.run(name, input)`: the wrapper named `case.<name>`, by name."""
    lines = ["def cases.run?(+name: String, +input: J.Val) -> J.Val:",
             "  cases.run.n0(String.eq(name, \"" + (cases[0] if cases else "") + "\"), name, input)", ""]
    for i, c in enumerate(cases):
        nxt = (f"cases.run.n{i + 1}(String.eq(name, \"{cases[i + 1]}\"), name, input)"
               if i + 1 < len(cases) else "J.JS{\"no case wrapper\"}")
        lines += [f"def cases.run.n{i}?(hit: Bool, +name: String, +input: J.Val) -> J.Val:",
                  "  match hit:", "    case True{}:", f"      case.{c}(input)",
                  "    case False{}:", f"      {nxt}", ""]
    if not cases:
        lines += ["def cases.run.n0?(hit: Bool, +name: String, +input: J.Val) -> J.Val:",
                  "  J.JS{\"no case wrapper\"}", ""]
    return "\n".join(lines)


if __name__ == "__main__":
    main()

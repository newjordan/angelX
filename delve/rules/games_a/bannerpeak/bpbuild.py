#!/usr/bin/env python3
"""Private builds for the games_a port: no shared delve/build/ output, so the
other agents' rebuilds never race ours.

    gabuild.py build  SUB [SUB ...]      -> delve/.ga/<first SUB>/rules.bend, bend-checked
    gabuild.py cases  SUB [SUB ...] -- FN [FN ...]   build + run recorded cases for FN

SUB is a directory under delve/rules/games_a/ (kit is always included). Stubs:
delve/stubs/games_a*.bend only. Output dir name = the first SUB.
"""
import gzip, importlib.util, os, pathlib, re, subprocess, sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
BEND = pathlib.Path.home() / ".cache/angel/bend2/221a350c2f7f7134884740018e6e90001fb6d230/bin/bend"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


B = load("dbuild", ROOT / "build.py")
C = load("dcases", ROOT / "tests" / "cases.py")


def build(subs):
    out_dir = ROOT / ".ga" / subs[0]
    out_dir.mkdir(parents=True, exist_ok=True)
    defined, out, errors = {}, [], []
    files = []
    for s in ["kit"] + [x for x in subs if x != "kit"]:
        files += sorted((ROOT / "rules" / "games_a" / s).rglob("*.bend"))
    for f in files:
        text = f.read_text()
        if re.search(r"^import\s", text, re.M):
            errors.append(f"{f}: fragments have no imports")
            continue
        for b in B.blocks(text):
            n = B.name(b)
            if n is not None:
                if n in defined:
                    errors.append(f"{n}: defined in {defined[n]} and {f}")
                    continue
                defined[n] = f
                b = B.unsafe(b)
            out.append(b)
    stubbed = set()
    for f in sorted((ROOT / "stubs").glob("games_a_bannerpeak.bend")):
        for b in B.blocks(f.read_text()):
            n = B.name(b)
            if n is None or n in defined or n in stubbed:
                continue
            stubbed.add(n)
            out.append(B.unsafe(b))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)
    cases = sorted(n[len("case."):] for n in defined if n.startswith("case."))
    out.append(B.dispatch(cases))
    head = ["import Base"]
    for lib, alias in B.LIBS:
        if (ROOT / "lib" / f"{lib}.bend").is_file():
            head.append(f"import ../../lib/{lib}.bend as {alias}")
    (out_dir / "rules.bend").write_text("\n".join(head) + "\n\n" + "\n".join(out))
    main = (ROOT / "tests" / "cases_main.bend").read_text().replace("../build/rules.bend", "./rules.bend")
    main = main.replace("import ../lib/", "import ../../lib/")
    (out_dir / "cases_main.bend").write_text(main)
    print(f"{out_dir}/rules.bend: {len(defined)} defs, {len(stubbed)} stubs, {len(cases)} case wrappers")
    return out_dir, cases


def check(out_dir):
    r = subprocess.run(["timeout", "300", str(BEND), str(out_dir / "rules.bend")], capture_output=True, text=True)
    txt = r.stdout + r.stderr
    # the build marks every def unsafe: that verdict is expected
    keep = [l for l in txt.splitlines() if not l.startswith("defs that depend on @unsafe") and "SOME PROOFS FAIL" not in l
            and not re.match(r"Error: \d+ defs rely on unsafe", l)]
    if keep:
        print("\n".join(l[:500] for l in keep[:60]))
    return not keep


def run_cases(out_dir, names, cases_dir=None, maxn=0):
    cases_dir = cases_dir or os.environ.get("DELVE_CASES")
    if not cases_dir:
        sys.exit("set DELVE_CASES to the Bend case directory")
    exe = out_dir / "cases"
    r = subprocess.run(["timeout", "900", str(BEND), str(out_dir / "cases_main.bend"), "-o", str(exe)],
                       capture_output=True, text=True)
    if r.returncode != 0 or not exe.exists():
        print((r.stdout + r.stderr)[-3000:]); return 1
    bad_total = 0
    for n in names:
        src = None
        for ext in (".jsonl", ".jsonl.gz"):
            p = pathlib.Path(cases_dir) / (n + ext)
            if p.exists():
                src = p
        if src is None:
            print(f"{n}: no cases"); continue
        opener = gzip.open if src.suffix == ".gz" else open
        cs = [C.load(l) for l in opener(src, "rt") if l.strip()]
        if maxn:
            cs = cs[:maxn]
        feed = "".join(C.bj(c) + "\n" for c in cs)
        r = subprocess.run(["timeout", "900", str(exe)], input=feed, capture_output=True, text=True)
        lines = r.stdout.splitlines()
        bad = [(i, l) for i, l in enumerate(lines) if not l.startswith("ok ")]
        if len(lines) != len(cs):
            print(f"{n}: runner answered {len(lines)} of {len(cs)} cases\n{r.stderr[-1500:]}")
            bad_total += 1
            continue
        print(f"{n}: {len(cs) - len(bad)}/{len(cs)} pass")
        for i, l in bad[:3]:
            got = l.split(" ", 2)[2] if l.count(" ") >= 2 else ""
            try:
                print("   case", i, C.first_diff(C.norm(cs[i]["out"]), C.unbj(got)))
            except Exception as e:
                print("   case", i, "unreadable answer:", got[:200], e)
        bad_total += len(bad)
    return 1 if bad_total else 0


def main():
    a = sys.argv[1:]
    if not a or a[0] not in ("build", "cases"):
        print(__doc__); sys.exit(2)
    mode, rest = a[0], a[1:]
    maxn = 0
    if "--max" in rest:
        i = rest.index("--max"); maxn = int(rest[i + 1]); rest = rest[:i] + rest[i + 2:]
    if mode == "build":
        out_dir, _ = build(rest)
        sys.exit(0 if check(out_dir) else 1)
    subs, fns = (rest[:rest.index("--")], rest[rest.index("--") + 1:]) if "--" in rest else (rest, [])
    out_dir, cases = build(subs)
    if not check(out_dir):
        sys.exit(1)
    sys.exit(run_cases(out_dir, fns or cases, maxn=maxn))


main()

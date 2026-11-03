#!/usr/bin/env python3
"""The world group's private build and case run (the shared build writes one
rules.bend all eight agents race on). Compiles .bx, assembles the libs + world
fragments + world stubs into delve/build/world.bend, checks it, and runs cases.

    python3 delve/rules/world/wb.py [--check] [--cases NAME ...]
"""
import argparse, gzip, importlib.util, json, os, pathlib, re, subprocess, sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent
BEND = pathlib.Path.home() / ".cache/angel/bend2/221a350c2f7f7134884740018e6e90001fb6d230/bin/bend"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def unsafe_def(b, blk):
    """A def that computes a type (`-> Data`) stays checkable; the rest is @unsafe."""
    first = blk.split("\n", 1)[0]
    if first.rstrip().endswith("-> Data:") or first.rstrip().endswith("-> Type:"):
        return blk
    return b.unsafe(blk)


def git_files(prefix):
    out = subprocess.run(["git", "ls-tree", "-r", "--name-only", "HEAD", prefix], cwd=ROOT.parent, capture_output=True, text=True).stdout
    return [l for l in out.splitlines() if l.endswith(".bend")]


def git_text(path):
    return subprocess.run(["git", "show", "HEAD:" + path], cwd=ROOT.parent, capture_output=True, text=True).stdout


def assemble():
    if subprocess.run([sys.executable, str(HERE / "bx.py")]).returncode != 0:
        sys.exit(1)
    b = load("build", ROOT / "build.py")
    defined, out, errors, types = {}, [], [], []
    for f in sorted(HERE.rglob("*.bend")):
        text = f.read_text()
        for blk in b.blocks(text):
            n = b.name(blk)
            if n is not None:
                if n in defined:
                    errors.append(f"{n}: defined in {defined[n]} and {f}")
                    continue
                defined[n] = f
                blk = unsafe_def(b, blk)
                # The Rust tests keep the side-on halls on (`cfg!(test)`); the shipped default is off.
                if n in ("layout.wanted_on", "layout.ledge_on"):
                    blk = blk.replace("False{}", "True{}")
            (out if n is not None else types).append(blk)
    # The other groups' committed fragments and stubs, only the names mine reach.
    mine = set(defined)
    pool, stub_pool = {}, {}
    for p in sorted(git_files("delve/rules")):
        if "/world/" in p or "/_tools/" in p:
            continue
        for blk in b.blocks(git_text(p)):
            n = b.name(blk)
            if n is None:
                types.append(blk)
            elif not n.startswith("case.") and n not in mine:
                pool.setdefault(n, blk)
    own_stub = ROOT / "stubs" / "world.bend"
    stub_texts = [own_stub.read_text() if own_stub.is_file() else ""]
    stub_texts += [git_text(p) for p in sorted(git_files("delve/stubs")) if not p.endswith("/world.bend")]
    for t in stub_texts:
        for blk in b.blocks(t):
            n = b.name(blk)
            if n is not None and n not in mine:
                stub_pool.setdefault(n, blk)
    tok = re.compile(r"[A-Za-z_][A-Za-z0-9_.]*")
    need, seen, stubbed = [], set(), set()
    def refs(text):
        for m in tok.finditer(text):
            n = m.group(0)
            if n not in seen and (n in pool or n in stub_pool):
                seen.add(n)
                need.append(n)
    # Foreign defs that do not build against this group's API yet: use this group's stub.
    for n in [n for n in pool if n.startswith(("colosseum.", "nooks."))]:
        pool.pop(n, None)
    for blk in out + types:
        refs(blk)
    while need:
        n = need.pop()
        if n in pool:
            blk = pool[n]
        else:
            blk = stub_pool[n]
            stubbed.add(n)
        defined[n] = "other"
        ub = unsafe_def(b, blk)
        (types if ub is blk else out).append(ub)
        refs(blk)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)
    cases = sorted(n[len("case."):] for n in defined if n.startswith("case."))
    out.append(b.dispatch(cases))
    out = types + out
    head = ["import Base"]
    for lib, alias in b.LIBS:
        if (ROOT / "lib" / f"{lib}.bend").is_file():
            head.append(f"import ../lib/{lib}.bend as {alias}")
    (ROOT / "build").mkdir(exist_ok=True)
    (ROOT / "build" / "world.bend").write_text("\n".join(head) + "\n\n" + "\n".join(out))
    print(f"delve/build/world.bend: {len(defined)} defs, {len(stubbed)} stubs, {len(cases)} case wrappers")
    return cases


def check():
    r = subprocess.run(["timeout", "300", str(BEND), str(ROOT / "build" / "world.bend")], capture_output=True, text=True)
    text = (r.stdout + r.stderr)
    lines = [l for l in text.splitlines() if not l.startswith("defs that depend") and "SOME PROOFS" not in l and "rely on unsafe" not in l]
    print("\n".join(lines[:40]))
    return not any(l.startswith(("Error", "- ")) for l in lines)


MAIN = '''import Base
import ../../../lib/j.bend as J
import ../../../lib/bj.bend as B
import ../../../build/world.bend as R
'''


def run_cases(names, cases_dir, limit):
    tests = load("cases", ROOT / "tests" / "cases.py")
    src = (ROOT / "tests" / "cases_main.bend").read_text().replace("../build/rules.bend", "./world.bend")
    (ROOT / "build" / "cases_main_world.bend").write_text(src)
    exe = ROOT / "build" / "cases_world"
    r = subprocess.run(["timeout", "900", str(BEND), str(ROOT / "build" / "cases_main_world.bend"), "-o", str(exe)], capture_output=True, text=True)
    if r.returncode != 0 or not exe.exists():
        print(r.stdout + r.stderr); sys.exit(1)
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
        cs = [tests.load(l) for l in opener(src, "rt") if l.strip()]
        if limit:
            cs = cs[:limit]
        feed = "".join(tests.bj(c) + "\n" for c in cs)
        r = subprocess.run(["timeout", "900", str(exe)], input=feed, capture_output=True, text=True)
        lines = r.stdout.splitlines()
        bad = [(i, l) for i, l in enumerate(lines) if not l.startswith("ok ")]
        if len(lines) != len(cs):
            print(f"{n}: runner answered {len(lines)} of {len(cs)}\n{r.stderr[-1500:]}")
            bad_total += 1
            continue
        print(f"{n}: {len(cs) - len(bad)}/{len(cs)} pass")
        for i, l in bad[:3]:
            got = l.split(" ", 2)[2] if l.count(" ") >= 2 else ""
            try:
                print("   case", i, tests.first_diff(tests.norm(cs[i]["out"]), tests.unbj(got)))
            except Exception as e:
                print("   case", i, "unreadable answer:", got[:200], e)
        bad_total += len(bad)
    return bad_total


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cases-dir", default=os.environ.get("DELVE_CASES"))
    ap.add_argument("--max", type=int, default=0)
    ap.add_argument("names", nargs="*")
    a = ap.parse_args()
    if not a.cases_dir:
        ap.error("set DELVE_CASES or pass --cases-dir")
    cases = assemble()
    ok = check()
    if not ok:
        sys.exit(1)
    if a.names:
        names = cases if a.names == ["all"] else a.names
        sys.exit(1 if run_cases(names, a.cases_dir, a.max) else 0)


if __name__ == "__main__":
    main()

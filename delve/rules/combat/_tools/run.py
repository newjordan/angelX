#!/usr/bin/env python3
"""Run recorded cases through the combat group alone.

    python3 delve/rules/combat/_tools/run.py [--max N] [--cases DIR] NAME...   (NAME = a case wrapper name, or `all`)

Builds delve/build/combat.bend (mk.py), compiles the case runner against it and
prints pass/fail per name like delve/tests/cases.py.
"""
import argparse
import gzip
import importlib.util
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location("dcases", ROOT / "delve" / "tests" / "cases.py")
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

ap = argparse.ArgumentParser()
ap.add_argument("--cases", default=os.environ.get("DELVE_CASES"))
ap.add_argument("--max", type=int, default=0)
ap.add_argument("--keep", action="store_true")
ap.add_argument("names", nargs="*")
a = ap.parse_args()
if not a.cases:
    ap.error("set DELVE_CASES or pass --cases")

subprocess.run([sys.executable, str(pathlib.Path(__file__).with_name("mk.py"))], check=True)
text = (ROOT / "delve" / "tests" / "cases_main.bend").read_text().replace("../build/rules.bend", "./combat.bend").replace("../lib/", "../lib/")
main = ROOT / "delve" / "build" / "combat_main.bend"
main.write_text(text)
exe = ROOT / "delve" / "build" / "combat_cases"
if exe.exists():
    exe.unlink()
r = subprocess.run(["timeout", "900", str(c.BEND), str(main), "-o", str(exe)], capture_output=True, text=True)
if r.returncode != 0 or not exe.exists():
    print(r.stdout + r.stderr)
    sys.exit(1)
names = a.names
if not names or names == ["all"]:
    names = []
    for f in sorted((ROOT / "delve" / "rules" / "combat").glob("*.bend")):
        names += re.findall(r"^def\s+case\.([^\s(?]+)", f.read_text(), re.M)
bad_total = 0
for n in names:
    src = None
    for ext in (".jsonl", ".jsonl.gz"):
        p = pathlib.Path(a.cases) / (n + ext)
        if p.exists():
            src = p
    if src is None:
        print("%s: no cases" % n)
        continue
    opener = gzip.open if src.suffix == ".gz" else open
    cases = [c.load(l) for l in opener(src, "rt") if l.strip()]
    if a.max:
        cases = cases[: a.max]
    feed = "".join(c.bj(x) + "\n" for x in cases)
    r = subprocess.run(["timeout", "900", str(exe)], input=feed, capture_output=True, text=True)
    lines = r.stdout.splitlines()
    if len(lines) != len(cases):
        print("%s: runner answered %d of %d cases\n%s" % (n, len(lines), len(cases), r.stderr[-1500:]))
        bad_total += 1
        continue
    bad = [(i, l) for i, l in enumerate(lines) if not l.startswith("ok ")]
    print("%s: %d/%d pass" % (n, len(cases) - len(bad), len(cases)))
    for i, l in bad[:3]:
        got = l.split(" ", 2)[2] if l.count(" ") >= 2 else ""
        try:
            print("   case", i, c.first_diff(c.norm(cases[i]["out"]), c.unbj(got)))
        except Exception as e:
            print("   case", i, "unreadable answer:", got[:200], e)
    bad_total += len(bad)
sys.exit(1 if bad_total else 0)

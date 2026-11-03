#!/usr/bin/env python3
"""Local build for the combat group: its fragments and its own stubs only
(other groups' half-written stubs never block a check).

    python3 delve/rules/combat/_tools/mk.py [out.bend]   -> delve/build/combat.bend
"""
import importlib.util
import re
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location("dbuild", ROOT / "delve" / "build.py")
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)

subprocess.run([sys.executable, str(pathlib.Path(__file__).with_name("bendx.py"))], check=True, stdout=subprocess.DEVNULL)
out, defined, errors = [], {}, []
import os
def git_text(path):
    return subprocess.run(["git", "show", "HEAD:" + path], cwd=ROOT, capture_output=True, text=True).stdout

def git_files(prefix):
    r = subprocess.run(["git", "ls-tree", "-r", "HEAD", "--name-only", prefix], cwd=ROOT, capture_output=True, text=True)
    return sorted(x for x in r.stdout.split() if x.endswith(".bend"))

sources = []   # (label, text)
for f in sorted((ROOT / "delve" / "rules" / "combat").glob("*.bend")):
    sources.append((str(f.relative_to(ROOT)), f.read_text()))
for g in os.environ.get("MK_WITH", "").split():
    for f in git_files("delve/rules/" + g):
        sources.append((f, git_text(f)))
for extra in os.environ.get("MK_FILES", "delve/rules/world/00_types.bend delve/rules/world/tiles.bend delve/rules/world/room.bend delve/rules/core/consts.bend delve/rules/core/grid.bend delve/rules/core/desperate.bend delve/rules/core/u.bend").split():
    sources.append((extra, git_text(extra)))
for label, text in sources:
    for blk in b.blocks(text):
        n = b.name(blk)
        if n is not None:
            if n in defined:
                errors.append("%s: defined in %s and %s" % (n, defined[n], label))
                continue
            defined[n] = label
            blk = b.unsafe(blk)
        out.append(blk)
DROP = set(os.environ.get('MK_DROP', 'altar.Run.wish_altar').split())
want = os.environ.get("MK_STUBS", "").split()
stub_texts = [("combat", (ROOT / "delve" / "stubs" / "combat.bend").read_text())] if (ROOT / "delve" / "stubs" / "combat.bend").exists() else []
for g in want:
    t = git_text("delve/stubs/%s.bend" % g)
    t = re.sub(r'Maybe<&2, ([\w.]+) & ([\w.]+)>', r'Maybe<&2, Sigma<&2, &2, \1, _ => \2>>', t)
    if t:
        stub_texts.append((g, t))
for g, text in stub_texts:
    for blk in b.blocks(text):
        n = b.name(blk)
        if n is None or n in defined or n in DROP:
            continue
        defined[n] = "stub"
        out.append(b.unsafe(blk))
cases = sorted(n[len("case."):] for n in defined if n.startswith("case.") and defined[n] != "stub")
out.append(b.dispatch(cases))
head = ["import Base"]
for lib, alias in b.LIBS:
    if (ROOT / "delve" / "lib" / (lib + ".bend")).is_file():
        head.append("import ../lib/%s.bend as %s" % (lib, alias))
dest = ROOT / "delve" / "build" / "combat.bend"
dest.parent.mkdir(exist_ok=True)
dest.write_text("\n".join(head) + "\n\n" + "\n".join(out))
if errors:
    print("\n".join(errors), file=sys.stderr)
    sys.exit(1)
print("delve/build/combat.bend: %d defs, %d case wrappers" % (len(defined), len(cases)))

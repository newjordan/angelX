#!/usr/bin/env python3
"""Run recorded Rust cases through the Bend port.

    python3 delve/tests/cases.py [--only GROUP ...] [--cases DIR] NAME|GROUP ...

NAME is a Bend def name with recorded cases (DIR/<name>.jsonl[.gz]); GROUP
expands to every `case.<name>` wrapper in delve/rules/GROUP. Builds the program
(delve/build.py, `--only` passed through), compiles the case runner natively,
and prints pass/fail per name with the first differing path of each failure.
"""
import argparse
import gzip
import json
import os
import pathlib
import re
import struct
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent.parent
BEND = pathlib.Path.home() / ".cache/angel/bend2/221a350c2f7f7134884740018e6e90001fb6d230/bin/bend"


class Float(str):
    pass


def bj(v):
    """A JSON value (floats kept as their text) as BJ1."""
    if v is None:
        return "null"
    if v is True:
        return "true"
    if v is False:
        return "false"
    if isinstance(v, Float):
        return "f%08x" % struct.unpack("<I", struct.pack("<f", float(v)))[0]
    if isinstance(v, int):
        if v >= 1 << 32:
            return "w%016x" % v
        return str(v)
    if isinstance(v, str):
        return json.dumps(v, ensure_ascii=False)
    if isinstance(v, list):
        return "[" + ",".join(bj(x) for x in v) + "]"
    return "{" + ",".join(json.dumps(k, ensure_ascii=False) + ":" + bj(x) for k, x in v.items()) + "}"


def load(text):
    return json.loads(text, parse_float=Float)


TOKEN = re.compile(r'"(?:[^"\\]|\\.)*"|f[0-9a-f]{8}(?![a-z])|w[0-9a-f]{16}|-?\d+|true|false|null|[\[\]{},:]')


def unbj(text):
    """BJ1 back to Python values: floats as f32 bit patterns tagged ('f', bits)."""
    toks = TOKEN.findall(text)
    pos = 0

    def val():
        nonlocal pos
        t = toks[pos]; pos += 1
        if t == "{":
            out = {}
            if toks[pos] == "}":
                pos += 1; return out
            while True:
                k = json.loads(toks[pos]); pos += 2
                out[k] = val()
                t = toks[pos]; pos += 1
                if t == "}":
                    return out
        if t == "[":
            out = []
            if toks[pos] == "]":
                pos += 1; return out
            while True:
                out.append(val())
                t = toks[pos]; pos += 1
                if t == "]":
                    return out
        if t[0] == '"':
            return json.loads(t)
        if t[0] == "f" and len(t) == 9:
            return ("f", int(t[1:], 16))
        if t[0] == "w":
            return int(t[1:], 16)
        return {"true": True, "false": False, "null": None}.get(t, int(t) if t not in ("true", "false", "null") else None)
    return val()


def norm(v):
    if isinstance(v, Float):
        return ("f", struct.unpack("<I", struct.pack("<f", float(v)))[0])
    if isinstance(v, list):
        return [norm(x) for x in v]
    if isinstance(v, dict):
        return {k: norm(x) for k, x in v.items()}
    return v


def first_diff(a, b, path="$"):
    if type(a) != type(b):
        return f"{path}: want {short(a)} got {short(b)}"
    if isinstance(a, dict):
        for k in sorted(set(a) | set(b)):
            if k not in b:
                return f"{path}.{k}: missing (want {short(a[k])})"
            if k not in a:
                return f"{path}.{k}: unexpected {short(b[k])}"
            d = first_diff(a[k], b[k], f"{path}.{k}")
            if d:
                return d
        return None
    if isinstance(a, list):
        if len(a) != len(b):
            return f"{path}: want {len(a)} items got {len(b)}"
        for i, (x, y) in enumerate(zip(a, b)):
            d = first_diff(x, y, f"{path}[{i}]")
            if d:
                return d
        return None
    if a != b:
        return f"{path}: want {short(a)} got {short(b)}"
    return None


def short(v):
    if isinstance(v, tuple):
        return "%r" % struct.unpack("<f", struct.pack("<I", v[1]))[0]
    s = json.dumps(v, default=str)
    return s if len(s) < 120 else s[:117] + "..."


def wrappers(group):
    out = []
    for f in (HERE / "rules" / group).rglob("*.bend"):
        out += re.findall(r"^def\s+case\.([^\s(?]+)", f.read_text(), re.M)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*", default=None)
    ap.add_argument("--cases", default=os.environ.get("DELVE_CASES"))
    ap.add_argument("--max", type=int, default=0, help="cases per name (0 = all)")
    ap.add_argument("names", nargs="+")
    a = ap.parse_args()
    if not a.cases:
        ap.error("set DELVE_CASES or pass --cases")
    names = []
    for n in a.names:
        names += wrappers(n) if (HERE / "rules" / n).is_dir() else [n]
    build = [sys.executable, str(HERE / "build.py")] + (["--only"] + a.only if a.only is not None else [])
    subprocess.run(build, check=True)
    exe = HERE / "build" / "cases"
    r = subprocess.run(["timeout", "600", str(BEND), str(HERE / "tests" / "cases_main.bend"), "-o", str(exe)],
                       capture_output=True, text=True)
    if r.returncode != 0 or not exe.exists():
        print(r.stdout + r.stderr); sys.exit(1)
    total_bad = 0
    for n in names:
        src = None
        for ext in (".jsonl", ".jsonl.gz"):
            p = pathlib.Path(a.cases) / (n + ext)
            if p.exists():
                src = p
        if src is None:
            print(f"{n}: no cases"); continue
        opener = gzip.open if src.suffix == ".gz" else open
        cases = [load(l) for l in opener(src, "rt") if l.strip()]
        if a.max:
            cases = cases[: a.max]
        feed = "".join(bj(c) + "\n" for c in cases)
        r = subprocess.run(["timeout", "600", str(exe)], input=feed, capture_output=True, text=True)
        lines = r.stdout.splitlines()
        bad = [(i, l) for i, l in enumerate(lines) if not l.startswith("ok ")]
        if len(lines) != len(cases):
            print(f"{n}: runner answered {len(lines)} of {len(cases)} cases\n{r.stderr[-2000:]}")
            total_bad += 1
            continue
        print(f"{n}: {len(cases) - len(bad)}/{len(cases)} pass")
        for i, l in bad[:3]:
            got = l.split(" ", 2)[2] if l.count(" ") >= 2 else ""
            try:
                print("   case", i, first_diff(norm(cases[i]["out"]), unbj(got)))
            except Exception as e:
                print("   case", i, "unreadable answer:", got[:200], e)
        total_bad += len(bad)
    sys.exit(1 if total_bad else 0)


if __name__ == "__main__":
    main()

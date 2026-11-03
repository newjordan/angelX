"""Base vs fine-tuned Mage on the same targets, same container runtime.
Pairs bench/results.jsonl (base) with bench/results-tuned.jsonl by target id.
Held-out test split first; dev targets were trained on and are marked so.
  python3 -I analyze_ab.py <bench-dir>   -> ab_analysis.json
"""
import collections, json, math, sys

root = sys.argv[1]
base = {r["id"]: r for r in map(json.loads, open(f"{root}/results.jsonl"))}
tuned = {r["id"]: r for r in map(json.loads, open(f"{root}/results-tuned.jsonl"))}
ids = [i for i in tuned if i in base]
ARMS = ("first", "z2", "casc")


def wilson(k, n, z=1.96):
    if n == 0:
        return 0.0, 0.0
    p, d = k / n, 1 + z * z / n
    c, h = (p + z * z / (2 * n)) / d, z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0.0, c - h), min(1.0, c + h)


def compare(sel, arm):
    b = [base[i][arm]["hit"] for i in sel]
    t = [tuned[i][arm]["hit"] for i in sel]
    gained = sum(1 for x, y in zip(b, t) if y and not x)
    lost = sum(1 for x, y in zip(b, t) if x and not y)
    m = gained + lost
    p = 1.0 if m == 0 else min(1.0, 2 * sum(math.comb(m, i) for i in range(0, min(gained, lost) + 1)) / 2 ** m)
    n = len(sel)
    kb, kt = sum(b), sum(t)
    return {"n": n, "base": kb, "tuned": kt, "base_pct": round(100 * kb / n, 1) if n else None,
            "tuned_pct": round(100 * kt / n, 1) if n else None,
            "tuned_ci": [round(100 * x, 1) for x in wilson(kt, n)], "gained": gained, "lost": lost,
            "delta_pts": round(100 * (kt - kb) / n, 1) if n else None, "p": p}


def group(key):
    g = collections.defaultdict(list)
    for i in ids:
        g[key(base[i])].append(i)
    return g


out = {"n": len(ids), "by_src_split": {}, "test_by_size_kind": {}, "test_by_app": {}}
for (src, split), sel in sorted(group(lambda r: (r["src"], r["split"])).items()):
    out["by_src_split"][f"{src}/{split}"] = {a: compare(sel, a) for a in ARMS}
test = [i for i in ids if base[i]["split"] == "test"]
for k, sel in sorted(group(lambda r: (r["src"], r["size_bin"], r["kind"])).items()):
    sel = [i for i in sel if i in test]
    if len(sel) >= 8:
        out["test_by_size_kind"]["/".join(k)] = compare(sel, "casc")
fam = lambda r: r["page"].split("-")[0] if r["src"] == "desk" else "web"
for f, sel in sorted(group(fam).items()):
    sel = [i for i in sel if i in test]
    if sel:
        out["test_by_app"][f] = compare(sel, "casc")
for src in ("desk", "web"):
    sel = [i for i in test if base[i]["src"] == src]
    out["by_src_split"][f"{src}/test-all"] = {a: compare(sel, a) for a in ARMS}
json.dump(out, open(f"{root}/ab_analysis.json", "w"), indent=1)
for k, v in out["by_src_split"].items():
    for a, s in v.items():
        print(f"{k:14} {a:5} base {s['base']}/{s['n']} ({s['base_pct']}%)  tuned {s['tuned']}/{s['n']} ({s['tuned_pct']}%)  "
              f"+{s['gained']} -{s['lost']}  delta {s['delta_pts']} pts  p={s['p']:.2g}")
print("-- test split by app (ground)")
for k, s in out["test_by_app"].items():
    print(f"{k:10} {s['base']}/{s['n']} -> {s['tuned']}/{s['n']}  +{s['gained']} -{s['lost']}")
print("-- test split by size/kind (ground)")
for k, s in out["test_by_size_kind"].items():
    print(f"{k:22} {s['base_pct']}% -> {s['tuned_pct']}%  +{s['gained']} -{s['lost']}  p={s['p']:.2g}")

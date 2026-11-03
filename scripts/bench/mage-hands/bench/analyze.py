"""Analyze the offline bench (results.jsonl): hit rates with Wilson 95% CIs,
exact McNemar tests for paired arms, run-to-run noise, strata, and verdicts
on the pre-registered hypotheses. Writes analysis.json.
  python3 -I analyze.py <bench-dir>
"""
import collections, json, math, statistics, sys

root = sys.argv[1]
rows = [json.loads(l) for l in open(f"{root}/results.jsonl")]
ARMS = ("first", "z2", "casc", "repeat", "chat")


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (p, max(0.0, c - h), min(1.0, c + h))


def mcnemar(a, b):
    """Exact two-sided McNemar on paired booleans: (b-only, a-only, p)."""
    only_a = sum(1 for x, y in zip(a, b) if x and not y)
    only_b = sum(1 for x, y in zip(a, b) if y and not x)
    n = only_a + only_b
    if n == 0:
        return only_b, only_a, 1.0
    k = min(only_a, only_b)
    p = sum(math.comb(n, i) for i in range(0, k + 1)) / 2 ** n
    return only_b, only_a, min(1.0, 2 * p)


def rate(rs, arm):
    k = sum(r[arm]["hit"] for r in rs)
    p, lo, hi = wilson(k, len(rs))
    d = [r[arm]["dist"] for r in rs if r[arm]["dist"] is not None]
    return {"k": k, "n": len(rs), "rate": round(p, 3), "ci": [round(lo, 3), round(hi, 3)],
            "median_dist": round(statistics.median(d), 1) if d else None,
            "parse_fail": sum(r[arm]["px"] is None for r in rs),
            "median_secs": round(statistics.median(r[arm]["secs"] for r in rs), 2)}


def group(key):
    g = collections.defaultdict(list)
    for r in rows:
        g[key(r)].append(r)
    return g


out = {"n": len(rows), "overall": {}, "by_src_split": {}, "by_size_kind": {}, "by_theme": {}, "by_family": {},
       "paired": {}, "noise": {}}
for src in ("desk", "web", "all"):
    rs = [r for r in rows if src == "all" or r["src"] == src]
    if rs:
        out["overall"][src] = {a: rate(rs, a) for a in ARMS}
for (src, split), rs in sorted(group(lambda r: (r["src"], r["split"])).items()):
    out["by_src_split"][f"{src}/{split}"] = {a: rate(rs, a) for a in ("first", "casc")}
for (src, sb, kind), rs in sorted(group(lambda r: (r["src"], r["size_bin"], r["kind"])).items()):
    out["by_size_kind"][f"{src}/{sb}/{kind}"] = {a: rate(rs, a) for a in ("first", "casc")}
for (theme,), rs in sorted(group(lambda r: (r["theme"] if r["src"] == "desk" else "web",)).items()):
    out["by_theme"][theme] = {a: rate(rs, a) for a in ("first", "casc")}
fam = lambda r: r["page"].split("-")[0] if r["src"] == "desk" else "web"
for f, rs in sorted(group(lambda r: fam(r)).items()):
    out["by_family"][f] = {a: rate(rs, a) for a in ("first", "casc")}
for src in ("desk", "web", "all"):
    rs = [r for r in rows if src == "all" or r["src"] == src]
    if not rs:
        continue
    for a, b in (("first", "casc"), ("first", "z2"), ("z2", "casc"), ("chat", "first")):
        gained, lost, p = mcnemar([r[a]["hit"] for r in rs], [r[b]["hit"] for r in rs])
        out["paired"][f"{src}: {a} -> {b}"] = {"gained": gained, "lost": lost, "p": p,
                                                 "delta_pts": round(100 * (gained - lost) / len(rs), 1)}
    flips = sum(r["first"]["hit"] != r["repeat"]["hit"] for r in rs)
    moved = [math.dist(r["first"]["px"], r["repeat"]["px"]) for r in rs if r["first"]["px"] and r["repeat"]["px"]]
    out["noise"][src] = {"hit_flips": flips, "n": len(rs), "flip_rate": round(flips / len(rs), 3),
                         "same_point": sum(m < 1 for m in moved), "median_move_px": round(statistics.median(moved), 1) if moved else None}

# Pre-registered hypotheses.
o, pr = out["overall"], out["paired"]
def pts(src, a, b):
    return round(100 * (o[src][b]["rate"] - o[src][a]["rate"]), 1) if src in o else None
small = [r for r in rows if r["size_bin"] in ("xs<16", "s16-23")]
icon = [r for r in rows if r["kind"] == "icon"]
text = [r for r in rows if r["kind"] == "text"]
H = {
    "H1 cascade beats first look (web +25..35, desk +15..25 pts)": {
        "web_pts": pts("web", "first", "casc"), "desk_pts": pts("desk", "first", "casc"),
        "web_p": pr.get("web: first -> casc", {}).get("p"), "desk_p": pr.get("desk: first -> casc", {}).get("p")},
    "H2 small targets (<24 px) under 40% even with cascade": {"casc_small": rate(small, "casc") if small else None},
    "H3 icon-only at least 25 pts below text (cascade)": {
        "icon": rate(icon, "casc") if icon else None, "text": rate(text, "casc") if text else None},
    "H4 run-to-run noise <= 2 pts": out["noise"],
    "H6 /generate beats the OpenAI chat path": {"all": pr.get("all: chat -> first")},
}
out["hypotheses"] = H
json.dump(out, open(f"{root}/analysis.json", "w"), indent=1)


def line(name, s):
    return f"{name:28} {s['k']:4}/{s['n']:<4} {100*s['rate']:5.1f}%  [{100*s['ci'][0]:4.1f}-{100*s['ci'][1]:4.1f}]  med {s['median_dist']}px  {s['median_secs']}s"


for src, arms in o.items():
    print(f"== {src}")
    for a, s in arms.items():
        print(line(a, s))
print("== paired (McNemar)")
for k, v in pr.items():
    print(f"{k:24} +{v['gained']} -{v['lost']}  delta {v['delta_pts']} pts  p={v['p']:.2g}")
print("== noise", json.dumps(out["noise"]))

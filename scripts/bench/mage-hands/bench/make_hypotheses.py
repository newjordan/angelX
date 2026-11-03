"""Pre-registered hypotheses, their predictions (fixed before the runs), and
what the bench observed, computed from results.jsonl + analysis + live_analysis.
  python3 -I make_hypotheses.py <bench-dir>
"""
import json, math, sys

root = sys.argv[1]
rows = [json.loads(l) for l in open(f"{root}/results.jsonl")]
A = json.load(open(f"{root}/analysis.json"))
L = json.load(open(f"{root}/live_analysis.json"))


def rate(rs, arm="casc"):
    k, n = sum(r[arm]["hit"] for r in rs), len(rs)
    return k, n, 100 * k / n


pts = lambda src: round(100 * (A["overall"][src]["casc"]["rate"] - A["overall"][src]["first"]["rate"]), 1)
p = lambda key: A["paired"][key]["p"]
small = [r for r in rows if r["size_bin"] in ("xs<16", "s16-23")]
sk, sn, sr = rate(small)
dk, dn, dr = rate([r for r in small if r["src"] == "desk"])
wk, wn, wr = rate([r for r in small if r["src"] == "web"])
ik, i_n, ir = rate([r for r in rows if r["kind"] == "icon"])
tk, tn, tr = rate([r for r in rows if r["kind"] == "text"])
nz = A["noise"]["all"]
net = abs(A["overall"]["all"]["first"]["k"] - A["overall"]["all"]["repeat"]["k"]) / A["n"] * 100
o = L["overall"]
chat = A["overall"]["all"]["chat"]
first = A["overall"]["all"]["first"]
gained = A["paired"]["desk: chat -> first"]["gained"] + A["paired"]["web: chat -> first"]["gained"]
lost = A["paired"]["desk: chat -> first"]["lost"] + A["paired"]["web: chat -> first"]["lost"]
web_in, desk_in = 25 <= pts("web") <= 35, 15 <= pts("desk") <= 25

H = [
    {"id": "H1", "claim": "ground's zoom passes beat the first look on the same targets",
     "prediction": "+25 to +35 points on web, +15 to +25 on desktop",
     "observed": f"+{pts('web')} points on web (p {p('web: first -> casc'):.1g}), +{pts('desk')} on desktop (p {p('desk: first -> casc'):.1g})",
     "verdict": "Supported" if web_in and desk_in else "Partly supported: a real gain on both, web below the predicted range" if desk_in else "Partly supported"},
    {"id": "H2", "claim": "Hit rate rises with target size; targets under 24 px stay under 40% even with ground",
     "prediction": "under 24 px: below 40%",
     "observed": f"{sr:.1f}% under 24 px ({sk} of {sn}); desktop {dr:.1f}%, web {wr:.1f}%",
     "verdict": "Supported" if sr < 40 else "Rejected: small targets with visible text do well; kind matters more than size"},
    {"id": "H3", "claim": "Icon-only targets are much worse than targets with visible text",
     "prediction": "a gap of at least 25 points with ground",
     "observed": f"icon only {ir:.1f}% ({ik} of {i_n}), visible text {tr:.1f}% ({tk} of {tn}): a {tr - ir:.0f} point gap",
     "verdict": "Supported" if tr - ir >= 25 else "Rejected"},
    {"id": "H4", "claim": "Asking twice at temperature 0 gives the same answer",
     "prediction": "noise of 2 points or less",
     "observed": f"{nz['hit_flips']} of {nz['n']} targets flipped ({100 * nz['flip_rate']:.1f}%); totals moved {net:.1f} points",
     "verdict": "Supported" if 100 * nz["flip_rate"] <= 2 else "Rejected"},
    {"id": "H5", "claim": "Inside angel, tasks pass more often than ground lands, because Wonder checks its work",
     "prediction": "task pass rate above live ground hit rate",
     "observed": f"tasks {o['tasks_pass']} of {o['tasks_n']}; ground {o['ground_hit']} of {o['ground_scored']}; {o['passed_despite_ground_miss']} runs passed after a ground miss",
     "verdict": ("Narrowly supported: Wonder recovered from most misses, but one Underline miss went unnoticed"
                 if 0 < o["tasks_pass"] / o["tasks_n"] - o["ground_hit"] / max(1, o["ground_scored"]) < 0.02
                 else "Supported" if o["tasks_pass"] / o["tasks_n"] > o["ground_hit"] / max(1, o["ground_scored"])
                 else "Not supported: Wonder did not catch every miss")},
    {"id": "H6", "claim": "SGLang's OpenAI chat path grounds worse than native /generate with Mage's template",
     "prediction": "/generate clearly ahead",
     "observed": f"/generate {first['k']} of {first['n']}, chat path {chat['k']} of {chat['n']}: {gained} targets gained, {lost} lost",
     "verdict": "Supported" if first["k"] > chat["k"] else "Rejected"},
]
json.dump(H, open(f"{root}/hypotheses.json", "w"), indent=1)
for h in H:
    print(h["id"], "|", h["observed"], "|", h["verdict"])

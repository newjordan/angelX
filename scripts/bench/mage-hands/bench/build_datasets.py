"""Turn analysis.json + live_analysis.json into the dashboard's datasets
(one JSON array of rows per file) under <bench>/dash/data/.
  python3 -I build_datasets.py <bench-dir>
"""
import json, os, sys

root = sys.argv[1]
A = json.load(open(f"{root}/analysis.json"))
L = json.load(open(f"{root}/live_analysis.json")) if os.path.exists(f"{root}/live_analysis.json") else None
out = f"{root}/dash/data"
os.makedirs(out, exist_ok=True)
SRC = {"web": "Web pages", "desk": "Desktop apps"}
ARM = {"first": "First look", "z2": "First look + 2× zoom", "casc": "ground (2× then 3× zoom)",
       "repeat": "First look, repeated", "chat": "First look via OpenAI chat path"}
pct = lambda x: round(100 * x, 1)


def save(name, rows):
    json.dump(rows, open(f"{out}/{name}.json", "w"), indent=1)
    print(name, len(rows), "rows")


rows = []
for src in ("web", "desk"):
    for arm in ("first", "z2", "casc", "chat"):
        s = A["overall"][src][arm]
        rows.append({"key": f"{src}/{arm}", "source": SRC[src], "method": ARM[arm], "hits": s["k"], "targets": s["n"],
                     "hit_pct": pct(s["rate"]), "ci_low_pct": pct(s["ci"][0]), "ci_high_pct": pct(s["ci"][1]),
                     "median_miss_px": s["median_dist"], "median_seconds": s["median_secs"],
                     "unparsed": s["parse_fail"]})
save("arms", rows)

rows = []
for k, v in A["paired"].items():
    src, cmp_ = k.split(": ")
    if src == "all":
        continue
    a, b = cmp_.split(" -> ")
    rows.append({"key": f"{src}/{a}-{b}", "source": SRC[src], "comparison": f"{ARM[a]} → {ARM[b]}",
                 "gained": v["gained"], "lost": v["lost"], "delta_pts": v["delta_pts"], "p_value": float(f"{v['p']:.2g}")})
save("paired", rows)

SIZE = {"xs<16": "under 16 px", "s16-23": "16–23 px", "m24-39": "24–39 px", "l40+": "40 px and up"}
rows = []
for k, v in A["by_size_kind"].items():
    src, sb, kind = k.split("/")
    if v["casc"]["n"] < 8:
        continue
    rows.append({"key": k, "source": SRC[src], "size": SIZE[sb], "kind": "icon only" if kind == "icon" else "visible text",
                 "targets": v["casc"]["n"], "first_pct": pct(v["first"]["rate"]), "ground_pct": pct(v["casc"]["rate"]),
                 "ci_low_pct": pct(v["casc"]["ci"][0]), "ci_high_pct": pct(v["casc"]["ci"][1])})
order = {"under 16 px": 0, "16–23 px": 1, "24–39 px": 2, "40 px and up": 3}
rows.sort(key=lambda r: (r["source"], order[r["size"]], r["kind"]))
save("strata", rows)

FAM = {"web": "Web pages", "nautilus": "Files (Nautilus)", "papers": "Papers", "evince": "Evince", "disks": "Disks",
       "xournalpp": "Xournal++", "pinta": "Pinta", "writer": "LibreOffice Writer", "calc": "LibreOffice Calc",
       "impress": "LibreOffice Impress", "draw": "LibreOffice Draw", "printers": "Printers", "zenity": "Zenity dialogs",
       "pair": "Two apps side by side", "kdenlive": "Kdenlive", "omacalc": "Omacalc"}
rows = []
for k, v in A["by_family"].items():
    rows.append({"key": k, "app": FAM.get(k, k), "targets": v["casc"]["n"], "first_pct": pct(v["first"]["rate"]),
                 "ground_pct": pct(v["casc"]["rate"]), "ci_low_pct": pct(v["casc"]["ci"][0]), "ci_high_pct": pct(v["casc"]["ci"][1])})
rows.sort(key=lambda r: -r["ground_pct"])
save("apps", rows)

rows = []
for k, v in A["by_src_split"].items():
    src, split = k.split("/")
    rows.append({"key": k, "source": SRC[src], "split": "held-out test" if split == "test" else "dev",
                 "targets": v["casc"]["n"], "first_pct": pct(v["first"]["rate"]), "ground_pct": pct(v["casc"]["rate"]),
                 "ci_low_pct": pct(v["casc"]["ci"][0]), "ci_high_pct": pct(v["casc"]["ci"][1])})
save("splits", rows)

rows = []
for src in ("web", "desk"):
    v = A["noise"][src]
    rows.append({"key": src, "source": SRC[src], "targets": v["n"], "hit_flips": v["hit_flips"],
                 "flip_pct": pct(v["flip_rate"]), "same_point": v["same_point"]})
save("noise", rows)

H = json.load(open(f"{root}/hypotheses.json"))
save("hypotheses", H)

if L:
    LABEL = {"calc-8008": "Omacalc: 1001 × 8 = 8008", "calc-58008": "Omacalc: 7251 × 8 = 58008",
             "calc-5318008": "Omacalc: 2659004 × 2 = 5318008", "calc-hello": "Omacalc: 7734 ÷ 10000 = 0.7734",
             "writer-bold": "Writer: make text bold", "writer-iu": "Writer: italic + underline",
             "zenity-list": "Zenity: tick exactly two wishes", "nautilus-open": "Files: open Projects → knights-wish"}
    rows = []
    for t, s in L["by_task"].items():
        rows.append({"key": t, "task": LABEL.get(t, t), "passed": s["pass"], "runs": s["n"],
                     "ground_hits": s["ground_hit"], "ground_scored": s["ground_scored"],
                     "ground_pct": pct(s["ground_hit"] / s["ground_scored"]) if s["ground_scored"] else None,
                     "median_seconds": s["median_wall_s"], "median_steps": s["median_hops"]})
    save("live_tasks", rows)
    o = L["overall"]
    save("live_overall", [{"key": "all", "passed": o["tasks_pass"], "runs": o["tasks_n"],
                           "pass_pct": pct(o["tasks_pass"] / o["tasks_n"]) if o["tasks_n"] else None,
                           "ci_low_pct": pct(o["tasks_ci"][0]), "ci_high_pct": pct(o["tasks_ci"][1]),
                           "ground_hits": o["ground_hit"], "ground_scored": o["ground_scored"],
                           "ground_pct": pct(o["ground_hit"] / o["ground_scored"]) if o["ground_scored"] else None,
                           "ground_ci_low_pct": pct(o["ground_ci"][0]), "ground_ci_high_pct": pct(o["ground_ci"][1]),
                           "passed_after_a_miss": o["passed_despite_ground_miss"]}])

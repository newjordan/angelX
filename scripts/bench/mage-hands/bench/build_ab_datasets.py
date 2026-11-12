"""Dashboard datasets for the fine-tune A/B (ab_analysis.json) and the live
fine-tuned bench (live_analysis-angel-live-tuned.json), into dash/data/.
  python3 -I build_ab_datasets.py <bench-dir>
"""
import json, os, sys

root = sys.argv[1]
A = json.load(open(f"{root}/ab_analysis.json"))
out = f"{root}/dash/data"
os.makedirs(out, exist_ok=True)
SRC = {"web": "Web pages", "desk": "Desktop apps"}
ARM = {"first": "First look", "casc": "ground"}
SIZE = {"xs<16": "under 16 px", "s16-23": "16–23 px", "m24-39": "24–39 px", "l40+": "40 px and up"}
FAM = {"web": "Web pages", "disks": "Disks", "printers": "Printers", "pinta": "Pinta", "kdenlive": "Kdenlive",
       "zenity": "Zenity dialogs", "omacalc": "Omacalc", "pair": "Two apps side by side"}


def save(name, rows):
    json.dump(rows, open(f"{out}/{name}.json", "w"), indent=1)
    print(name, len(rows), "rows")


rows = []
for split in ("test", "dev"):
    for src in ("desk", "web"):
        for arm in ("casc", "first"):
            s = A["by_src_split"][f"{src}/{split}"][arm]
            rows.append({"key": f"{src}/{split}/{arm}", "source": SRC[src],
                         "split": "held-out test" if split == "test" else "dev (trained on)", "method": ARM[arm],
                         "targets": s["n"], "base_pct": s["base_pct"], "tuned_pct": s["tuned_pct"],
                         "tuned_ci_low_pct": s["tuned_ci"][0], "tuned_ci_high_pct": s["tuned_ci"][1],
                         "gained": s["gained"], "lost": s["lost"], "delta_pts": s["delta_pts"], "p_value": float(f"{s['p']:.2g}")})
save("ab_split", rows)

rows = []
for k, s in A["test_by_size_kind"].items():
    src, sb, kind = k.split("/")
    rows.append({"key": k, "source": SRC[src], "size": SIZE[sb], "kind": "icon only" if kind == "icon" else "visible text",
                 "targets": s["n"], "first_pct": s["base_pct"], "ground_pct": s["tuned_pct"],
                 "ci_low_pct": s["tuned_ci"][0], "ci_high_pct": s["tuned_ci"][1], "gained": s["gained"], "lost": s["lost"]})
save("ab_strata", rows)

rows = []
for k, s in A["test_by_app"].items():
    rows.append({"key": k, "app": FAM.get(k, k), "targets": s["n"], "first_pct": s["base_pct"], "ground_pct": s["tuned_pct"],
                 "ci_low_pct": s["tuned_ci"][0], "ci_high_pct": s["tuned_ci"][1], "gained": s["gained"], "lost": s["lost"]})
rows.sort(key=lambda r: -r["ground_pct"])
save("ab_apps", rows)

L = f"{root}/live_analysis-angel-live-tuned.json"
if os.path.exists(L):
    T = json.load(open(L))
    LABEL = {"calc-8008": "Omacalc: 1001 × 8 = 8008", "calc-58008": "Omacalc: 7251 × 8 = 58008",
             "calc-5318008": "Omacalc: 2659004 × 2 = 5318008", "calc-hello": "Omacalc: 7734 ÷ 10000 = 0.7734",
             "writer-bold": "Writer: make text bold", "writer-iu": "Writer: italic + underline",
             "zenity-list": "Zenity: tick exactly two wishes", "nautilus-open": "Files: open Projects → knights-wish"}
    rows = []
    for t, s in T["by_task"].items():
        rows.append({"key": t, "task": LABEL.get(t, t), "passed": s["pass"], "runs": s["n"],
                     "ground_hits": s["ground_hit"], "ground_scored": s["ground_scored"],
                     "median_seconds": s["median_wall_s"], "median_steps": s["median_hops"]})
    save("live_tuned_tasks", rows)
    o = T["overall"]
    save("live_tuned_overall", [{"key": "all", "passed": o["tasks_pass"], "runs": o["tasks_n"],
                                 "pass_pct": round(100 * o["tasks_pass"] / o["tasks_n"], 1) if o["tasks_n"] else None,
                                 "ground_hits": o["ground_hit"], "ground_scored": o["ground_scored"],
                                 "ground_pct": round(100 * o["ground_hit"] / o["ground_scored"], 1) if o["ground_scored"] else None,
                                 "infrastructure_errors": o.get("infra_errors", 0),
                                 "invalid_runs": o.get("invalid_runs", 0),
                                 "expected_runs": o.get("expected_runs"),
                                 "missing_runs": o.get("missing_runs"),
                                 "status": o.get("status", "unverified_legacy_summary")}])

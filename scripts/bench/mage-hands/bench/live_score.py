"""Score the live bench runs (angel-live/*/): verdicts plus every `ground`
call checked against the captured AT-SPI box of the control it named.
  python3 -I live_score.py <bench-dir>
"""
import collections, glob, json, math, os, re, statistics, sys

root = sys.argv[1]
REQUIRE_COMPLETE = "--require-complete" in sys.argv[2:]
TASKS = os.environ.get("LIVE_TASKS", "calc-8008 calc-58008 calc-5318008 calc-hello writer-bold writer-iu zenity-list nautilus-open").split()
REPS = int(os.environ.get("LIVE_REPS", "3"))
if REPS < 1 or not TASKS:
    sys.exit("LIVE_REPS must be positive and LIVE_TASKS must not be empty")
EXPECTED = {f"{t}-r{k}" for t in TASKS for k in range(1, REPS + 1)}
pages = {p["name"]: p for p in json.load(open(f"{root}/desk/pages.json"))}
REF = {"calc": "omacalc", "writer": "writer-dark", "zenity": "zenity-list", "nautilus": "nautilus-home"}
CALL = re.compile(r"\[task-event\] call desktop: action=(\w+)(?:, (.*))?$")
GROUND_DONE = re.compile(r"done desktop .*?: (.*) is at \((-?\d+),(-?\d+)\)")
OPS = [(r"multipl|×|\btimes\b|\*", "Multiply"), (r"divi|÷", "Divide"), (r"equal|=", "Equals"),
       (r"decimal|point|\bdot\b", "."), (r"all clear|\bAC\b|clear", "All clear"), (r"plus|\badd\b", "Add"),
       (r"minus|subtract", "Subtract"), (r"backspace", "Backspace")]


def wilson(k, n, z=1.96):
    if n == 0:
        return None, None
    p, d = k / n, 1 + z * z / n
    c, h = (p + z * z / (2 * n)) / d, z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0.0, c - h), min(1.0, c + h)


def truth_for(task, target):
    if task.startswith("nautilus"):
        # The captured home folder held six folders and the live runs' home holds three,
        # so the captured boxes are not where the live folders are: unscored.
        return None, None
    page = pages[REF[task.split("-")[0]]]
    els = {e["name"]: e["box"] for e in page["elements"] if e["role"] != "label"} | \
          {e["name"]: e["box"] for e in page["elements"] if e["role"] == "label" and e["name"] not in
           {x["name"] for x in page["elements"] if x["role"] != "label"}}
    t = target.lower()
    if task.startswith("calc"):
        for pat, name in OPS:
            if re.search(pat, t, re.I):
                return name, els.get(name)
        m = re.search(r"(?<![\d.])(\d)(?![\d.])", t)
        return (m.group(1), els.get(m.group(1))) if m else (None, None)
    # Whole-word match on the control's name (so "formatting" never matches "Format"),
    # real controls over labels and titles; checkboxes have no captured box: unscored.
    if re.search(r"check ?box|tick", t):
        return None, None
    ctrl = {"push button", "toggle button", "button", "table cell", "list item", "check box", "menu item", "page tab", "icon"}
    best = None
    for e in page["elements"]:
        n = e["name"].lower().replace(". folder", "")
        if len(n) < 2 or not re.search(rf"(?<!\w){re.escape(n)}(?!\w)", t):
            continue
        rank = (e["role"] in ctrl, len(n))
        if best is None or rank > best[0]:
            best = (rank, e["name"], e["box"])
    if best:
        return best[1], best[2]
    return best or (None, None)


runs, infra_runs, invalid_runs = [], [], []
LIVE = os.environ.get("LIVE_DIR", "angel-live")
for d in sorted(glob.glob(f"{root}/{LIVE}/*-r*")):
    if not os.path.exists(f"{d}/verdict.json"):
        continue
    v = json.load(open(f"{d}/verdict.json"))
    task = v["task"]
    if task.startswith("writer"):
        # Re-derived from the recorded AT-SPI state: the first verifier wanted the
        # state list to be exactly ['checked'], and LibreOffice also reports 'focused'.
        on = lambda name: re.search(rf"toggle button \| '{name}' \| text='' \| \[[^]]*'checked'", v["observed"]) is not None
        text_ok = "Knights Wish" in v["observed"]
        v["pass"] = text_ok and (on("Bold") if task == "writer-bold" else on("Italic") and on("Underline"))
    try:
        tj = json.load(open(f"{d}/task.json"))
    except Exception:
        tj = {}
    calls, grounds, pending = collections.Counter(), [], None
    transport_error = False
    log_path = os.path.join(d, "task.err")
    log = open(log_path, errors="replace").read() if os.path.exists(log_path) else ""
    for line in log.splitlines():
        m = CALL.search(line)
        if m:
            calls[m.group(1)] += 1
            if m.group(1) == "ground":
                pending = (m.group(2) or "").split("target=", 1)[-1]
            continue
        if pending is not None and re.search(r"HTTP.?5\d\d|500 Internal|CUDA-capable device|Connection refused|busy or unavailable", line, re.I):
            transport_error = True
        g = GROUND_DONE.search(line)
        if g and pending is not None:
            x, y = int(g.group(2)), int(g.group(3))
            name, box = truth_for(task, pending)
            hit = None if box is None else (box[0] <= x <= box[2] and box[1] <= y <= box[3])
            grounds.append({"target": pending, "point": [x, y], "truth": name, "hit": hit})
            pending = None
    scored = [g for g in grounds if g["hit"] is not None]
    reason = None
    if v.get("angel_rc") in (126, 127):
        reason = f"launcher_exit_{v['angel_rc']}"
    elif tj.get("stop_reason") == "provider_error":
        reason = "provider_error"
    elif transport_error:
        reason = "ground_transport_error"
    elif not tj:
        reason = "missing_task_record"
    if reason:
        infra_runs.append({"run": os.path.basename(d), "task": task, "infra": True,
                           "reason": reason, "error": tj.get("error"), "angel_rc": v.get("angel_rc")})
        continue
    reason = None
    if os.path.basename(d) not in EXPECTED:
        reason = "unexpected_run"
    elif not calls["ground"]:
        reason = "no_successful_ground"
    elif task.startswith("calc") and (calls["key"] or calls["type"]):
        reason = "keyboard_used_on_calculator"
    if reason:
        invalid_runs.append({"run": os.path.basename(d), "task": task, "reason": reason})
        continue
    # An app-side pass alone is not a Mage pass. Zero parseable model points or
    # a nonzero driver exit is a failure, even if the brain finished by itself.
    passed = bool(v["pass"] and grounds and v.get("angel_rc") == 0)
    runs.append({"run": os.path.basename(d), "task": task, "pass": passed, "observed": v["observed"], "infra": False,
                 "error": tj.get("error"),
                 "wall_s": v["wall_s"], "hops": tj.get("hops"), "status": tj.get("status"),
                 "ground_attempts": calls["ground"], "ground": len(grounds), "ground_scored": len(scored), "ground_hit": sum(g["hit"] for g in scored),
                 "clicks": calls["click"], "keys": calls["key"] + calls["type"], "grounds": grounds})

for d in sorted(glob.glob(f"{root}/{LIVE}-infra/*-r*")):  # first attempts set aside as mulligans
    try:
        tj = json.load(open(f"{d}/task.json"))
    except Exception:
        tj = {}
    infra_runs.append({"run": os.path.basename(d) + " (first attempt)", "task": os.path.basename(d).rsplit("-r", 1)[0],
                       "infra": True, "reason": "archived_infrastructure_attempt",
                       "error": tj.get("error"), "hops": tj.get("hops")})
runs = [r for r in runs if not r["infra"]]  # provider errors are mulligans: rerun, never counted
by_task = collections.defaultdict(list)
for r in runs:
    by_task[r["task"]].append(r)
summary = {}
for t, rs in sorted(by_task.items()):
    k = sum(r["pass"] for r in rs)
    gs = sum(r["ground_scored"] for r in rs)
    gh = sum(r["ground_hit"] for r in rs)
    summary[t] = {"pass": k, "n": len(rs), "ground_hit": gh, "ground_scored": gs,
                  "median_wall_s": statistics.median(r["wall_s"] for r in rs),
                  "median_hops": statistics.median(r["hops"] or 0 for r in rs),
                  "keyboard_on_calc": sum(r["keys"] for r in rs) if t.startswith("calc") else None}
K, N = sum(r["pass"] for r in runs), len(runs)
GH, GS = sum(r["ground_hit"] for r in runs), sum(r["ground_scored"] for r in runs)
recovered = sum(1 for r in runs if r["pass"] and r["ground_scored"] and r["ground_hit"] < r["ground_scored"])
missing = sorted(EXPECTED - {r["run"] for r in runs})
overall = {"status": "incomplete" if missing else "complete", "expected_runs": len(EXPECTED),
           "missing_runs": len(missing), "invalid_runs": len(invalid_runs), "infra_errors": len(infra_runs), "tasks_pass": K, "tasks_n": N, "tasks_ci": wilson(K, N), "ground_hit": GH, "ground_scored": GS,
           "ground_ci": wilson(GH, GS), "passed_despite_ground_miss": recovered}
json.dump({"overall": overall, "by_task": summary, "runs": runs, "infra_runs": infra_runs,
           "invalid_runs": invalid_runs, "missing_runs": missing}, open(f"{root}/" + ("live_analysis.json" if LIVE == "angel-live" else f"live_analysis-{LIVE}.json"), "w"), indent=1)
for t, s in summary.items():
    print(f"{t:14} pass {s['pass']}/{s['n']}  ground {s['ground_hit']}/{s['ground_scored']}  "
          f"wall {s['median_wall_s']}s  hops {s['median_hops']}" + (f"  keys {s['keyboard_on_calc']}" if s["keyboard_on_calc"] is not None else ""))
print(f"infrastructure errors (not model failures): {len(infra_runs)} {[r['run'] for r in infra_runs]}")
print(f"overall tasks {K}/{N} ci {tuple(round(x, 2) if x is not None else None for x in overall['tasks_ci'])}; ground {GH}/{GS} "
      f"ci {tuple(round(x, 2) if x is not None else None for x in overall['ground_ci'])}; passed despite a ground miss: {recovered}")

print(f"{overall['status']}: {N}/{len(EXPECTED)} valid runs; {len(missing)} still needed; {len(invalid_runs)} invalid")
if REQUIRE_COMPLETE and missing:
    sys.exit(2)

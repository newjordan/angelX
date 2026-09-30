#!/usr/bin/env python3
"""Release benchmark report for polyglot-v1: models inside angelX, plus Grok 4.7 by harness.

Selects runs through angelx_board.collect(), then checks their raw traces against
the named task catalog. Scores and wall times must be observed; missing usage
stays unknown with per-field coverage. Each model's newest complete angelX run is used;
a model announced in EXPECTED without a complete run is listed as "running" and left out of the
video, the card and the post.

Writes to --out (default: <bench root>/report-0.1.6/):
  models.json          every number rendered below, plus per-task results
  report.html          self-contained page, light and dark
  post.txt             draft post, numbers only from models.json
  scoreboard.mp4       1080x1080 H.264 yuv420p, 16 s: the board fills, then the end card (skipped with --no-video)
  scoreboard-final.png 1080x1080 last frame
  card.png             1200x675 post card

The wordmark comes from website/assets/logo-mark.png (--site, else ./website, else ~/angelX/website).

usage: python3 scripts/build_release_report.py [--bench-root DIR] [--out DIR] [--site DIR] [--no-video]
"""

from __future__ import annotations

import argparse
import base64
from collections import Counter
import hashlib
import html
import importlib.util
import json
import math
import os
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

sys.dont_write_bytecode = True  # the bench root is read-only data

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent
TEMPLATES = HERE / "release-report"
spec = importlib.util.spec_from_file_location("release_report_history", HERE / "analyze-polyglot-history.py")
history = importlib.util.module_from_spec(spec)
spec.loader.exec_module(history)
DEFAULT_BENCH = Path.home() / "angel_tests" / "angelX-bench" / "polyglot-20260921"
# The site (website/, untracked) carries the wordmark; the main checkout's copy is the fallback.
SITE_DIRS = [REPO_ROOT / "website", Path.home() / "angelX" / "website"]
LETTERHEAD = "angelX | github.com/newjordan/angelX · @frostforger"

NAMES = {
    "deepseek-flash": "DeepSeek V4.1 Flash",
    "glm-5.3-flash": "GLM-5.3-Flash",
    "grok-4.7": "Grok 4.7",
    "gpt-6-luna": "gpt-6-luna",
    "muse-spark-1.3": "Muse Spark",
}
# Announced runs: shown as "running" until their traces hold every task.
EXPECTED = {"muse": "Muse Spark"}
GROK_HARNESSES = ["angelx", "omp", "opencode", "hermes", "primebash"]
LANGS = [("javascript", "JavaScript", "JS"), ("python", "Python", "Python"), ("rust", "Rust", "Rust"), ("cpp", "C++", "C++")]
LANG_OF = {"js": "javascript", "py": "python", "rust": "rust", "cpp": "cpp"}
EVALUATOR = "Verifiers v0.3.1"
FONTS_URL = "https://fonts.googleapis.com/css2?family=Cinzel:wght@600;700&family=VT323&display=swap"
POST_LIMIT = 280


# ---------------------------------------------------------------- data

def load_board(bench: Path):
    sys.path.insert(0, str(bench))
    import angelx_board  # noqa: E402  (imports analyze from the same root)

    return angelx_board


def reasoning_label(value) -> str:
    return "off" if value in (None, "none") else str(value)


def harness_version(bench: Path, model: str, harness: str, run: str) -> str | None:
    try:
        cfg = json.loads((bench / "runs" / model / harness / run / "configs" / "resolved" / "eval.json").read_text())
    except (OSError, ValueError):
        return None
    spec = ((cfg.get("env") or {}).get("agent") or {}).get("harness") or {}
    for key, value in spec.items():
        if key.endswith("_bin") and isinstance(value, str):
            m = re.search(r"/installs/[^/]+/(\d+\.\d+\.\d+)/", value)
            if m:
                return m.group(1)
    return None


def rollouts_per_task(bench: Path, model: str, harness: str, run: str) -> int | None:
    try:
        cfg = json.loads((bench / "runs" / model / harness / run / "configs" / "resolved" / "eval.json").read_text())
    except (OSError, ValueError):
        return None
    return cfg.get("num_rollouts")


def task_catalog(bench):
    payload = (bench / "tasks-polyglot-v1.json").read_bytes()
    tasks = json.loads(payload)
    if not isinstance(tasks, list) or not tasks:
        raise ValueError("task catalog must be a nonempty list")
    names = [task.get("name") if isinstance(task, dict) else None for task in tasks]
    if any(not isinstance(name, str) or not name for name in names) or len(set(names)) != len(names):
        raise ValueError("task catalog requires unique nonempty names")
    return names, hashlib.sha256(payload).hexdigest()


def checked_rows(run_dir, catalog):
    path = run_dir / "traces.jsonl"
    payload = path.read_bytes()
    rows = []
    for line_no, line in enumerate(payload.splitlines(), 1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except ValueError as error:
            raise ValueError(f"invalid trace JSON: {path}:{line_no}") from error
        if not isinstance(record, dict) or not isinstance(record.get("traces"), list) or not record["traces"]:
            raise ValueError(f"missing trace attempts: {path}:{line_no}")
        for attempt, trace in enumerate(record["traces"], 1):
            try:
                if not isinstance(trace, dict):
                    raise ValueError("trace must be an object")
                row = history.extract_row(record, trace, run_dir.name)
                if row["task"] is not None and not isinstance(row["task"], str):
                    raise ValueError("task name must be a string")
                trace_task = ((trace.get("task") or {}).get("data") or {}).get("name")
                if trace_task is not None and trace_task != row["task"]:
                    raise ValueError("record/trace task identity conflict")
                info = trace.get("info") or {}
                cap = history.number(((info.get("agent_exit") or {}).get("wall_cap") or {}).get("secs"))
                row["wall_cap_s"] = cap if cap is not None and cap >= 0 else None
                isolation = (info.get("agent_isolation") or {}).get("mode")
                row["isolation"] = isolation if isinstance(isolation, str) and isolation else "unknown"
            except (AttributeError, TypeError, ValueError) as error:
                raise ValueError(f"invalid trace shape/identity: {path}:{line_no}:{attempt}") from error
            rows.append(row)
    counts = Counter(row["task"] for row in rows)
    missing = sorted(set(catalog) - set(counts))
    unexpected = sorted(str(name) for name in set(counts) - set(catalog))
    duplicates = {str(name): count for name, count in counts.items() if count != 1}
    if missing or unexpected or duplicates:
        raise ValueError(f"unsafe cohort {run_dir}: missing={missing}; unexpected={unexpected}; duplicate attempts={duplicates}")
    for row in rows:
        if row["score"] not in (0, 1):
            raise ValueError(f"unsafe cohort {run_dir}: {row['task']} missing observed binary score")
        if history.number(row["wall_s"]) is None or row["wall_s"] < 0:
            raise ValueError(f"unsafe cohort {run_dir}: {row['task']} missing observed wall time")
    return rows, hashlib.sha256(payload).hexdigest()


def entry(board, bench: Path, c: dict, catalog=None) -> dict:
    cfg = c["config"]
    run_dir = bench / "runs" / c["model"] / c["harness"] / c["run"]
    catalog = task_catalog(bench)[0] if catalog is None else catalog
    rollout_count = rollouts_per_task(bench, c["model"], c["harness"], c["run"])
    if not isinstance(rollout_count, int) or isinstance(rollout_count, bool) or rollout_count != 1:
        raise ValueError(f"unsafe cohort {run_dir}: report requires recorded num_rollouts=1")
    rows, trace_sha256 = checked_rows(run_dir, catalog)
    s = history.aggregate(rows)
    fields = ("calls", "input", "cached_input", "uncached_input", "output", "reasoning")
    totals, subtotals, coverage = history.receipt_fields({field: [row[field] for row in rows] for field in fields})
    receipt_coverage = {}
    # A row's partial receipt subtotal remains useful even when its task total
    # is unknown. Preserve it separately from complete chart/report values.
    for field in fields[1:]:
        values = [history.number(row["usage_reported_subtotals"].get(field)) for row in rows]
        known = [value for value in values if value is not None and value >= 0]
        subtotals[field] = sum(known) if known else None
        receipts = [row["usage_metric_coverage"][field] for row in rows]
        attempts = [receipt["attempts"] for receipt in receipts]
        reported = [receipt["reported"] for receipt in receipts]
        valid_count = lambda value: isinstance(value, int) and not isinstance(value, bool) and value >= 0
        observed = [value for value in reported if valid_count(value)]
        receipt_coverage[field] = {"reported": sum(observed) if observed else None,
                                   "attempts": sum(attempts) if all(valid_count(value) for value in attempts) else None,
                                   "complete": coverage[field]["complete"]}
    metered = c["metered"]
    model_id = cfg.get("model_id") or c["model"]

    def m(v):
        return v if metered else None

    solved_walls = [row["wall_s"] for row in rows if row["solved"]]
    total_in = totals["input"]
    cache_hit = (totals["cached_input"] / total_in if total_in is not None and total_in > 0
                 and totals["cached_input"] is not None and totals["cached_input"] <= total_in else None)
    by_language = {}
    for language, _, _ in LANGS:
        language_rows = [row for row in rows if LANG_OF.get(row["task"].split("-")[0]) == language]
        if language_rows:
            by_language[language] = {"solved": sum(row["solved"] for row in language_rows), "n": len(language_rows)}
    return {
        "key": c["model"],
        "name": NAMES.get(model_id, model_id),
        "model_id": model_id,
        "harness": c["harness"],
        "harness_name": board.HARNESS_NAMES.get(c["harness"], c["harness"]),
        "harness_version": harness_version(bench, c["model"], c["harness"], c["run"]),
        "run": c["run"],
        "status": c["status"],
        "reasoning": reasoning_label(cfg.get("reasoning")),
        "temperature": cfg.get("temperature"),
        "angelx_bin_sha256": cfg.get("bin_sha256"),
        "angelx_source_sha256": cfg.get("source_sha256"),
        "metered": metered,
        "solved": s["solved"],
        "tasks": len(rows),
        "solve_rate": s["solved"] / len(rows),
        "by_language": {k: by_language.get(k) for k, _, _ in LANGS},
        "wall_median_s": s["metrics"]["wall_s"]["median"],
        "solved_wall_median_s": statistics.median(solved_walls) if solved_walls else None,
        "wall_total_s": s["metrics"]["wall_s"]["total"],
        "calls_per_task": m(totals["calls"] / len(rows) if totals["calls"] is not None else None),
        "input_tokens": m(totals["input"]),
        "uncached_input_tokens": m(totals["uncached_input"]),
        "cache_hit": m(cache_hit),
        "output_tokens": m(totals["output"]),
        "reasoning_tokens": m(totals["reasoning"]),
        "metric_coverage": coverage,
        "usage_metric_coverage": receipt_coverage,
        "reported_subtotals": subtotals,
        "cache_invalid_attempts": s["cache_invalid_tasks"],
        "trace_sha256": trace_sha256,
        "timeouts": s["timeouts"],
        "tamper_check_fails": s["integrity_failures"],
        "nonzero_exits": sum(row["exit_code"] is not None and row["exit_code"] != 0 for row in rows),
        "unscored": s["unscored"],
        "model_call_errors": m(s["call_errors"]),
        "wall_caps_s": sorted({row["wall_cap_s"] for row in rows} - {None}),
        "isolation": sorted({row["isolation"] for row in rows}),
        "rollouts_per_task": rollouts_per_task(bench, c["model"], c["harness"], c["run"]),
        "unsolved": sorted(row["task"] for row in rows if not row["solved"]),
        # execution order, as the traces were written
        "attempts": [
            {
                "task": r["task"],
                "language": LANG_OF.get(r["task"].split("-")[0]),
                "solved": r["solved"],
                "wall_s": round(r["wall_s"], 3),
            }
            for r in rows
        ],
    }


def build_data(bench: Path) -> dict:
    catalog, catalog_sha256 = task_catalog(bench)
    board = load_board(bench)
    tasks = len(catalog)
    if board.TASKS != tasks:
        raise ValueError("board task count conflicts with the named catalog")
    cells = board.collect()
    complete = {}
    for c in cells:
        if c["status"] == "complete" and c["model"] not in complete:
            complete[c["model"]] = c
    models = [entry(board, bench, c, catalog) for c in complete.values()]
    # A model without a complete run is listed by name only: no partial numbers anywhere.
    pending: dict[str, dict] = {}
    for c in cells:
        if c["model"] not in complete and c["model"] not in pending:
            pending[c["model"]] = {
                "key": c["model"],
                "name": NAMES.get(c["config"].get("model_id") or "", EXPECTED.get(c["model"], c["model"])),
                "status": "running",
                "reasoning": reasoning_label(c["config"].get("reasoning")),
            }
    for key, name in EXPECTED.items():
        if key not in complete and key not in pending:
            pending[key] = {"key": key, "name": name, "status": "running", "reasoning": None}
    pending = list(pending.values())

    grok = []
    for c in board.collect("grok"):
        if c["status"] == "complete" and c["harness"] in GROK_HARNESSES and all(g["harness"] != c["harness"] for g in grok):
            grok.append(entry(board, bench, c, catalog))
    grok.sort(key=lambda g: GROK_HARNESSES.index(g["harness"]))

    everything = models + grok
    caps = sorted({cap for e in everything for cap in e["wall_caps_s"]})
    isolation = sorted({i for e in everything for i in e["isolation"]})
    attempts = sorted({e["rollouts_per_task"] for e in everything if e["rollouts_per_task"] is not None})
    langs = {}
    for name in catalog:
        prefix = name.split("-")[0]
        langs[prefix] = langs.get(prefix, 0) + 1
    by_lang = {"javascript": langs.get("js", 0), "python": langs.get("py", 0), "rust": langs.get("rust", 0), "cpp": langs.get("cpp", 0)}

    data = {
        "schema": "angelx-release-report/v1",
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC"),
        "suite": {"name": "polyglot-v1", "tasks": tasks, "task_names": catalog,
                  "catalog_sha256": catalog_sha256, "languages": by_lang},
        "cell": {
            "wall_cap_s": caps[0] if len(caps) == 1 else caps,
            "attempts_per_task": attempts[0] if len(attempts) == 1 else attempts,
            "home": "fresh" if isolation == ["fresh-home"] else isolation,
            "grading": "evaluator-owned",
            "sampling": "imposed by the proxy",
            "evaluator": EVALUATOR,
        },
        "models": models,
        "pending": pending,
        "grok_by_harness": grok,
    }
    validate_report(data)
    return data


def validate_report(data):
    """Guard direct rendering entry points as well as the CLI builder."""
    suite = data.get("suite") or {}
    catalog = suite.get("task_names")
    if (not isinstance(catalog, list) or not catalog or any(not isinstance(name, str) or not name for name in catalog)
            or len(set(catalog)) != len(catalog) or not isinstance(suite.get("tasks"), int)
            or isinstance(suite.get("tasks"), bool) or suite["tasks"] != len(catalog)):
        raise ValueError("report requires its actual unique named task catalog")
    for model in data.get("models", []) + data.get("grok_by_harness", []):
        attempts = model.get("attempts")
        if not isinstance(attempts, list) or len(attempts) != len(catalog):
            raise ValueError(f"unsafe report {model.get('name')}: incomplete attempt catalog")
        names = [attempt.get("task") if isinstance(attempt, dict) else None for attempt in attempts]
        if any(not isinstance(name, str) for name in names) or Counter(names) != Counter(catalog):
            raise ValueError(f"unsafe report {model.get('name')}: duplicate/missing/unexpected tasks")
        for attempt in attempts:
            wall = history.number(attempt.get("wall_s"))
            if not isinstance(attempt.get("solved"), bool) or wall is None or wall < 0:
                raise ValueError(f"unsafe report {model.get('name')}: {attempt['task']} missing observed score/wall")
        total = history.number(model.get("wall_total_s"))
        if (not isinstance(model.get("tasks"), int) or isinstance(model.get("tasks"), bool)
                or not isinstance(model.get("solved"), int) or isinstance(model.get("solved"), bool)
                or model["tasks"] != len(catalog) or model["solved"] != sum(attempt["solved"] for attempt in attempts)
                or total is None or total < 0):
            raise ValueError(f"unsafe report {model.get('name')}: invalid count/time totals")
        # Attempt arrays round milliseconds for display; allow only that known
        # rounding bound when checking the unrounded aggregate used by media.
        if not math.isclose(total, sum(attempt["wall_s"] for attempt in attempts),
                            rel_tol=1e-12, abs_tol=len(attempts) * .0005 + 1e-9):
            raise ValueError(f"unsafe report {model.get('name')}: time total conflicts with attempts")


# ---------------------------------------------------------------- formatting

def f_int(v) -> str:
    return f"{v:,}" if v is not None else "not metered"


def f_s(v) -> str:
    return f"{v:.1f} s" if v is not None else "—"


def f_min(v) -> str:
    return f"{v / 60:.1f} min"


def f_pct(v) -> str:
    return f"{100 * v:.1f}%"


def f_solved(e) -> str:
    return f"{e['solved']} / {e['tasks']}"


def f_lang(e, key) -> str:
    v = e["by_language"].get(key)
    return f"{v['solved']} / {v['n']}" if v else "—"


def f_calls(e) -> str:
    return f"{e['calls_per_task']:.1f}" if e["calls_per_task"] is not None else "not metered"


def cell_line(data: dict) -> str:
    cell = data["cell"]
    cap = cell["wall_cap_s"]
    attempts = cell["attempts_per_task"]
    parts = [
        f"{cap} s wall" if isinstance(cap, int) else "wall " + "/".join(f"{c} s" for c in cap),
        "one attempt" if attempts == 1 else f"{attempts} attempts",
        "fresh HOME" if cell["home"] == "fresh" else "HOME " + "/".join(cell["home"]),
        "evaluator-owned grading",
        "sampling imposed by the proxy",
        "thinking per model: " + ", ".join(f"{m['name']} {m['reasoning']}" for m in data["models"]),
    ]
    return " · ".join(parts)


# ---------------------------------------------------------------- fonts

def font_css(cache_dir: Path) -> str:
    """Cinzel + VT323 as inline data: URIs, cached; empty string when offline."""
    cached = cache_dir / "fonts-inline.css"
    if cached.is_file():
        return cached.read_text()
    ua = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"
    try:
        req = urllib.request.Request(FONTS_URL, headers={"User-Agent": ua})
        css = urllib.request.urlopen(req, timeout=20).read().decode()

        def inline(m):
            data = urllib.request.urlopen(urllib.request.Request(m.group(1), headers={"User-Agent": ua}), timeout=20).read()
            return f"url(data:font/woff2;base64,{base64.b64encode(data).decode()})"

        css = re.sub(r"url\((https://[^)]+)\)", inline, css)
    except Exception as exc:  # offline: pages fall back to the Google Fonts link
        print(f"fonts: inline fetch failed ({exc}); using the Google Fonts link", file=sys.stderr)
        return ""
    cache_dir.mkdir(parents=True, exist_ok=True)
    cached.write_text(css)
    return css


def find_logo(site: Path | None) -> Path | None:
    for d in ([site] if site else []) + SITE_DIRS:
        f = d / "assets" / "logo-mark.png"
        if f.is_file():
            return f
    print("logo: website/assets/logo-mark.png not found; the letterhead text stands alone", file=sys.stderr)
    return None


def logo_img(logo: Path | None, width: int = 840) -> str:
    """The wordmark as an inline data: URI: flattened onto the plate's #050506 and JPEG-encoded
    when ffmpeg is available (the report stays small), else the original PNG."""
    if not logo:
        return ""
    data, mime = logo.read_bytes(), "image/png"
    if shutil.which("ffmpeg"):
        with tempfile.TemporaryDirectory() as tmp:
            small = Path(tmp) / "logo.jpg"
            graph = f"[0]split[a][b];[b]drawbox=c=0x050506:t=fill[bg];[bg][a]overlay,scale={width}:-1:flags=lanczos"
            r = subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(logo), "-filter_complex", graph,
                                "-frames:v", "1", "-update", "1", "-q:v", "2", str(small)], capture_output=True)
            if r.returncode == 0 and small.is_file():
                data, mime = small.read_bytes(), "image/jpeg"
    return f'<img src="data:{mime};base64,{base64.b64encode(data).decode()}" alt="AngelX" width="1429" height="331">'


def font_link(fonts: str) -> str:
    return "" if fonts else f'<link rel="stylesheet" href="{FONTS_URL}">'


def fill(template: str, **values: str) -> str:
    for key, value in values.items():
        template = template.replace("{{" + key + "}}", value)
    return template


# ---------------------------------------------------------------- report.html

def strip_svg(e: dict) -> str:
    """136 tasks grouped by language, two rows per group; filled = solved."""
    pitch, cell, gap_group = 11, 8, 18
    groups = []
    for key, label, _ in LANGS:
        items = sorted((a for a in e["attempts"] if a["language"] == key), key=lambda a: a["task"])
        groups.append((label, items))
    x = 0
    parts = []
    for label, items in groups:
        cols = (len(items) + 1) // 2
        parts.append(f'<text x="{x}" y="10" class="sl">{html.escape(label)}</text>')
        for i, a in enumerate(items):
            cx, cy = x + (i // 2) * pitch, 16 + (i % 2) * pitch
            tip = f"{a['task']} · {'passed' if a['solved'] else 'failed'} · {a['wall_s']:.1f} s"
            cls = "ok" if a["solved"] else "no"
            parts.append(f'<rect class="{cls}" x="{cx}" y="{cy}" width="{cell}" height="{cell}"><title>{html.escape(tip)}</title></rect>')
        x += cols * pitch + gap_group
    width = x - gap_group
    return (f'<svg class="strip" viewBox="-1 0 {width + 2} {16 + 2 * pitch}" role="img" '
            f'aria-label="{html.escape(e["name"])}: {e["solved"]} of {e["tasks"]} solved">{"".join(parts)}</svg>')


def table_html(entries: list[dict], header: list[str], sub: list[str], rows: list[tuple[str, list[str]]], cls: str) -> str:
    out = [f'<div class="tbl-wrap"><table class="{cls}"><thead><tr><th></th>']
    for h, s in zip(header, sub):
        out.append(f'<th scope="col">{html.escape(h)}<span class="th-sub">{html.escape(s)}</span></th>')
    out.append("</tr></thead><tbody>")
    for label, vals in rows:
        klass = ' class="key"' if label == "Solved" else ""
        tds = "".join(('<td class="nm">' if v == "not metered" else "<td>") + html.escape(v) + "</td>" for v in vals)
        out.append(f'<tr{klass}><th scope="row">{html.escape(label)}</th>{tds}</tr>')
    out.append("</tbody></table></div>")
    return "".join(out)


def metric_rows(entries: list[dict], with_reasoning: bool) -> list[tuple[str, list[str]]]:
    rows = []
    if with_reasoning:
        rows.append(("Thinking", [e["reasoning"] for e in entries]))
        rows.append(("Temperature", [str(e["temperature"]) for e in entries]))
    rows += [
        ("Solved", [f_solved(e) for e in entries]),
        ("Solve rate", [f_pct(e["solve_rate"]) for e in entries]),
        *[(label, [f_lang(e, key) for e in entries]) for key, label, _ in LANGS],
        ("Median wall", [f_s(e["wall_median_s"]) for e in entries]),
        ("Median wall, solved", [f_s(e["solved_wall_median_s"]) for e in entries]),
        ("Total agent time", [f_min(e["wall_total_s"]) for e in entries]),
        ("Model calls / task", [f_calls(e) for e in entries]),
        ("Input tokens", [f_int(e["input_tokens"]) for e in entries]),
        ("Output tokens", [f_int(e["output_tokens"]) for e in entries]),
        ("Reasoning tokens", [f_int(e["reasoning_tokens"]) for e in entries]),
        ("Timeouts", [str(e["timeouts"]) for e in entries]),
        ("Tamper-check fails", [str(e["tamper_check_fails"]) for e in entries]),
    ]
    return rows


def render_report(data: dict, fonts: str, logo: Path | None) -> str:
    validate_report(data)
    models = data["models"]
    grok = data["grok_by_harness"]
    suite = data["suite"]
    langs = suite["languages"]
    lede = (f"{suite['tasks']} repository-repair tasks ({langs['javascript']} JS, {langs['python']} Python, "
            f"{langs['rust']} Rust, {langs['cpp']} C++) · {data['cell']['wall_cap_s']} s limit · graded by the evaluator")

    model_table = table_html(
        models, [m["name"] for m in models], [""] * len(models), metric_rows(models, True), "models")
    pending = "".join(
        f'<p class="pending"><span class="dot"></span>{html.escape(p["name"])}'
        + (f" · thinking {html.escape(p['reasoning'])}" if p.get("reasoning") else "")
        + " · running</p>"
        for p in data["pending"])
    strips = "".join(
        f'<div class="strip-row"><p class="strip-name">{html.escape(m["name"])}'
        f'<span>{m["solved"]} / {m["tasks"]}</span></p>{strip_svg(m)}</div>' for m in models)

    grok_sub = [(f"{g['harness_version']}" if g["harness_version"] else "") for g in grok]
    grok_rows = [("Thinking", [g["reasoning"] for g in grok])] + metric_rows(grok, False)
    grok_table = table_html(grok, [g["harness_name"] for g in grok], grok_sub, grok_rows, "harness")

    def unsolved_list(entries, label_key):
        items = []
        for e in entries:
            names = ", ".join(e["unsolved"]) if e["unsolved"] else "none"
            items.append(f"<li><strong>{html.escape(e[label_key])}</strong> {html.escape(names)}</li>")
        return "<ul class=\"unsolved\">" + "".join(items) + "</ul>"

    unmetered = [m["name"] for m in models + grok if not m["metered"]]
    notes = [
        "Wall: agent time per attempt. Total agent time: sum over all attempts.",
        "Tokens: run totals as metered by the proxy.",
        "Unknown usage fields stay unknown; per-field coverage and reported subtotals are retained in models.json.",
    ]
    if unmetered:
        notes.append(f"{', '.join(sorted(set(unmetered)))}: token usage and model calls are not metered.")
    notes.append(f"Graded by task-native test suites on Prime Intellect evaluators ({data['cell']['evaluator']}) · polyglot-v1.")

    template = (TEMPLATES / "report.html").read_text()
    return fill(
        template,
        FONTS=fonts,
        FONT_LINK=font_link(fonts),
        LOGO=logo_img(logo),
        SUB=html.escape(LETTERHEAD),
        LEDE=html.escape(lede),
        CELL=html.escape(cell_line(data)),
        MODEL_TABLE=model_table,
        PENDING=pending,
        STRIPS=strips,
        GROK_TABLE=grok_table,
        GROK_TITLE=html.escape(f"{grok[0]['name']} by harness" if grok else "By harness"),
        UNSOLVED_MODELS=unsolved_list(models, "name"),
        UNSOLVED_GROK=unsolved_list(grok, "harness_name"),
        NOTES="".join(f"<p>{html.escape(n)}</p>" for n in notes),
        STAMP=html.escape(data["generated_at"]),
    )


# ---------------------------------------------------------------- post

def render_post(data: dict) -> str:
    validate_report(data)
    suite = data["suite"]
    cap = data["cell"]["wall_cap_s"]
    lines = [f"{m['name']}: {m['solved']}/{m['tasks']}" for m in data["models"]]
    heads = [
        f"angelX · polyglot-v1 · {suite['tasks']} tasks (JS, Python, Rust, C++) · one attempt · {cap} s limit",
        f"angelX · polyglot-v1 · {suite['tasks']} tasks · one attempt · {cap} s limit",
        f"angelX · polyglot-v1 · {suite['tasks']} tasks",
    ]
    for head in heads:
        text = head + "\n\n" + "\n".join(lines)
        if len(text) <= POST_LIMIT:
            return text + "\n"
    raise SystemExit(f"post: {len(text)} chars exceeds {POST_LIMIT}")


# ---------------------------------------------------------------- video + card

def render_media(data: dict, fonts: str, out: Path, logo: Path | None) -> None:
    validate_report(data)
    if not data.get("models"):
        raise ValueError("media requires at least one complete model")
    node = shutil.which("node")
    if not node or not shutil.which("chromium") or not shutil.which("ffmpeg"):
        raise SystemExit("video: needs node, chromium and ffmpeg on PATH (or pass --no-video)")
    board = {
        "tasks": data["suite"]["tasks"],
        "cap": data["cell"]["wall_cap_s"],
        "evaluator": data["cell"]["evaluator"],
        "models": [
            {
                "name": m["name"],
                "thinking": m["reasoning"],
                "solved": m["solved"],
                "tasks": m["tasks"],
                "total_s": m["wall_total_s"],
                "attempts": [[1 if a["solved"] else 0, a["wall_s"]] for a in m["attempts"]],
            }
            for m in data["models"]
        ],
    }
    template = (TEMPLATES / "scoreboard.html").read_text()
    capture = TEMPLATES / "capture.cjs"
    with tempfile.TemporaryDirectory(prefix="angelx-release-") as tmp:
        tmp = Path(tmp)
        mark = ""
        if logo:
            (tmp / "assets").mkdir()
            shutil.copy(logo, tmp / "assets" / "logo-mark.png")
            mark = '<img class="mark" src="assets/logo-mark.png" alt="AngelX">'
        for layout, (w, h) in {"square": (1080, 1080), "card": (1200, 675)}.items():
            page = tmp / f"{layout}.html"
            page.write_text(fill(template, FONTS=fonts, FONT_LINK=font_link(fonts), BOARD=json.dumps(board), MARK=mark,
                                 LETTERHEAD=html.escape(LETTERHEAD), LAYOUT=layout, W=str(w), H=str(h)))
        subprocess.run([node, str(capture), "--html", str(tmp / "square.html"), "--size", "1080x1080",
                        "--mp4", str(out / "scoreboard.mp4"), "--png", str(out / "scoreboard-final.png")], check=True)
        subprocess.run([node, str(capture), "--html", str(tmp / "card.html"), "--size", "1200x675",
                        "--png", str(out / "card.png")], check=True)


# ---------------------------------------------------------------- main

def main(argv=None) -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--bench-root", type=Path, default=Path(os.environ.get("ANGELX_BENCH_ROOT", DEFAULT_BENCH)))
    parser.add_argument("--out", type=Path)
    parser.add_argument("--no-video", action="store_true", help="skip scoreboard.mp4, scoreboard-final.png and card.png")
    parser.add_argument("--site", type=Path, help="website/ checkout holding assets/logo-mark.png")
    parser.add_argument("--font-cache", type=Path,
                        default=Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "angelx-release-report")
    args = parser.parse_args(argv)
    bench = args.bench_root.expanduser().resolve()
    out = (args.out or bench / "report-0.1.6").expanduser()
    try:
        data = build_data(bench)
    except (OSError, ValueError) as error:
        parser.exit(2, f"{error}\n")
    if not data["models"]:
        raise SystemExit("no complete angelX runs found")
    fonts = font_css(args.font_cache)
    logo = find_logo(args.site)

    report = render_report(data, fonts, logo)
    post = render_post(data)
    out.mkdir(parents=True, exist_ok=True)
    (out / "models.json").write_text(json.dumps(data, indent=2, allow_nan=False) + "\n")
    (out / "report.html").write_text(report)
    (out / "post.txt").write_text(post)
    if not args.no_video:
        render_media(data, fonts, out, logo)

    for m in data["models"]:
        print(f"{m['name']:<22} {m['solved']:>3} / {m['tasks']}  median {f_s(m['wall_median_s'])}  total {f_min(m['wall_total_s'])}")
    for p in data["pending"]:
        print(f"{p['name']:<22} {p['status']}")
    print(f"post ({len(post.rstrip())} chars):\n{post}")
    for f in sorted(out.iterdir()):
        if f.is_file():
            print(f"  {f.name:<22} {f.stat().st_size:>10,} B")


if __name__ == "__main__":
    main()

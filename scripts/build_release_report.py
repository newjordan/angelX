#!/usr/bin/env python3
"""Release benchmark report for polyglot-v1: models inside angelX, plus Grok 4.7 by harness.

Scores with the bench's own analyze.py (load_rollouts, aggregate) through angelx_board.collect(),
so every count matches the published boards. Each model's newest complete angelX run is used;
a model announced in EXPECTED without a complete run is listed as "running" and left out of the
video, the card and the post.

Writes to --out (default: <bench root>/report-0.1.6/):
  models.json          every number rendered below, plus per-task results
  report.html          self-contained page, light and dark
  post.txt             draft post, numbers only from models.json
  scoreboard.mp4       1080x1080 H.264 yuv420p, 15 s   (skipped with --no-video)
  scoreboard-final.png 1080x1080 last frame
  card.png             1200x675 post card

usage: python3 scripts/build_release_report.py [--bench-root DIR] [--out DIR] [--no-video]
"""

from __future__ import annotations

import argparse
import base64
import html
import json
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
TEMPLATES = HERE / "release-report"
DEFAULT_BENCH = Path.home() / "angel_tests" / "angelX-bench" / "polyglot-20260921"

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
EVALUATOR = "Prime Intellect Verifiers v0.3.1"
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


def entry(board, bench: Path, c: dict) -> dict:
    s = c["summary"]
    cfg = c["config"]
    run_dir = bench / "runs" / c["model"] / c["harness"] / c["run"]
    rows = board.rollouts(run_dir)
    metered = c["metered"]
    model_id = cfg.get("model_id") or c["model"]

    def m(v):
        return v if metered else None

    solved_walls = c["solved_wall_s"]
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
        "metered": metered,
        "solved": s["solved"],
        "tasks": s["rollouts"],
        "solve_rate": s["solve_rate"],
        "by_language": {k: s["by_language"].get(k) for k, _, _ in LANGS},
        "wall_median_s": s["wall_median_s"],
        "solved_wall_median_s": statistics.median(solved_walls) if solved_walls else None,
        "wall_total_s": s["wall_total_s"],
        "calls_per_task": m(s["calls_mean"]),
        "input_tokens": m(s["input_tokens_total"]),
        "uncached_input_tokens": m(s["uncached_input_total"]),
        "cache_hit": m(s["cache_hit"]),
        "output_tokens": m(s["completion_total"]),
        "reasoning_tokens": m(s["reasoning_total"]),
        "timeouts": s["timed_out"],
        "tamper_check_fails": s["integrity_failures"],
        "nonzero_exits": s["nonzero_exit"],
        "unscored": s["unscored"],
        "model_call_errors": m(s["call_errors"]),
        "wall_caps_s": sorted({r["wall_cap_s"] for r in rows if r["wall_cap_s"] is not None}),
        "isolation": sorted({r["isolation"] or "unknown" for r in rows}),
        "rollouts_per_task": rollouts_per_task(bench, c["model"], c["harness"], c["run"]),
        "unsolved": c["unsolved"],
        # execution order, as the traces were written
        "attempts": [
            {
                "task": r["task"],
                "language": r["language"],
                "solved": r["solved"],
                "wall_s": round(r["agent_wall_s"], 3) if r["agent_wall_s"] is not None else None,
            }
            for r in rows
        ],
    }


def build_data(bench: Path) -> dict:
    board = load_board(bench)
    tasks = board.TASKS
    cells = board.collect()
    complete = {}
    for c in cells:
        if c["status"] == "complete" and c["model"] not in complete:
            complete[c["model"]] = c
    models = [entry(board, bench, c) for c in complete.values()]
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
            grok.append(entry(board, bench, c))
    grok.sort(key=lambda g: GROK_HARNESSES.index(g["harness"]))

    everything = models + grok
    caps = sorted({cap for e in everything for cap in e["wall_caps_s"]})
    isolation = sorted({i for e in everything for i in e["isolation"]})
    attempts = sorted({e["rollouts_per_task"] for e in everything if e["rollouts_per_task"] is not None})
    task_list = json.loads((bench / "tasks-polyglot-v1.json").read_text())
    langs = {}
    for t in task_list:
        prefix = (t.get("name") or "").split("-")[0]
        langs[prefix] = langs.get(prefix, 0) + 1
    by_lang = {"javascript": langs.get("js", 0), "python": langs.get("py", 0), "rust": langs.get("rust", 0), "cpp": langs.get("cpp", 0)}

    return {
        "schema": "angelx-release-report/v1",
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC"),
        "suite": {"name": "polyglot-v1", "tasks": tasks, "languages": by_lang},
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
        "reasoning per model: " + ", ".join(f"{m['name']} {m['reasoning']}" for m in data["models"]),
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
            tip = f"{a['task']} · {'solved' if a['solved'] else 'unsolved'} · {a['wall_s']:.1f} s"
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
        rows.append(("Reasoning", [e["reasoning"] for e in entries]))
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


def render_report(data: dict, fonts: str) -> str:
    models = data["models"]
    grok = data["grok_by_harness"]
    suite = data["suite"]
    langs = suite["languages"]
    lede = (f"{suite['tasks']} repository-repair tasks: {langs['javascript']} JavaScript, {langs['python']} Python, "
            f"{langs['rust']} Rust, {langs['cpp']} C++.")

    model_table = table_html(
        models, [m["name"] for m in models], [""] * len(models), metric_rows(models, True), "models")
    pending = "".join(
        f'<p class="pending"><span class="dot"></span>{html.escape(p["name"])}'
        + (f" · reasoning {html.escape(p['reasoning'])}" if p.get("reasoning") else "")
        + " · running</p>"
        for p in data["pending"])
    strips = "".join(
        f'<div class="strip-row"><p class="strip-name">{html.escape(m["name"])}'
        f'<span>{m["solved"]} / {m["tasks"]}</span></p>{strip_svg(m)}</div>' for m in models)

    grok_sub = [(f"{g['harness_version']}" if g["harness_version"] else "") for g in grok]
    grok_rows = [("Reasoning", [g["reasoning"] for g in grok])] + metric_rows(grok, False)
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
    ]
    if unmetered:
        notes.append(f"{', '.join(sorted(set(unmetered)))}: token usage and model calls are not metered.")
    notes.append(f"Evaluator: {data['cell']['evaluator']}.")

    template = (TEMPLATES / "report.html").read_text()
    return fill(
        template,
        FONTS=fonts,
        FONT_LINK=font_link(fonts),
        TITLE="angelX · polyglot-v1",
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

def render_media(data: dict, fonts: str, out: Path) -> None:
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
                "reasoning": m["reasoning"],
                "solved": m["solved"],
                "tasks": m["tasks"],
                "total_s": m["wall_total_s"],
                "attempts": [[1 if a["solved"] else 0, a["wall_s"] or 0] for a in m["attempts"]],
            }
            for m in data["models"]
        ],
    }
    template = (TEMPLATES / "scoreboard.html").read_text()
    capture = TEMPLATES / "capture.cjs"
    with tempfile.TemporaryDirectory(prefix="angelx-release-") as tmp:
        tmp = Path(tmp)
        for layout, (w, h) in {"square": (1080, 1080), "card": (1200, 675)}.items():
            page = tmp / f"{layout}.html"
            page.write_text(fill(template, FONTS=fonts, FONT_LINK=font_link(fonts), BOARD=json.dumps(board),
                                 LAYOUT=layout, W=str(w), H=str(h)))
        subprocess.run([node, str(capture), "--html", str(tmp / "square.html"), "--size", "1080x1080",
                        "--mp4", str(out / "scoreboard.mp4"), "--png", str(out / "scoreboard-final.png")], check=True)
        subprocess.run([node, str(capture), "--html", str(tmp / "card.html"), "--size", "1200x675",
                        "--png", str(out / "card.png")], check=True)


# ---------------------------------------------------------------- main

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--bench-root", type=Path, default=Path(os.environ.get("ANGELX_BENCH_ROOT", DEFAULT_BENCH)))
    parser.add_argument("--out", type=Path)
    parser.add_argument("--no-video", action="store_true", help="skip scoreboard.mp4, scoreboard-final.png and card.png")
    parser.add_argument("--font-cache", type=Path,
                        default=Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "angelx-release-report")
    args = parser.parse_args()
    bench = args.bench_root.expanduser().resolve()
    out = (args.out or bench / "report-0.1.6").expanduser()
    out.mkdir(parents=True, exist_ok=True)

    data = build_data(bench)
    if not data["models"]:
        raise SystemExit("no complete angelX runs found")
    fonts = font_css(args.font_cache)

    (out / "models.json").write_text(json.dumps(data, indent=2) + "\n")
    (out / "report.html").write_text(render_report(data, fonts))
    post = render_post(data)
    (out / "post.txt").write_text(post)
    if not args.no_video:
        render_media(data, fonts, out)

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

#!/usr/bin/env python3
"""Render benchmark plots as static Retina PNGs in GitHub light and dark palettes
for README.md (<picture> + prefers-color-scheme).

Generates:
  docs/images/bench/race-{dark,light}.png
  docs/images/bench/score-{dark,light}.png
  docs/images/bench/bars-{dark,light}.png
  docs/images/bench/seconds-{dark,light}.png
  docs/images/bench/context-{dark,light}.png
  docs/images/bench/cache-{dark,light}.png
  (plus legacy fallback attempts-output-{dark,light}.png)

Usage:
  python3 scripts/render_readme_graphs.py [output_dir]
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT = REPO_ROOT / "docs" / "images" / "bench"
PAGE_W = 900
DATE = "2026-09-21"

THEMES = {
    "dark": dict(
        BG="#0d1117",
        INK="#e6edf3",
        DIM="#9198a1",
        D3="#6e7681",
        FAINT="#7d8590",
        RULE="#30363d",
        HERO="#4493f8",
    ),
    "light": dict(
        BG="#ffffff",
        INK="#1f2328",
        DIM="#59636e",
        D3="#818b98",
        FAINT="#656d76",
        RULE="#d1d9e0",
        HERO="#0969da",
    ),
}

FIGURE_DEFS = [
    {
        "name": "race",
        "fig_id": "tv-race",
        "key": "race",
        "title": "the race to 136",
        "legend": '<span><i class="ax"></i>angelX</span><span><i class="oc"></i>OpenCode</span><span><i class="om"></i>omp</span>',
    },
    {
        "name": "score",
        "fig_id": "tv-score",
        "key": "score",
        "title": "every attempt, placed at the moment it finished",
        "legend": '<span><i class="pass"></i>passed</span><span><i class="fail"></i>failed</span>',
    },
    {
        "name": "bars",
        "fig_id": "tv-bars",
        "key": "bars",
        "title": "output per task",
        "legend": '<span><i class="ax"></i>angelX</span><span><i class="oc"></i>OpenCode</span><span><i class="om"></i>omp</span>',
    },
    {
        "name": "seconds",
        "fig_id": "tv-trace",
        "key": "trace",
        "title": "seconds per attempt",
        "legend": '<span class="hero">■ angelX</span><span>+ OpenCode</span><span>× omp</span>',
    },
    {
        "name": "context",
        "fig_id": "tv-burn",
        "key": "burn",
        "title": "context burned",
        "legend": '<span><i class="ax"></i>angelX</span><span><i class="oc"></i>OpenCode</span><span><i class="om"></i>omp</span>',
    },
    {
        "name": "cache",
        "fig_id": "tv-cache",
        "key": "cache",
        "title": "cache",
        "legend": '<span><i class="ax"></i>angelX</span><span><i class="oc"></i>OpenCode</span><span><i class="om"></i>omp</span>',
    },
]


def make_page(fdef: dict, t: dict) -> str:
    fig_id = fdef["fig_id"]
    key = fdef["key"]
    title = fdef["title"]
    legend = fdef["legend"]
    return f"""<!doctype html><html><head><meta charset="utf-8">
<link href="https://fonts.googleapis.com/css2?family=VT323&display=swap" rel="stylesheet">
<link rel="stylesheet" href="bench.css">
<style>
html,body{{margin:0;background:{t['BG']}}}
body{{width:{PAGE_W}px;padding:22px 10px 12px;box-sizing:border-box}}
.bench{{margin:0;--b-bg:{t['BG']};--b-ink:{t['INK']};--b-dim:{t['DIM']};--b-rule:{t['RULE']}}}
.bench .tv{{background:{t['BG']};margin:0}}
.bench .replay{{display:none}}
.bench .panels{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px 22px}}
.bench .panel-name{{margin:0 0 2px;font-family:var(--b-dos);font-size:17px;letter-spacing:.12em;color:var(--b-dim)}}
.bench svg{{width:100%;height:auto;display:block;overflow:visible}}
.bench svg .vt{{font-family:var(--b-dos)}}
.bench .legend{{display:flex;flex-wrap:wrap;gap:4px 22px;margin:10px 0 0;padding-top:8px;border-top:1px dashed var(--b-rule);font-family:var(--b-dos);font-size:17px;letter-spacing:.06em;color:var(--b-dim)}}
.bench .legend i{{display:inline-block;width:34px;height:6px;margin-right:8px;vertical-align:middle;background-repeat:repeat-x;background-size:var(--w) 6px}}
.bench .legend .ax{{--w:4px;background-image:linear-gradient(90deg,{t['HERO']} 0 2px,transparent 2px)}}
.bench .legend .oc{{--w:8px;background-image:linear-gradient(90deg,{t['DIM']} 0 2px,transparent 2px)}}
.bench .legend .om{{--w:16px;background-image:linear-gradient(90deg,{t['D3']} 0 2px,transparent 2px 4px,{t['D3']} 4px 6px,transparent 6px)}}
.bench .legend .hero{{color:{t['HERO']}}}
.bench .legend .pass{{display:inline-block;width:8px;height:8px;margin-right:6px;vertical-align:middle;background:{t['INK']}}}
.bench .legend .fail{{display:inline-block;width:6px;height:6px;margin-right:6px;vertical-align:middle;border:1px solid {t['DIM']}}}
</style></head><body>
<div class="bench">
  <figure class="tv" id="{fig_id}">
    <figcaption class="tv-title">◇ measured · {title} · {DATE} ◇</figcaption>
    <div class="panels">
      <div><p class="panel-name">DEEPSEEK V4.1 FLASH · THINKING OFF</p>
        <svg id="{key}-deepseek" viewBox="0 0 460 262" role="img"></svg></div>
      <div><p class="panel-name">GLM-5.3-FLASH · THINKING LOW</p>
        <svg id="{key}-glm" viewBox="0 0 460 262" role="img"></svg></div>
    </div>
    <div class="legend">{legend}</div>
  </figure>
</div>
<script>
window.BENCH_THEME = {json.dumps(t)};
window.BENCH_STATIC = true;
window.BENCH_EAGER = true;
</script>
<script src="bench-data.js"></script>
<script src="bench.js"></script>
<script>
document.querySelectorAll('.legend span').forEach(s => {{ if (s.textContent.startsWith('■')) s.classList.add('hero'); }});
var h = Math.ceil(document.body.getBoundingClientRect().height);
var d = document.createElement("div");
d.id = "measured-height";
d.setAttribute("data-height", h);
document.body.appendChild(d);
</script></body></html>"""


def main() -> None:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_OUT
    out_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="angelx_bench_render_") as tmp:
        work_dir = Path(tmp)
        shutil.copy(REPO_ROOT / "website" / "css" / "bench.css", work_dir / "bench.css")
        shutil.copy(REPO_ROOT / "website" / "js" / "bench.js", work_dir / "bench.js")
        shutil.copy(REPO_ROOT / "website" / "js" / "bench-data.js", work_dir / "bench-data.js")

        for fdef in FIGURE_DEFS:
            for theme, t in THEMES.items():
                html_file = work_dir / f"{fdef['name']}-{theme}.html"
                html_file.write_text(make_page(fdef, t))

                res = subprocess.run([
                    "chromium", "--headless=new", "--disable-gpu", "--no-sandbox",
                    "--dump-dom", html_file.as_uri()
                ], capture_output=True, text=True, check=True)

                m = re.search(r'data-height="(\d+)"', res.stdout)
                height = int(m.group(1)) if m else 380

                png_file = out_dir / f"{fdef['name']}-{theme}.png"
                subprocess.run([
                    "chromium", "--headless=new", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                    "--force-device-scale-factor=2", "--virtual-time-budget=6000",
                    f"--window-size={PAGE_W},{height}", f"--screenshot={png_file}", html_file.as_uri()
                ], capture_output=True, check=True)
                print(f"Rendered {png_file.name} ({PAGE_W * 2}x{height * 2})")

        # Copy score as attempts-output fallback for backwards compatibility
        for theme in ("dark", "light"):
            score_png = out_dir / f"score-{theme}.png"
            legacy_png = out_dir / f"attempts-output-{theme}.png"
            if score_png.exists():
                shutil.copy(score_png, legacy_png)
                print(f"Updated legacy fallback {legacy_png.name}")


if __name__ == "__main__":
    main()

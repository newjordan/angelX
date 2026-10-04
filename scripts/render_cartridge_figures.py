#!/usr/bin/env python3
"""Render the cartridge figures for docs/CARTRIDGES.md in the benchmark charts'
panel style (dot-matrix marks, the title set into the frame), as Retina PNGs in
GitHub light and dark palettes.

  docs/images/cartridges/{slot,turn,lines}-{dark,light}.png

The figures read docs/images/cartridges/cartridges.json. `--measure DIR` first
recounts that file's line totals from this checkout and the private cartridge at
DIR (the Yukon cartridge never ships, so its numbers are committed as data).

Usage:
  python3 scripts/render_cartridge_figures.py [--measure ~/.angelX/cartridges/yukon]
"""
from __future__ import annotations

import html
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent
OUT = REPO_ROOT / "docs" / "images" / "cartridges"
DATA = OUT / "cartridges.json"
sys.path.insert(0, str(HERE))
from render_readme_graphs import PAGE_W, THEMES  # noqa: E402  same palettes and width

DOT = 2.8


def sq(x: float, y: float, s: float = DOT) -> str:
    return f"M{x:.1f} {y:.1f}h{s}v{s}h-{s}z"


def rule(x0: float, x1: float, y: float, step: float = 3) -> str:
    d, x = "", x0
    while x < x1:
        d += sq(x, y, 1.4)
        x += step
    return d


def text(x, y, body, size=12.5, fill="INK", anchor="start", cls="vt"):
    # Inline style: bench.css paints `.vt` text in currentColor.
    return (f'<text x="{x}" y="{y}" class="{cls}" font-size="{size}" style="fill:{{{fill}}}" '
            f'text-anchor="{anchor}">{html.escape(body)}</text>')


def mark(x: float, y: float, kind: str) -> str:
    """A 3x3 dot block: filled for Rust hooks, a dotted ring for the toml, an
    outline for an unused plug."""
    out = ""
    for r in range(3):
        for c in range(3):
            px, py = x + c * 3.6, y + r * 3.6
            if kind == "rust":
                out += f'<path d="{sq(px, py)}" fill="{{INK}}"/>'
            elif kind == "toml" and (r, c) != (1, 1):
                out += f'<path d="{sq(px, py)}" fill="{{DIM}}"/>'
    if kind == "none":
        out += (f'<rect x="{x + .5}" y="{y + .5}" width="9.8" height="9.8" fill="none" '
                f'stroke="{{RULE}}" stroke-width="1"/>')
    return out


def slot_svg(data: dict) -> str:
    """Rows: the engine's plug points. Columns: each cartridge's mark."""
    cols = data["cartridges"]
    rows = data["plug_points"]
    top, pitch = 30, 22
    xs = [330, 405]
    body = []
    for i, c in enumerate(cols):
        body.append(text(xs[i] + 5, 16, c["id"], 12, "DIM", "middle"))
    for r, row in enumerate(rows):
        y = top + r * pitch
        body.append(text(8, y + 9, row["label"], 12, "INK"))
        body.append(text(160, y + 9, row["field"], 11, "FAINT"))
        for i, c in enumerate(cols):
            body.append(mark(xs[i], y, c["uses"].get(row["field"], "none")))
    end = top + len(rows) * pitch
    return f'<svg viewBox="0 0 460 {end}" role="img">{"".join(body)}</svg>'


def turn_svg(data: dict) -> str:
    """Rows: a competition turn in order. Columns: what each cartridge does."""
    steps = data["turn"]
    top, pitch = 34, 34
    body = [text(170, 16, data["cartridges"][0]["id"], 12, "DIM"),
            text(320, 16, data["cartridges"][1]["id"], 12, "DIM")]
    line = ""
    for k in range(len(steps) - 1):
        y0 = top + k * pitch + 6
        yy = y0 + 8
        while yy < y0 + pitch - 2:
            line += sq(12.6, yy, 1.4)
            yy += 3
    body.append(f'<path d="{line}" fill="{{RULE}}"/>')
    for k, step in enumerate(steps):
        y = top + k * pitch
        body.append(f'<path d="{sq(10, y, 6)}" fill="{{HERO}}"/>')
        body.append(text(24, y + 6, step["step"], 12.5, "INK"))
        body.append(text(24, y + 19, step["hook"], 10.5, "FAINT"))
        for x, key in ((170, "yukon"), (320, "lean")):
            value = step[key]
            body.append(text(x, y + 6, value, 11.5, "FAINT" if value == "—" else "INK"))
    end = top + len(steps) * pitch - 8
    return f'<svg viewBox="0 0 460 {end}" role="img">{"".join(body)}</svg>'


def lines_svg(data: dict) -> str:
    """Dot-matrix bars: lines of code that ship, and lines that stay yours."""
    unit = data["lines"]["unit"]
    groups = data["lines"]["groups"]
    body, y = [], 14
    for g in groups:
        color = "HERO" if g["ships"] else "D3"
        total = sum(r["lines"] for r in g["rows"])
        body.append(text(8, y, g["title"], 12, "DIM"))
        body.append(text(452, y, f"{total:,} lines", 12, "DIM", "end"))
        y += 10
        for r in g["rows"]:
            y += 16
            body.append(text(8, y + 1, r["name"], 12, "INK"))
            d = ""
            for c in range(max(1, round(r["lines"] / unit))):
                for k in range(2):
                    d += sq(150 + c * 3.4, y - 7 + k * 3.6)
            body.append(f'<path d="{d}" fill="{{{color}}}"/>')
            body.append(text(452, y + 1, f"{r['lines']:,}", 12, "INK", "end", "vt num"))
        y += 24
    body.append(text(8, y, f"one column = {unit} lines", 11, "FAINT"))
    return f'<svg viewBox="0 0 460 {y + 6}" role="img">{"".join(body)}</svg>'


FIGURES = [
    ("slot", "what a cartridge plugs into", slot_svg,
     '<span><i class="rust"></i>Rust hook</span><span><i class="toml"></i>cartridge.toml</span>'
     '<span><i class="none"></i>not used</span>'),
    ("turn", "one competition turn, through the cartridge", turn_svg, ""),
    ("lines", "what ships, and what stays yours", lines_svg,
     '<span><i class="ax"></i>angelX</span><span><i class="om"></i>the yukon cartridge</span>'),
]


def page(title: str, svg: str, legend: str, t: dict) -> str:
    svg = svg.format(**t)
    legend_html = f'<div class="legend">{legend}</div>' if legend else ""
    return f"""<!doctype html><html><head><meta charset="utf-8">
<link href="https://fonts.googleapis.com/css2?family=VT323&display=swap" rel="stylesheet">
<link rel="stylesheet" href="bench.css">
<style>
html,body{{margin:0;background:{t['BG']}}}
body{{width:{PAGE_W}px;padding:22px 10px 12px;box-sizing:border-box}}
.bench{{margin:0;--b-bg:{t['BG']};--b-ink:{t['INK']};--b-dim:{t['DIM']};--b-rule:{t['RULE']}}}
.bench .tv{{background:{t['BG']};margin:0;padding-top:34px}}
.bench .tv-title{{font-size:26px}}
.bench svg{{width:100%;height:auto;display:block;overflow:visible}}
.bench svg .vt{{font-family:var(--b-dos);font-size:inherit}}
.bench svg text{{font-weight:500}}
.bench .legend{{display:flex;flex-wrap:wrap;justify-content:center;gap:4px 28px;margin:12px 0 0;padding-top:10px;
  border-top:1px dashed var(--b-rule);font-family:var(--b-dos);font-size:26px;letter-spacing:.06em;color:var(--b-dim)}}
.bench .legend i{{display:inline-block;width:44px;height:8px;margin-right:8px;vertical-align:middle;background-repeat:repeat-x;background-size:var(--w) 6px}}
.bench .legend .ax{{--w:4px;background-image:linear-gradient(90deg,{t['HERO']} 0 2px,transparent 2px)}}
.bench .legend .om{{--w:4px;background-image:linear-gradient(90deg,{t['D3']} 0 2px,transparent 2px)}}
.bench .legend .rust{{width:12px;height:12px;background:{t['INK']}}}
.bench .legend .toml{{width:12px;height:12px;background:radial-gradient(circle,{t['BG']} 0 2px,{t['DIM']} 2px)}}
.bench .legend .none{{width:12px;height:12px;border:1px solid {t['RULE']}}}
</style></head><body>
<div class="bench"><figure class="tv">
<figcaption class="tv-title">{html.escape(title)}</figcaption>
{svg}
{legend_html}
</figure></div>
<script>
var h = Math.ceil(document.body.getBoundingClientRect().height);
var d = document.createElement("div"); d.id = "measured-height"; d.setAttribute("data-height", h);
document.body.appendChild(d);
</script></body></html>"""


def count(path: Path) -> int:
    return sum(1 for _ in path.open()) if path.is_file() else 0


def measure(cartridge: Path) -> None:
    """Recount every row whose `files` name paths in this checkout (`repo:`) or
    in the private cartridge (`cartridge:`)."""
    data = json.loads(DATA.read_text())
    for group in data["lines"]["groups"]:
        for row in group["rows"]:
            total = 0
            for spec in row["files"]:
                base, pattern = spec.split(":", 1)
                root = REPO_ROOT if base == "repo" else cartridge
                total += sum(count(p) for p in sorted(root.glob(pattern)))
            row["lines"] = total
    DATA.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    print(f"measured {DATA.relative_to(REPO_ROOT)}")


def main() -> None:
    args = sys.argv[1:]
    if args[:1] == ["--measure"]:
        measure(Path(args[1]).expanduser())
    data = json.loads(DATA.read_text())
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="angelx_cartridge_render_") as tmp:
        work = Path(tmp)
        shutil.copy(HERE / "readme-charts" / "bench.css", work / "bench.css")
        for name, title, draw, legend in FIGURES:
            for theme, t in THEMES.items():
                html_file = work / f"{name}-{theme}.html"
                html_file.write_text(page(title, draw(data), legend, t))
                res = subprocess.run(["chromium", "--headless=new", "--disable-gpu", "--no-sandbox",
                                      "--virtual-time-budget=6000", "--dump-dom", html_file.as_uri()],
                                     capture_output=True, text=True, check=True)
                m = re.search(r'data-height="(\d+)"', res.stdout)
                height = int(m.group(1)) if m else 600
                png = OUT / f"{name}-{theme}.png"
                subprocess.run(["chromium", "--headless=new", "--disable-gpu", "--no-sandbox",
                                "--hide-scrollbars", "--force-device-scale-factor=2",
                                "--virtual-time-budget=6000", f"--window-size={PAGE_W},{height}",
                                f"--screenshot={png}", html_file.as_uri()],
                               capture_output=True, check=True)
                print(f"rendered {png.relative_to(REPO_ROOT)} ({PAGE_W * 2}x{height * 2})")


if __name__ == "__main__":
    main()

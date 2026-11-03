#!/usr/bin/env python3
"""Muse Pareto chart: input tokens per task against median wall per task.

Every point is a published receipt in this repo (README bench tables and
docs/telemetry/), never a recomputed claim:

- angelX + Muse Spark, 2026-10-26 (this branch's base): subset of 16,
  docs/telemetry/muse-pareto-baseline-20261026.md.
- angelX + Muse Spark, 2026-09-23: README 0.1.6 table (full 136).
- angelX vs Muse Code on Muse Spark, 2026-09-30:
  docs/telemetry/muse-hermes-h2h-20260930.md (full 136; wall is mean
  agent duration per task there, marked *).
- DeepSeek harness comparison, 2026-09-21: README table (full 136).

Output: an SVG scatter (log x) with the nondominated frontier drawn
through the Muse points. Checkout-only tooling, like its neighbours.
"""
import math
import sys
from pathlib import Path

# (label, input tokens per task, wall seconds per task, solved, group)
POINTS = [
    ("angelX + Spark\n2026-10-26 (16 tasks)", 43_103, 7.82, "16/16", "muse"),
    ("angelX + Spark\n2026-09-23 (136)", 5_900_000 / 136, 25.5, "136/136", "muse"),
    ("angelX + Spark\n2026-09-30 (136)*", 6_700_000 / 136, 50.8 * 60 / 136, "136/136", "muse"),
    ("Muse Code + Spark\n2026-09-30 (136)*", 25_860_000 / 136, 165.7 * 60 / 136, "136/136", "other"),
    ("angelX + DeepSeek\n2026-09-21 (136)", 11_500_000 / 136, 9.9, "133/136", "other"),
    ("OpenCode + DeepSeek\n2026-09-21 (136)", 67_700_000 / 136, 8.6, "132/136", "other"),
    ("oh-my-pi + DeepSeek\n2026-09-21 (93, capped)", 200_100_000 / 93, 15.3, "86/93", "other"),
]

W, H = 1180, 840
LEFT, RIGHT, TOP, BOT = 110, 60, 90, 170

# Per-point label placement: (anchor, dx, dy) so close points do not collide.
PLACEMENT = {
    "angelX + Spark\n2026-10-26 (16 tasks)": ("start", 14, -34),
    "angelX + Spark\n2026-09-23 (136)": ("start", 14, -30),
    "angelX + Spark\n2026-09-30 (136)*": ("start", 14, 22),
    "angelX + DeepSeek\n2026-09-21 (136)": ("start", 14, -16),
}
X0, X1 = math.log10(20_000), math.log10(3_000_000)
Y0, Y1 = 0.0, 85.0


def sx(tok: float) -> float:
    return LEFT + (math.log10(tok) - X0) / (X1 - X0) * (W - LEFT - RIGHT)


def sy(wall: float) -> float:
    return TOP + (Y1 - wall) / (Y1 - Y0) * (H - TOP - BOT)


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;")


def main() -> None:
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("muse-pareto.svg")
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">',
        '<rect width="100%" height="100%" fill="#0d1117"/>',
        '<text x="110" y="48" font-family="monospace" font-size="26" fill="#e6edf3">'
        "Same models, less spent: the harness is the frontier</text>",
        '<text x="110" y="76" font-family="monospace" font-size="15" fill="#8b949e">'
        "Polyglot-v1 repository repair - input tokens per task vs median wall per task (lower-left wins)</text>",
    ]
    # Grid: decades on x, 10 s steps on y.
    for tok, name in [(20_000, "20k"), (50_000, "50k"), (100_000, "100k"), (200_000, "200k"),
                      (500_000, "500k"), (1_000_000, "1M"), (2_000_000, "2M")]:
        x = sx(tok)
        parts.append(f'<line x1="{x:.0f}" y1="{TOP}" x2="{x:.0f}" y2="{H - BOT}" stroke="#21262d"/>')
        parts.append(f'<text x="{x:.0f}" y="{H - BOT + 28}" text-anchor="middle" font-family="monospace" font-size="14" fill="#8b949e">{name}</text>')
    for wall in range(0, 90, 10):
        y = sy(wall)
        parts.append(f'<line x1="{LEFT}" y1="{y:.0f}" x2="{W - RIGHT}" y2="{y:.0f}" stroke="#21262d"/>')
        parts.append(f'<text x="{LEFT - 12}" y="{y + 5:.0f}" text-anchor="end" font-family="monospace" font-size="14" fill="#8b949e">{wall}s</text>')
    parts.append(f'<text x="{(LEFT + W - RIGHT) / 2:.0f}" y="{H - BOT + 62}" text-anchor="middle" font-family="monospace" font-size="16" fill="#c9d1d9">input tokens per task (log)</text>')
    parts.append(f'<text x="30" y="{(TOP + H - BOT) / 2:.0f}" text-anchor="middle" transform="rotate(-90 30 {(TOP + H - BOT) / 2:.0f})" font-family="monospace" font-size="16" fill="#c9d1d9">median wall per task</text>')
    # The Muse points in date order: the frontier moving down-left. The
    # 2026-10-26 point dominates both September points on both axes.
    muse = [p for p in POINTS if p[4] == "muse"][::-1]
    pts = " ".join(f"{sx(p[1]):.0f},{sy(p[2]):.0f}" for p in muse)
    parts.append(f'<polyline points="{pts}" fill="none" stroke="#d29922" stroke-width="2.5" stroke-dasharray="7 5"/>')
    for label, tok, wall, solved, group in POINTS:
        x, y = sx(tok), sy(wall)
        fill = "#d29922" if group == "muse" else "#58a6ff"
        r = 9 if group == "muse" else 7
        parts.append(f'<circle cx="{x:.0f}" cy="{y:.0f}" r="{r}" fill="{fill}" stroke="#0d1117" stroke-width="2"/>')
        anchor, dx, dy = PLACEMENT.get(
            label, ("start" if tok < 400_000 else "end", 14 if tok < 400_000 else -14, -16)
        )
        for i, line in enumerate((label + f"\n{solved}").split("\n")):
            parts.append(
                f'<text x="{x + dx:.0f}" y="{y + dy + i * 17:.0f}" text-anchor="{anchor}" '
                f'font-family="monospace" font-size="13.5" fill="#e6edf3">{esc(line)}</text>'
            )
    for i, note in enumerate([
        "*2026-09-30 wall is mean agent duration per task.",
        "Receipts: README bench tables; docs/telemetry/muse-hermes-h2h-20260930.md;",
        "docs/telemetry/muse-pareto-baseline-20261026.md",
    ]):
        parts.append(
            f'<text x="110" y="{H - 44 + i * 17}" font-family="monospace" font-size="12.5" '
            f'fill="#8b949e">{esc(note)}</text>'
        )
    parts.append("</svg>")
    out.write_text("\n".join(parts) + "\n")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

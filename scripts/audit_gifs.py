#!/usr/bin/env python3
"""Turn the audit frame folders into looping GIFs, 1x and 2x.

usage: audit_gifs.py <frames-dir> <out-dir> [game ...]

Each <frames-dir>/<game>/f0000.png ... (15 fps) becomes <out-dir>/<game>.gif
(1x) and <out-dir>/<game>-2x.gif (2x, nearest neighbour). Frames are flattened
on black; one palette per GIF; loops forever.
"""
import subprocess
import sys
from pathlib import Path


def main() -> None:
    frames, out = Path(sys.argv[1]), Path(sys.argv[2])
    only = set(sys.argv[3:])
    out.mkdir(parents=True, exist_ok=True)
    for d in sorted(p for p in frames.iterdir() if p.is_dir()):
        if only and d.name not in only:
            continue
        if not (d / "f0000.png").exists():
            continue
        for scale, suffix in ((1, ""), (2, "-2x")):
            vf = (
                f"scale=iw*{scale}:ih*{scale}:flags=neighbor,"
                "split[a][b];[a]palettegen=max_colors=128:stats_mode=full[p];"
                "[b][p]paletteuse=dither=none"
            )
            subprocess.run(
                [
                    "ffmpeg", "-y", "-loglevel", "error", "-framerate", "15",
                    "-i", str(d / "f%04d.png"), "-vf", vf, "-loop", "0",
                    str(out / f"{d.name}{suffix}.gif"),
                ],
                check=True,
            )
        print("gif", d.name)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Bard tracks, step 5: lay the game's own sound schedule on a timeline, and check it.

The schedule is what the sim's `Run::seat_sounds` held for the Bard, tick by
tick (`write_bard_demo_schedule` in the Bard's tests): `[[tick, "bard_a_000"], ...]`
at 30 ticks a second. Every named slice is decoded (as the player does) and
placed at its tick's time; the result is written as an mp3 with a spectrogram,
and measured for loudness, slice-edge clicks and sync drift. The schedule may also be
`{"events": [...], "holds": [[from_tick, to_tick, "label"], ...]}`; the holds are drawn on the spectrogram.

    python3 demo.py SFX_DIR SCHEDULE.json OUT_DIR NAME DRUMS_REF.wav
"""
import json
import os
import pathlib
import re
import subprocess
import sys

import numpy as np
import scipy.signal as ss
import soundfile as sf

SR = 44100
HZ = 30
BEAT = int(0.4 * SR)


def decode(path):
    raw = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(path), "-f", "f32le", "-ac", "2", "-ar", str(SR), "-"],
                         check=True, capture_output=True).stdout
    return np.frombuffer(raw, dtype="<f4").reshape(-1, 2).astype(np.float64)


def render(sfx, events, tail_s=1.0):
    cache = {}
    end = 0
    placed = []
    for tick, name in events:
        f = pathlib.Path(sfx) / f"{name}.mp3"
        if not f.is_file():
            raise SystemExit(f"missing {f}")
        if name not in cache:
            cache[name] = decode(f)
        a = int(round(tick * SR / HZ))
        placed.append((a, name))
        end = max(end, a + len(cache[name]))
    out = np.zeros((end + int(tail_s * SR), 2))
    for a, name in placed:
        out[a:a + len(cache[name])] += cache[name]
    return out, placed, cache


def lufs(x, tmp):
    sf.write(tmp, x, SR, subtype="FLOAT")
    r = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", tmp, "-af", "ebur128=peak=true", "-f", "null", "-"], capture_output=True, text=True)
    i = re.findall(r"I:\s+(-?[\d.]+) LUFS", r.stderr)
    tp = re.findall(r"Peak:\s+(-?[\d.]+) dBFS", r.stderr)
    return (float(i[-1]) if i else None), (float(tp[-1]) if tp else None)


def click_report(out, placed, cache):
    """At every slice boundary: the jump between consecutive samples next to it, against the
    typical jump nearby (a click is a jump far above its neighbourhood)."""
    mono = out.mean(axis=1)
    d = np.abs(np.diff(mono))
    ratios = []
    for a, name in placed:
        if a < 600 or a + 600 >= len(d):
            continue
        near = d[a - 8:a + 8].max()
        around = np.percentile(d[a - 600:a + 600], 99.5) + 1e-6
        ratios.append(near / around)
    ratios = np.array(ratios) if ratios else np.array([0.0])
    return {"boundaries": len(ratios), "max_jump_ratio": round(float(ratios.max()), 2), "p95_jump_ratio": round(float(np.percentile(ratios, 95)), 2)}


def sync_report(out, holds, ref):
    """Sync against the schedule. The drums alone, as the build stretched them onto the 0.4 s grid
    (`drums_ref.wav`), are laid where the ticks say each bar of a hold begins; the lag at which the
    demo's own audio lines up with them, bar by bar from the drop on (the bars with the drums in),
    is how far the music is from the volleys: a flat line near 0 is sync, a slope is drift."""
    ref, rsr = sf.read(ref, dtype="float64")
    assert rsr == SR
    mono = out.mean(axis=1)
    lags, bars = [], []
    for from_tick, to_tick, label in holds:
        if not label.startswith("hold"):
            continue
        first_bar = 3  # bars 1-3 are the lead and the bass: the drums are in from bar 4
        n = 0
        while True:
            beat = (first_bar + n) * 4
            start = int(round((from_tick + 12 * beat) * SR / HZ))
            end = start + 4 * BEAT
            if end > len(mono) or (from_tick + 12 * (beat + 4)) > to_tick or (beat + 4) * BEAT > len(ref):
                break
            want = ref[beat * BEAT:(beat + 4) * BEAT]
            pad = int(0.08 * SR)
            have = mono[start - pad:end + pad]
            c = ss.correlate(have, want, mode="valid", method="fft")
            lag = (int(np.argmax(c)) - pad) / SR * 1000
            lags.append(lag)
            bars.append(n)
            n += 1
    lags = np.array(lags) if lags else np.array([0.0])
    slope = float(np.polyfit(bars, lags, 1)[0]) if len(lags) > 2 else 0.0
    return {"bars": len(lags), "median_lag_ms": round(float(np.median(lags)), 2), "max_abs_lag_ms": round(float(np.abs(lags).max()), 2),
            "drift_ms_per_bar": round(slope, 3)}


def spectrogram(out, path, title, holds=()):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    f, t, s = ss.spectrogram(out.mean(axis=1), SR, nperseg=2048, noverlap=1536)
    fig, ax = plt.subplots(figsize=(16, 4.2), dpi=100)
    ax.pcolormesh(t, f, 10 * np.log10(s + 1e-12), shading="auto", vmin=-110, vmax=-30, cmap="magma")
    ax.set_ylim(0, 12000)
    for a, b, label in holds:
        ax.axvline(a / HZ, color="white", lw=0.8, alpha=0.8)
        ax.axvline(b / HZ, color="cyan", lw=0.8, alpha=0.8, ls="--")
        ax.text(a / HZ + 0.15, 11200, label, color="white", fontsize=8, va="top")
    ax.set_xlim(0, len(out) / SR)
    ax.set_xlabel("seconds")
    ax.set_ylabel("Hz")
    ax.set_title(title)
    fig.tight_layout()
    fig.savefig(path)
    plt.close(fig)


def main(sfx, schedule, out_dir, name, ref):
    sched = json.loads(pathlib.Path(schedule).read_text())
    holds = sched.get("holds", []) if isinstance(sched, dict) else []
    events = sched["events"] if isinstance(sched, dict) else sched
    events = [(t, n) for t, n in events if n.startswith("bard_")]
    out, placed, cache = render(sfx, events)
    out_dir = pathlib.Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    wav = out_dir / f"{name}.wav"
    # headroom: slices are at the effects' level; the demo is the sum of what the game plays
    sf.write(wav, out, SR, subtype="PCM_16")
    subprocess.run(["ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-i", str(wav), "-c:a", "libmp3lame", "-b:a", "192k", str(out_dir / f"{name}.mp3")], check=True)
    wav.unlink()
    l, tp = lufs(out, os.path.join(os.environ.get("TMPDIR", "/tmp"), "_demo_l.wav"))
    rep = {"name": name, "seconds": round(len(out) / SR, 2), "slices": len(placed), "lufs": l, "true_peak_db": tp,
           "clicks": click_report(out, placed, cache), "sync_vs_ticks": sync_report(out, holds, ref)}
    spectrogram(out, out_dir / f"{name}.png", f"{name}: the Bard's own sounds on the game's schedule ({rep['seconds']} s, {l} LUFS)", holds)
    (out_dir / f"{name}.check.json").write_text(json.dumps(rep, indent=1))
    print(json.dumps(rep))


if __name__ == "__main__":
    main(*sys.argv[1:6])

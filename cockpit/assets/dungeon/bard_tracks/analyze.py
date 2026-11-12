#!/usr/bin/env python3
"""Bard tracks, step 2: the beat grid and the section map of each song.

Reads the six htdemucs_6s stems of every source song (`stems6/<id>_(Stem)_htdemucs_6s.wav`),
folds them into three layers (high synth, bass, drums), finds the kick grid
(tempo and phase), the downbeat and the bars, then labels every bar by which
layers are playing and cuts the song into sections.

    python3 analyze.py STEMS6_DIR OUT_DIR [id ...]

Writes `OUT_DIR/<id>.json`: the kick grid (tempo, phase, how loose), the downbeat, the level of each
layer per bar, and the sections. numpy, scipy and soundfile only.
"""
import json
import pathlib
import sys

import numpy as np
import scipy.signal as ss
import soundfile as sf

LEAD_STEMS = ("Other", "Guitar", "Piano", "Vocals")  # the songs are instrumentals: a "vocals" stem is lead synth


def load_layers(stems6, sid):
    """(lead, bass, drums) as float32 stereo arrays at 48 kHz."""
    def get(name):
        x, sr = sf.read(pathlib.Path(stems6) / f"{sid}_({name})_htdemucs_6s.wav", dtype="float32")
        assert sr == 48000, sr
        return x
    lead = sum(get(n) for n in LEAD_STEMS)
    return lead, get("Bass"), get("Drums")


def db(x, floor=-90.0):
    return float(max(floor, 20 * np.log10(np.sqrt(np.mean(np.square(x))) + 1e-9)))


def kick_grid(drums, sr, prior_bpm):
    """(period_s, t0_s, residual_ms): the kick grid by a robust line fit."""
    m = drums.mean(axis=1)
    b, a = ss.butter(4, 140 / (sr / 2), "low")
    low = ss.filtfilt(b, a, m)
    env = ss.fftconvolve(np.abs(low), np.hanning(int(sr * 0.01)), "same")
    hop = 48  # 1 ms
    e = env[::hop]
    fs = sr / hop
    flux = np.maximum(0, np.diff(e, prepend=e[0]))
    # tempo by autocorrelation near the prior
    lo, hi = int(fs * 60 / (prior_bpm * 1.06)), int(fs * 60 / (prior_bpm * 0.94))
    ac = np.correlate(flux, flux, "full")[len(flux) - 1:]
    lag0 = lo + int(np.argmax(ac[lo:hi]))
    # sub-lag refine on a parabola
    y0, y1, y2 = ac[lag0 - 1], ac[lag0], ac[lag0 + 1]
    lag = lag0 + 0.5 * (y0 - y2) / (y0 - 2 * y1 + y2 + 1e-12)
    period = lag / fs
    # kick peaks
    peaks, _ = ss.find_peaks(flux, height=np.percentile(flux, 97) * 0.5, distance=int(fs * period * 0.6))
    t = peaks / fs
    # robust fit t = t0 + k * period, assigning peaks to the nearest slot
    t0 = t[np.argmax(flux[peaks])] if len(t) else 0.0
    for _ in range(6):
        k = np.round((t - t0) / period)
        res = t - (t0 + k * period)
        keep = np.abs(res) < 0.03
        if keep.sum() < 8:
            break
        A = np.vstack([np.ones(keep.sum()), k[keep]]).T
        (t0, period), *_ = np.linalg.lstsq(A, t[keep], rcond=None)
    k = np.round((t - t0) / period)
    res = t - (t0 + k * period)
    keep = np.abs(res) < 0.03
    # move t0 to the earliest slot so the grid starts near 0
    n = np.floor(t0 / period)
    t0 = t0 - n * period
    return float(period), float(t0), float(np.std(res[keep]) * 1000 if keep.any() else 99.0), int(keep.sum())


def bar_table(lead, bass, drums, sr, period, t0, beats_per_bar=4):
    """Per-beat levels (dB) of each layer on the grid."""
    beat = int(round(period * sr))
    n = int((lead.shape[0] / sr - t0) / period)
    rows = []
    for i in range(n):
        s = int(round((t0 + i * period) * sr))
        e = s + beat
        rows.append([db(lead[s:e]), db(bass[s:e]), db(drums[s:e])])
    return np.array(rows)


def full_beats(levels):
    """Beats where lead, bass and drums all play (each within its own band of the song's loud level)."""
    ref = np.percentile(levels, 90, axis=0)
    return np.all(levels > (ref - np.array([14.0, 14.0, 12.0])), axis=1)


def downbeat_phase(levels, kick_low=None):
    """Which beat (0-3) starts a bar. A drop lands on a bar line: every place the full band
    comes back after a gap votes for its own beat; the layers' change at each beat breaks ties."""
    full = full_beats(levels)
    votes = np.zeros(4)
    i = 4
    while i < len(full):
        if full[i] and not full[i - 4:i].any():
            run = 0
            while i + run < len(full) and full[i + run]:
                run += 1
            if run >= 16:
                votes[i % 4] += min(run, 64)
            i += run
        else:
            i += 1
    d = np.abs(np.diff(levels, axis=0, prepend=levels[:1])).sum(axis=1)
    novelty = np.array([d[p::4].sum() for p in range(4)])
    score = votes + 1e-3 * novelty
    return int(np.argmax(score)), [round(float(v), 1) for v in votes]


def label_bars(levels_bar):
    """Label bars by the layers playing: L lead, B bass, D drums (a layer is 'on'
    above a floor relative to the song's own loud level)."""
    ref = np.percentile(levels_bar, 90, axis=0)
    on = levels_bar > (ref - np.array([14.0, 14.0, 12.0]))
    labels = ["".join(c for c, f in zip("LBD", row) if f) or "-" for row in on]
    return labels, on


def sections(labels, levels_bar):
    """Cut bars into runs of the same label; name them."""
    out = []
    start = 0
    for i in range(1, len(labels) + 1):
        if i == len(labels) or labels[i] != labels[start]:
            out.append((start, i, labels[start]))
            start = i
    # merge runs shorter than 2 bars into the previous one
    merged = []
    for s, e, lab in out:
        if merged and (e - s) < 2:
            ps, pe, pl = merged[-1]
            merged[-1] = (ps, e, pl)
        else:
            merged.append((s, e, lab))
    named = []
    for s, e, lab in merged:
        lead_db, bass_db, drum_db = [round(float(v), 1) for v in levels_bar[s:e].mean(axis=0)]
        if lab == "LBD":
            name = "drop"
        elif "D" in lab and "L" not in lab:
            name = "intro" if not named else "build"
        elif lab in ("L", "LB", "B"):
            name = "breakdown"
        elif "D" in lab:
            name = "build"
        else:
            name = "gap"
        named.append({"from_bar": int(s), "to_bar": int(e), "bars": int(e - s), "layers": lab, "name": name,
                      "lead_db": lead_db, "bass_db": bass_db, "drums_db": drum_db})
    return named


def analyze(stems6, sid, prior_bpm, out):
    lead, bass, drums = load_layers(stems6, sid)
    sr = 48000
    period, t0, jitter_ms, kicks = kick_grid(drums, sr, prior_bpm)
    levels = bar_table(lead, bass, drums, sr, period, t0)
    phase, score = downbeat_phase(levels, None)
    # bars start at beat `phase`
    lv = levels[phase:]
    nbars = len(lv) // 4
    bar = lv[: nbars * 4].reshape(nbars, 4, 3)
    bar_db = 10 * np.log10(np.mean(10 ** (bar / 10), axis=1) + 1e-12)
    labels, on = label_bars(bar_db)
    secs = sections(labels, bar_db)
    res = {
        "id": sid, "bpm": round(60 / period, 3), "period_s": period, "t0_s": t0,
        "kick_jitter_ms": round(jitter_ms, 2), "kicks_fit": kicks,
        "downbeat_beat": phase, "downbeat_scores": [round(float(s), 1) for s in score],
        "bar0_s": t0 + phase * period, "bars": int(nbars),
        "bar_labels": labels, "bar_db": np.round(bar_db, 1).tolist(), "sections": secs,
    }
    pathlib.Path(out).mkdir(parents=True, exist_ok=True)
    (pathlib.Path(out) / f"{sid}.json").write_text(json.dumps(res, indent=1))
    return res


if __name__ == "__main__":
    stems6, out = sys.argv[1], sys.argv[2]
    priors = json.loads((pathlib.Path(__file__).parent / "songs.json").read_text())
    ids = sys.argv[3:] or list(priors)
    for sid in ids:
        r = analyze(stems6, sid, priors[sid]["bpm"], out)
        print(sid, f"{r['bpm']:.2f} bpm", f"jitter {r['kick_jitter_ms']} ms", f"kicks {r['kicks_fit']}",
              f"bars {r['bars']}", f"downbeat {r['downbeat_beat']}")
        print("  ", "".join({"LBD": "#", "LB": "b", "L": "l", "BD": "k", "D": "d", "B": "_", "LD": "x", "-": "."}.get(l, "?") for l in r["bar_labels"]))
        for s in r["sections"]:
            print("   ", s)

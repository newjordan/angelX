#!/usr/bin/env python3
"""Bard tracks, step 4: stretch an excerpt to the Bard's tempo, pre-mix the three
tiers and cut them into one-beat slices.

The Bard looses a volley every 4 ticks at 30 Hz, so a beat of 3 volleys is
exactly 0.4 s: 150 BPM. Each song's excerpt (`pick.py`) is stretched with its
pitch kept (rubberband, the finer engine) so its own beats fall on that grid,
then mixed per tier (1 high synth, 2 + bassline, 3 + drums), levelled and cut
into 0.4 s slices, each with a short complementary fade at both edges so the
slices butt together without a click, and encoded as 96 kbit/s stereo mp3 at
44.1 kHz like the rest of the effects.

    python3 build.py STEMS6_DIR ANALYSIS_DIR WORK_DIR OUT_SFX_DIR SONG_LETTER=ID ...

Needs numpy, scipy, soundfile, and ffmpeg and rubberband on the path.
"""
import json
import pathlib
import re
import subprocess
import sys

import numpy as np
import scipy.signal as ss
import soundfile as sf

SR_SRC = 48000
SR = 44100
BEAT_S = 0.4                       # 3 volleys of 4 ticks at 30 Hz
BEAT = int(round(BEAT_S * SR))     # 17640 samples
BEATS = 96                         # 24 bars of 4
LOOP_FROM_BEAT = 32                # held past the last slice, the song goes round again from bar 9
FADE = 705                         # 16 ms: the edge fade, and the tail each slice carries into the next beat
FIRST_FADE = 132                   # 3 ms: the song's first slice has nothing before it to crossfade with
TIER_AT_BEAT = (0, 4, 12)          # tier 1 on bar 1, tier 2 on bars 2-3, tier 3 from bar 4 (the drop)
TIER_LUFS = (-22.0, -20.5, -19.5)  # the effects sit at a median of -19.3 LUFS
CEILING = 0.794                    # -2 dBFS, like the rest of the effects
LULLABY_BEATS = 5                  # 2 s, the Lullaby's length (`bard::SONG_TICKS`), plus a fade tail
LULLABY_TAIL_S = 0.5

LEAD_STEMS = ("Other", "Guitar", "Piano", "Vocals")


def tier_of_beat(b):
    return 0 if b < TIER_AT_BEAT[1] else (1 if b < TIER_AT_BEAT[2] else 2)


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, **kw)


def read_stem(stems6, sid, name, t0, t1):
    path = pathlib.Path(stems6) / f"{sid}_({name})_htdemucs_6s.wav"
    a, b = int(t0 * SR_SRC), int(t1 * SR_SRC)
    x, sr = sf.read(path, start=max(a, 0), stop=b, dtype="float64", always_2d=True)
    assert sr == SR_SRC
    return x


def local_grid(drums_stem_path, start_s, period_s, beats):
    """Refit the kick grid over the excerpt itself: (period, t of the excerpt's first beat, rms residual ms)."""
    lo = start_s - 1.0
    hi = start_s + beats * period_s + 1.0
    x, sr = sf.read(drums_stem_path, start=int(lo * SR_SRC), stop=int(hi * SR_SRC), dtype="float64", always_2d=True)
    m = x.mean(axis=1)
    bq, aq = ss.butter(4, 140 / (sr / 2), "low")
    low = ss.filtfilt(bq, aq, m)
    env = ss.fftconvolve(np.abs(low), np.hanning(int(sr * 0.006)), "same")
    hop = 48
    e = env[::hop]
    fs = sr / hop
    flux = np.maximum(0, np.diff(e, prepend=e[0]))
    t_pred0 = start_s - lo
    period = period_s
    first = t_pred0
    for _ in range(3):
        ks, ts = [], []
        for k in range(-3, beats + 3):
            c = first + k * period
            a, b = int((c - 0.045) * fs), int((c + 0.045) * fs)
            if a < 0 or b >= len(flux):
                continue
            seg = flux[a:b]
            if seg.max() < 0.15 * flux.max():
                continue
            ks.append(k)
            ts.append((a + int(np.argmax(seg))) / fs)
        ks, ts = np.array(ks), np.array(ts)
        A = np.vstack([np.ones(len(ks)), ks]).T
        (first, period), *_ = np.linalg.lstsq(A, ts, rcond=None)
    res = ts - (first + ks * period)
    return float(period), float(first + lo), float(np.std(res) * 1000), len(ks)


def stretch(x, ratio, tmp):
    """Time-stretch float64 stereo `x` by `ratio` (output/input length), pitch kept."""
    tmp = pathlib.Path(tmp)
    sf.write(tmp / "in.wav", x, SR_SRC, subtype="FLOAT")
    run(["nice", "-n", "10", "rubberband", "-3", "-t", f"{ratio:.9f}", "--no-threads", "-q",
         str(tmp / "in.wav"), str(tmp / "out.wav")])
    y, sr = sf.read(tmp / "out.wav", dtype="float64", always_2d=True)
    assert sr == SR_SRC
    return y


def resample(x):
    return ss.resample_poly(x, 147, 160, axis=0)  # 48 kHz -> 44.1 kHz


def highpass(x, hz, sr=SR_SRC):
    sos = ss.butter(4, hz / (sr / 2), "high", output="sos")
    return ss.sosfilt(sos, x, axis=0)


def lufs(x, sr, tmp):
    """Integrated loudness by ffmpeg's ebur128 (BS.1770, gated)."""
    tmp = pathlib.Path(tmp)
    sf.write(tmp / "l.wav", x, sr, subtype="FLOAT")
    r = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", str(tmp / "l.wav"), "-af", "ebur128=peak=true", "-f", "null", "-"],
                       capture_output=True, text=True)
    m = re.findall(r"I:\s+(-?[\d.]+) LUFS", r.stderr)
    return float(m[-1]) if m else -70.0


def limit(x, sr, tmp):
    """-2 dBFS true-peak-ish ceiling with ffmpeg's look-ahead limiter."""
    tmp = pathlib.Path(tmp)
    sf.write(tmp / "lim_in.wav", x, sr, subtype="FLOAT")
    run(["ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-i", str(tmp / "lim_in.wav"),
         "-af", f"alimiter=limit={CEILING}:attack=3:release=40:level=disabled", "-c:a", "pcm_f32le", str(tmp / "lim_out.wav")])
    y, _ = sf.read(tmp / "lim_out.wav", dtype="float64", always_2d=True)
    return y


def edge(n_head, n_tail, length_total):
    """(head, tail) windows: raised-cosine complementary pair (sin^2, cos^2)."""
    head = np.sin(0.5 * np.pi * (np.arange(n_head) + 0.5) / n_head) ** 2
    tail = np.cos(0.5 * np.pi * (np.arange(n_tail) + 0.5) / n_tail) ** 2
    return head, tail


def slice_audio(mix, beat_index, first):
    """One-beat slice `beat_index` of a continuous 44.1 kHz mix, with its edge fades; the tail
    carries FADE samples of the next beat so a perfectly timed successor crossfades into it."""
    a = beat_index * BEAT
    seg = mix[a:a + BEAT + FADE].copy()
    head, _ = edge(FIRST_FADE if first else FADE, FADE, len(seg))
    seg[:len(head)] *= head[:, None]
    _, tail = edge(FADE, FADE, len(seg))
    seg[-FADE:] *= tail[:, None]
    return seg


def kick_sync(drums, sr=SR):
    """How the stretched drums' kicks sit on the 0.4 s grid: (median ms, std ms, ms drift per beat, kicks used)."""
    bq, aq = ss.butter(4, 140 / (sr / 2), "low")
    low = ss.filtfilt(bq, aq, drums.mean(axis=1))
    env = ss.fftconvolve(np.abs(low), np.hanning(int(sr * 0.006)), "same")
    flux = np.maximum(0, np.diff(env, prepend=env[0]))
    thr = 0.3 * np.percentile(flux, 99.5)
    offs, bs = [], []
    for b in range(2, BEATS - 2):
        a = b * BEAT - int(0.05 * sr)
        seg = flux[a:a + int(0.1 * sr)]
        if seg.max() < thr:
            continue
        offs.append((int(np.argmax(seg)) - int(0.05 * sr)) / sr * 1000)
        bs.append(b)
    offs, bs = np.array(offs), np.array(bs)
    if len(offs) < 8:
        return None
    return {"median_ms": round(float(np.median(offs)), 1), "std_ms": round(float(offs.std()), 1),
            "drift_ms_per_beat": round(float(np.polyfit(bs, offs, 1)[0]), 3), "kicks": int(len(offs))}


def encode(seg, path, tmp):
    tmp = pathlib.Path(tmp)
    sf.write(tmp / "s.wav", seg, SR, subtype="PCM_24")
    run(["ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-i", str(tmp / "s.wav"), "-c:a", "libmp3lame",
         "-b:a", "96k", "-joint_stereo", "1", "-ar", str(SR), "-ac", "2", "-write_xing", "1", "-id3v2_version", "0",
         "-write_id3v1", "0", str(path)])


def lullaby_start(an, ex):
    """The lullaby is a breakdown or pad phrase: the loudest lead-and-pads stretch with no drums,
    at least two bars long, else the excerpt's own first bar."""
    best = None
    for s in an["sections"]:
        if "D" in s["layers"] or "L" not in s["layers"] or s["bars"] < 2:
            continue
        if best is None or s["lead_db"] > best["lead_db"]:
            best = s
    if best is None:
        return ex["start_bar"], "excerpt bar 1 (no breakdown)"
    # a bar of it, one bar in when it is long enough (past the entrance)
    bar = best["from_bar"] + (1 if best["bars"] >= 3 else 0)
    return bar, f"{best['name']} bars {best['from_bar']}-{best['to_bar']} ({best['layers']})"


def build_song(letter, sid, stems6, an_dir, work, out_sfx, report):
    an = json.loads((pathlib.Path(an_dir) / f"{sid}.json").read_text())
    ex = json.loads((pathlib.Path(an_dir) / f"{sid}.excerpt.json").read_text())
    work = pathlib.Path(work) / sid
    work.mkdir(parents=True, exist_ok=True)
    drums_path = pathlib.Path(stems6) / f"{sid}_(Drums)_htdemucs_6s.wav"
    period, first_beat_s, resid, nk = local_grid(drums_path, ex["start_s"], an["period_s"], BEATS)
    ratio = BEAT_S / period
    pre, post = 0.6, 1.2  # source seconds around the excerpt (the tail of the last slice, the stretch's warm-up)
    t0 = first_beat_s - pre
    t1 = first_beat_s + BEATS * period + post
    layers = {}
    lead = sum(read_stem(stems6, sid, n, t0, t1) for n in LEAD_STEMS)
    layers["lead"] = highpass(lead, 140.0)
    layers["bass"] = read_stem(stems6, sid, "Bass", t0, t1)
    layers["drums"] = read_stem(stems6, sid, "Drums", t0, t1)
    n = BEATS * BEAT + BEAT * 2
    for attempt in range(3):
        full = {k: resample(stretch(v, ratio, work)) for k, v in layers.items()}
        off = int(round(pre * ratio * SR))

        def cut(off):
            c = {k: v[off:off + n] for k, v in full.items()}
            m = min(len(v) for v in c.values())
            return {k: v[:m] for k, v in c.items()}

        st = cut(off)
        # the kicks' drift along the 0.4 s grid says the tempo fit was a hair off: stretch again by it
        ks = kick_sync(st["drums"])
        if not ks or abs(ks["drift_ms_per_beat"]) < 0.08:
            break
        ratio *= 1 - ks["drift_ms_per_beat"] / (BEAT_S * 1000)
    # land the median kick a hair after the slice start (the kick is the beat the volley's tick calls)
    first = kick_sync(st["drums"])
    if first:
        off += int(round((first["median_ms"] - 2.0) / 1000 * SR))
        st = cut(off)
    mixes = [st["lead"], st["lead"] + st["bass"], st["lead"] + st["bass"] + st["drums"]]
    span = [(0, TIER_AT_BEAT[1]), (TIER_AT_BEAT[1], TIER_AT_BEAT[2]), (TIER_AT_BEAT[2], BEATS)]
    gains = []
    final = []
    for t, (mix, (b0, b1)) in enumerate(zip(mixes, span)):
        # the loudness is measured over the beats the tier plays; tiers 1 and 2 are short, so
        # measure over the whole phrase from bar 4 on for the full band and the same stretch for the others
        seg = mix[b0 * BEAT:b1 * BEAT]
        if t < 2:
            seg = mix[TIER_AT_BEAT[2] * BEAT:BEATS * BEAT]  # steady phrase of the same layers, longer window
        l0 = lufs(seg, SR, work)
        g = 10 ** ((TIER_LUFS[t] - l0) / 20)
        gains.append(g)
        y = limit(mix * g, SR, work)
        final.append(y)
    # lullaby: breakdown / pad phrase, lead only, 5 beats and a fade tail
    lbar, lwhere = lullaby_start(an, ex)
    l_t0 = an["bar0_s"] + lbar * 4 * an["period_s"]
    # no drums to fit a grid to in a breakdown: the song's own grid places the bar
    lp, lfirst, lres = an["period_s"], l_t0, 0.0
    lratio = BEAT_S / lp
    lt0 = lfirst - 0.4
    lt1 = lfirst + (LULLABY_BEATS + 3) * lp
    lead_l = sum(read_stem(stems6, sid, n_, lt0, lt1) for n_ in LEAD_STEMS)
    bass_l = read_stem(stems6, sid, "Bass", lt0, lt1)
    pad = highpass(lead_l, 140.0) + 0.7 * bass_l
    pad = resample(stretch(pad, lratio, work))
    loff = int(round(0.4 * lratio * SR))
    ln = LULLABY_BEATS * BEAT + int(LULLABY_TAIL_S * SR)
    pad = pad[loff:loff + ln]
    fade_in = np.sin(0.5 * np.pi * (np.arange(int(0.012 * SR)) + 0.5) / int(0.012 * SR)) ** 2
    pad[:len(fade_in)] *= fade_in[:, None]
    tail_n = int(LULLABY_TAIL_S * SR)
    pad[-tail_n:] *= np.cos(0.5 * np.pi * (np.arange(tail_n) + 0.5) / tail_n)[:, None] ** 2
    lg = 10 ** ((TIER_LUFS[1] - lufs(pad, SR, work)) / 20)
    pad = limit(pad * lg, SR, work)
    out_sfx = pathlib.Path(out_sfx)
    out_sfx.mkdir(parents=True, exist_ok=True)
    for b in range(BEATS):
        seg = slice_audio(final[tier_of_beat(b)], b, b == 0)
        encode(seg, out_sfx / f"bard_{letter}_{b:03d}.mp3", work)
    encode(pad, out_sfx / f"bard_{letter}_lull.mp3", work)
    info = {
        "letter": letter, "id": sid, "title": json.loads((pathlib.Path(__file__).parent / "songs.json").read_text())[sid]["title"],
        "source_bpm": round(60 / period, 3), "stretch_ratio": round(ratio, 5), "speed": round(1 / ratio, 4), "source_bpm_after_drift": round(60 / (BEAT_S / ratio), 3),
        "grid_residual_ms": round(resid, 2), "grid_kicks": nk, "excerpt_start_bar": ex["start_bar"],
        "excerpt_start_s": round(first_beat_s, 3), "excerpt_labels": ex["labels"], "tier_gain_db": [round(20 * np.log10(g), 2) for g in gains],
        "lullaby": lwhere, "lullaby_grid_residual_ms": round(lres, 2),
        "kick_sync": kick_sync(st["drums"]),
    }
    report.append(info)
    # keep the stretched, levelled continuous tier mixes and the drums alone for the checks (not shipped)
    sf.write(work / "drums_ref.wav", st["drums"][:BEATS * BEAT + FADE].mean(axis=1), SR, subtype="PCM_24")
    for t, y in enumerate(final):
        sf.write(work / f"tier{t + 1}.wav", y[:BEATS * BEAT + FADE], SR, subtype="PCM_24")
    return info


if __name__ == "__main__":
    stems6, an_dir, work, out_sfx = sys.argv[1:5]
    jobs = [a.split("=") for a in sys.argv[5:]]
    report = []
    for letter, sid in jobs:
        info = build_song(letter, sid, stems6, an_dir, work, out_sfx, report)
        print(json.dumps(info))
    path = pathlib.Path(work) / "tracks.json"
    old = json.loads(path.read_text())["songs"] if path.exists() else []
    songs = sorted([o for o in old if o["letter"] not in {r["letter"] for r in report}] + report, key=lambda o: o["letter"])
    path.write_text(json.dumps({
        "beat_s": BEAT_S, "volleys_per_beat": 3, "beats": BEATS, "loop_from_beat": LOOP_FROM_BEAT,
        "tier_at_beat": list(TIER_AT_BEAT), "tier_lufs": list(TIER_LUFS), "slice_tail_ms": round(FADE / SR * 1000, 1),
        "sample_rate": SR, "songs": songs,
    }, indent=1))

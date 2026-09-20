#!/usr/bin/env python3
"""video_beats worker: decode audio -> tempo/beat grid/onsets/energy curve -> JSON.

Backends tried in order: librosa (pure python + numba/llvmlite wheels), aubio,
essentia. `beat_this` is intentionally skipped (torch is too heavy to pull
transitively for a tool). With no backend installed we still emit the onset/energy
channels so a beat grid can be hand-authored from them.
"""
import json
import subprocess
import sys


def decode_mono(path):
    p = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", path, "-vn", "-ac", "1", "-ar", "22050", "-f", "f32le", "-"],
        capture_output=True, check=True,
    )
    import numpy as np

    return np.frombuffer(p.stdout, dtype=np.float32), 22050


def energy_windows(y, sr, win=5.0, hop=1.0):
    import numpy as np

    n = int(win * sr)
    step = int(hop * sr)
    out = []
    for start in range(0, max(len(y) - n, 0) + 1, step):
        seg = y[start : start + n]
        out.append({"t": round(start / sr, 2), "rms": round(float(np.sqrt(np.mean(seg**2))), 5)})
    return out


def try_librosa(path):
    import librosa

    y, sr = librosa.load(path, sr=22050)
    tempo, beats = librosa.beat.beat_track(y=y, sr=sr, units="time")
    tempo = float(tempo.item() if hasattr(tempo, "item") else tempo)
    onsets = librosa.onset.onset_detect(y=y, sr=sr, units="time")
    rms = librosa.feature.rms(y=y)[0]
    times = librosa.times_like(rms, sr=sr)
    w = [{"t": round(float(t), 2), "rms": round(float(rms[(times >= t) & (times < t + 5)].mean()), 5)}
         for t in range(0, int(times[-1]) - 4, 5) if (times >= t).any() and (times < t + 5).any()]
    return tempo, [round(float(b), 3) for b in beats], [round(float(o), 3) for o in onsets], w


def try_aubio(path):
    import numpy as np
    import aubio

    src = aubio.source(path, 22050, 1024)
    tempo_o = aubio.tempo("default", 1024, 512, 22050)
    onset_o = aubio.onset("default", 1024, 512, 22050)
    beats, onsets, y = [], [], []
    while True:
        samples, read = src()
        y.append(samples)
        if tempo_o(samples):
            beats.append(round(float(tempo_o.get_last_s()), 3))
        if onset_o(samples):
            onsets.append(round(float(onset_o.get_last_s()), 3))
        if read < 1024:
            break
    y = np.concatenate(y)
    return float(tempo_o.get_bpm()), beats, onsets, energy_windows(y, 22050)


def try_essentia(path):
    import essentia.standard as es

    audio = es.MonoLoader(filename=path, sampleRate=22050)()
    tempo, beats, *_ = es.RhythmExtractor2013()(audio)
    onsets = es.Onsets()(es.Spectrum()(es.FrameGenerator(audio, frameSize=1024, hopSize=512)) for _ in [])
    return float(tempo), [round(float(b), 3) for b in beats], [round(float(o), 3) for o in onsets], energy_windows(audio, 22050)


def main():
    path = sys.argv[1]
    for name, fn in (("librosa", try_librosa), ("aubio", try_aubio), ("essentia", try_essentia)):
        try:
            tempo, beats, onsets, energy = fn(path)
        except ImportError:
            continue
        except Exception as e:  # backend present but choked — surface it
            print(json.dumps({"backend": name, "error": f"{type(e).__name__}: {e}"}))
            return
        print(json.dumps({
            "backend": name, "tempo_bpm": round(tempo, 2), "beats": beats,
            "onsets": onsets, "energy_5s": energy,
        }))
        return
    # No backend: still useful — energy curve + ffmpeg-level silence hints.
    y, sr = decode_mono(path)
    print(json.dumps({
        "backend": "energy-only",
        "hint": "pip install --user librosa for tempo/beat grid; onsets need a backend",
        "tempo_bpm": None, "beats": [], "onsets": [], "energy_5s": energy_windows(y, sr),
    }))


if __name__ == "__main__":
    main()

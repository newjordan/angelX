#!/usr/bin/env python3
"""Bard tracks, step 1: split each source song into stems (htdemucs_6s: vocals, drums, bass, other, guitar, piano).

Run where a GPU is, in a Python environment that has torch and audio-separator:

    python3 separate.py SRC_DIR OUT_DIR MODELS_DIR

SRC_DIR holds `<id>.wav`; OUT_DIR gets `<id>_(Stem)_htdemucs_6s.wav` (48 kHz); MODELS_DIR holds the model files
(`htdemucs_6s.yaml` and its checkpoint). The songs are instrumentals, so `analyze.py` and `build.py` fold the
"vocals" stem back into the lead synth: it is the lead the separator took for a voice.
"""
import json
import pathlib
import sys
import time

import torch
from audio_separator.separator import Separator

# WAV in, WAV out: this never calls ffmpeg (a box without it would otherwise refuse to start).
Separator.check_ffmpeg_installed = lambda self: None

src, out, models = (pathlib.Path(a) for a in sys.argv[1:4])
out.mkdir(parents=True, exist_ok=True)
sep = Separator(output_dir=str(out), model_file_dir=str(models), output_format="WAV", sample_rate=48000, use_soundfile=True)
sep.load_model(model_filename="htdemucs_6s.yaml")
prov = {}
for wav in sorted(src.glob("*.wav")):
    sid = wav.stem
    if (out / f"{sid}_(Drums)_htdemucs_6s.wav").exists():
        continue
    t0 = time.time()
    names = {s: f"{sid}_({s})_htdemucs_6s" for s in ("Vocals", "Drums", "Bass", "Other", "Guitar", "Piano")}
    files = sep.separate(str(wav), names)
    prov[sid] = {"files": files, "seconds": round(time.time() - t0, 1), "cuda": torch.cuda.is_available()}
    print(sid, prov[sid], flush=True)
(out / "provenance.json").write_text(json.dumps(prov, indent=1))

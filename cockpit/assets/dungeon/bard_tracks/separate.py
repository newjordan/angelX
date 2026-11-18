#!/usr/bin/env python3
import json
import pathlib
import sys
import time

import torch
from audio_separator.separator import Separator

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

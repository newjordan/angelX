"""Merge a Mage-hands LoRA adapter + trained merger into a copy of the
checkpoint: W' = W + (alpha / r) * B @ A on each targeted linear, the
merger tensors replaced. Pure tensor math on safetensors; no model code runs.
  python3 -I merge_lora.py <base-model-dir> <adapter-dir> <out-dir>
"""
import json, os, re, shutil, sys
import torch
from safetensors.torch import load_file, save_file

base, adapter, out = sys.argv[1:4]
cfg = json.load(open(f"{adapter}/adapter_config.json"))
scale = cfg["lora_alpha"] / cfg["r"]
lora = load_file(f"{adapter}/adapter_model.safetensors")
merger = load_file(f"{adapter}/merger.safetensors")

# "base_model.model.model.language_model.layers.N.<mod>.lora_A.weight" -> checkpoint "model.layers.N.<mod>.weight"
pairs = {}
for k, v in lora.items():
    m = re.match(r"base_model\.model\.model\.language_model\.(layers\.\d+\..+)\.lora_([AB])\.weight$", k)
    if not m:
        raise SystemExit(f"unexpected adapter key {k}")
    pairs.setdefault(f"model.{m.group(1)}.weight", {})[m.group(2)] = v
print(f"{len(pairs)} LoRA pairs, scale {scale}, {len(merger)} merger tensors")

os.makedirs(out, exist_ok=True)
for f in os.listdir(base):
    if not f.endswith(".safetensors") and os.path.isfile(f"{base}/{f}"):
        shutil.copy(f"{base}/{f}", f"{out}/{f}")
idx = json.load(open(f"{base}/model.safetensors.index.json"))
done, swapped = set(), set()
for shard in sorted(set(idx["weight_map"].values())):
    t = load_file(f"{base}/{shard}")
    for k in list(t):
        if k in pairs:
            A, B = pairs[k]["A"].float(), pairs[k]["B"].float()
            t[k] = (t[k].float() + scale * (B @ A)).to(t[k].dtype)
            done.add(k)
        if k in merger:
            t[k] = merger[k].to(t[k].dtype)
            swapped.add(k)
    save_file(t, f"{out}/{shard}", metadata={"format": "pt"})
    print("wrote", shard)
missing = set(pairs) - done
if missing or swapped != set(merger):
    raise SystemExit(f"not merged: {sorted(missing)[:5]} merger {sorted(set(merger) - swapped)}")
print(f"merged {len(done)} linears, swapped {len(swapped)} merger tensors -> {out}")

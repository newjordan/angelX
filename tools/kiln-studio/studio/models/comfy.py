"""ComfyUI on the DGX Spark: concept images, only when a job asks for them.

Started over `ssh $KILN_SPARK_SSH` (default `gpu-host`) in the tmux session `comfy` (the wish painter's
session, so the two share one ComfyUI), tunnelled to 127.0.0.1:18188, and
stopped by its own script once its log has been quiet for KILN_COMFY_IDLE
seconds (default 1200). The graph is z-image turbo, as the painter's `turbo`.
A concept is a reference to look at: it is never the asset, and its colours
are not the realm's (a realm-ink copy is written beside it).
"""
import json
import os
import random
import socket
import subprocess
import threading
import time
import urllib.request
import uuid
from pathlib import Path

from .base import Adapter, Answer

SPARK_SSH = os.environ.get("KILN_SPARK_SSH", "gpu-host")
PORT = 18188
URL = f"http://127.0.0.1:{PORT}"
IDLE = int(os.environ.get("KILN_COMFY_IDLE", "1200"))
STYLE = {
    "figure": ("Pixel art game character concept, chunky readable shapes, bold dark outline, limited palette, "
               "one light from the upper left, plain flat grey background, the figure alone and whole, "),
    "prop": ("Pixel art game prop concept, chunky readable shapes, bold dark outline, limited palette, "
             "one light from the upper left, plain flat grey background, the object alone and whole, "),
    "vfx": ("Pixel art game spell effect concept, hard colour bands, no blur, limited palette, "
            "on a plain dark dungeon floor, seen from a three-quarter top-down view, "),
}

RUN = r"""#!/bin/bash
cd ~/comfyui && . .venv/bin/activate
python main.py --listen 127.0.0.1 --port 8188 >> ~/comfyui.log 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null' EXIT HUP TERM
touch ~/comfyui.log
while kill -0 $pid 2>/dev/null; do
  sleep 30
  idle=$(( $(date +%s) - $(stat -c %Y ~/comfyui.log) ))
  if [ "$idle" -gt IDLE ]; then
    echo "[kiln] $(date) no job for ${idle}s: stopping ComfyUI" >> ~/comfyui.log
    kill $pid; wait $pid; exit 0
  fi
done
"""

_lock = threading.Lock()


def answers(timeout=3):
    try:
        urllib.request.urlopen(URL + "/system_stats", timeout=timeout)
        return True
    except Exception:
        return False


def start():
    script = RUN.replace("IDLE", str(IDLE))
    remote = ("mkdir -p ~/.cache/kiln && cat > ~/.cache/kiln/comfy-run.sh && chmod +x ~/.cache/kiln/comfy-run.sh && "
              "{ curl -s -m 2 -o /dev/null localhost:8188/system_stats || tmux has-session -t comfy 2>/dev/null || "
              "tmux new-session -d -s comfy ~/.cache/kiln/comfy-run.sh; }")
    subprocess.run(["ssh", "-o", "ConnectTimeout=10", "-o", "BatchMode=yes", SPARK_SSH, remote], input=script.encode(),
                   timeout=30, stdout=subprocess.DEVNULL, check=True)


def up():
    """ComfyUI reachable through the tunnel, started on the Spark if down; seconds it took."""
    with _lock:
        if answers():
            return 0.0
        t0 = time.time()
        start()
        with socket.socket() as s:
            busy = s.connect_ex(("127.0.0.1", PORT)) == 0
        if not busy:
            subprocess.run(["ssh", "-f", "-N", "-o", "ExitOnForwardFailure=yes", "-o", "BatchMode=yes",
                            "-L", f"{PORT}:localhost:8188", SPARK_SSH], timeout=20, check=True)
        for _ in range(240):
            if answers():
                return time.time() - t0
            time.sleep(0.5)
        raise RuntimeError("ComfyUI did not answer within 2 minutes of starting")


def stop():
    subprocess.run(["ssh", "-o", "ConnectTimeout=10", "-o", "BatchMode=yes", SPARK_SSH,
                    "tmux kill-session -t comfy 2>/dev/null; true"], timeout=30)


def turbo(prompt, seed, out, size=1024):
    graph = {
        "1": {"class_type": "UNETLoader", "inputs": {"unet_name": "z_image_turbo_bf16.safetensors", "weight_dtype": "default"}},
        "2": {"class_type": "CLIPLoader", "inputs": {"clip_name": "qwen_3_4b.safetensors", "type": "lumina2"}},
        "3": {"class_type": "VAELoader", "inputs": {"vae_name": "ae.safetensors"}},
        "4": {"class_type": "CLIPTextEncode", "inputs": {"text": prompt, "clip": ["2", 0]}},
        "5": {"class_type": "ConditioningZeroOut", "inputs": {"conditioning": ["4", 0]}},
        "6": {"class_type": "ModelSamplingAuraFlow", "inputs": {"model": ["1", 0], "shift": 3.0}},
        "7": {"class_type": "EmptySD3LatentImage", "inputs": {"width": size, "height": size, "batch_size": 1}},
        "8": {"class_type": "KSampler", "inputs": {"model": ["6", 0], "positive": ["4", 0], "negative": ["5", 0],
              "latent_image": ["7", 0], "seed": seed, "steps": 9, "cfg": 1.0, "sampler_name": "res_multistep",
              "scheduler": "simple", "denoise": 1.0}},
        "9": {"class_type": "VAEDecode", "inputs": {"samples": ["8", 0], "vae": ["3", 0]}},
        "10": {"class_type": "SaveImage", "inputs": {"images": ["9", 0], "filename_prefix": "kiln"}},
    }
    req = urllib.request.Request(URL + "/prompt", json.dumps({"prompt": graph, "client_id": str(uuid.uuid4())}).encode(),
                                 {"Content-Type": "application/json"})
    pid = json.load(urllib.request.urlopen(req, timeout=30))["prompt_id"]
    for _ in range(600):
        h = json.load(urllib.request.urlopen(f"{URL}/history/{pid}", timeout=10))
        if pid in h and h[pid].get("outputs"):
            img = h[pid]["outputs"]["10"]["images"][0]
            data = urllib.request.urlopen(f"{URL}/view?filename={img['filename']}&type=output&subfolder={img['subfolder']}",
                                          timeout=30).read()
            Path(out).write_bytes(data)
            return out
        if pid in h and h[pid].get("status", {}).get("status_str") == "error":
            raise RuntimeError("comfy: " + json.dumps(h[pid]["status"])[:400])
        time.sleep(0.5)
    raise RuntimeError("comfy: no picture in 5 minutes")


def realm_copy(src, out, side=128):
    """The concept brought to `side` px and the realm's nearest inks: a guide to shapes and values."""
    from PIL import Image
    from ..realm import realm
    pal = realm()
    im = Image.open(src).convert("RGB").resize((side, side), Image.BOX)
    px = im.load()
    cache = {}
    for y in range(side):
        for x in range(side):
            c = px[x, y]
            if c not in cache:
                cache[c] = pal.rgb[pal.nearest(c)]
            px[x, y] = cache[c]
    im.save(out)
    return out


class Comfy(Adapter):
    name = "comfy"

    def available(self):
        if answers():
            return True, "ComfyUI answering at " + URL
        r = subprocess.run(["ssh", "-o", "ConnectTimeout=8", "-o", "BatchMode=yes", SPARK_SSH, "true"],
                           capture_output=True, text=True, timeout=20)
        return (True, f"{SPARK_SSH} reachable; ComfyUI starts on demand") if r.returncode == 0 else \
            (False, f"ssh {SPARK_SSH}: {r.stderr.strip()[:200]}")

    def ask(self, prompt, images=(), workdir=None, budget=600, n=1, seed=None, kind="figure"):
        t0 = time.time()
        try:
            res = self.concepts(prompt, Path(workdir or "."), n=n, seed=seed, kind=kind)
        except Exception as e:
            return Answer(model="comfy", ok=False, error=str(e)[:500], seconds=time.time() - t0)
        return Answer(model="comfy", text=json.dumps(res), files=res["files"], seconds=time.time() - t0, turns=n)

    def concepts(self, prompt, out_dir, n=1, seed=None, kind="figure"):
        out_dir = Path(out_dir)
        out_dir.mkdir(parents=True, exist_ok=True)
        warm = up()
        seed = seed if seed is not None else random.randrange(1 << 31)
        files, realm_files = [], []
        for i in range(n):
            stem = f"concept-{seed + i}"
            png = turbo(STYLE.get(kind, STYLE["figure"]) + prompt, seed + i, out_dir / f"{stem}.png")
            files.append(str(png))
            realm_files.append(realm_copy(png, out_dir / f"{stem}-realm.png"))
        return {"files": files, "realm": [str(p) for p in realm_files], "seed": seed, "start_secs": round(warm, 1),
                "note": f"ComfyUI stops itself after {IDLE}s without a job"}

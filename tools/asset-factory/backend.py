"""Local Qwen-Image through the game's ComfyUI image service, with receipts."""
import hashlib
import json
import os
from pathlib import Path
import time
import urllib.parse
import urllib.request
import fcntl


def request(url, path, body=None):
    req = urllib.request.Request(url + path, None if body is None else json.dumps(body).encode(),
                                 {'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=120) as response:
        return json.load(response)


def workflow(prompt, seed):
    return {
        '1': {'class_type': 'UNETLoader', 'inputs': {'unet_name': 'qwen_image_2.1_bf16.safetensors', 'weight_dtype': 'default'}},
        '2': {'class_type': 'CLIPLoader', 'inputs': {'clip_name': 'qwen3vl_8b_bf16.safetensors', 'type': 'qwen_image'}},
        '3': {'class_type': 'VAELoader', 'inputs': {'vae_name': 'qwen_image_2.1_vae_bf16.safetensors'}},
        '4': {'class_type': 'CLIPTextEncode', 'inputs': {'text': prompt, 'clip': ['2', 0]}},
        '5': {'class_type': 'CLIPTextEncode', 'inputs': {'text': 'photograph, 3d render, gradients, glow, text, white highlights, multiple figures, ground shadow', 'clip': ['2', 0]}},
        '6': {'class_type': 'EmptySD3LatentImage', 'inputs': {'width': 512, 'height': 512, 'batch_size': 1}},
        '7': {'class_type': 'ModelSamplingAuraFlow', 'inputs': {'model': ['1', 0], 'shift': 3.1}},
        '8': {'class_type': 'KSampler', 'inputs': {'model': ['7', 0], 'positive': ['4', 0], 'negative': ['5', 0],
              'latent_image': ['6', 0], 'seed': seed, 'steps': 20, 'cfg': 4, 'sampler_name': 'euler', 'scheduler': 'simple', 'denoise': 1}},
        '9': {'class_type': 'VAEDecode', 'inputs': {'samples': ['8', 0], 'vae': ['3', 0]}},
        '10': {'class_type': 'SaveImage', 'inputs': {'images': ['9', 0], 'filename_prefix': 'delve-factory'}}}


def generate(brief, kind, output, log, seed=39):
    url = os.environ.get('FACTORY_COMFY_URL', 'http://127.0.0.1:18189').rstrip('/')
    prompt = (f'One isolated {kind} game sprite: {brief}. Side view facing right. '
              'Delve medieval dungeon pixel art. Readable simple silhouette, deliberately coarse square pixel clusters, '
              '16 pixel tall design enlarged without smoothing. Tonal muted grey stone and steel, one small dull ochre accent. '
              'No pure white, no glow, no hypercontrast. Flat solid magenta background, no shadow, no floor, no text. ')
    if kind == 'worn':
        prompt += 'Only empty torso armour breastplate with shoulder plates, no person, no head, no legs, no arms. Wide and short, 2:1 aspect.'
    if kind == 'weapon':
        prompt += 'Weapon alone upright, blade or shaft pointing up, handle at bottom, no hands or character.'
    graph = workflow(prompt, seed)
    log = Path(log); log.parent.mkdir(parents=True, exist_ok=True)
    # Reserve before network I/O: even failed paid calls consume the lane budget.
    with log.open('a+') as stream:
        fcntl.flock(stream, fcntl.LOCK_EX); stream.seek(0)
        count = sum(json.loads(line).get('event') == 'requested' for line in stream if line.strip())
        if count >= 30:
            raise ValueError('generation budget exhausted (30 calls); use a source image or targeted local edit')
        call = hashlib.sha256((prompt + str(seed) + str(time.time_ns())).encode()).hexdigest()[:16]
        record = dict(event='requested', call=call, backend='local Qwen-Image 2.1', paid=False,
                      seed=seed, prompt=prompt, workflow=graph, time=time.time())
        stream.write(json.dumps(record) + '\n'); stream.flush()
    start = time.monotonic()
    try:
        submitted = request(url, '/prompt', {'prompt': graph, 'client_id': 'delve-asset-factory'})
        pid = submitted['prompt_id']
        deadline = time.monotonic() + 1200
        while time.monotonic() < deadline:
            history = request(url, '/history/' + pid).get(pid)
            if history:
                if history.get('status', {}).get('status_str') == 'error':
                    raise RuntimeError(json.dumps(history['status']))
                images = history.get('outputs', {}).get('10', {}).get('images', [])
                if images:
                    query = urllib.parse.urlencode(images[0])
                    with urllib.request.urlopen(url + '/view?' + query, timeout=120) as response:
                        data = response.read(8 * 1024 * 1024 + 1)
                    if len(data) > 8 * 1024 * 1024:
                        raise ValueError('backend PNG exceeds 8 MiB')
                    Path(output).write_bytes(data)
                    result = dict(event='finished', call=call, prompt_id=pid, seconds=time.monotonic()-start,
                                  sha256=hashlib.sha256(data).hexdigest(), file=str(output))
                    break
            time.sleep(2)
        else:
            raise TimeoutError('Qwen image generation exceeded 20 minutes')
    except Exception as error:
        with log.open('a') as stream:
            stream.write(json.dumps(dict(event='failed', call=call, seconds=time.monotonic()-start, error=str(error)))+'\n')
        raise
    with log.open('a') as stream:
        stream.write(json.dumps(result)+'\n')
    return result

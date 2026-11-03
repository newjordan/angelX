"""OpenAI-compatible /v1 shim over transformers for microsoft/Mage-VL-Cowork.

The card's runtimes are SGLang (docker) and plain transformers; vLLM support
for llava_onevision2 is unverified. This serves /v1/models and
/v1/chat/completions (stream and non-stream) on loopback so angel's
ANGEL_VISION_URL sidecar and the grounding probe can talk to it.

  venv/bin/python bin/serve_mage.py --model ~/models/Mage-VL-Cowork --port 30001
"""

import argparse
import base64
import io
import json
import re
import sys
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import torch
from PIL import Image
from transformers import AutoModelForCausalLM, AutoProcessor

ap = argparse.ArgumentParser()
ap.add_argument("--model", required=True)
ap.add_argument("--name", default="microsoft/Mage-VL-Cowork")
ap.add_argument("--host", default="127.0.0.1")
ap.add_argument("--port", type=int, default=30001)
ap.add_argument("--max-new-tokens", type=int, default=2048)
args = ap.parse_args()

processor = AutoProcessor.from_pretrained(args.model, trust_remote_code=True)
model = AutoModelForCausalLM.from_pretrained(
    args.model, trust_remote_code=True, torch_dtype=torch.bfloat16, device_map="cuda"
).eval()
gpu = threading.Lock()

# The checkpoint ships an lm_head distinct from embed_tokens, and the reference
# SGLang runtime refuses tied embeddings. transformers may tie them on load
# (tie_word_embeddings defaults on); always serve the checkpoint's own head.
emb = model.get_input_embeddings().weight
tied = model.lm_head.weight.data_ptr() == emb.data_ptr()
from safetensors import safe_open  # noqa: E402

_idx = json.load(open(f"{args.model}/model.safetensors.index.json"))["weight_map"]
with safe_open(f"{args.model}/{_idx['lm_head.weight']}", "pt") as _f:
    _head = _f.get_tensor("lm_head.weight").to(model.device, model.dtype)
same = torch.equal(model.lm_head.weight.data, _head)
model.lm_head.weight = torch.nn.Parameter(_head, requires_grad=False)
print(f"lm_head was tied to embed_tokens: {tied}; loaded head matched checkpoint: {same}", flush=True)


def load_image(url):
    if url.startswith("data:"):
        raw = base64.b64decode(url.split(",", 1)[1])
    elif url.startswith("file://"):
        raw = open(url[7:], "rb").read()
    else:
        raise ValueError("only data: and file:// image urls are served")
    return Image.open(io.BytesIO(raw)).convert("RGB")


def convert(messages):
    """OpenAI messages -> chat-template messages + images in order."""
    out, images = [], []
    for m in messages:
        role, content = m.get("role", "user"), m.get("content")
        if role == "developer":
            role = "system"
        if isinstance(content, list):
            parts = []
            for p in content:
                kind = p.get("type")
                if kind in ("image_url", "input_image"):
                    iu = p.get("image_url")
                    url = iu.get("url") if isinstance(iu, dict) else iu
                    images.append(load_image(url))
                    parts.append({"type": "image"})
                elif kind in ("text", "input_text"):
                    parts.append({"type": "text", "text": p.get("text", "")})
            content = parts
        elif content is None:
            content = ""
        if role == "assistant" and m.get("tool_calls"):
            calls = "".join(
                "\n<tool_call>\n"
                + json.dumps({"name": c["function"]["name"], "arguments": c["function"].get("arguments")})
                + "\n</tool_call>"
                for c in m["tool_calls"]
            )
            content = (content if isinstance(content, str) else "") + calls
        out.append({"role": role, "content": content})
    return out, images


def generate(body):
    messages, images = convert(body.get("messages", []))
    kwargs = body.get("chat_template_kwargs") or {}
    prompt = processor.apply_chat_template(
        messages, tokenize=False, add_generation_prompt=True, **kwargs
    )
    inputs = processor(
        text=[prompt], images=images or None, return_tensors="pt"
    ).to(model.device)
    if "pixel_values" in inputs:
        inputs["pixel_values"] = inputs["pixel_values"].to(model.dtype)
    if "image_grid_thw" in inputs:
        # The vision RoPE reads explicit row-major [t, h, w] ids per patch
        # (the card's transformers example omits them and crashes on None);
        # same ids as its grid path, each image starting at t=0.
        pos = []
        for t, h, w in inputs["image_grid_thw"].tolist():
            pos.append(torch.stack([
                torch.arange(t).repeat_interleave(h * w),
                torch.arange(h).repeat_interleave(w).repeat(t),
                torch.arange(w).repeat(h).repeat(t),
            ], dim=-1))
        pp = torch.cat(pos)
        if body.get("mage_pp_layout") == "block":
            # The official LLaVA-OneVision-2 processor's layout: positions
            # pre-reordered into 2x2 merge blocks (build_patch_positions).
            out, off = [], 0
            for t, h, w in inputs["image_grid_thw"].tolist():
                idx = torch.arange(t * h * w).view(t, h // 2, 2, w // 2, 2).permute(0, 1, 3, 2, 4).reshape(-1)
                out.append(pp[off:off + t * h * w][idx])
                off += t * h * w
            pp = torch.cat(out)
        inputs["patch_positions"] = pp.to(model.device)
    temp = float(body.get("temperature") or 0.0)
    gen = {
        "max_new_tokens": int(body.get("max_tokens") or body.get("max_completion_tokens") or args.max_new_tokens),
        "do_sample": temp > 0,
    }
    if temp > 0:
        gen.update(temperature=temp, top_p=float(body.get("top_p") or 0.8))
    with gpu, torch.inference_mode():
        t0 = time.time()
        ids = model.generate(**inputs, **gen)
        dt = time.time() - t0
    n_in = inputs["input_ids"].shape[1]
    text = processor.tokenizer.decode(ids[0, n_in:], skip_special_tokens=True)
    reasoning = ""
    if "</think>" in text:
        reasoning, text = text.split("</think>", 1)
        reasoning = reasoning.replace("<think>", "").strip()
    usage = {
        "prompt_tokens": n_in,
        "completion_tokens": int(ids.shape[1] - n_in),
        "total_tokens": int(ids.shape[1]),
    }
    sys.stderr.write(f"gen {n_in}+{usage['completion_tokens']} tok in {dt:.1f}s, {len(images)} image(s)\n")
    return text.strip(), reasoning, usage


class H(BaseHTTPRequestHandler):
    def _json(self, code, obj):
        raw = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        if self.path.rstrip("/").endswith("/models"):
            return self._json(200, {"object": "list", "data": [
                {"id": args.name, "object": "model", "owned_by": "local", "max_model_len": 32768}
            ]})
        self._json(404, {"error": {"message": "not found"}})

    def do_POST(self):
        if not self.path.rstrip("/").endswith("/chat/completions"):
            return self._json(404, {"error": {"message": "not found"}})
        try:
            body = json.loads(self.rfile.read(int(self.headers.get("content-length", 0))))
            text, reasoning, usage = generate(body)
        except Exception as e:  # report, never crash the server
            return self._json(400, {"error": {"message": f"{type(e).__name__}: {e}"}})
        cid, now = "chatcmpl-" + uuid.uuid4().hex[:12], int(time.time())
        msg = {"role": "assistant", "content": text}
        if reasoning:
            msg["reasoning_content"] = reasoning
        if not body.get("stream"):
            return self._json(200, {
                "id": cid, "object": "chat.completion", "created": now, "model": args.name,
                "choices": [{"index": 0, "message": msg, "finish_reason": "stop"}],
                "usage": usage,
            })
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("cache-control", "no-cache")
        self.end_headers()
        frames = [
            {"choices": [{"index": 0, "delta": msg, "finish_reason": None}]},
            {"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]},
        ]
        if (body.get("stream_options") or {}).get("include_usage"):
            frames.append({"choices": [], "usage": usage})
        for f in frames:
            f.update(id=cid, object="chat.completion.chunk", created=now, model=args.name)
            self.wfile.write(f"data: {json.dumps(f)}\n\n".encode())
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()

    def log_message(self, fmt, *a):
        sys.stderr.write("%s %s\n" % (self.address_string(), fmt % a))


print(f"serving {args.name} on http://{args.host}:{args.port}/v1", flush=True)
ThreadingHTTPServer((args.host, args.port), H).serve_forever()

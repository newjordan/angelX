"""Offline Mage grounding bench against the reference SGLang container.

Per target, five model calls:
  first   - one look, `ground`'s exact request (/generate, Mage's template, JPEG q95)
  z2      - first, then re-look on a 2x crop scaled back to full frame
  casc    - z2, then a 3x crop (what `ground` ships)
  repeat  - the first look again (run-to-run noise at temperature 0)
  chat    - one look through SGLang's OpenAI /v1/chat/completions (H6)
  python3 -I bench_run.py <bench-dir> [limit]   (venv python with PIL + transformers)
"""
import base64, io, json, os, re, sys, time, urllib.request
from concurrent.futures import ThreadPoolExecutor
from PIL import Image

root = sys.argv[1]
FAST = os.environ.get("BENCH_FAST") == "1"  # first / z2 / cascade only
limit = int(sys.argv[2]) if len(sys.argv) > 2 else None
BASE = "http://127.0.0.1:30001"
# The prompt `ground` sends; HANDS_GROUND_PROMPT overrides the checkout's copy.
SYSTEM = open(os.environ.get("HANDS_GROUND_PROMPT") or os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "../../../../cockpit/src/agent/tools/hands_ground_prompt.txt")).read()
DIRECT = " The target is visible on the current screen; click it directly in one action."
PT = re.compile(r'"coordinate"\s*:\s*\[\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)\s*\]')


def user_text(instruction):
    return ("\nPlease generate the next move according to the UI screenshot, instruction and previous "
            f"actions.\n\nInstruction: {instruction.rstrip('.')}.{DIRECT}\n\nPrevious actions:\nNone")


def prompt(instruction):  # byte-identical to hands.rs ground_prompt
    return (f"<|im_start|>system\n{SYSTEM}<|im_end|>\n<|im_start|>user\n"
            f"<|vision_start|><|image_pad|><|vision_end|>{user_text(instruction)}<|im_end|>\n"
            "<|im_start|>assistant\n<think>\n\n</think>\n\n")


def jpeg(im):
    b = io.BytesIO()
    im.save(b, format="JPEG", quality=95)
    return base64.b64encode(b.getvalue()).decode()


def post(path, body):
    req = urllib.request.Request(BASE + path, json.dumps(body).encode(), {"content-type": "application/json"})
    t = time.time()
    out = json.load(urllib.request.urlopen(req, timeout=600))
    return out, time.time() - t


def parse(text):
    m = PT.search(text or "")
    if not m:
        return None
    x, y = float(m.group(1)), float(m.group(2))
    return (x, y) if 0 <= x <= 999 and 0 <= y <= 999 else None


def look(im, instruction):
    out, dt = post("/generate", {"text": prompt(instruction), "image_data": [jpeg(im)],
                                 "sampling_params": {"temperature": 0.0, "max_new_tokens": 1024}})
    p = parse(out.get("text"))
    W, H = im.size
    return (None if p is None else (p[0] / 999 * (W - 1), p[1] / 999 * (H - 1))), dt, out.get("text", "")[:300]


def look_chat(im, instruction):
    body = {"model": "Mage-VL-Cowork", "temperature": 0, "max_tokens": 1024,
            "chat_template_kwargs": {"enable_thinking": False},
            "messages": [{"role": "system", "content": SYSTEM},
                         {"role": "user", "content": [
                             {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64," + jpeg(im)}},
                             {"type": "text", "text": user_text(instruction)}]}]}
    out, dt = post("/v1/chat/completions", body)
    text = out["choices"][0]["message"].get("content") or ""
    p = parse(text)
    W, H = im.size
    return (None if p is None else (p[0] / 999 * (W - 1), p[1] / 999 * (H - 1))), dt, text[:300]


def zoom(im, at, z, instruction):
    W, H = im.size
    cw, ch = W // z, H // z
    x0 = int(min(max(round(at[0] - cw / 2), 0), W - cw))
    y0 = int(min(max(round(at[1] - ch / 2), 0), H - ch))
    up = im.crop((x0, y0, x0 + cw, y0 + ch)).resize((W, H), Image.LANCZOS)
    p, dt, raw = look(up, instruction)
    return (at if p is None else (x0 + p[0] / W * cw, y0 + p[1] / H * ch)), dt, raw


def score(p, b):
    if p is None:
        return {"px": None, "hit": False, "dist": None}
    cx, cy = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
    return {"px": [round(p[0], 1), round(p[1], 1)], "hit": b[0] <= p[0] <= b[2] and b[1] <= p[1] <= b[3],
            "dist": round(((p[0] - cx) ** 2 + (p[1] - cy) ** 2) ** 0.5, 1)}


def run(it):
    im = Image.open(f"{root}/{it['shot']}").convert("RGB")
    ins, b = it["instruction"], it["box"]
    rec = {k: it[k] for k in ("id", "src", "page", "split", "theme", "size_bin", "kind", "role", "instruction", "box")}
    p1, t1, raw1 = look(im, ins)
    rec["first"] = {**score(p1, b), "secs": round(t1, 2), "raw": raw1}
    p2, t2 = p1, 0.0
    if p1 is not None:
        p2, t2, _ = zoom(im, p1, 2, ins)
    rec["z2"] = {**score(p2, b), "secs": round(t1 + t2, 2)}
    p3, t3 = p2, 0.0
    if p2 is not None:
        p3, t3, _ = zoom(im, p2, 3, ins)
    rec["casc"] = {**score(p3, b), "secs": round(t1 + t2 + t3, 2)}
    if not FAST:
        pr, tr, _ = look(im, ins)
        rec["repeat"] = {**score(pr, b), "secs": round(tr, 2)}
        pc, tc, rawc = look_chat(im, ins)
        rec["chat"] = {**score(pc, b), "secs": round(tc, 2), "raw": rawc}
    return rec


items = json.load(open(f"{root}/items.json"))[:limit]
done = set()
out_path = f"{root}/" + os.environ.get("BENCH_OUT", "results.jsonl")
try:
    done = {json.loads(l)["id"] for l in open(out_path)}
except FileNotFoundError:
    pass
todo = [it for it in items if it["id"] not in done]
print(f"{len(items)} items, {len(done)} done, {len(todo)} to run", flush=True)
with open(out_path, "a") as f, ThreadPoolExecutor(2) as ex:
    for n, rec in enumerate(ex.map(run, todo), 1):
        f.write(json.dumps(rec) + "\n")
        f.flush()
        if n % 25 == 0:
            print(f"{n}/{len(todo)}", flush=True)
print("done", flush=True)

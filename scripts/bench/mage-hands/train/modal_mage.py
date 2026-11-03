"""Fine-tune microsoft/Mage-VL-Cowork as angel's hands (desktop `ground`).

Budget: $15 operator-approved (2026-10-30). B200 at ~$6.25/h. Hard caps:
smoke() times out at 15 min, train() at 100 min, and training itself stops
at its own wall-clock deadline and saves. The model download runs on CPU.

LoRA r=64 on every language-model linear (q/k/v/o/gate/up/down, 252
matrices) plus the vision->language merger in full; vision encoder frozen.
Loss on the answer tokens only. Samples are ground's exact request (see
make_train.py), dev split only.

  modal run modal_mage.py::download
  modal run modal_mage.py::smoke --run smoke1
  modal run --detach modal_mage.py::train --run main1 --minutes 80
"""
import modal

app = modal.App("mage-hands-train")
image = (
    modal.Image.debian_slim(python_version="3.11")
    .pip_install("torch==2.8.0", "torchvision==0.23.0", "transformers==4.57.3", "peft==0.17.1",
                 "accelerate", "pillow", "safetensors", "huggingface_hub[hf_xet]")
)
data_vol = modal.Volume.from_name("mage-train", create_if_missing=True)
hf_vol = modal.Volume.from_name("hf-cache")
VOLS = {"/data": data_vol, "/root/.cache/huggingface": hf_vol}
MODEL = "microsoft/Mage-VL-Cowork"
B200_PER_HOUR = 6.25
TARGETS = r".*language_model\.layers\.\d+\.(self_attn\.(q|k|v|o)_proj|mlp\.(gate|up|down)_proj)"


@app.function(image=image, volumes=VOLS, timeout=1800, cpu=4)
def download():
    from huggingface_hub import snapshot_download
    path = snapshot_download(MODEL)
    hf_vol.commit()
    return path


def run_training(run, minutes, lr, rank, accum, workers, sanity):
    import io, json, math, os, random, time
    import torch
    import torch.nn.functional as F
    from PIL import Image
    from huggingface_hub import snapshot_download
    from peft import LoraConfig, get_peft_model
    from safetensors.torch import save_file
    from transformers import (AutoModelForCausalLM, AutoTokenizer, Qwen2VLImageProcessorFast,
                              Qwen2VLVideoProcessor, Qwen2_5_VLProcessor)

    t0 = time.time()
    deadline = t0 + minutes * 60
    out = f"/data/runs/{run}"
    os.makedirs(out, exist_ok=True)
    M = snapshot_download(MODEL, local_files_only=True)
    tok = AutoTokenizer.from_pretrained(M)
    proc = Qwen2_5_VLProcessor(image_processor=Qwen2VLImageProcessorFast.from_pretrained(M), tokenizer=tok,
                               video_processor=Qwen2VLVideoProcessor.from_pretrained(M),
                               chat_template=open(f"{M}/chat_template.jinja").read())
    SYSTEM = open("/data/system.txt").read()
    samples = [json.loads(l) for l in open("/data/train.jsonl")]
    random.Random(run).shuffle(samples)

    def prompt(target):
        user = ("\nPlease generate the next move according to the UI screenshot, instruction and previous actions.\n\n"
                f"Instruction: Click {target}. The target is visible on the current screen; click it directly in one action."
                "\n\nPrevious actions:\nNone")
        return (f"<|im_start|>system\n{SYSTEM}<|im_end|>\n<|im_start|>user\n<|vision_start|><|image_pad|><|vision_end|>"
                f"{user}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n")

    class DS(torch.utils.data.Dataset):
        def __len__(self):
            return len(samples)

        def __getitem__(self, i):
            s = samples[i]
            img = Image.open(f"/data/screens/{s['screen']}.png").convert("RGB")
            if s["crop"]:
                x0, y0, cw, ch = s["crop"]
                img = img.crop((x0, y0, x0 + cw, y0 + ch)).resize((1920, 1080), Image.LANCZOS)
            b = io.BytesIO()
            img.save(b, format="JPEG", quality=95)  # ground sends JPEG q95
            img = Image.open(io.BytesIO(b.getvalue())).convert("RGB")
            enc = proc(text=[prompt(s["target"])], images=[img], return_tensors="pt")
            ans = tok(s["answer"] + "<|im_end|>\n", add_special_tokens=False, return_tensors="pt")["input_ids"]
            pos = []
            for t, h, w in enc["image_grid_thw"].tolist():
                pos.append(torch.stack([torch.arange(t).repeat_interleave(h * w), torch.arange(h).repeat_interleave(w).repeat(t),
                                        torch.arange(w).repeat(h).repeat(t)], dim=-1))
            return {"input_ids": torch.cat([enc["input_ids"], ans], dim=1), "n_ans": ans.shape[1],
                    "pixel_values": enc["pixel_values"], "image_grid_thw": enc["image_grid_thw"],
                    "patch_positions": torch.cat(pos)}

    loader = torch.utils.data.DataLoader(DS(), batch_size=None, shuffle=False, num_workers=workers,
                                         prefetch_factor=4, persistent_workers=True)

    model = AutoModelForCausalLM.from_pretrained(M, trust_remote_code=True, dtype=torch.bfloat16,
                                                 attn_implementation="sdpa").cuda()
    model.config.use_cache = False
    model.requires_grad_(False)
    model = get_peft_model(model, LoraConfig(r=rank, lora_alpha=2 * rank, lora_dropout=0.05, target_modules=TARGETS))
    inner = model.base_model.model
    merger = {n: p for n, p in model.named_parameters() if ".visual.merger." in n}
    for p in merger.values():
        p.data = p.data.float()
        p.requires_grad_(True)
    lora = [p for n, p in model.named_parameters() if "lora_" in n]
    for p in lora:
        p.data = p.data.float()
    inner.model.language_model.gradient_checkpointing_enable(gradient_checkpointing_kwargs={"use_reentrant": False})
    opt = torch.optim.AdamW([{"params": lora, "lr": lr}, {"params": list(merger.values()), "lr": lr * 0.2}], weight_decay=0.0)
    base_lrs = [g["lr"] for g in opt.param_groups]
    n_lora = sum(p.numel() for p in lora)
    print(f"loaded in {time.time() - t0:.0f}s; lora params {n_lora / 1e6:.1f}M, merger {sum(p.numel() for p in merger.values()) / 1e6:.1f}M; "
          f"{len(samples)} samples; train until {minutes} min", flush=True)

    def save(tag):
        model.save_pretrained(f"{out}/{tag}")
        # checkpoint names: "visual.merger.*" (the PEFT name is "...model.visual.merger.*")
        save_file({"visual." + n.split(".visual.", 1)[1]: p.detach().to(torch.bfloat16).cpu().contiguous()
                   for n, p in merger.items()}, f"{out}/{tag}/merger.safetensors")
        data_vol.commit()
        print(f"saved {tag}", flush=True)

    model.train()
    t_train = time.time()
    step = seen = 0
    run_loss = []
    last_save = time.time()
    for b in loader:
        frac = (time.time() - t_train) / max(1.0, deadline - t_train)
        if frac >= 1.0:
            break
        warm = min(1.0, (step + 1) / 15)
        scale = warm * (0.1 + 0.9 * 0.5 * (1 + math.cos(math.pi * min(1.0, frac))))
        for g, base in zip(opt.param_groups, base_lrs):
            g["lr"] = base * scale
        ids = b["input_ids"].cuda()
        with torch.autocast("cuda", dtype=torch.bfloat16):
            h = inner.model(input_ids=ids, pixel_values=b["pixel_values"].cuda().to(torch.bfloat16),
                            image_grid_thw=b["image_grid_thw"].cuda(), patch_positions=b["patch_positions"].cuda(),
                            use_cache=False, return_dict=True).last_hidden_state
            L, n = ids.shape[1], b["n_ans"]
            logits = inner.lm_head(h[:, L - n - 1:L - 1]).float()
        loss = F.cross_entropy(logits.reshape(-1, logits.size(-1)), ids[:, L - n:].reshape(-1))
        (loss / accum).backward()
        run_loss.append(loss.item())
        seen += 1
        if seen % accum == 0:
            torch.nn.utils.clip_grad_norm_(lora + list(merger.values()), 1.0)
            opt.step()
            opt.zero_grad(set_to_none=True)
            step += 1
            if step % 5 == 0:
                el = time.time() - t_train
                print(f"step {step} samples {seen} loss {sum(run_loss[-accum * 5:]) / len(run_loss[-accum * 5:]):.4f} "
                      f"{seen / el:.2f} samples/s lr {opt.param_groups[0]['lr']:.2e} "
                      f"mem {torch.cuda.max_memory_allocated() / 2**30:.1f} GiB elapsed {el / 60:.1f} min "
                      f"(~${(time.time() - t0) / 3600 * B200_PER_HOUR:.2f} so far)", flush=True)
            if time.time() - last_save > 15 * 60:
                save(f"ckpt-{step}")
                last_save = time.time()
    save("final")

    if sanity:
        model.eval()
        hits = 0
        for i in range(sanity):
            s = samples[-1 - i]
            b = DS()[len(samples) - 1 - i]
            ids = b["input_ids"][:, :-b["n_ans"]].cuda()
            with torch.no_grad(), torch.autocast("cuda", dtype=torch.bfloat16):
                g = model.generate(input_ids=ids, pixel_values=b["pixel_values"].cuda().to(torch.bfloat16),
                                   image_grid_thw=b["image_grid_thw"].cuda(), patch_positions=b["patch_positions"].cuda(),
                                   max_new_tokens=80, do_sample=False)
            text = tok.decode(g[0, ids.shape[1]:], skip_special_tokens=True)
            want = json.loads(s["answer"].split("<tool_call>\n", 1)[1].split("\n</tool_call>")[0])["arguments"]["coordinate"]
            got = None
            try:
                got = json.loads(text.split("<tool_call>\n", 1)[1].split("\n</tool_call>")[0])["arguments"]["coordinate"]
            except Exception:
                pass
            ok = got is not None and abs(got[0] - want[0]) <= 15 and abs(got[1] - want[1]) <= 15
            hits += ok
            print(f"sanity {i}: want {want} got {got} {'ok' if ok else 'off'} | {text[:90]!r}", flush=True)
        print(f"sanity (train samples, not an eval): {hits}/{sanity} within 15/999", flush=True)
    total = time.time() - t0
    print(f"done: {step} steps, {seen} samples, {total / 60:.1f} min, ~${total / 3600 * B200_PER_HOUR:.2f}", flush=True)
    return {"steps": step, "samples": seen, "minutes": round(total / 60, 1)}


@app.function(image=image, gpu="B200", cpu=16, memory=65536, volumes=VOLS, timeout=15 * 60)
def smoke(run: str = "smoke1"):
    return run_training(run, minutes=4, lr=1e-4, rank=64, accum=8, workers=12, sanity=3)


@app.function(image=image, gpu="B200", cpu=16, memory=65536, volumes=VOLS, timeout=100 * 60)
def train(run: str = "main1", minutes: int = 80):
    return run_training(run, minutes=min(minutes, 88), lr=1e-4, rank=64, accum=8, workers=12, sanity=0)


@app.function(image=image, gpu="B200", cpu=8, memory=65536, volumes=VOLS, timeout=45 * 60)
def evaluate(run: str = "main1", tag: str = "final"):
    """Held-out test split, base vs fine-tuned, through ground's exact pipeline
    (template prompt, JPEG q95, first look then 2x and 3x crops scaled back up).
    Same code and process for both; base = adapter disabled + original merger."""
    import contextlib, io, json, math, re, time
    import torch
    from PIL import Image
    from huggingface_hub import snapshot_download
    from peft import PeftModel
    from safetensors.torch import load_file
    from transformers import (AutoModelForCausalLM, AutoTokenizer, Qwen2VLImageProcessorFast,
                              Qwen2VLVideoProcessor, Qwen2_5_VLProcessor, StoppingCriteria)

    t0 = time.time()
    M = snapshot_download(MODEL, local_files_only=True)
    tok = AutoTokenizer.from_pretrained(M)
    proc = Qwen2_5_VLProcessor(image_processor=Qwen2VLImageProcessorFast.from_pretrained(M), tokenizer=tok,
                               video_processor=Qwen2VLVideoProcessor.from_pretrained(M),
                               chat_template=open(f"{M}/chat_template.jinja").read())
    SYSTEM = open("/data/system.txt").read()
    base = AutoModelForCausalLM.from_pretrained(M, trust_remote_code=True, dtype=torch.bfloat16,
                                                attn_implementation="sdpa").cuda().eval()
    orig = {k: v.clone() for k, v in base.model.visual.merger.state_dict().items()}
    model = PeftModel.from_pretrained(base, f"/data/runs/{run}/{tag}").eval()
    merger = model.base_model.model.model.visual.merger
    tuned = {k.split("visual.merger.", 1)[1]: v for k, v in load_file(f"/data/runs/{run}/{tag}/merger.safetensors").items()}
    items = json.load(open("/data/eval/items-test.json"))
    PT = re.compile(r'"coordinate"\s*:\s*\[\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)\s*\]')

    class StopAtPoint(StoppingCriteria):
        def __init__(self, n):
            self.n = n

        def __call__(self, ids, scores, **kw):
            return PT.search(tok.decode(ids[0, self.n:])) is not None

    def prompt(target):
        user = ("\nPlease generate the next move according to the UI screenshot, instruction and previous actions.\n\n"
                f"Instruction: Click {target}. The target is visible on the current screen; click it directly in one action."
                "\n\nPrevious actions:\nNone")
        return (f"<|im_start|>system\n{SYSTEM}<|im_end|>\n<|im_start|>user\n<|vision_start|><|image_pad|><|vision_end|>"
                f"{user}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n")

    def look(img, target):
        b = io.BytesIO()
        img.save(b, format="JPEG", quality=95)
        img = Image.open(io.BytesIO(b.getvalue())).convert("RGB")
        enc = proc(text=[prompt(target)], images=[img], return_tensors="pt")
        pos = [torch.stack([torch.arange(t).repeat_interleave(h * w), torch.arange(h).repeat_interleave(w).repeat(t),
                            torch.arange(w).repeat(h).repeat(t)], dim=-1) for t, h, w in enc["image_grid_thw"].tolist()]
        n = enc["input_ids"].shape[1]
        with torch.no_grad():
            g = model.generate(input_ids=enc["input_ids"].cuda(), attention_mask=enc["attention_mask"].cuda(),
                               pixel_values=enc["pixel_values"].cuda().to(torch.bfloat16),
                               image_grid_thw=enc["image_grid_thw"].cuda(), patch_positions=torch.cat(pos).cuda(),
                               max_new_tokens=96, do_sample=False, pad_token_id=tok.eos_token_id,
                               stopping_criteria=[StopAtPoint(n)])
        m = PT.search(tok.decode(g[0, n:]))
        if not m:
            return None
        x, y = float(m.group(1)), float(m.group(2))
        if not (0 <= x <= 999 and 0 <= y <= 999):
            return None
        W, H = img.size
        return x / 999 * (W - 1), y / 999 * (H - 1)

    def zoom(img, at, z, target):
        W, H = img.size
        cw, ch = W // z, H // z
        x0 = int(min(max(round(at[0] - cw / 2), 0), W - cw))
        y0 = int(min(max(round(at[1] - ch / 2), 0), H - ch))
        p = look(img.crop((x0, y0, x0 + cw, y0 + ch)).resize((W, H), Image.LANCZOS), target)
        return at if p is None else (x0 + p[0] / W * cw, y0 + p[1] / H * ch)

    def hit(p, b):
        return p is not None and b[0] <= p[0] <= b[2] and b[1] <= p[1] <= b[3]

    rows = []
    for cond in ("base", "tuned"):
        merger.load_state_dict(orig if cond == "base" else tuned)
        ctx = model.disable_adapter() if cond == "base" else contextlib.nullcontext()
        with ctx:
            for k, it in enumerate(items):
                img = Image.open(f"/data/eval/shots/{it['shot_file']}").convert("RGB")
                target = it["instruction"].strip().rstrip(".")
                target = target[6:] if target.lower().startswith("click ") else target
                p1 = look(img, target)
                p2 = zoom(img, p1, 2, target) if p1 else None
                p3 = zoom(img, p2, 3, target) if p2 else None
                rows.append({"cond": cond, "id": it["id"], "src": it["src"], "kind": it["kind"], "size_bin": it["size_bin"],
                             "first": hit(p1, it["box"]), "z2": hit(p2, it["box"]), "casc": hit(p3, it["box"]),
                             "point": p3})
                if k % 50 == 0:
                    print(f"{cond} {k}/{len(items)} {time.time() - t0:.0f}s (~${(time.time() - t0) / 3600 * B200_PER_HOUR:.2f})", flush=True)
    with open(f"/data/runs/{run}/eval-{tag}.jsonl", "w") as f:
        for r in rows:
            f.write(json.dumps(r) + "\n")
    data_vol.commit()

    def rate(rs, a):
        return sum(r[a] for r in rs), len(rs)
    summary = {}
    for src in ("desk", "web"):
        for a in ("first", "z2", "casc"):
            b = [r for r in rows if r["cond"] == "base" and r["src"] == src]
            t = [r for r in rows if r["cond"] == "tuned" and r["src"] == src]
            (kb, n), (kt, _) = rate(b, a), rate(t, a)
            gained = sum(1 for x, y in zip(b, t) if y[a] and not x[a])
            lost = sum(1 for x, y in zip(b, t) if x[a] and not y[a])
            m = gained + lost
            p = 1.0 if m == 0 else min(1.0, 2 * sum(math.comb(m, i) for i in range(0, min(gained, lost) + 1)) / 2 ** m)
            summary[f"{src}/{a}"] = {"base": kb, "tuned": kt, "n": n, "gained": gained, "lost": lost, "p": p}
            print(f"{src:4} {a:5} base {kb}/{n} ({100 * kb / n:.1f}%)  tuned {kt}/{n} ({100 * kt / n:.1f}%)  "
                  f"+{gained} -{lost}  p={p:.2g}", flush=True)
    total = time.time() - t0
    print(f"eval done in {total / 60:.1f} min (~${total / 3600 * B200_PER_HOUR:.2f})", flush=True)
    return summary

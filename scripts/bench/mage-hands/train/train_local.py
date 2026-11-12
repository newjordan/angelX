#!/usr/bin/env python3
"""Train Mage hands locally on the verified click-grounding dataset.

Faithful port of the reference trainer (`modal_mage.run_training`) with no
cloud dependency: same prompt, same JPEG q95 request, same answer-token-only
loss, same LoRA targets, same fp32 merger, same gradient checkpointing.

Differences are explicit and VRAM-driven: optional --max-side downscaling,
single-sample loader, and resume from an existing adapter.

It trains only. It does not deploy, promote, or touch the running cockpit.
"""
from __future__ import annotations

import argparse
import io
import json
import math
import os
import random
import sys
import time
from pathlib import Path

import torch
import torch.nn.functional as F
from PIL import Image

TARGETS = r".*language_model\.layers\.\d+\.(self_attn\.(q|k|v|o)_proj|mlp\.(gate|up|down)_proj)"


def build_proc(model_dir: str, tok):
    from transformers import Qwen2VLImageProcessorFast, Qwen2VLVideoProcessor, Qwen2_5_VLProcessor
    tmpl = Path(model_dir) / "chat_template.jinja"
    return Qwen2_5_VLProcessor(
        image_processor=Qwen2VLImageProcessorFast.from_pretrained(model_dir),
        tokenizer=tok,
        video_processor=Qwen2VLVideoProcessor.from_pretrained(model_dir),
        chat_template=tmpl.read_text() if tmpl.exists() else None,
    )


def prompt_for(system: str, target: str) -> str:
    user = ("\nPlease generate the next move according to the UI screenshot, instruction and previous actions.\n\n"
            f"Instruction: Click {target}. The target is visible on the current screen; click it directly in one action."
            "\n\nPrevious actions:\nNone")
    return (f"<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n<|vision_start|><|image_pad|><|vision_end|>"
            f"{user}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n")


def load_image(data: Path, row: dict, max_side: int) -> Image.Image:
    rel = row["screen"]
    path = data / rel if not str(rel).startswith("screens/") else data / rel
    if not path.is_file():
        path = data / "screens" / Path(rel).name
    img = Image.open(path).convert("RGB")
    crop = row.get("crop")
    if crop:
        x0, y0, cw, ch = crop
        img = img.crop((x0, y0, x0 + cw, y0 + ch)).resize((1920, 1080), Image.LANCZOS)
    if max_side and max(img.size) > max_side:
        scale = max_side / max(img.size)
        img = img.resize((max(1, int(img.width * scale)), max(1, int(img.height * scale))), Image.LANCZOS)
    buf = io.BytesIO()
    img.save(buf, format="JPEG", quality=95)
    return Image.open(io.BytesIO(buf.getvalue())).convert("RGB")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True, help="dataset dir with train.jsonl + screens/")
    ap.add_argument("--model", required=True, help="local Mage base model dir")
    ap.add_argument("--out", required=True, help="run output root")
    ap.add_argument("--run", required=True)
    ap.add_argument("--minutes", type=float, default=30)
    ap.add_argument("--lr", type=float, default=1e-4)
    ap.add_argument("--rank", type=int, default=64)
    ap.add_argument("--accum", type=int, default=8)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--max-side", type=int, default=0, help="cap the long image side (0 = native)")
    ap.add_argument("--init-adapter", default="", help="continue from an existing adapter dir")
    ap.add_argument("--limit", type=int, default=0, help="use only the first N train rows (smoke)")
    ap.add_argument("--save-every", type=float, default=10.0, help="minutes between checkpoints")
    ap.add_argument("--no-checkpoint", action="store_true",
                    help="disable gradient checkpointing (faster when VRAM allows)")
    ap.add_argument("--seed", type=int, default=20261101)
    args = ap.parse_args(argv)

    data = Path(args.data).resolve()
    out = Path(args.out).resolve() / args.run
    out.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    deadline = t0 + args.minutes * 60

    from peft import LoraConfig, PeftModel, get_peft_model
    from safetensors.torch import load_file, save_file
    from transformers import AutoModelForCausalLM, AutoTokenizer

    tok = AutoTokenizer.from_pretrained(args.model, trust_remote_code=True)
    proc = build_proc(args.model, tok)
    system = (data / "system.txt").read_text()
    samples = [json.loads(line) for line in (data / "train.jsonl").read_text().splitlines() if line.strip()]
    if args.limit:
        samples = samples[:args.limit]
    random.Random(args.seed).shuffle(samples)
    print(f"{len(samples)} samples; max_side={args.max_side or 'native'}; rank={args.rank}", flush=True)

    class DS(torch.utils.data.Dataset):
        def __len__(self):
            return len(samples)

        def __getitem__(self, i):
            s = samples[i]
            img = load_image(data, s, args.max_side)
            enc = proc(text=[prompt_for(system, s["target"])], images=[img], return_tensors="pt")
            ans = tok(s["answer"] + "<|im_end|>\n", add_special_tokens=False, return_tensors="pt")["input_ids"]
            pos = []
            for t, h, w in enc["image_grid_thw"].tolist():
                pos.append(torch.stack([torch.arange(t).repeat_interleave(h * w),
                                        torch.arange(h).repeat_interleave(w).repeat(t),
                                        torch.arange(w).repeat(h).repeat(t)], dim=-1))
            return {"input_ids": torch.cat([enc["input_ids"], ans], dim=1), "n_ans": ans.shape[1],
                    "pixel_values": enc["pixel_values"], "image_grid_thw": enc["image_grid_thw"],
                    "patch_positions": torch.cat(pos)}

    loader = torch.utils.data.DataLoader(DS(), batch_size=None, shuffle=False, num_workers=args.workers,
                                         prefetch_factor=2 if args.workers else None,
                                         persistent_workers=bool(args.workers))

    model = AutoModelForCausalLM.from_pretrained(args.model, trust_remote_code=True, dtype=torch.bfloat16,
                                                 attn_implementation="sdpa").cuda()
    model.config.use_cache = False
    model.requires_grad_(False)
    if args.init_adapter:
        model = PeftModel.from_pretrained(model, args.init_adapter, is_trainable=True)
        print(f"continued from adapter {args.init_adapter}", flush=True)
    else:
        model = get_peft_model(model, LoraConfig(r=args.rank, lora_alpha=2 * args.rank,
                                                 lora_dropout=0.05, target_modules=TARGETS))
    inner = model.base_model.model
    merger = {n: p for n, p in model.named_parameters() if ".visual.merger." in n}
    for p in merger.values():
        p.data = p.data.float()
        p.requires_grad_(True)
    lora = [p for n, p in model.named_parameters() if "lora_" in n]
    for p in lora:
        p.data = p.data.float()
    if args.init_adapter:
        merger_file = Path(args.init_adapter) / "merger.safetensors"
        if merger_file.is_file():
            saved = load_file(str(merger_file))
            hits = 0
            for name, p in merger.items():
                key = "visual." + name.split(".visual.", 1)[1] if ".visual." in name else name
                if key in saved:
                    p.data.copy_(saved[key].to(p.device, torch.float32))
                    hits += 1
            print(f"loaded {hits} merger tensors from {merger_file}", flush=True)
    if not args.no_checkpoint:
        inner.model.language_model.gradient_checkpointing_enable(
            gradient_checkpointing_kwargs={"use_reentrant": False})
    else:
        inner.model.language_model.gradient_checkpointing_disable()
        print("gradient checkpointing disabled", flush=True)
    opt = torch.optim.AdamW([{"params": lora, "lr": args.lr},
                             {"params": list(merger.values()), "lr": args.lr * 0.2}], weight_decay=0.0)
    base_lrs = [g["lr"] for g in opt.param_groups]
    print(f"loaded in {time.time() - t0:.0f}s; lora {sum(p.numel() for p in lora)/1e6:.1f}M; "
          f"merger {sum(p.numel() for p in merger.values())/1e6:.1f}M", flush=True)

    def save(tag: str):
        dest = out / tag
        model.save_pretrained(str(dest))
        save_file({"visual." + n.split(".visual.", 1)[1]: p.detach().to(torch.bfloat16).cpu().contiguous()
                   for n, p in merger.items()}, str(dest / "merger.safetensors"))
        print(f"saved {dest}", flush=True)

    model.train()
    step = seen = 0
    losses: list[float] = []
    last_save = time.time()
    oom = False
    try:
        for b in loader:
            if time.time() >= deadline:
                break
            frac = (time.time() - t0) / max(1.0, deadline - t0)
            warm = min(1.0, (step + 1) / 15)
            scale = warm * (0.1 + 0.9 * 0.5 * (1 + math.cos(math.pi * min(1.0, frac))))
            for g, base in zip(opt.param_groups, base_lrs):
                g["lr"] = base * scale
            ids = b["input_ids"].cuda()
            with torch.autocast("cuda", dtype=torch.bfloat16):
                h = inner.model(input_ids=ids, pixel_values=b["pixel_values"].cuda().to(torch.bfloat16),
                                image_grid_thw=b["image_grid_thw"].cuda(),
                                patch_positions=b["patch_positions"].cuda(),
                                use_cache=False, return_dict=True).last_hidden_state
                n = b["n_ans"]
                L = ids.shape[1]
                logits = inner.lm_head(h[:, L - n - 1:L - 1]).float()
            loss = F.cross_entropy(logits.reshape(-1, logits.size(-1)), ids[:, L - n:].reshape(-1))
            (loss / args.accum).backward()
            losses.append(loss.item())
            seen += 1
            if seen % args.accum == 0:
                torch.nn.utils.clip_grad_norm_(lora + list(merger.values()), 1.0)
                opt.step()
                opt.zero_grad(set_to_none=True)
                step += 1
                if step % 5 == 0:
                    el = time.time() - t0
                    print(f"step {step} samples {seen} loss {sum(losses[-args.accum*5:])/len(losses[-args.accum*5:]):.4f} "
                          f"{seen/max(el,1e-9):.2f} samples/s lr {opt.param_groups[0]['lr']:.2e} "
                          f"mem {torch.cuda.max_memory_allocated()/2**30:.1f} GiB elapsed {el/60:.1f} min", flush=True)
                if (time.time() - last_save) / 60 >= args.save_every:
                    # Timestamped snapshots so intermediate checkpoints can be
                    # evaluated independently instead of being overwritten.
                    save(time.strftime("checkpoint-%H%M%S", time.localtime()))
                    last_save = time.time()
    except torch.cuda.OutOfMemoryError as exc:
        oom = True
        print(f"OOM after {seen} samples: {exc}", flush=True)
        torch.cuda.empty_cache()
    if seen % args.accum:
        torch.nn.utils.clip_grad_norm_(lora + list(merger.values()), 1.0)
        opt.step()
        opt.zero_grad(set_to_none=True)
        step += 1
    save("final")
    summary = {"run": args.run, "steps": step, "samples": seen, "minutes": round((time.time() - t0) / 60, 1),
               "samples_per_s": round(seen / max(time.time() - t0, 1e-9), 3),
               "peak_gib": round(torch.cuda.max_memory_allocated() / 2**30, 1),
               "max_side": args.max_side, "rank": args.rank, "oom": oom,
               "grad_checkpointing": not args.no_checkpoint,
               "init_adapter": args.init_adapter or None, "data": str(data),
               "mean_loss": round(sum(losses) / len(losses), 4) if losses else None}
    (out / f"summary-{int(time.time())}.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2), flush=True)
    return 0 if not oom else 3


if __name__ == "__main__":
    sys.exit(main())

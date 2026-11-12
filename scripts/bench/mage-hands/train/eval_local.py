#!/usr/bin/env python3
"""Held-out click accuracy for Mage hands, with and without an adapter.

This is the measurement that decides promotion: given the exact training prompt
and one screenshot, does the model click inside the annotated target box?

Reports both criteria:
  hit_box   click lands inside the gold box (the dataset's own contract)
  hit_15    click within 15/999 normalized units of the gold box centre
            (the reference trainer's sanity criterion)

Run it on `test.jsonl` only. Training must never see these rows.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import time
from collections import Counter, defaultdict
from pathlib import Path

import torch
from PIL import Image

TARGETS = r".*language_model\.layers\.\d+\.(self_attn\.(q|v|k|o)_proj|mlp\.(gate|up|down)_proj)"


def prompt_for(system: str, target: str) -> str:
    user = ("\nPlease generate the next move according to the UI screenshot, instruction and previous actions.\n\n"
            f"Instruction: Click {target}. The target is visible on the current screen; click it directly in one action."
            "\n\nPrevious actions:\nNone")
    return (f"<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n<|vision_start|><|image_pad|><|vision_end|>"
            f"{user}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n")


def parse_coord(text: str):
    for block in re.findall(r"<tool_call>\s*(.*?)\s*</tool_call>", text, re.S):
        try:
            call = json.loads(block)
        except ValueError:
            continue
        args = call.get("arguments", {})
        pt = args.get("coordinate")
        if (args.get("action") == "click" and isinstance(pt, list) and len(pt) == 2
                and all(type(v) in (int, float) and 0 <= v <= 999 for v in pt)):
            return [float(pt[0]), float(pt[1])]
    return None


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--adapter", default="")
    ap.add_argument("--split", default="test")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--max-side", type=int, default=0)
    ap.add_argument("--max-new-tokens", type=int, default=128)
    ap.add_argument("--out", required=True)
    args = ap.parse_args(argv)

    from peft import PeftModel
    from transformers import AutoModelForCausalLM, AutoTokenizer, Qwen2VLImageProcessorFast, Qwen2_5_VLProcessor, Qwen2VLVideoProcessor

    data = Path(args.data).resolve()
    tok = AutoTokenizer.from_pretrained(args.model, trust_remote_code=True)
    tmpl = Path(args.model) / "chat_template.jinja"
    proc = Qwen2_5_VLProcessor(image_processor=Qwen2VLImageProcessorFast.from_pretrained(args.model),
                               tokenizer=tok,
                               video_processor=Qwen2VLVideoProcessor.from_pretrained(args.model),
                               chat_template=tmpl.read_text() if tmpl.exists() else None)
    system = (data / "system.txt").read_text()
    rows = [json.loads(x) for x in (data / f"{args.split}.jsonl").read_text().splitlines() if x.strip()]
    if args.limit:
        # stratified by family so a subset is not one app
        by_family: dict[str, list[dict]] = defaultdict(list)
        for r in rows:
            by_family[r["family"]].append(r)
        per = max(1, args.limit // max(1, len(by_family)))
        rows = [r for fam in sorted(by_family) for r in by_family[fam][:per]]
    print(f"evaluating {len(rows)} rows from {args.split}", flush=True)

    model = AutoModelForCausalLM.from_pretrained(args.model, trust_remote_code=True, dtype=torch.bfloat16,
                                                 attn_implementation="sdpa").cuda()
    if args.adapter:
        model = PeftModel.from_pretrained(model, args.adapter, is_trainable=False)
        merger_file = Path(args.adapter) / "merger.safetensors"
        if merger_file.is_file():
            from safetensors.torch import load_file
            saved = load_file(str(merger_file))
            hits = 0
            for name, p in model.named_parameters():
                key = "visual." + name.split(".visual.", 1)[1] if ".visual." in name else name
                if ".visual.merger." in name and key in saved:
                    p.data.copy_(saved[key].to(p.device, torch.float32))
                    hits += 1
            print(f"merger tensors restored: {hits}", flush=True)
    model.eval()

    results = []
    t0 = time.time()
    for i, row in enumerate(rows):
        rel = row["screen"]
        path = data / rel if (data / rel).is_file() else data / "screens" / Path(rel).name
        img = Image.open(path).convert("RGB")
        crop = row.get("crop")
        if crop:
            x0, y0, cw, ch = crop
            img = img.crop((x0, y0, x0 + cw, y0 + ch)).resize((1920, 1080), Image.LANCZOS)
        if args.max_side and max(img.size) > args.max_side:
            sc = args.max_side / max(img.size)
            img = img.resize((max(1, int(img.width * sc)), max(1, int(img.height * sc))), Image.LANCZOS)
        enc = proc(text=[prompt_for(system, row["target"])], images=[img], return_tensors="pt").to("cuda")
        # The custom model requires explicit patch positions; the processor does
        # not emit them, so derive them exactly as the trainer does.
        pos = []
        for t, h, w in enc["image_grid_thw"].tolist():
            pos.append(torch.stack([torch.arange(t).repeat_interleave(h * w),
                                    torch.arange(h).repeat_interleave(w).repeat(t),
                                    torch.arange(w).repeat(h).repeat(t)], dim=-1))
        enc["patch_positions"] = torch.cat(pos).cuda()
        with torch.no_grad():
            out = model.generate(**enc, max_new_tokens=args.max_new_tokens, do_sample=False,
                                 pad_token_id=tok.pad_token_id or tok.eos_token_id)
        text = tok.decode(out[0][enc["input_ids"].shape[1]:], skip_special_tokens=True)
        got = parse_coord(text)

        box = row["box"]
        w, h = row["width"], row["height"]
        hit_box = hit_15 = False
        if got:
            px = got[0] / 999 * (w - 1)
            py = got[1] / 999 * (h - 1)
            if crop:
                px = crop[0] + px / w * crop[2]
                py = crop[1] + py / h * crop[3]
            hit_box = box[0] <= px <= box[2] and box[1] <= py <= box[3]
            gold = [(box[0] + box[2]) / 2 / (w - 1) * 999, (box[1] + box[3]) / 2 / (h - 1) * 999]
            hit_15 = abs(got[0] - gold[0]) <= 15 and abs(got[1] - gold[1]) <= 15
        results.append({"i": i, "family": row["family"], "target": row["target"], "want": row["box"],
                        "got": got, "hit_box": hit_box, "hit_15": hit_15})
        if (i + 1) % 25 == 0:
            n = i + 1
            print(f"{n}/{len(rows)} box {sum(r['hit_box'] for r in results)/n:.3f} "
                  f"d15 {sum(r['hit_15'] for r in results)/n:.3f} "
                  f"{(time.time()-t0)/n:.2f}s/row", flush=True)

    by_family = defaultdict(lambda: [0, 0, 0])
    for r in results:
        f = by_family[r["family"]]
        f[0] += 1
        f[1] += r["hit_box"]
        f[2] += r["hit_15"]
    summary = {
        "adapter": args.adapter or None, "split": args.split, "rows": len(results),
        "hit_box": round(sum(r["hit_box"] for r in results) / max(1, len(results)), 4),
        "hit_15": round(sum(r["hit_15"] for r in results) / max(1, len(results)), 4),
        "parsed": round(sum(1 for r in results if r["got"]) / max(1, len(results)), 4),
        "minutes": round((time.time() - t0) / 60, 1),
        "by_family": {k: {"n": v[0], "box": round(v[1] / v[0], 3), "d15": round(v[2] / v[0], 3)}
                      for k, v in sorted(by_family.items())},
    }
    Path(args.out).write_text(json.dumps({"summary": summary, "results": results}, indent=2) + "\n")
    print(json.dumps(summary, indent=2), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())

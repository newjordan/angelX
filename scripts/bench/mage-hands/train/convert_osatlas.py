import json, sys, argparse, collections, hashlib

MAX_TARGET = 100

def q(v):
    return max(0, min(999, round(float(v) * 999)))

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--linux"); ap.add_argument("--macos")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    drop = collections.Counter(); per_family = collections.Counter()
    seen = set(); kept = 0; types = collections.Counter()
    with open(a.out, "w") as out:
        for path, family in ((a.linux, "osatlas-linux"), (a.macos, "osatlas-macos")):
            if not path: continue
            for rec in json.load(open(path)):
                img = rec.get("img_filename") or ""
                if not img: drop["no_image"] += 1; continue
                stem = img[:-4] if img.lower().endswith(".png") else img
                for el in (rec.get("elements") or []):
                    tgt = (el.get("instruction") or "").strip()
                    if not tgt: drop["empty_target"] += 1; continue
                    if len(tgt) > MAX_TARGET: drop["target_too_long"] += 1; continue
                    bb = el.get("bbox") or []
                    if len(bb) != 4: drop["bad_bbox_len"] += 1; continue
                    x0, y0, x1, y1 = (max(0.0, min(1.0, float(v))) for v in bb)
                    if x1 <= x0 or y1 <= y0: drop["degenerate_bbox"] += 1; continue
                    key = (stem, tgt)
                    if key in seen: drop["duplicate_screen_target"] += 1; continue
                    seen.add(key)
                    cx, cy = q((x0 + x1) / 2), q((y0 + y1) / 2)
                    call = {"name": "computer_use",
                            "arguments": {"action": "click", "coordinate": [cx, cy], "button": "left"}}
                    out.write(json.dumps({
                        "answer": f"Action: Click {tgt}.\n\n<tool_call>\n{json.dumps(call)}\n</tool_call>",
                        "target": tgt,
                        "screen": stem,
                        "crop": None,
                        "box_norm": [x0, y0, x1, y1],
                        "family": family,
                        "data_type": el.get("data_type"),
                        "group": f"{family}:{hashlib.sha256(stem.encode()).hexdigest()[:12]}",
                        "entities": [],
                        "split": "train",
                        "zoom": 1,
                        "source_dataset": "OS-Copilot/OS-Atlas-data",
                    }) + "\n")
                    kept += 1; per_family[family] += 1; types[el.get("data_type")] += 1
    print(f"rows written : {kept}")
    print(f"\n=== by family ===");      [print(f"  {c:7d}  {k}") for k, c in per_family.most_common()]
    print(f"\n=== dropped ===");        [print(f"  {c:7d}  {k}") for k, c in drop.most_common()]
    print(f"\n=== top element types ==="); [print(f"  {c:7d}  {k}") for k, c in types.most_common(10)]

if __name__ == "__main__":
    main()

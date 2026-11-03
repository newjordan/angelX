import sys, os, json
from PIL import Image, ImageDraw
d = sys.argv[1]; out = sys.argv[2]; per = int(sys.argv[3]) if len(sys.argv) > 3 else 5
anims = [l.split("_sheet")[0] for l in sorted(os.listdir(d)) if l.endswith("_sheet.json")]
rows = []
for a in anims:
    m = json.load(open(f"{d}/{a}_sheet.json")); sh = Image.open(f"{d}/{a}_sheet.png")
    n = len(m["frames"]); idx = [round(i * (n - 1) / (per - 1)) for i in range(per)]
    rows.append((a, [sh.crop((m["frames"][i]["x"], 0, m["frames"][i]["x"] + m["frame_w"], m["frame_h"])) for i in idx]))
fw, fh = rows[0][1][0].size
C = Image.new("RGB", (per * fw + 90, len(rows) * fh), (40, 40, 52)); dr = ImageDraw.Draw(C)
for r, (a, fr) in enumerate(rows):
    dr.text((2, r * fh + 4), a, fill="white")
    for i, f in enumerate(fr): C.paste(f, (90 + i * fw, r * fh), f)
C.save(out); print(C.size)

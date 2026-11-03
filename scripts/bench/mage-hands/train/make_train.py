"""Training samples for Mage-as-hands, dev split only (bench test apps and
test web domains never appear). Each sample is ground's exact request: a
view of the screen (full frame, or a 2x/3x crop scaled back to full size, its
centre jittered off the target like a real cascade step), an instruction in
one of several phrasings, and Mage's own answer format with the box centre on
the [0, 999] frame.
  python3 -I make_train.py <out-dir>
"""
import collections, hashlib, json, os, random, re, shutil, sys
from urllib.parse import urlparse

out = sys.argv[1]
os.makedirs(f"{out}/screens", exist_ok=True)
rng = random.Random(8008)
W, H = 1920, 1080
TEST_APPS = ("disks", "printers", "pinta", "kdenlive", "zenity", "omacalc")
INTERNAL = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)+$")
MAGE = os.environ.get("MAGE_ROOT") or os.path.expanduser("~/mage")
SOURCES = [(f"{MAGE}/bench/desk", "desk"), (f"{MAGE}/train/desk", "desk"),
           (f"{MAGE}/bench/web", "web"), (f"{MAGE}/train/web", "web")]
CAP = {"desk": 45, "web": 25}


def test_domain(url):
    host = urlparse(url).hostname or ""
    dom = ".".join(host.split(".")[-2:])
    return int(hashlib.sha1(dom.encode()).hexdigest(), 16) % 5 < 2


def word(e, src):
    role = e.get("role") or ""
    if src == "web":
        t = e["tag"]
        if t in ("input", "textarea") and e.get("type") not in ("submit", "button", "checkbox", "radio"):
            return "text field"
        return {"a": "link", "select": "dropdown"}.get(t, "tab" if role == "tab" else "button")
    return {"push button": "button", "button": "button", "toggle button": "button", "menu": "menu",
            "menu item": "menu item", "page tab": "tab", "check box": "checkbox", "radio button": "option",
            "combo box": "dropdown", "entry": "text field", "text": "text field", "password text": "text field",
            "spin button": "number field", "link": "link", "table column header": "column header"}.get(role, "item")


def is_icon(e, src):
    w, h = e["box"][2] - e["box"][0], e["box"][3] - e["box"][1]
    if src == "web":
        return not e.get("text")
    return e.get("role") in ("push button", "toggle button", "button") and max(w, h) <= 48 and 0.6 < w / max(h, 1) < 1.6


def phrase(name, kind, icon):
    forms = [f'the "{name}" {kind}', f"the {name} {kind}", f'"{name}"']
    if icon:
        forms += [f"the {name} icon", f"the {name} {kind} in the toolbar"]
    return rng.choice(forms)


def view(cx, cy, z):
    """Crop (x0, y0, cw, ch) 1/z of the frame holding (cx, cy), its centre jittered off the target."""
    if z == 1:
        return None
    cw, ch = W // z, H // z
    jx, jy = rng.uniform(-0.35, 0.35) * cw, rng.uniform(-0.35, 0.35) * ch
    x0 = min(max(round(cx + jx - cw / 2), 0), W - cw)
    y0 = min(max(round(cy + jy - ch / 2), 0), H - ch)
    x0 = min(max(x0, round(cx) - cw + 8), round(cx) - 8)  # keep the target inside with a margin
    y0 = min(max(y0, round(cy) - ch + 8), round(cy) - 8)
    return [min(max(x0, 0), W - cw), min(max(y0, 0), H - ch), cw, ch]


samples, screens, stats = [], {}, collections.Counter()
for root, src in SOURCES:
    pages = json.load(open(f"{root}/pages.json"))
    for p in pages:
        if src == "desk" and any(a in p["name"] for a in TEST_APPS):
            continue
        if src == "web" and (len(p["elements"]) < 8 or test_domain(p["url"])):
            continue
        els = [e for e in p["elements"] if (e.get("text") or e.get("label"))]
        for e in els:
            e["name"] = (e.get("text") or e.get("label")).strip()
        counts = collections.Counter(e["name"].lower() for e in els)
        pool, seen = [], set()
        for e in els:
            n, b = e["name"], e["box"]
            if counts[n.lower()] != 1 or not ((1 if src == "desk" else 2) <= len(n) <= 40) or n.lower() in seen:
                continue
            if INTERNAL.match(n) or "\n" in n:
                continue
            if b[2] - b[0] < 6 or b[3] - b[1] < 6 or b[0] < 0 or b[1] < 0 or b[2] > W or b[3] > H:
                continue
            seen.add(n.lower())
            pool.append(e)
        rng.shuffle(pool)
        pool.sort(key=lambda e: not is_icon(e, src))  # icons first: the weakness
        pool = pool[:CAP[src]]
        sid = hashlib.sha1(f"{root}/{p['shot']}".encode()).hexdigest()[:12]
        screens[sid] = f"{root}/{p['shot']}"
        for e in pool:
            icon = is_icon(e, src)
            cx, cy = (e["box"][0] + e["box"][2]) / 2, (e["box"][1] + e["box"][3]) / 2
            zooms = [1, 2, 3] if (src == "desk" or icon) else [1, 2]
            for z in zooms:
                crop = view(cx, cy, z)
                if crop:
                    x0, y0, cw, ch = crop
                    vx, vy = (cx - x0) * W / cw, (cy - y0) * H / ch
                else:
                    vx, vy = cx, cy
                nx, ny = round(vx / (W - 1) * 999), round(vy / (H - 1) * 999)
                if not (0 <= nx <= 999 and 0 <= ny <= 999):
                    continue
                target = phrase(e["name"], word(e, src), icon)
                answer = (f"Action: Click {target}.\n\n<tool_call>\n"
                          + json.dumps({"name": "computer_use", "arguments": {"action": "click", "coordinate": [nx, ny], "button": "left"}})
                          + "\n</tool_call>")
                samples.append({"screen": sid, "crop": crop, "target": target, "answer": answer,
                                "src": src, "icon": icon, "zoom": z})
                stats[(src, "icon" if icon else "text", z)] += 1

rng.shuffle(samples)
for sid, path in screens.items():
    shutil.copy(path, f"{out}/screens/{sid}.png")
with open(f"{out}/train.jsonl", "w") as f:
    for s in samples:
        f.write(json.dumps(s) + "\n")
shutil.copy(os.environ.get("HANDS_GROUND_PROMPT") or os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "../../../../cockpit/src/agent/tools/hands_ground_prompt.txt"), f"{out}/system.txt")
json.dump(screens, open(f"{out}/screens.json", "w"), indent=1)
print(len(samples), "samples from", len(screens), "screens")
for k, v in sorted(stats.items()):
    print(" ", k, v)

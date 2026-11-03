"""Bench targets from captured pages: unique visible names, stratified by
size bin x kind (text vs icon-only), fixed seed; split dev/test by app family
(desktop) or registrable domain (web) so test never leaks into training.
  python3 -I make_bench_items.py <bench-dir>
"""
import collections, hashlib, json, random, sys
from urllib.parse import urlparse

root = sys.argv[1]
rng = random.Random(20261030)
TEST_APPS = ("disks", "printers", "pinta", "kdenlive", "zenity", "omacalc")
PER_PAGE = {"desk": 12, "web": 6}


def size_bin(b):
    m = min(b[2] - b[0], b[3] - b[1])
    return "xs<16" if m < 16 else "s16-23" if m < 24 else "m24-39" if m < 40 else "l40+"


def kind_of(e, src):
    w, h = e["box"][2] - e["box"][0], e["box"][3] - e["box"][1]
    if src == "web":
        return "text" if e["text"] else "icon"
    if e.get("role") in ("push button", "toggle button", "button") and max(w, h) <= 48 and 0.6 < w / max(h, 1) < 1.6:
        return "icon"
    return "text"


def word(e, src):
    role = e.get("role") or ""
    if src == "web":
        t = e["tag"]
        if t in ("input", "textarea") and e["type"] not in ("submit", "button", "checkbox", "radio"):
            return "text field"
        return {"a": "link", "select": "dropdown"}.get(t, "tab" if role == "tab" else "button")
    return {"push button": "button", "button": "button", "toggle button": "button", "menu": "menu",
            "menu item": "menu item", "page tab": "tab", "check box": "checkbox", "radio button": "option",
            "combo box": "dropdown", "entry": "text field", "text": "text field", "password text": "text field",
            "spin button": "number field", "link": "link", "table column header": "column header"}.get(role, "item")


def split_of(page, src):
    if src == "desk":
        return "test" if any(a in page["name"] for a in TEST_APPS) else "dev"
    host = urlparse(page["url"]).hostname or ""
    dom = ".".join(host.split(".")[-2:])
    return "test" if int(hashlib.sha1(dom.encode()).hexdigest(), 16) % 5 < 2 else "dev"


items = []
for src in ("desk", "web"):
    try:
        pages = json.load(open(f"{root}/{src}/pages.json"))
    except FileNotFoundError:
        continue
    for p in pages:
        if src == "web" and len(p["elements"]) < 8:
            continue  # bot wall or blank render
        els = [e for e in p["elements"] if (e.get("text") or e.get("label"))]
        for e in els:
            e["name"] = e.get("text") or e.get("label")
        counts = collections.Counter(e["name"].lower() for e in els)
        minlen = 1 if src == "desk" else 2
        pool, seen = [], set()
        for e in els:
            n, b = e["name"], e["box"]
            if counts[n.lower()] != 1 or not (minlen <= len(n) <= 40) or n.lower() in seen:
                continue
            if b[2] - b[0] < 6 or b[3] - b[1] < 6 or b[0] < 0 or b[1] < 0 or b[2] > 1920 or b[3] > 1080:
                continue
            seen.add(n.lower())
            pool.append(e)
        strata = collections.defaultdict(list)
        for e in pool:
            strata[(size_bin(e["box"]), kind_of(e, src))].append(e)
        for v in strata.values():
            rng.shuffle(v)
        keys, picked = sorted(strata), []
        while len(picked) < PER_PAGE[src] and any(strata[k] for k in keys):
            for k in keys:
                if strata[k] and len(picked) < PER_PAGE[src]:
                    picked.append(strata[k].pop())
        for i, e in enumerate(picked):
            label_only = src == "web" and not e["text"]
            what = f'the {word(e, src)} labelled "{e["name"]}"' if label_only else f'the "{e["name"]}" {word(e, src)}'
            items.append({"id": f"{src}:{p['name']}:{i}", "src": src, "page": p["name"], "url": p["url"],
                          "theme": p.get("theme", "light" if src == "web" else ""), "split": split_of(p, src),
                          "shot": f"{src}/{p['shot']}", "instruction": f"Click {what}.", "box": e["box"],
                          "size_bin": size_bin(e["box"]), "kind": kind_of(e, src), "role": e.get("role") or e.get("tag")})
json.dump(items, open(f"{root}/items.json", "w"), indent=1)
c = collections.Counter((i["src"], i["split"]) for i in items)
print(len(items), "items", dict(c))
print(dict(collections.Counter((i["src"], i["size_bin"], i["kind"]) for i in items)))

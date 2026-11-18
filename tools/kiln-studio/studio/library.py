import json
import re
import shutil
import time
from pathlib import Path

from . import paths


def slug(text, most=48):
    s = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return s[:most].rstrip("-") or "piece"


def pieces():
    if not paths.LIBRARY.exists():
        return []
    found = []
    for d in sorted(paths.LIBRARY.iterdir()):
        notes = d / "notes.json"
        if notes.exists():
            try:
                found.append(dict(json.loads(notes.read_text()), dir=str(d)))
            except ValueError:
                continue
    return found


def search(query="", limit=8):
    words = set(re.findall(r"[a-z0-9]+", query.lower()))
    scored = []
    for p in pieces():
        text = " ".join([p.get("name", ""), p.get("brief", ""), " ".join(p.get("tags", [])),
                         " ".join(n.get("note", "") for n in p.get("votes", []))]).lower()
        have = set(re.findall(r"[a-z0-9]+", text))
        score = len(words & have) if words else 1
        if score:
            scored.append((score, p.get("added", 0), p))
    scored.sort(key=lambda s: (-s[0], -s[1]))
    return [{k: p.get(k) for k in ("name", "dir", "brief", "tags", "source", "shot", "backend", "votes")}
            for _, _, p in scored[:limit]]


def add(name, source, notes="", tags=(), shot=None, renders=(), brief="", votes=(), origin=None, by="review"):
    d = paths.LIBRARY / slug(name)
    d.mkdir(parents=True, exist_ok=True)
    src = Path(source)
    shutil.copyfile(src, d / src.name)
    record = {"name": name, "source": src.name, "brief": brief, "tags": list(tags), "notes": notes,
              "votes": list(votes), "origin": origin or {}, "by": by, "added": round(time.time())}
    if shot:
        shutil.copyfile(shot, d / "shot.png")
        record["shot"] = "shot.png"
    for r in renders:
        r = Path(r)
        if r.exists():
            shutil.copyfile(r, d / r.name)
    record["renders"] = sorted(p.name for p in d.iterdir() if p.suffix == ".png")
    (d / "notes.json").write_text(json.dumps(record, indent=1) + "\n")
    return {"dir": str(d), **record}

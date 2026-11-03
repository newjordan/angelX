"""The critic: a vision pass over a shot, against the brief and the rubric.

Each critic model (claude, codex; both by default, in parallel) looks at the
grain ladder shot with the eyes' findings beside it and answers JSON held to
SCHEMA: a 0-10 score per rubric line, an overall, a summary, strengths,
faults and fixes. The panel's scores are averaged; variants rank by overall.
"""
import json
import threading
from pathlib import Path

from . import models, paths

KEYS = ["silhouette_16", "readability", "palette_light", "proportions", "brief_fit", "craft"]

SCHEMA = {
    "type": "object", "additionalProperties": False,
    "required": ["scores", "overall", "summary", "strengths", "faults", "fixes"],
    "properties": {
        "scores": {"type": "object", "additionalProperties": False, "required": KEYS,
                   "properties": {k: {"type": "integer"} for k in KEYS}},
        "overall": {"type": "number"},
        "summary": {"type": "string"},
        "strengths": {"type": "array", "items": {"type": "string"}},
        "faults": {"type": "array", "items": {"type": "string"}},
        "fixes": {"type": "array", "items": {"type": "string"}},
    },
}

DEFAULT = ("claude", "codex")


def rubric():
    try:
        return json.loads(paths.BIBLE_JSON.read_text())["rubric"]
    except (OSError, ValueError, KeyError):
        return {k: k for k in KEYS}


def prompt_for(brief, facts=None, findings=None, image_name="shot.png"):
    lines = "\n".join(f"- {k}: {v}" for k, v in rubric().items())
    extra = ""
    if facts:
        extra += "\nThe eyes' measurements (exact, from the pixels):\n" + json.dumps(facts) + "\n"
    if findings is not None:
        extra += "\nThe lints' findings: " + (json.dumps(findings) if findings else "none") + "\n"
    return f"""You are the art critic for the Delve, a co-op pixel dungeon game drawn in its own 74-ink palette.
Judge one piece of pixel art against its brief, strictly and concretely, as a lead pixel artist would.

The brief:
{brief.strip()}

The picture ({image_name}) is kiln's grain ladder shot, left to right: the piece at the game's own size
(16 px a tile, 1 screen px a pixel), then at grain 1, grain 2 and grain 4 side by side at 64 screen px a
tile on the Crypt floor, then grain 4 zoomed 4x. Rows below it, when present, are the animation's frames
(at 64 px a tile, then at grain 1). Grain 1 and 2 are reductions of the grain-4 drawing.
{extra}
Score each line of the rubric from 0 (fails) to 10 (a shipped game's best), for this piece:
{lines}

overall is your own 0-10 judgement of the piece as game art, not an average. Name faults by where they
are (the skull, the left arm, the sword's tip). fixes are the next edits you would make, most important
first, at most five. Answer with the JSON only."""


def _one(name, png, brief, workdir, facts, findings, budget):
    adapter = models.get(name)
    work = Path(workdir) / name
    work.mkdir(parents=True, exist_ok=True)
    shot = work / "shot.png"
    shot.write_bytes(Path(png).read_bytes())
    prompt = prompt_for(brief, facts, findings, "shot.png")
    if name == "claude":
        ans = adapter.ask(prompt + "\n\nRead shot.png in your folder first.", workdir=work, budget=budget,
                          schema=SCHEMA, label="critic", file_tools=["Read"])
    else:
        ans = adapter.ask(prompt, images=[shot], workdir=work, budget=budget, schema=SCHEMA, label="critic",
                          sandbox="read-only", effort="medium")
    data = ans.data if isinstance(ans.data, dict) and "scores" in ans.data else None
    if data is None and ans.text:
        try:
            t = ans.text
            data = json.loads(t[t.find("{"): t.rfind("}") + 1])
        except ValueError:
            data = None
    if data:
        data["scores"] = {k: max(0, min(10, int(round(float(data["scores"].get(k, 0)))))) for k in KEYS}
        data["overall"] = max(0.0, min(10.0, float(data.get("overall", 0))))
    return ans, data


def critique(png, brief, workdir, panel=None, facts=None, findings=None, ledger=None, budget=420, stage="critic", **tags):
    """The panel's verdict on one shot: {scores, overall, by: {model: verdict}, errors}."""
    names = list(panel or DEFAULT)
    results = {}

    def go(n):
        try:
            results[n] = _one(n, png, brief, workdir, facts, findings, budget)
        except Exception as e:  # a model that can't be reached is reported, not fatal
            results[n] = (models_error(n, e), None)

    threads = [threading.Thread(target=go, args=(n,)) for n in names]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    by, errors = {}, {}
    for n in names:
        ans, data = results[n]
        if ledger:
            ledger.add(stage, ans, critic=n, **tags)
        if data:
            by[n] = data
        else:
            errors[n] = ans.error or "no JSON verdict: " + ans.text[:300]
    verdict = {"by": by, "errors": errors}
    if by:
        verdict["scores"] = {k: round(sum(v["scores"][k] for v in by.values()) / len(by), 2) for k in KEYS}
        verdict["overall"] = round(sum(v["overall"] for v in by.values()) / len(by), 2)
    out = Path(workdir) / "critique.json"
    out.write_text(json.dumps(verdict, indent=1))
    return verdict


def models_error(name, e):
    from .models.base import Answer
    return Answer(model=name, ok=False, error=str(e)[:400])

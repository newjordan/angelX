"""Jobs: a brief in, variants out, ranked and put up for review.

    direct    treebeard expands the brief against the bible and the library (director.json, director.md)
    concept   ComfyUI on the Spark draws reference pictures, when the job asks (--concept)
    build     N agents (claude, codex) draw a variant each, in parallel, each in its own folder with
              kiln's MCP tools, under the job's time budget
    critique  the studio takes each variant's final shot and check, and the critic panel scores it
    review    the review page shows them; people vote

Everything lives in ~/.angelX/kiln/jobs/<id>/: job.json (what was asked), brief.md, director.*,
status.json (each stage and variant, time and calls), calls.jsonl (every model call),
variants/<n>/ (the agent's folder: its source, versions/, shots/, logs, NOTES.md, result.json),
critique.json and votes.json. A stage that finished is not run again by `job resume`.
"""
import json
import os
import re
import shutil
import sys
import threading
import time
from pathlib import Path

from . import backends, critic, crib, library, models, paths
from .backends.pxl import prepare as pxl_prepare
from .models.base import Answer, Ledger
from .tools import Studio, load_bible

AGENT_TOOLS = ["shot", "check", "describe", "render", "compare", "export", "library_search"]


def parse_budget(text):
    """'20m', '90s', '1h' or seconds, to seconds."""
    m = re.fullmatch(r"\s*(\d+(?:\.\d+)?)\s*([smh]?)\s*", str(text))
    if not m:
        raise ValueError(f"budget {text!r}: say 20m, 90s or 1h")
    return int(float(m.group(1)) * {"": 1, "s": 1, "m": 60, "h": 3600}[m.group(2)])


def slug(text, most=40):
    return library.slug(text, most)


class Job:
    def __init__(self, dirpath):
        self.dir = Path(dirpath)
        self.id = self.dir.name
        self.lock = threading.Lock()
        self.ledger = Ledger(self.dir / "calls.jsonl")

    # --- files ---------------------------------------------------------------

    @property
    def spec(self):
        return json.loads((self.dir / "job.json").read_text())

    def status(self):
        p = self.dir / "status.json"
        return json.loads(p.read_text()) if p.exists() else {}

    def update(self, fn):
        with self.lock:
            st = self.status()
            fn(st)
            st["updated"] = round(time.time(), 1)
            tmp = self.dir / "status.json.tmp"
            tmp.write_text(json.dumps(st, indent=1))
            tmp.replace(self.dir / "status.json")
            return st

    def stage(self, name, state, **extra):
        def fn(st):
            s = st.setdefault("stages", {}).setdefault(name, {})
            s["state"] = state
            if state == "running":
                s["started"] = round(time.time(), 1)
            elif "started" in s:
                s["secs"] = round(time.time() - s["started"], 1)
            s.update(extra)
            st["state"] = name if state == "running" else st.get("state")
        return self.update(fn)

    def done(self, name):
        return self.status().get("stages", {}).get(name, {}).get("state") == "done"

    def say(self, *a):
        line = f"[{time.strftime('%H:%M:%S')}] [{self.id}] " + " ".join(str(x) for x in a)
        print(line, file=sys.stderr, flush=True)
        with open(self.dir / "job.log", "a") as f:
            f.write(line + "\n")

    def totals(self):
        calls = [json.loads(l) for l in (self.dir / "calls.jsonl").read_text().splitlines()] \
            if (self.dir / "calls.jsonl").exists() else []
        return {"calls": len(calls), "ok": sum(1 for c in calls if c["ok"]),
                "cost_usd": round(sum(c.get("cost_usd") or 0 for c in calls), 3),
                "model_secs": round(sum(c.get("seconds") or 0 for c in calls), 1),
                "by_model": {m: sum(1 for c in calls if c["model"].split(":")[0] == m)
                             for m in sorted({c["model"].split(":")[0] for c in calls})}}

    # --- the stages ----------------------------------------------------------

    def direct(self):
        spec = self.spec
        self.stage("direct", "running")
        bible = load_bible()
        hits = library.search(spec["brief"], limit=4)
        lessons = paths.LESSONS.read_text()[-3000:] if paths.LESSONS.exists() else ""
        system = ("You are the art director of kiln, the pixel studio of the Delve, a co-op pixel dungeon game. "
                  "Expand a brief into a plan an artist agent follows. Reply with JSON only, these keys: "
                  "name (kebab-case, 2-4 words), kind (figure, prop or vfx), canvas ([w, h] at grain 4, "
                  "64 px a tile), frames (1 for a still, 2-6 for an idle loop, more for an effect), fps, "
                  "pose (one or two sentences), masses (list: the 3-6 big shapes, each with the ramp it uses "
                  "and where it sits), ramps (object: material -> realm ramp keys from the bible's table), "
                  "must (list of what the piece needs to read as the brief), avoid (list), "
                  "concept_prompt (one sentence describing the subject for an image model: no style words).")
        user = (f"Brief: {spec['brief']}\n\nThe style bible's numbers:\n" + json.dumps(
                    {k: bible.get(k) for k in ("grain", "light", "outline", "view", "stature_hd_px", "proportions",
                                               "canvas", "ramps", "signal", "white_core_inks")}) +
                ("\n\nLibrary pieces people kept that match:\n" + json.dumps(hits) if hits else "") +
                ("\n\nLessons from reviews:\n" + lessons if lessons else ""))
        adapter = models.get("treebeard")
        ans = adapter.ask(user, system=system, json_out=True, budget=120, max_tokens=1800)
        self.ledger.add("direct", ans)
        plan = ans.data if ans.ok and isinstance(ans.data, dict) else None
        if not plan:
            self.say("director: treebeard gave no plan:", ans.error[:200], "- using the bible's defaults")
            plan = {}
        plan = self._settle(plan, spec, bible)
        (self.dir / "director.json").write_text(json.dumps(plan, indent=1))
        (self.dir / "director.md").write_text(director_md(plan, spec, hits, bible))
        self.stage("direct", "done", by="treebeard" if ans.ok and ans.data else "defaults")
        self.say("director:", plan["name"], plan["kind"], plan["canvas"], f"{plan['frames']} frame(s)", "->", plan["source"])
        return plan

    def _settle(self, plan, spec, bible):
        """The director's plan held to what the job fixed and the bible allows."""
        kind = spec.get("kind") or plan.get("kind") or "figure"
        if kind not in ("figure", "prop", "vfx"):
            kind = "figure"
        name = slug(spec.get("name") or plan.get("name") or spec["brief"], 32)
        backend = spec.get("backend") or backends.pick(kind).name
        b = backends.get(backend)
        canvas = plan.get("canvas") if isinstance(plan.get("canvas"), list) and len(plan.get("canvas")) == 2 else None
        canvas = [int(c) for c in (canvas or bible.get("canvas", {}).get("figure", [64, 64]))]
        canvas = [max(16, min(256, c - c % 4)) for c in canvas]
        from .realm import realm
        pal = realm()
        named = dict(bible.get("ramps", {}), signal="0123w")
        ramps = {}
        for k, v in (plan.get("ramps") or {}).items():
            if not isinstance(v, str):
                continue
            # A ramp named by a word ("bone", "signal") is the bible's; else realm keys only.
            v = named.get(v.strip().lower(), v)
            ramps[k] = "".join(c for c in v if c in pal.rgb)
        frames = plan.get("frames") if isinstance(plan.get("frames"), int) else (1 if kind != "vfx" else 16)
        if kind == "vfx":
            # An effect moves: 8 frames at least. It fits round one figure: 128 px a side at most.
            frames = max(8, frames)
            canvas = [min(128, c) for c in canvas]
        return {**plan, "name": name, "kind": kind, "backend": backend, "source": f"{name}{b.ext}",
                "canvas": canvas, "frames": max(1, min(64, frames)), "fps": plan.get("fps") or (6 if kind != "vfx" else 15),
                "ramps": {k: v for k, v in ramps.items() if v}}

    def concept(self, plan):
        self.stage("concept", "running")
        prompt = plan.get("concept_prompt") or self.spec["brief"]
        ans = models.get("comfy").ask(prompt, workdir=self.dir / "concepts", n=2, kind=plan["kind"])
        self.ledger.add("concept", ans)
        if not ans.ok:
            self.say("concept: ComfyUI failed:", ans.error)
            self.stage("concept", "failed", error=ans.error)
            return []
        res = json.loads(ans.text)
        self.stage("concept", "done", files=res["files"], start_secs=res.get("start_secs"))
        self.say("concept:", len(res["files"]), "pictures in", round(ans.seconds), "s")
        return res["files"] + res.get("realm", [])

    def variant_dir(self, n):
        return self.dir / "variants" / str(n)

    def variant_plan(self, n, plan):
        """The plan for variant n: the backends the job named are given out in
        turn (a VFX job: one variant to the vfx cartridge, others to kiln); one
        that is unavailable falls back to the kind's preferred backend."""
        chosen = self.spec.get("backends") or [plan["backend"]]
        want = chosen[(n - 1) % len(chosen)]
        b = backends.get(want)
        ok, why = backends.available(b, plan["kind"])
        note = ""
        if not ok:
            fallback = backends.pick(plan["kind"])
            note = f"{want} is unavailable ({why}); this variant draws with {fallback.name}"
            b = fallback
        if b.name == "kiln" and plan["kind"] == "vfx":
            w, h = plan["canvas"]
            note += (f" kiln measures in game pixels (16 a tile): your `canvas` is [{w // 4}, {h // 4}] "
                     f"for the director's {w}x{h} at grain 4.")
        elif b.name == "kiln":
            w, h = plan["canvas"]
            note += (f" kiln measures in game pixels (16 a tile): the director's {w}x{h} at grain 4 is a "
                     f"[{w // 4}, {h // 4}] canvas; widen it for a weapon as the knight's [28, 20] does. Name the "
                     f"idle clip `idle`.")
        elif b.name == "vfx":
            note += " The cartridge's `w` and `h` are picture pixels at grain 4 (64 a tile)."
        return dict(plan, backend=b.name, source=plan["name"] + b.ext, backend_note=note.strip())

    def prepare_variant(self, n, plan, refs):
        d = self.variant_dir(n)
        d.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(self.dir / "brief.md", d / "brief.md")
        director = (self.dir / "director.md").read_text()
        if plan["source"] not in director or plan.get("backend_note"):
            director = director.replace("## The big masses", f"**This variant** draws with {plan['backend']}: write "
                                        f"`{plan['source']}`. {plan.get('backend_note', '')}\n\n## The big masses", 1)
        (d / "director.md").write_text(director)
        shutil.copyfile(paths.SKILL, d / "SKILL.md")
        shutil.copyfile(paths.BIBLE, d / "BIBLE.md")
        shutil.copyfile(paths.BIBLE_JSON, d / "bible.json")
        if paths.LESSONS.exists():
            shutil.copyfile(paths.LESSONS, d / "lessons.md")
        crib_src = self.dir / f"crib-{plan['backend']}.md"
        if not crib_src.exists():
            crib.write(plan["backend"], crib_src, kind=plan["kind"])
        shutil.copyfile(crib_src, d / "crib.md")
        if plan["backend"] == "pxl":
            pxl_prepare(d)
        for r in refs:
            (d / "refs").mkdir(exist_ok=True)
            shutil.copyfile(r, d / "refs" / Path(r).name)
        return d

    def agent_prompt(self, n, agent, plan, budget):
        spec = self.spec
        minutes = max(1, budget // 60)
        lookup = f" `node {paths.PXL} help <op>` looks a pxl op up." if plan["backend"] == "pxl" else ""
        shell_note = ("You have a shell, but use it only to read files and look things up." + lookup +
                      " Draw and look through the kiln MCP tools. Never run git.") if agent == "codex" else \
                     "Your tools are the file tools (in this folder only) and the kiln MCP tools."
        return f"""You are variant {n} of kiln job {self.id}, drawing for the Delve.

First read SKILL.md in this folder and follow it. Then brief.md, director.md, crib.md and BIBLE.md.

The brief: {spec['brief']}

Write `{plan['source']}` ({plan['backend']} source) in this folder; the asset named `{plan['name']}` in it is
the piece (with frames, make `{plan['name']}` the animation itself and name helper parts `_...`). The
director's plan: {plan['canvas'][0]}x{plan['canvas'][1]} at grain 4, {plan['frames']} frame(s). {shell_note}

You have {minutes} minutes. Look at every shot you take. When you stop, write NOTES.md and reply in
two or three sentences."""

    def build_one(self, n, agent, plan, budget, refs):
        plan = self.variant_plan(n, plan)
        d = self.prepare_variant(n, plan, refs)
        brief_path = d / "brief.md"
        tools = AGENT_TOOLS + (["concept"] if self.spec.get("concept") else [])
        mcp = [sys.executable, str(paths.STUDIO / "kiln-studio"), "mcp", "--root", str(d), "--kind", plan["kind"],
               "--brief", str(brief_path), "--tools", ",".join(tools), "--deadline", str(round(time.time() + budget))]
        self.update(lambda st: st.setdefault("variants", {}).update(
            {str(n): {"agent": agent, "state": "running", "started": round(time.time(), 1)}}))
        self.say(f"variant {n}: {agent} drawing with {plan['backend']} ({budget // 60} min budget)")
        adapter = models.get(agent)
        prompt = self.agent_prompt(n, agent, plan, budget)
        if (d / plan["source"]).exists():
            prompt += ("\n\nThis is a resumed run: your source, versions/ and shots/ from before are in the folder. "
                       "Look at the last shot and carry on from there.")
        try:
            ans = adapter.ask(prompt, workdir=d, budget=budget, mcp=mcp, tools=tools, label="agent")
        except Exception as e:
            ans = Answer(model=agent, ok=False, error=str(e)[:500])
        self.ledger.add("build", ans, variant=n)
        result = self.finish_variant(n, agent, plan, ans)
        self.say(f"variant {n}: {agent} {result['state']} in {round(ans.seconds)}s, {result.get('shots', 0)} shots, "
                 f"{result.get('versions', 0)} versions" + (f" ({ans.error[:120]})" if ans.error else ""))
        return result

    def finish_variant(self, n, agent, plan, ans):
        d = self.variant_dir(n)
        src = d / plan["source"]
        mcp_calls = [json.loads(l) for l in (d / "mcp.log").read_text().splitlines()] if (d / "mcp.log").exists() else []
        versions = json.loads((d / "versions" / "versions.json").read_text()) if (d / "versions" / "versions.json").exists() else []
        result = {"agent": agent, "backend": plan["backend"], "source": plan["source"], "model": ans.model, "secs": round(ans.seconds, 1), "cost_usd": ans.cost_usd,
                  "tokens": ans.tokens, "turns": ans.turns, "session": ans.session, "answer": ans.text[:2000],
                  "error": ans.error, "denials": ans.denials[:10],
                  "tool_calls": len(mcp_calls), "shots": sum(1 for c in mcp_calls if c["tool"] == "shot"),
                  "tool_errors": sum(1 for c in mcp_calls if c.get("error")), "versions": len(versions)}
        if src.exists():
            studio = Studio(d, kind=plan["kind"], log=d / "studio.log")
            try:
                shot = studio.shot(plan["source"])
                chk = studio.check(plan["source"])
                result.update(state="done" if ans.ok else ("timeout" if "timed out" in ans.error else "partial"),
                              shot=os.path.relpath(shot["shot"], self.dir), render=os.path.relpath(shot["render"], self.dir),
                              frames=shot["frames"], frame=shot["frame"], facts=shot["facts"],
                              check={"ok": chk["ok"], "errors": chk["errors"], "warnings": chk["warnings"],
                                     "findings": chk["findings"][:12]})
            except Exception as e:
                result.update(state="broken", error=(ans.error + " | final render: " + str(e))[:800])
        else:
            result.update(state="failed", error=ans.error or f"no {plan['source']} written")
        (d / "result.json").write_text(json.dumps(result, indent=1))
        self.update(lambda st: st.setdefault("variants", {}).setdefault(str(n), {}).update(
            {k: result.get(k) for k in ("agent", "backend", "state", "secs", "shots", "versions", "tool_calls", "error", "shot", "cost_usd")}))
        return result

    def build(self, plan, refs):
        spec = self.spec
        self.stage("build", "running")
        st = self.status().get("variants", {})
        todo = [(n, spec["agents"][(n - 1) % len(spec["agents"])]) for n in range(1, spec["variants"] + 1)
                if st.get(str(n), {}).get("state") not in ("done",)]
        threads = [threading.Thread(target=self.build_one, args=(n, a, plan, spec["budget_s"], refs)) for n, a in todo]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        states = {k: v.get("state") for k, v in self.status().get("variants", {}).items()}
        drawn = [k for k, v in states.items() if v in ("done", "timeout", "partial")]
        self.stage("build", "done" if drawn else "failed", variants=states)
        return drawn

    def critique(self, plan):
        self.stage("critique", "running")
        spec = self.spec
        brief = (spec["brief"] + "\n\nThe director's plan: " + json.dumps(
            {k: plan.get(k) for k in ("name", "kind", "canvas", "frames", "pose", "must", "avoid")}))
        out = {}
        threads = []
        lock = threading.Lock()

        def go(n, res):
            v = critic.critique(self.dir / res["shot"], brief, self.variant_dir(n) / "critic",
                                panel=spec.get("critics") or critic.DEFAULT, facts=res.get("facts"),
                                findings=(res.get("check") or {}).get("findings"), ledger=self.ledger, variant=int(n))
            with lock:
                out[n] = dict(v, agent=res["agent"])

        for n in sorted(self.status().get("variants", {}), key=int):
            rp = self.variant_dir(n) / "result.json"
            res = json.loads(rp.read_text()) if rp.exists() else {}
            if res.get("shot"):
                threads.append(threading.Thread(target=go, args=(n, res)))
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        ranking = sorted((n for n in out if "overall" in out[n]), key=lambda n: -out[n]["overall"])
        doc = {"variants": out, "ranking": ranking, "panel": spec.get("critics") or list(critic.DEFAULT)}
        (self.dir / "critique.json").write_text(json.dumps(doc, indent=1))
        self.stage("critique", "done" if ranking else "failed", ranking=ranking)
        for n in ranking:
            self.say(f"critic: variant {n} ({out[n]['agent']}) {out[n]['overall']}/10", out[n].get("scores"))
        return doc

    # --- running -------------------------------------------------------------

    def run(self):
        t0 = time.time()
        self.update(lambda st: st.setdefault("started", round(t0, 1)))
        try:
            plan = json.loads((self.dir / "director.json").read_text()) if self.done("direct") else self.direct()
            refs = []
            if self.spec.get("concept"):
                if self.done("concept"):
                    refs = sorted(str(p) for p in (self.dir / "concepts").glob("*.png"))
                else:
                    refs = self.concept(plan)
            if not self.done("build"):
                self.build(plan, refs)
            if not self.done("critique"):
                self.critique(plan)
            final = "review" if self.done("critique") else "failed"
        except Exception as e:
            self.say("failed:", repr(e))
            self.update(lambda st: st.update(error=repr(e)[:800]))
            final = "failed"
        totals = self.totals()
        self.update(lambda st: st.update(state=final, finished=round(time.time(), 1),
                                         wall_secs=round(time.time() - st.get("started", t0), 1), totals=totals))
        self.say(f"{final}: wall {round(time.time() - t0)}s this run; {totals['calls']} model calls "
                 f"({totals['by_model']}), ${totals['cost_usd']} reported")
        return self.status()


def director_md(plan, spec, hits, bible):
    def item(x):
        if isinstance(x, dict):
            key = next((k for k in ("shape", "name", "part", "mass", "what", "label") if x.get(k)), None)
            head = x[key] if key else ""
            rest = "; ".join(f"{v}" for k, v in x.items() if k not in (key, "ramp"))
            ramp = f"`{x['ramp']}`" if x.get("ramp") else ""
            return " ".join(p for p in (f"**{head}**" if head else "", f"({ramp})" if head and ramp else ramp) if p) + \
                (f": {rest}" if rest else "")
        return str(x)

    def items(xs):
        return "\n".join(f"- {item(x)}" for x in (xs or [])) or "- (none)"
    ramps = "\n".join(f"| {k} | `{v}` |" for k, v in plan.get("ramps", {}).items()) or "| (the bible's table) | |"
    lib = "\n".join(f"- {h['name']}: {h.get('dir')}" for h in hits) or "- (nothing kept yet)"
    return f"""# Director: {plan['name']}

The brief: {spec['brief']}

- **Write** `{plan['source']}` ({plan['backend']}); the asset `{plan['name']}` in it is the piece (an animation
  when it has frames; helper parts are named `_...`).
- **Kind** {plan['kind']}; **canvas** {plan['canvas'][0]} x {plan['canvas'][1]} at grain 4 (64 px a tile),
  {plan['frames']} frame(s) at {plan['fps']} fps.
- **Budget** {spec['budget_s'] // 60} minutes for your variant.
- **Pose** {plan.get('pose') or 'standing, three-quarter view facing right'}

## The big masses

{items(plan.get('masses'))}

## Ramps (realm keys, dark to light)

| material | ramp |
|---|---|
{ramps}

## Must read as

{items(plan.get('must'))}

## Avoid

{items(plan.get('avoid'))}
- everything in BIBLE.md's Avoid

## The library

{lib}
"""


def new(brief, variants=2, agents=("claude", "codex"), budget="20m", concept=False, critics=None,
        kind=None, backend=None, name=None, backends_=None):
    for a in agents:
        if a not in ("claude", "codex"):
            raise ValueError(f"agent {a!r}: claude or codex")
    for b in ([backend] if backend else []) + list(backends_ or []):
        backends.get(b)
    jid = time.strftime("%Y%m%d-%H%M%S-") + slug(name or brief, 28)
    d = paths.JOBS / jid
    d.mkdir(parents=True)
    spec = {"id": jid, "brief": brief, "variants": int(variants), "agents": list(agents),
            "budget_s": parse_budget(budget), "concept": bool(concept), "critics": list(critics or critic.DEFAULT),
            "kind": kind, "backend": backend or (backends_[0] if backends_ else None),
            "backends": list(backends_ or []), "name": name, "created": round(time.time(), 1)}
    (d / "job.json").write_text(json.dumps(spec, indent=1))
    (d / "brief.md").write_text(brief.strip() + "\n")
    job = Job(d)
    job.update(lambda st: st.update(id=jid, state="new", stages={}, variants={}))
    return job


def all_jobs():
    if not paths.JOBS.exists():
        return []
    return [Job(d) for d in sorted(paths.JOBS.iterdir(), reverse=True) if (d / "job.json").exists()]


def find(jid):
    d = paths.JOBS / jid
    if (d / "job.json").exists():
        return Job(d)
    hits = [j for j in all_jobs() if j.id.startswith(jid) or jid in j.id]
    if len(hits) == 1:
        return hits[0]
    raise SystemExit(f"no job {jid!r}" if not hits else f"{jid!r} is several jobs: {', '.join(j.id for j in hits)}")

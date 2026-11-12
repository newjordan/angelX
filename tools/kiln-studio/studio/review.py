"""The review page: every job's variants at true size and zoomed, the
critic's notes, and votes (keep, fix, drop) with a note, saved to the job.

    kiln-studio serve [--host 127.0.0.1] [--port 8770]

A keep copies the piece to the library; a note becomes a lesson appended to
~/.angelX/kiln/style/lessons.md, which the next jobs read. The page is one
static file that talks only to the JSON API below with relative URLs, and a
vote names its voter, so the same page can be put behind a host later for
friends to vote remotely.

    GET  /api/jobs                       the jobs, newest first
    GET  /api/jobs/<id>                  one job: brief, director, status, variants, critique, votes
    POST /api/jobs/<id>/vote             {variant, vote: keep|fix|drop, note, voter}
    GET  /api/library                    the kept pieces
    GET  /files/<id>/<path>              a file in a job's folder
    GET  /library/<name>/<file>          a file of a kept piece
    GET  /lighter                        Pixel Art Lighter (also `kiln-studio lighter`)
"""
import json
import mimetypes
import re
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlparse

from . import library, paths
from .jobs import Job, all_jobs

VOTES = ("keep", "fix", "drop")
_vote_lock = threading.Lock()


def job_summary(job):
    spec, st = job.spec, job.status()
    crit = read_json(job.dir / "critique.json") or {}
    rank = crit.get("ranking") or []
    best = None
    if rank:
        res = read_json(job.variant_dir(rank[0]) / "result.json") or {}
        best = res.get("shot")
    votes = read_json(job.dir / "votes.json") or []
    return {"id": job.id, "brief": spec["brief"], "state": st.get("state"), "created": spec.get("created"),
            "variants": spec["variants"], "agents": spec["agents"], "ranking": rank, "best_shot": best,
            "votes": len(votes), "wall_secs": st.get("wall_secs"), "totals": st.get("totals")}


def job_detail(job):
    spec, st = job.spec, job.status()
    crit = read_json(job.dir / "critique.json") or {}
    plan = read_json(job.dir / "director.json") or {}
    variants = []
    for n in range(1, spec["variants"] + 1):
        d = job.variant_dir(n)
        res = read_json(d / "result.json") or {"state": (st.get("variants", {}).get(str(n)) or {}).get("state", "waiting")}
        versions = read_json(d / "versions" / "versions.json") or []
        notes = (d / "NOTES.md").read_text()[:4000] if (d / "NOTES.md").exists() else ""
        variants.append({"n": n, "result": res, "critique": (crit.get("variants") or {}).get(str(n)),
                         "notes": notes, "source": res.get("source") or plan.get("source"),
                         "versions": [{"n": v["n"], "shot": f"variants/{n}/versions/{v['shot']}" if v.get("shot") else None,
                                       "file": f"variants/{n}/versions/{v['file']}"} for v in versions]})
    return {"id": job.id, "spec": spec, "status": st, "director": plan,
            "director_md": (job.dir / "director.md").read_text() if (job.dir / "director.md").exists() else "",
            "variants": variants, "ranking": crit.get("ranking") or [], "panel": crit.get("panel"),
            "votes": read_json(job.dir / "votes.json") or []}


def read_json(p):
    try:
        return json.loads(Path(p).read_text())
    except (OSError, ValueError):
        return None


def vote(job, body):
    n = str(body.get("variant", ""))
    choice = body.get("vote")
    note = str(body.get("note") or "").strip()[:2000]
    voter = re.sub(r"[^\w .@-]", "", str(body.get("voter") or "anon"))[:40] or "anon"
    if choice not in VOTES:
        raise ValueError(f"vote is one of {', '.join(VOTES)}")
    if not n.isdigit() or not 1 <= int(n) <= job.spec["variants"]:
        raise ValueError(f"no variant {n!r}")
    entry = {"variant": int(n), "vote": choice, "note": note, "voter": voter, "t": round(time.time())}
    with _vote_lock:
        votes = read_json(job.dir / "votes.json") or []
        votes.append(entry)
        (job.dir / "votes.json").write_text(json.dumps(votes, indent=1))
    plan = read_json(job.dir / "director.json") or {}
    d = job.variant_dir(n)
    res = read_json(d / "result.json") or {}
    out = {"vote": entry}
    source = res.get("source") or plan.get("source")
    if choice == "keep" and source and (d / source).exists():
        crit = ((read_json(job.dir / "critique.json") or {}).get("variants") or {}).get(n) or {}
        kept = library.add(f"{plan.get('name', 'piece')}-{job.id[:8]}-v{n}", d / source,
                           notes=note, tags=[plan.get("kind", ""), res.get("agent", ""), res.get("backend") or plan.get("backend", "")],
                           shot=job.dir / res["shot"] if res.get("shot") else None,
                           renders=[job.dir / res["render"]] if res.get("render") else [],
                           brief=job.spec["brief"], votes=[v for v in votes if str(v["variant"]) == n],
                           origin={"job": job.id, "variant": int(n), "agent": res.get("agent"),
                                   "critic_overall": crit.get("overall")})
        out["library"] = kept["dir"]
    if note:
        paths.LESSONS.parent.mkdir(parents=True, exist_ok=True)
        with open(paths.LESSONS, "a") as f:
            f.write(f"- {time.strftime('%Y-%m-%d')} {plan.get('name', job.id)} v{n} ({res.get('agent', '?')}), "
                    f"{choice} by {voter}: {note}\n")
        out["lesson"] = str(paths.LESSONS)
    return out


class Handler(BaseHTTPRequestHandler):
    server_version = "kiln-review/0.1"

    def log_message(self, fmt, *args):
        pass

    def send(self, code, body, ctype="application/json"):
        data = body if isinstance(body, bytes) else (json.dumps(body) if ctype == "application/json" else body).encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype + ("; charset=utf-8" if ctype.startswith(("text", "application/json")) else ""))
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def file(self, base, rel):
        base = base.resolve()
        p = (base / rel).resolve()
        if base not in p.parents or not p.is_file():
            return self.send(404, {"error": "no such file"})
        ctype = mimetypes.guess_type(p.name)[0] or "text/plain"
        if p.suffix in (".js", ".md", ".json", ".log", ".txt") or p.name.endswith(".px.js"):
            ctype = "text/plain"
        self.send(200, p.read_bytes(), ctype)

    def job(self, jid):
        if not re.fullmatch(r"[\w.-]+", jid) or not (paths.JOBS / jid / "job.json").exists():
            return None
        return Job(paths.JOBS / jid)

    def do_GET(self):
        path = unquote(urlparse(self.path).path)
        if path in ("/", "/index.html"):
            return self.send(200, PAGE, "text/html")
        if path in ("/lighter", "/lighter/"):
            return self.send(200, lighter_page(), "text/html")
        if path == "/api/jobs":
            return self.send(200, [job_summary(j) for j in all_jobs()])
        if path == "/api/library":
            return self.send(200, library.pieces())
        m = re.fullmatch(r"/api/jobs/([\w.-]+)", path)
        if m:
            job = self.job(m.group(1))
            return self.send(200, job_detail(job)) if job else self.send(404, {"error": "no such job"})
        m = re.fullmatch(r"/files/([\w.-]+)/(.+)", path)
        if m:
            job = self.job(m.group(1))
            return self.file(job.dir, m.group(2)) if job else self.send(404, {"error": "no such job"})
        m = re.fullmatch(r"/library/([\w.-]+)/(.+)", path)
        if m:
            return self.file(paths.LIBRARY / m.group(1), m.group(2))
        self.send(404, {"error": "not found"})

    def do_POST(self):
        m = re.fullmatch(r"/api/jobs/([\w.-]+)/vote", urlparse(self.path).path)
        if not m:
            return self.send(404, {"error": "not found"})
        job = self.job(m.group(1))
        if not job:
            return self.send(404, {"error": "no such job"})
        try:
            n = int(self.headers.get("Content-Length") or 0)
            if n > 65536:
                raise ValueError("too long")
            body = json.loads(self.rfile.read(n) or b"{}")
            return self.send(200, vote(job, body))
        except ValueError as e:
            return self.send(400, {"error": str(e)})


def serve(host="127.0.0.1", port=8770):
    _serve(host, port, Handler, f"kiln review on http://{host}:{port}/  (jobs in {paths.JOBS})")


LIGHTER = Path(__file__).resolve().parent.parent / "lighter" / "Pixel-Art-Lighter.html"


def lighter_page():
    return LIGHTER.read_bytes()


class LighterHandler(BaseHTTPRequestHandler):
    server_version = "kiln-lighter/3.5.22"

    def log_message(self, fmt, *args):
        pass

    def do_GET(self):
        path = unquote(urlparse(self.path).path)
        if path in ("/", "/index.html", "/lighter", "/lighter/"):
            return Handler.send(self, 200, lighter_page(), "text/html")
        self.send_error(404)


def serve_lighter(host="127.0.0.1", port=8771):
    _serve(
        host, port, LighterHandler,
        f"Pixel Art Lighter on http://{host}:{port}/  (export name-Normal.png; rename to <file>_n.png, green up)",
    )


def _serve(host, port, handler, banner):
    httpd = ThreadingHTTPServer((host, port), handler)
    print(banner, flush=True)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass


PAGE = r"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Kiln Review</title>
<style>
:root{--bg:#151614;--panel:#1f1d1a;--line:#322b2b;--ink:#e3d2c3;--dim:#94968c;--keep:#5a7330;--fix:#a97639;--drop:#a8341f;--accent:#639dc2}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:14px/1.45 ui-sans-serif,system-ui,sans-serif}
header{display:flex;gap:16px;align-items:baseline;padding:12px 16px;border-bottom:1px solid var(--line);flex-wrap:wrap}
header h1{font-size:16px;margin:0;letter-spacing:.04em}header a{color:var(--accent);text-decoration:none}
main{padding:16px;max-width:1500px;margin:0 auto}
.jobs{display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:12px}
.card{background:var(--panel);border:1px solid var(--line);border-radius:6px;padding:12px;color:inherit;text-decoration:none;display:block}
.card img{max-width:100%;image-rendering:pixelated;display:block;margin-top:8px}
.meta{color:var(--dim);font-size:12px}.state{font-size:11px;padding:1px 6px;border-radius:9px;border:1px solid var(--line);margin-left:6px}
.variant{background:var(--panel);border:1px solid var(--line);border-radius:6px;padding:14px;margin:14px 0}
.variant h2{margin:0 0 6px;font-size:15px}.rank{color:#ecb64a}
.shot{max-width:100%;image-rendering:pixelated;display:block;margin:8px 0;border:1px solid var(--line)}
.row{display:flex;gap:18px;flex-wrap:wrap;align-items:flex-start}
.anim{image-rendering:pixelated;background-repeat:no-repeat;border:1px solid var(--line);background-color:#3d3a36}
@keyframes play{to{background-position-x:var(--end)}}
table{border-collapse:collapse;font-size:12px}td,th{padding:2px 8px;border-bottom:1px solid var(--line);text-align:left}
ul{margin:4px 0 8px 18px;padding:0}li{margin:2px 0}
.vote{display:flex;gap:8px;flex-wrap:wrap;align-items:center;margin-top:10px}
.vote button{background:var(--line);color:var(--ink);border:1px solid #515349;border-radius:4px;padding:6px 14px;cursor:pointer;font:inherit}
.vote button.keep{background:var(--keep)}.vote button.fix{background:var(--fix)}.vote button.drop{background:var(--drop)}
.vote textarea{flex:1 1 320px;min-height:38px;background:#0b0a08;color:var(--ink);border:1px solid var(--line);border-radius:4px;padding:6px;font:inherit}
.vote input{background:#0b0a08;color:var(--ink);border:1px solid var(--line);border-radius:4px;padding:6px;width:120px;font:inherit}
.zoom button{background:none;border:1px solid var(--line);color:var(--dim);border-radius:3px;cursor:pointer;margin-right:2px}
.zoom button.on{color:var(--ink);border-color:var(--accent)}
.versions{display:flex;gap:6px;flex-wrap:wrap}.versions img{height:72px;image-rendering:pixelated;border:1px solid var(--line)}
pre{white-space:pre-wrap;background:#0b0a08;padding:8px;border-radius:4px;font-size:12px;max-height:260px;overflow:auto}
details summary{cursor:pointer;color:var(--dim);margin:6px 0}.err{color:#de789e}
</style></head><body>
<header><h1><a href="#/">kiln review</a></h1><a href="/lighter">lighter</a><span class="meta" id="where"></span></header>
<main id="app">loading</main>
<script>
const $ = s => document.querySelector(s);
const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;"}[c]));
const get = u => fetch(u).then(r => r.json());
let voter = ""; try { voter = localStorage.getItem("kiln-voter") || "" } catch (e) {}
const KEYS = ["silhouette_16","readability","palette_light","proportions","brief_fit","craft"];

async function home() {
  $("#where").textContent = "jobs";
  const jobs = await get("api/jobs");
  $("#app").innerHTML = jobs.length ? `<div class="jobs">${jobs.map(j => `
    <a class="card" href="#/job/${esc(j.id)}"><b>${esc(j.brief)}</b><span class="state">${esc(j.state)}</span>
    <div class="meta">${esc(j.id)} · ${j.variants} variants by ${esc(j.agents.join(", "))}${j.wall_secs ? " · " + Math.round(j.wall_secs/60) + " min" : ""} · ${j.votes} votes</div>
    ${j.best_shot ? `<img src="files/${esc(j.id)}/${esc(j.best_shot)}" alt="best variant">` : ""}</a>`).join("")}</div>`
    : "No jobs yet: kiln-studio job new \"a brief\"";
}

function anim(job, v, zoom) {
  const r = v.result; if (!r.render || !r.frames) return "";
  const [fw, fh] = r.frame, n = r.frames, fps = (r.fps || 6);
  const w = fw * zoom, h = fh * zoom, sw = fw * n * zoom;
  return `<div class="anim" title="${n} frame(s), ${zoom}x" style="width:${w}px;height:${h}px;background-image:url('files/${esc(job.id)}/${esc(r.render)}');
    background-size:${sw}px ${h}px;--end:-${sw}px;${n > 1 ? `animation:play ${(n / fps).toFixed(2)}s steps(${n}) infinite` : ""}"></div>
    <div class="meta">${n > 1 ? `the loop, ${n} frames at ${fps} fps` : "the piece"}, ${zoom}x</div>`;
}

function verdict(c) {
  if (!c) return `<div class="meta">no critique</div>`;
  const by = c.by || {}, names = Object.keys(by);
  const rows = KEYS.map(k => `<tr><td>${k}</td>${names.map(n => `<td>${by[n].scores[k]}</td>`).join("")}<td><b>${c.scores ? c.scores[k] : ""}</b></td></tr>`).join("");
  const notes = names.map(n => `<details ${names.length === 1 ? "open" : ""}><summary>${esc(n)}: ${esc(by[n].summary)}</summary>
    <b>strengths</b><ul>${(by[n].strengths||[]).map(s => `<li>${esc(s)}</li>`).join("")}</ul>
    <b>faults</b><ul>${(by[n].faults||[]).map(s => `<li>${esc(s)}</li>`).join("")}</ul>
    <b>fixes</b><ul>${(by[n].fixes||[]).map(s => `<li>${esc(s)}</li>`).join("")}</ul></details>`).join("");
  const errs = Object.entries(c.errors || {}).map(([n, e]) => `<div class="err">${esc(n)}: ${esc(e)}</div>`).join("");
  return `<table><tr><th>rubric</th>${names.map(n => `<th>${esc(n)}</th>`).join("")}<th>panel</th></tr>${rows}
    <tr><td><b>overall</b></td>${names.map(n => `<td>${by[n].overall}</td>`).join("")}<td><b>${c.overall ?? ""}</b></td></tr></table>${notes}${errs}`;
}

async function jobPage(id) {
  const job = await get("api/jobs/" + encodeURIComponent(id));
  if (job.error) { $("#app").textContent = job.error; return; }
  $("#where").textContent = job.id;
  const st = job.status, t = st.totals || {};
  const order = [...job.ranking, ...job.variants.map(v => String(v.n)).filter(n => !job.ranking.includes(n))];
  const zoom = Number(sessionStorage.getItem("kiln-zoom") || 4);
  $("#app").innerHTML = `<h2 style="margin:0 0 4px">${esc(job.spec.brief)}</h2>
    <div class="meta">${esc(st.state)} · ${job.spec.variants} variants (${esc(job.spec.agents.join(", "))}) · budget ${Math.round(job.spec.budget_s/60)} min each
    ${st.wall_secs ? " · wall " + Math.round(st.wall_secs) + " s" : ""}${t.calls ? ` · ${t.calls} model calls ${esc(JSON.stringify(t.by_model))}` : ""}
    · critics ${esc((job.panel || job.spec.critics || []).join(", "))}</div>
    <details><summary>the director's plan</summary><pre>${esc(job.director_md)}</pre></details>
    <div class="zoom meta">animation zoom ${[1,2,4,8].map(z => `<button class="${z === zoom ? "on" : ""}" data-z="${z}">${z}x</button>`).join("")}</div>
    ${order.map((n, i) => { const v = job.variants[Number(n) - 1], r = v.result, c = v.critique; const votes = job.votes.filter(x => String(x.variant) === n);
      return `<section class="variant" id="v${n}">
      <h2>${job.ranking.includes(n) ? `<span class="rank">#${i + 1}</span> ` : ""}variant ${n}: ${esc(r.agent || "")}${r.backend ? ` <span class="meta">with ${esc(r.backend)}</span>` : ""} <span class="state">${esc(r.state)}</span>
        ${c && c.overall != null ? `<span class="meta"> critic ${c.overall}/10</span>` : ""}</h2>
      <div class="meta">${r.secs ? Math.round(r.secs) + " s" : ""}${r.shots != null ? ` · ${r.shots} shots, ${r.tool_calls} tool calls, ${r.versions} versions` : ""}
        ${r.cost_usd ? ` · $${r.cost_usd.toFixed(2)}` : ""}${r.check ? ` · check: ${r.check.errors} errors, ${r.check.warnings} warnings` : ""}
        ${r.facts ? ` · ${Object.keys(r.facts.inks || {}).length} inks` : ""}</div>
      ${r.error ? `<div class="err">${esc(r.error)}</div>` : ""}
      ${r.shot ? `<img class="shot" src="files/${esc(job.id)}/${esc(r.shot)}" alt="grain ladder">` : ""}
      <div class="row"><div>${anim(job, v, zoom)}</div>
        <div style="flex:1 1 420px">${verdict(c)}</div></div>
      ${r.check && r.check.findings && r.check.findings.length ? `<details><summary>check findings (${r.check.findings.length})</summary><ul>${r.check.findings.map(f => `<li>${esc(f.level)} ${esc(f.rule)}: ${esc(f.msg)}</li>`).join("")}</ul></details>` : ""}
      ${v.notes ? `<details><summary>the agent's notes</summary><pre>${esc(v.notes)}</pre></details>` : ""}
      ${v.versions.length ? `<details><summary>${v.versions.length} versions</summary><div class="versions">${v.versions.filter(x => x.shot).map(x => `<a href="files/${esc(job.id)}/${esc(x.shot)}" title="v${x.n}"><img src="files/${esc(job.id)}/${esc(x.shot)}" alt="v${x.n}"></a>`).join("")}</div>
        <div class="meta">${v.versions.map(x => `<a href="files/${esc(job.id)}/${esc(x.file)}" style="color:var(--accent)">v${x.n}</a>`).join(" ")}</div></details>` : ""}
      ${votes.length ? `<div class="meta">votes: ${votes.map(x => `<b>${esc(x.vote)}</b> by ${esc(x.voter)}${x.note ? ": " + esc(x.note) : ""}`).join(" · ")}</div>` : ""}
      <div class="vote" data-n="${n}"><input placeholder="your name" value="${esc(voter)}">
        <textarea placeholder="a note: what works, what to fix (it becomes a lesson for the next jobs)"></textarea>
        <button class="keep">keep</button><button class="fix">fix</button><button class="drop">drop</button></div>
      </section>`; }).join("")}`;
  document.querySelectorAll(".zoom button").forEach(b => b.onclick = () => { sessionStorage.setItem("kiln-zoom", b.dataset.z); jobPage(id); });
  document.querySelectorAll(".vote").forEach(box => box.querySelectorAll("button").forEach(b => b.onclick = async () => {
    voter = box.querySelector("input").value.trim(); try { localStorage.setItem("kiln-voter", voter) } catch (e) {}
    const res = await fetch(`api/jobs/${encodeURIComponent(id)}/vote`, {method: "POST", headers: {"Content-Type": "application/json"},
      body: JSON.stringify({variant: Number(box.dataset.n), vote: b.className, note: box.querySelector("textarea").value, voter})}).then(r => r.json());
    if (res.error) alert(res.error); else jobPage(id);
  }));
}

function route() { const m = location.hash.match(/^#\/job\/(.+)$/); m ? jobPage(decodeURIComponent(m[1])) : home(); }
window.addEventListener("hashchange", route); route();
</script></body></html>
"""

"""The kiln-studio command line.

    kiln-studio job new "<brief>" [--variants N] [--agents claude,codex] [--budget 20m] [--concept]
    kiln-studio job list | status <id> | resume <id>
    kiln-studio serve [--port 8770]            the review page on 127.0.0.1
    kiln-studio mcp --root <dir>               the kiln MCP server on stdio
    kiln-studio mcp-config --root <dir>        how to add it to Claude Code or Codex
    kiln-studio shot|check|describe|export <source> [--name N]
    kiln-studio models | backends              what is reachable here
    kiln-studio comfy up|stop|status
"""
import argparse
import json
import os
import subprocess
import sys

from . import paths


def main(argv=None):
    ap = argparse.ArgumentParser(prog="kiln-studio", description="kiln's brain and taste loop: jobs run by the owner's "
                                 "models, the kiln MCP server, the review page.")
    sub = ap.add_subparsers(dest="cmd", required=True)

    j = sub.add_parser("job", help="make, list, watch and resume jobs")
    jsub = j.add_subparsers(dest="jcmd", required=True)
    jn = jsub.add_parser("new", help="a new job from a brief, run now")
    jn.add_argument("brief")
    jn.add_argument("--variants", type=int, default=2)
    jn.add_argument("--agents", default="claude,codex", help="who draws, in turn (claude, codex)")
    jn.add_argument("--budget", default="20m", help="each variant's time (20m, 900s, 1h)")
    jn.add_argument("--concept", action="store_true", help="concept pictures from ComfyUI on the Spark first")
    jn.add_argument("--critics", default="claude,codex", help="the critic panel")
    jn.add_argument("--kind", choices=["figure", "prop", "vfx"])
    jn.add_argument("--backend", choices=["kiln", "pxl", "vfx"], help="every variant's backend")
    jn.add_argument("--backends", help="backends given to the variants in turn, e.g. vfx,kiln")
    jn.add_argument("--name")
    jn.add_argument("--detach", action="store_true", help="run in the background; `job status` to watch")
    jsub.add_parser("list", help="every job, newest first")
    js = jsub.add_parser("status", help="one job's stages, variants, calls and ranking")
    js.add_argument("id")
    js.add_argument("--json", action="store_true")
    jr = jsub.add_parser("resume", help="run what a job has not finished")
    jr.add_argument("id")
    jr.add_argument("--detach", action="store_true")
    jr.add_argument("--redo", choices=["direct", "build", "critique"], help="run this stage (and those after it) again")

    s = sub.add_parser("serve", help="the review page")
    s.add_argument("--port", type=int, default=8770)
    s.add_argument("--host", default="127.0.0.1")

    m = sub.add_parser("mcp", help="the kiln MCP server on stdio")
    m.add_argument("--root", default=".", help="the workspace every path is inside (default: here)")
    m.add_argument("--kind", choices=["figure", "prop", "vfx"])
    m.add_argument("--brief", help="a file holding the brief critique judges against")
    m.add_argument("--tools", help="comma-separated tools to offer (default: all)")
    m.add_argument("--deadline", type=float, help="unix time the caller's budget ends; results say what is left")

    mc = sub.add_parser("mcp-config", help="print how to give Claude Code or Codex the kiln server")
    mc.add_argument("--root", default=".")

    for name in ("shot", "check", "describe", "export"):
        p = sub.add_parser(name, help=f"the {name} tool on a source, here")
        p.add_argument("source")
        p.add_argument("--name")
        p.add_argument("--root", default=".")

    sub.add_parser("models", help="which of the owner's models answer here")
    sub.add_parser("backends", help="which backends are usable here")
    c = sub.add_parser("comfy", help="ComfyUI on the Spark")
    c.add_argument("action", choices=["up", "stop", "status"])

    a = ap.parse_args(argv)
    return COMMANDS[a.cmd](a) or 0


def cmd_job(a):
    from . import jobs
    if a.jcmd == "new":
        job = jobs.new(a.brief, variants=a.variants, agents=[x.strip() for x in a.agents.split(",") if x.strip()],
                       budget=a.budget, concept=a.concept, critics=[x.strip() for x in a.critics.split(",") if x.strip()],
                       kind=a.kind, backend=a.backend, name=a.name,
                       backends_=[x.strip() for x in (a.backends or "").split(",") if x.strip()])
        print(job.id)
        return detach(job) if a.detach else report(job.run())
    if a.jcmd == "list":
        for job in jobs.all_jobs():
            st = job.status()
            spec = job.spec
            rank = st.get("stages", {}).get("critique", {}).get("ranking") or []
            print(f"{job.id:48} {st.get('state', '?'):9} {spec['variants']}x {','.join(spec['agents']):13} "
                  f"{'best v' + rank[0] if rank else '':8} {spec['brief'][:60]}")
        return 0
    job = jobs.find(a.id)
    if a.jcmd == "status":
        st = job.status()
        if a.json:
            print(json.dumps(st, indent=1))
            return 0
        return report(st)
    if a.jcmd == "resume":
        if a.redo:
            order = ["direct", "build", "critique"]
            later = order[order.index(a.redo):]

            def forget(st):
                for s in later:
                    st.get("stages", {}).pop(s, None)
                if "build" in later:
                    st["variants"] = {}
            job.update(forget)
        return detach(job) if a.detach else report(job.run())


def detach(job):
    log = open(job.dir / "run.log", "a")
    subprocess.Popen([sys.executable, str(paths.STUDIO / "kiln-studio"), "job", "resume", job.id], stdout=log,
                     stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True)
    print(f"running in the background: kiln-studio job status {job.id}  (log: {job.dir / 'job.log'})")
    return 0


def report(st):
    print(f"job {st.get('id')}: {st.get('state')}" + (f", wall {st['wall_secs']}s" if st.get("wall_secs") else ""))
    for name, s in (st.get("stages") or {}).items():
        print(f"  {name:9} {s.get('state'):8} {s.get('secs', '')}s" + (f"  {s.get('by')}" if s.get("by") else ""))
    for n, v in sorted((st.get("variants") or {}).items(), key=lambda kv: int(kv[0])):
        print(f"  variant {n}: {v.get('agent')} {v.get('state')} {v.get('secs', '')}s, {v.get('shots') or 0} shots, "
              f"{v.get('versions') or 0} versions" + (f"  {(v.get('error') or '')[:100]}" if v.get("error") else ""))
    t = st.get("totals")
    if t:
        print(f"  calls: {t['calls']} {t['by_model']}, {t['ok']} ok, ${t['cost_usd']} reported, {t['model_secs']}s of model time")
    rank = (st.get("stages") or {}).get("critique", {}).get("ranking")
    if rank:
        print("  ranking: " + " > ".join("v" + r for r in rank))
    return 0 if st.get("state") in ("review", "new", None) else 1


def cmd_serve(a):
    from .review import serve
    serve(a.host, a.port)


def cmd_mcp(a):
    from .mcp_server import serve
    serve(a.root, kind=a.kind, brief=a.brief, allowed=set(a.tools.split(",")) if a.tools else None, deadline=a.deadline)


def cmd_mcp_config(a):
    root = os.path.abspath(a.root)
    exe = str(paths.STUDIO / "kiln-studio")
    print("Claude Code (a file for --mcp-config, or `claude mcp add`):")
    print(json.dumps({"mcpServers": {"kiln": {"command": exe, "args": ["mcp", "--root", root]}}}))
    print(f"  claude mcp add kiln -- {exe} mcp --root {root}")
    print("\nCodex (per call, config.toml untouched):")
    print(f"  codex exec -c 'mcp_servers.kiln.command=\"{exe}\"' -c 'mcp_servers.kiln.args=[\"mcp\",\"--root\",\"{root}\"]' ...")


def cmd_look(a):
    from .backends import BackendError
    from .tools import Studio, ToolError
    studio = Studio(a.root, log=os.path.join(a.root, "studio.log"))
    try:
        res = getattr(studio, a.cmd)(a.source, a.name)
    except (BackendError, ToolError) as e:
        print(f"{a.cmd}: {e}", file=sys.stderr)
        return 1
    print(json.dumps(res, indent=1))


def cmd_models(a):
    from . import models
    for name in models.ALL:
        try:
            ok, what = models.get(name).available()
        except Exception as e:
            ok, what = False, str(e)
        print(f"{name:10} {'yes' if ok else 'no ':3}  {what}")


def cmd_backends(a):
    from . import backends
    for name in backends.BACKENDS:
        ok, what = backends.get(name).available()
        print(f"{name:5} {'yes' if ok else 'no ':3}  {what}")


def cmd_comfy(a):
    from .models import comfy
    if a.action == "up":
        print(f"ComfyUI up ({comfy.up():.1f}s); it stops itself after {comfy.IDLE}s without a job")
    elif a.action == "stop":
        comfy.stop()
        print("ComfyUI stopped")
    else:
        print("answering" if comfy.answers() else "down")


COMMANDS = {"job": cmd_job, "serve": cmd_serve, "mcp": cmd_mcp, "mcp-config": cmd_mcp_config,
            "shot": cmd_look, "check": cmd_look, "describe": cmd_look, "export": cmd_look,
            "models": cmd_models, "backends": cmd_backends, "comfy": cmd_comfy}

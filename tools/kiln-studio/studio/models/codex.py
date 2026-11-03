"""Codex (`codex exec`, gpt-6.1-sol), as the wish painter calls it.

The sandbox is workspace-write rooted in the workdir: it writes there only,
and has no network. kiln's MCP server is added for the one call with `-c`
(the owner's config.toml is never edited) and limited to the tools the job
allows with `enabled_tools`. Images go in with `-i`; a JSON answer is held
to a schema with `--output-schema`.
"""
import json
import os
import shutil
import tempfile
import time
from pathlib import Path

from .. import paths
from .base import Adapter, Answer, files_in, run_logged


class Codex(Adapter):
    name = "codex"
    vision = True
    agentic = True

    def __init__(self, model=None, effort=None):
        self.model = model or os.environ.get("KILN_CODEX_MODEL", "gpt-6.1-sol")
        self.effort = effort or os.environ.get("KILN_CODEX_EFFORT", "medium")
        self.stall = int(os.environ.get("KILN_CODEX_STALL", "300"))

    def available(self):
        exe = shutil.which("codex")
        return (True, exe) if exe else (False, "no `codex` on PATH")

    def ask(self, prompt, images=(), workdir=None, budget=600, mcp=None, tools=None, schema=None,
            label="codex", sandbox="workspace-write", effort=None):
        work = Path(workdir or ".").resolve()
        work.mkdir(parents=True, exist_ok=True)
        last = work / f"{label}.last.txt"
        cmd = ["codex", "exec", "--skip-git-repo-check", "--json", "-m", self.model,
               "-c", f'model_reasoning_effort="{effort or self.effort}"', "-s", sandbox, "-C", str(work),
               "-o", str(last)]
        if mcp:
            cmd += ["-c", f"mcp_servers.kiln.command={json.dumps(mcp[0])}",
                    "-c", f"mcp_servers.kiln.args={json.dumps(mcp[1:])}",
                    "-c", "mcp_servers.kiln.tool_timeout_sec=300"]
            if tools:
                cmd += ["-c", f"mcp_servers.kiln.enabled_tools={json.dumps(list(tools))}"]
        for img in images:
            cmd += ["-i", str(Path(img).resolve())]
        if schema:
            sp = work / f"{label}.schema.json"
            sp.write_text(json.dumps(schema))
            cmd += ["--output-schema", str(sp)]
        cmd.append("-")
        log = work / f"{label}.log"
        (work / f"{label}.prompt.md").write_text(prompt)
        env = dict(paths.node_env(), TMPDIR=os.environ.get("TMPDIR", tempfile.gettempdir()))
        t0 = time.time()
        # The model has been seen to send nothing at all for many minutes (no
        # error, no event): such a run is killed at STALL seconds and tried once more.
        alive = lambda text: '"type":"item.' in text or '"type":"error"' in text or '"type":"turn.failed"' in text
        # A short call (a critic's) gets half its budget before it counts as stalled, so a retry still fits.
        stall = min(self.stall, max(150, budget // 2))
        code, secs = run_logged(cmd, log, stdin_text=prompt, cwd=work, env=env, timeout=budget, alive=alive, stall=stall)
        stalls = 0
        if code == "stalled":
            stalls = 1
            left = budget - (time.time() - t0)
            if left > 120:
                log.rename(work / f"{label}.stalled.log")
                code, _ = run_logged(cmd, log, stdin_text=prompt, cwd=work, env=env, timeout=left, alive=alive, stall=stall)
                stalls += code == "stalled"
            secs = time.time() - t0
        ans = Answer(model=f"codex:{self.model}", seconds=secs, log=str(log), files=files_in(work, t0))
        ans.data = {"stalls": stalls} if stalls else {}
        tokens, turns, errors = 0, 0, []
        for line in log.read_text(errors="replace").splitlines():
            if not line.startswith("{"):
                continue
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            kind = ev.get("type")
            if kind == "thread.started":
                ans.session = ev.get("thread_id", "")
            elif kind == "turn.completed":
                u = ev.get("usage") or {}
                tokens += (u.get("input_tokens") or 0) + (u.get("output_tokens") or 0)
            elif kind == "item.completed" and (ev.get("item") or {}).get("type") in ("mcp_tool_call", "command_execution"):
                turns += 1
            elif kind in ("error", "turn.failed"):
                errors.append(json.dumps(ev)[:400])
        ans.tokens, ans.turns = tokens, turns
        ans.text = last.read_text() if last.exists() else ""
        if code == "stalled" or (stalls and not log.exists()):
            ans.ok, ans.error = False, (f"the model sent nothing for {stall}s, {stalls} time(s) "
                                        "(no error, no event: stalled upstream)")
        elif code is None:
            ans.ok, ans.error = False, f"timed out after {round(secs)}s" + (f" (after {stalls} stall)" if stalls else "")
        elif code:
            ans.ok, ans.error = False, f"exit {code}: " + ("; ".join(errors) or log.read_text()[-800:])
        if schema and ans.text:
            try:
                ans.data = dict(json.loads(ans.text), **({"stalls": stalls} if stalls else {}))
            except ValueError:
                pass
        return ans

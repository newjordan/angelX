"""Claude Code, headless (`claude -p`).

A job agent gets the file tools and kiln's MCP tools, nothing else: no shell,
no web, `--restricted` so its file tools stay in its workdir, `--strict-mcp-
config` so only kiln is loaded, and `--permission-prompts none` so anything
not allowed is refused rather than asked. Images are files it Reads.
"""
import json
import os
import shutil
import time
from pathlib import Path

from .. import paths
from .base import Adapter, Answer, files_in, run_logged

FILE_TOOLS = ["Read", "Write", "Edit", "Glob"]


class Claude(Adapter):
    name = "claude"
    vision = True
    agentic = True

    def __init__(self, model=None, effort=None):
        self.model = model or os.environ.get("KILN_CLAUDE_MODEL")
        self.effort = effort or os.environ.get("KILN_CLAUDE_EFFORT")

    def available(self):
        exe = shutil.which("claude")
        return (True, exe) if exe else (False, "no `claude` on PATH")

    def ask(self, prompt, images=(), workdir=None, budget=600, mcp=None, tools=None, schema=None,
            label="claude", resume=None, max_usd=None, file_tools=None):
        """mcp: the kiln server's argv (rooted in workdir) or None; tools: the
        kiln tool names to allow (all the server offers when None)."""
        work = Path(workdir or ".").resolve()
        work.mkdir(parents=True, exist_ok=True)
        seen = []
        for img in images:
            img = Path(img).resolve()
            if work not in img.parents:
                dst = work / "refs" / img.name
                dst.parent.mkdir(exist_ok=True)
                shutil.copyfile(img, dst)
                img = dst
            seen.append(str(img.relative_to(work)))
        if seen:
            prompt += "\n\nLook at these images (Read each one): " + ", ".join(seen)
        file_tools = list(file_tools or FILE_TOOLS)
        allowed = list(file_tools)
        cmd = ["claude", "-p", "--output-format", "json", "--restricted", "--tools", ",".join(file_tools),
               "--strict-mcp-config", "--permission-prompts", "none"]
        if mcp:
            cfg = work / f"{label}.mcp.json"
            cfg.write_text(json.dumps({"mcpServers": {"kiln": {"command": mcp[0], "args": mcp[1:]}}}))
            cmd += ["--mcp-config", str(cfg)]
            allowed += [f"mcp__kiln__{t}" for t in tools] if tools else ["mcp__kiln"]
        cmd += ["--allowedTools", " ".join(allowed)]
        if self.model:
            cmd += ["--model", self.model]
        if self.effort:
            cmd += ["--effort", self.effort]
        if schema:
            cmd += ["--json-schema", json.dumps(schema)]
        if max_usd:
            cmd += ["--max-budget-usd", str(max_usd)]
        if resume:
            cmd += ["--resume", resume]
        log = work / f"{label}.log"
        (work / f"{label}.prompt.md").write_text(prompt)
        t0 = time.time()
        env = dict(paths.node_env())
        code, secs = run_logged(cmd, log, stdin_text=prompt, cwd=work, env=env, timeout=budget)
        ans = Answer(model=self.name + (f":{self.model}" if self.model else ""), seconds=secs, log=str(log),
                     files=files_in(work, t0))
        if code is None:
            ans.ok, ans.error = False, f"timed out after {budget}s"
            return ans
        raw = log.read_text()
        try:
            data = json.loads(raw[raw.find("{"):]) if "{" in raw else {}
        except ValueError:
            data = {}
        if not data:
            ans.ok, ans.error = False, f"exit {code}: {raw.strip()[-800:]}"
            return ans
        ans.text = data.get("result") or ""
        ans.cost_usd = data.get("total_cost_usd") or 0
        ans.turns = data.get("num_turns") or 0
        ans.session = data.get("session_id") or ""
        usage = data.get("usage") or {}
        ans.tokens = sum(usage.get(k) or 0 for k in ("input_tokens", "output_tokens", "cache_read_input_tokens",
                                                     "cache_creation_input_tokens"))
        ans.denials = data.get("permission_denials") or []
        if data.get("structured_output") is not None:
            ans.data = data["structured_output"]
        if data.get("is_error") or code:
            ans.ok = False
            ans.error = f"exit {code}: {data.get('subtype') or ''} {ans.text[:600]}".strip()
        return ans

"""The kiln MCP server: kiln's hands and eyes as tools, over stdio.

Newline-delimited JSON-RPC 2.0, as MCP's stdio transport has it: initialize,
ping, tools/list and tools/call; diagnostics go to stderr. Every path a tool
takes is inside the server's root (`--root`, default the current folder);
the library is read from anywhere.

    kiln-studio mcp --root <dir> [--kind figure|vfx] [--brief <file>] [--tools a,b] [--deadline <unix time>]

With a deadline (a job's budget), every result says how much time is left.

A Claude Code or Codex agent gets it as an MCP server named `kiln`.
"""
import base64
import json
import os
import sys
import time
import traceback

from .backends import BackendError
from .tools import Studio, ToolError

VERSION = "0.1.0"
PROTOCOLS = ("2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25")

SOURCE = {"type": "string", "description": "the source file, relative to the workspace: <name>.px.js (pxl), <name>.vfx.json (vfx) or <name>.kiln.json (kiln)"}
NAME = {"type": "string", "description": "the asset in the source; the first one when left out"}

TOOLS = [
    {"name": "render", "description": "Render an asset. Returns its strip's path, frames, size, grain and facts (box, inks used, lone pixels, how it reads at grain 1). Keeps a version of the source when it changed.",
     "inputSchema": {"type": "object", "properties": {"source": SOURCE, "name": NAME}, "required": ["source"], "additionalProperties": False}},
    {"name": "shot", "description": "Render and write the grain ladder shot: grain 1 at the game's size, grains 1, 2 and 4 side by side on the Crypt floor at 64 px a tile, a zoom, and every frame. Returns the shot (as an image and its path), the eyes' findings and facts. LOOK at it before you change anything.",
     "inputSchema": {"type": "object", "properties": {"source": SOURCE, "name": NAME, "image": {"type": "boolean", "description": "send the shot as an image too (default true)"}}, "required": ["source"], "additionalProperties": False}},
    {"name": "check", "description": "Lint an asset: the realm's inks only, no half-transparent pixels, lone pixels, the outline on the silhouette, floor contact, the silhouette whole at grain 1, the ink count; then the backend's own checks. ok is false when there are errors.",
     "inputSchema": {"type": "object", "properties": {"source": SOURCE, "name": NAME}, "required": ["source"], "additionalProperties": False}},
    {"name": "describe", "description": "An asset as text: its facts, how each frame moves, the first frame's ink rows at grain 1 (the game's 16 px a tile), and the backend's own description.",
     "inputSchema": {"type": "object", "properties": {"source": SOURCE, "name": NAME}, "required": ["source"], "additionalProperties": False}},
    {"name": "compare", "description": "An asset against a reference PNG (`ref`) or another source (`other`): the backend's row comparison, or pixel and silhouette differences.",
     "inputSchema": {"type": "object", "properties": {"source": SOURCE, "name": NAME, "ref": {"type": "string"}, "other": {"type": "string"}}, "required": ["source"], "additionalProperties": False}},
    {"name": "export", "description": "Export an asset: each frame's PNG at its grain, ink rows at grain 1 (rows.txt), an HD pack entry (pack.json, scale 4), the source, into export/<name>/.",
     "inputSchema": {"type": "object", "properties": {"source": SOURCE, "name": NAME}, "required": ["source"], "additionalProperties": False}},
    {"name": "library_search", "description": "Search the library of pieces people kept (name, brief, tags, votes' notes). Each hit gives its folder, source and shot to read.",
     "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer", "minimum": 1, "maximum": 20}}, "additionalProperties": False}},
    {"name": "library_add", "description": "Keep a piece in the library: its source, its shot, notes and tags.",
     "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}, "source": SOURCE, "notes": {"type": "string"}, "tags": {"type": "array", "items": {"type": "string"}}, "shot": {"type": "string"}}, "required": ["name", "source"], "additionalProperties": False}},
    {"name": "critique", "description": "A vision model scores a shot against the brief and the rubric (silhouette at 16 px, readability at true size, palette and light, proportions, fit to the brief, craft). JSON scores, notes and fixes. Slow (a model call, a minute or two).",
     "inputSchema": {"type": "object", "properties": {"shot": {"type": "string", "description": "a shot PNG in the workspace"}, "brief": {"type": "string", "description": "the brief; the job's own when left out"}, "critic": {"type": "string", "enum": ["claude", "codex"]}}, "required": ["shot"], "additionalProperties": False}},
    {"name": "concept", "description": "Concept images from ComfyUI on the DGX Spark (z-image turbo, 1024 px), written to concepts/. Starts ComfyUI if it is down; it stops itself when idle. A reference to look at, never the asset.",
     "inputSchema": {"type": "object", "properties": {"prompt": {"type": "string"}, "n": {"type": "integer", "minimum": 1, "maximum": 4}, "seed": {"type": "integer"}}, "required": ["prompt"], "additionalProperties": False}},
]

INSTRUCTIONS = ("kiln draws the Delve's pixel art. Write a source file in the workspace, then call shot and "
                "LOOK at the picture, check, describe, revise. Sources: <name>.px.js (pxl: use(\"realm.hex\")), "
                "<name>.vfx.json (the vfx cartridge's recipe, realm ramps only). The skill and the style bible "
                "say how.")


class Server:
    def __init__(self, studio, allowed=None, out=sys.stdout, deadline=None):
        self.studio = studio
        self.tools = [t for t in TOOLS if not allowed or t["name"] in allowed]
        self.out = out
        self.deadline = deadline
        self.started = time.time()

    def send(self, msg):
        trace("out", msg)
        self.out.write(json.dumps(msg) + "\n")
        self.out.flush()

    def handle(self, msg):
        method, mid = msg.get("method"), msg.get("id")
        if method is None:
            return None  # a response to something we never asked
        if mid is None:
            return None  # notifications: initialized, cancelled
        try:
            if method == "initialize":
                asked = (msg.get("params") or {}).get("protocolVersion")
                result = {"protocolVersion": asked if asked in PROTOCOLS or (asked or "") > PROTOCOLS[-1] else PROTOCOLS[-1],
                          "capabilities": {"tools": {"listChanged": False}},
                          "serverInfo": {"name": "kiln", "version": VERSION},
                          "instructions": INSTRUCTIONS}
            elif method == "ping":
                result = {}
            elif method == "tools/list":
                result = {"tools": self.tools}
            elif method == "tools/call":
                result = self.call((msg.get("params") or {}).get("name"), (msg.get("params") or {}).get("arguments") or {})
            else:
                return {"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": f"no method {method}"}}
        except Exception as e:  # a bug in the server, not a refused input
            traceback.print_exc(file=sys.stderr)
            return {"jsonrpc": "2.0", "id": mid, "error": {"code": -32603, "message": str(e)[:500]}}
        return {"jsonrpc": "2.0", "id": mid, "result": result}

    def call(self, name, args):
        if name not in {t["name"] for t in self.tools}:
            return text_result(f"no tool {name!r} here", error=True)
        t0 = time.time()
        try:
            want_image = args.pop("image", True) if name == "shot" else False
            res = getattr(self.studio, name)(**args)
        except (ToolError, BackendError, ValueError, TypeError) as e:
            self.studio.log(name, args, error=e, t0=t0)
            return text_result(f"{name}: {e}", error=True)
        self.studio.log(name, args, t0=t0)
        if self.deadline and isinstance(res, dict):
            left = self.deadline - time.time()
            res["clock"] = (f"{mins(time.time() - self.started)} used, {mins(max(0, left))} left of your budget"
                            + ("; finish now: write NOTES.md" if left < 120 else ""))
        content = [{"type": "text", "text": json.dumps(res, indent=1)}]
        if want_image and res.get("shot"):
            data = base64.b64encode(open(res["shot"], "rb").read()).decode()
            content.append({"type": "image", "data": data, "mimeType": "image/png"})
        return {"content": content, "isError": False}


def trace(way, msg):
    """With KILN_MCP_TRACE set to a file, every message in and out is appended to it."""
    path = os.environ.get("KILN_MCP_TRACE")
    if path:
        with open(path, "a") as f:
            f.write(json.dumps({"t": round(time.time(), 3), way: msg})[:4000] + "\n")


def mins(secs):
    return f"{int(secs // 60)}m{int(secs % 60):02d}s"


def text_result(text, error=False):
    return {"content": [{"type": "text", "text": text}], "isError": error}


def serve(root, kind=None, brief=None, allowed=None, deadline=None):
    brief_text = open(brief).read() if brief else None
    studio = Studio(root, kind=kind, brief=brief_text)
    server = Server(studio, allowed=allowed, deadline=deadline)
    print(f"[kiln mcp] root {studio.root}", file=sys.stderr)
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except ValueError:
            server.send({"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "not JSON"}})
            continue
        trace("in", msg)
        batch = msg if isinstance(msg, list) else [msg]
        for m in batch:
            reply = server.handle(m)
            if reply is not None:
                server.send(reply)

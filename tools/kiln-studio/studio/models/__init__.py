"""The owner's models, one interface (studio.models.base.Adapter):

    claude     Claude Code headless (`claude -p`), an agent with kiln's tools
    codex      Codex (`codex exec`, gpt-6.1-sol), an agent with kiln's tools
    treebeard  the local treebeard router, quick text
    comfy      ComfyUI on the DGX Spark, concept images
"""
from .base import Adapter, Answer, Ledger, ModelError


def get(name, **opts):
    if name == "claude":
        from .claude import Claude
        return Claude(**opts)
    if name == "codex":
        from .codex import Codex
        return Codex(**opts)
    if name == "treebeard":
        from .treebeard import Treebeard
        return Treebeard()
    if name == "comfy":
        from .comfy import Comfy
        return Comfy()
    raise ModelError(f"no model {name!r}: claude, codex, treebeard or comfy")


ALL = ("claude", "codex", "treebeard", "comfy")

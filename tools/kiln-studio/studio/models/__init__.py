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

"""Where kiln-studio finds its tools and keeps its data.

Everything a job writes lives under KILN_HOME (default ~/.angelX/kiln). The
repo's own files (the realm's inks, the skill, the style bible) are read from
the checkout this package sits in.
"""
import os
import shutil
from pathlib import Path

STUDIO = Path(__file__).resolve().parent.parent
REPO = STUDIO.parent.parent
REALM_HEX = REPO / "crates" / "kiln" / "assets" / "realm.hex"
SKILL = STUDIO / "skill" / "SKILL.md"
BIBLE = STUDIO / "style" / "BIBLE.md"
BIBLE_JSON = STUDIO / "style" / "bible.json"

HOME = Path(os.environ.get("KILN_HOME", Path.home() / ".angelX" / "kiln"))
JOBS = HOME / "jobs"
LIBRARY = HOME / "library"
LESSONS = HOME / "style" / "lessons.md"

_MISE_NODE = Path.home() / ".local/share/mise/installs/node/26.8.1/bin/node"
NODE = os.environ.get("KILN_NODE") or (str(_MISE_NODE) if _MISE_NODE.exists() else shutil.which("node") or "node")
PXL = os.environ.get("KILN_PXL", str(Path.home() / "tools" / "claude-pxl" / "bin" / "pxl.js"))
VFX = os.environ.get("KILN_VFX", str(Path.home() / ".angelX" / "cartridges" / "vfx" / "bin"))


def kiln_bin():
    """The kiln command (crates/kiln, feature `cli`): KILN_BIN, else `kiln` on
    PATH, else the checkout's own fast build; None when there is none."""
    for found in (os.environ.get("KILN_BIN"), shutil.which("kiln"), str(REPO / "crates" / "target" / "fast" / "kiln")):
        if found and os.access(found, os.X_OK):
            return found
    return None


def kiln_fx_bin():
    """kiln-studio's own kiln effect renderer (tools/kiln-studio/fx-render), or None until built."""
    for found in (os.environ.get("KILN_FX"), str(STUDIO / "fx-render" / "target" / "release" / "kiln-fx")):
        if found and os.access(found, os.X_OK):
            return found
    return None


def node_env():
    """The environment for node tools: the pinned node first on PATH."""
    env = dict(os.environ)
    env["PATH"] = str(Path(NODE).parent) + os.pathsep + env.get("PATH", "")
    return env

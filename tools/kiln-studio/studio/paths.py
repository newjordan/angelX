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
    for found in (os.environ.get("KILN_BIN"), shutil.which("kiln"), str(REPO / "crates" / "target" / "fast" / "kiln")):
        if found and os.access(found, os.X_OK):
            return found
    return None


def kiln_fx_bin():
    for found in (os.environ.get("KILN_FX"), str(STUDIO / "fx-render" / "target" / "release" / "kiln-fx")):
        if found and os.access(found, os.X_OK):
            return found
    return None


def node_env():
    env = dict(os.environ)
    env["PATH"] = str(Path(NODE).parent) + os.pathsep + env.get("PATH", "")
    return env

"""The hands kiln-studio drives, behind one interface.

A backend turns a source file (its own format: a kiln recipe, a pxl
`.px.js`, a vfx recipe) into pictures, and answers check, describe, compare
and export in its own terms. The studio's eyes (studio.realm) then look at
every backend's pictures the same way, so jobs, the critic and the review
page never care which hands drew a piece.

    kiln  crates/kiln, the realm's own renderer: effects through kiln-fx now, figures when its CLI lands
    pxl   ~/tools/claude-pxl (KILN_PXL), pixel art as JavaScript, for sketches until kiln covers a need
    vfx   ~/.angelX/cartridges/vfx, the Bend pixel-VFX cartridge's layered recipes
"""
from .base import Backend, BackendError, Render
from .kiln import KilnBackend
from .pxl import PxlBackend
from .vfx import VfxBackend

BACKENDS = {"kiln": KilnBackend, "pxl": PxlBackend, "vfx": VfxBackend}

# Which backend a job of each kind uses when none is named: kiln once it is
# built, else the sketcher that covers the kind today.
PREFERRED = {"figure": ["kiln", "pxl"], "prop": ["kiln", "pxl"], "vfx": ["kiln", "vfx"]}


def get(name):
    try:
        return BACKENDS[name]()
    except KeyError:
        raise BackendError(f"no backend {name!r}: one of {', '.join(BACKENDS)}") from None


def for_source(path):
    """The backend whose source format a file is in, by its name."""
    name = str(path)
    for b in BACKENDS.values():
        if name.endswith(b.ext):
            return b()
    raise BackendError(f"{path}: not a source any backend reads ({', '.join(b.ext for b in BACKENDS.values())})")


def available(b, kind=None):
    """(ok, why) for a backend, for one kind of job when it draws several."""
    try:
        return b.available(kind)
    except TypeError:
        return b.available()


def pick(kind):
    """The first available backend for a kind of job."""
    for name in PREFERRED.get(kind, PREFERRED["figure"]):
        b = get(name)
        if available(b, kind)[0]:
            return b
    raise BackendError(f"no backend available for {kind}: " +
                       "; ".join(f"{n}: {available(get(n), kind)[1]}" for n in PREFERRED.get(kind, [])))

from .base import Backend, BackendError, Render
from .kiln import KilnBackend
from .pxl import PxlBackend
from .vfx import VfxBackend

BACKENDS = {"kiln": KilnBackend, "pxl": PxlBackend, "vfx": VfxBackend}

PREFERRED = {"figure": ["kiln", "pxl"], "prop": ["kiln", "pxl"], "vfx": ["kiln", "vfx"]}


def get(name):
    try:
        return BACKENDS[name]()
    except KeyError:
        raise BackendError(f"no backend {name!r}: one of {', '.join(BACKENDS)}") from None


def for_source(path):
    name = str(path)
    for b in BACKENDS.values():
        if name.endswith(b.ext):
            return b()
    raise BackendError(f"{path}: not a source any backend reads ({', '.join(b.ext for b in BACKENDS.values())})")


def available(b, kind=None):
    try:
        return b.available(kind)
    except TypeError:
        return b.available()


def pick(kind):
    for name in PREFERRED.get(kind, PREFERRED["figure"]):
        b = get(name)
        if available(b, kind)[0]:
            return b
    raise BackendError(f"no backend available for {kind}: " +
                       "; ".join(f"{n}: {available(get(n), kind)[1]}" for n in PREFERRED.get(kind, [])))

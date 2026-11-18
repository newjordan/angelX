import subprocess
from dataclasses import dataclass, field


class BackendError(Exception):
    """A backend refused a source or failed; the message names the cause."""


@dataclass
class Render:
    png: str
    frame: tuple
    frames: int = 1
    grain: int = 4
    fps: float = 0
    files: list = field(default_factory=list)
    notes: str = ""
    meta: dict = field(default_factory=dict)

    def as_dict(self):
        return {"png": self.png, "frame": list(self.frame), "frames": self.frames, "grain": self.grain,
                "fps": self.fps, "files": self.files, "notes": self.notes, **({"meta": self.meta} if self.meta else {})}


class Backend:
    name = ""
    ext = ""
    kinds = ()
    grain = 4

    def available(self):
        raise NotImplementedError

    def names(self, source):
        raise NotImplementedError

    def render(self, source, name, out_dir):
        raise NotImplementedError

    def check(self, source, name):
        return []

    def describe(self, source, name):
        return ""

    def compare(self, source, name, ref):
        raise BackendError(f"{self.name} cannot compare")

    def template(self, name, kind):
        return ""


def run(cmd, cwd=None, env=None, timeout=120, stdin=None):
    try:
        p = subprocess.run(cmd, cwd=cwd, env=env, input=stdin, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise BackendError(f"{cmd[0]} timed out after {timeout}s") from None
    except FileNotFoundError:
        raise BackendError(f"{cmd[0]} not found") from None
    return p.returncode, p.stdout, p.stderr

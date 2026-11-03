"""The backend interface and what a render returns."""
import subprocess
from dataclasses import dataclass, field


class BackendError(Exception):
    """A backend refused a source or failed; the message names the cause."""


@dataclass
class Render:
    """One asset rendered: a strip of `frames` frames of `frame` size, drawn at `grain`."""
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
    ext = ""  # a source file's suffix
    kinds = ()  # the kinds of job it can draw: figure, prop, vfx
    grain = 4  # the grain its sources draw at

    def available(self):
        """(True, version) when usable, else (False, why not)."""
        raise NotImplementedError

    def names(self, source):
        """The asset names a source defines."""
        raise NotImplementedError

    def render(self, source, name, out_dir):
        """Render one asset into out_dir; a Render."""
        raise NotImplementedError

    def check(self, source, name):
        """The backend's own lints: a list of {rule, level, msg}."""
        return []

    def describe(self, source, name):
        """The backend's own text description of an asset."""
        return ""

    def compare(self, source, name, ref):
        """The asset against a reference PNG, as text."""
        raise BackendError(f"{self.name} cannot compare")

    def template(self, name, kind):
        """A starting source for a new asset, as text."""
        return ""


def run(cmd, cwd=None, env=None, timeout=120, stdin=None):
    """A command's (exit code, stdout, stderr), never raising on a non-zero exit."""
    try:
        p = subprocess.run(cmd, cwd=cwd, env=env, input=stdin, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise BackendError(f"{cmd[0]} timed out after {timeout}s") from None
    except FileNotFoundError:
        raise BackendError(f"{cmd[0]} not found") from None
    return p.returncode, p.stdout, p.stderr

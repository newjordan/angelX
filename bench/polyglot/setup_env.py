"""Toolchain setup and checks: `polyglot.py setup` (installs) and `polyglot.py doctor` (checks only).

What the kit installs, all under the work directory:
  * venv/      Python venv with pytest (and cmake from PyPI if the host has none)
  * js/        the shared Jest/Babel test dependencies, from the pinned lockfile
  * toolbin/   python3/python/pytest (and cmake/ctest) wrappers, first on PATH for
               agents and the grader, so both use the same interpreter
The host must provide git, node + npm, a Rust toolchain (cargo, rustc), make and g++.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

from common import KIT, tool_path, toolbin, work_dir

PYTEST_PIN = "pytest==9.1.1"
CMAKE_PIN = "cmake==4.4.3"
HOST_TOOLS = ["git", "node", "npm", "cargo", "rustc", "make", "g++"]
MIN_FREE_GB = 5


def version(cmd: list[str], env: dict | None = None) -> str:
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=60, env=env)
        text = (out.stdout or out.stderr).strip().splitlines()
        return text[0] if text and out.returncode == 0 else f"FAILED ({out.returncode})"
    except (OSError, subprocess.TimeoutExpired) as err:
        return f"FAILED ({err})"


def wrapper(name: str, target: Path) -> None:
    path = toolbin() / name
    path.write_text(f'#!/bin/sh\nexec "{target}" "$@"\n')
    path.chmod(0o755)


def setup() -> int:
    work = work_dir()
    missing = [t for t in HOST_TOOLS if not shutil.which(t)]
    if missing:
        print(f"missing host tools: {' '.join(missing)}. Install them, then rerun setup.")
        return 1
    toolbin().mkdir(parents=True, exist_ok=True)
    venv = work / "venv"
    if not (venv / "bin" / "python3").exists():
        subprocess.run([sys.executable, "-m", "venv", str(venv)], check=True)
    pip = [str(venv / "bin" / "python3"), "-m", "pip", "install", "--quiet", "--disable-pip-version-check"]
    subprocess.run(pip + [PYTEST_PIN], check=True)
    for name in ("python3", "python"):
        wrapper(name, venv / "bin" / "python3")
    wrapper("pytest", venv / "bin" / "pytest")
    if not shutil.which("cmake"):
        subprocess.run(pip + [CMAKE_PIN], check=True)
        for name in ("cmake", "ctest"):
            wrapper(name, venv / "bin" / name)
    js = work / "js"
    js.mkdir(parents=True, exist_ok=True)
    for name in ("package.json", "package-lock.json"):
        shutil.copyfile(KIT / "js" / name, js / name)
    if not (js / "node_modules" / ".bin" / "jest").exists():
        subprocess.run(["npm", "ci", "--no-audit", "--no-fund", "--loglevel=error"], cwd=js, check=True)
    return doctor()


def doctor() -> int:
    env = {**os.environ, "PATH": tool_path()}
    checks = {
        "python3": version(["python3", "--version"], env),
        "pytest": version(["python3", "-m", "pytest", "--version"], env),
        "node": version(["node", "--version"], env),
        "npm": version(["npm", "--version"], env),
        "jest": "ok" if (work_dir() / "js" / "node_modules" / ".bin" / "jest").exists() else "FAILED (run setup)",
        "cargo": version(["cargo", "--version"], env),
        "cmake": version(["cmake", "--version"], env),
        "make": version(["make", "--version"], env),
        "g++": version(["g++", "--version"], env),
    }
    bad = [k for k, v in checks.items() if v.startswith("FAILED")]
    for key, value in checks.items():
        print(f"  {key:8} {value}")
    tmp = Path(os.environ.get("TMPDIR") or "/tmp")
    free_gb = shutil.disk_usage(tmp).free / 1e9
    print(f"  TMPDIR   {tmp} ({free_gb:.1f} GB free)")
    if free_gb < MIN_FREE_GB:
        print(
            f"  warning: under {MIN_FREE_GB} GB free in TMPDIR. Rust and C++ builds fail with "
            "'No space left' or 'Disk quota exceeded'; point TMPDIR at a bigger disk."
        )
    node_major = checks["node"].lstrip("v").split(".")[0]
    if node_major.isdigit() and int(node_major) < 20:
        print("  warning: Node 20 or newer is expected (the published runs used Node 26).")
    print("doctor: ok" if not bad else f"doctor: FAILED: {', '.join(bad)}")
    return 1 if bad else 0

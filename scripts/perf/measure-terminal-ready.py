#!/usr/bin/env python3
"""Bounded offline PTY readiness/input-echo measurement of an existing image.

No model turn is submitted. Each isolated process group is reaped. `--launcher`
measures the normal source launcher (Cargo check included), not ANGEL_NO_BUILD.
The fixture disables discovery/background work; this is not a Glass/cold-cache
or real-provider startup benchmark. Raw terminal text is never persisted.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time

ROOT = Path(__file__).resolve().parents[2]


def sample(command, env):
    # The fixture HOME is shared by the bounded samples: changing HOME is a
    # Cargo build-script input, not an unchanged startup.
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 140, 0, 0))

    def child_terminal():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    ready = b"ready-probe-329174"
    echo = b"echo-probe-853619"
    command = [*command, "--driver", "practice", "--draft", ready.decode()]
    started = time.perf_counter()
    process = None
    try:
        process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave,
                                   cwd=ROOT, env=env, preexec_fn=child_terminal)
        os.close(slave)
        slave = None
        first = None
        buffer = bytearray()
        echoed_at = None
        ready_at = None
        deadline = started + 15
        while time.perf_counter() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"PTY exited before input echo ({process.returncode})")
            if not select.select([master], [], [], .02)[0]:
                continue
            block = os.read(master, 65536)
            now = time.perf_counter()
            if not block:
                raise RuntimeError("PTY closed before input echo")
            if first is None:
                first = now
            buffer.extend(block)
            if b"\x1b[6n" in buffer:
                os.write(master, b"\x1b[1;1R")
                buffer = buffer.replace(b"\x1b[6n", b"")
            if ready_at is None and ready in buffer:
                ready_at = now
                buffer.clear()
                os.write(master, echo)
                echoed_at = time.perf_counter()
            elif ready_at is not None and echo in buffer:
                return {"first_output_ms": (first - started) * 1000,
                        "draft_visible_ms": (ready_at - started) * 1000,
                        "input_echo_ms": (now - echoed_at) * 1000}
            if len(buffer) > 4 * 1024 * 1024:
                raise RuntimeError("PTY capture exceeded bound")
        raise RuntimeError(f"PTY readiness/input echo timed out (last output: {bytes(buffer[-400:])!r})")
    finally:
        if process is not None:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
        os.close(master)
        if slave is not None:
            os.close(slave)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--image", type=Path, default=ROOT / "cockpit/target/fast/angel")
    parser.add_argument("--launcher", action="store_true")
    args = parser.parse_args()
    with args.output.open("x") as file:
        file.write("{}\n")
    report = {"schema": "angel.terminal-ready/v1", "launcher": args.launcher,
              "scope": "warm offline practice; 140x40 PTY; discovery/background disabled",
              "image_sha256": None,
              "samples": []}
    with args.image.open("rb") as image:
        report["image_sha256"] = hashlib.file_digest(image, "sha256").hexdigest()
    env = dict(TERM="xterm-256color", LANG="C.UTF-8", ANGEL_DRIVER="practice",
               ANGEL_API_CLUBS="none", ANGEL_SETUP="0", ANGEL_GLASS="0", ANGEL_FAST="1",
               ANGEL_BIND_SOURCE="0", ANGEL_BUILD_BACKEND="auto", ANGEL_BACKGROUND_BUILD="0",
               ANGEL_SCAN="0", ANGEL_SCAN_INTERVAL_SECS="0", ANGEL_BAG_PROBE="0",
               ANGEL_HYDRA_DISCOVER="0", ANGEL_HYDRA_PUBLISH="0", ANGEL_LSP="0",
               ANGEL_EXPERIENCE="0", ANGEL_DOSSIER="0", ANGEL_ROUTE_MEMORY="0",
               ANGEL_TUI_MOTION="off", ANGEL_IMAGE_PROTOCOL="halfblocks",
               ANGEL_OVERWATCH_CMD="true", ANGEL_COMP_MODE="1", ANGEL_ATLAS="0",
               ANGEL_BACKPLANE="off", ANGEL_TAILNET_RESOLVE="0", ANGEL_PROBE="0")
    report["fixture_env"] = env
    command = [str(ROOT / "bin/angelX"), "--term"] if args.launcher else [str(args.image.resolve())]
    try:
        with tempfile.TemporaryDirectory(prefix="angel-ready-") as temporary:
            root = Path(temporary)
            fixture = {key: os.environ[key] for key in ("PATH", "CARGO_HOME", "RUSTUP_HOME") if key in os.environ}
            fixture.setdefault("RUSTUP_HOME", str(Path.home() / ".rustup"))
            fixture.setdefault("CARGO_HOME", str(Path.home() / ".cargo"))
            fixture.update(env)
            # Keep source cartridges, browser/media and Glass compiled in. Only
            # runtime state is isolated; using a blank cartridge registry would
            # compare different applications.
            fixture["ANGEL_CARTRIDGES"] = os.environ.get("ANGEL_CARTRIDGES", str(Path.home() / ".angelX/cartridges"))
            for key in ("HOME", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME", "TMPDIR",
                        "ANGEL_SESSION_DIR", "ANGEL_WORKSPACE"):
                path = root / key
                path.mkdir()
                fixture[key] = str(path)
            if args.launcher:
                # Prime the HOME/cartridge build-script transition once, outside
                # the warm observations, and explicitly account for that cost.
                started = time.perf_counter()
                proc = subprocess.Popen([str(ROOT / "bin/angelX"), "--version"],
                                        cwd=ROOT, env=fixture, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, start_new_session=True)
                try:
                    stdout, stderr = proc.communicate(timeout=360)
                except BaseException:
                    try:
                        os.killpg(proc.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    proc.communicate()
                    raise
                report["fixture_prime_seconds"] = time.perf_counter() - started
                report["fixture_prime_exit_code"] = proc.returncode
                if proc.returncode:
                    log = args.output.with_suffix(".prime.log")
                    with log.open("xb") as file:
                        file.write(stderr)
                    report["fixture_prime_log"] = log.name
                    raise RuntimeError(f"fixture prime failed: {stderr[-400:]!r}")
                with args.image.open("rb") as image:
                    report["image_sha256"] = hashlib.file_digest(image, "sha256").hexdigest()
            for _ in range(3):
                row = sample(command, fixture)
                report["samples"].append(row)
                print(json.dumps(row), flush=True)
    finally:
        args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
